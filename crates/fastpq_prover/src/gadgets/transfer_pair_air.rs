//! Quadratic transfer-pair table and execution-permutation AIR numerators.
//!
//! This module is a protocol building block and is not wired into proof admission.
//! It compares complete field tuples, with multiplicity, between execution rows
//! and a canonical table containing exactly a debit and credit for every delta.
//! Table order is transcript order, then delta order, then debit before credit.
//! The two rows share the declared amount and complete pair identity; their
//! individual keys and balances are included in the permutation tuple.
//!
//! # Required statement and commitment phases
//!
//! 1. The verifier obtains an independently authenticated statement containing the
//!    execution profile, complete PublicIO, domain size, pair count, both packing
//!    widths, schema/scale convention, and the complete canonical table commitment.
//!    Prover metadata, an authority digest, or a witness-supplied flag cannot
//!    authenticate that statement. Both table rows must be derived from the same
//!    original delta, including the asset and both counterpart accounts.
//! 2. Execution and table base columns, including both inclusive counts, are
//!    committed before independently sampled Fp4 compression and shift challenges.
//!    The authenticated table must use canonical byte encodings, checked lengths
//!    and zero-padded 7-byte packing. These properties require authenticated table
//!    construction or additional byte/canonicalization AIR; Rust constructors alone
//!    do not constrain malicious openings. Execution tuples must bind to the actual
//!    execution columns, not a second unconnected copy.
//! 3. The two factor and two inclusive product Fp4 columns are committed before
//!    AIR aggregation challenges. All column oracles need degree proofs and every
//!    returned numerator needs the appropriate all-row, transition, or boundary
//!    zerofier. Extension challenges must not be truncated to the base field.
//!
//! Products avoid inversions. A zero factor can mask other differences and is a
//! challenge failure event, as are tuple-compression collisions. A production
//! soundness reduction must count those events and Fiat-Shamir/adaptive attempts;
//! this module neither retries challenges nor claims a concrete security level.
//! Counts enforce exact cardinality even when a factor equals the padding factor.
//! They do not remove the probabilistic product/compression error.
//!
//! TODO: Integrate authenticated statements, execution-column bindings, committed
//! postchallenge columns and their degree/quotient proofs. Full hash/key relations,
//! sequential roots, source authorization and witness replay remain mandatory.

use super::transfer_integer_air::{IntegerAirField, TransferIntegerWitness};

/// Maximum degree of a numerator in pre- and postchallenge trace variables.
pub const MAX_CONSTRAINT_DEGREE: usize = 2;
/// Additional base-field inclusive count columns.
pub const BASE_AUXILIARY_COLUMN_COUNT: usize = 2;
/// Additional extension-field columns: two factors and two inclusive products.
pub const EXTENSION_AUXILIARY_COLUMN_COUNT: usize = 4;

/// Checked public cardinality; construction does not authenticate its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairCardinality {
    pair_count: u32,
    trace_rows: u32,
    active_rows: u32,
}

impl PairCardinality {
    /// Require a nonzero power-of-two domain and room for both rows of every pair.
    ///
    /// A `u32` domain is strictly smaller than the Goldilocks modulus. Starting
    /// from zero and adding Boolean selectors therefore counts exact integers.
    #[must_use]
    pub fn new(pair_count: u32, trace_rows: u32) -> Option<Self> {
        let active_rows = pair_count.checked_mul(2)?;
        (trace_rows.is_power_of_two() && active_rows <= trace_rows).then_some(Self {
            pair_count,
            trace_rows,
            active_rows,
        })
    }

    /// Number of complete transfer pairs in the authenticated statement.
    #[must_use]
    pub const fn pair_count(self) -> u32 {
        self.pair_count
    }

    /// Exact committed trace-domain size, also required by the surrounding proof.
    #[must_use]
    pub const fn trace_rows(self) -> u32 {
        self.trace_rows
    }

    /// Exact number of active rows required in each table.
    #[must_use]
    pub const fn active_rows(self) -> u32 {
        self.active_rows
    }
}

/// Complete length-delimited byte string represented by zero-padded 7-byte limbs.
///
/// Arbitrary field openings require the canonicality contract in the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedIdentity<F, const LIMBS: usize> {
    /// Original byte length, including any trailing zero bytes.
    pub byte_len: F,
    /// Little-endian groups of at most seven bytes; unused high bytes/limbs are zero.
    pub limbs: [F; LIMBS],
}

