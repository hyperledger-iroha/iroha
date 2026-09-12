//! Evidence viewer operation handlers for authenticated broker dispatch.

use super::*;

pub(super) fn evidence_viewer_issue_challenge(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let issue = decode_canonical::<EvidenceViewerIssueChallengeRequestWireV1>(
        &request.payload,
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )?;
    let secret = broker_backend!(state, evidence_viewer_webauthn)
        .issue_challenge(issue.binding_digest, issue.expires_at_unix_ms)
        .map_err(|error| match error {
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                BrokerError::Rejected
            }
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
            | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                BrokerError::Ambiguous
            }
        })?;
    let result = EvidenceViewerSecretResultWireV1 {
        secret: secret.expose().as_bytes().to_vec(),
    };
    validate_evidence_viewer_secret(&result.secret)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&result, MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
}

pub(super) fn evidence_viewer_verify_and_consume(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let verify = decode_canonical::<EvidenceViewerVerifyAndConsumeRequestWireV1>(
        &request.payload,
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )?;
    let configured = required_binding_ref!(&request.binding, evidence_viewer_webauthn_binding);
    validate_evidence_viewer_verify_and_consume_wire(&verify, configured)?;
    let challenge = validate_evidence_viewer_secret(&verify.challenge)?;
    let result = broker_backend!(state, evidence_viewer_webauthn)
        .verify_and_consume(
            challenge,
            &verify.assertion,
            verify.binding_digest,
            &verify.rp_id,
            &verify.allowed_origins,
            verify.now_unix_ms,
        )
        .map_err(|error| match error {
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                BrokerError::Rejected
            }
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
            | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                BrokerError::Ambiguous
            }
        })?;
    if result.attestation_digest == [0; 32] || result.credential_id_digest == [0; 32] {
        return Err(BrokerError::Rejected);
    }
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(
        &EvidenceViewerWebAuthnResultWireV1 {
            attestation_digest: result.attestation_digest,
            credential_id_digest: result.credential_id_digest,
            authenticator_counter: result.authenticator_counter,
        },
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )
}

pub(super) fn evidence_viewer_grant_issue(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let issue = decode_canonical::<EvidenceViewerGrantIssueRequestWireV1>(
        &request.payload,
        MAX_EVIDENCE_VIEWER_CLAIMS_BYTES_V1,
    )?;
    let secret = broker_backend!(state, evidence_viewer_grants)
        .issue(&issue.claims)
        .map_err(|error| match error {
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                BrokerError::Rejected
            }
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
            | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                BrokerError::Ambiguous
            }
        })?;
    let result = EvidenceViewerSecretResultWireV1 {
        secret: secret.expose().as_bytes().to_vec(),
    };
    validate_evidence_viewer_secret(&result.secret)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&result, MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
}

pub(super) fn evidence_viewer_grant_verify(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let verify = decode_canonical::<EvidenceViewerGrantVerifyRequestWireV1>(
        &request.payload,
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )?;
    let token = validate_evidence_viewer_secret(&verify.token)?;
    broker_backend!(state, evidence_viewer_grants)
        .verify(token, &verify.claims, verify.now_unix_ms)
        .map_err(|error| match error {
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                BrokerError::Rejected
            }
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
            | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                BrokerError::Unavailable
            }
        })?;
    requalify()?;
    encode_canonical(&(), MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
}

pub(super) fn evidence_viewer_grant_revoke(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let revoke = decode_canonical::<EvidenceViewerGrantRevokeRequestWireV1>(
        &request.payload,
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )?;
    broker_backend!(state, evidence_viewer_grants)
        .revoke(revoke.token_digest)
        .map_err(|error| match error {
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                BrokerError::Rejected
            }
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
            | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                BrokerError::Ambiguous
            }
        })?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&(), MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
}

pub(super) fn evidence_viewer_receipt_sign(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let sign = decode_canonical::<PurposeSignRequestWireV1>(
        &request.payload,
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )?;
    let purpose = validate_evidence_purpose_signing_request(&sign, &request.binding)?;
    let signer = broker_backend!(state, evidence_viewer_receipt_signer);
    let signature = signer
        .sign(purpose, &sign.payload)
        .map_err(|error| match error {
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                BrokerError::Rejected
            }
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
            | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                BrokerError::Unavailable
            }
        })?;
    let public_key =
        required_binding_value!(&request.binding, evidence_viewer_receipt_signer_public_key);
    verify_evidence_viewer_ed25519_signature(public_key, signature, &sign.payload)?;
    requalify()?;
    encode_canonical(
        &SignResultWireV1 { signature },
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )
}

