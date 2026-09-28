//! Whole-profile retained allocation bounds, without materializing private traces.

use super::*;

#[test]
fn whole_main_retaining_every_masked_coefficient_exceeds_the_release_memory_ceiling() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let geometry = layout
        .trace_groups
        .iter()
        .map(|group| (group.native_trace_log2, group.base_width + group.aux_width))
        .collect::<Vec<_>>();
    assert_eq!(
        geometry,
        [
            (5, 800),
            (8, 190),
            (15, 49),
            (16, 805),
            (18, 67),
            (19, 3712)
        ]
    );
    let masks = MASK_DEGREE + 1;
    assert_eq!(masks, 1816);
    let coefficient_bytes = geometry
        .iter()
        .map(|(log, width)| ((1_u64 << log) + masks as u64) * *width as u64 * 8)
        .sum::<u64>();
    let mask_bytes = geometry
        .iter()
        .map(|(_, width)| *width as u64 * masks as u64 * 8)
        .sum::<u64>();
    assert_eq!(coefficient_bytes, 16_226_947_392);
    assert_eq!(mask_bytes, 81_690_944);
    assert!(coefficient_bytes > super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1);
    // This is a payload lower bound, excluding borrowed assembly/source traces,
    // quotient matrices, public fixed polynomials, FFT scratch and allocator overhead.
    assert_eq!(((1_u64 << 19) + masks as u64) * 3712 * 8, 15_623_184_384);
}
