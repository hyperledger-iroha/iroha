//! Synthetic constant-column proofs for the fixed compact codec and opening tests.
//!
//! This relation exercises the complete required geometry and real polynomial
//! constraints. It is separate from the production transfer AIR and its witnesses.
//! Raw verification tests use an explicit 64 MiB diagnostic allocation policy;
//! the unchanged 32 MiB baseline is exercised as a rejection control.

use std::sync::{Mutex, OnceLock};

use super::{
    CompactProof, FixedAir, FixedAirSchema, Result, VerifyLimits, prove, shape,
    shared_openings::{self, SharedProof},
};
use crate::backend::{add_mod, mul_mod, sub_mod};

const TRACE_ROWS: usize = 65_536;
const WIDTH: usize = 342;
const CONSTRAINTS: usize = 923;

/// Public constant which fixes every current and next row cell.
pub(super) struct FixedColumnsAir {
    pub(super) public: [u8; 8],
}

impl FixedColumnsAir {
    pub(super) fn new(value: u8) -> Self {
        Self {
            public: u64::from(value).to_le_bytes(),
        }
    }

    pub(super) fn row(&self) -> Vec<u64> {
        let first = u64::from_le_bytes(self.public);
        (0..WIDTH)
            .map(|column| add_mod(first, column as u64))
            .collect()
    }

    pub(super) fn columns(&self) -> Vec<Vec<u64>> {
        self.row()
            .into_iter()
            .map(|value| vec![value; TRACE_ROWS])
            .collect()
    }
}

impl FixedAir for FixedColumnsAir {
    fn schema(&self) -> FixedAirSchema {
        FixedAirSchema {
            trace_rows: TRACE_ROWS,
            width: WIDTH,
            constraints: CONSTRAINTS,
            identity: "fastpq:test:constant-columns:rows65536:width342:constraints923:v1",
        }
    }

    fn statement_bytes(&self) -> &[u8] {
        &self.public
    }

    fn evaluate(&self, _: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
        if current.len() != WIDTH || next.len() != WIDTH {
            return Err(shape(
                "constant fixture requires complete current and next rows",
            ));
        }
        let expected = self.row();
        let mut constraints = Vec::with_capacity(CONSTRAINTS);
        constraints.extend(
            current
                .iter()
                .zip(&expected)
                .map(|(&value, &fixed)| sub_mod(value, fixed)),
        );
        constraints.extend(
            next.iter()
                .zip(&expected)
                .map(|(&value, &fixed)| sub_mod(value, fixed)),
        );
        // Fill the remaining independently mixed slots with nonzero multiples
        // of current-cell equalities; no slot bypasses the declared relation.
        for column in 0..CONSTRAINTS - 2 * WIDTH {
            constraints.push(mul_mod(constraints[column], column as u64 + 1));
        }
        Ok(constraints)
    }
}

/// Only bounded public proof objects survive construction of a cached fixture.
pub(super) struct Fixture {
    pub(super) digest: [u8; 8],
    pub(super) compact: CompactProof,
    pub(super) shared: SharedProof,
}

pub(super) fn limits() -> VerifyLimits {
    VerifyLimits {
        max_proof_bytes: 16 * 1024 * 1024,
        max_queries: 375,
        ..VerifyLimits::default()
    }
}

fn build(public: u8) -> Fixture {
    // Each construction expands the full mandatory trace. Concurrent tests
    // share the cached proofs and never expand both fixtures simultaneously.
    static BUILD: Mutex<()> = Mutex::new(());
    let _construction = BUILD.lock().expect("compact fixture builder lock");
    let relation = FixedColumnsAir::new(public);
    let compact = prove(&relation, &FixedColumnsAir::new(7).columns())
        .expect("construct compact fixture through the actual prover");
    let shared = shared_openings::from_compact(&relation, &compact, limits())
        .expect("convert authenticated compact fixture openings");
    Fixture {
        digest: relation.public,
        compact,
        shared,
    }
}

pub(super) fn fixture() -> &'static Fixture {
    static VALID: OnceLock<Fixture> = OnceLock::new();
    VALID.get_or_init(|| build(7))
}

pub(super) fn false_fixture() -> &'static Fixture {
    static FALSE: OnceLock<Fixture> = OnceLock::new();
    FALSE.get_or_init(|| build(8))
}

#[test]
fn constant_fixture_constraints_bind_every_current_and_next_column() {
    let air = FixedColumnsAir::new(7);
    let row = air.row();
    assert_eq!(row.len(), WIDTH);
    assert_eq!(air.schema().trace_rows, TRACE_ROWS);
    assert_eq!(air.statement_bytes(), 7_u64.to_le_bytes());
    let satisfied = air.evaluate(3, &row, &row).unwrap();
    assert_eq!(satisfied, vec![0; CONSTRAINTS]);
    for column in 0..WIDTH {
        let mut changed = row.clone();
        changed[column] = add_mod(changed[column], 1);
        assert_ne!(air.evaluate(3, &changed, &row).unwrap()[column], 0);
        assert_ne!(air.evaluate(3, &row, &changed).unwrap()[WIDTH + column], 0);
    }
    assert!(air.evaluate(3, &row[..WIDTH - 1], &row).is_err());
    assert!(air.evaluate(3, &row, &row[..WIDTH - 1]).is_err());
    assert!(
        FixedColumnsAir::new(8)
            .evaluate(3, &row, &row)
            .unwrap()
            .iter()
            .all(|&constraint| constraint != 0)
    );
    assert_eq!(limits().max_queries, 375);
    assert_eq!(VerifyLimits::default().max_queries, 136);
}
