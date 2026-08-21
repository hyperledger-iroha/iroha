//! Fail-closed Kagemusha V4 validator qualification and activation receipts.
//!
//! These types close the release-record publication cycle without claiming to
//! operate a validator or signer. A caller must separately collect four
//! validator signatures, submit the exact governed activation transaction, and
//! capture its result-bearing finalized block. Validation binds those artifacts
//! together; it never creates, publishes, submits, or activates them.

// TODO: Wire these validators to same-read host evidence capture, validator and
// receipt signers, governed submission, and finality collection only after
// those typed runtime APIs exist; accepting caller-asserted substitutes here
// would recreate the publication gap this module is meant to close.

use crate::{
    NetworkId,
    account::{AccountId, MultisigPolicy},
    block::{
        SignedBlock,
        consensus_v2::HeightContextId,
        decode_framed_signed_block,
        proofs::{AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1, TrustedBlockProofAnchor},
    },
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
    isi::{Instruction as _, offline::ActivateKagemushaRecursiveReleaseV4},
    peer::PeerId,
    query::CommittedTransaction,
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};
#[cfg(feature = "json")]
use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use std::fmt;
use thiserror::Error;

/// Exact first-release validator count required by Kagemusha activation.
pub const KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT: usize = 4;
/// Minimum distinct governance signers required by the V4 activation corridor.
pub const KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS: usize = 2;
/// Maximum canonical Norito bytes accepted for one activation receipt.
pub const KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES: usize = 64 * 1024 * 1024;
/// Maximum raw JSON request bytes accepted by the bounded receipt decoder.
///
/// This is an ingress ceiling, not a peak-memory guarantee. The embedded block
/// parser independently rejects a base64 token whose decoded form exceeds the
/// 32 MiB authenticated-block limit before allocating decoded storage.
pub const KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_JSON_BYTES: usize = 96 * 1024 * 1024;
/// Schema id of a validator qualification body.
pub const KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_BODY_SCHEMA: &str =
    "iroha.kagemusha.v4.validator_qualification_seal_body.v1";
/// Schema id of a signed validator qualification seal.
pub const KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_SCHEMA: &str =
    "iroha.kagemusha.v4.validator_qualification_seal.v1";
/// Schema id of an activation-finality receipt body.
pub const KAGEMUSHA_V4_ACTIVATION_FINALITY_RECEIPT_BODY_SCHEMA: &str =
    "iroha.kagemusha.v4.activation_finality_receipt_body.v1";
/// Schema id of a signed activation-finality receipt.
pub const KAGEMUSHA_V4_ACTIVATION_FINALITY_RECEIPT_SCHEMA: &str =
    "iroha.kagemusha.v4.activation_finality_receipt.v1";
/// Current validator qualification and activation receipt version.
pub const KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION: u16 = 1;
/// Domain separator for validator qualification signatures.
pub const KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SIGNATURE_DOMAIN: &[u8] =
    b"iroha:kagemusha:v4:validator-qualification-seal:v1\0";
/// Domain separator for durable activation-finality receipt signatures.
pub const KAGEMUSHA_V4_ACTIVATION_FINALITY_SIGNATURE_DOMAIN: &[u8] =
    b"iroha:kagemusha:v4:activation-finality-receipt:v1\0";

/// Exact byte identity used for an externally held immutable input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaExactBytesDigestV1 {
    /// Exact byte length.
    pub byte_len: u64,
    /// SHA-256 of the exact bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sha256: [u8; 32],
}

impl KagemushaExactBytesDigestV1 {
    /// Derive the exact byte identity of a non-empty input.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] when the input is
    /// empty or its length does not fit the wire field.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        if bytes.is_empty() {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "exact_bytes.byte_len",
            ));
        }
        let byte_len = u64::try_from(bytes.len()).map_err(|_| {
            KagemushaPromotionReceiptValidationError::InvalidField("exact_bytes.byte_len")
        })?;
        Ok(Self {
            byte_len,
            sha256: Sha256::digest(bytes).into(),
        })
    }

    /// Validate the non-empty, non-zero identity shape.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] for a zero length
    /// or all-zero digest.
    pub fn validate(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        if self.byte_len == 0 || self.sha256 == [0; 32] {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "exact_bytes",
            ));
        }
        Ok(())
    }

    /// Return whether this identity describes the supplied exact bytes.
    #[must_use]
    pub fn matches_bytes(&self, bytes: &[u8]) -> bool {
        u64::try_from(bytes.len()) == Ok(self.byte_len)
            && <[u8; 32]>::from(Sha256::digest(bytes)) == self.sha256
    }
}

/// Shared release, policy, catalog, genesis, and consensus identity.
///
/// Every validator body must carry this value byte-for-byte. Host-local
/// executable, configuration-source, and catalog-seal identities deliberately
/// live outside it because heterogeneous validators need not share those bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4PromotionBindingV1 {
    /// Unique non-zero promotion-run identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub promotion_id: [u8; 32],
    /// Exact genesis-derived network identity.
    pub network_id: NetworkId,
    /// SHA-256 of the reviewed clean source-closure descriptor.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub reviewed_source_closure_descriptor_sha256: [u8; 32],
    /// SHA-256 of the canonical promoted V4 manifest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub manifest_sha256: [u8; 32],
    /// SHA-256 of the canonical promoted V4 release record.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_record_sha256: [u8; 32],
    /// Exact root-custodied release-policy source bytes.
    pub release_policy_source: KagemushaExactBytesDigestV1,
    /// Exact canonical governed device-attestation policy bytes.
    pub device_attestation_policy_norito: KagemushaExactBytesDigestV1,
    /// Exact signed-genesis source bytes for an ordinary genesis-rooted node.
    pub signed_genesis: KagemushaExactBytesDigestV1,
    /// Deterministic logical digest of the complete ordered Kagemusha catalog.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub catalog_consensus_policy_digest: [u8; 32],
    /// Aggregate protocol execution-policy identity authenticated by finality.
    pub execution_policy_hash: Hash,
}

impl KagemushaV4PromotionBindingV1 {
    fn validate(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        let required_digests = [
            self.promotion_id,
            self.reviewed_source_closure_descriptor_sha256,
            self.manifest_sha256,
            self.release_record_sha256,
            self.catalog_consensus_policy_digest,
        ];
        if required_digests.iter().any(|digest| *digest == [0; 32])
            || self.network_id.as_bytes().iter().all(|byte| *byte == 0)
            || self.execution_policy_hash == Hash::prehashed([0; Hash::LENGTH])
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "promotion_binding",
            ));
        }
        self.release_policy_source.validate()?;
        self.device_attestation_policy_norito.validate()?;
        self.signed_genesis.validate()?;
        Ok(())
    }
}

