//! Typed absence details within the canonical Torii error envelope.

use iroha_data_model::sns::{NameSelectorV1, SuffixId};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// Exact discriminator for an absent registration, distinct from other HTTP 404 errors.
pub const SNS_REGISTRATION_NOT_FOUND_CODE: &str = "sns_registration_not_found";
/// Maximum size of the complete typed missing-registration error envelope.
pub const SNS_REGISTRATION_NOT_FOUND_MAX_BYTES: usize = 4096;

/// Error details returned only when the canonical registration key is absent from ledger state.
///
/// This is a lookup result, not a cryptographic proof of ledger state.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoSerialize,
    NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_torii_shared::sns::SnsRegistrationNotFoundV1")]
#[norito(deny_unknown_fields)]
pub struct SnsRegistrationNotFoundV1 {
    /// Fixed namespace of the missing registration.
    pub suffix_id: SuffixId,
    /// Canonical label of the missing registration.
    pub label: String,
}

impl SnsRegistrationNotFoundV1 {
    /// Construct the response from an authoritative missing-registration result.
    #[must_use]
    pub fn new(suffix_id: SuffixId, label: String) -> Self {
        Self { suffix_id, label }
    }

    /// Check the exact canonical selector requested by a client.
    /// The enclosing error code and HTTP status must be checked separately.
    #[must_use]
    pub fn matches_selector(&self, selector: &NameSelectorV1) -> bool {
        selector.version == NameSelectorV1::VERSION
            && self.suffix_id == selector.suffix_id
            && self.label == selector.label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_registration_response_requires_exact_fields_and_selector() {
        let selector = NameSelectorV1::new(4099, "dpn").expect("selector");
        let value = SnsRegistrationNotFoundV1::new(selector.suffix_id, selector.label.clone());
        let json = norito::json::to_json(&value).expect("encode absence");
        assert_eq!(
            norito::json::from_str::<SnsRegistrationNotFoundV1>(&json).expect("decode"),
            value
        );
        assert!(value.matches_selector(&selector));
        assert!(!value.matches_selector(&NameSelectorV1 {
            version: 0,
            ..selector.clone()
        }));
        let envelope = crate::ErrorEnvelope::new(
            SNS_REGISTRATION_NOT_FOUND_CODE,
            "The requested SNS registration does not exist.",
        )
        .with_details(crate::ErrorDetails {
            sns_registration_not_found: Some(value.clone()),
            ..crate::ErrorDetails::default()
        });
        assert!(!envelope.details.as_ref().unwrap().is_empty());
        let json_wire = norito::json::to_vec(&envelope).expect("encode complete JSON envelope");
        let native_wire = norito::to_bytes(&envelope).expect("encode complete Norito envelope");
        for decoded in [
            norito::json::from_slice::<crate::ErrorEnvelope>(&json_wire)
                .expect("decode complete JSON envelope"),
            norito::decode_from_bytes::<crate::ErrorEnvelope>(&native_wire)
                .expect("decode complete Norito envelope"),
        ] {
            assert_eq!(decoded.code(), SNS_REGISTRATION_NOT_FOUND_CODE);
            assert_eq!(
                decoded.details.unwrap().sns_registration_not_found,
                Some(value.clone())
            );
        }
        assert!(json_wire.len() <= SNS_REGISTRATION_NOT_FOUND_MAX_BYTES);
        for invalid in [
            r#"{}"#,
            r#"{"suffix_id":4099}"#,
            r#"{"suffix_id":4099,"label":"dpn","extra":0}"#,
            r#"{"suffix_id":4099,"label":"dpn","label":"other"}"#,
            r#"{"code":"sns.registration_not_found","suffix_id":4099,"label":"dpn"}"#,
        ] {
            assert!(
                norito::json::from_str::<SnsRegistrationNotFoundV1>(invalid).is_err(),
                "{invalid}"
            );
        }
        for changed in [
            SnsRegistrationNotFoundV1 {
                suffix_id: 4097,
                ..value.clone()
            },
            SnsRegistrationNotFoundV1 {
                label: "other".to_owned(),
                ..value
            },
        ] {
            assert!(!changed.matches_selector(&selector));
        }
    }
}
