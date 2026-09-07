//! Bounded public transfer claims, separated from private SMT path witnesses.
//!
//! These constructors check the current public accounts, amounts, balances,
//! occurrence multiplicity, stable key order and collision-resolved allocation.
//! They compute only public key/value/leaf hashes and the existing public
//! Poseidon and ordering digests. They never inspect SMT siblings or witness
//! roots, construct an SMT, or invoke a trace, FFT, LDE or proof verifier.
//!
//! Construction does not authenticate a caller, authority digest, transaction or
//! state root. The enclosing verifier must bind the complete public claims,
//! profile, PublicIO and intermediate-root commitments before its challenges,
//! and prove every private node hash and root link. The current transfer roots
//! describe a touched-balance tree, not automatically consensus-wide state.
//! TODO: Integrate this public/private boundary into a reviewed succinct proof
//! schema; this module does not change admission or remove native replay.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    io::{self, Write},
};

use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    asset::id::AssetDefinitionId,
    fastpq::{TransferDeltaTranscript, TransferTranscript, normalized_numeric_to_u64},
};
use iroha_primitives::numeric::Quantity;
use iroha_zkp_halo2::poseidon::PoseidonByteHasher;
use norito::codec::Encode as NoritoEncode;

use super::{
    compact_smt_air::{DigestLimbs, PublicStatement, PublicUpdate},
    transfer_row_binding::{TransferRowOccurrence, TransferRowRole},
};
use crate::{
    Error, OperationKind, ProofSemantics, PublicInputs, Result, StateTransition, VerifyLimits,
};

const KEY_DOMAIN: &[u8] = b"fastpq:v1:smt:key|";
const VALUE_DOMAIN: &[u8] = b"fastpq:v1:smt:value|";
const LEAF_DOMAIN: &[u8] = b"fastpq:v1:smt:leaf|";
const ORDERING_DOMAIN: &[u8] = b"fastpq:v1:ordering";

// The public transport model owns the path-free occurrence types. Preparing a
// relation still requires the private constructor and bounded checks below;
// successful model decoding alone does not perform those checks.
pub use iroha_data_model::fastpq::{
    FastpqPublicTransferDeltaV1 as PublicTransferDelta,
    FastpqPublicTransferTranscriptV1 as PublicTransferTranscript,
};

/// Explicit bounds applied before derived tables or hashes are allocated.
///
/// Defaults inherit existing verifier transition and batch ceilings; they are
/// independent preparation ceilings, not a promise that a succinct proof fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicTransferLimits {
    /// Maximum transcript occurrences.
    pub max_transcripts: usize,
    /// Maximum complete debit/credit pairs.
    pub max_deltas: usize,
    /// Maximum canonical execution rows.
    pub max_rows: usize,
    /// Maximum counted canonical public bytes, excluding all private witnesses.
    pub max_public_bytes: usize,
    /// Maximum distinct complete balance keys.
    pub max_unique_keys: usize,
    /// Maximum occupied-interval lookups during first-free path allocation.
    pub max_allocation_steps: usize,
}

impl Default for PublicTransferLimits {
    fn default() -> Self {
        let verifier = VerifyLimits::default();
        Self {
            max_transcripts: verifier.max_transitions / 2,
            max_deltas: verifier.max_transitions / 2,
            max_rows: verifier.max_transitions,
            max_public_bytes: verifier.max_batch_bytes,
            max_unique_keys: verifier.max_transitions,
            max_allocation_steps: verifier.max_transitions.saturating_mul(4),
        }
    }
}

/// Complete canonical key hash and its unique collision-resolved path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKeyAllocation {
    /// Exact full UTF-8 `asset/{asset_definition}/{account}` bytes.
    pub key: Vec<u8>,
    /// Marked BLAKE2b-256 of the complete key-domain message, including long keys.
    pub key_hash: [u8; 32],
    /// First-free wrapping probe in lexicographic full-key allocation order.
    pub path: u32,
}

/// Normalized occurrence associated with one exact canonical execution row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicTransferRow {
    /// Original transcript/delta/pair ordinal.
    pub occurrence: TransferRowOccurrence,
    /// Declared sender or receiver role, including zero-amount updates.
    pub role: TransferRowRole,
    /// Index into the complete sorted public key/allocation table.
    pub key_index: usize,
    /// Asset normalization scale selected from the original public claims.
    pub asset_scale: u32,
    /// Exact unsigned pre-balance.
    pub before: u64,
    /// Exact unsigned post-balance.
    pub after: u64,
    /// Exact declared unsigned amount, not inferred from equal balances.
    pub amount: u64,
    /// Full public leaf hashes and allocated path for this update.
    pub update: PublicUpdate,
}

/// A delta in chronological order, with ports into the key-sorted execution rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicTransferPair {
    /// Original transcript/delta/pair ordinal.
    pub occurrence: TransferRowOccurrence,
    /// Canonical execution row indices, debit then credit.
    pub row_indices: [usize; 2],
    /// Exact public leaf/path bindings, debit then credit.
    pub updates: [PublicUpdate; 2],
}

/// Checked public-work counts; no private node hashes are included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicPreparationWork {
    /// Sum of the fixed-default bare Norito public field encodings documented below.
    pub public_bytes: usize,
    /// One complete hash per unique key, also reused for allocation and leaves.
    pub key_hashes: usize,
    /// One before/after value hash for every participant update.
    pub value_hashes: usize,
    /// One before/after leaf hash for every participant update.
    pub leaf_hashes: usize,
    /// Number of deterministic occupied-interval lookups used by allocation.
    pub allocation_steps: usize,
}

/// Validated public bindings, borrowing original claims and canonical execution rows.
///
/// Private fields prevent replacing a validated table without rerunning preparation.
/// This type establishes internal consistency only, not external authentication.
#[derive(Debug)]
pub struct PreparedPublicTransfers<'a> {
    claims: &'a [PublicTransferTranscript],
    transitions: &'a [StateTransition],
    public_inputs: PublicInputs,
    semantics: ProofSemantics,
    rows: Vec<PublicTransferRow>,
    pairs: Vec<PublicTransferPair>,
    keys: Vec<PublicKeyAllocation>,
    ordering_hash: Hash,
    work: PublicPreparationWork,
}

