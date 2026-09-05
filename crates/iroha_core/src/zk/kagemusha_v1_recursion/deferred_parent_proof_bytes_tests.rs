//! Canonical proof-byte regressions over the actual deferred Poseidon transcript.
//!
//! These tests exercise the scalar-half parser and its assigned byte-encoding relation, not a
//! complete ordinary proof, reciprocal curve audit, mint authorization, or hardware profile.
//! Proof items below are deliberately small transcript fixtures, never fabricated monetary proofs.

use std::collections::BTreeSet;

use ff::{Field as _, PrimeField as _};
use halo2_base::{ContextCell, ContextTag};
use halo2_proofs::{
    dev::{MockProver, VerifyFailure},
    halo2curves::{
        group::Curve as _,
        pasta::{EpAffine, EqAffine},
    },
};
use snark_verifier::util::transcript::{Transcript as _, TranscriptRead as _};

use super::*;
use crate::zk::pasta_sha256::PastaSha256ByteV1;

const PROOF_BYTES_TEST_K: usize = 12;
const PROOF_BYTES_LOOKUP_BITS: usize = 8;

#[test]
fn ordinary_parser_bounds_individual_and_total_release_challenges() {
    assert_eq!(
        super::proof_bytes::validate_ordinary_challenge_profile_v1(&[1, 2, 1])
            .expect("the fixed release challenge profile"),
        4
    );
    assert!(
        super::proof_bytes::validate_ordinary_challenge_profile_v1(&[3, 0, 0]).is_err(),
        "one phase cannot exceed the release maximum even when its total is small"
    );
    assert!(
        super::proof_bytes::validate_ordinary_challenge_profile_v1(&[2, 2, 1]).is_err(),
        "individually bounded phases cannot exceed the aggregate release maximum"
    );
    assert!(
        super::proof_bytes::validate_ordinary_challenge_profile_v1(&[usize::MAX]).is_err(),
        "an attacker-controlled count is rejected before transcript allocation"
    );
}

#[test]
fn hybrid_parser_bounds_carrier_by_authenticated_lagrange_capacity() {
    let capacity = 1_usize << KAGEMUSHA_RECURSION_IPA_K_V1;
    super::proof_bytes::validate_hybrid_carrier_lagrange_capacity_v1(capacity, capacity)
        .expect("the final authenticated Lagrange base remains usable");
    assert!(
        super::proof_bytes::validate_hybrid_carrier_lagrange_capacity_v1(capacity + 1, capacity)
            .is_err(),
        "a carrier wider than the authenticated SRS cannot enter recursive parsing"
    );
}

#[test]
fn hybrid_parser_accounts_for_each_proof_supplied_commitment() {
    assert_eq!(
        super::proof_bytes::hybrid_proof_supplied_commitment_bytes_v1(1)
            .expect("one compressed Pasta point"),
        32
    );
    assert_eq!(
        super::proof_bytes::hybrid_proof_supplied_commitment_bytes_v1(2)
            .expect("two compressed Pasta points"),
        64
    );
    assert!(
        super::proof_bytes::hybrid_proof_supplied_commitment_bytes_v1(usize::MAX).is_err(),
        "proof framing rejects a commitment-count overflow"
    );
}

#[test]
fn hybrid_parser_requires_canonical_commitment_limb_order() {
    let validate = |pairs: &[[usize; 2]], expected| {
        super::proof_bytes::validate_hybrid_commitment_limb_indices_v1(8, pairs, expected)
    };
    validate(&[[2, 3]], 1).expect("legacy one-carrier binding");
    validate(&[[2, 3], [4, 5]], 2).expect("claim two-carrier binding");

    for (pairs, expected) in [
        (&[][..], 1),
        (&[[2, 3], [4, 5]][..], 1),
        (&[[2, 3]][..], 2),
        (&[[3, 2], [4, 5]][..], 2),
        (&[[4, 5], [2, 3]][..], 2),
        (&[[2, 3], [5, 6]][..], 2),
        (&[[2, 3], [3, 4]][..], 2),
        (&[[4, 5], [7, 8]][..], 2),
    ] {
        assert!(
            validate(pairs, expected).is_err(),
            "missing, extra, reversed, swapped, overlapping, gapped, or out-of-range limbs must fail"
        );
    }
}

#[derive(Clone, Copy)]
enum ReadItem {
    Scalar,
    Point,
}

