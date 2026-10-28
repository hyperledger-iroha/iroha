//! Structural JNI producer regressions using the maintained typed Exact12 fixtures.

use super::*;
use iroha_data_model::{
    privacy::*,
    transaction::{TransactionBuilder, TransactionPayload, signed_attachments::*},
};

fn fixture() -> (
    SubmitPrivacyZkX509CertificateV1,
    TransactionPayload,
    Vec<u8>,
    Vec<u8>,
) {
    let row = privacy_exact12_fixture_bundle_v1()
        .expect("maintained fixtures")
        .rows
        .into_iter()
        .find(|row| row.protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1)
        .expect("X509 row");
    let request = norito::decode_canonical(&row.submit_proof_instruction_norito)
        .expect("canonical proof-free request");
    let builder = TransactionBuilder::decode_payload(&row.unsigned_transaction_payload_norito)
        .expect("original fixture payload");
    (
        request,
        builder.payload().clone(),
        row.statement_norito,
        row.transaction_intent_projection_norito,
    )
}

#[test]
fn request_construction_and_inspection_bind_actual_context_without_proof_bytes() {
    let (request, _, _, _) = fixture();
    let statement = norito::encode_canonical(&PrivacyStatementV1::IrohaZkX509StarkP256V1(
        request.statement.clone(),
    ))
    .unwrap();
    let canonical = norito::encode_canonical(&request).unwrap();
    assert_eq!(
        create_request(&statement, i64::from(request.proof_ref.action_index)),
        Some(canonical.clone())
    );
    let inspected = inspect_request(&canonical).expect("nine real fields");
    let context = &request.statement.context;
    assert_eq!(inspected.len(), 9);
    assert_eq!(inspected[0], canonical);
    assert_eq!(
        inspected[1],
        request.proof_ref.action_index.to_le_bytes().to_vec()
    );
    assert_eq!(
        inspected[2],
        request
            .public_inputs_digest_v1()
            .unwrap()
            .as_bytes()
            .to_vec()
    );
    assert_eq!(inspected[3], context.network_id.as_bytes().to_vec());
    assert_eq!(inspected[4], context.parameter_id.as_bytes().to_vec());
    assert_eq!(inspected[5], context.parameter_digest.as_bytes().to_vec());
    assert_eq!(inspected[6], context.verifier_digest.as_bytes().to_vec());
    assert_eq!(
        inspected[7],
        context.statement_schema_digest.as_bytes().to_vec()
    );
    assert_eq!(
        inspected[8],
        context.engine_manifest_digest.as_bytes().to_vec()
    );
    assert!(context.transaction_intent_digest.is_zero());
    assert_eq!(request.proof_ref.slot_index, 0);
    assert!(create_request(&statement, -1).is_none());
    assert!(create_request(&statement, SIGNED_ATTACHMENTS_MAX_ACTIONS_V1 as i64).is_none());
    assert!(create_request(&statement, i64::from(u32::MAX) + 1).is_none());
    assert_eq!(
        construction_frame_limit(),
        SubmitPrivacyProofV1::canonical_frame_byte_limit_v1()
            .max(SubmitPrivacyZkX509CertificateV1::canonical_frame_byte_limit_v1())
    );
}

