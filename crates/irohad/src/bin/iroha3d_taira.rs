//! Taira launcher with Soracloud signing at FD 198 and private mint-finality seed custody at FD 199.

#[cfg(unix)]
fn main() {
    irohad::taira_runtime_signer::main_entry();
}

#[cfg(not(unix))]
fn main() {
    eprintln!("the Taira runtime-signer launcher requires Unix descriptor custody");
    std::process::exit(2);
}
