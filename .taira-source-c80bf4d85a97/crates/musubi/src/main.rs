#![doc = "Musubi package-manager entrypoint for Kotodama source packages."]
#![deny(deprecated)]

fn main() {
    if let Err(error) = musubi::run() {
        if let Some(rendered) = musubi::rendered_diagnostics(&error) {
            eprintln!("{rendered}");
        } else {
            eprintln!("Error: {error:?}");
        }
        std::process::exit(1);
    }
}
