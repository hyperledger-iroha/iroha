//! Initial-only output coalescing remains inside the original global envelope.

use super::*;

#[test]
fn initial_coalescing_charges_source_overlap_and_keeps_all_global_admissions_unchanged() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = MainProverBufferPlanV1::new_v1(&layout).unwrap();
    let rows = layout.common_lde_size();
    let additional = aggregate::joined_trace::initial_coalescing_extra_payload_v1(rows).unwrap();
    assert_eq!(plan.replay_batch, 310_494_720);
    assert_eq!(plan.initial_commit_batch, plan.replay_batch + additional);
    assert!(
        plan.initial_commit_batch
            >= 16 * rows * size_of::<F>()
                + 8 * ((1usize << 19) + MASK_DEGREE + 1) * size_of::<F>()
                + 2 * (1usize << 19) * size_of::<F>()
    );
    assert!(additional < 8 * rows * size_of::<F>() + (2 << 20));
    // Independent pre-coalescing pin: the largest registration remains the
    // global maximum. Initial hashing may not reduce source/runtime reserves.
    assert_eq!(plan.maximum_live_buffers, CURRENT_FULL_PROFILE_PEAK_V1);
    let mut previous = plan;
    previous.initial_commit_batch = previous.replay_batch;
    use super::super::main_ca_resources::{MainCaBufferPhaseV1 as Phase, MainCaJointBufferPlanV1};
    let joint = MainCaJointBufferPlanV1::from_common_v1(plan).unwrap();
    let old_joint = MainCaJointBufferPlanV1::from_common_v1(previous).unwrap();
    for phase in [
        Phase::OriginalCommitments,
        Phase::Registration,
        Phase::ShaUnion,
        Phase::PrivateLinks,
        Phase::Finalization,
    ] {
        let actual = joint.required_v1(phase).unwrap();
        assert!(actual <= plan.maximum_live_buffers);
        assert_eq!(
            actual - old_joint.required_v1(phase).unwrap(),
            if phase == Phase::OriginalCommitments {
                additional
            } else {
                0
            }
        );
    }
    for assembly in [0, 288_345_698] {
        assert_eq!(
            plan.check_before_sources_v1(assembly).unwrap(),
            previous.check_before_sources_v1(assembly).unwrap()
        );
        assert_eq!(
            plan.check_source_payloads_v1(&[assembly], MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1)
                .unwrap(),
            previous
                .check_source_payloads_v1(&[assembly], MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1)
                .unwrap()
        );
        // Finalization already exceeds the larger initial phase, so the
        // current eight-column GPU allowance remains exactly unchanged too.
        assert_eq!(
            plan.joined_transform_admission_v1(assembly).unwrap(),
            previous.joined_transform_admission_v1(assembly).unwrap()
        );
    }
    let available =
        usize::try_from(super::super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1)
            .unwrap()
            - plan.joined_transform_admission_v1(288_345_698).unwrap();
    assert!(
        fastpq_prover::goldilocks_transform::metal_goldilocks_transform_extra_payload_v1(rows, 8)
            .unwrap()
            <= available
    );
}
