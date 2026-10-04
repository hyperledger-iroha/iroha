//! **Prototype**: the G1 statement field encoding of the split-lineage step
//! relations, as measured in M7 (`g3_proof_scaling_measurement_tests.rs`,
//! `m7_step`).
//!
//! The split-lineage design (step proofs `sigma_send`/`sigma_recv` on the
//! payment path, the recursive lineage proof in the background) is a
//! proposal whose owner approval is pending. Nothing in this module is a
//! protocol format: the domain, relation identifiers and field order are
//! the M7 measurement labels, kept so the prototype step relations hash the
//! same statement M7 measured. No protocol path uses this module; a frozen
//! encoding will get its own versioned type and vectors.
//!
//! # Encoding (32 field elements of the proof's own parity `F`)
//!
//! | index | field |
//! | --- | --- |
//! | 0 | version (1) |
//! | 1-2 | scheme id, two little-endian 128-bit limbs |
//! | 3-4 | relation id (`u128`), 0 |
//! | 5-6 | credential digest limbs |
//! | 7-8 | asset id limbs |
//! | 9 | successor lifecycle |
//! | 10 | successor sequence |
//! | 11 | next load ordinal |
//! | 12 | predecessor commitment, own parity |
//! | 13-14 | predecessor commitment, other parity, two limbs |
//! | 15 | successor commitment, own parity |
//! | 16-17 | successor commitment, other parity, two limbs |
//! | 18 | effect tag (Send 3, Receive 4) |
//! | 19-31 | the effect union, zero padded to 13 fields |
//!
//! The digest is the KAGEMUSHA sponge `hash_with_domain(m7stmnt1, fields)`,
//! 18 permutations, or 17 with the folded prefix. A verifier recomputes it
//! natively from the canonical statement, so the limbs in it are bound by
//! the public digest.
//!
//! # Cross-field values (spec S6)
//!
//! A value of the other Pasta field enters this circuit's field as two
//! limbs. Where a relation must also reason about such a value (not only
//! hash it), [`assign_foreign_scalar`] constrains the limbs to the canonical
//! injective encoding `lo < 2^128`, `hi < 2^127`, `lo + 2^128 hi < modulus`,
//! so `s >= modulus` cannot be passed as `s mod modulus`.

use iroha_pasta::{PastaField, poseidon::PoseidonField};
use iroha_plonk::frontend::{Error, Region, Value};

use crate::{
    cells::{Bit, U128, Uint, Word},
    poseidon::{AbsorbInput, SpongeChip},
    range::u128::UintChip,
};

/// Fields of the statement encoding.
pub const STATEMENT_FIELDS: usize = 32;
/// Fields of the effect union.
pub const EFFECT_UNION_FIELDS: usize = 13;
/// The prototype statement domain (M7 label).
pub const STATEMENT_DOMAIN: u64 = u64::from_le_bytes(*b"m7stmnt1");
/// The statement version.
pub const STATEMENT_VERSION: u64 = 1;

/// The prototype step relation a statement belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StepRelation {
    /// `sigma_send`.
    Send,
    /// `sigma_recv`.
    Receive,
}

impl StepRelation {
    /// The relation identifier (M7 label).
    #[must_use]
    pub const fn relation_id(self) -> u128 {
        match self {
            Self::Send => u128::from_le_bytes(*b"m7-sigma-send-v1"),
            Self::Receive => u128::from_le_bytes(*b"m7-sigma-recv-v1"),
        }
    }

    /// The effect tag.
    #[must_use]
    pub const fn effect_tag(self) -> u64 {
        match self {
            Self::Send => 3,
            Self::Receive => 4,
        }
    }
}

/// The two little-endian 128-bit limbs `(lo, hi)` of a 32-byte value.
#[must_use]
pub fn bytes_to_limbs(bytes: &[u8; 32]) -> [u128; 2] {
    let mut lo = [0_u8; 16];
    let mut hi = [0_u8; 16];
    lo.copy_from_slice(&bytes[..16]);
    hi.copy_from_slice(&bytes[16..]);
    [u128::from_le_bytes(lo), u128::from_le_bytes(hi)]
}

