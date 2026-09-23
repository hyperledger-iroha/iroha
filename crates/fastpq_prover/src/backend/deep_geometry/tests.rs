//! Exact subgroup, quotient-half and full-field OOD linkage controls.

use super::*;
use crate::gadgets::compact_smt_air::{DigestLimbs, PublicStatement, PublicUpdate};
use iroha_crypto::Hash;

fn digest(seed: u8) -> DigestLimbs {
    let hash = Hash::new([seed; 33]);
    let bytes: &[u8; 32] = hash.as_ref();
    core::array::from_fn(|limb| {
        u32::from_le_bytes(bytes[4 * limb..4 * limb + 4].try_into().unwrap())
    })
}

fn air() -> CompactTransferAir {
    CompactTransferAir::new(
        &PublicStatement {
            updates: [
                PublicUpdate {
                    old_leaf: digest(1),
                    new_leaf: digest(2),
                    path: 0xa59c_71e3,
                },
                PublicUpdate {
                    old_leaf: digest(3),
                    new_leaf: digest(4),
                    path: 0x6a35_8e1c,
                },
            ],
            old_root: digest(5),
            new_root: digest(6),
        },
        Some(b"independent caller OOD context"),
    )
    .unwrap()
}

fn dense(seed: u64) -> F {
    F::new([seed, seed + 3, seed + 5, seed + 7]).unwrap()
}

struct Fixture {
    geometry: DeepGeometry,
    relation: CompactTransferAir,
    alphas: Vec<F>,
    z: F,
    current: [F; COMMITTED_COLUMN_COUNT],
    next: [F; COMMITTED_COLUMN_COUNT],
    quotient: [F; 2],
}

impl Fixture {
    fn new() -> Self {
        let geometry = DeepGeometry::new().unwrap();
        let relation = air();
        let alphas = (0..CONSTRAINTS)
            .map(|index| dense(index as u64 + 13))
            .collect::<Vec<_>>();
        let z = dense(101);
        let current = core::array::from_fn(|column| dense(column as u64 + 17));
        let next = core::array::from_fn(|column| dense(column as u64 + 701));
        let (full_current, full_next) = geometry
            .public_columns
            .reconstruct_pair_at(z, &current, &next)
            .unwrap();
        let residues = relation.evaluate_at(z, &full_current, &full_next).unwrap();
        let mut expected = F::ZERO;
        for index in 0..CONSTRAINTS {
            expected = expected.add(alphas[index].mul(residues[index]));
        }
        let z_to_n = z.power(TRACE_ROWS as u64);
        let quotient_one = dense(1009);
        let quotient_zero = expected
            .mul(z_to_n.sub(F::ONE).inverse().unwrap())
            .sub(z_to_n.mul(quotient_one));
        Self {
            geometry,
            relation,
            alphas,
            z,
            current,
            next,
            quotient: [quotient_zero, quotient_one],
        }
    }

    fn check(&self) -> Result<DeepComposition> {
        self.geometry.check_ood(
            &self.relation,
            &self.alphas,
            self.z,
            &self.current,
            &self.next,
            &self.quotient,
        )
    }
}

#[test]
fn geometry_extends_the_exact_existing_subgroup_and_preserves_every_fold() {
    let geometry = DeepGeometry::new().unwrap();
    let params = DeepGeometry::polynomial_parameters();
    assert_eq!(params.lde_root, LDE_ROOT);
    assert_eq!(params.trace_root, FASTPQ_FINAL_V1.trace_root);
    assert_eq!(params.trace_log_size, FASTPQ_FINAL_V1.trace_log_size);
    assert_eq!(params.omega_coset, FASTPQ_FINAL_V1.omega_coset);
    assert_eq!(field_pow(LDE_ROOT, 16), FASTPQ_FINAL_V1.lde_root);
    assert_eq!(field_pow(LDE_ROOT, 128), geometry.trace_generator());
    assert_eq!(field_pow(LDE_ROOT, LDE_ROWS as u64), 1);
    assert_ne!(field_pow(LDE_ROOT, (LDE_ROWS / 2) as u64), 1);
    assert_ne!(field_pow(COSET_OFFSET, LDE_ROWS as u64), 1);
    assert_eq!((QUERY_COUNT, QUERY_CANDIDATES, CONSTRAINTS), (64, 74, 923));
    let mut domain = geometry.domain();
    for (round, arity) in FRI_ARITIES.into_iter().enumerate() {
        assert_eq!(field_pow(domain.generator, FRI_LENGTHS[round] as u64), 1);
        assert_ne!(
            field_pow(domain.generator, (FRI_LENGTHS[round] / 2) as u64),
            1
        );
        assert_eq!(FRI_LENGTHS[round + 1] * arity, FRI_LENGTHS[round]);
        assert_eq!(FRI_DEGREES[round + 1] * arity, FRI_DEGREES[round]);
        let next = domain.folded(arity);
        for index in [0, 1, 63, FRI_LENGTHS[round + 1] - 1] {
            assert_eq!(
                field_pow(domain.point(index), arity as u64),
                next.point(index)
            );
        }
        domain = next;
    }
    assert_eq!(FRI_LENGTHS[5], 128);
    assert_eq!(FRI_DEGREES[5], 1);
    assert_eq!(field_pow(domain.generator, 128), 1);
    assert_ne!(field_pow(domain.generator, 64), 1);
}