pub(super) fn evidence_viewer_erase(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let erase = decode_canonical::<EvidenceViewerEraseRequestWireV1>(
        &request.payload,
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )?;
    let commit_digest = broker_backend!(state, evidence_viewer_erasure)
        .erase(
            erase.operation_id,
            erase.quarantine_id,
            erase.object_id,
            erase.evidence_digest,
        )
        .map_err(|error| match error {
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                BrokerError::Rejected
            }
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
            | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                BrokerError::Ambiguous
            }
        })?;
    if commit_digest == [0; 32] {
        return Err(BrokerError::Rejected);
    }
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(
        &EvidenceViewerEraseResultWireV1 { commit_digest },
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )
}

pub(super) fn evidence_viewer_checkpoint_load(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let store = broker_backend!(state, evidence_viewer_checkpoint_store);
    let record = store.load_latest().map_err(|error| match error {
        sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreExternalErrorV1::Unavailable
        | sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreExternalErrorV1::Ambiguous => {
            BrokerError::Unavailable
        }
        sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreExternalErrorV1::Rejected => {
            BrokerError::Rejected
        }
    })?;
    let record = record
        .map(|record| {
            let bytes = encode_canonical(
                &record,
                evidence_viewer_checkpoint_record_limit(&request.binding)?,
            )?;
            decode_evidence_viewer_checkpoint_record(&bytes, &request.binding)?;
            Ok(bytes)
        })
        .transpose()?;
    requalify()?;
    encode_canonical(&record, MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1)
}

pub(super) fn evidence_viewer_checkpoint_compare_and_swap(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let compare = decode_canonical::<EvidenceViewerCheckpointCompareAndSwapRequestWireV1>(
        &request.payload,
        MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
    )?;
    let next = decode_evidence_viewer_checkpoint_record(&compare.next_record, &request.binding)?;
    let store = broker_backend!(state, evidence_viewer_checkpoint_store);
    let current = store.load_latest().map_err(|error| match error {
        sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreExternalErrorV1::Unavailable
        | sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreExternalErrorV1::Ambiguous => {
            BrokerError::Unavailable
        }
        sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreExternalErrorV1::Rejected => {
            BrokerError::Rejected
        }
    })?;
    if let Some(current) = current.as_ref() {
        let current_bytes = encode_canonical(
            current,
            evidence_viewer_checkpoint_record_limit(&request.binding)?,
        )?;
        decode_evidence_viewer_checkpoint_record(&current_bytes, &request.binding)
            .map_err(|_| BrokerError::Protocol)?;
    }
    validate_evidence_viewer_checkpoint_successor(
        current.as_ref(),
        compare.expected_revision,
        &next,
    )?;
    store
        .compare_and_swap_latest(compare.expected_revision, &next)
        .map_err(|error| {
            match error {
        sorafs_node::evidence_viewer::
            EvidenceViewerCheckpointStoreExternalErrorV1::Rejected => {
            BrokerError::Conflict
        }
        sorafs_node::evidence_viewer::
            EvidenceViewerCheckpointStoreExternalErrorV1::Unavailable
        | sorafs_node::evidence_viewer::
            EvidenceViewerCheckpointStoreExternalErrorV1::Ambiguous => {
            BrokerError::Ambiguous
        }
    }
        })?;
    let readback = store.load_latest().map_err(|_| BrokerError::Ambiguous)?;
    if readback.as_ref() != Some(&next) {
        return Err(BrokerError::Ambiguous);
    }
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&(), MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
}

