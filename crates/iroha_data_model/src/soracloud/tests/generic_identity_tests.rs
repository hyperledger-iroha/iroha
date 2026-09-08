// Immutable identities for the borrowed FHE signing-preimage owner.

#[cfg(feature = "json")]
#[test]
fn fhe_provenance_identity_frames_match_capture() {
    use crate::generic_identity_tests::encoded_record;

    let first = sample_fhe_full_bootstrap_execution_proof();
    let second = sample_fhe_full_bootstrap_execution_proof_with_statement(sample_hash(22));
    let cases = [
        ("without-proofs", None, None, Vec::new()),
        (
            "public-key-proof",
            Some(sample_fhe_public_key_proof()),
            None,
            Vec::new(),
        ),
        (
            "bootstrap-key-proof",
            None,
            Some(sample_fhe_bootstrap_key_proof()),
            Vec::new(),
        ),
        (
            "ordered-execution-proofs",
            Some(sample_fhe_public_key_proof()),
            Some(sample_fhe_bootstrap_key_proof()),
            vec![first.clone(), second.clone()],
        ),
        (
            "reversed-execution-proofs",
            Some(sample_fhe_public_key_proof()),
            Some(sample_fhe_bootstrap_key_proof()),
            vec![second, first],
        ),
    ];
    let mut rows = Vec::new();
    let mut execution_order_preimages = Vec::new();
    for (
        label,
        public_key_proof,
        bootstrap_key_zero_refresh_proof,
        full_bootstrap_execution_proofs,
    ) in cases
    {
        let value = FheJobRunProvenancePayloadV1 {
            service_name: "health_portal",
            binding_name: "private_state",
            job: sample_fhe_job_spec(),
            policy_reference: sample_fhe_policy_reference(),
            public_key_proof,
            bootstrap_key_zero_refresh_proof,
            full_bootstrap_execution_proofs,
        };
        let preimage = encode_fhe_job_run_provenance_payload(
            value.service_name,
            value.binding_name,
            value.job.clone(),
            value.policy_reference.clone(),
            value.public_key_proof.clone(),
            value.bootstrap_key_zero_refresh_proof.clone(),
            value.full_bootstrap_execution_proofs.clone(),
        )
        .expect("encode exact existing FHE signing preimage");
        assert_eq!(
            norito::encode_canonical(&value).expect("frame borrowed FHE payload"),
            preimage
        );
        if value.full_bootstrap_execution_proofs.len() == 2 {
            execution_order_preimages.push(preimage.clone());
        }
        rows.push(norito::json!({
            "case": label,
            "encoding": (encoded_record(&value)),
            "production_signing_preimage_hex": (hex::encode(preimage)),
            "decode_reencode_exact": false,
            "decode_scope": "encoding-only borrowed signing preimage; no usable owned-lifetime decoder",
        }));
    }
    assert_eq!(rows.len(), 5);
    assert_eq!(execution_order_preimages.len(), 2);
    assert_ne!(execution_order_preimages[0], execution_order_preimages[1]);
    let expected: norito::json::Value = norito::json::from_str(include_str!(
        "../../../tests/fixtures/fhe_provenance_identity_frames.json"
    ))
    .expect("read immutable FHE signing-preimage frames");
    assert_eq!(norito::json!({"cases": rows}), expected);
}
