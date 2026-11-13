//! Explicit consent for deferred execution (`specs/live_runtime_upgrades.md` §8.2).
//!
//! These records belong to a selected release addition. They never extend the frozen
//! multisig proposal or trigger-action layouts, and absence never manufactures consent.

use norito::codec::{Decode, Encode};

use crate::{account::AccountId, trigger::TriggerId};

use super::lifecycle::ExpectationV1;

/// Native service that captures and enforces deferred execution consent.
pub const SERVICE: &str = "deferred_consent";
/// Immutable semantics selected by [`SERVICE`].
pub const SERVICE_CLOSURE: &[u8] =
    b"iroha:runtime:deferred-consent:v1:exact-occurrence-capture-or-explicit-current-bindings";

/// The occurrence or trigger incarnation whose consent the kernel owns.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::deferred::DeferredConsentKeyV1")]
#[norito(tag = "kind", content = "content")]
pub enum DeferredConsentKeyV1 {
    /// A trigger ID; registration replaces its record only after the prior incarnation ends.
    Trigger(TriggerId),
    /// An immutable native multisig occurrence, scoped to its account.
    Multisig {
        /// Account whose quorum authorizes the occurrence.
        owner: AccountId,
        /// Exact occurrence identity already derived from the network, account and nonce.
        proposal: [u8; 32],
    },
}

impl norito::json::JsonKeyCodec for DeferredConsentKeyV1 {
    fn encode_json_key(&self, out: &mut String) {
        let bytes = norito::encode_canonical(self)
            .expect("a typed deferred-consent key canonically encodes");
        norito::json::write_json_string(&hex::encode_upper(bytes), out);
    }

    fn encode_json_key_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_canonical_json_hex_to(self, out)
    }

    fn decode_json_key(encoded: &str) -> Result<Self, norito::json::Error> {
        let invalid =
            || norito::json::Error::Message("invalid canonical deferred-consent key".into());
        if encoded.is_empty()
            || encoded.len() > 32_768
            || !encoded.len().is_multiple_of(2)
            || !encoded
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
        {
            return Err(invalid());
        }
        let bytes = hex::decode(encoded).map_err(|_| invalid())?;
        let value = norito::decode_canonical_with_limits::<Self>(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| invalid())?;
        if norito::encode_canonical(&value).map_err(|_| invalid())? != bytes {
            return Err(invalid());
        }
        Ok(value)
    }
}

/// The semantics explicitly consented to when the deferred work is registered.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::deferred::DeferredConsentV1")]
#[norito(tag = "kind", content = "content")]
pub enum DeferredConsentV1 {
    /// Default: every reached binding, profile and immutable dependency is captured.
    Captured(ExpectationV1),
    /// The registration explicitly follows the bindings in force when it executes.
    CurrentBindings,
}

/// Kernel-owned consent, separate from the immutable payload already stored by its owner.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::deferred::DeferredConsentRecordV1")]
pub struct DeferredConsentRecordV1 {
    /// Authority under which the deferred executable runs.
    pub authority: AccountId,
    /// Hash of the exact canonical executable or multisig instruction list.
    pub executable: [u8; 32],
    /// Captured semantics or explicit current-binding consent.
    pub consent: DeferredConsentV1,
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::json::JsonKeyCodec as _;

    #[test]
    fn consent_keys_roundtrip_canonically_and_refuse_ambiguous_json_keys() {
        let key = DeferredConsentKeyV1::Trigger("scheduled".parse().unwrap());
        let bytes = norito::encode_canonical(&key).unwrap();
        assert_eq!(
            norito::decode_canonical::<DeferredConsentKeyV1>(&bytes).unwrap(),
            key
        );
        let mut quoted = String::new();
        key.encode_json_key(&mut quoted);
        let inner: String = norito::json::from_str(&quoted).unwrap();
        assert_eq!(DeferredConsentKeyV1::decode_json_key(&inner).unwrap(), key);
        assert!(DeferredConsentKeyV1::decode_json_key(&inner.to_lowercase()).is_err());
        assert!(DeferredConsentKeyV1::decode_json_key(&(inner.clone() + "00")).is_err());
        assert!(DeferredConsentKeyV1::decode_json_key(" ").is_err());
    }

    #[test]
    fn consent_records_roundtrip_and_bounded_map_keys_preserve_exact_bytes() {
        struct KeyJson<'a>(&'a DeferredConsentKeyV1);
        impl norito::json::JsonSerialize for KeyJson<'_> {
            fn json_serialize(&self, out: &mut String) {
                self.0.encode_json_key(out);
            }
            fn json_serialize_to(
                &self,
                out: &mut dyn norito::json::JsonWriteSink,
            ) -> Result<(), norito::json::BoundedJsonError> {
                self.0.encode_json_key_to(out)
            }
        }
        let authority = AccountId::new(
            iroha_crypto::KeyPair::from_seed(vec![7; 32], iroha_crypto::Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let definition = super::super::definition::DefinitionId([1; 32]);
        for consent in [
            DeferredConsentV1::CurrentBindings,
            DeferredConsentV1::Captured(ExpectationV1 {
                bindings: Vec::new(),
                services_profile: definition,
                kernel: definition,
                commitment_profile: definition,
                definitions: vec![definition],
                exact_bindings: true,
            }),
        ] {
            let record = DeferredConsentRecordV1 {
                authority: authority.clone(),
                executable: [2; 32],
                consent,
            };
            let canonical = norito::encode_canonical(&record).unwrap();
            assert_eq!(
                norito::decode_canonical::<DeferredConsentRecordV1>(&canonical).unwrap(),
                record
            );
            let json = norito::json::to_json(&record).unwrap();
            assert_eq!(
                norito::json::to_json_bounded(&record, json.len()).unwrap(),
                json
            );
            assert_eq!(
                norito::json::from_str::<DeferredConsentRecordV1>(&json).unwrap(),
                record
            );
            assert!(matches!(
                norito::json::to_json_bounded(&record, json.len() - 1),
                Err(norito::json::BoundedJsonError::BodyTooLarge)
            ));
        }
        for key in [
            DeferredConsentKeyV1::Trigger("bounded".parse().unwrap()),
            DeferredConsentKeyV1::Multisig {
                owner: authority,
                proposal: [3; 32],
            },
        ] {
            let mut json = String::new();
            key.encode_json_key(&mut json);
            let inner: String = norito::json::from_str(&json).unwrap();
            assert_eq!(DeferredConsentKeyV1::decode_json_key(&inner).unwrap(), key);
            assert_eq!(
                norito::json::to_json_bounded(&KeyJson(&key), json.len()).unwrap(),
                json
            );
            assert!(matches!(
                norito::json::to_json_bounded(&KeyJson(&key), json.len() - 1),
                Err(norito::json::BoundedJsonError::BodyTooLarge)
            ));
        }
    }
}
