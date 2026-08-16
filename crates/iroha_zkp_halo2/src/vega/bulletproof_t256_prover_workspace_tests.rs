// Static and local-mutex tests for the fixed T256 membership workspace.

use crate::generalized_bulletproof::ProverTranscript as _;

type WorkspaceRole = T256MembershipWorkspaceRoleV1;
type MembershipError = ZkAmsT256MembershipErrorV1;

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    source
        .split_once(start)
        .unwrap()
        .1
        .split_once(end)
        .unwrap()
        .0
}

fn assert_source_order(source: &str, steps: &[&str]) {
    let mut prior = 0;
    for step in steps {
        let offset = source.find(*step).expect("workspace corridor step");
        assert!(offset >= prior);
        prior = offset + step.len();
    }
}
fn assert_source_count(source: &str, needle: &str, count: usize) {
    assert_eq!(source.matches(needle).count(), count);
}

fn preflight_proving_then_acquire_for_test(
    context: [u8; 32],
    ordinal: u16,
    coefficients: &[i8],
    bound: ZkAmsT256MembershipBoundV1,
    blinding: &Scalar,
    lease: &Mutex<()>,
) -> Result<(), MembershipError> {
    preflight_zk_ams_t256_membership_proving_v1(context, ordinal, coefficients, bound, blinding)?;
    let _guard = acquire_zk_ams_t256_membership_workspace_v1(lease, WorkspaceRole::Proving)?;
    Ok(())
}

#[test]
fn fixed_t256_workspace_is_non_reentrant_poisoned_and_preflight_first() {
    assert!(!ZkAmsT256BulletproofSuiteV1::ALLOW_PARALLEL_PROVER_WORKSPACE_V1);
    assert!(TinyT256Suite::ALLOW_PARALLEL_PROVER_WORKSPACE_V1);
    let context = keccak256(b"t256-prover-workspace-preflight");
    let coefficients = vec![0_i8; ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1];
    let blinding = Scalar::from_u64(1);
    let poisoned = Mutex::new(());
    assert!(
        std::panic::catch_unwind(|| {
            let _guard =
                acquire_zk_ams_t256_membership_workspace_v1(&poisoned, WorkspaceRole::Proving)
                    .expect("fresh local workspace");
            panic!("deliberately poison local T256 membership workspace");
        })
        .is_err()
    );
    let mut outside = coefficients.clone();
    outside[19] = 3;
    let preflight = |context, ordinal, coefficients: &[i8], blinding| {
        preflight_proving_then_acquire_for_test(
            context,
            ordinal,
            coefficients,
            ZkAmsT256MembershipBoundV1::Two,
            blinding,
            &poisoned,
        )
    };
    for (actual, expected) in [
        (
            preflight(
                context,
                0,
                &coefficients[..coefficients.len() - 1],
                &blinding,
            ),
            MembershipError::CoefficientCount,
        ),
        (
            preflight([0; 32], 0, &coefficients, &blinding),
            MembershipError::Context,
        ),
        (
            preflight(
                context,
                ZK_AMS_MEMBERSHIP_MAX_CHUNK_ORDINAL_V1 + 1,
                &coefficients,
                &blinding,
            ),
            MembershipError::ChunkOrdinal,
        ),
        (
            preflight(context, 0, &coefficients, &Scalar::ZERO),
            MembershipError::Blinding,
        ),
        (
            preflight(context, 0, &outside, &blinding),
            MembershipError::CoefficientOutOfRange { index: 19 },
        ),
    ] {
        assert_eq!(actual, Err(expected));
    }
    use ZkAmsT256MembershipErrorV1::{
        CommitmentLeasePoisoned, ProvingLeasePoisoned, VerificationLeasePoisoned,
    };
    for role_error in [
        (WorkspaceRole::Proving, ProvingLeasePoisoned),
        (WorkspaceRole::Commitment, CommitmentLeasePoisoned),
        (WorkspaceRole::Verification, VerificationLeasePoisoned),
    ] {
        assert!(matches!(
            acquire_zk_ams_t256_membership_workspace_v1(&poisoned, role_error.0),
            Err(error) if error == role_error.1
        ));
    }
    let fresh = Mutex::new(());
    let guard = acquire_zk_ams_t256_membership_workspace_v1(&fresh, WorkspaceRole::Proving)
        .expect("fresh workspace");
    assert!(matches!(
        acquire_zk_ams_t256_membership_workspace_v1(&fresh, WorkspaceRole::Verification,),
        Err(MembershipError::WorkspaceLeaseReentered)
    ));
    drop(guard);
    assert!(fresh.try_lock().is_ok());
    let ordinary_error = (|| -> Result<(), MembershipError> {
        let _guard =
            acquire_zk_ams_t256_membership_workspace_v1(&fresh, WorkspaceRole::Commitment)?;
        Err(MembershipError::Context)
    })();
    assert_eq!(ordinary_error, Err(MembershipError::Context));
    assert!(fresh.try_lock().is_ok());
}

