extern crate version_check as rustc;

use std::env;

use rustc::Channel;

const NIGHTLY_FEATURES: &[(&str, &str)] = &[
    ("NIGHTLY_GENERIC_CONST_EXPRS", "stq_generic_const_exprs"),
    ("NIGHTLY_GENERIC_CONST_ITEMS", "stq_generic_const_items"),
    ("NIGHTLY_GENERIC_SQL", "stq_generic_sql"),
    ("NIGHTLY_STR_GENERICS", "stq_str_generics"),
    ("NIGHTLY_TRIVIAL_BOUNDS", "stq_trivial_bounds"),
];

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(stq_nightly)");
    for (_, cfg) in NIGHTLY_FEATURES {
        println!("cargo::rustc-check-cfg=cfg({cfg})");
    }

    if env::var_os("CARGO_FEATURE_NIGHTLY").is_none() {
        return;
    }
    if !Channel::read().is_some_and(|channel| channel.supports_features()) {
        println!(
            "cargo::error=the `nightly` features of static-typed-queries-core need a nightly \
             toolchain; build with `cargo +nightly` or turn them off"
        );
        return;
    }

    println!("cargo::rustc-cfg=stq_nightly");
    for (feature, cfg) in NIGHTLY_FEATURES {
        if env::var_os(format!("CARGO_FEATURE_{feature}")).is_some() {
            println!("cargo::rustc-cfg={cfg}");
        }
    }
}