/// The limbs as field elements.
#[must_use]
pub fn limb_fields<F: PastaField>(limbs: [u128; 2]) -> [F; 2] {
    limbs.map(F::from_u128)
}

/// The native prototype statement (own parity `F`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatementV1<F> {
    /// The step relation.
    pub relation: StepRelation,
    /// The scheme identifier.
    pub scheme_id: [u8; 32],
    /// The credential digest.
    pub credential: [u8; 32],
    /// The asset identifier.
    pub asset: [u8; 32],
    /// The successor lifecycle.
    pub lifecycle: u64,
    /// The successor sequence number.
    pub sequence: u128,
    /// The next load ordinal.
    pub next_load: u128,
    /// The predecessor commitment component of this parity.
    pub predecessor: F,
    /// The canonical encoding of the predecessor's other-parity component.
    pub predecessor_other: [u8; 32],
    /// The successor commitment component of this parity.
    pub successor: F,
    /// The canonical encoding of the successor's other-parity component.
    pub successor_other: [u8; 32],
    /// The effect fields (at most 13).
    pub effect: Vec<F>,
}

impl<F: PoseidonField> StatementV1<F> {
    /// The 32-field encoding, or `None` for more than 13 effect fields.
    #[must_use]
    pub fn encode(&self) -> Option<[F; STATEMENT_FIELDS]> {
        if self.effect.len() > EFFECT_UNION_FIELDS {
            return None;
        }
        let limbs = |bytes: &[u8; 32]| limb_fields::<F>(bytes_to_limbs(bytes));
        let [scheme_lo, scheme_hi] = limbs(&self.scheme_id);
        let [credential_lo, credential_hi] = limbs(&self.credential);
        let [asset_lo, asset_hi] = limbs(&self.asset);
        let [predecessor_lo, predecessor_hi] = limbs(&self.predecessor_other);
        let [successor_lo, successor_hi] = limbs(&self.successor_other);
        let mut fields = vec![
            F::from(STATEMENT_VERSION),
            scheme_lo,
            scheme_hi,
            F::from_u128(self.relation.relation_id()),
            F::ZERO,
            credential_lo,
            credential_hi,
            asset_lo,
            asset_hi,
            F::from(self.lifecycle),
            F::from_u128(self.sequence),
            F::from_u128(self.next_load),
            self.predecessor,
            predecessor_lo,
            predecessor_hi,
            self.successor,
            successor_lo,
            successor_hi,
            F::from(self.relation.effect_tag()),
        ];
        fields.extend_from_slice(&self.effect);
        fields.resize(STATEMENT_FIELDS, F::ZERO);
        fields.try_into().ok()
    }

    /// The statement digest, or `None` for more than 13 effect fields.
    #[must_use]
    pub fn digest(&self) -> Option<F> {
        self.encode()
            .map(|fields| iroha_pasta::poseidon::hash_with_domain(STATEMENT_DOMAIN, &fields))
    }
}

