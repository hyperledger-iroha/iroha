//! The prototype G1 statement encoding (split lineage, owner approval
//! pending; not a protocol format): the in-circuit digest equals the native
//! `StatementV1::digest` on both parities and both relations, the
//! other-parity limbs go through the canonical S6 encoding (values at or
//! above the foreign modulus are unsatisfiable, so `s >= p` cannot pass as
//! `s mod p`), plus the tamper suite and the inventory (17 folded blocks).

mod common;

use common::{
    Chips, GadgetCircuit, Inputs, RANGE_COLUMN, Shape, accepts, assigned, extent, lane_columns,
    report,
};
use ff::Field as _;
use iroha_pasta::{Fp, Fq, PastaField, poseidon::PoseidonField};
use iroha_plonk::{
    check::{CheckFailure, CheckMode, check_circuit},
    frontend::{Error, Region, configure, synthesize},
};
use iroha_plonk_gadgets::{
    MAX_GATE_DEGREE, UintChip, Word,
    cells::low_u128,
    poseidon::ROWS_PER_PERMUTATION,
    statement::{
        STATEMENT_DOMAIN, STATEMENT_FIELDS, StatementCells, StatementV1, StepRelation,
        assign_foreign_scalar, bytes_to_limbs, foreign_limbs, foreign_value_native, limb_fields,
        statement_digest,
    },
    tamper::{Tamper, assigned_advice_cells, check_tampered},
};

/// Plain statement fields before the effect: scheme (2), credential (2),
/// asset (2), lifecycle, sequence, next load, predecessor, successor.
const PLAIN: usize = 11;
/// `k` of the statement circuits (17 folded blocks, 9-bit limbs).
const K: u32 = 10;

/// Inputs: the plain fields, the effect (`arg 1` fields), then the
/// predecessor's and successor's other-parity limbs. `arg 0` selects the
/// relation (0 Send, 1 Receive).
fn statement_program<F: PoseidonField, G: PastaField>(
    chips: &mut Chips<F>,
    region: &mut Region<'_, F>,
    inputs: &Inputs<F>,
) -> Result<Vec<Word<F>>, Error> {
    let effect = usize::try_from(inputs.arg(1)).map_err(|_| Error::Synthesis)?;
    let plain = (0..PLAIN + effect)
        .map(|i| inputs.get(i))
        .collect::<Vec<_>>();
    let words = chips.glue.witnesses(region, &plain)?;
    let limbs = |first: usize| {
        inputs
            .get(first)
            .zip(inputs.get(first + 1))
            .map(|(lo, hi)| [low_u128(&lo), low_u128(&hi)])
    };
    let mut uint = UintChip::new(&mut chips.glue, &mut chips.range);
    let predecessor_other =
        assign_foreign_scalar::<F, G>(&mut uint, region, limbs(PLAIN + effect))?;
    let successor_other =
        assign_foreign_scalar::<F, G>(&mut uint, region, limbs(PLAIN + effect + 2))?;
    let cells = StatementCells {
        scheme_id: [&words[0], &words[1]],
        credential: [&words[2], &words[3]],
        asset: [&words[4], &words[5]],
        lifecycle: &words[6],
        sequence: &words[7],
        next_load: &words[8],
        predecessor: &words[9],
        predecessor_other: [predecessor_other.lo().word(), predecessor_other.hi().word()],
        successor: &words[10],
        successor_other: [successor_other.lo().word(), successor_other.hi().word()],
        effect: &words[PLAIN..],
    };
    let relation = if inputs.arg(0) == 0 {
        StepRelation::Send
    } else {
        StepRelation::Receive
    };
    Ok(vec![statement_digest(
        &mut chips.sponges[0],
        region,
        relation,
        &cells,
    )?])
}

/// A statement with distinct field values; the other-parity components are
/// the canonical encodings of `G` elements unless overridden.
fn sample_statement<F: PoseidonField, G: PastaField>(
    relation: StepRelation,
    effect: usize,
) -> StatementV1<F> {
    let bytes = |seed: u8| {
        core::array::from_fn(|i| {
            seed.wrapping_mul(31)
                .wrapping_add(u8::try_from(i).unwrap_or(0))
        })
    };
    StatementV1 {
        relation,
        scheme_id: bytes(1),
        credential: bytes(2),
        asset: bytes(3),
        lifecycle: 1,
        sequence: (1 << 70) + 3,
        next_load: 12,
        predecessor: F::from(0xdead_beef_u64),
        predecessor_other: (-G::from(5u64)).to_repr(),
        successor: -F::from(77u64),
        successor_other: G::from(1_000_003u64).to_repr(),
        effect: (0..effect).map(|i| F::from(100 + i as u64)).collect(),
    }
}

