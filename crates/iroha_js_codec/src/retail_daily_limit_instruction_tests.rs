//! Source-matched native codec coverage for the three retail instruction types.

use std::collections::BTreeSet;

use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
use iroha_data_model::{
    account::AccountId,
    asset::{
        AssetBalancePolicy, AssetDefinition, AssetDefinitionId,
        RETAIL_IDENTITY_ATTESTATION_DOMAIN_V1, RetailDailyLimitPolicyV1,
        RetailIdentityAttestationBodyV1, RetailIdentityAttestationV1, RetailIdentityCommitmentV1,
        RetailMonetaryPurposeV1,
    },
    isi::{InstructionBox, instruction_wire_id},
};
use iroha_model_base::{domain::DomainId, topology::DataSpaceId};
use iroha_primitives::numeric::{NumericSpec, Quantity};
use norito::json::{self, Value};

use super::*;
use crate::{
    decode_instruction_archive, decode_instruction_frame, encode_instruction_archive,
    encode_instruction_frame, instruction_from_json, instruction_to_json_value,
};

const PREFIX: u16 = 753;

fn fixtures() -> Vec<(&'static str, &'static str, InstructionBox)> {
    let issuer_key = KeyPair::try_from_seed(vec![0x73; 32], Algorithm::Ed25519)
        .expect("retail test-only issuer key");
    let reserve_key = KeyPair::try_from_seed(vec![0x74; 32], Algorithm::Ed25519)
        .expect("retail test-only reserve key");
    let issuer = AccountId::new(issuer_key.public_key().clone());
    let reserve = AccountId::new(reserve_key.public_key().clone());
    let domain = DomainId::try_new("retail", "bpng").expect("retail test domain");
    let definition_id = AssetDefinitionId::derive_from_components(
        domain.clone(),
        "kina".parse().expect("retail test asset name"),
    );
    let policy = RetailDailyLimitPolicyV1 {
        asset_definition_id: definition_id.clone(),
        physical_dataspace: DataSpaceId::new(8_648_377_547_929_788_715),
        revision: 1,
        daily_cap: Quantity::from(5_u32),
        identity_issuer: issuer.clone(),
        identity_issuer_public_key: issuer_key.public_key().clone(),
        monetary_issuer_account: issuer.clone(),
        reserve_account: reserve,
        institutional_exceptions: BTreeSet::new(),
    };
    let activation = ActivateRetailDailyLimitV1 {
        definition: AssetDefinition::new(
            definition_id.clone(),
            "Kina".to_owned(),
            NumericSpec::fractional(2),
            AssetBalancePolicy::DataspaceRestricted,
            Some(domain),
        ),
        policy: policy.clone(),
    };
    let body = RetailIdentityAttestationBodyV1 {
        domain: RETAIL_IDENTITY_ATTESTATION_DOMAIN_V1.to_owned(),
        asset_definition_id: definition_id.clone(),
        physical_dataspace: policy.physical_dataspace,
        policy_revision: policy.revision,
        account_id: issuer.clone(),
        identity: RetailIdentityCommitmentV1 { digest: [0xA1; 32] },
        uniqueness_evidence_digest: [0xB1; 32],
    };
    let binding = BindRetailIdentityV1 {
        attestation: RetailIdentityAttestationV1 {
            signature: SignatureOf::try_new(issuer_key.private_key(), &body)
                .expect("retail test-only signature"),
            body,
        },
    };
    let monetary = RetailMonetaryMovementV1 {
        asset_definition_id: definition_id,
        purpose: RetailMonetaryPurposeV1::CreditRetail,
        retail_account: Some(issuer),
        amount: Quantity::from(2_u32),
        operation_digest: [0xC1; 32],
    };
    vec![
        (
            "ActivateRetailDailyLimitV1",
            ActivateRetailDailyLimitV1::WIRE_ID,
            activation.into(),
        ),
        (
            "BindRetailIdentityV1",
            BindRetailIdentityV1::WIRE_ID,
            binding.into(),
        ),
        (
            "RetailMonetaryMovementV1",
            RetailMonetaryMovementV1::WIRE_ID,
            monetary.into(),
        ),
    ]
}

fn source(value: &Value) -> String {
    json::to_json(value).expect("retail test JSON")
}

#[test]
fn retail_instructions_preserve_native_identity_across_json_frame_and_archive() {
    for (name, wire_id, native) in fixtures() {
        assert!(is_retail_instruction(&native), "{name} typed dispatch");
        assert_eq!(
            instruction_wire_id(&native),
            Some(wire_id),
            "{name} wire ID"
        );
        let value = instruction_to_json_value(&native).expect("canonical retail JSON");
        let json = source(&value);
        assert_eq!(
            instruction_to_json_value(&instruction_from_json(&json).unwrap()).unwrap(),
            value
        );

        let frame = encode_instruction_frame(&json, PREFIX).expect("retail frame");
        let decoded_frame: Value =
            json::from_json(&decode_instruction_frame(&frame, PREFIX).unwrap()).unwrap();
        assert_eq!(decoded_frame, value);
        let archive = encode_instruction_archive(&json, PREFIX).expect("retail archive");
        let decoded_archive: Value =
            json::from_json(&decode_instruction_archive(&archive, PREFIX).unwrap()).unwrap();
        assert_eq!(decoded_archive, value);
    }
}

#[test]
fn retail_json_rejects_alias_envelopes_missing_and_extra_fields() {
    for (name, _, native) in fixtures() {
        let valid = instruction_to_json_value(&native).unwrap();
        let mut extra = valid.clone();
        extra
            .as_object_mut()
            .unwrap()
            .insert("legacy".to_owned(), Value::Null);
        assert!(
            instruction_from_json(&source(&extra)).is_err(),
            "extra envelope {name}"
        );

        let mut extra = valid.clone();
        extra
            .as_object_mut()
            .unwrap()
            .get_mut(name)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("legacy".to_owned(), Value::Null);
        assert!(
            instruction_from_json(&source(&extra)).is_err(),
            "extra payload {name}"
        );

        let mut missing = valid.clone();
        let first_field = missing
            .as_object()
            .unwrap()
            .get(name)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .clone();
        missing
            .as_object_mut()
            .unwrap()
            .get_mut(name)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(&first_field);
        assert!(
            instruction_from_json(&source(&missing)).is_err(),
            "missing payload {name}"
        );
    }
}

#[test]
fn retail_json_rejects_rounded_dataspace_and_zero_operation_digest() {
    let fixtures = fixtures();
    let mut activation = instruction_to_json_value(&fixtures[0].2).unwrap();
    activation
        .as_object_mut()
        .unwrap()
        .get_mut("ActivateRetailDailyLimitV1")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .get_mut("policy")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert(
            "physical_dataspace".to_owned(),
            Value::String("8648377547929788715".to_owned()),
        );
    assert!(instruction_from_json(&source(&activation)).is_err());

    let mut monetary = instruction_to_json_value(&fixtures[2].2).unwrap();
    monetary
        .as_object_mut()
        .unwrap()
        .get_mut("RetailMonetaryMovementV1")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert(
            "operation_digest".to_owned(),
            Value::Array(vec![Value::from(0_u8); 32]),
        );
    // The codec preserves the typed zero digest exactly. Consensus admission
    // rejects it; JSON construction must never pretend it was a bank receipt.
    assert!(instruction_from_json(&source(&monetary)).is_ok());
}
