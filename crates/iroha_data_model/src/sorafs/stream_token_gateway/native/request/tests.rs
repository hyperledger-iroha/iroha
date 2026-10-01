//! Native gateway mutation shape, identity, bounded maintenance and canonical framing.

use super::*;
use crate::{
    account::AccountId,
    sorafs::{
        capacity::ProviderId,
        reputation::{
            StreamTokenExcludedKindV1, StreamTokenRequestRouteV1, StreamTokenValidationBindingV1,
            StreamTokenValidationOutcomeV1, StreamTokenValidationRequestContextV1,
            derive_stream_token_gateway_id_v1,
        },
        stream_token_gateway::{
            StreamTokenGatewayAdmissionQualificationV1, StreamTokenGatewayQuotaRequestV1,
            stream_token_gateway_lease_expiry_unix_ms_v1,
        },
    },
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use std::collections::BTreeSet;

fn policy() -> StreamTokenGatewayPolicyV1 {
    let network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"gateway-request-network",
    )));
    let account = |seed| {
        AccountId::new(
            KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    };
    let compliance_gateway_id = "gateway.request-test".to_owned();
    let mut policy = StreamTokenGatewayPolicyV1 {
        network_id,
        qualification: StreamTokenGatewayAdmissionQualificationV1 {
            gateway_id: derive_stream_token_gateway_id_v1(&network_id, &compliance_gateway_id)
                .unwrap(),
            revision: 1,
            policy_digest: [1; 32],
            max_pending: 64,
            max_tracked_tokens: 32,
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
    policy.qualification.policy_digest = policy.calculate_policy_digest().unwrap();
    policy
}
fn envelope(action: StreamTokenGatewayActionV1) -> StreamTokenGatewayRequestV1 {
    let policy = policy();
    StreamTokenGatewayRequestV1 {
        network_id: policy.network_id,
        gateway_id: policy.qualification.gateway_id,
        expected_policy_revision: policy.qualification.revision,
        expected_policy_digest: policy.qualification.policy_digest,
        action,
    }
}
fn admission() -> StreamTokenGatewayAdmissionRequestV1 {
    StreamTokenGatewayAdmissionRequestV1 {
        serving_attempt_id: [0x11; 32],
        context: StreamTokenValidationRequestContextV1::try_new(
            ProviderId::new([0x21; 32]),
            [0x22; 32],
            sorafs_manifest::canonical_manifest_root_cid([0x23; 32]),
            "sorafs.sf1@1.0.0".to_owned(),
            "gateway-request-test",
            Some(b"dG9rZW4="),
            StreamTokenRequestRouteV1::car_range(0, 63).unwrap(),
        )
        .unwrap(),
        token_body_digest: Some([0x24; 32]),
        token_key_version: Some(1),
        validated_at_unix_ms: 1_800_000_000_000,
        status: StreamTokenValidationStatusV1::Accepted,
        quota: Some(StreamTokenGatewayQuotaRequestV1 {
            token_id: "31".repeat(16),
            max_streams: 2,
            requests_per_minute: 60,
            rate_limit_bytes: 1_048_576,
            requested_bytes: 64,
            expires_at_epoch: 1_800_000_600,
            observed_at_epoch: 1_800_000_000,
        }),
    }
}
fn record() -> StreamTokenGatewayAdmissionRecordV1 {
    let request = admission();
    let qualification = policy().qualification;
    let expires = request.quota.as_ref().unwrap().expires_at_epoch;
    StreamTokenGatewayAdmissionRecordV1 {
        serving_attempt_id: request.serving_attempt_id,
        admitted_under: qualification,
        provider_id: request.context.provider_id(),
        outcome: StreamTokenValidationOutcomeV1 {
            binding: StreamTokenValidationBindingV1 {
                gateway_id: qualification.gateway_id,
                gateway_sequence: 1,
                request_context_digest: request.context.digest().unwrap(),
            },
            token_body_digest: request.token_body_digest,
            token_key_version: request.token_key_version,
            validated_at_unix_ms: request.validated_at_unix_ms,
            status: request.status,
        },
        retry_after_secs: None,
        lease_id: Some([0x41; 32]),
        lease_expires_at_unix_ms: Some(
            stream_token_gateway_lease_expiry_unix_ms_v1(
                request.validated_at_unix_ms,
                expires,
                qualification.lease_ttl_ms,
            )
            .unwrap(),
        ),
        lease_token_expires_at_epoch: Some(expires),
    }
}
fn initial() -> StreamTokenGatewayRequestV1 {
    let mut request = envelope(StreamTokenGatewayActionV1::Configure(policy()));
    request.expected_policy_revision = 0;
    request.expected_policy_digest = [0; 32];
    request
}

#[test]
fn configure_requires_zero_bootstrap_or_adjacent_current_policy_cas() {
    let first = initial();
    first.validate().unwrap();
    let changes: [fn(&mut StreamTokenGatewayRequestV1); 4] = [
        |r: &mut StreamTokenGatewayRequestV1| r.expected_policy_revision = 1,
        |r: &mut StreamTokenGatewayRequestV1| r.expected_policy_digest = [1; 32],
        |r: &mut StreamTokenGatewayRequestV1| r.gateway_id = [2; 32],
        |r: &mut StreamTokenGatewayRequestV1| {
            r.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"foreign-network",
            )))
        },
    ];
    for change in changes {
        let mut wrong = first.clone();
        change(&mut wrong);
        assert!(wrong.validate().is_err());
    }
    let mut replacement = policy();
    replacement.qualification.revision += 1;
    replacement.qualification.policy_digest = replacement.calculate_policy_digest().unwrap();
    let valid = envelope(StreamTokenGatewayActionV1::Configure(replacement));
    valid.validate().unwrap();
    for revision in [0, 2, u64::MAX] {
        let mut wrong = valid.clone();
        wrong.expected_policy_revision = revision;
        assert!(wrong.validate().is_err());
    }
}

