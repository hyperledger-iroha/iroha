//! Exact wallet custody plan boundaries using public test keys and real signatures.
//!
//! Fixture records are caller claims; these controls do not authenticate native
//! execution, provider permission, finality, hardware or operational custody.

use super::*;
use iroha_crypto::{Algorithm, KeyPair, Signature};
use sorafs_manifest::signer::{
    custody::{
        SIGNER_CUSTODY_MAGIC_V1, SIGNER_CUSTODY_VERSION_V1, SignerCustodyAuthorityV1,
        SignerCustodyRecordV1, SignerCustodyStatementV1,
    },
    protocol::SignerKeyAlgorithmV1,
};
use std::time::Instant;

fn test_key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap()
}

fn fixture() -> (Config, SignerCustodyPolicyV1) {
    let config = super::super::tests::fixture_config();
    // Canonical public labels exercise the real grammar; deterministic keys and
    // caller claims below remain disposable test material, not qualified custody.
    let policy = SignerCustodyPolicyV1 {
        binding: SignerCustodyBindingV1 {
            chain_id: config.chain.to_string(),
            network_id: *config.network_id.as_bytes(),
            runtime_handle: "hsm://stream/primary".into(),
            key_handle: "pkcs11:stream/key-1".into(),
            service_id: "stream-service".into(),
            administrator_id: "stream-admin".into(),
            role: SignerRoleV1::StreamToken,
            purpose: SignerPurposeBindingV1::StreamToken {
                provider_id: [3; 32],
            },
            algorithm: SignerKeyAlgorithmV1::Ed25519,
            public_key: test_key(4).public_key().clone(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [5; 32],
        },
        attester_authority: SignerCustodyAuthorityV1 {
            service_id: "custody-service".into(),
            administrator_id: "custody-admin".into(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [6; 32],
        },
        attester_public_key: test_key(7).public_key().clone(),
        active_from_unix_ms: 100,
        active_until_unix_ms: 5_000,
        max_validity_ms: 2_000,
        max_anchor_age_ms: 1_000,
    };
    policy.validate().unwrap();
    (config, policy)
}

fn absent_selection(policy: &SignerCustodyPolicyV1) -> StreamTokenCustodySelection {
    StreamTokenCustodySelection {
        provider_id: ProviderId::new([3; 32]),
        binding: policy.binding.clone(),
        expected_revision: 0,
        expected_digest: [0; 32],
        current: None,
    }
}

fn configure_plan(policy: SignerCustodyPolicyV1) -> Plan {
    Plan {
        selection: absent_selection(&policy),
        action: Action::Configure(policy),
        validated_at_unix_ms: 1_000,
        deadline_unix_ms: 2_000,
    }
}

fn enrolled_request_plan(config: &Config, policy: SignerCustodyPolicyV1, revoked: bool) -> Plan {
    let mut state = configure_signer_custody_policy_v1(None, policy.clone()).unwrap();
    state.signer_revoked = revoked;
    let record = StreamTokenCustodyControlRecordV1 {
        provider_id: ProviderId::new([3; 32]),
        revision: 1,
        predecessor_digest: [0; 32],
        request_digest: [8; 32],
        execution_height: 10,
        ordinal: 0,
        recorded_at_unix_ms: 800,
        authority: config.account.clone(),
        control_state: encode_bounded(&state, SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1).unwrap(),
        active_enrollment: None,
    };
    let digest = record.canonical_digest().unwrap();
    let anchor = SignerCustodyAnchorV1 {
        height: 10,
        block_hash: [9; 32],
        state_digest: digest,
    };
    let statement = SignerCustodyStatementV1 {
        magic: SIGNER_CUSTODY_MAGIC_V1,
        version: SIGNER_CUSTODY_VERSION_V1,
        binding: policy.binding.clone(),
        authority: policy.attester_authority.clone(),
        anchor,
        sequence: 1,
        predecessor_digest: [0; 32],
        issued_at_unix_ms: 900,
        expires_at_unix_ms: 2_000,
        evidence_digest: [10; 32],
        revoked: false,
    };
    let signature = Signature::try_new(
        test_key(7).private_key(),
        &statement.signing_payload().unwrap(),
    )
    .unwrap();
    let signed = SignerCustodyRecordV1 {
        statement,
        attestation: signature.payload().try_into().unwrap(),
    };
    Plan {
        selection: StreamTokenCustodySelection {
            provider_id: ProviderId::new([3; 32]),
            binding: policy.binding,
            expected_revision: 1,
            expected_digest: digest,
            current: Some(record),
        },
        action: Action::Enroll {
            anchor,
            anchor_observed_at_unix_ms: 950,
            issued_at_unix_ms: 900,
            expires_at_unix_ms: 2_000,
            enrollment: encode_bounded(&signed, SIGNER_CUSTODY_MAX_BYTES_V1).unwrap(),
        },
        validated_at_unix_ms: 1_000,
        deadline_unix_ms: 1_900,
    }
}

#[test]
fn configure_plan_retains_exact_instruction_canonical_bytes_purpose_and_deadline() {
    let (config, policy) = fixture();
    for change in [
        |policy: &mut SignerCustodyPolicyV1| {
            policy.binding.runtime_handle = "hsm://stream/test-only".into();
        },
        |policy: &mut SignerCustodyPolicyV1| {
            policy.binding.key_handle = "pkcs11:stream/test-only".into();
        },
        |policy: &mut SignerCustodyPolicyV1| {
            policy.binding.service_id = "test-stream-service".into();
        },
        |policy: &mut SignerCustodyPolicyV1| {
            policy.binding.administrator_id = "test-stream-admin".into();
        },
        |policy: &mut SignerCustodyPolicyV1| {
            policy.attester_authority.service_id = "test-custody-service".into();
        },
        |policy: &mut SignerCustodyPolicyV1| {
            policy.attester_authority.administrator_id = "test-custody-admin".into();
        },
    ] {
        let mut malformed = policy.clone();
        change(&mut malformed);
        assert!(malformed.validate().is_err());
    }
    let plan = configure_plan(policy.clone());
    let expected: InstructionBox = MutateSorafsStreamTokenCustody {
        provider_id: ProviderId::new([3; 32]),
        expected_revision: 0,
        expected_digest: [0; 32],
        action: SorafsStreamTokenCustodyActionV1::Configure(
            norito::encode_canonical(&policy).unwrap(),
        ),
    }
    .into();
    assert_eq!(plan.instruction(&config).unwrap(), expected);
    let bytes = encode_bounded(&plan, MAX_PLAN_BYTES).unwrap();
    let decoded: Plan = decode_bounded(&bytes, MAX_PLAN_BYTES).unwrap();
    assert_eq!(encode_bounded(&decoded, MAX_PLAN_BYTES).unwrap(), bytes);
    assert_eq!(
        instructions(
            &config,
            &bytes,
            NativeOperationKind::StreamTokenCustodyConfigure,
            1_999
        )
        .unwrap(),
        vec![expected]
    );
    for deadline in [0, 1_000, 2_001] {
        assert!(
            instructions(
                &config,
                &bytes,
                NativeOperationKind::StreamTokenCustodyConfigure,
                deadline
            )
            .is_err()
        );
    }
    assert!(
        instructions(
            &config,
            &bytes,
            NativeOperationKind::StreamTokenCustodyEnroll,
            1_999
        )
        .is_err()
    );
    assert!(decode_bounded::<Plan>(&bytes[..bytes.len() - 1], MAX_PLAN_BYTES).is_err());
    let mut appended = bytes.clone();
    appended.push(0);
    assert!(decode_bounded::<Plan>(&appended, MAX_PLAN_BYTES).is_err());
    assert!(decode_bounded::<Plan>(&bytes, bytes.len() - 1).is_err());
}

#[test]
fn target_rejects_foreign_provider_network_revision_and_coherent_predecessor_claims() {
    let (config, policy) = fixture();
    let plan = enrolled_request_plan(&config, policy.clone(), false);
    plan.selection.validate(&config).unwrap();
    let changes: &[fn(&mut StreamTokenCustodySelection)] = &[
        |s| s.provider_id = ProviderId::new([11; 32]),
        |s| s.binding.network_id = [12; 32],
        |s| s.expected_revision += 1,
        |s| s.expected_digest[0] ^= 1,
        |s| {
            let record = s.current.as_mut().unwrap();
            record.execution_height = 0;
            s.expected_digest = record.canonical_digest().unwrap();
        },
        |s| {
            let record = s.current.as_mut().unwrap();
            record.request_digest = [0; 32];
            s.expected_digest = record.canonical_digest().unwrap();
        },
        |s| {
            let record = s.current.as_mut().unwrap();
            record.predecessor_digest = [13; 32];
            s.expected_digest = record.canonical_digest().unwrap();
        },
    ];
    for change in changes {
        let mut changed = plan.selection.clone();
        change(&mut changed);
        assert!(changed.validate(&config).is_err());
    }
    let mut absent = absent_selection(&policy);
    absent.expected_digest = [1; 32];
    assert!(absent.validate(&config).is_err());
    let mut absent = absent_selection(&policy);
    absent.expected_revision = 1;
    assert!(absent.validate(&config).is_err());
    let mut oversized = plan.selection.clone();
    oversized
        .current
        .as_mut()
        .unwrap()
        .control_state
        .resize(SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1 + 1, 0);
    assert!(oversized.admit().is_err());
}

#[test]
fn manager_must_remain_independent_of_both_selected_keys() {
    let (config, policy) = fixture();
    configure_plan(policy.clone()).instruction(&config).unwrap();
    for key in [test_key(4), test_key(7)] {
        let mut other = config.clone();
        other.key_pair = key;
        assert!(configure_plan(policy.clone()).instruction(&other).is_err());
    }
    let mut changed = configure_plan(policy);
    changed.selection.binding.key_revision += 1;
    assert!(changed.instruction(&config).is_err());
}

#[test]
fn enrollment_uses_real_signature_and_exact_original_anchor_interval_and_revocation() {
    let (config, policy) = fixture();
    let plan = enrolled_request_plan(&config, policy.clone(), false);
    let Action::Enroll { enrollment, .. } = &plan.action else {
        unreachable!()
    };
    let expected: InstructionBox = MutateSorafsStreamTokenCustody {
        provider_id: plan.selection.provider_id,
        expected_revision: 1,
        expected_digest: plan.selection.expected_digest,
        action: SorafsStreamTokenCustodyActionV1::Enroll(enrollment.clone()),
    }
    .into();
    assert_eq!(plan.instruction(&config).unwrap(), expected);
    for index in 0..5 {
        let mut changed = plan.clone();
        let Action::Enroll {
            anchor,
            anchor_observed_at_unix_ms,
            issued_at_unix_ms,
            expires_at_unix_ms,
            enrollment,
        } = &mut changed.action
        else {
            unreachable!()
        };
        match index {
            0 => anchor.block_hash[0] ^= 1,
            1 => *anchor_observed_at_unix_ms = 1_001,
            2 => *issued_at_unix_ms += 1,
            3 => *expires_at_unix_ms -= 1,
            _ => {
                let mut signed: SignerCustodyRecordV1 =
                    decode_bounded(enrollment, SIGNER_CUSTODY_MAX_BYTES_V1).unwrap();
                signed.attestation[0] ^= 1;
                *enrollment = encode_bounded(&signed, SIGNER_CUSTODY_MAX_BYTES_V1).unwrap();
            }
        }
        assert!(changed.instruction(&config).is_err());
    }
    let mut changed = plan.clone();
    changed.deadline_unix_ms = 2_001;
    assert!(changed.instruction(&config).is_err());
    assert!(
        enrolled_request_plan(&config, policy, true)
            .instruction(&config)
            .is_err(),
        "coherently signed new anchor cannot replace current revocation"
    );
}

#[test]
fn fee_and_enrollment_bounds_are_checked_before_request_cloning() {
    let (_, policy) = fixture();
    let options = BoundedTransactionOptions {
        fee_payment: FeePaymentIntent::authority(Vec::new(), None),
        max_total_fees: BTreeMap::new(),
        deadline: Instant::now() + Duration::from_secs(10),
    };
    validate_options(&options).unwrap();
    let mut request = StreamTokenCustodyConfigureRequest {
        selection: absent_selection(&policy),
        policy,
        deadline_unix_ms: 2_000,
        options,
    };
    let asset: AssetDefinitionId = XOR_ASSET_DEFINITION.parse().unwrap();
    request
        .options
        .max_total_fees
        .insert(asset, Quantity::zero());
    assert!(CustodyExpectation::Configure(&request).plan(1_000).is_err());
    request.options.max_total_fees.clear();
    CustodyExpectation::Configure(&request).plan(1_000).unwrap();
    let mut enroll = StreamTokenCustodyEnrollRequest {
        selection: request.selection,
        anchor: SignerCustodyAnchorV1 {
            height: 10,
            block_hash: [9; 32],
            state_digest: [10; 32],
        },
        anchor_observed_at_unix_ms: 950,
        issued_at_unix_ms: 900,
        expires_at_unix_ms: 2_000,
        enrollment: Vec::new(),
        deadline_unix_ms: 1_900,
        options: request.options,
    };
    assert!(CustodyExpectation::Enroll(&enroll).plan(1_000).is_err());
    enroll.enrollment.resize(SIGNER_CUSTODY_MAX_BYTES_V1 + 1, 0);
    assert!(CustodyExpectation::Enroll(&enroll).plan(1_000).is_err());
}
