use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};
use syn::{Generics, Ident, Type, parse_quote};

use crate::args::{Fetch, Step};
use crate::emit::docs;
use crate::emit::{doc, krate};
use crate::model::item::{Item, Role};
use crate::naming::camel;

impl<'a> Item<'a> {
    pub(crate) fn builder_in(
        &self,
        states: &[TokenStream],
        hooked: &TokenStream,
        values: &TokenStream,
        parent: &TokenStream,
    ) -> TokenStream {
        let builder = &self.builder;
        let args = self.item_args();
        quote!(#builder<#(#args,)* #(#states,)* #hooked, #values, #parent>)
    }

    pub(crate) fn generics(&self, extra: &[TokenStream]) -> Generics {
        let mut generics = self.input.generics.clone();
        for param in extra {
            generics
                .params
                .push(syn::parse2(param.clone()).expect("a generic parameter"));
        }
        generics
    }

    pub(crate) fn run_steps(
        &self,
        steps: &[Step],
        dialect: &Type,
        conn: &Ident,
        counts: &mut (usize, usize),
    ) -> (Vec<TokenStream>, Vec<Ident>, Vec<TokenStream>) {
        let krate = krate();
        let ident = &self.input.ident;
        let run = quote!(#krate::statement::run);
        let sqlx = quote!(#krate::__private::sqlx);
        let mut statements = Vec::new();
        let mut names = Vec::new();
        let mut outputs = Vec::new();
        for step in steps {
            match step {
                Step::Run(_, Fetch::Cte) => {}
                Step::Run(ty, fetch) => {
                    let name = format_ident!("__step{}", counts.0);
                    let index = Literal::usize_unsuffixed(counts.0);
                    counts.0 += 1;
                    let row = quote!(<#ty as #run::Step>::Row);
                    let (fetch, output) = match fetch {
                        Fetch::Default => (
                            quote!(<#ty as #run::Step>::Fetch),
                            quote!(#run::Output<#ty, #dialect>),
                        ),
                        Fetch::One => (quote!(#run::One<#row>), row),
                        Fetch::Optional => (
                            quote!(#run::Optional<#row>),
                            quote!(::core::option::Option<#row>),
                        ),
                        Fetch::Cte => unreachable!("CTE steps run as part of the next step"),
                    };
                    if !matches!(step, Step::Run(_, Fetch::Default)) {
                        statements.push(quote!(const _: () = #run::rows::<#ty>();));
                    }
                    statements.push(quote! {
                        let #name = #run::step::<#dialect, #fetch, _>(
                            &params,
                            &<#ident as #krate::transaction::Transaction>::STEPS[#index],
                            &mut #conn,
                        )
                        .await?;
                    });
                    outputs.push(output);
                    names.push(name);
                }
                Step::Savepoint(steps) => {
                    let name = format_ident!("__group{}", counts.1);
                    let savepoint = format_ident!("__savepoint{}", counts.1);
                    counts.1 += 1;
                    let (inner, inner_names, inner_outputs) =
                        self.run_steps(steps, dialect, &savepoint, counts);
                    statements.push(quote! {
                        let #name = {
                            let mut #savepoint = #sqlx::Acquire::begin(&mut #conn).await?;
                            let result = async {
                                #(#inner)*
                                ::core::result::Result::Ok::<_, #sqlx::Error>((#(#inner_names,)*))
                            }
                            .await;
                            match result {
                                ::core::result::Result::Ok(output) => {
                                    #savepoint.commit().await?;
                                    ::core::result::Result::Ok(output)
                                }
                                ::core::result::Result::Err(error) => {
                                    #savepoint.rollback().await?;
                                    ::core::result::Result::Err(error)
                                }
                            }
                        };
                    });
                    outputs
                        .push(quote!(::core::result::Result<(#(#inner_outputs,)*), #sqlx::Error>));
                    names.push(name);
                }
            }
        }
        (statements, names, outputs)
    }

    pub(crate) fn transaction_run(
        &self,
        steps: &[Step],
        dialect: &Type,
        complete: &TokenStream,
        finish: &TokenStream,
    ) -> TokenStream {
        let krate = krate();
        let ident = &self.input.ident;
        let b = quote!(#krate::builder);
        let run = quote!(#krate::statement::run);
        let driver = quote!(#krate::dialect::driver);
        let sqlx = quote!(#krate::__private::sqlx);
        let transaction = quote!(<#ident as #krate::transaction::Transaction>);
        let tx = format_ident!("tx");
        let (runs, names, outputs) = self.run_steps(steps, dialect, &tx, &mut (0, 0));
        let generics = self.generics(&[quote!(__H), quote!(__V)]);
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        quote! {
            #krate::__if_sqlx! {
                impl #impl_generics #complete #where_clause {
                    /// Runs the steps in order in one transaction, with each hook once around them, and returns the output of each step.
                    pub async fn run<'c, __I>(
                        self,
                        conn: impl #sqlx::Acquire<'c, Database = #driver::Database<#dialect>>,
                    ) -> ::core::result::Result<(#(#outputs,)*), #sqlx::Error>
                    where
                        #ident: #b::HookNeeds<__V, __I>,
                        __V: #krate::statement::params::BindHooks<#driver::Database<#dialect>>,
                    {
                        let params = #finish;
                        let mut tx = #sqlx::Acquire::begin(conn).await?;
                        #run::hooks::<#dialect, __V>(&self.__values, #transaction::BEFORE, &mut tx).await?;
                        #(#runs)*
                        #run::hooks::<#dialect, __V>(&self.__values, #transaction::AFTER, &mut tx).await?;
                        tx.commit().await?;
                        ::core::result::Result::Ok((#(#names,)*))
                    }
                }
            }
        }
    }

    pub(crate) fn builder(&self, dialect: &Type, row: Option<&Type>) -> TokenStream {
        let krate = krate();
        let ident = &self.input.ident;
        let (impl_generics, ty_generics, where_clause) = self.input.generics.split_for_impl();
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
        let vis = &self.input.vis;
        let module = self.module();
        let states = self.states();
        let params_ty = self.ty();
        let args = self.item_args();
        let fields = self.fields();
        let root = quote!(#b::Root);
        let parent = quote!(__K);
        let hooked = quote!(__H);
        let values = quote!(__V);
        let except = |slot: usize, extra: &[TokenStream]| {
            let mut params: Vec<TokenStream> = states
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != slot)
                .map(|(_, state)| state.clone())
                .collect();
            params.extend(extra.iter().cloned());
            params.push(hooked.clone());
            params.push(values.clone());
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
        struct_params.push(quote!(__V = ()));
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
        let hidden = matches!(self.role, Role::Wrapper).then(|| quote!(#[doc(hidden)]));
        let builder_doc = docs::attrs(&[
            format!(
                "Builds [`{}`]; start with `{}`.",
                self.params_ident,
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
                __values: __V,
                __parent: __K,
                __state: ::core::marker::PhantomData<fn() -> (#(#group_states,)* #(#args,)* __H)>,
            }
        });

        let generics = self.generics(&{
            let mut params = states.clone();
            params.push(hooked.clone());
            params.push(values.clone());
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
        let rooted = self.builder_in(&states, &hooked, &values, &root);
        out.extend(quote! {
            impl #impl_generics #b::Ready for #rooted #where_clause {
                type Out = #all;
            }
        });
        let generics = self.generics(&{
            let mut params = states.clone();
            params.push(parent.clone());
            params.push(hooked.clone());
            params.push(values.clone());
            params
        });
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let scoped = self.builder_in(&states, &hooked, &values, &parent);
        out.extend(quote! {
            impl #impl_generics #b::Scope<__K> for #rooted #where_clause {
                type Scoped = #scoped;

                fn scope(self, parent: __K) -> #scoped {
                    #builder {
                        #(#fields: self.#fields,)*
                        __values: self.__values,
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
            let next = self.builder_in(&next, &hooked, &values, &root);
            let mut generics = except(index, &[quote!(__Rest: #b::Remaining), parent.clone()]);
            generics
                .make_where_clause()
                .predicates
                .push(parse_quote!(__K: #b::Fill<#next>));
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let current = self.builder_in(&current, &hooked, &values, &parent);
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
                            __values: self.__values,
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
            let vacant = self.builder_in(&vacant, &hooked, &values, &parent);
            let mut current = states.clone();
            current[slot] = quote!(#b::Open<__B>);
            let current = self.builder_in(&current, &hooked, &values, &parent);
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
                            __values: self.__values,
                            __parent: self.__parent,
                            __state: ::core::marker::PhantomData,
                        }))
                    }
                }
            });

            let mut filled = states.clone();
            filled[slot] = quote!(<__Item as #b::Settled>::Slot);
            let filled = self.builder_in(&filled, &hooked, &values, &root);
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
                            __values: parent.__values,
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
        let with_hooks = quote!(#b::WithHooks);
        let unhooked = self.builder_in(&complete, &quote!(#b::NoHooks), &quote!(()), &root);
        let run_builder = self.builder_in(&complete, &with_hooks, &values, &root);
        let complete = self.builder_in(&complete, &hooked, &values, &root);
        let generics = self.generics(&[hooked.clone(), values.clone()]);
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let params_ident = &self.params_ident;
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
        let finish = quote! {
            #params_ident {
                #(#extract_groups,)*
                #(#extract_children,)*
                #marker_value
            }
        };
        out.extend(quote! {
            impl #impl_generics #b::Finish for #complete #where_clause {
                type Params = #params_ty;

                fn finish(self) -> #params_ty {
                    #finish
                }
            }

            impl #impl_generics #complete #where_clause {
                /// Returns the parameters.
                pub fn build(self) -> #params_ty {
                    #b::Finish::finish(self)
                }
            }
        });
        if self.runs_as_statement() {
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
            let acquire = quote!(impl #sqlx::Acquire<'c, Database = #driver::Database<#dialect>>);
            let covered = quote!(#ident: #b::HookNeeds<__V, __I>);
            let run_main = match row {
                Some(row) => {
                    let doc = doc(&format!(
                        "Runs the statement and its hooks in one transaction, and reads each row of the statement as {}.",
                        docs::link(row, &[])
                    ));
                    quote! {
                        #doc
                        pub async fn run<'c, __I>(
                            self,
                            conn: #acquire,
                        ) -> ::core::result::Result<::std::vec::Vec<#row>, #error>
                        where
                            #covered,
                        {
                            let params = #finish;
                            #run::fetch_all::<#ident, #row, __V, _>(&params, &self.__values, conn).await
                        }
                    }
                }
                None => quote! {
                    /// Runs the statement and its hooks in one transaction, and returns the number of rows the statement affected.
                    pub async fn run<'c, __I>(
                        self,
                        conn: #acquire,
                    ) -> ::core::result::Result<u64, #error>
                    where
                        #covered,
                    {
                        let params = #finish;
                        #run::execute::<#ident, __V, _>(&params, &self.__values, conn).await
                    }
                },
            };
            out.extend(quote! {
                #krate::__if_sqlx! {
                    impl<__V> #run_builder
                    where
                        __V: #krate::statement::params::BindHooks<#driver::Database<#dialect>>,
                    {
                        #run_main

                        /// Runs the statement and its hooks in one transaction, and reads each row of the statement as `O`.
                        pub async fn run_as<'c, O, __I>(
                            self,
                            conn: #acquire,
                        ) -> ::core::result::Result<::std::vec::Vec<O>, #error>
                        where
                            O: for<'r> #sqlx::FromRow<'r, #driver::Row<#dialect>>
                                + ::core::marker::Send
                                + ::core::marker::Unpin,
                            #covered,
                        {
                            let params = #finish;
                            #run::fetch_all::<#ident, O, __V, _>(&params, &self.__values, conn).await
                        }
                    }
                }
            });
        }

        if self.runs_as_statement() || self.steps().is_some() {
            let mut params = states.clone();
            params.push(values.clone());
            let generics = self.generics(&params);
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let open = self.builder_in(&states, &with_hooks, &values, &root);
            let given = self.builder_in(
                &states,
                &with_hooks,
                &quote!((#b::HookValues<__Hook>, __V)),
                &root,
            );
            out.extend(quote! {
                impl #impl_generics #open #where_clause {
                    /// Gives the values of a hook with parameters, from its complete builder. Each hook takes its values once and runs once.
                    pub fn with<__B, __Hook, __Once>(self, values: __B) -> #given
                    where
                        __B: #b::Finish,
                        __B::Params: #b::ParamsOf<Item = __Hook>,
                        __Hook: #krate::sql::Sql,
                        (#b::HookValues<__Hook>, __V): #b::Provides<__Hook, __Once>,
                    {
                        #builder {
                            #(#fields: self.#fields,)*
                            __values: #b::with(values, self.__values),
                            __parent: #root,
                            __state: ::core::marker::PhantomData,
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
        if let Some(steps) = self.steps() {
            out.extend(self.transaction_run(steps, dialect, &complete, &finish));
        }

        let hooks = if self.runs_as_statement() || self.steps().is_some() {
            quote!(<#ident as #krate::render::Render>::Hooks)
        } else {
            quote!(#b::NoHooks)
        };
        let initial = self.builder_in(&initial, &hooks, &quote!(()), &root);
        let mut generics = self.input.generics.clone();
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
                        __values: (),
                        __parent: #root,
                        __state: ::core::marker::PhantomData,
                    }
                }
            }
        });
        out
    }
}
