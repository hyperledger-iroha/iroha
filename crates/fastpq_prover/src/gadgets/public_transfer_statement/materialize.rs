//! Bounded private SMT materialization from validated public two-update facts.
//!
//! Initial leaves come from each key's first chronological occurrence. Every
//! subsequent update must match the current leaf exactly. Public arithmetic,
//! full keys and path allocation belong to the shared public preparation engine.
//! Derived roots describe the touched-state tree and establish no finality.

use std::fmt;

use iroha_allocation::{AllocationBudget, AllocationReservation};

mod funded_tree;

use iroha_crypto::Hash;
use iroha_data_model::fastpq::{FastpqQuantityUnits, TransferSmtWitness};

use super::{
    ByteBudget, DeltaView, PreparedPublicTransfers, PublicKeyAllocation, PublicTransferLimits,
    PublicTransferTranscript, asset_scales, balance_key, check_limit, checked_add, checked_u32,
    encode_quantity_units_v1, invariant, measure_delta, measure_header, normalized_values_for,
    prepare_quantity_public_transfers,
};
use crate::{
    OperationKind, ProofSemantics, PublicInputs, Result, StateTransition,
    gadgets::compact_smt_air::PublicUpdate,
};

const HEIGHT: usize = 32;
const NODE_DOMAIN: &[u8] = b"fastpq:v1:smt:node|";
const PAD_DOMAIN: &[u8] = b"fastpq:v1:smt:pad|";

/// One checked canonical row projected by a strict internal public preparation.
/// It carries no authority and is never accepted directly from external callers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::gadgets) struct CheckedUpdateRow {
    /// Index into the complete collision-resolved key table.
    pub(in crate::gadgets) key_index: usize,
    /// Group, effect-within-group and global pair ordinals, respectively.
    pub(in crate::gadgets) occurrence: [u32; 3],
    /// Sequential update position within the pair, exactly zero or one.
    pub(in crate::gadgets) leg: usize,
    /// Complete leaf and collision-resolved path binding.
    pub(in crate::gadgets) update: PublicUpdate,
}

/// Exact chronological ports for one pair of sequential state updates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::gadgets) struct CheckedUpdatePair {
    /// Group, effect-within-group and global pair ordinals, respectively.
    pub(in crate::gadgets) occurrence: [u32; 3],
    /// Canonical row ports in chronological update order.
    pub(in crate::gadgets) row_indices: [usize; 2],
    /// Full public update ports in the same order.
    pub(in crate::gadgets) updates: [PublicUpdate; 2],
}

/// Immutable, allocation-free view of a strictly prepared internal update table.
///
/// Implementations must originate from bounded public preparation: exact typed
/// semantics, canonical key/value hashes, deterministic collision allocation,
/// quantity arithmetic and public occurrence coverage remain that owner's work.
/// Materialization independently rechecks every port and chronological leaf,
/// but does not authenticate the public claims or their expected root authority.
/// No public artifact API accepts this trait or caller-constructed update ports.
/// Every method must expose the same immutable table for the entire invocation.
pub(in crate::gadgets) trait CheckedUpdateTable {
    /// Public context; only empty construction keeps the supplied root pair.
    fn public_inputs(&self) -> PublicInputs;
    /// Complete sorted, collision-resolved key table from strict preparation.
    fn keys(&self) -> &[PublicKeyAllocation];
    /// Exact canonical row count.
    fn row_count(&self) -> usize;
    /// Exact chronological two-update pair count.
    fn pair_count(&self) -> usize;
    /// Borrow one canonical row's checked ports without allocating a projection.
    fn row(&self, index: usize) -> Option<CheckedUpdateRow>;
    /// Borrow one chronological pair's checked ports without allocating a projection.
    fn pair(&self, index: usize) -> Option<CheckedUpdatePair>;
}

