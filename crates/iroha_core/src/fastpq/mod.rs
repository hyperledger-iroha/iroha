//! FASTPQ-specific transcript helpers shared across the host.
pub mod lane;

use std::collections::BTreeMap;

use fastpq_prover::{
    OperationKind, PoseidonSponge, PublicInputs, StateTransition, TransitionBatch,
};
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
    role::{Role, RoleId},
};
use iroha_primitives::numeric::Numeric;
use iroha_zkp_halo2::poseidon as halo2_poseidon;
use norito::{codec::Encode as NoritoEncode, to_bytes};
use thiserror::Error;

const AUTHORITY_DIGEST_DOMAIN: &[u8] = b"iroha:fastpq:v1:authority|";
const TX_SET_HASH_DOMAIN: &[u8] = b"fastpq:v1:tx_set";
const PERMISSION_TABLE_NODE_DOMAIN: &[u8] = b"fastpq:v1:poseidon_node";
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
    Hash::prehashed(halo2_poseidon::hash_bytes(&preimage))
}

fn append_encoded<T: NoritoEncode>(buffer: &mut Vec<u8>, value: &T) {
    buffer.extend_from_slice(&value.encode());
}

/// Build a FASTPQ public input template for the supplied block witness.
#[must_use]
pub fn public_inputs_template_from_block(
    header: &BlockHeader,
    witness: &ExecWitness,
    perm_root: [u8; 32],
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
        perm_root,
    }
}

pub(crate) fn dataspace_id_bytes(dsid: DataSpaceId) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&dsid.as_u64().to_le_bytes());
    out
}

pub(crate) fn permission_table_root<'a, I>(roles: I) -> [u8; 32]
where
    I: IntoIterator<Item = (&'a RoleId, &'a Role)>,
{
    let mut entries = Vec::new();
    for (role_id, role) in roles {
        let role_bytes = hash_encoded(role_id);
        for permission in &role.permissions {
            let epoch = role.permission_epoch(permission).unwrap_or_default();
            entries.push(PermissionTableEntry {
                role_bytes,
                permission_bytes: hash_encoded(permission),
                epoch_bytes: epoch.to_le_bytes(),
            });
        }
    }
    if entries.is_empty() {
        return [0u8; 32];
    }
    entries.sort_unstable_by(|left, right| {
        (left.role_bytes, left.permission_bytes, left.epoch_bytes).cmp(&(
            right.role_bytes,
            right.permission_bytes,
            right.epoch_bytes,
        ))
    });
    let hashes: Vec<u64> = entries.iter().map(permission_hash_from_entry).collect();
    field_element_bytes(poseidon_merkle_root(&hashes))
}

#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_field_names)]
struct PermissionTableEntry {
    role_bytes: [u8; 32],
    permission_bytes: [u8; 32],
    epoch_bytes: [u8; 8],
}

fn hash_encoded<T: NoritoEncode>(value: &T) -> [u8; 32] {
    let hash = Hash::new(value.encode());
    hash.into()
}

fn permission_hash_from_entry(entry: &PermissionTableEntry) -> u64 {
    let mut payload = Vec::with_capacity(32 + 32 + 8);
    payload.extend_from_slice(&entry.role_bytes);
    payload.extend_from_slice(&entry.permission_bytes);
    payload.extend_from_slice(&entry.epoch_bytes);
    let packed = fastpq_prover::pack_bytes(&payload);
    fastpq_prover::hash_field_elements(&packed.limbs)
}

fn poseidon_merkle_root(leaves: &[u64]) -> u64 {
    if leaves.is_empty() {
        return 0;
    }
    let mut current = leaves.to_vec();
    while current.len() > 1 {
        if current.len() % 2 == 1 {
            let last = *current.last().expect("non-empty vector");
            current.push(last);
        }
        let mut next = Vec::with_capacity(current.len() / 2);
        for pair in current.chunks(2) {
            next.push(hash_field_with_domain(
                PERMISSION_TABLE_NODE_DOMAIN,
                &[pair[0], pair[1]],
            ));
        }
        current = next;
    }
    current[0]
}

fn hash_field_with_domain(domain: &[u8], values: &[u64]) -> u64 {
    let mut sponge = PoseidonSponge::new();
    sponge.absorb(domain_seed(domain));
    sponge.absorb_slice(values);
    sponge.squeeze()
}

