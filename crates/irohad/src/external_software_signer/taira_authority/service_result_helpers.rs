// Result, envelope, and deployment-finalization helpers for the Taira authority.
//
// This file is textually included by `service.rs`; keeping the helpers in that
// namespace preserves their private visibility and their exact call surface.

fn envelope_claims_json(
    role: TairaAuthorityRoleV1,
    request: &ParsedClientRequestV1,
    assignment: &RunAssignmentV1,
    qualification_probe_results: Option<Value>,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-envelope-claims.v1"),
    );
    object.insert("role".into(), Value::from(role.as_str()));
    object.insert(
        "replay_namespace".into(),
        Value::from(role.replay_namespace()),
    );
    object.insert(
        "operation_id".into(),
        Value::from(hex::encode(request.operation_id)),
    );
    object.insert("run_id".into(), Value::from(hex::encode(request.run_id)));
    object.insert(
        "subject_sha256".into(),
        Value::from(hex::encode(request.subject_sha256)),
    );
    object.insert(
        "artifact_manifest_sha256".into(),
        Value::from(hex::encode(request.manifest_sha256)),
    );
    object.insert(
        "issued_at_unix_millis".into(),
        Value::from(assignment.issued_at_unix_millis),
    );
    object.insert(
        "expires_at_unix_millis".into(),
        Value::from(assignment.expires_at_unix_millis),
    );
    if role == TairaAuthorityRoleV1::NativeEvidence {
        let (
            Some(controller_digest),
            Some(controller_host_id),
            Some(controller_installation_id),
            Some(run_nonce),
        ) = (
            assignment.controller_digest,
            assignment.controller_host_id.as_deref(),
            assignment.controller_installation_id.as_deref(),
            assignment.run_nonce,
        )
        else {
            return Err(TairaAuthorityErrorV1::State);
        };
        object.insert(
            "controller_digest".into(),
            Value::from(hex::encode(controller_digest)),
        );
        object.insert("controller_host_id".into(), Value::from(controller_host_id));
        object.insert(
            "controller_installation_id".into(),
            Value::from(controller_installation_id),
        );
        object.insert("run_nonce".into(), Value::from(hex::encode(run_nonce)));
    } else if assignment.controller_digest.is_some()
        || assignment.controller_host_id.is_some()
        || assignment.controller_installation_id.is_some()
        || assignment.run_nonce.is_some()
    {
        return Err(TairaAuthorityErrorV1::State);
    }
    object.insert("subject".into(), request.subject.clone());
    object.insert("artifact_manifest".into(), request.manifest_value.clone());
    if let Some(probe_results) = qualification_probe_results {
        let mut role_result = Map::new();
        role_result.insert("probe_results".into(), probe_results);
        object.insert("role_result".into(), Value::Object(role_result));
    }
    canonical_json_line(&Value::Object(object))
}

fn authority_envelope_json(
    role: TairaAuthorityRoleV1,
    claims_json: &[u8],
    receipt: &SoftwareSignerSignatureReceiptV1,
    binding: &TairaAuthorityPublicBindingV1,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let claims = parse_canonical_json(claims_json)?;
    let mut object = Map::new();
    object.insert("schema".into(), Value::from(role.envelope_schema()));
    object.insert("schema_version".into(), Value::from(1_u64));
    object.insert("role".into(), Value::from(role.as_str()));
    object.insert("claims".into(), claims);
    object.insert("signature_algorithm".into(), Value::from("ed25519"));
    object.insert(
        "binding_sha256".into(),
        Value::from(hex::encode(
            binding
                .sha256()
                .map_err(|()| TairaAuthorityErrorV1::Binding)?,
        )),
    );
    object.insert(
        "signature".into(),
        Value::from(hex::encode(&receipt.signature)),
    );
    object.insert(
        "audit_sequence".into(),
        Value::from(receipt.commit_sequence),
    );
    object.insert(
        "audit_head".into(),
        Value::from(hex::encode(receipt.commit_audit_head)),
    );
    canonical_json_line(&Value::Object(object))
}