impl<V> CheckedUpdateTable for PreparedPublicTransfers<'_, V> {
    fn public_inputs(&self) -> PublicInputs {
        self.public_inputs
    }

    fn keys(&self) -> &[PublicKeyAllocation] {
        &self.keys
    }

    fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn pair_count(&self) -> usize {
        self.pairs.len()
    }

    fn row(&self, index: usize) -> Option<CheckedUpdateRow> {
        self.rows.get(index).map(|row| CheckedUpdateRow {
            key_index: row.key_index,
            occurrence: [
                row.occurrence.transcript_ordinal,
                row.occurrence.delta_ordinal,
                row.occurrence.pair_ordinal,
            ],
            // The unchanged pair-port check requires debit first and credit second.
            leg: usize::from(row.role.is_debit() == 0),
            update: row.update,
        })
    }

    fn pair(&self, index: usize) -> Option<CheckedUpdatePair> {
        self.pairs.get(index).map(|pair| CheckedUpdatePair {
            occurrence: [
                pair.occurrence.transcript_ordinal,
                pair.occurrence.delta_ordinal,
                pair.occurrence.pair_ordinal,
            ],
            row_indices: pair.row_indices,
            updates: pair.updates,
        })
    }
}

/// Explicit limits for private touched-state tree and path construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(
    clippy::struct_field_names,
    reason = "every field is an inclusive cap and `max_` separates it from the measured count; \
              the fields are public API shared with `iroha_core`"
)]
pub struct TransferSmtBuildLimits {
    /// Maximum chronological participant updates, including zero/self updates.
    pub max_updates: usize,
    /// Maximum distinct full state keys.
    pub max_unique_keys: usize,
    /// Maximum retained occupied nodes across all 33 tree levels.
    pub max_retained_nodes: usize,
    /// Maximum output sibling hashes, exactly 32 per participant update.
    pub max_sibling_hashes: usize,
    /// Maximum internal-node hashes for initial construction and updates.
    /// Fixed padding setup adds at most 33 hashes independently of input size.
    pub max_node_hashes: usize,
}

impl TransferSmtBuildLimits {
    /// Derive conservative tree bounds from one explicit participant-update cap.
    /// Returns `None` if any bound would overflow the host's address space.
    #[must_use]
    pub fn for_update_limit(max_updates: usize) -> Option<Self> {
        Some(Self {
            max_updates,
            max_unique_keys: max_updates,
            max_retained_nodes: max_updates.checked_mul(HEIGHT + 1)?,
            max_sibling_hashes: max_updates.checked_mul(HEIGHT)?,
            max_node_hashes: max_updates.checked_mul(2 * HEIGHT)?,
        })
    }

    /// Checked conservative bytes for all tree scratch and returned path backing.
    ///
    /// The caller admits this demand from its original finite pool as part of the
    /// complete operation. Construction partitions these prepaid bytes without
    /// acquiring another pool. Unused scratch credit refunds on return; output
    /// vector, path, sibling and ledger credits remain with their physical owners.
    /// Public preparation, error diagnostics and enclosing owners are separate.
    ///
    /// # Errors
    /// Rejects count/sibling caps, inconsistent empty or odd update counts and
    /// unrepresentable concrete layouts or layout sums before allocating.
    pub fn allocation_bytes(self, updates: usize, unique_keys: usize) -> Result<usize> {
        funded_tree::allocation_bytes(self, updates, unique_keys)
    }
}

/// Exact bounded private-tree work, excluding already checked public leaf hashes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransferSmtBuildWork {
    /// Chronological participant update count.
    pub updates: usize,
    /// Number of distinct occupied leaves.
    pub unique_keys: usize,
    /// Occupied nodes retained across all levels.
    pub retained_nodes: usize,
    /// Output sibling hash count.
    pub sibling_hashes: usize,
    /// Internal-node hashes used to seed and update the tree.
    pub node_hashes: usize,
}

