#[test]
fn decryption_request_validate_for_policy_accepts_consistent_payload() {
    let policy = sample_decryption_authority_policy();
    let request = sample_decryption_request();
    assert!(
        request.validate_for_policy(&policy).is_ok(),
        "consistent decryption request should pass policy admission checks"
    );
}
#[test]
fn decryption_request_validate_for_policy_rejects_jurisdiction_mismatch() {
    let policy = sample_decryption_authority_policy();
    let mut request = sample_decryption_request();
    request.jurisdiction_tag = "eu_gdpr".to_string();
    let error = request
        .validate_for_policy(&policy)
        .expect_err("jurisdiction mismatch should fail policy admission");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "jurisdiction_tag",
            ..
        }
    ));
}
#[test]
fn decryption_request_validate_for_policy_rejects_missing_consent_evidence() {
    let policy = sample_decryption_authority_policy();
    let mut request = sample_decryption_request();
    request.consent_evidence_hash = None;
    let error = request
        .validate_for_policy(&policy)
        .expect_err("missing consent evidence should fail when policy requires it");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "consent_evidence_hash",
            ..
        }
    ));
}
#[test]
fn decryption_request_validate_rejects_break_glass_without_reason() {
    let mut request = sample_decryption_request();
    request.break_glass = true;
    request.break_glass_reason = None;
    let error = request
        .validate()
        .expect_err("break-glass request without reason must fail");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "break_glass_reason",
            ..
        }
    ));
}
#[test]
fn decryption_request_validate_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    let mut request = sample_decryption_request();
    request.ciphertext_commitment = zero_digest;
    let error = request
        .validate()
        .expect_err("ciphertext commitment placeholder must fail admission");
    assert_zero_prehash_digest_error(&error, "ciphertext_commitment");
    let mut request = sample_decryption_request();
    request.consent_evidence_hash = Some(zero_digest);
    let error = request
        .validate()
        .expect_err("consent evidence hash placeholder must fail admission");
    assert_zero_prehash_digest_error(&error, "consent_evidence_hash");
    let mut request = sample_decryption_request();
    request.governance_tx_hash = zero_digest;
    let error = request
        .validate()
        .expect_err("governance transaction hash placeholder must fail admission");
    assert_zero_prehash_digest_error(&error, "governance_tx_hash");
}
#[test]
fn ciphertext_query_spec_validate_rejects_max_results_over_limit() {
    let mut spec = sample_ciphertext_query_spec();
    spec.max_results = NonZeroU16::new(500).expect("nonzero");
    let error = spec
        .validate()
        .expect_err("max_results above deterministic bound must fail");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "max_results",
            ..
        }
    ));
}
#[test]
fn ciphertext_query_response_validate_rejects_standard_projection_without_state_key() {
    let mut response = sample_ciphertext_query_response();
    response.metadata_level = CiphertextQueryMetadataLevelV1::Standard;
    let error = response
        .validate()
        .expect_err("standard projection must require state keys");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "results.state_key",
            ..
        }
    ));
}
#[test]
fn ciphertext_query_response_validate_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    let mut response = sample_ciphertext_query_response();
    response.query_hash = zero_digest;
    let error = response
        .validate()
        .expect_err("query hash placeholder must fail admission");
    assert_zero_prehash_digest_error(&error, "query_hash");
    let mut response = sample_ciphertext_query_response();
    response.results[0].state_key_digest = zero_digest;
    let error = response
        .validate()
        .expect_err("state key digest placeholder must fail admission");
    assert_zero_prehash_digest_error(&error, "state_key_digest");
    let mut response = sample_ciphertext_query_response();
    response.results[0].ciphertext_commitment = zero_digest;
    let error = response
        .validate()
        .expect_err("ciphertext commitment placeholder must fail admission");
    assert_zero_prehash_digest_error(&error, "ciphertext_commitment");
    let mut response = sample_ciphertext_query_response();
    response.results[0].governance_tx_hash = zero_digest;
    let error = response
        .validate()
        .expect_err("governance transaction hash placeholder must fail admission");
    assert_zero_prehash_digest_error(&error, "governance_tx_hash");
    let mut response = sample_ciphertext_query_response();
    response.results[0]
        .proof
        .as_mut()
        .expect("sample proof")
        .leaf_hash = zero_digest;
    let error = response
        .validate()
        .expect_err("proof leaf hash placeholder must fail admission");
    assert_zero_prehash_digest_error(&error, "leaf_hash");
    let mut response = sample_ciphertext_query_response();
    response.results[0]
        .proof
        .as_mut()
        .expect("sample proof")
        .anchor_hash = zero_digest;
    let error = response
        .validate()
        .expect_err("proof anchor hash placeholder must fail admission");
    assert_zero_prehash_digest_error(&error, "anchor_hash");
}
#[test]
fn ciphertext_query_response_validate_accepts_minimal_projection_with_proof() {
    let spec = sample_ciphertext_query_spec();
    let response = sample_ciphertext_query_response();
    assert!(
        spec.validate().is_ok(),
        "consistent ciphertext query spec should validate"
    );
    assert!(
        response.validate().is_ok(),
        "consistent ciphertext query response should validate"
    );
}
#[test]
fn soracloud_host_request_envelope_validate_rejects_payload_operation_mismatch() {
    let request = SoracloudHostRequestEnvelopeV1 {
        schema_version: SORACLOUD_HOST_REQUEST_VERSION_V1,
        operation: SoracloudHostOperationV1::ReadConfig,
        payload: SoracloudHostRequestPayloadV1::EgressFetch(SoracloudEgressFetchRequestV1 {
            url: "https://example.invalid/data".to_string(),
            max_bytes: 1024,
            expected_hash: None,
        }),
    };
    let error = request
        .validate()
        .expect_err("operation must match payload type");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "soracloud host request envelope",
            field: "operation",
            ..
        }
    ));
}
#[test]
fn soracloud_host_request_envelope_validate_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    let request = SoracloudHostRequestEnvelopeV1 {
        schema_version: SORACLOUD_HOST_REQUEST_VERSION_V1,
        operation: SoracloudHostOperationV1::EgressFetch,
        payload: SoracloudHostRequestPayloadV1::EgressFetch(SoracloudEgressFetchRequestV1 {
            url: "https://example.invalid/data".to_string(),
            max_bytes: 1024,
            expected_hash: Some(zero_digest),
        }),
    };
    let error = request
        .validate()
        .expect_err("egress expected hash placeholder must fail admission");
    assert_zero_prehash_digest_error(&error, "expected_hash");
    let request = SoracloudHostRequestEnvelopeV1 {
        schema_version: SORACLOUD_HOST_REQUEST_VERSION_V1,
        operation: SoracloudHostOperationV1::EmitStateMutation,
        payload: SoracloudHostRequestPayloadV1::EmitStateMutation(
            SoracloudEmitStateMutationRequestV1 {
                binding_name: sample_name("state"),
                state_key: "/state/key".to_string(),
                operation: SoraStateMutationOperationV1::Upsert,
                encryption: SoraStateEncryptionV1::Plaintext,
                payload_bytes: None,
                payload: None,
                payload_commitment: Some(zero_digest),
            },
        ),
    };
    let error = request
        .validate()
        .expect_err("state mutation placeholder commitment must fail admission");
    assert_zero_prehash_digest_error(&error, "payload_commitment");
}
#[test]
fn soracloud_host_request_envelope_validate_rejects_payload_commitment_mismatch() {
    let request = SoracloudHostRequestEnvelopeV1 {
        schema_version: SORACLOUD_HOST_REQUEST_VERSION_V1,
        operation: SoracloudHostOperationV1::EmitStateMutation,
        payload: SoracloudHostRequestPayloadV1::EmitStateMutation(
            SoracloudEmitStateMutationRequestV1 {
                binding_name: sample_name("state"),
                state_key: "/state/key".to_string(),
                operation: SoraStateMutationOperationV1::Upsert,
                encryption: SoraStateEncryptionV1::Plaintext,
                payload_bytes: Some(3),
                payload: Some(b"abc".to_vec()),
                payload_commitment: Some(sample_hash(44)),
            },
        ),
    };
    let error = request
        .validate()
        .expect_err("payload commitment must match payload bytes");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "payload_commitment",
            ..
        }
    ));
}
#[test]
fn soracloud_host_response_envelope_validate_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    macro_rules! assert_response_digest_rejects {
        ($field:literal, $operation:expr, $payload:expr) => {{
            let response = SoracloudHostResponseEnvelopeV1 {
                schema_version: SORACLOUD_HOST_RESPONSE_VERSION_V1,
                operation: $operation,
                payload: $payload,
            };
            let error = response
                .validate()
                .expect_err("host response placeholder digest must fail admission");
            assert_zero_prehash_digest_error(&error, $field);
        }};
    }
    assert_response_digest_rejects!(
        "mutation_commitment",
        SoracloudHostOperationV1::EmitStateMutation,
        SoracloudHostResponsePayloadV1::EmitStateMutation(SoracloudEmitStateMutationResponseV1 {
            mutation_commitment: zero_digest,
        },)
    );
    assert_response_digest_rejects!(
        "message_id",
        SoracloudHostOperationV1::EmitMailboxMessage,
        SoracloudHostResponsePayloadV1::EmitMailboxMessage(SoracloudEmitMailboxMessageResponseV1 {
            message_id: zero_digest,
            payload_commitment: sample_hash(45),
        },)
    );
    assert_response_digest_rejects!(
        "payload_commitment",
        SoracloudHostOperationV1::EmitMailboxMessage,
        SoracloudHostResponsePayloadV1::EmitMailboxMessage(SoracloudEmitMailboxMessageResponseV1 {
            message_id: sample_hash(46),
            payload_commitment: zero_digest,
        },)
    );
    assert_response_digest_rejects!(
        "artifact_hash",
        SoracloudHostOperationV1::AppendJournal,
        SoracloudHostResponsePayloadV1::AppendJournal(SoracloudAppendJournalResponseV1 {
            artifact_hash: zero_digest,
        })
    );
    assert_response_digest_rejects!(
        "artifact_hash",
        SoracloudHostOperationV1::PublishCheckpoint,
        SoracloudHostResponsePayloadV1::PublishCheckpoint(SoracloudPublishCheckpointResponseV1 {
            artifact_hash: zero_digest,
        },)
    );
    assert_response_digest_rejects!(
        "body_hash",
        SoracloudHostOperationV1::EgressFetch,
        SoracloudHostResponsePayloadV1::EgressFetch(SoracloudEgressFetchResponseV1 {
            status_code: 200,
            content_type: Some("application/octet-stream".to_string()),
            body: b"abc".to_vec(),
            body_hash: zero_digest,
        })
    );
}
#[test]
fn soracloud_host_response_envelope_validate_rejects_body_hash_mismatch() {
    let response = SoracloudHostResponseEnvelopeV1 {
        schema_version: SORACLOUD_HOST_RESPONSE_VERSION_V1,
        operation: SoracloudHostOperationV1::EgressFetch,
        payload: SoracloudHostResponsePayloadV1::EgressFetch(SoracloudEgressFetchResponseV1 {
            status_code: 200,
            content_type: Some("application/octet-stream".to_string()),
            body: b"abc".to_vec(),
            body_hash: sample_hash(47),
        }),
    };
    let error = response
        .validate()
        .expect_err("egress body hash must match response body");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "body_hash",
            ..
        }
    ));
}
#[test]
fn secret_envelope_validate_rejects_empty_ciphertext() {
    let mut envelope = sample_secret_envelope();
    envelope.ciphertext.clear();
    let error = envelope
        .validate()
        .expect_err("secret envelope without ciphertext must fail");
    assert!(matches!(
        error,
        SoracloudManifestError::EmptyField {
            field: "ciphertext",
            ..
        }
    ));
}
#[test]
fn secret_envelope_validate_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    let mut commitment = sample_secret_envelope();
    commitment.commitment = zero_digest;
    let error = commitment
        .validate()
        .expect_err("secret commitment placeholder must fail admission");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "commitment",
            ..
        }
    ));
    assert!(error.to_string().contains("zero prehash sentinel"));
    let mut aad_digest = sample_secret_envelope();
    aad_digest.aad_digest = Some(zero_digest);
    let error = aad_digest
        .validate()
        .expect_err("secret AAD digest placeholder must fail admission");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "aad_digest",
            ..
        }
    ));
    assert!(error.to_string().contains("zero prehash sentinel"));
}
#[test]
fn ciphertext_state_metadata_validate_rejects_zero_prehash_commitment_sentinel() {
    let mut record = sample_ciphertext_state_record();
    record.metadata.commitment = zero_prehash_statement_hash();
    let error = record
        .metadata
        .validate()
        .expect_err("metadata commitment placeholder must fail admission");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "commitment",
            ..
        }
    ));
    assert!(error.to_string().contains("zero prehash sentinel"));
}
#[test]
fn ciphertext_state_record_validate_rejects_payload_size_mismatch() {
    let mut record = sample_ciphertext_state_record();
    record.metadata.payload_bytes = NonZeroU64::new(1).expect("nonzero");
    let error = record
        .validate()
        .expect_err("payload size mismatch must be rejected");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "metadata.payload_bytes",
            ..
        }
    ));
}
#[test]
fn ciphertext_state_record_validate_accepts_consistent_record() {
    let record = sample_ciphertext_state_record();
    assert!(
        record.validate().is_ok(),
        "consistent ciphertext state record must validate"
    );
}
#[test]
fn training_job_record_validation_accepts_consistent_state() {
    sample_training_job_record()
        .validate()
        .expect("valid record");
}
#[test]
fn training_job_record_validation_rejects_zero_prehash_metrics_hash_sentinel() {
    let mut record = sample_training_job_record();
    record.latest_metrics_hash = Some(zero_prehash_statement_hash());
    let error = record
        .validate()
        .expect_err("training metrics placeholder digest must fail admission");
    assert_zero_prehash_digest_error(&error, "latest_metrics_hash");
}
#[test]
fn training_job_audit_event_validation_accepts_consistent_state() {
    sample_training_job_audit_event()
        .validate()
        .expect("valid audit event");
}
#[test]
fn training_job_audit_event_validation_rejects_zero_prehash_metrics_hash_sentinel() {
    let mut event = sample_training_job_audit_event();
    event.latest_metrics_hash = Some(zero_prehash_statement_hash());
    let error = event
        .validate()
        .expect_err("training audit metrics placeholder digest must fail admission");
    assert_zero_prehash_digest_error(&error, "latest_metrics_hash");
}
#[test]
fn model_registry_validation_accepts_consistent_state() {
    sample_model_registry().validate().expect("valid registry");
}
#[test]
fn model_weight_version_validation_rejects_partial_promotion_metadata() {
    let mut record = sample_model_weight_version_record();
    record.promoted_by = None;
    let error = record.validate().expect_err("must reject partial metadata");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora model weight version record",
            field: "promotion_metadata",
            ..
        }
    ));
}
#[test]
fn model_weight_version_validation_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    macro_rules! assert_weight_digest_rejects {
        ($field:literal, $assign:expr) => {{
            let mut record = sample_model_weight_version_record();
            $assign(&mut record, zero_digest);
            let error = record
                .validate()
                .expect_err("weight-version placeholder digest must fail admission");
            assert!(matches!(
                error,
                SoracloudManifestError::InvalidField {
                    manifest: "sora model weight version record",
                    field: $field,
                    ..
                }
            ));
            assert!(error.to_string().contains("zero prehash sentinel"));
        }};
    }
    assert_weight_digest_rejects!(
        "weight_artifact_hash",
        |record: &mut SoraModelWeightVersionRecordV1, value| {
            record.weight_artifact_hash = value;
        }
    );
    assert_weight_digest_rejects!(
        "training_config_hash",
        |record: &mut SoraModelWeightVersionRecordV1, value| {
            record.training_config_hash = value;
        }
    );
    assert_weight_digest_rejects!(
        "reproducibility_hash",
        |record: &mut SoraModelWeightVersionRecordV1, value| {
            record.reproducibility_hash = value;
        }
    );
    assert_weight_digest_rejects!(
        "provenance_attestation_hash",
        |record: &mut SoraModelWeightVersionRecordV1, value| {
            record.provenance_attestation_hash = value;
        }
    );
    assert_weight_digest_rejects!(
        "gate_report_hash",
        |record: &mut SoraModelWeightVersionRecordV1, value| {
            record.gate_report_hash = Some(value);
        }
    );
}
#[test]
fn model_weight_audit_event_validation_accepts_consistent_state() {
    sample_model_weight_audit_event()
        .validate()
        .expect("valid weight audit event");
}
#[test]
fn model_artifact_record_validation_accepts_consistent_state() {
    sample_model_artifact_record()
        .validate()
        .expect("valid artifact record");
}
#[test]
fn model_artifact_record_validation_rejects_user_upload_without_storage_metadata() {
    let mut record = sample_model_artifact_record();
    record.training_job_id.clear();
    record.source_provenance = Some(SoraModelProvenanceRefV1 {
        kind: SoraModelProvenanceKindV1::UserUpload,
        id: "upload-1".to_string(),
    });
    record.chunk_manifest_root = None;
    let error = record
        .validate()
        .expect_err("user uploads must keep SoraFS storage metadata");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora model artifact record",
            field: "chunk_manifest_root",
            ..
        }
    ));
}
#[test]
fn model_artifact_record_validation_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    macro_rules! assert_artifact_digest_rejects {
        ($field:literal, $assign:expr) => {{
            let mut record = sample_model_artifact_record();
            $assign(&mut record, zero_digest);
            let error = record
                .validate()
                .expect_err("model-artifact placeholder digest must fail admission");
            assert!(matches!(
                error,
                SoracloudManifestError::InvalidField {
                    manifest: "sora model artifact record",
                    field: $field,
                    ..
                }
            ));
            assert!(error.to_string().contains("zero prehash sentinel"));
        }};
    }
    assert_artifact_digest_rejects!(
        "weight_artifact_hash",
        |record: &mut SoraModelArtifactRecordV1, value| {
            record.weight_artifact_hash = value;
        }
    );
    assert_artifact_digest_rejects!(
        "training_config_hash",
        |record: &mut SoraModelArtifactRecordV1, value| {
            record.training_config_hash = value;
        }
    );
    assert_artifact_digest_rejects!(
        "reproducibility_hash",
        |record: &mut SoraModelArtifactRecordV1, value| {
            record.reproducibility_hash = value;
        }
    );
    assert_artifact_digest_rejects!(
        "provenance_attestation_hash",
        |record: &mut SoraModelArtifactRecordV1, value| {
            record.provenance_attestation_hash = value;
        }
    );
    assert_artifact_digest_rejects!(
        "chunk_manifest_root",
        |record: &mut SoraModelArtifactRecordV1, value| {
            record.source_provenance = Some(SoraModelProvenanceRefV1 {
                kind: SoraModelProvenanceKindV1::UserUpload,
                id: "upload-1".to_string(),
            });
            record.training_job_id.clear();
            record.chunk_manifest_root = Some(value);
        }
    );
}
#[test]
fn uploaded_model_bundle_validation_rejects_wrapped_key_recipient_mismatch() {
    let mut bundle = sample_uploaded_model_bundle();
    bundle.wrapped_bundle_key.recipient_key_id = "other-recipient".to_string();
    let error = bundle
        .validate()
        .expect_err("wrapped key must target the advertised upload recipient");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora uploaded model bundle",
            field: "wrapped_bundle_key.recipient_key_id",
            ..
        }
    ));
}
#[test]
fn uploaded_model_recipient_validation_rejects_x25519_length_drift() {
    let mut recipient = sample_uploaded_model_encryption_recipient();
    recipient.public_key_bytes = vec![7u8; 31];
    recipient.public_key_fingerprint = Hash::new(recipient.public_key_bytes.as_slice());
    let error = recipient
        .validate()
        .expect_err("X25519 upload recipient keys must be exactly 32 bytes");
    match error {
        SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason,
        } => {
            assert_eq!(manifest, "sora uploaded model encryption recipient");
            assert_eq!(field, "public_key_bytes");
            assert!(reason.contains("32 bytes"), "unexpected reason: {reason}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn uploaded_model_recipient_validation_rejects_low_order_x25519_public_key() {
    let mut recipient = sample_uploaded_model_encryption_recipient();
    recipient.public_key_bytes = vec![0u8; 32];
    recipient.public_key_fingerprint = Hash::new(recipient.public_key_bytes.as_slice());
    let error = recipient
        .validate()
        .expect_err("low-order X25519 upload recipient keys must fail closed");
    match error {
        SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason,
        } => {
            assert_eq!(manifest, "sora uploaded model encryption recipient");
            assert_eq!(field, "public_key_bytes");
            assert!(reason.contains("low-order"), "unexpected reason: {reason}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn uploaded_model_recipient_validation_rejects_zero_prehash_fingerprint_sentinel() {
    let mut recipient = sample_uploaded_model_encryption_recipient();
    recipient.public_key_fingerprint = zero_prehash_statement_hash();
    let error = recipient
        .validate()
        .expect_err("recipient fingerprint placeholder must fail admission");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora uploaded model encryption recipient",
            field: "public_key_fingerprint",
            ..
        }
    ));
    assert!(error.to_string().contains("zero prehash sentinel"));
}
#[test]
fn uploaded_model_wrapped_key_validation_rejects_x25519_length_drift() {
    let mut wrapped_key = sample_uploaded_model_wrapped_key();
    wrapped_key.ephemeral_public_key = vec![8u8; 33];
    let error = wrapped_key
        .validate()
        .expect_err("X25519 ephemeral upload keys must be exactly 32 bytes");
    match error {
        SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason,
        } => {
            assert_eq!(manifest, "sora uploaded model wrapped key");
            assert_eq!(field, "ephemeral_public_key");
            assert!(reason.contains("32 bytes"), "unexpected reason: {reason}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn uploaded_model_wrapped_key_validation_rejects_low_order_x25519_ephemeral() {
    let mut wrapped_key = sample_uploaded_model_wrapped_key();
    wrapped_key.ephemeral_public_key = vec![0u8; 32];
    let error = wrapped_key
        .validate()
        .expect_err("low-order X25519 ephemeral upload keys must fail closed");
    match error {
        SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason,
        } => {
            assert_eq!(manifest, "sora uploaded model wrapped key");
            assert_eq!(field, "ephemeral_public_key");
            assert!(reason.contains("low-order"), "unexpected reason: {reason}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn uploaded_model_wrapped_key_validation_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    let mut ciphertext_hash = sample_uploaded_model_wrapped_key();
    ciphertext_hash.ciphertext_hash = zero_digest;
    let error = ciphertext_hash
        .validate()
        .expect_err("wrapped-key ciphertext hash placeholder must fail admission");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora uploaded model wrapped key",
            field: "ciphertext_hash",
            ..
        }
    ));
    assert!(error.to_string().contains("zero prehash sentinel"));
    let mut aad_digest = sample_uploaded_model_wrapped_key();
    aad_digest.aad_digest = zero_digest;
    let error = aad_digest
        .validate()
        .expect_err("wrapped-key AAD digest placeholder must fail admission");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora uploaded model wrapped key",
            field: "aad_digest",
            ..
        }
    ));
    assert!(error.to_string().contains("zero prehash sentinel"));
}
#[test]
fn uploaded_model_bundle_validation_rejects_zero_storage_metadata() {
    let mut bundle = sample_uploaded_model_bundle();
    bundle.chunk_count = 0;
    let error = bundle
        .validate()
        .expect_err("zero storage dimensions must fail closed");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora uploaded model bundle",
            field: "chunk_count",
            ..
        }
    ));
}
#[test]
fn uploaded_model_bundle_validation_rejects_zero_prehash_root_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    let mut plaintext_root = sample_uploaded_model_bundle();
    plaintext_root.plaintext_root = zero_digest;
    let error = plaintext_root
        .validate()
        .expect_err("plaintext root placeholder must fail admission");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora uploaded model bundle",
            field: "plaintext_root",
            ..
        }
    ));
    assert!(error.to_string().contains("zero prehash sentinel"));
    let mut bundle_root = sample_uploaded_model_bundle();
    bundle_root.bundle_root = zero_digest;
    let error = bundle_root
        .validate()
        .expect_err("bundle root placeholder must fail admission");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora uploaded model bundle",
            field: "bundle_root",
            ..
        }
    ));
    assert!(error.to_string().contains("zero prehash sentinel"));
    let mut chunk_manifest_root = sample_uploaded_model_bundle();
    chunk_manifest_root.chunk_manifest_root = zero_digest;
    let error = chunk_manifest_root
        .validate()
        .expect_err("chunk manifest root placeholder must fail admission");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora uploaded model bundle",
            field: "chunk_manifest_root",
            ..
        }
    ));
    assert!(error.to_string().contains("zero prehash sentinel"));
}
#[test]
fn uploaded_model_bundle_validation_rejects_adversarial_modalities() {
    for modalities in [
        vec![" ".to_string()],
        vec!["text\nimage".to_string()],
        vec!["text".to_string(), "text".to_string()],
    ] {
        let mut bundle = sample_uploaded_model_bundle();
        bundle.modalities = modalities;
        let error = bundle
            .validate()
            .expect_err("malformed modalities must fail closed");
        assert!(matches!(
            error,
            SoracloudManifestError::EmptyField {
                manifest: "sora uploaded model bundle",
                field: "modalities",
            } | SoracloudManifestError::InvalidField {
                manifest: "sora uploaded model bundle",
                field: "modalities",
                ..
            }
        ));
    }
}
#[test]
fn private_model_artifact_ref_validation_rejects_zero_prehash_artifact_hash_sentinel() {
    let mut artifact = sample_private_model_artifact_ref("input", 0x11);
    artifact.artifact_hash = zero_prehash_statement_hash();
    let error = artifact
        .validate()
        .expect_err("private artifact hash placeholder must fail admission");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora private model artifact ref",
            field: "artifact_hash",
            ..
        }
    ));
    assert!(error.to_string().contains("zero prehash sentinel"));
}
#[test]
fn private_uploaded_model_execution_receipt_validation_rejects_zero_prehash_commitments() {
    let zero_digest = zero_prehash_statement_hash();
    macro_rules! assert_receipt_digest_rejects {
        ($field:literal, $assign:expr) => {{
            let mut receipt = sample_private_uploaded_model_execution_receipt();
            $assign(&mut receipt, zero_digest);
            let error = receipt
                .validate()
                .expect_err("private receipt placeholder commitment must fail admission");
            assert!(matches!(
                error,
                SoracloudManifestError::InvalidField {
                    manifest: "sora private uploaded model execution receipt",
                    field: $field,
                    ..
                }
            ));
            assert!(error.to_string().contains("zero prehash sentinel"));
        }};
    }
    assert_receipt_digest_rejects!(
        "receipt_id",
        |receipt: &mut SoraPrivateUploadedModelExecutionReceiptV1, value| {
            receipt.receipt_id = value;
        }
    );
    assert_receipt_digest_rejects!(
        "model_bundle_root",
        |receipt: &mut SoraPrivateUploadedModelExecutionReceiptV1, value| {
            receipt.model_bundle_root = value;
        }
    );
    assert_receipt_digest_rejects!(
        "input_commitment",
        |receipt: &mut SoraPrivateUploadedModelExecutionReceiptV1, value| {
            receipt.input_commitment = value;
        }
    );
    assert_receipt_digest_rejects!(
        "output_commitment",
        |receipt: &mut SoraPrivateUploadedModelExecutionReceiptV1, value| {
            receipt.output_commitment = value;
        }
    );
    assert_receipt_digest_rejects!(
        "request_commitment",
        |receipt: &mut SoraPrivateUploadedModelExecutionReceiptV1, value| {
            receipt.request_commitment = value;
        }
    );
    assert_receipt_digest_rejects!(
        "result_commitment",
        |receipt: &mut SoraPrivateUploadedModelExecutionReceiptV1, value| {
            receipt.result_commitment = value;
        }
    );
}
#[test]
fn private_uploaded_model_execution_receipt_round_trips_and_validates() {
    let mut receipt = sample_private_uploaded_model_execution_receipt();
    receipt.validate().expect("valid private receipt");
    let encoded = norito::to_bytes(&receipt).expect("encode private receipt");
    let decoded: SoraPrivateUploadedModelExecutionReceiptV1 =
        norito::decode_from_bytes(&encoded).expect("decode private receipt");
    assert_eq!(decoded, receipt);
    receipt.output_artifact.artifact_role = "plaintext".to_string();
    let error = receipt
        .validate()
        .expect_err("output artifact must stay role-bound");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora private uploaded model execution receipt",
            field: "output_artifact.artifact_role",
            ..
        }
    ));
}

