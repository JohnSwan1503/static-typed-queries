use proc_macro2::{Literal, Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{GenericParam, Generics, Ident, ItemStruct, LitStr, Type, parse_quote};

use crate::analyze::{self, Analysis, Engine, Kind};
use crate::args::{Args, Placement};
use crate::naming::{camel, field_name, snake_case, to_ident, unique};
use crate::template::{self, Segment, Template};

const RESERVED: &[&str] = &["build", "builder", "finish"];

fn krate() -> TokenStream {
    quote!(::static_typed_queries)
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
    );

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
    let node = node(
        &node_name,
        fingerprint(ident, &item),
        kind,
        inject,
        parts(&template, &analysis),
    );

    let dialect = &args.dialect;
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
    let params = Params::new(&template, &analysis, &item, sql)?;
    let params_ty = params.ty();
    let params_struct = params.definition();
    let bind_params = params.bind_impl();
    let builder = params.builder();
    let statement = item.generics.params.is_empty().then(|| {
        quote! {
            #krate::impl_statement!(#ident);

            impl #ident {
                pub const SQL: &'static str = <Self as #krate::statement::Statement>::SQL;
            }
        }
    });

    Ok(quote! {
        #item

        #params_struct

        impl #impl_generics #krate::sql::Sql for #ident #ty_generics #where_clause {
            type Dialect = #dialect;
            type Params = #params_ty;
            const NODE: &'static #krate::node::Node = #node;
        }

        #bind_params
        #builder
        #statement
    })
}