/// The in-circuit statement fields (constants are added by
/// [`statement_digest`]).
#[derive(Clone, Copy, Debug)]
pub struct StatementCells<'a, F: PastaField> {
    /// The scheme identifier limbs.
    pub scheme_id: [&'a Word<F>; 2],
    /// The credential digest limbs.
    pub credential: [&'a Word<F>; 2],
    /// The asset identifier limbs.
    pub asset: [&'a Word<F>; 2],
    /// The successor lifecycle.
    pub lifecycle: &'a Word<F>,
    /// The successor sequence number.
    pub sequence: &'a Word<F>,
    /// The next load ordinal.
    pub next_load: &'a Word<F>,
    /// The predecessor commitment component of this parity.
    pub predecessor: &'a Word<F>,
    /// The predecessor's other-parity component limbs.
    pub predecessor_other: [&'a Word<F>; 2],
    /// The successor commitment component of this parity.
    pub successor: &'a Word<F>,
    /// The successor's other-parity component limbs.
    pub successor_other: [&'a Word<F>; 2],
    /// The effect fields (at most 13; the rest are zero).
    pub effect: &'a [Word<F>],
}

/// Hashes the statement encoding of `cells` for `relation` and returns the
/// digest cell.
///
/// # Errors
///
/// [`Error::Synthesis`] for more than 13 effect fields, and [`Error`] from
/// the layout.
pub fn statement_digest<F: PoseidonField>(
    sponge: &mut SpongeChip<F>,
    region: &mut Region<'_, F>,
    relation: StepRelation,
    cells: &StatementCells<'_, F>,
) -> Result<Word<F>, Error> {
    if cells.effect.len() > EFFECT_UNION_FIELDS {
        return Err(Error::Synthesis);
    }
    let word = AbsorbInput::Word;
    let mut inputs = vec![
        AbsorbInput::Constant(F::from(STATEMENT_VERSION)),
        word(cells.scheme_id[0]),
        word(cells.scheme_id[1]),
        AbsorbInput::Constant(F::from_u128(relation.relation_id())),
        AbsorbInput::Constant(F::ZERO),
        word(cells.credential[0]),
        word(cells.credential[1]),
        word(cells.asset[0]),
        word(cells.asset[1]),
        word(cells.lifecycle),
        word(cells.sequence),
        word(cells.next_load),
        word(cells.predecessor),
        word(cells.predecessor_other[0]),
        word(cells.predecessor_other[1]),
        word(cells.successor),
        word(cells.successor_other[0]),
        word(cells.successor_other[1]),
        AbsorbInput::Constant(F::from(relation.effect_tag())),
    ];
    inputs.extend(cells.effect.iter().map(AbsorbInput::Word));
    inputs.resize(STATEMENT_FIELDS, AbsorbInput::Constant(F::ZERO));
    sponge.hash(region, STATEMENT_DOMAIN, &inputs)
}

/// The canonical limbs `(lo, hi)` of a field element `value`.
#[must_use]
pub fn foreign_limbs<G: PastaField>(value: &G) -> [u128; 2] {
    let limbs = value.to_canonical_limbs();
    [
        u128::from(limbs[0]) | (u128::from(limbs[1]) << 64),
        u128::from(limbs[2]) | (u128::from(limbs[3]) << 64),
    ]
}

/// Native reference of [`assign_foreign_scalar`]: the element of `G` whose
/// canonical limbs are `limbs`, or `None` when they encode a value `>=` the
/// modulus.
#[must_use]
pub fn foreign_value_native<G: PastaField>(limbs: [u128; 2]) -> Option<G> {
    let [lo, hi] = limbs;
    let word = |value: u128| u64::try_from(value & u128::from(u64::MAX)).unwrap_or(0);
    let words = [word(lo), word(lo >> 64), word(hi), word(hi >> 64)];
    Option::from(G::from_canonical_limbs(words))
}

/// A cross-field value in its canonical injective encoding (spec S6):
/// `lo < 2^128`, `hi < 2^127` and `lo + 2^128 hi < |G|` for the foreign
/// field `G` it was checked against.
#[derive(Clone, Debug)]
pub struct ForeignScalar<F: PastaField> {
    lo: U128<F>,
    hi: Uint<F, 127>,
}

impl<F: PastaField> ForeignScalar<F> {
    /// The low limb.
    #[must_use]
    pub const fn lo(&self) -> &U128<F> {
        &self.lo
    }

    /// The high limb.
    #[must_use]
    pub const fn hi(&self) -> &Uint<F, 127> {
        &self.hi
    }
}

/// Assigns the limbs of an element of the foreign field `G` and constrains
/// their encoding to be canonical.
///
/// With `m - 1 = M_lo + 2^128 M_hi` (`M_hi = 2^126` for both Pasta fields):
/// `hi <= M_hi`, and when `hi = M_hi`, `lo <= M_lo`. The second condition is
/// the range check of `[hi = M_hi] (M_lo - lo)`.
///
/// # Errors
///
/// [`Error`] from the layout.
pub fn assign_foreign_scalar<F: PastaField, G: PastaField>(
    uint: &mut UintChip<'_, F>,
    region: &mut Region<'_, F>,
    limbs: Value<[u128; 2]>,
) -> Result<ForeignScalar<F>, Error> {
    let [max_lo, max_hi] = foreign_limbs(&-G::ONE);
    let lo = uint.assign::<128>(region, limbs.map(|[lo, _]| lo))?;
    let hi = uint.assign::<127>(region, limbs.map(|[_, hi]| hi))?;
    let max_hi_cell = uint.constant::<127>(region, max_hi)?;
    uint.assert_le(region, &hi, &max_hi_cell)?;
    let glue = uint.glue();
    let offset = glue.add_constant(region, hi.word(), -F::from_u128(max_hi))?;
    let at_max: Bit<F> = glue.is_zero(region, &offset)?;
    let slack = glue.linear(region, &[(-F::ONE, lo.word())], F::from_u128(max_lo))?;
    let bounded = glue.mul(region, at_max.word(), &slack)?;
    uint.range().range_check(region, &bounded, 128)?;
    Ok(ForeignScalar { lo, hi })
}

#[cfg(test)]
mod tests {
    use ff::{Field, PrimeField};
    use iroha_pasta::{Fp, Fq};

    use super::*;

    #[test]
    fn relation_labels() {
        assert_ne!(
            StepRelation::Send.relation_id(),
            StepRelation::Receive.relation_id()
        );
        assert_eq!(StepRelation::Send.effect_tag(), 3);
        assert_eq!(StepRelation::Receive.effect_tag(), 4);
        assert_eq!(STATEMENT_DOMAIN, u64::from_le_bytes(*b"m7stmnt1"));
    }

    #[test]
    fn limbs_split_little_endian_halves() {
        let mut bytes = [0_u8; 32];
        bytes[0] = 1;
        bytes[16] = 2;
        bytes[31] = 0x80;
        assert_eq!(bytes_to_limbs(&bytes), [1, 2 | (0x80 << 120)]);
        assert_eq!(limb_fields::<Fp>([3, 4]), [Fp::from(3u64), Fp::from(4u64)]);
    }

    fn foreign_round_trip<G: PastaField>() {
        let max = -G::ONE;
        let limbs = foreign_limbs(&max);
        assert_eq!(limbs[1], 1 << 126);
        assert_eq!(foreign_value_native::<G>(limbs), Some(max));
        assert_eq!(foreign_value_native::<G>([limbs[0] + 1, limbs[1]]), None);
        assert_eq!(foreign_value_native::<G>([0, 1 << 127]), None);
        let seven = G::from(7u64);
        assert_eq!(
            foreign_value_native::<G>(foreign_limbs(&seven)),
            Some(seven)
        );
    }

    #[test]
    fn foreign_limbs_are_canonical() {
        foreign_round_trip::<Fp>();
        foreign_round_trip::<Fq>();
    }

    #[test]
    fn statement_encoding_layout() {
        let statement = StatementV1::<Fq> {
            relation: StepRelation::Receive,
            scheme_id: [1; 32],
            credential: [2; 32],
            asset: [3; 32],
            lifecycle: 1,
            sequence: 9,
            next_load: 4,
            predecessor: Fq::from(5u64),
            predecessor_other: [6; 32],
            successor: Fq::from(7u64),
            successor_other: [8; 32],
            effect: vec![Fq::from(10u64); 7],
        };
        let fields = statement.encode().expect("encoding");
        assert_eq!(fields[0], Fq::ONE);
        assert_eq!(
            fields[3],
            Fq::from_u128(StepRelation::Receive.relation_id())
        );
        assert_eq!(fields[4], Fq::ZERO);
        assert_eq!(fields[12], Fq::from(5u64));
        assert_eq!(fields[15], Fq::from(7u64));
        assert_eq!(fields[18], Fq::from(4u64));
        assert_eq!(fields[25], Fq::from(10u64));
        assert_eq!(fields[26], Fq::ZERO);
        assert_eq!(
            statement.digest(),
            Some(iroha_pasta::poseidon::hash_with_domain(
                STATEMENT_DOMAIN,
                &fields
            ))
        );
        let mut long = statement;
        long.effect = vec![Fq::ONE; EFFECT_UNION_FIELDS + 1];
        assert_eq!(long.encode(), None);
        assert_eq!(long.digest(), None);
    }
}
