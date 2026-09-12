//! Occurrence-preserving bindings from validated transfer deltas to execution rows.
//!
//! The input must come from `transfer::transcripts_to_witnesses` after native
//! transcript, key-allocation, hash and public-root validation. Its public Rust
//! fields do not constitute an authenticated type. This adapter checks cheap
//! consistency conditions and exact row multiplicity; it does not rehash paths,
//! authenticate authority, or replace the verifier's mandatory witness replay.
//!
//! Rows must already be in the canonical statement order (full key, then operation
//! rank, stable for equal keys/ranks). Updates instead occur in transcript/delta
//! order, sender before receiver. Identical key/pre/post rows consume occurrences
//! in FIFO order, including zero self-transfers whose two legs are identical.
//! Their ordinals and declared roles remain distinct even if every value repeats.
//!
//! Public semantic statements may expose these exact identities, balances and
//! amounts for bounded public processing. This adapter assumes no new privacy
//! boundary. The trace builder uses these bindings for integer auxiliaries and
//! existing SMT projections. TODO: Prove their complete authenticated statement,
//! hash and public-root bindings; constructors alone do not constrain openings.

use std::collections::{HashMap, VecDeque};

use norito::codec::Encode as NoritoEncode;

use super::{
    transfer::{
        TRANSFER_MERKLE_HEIGHT, TransferDeltaWitness, TransferGadgetInput, TransferMerkleProof,
        TransferRowKey,
    },
    transfer_integer_air::{TransferIntegerWitness, Unsigned64Witness},
    transfer_pair_air::{PackedIdentity, PairIdentity, PairTuple},
};
use crate::{Error, OperationKind, StateTransition};

/// Semantic role of one participant update, independent of its balance difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferRowRole {
    /// Sender update, including a zero-amount debit.
    Debit,
    /// Receiver update, including a zero-amount credit.
    Credit,
}

impl TransferRowRole {
    /// Boolean field value required by the integer and pair AIR relations.
    #[must_use]
    pub const fn is_debit(self) -> u64 {
        match self {
            Self::Debit => 1,
            Self::Credit => 0,
        }
    }
}

/// Exact occurrence in the original transcript/delta sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferRowOccurrence {
    /// Zero-based transcript position; equal batch hashes do not merge positions.
    pub transcript_ordinal: u32,
    /// Zero-based delta position within this transcript.
    pub delta_ordinal: u32,
    /// Zero-based delta position across all transcripts, shared by its two roles.
    pub pair_ordinal: u32,
}

/// Inputs absent from `TransferGadgetInput`, supplied by the authenticated statement.
///
/// The scale must be the original `transfer_asset_scales` result for this asset.
/// Call and authority bytes must use the statement's fixed canonical encodings
/// and retain its real transaction/call occurrence and full authority context.
/// The caller must validate them against the original statement. This structure
/// and a successful tuple conversion do not authenticate their contents.
#[derive(Debug, Clone, Copy)]
pub struct TransferPairContext<'a> {
    /// Complete canonical transaction/call identity from the outer statement.
    pub call_identity: &'a [u8],
    /// Complete canonical authority context from the outer statement.
    pub authority_identity: &'a [u8],
    /// Common asset scale used to normalize the validated integer amounts.
    pub asset_scale: u32,
}

/// Immutable association of an actual statement row with one validated occurrence.
#[derive(Debug, Clone, Copy)]
pub struct TransferRowBinding<'a> {
    occurrence: TransferRowOccurrence,
    role: TransferRowRole,
    input: &'a TransferGadgetInput,
    delta: &'a TransferDeltaWitness,
    transition: &'a StateTransition,
}

impl<'a> TransferRowBinding<'a> {
    /// Position of the original delta; cloning a binding preserves its identity.
    #[must_use]
    pub const fn occurrence(&self) -> TransferRowOccurrence {
        self.occurrence
    }

    /// Declared sender/receiver role, including when the amount is zero.
    #[must_use]
    pub const fn role(&self) -> TransferRowRole {
        self.role
    }

