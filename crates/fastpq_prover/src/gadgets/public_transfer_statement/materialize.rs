//! Bounded private SMT materialization from validated public transfer facts.
//!
//! Initial leaves come from each key's first chronological occurrence. Every
//! subsequent update must match the current leaf exactly. Public arithmetic,
//! full keys and path allocation belong to the shared public preparation engine.
//! Derived roots describe this touched-balance tree, not authenticated finality.

use std::collections::BTreeMap;

use iroha_crypto::Hash;
use iroha_data_model::fastpq::{FastpqQuantityUnits, TransferSmtWitness};

use super::{
    ByteBudget, DeltaView, PreparedPublicTransfers, PublicTransferLimits, PublicTransferTranscript,
    asset_scales, balance_key, check_limit, checked_add, checked_u32, encode_quantity_units_v1,
    invariant, measure_delta, measure_header, normalized_values_for,
    prepare_quantity_public_transfers,
};
use crate::{OperationKind, ProofSemantics, PublicInputs, Result, StateTransition};

const HEIGHT: usize = 32;
const NODE_DOMAIN: &[u8] = b"fastpq:v1:smt:node|";
const PAD_DOMAIN: &[u8] = b"fastpq:v1:smt:pad|";

/// Explicit limits for private touched-balance tree and path construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransferSmtBuildLimits {
    /// Maximum chronological participant updates, including zero/self updates.
    pub max_updates: usize,
    /// Maximum distinct full balance keys.
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

/// Locally generated private paths in original pair order, debit then credit.
/// These data establish no source authority or proof-verification result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivedTransferSmtWitnesses {
    roots: ([u8; 32], [u8; 32]),
    pairs: Vec<[TransferSmtWitness; 2]>,
    work: TransferSmtBuildWork,
}

impl DerivedTransferSmtWitnesses {
    /// Exact initial and final roots of the constructed touched-balance tree.
    #[must_use]
    pub const fn roots(&self) -> ([u8; 32], [u8; 32]) {
        self.roots
    }

