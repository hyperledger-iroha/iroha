//! Secret-free runtime-effective configuration evidence for Kagemusha V4.

use crate::{
    ChainId,
    block::{
        BlockHeader,
        consensus_v2::{SumeragiV2GenesisContextParameters, finality::MAX_VALIDATOR_POP_BYTES},
    },
    peer::PeerId,
};
use iroha_crypto::{Algorithm, Hash, HashOf, PublicKey};
use iroha_primitives::addr::SocketAddr;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest, Sha256};

use super::kagemusha_promotion_receipt::{
    KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT, KagemushaPromotionReceiptValidationError,
};

/// Domain separating the consensus lock for one complete runtime projection.
pub const KAGEMUSHA_V4_RUNTIME_EFFECTIVE_CONFIG_SHA256_DOMAIN_V1: &[u8] =
    b"iroha.kagemusha.v4.runtime_effective_config.sha256.v1";

/// One effective public validator endpoint and its genesis-authenticated `PoP`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4RuntimeValidatorProjectionV1 {
    /// Canonical BLS validator identity.
    pub validator_id: PeerId,
    /// Effective public P2P address; a hostname must use canonical ASCII IDNA,
    /// must not use a canonical or legacy numeric IP spelling, and must not
    /// carry a trailing root dot.
    pub public_address: SocketAddr,
    /// Exact `PoP` retained by the frozen height-one context.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub bls_pop: Vec<u8>,
}

fn is_legacy_ipv4_numeric_spelling(host: &str) -> bool {
    let mut component_count = 0_usize;
    let all_numeric = host.split('.').all(|component| {
        component_count += 1;
        if let Some(hex) = component.strip_prefix("0x") {
            !hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
        } else {
            !component.is_empty() && component.bytes().all(|byte| byte.is_ascii_digit())
        }
    });
    all_numeric && (1..=4).contains(&component_count)
}

impl KagemushaV4RuntimeValidatorProjectionV1 {
    fn validate(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        let host_is_canonical = match &self.public_address {
            SocketAddr::Host(address) => {
                let host: &str = address.host.as_ref();
                let unbracketed_host = host
                    .strip_prefix('[')
                    .and_then(|host| host.strip_suffix(']'))
                    .unwrap_or(host);
                crate::name::canonicalize_domain_label(host)
                    .is_ok_and(|canonical| canonical == host)
                    && !host.ends_with('.')
                    && unbracketed_host.parse::<std::net::IpAddr>().is_err()
                    && !is_legacy_ipv4_numeric_spelling(host)
            }
            SocketAddr::Ipv4(_) | SocketAddr::Ipv6(_) => true,
        };
        if self.validator_id.public_key().try_algorithm() != Ok(Algorithm::BlsNormal)
            || self.public_address.port() == 0
            || !host_is_canonical
            || self.bls_pop.is_empty()
            || self.bls_pop.len() > MAX_VALIDATOR_POP_BYTES
            || iroha_crypto::bls_normal_pop_verify(self.validator_id.public_key(), &self.bls_pop)
                .is_err()
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "validator_qualification.runtime_effective_config.validators",
            ));
        }
        Ok(())
    }
}

/// Canonical projection derived after every startup configuration overlay.
///
/// The Sumeragi fingerprint commits the complete protocol-effective shared
/// config. The genesis context and `PoPs` are copied from authenticated signed
/// genesis or retained validator-qualification and snapshot authorities, not
/// reconstructed from caller assertions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4RuntimeEffectiveConfigProjectionV1 {
    /// Effective chain identifier.
    pub chain: ChainId,
    /// Effective I105 chain discriminant.
    pub chain_discriminant: u16,
    /// Whether this node is configured as a voting validator.
    pub is_validator: bool,
    /// Independently configured genesis verifier key.
    pub genesis_public_key: PublicKey,
    /// Independently configured exact genesis header hash.
    pub genesis_expected_hash: HashOf<BlockHeader>,
    /// Exact effective topology in strict genesis identity order.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_array"))]
    pub validators:
        [KagemushaV4RuntimeValidatorProjectionV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    /// Fingerprint of the complete post-overlay `SumeragiV2Config`.
    pub sumeragi_config_fingerprint: Hash,
    /// Signed and staged DA, Nexus/AMX, and execution-policy commitments.
    pub genesis_context: SumeragiV2GenesisContextParameters,
    /// Effective decoded Kagemusha verifier memory ceiling.
    pub kagemusha_max_decoded_bytes: u64,
}

impl KagemushaV4RuntimeEffectiveConfigProjectionV1 {
    /// Return the domain-separated SHA-256 committed by activation and consensus.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] when the projection
    /// is invalid or cannot be encoded canonically.
    pub fn consensus_sha256(&self) -> Result<[u8; 32], KagemushaPromotionReceiptValidationError> {
        self.validate()?;
        let canonical = norito::encode_canonical(self)
            .map_err(|_| KagemushaPromotionReceiptValidationError::ReceiptEncode)?;
        let mut hasher = Sha256::new();
        hasher.update(KAGEMUSHA_V4_RUNTIME_EFFECTIVE_CONFIG_SHA256_DOMAIN_V1);
        hasher.update(
            u64::try_from(canonical.len())
                .map_err(|_| KagemushaPromotionReceiptValidationError::ReceiptEncode)?
                .to_be_bytes(),
        );
        hasher.update(canonical);
        Ok(hasher.finalize().into())
    }

