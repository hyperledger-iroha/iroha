//! Opaque keyset cursors.
//!
//! A cursor is `base64url(version | collection tag | query digest[16] |
//! canonical JSON array of the last row's sort values)`. The digest binds the
//! collection path (account or asset definition), filter, sort and aggregate
//! so a cursor cannot be replayed against another query; positions are plain keyset values, so cursors never expire and need
//! no server state. A forged position only moves the caller within rows the
//! caller is already allowed to see.
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use iroha_torii_shared::list_query::CURSOR_MAX_BYTES;
use norito::json::{self, Value};

const VERSION: u8 = 1;
const HEADER_BYTES: usize = 2 + DIGEST_BYTES;
/// Bytes of the query digest embedded in each cursor.
pub(super) const DIGEST_BYTES: usize = 16;

/// Why a cursor was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CursorError {
    /// Not a cursor produced by this server.
    Malformed,
    /// Produced for another collection.
    OtherCollection,
    /// Produced for another path, filter, sort or aggregate.
    OtherQuery,
}

impl CursorError {
    pub(super) const fn message(self) -> &'static str {
        match self {
            Self::Malformed => "`cursor` is not a `next_cursor` value returned by this endpoint",
            Self::OtherCollection => "`cursor` was issued by a different collection",
            Self::OtherQuery => {
                "`cursor` was issued for a different path, filter, sort or aggregate; repeat the original query or start without a cursor"
            }
        }
    }
}

/// Digest of the order-defining parts of a query.
pub(super) fn digest(parts: &[&str]) -> [u8; DIGEST_BYTES] {
    let mut material = Vec::new();
    for part in parts {
        material.extend_from_slice(&(part.len() as u64).to_be_bytes());
        material.extend_from_slice(part.as_bytes());
    }
    let hash = iroha_crypto::Hash::new(&material);
    let mut out = [0u8; DIGEST_BYTES];
    out.copy_from_slice(&hash.as_ref()[..DIGEST_BYTES]);
    out
}

/// Encode the keyset position after a row.
pub(super) fn encode(tag: u8, digest: &[u8; DIGEST_BYTES], key: &[Value]) -> String {
    let key_json = json::to_json(&Value::Array(key.to_vec())).expect("JSON values serialize");
    let mut frame = Vec::with_capacity(HEADER_BYTES + key_json.len());
    frame.push(VERSION);
    frame.push(tag);
    frame.extend_from_slice(digest);
    frame.extend_from_slice(key_json.as_bytes());
    URL_SAFE_NO_PAD.encode(frame)
}

/// Decode a keyset position, checking that it belongs to this query.
pub(super) fn decode(
    cursor: &str,
    tag: u8,
    digest: &[u8; DIGEST_BYTES],
    arity: usize,
) -> Result<Vec<Value>, CursorError> {
    if cursor.is_empty() || cursor.len() > CURSOR_MAX_BYTES {
        return Err(CursorError::Malformed);
    }
    let frame = URL_SAFE_NO_PAD
        .decode(cursor.as_bytes())
        .map_err(|_| CursorError::Malformed)?;
    if frame.len() < HEADER_BYTES || frame[0] != VERSION {
        return Err(CursorError::Malformed);
    }
    if frame[1] != tag {
        return Err(CursorError::OtherCollection);
    }
    if frame[2..HEADER_BYTES] != digest[..] {
        return Err(CursorError::OtherQuery);
    }
    let key_json =
        std::str::from_utf8(&frame[HEADER_BYTES..]).map_err(|_| CursorError::Malformed)?;
    match json::parse_value(key_json) {
        Ok(Value::Array(values)) if values.len() == arity => Ok(values),
        _ => Err(CursorError::Malformed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_binding() {
        let digest = digest(&["domains", "id = 1", ""]);
        let key = vec![Value::from("wonderland"), Value::from(7u64)];
        let cursor = encode(1, &digest, &key);
        assert!(
            cursor
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        );
        assert_eq!(decode(&cursor, 1, &digest, 2).expect("decode"), key);
        assert_eq!(
            decode(&cursor, 2, &digest, 2),
            Err(CursorError::OtherCollection)
        );
        let other = super::digest(&["domains", "id = 2", ""]);
        assert_eq!(decode(&cursor, 1, &other, 2), Err(CursorError::OtherQuery));
        assert_eq!(decode(&cursor, 1, &digest, 3), Err(CursorError::Malformed));
        assert_eq!(
            decode("not-a-cursor", 1, &digest, 2),
            Err(CursorError::Malformed)
        );
        assert_eq!(decode("", 1, &digest, 2), Err(CursorError::Malformed));
    }

    #[test]
    fn digest_separates_parts() {
        assert_ne!(digest(&["ab", "c"]), digest(&["a", "bc"]));
    }
}
