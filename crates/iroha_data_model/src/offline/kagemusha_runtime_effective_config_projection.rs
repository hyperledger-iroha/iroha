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

use super::kagemusha_promotion_receipt::{
    KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT, KagemushaPromotionReceiptValidationError,
};

/// One effective public validator endpoint and its genesis-authenticated PoP.
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
    /// Effective public P2P address.
    pub public_address: SocketAddr,
    /// Exact PoP retained by the frozen height-one context.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub bls_pop: Vec<u8>,
}

impl KagemushaV4RuntimeValidatorProjectionV1 {
    fn validate(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        if self.validator_id.public_key().try_algorithm() != Ok(Algorithm::BlsNormal)
            || self.public_address.port() == 0
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
/// config. The genesis context and PoPs are copied from the successfully
/// frozen height-one bootstrap, not reconstructed from caller assertions.
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
    /// Validate the bounded public projection and its cryptographic identities.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] for a non-validator,
    /// zero commitment, malformed PoP, or non-canonical validator order.
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
        for validator in &self.validators {
            validator.validate()?;
            if previous.is_some_and(|id: &PeerId| id >= &validator.validator_id) {
                return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                    "validator_qualification.runtime_effective_config.validators",
                ));
            }
            previous = Some(&validator.validator_id);
        }
        Ok(())
    }
}
