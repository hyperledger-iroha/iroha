//! Transfer gadget scaffolding shared between the planner and trace builder.

use std::collections::{BTreeMap, HashMap, VecDeque};

use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    asset::id::AssetDefinitionId,
    fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
};
use iroha_primitives::numeric::Numeric;
use iroha_zkp_halo2::poseidon;
use norito::{codec::Encode as NoritoEncode, decode_from_bytes};

use crate::{Error, OperationKind, StateTransition};

/// Height of the synthetic SMT used by the transfer gadget.
pub const TRANSFER_MERKLE_HEIGHT: usize = 32;

/// Proof flavor used to derive deterministic SMT paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofFlavor {
    /// Sender-side balance entry.
    Sender,
    /// Receiver-side balance entry.
    Receiver,
}

impl ProofFlavor {
    fn salt(self) -> &'static [u8] {
        match self {
            ProofFlavor::Sender => b"fastpq:smt:from",
            ProofFlavor::Receiver => b"fastpq:smt:to",
        }
    }
}

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
    /// Optional SMT proofs captured from the host.
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

    /// Returns the estimated gadget row budget (placeholder until TF-3 lands the real constraints).
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
#[must_use]
pub fn index_row_proofs(
    inputs: &[TransferGadgetInput],
) -> HashMap<TransferRowKey, TransferMerkleProof> {
    let mut map = HashMap::new();
    for witness in inputs {
        for delta in &witness.deltas {
            let sender_key = balance_key(&delta.asset_definition, &delta.from_account);
            let receiver_key = balance_key(&delta.asset_definition, &delta.to_account);
            if let Some(proof) = &delta.smt_proof.from {
                map.insert(
                    TransferRowKey::new(
                        sender_key.clone(),
                        delta.from_balance_before.to_le_bytes().to_vec(),
                        delta.from_balance_after.to_le_bytes().to_vec(),
                    ),
                    proof.clone(),
                );
            }
            if let Some(proof) = &delta.smt_proof.to {
                map.insert(
                    TransferRowKey::new(
                        receiver_key.clone(),
                        delta.to_balance_before.to_le_bytes().to_vec(),
                        delta.to_balance_after.to_le_bytes().to_vec(),
                    ),
                    proof.clone(),
                );
            }
        }
    }
    map
}

/// Wrapper containing optional SMT proofs for both participants of a transfer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransferSmtProof {
    /// Sender Merkle proof emitted by the host.
    pub from: Option<TransferMerkleProof>,
    /// Receiver Merkle proof emitted by the host.
    pub to: Option<TransferMerkleProof>,
}

impl TransferSmtProof {
    fn from_transcript(from_bytes: Option<&[u8]>, to_bytes: Option<&[u8]>) -> Result<Self, Error> {
        Ok(Self {
            from: from_bytes.map(TransferMerkleProof::decode).transpose()?,
            to: to_bytes.map(TransferMerkleProof::decode).transpose()?,
        })
    }

    /// Returns true if both sender and receiver proofs are present.
    #[must_use]
    pub fn has_paired_paths(&self) -> bool {
        self.from.is_some() && self.to.is_some()
    }

    fn ensure_sender(&mut self, key: &[u8], balance_before: u64) {
        if self.from.is_none() {
            self.from = Some(synthesize_row_proof(
                key,
                balance_before,
                ProofFlavor::Sender,
            ));
        }
    }

    fn ensure_receiver(&mut self, key: &[u8], balance_before: u64) {
        if self.to.is_none() {
            self.to = Some(synthesize_row_proof(
                key,
                balance_before,
                ProofFlavor::Receiver,
            ));
        }
    }
}

/// Merkle proof payload describing the path for a single leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferMerkleProof {
    /// Bitset describing the direction taken at each level (LSB-first per byte).
    pub path_bits: Vec<u8>,
    /// Sibling node hashes encountered along the path.
    pub siblings: Vec<Hash>,
}

impl TransferMerkleProof {
    fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let blob: TransferMerkleProofBlob =
            decode_from_bytes(bytes).map_err(|source| Error::TransferProofDecode { source })?;
        if blob.version != TRANSFER_MERKLE_PROOF_VERSION {
            return Err(Error::TransferProofUnsupportedVersion {
                version: blob.version,
            });
        }
        Ok(Self {
            path_bits: normalize_path_bits(blob.path_bits),
            siblings: normalize_siblings(blob.siblings),
        })
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

