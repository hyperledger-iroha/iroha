use super::*;
use crate::vega::MaskedRelaxedRandomErrorV1;
struct KatRandomV2(u64);
impl KatRandomV2 {
    const fn new() -> Self {
        Self(1)
    }
}
impl MaskedRelaxedRandomSourceV1 for KatRandomV2 {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        for (offset, byte) in destination.iter_mut().enumerate() {
            *byte = self
                .0
                .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                .rotate_left((offset % 64) as u32)
                .to_le_bytes()[offset % 8];
        }
        self.0 = self.0.wrapping_add(1);
        Ok(())
    }
}
struct ZeroRandomV2;
impl MaskedRelaxedRandomSourceV1 for ZeroRandomV2 {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        destination.fill(0);
        Ok(())
    }
}
struct FailingRandomV2(usize);
impl MaskedRelaxedRandomSourceV1 for FailingRandomV2 {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        if self.0 == 0 {
            return Err(MaskedRelaxedRandomErrorV1::Unavailable);
        }
        self.0 -= 1;
        destination.fill(0x5a);
        Ok(())
    }
}
struct PanickingRandomV2(usize);
impl MaskedRelaxedRandomSourceV1 for PanickingRandomV2 {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        assert!(self.0 != 0, "injected entropy unwind");
        self.0 -= 1;
        destination.fill(0xa5);
        Ok(())
    }
}
fn scalar(value: u64) -> Scalar {
    Scalar::from_u64(value)
}
fn point(hex_value: &str) -> Point {
    Point::from_non_identity_wire_bytes_exact(&hex::decode(hex_value).expect("literal hex"))
        .expect("literal canonical T256 point")
}
struct FixtureV2 {
    hyrax_commitments: Vec<Point>,
    bp_commitments: Vec<Point>,
    proof: Vec<u8>,
    bridge_root: [u8; 32],
}
impl FixtureV2 {
    fn statement(&self) -> KernelStatementV2<'_> {
        KernelStatementV2 {
            binding_digest: [0x51; 32],
            hyrax_commitments: &self.hyrax_commitments,
            bp_commitments: &self.bp_commitments,
        }
    }
}
fn fixture_v2(mask_tweak: u64) -> FixtureV2 {
    let fixture = detached_kernel_test_fixture_with_mask_v2([0x51; 32], mask_tweak)
        .expect("analytical exact-shape fixture");
    FixtureV2 {
        hyrax_commitments: fixture.hyrax_commitments,
        bp_commitments: fixture.bp_commitments,
        proof: fixture.proof,
        bridge_root: fixture.bridge_root,
    }
}
fn sparse_openings_v2() -> (ZeroizingT256ScalarVecV1, ZeroizingT256ScalarVecV1) {
    let mut first = ZeroizingT256ScalarVecV1::with_capacity(BRIDGE_BASIS_VIEW_V2);
    let mut second = ZeroizingT256ScalarVecV1::with_capacity(BRIDGE_BASIS_VIEW_V2);
    for _ in 0..BRIDGE_BASIS_VIEW_V2 {
        first.push(Scalar::zero());
        second.push(Scalar::zero());
    }
    first.as_mut_slice()[0] = scalar(3);
    first.as_mut_slice()[BRIDGE_BASIS_VIEW_V2 - 1] = scalar(257);
    second.as_mut_slice()[1] = scalar(5);
    second.as_mut_slice()[BRIDGE_BASIS_VIEW_V2 - 1] = scalar(263);
    (first, second)
}
fn validate_sparse_openings_v2(
    fixture: &FixtureV2,
    hyrax_basis: &CheckedBasisV2,
    bp_basis: &CheckedBasisV2,
    first: &ZeroizingT256ScalarVecV1,
    second: &ZeroizingT256ScalarVecV1,
) -> Result<(), BridgeErrorV2> {
    if public_commit_for_fixture_v2(&hyrax_basis.points, first.as_slice())?
        != fixture.hyrax_commitments[0]
        || public_commit_for_fixture_v2(&hyrax_basis.points, second.as_slice())?
            != fixture.hyrax_commitments[1]
        || public_commit_for_fixture_v2(&bp_basis.points, first.as_slice())?
            != fixture.bp_commitments[0]
        || public_commit_for_fixture_v2(&bp_basis.points, second.as_slice())?
            != fixture.bp_commitments[1]
    {
        return Err(BridgeErrorV2::Commitment);
    }
    Ok(())
}
fn sample_mask_for_fixture_v2<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
    hyrax_basis: &CheckedBasisV2,
    bp_basis: &CheckedBasisV2,
) -> Result<ZeroizingT256ScalarVecV1, BridgeErrorV2> {
    for _ in 0..BRIDGE_MAX_MASK_ATTEMPTS_V2 {
        let mut mask = ZeroizingT256ScalarVecV1::with_capacity(BRIDGE_BASIS_VIEW_V2);
        for _ in 0..BRIDGE_BASIS_VIEW_V2 {
            let mut entropy = ZeroizingRandomBytesV1::<BRIDGE_MASK_ENTROPY_BYTES_V2>::zeroed();
            random
                .fill_bytes(entropy.as_mut_slice())
                .map_err(|_| BridgeErrorV2::Random)?;
            mask.push(Scalar::from_uniform_le_bytes_ref(entropy.as_array()));
        }
        let hyrax = public_commit_for_fixture_v2(&hyrax_basis.points, mask.as_slice())?;
        let bp = public_commit_for_fixture_v2(&bp_basis.points, mask.as_slice())?;
        if !hyrax.is_identity() && !bp.is_identity() {
            return Ok(mask);
        }
    }
    Err(BridgeErrorV2::Random)
}
#[test]
fn exact_topology_proof_size_and_fail_closed_gates_are_frozen() {
    assert_eq!(BRIDGE_ROWS_V2, 1_024 + 512);
    assert_eq!(BRIDGE_VALUE_COLUMNS_V2, 1_024);
    assert_eq!(BRIDGE_BASIS_VIEW_V2, 1_025);
    assert_eq!(BRIDGE_MASK_POINT_BYTES_V2, 2 * 33);
    assert_eq!(BRIDGE_RESPONSE_BYTES_V2, 1_025 * 32);
    assert_eq!(BRIDGE_RAW_PROOF_BYTES_V2, 2 * 33 + 1_025 * 32);
    assert_eq!(BRIDGE_RAW_PROOF_BYTES_V2, 32_866);
    assert_eq!(BRIDGE_MAX_MASK_ATTEMPTS_V2, 2);
    assert_eq!(BRIDGE_MASK_ENTROPY_BYTES_V2, 64);
    assert_eq!(BRIDGE_MASK_STATISTICAL_SECURITY_BITS_V2, 245);
    assert_eq!(
        HYRAX_KEY_LABEL_V2,
        super::super::super::COMMITMENT_KEY_LABEL_V1
    );
    const {
        assert!(!BRIDGE_SOURCE_BOUND_V2);
        assert!(!BRIDGE_PACKING_BOUND_V2);
        assert!(!BRIDGE_TERMINAL_WIRED_V2);
        assert!(!BRIDGE_RELEASE_ENABLED_V2);
    }
    assert_t256_suite_v2();
    let source = include_str!("terminal_cross_basis_ipa.rs");
    let bound = source
        .split("struct BoundT256BridgeRowSetV2")
        .nth(1)
        .expect("bound type")
        .split("struct VerifiedBridgeBindingV2")
        .next()
        .expect("bounded source section");
    assert!(bound.contains("source_binding: Infallible"));
    assert!(bound.contains("packing_binding: Infallible"));
    assert!(!bound.contains("pub "));
    assert!(!bound.contains("derive("));
    assert!(!source.contains("impl Clone for BoundT256BridgeRowSetV2"));
    assert!(!source.contains("impl core::fmt::Debug for BoundT256BridgeRowSetV2"));
    assert!(!source.contains("callback"));
    assert!(!source.contains("claimed_t"));
    assert!(!source.contains("prove_ipa_v2"));
    assert!(source.contains("sample_mask_v2"));
    assert!(source.contains("schnorr_challenge_v2"));
    assert!(source.contains("respond_v2"));
    assert!(source.contains("Result<SecretPoint<Point>, BridgeErrorV2>"));
    assert!(!source.contains("Ok(*commitment.expose_ref())"));
    assert!(source.contains("fn point(&mut self, point: &Point)"));
    assert!(source.contains("SecretT256PointEncodingV1::new(point)"));
    assert!(source.contains("writer.point(hyrax_mask.expose_ref())"));
    assert!(source.contains("writer.point(bp_mask.expose_ref())"));
    assert!(!source.contains("fn point(&mut self, point: Point)"));
}
#[test]
fn literal_framing_commitment_order_and_challenges_match_independent_kats() {
    // Values were computed independently with PyCryptodome Keccak-256 and
    // integer reduction modulo the published T256 scalar modulus.
    let seed = [0x42; 32];
    let counter_bytes = (0_u8..8).collect::<Vec<_>>();
    assert_eq!(
        hex::encode(
            framed_hash_v2(
                b"iroha.zk-ams.v2.cross-basis.kat",
                &[b"alpha", &counter_bytes, &seed],
            )
            .unwrap()
        ),
        "f7afd78556c8a5cc956b37c7a12684d6e4ec0a4f72b5b1b2a66ce990d10ef5bf"
    );
    assert_eq!(
        hex::encode(challenge_v2(ETA_DOMAIN_V2, seed).unwrap().to_le_bytes()),
        "314484b509c503095472ccee5acda0469d2e9cd749e0eee745da7dc4783d1467"
    );
    assert_eq!(
        hex::encode(
            schnorr_challenge_v2(
                [0x44; 32],
                [0x11; 32],
                [0x22; 32],
                &point("8016f70c3f35b3257896971b306635647bc52eb7cad7a5eca1a42f2340737749e3"),
                &point("00a37dc092877e239385cd8392ba2360ce1859a37f7a2b9c626b336608d2ce4cfe"),
            )
            .unwrap()
            .to_le_bytes()
        ),
        "df03935bed83f5a5742cda4ed6d6be63850cd6a3ff85148db665b7b0ece7fef2"
    );
    let twice = point("8016f70c3f35b3257896971b306635647bc52eb7cad7a5eca1a42f2340737749e3");
    let seven = point("00a37dc092877e239385cd8392ba2360ce1859a37f7a2b9c626b336608d2ce4cfe");
    let mut hyrax = Vec::with_capacity(BRIDGE_ROWS_V2);
    let mut bp = Vec::with_capacity(BRIDGE_ROWS_V2);
    for row in 0..BRIDGE_ROWS_V2 {
        hyrax.push(if row.is_multiple_of(2) { twice } else { seven });
        bp.push(if row.is_multiple_of(2) { seven } else { twice });
    }
    let statement = KernelStatementV2 {
        binding_digest: [0x33; 32],
        hyrax_commitments: &hyrax,
        bp_commitments: &bp,
    };
    let root = commitment_root_v2(&statement, [0x11; 32], [0x22; 32]).unwrap();
    assert_eq!(
        hex::encode(root),
        "eaebaaa95c12f2c5cb3181387f82872841ce1e49da3f619a622799ab7fcb262f"
    );
    assert_eq!(
        hex::encode(challenge_v2(ETA_DOMAIN_V2, root).unwrap().to_le_bytes()),
        "13772b036ee800275955ff28fa67f7a5ba1afde661f42b6933637a7979104843"
    );
}
#[test]
fn representation_sigma_simulates_and_extracts_the_same_opening() {
    let hyrax_basis = hyrax_basis_v2().expect("fixed Hyrax basis");
    let bp_basis = bp_basis_v2().expect("fixed BP basis");
    let mut opening = ZeroizingT256ScalarVecV1::with_capacity(BRIDGE_BASIS_VIEW_V2);
    let mut mask = ZeroizingT256ScalarVecV1::with_capacity(BRIDGE_BASIS_VIEW_V2);
    let mut simulated_response = ZeroizingT256ScalarVecV1::with_capacity(BRIDGE_BASIS_VIEW_V2);
    for column in 0..BRIDGE_BASIS_VIEW_V2 {
        opening.push(scalar((column as u64).wrapping_mul(17).wrapping_add(3)));
        mask.push(scalar((column as u64).wrapping_mul(29).wrapping_add(5)));
        simulated_response.push(scalar((column as u64).wrapping_mul(31).wrapping_add(7)));
    }
    let aggregate = AggregatedRowsV2 {
        hyrax_commitment: public_commit_for_fixture_v2(&hyrax_basis.points, opening.as_slice())
            .unwrap(),
        bp_commitment: public_commit_for_fixture_v2(&bp_basis.points, opening.as_slice()).unwrap(),
        opening,
    };
    // Perfect HVZK simulator for a chosen challenge/response: compute both
    // first messages without reading the witness opening.
    let simulated_challenge = scalar(41);
    let simulated_hyrax_response =
        public_commit_for_fixture_v2(&hyrax_basis.points, simulated_response.as_slice()).unwrap();
    let simulated_hyrax_mask = simulated_hyrax_response
        + aggregate
            .hyrax_commitment
            .mul_scalar(simulated_challenge)
            .negate();
    let simulated_bp_response =
        public_commit_for_fixture_v2(&bp_basis.points, simulated_response.as_slice()).unwrap();
    let simulated_bp_mask = simulated_bp_response
        + aggregate
            .bp_commitment
            .mul_scalar(simulated_challenge)
            .negate();
    assert_eq!(
        public_commit_for_fixture_v2(&hyrax_basis.points, simulated_response.as_slice()).unwrap(),
        simulated_hyrax_mask + aggregate.hyrax_commitment.mul_scalar(simulated_challenge)
    );
    assert_eq!(
        public_commit_for_fixture_v2(&bp_basis.points, simulated_response.as_slice()).unwrap(),
        simulated_bp_mask + aggregate.bp_commitment.mul_scalar(simulated_challenge)
    );
    // Two accepting responses with the same first message and distinct
    // challenges extract the one vector opening both commitments.
    let first_challenge = scalar(43);
    let second_challenge = scalar(47);
    let mut first_response = ZeroizingT256ScalarVecV1::with_capacity(BRIDGE_BASIS_VIEW_V2);
    let mut second_response = ZeroizingT256ScalarVecV1::with_capacity(BRIDGE_BASIS_VIEW_V2);
    for (randomizer, value) in mask.as_slice().iter().zip(aggregate.opening.as_slice()) {
        first_response.push(*randomizer + first_challenge * *value);
        second_response.push(*randomizer + second_challenge * *value);
    }
    let inverse_delta = (first_challenge - second_challenge).inverse().unwrap();
    for ((first, second), expected) in first_response
        .as_slice()
        .iter()
        .zip(second_response.as_slice())
        .zip(aggregate.opening.as_slice())
    {
        assert_eq!((*first - *second) * inverse_delta, *expected);
    }
}
#[test]
fn representation_equality_rejects_opening_statement_basis_entropy_and_wire_attacks() {
    let hyrax_basis = hyrax_basis_v2().expect("fixed Hyrax basis");
    let bp_basis = bp_basis_v2().expect("fixed BP basis");
    assert_ne!(hyrax_basis.digest, bp_basis.digest);
    let fixture = fixture_v2(0);
    assert_eq!(fixture.proof.len(), BRIDGE_RAW_PROOF_BYTES_V2);
    let expected_root = verify_kernel_with_bases_v2(
        &fixture.statement(),
        &fixture.proof,
        &hyrax_basis,
        &bp_basis,
    )
    .expect("both full-shape representation equations");
    assert_eq!(expected_root, fixture.bridge_root);
    assert_eq!(
        verify_detached_kernel_prerequisite_v2(
            [0x51; 32],
            &fixture.hyrax_commitments,
            &fixture.bp_commitments,
            &fixture.proof,
        ),
        Ok(expected_root)
    );
    let second = fixture_v2(9_001);
    assert_ne!(fixture.proof, second.proof);
    assert_eq!(second.bridge_root, expected_root);
    assert_eq!(
        verify_kernel_with_bases_v2(&second.statement(), &second.proof, &hyrax_basis, &bp_basis,),
        Ok(expected_root)
    );
    let before_zeroized =
        super::super::super::super::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1();
    {
        let (mut first, mut second) = sparse_openings_v2();
        validate_sparse_openings_v2(&fixture, &hyrax_basis, &bp_basis, &first, &second)
            .expect("matching sparse rows");
        first.as_mut_slice()[0] += Scalar::one();
        assert_eq!(
            validate_sparse_openings_v2(&fixture, &hyrax_basis, &bp_basis, &first, &second),
            Err(BridgeErrorV2::Commitment)
        );
        first.as_mut_slice()[0] -= Scalar::one();
        second.as_mut_slice()[BRIDGE_VALUE_COLUMNS_V2] += Scalar::one();
        assert_eq!(
            validate_sparse_openings_v2(&fixture, &hyrax_basis, &bp_basis, &first, &second),
            Err(BridgeErrorV2::Commitment)
        );
    }
    assert!(
        super::super::super::super::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1()
            > before_zeroized
    );
    sample_mask_for_fixture_v2(&mut KatRandomV2::new(), &hyrax_basis, &bp_basis)
        .expect("available nonzero entropy");
    assert_eq!(
        sample_mask_for_fixture_v2(&mut ZeroRandomV2, &hyrax_basis, &bp_basis).map(|_| ()),
        Err(BridgeErrorV2::Random)
    );
    let before_entropy_error =
        super::super::super::super::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1();
    assert_eq!(
        sample_mask_for_fixture_v2(&mut FailingRandomV2(3), &hyrax_basis, &bp_basis).map(|_| ()),
        Err(BridgeErrorV2::Random)
    );
    assert!(
        super::super::super::super::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1()
            > before_entropy_error
    );
    let before_entropy_unwind =
        super::super::super::super::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = sample_mask_for_fixture_v2(&mut PanickingRandomV2(3), &hyrax_basis, &bp_basis);
        }))
        .is_err()
    );
    assert!(
        super::super::super::super::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1()
            > before_entropy_unwind
    );
    let mut reordered_hyrax = fixture.hyrax_commitments.clone();
    let mut reordered_bp = fixture.bp_commitments.clone();
    reordered_hyrax.swap(0, 1);
    reordered_bp.swap(0, 1);
    let reordered = KernelStatementV2 {
        binding_digest: [0x51; 32],
        hyrax_commitments: &reordered_hyrax,
        bp_commitments: &reordered_bp,
    };
    assert!(
        verify_kernel_with_bases_v2(&reordered, &fixture.proof, &hyrax_basis, &bp_basis).is_err()
    );
    let changed_binding = KernelStatementV2 {
        binding_digest: [0x52; 32],
        hyrax_commitments: &fixture.hyrax_commitments,
        bp_commitments: &fixture.bp_commitments,
    };
    assert!(
        verify_kernel_with_bases_v2(&changed_binding, &fixture.proof, &hyrax_basis, &bp_basis)
            .is_err()
    );
    let mut changed_hyrax = fixture.hyrax_commitments.clone();
    changed_hyrax[17] += hyrax_basis.points[31];
    let changed_commitment = KernelStatementV2 {
        binding_digest: [0x51; 32],
        hyrax_commitments: &changed_hyrax,
        bp_commitments: &fixture.bp_commitments,
    };
    assert!(
        verify_kernel_with_bases_v2(&changed_commitment, &fixture.proof, &hyrax_basis, &bp_basis)
            .is_err()
    );
    let mut changed_bp = fixture.bp_commitments.clone();
    changed_bp[23] += bp_basis.points[37];
    let changed_commitment = KernelStatementV2 {
        binding_digest: [0x51; 32],
        hyrax_commitments: &fixture.hyrax_commitments,
        bp_commitments: &changed_bp,
    };
    assert!(
        verify_kernel_with_bases_v2(&changed_commitment, &fixture.proof, &hyrax_basis, &bp_basis)
            .is_err()
    );
    let mut changed_basis = hyrax_basis_v2().unwrap();
    changed_basis.points.swap(0, 1);
    validate_independent_points_v2(&changed_basis.points).unwrap();
    changed_basis.digest = basis_digest_v2(&changed_basis.points).unwrap();
    assert!(
        verify_kernel_with_bases_v2(
            &fixture.statement(),
            &fixture.proof,
            &changed_basis,
            &bp_basis
        )
        .is_err()
    );
    let mut duplicate_basis = hyrax_basis_v2().unwrap();
    duplicate_basis.points[1] = duplicate_basis.points[0];
    assert_eq!(
        validate_independent_points_v2(&duplicate_basis.points),
        Err(BridgeErrorV2::Basis)
    );
    duplicate_basis.points[1] = duplicate_basis.points[0].negate();
    assert_eq!(
        validate_independent_points_v2(&duplicate_basis.points),
        Err(BridgeErrorV2::Basis)
    );
    let mut colliding_bp_basis = bp_basis_v2().unwrap();
    colliding_bp_basis.points[0] = hyrax_basis.points[0];
    colliding_bp_basis.digest = basis_digest_v2(&colliding_bp_basis.points).unwrap();
    assert_eq!(
        validate_disjoint_bases_v2(&hyrax_basis, &colliding_bp_basis),
        Err(BridgeErrorV2::Basis)
    );
    let first_response = BRIDGE_MASK_POINT_BYTES_V2;
    let mut noncanonical_response = fixture.proof.clone();
    noncanonical_response[first_response..first_response + BRIDGE_SCALAR_BYTES_V2].fill(0xff);
    assert_eq!(
        verify_kernel_with_bases_v2(
            &fixture.statement(),
            &noncanonical_response,
            &hyrax_basis,
            &bp_basis
        ),
        Err(BridgeErrorV2::Wire)
    );
    let mut changed_hyrax_mask = fixture.proof.clone();
    changed_hyrax_mask[7] ^= 1;
    assert!(
        verify_kernel_with_bases_v2(
            &fixture.statement(),
            &changed_hyrax_mask,
            &hyrax_basis,
            &bp_basis
        )
        .is_err()
    );
    let mut changed_bp_mask = fixture.proof.clone();
    changed_bp_mask[BRIDGE_POINT_BYTES_V2 + 13] ^= 1;
    assert!(
        verify_kernel_with_bases_v2(
            &fixture.statement(),
            &changed_bp_mask,
            &hyrax_basis,
            &bp_basis
        )
        .is_err()
    );
    let mut changed_response = fixture.proof.clone();
    changed_response[first_response + 17] ^= 1;
    assert!(
        verify_kernel_with_bases_v2(
            &fixture.statement(),
            &changed_response,
            &hyrax_basis,
            &bp_basis
        )
        .is_err()
    );
    assert_eq!(
        ProofReaderV2::new(&fixture.proof[..BRIDGE_RAW_PROOF_BYTES_V2 - 1]).map(|_| ()),
        Err(BridgeErrorV2::Wire)
    );
    let mut trailing = fixture.proof.clone();
    trailing.push(0);
    assert_eq!(
        ProofReaderV2::new(&trailing).map(|_| ()),
        Err(BridgeErrorV2::ProofTooLarge)
    );
}
