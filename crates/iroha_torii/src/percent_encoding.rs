//! Percent-encoding of URI components.
//!
//! [`encode`] escapes every byte except the RFC 3986 unreserved characters
//! (`A-Z`, `a-z`, `0-9`, `-`, `.`, `_`, `~`) as `%XX` with uppercase hex.
//! [`decode`] reverses `%XX` escapes with hex digits in either case, keeps `+`
//! and malformed `%` sequences as they are, and requires the result to be UTF-8.
//! Both borrow their input when nothing changes.

use std::{borrow::Cow, string::FromUtf8Error};

/// Whether `byte` stays unescaped in a component.
fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

/// Uppercase hex digit of a nibble.
fn hex_digit(nibble: u8) -> char {
    char::from(b"0123456789ABCDEF"[usize::from(nibble & 0x0f)])
}

/// Value of an ASCII hex digit in either case.
fn hex_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        _ => None,
    }
}

/// Percent-encode `text` as one URI component.
pub(crate) fn encode(text: &str) -> Cow<'_, str> {
    let bytes = text.as_bytes();
    let Some(first_escape) = bytes.iter().position(|byte| !is_unreserved(*byte)) else {
        return Cow::Borrowed(text);
    };
    let escapes = bytes[first_escape..]
        .iter()
        .filter(|byte| !is_unreserved(**byte))
        .count();
    let mut encoded = String::with_capacity(bytes.len() + 2 * escapes);
    encoded.push_str(&text[..first_escape]);
    for &byte in &bytes[first_escape..] {
        if is_unreserved(byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(hex_digit(byte >> 4));
            encoded.push(hex_digit(byte));
        }
    }
    Cow::Owned(encoded)
}

/// Decode `%XX` escapes in `text`.
///
/// # Errors
/// Returns the UTF-8 error when the decoded bytes are not valid UTF-8.
pub(crate) fn decode(text: &str) -> Result<Cow<'_, str>, FromUtf8Error> {
    let bytes = text.as_bytes();
    let Some(first_percent) = bytes.iter().position(|byte| *byte == b'%') else {
        return Ok(Cow::Borrowed(text));
    };
    let mut decoded = Vec::with_capacity(bytes.len());
    decoded.extend_from_slice(&bytes[..first_percent]);
    let mut index = first_percent;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte != b'%' {
            decoded.push(byte);
            index += 1;
            continue;
        }
        match bytes.get(index + 1..index + 3) {
            Some(&[high, low]) => match (hex_value(high), hex_value(low)) {
                (Some(high), Some(low)) => {
                    decoded.push((high << 4) | low);
                    index += 3;
                }
                // A hex digit followed by a non-hex byte stays literal; the
                // second byte is examined again, so it may start an escape.
                (Some(_), None) => {
                    decoded.extend_from_slice(&[b'%', high]);
                    index += 2;
                }
                (None, _) => {
                    decoded.push(b'%');
                    index += 1;
                }
            },
            // Fewer than two bytes follow: the rest is literal.
            _ => {
                decoded.extend_from_slice(&bytes[index..]);
                break;
            }
        }
    }
    String::from_utf8(decoded).map(Cow::Owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_escapes_all_but_unreserved_bytes() {
        assert!(matches!(
            encode("pure-ascii_1.2~"),
            Cow::Borrowed("pure-ascii_1.2~")
        ));
        assert!(matches!(encode(""), Cow::Borrowed("")));
        assert_eq!(encode("this that"), "this%20that");
        assert_eq!(encode("&a%b!c.d?e"), "%26a%25b%21c.d%3Fe");
        assert_eq!(encode("x\0x"), "x%00x");
        assert_eq!(encode("a/b+c:d"), "a%2Fb%2Bc%3Ad");
        assert_eq!(
            encode("\u{1f47e} Exterminate!"),
            "%F0%9F%91%BE%20Exterminate%21"
        );
        assert_eq!(encode("Say what\u{203d}"), "Say%20what%E2%80%BD");
    }

    #[test]
    fn decode_reverses_escapes_and_keeps_malformed_ones() {
        assert!(matches!(decode("hello"), Ok(Cow::Borrowed("hello"))));
        assert!(matches!(decode("hello%20"), Ok(Cow::Owned(text)) if text == "hello "));
        assert_eq!(
            decode("%F0%9F%91%BE%20Exterminate%21").unwrap(),
            "\u{1f47e} Exterminate!"
        );
        assert_eq!(decode("a+b%2Bc%2bd").unwrap(), "a+b+c+d");
        for (raw, expected) in [
            ("this%20that%", "this that%"),
            ("this%20that%2", "this that%2"),
            ("this%20that%%", "this that%%"),
            ("this%20that%2%", "this that%2%"),
            ("this%2that", "this%2that"),
            ("this%%2that", "this%%2that"),
            ("this%2x%26that", "this%2x&that"),
            ("this%2%26that", "this%2&that"),
            ("%25%s%1G", "%%s%1G"),
        ] {
            assert_eq!(decode(raw).unwrap(), expected, "{raw}");
        }
        let error = decode("%F0%0F%91%BE").expect_err("invalid UTF-8");
        assert_eq!(error.into_bytes(), [0xF0, 0x0F, 0x91, 0xBE]);
    }

    #[test]
    fn hex_helpers_cover_both_cases() {
        assert_eq!(hex_value(b'3'), Some(3));
        assert_eq!(hex_value(b'a'), Some(10));
        assert_eq!(hex_value(b'F'), Some(15));
        assert_eq!(hex_value(b'G'), None);
        assert_eq!(hex_value(9), None);
        assert_eq!(hex_digit(0x0a), 'A');
        assert_eq!(hex_digit(0xf3), '3');
        assert!(is_unreserved(b'~') && is_unreserved(b'Z') && !is_unreserved(b'+'));
    }
}