pub(super) fn evidence_viewer_archive_install(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let install = decode_canonical::<EvidenceViewerArchiveInstallRequestWireV1>(
        &request.payload,
        MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
    )?;
    let archive = broker_backend!(state, evidence_viewer_compaction_archive);
    let signature = archive
        .install(
            install.operation_id,
            install.receipt_message,
            &install.canonical_artifact,
        )
        .map_err(|error| match error {
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                BrokerError::Rejected
            }
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
            | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                BrokerError::Ambiguous
            }
        })?;
    let public_key = required_binding_value!(&request.binding, evidence_viewer_archive_public_key);
    verify_evidence_viewer_ed25519_signature(public_key, signature, &install.receipt_message)?;
    let readback = archive
        .read(install.operation_id)
        .map_err(|_| BrokerError::Ambiguous)?
        .ok_or(BrokerError::Ambiguous)?;
    if readback.canonical_artifact != install.canonical_artifact || readback.signature != signature
    {
        return Err(BrokerError::Ambiguous);
    }
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(
        &SignResultWireV1 { signature },
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )
}

pub(super) fn evidence_viewer_archive_read(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let read = decode_canonical::<EvidenceViewerArchiveReadRequestWireV1>(
        &request.payload,
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )?;
    let max_bytes = usize::try_from(required_binding_value!(
        &request.binding,
        evidence_viewer_archive_max_bytes
    ))
    .map_err(|_| BrokerError::Rejected)?;
    let readback = broker_backend!(state, evidence_viewer_compaction_archive)
        .read(read.operation_id)
        .map_err(|error| match error {
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                BrokerError::Rejected
            }
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
            | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                BrokerError::Unavailable
            }
        })?
        .map(|readback| {
            if readback.canonical_artifact.is_empty()
                || readback.canonical_artifact.len() > max_bytes
                || readback.signature == [0; 64]
            {
                return Err(BrokerError::Rejected);
            }
            Ok(EvidenceViewerArchiveReadbackWireV1 {
                canonical_artifact: readback.canonical_artifact,
                signature: readback.signature,
            })
        })
        .transpose()?;
    requalify()?;
    encode_canonical(&readback, MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1)
}

pub(super) fn evidence_viewer_transparency_load(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let publisher = broker_backend!(state, evidence_viewer_transparency_publisher);
    let head = publisher.load_head().map_err(|error| {
        match error {
        sorafs_node::evidence_viewer::transparency_producer::
            EvidenceViewerTransparencyPublisherExternalErrorV1::Unavailable
        | sorafs_node::evidence_viewer::transparency_producer::
            EvidenceViewerTransparencyPublisherExternalErrorV1::Backpressure
        | sorafs_node::evidence_viewer::transparency_producer::
            EvidenceViewerTransparencyPublisherExternalErrorV1::Ambiguous => {
            BrokerError::Unavailable
        }
        sorafs_node::evidence_viewer::transparency_producer::
            EvidenceViewerTransparencyPublisherExternalErrorV1::Rejected => {
            BrokerError::Rejected
        }
    }
    })?;
    if let Some(head) = head.as_ref() {
        validate_evidence_viewer_transparency_head_body(&head.body, &request.binding)?;
        if head.signature == [0; 64] || head.head_digest == [0; 32] {
            return Err(BrokerError::Rejected);
        }
    }
    requalify()?;
    encode_canonical(&head, MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1)
}

pub(super) fn evidence_viewer_transparency_compare_and_publish(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let body = decode_canonical::<
        sorafs_node::evidence_viewer::transparency_producer::EvidenceViewerTransparencyHeadBodyV1,
    >(&request.payload, MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1)?;
    validate_evidence_viewer_transparency_head_body(&body, &request.binding)?;
    let publisher = broker_backend!(state, evidence_viewer_transparency_publisher);
    let publish_result = publisher.compare_and_publish(&body);
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    publish_result.map_err(|error| {
        match error {
        sorafs_node::evidence_viewer::transparency_producer::
            EvidenceViewerTransparencyPublisherExternalErrorV1::Rejected => {
            BrokerError::Rejected
        }
        sorafs_node::evidence_viewer::transparency_producer::
            EvidenceViewerTransparencyPublisherExternalErrorV1::Unavailable
        | sorafs_node::evidence_viewer::transparency_producer::
            EvidenceViewerTransparencyPublisherExternalErrorV1::Backpressure => {
            BrokerError::Unavailable
        }
        sorafs_node::evidence_viewer::transparency_producer::
            EvidenceViewerTransparencyPublisherExternalErrorV1::Ambiguous => {
            BrokerError::Ambiguous
        }
    }
    })?;
    encode_canonical(&(), MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
}
