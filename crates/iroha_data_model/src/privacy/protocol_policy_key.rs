//! Canonical Model-owned key for committed optional protocol limits.

use super::PrivacyProtocolIdV1;
use norito::{Decode, Encode};

/// Canonical key of an optional committed per-protocol resource policy.
///
/// Its original wire identity is independent of the Rust crate that owns this type.
/// Moving the implementation must preserve the installed storage definition and key bytes.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core::privacy_state::PrivacyProtocolPolicyKeyV1")]
pub struct PrivacyProtocolPolicyKeyV1 {
    protocol_id: PrivacyProtocolIdV1,
}

impl PrivacyProtocolPolicyKeyV1 {
    /// Select exactly one canonical native protocol.
    #[must_use]
    pub const fn new(protocol_id: PrivacyProtocolIdV1) -> Self {
        Self { protocol_id }
    }

    /// Protocol identified by this storage key.
    #[must_use]
    pub const fn protocol_id(self) -> PrivacyProtocolIdV1 {
        self.protocol_id
    }

    fn canonical_json_key(self) -> String {
        let encoded = norito::encode_canonical(&self)
            .expect("fixed protocol policy key always canonically encodes");
        debug_assert!(encoded.len() <= MAX_POLICY_KEY_FRAME_BYTES);
        hex::encode_upper(encoded)
    }
}

// One fixed frame header plus one closed protocol enum, with ample framing reserve.
// The bound is checked before hex allocation or canonical decoding.
const MAX_POLICY_KEY_FRAME_BYTES: usize = 128;

impl norito::json::JsonKeyCodec for PrivacyProtocolPolicyKeyV1 {
    fn encode_json_key(&self, out: &mut String) {
        norito::json::write_json_string(&self.canonical_json_key(), out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, norito::json::Error> {
        let invalid =
            || norito::json::Error::Message("invalid canonical protocol policy key".into());
        if encoded.is_empty()
            || encoded.len() > 2 * MAX_POLICY_KEY_FRAME_BYTES
            || !encoded.len().is_multiple_of(2)
            || !encoded
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
        {
            return Err(invalid());
        }
        let bytes = hex::decode(encoded).map_err(|_| invalid())?;
        let decoded = norito::decode_canonical_with_limits::<Self>(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| invalid())?;
        let canonical = norito::encode_canonical(&decoded).map_err(|_| invalid())?;
        if canonical != bytes {
            return Err(invalid());
        }
        Ok(decoded)
    }
}

impl norito::json::JsonObjectKey for PrivacyProtocolPolicyKeyV1 {
    fn visit_json_key_text<E>(
        &self,
        mut visitor: impl FnMut(&str) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(&self.canonical_json_key())
    }
}

impl norito::json::JsonObjectKeyOwned for PrivacyProtocolPolicyKeyV1 {
    fn from_json_key_text(key: &str) -> Result<Self, norito::json::Error> {
        <Self as norito::json::JsonKeyCodec>::decode_json_key(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::json::JsonKeyCodec as _;
    use std::collections::BTreeMap;

    #[test]
    fn model_owner_preserves_original_policy_key_identity_and_frame() {
        // The original one-field key: canonical header, a four-byte enum field,
        // and the IVM private-note protocol's unchanged discriminant (10).
        const ORIGINAL_FRAME: &str = "4E5254300000B6E07A5A4519921C50F7C32D5AFE2A30000500000000000000903261DB39E620FD02040A000000";
        assert_eq!(
            norito::schema::identity::nominal_name::<PrivacyProtocolPolicyKeyV1>(),
            "iroha_core::privacy_state::PrivacyProtocolPolicyKeyV1"
        );
        let expected = hex::decode(ORIGINAL_FRAME).unwrap();
        let key = PrivacyProtocolPolicyKeyV1::new(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
        assert_eq!(norito::encode_canonical(&key).unwrap(), expected);
        assert_eq!(
            norito::decode_canonical::<PrivacyProtocolPolicyKeyV1>(&expected).unwrap(),
            key
        );
        assert_eq!(key.canonical_json_key(), ORIGINAL_FRAME);
        assert_eq!(
            PrivacyProtocolPolicyKeyV1::decode_json_key(ORIGINAL_FRAME).unwrap(),
            key
        );
    }

    #[test]
    fn policy_key_roundtrip_is_bounded_and_canonical() {
        let key = PrivacyProtocolPolicyKeyV1::new(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
        assert_eq!(
            key.protocol_id(),
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1
        );
        let mut encoded = String::new();
        key.encode_json_key(&mut encoded);
        let encoded: String = norito::json::from_str(&encoded).unwrap();
        assert!(encoded.len() <= 2 * MAX_POLICY_KEY_FRAME_BYTES);
        assert_eq!(
            PrivacyProtocolPolicyKeyV1::decode_json_key(&encoded).unwrap(),
            key
        );
        assert!(PrivacyProtocolPolicyKeyV1::decode_json_key(&encoded.to_lowercase()).is_err());
        assert!(PrivacyProtocolPolicyKeyV1::decode_json_key(&format!("{encoded}00")).is_err());
        assert!(
            PrivacyProtocolPolicyKeyV1::decode_json_key(
                &"0".repeat(2 * MAX_POLICY_KEY_FRAME_BYTES + 2)
            )
            .is_err()
        );
        assert!(PrivacyProtocolPolicyKeyV1::decode_json_key(&encoded[2..]).is_err());
        let original = BTreeMap::from([(key, 7_u64)]);
        let json = norito::json::to_vec(&original).unwrap();
        assert_eq!(
            norito::json::from_slice::<BTreeMap<PrivacyProtocolPolicyKeyV1, u64>>(&json).unwrap(),
            original
        );
        for invalid in [
            encoded.to_lowercase(),
            format!("{encoded}00"),
            encoded[1..].to_owned(),
            "0".repeat(2 * MAX_POLICY_KEY_FRAME_BYTES + 2),
        ] {
            let json = norito::json::to_vec(&BTreeMap::from([(invalid, 7_u64)])).unwrap();
            assert!(
                norito::json::from_slice::<BTreeMap<PrivacyProtocolPolicyKeyV1, u64>>(&json)
                    .is_err()
            );
        }
    }
}
