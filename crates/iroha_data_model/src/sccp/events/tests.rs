//! Unit tests for the SCCP v1 data events.

use super::*;
use crate::sccp::{
    light_client::{SccpLcEquivocationFreezeV1, SccpLcParliamentFreezeV1},
    test_support::{assert_rejects_unknown_field, roundtrip},
};
use iroha_crypto::{Algorithm, KeyPair};

const ETH: SccpNetworkV1 = SccpNetworkV1::EthereumMainnet;
const TON: SccpNetworkV1 = SccpNetworkV1::TonMainnet;

fn peer(seed: u8) -> PeerId {
    let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("deterministic Ed25519 seed");
    PeerId::new(key_pair.public_key().clone())
}

fn account(seed: u8) -> AccountId {
    let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("deterministic Ed25519 seed");
    AccountId::new(key_pair.public_key().clone())
}

fn point() -> SccpLcPointV1 {
    SccpLcPointV1 {
        source_height: 21_000_000,
        block_hash: [0x21; 32],
        source_time_ms: 1_758_000_000_000,
    }
}

/// One event of every variant, in declaration order, with its JSON tag.
#[allow(clippy::too_many_lines)]
fn every_event() -> Vec<(SccpEvent, &'static str)> {
    vec![
        (
            SccpEvent::MessageRecorded(SccpMessageRecordedV1 {
                message_id: [1; 32],
                network: ETH,
                revision: 1,
                nonce: 0,
                height: 10,
                commitment_index: 0,
                deadline_ms: 1_758_604_800_000,
            }),
            "message_recorded",
        ),
        (
            SccpEvent::BlockCommitted(SccpBlockCommittedV1 {
                height: 10,
                root: [2; 32],
                count: 3,
                history_size: 4,
            }),
            "block_committed",
        ),
        (
            SccpEvent::SubjectCreated(SccpSubjectCreatedV1 {
                height: 10,
                generation: 2,
                message_count: 3,
                rotation: true,
            }),
            "subject_created",
        ),
        (
            SccpEvent::AttestationSigned(SccpAttestationSignedV1 {
                height: 10,
                generation: 2,
                signer_index: 30,
                address: [3; 20],
            }),
            "attestation_signed",
        ),
        (
            SccpEvent::BlockAttested(SccpBlockAttestedV1 {
                height: 10,
                generation: 2,
                signer_bitmap: 0b1011,
            }),
            "block_attested",
        ),
        (
            SccpEvent::RosterGenerationCreated(SccpRosterGenerationCreatedV1 {
                generation: 3,
                digest: [4; 32],
                activation_height: 11,
                valid_until_ms: 1_759_209_600_000,
            }),
            "roster_generation_created",
        ),
        (
            SccpEvent::RosterDerivationFailed(SccpRosterDerivationFailedV1 {
                height: 3_600,
                generation: 2,
                reason: SccpRosterDerivationFailureV1::RosterSizeOutOfRange,
                roster_size: 32,
            }),
            "roster_derivation_failed",
        ),
        (
            SccpEvent::HandoffStalled(SccpHandoffStalledV1 {
                height: 3_600,
                generation: 2,
            }),
            "handoff_stalled",
        ),
        (
            SccpEvent::BridgeKeySet(SccpBridgeKeySetV1 {
                peer: peer(1),
                address: Some([5; 20]),
                account: Some(account(5)),
                activation_epoch: 4,
            }),
            "bridge_key_set",
        ),
        (
            SccpEvent::AttestationFault(SccpAttestationFaultV1 {
                peer: peer(2),
                address: [6; 20],
                height: 9,
                statement_hash: [7; 32],
                reported_at_height: 12,
            }),
            "attestation_fault",
        ),
        (
            SccpEvent::InboundProven(SccpInboundProvenV1 {
                message_id: [8; 32],
                network: TON,
                revision: 1,
                amount: u128::MAX,
                source_locator: SccpSourceLocatorV1 {
                    source_height: 50_000_000,
                    block_hash: [9; 32],
                    index_in_block: 2,
                },
                fee_due: 10_000_000,
            }),
            "inbound_proven",
        ),
        (
            SccpEvent::RecipientRegistered(SccpRecipientRegisteredV1 {
                account: account(3),
                network: TON,
                message_id: Some([8; 32]),
            }),
            "recipient_registered",
        ),
        (
            SccpEvent::InboundReleased(SccpInboundReleasedV1 {
                message_id: [8; 32],
                network: TON,
                revision: 1,
                recipient: account(3),
                amount: 5_000_000_000,
                fee: 10_000_000,
            }),
            "inbound_released",
        ),
        (
            SccpEvent::InboundBounced(SccpInboundBouncedV1 {
                message_id: [10; 32],
                network: ETH,
                revision: 1,
                bounce_message_id: [11; 32],
                bounce_revision: 2,
                amount: 7,
                reason: SccpBounceReasonV1::EscrowRecipient,
            }),
            "inbound_bounced",
        ),
        (
            SccpEvent::InboundLiabilityShortfall(SccpInboundLiabilityShortfallV1 {
                message_id: [12; 32],
                network: ETH,
                revision: 1,
                amount: 9,
                liability: 8,
            }),
            "inbound_liability_shortfall",
        ),
        (
            SccpEvent::OutboundVoided(SccpOutboundVoidedV1 {
                message_id: [13; 32],
                network: ETH,
                revision: 1,
                nonce: 4,
                kind: SccpVoidKindV1::Frozen,
                refund_pending: true,
            }),
            "outbound_voided",
        ),
        (
            SccpEvent::OutboundRefunded(SccpOutboundRefundedV1 {
                message_id: [13; 32],
                network: ETH,
                revision: 1,
                nonce: 4,
                recipient: account(4),
                amount: 1_000_000_000,
            }),
            "outbound_refunded",
        ),
        (
            SccpEvent::OutboundStranded(SccpOutboundStrandedV1 {
                message_id: [14; 32],
                network: ETH,
                revision: 1,
                nonce: 5,
                amount: 1_000_000_000,
            }),
            "outbound_stranded",
        ),
        (
            SccpEvent::StrandedReleased(SccpStrandedReleasedV1 {
                network: ETH,
                recipient: account(6),
                amount: 1_000_000_000,
                memo: "return bounced value".to_owned(),
                proposal_id: [15; 32],
            }),
            "stranded_released",
        ),
        (
            SccpEvent::LightClientAdvanced(SccpLightClientAdvancedV1 {
                network: ETH,
                latest_set_id: 1_400,
                latest_finalized: point(),
                state_hash: [16; 32],
            }),
            "light_client_advanced",
        ),
        (
            SccpEvent::LightClientFrozen(SccpLightClientFrozenV1 {
                network: ETH,
                reason: SccpLcFreezeReasonV1::Equivocation(SccpLcEquivocationFreezeV1 {
                    evidence_hash: [17; 32],
                }),
            }),
            "light_client_frozen",
        ),
        (
            SccpEvent::LightClientInitialized(SccpLightClientInitializedV1 {
                network: ETH,
                latest_set_id: 1_399,
                latest_finalized: point(),
                state_hash: [18; 32],
                proposal_id: [19; 32],
            }),
            "light_client_initialized",
        ),
        (
            SccpEvent::TrustedCheckpointInstalled(SccpTrustedCheckpointInstalledV1 {
                network: ETH,
                source_height: 20_000_000,
                block_hash: [20; 32],
                proposal_id: [21; 32],
            }),
            "trusted_checkpoint_installed",
        ),
        (
            SccpEvent::RevisionActivationChanged(SccpRevisionActivationChangedV1 {
                network: ETH,
                revision: 1,
                from: Some(SccpRouteActivationV1::Staged),
                to: Some(SccpRouteActivationV1::Bidirectional),
            }),
            "revision_activation_changed",
        ),
        (
            SccpEvent::ControlRecorded(SccpControlRecordedV1 {
                network: ETH,
                revision: 1,
                control_nonce: 1,
                paused: true,
                height: 30,
                commitment_index: 0,
            }),
            "control_recorded",
        ),
        (
            SccpEvent::GovernanceEnacted(SccpGovernanceEnactedV1 {
                proposal_id: [22; 32],
                subjects: vec![
                    SccpGovernanceSubjectV1::Route(ETH),
                    SccpGovernanceSubjectV1::RouteControl(ETH),
                    SccpGovernanceSubjectV1::BridgeKeyFault(peer(3)),
                ],
            }),
            "governance_enacted",
        ),
    ]
}

