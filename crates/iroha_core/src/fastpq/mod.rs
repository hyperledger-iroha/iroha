//! FASTPQ-specific transcript helpers shared across the host.
pub mod lane;

use std::collections::BTreeMap;

use fastpq_prover::{OperationKind, PublicInputs, StateTransition, TransitionBatch};
use iroha_crypto::Hash;
use iroha_data_model::{
    DataSpaceId,
    account::AccountId,
    asset::id::AssetDefinitionId,
    block::{BlockHeader, consensus::ExecWitness},
    fastpq::{
        FastpqOperationKind, FastpqPublicInputs, FastpqRolePermissionDelta, FastpqStateTransition,
        FastpqTransitionBatch, TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript,
        TransferTranscript, TransferTranscriptBundle,
    },
};
use iroha_primitives::numeric::Numeric;
use iroha_zkp_halo2::poseidon;
use norito::{codec::Encode as NoritoEncode, to_bytes};
use thiserror::Error;

const AUTHORITY_DIGEST_DOMAIN: &[u8] = b"iroha:fastpq:v1:authority|";
const TX_SET_HASH_DOMAIN: &[u8] = b"fastpq:v1:tx_set";
/// Metadata key storing the originating entry hash for a batch.
pub const ENTRY_HASH_METADATA_KEY: &str = "entry_hash";
/// Metadata key storing the transcript count embedded in a batch.
pub const TRANSCRIPT_COUNT_METADATA_KEY: &str = "transcript_count";

/// Canonical FASTPQ parameter name used across the host and CLI helpers.
pub const FASTPQ_CANONICAL_PARAMETER_SET: &str = "fastpq-lane-balanced";

/// Base fields for FASTPQ public inputs shared across batches in a block.
#[derive(Debug, Clone, Copy)]
pub struct FastpqPublicInputsTemplate {
    /// Data-space identifier (little-endian UUID bytes).
    pub dsid: [u8; 16],
    /// Slot timestamp (nanoseconds since epoch).
    pub slot: u64,
    /// Sparse Merkle tree root before executing the batch.
    pub old_root: [u8; 32],
    /// Sparse Merkle tree root after executing the batch.
    pub new_root: [u8; 32],
    /// Permission table commitment for this slot.
    pub perm_root: [u8; 32],
}

impl FastpqPublicInputsTemplate {
    /// Build full public inputs using a precomputed transaction set hash.
    #[must_use]
    pub const fn with_tx_set_hash(self, tx_set_hash: [u8; 32]) -> FastpqPublicInputs {
        FastpqPublicInputs {
            dsid: self.dsid,
            slot: self.slot,
            old_root: self.old_root,
            new_root: self.new_root,
            perm_root: self.perm_root,
            tx_set_hash,
        }
    }
}

/// Errors that can occur while mapping transfer transcripts into FASTPQ transition batches.
#[derive(Debug, Error)]
pub enum TranscriptBatchError {
    /// Encountered a Numeric value that cannot be encoded as a 64-bit little-endian integer.
    #[error("numeric value `{value}` exceeds the supported 64-bit integer range or is fractional")]
    NumericEncoding {
        /// Numeric value that fell outside the FASTPQ prover's supported range.
        value: Numeric,
    },
    /// Norito serialization of transcript metadata failed.
    #[error("failed to encode transfer transcripts for gadget metadata")]
    MetadataEncoding {
        /// Underlying Norito error.
        #[from]
        source: norito::core::Error,
    },
    /// Execution witness does not carry precomputed FASTPQ batches.
    #[error("execution witness missing fastpq batches with public inputs")]
    MissingFastpqBatches,
}

/// Compute the canonical authority digest hashed by the host.
#[must_use]
pub fn authority_digest(authority: &AccountId) -> Hash {
    let mut payload = Vec::with_capacity(AUTHORITY_DIGEST_DOMAIN.len() + 96);
    payload.extend_from_slice(AUTHORITY_DIGEST_DOMAIN);
    payload.extend_from_slice(&authority.encode());
    Hash::new(payload)
}

