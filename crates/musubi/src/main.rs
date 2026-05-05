#![doc = "Musubi package-manager entrypoint for Kotodama source packages."]
#![deny(deprecated)]

fn main() -> eyre::Result<()> {
    musubi::run()
}