/// The circuit of `statement` and its public digest.
fn circuit<F: PoseidonField, G: PastaField>(
    statement: &StatementV1<F>,
) -> (GadgetCircuit<F>, Vec<F>) {
    let limbs = |bytes: &[u8; 32]| limb_fields::<F>(bytes_to_limbs(bytes));
    let mut inputs = Vec::new();
    for bytes in [
        &statement.scheme_id,
        &statement.credential,
        &statement.asset,
    ] {
        inputs.extend(limbs(bytes));
    }
    inputs.extend([
        F::from(statement.lifecycle),
        F::from_u128(statement.sequence),
        F::from_u128(statement.next_load),
        statement.predecessor,
        statement.successor,
    ]);
    inputs.extend(statement.effect.iter().copied());
    inputs.extend(limbs(&statement.predecessor_other));
    inputs.extend(limbs(&statement.successor_other));
    let relation = u64::from(statement.relation == StepRelation::Receive);
    let effect = u64::try_from(statement.effect.len()).expect("effect length");
    let shape = Shape::new(1, 9, 1)
        .with_args(&[relation, effect])
        .folding(&[(STATEMENT_DOMAIN, STATEMENT_FIELDS)]);
    let digest = statement.digest().expect("at most 13 effect fields");
    (
        GadgetCircuit::new(shape, statement_program::<F, G>, inputs),
        vec![digest],
    )
}

fn digests_match<F: PoseidonField, G: PastaField>() {
    for (relation, effect) in [(StepRelation::Send, 13), (StepRelation::Receive, 7)] {
        let statement = sample_statement::<F, G>(relation, effect);
        // The other-parity limbs are canonical encodings of G elements.
        let limbs = bytes_to_limbs(&statement.predecessor_other);
        assert_eq!(limbs, foreign_limbs(&-G::from(5u64)));
        assert!(foreign_value_native::<G>(limbs).is_some());
        let (circuit, public) = circuit::<F, G>(&statement);
        assert!(
            accepts(&circuit, K, &public),
            "{}",
            report(&circuit, K, &public)
        );
        let mut wrong = public.clone();
        wrong[0] += F::ONE;
        assert!(!accepts(&circuit, K, &wrong));
    }
}

#[test]
fn in_circuit_digest_equals_the_native_statement_digest() {
    digests_match::<Fp, Fq>();
    digests_match::<Fq, Fp>();
}

/// Whether the strict check fails, and only through range lookups.
fn only_range_failures<F: PoseidonField>(circuit: &GadgetCircuit<F>, public: &[F]) -> bool {
    let report = check_circuit(circuit, K, &[public.to_vec()], CheckMode::Strict).expect("check");
    !report.is_satisfied()
        && report
            .failures()
            .iter()
            .all(|failure| matches!(failure, CheckFailure::LookupInputMissing { .. }))
}

fn non_canonical_limbs_are_unsatisfiable<F: PoseidonField, G: PastaField>() {
    let [max_lo, max_hi] = foreign_limbs(&-G::ONE);
    let encode = |[lo, hi]: [u128; 2]| {
        let mut bytes = [0_u8; 32];
        bytes[..16].copy_from_slice(&lo.to_le_bytes());
        bytes[16..].copy_from_slice(&hi.to_le_bytes());
        bytes
    };
    // The modulus itself, the modulus + 5 (whose reduction is 5: `s >= p`
    // passed in place of `s mod p`), a high limb above the modulus's, and a
    // high limb of 2^127.
    for limbs in [
        [max_lo + 1, max_hi],
        [max_lo + 6, max_hi],
        [0, max_hi + 1],
        [u128::MAX, max_hi],
        [0, 1 << 127],
    ] {
        assert_eq!(foreign_value_native::<G>(limbs), None);
        let mut statement = sample_statement::<F, G>(StepRelation::Send, 4);
        statement.successor_other = encode(limbs);
        let (circuit, public) = circuit::<F, G>(&statement);
        assert!(only_range_failures(&circuit, &public), "limbs {limbs:x?}");
    }
    // The largest canonical value is accepted.
    let mut statement = sample_statement::<F, G>(StepRelation::Send, 4);
    statement.successor_other = encode([max_lo, max_hi]);
    let (circuit, public) = circuit::<F, G>(&statement);
    assert!(
        accepts(&circuit, K, &public),
        "{}",
        report(&circuit, K, &public)
    );
}

