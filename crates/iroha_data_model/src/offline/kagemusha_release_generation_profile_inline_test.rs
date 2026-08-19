#[test]
fn v4_release_generation_profile_is_exact_compact_geometry() {
    let reviewed = circuit_params();
    reviewed
        .validate_release_generation_profile()
        .expect("reviewed compact degree-17 generation profile");
    let encoded = norito::to_bytes(&reviewed).expect("encode reviewed circuit profile");
    let decoded: KagemushaStepCircuitParamsV4 =
        norito::decode_from_bytes(&encoded).expect("decode reviewed circuit profile");
    assert_eq!(decoded, reviewed);
    assert_eq!(
        norito::to_bytes(&decoded).expect("re-encode reviewed circuit profile"),
        encoded,
        "the constructor must remain a canonical Norito release input"
    );
    assert_eq!(reviewed.version, KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4);
    assert_eq!(reviewed.k, KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4);
    assert_eq!(reviewed.public_input_limbs, 66);
    assert_eq!(reviewed.max_parent_proof_bytes, 93_120);
    assert_eq!(
        KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_INITIALIZATION_BYTES_V4,
        186_852
    );
    assert_eq!(
        KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4,
        191_862
    );
    assert_eq!(KAGEMUSHA_STEP_CIRCUIT_RELEASE_ADVICE_COLUMNS_V4, [220]);
    assert_eq!(KAGEMUSHA_STEP_CIRCUIT_RELEASE_LOOKUP_COLUMNS_V4, [25, 0, 0]);
    let mut uncalibrated = reviewed.clone();
    uncalibrated.num_advice_per_phase = vec![1];
    uncalibrated.num_lookup_advice_per_phase = vec![1];
    assert!(uncalibrated.validate().is_err());
    assert!(
        uncalibrated.validate_release_generation_profile().is_err(),
        "uncalibrated geometry must not authorize release generation"
    );
    let mut uncalibrated_proof = reviewed.clone();
    uncalibrated_proof.max_parent_proof_bytes += 1;
    assert!(uncalibrated_proof.validate().is_ok());
    assert!(
        uncalibrated_proof
            .validate_release_generation_profile()
            .is_err(),
        "uncalibrated proof length must not authorize release generation"
    );
    let mut phantom_phase = reviewed.clone();
    phantom_phase.num_advice_per_phase.push(1);
    phantom_phase.num_lookup_advice_per_phase.push(0);
    assert!(
        phantom_phase.validate().is_err(),
        "Kagemusha must reject an unconstrained speculative advice phase"
    );
    let mut unreviewed_degree = reviewed;
    unreviewed_degree.k = KAGEMUSHA_STEP_CIRCUIT_MAXIMUM_K_V4 + 1;
    unreviewed_degree.lookup_bits = unreviewed_degree.k - 1;
    assert!(unreviewed_degree.validate().is_err());
    assert!(
        unreviewed_degree
            .validate_release_generation_profile()
            .is_err()
    );
}
