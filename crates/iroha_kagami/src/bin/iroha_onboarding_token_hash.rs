//! Derive the BLAKE3 digest used by Torii's signer-backed onboarding token gate.
//!
//! The raw token is read only from standard input and is never echoed. A single trailing LF or
//! CRLF from a private token file is ignored; every other byte is authenticated exactly.
use std::io::{self, Read as _};
const MIN_TOKEN_BYTES: usize = 32;
const MAX_TOKEN_BYTES: usize = 256;
const MAX_STDIN_BYTES: u64 = (MAX_TOKEN_BYTES + 3) as u64;
fn canonical_token_bytes(input: &[u8]) -> Result<&[u8], &'static str> {
    let token = input
        .strip_suffix(b"\r\n")
        .or_else(|| input.strip_suffix(b"\n"))
        .unwrap_or(input);
    if !(MIN_TOKEN_BYTES..=MAX_TOKEN_BYTES).contains(&token.len()) {
        return Err("onboarding token must contain 32 through 256 bytes");
    }
    if !token
        .iter()
        .all(|byte| byte.is_ascii_graphic() && !byte.is_ascii_whitespace())
    {
        return Err("onboarding token must contain only non-whitespace printable ASCII bytes");
    }
    Ok(token)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = Vec::new();
    io::stdin().take(MAX_STDIN_BYTES).read_to_end(&mut input)?;
    if input.len() > MAX_TOKEN_BYTES + 2 {
        return Err(io::Error::other("onboarding token input is too long").into());
    }
    let token = canonical_token_bytes(&input).map_err(io::Error::other)?;
    println!("{}", blake3::hash(token).to_hex());
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strips_one_file_line_ending_but_no_other_bytes() {
        let token = b"0123456789abcdef0123456789abcdef";
        let mut lf = token.to_vec();
        lf.push(b'\n');
        let mut crlf = token.to_vec();
        crlf.extend_from_slice(b"\r\n");
        assert_eq!(canonical_token_bytes(token), Ok(token.as_slice()));
        assert_eq!(canonical_token_bytes(&lf), Ok(token.as_slice()));
        assert_eq!(canonical_token_bytes(&crlf), Ok(token.as_slice()));
    }
    #[test]
    fn rejects_short_multiline_and_whitespace_tokens() {
        assert!(canonical_token_bytes(b"too-short").is_err());
        assert!(canonical_token_bytes(b"0123456789abcdef\n0123456789abcdef").is_err());
        assert!(canonical_token_bytes(b" 0123456789abcdef0123456789abcdef").is_err());
    }
}
