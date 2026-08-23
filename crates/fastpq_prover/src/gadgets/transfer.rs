//! Transfer gadget validation shared between the planner and trace builder.
use crate::{Error, OperationKind, StateTransition};
use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    asset::id::AssetDefinitionId,
    fastpq::{
        TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferSmtWitness,
        TransferTranscript, normalized_numeric_to_u64, transfer_asset_scales,
    },
};
use iroha_primitives::numeric::{Numeric, Quantity};
use iroha_zkp_halo2::poseidon;
use norito::{codec::Encode as NoritoEncode, decode_from_bytes};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
/// Height of the V1 transfer SMT used by the transfer gadget.
pub const TRANSFER_MERKLE_HEIGHT: usize = 32;
/// Witness describing a single transfer delta after validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferDeltaWitness {
    /// Source account.
    pub from_account: AccountId,
    /// Destination account.
    pub to_account: AccountId,
    /// Asset being transferred.
    pub asset_definition: AssetDefinitionId,
    /// Transfer amount.
    pub amount: u64,
    /// Sender balance before the transfer.
    pub from_balance_before: u64,
    /// Sender balance after the transfer.
    pub from_balance_after: u64,
    /// Receiver balance before the transfer.
    pub to_balance_before: u64,
    /// Receiver balance after the transfer.
    pub to_balance_after: u64,
    /// Poseidon digest used by the gadget commitment.
    pub poseidon_digest: Hash,
    /// SMT proofs captured from the host.
    pub smt_proof: TransferSmtProof,
}
/// Structured transcript input ready for the transfer gadget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferGadgetInput {
    /// Batch hash associated with the transcript.
    pub batch_hash: Hash,
    /// Host-provided digest of the authority set.
    pub authority_digest: Hash,
    /// Validated deltas covered by the transcript.
    pub deltas: Vec<TransferDeltaWitness>,
}
/// Lightweight summary of the transfer gadget workload scheduled for this batch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransferGadgetPlan {
    witnesses: Vec<TransferGadgetInput>,
    total_deltas: usize,
}
impl TransferGadgetPlan {
    /// Construct a plan from the validated witnesses.
    #[must_use]
    pub fn from_inputs(inputs: &[TransferGadgetInput]) -> Self {
        let total_deltas = inputs.iter().map(|input| input.deltas.len()).sum();
        Self {
            witnesses: inputs.to_vec(),
            total_deltas,
        }
    }
    /// Returns the total number of batches carrying transfer witnesses.
    #[must_use]
    pub fn batch_count(&self) -> usize {
        self.witnesses.len()
    }
    /// Returns the total number of transfer deltas covered by this plan.
    #[must_use]
    pub fn total_deltas(&self) -> usize {
        self.total_deltas
    }
    /// Returns the estimated gadget row budget used by the V1 planner.
    #[must_use]
    pub fn estimated_row_budget(&self) -> usize {
        const ROWS_PER_DELTA_ESTIMATE: usize = 2;
        self.total_deltas * ROWS_PER_DELTA_ESTIMATE
    }
    /// Borrow the structured witnesses scheduled for the gadget.
    #[must_use]
    pub fn witnesses(&self) -> &[TransferGadgetInput] {
        &self.witnesses
    }
}
/// Lookup key identifying a transfer row inside a FASTPQ batch.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransferRowKey {
    key: Vec<u8>,
    pre_value: Vec<u8>,
    post_value: Vec<u8>,
}
impl TransferRowKey {
    /// Construct a key from explicit components.
    #[must_use]
    pub fn new(key: Vec<u8>, pre_value: Vec<u8>, post_value: Vec<u8>) -> Self {
        Self {
            key,
            pre_value,
            post_value,
        }
    }
    /// Construct a key from a transition row.
    #[must_use]
    pub fn from_transition(transition: &StateTransition) -> Self {
        Self {
            key: transition.key.clone(),
            pre_value: transition.pre_value.clone(),
            post_value: transition.post_value.clone(),
        }
    }
}
/// Build an index of transfer proofs keyed by transition rows.
///
/// A row shape may legitimately recur after intervening updates to other leaves. Preserve every
/// proof in transcript order so callers do not silently replace an earlier Merkle path with the
/// later one.
#[must_use]
pub fn index_row_proofs(
    inputs: &[TransferGadgetInput],
) -> HashMap<TransferRowKey, VecDeque<TransferMerkleProof>> {
    let mut map = HashMap::new();
    for witness in inputs {
        for delta in &witness.deltas {
            let sender_key = balance_key(&delta.asset_definition, &delta.from_account);
            let receiver_key = balance_key(&delta.asset_definition, &delta.to_account);
            map.entry(TransferRowKey::new(
                sender_key.clone(),
                delta.from_balance_before.to_le_bytes().to_vec(),
                delta.from_balance_after.to_le_bytes().to_vec(),
            ))
            .or_insert_with(VecDeque::new)
            .push_back(delta.smt_proof.from.clone());
            map.entry(TransferRowKey::new(
                receiver_key.clone(),
                delta.to_balance_before.to_le_bytes().to_vec(),
                delta.to_balance_after.to_le_bytes().to_vec(),
            ))
            .or_insert_with(VecDeque::new)
            .push_back(delta.smt_proof.to.clone());
        }
    }
    map
}
/// Wrapper containing SMT update proofs for both participants of a transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferSmtProof {
    /// Sender Merkle proof emitted by the host.
    pub from: TransferMerkleProof,
    /// Receiver Merkle proof emitted by the host.
    pub to: TransferMerkleProof,
}
impl TransferSmtProof {
    fn from_transcript(
        delta: &TransferDeltaTranscript,
        snapshot: &BalanceSnapshot,
    ) -> Result<Self, Error> {
        let from = TransferMerkleProof::from_witness(&delta.from_smt_witness)?;
        from.verify_update(
            &balance_key(&delta.asset_definition, &delta.from_account),
            snapshot.from_before,
            snapshot.from_after,
            "sender",
        )?;
        let to = TransferMerkleProof::from_witness(&delta.to_smt_witness)?;
        to.verify_update(
            &balance_key(&delta.asset_definition, &delta.to_account),
            snapshot.to_before,
            snapshot.to_after,
            "receiver",
        )?;
        Ok(Self { from, to })
    }
    /// Returns true if both sender and receiver proofs are present.
    #[must_use]
    pub fn has_paired_paths(&self) -> bool {
        self.from.siblings.len() == self.to.siblings.len()
            && self.from.path_bits.len() == self.to.path_bits.len()
    }
}
/// Merkle proof payload describing the path for a single leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferMerkleProof {
    /// Root before applying this update.
    pub root_before: [u8; 32],
    /// Root after applying this update.
    pub root_after: [u8; 32],
    /// Bitset describing the direction taken at each level (LSB-first per byte).
    pub path_bits: Vec<u8>,
    /// Sibling node hashes encountered along the path.
    pub siblings: Vec<[u8; 32]>,
}
impl TransferMerkleProof {
    fn from_witness(witness: &TransferSmtWitness) -> Result<Self, Error> {
        let required_path_bytes = TRANSFER_MERKLE_HEIGHT.div_ceil(8);
        if witness.path_bits.len() != required_path_bytes {
            return Err(Error::TransferInvariant {
                details: format!(
                    "transfer Merkle proof has {} path byte(s), expected {required_path_bytes}",
                    witness.path_bits.len()
                ),
            });
        }
        if witness.siblings.len() != TRANSFER_MERKLE_HEIGHT {
            return Err(Error::TransferInvariant {
                details: format!(
                    "transfer Merkle proof has {} sibling(s), expected {TRANSFER_MERKLE_HEIGHT}",
                    witness.siblings.len()
                ),
            });
        }
        Ok(Self {
            root_before: witness.root_before,
            root_after: witness.root_after,
            path_bits: witness.path_bits.clone(),
            siblings: witness.siblings.clone(),
        })
    }
    fn verify_update(
        &self,
        key: &[u8],
        value_before: u64,
        value_after: u64,
        role: &'static str,
    ) -> Result<(), Error> {
        let expected_path = path_index(key).to_le_bytes();
        if self.path_bits.as_slice() != expected_path {
            return Err(Error::TransferInvariant {
                details: format!(
                    "transfer {role} SMT proof path does not match the authenticated balance key"
                ),
            });
        }
        let computed_before = self.compute_root(key, value_before);
        if computed_before.as_ref() != self.root_before.as_ref() {
            return Err(Error::TransferInvariant {
                details: format!("transfer {role} SMT proof does not authenticate the pre-root"),
            });
        }
        let computed_after = self.compute_root(key, value_after);
        if computed_after.as_ref() != self.root_after.as_ref() {
            return Err(Error::TransferInvariant {
                details: format!("transfer {role} SMT proof does not authenticate the post-root"),
            });
        }
        Ok(())
    }
    fn compute_root(&self, key: &[u8], balance: u64) -> Hash {
        let mut current = leaf_hash(key, balance);
        for level in 0..TRANSFER_MERKLE_HEIGHT {
            let sibling = Hash::prehashed(self.sibling(level));
            let (left, right) = if self.bit(level) == 0 {
                (current, sibling)
            } else {
                (sibling, current)
            };
            current = internal_hash(&left, &right);
        }
        current
    }
    /// Returns the bit value for the given depth.
    #[must_use]
    pub fn bit(&self, level: usize) -> u64 {
        if level >= TRANSFER_MERKLE_HEIGHT {
            return 0;
        }
        let byte_index = level / 8;
        let bit_index = level % 8;
        u64::from(
            self.path_bits
                .get(byte_index)
                .copied()
                .map_or(0, |byte| (byte >> bit_index) & 1),
        )
    }
    /// Returns the sibling hash for the given depth.
    #[must_use]
    pub fn sibling(&self, level: usize) -> [u8; 32] {
        if level >= TRANSFER_MERKLE_HEIGHT {
            return padding_hash(level).into();
        }
        self.siblings
            .get(level)
            .copied()
            .expect("validated transfer Merkle proof contains every sibling")
    }
}
const KEY_DOMAIN: &[u8] = b"fastpq:v1:smt:key|";
const VALUE_DOMAIN: &[u8] = b"fastpq:v1:smt:value|";
const LEAF_DOMAIN: &[u8] = b"fastpq:v1:smt:leaf|";
const NODE_DOMAIN: &[u8] = b"fastpq:v1:smt:node|";
const PAD_DOMAIN: &[u8] = b"fastpq:v1:smt:pad|";
pub(crate) fn leaf_hash(key: &[u8], balance: u64) -> Hash {
    let mut payload = Vec::with_capacity(LEAF_DOMAIN.len() + 64);
    payload.extend_from_slice(LEAF_DOMAIN);
    payload.extend_from_slice(key_hash(key).as_ref());
    payload.extend_from_slice(value_hash(balance).as_ref());
    Hash::new(payload)
}
pub(crate) fn internal_hash(left: &Hash, right: &Hash) -> Hash {
    let mut payload = Vec::with_capacity(NODE_DOMAIN.len() + 64);
    payload.extend_from_slice(NODE_DOMAIN);
    payload.extend_from_slice(left.as_ref());
    payload.extend_from_slice(right.as_ref());
    Hash::new(payload)
}
fn key_hash(key: &[u8]) -> Hash {
    let mut payload = Vec::with_capacity(KEY_DOMAIN.len() + key.len());
    payload.extend_from_slice(KEY_DOMAIN);
    payload.extend_from_slice(key);
    Hash::new(payload)
}
fn value_hash(balance: u64) -> Hash {
    let mut payload = Vec::with_capacity(VALUE_DOMAIN.len() + 8);
    payload.extend_from_slice(VALUE_DOMAIN);
    payload.extend_from_slice(&balance.to_le_bytes());
    Hash::new(payload)
}
fn padding_hash(level: usize) -> Hash {
    let mut payload = Vec::with_capacity(PAD_DOMAIN.len() + 8);
    payload.extend_from_slice(PAD_DOMAIN);
    payload.extend_from_slice(&(level as u64).to_le_bytes());
    Hash::new(payload)
}
/// Build real V1 SMT update witnesses for a sender debit followed by a receiver
/// credit in the same transfer delta.
///
/// # Errors
/// Returns [`Error::TransferInvariant`] if distinct balance keys collide in the
/// 32-bit V1 transfer tree or if a self-transfer credit does not start from the
/// sender's post-debit balance.
pub fn build_transfer_smt_witness_pair(
    sender_key: &[u8],
    sender_before: u64,
    sender_after: u64,
    receiver_key: &[u8],
    receiver_before: u64,
    receiver_after: u64,
) -> Result<(TransferSmtWitness, TransferSmtWitness), Error> {
    let mut state = TransferSmtState::default();
    state.insert(sender_key, sender_before)?;
    if sender_key != receiver_key {
        state.insert(receiver_key, receiver_before)?;
    }
    let sender = state.update_witness(sender_key, sender_before, sender_after)?;
    let receiver = state.update_witness(receiver_key, receiver_before, receiver_after)?;
    Ok((sender, receiver))
}
/// Attach chained V1 transfer SMT witnesses to all deltas in transcript order
/// and return the resulting `(old_root, new_root)` pair.
///
/// This helper is intended for fixture builders and execution-captured V1 batch materialization. It
/// does not synthesize independent row paths: every witness is derived from one shared transfer SMT
/// and the returned roots must be used as the batch public roots. Repairs are committed only after
/// the full sequence succeeds, so an error leaves every input transcript unchanged.
///
/// # Errors
/// Returns [`Error::TransferInvariant`] if a transcript is empty or carries an invalid supplied
/// digest, a balance cannot be normalized, two leaves collide in the 32-bit V1 transfer tree, or a
/// declared transfer delta is not arithmetically valid.
pub fn attach_transfer_smt_witnesses(
    transcripts: &mut [TransferTranscript],
) -> Result<([u8; 32], [u8; 32]), Error> {
    validate_transcript_structure_and_digests(transcripts)?;
    let asset_scales = transfer_asset_scales(transcripts);
    let mut state = TransferSmtState::default();
    let mut seeded_keys = BTreeSet::new();
    let mut delta_count = 0usize;
    for transcript in transcripts.iter() {
        for delta in &transcript.deltas {
            delta_count = delta_count.saturating_add(1);
            let scale = asset_scale(&asset_scales, delta);
            let from_key = balance_key(&delta.asset_definition, &delta.from_account);
            if seeded_keys.insert(from_key.clone()) {
                state.insert(
                    &from_key,
                    numeric_to_u64("from_balance_before", &delta.from_balance_before, scale)?,
                )?;
            }
            let to_key = balance_key(&delta.asset_definition, &delta.to_account);
            if seeded_keys.insert(to_key.clone()) {
                state.insert(
                    &to_key,
                    numeric_to_u64("to_balance_before", &delta.to_balance_before, scale)?,
                )?;
            }
        }
    }
    if delta_count == 0 {
        return Err(Error::TransferInvariant {
            details: "transfer SMT witness material requires at least one delta".into(),
        });
    }
    let old_root = state.root().into();
    let mut staged_updates = Vec::with_capacity(delta_count);
    for transcript in transcripts.iter() {
        for delta in &transcript.deltas {
            let scale = asset_scale(&asset_scales, delta);
            // Validate the declared transfer at its own precision before replacing stale balance
            // snapshots with the values chained from the live SMT state. Requiring every stale
            // balance to fit the stable asset scale here would prevent the repair below.
            let _ = BalanceSnapshot::from_delta(delta)?;
            let amount = numeric_to_u64("amount", &delta.amount, scale)?;
            let from_key = balance_key(&delta.asset_definition, &delta.from_account);
            let from_balance_before = state.current_value(&from_key)?;
            let from_balance_after =
                from_balance_before
                    .checked_sub(amount)
                    .ok_or_else(|| Error::TransferInvariant {
                        details: format!(
                            "sender balance underflow while chaining transfer SMT: before={from_balance_before}, amount={amount}"
                        ),
                    })?;
            let from_balance_before_quantity =
                Quantity::try_from_numeric(Numeric::new(from_balance_before, scale))
                    .expect("non-negative FASTPQ quantity");
            let from_balance_after_quantity =
                Quantity::try_from_numeric(Numeric::new(from_balance_after, scale))
                    .expect("non-negative FASTPQ quantity");
            let from_smt_witness =
                state.update_witness(&from_key, from_balance_before, from_balance_after)?;
            let to_key = balance_key(&delta.asset_definition, &delta.to_account);
            let to_balance_before = state.current_value(&to_key)?;
            let to_balance_after =
                to_balance_before
                    .checked_add(amount)
                    .ok_or_else(|| Error::TransferInvariant {
                        details: "receiver balance overflow while chaining transfer SMT".into(),
                    })?;
            let to_balance_before_quantity =
                Quantity::try_from_numeric(Numeric::new(to_balance_before, scale))
                    .expect("non-negative FASTPQ quantity");
            let to_balance_after_quantity =
                Quantity::try_from_numeric(Numeric::new(to_balance_after, scale))
                    .expect("non-negative FASTPQ quantity");
            let to_smt_witness =
                state.update_witness(&to_key, to_balance_before, to_balance_after)?;
            staged_updates.push(AttachedTransferDelta {
                from_balance_before: from_balance_before_quantity,
                from_balance_after: from_balance_after_quantity,
                to_balance_before: to_balance_before_quantity,
                to_balance_after: to_balance_after_quantity,
                from_smt_witness,
                to_smt_witness,
            });
        }
    }
    let new_root = state.root().into();
    let mut updates = staged_updates.into_iter();
    for transcript in transcripts {
        for delta in &mut transcript.deltas {
            let update = updates
                .next()
                .expect("one staged FASTPQ witness update per transfer delta");
            delta.from_balance_before = update.from_balance_before;
            delta.from_balance_after = update.from_balance_after;
            delta.to_balance_before = update.to_balance_before;
            delta.to_balance_after = update.to_balance_after;
            delta.from_smt_witness = update.from_smt_witness;
            delta.to_smt_witness = update.to_smt_witness;
        }
    }
    debug_assert!(updates.next().is_none());
    Ok((old_root, new_root))
}
struct AttachedTransferDelta {
    from_balance_before: Quantity,
    from_balance_after: Quantity,
    to_balance_before: Quantity,
    to_balance_after: Quantity,
    from_smt_witness: TransferSmtWitness,
    to_smt_witness: TransferSmtWitness,
}
struct TransferSmtState {
    levels: Vec<BTreeMap<u32, Hash>>,
    balances: BTreeMap<u32, (Vec<u8>, u64)>,
}
impl Default for TransferSmtState {
    fn default() -> Self {
        Self {
            levels: (0..=TRANSFER_MERKLE_HEIGHT)
                .map(|_| BTreeMap::new())
                .collect(),
            balances: BTreeMap::new(),
        }
    }
}
impl TransferSmtState {
    fn insert(&mut self, key: &[u8], value: u64) -> Result<(), Error> {
        let path = path_index(key);
        let leaf = leaf_hash(key, value);
        if let Some((existing_key, existing_value)) = self.balances.get(&path) {
            if existing_key.as_slice() != key || *existing_value != value {
                return Err(Error::TransferInvariant {
                    details: "transfer SMT key path collision".into(),
                });
            }
        } else {
            self.balances.insert(path, (key.to_vec(), value));
        }
        if let Some(existing) = self.levels[0].insert(path, leaf)
            && existing != leaf
        {
            return Err(Error::TransferInvariant {
                details: "transfer SMT key path collision".into(),
            });
        }
        self.recompute_path(path);
        Ok(())
    }
    fn current_value(&self, key: &[u8]) -> Result<u64, Error> {
        let path = path_index(key);
        match self.balances.get(&path) {
            Some((existing_key, value)) if existing_key.as_slice() == key => Ok(*value),
            Some(_) => Err(Error::TransferInvariant {
                details: "transfer SMT key path collision".into(),
            }),
            None => Err(Error::TransferInvariant {
                details: "transfer SMT key missing from seeded state".into(),
            }),
        }
    }
    fn update_witness(
        &mut self,
        key: &[u8],
        value_before: u64,
        value_after: u64,
    ) -> Result<TransferSmtWitness, Error> {
        let path = path_index(key);
        let expected_before = leaf_hash(key, value_before);
        if self.levels[0].get(&path) != Some(&expected_before) {
            return Err(Error::TransferInvariant {
                details: "transfer SMT pre-balance does not match current state".into(),
            });
        }
        match self.balances.get(&path) {
            Some((existing_key, value))
                if existing_key.as_slice() == key && *value == value_before => {}
            Some(_) => {
                return Err(Error::TransferInvariant {
                    details: "transfer SMT key path collision".into(),
                });
            }
            None => {
                return Err(Error::TransferInvariant {
                    details: "transfer SMT key missing from seeded state".into(),
                });
            }
        }
        let root_before: [u8; 32] = self.root().into();
        let siblings = self.siblings_for(path);
        let path_bits = path.to_le_bytes().to_vec();
        self.levels[0].insert(path, leaf_hash(key, value_after));
        self.balances.insert(path, (key.to_vec(), value_after));
        self.recompute_path(path);
        let root_after: [u8; 32] = self.root().into();
        Ok(TransferSmtWitness::new(
            root_before,
            root_after,
            path_bits,
            siblings,
        ))
    }
    fn siblings_for(&self, path: u32) -> Vec<[u8; 32]> {
        let mut siblings = Vec::with_capacity(TRANSFER_MERKLE_HEIGHT);
        let mut index = path;
        for level in 0..TRANSFER_MERKLE_HEIGHT {
            let sibling_index = index ^ 1;
            let sibling = self.levels[level]
                .get(&sibling_index)
                .copied()
                .unwrap_or_else(|| padding_hash(level));
            siblings.push(sibling.into());
            index >>= 1;
        }
        siblings
    }
    fn root(&self) -> Hash {
        if self.levels[0].is_empty() {
            return padding_hash(TRANSFER_MERKLE_HEIGHT);
        }
        self.levels[TRANSFER_MERKLE_HEIGHT]
            .get(&0)
            .copied()
            .unwrap_or_else(|| padding_hash(TRANSFER_MERKLE_HEIGHT))
    }
    fn recompute_path(&mut self, path: u32) {
        let mut index = path;
        for level in 0..TRANSFER_MERKLE_HEIGHT {
            let parent = index >> 1;
            let left = self.levels[level]
                .get(&(parent << 1))
                .copied()
                .unwrap_or_else(|| padding_hash(level));
            let right = self.levels[level]
                .get(&((parent << 1) | 1))
                .copied()
                .unwrap_or_else(|| padding_hash(level));
            self.levels[level + 1].insert(parent, internal_hash(&left, &right));
            index = parent;
        }
    }
}
fn path_index(key: &[u8]) -> u32 {
    let hash = key_hash(key);
    let bytes = hash.as_ref();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
/// Decode transfer transcripts embedded in the batch metadata.
///
/// # Errors
/// Returns an error when the metadata payload cannot be decoded or is not the
/// exact canonical Norito representation.
pub fn decode_transcripts(
    metadata: &BTreeMap<String, Vec<u8>>,
) -> Result<Option<Vec<TransferTranscript>>, Error> {
    let Some(encoded) = metadata.get(TRANSFER_TRANSCRIPTS_METADATA_KEY) else {
        return Ok(None);
    };
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let transcripts: Vec<TransferTranscript> =
        decode_from_bytes(encoded).map_err(|source| Error::TransferMetadataDecode { source })?;
    let canonical = norito::to_bytes(&transcripts).map_err(Error::Encode)?;
    if canonical.as_slice() != encoded.as_slice() {
        return Err(Error::TransferInvariant {
            details: "transfer transcript metadata must use canonical Norito bytes".into(),
        });
    }
    Ok(Some(transcripts))
}
/// Convert validated transcripts into structured gadget inputs.
///
/// # Errors
/// Returns [`Error::TransferInvariant`] when a supplied digest violates transcript policy or the
/// transcript fails structural, arithmetic, or SMT-root checks.
pub fn transcripts_to_witnesses(
    transcripts: &[TransferTranscript],
    expected_old_root: &[u8; 32],
    expected_new_root: &[u8; 32],
) -> Result<Vec<TransferGadgetInput>, Error> {
    validate_transcript_structure_and_digests(transcripts)?;
    let asset_scales = transfer_asset_scales(transcripts);
    let mut current_root = *expected_old_root;
    let mut inputs = Vec::with_capacity(transcripts.len());
    for transcript in transcripts {
        let authority_digest = transcript.authority_digest;
        let mut deltas = Vec::with_capacity(transcript.deltas.len());
        for delta in &transcript.deltas {
            let snapshot =
                BalanceSnapshot::from_delta_at_scale(delta, asset_scale(&asset_scales, delta))?;
            let poseidon_digest = compute_poseidon_digest(delta, &transcript.batch_hash);
            let smt_proof = TransferSmtProof::from_transcript(delta, &snapshot)?;
            require_root(smt_proof.from.root_before, current_root, "sender pre-root")?;
            current_root = smt_proof.from.root_after;
            require_root(smt_proof.to.root_before, current_root, "receiver pre-root")?;
            current_root = smt_proof.to.root_after;
            deltas.push(TransferDeltaWitness {
                from_account: delta.from_account.clone(),
                to_account: delta.to_account.clone(),
                asset_definition: delta.asset_definition.clone(),
                amount: snapshot.transfer_amount(),
                from_balance_before: snapshot.from_before,
                from_balance_after: snapshot.from_after,
                to_balance_before: snapshot.to_before,
                to_balance_after: snapshot.to_after,
                poseidon_digest,
                smt_proof,
            });
        }
        inputs.push(TransferGadgetInput {
            batch_hash: transcript.batch_hash,
            authority_digest,
            deltas,
        });
    }
    require_root(current_root, *expected_new_root, "final post-root")?;
    Ok(inputs)
}
fn require_root(actual: [u8; 32], expected: [u8; 32], label: &'static str) -> Result<(), Error> {
    if actual != expected {
        return Err(Error::TransferInvariant {
            details: format!("transfer SMT {label} mismatch"),
        });
    }
    Ok(())
}
/// Verify arithmetic and digest invariants for transfer transcripts.
///
/// # Errors
/// Returns an error if any transcript arithmetic or digest invariant fails.
pub fn verify_transcripts(
    transitions: &[StateTransition],
    transcripts: &[TransferTranscript],
) -> Result<(), Error> {
    validate_transcript_structure_and_digests(transcripts)?;
    if transcripts.is_empty() {
        return Ok(());
    }
    let asset_scales = transfer_asset_scales(transcripts);
    let mut transfer_rows = index_transfers(transitions);
    for transcript in transcripts {
        for delta in &transcript.deltas {
            let snapshot =
                BalanceSnapshot::from_delta_at_scale(delta, asset_scale(&asset_scales, delta))?;
            ensure_transfer_rows(&mut transfer_rows, transitions, delta, &snapshot)?;
        }
    }
    if !transfer_rows.is_empty() {
        let remaining = transfer_rows.values().map(VecDeque::len).sum::<usize>();
        return Err(Error::TransferInvariant {
            details: format!("transfer transcripts did not cover {remaining} transfer row(s)"),
        });
    }
    Ok(())
}
fn validate_transcript_structure_and_digests(
    transcripts: &[TransferTranscript],
) -> Result<(), Error> {
    for transcript in transcripts {
        match transcript.deltas.as_slice() {
            [] => {
                return Err(Error::TransferInvariant {
                    details: "transfer transcript must contain at least one delta".into(),
                });
            }
            [delta] => {
                if let Some(expected) = &transcript.poseidon_preimage_digest {
                    let actual = compute_poseidon_digest(delta, &transcript.batch_hash);
                    if &actual != expected {
                        return Err(Error::TransferInvariant {
                            details: format!(
                                "poseidon digest mismatch for transfer {} -> {} ({})",
                                delta.from_account, delta.to_account, delta.asset_definition
                            ),
                        });
                    }
                }
            }
            [_, _, ..] => {
                if transcript.poseidon_preimage_digest.is_some() {
                    return Err(Error::TransferInvariant {
                        details: "multi-delta transcripts must omit poseidon_preimage_digest until per-delta digests land".into(),
                    });
                }
            }
        }
    }
    Ok(())
}
fn ensure_transfer_rows(
    index: &mut HashMap<Vec<u8>, VecDeque<usize>>,
    transitions: &[StateTransition],
    delta: &TransferDeltaTranscript,
    snapshot: &BalanceSnapshot,
) -> Result<(), Error> {
    let sender_key = balance_key(&delta.asset_definition, &delta.from_account);
    take_matching_row(
        index,
        transitions,
        sender_key.as_slice(),
        snapshot.sender_before_bytes(),
        snapshot.sender_after_bytes(),
        "sender",
    )?;
    let receiver_key = balance_key(&delta.asset_definition, &delta.to_account);
    take_matching_row(
        index,
        transitions,
        receiver_key.as_slice(),
        snapshot.receiver_before_bytes(),
        snapshot.receiver_after_bytes(),
        "receiver",
    )
}
fn take_matching_row(
    index: &mut HashMap<Vec<u8>, VecDeque<usize>>,
    transitions: &[StateTransition],
    key: &[u8],
    expected_pre: [u8; 8],
    expected_post: [u8; 8],
    role: &'static str,
) -> Result<(), Error> {
    let mut matched = false;
    let mut remove_key = false;
    {
        let Some(entries) = index.get_mut(key) else {
            return Err(Error::TransferInvariant {
                details: format!(
                    "missing transfer row ({role}) for key {}",
                    String::from_utf8_lossy(key)
                ),
            });
        };
        let mut attempts = entries.len();
        while attempts > 0 {
            let idx = entries
                .pop_front()
                .expect("entries length matches attempts");
            let row = &transitions[idx];
            if row.pre_value.as_slice() == expected_pre
                && row.post_value.as_slice() == expected_post
            {
                matched = true;
                remove_key = entries.is_empty();
                break;
            }
            entries.push_back(idx);
            attempts -= 1;
        }
    }
    if !matched {
        return Err(Error::TransferInvariant {
            details: format!(
                "no transfer row ({role}) matched key {} and expected balances",
                String::from_utf8_lossy(key)
            ),
        });
    }
    if remove_key {
        index.remove(key);
    }
    Ok(())
}
fn index_transfers(transitions: &[StateTransition]) -> HashMap<Vec<u8>, VecDeque<usize>> {
    let mut map = HashMap::new();
    for (idx, transition) in transitions.iter().enumerate() {
        if matches!(transition.operation, OperationKind::Transfer) {
            map.entry(transition.key.clone())
                .or_insert_with(VecDeque::new)
                .push_back(idx);
        }
    }
    map
}
fn balance_key(asset: &AssetDefinitionId, account: &AccountId) -> Vec<u8> {
    format!("asset/{asset}/{account}").into_bytes()
}
/// Compute the Poseidon digest committed by a transfer transcript entry.
pub fn compute_poseidon_digest(delta: &TransferDeltaTranscript, batch_hash: &Hash) -> Hash {
    let mut hasher = poseidon::PoseidonByteHasher::new();
    append_encoded(&mut hasher, &delta.from_account);
    append_encoded(&mut hasher, &delta.to_account);
    append_encoded(&mut hasher, &delta.asset_definition);
    append_encoded(&mut hasher, &delta.amount);
    hasher.update(batch_hash.as_ref());
    Hash::prehashed(hasher.finalize())
}
fn append_encoded<W: std::io::Write>(writer: &mut W, value: &impl NoritoEncode) {
    value.encode_to(writer);
}
struct BalanceSnapshot {
    amount: u64,
    from_before: u64,
    from_after: u64,
    to_before: u64,
    to_after: u64,
}
impl BalanceSnapshot {
    fn from_delta(delta: &TransferDeltaTranscript) -> Result<Self, Error> {
        Self::from_delta_at_scale(delta, delta.normalized_scale())
    }
    fn from_delta_at_scale(
        delta: &TransferDeltaTranscript,
        target_scale: u32,
    ) -> Result<Self, Error> {
        let amount = numeric_to_u64("amount", &delta.amount, target_scale)?;
        let from_before = numeric_to_u64(
            "from_balance_before",
            &delta.from_balance_before,
            target_scale,
        )?;
        let from_after = numeric_to_u64(
            "from_balance_after",
            &delta.from_balance_after,
            target_scale,
        )?;
        let to_before =
            numeric_to_u64("to_balance_before", &delta.to_balance_before, target_scale)?;
        let to_after = numeric_to_u64("to_balance_after", &delta.to_balance_after, target_scale)?;
        if from_before < amount {
            return Err(Error::TransferInvariant {
                details: format!("sender balance underflow: before={from_before}, amount={amount}"),
            });
        }
        if from_after != from_before - amount {
            return Err(Error::TransferInvariant {
                details: format!(
                    "sender balance mismatch: before={from_before}, after={from_after}, amount={amount}"
                ),
            });
        }
        if to_before
            .checked_add(amount)
            .ok_or_else(|| Error::TransferInvariant {
                details: "receiver balance overflow during transfer".to_string(),
            })?
            != to_after
        {
            return Err(Error::TransferInvariant {
                details: format!(
                    "receiver balance mismatch: before={to_before}, after={to_after}, amount={amount}"
                ),
            });
        }
        if delta.from_account == delta.to_account
            && (to_before != from_after || to_after != from_before)
        {
            return Err(Error::TransferInvariant {
                details: format!(
                    "self-transfer legs do not chain: sender={from_before}->{from_after}, receiver={to_before}->{to_after}"
                ),
            });
        }
        Ok(Self {
            amount,
            from_before,
            from_after,
            to_before,
            to_after,
        })
    }
    fn sender_before_bytes(&self) -> [u8; 8] {
        self.from_before.to_le_bytes()
    }
    fn sender_after_bytes(&self) -> [u8; 8] {
        self.from_after.to_le_bytes()
    }
    fn receiver_before_bytes(&self) -> [u8; 8] {
        self.to_before.to_le_bytes()
    }
    fn receiver_after_bytes(&self) -> [u8; 8] {
        self.to_after.to_le_bytes()
    }
    fn transfer_amount(&self) -> u64 {
        self.amount
    }
}
fn asset_scale(scales: &BTreeMap<AssetDefinitionId, u32>, delta: &TransferDeltaTranscript) -> u32 {
    scales
        .get(&delta.asset_definition)
        .copied()
        .unwrap_or_else(|| delta.normalized_scale())
}
fn numeric_to_u64(field: &'static str, value: &Quantity, target_scale: u32) -> Result<u64, Error> {
    normalized_numeric_to_u64(value.as_numeric(), target_scale)
        .ok_or(Error::TransferNumericBounds { field })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OperationKind, StateTransition};
    use iroha_crypto::Hash;
    use iroha_data_model::{
        DomainId,
        asset::id::AssetDefinitionId,
        fastpq::{TransferDeltaTranscript, TransferTranscript},
    };
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use norito::to_bytes;
    #[test]
    fn decode_transcripts_absent_metadata() {
        let metadata = BTreeMap::new();
        assert!(decode_transcripts(&metadata).expect("decode").is_none());
    }
    #[test]
    fn decode_transcripts_round_trip() {
        let transcript = sample_transcript();
        let mut metadata = BTreeMap::new();
        metadata.insert(
            TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
            to_bytes(&vec![transcript.clone()]).expect("encode"),
        );
        let decoded = decode_transcripts(&metadata)
            .expect("decode")
            .expect("present");
        assert_eq!(decoded, vec![transcript]);
    }
    #[test]
    fn decode_transcripts_rejects_alternate_norito_layout() {
        let transcripts = vec![sample_transcript()];
        let canonical = to_bytes(&transcripts).expect("encode canonical transcripts");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            to_bytes(&transcripts).expect("encode alternate-layout transcripts")
        };
        assert_ne!(alternate, canonical);
        assert_eq!(
            decode_from_bytes::<Vec<TransferTranscript>>(&alternate)
                .expect("ordinary Norito accepts advertised alternate layout"),
            transcripts
        );
        let mut metadata = BTreeMap::new();
        metadata.insert(TRANSFER_TRANSCRIPTS_METADATA_KEY.into(), alternate);
        let err = decode_transcripts(&metadata).expect_err("alternate transcript layout must fail");
        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("canonical Norito"))
        );
    }
    #[test]
    fn decode_transcripts_accepts_empty_transcript_list() {
        let mut metadata = BTreeMap::new();
        metadata.insert(
            TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
            to_bytes(&Vec::<TransferTranscript>::new()).expect("encode empty transcripts"),
        );
        let decoded = decode_transcripts(&metadata)
            .expect("decode")
            .expect("metadata is present");
        assert!(decoded.is_empty());
    }
    #[test]
    fn decode_transcripts_rejects_malformed_metadata() {
        let mut metadata = BTreeMap::new();
        metadata.insert(TRANSFER_TRANSCRIPTS_METADATA_KEY.into(), vec![0xFF, 0x00]);
        let err = decode_transcripts(&metadata).expect_err("malformed transcript metadata");
        assert!(matches!(err, Error::TransferMetadataDecode { .. }));
    }
    #[test]
    fn verify_transcripts_checks_balances() {
        let transcript = sample_transcript();
        let transitions = sample_transitions(&transcript);
        let result = verify_transcripts(&transitions, &[transcript]);
        assert!(result.is_ok());
    }
    #[test]
    fn verify_transcripts_accepts_empty_transcript_set() {
        let transitions = vec![StateTransition::new(
            b"asset/uncovered/transfer".to_vec(),
            1_u64.to_le_bytes().to_vec(),
            2_u64.to_le_bytes().to_vec(),
            OperationKind::Transfer,
        )];
        verify_transcripts(&transitions, &[]).expect("empty transcript set is a no-op");
    }
    #[test]
    fn verify_transcripts_rejects_an_empty_transcript() {
        let mut transcript = sample_transcript();
        transcript.deltas.clear();
        transcript.poseidon_preimage_digest = None;

        let err = verify_transcripts(&[], &[transcript])
            .expect_err("a present transcript must contain a delta");
        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("at least one delta"))
        );
    }
    #[test]
    fn verify_transcripts_detects_sender_mismatch() {
        let mut transcript = sample_transcript();
        transcript.deltas[0].from_balance_after = Quantity::from(1u32);
        let transitions = sample_transitions(&transcript);
        let err = verify_transcripts(&transitions, &[transcript]).expect_err("must fail");
        assert!(matches!(err, Error::TransferInvariant { .. }));
    }
    #[test]
    fn verify_transcripts_rejects_sender_underflow() {
        let mut transcript = sample_transcript();
        transcript.deltas[0].amount = Quantity::from(201u32);
        transcript.deltas[0].from_balance_after = Quantity::from(0u32);
        transcript.deltas[0].to_balance_after = Quantity::from(202u32);
        let transitions = sample_transitions(&transcript);
        let err = verify_transcripts(&transitions, &[transcript]).expect_err("underflow fails");
        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("underflow"))
        );
    }
    #[test]
    fn verify_transcripts_rejects_receiver_mismatch() {
        let mut transcript = sample_transcript();
        transcript.deltas[0].to_balance_after = Quantity::from(44u32);
        let transitions = sample_transitions(&transcript);
        let err = verify_transcripts(&transitions, &[transcript]).expect_err("receiver mismatch");
        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("receiver balance mismatch"))
        );
    }
    #[test]
    fn verify_transcripts_rejects_receiver_overflow() {
        let mut transcript = sample_transcript();
        transcript.deltas[0].amount = Quantity::from(1u32);
        transcript.deltas[0].from_balance_after = Quantity::from(199u32);
        transcript.deltas[0].to_balance_before = Quantity::from(u64::MAX);
        transcript.deltas[0].to_balance_after = Quantity::from(u64::MAX);
        let transitions = sample_transitions(&transcript);
        let err = verify_transcripts(&transitions, &[transcript]).expect_err("overflow fails");
        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("overflow"))
        );
    }
    #[test]
    fn transfer_snapshot_rejects_unlinked_self_transfer_legs() {
        let mut transcript = sample_transcript();
        let delta = &mut transcript.deltas[0];
        delta.to_account = delta.from_account.clone();
        delta.to_balance_before = Quantity::from(158u32);
        delta.to_balance_after = Quantity::from(200u32);
        BalanceSnapshot::from_delta(delta).expect("canonical self-transfer legs chain");

        delta.to_balance_before = Quantity::from(1_000u32);
        delta.to_balance_after = Quantity::from(1_042u32);
        let err = BalanceSnapshot::from_delta(delta)
            .expect_err("independently balanced self-transfer legs must still chain");
        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("self-transfer legs do not chain"))
        );
    }
    #[test]
    fn transcript_quantity_domain_and_gadget_bounds_reject_invalid_amounts() {
        assert!(
            Quantity::try_from_numeric(Numeric::from(-1_i64)).is_err(),
            "negative transcript amounts must be unrepresentable"
        );
        let err = super::numeric_to_u64("amount", &Quantity::from(u128::MAX), 0)
            .expect_err("amount outside the V1 u64 gadget domain must fail");
        assert!(matches!(
            err,
            Error::TransferNumericBounds { field } if field == "amount"
        ));
    }
    #[test]
    fn verify_transcripts_detects_poseidon_mismatch() {
        let mut transcript = sample_transcript();
        transcript.poseidon_preimage_digest = Some(Hash::prehashed([0xAA; 32]));
        let transitions = sample_transitions(&transcript);
        let err = verify_transcripts(&transitions, &[transcript]).expect_err("digest mismatch");
        assert!(matches!(err, Error::TransferInvariant { .. }));
    }
    #[test]
    fn compute_poseidon_digest_matches_canonical_encoded_preimage() {
        let transcript = sample_transcript();
        let delta = &transcript.deltas[0];
        let mut preimage = Vec::new();
        preimage.extend_from_slice(&delta.from_account.encode());
        preimage.extend_from_slice(&delta.to_account.encode());
        preimage.extend_from_slice(&delta.asset_definition.encode());
        preimage.extend_from_slice(&delta.amount.encode());
        preimage.extend_from_slice(transcript.batch_hash.as_ref());
        assert_eq!(
            compute_poseidon_digest(delta, &transcript.batch_hash),
            Hash::prehashed(poseidon::hash_bytes(&preimage))
        );
    }
    #[test]
    fn verify_transcripts_rejects_unmatched_transfer_rows() {
        let transcript = sample_transcript();
        let mut transitions = sample_transitions(&transcript);
        transitions.push(StateTransition::new(
            b"asset/extra/row".to_vec(),
            0u64.to_le_bytes().to_vec(),
            1u64.to_le_bytes().to_vec(),
            OperationKind::Transfer,
        ));
        let err = verify_transcripts(&transitions, &[transcript]).expect_err("extra row fails");
        assert!(matches!(err, Error::TransferInvariant { .. }));
    }
    #[test]
    fn verify_transcripts_ignores_non_transfer_rows() {
        let transcript = sample_transcript();
        let mut transitions = sample_transitions(&transcript);
        transitions.push(StateTransition::new(
            b"asset/ignored/meta".to_vec(),
            b"before".to_vec(),
            b"after".to_vec(),
            OperationKind::MetaSet,
        ));
        verify_transcripts(&transitions, &[transcript]).expect("non-transfer rows are ignored");
    }
    #[test]
    fn verify_transcripts_rejects_sender_row_balance_mismatch() {
        let transcript = sample_transcript();
        let sender_key = balance_key(
            &transcript.deltas[0].asset_definition,
            &transcript.deltas[0].from_account,
        );
        let mut transitions = sample_transitions(&transcript);
        let sender = transitions
            .iter_mut()
            .find(|transition| transition.key == sender_key)
            .expect("sender transition");
        sender.post_value = 157_u64.to_le_bytes().to_vec();
        let err = verify_transcripts(&transitions, &[transcript]).expect_err("sender row mismatch");
        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("no transfer row (sender) matched"))
        );
    }
    #[test]
    fn verify_transcripts_rejects_missing_receiver_row() {
        let transcript = sample_transcript();
        let receiver_key = balance_key(
            &transcript.deltas[0].asset_definition,
            &transcript.deltas[0].to_account,
        );
        let mut transitions = sample_transitions(&transcript);
        transitions.retain(|transition| transition.key != receiver_key);
        let err = verify_transcripts(&transitions, &[transcript]).expect_err("receiver row absent");
        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("missing transfer row (receiver)"))
        );
    }
    fn sample_transcript() -> TransferTranscript {
        use iroha_test_samples::{ALICE_ID, BOB_ID};
        let alice = (*ALICE_ID).clone();
        let bob = (*BOB_ID).clone();
        let asset = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let mut delta = TransferDeltaTranscript {
            from_account: alice.clone(),
            to_account: bob.clone(),
            asset_definition: asset.clone(),
            amount: Quantity::from(42u32),
            from_balance_before: Quantity::from(200u32),
            from_balance_after: Quantity::from(158u32),
            to_balance_before: Quantity::from(1u32),
            to_balance_after: Quantity::from(43u32),
            from_smt_witness: TransferSmtWitness::default(),
            to_smt_witness: TransferSmtWitness::default(),
        };
        attach_delta_witnesses(&mut delta);
        let batch_hash = Hash::prehashed([0x11; 32]);
        let digest = compute_poseidon_digest(&delta, &batch_hash);
        TransferTranscript {
            batch_hash,
            deltas: vec![delta],
            authority_digest: Hash::new(b"authority"),
            poseidon_preimage_digest: Some(digest),
        }
    }
    fn sample_transitions(transcript: &TransferTranscript) -> Vec<StateTransition> {
        transcript
            .deltas
            .iter()
            .flat_map(|delta| {
                let target_scale = delta.normalized_scale();
                let sender = StateTransition::new(
                    balance_key(&delta.asset_definition, &delta.from_account),
                    numeric_to_le_bytes(&delta.from_balance_before, target_scale),
                    numeric_to_le_bytes(&delta.from_balance_after, target_scale),
                    OperationKind::Transfer,
                );
                let receiver = StateTransition::new(
                    balance_key(&delta.asset_definition, &delta.to_account),
                    numeric_to_le_bytes(&delta.to_balance_before, target_scale),
                    numeric_to_le_bytes(&delta.to_balance_after, target_scale),
                    OperationKind::Transfer,
                );
                [sender, receiver]
            })
            .collect()
    }
    #[test]
    fn transcripts_to_witnesses_emit_structured_witness() {
        let transcript = sample_transcript();
        let (old_root, new_root) = transcript_roots(&transcript);
        let inputs =
            transcripts_to_witnesses(&[transcript], &old_root, &new_root).expect("witnesses");
        assert_eq!(inputs.len(), 1);
        let gadget = &inputs[0];
        assert_eq!(gadget.deltas.len(), 1);
        let delta = &gadget.deltas[0];
        assert_eq!(delta.amount, 42);
        assert_eq!(delta.from_balance_before, 200);
        assert_eq!(delta.to_balance_after, 43);
        assert!(delta.smt_proof.has_paired_paths());
    }
    #[test]
    fn transfer_smt_witness_pair_chains_self_transfer_on_one_leaf() {
        let key = b"asset/rose/alice";
        let (sender, receiver) = build_transfer_smt_witness_pair(key, 200, 158, key, 158, 200)
            .expect("self-transfer debit and credit chain");

        assert_eq!(sender.root_after, receiver.root_before);
        assert_eq!(sender.root_before, receiver.root_after);
        TransferMerkleProof::from_witness(&sender)
            .expect("sender proof shape")
            .verify_update(key, 200, 158, "sender")
            .expect("sender proof authenticates debit");
        TransferMerkleProof::from_witness(&receiver)
            .expect("receiver proof shape")
            .verify_update(key, 158, 200, "receiver")
            .expect("receiver proof authenticates credit");
    }
    #[test]
    fn transfer_smt_witness_pair_rejects_unchained_self_transfer_credit() {
        let key = b"asset/rose/alice";
        let err = build_transfer_smt_witness_pair(key, 200, 158, key, 159, 201)
            .expect_err("self-transfer credit must start at the post-debit balance");

        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("pre-balance does not match current state"))
        );
    }
    #[test]
    fn transcripts_to_witnesses_accept_multi_delta_batches() {
        let transcript = sample_multi_transcript();
        let (old_root, new_root) = transcript_roots(&transcript);
        let witnesses =
            transcripts_to_witnesses(std::slice::from_ref(&transcript), &old_root, &new_root)
                .expect("witnesses");
        assert_eq!(witnesses.len(), 1);
        assert_eq!(witnesses[0].deltas.len(), transcript.deltas.len());
        assert!(
            witnesses[0]
                .deltas
                .iter()
                .all(|delta| delta.smt_proof.has_paired_paths())
        );
    }
    #[test]
    fn transcripts_to_witnesses_decodes_smt_witnesses() {
        let transcript = sample_transcript();
        let (old_root, new_root) = transcript_roots(&transcript);
        let witnesses =
            transcripts_to_witnesses(&[transcript], &old_root, &new_root).expect("witnesses");
        let smt = &witnesses[0].deltas[0].smt_proof;
        assert!(smt.has_paired_paths());
        assert_eq!(smt.from.path_bits.len(), TRANSFER_MERKLE_HEIGHT.div_ceil(8));
        assert_eq!(smt.to.siblings.len(), TRANSFER_MERKLE_HEIGHT);
    }
    #[test]
    fn transfer_merkle_proof_out_of_range_accessors_are_stable() {
        let transcript = sample_transcript();
        let (old_root, new_root) = transcript_roots(&transcript);
        let witnesses =
            transcripts_to_witnesses(&[transcript], &old_root, &new_root).expect("witnesses");
        let proof = &witnesses[0].deltas[0].smt_proof.from;
        assert_eq!(proof.bit(TRANSFER_MERKLE_HEIGHT), 0);
        assert_eq!(
            proof.sibling(TRANSFER_MERKLE_HEIGHT),
            <[u8; 32]>::from(padding_hash(TRANSFER_MERKLE_HEIGHT))
        );
    }
    #[test]
    fn transfer_merkle_proof_rejects_extra_siblings() {
        let transcript = sample_transcript();
        let mut witness = transcript.deltas[0].from_smt_witness.clone();
        witness.siblings.push([0xAA; 32]);
        let err = TransferMerkleProof::from_witness(&witness).expect_err("extra sibling fails");
        assert!(matches!(err, Error::TransferInvariant { details } if details.contains("sibling")));
    }
    #[test]
    fn attach_transfer_smt_witnesses_rejects_empty_material() {
        let err = attach_transfer_smt_witnesses(&mut []).expect_err("empty witnesses must fail");
        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("at least one delta"))
        );
    }
    #[test]
    fn attach_transfer_smt_witnesses_rejects_empty_transcript_in_mixed_input() {
        let mut empty = sample_transcript();
        empty.deltas.clear();
        empty.poseidon_preimage_digest = None;
        let mut transcripts = vec![empty, sample_transcript()];

        let err = attach_transfer_smt_witnesses(&mut transcripts)
            .expect_err("every present transcript must contain a delta");
        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("at least one delta"))
        );
    }
    #[test]
    fn attach_transfer_smt_witnesses_rejects_stale_single_delta_digest_before_mutation() {
        let mut transcript = sample_transcript();
        transcript.poseidon_preimage_digest = Some(Hash::prehashed([0xEE; 32]));
        transcript.deltas[0].from_smt_witness = TransferSmtWitness::default();
        transcript.deltas[0].to_smt_witness = TransferSmtWitness::default();
        let original = transcript.clone();

        let err = attach_transfer_smt_witnesses(std::slice::from_mut(&mut transcript))
            .expect_err("stale supplied digest must fail before witness attachment");
        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("poseidon digest mismatch"))
        );
        assert_eq!(transcript, original);
    }
    #[test]
    fn attach_transfer_smt_witnesses_rejects_multi_delta_digest_before_mutation() {
        let mut transcript = sample_multi_transcript();
        transcript.poseidon_preimage_digest = Some(Hash::prehashed([0xEE; 32]));
        let original = transcript.clone();

        let err = attach_transfer_smt_witnesses(std::slice::from_mut(&mut transcript))
            .expect_err("multi-delta transcript cannot carry one aggregate digest");
        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("multi-delta transcripts must omit"))
        );
        assert_eq!(transcript, original);
    }
    #[test]
    fn attach_transfer_smt_witnesses_rejects_late_overflow_without_partial_mutation() {
        let mut first = sample_transcript();
        let first_delta = &mut first.deltas[0];
        first_delta.amount = Quantity::from(1_u64);
        first_delta.from_balance_before = Quantity::from(1_u64);
        first_delta.from_balance_after = Quantity::from(0_u64);
        first_delta.to_balance_before = Quantity::from(u64::MAX - 1);
        first_delta.to_balance_after = Quantity::from(u64::MAX);
        first_delta.from_smt_witness = TransferSmtWitness::default();
        first_delta.to_smt_witness = TransferSmtWitness::default();
        first.poseidon_preimage_digest = None;

        let mut second = sample_transcript();
        second.batch_hash = Hash::prehashed([0x22; 32]);
        let second_delta = &mut second.deltas[0];
        second_delta.from_account = (*iroha_test_samples::CARPENTER_ID).clone();
        second_delta.amount = Quantity::from(1_u64);
        second_delta.from_balance_before = Quantity::from(1_u64);
        second_delta.from_balance_after = Quantity::from(0_u64);
        second_delta.to_balance_before = Quantity::from(0_u64);
        second_delta.to_balance_after = Quantity::from(1_u64);
        second_delta.from_smt_witness = TransferSmtWitness::default();
        second_delta.to_smt_witness = TransferSmtWitness::default();
        second.poseidon_preimage_digest = None;

        let mut transcripts = vec![first, second];
        let original = transcripts.clone();
        let err = attach_transfer_smt_witnesses(&mut transcripts)
            .expect_err("the second credit cannot increase a balance already at u64::MAX");
        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("receiver balance overflow"))
        );
        assert_eq!(transcripts, original);
    }
    #[test]
    fn attach_transfer_smt_witnesses_chains_multiple_transcripts() {
        let mut transcripts = vec![sample_transcript(), chained_second_transcript()];
        let (old_root, new_root) =
            attach_transfer_smt_witnesses(&mut transcripts).expect("attach witnesses");
        let witnesses =
            transcripts_to_witnesses(&transcripts, &old_root, &new_root).expect("witnesses");
        assert_eq!(witnesses.len(), 2);
        assert_eq!(
            witnesses[0].deltas[0].smt_proof.to.root_after,
            witnesses[1].deltas[0].smt_proof.from.root_before
        );
        let transitions: Vec<_> = transcripts.iter().flat_map(sample_transitions).collect();
        verify_transcripts(&transitions, &transcripts).expect("transcripts verify");
    }
    #[test]
    fn attach_transfer_smt_witnesses_repairs_higher_precision_stale_balances() {
        let first = sample_transcript();
        let mut second = sample_transcript();
        let delta = &mut second.deltas[0];
        delta.amount = "0.5".parse().expect("canonical quantity");
        delta.from_balance_before = "158.001".parse().expect("canonical quantity");
        delta.from_balance_after = "157.501".parse().expect("canonical quantity");
        delta.to_balance_before = "43.001".parse().expect("canonical quantity");
        delta.to_balance_after = "43.501".parse().expect("canonical quantity");
        delta.from_smt_witness = TransferSmtWitness::default();
        delta.to_smt_witness = TransferSmtWitness::default();
        second.poseidon_preimage_digest = None;
        let mut transcripts = vec![first, second];

        let (old_root, new_root) =
            attach_transfer_smt_witnesses(&mut transcripts).expect("attach witnesses");
        let second = &transcripts[1].deltas[0];
        assert_eq!(second.from_balance_before, Quantity::from(158u32));
        assert_eq!(
            second.from_balance_after,
            "157.5".parse::<Quantity>().expect("canonical quantity")
        );
        assert_eq!(second.to_balance_before, Quantity::from(43u32));
        assert_eq!(
            second.to_balance_after,
            "43.5".parse::<Quantity>().expect("canonical quantity")
        );
        let witnesses = transcripts_to_witnesses(&transcripts, &old_root, &new_root)
            .expect("chained witnesses verify");
        assert_eq!(witnesses[0].deltas[0].from_balance_before, 2_000);
        assert_eq!(witnesses[1].deltas[0].from_balance_before, 1_580);
        assert_eq!(witnesses[1].deltas[0].from_balance_after, 1_575);
    }
    #[test]
    fn transcripts_to_witnesses_accepts_empty_list_when_roots_match() {
        let root = [0x5A; 32];
        let witnesses = transcripts_to_witnesses(&[], &root, &root).expect("empty witnesses");
        assert!(witnesses.is_empty());
    }
    #[test]
    fn transcripts_to_witnesses_rejects_empty_list_when_roots_differ() {
        let old_root = [0x5A; 32];
        let new_root = [0xA5; 32];
        let err =
            transcripts_to_witnesses(&[], &old_root, &new_root).expect_err("root mismatch fails");
        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("final post-root"))
        );
    }
    #[test]
    fn transcripts_to_witnesses_reject_missing_receiver_proof() {
        let mut transcript = sample_transcript();
        let (old_root, new_root) = transcript_roots(&transcript);
        transcript.deltas[0].to_smt_witness = TransferSmtWitness::default();
        let err = transcripts_to_witnesses(&[transcript], &old_root, &new_root)
            .expect_err("missing receiver proof must fail");
        assert!(matches!(err, Error::TransferInvariant { .. }));
    }
    #[test]
    fn transcripts_to_witnesses_reject_missing_sender_proof() {
        let mut transcript = sample_transcript();
        let (old_root, new_root) = transcript_roots(&transcript);
        transcript.deltas[0].from_smt_witness = TransferSmtWitness::default();
        let err = transcripts_to_witnesses(&[transcript], &old_root, &new_root)
            .expect_err("missing sender proof must fail");
        assert!(matches!(err, Error::TransferInvariant { .. }));
    }
    #[test]
    fn transcripts_to_witnesses_reject_unchained_transcript_roots() {
        let transcript = sample_transcript();
        let (old_root, new_root) = transcript_roots(&transcript);
        let err = transcripts_to_witnesses(&[transcript.clone(), transcript], &old_root, &new_root)
            .expect_err("second transcript must chain from first post-root");
        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("sender pre-root"))
        );
    }
    #[test]
    fn transcripts_to_witnesses_reject_wrong_final_root() {
        let transcript = sample_transcript();
        let (old_root, mut new_root) = transcript_roots(&transcript);
        new_root[0] ^= 0x01;
        let err = transcripts_to_witnesses(&[transcript], &old_root, &new_root)
            .expect_err("wrong final root");
        assert!(matches!(err, Error::TransferInvariant { .. }));
    }
    #[test]
    fn transcripts_to_witnesses_reject_wrong_initial_root() {
        let transcript = sample_transcript();
        let (mut old_root, new_root) = transcript_roots(&transcript);
        old_root[0] ^= 0x01;
        let err = transcripts_to_witnesses(&[transcript], &old_root, &new_root)
            .expect_err("wrong initial root");
        assert!(
            matches!(err, Error::TransferInvariant { details } if details.contains("sender pre-root"))
        );
    }
    #[test]
    fn transcripts_to_witnesses_reject_wrong_sibling() {
        let mut transcript = sample_transcript();
        let (old_root, new_root) = transcript_roots(&transcript);
        transcript.deltas[0].from_smt_witness.siblings[0][0] ^= 0x01;
        let err = transcripts_to_witnesses(&[transcript], &old_root, &new_root)
            .expect_err("wrong sibling");
        assert!(matches!(err, Error::TransferInvariant { .. }));
    }
    #[test]
    fn transfer_merkle_proof_rejects_self_consistent_path_for_another_key() {
        let transcript = sample_transcript();
        let delta = &transcript.deltas[0];
        let snapshot = BalanceSnapshot::from_delta(delta).expect("normalized balances");
        let key = balance_key(&delta.asset_definition, &delta.from_account);
        let mut proof = TransferMerkleProof::from_witness(&delta.from_smt_witness)
            .expect("valid sender witness shape");

        proof.path_bits[0] ^= 1;
        proof.root_before = proof.compute_root(&key, snapshot.from_before).into();
        proof.root_after = proof.compute_root(&key, snapshot.from_after).into();
        assert_eq!(
            <[u8; 32]>::from(proof.compute_root(&key, snapshot.from_before)),
            proof.root_before
        );
        assert_eq!(
            <[u8; 32]>::from(proof.compute_root(&key, snapshot.from_after)),
            proof.root_after
        );

        let error = proof
            .verify_update(&key, snapshot.from_before, snapshot.from_after, "sender")
            .expect_err("a self-consistent proof at the wrong path must fail");
        assert!(matches!(
            error,
            Error::TransferInvariant { details }
                if details.contains("path") && details.contains("balance key")
        ));
    }
    #[test]
    fn transcripts_to_witnesses_reject_truncated_merkle_proofs() {
        let mut transcript = sample_transcript();
        let (old_root, new_root) = transcript_roots(&transcript);
        transcript.deltas[0].from_smt_witness.path_bits.truncate(1);
        let err = transcripts_to_witnesses(&[transcript], &old_root, &new_root)
            .expect_err("truncated proof");
        assert!(matches!(err, Error::TransferInvariant { .. }));
    }
    #[test]
    fn transcripts_to_witnesses_reject_multi_delta_digest() {
        let mut transcript = sample_multi_transcript();
        let digest = compute_poseidon_digest(&transcript.deltas[0], &transcript.batch_hash);
        transcript.poseidon_preimage_digest = Some(digest);
        let (old_root, new_root) = transcript_roots(&transcript);
        let err =
            transcripts_to_witnesses(&[transcript], &old_root, &new_root).expect_err("must fail");
        assert!(matches!(err, Error::TransferInvariant { .. }));
    }
    #[test]
    fn verify_transcripts_accepts_multi_delta_batches() {
        let transcript = sample_multi_transcript();
        let transitions = sample_transitions(&transcript);
        assert!(verify_transcripts(&transitions, &[transcript]).is_ok());
    }
    #[test]
    fn verify_transcripts_accepts_mixed_scale_balances() {
        let mut transcript = sample_transcript();
        let half = "0.5".parse::<Quantity>().expect("canonical quantity");
        transcript.deltas[0].amount = half.clone();
        transcript.deltas[0].from_balance_before = Quantity::from(1_u64);
        transcript.deltas[0].from_balance_after = half.clone();
        transcript.deltas[0].to_balance_before = Quantity::from(0_u64);
        transcript.deltas[0].to_balance_after = half;
        transcript.poseidon_preimage_digest = Some(compute_poseidon_digest(
            &transcript.deltas[0],
            &transcript.batch_hash,
        ));
        let transitions = sample_transitions(&transcript);
        assert!(verify_transcripts(&transitions, &[transcript]).is_ok());
    }
    #[test]
    fn transfer_plan_summarises_witnesses() {
        let transcript = sample_multi_transcript();
        let (old_root, new_root) = transcript_roots(&transcript);
        let witnesses =
            transcripts_to_witnesses(std::slice::from_ref(&transcript), &old_root, &new_root)
                .expect("witnesses");
        let plan = TransferGadgetPlan::from_inputs(&witnesses);
        assert_eq!(plan.batch_count(), 1);
        assert_eq!(plan.total_deltas(), transcript.deltas.len());
        assert_eq!(plan.estimated_row_budget(), transcript.deltas.len() * 2);
        assert_eq!(plan.witnesses(), witnesses.as_slice());
    }
    #[test]
    fn row_proof_index_contains_sender_and_receiver_entries() {
        let transcript = sample_transcript();
        let (old_root, new_root) = transcript_roots(&transcript);
        let witnesses =
            transcripts_to_witnesses(std::slice::from_ref(&transcript), &old_root, &new_root)
                .expect("witnesses");
        let index = index_row_proofs(&witnesses);
        assert_eq!(index.len(), 2);
        let delta = &transcript.deltas[0];
        let sender_key = TransferRowKey::new(
            balance_key(&delta.asset_definition, &delta.from_account),
            (200u64).to_le_bytes().to_vec(),
            (158u64).to_le_bytes().to_vec(),
        );
        assert_eq!(index.get(&sender_key).map(VecDeque::len), Some(1));
        let receiver_key = TransferRowKey::new(
            balance_key(&delta.asset_definition, &delta.to_account),
            (1u64).to_le_bytes().to_vec(),
            (43u64).to_le_bytes().to_vec(),
        );
        assert_eq!(index.get(&receiver_key).map(VecDeque::len), Some(1));
    }

    #[test]
    fn row_proof_index_preserves_repeated_row_paths_in_order() {
        let transcript = sample_transcript();
        let (old_root, new_root) = transcript_roots(&transcript);
        let witnesses =
            transcripts_to_witnesses(std::slice::from_ref(&transcript), &old_root, &new_root)
                .expect("witnesses");
        let first = witnesses[0].deltas[0].clone();
        let mut later = first.clone();
        later.smt_proof.from.root_before[0] ^= 0x5A;
        later.smt_proof.from.siblings[0][0] ^= 0xA5;
        let inputs = [TransferGadgetInput {
            batch_hash: witnesses[0].batch_hash,
            authority_digest: witnesses[0].authority_digest,
            deltas: vec![first.clone(), later.clone()],
        }];

        let index = index_row_proofs(&inputs);
        let sender_key = TransferRowKey::new(
            balance_key(&first.asset_definition, &first.from_account),
            first.from_balance_before.to_le_bytes().to_vec(),
            first.from_balance_after.to_le_bytes().to_vec(),
        );
        let queue = index.get(&sender_key).expect("repeated sender row");
        assert_eq!(queue.len(), 2);
        assert_eq!(queue[0], first.smt_proof.from);
        assert_eq!(queue[1], later.smt_proof.from);
    }
    #[test]
    fn transfer_row_key_from_transition_matches_explicit_key() {
        let transition = StateTransition::new(
            b"asset/row/key".to_vec(),
            3_u64.to_le_bytes().to_vec(),
            4_u64.to_le_bytes().to_vec(),
            OperationKind::Transfer,
        );
        assert_eq!(
            TransferRowKey::from_transition(&transition),
            TransferRowKey::new(
                transition.key.clone(),
                transition.pre_value.clone(),
                transition.post_value.clone(),
            )
        );
    }
    fn sample_multi_transcript() -> TransferTranscript {
        let mut transcript = sample_transcript();
        let second_delta = TransferDeltaTranscript {
            from_account: (*BOB_ID).clone(),
            to_account: (*ALICE_ID).clone(),
            asset_definition: AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "lily".parse().unwrap(),
            ),
            amount: Quantity::from(7u32),
            from_balance_before: Quantity::from(90u32),
            from_balance_after: Quantity::from(83u32),
            to_balance_before: Quantity::from(5u32),
            to_balance_after: Quantity::from(12u32),
            from_smt_witness: TransferSmtWitness::default(),
            to_smt_witness: TransferSmtWitness::default(),
        };
        transcript.deltas.push(second_delta);
        transcript.poseidon_preimage_digest = None;
        attach_transcript_witnesses(&mut transcript);
        transcript
    }
    fn chained_second_transcript() -> TransferTranscript {
        let mut transcript = sample_transcript();
        transcript.batch_hash = Hash::prehashed([0x22; 32]);
        {
            let delta = &mut transcript.deltas[0];
            delta.amount = Quantity::from(8u32);
            delta.from_balance_before = Quantity::from(158u32);
            delta.from_balance_after = Quantity::from(150u32);
            delta.to_balance_before = Quantity::from(43u32);
            delta.to_balance_after = Quantity::from(51u32);
            delta.from_smt_witness = TransferSmtWitness::default();
            delta.to_smt_witness = TransferSmtWitness::default();
        }
        transcript.poseidon_preimage_digest = Some(compute_poseidon_digest(
            &transcript.deltas[0],
            &transcript.batch_hash,
        ));
        transcript
    }
    fn attach_delta_witnesses(delta: &mut TransferDeltaTranscript) {
        let (from_witness, to_witness) = build_transfer_smt_witness_pair(
            &balance_key(&delta.asset_definition, &delta.from_account),
            numeric_to_u64(&delta.from_balance_before, delta.normalized_scale()),
            numeric_to_u64(&delta.from_balance_after, delta.normalized_scale()),
            &balance_key(&delta.asset_definition, &delta.to_account),
            numeric_to_u64(&delta.to_balance_before, delta.normalized_scale()),
            numeric_to_u64(&delta.to_balance_after, delta.normalized_scale()),
        )
        .expect("valid transfer witness");
        delta.from_smt_witness = from_witness;
        delta.to_smt_witness = to_witness;
    }
    fn attach_transcript_witnesses(transcript: &mut TransferTranscript) {
        let mut state = TransferSmtState::default();
        let mut seeded_keys = BTreeSet::new();
        for delta in &transcript.deltas {
            let scale = delta.normalized_scale();
            let from_key = balance_key(&delta.asset_definition, &delta.from_account);
            if seeded_keys.insert(from_key.clone()) {
                state
                    .insert(&from_key, numeric_to_u64(&delta.from_balance_before, scale))
                    .expect("sender leaf");
            }
            let to_key = balance_key(&delta.asset_definition, &delta.to_account);
            if seeded_keys.insert(to_key.clone()) {
                state
                    .insert(&to_key, numeric_to_u64(&delta.to_balance_before, scale))
                    .expect("receiver leaf");
            }
        }
        for delta in &mut transcript.deltas {
            let scale = delta.normalized_scale();
            let from_key = balance_key(&delta.asset_definition, &delta.from_account);
            delta.from_smt_witness = state
                .update_witness(
                    &from_key,
                    numeric_to_u64(&delta.from_balance_before, scale),
                    numeric_to_u64(&delta.from_balance_after, scale),
                )
                .expect("sender update");
            let to_key = balance_key(&delta.asset_definition, &delta.to_account);
            delta.to_smt_witness = state
                .update_witness(
                    &to_key,
                    numeric_to_u64(&delta.to_balance_before, scale),
                    numeric_to_u64(&delta.to_balance_after, scale),
                )
                .expect("receiver update");
        }
    }
    fn transcript_roots(transcript: &TransferTranscript) -> ([u8; 32], [u8; 32]) {
        let old_root = transcript
            .deltas
            .first()
            .expect("sample has delta")
            .from_smt_witness
            .root_before;
        let new_root = transcript
            .deltas
            .last()
            .expect("sample has delta")
            .to_smt_witness
            .root_after;
        (old_root, new_root)
    }
    fn numeric_to_u64(value: &Quantity, target_scale: u32) -> u64 {
        normalized_numeric_to_u64(value.as_numeric(), target_scale).expect("quantity fits u64")
    }
    fn numeric_to_le_bytes(value: &Quantity, target_scale: u32) -> Vec<u8> {
        let amount =
            normalized_numeric_to_u64(value.as_numeric(), target_scale).expect("quantity fits u64");
        amount.to_le_bytes().to_vec()
    }
}
