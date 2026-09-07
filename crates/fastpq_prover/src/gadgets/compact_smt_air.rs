//! Narrow, directly linked SMT program for two sequential balance updates.
//!
//! Each update proves 32 levels in least-significant-path-bit order, alternating
//! an old-child hash and a new-child hash with one shared sibling. The complete
//! node preimage is `b"fastpq:v1:smt:node|" || left[32] || right[32]`; each hash
//! is the separately constrained 408-row compact BLAKE2b-256/Iroha primitive.
//! Full u32 child/sibling ports are linked to all message bits, including limbs
//! crossing adjacent 24-byte import rows. No digest is reduced to one field cell.
//!
//! Public leaves and collision-resolved paths must be independently authenticated
//! and included in the proof statement before transcript challenges. Public leaf
//! computation must use exact canonical keys/values and all domain hashes; the
//! public path must follow the complete allocation/probing rule in transfer.rs.
//! Public input types or witness constructors do not authenticate those facts.
//!
//! The logical program uses 52,224 rows and 342 base-field columns: 310 hash
//! columns plus four complete eight-u32 ports. Including an outer selector takes
//! 343 columns, below 512. The physical schedule pads each 408-row hash
//! to 512 rows, using all 65,536 rows for one transfer. Its 13,312 padding rows
//! carry SMT state and are not a free bridge budget. One exact transfer is the
//! scope here; this does not establish multi-transfer capacity or proof-byte fit.
//!
//! All local/transition numerators have degree at most two. For degree-<N column
//! polynomials their numerators have degree at most 2(N-1); dividing by any
//! nonempty applicable fixed-domain zerofier gives degree <2N. Linear links have
//! degree <N before division. The complete trace needs degree <N and the combined
//! quotient needs the conservative <2N bound. An alternative multiplies by
//! verifier-known degree-<N periodic selectors and divides by the full degree-N
//! row zerofier; it also needs a proved <2N quotient bound. Free witness-selector
//! gates are not that construction. Schedule/path-specific domains must
//! be authenticated and evaluated correctly at LDE points, not inferred from a
//! sampled row label; final/update/block boundaries are not cyclic transitions.
//!
//! At export-to-padding edges the just-hashed child advances; all four ports
//! persist through 104 rows with zero hash cells. A new sibling is introduced
//! only at the next level's import boundary. Across the update boundary, the
//! prior new root passes through padding and seeds the next starting root.
//! The final physical row binds the carried new root without a cyclic edge.
//!
//! TODO: Integrate the committed proof schema, fixed-domain zerofiers, public
//! statement binding and complete joint degree proof; validate default proof
//! bytes, runtime and independent protocol soundness. This module has no admission
//! call site and does not remove replay or change production limits. Neither
//! logical nor physical native row indices determine the AIR at an LDE point:
//! the verifier must evaluate the authenticated fixed schedule polynomials.

use super::{
    compact_blake2b_air::{self, CompactHashWitness, CompactRow},
    transfer_integer_air::IntegerAirField,
};

/// Complete little-endian 32-byte digest in eight exact u32 limbs.
pub type DigestLimbs = [u32; 8];
/// Fixed updates in one transfer delta: debit then credit.
pub const UPDATE_COUNT: usize = 2;
/// Exact number of authenticated path bits and shared siblings per update.
pub const PATH_LEVELS: usize = 32;
/// Old and new node hashes at each level.
pub const HASHES_PER_LEVEL: usize = 2;
/// Node hashes proved by this complete two-update program.
pub const HASH_COUNT: usize = UPDATE_COUNT * PATH_LEVELS * HASHES_PER_LEVEL;
/// Exact non-padding rows per transfer delta.
pub const ROW_COUNT: usize = HASH_COUNT * compact_blake2b_air::ROW_COUNT;
/// Physical subgroup period, including 104 state-carry padding rows.
pub const PHYSICAL_HASH_ROWS: usize = 512;
/// Exact physical rows for one transfer, including all state-carry padding.
pub const PHYSICAL_ROW_COUNT: usize = HASH_COUNT * PHYSICAL_HASH_ROWS;
/// Physical rows for one complete balance update.
pub const PHYSICAL_ROWS_PER_UPDATE: usize = PHYSICAL_ROW_COUNT / UPDATE_COUNT;
/// Rows belonging to one full balance update.
pub const ROWS_PER_UPDATE: usize = ROW_COUNT / UPDATE_COUNT;
/// Complete base-field schema width, excluding the outer active selector.
pub const COLUMN_COUNT: usize = compact_blake2b_air::COLUMN_COUNT + 4 * 8;
/// Highest algebraic degree in any local or transition numerator.
pub const MAX_CONSTRAINT_DEGREE: usize = 2;
/// Conservative quotient degree multiplier after fixed-domain division.
pub const QUOTIENT_DEGREE_EXPANSION: usize = 2;

const NODE_DOMAIN: &[u8; 19] = b"fastpq:v1:smt:node|";
const NODE_BYTES: usize = NODE_DOMAIN.len() + 64;

/// Exact public leaves and externally collision-resolved path for one update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicUpdate {
    /// Domain-separated old leaf computed from the exact public key and old value.
    pub old_leaf: DigestLimbs,
    /// Domain-separated new leaf computed from the exact public key and new value.
    pub new_leaf: DigestLimbs,
    /// Complete allocated path; this is not merely an unproved key-hash projection.
    pub path: u32,
}

/// Independently authenticated public statement; constructing it establishes no trust.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicStatement {
    /// Debit then credit leaves/paths in the exact public transfer order.
    pub updates: [PublicUpdate; UPDATE_COUNT],
    /// Root immediately before the debit.
    pub old_root: DigestLimbs,
    /// Root immediately after the credit.
    pub new_root: DigestLimbs,
}

/// Checked position in the fixed two-update program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowIndex(usize);

impl RowIndex {
    /// Accept exactly positions zero through 52,223.
    #[must_use]
    pub fn new(index: usize) -> Option<Self> {
        (index < ROW_COUNT).then_some(Self(index))
    }
    /// Absolute fixed schedule position.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
    /// Debit/credit update ordinal.
    #[must_use]
    pub const fn update(self) -> usize {
        self.0 / ROWS_PER_UPDATE
    }
    /// Current path level, in least-significant-bit order.
    #[must_use]
    pub const fn level(self) -> usize {
        (self.0 / compact_blake2b_air::ROW_COUNT / 2) % PATH_LEVELS
    }
    /// Whether the current hash processes the new child.
    #[must_use]
    pub const fn is_new(self) -> bool {
        (self.0 / compact_blake2b_air::ROW_COUNT) % 2 == 1
    }
    /// Fixed position inside the compact hash primitive.
    #[must_use]
    pub fn hash_index(self) -> compact_blake2b_air::RowIndex {
        compact_blake2b_air::RowIndex::new(self.0 % compact_blake2b_air::ROW_COUNT)
            .expect("remainder is in the fixed hash schedule")
    }
}