#[test]
#[cfg(target_pointer_width = "64")]
fn fixed_t256_ipa_fold_capacity_payloads_are_pinned_without_rss_claims() {
    let (n, scalar, point) = (
        65_536,
        core::mem::size_of::<Scalar>(),
        core::mem::size_of::<Point>(),
    );
    assert_eq!((scalar, point), (32, 96));
    assert_eq!(
        (4 * n * scalar, 2 * n * scalar, n * scalar),
        (8_388_608, 4_194_304, 2_097_152)
    );
    assert_eq!(n * point, 6_291_456);
    assert_eq!(
        ((5 * n / 4) * point, (3 * n / 2) * point),
        (7_864_320, 9_437_184)
    );
    assert_eq!(
        (
            (5 * n / 4) * point + n * scalar,
            (3 * n / 2) * point + n * scalar
        ),
        (9_961_472, 11_534_336)
    );
}

#[test]
fn fixed_t256_transcript_heap_is_only_the_exact_public_proof_buffer() {
    for (n, proof_bytes, final_state_bytes, challenge_scratch_bytes) in [
        (32_768_usize, 1_447, 2_361, 2_310),
        (65_536, 1_513, 2_467, 2_416),
    ] {
        let rounds = usize::try_from(n.ilog2()).expect("T256 rounds fit usize");
        assert_eq!(membership_proof_len(n), Ok(proof_bytes));
        assert_eq!(771 + 106 * rounds, final_state_bytes);
        assert_eq!(720 + 106 * rounds, challenge_scratch_bytes);
        let buffer = ExactT256ProofBufferV1::new(proof_bytes).expect("exact proof buffer");
        assert_eq!((buffer.len(), buffer.capacity()), (0, proof_bytes));
    }
    let point = TinyT256Suite::generators().g;
    transcript_v1::reset_partial_proof_buffer_drops_v1();
    let mut transcript = T256BulletproofProverTranscriptV1::<TinyT256Suite>::new(
        keccak256(b"exact-transcript-capacity-context"),
        keccak256(b"exact-transcript-capacity-basis"),
        0,
        2,
        &point,
        32,
    )
    .expect("exact partial transcript");
    assert_eq!(
        (transcript.partial_proof_len(), transcript.proof_capacity()),
        (0, 32)
    );
    assert_eq!(
        transcript.push_point(&point),
        Err(GeneralizedBulletproofErrorV1::ResourceOverflow)
    );
    assert_eq!(transcript.partial_proof_len(), 0);
    assert_eq!(
        transcript.complete(),
        Err(GeneralizedBulletproofErrorV1::TranscriptConsumption)
    );
    assert_eq!(transcript_v1::partial_proof_buffer_drops_v1(), 1);
    transcript_v1::reset_partial_proof_buffer_drops_v1();
    assert!(
        std::panic::catch_unwind(|| {
            let mut transcript = T256BulletproofProverTranscriptV1::<TinyT256Suite>::new(
                keccak256(b"exact-transcript-unwind-context"),
                keccak256(b"exact-transcript-unwind-basis"),
                1,
                2,
                &point,
                32,
            )
            .expect("exact unwind transcript");
            transcript
                .push_scalar(&Scalar::from_u64(7))
                .expect("one scalar fills proof buffer");
            panic!("exercise partial T256 proof-buffer unwind");
        })
        .is_err()
    );
    assert_eq!(transcript_v1::partial_proof_buffer_drops_v1(), 1);
}

