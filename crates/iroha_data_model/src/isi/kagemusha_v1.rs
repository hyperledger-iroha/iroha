//! Chain-facing instructions and operation records for Kagemusha V1.
//!
//! Top-up settlement is deliberately split at the finality boundary. The
//! accepted instruction fixes every issuance/output identifier and every byte
//! needed to reproduce the terminal mint credit, while atomically debiting the
//! online account and crediting the pooled reserve. Finality later attaches a
//! proof of that already-committed receipt and the exact mint credit; attaching
//! it is idempotent and never changes reserve totals. Redemption is one phase
//! because its hardware-bound terminal voucher exists before chain execution.

use super::*;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    block::consensus_v2::{
        HeightContextId, MAX_VALIDATORS_PER_HEIGHT, finality::V2FinalityArtifact,
        is_valid_committee_size,
    },
    nexus::AxtAssetIncarnationV1,
    kagemusha::{
        KAGEMUSHA_ASSET_SCALE_MAX_V1, KAGEMUSHA_WIRE_VERSION_V1,
        KagemushaEncryptedCreditEnvelopeV1, KagemushaHardwareCredentialV1,
        KagemushaHardwareProfileV1, KagemushaMintAuthorizationContextV1,
        KagemushaMintAuthorizationStatementV1, KagemushaMintAuthorizationV1,
        KagemushaMintCreditStatementV1, KagemushaMintCreditV1,
        KagemushaPastaStateCommitmentV1, KagemushaRedemptionVoucherV1,
        kagemusha_ciphertext_digest_v1, kagemusha_liability_pool_id_v1,
    },
    peer::PeerId,
};
use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use std::{string::String, vec::Vec};

/// Sole chain-facing Kagemusha instruction and operation layout version.
pub const KAGEMUSHA_CHAIN_VERSION_V1: u16 = 1;
/// Stable schema name for the canonical top-up request body.
pub const KAGEMUSHA_TOP_UP_REQUEST_SCHEMA_NAME_V1: &str =
    "iroha.torii.v1.kagemusha.top_up.request";
/// Stable schema name for the canonical redemption request body.
pub const KAGEMUSHA_REDEMPTION_REQUEST_SCHEMA_NAME_V1: &str =
    "iroha.torii.v1.kagemusha.redeem.request";
/// Exact number of siblings in a proof against the ordinary-write sparse tree.
pub const KAGEMUSHA_RESERVE_RECEIPT_WITNESS_SIBLINGS_V1: usize = 256;
/// Reserved ordinary-write key tag for a finalized Kagemusha operation.
pub const KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_TAG_V1: u8 = 0xD5;
/// Exact tagged key length: one tag byte followed by the operation identifier.
pub const KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_BYTES_V1: usize = 33;

const TOP_UP_ISSUANCE_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:top-up-issuance";
const TOP_UP_REQUEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:top-up-request";
const REDEMPTION_REQUEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:redemption-request";
const RESERVE_RECEIPT_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:reserve-receipt";
const MINT_FINALITY_SEAL_MESSAGE_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:mint-finality-seal-message";
const MINT_FINALITY_EPOCH_ROSTER_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:mint-finality-epoch-roster";
const MINT_FINALITY_PEER_ID_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:mint-finality-peer-id";
/// Domain for the marked SHA-256 bridge from paired Poseidon roots to `ExecutionCommitment`.
pub const KAGEMUSHA_MINT_FINALITY_ROOT_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:mint-finality-root";

/// Largest exact `2f + 1` mint-finality seal set admitted by the consensus roster bound.
pub const KAGEMUSHA_MINT_FINALITY_MAX_SEALS_V1: usize = (MAX_VALIDATORS_PER_HEIGHT * 2) / 3 + 1;
/// Depth of the sparse block-local top-up commitment tree.
///
/// The 32-bit index space removes any special Kagemusha admission maximum; ordinary bounded
/// block bytes remain the practical throughput limit.
pub const KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1: usize = 32;

/// Separately provisioned paired-Pasta public keys for one consensus validator.
///
/// These keys are intentionally not derived from the validator's BLS public
/// key.  The epoch roster is mandatory authority which must match the frozen
/// consensus roster exactly.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaMintFinalityValidatorKeysV1 {
    /// Exact consensus identity occupying this roster position.
    pub validator: PeerId,
    /// Canonical compressed Pallas key verified by the Eq/Fp helper.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_proof_public_key: [u8; 32],
    /// Canonical compressed Vesta key verified by the Ep/Fq helper.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep_proof_public_key: [u8; 32],
}

/// Exact epoch-scoped Kagemusha mint-finality authority.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaMintFinalityEpochRosterV1 {
    /// Sole first-release layout version.
    pub version: u16,
    /// Network whose frozen consensus epochs may use these keys.
    pub network_id: NetworkId,
    /// Exact Sumeragi election epoch.
    pub epoch: u64,
    /// Strictly ordered exact `3f + 1` consensus/Pasta key roster.
    pub validators: Vec<KagemushaMintFinalityValidatorKeysV1>,
}

impl KagemushaMintFinalityEpochRosterV1 {
    /// Validate exact committee geometry, identity order, and key uniqueness.
    ///
    /// Canonical curve-point decoding is additionally performed by Core before
    /// the roster is admitted for signing or verification.
    ///
    /// # Errors
    ///
    /// Returns an error for a wrong version/network, invalid committee, repeated
    /// identity or key, or a zero public-key encoding.
    pub fn validate(&self) -> Result<(), KagemushaIsiValidationErrorV1> {
        require_chain_version(self.version)?;
        if self.network_id.as_bytes() == &[0; 32]
            || !is_valid_committee_size(self.validators.len())
            || self.validators.len() > MAX_VALIDATORS_PER_HEIGHT
            || self
                .validators
                .windows(2)
                .any(|pair| pair[0].validator >= pair[1].validator)
        {
            return Err(invalid("mint_finality.epoch_roster"));
        }
        for (index, validator) in self.validators.iter().enumerate() {
            if validator.eq_proof_public_key == [0; 32]
                || validator.ep_proof_public_key == [0; 32]
                || self.validators[..index].iter().any(|prior| {
                    prior.eq_proof_public_key == validator.eq_proof_public_key
                        || prior.ep_proof_public_key == validator.ep_proof_public_key
                })
            {
                return Err(invalid("mint_finality.epoch_roster.keys"));
            }
        }
        Ok(())
    }

    /// Derive the authority identifier signed in every block-level seal.
    ///
    /// # Errors
    ///
    /// The digest uses a circuit-reproducible fixed-width transcript: header fields followed by
    /// all 31 validator slots. Active slots contain an independently domain-separated canonical
    /// peer-ID digest and both Pasta keys; inactive slots are an all-zero payload. This avoids
    /// asking the recursive circuit to reproduce variable-width Norito framing while still
    /// committing the exact consensus identities, key order, and committee size.
    ///
    /// Returns an error unless the complete roster is valid and each peer identity is canonically
    /// encodable.
    pub fn finality_epoch_id(&self) -> Result<[u8; 32], KagemushaIsiValidationErrorV1> {
        self.validate()?;
        let mut hasher = Sha256::new();
        hasher.update(MINT_FINALITY_EPOCH_ROSTER_DOMAIN_V1);
        hasher.update([0]);
        hasher.update(self.version.to_le_bytes());
        hasher.update(self.network_id.as_bytes());
        hasher.update(self.epoch.to_le_bytes());
        hasher.update(
            u32::try_from(self.validators.len())
                .map_err(|_| invalid("mint_finality.epoch_roster"))?
                .to_le_bytes(),
        );
        for slot in 0..MAX_VALIDATORS_PER_HEIGHT {
            if let Some(validator) = self.validators.get(slot) {
                hasher.update([1]);
                hasher.update(kagemusha_mint_finality_peer_id_digest_v1(
                    &validator.validator,
                )?);
                hasher.update(validator.eq_proof_public_key);
                hasher.update(validator.ep_proof_public_key);
            } else {
                hasher.update([0]);
                hasher.update([0; 96]);
            }
        }
        Ok(hasher.finalize().into())
    }
}

/// Return the fixed-width identity used for a consensus peer in the mint-finality roster.
///
/// # Errors
///
/// Returns an error only when canonical Norito encoding of the peer identifier fails.
pub fn kagemusha_mint_finality_peer_id_digest_v1(
    peer_id: &PeerId,
) -> Result<[u8; 32], KagemushaIsiValidationErrorV1> {
    let bytes = norito::encode_canonical(peer_id)
        .map_err(|error| KagemushaIsiValidationErrorV1::Encoding(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(MINT_FINALITY_PEER_ID_DOMAIN_V1);
    hasher.update([0]);
    hasher.update(
        u64::try_from(bytes.len())
            .map_err(|_| invalid("mint_finality.epoch_roster.peer_id"))?
            .to_le_bytes(),
    );
    hasher.update(bytes);
    Ok(hasher.finalize().into())
}

/// Bridge paired field-native top-up roots into the existing consensus `Hash` field.
///
/// The final marker operation is exactly [`Hash::prehashed`], so the helper circuit can reproduce
/// the SHA-256 bytes and force the same logical Iroha-hash marker bit without a Blake2b gadget.
#[must_use]
pub fn kagemusha_mint_finality_root_v1(root: KagemushaPastaStateCommitmentV1) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(KAGEMUSHA_MINT_FINALITY_ROOT_DOMAIN_V1);
    hasher.update([0]);
    hasher.update(root.eq);
    hasher.update(root.ep);
    Hash::prehashed(hasher.finalize().into())
}

/// Structural failure at the Kagemusha V1 chain boundary.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KagemushaIsiValidationErrorV1 {
    /// A value used a version other than the sole first-release layout.
    #[error("unsupported Kagemusha chain version {actual}")]
    UnsupportedVersion {
        /// Encountered version.
        actual: u16,
    },
    /// A required public field was zero, inconsistent, or non-canonical.
    #[error("invalid Kagemusha field `{field}`")]
    InvalidField {
        /// Stable field label.
        field: &'static str,
    },
    /// A canonical wire object failed its own validation.
    #[error("invalid Kagemusha wire object: {0}")]
    InvalidWire(String),
    /// Canonical Norito encoding needed for a commitment failed.
    #[error("failed to encode canonical Kagemusha data: {0}")]
    Encoding(String),
    /// Finality evidence or its reserve-receipt witness was invalid.
    #[error("invalid Kagemusha finality evidence: {0}")]
    InvalidFinality(String),
    /// Checked reserve subtraction failed.
    #[error("Kagemusha reserve totals underflow")]
    ReserveUnderflow,
    /// An operation status combined mutually exclusive fields.
    #[error("invalid Kagemusha operation status shape")]
    InvalidStatus,
    /// A terminal result was inspected without an externally pinned consensus context.
    #[error("Kagemusha terminal validation requires a caller-pinned finality context")]
    MissingTrustAnchor,
}

/// Immutable kind of one idempotent reserve operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum KagemushaOperationKindV1 {
    /// Debit online funds, increase the reserve, and fix one mint output.
    #[codec(index = 0)]
    TopUp,
    /// Consume one terminal nullifier, decrease the reserve, and credit an account.
    #[codec(index = 1)]
    Redemption,
}

/// Immutable top-up intent accepted before its containing block is finalized.
///
/// `operation_id`, `issuance_commitment`, and `credit_id` are fixed before the
/// ledger commit. The ciphertext and artifact digest are retained verbatim so
/// crash recovery can reproduce the exact terminal [`KagemushaMintCreditV1`].
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(schema_name = "iroha.torii.v1.kagemusha.top_up.request")]
#[norito(deny_unknown_fields)]
pub struct KagemushaTopUpRequestV1 {
    /// Chain layout version.
    pub version: u16,
    /// Globally unique idempotency key chosen before submission.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub operation_id: [u8; 32],
    /// Deterministic pre-encryption commitment to the issuance identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub issuance_commitment: [u8; 32],
    /// Unique mint-credit identifier fixed by this intent.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_id: [u8; 32],
    /// Authenticated paired-Pasta proof release selected for minting.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_id: [u8; 32],
    /// Exact governed proof suite selected from the authenticated release.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub suite_id: [u8; 32],
    /// Digest of the exact release-pinned verifying-key set.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub vk_digest: [u8; 32],
    /// Exact network whose reserve accepts the liability.
    pub network_id: NetworkId,
    /// Asset debited online and represented by the resulting Kagemusha credit.
    pub asset: AssetDefinitionId,
    /// Exact incarnation of the debited asset definition.
    ///
    /// Structural validation checks the typed token; Core must additionally
    /// match it to the authoritative registered-asset incarnation in world
    /// state rather than treating request bytes as authority.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Authoritative fixed asset scale.
    pub scale: u32,
    /// Positive amount in atomic units.
    pub amount: u128,
    /// Sole deterministic reserve pool for `(network_id, asset, asset_incarnation)`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub liability_pool_id: [u8; 32],
    /// Online account atomically debited by this operation.
    pub payer: AccountId,
    /// Kagemusha account that owns the resulting device-bound mint credit.
    pub recipient: AccountId,
    /// Complete compact credential authenticated by the mint helper.
    ///
    /// This embedded value is not monetary authority by itself. Core must
    /// authenticate it against the profile enabled by the exact release.
    pub hardware_credential: KagemushaHardwareCredentialV1,
    /// Per-mint randomized commitment to the recipient credential.
    ///
    /// Stable credential, lane, hardware epoch, and key identities remain
    /// private and do not appear in the resulting public mint credit. This is
    /// sampled before and independently of the final credit ID.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_credential_commitment: [u8; 32],
    /// Receiver-bound private-value commitment sampled independently of the
    /// final credit ID and fixed before chain execution.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_commitment: [u8; 32],
    /// Fresh recipient encryption key whose private half is held by qualified hardware.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_one_time_key: [u8; 32],
    /// Recipient-only encrypted opening retained for exact recovery.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub encrypted_credit: Vec<u8>,
    /// Digest of the exact proof artifact manifest used for final minting.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub artifact_manifest_digest: [u8; 32],
    /// Paired recipient authorization verified before the payer debit.
    ///
    /// `None` is permitted only in an in-memory construction draft before the
    /// issuance commitment, credit ID, and AEAD bytes exist. Canonical request
    /// validation and chain admission require `Some`.
    #[norito(required)]
    pub mint_authorization: Option<KagemushaMintAuthorizationV1>,
}

