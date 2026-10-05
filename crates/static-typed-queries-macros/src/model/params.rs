use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident};
use syn::{Ident, ItemStruct, LitStr, Type};

use crate::args::Step;
use crate::emit::docs::{self};
use crate::naming::{camel, field_name, short_name, snake_case, to_ident, type_key, unique};
use crate::sql::analyze::Analysis;
use crate::sql::template::{Segment, Template};

pub(crate) const RESERVED: &[&str] = &[
    "build", "builder", "finish", "query", "query_as", "run", "run_as", "with",
];

pub(crate) struct Group {
    pub(crate) name: Ident,
    pub(crate) field: Ident,
    pub(crate) ty: Type,
    pub(crate) slots: Vec<u16>,
    pub(crate) origins: Vec<String>,
    pub(crate) state: Ident,
    pub(crate) marker: Ident,
}

pub(crate) struct Child {
    pub(crate) name: Ident,
    pub(crate) field: Ident,
    pub(crate) ty: Type,
    pub(crate) named: Type,
    pub(crate) state: Ident,
}

pub(crate) struct Params<'a> {
    pub(crate) item: &'a ItemStruct,
    pub(crate) ident: Ident,
    pub(crate) builder: Ident,
    pub(crate) groups: Vec<Group>,
    pub(crate) children: Vec<Child>,
    pub(crate) synthetic: bool,
    pub(crate) runs: bool,
    pub(crate) steps: Option<&'a [Step]>,
}

impl<'a> Params<'a> {
    pub(crate) fn new(
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
            steps: None,
        })
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
        self.item
            .generics
            .type_params()
            .map(|param| &param.ident)
            .collect()
    }

    pub(crate) fn constructor(&self) -> String {
        let args: Vec<String> = self.item_args().iter().map(ToString::to_string).collect();
        let ident = &self.item.ident;
        if args.is_empty() {
            format!("{ident}::builder()")
        } else {
            format!("{ident}::<{}>::builder()", args.join(", "))
        }
    }

    pub(crate) fn module(&self) -> Ident {
        format_ident!("__{}_builder", snake_case(&self.item.ident.to_string()))
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

    pub(crate) fn statement(&self) -> bool {
        self.item.generics.params.is_empty() && self.runs
    }

    pub(crate) fn fields(&self) -> Vec<&Ident> {
        self.groups
            .iter()
            .map(|group| &group.field)
            .chain(self.children.iter().map(|child| &child.field))
            .collect()
    }
}

pub(crate) fn builder_field(name: &Ident) -> Ident {
    format_ident!("__{}", name.to_string().trim_start_matches("r#"))
}
