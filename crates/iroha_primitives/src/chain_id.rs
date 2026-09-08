//! Canonical exact ASCII chain-label grammar shared by public protocol boundaries.

/// Maximum canonical chain label length in bytes.
pub const MAX_CHAIN_ID_BYTES: usize = 128;

/// Validate the sole first-release chain-label grammar without allocating or normalizing.
///
/// # Errors
/// Returns the stable reason when empty, oversized or noncanonical text is supplied.
pub fn validate_chain_id(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("`ChainId` must not be empty");
    }
    if value.len() > MAX_CHAIN_ID_BYTES {
        return Err("`ChainId` exceeds the 128-byte ASCII limit");
    }
    let bytes = value.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        || !bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        || bytes
            .iter()
            .any(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'.' | b'_' | b':' | b'-'))
    {
        return Err(
            "`ChainId` must be exact ASCII text beginning and ending with an alphanumeric byte and containing only alphanumerics, `.`, `_`, `:`, or `-`",
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_existing_chain_label_grammar_is_preserved() {
        for accepted in ["a", "CHAIN.a_b:c-1", &"a".repeat(MAX_CHAIN_ID_BYTES)] {
            assert!(validate_chain_id(accepted).is_ok());
        }
        for rejected in [
            "",
            "a b",
            "-chain",
            "chain_",
            "a/b",
            "é",
            "a\0b",
            &"a".repeat(MAX_CHAIN_ID_BYTES + 1),
        ] {
            assert!(validate_chain_id(rejected).is_err());
        }
    }
}