impl PartialOrd for KagemushaTopUpRequestV1 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for KagemushaTopUpRequestV1 {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.encode().cmp(&other.encode())
    }
}

/// Canonical caller-supplied issuance intent.
///
/// The two derived identifiers and encrypted credit are intentionally absent
/// to keep construction acyclic. The issuance commitment is computed first,
/// then the credit ID, and only then may AEAD bind that ID as plaintext or
/// associated data. The complete canonical top-up request digest, reserve
/// finality proof, and mint lifecycle ciphertext digest subsequently bind the
/// exact encrypted bytes. `credit_commitment` is a pre-ID randomized value and
/// must not be derived from the final credit ID.
#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.top-up-issuance-preimage")]
struct KagemushaTopUpIssuancePreimageV1 {
    version: u16,
    operation_id: [u8; 32],
    release_id: [u8; 32],
    suite_id: [u8; 32],
    vk_digest: [u8; 32],
    network_id: NetworkId,
    asset: AssetDefinitionId,
    asset_incarnation: AxtAssetIncarnationV1,
    scale: u32,
    amount: u128,
    liability_pool_id: [u8; 32],
    payer: AccountId,
    recipient: AccountId,
    hardware_credential: KagemushaHardwareCredentialV1,
    recipient_credential_commitment: [u8; 32],
    credit_commitment: [u8; 32],
    recipient_one_time_key: [u8; 32],
    authorization_context_digest: [u8; 32],
    artifact_manifest_digest: [u8; 32],
}

impl KagemushaTopUpRequestV1 {
    /// Project the exact ID-independent context authorized by recipient hardware.
    #[must_use]
    pub fn mint_authorization_context(&self) -> KagemushaMintAuthorizationContextV1 {
        KagemushaMintAuthorizationContextV1 {
            version: self.version,
            operation_id: self.operation_id,
            release_id: self.release_id,
            suite_id: self.suite_id,
            vk_digest: self.vk_digest,
            artifact_manifest_digest: self.artifact_manifest_digest,
            network_id: self.network_id,
            asset: self.asset.clone(),
            asset_incarnation: self.asset_incarnation,
            scale: self.scale,
            liability_pool_id: self.liability_pool_id,
            amount: self.amount,
            payer: self.payer.clone(),
            recipient: self.recipient.clone(),
            hardware_credential_id: self.hardware_credential.credential_id,
            hardware_profile_id: self.hardware_credential.hardware_profile_id,
            policy_epoch: self.hardware_credential.policy_epoch,
            recipient_credential_commitment: self.recipient_credential_commitment,
            credit_commitment: self.credit_commitment,
            recipient_one_time_key: self.recipient_one_time_key,
        }
    }

    fn issuance_preimage(
        &self,
    ) -> Result<KagemushaTopUpIssuancePreimageV1, KagemushaIsiValidationErrorV1> {
        Ok(KagemushaTopUpIssuancePreimageV1 {
            version: self.version,
            operation_id: self.operation_id,
            release_id: self.release_id,
            suite_id: self.suite_id,
            vk_digest: self.vk_digest,
            network_id: self.network_id,
            asset: self.asset.clone(),
            asset_incarnation: self.asset_incarnation,
            scale: self.scale,
            amount: self.amount,
            liability_pool_id: self.liability_pool_id,
            payer: self.payer.clone(),
            recipient: self.recipient.clone(),
            hardware_credential: self.hardware_credential,
            recipient_credential_commitment: self.recipient_credential_commitment,
            credit_commitment: self.credit_commitment,
            recipient_one_time_key: self.recipient_one_time_key,
            authorization_context_digest: self
                .mint_authorization_context()
                .canonical_digest()
                .map_err(wire_error)?,
            artifact_manifest_digest: self.artifact_manifest_digest,
        })
    }

    fn mint_statement_with_credit_id(
        &self,
        credit_id: [u8; 32],
        minted_at_ms: u64,
        mint_authorization_digest: [u8; 32],
    ) -> Result<KagemushaMintCreditStatementV1, KagemushaIsiValidationErrorV1> {
        Ok(KagemushaMintCreditStatementV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            lifecycle: crate::kagemusha::KagemushaLifecycleBindingV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                network_id: self.network_id,
                protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
                suite_id: self.suite_id,
                vk_digest: self.vk_digest,
                release_id: self.release_id,
                asset: self.asset.clone(),
                asset_incarnation: self.asset_incarnation,
                scale: self.scale,
                liability_pool_id: self.liability_pool_id,
                hardware_profile_id: self.hardware_credential.hardware_profile_id,
                policy_epoch: self.hardware_credential.policy_epoch,
                operation_kind: crate::kagemusha::KagemushaOperationKindV1::MintFold,
                request_id: [0; 32],
                credit_id,
                ciphertext_digest: kagemusha_ciphertext_digest_v1(&self.encrypted_credit),
            },
            recipient_credential_commitment: self.recipient_credential_commitment,
            authorization_context_digest: self
                .mint_authorization_context()
                .canonical_digest()
                .map_err(wire_error)?,
            mint_authorization_digest,
            amount: self.amount,
            issuance_commitment: self.issuance_commitment,
            recipient: self.recipient.clone(),
            credit_commitment: self.credit_commitment,
            minted_at_ms,
        })
    }

    fn validate_identity_base(&self) -> Result<(), KagemushaIsiValidationErrorV1> {
        require_chain_version(self.version)?;
        require_nonzero("top_up.operation_id", self.operation_id)?;
        require_nonzero("top_up.release_id", self.release_id)?;
        require_nonzero("top_up.suite_id", self.suite_id)?;
        require_nonzero("top_up.vk_digest", self.vk_digest)?;
        require_nonzero("top_up.liability_pool_id", self.liability_pool_id)?;
        require_nonzero(
            "top_up.recipient_credential_commitment",
            self.recipient_credential_commitment,
        )?;
        require_nonzero("top_up.credit_commitment", self.credit_commitment)?;
        require_nonzero("top_up.recipient_one_time_key", self.recipient_one_time_key)?;
        require_nonzero(
            "top_up.artifact_manifest_digest",
            self.artifact_manifest_digest,
        )?;
        if self.network_id.as_bytes() == &[0; 32]
            || self.scale > KAGEMUSHA_ASSET_SCALE_MAX_V1
            || self.amount == 0
        {
            return Err(invalid("top_up.header"));
        }
        self.asset_incarnation
            .validate()
            .map_err(|_| invalid("top_up.asset_incarnation"))?;
        self.hardware_credential
            .validate_shape()
            .map_err(wire_error)?;
        self.mint_authorization_context()
            .validate_shape()
            .map_err(wire_error)?;
        if self.hardware_credential.network_id != self.network_id
            || self.hardware_credential.suite_id != self.suite_id
        {
            return Err(invalid("top_up.hardware_credential.context"));
        }
        if self.liability_pool_id
            != kagemusha_liability_pool_id_v1(
                &self.network_id,
                &self.asset,
                self.asset_incarnation,
            )
            .map_err(wire_error)?
        {
            return Err(invalid("top_up.liability_pool_id"));
        }
        Ok(())
    }

    fn validate_shape_base(&self) -> Result<(), KagemushaIsiValidationErrorV1> {
        self.validate_identity_base()?;
        KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
            &self.encrypted_credit,
            self.recipient_one_time_key,
        )
        .map_err(wire_error)?;
        Ok(())
    }

    /// Compute the deterministic pre-encryption issuance identifier.
    ///
    /// Construction order is issuance commitment, credit ID, then encrypted
    /// credit. The final canonical request digest and mint-finality proof bind
    /// the encrypted bytes after this pre-encryption identity is fixed.
    ///
    /// # Errors
    ///
    /// Returns an error when the intent is structurally invalid or cannot be encoded canonically.
    pub fn expected_issuance_commitment(
        &self,
    ) -> Result<[u8; 32], KagemushaIsiValidationErrorV1> {
        self.validate_identity_base()?;
        digest_encoded(TOP_UP_ISSUANCE_DOMAIN_V1, &self.issuance_preimage()?)
    }

    /// Compute the unique output-credit identifier fixed by this intent.
    ///
    /// # Errors
    ///
    /// Returns an error when the intent is structurally invalid or the wire preimage cannot encode.
    pub fn expected_credit_id(&self) -> Result<[u8; 32], KagemushaIsiValidationErrorV1> {
        self.validate_identity_base()?;
        require_nonzero("top_up.issuance_commitment", self.issuance_commitment)?;
        self.mint_statement_with_credit_id([0; 32], 0, [0; 32])?
            .expected_credit_id()
            .map_err(wire_error)
    }

    /// Fill the issuance commitment and credit ID before constructing AEAD bytes.
    ///
    /// The returned request may still have an empty `encrypted_credit`; callers
    /// can safely bind the returned credit ID into AEAD plaintext or associated
    /// data, then build and attach the recipient authorization before calling
    /// [`Self::validate_shape`].
    ///
    /// # Errors
    ///
    /// Returns an error when the pre-encryption intent is invalid or hashing fails.
    pub fn seal_pre_encryption_identifiers(
        mut self,
    ) -> Result<Self, KagemushaIsiValidationErrorV1> {
        self.validate_identity_base()?;
        self.issuance_commitment = self.expected_issuance_commitment()?;
        self.credit_id = self.expected_credit_id()?;
        Ok(self)
    }

    /// Fill both deterministic identifiers for a request with existing AEAD bytes.
    ///
    /// Callers that need the credit ID as AEAD plaintext or associated data use
    /// [`Self::seal_pre_encryption_identifiers`] first. This convenience method
    /// is for requests whose encrypted credit is already populated without a
    /// dependency on the final credit ID. Both paths still require a later
    /// [`Self::attach_mint_authorization`] call before admission.
    ///
    /// # Errors
    ///
    /// Returns an error when the base intent is invalid or canonical hashing fails.
    pub fn seal_identifiers(self) -> Result<Self, KagemushaIsiValidationErrorV1> {
        let self_ = self.seal_pre_encryption_identifiers()?;
        self_.validate_shape_base()?;
        Ok(self_)
    }

    /// Build the exact post-encryption statement recipient hardware must prove.
    ///
    /// This method is usable before `mint_authorization` is attached. It
    /// verifies both derived identifiers and the final ciphertext digest, then
    /// returns the sole statement accepted by [`Self::validate_shape`].
    ///
    /// # Errors
    ///
    /// Returns an error unless the context, identifiers, and encrypted credit
    /// are complete and canonical.
    pub fn mint_authorization_statement(
        &self,
    ) -> Result<KagemushaMintAuthorizationStatementV1, KagemushaIsiValidationErrorV1> {
        self.validate_shape_base()?;
        require_nonzero("top_up.issuance_commitment", self.issuance_commitment)?;
        require_nonzero("top_up.credit_id", self.credit_id)?;
        if self.issuance_commitment != self.expected_issuance_commitment()? {
            return Err(invalid("top_up.issuance_commitment"));
        }
        if self.credit_id != self.expected_credit_id()? {
            return Err(invalid("top_up.credit_id"));
        }
        let statement = KagemushaMintAuthorizationStatementV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            context: self.mint_authorization_context(),
            issuance_commitment: self.issuance_commitment,
            credit_id: self.credit_id,
            ciphertext_digest: kagemusha_ciphertext_digest_v1(&self.encrypted_credit),
        };
        statement.validate_shape().map_err(wire_error)?;
        statement.encrypted_credit_aad().map_err(wire_error)?;
        Ok(statement)
    }

    /// Attach and validate the release-pinned recipient authorization proof.
    ///
    /// # Errors
    ///
    /// Returns an error unless `authorization` proves the exact derived IDs,
    /// recipient context, and encrypted credit already present in this request.
    pub fn attach_mint_authorization(
        mut self,
        authorization: KagemushaMintAuthorizationV1,
    ) -> Result<Self, KagemushaIsiValidationErrorV1> {
        self.mint_authorization = Some(authorization);
        self.validate_shape()?;
        Ok(self)
    }

    /// Validate all structural context, credential, reserve, output, and ID bindings.
    ///
    /// This checks the embedded credential's shape only. It does not make the
    /// submitter-selected profile or release authoritative; monetary execution
    /// must call [`Self::validate_against_profile`] using the profile selected
    /// from the authenticated release catalog and must independently match
    /// `release_id`, `suite_id`, `vk_digest`, and `artifact_manifest_digest`.
    ///
    /// # Errors
    ///
    /// Returns a typed error when any field is invalid or either identifier is non-canonical.
    pub fn validate_shape(&self) -> Result<(), KagemushaIsiValidationErrorV1> {
        let expected_authorization_statement = self.mint_authorization_statement()?;
        let authorization = self
            .mint_authorization
            .as_ref()
            .ok_or_else(|| invalid("top_up.mint_authorization.presence"))?;
        authorization.validate_shape().map_err(wire_error)?;
        if authorization.statement != expected_authorization_statement {
            return Err(invalid("top_up.mint_authorization.binding"));
        }
        Ok(())
    }

    /// Authenticate the embedded credential against an exact governed profile.
    ///
    /// The caller must resolve `profile` from the authenticated release whose
    /// identity, suite, verifying keys, and artifact manifest match this
    /// request. A self-consistent caller-supplied profile is not chain authority.
    ///
    /// # Errors
    ///
    /// Returns an error when structural request validation or credential/profile
    /// authentication fails.
    pub fn validate_against_profile(
        &self,
        profile: &KagemushaHardwareProfileV1,
    ) -> Result<(), KagemushaIsiValidationErrorV1> {
        self.validate_shape()?;
        self.hardware_credential
            .validate_against_profile(profile)
            .map_err(wire_error)
    }

    fn mint_statement_shape(
        &self,
        committed_at_ms: u64,
    ) -> Result<KagemushaMintCreditStatementV1, KagemushaIsiValidationErrorV1> {
        self.validate_shape()?;
        if committed_at_ms == 0 {
            return Err(invalid("top_up.committed_at_ms"));
        }
        let mint_authorization_digest = self
            .mint_authorization
            .as_ref()
            .ok_or_else(|| invalid("top_up.mint_authorization.presence"))?
            .canonical_digest()
            .map_err(wire_error)?;
        let statement = self.mint_statement_with_credit_id(
            self.credit_id,
            committed_at_ms,
            mint_authorization_digest,
        )?;
        statement.validate_shape().map_err(wire_error)?;
        Ok(statement)
    }

    /// Return the exact authenticated mint statement attached after finality.
    ///
    /// `committed_at_ms` must come from the certified reserve receipt. It is not
    /// accepted from a client and does not participate in the pre-commit credit ID.
    /// `profile` must be the enabled profile resolved from the authenticated
    /// release; the caller must also exact-match the request's release, suite,
    /// verifying-key, and artifact-manifest bindings to that release.
    ///
    /// # Errors
    ///
    /// Returns an error unless this request and its profile credential are valid.
    pub fn mint_statement_against_profile(
        &self,
        profile: &KagemushaHardwareProfileV1,
        committed_at_ms: u64,
    ) -> Result<KagemushaMintCreditStatementV1, KagemushaIsiValidationErrorV1> {
        self.validate_against_profile(profile)?;
        self.mint_statement_shape(committed_at_ms)
    }

    /// Return the canonical request digest committed by the reserve receipt.
    ///
    /// # Errors
    ///
    /// Returns an error unless this request is valid and canonically encodable.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaIsiValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(TOP_UP_REQUEST_DOMAIN_V1, self)
    }
}