    /// Actual matched canonical statement row, including all key and value bytes.
    #[must_use]
    pub const fn transition(&self) -> &'a StateTransition {
        self.transition
    }

    /// Original validated transcript input, retaining batch and authority digests.
    #[must_use]
    pub const fn input(&self) -> &'a TransferGadgetInput {
        self.input
    }

    /// Original validated delta with its declared normalized amount and identities.
    #[must_use]
    pub const fn delta(&self) -> &'a TransferDeltaWitness {
        self.delta
    }

    /// Original full-width SMT proof for this exact participant occurrence.
    #[must_use]
    pub const fn proof(&self) -> &'a TransferMerkleProof {
        match self.role {
            TransferRowRole::Debit => &self.delta.smt_proof.from,
            TransferRowRole::Credit => &self.delta.smt_proof.to,
        }
    }

    /// Exact arithmetic witness using the declared amount and role, without reduction.
    ///
    /// Arithmetic was checked when creating this binding. In particular, a zero
    /// sender remains a debit rather than inheriting a role from `before > after`.
    #[must_use]
    pub fn integer_witness(&self) -> TransferIntegerWitness {
        let (before, after) = participant_balances(self.delta, self.role);
        let addend = match self.role {
            TransferRowRole::Debit => after,
            TransferRowRole::Credit => before,
        };
        let low_mask = u64::from(u32::MAX);
        TransferIntegerWitness {
            before: Unsigned64Witness::from_integer(before),
            after: Unsigned64Witness::from_integer(after),
            amount: Unsigned64Witness::from_integer(self.delta.amount),
            carry_32: ((addend & low_mask) + (self.delta.amount & low_mask)) >> 32,
            is_debit: self.role.is_debit(),
        }
    }

    /// Prepare exact pair fields from this row and explicit public statement context.
    ///
    /// Key bytes are the complete canonical `FastpqBalanceKeyV1` Norito frame.
    /// Asset and both accounts use their exact Norito `Encode::encode` bytes.
    /// Call bytes are `batch_hash[32] || LE32(transcript) || LE32(delta) ||
    /// LE32(context_len) || context`; authority bytes are `authority_digest[32]
    /// || LE32(context_len) || context`. The tuple's typed fields distinguish
    /// these encodings. Every identity also carries its exact byte length and
    /// zero-padded seven-byte limbs; integers use 56+8-bit limbs. Neither digests
    /// nor wide integers are projected into one Goldilocks element.
    ///
    /// This candidate table encoding does not change production wire admission.
    /// Its external context and scale must be authenticated by the caller.
    ///
    /// # Errors
    /// Returns [`Error::TransferInvariant`] if an identity exceeds its declared
    /// packing width, or a context length cannot be represented exactly as `u32`.
    pub fn pair_tuple<const KEY: usize, const IDENTITY: usize>(
        &self,
        context: TransferPairContext<'_>,
    ) -> Result<PairTuple<u64, KEY, IDENTITY>, Error> {
        let mut call_prefix = [0_u8; 40];
        call_prefix[..32].copy_from_slice(self.input.batch_hash.as_ref());
        call_prefix[32..36].copy_from_slice(&self.occurrence.transcript_ordinal.to_le_bytes());
        call_prefix[36..].copy_from_slice(&self.occurrence.delta_ordinal.to_le_bytes());
        let identity = PairIdentity {
            call: pack_context(&call_prefix, context.call_identity)?,
            authority: pack_context(
                self.input.authority_digest.as_ref(),
                context.authority_identity,
            )?,
            asset: pack_identity(&self.delta.asset_definition.encode())?,
            sender: pack_identity(&self.delta.from_account.encode())?,
            receiver: pack_identity(&self.delta.to_account.encode())?,
            asset_scale: u64::from(context.asset_scale),
        };
        let integer = self.integer_witness();
        Ok(PairTuple {
            pair_ordinal: u64::from(self.occurrence.pair_ordinal),
            is_debit: self.role.is_debit(),
            key: pack_identity(&self.transition.key)?,
            before: integer.before.packed,
            after: integer.after.packed,
            amount: integer.amount.packed,
            identity,
        })
    }
}