fn durable_receipt_claims_json(
    role: TairaAuthorityRoleV1,
    request: &ParsedClientRequestV1,
    envelope_json: &[u8],
    admitted_at: u64,
    envelope_receipt: &SoftwareSignerSignatureReceiptV1,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-durable-receipt-claims.v1"),
    );
    object.insert("role".into(), Value::from(role.as_str()));
    object.insert("decision".into(), Value::from("admitted"));
    object.insert(
        "replay_namespace".into(),
        Value::from(role.replay_namespace()),
    );
    object.insert(
        "operation_id".into(),
        Value::from(hex::encode(request.operation_id)),
    );
    object.insert("run_id".into(), Value::from(hex::encode(request.run_id)));
    object.insert(
        "subject_sha256".into(),
        Value::from(hex::encode(request.subject_sha256)),
    );
    object.insert(
        "authority_envelope_sha256".into(),
        Value::from(hex::encode(sha256(envelope_json))),
    );
    object.insert("admitted_at_unix_millis".into(), Value::from(admitted_at));
    object.insert(
        "authority_audit_sequence".into(),
        Value::from(envelope_receipt.commit_sequence),
    );
    object.insert(
        "authority_audit_head".into(),
        Value::from(hex::encode(envelope_receipt.commit_audit_head)),
    );
    canonical_json_line(&Value::Object(object))
}

fn durable_receipt_json(
    role: TairaAuthorityRoleV1,
    claims_json: &[u8],
    receipt: &SoftwareSignerSignatureReceiptV1,
    binding: &TairaAuthorityPublicBindingV1,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let claims = parse_canonical_json(claims_json)?;
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-durable-receipt.v1"),
    );
    object.insert("schema_version".into(), Value::from(1_u64));
    object.insert("role".into(), Value::from(role.as_str()));
    object.insert("claims".into(), claims);
    object.insert("signature_algorithm".into(), Value::from("ed25519"));
    object.insert(
        "binding_sha256".into(),
        Value::from(hex::encode(
            binding
                .sha256()
                .map_err(|()| TairaAuthorityErrorV1::Binding)?,
        )),
    );
    object.insert(
        "signature".into(),
        Value::from(hex::encode(&receipt.signature)),
    );
    object.insert(
        "audit_sequence".into(),
        Value::from(receipt.commit_sequence),
    );
    object.insert(
        "audit_head".into(),
        Value::from(hex::encode(receipt.commit_audit_head)),
    );
    canonical_json_line(&Value::Object(object))
}

fn assignment_result_json(
    stored: &StoredRunAssignmentV1,
    replayed: bool,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let assignment = parse_canonical_json(&stored.assignment_json)?;
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-run-assignment-result.v1"),
    );
    object.insert("role".into(), Value::from(stored.assignment.role.as_str()));
    object.insert(
        "status".into(),
        Value::from(if replayed { "replayed" } else { "assigned" }),
    );
    object.insert("assignment".into(), assignment);
    object.insert(
        "signature".into(),
        Value::from(hex::encode(&stored.receipt.signature)),
    );
    object.insert(
        "audit_sequence".into(),
        Value::from(stored.receipt.commit_sequence),
    );
    object.insert(
        "audit_head".into(),
        Value::from(hex::encode(stored.receipt.commit_audit_head)),
    );
    canonical_json_line(&Value::Object(object))
}

fn dry_run_result_json(request: &ParsedClientRequestV1) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-client-result.v1"),
    );
    object.insert("role".into(), Value::from("deploy-issuance"));
    object.insert(
        "operation_id".into(),
        Value::from(hex::encode(request.operation_id)),
    );
    object.insert("status".into(), Value::from("verified"));
    object.insert("authority_envelope".into(), Value::Object(Map::new()));
    object.insert("durable_receipt".into(), Value::Object(Map::new()));
    canonical_json_line(&Value::Object(object))
}