/// One hardware-bound redemption request submitted to consensus.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(schema_name = "iroha.torii.v1.kagemusha.redeem.request")]
#[norito(deny_unknown_fields)]
pub struct KagemushaRedemptionRequestV1 {
    /// Chain layout version.
    pub version: u16,
    /// Globally unique idempotency key chosen before submission.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub operation_id: [u8; 32],
    /// Full or partial unlinkable terminal voucher, including its hardware
    /// commit certificate and final wrapper proof.
    pub voucher: KagemushaRedemptionVoucherV1,
}

impl PartialOrd for KagemushaRedemptionRequestV1 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for KagemushaRedemptionRequestV1 {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.encode().cmp(&other.encode())
    }
}

impl KagemushaRedemptionRequestV1 {
    /// Return the complete released lifecycle binding authenticated by the
    /// terminal wrapper.
    #[must_use]
    pub const fn lifecycle(&self) -> &crate::kagemusha::KagemushaLifecycleBindingV1 {
        &self.voucher.statement.lifecycle
    }

    /// Return the proof-derived terminal nullifier used for exact conflict
    /// detection, without exposing a predecessor or successor state.
    #[must_use]
    pub const fn terminal_nullifier(&self) -> [u8; 32] {
        self.voucher.statement.terminal_nullifier
    }

    /// Validate the operation identity and recursive-voucher shape.
    ///
    /// This does not cryptographically verify the wrapper proof or authenticate
    /// its release. Monetary admission must use the release-pinned Core
    /// verifier and its verified typestate.
    ///
    /// # Errors
    ///
    /// Returns an error for a wrong version, zero operation ID, or invalid voucher.
    pub fn validate_shape(&self) -> Result<(), KagemushaIsiValidationErrorV1> {
        require_chain_version(self.version)?;
        require_nonzero("redemption.operation_id", self.operation_id)?;
        self.voucher.validate_shape().map_err(wire_error)
    }

    /// Return the canonical request digest committed by the reserve receipt.
    ///
    /// # Errors
    ///
    /// Returns an error unless this request is valid and canonically encodable.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaIsiValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(REDEMPTION_REQUEST_DOMAIN_V1, self)
    }
}

/// Canonical post-operation reserve totals committed by consensus.
///
/// The authenticated invariant is always
/// `available = total_topups - total_redemptions`. Peer-to-peer payments never
/// create this record and never modify these totals.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaReserveReceiptV1 {
    /// Receipt layout version.
    pub version: u16,
    /// Idempotent operation identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub operation_id: [u8; 32],
    /// Immutable operation kind.
    pub kind: KagemushaOperationKindV1,
    /// Digest of the exact request whose execution produced this receipt.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// Canonical mint-statement digest fixed by a top-up's committed block time.
    ///
    /// This is non-zero only for [`KagemushaOperationKindV1::TopUp`].  It is
    /// carried by the consensus receipt because the request digest alone cannot
    /// reconstruct the statement's `minted_at_ms` binding.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub mint_statement_digest: [u8; 32],
    /// Exact network owning the reserve.
    pub network_id: NetworkId,
    /// Asset represented by the reserve.
    pub asset: AssetDefinitionId,
    /// Exact incarnation of the represented asset definition, copied from
    /// authoritative world state rather than selected by the submitter.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Authoritative asset scale.
    pub scale: u32,
    /// Sole deterministic liability pool.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub liability_pool_id: [u8; 32],
    /// Positive amount settled by this operation.
    pub amount: u128,
    /// Canonical digest of the immediately preceding receipt for this pool.
    ///
    /// The all-zero value is reserved for the canonical first top-up. Every
    /// later operation uses this as a constant-size compare-and-swap token;
    /// Core must match it against the pool's current head before committing.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub previous_pool_receipt_digest: [u8; 32],
    /// Checked sum of all top-ups after this operation.
    pub total_topups: u128,
    /// Checked sum of all redemptions after this operation.
    pub total_redemptions: u128,
    /// Exact signed transaction containing the operation.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub transaction_hash: [u8; 32],
    /// Authoritative committed-block time in Unix milliseconds.
    pub committed_at_ms: u64,
}

impl KagemushaReserveReceiptV1 {
    /// Return the exact reserve available after this operation.
    ///
    /// # Errors
    ///
    /// Returns an error if persisted redemptions exceed top-ups.
    pub fn available(&self) -> Result<u128, KagemushaIsiValidationErrorV1> {
        self.total_topups
            .checked_sub(self.total_redemptions)
            .ok_or(KagemushaIsiValidationErrorV1::ReserveUnderflow)
    }

    /// Validate the canonical pool identity, checked totals, and operation fields.
    ///
    /// # Errors
    ///
    /// Returns an error when any receipt or reserve invariant fails.
    pub fn validate(&self) -> Result<(), KagemushaIsiValidationErrorV1> {
        require_chain_version(self.version)?;
        require_nonzero("reserve_receipt.operation_id", self.operation_id)?;
        require_nonzero("reserve_receipt.request_digest", self.request_digest)?;
        require_nonzero("reserve_receipt.liability_pool_id", self.liability_pool_id)?;
        require_nonzero("reserve_receipt.transaction_hash", self.transaction_hash)?;
        if self.network_id.as_bytes() == &[0; 32]
            || self.scale > KAGEMUSHA_ASSET_SCALE_MAX_V1
            || self.amount == 0
            || self.committed_at_ms == 0
        {
            return Err(invalid("reserve_receipt.header"));
        }
        self.asset_incarnation
            .validate()
            .map_err(|_| invalid("reserve_receipt.asset_incarnation"))?;
        if self.liability_pool_id
            != kagemusha_liability_pool_id_v1(
                &self.network_id,
                &self.asset,
                self.asset_incarnation,
            )
            .map_err(wire_error)?
        {
            return Err(invalid("reserve_receipt.liability_pool_id"));
        }
        self.available()?;
        let is_canonical_first_top_up = self.kind == KagemushaOperationKindV1::TopUp
            && self.total_topups == self.amount
            && self.total_redemptions == 0;
        if (self.previous_pool_receipt_digest == [0; 32]) != is_canonical_first_top_up {
            return Err(invalid("reserve_receipt.previous_pool_receipt_digest"));
        }
        match self.kind {
            KagemushaOperationKindV1::TopUp => {
                require_nonzero(
                    "reserve_receipt.mint_statement_digest",
                    self.mint_statement_digest,
                )?;
                if self.amount > self.total_topups {
                    return Err(invalid("reserve_receipt.total_topups"));
                }
            }
            KagemushaOperationKindV1::Redemption => {
                if self.mint_statement_digest != [0; 32] {
                    return Err(invalid("reserve_receipt.mint_statement_digest"));
                }
                if self.amount > self.total_redemptions {
                    return Err(invalid("reserve_receipt.total_redemptions"));
                }
            }
        }
        Ok(())
    }

    /// Validate only this receipt's shape and compare-and-swap predecessor
    /// against the current pool head.
    ///
    /// `None` denotes a pool with no prior receipt and is accepted only by the
    /// canonical first top-up. A populated head must be nonzero and match the
    /// exact predecessor digest carried by this receipt.
    ///
    /// This check does not authenticate the reserve-total delta because a head
    /// digest does not reveal the preceding totals. Consensus execution must
    /// use [`Self::validate_against_previous_receipt`] before accepting a
    /// reserve mutation; this helper is only suitable for shape/CAS checks.
    ///
    /// # Errors
    ///
    /// Returns an error when the receipt is invalid, the pool initialization
    /// state is inconsistent, or the predecessor does not match.
    pub fn validate_against_pool_head(
        &self,
        current_pool_head: Option<[u8; 32]>,
    ) -> Result<(), KagemushaIsiValidationErrorV1> {
        self.validate()?;
        match current_pool_head {
            None if self.previous_pool_receipt_digest == [0; 32] => Ok(()),
            Some(head) if head != [0; 32] && self.previous_pool_receipt_digest == head => Ok(()),
            _ => Err(invalid("reserve_receipt.previous_pool_receipt_digest")),
        }
    }

    /// Authenticate this receipt's exact delta from the complete preceding receipt.
    ///
    /// `None` represents a newly created pool with zero top-ups and zero
    /// redemptions, and therefore admits only the canonical first top-up. For a
    /// populated pool, this method verifies the exact predecessor digest and
    /// requires network, asset, incarnation, scale, and derived pool identity
    /// to remain unchanged. A top-up must checked-add only `total_topups`; a
    /// redemption must checked-add only `total_redemptions`.
    ///
    /// # Errors
    ///
    /// Returns an error when either receipt is invalid, the predecessor or pool
    /// context differs, checked addition overflows, or either post-operation
    /// total is not the unique delta implied by `amount` and `kind`.
    pub fn validate_against_previous_receipt(
        &self,
        previous: Option<&Self>,
    ) -> Result<(), KagemushaIsiValidationErrorV1> {
        self.validate()?;

        let (previous_total_topups, previous_total_redemptions) = match previous {
            None => {
                if self.kind != KagemushaOperationKindV1::TopUp
                    || self.previous_pool_receipt_digest != [0; 32]
                {
                    return Err(invalid("reserve_receipt.previous_pool_receipt_digest"));
                }
                (0, 0)
            }
            Some(previous) => {
                previous.validate()?;
                if self.network_id != previous.network_id
                    || self.asset != previous.asset
                    || self.asset_incarnation != previous.asset_incarnation
                    || self.scale != previous.scale
                    || self.liability_pool_id != previous.liability_pool_id
                {
                    return Err(invalid("reserve_receipt.pool_context"));
                }
                if self.previous_pool_receipt_digest != previous.canonical_digest()? {
                    return Err(invalid("reserve_receipt.previous_pool_receipt_digest"));
                }
                (previous.total_topups, previous.total_redemptions)
            }
        };

        match self.kind {
            KagemushaOperationKindV1::TopUp => {
                let expected_total_topups = previous_total_topups
                    .checked_add(self.amount)
                    .ok_or_else(|| invalid("reserve_receipt.total_topups"))?;
                if self.total_topups != expected_total_topups
                    || self.total_redemptions != previous_total_redemptions
                {
                    return Err(invalid("reserve_receipt.top_up_delta"));
                }
            }
            KagemushaOperationKindV1::Redemption => {
                let expected_total_redemptions = previous_total_redemptions
                    .checked_add(self.amount)
                    .ok_or_else(|| invalid("reserve_receipt.total_redemptions"))?;
                if self.total_topups != previous_total_topups
                    || self.total_redemptions != expected_total_redemptions
                {
                    return Err(invalid("reserve_receipt.redemption_delta"));
                }
            }
        }

        Ok(())
    }

