//! Authenticated native verification for Torii block-entry proofs.
//!
//! The JavaScript SDK's pure Merkle helper deliberately cannot establish a trust anchor. This
//! module accepts the exact executed block wire and Torii's canonical finality/proof archives,
//! verifies Sumeragi-v2 finality under an application-pinned network context, and only then asks
//! the data model to derive its non-serializable `TrustedBlockProofAnchor` capability.
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::{
        SignedBlock,
        consensus_v2::{HeightContext, HeightContextId},
        decode_versioned_signed_block,
        proofs::{
            AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1, BlockProofs,
            TrustedBlockProofAnchor,
        },
    },
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
    transaction::signed::TransactionEntrypoint,
};
use napi::bindgen_prelude::Buffer;
use napi_derive::napi;
use std::{fmt, str::FromStr as _};
/// First-release authenticated block-proof bridge version.
const AUTHENTICATED_BLOCK_PROOFS_VERSION_V1: u8 = 1;
/// Maximum canonical Norito bytes accepted for one bridge finality proof.
const AUTHENTICATED_BLOCK_PROOFS_MAX_FINALITY_PROOF_BYTES_V1: usize = 9 * 1024 * 1024;
/// Maximum canonical Norito bytes accepted for one block-proof response.
const AUTHENTICATED_BLOCK_PROOFS_MAX_PROOF_BYTES_V1: usize = 16 * 1024 * 1024;
/// Bounded inputs for one authenticated block-proof verification.
///
/// `previous_finality_proof_norito` is the optional last proof in the application's already-pinned
/// verifier state. When present, it must match `trusted_context_id` and the target proof must be
/// its immediate successor. When absent, the target proof itself must match `trusted_context_id`.
#[napi(object, use_nullable = true)]
pub struct JsAuthenticatedBlockProofInputV1 {
    /// Exact bridge ABI version. The first release requires `1`.
    pub version: u8,
    /// Application-pinned exact genesis-derived network identity.
    pub network_id: String,
    /// Application-pinned, marked 32-byte `HeightContextId`.
    pub trusted_context_id: Buffer,
    /// Application-selected, marked 32-byte transaction entrypoint hash.
    pub expected_entry_hash: Buffer,
    /// Optional canonical Norito `BridgeFinalityProof` for immediate-successor verification.
    pub previous_finality_proof_norito: Option<Buffer>,
    /// Canonical Norito `BridgeFinalityProof` for the target block.
    pub finality_proof_norito: Buffer,
    /// Exact canonical executed `SignedBlockWire` bytes for the target block.
    pub executed_block_wire: Buffer,
    /// Canonical Norito `BlockProofs` bytes returned by Torii.
    pub block_proofs_norito: Buffer,
}
/// Authenticated native verdict for one Torii `BlockProofs` response.
///
/// Finality is valid whenever this object is returned. `valid` additionally
/// states whether the requested entry/result proofs match the finality-bound
/// executed block. A malformed or unauthenticated input rejects the promise.
#[napi(object)]
pub struct JsAuthenticatedBlockProofVerdictV1 {
    /// Whether all entry, result, geometry, root, and transcript checks passed.
    pub valid: bool,
    /// Stable verdict code (`valid` or `block_proofs_mismatch`).
    pub code: String,
    /// Authenticated block height, rendered losslessly as a decimal string.
    pub block_height: String,
    /// Authenticated block-header hash in lowercase hexadecimal.
    pub block_hash_hex: String,
    /// Authenticated executed-block-wire hash in lowercase hexadecimal.
    pub executed_block_wire_hash_hex: String,
    /// Authenticated target entrypoint hash in lowercase hexadecimal.
    pub entry_hash_hex: String,
    /// Verified current height-context id for application successor state.
    pub height_context_id_hex: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerificationErrorCode {
    UnsupportedVersion,
    InvalidNetworkId,
    InvalidContextId,
    InvalidEntryHash,
    EmptyInput,
    InputTooLarge,
    NonCanonicalFinalityProof,
    NonCanonicalBlockProofs,
    NonCanonicalBlockWire,
    FinalityRejected,
    AnchorRejected,
}
impl VerificationErrorCode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "unsupported_version",
            Self::InvalidNetworkId => "invalid_network_id",
            Self::InvalidContextId => "invalid_context_id",
            Self::InvalidEntryHash => "invalid_entry_hash",
            Self::EmptyInput => "empty_input",
            Self::InputTooLarge => "input_too_large",
            Self::NonCanonicalFinalityProof => "noncanonical_finality_proof",
            Self::NonCanonicalBlockProofs => "noncanonical_block_proofs",
            Self::NonCanonicalBlockWire => "noncanonical_block_wire",
            Self::FinalityRejected => "finality_rejected",
            Self::AnchorRejected => "anchor_rejected",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct VerificationError {
    code: VerificationErrorCode,
    message: String,
}
impl VerificationError {
    fn new(code: VerificationErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl fmt::Display for VerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "authenticated_block_proofs_v1/{}: {}",
            self.code.as_str(),
            self.message
        )
    }
}
struct RawVerificationInputV1<'a> {
    version: u8,
    network_id: &'a str,
    trusted_context_id: &'a [u8],
    expected_entry_hash: &'a [u8],
    previous_finality_proof_norito: Option<&'a [u8]>,
    finality_proof_norito: &'a [u8],
    executed_block_wire: &'a [u8],
    block_proofs_norito: &'a [u8],
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct AuthenticatedBlockProofVerdictV1 {
    valid: bool,
    block_height: u64,
    block_hash_hex: String,
    executed_block_wire_hash_hex: String,
    entry_hash_hex: String,
    height_context_id_hex: String,
}
impl From<AuthenticatedBlockProofVerdictV1> for JsAuthenticatedBlockProofVerdictV1 {
    fn from(verdict: AuthenticatedBlockProofVerdictV1) -> Self {
        Self {
            valid: verdict.valid,
            code: if verdict.valid {
                "valid".to_owned()
            } else {
                "block_proofs_mismatch".to_owned()
            },
            block_height: verdict.block_height.to_string(),
            block_hash_hex: verdict.block_hash_hex,
            executed_block_wire_hash_hex: verdict.executed_block_wire_hash_hex,
            entry_hash_hex: verdict.entry_hash_hex,
            height_context_id_hex: verdict.height_context_id_hex,
        }
    }
}
/// Verify one Torii `BlockProofs` response through Rust-authenticated finality.
///
/// The CPU-heavy BLS and Merkle work runs outside the Node event loop. Every
/// archive is size-checked before decoding and must be an exact canonical V1
/// re-encoding. Cryptographic or binding failures reject the promise; a valid
/// finality chain carrying mismatched block proofs resolves to `valid: false`.
#[napi(js_name = "blockProofsVerifyAuthenticatedV1")]
pub async fn block_proofs_verify_authenticated_v1(
    input: JsAuthenticatedBlockProofInputV1,
) -> napi::Result<JsAuthenticatedBlockProofVerdictV1> {
    tokio::task::spawn_blocking(move || {
        verify_raw_v1(RawVerificationInputV1 {
            version: input.version,
            network_id: &input.network_id,
            trusted_context_id: input.trusted_context_id.as_ref(),
            expected_entry_hash: input.expected_entry_hash.as_ref(),
            previous_finality_proof_norito: input.previous_finality_proof_norito.as_deref(),
            finality_proof_norito: input.finality_proof_norito.as_ref(),
            executed_block_wire: input.executed_block_wire.as_ref(),
            block_proofs_norito: input.block_proofs_norito.as_ref(),
        })
        .map(JsAuthenticatedBlockProofVerdictV1::from)
    })
    .await
    .map_err(|error| {
        napi::Error::new(
            napi::Status::GenericFailure,
            format!("authenticated BlockProofs verifier task failed: {error}"),
        )
    })?
    .map_err(|error| napi::Error::new(napi::Status::InvalidArg, error.to_string()))
}
fn verify_raw_v1(
    input: RawVerificationInputV1<'_>,
) -> Result<AuthenticatedBlockProofVerdictV1, VerificationError> {
    if input.version != AUTHENTICATED_BLOCK_PROOFS_VERSION_V1 {
        return Err(VerificationError::new(
            VerificationErrorCode::UnsupportedVersion,
            format!(
                "version {} is unsupported; expected {AUTHENTICATED_BLOCK_PROOFS_VERSION_V1}",
                input.version
            ),
        ));
    }
    let network_id = NetworkId::from_str(input.network_id).map_err(|error| {
        VerificationError::new(
            VerificationErrorCode::InvalidNetworkId,
            format!("network_id is not canonical: {error}"),
        )
    })?;
    if network_id.to_string() != input.network_id {
        return Err(VerificationError::new(
            VerificationErrorCode::InvalidNetworkId,
            "network_id must use the canonical lowercase genesis-hash encoding",
        ));
    }
    let trusted_context_id = parse_height_context_id(input.trusted_context_id)?;
    let expected_entry_hash = parse_entry_hash(input.expected_entry_hash)?;
    let previous_finality = input
        .previous_finality_proof_norito
        .map(|bytes| decode_finality_proof(bytes, "previous_finality_proof_norito"))
        .transpose()?;
    let finality = decode_finality_proof(input.finality_proof_norito, "finality_proof_norito")?;
    let mut finality_verifier =
        BridgeFinalityVerifier::with_context(network_id, trusted_context_id);
    if let Some(previous) = previous_finality.as_ref() {
        finality_verifier.verify(previous).map_err(|error| {
            VerificationError::new(
                VerificationErrorCode::FinalityRejected,
                format!("pinned predecessor finality proof was rejected: {error}"),
            )
        })?;
    }
    finality_verifier.verify(&finality).map_err(|error| {
        VerificationError::new(
            VerificationErrorCode::FinalityRejected,
            format!("target finality proof was rejected: {error}"),
        )
    })?;
    // Authenticate the comparatively small finality inputs before allocating
    // or decoding the larger carriers. An invalid QC therefore cannot use
    // either field as an amplification stage. Decode and bind the block before
    // touching the proof archive so a wrong wire cannot amplify through it.
    let block = decode_executed_block_wire(input.executed_block_wire)?;
    // This is intentionally the only anchor construction path. The data-model
    // capability re-verifies the untrusted artifact, binds it to the decoded
    // header and exact executed wire, and recomputes all roots and transcripts.
    let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
        &block,
        &finality.finality_artifact,
        &expected_entry_hash,
    )
    .map_err(|error| {
        VerificationError::new(
            VerificationErrorCode::AnchorRejected,
            format!("finality-bound executed block could not derive an anchor: {error}"),
        )
    })?;
    let block_proofs = decode_block_proofs(input.block_proofs_norito)?;
    Ok(AuthenticatedBlockProofVerdictV1 {
        valid: block_proofs.verify(&anchor),
        block_height: anchor.block_height().get(),
        block_hash_hex: hex::encode(anchor.block_hash().as_ref()),
        executed_block_wire_hash_hex: hex::encode(anchor.executed_block_wire_hash().as_ref()),
        entry_hash_hex: hex::encode(anchor.entry_hash().as_ref()),
        height_context_id_hex: hex::encode(finality.finality_artifact.context_id().0.as_ref()),
    })
}
fn parse_height_context_id(bytes: &[u8]) -> Result<HeightContextId, VerificationError> {
    parse_marked_hash(
        bytes,
        "trusted_context_id",
        VerificationErrorCode::InvalidContextId,
    )
    .map(|hash| HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(hash)))
}
fn parse_entry_hash(bytes: &[u8]) -> Result<HashOf<TransactionEntrypoint>, VerificationError> {
    parse_marked_hash(
        bytes,
        "expected_entry_hash",
        VerificationErrorCode::InvalidEntryHash,
    )
    .map(HashOf::<TransactionEntrypoint>::from_untyped_unchecked)
}
fn parse_marked_hash(
    bytes: &[u8],
    label: &'static str,
    code: VerificationErrorCode,
) -> Result<Hash, VerificationError> {
    let exact: [u8; Hash::LENGTH] = bytes.try_into().map_err(|_| {
        VerificationError::new(
            code,
            format!(
                "{label} must contain exactly {} marked hash bytes",
                Hash::LENGTH
            ),
        )
    })?;
    let hash = Hash::from_str(&hex::encode(exact)).map_err(|error| {
        VerificationError::new(code, format!("{label} is not a marked Iroha hash: {error}"))
    })?;
    Ok(hash)
}
fn decode_finality_proof(
    bytes: &[u8],
    label: &'static str,
) -> Result<BridgeFinalityProof, VerificationError> {
    enforce_archive_size(
        bytes,
        AUTHENTICATED_BLOCK_PROOFS_MAX_FINALITY_PROOF_BYTES_V1,
        label,
    )?;
    decode_canonical_archive(
        bytes,
        label,
        VerificationErrorCode::NonCanonicalFinalityProof,
    )
}
fn decode_block_proofs(bytes: &[u8]) -> Result<BlockProofs, VerificationError> {
    const LABEL: &str = "block_proofs_norito";
    enforce_archive_size(bytes, AUTHENTICATED_BLOCK_PROOFS_MAX_PROOF_BYTES_V1, LABEL)?;
    decode_canonical_archive(bytes, LABEL, VerificationErrorCode::NonCanonicalBlockProofs)
}
fn decode_canonical_archive<T>(
    bytes: &[u8],
    label: &'static str,
    code: VerificationErrorCode,
) -> Result<T, VerificationError>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let limits = authenticated_decode_limits(bytes.len());
    norito::decode_canonical_with_limits(bytes, limits).map_err(|error| {
        VerificationError::new(
            code,
            format!("{label} is not bounded canonical Norito: {error}"),
        )
    })
}
fn decode_executed_block_wire(bytes: &[u8]) -> Result<SignedBlock, VerificationError> {
    const LABEL: &str = "executed_block_wire";
    enforce_archive_size(
        bytes,
        AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
        LABEL,
    )?;
    let limits = authenticated_decode_limits(bytes.len());
    let block = norito::core::with_decode_limits(limits, || {
        decode_versioned_signed_block(bytes)
            .map_err(|error| norito::core::Error::Message(error.to_string()))
    })
    .map_err(|error| {
        VerificationError::new(
            VerificationErrorCode::NonCanonicalBlockWire,
            format!("{LABEL} did not decode as SignedBlockWire: {error}"),
        )
    })?;
    let canonical = block.encode_wire().map_err(|error| {
        VerificationError::new(
            VerificationErrorCode::NonCanonicalBlockWire,
            format!("{LABEL} could not be canonically re-encoded: {error}"),
        )
    })?;
    if canonical != bytes {
        return Err(VerificationError::new(
            VerificationErrorCode::NonCanonicalBlockWire,
            format!("{LABEL} is not its exact canonical SignedBlockWire re-encoding"),
        ));
    }
    Ok(block)
}
fn authenticated_decode_limits(encoded_len: usize) -> norito::DecodeLimits {
    let canonical = norito::canonical_decode_limits(encoded_len);
    norito::DecodeLimits::new(
        canonical.max_sequence_elements(),
        canonical.max_field_bytes(),
        canonical.max_total_elements(),
        encoded_len.saturating_mul(12).saturating_add(1024 * 1024),
        128,
    )
}
fn enforce_archive_size(
    bytes: &[u8],
    maximum: usize,
    label: &'static str,
) -> Result<(), VerificationError> {
    if bytes.is_empty() {
        return Err(VerificationError::new(
            VerificationErrorCode::EmptyInput,
            format!("{label} must not be empty"),
        ));
    }
    if bytes.len() > maximum {
        return Err(VerificationError::new(
            VerificationErrorCode::InputTooLarge,
            format!("{label} exceeds its {maximum}-byte limit"),
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, MerkleTreeCommitment, Signature, SignatureOf};
    use iroha_data_model::{
        account::AccountId,
        block::{
            BlockHeader, BlockSignature,
            consensus_v2::{
                BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
                ExecutionCommitment, GlobalPhase, HeightContext, PayloadEncoding,
                QuorumCertificate, ValidatorPower, Vote, finality::V2FinalityArtifact,
            },
            proofs::ExecutionReceiptProof,
        },
        bridge::{BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeFinalityProof},
        peer::PeerId,
        transaction::{
            FeePaymentIntent, TransactionResultInner,
            signed::{TransactionBuilder, TransactionResult},
        },
        trigger::DataTriggerSequence,
    };
    use std::num::NonZeroU64;
    const FIXTURE_NETWORK_ID: &str =
        "a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5";
    struct Fixture {
        block: SignedBlock,
        finality: BridgeFinalityProof,
        block_proofs: BlockProofs,
        alternate_block_proofs: BlockProofs,
        finality_keys: Vec<KeyPair>,
        trusted_context_id: [u8; Hash::LENGTH],
    }
    fn checked_keypair(algorithm: Algorithm) -> KeyPair {
        KeyPair::try_random_with_algorithm(algorithm)
            .unwrap_or_else(|error| panic!("{algorithm:?} fixture key generation failed: {error}"))
    }
    fn make_fixture() -> Fixture {
        let transaction_key = checked_keypair(Algorithm::Ed25519);
        let alternate_transaction_key = checked_keypair(Algorithm::Ed25519);
        let network_id: NetworkId = FIXTURE_NETWORK_ID
            .parse()
            .expect("fixture network identity");
        let transaction = TransactionBuilder::new(
            network_id,
            AccountId::new(transaction_key.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .try_sign(transaction_key.private_key())
        .expect("fixture transaction signature");
        let entry_hash = transaction.hash_as_entrypoint();
        let alternate_transaction = TransactionBuilder::new(
            network_id,
            AccountId::new(alternate_transaction_key.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .try_sign(alternate_transaction_key.private_key())
        .expect("alternate fixture transaction signature");
        let alternate_entry_hash = alternate_transaction.hash_as_entrypoint();
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(transaction_key.private_key(), header.hash())
                .expect("fixture block signature"),
        );
        let mut block =
            SignedBlock::presigned(signature, header, vec![transaction, alternate_transaction]);
        block
            .set_transaction_results(
                Vec::new(),
                &[entry_hash, alternate_entry_hash],
                vec![
                    TransactionResultInner::Ok(DataTriggerSequence::default()),
                    TransactionResultInner::Ok(DataTriggerSequence::default()),
                ],
            )
            .expect("fixture block results align");
        let block_proofs = block
            .proofs_for_entry_hash(&entry_hash)
            .expect("fixture block proof exists");
        let alternate_block_proofs = block
            .proofs_for_entry_hash(&alternate_entry_hash)
            .expect("alternate fixture block proof exists");
        let executed_block_wire = block
            .encode_wire()
            .expect("encode authenticated proof fixture block wire");
        let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"authenticated proof fixture parent state"),
            Hash::new(b"authenticated proof fixture post state"),
            Hash::new(b"authenticated proof fixture ordinary writes"),
            u64::try_from(executed_block_wire.len())
                .expect("authenticated proof fixture block wire length fits u64"),
            Hash::new(&executed_block_wire),
        );
        let (artifact, finality_keys) =
            finalized_artifact_for_block(&block, network_id, execution_commitment, None, 1);
        let trusted_context_id = *artifact.context_id().0.as_ref();
        let finality = BridgeFinalityProof {
            version: BRIDGE_FINALITY_PROOF_VERSION_V2,
            block_header: block.header(),
            finality_artifact: artifact,
        };
        Fixture {
            block,
            finality,
            block_proofs,
            alternate_block_proofs,
            finality_keys,
            trusted_context_id,
        }
    }
    fn finalized_artifact_for_block(
        block: &SignedBlock,
        network_id: NetworkId,
        execution_commitment: ExecutionCommitment,
        parent_commit_qc: Option<QuorumCertificate>,
        height: u64,
    ) -> (V2FinalityArtifact, Vec<KeyPair>) {
        let mut keys = (0..4)
            .map(|_| checked_keypair(Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| {
            PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
        });
        let roster = keys
            .iter()
            .map(|key| ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let validator_set_pops = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture validator PoP")
            })
            .collect::<Vec<_>>();
        let context = HeightContext {
            network_id,
            protocol_version: iroha_data_model::block::consensus_v2::PROTOCOL_VERSION,
            height,
            epoch: 0,
            epoch_end_height: 10,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc,
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"authenticated proof fixture nexus context"),
            execution_policy_hash: Hash::new(b"authenticated proof fixture execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
            leader_seed: [0xA7; 32],
        };
        let subject = BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash: block.hash(),
            payload_hash: block
                .canonical_proposal_wire_hash()
                .expect("fixture proposal wire hashes"),
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height,
            view: block.header().view_change_index(),
        };
        let commit_qc = signed_commit_qc(&context, subject, execution_commitment, round, &keys);
        let artifact = V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
        artifact.verify().expect("fixture finality verifies");
        artifact
            .validate_for_header(&block.header())
            .expect("fixture finality matches block header");
        (artifact, keys)
    }
    fn signed_commit_qc(
        _context: &HeightContext,
        subject: BlockSubject,
        execution_commitment: ExecutionCommitment,
        round: ConsensusRound,
        keys: &[KeyPair],
    ) -> QuorumCertificate {
        let signers = vec![0, 1, 2];
        let preimage = Vote {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = signers
            .iter()
            .map(|index| {
                Signature::try_new(keys[*index].private_key(), &preimage)
                    .expect("fixture commit vote signature")
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate fixture commit votes"),
        }
    }
    fn make_successor(parent: &BridgeFinalityProof, keys: &[KeyPair]) -> BridgeFinalityProof {
        let parent_artifact = &parent.finality_artifact;
        let height = parent_artifact.height + 1;
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero successor height"),
            Some(parent_artifact.block_hash),
            None,
            None,
            0,
            0,
        );
        let signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(keys[0].private_key(), header.hash())
                .expect("sign successor fixture header"),
        );
        let mut block = SignedBlock::presigned(signature, header, Vec::new());
        block
            .set_transaction_results(Vec::new(), &[], Vec::new())
            .expect("empty successor fixture accepts empty results");
        let executed_block_wire = block
            .encode_wire()
            .expect("encode successor fixture block wire");
        let context = HeightContext {
            network_id: parent_artifact.height_context.network_id,
            protocol_version: iroha_data_model::block::consensus_v2::PROTOCOL_VERSION,
            height,
            epoch: parent_artifact.height_context.epoch,
            epoch_end_height: parent_artifact.height_context.epoch_end_height,
            next_epoch_snapshot: None,
            mode: parent_artifact.height_context.mode,
            parent_commit_qc: Some(parent_artifact.commit_qc.clone()),
            snapshot_bootstrap: None,
            quorum: parent_artifact.height_context.quorum,
            roster: parent_artifact.height_context.roster.clone(),
            nexus_amx_context_hash: Hash::new(
                [b"successor nexus".as_slice(), &height.to_be_bytes()].concat(),
            ),
            execution_policy_hash: parent_artifact.height_context.execution_policy_hash,
            da_layout: parent_artifact.height_context.da_layout,
            leader_seed: parent_artifact.height_context.leader_seed,
        };
        let subject = BlockSubject {
            parent_block_hash: Some(parent_artifact.block_hash),
            block_hash: block.hash(),
            payload_hash: block
                .canonical_proposal_wire_hash()
                .expect("hash successor fixture proposal wire"),
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height,
            view: 0,
        };
        let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new([b"successor parent state".as_slice(), &height.to_be_bytes()].concat()),
            Hash::new([b"successor post state".as_slice(), &height.to_be_bytes()].concat()),
            Hash::new([b"successor writes".as_slice(), &height.to_be_bytes()].concat()),
            u64::try_from(executed_block_wire.len())
                .expect("successor fixture block wire length fits u64"),
            Hash::new(&executed_block_wire),
        );
        let commit_qc = signed_commit_qc(&context, subject, execution_commitment, round, keys);
        let artifact = V2FinalityArtifact::new(
            context,
            subject,
            commit_qc,
            parent_artifact.validator_set_pops.clone(),
        );
        artifact.verify().expect("successor finality verifies");
        let proof = BridgeFinalityProof {
            version: BRIDGE_FINALITY_PROOF_VERSION_V2,
            block_header: block.header(),
            finality_artifact: artifact,
        };
        let mut verifier = BridgeFinalityVerifier::with_context(
            parent_artifact.height_context.network_id,
            parent_artifact.context_id(),
        );
        verifier.verify(parent).expect("fixture parent verifies");
        verifier.verify(&proof).expect("fixture successor verifies");
        proof
    }
    fn verify_typed(
        fixture: &Fixture,
        previous: Option<&BridgeFinalityProof>,
        finality: &BridgeFinalityProof,
        block: &SignedBlock,
        block_proofs: &BlockProofs,
        network_id: &str,
        trusted_context_id: &[u8],
    ) -> Result<AuthenticatedBlockProofVerdictV1, VerificationError> {
        verify_typed_with_expected(
            previous,
            finality,
            block,
            block_proofs,
            network_id,
            trusted_context_id,
            &fixture.block_proofs.entry_hash,
        )
    }
    fn verify_typed_with_expected(
        previous: Option<&BridgeFinalityProof>,
        finality: &BridgeFinalityProof,
        block: &SignedBlock,
        block_proofs: &BlockProofs,
        network_id: &str,
        trusted_context_id: &[u8],
        expected_entry_hash: &HashOf<TransactionEntrypoint>,
    ) -> Result<AuthenticatedBlockProofVerdictV1, VerificationError> {
        let previous_bytes = previous
            .map(|proof| norito::encode_canonical(proof).expect("encode predecessor finality"));
        let finality_bytes =
            norito::encode_canonical(finality).expect("encode target finality proof");
        let block_wire = block.encode_wire().expect("encode executed block wire");
        let block_proof_bytes =
            norito::encode_canonical(block_proofs).expect("encode block proofs");
        verify_raw_v1(RawVerificationInputV1 {
            version: AUTHENTICATED_BLOCK_PROOFS_VERSION_V1,
            network_id,
            trusted_context_id,
            expected_entry_hash: expected_entry_hash.as_ref(),
            previous_finality_proof_norito: previous_bytes.as_deref(),
            finality_proof_norito: &finality_bytes,
            executed_block_wire: &block_wire,
            block_proofs_norito: &block_proof_bytes,
        })
    }
    #[test]
    fn real_finality_block_wire_and_proofs_produce_authenticated_verdict() {
        let fixture = make_fixture();
        let verdict = verify_typed(
            &fixture,
            None,
            &fixture.finality,
            &fixture.block,
            &fixture.block_proofs,
            FIXTURE_NETWORK_ID,
            &fixture.trusted_context_id,
        )
        .expect("valid authenticated block proof");
        assert!(verdict.valid);
        assert_eq!(verdict.block_height, 1);
        assert_eq!(
            verdict.height_context_id_hex,
            hex::encode(fixture.trusted_context_id)
        );
    }
    #[test]
    fn exported_boundary_authenticates_fixture() {
        let fixture = make_fixture();
        let input = JsAuthenticatedBlockProofInputV1 {
            version: AUTHENTICATED_BLOCK_PROOFS_VERSION_V1,
            network_id: FIXTURE_NETWORK_ID.to_owned(),
            trusted_context_id: Buffer::from(fixture.trusted_context_id.to_vec()),
            expected_entry_hash: Buffer::from(fixture.block_proofs.entry_hash.as_ref().to_vec()),
            previous_finality_proof_norito: None,
            finality_proof_norito: Buffer::from(
                norito::encode_canonical(&fixture.finality).expect("encode finality proof"),
            ),
            executed_block_wire: Buffer::from(
                fixture
                    .block
                    .encode_wire()
                    .expect("encode executed block wire"),
            ),
            block_proofs_norito: Buffer::from(
                norito::encode_canonical(&fixture.block_proofs).expect("encode block proofs"),
            ),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("build test runtime");
        let verdict = runtime
            .block_on(block_proofs_verify_authenticated_v1(input))
            .expect("authenticated exported boundary");
        assert!(verdict.valid);
        assert_eq!(verdict.code, "valid");
        assert_eq!(verdict.block_height, "1");
        assert_eq!(
            verdict.height_context_id_hex,
            hex::encode(fixture.trusted_context_id)
        );
    }
    #[test]
    fn forged_qc_pop_and_roster_fail_before_anchor_derivation() {
        let fixture = make_fixture();
        let mut forged_qc = fixture.finality.clone();
        forged_qc.finality_artifact.commit_qc.aggregate_signature[0] ^= 0x80;
        assert_rejected_finality(&fixture, &forged_qc);
        let forged_qc_bytes =
            norito::encode_canonical(&forged_qc).expect("encode forged finality fixture");
        let preflight_error = verify_raw_v1(RawVerificationInputV1 {
            version: AUTHENTICATED_BLOCK_PROOFS_VERSION_V1,
            network_id: FIXTURE_NETWORK_ID,
            trusted_context_id: &fixture.trusted_context_id,
            expected_entry_hash: fixture.block_proofs.entry_hash.as_ref(),
            previous_finality_proof_norito: None,
            finality_proof_norito: &forged_qc_bytes,
            executed_block_wire: &[],
            block_proofs_norito: &[],
        })
        .expect_err("forged finality must fail before carrier preflight");
        assert_eq!(
            preflight_error.code,
            VerificationErrorCode::FinalityRejected
        );
        let mut forged_pop = fixture.finality.clone();
        forged_pop.finality_artifact.validator_set_pops[0][0] ^= 0x80;
        assert_rejected_finality(&fixture, &forged_pop);
        let mut forged_roster = fixture.finality.clone();
        forged_roster
            .finality_artifact
            .height_context
            .roster
            .swap(0, 1);
        assert_rejected_finality(&fixture, &forged_roster);
    }
    fn assert_rejected_finality(fixture: &Fixture, finality: &BridgeFinalityProof) {
        let error = verify_typed(
            fixture,
            None,
            finality,
            &fixture.block,
            &fixture.block_proofs,
            FIXTURE_NETWORK_ID,
            &fixture.trusted_context_id,
        )
        .expect_err("forged finality must fail closed");
        assert_eq!(error.code, VerificationErrorCode::FinalityRejected);
    }
    #[test]
    fn wrong_network_context_header_height_and_wire_fail_closed() {
        let fixture = make_fixture();
        let wrong_network = verify_typed(
            &fixture,
            None,
            &fixture.finality,
            &fixture.block,
            &fixture.block_proofs,
            "b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5",
            &fixture.trusted_context_id,
        )
        .expect_err("wrong network must fail");
        assert_eq!(wrong_network.code, VerificationErrorCode::FinalityRejected);
        let wrong_context = Hash::new(b"untrusted height context");
        let wrong_context = verify_typed(
            &fixture,
            None,
            &fixture.finality,
            &fixture.block,
            &fixture.block_proofs,
            FIXTURE_NETWORK_ID,
            wrong_context.as_ref(),
        )
        .expect_err("wrong context must fail");
        assert_eq!(wrong_context.code, VerificationErrorCode::FinalityRejected);
        let mut wrong_header = fixture.finality.clone();
        wrong_header.block_header.set_view_change_index(1);
        assert_rejected_finality(&fixture, &wrong_header);
        let mut wrong_height = fixture.finality.clone();
        wrong_height.finality_artifact.height = 2;
        assert_rejected_finality(&fixture, &wrong_height);
        let other = make_fixture();
        let wrong_wire = verify_typed(
            &fixture,
            None,
            &fixture.finality,
            &other.block,
            &fixture.block_proofs,
            FIXTURE_NETWORK_ID,
            &fixture.trusted_context_id,
        )
        .expect_err("another canonical block wire must fail");
        assert_eq!(wrong_wire.code, VerificationErrorCode::AnchorRejected);
    }
    #[test]
    fn stale_and_skipped_successor_state_is_rejected() {
        let fixture = make_fixture();
        let stale = verify_typed(
            &fixture,
            Some(&fixture.finality),
            &fixture.finality,
            &fixture.block,
            &fixture.block_proofs,
            FIXTURE_NETWORK_ID,
            &fixture.trusted_context_id,
        )
        .expect_err("stale proof must fail");
        assert_eq!(stale.code, VerificationErrorCode::FinalityRejected);
        assert!(stale.message.contains("stale"));
        let height_two = make_successor(&fixture.finality, &fixture.finality_keys);
        let height_three = make_successor(&height_two, &fixture.finality_keys);
        let skipped = verify_typed(
            &fixture,
            Some(&fixture.finality),
            &height_three,
            &fixture.block,
            &fixture.block_proofs,
            FIXTURE_NETWORK_ID,
            &fixture.trusted_context_id,
        )
        .expect_err("skipped proof must fail");
        assert_eq!(skipped.code, VerificationErrorCode::FinalityRejected);
        assert!(skipped.message.contains("advances past"));
    }
    #[test]
    fn root_geometry_result_and_transcript_mutations_return_invalid_verdicts() {
        let fixture = make_fixture();
        let assert_invalid = |proofs: &BlockProofs| {
            let verdict = verify_typed(
                &fixture,
                None,
                &fixture.finality,
                &fixture.block,
                proofs,
                FIXTURE_NETWORK_ID,
                &fixture.trusted_context_id,
            )
            .expect("valid finality with invalid BlockProofs returns a verdict");
            assert!(!verdict.valid);
        };
        let mut wrong_root = fixture.block_proofs.clone();
        wrong_root.entry_commitment = MerkleTreeCommitment::new(
            HashOf::from_untyped_unchecked(Hash::new(b"forged entry root")),
            wrong_root.entry_commitment.leaf_count(),
        );
        assert_invalid(&wrong_root);
        let mut wrong_geometry = fixture.block_proofs.clone();
        wrong_geometry.entry_commitment = MerkleTreeCommitment::new(
            *wrong_geometry.entry_commitment.root(),
            NonZeroU64::new(wrong_geometry.entry_commitment.leaf_count().get() + 1)
                .expect("non-zero forged leaf count"),
        );
        assert_invalid(&wrong_geometry);
        let mut wrong_result = fixture.block_proofs.clone();
        wrong_result.result_proof = ExecutionReceiptProof::new(
            HashOf::<TransactionResult>::from_untyped_unchecked(Hash::new(b"forged result")),
            wrong_result.result_proof.proof().clone(),
        );
        assert_invalid(&wrong_result);
        let mut wrong_transcript = fixture.block_proofs.clone();
        wrong_transcript
            .fastpq_transcripts
            .insert(Hash::new(b"forged FASTPQ transcript key"), Vec::new());
        assert_invalid(&wrong_transcript);
    }
    #[test]
    fn another_valid_entry_proof_from_the_same_finalized_block_is_rejected() {
        let fixture = make_fixture();
        let verdict = verify_typed(
            &fixture,
            None,
            &fixture.finality,
            &fixture.block,
            &fixture.alternate_block_proofs,
            FIXTURE_NETWORK_ID,
            &fixture.trusted_context_id,
        )
        .expect("valid finality with a substituted proof returns a verdict");
        assert!(!verdict.valid);
        assert_eq!(
            verdict.entry_hash_hex,
            hex::encode(fixture.block_proofs.entry_hash.as_ref())
        );
    }
    #[test]
    fn raw_boundary_rejects_unmarked_context_noncanonical_archives_and_headerless_wire() {
        let fixture = make_fixture();
        let finality = norito::encode_canonical(&fixture.finality).expect("encode finality");
        let proofs = norito::encode_canonical(&fixture.block_proofs).expect("encode proofs");
        let wire = fixture.block.encode_wire().expect("encode wire");
        let mut unmarked_context = fixture.trusted_context_id;
        unmarked_context[Hash::LENGTH - 1] &= !1;
        let context_error = verify_raw_v1(RawVerificationInputV1 {
            version: AUTHENTICATED_BLOCK_PROOFS_VERSION_V1,
            network_id: FIXTURE_NETWORK_ID,
            trusted_context_id: &unmarked_context,
            expected_entry_hash: fixture.block_proofs.entry_hash.as_ref(),
            previous_finality_proof_norito: None,
            finality_proof_norito: &finality,
            executed_block_wire: &wire,
            block_proofs_norito: &proofs,
        })
        .expect_err("unmarked context hash must fail");
        assert_eq!(context_error.code, VerificationErrorCode::InvalidContextId);
        let mut unmarked_entry_hash = *fixture.block_proofs.entry_hash.as_ref();
        unmarked_entry_hash[Hash::LENGTH - 1] &= !1;
        let entry_hash_error = verify_raw_v1(RawVerificationInputV1 {
            version: AUTHENTICATED_BLOCK_PROOFS_VERSION_V1,
            network_id: FIXTURE_NETWORK_ID,
            trusted_context_id: &fixture.trusted_context_id,
            expected_entry_hash: &unmarked_entry_hash,
            previous_finality_proof_norito: None,
            finality_proof_norito: &finality,
            executed_block_wire: &wire,
            block_proofs_norito: &proofs,
        })
        .expect_err("unmarked expected entry hash must fail");
        assert_eq!(
            entry_hash_error.code,
            VerificationErrorCode::InvalidEntryHash
        );
        let wire_preflight_error = verify_raw_v1(RawVerificationInputV1 {
            version: AUTHENTICATED_BLOCK_PROOFS_VERSION_V1,
            network_id: FIXTURE_NETWORK_ID,
            trusted_context_id: &fixture.trusted_context_id,
            expected_entry_hash: fixture.block_proofs.entry_hash.as_ref(),
            previous_finality_proof_norito: None,
            finality_proof_norito: &finality,
            executed_block_wire: &[1],
            block_proofs_norito: &[],
        })
        .expect_err("invalid wire must fail before proof-archive preflight");
        assert_eq!(
            wire_preflight_error.code,
            VerificationErrorCode::NonCanonicalBlockWire
        );
        let mut noncanonical_finality = finality.clone();
        noncanonical_finality.push(0);
        let finality_error = verify_raw_v1(RawVerificationInputV1 {
            version: AUTHENTICATED_BLOCK_PROOFS_VERSION_V1,
            network_id: FIXTURE_NETWORK_ID,
            trusted_context_id: &fixture.trusted_context_id,
            expected_entry_hash: fixture.block_proofs.entry_hash.as_ref(),
            previous_finality_proof_norito: None,
            finality_proof_norito: &noncanonical_finality,
            executed_block_wire: &wire,
            block_proofs_norito: &proofs,
        })
        .expect_err("trailing finality byte must fail");
        assert_eq!(
            finality_error.code,
            VerificationErrorCode::NonCanonicalFinalityProof
        );
        let mut noncanonical_proofs = proofs.clone();
        noncanonical_proofs.push(0);
        let proof_error = verify_raw_v1(RawVerificationInputV1 {
            version: AUTHENTICATED_BLOCK_PROOFS_VERSION_V1,
            network_id: FIXTURE_NETWORK_ID,
            trusted_context_id: &fixture.trusted_context_id,
            expected_entry_hash: fixture.block_proofs.entry_hash.as_ref(),
            previous_finality_proof_norito: None,
            finality_proof_norito: &finality,
            executed_block_wire: &wire,
            block_proofs_norito: &noncanonical_proofs,
        })
        .expect_err("trailing block-proof byte must fail");
        assert_eq!(
            proof_error.code,
            VerificationErrorCode::NonCanonicalBlockProofs
        );
        let deframed = iroha_data_model::block::deframe_versioned_signed_block_bytes(&wire)
            .expect("deframe fixture wire");
        let wire_error = verify_raw_v1(RawVerificationInputV1 {
            version: AUTHENTICATED_BLOCK_PROOFS_VERSION_V1,
            network_id: FIXTURE_NETWORK_ID,
            trusted_context_id: &fixture.trusted_context_id,
            expected_entry_hash: fixture.block_proofs.entry_hash.as_ref(),
            previous_finality_proof_norito: None,
            finality_proof_norito: &finality,
            executed_block_wire: deframed.bare_versioned.as_ref(),
            block_proofs_norito: &proofs,
        })
        .expect_err("headerless block wire must fail");
        assert_eq!(
            wire_error.code,
            VerificationErrorCode::NonCanonicalBlockWire
        );
    }
    #[test]
    fn size_preflight_rejects_before_decode() {
        let oversized = vec![0; AUTHENTICATED_BLOCK_PROOFS_MAX_FINALITY_PROOF_BYTES_V1 + 1];
        let error = decode_finality_proof(&oversized, "oversized")
            .expect_err("oversized finality proof must fail before decode");
        assert_eq!(error.code, VerificationErrorCode::InputTooLarge);
        let empty = decode_block_proofs(&[]).expect_err("empty block proofs must fail");
        assert_eq!(empty.code, VerificationErrorCode::EmptyInput);
    }
}
