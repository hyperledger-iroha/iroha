// Validation-fee JavaScript host fixtures, wire parity, fingerprints, and rejection cases.
fn validation_fee_account(seed: u8) -> AccountId {
    let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("validation-fee fixture keypair");
    AccountId::new(keypair.public_key().clone())
}
fn validation_fee_proposal_operator_fixture() -> AccountId {
    validation_fee_account(7)
}
fn validation_fee_asset(domain: &str, name: &str) -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new(domain, "universal").expect("validation-fee fixture domain"),
        name.parse().expect("validation-fee fixture asset name"),
    )
}
fn validation_fee_payout_binding_fixture() -> ValidationFeeTreasuryPayoutBindingV1 {
    let contract_address: ContractAddress =
        "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
            .parse()
            .expect("validation-fee payout contract address");
    ValidationFeeTreasuryPayoutBindingV1 {
        treasury_account_id: contract_address.subject_id(),
        contract_address,
        code_hash: [0x34; 32],
        entrypoint: "autonomous_validation_fee_tick"
            .parse()
            .expect("validation-fee payout entrypoint"),
        ds_asset_id: validation_fee_asset("cbsi", "ds"),
        xor_asset_id: validation_fee_asset("xor", "xor"),
        pool_vault_account_id: validation_fee_account(2),
        batch_ds: validation_fee_payout_batch_ds(),
        min_xor_out: validation_fee_payout_min_xor(),
        max_xor_out: validation_fee_payout_max_xor(),
        recipients: (3..=6)
            .map(|seed| ValidationFeeTreasuryPayoutRecipientV1 {
                account_id: validation_fee_account(seed),
                share: validation_fee_payout_recipient_share(),
            })
            .collect(),
    }
}
fn validation_fee_policy_fixture(
    payout_binding: Option<ValidationFeeTreasuryPayoutBindingV1>,
) -> ValidationFeePolicyV1 {
    let treasury_account_id = payout_binding.as_ref().map_or_else(
        || validation_fee_account(1),
        |binding| binding.treasury_account_id.clone(),
    );
    ValidationFeePolicyV1 {
        schema_version: VALIDATION_FEE_POLICY_SCHEMA_VERSION,
        network_id: NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0x13; 32]),
        )),
        policy_version: 1,
        previous_policy_hash: None,
        ds_asset_id: validation_fee_asset("cbsi", "ds"),
        ds_scale: VALIDATION_FEE_DS_SCALE,
        fee: initial_validation_fee_amount(),
        treasury_account_id,
        charging_mode: ValidationFeeChargingMode::PerQualifyingTransferInstruction,
        effective_from_height: 121_100,
        expires_after_height: None,
        exemption_classes: payout_binding
            .as_ref()
            .map(|_| vec![VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS.to_owned()])
            .unwrap_or_default(),
        treasury_payout_binding: payout_binding,
    }
}
fn assert_validation_fee_policy_instruction_roundtrip(
    policy: ValidationFeePolicyV1,
    payout_lifecycle_proposal_id: Option<[u8; 32]>,
) {
    const WIRE_ID: &str = "iroha.instruction.v1::governance::ProposeValidationFeePolicy";
    let instruction: InstructionBox = ProposeValidationFeePolicy {
        policy,
        payout_lifecycle_proposal_id,
    }
    .into();
    let original_dyn_bytes = InstructionTrait::dyn_encode(&*instruction);
    assert_eq!(
        iroha_data_model::isi::instruction_wire_id(&instruction),
        Some(WIRE_ID)
    );
    let framed =
        iroha_data_model::isi::frame_instruction_payload(WIRE_ID, original_dyn_bytes.as_slice())
            .expect("frame validation-fee instruction");
    let decoded =
        decode_instruction_aligned(&framed).expect("decode framed validation-fee instruction");
    assert_eq!(
        InstructionTrait::id(&*decoded),
        InstructionTrait::id(&*instruction)
    );
    assert_eq!(
        InstructionTrait::dyn_encode(&*decoded),
        original_dyn_bytes,
        "typed frame decode must preserve exact native instruction bytes"
    );
    let json_value =
        instruction_to_json_value(&decoded).expect("render validation-fee instruction JSON");
    let json_payload = json::to_json(&json_value).expect("encode validation-fee JSON");
    let reconstructed =
        value_to_instruction(json_value).expect("rebuild validation-fee instruction");
    assert_eq!(InstructionTrait::id(&*reconstructed), WIRE_ID);
    assert_eq!(
        InstructionTrait::dyn_encode(&*reconstructed),
        original_dyn_bytes,
        "decoded JSON must rebuild the exact native instruction bytes"
    );
    let network_id = test_network_id(b"validation-fee-js-test");
    let draft = build_transaction_payload_from_instructions_json(
        network_id,
        validation_fee_account(7),
        vec![json_payload],
        authority_fee_payment_json(),
        None,
        Some(1_700_000_000_000),
        Some(60_000),
        Some(9),
    )
    .expect("build validation-fee transaction payload");
    let payload: TransactionPayload =
        json::from_json(&draft.payload_json).expect("decode validation-fee draft payload");
    assert_eq!(
        payload.domain,
        iroha_data_model::transaction::TransactionDomain::Network(network_id),
        "validation-fee draft must bind the exact requested NetworkId"
    );
    let Executable::Instructions(batch) = &payload.instructions else {
        panic!("validation-fee draft must contain an instruction batch")
    };
    let rebuilt = batch
        .iter()
        .next()
        .expect("validation-fee draft instruction");
    assert_eq!(InstructionTrait::id(&**rebuilt), WIRE_ID);
    assert_eq!(
        InstructionTrait::dyn_encode(&**rebuilt),
        original_dyn_bytes,
        "buildTransactionPayload path must preserve exact native instruction bytes"
    );
}
#[test]
fn validation_fee_policy_instruction_roundtrips_without_payout() {
    assert_validation_fee_policy_instruction_roundtrip(validation_fee_policy_fixture(None), None);
}
#[test]
fn validation_fee_policy_instruction_roundtrips_with_payout_and_even_hashes() {
    assert_validation_fee_policy_instruction_roundtrip(
        validation_fee_policy_fixture(Some(validation_fee_payout_binding_fixture())),
        Some([0x56; 32]),
    );
}
#[test]
fn validation_fee_policy_instruction_json_rejects_unknown_and_legacy_fields() {
    let instruction: InstructionBox = ProposeValidationFeePolicy {
        policy: validation_fee_policy_fixture(None),
        payout_lifecycle_proposal_id: None,
    }
    .into();
    let mut value =
        instruction_to_json_value(&instruction).expect("validation-fee instruction JSON");
    value
        .get_mut("ProposeValidationFeePolicy")
        .and_then(json::Value::as_object_mut)
        .expect("validation-fee instruction fields")
        .insert("window".to_owned(), json::Value::Null);
    let error = value_to_instruction(value).expect_err("legacy window alias must be rejected");
    assert!(error.reason.contains("must contain exactly"));
    let mut value =
        instruction_to_json_value(&instruction).expect("validation-fee instruction JSON");
    let policy = value
        .get_mut("ProposeValidationFeePolicy")
        .and_then(|value| value.get_mut("policy"))
        .and_then(json::Value::as_object_mut)
        .expect("validation-fee policy fields");
    policy.remove("network_id");
    policy.insert(
        "chain_id".to_owned(),
        json::Value::String("legacy".to_owned()),
    );
    policy.insert(
        "genesis_hash".to_owned(),
        json::Value::String("13".repeat(32)),
    );
    let error =
        value_to_instruction(value).expect_err("legacy dual identity fields must be rejected");
    assert!(error.reason.contains("must contain exactly"));
}
#[test]
fn validation_fee_policy_proposal_fingerprint_matches_native_kind() {
    for (policy, payout_lifecycle_proposal_id) in [
        (validation_fee_policy_fixture(None), None),
        (
            validation_fee_policy_fixture(Some(validation_fee_payout_binding_fixture())),
            Some([0x56; 32]),
        ),
    ] {
        let proposal_operator = validation_fee_proposal_operator_fixture();
        let expected = ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
            proposal_operator: proposal_operator.clone(),
            policy: policy.clone(),
            payout_lifecycle_proposal_id,
        })
        .fingerprint();
        let policy_json = json::to_json(&policy).expect("validation-fee policy JSON");
        let actual = validation_fee_policy_proposal_fingerprint_v1(
            proposal_operator.to_string(),
            policy_json,
            payout_lifecycle_proposal_id.map(|id| Uint8Array::from(id.to_vec())),
        )
        .expect("validation-fee policy fingerprint");
        assert_eq!(actual.as_ref(), expected);
    }
}
#[test]
fn validation_fee_policy_proposal_fingerprint_binds_the_exact_operator() {
    let policy_json =
        json::to_json(&validation_fee_policy_fixture(None)).expect("validation-fee policy JSON");
    let first = validation_fee_policy_proposal_fingerprint_v1(
        validation_fee_account(7).to_string(),
        policy_json.clone(),
        None,
    )
    .expect("first operator fingerprint");
    let second = validation_fee_policy_proposal_fingerprint_v1(
        validation_fee_account(8).to_string(),
        policy_json,
        None,
    )
    .expect("second operator fingerprint");
    assert_ne!(first.as_ref(), second.as_ref());
}
#[test]
fn validation_fee_payout_lifecycle_proposal_fingerprint_matches_native_kind() {
    let payout_binding = validation_fee_payout_binding_fixture();
    let proposal_operator = validation_fee_proposal_operator_fixture();
    let expected =
        ProposalKind::ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal {
            proposal_operator: proposal_operator.clone(),
            payout_binding: payout_binding.clone(),
        })
        .fingerprint();
    let actual = validation_fee_payout_lifecycle_proposal_fingerprint_v1(
        proposal_operator.to_string(),
        json::to_json(&payout_binding).expect("validation-fee payout binding JSON"),
    )
    .expect("validation-fee payout lifecycle fingerprint");
    assert_eq!(actual.as_ref(), expected);
}
#[test]
fn validation_fee_proposal_fingerprints_match_native_release_preimages() {
    let proposal_operator = validation_fee_proposal_operator_fixture();
    let policy = validation_fee_policy_fixture(None);
    let policy_fingerprint = validation_fee_policy_proposal_fingerprint_v1(
        proposal_operator.to_string(),
        json::to_json(&policy).expect("validation-fee policy JSON"),
        None,
    )
    .expect("validation-fee policy fingerprint");
    assert_eq!(
        policy_fingerprint.as_ref(),
        ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
            proposal_operator: proposal_operator.clone(),
            policy,
            payout_lifecycle_proposal_id: None,
        })
        .fingerprint()
    );
    let payout_binding = validation_fee_payout_binding_fixture();
    let payout_fingerprint = validation_fee_payout_lifecycle_proposal_fingerprint_v1(
        proposal_operator.to_string(),
        json::to_json(&payout_binding).expect("validation-fee payout binding JSON"),
    )
    .expect("validation-fee payout lifecycle fingerprint");
    assert_eq!(
        payout_fingerprint.as_ref(),
        ProposalKind::ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal {
            proposal_operator,
            payout_binding,
        })
        .fingerprint()
    );
}
#[test]
fn validation_fee_policy_proposal_fingerprint_rejects_unknown_policy_fields() {
    let policy = validation_fee_policy_fixture(None);
    let mut value = json::to_value(&policy).expect("validation-fee policy JSON");
    value
        .as_object_mut()
        .expect("validation-fee policy fields")
        .insert(
            "fee_asset_id".to_owned(),
            json::Value::String("legacy".to_owned()),
        );
    let result = validation_fee_policy_proposal_fingerprint_v1(
        validation_fee_proposal_operator_fixture().to_string(),
        json::to_json(&value).expect("legacy validation-fee policy JSON"),
        None,
    );
    assert_napi_error_contains!(
        result,
        "legacy policy alias must be rejected",
        "must contain exactly"
    );
}
#[test]
fn validation_fee_payout_lifecycle_rejects_retired_sbd_field_names() {
    let payout_binding = validation_fee_payout_binding_fixture();
    let mut value = json::to_value(&payout_binding).expect("payout binding JSON");
    let fields = value.as_object_mut().expect("payout binding JSON fields");
    let ds_asset_id = fields.remove("ds_asset_id").expect("DS asset field");
    fields.insert("sbd_asset_id".to_owned(), ds_asset_id);
    let result = validation_fee_payout_lifecycle_proposal_fingerprint_v1(
        validation_fee_proposal_operator_fixture().to_string(),
        json::to_json(&value).expect("retired payout binding JSON"),
    );
    assert_napi_error_contains!(
        result,
        "retired payout binding field must be rejected",
        "must contain exactly"
    );
}
#[test]
fn validation_fee_policy_proposal_fingerprint_rejects_wrong_contract_subject() {
    let mut policy = validation_fee_policy_fixture(Some(validation_fee_payout_binding_fixture()));
    policy.treasury_account_id = validation_fee_account(9);
    let result = validation_fee_policy_proposal_fingerprint_v1(
        validation_fee_proposal_operator_fixture().to_string(),
        json::to_json(&policy).expect("mismatched validation-fee policy JSON"),
        Some(Uint8Array::from(vec![0x56; 32])),
    );
    assert_napi_error_contains!(
        result,
        "mismatched payout contract subject must be rejected",
        "contract subject must equal the policy treasury"
    );
}
#[test]
fn validation_fee_payout_lifecycle_fingerprint_rejects_invalid_binding() {
    let mut payout_binding = validation_fee_payout_binding_fixture();
    payout_binding.code_hash = [0; 32];
    let result = validation_fee_payout_lifecycle_proposal_fingerprint_v1(
        validation_fee_proposal_operator_fixture().to_string(),
        json::to_json(&payout_binding).expect("validation-fee payout binding JSON"),
    );
    assert_napi_error_contains!(
        result,
        "invalid payout binding must fail closed",
        "code hash must be non-zero"
    );
}
