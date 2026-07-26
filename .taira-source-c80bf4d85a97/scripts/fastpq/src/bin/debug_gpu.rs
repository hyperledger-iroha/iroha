use std::env;

fn main() {
    // Print all FASTPQ/Metal-related env vars.
    for (key, value) in env::vars() {
        if key.starts_with("FASTPQ_") || key.starts_with("IVM_") {
            println!("{key}={value}");
        }
    }
}