/// Bind every transfer occurrence once to an already-canonical statement row.
///
/// The result has exactly `transitions.len()` entries; metadata rows contain
/// `None`. A complete row match includes the full key and exactly eight old/new
/// bytes. Recurring identical rows take the earliest remaining occurrence, in
/// transcript/delta/debit-before-credit order. No hash-map iteration determines
/// output order. Inputs and proofs are borrowed, so paths are not cloned.
///
/// Cheap checks reject inconsistent public witness structs (amount arithmetic,
/// self-transfer continuity, root chaining, path shape and canonical markers).
/// They do not establish hash validity, collision-resolved path allocation or
/// the public root endpoints; those remain the validated-input prerequisite.
///
/// # Errors
/// Returns [`Error::TransferInvariant`] for noncanonical order, cardinality or
/// ordinal overflow, an empty transcript, malformed/inconsistent witness data,
/// missing/extra rows, or any unconsumed occurrence.
pub fn bind_canonical_rows<'a>(
    transitions: &'a [StateTransition],
    inputs: &'a [TransferGadgetInput],
) -> Result<Vec<Option<TransferRowBinding<'a>>>, Error> {
    if transitions.windows(2).any(|rows| {
        (&rows[0].key, rows[0].operation_rank()) > (&rows[1].key, rows[1].operation_rank())
    }) {
        return Err(invariant(
            "transfer row binding requires canonical statement order",
        ));
    }
    let pair_count = inputs.iter().try_fold(0_u32, |count, input| {
        let deltas = u32::try_from(input.deltas.len())
            .map_err(|_| invariant("transfer delta occurrence count exceeds u32"))?;
        count
            .checked_add(deltas)
            .ok_or_else(|| invariant("transfer pair occurrence count exceeds u32"))
    })?;
    let expected_rows = pair_count
        .checked_mul(2)
        .ok_or_else(|| invariant("transfer participant occurrence count exceeds u32"))?;
    let actual_rows = transitions
        .iter()
        .filter(|row| row.operation == OperationKind::Transfer)
        .count();
    if usize::try_from(expected_rows).ok() != Some(actual_rows) {
        return Err(invariant(
            "transfer row and participant occurrence counts differ",
        ));
    }

    let mut pending: HashMap<TransferRowKey, VecDeque<PendingOccurrence<'a>>> = HashMap::new();
    let mut last_values: HashMap<Vec<u8>, u64> = HashMap::new();
    let mut current_root = None;
    let mut pair_ordinal = 0_u32;
    for (transcript_index, input) in inputs.iter().enumerate() {
        let transcript_ordinal = u32::try_from(transcript_index)
            .map_err(|_| invariant("transfer transcript ordinal exceeds u32"))?;
        if input.deltas.is_empty() {
            return Err(invariant(
                "transfer transcript must contain at least one delta",
            ));
        }
        for (delta_index, delta) in input.deltas.iter().enumerate() {
            let delta_ordinal = u32::try_from(delta_index)
                .map_err(|_| invariant("transfer delta ordinal exceeds u32"))?;
            check_delta(delta)?;
            let occurrence = TransferRowOccurrence {
                transcript_ordinal,
                delta_ordinal,
                pair_ordinal,
            };
            for role in [TransferRowRole::Debit, TransferRowRole::Credit] {
                let account = match role {
                    TransferRowRole::Debit => &delta.from_account,
                    TransferRowRole::Credit => &delta.to_account,
                };
                let key = iroha_data_model::fastpq::transfer_balance_key(
                    &delta.asset_definition,
                    account,
                )?;
                let (before, after) = participant_balances(delta, role);
                if last_values.get(&key).is_some_and(|value| *value != before) {
                    return Err(invariant("transfer repeated-key balances do not chain"));
                }
                last_values.insert(key.clone(), after);
                let proof = match role {
                    TransferRowRole::Debit => &delta.smt_proof.from,
                    TransferRowRole::Credit => &delta.smt_proof.to,
                };
                check_proof_shape(proof)?;
                if current_root.is_some_and(|root| root != proof.root_before) {
                    return Err(invariant("transfer occurrence roots do not chain"));
                }
                current_root = Some(proof.root_after);
                pending
                    .entry(TransferRowKey::new(
                        key,
                        before.to_le_bytes().to_vec(),
                        after.to_le_bytes().to_vec(),
                    ))
                    .or_default()
                    .push_back(PendingOccurrence {
                        occurrence,
                        role,
                        input,
                        delta,
                    });
            }
            pair_ordinal += 1; // Checked total bounds this increment, including the final delta.
        }
    }

    let mut bindings = Vec::with_capacity(transitions.len());
    for transition in transitions {
        if transition.operation != OperationKind::Transfer {
            bindings.push(None);
            continue;
        }
        let key = TransferRowKey::from_transition(transition);
        let queue = pending
            .get_mut(&key)
            .ok_or_else(|| invariant("transfer row has no matching validated occurrence"))?;
        let next = queue
            .pop_front()
            .ok_or_else(|| invariant("transfer row duplicates an already consumed occurrence"))?;
        bindings.push(Some(TransferRowBinding {
            occurrence: next.occurrence,
            role: next.role,
            input: next.input,
            delta: next.delta,
            transition,
        }));
    }
    if pending.values().any(|queue| !queue.is_empty()) {
        return Err(invariant(
            "validated transfer occurrence has no statement row",
        ));
    }
    Ok(bindings)
}