#[test]
fn request_bounds_complete_frame_before_nested_policy_validation() {
    let mut request = initial();
    let StreamTokenGatewayActionV1::Configure(policy) = &mut request.action else {
        unreachable!()
    };
    policy.compliance_gateway_id = "x".repeat(STREAM_TOKEN_GATEWAY_MAX_REQUEST_BYTES_V1);
    assert_eq!(request.validate(), Err(Error::InvalidRequest));
}

#[test]
fn maintenance_requires_live_cas_and_exact_nonzero_bounded_prefix() {
    for max_items in [1, STREAM_TOKEN_GATEWAY_MAX_EXPIRY_ITEMS_V1] {
        envelope(StreamTokenGatewayActionV1::Expire { max_items })
            .validate()
            .unwrap();
    }
    for max_items in [0, STREAM_TOKEN_GATEWAY_MAX_EXPIRY_ITEMS_V1 + 1, u32::MAX] {
        assert!(
            envelope(StreamTokenGatewayActionV1::Expire { max_items })
                .validate()
                .is_err()
        );
    }
    let valid = envelope(StreamTokenGatewayActionV1::Expire { max_items: 1 });
    for (revision, digest) in [(0, [0; 32]), (0, [1; 32]), (1, [0; 32])] {
        let mut wrong = valid.clone();
        wrong.expected_policy_revision = revision;
        wrong.expected_policy_digest = digest;
        assert!(wrong.validate().is_err());
    }
    let mut inert = valid;
    inert.gateway_id = [0; 32];
    assert!(inert.validate().is_err());
}

#[test]
fn retained_records_bind_gateway_and_original_policy_without_reinterpreting_rotation() {
    for action in [
        StreamTokenGatewayActionV1::Acknowledge(record()),
        StreamTokenGatewayActionV1::ReleaseLease(record()),
    ] {
        let current = envelope(action);
        current.validate().unwrap();
        let mut rotated = current.clone();
        rotated.expected_policy_revision += 1;
        rotated.expected_policy_digest = [0x71; 32];
        rotated
            .validate()
            .expect("original record survives a newer current policy");
        let mut substituted = current.clone();
        substituted.expected_policy_digest = [0x72; 32];
        assert!(substituted.validate().is_err());
        let mut rebound = current;
        rebound.gateway_id = [0x73; 32];
        assert!(rebound.validate().is_err());
    }
    let mut excluded = record();
    excluded.outcome.status =
        StreamTokenValidationStatusV1::Excluded(StreamTokenExcludedKindV1::MissingToken);
    excluded.outcome.token_body_digest = None;
    excluded.outcome.token_key_version = None;
    excluded.lease_id = None;
    excluded.lease_expires_at_unix_ms = None;
    excluded.lease_token_expires_at_epoch = None;
    envelope(StreamTokenGatewayActionV1::Acknowledge(excluded))
        .validate()
        .unwrap();
    assert!(
        envelope(StreamTokenGatewayActionV1::ReleaseLease(excluded))
            .validate()
            .is_err()
    );
}

#[test]
fn admission_preserves_serving_identity_and_diagnostic_expiry_semantics() {
    let valid = envelope(StreamTokenGatewayActionV1::Admit(admission()));
    valid.validate().unwrap();
    let mut zero_attempt = valid.clone();
    let StreamTokenGatewayActionV1::Admit(request) = &mut zero_attempt.action else {
        unreachable!()
    };
    request.serving_attempt_id = [0; 32];
    assert!(zero_attempt.validate().is_err());
    let mut diagnostic = valid;
    let StreamTokenGatewayActionV1::Admit(request) = &mut diagnostic.action else {
        unreachable!()
    };
    request.status =
        StreamTokenValidationStatusV1::Excluded(StreamTokenExcludedKindV1::InvalidSignature);
    request.quota.as_mut().unwrap().expires_at_epoch += 86_400;
    diagnostic
        .validate()
        .expect("native shape preserves authenticatable diagnostic claims");
}

