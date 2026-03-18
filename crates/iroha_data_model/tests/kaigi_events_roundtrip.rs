//! Roundtrip coverage for Kaigi domain event summaries.

use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::{
    events::data::prelude::KaigiRelayRegistrationSummary,
    prelude::{
        AccountId, Decode, DomainEvent, DomainId, Encode, KaigiId, KaigiParticipantCommitment,
        KaigiPrivacyMode, KaigiRelayHealthStatus, KaigiRelayHealthSummary,
        KaigiRelayManifestSummary, KaigiRosterSummary, KaigiUsageSummary, Name,
    },
};

fn sample_domain_id() -> DomainId {
    "kaigi_domain".parse().expect("domain id")
}

fn sample_call_id() -> KaigiId {
    KaigiId::new(
        sample_domain_id(),
        "daily-standup".parse::<Name>().expect("call name"),
    )
}

#[test]
fn roster_summary_roundtrips_via_norito() {
    let summary = DomainEvent::KaigiRosterSummary(KaigiRosterSummary::new(
        sample_call_id(),
        KaigiPrivacyMode::ZkRosterV1,
        0,
        3,
        2,
        Some(Hash::prehashed([0x55; 32])),
    ));

    let bytes = summary.encode();
    let decoded = DomainEvent::decode(&mut bytes.as_slice()).expect("decode roster summary");
    assert_eq!(summary, decoded);
}

#[test]
fn relay_manifest_summary_roundtrips_via_norito() {
    let summary = DomainEvent::KaigiRelayManifestUpdated(KaigiRelayManifestSummary::new(
        sample_call_id(),
        5,
        123_456,
    ));

    let bytes = summary.encode();
    let decoded =
        DomainEvent::decode(&mut bytes.as_slice()).expect("decode relay manifest summary");
    assert_eq!(summary, decoded);
}

#[test]
fn usage_summary_roundtrips_via_norito() {
    let summary =
        DomainEvent::KaigiUsageSummary(KaigiUsageSummary::new(sample_call_id(), 42_000, 1234, 7));

    let bytes = summary.encode();
    let decoded = DomainEvent::decode(&mut bytes.as_slice()).expect("decode usage summary");
    assert_eq!(summary, decoded);
}

#[test]
fn participant_commitment_roundtrip_preserves_payload() {
    let commitment = KaigiParticipantCommitment {
        commitment: Hash::prehashed([0xAA; 32]),
        alias_tag: Some("speaker".to_owned()),
    };

    let bytes = commitment.encode();
    let decoded =
        KaigiParticipantCommitment::decode(&mut bytes.as_slice()).expect("decode commitment");
    assert_eq!(commitment, decoded);
}

#[test]
fn relay_registration_summary_roundtrips_via_norito() {
    let domain_id = sample_domain_id();
    let relay_id = AccountId::new(KeyPair::random().public_key().clone());
    let summary = DomainEvent::KaigiRelayRegistered(KaigiRelayRegistrationSummary::new(
        domain_id,
        relay_id.clone(),
        9,
        Hash::prehashed([0xAB; 32]),
    ));

    let bytes = summary.encode();
    let decoded =
        DomainEvent::decode(&mut bytes.as_slice()).expect("decode relay registration summary");
    assert_eq!(summary, decoded);
    if let DomainEvent::KaigiRelayRegistered(decoded_summary) = decoded {
        assert_eq!(decoded_summary.relay(), &relay_id);
        assert_eq!(*decoded_summary.bandwidth_class(), 9);
    } else {
        panic!("unexpected domain event variant");
    }
}

#[test]
fn relay_health_summary_roundtrips_via_norito() {
    let call = sample_call_id();
    let relay = AccountId::new(KeyPair::random().public_key().clone());
    let summary = DomainEvent::KaigiRelayHealthUpdated(KaigiRelayHealthSummary::new(
        call.clone(),
        relay.clone(),
        KaigiRelayHealthStatus::Unavailable,
        123_456,
    ));

    let bytes = summary.encode();
    let decoded = DomainEvent::decode(&mut bytes.as_slice()).expect("decode relay health summary");
    assert_eq!(summary, decoded);
    if let DomainEvent::KaigiRelayHealthUpdated(decoded_summary) = decoded {
        assert_eq!(&decoded_summary.call, &call);
        assert_eq!(&decoded_summary.relay, &relay);
        assert_eq!(decoded_summary.status, KaigiRelayHealthStatus::Unavailable);
    } else {
        panic!("unexpected domain event variant");
    }
}
