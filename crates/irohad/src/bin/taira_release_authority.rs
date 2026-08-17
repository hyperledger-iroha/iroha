//! Native command-line client and server for the eight Taira release authorities.
//! All authority socket traffic uses canonical Norito `IRTAUT01` frames.

#[cfg(not(unix))]
fn main() {
    eprintln!("Taira release authority requires authenticated Unix peer credentials");
    std::process::exit(2);
}

#[cfg(unix)]
fn main() {
    if let Err(message) = irohad::external_software_signer::taira_authority::run_cli() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}