#[test]
fn every_event_roundtrips_with_a_snake_case_tag() {
    let events = every_event();
    assert_eq!(events.len(), 26, "§4.17 lists 26 events");
    for (event, tag) in &events {
        roundtrip(event);
        let value = norito::json::to_value(event).expect("value");
        assert_eq!(
            value.get("event").and_then(norito::json::Value::as_str),
            Some(*tag)
        );
        assert_rejects_unknown_field(event, &[]);
        assert_rejects_unknown_field(event, &["payload"]);
    }
}

#[test]
fn codec_indices_follow_declaration_order() {
    for (index, (event, _)) in every_event().into_iter().enumerate() {
        let tag = u32::try_from(index).expect("small").encode();
        assert!(event.encode().starts_with(&tag), "{event:?}");
    }
}

#[test]
fn event_set_matches_exactly_its_variant() {
    let events = every_event();
    let all = SccpEventSet::all();
    for (index, (event, _)) in events.iter().enumerate() {
        assert!(all.matches(event));
        assert!(!SccpEventSet::empty().matches(event));
        let matching: Vec<usize> = events
            .iter()
            .enumerate()
            .filter(|(_, (candidate, _))| {
                std::mem::discriminant(candidate) == std::mem::discriminant(event)
            })
            .map(|(position, _)| position)
            .collect();
        assert_eq!(matching, vec![index]);
    }
    let set = SccpEventSet::MessageRecorded | SccpEventSet::GovernanceEnacted;
    let matched: Vec<&str> = events
        .iter()
        .filter(|(event, _)| set.matches(event))
        .map(|(_, tag)| *tag)
        .collect();
    assert_eq!(matched, vec!["message_recorded", "governance_enacted"]);
}

