//! Small helpers shared by the SoraNet gateway and PoP evidence generators.

use blake3::Hasher as Blake3;
use eyre::{Result, WrapErr};
use std::{fs, path::Path};

/// Lower-case ASCII alphanumerics, map every other character to `-`, and trim edge dashes.
pub(crate) fn sanitize_label(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// Stream the file through BLAKE3 and return the lower-case hex digest.
pub(crate) fn file_blake3_hex(path: &Path) -> Result<String> {
    let mut hasher = Blake3::new();
    let mut file = fs::File::open(path).wrap_err_with(|| format!("open {}", path.display()))?;
    std::io::copy(&mut file, &mut hasher).wrap_err_with(|| format!("hash {}", path.display()))?;
    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_label_lowercases_and_trims_separators() {
        assert_eq!(sanitize_label("  SoraNet PoP #1 "), "soranet-pop--1");
        assert_eq!(sanitize_label("--"), "");
    }

    #[test]
    fn file_blake3_hex_matches_in_memory_digest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("payload.bin");
        fs::write(&path, b"soranet").expect("write payload");
        assert_eq!(
            file_blake3_hex(&path).expect("hash payload"),
            blake3::hash(b"soranet").to_hex().to_string()
        );
    }
}