/// One validator's exact signed qualification statement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4ValidatorQualificationSealBodyV1 {
    /// Exact body schema.
    pub schema: String,
    /// Body version.
    pub version: u16,
    /// Shared promotion and consensus identity.
    pub binding: KagemushaV4PromotionBindingV1,
    /// Validator identity and signing key.
    pub validator_id: PeerId,
    /// Exact validator-local `iroha3d` executable bytes.
    pub iroha3d_executable: KagemushaExactBytesDigestV1,
    /// Exact flattened TOML source bytes read on this validator.
    ///
    /// This deliberately does not claim to cover environment, command-line,
    /// or profile overlays. `execution_policy_hash` binds consensus-relevant
    /// effective policy, while the protected launcher must enforce its exact
    /// environment contract independently.
    pub flattened_toml_config_source: KagemushaExactBytesDigestV1,
    /// Exact host-local canonical catalog qualification-seal bytes.
    pub catalog_qualification_seal: KagemushaExactBytesDigestV1,
}

impl KagemushaV4ValidatorQualificationSealBodyV1 {
    /// Return the domain-separated typed hash signed by the validator.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SIGNATURE_DOMAIN,
            &self.encode(),
        ]))
    }

    /// Validate non-cryptographic body structure.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] when a schema,
    /// shared binding, validator algorithm, or exact-byte identity is invalid.
    pub fn validate_structure(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        if self.schema != KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_BODY_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || !matches!(
                self.validator_id.public_key().try_algorithm(),
                Ok(Algorithm::BlsNormal)
            )
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "validator_qualification.body",
            ));
        }
        self.binding.validate()?;
        self.iroha3d_executable.validate()?;
        self.flattened_toml_config_source.validate()?;
        self.catalog_qualification_seal.validate()?;
        Ok(())
    }
}

/// Signed qualification seal emitted by one validator host.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4ValidatorQualificationSealV1 {
    /// Exact seal schema.
    pub schema: String,
    /// Seal version.
    pub version: u16,
    /// Signed host statement.
    pub body: KagemushaV4ValidatorQualificationSealBodyV1,
    /// Validator signature over `body.signing_hash()`.
    pub signature: SignatureOf<KagemushaV4ValidatorQualificationSealBodyV1>,
}

impl KagemushaV4ValidatorQualificationSealV1 {
    /// Sign a structurally valid body with its validator key.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] when the body is
    /// invalid, the key does not match `validator_id`, or signing fails.
    pub fn try_sign(
        body: KagemushaV4ValidatorQualificationSealBodyV1,
        key_pair: &KeyPair,
    ) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        body.validate_structure()?;
        if key_pair.public_key() != body.validator_id.public_key() {
            return Err(KagemushaPromotionReceiptValidationError::SignerMismatch(
                "validator_qualification",
            ));
        }
        let signature = SignatureOf::try_from_hash(key_pair.private_key(), body.signing_hash())
            .map_err(|_| {
                KagemushaPromotionReceiptValidationError::InvalidSignature(
                    "validator_qualification",
                )
            })?;
        Ok(Self {
            schema: KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            body,
            signature,
        })
    }

    /// Validate body structure and the validator signature.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] for any malformed
    /// schema, binding, signer, or signature.
    pub fn verify(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        if self.schema != KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "validator_qualification.seal",
            ));
        }
        self.body.validate_structure()?;
        verify_typed_signature_hash(
            &self.signature,
            self.body.validator_id.public_key(),
            self.body.signing_hash(),
            "validator_qualification",
        )
    }
}

/// Bounded exact `SignedBlockWire` bytes retained by a durable receipt.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(reuse_archived)]
#[repr(transparent)]
pub struct KagemushaFinalizedBlockWireV1(Vec<u8>);

impl fmt::Debug for KagemushaFinalizedBlockWireV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KagemushaFinalizedBlockWireV1")
            .field("byte_len", &self.0.len())
            .finish()
    }
}

impl KagemushaFinalizedBlockWireV1 {
    /// Construct a non-empty block wire within the 32 MiB authenticated limit.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] when the bytes are
    /// empty or exceed the public authenticated-block ceiling.
    pub fn try_from_bytes(
        bytes: Vec<u8>,
    ) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        if bytes.is_empty() || bytes.len() > AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1 {
            return Err(KagemushaPromotionReceiptValidationError::BlockWireSize {
                actual: bytes.len(),
                maximum: AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
            });
        }
        Ok(Self(bytes))
    }

    /// Borrow the exact framed block bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Return the exact byte length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return whether the wrapper contains no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl TryFrom<Vec<u8>> for KagemushaFinalizedBlockWireV1 {
    type Error = KagemushaPromotionReceiptValidationError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from_bytes(value)
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for KagemushaFinalizedBlockWireV1 {
    fn write_json(&self, out: &mut String) {
        norito::json::write_base64_json(&self.0, out);
    }

    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_base64_json_to(&self.0, out)
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for KagemushaFinalizedBlockWireV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let (encoded, decoded_len) =
            bounded_base64_token(parser, AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1)?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|error| norito::json::Error::Message(error.to_string()))?;
        if bytes.len() != decoded_len {
            return Err(receipt_json_error(
                "base64 decoder length disagrees with bounded preflight",
            ));
        }
        Self::try_from_bytes(bytes).map_err(|error| receipt_json_error(&error.to_string()))
    }

    fn json_from_value(value: &norito::json::Value) -> Result<Self, norito::json::Error> {
        let encoded = value
            .as_str()
            .ok_or_else(|| receipt_json_error("expected canonical base64 string"))?;
        let decoded_len = canonical_base64_decoded_len(
            encoded,
            AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
        )?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|error| norito::json::Error::Message(error.to_string()))?;
        if bytes.len() != decoded_len {
            return Err(receipt_json_error(
                "base64 decoder length disagrees with bounded preflight",
            ));
        }
        Self::try_from_bytes(bytes).map_err(|error| receipt_json_error(&error.to_string()))
    }
}

/// Complete signed receipt body for one finalized governed activation.
#[derive(Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4ActivationFinalityReceiptBodyV1 {
    /// Exact receipt-body schema.
    pub schema: String,
    /// Receipt-body version.
    pub version: u16,
    /// Shared promotion, release, catalog, and consensus binding.
    pub binding: KagemushaV4PromotionBindingV1,
    /// Public key of the independent durable-receipt issuer.
    pub issuer: PublicKey,
    /// Exact governed account which authorized the activation transaction.
    pub governance_authority: AccountId,
    /// Four validator seals in strict `validator_id` order.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_array"))]
    pub validator_seals:
        [KagemushaV4ValidatorQualificationSealV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    /// Payload-only activation intent identity from `SignedTransaction::hash()`.
    pub activation_transaction_intent: HashOf<SignedTransaction>,
    /// Exact authorization-bearing bytes from `SignedTransaction::encode_wire_v1()`.
    pub activation_transaction_wire: KagemushaExactBytesDigestV1,
    /// Committed entrypoint/result and their inclusion paths.
    pub committed_transaction: CommittedTransaction,
    /// Exact result-bearing canonical framed block bytes.
    pub finalized_block_wire: KagemushaFinalizedBlockWireV1,
    /// Separately retained exact identity of `finalized_block_wire`.
    pub finalized_block_wire_digest: KagemushaExactBytesDigestV1,
    /// Exact durable Sumeragi-v2 finality material for the carrier block.
    pub finality_proof: BridgeFinalityProof,
}

