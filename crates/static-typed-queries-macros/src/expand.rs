use proc_macro2::{Literal, Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{
    GenericArgument, GenericParam, Generics, Ident, ItemStruct, LitStr, PathArguments, Type,
    parse_quote,
};

use crate::analyze::{self, Analysis, Engine, Kind};
use crate::args::{Args, Placement};
use crate::docs;
use crate::naming::{camel, field_name, short_name, snake_case, to_ident, unique};
use crate::template::{self, Segment, Template};

const RESERVED: &[&str] = &["build", "builder", "finish", "query"];

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
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "tables can't be generic",
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
    let node_name = table.rsplit('.').next().unwrap_or(&table);
    let ident = &item.ident;
    let dialect = &args.dialect;
    let node = node(
        node_name,
        fingerprint(ident, &item),
        quote!(Table),
        quote!(#krate::node::inject::Inject::Ident),
        parts,
        &[],
    );
    let fmt = fmt(&args, &item, false)?;

    Ok(quote! {
        #item

        impl #krate::sql::Sql for #ident {
            type Dialect = #dialect;
            type Params = ();
            const NODE: &'static #krate::node::Node = #node;
        }

        impl #krate::builder::Build for #ident {
            type Builder = #krate::builder::NoParams;

            fn builder() -> Self::Builder {
                #krate::builder::NoParams
            }
        }

        #fmt
    })
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
    let template = template::parse(sql)?;
    let analysis = analyze::analyze(&template, Engine::of(&args.dialect), sql)?;
    let kind = match analysis.kind {
        Kind::Query => quote!(Query),
        Kind::Dml => quote!(Dml),
        Kind::Ddl => quote!(Ddl),
    };

    let ident = &item.ident;
    let node_name = args
        .name
        .as_ref()
        .map_or_else(|| snake_case(&ident.to_string()), LitStr::value);
    let inject = placement(args.placement.unwrap_or(Placement::Subquery));
    let wrappers = separate(&args, &item, &template)?;
    let separate: Vec<(Type, Type)> = wrappers
        .iter()
        .map(|(named, wrapper)| {
            let wrapper = &wrapper.ident;
            (named.clone(), parse_quote!(#wrapper))
        })
        .collect();
    let items = items(&template, &item.generics, &separate);
    let params = Params::new(&template, &analysis, &items, &item, sql)?;
    let fields: Vec<Type> = items.iter().map(|(ty, _)| ty.clone()).collect();
    let node = node(
        &node_name,
        fingerprint(ident, &item),
        kind,
        inject,
        parts(&template, &analysis, &params),
        &fields,
    );
    let wrappers = wrappers
        .iter()
        .map(|(named, wrapper)| self::wrapper(named, wrapper, &args.dialect, sql))
        .collect::<syn::Result<Vec<_>>>()?;

    let dialect = &args.dialect;
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
    let params_ty = params.ty();
    let params_struct = params.definition();
    let bind_params = params.bind_impl();
    let builder = params.builder(dialect);
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
        quote! {
            #krate::impl_statement!(#ident);

            impl #ident {
                /// The SQL, rendered at compile time.
                pub const SQL: &'static str = <Self as #krate::statement::Statement>::SQL;
            }

            #test
        }
    });
    let fmt = fmt(&args, &item, true)?;
    let mut documented = item.clone();
    documented.attrs.extend(params.item_docs(sql));

    Ok(quote! {
        #documented

        #params_struct

        impl #impl_generics #krate::sql::Sql for #ident #ty_generics #where_clause {
            type Dialect = #dialect;
            type Params = #params_ty;
            const NODE: &'static #krate::node::Node = #node;
        }

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
        refs: Vec::new(),
        names: Vec::new(),
    };
    let mut params = Params::new(&template, &analysis, &items, item, sql)?;
    params.synthetic = true;
    let ident = &item.ident;
    let fields: Vec<Type> = items.iter().map(|(ty, _)| ty.clone()).collect();
    let node = node(
        &snake_case(&ident.to_string()),
        fingerprint(ident, item),
        quote!(Scope),
        quote!(#krate::node::inject::Inject::Subquery),
        Vec::new(),
        &fields,
    );
    let params_ty = params.ty();
    let params_struct = params.definition();
    let bind_params = params.bind_impl();
    let builder = params.builder(dialect);
    Ok(quote! {
        #item

        #params_struct

        impl #krate::sql::Sql for #ident {
            type Dialect = #dialect;
            type Params = #params_ty;
            const NODE: &'static #krate::node::Node = #node;
        }

        #bind_params
        #builder
    })
}

