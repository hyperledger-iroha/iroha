#![doc = "Musubi package-manager entrypoint for Kotodama source packages."]
#![deny(deprecated)]

#[path = "../musubi.rs"]
mod musubi;

fn main() -> eyre::Result<()> {
    musubi::run()
}