impl fmt::Debug for KagemushaV4ActivationFinalityReceiptBodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KagemushaV4ActivationFinalityReceiptBodyV1")
            .field("schema", &self.schema)
            .field("version", &self.version)
            .field("promotion_id", &self.binding.promotion_id)
            .field("issuer", &self.issuer)
            .field("governance_authority", &self.governance_authority)
            .field(
                "activation_transaction_intent",
                &self.activation_transaction_intent,
            )
            .field("finalized_block_wire_len", &self.finalized_block_wire.len())
            .field(
                "finalized_block_wire_digest",
                &self.finalized_block_wire_digest,
            )
            .field(
                "finalized_height",
                &self.finality_proof.finality_artifact.height,
            )
            .finish_non_exhaustive()
    }
}

impl KagemushaV4ActivationFinalityReceiptBodyV1 {
    /// Return the domain-separated typed hash signed by the receipt issuer.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            KAGEMUSHA_V4_ACTIVATION_FINALITY_SIGNATURE_DOMAIN,
            &self.encode(),
        ]))
    }

    fn validate_structure(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        if self.schema != KAGEMUSHA_V4_ACTIVATION_FINALITY_RECEIPT_BODY_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || self.finalized_block_wire.is_empty()
            || self.finalized_block_wire.len() > AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "activation_receipt.body",
            ));
        }
        if !supports_receipt_signature_algorithm(&self.issuer) {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "activation_receipt.issuer",
            ));
        }
        validate_governance_multisig_policy(&self.governance_authority)?;
        self.binding.validate()?;
        self.activation_transaction_wire.validate()?;
        self.finalized_block_wire_digest.validate()?;
        if !self
            .finalized_block_wire_digest
            .matches_bytes(self.finalized_block_wire.as_bytes())
        {
            return Err(KagemushaPromotionReceiptValidationError::BlockWireDigest);
        }
        if self
            .activation_transaction_intent
            .as_ref()
            .iter()
            .all(|byte| *byte == 0)
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "activation_receipt.transaction_intent",
            ));
        }
        Ok(())
    }
}

/// Signed durable receipt for one exact finalized activation.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4ActivationFinalityReceiptV1 {
    /// Exact receipt schema.
    pub schema: String,
    /// Receipt version.
    pub version: u16,
    /// Signed durable statement.
    pub body: KagemushaV4ActivationFinalityReceiptBodyV1,
    /// Independent issuer signature over `body.signing_hash()`.
    pub signature: SignatureOf<KagemushaV4ActivationFinalityReceiptBodyV1>,
}

/// External immutable expectations required to trust an activation receipt.
///
/// This verifier configuration is intentionally not serialized with the
/// receipt. Loading these values from the receipt itself would turn duplicate
/// fields into attacker-controlled assertions rather than trust anchors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KagemushaV4ActivationReceiptExpectationsV1 {
    /// Exact shared promotion binding approved before submission.
    pub binding: KagemushaV4PromotionBindingV1,
    /// Independently pinned durable-receipt issuer.
    pub receipt_issuer: PublicKey,
    /// Exact governance account permitted to authorize activation.
    pub governance_authority: AccountId,
    /// Exact canonical multisignature policy independently approved for governance.
    pub governance_multisig_policy: MultisigPolicy,
    /// Four exact expected host bodies in strict validator order.
    pub validator_bodies:
        [KagemushaV4ValidatorQualificationSealBodyV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    /// Exact payload-only transaction intent approved by governance.
    pub activation_transaction_intent: HashOf<SignedTransaction>,
    /// Exact authorization-bearing transaction wire approved by governance.
    pub activation_transaction_wire: KagemushaExactBytesDigestV1,
    /// Exact height context trusted for this one receipt.
    pub activation_height_context: HeightContextId,
}

impl KagemushaV4ActivationReceiptExpectationsV1 {
    fn validate(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        self.binding.validate()?;
        self.activation_transaction_wire.validate()?;
        let authority_policy = validate_governance_multisig_policy(&self.governance_authority)?;
        if authority_policy != &self.governance_multisig_policy {
            return Err(KagemushaPromotionReceiptValidationError::GovernanceAuthority);
        }
        if !supports_receipt_signature_algorithm(&self.receipt_issuer)
            || self
                .activation_transaction_intent
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || self
                .activation_height_context
                .0
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "activation_receipt.expectations",
            ));
        }
        let mut previous = None;
        for body in &self.validator_bodies {
            body.validate_structure()?;
            if body.binding != self.binding
                || previous.is_some_and(|validator: &PeerId| validator >= &body.validator_id)
            {
                return Err(KagemushaPromotionReceiptValidationError::ValidatorSet);
            }
            previous = Some(&body.validator_id);
        }
        Ok(())
    }
}

/// Authenticated receipt result returned only after every binding verifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KagemushaV4VerifiedActivationReceiptV1 {
    /// Exact finalized block height.
    pub finalized_height: u64,
    /// Hash of the exact finalized block header.
    pub finalized_block_hash: HashOf<crate::block::BlockHeader>,
    /// Exact payload-only activation transaction intent.
    pub activation_transaction_intent: HashOf<SignedTransaction>,
}