impl<const LIMBS: usize> PackedIdentity<u64, LIMBS> {
    /// Pack all bytes injectively, rejecting truncation or an unrepresentable length.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let capacity = LIMBS.checked_mul(7)?;
        let byte_len = u32::try_from(bytes.len()).ok()?;
        if bytes.len() > capacity {
            return None;
        }
        let mut limbs = [0; LIMBS];
        for (limb, chunk) in limbs.iter_mut().zip(bytes.chunks(7)) {
            let mut word = [0; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            *limb = u64::from_le_bytes(word);
        }
        Some(Self {
            byte_len: u64::from(byte_len),
            limbs,
        })
    }
}

impl<F: IntegerAirField, const LIMBS: usize> PackedIdentity<F, LIMBS> {
    /// Empty, zero-padded identity used on inactive rows.
    #[must_use]
    pub fn inactive() -> Self {
        Self {
            byte_len: F::ZERO,
            limbs: [F::ZERO; LIMBS],
        }
    }

    fn append_fields(&self, fields: &mut Vec<F>) {
        fields.push(self.byte_len);
        fields.extend_from_slice(&self.limbs);
    }
}

/// Complete identity shared by the debit and credit of one transcript delta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairIdentity<F, const LIMBS: usize> {
    /// Canonical execution-call identity, including its transaction/call occurrence.
    pub call: PackedIdentity<F, LIMBS>,
    /// Canonical authority identity; equality alone does not prove authorization.
    pub authority: PackedIdentity<F, LIMBS>,
    /// Canonical full asset-definition identity.
    pub asset: PackedIdentity<F, LIMBS>,
    /// Canonical full sender account identity.
    pub sender: PackedIdentity<F, LIMBS>,
    /// Canonical full receiver account identity.
    pub receiver: PackedIdentity<F, LIMBS>,
    /// Normalized asset scale fixed by the authenticated table schema.
    pub asset_scale: F,
}

impl<F: IntegerAirField, const LIMBS: usize> PairIdentity<F, LIMBS> {
    /// Canonical zero identity for an inactive row.
    #[must_use]
    pub fn inactive() -> Self {
        Self {
            call: PackedIdentity::inactive(),
            authority: PackedIdentity::inactive(),
            asset: PackedIdentity::inactive(),
            sender: PackedIdentity::inactive(),
            receiver: PackedIdentity::inactive(),
            asset_scale: F::ZERO,
        }
    }

    fn append_fields(&self, fields: &mut Vec<F>) {
        for identity in [
            &self.call,
            &self.authority,
            &self.asset,
            &self.sender,
            &self.receiver,
        ] {
            identity.append_fields(fields);
        }
        fields.push(self.asset_scale);
    }
}

/// Tuple compared between the execution and canonical pair tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairTuple<F, const KEY_LIMBS: usize, const IDENTITY_LIMBS: usize> {
    /// Zero-based ordinal in transcript/delta order, shared by exactly two table rows.
    pub pair_ordinal: F,
    /// One for debit, zero for credit; zero-amount rows retain their declared role.
    pub is_debit: F,
    /// Complete row-specific balance key, not a scalar projection or truncated hash.
    pub key: PackedIdentity<F, KEY_LIMBS>,
    /// Balance before the row, packed in exact 56+8-bit limbs.
    pub before: [F; 2],
    /// Balance after the row, packed in exact 56+8-bit limbs.
    pub after: [F; 2],
    /// Declared amount in the same exact 56+8-bit representation.
    pub amount: [F; 2],
    /// Both counterpart accounts, asset, call, authority and scale of the delta.
    pub identity: PairIdentity<F, IDENTITY_LIMBS>,
}

impl<F: IntegerAirField, const KEY: usize, const IDENTITY: usize> PairTuple<F, KEY, IDENTITY> {
    /// Canonical tuple for inactive execution or table rows.
    #[must_use]
    pub fn inactive() -> Self {
        Self {
            pair_ordinal: F::ZERO,
            is_debit: F::ZERO,
            key: PackedIdentity::inactive(),
            before: [F::ZERO; 2],
            after: [F::ZERO; 2],
            amount: [F::ZERO; 2],
            identity: PairIdentity::inactive(),
        }
    }

    /// Exact field order for compression and the authenticated table schema.
    #[must_use]
    pub fn fields(&self) -> Vec<F> {
        let mut fields = Vec::with_capacity(tuple_field_count::<KEY, IDENTITY>());
        fields.extend_from_slice(&[self.pair_ordinal, self.is_debit]);
        self.key.append_fields(&mut fields);
        fields.extend_from_slice(&self.before);
        fields.extend_from_slice(&self.after);
        fields.extend_from_slice(&self.amount);
        self.identity.append_fields(&mut fields);
        fields
    }
}

