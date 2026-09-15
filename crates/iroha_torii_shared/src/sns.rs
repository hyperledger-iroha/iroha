//! Typed absence response for authoritative SNS registration lookups.

use iroha_data_model::sns::{NameSelectorV1, SuffixId};
use norito::derive::{JsonDeserialize, JsonSerialize};

/// Exact discriminator for an absent registration, distinct from other HTTP 404 errors.
pub const SNS_REGISTRATION_NOT_FOUND_CODE: &str = "sns.registration_not_found";
/// Maximum size of a typed missing-registration response.
pub const SNS_REGISTRATION_NOT_FOUND_MAX_BYTES: usize = 4096;

/// Returned only when the canonical registration key is absent from ledger state.
///
/// This is a lookup result, not a cryptographic proof of ledger state.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SnsRegistrationNotFoundV1 {
    /// Must equal [`SNS_REGISTRATION_NOT_FOUND_CODE`].
    pub code: String,
    /// Fixed namespace of the missing registration.
    pub suffix_id: SuffixId,
    /// Canonical label of the missing registration.
    pub label: String,
}

impl SnsRegistrationNotFoundV1 {
    /// Construct the response from an authoritative missing-registration result.
    #[must_use]
    pub fn new(suffix_id: SuffixId, label: String) -> Self {
        Self {
            code: SNS_REGISTRATION_NOT_FOUND_CODE.to_owned(),
            suffix_id,
            label,
        }
    }

    /// Check the discriminator and exact canonical selector requested by a client.
    #[must_use]
    pub fn matches_selector(&self, selector: &NameSelectorV1) -> bool {
        self.code == SNS_REGISTRATION_NOT_FOUND_CODE
            && selector.version == NameSelectorV1::VERSION
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
        for invalid in [
            r#"{}"#,
            r#"{"code":"sns.registration_not_found","suffix_id":4099}"#,
            r#"{"code":"sns.registration_not_found","suffix_id":4099,"label":"dpn","extra":0}"#,
            r#"{"code":"sns.registration_not_found","suffix_id":4099,"label":"dpn","label":"other"}"#,
        ] {
            assert!(
                norito::json::from_str::<SnsRegistrationNotFoundV1>(invalid).is_err(),
                "{invalid}"
            );
        }
        for changed in [
            SnsRegistrationNotFoundV1 {
                code: "other".to_owned(),
                ..value.clone()
            },
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
