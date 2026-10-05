//! Canonical string-key codecs for persisted typed maps.
//!
//! The serialization owner supplies the key contract independently of any map engine.

use crate::json::{self, BoundedJsonError, JsonWriteSink};

/// Helper trait for converting storage keys to and from their JSON string representation.
pub trait JsonKeyCodec: Sized {
    /// Write the canonical JSON string representation of this key to `out`.
    fn encode_json_key(&self, out: &mut String);
    /// Write this same canonical quoted key through a checked sink.
    ///
    /// Unmigrated key codecs refuse before formatting or allocating any scratch.
    fn encode_json_key_to(&self, _out: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
        Err(BoundedJsonError::Unsupported)
    }
    /// Parse a key from a JSON string representation.
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error>;
}
const HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";
fn append_hex_upper(bytes: &[u8], out: &mut String) {
    out.reserve(bytes.len() * 2);
    for &byte in bytes {
        let hi = (byte >> 4) as usize;
        let lo = (byte & 0x0F) as usize;
        out.push(HEX_DIGITS[hi] as char);
        out.push(HEX_DIGITS[lo] as char);
    }
}
fn decode_hex_nibble(byte: u8) -> Result<u8, json::Error> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(json::Error::Message(format!(
            "invalid hex digit `{}`",
            byte as char
        ))),
    }
}
fn decode_hex_array<const N: usize>(encoded: &str) -> Result<[u8; N], json::Error> {
    if encoded.len() != N * 2 {
        return Err(json::Error::Message(format!(
            "expected {len} hex digits, got {}",
            encoded.len(),
            len = N * 2
        )));
    }
    let mut out = [0u8; N];
    let bytes = encoded.as_bytes();
    for (i, chunk) in bytes.chunks_exact(2).enumerate() {
        let hi = decode_hex_nibble(chunk[0])?;
        let lo = decode_hex_nibble(chunk[1])?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}
impl JsonKeyCodec for String {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(self, out);
    }
    fn encode_json_key_to(&self, out: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
        json::write_json_string_to(self, out)
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        Ok(encoded.to_owned())
    }
}
impl JsonKeyCodec for u64 {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
    fn encode_json_key_to(&self, out: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
        json::write_json_display_to(self, out)
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        encoded
            .parse::<u64>()
            .map_err(|err| json::Error::Message(format!("invalid map key `{encoded}`: {err}")))
    }
}
impl<const N: usize> JsonKeyCodec for [u8; N] {
    fn encode_json_key(&self, out: &mut String) {
        let mut buf = String::new();
        append_hex_upper(self, &mut buf);
        json::write_json_string(&buf, out);
    }
    fn encode_json_key_to(&self, out: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
        json::write_upper_hex_json_string_to(self, out)
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        decode_hex_array(encoded)
    }
}
/// Delimiter for tuple keys (chosen outside the ASCII printable range to avoid collisions).
const TUPLE_KEY_SEPARATOR: char = '\u{1f}';
impl<K: JsonKeyCodec> JsonKeyCodec for (K, u64, u64) {
    fn encode_json_key(&self, out: &mut String) {
        let mut first = String::new();
        self.0.encode_json_key(&mut first);
        // Keep the first key's JSON quoting: embedded separators are escaped,
        // and the two integer coordinates remain separate, exact components.
        let joined = format!(
            "{first}{TUPLE_KEY_SEPARATOR}{}{TUPLE_KEY_SEPARATOR}{}",
            self.1, self.2
        );
        json::write_json_string(&joined, out);
    }
    fn encode_json_key_to(&self, out: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
        out.push('"')?;
        self.0.encode_json_key_to(&mut EscapedKeySink(out))?;
        write_key_separator(out)?;
        json::visit_json_display_text(&self.1, |text| out.push_str(text))?;
        write_key_separator(out)?;
        json::visit_json_display_text(&self.2, |text| out.push_str(text))?;
        out.push('"')
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let mut parts = encoded.rsplitn(3, TUPLE_KEY_SEPARATOR);
        let invalid =
            || json::Error::Message("expected a quoted key and two u64 coordinates".into());
        let third = parts.next().ok_or_else(invalid)?;
        let second = parts.next().ok_or_else(invalid)?;
        let first = parts.next().ok_or_else(invalid)?;
        let first: String = json::from_str(first)?;
        Ok((
            K::decode_json_key(&first)?,
            u64::decode_json_key(second)?,
            u64::decode_json_key(third)?,
        ))
    }
}
impl JsonKeyCodec for (String, String) {
    fn encode_json_key(&self, out: &mut String) {
        let mut buf = String::with_capacity(self.0.len() + self.1.len() + 1);
        buf.push_str(&self.0);
        buf.push(TUPLE_KEY_SEPARATOR);
        buf.push_str(&self.1);
        json::write_json_string(&buf, out);
    }
    fn encode_json_key_to(&self, out: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
        out.push('"')?;
        write_key_content(&self.0, out)?;
        write_key_separator(out)?;
        write_key_content(&self.1, out)?;
        out.push('"')
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        encoded
            .split_once(TUPLE_KEY_SEPARATOR)
            .map(|(left, right)| (left.to_owned(), right.to_owned()))
            .ok_or_else(|| {
                json::Error::Message("expected contract tuple key to contain unit separator".into())
            })
    }
}
impl JsonKeyCodec for (String, String, String) {
    fn encode_json_key(&self, out: &mut String) {
        let mut buf = String::with_capacity(self.0.len() + self.1.len() + self.2.len() + 2);
        buf.push_str(&self.0);
        buf.push(TUPLE_KEY_SEPARATOR);
        buf.push_str(&self.1);
        buf.push(TUPLE_KEY_SEPARATOR);
        buf.push_str(&self.2);
        json::write_json_string(&buf, out);
    }
    fn encode_json_key_to(&self, out: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
        out.push('"')?;
        write_key_content(&self.0, out)?;
        write_key_separator(out)?;
        write_key_content(&self.1, out)?;
        write_key_separator(out)?;
        write_key_content(&self.2, out)?;
        out.push('"')
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let mut parts = encoded.splitn(3, TUPLE_KEY_SEPARATOR);
        let first = parts.next().ok_or_else(|| {
            json::Error::Message("expected triple tuple key to contain unit separator".into())
        })?;
        let second = parts.next().ok_or_else(|| {
            json::Error::Message("expected triple tuple key to contain unit separator".into())
        })?;
        let third = parts.next().ok_or_else(|| {
            json::Error::Message("expected triple tuple key to contain unit separator".into())
        })?;
        Ok((first.to_owned(), second.to_owned(), third.to_owned()))
    }
}
impl JsonKeyCodec for (String, u32) {
    fn encode_json_key(&self, out: &mut String) {
        let mut buf = String::with_capacity(self.0.len() + 11 + 1);
        buf.push_str(&self.0);
        buf.push(TUPLE_KEY_SEPARATOR);
        buf.push_str(&self.1.to_string());
        json::write_json_string(&buf, out);
    }
    fn encode_json_key_to(&self, out: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
        out.push('"')?;
        write_key_content(&self.0, out)?;
        write_key_separator(out)?;
        json::visit_json_display_text(&self.1, |text| out.push_str(text))?;
        out.push('"')
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let (left, right) = encoded.split_once(TUPLE_KEY_SEPARATOR).ok_or_else(|| {
            json::Error::Message("expected circuit tuple key to contain unit separator".into())
        })?;
        let version = right.parse::<u32>().map_err(|err| {
            json::Error::Message(format!("invalid circuit version `{right}`: {err}"))
        })?;
        Ok((left.to_owned(), version))
    }
}
impl JsonKeyCodec for (String, String, u16) {
    fn encode_json_key(&self, out: &mut String) {
        let mut buf = String::with_capacity(self.0.len() + self.1.len() + 6 + 2);
        buf.push_str(&self.0);
        buf.push(TUPLE_KEY_SEPARATOR);
        buf.push_str(&self.1);
        buf.push(TUPLE_KEY_SEPARATOR);
        buf.push_str(&self.2.to_string());
        json::write_json_string(&buf, out);
    }
    fn encode_json_key_to(&self, out: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
        out.push('"')?;
        write_key_content(&self.0, out)?;
        write_key_separator(out)?;
        write_key_content(&self.1, out)?;
        write_key_separator(out)?;
        json::visit_json_display_text(&self.2, |text| out.push_str(text))?;
        out.push('"')
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let mut parts = encoded.splitn(3, TUPLE_KEY_SEPARATOR);
        let first = parts.next().ok_or_else(|| {
            json::Error::Message(
                "expected inrou replica tuple key to contain unit separator".into(),
            )
        })?;
        let second = parts.next().ok_or_else(|| {
            json::Error::Message(
                "expected inrou replica tuple key to contain unit separator".into(),
            )
        })?;
        let third = parts.next().ok_or_else(|| {
            json::Error::Message(
                "expected inrou replica tuple key to contain unit separator".into(),
            )
        })?;
        let replica_slot = third.parse::<u16>().map_err(|err| {
            json::Error::Message(format!("invalid inrou replica slot `{third}`: {err}"))
        })?;
        Ok((first.to_owned(), second.to_owned(), replica_slot))
    }
}

// Use the sole string-content walker for each borrowed chunk. No quoted key text
// or composite tuple buffer is materialized during counting or filling.
fn write_key_content(text: &str, out: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
    super::bounded::write_json_string_content_to(text, out)
}
fn write_key_separator(out: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
    write_key_content("\u{1f}", out)
}
struct EscapedKeySink<'a>(&'a mut dyn JsonWriteSink);
impl JsonWriteSink for EscapedKeySink<'_> {
    fn push(&mut self, value: char) -> Result<(), BoundedJsonError> {
        let mut encoded = [0; 4];
        write_key_content(value.encode_utf8(&mut encoded), self.0)
    }
    fn push_str(&mut self, value: &str) -> Result<(), BoundedJsonError> {
        write_key_content(value, self.0)
    }
}

#[cfg(test)]
mod tests;
