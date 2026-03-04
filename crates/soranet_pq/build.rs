//! Emits a cfg flag when this crate is built as the primary package so the FFI
//! artifacts can be generated without rebuilding downstream dependencies.

fn main() {
    println!("cargo:rustc-check-cfg=cfg(soranet_pq_primary_package)");
    if std::env::var_os("CARGO_PRIMARY_PACKAGE").is_some() {
        println!("cargo:rustc-cfg=soranet_pq_primary_package");
    }
}
