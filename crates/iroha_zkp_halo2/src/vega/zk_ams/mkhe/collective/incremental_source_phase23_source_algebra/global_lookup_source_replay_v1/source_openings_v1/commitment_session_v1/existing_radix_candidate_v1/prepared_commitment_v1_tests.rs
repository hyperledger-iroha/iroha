//! Actual prepared-value adoption uses its own session rho and exact point.
use super::*;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::TestPreparedLowDigitV1;

#[test]
fn actual_nonzero_values_adopt_matching_msm_and_retain_same_sampled_blinding() {
    let _guard = crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::prepared_commitment_test_guard_v1();
    let fixture = TestPreparedLowDigitV1::new_v1();
    let session = super::super::tests::source_complete_session_v1(
        [0x31; 32],
        [0x41; 32],
        TestEntropyFaultV1::None,
    );
    let mut assembly = session.into_existing_radix_candidate_assembly_v1().unwrap();
    assert!(!assembly.all_prepared_values_committed_v1().unwrap());
    assembly = assembly
        .commit_prepared_values_v1(&fixture.statement_v1(0))
        .unwrap();
    let live = assembly.live.as_ref().unwrap();
    assert_eq!(live.next_wire_ordinal, 1);
    assert!(live.pending.is_none());
    assert_eq!(live.blindings.len(), 1);
    let rho = live.blindings.as_slice()[0];
    assert_eq!(
        rho.to_be_bytes(),
        hex_literal::hex!("236d3b8112318b84c14cb221111bf3e0fc756c0e8fc09c2d9c143499de6e1b2e")
    );
    let ticket = live.session.live.as_ref().unwrap().inventory.slots[344]
        .as_ref()
        .unwrap();
    assert_eq!(
        ticket.point_wire,
        TestPreparedLowDigitV1::expected_commitment_v1(rho)
            .to_non_identity_wire_bytes()
            .unwrap()
    );
    assert!(!assembly.all_prepared_values_committed_v1().unwrap());
    assert!(assembly.finish_v1().is_err());
}