#[test]
fn fixed_t256_workspace_source_graph_is_closed_and_capped() {
    let parent = include_str!("bulletproof_t256.rs");
    let lease = include_str!("bulletproof_t256_workspace_lease_v1.rs");
    let transcript = include_str!("bulletproof_t256_transcript_v1.rs");
    let generalized = include_str!("../generalized_bulletproof.rs");
    let exact_eight = include_str!("zk_ams/mkhe/exact_eight_chunk_membership.rs");
    assert_source_count(
        lease,
        "static ZK_AMS_T256_MEMBERSHIP_WORKSPACE_LEASE_V1: Mutex<()>",
        1,
    );
    assert!(parent.contains("let mut openings = try_exact_capacity_vec_v1(1)?;"));
    assert_source_order(
        source_between(
            lease,
            "fn acquire_zk_ams_t256_cpk_workspace_v1()",
            "fn first_out_of_range_coefficient_v1(",
        ),
        &[
            "acquire_zk_ams_t256_membership_workspace_v1(",
            "&ZK_AMS_T256_MEMBERSHIP_WORKSPACE_LEASE_V1",
            "T256MembershipWorkspaceRoleV1::Commitment",
            "GeneralizedBulletproofErrorV1::ResourceOverflow",
        ],
    );
    let preflights = lease
        .split_once("fn first_out_of_range_coefficient_v1(")
        .expect("allocation-free preflight boundary")
        .1;
    let shape_helpers = source_between(parent, "fn membership_shape(", "fn signed_scalar(");
    let bound_helpers = source_between(
        parent,
        "impl ZkAmsT256MembershipBoundV1 {",
        "impl TryFrom<u8> for ZkAmsT256MembershipBoundV1",
    );
    let exact_source =
        include_str!("../generalized_bulletproof/exact_small_coefficient_source_v1.rs");
    let exact_source_constructor = source_between(
        exact_source,
        "pub(crate) fn new(",
        "fn validate_statement_shape(",
    );
    for source in [
        preflights,
        shape_helpers,
        bound_helpers,
        exact_source_constructor,
    ] {
        for forbidden in [
            "Vec",
            "vec!",
            "Box",
            "String",
            "reserve",
            ".collect",
            "generators",
            "OnceLock",
            "acquire_zk_ams_t256_membership_workspace_v1",
            "Mutex",
            ".lock(",
            "rayon",
            "par_",
        ] {
            assert!(!source.contains(forbidden));
        }
    }
    assert_source_count(generalized, "ALLOW_PARALLEL_PROVER_WORKSPACE_V1", 4);
    assert!(parent.contains("const ALLOW_PARALLEL_PROVER_WORKSPACE_V1: bool = false;"));
    let mut parallel_sites = vec![
        generalized
            .find("par_chunks(SECRET_MSM_CHUNK_TERMS_V1)")
            .expect("secret-MSM parallel site"),
        generalized
            .find(".into_par_iter()")
            .expect("public-point fold parallel site"),
    ];
    parallel_sites.extend(
        generalized
            .match_indices("rayon::join(")
            .map(|(offset, _)| offset),
    );
    assert_eq!(parallel_sites.len(), 3);
    assert_eq!(generalized.matches("rayon::").count(), 2);
    assert_eq!(generalized.matches(".par_").count(), 1);
    assert_eq!(generalized.matches(".into_par_").count(), 1);
    for location in parallel_sites {
        let policy = generalized[..location]
            .rfind("if S::ALLOW_PARALLEL_PROVER_WORKSPACE_V1")
            .expect("suite policy before parallel prover site");
        assert!(location - policy < 1_500);
    }
    let production = parent.rsplit_once("\n#[cfg(test)]\nmod tests {").unwrap().0;
    for (start, end, steps) in [
        (
            "pub(super) fn prove_zk_ams_t256_membership_chunk_v1",
            "pub(super) fn commit_zk_ams_t256_membership_chunk_v1",
            [
                "preflight_zk_ams_t256_membership_proving_v1(",
                "acquire_zk_ams_t256_membership_workspace_v1(",
                "&ZK_AMS_T256_MEMBERSHIP_WORKSPACE_LEASE_V1",
                "T256MembershipWorkspaceRoleV1::Proving",
                "zk_ams_t256_bulletproof_generator_basis_digest_v1()",
                "ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1",
                "prove_membership_chunk_for_suite::<ZkAmsT256BulletproofSuiteV1, _>(",
            ],
        ),
        (
            "pub(super) fn commit_zk_ams_t256_membership_chunk_v1",
            "pub(super) fn verify_zk_ams_t256_membership_chunk_v1",
            [
                "preflight_zk_ams_t256_membership_opening_v1(",
                "acquire_zk_ams_t256_membership_workspace_v1(",
                "&ZK_AMS_T256_MEMBERSHIP_WORKSPACE_LEASE_V1",
                "T256MembershipWorkspaceRoleV1::Commitment",
                "zk_ams_t256_bulletproof_generator_basis_digest_v1()",
                "ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1",
                "membership_commitment_for_suite::<ZkAmsT256BulletproofSuiteV1>(",
            ],
        ),
    ] {
        assert_source_order(source_between(production, start, end), &steps);
    }
    let verifier = source_between(
        production,
        "fn verify_membership_input_for_suite_with_lease_v1<S>(",
        "fn verify_zk_ams_t256_membership_input_v1(",
    );
    assert_source_order(
        verifier,
        &[
            "prepare_zk_ams_t256_membership_verification_v1(",
            "acquire_zk_ams_t256_membership_workspace_v1(",
            "lease,",
            "T256MembershipWorkspaceRoleV1::Verification",
            "zk_ams_t256_bulletproof_generator_basis_digest_v1()",
            "ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1",
            "verify_prepared_membership_chunk_for_suite::<S>(",
        ],
    );
    let canonical_verifier = source_between(
        production,
        "fn verify_zk_ams_t256_membership_input_v1(",
        "pub(super) fn prove_zk_ams_t256_membership_chunk_v1",
    );
    assert_source_count(
        canonical_verifier,
        "&ZK_AMS_T256_MEMBERSHIP_WORKSPACE_LEASE_V1",
        1,
    );
    assert_source_order(
        canonical_verifier,
        &[
            "verify_membership_input_for_suite_with_lease_v1::<ZkAmsT256BulletproofSuiteV1>(",
            "&ZK_AMS_T256_MEMBERSHIP_WORKSPACE_LEASE_V1",
            "T256MembershipVerifierBasisV1::Canonical",
        ],
    );
    assert_source_count(production, "&ZK_AMS_T256_MEMBERSHIP_WORKSPACE_LEASE_V1", 3);
    assert_source_count(
        production,
        "prove_membership_chunk_for_suite::<ZkAmsT256BulletproofSuiteV1, _>(",
        1,
    );
    let prover = source_between(
        production,
        "fn prove_membership_chunk_for_suite<S, R>(",
        "enum ZkAmsT256MembershipVerificationInputV1",
    );
    assert_source_order(
        prover,
        &[
            "let expected_proof_len = membership_proof_len(padded_gates)?;",
            "let proof_buffer = ExactT256ProofBufferV1::new(expected_proof_len)?;",
            "membership_witness::<S>(",
            "new_with_exact_proof_buffer(",
            "statement.prove(rng, &mut transcript, witness)?;",
            "transcript.complete()?;",
        ],
    );
    assert_source_count(transcript, "try_exact_capacity_vec_v1(expected_len)?", 1);
    assert_source_count(transcript, "challenge_prefixed: Keccak256", 2);
    assert_source_count(transcript, ".fork_v1()", 2);
    for forbidden in [
        "state: Vec<u8>",
        "Vec::with_capacity",
        "input.clone()",
        "keccak256(&low)",
        "keccak256(&input)",
        "String",
        "rayon",
        "callback",
    ] {
        assert!(!transcript.contains(forbidden));
    }
    let proof_owner = source_between(
        transcript,
        "pub(super) struct ExactT256ProofBufferV1",
        "struct T256TranscriptStateV1",
    );
    for required in [
        "checked_add(encoded.len())",
        "end > self.expected_len",
        "self.bytes.capacity() != self.expected_len",
        "bytes.fill(0)",
        "compiler_fence",
    ] {
        assert!(proof_owner.contains(required));
    }
    let prover_transcript = source_between(
        transcript,
        "impl<S> ProverTranscript<S> for T256BulletproofProverTranscriptV1<S>",
        "/// Exact, allocation-bounded verifier transcript",
    );
    for method in ["fn push_scalar(", "fn push_point("] {
        let method = prover_transcript.split_once(method).unwrap().1;
        let proof = method
            .find("self.proof.append(encoded.as_ref())?")
            .expect("bounded proof append");
        let state = method
            .find("self.state.append(encoded.as_ref())")
            .expect("infallible streamed-state append");
        assert!(proof < state);
    }
    for (start, end) in [
        (
            "fn membership_commitment_for_suite<S>(",
            "fn membership_witness<S>(",
        ),
        (
            "fn prove_membership_chunk_for_suite<S, R>(",
            "enum ZkAmsT256MembershipVerificationInputV1",
        ),
        (
            "fn verify_prepared_membership_chunk_for_suite<S>(",
            "fn verify_membership_input_for_suite_with_lease_v1<S>(",
        ),
    ] {
        let core = source_between(production, start, end);
        assert!(!core.contains("acquire_zk_ams_t256_membership_workspace_v1"));
    }
    assert!(!exact_eight.contains("ensure_canonical_generator_basis"));
    assert!(!exact_eight.contains("zk_ams_t256_bulletproof_generator_basis_digest_v1"));
    for source in [production, lease] {
        for forbidden in [
            "clear_poison",
            "into_inner",
            "unsafe",
            "MaybeUninit",
            "ManuallyDrop",
        ] {
            assert!(!source.contains(forbidden));
        }
    }
    for (source, max_lines, max_bytes) in [
        (parent, 3_000, 120 * 1_024),
        (generalized, 3_000, 120 * 1_024),
        (lease, 500, 24 * 1_024),
        (transcript, 500, 24 * 1_024),
        (
            include_str!("bulletproof_t256_prover_workspace_tests.rs"),
            500,
            24 * 1_024,
        ),
    ] {
        assert!(source.lines().count() <= max_lines);
        assert!(source.len() <= max_bytes);
    }
}