const MIXED_ITEMS: [ReadItem; 4] = [
    ReadItem::Scalar,
    ReadItem::Point,
    ReadItem::Scalar,
    ReadItem::Point,
];

struct CapturedFixture<F: BigPrimeField> {
    builder: BaseCircuitBuilder<F>,
    scalar_sources: Vec<AssignedValue<F>>,
}

fn mixed_bytes<C>(first: C::ScalarExt, second: C::ScalarExt, multiple: u64) -> Vec<u8>
where
    C: CurveAffineExt,
{
    let point = (C::generator() * C::ScalarExt::from(multiple)).to_affine();
    let opposite = (-point.to_curve()).to_affine();
    let mut bytes = Vec::with_capacity(128);
    bytes.extend_from_slice(first.to_repr().as_ref());
    bytes.extend_from_slice(point.to_bytes().as_ref());
    bytes.extend_from_slice(second.to_repr().as_ref());
    bytes.extend_from_slice(opposite.to_bytes().as_ref());
    bytes
}

fn captured_fixture<C>(
    raw: &[u8],
    items: &[ReadItem],
    expected_byte_len: usize,
    common_inputs: bool,
) -> Result<CapturedFixture<C::ScalarExt>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let mut builder = BaseCircuitBuilder::<C::ScalarExt>::default()
        .use_k(PROOF_BYTES_TEST_K)
        .use_lookup_bits(PROOF_BYTES_LOOKUP_BITS)
        .use_instance_columns(1);
    let range = builder.range_chip();
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    let (reader, position) = ExactReader::new(raw);
    let mut transcript =
        DeferredTranscript::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(&loader, reader);
    let mut scalar_sources = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if common_inputs {
            let public_scalar = loader.assign_scalar(C::ScalarExt::from(100 + index as u64));
            let public_point = loader.assign_ec_point(C::generator());
            transcript.common_scalar(&public_scalar)?;
            transcript.common_ec_point(&public_point)?;
            let _challenge = transcript.squeeze_challenge();
            assert_eq!(
                transcript.loaded_stream.len(),
                index,
                "public inputs, constants and challenges are not proof-read items"
            );
        }
        match item {
            ReadItem::Scalar => {
                let scalar = transcript.read_scalar()?;
                scalar_sources.push(*scalar.assigned());
            }
            ReadItem::Point => {
                let _point = transcript.read_ec_point()?;
            }
        }
        assert_eq!(transcript.loaded_stream.len(), index + 1);
    }
    // The real ordinary-proof entry point also requires exact reader exhaustion. In particular,
    // a correct prefix cannot make an unused suffix part of the assigned proof representation.
    if position.get() != raw.len() {
        return Err(transcript_error("proof-byte fixture has trailing bytes"));
    }
    let bytes =
        canonical_loaded_proof_bytes_v1(&loader, &transcript.loaded_stream, expected_byte_len)?;
    assert_eq!(bytes.len(), expected_byte_len);
    assert_eq!(
        bytes
            .iter()
            .copied()
            .map(PastaSha256ByteV1::test_value)
            .collect::<Vec<_>>(),
        raw,
        "canonical assigned encodings reproduce the exact bytes read"
    );
    let output = bytes
        .iter()
        .map(|byte| byte.assigned().expect("proof bytes retain cell identity"))
        .collect();
    drop(transcript);
    *builder.pool(0) = loader.take_ctx();
    builder.assigned_instances = vec![output];
    builder.calculate_params(Some(9));
    Ok(CapturedFixture {
        builder,
        scalar_sources,
    })
}

fn verify_bytes<F: BigPrimeField>(
    builder: &BaseCircuitBuilder<F>,
    expected: &[u8],
) -> Result<(), Vec<VerifyFailure>> {
    let public = expected
        .iter()
        .map(|byte| F::from(u64::from(*byte)))
        .collect();
    MockProver::run(PROOF_BYTES_TEST_K as u32, builder, vec![public])
        .expect("deferred proof-byte mock prover")
        .verify()
}