#[derive(Clone, Copy)]
struct PendingOccurrence<'a> {
    occurrence: TransferRowOccurrence,
    role: TransferRowRole,
    input: &'a TransferGadgetInput,
    delta: &'a TransferDeltaWitness,
}

fn invariant(details: &str) -> Error {
    Error::TransferInvariant {
        details: details.to_owned(),
    }
}

fn participant_balances(delta: &TransferDeltaWitness, role: TransferRowRole) -> (u64, u64) {
    match role {
        TransferRowRole::Debit => (delta.from_balance_before, delta.from_balance_after),
        TransferRowRole::Credit => (delta.to_balance_before, delta.to_balance_after),
    }
}

fn check_delta(delta: &TransferDeltaWitness) -> Result<(), Error> {
    if delta.from_balance_before.checked_sub(delta.amount) != Some(delta.from_balance_after) {
        return Err(invariant(
            "transfer sender amount and balances are inconsistent",
        ));
    }
    if delta.to_balance_before.checked_add(delta.amount) != Some(delta.to_balance_after) {
        return Err(invariant(
            "transfer receiver amount and balances are inconsistent",
        ));
    }
    if delta.from_account == delta.to_account
        && (delta.to_balance_before != delta.from_balance_after
            || delta.to_balance_after != delta.from_balance_before)
    {
        return Err(invariant("transfer self-transfer legs do not chain"));
    }
    Ok(())
}

fn check_proof_shape(proof: &TransferMerkleProof) -> Result<(), Error> {
    if proof.path_bits.len() != TRANSFER_MERKLE_HEIGHT.div_ceil(8)
        || proof.siblings.len() != TRANSFER_MERKLE_HEIGHT
    {
        return Err(invariant("transfer occurrence has a malformed SMT path"));
    }
    if [&proof.root_before, &proof.root_after]
        .into_iter()
        .chain(proof.siblings.iter())
        .any(|hash| hash[31] & 1 == 0)
    {
        return Err(invariant(
            "transfer occurrence has a noncanonical SMT hash marker",
        ));
    }
    Ok(())
}

fn pack_identity<const LIMBS: usize>(bytes: &[u8]) -> Result<PackedIdentity<u64, LIMBS>, Error> {
    PackedIdentity::from_bytes(bytes)
        .ok_or_else(|| invariant("transfer pair identity exceeds declared packing width"))
}

