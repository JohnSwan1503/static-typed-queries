use proc_macro2::{Literal, Span, TokenStream};
use quote::{ToTokens, format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{
    Field, Fields, GenericArgument, GenericParam, Generics, Ident, ItemStruct, LitStr,
    PathArguments, Type, parse_quote,
};

use crate::analyze::{self, Analysis, Columns, Engine, Kind};
use crate::args::{Args, Placement};
use crate::docs;
use crate::naming::{camel, field_name, short_name, snake_case, to_ident, unique};
use crate::template::{self, Segment, Template};

const RESERVED: &[&str] = &[
    "build", "builder", "finish", "query", "query_as", "run", "run_as",
];

fn krate() -> TokenStream {
    quote!(::static_typed_queries::__private)
}

pub(crate) fn table(args: Args, item: ItemStruct) -> syn::Result<TokenStream> {
    let krate = krate();
    if let Some(sql) = &args.sql {
        return Err(syn::Error::new(sql.span(), "tables take `name`, not `sql`"));
    }
    if args.placement.is_some() {
        return Err(syn::Error::new(
            Span::call_site(),
            "tables are always referenced by name",
        ));
    }
    if let Some(parse_check) = &args.parse_check {
        return Err(syn::Error::new(
            parse_check.span(),
            "tables have no SQL to check",
        ));
    }
    if let Some(grammar) = &args.grammar {
        return Err(syn::Error::new(
            grammar.span(),
            "tables have no SQL to check",
        ));
    }
    if let Some(row) = &args.row {
        return Err(syn::Error::new_spanned(
            row,
            "tables aren't statements; give `row` to a query that selects from it",
        ));
    }
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "tables can't be generic",
        ));
    }
    if !named_fields(&item).is_empty() {
        return Err(syn::Error::new_spanned(
            &item.fields,
            "tables can't have fields; give them to a query that selects from the table to read its rows",
        ));
    }
    let name = args
        .name
        .as_ref()
        .ok_or_else(|| syn::Error::new(Span::call_site(), "tables need `name = \"...\"`"))?;
    let table = name.value();
    let mut parts = Vec::new();
    for (i, segment) in table.split('.').enumerate() {
        if i > 0 {
            parts.push(quote!(#krate::part::lit::Lit::part(".")));
        }
        parts.push(quote!(#krate::part::ident::Ident::part(#segment)));
    }
    let before = hooks(args.before.as_deref())?;
    let after = hooks(args.after.as_deref())?;
    let mut items = Vec::new();
    for ty in before.iter().chain(&after) {
        add_item(ty, &item.generics, &[], &mut items);
    }
    let template = Template {
        segments: Vec::new(),
        params: Vec::new(),
        children: Vec::new(),
    };
    let analysis = Analysis {
        kind: Kind::Query,
        returns_rows: false,
        columns: Columns::default(),
        refs: Vec::new(),
        names: Vec::new(),
    };
    let mut params = Params::new(&template, &analysis, &items, &item, name)?;
    params.runs = false;
    let fields: Vec<Type> = items.iter().map(|(ty, _)| ty.clone()).collect();
    let node_name = table.rsplit('.').next().unwrap_or(&table);
    let ident = &item.ident;
    let dialect = &args.dialect;
    let node = node(
        node_name,
        fingerprint(ident, &item),
        quote!(Table),
        quote!(#krate::node::inject::Inject::Ident),
        parts,
        &fields,
        [&before, &after],
    );
    let check = (!before.is_empty() || !after.is_empty()).then(|| {
        quote! {
            const _: () = #krate::render::check_hooks(<#ident as #krate::sql::Sql>::NODE);
        }
    });
    let params_ty = params.ty();
    let params_struct = params.definition();
    let derives = params.derives();
    let bind_params = params.bind_impl();
    let builder = params.builder(dialect, None);
    let embed_checks = embeds(&item, &fields, dialect);
    let fmt = fmt(&args, &item, false)?;
    let mut documented = item.clone();
    documented
        .attrs
        .extend(params.item_docs(Source::Table(&before, &after)));

    Ok(quote! {
        #documented

        #params_struct
        #derives

        impl #krate::sql::Sql for #ident {
            type Dialect = #dialect;
            type Params = #params_ty;
            const NODE: &'static #krate::node::Node = #node;
        }

        #check
        #embed_checks
        #bind_params
        #builder
        #fmt
    })
}

fn hooks(types: Option<&[Type]>) -> syn::Result<Vec<Type>> {
    let mut hooks: Vec<Type> = Vec::new();
    for ty in types.unwrap_or_default() {
        if hooks.iter().any(|other| type_key(other) == type_key(ty)) {
            return Err(syn::Error::new_spanned(
                ty,
                format!("`{}` is listed twice", docs::type_string(ty)),
            ));
        }
        hooks.push(ty.clone());
    }
    Ok(hooks)
}

fn no_hooks(args: &Args) -> syn::Result<()> {
    match args.before.iter().chain(&args.after).flatten().next() {
        Some(ty) => Err(syn::Error::new_spanned(
            ty,
            "only tables take `before` and `after`",
        )),
        None => Ok(()),
    }
}

pub(crate) fn query(args: Args, item: ItemStruct) -> syn::Result<TokenStream> {
    let krate = krate();
    if let Some(param) = item
        .generics
        .params
        .iter()
        .find(|param| !matches!(param, GenericParam::Type(_)))
    {
        return Err(syn::Error::new_spanned(
            param,
            "only type parameters are supported on queries",
        ));
    }
    let sql = args
        .sql
        .as_ref()
        .ok_or_else(|| syn::Error::new(Span::call_site(), "queries need `sql = \"...\"`"))?;
    no_hooks(&args)?;
    let template = template::parse(sql)?;
    let engine = Engine::of(&args.dialect, args.grammar.as_ref(), &item.generics)?;
    let analysis = analyze::analyze(&template, engine, sql)?;
    if let Some(row) = &args.row {
        if !item.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                row,
                "generic queries can't take `row`; they aren't statements on their own",
            ));
        }
        if !analysis.returns_rows {
            return Err(syn::Error::new_spanned(
                row,
                "this statement returns no rows; only queries and statements with `RETURNING` take `row`",
            ));
        }
    }
    let row = row(&args, &item, analysis.returns_rows, Some(&analysis.columns))?;
    let kind = match analysis.kind {
        Kind::Query => quote!(Query),
        Kind::Dml => quote!(Dml),
        Kind::Ddl => quote!(Ddl),
    };

    let wrappers = separate(&args, &item, &template)?;
    let separate: Vec<(Type, Type)> = wrappers
        .iter()
        .map(|(named, wrapper)| {
            let wrapper = &wrapper.ident;
            (named.clone(), parse_quote!(#wrapper))
        })
        .collect();
    let items = items(&template, &item.generics, &separate);
    let fields: Vec<Type> = items.iter().map(|(ty, _)| ty.clone()).collect();
    let embed_checks = embeds(&item, &fields, &args.dialect);
    let ident = &item.ident;
    let node_name = args
        .name
        .as_ref()
        .map_or_else(|| snake_case(&ident.to_string()), LitStr::value);
    let inject = placement(args.placement.unwrap_or(Placement::Subquery));
    let params = Params::new(&template, &analysis, &items, &item, sql)?;
    let node = node(
        &node_name,
        fingerprint(ident, &item),
        kind,
        inject,
        parts(&template, &analysis, &params),
        &fields,
        [&[], &[]],
    );
    let wrappers = wrappers
        .iter()
        .map(|(named, wrapper)| self::wrapper(named, wrapper, &args.dialect, sql))
        .collect::<syn::Result<Vec<_>>>()?;

    let dialect = &args.dialect;
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
    let params_ty = params.ty();
    let params_struct = params.definition();
    let derives = params.derives();
    let bind_params = params.bind_impl();
    let builder = params.builder(dialect, row.as_ref());
    let parse_check = args.parse_check.as_ref().is_none_or(|check| check.value);
    let statement = item.generics.params.is_empty().then(|| {
        let test = format_ident!("{}_sql_parses", snake_case(&ident.to_string()));
        let test = parse_check.then(|| {
            quote! {
                #krate::__if_parse_check! {
                    #[cfg(test)]
                    #[test]
                    fn #test() {
                        #krate::check::parse::<#ident>();
                    }
                }
            }
        });
        let rows = row.as_ref().map(|row| {
            quote! {
                impl #krate::statement::Rows for #ident {
                    type Row = #row;
                }
            }
        });
        let from_row = from_row(&item, dialect);
        quote! {
            #krate::impl_statement!(#ident);
            #rows
            #from_row

            impl #ident {
                /// The SQL, rendered at compile time.
                pub const SQL: &'static str = <Self as #krate::statement::Statement>::SQL;
            }

            #test
        }
    });
    let fmt = fmt(&args, &item, true)?;
    let mut documented = item.clone();
    documented
        .attrs
        .extend(params.item_docs(Source::Template(sql)));
    documented.attrs.extend(row_docs(&item));

    Ok(quote! {
        #documented

        #params_struct
        #derives

        impl #impl_generics #krate::sql::Sql for #ident #ty_generics #where_clause {
            type Dialect = #dialect;
            type Params = #params_ty;
            const NODE: &'static #krate::node::Node = #node;
        }

        #embed_checks
        #bind_params
        #builder
        #statement
        #fmt
        #(#wrappers)*
    })
}