fn node(
    name: &str,
    fingerprint: TokenStream,
    kind: TokenStream,
    inject: TokenStream,
    parts: Vec<TokenStream>,
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

fn parts(template: &Template, analysis: &Analysis) -> Vec<TokenStream> {
    let krate = krate();
    let mut positions = analysis.refs.iter();
    template
        .segments
        .iter()
        .map(|segment| match segment {
            Segment::Lit(text) => quote!(#krate::part::lit::Lit::part(#text)),
            Segment::Param(slot) => {
                let slot = Literal::u16_unsuffixed(*slot);
                quote!(#krate::part::param::Param::part(#slot))
            }
            Segment::Ref(item) => {
                let position = positions.next().expect("a position for every reference");
                let ty = &item.ty;
                let node = quote!(<#ty as #krate::sql::Sql>::NODE);
                if !position.from {
                    return quote!(#krate::part::expr::Expr::part(#node));
                }
                let rule = if position.given_alias {
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
    state: Ident,
}

struct Child {
    name: Ident,
    field: Ident,
    ty: Type,
    state: Ident,
}

struct Params<'a> {
    item: &'a ItemStruct,
    ident: Ident,
    builder: Ident,
    groups: Vec<Group>,
    children: Vec<Child>,
}

impl<'a> Params<'a> {
    fn new(
        template: &Template,
        analysis: &Analysis,
        item: &'a ItemStruct,
        sql: &LitStr,
    ) -> syn::Result<Params<'a>> {
        let mut groups: Vec<Group> = Vec::new();
        for (slot, param) in template.params.iter().enumerate() {
            let name = match (&param.name, &analysis.names[slot]) {
                (Some(name), _) => name.clone(),
                (None, Some(inferred)) => to_ident(inferred),
                (None, None) => format_ident!("bind{}", slot + 1),
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
                }
                None => groups.push(Group {
                    state: format_ident!("Param{}", camel(&name)),
                    field: builder_field(&name),
                    name,
                    ty: param.ty.clone(),
                    slots: vec![slot as u16],
                }),
            }
        }

        let mut taken: Vec<String> = groups
            .iter()
            .map(|group| group.name.to_string())
            .chain(RESERVED.iter().map(|name| (*name).to_owned()))
            .collect();
        let children = template
            .children
            .iter()
            .map(|ty| {
                let name = unique(field_name(ty), &taken);
                taken.push(name.clone());
                let name = format_ident!("{name}");
                Child {
                    state: format_ident!("Item{}", camel(&name)),
                    field: builder_field(&name),
                    name,
                    ty: ty.clone(),
                }
            })
            .collect();
        Ok(Params {
            item,
            ident: format_ident!("{}Params", item.ident),
            builder: format_ident!("{}Builder", item.ident),
            groups,
            children,
        })
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
            match group.slots.len() {
                1 => quote!(pub #name: #ty),
                len => {
                    let len = Literal::usize_unsuffixed(len);
                    quote!(pub #name: [#ty; #len])
                }
            }
        });
        let children = self.children.iter().map(|child| {
            let (name, ty) = (&child.name, &child.ty);
            quote!(pub #name: <#ty as #krate::sql::Sql>::Params)
        });
        Some(quote! {
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

    fn builder(&self) -> TokenStream {
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
        let mut out = TokenStream::new();

        let generics = self.generics(&states);
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
        out.extend(quote! {
            #vis struct #builder #generics #where_clause {
                #(#group_fields,)*
                #(#child_fields,)*
                __state: ::core::marker::PhantomData<fn() -> (#(#group_states,)* #(#args,)*)>,
            }
        });

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
            free.push(quote!(__Rest: #krate::builder::Remaining));
            let generics = self.generics(&free);
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let current = self.builder_ty(&current);
            let next = self.builder_ty(&next);
            let others = fields.iter().filter(|other| **other != field);
            out.extend(quote! {
                impl #impl_generics #current #where_clause {
                    pub fn #name(self, value: #ty) -> #next {
                        let mut #field = self.#field;
                        #field[#len - 1 - <__Rest as #krate::builder::Remaining>::N] =
                            ::core::option::Option::Some(value);
                        #builder {
                            #field,
                            #(#others: self.#others,)*
                            __state: ::core::marker::PhantomData,
                        }
                    }
                }
            });
        }

        for (index, child) in self.children.iter().enumerate() {
            let name = &child.name;
            let field = &child.field;
            let ty = &child.ty;
            let mut next = states.clone();
            next[self.groups.len() + index] = quote!(__Next);
            let mut generics = self.generics(&states);
            generics
                .make_where_clause()
                .predicates
                .push(parse_quote!(#ty: #krate::builder::Build));
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let current = self.builder_ty(&states);
            let next = self.builder_ty(&next);
            let others = fields.iter().filter(|other| **other != field);
            out.extend(quote! {
                impl #impl_generics #current #where_clause {
                    pub fn #name<__Next>(
                        self,
                        build: impl ::core::ops::FnOnce(<#ty as #krate::builder::Build>::Builder) -> __Next,
                    ) -> #next {
                        #builder {
                            #field: build(<#ty as #krate::builder::Build>::builder()),
                            #(#others: self.#others,)*
                            __state: ::core::marker::PhantomData,
                        }
                    }
                }
            });
        }

        let child_states: Vec<TokenStream> = self
            .children
            .iter()
            .map(|child| child.state.to_token_stream())
            .collect();
        let mut generics = self.generics(&child_states);
        for child in &self.children {
            let (state, ty) = (&child.state, &child.ty);
            generics.make_where_clause().predicates.push(parse_quote!(
                #state: #krate::builder::Finish<<#ty as #krate::sql::Sql>::Params>
            ));
        }
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let complete: Vec<TokenStream> = self
            .groups
            .iter()
            .map(|_| quote!(#krate::builder::Set))
            .chain(child_states.iter().cloned())
            .collect();
        let complete = self.builder_ty(&complete);
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
            impl #impl_generics #krate::builder::Finish<#params_ty> for #complete #where_clause {
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
        let any = self.builder_ty(&states);
        out.extend(quote! {
            impl #impl_generics #any #where_clause {
                pub fn build(self) -> #params_ty
                where
                    Self: #krate::builder::Finish<#params_ty>,
                {
                    #krate::builder::Finish::finish(self)
                }
            }
        });

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
                        __state: ::core::marker::PhantomData,
                    }
                }
            }
        });
        out
    }
}

fn builder_field(name: &Ident) -> Ident {
    format_ident!("__{}", name.to_string().trim_start_matches("r#"))
}

fn type_key(ty: &Type) -> String {
    ty.to_token_stream().to_string()
}