#[test]
fn s6_non_canonical_foreign_limbs_are_unsatisfiable() {
    non_canonical_limbs_are_unsatisfiable::<Fp, Fq>();
    non_canonical_limbs_are_unsatisfiable::<Fq, Fp>();
}

#[test]
fn an_effect_longer_than_13_fields_is_an_error() {
    let statement = sample_statement::<Fp, Fq>(StepRelation::Send, 14);
    assert_eq!(statement.digest(), None);
    let short = sample_statement::<Fp, Fq>(StepRelation::Send, 13);
    let (mut circuit, public) = circuit::<Fp, Fq>(&short);
    circuit.shape.args[1] = 14;
    circuit.inputs.insert(PLAIN + 13, Fp::ONE);
    assert_eq!(
        synthesize(&circuit, K, Some(&[public][..])).map(|_| ()),
        Err(Error::Synthesis)
    );
}

#[test]
fn inventory_17_blocks_and_the_s6_checks() {
    let statement = sample_statement::<Fp, Fq>(StepRelation::Receive, 7);
    let (circuit, public) = circuit::<Fp, Fq>(&statement);
    let flags = assigned(&circuit, K, &public);
    // The folded statement digest: 17 blocks.
    assert_eq!(
        extent(&flags[lane_columns(0)[0]]),
        17 * ROWS_PER_PERMUTATION
    );
    // Each foreign scalar: four range checks (lo 128, hi 127, the bound of
    // hi 127, the conditional bound of lo 128) of 16 rows at b = 9.
    assert_eq!(extent(&flags[RANGE_COLUMN]), 2 * 4 * 16);
    // Every gate of the composite circuit keeps the degree policy, and the
    // circuit degree (lookups included) is exactly the policy bound.
    let (cs, _) = configure(&circuit).expect("configure");
    for gate in cs.gates() {
        for poly in gate.polynomials() {
            assert!(poly.degree() <= MAX_GATE_DEGREE, "gate {}", gate.name());
        }
    }
    assert_eq!(cs.degree(), MAX_GATE_DEGREE);
}

/// Tampers `cells`; returns those the checker accepted.
fn undetected<F: PoseidonField>(
    circuit: &GadgetCircuit<F>,
    public: &[F],
    cells: &[(usize, usize)],
) -> Vec<(usize, usize)> {
    cells
        .iter()
        .copied()
        .filter(|(column, row)| {
            let tamper = Tamper {
                column: *column,
                row: *row,
                delta: F::ONE,
            };
            check_tampered(circuit, K, &[public.to_vec()], Some(tamper))
                .expect("check")
                .is_satisfied()
        })
        .collect()
}

#[test]
fn glue_range_and_boundary_lane_cells_are_pinned() {
    // Every glue and range cell, and every lane cell of the first and last
    // blocks (the lane suites tamper every block of smaller sponges).
    let statement = sample_statement::<Fq, Fp>(StepRelation::Send, 13);
    let (circuit, public) = circuit::<Fq, Fp>(&statement);
    let lane = lane_columns(0);
    let cells = assigned_advice_cells(&circuit, K, std::slice::from_ref(&public))
        .expect("cells")
        .into_iter()
        .filter(|(column, row)| {
            !lane.contains(column)
                || *row < ROWS_PER_PERMUTATION
                || *row >= 16 * ROWS_PER_PERMUTATION
        })
        .collect::<Vec<_>>();
    assert_eq!(undetected(&circuit, &public, &cells), Vec::new());
}

#[test]
#[ignore = "every cell of the 17-block statement circuit; run in release"]
fn every_statement_cell_is_pinned() {
    let statement = sample_statement::<Fp, Fq>(StepRelation::Receive, 7);
    let (circuit, public) = circuit::<Fp, Fq>(&statement);
    let cells = assigned_advice_cells(&circuit, K, std::slice::from_ref(&public)).expect("cells");
    assert_eq!(undetected(&circuit, &public, &cells), Vec::new());
}
