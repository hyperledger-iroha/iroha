use super::Error;
const CHECKSUM_WIDTH: usize = 4;
const CRC_POLY: u16 = 0x1021;
const CRC_INIT: u16 = 0xFFFF;
fn crc16(tag: &str, body: &str) -> u16 {
    let mut crc = CRC_INIT;
    for byte in tag
        .as_bytes()
        .iter()
        .copied()
        .chain(Some(b':'))
        .chain(body.as_bytes().iter().copied())
    {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ CRC_POLY;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}
/// Format a canonical literal of the form `<tag>:<body>#<crc16>`.
pub fn format(tag: &str, body: &str) -> String {
    let checksum = crc16(tag, body);
    format!("{tag}:{body}#{checksum:04X}")
}
/// Parse a canonical literal `<tag>:<body>#<crc>` and return the body.
pub fn parse<'a>(tag: &str, candidate: &'a str) -> Result<&'a str, Error> {
    if !candidate.starts_with(tag) {
        return Err(Error::Message(format!(
            "literal `{candidate}` must start with `{tag}:`"
        )));
    }
    let Some(rest) = candidate.get(tag.len()..) else {
        return Err(Error::Message(format!(
            "literal `{candidate}` missing ':' separator after tag `{tag}`"
        )));
    };
    let Some(body_and_checksum) = rest.strip_prefix(':') else {
        return Err(Error::Message(format!(
            "literal `{candidate}` missing ':' separator after tag `{tag}`"
        )));
    };
    let Some(hash_pos) = body_and_checksum.rfind('#') else {
        return Err(Error::Message(format!(
            "literal `{candidate}` missing checksum delimiter '#'"
        )));
    };
    let body = &body_and_checksum[..hash_pos];
    let checksum_str = &body_and_checksum[hash_pos + 1..];
    if body.is_empty() {
        return Err(Error::Message(format!(
            "literal `{candidate}` has empty body"
        )));
    }
    if checksum_str.len() != CHECKSUM_WIDTH {
        return Err(Error::Message(format!(
            "literal `{candidate}` checksum must be {CHECKSUM_WIDTH} hex digits"
        )));
    }
    if checksum_str.bytes().any(|byte| byte.is_ascii_lowercase()) {
        return Err(Error::Message(format!(
            "literal `{candidate}` checksum must use uppercase hex digits"
        )));
    }
    let parsed_checksum = u16::from_str_radix(checksum_str, 16).map_err(|_| {
        Error::Message(format!(
            "literal `{candidate}` checksum `{checksum_str}` is not valid hex"
        ))
    })?;
    let expected = crc16(tag, body);
    if parsed_checksum != expected {
        return Err(Error::Message(format!(
            "literal `{candidate}` checksum mismatch (expected {expected:04X})"
        )));
    }
    Ok(body)
}

/// Parse a canonical literal without constructing source-sized diagnostics.
///
/// This is the allocation-free counterpart used by bounded decoders that map
/// malformed input to their own fixed error. It accepts exactly the same
/// canonical spelling as [`parse`].
#[doc(hidden)]
pub fn parse_without_diagnostics<'a>(tag: &str, candidate: &'a str) -> Option<&'a str> {
    let rest = candidate.strip_prefix(tag)?.strip_prefix(':')?;
    let hash_pos = rest.rfind('#')?;
    let body = &rest[..hash_pos];
    let checksum = &rest[hash_pos + 1..];
    if body.is_empty()
        || checksum.len() != CHECKSUM_WIDTH
        || checksum.bytes().any(|byte| byte.is_ascii_lowercase())
    {
        return None;
    }
    let parsed = u16::from_str_radix(checksum, 16).ok()?;
    (parsed == crc16(tag, body)).then_some(body)
}
#[cfg(test)]
mod tests {
    use super::{CHECKSUM_WIDTH, format, parse, parse_without_diagnostics};
    #[test]
    fn format_and_parse_roundtrip() {
        let literal = format("hash", "ABCDEF");
        let body = parse("hash", &literal).expect("parse literal");
        assert_eq!(body, "ABCDEF");
        assert_eq!(parse_without_diagnostics("hash", &literal), Some(body));
    }
    #[test]
    fn parse_rejects_missing_tag() {
        assert!(parse("hash", "deadbeef").is_err());
    }
    #[test]
    fn parse_rejects_bad_checksum() {
        let mut literal = format("hash", "ABCDEF");
        literal.truncate(literal.len() - 4);
        literal.push_str("0000");
        assert!(parse("hash", &literal).is_err());
        assert_eq!(parse_without_diagnostics("hash", &literal), None);
    }
    #[test]
    fn parse_rejects_lowercase_checksum() {
        let literal = format("hash", "ABCDEF");
        let checksum_start = literal.len() - CHECKSUM_WIDTH;
        let lowercase = std::format!(
            "{}{}",
            &literal[..checksum_start],
            literal[checksum_start..].to_ascii_lowercase()
        );
        assert_ne!(lowercase, literal, "fixture checksum must contain A-F");
        assert!(parse("hash", &lowercase).is_err());
    }
}