/// Compute the Poseidon digest of a transfer delta preimage.
#[must_use]
pub fn poseidon_preimage_digest(delta: &TransferDeltaTranscript, batch_hash: &Hash) -> Hash {
    let mut preimage = Vec::with_capacity(256);
    append_encoded(&mut preimage, &delta.from_account);
    append_encoded(&mut preimage, &delta.to_account);
    append_encoded(&mut preimage, &delta.asset_definition);
    append_encoded(&mut preimage, &delta.amount);
    preimage.extend_from_slice(batch_hash.as_ref());
    Hash::prehashed(poseidon::hash_bytes(&preimage))
}

fn append_encoded<T: NoritoEncode>(buffer: &mut Vec<u8>, value: &T) {
    buffer.extend_from_slice(&value.encode());
}

/// Build a FASTPQ public input template for the supplied block witness.
#[must_use]
pub fn public_inputs_template_from_block(
    header: &BlockHeader,
    witness: &ExecWitness,
) -> FastpqPublicInputsTemplate {
    let creation_ms = u64::try_from(header.creation_time().as_millis()).unwrap_or(u64::MAX);
    let slot = creation_ms.saturating_mul(1_000_000);
    let old_root = crate::sumeragi::exec::parent_state_from_witness(witness);
    let new_root = crate::sumeragi::exec::post_state_from_witness(witness);
    FastpqPublicInputsTemplate {
        dsid: dataspace_id_bytes(DataSpaceId::GLOBAL),
        slot,
        old_root: old_root.into(),
        new_root: new_root.into(),
        // TODO: thread permission table commitments once the WSV wiring lands.
        perm_root: [0; 32],
    }
}

fn dataspace_id_bytes(dsid: DataSpaceId) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&dsid.as_u64().to_le_bytes());
    out
}

fn public_inputs_from_template(
    template: FastpqPublicInputsTemplate,
    entry_hash: &Hash,
) -> FastpqPublicInputs {
    let tx_set_hash = tx_set_hash_from_entry_hash(entry_hash);
    template.with_tx_set_hash(tx_set_hash)
}

fn tx_set_hash_from_entry_hash(entry_hash: &Hash) -> [u8; 32] {
    // TODO: replace entry-hash derivation with scheduler-provided tx set hash.
    let mut payload = Vec::with_capacity(TX_SET_HASH_DOMAIN.len() + Hash::LENGTH);
    payload.extend_from_slice(TX_SET_HASH_DOMAIN);
    payload.extend_from_slice(entry_hash.as_ref());
    Hash::new(payload).into()
}

/// Convert a collection of transfer transcripts into a canonical FASTPQ transition batch.
///
/// The caller is responsible for supplying `public_inputs` and threading metadata
/// (entry hash, transcript count, etc.) into the returned batch if required by downstream consumers.
///
/// # Errors
/// Returns [`TranscriptBatchError`] if any transcript fails to append to the batch.
pub fn batch_from_transcripts<'a, I>(
    parameter_set: impl Into<String>,
    public_inputs: FastpqPublicInputs,
    transcripts: I,
) -> Result<TransitionBatch, TranscriptBatchError>
where
    I: IntoIterator<Item = &'a TransferTranscript>,
{
    let transcripts: Vec<TransferTranscript> = transcripts.into_iter().cloned().collect();
    let mut batch = TransitionBatch::new(parameter_set, public_inputs_from_dto(&public_inputs));
    for transcript in &transcripts {
        append_transcript(&mut batch, transcript)?;
    }
    attach_transcript_metadata(&mut batch, transcripts)?;
    batch.sort();
    Ok(batch)
}