fn assert_exact_mixed_bytes<C>()
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    for (first, second, multiple) in [
        (C::ScalarExt::ZERO, C::ScalarExt::ONE, 1),
        (C::ScalarExt::ONE, -C::ScalarExt::ONE, 7),
        (C::ScalarExt::from(0x1234), C::ScalarExt::from(0x5678), 13),
    ] {
        let raw = mixed_bytes::<C>(first, second, multiple);
        assert_eq!(raw.len(), 128);
        let fixture = captured_fixture::<C>(&raw, &MIXED_ITEMS, raw.len(), false)
            .expect("canonical mixed read sequence");
        verify_bytes(&fixture.builder, &raw).expect("exact transcript-byte relation");
    }
}

#[test]
fn proof_bytes_preserve_mixed_read_order_and_canonical_encodings_in_both_parities() {
    assert_exact_mixed_bytes::<EqAffine>();
    assert_exact_mixed_bytes::<EpAffine>();
}

fn assert_common_inputs_excluded<C>()
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let raw = mixed_bytes::<C>(C::ScalarExt::from(23), C::ScalarExt::from(41), 5);
    let plain = captured_fixture::<C>(&raw, &MIXED_ITEMS, raw.len(), false)
        .expect("plain proof-read transcript");
    let common = captured_fixture::<C>(&raw, &MIXED_ITEMS, raw.len(), true)
        .expect("proof reads interleaved with public-input absorption");
    assert_eq!(
        plain.builder.assigned_instances[0].len(),
        common.builder.assigned_instances[0].len()
    );
    verify_bytes(&plain.builder, &raw).expect("plain exact bytes");
    verify_bytes(&common.builder, &raw).expect("public inputs do not contaminate proof bytes");
}

#[test]
fn proof_bytes_exclude_common_inputs_and_challenges_in_both_parities() {
    assert_common_inputs_excluded::<EqAffine>();
    assert_common_inputs_excluded::<EpAffine>();
}

fn assert_substitution_rejected<C>()
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let raw = mixed_bytes::<C>(C::ScalarExt::from(23), C::ScalarExt::from(41), 5);
    let fixture =
        captured_fixture::<C>(&raw, &MIXED_ITEMS, raw.len(), false).expect("canonical transcript");
    let mut scalar_swap = raw.clone();
    scalar_swap[..32].copy_from_slice(C::ScalarExt::from(24).to_repr().as_ref());
    let point_swap = mixed_bytes::<C>(C::ScalarExt::from(23), C::ScalarExt::from(41), 7);
    let mut sign_swap = raw.clone();
    sign_swap[63] ^= 0x80;
    let mut reordered = raw.clone();
    reordered[..64].copy_from_slice(&raw[64..]);
    reordered[64..].copy_from_slice(&raw[..64]);
    let mut byte_swap = raw.clone();
    byte_swap[0] ^= 0x40;
    for different in [scalar_swap, point_swap, sign_swap, reordered, byte_swap] {
        assert!(
            verify_bytes(&fixture.builder, &different).is_err(),
            "a separately supplied scalar, point, sign, ordering or byte is not the read stream"
        );
    }
}

#[test]
fn proof_bytes_reject_scalar_point_sign_order_and_byte_substitution_in_both_parities() {
    assert_substitution_rejected::<EqAffine>();
    assert_substitution_rejected::<EpAffine>();
}

/// Change every copied Base/lookup value, avoiding rejection solely from a stale copy cache.
fn replace_equivalence_class<F: BigPrimeField>(
    builder: &mut BaseCircuitBuilder<F>,
    target: AssignedValue<F>,
    replacement: F,
) {
    assert_eq!(builder.lookup_bits(), Some(PROOF_BYTES_LOOKUP_BITS));
    let target = target.cell.expect("assigned mutation target");
    let equalities = builder
        .core()
        .copy_manager
        .lock()
        .expect("copy manager")
        .advice_equalities
        .clone();
    let mut cells = BTreeSet::from([target]);
    loop {
        let previous = cells.len();
        for (left, right) in &equalities {
            if cells.contains(left) || cells.contains(right) {
                cells.insert(*left);
                cells.insert(*right);
            }
        }
        if previous == cells.len() {
            break;
        }
    }
    for cell in &cells {
        assert_eq!(cell.type_id(), target.type_id());
        assert_eq!(cell.context_id(), 0);
        builder
            .main(0)
            .replace_advice_with_trivial(cell.offset(), replacement);
    }
    let replacement = builder
        .main(0)
        .get(isize::try_from(target.offset()).expect("mutation offset"))
        .value;
    for manager in builder.lookup_manager() {
        let mut lookups = manager.cells_to_lookup.lock().expect("lookup cells");
        for row in lookups.values_mut().flatten() {
            for lookup in row {
                if lookup.cell.is_some_and(|cell| cells.contains(&cell)) {
                    lookup.value = replacement;
                }
            }
        }
    }
}

