pub fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    nightly();
    beta();
    stable();
}

#[rustversion::nightly]
fn nightly() {
    println!("cargo:rustc-check-cfg=cfg(nightly)");
    println!("cargo:rustc-cfg=nightly");
}

#[rustversion::not(nightly)]
fn nightly() {
    println!("cargo:rustc-check-cfg=cfg(nightly)");
}

#[rustversion::beta]
fn beta() {
    println!("cargo:rustc-check-cfg=cfg(beta)");
    println!("cargo:rustc-cfg=beta");
}

#[rustversion::not(beta)]
fn beta() {
    println!("cargo:rustc-check-cfg=cfg(beta)");
}

#[rustversion::stable]
fn stable() {
    println!("cargo:rustc-check-cfg=cfg(stable)");
    println!("cargo:rustc-cfg=stable");
}

#[rustversion::not(stable)]
fn stable() {
    println!("cargo:rustc-check-cfg=cfg(stable)");
}