fn append_transcript(
    batch: &mut TransitionBatch,
    transcript: &TransferTranscript,
) -> Result<(), TranscriptBatchError> {
    for delta in &transcript.deltas {
        push_transfer_delta(batch, delta)?;
    }
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn attach_transcript_metadata(
    batch: &mut TransitionBatch,
    transcripts: Vec<TransferTranscript>,
) -> Result<(), TranscriptBatchError> {
    if transcripts.is_empty() {
        return Ok(());
    }
    let encoded = to_bytes(&transcripts)?;
    batch
        .metadata
        .insert(TRANSFER_TRANSCRIPTS_METADATA_KEY.into(), encoded);
    Ok(())
}

fn push_transfer_delta(
    batch: &mut TransitionBatch,
    delta: &TransferDeltaTranscript,
) -> Result<(), TranscriptBatchError> {
    let from_key = balance_key(&delta.asset_definition, &delta.from_account);
    let to_key = balance_key(&delta.asset_definition, &delta.to_account);
    let from_pre = encode_numeric_le(delta.from_balance_before.clone())?;
    let from_post = encode_numeric_le(delta.from_balance_after.clone())?;
    let to_pre = encode_numeric_le(delta.to_balance_before.clone())?;
    let to_post = encode_numeric_le(delta.to_balance_after.clone())?;

    batch.push(StateTransition::new(
        from_key,
        from_pre,
        from_post,
        OperationKind::Transfer,
    ));
    batch.push(StateTransition::new(
        to_key,
        to_pre,
        to_post,
        OperationKind::Transfer,
    ));
    Ok(())
}

fn balance_key(asset: &AssetDefinitionId, account: &AccountId) -> Vec<u8> {
    format!("asset/{asset}/{account}").into_bytes()
}

fn encode_numeric_le(value: Numeric) -> Result<Vec<u8>, TranscriptBatchError> {
    let integer: u64 = match value.clone().try_into() {
        Ok(v) => v,
        Err(_) => return Err(TranscriptBatchError::NumericEncoding { value }),
    };
    Ok(integer.to_le_bytes().to_vec())
}

/// Convert the FASTPQ batches stored in an [`ExecWitness`] into prover batches.
///
/// # Errors
/// Returns [`TranscriptBatchError::MissingFastpqBatches`] when transcripts are present
/// without prebuilt batches.
pub fn batches_from_exec_witness(
    witness: &ExecWitness,
) -> Result<Vec<TransitionBatch>, TranscriptBatchError> {
    if !witness.fastpq_batches.is_empty() {
        return Ok(witness
            .fastpq_batches
            .iter()
            .map(transition_batch_from_dto)
            .collect());
    }
    if witness.fastpq_transcripts.is_empty() {
        return Ok(Vec::new());
    }
    Err(TranscriptBatchError::MissingFastpqBatches)
}

/// Convert transcript bundles into FASTPQ batches, preserving execution order.
///
/// # Errors
/// Returns [`TranscriptBatchError`] if constructing a batch fails.
pub fn batches_from_bundles<'a, I>(
    parameter_set: &str,
    public_inputs: FastpqPublicInputsTemplate,
    bundles: I,
) -> Result<Vec<TransitionBatch>, TranscriptBatchError>
where
    I: IntoIterator<Item = &'a TransferTranscriptBundle>,
{
    let mut batches = Vec::new();
    for bundle in bundles {
        let inputs = public_inputs_from_template(public_inputs, &bundle.entry_hash);
        let mut batch =
            batch_from_transcripts(parameter_set.to_string(), inputs, &bundle.transcripts)?;
        annotate_metadata(&mut batch, &bundle.entry_hash, bundle.transcripts.len());
        batches.push(batch);
    }
    Ok(batches)
}

fn annotate_metadata(batch: &mut TransitionBatch, entry_hash: &Hash, transcript_count: usize) {
    batch
        .metadata
        .insert(ENTRY_HASH_METADATA_KEY.into(), entry_hash.as_ref().to_vec());
    batch.metadata.insert(
        TRANSCRIPT_COUNT_METADATA_KEY.into(),
        (transcript_count as u64).to_le_bytes().to_vec(),
    );
}

/// Convert a map of transcripts grouped by entry hash into DTO batches.
///
/// # Errors
/// Returns [`TranscriptBatchError`] if constructing the batches fails.
pub fn dto_batches_from_transcripts(
    parameter_set: &str,
    public_inputs: FastpqPublicInputsTemplate,
    transcripts: &BTreeMap<Hash, Vec<TransferTranscript>>,
) -> Result<Vec<FastpqTransitionBatch>, TranscriptBatchError> {
    let bundles: Vec<_> = transcripts
        .iter()
        .map(|(entry_hash, entries)| TransferTranscriptBundle {
            entry_hash: *entry_hash,
            transcripts: entries.clone(),
        })
        .collect();
    let batches = batches_from_bundles(parameter_set, public_inputs, bundles.iter())?;
    Ok(batches.iter().map(transition_batch_to_dto).collect())
}

/// Convert a prover batch into its DTO representation suitable for `ExecWitness`.
#[must_use]
pub fn transition_batch_to_dto(batch: &TransitionBatch) -> FastpqTransitionBatch {
    transition_batch_to_dto_ref(batch)
}

