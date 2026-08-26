#[test]
fn mutation_draft_json_is_an_exact_closed_v1_object() {
    let signer = sample_ed25519_keypair(0xD4);
    let canonical = SoracloudMutationDraftResponse {
        ok: true,
        authority: AccountId::new(signer.public_key().clone()),
        signed_by: signer.public_key().clone(),
        tx_instructions: vec![SoracloudTxInstruction {
            wire_id: "iroha.soracloud.agent.autonomy.run".to_owned(),
            payload_hex: "00ff".to_owned(),
        }],
    };
    canonical.validate().expect("canonical draft must validate");

    let value = norito::json::to_value(&canonical).expect("encode canonical draft JSON");
    assert_eq!(
        norito::json::from_value::<SoracloudMutationDraftResponse>(value.clone())
            .expect("decode canonical draft JSON"),
        canonical
    );

    for required in ["ok", "authority", "signed_by", "tx_instructions"] {
        let mut missing = value.clone();
        missing
            .as_object_mut()
            .expect("draft JSON object")
            .remove(required);
        norito::json::from_value::<SoracloudMutationDraftResponse>(missing)
            .expect_err("an omitted draft field must be rejected");
    }

    let mut unknown = value.clone();
    unknown
        .as_object_mut()
        .expect("draft JSON object")
        .insert("legacy_payload".to_owned(), norito::json::Value::from(true));
    norito::json::from_value::<SoracloudMutationDraftResponse>(unknown)
        .expect_err("an unknown draft field must be rejected");

    let instruction = value
        .get("tx_instructions")
        .and_then(norito::json::Value::as_array)
        .and_then(|instructions| instructions.first())
        .expect("canonical instruction JSON")
        .clone();
    for required in ["wire_id", "payload_hex"] {
        let mut missing = instruction.clone();
        missing
            .as_object_mut()
            .expect("instruction JSON object")
            .remove(required);
        norito::json::from_value::<SoracloudTxInstruction>(missing)
            .expect_err("an omitted instruction field must be rejected");
    }
    let mut unknown = instruction;
    unknown
        .as_object_mut()
        .expect("instruction JSON object")
        .insert("raw_payload".to_owned(), norito::json::Value::from(true));
    norito::json::from_value::<SoracloudTxInstruction>(unknown)
        .expect_err("an unknown instruction field must be rejected");
}

#[test]
fn mutation_draft_validation_rejects_empty_or_noncanonical_successes() {
    let signer = sample_ed25519_keypair(0xD5);
    let instruction = SoracloudTxInstruction {
        wire_id: "iroha.soracloud.agent.autonomy.run".to_owned(),
        payload_hex: "00ff".to_owned(),
    };
    let mut draft = SoracloudMutationDraftResponse {
        ok: true,
        authority: AccountId::new(signer.public_key().clone()),
        signed_by: signer.public_key().clone(),
        tx_instructions: vec![instruction],
    };

    draft.tx_instructions.clear();
    assert!(
        draft.validate().is_err(),
        "empty successful draft must fail"
    );
    draft.tx_instructions.push(SoracloudTxInstruction {
        wire_id: "iroha.soracloud.agent.autonomy.run".to_owned(),
        payload_hex: "00FF".to_owned(),
    });
    assert!(
        draft.validate().is_err(),
        "non-canonical uppercase payload hex must fail"
    );
    draft.tx_instructions[0].payload_hex = "00ff".to_owned();
    draft.ok = false;
    assert!(
        draft.validate().is_err(),
        "a false success marker must not be admitted as a draft response"
    );
}