fn assert_coordinated_byte_mutations<C>()
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let raw = mixed_bytes::<C>(C::ScalarExt::from(23), C::ScalarExt::from(41), 5);
    // Cover scalar bytes, point coordinate bytes and each point's compressed sign bit. Both
    // output witnesses and public outputs are changed, leaving their defining relation to fail.
    for index in [0, 31, 32, 63, 64, 95, 127] {
        let mut fixture = captured_fixture::<C>(&raw, &MIXED_ITEMS, raw.len(), false)
            .expect("canonical transcript");
        let mut changed = raw.clone();
        changed[index] ^= 0x80;
        let target = fixture.builder.assigned_instances[0][index];
        replace_equivalence_class(
            &mut fixture.builder,
            target,
            C::ScalarExt::from(u64::from(changed[index])),
        );
        let failures = verify_bytes(&fixture.builder, &changed)
            .expect_err("coordinated byte substitution must fail encoding constraints");
        assert!(
            failures
                .iter()
                .any(|failure| matches!(failure, VerifyFailure::ConstraintNotSatisfied { .. })),
            "expected a defining arithmetic failure, not only a copy/instance failure: {failures:?}"
        );
    }
}

#[test]
fn proof_bytes_reject_coordinated_serialized_byte_mutations_in_both_parities() {
    assert_coordinated_byte_mutations::<EqAffine>();
    assert_coordinated_byte_mutations::<EpAffine>();
}

fn assert_source_scalar_mutation<C>()
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let raw = C::ScalarExt::from(23).to_repr().as_ref().to_vec();
    let mut fixture = captured_fixture::<C>(&raw, &[ReadItem::Scalar], 32, false)
        .expect("single scalar transcript");
    assert_eq!(fixture.scalar_sources.len(), 1);
    let source = fixture.scalar_sources[0];
    replace_equivalence_class(&mut fixture.builder, source, C::ScalarExt::from(24));
    // There is no challenge squeeze or curve equation in this fixture. The unchanged bytes
    // must remain equality-bound to the very scalar cell read by the verifier, not its host value.
    let failures = verify_bytes(&fixture.builder, &raw)
        .expect_err("detaching a proof scalar from its serialized bytes must fail");
    assert!(
        failures
            .iter()
            .any(|failure| matches!(failure, VerifyFailure::ConstraintNotSatisfied { .. }))
    );
}

#[test]
fn proof_bytes_remain_bound_to_original_scalar_cells_in_both_parities() {
    assert_source_scalar_mutation::<EqAffine>();
    assert_source_scalar_mutation::<EpAffine>();
}

fn assert_malformed_encodings<C>()
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    assert!(captured_fixture::<C>(&[0xff; 32], &[ReadItem::Scalar], 32, false).is_err());
    let mut modulus = (-C::ScalarExt::ONE).to_repr().as_ref().to_vec();
    for byte in &mut modulus {
        let (value, carry) = byte.overflowing_add(1);
        *byte = value;
        if !carry {
            break;
        }
    }
    assert!(captured_fixture::<C>(&modulus, &[ReadItem::Scalar], 32, false).is_err());
    assert!(captured_fixture::<C>(&[0xff; 32], &[ReadItem::Point], 32, false).is_err());
    assert!(
        captured_fixture::<C>(
            C::identity().to_bytes().as_ref(),
            &[ReadItem::Point],
            32,
            false,
        )
        .is_err()
    );
    let raw = mixed_bytes::<C>(C::ScalarExt::ZERO, C::ScalarExt::ONE, 1);
    for length in [0, 1, 31, 32, 63, 64, 95, 96, 127] {
        assert!(captured_fixture::<C>(&raw[..length], &MIXED_ITEMS, raw.len(), false).is_err());
    }
    let mut trailing = raw.clone();
    trailing.push(0);
    assert!(captured_fixture::<C>(&trailing, &MIXED_ITEMS, raw.len(), false).is_err());
}

#[test]
fn proof_bytes_reject_noncanonical_malformed_identity_and_inexact_inputs_in_both_parities() {
    assert_malformed_encodings::<EqAffine>();
    assert_malformed_encodings::<EpAffine>();
}