fn pack_context<const LIMBS: usize>(
    prefix: &[u8],
    context: &[u8],
) -> Result<PackedIdentity<u64, LIMBS>, Error> {
    let length = u32::try_from(context.len())
        .map_err(|_| invariant("transfer pair context length exceeds u32"))?;
    let capacity = LIMBS
        .checked_mul(7)
        .ok_or_else(|| invariant("transfer pair identity capacity overflows"))?;
    let total = prefix
        .len()
        .checked_add(4)
        .and_then(|size| size.checked_add(context.len()))
        .filter(|size| *size <= capacity)
        .ok_or_else(|| invariant("transfer pair context exceeds declared packing width"))?;
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(prefix);
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(context);
    pack_identity(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gadgets::{
        transfer::{
            attach_transfer_smt_witnesses, compute_poseidon_digest, transcripts_to_witnesses,
        },
        transfer_integer_air,
    };
    use iroha_crypto::Hash;
    use iroha_data_model::{
        asset::id::AssetDefinitionId,
        fastpq::{TransferDeltaTranscript, TransferSmtWitness, TransferTranscript},
    };
    use iroha_model_base::domain::DomainId;
    use iroha_primitives::numeric::Quantity;
    use iroha_test_samples::{ALICE_ID, BOB_ID};

    fn draft_delta(
        amount: u64,
        before: u64,
        receiver: u64,
        same_account: bool,
    ) -> TransferDeltaTranscript {
        TransferDeltaTranscript {
            from_account: (*ALICE_ID).clone(),
            to_account: if same_account {
                (*ALICE_ID).clone()
            } else {
                (*BOB_ID).clone()
            },
            asset_definition: AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            ),
            amount: Quantity::from(amount),
            from_balance_before: Quantity::from(before),
            from_balance_after: Quantity::from(before - amount),
            to_balance_before: Quantity::from(receiver),
            to_balance_after: Quantity::from(receiver + amount),
            from_smt_witness: TransferSmtWitness::default(),
            to_smt_witness: TransferSmtWitness::default(),
        }
    }

    fn fixture(groups: Vec<Vec<TransferDeltaTranscript>>) -> Vec<TransferGadgetInput> {
        let mut transcripts: Vec<_> = groups
            .into_iter()
            .map(|deltas| TransferTranscript {
                batch_hash: Hash::new(b"same batch hash, distinct occurrences"),
                authority_digest: Hash::new(b"authority context digest"),
                deltas,
                poseidon_preimage_digest: None,
            })
            .collect();
        let (old, new) = attach_transfer_smt_witnesses(&mut transcripts).unwrap();
        for transcript in &mut transcripts {
            if let [delta] = transcript.deltas.as_slice() {
                transcript.poseidon_preimage_digest =
                    Some(compute_poseidon_digest(delta, &transcript.batch_hash));
            }
        }
        transcripts_to_witnesses(&transcripts, &old, &new).unwrap()
    }

    fn rows(inputs: &[TransferGadgetInput]) -> Vec<StateTransition> {
        let mut rows = Vec::new();
        for input in inputs {
            for delta in &input.deltas {
                for role in [TransferRowRole::Debit, TransferRowRole::Credit] {
                    let account = if role == TransferRowRole::Debit {
                        &delta.from_account
                    } else {
                        &delta.to_account
                    };
                    let (before, after) = participant_balances(delta, role);
                    rows.push(StateTransition::new(
                        iroha_data_model::fastpq::transfer_balance_key(
                            &delta.asset_definition,
                            account,
                        )
                        .unwrap(),
                        before.to_le_bytes().to_vec(),
                        after.to_le_bytes().to_vec(),
                        OperationKind::Transfer,
                    ));
                }
            }
        }
        rows.sort_by(|a, b| (&a.key, a.operation_rank()).cmp(&(&b.key, b.operation_rank())));
        rows
    }

    fn assert_integer(binding: &TransferRowBinding<'_>) {
        let integer = binding.integer_witness();
        assert_eq!(integer.is_debit, binding.role().is_debit());
        assert!(
            transfer_integer_air::constraint_residues(1, 8, 8, &integer)
                .into_iter()
                .all(|value| value == 0)
        );
    }

    #[test]
    fn canonical_rows_bind_exact_borrowed_occurrences_and_metadata() {
        let inputs = fixture(vec![
            vec![draft_delta(7, 90, 3, false)],
            vec![draft_delta(2, 83, 10, false)],
        ]);
        let mut transitions = rows(&inputs);
        transitions.push(StateTransition::new(
            b"metadata".to_vec(),
            vec![1],
            vec![2],
            OperationKind::MetaSet,
        ));
        let bindings = bind_canonical_rows(&transitions, &inputs).unwrap();
        assert_eq!(bindings.len(), transitions.len());
        assert!(bindings.last().unwrap().is_none());
        for (row, binding) in transitions
            .iter()
            .zip(&bindings)
            .filter_map(|(r, b)| b.as_ref().map(|b| (r, b)))
        {
            assert!(std::ptr::eq(binding.transition(), row));
            let occurrence = binding.occurrence();
            assert!(std::ptr::eq(
                binding.input(),
                &inputs[occurrence.transcript_ordinal as usize]
            ));
            assert!(std::ptr::eq(
                binding.delta(),
                &binding.input().deltas[occurrence.delta_ordinal as usize]
            ));
            let expected = if binding.role() == TransferRowRole::Debit {
                &binding.delta().smt_proof.from
            } else {
                &binding.delta().smt_proof.to
            };
            assert!(std::ptr::eq(binding.proof(), expected));
            assert_integer(binding);
        }
    }

    #[test]
    fn repeated_identical_zero_self_rows_keep_fifo_roles_and_occurrences() {
        let delta = draft_delta(0, 17, 17, true);
        let inputs = fixture(vec![vec![delta.clone(), delta.clone()], vec![delta]]);
        let transitions = rows(&inputs);
        assert!(transitions.windows(2).all(|rows| rows[0] == rows[1]));
        let bindings = bind_canonical_rows(&transitions, &inputs).unwrap();
        for (index, binding) in bindings.into_iter().enumerate() {
            let binding = binding.unwrap();
            assert_eq!(binding.occurrence().pair_ordinal, (index / 2) as u32);
            assert_eq!(
                binding.occurrence().transcript_ordinal,
                u32::from(index >= 4)
            );
            assert_eq!(
                binding.occurrence().delta_ordinal,
                if index < 4 { (index / 2) as u32 } else { 0 }
            );
            assert_eq!(binding.role().is_debit(), u64::from(index % 2 == 0));
            assert_integer(&binding);
            let cloned = binding;
            assert_eq!(cloned.occurrence(), binding.occurrence());
            assert!(std::ptr::eq(cloned.proof(), binding.proof()));
        }
    }

    #[test]
    fn equal_key_statement_reordering_preserves_chronological_proof_identity() {
        let inputs = fixture(vec![vec![
            draft_delta(5, 70, 65, true),
            draft_delta(3, 70, 67, true),
        ]]);
        let mut transitions = rows(&inputs);
        transitions.reverse(); // Equal keys/ranks permit any original stable input order.
        let bindings = bind_canonical_rows(&transitions, &inputs).unwrap();
        let sequence: Vec<_> = bindings
            .into_iter()
            .map(|row| {
                let row = row.unwrap();
                assert_integer(&row);
                (row.occurrence().pair_ordinal, row.role())
            })
            .collect();
        assert_eq!(
            sequence,
            vec![
                (1, TransferRowRole::Credit),
                (1, TransferRowRole::Debit),
                (0, TransferRowRole::Credit),
                (0, TransferRowRole::Debit)
            ]
        );
    }

    #[test]
    fn repeated_identical_rows_retain_different_intervening_state_proofs() {
        let zero = draft_delta(0, 20, 4, false);
        let mut intervening = draft_delta(3, 50, 7, false);
        intervening.asset_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "tulip".parse().unwrap(),
        );
        let inputs = fixture(vec![vec![zero.clone(), intervening, zero]]);
        assert_ne!(
            inputs[0].deltas[0].smt_proof.from.root_before,
            inputs[0].deltas[2].smt_proof.from.root_before
        );
        let transitions = rows(&inputs);
        let bindings = bind_canonical_rows(&transitions, &inputs).unwrap();
        let repeated: Vec<_> = bindings
            .iter()
            .flatten()
            .filter(|binding| {
                binding.role() == TransferRowRole::Debit && binding.delta().amount == 0
            })
            .collect();
        assert_eq!(repeated.len(), 2);
        assert_eq!(repeated[0].transition(), repeated[1].transition());
        for (binding, pair) in repeated.into_iter().zip([0, 2]) {
            assert_eq!(binding.occurrence().pair_ordinal, pair as u32);
            assert!(std::ptr::eq(
                binding.proof(),
                &inputs[0].deltas[pair].smt_proof.from
            ));
            assert_integer(binding);
        }
    }

    #[test]
    fn full_u64_amounts_and_carries_are_not_field_reduced() {
        for (amount, before, receiver) in
            [(u64::MAX, u64::MAX, 0), (1, u64::MAX, u64::from(u32::MAX))]
        {
            let inputs = fixture(vec![vec![draft_delta(amount, before, receiver, false)]]);
            let transitions = rows(&inputs);
            for binding in bind_canonical_rows(&transitions, &inputs)
                .unwrap()
                .into_iter()
                .flatten()
            {
                assert_integer(&binding);
                assert_eq!(
                    binding.integer_witness().amount.packed,
                    Unsigned64Witness::from_integer(amount).packed
                );
            }
        }
    }

    #[test]
    fn missing_extra_duplicate_and_malformed_rows_are_rejected() {
        let inputs = fixture(vec![vec![draft_delta(4, 20, 2, false)]]);
        let original = rows(&inputs);
        let mut missing = original.clone();
        missing.pop();
        assert!(bind_canonical_rows(&missing, &inputs).is_err());
        let mut extra = original.clone();
        extra.push(original[1].clone());
        assert!(bind_canonical_rows(&extra, &inputs).is_err());
        let duplicate = vec![original[0].clone(), original[0].clone()];
        assert!(bind_canonical_rows(&duplicate, &inputs).is_err());
        for malformed in [vec![0; 7], vec![0; 9], 123_u64.to_le_bytes().to_vec()] {
            let mut changed = original.clone();
            changed[0].pre_value = malformed;
            assert!(bind_canonical_rows(&changed, &inputs).is_err());
        }
        let mut changed = original.clone();
        changed.reverse();
        assert!(bind_canonical_rows(&changed, &inputs).is_err());
        assert!(bind_canonical_rows(&original, &[]).is_err());
        assert!(bind_canonical_rows(&[], &inputs).is_err());
        assert!(bind_canonical_rows(&[], &[]).unwrap().is_empty());
    }

    #[test]
    fn mutated_public_witness_structs_fail_cheap_consistency_checks() {
        let inputs = fixture(vec![vec![
            draft_delta(4, 20, 2, false),
            draft_delta(1, 16, 6, false),
        ]]);
        let transitions = rows(&inputs);
        for mutation in 0..8 {
            let mut changed = inputs.clone();
            let delta = &mut changed[0].deltas[0];
            match mutation {
                0 => delta.amount += 1,
                1 => delta.smt_proof.from.path_bits.pop().map(|_| ()).unwrap(),
                2 => delta.smt_proof.to.siblings.pop().map(|_| ()).unwrap(),
                3 => delta.smt_proof.from.siblings[0][31] &= !1,
                4 => delta.smt_proof.to.root_before[0] ^= 1,
                5 => delta.smt_proof.from.root_before[31] &= !1,
                6 => {
                    changed[0].deltas[1].from_balance_before += 1;
                    changed[0].deltas[1].from_balance_after += 1;
                }
                _ => changed[0].deltas.clear(),
            }
            assert!(
                bind_canonical_rows(&transitions, &changed).is_err(),
                "mutation {mutation}"
            );
        }
        let mut changed = inputs.clone();
        changed[0].deltas[0].to_account = changed[0].deltas[0].from_account.clone();
        assert!(bind_canonical_rows(&transitions, &changed).is_err());
    }

    fn unpack<const N: usize>(packed: PackedIdentity<u64, N>) -> Vec<u8> {
        let mut bytes: Vec<_> = packed
            .limbs
            .into_iter()
            .flat_map(|limb| limb.to_le_bytes().into_iter().take(7))
            .collect();
        assert!(
            bytes[packed.byte_len as usize..]
                .iter()
                .all(|byte| *byte == 0)
        );
        bytes.truncate(packed.byte_len as usize);
        bytes
    }

    #[test]
    fn pair_tuple_encodes_full_identities_lengths_context_and_declared_role() {
        let delta = draft_delta(0, u64::MAX, u64::MAX, true);
        let inputs = fixture(vec![vec![delta.clone()], vec![delta]]);
        let transitions = rows(&inputs);
        let bindings = bind_canonical_rows(&transitions, &inputs).unwrap();
        let context = TransferPairContext {
            call_identity: b"call\0",
            authority_identity: b"authority\0",
            asset_scale: 9,
        };
        let mut tuples = Vec::new();
        for binding in bindings.into_iter().flatten() {
            let tuple = binding.pair_tuple::<128, 128>(context).unwrap();
            assert_eq!(unpack(tuple.key), binding.transition().key);
            assert_eq!(
                unpack(tuple.identity.asset),
                binding.delta().asset_definition.encode()
            );
            assert_eq!(
                unpack(tuple.identity.sender),
                binding.delta().from_account.encode()
            );
            assert_eq!(
                unpack(tuple.identity.receiver),
                binding.delta().to_account.encode()
            );
            let mut call = binding.input().batch_hash.as_ref().to_vec();
            call.extend_from_slice(&binding.occurrence().transcript_ordinal.to_le_bytes());
            call.extend_from_slice(&binding.occurrence().delta_ordinal.to_le_bytes());
            call.extend_from_slice(&(context.call_identity.len() as u32).to_le_bytes());
            call.extend_from_slice(context.call_identity);
            assert_eq!(unpack(tuple.identity.call), call);
            let mut authority = binding.input().authority_digest.as_ref().to_vec();
            authority.extend_from_slice(&(context.authority_identity.len() as u32).to_le_bytes());
            authority.extend_from_slice(context.authority_identity);
            assert_eq!(unpack(tuple.identity.authority), authority);
            assert_eq!(tuple.identity.asset_scale, 9);
            assert_eq!(
                tuple.before,
                Unsigned64Witness::from_integer(u64::MAX).packed
            );
            assert_eq!(tuple.is_debit, binding.role().is_debit());
            assert_eq!(
                crate::gadgets::transfer_pair_air::integer_binding_residues(
                    1,
                    &tuple,
                    &binding.integer_witness(),
                ),
                [0; 7]
            );
            if binding.role() == TransferRowRole::Debit {
                let inferred = TransferIntegerWitness::from_balances(u64::MAX, u64::MAX);
                assert_ne!(
                    crate::gadgets::transfer_pair_air::integer_binding_residues(
                        1, &tuple, &inferred,
                    ),
                    [0; 7],
                    "the original balance-inferred constructor loses zero debit identity"
                );
            }
            assert!(binding.pair_tuple::<1, 128>(context).is_err());
            assert!(binding.pair_tuple::<128, 1>(context).is_err());
            tuples.push(tuple);
        }
        assert_eq!(tuples[0].identity, tuples[1].identity);
        assert_ne!(tuples[0].identity.call, tuples[2].identity.call);
        let plain = pack_context::<2>(b"x", b"z").unwrap();
        let zero = pack_context::<2>(b"x", b"z\0").unwrap();
        assert_ne!(plain, zero);
        assert!(pack_context::<1>(b"1234", b"").is_err());
        assert!(pack_identity::<0>(b"x").is_err());
    }
}