/// Checked base-domain position in the physical 512-row-per-hash schedule.
///
/// This position chooses a fixed constraint domain, not a witness opcode. It
/// must not be computed from a sampled LDE row label to evaluate constraints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalRowIndex(usize);

impl PhysicalRowIndex {
    /// Accept exactly positions zero through 65,535.
    #[must_use]
    pub fn new(index: usize) -> Option<Self> {
        (index < PHYSICAL_ROW_COUNT).then_some(Self(index))
    }
    /// Absolute physical schedule position.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
    /// Hash invocation ordinal, zero through 127.
    #[must_use]
    pub const fn hash_ordinal(self) -> usize {
        self.0 / PHYSICAL_HASH_ROWS
    }
    /// Phase within one hash invocation: 0..407 execute, 408..511 carry.
    #[must_use]
    pub const fn phase(self) -> usize {
        self.0 % PHYSICAL_HASH_ROWS
    }
    /// Whether this row contains canonical zero hash cells and carried SMT ports.
    #[must_use]
    pub const fn is_padding(self) -> bool {
        self.phase() >= compact_blake2b_air::ROW_COUNT
    }
    /// Debit/credit update ordinal.
    #[must_use]
    pub const fn update(self) -> usize {
        self.0 / PHYSICAL_ROWS_PER_UPDATE
    }
    /// Current path level, in least-significant-bit order.
    #[must_use]
    pub const fn level(self) -> usize {
        (self.hash_ordinal() / HASHES_PER_LEVEL) % PATH_LEVELS
    }
    /// Whether this invocation hashes the new child.
    #[must_use]
    pub const fn is_new(self) -> bool {
        self.hash_ordinal() % HASHES_PER_LEVEL == 1
    }
    /// Corresponding logical position, or `None` on a padding row.
    #[must_use]
    pub fn logical_index(self) -> Option<RowIndex> {
        (!self.is_padding())
            .then(|| RowIndex(self.hash_ordinal() * compact_blake2b_air::ROW_COUNT + self.phase()))
    }
}

/// One complete narrow hash/SMT row with all carried ports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SmtRow<F = u64> {
    /// One row of the fully constrained compact hash.
    pub hash: CompactRow<F>,
    /// Current old child, advanced only after the old node hash completes.
    pub old_child: [F; 8],
    /// Current new child, advanced only after the new node hash completes.
    pub new_child: [F; 8],
    /// Same complete sibling throughout both hashes of this level.
    pub sibling: [F; 8],
    /// Root before this update, carried from the public root or prior update's new root.
    pub starting_root: [F; 8],
}

impl<F: IntegerAirField> SmtRow<F> {
    /// Canonical zero row for inactive programs.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            hash: CompactRow::zero(),
            old_child: [F::ZERO; 8],
            new_child: [F::ZERO; 8],
            sibling: [F::ZERO; 8],
            starting_root: [F::ZERO; 8],
        }
    }
}

/// Fixed-shape heap-backed program; semantic validity remains a polynomial obligation.
#[derive(Debug, PartialEq, Eq)]
pub struct SmtWitness<F = u64> {
    rows: Box<[SmtRow<F>]>,
}

impl<F> SmtWitness<F> {
    /// Check the exact public row count without pretending to validate arithmetic.
    #[must_use]
    pub fn from_rows(rows: Vec<SmtRow<F>>) -> Option<Self> {
        (rows.len() == ROW_COUNT).then(|| Self {
            rows: rows.into_boxed_slice(),
        })
    }
    /// All fixed schedule rows.
    #[must_use]
    pub fn rows(&self) -> &[SmtRow<F>] {
        &self.rows
    }
    /// Mutate openings while preserving the complete schedule shape.
    #[must_use]
    pub fn rows_mut(&mut self) -> &mut [SmtRow<F>] {
        &mut self.rows
    }
}

/// Fixed-shape physical program, including canonical hash padding and SMT carries.
#[derive(Debug, PartialEq, Eq)]
pub struct PhysicalSmtWitness<F = u64> {
    rows: Box<[SmtRow<F>]>,
}

impl<F> PhysicalSmtWitness<F> {
    /// Check the exact public shape; arithmetic remains a polynomial obligation.
    #[must_use]
    pub fn from_rows(rows: Vec<SmtRow<F>>) -> Option<Self> {
        (rows.len() == PHYSICAL_ROW_COUNT).then(|| Self {
            rows: rows.into_boxed_slice(),
        })
    }
    /// All 65,536 rows in their authenticated fixed schedule order.
    #[must_use]
    pub fn rows(&self) -> &[SmtRow<F>] {
        &self.rows
    }
    /// Mutate openings while retaining the exact physical schedule shape.
    #[must_use]
    pub fn rows_mut(&mut self) -> &mut [SmtRow<F>] {
        &mut self.rows
    }
}

impl<F: IntegerAirField> SmtWitness<F> {
    /// Consume a logical witness and insert all 104 carry rows after each hash.
    ///
    /// Reverse in-place copies avoid retaining a second complete logical
    /// witness. Invalid logical rows stay invalid: this is a layout conversion,
    /// not a validity check or a source of authenticated input.
    #[must_use]
    pub fn into_physical(self) -> PhysicalSmtWitness<F> {
        let mut rows = self.rows.into_vec();
        rows.resize(PHYSICAL_ROW_COUNT, SmtRow::zero());
        for hash in (0..HASH_COUNT).rev() {
            let source = hash * compact_blake2b_air::ROW_COUNT;
            let destination = hash * PHYSICAL_HASH_ROWS;
            rows.copy_within(source..source + compact_blake2b_air::ROW_COUNT, destination);
            let mut padding = rows[destination + compact_blake2b_air::ROW_COUNT - 1];
            if hash % HASHES_PER_LEVEL == 0 {
                padding.old_child = padding.hash.digest;
            } else {
                padding.new_child = padding.hash.digest;
            }
            padding.hash = CompactRow::zero();
            rows[destination + compact_blake2b_air::ROW_COUNT..destination + PHYSICAL_HASH_ROWS]
                .fill(padding);
        }
        PhysicalSmtWitness {
            rows: rows.into_boxed_slice(),
        }
    }
}