/// Convert a prover batch reference into a DTO (borrowing-friendly helper).
#[must_use]
pub fn transition_batch_to_dto_ref(batch: &TransitionBatch) -> FastpqTransitionBatch {
    let transitions = batch
        .transitions
        .iter()
        .map(state_transition_to_dto)
        .collect();
    FastpqTransitionBatch {
        parameter: batch.parameter.clone(),
        public_inputs: public_inputs_to_dto(&batch.public_inputs),
        transitions,
        metadata: batch.metadata.clone(),
    }
}

/// Convert a DTO batch back into the prover representation.
#[must_use]
pub fn transition_batch_from_dto(dto: &FastpqTransitionBatch) -> TransitionBatch {
    let mut batch = TransitionBatch::new(
        dto.parameter.clone(),
        public_inputs_from_dto(&dto.public_inputs),
    );
    for transition in &dto.transitions {
        batch.push(StateTransition::new(
            transition.key.clone(),
            transition.pre_value.clone(),
            transition.post_value.clone(),
            operation_from_dto(&transition.operation),
        ));
    }
    batch.metadata = dto.metadata.clone();
    batch
}

fn public_inputs_to_dto(inputs: &PublicInputs) -> FastpqPublicInputs {
    FastpqPublicInputs {
        dsid: inputs.dsid,
        slot: inputs.slot,
        old_root: inputs.old_root,
        new_root: inputs.new_root,
        perm_root: inputs.perm_root,
        tx_set_hash: inputs.tx_set_hash,
    }
}

fn public_inputs_from_dto(inputs: &FastpqPublicInputs) -> PublicInputs {
    PublicInputs {
        dsid: inputs.dsid,
        slot: inputs.slot,
        old_root: inputs.old_root,
        new_root: inputs.new_root,
        perm_root: inputs.perm_root,
        tx_set_hash: inputs.tx_set_hash,
    }
}

fn state_transition_to_dto(transition: &StateTransition) -> FastpqStateTransition {
    FastpqStateTransition {
        key: transition.key.clone(),
        pre_value: transition.pre_value.clone(),
        post_value: transition.post_value.clone(),
        operation: operation_to_dto(&transition.operation),
    }
}

fn operation_to_dto(operation: &OperationKind) -> FastpqOperationKind {
    match operation {
        OperationKind::Transfer => FastpqOperationKind::Transfer,
        OperationKind::Mint => FastpqOperationKind::Mint,
        OperationKind::Burn => FastpqOperationKind::Burn,
        OperationKind::RoleGrant {
            role_id,
            permission_id,
            epoch,
        } => FastpqOperationKind::RoleGrant(FastpqRolePermissionDelta {
            role_id: role_id.clone(),
            permission_id: permission_id.clone(),
            epoch: *epoch,
        }),
        OperationKind::RoleRevoke {
            role_id,
            permission_id,
            epoch,
        } => FastpqOperationKind::RoleRevoke(FastpqRolePermissionDelta {
            role_id: role_id.clone(),
            permission_id: permission_id.clone(),
            epoch: *epoch,
        }),
        OperationKind::MetaSet => FastpqOperationKind::MetaSet,
    }
}