#[test]
fn private_uploaded_model_execution_receipt_requires_canonical_commitments_and_identity() {
    let canonical = sample_private_uploaded_model_execution_receipt();

    let mut substituted_runtime = canonical.clone();
    substituted_runtime.runtime_version = "soracloud.quantized-cpu.v2".to_owned();
    substituted_runtime.request_commitment =
        derive_soracloud_private_model_request_commitment_v1(&substituted_runtime);
    substituted_runtime.result_commitment =
        derive_soracloud_private_model_result_commitment_v1(&substituted_runtime);
    substituted_runtime.receipt_id =
        derive_soracloud_private_uploaded_model_execution_receipt_id_v1(&substituted_runtime);
    let error = substituted_runtime
        .validate()
        .expect_err("a substituted private runtime version must fail validation");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora private uploaded model execution receipt",
            field: "runtime_version",
            ..
        }
    ));

    let mut substituted_request = canonical.clone();
    substituted_request.request_commitment = sample_hash(0xE1);
    substituted_request.receipt_id =
        derive_soracloud_private_uploaded_model_execution_receipt_id_v1(&substituted_request);
    let error = substituted_request
        .validate()
        .expect_err("a substituted private request commitment must fail validation");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora private uploaded model execution receipt",
            field: "request_commitment",
            ..
        }
    ));

    let mut substituted_output_destination = canonical.clone();
    substituted_output_destination.output_artifact.artifact_hash = sample_hash(0xE4);
    substituted_output_destination.result_commitment =
        derive_soracloud_private_model_result_commitment_v1(&substituted_output_destination);
    substituted_output_destination.receipt_id =
        derive_soracloud_private_uploaded_model_execution_receipt_id_v1(
            &substituted_output_destination,
        );
    let error = substituted_output_destination
        .validate()
        .expect_err("a substituted private output destination must invalidate the request");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora private uploaded model execution receipt",
            field: "request_commitment",
            ..
        }
    ));

    let mut substituted_result = canonical.clone();
    substituted_result.result_commitment = sample_hash(0xE2);
    substituted_result.receipt_id =
        derive_soracloud_private_uploaded_model_execution_receipt_id_v1(&substituted_result);
    let error = substituted_result
        .validate()
        .expect_err("a substituted private result commitment must fail validation");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora private uploaded model execution receipt",
            field: "result_commitment",
            ..
        }
    ));

    let mut substituted_id = canonical.clone();
    substituted_id.receipt_id = sample_hash(0xE3);
    let error = substituted_id
        .validate()
        .expect_err("a substituted private receipt id must fail validation");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora private uploaded model execution receipt",
            field: "receipt_id",
            ..
        }
    ));

    let canonical_id = canonical.receipt_id;
    let mut submission = canonical;
    submission.emitted_sequence = 0;
    assert_eq!(
        derive_soracloud_private_uploaded_model_execution_receipt_id_v1(&submission),
        canonical_id,
        "ledger assignment must not change private receipt identity"
    );
    submission
        .validate_submission()
        .expect("the canonical identity must remain valid before sequence assignment");
}