/// Exact tuple width: ordinal, role, key, six numeric limbs and complete pair identity.
#[must_use]
pub const fn tuple_field_count<const KEY: usize, const IDENTITY: usize>() -> usize {
    15 + KEY + 5 * IDENTITY
}

/// One row from each prechallenge table and their committed inclusive counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairBaseRow<F, const KEY: usize, const IDENTITY: usize> {
    /// Boolean selector for an execution transfer row; metadata/padding select zero.
    pub execution_active: F,
    /// Boolean selector for the canonical table's active prefix.
    pub table_active: F,
    /// Execution tuple bound to the actual execution/integer columns.
    pub execution: PairTuple<F, KEY, IDENTITY>,
    /// Independently authenticated canonical table tuple.
    pub table: PairTuple<F, KEY, IDENTITY>,
    /// Number of execution transfer rows up to and including this row.
    pub execution_count: F,
    /// Number of active canonical table rows up to and including this row.
    pub table_count: F,
}

/// Independently sampled extension-field challenges fixed after base commitments.
///
/// Instantiate with Fp4 for the protocol. The generic field supports algebraic tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairChallenges<F> {
    /// Polynomial tuple-compression challenge.
    pub compression: F,
    /// Independent additive shift in the multiset factors.
    pub shift: F,
}

/// Four Fp4 columns committed after challenges and before AIR aggregation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairExtensionRow<F> {
    /// Execution factor, equal to one on inactive rows.
    pub execution_factor: F,
    /// Table factor, equal to one on inactive rows.
    pub table_factor: F,
    /// Inclusive product of execution factors through this row.
    pub execution_product: F,
    /// Inclusive product of table factors through this row.
    pub table_product: F,
}

/// Linear tuple compression with a fixed challenge; every field participates.
#[must_use]
pub fn compress_tuple<F: IntegerAirField, const KEY: usize, const IDENTITY: usize>(
    tuple: &PairTuple<F, KEY, IDENTITY>,
    challenge: F,
) -> F {
    tuple
        .fields()
        .into_iter()
        .rev()
        .fold(F::ZERO, |value, field| value.mul(challenge).add(field))
}

/// Generate factor/product witnesses without divisions or challenge resampling.
///
/// `previous_products` is `[1, 1]` for the first row. Witness generation does not
/// validate the base table or constitute any authorization decision.
#[must_use]
pub fn extension_witness<F: IntegerAirField, const KEY: usize, const IDENTITY: usize>(
    row: &PairBaseRow<F, KEY, IDENTITY>,
    challenges: PairChallenges<F>,
    previous_products: [F; 2],
) -> PairExtensionRow<F> {
    let factor = |active: F, tuple: &PairTuple<F, KEY, IDENTITY>| {
        F::ONE.add(
            active.mul(
                challenges
                    .shift
                    .add(compress_tuple(tuple, challenges.compression))
                    .sub(F::ONE),
            ),
        )
    };
    let execution_factor = factor(row.execution_active, &row.execution);
    let table_factor = factor(row.table_active, &row.table);
    PairExtensionRow {
        execution_factor,
        table_factor,
        execution_product: previous_products[0].mul(execution_factor),
        table_product: previous_products[1].mul(table_factor),
    }
}

/// Number of all-row numerators, including zeroed inactive tuple fields.
#[must_use]
pub const fn row_constraint_count<const KEY: usize, const IDENTITY: usize>() -> usize {
    7 + 2 * tuple_field_count::<KEY, IDENTITY>()
}

/// Evaluate Booleanity, inactive zeros, table ordinal/count binding and factors.
///
/// These numerators use the all-row zerofier. Count recurrences and pair ordering
/// are separate transition constraints, not host-side acceptance predicates.
#[must_use]
pub fn row_residues<F: IntegerAirField, const KEY: usize, const IDENTITY: usize>(
    row: &PairBaseRow<F, KEY, IDENTITY>,
    extension: &PairExtensionRow<F>,
    challenges: PairChallenges<F>,
) -> Vec<F> {
    let mut residues = Vec::with_capacity(row_constraint_count::<KEY, IDENTITY>());
    for (active, tuple) in [
        (row.execution_active, &row.execution),
        (row.table_active, &row.table),
    ] {
        residues.push(active.mul(active.sub(F::ONE)));
        residues.push(tuple.is_debit.mul(tuple.is_debit.sub(active)));
        for value in tuple.fields() {
            residues.push(F::ONE.sub(active).mul(value));
        }
    }
    let two = F::from_u32(2);
    residues.push(
        row.table_active.mul(
            two.mul(row.table.pair_ordinal)
                .sub(row.table_count)
                .add(two)
                .sub(row.table.is_debit),
        ),
    );
    let expected = extension_witness(row, challenges, [F::ONE; 2]);
    residues.push(extension.execution_factor.sub(expected.execution_factor));
    residues.push(extension.table_factor.sub(expected.table_factor));
    residues
}