fn operation_from_dto(operation: &FastpqOperationKind) -> OperationKind {
    match operation {
        FastpqOperationKind::Transfer => OperationKind::Transfer,
        FastpqOperationKind::Mint => OperationKind::Mint,
        FastpqOperationKind::Burn => OperationKind::Burn,
        FastpqOperationKind::RoleGrant(delta) => OperationKind::RoleGrant {
            role_id: delta.role_id.clone(),
            permission_id: delta.permission_id.clone(),
            epoch: delta.epoch,
        },
        FastpqOperationKind::RoleRevoke(delta) => OperationKind::RoleRevoke {
            role_id: delta.role_id.clone(),
            permission_id: delta.permission_id.clone(),
            epoch: delta.epoch,
        },
        FastpqOperationKind::MetaSet => OperationKind::MetaSet,
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, num::NonZeroU64, str::FromStr};

    use iroha_data_model::{
        block::{
            BlockHeader,
            consensus::{ExecKv, ExecWitness},
        },
        fastpq::{TransferTranscript, TransferTranscriptBundle},
    };
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use norito::decode_from_bytes;

    use super::*;

    #[test]
    fn authority_digest_matches_known_vector() {
        let digest = authority_digest(&ALICE_ID);
        assert_eq!(
            hex::encode(digest.as_ref()),
            "06afa345e4260488ec10a9d0fc7963dbdc41a7b428b81696ccbd50b1a5cc35b1"
        );
    }

    #[test]
    fn poseidon_digest_matches_known_vector() {
        let asset =
            iroha_data_model::asset::AssetDefinitionId::from_str("rose#wonderland").unwrap();
        let delta = TransferDeltaTranscript {
            from_account: (*ALICE_ID).clone(),
            to_account: (*BOB_ID).clone(),
            asset_definition: asset,
            amount: Numeric::from(42u32),
            from_balance_before: Numeric::from(200u32),
            from_balance_after: Numeric::from(158u32),
            to_balance_before: Numeric::from(1u32),
            to_balance_after: Numeric::from(43u32),
            from_merkle_proof: None,
            to_merkle_proof: None,
        };
        let batch_hash = Hash::prehashed([0x11; 32]);
        let digest = poseidon_preimage_digest(&delta, &batch_hash);
        assert_eq!(
            hex::encode(digest.as_ref()),
            "06d5ecd0480693dcfbb08b0e7b3f0ea217792e3bbee45b806bc8cfc4dc10ab2d"
        );
    }

    #[test]
    fn public_inputs_template_from_block_uses_header_and_roots() {
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("height"),
            None,
            None,
            None,
            123,
            0,
        );
        let witness = ExecWitness {
            reads: vec![ExecKv {
                key: b"key".to_vec(),
                value: b"old".to_vec(),
            }],
            writes: vec![ExecKv {
                key: b"key".to_vec(),
                value: b"new".to_vec(),
            }],
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };
        let template = public_inputs_template_from_block(&header, &witness);
        let mut expected_dsid = [0u8; 16];
        expected_dsid[..8].copy_from_slice(&DataSpaceId::GLOBAL.as_u64().to_le_bytes());
        assert_eq!(template.dsid, expected_dsid);
        assert_eq!(template.slot, 123_000_000);
        assert_eq!(template.perm_root, [0u8; 32]);
        assert_eq!(
            template.old_root,
            crate::sumeragi::exec::parent_state_from_witness(&witness).into()
        );
        assert_eq!(
            template.new_root,
            crate::sumeragi::exec::post_state_from_witness(&witness).into()
        );
    }

    #[test]
    fn public_inputs_from_template_derives_tx_set_hash() {
        let template = sample_template();
        let entry_hash = Hash::prehashed([0x22; 32]);
        let inputs = public_inputs_from_template(template, &entry_hash);
        let mut payload = Vec::with_capacity(TX_SET_HASH_DOMAIN.len() + Hash::LENGTH);
        payload.extend_from_slice(TX_SET_HASH_DOMAIN);
        payload.extend_from_slice(entry_hash.as_ref());
        let expected: [u8; 32] = Hash::new(payload).into();
        assert_eq!(inputs.tx_set_hash, expected);
        assert_eq!(inputs.dsid, template.dsid);
        assert_eq!(inputs.slot, template.slot);
        assert_eq!(inputs.old_root, template.old_root);
        assert_eq!(inputs.new_root, template.new_root);
        assert_eq!(inputs.perm_root, template.perm_root);
    }

    #[test]
    fn batch_from_transcripts_builds_transfer_rows() {
        let transcript = sample_transcript();
        let batch = batch_from_transcripts(
            "fastpq-lane-balanced",
            sample_public_inputs(),
            [&transcript],
        )
        .unwrap();
        assert_eq!(batch.transitions.len(), 2);
        let delta = &transcript.deltas[0];
        let sender_key = format!("asset/{}/{}", delta.asset_definition, delta.from_account);
        let receiver_key = format!("asset/{}/{}", delta.asset_definition, delta.to_account);

        let sender_row = batch
            .transitions
            .iter()
            .find(|row| row.key == sender_key.as_bytes())
            .expect("sender row present");
        assert_eq!(sender_row.operation_rank(), OperationKind::Transfer.rank());
        assert_eq!(decode_le(&sender_row.pre_value), 200);
        assert_eq!(decode_le(&sender_row.post_value), 158);

        let receiver_row = batch
            .transitions
            .iter()
            .find(|row| row.key == receiver_key.as_bytes())
            .expect("receiver row present");
        assert_eq!(decode_le(&receiver_row.pre_value), 1);
        assert_eq!(decode_le(&receiver_row.post_value), 43);
    }

    #[test]
    fn batch_from_transcripts_embeds_transfer_metadata() {
        let transcript = sample_transcript();
        let batch = batch_from_transcripts(
            FASTPQ_CANONICAL_PARAMETER_SET,
            sample_public_inputs(),
            [&transcript],
        )
        .unwrap();
        let encoded = batch
            .metadata
            .get(TRANSFER_TRANSCRIPTS_METADATA_KEY)
            .expect("transfer metadata");
        let decoded: Vec<TransferTranscript> =
            decode_from_bytes(encoded).expect("decode transcripts");
        assert_eq!(decoded, vec![transcript]);
    }

    #[test]
    fn batch_from_transcripts_rejects_fractional_values() {
        let mut transcript = sample_transcript();
        transcript.deltas[0].from_balance_before = Numeric::try_new(15, 1).unwrap();
        let err = batch_from_transcripts(
            "fastpq-lane-balanced",
            sample_public_inputs(),
            [&transcript],
        )
        .unwrap_err();
        assert!(matches!(err, TranscriptBatchError::NumericEncoding { .. }));
    }

    #[test]
    fn batches_from_bundles_add_metadata() {
        let bundle = sample_bundle(Hash::prehashed([0x33; 32]));
        let batches =
            batches_from_bundles(FASTPQ_CANONICAL_PARAMETER_SET, sample_template(), [&bundle])
                .expect("batches");
        assert_eq!(batches.len(), 1);
        let batch = &batches[0];
        let entry_hex = batch
            .metadata
            .get(ENTRY_HASH_METADATA_KEY)
            .map(hex::encode)
            .expect("entry hash metadata");
        assert_eq!(entry_hex, hex::encode(bundle.entry_hash.as_ref()));
        let transcript_count_bytes = batch
            .metadata
            .get(TRANSCRIPT_COUNT_METADATA_KEY)
            .expect("transcript count metadata");
        assert_eq!(
            decode_le(transcript_count_bytes),
            bundle.transcripts.len() as u64
        );
    }

    #[test]
    fn batches_from_exec_witness_match_bundle_order() {
        let bundle_a = sample_bundle(Hash::prehashed([0x41; 32]));
        let bundle_b = sample_bundle(Hash::prehashed([0x42; 32]));
        let bundles = [&bundle_a, &bundle_b];
        let built =
            batches_from_bundles(FASTPQ_CANONICAL_PARAMETER_SET, sample_template(), bundles)
                .expect("batches");
        let witness = ExecWitness {
            reads: Vec::new(),
            writes: Vec::new(),
            fastpq_transcripts: vec![bundle_a.clone(), bundle_b.clone()],
            fastpq_batches: built.iter().map(transition_batch_to_dto).collect(),
        };
        let batches = batches_from_exec_witness(&witness).expect("batches");
        assert_eq!(batches.len(), 2);
        let first_entry = hex::encode(
            batches[0]
                .metadata
                .get(ENTRY_HASH_METADATA_KEY)
                .expect("metadata"),
        );
        let second_entry = hex::encode(
            batches[1]
                .metadata
                .get(ENTRY_HASH_METADATA_KEY)
                .expect("metadata"),
        );
        assert_eq!(first_entry, hex::encode(bundle_a.entry_hash.as_ref()));
        assert_eq!(second_entry, hex::encode(bundle_b.entry_hash.as_ref()));
    }

    #[test]
    fn batches_from_exec_witness_rejects_missing_batches() {
        let bundle = sample_bundle(Hash::prehashed([0x43; 32]));
        let witness = ExecWitness {
            reads: Vec::new(),
            writes: Vec::new(),
            fastpq_transcripts: vec![bundle],
            fastpq_batches: Vec::new(),
        };
        let err = batches_from_exec_witness(&witness).expect_err("missing batches");
        assert!(matches!(err, TranscriptBatchError::MissingFastpqBatches));
    }

    #[test]
    fn batches_from_exec_witness_prefers_prebuilt_batches() {
        let transcript = sample_transcript();
        let batch = batch_from_transcripts(
            FASTPQ_CANONICAL_PARAMETER_SET,
            sample_public_inputs(),
            [&transcript],
        )
        .unwrap();
        let dto = transition_batch_to_dto(&batch);
        let witness = ExecWitness {
            reads: Vec::new(),
            writes: Vec::new(),
            fastpq_transcripts: Vec::new(),
            fastpq_batches: vec![dto],
        };
        let batches = batches_from_exec_witness(&witness).expect("batches");
        assert_eq!(batches.len(), 1);
        assert_eq!(
            dto_transitions(&batches[0].transitions),
            dto_transitions(&batch.transitions)
        );
    }

    #[test]
    fn transition_batch_dto_roundtrip_preserves_metadata() {
        let transcript = sample_transcript();
        let mut batch = batch_from_transcripts(
            FASTPQ_CANONICAL_PARAMETER_SET,
            sample_public_inputs(),
            [&transcript],
        )
        .unwrap();
        batch.public_inputs = PublicInputs {
            dsid: [0x11; 16],
            slot: 42,
            old_root: [0x22; 32],
            new_root: [0x33; 32],
            perm_root: [0x44; 32],
            tx_set_hash: [0x55; 32],
        };
        batch.metadata.insert("test".into(), vec![0xAA, 0xBB, 0xCC]);
        let dto = transition_batch_to_dto(&batch);
        let restored = transition_batch_from_dto(&dto);
        assert_eq!(restored.parameter, batch.parameter);
        assert_eq!(
            dto_transitions(&restored.transitions),
            dto_transitions(&batch.transitions)
        );
        assert_eq!(restored.public_inputs, batch.public_inputs);
        assert_eq!(restored.metadata, batch.metadata);
    }

    #[test]
    fn dto_batches_from_transcripts_embed_entry_hash() {
        let bundle = sample_bundle(Hash::prehashed([0x24; 32]));
        let mut map = BTreeMap::new();
        map.insert(bundle.entry_hash, bundle.transcripts.clone());
        let batches =
            dto_batches_from_transcripts(FASTPQ_CANONICAL_PARAMETER_SET, sample_template(), &map)
                .expect("dto");
        assert_eq!(batches.len(), 1);
        let entry_hex = hex::encode(
            batches[0]
                .metadata
                .get(ENTRY_HASH_METADATA_KEY)
                .expect("entry metadata"),
        );
        assert_eq!(entry_hex, hex::encode(Hash::prehashed([0x24; 32]).as_ref()));
    }

    fn dto_transitions(transitions: &[StateTransition]) -> Vec<FastpqStateTransition> {
        transitions.iter().map(state_transition_to_dto).collect()
    }

    fn decode_le(bytes: &[u8]) -> u64 {
        let mut chunk = [0u8; 8];
        chunk[..bytes.len()].copy_from_slice(bytes);
        u64::from_le_bytes(chunk)
    }

    fn sample_template() -> FastpqPublicInputsTemplate {
        FastpqPublicInputsTemplate {
            dsid: [0u8; 16],
            slot: 0,
            old_root: [0u8; 32],
            new_root: [0u8; 32],
            perm_root: [0u8; 32],
        }
    }

    fn sample_public_inputs() -> FastpqPublicInputs {
        sample_template().with_tx_set_hash([0u8; 32])
    }

    fn sample_transcript() -> TransferTranscript {
        let asset =
            iroha_data_model::asset::AssetDefinitionId::from_str("rose#wonderland").unwrap();
        TransferTranscript {
            batch_hash: Hash::prehashed([0xAA; 32]),
            deltas: vec![TransferDeltaTranscript {
                from_account: (*ALICE_ID).clone(),
                to_account: (*BOB_ID).clone(),
                asset_definition: asset,
                amount: Numeric::from(42u32),
                from_balance_before: Numeric::from(200u32),
                from_balance_after: Numeric::from(158u32),
                to_balance_before: Numeric::from(1u32),
                to_balance_after: Numeric::from(43u32),
                from_merkle_proof: None,
                to_merkle_proof: None,
            }],
            authority_digest: authority_digest(&ALICE_ID),
            poseidon_preimage_digest: None,
        }
    }

    fn sample_bundle(entry_hash: Hash) -> TransferTranscriptBundle {
        TransferTranscriptBundle {
            entry_hash,
            transcripts: vec![sample_transcript()],
        }
    }
}