fn items(template: &Template, generics: &Generics, separate: &[(Type, Type)]) -> Vec<(Type, Type)> {
    let mut items = Vec::new();
    for ty in &template.children {
        add_item(ty, generics, separate, &mut items);
    }
    items
}

fn add_item(
    ty: &Type,
    generics: &Generics,
    separate: &[(Type, Type)],
    items: &mut Vec<(Type, Type)>,
) {
    let open = generics.type_params().any(
        |param| matches!(ty, Type::Path(path) if path.qself.is_none() && path.path.is_ident(&param.ident)),
    );
    if open
        || items
            .iter()
            .any(|(_, named)| type_key(named) == type_key(ty))
    {
        return;
    }
    if let Some((_, wrapper)) = separate
        .iter()
        .find(|(named, _)| type_key(named) == type_key(ty))
    {
        items.push((wrapper.clone(), ty.clone()));
        return;
    }
    items.push((ty.clone(), ty.clone()));
    for arg in type_args(ty) {
        add_item(arg, generics, separate, items);
    }
}

fn type_args(ty: &Type) -> Vec<&Type> {
    let Type::Path(path) = ty else {
        return Vec::new();
    };
    let Some(PathArguments::AngleBracketed(args)) =
        path.path.segments.last().map(|last| &last.arguments)
    else {
        return Vec::new();
    };
    args.args
        .iter()
        .filter_map(|arg| match arg {
            GenericArgument::Type(arg) => Some(arg),
            _ => None,
        })
        .collect()
}

fn separate(
    args: &Args,
    item: &ItemStruct,
    template: &Template,
) -> syn::Result<Vec<(Type, ItemStruct)>> {
    let Some(types) = &args.separate else {
        return Ok(Vec::new());
    };
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "`separate` isn't supported on generic queries",
        ));
    }
    let mut wrappers: Vec<(Type, ItemStruct)> = Vec::new();
    for ty in types {
        let key = type_key(ty);
        if !template.children.iter().any(|child| type_key(child) == key) {
            return Err(syn::Error::new_spanned(
                ty,
                format!(
                    "`{}` isn't referenced in the template",
                    docs::type_string(ty)
                ),
            ));
        }
        if type_args(ty).is_empty() {
            return Err(syn::Error::new_spanned(
                ty,
                format!(
                    "`{}` has no type arguments, so there's nothing to separate",
                    docs::type_string(ty)
                ),
            ));
        }
        if wrappers.iter().any(|(other, _)| type_key(other) == key) {
            return Err(syn::Error::new_spanned(
                ty,
                format!("`{}` is listed twice", docs::type_string(ty)),
            ));
        }
        let ident = format_ident!(
            "__{}{}",
            item.ident,
            camel(&format_ident!("{}", field_name(ty)))
        );
        let vis = &item.vis;
        wrappers.push((ty.clone(), parse_quote!(#[doc(hidden)] #vis struct #ident;)));
    }
    Ok(wrappers)
}

fn wrapper(
    named: &Type,
    item: &ItemStruct,
    dialect: &Type,
    sql: &LitStr,
) -> syn::Result<TokenStream> {
    let krate = krate();
    let mut items = Vec::new();
    add_item(named, &item.generics, &[], &mut items);
    let template = Template {
        segments: Vec::new(),
        params: Vec::new(),
        children: Vec::new(),
    };
    let analysis = Analysis {
        kind: Kind::Query,
        returns_rows: false,
        columns: Columns::default(),
        refs: Vec::new(),
        names: Vec::new(),
    };
    let mut params = Params::new(&template, &analysis, &items, item, sql)?;
    params.synthetic = true;
    params.runs = false;
    let ident = &item.ident;
    let fields: Vec<Type> = items.iter().map(|(ty, _)| ty.clone()).collect();
    let node = node(
        &snake_case(&ident.to_string()),
        fingerprint(ident, item),
        quote!(Scope),
        quote!(#krate::node::inject::Inject::Subquery),
        Vec::new(),
        &fields,
        [&[], &[]],
    );
    let params_ty = params.ty();
    let params_struct = params.definition();
    let derives = params.derives();
    let bind_params = params.bind_impl();
    let builder = params.builder(dialect, None);
    let embed_checks = embeds(item, &fields, dialect);
    Ok(quote! {
        #item

        #params_struct
        #derives

        impl #krate::sql::Sql for #ident {
            type Dialect = #dialect;
            type Params = #params_ty;
            const NODE: &'static #krate::node::Node = #node;
        }

        #embed_checks

        #bind_params
        #builder
    })
}

pub(crate) fn statement(args: Args, item: ItemStruct) -> syn::Result<TokenStream> {
    let krate = krate();
    let target = &args.dialect;
    for (present, name) in [
        (args.sql.is_some(), "sql"),
        (args.name.is_some(), "name"),
        (args.placement.is_some(), "cte` or `subquery"),
        (args.separate.is_some(), "separate"),
        (args.grammar.is_some(), "grammar"),
    ] {
        if present {
            return Err(syn::Error::new(
                Span::call_site(),
                format!(
                    "statements take their SQL from `{}`, so they don't take `{name}`",
                    docs::type_string(target)
                ),
            ));
        }
    }
    no_hooks(&args)?;
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "statements can't be generic; name the instantiation instead",
        ));
    }
    let ident = &item.ident;
    let dialect: Type = parse_quote!(<#target as #krate::sql::Sql>::Dialect);
    let mut items = Vec::new();
    add_item(target, &item.generics, &[], &mut items);
    let source = LitStr::new(&docs::type_string(target), Span::call_site());
    let template = Template {
        segments: Vec::new(),
        params: Vec::new(),
        children: Vec::new(),
    };
    let analysis = Analysis {
        kind: Kind::Query,
        returns_rows: true,
        columns: Columns::default(),
        refs: Vec::new(),
        names: Vec::new(),
    };
    let params = Params::new(&template, &analysis, &items, &item, &source)?;
    let fields: Vec<Type> = items.iter().map(|(ty, _)| ty.clone()).collect();
    let node = node(
        &snake_case(&ident.to_string()),
        fingerprint(ident, &item),
        quote!(Scope),
        quote!(#krate::node::inject::Inject::Subquery),
        Vec::new(),
        &fields,
        [&[], &[]],
    );
    let params_ty = params.ty();
    let params_struct = params.definition();
    let derives = params.derives();
    let bind_params = params.bind_impl();
    let row = row(&args, &item, true, None)?;
    let builder = params.builder(&dialect, row.as_ref());
    let embed_checks = embeds(&item, &fields, &dialect);
    let fmt = fmt(&args, &item, true)?;
    let rows = row.as_ref().map(|row| {
        quote! {
            impl #krate::statement::Rows for #ident {
                type Row = #row;
            }
        }
    });
    let from_row = from_row(&item, &dialect);
    let parse_check = args.parse_check.as_ref().is_none_or(|check| check.value);
    let test = parse_check.then(|| {
        let test = format_ident!("{}_sql_parses", snake_case(&ident.to_string()));
        quote! {
            #krate::__if_parse_check! {
                #[cfg(test)]
                #[test]
                fn #test() {
                    #krate::check::parse::<#ident>();
                }
            }
        }
    });
    let mut documented = item.clone();
    documented
        .attrs
        .extend(params.item_docs(Source::Statement(target)));
    documented.attrs.extend(row_docs(&item));
    Ok(quote! {
        #documented

        #params_struct
        #derives

        impl #krate::sql::Sql for #ident {
            type Dialect = #dialect;
            type Params = #params_ty;
            const NODE: &'static #krate::node::Node = #node;
        }

        #embed_checks
        #bind_params
        #builder

        #krate::impl_statement!(#ident);
        #rows
        #from_row

        impl #ident {
            /// The SQL, rendered at compile time.
            pub const SQL: &'static str = <Self as #krate::statement::Statement>::SQL;
        }

        #test
        #fmt
    })
}