#[test]
fn request_boundary_rejects_materialized_inline_wrong_type_and_malformed_frames() {
    let (request, _, materialized, _) = fixture();
    let canonical = norito::encode_canonical(&request).unwrap();
    let independent = norito::encode_canonical(&PrivacyStatementV1::IrohaZkX509StarkP256V1(
        request.statement.clone(),
    ))
    .unwrap();
    assert!(
        create_request(&materialized, 0).is_none(),
        "caller-derived intent is forbidden"
    );
    assert!(inspect_request(&materialized).is_none());
    let mut trailing = canonical.clone();
    trailing.push(0);
    for bad in [
        Vec::new(),
        canonical[..canonical.len() - 1].to_vec(),
        trailing,
        vec![0; SubmitPrivacyZkX509CertificateV1::canonical_frame_byte_limit_v1() + 1],
    ] {
        assert!(inspect_request(&bad).is_none());
    }
    let mut trailing = independent.clone();
    trailing.push(0);
    for bad in [
        Vec::new(),
        independent[..independent.len() - 1].to_vec(),
        trailing,
        vec![
            0;
            TAIRA_PRIVACY_MAX_STATEMENT_AND_ENCRYPTED_OUTPUT_BYTES_PER_TRANSACTION_V1 as usize + 1
        ],
    ] {
        assert!(create_request(&bad, 0).is_none());
    }
    let mut bad = request.clone();
    bad.proof_ref.slot_index = 1;
    assert!(inspect_request(&norito::encode_canonical(&bad).unwrap()).is_none());
    bad = request.clone();
    bad.statement.context.transaction_intent_digest =
        PrivacyTransactionIntentDigestV1::new([7; 32]);
    assert!(inspect_request(&norito::encode_canonical(&bad).unwrap()).is_none());
    bad = request;
    bad.proof_ref.action_index = SIGNED_ATTACHMENTS_MAX_ACTIONS_V1 as u32;
    assert!(inspect_request(&norito::encode_canonical(&bad).unwrap()).is_none());
    for row in privacy_exact12_fixture_bundle_v1().unwrap().rows {
        assert!(inspect_request(&row.envelope_norito).is_none());
        if row.protocol_id != PrivacyProtocolIdV1::IrohaZkX509StarkP256V1 {
            assert!(inspect_request(&row.submit_proof_instruction_norito).is_none());
            assert!(create_request(&row.statement_norito, 0).is_none());
        }
    }
}

#[test]
fn materialization_uses_original_action_statement_and_preserves_privacy_ordinal() {
    let (request, payload, expected_statement, expected_projection) = fixture();
    payload.validate_signed_attachments_v1().unwrap();
    let intent = payload.ordered_intent_digest_v1().unwrap();
    let action = payload
        .attachments
        .as_ref()
        .unwrap()
        .action(request.proof_ref.action_index)
        .unwrap();
    let request_frame = norito::encode_canonical(&request).unwrap();
    let action_frame = norito::encode_canonical(action).unwrap();
    assert_eq!(
        norito::encode_canonical(&action.statement_for_v1(intent)).unwrap(),
        expected_projection
    );
    let materialized =
        materialize_statement(&request_frame, &action_frame, intent.as_bytes()).unwrap();
    assert_eq!(materialized, expected_statement);
    let tagged: PrivacyStatementV1 = norito::decode_canonical(&materialized).unwrap();
    assert_eq!(
        tagged.context().action_index,
        request.statement.context.action_index
    );
    assert_eq!(
        tagged.context().transaction_intent_digest.as_bytes(),
        action.statement_digest_for_v1(intent).unwrap().as_bytes()
    );
    assert_ne!(
        tagged.context().transaction_intent_digest.as_bytes(),
        intent.as_bytes()
    );
    assert!(materialize_statement(&request_frame, &action_frame, &[0; 32]).is_none());
    assert!(materialize_statement(&request_frame, &action_frame, &[1; 31]).is_none());
    assert!(materialize_statement(&request_frame, &action_frame, &[1; 33]).is_none());
    let mut trailing = action_frame.clone();
    trailing.push(0);
    for bad in [
        Vec::new(),
        action_frame[..action_frame.len() - 1].to_vec(),
        trailing,
        vec![0; SIGNED_ATTACHMENTS_MAX_CANONICAL_FRAME_BYTES_V1 + 1],
    ] {
        assert!(materialize_statement(&request_frame, &bad, intent.as_bytes()).is_none());
    }
    for mutation in 0..8 {
        let mut substituted = action.clone();
        match mutation {
            0 => substituted.public_inputs_digest = ActionPublicInputsDigestV1::new([0x51; 32]),
            1 => substituted.action_index += 1,
            2 => substituted.role = ActionRoleV1::Effect,
            3 => {
                substituted.operation =
                    ActionOperationV1::NativeInstruction(SubmitPrivacyProofV1::WIRE_ID.into())
            }
            4 => substituted.attachments.clear(),
            5 => substituted.attachments[0].kind = AttachmentKindV1::KeyMaterial,
            6 => substituted.attachments[0].role = AttachmentRoleV1::ExecutionProof,
            _ => {
                let duplicate = substituted.attachments[0];
                substituted.attachments.push(duplicate);
            }
        }
        assert!(
            materialize_statement(
                &request_frame,
                &norito::encode_canonical(&substituted).unwrap(),
                intent.as_bytes(),
            )
            .is_none()
        );
    }
    // Authority and invocation are actual action inputs, not values manufactured by JNI.
    // Changing them derives a different digest; producer materialization grants no authority.
    for mutation in 0..3 {
        let mut changed = action.clone();
        let mut changed_intent = intent;
        match mutation {
            0 => changed.authority = ActionAuthorityBindingV1::Account(payload.authority.clone()),
            1 => {
                changed.invocation = ActionInvocationBindingV1::Direct {
                    instruction_index: 1,
                }
            }
            _ => changed_intent = OrderedIntentDigestV1::new([0x91; 32]),
        }
        let changed = materialize_statement(
            &request_frame,
            &norito::encode_canonical(&changed).unwrap(),
            changed_intent.as_bytes(),
        )
        .unwrap();
        let changed: PrivacyStatementV1 = norito::decode_canonical(&changed).unwrap();
        assert_ne!(
            changed.context().transaction_intent_digest,
            tagged.context().transaction_intent_digest
        );
    }
}

