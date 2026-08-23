//! Taira daemon launcher with one runtime signer inherited at fixed FD 198.

#[cfg(unix)]
fn main() {
    irohad::taira_runtime_signer::main_entry();
}

#[cfg(not(unix))]
fn main() {
    eprintln!("the Taira runtime-signer launcher requires Unix descriptor custody");
    std::process::exit(2);
}