    /// Return the next constant-size pool-head commitment.
    ///
    /// The result is the domain-separated digest of this exact post-operation
    /// receipt. A later receipt must carry it as
    /// `previous_pool_receipt_digest`.
    ///
    /// # Errors
    ///
    /// Returns an error unless the receipt is valid and canonically encodable.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaIsiValidationErrorV1> {
        self.validate()?;
        digest_encoded(RESERVE_RECEIPT_DOMAIN_V1, self)
    }
}

/// Circuit-native leaf committed by a finalized block for one Kagemusha V1 top-up.
///
/// The statement digest binds the complete receiver credit, while the receipt digest binds the
/// exact consensus reserve mutation. The paired helper circuit exposes `statement_digest` and
/// `amount` and proves this leaf's fixed-depth membership in the root signed by the Commit quorum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaTopUpLeafV1 {
    /// Chain layout version.
    pub version: u16,
    /// Exact committed top-up operation.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub operation_id: [u8; 32],
    /// Canonical digest of the complete typed reserve receipt.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub reserve_receipt_digest: [u8; 32],
    /// Canonical mint statement digest exposed as the first two helper instances.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub statement_digest: [u8; 32],
    /// Positive mint amount exposed as the third helper instance.
    pub amount: u128,
}

impl KagemushaTopUpLeafV1 {
    /// Validate all exact top-up leaf bindings.
    ///
    /// # Errors
    ///
    /// Returns an error for a wrong version, a zero identity, or a zero amount.
    pub fn validate(&self) -> Result<(), KagemushaIsiValidationErrorV1> {
        require_chain_version(self.version)?;
        require_nonzero("mint_finality.leaf.operation_id", self.operation_id)?;
        require_nonzero(
            "mint_finality.leaf.reserve_receipt_digest",
            self.reserve_receipt_digest,
        )?;
        require_nonzero("mint_finality.leaf.statement_digest", self.statement_digest)?;
        if self.amount == 0 {
            return Err(invalid("mint_finality.leaf.amount"));
        }
        Ok(())
    }
}

/// Private fixed-depth membership witness consumed by both mint-helper parities.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaTopUpMembershipWitnessV1 {
    /// Exact leaf whose statement and amount become public helper instances.
    pub leaf: KagemushaTopUpLeafV1,
    /// Zero-based position in the block's canonical top-up order.
    pub leaf_index: u32,
    /// Paired field-native root whose marked SHA-256 bridge is signed in the execution commitment.
    pub root: KagemushaPastaStateCommitmentV1,
    /// Exactly 32 paired siblings from leaf to root.
    pub siblings: Vec<KagemushaPastaStateCommitmentV1>,
}

impl KagemushaTopUpMembershipWitnessV1 {
    /// Validate fixed-depth shape and ensure the leaf is within the sealed real-leaf count.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid leaf, path length, root, count, or leaf position.
    pub fn validate(&self, top_up_count: u32) -> Result<(), KagemushaIsiValidationErrorV1> {
        self.leaf.validate()?;
        require_nonzero("mint_finality.membership.root.eq", self.root.eq)?;
        require_nonzero("mint_finality.membership.root.ep", self.root.ep)?;
        if self.siblings.len() != KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1
            || top_up_count == 0
            || self.leaf_index >= top_up_count
        {
            return Err(invalid("mint_finality.membership"));
        }
        Ok(())
    }

    /// Validate shape and bind the paired root to one signed block message.
    ///
    /// # Errors
    ///
    /// Returns an error when the path is malformed or its root bridge/count does not match the
    /// block-level mint-finality seal message.
    pub fn validate_against(
        &self,
        message: &KagemushaMintFinalitySealMessageV1,
    ) -> Result<(), KagemushaIsiValidationErrorV1> {
        message.validate()?;
        self.validate(message.kagemusha_top_up_count)?;
        if kagemusha_mint_finality_root_v1(self.root) != message.kagemusha_top_up_root {
            return Err(invalid("mint_finality.membership.root"));
        }
        Ok(())
    }
}

/// Fixed block-level message authorized by the consensus mint-finality seal quorum.
///
/// The paired seals are appended to Commit votes over the same subject and execution result as the
/// ordinary BLS signature. They authenticate the circuit-friendly top-up root without creating a
/// second consensus round or accepting a host-side finality result as monetary authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaMintFinalitySealMessageV1 {
    /// Chain layout version.
    pub version: u16,
    /// Epoch-scoped identity of the fixed paired Pasta validator keys.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub finality_epoch_id: [u8; 32],
    /// Exact `3f + 1` validator count baked into the mint helper keys.
    pub validator_count: u32,
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Finalized block height.
    pub block_height: u64,
    /// Frozen consensus context governing `block_height`.
    pub height_context_id: HeightContextId,
    /// Digest of the exact Commit vote subject.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub subject_digest: [u8; 32],
    /// Digest of the full exact execution commitment signed by the ordinary Commit vote.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub execution_commitment_digest: [u8; 32],
    /// Marked SHA-256 bridge of the paired Poseidon top-up tree root.
    pub kagemusha_top_up_root: Hash,
    /// Number of real leaves in the sparse depth-32 tree.
    pub kagemusha_top_up_count: u32,
    /// Next epoch's complete Pasta-roster identity when this Commit seals an epoch boundary.
    ///
    /// The old epoch's exact quorum signs this value even when the boundary block has no top-ups,
    /// providing the recursive authority-rotation carrier needed by later mint proofs.
    #[norito(required)]
    #[cfg_attr(
        feature = "json",
        norito(json = "crate::json_helpers::fixed_bytes::option")
    )]
    pub next_finality_epoch_id: Option<[u8; 32]>,
}

impl KagemushaMintFinalitySealMessageV1 {
    /// Validate the fixed message shape and consensus committee geometry.
    ///
    /// # Errors
    ///
    /// Returns an error when any authority-bearing identity is absent, the top-up projection is
    /// empty or oversized, or `validator_count` is not an admitted `3f + 1` committee size.
    pub fn validate(&self) -> Result<(), KagemushaIsiValidationErrorV1> {
        require_chain_version(self.version)?;
        for (field, value) in [
            ("mint_finality.finality_epoch_id", self.finality_epoch_id),
            ("mint_finality.subject_digest", self.subject_digest),
            (
                "mint_finality.execution_commitment_digest",
                self.execution_commitment_digest,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        let validator_count = usize::try_from(self.validator_count)
            .map_err(|_| invalid("mint_finality.validator_count"))?;
        if self
            .next_finality_epoch_id
            .is_some_and(|next_epoch_id| next_epoch_id == [0; 32])
            || !is_valid_committee_size(validator_count)
            || self.network_id.as_bytes() == &[0; 32]
            || self.block_height == 0
            || (self.kagemusha_top_up_count == 0 && self.next_finality_epoch_id.is_none())
            || self.kagemusha_top_up_root == Hash::prehashed([0; Hash::LENGTH])
            || self
                .height_context_id
                .0
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(invalid("mint_finality.header"));
        }
        Ok(())
    }

    /// Return the fixed-width SHA-256 message signed by both Pasta validator keys.
    ///
    /// The explicit field encoding is intentionally independent of Norito container choices so
    /// the same byte sequence can be reconstructed by the mint helper circuit.
    ///
    /// # Errors
    ///
    /// Returns an error unless the complete message is structurally valid.
    pub fn signing_digest(&self) -> Result<[u8; 32], KagemushaIsiValidationErrorV1> {
        self.validate()?;
        let mut hasher = Sha256::new();
        hasher.update(MINT_FINALITY_SEAL_MESSAGE_DOMAIN_V1);
        hasher.update([0]);
        hasher.update(self.version.to_le_bytes());
        hasher.update(self.finality_epoch_id);
        hasher.update(self.validator_count.to_le_bytes());
        hasher.update(self.network_id.as_bytes());
        hasher.update(self.block_height.to_le_bytes());
        hasher.update(self.height_context_id.0.as_ref());
        hasher.update(self.subject_digest);
        hasher.update(self.execution_commitment_digest);
        hasher.update(self.kagemusha_top_up_root.as_ref());
        hasher.update(self.kagemusha_top_up_count.to_le_bytes());
        match self.next_finality_epoch_id {
            Some(next_epoch_id) => {
                hasher.update([1]);
                hasher.update(next_epoch_id);
            }
            None => {
                hasher.update([0]);
                hasher.update([0; 32]);
            }
        }
        Ok(hasher.finalize().into())
    }
}

/// One canonical Schnorr signature for an epoch-scoped Pasta validator key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaPastaSchnorrSignatureV1 {
    /// Canonical compressed non-identity nonce point.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub nonce_commitment: [u8; 32],
    /// Canonical non-zero response scalar.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub response: [u8; 32],
}

impl KagemushaPastaSchnorrSignatureV1 {
    fn validate(&self) -> Result<(), KagemushaIsiValidationErrorV1> {
        require_nonzero(
            "mint_finality.signature.nonce_commitment",
            self.nonce_commitment,
        )?;
        require_nonzero("mint_finality.signature.response", self.response)
    }
}

/// Paired signatures from one fixed roster position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaMintFinalityValidatorSealV1 {
    /// Zero-based position in the exact epoch roster.
    pub validator_index: u32,
    /// Signature checked by the Eq/Fp mint helper using the roster's Pallas key.
    pub eq_proof_signature: KagemushaPastaSchnorrSignatureV1,
    /// Signature checked by the Ep/Fq mint helper using the roster's Vesta key.
    pub ep_proof_signature: KagemushaPastaSchnorrSignatureV1,
}

/// Canonical auxiliary payload appended to one BLS Commit-vote signature.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaMintFinalitySealShareV1 {
    /// Sole first-release layout version.
    pub version: u16,
    /// Common block-level statement signed by every validator.
    pub message: KagemushaMintFinalitySealMessageV1,
    /// Paired seal belonging to the enclosing vote's signer index.
    pub seal: KagemushaMintFinalityValidatorSealV1,
}

impl KagemushaMintFinalitySealShareV1 {
    /// Validate the canonical share shape against the message's roster bound.
    ///
    /// # Errors
    ///
    /// Returns an error for a wrong version/message, out-of-range signer, or
    /// malformed paired signature encoding.
    pub fn validate(&self) -> Result<(), KagemushaIsiValidationErrorV1> {
        require_chain_version(self.version)?;
        self.message.validate()?;
        if self.seal.validator_index >= self.message.validator_count {
            return Err(invalid("mint_finality.share.validator_index"));
        }
        self.seal.eq_proof_signature.validate()?;
        self.seal.ep_proof_signature.validate()
    }
}

/// Exact `2f + 1` paired validator seals for one finalized top-up receipt.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaMintFinalitySealBundleV1 {
    /// Common signed finality message.
    pub message: KagemushaMintFinalitySealMessageV1,
    /// Strictly increasing exact-quorum signer records.
    pub seals: Vec<KagemushaMintFinalityValidatorSealV1>,
}

impl KagemushaMintFinalitySealBundleV1 {
    /// Validate the complete fixed-roster/quorum structure before circuit proving.
    ///
    /// Canonical point/scalar decoding and every signature equation are additionally enforced by
    /// the paired helper circuits.
    ///
    /// # Errors
    ///
    /// Returns an error unless signer indices are strictly increasing, in range, and exactly the
    /// canonical `2f + 1` threshold for `message.validator_count`.
    pub fn validate(&self) -> Result<(), KagemushaIsiValidationErrorV1> {
        self.message.validate()?;
        let validator_count = usize::try_from(self.message.validator_count)
            .map_err(|_| invalid("mint_finality.validator_count"))?;
        let expected = validator_count * 2 / 3 + 1;
        if self.seals.len() != expected
            || self.seals.len() > KAGEMUSHA_MINT_FINALITY_MAX_SEALS_V1
            || self
                .seals
                .windows(2)
                .any(|pair| pair[0].validator_index >= pair[1].validator_index)
            || self.seals.iter().any(|seal| {
                usize::try_from(seal.validator_index).map_or(true, |i| i >= validator_count)
            })
        {
            return Err(invalid("mint_finality.seals"));
        }
        for seal in &self.seals {
            seal.eq_proof_signature.validate()?;
            seal.ep_proof_signature.validate()?;
        }
        Ok(())
    }
}

/// Sparse-Merkle proof that one canonical reserve receipt was an ordinary write.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaReserveReceiptWitnessV1 {
    /// Exact `0xD5 || operation_id` execution-witness key.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub key: Vec<u8>,
    /// Typed canonical value stored under `key`.
    pub receipt: KagemushaReserveReceiptV1,
    /// Exactly 256 siblings from leaf to ordinary-write root.
    pub siblings: Vec<Hash>,
}

impl KagemushaReserveReceiptWitnessV1 {
    /// Derive the sole ordinary-write key for an operation.
    #[must_use]
    pub fn expected_key(operation_id: [u8; 32]) -> Vec<u8> {
        let mut key = Vec::with_capacity(KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_BYTES_V1);
        key.push(KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_TAG_V1);
        key.extend_from_slice(&operation_id);
        key
    }

