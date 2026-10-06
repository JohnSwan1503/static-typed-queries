mod finish;
mod items;
mod setters;
mod shape;

use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{Fields, GenericParam, Generics, Ident, ItemStruct};

use crate::emit::{krate, krate_path};
use crate::model::fields::{self, Field};
use crate::naming::{camel, snake_case, unique};

// The generated builder is `NameBuilder<T…, states…, __K>`: the struct's type parameters, one
// state per field, and the parent a scoped builder returns to. A value field's state is its marker
// around `Missing` or `Filled`; an item field's is `Open<builder>` or `Built<item>`.
pub(crate) struct Builder<'a> {
    input: &'a ItemStruct,
    fields: &'a [Field],
    krate: TokenStream,
    path: TokenStream,
    name: Ident,
    module: Ident,
    slots: Vec<Slot>,
}

// The method is named after the struct's field and the builder's field adds an underscore, so
// rustc never suggests the field for the method and no field meets `__parent` or `__state`. Both
// are spanned at the call site, so an unused builder isn't reported against the struct.
struct Slot {
    method: Ident,
    field: Ident,
    state: Ident,
    marker: Option<Ident>,
    camel: String,
}

// Returns the definitions, which must stay nameable, and the impls, which go in `emit::scoped`.
// `methods` go on the complete builder when a database feature is enabled.
pub(crate) fn builder(
    input: &ItemStruct,
    fields: &[Field],
    methods: TokenStream,
    facade: &TokenStream,
) -> (TokenStream, TokenStream) {
    if fields.is_empty() {
        return (TokenStream::new(), built(input));
    }
    Builder::new(input, fields, facade).emit(methods)
}

impl<'a> Builder<'a> {
    fn new(input: &'a ItemStruct, fields: &'a [Field], facade: &TokenStream) -> Self {
        let mut taken: Vec<String> = Vec::new();
        let slots = fields
            .iter()
            .map(|field| {
                let mut camel = camel(&field.ident);
                if syn::parse_str::<Ident>(&camel).is_err() {
                    camel.insert_str(0, "Field");
                }
                let camel = unique(camel, &taken);
                taken.push(camel.clone());
                let (state, marker) = if field.is_item() {
                    (format_ident!("Item{camel}"), None)
                } else {
                    (
                        format_ident!("Param{camel}"),
                        Some(format_ident!("{camel}")),
                    )
                };
                let mut method = field.ident.clone();
                method.set_span(Span::call_site());
                Slot {
                    field: format_ident!("{}_", field.ident, span = Span::call_site()),
                    method,
                    state,
                    marker,
                    camel,
                }
            })
            .collect();
        Builder {
            input,
            fields,
            krate: krate(),
            path: krate_path(facade),
            name: format_ident!("{}Builder", input.ident),
            module: format_ident!("__{}_builder", snake_case(&input.ident.to_string())),
            slots,
        }
    }

    fn emit(&self, methods: TokenStream) -> (TokenStream, TokenStream) {
        let mut definitions = self.markers();
        definitions.extend(self.definition());
        definitions.extend(self.holes());
        let mut impls = self.marker_impls();
        impls.extend(self.ready());
        impls.extend(self.scope());
        impls.extend(self.setters());
        impls.extend(self.item_methods());
        impls.extend(self.finish(methods));
        impls.extend(self.build());
        (definitions, impls)
    }

    fn args(&self) -> Vec<&Ident> {
        self.input
            .generics
            .type_params()
            .map(|param| &param.ident)
            .collect()
    }

    fn states(&self) -> Vec<TokenStream> {
        self.slots
            .iter()
            .map(|slot| slot.state.to_token_stream())
            .collect()
    }

    fn ty(&self, states: &[TokenStream], parent: &TokenStream) -> TokenStream {
        let name = &self.name;
        let args = self.args();
        quote!(#name<#(#args,)* #(#states,)* #parent>)
    }

    fn generics(&self, extra: impl IntoIterator<Item = TokenStream>) -> Generics {
        let mut generics = self.input.generics.clone();
        for param in extra {
            generics
                .params
                .push(syn::parse2::<GenericParam>(param).expect("a generic parameter"));
        }
        generics
    }

    // The states of every slot but `slot`, followed by `extra`.
    fn generics_except(&self, slot: usize, extra: &[TokenStream]) -> Generics {
        let states = self
            .slots
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != slot)
            .map(|(_, other)| other.state.to_token_stream());
        self.generics(states.chain(extra.iter().cloned()))
    }

    fn states_with(&self, slot: usize, state: TokenStream) -> Vec<TokenStream> {
        let mut states = self.states();
        states[slot] = state;
        states
    }

    fn marker(&self, slot: usize, state: TokenStream) -> TokenStream {
        let module = &self.module;
        let marker = self.slots[slot].marker.as_ref().expect("a value field");
        quote!(#module::#marker<#state>)
    }

    // A builder literal: each slot's field from `value(slot)`, then the parent.
    fn literal(&self, value: impl Fn(usize) -> TokenStream, parent: TokenStream) -> TokenStream {
        let name = &self.name;
        let fields = self.slots.iter().enumerate().map(|(i, slot)| {
            let field = &slot.field;
            let value = value(i);
            quote!(#field: #value)
        });
        quote! {
            #name {
                #(#fields,)*
                __parent: #parent,
                __state: ::core::marker::PhantomData,
            }
        }
    }

    fn constructor(&self) -> String {
        let args: Vec<String> = self.args().iter().map(ToString::to_string).collect();
        let ident = &self.input.ident;
        if args.is_empty() {
            format!("{ident}::builder()")
        } else {
            format!("{ident}::<{}>::builder()", args.join(", "))
        }
    }
}

// A struct that holds no values is built from the start.
fn built(input: &ItemStruct) -> TokenStream {
    let krate = krate();
    let ident = &input.ident;
    let value = match &input.fields {
        Fields::Unit => quote!(#ident),
        Fields::Unnamed(fields) => {
            let phantoms = fields
                .unnamed
                .iter()
                .map(|_| quote!(::core::marker::PhantomData));
            quote!(#ident(#(#phantoms),*))
        }
        Fields::Named(_) => {
            let phantoms = fields::phantoms(input);
            quote!(#ident { #(#phantoms: ::core::marker::PhantomData),* })
        }
    };
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    quote! {
        impl #impl_generics #krate::Build for #ident #ty_generics #where_clause {
            type Builder = #krate::Built<Self>;

            fn builder() -> Self::Builder {
                #krate::Built(#value)
            }
        }
    }
}
