use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};
use syn::{Ident, ItemStruct, Type};

use crate::args::{Fetch, Step};
use crate::emit::{doc, docs, krate};

// The methods that bind or run a statement exist twice: on the struct, and on its complete
// builder, which builds the struct first.
enum Receiver<'a> {
    Struct,
    Builder(&'a Ident),
}

impl Receiver<'_> {
    fn subject(&self) -> TokenStream {
        match self {
            Receiver::Struct => quote!(Self),
            Receiver::Builder(ident) => quote!(#ident),
        }
    }

    // The receiver of a method that only reads the values, and the struct it reads.
    fn borrowed(&self) -> (TokenStream, TokenStream) {
        let krate = krate();
        match self {
            Receiver::Struct => (quote!(&self), quote!(self)),
            Receiver::Builder(_) => (quote!(self), quote!(&#krate::Finish::finish(self))),
        }
    }

    fn owned(&self) -> TokenStream {
        let krate = krate();
        match self {
            Receiver::Struct => quote!(self),
            Receiver::Builder(_) => quote!(#krate::Finish::finish(self)),
        }
    }
}

// How a statement reads its rows: each one as its `row`, or without one the number affected.
pub(crate) struct Rows {
    pub(crate) fetch: TokenStream,
    pub(crate) output: TokenStream,
    pub(crate) row: TokenStream,
}

pub(crate) fn rows(row: Option<&Type>) -> Rows {
    let krate = krate();
    let run = quote!(#krate::run);
    match row {
        Some(row) => Rows {
            fetch: quote!(#run::AllRows<#row>),
            output: quote!(::std::vec::Vec<#row>),
            row: quote!(#row),
        },
        None => Rows {
            fetch: quote!(#run::Affected),
            output: quote!(u64),
            row: quote!(#run::NoRow),
        },
    }
}

// Returns the struct's methods, and the methods of its complete builder.
pub(crate) fn statement_methods(
    input: &ItemStruct,
    dialect: &Type,
    row: Option<&Type>,
) -> (TokenStream, TokenStream) {
    let krate = krate();
    let ident = &input.ident;
    let methods = statement_api(dialect, row, &Receiver::Struct);
    let impls = quote! {
        #krate::__if_sqlx! {
            impl #ident {
                #methods
            }
        }
    };
    (
        impls,
        statement_api(dialect, row, &Receiver::Builder(ident)),
    )
}

// A statement binds itself with `query` when it has no hooks, and otherwise runs with its hooks
// in one transaction through `with` and `run`; `Single` and `Hooked` keep the wrong set out.
fn statement_api(dialect: &Type, row: Option<&Type>, receiver: &Receiver) -> TokenStream {
    let krate = krate();
    let driver = quote!(#krate::driver);
    let sqlx = quote!(#krate::sqlx);
    let error = quote!(#sqlx::Error);
    let acquire = quote!(impl #sqlx::Acquire<'c, Database = #driver::Database<#dialect>>);
    let subject = receiver.subject();
    let (borrowed, statement) = receiver.borrowed();
    let owned = receiver.owned();
    let query = match row {
        Some(row) => {
            let doc = doc(&format!(
                "Binds the values to the SQL as a `sqlx` query that reads each row as {}.",
                docs::link(row, &[])
            ));
            quote! {
                #doc
                pub fn query<'q, __I>(
                    #borrowed,
                ) -> ::core::result::Result<#driver::QueryAs<'q, #dialect, #row>, #error>
                where
                    #subject: #krate::Single<__I>,
                {
                    #krate::query_as::<#subject, #row, __I>(#statement)
                }
            }
        }
        None => quote! {
            /// Binds the values to the SQL as a `sqlx` query.
            pub fn query<'q, __I>(
                #borrowed,
            ) -> ::core::result::Result<#driver::Query<'q, #dialect>, #error>
            where
                #subject: #krate::Single<__I>,
            {
                #krate::query::<#subject, __I>(#statement)
            }
        },
    };
    let output = rows(row).output;
    let run_doc = match row {
        Some(row) => doc(&format!(
            "Runs the statement and its hooks in one transaction, and reads each row of the statement as {}.",
            docs::link(row, &[])
        )),
        None => doc(
            "Runs the statement and its hooks in one transaction, and returns the number of rows the statement affected.",
        ),
    };
    quote! {
        #query

        /// Binds the values to the SQL as a `sqlx` query that reads each row as `O`.
        pub fn query_as<'q, O, __I>(
            #borrowed,
        ) -> ::core::result::Result<#driver::QueryAs<'q, #dialect, O>, #error>
        where
            O: for<'r> #sqlx::FromRow<'r, #driver::Row<#dialect>>,
            #subject: #krate::Single<__I>,
        {
            #krate::query_as::<#subject, O, __I>(#statement)
        }

        /// Gives the values of a hook with parameters, as a value of the hook or its complete builder. Each hook takes its values once and runs once.
        pub fn with<__B, __H, __G, __I>(
            self,
            values: __B,
        ) -> #krate::With<#subject, (#krate::HookValues<__H>, ())>
        where
            #subject: #krate::Hooked<__G>,
            __B: #krate::Finish<Output = __H>,
            __H: #krate::Values,
            (#krate::HookValues<__H>, ()): #krate::Provides<__H, __I>,
        {
            #krate::With::new(#owned).with(values)
        }

        #run_doc
        pub async fn run<'c, __G, __I>(
            self,
            conn: #acquire,
        ) -> ::core::result::Result<#output, #error>
        where
            #subject: #krate::Hooked<__G> + #krate::HookNeeds<(), __I>,
        {
            #krate::With::new(#owned).run(conn).await
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
            #subject: #krate::Hooked<__G> + #krate::HookNeeds<(), __I>,
        {
            #krate::With::new(#owned).run_as(conn).await
        }
    }
}

// Returns the struct's methods and impls, and the methods of its complete builder.
pub(crate) fn transaction_methods(
    input: &ItemStruct,
    dialect: &Type,
    steps: &[Step],
) -> (TokenStream, TokenStream) {
    let krate = krate();
    let ident = &input.ident;
    let driver = quote!(#krate::driver);
    let conn = format_ident!("conn");
    let (runs, names, outputs) = run_steps(ident, steps, dialect, &conn, &mut (0, 0));
    let output = quote!((#(#outputs,)*));
    let methods = transaction_api(dialect, &output, &Receiver::Struct);
    let impls = quote! {
        #krate::__if_sqlx! {
            impl #ident {
                #methods
            }

            impl #krate::Run for #ident {
                type Output = #output;

                async fn run(
                    self,
                    conn: &mut #driver::Connection<#dialect>,
                ) -> ::core::result::Result<#output, #krate::sqlx::Error> {
                    #(#runs)*
                    ::core::result::Result::Ok((#(#names,)*))
                }
            }
        }
    };
    (
        impls,
        transaction_api(dialect, &output, &Receiver::Builder(ident)),
    )
}

fn transaction_api(dialect: &Type, output: &TokenStream, receiver: &Receiver) -> TokenStream {
    let krate = krate();
    let driver = quote!(#krate::driver);
    let sqlx = quote!(#krate::sqlx);
    let subject = receiver.subject();
    let owned = receiver.owned();
    quote! {
        /// Gives the values of a hook with parameters, as a value of the hook or its complete builder. Each hook takes its values once and runs once.
        pub fn with<__B, __H, __I>(
            self,
            values: __B,
        ) -> #krate::With<#subject, (#krate::HookValues<__H>, ())>
        where
            __B: #krate::Finish<Output = __H>,
            __H: #krate::Values,
            (#krate::HookValues<__H>, ()): #krate::Provides<__H, __I>,
        {
            #krate::With::new(#owned).with(values)
        }

        /// Runs the steps in order in one transaction, with each hook once around them, and returns the output of each step.
        pub async fn run<'c, __I>(
            self,
            conn: impl #sqlx::Acquire<'c, Database = #driver::Database<#dialect>>,
        ) -> ::core::result::Result<#output, #sqlx::Error>
        where
            #subject: #krate::HookNeeds<(), __I>,
        {
            #krate::With::new(#owned).run(conn).await
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
