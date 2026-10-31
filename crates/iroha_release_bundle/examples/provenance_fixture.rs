//! Exercise the canonical publisher JSON emitter for cross-language fixture checks.
//!
//! Emitted fixture bytes are not independent release trust or provenance.

#[path = "../src/bin/iroha_release_bundle/provenance.rs"]
mod provenance;

fn main() -> std::io::Result<()> {
    use std::io::{Read as _, Write as _};
    let mut bytes = Vec::new();
    std::io::stdin()
        .take((provenance::MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > provenance::MAX_BYTES {
        return Err(std::io::Error::other("fixture input exceeds its bound"));
    }
    let value: provenance::Publisher = norito::json::from_slice(&bytes)
        .map_err(|_| std::io::Error::other("fixture input violates the exact emitter schema"))?;
    std::io::stdout().write_all(&value.encode()?)
}
