//! Native gateway policy commitment, authority separation and canonical codec tests.

use super::*;
use crate::account::{MultisigMember, MultisigPolicy};
use iroha_crypto::{Algorithm, HashOf, KeyPair};

fn account(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

fn policy() -> StreamTokenGatewayPolicyV1 {
    let network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"gateway-policy-network",
    )));
    let compliance_gateway_id = "gateway.dev-registry".to_owned();
    let mut policy = StreamTokenGatewayPolicyV1 {
        network_id,
        qualification: StreamTokenGatewayAdmissionQualificationV1 {
            gateway_id: derive_stream_token_gateway_id_v1(&network_id, &compliance_gateway_id)
                .expect("gateway identity"),
            revision: 1,
            policy_digest: [1; 32],
            max_pending: 128,
            max_tracked_tokens: 64,
            lease_ttl_ms: 120_000,
        },
        compliance_gateway_id,
        operators: BTreeSet::from([account(1)]),
        observers: BTreeSet::from([account(2)]),
        valid_from_unix_ms: 1_800_000_000_000,
        valid_until_unix_ms: 1_800_003_600_000,
        max_observation_age_ms: 30_000,
        admission_enabled: true,
    };
    resign(&mut policy);
    policy
}

fn resign(policy: &mut StreamTokenGatewayPolicyV1) {
    policy.qualification.policy_digest = policy.calculate_policy_digest().expect("policy digest");
}

#[test]
fn policy_digest_binds_every_semantic_field_and_excludes_only_itself() {
    let original = policy();
    original.validate().expect("valid policy");
    let digest = original.qualification.policy_digest;
    let mut arbitrary_digest = original.clone();
    arbitrary_digest.qualification.policy_digest = [9; 32];
    assert_eq!(arbitrary_digest.calculate_policy_digest().unwrap(), digest);
    assert!(arbitrary_digest.validate().is_err());

    let changes: [fn(&mut StreamTokenGatewayPolicyV1); 12] = [
        |p| {
            p.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"other-network",
            )))
        },
        |p| p.compliance_gateway_id.push_str("-other"),
        |p| p.qualification.gateway_id = [3; 32],
        |p| p.qualification.revision += 1,
        |p| p.qualification.max_pending += 1,
        |p| p.qualification.max_tracked_tokens += 1,
        |p| p.qualification.lease_ttl_ms += 1,
        |p| {
            p.operators.insert(account(3));
        },
        |p| {
            p.observers.insert(account(4));
        },
        |p| p.valid_from_unix_ms += 1,
        |p| p.valid_until_unix_ms += 1,
        |p| p.max_observation_age_ms += 1,
    ];
    for change in changes {
        let mut changed = original.clone();
        change(&mut changed);
        assert_ne!(changed.calculate_policy_digest().unwrap(), digest);
        assert!(
            changed.validate().is_err(),
            "old digest cannot authorize new policy"
        );
    }
    let mut disabled = original;
    disabled.admission_enabled = false;
    assert_ne!(disabled.calculate_policy_digest().unwrap(), digest);
    assert!(disabled.validate().is_err());
}

#[test]
fn policy_rejects_malformed_roles_time_scope_and_frame_size() {
    let changes: [fn(&mut StreamTokenGatewayPolicyV1); 10] = [
        |p| p.operators.clear(),
        |p| p.observers.clear(),
        |p| p.observers = p.operators.clone(),
        |p| p.operators = (1..=17).map(account).collect(),
        |p| p.observers = (32..=48).map(account).collect(),
        |p| p.valid_from_unix_ms = 0,
        |p| p.valid_until_unix_ms = p.valid_from_unix_ms,
        |p| p.valid_until_unix_ms = u64::MAX,
        |p| p.max_observation_age_ms = 0,
        |p| p.max_observation_age_ms = STREAM_TOKEN_GATEWAY_MAX_OBSERVATION_AGE_MS_V1 + 1,
    ];
    for change in changes {
        let mut changed = policy();
        change(&mut changed);
        resign(&mut changed);
        assert!(changed.validate().is_err());
    }
    let mut wrong_gateway = policy();
    wrong_gateway.compliance_gateway_id = "gateway.another".to_owned();
    resign(&mut wrong_gateway);
    assert!(wrong_gateway.validate().is_err());
    let mut oversized = policy();
    oversized.compliance_gateway_id = "a".repeat(STREAM_TOKEN_GATEWAY_MAX_POLICY_BYTES_V1);
    assert_eq!(
        oversized.calculate_policy_digest(),
        Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest)
    );
}

