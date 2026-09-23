//! Structured genesis instruction codecs and validation fixtures.

use super::*;
#[allow(unused_imports)]
use iroha_data_model::{
    alias_setup::{
        AliasDataSpaceIntentV1, AliasIntentV1, AliasLeaseAcquisitionV1, AliasQuoteGuardV1,
        ResolvedDataSpaceV1,
    },
    asset::AssetDefinitionAlias,
    domain::Domain,
    isi::{
        GrantBox, Log, MintBox, RegisterBox, SetParameter, TransferBox,
        alias_setup::EnsureAlias,
        governance::RegisterCitizen,
        nexus::{
            ActivateFeeSponsorProgramRevision, CreateFeeSponsorProgram,
            EnrollFeeSponsorBeneficiary, FundFeeSponsorProgram, StageFeeSponsorProgramRevision,
        },
        staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
    },
    level::Level,
    nexus::{
        FeeSponsorAssetBudget, FeeSponsorEligibility, FeeSponsorNativeInstructionSelector,
        FeeSponsorProgram, FeeSponsorProgramId, FeeSponsorProgramRevision, FeeSponsorRule,
        FeeSponsorRuleEffect, FeeSponsorRuleSelector,
    },
    parameter::{Parameter, TransactionParameter},
    permission::Permission,
    prelude::{
        AccountId, AssetDefinitionId, AssetId, Grant, InstructionBox, Mint, Register, Transfer,
        Unregister,
    },
    role::Role,
};
use iroha_executor_data_model::permission::{
    account::{AccountAliasPermissionScope, CanManageAccountAlias, CanResolveAccountAlias},
    parameter::CanSetParameters,
};
#[allow(unused_imports)]
use iroha_model_base::metadata::Metadata;
#[allow(unused_imports)]
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
use iroha_primitives::json::Json;
use iroha_test_samples::ALICE_ID;
use norito::json::Map;
use std::{collections::BTreeSet, num::NonZeroU64, path::PathBuf};
#[test]
fn instructions_to_value_keeps_structure() {
    let domain = Register::domain(Domain::new(DomainId::try_new("demo", "universal").unwrap()));
    let value = instructions_to_value(&[InstructionBox::from(domain)]);
    let arr = value.as_array().expect("array");
    assert_eq!(arr.len(), 1);
    let outer = arr[0].as_object().expect("outer object");
    assert!(outer.contains_key("Register"));
}
#[test]
fn structured_numeric_fields_handle_u128_exactly() {
    assert_eq!(
        parse_u32(Value::Number(Number::U128(u128::from(u32::MAX))), "lane_id")
            .expect("u32 maximum"),
        u32::MAX
    );
    assert!(
        parse_u32(
            Value::Number(Number::U128(u128::from(u32::MAX) + 1)),
            "lane_id"
        )
        .is_err()
    );
    assert_eq!(
        parse_u64(
            Value::Number(Number::U128(u128::from(u64::MAX))),
            "revision"
        )
        .expect("u64 maximum"),
        u64::MAX
    );
    assert!(
        parse_u64(
            Value::Number(Number::U128(u128::from(u64::MAX) + 1)),
            "revision"
        )
        .is_err()
    );
    assert_eq!(
        parse_numeric(Value::Number(Number::U128(u128::MAX)))
            .expect("u128 must fit the exact Numeric domain")
            .to_string(),
        u128::MAX.to_string()
    );

    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("u128", "universal").expect("domain"),
        "coin".parse().expect("asset name"),
    );
    let mut fields = Map::new();
    fields.insert(
        "asset_definition_id".to_owned(),
        Value::String(asset_definition_id.to_string()),
    );
    fields.insert(
        "lease_expiry_ms".to_owned(),
        Value::Number(Number::U128(u128::from(u64::MAX) + 1)),
    );
    let error = try_decode_set_asset_definition_alias(Value::Object(fields))
        .expect_err("lease expiry above u64 must be rejected");
    assert!(error.to_string().contains("lease_expiry_ms"));
}
#[test]
fn serialize_register_uses_structured_json() {
    let domain = Register::domain(Domain::new(
        DomainId::try_new("structured", "universal").unwrap(),
    ));
    let instruction: InstructionBox = domain.into();
    let mut out = String::new();
    serialize(&[instruction], &mut out);
    let parsed = norito::json::from_str::<Value>(&out).expect("parse serialized JSON");
    let array = parsed.as_array().expect("instructions array");
    assert!(array.first().unwrap().is_object());
}
#[test]
fn role_lifecycle_uses_strict_structured_genesis_json() {
    let role_id: RoleId = "genesis_alias_bootstrap".parse().expect("role id");
    let permission = Permission::from(CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::new(10)),
    });
    let new_role = Role::new(role_id.clone(), ALICE_ID.clone()).add_permission(permission.clone());
    let instructions = vec![
        InstructionBox::from(Register::role(new_role)),
        InstructionBox::from(Unregister::role(role_id.clone())),
    ];
    let encoded = instructions_to_value(&instructions);
    let values = encoded.as_array().expect("instruction array");
    assert!(
        values[0]
            .get("Register")
            .and_then(|value| value.get("Role"))
            .is_some()
    );
    assert!(
        values[1]
            .get("Unregister")
            .and_then(|value| value.get("Role"))
            .is_some()
    );
    assert!(values.iter().all(Value::is_object));

    let decoded = from_value(&encoded).expect("decode structured role lifecycle");
    let RegisterBox::Role(register) = decoded[0]
        .as_any()
        .downcast_ref::<RegisterBox>()
        .expect("role registration")
    else {
        panic!("first instruction must register a role");
    };
    assert_eq!(register.object().inner().id, role_id);
    assert_eq!(register.object().grant_to(), &*ALICE_ID);
    assert_eq!(
        register.object().inner().permissions().collect::<Vec<_>>(),
        vec![&permission]
    );
    let UnregisterBox::Role(unregister) = decoded[1]
        .as_any()
        .downcast_ref::<UnregisterBox>()
        .expect("role unregistration")
    else {
        panic!("second instruction must unregister a role");
    };
    assert_eq!(unregister.object(), &role_id);

    let mut extra = values[0].clone();
    extra
        .get_mut("Register")
        .and_then(|value| value.get_mut("Role"))
        .and_then(Value::as_object_mut)
        .expect("role fields")
        .insert("unexpected".to_owned(), Value::Bool(true));
    let error =
        from_value(&Value::Array(vec![extra])).expect_err("unknown role fields must be rejected");
    assert!(error.to_string().contains("unexpected"));

    let mut nested_extra = values[0].clone();
    nested_extra
        .get_mut("Register")
        .and_then(|value| value.get_mut("Role"))
        .and_then(|value| value.get_mut("permissions"))
        .and_then(Value::as_array_mut)
        .and_then(|permissions| permissions.first_mut())
        .and_then(Value::as_object_mut)
        .expect("permission fields")
        .insert("unexpected".to_owned(), Value::Bool(true));
    let error = from_value(&Value::Array(vec![nested_extra]))
        .expect_err("unknown role permission fields must be rejected");
    assert!(error.to_string().contains("unexpected"));
}
#[test]
fn fee_sponsor_lifecycle_uses_structured_genesis_json() {
    let program_id =
        FeeSponsorProgramId::new(ALICE_ID.clone(), "default".parse().expect("program name"));
    let fee_asset_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("universal", "universal").expect("domain"),
        "xor".parse().expect("asset name"),
    );
    let revision = FeeSponsorProgramRevision {
        program_id: program_id.clone(),
        revision: 1,
        eligibility: FeeSponsorEligibility::EnrolledOnly,
        rules: vec![FeeSponsorRule {
            id: "onboarding".parse().expect("rule name"),
            effect: FeeSponsorRuleEffect::Allow,
            selectors: vec![FeeSponsorRuleSelector::NativeInstruction(
                FeeSponsorNativeInstructionSelector {
                    wire_id: RegisterBox::WIRE_ID.to_owned(),
                    asset_definition_id: None,
                },
            )],
        }],
        asset_budgets: vec![FeeSponsorAssetBudget {
            asset_definition_id: fee_asset_id.clone(),
            per_transaction: Quantity::from(10_u64),
            per_block: Quantity::from(100_u64),
            per_program_epoch: Quantity::from(1_000_u64),
            per_beneficiary_epoch: Quantity::from(100_u64),
            reserve_floor: Quantity::from(10_u64),
            epoch_length_blocks: NonZeroU64::new(100).expect("non-zero"),
        }],
    };
    let instructions = vec![
        InstructionBox::from(CreateFeeSponsorProgram {
            program: FeeSponsorProgram::new(program_id.clone(), program_id.sponsor.clone()),
        }),
        InstructionBox::from(StageFeeSponsorProgramRevision { revision }),
        InstructionBox::from(EnrollFeeSponsorBeneficiary {
            program_id: program_id.clone(),
            beneficiary: ALICE_ID.clone(),
        }),
        InstructionBox::from(FundFeeSponsorProgram {
            program_id: program_id.clone(),
            asset_definition_id: fee_asset_id,
            amount: Quantity::from(1_000_u64),
        }),
        InstructionBox::from(ActivateFeeSponsorProgramRevision {
            program_id,
            revision: 1,
            activate_at_height: 1,
        }),
    ];
    let value = instructions_to_value(&instructions);
    let array = value.as_array().expect("instruction array");
    for (value, expected_key) in array.iter().zip([
        "CreateFeeSponsorProgram",
        "StageFeeSponsorProgramRevision",
        "EnrollFeeSponsorBeneficiary",
        "FundFeeSponsorProgram",
        "ActivateFeeSponsorProgramRevision",
    ]) {
        assert!(
            value
                .as_object()
                .is_some_and(|object| object.contains_key(expected_key)),
            "missing structured {expected_key}: {value:?}"
        );
    }
    let decoded = from_value(&value).expect("decode structured fee sponsor lifecycle");
    assert_eq!(decoded.len(), instructions.len());
    assert!(
        decoded[0]
            .as_any()
            .downcast_ref::<CreateFeeSponsorProgram>()
            .is_some()
    );
    assert!(
        decoded[4]
            .as_any()
            .downcast_ref::<ActivateFeeSponsorProgramRevision>()
            .is_some()
    );
}
#[test]
fn ensure_alias_uses_strict_structured_genesis_json() {
    let payment_asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("assets", "universal").expect("asset domain"),
        "xor".parse().expect("asset name"),
    );
    let ensure = EnsureAlias::new(
        AliasIntentV1::Dataspace(AliasDataSpaceIntentV1 {
            dataspace: ResolvedDataSpaceV1::new(
                "dpn".parse().expect("dataspace alias"),
                DataSpaceId::new(10),
            ),
            owner: ALICE_ID.clone(),
        }),
        AliasLeaseAcquisitionV1::new(1, None),
        AliasQuoteGuardV1 {
            expected_policy_version: 2,
            expected_payment_asset: payment_asset,
            max_amount: Quantity::from(1_u64),
            valid_until_ms: u64::MAX,
        },
    );
    let instruction = InstructionBox::from(ensure.clone());
    let encoded = instructions_to_value(std::slice::from_ref(&instruction));
    let encoded_ensure = encoded
        .as_array()
        .and_then(|array| array.first())
        .and_then(|value| value.get("EnsureAlias"))
        .cloned()
        .expect("structured EnsureAlias object");
    let decoded = from_value(&encoded).expect("decode structured EnsureAlias");
    assert_eq!(
        decoded[0].as_any().downcast_ref::<EnsureAlias>(),
        Some(&ensure)
    );

    let Value::Object(mut extra_fields) = encoded_ensure.clone() else {
        panic!("EnsureAlias fields must be an object");
    };
    extra_fields.insert("unexpected".to_owned(), Value::Null);
    let mut extra_outer = Map::new();
    extra_outer.insert("EnsureAlias".to_owned(), Value::Object(extra_fields));
    let error = from_value(&Value::Array(vec![Value::Object(extra_outer)]))
        .expect_err("unknown EnsureAlias fields must be rejected");
    assert!(error.to_string().contains("unexpected"), "{error}");

    let Value::Object(mut unknown_kind_fields) = encoded_ensure else {
        panic!("EnsureAlias fields must be an object");
    };
    let Value::Object(mut intent) = unknown_kind_fields
        .remove("intent")
        .expect("EnsureAlias intent")
    else {
        panic!("EnsureAlias intent must be an object");
    };
    intent.insert("kind".to_owned(), Value::String("unknown".to_owned()));
    unknown_kind_fields.insert("intent".to_owned(), Value::Object(intent));
    let mut unknown_kind_outer = Map::new();
    unknown_kind_outer.insert("EnsureAlias".to_owned(), Value::Object(unknown_kind_fields));
    from_value(&Value::Array(vec![Value::Object(unknown_kind_outer)]))
        .expect_err("unknown EnsureAlias intent kinds must be rejected");
}
#[test]
fn register_citizen_uses_structured_genesis_json() {
    let instruction = InstructionBox::from(RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: Quantity::from(10_000_u64),
    });
    let value = instructions_to_value(std::slice::from_ref(&instruction));
    let array = value.as_array().expect("instruction array");
    let fields = array[0]
        .as_object()
        .and_then(|outer| outer.get("RegisterCitizen"))
        .and_then(Value::as_object)
        .expect("structured RegisterCitizen");
    assert_eq!(
        fields.get("owner").and_then(Value::as_str),
        ALICE_ID.canonical_i105().ok().as_deref()
    );
    assert_eq!(fields.get("amount").and_then(Value::as_str), Some("10000"));
    let decoded = from_value(&value).expect("decode structured RegisterCitizen");
    assert_eq!(decoded.len(), 1);
    assert_eq!(
        decoded[0].as_any().downcast_ref::<RegisterCitizen>(),
        instruction.as_any().downcast_ref::<RegisterCitizen>()
    );
}
#[test]
fn value_to_instruction_rejects_bytes() {
    let value = Value::Array(vec![Value::Number(Number::U64(1))]);
    let err = value_to_instruction(value).expect_err("byte arrays should be rejected");
    assert!(err.to_string().contains("byte arrays"));
}
#[test]
fn value_to_instruction_rejects_invalid_base64_string() {
    let value = Value::String("***".to_string());
    let err = value_to_instruction(value).expect_err("invalid base64 should fail");
    assert!(err.to_string().contains("invalid base64"));
}
#[test]
fn value_to_instruction_accepts_base64_string_for_custom_instruction() {
    super::super::init_instruction_registry();
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("zk", "universal").expect("domain"),
        "xor".parse().expect("asset name"),
    );
    let instruction = InstructionBox::from(iroha_data_model::isi::zk::RegisterZkAsset::new(
        asset_definition_id,
        None,
        None,
    ));
    let value = instruction_value(&instruction);
    assert!(
        value.is_string(),
        "custom instruction should fall back to base64"
    );
    let decoded = value_to_instruction(value).expect("base64-encoded instruction should decode");
    assert_eq!(
        norito::codec::encode_adaptive(&decoded),
        norito::codec::encode_adaptive(&instruction)
    );
}
#[test]
fn base64_instruction_rejects_valid_noncanonical_norito_layout() {
    super::super::init_instruction_registry();
    let instruction = InstructionBox::from(Log::new(
        Level::INFO,
        "canonical genesis boundary".to_owned(),
    ));
    let canonical =
        norito::encode_canonical(&instruction).expect("encode canonical genesis instruction");
    let canonical_value =
        Value::String(base64::engine::general_purpose::STANDARD.encode(canonical.as_slice()));
    value_to_instruction(canonical_value)
        .expect("canonical base64 genesis instruction must decode");
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let alternate = {
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::core::to_bytes(&instruction).expect("encode valid alternate-layout instruction")
    };
    assert_ne!(alternate, canonical);
    let alternate_value =
        Value::String(base64::engine::general_purpose::STANDARD.encode(alternate.as_slice()));
    let error = value_to_instruction(alternate_value)
        .expect_err("noncanonical base64 genesis instruction must be rejected");
    assert!(error.to_string().contains("canonical"));
}
#[test]
fn structured_genesis_rejects_negative_asset_mint_quantity() {
    let asset_id = AssetId::new(
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "coin".parse().expect("asset name"),
        ),
        ALICE_ID.clone(),
    );
    let source =
        format!(r#"{{"Mint":{{"Asset":{{"object":"-0.01","destination":"{asset_id}"}}}}}}"#);
    let value = norito::json::from_str(&source).expect("parse structured mint");
    let error = value_to_instruction(value)
        .expect_err("negative asset quantity must not enter genesis instructions");
    assert!(error.to_string().contains("invalid asset mint quantity"));
}
#[test]
fn deserialize_structured_instructions_roundtrip() {
    let account_id = ALICE_ID.clone();
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone());
    let asset_def_id: AssetDefinitionId =
        AssetDefinitionId::derive_from_components(domain_id.clone(), "coin".parse().unwrap());
    let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
    let asset_alias: AssetDefinitionAlias = "coin#wonderland.universal".parse().unwrap();
    let parameter = Parameter::Transaction(TransactionParameter::MaxInstructions(
        NonZeroU64::new(64).unwrap(),
    ));
    let instructions: Vec<InstructionBox> = vec![
        Register::domain(domain.clone()).into(),
        Mint::asset_quantity(42u32, asset_id.clone()).into(),
        Transfer::asset_definition(account_id.clone(), asset_def_id.clone(), account_id.clone())
            .into(),
        Grant::account_permission(CanSetParameters, account_id.clone()).into(),
        SetParameter::new(parameter.clone()).into(),
        SetAssetDefinitionAlias::bind(asset_def_id.clone(), asset_alias.clone(), None).into(),
    ];
    let mut json_text = String::new();
    serialize(&instructions, &mut json_text);
    let parsed = norito::json::from_str::<Value>(&json_text).expect("parse serialized JSON");
    let instructions = from_value(&parsed).expect("deserialize instructions");
    assert_eq!(instructions.len(), 6);
    match instructions[0].as_any().downcast_ref::<RegisterBox>() {
        Some(RegisterBox::Domain(reg)) => assert_eq!(reg.object(), &domain),
        other => panic!("unexpected register instruction: {other:?}"),
    }
    match instructions[1].as_any().downcast_ref::<MintBox>() {
        Some(MintBox::Asset(mint)) => {
            assert_eq!(mint.destination(), &asset_id);
            assert_eq!(mint.object().to_string(), "42");
        }
        other => panic!("unexpected mint instruction: {other:?}"),
    }
    match instructions[2].as_any().downcast_ref::<TransferBox>() {
        Some(TransferBox::AssetDefinition(tr)) => {
            assert_eq!(tr.object(), &asset_def_id);
        }
        other => panic!("unexpected transfer instruction: {other:?}"),
    }
    match instructions[3].as_any().downcast_ref::<GrantBox>() {
        Some(GrantBox::Permission(grant)) => {
            assert_eq!(grant.destination(), &account_id);
            assert_eq!(grant.object().name(), "CanSetParameters");
            assert_eq!(grant.object().payload(), &Json::default());
        }
        other => panic!("unexpected grant instruction: {other:?}"),
    }
    match instructions[4].as_any().downcast_ref::<SetParameter>() {
        Some(set_param) => assert_eq!(set_param.inner(), &parameter),
        other => panic!("unexpected set-parameter instruction: {other:?}"),
    }
    match instructions[5]
        .as_any()
        .downcast_ref::<SetAssetDefinitionAlias>()
    {
        Some(set_alias) => {
            assert_eq!(set_alias.asset_definition_id(), &asset_def_id);
            assert_eq!(set_alias.alias().as_ref(), Some(&asset_alias));
            assert_eq!(set_alias.lease_expiry_ms(), &None);
        }
        other => panic!("unexpected set-asset-definition-alias instruction: {other:?}"),
    }
}
#[test]
fn scoped_alias_permission_grants_preserve_payloads_through_genesis_json() {
    let account_id = ALICE_ID.clone();
    let universal = AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL);
    let private = AccountAliasPermissionScope::Dataspace(DataSpaceId::new(10));
    let domain = AccountAliasPermissionScope::Domain(
        DomainId::parse_fully_qualified("hbl.sbp").expect("domain scope must parse"),
    );
    let cases = [universal, private, domain]
        .into_iter()
        .flat_map(|scope| {
            [
                (
                    Permission::from(CanManageAccountAlias {
                        scope: scope.clone(),
                    }),
                    scope.clone(),
                ),
                (
                    Permission::from(CanResolveAccountAlias {
                        scope: scope.clone(),
                    }),
                    scope,
                ),
            ]
        })
        .collect::<Vec<_>>();
    let instructions = cases
        .iter()
        .map(|(permission, _)| {
            Grant::account_permission(permission.clone(), account_id.clone()).into()
        })
        .collect::<Vec<InstructionBox>>();
    let encoded = instructions_to_value(&instructions);
    for instruction in encoded.as_array().expect("instruction array") {
        let payload = instruction
            .get("Grant")
            .and_then(|value| value.get("Permission"))
            .and_then(|value| value.get("object"))
            .and_then(|value| value.get("payload"))
            .expect("structured permission grant must include its payload");
        assert_ne!(
            payload,
            &Value::Null,
            "scoped permission payload must not collapse to null"
        );
    }
    let decoded = from_value(&encoded).expect("decode structured permission grants");
    assert_eq!(decoded.len(), cases.len());
    let mut unique = BTreeSet::new();
    for (instruction, (expected_permission, expected_scope)) in decoded.iter().zip(&cases) {
        let GrantBox::Permission(grant) = instruction
            .as_any()
            .downcast_ref::<GrantBox>()
            .expect("decoded instruction must be a permission grant")
        else {
            panic!("decoded grant must target an account");
        };
        assert_eq!(grant.destination(), &account_id);
        assert_eq!(grant.object(), expected_permission);
        assert!(
            unique.insert((grant.destination().clone(), grant.object().clone())),
            "scoped permission grants must remain distinct"
        );
        match expected_permission.name() {
            "CanManageAccountAlias" => assert_eq!(
                CanManageAccountAlias::try_from(grant.object())
                    .expect("decode manage permission")
                    .scope,
                expected_scope.clone()
            ),
            "CanResolveAccountAlias" => assert_eq!(
                CanResolveAccountAlias::try_from(grant.object())
                    .expect("decode resolve permission")
                    .scope,
                expected_scope.clone()
            ),
            name => panic!("unexpected alias permission `{name}`"),
        }
    }
}
#[test]
fn deserialize_structured_register_account_with_label() {
    let account_id = ALICE_ID.clone();
    let account_literal = account_literal(&account_id).expect("account literal");
    let expected_label = iroha_data_model::account::rekey::AccountAlias::new(
        "admin1".parse().expect("alias label"),
        Some("hbl".parse().expect("alias domain")),
        iroha_model_base::topology::DataSpaceId::new(10),
    );
    let register_json = format!(
        r#"{{
            "Register": {{
                "Account": {{
                    "id": "{account_literal}",
                    "label": {{
                        "label": "admin1",
                        "domain": "hbl",
                        "dataspace": 10
                    }},
                    "metadata": {{}},
                    "opaque_ids": [],
                    "uaid": null
                }}
            }}
        }}"#
    );
    let register_value =
        norito::json::from_str(&register_json).expect("parse register instruction");
    let instruction =
        super::value_to_instruction(register_value).expect("structured account decodes");
    let RegisterBox::Account(account) = instruction
        .as_any()
        .downcast_ref::<RegisterBox>()
        .expect("RegisterBox variant")
    else {
        panic!("expected account registration");
    };
    assert_eq!(account.object().id(), &account_id);
    assert_eq!(account.object().label(), Some(&expected_label));
}
#[test]
fn deserialize_grant_without_payload_defaults_to_null() {
    let account_id = ALICE_ID.clone();
    let account_literal = account_literal(&account_id).expect("account literal");
    let grant_json = format!(
        r#"{{"Grant":{{"Permission":{{"destination":"{account_literal}","object":{{"name":"CanSetParameters"}}}}}}}}"#
    );
    let grant_value = norito::json::from_str(&grant_json).expect("parse grant instruction literal");
    let instruction = super::value_to_instruction(grant_value).expect("structured grant decodes");
    let GrantBox::Permission(grant) = instruction
        .as_any()
        .downcast_ref::<GrantBox>()
        .expect("GrantBox variant")
    else {
        panic!("expected permission grant");
    };
    assert_eq!(grant.destination(), &account_id);
    assert_eq!(grant.object().name(), "CanSetParameters");
    assert_eq!(grant.object().payload(), &Json::default());
}
#[test]
fn deserialize_structured_instructions_supports_npos_bootstrap() {
    let validator_id = ALICE_ID.clone();
    let validator_peer_id = PeerId::from(validator_id.expect_single_signatory().clone());
    let monetary_plan = genesis_registration_plan();
    let register = RegisterPublicLaneValidator::new(
        LaneId::SINGLE,
        validator_id.clone(),
        validator_peer_id.clone(),
        validator_id.clone(),
        Quantity::from(10_u64),
        Metadata::default(),
        monetary_plan.clone(),
    );
    let activate = ActivatePublicLaneValidator::new(LaneId::SINGLE, validator_id.clone());
    let instructions: Vec<InstructionBox> = vec![
        InstructionBox::from(register),
        InstructionBox::from(activate),
    ];
    let mut json_text = String::new();
    serialize(&instructions, &mut json_text);
    let parsed = norito::json::from_str::<Value>(&json_text).expect("parse serialized JSON");
    let instructions = from_value(&parsed).expect("deserialize instructions");
    assert_eq!(instructions.len(), 2);
    match instructions[0]
        .as_any()
        .downcast_ref::<RegisterPublicLaneValidator>()
    {
        Some(register) => {
            assert_eq!(*register.lane_id(), LaneId::SINGLE);
            assert_eq!(register.validator(), &validator_id);
            assert_eq!(register.peer_id(), &validator_peer_id);
            assert_eq!(register.stake_account(), &validator_id);
            assert_eq!(register.initial_stake(), &Quantity::from(10_u64));
            assert!(register.metadata().is_empty());
            assert_eq!(register.monetary_plan(), &monetary_plan);
        }
        other => panic!("unexpected register validator instruction: {other:?}"),
    }
    match instructions[1]
        .as_any()
        .downcast_ref::<ActivatePublicLaneValidator>()
    {
        Some(activate) => {
            assert_eq!(*activate.lane_id(), LaneId::SINGLE);
            assert_eq!(activate.validator(), &validator_id);
        }
        other => panic!("unexpected activate validator instruction: {other:?}"),
    }
}
#[test]
fn deserialize_npos_bootstrap_rejects_negative_initial_stake() {
    let validator_id = ALICE_ID.clone();
    let register = RegisterPublicLaneValidator::new(
        LaneId::SINGLE,
        validator_id.clone(),
        PeerId::from(validator_id.expect_single_signatory().clone()),
        validator_id,
        Quantity::from(10_u64),
        Metadata::default(),
        genesis_registration_plan(),
    );
    let mut json_text = String::new();
    serialize(&[InstructionBox::from(register)], &mut json_text);
    let negative = json_text.replace(r#""initial_stake":"10""#, r#""initial_stake":"-1""#);
    assert_ne!(negative, json_text, "fixture must replace the stake field");
    let parsed = norito::json::from_str::<Value>(&negative)
        .expect("negative quantity remains syntactically valid JSON");
    let error = from_value(&parsed).expect_err("negative initial stake must be rejected");
    assert!(
        error.to_string().contains("invalid initial stake quantity"),
        "unexpected error: {error}"
    );
}
fn genesis_registration_plan() -> iroha_data_model::nexus::PublicLaneMonetaryPlanV1 {
    let definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("staking", "universal").expect("domain"),
        "coin".parse().expect("asset name"),
    );
    iroha_data_model::nexus::PublicLaneMonetaryPlanV1::genesis_registration(
        AssetId::new(definition.clone(), ALICE_ID.clone()),
        AssetId::new(definition, iroha_test_samples::BOB_ID.clone()),
        Quantity::from(10_u64),
    )
}
#[test]
fn deserialize_npos_bootstrap_requires_an_explicit_monetary_plan() {
    let register = RegisterPublicLaneValidator::new(
        LaneId::SINGLE,
        ALICE_ID.clone(),
        PeerId::from(ALICE_ID.expect_single_signatory().clone()),
        ALICE_ID.clone(),
        Quantity::from(10_u64),
        Metadata::default(),
        genesis_registration_plan(),
    );
    let original =
        instruction_to_value(&InstructionBox::from(register)).expect("structured registration");
    for replacement in [None, Some(Value::Null), Some(Value::Object(Map::new()))] {
        let mut value = original.clone();
        let fields = value
            .as_object_mut()
            .expect("instruction object")
            .get_mut("RegisterPublicLaneValidator")
            .expect("registration")
            .as_object_mut()
            .expect("registration fields");
        fields.remove("monetary_plan");
        if let Some(replacement) = replacement {
            fields.insert("monetary_plan".to_owned(), replacement);
        }
        assert!(
            super::value_to_instruction(value).is_err(),
            "missing, null or incomplete monetary authority must be rejected"
        );
    }
}
fn assert_genesis_source_template_parses_structured_instructions(relative_path: &str) {
    super::super::init_instruction_registry();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let value: Value = norito::json::from_str(&raw).expect("parse genesis JSON");
    let chain_discriminant = value
        .as_object()
        .and_then(|obj| obj.get("chain_discriminant"))
        .cloned()
        .and_then(|value| norito::json::value::from_value::<u16>(value).ok())
        .expect("chain_discriminant");
    let _chain_discriminant =
        iroha_data_model::account::address::ChainDiscriminantGuard::enter(chain_discriminant);
    let transactions = value
        .as_object()
        .and_then(|obj| obj.get("transactions"))
        .and_then(Value::as_array)
        .expect("transactions array");
    let mut parameter_blocks = 0;
    for (index, tx) in transactions.iter().enumerate() {
        norito::json::value::from_value::<RawGenesisTx>(tx.clone())
            .unwrap_or_else(|err| panic!("decode transaction {index}: {err}"));
        if let Some(parameters_value) = tx.as_object().and_then(|obj| obj.get("parameters")) {
            let parameters =
                norito::json::value::from_value::<Parameters>(parameters_value.clone())
                    .expect("decode structured parameters");
            parameter_blocks += 1;
            let block = parameters.block();
            assert_eq!(block.max_time_trigger_invocations().get(), 512);
            assert_eq!(
                block.execution_output(),
                iroha_data_model::parameter::ExecutionOutputPolicyV1::bootstrap()
            );
            block
                .execution_output()
                .validate_time_invocations(block.max_time_trigger_invocations().get())
                .expect("template must budget its complete time-trigger execution output");
        }
        if let Some(instructions) = tx
            .as_object()
            .and_then(|obj| obj.get("instructions"))
            .and_then(Value::as_array)
        {
            for instruction in instructions {
                super::value_to_instruction(instruction.clone())
                    .expect("decode structured instruction");
            }
        }
    }
    assert_eq!(parameter_blocks, 1, "one authoritative parameter block");
    assert!(
        super::RawGenesisTransaction::from_path(&path).is_err(),
        "{} must remain an incomplete source template until an operator supplies mint-finality authority",
        path.display()
    );
}
#[test]
fn defaults_genesis_source_template_parses_structured_instructions() {
    assert_genesis_source_template_parses_structured_instructions(
        "../../defaults/genesis.template.json",
    );
}
#[test]
fn taira_genesis_source_template_parses_structured_instructions() {
    assert_genesis_source_template_parses_structured_instructions(
        "../../configs/soranexus/taira/genesis.template.json",
    );
}
#[test]
fn kagami_and_nexus_source_templates_declare_complete_execution_output_limits() {
    for template in [
        "../../defaults/kagami/iroha3-dev/genesis.template.json",
        "../../defaults/kagami/iroha3-nexus/genesis.template.json",
        "../../defaults/nexus/genesis.template.json",
        "../../configs/soranexus/nexus/genesis.template.json",
    ] {
        assert_genesis_source_template_parses_structured_instructions(template);
    }
}
#[test]
fn dev_source_template_prefunds_exact_canonical_staking_plans() {
    use iroha_config::parameters::defaults::nexus::{fees, staking};
    use iroha_data_model::nexus::PublicLaneMonetaryPlanV1;

    super::super::init_instruction_registry();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../defaults/kagami/iroha3-dev/genesis.template.json");
    let template: Value = norito::json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let fields = template.as_object().unwrap();
    let discriminant: u16 =
        norito::json::value::from_value(fields["chain_discriminant"].clone()).unwrap();
    let _guard = iroha_data_model::account::address::ChainDiscriminantGuard::enter(discriminant);
    let definition: AssetDefinitionId = staking::stake_asset_id().parse().unwrap();
    assert_eq!(definition.to_string(), fees::fee_asset_id());
    let escrow = parse_account_id(&staking::stake_escrow_account_id(), "staking escrow").unwrap();
    let mut registered = false;
    let mut prefunded = std::collections::BTreeMap::new();
    let mut registrations = 0;
    for value in fields["transactions"].as_array().unwrap() {
        let transaction: RawGenesisTx = norito::json::value::from_value(value.clone()).unwrap();
        if let Some(parameters) = &transaction.parameters {
            let custom = parameters
                .custom()
                .get(&super::super::SumeragiNposParameters::parameter_id())
                .unwrap();
            let npos = super::super::SumeragiNposParameters::from_custom_parameter(custom).unwrap();
            assert_eq!(npos.xor_asset_definition_id, definition);
        }
        for instruction in transaction.instructions {
            if let Some(RegisterBox::AssetDefinition(register)) =
                instruction.as_any().downcast_ref::<RegisterBox>()
            {
                if register.object().id() == &definition {
                    assert!(!registered, "canonical XOR must be registered once");
                    registered = true;
                }
            }
            if let Some(MintBox::Asset(mint)) = instruction.as_any().downcast_ref::<MintBox>() {
                if mint.destination().definition() == &definition {
                    assert!(registered, "register canonical XOR before minting");
                    assert!(
                        prefunded
                            .insert(mint.destination().clone(), mint.object().to_string())
                            .is_none()
                    );
                }
            }
            if let Some(register) = instruction
                .as_any()
                .downcast_ref::<RegisterPublicLaneValidator>()
            {
                let source = AssetId::new(definition.clone(), register.stake_account().clone());
                assert_eq!(register.validator(), register.stake_account());
                assert_eq!(register.initial_stake(), &Quantity::from(10_000_u64));
                assert_eq!(
                    prefunded.remove(&source),
                    Some(register.initial_stake().to_string())
                );
                assert_eq!(
                    register.monetary_plan(),
                    &PublicLaneMonetaryPlanV1::genesis_registration(
                        source,
                        AssetId::new(definition.clone(), escrow.clone()),
                        register.initial_stake().clone(),
                    )
                );
                registrations += 1;
            }
        }
    }
    assert!(registered);
    assert_eq!(registrations, 4, "all four validators require funded plans");
    assert!(
        prefunded.is_empty(),
        "every staking allocation must be consumed"
    );
}
#[test]
fn parse_allows_null_executor_in_canonical_manifest() {
    let mut manifest_fields = norito::json::Map::new();
    manifest_fields.insert("chain".to_string(), Value::String("test-chain".to_string()));
    manifest_fields.insert(
        "chain_discriminant".to_string(),
        norito::json::value::to_value(&iroha_data_model::account::address::chain_discriminant())
            .expect("serialize chain discriminant"),
    );
    manifest_fields.insert("executor".to_string(), Value::Null);
    manifest_fields.insert(
        "wire_protocol_version".to_string(),
        norito::json::value::to_value(&CONSENSUS_PROTOCOL_VERSION)
            .expect("serialize wire protocol version"),
    );
    manifest_fields.insert("ivm_dir".to_string(), Value::String(".".to_string()));
    manifest_fields.insert(
        "consensus_mode".to_string(),
        Value::String("Permissioned".to_string()),
    );
    manifest_fields.insert(
        "sumeragi_v2".to_string(),
        norito::json::value::to_value(&SumeragiV2GenesisContextParameters::recommended())
            .expect("serialize v2 genesis context"),
    );
    manifest_fields.insert(
        "kagemusha_mint_finality".to_string(),
        norito::json::value::to_value(
            &super::super::deterministic_test_kagemusha_mint_finality_genesis_parameters(),
        )
        .expect("serialize mint-finality authority"),
    );
    manifest_fields.insert(
        "transactions".to_string(),
        Value::Array(vec![Value::Object(norito::json::Map::new())]),
    );
    let manifest = Value::Object(manifest_fields);
    let parsed: RawGenesisTransaction =
        norito::json::value::from_value(manifest).expect("canonical manifest parses");
    assert!(parsed.executor.is_none());
    assert_eq!(parsed.transactions.len(), 1);
}
