//! Direct required-mode transcript controls, independent of full-bootstrap execution fixtures.

use super::*;

fn required_mode_material(
    mode: BfvRefreshTranscriptModeV1,
) -> (
    BfvParameters,
    BfvEvaluationKeyBundle,
    BfvEvaluationKeyRefreshTranscriptV1,
) {
    let params = ram_lfe_bfv_parameters_v1();
    let seed = b"soracloud-required-mode-direct-keygen";
    let (_, public_key, relinearization_key) = match mode {
        BfvRefreshTranscriptModeV1::ExactLift => keygen_from_seed(&params, seed),
        BfvRefreshTranscriptModeV1::BoundedNoise => {
            keygen_bounded_noise_with_relinearization_from_seed(&params, seed)
        }
    }
    .expect("construct direct required-mode test key");
    let rotation_seed = b"soracloud-required-mode-direct-rotation";
    let rotation_key = match mode {
        BfvRefreshTranscriptModeV1::ExactLift => {
            rotation_key_from_seed(&params, &public_key, 1, rotation_seed)
        }
        BfvRefreshTranscriptModeV1::BoundedNoise => {
            rotation_key_bounded_noise_from_seed(&params, &public_key, 1, rotation_seed)
        }
    }
    .expect("construct mode-specific rotation transcript material");
    let keys = BfvEvaluationKeyBundle {
        relinearization_key,
        rotation_keys: vec![rotation_key],
        galois_keys: Vec::new(),
        bootstrap_key: None,
    };
    let transcript = BfvEvaluationKeyRefreshTranscriptV1 {
        public_key,
        rotation_transcripts: vec![BfvRotationRefreshTranscriptV1 {
            rotation_steps: 1,
            seed: rotation_seed.to_vec(),
        }],
        bootstrap_transcript: None,
    };
    // Exercise real canonical material bytes before calling the typed helper.
    // No successful full-bootstrap result or release-audit package is needed.
    let key_bytes = norito::encode_canonical(&keys).expect("canonical test evaluation-key bytes");
    let transcript_bytes =
        norito::encode_canonical(&transcript).expect("canonical test transcript bytes");
    let decoded_keys: BfvEvaluationKeyBundle =
        norito::decode_canonical(&key_bytes).expect("decode canonical evaluation-key bytes");
    let decoded_transcript: BfvEvaluationKeyRefreshTranscriptV1 =
        norito::decode_canonical(&transcript_bytes).expect("decode canonical transcript bytes");
    assert_eq!(decoded_keys, keys);
    assert_eq!(decoded_transcript, transcript);
    (params, decoded_keys, decoded_transcript)
}

#[test]
fn required_refresh_mode_accepts_only_the_explicit_matching_mode() {
    for actual_mode in [
        BfvRefreshTranscriptModeV1::ExactLift,
        BfvRefreshTranscriptModeV1::BoundedNoise,
    ] {
        let (params, keys, transcript) = required_mode_material(actual_mode);
        validate_soracloud_fhe_full_bootstrap_release_audit_refresh_transcript_v1(
            "direct required-mode control",
            &params,
            &keys,
            &transcript,
            actual_mode,
        )
        .expect("matching explicit mode replays the canonical transcript");
        let (wrong_mode, label) = match actual_mode {
            BfvRefreshTranscriptModeV1::ExactLift => {
                (BfvRefreshTranscriptModeV1::BoundedNoise, "bounded-noise")
            }
            BfvRefreshTranscriptModeV1::BoundedNoise => {
                (BfvRefreshTranscriptModeV1::ExactLift, "exact-lift")
            }
        };
        let error = validate_soracloud_fhe_full_bootstrap_release_audit_refresh_transcript_v1(
            "direct required-mode control",
            &params,
            &keys,
            &transcript,
            wrong_mode,
        )
        .expect_err("a mismatched explicit mode must not retry the other mode");
        assert_invalid_parameter_contains(
            error,
            &format!("direct required-mode control refresh transcript failed {label} validation"),
        );
    }
}

#[test]
fn required_refresh_mode_rejects_malformed_transcripts_and_key_shapes() {
    for mode in [
        BfvRefreshTranscriptModeV1::ExactLift,
        BfvRefreshTranscriptModeV1::BoundedNoise,
    ] {
        let (params, keys, transcript) = required_mode_material(mode);
        let label = match mode {
            BfvRefreshTranscriptModeV1::ExactLift => "exact-lift",
            BfvRefreshTranscriptModeV1::BoundedNoise => "bounded-noise",
        };
        for seed in [
            Vec::new(),
            vec![0; BFV_REFRESH_TRANSCRIPT_SEED_MAX_BYTES],
            vec![1; BFV_REFRESH_TRANSCRIPT_SEED_MAX_BYTES + 1],
        ] {
            let mut malformed = transcript.clone();
            malformed.rotation_transcripts[0].seed = seed;
            let error = validate_soracloud_fhe_full_bootstrap_release_audit_refresh_transcript_v1(
                "direct required-mode control",
                &params,
                &keys,
                &malformed,
                mode,
            )
            .expect_err("malformed transcript inventory cannot pass the typed helper");
            assert_invalid_parameter_contains(error.clone(), "rotation_transcripts.seed");
            assert_invalid_parameter_contains(
                error,
                &format!("refresh transcript failed {label} validation"),
            );
        }
        let mut malformed_keys = keys.clone();
        malformed_keys.rotation_keys[0]
            .zero_refresh
            .c0
            .pop()
            .expect("valid rotation material has nonempty coefficients");
        let error = validate_soracloud_fhe_full_bootstrap_release_audit_refresh_transcript_v1(
            "direct required-mode control",
            &params,
            &malformed_keys,
            &transcript,
            mode,
        )
        .expect_err("malformed rotation ciphertext shape cannot pass transcript replay");
        assert_invalid_parameter_contains(
            error,
            &format!("refresh transcript failed {label} validation"),
        );
    }
}