fn named_fields(item: &ItemStruct) -> Vec<&Field> {
    match &item.fields {
        Fields::Named(fields) => fields.named.iter().collect(),
        Fields::Unnamed(_) | Fields::Unit => Vec::new(),
    }
}

fn column_name(field: &Field) -> String {
    let name = field.ident.as_ref().expect("a named field");
    name.to_string().trim_start_matches("r#").to_owned()
}

fn row(
    args: &Args,
    item: &ItemStruct,
    returns_rows: bool,
    columns: Option<&Columns>,
) -> syn::Result<Option<Type>> {
    let fields = named_fields(item);
    if fields.is_empty() {
        return Ok(args.row.clone());
    }
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.fields,
            "generic queries aren't statements, so their fields can't be a row type; declare them as `struct Name<T>(PhantomData<T>)`",
        ));
    }
    if let Some(row) = &args.row {
        return Err(syn::Error::new_spanned(
            row,
            "this struct's fields are its row type, so it doesn't take `row`",
        ));
    }
    if !returns_rows {
        return Err(syn::Error::new_spanned(
            &item.fields,
            "this statement returns no rows, so its struct can't have fields; only queries and statements with `RETURNING` have a row type",
        ));
    }
    if let Some(columns) = columns.filter(|columns| columns.complete) {
        for field in &fields {
            let name = column_name(field);
            if !columns.names.contains(&name) {
                let names: Vec<String> = columns
                    .names
                    .iter()
                    .map(|name| format!("`{name}`"))
                    .collect();
                return Err(syn::Error::new_spanned(
                    &field.ident,
                    format!(
                        "the statement returns no column named `{name}`; its columns are {}",
                        names.join(", ")
                    ),
                ));
            }
        }
    }
    let ident = &item.ident;
    Ok(Some(parse_quote!(#ident)))
}

fn from_row(item: &ItemStruct, dialect: &Type) -> Option<TokenStream> {
    let fields = named_fields(item);
    if fields.is_empty() {
        return None;
    }
    let krate = krate();
    let sqlx = quote!(#krate::__private::sqlx);
    let driver = quote!(#krate::dialect::driver);
    let ident = &item.ident;
    let reads = fields.iter().map(|field| {
        let (name, ty) = (&field.ident, &field.ty);
        let column = column_name(field);
        quote_spanned!(ty.span()=> #name: #sqlx::Row::try_get::<#ty, _>(row, #column)?)
    });
    Some(quote! {
        #krate::__if_sqlx! {
            impl<'__r> #sqlx::FromRow<'__r, #driver::Row<#dialect>> for #ident {
                fn from_row(
                    row: &'__r #driver::Row<#dialect>,
                ) -> ::core::result::Result<Self, #sqlx::Error> {
                    ::core::result::Result::Ok(Self {
                        #(#reads,)*
                    })
                }
            }
        }
    })
}

fn row_docs(item: &ItemStruct) -> Vec<syn::Attribute> {
    if named_fields(item).is_empty() {
        return Vec::new();
    }
    docs::attrs(&[
        String::new(),
        "# Row".to_owned(),
        String::new(),
        "Each row the statement returns is read into this struct, matching fields to columns by name."
            .to_owned(),
    ])
}

fn embeds(item: &ItemStruct, items: &[Type], dialect: &Type) -> TokenStream {
    let krate = krate();
    let ident = &item.ident;
    if item.generics.params.is_empty() {
        let checks = items.iter().map(|ty| {
            quote! {
                const _: () = {
                    #krate::embed::embeds::<#dialect, <#ty as #krate::sql::Sql>::Dialect>();
                    #krate::embed::checked::<#ty>();
                };
            }
        });
        return quote! {
            impl #krate::embed::Checked for #ident {}

            #(#checks)*
        };
    }
    let mut generics = item.generics.clone();
    let clause = generics.make_where_clause();
    for ty in items {
        clause.predicates.push(parse_quote!(
            <#ty as #krate::sql::Sql>::Dialect: #krate::embed::EmbedsIn<#dialect>
        ));
        clause
            .predicates
            .push(parse_quote!(#ty: #krate::embed::Checked));
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote! {
        impl #impl_generics #krate::embed::Checked for #ident #ty_generics #where_clause {}
    }
}

fn node(
    name: &str,
    fingerprint: TokenStream,
    kind: TokenStream,
    inject: TokenStream,
    parts: Vec<TokenStream>,
    items: &[Type],
    [before, after]: [&[Type]; 2],
) -> TokenStream {
    let krate = krate();
    quote! {
        &#krate::node::Node {
            name: #krate::node::name::Name::new(#name),
            fingerprint: #fingerprint,
            kind: #krate::node::kind::Kind::#kind,
            inject: #inject,
            parts: #krate::part::Parts(&[#(#parts),*]),
            before: #krate::node::hooks::Hooks(&[#(<#before as #krate::sql::Sql>::NODE),*]),
            after: #krate::node::hooks::Hooks(&[#(<#after as #krate::sql::Sql>::NODE),*]),
            items: #krate::node::items::Items(&[#(<#items as #krate::sql::Sql>::NODE),*]),
        }
    }
}

fn fingerprint(ident: &Ident, item: &ItemStruct) -> TokenStream {
    let krate = krate();
    let path = format!("::{ident}");
    let mut fingerprint = quote! {
        #krate::node::fingerprint::Fingerprint::of(
            ::core::concat!(::core::module_path!(), #path)
        )
    };
    for param in item.generics.type_params() {
        let param = &param.ident;
        fingerprint = quote!(#fingerprint.combine(<#param as #krate::sql::Sql>::NODE.fingerprint));
    }
    fingerprint
}

fn placement(placement: Placement) -> TokenStream {
    let krate = krate();
    match placement {
        Placement::Cte { recursive } => quote!(#krate::node::inject::Inject::cte(#recursive)),
        Placement::Subquery => quote!(#krate::node::inject::Inject::Subquery),
    }
}

fn parts(template: &Template, analysis: &Analysis, params: &Params) -> Vec<TokenStream> {
    let krate = krate();
    let mut positions = analysis.refs.iter();
    template
        .segments
        .iter()
        .map(|segment| match segment {
            Segment::Lit(text) => quote!(#krate::part::lit::Lit::part(#text)),
            Segment::Param(slot) => {
                let (field, ty) = params.describe(*slot);
                let slot = Literal::u16_unsuffixed(*slot);
                quote!(#krate::part::param::Param::part(#slot, #field, #ty))
            }
            Segment::Ref(item) => {
                let position = positions.next().expect("a position for every reference");
                let ty = &item.ty;
                let node = quote!(<#ty as #krate::sql::Sql>::NODE);
                if !position.from {
                    return quote!(#krate::part::expr::Expr::part(#node));
                }
                let rule = if position.alias.is_some() {
                    quote!(Given)
                } else {
                    quote!(NodeName)
                };
                let inject = match item.placement {
                    Some(placement) => {
                        let placement = self::placement(placement);
                        quote!(::core::option::Option::Some(#placement))
                    }
                    None => quote!(::core::option::Option::None),
                };
                quote! {
                    #krate::part::from::From::part(
                        #node,
                        #krate::part::from::rule::AliasRule::#rule,
                        #inject,
                    )
                }
            }
        })
        .collect()
}

struct Group {
    name: Ident,
    field: Ident,
    ty: Type,
    slots: Vec<u16>,
    origins: Vec<String>,
    state: Ident,
    marker: Ident,
}

struct Child {
    name: Ident,
    field: Ident,
    ty: Type,
    named: Type,
    state: Ident,
}

enum Source<'a> {
    Template(&'a LitStr),
    Statement(&'a Type),
    Table(&'a [Type], &'a [Type]),
}

struct Params<'a> {
    item: &'a ItemStruct,
    ident: Ident,
    builder: Ident,
    groups: Vec<Group>,
    children: Vec<Child>,
    synthetic: bool,
    runs: bool,
}

impl<'a> Params<'a> {
    fn new(
        template: &Template,
        analysis: &Analysis,
        items: &[(Type, Type)],
        item: &'a ItemStruct,
        sql: &LitStr,
    ) -> syn::Result<Params<'a>> {
        let mut groups: Vec<Group> = Vec::new();
        for (slot, param) in template.params.iter().enumerate() {
            let (name, origin) = match (&param.name, &analysis.names[slot]) {
                (Some(name), _) => (
                    name.clone(),
                    format!("declared as `{{{name}: {}}}`", docs::type_string(&param.ty)),
                ),
                (None, Some((inferred, origin))) => {
                    (to_ident(inferred), docs::origin(*origin, inferred))
                }
                (None, None) => (
                    format_ident!("bind{}", slot + 1),
                    "not named by the SQL around it".to_owned(),
                ),
            };
            let name = if RESERVED.contains(&name.to_string().as_str()) {
                format_ident!("{name}_")
            } else {
                name
            };
            match groups.iter_mut().find(|group| group.name == name) {
                Some(group) => {
                    if type_key(&group.ty) != type_key(&param.ty) {
                        return Err(syn::Error::new(
                            sql.span(),
                            format!(
                                "parameters named `{name}` have different types (`{}` and `{}`); name them with `{{name: Type}}`",
                                type_key(&group.ty),
                                type_key(&param.ty)
                            ),
                        ));
                    }
                    group.slots.push(slot as u16);
                    group.origins.push(origin);
                }
                None => groups.push(Group {
                    state: format_ident!("Param{}", camel(&name)),
                    marker: format_ident!("{}", camel(&name)),
                    field: builder_field(&name),
                    name,
                    ty: param.ty.clone(),
                    slots: vec![slot as u16],
                    origins: vec![origin],
                }),
            }
        }

        let mut markers: Vec<String> = Vec::new();
        for group in &mut groups {
            let marker = unique(group.marker.to_string(), &markers);
            markers.push(marker.clone());
            group.marker = format_ident!("{marker}");
        }

        let mut taken: Vec<String> = groups
            .iter()
            .map(|group| group.name.to_string())
            .chain(RESERVED.iter().map(|name| (*name).to_owned()))
            .collect();
        let refs = template
            .segments
            .iter()
            .filter_map(|segment| match segment {
                Segment::Ref(item) => Some(&item.ty),
                _ => None,
            });
        let mut aliases: Vec<(String, &str)> = Vec::new();
        for (ty, position) in refs.zip(&analysis.refs) {
            if let Some(alias) = &position.alias
                && alias.chars().count() >= 2
                && !aliases.iter().any(|(key, _)| *key == type_key(ty))
            {
                aliases.push((type_key(ty), alias));
            }
        }
        let preferred: Vec<String> = items
            .iter()
            .map(
                |(_, named)| match aliases.iter().find(|(key, _)| *key == type_key(named)) {
                    Some((_, alias)) => to_ident(alias).to_string(),
                    None => short_name(named),
                },
            )
            .collect();
        let children = items
            .iter()
            .zip(&preferred)
            .map(|((ty, named), name)| {
                let shared = preferred.iter().filter(|other| *other == name).count() > 1;
                let name = if shared || taken.contains(name) {
                    field_name(named)
                } else {
                    name.clone()
                };
                let name = unique(name, &taken);
                taken.push(name.clone());
                let name = format_ident!("{name}");
                Child {
                    state: format_ident!("Item{}", camel(&name)),
                    field: builder_field(&name),
                    name,
                    ty: ty.clone(),
                    named: named.clone(),
                }
            })
            .collect();
        Ok(Params {
            item,
            ident: format_ident!("{}Params", item.ident),
            builder: format_ident!("{}Builder", item.ident),
            groups,
            children,
            synthetic: false,
            runs: true,
        })
    }

    fn describe(&self, slot: u16) -> (String, String) {
        self.groups
            .iter()
            .find_map(|group| {
                let index = group.slots.iter().position(|other| *other == slot)?;
                let field = match group.slots.len() {
                    1 => group.name.to_string(),
                    _ => format!("{}[{index}]", group.name),
                };
                Some((field, docs::type_string(&group.ty)))
            })
            .expect("every slot belongs to a group")
    }

    fn marker(&self) -> Option<TokenStream> {
        let args = self.item_args();
        (!args.is_empty()).then(|| quote!(::core::marker::PhantomData<fn() -> (#(#args,)*)>))
    }

    fn marker_value(&self) -> Option<TokenStream> {
        self.marker()
            .map(|_| quote!(__marker: ::core::marker::PhantomData,))
    }

    fn is_empty(&self) -> bool {
        self.groups.is_empty() && self.children.is_empty()
    }

    fn item_args(&self) -> Vec<&Ident> {
        self.item
            .generics
            .type_params()
            .map(|param| &param.ident)
            .collect()
    }

    fn constructor(&self) -> String {
        let args: Vec<String> = self.item_args().iter().map(ToString::to_string).collect();
        let ident = &self.item.ident;
        if args.is_empty() {
            format!("{ident}::builder()")
        } else {
            format!("{ident}::<{}>::builder()", args.join(", "))
        }
    }

    fn module(&self) -> Ident {
        format_ident!("__{}_builder", snake_case(&self.item.ident.to_string()))
    }

    fn link(&self, ty: &Type) -> String {
        docs::link(ty, &self.item_args())
    }

    fn item_docs(&self, source: Source) -> Vec<syn::Attribute> {
        let mut lines = Vec::new();
        if self
            .item
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("doc"))
        {
            lines.push(String::new());
        }
        match source {
            Source::Template(sql) => {
                lines.push("# Template".to_owned());
                lines.push(String::new());
                lines.push("```sql".to_owned());
                lines.extend(docs::template(&sql.value()));
                lines.push("```".to_owned());
            }
            Source::Statement(target) => {
                lines.push(format!("Runs {} as a statement.", self.link(target)))
            }
            Source::Table(before, after) => {
                let list = |hooks: &[Type]| {
                    let links: Vec<String> = hooks.iter().map(|ty| self.link(ty)).collect();
                    links.join(", ")
                };
                let runs = match (before.is_empty(), after.is_empty()) {
                    (true, true) => return Vec::new(),
                    (false, true) => format!("{} before it", list(before)),
                    (true, false) => format!("{} after it", list(after)),
                    (false, false) => {
                        format!("{} before it and {} after it", list(before), list(after))
                    }
                };
                lines.push("# Hooks".to_owned());
                lines.push(String::new());
                lines.push(format!("Every statement that uses this table runs {runs}."));
            }
        }
        if !self.is_empty() {
            lines.push(String::new());
            lines.push("# Parameters".to_owned());
            lines.push(String::new());
            lines.push(format!(
                "Set them through [`{}`], from `{}`:",
                self.builder,
                self.constructor()
            ));
            lines.push(String::new());
            for group in &self.groups {
                let times = match group.slots.len() {
                    1 => String::new(),
                    len => format!(" ×{len}"),
                };
                lines.push(format!(
                    "- `.{}({})`{times}: {}",
                    group.name,
                    docs::type_string(&group.ty),
                    docs::origins(&group.origins)
                ));
            }
            for child in &self.children {
                let separate = if type_key(&child.ty) == type_key(&child.named) {
                    ""
                } else {
                    ", with its own values for its type arguments"
                };
                lines.push(format!(
                    "- `.{}()`, then a setter: the parameters of {}{separate}",
                    child.name,
                    self.link(&child.named)
                ));
            }
            if !self.children.is_empty() {
                lines.push(String::new());
                lines.push("Items without parameters, such as tables, need no call.".to_owned());
            }
        }
        docs::attrs(&lines)
    }

    fn ty(&self) -> TokenStream {
        if self.is_empty() {
            return quote!(());
        }
        let ident = &self.ident;
        let args = self.item_args();
        if args.is_empty() {
            quote!(#ident)
        } else {
            quote!(#ident<#(#args),*>)
        }
    }

    fn definition(&self) -> Option<TokenStream> {
        if self.is_empty() {
            return None;
        }
        let krate = krate();
        let vis = &self.item.vis;
        let ident = &self.ident;
        let generics = &self.item.generics;
        let where_clause = &generics.where_clause;
        let groups = self.groups.iter().map(|group| {
            let (name, ty) = (&group.name, &group.ty);
            let origins = docs::origins(&group.origins);
            match group.slots.len() {
                1 => {
                    let doc = doc(&format!("{}.", docs::capitalize(&origins)));
                    quote!(#doc pub #name: #ty)
                }
                len => {
                    let doc = doc(&format!("{len} values in template order, {origins}."));
                    let len = Literal::usize_unsuffixed(len);
                    quote!(#doc pub #name: [#ty; #len])
                }
            }
        });
        let children = self.children.iter().map(|child| {
            let (name, ty) = (&child.name, &child.ty);
            let doc = doc(&format!("The parameters of {}.", self.link(&child.named)));
            quote!(#doc pub #name: <#ty as #krate::sql::Sql>::Params)
        });
        let doc = doc(&format!(
            "The parameters of [`{}`], built by [`{}`].",
            self.item.ident, self.builder
        ));
        let hidden = self.synthetic.then(|| quote!(#[doc(hidden)]));
        let marker = self.marker().map(|marker| {
            quote! {
                #[doc(hidden)]
                pub __marker: #marker,
            }
        });
        Some(quote! {
            #doc
            #hidden
            #vis struct #ident #generics #where_clause {
                #(#groups,)*
                #(#children,)*
                #marker
            }
        })
    }

    fn derives(&self) -> Option<TokenStream> {
        if self.is_empty() {
            return None;
        }
        let ident = &self.ident;
        let name = ident.to_string();
        let krate = krate();
        let fields: Vec<(&Ident, TokenStream)> = self
            .groups
            .iter()
            .map(|group| {
                let ty = &group.ty;
                let ty = match group.slots.len() {
                    1 => quote!(#ty),
                    len => {
                        let len = Literal::usize_unsuffixed(len);
                        quote!([#ty; #len])
                    }
                };
                (&group.name, ty)
            })
            .chain(self.children.iter().map(|child| {
                let ty = &child.ty;
                (&child.name, quote!(<#ty as #krate::sql::Sql>::Params))
            }))
            .collect();
        let names: Vec<&Ident> = fields.iter().map(|(name, _)| *name).collect();
        let marker_value = self.marker_value();
        let labels: Vec<String> = names
            .iter()
            .map(|name| name.to_string().trim_start_matches("r#").to_owned())
            .collect();
        let bounded = |bound: TokenStream| {
            let mut generics = self.item.generics.clone();
            let where_clause = generics.make_where_clause();
            for (_, ty) in &fields {
                where_clause
                    .predicates
                    .push(parse_quote!(for<'__a> #ty: #bound));
            }
            generics
        };
        let (_, ty_generics, _) = self.item.generics.split_for_impl();
        let generics = bounded(quote!(::core::clone::Clone));
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let mut out = quote! {
            impl #impl_generics ::core::clone::Clone for #ident #ty_generics #where_clause {
                fn clone(&self) -> Self {
                    Self {
                        #(#names: ::core::clone::Clone::clone(&self.#names),)*
                        #marker_value
                    }
                }
            }
        };
        let generics = bounded(quote!(::core::fmt::Debug));
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        out.extend(quote! {
            impl #impl_generics ::core::fmt::Debug for #ident #ty_generics #where_clause {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.debug_struct(#name)
                        #(.field(#labels, &self.#names))*
                        .finish()
                }
            }
        });
        let generics = bounded(quote!(::core::cmp::PartialEq));
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        out.extend(quote! {
            impl #impl_generics ::core::cmp::PartialEq for #ident #ty_generics #where_clause {
                fn eq(&self, other: &Self) -> bool {
                    true #(&& self.#names == other.#names)*
                }
            }
        });
        let generics = bounded(quote!(::core::cmp::Eq));
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        out.extend(quote! {
            impl #impl_generics ::core::cmp::Eq for #ident #ty_generics #where_clause {}
        });
        Some(out)
    }

    fn bind_impl(&self) -> Option<TokenStream> {
        if self.is_empty() {
            return None;
        }
        let krate = krate();
        let sqlx = quote!(#krate::__private::sqlx);
        let bind = quote!(#krate::statement::params::BindParams);

        let mut generics = self.item.generics.clone();
        generics
            .params
            .insert(0, GenericParam::Type(parse_quote!(__DB: #sqlx::Database)));
        let where_clause = generics.make_where_clause();
        for group in &self.groups {
            let ty = &group.ty;
            where_clause
                .predicates
                .push(parse_quote!(for<'__t> #ty: #sqlx::Encode<'__t, __DB> + #sqlx::Type<__DB>));
        }
        for child in &self.children {
            let ty = &child.ty;
            where_clause
                .predicates
                .push(parse_quote!(<#ty as #krate::sql::Sql>::Params: #bind<__DB>));
        }
        let (impl_generics, _, where_clause) = generics.split_for_impl();

        let own = self.groups.iter().flat_map(|group| {
            let name = &group.name;
            let single = group.slots.len() == 1;
            group.slots.iter().enumerate().map(move |(index, slot)| {
                let slot = Literal::u16_unsuffixed(*slot);
                let index = Literal::usize_unsuffixed(index);
                if single {
                    quote!(([], #slot) => args.add(&self.#name),)
                } else {
                    quote!(([], #slot) => args.add(&self.#name[#index]),)
                }
            })
        });
        let children = self.children.iter().enumerate().map(|(step, child)| {
            let name = &child.name;
            let step = Literal::u16_unsuffixed(step as u16);
            quote!(([#step, rest @ ..], _) => #bind::<__DB>::bind(&self.#name, rest, slot, args),)
        });
        let ty = self.ty();

        Some(quote! {
            #krate::__if_sqlx! {
                impl #impl_generics #bind<__DB> for #ty #where_clause {
                    fn bind(
                        &self,
                        path: &[u16],
                        slot: u16,
                        args: &mut <__DB as #sqlx::Database>::Arguments,
                    ) -> ::core::result::Result<(), #sqlx::error::BoxDynError> {
                        use #sqlx::Arguments as _;
                        match (path, slot) {
                            #(#own)*
                            #(#children)*
                            _ => ::core::result::Result::Err(
                                #krate::statement::params::unknown(path, slot),
                            ),
                        }
                    }
                }
            }
        })
    }

    fn states(&self) -> Vec<TokenStream> {
        self.groups
            .iter()
            .map(|group| group.state.to_token_stream())
            .chain(
                self.children
                    .iter()
                    .map(|child| child.state.to_token_stream()),
            )
            .collect()
    }

    fn builder_in(
        &self,
        states: &[TokenStream],
        hooked: &TokenStream,
        parent: &TokenStream,
    ) -> TokenStream {
        let builder = &self.builder;
        let args = self.item_args();
        quote!(#builder<#(#args,)* #(#states,)* #hooked, #parent>)
    }

    fn generics(&self, extra: &[TokenStream]) -> Generics {
        let mut generics = self.item.generics.clone();
        for param in extra {
            generics
                .params
                .push(syn::parse2(param.clone()).expect("a generic parameter"));
        }
        generics
    }

    fn statement(&self) -> bool {
        self.item.generics.params.is_empty() && self.runs
    }

    fn fields(&self) -> Vec<&Ident> {
        self.groups
            .iter()
            .map(|group| &group.field)
            .chain(self.children.iter().map(|child| &child.field))
            .collect()
    }

    fn builder(&self, dialect: &Type, row: Option<&Type>) -> TokenStream {
        let krate = krate();
        let ident = &self.item.ident;
        let (impl_generics, ty_generics, where_clause) = self.item.generics.split_for_impl();
        if self.is_empty() {
            return quote! {
                impl #impl_generics #krate::builder::Build for #ident #ty_generics #where_clause {
                    type Builder = #krate::builder::NoParams;

                    fn builder() -> Self::Builder {
                        #krate::builder::NoParams
                    }
                }
            };
        }

        let b = quote!(#krate::builder);
        let builder = &self.builder;
        let vis = &self.item.vis;
        let module = self.module();
        let states = self.states();
        let params_ty = self.ty();
        let args = self.item_args();
        let fields = self.fields();
        let root = quote!(#b::Root);
        let parent = quote!(__K);
        let hooked = quote!(__H);
        let except = |slot: usize, extra: &[TokenStream]| {
            let mut params: Vec<TokenStream> = states
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != slot)
                .map(|(_, state)| state.clone())
                .collect();
            params.extend(extra.iter().cloned());
            params.push(hooked.clone());
            self.generics(&params)
        };
        let mut out = TokenStream::new();

        let markers = self.groups.iter().map(|group| {
            let marker = &group.marker;
            quote! {
                pub struct #marker<__S>(::core::marker::PhantomData<__S>);

                impl<__S: #b::Ready> #b::Ready for #marker<__S> {
                    type Out = __S::Out;
                }
            }
        });
        out.extend(quote! {
            #[doc(hidden)]
            #vis mod #module {
                #(#markers)*
            }
        });

        let mut struct_params = states.clone();
        struct_params.push(quote!(__H = #b::NoHooks));
        struct_params.push(quote!(__K = #root));
        let generics = self.generics(&struct_params);
        let where_clause = &generics.where_clause;
        let group_fields = self.groups.iter().map(|group| {
            let (field, ty) = (&group.field, &group.ty);
            let len = Literal::usize_unsuffixed(group.slots.len());
            quote!(#field: [::core::option::Option<#ty>; #len])
        });
        let child_fields = self.children.iter().map(|child| {
            let (field, state) = (&child.field, &child.state);
            quote!(#field: #state)
        });
        let group_states = self.groups.iter().map(|group| &group.state);
        let hidden = self.synthetic.then(|| quote!(#[doc(hidden)]));
        let builder_doc = docs::attrs(&[
            format!(
                "Builds [`{}`]; start with `{}`.",
                self.ident,
                self.constructor()
            ),
            String::new(),
            format!(
                "Each parameter of [`{ident}`] has a setter while it still needs a value, and each item with parameters left has a method that moves to its builder for one setter call. `build` appears once everything is set."
            ),
        ]);
        out.extend(quote! {
            #(#builder_doc)*
            #hidden
            #vis struct #builder #generics #where_clause {
                #(#group_fields,)*
                #(#child_fields,)*
                __parent: __K,
                __state: ::core::marker::PhantomData<fn() -> (#(#group_states,)* #(#args,)* __H)>,
            }
        });

        let generics = self.generics(&{
            let mut params = states.clone();
            params.push(hooked.clone());
            params
        });
        let mut ready_generics = generics.clone();
        let ready_clause = ready_generics.make_where_clause();
        let outs: Vec<TokenStream> = states
            .iter()
            .map(|state| quote!(<#state as #b::Ready>::Out))
            .collect();
        for state in &states {
            ready_clause
                .predicates
                .push(parse_quote!(#state: #b::Ready));
        }
        let mut all = outs.last().cloned().expect("a builder has a state");
        for out in outs.iter().rev().skip(1) {
            ready_clause
                .predicates
                .push(parse_quote!(#out: #b::And<#all>));
            all = quote!(<#out as #b::And<#all>>::Out);
        }
        let (impl_generics, _, where_clause) = ready_generics.split_for_impl();
        let rooted = self.builder_in(&states, &hooked, &root);
        out.extend(quote! {
            impl #impl_generics #b::Ready for #rooted #where_clause {
                type Out = #all;
            }
        });
        let generics = self.generics(&{
            let mut params = states.clone();
            params.push(parent.clone());
            params.push(hooked.clone());
            params
        });
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let scoped = self.builder_in(&states, &hooked, &parent);
        out.extend(quote! {
            impl #impl_generics #b::Scope<__K> for #rooted #where_clause {
                type Scoped = #scoped;

                fn scope(self, parent: __K) -> #scoped {
                    #builder {
                        #(#fields: self.#fields,)*
                        __parent: parent,
                        __state: ::core::marker::PhantomData,
                    }
                }
            }
        });

        for (index, group) in self.groups.iter().enumerate() {
            let (name, field, ty, marker) = (&group.name, &group.field, &group.ty, &group.marker);
            let len = Literal::usize_unsuffixed(group.slots.len());
            let mut current = states.clone();
            current[index] = quote!(#module::#marker<#b::Missing<__Rest>>);
            let mut next = states.clone();
            next[index] = quote!(#module::#marker<__Rest>);
            let next = self.builder_in(&next, &hooked, &root);
            let mut generics = except(index, &[quote!(__Rest: #b::Remaining), parent.clone()]);
            generics
                .make_where_clause()
                .predicates
                .push(parse_quote!(__K: #b::Fill<#next>));
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let current = self.builder_in(&current, &hooked, &parent);
            let others = fields.iter().filter(|other| **other != field);
            let origins = docs::origins(&group.origins);
            let doc = doc(&match group.slots.len() {
                1 => format!("Sets `{name}`, {origins}."),
                len => format!(
                    "Sets the next of {len} `{name}` values, {origins}. Call it once per value, in template order."
                ),
            });
            out.extend(quote! {
                impl #impl_generics #current #where_clause {
                    #doc
                    pub fn #name(self, value: #ty) -> <__K as #b::Fill<#next>>::Output {
                        let mut #field = self.#field;
                        #field[#len - 1 - <__Rest as #b::Remaining>::N] =
                            ::core::option::Option::Some(value);
                        #b::Fill::fill(self.__parent, #builder {
                            #field,
                            #(#others: self.#others,)*
                            __parent: #root,
                            __state: ::core::marker::PhantomData,
                        })
                    }
                }
            });
        }

        for (index, child) in self.children.iter().enumerate() {
            let slot = self.groups.len() + index;
            let (name, field) = (&child.name, &child.field);
            let hole = format_ident!("__{}{}", builder, camel(name));
            let others: Vec<&&Ident> = fields.iter().filter(|other| **other != field).collect();
            let mut vacant = states.clone();
            vacant[slot] = quote!(());
            let vacant = self.builder_in(&vacant, &hooked, &parent);
            let mut current = states.clone();
            current[slot] = quote!(#b::Open<__B>);
            let current = self.builder_in(&current, &hooked, &parent);
            let generics = except(slot, &[quote!(__B), parent.clone()]);
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let doc = doc(&format!(
                "Moves to the builder of {}. Its next setter sets one of its parameters and returns to the outermost builder.",
                self.link(&child.named)
            ));
            out.extend(quote! {
                #[doc(hidden)]
                #vis struct #hole<P>(P);

                impl #impl_generics #current #where_clause {
                    #doc
                    pub fn #name<__Scoped>(self) -> __Scoped
                    where
                        __B: #b::Scope<#hole<#vacant>, Scoped = __Scoped>,
                    {
                        #b::Scope::scope(self.#field.0, #hole(#builder {
                            #field: (),
                            #(#others: self.#others,)*
                            __parent: self.__parent,
                            __state: ::core::marker::PhantomData,
                        }))
                    }
                }
            });

            let mut filled = states.clone();
            filled[slot] = quote!(<__Item as #b::Settled>::Slot);
            let filled = self.builder_in(&filled, &hooked, &root);
            let mut generics = except(slot, &[parent.clone(), quote!(__Item)]);
            let clause = generics.make_where_clause();
            clause.predicates.push(parse_quote!(__Item: #b::Settled));
            clause.predicates.push(parse_quote!(__K: #b::Fill<#filled>));
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            out.extend(quote! {
                impl #impl_generics #b::Fill<__Item> for #hole<#vacant> #where_clause {
                    type Output = <__K as #b::Fill<#filled>>::Output;

                    fn fill(self, item: __Item) -> Self::Output {
                        let parent = self.0;
                        #b::Fill::fill(parent.__parent, #builder {
                            #field: #b::Settled::settled(item),
                            #(#others: parent.#others,)*
                            __parent: #root,
                            __state: ::core::marker::PhantomData,
                        })
                    }
                }
            });
        }

        let complete: Vec<TokenStream> = self
            .groups
            .iter()
            .map(|group| {
                let marker = &group.marker;
                quote!(#module::#marker<#b::Filled>)
            })
            .chain(self.children.iter().map(|child| {
                let ty = &child.ty;
                quote!(#b::Built<<#ty as #krate::sql::Sql>::Params>)
            }))
            .collect();
        let unhooked = self.builder_in(&complete, &quote!(#b::NoHooks), &root);
        let with_hooks = self.builder_in(&complete, &quote!(#b::WithHooks), &root);
        let complete = self.builder_in(&complete, &hooked, &root);
        let generics = self.generics(&[hooked.clone()]);
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let params_ident = &self.ident;
        let extract_groups = self.groups.iter().map(|group| {
            let (name, field) = (&group.name, &group.field);
            let expect = quote!(.expect("the builder's type guarantees every parameter is set"));
            if group.slots.len() == 1 {
                quote!(#name: { let [value] = self.#field; value #expect })
            } else {
                quote!(#name: self.#field.map(|value| value #expect))
            }
        });
        let extract_children = self.children.iter().map(|child| {
            let (name, field) = (&child.name, &child.field);
            quote!(#name: self.#field.0)
        });
        let marker_value = self.marker_value();
        out.extend(quote! {
            impl #impl_generics #b::Finish for #complete #where_clause {
                type Params = #params_ty;

                fn finish(self) -> #params_ty {
                    #params_ident {
                        #(#extract_groups,)*
                        #(#extract_children,)*
                        #marker_value
                    }
                }
            }

            impl #impl_generics #complete #where_clause {
                /// Returns the parameters.
                pub fn build(self) -> #params_ty {
                    #b::Finish::finish(self)
                }
            }
        });
        if self.statement() {
            let driver = quote!(#krate::dialect::driver);
            let error = quote!(#krate::__private::sqlx::Error);
            let query = match row {
                Some(row) => {
                    let doc = doc(&format!(
                        "Binds the parameters to the SQL as a `sqlx` query that reads each row as {}.",
                        docs::link(row, &[])
                    ));
                    quote! {
                        #doc
                        pub fn query<'q>(
                            self,
                        ) -> ::core::result::Result<#driver::QueryAs<'q, #dialect, #row>, #error> {
                            #krate::statement::query_as::<#ident, #row>(&#b::Finish::finish(self))
                        }
                    }
                }
                None => quote! {
                    /// Binds the parameters to the SQL as a `sqlx` query.
                    pub fn query<'q>(
                        self,
                    ) -> ::core::result::Result<#driver::Query<'q, #dialect>, #error> {
                        #krate::statement::query::<#ident>(&#b::Finish::finish(self))
                    }
                },
            };
            out.extend(quote! {
                #krate::__if_sqlx! {
                    impl #unhooked {
                        #query

                        /// Binds the parameters to the SQL as a `sqlx` query that reads each row as `O`.
                        pub fn query_as<'q, O>(
                            self,
                        ) -> ::core::result::Result<#driver::QueryAs<'q, #dialect, O>, #error>
                        where
                            O: for<'r> #krate::__private::sqlx::FromRow<'r, #driver::Row<#dialect>>,
                        {
                            #krate::statement::query_as::<#ident, O>(&#b::Finish::finish(self))
                        }
                    }
                }
            });
            let sqlx = quote!(#krate::__private::sqlx);
            let run = quote!(#krate::statement::run);
            let acquire = quote!(#sqlx::Acquire<'c, Database = #driver::Database<#dialect>>);
            let run_main = match row {
                Some(row) => {
                    let doc = doc(&format!(
                        "Runs the statement and its hooks in one transaction, and reads each row of the statement as {}.",
                        docs::link(row, &[])
                    ));
                    quote! {
                        #doc
                        pub async fn run<'c, A: #acquire>(
                            self,
                            conn: A,
                        ) -> ::core::result::Result<::std::vec::Vec<#row>, #error> {
                            let params = #b::Finish::finish(self);
                            #run::fetch_all::<#ident, #row, A>(&params, conn).await
                        }
                    }
                }
                None => quote! {
                    /// Runs the statement and its hooks in one transaction, and returns the number of rows the statement affected.
                    pub async fn run<'c, A: #acquire>(
                        self,
                        conn: A,
                    ) -> ::core::result::Result<u64, #error> {
                        let params = #b::Finish::finish(self);
                        #run::execute::<#ident, A>(&params, conn).await
                    }
                },
            };
            out.extend(quote! {
                #krate::__if_sqlx! {
                    impl #with_hooks {
                        #run_main

                        /// Runs the statement and its hooks in one transaction, and reads each row of the statement as `O`.
                        pub async fn run_as<'c, O, A: #acquire>(
                            self,
                            conn: A,
                        ) -> ::core::result::Result<::std::vec::Vec<O>, #error>
                        where
                            O: for<'r> #sqlx::FromRow<'r, #driver::Row<#dialect>>
                                + ::core::marker::Send
                                + ::core::marker::Unpin,
                        {
                            let params = #b::Finish::finish(self);
                            #run::fetch_all::<#ident, O, A>(&params, conn).await
                        }
                    }
                }
            });
        }

        let initial: Vec<TokenStream> = self
            .groups
            .iter()
            .map(|group| {
                let marker = &group.marker;
                let count = (0..group.slots.len())
                    .fold(quote!(#b::Filled), |rest, _| quote!(#b::Missing<#rest>));
                quote!(#module::#marker<#count>)
            })
            .chain(self.children.iter().map(|child| {
                let ty = &child.ty;
                quote!(<<#ty as #b::Build>::Builder as #b::Settled>::Slot)
            }))
            .collect();
        let hooks = if self.statement() {
            quote!(<#ident as #krate::render::Render>::Hooks)
        } else {
            quote!(#b::NoHooks)
        };
        let initial = self.builder_in(&initial, &hooks, &root);
        let mut generics = self.item.generics.clone();
        for child in &self.children {
            let ty = &child.ty;
            let clause = generics.make_where_clause();
            clause.predicates.push(parse_quote!(#ty: #b::Build));
            clause
                .predicates
                .push(parse_quote!(<#ty as #b::Build>::Builder: #b::Settled));
        }
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let empty_groups = self.groups.iter().map(|group| {
            let field = &group.field;
            let len = Literal::usize_unsuffixed(group.slots.len());
            quote!(#field: [const { ::core::option::Option::None }; #len])
        });
        let start_children = self.children.iter().map(|child| {
            let (field, ty) = (&child.field, &child.ty);
            quote!(#field: #b::Settled::settled(<#ty as #b::Build>::builder()))
        });
        out.extend(quote! {
            impl #impl_generics #b::Build for #ident #ty_generics #where_clause {
                type Builder = #initial;

                fn builder() -> Self::Builder {
                    #builder {
                        #(#empty_groups,)*
                        #(#start_children,)*
                        __parent: #root,
                        __state: ::core::marker::PhantomData,
                    }
                }
            }
        });
        out
    }
}

fn doc(text: &str) -> TokenStream {
    let text = format!(" {text}");
    quote!(#[doc = #text])
}

fn builder_field(name: &Ident) -> Ident {
    format_ident!("__{}", name.to_string().trim_start_matches("r#"))
}

fn type_key(ty: &Type) -> String {
    ty.to_token_stream().to_string()
}

fn fmt(args: &Args, item: &ItemStruct, statement: bool) -> syn::Result<TokenStream> {
    let krate = krate();
    let ident = &item.ident;
    let mut out = TokenStream::new();
    for (option, allowed, macro_name) in [
        (&args.display, ["sql", "name"], quote!(impl_display)),
        (&args.debug, ["sql", "tree"], quote!(impl_debug)),
    ] {
        let Some(value) = option else { continue };
        if !allowed.iter().any(|allowed| value == allowed) {
            return Err(syn::Error::new(
                value.span(),
                format!("expected `{}` or `{}`", allowed[0], allowed[1]),
            ));
        }
        if !item.generics.params.is_empty() {
            return Err(syn::Error::new(
                value.span(),
                "generic items can't take `display`/`debug`; give them to a `#[statement]` for an instantiation",
            ));
        }
        if !named_fields(item).is_empty() {
            return Err(syn::Error::new(
                value.span(),
                "a struct with fields is its own row type, so `display`/`debug` would describe each row; implement them yourself",
            ));
        }
        if value == "sql" && !statement {
            return Err(syn::Error::new(
                value.span(),
                "tables have no SQL of their own",
            ));
        }
        out.extend(quote!(#krate::#macro_name!(#ident => #value);));
    }
    Ok(out)
}
