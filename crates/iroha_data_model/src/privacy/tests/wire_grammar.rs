// Frozen wire custody and independently framed Ironwood operations.

#[test]
fn exact12_runtime_projection_preserves_original_wire_bytes_and_digest() {
    for protocol in PrivacyRuntimeProtocolIdV1::ALL {
        let runtime = envelope(statement_for(protocol));
        let original: PrivacyProofEnvelopeV1 = runtime.clone().try_into().unwrap();
        // Storage migration projects exact original values without moving a baseline tag.
        assert_eq!(original.encode(), runtime.encode());
        assert_eq!(
            original.statement.digest().unwrap(),
            runtime.statement.digest().unwrap()
        );
        assert_eq!(
            PrivacyRuntimeProofEnvelopeV1::from(original.clone()),
            runtime
        );
        let original_bytes = norito::encode_canonical(&original).unwrap();
        let projected: PrivacyProofEnvelopeV1 =
            PrivacyRuntimeProofEnvelopeV1::from(original.clone())
                .try_into()
                .unwrap();
        assert_eq!(
            norito::encode_canonical(&projected).unwrap(),
            original_bytes
        );
        let instruction = runtime.into_submission_v1().unwrap();
        let captured = instruction
            .as_any()
            .downcast_ref::<crate::isi::privacy::SubmitPrivacyProofV1>()
            .unwrap();
        assert_eq!(captured.envelope, original);
    }
}

#[test]
fn frozen_wire_rejects_each_independent_ironwood_discriminant() {
    let base = envelope(statement_for(
        PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1,
    ));
    let ironwood = envelope(statement_for(
        PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
    ));
    let mut wrong_protocol = base.clone();
    wrong_protocol.protocol_id = PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1;
    let mut wrong_statement = base.clone();
    wrong_statement.statement = ironwood.statement.clone();
    let mut wrong_proof = base;
    wrong_proof.proof = ironwood.proof.clone();
    for candidate in [
        wrong_protocol,
        wrong_statement,
        wrong_proof,
        ironwood.clone(),
    ] {
        assert!(PrivacyProofEnvelopeV1::try_from(candidate.clone()).is_err());
        assert!(PrivacyProofEnvelopeV1::decode(&mut candidate.encode().as_slice()).is_err());
    }
    assert!(PrivacyProtocolIdV1::decode(&mut 12_u32.to_le_bytes().as_slice()).is_err());
    let namespace = PrivacyRuntimeNamespaceV1::from_statement(&ironwood.statement);
    assert!(PrivacyNamespaceV1::try_from(namespace).is_err());
    assert!(PrivacyNamespaceV1::decode(&mut namespace.encode().as_slice()).is_err());
    let limits = PrivacyRuntimeProtocolLimitsV1::canonical_default_v1(
        PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
    );
    assert!(PrivacyProtocolLimitsV1::try_from(limits).is_err());
    assert!(PrivacyProtocolLimitsV1::decode(&mut limits.encode().as_slice()).is_err());
}

#[test]
fn ironwood_uses_dedicated_instruction_and_statement_frames() {
    let runtime = envelope(statement_for(
        PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
    ));
    let instruction = runtime.clone().into_submission_v1().unwrap();
    assert!(
        instruction
            .as_any()
            .downcast_ref::<crate::isi::privacy::SubmitPrivacyProofV1>()
            .is_none()
    );
    let captured = instruction
        .as_any()
        .downcast_ref::<crate::isi::privacy::SubmitPrivacyIronwoodProofV1>()
        .unwrap();
    assert_eq!(
        captured.envelope.statement.digest().unwrap(),
        runtime.statement.digest().unwrap()
    );
    assert_eq!(
        PrivacyRuntimeProofEnvelopeV1::from(captured.envelope.clone()),
        runtime
    );
    assert!(
        norito::decode_canonical::<PrivacyProofEnvelopeV1>(
            &norito::encode_canonical(&captured.envelope).unwrap()
        )
        .is_err()
    );
    let mut malformed = captured.envelope.clone();
    malformed.statement_digest = PrivacyStatementDigestV1::new([0; 32]);
    assert!(
        malformed
            .validate_with_limits(&PrivacyConsensusLimitsV1::taira_default())
            .is_err()
    );
}