impl KagemushaV4ActivationFinalityReceiptV1 {
    /// Sign a structurally valid body with its declared receipt-issuer key.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] when structure,
    /// issuer identity, or signing fails. This constructor does not replace
    /// [`Self::verify`] and does not claim that activation occurred.
    pub fn try_sign(
        body: KagemushaV4ActivationFinalityReceiptBodyV1,
        key_pair: &KeyPair,
    ) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        body.validate_structure()?;
        enforce_activation_receipt_frame_size(&body)?;
        if key_pair.public_key() != &body.issuer {
            return Err(KagemushaPromotionReceiptValidationError::SignerMismatch(
                "activation_receipt",
            ));
        }
        let signature = SignatureOf::try_from_hash(key_pair.private_key(), body.signing_hash())
            .map_err(|_| {
                KagemushaPromotionReceiptValidationError::InvalidSignature("activation_receipt")
            })?;
        let receipt = Self {
            schema: KAGEMUSHA_V4_ACTIVATION_FINALITY_RECEIPT_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            body,
            signature,
        };
        enforce_activation_receipt_frame_size(&receipt)?;
        Ok(receipt)
    }

    /// Decode one exact canonical receipt under explicit cumulative limits.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] before decode when
    /// the outer frame exceeds 64 MiB, or when canonical decoding/resource
    /// limits and embedded block-wire bounds are violated.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES {
            return Err(KagemushaPromotionReceiptValidationError::ReceiptSize {
                actual: bytes.len(),
                maximum: KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES,
            });
        }
        let limits = receipt_decode_limits(bytes.len());
        let receipt: Self = norito::decode_canonical_with_limits(bytes, limits)
            .map_err(|_| KagemushaPromotionReceiptValidationError::ReceiptDecode)?;
        receipt.body.validate_structure()?;
        Ok(receipt)
    }

    /// Decode a raw JSON receipt only after enforcing the ingress byte cap.
    ///
    /// The embedded block-wire parser also bounds decoded base64 length before
    /// allocation. Callers receiving HTTP bodies must invoke this method on the
    /// already size-limited raw body rather than parse an unbounded JSON value.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] for an oversized
    /// body, non-canonical JSON, oversized block token, or invalid structure.
    #[cfg(feature = "json")]
    pub fn decode_json_bounded(
        json: &str,
    ) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        if json.is_empty() || json.len() > KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_JSON_BYTES {
            return Err(KagemushaPromotionReceiptValidationError::JsonSize {
                actual: json.len(),
                maximum: KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_JSON_BYTES,
            });
        }
        let receipt: Self = norito::json::from_str(json)
            .map_err(|_| KagemushaPromotionReceiptValidationError::ReceiptJsonDecode)?;
        enforce_activation_receipt_frame_size(&receipt)?;
        receipt.body.validate_structure()?;
        Ok(receipt)
    }

    /// Verify every external, signature, transaction, inclusion, and finality binding.
    ///
    /// A fresh local [`BridgeFinalityVerifier`] is created for this receipt and
    /// pinned to `expectations.activation_height_context`; failed input can
    /// therefore never advance shared verifier state. The method performs no
    /// live write and does not submit or activate anything.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] on the first
    /// schema, expectation, seal, transaction, block, or finality mismatch.
    pub fn verify(
        &self,
        expectations: &KagemushaV4ActivationReceiptExpectationsV1,
    ) -> Result<KagemushaV4VerifiedActivationReceiptV1, KagemushaPromotionReceiptValidationError>
    {
        if self.schema != KAGEMUSHA_V4_ACTIVATION_FINALITY_RECEIPT_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "activation_receipt",
            ));
        }
        expectations.validate()?;
        self.body.validate_structure()?;
        if self.body.binding != expectations.binding
            || self.body.issuer != expectations.receipt_issuer
            || self.body.governance_authority != expectations.governance_authority
            || self.body.activation_transaction_intent != expectations.activation_transaction_intent
            || self.body.activation_transaction_wire != expectations.activation_transaction_wire
        {
            return Err(KagemushaPromotionReceiptValidationError::ExpectationMismatch);
        }
        enforce_activation_receipt_frame_size(self)?;
        verify_typed_signature_hash(
            &self.signature,
            &self.body.issuer,
            self.body.signing_hash(),
            "activation_receipt",
        )?;
        verify_kagemusha_v4_validator_qualification_seals(
            &self.body.validator_seals,
            &expectations.validator_bodies,
            &expectations.binding,
        )?;

        let block = decode_exact_finalized_block(self.body.finalized_block_wire.as_bytes())?;
        let committed = &self.body.committed_transaction;
        if committed.merge_inclusion.is_some()
            || !committed.verify_inclusion_in_block(&block)
            || committed.result.0.is_err()
            || block
                .entrypoint_hashes()
                .filter(|entry_hash| entry_hash == &committed.entrypoint_hash)
                .count()
                != 1
        {
            return Err(KagemushaPromotionReceiptValidationError::CommittedTransaction);
        }
        let TransactionEntrypoint::External(transaction) = &committed.entrypoint else {
            return Err(KagemushaPromotionReceiptValidationError::ActivationTransaction);
        };
        transaction
            .verify_signature()
            .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationTransaction)?;
        let transaction_policy = validate_governance_multisig_policy(transaction.authority())?;
        if transaction_policy != &expectations.governance_multisig_policy
            || transaction.multisig_signatures().is_none_or(|bundle| {
                bundle.signatures.len() < KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS
            })
        {
            return Err(KagemushaPromotionReceiptValidationError::GovernanceAuthority);
        }
        if transaction.authority() != &expectations.governance_authority
            || transaction.network_id() != Some(&expectations.binding.network_id)
            || transaction.hash() != expectations.activation_transaction_intent
        {
            return Err(KagemushaPromotionReceiptValidationError::ActivationTransaction);
        }
        let transaction_wire = transaction
            .encode_wire_v1()
            .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationTransaction)?;
        if !expectations
            .activation_transaction_wire
            .matches_bytes(&transaction_wire)
        {
            return Err(KagemushaPromotionReceiptValidationError::ActivationTransaction);
        }
        let activation = direct_activation_instruction(transaction)?;
        validate_activation_binding(activation, &expectations.binding)?;

        let artifact = &self.body.finality_proof.finality_artifact;
        if artifact.height_context.execution_policy_hash
            != expectations.binding.execution_policy_hash
            || artifact.height_context.roster.len() != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT
            || artifact
                .height_context
                .roster
                .iter()
                .zip(&expectations.validator_bodies)
                .any(|(member, body)| member.power != 1 || member.validator != body.validator_id)
        {
            return Err(KagemushaPromotionReceiptValidationError::FinalityRoster);
        }
        let mut finality_verifier = BridgeFinalityVerifier::with_context(
            expectations.binding.network_id,
            expectations.activation_height_context,
        );
        finality_verifier
            .verify(&self.body.finality_proof)
            .map_err(|_| KagemushaPromotionReceiptValidationError::Finality)?;
        let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
            &block,
            artifact,
            &committed.entrypoint_hash,
        )
        .map_err(|_| KagemushaPromotionReceiptValidationError::FinalityBlockBinding)?;
        let entry_index = usize::try_from(anchor.entry_index())
            .map_err(|_| KagemushaPromotionReceiptValidationError::FinalityBlockBinding)?;
        let block_entrypoint = block
            .entrypoints_cloned()
            .nth(entry_index)
            .ok_or(KagemushaPromotionReceiptValidationError::FinalityBlockBinding)?;
        let TransactionEntrypoint::External(block_transaction) = block_entrypoint else {
            return Err(KagemushaPromotionReceiptValidationError::ActivationAuthorizationWire);
        };
        let block_transaction_wire = block_transaction
            .encode_wire_v1()
            .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationAuthorizationWire)?;
        if block_transaction_wire != transaction_wire {
            return Err(KagemushaPromotionReceiptValidationError::ActivationAuthorizationWire);
        }

        Ok(KagemushaV4VerifiedActivationReceiptV1 {
            finalized_height: block.header().height().get(),
            finalized_block_hash: block.hash(),
            activation_transaction_intent: transaction.hash(),
        })
    }
}

