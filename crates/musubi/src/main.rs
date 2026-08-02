#![doc = "Musubi package-manager entrypoint for Kotodama source packages."]
#![deny(deprecated)]

fn main() {
    let exit_code = musubi::run();
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}