    /// Reconstruct the ordinary-write sparse-Merkle root.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed receipt, key, sibling count, or encoding failure.
    pub fn reconstructed_root(&self) -> Result<Hash, KagemushaIsiValidationErrorV1> {
        self.receipt.validate()?;
        if self.key != Self::expected_key(self.receipt.operation_id) {
            return Err(invalid("reserve_receipt_witness.key"));
        }
        if self.siblings.len() != KAGEMUSHA_RESERVE_RECEIPT_WITNESS_SIBLINGS_V1 {
            return Err(invalid("reserve_receipt_witness.siblings"));
        }
        let value = norito::encode_canonical(&self.receipt)
            .map_err(|error| KagemushaIsiValidationErrorV1::Encoding(error.to_string()))?;
        let path = Hash::new(&self.key);
        let value_hash = Hash::new(&value);
        let mut leaf_preimage = Vec::with_capacity(1 + 2 * Hash::LENGTH);
        leaf_preimage.push(0);
        leaf_preimage.extend_from_slice(path.as_ref());
        leaf_preimage.extend_from_slice(value_hash.as_ref());
        let mut current = Hash::new(leaf_preimage);
        for (level, sibling) in self.siblings.iter().copied().enumerate() {
            let path_bit = 255_usize.saturating_sub(level);
            let byte = path.as_ref()[path_bit / 8];
            let right = byte & (1_u8 << (path_bit % 8)) != 0;
            current = if right {
                ordinary_smt_node_hash(sibling, current)
            } else {
                ordinary_smt_node_hash(current, sibling)
            };
        }
        Ok(current)
    }

    /// Verify this receipt against a finality-authenticated ordinary-write root.
    #[must_use]
    pub fn verify(&self, expected_ordinary_writes_root: Hash) -> bool {
        self.reconstructed_root()
            .is_ok_and(|root| root == expected_ordinary_writes_root)
    }
}

/// Complete finality attachment for one already-committed reserve operation.
///
/// The frozen-roster certificate proves the block, while the 256-level witness
/// proves the exact typed receipt under its authenticated ordinary-write root.
/// Host-side digest comparison alone grants no monetary authority.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaOperationFinalityV1 {
    /// Finality attachment version.
    pub version: u16,
    /// Full frozen-roster Sumeragi finality certificate.
    pub finality_artifact: V2FinalityArtifact,
    /// Proof of the operation receipt under the certified ordinary-write root.
    pub reserve_receipt_witness: KagemushaReserveReceiptWitnessV1,
    /// Exact sparse-Poseidon top-up membership, present only for a mint receipt.
    ///
    /// The full 32-sibling path is retained because the ordinary-write proof
    /// cannot establish membership in the separate root authenticated by the
    /// paired Pasta Commit quorum.
    #[norito(required)]
    pub top_up_membership_witness: Option<KagemushaTopUpMembershipWitnessV1>,
}

/// Caller-pinned consensus context for one exact finalized operation block.
///
/// This value must come from release-pinned state or an already authenticated
/// context chain. It is never selected from the untrusted operation response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaFinalityTrustAnchorV1 {
    /// Exact genesis-derived network identity.
    pub network_id: NetworkId,
    /// Height governed by the pinned context.
    pub block_height: u64,
    /// Externally authenticated context identifier at `block_height`.
    pub height_context_id: HeightContextId,
}

impl KagemushaFinalityTrustAnchorV1 {
    /// Validate the externally supplied network, height, and context identity.
    ///
    /// # Errors
    ///
    /// Returns an error if any trust-anchor field is reserved or malformed.
    pub fn validate(&self) -> Result<(), KagemushaIsiValidationErrorV1> {
        if self.network_id.as_bytes() == &[0; 32]
            || self.block_height == 0
            || self
                .height_context_id
                .0
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(invalid("finality_trust_anchor"));
        }
        Ok(())
    }
}

impl KagemushaOperationFinalityV1 {
    /// Cryptographically verify the block certificate and exact reserve receipt
    /// against a caller-pinned network context.
    ///
    /// # Errors
    ///
    /// Returns an error when the pinned context, frozen-roster finality, network,
    /// height, or receipt witness fails.
    pub fn validate_against(
        &self,
        trust_anchor: &KagemushaFinalityTrustAnchorV1,
    ) -> Result<(), KagemushaIsiValidationErrorV1> {
        require_chain_version(self.version)?;
        trust_anchor.validate()?;
        if self.finality_artifact.height != trust_anchor.block_height
            || self.finality_artifact.height_context.network_id != trust_anchor.network_id
            || self.finality_artifact.context_id() != trust_anchor.height_context_id
            || self.reserve_receipt_witness.receipt.network_id != trust_anchor.network_id
        {
            return Err(KagemushaIsiValidationErrorV1::InvalidFinality(
                "response finality does not match the caller-pinned network context".into(),
            ));
        }
        self.finality_artifact
            .verify()
            .map_err(|error| KagemushaIsiValidationErrorV1::InvalidFinality(error.to_string()))?;
        let expected_root = self
            .finality_artifact
            .commit_qc
            .execution_commitment
            .ordinary_writes_root;
        if !self.reserve_receipt_witness.verify(expected_root) {
            return Err(KagemushaIsiValidationErrorV1::InvalidFinality(
                "reserve receipt is not included in the certified ordinary-write root".into(),
            ));
        }
        let receipt = &self.reserve_receipt_witness.receipt;
        match (receipt.kind, &self.top_up_membership_witness) {
            (KagemushaOperationKindV1::TopUp, Some(witness)) => {
                let commitment = self.finality_artifact.commit_qc.execution_commitment;
                witness.validate(commitment.kagemusha_top_up_count)?;
                let expected_top_up_root = commitment
                    .kagemusha_top_up_root
                    .ok_or_else(|| invalid("operation_finality.top_up_membership.root"))?;
                if kagemusha_mint_finality_root_v1(witness.root) != expected_top_up_root
                    || witness.leaf.operation_id != receipt.operation_id
                    || witness.leaf.reserve_receipt_digest != receipt.canonical_digest()?
                    || witness.leaf.statement_digest != receipt.mint_statement_digest
                    || witness.leaf.amount != receipt.amount
                {
                    return Err(invalid("operation_finality.top_up_membership"));
                }
            }
            (KagemushaOperationKindV1::Redemption, None) => {}
            _ => return Err(invalid("operation_finality.top_up_membership.presence")),
        }
        Ok(())
    }

    /// Return the finalized block height.
    #[must_use]
    pub const fn finalized_block_height(&self) -> u64 {
        self.finality_artifact.height
    }
}

/// Terminal result of one finalized top-up.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaTopUpResultV1 {
    /// Result layout version.
    pub version: u16,
    /// Exact accepted intent retained for idempotent recovery.
    pub request: KagemushaTopUpRequestV1,
    /// Certified receipt that committed the debit and reserve increase.
    pub finality: KagemushaOperationFinalityV1,
    /// Exact byte-reproducible device-bound mint credit.
    pub mint_credit: KagemushaMintCreditV1,
}

impl KagemushaTopUpResultV1 {
    /// Validate the complete request shape, reserve receipt, finality, and mint projection.
    ///
    /// This method does not authenticate the request's hardware profile or
    /// proof release. Chain monetary execution must first call
    /// [`KagemushaTopUpRequestV1::validate_against_profile`] with the enabled
    /// profile resolved from the exact authenticated release.
    ///
    /// # Errors
    ///
    /// Returns an error if any field differs from the pre-commit intent or certified receipt.
    pub fn validate_against(
        &self,
        trust_anchor: &KagemushaFinalityTrustAnchorV1,
    ) -> Result<(), KagemushaIsiValidationErrorV1> {
        require_chain_version(self.version)?;
        self.request.validate_shape()?;
        self.finality.validate_against(trust_anchor)?;
        self.mint_credit.validate_shape().map_err(wire_error)?;
        self.mint_credit
            .validate_shape_against_authorization(
                self.request
                    .mint_authorization
                    .as_ref()
                    .ok_or_else(|| invalid("top_up.mint_authorization.presence"))?,
            )
            .map_err(wire_error)?;
        let receipt = &self.finality.reserve_receipt_witness.receipt;
        let statement = &self.mint_credit.statement;
        if receipt.operation_id != self.request.operation_id
            || receipt.kind != KagemushaOperationKindV1::TopUp
            || receipt.request_digest != self.request.canonical_digest()?
            || receipt.network_id != self.request.network_id
            || receipt.asset != self.request.asset
            || receipt.asset_incarnation != self.request.asset_incarnation
            || receipt.scale != self.request.scale
            || receipt.liability_pool_id != self.request.liability_pool_id
            || receipt.amount != self.request.amount
            || statement != &self.request.mint_statement_shape(receipt.committed_at_ms)?
            || receipt.mint_statement_digest != statement.canonical_digest().map_err(wire_error)?
            || self.mint_credit.encrypted_credit != self.request.encrypted_credit
            || self.mint_credit.artifact_manifest_digest != self.request.artifact_manifest_digest
        {
            return Err(invalid("top_up_result.binding"));
        }
        Ok(())
    }
}

/// Terminal result of one finalized full or partial redemption.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaRedemptionResultV1 {
    /// Result layout version.
    pub version: u16,
    /// Exact accepted terminal request.
    pub request: KagemushaRedemptionRequestV1,
    /// Certified reserve debit and beneficiary-credit receipt.
    pub finality: KagemushaOperationFinalityV1,
}

impl KagemushaRedemptionResultV1 {
    /// Validate request, recursive voucher, terminal nullifier, and certified receipt binding.
    ///
    /// The hardware commit instant and lease window remain private wrapper
    /// witnesses. Their deadline predicate is authenticated by the release-pinned
    /// proof rather than compared to the public ledger timestamp here.
    ///
    /// # Errors
    ///
    /// Returns an error if finality fails or the receipt differs from the voucher/request.
    pub fn validate_against(
        &self,
        trust_anchor: &KagemushaFinalityTrustAnchorV1,
    ) -> Result<(), KagemushaIsiValidationErrorV1> {
        require_chain_version(self.version)?;
        self.request.validate_shape()?;
        self.finality.validate_against(trust_anchor)?;
        let receipt = &self.finality.reserve_receipt_witness.receipt;
        let statement = &self.request.voucher.statement;
        let lifecycle = self.request.lifecycle();
        if receipt.operation_id != self.request.operation_id
            || receipt.kind != KagemushaOperationKindV1::Redemption
            || receipt.request_digest != self.request.canonical_digest()?
            || receipt.network_id != lifecycle.network_id
            || receipt.asset != lifecycle.asset
            || receipt.asset_incarnation != lifecycle.asset_incarnation
            || receipt.scale != lifecycle.scale
            || receipt.liability_pool_id != lifecycle.liability_pool_id
            || receipt.amount != statement.amount
        {
            return Err(invalid("redemption_result.binding"));
        }
        Ok(())
    }
}

/// Applied result selected by operation kind.
#[expect(
    clippy::large_enum_variant,
    reason = "boxing would change the sole first-release Norito wire shape"
)]
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "kind",
    content = "result",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum KagemushaOperationResultV1 {
    /// Finalized top-up and exact mint credit.
    #[codec(index = 0)]
    TopUp(KagemushaTopUpResultV1),
    /// Finalized full or partial redemption.
    #[codec(index = 1)]
    Redemption(KagemushaRedemptionResultV1),
}

impl KagemushaOperationResultV1 {
    /// Return the immutable operation kind.
    #[must_use]
    pub const fn kind(&self) -> KagemushaOperationKindV1 {
        match self {
            Self::TopUp(_) => KagemushaOperationKindV1::TopUp,
            Self::Redemption(_) => KagemushaOperationKindV1::Redemption,
        }
    }

    /// Return the immutable idempotency key.
    #[must_use]
    pub const fn operation_id(&self) -> [u8; 32] {
        match self {
            Self::TopUp(result) => result.request.operation_id,
            Self::Redemption(result) => result.request.operation_id,
        }
    }

    /// Validate the selected terminal result.
    ///
    /// # Errors
    ///
    /// Returns the selected result's validation failure.
    pub fn validate_against(
        &self,
        trust_anchor: &KagemushaFinalityTrustAnchorV1,
    ) -> Result<(), KagemushaIsiValidationErrorV1> {
        match self {
            Self::TopUp(result) => result.validate_against(trust_anchor),
            Self::Redemption(result) => result.validate_against(trust_anchor),
        }
    }
}

/// Observable lifecycle state of an idempotent public operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "state", content = "value", rename_all = "snake_case")]
pub enum KagemushaOperationStateV1 {
    /// Accepted but not yet finalized.
    #[codec(index = 0)]
    Pending,
    /// Finality and the terminal result are available.
    #[codec(index = 1)]
    Applied,
    /// The operation failed permanently without a reserve mutation.
    #[codec(index = 2)]
    Rejected,
}

/// Stable machine-readable reason for terminal rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "code", content = "value", rename_all = "snake_case")]
pub enum KagemushaOperationRejectionCodeV1 {
    /// The canonical request was malformed or inconsistent.
    #[codec(index = 0)]
    InvalidRequest,
    /// The submitting authority could not debit or credit the requested account.
    #[codec(index = 1)]
    Unauthorized,
    /// The online payer balance was insufficient.
    #[codec(index = 2)]
    InsufficientOnlineBalance,
    /// Recursive, finality, or hardware proof verification failed.
    #[codec(index = 3)]
    InvalidProof,
    /// The hardware policy was absent, unqualified, or mismatched.
    #[codec(index = 4)]
    HardwarePolicyRejected,
    /// An operation, issuance, credit, redemption, or nullifier identity conflicted.
    #[codec(index = 5)]
    IdentityConflict,
    /// The pooled reserve could not cover the redemption.
    #[codec(index = 6)]
    ReserveUnderflow,
    /// Checked `u128` amount or reserve-total arithmetic overflowed.
    #[codec(index = 7)]
    ArithmeticOverflow,
    /// Consensus could not durably complete the operation.
    #[codec(index = 8)]
    InternalFailure,
}

