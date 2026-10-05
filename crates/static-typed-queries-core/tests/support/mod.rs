macro_rules! node {
    (
        $name:literal,
        $fingerprint:literal,
        $kind:ident,
        $inject:expr,
        [$($part:expr),* $(,)?]
        $(, items = [$($item:expr),* $(,)?])?
        $(, before = [$($before:expr),* $(,)?])?
        $(, after = [$($after:expr),* $(,)?])?
        $(,)?
    ) => {
        &static_typed_queries_core::node::Node {
            name: static_typed_queries_core::node::name::Name::new($name),
            fingerprint: static_typed_queries_core::node::fingerprint::Fingerprint($fingerprint),
            kind: static_typed_queries_core::node::kind::Kind::$kind,
            inject: $inject,
            parts: static_typed_queries_core::part::Parts(&[$($part),*]),
            before: static_typed_queries_core::node::hooks::Hooks(&[$($($before),*)?]),
            after: static_typed_queries_core::node::hooks::Hooks(&[$($($after),*)?]),
            items: static_typed_queries_core::node::items::Items(&[$($($item),*)?]),
        }
    };
}

macro_rules! root {
    ($ty:ident: $dialect:ty = $node:expr) => {
        pub struct $ty;

        impl static_typed_queries_core::sql::Sql for $ty {
            type Dialect = $dialect;
            const NODE: &'static static_typed_queries_core::node::Node = $node;
        }

        impl static_typed_queries_core::values::Valueless for $ty {}

        static_typed_queries_core::impl_statement!($ty);
    };
}