    /// Returns the sibling hash for the given depth, padding deterministically when missing.
    #[must_use]
    pub fn sibling(&self, level: usize) -> Hash {
        if level >= TRANSFER_MERKLE_HEIGHT {
            return padding_hash(level);
        }
        self.siblings
            .get(level)
            .copied()
            .unwrap_or_else(|| padding_hash(level))
    }
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
struct TransferMerkleProofBlob {
    version: u16,
    path_bits: Vec<u8>,
    siblings: Vec<Hash>,
}

const TRANSFER_MERKLE_PROOF_VERSION: u16 = 1;
const KEY_DOMAIN: &[u8] = b"fastpq:smt:key|";
const VALUE_DOMAIN: &[u8] = b"fastpq:smt:value|";
const LEAF_DOMAIN: &[u8] = b"fastpq:smt:leaf|";
const NODE_DOMAIN: &[u8] = b"fastpq:smt:node|";
const SIBLING_DOMAIN: &[u8] = b"fastpq:smt:sibling|";
const PAD_DOMAIN: &[u8] = b"fastpq:smt:pad|";

/// Deterministically synthesise a Merkle proof for a transfer row.
#[must_use]
pub fn synthesize_row_proof(key: &[u8], balance: u64, flavor: ProofFlavor) -> TransferMerkleProof {
    let key_hash = synthetic_key_hash(key, flavor);
    let mut path_bits = vec![0u8; TRANSFER_MERKLE_HEIGHT.div_ceil(8)];
    let mut siblings = Vec::with_capacity(TRANSFER_MERKLE_HEIGHT);
    let key_bytes = key_hash.as_ref();
    for level in 0..TRANSFER_MERKLE_HEIGHT {
        let byte_index = level / 8;
        let bit_index = level % 8;
        let bit = (key_bytes[byte_index % key_bytes.len()] >> bit_index) & 1;
        if bit == 1 {
            path_bits[byte_index] |= 1 << bit_index;
        }
        siblings.push(synthetic_sibling_hash(
            level,
            key_hash.as_ref(),
            balance,
            flavor,
        ));
    }
    TransferMerkleProof {
        path_bits,
        siblings,
    }
}

pub(crate) fn synthetic_leaf_hash(key: &[u8], balance: u64, flavor: ProofFlavor) -> Hash {
    let mut payload = Vec::with_capacity(LEAF_DOMAIN.len() + 64);
    payload.extend_from_slice(LEAF_DOMAIN);
    payload.extend_from_slice(synthetic_key_hash(key, flavor).as_ref());
    payload.extend_from_slice(synthetic_value_hash(balance, flavor).as_ref());
    Hash::new(payload)
}

pub(crate) fn synthetic_internal_hash(left: &Hash, right: &Hash) -> Hash {
    let mut payload = Vec::with_capacity(NODE_DOMAIN.len() + 64);
    payload.extend_from_slice(NODE_DOMAIN);
    payload.extend_from_slice(left.as_ref());
    payload.extend_from_slice(right.as_ref());
    Hash::new(payload)
}

fn synthetic_key_hash(key: &[u8], flavor: ProofFlavor) -> Hash {
    let mut payload = Vec::with_capacity(KEY_DOMAIN.len() + flavor.salt().len() + key.len());
    payload.extend_from_slice(KEY_DOMAIN);
    payload.extend_from_slice(flavor.salt());
    payload.extend_from_slice(key);
    Hash::new(payload)
}

fn synthetic_value_hash(balance: u64, flavor: ProofFlavor) -> Hash {
    let mut payload = Vec::with_capacity(VALUE_DOMAIN.len() + flavor.salt().len() + 8);
    payload.extend_from_slice(VALUE_DOMAIN);
    payload.extend_from_slice(flavor.salt());
    payload.extend_from_slice(&balance.to_le_bytes());
    Hash::new(payload)
}

fn synthetic_sibling_hash(
    level: usize,
    key_hash_bytes: &[u8],
    balance: u64,
    flavor: ProofFlavor,
) -> Hash {
    let mut payload = Vec::with_capacity(SIBLING_DOMAIN.len() + 64 + 8 + flavor.salt().len());
    payload.extend_from_slice(SIBLING_DOMAIN);
    payload.extend_from_slice(&(level as u64).to_le_bytes());
    payload.extend_from_slice(key_hash_bytes);
    payload.extend_from_slice(&balance.to_le_bytes());
    payload.extend_from_slice(flavor.salt());
    Hash::new(payload)
}

fn normalize_path_bits(mut bits: Vec<u8>) -> Vec<u8> {
    let required = TRANSFER_MERKLE_HEIGHT.div_ceil(8);
    if bits.len() < required {
        bits.resize(required, 0);
    } else if bits.len() > required {
        bits.truncate(required);
    }
    bits
}

fn normalize_siblings(mut siblings: Vec<Hash>) -> Vec<Hash> {
    if siblings.len() < TRANSFER_MERKLE_HEIGHT {
        for level in siblings.len()..TRANSFER_MERKLE_HEIGHT {
            siblings.push(padding_hash(level));
        }
    } else if siblings.len() > TRANSFER_MERKLE_HEIGHT {
        siblings.truncate(TRANSFER_MERKLE_HEIGHT);
    }
    siblings
}

fn padding_hash(level: usize) -> Hash {
    let mut payload = Vec::with_capacity(PAD_DOMAIN.len() + 8);
    payload.extend_from_slice(PAD_DOMAIN);
    payload.extend_from_slice(&(level as u64).to_le_bytes());
    Hash::new(payload)
}

/// Decode transfer transcripts embedded in the batch metadata.
///
/// # Errors
/// Returns an error when the metadata payload cannot be decoded.
pub fn decode_transcripts(
    metadata: &BTreeMap<String, Vec<u8>>,
) -> Result<Option<Vec<TransferTranscript>>, Error> {
    let Some(encoded) = metadata.get(TRANSFER_TRANSCRIPTS_METADATA_KEY) else {
        return Ok(None);
    };
    let transcripts =
        decode_from_bytes(encoded).map_err(|source| Error::TransferMetadataDecode { source })?;
    Ok(Some(transcripts))
}

/// Convert validated transcripts into structured gadget inputs.
///
/// # Errors
/// Returns [`Error::TransferInvariant`] when required digests are missing or
/// the transcript fails arithmetic checks.
pub fn transcripts_to_witnesses(
    transcripts: &[TransferTranscript],
) -> Result<Vec<TransferGadgetInput>, Error> {
    transcripts
        .iter()
        .map(|transcript| {
            if transcript.deltas.is_empty() {
                return Err(Error::TransferInvariant {
                    details: "transfer transcript must contain at least one delta".into(),
                });
            }
            let authority_digest = transcript.authority_digest;
            let mut deltas = Vec::with_capacity(transcript.deltas.len());
            for delta in &transcript.deltas {
                let snapshot = BalanceSnapshot::from_delta(delta)?;
                let poseidon_digest = compute_poseidon_digest(delta, &transcript.batch_hash);
                enforce_poseidon_policy(transcript, &poseidon_digest)?;
                let sender_key = balance_key(&delta.asset_definition, &delta.from_account);
                let receiver_key = balance_key(&delta.asset_definition, &delta.to_account);
                let mut smt_proof = TransferSmtProof::from_transcript(
                    delta.from_merkle_proof.as_deref(),
                    delta.to_merkle_proof.as_deref(),
                )?;
                smt_proof.ensure_sender(&sender_key, snapshot.from_before);
                smt_proof.ensure_receiver(&receiver_key, snapshot.to_before);

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
            Ok(TransferGadgetInput {
                batch_hash: transcript.batch_hash,
                authority_digest,
                deltas,
            })
        })
        .collect()
}

/// Verify arithmetic and digest invariants for transfer transcripts.
///
/// # Errors
/// Returns an error if any transcript arithmetic or digest invariant fails.
pub fn verify_transcripts(
    transitions: &[StateTransition],
    transcripts: &[TransferTranscript],
) -> Result<(), Error> {
    if transcripts.is_empty() {
        return Ok(());
    }
    let mut transfer_rows = index_transfers(transitions);
    for transcript in transcripts {
        for delta in &transcript.deltas {
            let snapshot = BalanceSnapshot::from_delta(delta)?;
            let poseidon_digest = compute_poseidon_digest(delta, &transcript.batch_hash);
            enforce_poseidon_policy(transcript, &poseidon_digest)?;
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

fn enforce_poseidon_policy(transcript: &TransferTranscript, digest: &Hash) -> Result<(), Error> {
    if let Some(expected) = &transcript.poseidon_preimage_digest {
        if transcript.deltas.len() == 1 {
            if digest != expected {
                return Err(Error::TransferInvariant {
                    details: format!(
                        "poseidon digest mismatch for transfer {} -> {} ({})",
                        transcript.deltas[0].from_account,
                        transcript.deltas[0].to_account,
                        transcript.deltas[0].asset_definition
                    ),
                });
            }
        } else {
            return Err(Error::TransferInvariant {
                details: "multi-delta transcripts must omit poseidon_preimage_digest until per-delta digests land".into(),
            });
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
    let mut preimage = Vec::with_capacity(256);
    append_encoded(&mut preimage, &delta.from_account);
    append_encoded(&mut preimage, &delta.to_account);
    append_encoded(&mut preimage, &delta.asset_definition);
    append_encoded(&mut preimage, &delta.amount);
    preimage.extend_from_slice(batch_hash.as_ref());
    Hash::prehashed(poseidon::hash_bytes(&preimage))
}

fn append_encoded(buffer: &mut Vec<u8>, value: &impl NoritoEncode) {
    buffer.extend_from_slice(&value.encode());
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
        let amount = numeric_to_u64("amount", &delta.amount)?;
        let from_before = numeric_to_u64("from_balance_before", &delta.from_balance_before)?;
        let from_after = numeric_to_u64("from_balance_after", &delta.from_balance_after)?;
        let to_before = numeric_to_u64("to_balance_before", &delta.to_balance_before)?;
        let to_after = numeric_to_u64("to_balance_after", &delta.to_balance_after)?;

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

fn numeric_to_u64(field: &'static str, value: &Numeric) -> Result<u64, Error> {
    value
        .clone()
        .try_into()
        .map_err(|_| Error::TransferNumericBounds { field })
}

#[cfg(test)]
mod tests {
    use iroha_crypto::Hash;
    use iroha_data_model::{
        asset::id::AssetDefinitionId,
        fastpq::{TransferDeltaTranscript, TransferTranscript},
    };
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use norito::to_bytes;

    use super::*;
    use crate::{OperationKind, StateTransition};

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
    fn verify_transcripts_checks_balances() {
        let transcript = sample_transcript();
        let transitions = sample_transitions(&transcript);
        let result = verify_transcripts(&transitions, &[transcript]);
        assert!(result.is_ok());
    }

    #[test]
    fn verify_transcripts_detects_sender_mismatch() {
        let mut transcript = sample_transcript();
        transcript.deltas[0].from_balance_after = Numeric::from(1u32);
        let transitions = sample_transitions(&transcript);
        let err = verify_transcripts(&transitions, &[transcript]).expect_err("must fail");
        assert!(matches!(err, Error::TransferInvariant { .. }));
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

    fn sample_transcript() -> TransferTranscript {
        use iroha_test_samples::{ALICE_ID, BOB_ID};
        let alice = (*ALICE_ID).clone();
        let bob = (*BOB_ID).clone();
        let asset = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let delta = TransferDeltaTranscript {
            from_account: alice.clone(),
            to_account: bob.clone(),
            asset_definition: asset.clone(),
            amount: Numeric::from(42u32),
            from_balance_before: Numeric::from(200u32),
            from_balance_after: Numeric::from(158u32),
            to_balance_before: Numeric::from(1u32),
            to_balance_after: Numeric::from(43u32),
            from_merkle_proof: None,
            to_merkle_proof: None,
        };
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
                let sender = StateTransition::new(
                    balance_key(&delta.asset_definition, &delta.from_account),
                    numeric_to_le_bytes(&delta.from_balance_before),
                    numeric_to_le_bytes(&delta.from_balance_after),
                    OperationKind::Transfer,
                );
                let receiver = StateTransition::new(
                    balance_key(&delta.asset_definition, &delta.to_account),
                    numeric_to_le_bytes(&delta.to_balance_before),
                    numeric_to_le_bytes(&delta.to_balance_after),
                    OperationKind::Transfer,
                );
                [sender, receiver]
            })
            .collect()
    }

    #[test]
    fn transcripts_to_witnesses_emit_structured_witness() {
        let transcript = sample_transcript();
        let inputs = transcripts_to_witnesses(&[transcript]).expect("witnesses");
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
    fn transcripts_to_witnesses_accept_multi_delta_batches() {
        let transcript = sample_multi_transcript();
        let witnesses =
            transcripts_to_witnesses(std::slice::from_ref(&transcript)).expect("witnesses");
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
    fn transcripts_to_witnesses_decode_merkle_proofs() {
        let mut transcript = sample_transcript();
        let proof_bytes = sample_merkle_proof_bytes(TRANSFER_MERKLE_PROOF_VERSION);
        transcript.deltas[0].from_merkle_proof = Some(proof_bytes.clone());
        transcript.deltas[0].to_merkle_proof = Some(proof_bytes);
        let witnesses = transcripts_to_witnesses(&[transcript]).expect("witnesses");
        let smt = &witnesses[0].deltas[0].smt_proof;
        assert!(smt.has_paired_paths());
        assert_eq!(
            smt.from.as_ref().expect("from proof").path_bits.as_slice(),
            &[0x5A, 0x00, 0x00, 0x00]
        );
        assert_eq!(
            smt.to.as_ref().expect("to proof").siblings.len(),
            TRANSFER_MERKLE_HEIGHT
        );
    }

    #[test]
    fn transcripts_to_witnesses_reject_unknown_merkle_proof_version() {
        let mut transcript = sample_transcript();
        transcript.deltas[0].from_merkle_proof =
            Some(sample_merkle_proof_bytes(TRANSFER_MERKLE_PROOF_VERSION + 1));
        let err = transcripts_to_witnesses(&[transcript]).expect_err("invalid version");
        assert!(matches!(
            err,
            Error::TransferProofUnsupportedVersion { version }
                if version == TRANSFER_MERKLE_PROOF_VERSION + 1
        ));
    }

    #[test]
    fn transcripts_to_witnesses_reject_multi_delta_digest() {
        let mut transcript = sample_multi_transcript();
        let digest = compute_poseidon_digest(&transcript.deltas[0], &transcript.batch_hash);
        transcript.poseidon_preimage_digest = Some(digest);
        let err = transcripts_to_witnesses(&[transcript]).expect_err("must fail");
        assert!(matches!(err, Error::TransferInvariant { .. }));
    }

    #[test]
    fn verify_transcripts_accepts_multi_delta_batches() {
        let transcript = sample_multi_transcript();
        let transitions = sample_transitions(&transcript);
        assert!(verify_transcripts(&transitions, &[transcript]).is_ok());
    }

    #[test]
    fn transfer_plan_summarises_witnesses() {
        let transcript = sample_multi_transcript();
        let witnesses =
            transcripts_to_witnesses(std::slice::from_ref(&transcript)).expect("witnesses");
        let plan = TransferGadgetPlan::from_inputs(&witnesses);
        assert_eq!(plan.batch_count(), 1);
        assert_eq!(plan.total_deltas(), transcript.deltas.len());
        assert_eq!(plan.estimated_row_budget(), transcript.deltas.len() * 2);
        assert_eq!(plan.witnesses(), witnesses.as_slice());
    }

    #[test]
    fn row_proof_index_contains_sender_and_receiver_entries() {
        let transcript = sample_transcript();
        let witnesses =
            transcripts_to_witnesses(std::slice::from_ref(&transcript)).expect("witnesses");
        let index = index_row_proofs(&witnesses);
        assert_eq!(index.len(), 2);
        let delta = &transcript.deltas[0];
        let sender_key = TransferRowKey::new(
            balance_key(&delta.asset_definition, &delta.from_account),
            (200u64).to_le_bytes().to_vec(),
            (158u64).to_le_bytes().to_vec(),
        );
        assert!(index.contains_key(&sender_key));
        let receiver_key = TransferRowKey::new(
            balance_key(&delta.asset_definition, &delta.to_account),
            (1u64).to_le_bytes().to_vec(),
            (43u64).to_le_bytes().to_vec(),
        );
        assert!(index.contains_key(&receiver_key));
    }

    fn sample_multi_transcript() -> TransferTranscript {
        let mut transcript = sample_transcript();
        let second_delta = TransferDeltaTranscript {
            from_account: (*BOB_ID).clone(),
            to_account: (*ALICE_ID).clone(),
            asset_definition: AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "lily".parse().unwrap(),
            ),
            amount: Numeric::from(7u32),
            from_balance_before: Numeric::from(90u32),
            from_balance_after: Numeric::from(83u32),
            to_balance_before: Numeric::from(5u32),
            to_balance_after: Numeric::from(12u32),
            from_merkle_proof: None,
            to_merkle_proof: None,
        };
        transcript.deltas.push(second_delta);
        transcript.poseidon_preimage_digest = None;
        transcript
    }

    fn sample_merkle_proof_bytes(version: u16) -> Vec<u8> {
        let blob = TransferMerkleProofBlob {
            version,
            path_bits: vec![0x5A],
            siblings: vec![Hash::prehashed([0x23; 32])],
        };
        to_bytes(&blob).expect("encode proof blob")
    }

    fn numeric_to_le_bytes(value: &Numeric) -> Vec<u8> {
        let amount: u64 = value.clone().try_into().expect("numeric fits u64");
        amount.to_le_bytes().to_vec()
    }
}