#[test]
fn ironwood_transaction_intent_binds_dedicated_wire_and_refuses_mixed_or_changed_input() {
    use crate::{
        isi::{InstructionBox, privacy::SubmitPrivacyIronwoodProofV1},
        transaction::{Executable, FeePaymentIntent, PrivacySubmissionRefV1, TransactionBuilder},
    };
    let runtime = envelope(statement_for(
        PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
    ));
    let mut draft = SubmitPrivacyIronwoodProofV1::new(runtime.try_into().unwrap());
    draft.envelope.statement.context.transaction_intent_digest =
        PrivacyTransactionIntentDigestV1::new([0; 32]);
    let build = |instructions: Vec<InstructionBox>| {
        let mut builder = TransactionBuilder::new(
            draft.envelope.statement.context.network_id,
            account(0x71),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions);
        builder.set_creation_time(std::time::Duration::from_millis(1_800_000_000_000));
        builder.into_payload().unwrap()
    };
    let draft_payload = build(vec![draft.clone().into()]);
    let intent = draft_payload
        .privacy_transaction_intent_digest_v1()
        .unwrap();
    let mut final_instruction = draft.clone();
    final_instruction
        .envelope
        .statement
        .context
        .transaction_intent_digest = intent;
    final_instruction.envelope.statement_digest =
        final_instruction.envelope.statement.digest().unwrap();
    let payload = build(vec![final_instruction.clone().into()]);
    let (observed, original) = payload
        .privacy_transaction_intent_binding_if_present_v1()
        .unwrap()
        .unwrap();
    assert_eq!(observed, intent);
    assert!(matches!(original, PrivacySubmissionRefV1::Ironwood(_)));
    assert_eq!(
        original.canonical_bytes().unwrap(),
        norito::encode_canonical(&final_instruction).unwrap()
    );
    assert_eq!(
        payload
            .privacy_transaction_intent_projection_bytes_v1()
            .unwrap(),
        draft_payload
            .privacy_transaction_intent_projection_bytes_v1()
            .unwrap()
    );
    let mut tampered = final_instruction.clone();
    tampered.envelope.statement.expiry_height += 1;
    let tampered = build(vec![tampered.into()]);
    assert!(
        tampered
            .privacy_transaction_intent_binding_if_present_v1()
            .is_err()
    );
    let old = envelope(statement_for(
        PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1,
    ))
    .into_submission_v1()
    .unwrap();
    let mixed = build(vec![old, final_instruction.clone().into()]);
    assert!(mixed.privacy_transaction_intent_digest_v1().is_err());
    let mut wrong_network = payload.clone();
    wrong_network.domain = crate::transaction::TransactionDomain::Network(network_id(0x99));
    assert!(
        wrong_network
            .privacy_transaction_intent_binding_if_present_v1()
            .is_err()
    );
    let Executable::Instructions(instructions) = payload.instructions() else {
        panic!("direct fixture")
    };
    assert_eq!(instructions.len(), 1);
}

#[test]
fn selected_wire_budget_measures_actual_independent_envelope_frames() {
    for protocol in PrivacyRuntimeProtocolIdV1::SUPPORTED_PROTOCOLS {
        let runtime = envelope(statement_for(protocol));
        let bytes = runtime.canonical_wire_bytes_v1().unwrap();
        let (owned_bytes, statement_bytes) =
            if protocol == PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1 {
                let owned: PrivacyIronwoodProofEnvelopeV1 = runtime.clone().try_into().unwrap();
                (
                    norito::encode_canonical(&owned).unwrap(),
                    norito::encode_canonical(&owned.statement).unwrap(),
                )
            } else {
                let owned: PrivacyProofEnvelopeV1 = runtime.clone().try_into().unwrap();
                (
                    norito::encode_canonical(&owned).unwrap(),
                    norito::encode_canonical(&owned.statement).unwrap(),
                )
            };
        assert_eq!(
            bytes, owned_bytes,
            "selected concrete envelope: {protocol:?}"
        );
        assert_eq!(
            runtime.statement.canonical_wire_bytes_v1().unwrap(),
            statement_bytes,
            "selected concrete statement: {protocol:?}"
        );
        assert_eq!(
            runtime.canonical_wire_frame_len_v1().unwrap(),
            owned_bytes.len()
        );
        if protocol == PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1 {
            let concrete: PrivacyIronwoodProofEnvelopeV1 = runtime.try_into().unwrap();
            assert_eq!(bytes, norito::encode_canonical(&concrete).unwrap());
        } else {
            let frozen: PrivacyProofEnvelopeV1 = runtime.try_into().unwrap();
            assert_eq!(bytes, norito::encode_canonical(&frozen).unwrap());
        }
    }
}

#[test]
fn selected_wire_views_borrow_large_proofs_and_reject_cross_grammar_tags() {
    for protocol in [
        PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1,
        PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
    ] {
        let mut runtime = envelope(statement_for(protocol));
        *runtime.proof.bytes_mut() = PrivacyProofBytesV1::new(vec![0x6c; 1024 * 1024]);
        if protocol == PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1 {
            let view = runtime.ironwood_wire_ref().unwrap();
            assert!(core::ptr::eq(view.proof.0, runtime.proof.bytes()));
        } else {
            let view = runtime.exact12_wire_ref().unwrap();
            assert!(core::ptr::eq(view.proof.0, &runtime.proof));
            assert!(core::ptr::eq(view.statement.0, &runtime.statement));
        }
        let concrete = if protocol == PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1 {
            norito::encode_canonical(
                &PrivacyIronwoodProofEnvelopeV1::try_from(runtime.clone()).unwrap(),
            )
            .unwrap()
        } else {
            norito::encode_canonical(&PrivacyProofEnvelopeV1::try_from(runtime.clone()).unwrap())
                .unwrap()
        };
        assert_eq!(runtime.canonical_wire_bytes_v1().unwrap(), concrete);
        assert_eq!(
            runtime.canonical_wire_frame_len_v1().unwrap(),
            concrete.len()
        );
        runtime.protocol_id = if protocol == PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1 {
            PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1
        } else {
            PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1
        };
        assert!(runtime.canonical_wire_bytes_v1().is_err());
        assert!(runtime.canonical_wire_frame_len_v1().is_err());
    }
}