fn validate_governance_multisig_policy(
    authority: &AccountId,
) -> Result<&MultisigPolicy, KagemushaPromotionReceiptValidationError> {
    let Some(policy) = authority.controller().multisig_policy() else {
        return Err(KagemushaPromotionReceiptValidationError::GovernanceAuthority);
    };
    if usize::from(policy.threshold()) < KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS
        || policy.members().len() < KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS
    {
        return Err(KagemushaPromotionReceiptValidationError::GovernanceAuthority);
    }
    Ok(policy)
}

fn enforce_activation_receipt_frame_size<T: norito::NoritoSerialize>(
    value: &T,
) -> Result<usize, KagemushaPromotionReceiptValidationError> {
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let actual = norito::core::encoded_frame_len(value)
        .map_err(|_| KagemushaPromotionReceiptValidationError::ReceiptEncode)?;
    if actual > KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES {
        return Err(KagemushaPromotionReceiptValidationError::ReceiptSize {
            actual,
            maximum: KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES,
        });
    }
    Ok(actual)
}

/// Verify four exact validator seals against external bodies and one shared binding.
///
/// # Errors
///
/// Returns [`KagemushaPromotionReceiptValidationError`] unless both actual and
/// expected bodies are strictly sorted, distinct, byte-for-byte equal, carry
/// the shared binding, and have valid validator signatures.
pub fn verify_kagemusha_v4_validator_qualification_seals(
    seals: &[KagemushaV4ValidatorQualificationSealV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    expected_bodies: &[KagemushaV4ValidatorQualificationSealBodyV1;
         KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    binding: &KagemushaV4PromotionBindingV1,
) -> Result<(), KagemushaPromotionReceiptValidationError> {
    binding.validate()?;
    let mut previous = None;
    for (seal, expected) in seals.iter().zip(expected_bodies) {
        expected.validate_structure()?;
        if expected.binding != *binding
            || seal.body != *expected
            || previous.is_some_and(|validator: &PeerId| validator >= &seal.body.validator_id)
        {
            return Err(KagemushaPromotionReceiptValidationError::ValidatorSet);
        }
        seal.verify()?;
        previous = Some(&seal.body.validator_id);
    }
    Ok(())
}

fn direct_activation_instruction(
    transaction: &SignedTransaction,
) -> Result<&ActivateKagemushaRecursiveReleaseV4, KagemushaPromotionReceiptValidationError> {
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(KagemushaPromotionReceiptValidationError::ActivationTransaction);
    };
    if instructions.len() != 1 {
        return Err(KagemushaPromotionReceiptValidationError::ActivationTransaction);
    }
    instructions[0]
        .as_any()
        .downcast_ref::<ActivateKagemushaRecursiveReleaseV4>()
        .ok_or(KagemushaPromotionReceiptValidationError::ActivationTransaction)
}

fn validate_activation_binding(
    instruction: &ActivateKagemushaRecursiveReleaseV4,
    binding: &KagemushaV4PromotionBindingV1,
) -> Result<(), KagemushaPromotionReceiptValidationError> {
    let activation = instruction.activation();
    activation
        .validate_structure()
        .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationPayload)?;
    let manifest = &activation.release_record.manifest;
    let manifest_sha256 = manifest
        .canonical_sha256()
        .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationPayload)?;
    let release_record_sha256 = canonical_sha256(&activation.release_record)?;
    let device_policy = norito::encode_canonical(instruction.device_attestation_policy())
        .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationPayload)?;
    if manifest.network_id != binding.network_id
        || manifest.reviewed_source_closure_descriptor_sha256
            != binding.reviewed_source_closure_descriptor_sha256
        || manifest_sha256 != binding.manifest_sha256
        || release_record_sha256 != binding.release_record_sha256
        || activation.configured_policy_sha256 != binding.release_policy_source.sha256
        || !binding
            .device_attestation_policy_norito
            .matches_bytes(&device_policy)
    {
        return Err(KagemushaPromotionReceiptValidationError::ActivationPayload);
    }
    Ok(())
}

fn canonical_sha256<T: norito::NoritoSerialize>(
    value: &T,
) -> Result<[u8; 32], KagemushaPromotionReceiptValidationError> {
    norito::encode_canonical(value)
        .map(|bytes| Sha256::digest(bytes).into())
        .map_err(|_| KagemushaPromotionReceiptValidationError::ReceiptEncode)
}

fn verify_typed_signature_hash<T>(
    signature: &SignatureOf<T>,
    signer: &PublicKey,
    signing_hash: HashOf<T>,
    field: &'static str,
) -> Result<(), KagemushaPromotionReceiptValidationError> {
    match signer.try_algorithm() {
        Ok(Algorithm::Ed25519) => {
            iroha_crypto::ed25519_parse_signature(signature.payload())
                .map_err(|_| KagemushaPromotionReceiptValidationError::InvalidSignature(field))?;
        }
        Ok(Algorithm::MlDsa) => {
            iroha_crypto::mldsa65_parse_signature(signature.payload())
                .map_err(|_| KagemushaPromotionReceiptValidationError::InvalidSignature(field))?;
        }
        Ok(Algorithm::BlsNormal) => {}
        _ => {
            return Err(KagemushaPromotionReceiptValidationError::InvalidSignature(
                field,
            ));
        }
    }
    signature
        .verify_hash(signer, signing_hash)
        .map_err(|_| KagemushaPromotionReceiptValidationError::InvalidSignature(field))
}

fn supports_receipt_signature_algorithm(signer: &PublicKey) -> bool {
    matches!(
        signer.try_algorithm(),
        Ok(Algorithm::Ed25519 | Algorithm::MlDsa | Algorithm::BlsNormal)
    )
}

fn receipt_decode_limits(encoded_len: usize) -> norito::DecodeLimits {
    let maximum = KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES;
    let allocation = maximum.saturating_mul(8);
    norito::DecodeLimits::new(
        AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
        maximum,
        encoded_len.saturating_mul(8),
        allocation,
        norito::core::MAX_OWNED_VALUE_DECODE_DEPTH,
    )
}

fn block_decode_limits() -> norito::DecodeLimits {
    let maximum = AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1;
    norito::DecodeLimits::new(
        maximum.saturating_mul(8),
        maximum,
        maximum.saturating_mul(8),
        maximum.saturating_mul(8),
        norito::core::MAX_OWNED_VALUE_DECODE_DEPTH,
    )
}

fn decode_exact_finalized_block(
    bytes: &[u8],
) -> Result<SignedBlock, KagemushaPromotionReceiptValidationError> {
    if bytes.is_empty() || bytes.len() > AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1 {
        return Err(KagemushaPromotionReceiptValidationError::BlockWireSize {
            actual: bytes.len(),
            maximum: AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
        });
    }
    let block = norito::with_decode_limits(block_decode_limits(), || {
        decode_framed_signed_block(bytes).map_err(|_| norito::core::Error::LengthMismatch)
    })
    .map_err(|_| KagemushaPromotionReceiptValidationError::BlockDecode)?;
    let canonical = block
        .encode_wire()
        .map_err(|_| KagemushaPromotionReceiptValidationError::BlockEncode)?;
    if canonical != bytes {
        return Err(KagemushaPromotionReceiptValidationError::NonCanonicalBlock);
    }
    Ok(block)
}