/// Bind the execution tuple's numeric columns and declared role to the integer AIR.
///
/// The existing integer gadget must also be enforced with the same selector and
/// exact eight-byte lengths. Zero-amount debit witnesses must set `is_debit = 1`;
/// direction inferred from balance comparison would lose that declared role.
#[must_use]
pub fn integer_binding_residues<F: IntegerAirField, const KEY: usize, const IDENTITY: usize>(
    execution_active: F,
    tuple: &PairTuple<F, KEY, IDENTITY>,
    integer: &TransferIntegerWitness<F>,
) -> [F; 7] {
    let tuple_values = [
        tuple.before[0],
        tuple.before[1],
        tuple.after[0],
        tuple.after[1],
        tuple.amount[0],
        tuple.amount[1],
        tuple.is_debit,
    ];
    let integer_values = [
        integer.before.packed[0],
        integer.before.packed[1],
        integer.after.packed[0],
        integer.after.packed[1],
        integer.amount.packed[0],
        integer.amount.packed[1],
        integer.is_debit,
    ];
    core::array::from_fn(|index| {
        execution_active.mul(tuple_values[index].sub(integer_values[index]))
    })
}

/// First-row numerators: counts start at their selectors and products at their factors.
#[must_use]
pub fn initial_residues<F: IntegerAirField, const KEY: usize, const IDENTITY: usize>(
    row: &PairBaseRow<F, KEY, IDENTITY>,
    extension: &PairExtensionRow<F>,
) -> [F; 6] {
    [
        row.execution_count.sub(row.execution_active),
        row.table_count.sub(row.table_active),
        row.table.is_debit.sub(row.table_active),
        row.table.pair_ordinal,
        extension.execution_product.sub(extension.execution_factor),
        extension.table_product.sub(extension.table_factor),
    ]
}

/// Number of transition numerators: seven structural and all shared pair fields.
#[must_use]
pub const fn transition_constraint_count<const IDENTITY: usize>() -> usize {
    15 + 5 * IDENTITY
}

/// Evaluate transitions on every row except the final domain row.
///
/// Selectors need no cubic gates: factor columns absorb selection before product
/// recurrence. Activated debit roles gate pair equality. Alternation also prevents
/// an incomplete debit row from being followed by padding.
#[must_use]
pub fn transition_residues<F: IntegerAirField, const KEY: usize, const IDENTITY: usize>(
    current: &PairBaseRow<F, KEY, IDENTITY>,
    next: &PairBaseRow<F, KEY, IDENTITY>,
    current_extension: &PairExtensionRow<F>,
    next_extension: &PairExtensionRow<F>,
) -> Vec<F> {
    let mut residues = Vec::with_capacity(transition_constraint_count::<IDENTITY>());
    residues.extend_from_slice(&[
        next.execution_count
            .sub(current.execution_count)
            .sub(next.execution_active),
        next.table_count
            .sub(current.table_count)
            .sub(next.table_active),
        next.table_active.mul(F::ONE.sub(current.table_active)),
        next.table
            .is_debit
            .add(current.table.is_debit)
            .sub(current.table_active.mul(next.table_active)),
        next.table_active.mul(
            next.table
                .pair_ordinal
                .sub(current.table.pair_ordinal)
                .sub(F::ONE)
                .add(current.table.is_debit),
        ),
        next_extension.execution_product.sub(
            current_extension
                .execution_product
                .mul(next_extension.execution_factor),
        ),
        next_extension.table_product.sub(
            current_extension
                .table_product
                .mul(next_extension.table_factor),
        ),
    ]);
    let mut current_shared = current.table.amount.to_vec();
    current.table.identity.append_fields(&mut current_shared);
    let mut next_shared = next.table.amount.to_vec();
    next.table.identity.append_fields(&mut next_shared);
    residues.extend(
        current_shared
            .into_iter()
            .zip(next_shared)
            .map(|(left, right)| current.table.is_debit.mul(right.sub(left))),
    );
    residues
}