    /// Validate the bounded public projection and its cryptographic identities.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] for a non-validator,
    /// zero commitment, malformed `PoP`, non-canonical validator order, or a
    /// non-canonical or duplicate public endpoint.
    pub fn validate(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        let zero = Hash::prehashed([0; Hash::LENGTH]);
        if !self.is_validator
            || self.genesis_expected_hash == HashOf::from_untyped_unchecked(zero)
            || self.sumeragi_config_fingerprint == zero
            || self.kagemusha_max_decoded_bytes == 0
            || self.genesis_context.validate().is_err()
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "validator_qualification.runtime_effective_config",
            ));
        }
        let mut previous = None;
        let mut public_addresses = std::collections::BTreeSet::new();
        for validator in &self.validators {
            validator.validate()?;
            if previous.is_some_and(|id: &PeerId| id >= &validator.validator_id)
                || !public_addresses.insert(validator.public_address.to_literal())
            {
                return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                    "validator_qualification.runtime_effective_config.validators",
                ));
            }
            previous = Some(&validator.validator_id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::KeyPair;

    fn valid_projection() -> KagemushaV4RuntimeEffectiveConfigProjectionV1 {
        let mut keys = [0x91_u8, 0x92, 0x93, 0x94]
            .into_iter()
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let validators = keys
            .iter()
            .enumerate()
            .map(|(index, key)| KagemushaV4RuntimeValidatorProjectionV1 {
                validator_id: PeerId::new(key.public_key().clone()),
                public_address: format!("127.0.0.1:{}", 16_000 + index)
                    .parse()
                    .expect("fixture validator address"),
                bls_pop: iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture validator PoP"),
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("exactly four runtime validators");
        KagemushaV4RuntimeEffectiveConfigProjectionV1 {
            chain: ChainId::from("kagemusha-runtime-projection-test"),
            chain_discriminant: 42,
            is_validator: true,
            genesis_public_key: KeyPair::from_seed(vec![0x90; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
            genesis_expected_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"genesis header",
            )),
            validators,
            sumeragi_config_fingerprint: Hash::new(b"effective Sumeragi V2 config"),
            genesis_context: SumeragiV2GenesisContextParameters::recommended(),
            kagemusha_max_decoded_bytes: 64 * 1024 * 1024,
        }
    }

    #[test]
    fn runtime_projection_rejects_duplicate_public_validator_addresses() {
        let mut projection = valid_projection();
        projection.validate().expect("valid runtime projection");

        projection.validators[1].public_address = projection.validators[0].public_address.clone();

        assert!(projection.validate().is_err());
        assert!(projection.consensus_sha256().is_err());
    }

    #[test]
    fn runtime_projection_requires_canonical_host_addresses() {
        let mut projection = valid_projection();
        projection.validators[0].public_address = "validator.example:16000"
            .parse()
            .expect("canonical fixture host address");
        projection
            .validate()
            .expect("lower-case host address is canonical");

        projection.validators[0].public_address = "Validator.EXAMPLE:16000"
            .parse()
            .expect("mixed-case fixture host address");
        assert!(projection.validate().is_err());
        assert!(projection.consensus_sha256().is_err());

        projection.validators[0].public_address =
            SocketAddr::Host(iroha_primitives::addr::SocketAddrHost {
                host: "éxample.test".into(),
                port: 16_000,
            });
        assert!(
            projection.validate().is_err(),
            "Unicode host spelling must use its canonical ASCII IDNA form",
        );
    }

    #[test]
    fn runtime_projection_rejects_semantically_duplicate_address_variants() {
        let mut projection = valid_projection();
        projection.validators[1].public_address =
            SocketAddr::Host(iroha_primitives::addr::SocketAddrHost {
                host: "127.0.0.1".into(),
                port: 16_000,
            });

        assert!(projection.validate().is_err());
        assert!(projection.consensus_sha256().is_err());
    }

    #[test]
    fn runtime_projection_rejects_numeric_host_variants_without_a_duplicate() {
        let mut projection = valid_projection();
        for host in [
            "127.0.0.1",
            "127.1",
            "0177.0.0.1",
            "0x7f000001",
            "2130706433",
            "[::1]",
        ] {
            projection.validators[0].public_address =
                SocketAddr::Host(iroha_primitives::addr::SocketAddrHost {
                    host: host.into(),
                    port: 16_000,
                });
            assert!(
                projection.validate().is_err(),
                "numeric host spelling `{host}` must use the canonical IP variant",
            );
        }

        projection.validators[0].public_address =
            SocketAddr::Host(iroha_primitives::addr::SocketAddrHost {
                host: "123.example".into(),
                port: 16_000,
            });
        projection
            .validate()
            .expect("a hostname with one numeric label is not an IP spelling");
    }
}