impl<'a> PreparedPublicTransfers<'a> {
    /// Original exact public typed quantities and transcript identities.
    #[must_use]
    pub const fn claims(&self) -> &'a [PublicTransferTranscript] {
        self.claims
    }
    /// Actual key-sorted execution rows, not an independently invented copy.
    #[must_use]
    pub const fn transitions(&self) -> &'a [StateTransition] {
        self.transitions
    }
    /// Complete caller-supplied public input bytes, with no scalar projection.
    #[must_use]
    pub const fn public_inputs(&self) -> &PublicInputs {
        &self.public_inputs
    }
    /// Explicit selected transfer profile; metadata never selects this value.
    #[must_use]
    pub const fn semantics(&self) -> ProofSemantics {
        self.semantics
    }
    /// One occurrence binding per canonical execution row, in the same order.
    #[must_use]
    pub fn rows(&self) -> &[PublicTransferRow] {
        &self.rows
    }
    /// Original chronological deltas, each containing debit then credit ports.
    #[must_use]
    pub fn pairs(&self) -> &[PublicTransferPair] {
        &self.pairs
    }
    /// Complete unique keys and allocation, in bytewise lexicographic order.
    #[must_use]
    pub fn keys(&self) -> &[PublicKeyAllocation] {
        &self.keys
    }
    /// Native ordering-domain hash of default-layout Norito canonical transitions.
    #[must_use]
    pub const fn ordering_hash(&self) -> Hash {
        self.ordering_hash
    }
    /// Exact public hash counts and checked preparation byte/allocation work.
    #[must_use]
    pub const fn work(&self) -> PublicPreparationWork {
        self.work
    }

    /// Bind one compact SMT statement per delta to an explicit shared root chain.
    ///
    /// Supply exactly `pairs.len()-1` intermediate roots, independently committed
    /// as public claims before proof challenges. No root is read from a private
    /// witness. The first and last roots are always the original complete public
    /// inputs; adjacent statements share the exact same supplied intermediate.
    /// Acceptance still needs a valid proof for every statement and an authorized
    /// source root. Multiple returned statements do not establish multi-delta
    /// capacity in the current one-delta compact trace.
    ///
    /// # Errors
    /// Returns an error for an incorrect intermediate count or unmarked digest.
    pub fn compact_statements(
        &self,
        intermediate_roots: &[[u8; 32]],
    ) -> Result<Vec<PublicStatement>> {
        let expected = self.pairs.len().saturating_sub(1);
        if intermediate_roots.len() != expected {
            return Err(invariant(
                "public transfer intermediate-root count mismatch",
            ));
        }
        if self.pairs.is_empty() {
            return Ok(Vec::new());
        }
        for root in intermediate_roots {
            require_marked(root)?;
        }
        let mut current = self.public_inputs.old_root;
        Ok(self
            .pairs
            .iter()
            .enumerate()
            .map(|(index, pair)| {
                let next = intermediate_roots
                    .get(index)
                    .copied()
                    .unwrap_or(self.public_inputs.new_root);
                let statement = PublicStatement {
                    updates: pair.updates,
                    old_root: digest_limbs(current),
                    new_root: digest_limbs(next),
                };
                current = next;
                statement
            })
            .collect())
    }
}

/// Copy only public fields from already-decoded legacy transcript metadata.
///
/// Neither SMT root, path bits nor siblings are inspected, encoded or cloned.
/// This convenience boundary does not make decoding legacy private metadata a
/// succinct verifier operation; the eventual public interface must supply these
/// path-free claims directly. Empty/multi/single-delta policy is checked by
/// [`prepare_public_transfers`].
///
/// # Errors
/// Returns an error before copying if public count or byte limits are exceeded.
pub fn public_claims_from_transcripts(
    transcripts: &[TransferTranscript],
    limits: PublicTransferLimits,
) -> Result<Vec<PublicTransferTranscript>> {
    check_limit(
        "max_public_transfer_transcripts",
        transcripts.len(),
        limits.max_transcripts,
    )?;
    let mut budget = ByteBudget::new(limits.max_public_bytes);
    budget.encoded(&checked_u32(transcripts.len())?)?;
    let mut count = 0_usize;
    for transcript in transcripts {
        count = checked_add(count, transcript.deltas.len())?;
        check_limit("max_public_transfer_deltas", count, limits.max_deltas)?;
        measure_header(
            &mut budget,
            &transcript.batch_hash,
            &transcript.authority_digest,
            transcript.poseidon_preimage_digest,
            transcript.deltas.len(),
        )?;
        for delta in &transcript.deltas {
            measure_delta(&mut budget, DeltaView::from(delta))?;
        }
    }
    Ok(transcripts
        .iter()
        .map(PublicTransferTranscript::from)
        .collect())
}