#[cfg(feature = "json")]
fn receipt_json_error(message: &str) -> norito::json::Error {
    norito::json::Error::InvalidField {
        field: "finalized_block_wire".to_owned(),
        message: message.to_owned(),
    }
}

#[cfg(feature = "json")]
fn base64_encoded_len(maximum_decoded_bytes: usize) -> Option<usize> {
    maximum_decoded_bytes
        .checked_add(2)?
        .checked_div(3)?
        .checked_mul(4)
}

#[cfg(feature = "json")]
fn base64_sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[cfg(feature = "json")]
fn canonical_base64_decoded_len(
    encoded: &str,
    maximum_decoded_bytes: usize,
) -> Result<usize, norito::json::Error> {
    if encoded.is_empty() || !encoded.len().is_multiple_of(4) {
        return Err(receipt_json_error(
            "base64 token has a non-canonical length",
        ));
    }
    let padding = match encoded.as_bytes() {
        [.., b'=', b'='] => 2,
        [.., b'='] => 1,
        _ => 0,
    };
    let payload_len = encoded.len() - padding;
    let bytes = encoded.as_bytes();
    if bytes[..payload_len]
        .iter()
        .any(|byte| base64_sextet(*byte).is_none())
        || bytes[payload_len..].iter().any(|byte| *byte != b'=')
    {
        return Err(receipt_json_error(
            "expected canonical padded standard base64",
        ));
    }
    let tail_is_canonical = match padding {
        0 => true,
        1 => {
            payload_len % 4 == 3
                && base64_sextet(bytes[payload_len - 1])
                    .is_some_and(|sextet| sextet.is_multiple_of(4))
        }
        2 => {
            payload_len % 4 == 2
                && base64_sextet(bytes[payload_len - 1])
                    .is_some_and(|sextet| sextet.is_multiple_of(16))
        }
        _ => false,
    };
    if !tail_is_canonical {
        return Err(receipt_json_error(
            "base64 token has non-canonical tail bits",
        ));
    }
    let decoded_len = encoded
        .len()
        .checked_div(4)
        .and_then(|length| length.checked_mul(3))
        .and_then(|length| length.checked_sub(padding))
        .ok_or_else(|| receipt_json_error("invalid base64 decoded length"))?;
    if decoded_len > maximum_decoded_bytes {
        return Err(receipt_json_error(
            "decoded block wire exceeds the 32 MiB limit",
        ));
    }
    Ok(decoded_len)
}

#[cfg(feature = "json")]
fn bounded_base64_token<'a>(
    parser: &mut norito::json::Parser<'a>,
    maximum_decoded_bytes: usize,
) -> Result<(&'a str, usize), norito::json::Error> {
    let maximum_encoded_bytes = base64_encoded_len(maximum_decoded_bytes)
        .ok_or_else(|| receipt_json_error("base64 length arithmetic overflow"))?;
    parser.skip_ws();
    let start = parser.position();
    let decoded_token_len = parser.skip_string_bounded(maximum_encoded_bytes)?;
    let end = parser.position();
    let encoded = parser
        .input()
        .get(start.saturating_add(1)..end.saturating_sub(1))
        .ok_or_else(|| receipt_json_error("invalid JSON string bounds"))?;
    if encoded.len() != decoded_token_len || encoded.as_bytes().contains(&b'\\') {
        return Err(receipt_json_error(
            "base64 token must use its unescaped canonical spelling",
        ));
    }
    let decoded_len = canonical_base64_decoded_len(encoded, maximum_decoded_bytes)?;
    Ok((encoded, decoded_len))
}