fn deployment_finalization_input(
    request: &ParsedClientRequestV1,
    applied: &StoredAuthorizationV1,
    result: &DeploymentResultV1,
    finalized_at_unix_millis: u64,
    binding: TairaAuthorityPublicBindingV1,
    previous_audit_sequence: u64,
    previous_audit_head: [u8; 32],
) -> Result<StoredDeploymentFinalizationInputV1, TairaAuthorityErrorV1> {
    if request.deploy_disposition != Some(DeployDispositionV1::Finalize)
        || request.deployment_result.as_ref() != Some(result)
        || finalized_at_unix_millis == 0
        || finalized_at_unix_millis < applied.admitted_at_unix_millis
        || binding.role != TairaAuthorityRoleV1::DeployIssuance
        || binding.validate().is_err()
        || previous_audit_sequence == 0
        || previous_audit_head == [0; 32]
        || applied.consumption.operation_id != request.operation_id
        || applied.consumption.run_id != request.run_id
        || applied.consumption.request_sha256 != request.request_sha256
        || applied.consumption.subject_sha256 != request.subject_sha256
        || applied.consumption.artifact_manifest_sha256 != request.manifest_sha256
    {
        return Err(TairaAuthorityErrorV1::State);
    }
    Ok(StoredDeploymentFinalizationInputV1 {
        operation_id: request.operation_id,
        run_id: request.run_id,
        apply_request_sha256: applied.consumption.request_sha256,
        finalization_request_sha256: request.wire_request_sha256,
        finalization_request_json: request.canonical_request_json.clone(),
        subject_sha256: request.subject_sha256,
        artifact_manifest_sha256: request.manifest_sha256,
        outcome: result.outcome.clone(),
        result_sha256: result.result_sha256,
        finalized_at_unix_millis,
        binding_sha256: binding
            .sha256()
            .map_err(|()| TairaAuthorityErrorV1::Binding)?,
        binding,
        previous_audit_sequence,
        previous_audit_head,
    })
}

fn verify_deployment_finalization_input(
    input: &StoredDeploymentFinalizationInputV1,
    applied: &StoredAuthorizationV1,
) -> Result<ParsedClientRequestV1, TairaAuthorityErrorV1> {
    let request = parse_client_request(
        &input.finalization_request_json,
        TairaAuthorityRoleV1::DeployIssuance,
    )
    .map_err(|_| TairaAuthorityErrorV1::State)?;
    let result = request
        .deployment_result
        .as_ref()
        .ok_or(TairaAuthorityErrorV1::State)?;
    let expected = deployment_finalization_input(
        &request,
        applied,
        result,
        input.finalized_at_unix_millis,
        input.binding.clone(),
        input.previous_audit_sequence,
        input.previous_audit_head,
    )?;
    if &expected != input {
        return Err(TairaAuthorityErrorV1::State);
    }
    Ok(request)
}

fn deployment_finalization_input_matches_request(
    input: &StoredDeploymentFinalizationInputV1,
    request: &ParsedClientRequestV1,
    applied: &StoredAuthorizationV1,
) -> Result<bool, TairaAuthorityErrorV1> {
    verify_deployment_finalization_input(input, applied)?;
    let Some(result) = &request.deployment_result else {
        return Ok(false);
    };
    Ok(
        request.deploy_disposition == Some(DeployDispositionV1::Finalize)
            && input.operation_id == request.operation_id
            && input.run_id == request.run_id
            && input.apply_request_sha256 == request.request_sha256
            && input.finalization_request_sha256 == request.wire_request_sha256
            && input.finalization_request_json == request.canonical_request_json
            && input.subject_sha256 == request.subject_sha256
            && input.artifact_manifest_sha256 == request.manifest_sha256
            && input.outcome == result.outcome
            && input.result_sha256 == result.result_sha256,
    )
}

fn deployment_finalization_decision_claims_json(
    input: &StoredDeploymentFinalizationInputV1,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.deployment-finalization-decision-claims.v1"),
    );
    object.insert("role".into(), Value::from("deploy-issuance"));
    object.insert(
        "operation_id".into(),
        Value::from(hex::encode(input.operation_id)),
    );
    object.insert("run_id".into(), Value::from(hex::encode(input.run_id)));
    object.insert(
        "apply_request_sha256".into(),
        Value::from(hex::encode(input.apply_request_sha256)),
    );
    object.insert(
        "finalization_request_sha256".into(),
        Value::from(hex::encode(input.finalization_request_sha256)),
    );
    object.insert(
        "subject_sha256".into(),
        Value::from(hex::encode(input.subject_sha256)),
    );
    object.insert(
        "artifact_manifest_sha256".into(),
        Value::from(hex::encode(input.artifact_manifest_sha256)),
    );
    object.insert("outcome".into(), Value::from(input.outcome.clone()));
    object.insert(
        "result_sha256".into(),
        Value::from(hex::encode(input.result_sha256)),
    );
    object.insert(
        "finalized_at_unix_millis".into(),
        Value::from(input.finalized_at_unix_millis),
    );
    object.insert(
        "binding_sha256".into(),
        Value::from(hex::encode(input.binding_sha256)),
    );
    canonical_json_line(&Value::Object(object))
}

