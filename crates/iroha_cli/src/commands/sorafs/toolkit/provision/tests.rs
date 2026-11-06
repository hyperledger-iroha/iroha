//! Closed instruction request validation and actual tx-stdin compatibility.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::AccountId,
    sorafs::{
        pin_registry::{
            ProviderIngestCompletionAuthorityV1, ProviderIngestCompletionSignerPolicyV1,
        },
        reserve::{ReserveAuthorityPolicyV1, ReservePolicyV1},
    },
};

const NOW: u64 = 1_800_000_000;

fn account(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

fn build_json(json: norito::json::Value) -> Result<InstructionBox> {
    instruction::build(
        norito::json::from_slice(&norito::json::to_vec(&json)?)?,
        NOW,
    )
}

#[test]
fn reserve_policy_uses_native_validation_and_roundtrips_actual_transaction_stdin() {
    let policy = ReserveAuthorityPolicyV1 {
        version: 1,
        revision: 1,
        predecessor_policy_digest: None,
        economics: ReservePolicyV1::default(),
        asset_definition: iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            iroha_model_base::domain::DomainId::try_new("reserve", "universal").expect("domain"),
            "xor".parse().expect("asset name"),
        ),
        custody_account: account(1),
        treasury_account: account(2),
        operations_authority: account(2),
        decision_authority: account(2),
        grace_period_days: 7,
        default_after_days: 30,
        max_provider_debt: "1000".parse().expect("debt"),
        max_pending_movements_per_provider: 4,
        max_open_appeals_per_provider: 2,
    };
    let json = norito::json!({"kind": "reserve_policy", "payload": policy});
    let instruction = build_json(json).expect("native policy instruction");
    let directory = tempfile::tempdir().expect("temporary output");
    let output = directory.path().join("instruction.json");
    write_instruction(&output, instruction.clone()).expect("instruction JSON");
    let parsed: Vec<InstructionBox> =
        norito::json::from_slice(&std::fs::read(&output).expect("read output"))
            .expect("same path as tx stdin");
    assert_eq!(parsed, vec![instruction]);
    assert!(
        write_instruction(&output, parsed[0].clone()).is_err(),
        "never overwrite reviewed artifacts"
    );
}

#[test]
fn completion_requires_separate_signer_and_canonical_policy_identity() {
    let policy = ProviderIngestCompletionSignerPolicyV1 {
        policy_id: [3; 32],
        revision: 1,
        predecessor_digest: None,
        policy_digest: [4; 32],
    };
    let next = ProviderIngestCompletionAuthorityV1::new(account(1), account(2), policy);
    let id = iroha_data_model::sorafs::capacity::ProviderId::new([9; 32]);
    let valid = norito::json!({"kind": "completion_authority", "payload": {"provider_id": id, "expected_current": null, "next": next}});
    build_json(valid).expect("separate completion authority");
    let next = ProviderIngestCompletionAuthorityV1::new(account(1), account(1), policy);
    assert!(build_json(norito::json!({"kind": "completion_authority", "payload": {"provider_id": id, "expected_current": null, "next": next}})).is_err());
}

#[test]
fn closed_request_rejects_unknown_actions_and_zero_movements() {
    assert!(build_json(norito::json!({"kind": "execute_arbitrary", "payload": {}})).is_err());
    let zero = [0u8; 32];
    let policy = [1u8; 32];
    let error = build_json(norito::json!({"kind": "reserve_decision", "payload": {"movement_id": zero, "expected_provider_revision": 2, "policy_digest": policy, "approve": true, "rationale": "reviewed top-up"}}))
        .expect_err("zero movement must fail semantic validation");
    assert!(error.to_string().contains("movement"));
}

#[test]
fn capacity_constructor_rejects_expiration_and_unknown_fields() {
    let provider_id = [1u8; 32];
    let stake_pool_id = [2u8; 32];
    let json = norito::json!({"kind": "capacity", "payload": {"provider_id": provider_id, "owner": (account(1)), "stake_pool_id": stake_pool_id, "stake_xor": "100", "committed_capacity_gib": 4, "profile_id": "sorafs.sf1@1.0.0", "storage_class": "hot", "valid_from": (NOW - 60), "valid_until": (NOW + 172800)}});
    build_json(json.clone()).expect("native capacity declaration");
    let mut expired = json.clone();
    expired
        .as_object_mut()
        .expect("request object")
        .get_mut("payload")
        .expect("payload")
        .as_object_mut()
        .expect("payload object")
        .insert("valid_until".into(), norito::json::Value::from(NOW));
    assert!(build_json(expired).is_err());
    let mut unknown = json;
    unknown
        .as_object_mut()
        .expect("request object")
        .get_mut("payload")
        .expect("payload")
        .as_object_mut()
        .expect("payload object")
        .insert("ignored".into(), norito::json::Value::Null);
    assert!(build_json(unknown).is_err());
}
