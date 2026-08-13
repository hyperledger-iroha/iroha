use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use super::*;
static DIRECTORY_SEQUENCE_V2: AtomicU64 = AtomicU64::new(0);
static TEST_MODULI_V2: [u64; 2] = [97, 113];
struct TestDirectoryV2(PathBuf);
impl TestDirectoryV2 {
    fn new_v2() -> Self {
        let sequence = DIRECTORY_SEQUENCE_V2.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "iroha-q-pcs-prover-v2-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated prover prerequisite directory");
        Self(path)
    }
}
impl Drop for TestDirectoryV2 {
    fn drop(&mut self) {
        fs::remove_dir(&self.0).expect("remove empty prover prerequisite directory");
    }
}
fn geometry_v2() -> SpoolGeometryV2 {
    SpoolGeometryV2 {
        ring_degree: 4,
        domain_log: 4,
        query_count: 4,
        coefficient_values_per_block: 2,
        lde_values_per_block: 2,
        moduli: &TEST_MODULI_V2,
    }
}
fn context_v2() -> PublicSpoolContextV2 {
    PublicSpoolContextV2 {
        sealed_source_transcript_digest: [0x31; 32],
        source_algebra_binding_digest: [0x52; 32],
    }
}
fn pass_v2(directory: &TestDirectoryV2) -> MaskedCoefficientPassV2 {
    MaskedCoefficientPassV2::create_with_geometry_v2(
        &directory.0,
        geometry_v2(),
        context_v2(),
        AuthenticatedReplayPermitV2::TestOnly,
    )
    .expect("create tiny masked coefficient pass")
}
fn secret_v2(values: &[u64]) -> SecretResiduesV2 {
    let mut owner = SecretResiduesV2::new_zeroed_exact_v2(values.len()).expect("secret owner");
    owner.as_mut_slice_v2().copy_from_slice(values);
    assert_eq!(owner.values.len(), owner.values.capacity());
    owner
}
fn relation_v2(
    limb: u8,
    repetition: u8,
    product: &[u64],
    quotient: &[u64],
) -> SecretRelationCoefficientsV2 {
    SecretRelationCoefficientsV2::new_v2(limb, repetition, secret_v2(product), secret_v2(quotient))
}
fn zero_relation_v2(limb: u8, repetition: u8) -> SecretRelationCoefficientsV2 {
    relation_v2(limb, repetition, &[0; 7], &[0; 3])
}
#[derive(Default)]
struct CountingEntropyV2 {
    next: u64,
    calls: usize,
    coordinates: Vec<MaskSampleDomainV2>,
}
impl MaskEntropyV2 for CountingEntropyV2 {
    fn fill_word_v2(
        &mut self,
        coordinate: MaskSampleDomainV2,
        destination: &mut [u8; 8],
    ) -> Result<(), ()> {
        self.calls += 1;
        self.next += 1;
        self.coordinates.push(coordinate);
        destination.copy_from_slice(&self.next.to_be_bytes());
        Ok(())
    }
}
struct ConstantEntropyV2(u64);
impl MaskEntropyV2 for ConstantEntropyV2 {
    fn fill_word_v2(
        &mut self,
        _coordinate: MaskSampleDomainV2,
        destination: &mut [u8; 8],
    ) -> Result<(), ()> {
        destination.copy_from_slice(&self.0.to_be_bytes());
        Ok(())
    }
}
struct FailingEntropyV2;
impl MaskEntropyV2 for FailingEntropyV2 {
    fn fill_word_v2(
        &mut self,
        _coordinate: MaskSampleDomainV2,
        _destination: &mut [u8; 8],
    ) -> Result<(), ()> {
        Err(())
    }
}
struct PanickingEntropyV2;
impl MaskEntropyV2 for PanickingEntropyV2 {
    fn fill_word_v2(
        &mut self,
        _coordinate: MaskSampleDomainV2,
        _destination: &mut [u8; 8],
    ) -> Result<(), ()> {
        panic!("intentional external entropy unwind")
    }
}
struct RejectThenAcceptEntropyV2 {
    reject: usize,
    calls: usize,
}
impl MaskEntropyV2 for RejectThenAcceptEntropyV2 {
    fn fill_word_v2(
        &mut self,
        _coordinate: MaskSampleDomainV2,
        destination: &mut [u8; 8],
    ) -> Result<(), ()> {
        let value = if self.calls < self.reject {
            u64::MAX
        } else {
            102
        };
        self.calls += 1;
        destination.copy_from_slice(&value.to_be_bytes());
        Ok(())
    }
}
fn read_row_v2(stage: QPcsCoefficientReplayStageV2) -> (QPcsCoefficientReplayStageV2, Vec<u64>) {
    let geometry = geometry_v2();
    let mut reader = stage
        .begin_next_coefficient_row_v2()
        .expect("begin exact-purpose coefficient replay");
    let mut values = Vec::new();
    for _ in 0..geometry
        .coefficient_blocks_per_component_v2()
        .expect("coefficient blocks")
    {
        let chunk = reader.read_next_block_v2().expect("authenticated block");
        for encoded in chunk.bytes_v2().chunks_exact(8) {
            values.push(u64::from_be_bytes(encoded.try_into().expect("u64")));
        }
    }
    (
        reader.complete_v2().expect("complete exact row replay"),
        values,
    )
}
#[cfg(unix)]
#[test]
fn literal_mask_equations_spool_order_top_zeros_and_replay_are_exact() {
    let directory = TestDirectoryV2::new_v2();
    let mut pass = pass_v2(&directory);
    let mut entropy = CountingEntropyV2::default();
    for limb in 0..2 {
        for repetition in 0..5 {
            let relation = if limb == 0 && repetition == 0 {
                relation_v2(limb, repetition, &[10, 20, 30, 40, 50, 60, 70], &[7, 8, 9])
            } else {
                zero_relation_v2(limb, repetition)
            };
            pass.absorb_next_relation_v2(relation, &mut entropy)
                .expect("write masked relation in canonical order");
        }
    }
    assert_eq!(entropy.calls, 2 * 5 * 3);
    let first = entropy.coordinates[0];
    assert_eq!(first.domain, MASK_SAMPLE_DOMAIN_V2);
    assert_eq!(first.sealed_source_transcript_digest, [0x31; 32]);
    assert_eq!(first.source_algebra_binding_digest, [0x52; 32]);
    assert_eq!(
        (
            first.limb,
            first.repetition,
            first.coefficient,
            first.attempt,
            first.modulus
        ),
        (0, 0, 0, 0, 97)
    );
    let mut sealed = pass
        .seal_coefficients_v2()
        .expect("seal coefficients before any LDE write");
    assert_eq!(sealed.context.sealed_source_transcript_digest, [0x31; 32]);
    let stage = sealed.stage.take().expect("sealed replay stage");
    let (stage, product_low) = read_row_v2(stage);
    let (stage, product_high) = read_row_v2(stage);
    let (_stage, quotient) = read_row_v2(stage);
    // Independent literal oracle for S=[1,2,3], including both fixed top zeros.
    assert_eq!(product_low, [11, 22, 33, 40]);
    assert_eq!(product_high, [51, 62, 73, 0]);
    assert_eq!(quotient, [8, 10, 12, 0]);
}
#[cfg(unix)]
#[test]
fn missing_reordered_oversized_and_noncanonical_sources_fail_closed() {
    let directory = TestDirectoryV2::new_v2();
    assert!(matches!(
        pass_v2(&directory).seal_coefficients_v2(),
        Err(ProverPrerequisiteErrorV2::MissingRelations)
    ));
    let mut reordered = pass_v2(&directory);
    assert_eq!(
        reordered
            .absorb_next_relation_v2(zero_relation_v2(0, 1), &mut CountingEntropyV2::default()),
        Err(ProverPrerequisiteErrorV2::InvalidRelationOrder)
    );
    assert_eq!(
        reordered
            .absorb_next_relation_v2(zero_relation_v2(0, 0), &mut CountingEntropyV2::default()),
        Err(ProverPrerequisiteErrorV2::Poisoned)
    );
    let mut oversized = pass_v2(&directory);
    assert_eq!(
        oversized.absorb_next_relation_v2(
            relation_v2(0, 0, &[0, 0, 0, 0, 0, 0, 0, 1], &[0; 3]),
            &mut CountingEntropyV2::default()
        ),
        Err(ProverPrerequisiteErrorV2::InvalidSourceShape)
    );
    let mut noncanonical = pass_v2(&directory);
    assert_eq!(
        noncanonical.absorb_next_relation_v2(
            relation_v2(0, 0, &[97, 0, 0, 0, 0, 0, 0], &[0; 3]),
            &mut CountingEntropyV2::default()
        ),
        Err(ProverPrerequisiteErrorV2::NonCanonicalResidue)
    );
}
#[cfg(unix)]
#[test]
fn entropy_failure_reuse_and_fixed_rejection_bound_poison_the_pass() {
    let directory = TestDirectoryV2::new_v2();
    let words_before = SECRET_ENTROPY_WORD_DROPS_V2.load(Ordering::SeqCst);
    let residues_before = SECRET_RESIDUE_DROPS_V2.load(Ordering::SeqCst);
    let mut failed = pass_v2(&directory);
    assert_eq!(
        failed.absorb_next_relation_v2(zero_relation_v2(0, 0), &mut FailingEntropyV2),
        Err(ProverPrerequisiteErrorV2::EntropyFailure)
    );
    assert!(SECRET_ENTROPY_WORD_DROPS_V2.load(Ordering::SeqCst) > words_before);
    assert!(SECRET_RESIDUE_DROPS_V2.load(Ordering::SeqCst) >= residues_before + 3);
    assert_eq!(
        failed.absorb_next_relation_v2(zero_relation_v2(0, 0), &mut CountingEntropyV2::default()),
        Err(ProverPrerequisiteErrorV2::Poisoned)
    );
    let mut reused = pass_v2(&directory);
    let mut constant = ConstantEntropyV2(1);
    reused
        .absorb_next_relation_v2(zero_relation_v2(0, 0), &mut constant)
        .expect("first mask value");
    assert_eq!(
        reused.absorb_next_relation_v2(zero_relation_v2(0, 1), &mut constant),
        Err(ProverPrerequisiteErrorV2::ReusedMask)
    );
    assert_eq!(
        reused.absorb_next_relation_v2(zero_relation_v2(0, 2), &mut constant),
        Err(ProverPrerequisiteErrorV2::Poisoned)
    );
    let mut rejected = pass_v2(&directory);
    let mut always_reject = ConstantEntropyV2(u64::MAX);
    assert_eq!(
        rejected.absorb_next_relation_v2(zero_relation_v2(0, 0), &mut always_reject),
        Err(ProverPrerequisiteErrorV2::RejectionBoundExhausted)
    );
}
#[test]
fn rejection_sampling_accepts_only_after_the_exact_last_bounded_attempt() {
    let mut entropy = RejectThenAcceptEntropyV2 {
        reject: 255,
        calls: 0,
    };
    let mask = sample_mask_v2(1, 97, context_v2(), 0, 0, &mut entropy).expect("attempt 255");
    assert_eq!(entropy.calls, usize::from(MASK_SAMPLE_ATTEMPTS_V2));
    assert_eq!(mask.as_slice_v2(), [5]);
    let zone = u64::MAX - u64::MAX % 97;
    assert_eq!(zone % 97, 0);
    assert!(102 < zone);
    let mut exhausted = RejectThenAcceptEntropyV2 {
        reject: 256,
        calls: 0,
    };
    assert!(matches!(
        sample_mask_v2(1, 97, context_v2(), 0, 0, &mut exhausted),
        Err(ProverPrerequisiteErrorV2::RejectionBoundExhausted)
    ));
    assert_eq!(exhausted.calls, usize::from(MASK_SAMPLE_ATTEMPTS_V2));
}
#[cfg(unix)]
#[test]
fn entropy_and_explicit_take_unwinds_zeroize_and_leave_no_reusable_state() {
    let directory = TestDirectoryV2::new_v2();
    let residues_before = SECRET_RESIDUE_DROPS_V2.load(Ordering::SeqCst);
    let words_before = SECRET_ENTROPY_WORD_DROPS_V2.load(Ordering::SeqCst);
    let mut pass = pass_v2(&directory);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = pass.absorb_next_relation_v2(zero_relation_v2(0, 0), &mut PanickingEntropyV2);
    }));
    assert!(unwind.is_err());
    assert!(SECRET_RESIDUE_DROPS_V2.load(Ordering::SeqCst) >= residues_before + 3);
    assert!(SECRET_ENTROPY_WORD_DROPS_V2.load(Ordering::SeqCst) > words_before);
    assert_eq!(
        pass.absorb_next_relation_v2(zero_relation_v2(0, 0), &mut CountingEntropyV2::default()),
        Err(ProverPrerequisiteErrorV2::Poisoned)
    );
    let mut explicit = pass_v2(&directory);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        explicit.panic_after_take_for_test_v2();
    }));
    assert!(unwind.is_err());
    assert_eq!(
        explicit.absorb_next_relation_v2(zero_relation_v2(0, 0), &mut CountingEntropyV2::default()),
        Err(ProverPrerequisiteErrorV2::Poisoned)
    );
}
#[test]
fn reuse_guard_compares_complete_values_without_a_digest_collision_surface() {
    let mut guard = MaskReuseGuardV2::new_v2().expect("exact five-mask guard");
    let first = secret_v2(&[1, 2, 3]);
    guard.check_v2(0, 0, &first).expect("first exact mask");
    guard.commit_v2(first).expect("retain first exact mask");
    assert_eq!(
        guard.check_v2(0, 1, &secret_v2(&[1, 2, 3])),
        Err(ProverPrerequisiteErrorV2::ReusedMask)
    );
    guard
        .check_v2(0, 1, &secret_v2(&[1, 2, 4]))
        .expect("one-coordinate difference is not reuse");
}
#[test]
fn authority_and_all_downstream_completion_gates_remain_false() {
    assert!(!MASKED_COEFFICIENTS_COMPLETE_V2);
    assert!(!INITIAL_C0_ROOT_PREPARED_V2);
    assert!(!INITIAL_C0_ROOT_FROZEN_V2);
    assert!(!POST_ROOT_POINTS_DERIVED_V2);
    assert!(!CQ_ROWS_WRITTEN_V2);
    assert!(!FRI_FIRST_PASS_COMPLETE_V2);
    assert!(!FRI_SECOND_PASS_COMPLETE_V2);
    assert!(!CANONICAL_PROOF_EMITTED_V2);
    assert!(!PROVER_RELEASE_READY_V2);
    let source = include_str!("prover_v2.rs");
    assert!(source.contains("source_aggregation: Infallible"));
    assert!(source.contains("algebra_verification: Infallible"));
    assert!(source.contains("authenticated_replay: Infallible"));
    assert!(!source.contains("pub fn"));
}