    /// Private update paths, preserving every chronological occurrence.
    #[must_use]
    pub fn pairs(&self) -> &[[TransferSmtWitness; 2]] {
        &self.pairs
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
    ///
    /// # Errors
    /// Rejects construction limits, inconsistent internal ports or mismatching roots.
    pub fn build_smt_witnesses(
        &self,
        limits: TransferSmtBuildLimits,
    ) -> Result<DerivedTransferSmtWitnesses> {
        let built = derive(self, limits)?;
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
    let witnesses = derive(&prepared, tree_limits)?;
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

fn preflight<V>(
    prepared: &PreparedPublicTransfers<'_, V>,
    limits: TransferSmtBuildLimits,
) -> Result<TransferSmtBuildWork> {
    let updates = prepared
        .pairs
        .len()
        .checked_mul(2)
        .ok_or_else(|| invariant("SMT update count overflows"))?;
    let unique_keys = prepared.keys.len();
    check_limit("max_transfer_smt_updates", updates, limits.max_updates)?;
    check_limit("max_transfer_smt_keys", unique_keys, limits.max_unique_keys)?;
    check_limit(
        "max_transfer_smt_nodes",
        unique_keys,
        limits.max_retained_nodes,
    )?;
    if updates != prepared.rows.len() || (updates == 0) != (unique_keys == 0) {
        return Err(invariant("SMT public table cardinality is inconsistent"));
    }
    let sibling_hashes = updates
        .checked_mul(HEIGHT)
        .ok_or_else(|| invariant("SMT sibling count overflows"))?;
    check_limit(
        "max_transfer_smt_siblings",
        sibling_hashes,
        limits.max_sibling_hashes,
    )?;
    let mut paths: Vec<_> = prepared.keys.iter().map(|key| key.path).collect();
    paths.sort_unstable();
    if paths.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invariant("SMT public key paths are not unique"));
    }
    let mut retained_nodes = 0_usize;
    for level in 0..=HEIGHT {
        let mut previous = None;
        for path in &paths {
            let parent = u64::from(*path) >> level;
            if previous != Some(parent) {
                retained_nodes = checked_add(retained_nodes, 1)?;
                check_limit(
                    "max_transfer_smt_nodes",
                    retained_nodes,
                    limits.max_retained_nodes,
                )?;
                previous = Some(parent);
            }
        }
    }
    let node_hashes = checked_add(retained_nodes - unique_keys, sibling_hashes)?;
    check_limit(
        "max_transfer_smt_node_hashes",
        node_hashes,
        limits.max_node_hashes,
    )?;
    Ok(TransferSmtBuildWork {
        updates,
        unique_keys,
        retained_nodes,
        sibling_hashes,
        node_hashes,
    })
}

fn derive<V>(
    prepared: &PreparedPublicTransfers<'_, V>,
    limits: TransferSmtBuildLimits,
) -> Result<DerivedTransferSmtWitnesses> {
    let work = preflight(prepared, limits)?;
    if work.updates == 0 {
        return Ok(DerivedTransferSmtWitnesses {
            roots: (
                prepared.public_inputs.old_root,
                prepared.public_inputs.new_root,
            ),
            pairs: Vec::new(),
            work,
        });
    }
    let pads = core::array::from_fn(padding);
    let mut tree = Tree {
        levels: core::array::from_fn(|_| BTreeMap::new()),
        pads,
        hashes: 0,
    };
    let mut initial = vec![false; work.unique_keys];
    let mut rows_seen = vec![false; work.updates];
    for (ordinal, pair) in prepared.pairs.iter().enumerate() {
        if pair.occurrence.pair_ordinal as usize != ordinal {
            return Err(invariant("SMT pair occurrence order is inconsistent"));
        }
        for (leg, index) in pair.row_indices.into_iter().enumerate() {
            let row = prepared
                .rows
                .get(index)
                .ok_or_else(|| invariant("SMT row index is invalid"))?;
            let key = prepared
                .keys
                .get(row.key_index)
                .ok_or_else(|| invariant("SMT key index is invalid"))?;
            if std::mem::replace(&mut rows_seen[index], true)
                || row.role.is_debit() != u64::from(leg == 0)
                || row.update != pair.updates[leg]
                || row.update.path != key.path
                || row.occurrence != pair.occurrence
            {
                return Err(invariant(
                    "SMT pair ports differ from their exact public rows",
                ));
            }
            if !initial[row.key_index] {
                tree.levels[0].insert(key.path, digest(row.update.old_leaf)?);
                initial[row.key_index] = true;
            }
        }
    }
    if initial.iter().any(|seen| !seen) {
        return Err(invariant("SMT key lacks a first public occurrence"));
    }
    tree.seed();
    let old_root = tree.root().into();
    let mut pairs = Vec::with_capacity(prepared.pairs.len());
    for pair in &prepared.pairs {
        let debit = tree.update(pair.updates[0])?;
        let credit = tree.update(pair.updates[1])?;
        pairs.push([debit, credit]);
    }
    if tree.hashes != work.node_hashes
        || tree.levels.iter().map(BTreeMap::len).sum::<usize>() != work.retained_nodes
    {
        return Err(invariant("SMT construction work differs from preflight"));
    }
    Ok(DerivedTransferSmtWitnesses {
        roots: (old_root, tree.root().into()),
        pairs,
        work,
    })
}

struct Tree {
    levels: [BTreeMap<u32, Hash>; HEIGHT + 1],
    pads: [Hash; HEIGHT + 1],
    hashes: usize,
}

impl Tree {
    fn node(&mut self, level: usize, parent: u32) -> Hash {
        let left = self.levels[level]
            .get(&(parent << 1))
            .unwrap_or(&self.pads[level]);
        let right = self.levels[level]
            .get(&((parent << 1) | 1))
            .unwrap_or(&self.pads[level]);
        let hash = Hash::new_from_chunks(&[NODE_DOMAIN, left.as_ref(), right.as_ref()]);
        self.hashes += 1; // The checked preflight bounds every seed/update hash.
        hash
    }

    fn seed(&mut self) {
        for level in 0..HEIGHT {
            let parents: Vec<_> = self.levels[level].keys().map(|path| path >> 1).collect();
            let mut previous = None;
            for parent in parents {
                if previous != Some(parent) {
                    let hash = self.node(level, parent);
                    self.levels[level + 1].insert(parent, hash);
                    previous = Some(parent);
                }
            }
        }
    }

    fn root(&self) -> Hash {
        self.levels[HEIGHT]
            .get(&0)
            .copied()
            .unwrap_or(self.pads[HEIGHT])
    }

    fn update(
        &mut self,
        update: super::super::compact_smt_air::PublicUpdate,
    ) -> Result<TransferSmtWitness> {
        let before = digest(update.old_leaf)?;
        let after = digest(update.new_leaf)?;
        if self.levels[0].get(&update.path) != Some(&before) {
            return Err(invariant(
                "SMT chronological pre-leaf does not match current state",
            ));
        }
        let root_before = self.root().into();
        let mut siblings = Vec::with_capacity(HEIGHT);
        let mut path = update.path;
        self.levels[0].insert(path, after);
        for level in 0..HEIGHT {
            siblings.push(
                self.levels[level]
                    .get(&(path ^ 1))
                    .copied()
                    .unwrap_or(self.pads[level])
                    .into(),
            );
            let parent = path >> 1;
            let hash = self.node(level, parent);
            self.levels[level + 1].insert(parent, hash);
            path = parent;
        }
        Ok(TransferSmtWitness::new(
            root_before,
            self.root().into(),
            update.path.to_le_bytes().to_vec(),
            siblings,
        ))
    }
}

#[cfg(test)]
mod tests;