fn deployment_finalization_claims_json(
    input: &StoredDeploymentFinalizationInputV1,
    authority_envelope_json: &[u8],
    decision_receipt: &SoftwareSignerSignatureReceiptV1,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let decision_claims = deployment_finalization_decision_claims_json(input)?;
    let decision_signing_payload = taira_signing_payload(&decision_claims)?;
    let decision_operation = digest_parts_sha256(
        DEPLOYMENT_FINALIZATION_OPERATION_DOMAIN_V1,
        &[&input.operation_id],
    );
    if decision_receipt.operation_id != decision_operation {
        return Err(TairaAuthorityErrorV1::State);
    }
    let mut object = parse_canonical_json(&decision_claims)?
        .as_object()
        .cloned()
        .ok_or(TairaAuthorityErrorV1::State)?;
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.deployment-finalization-claims.v1"),
    );
    object.insert(
        "authority_envelope_sha256".into(),
        Value::from(hex::encode(sha256(authority_envelope_json))),
    );
    object.insert(
        "decision_operation_id".into(),
        Value::from(hex::encode(decision_operation)),
    );
    object.insert(
        "decision_signing_payload_sha256".into(),
        Value::from(hex::encode(sha256(&decision_signing_payload))),
    );
    object.insert(
        "decision_audit_sequence".into(),
        Value::from(decision_receipt.commit_sequence),
    );
    object.insert(
        "decision_audit_head".into(),
        Value::from(hex::encode(decision_receipt.commit_audit_head)),
    );
    canonical_json_line(&Value::Object(object))
}

fn deployment_finalization_result_json(
    operation_id: [u8; 32],
    authority_envelope_json: &[u8],
    durable_receipt_json: &[u8],
    replayed: bool,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-client-result.v1"),
    );
    object.insert("role".into(), Value::from("deploy-issuance"));
    object.insert(
        "operation_id".into(),
        Value::from(hex::encode(operation_id)),
    );
    object.insert(
        "status".into(),
        Value::from(if replayed { "replayed" } else { "finalized" }),
    );
    object.insert(
        "authority_envelope".into(),
        parse_canonical_json(authority_envelope_json)?,
    );
    object.insert(
        "durable_receipt".into(),
        parse_canonical_json(durable_receipt_json)?,
    );
    canonical_json_line(&Value::Object(object))
}

fn replayed_finalization_result_json(
    stored: &StoredDeploymentFinalizationV1,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let value = parse_canonical_json(&stored.result_json)?;
    let mut object = value
        .as_object()
        .cloned()
        .ok_or(TairaAuthorityErrorV1::State)?;
    object.insert("status".into(), Value::from("replayed"));
    canonical_json_line(&Value::Object(object))
}

fn authorization_result_json(
    stored: &StoredAuthorizationV1,
    role: TairaAuthorityRoleV1,
    replayed: bool,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let envelope = parse_canonical_json(&stored.authority_envelope_json)?;
    let receipt = parse_canonical_json(&stored.durable_receipt_json)?;
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-client-result.v1"),
    );
    object.insert("role".into(), Value::from(role.as_str()));
    object.insert(
        "operation_id".into(),
        Value::from(hex::encode(stored.consumption.operation_id)),
    );
    object.insert(
        "status".into(),
        Value::from(if replayed { "replayed" } else { "authorized" }),
    );
    object.insert("authority_envelope".into(), envelope);
    object.insert("durable_receipt".into(), receipt);
    canonical_json_line(&Value::Object(object))
}

fn verification_result_json(
    stored: &StoredAuthorizationV1,
    role: TairaAuthorityRoleV1,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let envelope = parse_canonical_json(&stored.authority_envelope_json)?;
    let receipt = parse_canonical_json(&stored.durable_receipt_json)?;
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-client-result.v1"),
    );
    object.insert("role".into(), Value::from(role.as_str()));
    object.insert(
        "operation_id".into(),
        Value::from(hex::encode(stored.consumption.operation_id)),
    );
    object.insert("status".into(), Value::from("valid"));
    object.insert("authority_envelope".into(), envelope);
    object.insert("durable_receipt".into(), receipt);
    canonical_json_line(&Value::Object(object))
}