/// Failure while decoding or validating a Kagemusha promotion receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum KagemushaPromotionReceiptValidationError {
    /// One named structural field is invalid.
    #[error("invalid Kagemusha promotion receipt field: {0}")]
    InvalidField(&'static str),
    /// A signer does not match the signed identity.
    #[error("Kagemusha promotion receipt signer mismatch: {0}")]
    SignerMismatch(&'static str),
    /// A signature is malformed or invalid.
    #[error("invalid Kagemusha promotion receipt signature: {0}")]
    InvalidSignature(&'static str),
    /// Four validator identities or their order differ.
    #[error("Kagemusha validator set is not the exact ordered four-validator set")]
    ValidatorSet,
    /// Receipt-controlled values differ from external expectations.
    #[error("Kagemusha activation receipt differs from external expectations")]
    ExpectationMismatch,
    /// Canonical receipt encoding failed.
    #[error("failed to encode the canonical Kagemusha activation receipt")]
    ReceiptEncode,
    /// Canonical bounded receipt decoding failed.
    #[error("failed to decode the canonical bounded Kagemusha activation receipt")]
    ReceiptDecode,
    /// Bounded JSON receipt decoding failed.
    #[error("failed to decode the bounded Kagemusha activation receipt JSON")]
    ReceiptJsonDecode,
    /// Canonical receipt input violates the encoded-size ceiling.
    #[error("Kagemusha activation receipt is {actual} bytes; maximum is {maximum}")]
    ReceiptSize {
        /// Actual byte length.
        actual: usize,
        /// Maximum permitted byte length.
        maximum: usize,
    },
    /// Raw JSON input violates the ingress ceiling.
    #[error("Kagemusha activation receipt JSON is {actual} bytes; maximum is {maximum}")]
    JsonSize {
        /// Actual byte length.
        actual: usize,
        /// Maximum permitted byte length.
        maximum: usize,
    },
    /// Embedded finalized block wire violates its public ceiling.
    #[error("Kagemusha finalized block wire is {actual} bytes; maximum is {maximum}")]
    BlockWireSize {
        /// Actual byte length.
        actual: usize,
        /// Maximum permitted byte length.
        maximum: usize,
    },
    /// The separately retained exact block identity differs from its bytes.
    #[error("finalized SignedBlockWire digest does not match its exact bytes")]
    BlockWireDigest,
    /// Framed signed-block decoding failed.
    #[error("failed to decode the exact finalized SignedBlockWire")]
    BlockDecode,
    /// Canonical block re-encoding failed.
    #[error("failed to re-encode the finalized SignedBlockWire")]
    BlockEncode,
    /// Decoded block bytes are not their exact canonical re-encoding.
    #[error("finalized SignedBlockWire is not canonical")]
    NonCanonicalBlock,
    /// Committed entrypoint, result, or inclusion proof is invalid.
    #[error("invalid committed activation transaction or execution result")]
    CommittedTransaction,
    /// The committed entrypoint is not the exact direct governed transaction.
    #[error("invalid governed activation transaction")]
    ActivationTransaction,
    /// Governance is not the exact strong multisignature authority and bundle.
    #[error("activation governance is not the exact approved strong multisignature policy")]
    GovernanceAuthority,
    /// The finalized block carries different authorization bytes for the same intent.
    #[error("finalized block activation authorization wire differs from the approved wire")]
    ActivationAuthorizationWire,
    /// The direct activation payload differs from the sealed release and policy.
    #[error("invalid Kagemusha V4 activation payload binding")]
    ActivationPayload,
    /// Finality roster and sealed validators differ.
    #[error("finality roster differs from the four sealed validators")]
    FinalityRoster,
    /// Sumeragi-v2 finality verification failed.
    #[error("invalid activation finality proof")]
    Finality,
    /// Finality does not authenticate the supplied result-bearing block bytes.
    #[error("activation finality does not bind the supplied block and entrypoint")]
    FinalityBlockBinding,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Level,
        account::MultisigMember,
        block::{
            BlockHeader,
            consensus_v2::{
                BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
                ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
                QuorumCertificate, ValidatorPower, finality::V2FinalityArtifact,
            },
        },
        prelude::Log,
        transaction::{
            DataTriggerSequence, FeePaymentIntent, TransactionBuilder, TransactionResult,
            TransactionResultInner,
        },
    };
    use iroha_crypto::MerkleTree;
    use std::num::NonZeroU64;

    fn exact(label: &[u8]) -> KagemushaExactBytesDigestV1 {
        KagemushaExactBytesDigestV1::from_bytes(label).expect("non-empty exact bytes")
    }

    fn network(label: &[u8]) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(label)))
    }

    fn binding() -> KagemushaV4PromotionBindingV1 {
        KagemushaV4PromotionBindingV1 {
            promotion_id: Sha256::digest(b"promotion run 1").into(),
            network_id: network(b"qualification network"),
            reviewed_source_closure_descriptor_sha256: Sha256::digest(b"source closure").into(),
            manifest_sha256: Sha256::digest(b"release manifest").into(),
            release_record_sha256: Sha256::digest(b"release record").into(),
            release_policy_source: exact(b"canonical release policy source"),
            device_attestation_policy_norito: exact(b"canonical device policy Norito"),
            signed_genesis: exact(b"exact signed genesis source"),
            catalog_consensus_policy_digest: Sha256::digest(b"ordered catalog policy").into(),
            execution_policy_hash: Hash::new(b"aggregate execution policy"),
        }
    }

    fn qualification_fixture() -> (
        KagemushaV4PromotionBindingV1,
        [KagemushaV4ValidatorQualificationSealBodyV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
        [KagemushaV4ValidatorQualificationSealV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
        Vec<KeyPair>,
    ) {
        let binding = binding();
        let mut keys = [0x31_u8, 0x32, 0x33, 0x34]
            .into_iter()
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let bodies: [KagemushaV4ValidatorQualificationSealBodyV1;
            KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT] = keys
            .iter()
            .enumerate()
            .map(|(index, key)| KagemushaV4ValidatorQualificationSealBodyV1 {
                schema: KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_BODY_SCHEMA.to_owned(),
                version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
                binding: binding.clone(),
                validator_id: PeerId::new(key.public_key().clone()),
                iroha3d_executable: exact(format!("host-{index}-iroha3d").as_bytes()),
                flattened_toml_config_source: exact(
                    format!("host-{index}-flattened-config-source").as_bytes(),
                ),
                catalog_qualification_seal: exact(
                    format!("host-{index}-catalog-qualification-seal").as_bytes(),
                ),
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("exactly four qualification bodies");
        let seals = bodies
            .iter()
            .zip(&keys)
            .map(|(body, key)| {
                KagemushaV4ValidatorQualificationSealV1::try_sign(body.clone(), key)
                    .expect("sign valid validator qualification body")
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("exactly four qualification seals");
        (binding, bodies, seals, keys)
    }

    #[test]
    fn four_host_seals_accept_per_host_bytes_and_roundtrip_canonically() {
        let (binding, bodies, seals, _) = qualification_fixture();
        verify_kagemusha_v4_validator_qualification_seals(&seals, &bodies, &binding)
            .expect("four exact validator seals verify");
        assert_ne!(
            bodies[0].iroha3d_executable, bodies[1].iroha3d_executable,
            "heterogeneous validator executables are per-host identities",
        );
        assert_ne!(
            bodies[0].catalog_qualification_seal, bodies[1].catalog_qualification_seal,
            "host-local catalog-seal bytes are never forced equal",
        );
        let canonical = norito::encode_canonical(&seals[0]).expect("encode qualification seal");
        let decoded =
            norito::decode_canonical::<KagemushaV4ValidatorQualificationSealV1>(&canonical)
                .expect("decode qualification seal");
        assert_eq!(decoded, seals[0]);
        decoded.verify().expect("roundtripped signature verifies");

        #[cfg(feature = "json")]
        {
            let json = norito::json::to_json(&seals[0]).expect("encode qualification JSON");
            let decoded: KagemushaV4ValidatorQualificationSealV1 =
                norito::json::from_str(&json).expect("decode qualification JSON");
            assert_eq!(decoded, seals[0]);
            decoded.verify().expect("JSON signature verifies");
        }
    }

    #[test]
    fn validator_seals_reject_reorder_replay_and_validly_expected_body_tamper() {
        let (binding, bodies, seals, keys) = qualification_fixture();

        let mut reordered = seals.clone();
        reordered.swap(0, 1);
        assert_eq!(
            verify_kagemusha_v4_validator_qualification_seals(&reordered, &bodies, &binding,),
            Err(KagemushaPromotionReceiptValidationError::ValidatorSet),
        );

        let mut replay_binding = binding.clone();
        replay_binding.promotion_id[0] ^= 1;
        let mut replay_body = bodies[0].clone();
        replay_body.binding = replay_binding;
        let replay_seal = KagemushaV4ValidatorQualificationSealV1::try_sign(replay_body, &keys[0])
            .expect("validly sign hostile replay body");
        let mut replayed = seals.clone();
        replayed[0] = replay_seal;
        assert_eq!(
            verify_kagemusha_v4_validator_qualification_seals(&replayed, &bodies, &binding),
            Err(KagemushaPromotionReceiptValidationError::ValidatorSet),
        );

        let mut tampered_body = bodies[0].clone();
        tampered_body.catalog_qualification_seal = exact(b"substituted catalog seal bytes");
        let mut tampered_expected = bodies.clone();
        tampered_expected[0] = tampered_body.clone();
        let mut tampered_seals = seals;
        tampered_seals[0].body = tampered_body;
        assert_eq!(
            verify_kagemusha_v4_validator_qualification_seals(
                &tampered_seals,
                &tampered_expected,
                &binding,
            ),
            Err(KagemushaPromotionReceiptValidationError::InvalidSignature(
                "validator_qualification",
            )),
        );
    }

    fn roundtrip_receipt() -> KagemushaV4ActivationFinalityReceiptV1 {
        let (binding, bodies, seals, _) = qualification_fixture();
        let issuer = KeyPair::from_seed(vec![0x71; 32], Algorithm::Ed25519);
        let governance_keys = [0x72_u8, 0x73, 0x74]
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519));
        let governance_policy = MultisigPolicy::new(
            2,
            governance_keys
                .iter()
                .map(|key| {
                    MultisigMember::new(key.public_key().clone(), 1)
                        .expect("valid governance member")
                })
                .collect(),
        )
        .expect("strong governance multisig policy");
        let authority = AccountId::new_multisig(governance_policy);
        let transaction = TransactionBuilder::new(
            binding.network_id,
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "roundtrip-only receipt fixture".into(),
        )])
        .sign_multisig([
            governance_keys[0].private_key(),
            governance_keys[1].private_key(),
        ]);
        let entrypoint = TransactionEntrypoint::External(transaction.clone());
        let entrypoint_hash = transaction.hash_as_entrypoint();
        let entrypoint_tree = MerkleTree::from_iter([entrypoint_hash]);
        let result = TransactionResult(
            TransactionResultInner::Ok(DataTriggerSequence::default()),
            Vec::new(),
        );
        let result_hash = HashOf::new(&result);
        let result_tree = MerkleTree::from_iter([result_hash]);
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero fixture height"),
            None,
            entrypoint_tree.root(),
            result_tree.root(),
            1,
            0,
        );
        let committed_transaction = CommittedTransaction {
            block_hash: header.hash(),
            entrypoint_hash,
            entrypoint_proof: entrypoint_tree.get_proof(0).expect("entrypoint proof"),
            entrypoint,
            result_hash,
            result_proof: result_tree.get_proof(0).expect("result proof"),
            result,
            merge_inclusion: None,
        };
        let roster = bodies
            .iter()
            .map(|body| ValidatorPower {
                validator: body.validator_id.clone(),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = HeightContext {
            network_id: binding.network_id,
            protocol_version: PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("four-validator quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"receipt fixture nexus context"),
            execution_policy_hash: binding.execution_policy_hash,
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
            leader_seed: [0xA5; 32],
        };
        let subject = BlockSubject {
            parent_block_hash: None,
            block_hash: header.hash(),
            payload_hash: Hash::new(b"roundtrip-only proposal payload"),
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height: 1,
            view: 0,
        };
        let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"roundtrip parent state"),
            Hash::new(b"roundtrip post state"),
            Hash::new(b"roundtrip writes"),
            1,
            Hash::new(b"roundtrip block wire"),
        );
        let finality_artifact = V2FinalityArtifact::new(
            context,
            subject,
            QuorumCertificate {
                round,
                proposal_round: round,
                phase: GlobalPhase::Commit,
                subject,
                execution_commitment,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xA5],
            },
            vec![vec![0x5A]; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
        );
        let transaction_wire = transaction
            .encode_wire_v1()
            .expect("encode authorization-bearing transaction");
        let finalized_block_wire = vec![0xA5];
        KagemushaV4ActivationFinalityReceiptV1::try_sign(
            KagemushaV4ActivationFinalityReceiptBodyV1 {
                schema: KAGEMUSHA_V4_ACTIVATION_FINALITY_RECEIPT_BODY_SCHEMA.to_owned(),
                version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
                binding,
                issuer: issuer.public_key().clone(),
                governance_authority: authority,
                validator_seals: seals,
                activation_transaction_intent: transaction.hash(),
                activation_transaction_wire: KagemushaExactBytesDigestV1::from_bytes(
                    &transaction_wire,
                )
                .expect("transaction wire identity"),
                committed_transaction,
                finalized_block_wire: KagemushaFinalizedBlockWireV1::try_from_bytes(
                    finalized_block_wire.clone(),
                )
                .expect("bounded roundtrip block carrier"),
                finalized_block_wire_digest: KagemushaExactBytesDigestV1::from_bytes(
                    &finalized_block_wire,
                )
                .expect("roundtrip block identity"),
                finality_proof: BridgeFinalityProof {
                    version: crate::bridge::BRIDGE_FINALITY_PROOF_VERSION_V2,
                    block_header: header,
                    finality_artifact,
                },
            },
            &issuer,
        )
        .expect("sign roundtrip receipt")
    }

    #[test]
    fn complete_receipt_norito_and_bounded_json_roundtrip_without_claiming_activation() {
        let receipt = roundtrip_receipt();
        let canonical = norito::encode_canonical(&receipt).expect("encode complete receipt");
        let decoded = KagemushaV4ActivationFinalityReceiptV1::decode_canonical(&canonical)
            .expect("bounded canonical receipt decode");
        assert_eq!(decoded, receipt);
        assert_eq!(
            decoded.verify(&KagemushaV4ActivationReceiptExpectationsV1 {
                binding: decoded.body.binding.clone(),
                receipt_issuer: decoded.body.issuer.clone(),
                governance_authority: decoded.body.governance_authority.clone(),
                governance_multisig_policy: decoded
                    .body
                    .governance_authority
                    .controller()
                    .multisig_policy()
                    .expect("roundtrip fixture has multisig governance")
                    .clone(),
                validator_bodies: decoded.body.validator_seals.clone().map(|seal| seal.body),
                activation_transaction_intent: decoded.body.activation_transaction_intent,
                activation_transaction_wire: decoded.body.activation_transaction_wire,
                activation_height_context: decoded
                    .body
                    .finality_proof
                    .finality_artifact
                    .context_id(),
            }),
            Err(KagemushaPromotionReceiptValidationError::BlockDecode),
            "a serializable placeholder is not evidence that activation occurred",
        );
        assert_eq!(
            enforce_activation_receipt_frame_size(&decoded)
                .expect("count canonical receipt without allocating output"),
            canonical.len(),
        );

        #[cfg(feature = "json")]
        {
            let json = norito::json::to_json(&receipt).expect("encode complete receipt JSON");
            let decoded = KagemushaV4ActivationFinalityReceiptV1::decode_json_bounded(&json)
                .expect("bounded JSON receipt decode");
            assert_eq!(decoded, receipt);
            let hostile = json.replacen('{', "{\"unexpected\":true,", 1);
            assert_eq!(
                KagemushaV4ActivationFinalityReceiptV1::decode_json_bounded(&hostile),
                Err(KagemushaPromotionReceiptValidationError::ReceiptJsonDecode),
            );
        }
    }

    #[cfg(feature = "json")]
    #[test]
    fn block_json_preflight_rejects_oversize_before_base64_decode() {
        assert!(canonical_base64_decoded_len("YWI=", 1).is_err());
        assert!(canonical_base64_decoded_len("YWJ=", 2).is_err());
        assert_eq!(
            canonical_base64_decoded_len("YWI=", 2).expect("canonical base64 within limit"),
            2
        );
        assert_eq!(
            KagemushaFinalizedBlockWireV1::try_from_bytes(Vec::new()),
            Err(KagemushaPromotionReceiptValidationError::BlockWireSize {
                actual: 0,
                maximum: AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
            }),
        );
    }
}