/// Locally generated private paths in original pair and sequential update order.
/// For transfers, the first update is the debit and the second is the credit.
/// The move-only owner retains original allocation credit; borrowed witnesses
/// cannot detach its backing or grow its vectors. These data establish no source
/// authority or proof-verification result.
pub struct DerivedTransferSmtWitnesses {
    roots: ([u8; 32], [u8; 32]),
    pairs: funded_tree::WitnessPairs,
    work: TransferSmtBuildWork,
}

impl fmt::Debug for DerivedTransferSmtWitnesses {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DerivedTransferSmtWitnesses")
            .field("roots", &self.roots)
            .field("pairs", &self.pairs())
            .field("work", &self.work)
            .finish()
    }
}

impl PartialEq for DerivedTransferSmtWitnesses {
    fn eq(&self, other: &Self) -> bool {
        self.roots == other.roots && self.pairs() == other.pairs() && self.work == other.work
    }
}

impl Eq for DerivedTransferSmtWitnesses {}

impl DerivedTransferSmtWitnesses {
    /// Exact initial and final roots of the constructed touched-state tree.
    #[must_use]
    pub const fn roots(&self) -> ([u8; 32], [u8; 32]) {
        self.roots
    }

    /// Roots after each complete two-update pair except the final pair.
    ///
    /// These are the chronological boundaries between segments of this already
    /// materialized complete batch. The iterator borrows the existing witnesses;
    /// it builds no tree, prepares no individual delta and allocates no storage.
    /// Empty and single-pair batches have no intermediate roots. Equal roots
    /// from distinct occurrences remain distinct iterator entries.
    ///
    /// These local touched-tree roots do not authenticate source finality.
    #[must_use]
    pub fn intermediate_roots(
        &self,
    ) -> impl ExactSizeIterator<Item = [u8; 32]> + DoubleEndedIterator + '_ {
        self.pairs()[..self.pairs().len().saturating_sub(1)]
            .iter()
            .map(|pair| pair[1].root_after)
    }

    /// Private update paths, preserving every chronological occurrence.
    #[must_use]
    pub fn pairs(&self) -> &[[TransferSmtWitness; 2]] {
        self.pairs.as_slice()
    }

    /// Exact node/sibling work retained with this complete successful result.
    #[must_use]
    pub const fn work(&self) -> TransferSmtBuildWork {
        self.work
    }
}

/// Full-domain producer output; original claims remain owned by the caller.
#[derive(Debug)]
pub struct QuantityTransferMaterialization {
    transitions: Vec<StateTransition>,
    public_inputs: PublicInputs,
    ordering_hash: Hash,
    witnesses: DerivedTransferSmtWitnesses,
}

impl QuantityTransferMaterialization {
    /// Exact canonical rows generated from the original public quantities.
    #[must_use]
    pub fn transitions(&self) -> &[StateTransition] {
        &self.transitions
    }

    /// Caller inputs with old/new roots replaced by the derived touched-tree roots.
    #[must_use]
    pub const fn public_inputs(&self) -> PublicInputs {
        self.public_inputs
    }

    /// Ordering commitment of the complete canonical row vector.
    #[must_use]
    pub const fn ordering_hash(&self) -> Hash {
        self.ordering_hash
    }

    /// Exact private paths and bounded construction work.
    #[must_use]
    pub const fn witnesses(&self) -> &DerivedTransferSmtWitnesses {
        &self.witnesses
    }

    /// Consume the result without copying its rows or private paths.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        Vec<StateTransition>,
        PublicInputs,
        Hash,
        DerivedTransferSmtWitnesses,
    ) {
        (
            self.transitions,
            self.public_inputs,
            self.ordering_hash,
            self.witnesses,
        )
    }
}

