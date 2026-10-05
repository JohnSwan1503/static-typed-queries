use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};
use syn::{Ident, Type};

use super::Builder;
use crate::args::{Fetch, Step};
use crate::emit::{doc, docs};

impl Builder<'_, '_> {
    pub(super) fn query(&self, dialect: &Type, row: Option<&Type>) -> TokenStream {
        let (b, krate) = (&self.b, &self.krate);
        let ident = &self.item.input.ident;
        let driver = quote!(#krate::driver);
        let error = quote!(#krate::sqlx::Error);
        let unhooked = self.complete(&quote!(#b::NoHooks), &quote!(()));
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
                        #krate::query_as::<#ident, #row>(&#b::Finish::finish(self))
                    }
                }
            }
            None => quote! {
                /// Binds the parameters to the SQL as a `sqlx` query.
                pub fn query<'q>(
                    self,
                ) -> ::core::result::Result<#driver::Query<'q, #dialect>, #error> {
                    #krate::query::<#ident>(&#b::Finish::finish(self))
                }
            },
        };
        quote! {
            #krate::__if_sqlx! {
                impl #unhooked {
                    #query

                    /// Binds the parameters to the SQL as a `sqlx` query that reads each row as `O`.
                    pub fn query_as<'q, O>(
                        self,
                    ) -> ::core::result::Result<#driver::QueryAs<'q, #dialect, O>, #error>
                    where
                        O: for<'r> #krate::sqlx::FromRow<'r, #driver::Row<#dialect>>,
                    {
                        #krate::query_as::<#ident, O>(&#b::Finish::finish(self))
                    }
                }
            }
        }
    }

    pub(super) fn run(&self, dialect: &Type, row: Option<&Type>) -> TokenStream {
        let (b, krate) = (&self.b, &self.krate);
        let ident = &self.item.input.ident;
        let driver = quote!(#krate::driver);
        let sqlx = quote!(#krate::sqlx);
        let error = quote!(#sqlx::Error);
        let run = quote!(#krate::run);
        let acquire = quote!(impl #sqlx::Acquire<'c, Database = #driver::Database<#dialect>>);
        let covered = quote!(#ident: #b::HookNeeds<__V, __I>);
        let hooked = self.complete(&quote!(#b::WithHooks), &self.values);
        let params = self.params();
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
                        let params = #params;
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
                    let params = #params;
                    #run::execute::<#ident, __V, _>(&params, &self.__values, conn).await
                }
            },
        };
        quote! {
            #krate::__if_sqlx! {
                impl<__V> #hooked
                where
                    __V: #krate::BindHooks<#driver::Database<#dialect>>,
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
                        let params = #params;
                        #run::fetch_all::<#ident, O, __V, _>(&params, &self.__values, conn).await
                    }
                }
            }
        }
    }

    pub(super) fn with(&self) -> TokenStream {
        let (b, krate) = (&self.b, &self.krate);
        let (name, fields, values, root) = (self.name, &self.fields, &self.values, &self.root);
        let with_hooks = quote!(#b::WithHooks);
        let mut params = self.states.clone();
        params.push(values.clone());
        let generics = self.generics(&params);
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let open = self.ty(&self.states, &with_hooks, values, root);
        let given = self.ty(
            &self.states,
            &with_hooks,
            &quote!((#b::HookValues<__Hook>, __V)),
            root,
        );
        quote! {
            impl #impl_generics #open #where_clause {
                /// Gives the values of a hook with parameters, from its complete builder. Each hook takes its values once and runs once.
                pub fn with<__B, __Hook, __Once>(self, values: __B) -> #given
                where
                    __B: #b::Finish,
                    __B::Params: #b::ParamsOf<Item = __Hook>,
                    __Hook: #krate::Sql,
                    (#b::HookValues<__Hook>, __V): #b::Provides<__Hook, __Once>,
                {
                    #name {
                        #(#fields: self.#fields,)*
                        __values: #b::with(values, self.__values),
                        __parent: #root,
                        __state: ::core::marker::PhantomData,
                    }
                }
            }
        }
    }

    pub(super) fn transaction_run(&self, steps: &[Step], dialect: &Type) -> TokenStream {
        let (b, krate) = (&self.b, &self.krate);
        let ident = &self.item.input.ident;
        let run = quote!(#krate::run);
        let driver = quote!(#krate::driver);
        let sqlx = quote!(#krate::sqlx);
        let transaction = quote!(<#ident as #krate::Transaction>);
        let tx = format_ident!("tx");
        let (runs, names, outputs) = self.run_steps(steps, dialect, &tx, &mut (0, 0));
        let complete = self.complete(&self.hooks, &self.values);
        let params = self.params();
        let generics = self.generics(&[self.hooks.clone(), self.values.clone()]);
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
                        __V: #krate::BindHooks<#driver::Database<#dialect>>,
                    {
                        let params = #params;
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

    fn run_steps(
        &self,
        steps: &[Step],
        dialect: &Type,
        conn: &Ident,
        counts: &mut (usize, usize),
    ) -> (Vec<TokenStream>, Vec<Ident>, Vec<TokenStream>) {
        let krate = &self.krate;
        let ident = &self.item.input.ident;
        let run = quote!(#krate::run);
        let sqlx = quote!(#krate::sqlx);
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
                            &<#ident as #krate::Transaction>::STEPS[#index],
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
}