#[test]
fn independent_request_ordinal_is_not_replaced_by_attachment_ordinal() {
    let (mut request, original, _, _) = fixture();
    let raw_proof = original
        .attachments
        .as_ref()
        .unwrap()
        .occurrence(0)
        .unwrap()
        .bytes
        .clone();
    request.proof_ref.action_index = 1;
    request.statement.context.action_index = 0;
    let mut plan = SignedAttachmentPlanV1::new();
    let log = iroha_data_model::isi::Log::new(iroha_data_model::Level::INFO, "prior action".into());
    let log: iroha_data_model::isi::InstructionBox = log.into();
    plan.push_action(PlannedActionV1 {
        operation: ActionOperationV1::NativeInstruction(
            iroha_data_model::isi::instruction_wire_id(&log)
                .unwrap()
                .into(),
        ),
        role: ActionRoleV1::Effect,
        authority: ActionAuthorityBindingV1::TransactionAuthority,
        invocation: ActionInvocationBindingV1::Direct {
            instruction_index: 0,
        },
        public_inputs_digest: ActionPublicInputsDigestV1::new([0x51; 32]),
        attachments: Vec::new(),
    });
    plan.push_action(PlannedActionV1 {
        operation: ActionOperationV1::NativeInstruction(
            SubmitPrivacyZkX509CertificateV1::WIRE_ID.into(),
        ),
        role: ActionRoleV1::ProofEffect,
        authority: ActionAuthorityBindingV1::TransactionAuthority,
        invocation: ActionInvocationBindingV1::Direct {
            instruction_index: 1,
        },
        public_inputs_digest: request.public_inputs_digest_v1().unwrap(),
        attachments: vec![PlannedAttachmentV1 {
            kind: AttachmentKindV1::Proof,
            role: AttachmentRoleV1::ActionProof,
            bytes: raw_proof,
        }],
    });
    let mut payload = TransactionBuilder::new(
        request.statement.context.network_id,
        original.authority,
        original.fee_payment,
    )
    .with_instructions([log, request.clone().into()])
    .with_signed_attachments(plan.declare())
    .payload()
    .clone();
    payload.bind_attachment_statements_v1().unwrap();
    payload.validate_signed_attachments_v1().unwrap();
    let action = payload.attachments.as_ref().unwrap().action(1).unwrap();
    let intent = payload.ordered_intent_digest_v1().unwrap();
    let output = materialize_statement(
        &norito::encode_canonical(&request).unwrap(),
        &norito::encode_canonical(action).unwrap(),
        intent.as_bytes(),
    )
    .unwrap();
    let output: PrivacyStatementV1 = norito::decode_canonical(&output).unwrap();
    assert_eq!(output.context().action_index, 0);
    assert_eq!(action.action_index, 1);
    assert_eq!(
        output.context().transaction_intent_digest.as_bytes(),
        action.statement_digest_for_v1(intent).unwrap().as_bytes()
    );
}

