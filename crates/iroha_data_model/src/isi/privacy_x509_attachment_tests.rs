// Proof-free request, original action binding and borrowed profile controls.

pub(super) fn x509_reference_request() -> SubmitPrivacyZkX509CertificateV1 {
    let sample = crate::privacy::privacy_action_role_samples_v1()
        .unwrap()
        .into_iter()
        .find(|sample| sample.envelope.protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1)
        .unwrap();
    let PrivacyStatementV1::IrohaZkX509StarkP256V1(mut statement) = sample.envelope.statement
    else {
        unreachable!()
    };
    statement.context.transaction_intent_digest = PrivacyTransactionIntentDigestV1::new([0; 32]);
    SubmitPrivacyZkX509CertificateV1::new(
        statement,
        ActionAttachmentRefV1 {
            action_index: 0,
            slot_index: 0,
        },
    )
}

fn x509_reference_payload(
    mut request: SubmitPrivacyZkX509CertificateV1,
    mixed: bool,
) -> crate::transaction::TransactionPayload {
    use crate::transaction::{FeePaymentIntent, TransactionBuilder, signed_attachments::*};
    let mut plan = SignedAttachmentPlanV1::new();
    let mut instructions = Vec::<InstructionBox>::new();
    if mixed {
        instructions.push(crate::isi::Log::new(crate::Level::INFO, "prior action".into()).into());
        plan.push_action(PlannedActionV1 {
            operation: ActionOperationV1::NativeInstruction(
                crate::isi::instruction_wire_id(&instructions[0])
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
    }
    request.proof_ref.action_index = u32::from(mixed);
    plan.push_action(PlannedActionV1 {
        operation: ActionOperationV1::NativeInstruction(
            SubmitPrivacyZkX509CertificateV1::WIRE_ID.into(),
        ),
        role: ActionRoleV1::ProofEffect,
        authority: ActionAuthorityBindingV1::TransactionAuthority,
        invocation: ActionInvocationBindingV1::Direct {
            instruction_index: u32::from(mixed),
        },
        public_inputs_digest: request.public_inputs_digest_v1().unwrap(),
        attachments: vec![PlannedAttachmentV1 {
            kind: AttachmentKindV1::Proof,
            role: AttachmentRoleV1::ActionProof,
            bytes: vec![1, 2, 3],
        }],
    });
    instructions.push(request.clone().into());
    TransactionBuilder::new(
        request.statement.context.network_id,
        request.statement.wallet_account,
        FeePaymentIntent::authority(vec![], None),
    )
    .with_instructions(instructions)
    .with_signed_attachments(plan.declare())
    .payload()
    .clone()
}

#[test]
fn x509_proof_free_request_is_registered_roundtrips_and_rejects_embedded_proof_json() {
    let request = x509_reference_request();
    assert_slice_roundtrip(SubmitPrivacyZkX509CertificateV1::WIRE_ID, request.clone());
    let json = norito::json::to_json(&request).unwrap();
    assert_eq!(
        norito::json::from_json::<SubmitPrivacyZkX509CertificateV1>(&json).unwrap(),
        request
    );
    let mut value = norito::json::to_value(&request).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("proof".into(), norito::json!([1, 2, 3]));
    let embedded_proof_json = norito::json::to_json(&value).unwrap();
    assert!(
        norito::json::from_json::<SubmitPrivacyZkX509CertificateV1>(&embedded_proof_json).is_err()
    );
    assert!(norito::json::from_value::<SubmitPrivacyZkX509CertificateV1>(value).is_err());
    let boxed: InstructionBox = request.clone().into();
    let frame = norito::encode_canonical(&boxed).unwrap();
    let decoded: InstructionBox = norito::decode_canonical(&frame).unwrap();
    assert_eq!(
        decoded
            .as_any()
            .downcast_ref::<SubmitPrivacyZkX509CertificateV1>(),
        Some(&request)
    );
}

#[test]
fn x509_materialization_binds_the_original_action_and_keeps_privacy_ordinal() {
    use crate::transaction::{Executable, signed_attachments::*};
    let payload = x509_reference_payload(x509_reference_request(), true);
    payload.validate_signed_attachments_v1().unwrap();
    let Executable::Instructions(instructions) = &payload.instructions else {
        unreachable!()
    };
    let request = instructions[1]
        .as_any()
        .downcast_ref::<SubmitPrivacyZkX509CertificateV1>()
        .unwrap();
    let action = payload.attachments.as_ref().unwrap().action(1).unwrap();
    let intent = payload.ordered_intent_digest_v1().unwrap();
    let limits = PrivacyConsensusLimitsV1::taira_default();
    let materialized = request
        .materialize_statement_v1(action, intent, &limits)
        .unwrap();
    assert_eq!(request.proof_ref.action_index, 1);
    assert_eq!(materialized.context.action_index, 0);
    assert_eq!(
        materialized.context.transaction_intent_digest.as_bytes(),
        action.statement_digest_for_v1(intent).unwrap().as_bytes()
    );
    assert_ne!(
        materialized.context.transaction_intent_digest.as_bytes(),
        intent.as_bytes()
    );
    assert!(
        request
            .statement
            .context
            .transaction_intent_digest
            .is_zero()
    );
    assert!(matches!(
        request.materialize_statement_v1(action, OrderedIntentDigestV1::new([0; 32]), &limits),
        Err(PrivacyZkX509RequestErrorV1::ZeroOrderedIntent)
    ));
    for mutation in 0..6 {
        let mut changed = action.clone();
        match mutation {
            0 => changed.action_index = 0,
            1 => {
                changed.operation =
                    ActionOperationV1::NativeInstruction(SubmitPrivacyProofV1::WIRE_ID.into())
            }
            2 => changed.role = ActionRoleV1::ProofVerification,
            3 => changed.attachments.clear(),
            4 => changed.attachments[0].kind = AttachmentKindV1::KeyMaterial,
            _ => changed.public_inputs_digest = ActionPublicInputsDigestV1::new([0x88; 32]),
        }
        assert!(
            request
                .materialize_statement_v1(&changed, intent, &limits)
                .is_err()
        );
    }
    for mutation in 0..3 {
        let mut changed = action.clone();
        match mutation {
            0 => changed.authority = ActionAuthorityBindingV1::Account(account(91)),
            1 => {
                changed.invocation = ActionInvocationBindingV1::Direct {
                    instruction_index: 0,
                }
            }
            _ => {
                // A different original ordered intent must yield another native binding.
                let different = request
                    .materialize_statement_v1(
                        action,
                        OrderedIntentDigestV1::new([0x91; 32]),
                        &limits,
                    )
                    .unwrap();
                assert_ne!(
                    different.context.transaction_intent_digest,
                    materialized.context.transaction_intent_digest
                );
                continue;
            }
        }
        assert_ne!(
            request
                .materialize_statement_v1(&changed, intent, &limits)
                .unwrap()
                .context
                .transaction_intent_digest,
            materialized.context.transaction_intent_digest
        );
    }
}

#[test]
fn x509_direct_request_requires_signed_occurrence_and_rejects_input_substitution() {
    use crate::transaction::Executable;
    let original = x509_reference_payload(x509_reference_request(), false);
    original.validate_signed_attachments_v1().unwrap();
    let mut missing = original.clone();
    missing.attachments = None;
    assert!(missing.validate_signed_attachments_v1().is_err());
    for mutation in 0..6 {
        let mut payload = original.clone();
        let mut request = x509_reference_request();
        match mutation {
            0 => request.proof_ref.slot_index = 1,
            1 => request.proof_ref.action_index = 1,
            2 => {
                request.statement.context.transaction_intent_digest =
                    PrivacyTransactionIntentDigestV1::new([9; 32])
            }
            3 => request.statement.context.action_index += 1,
            4 => request.statement.wallet_account = account(97),
            _ => {
                request.statement.wallet_challenge =
                    crate::privacy::PrivacyChallengeV1::new([17; 32])
            }
        }
        payload.instructions = Executable::Instructions(vec![request.into()].into());
        payload.bind_attachment_statements_v1().unwrap();
        assert!(
            payload.validate_signed_attachments_v1().is_err(),
            "mutation {mutation}"
        );
    }
    let mut different_proof = original.clone();
    different_proof
        .attachments
        .as_mut()
        .unwrap()
        .set_occurrence_bytes(0, vec![9; 7])
        .unwrap();
    different_proof.bind_attachment_statements_v1().unwrap();
    different_proof.validate_signed_attachments_v1().unwrap();
    assert_eq!(
        original.ordered_intent_digest_v1().unwrap(),
        different_proof.ordered_intent_digest_v1().unwrap()
    );
    assert_ne!(
        norito::encode_canonical(&original).unwrap(),
        norito::encode_canonical(&different_proof).unwrap()
    );
}

#[test]
fn x509_borrowed_profile_preserves_limits_and_exact_request_charge() {
    let sample = crate::privacy::privacy_action_role_samples_v1()
        .unwrap()
        .into_iter()
        .find(|s| s.envelope.protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1)
        .unwrap();
    let request = x509_reference_request();
    let payload = x509_reference_payload(request.clone(), false);
    let limits = PrivacyConsensusLimitsV1::taira_default();
    let materialized = request
        .materialize_statement_v1(
            payload.attachments.as_ref().unwrap().action(0).unwrap(),
            payload.ordered_intent_digest_v1().unwrap(),
            &limits,
        )
        .unwrap();
    let proof = [1, 2, 3];
    let (digest, bytes) = request
        .validate_materialized_against_profile_v1(
            &materialized,
            &proof,
            &sample.profile,
            &sample.profile.protocol_limits,
            &limits,
        )
        .unwrap();
    assert_eq!(
        bytes,
        (norito::canonical_frame_len(&request).unwrap() + proof.len()) as u64
    );
    assert_eq!(
        digest,
        PrivacyStatementV1::IrohaZkX509StarkP256V1(materialized.clone())
            .digest()
            .unwrap()
    );
    for raw in [&[][..], &[0, 0][..]] {
        assert!(
            request
                .validate_materialized_against_profile_v1(
                    &materialized,
                    raw,
                    &sample.profile,
                    &sample.profile.protocol_limits,
                    &limits,
                )
                .is_err()
        );
        assert_eq!(
            PrivacyProofBytesV1::validate_bytes_v1(raw, &limits),
            PrivacyProofBytesV1::new(raw.to_vec()).validate(&limits)
        );
    }
    let mut changed = materialized.clone();
    changed.wallet_account = account(99);
    assert!(
        request
            .validate_materialized_against_profile_v1(
                &changed,
                &proof,
                &sample.profile,
                &sample.profile.protocol_limits,
                &limits,
            )
            .is_err()
    );
    let mut profile = sample.profile.clone();
    profile.verifier_digest = PrivacyVerifierDigestV1::new([99; 32]);
    assert!(
        request
            .validate_materialized_against_profile_v1(
                &materialized,
                &proof,
                &profile,
                &profile.protocol_limits,
                &limits,
            )
            .is_err()
    );
    let mut tight = limits;
    tight.max_proof_bytes_per_action = 3;
    tight.max_statement_and_encrypted_output_bytes_per_transaction = u32::try_from(
        norito::canonical_frame_len(&PrivacyStatementV1::IrohaZkX509StarkP256V1(
            materialized.clone(),
        ))
        .unwrap(),
    )
    .unwrap();
    tight.max_action_bytes = u32::try_from(bytes - 1).unwrap();
    tight.validate().unwrap();
    assert!(matches!(
        request.validate_materialized_against_profile_v1(
            &materialized,
            &proof,
            &sample.profile,
            &sample.profile.protocol_limits,
            &tight,
        ),
        Err(PrivacyZkX509RequestErrorV1::Envelope(
            crate::privacy::PrivacyProofEnvelopeValidationError::ActionTooLarge { .. }
        ))
    ));
    tight.max_action_bytes += 1;
    assert_eq!(
        request
            .validate_materialized_against_profile_v1(
                &materialized,
                &proof,
                &sample.profile,
                &sample.profile.protocol_limits,
                &tight,
            )
            .unwrap()
            .1,
        bytes
    );
    tight.max_proof_bytes_per_action = 2;
    assert!(
        request
            .validate_materialized_against_profile_v1(
                &materialized,
                &proof,
                &sample.profile,
                &sample.profile.protocol_limits,
                &tight,
            )
            .is_err()
    );
}

#[test]
fn x509_inline_execution_is_retired_while_envelope_interchange_remains_valid() {
    use crate::transaction::{Executable, signed::PrivacyTransactionIntentErrorV1};
    let sample = crate::privacy::privacy_action_role_samples_v1()
        .unwrap()
        .into_iter()
        .find(|s| s.envelope.protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1)
        .unwrap();
    sample
        .envelope
        .validate_against_profile_v1(
            &sample.profile,
            &sample.profile.protocol_limits,
            &PrivacyConsensusLimitsV1::taira_default(),
        )
        .unwrap();
    let frame = norito::encode_canonical(&sample.envelope).unwrap();
    assert_eq!(
        norito::decode_canonical::<PrivacyProofEnvelopeV1>(&frame).unwrap(),
        sample.envelope
    );
    let mut payload = x509_reference_payload(x509_reference_request(), false);
    payload.attachments = None;
    payload.instructions =
        Executable::Instructions(vec![SubmitPrivacyProofV1::new(sample.envelope).into()].into());
    assert!(matches!(
        payload.privacy_transaction_intent_binding_if_present_v1(),
        Err(PrivacyTransactionIntentErrorV1::X509RequiresSignedAttachment)
    ));
    assert!(matches!(
        payload.validate_privacy_transaction_intent_binding_v1(),
        Err(PrivacyTransactionIntentErrorV1::X509RequiresSignedAttachment)
    ));
}

#[test]
fn x509_request_and_action_statement_have_exact_native_frame_layouts() {
    use crate::transaction::signed_attachments::{
        ACTION_STATEMENT_DIGEST_DOMAIN_V1, ActionAttachmentRefV1, ActionStatementV1,
    };
    let request = x509_reference_request();
    assert_eq!(
        norito::core::archived_payload_align::<SubmitPrivacyZkX509CertificateV1>(),
        8
    );
    assert_eq!(
        norito::core::archived_payload_align::<ActionStatementV1>(),
        8
    );
    assert_eq!(
        norito::core::archived_payload_align::<ActionAttachmentRefV1>(),
        4
    );
    assert!(
        norito::canonical_frame_len(&request).unwrap()
            <= SubmitPrivacyZkX509CertificateV1::canonical_frame_byte_limit_v1()
    );
    let flags = norito::core::default_encode_flags();
    let _canonical = norito::core::DecodeFlagsGuard::enter(flags);
    let reference_len = norito::core::encoded_payload_len(&request.proof_ref).unwrap();
    assert_eq!(
        reference_len,
        2 * (norito::core::len_prefix_len_with_flags(4, flags) + 4)
    );
    let boxed: InstructionBox = request.clone().into();
    assert!(
        norito::canonical_frame_len(&boxed).unwrap()
            <= SubmitPrivacyZkX509CertificateV1::canonical_instruction_box_frame_byte_limit_v1()
    );
    // The exact replacement equation is checked independently against actual serialization.
    let tagged = PrivacyStatementV1::IrohaZkX509StarkP256V1(request.statement.clone());
    let tagged_frame = norito::canonical_frame_len(&tagged).unwrap();
    let concrete_frame = norito::canonical_frame_len(&request).unwrap();
    let request_header =
        crate::isi::framed_instruction_payload_len_for::<SubmitPrivacyZkX509CertificateV1>(0)
            .unwrap();
    let tagged_header =
        crate::isi::framed_instruction_payload_len_for::<PrivacyStatementV1>(0).unwrap();
    assert_eq!(
        concrete_frame - request_header,
        tagged_frame - tagged_header - 4
            + norito::core::len_prefix_len_with_flags(reference_len, flags)
            + reference_len
    );
    assert_eq!(
        ACTION_STATEMENT_DIGEST_DOMAIN_V1,
        b"iroha.transaction.action-statement-digest.v1"
    );
    let encoded = norito::encode_canonical(&request.statement).unwrap();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"iroha.privacy.zk-x509-request-public-inputs.v1");
    hasher.update(&u64::try_from(encoded.len()).unwrap().to_le_bytes());
    hasher.update(&encoded);
    assert_eq!(
        request.public_inputs_digest_v1().unwrap().as_bytes(),
        hasher.finalize().as_bytes()
    );
    eprintln!(
        "X509_REQUEST_LAYOUT request_alignment={} action_statement_alignment={} reference_alignment={} request_schema={:02x?} action_statement_schema={:02x?}",
        norito::core::archived_payload_align::<SubmitPrivacyZkX509CertificateV1>(),
        norito::core::archived_payload_align::<ActionStatementV1>(),
        norito::core::archived_payload_align::<ActionAttachmentRefV1>(),
        norito::schema::identity::frame_hash::<SubmitPrivacyZkX509CertificateV1>(),
        norito::schema::identity::frame_hash::<ActionStatementV1>()
    );
}

#[test]
fn x509_request_encoding_projection_preserves_original_local_refusals() {
    use norito::core as ncore;
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
    let original = norito::with_decode_limits_scope(limits, || {
        ncore::classify_decode_attempt(|| ncore::reserve_decode_allocation(7))
    })
    .unwrap_err();
    let retained = original.local_refusal().unwrap();
    let refusal = PrivacyZkX509RequestErrorV1::from_encoding_attempt(original);
    let PrivacyZkX509RequestErrorV1::Attachment(refusal) = refusal else {
        panic!("the original local allocation refusal was converted to a semantic rejection")
    };
    assert_eq!(refusal.into_local_refusal(), Ok(retained));

    let invalid =
        ncore::classify_decode_attempt(|| Err::<(), _>(ncore::Error::LengthMismatch)).unwrap_err();
    assert_eq!(
        PrivacyZkX509RequestErrorV1::from_encoding_attempt(invalid),
        PrivacyZkX509RequestErrorV1::Attachment(
            crate::transaction::signed_attachments::SignedAttachmentErrorV1::CanonicalEncodingFailed
        )
    );
}
