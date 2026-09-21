//! Test-only full-geometry columns for codec, transcript and opening invariants.
//!
//! This is an explicit polynomial fixture, not HashDigestAir, a public transfer
//! relation, a release witness or evidence of real-relation qualification. All
//! 342 columns and 923 slots are accounted for: 342 current-value equalities,
//! 342 next/current equalities, then 239 independently scaled value equalities.
//! Every numerator has degree below N on degree-below-N trace columns.

use super::*;
use std::sync::OnceLock;

pub(super) const ROWS: usize = 65_536;
pub(super) const WIDTH: usize = 342;
pub(super) const SLOTS: usize = 923;

#[derive(Clone, Debug)]
pub(super) struct FixedColumnsAir {
    pub(super) public: [u8; 32],
}
impl FixedColumnsAir {
    pub(super) fn new(value: u8) -> Self {
        Self {
            public: [value; 32],
        }
    }
    pub(super) fn row(&self) -> Vec<u64> {
        (0..WIDTH)
            .map(|column| u64::from(self.public[0]) + column as u64)
            .collect()
    }
    pub(super) fn columns(&self) -> Vec<Vec<u64>> {
        self.row()
            .into_iter()
            .map(|value| vec![value; ROWS])
            .collect()
    }
}
impl FixedAir for FixedColumnsAir {
    fn schema(&self) -> FixedAirSchema {
        FixedAirSchema {
            trace_rows: ROWS,
            width: WIDTH,
            constraints: SLOTS,
            identity: "test-only:fixed-columns-and-next-equality:342:923:v1",
        }
    }
    fn statement_bytes(&self) -> &[u8] {
        &self.public
    }
    fn evaluate(&self, _: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
        if current.len() != WIDTH || next.len() != WIDTH {
            return Err(shape(
                "test fixed AIR requires complete 342-column current and next rows",
            ));
        }
        for (index, &value) in current.iter().chain(next).enumerate() {
            canonical_base(value, "fixed_column_fixture", &[index])?;
        }
        let differences = current
            .iter()
            .zip(self.row())
            .map(|(&value, expected)| crate::backend::sub_mod(value, expected))
            .collect::<Vec<_>>();
        let mut slots = Vec::with_capacity(SLOTS);
        slots.extend(differences.iter().copied());
        slots.extend(
            next.iter()
                .zip(current)
                .map(|(&next, &current)| crate::backend::sub_mod(next, current)),
        );
        slots.extend(
            differences
                .iter()
                .take(SLOTS - 2 * WIDTH)
                .enumerate()
                .map(|(index, &difference)| {
                    crate::backend::mul_mod(difference, (index + 2) as u64)
                }),
        );
        Ok(slots)
    }
}

/// Explicit test policy for the final full-row DTO; production defaults stay intact.
pub(super) fn limits() -> VerifyLimits {
    VerifyLimits {
        max_proof_bytes: 16 * 1024 * 1024,
        max_queries: 375,
        ..VerifyLimits::default()
    }
}

pub(super) struct Fixture {
    pub(super) digest: [u8; 32],
    pub(super) compact: CompactProof,
    pub(super) shared: shared_openings::SharedProof,
}
pub(super) fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let air = FixedColumnsAir::new(7);
        let columns = air.columns();
        let prepared = prepare_trace(&air, &columns).unwrap();
        let compact = prove_prepared(&air, &prepared).unwrap();
        // Complete public context is part of every row root. Cached trace reuse
        // under another statement must reject before deriving a new challenge.
        assert!(prove_prepared(&FixedColumnsAir::new(8), &prepared).is_err());
        let shared = shared_openings::from_compact(&air, &compact, limits()).unwrap();
        shared_openings::verify_shared(&air, &shared, limits()).unwrap();
        Fixture {
            digest: air.public,
            compact,
            shared,
        }
    })
}
pub(super) fn false_fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let air = FixedColumnsAir::new(8);
        let compact = prove(&air, &FixedColumnsAir::new(7).columns()).unwrap();
        let shared = shared_openings::from_compact(&air, &compact, limits()).unwrap();
        assert_ne!(compact.row_root, fixture().compact.row_root);
        assert_ne!(compact.mixed_root, fixture().compact.mixed_root);
        Fixture {
            digest: air.public,
            compact,
            shared,
        }
    })
}

#[test]
fn fixed_fixture_accounts_for_every_column_slot_and_canonical_row() {
    let air = FixedColumnsAir::new(7);
    let row = air.row();
    let slots = air.evaluate(19, &row, &row).unwrap();
    assert_eq!(slots.len(), SLOTS);
    assert!(slots.iter().all(|&value| value == 0));
    for column in 0..WIDTH {
        let mut changed = row.clone();
        changed[column] += 1;
        let current = air.evaluate(19, &changed, &row).unwrap();
        assert_eq!(current[column], 1);
        assert_eq!(current[WIDTH + column], GOLDILOCKS_MODULUS - 1);
        if column < SLOTS - 2 * WIDTH {
            assert_eq!(current[2 * WIDTH + column], (column + 2) as u64);
        }
        let next = air.evaluate(19, &row, &changed).unwrap();
        assert_eq!(next[WIDTH + column], 1);
        assert_eq!(next.iter().filter(|&&v| v != 0).count(), 1);
        changed[column] = GOLDILOCKS_MODULUS;
        assert!(air.evaluate(19, &changed, &row).is_err());
        assert!(air.evaluate(19, &row, &changed).is_err());
    }
    assert!(air.evaluate(19, &row[..WIDTH - 1], &row).is_err());
    assert!(air.evaluate(19, &row, &row[..WIDTH - 1]).is_err());
}