#[test]
fn hf_source_record_validation_accepts_consistent_state() {
    sample_hf_source_record().validate().expect("valid source");
}
#[test]
fn hf_source_record_validation_requires_full_lowercase_commit_oid() {
    for mutable_or_noncanonical in [
        "main",
        "4f9d72c",
        "4F9D72C4F9D72C4F9D72C4F9D72C4F9D72C4F9D",
        "4f9d72c4f9d72c4f9d72c4f9d72c4f9d72c4f9g",
    ] {
        let mut source = sample_hf_source_record();
        source.resolved_revision = mutable_or_noncanonical.to_owned();
        let error = source
            .validate()
            .expect_err("mutable or noncanonical HF revision must fail admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                manifest: "sora hf source record",
                field: "resolved_revision",
                ..
            }
        ));
    }
}
#[test]
fn hf_source_record_validation_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    let mut source = sample_hf_source_record();
    source.source_id = zero_digest;
    let error = source
        .validate()
        .expect_err("source placeholder id must fail admission");
    assert_zero_prehash_digest_error(&error, "source_id");
    let mut source = sample_hf_source_record();
    source.normalized_runtime_hash = zero_digest;
    let error = source
        .validate()
        .expect_err("normalized runtime placeholder hash must fail admission");
    assert_zero_prehash_digest_error(&error, "normalized_runtime_hash");
}
#[test]
fn hf_shared_lease_pool_validation_accepts_consistent_state() {
    sample_hf_shared_lease_pool()
        .validate()
        .expect("valid shared lease pool");
}
#[test]
fn hf_shared_lease_pool_validation_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    let mut pool = sample_hf_shared_lease_pool();
    pool.pool_id = zero_digest;
    let error = pool
        .validate()
        .expect_err("pool placeholder id must fail admission");
    assert_zero_prehash_digest_error(&error, "pool_id");
    let mut pool = sample_hf_shared_lease_pool();
    pool.source_id = zero_digest;
    let error = pool
        .validate()
        .expect_err("source placeholder id must fail pool admission");
    assert_zero_prehash_digest_error(&error, "source_id");
}
#[test]
fn hf_shared_lease_pool_binds_queued_profile_cap_and_asset() {
    let mut pool = sample_hf_shared_lease_pool();
    let resource_profile = sample_hf_resource_profile();
    let canonical_cap =
        hf_shared_lease_max_compute_reservation_fee_v1(&resource_profile, pool.lease_term_ms)
            .expect("canonical compute cap");
    pool.queued_next_window = Some(SoraHfSharedLeaseQueuedWindowV1 {
        sponsor_account_id: sample_account_id(0xC3),
        model_name: "demo_model".to_string(),
        lease_asset_definition_id: pool.lease_asset_definition_id.clone(),
        base_fee: xor_quantity_from_nanos(15_000),
        compute_reservation_cap: canonical_cap,
        resource_profile,
        sponsored_at_ms: 20_000,
        window_started_at_ms: pool.window_expires_at_ms,
        window_expires_at_ms: pool.window_expires_at_ms.saturating_add(pool.lease_term_ms),
        service_name: sample_name("demo_service"),
        apartment_name: Some(sample_name("demo_apartment")),
    });
    pool.validate()
        .expect("canonical queued-window cap and settlement asset are valid");
    pool.queued_next_window
        .as_mut()
        .expect("queued window")
        .compute_reservation_cap = xor_quantity_from_nanos(7_999);
    let error = pool
        .validate()
        .expect_err("a non-canonical compute cap must fail");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora hf shared lease pool",
            field: "queued_next_window.compute_reservation_cap",
            ..
        }
    ));

    let mut pool = sample_hf_shared_lease_pool();
    let resource_profile = sample_hf_resource_profile();
    let compute_reservation_cap =
        hf_shared_lease_max_compute_reservation_fee_v1(&resource_profile, pool.lease_term_ms)
            .expect("canonical compute cap");
    pool.queued_next_window = Some(SoraHfSharedLeaseQueuedWindowV1 {
        sponsor_account_id: sample_account_id(0xC3),
        model_name: "demo_model".to_string(),
        lease_asset_definition_id: AssetDefinitionId::from_uuid_bytes([
            0x66, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44,
            0x00, 0x0b,
        ])
        .expect("alternate asset definition"),
        base_fee: xor_quantity_from_nanos(15_000),
        compute_reservation_cap,
        resource_profile,
        sponsored_at_ms: 20_000,
        window_started_at_ms: pool.window_expires_at_ms,
        window_expires_at_ms: pool.window_expires_at_ms.saturating_add(pool.lease_term_ms),
        service_name: sample_name("demo_service"),
        apartment_name: Some(sample_name("demo_apartment")),
    });
    let error = pool
        .validate()
        .expect_err("a queued settlement asset mismatch must fail");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora hf shared lease pool",
            field: "queued_next_window.lease_asset_definition_id",
            ..
        }
    ));
}
#[test]
fn hf_shared_lease_pool_validation_rejects_misaligned_queued_window() {
    let mut pool = sample_hf_shared_lease_pool();
    let resource_profile = sample_hf_resource_profile();
    let compute_reservation_cap =
        hf_shared_lease_max_compute_reservation_fee_v1(&resource_profile, pool.lease_term_ms)
            .expect("canonical compute cap");
    pool.queued_next_window = Some(SoraHfSharedLeaseQueuedWindowV1 {
        sponsor_account_id: sample_account_id(0xC3),
        model_name: "demo_model".to_string(),
        lease_asset_definition_id: sample_asset_definition_id("4cuvDVPuLBKJyN6dPbRQhmLh68sU"),
        base_fee: xor_quantity_from_nanos(15_000),
        compute_reservation_cap,
        resource_profile,
        sponsored_at_ms: 20_000,
        window_started_at_ms: pool.window_expires_at_ms.saturating_add(1),
        window_expires_at_ms: pool
            .window_expires_at_ms
            .saturating_add(pool.lease_term_ms)
            .saturating_add(1),
        service_name: sample_name("demo_service"),
        apartment_name: Some(sample_name("demo_apartment")),
    });
    let error = pool
        .validate()
        .expect_err("must reject queued window misalignment");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora hf shared lease pool",
            field: "queued_next_window.window_started_at_ms",
            ..
        }
    ));
}
#[test]
fn hf_shared_lease_member_validation_accepts_consistent_state() {
    sample_hf_shared_lease_member()
        .validate()
        .expect("valid shared lease member");
}
#[test]
fn hf_shared_lease_member_validation_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    let mut member = sample_hf_shared_lease_member();
    member.pool_id = zero_digest;
    let error = member
        .validate()
        .expect_err("pool placeholder id must fail member admission");
    assert_zero_prehash_digest_error(&error, "pool_id");
    let mut member = sample_hf_shared_lease_member();
    member.source_id = zero_digest;
    let error = member
        .validate()
        .expect_err("source placeholder id must fail member admission");
    assert_zero_prehash_digest_error(&error, "source_id");
}
#[test]
fn hf_shared_lease_audit_event_validation_accepts_consistent_state() {
    sample_hf_shared_lease_audit_event()
        .validate()
        .expect("valid shared lease audit event");
}
#[test]
fn hf_shared_lease_audit_event_binds_activation_failure_reason() {
    let mut event = sample_hf_shared_lease_audit_event();
    event.action = SoraHfSharedLeaseActionV1::ActivationFailed;
    let error = event
        .validate()
        .expect_err("activation failures require an explicit reason");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "failure_reason",
            ..
        }
    ));
    event.failure_reason = Some("queued sponsor could not fund compute".to_owned());
    event
        .validate()
        .expect("an activation failure with a reason is valid");

    event.action = SoraHfSharedLeaseActionV1::Activate;
    let error = event
        .validate()
        .expect_err("successful activation must not carry a failure reason");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "failure_reason",
            ..
        }
    ));
}
#[test]
fn hf_shared_lease_audit_event_validation_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    let mut event = sample_hf_shared_lease_audit_event();
    event.pool_id = zero_digest;
    let error = event
        .validate()
        .expect_err("pool placeholder id must fail audit admission");
    assert_zero_prehash_digest_error(&error, "pool_id");
    let mut event = sample_hf_shared_lease_audit_event();
    event.source_id = zero_digest;
    let error = event
        .validate()
        .expect_err("source placeholder id must fail audit admission");
    assert_zero_prehash_digest_error(&error, "source_id");
}
#[test]
fn model_host_violation_evidence_validation_accepts_consistent_state() {
    sample_model_host_violation_evidence_record()
        .validate()
        .expect("valid model host violation evidence");
}
#[test]
fn model_host_violation_evidence_validation_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    macro_rules! assert_violation_digest_rejects {
        ($field:literal, $assign:expr) => {{
            let mut record = sample_model_host_violation_evidence_record();
            $assign(&mut record, zero_digest);
            let error = record
                .validate()
                .expect_err("violation evidence placeholder digest must fail admission");
            assert_zero_prehash_digest_error(&error, $field);
        }};
    }
    assert_violation_digest_rejects!(
        "evidence_id",
        |record: &mut SoraModelHostViolationEvidenceRecordV1, value| {
            record.evidence_id = value;
        }
    );
    assert_violation_digest_rejects!(
        "placement_id",
        |record: &mut SoraModelHostViolationEvidenceRecordV1, value| {
            record.placement_id = Some(value);
        }
    );
    assert_violation_digest_rejects!(
        "pool_id",
        |record: &mut SoraModelHostViolationEvidenceRecordV1, value| {
            record.pool_id = Some(value);
        }
    );
    assert_violation_digest_rejects!(
        "source_id",
        |record: &mut SoraModelHostViolationEvidenceRecordV1, value| {
            record.source_id = Some(value);
        }
    );
    assert_violation_digest_rejects!(
        "slash_id",
        |record: &mut SoraModelHostViolationEvidenceRecordV1, value| {
            record.slash_id = Some(value);
        }
    );
}
#[test]
fn model_host_violation_evidence_validation_rejects_missing_slash_id_when_penalized() {
    let mut record = sample_model_host_violation_evidence_record();
    record.slash_id = None;
    let error = record
        .validate()
        .expect_err("must reject missing slash id for applied penalty");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora model host violation evidence record",
            field: "slash_id",
            ..
        }
    ));
}
#[test]
fn model_artifact_audit_event_validation_rejects_empty_consumed_version() {
    let mut event = sample_model_artifact_audit_event();
    event.consumed_by_version = Some(String::new());
    let error = event.validate().expect_err("must reject empty version");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "sora model artifact audit event",
            field: "consumed_by_version",
            ..
        }
    ));
}