impl PhysicalSmtWitness<u64> {
    /// Generate the exact physical program for two consistent sequential updates.
    ///
    /// Statement authentication and all AIR obligations are still required.
    #[must_use]
    pub fn from_inputs(
        statement: &PublicStatement,
        siblings: &[[DigestLimbs; PATH_LEVELS]; UPDATE_COUNT],
    ) -> Option<Self> {
        SmtWitness::from_inputs(statement, siblings).map(SmtWitness::into_physical)
    }
}

fn marked(digest: &DigestLimbs) -> bool {
    digest[7] & (1 << 24) != 0
}
fn bytes(digest: &DigestLimbs) -> [u8; 32] {
    core::array::from_fn(|index| digest[index / 4].to_le_bytes()[index % 4])
}
fn output_limbs(hash: &CompactHashWitness) -> DigestLimbs {
    hash.rows()[compact_blake2b_air::ROW_COUNT - 1]
        .digest
        .map(|limb| u32::try_from(limb).expect("native compact hash outputs exact u32 limbs"))
}

impl SmtWitness<u64> {
    /// Generate all node hashes and direct carries for a consistent marked statement.
    ///
    /// Returns `None` on noncanonical markers or mismatching root chains. These
    /// native checks help witness construction; the AIR independently binds all
    /// roots, paths and marker bits and never substitutes these checks for proof.
    #[must_use]
    pub fn from_inputs(
        statement: &PublicStatement,
        siblings: &[[DigestLimbs; PATH_LEVELS]; UPDATE_COUNT],
    ) -> Option<Self> {
        if !marked(&statement.old_root)
            || !marked(&statement.new_root)
            || statement
                .updates
                .iter()
                .any(|u| !marked(&u.old_leaf) || !marked(&u.new_leaf))
            || siblings.iter().flatten().any(|s| !marked(s))
        {
            return None;
        }
        let mut rows = Vec::with_capacity(ROW_COUNT);
        let mut starting_root = statement.old_root;
        for update in 0..UPDATE_COUNT {
            let public = statement.updates[update];
            let mut old = public.old_leaf;
            let mut new = public.new_leaf;
            for level in 0..PATH_LEVELS {
                let sibling = siblings[update][level];
                for is_new in [false, true] {
                    let child = if is_new { new } else { old };
                    let (left, right) = if (public.path >> level) & 1 == 0 {
                        (child, sibling)
                    } else {
                        (sibling, child)
                    };
                    let mut payload = [0; NODE_BYTES];
                    payload[..NODE_DOMAIN.len()].copy_from_slice(NODE_DOMAIN);
                    payload[19..51].copy_from_slice(&bytes(&left));
                    payload[51..].copy_from_slice(&bytes(&right));
                    let hash = CompactHashWitness::from_bytes(&payload)
                        .expect("exact 83-byte node payload");
                    rows.extend(hash.rows().iter().copied().map(|hash| SmtRow {
                        hash,
                        old_child: old.map(u64::from),
                        new_child: new.map(u64::from),
                        sibling: sibling.map(u64::from),
                        starting_root: starting_root.map(u64::from),
                    }));
                    if is_new {
                        new = output_limbs(&hash);
                    } else {
                        old = output_limbs(&hash);
                    }
                }
            }
            if old != starting_root {
                return None;
            }
            starting_root = new;
        }
        if starting_root != statement.new_root {
            return None;
        }
        Self::from_rows(rows)
    }
}

fn message_bit<F: Copy>(row: &SmtRow<F>, byte: usize, bit: usize) -> F {
    row.hash.bits[byte / 8][(byte % 8) * 8 + bit]
}

fn expected_input<'a, F: Copy>(
    row: &'a SmtRow<F>,
    statement: &PublicStatement,
    index: RowIndex,
    port: usize,
) -> &'a [F; 8] {
    let right_child = (statement.updates[index.update()].path >> index.level()) & 1 != 0;
    if (port == 1) == right_child {
        if index.is_new() {
            &row.new_child
        } else {
            &row.old_child
        }
    } else {
        &row.sibling
    }
}

fn pack_input_limb<F: IntegerAirField>(
    row: &SmtRow<F>,
    next: Option<&SmtRow<F>>,
    offset: usize,
) -> F {
    let mut packed = F::ZERO;
    let mut weight = F::ONE;
    for bit in 0..32 {
        let byte = offset + bit / 8;
        let value = if byte < 24 {
            message_bit(row, byte, bit % 8)
        } else {
            message_bit(
                next.expect("crossing limb uses the adjacent import row"),
                byte - 24,
                bit % 8,
            )
        };
        packed = packed.add(value.mul(weight));
        weight = weight.add(weight);
    }
    packed
}

/// Local fixed-domain numerators, including all hash constraints and full boundaries.
#[must_use]
pub fn local_residues<F: IntegerAirField>(
    active: F,
    index: RowIndex,
    row: &SmtRow<F>,
    statement: &PublicStatement,
) -> Vec<F> {
    let hash_index = index.hash_index();
    let local = hash_index.get();
    let mut out = compact_blake2b_air::local_residues(active, hash_index, &row.hash);
    out.push(
        row.hash
            .byte_len
            .sub(F::from_u32(NODE_BYTES as u32).mul(active)),
    );
    if local < 6 {
        let first_byte = 24 * local;
        for (byte, &value) in NODE_DOMAIN.iter().enumerate() {
            if (first_byte..first_byte + 24).contains(&byte) {
                for bit in 0..8 {
                    out.push(message_bit(row, byte - first_byte, bit).sub(
                        if (value >> bit) & 1 == 1 {
                            active
                        } else {
                            F::ZERO
                        },
                    ));
                }
            }
        }
        for marker in [50, 82] {
            if (first_byte..first_byte + 24).contains(&marker) {
                out.push(message_bit(row, marker - first_byte, 0).sub(active));
            }
        }
        for port in 0..2 {
            for limb in 0..8 {
                let start = 19 + 32 * port + 4 * limb;
                if start / 24 == local && start % 24 <= 20 {
                    out.push(
                        pack_input_limb(row, None, start % 24)
                            .sub(expected_input(row, statement, index, port)[limb]),
                    );
                }
            }
        }
    }
    if index.0 % ROWS_PER_UPDATE == 0 {
        let public = statement.updates[index.update()];
        for limb in 0..8 {
            out.push(row.old_child[limb].sub(F::from_u32(public.old_leaf[limb]).mul(active)));
            out.push(row.new_child[limb].sub(F::from_u32(public.new_leaf[limb]).mul(active)));
            if index.0 == 0 {
                out.push(
                    row.starting_root[limb].sub(F::from_u32(statement.old_root[limb]).mul(active)),
                );
            }
        }
    }
    if index.0 % ROWS_PER_UPDATE == ROWS_PER_UPDATE - 1 {
        for limb in 0..8 {
            out.push(row.old_child[limb].sub(row.starting_root[limb]));
        }
    }
    if index.0 == ROW_COUNT - 1 {
        for limb in 0..8 {
            out.push(row.hash.digest[limb].sub(F::from_u32(statement.new_root[limb]).mul(active)));
        }
    }
    out
}