#[test]
fn complete_ood_identity_uses_both_quotient_halves_and_full_extension_points() {
    let mut fixture = Fixture::new();
    let checked = fixture.check().unwrap();
    let independently_prepared = DeepComposition::new(
        OodPair::new(fixture.z, fixture.geometry.trace_generator()).unwrap(),
        &fixture.current,
        &fixture.next,
        &fixture.quotient,
    )
    .unwrap();
    let query_row = (0..COMMITTED_COLUMN_COUNT)
        .map(|index| index as u64 + 5)
        .collect::<Vec<_>>();
    for point in [7, 73, GOLDILOCKS_MODULUS - 1] {
        assert_eq!(
            checked
                .base_value_at(point, &query_row, &fixture.quotient, dense(79))
                .unwrap(),
            independently_prepared
                .base_value_at(point, &query_row, &fixture.quotient, dense(79))
                .unwrap()
        );
    }
    for half in 0..2 {
        for lane in 0..4 {
            let original = fixture.quotient[half];
            let mut delta = [0; 4];
            delta[lane] = 1;
            fixture.quotient[half] = original.add(F::new(delta).unwrap());
            assert!(fixture.check().is_err(), "half={half}, lane={lane}");
            fixture.quotient[half] = original;
        }
    }
    fixture.quotient.swap(0, 1);
    assert!(fixture.check().is_err());
    fixture.quotient.swap(0, 1);
    let original = fixture.z;
    fixture.z = F::from_base(original.coefficients()[0]).unwrap();
    assert!(fixture.check().is_err());
}

#[test]
fn ood_linkage_rejects_wrong_public_polynomials_and_changed_answers() {
    let mut fixture = Fixture::new();
    fixture.check().unwrap();
    let (mut current, mut next) = fixture
        .geometry
        .public_columns
        .reconstruct_pair_at(fixture.z, &fixture.current, &fixture.next)
        .unwrap();
    // Substituting a physical-row phase or zeroing public columns off-domain is
    // not the verifier-owned polynomial reconstruction.
    for column in super::super::compact_public_columns::PUBLIC_COLUMNS {
        current[column] = F::ZERO;
        next[column] = F::ZERO;
    }
    let residues = fixture
        .relation
        .evaluate_at(fixture.z, &current, &next)
        .unwrap();
    let wrong_numerator = residues
        .iter()
        .zip(&fixture.alphas)
        .fold(F::ZERO, |sum, (&r, &alpha)| sum.add(r.mul(alpha)));
    let z_to_n = fixture.z.power(TRACE_ROWS as u64);
    let original_q = fixture.quotient;
    fixture.quotient = [
        wrong_numerator.mul(z_to_n.sub(F::ONE).inverse().unwrap()),
        F::ZERO,
    ];
    assert!(fixture.check().is_err());
    fixture.quotient = original_q;
    for index in [0, 31, 149, COMMITTED_COLUMN_COUNT - 1] {
        let original = fixture.current[index];
        fixture.current[index] = original.add(dense(4093));
        assert!(fixture.check().is_err(), "current column={index}");
        fixture.current[index] = original;
    }
    let original = fixture.alphas[0];
    fixture.alphas[0] = original.add(F::ONE);
    assert!(fixture.check().is_err());
}

#[test]
fn every_alpha_coordinate_and_ood_shape_is_checked() {
    let mut fixture = Fixture::new();
    for count in [0, CONSTRAINTS - 1, CONSTRAINTS + 1] {
        assert!(
            fixture
                .geometry
                .check_ood(
                    &fixture.relation,
                    &vec![F::ONE; count],
                    fixture.z,
                    &fixture.current,
                    &fixture.next,
                    &fixture.quotient
                )
                .is_err()
        );
    }
    for index in 0..CONSTRAINTS {
        for lane in 0..4 {
            let original = fixture.alphas[index];
            let mut words = [0; 4];
            words[lane] = GOLDILOCKS_MODULUS;
            fixture.alphas[index] = F::from_coefficients_unchecked_for_test(words);
            assert!(matches!(fixture.check(),
                Err(Error::NonCanonicalGoldilocksElement { context: "deep_ood_alpha", indices }) if indices == [index, lane]));
            fixture.alphas[index] = original;
        }
    }
    assert!(
        fixture
            .geometry
            .check_ood(
                &fixture.relation,
                &fixture.alphas,
                fixture.z,
                &fixture.current[..300],
                &fixture.next,
                &fixture.quotient
            )
            .is_err()
    );
    assert!(
        fixture
            .geometry
            .check_ood(
                &fixture.relation,
                &fixture.alphas,
                fixture.z,
                &fixture.current,
                &fixture.next[..300],
                &fixture.quotient
            )
            .is_err()
    );
    assert!(
        fixture
            .geometry
            .check_ood(
                &fixture.relation,
                &fixture.alphas,
                fixture.z,
                &fixture.current,
                &fixture.next,
                &fixture.quotient[..1]
            )
            .is_err()
    );
}
