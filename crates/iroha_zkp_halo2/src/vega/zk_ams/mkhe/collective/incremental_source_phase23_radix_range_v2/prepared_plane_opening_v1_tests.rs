//! Ordered value/tail lifecycle and secret-owner disposal on refused handoff.
use super::*;
use crate::vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1;

fn opening_fixture_v1(ordinal: u16) -> PreparedPlaneOpeningV1 {
    let mut values = ZeroizingT256ScalarVecV1::try_with_exact_capacity(16_384).unwrap();
    for index in 0..16_384 {
        values.push(VegaT256ScalarV1::from_u64(index));
    }
    PreparedPlaneOpeningV1::from_committed_v1(
        PreparedRadixValuesV1::from_exact_values_v1(values).unwrap(),
        PreparedPlaneOpeningTailV1::test_wire_fixture_v1(ordinal),
        ordinal,
    )
    .unwrap()
}

fn emit_values_v1(opening: &mut PreparedPlaneOpeningV1) {
    for expected in 0..32 {
        let chunk = opening.emit_next_value_chunk_v1(expected).unwrap();
        assert_eq!(chunk.len_v1(), 16_384);
        for (local, encoded) in chunk.as_slice_v1().chunks_exact(32).enumerate() {
            assert_eq!(&encoded[..30], &[0; 30]);
            assert_eq!(
                u16::from_be_bytes(encoded[30..].try_into().unwrap()),
                u16::from(expected) * 512 + local as u16
            );
        }
    }
}

#[test]
fn prepared_opening_tail_emits_after_exact_values_and_erases_vector_before_tail() {
    let mut opening = opening_fixture_v1(9_287);
    emit_values_v1(&mut opening);
    let vectors = zeroizing_t256_scalar_vec_drop_count_v1();
    let scalars = PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1();
    let tail = opening.emit_tail_v1().unwrap();
    assert_eq!(tail.len_v1(), 16_384);
    assert!(tail.as_slice_v1()[65..].iter().all(|byte| *byte == 0));
    assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), vectors + 1);
    assert_eq!(
        PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1(),
        scalars + 1
    );
    opening.finish_v1().unwrap();
}

#[test]
fn prepared_opening_tail_early_tail_wrong_value_order_and_retry_consume_both_owners() {
    for early_tail in [false, true] {
        let mut opening = opening_fixture_v1(0);
        let vectors = zeroizing_t256_scalar_vec_drop_count_v1();
        let scalars = PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1();
        if early_tail {
            assert!(opening.emit_tail_v1().is_err());
        } else {
            assert!(opening.emit_next_value_chunk_v1(1).is_err());
        }
        assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), vectors + 1);
        assert_eq!(
            PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1(),
            scalars + 1
        );
        assert!(opening.emit_next_value_chunk_v1(0).is_err());
        assert!(opening.emit_tail_v1().is_err());
        assert!(opening.finish_v1().is_err());
    }
}

#[test]
fn prepared_opening_tail_missing_tail_repeated_tail_and_post_tail_values_refuse_handoff() {
    for fault in 0..3 {
        let mut opening = opening_fixture_v1(7_224);
        emit_values_v1(&mut opening);
        if fault == 0 {
            let scalars = PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1();
            assert!(opening.finish_v1().is_err());
            assert_eq!(
                PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1(),
                scalars + 1
            );
            continue;
        }
        drop(opening.emit_tail_v1().unwrap());
        if fault == 1 {
            assert!(opening.emit_tail_v1().is_err());
        } else {
            assert!(opening.emit_next_value_chunk_v1(0).is_err());
        }
        assert!(opening.finish_v1().is_err());
    }
}

#[test]
fn prepared_opening_tail_rejects_mismatched_seal_and_already_emitted_values() {
    for previously_emitted in [false, true] {
        let mut opening = opening_fixture_v1(344);
        let mut live = opening.live.take().unwrap();
        let mut values = live.values.take().unwrap();
        if previously_emitted {
            drop(values.emit_next_v1(0).unwrap());
        }
        let vectors = zeroizing_t256_scalar_vec_drop_count_v1();
        let scalars = PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1();
        assert!(
            PreparedPlaneOpeningV1::from_committed_v1(
                values,
                live.tail.take().unwrap(),
                if previously_emitted { 344 } else { 345 }
            )
            .is_err()
        );
        assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), vectors + 1);
        assert_eq!(
            PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1(),
            scalars + 1
        );
    }
}

#[test]
fn prepared_opening_tail_unwind_disposes_unemitted_values_and_rho() {
    let opening = opening_fixture_v1(6_880);
    let vectors = zeroizing_t256_scalar_vec_drop_count_v1();
    let scalars = PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _opening = opening;
            panic!("injected prepared-plane consumer unwind");
        }))
        .is_err()
    );
    assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), vectors + 1);
    assert_eq!(
        PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1(),
        scalars + 1
    );
}