/// Deterministic terminal rejection record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaOperationRejectionV1 {
    /// Stable machine-readable failure class.
    pub code: KagemushaOperationRejectionCodeV1,
    /// Non-zero digest of restricted diagnostics, without unstable free text.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub detail_digest: [u8; 32],
}

/// Response returned by idempotent operation lookup.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaOperationStatusV1 {
    /// Status layout version.
    pub version: u16,
    /// Immutable operation identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub operation_id: [u8; 32],
    /// Immutable operation kind.
    pub kind: KagemushaOperationKindV1,
    /// Current monotonic lifecycle state.
    pub state: KagemushaOperationStateV1,
    /// Present exactly when `state` is `Applied`.
    #[norito(required)]
    pub result: Option<KagemushaOperationResultV1>,
    /// Present exactly when `state` is `Rejected`.
    #[norito(required)]
    pub rejection: Option<KagemushaOperationRejectionV1>,
}

impl KagemushaOperationStatusV1 {
    /// Validate the exact state/result/rejection combination and immutable identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the status shape, result, or identity is inconsistent.
    pub fn validate(&self) -> Result<(), KagemushaIsiValidationErrorV1> {
        require_chain_version(self.version)?;
        require_nonzero("operation_status.operation_id", self.operation_id)?;
        match (&self.state, &self.result, &self.rejection) {
            (KagemushaOperationStateV1::Pending, None, None) => Ok(()),
            (KagemushaOperationStateV1::Applied, Some(_), None) => {
                Err(KagemushaIsiValidationErrorV1::MissingTrustAnchor)
            }
            (KagemushaOperationStateV1::Rejected, None, Some(rejection)) => require_nonzero(
                "operation_status.rejection.detail_digest",
                rejection.detail_digest,
            ),
            _ => Err(KagemushaIsiValidationErrorV1::InvalidStatus),
        }
    }

    /// Validate status and authenticate an applied result against an externally pinned context.
    ///
    /// Pending and rejected statuses retain the same structural checks. Applied
    /// status additionally performs full certificate and receipt verification;
    /// the response can never select its own trust root.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid status shape, terminal result, identity,
    /// or finality proof relative to `trust_anchor`.
    pub fn validate_against(
        &self,
        trust_anchor: &KagemushaFinalityTrustAnchorV1,
    ) -> Result<(), KagemushaIsiValidationErrorV1> {
        require_chain_version(self.version)?;
        require_nonzero("operation_status.operation_id", self.operation_id)?;
        match (&self.state, &self.result, &self.rejection) {
            (KagemushaOperationStateV1::Pending, None, None) => Ok(()),
            (KagemushaOperationStateV1::Applied, Some(result), None) => {
                result.validate_against(trust_anchor)?;
                if result.kind() != self.kind || result.operation_id() != self.operation_id {
                    return Err(KagemushaIsiValidationErrorV1::InvalidStatus);
                }
                Ok(())
            }
            (KagemushaOperationStateV1::Rejected, None, Some(rejection)) => require_nonzero(
                "operation_status.rejection.detail_digest",
                rejection.detail_digest,
            ),
            _ => Err(KagemushaIsiValidationErrorV1::InvalidStatus),
        }
    }
}

/// Canonical operation lookup selector used by `/v1/kagemusha/operations/{operation_id}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaOperationLookupV1 {
    /// Lookup layout version.
    pub version: u16,
    /// Exact idempotency key selected by the route.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub operation_id: [u8; 32],
}

impl KagemushaOperationLookupV1 {
    /// Validate the sole lookup key.
    ///
    /// # Errors
    ///
    /// Returns an error for a wrong version or zero operation ID.
    pub fn validate(&self) -> Result<(), KagemushaIsiValidationErrorV1> {
        require_chain_version(self.version)?;
        require_nonzero("operation_lookup.operation_id", self.operation_id)
    }
}

isi! {
    /// Atomically debit online funds, increase the pooled reserve, and accept one fixed issuance.
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    pub struct TopUpKagemushaV1 {
        /// Complete deterministic pre-finality issuance intent.
        pub request: KagemushaTopUpRequestV1,
    }
}

isi! {
    /// Verify and settle one full or partial hardware-bound redemption voucher.
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    pub struct RedeemKagemushaV1 {
        /// Complete terminal redemption request.
        pub request: KagemushaRedemptionRequestV1,
    }
}

impl crate::seal::Instruction for TopUpKagemushaV1 {}
impl crate::seal::Instruction for RedeemKagemushaV1 {}

fn kagemusha_instruction_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_kagemusha_instruction_decode_from_slice {
    ($ty:ty, $request:ty) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = kagemusha_instruction_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }
                let mut offset = 0_usize;
                let request = super::decode_aos_canonical_field::<$request>(
                    super::read_aos_field(bytes, &mut offset, flags)?,
                    flags,
                )?;
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { request }, offset))
            }
        }
    };
}

impl_kagemusha_instruction_decode_from_slice!(TopUpKagemushaV1, KagemushaTopUpRequestV1);
impl_kagemusha_instruction_decode_from_slice!(
    RedeemKagemushaV1,
    KagemushaRedemptionRequestV1
);

impl TopUpKagemushaV1 {
    /// Canonical first-release instruction wire identifier.
    pub const WIRE_ID: &'static str = "iroha.kagemusha.v1.top_up";

    /// Construct a top-up instruction from a structurally valid deterministic intent.
    ///
    /// Construction does not grant monetary authority. Core must authenticate
    /// the request against the enabled release and hardware profile before
    /// executing the debit and reserve credit.
    ///
    /// # Errors
    ///
    /// Returns an error when the intent is invalid.
    pub fn new(
        request: KagemushaTopUpRequestV1,
    ) -> Result<Self, KagemushaIsiValidationErrorV1> {
        request.validate_shape()?;
        Ok(Self { request })
    }

    /// Validate the embedded deterministic top-up intent's shape.
    ///
    /// # Errors
    ///
    /// Returns the request validation failure.
    pub fn validate_shape(&self) -> Result<(), KagemushaIsiValidationErrorV1> {
        self.request.validate_shape()
    }
}

impl RedeemKagemushaV1 {
    /// Canonical first-release instruction wire identifier.
    pub const WIRE_ID: &'static str = "iroha.kagemusha.v1.redeem";

    /// Construct a redemption instruction from a verified-shape terminal request.
    ///
    /// # Errors
    ///
    /// Returns an error when the request or voucher is invalid.
    pub fn new(
        request: KagemushaRedemptionRequestV1,
    ) -> Result<Self, KagemushaIsiValidationErrorV1> {
        request.validate_shape()?;
        Ok(Self { request })
    }

    /// Validate the embedded full or partial redemption request.
    ///
    /// # Errors
    ///
    /// Returns the request validation failure.
    pub fn validate_shape(&self) -> Result<(), KagemushaIsiValidationErrorV1> {
        self.request.validate_shape()
    }
}

fn require_chain_version(version: u16) -> Result<(), KagemushaIsiValidationErrorV1> {
    if version != KAGEMUSHA_CHAIN_VERSION_V1 {
        return Err(KagemushaIsiValidationErrorV1::UnsupportedVersion { actual: version });
    }
    Ok(())
}

fn require_nonzero(
    field: &'static str,
    value: [u8; 32],
) -> Result<(), KagemushaIsiValidationErrorV1> {
    if value == [0; 32] {
        return Err(invalid(field));
    }
    Ok(())
}

fn invalid(field: &'static str) -> KagemushaIsiValidationErrorV1 {
    KagemushaIsiValidationErrorV1::InvalidField { field }
}

fn wire_error(error: impl core::fmt::Display) -> KagemushaIsiValidationErrorV1 {
    KagemushaIsiValidationErrorV1::InvalidWire(error.to_string())
}