/// Direct intra-hash, old/new, level and update-chain transition numerators.
///
/// Three unaligned u32 node-input limbs use adjacent import rows. All carried
/// ports remain equality-linked on those edges. `None` denotes the single final
/// program row, which requires local root boundaries and has no cyclic edge.
#[must_use]
pub fn transition_residues<F: IntegerAirField>(
    active: F,
    index: RowIndex,
    row: &SmtRow<F>,
    next: &SmtRow<F>,
    statement: &PublicStatement,
) -> Option<Vec<F>> {
    if index.0 == ROW_COUNT - 1 {
        return None;
    }
    let local = index.hash_index().get();
    let mut out = Vec::new();
    if local < compact_blake2b_air::ROW_COUNT - 1 {
        out.extend(
            compact_blake2b_air::transition_residues(
                active,
                index.hash_index(),
                &row.hash,
                &next.hash,
            )
            .expect("nonterminal hash edge"),
        );
        for port in 0..2 {
            for limb in 0..8 {
                let start = 19 + 32 * port + 4 * limb;
                if start / 24 == local && start % 24 > 20 {
                    out.push(
                        pack_input_limb(row, Some(next), start % 24)
                            .sub(expected_input(row, statement, index, port)[limb]),
                    );
                }
            }
        }
    }
    let end_hash = local == compact_blake2b_air::ROW_COUNT - 1;
    let end_update = index.0 % ROWS_PER_UPDATE == ROWS_PER_UPDATE - 1;
    if end_update {
        for limb in 0..8 {
            out.push(next.starting_root[limb].sub(row.hash.digest[limb]));
        }
    } else {
        for limb in 0..8 {
            out.push(next.starting_root[limb].sub(row.starting_root[limb]));
            out.push(next.old_child[limb].sub(if end_hash && !index.is_new() {
                row.hash.digest[limb]
            } else {
                row.old_child[limb]
            }));
            out.push(next.new_child[limb].sub(if end_hash && index.is_new() {
                row.hash.digest[limb]
            } else {
                row.new_child[limb]
            }));
            if !end_hash || !index.is_new() {
                out.push(next.sibling[limb].sub(row.sibling[limb]));
            }
        }
    }
    Some(out)
}

/// Fixed-domain local numerators for the complete physical schedule.
///
/// Execute rows retain the logical residue ordering. Padding rows begin with
/// all 310 hash cells, in the declared `CompactRow` field order, proving they
/// are exactly zero. Final update/program boundaries follow those hash slots.
/// The active selector and fixed schedule require the same authenticated outer
/// schema as the logical relation; native phase selection is not LDE evaluation.
#[must_use]
pub fn physical_local_residues<F: IntegerAirField>(
    active: F,
    index: PhysicalRowIndex,
    row: &SmtRow<F>,
    statement: &PublicStatement,
) -> Vec<F> {
    if let Some(logical) = index.logical_index() {
        return local_residues(active, logical, row, statement);
    }
    let mut out = Vec::with_capacity(compact_blake2b_air::COLUMN_COUNT + 16);
    out.extend(row.hash.working);
    out.extend(row.hash.message);
    out.extend(row.hash.chaining);
    out.extend(row.hash.bits.into_iter().flatten());
    out.extend(row.hash.carries);
    out.extend(row.hash.present);
    out.push(row.hash.byte_len);
    out.push(row.hash.prefix_count);
    out.extend(row.hash.digest);
    if index.0 % PHYSICAL_ROWS_PER_UPDATE == PHYSICAL_ROWS_PER_UPDATE - 1 {
        for limb in 0..8 {
            out.push(row.old_child[limb].sub(row.starting_root[limb]));
        }
    }
    if index.0 == PHYSICAL_ROW_COUNT - 1 {
        for limb in 0..8 {
            out.push(row.new_child[limb].sub(F::from_u32(statement.new_root[limb]).mul(active)));
        }
    }
    out
}