impl<V> PreparedPublicTransfers<'_, V> {
    /// Build private paths and require both derived roots to equal this table's inputs.
    /// No transcript quantities, identities or public ports are repaired.
    /// The supplied reservation must come from `budget` and cover
    /// [`TransferSmtBuildLimits::allocation_bytes`] for this table.
    ///
    /// # Errors
    /// Rejects construction limits, inconsistent internal ports or mismatching roots.
    pub fn build_smt_witnesses(
        &self,
        limits: TransferSmtBuildLimits,
        budget: &AllocationBudget,
        reservation: &mut AllocationReservation,
    ) -> Result<DerivedTransferSmtWitnesses> {
        let built = derive(self, limits, budget, reservation)?;
        if built.roots != (self.public_inputs.old_root, self.public_inputs.new_root) {
            return Err(invariant(
                "derived transfer SMT roots differ from public inputs",
            ));
        }
        Ok(built)
    }
}

/// Reconstruct bounded canonical quantity rows before public preparation or private trees.
///
/// This shares the materializer's exact scale selection, arithmetic, full key rendering,
/// quantity encoding, occurrence multiplicity and stable ordering. It performs no SMT
/// construction. Call [`prepare_quantity_public_transfers`] afterward to validate digest
/// policy, chronology, complete row coverage and public key/leaf bindings; these rows
/// alone do not establish those properties or authenticate any source.
///
/// `max_updates` preserves the caller's pre-allocation participant-update ceiling.
///
/// # Errors
/// Rejects public count/byte/update limits, invalid normalization or arithmetic,
/// failed account rendering and quantity encoding errors before returning any rows.
pub fn quantity_rows_for_public_preparation(
    claims: &[PublicTransferTranscript],
    public_inputs: PublicInputs,
    public_limits: PublicTransferLimits,
    max_updates: usize,
) -> Result<Vec<StateTransition>> {
    check_limit(
        "max_public_transfer_transcripts",
        claims.len(),
        public_limits.max_transcripts,
    )?;
    let mut budget = ByteBudget::new(public_limits.max_public_bytes);
    budget.encoded(&public_inputs)?;
    budget.encoded(&checked_u32(claims.len())?)?;
    let mut deltas = 0_usize;
    for claim in claims {
        deltas = checked_add(deltas, claim.deltas.len())?;
        check_limit(
            "max_public_transfer_deltas",
            deltas,
            public_limits.max_deltas,
        )?;
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
    let rows = deltas
        .checked_mul(2)
        .ok_or_else(|| invariant("quantity row count overflows"))?;
    checked_u32(rows)?;
    check_limit("max_public_transfer_rows", rows, public_limits.max_rows)?;
    check_limit("max_transfer_smt_updates", rows, max_updates)?;
    let scales = asset_scales(claims);
    let mut transitions = Vec::with_capacity(rows);
    let mut raw_bytes = 0_usize;
    for claim in claims {
        for delta in &claim.deltas {
            let values = normalized_values_for::<FastpqQuantityUnits>(
                delta,
                scales[&delta.asset_definition],
            )?;
            for (account, before, after) in [
                (&delta.from_account, values[1], values[2]),
                (&delta.to_account, values[3], values[4]),
            ] {
                let row = StateTransition::new(
                    balance_key(&delta.asset_definition, account)?,
                    encode_quantity_units_v1(&before)?,
                    encode_quantity_units_v1(&after)?,
                    OperationKind::Transfer,
                );
                for bytes in [&row.key, &row.pre_value, &row.post_value] {
                    raw_bytes = checked_add(raw_bytes, bytes.len())?;
                }
                check_limit(
                    "max_public_transfer_bytes",
                    raw_bytes,
                    public_limits.max_public_bytes,
                )?;
                transitions.push(row);
            }
        }
    }
    transitions.sort_by(|a, b| (&a.key, a.operation_rank()).cmp(&(&b.key, b.operation_rank())));
    Ok(transitions)
}

/// Materialize exact full-domain rows and private paths from original public claims.
///
/// Counts and public bytes are bounded before row construction. The public engine
/// validates arithmetic, digests, occurrence coverage and common scales. Nonempty
/// construction temporarily uses the defined empty-tree root while deriving actual
/// roots; that scratch context is private and never returned. Actual derived roots
/// are re-prepared under the same public limits before success. Empty input keeps
/// the caller's unchanged roots and remains subject to the ordinary/AXT profile rules.
///
/// This produces local facts, not authenticated state, an admitted profile or a proof.
/// `budget` and `reservation` fund the private tree only; canonical public row and
/// preparation storage remains a separate caller admission obligation.
///
/// # Errors
/// Rejects malformed claims, arithmetic/chronology failures, unsupported semantics,
/// exceeded public/private bounds and inconsistent leaf updates. Inputs are immutable.
pub fn materialize_quantity_public_transfers(
    claims: &[PublicTransferTranscript],
    mut public_inputs: PublicInputs,
    semantics: ProofSemantics,
    public_limits: PublicTransferLimits,
    tree_limits: TransferSmtBuildLimits,
    budget: &AllocationBudget,
    reservation: &mut AllocationReservation,
) -> Result<QuantityTransferMaterialization> {
    let transitions = quantity_rows_for_public_preparation(
        claims,
        public_inputs,
        public_limits,
        tree_limits.max_updates,
    )?;
    let mut scratch = public_inputs;
    if !transitions.is_empty() {
        let empty: [u8; 32] = padding(HEIGHT).into();
        scratch.old_root = empty;
        scratch.new_root = empty;
    }
    let prepared =
        prepare_quantity_public_transfers(&transitions, claims, scratch, semantics, public_limits)?;
    let witnesses = derive(&prepared, tree_limits, budget, reservation)?;
    (public_inputs.old_root, public_inputs.new_root) = witnesses.roots;
    let bound = prepare_quantity_public_transfers(
        &transitions,
        claims,
        public_inputs,
        semantics,
        public_limits,
    )?;
    let ordering_hash = bound.ordering_hash();
    Ok(QuantityTransferMaterialization {
        transitions,
        public_inputs,
        ordering_hash,
        witnesses,
    })
}

fn padding(level: usize) -> Hash {
    Hash::new_from_chunks(&[PAD_DOMAIN, &(level as u64).to_le_bytes()])
}

fn digest(limbs: [u32; 8]) -> Result<Hash> {
    let bytes: [u8; 32] = core::array::from_fn(|i| limbs[i / 4].to_le_bytes()[i % 4]);
    super::require_marked(&bytes)?;
    Ok(Hash::prehashed(bytes))
}

fn derive<V>(
    prepared: &PreparedPublicTransfers<'_, V>,
    limits: TransferSmtBuildLimits,
    budget: &AllocationBudget,
    reservation: &mut AllocationReservation,
) -> Result<DerivedTransferSmtWitnesses> {
    derive_two_update_smt(prepared, limits, budget, reservation)
}

/// Materialize one immutable, strictly prepared sequence of two-update effects.
///
/// This is the sole private-tree implementation for transfer and execution-effect
/// preparation. It rechecks cardinality, exact occurrence/leg/key ports, unique
/// paths and chronological leaves under the existing bounded tree-work rules.
/// The caller supplies the original pool and prepaid allocation demand. Returned
/// witnesses retain all their backing credit until actual deallocation.
/// Nonempty roots are derived locally; callers binding expected public roots must
/// compare them afterward. Empty construction requires an unchanged supplied root.
/// Neither result roots nor successful preparation authenticate source finality.
pub(in crate::gadgets) fn derive_two_update_smt<T: CheckedUpdateTable + ?Sized>(
    prepared: &T,
    limits: TransferSmtBuildLimits,
    budget: &AllocationBudget,
    reservation: &mut AllocationReservation,
) -> Result<DerivedTransferSmtWitnesses> {
    funded_tree::derive(prepared, limits, budget, reservation)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod checked_tests;

#[cfg(test)]
mod test_funding;
