//! One canonical lossless fixed-column size planner for assignment profiles and key storage.

use super::Assigned;
use ff::Field;
use std::io;

pub(crate) const CONSTANT: u8 = 0;
pub(crate) const BITSET: u8 = 1;
pub(crate) const RAW: u8 = 2;
pub(crate) const SPARSE_ZERO: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FixedColumnEncoding {
    pub(crate) mode: u8,
    pub(crate) payload_bytes: u64,
    pub(crate) nonzero: u32,
}

/// Observe actual field values, including equivalent fractions and zero denominators.
/// Counts need no inversions or degree-sized field copy. Zero multiplicities have no effect.
#[derive(Debug)]
pub(crate) struct FixedColumnAccumulator<F: Field> {
    first: Option<Assigned<F>>,
    constant: bool,
    binary: bool,
    rows: Option<u64>,
    nonzero: Option<u64>,
}

impl<F: Field> FixedColumnAccumulator<F> {
    pub(crate) fn new() -> Self {
        Self {
            first: None,
            constant: true,
            binary: true,
            rows: Some(0),
            nonzero: Some(0),
        }
    }

    pub(crate) fn observe(&mut self, value: Assigned<F>, multiplicity: usize) {
        if multiplicity == 0 {
            return;
        }
        let count = u64::try_from(multiplicity).ok();
        self.rows = self.rows.and_then(|rows| rows.checked_add(count?));
        let first = self.first.get_or_insert(value);
        self.constant &= value == *first;
        let zero = value == Assigned::Zero;
        self.binary &= zero || value == Assigned::Trivial(F::ONE);
        if !zero {
            self.nonzero = self.nonzero.and_then(|nonzero| nonzero.checked_add(count?));
        }
    }

    pub(crate) fn constant(&self) -> bool {
        self.constant
    }
    pub(crate) fn binary(&self) -> bool {
        self.binary
    }
    pub(crate) fn nonempty(&self) -> bool {
        self.first.is_some()
    }

    /// Choose the smallest eligible payload; equal byte lengths use the smaller mode tag.
    /// Every column, including constants and binary columns, may use sparse zero or raw.
    pub(crate) fn encoding(&self, scalar_bytes: usize) -> io::Result<FixedColumnEncoding> {
        let invalid =
            || io::Error::new(io::ErrorKind::InvalidData, "invalid structured fixed shape");
        let rows = self
            .rows
            .filter(|rows| *rows != 0 && *rows <= u64::from(u32::MAX))
            .ok_or_else(invalid)?;
        let nonzero = self
            .nonzero
            .filter(|count| *count <= rows)
            .ok_or_else(invalid)?;
        let width = u64::try_from(scalar_bytes)
            .ok()
            .filter(|width| *width != 0)
            .ok_or_else(invalid)?;
        let raw = rows.checked_mul(width).ok_or_else(invalid)?;
        let sparse = width
            .checked_add(4)
            .and_then(|stride| nonzero.checked_mul(stride))
            .and_then(|bytes| bytes.checked_add(4))
            .ok_or_else(invalid)?;
        let mut choice = (raw, RAW);
        for candidate in [
            (sparse, SPARSE_ZERO),
            (width, CONSTANT),
            (rows.div_ceil(8), BITSET),
        ] {
            if (candidate.1 != CONSTANT || self.constant) && (candidate.1 != BITSET || self.binary)
            {
                choice = choice.min(candidate);
            }
        }
        Ok(FixedColumnEncoding {
            mode: choice.1,
            payload_bytes: choice.0,
            nonzero: nonzero as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::halo2curves::pasta::{Fp, Fq};

    fn plan<F: Field>(values: &[(Assigned<F>, usize)]) -> FixedColumnEncoding {
        let mut column = FixedColumnAccumulator::new();
        for &(value, multiplicity) in values {
            column.observe(value, multiplicity);
        }
        column.encoding(32).unwrap()
    }

    fn cases<F: Field>() {
        assert_eq!(
            plan::<F>(&[(Assigned::Zero, 65536)]),
            FixedColumnEncoding {
                mode: SPARSE_ZERO,
                payload_bytes: 4,
                nonzero: 0
            }
        );
        assert_eq!(plan::<F>(&[(Assigned::Trivial(F::ONE), 1)]).mode, BITSET);
        assert_eq!(
            plan::<F>(&[(Assigned::Trivial(F::ONE), 256)]).mode,
            CONSTANT
        ); // 32-byte tie
        assert_eq!(
            plan::<F>(&[(Assigned::Trivial(F::ONE), 255), (Assigned::Zero, 65281)]).mode,
            BITSET
        );
        let three = F::ONE + F::ONE + F::ONE;
        assert_eq!(
            plan::<F>(&[(Assigned::Trivial(three), 255), (Assigned::Zero, 65281)]),
            FixedColumnEncoding {
                mode: SPARSE_ZERO,
                payload_bytes: 9184,
                nonzero: 255
            }
        );
        assert_eq!(
            plan::<F>(&[(Assigned::Trivial(three), 7), (Assigned::Zero, 1)]).mode,
            RAW
        ); // 256-byte tie
        let two = F::ONE + F::ONE;
        assert_eq!(
            plan::<F>(&[
                (Assigned::Rational(three * two, two), 255),
                (Assigned::Rational(F::ONE, F::ZERO), 65281)
            ]),
            plan::<F>(&[(Assigned::Trivial(three), 255), (Assigned::Zero, 65281)])
        );
        assert_eq!(
            plan::<F>(&[(Assigned::Trivial(three), 0), (Assigned::Zero, 1)]).mode,
            BITSET
        );
        let mut empty = FixedColumnAccumulator::<F>::new();
        assert!(empty.encoding(32).is_err());
        empty.observe(Assigned::Zero, 1);
        assert!(empty.encoding(0).is_err());
        if usize::BITS > 32 {
            let mut overflow = FixedColumnAccumulator::<F>::new();
            overflow.observe(Assigned::Zero, u32::MAX as usize);
            overflow.observe(Assigned::Zero, 1);
            assert!(overflow.encoding(32).is_err());
        }
    }

    #[test]
    fn both_fields_choose_exact_smallest_payload_and_stable_ties_from_real_assignments() {
        cases::<Fp>();
        cases::<Fq>();
    }
}
