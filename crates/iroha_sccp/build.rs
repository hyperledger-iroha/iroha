//! Captures exact compiler inputs for the signed SCCP validator-build identity.
use std::{env, process::Command};
fn canonical_build_value(value: &str) -> bool {
    !value.is_empty()
        && value.is_ascii()
        && value.len() <= 192
        && value
            .bytes()
            .all(|byte| byte == b' ' || (0x21..=0x7e).contains(&byte))
}
fn main() {
    println!("cargo:rerun-if-env-changed=RUSTC");
    let rustc = env::var_os("RUSTC").expect("Cargo must provide RUSTC to the SCCP build script");
    let output = Command::new(rustc)
        .arg("--version")
        .output()
        .expect("the selected Rust compiler must report its version");
    assert!(output.status.success(), "rustc --version must succeed");
    let rustc_version = String::from_utf8(output.stdout)
        .expect("rustc --version must be UTF-8")
        .trim()
        .to_owned();
    let target = env::var("TARGET").expect("Cargo must provide the exact target triple");
    let profile = env::var("PROFILE").expect("Cargo must provide the exact build profile");
    let mut features = env::vars()
        .filter_map(|(name, value)| {
            if value != "1" {
                return None;
            }
            name.strip_prefix("CARGO_FEATURE_")
                .filter(|name| *name != "DEFAULT")
                .map(str::to_owned)
        })
        .map(|name| name.to_ascii_lowercase().replace('_', "-"))
        .collect::<Vec<_>>();
    features.sort_unstable();
    features.dedup();
    let features = features.join(",");
    for (name, value) in [
        ("IROHA_SCCP_BUILD_TARGET", target),
        ("IROHA_SCCP_BUILD_PROFILE", profile),
        ("IROHA_SCCP_BUILD_FEATURES", features),
        ("IROHA_SCCP_RUSTC_VERSION", rustc_version),
    ] {
        let valid = if name == "IROHA_SCCP_BUILD_FEATURES" {
            value.is_empty() || canonical_build_value(&value)
        } else {
            canonical_build_value(&value)
        };
        assert!(valid, "{name} must be bounded canonical ASCII");
        println!("cargo:rustc-env={name}={value}");
    }
}
