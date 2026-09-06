//! Public account-bootstrap policy projected from the exact network and admitted signing set.

use std::collections::BTreeSet;

use iroha_crypto::Algorithm;
use iroha_data_model::NetworkId;
use norito::derive::{JsonDeserialize, JsonSerialize};

/// Maximum JSON representation accepted by first-release bootstrap clients.
pub const ACCOUNT_CAPABILITIES_MAX_BYTES_V1: usize = 4 * 1024;
/// Explicit first-release account-bootstrap default, also required by control-plane admission.
/// This is protocol policy, not an ordering preference in the admission allow-list.
pub const ACCOUNT_DEFAULT_SIGNING_V1: &str = "ed25519";

/// Public, account-free projection for `GET /v1/accounts/capabilities`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct AccountCapabilitiesV1 {
    /// Exact bootstrap schema version.
    pub schema_version: u16,
    /// Genesis-derived signing identity; never a display chain label.
    pub network_id: NetworkId,
    /// Canonical I105 address network discriminant.
    pub network_prefix: u16,
    /// Unique canonical names from the current signing admission configuration.
    pub allowed_signing: Vec<String>,
    /// Explicit V1 bootstrap default, required to be admitted.
    pub default_signing: String,
}

impl AccountCapabilitiesV1 {
    /// Build the bounded projection; reject inconsistent admission instead of inventing a default.
    pub fn from_admission(
        network_id: NetworkId,
        network_prefix: u16,
        allowed_signing: &[Algorithm],
    ) -> Result<Self, &'static str> {
        if !allowed_signing.contains(&Algorithm::Ed25519) {
            return Err("account bootstrap requires admitted Ed25519 signing");
        }
        let allowed_signing = allowed_signing
            .iter()
            .map(|algorithm| algorithm.as_static_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(str::to_owned)
            .collect();
        Ok(Self {
            schema_version: 1,
            network_id,
            network_prefix,
            allowed_signing,
            default_signing: ACCOUNT_DEFAULT_SIGNING_V1.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Hash, HashOf};

    fn network_id() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"account capability exact genesis identity",
        )))
    }

    #[test]
    fn account_capabilities_preserve_identity_and_explicit_default_independent_of_order() {
        let admitted = [
            Algorithm::Secp256k1,
            Algorithm::Ed25519,
            Algorithm::Secp256k1,
        ];
        let response = AccountCapabilitiesV1::from_admission(network_id(), 369, &admitted)
            .expect("valid account bootstrap admission");
        assert_eq!(response.network_id, network_id());
        assert_eq!(response.network_prefix, 369);
        assert_eq!(response.default_signing, "ed25519");
        assert_eq!(response.allowed_signing, ["ed25519", "secp256k1"]);
        let json = norito::json::to_vec(&response).expect("encode account capabilities");
        assert!(json.len() < ACCOUNT_CAPABILITIES_MAX_BYTES_V1);
        let value: norito::json::Value = norito::json::from_slice(&json).expect("parse JSON");
        let object = value.as_object().expect("object");
        assert_eq!(object.len(), 5);
        assert_eq!(value["schema_version"].as_u64(), Some(1));
        assert_eq!(
            value["network_id"].as_str(),
            Some(network_id().to_string().as_str())
        );
        let decoded: AccountCapabilitiesV1 = norito::json::from_slice(&json).expect("decode DTO");
        assert_eq!(decoded, response);
    }

    #[test]
    fn account_capabilities_reject_a_missing_protocol_default() {
        for admitted in [&[][..], &[Algorithm::Secp256k1][..]] {
            assert!(
                AccountCapabilitiesV1::from_admission(network_id(), u16::MAX, admitted).is_err()
            );
        }
    }
}