#[test]
fn replacement_rotates_authorities_and_limits_without_changing_gateway_identity() {
    let original = policy();
    let mut next = original.clone();
    next.qualification.revision += 1;
    next.qualification.lease_ttl_ms /= 2;
    next.qualification.max_pending = 1;
    next.qualification.max_tracked_tokens = 1;
    next.operators = BTreeSet::from([account(3)]);
    next.observers = BTreeSet::from([account(4)]);
    next.admission_enabled = false;
    resign(&mut next);
    next.validate_replacement(&original)
        .expect("adjacent governed replacement");
    assert_eq!(
        next.qualification.gateway_id,
        original.qualification.gateway_id
    );
    assert!(original.validate_replacement(&original).is_err());
    assert!(original.validate_replacement(&next).is_err());

    let mut gap = next.clone();
    gap.qualification.revision += 1;
    resign(&mut gap);
    assert!(gap.validate_replacement(&original).is_err());
    let mut renamed = next;
    renamed.compliance_gateway_id = "gateway.new".to_owned();
    renamed.qualification.gateway_id =
        derive_stream_token_gateway_id_v1(&renamed.network_id, &renamed.compliance_gateway_id)
            .unwrap();
    resign(&mut renamed);
    renamed.validate().expect("valid distinct gateway");
    assert!(renamed.validate_replacement(&original).is_err());
}

#[test]
fn admission_interval_is_half_open_and_disable_does_not_destroy_policy() {
    let mut policy = policy();
    assert!(!policy.allows_admission_at(policy.valid_from_unix_ms - 1));
    assert!(policy.allows_admission_at(policy.valid_from_unix_ms));
    assert!(policy.allows_admission_at(policy.valid_until_unix_ms - 1));
    assert!(!policy.allows_admission_at(policy.valid_until_unix_ms));
    policy.admission_enabled = false;
    resign(&mut policy);
    policy.validate().expect("valid recovery-only control");
    assert!(!policy.allows_admission_at(policy.valid_from_unix_ms));
}

#[test]
fn distinct_account_ids_cannot_hide_shared_operator_and_observer_keys() {
    let multisig = |members: &[u8]| {
        AccountId::new_multisig(
            MultisigPolicy::new(
                1,
                members
                    .iter()
                    .map(|seed| {
                        MultisigMember::new(account(*seed).expect_single_signatory().clone(), 1)
                            .unwrap()
                    })
                    .collect(),
            )
            .unwrap(),
        )
    };
    let operator = account(1);
    let overlapping = multisig(&[1, 3]);
    assert_ne!(operator, overlapping);
    for (operator, observer) in [
        (operator.clone(), overlapping.clone()),
        (overlapping, operator),
        (multisig(&[1, 3]), multisig(&[2, 3])),
    ] {
        let mut policy = policy();
        policy.operators = BTreeSet::from([operator]);
        policy.observers = BTreeSet::from([observer]);
        assert!(policy.operators.is_disjoint(&policy.observers));
        resign(&mut policy);
        assert_eq!(
            policy.validate(),
            Err(StreamTokenGatewayAdmissionErrorV1::BindingMismatch)
        );
    }
    let mut independent = policy();
    independent.operators = BTreeSet::from([multisig(&[1, 3])]);
    independent.observers = BTreeSet::from([multisig(&[2, 4])]);
    resign(&mut independent);
    independent
        .validate()
        .expect("disjoint multisig controllers");
}

#[test]
fn policy_and_execution_roundtrip_canonical_frames_and_strict_json() {
    let original = policy();
    let frame = norito::encode_canonical(&original).expect("policy encode");
    let decoded: StreamTokenGatewayPolicyV1 =
        norito::decode_canonical(&frame).expect("policy decode");
    assert_eq!(decoded, original);
    decoded.validate().expect("decoded policy binding");
    let json = norito::json::to_json(&original).expect("policy JSON");
    assert_eq!(
        norito::json::from_str::<StreamTokenGatewayPolicyV1>(&json).unwrap(),
        original
    );
    let mut value: norito::json::Value = norito::json::from_str(&json).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("retired_authority".to_owned(), norito::json::Value::Null);
    assert!(norito::json::from_value::<StreamTokenGatewayPolicyV1>(value).is_err());

    let execution = StreamTokenGatewayExecutionV1 {
        height: 2,
        transaction_hash: *Hash::new(b"native-gateway-execution").as_ref(),
        entry_index: 1,
        instruction_index: 0,
        recorded_at_unix_ms: original.valid_from_unix_ms,
        authority: account(1),
    };
    let frame = norito::encode_canonical(&execution).expect("execution encode");
    assert_eq!(
        norito::decode_canonical::<StreamTokenGatewayExecutionV1>(&frame).unwrap(),
        execution
    );
    let json = norito::json::to_json(&execution).expect("execution JSON");
    assert_eq!(
        norito::json::from_str::<StreamTokenGatewayExecutionV1>(&json).unwrap(),
        execution
    );
}