#[test]
fn all_native_actions_have_strict_current_owner_norito_and_json_roundtrips() {
    let requests = [
        initial(),
        envelope(StreamTokenGatewayActionV1::CancelReputationDelivery {
            record: record(), expected_recorder_policy_digest: [0x71; 32],
            reason: crate::sorafs::reputation::stream_token_delivery::StreamTokenReputationCancellationReasonV1::CredentialUnavailable,
        }),
        envelope(StreamTokenGatewayActionV1::Admit(admission())),
        envelope(StreamTokenGatewayActionV1::Acknowledge(record())),
        envelope(StreamTokenGatewayActionV1::ReleaseLease(record())),
        envelope(StreamTokenGatewayActionV1::Expire { max_items: 256 }),
    ];
    for request in requests {
        request.validate().unwrap();
        let frame = norito::encode_canonical(&request).unwrap();
        assert_eq!(
            frame[6..22],
            norito::schema::identity::frame_hash::<StreamTokenGatewayRequestV1>()
        );
        let mut foreign_owner = frame.clone();
        foreign_owner[6] ^= 1;
        assert!(matches!(
            norito::decode_canonical::<StreamTokenGatewayRequestV1>(&foreign_owner),
            Err(norito::Error::SchemaMismatch)
        ));
        assert_eq!(
            norito::decode_canonical::<StreamTokenGatewayRequestV1>(&frame).unwrap(),
            request
        );
        assert!(
            norito::decode_canonical::<StreamTokenGatewayRequestV1>(&frame[..frame.len() - 1])
                .is_err()
        );
        let mut trailing = frame;
        trailing.push(0);
        assert!(norito::decode_canonical::<StreamTokenGatewayRequestV1>(&trailing).is_err());
        let json = norito::json::to_json(&request).unwrap();
        assert_eq!(
            norito::json::from_str::<StreamTokenGatewayRequestV1>(&json).unwrap(),
            request
        );
        let extra = json.replacen('{', "{\"unexpected\":true,", 1);
        assert!(norito::json::from_str::<StreamTokenGatewayRequestV1>(&extra).is_err());
        let action_json = norito::json::to_json(&request.action).unwrap();
        assert_eq!(
            norito::json::from_str::<StreamTokenGatewayActionV1>(&action_json).unwrap(),
            request.action
        );
        let extra_action = action_json.replacen('{', "{\"unexpected\":true,", 1);
        assert!(norito::json::from_str::<StreamTokenGatewayActionV1>(&extra_action).is_err());
    }
    let schema = StreamTokenGatewayRequestV1::schema();
    assert!(schema.contains_key::<StreamTokenGatewayActionV1>());
    assert!(schema.contains_key::<StreamTokenGatewayPolicyV1>());
    assert!(schema.contains_key::<StreamTokenGatewayAdmissionRequestV1>());
    assert!(schema.contains_key::<StreamTokenGatewayAdmissionRecordV1>());
}

mod check;

#[test]
fn reputation_cancellation_binds_counted_original_and_both_current_policy_scopes() {
    use crate::sorafs::reputation::stream_token_delivery::StreamTokenReputationCancellationReasonV1 as Reason;
    let original = envelope(StreamTokenGatewayActionV1::CancelReputationDelivery {
        record: record(),
        expected_recorder_policy_digest: [0x71; 32],
        reason: Reason::AuthorityWithdrawn,
    });
    original.validate().unwrap();
    for change in 0..4 {
        let mut value = original.clone();
        let StreamTokenGatewayActionV1::CancelReputationDelivery {
            record,
            expected_recorder_policy_digest,
            ..
        } = &mut value.action
        else {
            unreachable!()
        };
        match change {
            0 => *expected_recorder_policy_digest = [0; 32],
            1 => record.admitted_under.gateway_id = [0x72; 32],
            2 => record.admitted_under.revision += 1,
            3 => record.admitted_under.policy_digest = [0x73; 32],
            _ => unreachable!(),
        }
        assert!(value.validate().is_err());
    }
    let mut excluded = record();
    excluded.outcome.status =
        StreamTokenValidationStatusV1::Excluded(StreamTokenExcludedKindV1::InvalidSignature);
    excluded.lease_id = None;
    excluded.lease_expires_at_unix_ms = None;
    excluded.lease_token_expires_at_epoch = None;
    excluded.validate_shape(excluded.admitted_under).unwrap();
    assert!(
        envelope(StreamTokenGatewayActionV1::CancelReputationDelivery {
            record: excluded,
            expected_recorder_policy_digest: [0x71; 32],
            reason: Reason::CredentialUnavailable,
        })
        .validate()
        .is_err()
    );
}
