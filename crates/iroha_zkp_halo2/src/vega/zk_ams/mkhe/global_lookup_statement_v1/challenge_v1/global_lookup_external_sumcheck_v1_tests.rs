//! Hostile-path and parity tests for the fused global-lookup suffix bridge.
use super::super::super::external_sumcheck_storage_v1::{
    global_cubic_final_round_fixture_v1, global_cubic_hollow_fixture_v1,
};
use super::*;
use std::path::Path;
fn context_v1() -> GlobalLookupContextV1 {
    GlobalLookupContextV1 {
        fixed_axes_digest: [0x11; 32],
        source_binding_digest: [0x22; 32],
        radix_range_digest: [0x33; 32],
        packing_digest: [0x44; 32],
        cross_field_digest: [0x55; 32],
        qpcs_initial_root: [0x66; 32],
    }
}
fn frames_v1() -> BoundTranscriptFramesV1 {
    BoundTranscriptFramesV1 {
        commitment_digest: [0x71; 32],
        inverse_digest: [0x72; 32],
        opening_digest: [0x75; 32],
        existing_commitments: EXISTING_ACTIVE_PLANES_V1,
        added_commitments: ADDED_PRE_Z_PLANES_V1,
        existing_inverses: EXISTING_ACTIVE_PLANES_V1,
        added_inverses: ADDED_INVERSE_PLANES_V1,
        cubic_messages: REQUIRED_CUBIC_MESSAGES_V1,
        hidden_endpoints: HIDDEN_ENDPOINTS_V1,
        multiplicity_commitments: MULTIPLICITY_COMMITMENTS_V1,
        sumcheck_mask_commitments: SUMCHECK_MASK_COMMITMENTS_V1,
        ipas: COEFFICIENT_IPAS_V1,
        table_ipas: TABLE_IPAS_V1,
        mask_ipas: MASK_IPAS_V1,
        gates: ENDPOINT_GATES_V1,
    }
}
fn seals_v1() -> BoundOwnerSealsV1 {
    BoundOwnerSealsV1 {
        source_packing_seal: SourcePackingOwnerSealV1::TestOnly,
        lookup_seal: LookupOwnerSealV1::TestOnly,
        proof_seal: ProofOwnerSealV1::TestOnly,
    }
}
fn gtilde_v1(ordinal: usize) -> [u8; CUBIC_MESSAGE_BYTES_V1] {
    let mut bytes = [0_u8; CUBIC_MESSAGE_BYTES_V1];
    for coefficient in 0..3 {
        let value = Scalar::from_u64((ordinal * 3 + coefficient + 1) as u64).to_le_bytes();
        bytes[coefficient * 32..(coefficient + 1) * 32].copy_from_slice(&value);
    }
    bytes
}
fn endpoint_v1(ordinal: usize) -> [u8; 33] {
    Point::canonical_generator()
        .expect("canonical generator")
        .mul_scalar(Scalar::from_u64((ordinal + 1) as u64))
        .to_non_identity_wire_bytes()
        .expect("non-identity endpoint")
}
fn sumcheck_after_v1(count: usize) -> GlobalLookupTranscriptV1<SumcheckStageV1> {
    let mut transcript = GlobalLookupTranscriptV1::begin_v1(context_v1(), seals_v1(), frames_v1())
        .unwrap()
        .absorb_commitments_and_derive_z_v1()
        .unwrap()
        .absorb_inverses_and_derive_relation_v1()
        .unwrap()
        .absorb_coefficient_residual_commitments_v1(
            core::array::from_fn(|ordinal| endpoint_v1(HIDDEN_ENDPOINTS_V1 + ordinal)),
            CoefficientResidualCommitmentSealV1::TestOnly,
        )
        .unwrap();
    for ordinal in 0..count {
        transcript = transcript
            .absorb_gtilde_v1(ordinal, gtilde_v1(ordinal))
            .unwrap();
    }
    transcript
}
fn point_v1(transcript: &GlobalLookupTranscriptV1<SumcheckStageV1>, round: usize) -> [Scalar; 29] {
    let mut point = [Scalar::zero(); 29];
    point[..round].copy_from_slice(
        &transcript.challenges.sumcheck[GLOBAL_MESSAGE_OFFSET_V1..GLOBAL_MESSAGE_OFFSET_V1 + round],
    );
    point
}
fn hollow_prefix_v1(
    public_context: [u8; 32],
    round: usize,
    point: [Scalar; 29],
    axes: [Scalar; 4],
    rho: [Scalar; 29],
    message: Option<[u8; 96]>,
) -> GlobalCubicPrefixReadyV1 {
    global_cubic_hollow_fixture_v1(
        public_context,
        round as u8,
        axes[0],
        rho,
        axes[1],
        axes[2],
        axes[3],
        point,
        message,
    )
    .unwrap_or_else(|error| panic!("hollow fixture failed: {error:?}"))
}
fn exact_hollow_prefix_v1(
    transcript: &GlobalLookupTranscriptV1<SumcheckStageV1>,
    message: Option<[u8; 96]>,
) -> GlobalCubicPrefixReadyV1 {
    let challenges = &transcript.challenges;
    hollow_prefix_v1(
        transcript.bound_context_digest,
        EXTERNAL_FIRST_ROUND_V1,
        point_v1(transcript, EXTERNAL_FIRST_ROUND_V1),
        [
            challenges.z,
            challenges.alpha,
            challenges.lambda,
            challenges.mu,
        ],
        challenges.rho,
        message,
    )
}
fn different_nonzero_v1(value: Scalar) -> Scalar {
    if value == Scalar::one() {
        Scalar::from_u64(2)
    } else {
        Scalar::one()
    }
}
#[test]
#[rustfmt::skip]
fn bound_context_digest_covers_each_exact_nonzero_context_frame() {
    let expected = bound_context_digest_v1(&context_v1()).unwrap();
    assert_eq!(expected, hex_literal::hex!("dc3d54b841ca0f46d33e0871f798797798aeab94735bad1419349e7983cb1074"));
    assert_ne!(expected, [0; 32]);
    for coordinate in 0..6 {
        let mut context = context_v1();
        match coordinate {
            0 => context.fixed_axes_digest[0] ^= 1,
            1 => context.source_binding_digest[0] ^= 1,
            2 => context.radix_range_digest[0] ^= 1,
            3 => context.packing_digest[0] ^= 1,
            4 => context.cross_field_digest[0] ^= 1,
            5 => context.qpcs_initial_root[0] ^= 1,
            _ => unreachable!(),
        }
        assert_ne!(bound_context_digest_v1(&context).unwrap(), expected);
    }
    let mut zero = context_v1();
    zero.fixed_axes_digest = [0; 32];
    assert_eq!(
        bound_context_digest_v1(&zero),
        Err(GlobalLookupErrorV1::Context)
    );
}
#[test]
fn exact_handoff_accepts_only_208_257_round_three_and_bound_prefix() {
    assert!(core::mem::needs_drop::<GlobalCubicPrefixReadyV1>());
    assert!(core::mem::needs_drop::<GlobalCubicOracleV1>());
    assert!(core::mem::needs_drop::<GlobalCubicCompleteV1>());
    let transcript = sumcheck_after_v1(HANDOFF_NEXT_SUMCHECK_V1);
    let prefix = exact_hollow_prefix_v1(&transcript, None);
    let session = GlobalLookupExternalSumcheckSessionV1::begin_v1(transcript, prefix).unwrap();
    assert!(core::mem::needs_drop::<GlobalLookupExternalSumcheckSessionV1>());
    drop(session);
    let mut early = sumcheck_after_v1(HANDOFF_NEXT_SUMCHECK_V1);
    early.next_sumcheck -= 1;
    let prefix = exact_hollow_prefix_v1(&early, None);
    assert!(matches!(
        GlobalLookupExternalSumcheckSessionV1::begin_v1(early, prefix),
        Err(GlobalLookupExternalSumcheckErrorV1::Transcript(
            GlobalLookupErrorV1::Order
        ))
    ));
    let mut wrong_ordinal = sumcheck_after_v1(HANDOFF_NEXT_SUMCHECK_V1);
    wrong_ordinal.challenge_ordinal -= 1;
    let prefix = exact_hollow_prefix_v1(&wrong_ordinal, None);
    assert!(GlobalLookupExternalSumcheckSessionV1::begin_v1(wrong_ordinal, prefix).is_err());
    let transcript = sumcheck_after_v1(HANDOFF_NEXT_SUMCHECK_V1);
    let mut point = point_v1(&transcript, 4);
    point[3] = Scalar::one();
    let challenges = &transcript.challenges;
    let prefix = hollow_prefix_v1(
        transcript.bound_context_digest,
        4,
        point,
        [
            challenges.z,
            challenges.alpha,
            challenges.lambda,
            challenges.mu,
        ],
        challenges.rho,
        None,
    );
    assert!(GlobalLookupExternalSumcheckSessionV1::begin_v1(transcript, prefix).is_err());
}
#[test]
fn context_axis_and_point_splices_are_rejected_one_at_a_time() {
    for mutation in 0..8 {
        let transcript = sumcheck_after_v1(HANDOFF_NEXT_SUMCHECK_V1);
        let challenges = &transcript.challenges;
        let mut public_context = transcript.bound_context_digest;
        let mut point = point_v1(&transcript, EXTERNAL_FIRST_ROUND_V1);
        let mut axes = [
            challenges.z,
            challenges.alpha,
            challenges.lambda,
            challenges.mu,
        ];
        let mut rho = challenges.rho;
        match mutation {
            0 => public_context[0] ^= 1,
            1 => {
                axes[0] = if axes[0] == Scalar::from_u64(40_000) {
                    Scalar::from_u64(40_001)
                } else {
                    Scalar::from_u64(40_000)
                }
            }
            2 => rho[0] = different_nonzero_v1(rho[0]),
            3 => axes[1] = different_nonzero_v1(axes[1]),
            4 => axes[2] = different_nonzero_v1(axes[2]),
            5 => axes[3] = different_nonzero_v1(axes[3]),
            6 => point[1] = different_nonzero_v1(point[1]),
            7 => point[3] = Scalar::one(),
            _ => unreachable!(),
        }
        let prefix = hollow_prefix_v1(
            public_context,
            EXTERNAL_FIRST_ROUND_V1,
            point,
            axes,
            rho,
            None,
        );
        assert!(GlobalLookupExternalSumcheckSessionV1::begin_v1(transcript, prefix).is_err());
    }
}
#[test]
fn missing_malformed_and_fold_failure_poison_the_move_only_session() {
    let directory = crate::testing::TestDirectory::new("global-lookup-sumcheck-errors");
    let transcript = sumcheck_after_v1(HANDOFF_NEXT_SUMCHECK_V1);
    let prefix = exact_hollow_prefix_v1(&transcript, None);
    let session = GlobalLookupExternalSumcheckSessionV1::begin_v1(transcript, prefix).unwrap();
    assert!(matches!(
        session.advance_v1(FoldSinkSealV1::TestOnly {
            directory: directory.path().to_path_buf(),
        }),
        Err(GlobalLookupExternalSumcheckErrorV1::Oracle(
            MOracleErrorV1::Order
        ))
    ));
    let transcript = sumcheck_after_v1(HANDOFF_NEXT_SUMCHECK_V1);
    let prefix = exact_hollow_prefix_v1(&transcript, Some([0xff; 96]));
    let session = GlobalLookupExternalSumcheckSessionV1::begin_v1(transcript, prefix).unwrap();
    assert!(matches!(
        session.advance_v1(FoldSinkSealV1::TestOnly {
            directory: directory.path().to_path_buf(),
        }),
        Err(GlobalLookupExternalSumcheckErrorV1::Transcript(
            GlobalLookupErrorV1::Encoding
        ))
    ));
    let transcript = sumcheck_after_v1(HANDOFF_NEXT_SUMCHECK_V1);
    let prefix = exact_hollow_prefix_v1(&transcript, Some(gtilde_v1(HANDOFF_NEXT_SUMCHECK_V1)));
    let session = GlobalLookupExternalSumcheckSessionV1::begin_v1(transcript, prefix).unwrap();
    assert!(matches!(
        session.advance_v1(FoldSinkSealV1::TestOnly {
            directory: directory.path().to_path_buf(),
        }),
        Err(GlobalLookupExternalSumcheckErrorV1::Oracle(
            MOracleErrorV1::Order
        ))
    ));
}
fn final_prefix_v1(
    transcript: &GlobalLookupTranscriptV1<SumcheckStageV1>,
    directory: &Path,
) -> GlobalCubicPrefixReadyV1 {
    let challenges = &transcript.challenges;
    global_cubic_final_round_fixture_v1(
        directory,
        transcript.bound_context_digest,
        challenges.z,
        challenges.rho,
        challenges.alpha,
        challenges.lambda,
        challenges.mu,
        point_v1(transcript, EXTERNAL_LAST_ROUND_V1),
        gtilde_v1(REQUIRED_CUBIC_MESSAGES_V1 - 1),
    )
    .unwrap_or_else(|error| panic!("final fixture failed: {error:?}"))
}
#[test]
fn final_bridge_message_has_exact_transcript_parity_kat_and_is_terminal() {
    let directory = crate::testing::TestDirectory::new("global-lookup-sumcheck-final");
    let transcript = sumcheck_after_v1(REQUIRED_CUBIC_MESSAGES_V1 - 1);
    let prefix = final_prefix_v1(&transcript, directory.path());
    let session =
        GlobalLookupExternalSumcheckSessionV1::from_aligned_test_only_v1(transcript, prefix)
            .unwrap();
    let transition = session
        .advance_v1(FoldSinkSealV1::TestOnly {
            directory: directory.path().to_path_buf(),
        })
        .unwrap();
    assert_eq!(
        transition.message_v1(),
        &gtilde_v1(REQUIRED_CUBIC_MESSAGES_V1 - 1)
    );
    let GlobalLookupExternalSumcheckTransitionV1::Complete(completion) = transition else {
        panic!("final round must be terminal");
    };
    let GlobalLookupExternalSumcheckCompleteV1 {
        mut transcript,
        oracle,
        ..
    } = *completion;
    let direct = sumcheck_after_v1(REQUIRED_CUBIC_MESSAGES_V1)
        .finish_sumcheck_v1()
        .unwrap();
    assert_eq!(
        transcript.state.fork_v1().finalize(),
        direct.state.fork_v1().finalize()
    );
    for ordinal in 0..HIDDEN_ENDPOINTS_V1 {
        transcript = transcript
            .absorb_endpoint_commitment_v1(ordinal, endpoint_v1(ordinal))
            .unwrap();
    }
    let transcript = transcript.derive_opening_batches_v1().unwrap();
    assert_eq!(
        transcript.absorb_openings_and_finish_v1().unwrap(),
        hex_literal::hex!("b7c4568000e11ee2a9833593cd8609ea2a026f2cf70be2335b498064c6860744")
    );
    drop(oracle);
}
#[test]
fn real_fold_sink_failure_returns_no_message_or_reusable_owner() {
    let directory = crate::testing::TestDirectory::new("global-lookup-sumcheck-failure");
    let transcript = sumcheck_after_v1(REQUIRED_CUBIC_MESSAGES_V1 - 1);
    let prefix = final_prefix_v1(&transcript, directory.path());
    let session =
        GlobalLookupExternalSumcheckSessionV1::from_aligned_test_only_v1(transcript, prefix)
            .unwrap();
    let not_a_directory = directory.path().join("fold-sink-file");
    std::fs::write(&not_a_directory, b"not a directory").unwrap();
    assert!(matches!(
        session.advance_v1(FoldSinkSealV1::TestOnly {
            directory: not_a_directory,
        }),
        Err(GlobalLookupExternalSumcheckErrorV1::Oracle(
            MOracleErrorV1::Spool
        ))
    ));
}
#[test]
fn source_guards_keep_the_bridge_fused_private_and_move_only() {
    let source = include_str!("global_lookup_external_sumcheck_v1.rs");
    let compact: String = source.split_whitespace().collect();
    let oracle = include_str!("../external_sumcheck_storage_v1/m_table_oracle_v1.rs");
    assert!(!source.contains("pub struct"));
    assert!(!source.contains("pub enum"));
    assert!(!source.contains("impl Clone for GlobalLookupExternalSumcheckSessionV1"));
    assert!(!source.contains("-> Scalar"));
    assert!(!source.contains("fn challenge_v1(&self"));
    assert!(source.contains("pub(super) struct GlobalLookupExternalSumcheckSessionV1"));
    assert!(source.contains("pub(super) enum GlobalLookupExternalSumcheckTransitionV1"));
    assert!(source.contains("live: Option<("));
    assert!(compact.contains("self.live.take()"));
    assert_eq!(source.matches("fold_with_raw_challenge_v1(").count(), 1);
    assert!(oracle.contains("struct GlobalCubicPrefixReadyV1"));
    assert!(!oracle.contains("OracleTranscriptSealV1"));
    assert!(!oracle.contains("shared_transcript: Infallible"));
    assert!(source.lines().count() <= 260);
    assert!(
        include_str!("global_lookup_external_sumcheck_v1_tests.rs")
            .lines()
            .count()
            <= 425
    );
}