fn node(
    name: &str,
    fingerprint: TokenStream,
    kind: TokenStream,
    inject: TokenStream,
    parts: Vec<TokenStream>,
    items: &[Type],
) -> TokenStream {
    let krate = krate();
    quote! {
        &#krate::node::Node {
            name: #krate::node::name::Name::new(#name),
            fingerprint: #fingerprint,
            kind: #krate::node::kind::Kind::#kind,
            inject: #inject,
            parts: #krate::part::Parts(&[#(#parts),*]),
            before: #krate::node::before::Before(&[]),
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
}

struct Child {
    name: Ident,
    field: Ident,
    ty: Type,
    named: Type,
    state: Ident,
}

struct Params<'a> {
    item: &'a ItemStruct,
    ident: Ident,
    builder: Ident,
    groups: Vec<Group>,
    children: Vec<Child>,
    synthetic: bool,
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
                    field: builder_field(&name),
                    name,
                    ty: param.ty.clone(),
                    slots: vec![slot as u16],
                    origins: vec![origin],
                }),
            }
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

    fn item_docs(&self, sql: &LitStr) -> Vec<syn::Attribute> {
        let mut lines = Vec::new();
        if self
            .item
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("doc"))
        {
            lines.push(String::new());
        }
        lines.push("# Template".to_owned());
        lines.push(String::new());
        lines.push("```sql".to_owned());
        lines.extend(docs::template(&sql.value()));
        lines.push("```".to_owned());
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
        Some(quote! {
            #doc
            #hidden
            #vis struct #ident #generics #where_clause {
                #(#groups,)*
                #(#children,)*
            }
        })
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

    fn builder_ty(&self, states: &[TokenStream]) -> TokenStream {
        let builder = &self.builder;
        let args = self.item_args();
        quote!(#builder<#(#args,)* #(#states),*>)
    }

    fn builder_in(&self, states: &[TokenStream], parent: &TokenStream) -> TokenStream {
        let builder = &self.builder;
        let args = self.item_args();
        quote!(#builder<#(#args,)* #(#states,)* #parent>)
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

    fn fields(&self) -> Vec<&Ident> {
        self.groups
            .iter()
            .map(|group| &group.field)
            .chain(self.children.iter().map(|child| &child.field))
            .collect()
    }

    fn builder(&self, dialect: &Type) -> TokenStream {
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

        let builder = &self.builder;
        let vis = &self.item.vis;
        let states = self.states();
        let params_ty = self.ty();
        let args = self.item_args();
        let fields = self.fields();
        let fill = quote!(#krate::builder::Fill);
        let root = quote!(#krate::builder::Root);
        let parent = quote!(__K);
        let with_parent = |extra: &[TokenStream]| {
            let mut params = extra.to_vec();
            params.push(parent.clone());
            self.generics(&params)
        };
        let mut out = TokenStream::new();

        let mut struct_params = states.clone();
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
                "There's a setter for every parameter of [`{ident}`], and a method for every item it references that moves to that item's builder for one setter call. `build` only compiles once every parameter is set."
            ),
        ]);
        out.extend(quote! {
            #(#builder_doc)*
            #hidden
            #vis struct #builder #generics #where_clause {
                #(#group_fields,)*
                #(#child_fields,)*
                __parent: __K,
                __state: ::core::marker::PhantomData<fn() -> (#(#group_states,)* #(#args,)*)>,
            }
        });

        let module = self.module();
        let mut traits = TokenStream::new();
        for (index, group) in self.groups.iter().enumerate() {
            let name = &group.name;
            let field = &group.field;
            let ty = &group.ty;
            let len = Literal::usize_unsuffixed(group.slots.len());
            let mut current = states.clone();
            current[index] = quote!(#krate::builder::Unset<__Rest>);
            let mut next = states.clone();
            next[index] = quote!(__Rest);
            let mut free: Vec<TokenStream> = states
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != index)
                .map(|(_, state)| state.clone())
                .collect();
            let generics = with_parent(&free);
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let mut done = states.clone();
            done[index] = quote!(#krate::builder::Set);
            let done = self.builder_in(&done, &parent);
            let again = format_ident!("{}Again", group.state);
            let (message, label) = match group.slots.len() {
                1 => (
                    format!("`{name}` is already set on the `{ident}` builder"),
                    format!("`{name}` takes one value"),
                ),
                len => (
                    format!("all {len} `{name}` values are already set on the `{ident}` builder"),
                    format!("`{name}` takes {len} values, one per appearance in the template"),
                ),
            };
            traits.extend(quote! {
                #[diagnostic::on_unimplemented(message = #message, label = #label)]
                pub trait #again {}

                #[diagnostic::do_not_recommend]
                impl #again for ::core::convert::Infallible {}
            });
            out.extend(quote! {
                impl #impl_generics #done #where_clause {
                    #[doc(hidden)]
                    pub fn #name<__Value: #module::#again>(self, _: __Value) -> Self {
                        self
                    }
                }
            });

            free.push(quote!(__Rest: #krate::builder::Remaining));
            let next = self.builder_ty(&next);
            let mut generics = with_parent(&free);
            generics
                .make_where_clause()
                .predicates
                .push(parse_quote!(__K: #fill<#next>));
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let current = self.builder_in(&current, &parent);
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
                    pub fn #name(self, value: #ty) -> <__K as #fill<#next>>::Output {
                        let mut #field = self.#field;
                        #field[#len - 1 - <__Rest as #krate::builder::Remaining>::N] =
                            ::core::option::Option::Some(value);
                        #fill::fill(self.__parent, #builder {
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
            let (name, field, state) = (&child.name, &child.field, &child.state);
            let hole = format_ident!("__{}{}", builder, camel(name));
            let others: Vec<&&Ident> = fields.iter().filter(|other| **other != field).collect();
            let mut vacant = states.clone();
            vacant[slot] = quote!(());
            let vacant = self.builder_in(&vacant, &parent);

            let generics = with_parent(&states);
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let current = self.builder_in(&states, &parent);
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
                        #state: #krate::builder::Scope<#hole<#vacant>, Scoped = __Scoped>,
                    {
                        #krate::builder::Scope::scope(self.#field, #hole(#builder {
                            #field: (),
                            #(#others: self.#others,)*
                            __parent: self.__parent,
                            __state: ::core::marker::PhantomData,
                        }))
                    }
                }
            });

            let mut filled = states.clone();
            filled[slot] = quote!(__Item);
            let filled = self.builder_ty(&filled);
            let mut free: Vec<TokenStream> = states
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != slot)
                .map(|(_, state)| state.clone())
                .collect();
            free.push(quote!(__Item));
            let mut generics = with_parent(&free);
            generics
                .make_where_clause()
                .predicates
                .push(parse_quote!(__K: #fill<#filled>));
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            out.extend(quote! {
                impl #impl_generics #fill<__Item> for #hole<#vacant> #where_clause {
                    type Output = <__K as #fill<#filled>>::Output;

                    fn fill(self, item: __Item) -> Self::Output {
                        let parent = self.0;
                        #fill::fill(parent.__parent, #builder {
                            #field: item,
                            #(#others: parent.#others,)*
                            __parent: #root,
                            __state: ::core::marker::PhantomData,
                        })
                    }
                }
            });
        }

        let generics = with_parent(&states);
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let any = self.builder_ty(&states);
        let scoped = self.builder_in(&states, &parent);
        out.extend(quote! {
            #[diagnostic::do_not_recommend]
            impl #impl_generics #krate::builder::Scope<__K> for #any #where_clause {
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

        let mut ready: Vec<syn::WherePredicate> = Vec::new();
        for group in &self.groups {
            let (name, state) = (&group.name, &group.state);
            let (message, label) = match group.slots.len() {
                1 => (
                    format!("`{ident}` is missing `{name}`"),
                    format!("call `.{name}(…)` on the `{ident}` builder first"),
                ),
                len => (
                    format!("`{ident}` is missing values for `{name}`"),
                    format!(
                        "call `.{name}(…)` {len} times on the `{ident}` builder first, once per value"
                    ),
                ),
            };
            traits.extend(quote! {
                #[diagnostic::on_unimplemented(message = #message, label = #label)]
                pub trait #state {}

                #[diagnostic::do_not_recommend]
                impl #state for #krate::builder::Set {}
            });
            ready.push(parse_quote!(#state: #module::#state));
        }
        for child in &self.children {
            let (state, ty) = (&child.state, &child.ty);
            ready.push(parse_quote!(
                #state: #krate::builder::Finish<<#ty as #krate::sql::Sql>::Params>
            ));
        }
        if !self.groups.is_empty() {
            out.extend(quote! {
                #[doc(hidden)]
                #vis mod #module {
                    #traits
                }
            });
        }
        let mut generics = self.generics(&states);
        generics
            .make_where_clause()
            .predicates
            .extend(ready.iter().cloned());
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
            quote!(#name: #krate::builder::Finish::finish(self.#field))
        });
        out.extend(quote! {
            impl #impl_generics #krate::builder::Finish<#params_ty> for #any #where_clause {
                fn finish(self) -> #params_ty {
                    #params_ident {
                        #(#extract_groups,)*
                        #(#extract_children,)*
                    }
                }
            }
        });

        let generics = self.generics(&states);
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        out.extend(quote! {
            impl #impl_generics #any #where_clause {
                /// Returns the parameters. Compiles only once every parameter is set.
                pub fn build(self) -> #params_ty
                where
                    #(#ready,)*
                {
                    #krate::builder::Finish::finish(self)
                }
            }
        });
        if self.item.generics.params.is_empty() && !self.synthetic {
            out.extend(quote! {
                #krate::__if_sqlx! {
                    impl #impl_generics #any #where_clause {
                        /// Binds the parameters to the SQL as a `sqlx` query. Compiles only
                        /// once every parameter is set.
                        pub fn query<'q>(
                            self,
                        ) -> ::core::result::Result<
                            #krate::dialect::driver::Query<'q, #dialect>,
                            #krate::__private::sqlx::Error,
                        >
                        where
                            #(#ready,)*
                        {
                            <#ident as #krate::statement::Statement>::query(
                                &#krate::builder::Finish::finish(self),
                            )
                        }
                    }
                }
            });
        }

        let initial: Vec<TokenStream> = self
            .groups
            .iter()
            .map(|group| {
                (0..group.slots.len()).fold(
                    quote!(#krate::builder::Set),
                    |rest, _| quote!(#krate::builder::Unset<#rest>),
                )
            })
            .chain(self.children.iter().map(|child| {
                let ty = &child.ty;
                quote!(<#ty as #krate::builder::Build>::Builder)
            }))
            .collect();
        let initial = self.builder_ty(&initial);
        let mut generics = self.item.generics.clone();
        for child in &self.children {
            let ty = &child.ty;
            generics
                .make_where_clause()
                .predicates
                .push(parse_quote!(#ty: #krate::builder::Build));
        }
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let empty_groups = self.groups.iter().map(|group| {
            let field = &group.field;
            let len = Literal::usize_unsuffixed(group.slots.len());
            quote!(#field: [const { ::core::option::Option::None }; #len])
        });
        let start_children = self.children.iter().map(|child| {
            let (field, ty) = (&child.field, &child.ty);
            quote!(#field: <#ty as #krate::builder::Build>::builder())
        });
        out.extend(quote! {
            impl #impl_generics #krate::builder::Build for #ident #ty_generics #where_clause {
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
                "generic items can't take `display`/`debug`; use the core macros on an instantiation",
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
