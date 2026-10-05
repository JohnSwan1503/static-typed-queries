use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident};
use syn::{Ident, ItemStruct, LitStr, Type};

use crate::args::Step;
use crate::emit::docs;
use crate::model::params::{Child, Group};
use crate::naming::snake_case;

pub(crate) enum Role<'a> {
    Table {
        before: &'a [Type],
        after: &'a [Type],
    },
    Query {
        sql: &'a LitStr,
    },
    Statement {
        target: &'a Type,
    },
    Wrapper,
    Transaction {
        steps: &'a [Step],
    },
}

pub(crate) struct Item<'a> {
    pub(crate) input: &'a ItemStruct,
    pub(crate) role: Role<'a>,
    pub(crate) params_ident: Ident,
    pub(crate) builder: Ident,
    pub(crate) groups: Vec<Group>,
    pub(crate) children: Vec<Child>,
}

impl<'a> Item<'a> {
    pub(crate) fn new(
        input: &'a ItemStruct,
        role: Role<'a>,
        groups: Vec<Group>,
        children: Vec<Child>,
    ) -> Item<'a> {
        Item {
            input,
            role,
            params_ident: format_ident!("{}Params", input.ident),
            builder: format_ident!("{}Builder", input.ident),
            groups,
            children,
        }
    }

    pub(crate) fn describe(&self, slot: u16) -> (String, String) {
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

    pub(crate) fn is_empty(&self) -> bool {
        self.groups.is_empty() && self.children.is_empty()
    }

    pub(crate) fn item_args(&self) -> Vec<&Ident> {
        self.input
            .generics
            .type_params()
            .map(|param| &param.ident)
            .collect()
    }

    pub(crate) fn constructor(&self) -> String {
        let args: Vec<String> = self.item_args().iter().map(ToString::to_string).collect();
        let ident = &self.input.ident;
        if args.is_empty() {
            format!("{ident}::builder()")
        } else {
            format!("{ident}::<{}>::builder()", args.join(", "))
        }
    }

    pub(crate) fn module(&self) -> Ident {
        format_ident!("__{}_builder", snake_case(&self.input.ident.to_string()))
    }

    pub(crate) fn link(&self, ty: &Type) -> String {
        docs::link(ty, &self.item_args())
    }

    pub(crate) fn states(&self) -> Vec<TokenStream> {
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

    pub(crate) fn runs_as_statement(&self) -> bool {
        match self.role {
            Role::Query { .. } => self.input.generics.params.is_empty(),
            Role::Statement { .. } => true,
            Role::Table { .. } | Role::Wrapper | Role::Transaction { .. } => false,
        }
    }

    pub(crate) fn steps(&self) -> Option<&'a [Step]> {
        match self.role {
            Role::Transaction { steps } => Some(steps),
            _ => None,
        }
    }

    pub(crate) fn fields(&self) -> Vec<&Ident> {
        self.groups
            .iter()
            .map(|group| &group.field)
            .chain(self.children.iter().map(|child| &child.field))
            .collect()
    }
}
