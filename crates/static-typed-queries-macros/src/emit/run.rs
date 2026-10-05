use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};
use syn::{Ident, ItemStruct, Type};

use crate::args::{Fetch, Step};
use crate::emit::{doc, docs, krate};

// A statement binds itself with `query` when it has no hooks, and otherwise runs with its hooks
// in one transaction through `with` and `run`; `Single` and `Hooked` keep the wrong set out.
pub(crate) fn statement_methods(
    input: &ItemStruct,
    dialect: &Type,
    row: Option<&Type>,
) -> TokenStream {
    let krate = krate();
    let ident = &input.ident;
    let driver = quote!(#krate::driver);
    let sqlx = quote!(#krate::sqlx);
    let error = quote!(#sqlx::Error);
    let acquire = quote!(impl #sqlx::Acquire<'c, Database = #driver::Database<#dialect>>);
    let query = match row {
        Some(row) => {
            let doc = doc(&format!(
                "Binds the values to the SQL as a `sqlx` query that reads each row as {}.",
                docs::link(row, &[])
            ));
            quote! {
                #doc
                pub fn query<'q, __I>(
                    &self,
                ) -> ::core::result::Result<#driver::QueryAs<'q, #dialect, #row>, #error>
                where
                    Self: #krate::Single<__I>,
                {
                    #krate::query_as::<Self, #row, __I>(self)
                }
            }
        }
        None => quote! {
            /// Binds the values to the SQL as a `sqlx` query.
            pub fn query<'q, __I>(
                &self,
            ) -> ::core::result::Result<#driver::Query<'q, #dialect>, #error>
            where
                Self: #krate::Single<__I>,
            {
                #krate::query::<Self, __I>(self)
            }
        },
    };
    let (output, run_doc) = match row {
        Some(row) => (
            quote!(::std::vec::Vec<#row>),
            doc(&format!(
                "Runs the statement and its hooks in one transaction, and reads each row of the statement as {}.",
                docs::link(row, &[])
            )),
        ),
        None => (
            quote!(u64),
            doc(
                "Runs the statement and its hooks in one transaction, and returns the number of rows the statement affected.",
            ),
        ),
    };
    let fetch = match row {
        Some(row) => quote!(#krate::run::AllRows<#row>),
        None => quote!(#krate::run::Affected),
    };
    quote! {
        #krate::__if_sqlx! {
            impl #ident {
                #query

                /// Binds the values to the SQL as a `sqlx` query that reads each row as `O`.
                pub fn query_as<'q, O, __I>(
                    &self,
                ) -> ::core::result::Result<#driver::QueryAs<'q, #dialect, O>, #error>
                where
                    O: for<'r> #sqlx::FromRow<'r, #driver::Row<#dialect>>,
                    Self: #krate::Single<__I>,
                {
                    #krate::query_as::<Self, O, __I>(self)
                }

                /// Gives the values of a hook with parameters. Each hook takes its values once and runs once.
                pub fn with<__H, __G, __I>(
                    self,
                    values: __H,
                ) -> #krate::With<Self, (#krate::HookValues<__H>, ())>
                where
                    Self: #krate::Hooked<__G>,
                    __H: #krate::Values,
                    (#krate::HookValues<__H>, ()): #krate::Provides<__H, __I>,
                {
                    #krate::With::new(self).with(values)
                }

                #run_doc
                pub async fn run<'c, __G, __I>(
                    self,
                    conn: #acquire,
                ) -> ::core::result::Result<#output, #error>
                where
                    Self: #krate::Hooked<__G> + #krate::HookNeeds<(), __I>,
                {
                    #krate::With::new(self).run(conn).await
                }

                /// Runs the statement and its hooks in one transaction, and reads each row of the statement as `O`.
                pub async fn run_as<'c, O, __G, __I>(
                    self,
                    conn: #acquire,
                ) -> ::core::result::Result<::std::vec::Vec<O>, #error>
                where
                    O: for<'r> #sqlx::FromRow<'r, #driver::Row<#dialect>>
                        + ::core::marker::Send
                        + ::core::marker::Unpin,
                    Self: #krate::Hooked<__G> + #krate::HookNeeds<(), __I>,
                {
                    #krate::With::new(self).run_as(conn).await
                }
            }

            impl #krate::Run for #ident {
                type Output = #output;
                const BEFORE: &'static [#krate::Hook] = <Self as #krate::Statement>::BEFORE;
                const AFTER: &'static [#krate::Hook] = <Self as #krate::Statement>::AFTER;

                async fn run(
                    self,
                    conn: &mut #driver::Connection<#dialect>,
                ) -> ::core::result::Result<#output, #error> {
                    #krate::run::statement::<#dialect, #fetch, Self>(&self, conn).await
                }
            }
        }
    }
}

pub(crate) fn transaction_methods(
    input: &ItemStruct,
    dialect: &Type,
    steps: &[Step],
) -> TokenStream {
    let krate = krate();
    let ident = &input.ident;
    let driver = quote!(#krate::driver);
    let sqlx = quote!(#krate::sqlx);
    let error = quote!(#sqlx::Error);
    let transaction = quote!(<#ident as #krate::Transaction>);
    let conn = format_ident!("conn");
    let (runs, names, outputs) = run_steps(ident, steps, dialect, &conn, &mut (0, 0));
    quote! {
        #krate::__if_sqlx! {
            impl #ident {
                /// Gives the values of a hook with parameters. Each hook takes its values once and runs once.
                pub fn with<__H, __I>(
                    self,
                    values: __H,
                ) -> #krate::With<Self, (#krate::HookValues<__H>, ())>
                where
                    __H: #krate::Values,
                    (#krate::HookValues<__H>, ()): #krate::Provides<__H, __I>,
                {
                    #krate::With::new(self).with(values)
                }

                /// Runs the steps in order in one transaction, with each hook once around them, and returns the output of each step.
                pub async fn run<'c, __I>(
                    self,
                    conn: impl #sqlx::Acquire<'c, Database = #driver::Database<#dialect>>,
                ) -> ::core::result::Result<(#(#outputs,)*), #error>
                where
                    Self: #krate::HookNeeds<(), __I>,
                {
                    #krate::With::new(self).run(conn).await
                }
            }

            impl #krate::Run for #ident {
                type Output = (#(#outputs,)*);
                const BEFORE: &'static [#krate::Hook] = #transaction::BEFORE;
                const AFTER: &'static [#krate::Hook] = #transaction::AFTER;

                async fn run(
                    self,
                    conn: &mut #driver::Connection<#dialect>,
                ) -> ::core::result::Result<(#(#outputs,)*), #error> {
                    #(#runs)*
                    ::core::result::Result::Ok((#(#names,)*))
                }
            }
        }
    }
}

fn run_steps(
    ident: &Ident,
    steps: &[Step],
    dialect: &Type,
    conn: &Ident,
    counts: &mut (usize, usize),
) -> (Vec<TokenStream>, Vec<Ident>, Vec<TokenStream>) {
    let krate = krate();
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
                        &self,
                        &<#ident as #krate::Transaction>::STEPS[#index],
                        &mut *#conn,
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
                    run_steps(ident, steps, dialect, &savepoint, counts);
                statements.push(quote! {
                    let #name = {
                        let mut #savepoint = #sqlx::Acquire::begin(&mut *#conn).await?;
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
                outputs.push(quote!(::core::result::Result<(#(#inner_outputs,)*), #sqlx::Error>));
                names.push(name);
            }
        }
    }
    (statements, names, outputs)
}