#[test]
fn optional_payload_fields_are_explicit_and_cover_none() {
    let revocation = SccpEvent::BridgeKeySet(SccpBridgeKeySetV1 {
        peer: peer(9),
        address: None,
        account: None,
        activation_epoch: 8,
    });
    roundtrip(&revocation);
    let removal = SccpEvent::RevisionActivationChanged(SccpRevisionActivationChangedV1 {
        network: TON,
        revision: 3,
        from: Some(SccpRouteActivationV1::Staged),
        to: None,
    });
    roundtrip(&removal);
    let registration = SccpRevisionActivationChangedV1 {
        network: TON,
        revision: 1,
        from: None,
        to: Some(SccpRouteActivationV1::Staged),
    };
    roundtrip(&registration);
    let mut value = norito::json::to_value(&registration).expect("value");
    value.as_object_mut().expect("object").remove("from");
    let json = norito::json::to_json(&value).expect("json");
    assert!(norito::json::from_json::<SccpRevisionActivationChangedV1>(&json).is_err());
    roundtrip(&SccpRecipientRegisteredV1 {
        account: account(1),
        network: ETH,
        message_id: None,
    });
}

#[test]
fn enum_payloads_roundtrip_in_every_variant() {
    for reason in [
        SccpRosterDerivationFailureV1::MissingNextEpochSnapshot,
        SccpRosterDerivationFailureV1::RosterSizeOutOfRange,
    ] {
        roundtrip(&reason);
    }
    for reason in [
        SccpBounceReasonV1::UndecodableRecipient,
        SccpBounceReasonV1::EscrowRecipient,
        SccpBounceReasonV1::InadmissibleController,
        SccpBounceReasonV1::CreditRefused,
    ] {
        roundtrip(&reason);
    }
    roundtrip(&SccpEvent::LightClientFrozen(SccpLightClientFrozenV1 {
        network: TON,
        reason: SccpLcFreezeReasonV1::Parliament(SccpLcParliamentFreezeV1 {
            proposal_id: [1; 32],
        }),
    }));
}

#[test]
fn amounts_are_decimal_strings() {
    let (event, _) = every_event().swap_remove(10);
    let json = norito::json::to_json(&event).expect("json");
    assert!(
        json.contains(&format!("\"amount\":\"{}\"", u128::MAX)),
        "{json}"
    );
    assert!(json.contains("\"fee_due\":\"10000000\""), "{json}");
}