/// Final-row numerators: exact cardinalities, complete pairs and equal multiset products.
#[must_use]
pub fn terminal_residues<F: IntegerAirField, const KEY: usize, const IDENTITY: usize>(
    row: &PairBaseRow<F, KEY, IDENTITY>,
    extension: &PairExtensionRow<F>,
    cardinality: PairCardinality,
) -> [F; 4] {
    let expected = F::from_u32(cardinality.active_rows());
    [
        row.execution_count.sub(expected),
        row.table_count.sub(expected),
        row.table.is_debit,
        extension.execution_product.sub(extension.table_product),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::transfer_integer_air::{self, Unsigned64Witness};
    use super::*;
    use crate::{GOLDILOCKS_MODULUS_V1, GoldilocksFp4V1};

    type Tuple<F = u64> = PairTuple<F, 2, 2>;
    type Row<F = u64> = PairBaseRow<F, 2, 2>;

    fn challenges() -> PairChallenges<GoldilocksFp4V1> {
        PairChallenges {
            compression: GoldilocksFp4V1::new([17, 3, 5, 7]).unwrap(),
            shift: GoldilocksFp4V1::new([19, 11, 13, 23]).unwrap(),
        }
    }

    fn identity(seed: u64) -> PackedIdentity<u64, 2> {
        PackedIdentity::from_bytes(&seed.to_le_bytes()).unwrap()
    }

    fn tuples() -> [Tuple; 4] {
        core::array::from_fn(|index| {
            let pair = index / 2;
            let is_debit = index % 2 == 0;
            let amount = if pair == 0 { 3 } else { 0 };
            let before = 10 + u64::try_from(index).unwrap();
            let after = if is_debit {
                before - amount
            } else {
                before + amount
            };
            PairTuple {
                pair_ordinal: u64::try_from(pair).unwrap(),
                is_debit: u64::from(is_debit),
                // Repeated keys in different pairs are intentional.
                key: identity(100 + u64::from(is_debit)),
                before: Unsigned64Witness::from_integer(before).packed,
                after: Unsigned64Witness::from_integer(after).packed,
                amount: Unsigned64Witness::from_integer(amount).packed,
                identity: PairIdentity {
                    call: identity(200 + u64::try_from(pair).unwrap()),
                    authority: identity(300),
                    asset: identity(400),
                    sender: identity(500),
                    receiver: identity(600),
                    asset_scale: 2,
                },
            }
        })
    }

    fn fixture(order: [usize; 4]) -> Vec<Row> {
        let tuples = tuples();
        let mut rows: Vec<_> = (0..8)
            .map(|index| Row {
                execution_active: u64::from([0, 2, 5, 7].contains(&index)),
                table_active: u64::from(index < 4),
                execution: PairTuple::inactive(),
                table: tuples
                    .get(index)
                    .copied()
                    .unwrap_or_else(PairTuple::inactive),
                execution_count: 0,
                table_count: 0,
            })
            .collect();
        for (row, tuple_index) in [0, 2, 5, 7].into_iter().zip(order) {
            rows[row].execution = tuples[tuple_index];
        }
        recount(&mut rows);
        rows
    }

    fn recount(rows: &mut [Row]) {
        let mut execution_count = 0;
        let mut table_count = 0;
        for row in rows {
            execution_count += row.execution_active;
            table_count += row.table_active;
            row.execution_count = execution_count;
            row.table_count = table_count;
        }
    }

    fn map_identity<F: Copy>(
        identity: PackedIdentity<u64, 2>,
        map: &mut impl FnMut(u64) -> F,
    ) -> PackedIdentity<F, 2> {
        PackedIdentity {
            byte_len: map(identity.byte_len),
            limbs: identity.limbs.map(map),
        }
    }

    fn map_tuple<F: Copy>(tuple: Tuple, map: &mut impl FnMut(u64) -> F) -> Tuple<F> {
        PairTuple {
            pair_ordinal: map(tuple.pair_ordinal),
            is_debit: map(tuple.is_debit),
            key: map_identity(tuple.key, map),
            before: tuple.before.map(&mut *map),
            after: tuple.after.map(&mut *map),
            amount: tuple.amount.map(&mut *map),
            identity: PairIdentity {
                call: map_identity(tuple.identity.call, map),
                authority: map_identity(tuple.identity.authority, map),
                asset: map_identity(tuple.identity.asset, map),
                sender: map_identity(tuple.identity.sender, map),
                receiver: map_identity(tuple.identity.receiver, map),
                asset_scale: map(tuple.identity.asset_scale),
            },
        }
    }

    fn map_row<F: Copy>(row: Row, map: &mut impl FnMut(u64) -> F) -> Row<F> {
        PairBaseRow {
            execution_active: map(row.execution_active),
            table_active: map(row.table_active),
            execution: map_tuple(row.execution, map),
            table: map_tuple(row.table, map),
            execution_count: map(row.execution_count),
            table_count: map(row.table_count),
        }
    }

    fn lift(rows: &[Row]) -> Vec<Row<GoldilocksFp4V1>> {
        rows.iter()
            .map(|&row| map_row(row, &mut |value| GoldilocksFp4V1::from_base(value).unwrap()))
            .collect()
    }

    fn auxiliary<F: IntegerAirField>(
        rows: &[Row<F>],
        challenges: PairChallenges<F>,
    ) -> Vec<PairExtensionRow<F>> {
        let mut products = [F::ONE; 2];
        rows.iter()
            .map(|row| {
                let extension = extension_witness(row, challenges, products);
                products = [extension.execution_product, extension.table_product];
                extension
            })
            .collect()
    }

    fn all_residues<F: IntegerAirField>(
        rows: &[Row<F>],
        extension: &[PairExtensionRow<F>],
        challenges: PairChallenges<F>,
        cardinality: PairCardinality,
    ) -> Vec<F> {
        assert_eq!(
            usize::try_from(cardinality.trace_rows()).unwrap(),
            rows.len()
        );
        let mut residues = initial_residues(&rows[0], &extension[0]).to_vec();
        for (row, extension) in rows.iter().zip(extension) {
            let row_residues = row_residues(row, extension, challenges);
            assert_eq!(row_residues.len(), row_constraint_count::<2, 2>());
            residues.extend(row_residues);
        }
        for index in 0..rows.len() - 1 {
            let transitions = transition_residues(
                &rows[index],
                &rows[index + 1],
                &extension[index],
                &extension[index + 1],
            );
            assert_eq!(transitions.len(), transition_constraint_count::<2>());
            residues.extend(transitions);
        }
        residues.extend(terminal_residues(
            rows.last().unwrap(),
            extension.last().unwrap(),
            cardinality,
        ));
        residues
    }

    fn valid(rows: &[Row]) -> bool {
        let rows = lift(rows);
        let extension = auxiliary(&rows, challenges());
        all_residues(
            &rows,
            &extension,
            challenges(),
            PairCardinality::new(2, 8).unwrap(),
        )
        .iter()
        .all(|residue| residue.is_zero())
    }

    fn mutate_tuple_field(tuple: Tuple, field_index: usize) -> Tuple {
        let mut index = 0;
        map_tuple(tuple, &mut |value| {
            let mapped = if index == field_index {
                value + 1
            } else {
                value
            };
            index += 1;
            mapped
        })
    }

    #[test]
    fn cardinality_rejects_overflow_non_domains_and_partial_capacity() {
        for (pairs, rows) in [(1, 0), (1, 1), (1, 3), (3, 4), (u32::MAX, 1 << 31)] {
            assert!(PairCardinality::new(pairs, rows).is_none());
        }
        let largest = PairCardinality::new(1 << 30, 1 << 31).unwrap();
        assert_eq!(largest.pair_count(), 1 << 30);
        assert_eq!(largest.active_rows(), 1 << 31);
        assert_eq!(largest.trace_rows(), 1 << 31);
        assert!(u64::from(largest.trace_rows()) < GOLDILOCKS_MODULUS_V1);
        assert_eq!(PairCardinality::new(0, 1).unwrap().active_rows(), 0);
    }

    #[test]
    fn packing_preserves_lengths_every_byte_and_zero_padding() {
        let seven = PackedIdentity::<u64, 2>::from_bytes(&[255; 7]).unwrap();
        assert_eq!(seven.limbs, [(1 << 56) - 1, 0]);
        assert_ne!(
            seven,
            PackedIdentity::from_bytes(&[255, 255, 255, 255, 255, 255, 255, 0]).unwrap()
        );
        assert_eq!(
            PackedIdentity::<u64, 2>::from_bytes(&[255; 14])
                .unwrap()
                .limbs,
            [(1 << 56) - 1; 2]
        );
        assert!(PackedIdentity::<u64, 2>::from_bytes(&[0; 15]).is_none());
        assert_eq!(
            PackedIdentity::<u64, 0>::from_bytes(&[]).unwrap(),
            PackedIdentity::inactive()
        );
        assert_eq!(
            Tuple::<u64>::inactive().fields(),
            vec![0; tuple_field_count::<2, 2>()]
        );
    }

    #[test]
    fn all_execution_permutations_keep_repeated_keys_and_zero_amount_roles() {
        let mut checked = 0;
        for a in 0..4 {
            for b in 0..4 {
                for c in 0..4 {
                    for d in 0..4 {
                        let order = [a, b, c, d];
                        if (0..4).all(|value| order.contains(&value)) {
                            assert!(valid(&fixture(order)), "{order:?}");
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(checked, 24);
    }

    #[test]
    fn each_tuple_field_is_bound_by_the_permutation() {
        let baseline = fixture([3, 0, 2, 1]);
        for field in 0..tuple_field_count::<2, 2>() {
            let mut rows = baseline.clone();
            rows[0].execution = mutate_tuple_field(rows[0].execution, field);
            assert!(!valid(&rows), "unbound tuple field {field}");
        }
        let mut rows = baseline;
        rows[0].execution = rows[2].execution;
        assert!(
            !valid(&rows),
            "duplicated one occurrence while retaining exact count"
        );
    }

    #[test]
    fn active_cardinality_and_inactive_tuple_values_are_constrained() {
        let baseline = fixture([0, 1, 2, 3]);
        let mut missing = baseline.clone();
        missing[7].execution_active = 0;
        missing[7].execution = Tuple::inactive();
        recount(&mut missing);
        assert!(!valid(&missing));
        let mut extra = baseline.clone();
        extra[1].execution_active = 1;
        extra[1].execution = extra[0].execution;
        recount(&mut extra);
        assert!(!valid(&extra));
        for field in 0..tuple_field_count::<2, 2>() {
            let mut hidden = baseline.clone();
            hidden[1].execution = mutate_tuple_field(hidden[1].execution, field);
            assert!(!valid(&hidden), "inactive field {field}");
        }
    }

    #[test]
    fn table_requires_prefix_alternation_ordinals_and_complete_pairs() {
        let baseline = fixture([0, 1, 2, 3]);
        for index in 0..4 {
            let mut role = baseline.clone();
            role[index].table.is_debit ^= 1;
            assert!(!valid(&role));
            let mut ordinal = baseline.clone();
            ordinal[index].table.pair_ordinal += 1;
            assert!(!valid(&ordinal));
        }
        let mut gap = baseline.clone();
        gap[4].table = gap[1].table;
        gap[4].table_active = 1;
        gap[1].table = Tuple::inactive();
        gap[1].table_active = 0;
        recount(&mut gap);
        assert!(!valid(&gap));
        let mut incomplete = baseline;
        incomplete[3].table = Tuple::inactive();
        incomplete[3].table_active = 0;
        recount(&mut incomplete);
        assert!(!valid(&incomplete));
    }

    #[test]
    fn pair_amount_and_every_counterpart_identity_field_must_match() {
        let baseline = fixture([0, 1, 2, 3]);
        // Fields after key and old/new balances include amount and complete identity.
        for field in 9..tuple_field_count::<2, 2>() {
            let mut rows = baseline.clone();
            let changed = mutate_tuple_field(rows[1].table, field);
            rows[1].table = changed;
            rows[2].execution = changed;
            assert!(
                !valid(&rows),
                "shared field {field} must match despite equal multisets"
            );
        }
    }

    #[test]
    fn committed_factors_products_and_counts_cannot_be_changed_independently() {
        let base = fixture([2, 0, 3, 1]);
        let rows = lift(&base);
        let extension = auxiliary(&rows, challenges());
        for index in 0..rows.len() {
            for field in 0..4 {
                let mut changed = extension.clone();
                let row = &mut changed[index];
                let value = match field {
                    0 => &mut row.execution_factor,
                    1 => &mut row.table_factor,
                    2 => &mut row.execution_product,
                    _ => &mut row.table_product,
                };
                *value = value.add(GoldilocksFp4V1::ONE);
                assert!(
                    all_residues(
                        &rows,
                        &changed,
                        challenges(),
                        PairCardinality::new(2, 8).unwrap()
                    )
                    .iter()
                    .any(|residue| !residue.is_zero()),
                    "row {index}, auxiliary {field}"
                );
            }
            for table_count in [false, true] {
                let mut changed = base.clone();
                if table_count {
                    changed[index].table_count += 1;
                } else {
                    changed[index].execution_count += 1;
                }
                assert!(!valid(&changed));
            }
        }
    }

    #[test]
    fn zero_amount_debit_binds_declared_role_without_changing_arithmetic() {
        let tuple = tuples()[2];
        let mut integer = TransferIntegerWitness::from_balances(12, 12);
        assert_eq!(
            integer.is_debit, 0,
            "native inference has no direction information"
        );
        assert_ne!(integer_binding_residues(1, &tuple, &integer), [0; 7]);
        integer.is_debit = 1;
        assert_eq!(integer_binding_residues(1, &tuple, &integer), [0; 7]);
        assert_eq!(
            transfer_integer_air::constraint_residues(1, 8, 8, &integer),
            [0; transfer_integer_air::CONSTRAINT_COUNT]
        );
        integer.amount.packed[0] = 1;
        assert_ne!(integer_binding_residues(1, &tuple, &integer), [0; 7]);
    }

    #[test]
    fn exact_numeric_limbs_distinguish_modular_aliases() {
        let mut left = tuples()[0];
        left.before = Unsigned64Witness::from_integer(GOLDILOCKS_MODULUS_V1).packed;
        let mut right = left;
        right.before = Unsigned64Witness::from_integer(0).packed;
        let mut lift = |value| GoldilocksFp4V1::from_base(value).unwrap();
        let left = map_tuple(left, &mut lift);
        let right = map_tuple(right, &mut lift);
        assert_ne!(
            compress_tuple(&left, challenges().compression),
            compress_tuple(&right, challenges().compression)
        );
    }

    #[test]
    fn empty_single_row_domain_has_unit_products_and_zero_counts() {
        let row = Row {
            execution_active: 0,
            table_active: 0,
            execution: Tuple::inactive(),
            table: Tuple::inactive(),
            execution_count: 0,
            table_count: 0,
        };
        let rows = lift(&[row]);
        let extension = auxiliary(&rows, challenges());
        assert_eq!(extension[0].execution_product, GoldilocksFp4V1::ONE);
        assert!(
            all_residues(
                &rows,
                &extension,
                challenges(),
                PairCardinality::new(0, 1).unwrap()
            )
            .iter()
            .all(|residue| residue.is_zero())
        );
    }

    #[test]
    fn zero_factor_is_a_real_challenge_failure_not_a_deterministic_security_claim() {
        let mut rows = fixture([0, 1, 2, 3]);
        rows[7].execution.key.limbs[0] += 1;
        assert!(!valid(&rows));
        let rows = lift(&rows);
        let mut bad = challenges();
        bad.shift = GoldilocksFp4V1::ZERO.sub(compress_tuple(&rows[0].table, bad.compression));
        let extension = auxiliary(&rows, bad);
        assert!(extension[0].execution_factor.is_zero());
        assert!(extension[0].table_factor.is_zero());
        assert!(
            all_residues(&rows, &extension, bad, PairCardinality::new(2, 8).unwrap())
                .iter()
                .all(|residue| residue.is_zero()),
            "shared zero masks a later different key; soundness must count this event"
        );
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Degree(usize);

    impl IntegerAirField for Degree {
        const ZERO: Self = Self(0);
        const ONE: Self = Self(0);
        fn from_u32(_: u32) -> Self {
            Self(0)
        }
        fn add(self, other: Self) -> Self {
            Self(self.0.max(other.0))
        }
        fn sub(self, other: Self) -> Self {
            Self(self.0.max(other.0))
        }
        fn mul(self, other: Self) -> Self {
            Self(self.0 + other.0)
        }
    }

    #[test]
    fn every_numerator_has_at_most_quadratic_formal_degree() {
        let row = map_row(fixture([0, 1, 2, 3])[0], &mut |_| Degree(1));
        let extension = PairExtensionRow {
            execution_factor: Degree(1),
            table_factor: Degree(1),
            execution_product: Degree(1),
            table_product: Degree(1),
        };
        let challenges = PairChallenges {
            compression: Degree(0),
            shift: Degree(0),
        };
        let residues = all_residues(
            &[row; 8],
            &[extension; 8],
            challenges,
            PairCardinality::new(2, 8).unwrap(),
        );
        assert_eq!(
            residues.iter().map(|degree| degree.0).max(),
            Some(MAX_CONSTRAINT_DEGREE)
        );
        let integer = TransferIntegerWitness::<Degree>::inactive();
        assert!(
            integer_binding_residues(Degree(1), &row.execution, &integer)
                .iter()
                .all(|degree| degree.0 <= MAX_CONSTRAINT_DEGREE)
        );
    }
}