#[test]
fn request_admission_checks_network_all_five_identities_and_resource_limits() {
    let sample = privacy_action_role_samples_v1()
        .unwrap()
        .into_iter()
        .find(|sample| sample.envelope.protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1)
        .unwrap();
    let profile = PrivacyCompiledProfileSnapshotV1 {
        protocol_id: sample.envelope.protocol_id,
        proof_system_id: sample.envelope.proof_system_id,
        engine_id: sample.envelope.engine_id,
        parameter_id: sample.envelope.parameter_id,
        parameter_digest: sample.envelope.parameter_digest,
        verifier_digest: sample.envelope.verifier_digest,
        statement_schema_digest: sample.envelope.statement_schema_digest,
        engine_manifest_digest: sample.envelope.engine_manifest_digest,
        protocol_limits: PrivacyProtocolLimitsV1::IrohaZkX509StarkP256V1,
    };
    let PrivacyStatementV1::IrohaZkX509StarkP256V1(mut statement) = sample.envelope.statement
    else {
        unreachable!()
    };
    statement.context.transaction_intent_digest = PrivacyTransactionIntentDigestV1::new([0; 32]);
    let request = SubmitPrivacyZkX509CertificateV1::new(
        statement,
        ActionAttachmentRefV1 {
            action_index: 0,
            slot_index: 0,
        },
    );
    let archive = norito::encode_canonical(&request).unwrap();
    let network = *request.statement.context.network_id.as_bytes();
    let limits = PrivacyConsensusLimitsV1::taira_default();
    assert!(request_admitted(
        &archive,
        &profile,
        &profile.protocol_limits,
        &limits,
        &network
    ));
    let mut other_network = network;
    other_network[0] ^= 1;
    assert!(!request_admitted(
        &archive,
        &profile,
        &profile.protocol_limits,
        &limits,
        &other_network
    ));
    let mut substitutions = Vec::new();
    let mut changed = request.clone();
    changed.statement.context.parameter_id = PrivacyParameterIdV1::new([0x61; 32]);
    substitutions.push(changed);
    let mut changed = request.clone();
    changed.statement.context.parameter_digest = PrivacyParameterDigestV1::new([0x62; 32]);
    substitutions.push(changed);
    let mut changed = request.clone();
    changed.statement.context.verifier_digest = PrivacyVerifierDigestV1::new([0x63; 32]);
    substitutions.push(changed);
    let mut changed = request.clone();
    changed.statement.context.statement_schema_digest =
        PrivacyStatementSchemaDigestV1::new([0x64; 32]);
    substitutions.push(changed);
    let mut changed = request.clone();
    changed.statement.context.engine_manifest_digest =
        PrivacyEngineManifestDigestV1::new([0x65; 32]);
    substitutions.push(changed);
    for changed in substitutions {
        assert_ne!(changed, request);
        assert!(!request_admitted(
            &norito::encode_canonical(&changed).unwrap(),
            &profile,
            &profile.protocol_limits,
            &limits,
            &network,
        ));
    }
    let mut wrong_profile = profile;
    wrong_profile.protocol_id = PrivacyProtocolIdV1::VeRangeTransparentRangeV1;
    assert!(!request_admitted(
        &archive,
        &wrong_profile,
        &profile.protocol_limits,
        &limits,
        &network,
    ));
    let mut reduced_limits = limits;
    reduced_limits.max_statement_and_encrypted_output_bytes_per_transaction = 1;
    assert!(!request_admitted(
        &archive,
        &profile,
        &profile.protocol_limits,
        &reduced_limits,
        &network,
    ));
    let index = PrivacyProtocolIdV1::ALL
        .iter()
        .position(|protocol| *protocol == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1)
        .unwrap();
    assert!(!super::super::java_privacy_exact12_submit_proof_admitted(
        b"unqualified",
        index as i32,
        &archive,
        &network,
    ));
}