fn assert_inventory_mismatch<C>()
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let raw = mixed_bytes::<C>(C::ScalarExt::ZERO, C::ScalarExt::ONE, 1);
    for expected in [0, 1, 32, 96, 127, 129, 160, usize::MAX] {
        assert!(captured_fixture::<C>(&raw, &MIXED_ITEMS, expected, false).is_err());
    }
}

#[test]
fn proof_bytes_reject_wrong_exact_inventory_in_both_parities() {
    assert_inventory_mismatch::<EqAffine>();
    assert_inventory_mismatch::<EpAffine>();
}

#[derive(Debug, PartialEq, Eq)]
struct ProofByteShape<F: BigPrimeField> {
    k: usize,
    advice_columns: Vec<usize>,
    fixed_columns: usize,
    lookup_columns: Vec<usize>,
    lookup_bits: Option<usize>,
    instance_columns: usize,
    advice_rows: Vec<usize>,
    selectors: Vec<Vec<bool>>,
    advice_equalities: Vec<(ContextCell, ContextCell)>,
    constant_equalities: Vec<(F, ContextCell)>,
    lookup_cells: Vec<(usize, ContextTag, ContextCell)>,
    instance_cells: Vec<Vec<ContextCell>>,
}

fn proof_byte_shape<F: BigPrimeField>(builder: &BaseCircuitBuilder<F>) -> ProofByteShape<F> {
    let params = &builder.config_params;
    let mut advice_rows = Vec::new();
    let mut selectors = Vec::new();
    for phase in &builder.core().phase_manager {
        for ctx in &phase.threads {
            advice_rows.push(ctx.advice_len());
            selectors.push(ctx.selector.iter().copied().collect());
        }
    }
    let mut lookup_cells = Vec::new();
    for (phase, manager) in builder.lookup_manager().iter().enumerate() {
        let lookups = manager.cells_to_lookup.lock().expect("lookup cells");
        for (tag, rows) in lookups.iter() {
            for row in rows {
                for value in row {
                    lookup_cells.push((phase, *tag, value.cell.expect("lookup position")));
                }
            }
        }
    }
    let copies = builder
        .core()
        .copy_manager
        .lock()
        .expect("copy constraints");
    ProofByteShape {
        k: params.k,
        advice_columns: params.num_advice_per_phase.clone(),
        fixed_columns: params.num_fixed,
        lookup_columns: params.num_lookup_advice_per_phase.clone(),
        lookup_bits: params.lookup_bits,
        instance_columns: params.num_instance_columns,
        advice_rows,
        selectors,
        advice_equalities: copies.advice_equalities.clone(),
        constant_equalities: copies
            .constant_equalities
            .iter()
            .map(|(value, cell)| (*value, *cell))
            .collect(),
        lookup_cells,
        instance_cells: builder
            .assigned_instances
            .iter()
            .map(|column| {
                column
                    .iter()
                    .map(|value| value.cell.expect("instance position"))
                    .collect()
            })
            .collect(),
    }
}

fn assert_fixed_topology<C>()
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let raw = mixed_bytes::<C>(C::ScalarExt::ZERO, C::ScalarExt::ONE, 1);
    let initial = captured_fixture::<C>(&raw, &MIXED_ITEMS, raw.len(), true)
        .expect("baseline transcript topology");
    let shape = proof_byte_shape(&initial.builder);
    for (first, second, multiple) in [
        (-C::ScalarExt::ONE, C::ScalarExt::ZERO, 1),
        (C::ScalarExt::ONE, C::ScalarExt::ONE, 7),
        (C::ScalarExt::from(23), C::ScalarExt::from(41), 13),
    ] {
        let raw = mixed_bytes::<C>(first, second, multiple);
        let fixture = captured_fixture::<C>(&raw, &MIXED_ITEMS, raw.len(), true)
            .expect("changed transcript values");
        assert_eq!(proof_byte_shape(&fixture.builder), shape);
        assert_eq!(
            proof_byte_shape(&fixture.builder.deep_clone().unknown(true)),
            shape
        );
    }
}

#[test]
fn proof_bytes_keep_identical_topology_across_values_in_both_parities() {
    assert_fixed_topology::<EqAffine>();
    assert_fixed_topology::<EpAffine>();
}
