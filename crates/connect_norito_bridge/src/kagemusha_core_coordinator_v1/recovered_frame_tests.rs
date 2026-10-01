//! Strict recovered wire checks; opaque byte fixtures convey no account/device authority.
use super::*;
const METHOD: KagemushaCoreCoordinatorMethodV1 =
    KagemushaCoreCoordinatorMethodV1::InitialEnrollment;
fn request(fields: &[Vec<u8>]) -> Vec<u8> {
    kagemusha_core_coordinator_encode_request_v1(fields).unwrap()
}
fn response(fields: &[Vec<u8>]) -> Vec<u8> {
    kagemusha_core_coordinator_encode_response_v1(fields).unwrap()
}

#[test]
fn recovered_begin_requires_exact_closed_original_challenge_shape() {
    let begin = request(&[9_u32.to_le_bytes().to_vec()]);
    kagemusha_core_coordinator_validate_method_request_v1(METHOD, &begin).unwrap();
    let fields = vec![
        1_u64.to_le_bytes().to_vec(),
        vec![1; 16],
        vec![2; 32],
        vec![3; 16],
        vec![4; 32],
    ];
    kagemusha_core_coordinator_validate_method_response_v1(METHOD, &begin, &response(&fields))
        .unwrap();
    for index in 0..fields.len() {
        let mut changed = fields.clone();
        changed[index].clear();
        assert!(
            kagemusha_core_coordinator_validate_method_response_v1(
                METHOD,
                &begin,
                &response(&changed)
            )
            .is_err()
        );
    }
    for (index, size) in [(1, 16385), (3, 2049)] {
        let mut changed = fields.clone();
        changed[index] = vec![1; size];
        assert!(
            kagemusha_core_coordinator_validate_method_response_v1(
                METHOD,
                &begin,
                &response(&changed)
            )
            .is_err()
        );
    }
    assert!(
        kagemusha_core_coordinator_validate_method_request_v1(
            METHOD,
            &request(&[9_u32.to_le_bytes().to_vec(), vec![1]])
        )
        .is_err()
    );
    assert!(
        kagemusha_core_coordinator_validate_method_request_v1(
            METHOD,
            &request(&[12_u32.to_le_bytes().to_vec()])
        )
        .is_err()
    );
}

#[test]
fn recovered_complete_bounds_original_signatures_and_correlates_only_exact_attempt() {
    let fields = vec![
        10_u32.to_le_bytes().to_vec(),
        3_u64.to_le_bytes().to_vec(),
        vec![1; 64],
        vec![2; 128],
    ];
    let frame = request(&fields);
    kagemusha_core_coordinator_validate_method_request_v1(METHOD, &frame).unwrap();
    kagemusha_core_coordinator_validate_method_response_v1(
        METHOD,
        &frame,
        &response(&[fields[1].clone()]),
    )
    .unwrap();
    assert!(
        kagemusha_core_coordinator_validate_method_response_v1(
            METHOD,
            &frame,
            &response(&[4_u64.to_le_bytes().to_vec()])
        )
        .is_err()
    );
    for (index, bytes) in [
        (1, 0_u64.to_le_bytes().to_vec()),
        (2, vec![1; 63]),
        (3, Vec::new()),
    ] {
        let mut changed = fields.clone();
        changed[index] = bytes;
        assert!(
            kagemusha_core_coordinator_validate_method_request_v1(METHOD, &request(&changed))
                .is_err()
        );
    }
    let mut oversized = fields;
    oversized[3] = vec![1; iroha_data_model::kagemusha::KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1 + 1];
    if let Ok(frame) = kagemusha_core_coordinator_encode_request_v1(&oversized) {
        assert!(kagemusha_core_coordinator_validate_method_request_v1(METHOD, &frame).is_err());
    }
}

#[test]
fn recovered_cancel_is_exact_original_attempt_and_has_no_replacement_response() {
    let frame = request(&[11_u32.to_le_bytes().to_vec(), 9_u64.to_le_bytes().to_vec()]);
    kagemusha_core_coordinator_validate_method_request_v1(METHOD, &frame).unwrap();
    kagemusha_core_coordinator_validate_method_response_v1(METHOD, &frame, &response(&[])).unwrap();
    assert!(
        kagemusha_core_coordinator_validate_method_response_v1(
            METHOD,
            &frame,
            &response(&[vec![1]])
        )
        .is_err()
    );
    assert!(
        kagemusha_core_coordinator_validate_method_request_v1(
            METHOD,
            &request(&[11_u32.to_le_bytes().to_vec(), 0_u64.to_le_bytes().to_vec()])
        )
        .is_err()
    );
}