/// Prepare exact public transfer semantics and compact SMT leaf/path bindings.
///
/// Work is bounded by public bytes/occurrences plus key ordering and allocation.
/// Hash-map iteration never controls output. An occupied-interval allocator
/// implements native first-free wrapping probes with at most four O(log U)
/// lookups per key, avoiding quadratic collision scans. Every claim occurs once
/// in the full-key/eight-byte-value FIFO row match; repeated-key balances also
/// chain in chronological update order. Source state and private path validity
/// remain outside this constructor.
///
/// Public-byte accounting streams fixed-default bare Norito encodings of full
/// PublicInputs, the canonical row vector, a u32 transcript count, each transcript's
/// two hashes/optional digest/u32 delta count, and each delta's three identities
/// and five quantities in declaration order. It is a resource measure, not a new
/// wire schema or substitute for binding those fields in the enclosing statement.
///
/// # Errors
/// Returns an error for limits, noncanonical order, unsupported profile/operations,
/// digest policy, normalization/arithmetic, incomplete row coverage or allocation.
#[allow(clippy::too_many_lines)]
pub fn prepare_public_transfers<'a>(
    transitions: &'a [StateTransition],
    claims: &'a [PublicTransferTranscript],
    public_inputs: PublicInputs,
    semantics: ProofSemantics,
    limits: PublicTransferLimits,
) -> Result<PreparedPublicTransfers<'a>> {
    check_limit(
        "max_public_transfer_rows",
        transitions.len(),
        limits.max_rows,
    )?;
    check_limit(
        "max_public_transfer_transcripts",
        claims.len(),
        limits.max_transcripts,
    )?;
    // Reject oversized raw public byte vectors before the codec can buffer
    // sequence fields while counting their exact framed representation.
    let mut raw_row_bytes = 0;
    for row in transitions {
        for bytes in [&row.key, &row.pre_value, &row.post_value] {
            raw_row_bytes = checked_add(raw_row_bytes, bytes.len())?;
            check_limit(
                "max_public_transfer_bytes",
                raw_row_bytes,
                limits.max_public_bytes,
            )?;
        }
    }
    // This bounded public-only copy has the same nominal Norito type as the
    // existing ordering commitment, and is reused for that archive below.
    let canonical_row_vec = transitions.to_vec();
    let mut budget = ByteBudget::new(limits.max_public_bytes);
    budget.encoded(&public_inputs)?;
    budget.encoded(&canonical_row_vec)?;
    budget.encoded(&checked_u32(claims.len())?)?;
    let mut pair_count = 0_usize;
    for claim in claims {
        pair_count = checked_add(pair_count, claim.deltas.len())?;
        check_limit("max_public_transfer_deltas", pair_count, limits.max_deltas)?;
        measure_header(
            &mut budget,
            &claim.batch_hash,
            &claim.authority_digest,
            claim.poseidon_preimage_digest,
            claim.deltas.len(),
        )?;
        for delta in &claim.deltas {
            measure_delta(&mut budget, DeltaView::from(delta))?;
        }
    }
    let row_count = pair_count
        .checked_mul(2)
        .ok_or_else(|| invariant("public transfer row count overflows"))?;
    checked_u32(row_count)?;
    let hashes_per_family = row_count
        .checked_mul(2)
        .ok_or_else(|| invariant("public value/leaf hash count overflows"))?;
    if row_count != transitions.len() {
        return Err(invariant("public transfer pair/row count mismatch"));
    }
    validate_profile(transitions, claims, &public_inputs, semantics)?;
    if transitions.windows(2).any(|rows| {
        (&rows[0].key, rows[0].operation_rank()) > (&rows[1].key, rows[1].operation_rank())
    }) {
        return Err(invariant(
            "public transfer rows are not in canonical statement order",
        ));
    }
    let scales = asset_scales(claims);
    let mut pending: HashMap<RowKey, VecDeque<PendingRow>> = HashMap::new();
    let mut last_values: HashMap<Vec<u8>, u64> = HashMap::new();
    let mut ordinal = 0_u32;
    for (transcript_index, claim) in claims.iter().enumerate() {
        check_digest_policy(claim)?;
        for (delta_index, delta) in claim.deltas.iter().enumerate() {
            let scale = scales[&delta.asset_definition];
            let values = normalized_values(delta, scale)?;
            let occurrence = TransferRowOccurrence {
                transcript_ordinal: checked_u32(transcript_index)?,
                delta_ordinal: checked_u32(delta_index)?,
                pair_ordinal: ordinal,
            };
            for (leg, (account, before, after)) in [
                (&delta.from_account, values[1], values[2]),
                (&delta.to_account, values[3], values[4]),
            ]
            .into_iter()
            .enumerate()
            {
                let key = balance_key(&delta.asset_definition, account);
                if last_values
                    .get(&key)
                    .is_some_and(|previous| *previous != before)
                {
                    return Err(invariant("public repeated-key balances do not chain"));
                }
                last_values.insert(key.clone(), after);
                pending
                    .entry(RowKey { key, before, after })
                    .or_default()
                    .push_back(PendingRow {
                        occurrence,
                        leg,
                        scale,
                        amount: values[0],
                    });
            }
            ordinal += 1; // Checked two-row cardinality bounds this increment.
        }
    }
    let mut keys: Vec<PublicKeyAllocation> = Vec::new();
    let mut ordered = Vec::with_capacity(transitions.len());
    for transition in transitions {
        let before = decode_balance(&transition.pre_value)?;
        let after = decode_balance(&transition.post_value)?;
        let key = RowKey {
            key: transition.key.clone(),
            before,
            after,
        };
        let occurrence = pending
            .get_mut(&key)
            .and_then(VecDeque::pop_front)
            .ok_or_else(|| invariant("public transfer row lacks an unconsumed exact occurrence"))?;
        if keys
            .last()
            .is_none_or(|previous| previous.key != transition.key)
        {
            let count = checked_add(keys.len(), 1)?;
            check_limit(
                "max_public_transfer_unique_keys",
                count,
                limits.max_unique_keys,
            )?;
            keys.push(PublicKeyAllocation {
                key: transition.key.clone(),
                key_hash: [0; 32],
                path: 0,
            });
        }
        ordered.push((occurrence, keys.len() - 1, before, after));
    }
    if pending.values().any(|queue| !queue.is_empty()) {
        return Err(invariant(
            "public transfer occurrence lacks an execution row",
        ));
    }
    let mut occupied = BTreeMap::new();
    let mut allocation_steps = 0;
    let unique_count = keys.len();
    for key in &mut keys {
        key.key_hash = Hash::new_from_chunks(&[KEY_DOMAIN, &key.key]).into();
        let base = u32::from_le_bytes(key.key_hash[..4].try_into().expect("four key-hash bytes"));
        key.path = allocate_path(
            &mut occupied,
            base,
            unique_count,
            &mut allocation_steps,
            limits.max_allocation_steps,
        )?;
    }
    let mut rows = Vec::with_capacity(row_count);
    let mut pair_rows = vec![[usize::MAX; 2]; pair_count];
    for (index, (pending, key_index, before, after)) in ordered.into_iter().enumerate() {
        let key = &keys[key_index];
        let update = PublicUpdate {
            old_leaf: digest_limbs(public_leaf(&key.key_hash, before)),
            new_leaf: digest_limbs(public_leaf(&key.key_hash, after)),
            path: key.path,
        };
        rows.push(PublicTransferRow {
            occurrence: pending.occurrence,
            role: if pending.leg == 0 {
                TransferRowRole::Debit
            } else {
                TransferRowRole::Credit
            },
            key_index,
            asset_scale: pending.scale,
            before,
            after,
            amount: pending.amount,
            update,
        });
        let slot = &mut pair_rows[pending.occurrence.pair_ordinal as usize][pending.leg];
        if *slot != usize::MAX {
            return Err(invariant(
                "duplicate public transfer participant occurrence",
            ));
        }
        *slot = index;
    }
    let pairs = pair_rows
        .into_iter()
        .map(|indices| {
            if indices.contains(&usize::MAX) {
                return Err(invariant("missing public transfer participant occurrence"));
            }
            Ok(PublicTransferPair {
                occurrence: rows[indices[0]].occurrence,
                row_indices: indices,
                updates: [rows[indices[0]].update, rows[indices[1]].update],
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let ordering_hash = {
        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        // The existing ordering hash uses the nominal Vec<StateTransition>
        // archive schema. A slice's distinct schema header is not substitutable.
        let encoded = norito::core::to_bytes(&canonical_row_vec)?;
        Hash::new_from_chunks(&[ORDERING_DOMAIN, &encoded])
    };
    Ok(PreparedPublicTransfers {
        claims,
        transitions,
        public_inputs,
        semantics,
        rows,
        pairs,
        keys,
        ordering_hash,
        work: PublicPreparationWork {
            public_bytes: budget.used,
            key_hashes: unique_count,
            value_hashes: hashes_per_family,
            leaf_hashes: hashes_per_family,
            allocation_steps,
        },
    })
}

#[derive(Hash, PartialEq, Eq)]
struct RowKey {
    key: Vec<u8>,
    before: u64,
    after: u64,
}
struct PendingRow {
    occurrence: TransferRowOccurrence,
    leg: usize,
    scale: u32,
    amount: u64,
}

fn validate_profile(
    rows: &[StateTransition],
    claims: &[PublicTransferTranscript],
    inputs: &PublicInputs,
    semantics: ProofSemantics,
) -> Result<()> {
    let invalid = |details: &str| Error::InvalidProofSemantics {
        profile: semantics.name(),
        details: details.into(),
    };
    if semantics == ProofSemantics::AxtOpaqueEffect {
        return Err(invalid("opaque effects are not public transfer claims"));
    }
    if rows.is_empty() {
        if semantics == ProofSemantics::AxtTransferClaim {
            return Err(invalid("AXT transfer claims must be nonempty"));
        }
        if !claims.is_empty() || inputs.old_root != inputs.new_root {
            return Err(invalid(
                "empty transfer statement must contain no claims and preserve its root",
            ));
        }
        return Ok(());
    }
    if rows
        .iter()
        .any(|row| row.operation != OperationKind::Transfer)
    {
        return Err(invalid(
            "public transfer statement contains a non-transfer operation",
        ));
    }
    require_marked(&inputs.old_root)?;
    require_marked(&inputs.new_root)
}

fn asset_scales(claims: &[PublicTransferTranscript]) -> BTreeMap<AssetDefinitionId, u32> {
    let mut scales = BTreeMap::<AssetDefinitionId, u32>::new();
    let mut seeded = BTreeSet::new();
    for claim in claims {
        for delta in &claim.deltas {
            let scale = scales.entry(delta.asset_definition.clone()).or_default();
            *scale = (*scale).max(delta.amount.scale());
            for (account, before, after) in [
                (
                    &delta.from_account,
                    &delta.from_balance_before,
                    &delta.from_balance_after,
                ),
                (
                    &delta.to_account,
                    &delta.to_balance_before,
                    &delta.to_balance_after,
                ),
            ] {
                if seeded.insert((delta.asset_definition.clone(), account.clone())) {
                    *scale = (*scale).max(before.scale()).max(after.scale());
                }
            }
        }
    }
    scales
}

fn normalized_values(delta: &PublicTransferDelta, scale: u32) -> Result<[u64; 5]> {
    let mut values = [0; 5];
    for (index, (field, quantity)) in [
        ("amount", &delta.amount),
        ("from_balance_before", &delta.from_balance_before),
        ("from_balance_after", &delta.from_balance_after),
        ("to_balance_before", &delta.to_balance_before),
        ("to_balance_after", &delta.to_balance_after),
    ]
    .into_iter()
    .enumerate()
    {
        values[index] = normalized_numeric_to_u64(quantity.as_numeric(), scale)
            .ok_or(Error::TransferNumericBounds { field })?;
    }
    let [amount, from_before, from_after, to_before, to_after] = values;
    if from_before.checked_sub(amount) != Some(from_after) {
        return Err(invariant("public sender arithmetic mismatch or underflow"));
    }
    if to_before.checked_add(amount) != Some(to_after) {
        return Err(invariant("public receiver arithmetic mismatch or overflow"));
    }
    if delta.from_account == delta.to_account
        && (to_before != from_after || to_after != from_before)
    {
        return Err(invariant("public self-transfer legs do not chain"));
    }
    Ok(values)
}

fn check_digest_policy(claim: &PublicTransferTranscript) -> Result<()> {
    match claim.deltas.as_slice() {
        [] => Err(invariant(
            "public transfer transcript must contain at least one delta",
        )),
        [delta] => {
            let expected = claim
                .poseidon_preimage_digest
                .ok_or_else(|| invariant("single public delta requires its Poseidon digest"))?;
            let mut hasher = PoseidonByteHasher::new();
            delta.from_account.encode_to(&mut hasher);
            delta.to_account.encode_to(&mut hasher);
            delta.asset_definition.encode_to(&mut hasher);
            delta.amount.encode_to(&mut hasher);
            hasher.update(claim.batch_hash.as_ref());
            if Hash::prehashed(hasher.finalize()) != expected {
                return Err(invariant("public transfer Poseidon digest mismatch"));
            }
            Ok(())
        }
        _ if claim.poseidon_preimage_digest.is_some() => Err(invariant(
            "multiple public deltas must omit the Poseidon digest",
        )),
        _ => Ok(()),
    }
}

fn public_leaf(key_hash: &[u8; 32], value: u64) -> [u8; 32] {
    let value_hash = Hash::new_from_chunks(&[VALUE_DOMAIN, &value.to_le_bytes()]);
    Hash::new_from_chunks(&[LEAF_DOMAIN, key_hash, value_hash.as_ref()]).into()
}

fn digest_limbs(bytes: [u8; 32]) -> DigestLimbs {
    core::array::from_fn(|index| {
        u32::from_le_bytes(
            bytes[index * 4..index * 4 + 4]
                .try_into()
                .expect("four digest bytes"),
        )
    })
}

fn allocate_path(
    occupied: &mut BTreeMap<u32, u32>,
    base: u32,
    key_count: usize,
    steps: &mut usize,
    max_steps: usize,
) -> Result<u32> {
    let mut charge = || {
        *steps = checked_add(*steps, 1)?;
        check_limit("max_public_transfer_allocation_steps", *steps, max_steps)
    };
    charge()?;
    let mut candidate = base;
    if let Some((_, &end)) = occupied
        .range(..=base)
        .next_back()
        .filter(|(_, end)| **end >= base)
    {
        if let Some(next) = end.checked_add(1) {
            candidate = next;
        } else {
            charge()?;
            candidate = match occupied.get(&0) {
                Some(end) => end
                    .checked_add(1)
                    .ok_or_else(|| invariant("public path space is full"))?,
                None => 0,
            };
        }
    }
    if u64::from(candidate.wrapping_sub(base)) >= u64::try_from(key_count).unwrap_or(u64::MAX) {
        return Err(invariant("public path exceeds native bounded probe window"));
    }
    charge()?;
    let left = occupied
        .range(..candidate)
        .next_back()
        .map(|(&start, &end)| (start, end))
        .filter(|(_, end)| end.checked_add(1) == Some(candidate));
    charge()?;
    let right = occupied
        .range(candidate..)
        .next()
        .map(|(&start, &end)| (start, end))
        .filter(|(start, _)| candidate.checked_add(1) == Some(*start));
    let start = left.map_or(candidate, |(start, _)| start);
    let end = right.map_or(candidate, |(_, end)| end);
    if let Some((start, _)) = left {
        occupied.remove(&start);
    }
    if let Some((start, _)) = right {
        occupied.remove(&start);
    }
    occupied.insert(start, end);
    Ok(candidate)
}

fn balance_key(asset: &AssetDefinitionId, account: &AccountId) -> Vec<u8> {
    format!("asset/{asset}/{account}").into_bytes()
}
fn decode_balance(bytes: &[u8]) -> Result<u64> {
    Ok(u64::from_le_bytes(bytes.try_into().map_err(|_| {
        Error::InvalidAssetValueLength {
            length: bytes.len(),
        }
    })?))
}
fn require_marked(bytes: &[u8; 32]) -> Result<()> {
    if bytes[31] & 1 == 0 {
        Err(invariant(
            "public transfer root has a noncanonical Iroha marker",
        ))
    } else {
        Ok(())
    }
}
fn invariant(details: &str) -> Error {
    Error::TransferInvariant {
        details: details.into(),
    }
}
fn checked_u32(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| invariant("public transfer count exceeds u32"))
}
fn checked_add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| invariant("public transfer resource count overflows"))
}
fn check_limit(limit: &'static str, actual: usize, max: usize) -> Result<()> {
    if actual > max {
        Err(Error::VerifierLimitExceeded { limit, actual, max })
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct DeltaView<'a> {
    from: &'a AccountId,
    to: &'a AccountId,
    asset: &'a AssetDefinitionId,
    quantities: [&'a Quantity; 5],
}
impl<'a> From<&'a TransferDeltaTranscript> for DeltaView<'a> {
    fn from(delta: &'a TransferDeltaTranscript) -> Self {
        Self {
            from: &delta.from_account,
            to: &delta.to_account,
            asset: &delta.asset_definition,
            quantities: [
                &delta.amount,
                &delta.from_balance_before,
                &delta.from_balance_after,
                &delta.to_balance_before,
                &delta.to_balance_after,
            ],
        }
    }
}
impl<'a> From<&'a PublicTransferDelta> for DeltaView<'a> {
    fn from(delta: &'a PublicTransferDelta) -> Self {
        Self {
            from: &delta.from_account,
            to: &delta.to_account,
            asset: &delta.asset_definition,
            quantities: [
                &delta.amount,
                &delta.from_balance_before,
                &delta.from_balance_after,
                &delta.to_balance_before,
                &delta.to_balance_after,
            ],
        }
    }
}
fn measure_delta(budget: &mut ByteBudget, delta: DeltaView<'_>) -> Result<()> {
    budget.encoded(delta.from)?;
    budget.encoded(delta.to)?;
    budget.encoded(delta.asset)?;
    for quantity in delta.quantities {
        budget.encoded(quantity)?;
    }
    Ok(())
}
fn measure_header(
    budget: &mut ByteBudget,
    batch: &Hash,
    authority: &Hash,
    digest: Option<Hash>,
    deltas: usize,
) -> Result<()> {
    budget.encoded(batch)?;
    budget.encoded(authority)?;
    budget.encoded(&digest)?;
    budget.encoded(&checked_u32(deltas)?)
}
struct ByteBudget {
    used: usize,
    max: usize,
}
impl ByteBudget {
    fn new(max: usize) -> Self {
        Self { used: 0, max }
    }
    fn encoded(&mut self, value: &impl NoritoEncode) -> Result<()> {
        let mut counter = ByteCounter(Some(0));
        value.encode_to(&mut counter);
        self.used = checked_add(
            self.used,
            counter
                .0
                .ok_or_else(|| invariant("public encoded byte length overflows"))?,
        )?;
        check_limit("max_public_transfer_bytes", self.used, self.max)
    }
}
struct ByteCounter(Option<usize>);
impl Write for ByteCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self.0.and_then(|count| count.checked_add(bytes.len()));
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TransitionBatch, gadgets::transfer, gadgets::transfer_row_binding};
    use iroha_data_model::{DomainId, fastpq::TransferSmtWitness};
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::{ALICE_ID, BOB_ID};

    fn draft_delta(amount: u64, sender: u64, receiver: u64, same: bool) -> TransferDeltaTranscript {
        TransferDeltaTranscript {
            from_account: (*ALICE_ID).clone(),
            to_account: if same {
                (*ALICE_ID).clone()
            } else {
                (*BOB_ID).clone()
            },
            asset_definition: AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            ),
            amount: Quantity::from(amount),
            from_balance_before: Quantity::from(sender),
            from_balance_after: Quantity::from(sender - amount),
            to_balance_before: Quantity::from(receiver),
            to_balance_after: Quantity::from(receiver + amount),
            from_smt_witness: TransferSmtWitness::default(),
            to_smt_witness: TransferSmtWitness::default(),
        }
    }

    fn fixture(
        groups: Vec<Vec<TransferDeltaTranscript>>,
    ) -> (Vec<TransferTranscript>, Vec<StateTransition>, PublicInputs) {
        let mut transcripts: Vec<_> = groups
            .into_iter()
            .map(|deltas| TransferTranscript {
                batch_hash: Hash::new(b"same transaction hash, distinct public occurrences"),
                authority_digest: Hash::new(b"public authority digest"),
                deltas,
                poseidon_preimage_digest: None,
            })
            .collect();
        let (old_root, new_root) =
            transfer::attach_transfer_smt_witnesses(&mut transcripts).unwrap();
        for transcript in &mut transcripts {
            if let [delta] = transcript.deltas.as_slice() {
                transcript.poseidon_preimage_digest = Some(transfer::compute_poseidon_digest(
                    delta,
                    &transcript.batch_hash,
                ));
            }
        }
        let inputs = PublicInputs {
            old_root,
            new_root,
            slot: u64::MAX,
            dsid: [0xAB; 16],
            perm_root: Hash::new(b"permissions").into(),
            tx_set_hash: Hash::new(b"transaction set").into(),
        };
        let witnesses =
            transfer::transcripts_to_witnesses(&transcripts, &old_root, &new_root).unwrap();
        let mut rows = Vec::new();
        for witness in &witnesses {
            for delta in &witness.deltas {
                for (account, before, after) in [
                    (
                        &delta.from_account,
                        delta.from_balance_before,
                        delta.from_balance_after,
                    ),
                    (
                        &delta.to_account,
                        delta.to_balance_before,
                        delta.to_balance_after,
                    ),
                ] {
                    rows.push(StateTransition::new(
                        balance_key(&delta.asset_definition, account),
                        before.to_le_bytes().to_vec(),
                        after.to_le_bytes().to_vec(),
                        OperationKind::Transfer,
                    ));
                }
            }
        }
        rows.sort_by(|left, right| {
            (&left.key, left.operation_rank()).cmp(&(&right.key, right.operation_rank()))
        });
        (transcripts, rows, inputs)
    }

    fn prepare<'a>(
        rows: &'a [StateTransition],
        claims: &'a [PublicTransferTranscript],
        inputs: PublicInputs,
    ) -> PreparedPublicTransfers<'a> {
        prepare_public_transfers(
            rows,
            claims,
            inputs,
            ProofSemantics::TransferStateTransition,
            PublicTransferLimits::default(),
        )
        .unwrap()
    }

    fn assert_native_equivalence(
        transcripts: &[TransferTranscript],
        rows: &[StateTransition],
        inputs: PublicInputs,
    ) {
        transfer::verify_transcripts(rows, transcripts).unwrap();
        let native =
            transfer::transcripts_to_witnesses(transcripts, &inputs.old_root, &inputs.new_root)
                .unwrap();
        let native_bindings = transfer_row_binding::bind_canonical_rows(rows, &native).unwrap();
        let claims =
            public_claims_from_transcripts(transcripts, PublicTransferLimits::default()).unwrap();
        let prepared = prepare(rows, &claims, inputs);
        assert_eq!(prepared.public_inputs(), &inputs);
        assert_eq!(prepared.claims(), claims);
        assert_eq!(prepared.transitions(), rows);
        assert_eq!(
            prepared.semantics(),
            ProofSemantics::TransferStateTransition
        );
        let scales = iroha_data_model::fastpq::transfer_asset_scales(transcripts);
        assert_eq!(asset_scales(&claims), scales);
        let mut native_batch = TransitionBatch::new("fastpq-state-transition-stark-v1", inputs);
        native_batch.transitions = rows.to_vec();
        assert_eq!(
            prepared.ordering_hash(),
            crate::ordering::ordering_hash(&native_batch).unwrap()
        );
        for (index, (public, native)) in prepared
            .rows()
            .iter()
            .zip(native_bindings.iter())
            .enumerate()
        {
            let native = native.as_ref().unwrap();
            assert_eq!(public.occurrence, native.occurrence());
            assert_eq!(public.role, native.role());
            assert_eq!(public.amount, native.delta().amount);
            assert_eq!(public.asset_scale, scales[&native.delta().asset_definition]);
            let key = &prepared.keys()[public.key_index];
            assert_eq!(key.key, rows[index].key);
            assert_eq!(
                key.path.to_le_bytes().as_slice(),
                native.proof().path_bits.as_slice()
            );
            assert_eq!(public.update.path, key.path);
            assert_eq!(
                public.update.old_leaf,
                digest_limbs(transfer::leaf_hash(&key.key, public.before).into())
            );
            assert_eq!(
                public.update.new_leaf,
                digest_limbs(transfer::leaf_hash(&key.key, public.after).into())
            );
            assert_eq!(
                key.key_hash,
                <[u8; 32]>::from(Hash::new_from_chunks(&[KEY_DOMAIN, &key.key]))
            );
        }
        let pair_count: usize = transcripts
            .iter()
            .map(|transcript| transcript.deltas.len())
            .sum();
        assert_eq!(prepared.pairs().len(), pair_count);
        assert_eq!(prepared.work().key_hashes, prepared.keys().len());
        assert_eq!(prepared.work().value_hashes, 4 * pair_count);
        assert_eq!(prepared.work().leaf_hashes, 4 * pair_count);
        assert!(prepared.work().allocation_steps <= 4 * prepared.keys().len());
        for pair in prepared.pairs() {
            for leg in 0..2 {
                let row = prepared.rows()[pair.row_indices[leg]];
                assert_eq!(row.occurrence, pair.occurrence);
                assert_eq!(row.role.is_debit(), u64::from(leg == 0));
                assert_eq!(row.update, pair.updates[leg]);
            }
        }
    }

    #[test]
    fn public_preparation_matches_native_multiple_zero_self_and_wide_transfers() {
        for groups in [
            vec![
                vec![draft_delta(7, 90, 2, false)],
                vec![draft_delta(3, 83, 9, false)],
            ],
            vec![vec![
                draft_delta(0, 20, 20, true),
                draft_delta(7, 20, 13, true),
                draft_delta(0, 20, 20, true),
            ]],
            vec![vec![draft_delta(u64::MAX, u64::MAX, 0, false)]],
            vec![vec![draft_delta(1, u64::MAX, u64::from(u32::MAX), false)]],
        ] {
            let (transcripts, mut rows, inputs) = fixture(groups);
            if rows.iter().all(|row| row.key == rows[0].key) {
                rows.reverse();
            }
            assert_native_equivalence(&transcripts, &rows, inputs);
        }
    }

    #[test]
    fn public_scales_match_native_fractional_and_first_use_rules() {
        let mut first = draft_delta(0, 100, 0, false);
        first.amount = Quantity::try_from_numeric(Numeric::new(125, 2)).unwrap();
        first.from_balance_after = Quantity::try_from_numeric(Numeric::new(9875, 2)).unwrap();
        first.to_balance_after = first.amount.clone();
        let mut second = draft_delta(0, 100, 0, false);
        second.amount = Quantity::try_from_numeric(Numeric::new(25, 3)).unwrap();
        second.from_balance_after = Quantity::try_from_numeric(Numeric::new(99975, 3)).unwrap();
        second.to_balance_after = second.amount.clone();
        let (transcripts, rows, inputs) = fixture(vec![vec![first], vec![second]]);
        assert_native_equivalence(&transcripts, &rows, inputs);
        let mut claims =
            public_claims_from_transcripts(&transcripts, PublicTransferLimits::default()).unwrap();
        assert_eq!(
            asset_scales(&claims).values().copied().collect::<Vec<_>>(),
            vec![3]
        );
        // A later balance's extra precision must not change the original scale.
        claims[1].deltas[0].from_balance_before =
            Quantity::try_from_numeric(Numeric::new(1, 9)).unwrap();
        assert_eq!(
            asset_scales(&claims).values().copied().collect::<Vec<_>>(),
            vec![3]
        );
        assert!(matches!(
            normalized_values(&claims[1].deltas[0], 3),
            Err(Error::TransferNumericBounds {
                field: "from_balance_before"
            })
        ));
    }

    #[test]
    fn legacy_projection_and_preparation_do_not_read_private_smt_material() {
        let (transcripts, rows, inputs) = fixture(vec![vec![draft_delta(4, 20, 2, false)]]);
        let limits = PublicTransferLimits::default();
        let expected = public_claims_from_transcripts(&transcripts, limits).unwrap();
        let baseline = prepare(&rows, &expected, inputs);
        let mut forged_private = transcripts.clone();
        for delta in &mut forged_private[0].deltas {
            for witness in [&mut delta.from_smt_witness, &mut delta.to_smt_witness] {
                witness.root_before = [0; 32];
                witness.root_after = [0; 32];
                witness.path_bits = vec![0xFF; 1024];
                witness.siblings = vec![[0; 32]; 1024];
            }
        }
        let claims = public_claims_from_transcripts(&forged_private, limits).unwrap();
        assert_eq!(claims, expected);
        let actual = prepare(&rows, &claims, inputs);
        assert_eq!(actual.rows(), baseline.rows());
        assert_eq!(actual.keys(), baseline.keys());
        assert_eq!(actual.work(), baseline.work());
        assert!(
            transfer::transcripts_to_witnesses(&forged_private, &inputs.old_root, &inputs.new_root)
                .is_err(),
            "public preparation must not be mistaken for a proof of the private path"
        );
    }

    #[test]
    fn compact_statements_pin_full_endpoints_and_explicit_shared_intermediates() {
        let (transcripts, rows, inputs) = fixture(vec![vec![
            draft_delta(2, 20, 2, false),
            draft_delta(1, 18, 4, false),
        ]]);
        let claims =
            public_claims_from_transcripts(&transcripts, PublicTransferLimits::default()).unwrap();
        let prepared = prepare(&rows, &claims, inputs);
        let intermediate = transcripts[0].deltas[0].to_smt_witness.root_after;
        let statements = prepared.compact_statements(&[intermediate]).unwrap();
        assert_eq!(statements.len(), 2);
        assert_eq!(statements[0].old_root, digest_limbs(inputs.old_root));
        assert_eq!(statements[0].new_root, digest_limbs(intermediate));
        assert_eq!(statements[1].old_root, statements[0].new_root);
        assert_eq!(statements[1].new_root, digest_limbs(inputs.new_root));
        assert_eq!(statements[0].updates, prepared.pairs()[0].updates);
        assert_eq!(statements[1].updates, prepared.pairs()[1].updates);
        assert!(prepared.compact_statements(&[]).is_err());
        assert!(prepared.compact_statements(&[intermediate; 2]).is_err());
        let mut malformed = intermediate;
        malformed[31] &= !1;
        assert!(prepared.compact_statements(&[malformed]).is_err());
        let mut other_claim = intermediate;
        other_claim[30] ^= 0x80;
        let changed = prepared.compact_statements(&[other_claim]).unwrap();
        assert_ne!(changed, statements);
        assert_eq!(changed[0].new_root, changed[1].old_root);
        assert_eq!(changed[0].old_root, statements[0].old_root);
        assert_eq!(changed[1].new_root, statements[1].new_root);
    }

    #[test]
    fn public_profiles_empty_cases_and_canonical_order_remain_explicit() {
        let limits = PublicTransferLimits::default();
        let empty = prepare_public_transfers(
            &[],
            &[],
            PublicInputs::default(),
            ProofSemantics::TransferStateTransition,
            limits,
        )
        .unwrap();
        assert!(empty.rows().is_empty());
        assert!(empty.compact_statements(&[]).unwrap().is_empty());
        assert!(empty.compact_statements(&[[1; 32]]).is_err());
        for semantics in [
            ProofSemantics::AxtTransferClaim,
            ProofSemantics::AxtOpaqueEffect,
        ] {
            assert!(
                prepare_public_transfers(&[], &[], PublicInputs::default(), semantics, limits)
                    .is_err()
            );
        }
        let changed = PublicInputs {
            new_root: [1; 32],
            ..PublicInputs::default()
        };
        assert!(
            prepare_public_transfers(
                &[],
                &[],
                changed,
                ProofSemantics::TransferStateTransition,
                limits
            )
            .is_err()
        );
        let (transcripts, mut rows, inputs) = fixture(vec![vec![draft_delta(1, 20, 2, false)]]);
        let claims = public_claims_from_transcripts(&transcripts, limits).unwrap();
        assert!(
            prepare_public_transfers(
                &rows,
                &claims,
                inputs,
                ProofSemantics::AxtTransferClaim,
                limits
            )
            .is_ok()
        );
        assert!(
            prepare_public_transfers(
                &rows,
                &claims,
                inputs,
                ProofSemantics::AxtOpaqueEffect,
                limits
            )
            .is_err()
        );
        rows.reverse();
        assert!(
            prepare_public_transfers(
                &rows,
                &claims,
                inputs,
                ProofSemantics::TransferStateTransition,
                limits
            )
            .is_err()
        );
        rows.reverse();
        rows[0].operation = OperationKind::MetaSet;
        assert!(
            prepare_public_transfers(
                &rows,
                &claims,
                inputs,
                ProofSemantics::TransferStateTransition,
                limits
            )
            .is_err()
        );
        rows[0].operation = OperationKind::Transfer;
        for old in [true, false] {
            let mut malformed = inputs;
            if old {
                malformed.old_root[31] &= !1;
            } else {
                malformed.new_root[31] &= !1;
            }
            assert!(
                prepare_public_transfers(
                    &rows,
                    &claims,
                    malformed,
                    ProofSemantics::TransferStateTransition,
                    limits
                )
                .is_err()
            );
        }
    }

    #[test]
    fn public_multiplicity_values_identities_and_digest_policy_reject_mutations() {
        let limits = PublicTransferLimits::default();
        let (transcripts, rows, inputs) = fixture(vec![vec![draft_delta(3, 20, 2, false)]]);
        let claims = public_claims_from_transcripts(&transcripts, limits).unwrap();
        for mutation in 0..9 {
            let mut changed_rows = rows.clone();
            let mut changed = claims.clone();
            match mutation {
                0 => {
                    changed_rows.pop();
                }
                1 => changed_rows.push(rows[1].clone()),
                2 => changed_rows[1] = rows[0].clone(),
                3 => changed_rows[0].pre_value.push(0),
                4 => changed_rows[0].key.push(0),
                5 => changed[0].poseidon_preimage_digest = None,
                6 => changed[0].poseidon_preimage_digest = Some(Hash::new(b"forged")),
                7 => changed[0].deltas[0].from_account = (*BOB_ID).clone(),
                _ => changed[0].deltas[0].from_balance_before = Quantity::from(500_u64),
            }
            assert!(
                prepare_public_transfers(
                    &changed_rows,
                    &changed,
                    inputs,
                    ProofSemantics::TransferStateTransition,
                    limits
                )
                .is_err(),
                "mutation {mutation}"
            );
        }
        let mut multi = claims[0].clone();
        multi.deltas.push(multi.deltas[0].clone());
        assert!(check_digest_policy(&multi).is_err());
        multi.deltas.clear();
        multi.poseidon_preimage_digest = None;
        assert!(check_digest_policy(&multi).is_err());
        let (transcripts, rows, inputs) = fixture(vec![vec![
            draft_delta(3, 20, 2, false),
            draft_delta(2, 17, 5, false),
        ]]);
        let mut changed = public_claims_from_transcripts(&transcripts, limits).unwrap();
        changed[0].deltas[1].from_balance_before = Quantity::from(100_u64);
        changed[0].deltas[1].from_balance_after = Quantity::from(98_u64);
        assert!(
            matches!(prepare_public_transfers(&rows, &changed, inputs, ProofSemantics::TransferStateTransition, limits),
            Err(Error::TransferInvariant { details }) if details.contains("repeated-key"))
        );
    }

    #[test]
    fn normalization_rejects_underflow_overflow_out_of_range_and_unlinked_self_legs() {
        let native = draft_delta(1, 20, 2, false);
        let transcripts = [TransferTranscript {
            batch_hash: Hash::new(b"x"),
            authority_digest: Hash::new(b"y"),
            deltas: vec![native],
            poseidon_preimage_digest: None,
        }];
        let original =
            public_claims_from_transcripts(&transcripts, PublicTransferLimits::default()).unwrap()
                [0]
            .deltas[0]
                .clone();
        for mutation in 0..4 {
            let mut delta = original.clone();
            match mutation {
                0 => delta.amount = Quantity::from(21_u64),
                1 => {
                    delta.to_balance_before = Quantity::from(u64::MAX);
                    delta.to_balance_after = Quantity::from(u64::MAX);
                }
                2 => delta.amount = Quantity::from(u128::MAX),
                _ => delta.to_account = delta.from_account.clone(),
            }
            assert!(normalized_values(&delta, 0).is_err());
        }
    }

    #[test]
    fn public_resource_limits_are_checked_and_exact_byte_boundary_is_stable() {
        let limits = PublicTransferLimits::default();
        let (transcripts, rows, inputs) = fixture(vec![vec![draft_delta(1, 20, 2, false)]]);
        let claims = public_claims_from_transcripts(&transcripts, limits).unwrap();
        let prepared = prepare(&rows, &claims, inputs);
        for mutation in 0..6 {
            let mut restricted = limits;
            match mutation {
                0 => restricted.max_transcripts = 0,
                1 => restricted.max_deltas = 0,
                2 => restricted.max_rows = 1,
                3 => restricted.max_public_bytes = 0,
                4 => restricted.max_unique_keys = 1,
                _ => restricted.max_allocation_steps = 0,
            }
            assert!(matches!(
                prepare_public_transfers(
                    &rows,
                    &claims,
                    inputs,
                    ProofSemantics::TransferStateTransition,
                    restricted
                ),
                Err(Error::VerifierLimitExceeded { .. })
            ));
        }
        let exact = PublicTransferLimits {
            max_public_bytes: prepared.work().public_bytes,
            ..limits
        };
        assert!(
            prepare_public_transfers(
                &rows,
                &claims,
                inputs,
                ProofSemantics::TransferStateTransition,
                exact
            )
            .is_ok()
        );
        assert!(
            prepare_public_transfers(
                &rows,
                &claims,
                inputs,
                ProofSemantics::TransferStateTransition,
                PublicTransferLimits {
                    max_public_bytes: exact.max_public_bytes - 1,
                    ..exact
                }
            )
            .is_err()
        );
        for restricted in [
            PublicTransferLimits {
                max_transcripts: 0,
                ..limits
            },
            PublicTransferLimits {
                max_deltas: 0,
                ..limits
            },
            PublicTransferLimits {
                max_public_bytes: 0,
                ..limits
            },
        ] {
            assert!(public_claims_from_transcripts(&transcripts, restricted).is_err());
        }
        assert!(checked_add(usize::MAX, 1).is_err());
        if let Ok(too_many) = usize::try_from(u64::from(u32::MAX) + 1) {
            assert!(checked_u32(too_many).is_err());
        }
        let mut counter = ByteCounter(Some(usize::MAX));
        counter.write_all(&[1]).unwrap();
        assert_eq!(counter.0, None);
        counter.flush().unwrap();
        let mut oversized = rows.clone();
        oversized[0].key = vec![0; limits.max_public_bytes + 1];
        assert!(matches!(
            prepare_public_transfers(
                &oversized,
                &claims,
                inputs,
                ProofSemantics::TransferStateTransition,
                limits
            ),
            Err(Error::VerifierLimitExceeded {
                limit: "max_public_transfer_bytes",
                ..
            })
        ));
    }

    #[test]
    fn interval_allocation_matches_native_first_free_collision_and_wrap_rule() {
        let bases = [0_u32, 1, 2, u32::MAX - 1, u32::MAX];
        for encoded in 0..625_usize {
            let mut digits = encoded;
            let mut occupied = BTreeMap::new();
            let mut reference = BTreeSet::new();
            let mut steps = 0;
            for _ in 0..4 {
                let base = bases[digits % bases.len()];
                digits /= bases.len();
                let expected = (0..4_u32)
                    .map(|offset| base.wrapping_add(offset))
                    .find(|candidate| reference.insert(*candidate))
                    .unwrap();
                assert_eq!(
                    allocate_path(&mut occupied, base, 4, &mut steps, 16).unwrap(),
                    expected
                );
            }
            assert!(steps <= 16);
        }
        let keys = [
            b"collision-probe/74003".as_slice(),
            b"collision-probe/7796".as_slice(),
        ];
        let hashes: Vec<[u8; 32]> = keys
            .iter()
            .map(|key| Hash::new_from_chunks(&[KEY_DOMAIN, key]).into())
            .collect();
        assert_eq!(&hashes[0][..4], &hashes[1][..4]);
        let (sender, receiver) =
            transfer::build_transfer_smt_witness_pair(keys[0], 2, 1, keys[1], 0, 1).unwrap();
        let mut occupied = BTreeMap::new();
        let mut steps = 0;
        for (hash, proof) in hashes.iter().zip([sender, receiver]) {
            let base = u32::from_le_bytes(hash[..4].try_into().unwrap());
            let path = allocate_path(&mut occupied, base, 2, &mut steps, 8).unwrap();
            assert_eq!(path.to_le_bytes().as_slice(), proof.path_bits.as_slice());
        }
        let mut occupied = BTreeMap::new();
        let mut steps = 0;
        assert!(allocate_path(&mut occupied, 0, 1, &mut steps, 0).is_err());
        assert!(occupied.is_empty());
        let mut occupied = BTreeMap::from([(0, u32::MAX)]);
        let mut steps = 0;
        assert!(allocate_path(&mut occupied, 0, 4, &mut steps, 4).is_err());
        let mut occupied = BTreeMap::from([(7, 7)]);
        let mut steps = 0;
        assert!(allocate_path(&mut occupied, 7, 1, &mut steps, 4).is_err());
    }

    #[test]
    fn public_leaf_hashes_preserve_full_values_long_keys_and_all_digest_limbs() {
        for length in [0, 110, 111, 128, 255] {
            let key: Vec<_> = (0..length).map(|index| (index % 251) as u8).collect();
            let key_hash: [u8; 32] = Hash::new_from_chunks(&[KEY_DOMAIN, &key]).into();
            for value in [0, 1, 0xffff_ffff_0000_0001, u64::MAX] {
                let hash = public_leaf(&key_hash, value);
                assert_eq!(hash, <[u8; 32]>::from(transfer::leaf_hash(&key, value)));
                assert_eq!(
                    digest_limbs(hash)
                        .into_iter()
                        .flat_map(u32::to_le_bytes)
                        .collect::<Vec<_>>(),
                    hash
                );
                assert_eq!(hash[31] & 1, 1);
            }
        }
    }

    #[test]
    fn public_preparation_is_independent_of_every_supported_ambient_codec_layout() {
        let (transcripts, rows, inputs) = fixture(vec![vec![draft_delta(1, 20, 2, false)]]);
        let limits = PublicTransferLimits::default();
        let claims = public_claims_from_transcripts(&transcripts, limits).unwrap();
        let expected = prepare(&rows, &claims, inputs);
        for flags in
            (u8::MIN..=u8::MAX).filter(|&flags| norito::core::validate_header_flags(flags).is_ok())
        {
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            let projected = public_claims_from_transcripts(&transcripts, limits).unwrap();
            assert_eq!(projected, claims);
            let actual = prepare(&rows, &projected, inputs);
            assert_eq!(actual.work(), expected.work());
            assert_eq!(actual.rows(), expected.rows());
            assert_eq!(actual.ordering_hash(), expected.ordering_hash());
            assert_eq!(norito::core::get_decode_flags(), flags);
        }
    }
}
