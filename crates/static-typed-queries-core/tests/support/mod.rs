macro_rules! node {
    ($name:literal, $fingerprint:literal, $kind:ident, $inject:expr, [$($part:expr),* $(,)?]) => {
        &static_typed_queries_core::node::Node {
            name: static_typed_queries_core::node::name::Name::new($name),
            fingerprint: static_typed_queries_core::node::fingerprint::Fingerprint($fingerprint),
            kind: static_typed_queries_core::node::kind::Kind::$kind,
            inject: $inject,
            parts: static_typed_queries_core::part::Parts(&[$($part),*]),
        }
    };
}

macro_rules! root {
    ($ty:ident: $dialect:ty = $node:expr) => {
        root!($ty: $dialect = $node, params = ());
    };
    ($ty:ident: $dialect:ty = $node:expr, params = $params:ty) => {
        pub struct $ty;

        impl static_typed_queries_core::sql::Sql for $ty {
            type Dialect = $dialect;
            type Params = $params;
            const NODE: &'static static_typed_queries_core::node::Node = $node;
        }

        static_typed_queries_core::impl_statement!($ty);
    };
}