/// Noncyclic direct transitions, including export, padding and next-import edges.
///
/// Export advances the just-hashed child into padding. All four complete ports
/// then persist until the final padding edge. An old-to-new edge preserves the
/// shared sibling; a new-to-next-level edge introduces its next sibling. The
/// debit-to-credit edge seeds the next starting root from the carried new root,
/// while the next local import fixes its public leaves. No hash state is carried
/// across padding: every padding row independently forces all hash cells zero.
#[must_use]
pub fn physical_transition_residues<F: IntegerAirField>(
    active: F,
    index: PhysicalRowIndex,
    row: &SmtRow<F>,
    next: &SmtRow<F>,
    statement: &PublicStatement,
) -> Option<Vec<F>> {
    if index.0 == PHYSICAL_ROW_COUNT - 1 {
        return None;
    }
    if index.phase() < compact_blake2b_air::ROW_COUNT - 1 {
        return transition_residues(
            active,
            index
                .logical_index()
                .expect("executing nonterminal hash phase"),
            row,
            next,
            statement,
        );
    }
    let export = index.phase() == compact_blake2b_air::ROW_COUNT - 1;
    let last_padding = index.phase() == PHYSICAL_HASH_ROWS - 1;
    let end_update = index.0 % PHYSICAL_ROWS_PER_UPDATE == PHYSICAL_ROWS_PER_UPDATE - 1;
    let mut out = Vec::with_capacity(32);
    for limb in 0..8 {
        if end_update {
            out.push(next.starting_root[limb].sub(row.new_child[limb]));
            continue;
        }
        out.push(next.starting_root[limb].sub(row.starting_root[limb]));
        out.push(next.old_child[limb].sub(if export && !index.is_new() {
            row.hash.digest[limb]
        } else {
            row.old_child[limb]
        }));
        out.push(next.new_child[limb].sub(if export && index.is_new() {
            row.hash.digest[limb]
        } else {
            row.new_child[limb]
        }));
        if !last_padding || !index.is_new() {
            out.push(next.sibling[limb].sub(row.sibling[limb]));
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::super::transfer;
    use super::*;
    use crate::GoldilocksFp4V1;
    use std::sync::OnceLock;

    struct Fixture {
        statement: PublicStatement,
        siblings: [[DigestLimbs; PATH_LEVELS]; UPDATE_COUNT],
        witness: SmtWitness,
    }

    fn from_bytes(bytes: &[u8; 32]) -> DigestLimbs {
        core::array::from_fn(|index| {
            u32::from_le_bytes(bytes[4 * index..4 * index + 4].try_into().unwrap())
        })
    }

    fn fixture() -> &'static Fixture {
        // The complete program is about 143 MB as u64 cells. Share it instead
        // of cloning it per adversarial test or allocating a giant stack array.
        static FIXTURE: OnceLock<Fixture> = OnceLock::new();
        FIXTURE.get_or_init(|| {
            let sender = b"compact-smt/sender";
            let receiver = b"compact-smt/receiver";
            let (debit, credit) =
                transfer::build_transfer_smt_witness_pair(sender, 100, 93, receiver, 11, 18)
                    .unwrap();
            assert_eq!(debit.root_after, credit.root_before);
            let statement = PublicStatement {
                updates: [
                    PublicUpdate {
                        old_leaf: from_bytes(transfer::leaf_hash(sender, 100).as_ref()),
                        new_leaf: from_bytes(transfer::leaf_hash(sender, 93).as_ref()),
                        path: u32::from_le_bytes(debit.path_bits.as_slice().try_into().unwrap()),
                    },
                    PublicUpdate {
                        old_leaf: from_bytes(transfer::leaf_hash(receiver, 11).as_ref()),
                        new_leaf: from_bytes(transfer::leaf_hash(receiver, 18).as_ref()),
                        path: u32::from_le_bytes(credit.path_bits.as_slice().try_into().unwrap()),
                    },
                ],
                old_root: from_bytes(&debit.root_before),
                new_root: from_bytes(&credit.root_after),
            };
            let siblings = [
                core::array::from_fn(|i| from_bytes(&debit.siblings[i])),
                core::array::from_fn(|i| from_bytes(&credit.siblings[i])),
            ];
            let witness = SmtWitness::from_inputs(&statement, &siblings)
                .expect("native sequential SMT paths must generate the complete compact program");
            Fixture {
                statement,
                siblings,
                witness,
            }
        })
    }

    fn row(index: usize) -> &'static SmtRow {
        &fixture().witness.rows()[index]
    }

    fn local_valid(index: usize, row: &SmtRow, statement: &PublicStatement) -> bool {
        local_residues(1, RowIndex::new(index).unwrap(), row, statement)
            .iter()
            .all(|&v| v == 0)
    }

    fn transition_valid(
        index: usize,
        current: &SmtRow,
        next: &SmtRow,
        statement: &PublicStatement,
    ) -> bool {
        transition_residues(1, RowIndex::new(index).unwrap(), current, next, statement)
            .unwrap()
            .iter()
            .all(|&v| v == 0)
    }

    fn map_row<F: Copy>(row: &SmtRow, mut map: impl FnMut(u64) -> F) -> SmtRow<F> {
        SmtRow {
            hash: CompactRow {
                working: row.hash.working.map(&mut map),
                message: row.hash.message.map(&mut map),
                chaining: row.hash.chaining.map(&mut map),
                bits: row.hash.bits.map(|bits| bits.map(&mut map)),
                carries: row.hash.carries.map(&mut map),
                present: row.hash.present.map(&mut map),
                byte_len: map(row.hash.byte_len),
                prefix_count: map(row.hash.prefix_count),
                digest: row.hash.digest.map(&mut map),
            },
            old_child: row.old_child.map(&mut map),
            new_child: row.new_child.map(&mut map),
            sibling: row.sibling.map(&mut map),
            starting_root: row.starting_root.map(map),
        }
    }

    fn physical_row(index: usize) -> SmtRow {
        let index = PhysicalRowIndex::new(index).unwrap();
        if let Some(logical) = index.logical_index() {
            return *row(logical.get());
        }
        // Derive a single reference row, avoiding another complete cached
        // physical witness for every adversarial test.
        let export = (index.hash_ordinal() + 1) * compact_blake2b_air::ROW_COUNT - 1;
        let mut carried = *row(export);
        if index.is_new() {
            carried.new_child = carried.hash.digest;
        } else {
            carried.old_child = carried.hash.digest;
        }
        carried.hash = CompactRow::zero();
        carried
    }

    fn physical_local_valid(index: usize, current: &SmtRow) -> bool {
        physical_local_residues(
            1,
            PhysicalRowIndex::new(index).unwrap(),
            current,
            &fixture().statement,
        )
        .iter()
        .all(|&v| v == 0)
    }

    fn physical_transition_valid(index: usize, current: &SmtRow, next: &SmtRow) -> bool {
        physical_transition_residues(
            1,
            PhysicalRowIndex::new(index).unwrap(),
            current,
            next,
            &fixture().statement,
        )
        .unwrap()
        .iter()
        .all(|&v| v == 0)
    }

    fn mutate_port(row: &mut SmtRow, port: usize, limb: usize) {
        let value = match port {
            0 => &mut row.old_child[limb],
            1 => &mut row.new_child[limb],
            2 => &mut row.sibling[limb],
            _ => &mut row.starting_root[limb],
        };
        *value ^= 1;
    }

    #[test]
    fn physical_program_matches_native_smt_and_preserves_every_logical_row() {
        let fixture = fixture();
        let mut witness =
            PhysicalSmtWitness::from_inputs(&fixture.statement, &fixture.siblings).unwrap();
        assert_eq!(witness.rows().len(), PHYSICAL_ROW_COUNT);
        for (index, current) in witness.rows().iter().enumerate() {
            let position = PhysicalRowIndex::new(index).unwrap();
            assert_eq!(position.get(), index);
            assert_eq!(position.hash_ordinal(), index / 512);
            assert_eq!(position.phase(), index % 512);
            assert_eq!(position.update(), index / PHYSICAL_ROWS_PER_UPDATE);
            assert_eq!(position.level(), (index / 1024) % PATH_LEVELS);
            assert_eq!(position.is_new(), (index / 512) % 2 == 1);
            assert_eq!(*current, physical_row(index), "physical row {index}");
            assert!(physical_local_valid(index, current), "local row {index}");
            if index + 1 < PHYSICAL_ROW_COUNT {
                assert!(
                    physical_transition_valid(index, current, &witness.rows()[index + 1]),
                    "edge {index}"
                );
            }
        }
        let update_end = &witness.rows()[PHYSICAL_ROWS_PER_UPDATE - 1];
        assert_eq!(update_end.old_child, update_end.starting_root);
        assert_eq!(
            update_end.new_child,
            witness.rows()[PHYSICAL_ROWS_PER_UPDATE].starting_root
        );
        assert_eq!(
            witness.rows()[PHYSICAL_ROW_COUNT - 1].new_child,
            fixture.statement.new_root.map(u64::from)
        );
        witness.rows_mut()[408].hash.working[0] = 1;
        assert!(!physical_local_valid(408, &witness.rows()[408]));
        assert!(PhysicalRowIndex::new(PHYSICAL_ROW_COUNT).is_none());
        assert!(PhysicalSmtWitness::<u64>::from_rows(Vec::new()).is_none());
        let mut wrong = fixture.statement;
        wrong.new_root[7] &= !(1 << 24);
        assert!(PhysicalSmtWitness::from_inputs(&wrong, &fixture.siblings).is_none());
    }

    #[test]
    fn every_hash_cell_is_zero_on_padding_with_exact_slot_order() {
        for index in [408, 511, 512 + 408, PHYSICAL_ROWS_PER_UPDATE - 1, 65_535] {
            let original = physical_row(index);
            for cell in 0..compact_blake2b_air::COLUMN_COUNT {
                let mut seen = 0;
                let changed = map_row(&original, |value| {
                    let result = if seen == cell { value + 1 } else { value };
                    seen += 1;
                    result
                });
                let values = physical_local_residues(
                    1,
                    PhysicalRowIndex::new(index).unwrap(),
                    &changed,
                    &fixture().statement,
                );
                assert_eq!(values[cell], 1, "row {index}, padding slot {cell}");
                assert_eq!(
                    values[..compact_blake2b_air::COLUMN_COUNT]
                        .iter()
                        .filter(|&&v| v != 0)
                        .count(),
                    1
                );
                assert!(!physical_local_valid(index, &changed));
            }
        }
        for phase in compact_blake2b_air::ROW_COUNT..PHYSICAL_HASH_ROWS {
            let mut changed = physical_row(phase);
            changed.hash.bits[phase % 3][phase % 64] = 1;
            assert!(
                !physical_local_valid(phase, &changed),
                "padding phase {phase}"
            );
        }
    }

    #[test]
    fn all_hash_export_padding_and_import_edges_bind_every_carried_limb() {
        for hash in 0..HASH_COUNT {
            for phase in [407, 408, 459, 510, 511] {
                let index = hash * PHYSICAL_HASH_ROWS + phase;
                if index == PHYSICAL_ROW_COUNT - 1 {
                    continue;
                }
                let current = physical_row(index);
                let next = physical_row(index + 1);
                for port in 0..4 {
                    for limb in 0..8 {
                        let mut changed = next;
                        mutate_port(&mut changed, port, limb);
                        let accepted = physical_transition_valid(index, &current, &changed);
                        let update_boundary = index == PHYSICAL_ROWS_PER_UPDATE - 1;
                        let new_level = phase == 511 && hash % 2 == 1;
                        assert_eq!(
                            accepted,
                            (update_boundary && port != 3) || (new_level && port == 2),
                            "edge {index}, port {port}, limb {limb}"
                        );
                    }
                }
            }
        }
        // Drift is caught at every one of the 103 internal padding edges.
        for index in 408..511 {
            let mut next = physical_row(index + 1);
            next.sibling[index % 8] ^= 1;
            assert!(!physical_transition_valid(
                index,
                &physical_row(index),
                &next
            ));
        }
    }

    #[test]
    fn forged_padding_children_cannot_detach_exports_or_update_roots() {
        for hash in [0, 1, 63, 64, 126, 127] {
            let export = hash * PHYSICAL_HASH_ROWS + 407;
            let carried = physical_row(export + 1);
            for limb in 0..8 {
                let mut forged = carried;
                mutate_port(&mut forged, hash % 2, limb);
                assert!(!physical_transition_valid(
                    export,
                    &physical_row(export),
                    &forged
                ));
                // A constant forged run passes its internal equality, but both
                // external edges remain independently bound to the actual hash.
                assert!(physical_transition_valid(export + 1, &forged, &forged));
                if hash != HASH_COUNT - 1 {
                    let end = hash * PHYSICAL_HASH_ROWS + 511;
                    assert!(!physical_transition_valid(
                        end,
                        &forged,
                        &physical_row(end + 1)
                    ));
                }
            }
        }
        let boundary = PHYSICAL_ROWS_PER_UPDATE - 1;
        let current = physical_row(boundary);
        let mut next = physical_row(boundary + 1);
        next.starting_root = current.hash.digest;
        assert!(!physical_transition_valid(boundary, &current, &next));
        for limb in 0..8 {
            let mut final_row = physical_row(PHYSICAL_ROW_COUNT - 1);
            final_row.new_child[limb] ^= 1;
            assert!(!physical_local_valid(PHYSICAL_ROW_COUNT - 1, &final_row));
            final_row = physical_row(PHYSICAL_ROW_COUNT - 1);
            final_row.old_child[limb] ^= 1;
            assert!(!physical_local_valid(PHYSICAL_ROW_COUNT - 1, &final_row));
        }
        assert!(
            physical_transition_residues(
                1,
                PhysicalRowIndex::new(PHYSICAL_ROW_COUNT - 1).unwrap(),
                &physical_row(PHYSICAL_ROW_COUNT - 1),
                &SmtRow::zero(),
                &fixture().statement,
            )
            .is_none()
        );
    }

    #[test]
    fn physical_inactive_and_fp4_relations_cover_every_edge_kind() {
        let statement = &fixture().statement;
        let zero = SmtRow::<u64>::zero();
        let embed = |value| GoldilocksFp4V1::from_base(value).unwrap();
        for index in [
            0,
            2,
            406,
            407,
            408,
            510,
            511,
            512 + 407,
            1023,
            PHYSICAL_ROWS_PER_UPDATE - 1,
            PHYSICAL_ROWS_PER_UPDATE,
            PHYSICAL_ROW_COUNT - 1,
        ] {
            let position = PhysicalRowIndex::new(index).unwrap();
            assert!(
                physical_local_residues(0, position, &zero, statement)
                    .iter()
                    .all(|&v| v == 0)
            );
            let mut changed = physical_row(index);
            changed.hash.bits[0][17] = 2;
            changed.new_child[0] += 1;
            assert_eq!(
                physical_local_residues(1, position, &changed, statement)
                    .into_iter()
                    .map(embed)
                    .collect::<Vec<_>>(),
                physical_local_residues(
                    GoldilocksFp4V1::ONE,
                    position,
                    &map_row(&changed, embed),
                    statement,
                )
            );
            if index + 1 < PHYSICAL_ROW_COUNT {
                assert!(
                    physical_transition_residues(0, position, &zero, &zero, statement)
                        .unwrap()
                        .iter()
                        .all(|&v| v == 0)
                );
                let next = physical_row(index + 1);
                assert_eq!(
                    physical_transition_residues(1, position, &changed, &next, statement)
                        .unwrap()
                        .into_iter()
                        .map(embed)
                        .collect::<Vec<_>>(),
                    physical_transition_residues(
                        GoldilocksFp4V1::ONE,
                        position,
                        &map_row(&changed, embed),
                        &map_row(&next, embed),
                        statement,
                    )
                    .unwrap()
                );
            }
        }
        let mut noncanonical = zero;
        noncanonical.hash.byte_len = 1;
        assert!(
            physical_local_residues(
                0,
                PhysicalRowIndex::new(408).unwrap(),
                &noncanonical,
                statement,
            )
            .iter()
            .any(|&v| v != 0)
        );
    }

    #[test]
    fn complete_two_update_program_matches_existing_native_smt_and_exact_shape() {
        let fixture = fixture();
        assert_eq!(fixture.witness.rows().len(), ROW_COUNT);
        for index in 0..ROW_COUNT {
            let current = row(index);
            assert!(
                local_valid(index, current, &fixture.statement),
                "local row {index}"
            );
            if index + 1 < ROW_COUNT {
                assert!(
                    transition_valid(index, current, row(index + 1), &fixture.statement),
                    "edge {index}"
                );
            }
        }
        let boundary = row(ROWS_PER_UPDATE - 1);
        assert_eq!(boundary.hash.digest, row(ROWS_PER_UPDATE).starting_root);
        assert_eq!(
            row(ROW_COUNT - 1).hash.digest,
            fixture.statement.new_root.map(u64::from)
        );
        assert!(RowIndex::new(ROW_COUNT).is_none());
        assert!(SmtWitness::<u64>::from_rows(Vec::new()).is_none());
        assert!(
            transition_residues(
                1,
                RowIndex::new(ROW_COUNT - 1).unwrap(),
                row(ROW_COUNT - 1),
                row(0),
                &fixture.statement
            )
            .is_none()
        );
    }

    #[test]
    fn every_public_path_bit_fixes_both_old_and_new_child_order() {
        let fixture = fixture();
        for update in 0..UPDATE_COUNT {
            for level in 0..PATH_LEVELS {
                let mut wrong = fixture.statement;
                wrong.updates[update].path ^= 1 << level;
                for is_new in [false, true] {
                    let start = update * ROWS_PER_UPDATE
                        + (2 * level + usize::from(is_new)) * compact_blake2b_air::ROW_COUNT;
                    let mut rejected = false;
                    for offset in 0..4 {
                        rejected |= !local_valid(start + offset, row(start + offset), &wrong);
                        rejected |= !transition_valid(
                            start + offset,
                            row(start + offset),
                            row(start + offset + 1),
                            &wrong,
                        );
                    }
                    assert!(rejected, "update {update}, level {level}, new {is_new}");
                }
            }
        }
    }

    #[test]
    fn full_ports_and_crossing_message_limbs_cannot_drift() {
        let statement = &fixture().statement;
        // The three words starting at bytes23,47,71 cross import-row boundaries.
        for offset in 0..3 {
            let current = row(offset);
            let original = row(offset + 1);
            let mut changed = *original;
            changed.hash.bits[0][0] ^= 1;
            assert!(
                !transition_valid(offset, current, &changed, statement),
                "crossing limb at import edge {offset}"
            );
        }
        for index in [0, 23, 407, 408, 815, 816, ROWS_PER_UPDATE - 2] {
            for port in 0..4 {
                for limb in 0..8 {
                    let mut changed = *row(index + 1);
                    let value = match port {
                        0 => &mut changed.old_child[limb],
                        1 => &mut changed.new_child[limb],
                        2 => &mut changed.sibling[limb],
                        _ => &mut changed.starting_root[limb],
                    };
                    *value += 1;
                    let accepted = transition_valid(index, row(index), &changed, statement);
                    // A new sibling is intentionally introduced after both hashes,
                    // and is immediately bound to the next level's exact message.
                    assert_eq!(
                        accepted,
                        index == 815 && port == 2,
                        "edge {index}, port {port}, limb {limb}"
                    );
                }
            }
        }
        let mut alias = *row(0);
        alias.old_child[0] += 1 << 32;
        assert!(
            !local_valid(0, &alias, statement),
            "u32 port cannot admit a modular alias"
        );
    }

    #[test]
    fn shared_sibling_and_all_marker_bits_are_enforced() {
        let fixture = fixture();
        for level in 0..PATH_LEVELS {
            let old_end = (2 * level + 1) * compact_blake2b_air::ROW_COUNT - 1;
            let mut changed = *row(old_end + 1);
            changed.sibling[7] ^= 1 << 24;
            assert!(!transition_valid(
                old_end,
                row(old_end),
                &changed,
                &fixture.statement
            ));
        }
        for marker_byte in [50, 82] {
            let index = marker_byte / 24;
            let byte = marker_byte % 24;
            let mut changed = *row(index);
            changed.hash.bits[byte / 8][8 * (byte % 8)] = 0;
            assert!(
                !local_valid(index, &changed, &fixture.statement),
                "node hash input marker byte {marker_byte}"
            );
        }
        let mut siblings = fixture.siblings;
        siblings[0][0][7] &= !(1 << 24);
        assert!(SmtWitness::from_inputs(&fixture.statement, &siblings).is_none());
    }

    #[test]
    fn exact_domain_leaf_roots_and_update_chain_boundaries_reject_aliases() {
        let fixture = fixture();
        let mut wrong = fixture.statement;
        wrong.updates[0].old_leaf[7] ^= 1 << 24;
        assert!(!local_valid(0, row(0), &wrong));
        wrong = fixture.statement;
        wrong.updates[1].new_leaf[0] ^= 1;
        assert!(!local_valid(ROWS_PER_UPDATE, row(ROWS_PER_UPDATE), &wrong));
        wrong = fixture.statement;
        wrong.old_root[7] ^= 1 << 24;
        assert!(!local_valid(0, row(0), &wrong));
        wrong = fixture.statement;
        wrong.new_root[7] ^= 1 << 24;
        assert!(!local_valid(ROW_COUNT - 1, row(ROW_COUNT - 1), &wrong));
        for update in 0..UPDATE_COUNT {
            let index = (update + 1) * ROWS_PER_UPDATE - 1;
            let mut changed = *row(index);
            changed.old_child[0] ^= 1;
            assert!(
                !local_valid(index, &changed, &fixture.statement),
                "old reconstructed root boundary {update}"
            );
        }
        let index = ROWS_PER_UPDATE - 1;
        for limb in 0..8 {
            let mut changed = *row(index + 1);
            changed.starting_root[limb] ^= 1;
            assert!(
                !transition_valid(index, row(index), &changed, &fixture.statement),
                "update-chain limb {limb}"
            );
        }
        let mut domain = *row(0);
        domain.hash.bits[0][0] ^= 1;
        assert!(!local_valid(0, &domain, &fixture.statement));
        let mut length = *row(0);
        length.hash.byte_len = 82;
        assert!(!local_valid(0, &length, &fixture.statement));
    }

    #[test]
    fn compact_register_carries_remain_bound_inside_smt_program() {
        let fixture = fixture();
        for index in [7, 407 + 8, ROWS_PER_UPDATE + 7, ROW_COUNT - 18] {
            let mut changed = *row(index + 1);
            changed.hash.working[0] += 1;
            assert!(!transition_valid(
                index,
                row(index),
                &changed,
                &fixture.statement
            ));
        }
        for index in [7, 8, ROWS_PER_UPDATE + 7, ROWS_PER_UPDATE + 8] {
            let mut changed = *row(index);
            changed.hash.carries[0] ^= 1;
            assert!(!local_valid(index, &changed, &fixture.statement));
        }
    }

    #[test]
    fn inactive_and_fp4_relations_cover_hash_smt_and_update_edges() {
        let statement = &fixture().statement;
        let zero = SmtRow::<u64>::zero();
        let embed = |value| GoldilocksFp4V1::from_base(value).unwrap();
        for index in [
            0,
            1,
            2,
            3,
            6,
            7,
            407,
            408,
            815,
            ROWS_PER_UPDATE - 1,
            ROWS_PER_UPDATE,
            ROW_COUNT - 1,
        ] {
            assert!(
                local_residues(0, RowIndex::new(index).unwrap(), &zero, statement)
                    .iter()
                    .all(|&v| v == 0)
            );
            if index + 1 < ROW_COUNT {
                assert!(
                    transition_residues(0, RowIndex::new(index).unwrap(), &zero, &zero, statement)
                        .unwrap()
                        .iter()
                        .all(|&v| v == 0)
                );
            }
            let mut base = *row(index);
            base.hash.bits[0][17] = 2;
            base.sibling[0] += 1;
            let extension = map_row(&base, embed);
            assert_eq!(
                local_residues(1, RowIndex::new(index).unwrap(), &base, statement)
                    .into_iter()
                    .map(embed)
                    .collect::<Vec<_>>(),
                local_residues(
                    GoldilocksFp4V1::ONE,
                    RowIndex::new(index).unwrap(),
                    &extension,
                    statement
                )
            );
            if index + 1 < ROW_COUNT {
                let next = row(index + 1);
                assert_eq!(
                    transition_residues(1, RowIndex::new(index).unwrap(), &base, next, statement)
                        .unwrap()
                        .into_iter()
                        .map(embed)
                        .collect::<Vec<_>>(),
                    transition_residues(
                        GoldilocksFp4V1::ONE,
                        RowIndex::new(index).unwrap(),
                        &extension,
                        &map_row(next, embed),
                        statement
                    )
                    .unwrap()
                );
            }
        }
    }

    #[derive(Clone, Copy)]
    struct Degree(usize);
    impl IntegerAirField for Degree {
        const ZERO: Self = Self(0);
        const ONE: Self = Self(0);
        fn from_u32(_: u32) -> Self {
            Self(0)
        }
        fn add(self, o: Self) -> Self {
            Self(self.0.max(o.0))
        }
        fn sub(self, o: Self) -> Self {
            Self(self.0.max(o.0))
        }
        fn mul(self, o: Self) -> Self {
            Self(self.0 + o.0)
        }
    }

    #[test]
    fn exact_schema_width_rows_and_full_schedule_degree_fit_current_trace_cap() {
        let mut cells = 0;
        let degree_row = map_row(&SmtRow::zero(), |_| {
            cells += 1;
            Degree(1)
        });
        assert_eq!(cells, COLUMN_COUNT);
        assert_eq!(COLUMN_COUNT, 342);
        assert_eq!(ROW_COUNT, 52_224);
        assert_eq!(ROW_COUNT.next_power_of_two(), 65_536);
        assert_eq!(PHYSICAL_HASH_ROWS - compact_blake2b_air::ROW_COUNT, 104);
        assert_eq!(PHYSICAL_ROW_COUNT, 65_536);
        assert_eq!(PHYSICAL_ROW_COUNT - ROW_COUNT, 13_312);
        assert!(COLUMN_COUNT + 1 <= 512);
        let mut max_degree = 0;
        // Every distinct hash phase is exercised in each update; level changes
        // alter only public constant choices, not polynomial degree.
        for update in 0..UPDATE_COUNT {
            for local in 0..compact_blake2b_air::ROW_COUNT {
                let index = RowIndex::new(update * ROWS_PER_UPDATE + local).unwrap();
                for value in local_residues(Degree(1), index, &degree_row, &fixture().statement)
                    .into_iter()
                    .chain(
                        transition_residues(
                            Degree(1),
                            index,
                            &degree_row,
                            &degree_row,
                            &fixture().statement,
                        )
                        .into_iter()
                        .flatten(),
                    )
                {
                    max_degree = max_degree.max(value.0);
                }
            }
        }
        for index in [815, ROWS_PER_UPDATE - 1, ROW_COUNT - 1] {
            for value in local_residues(
                Degree(1),
                RowIndex::new(index).unwrap(),
                &degree_row,
                &fixture().statement,
            )
            .into_iter()
            .chain(
                transition_residues(
                    Degree(1),
                    RowIndex::new(index).unwrap(),
                    &degree_row,
                    &degree_row,
                    &fixture().statement,
                )
                .into_iter()
                .flatten(),
            ) {
                max_degree = max_degree.max(value.0);
            }
        }
        assert_eq!(max_degree, MAX_CONSTRAINT_DEGREE);
        let mut physical_degree = 0;
        for hash in [0, 1, 63, 64, 127] {
            for phase in 0..PHYSICAL_HASH_ROWS {
                let position = PhysicalRowIndex::new(hash * PHYSICAL_HASH_ROWS + phase).unwrap();
                for value in
                    physical_local_residues(Degree(1), position, &degree_row, &fixture().statement)
                        .into_iter()
                        .chain(
                            physical_transition_residues(
                                Degree(1),
                                position,
                                &degree_row,
                                &degree_row,
                                &fixture().statement,
                            )
                            .into_iter()
                            .flatten(),
                        )
                {
                    physical_degree = physical_degree.max(value.0);
                }
            }
        }
        assert_eq!(physical_degree, MAX_CONSTRAINT_DEGREE);
        assert_eq!(QUOTIENT_DEGREE_EXPANSION, 2);
        assert!(core::mem::size_of::<SmtRow<GoldilocksFp4V1>>() < 16 * 1024);
    }
}