fn domain_seed(domain: &[u8]) -> u64 {
    let digest = Hash::new(domain);
    let bytes = digest.as_ref();
    let mut chunk = [0u8; 8];
    chunk.copy_from_slice(&bytes[..8]);
    let raw = u64::from_le_bytes(chunk);
    let reduced = u128::from(raw) % u128::from(fastpq_prover::FIELD_MODULUS);
    u64::try_from(reduced).expect("modulus reduction fits u64")
}

fn field_element_bytes(value: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..8].copy_from_slice(&value.to_le_bytes());
    out
}

fn public_inputs_from_template(
    template: FastpqPublicInputsTemplate,
    tx_set_hash: [u8; 32],
) -> FastpqPublicInputs {
    template.with_tx_set_hash(tx_set_hash)
}

/// Compute a transaction set commitment from ordered entrypoint hashes.
pub(crate) fn tx_set_hash_from_ordered_hashes<I, H>(hashes: I) -> [u8; 32]
where
    I: IntoIterator<Item = H>,
    H: AsRef<[u8; 32]>,
{
    let iter = hashes.into_iter();
    let (lower, _) = iter.size_hint();
    let mut payload =
        Vec::with_capacity(TX_SET_HASH_DOMAIN.len() + lower.saturating_mul(Hash::LENGTH));
    payload.extend_from_slice(TX_SET_HASH_DOMAIN);
    for hash in iter {
        let bytes = hash.as_ref();
        payload.extend_from_slice(bytes);
    }
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
    tx_set_hash: [u8; 32],
    bundles: I,
) -> Result<Vec<TransitionBatch>, TranscriptBatchError>
where
    I: IntoIterator<Item = &'a TransferTranscriptBundle>,
{
    let mut batches = Vec::new();
    let public_inputs = public_inputs_from_template(public_inputs, tx_set_hash);
    for bundle in bundles {
        let mut batch = batch_from_transcripts(
            parameter_set.to_string(),
            public_inputs,
            &bundle.transcripts,
        )?;
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
    tx_set_hash: [u8; 32],
    transcripts: &BTreeMap<Hash, Vec<TransferTranscript>>,
) -> Result<Vec<FastpqTransitionBatch>, TranscriptBatchError> {
    let bundles: Vec<_> = transcripts
        .iter()
        .map(|(entry_hash, entries)| TransferTranscriptBundle {
            entry_hash: *entry_hash,
            transcripts: entries.clone(),
        })
        .collect();
    let batches = batches_from_bundles(parameter_set, public_inputs, tx_set_hash, bundles.iter())?;
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
        Registrable,
        block::{
            BlockHeader,
            consensus::{ExecKv, ExecWitness},
        },
        fastpq::{TransferTranscript, TransferTranscriptBundle},
        permission::Permission,
        role::{Role, RoleId},
    };
    use iroha_primitives::{json::Json, numeric::Numeric};
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use norito::decode_from_bytes;

    use super::*;

    #[test]
    fn authority_digest_matches_known_vector() {
        let digest = authority_digest(&ALICE_ID);
        assert_eq!(
            hex::encode(digest.as_ref()),
            "45d1b2a0e6781b8bd07d756065fe335e18d9523d8d2e2432b41c52a75f1ade5d"
        );
    }

    #[test]
    fn poseidon_digest_matches_known_vector() {
        let asset = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "rose".parse().unwrap(),
        );
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
            "3c5d1adcd9e0679321f50c7c38db4a8f177887ca43fc5bc51f6b4c6873aee32d"
        );
    }

    #[test]
    fn permission_table_root_is_order_independent() {
        let perm_a = Permission::new("perm_a".to_string(), Json::new(()));
        let perm_b = Permission::new("perm_b".to_string(), Json::new(()));
        let role_a: RoleId = "role_a".parse().expect("role id");
        let role_b: RoleId = "role_b".parse().expect("role id");
        let role_a = Role::new(role_a.clone(), (*ALICE_ID).clone())
            .add_permission(perm_a.clone())
            .add_permission(perm_b)
            .build(&ALICE_ID);
        let role_b = Role::new(role_b.clone(), (*ALICE_ID).clone())
            .add_permission(perm_a)
            .build(&ALICE_ID);
        let first = [
            (role_b.id.clone(), role_b.clone()),
            (role_a.id.clone(), role_a.clone()),
        ];
        let second = [
            (role_a.id.clone(), role_a.clone()),
            (role_b.id.clone(), role_b.clone()),
        ];
        let root_first = permission_table_root(first.iter().map(|(id, role)| (id, role)));
        let root_second = permission_table_root(second.iter().map(|(id, role)| (id, role)));
        assert_eq!(root_first, root_second);
        assert_ne!(root_first, [0u8; 32]);
    }

    #[test]
    fn permission_table_root_tracks_permission_epochs() {
        let perm = Permission::new("perm_epoch".to_string(), Json::new(()));
        let role_id: RoleId = "role_epoch".parse().expect("role id");
        let role_epoch_0 = Role::new(role_id.clone(), (*ALICE_ID).clone())
            .add_permission_with_epoch(perm.clone(), 0)
            .build(&ALICE_ID);
        let role_epoch_7 = Role::new(role_id.clone(), (*ALICE_ID).clone())
            .add_permission_with_epoch(perm.clone(), 7)
            .build(&ALICE_ID);
        let root_epoch_0 = permission_table_root(
            [(role_id.clone(), role_epoch_0)]
                .iter()
                .map(|(id, role)| (id, role)),
        );
        let root_epoch_7 = permission_table_root(
            [(role_id.clone(), role_epoch_7)]
                .iter()
                .map(|(id, role)| (id, role)),
        );
        assert_ne!(root_epoch_0, root_epoch_7);
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
        let perm_root = [0x11; 32];
        let template = public_inputs_template_from_block(&header, &witness, perm_root);
        let mut expected_dsid = [0u8; 16];
        expected_dsid[..8].copy_from_slice(&DataSpaceId::GLOBAL.as_u64().to_le_bytes());
        assert_eq!(template.dsid, expected_dsid);
        assert_eq!(template.slot, 123_000_000);
        assert_eq!(template.perm_root, perm_root);
        assert_eq!(
            template.old_root,
            <[u8; 32]>::from(crate::sumeragi::exec::parent_state_from_witness(&witness))
        );
        assert_eq!(
            template.new_root,
            <[u8; 32]>::from(crate::sumeragi::exec::post_state_from_witness(&witness))
        );
    }

    #[test]
    fn public_inputs_from_template_uses_tx_set_hash() {
        let template = sample_template();
        let tx_set_hash = [0x22; 32];
        let inputs = public_inputs_from_template(template, tx_set_hash);
        assert_eq!(inputs.tx_set_hash, tx_set_hash);
        assert_eq!(inputs.dsid, template.dsid);
        assert_eq!(inputs.slot, template.slot);
        assert_eq!(inputs.old_root, template.old_root);
        assert_eq!(inputs.new_root, template.new_root);
        assert_eq!(inputs.perm_root, template.perm_root);
    }

    #[test]
    fn tx_set_hash_from_ordered_hashes_matches_domain() {
        let first = Hash::prehashed([0x11; 32]);
        let second = Hash::prehashed([0x22; 32]);
        let tx_set_hash = tx_set_hash_from_ordered_hashes([first, second]);
        let mut payload = Vec::with_capacity(TX_SET_HASH_DOMAIN.len() + 2 * Hash::LENGTH);
        payload.extend_from_slice(TX_SET_HASH_DOMAIN);
        payload.extend_from_slice(first.as_ref());
        payload.extend_from_slice(second.as_ref());
        let expected: [u8; 32] = Hash::new(payload).into();
        assert_eq!(tx_set_hash, expected);
        let reversed = tx_set_hash_from_ordered_hashes([second, first]);
        assert_ne!(tx_set_hash, reversed);
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
        let batches = batches_from_bundles(
            FASTPQ_CANONICAL_PARAMETER_SET,
            sample_template(),
            sample_tx_set_hash(),
            [&bundle],
        )
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
        let built = batches_from_bundles(
            FASTPQ_CANONICAL_PARAMETER_SET,
            sample_template(),
            sample_tx_set_hash(),
            bundles,
        )
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
        let batches = dto_batches_from_transcripts(
            FASTPQ_CANONICAL_PARAMETER_SET,
            sample_template(),
            sample_tx_set_hash(),
            &map,
        )
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
        sample_template().with_tx_set_hash(sample_tx_set_hash())
    }

    fn sample_tx_set_hash() -> [u8; 32] {
        [0xCC; 32]
    }

    fn sample_transcript() -> TransferTranscript {
        let asset = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "rose".parse().unwrap(),
        );
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