fn digest_encoded<T: Encode>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], KagemushaIsiValidationErrorV1> {
    let bytes = norito::encode_canonical(value)
        .map_err(|error| KagemushaIsiValidationErrorV1::Encoding(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
    Ok(hasher.finalize().into())
}

fn ordinary_smt_node_hash(left: Hash, right: Hash) -> Hash {
    let mut preimage = Vec::with_capacity(1 + 2 * Hash::LENGTH);
    preimage.push(1);
    preimage.extend_from_slice(left.as_ref());
    preimage.extend_from_slice(right.as_ref());
    Hash::new(preimage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        block::BlockHeader,
        domain::DomainId,
        kagemusha::{
            KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
            KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1,
            KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1,
            KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1, KagemushaDevicePublicKeyV1,
            KagemushaDeviceSignatureV1, KagemushaHardwarePlatformClassV1,
            KagemushaPairedProofV1, kagemusha_credit_opening_canonical_len_v1,
            kagemusha_device_key_reference_v1, kagemusha_suite_commitment_v1,
        },
    };
    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"kagemusha-v1-isi",
        )))
    }

    fn asset() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("asset name"),
        )
    }

    fn asset_incarnation(seed: u8) -> AxtAssetIncarnationV1 {
        let bytes = *Hash::new([seed]).as_ref();
        AxtAssetIncarnationV1::try_from_bytes(bytes).expect("canonical asset incarnation")
    }

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes((&[seed; 32]).into()).expect("P-256 signing key")
    }

    fn public_key(key: &SigningKey) -> KagemushaDevicePublicKeyV1 {
        KagemushaDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .expect("public key")
    }

    fn sign(key: &SigningKey, bytes: &[u8]) -> KagemushaDeviceSignatureV1 {
        let signature: Signature = key.sign(bytes);
        let signature = signature.normalize_s().unwrap_or(signature);
        KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
            .expect("canonical signature")
    }

    const fn suite_id() -> [u8; 32] {
        [0x10; 32]
    }

    fn hardware_profile() -> KagemushaHardwareProfileV1 {
        KagemushaHardwareProfileV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
            hardware_profile_id: [0; 32],
            provider_id: [0x21; 32],
            platform_class: KagemushaHardwarePlatformClassV1::DedicatedSecureElement,
            product_class_digest: [0x22; 32],
            firmware_policy_digest: [0x23; 32],
            enrollment_attestation_verifier_digest: [0x24; 32],
            attestation_trust_roots_digest: [0x25; 32],
            allowed_suite_commitment: kagemusha_suite_commitment_v1(suite_id()),
            policy_epoch: 1,
            governance_credential_public_key: public_key(&signing_key(0x31)),
            capability_mask: KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
            qualification_report_digest: [0x26; 32],
            valid_from_ms: 1,
            expires_at_ms: 100_000,
        }
        .seal_hardware_profile_id()
        .expect("hardware profile id")
    }

    fn hardware_credential(
        profile: &KagemushaHardwareProfileV1,
    ) -> KagemushaHardwareCredentialV1 {
        let device_public_key = public_key(&signing_key(7));
        let governance_key = signing_key(0x31);
        let mut credential = KagemushaHardwareCredentialV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            credential_id: [0; 32],
            network_id: network(),
            hardware_profile_id: profile.hardware_profile_id,
            suite_id: suite_id(),
            firmware_policy_digest: profile.firmware_policy_digest,
            policy_epoch: profile.policy_epoch,
            lane_commitment: [0x32; 32],
            hardware_epoch_id: [0x33; 32],
            hardware_epoch_generation: 1,
            device_public_key,
            device_key_reference: kagemusha_device_key_reference_v1(&device_public_key),
            issued_at_ms: 500,
            expires_at_ms: 90_000,
            governance_signature: sign(&governance_key, b"placeholder"),
        }
        .seal_credential_id()
        .expect("credential id");
        credential.governance_signature = sign(
            &governance_key,
            &credential
                .canonical_signing_bytes()
                .expect("credential signing bytes"),
        );
        credential
            .validate_against_profile(profile)
            .expect("credential/profile binding");
        credential
    }

    fn encrypted_credit_fixture(recipient_one_time_key: [u8; 32], tag: u8) -> Vec<u8> {
        let mut ephemeral_x25519_public_key = [0; 32];
        ephemeral_x25519_public_key[0] = 9;
        KagemushaEncryptedCreditEnvelopeV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            ephemeral_x25519_public_key,
            nonce: [tag; KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1],
            ciphertext_and_tag: vec![
                tag;
                kagemusha_credit_opening_canonical_len_v1()
                    .expect("opening length")
                    + KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1
            ],
        }
        .canonical_bytes_against_recipient_key(recipient_one_time_key)
        .expect("canonical encrypted credit fixture")
    }

    fn attach_test_mint_authorization(
        mut request: KagemushaTopUpRequestV1,
    ) -> KagemushaTopUpRequestV1 {
        request.mint_authorization = None;
        let statement = request
            .mint_authorization_statement()
            .expect("mint authorization statement");
        let semantic_digest = statement
            .canonical_digest()
            .expect("mint authorization semantic digest");
        request
            .attach_mint_authorization(KagemushaMintAuthorizationV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                statement,
                proof: KagemushaPairedProofV1 {
                    version: KAGEMUSHA_WIRE_VERSION_V1,
                    eq_protocol_digest: [0x42; 32],
                    ep_protocol_digest: [0x43; 32],
                    semantic_digest,
                    guard_eq_credential_audit: [0x44; 32],
                    guard_ep_credential_audit: [0x45; 32],
                    eq_deferred_audit: [0x46; 32],
                    ep_deferred_audit: [0x47; 32],
                    eq_proof: vec![0xA1; 128],
                    ep_proof: vec![0xB2; 128],
                    eq_history: vec![0xC3; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
                    ep_history: vec![0xD4; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
                },
            })
            .expect("attach mint authorization")
    }

    fn top_up_request() -> KagemushaTopUpRequestV1 {
        let profile = hardware_profile();
        let network_id = network();
        let asset = asset();
        let asset_incarnation = asset_incarnation(1);
        let recipient_one_time_key = [0x41; 32];
        let request = KagemushaTopUpRequestV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            operation_id: [1; 32],
            issuance_commitment: [0; 32],
            credit_id: [0; 32],
            release_id: [2; 32],
            suite_id: suite_id(),
            vk_digest: [0x11; 32],
            network_id,
            liability_pool_id: kagemusha_liability_pool_id_v1(
                &network_id,
                &asset,
                asset_incarnation,
            )
            .expect("liability pool"),
            asset,
            asset_incarnation,
            scale: 4,
            amount: 50_000,
            payer: account(0xA5),
            recipient: account(0xB6),
            hardware_credential: hardware_credential(&profile),
            recipient_credential_commitment: [3; 32],
            credit_commitment: [6; 32],
            recipient_one_time_key,
            encrypted_credit: encrypted_credit_fixture(recipient_one_time_key, 0x91),
            artifact_manifest_digest: [7; 32],
            mint_authorization: None,
        }
        .seal_identifiers()
        .expect("seal top-up identifiers");
        attach_test_mint_authorization(request)
    }

    fn redemption_request() -> KagemushaRedemptionRequestV1 {
        let network_id = network();
        let asset = asset();
        let asset_incarnation = asset_incarnation(1);
        let lifecycle = crate::kagemusha::KagemushaLifecycleBindingV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            network_id,
            protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
            suite_id: [0x10; 32],
            vk_digest: [0x11; 32],
            release_id: [1; 32],
            asset: asset.clone(),
            asset_incarnation,
            scale: 4,
            liability_pool_id: kagemusha_liability_pool_id_v1(
                &network_id,
                &asset,
                asset_incarnation,
            )
            .expect("liability pool"),
            hardware_profile_id: [0x12; 32],
            policy_epoch: 1,
            operation_kind: crate::kagemusha::KagemushaOperationKindV1::RedeemSplit,
            request_id: [0; 32],
            acceptance_ticket_id: [0; 32],
            credit_id: [0; 32],
            ciphertext_digest: [0; 32],
        };
        let commit_evidence = crate::kagemusha::KagemushaCommitEvidenceV1::TrustedTime(
            crate::kagemusha::KagemushaTrustedCommitTimeV1 {
                time_evidence_commitment: [0x13; 32],
            },
        );
        let statement = crate::kagemusha::KagemushaRedemptionStatementV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            lifecycle,
            amount: 12_000,
            beneficiary: account(0xB6),
            terminal_nullifier: [0x14; 32],
            redemption_commitment: [7; 32],
            redemption_id: [0; 32],
            commit_evidence,
        }
        .seal_redemption_id()
        .expect("seal redemption identity");
        let outbox_reservation = crate::kagemusha::KagemushaOutboxReservationV1 {
            reservation_id: [0x15; 32],
            operation_kind: crate::kagemusha::KagemushaOperationKindV1::RedeemSplit,
            reserved_outbox_bytes: crate::kagemusha::KAGEMUSHA_REDEMPTION_OUTBOX_MIN_BYTES_V1,
            issued_at_ms: 8_000,
            expires_at_ms: 10_000,
        };
        let unsealed_certificate = crate::kagemusha::KagemushaCommitCertificateV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            certificate_id: [0; 32],
            candidate_envelope_digest: [0x16; 32],
            lifecycle_binding_digest: statement
                .lifecycle
                .canonical_digest()
                .expect("lifecycle digest"),
            transition_nullifier: statement.terminal_nullifier,
            outbox_reservation_commitment: outbox_reservation
                .canonical_commitment()
                .expect("outbox reservation commitment"),
            commit_evidence,
            hardware_profile_id: statement.lifecycle.hardware_profile_id,
            policy_epoch: statement.lifecycle.policy_epoch,
            hardware_terminal_commitment: [0; 32],
        };
        let terminal_body = crate::kagemusha::KagemushaHardwareTerminalBodyV1 {
            version: unsealed_certificate.version,
            candidate_envelope_digest: unsealed_certificate.candidate_envelope_digest,
            lifecycle_binding_digest: unsealed_certificate.lifecycle_binding_digest,
            transition_nullifier: unsealed_certificate.transition_nullifier,
            outbox_reservation_commitment: unsealed_certificate.outbox_reservation_commitment,
            commit_evidence: unsealed_certificate.commit_evidence,
            hardware_profile_id: unsealed_certificate.hardware_profile_id,
            policy_epoch: unsealed_certificate.policy_epoch,
            private_successor_commitment: [0x17; 32],
            private_journal_commitment: [0x18; 32],
            private_recovery_commitment: [0x19; 32],
        };
        let commit_certificate = unsealed_certificate
            .seal_with_terminal_body(&terminal_body)
            .expect("seal commit certificate");
        let semantic_digest = statement
            .canonical_digest()
            .expect("redemption semantic digest");
        let commit_certificate_digest = digest_encoded(
            b"iroha:kagemusha:v1:commit-certificate",
            &commit_certificate,
        )
        .expect("commit certificate digest");
        let voucher = KagemushaRedemptionVoucherV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            statement,
            proof: crate::kagemusha::KagemushaCommitWrapperProofV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                eq_protocol_digest: [0x1A; 32],
                ep_protocol_digest: [0x1B; 32],
                semantic_digest,
                candidate_envelope_digest: commit_certificate.candidate_envelope_digest,
                commit_certificate_digest,
                eq_deferred_audit: [0x1C; 32],
                ep_deferred_audit: [0x1D; 32],
                eq_proof: vec![0xA1; 128],
                ep_proof: vec![0xB2; 128],
                eq_history: vec![0xC3; crate::kagemusha::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
                ep_history: vec![0xD4; crate::kagemusha::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
            },
            commit_certificate,
            artifact_manifest_digest: [8; 32],
        };
        KagemushaRedemptionRequestV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            operation_id: [9; 32],
            voucher,
        }
    }

    fn reserve_receipt(kind: KagemushaOperationKindV1) -> KagemushaReserveReceiptV1 {
        let network_id = network();
        let asset = asset();
        let asset_incarnation = asset_incarnation(1);
        KagemushaReserveReceiptV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            operation_id: [1; 32],
            kind,
            request_digest: [2; 32],
            mint_statement_digest: if kind == KagemushaOperationKindV1::TopUp {
                [4; 32]
            } else {
                [0; 32]
            },
            network_id,
            liability_pool_id: kagemusha_liability_pool_id_v1(
                &network_id,
                &asset,
                asset_incarnation,
            )
            .expect("liability pool"),
            asset,
            asset_incarnation,
            scale: 4,
            amount: 50,
            previous_pool_receipt_digest: [0x20; 32],
            total_topups: 100,
            total_redemptions: if kind == KagemushaOperationKindV1::Redemption {
                50
            } else {
                0
            },
            transaction_hash: [3; 32],
            committed_at_ms: 10_000,
        }
    }

    #[test]
    fn top_up_identifiers_are_deterministic_and_tamper_evident() {
        let request = top_up_request();
        let profile = hardware_profile();
        request
            .validate_shape()
            .expect("valid top-up request shape");
        request
            .validate_against_profile(&profile)
            .expect("authenticated top-up profile");
        assert_ne!(request.payer, request.recipient);
        assert_eq!(
            request.issuance_commitment,
            request
                .expected_issuance_commitment()
                .expect("issuance commitment")
        );
        assert_eq!(
            request.credit_id,
            request.expected_credit_id().expect("credit id")
        );
        assert_eq!(
            request
                .mint_statement_against_profile(&profile, 10_000)
                .expect("mint statement")
                .recipient,
            request.recipient,
            "the credited owner is independent of the debited payer"
        );

        let mut retargeted = request.clone();
        retargeted.recipient = account(0xC7);
        assert_ne!(
            retargeted
                .expected_issuance_commitment()
                .expect("retargeted issuance commitment"),
            request.issuance_commitment
        );
        assert_ne!(
            retargeted
                .expected_credit_id()
                .expect("retargeted credit id"),
            request.credit_id
        );
        assert!(retargeted.validate_shape().is_err());

        let mut tampered = request;
        tampered.amount += 1;
        assert!(tampered.validate_shape().is_err());
    }

    #[test]
    fn top_up_binds_the_full_credential_and_rejects_profile_substitution() {
        let profile = hardware_profile();
        let request = top_up_request();

        let governance_key = signing_key(0x31);
        let mut substituted_credential = request.hardware_credential;
        substituted_credential.lane_commitment = [0x72; 32];
        substituted_credential = substituted_credential
            .seal_credential_id()
            .expect("substituted credential id");
        substituted_credential.governance_signature = sign(
            &governance_key,
            &substituted_credential
                .canonical_signing_bytes()
                .expect("substituted credential signing bytes"),
        );
        substituted_credential
            .validate_against_profile(&profile)
            .expect("structurally legitimate alternate credential");

        let mut substituted = request.clone();
        substituted.hardware_credential = substituted_credential;
        assert_ne!(
            substituted
                .expected_issuance_commitment()
                .expect("substituted issuance commitment"),
            request.issuance_commitment,
            "the complete credential, including its issuer signature, is committed"
        );
        assert!(substituted.validate_shape().is_err());
        let resealed = substituted
            .seal_identifiers()
            .expect("reseal alternate credential request");
        assert_ne!(resealed.credit_id, request.credit_id);

        let mut fake_profile = profile;
        fake_profile.provider_id = [0x73; 32];
        fake_profile = fake_profile
            .seal_hardware_profile_id()
            .expect("fake profile id");
        fake_profile
            .validate()
            .expect("self-consistent fake profile");
        assert!(request.validate_against_profile(&fake_profile).is_err());
    }

    #[test]
    fn mint_lifecycle_binds_randomized_credential_and_ciphertext() {
        let profile = hardware_profile();
        let request = top_up_request();
        let statement = request
            .mint_statement_against_profile(&profile, 10_000)
            .expect("authenticated mint statement");
        assert_eq!(
            statement.lifecycle.operation_kind,
            crate::kagemusha::KagemushaOperationKindV1::MintFold
        );
        assert_eq!(statement.lifecycle.suite_id, request.suite_id);
        assert_eq!(statement.lifecycle.vk_digest, request.vk_digest);
        assert_eq!(
            statement.lifecycle.hardware_profile_id,
            request.hardware_credential.hardware_profile_id
        );
        assert_eq!(
            statement.recipient_credential_commitment,
            request.recipient_credential_commitment
        );
        assert_eq!(
            statement.lifecycle.ciphertext_digest,
            kagemusha_ciphertext_digest_v1(&request.encrypted_credit)
        );

        let mut changed_commitment = request.clone();
        changed_commitment.recipient_credential_commitment = [0x74; 32];
        let changed_commitment = changed_commitment
            .seal_identifiers()
            .expect("reseal randomized commitment");
        assert_ne!(
            changed_commitment.issuance_commitment,
            request.issuance_commitment
        );
        assert_ne!(changed_commitment.credit_id, request.credit_id);

        let original_request_digest = request.canonical_digest().expect("request digest");
        let mut changed_ciphertext = request.clone();
        let mut changed_envelope =
            KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
                &changed_ciphertext.encrypted_credit,
                changed_ciphertext.recipient_one_time_key,
            )
            .expect("decode encrypted credit");
        changed_envelope.ciphertext_and_tag[0] ^= 1;
        changed_ciphertext.encrypted_credit = changed_envelope
            .canonical_bytes_against_recipient_key(changed_ciphertext.recipient_one_time_key)
            .expect("re-encode changed encrypted credit");
        assert_eq!(
            changed_ciphertext
                .expected_issuance_commitment()
                .expect("changed ciphertext issuance commitment"),
            request.issuance_commitment
        );
        assert_eq!(
            changed_ciphertext
                .expected_credit_id()
                .expect("changed ciphertext credit id"),
            request.credit_id
        );
        let changed_ciphertext = attach_test_mint_authorization(changed_ciphertext);
        changed_ciphertext
            .validate_shape()
            .expect("pre-encryption identities remain valid");
        assert_ne!(
            changed_ciphertext
                .canonical_digest()
                .expect("changed request digest"),
            original_request_digest,
            "reserve finality binds the exact encrypted bytes"
        );
        let changed_statement = changed_ciphertext
            .mint_statement_against_profile(&profile, 10_000)
            .expect("changed ciphertext statement");
        assert_eq!(
            changed_statement.lifecycle.ciphertext_digest,
            kagemusha_ciphertext_digest_v1(&changed_ciphertext.encrypted_credit)
        );
        assert_ne!(
            changed_statement.lifecycle.ciphertext_digest,
            statement.lifecycle.ciphertext_digest
        );
        assert_eq!(changed_ciphertext.credit_id, request.credit_id);
    }

    #[test]
    fn top_up_requires_exact_recipient_authorization_before_admission() {
        let request = top_up_request();
        request.validate_shape().expect("exact authorization");

        let mut missing = request.clone();
        missing.mint_authorization = None;
        assert!(missing.validate_shape().is_err());
        assert!(missing.canonical_digest().is_err());

        let mut forged_statement = request.clone();
        forged_statement
            .mint_authorization
            .as_mut()
            .expect("authorization")
            .statement
            .credit_id = [0xFE; 32];
        assert!(forged_statement.validate_shape().is_err());

        let mut forged_proof = request;
        forged_proof
            .mint_authorization
            .as_mut()
            .expect("authorization")
            .proof
            .semantic_digest = [0xFD; 32];
        assert!(forged_proof.validate_shape().is_err());
    }

    #[test]
    fn top_up_identity_is_constructible_before_credit_id_bound_aead() {
        let mut draft = top_up_request();
        draft.issuance_commitment = [0; 32];
        draft.credit_id = [0; 32];
        draft.encrypted_credit.clear();

        let mut identified = draft
            .seal_pre_encryption_identifiers()
            .expect("pre-encryption identifiers");
        assert_ne!(identified.issuance_commitment, [0; 32]);
        assert_ne!(identified.credit_id, [0; 32]);

        identified.encrypted_credit =
            encrypted_credit_fixture(identified.recipient_one_time_key, 0x92);
        let identified = attach_test_mint_authorization(identified);
        identified
            .validate_shape()
            .expect("AEAD may bind the already-derived credit id");
    }

    #[test]
    fn finality_time_changes_statement_but_not_credit_identity() {
        let request = top_up_request();
        let profile = hardware_profile();
        let first = request
            .mint_statement_against_profile(&profile, 10_000)
            .expect("first finalized mint statement");
        let later = request
            .mint_statement_against_profile(&profile, 20_000)
            .expect("later finalized mint statement");

        assert_eq!(first.lifecycle.credit_id, request.credit_id);
        assert_eq!(later.lifecycle.credit_id, request.credit_id);
        assert_eq!(
            first.expected_credit_id().expect("first credit id"),
            request.credit_id
        );
        assert_eq!(
            later.expected_credit_id().expect("later credit id"),
            request.credit_id
        );
        assert_ne!(
            first.canonical_digest().expect("first statement digest"),
            later.canonical_digest().expect("later statement digest"),
            "the recursive mint proof still binds the authoritative commit time"
        );
    }

    #[test]
    fn top_up_and_reserve_pool_are_bound_to_asset_incarnation() {
        let request = top_up_request();
        let mut next_incarnation = request.clone();
        next_incarnation.asset_incarnation = asset_incarnation(2);
        next_incarnation.liability_pool_id = kagemusha_liability_pool_id_v1(
            &next_incarnation.network_id,
            &next_incarnation.asset,
            next_incarnation.asset_incarnation,
        )
        .expect("next-incarnation pool");
        next_incarnation = next_incarnation
            .seal_identifiers()
            .expect("seal next-incarnation top-up");

        assert_ne!(
            request.liability_pool_id,
            next_incarnation.liability_pool_id
        );
        assert_ne!(
            request.issuance_commitment,
            next_incarnation.issuance_commitment
        );
        assert_ne!(request.credit_id, next_incarnation.credit_id);

        let mut mismatched_receipt = reserve_receipt(KagemushaOperationKindV1::TopUp);
        mismatched_receipt.asset_incarnation = asset_incarnation(2);
        assert_eq!(
            mismatched_receipt.validate(),
            Err(invalid("reserve_receipt.liability_pool_id"))
        );
    }

    #[test]
    fn redemption_exposes_only_lifecycle_and_terminal_conflict_key() {
        let request = redemption_request();
        request
            .validate_shape()
            .expect("valid wrapped redemption shape");
        assert_eq!(
            request.lifecycle().operation_kind,
            crate::kagemusha::KagemushaOperationKindV1::RedeemSplit
        );
        assert_eq!(
            request.terminal_nullifier(),
            request.voucher.statement.terminal_nullifier
        );

        let mut rebound = request;
        rebound.voucher.commit_certificate.candidate_envelope_digest[0] ^= 1;
        assert!(rebound.validate_shape().is_err());
    }

    #[test]
    fn requests_and_receipts_roundtrip_canonically() {
        let top_up = top_up_request();
        let encoded = norito::encode_canonical(&top_up).expect("encode top-up");
        let decoded: KagemushaTopUpRequestV1 =
            norito::decode_canonical(&encoded).expect("decode top-up");
        assert_eq!(decoded, top_up);
        decoded.validate_shape().expect("decoded top-up validates");

        let redemption = redemption_request();
        redemption.validate_shape().expect("valid redemption shape");
        let encoded = norito::encode_canonical(&redemption).expect("encode redemption");
        let decoded: KagemushaRedemptionRequestV1 =
            norito::decode_canonical(&encoded).expect("decode redemption");
        assert_eq!(decoded, redemption);

        let receipt = reserve_receipt(KagemushaOperationKindV1::TopUp);
        let encoded = norito::encode_canonical(&receipt).expect("encode receipt");
        let decoded: KagemushaReserveReceiptV1 =
            norito::decode_canonical(&encoded).expect("decode receipt");
        assert_eq!(decoded, receipt);
        assert_eq!(decoded.available().expect("available reserve"), 100);
    }

    #[test]
    fn reserve_receipt_uses_checked_conservation() {
        let mut receipt = reserve_receipt(KagemushaOperationKindV1::Redemption);
        receipt.total_redemptions = receipt.total_topups + 1;
        assert_eq!(
            receipt.validate(),
            Err(KagemushaIsiValidationErrorV1::ReserveUnderflow)
        );

        let mut receipt = reserve_receipt(KagemushaOperationKindV1::TopUp);
        receipt.total_topups = receipt.amount - 1;
        assert_eq!(
            receipt.validate(),
            Err(invalid("reserve_receipt.total_topups"))
        );
    }

    #[test]
    fn reserve_receipt_chain_uses_an_opaque_cas_head_without_a_revision() {
        let mut first = reserve_receipt(KagemushaOperationKindV1::TopUp);
        first.previous_pool_receipt_digest = [0; 32];
        first.total_topups = first.amount;
        first.total_redemptions = 0;
        first
            .validate_against_pool_head(None)
            .expect("canonical first top-up");
        first
            .validate_against_previous_receipt(None)
            .expect("first top-up derives from zero totals");
        let first_head = first.canonical_digest().expect("first pool head");
        assert_ne!(first_head, [0; 32]);

        let mut next = reserve_receipt(KagemushaOperationKindV1::TopUp);
        next.previous_pool_receipt_digest = first_head;
        next.validate_against_pool_head(Some(first_head))
            .expect("matching pool CAS head");
        next.validate_against_previous_receipt(Some(&first))
            .expect("exact top-up delta from prior receipt");
        assert!(next.validate_against_pool_head(Some([0x21; 32])).is_err());

        let mut invalid_first = first;
        invalid_first.previous_pool_receipt_digest = [0x22; 32];
        assert_eq!(
            invalid_first.validate(),
            Err(invalid("reserve_receipt.previous_pool_receipt_digest"))
        );

        let mut unchained_redemption = reserve_receipt(KagemushaOperationKindV1::Redemption);
        unchained_redemption.previous_pool_receipt_digest = [0; 32];
        assert_eq!(
            unchained_redemption.validate(),
            Err(invalid("reserve_receipt.previous_pool_receipt_digest"))
        );
    }

    #[test]
    fn reserve_receipt_authority_rejects_forged_deltas_and_predecessors() {
        let mut first = reserve_receipt(KagemushaOperationKindV1::TopUp);
        first.previous_pool_receipt_digest = [0; 32];
        first.total_topups = first.amount;
        first.total_redemptions = 0;
        first
            .validate_against_previous_receipt(None)
            .expect("canonical first receipt");

        let mut top_up = reserve_receipt(KagemushaOperationKindV1::TopUp);
        top_up.previous_pool_receipt_digest = first.canonical_digest().expect("first digest");
        top_up
            .validate_against_previous_receipt(Some(&first))
            .expect("authoritative top-up delta");

        let mut underreported = top_up.clone();
        underreported.total_topups -= 1;
        assert_eq!(
            underreported.validate_against_previous_receipt(Some(&first)),
            Err(invalid("reserve_receipt.top_up_delta"))
        );

        let mut overreported = top_up.clone();
        overreported.total_topups += 1;
        assert_eq!(
            overreported.validate_against_previous_receipt(Some(&first)),
            Err(invalid("reserve_receipt.top_up_delta"))
        );

        let mut inflated_unchanged_total = top_up.clone();
        inflated_unchanged_total.total_redemptions += 1;
        assert_eq!(
            inflated_unchanged_total.validate_against_previous_receipt(Some(&first)),
            Err(invalid("reserve_receipt.top_up_delta"))
        );

        let mut other_previous = first.clone();
        other_previous.operation_id = [0x91; 32];
        assert_eq!(
            top_up.validate_against_previous_receipt(Some(&other_previous)),
            Err(invalid("reserve_receipt.previous_pool_receipt_digest"))
        );

        let mut mismatched_context = top_up.clone();
        mismatched_context.scale += 1;
        assert_eq!(
            mismatched_context.validate_against_previous_receipt(Some(&first)),
            Err(invalid("reserve_receipt.pool_context"))
        );

        let mut redemption = reserve_receipt(KagemushaOperationKindV1::Redemption);
        redemption.previous_pool_receipt_digest = top_up.canonical_digest().expect("top-up digest");
        redemption
            .validate_against_previous_receipt(Some(&top_up))
            .expect("authoritative redemption delta");

        let mut inflated_redemptions = redemption.clone();
        inflated_redemptions.total_redemptions += 1;
        assert_eq!(
            inflated_redemptions.validate_against_previous_receipt(Some(&top_up)),
            Err(invalid("reserve_receipt.redemption_delta"))
        );

        let mut inflated_topups = redemption;
        inflated_topups.total_topups += 1;
        assert_eq!(
            inflated_topups.validate_against_previous_receipt(Some(&top_up)),
            Err(invalid("reserve_receipt.redemption_delta"))
        );
    }

    #[test]
    fn reserve_receipt_authority_uses_checked_addition() {
        let mut saturated = reserve_receipt(KagemushaOperationKindV1::TopUp);
        saturated.amount = u128::MAX;
        saturated.previous_pool_receipt_digest = [0; 32];
        saturated.total_topups = u128::MAX;
        saturated.total_redemptions = 0;
        saturated
            .validate_against_previous_receipt(None)
            .expect("saturated canonical first receipt");

        let mut overflowing = reserve_receipt(KagemushaOperationKindV1::TopUp);
        overflowing.amount = 1;
        overflowing.previous_pool_receipt_digest =
            saturated.canonical_digest().expect("saturated digest");
        overflowing.total_topups = u128::MAX;
        overflowing.total_redemptions = 0;
        assert_eq!(
            overflowing.validate_against_previous_receipt(Some(&saturated)),
            Err(invalid("reserve_receipt.total_topups"))
        );
    }

    #[test]
    fn receipt_witness_requires_exact_key_and_256_levels() {
        let receipt = reserve_receipt(KagemushaOperationKindV1::TopUp);
        let witness = KagemushaReserveReceiptWitnessV1 {
            key: KagemushaReserveReceiptWitnessV1::expected_key(receipt.operation_id),
            receipt,
            siblings: vec![Hash::new([]); KAGEMUSHA_RESERVE_RECEIPT_WITNESS_SIBLINGS_V1],
        };
        let root = witness.reconstructed_root().expect("reconstruct root");
        assert!(witness.verify(root));

        let mut wrong_key = witness.clone();
        wrong_key.key[0] ^= 1;
        assert!(!wrong_key.verify(root));

        let mut short = witness;
        short.siblings.pop();
        assert!(!short.verify(root));
    }

    #[test]
    fn operation_status_shape_is_exact() {
        let pending = KagemushaOperationStatusV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            operation_id: [1; 32],
            kind: KagemushaOperationKindV1::TopUp,
            state: KagemushaOperationStateV1::Pending,
            result: None,
            rejection: None,
        };
        pending.validate().expect("valid pending status");

        let rejected = KagemushaOperationStatusV1 {
            state: KagemushaOperationStateV1::Rejected,
            rejection: Some(KagemushaOperationRejectionV1 {
                code: KagemushaOperationRejectionCodeV1::ReserveUnderflow,
                detail_digest: [2; 32],
            }),
            ..pending.clone()
        };
        rejected.validate().expect("valid rejected status");

        let invalid = KagemushaOperationStatusV1 {
            state: KagemushaOperationStateV1::Pending,
            rejection: rejected.rejection,
            ..pending
        };
        assert_eq!(
            invalid.validate(),
            Err(KagemushaIsiValidationErrorV1::InvalidStatus)
        );
    }

    #[test]
    fn instruction_registry_uses_only_kagemusha_v1_ids() {
        let top_up = TopUpKagemushaV1::new(top_up_request()).expect("top-up instruction");
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<TopUpKagemushaV1>(TopUpKagemushaV1::WIRE_ID)
            .register_with_id_slice::<RedeemKagemushaV1>(RedeemKagemushaV1::WIRE_ID);
        let (payload, flags) = norito::codec::encode_with_header_flags(&top_up);
        let framed =
            norito::core::frame_bare_with_header_flags::<TopUpKagemushaV1>(&payload, flags)
                .expect("frame top-up instruction");
        let decoded = crate::isi::InstructionRegistry::decode(
            &registry,
            TopUpKagemushaV1::WIRE_ID,
            &framed,
        )
        .expect("registered wire id")
        .expect("decode instruction");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }
}
