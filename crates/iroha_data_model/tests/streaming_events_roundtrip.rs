//! Roundtrip coverage for streaming capability ticket events.

use std::str::FromStr;

use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model::{
    metadata::Metadata,
    prelude::{
        AccountId, DataSpaceId, Decode, DomainEvent, DomainId, Encode, LaneId,
        StreamingPrivacyRelay, StreamingPrivacyRoute, StreamingRouteBinding,
        StreamingSoranetAccessKind, StreamingSoranetRoute, StreamingSoranetStreamTag,
        StreamingTicketCapabilities, StreamingTicketPolicy, StreamingTicketReady,
        StreamingTicketRecord, StreamingTicketRevoked,
    },
    soranet::ticket::{TicketBodyV1, TicketEnvelopeV1, TicketScopeV1},
};
use norito::{
    codec::encode_with_header_flags,
    core::{decode_from_bytes, frame_bare_with_header_flags},
};

fn sample_hash(seed: u8) -> Hash {
    Hash::prehashed([seed; 32])
}

fn seeded_account(domain: &str, seed: u8) -> AccountId {
    let _domain_id = DomainId::from_str(domain).expect("domain id");
    let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
    AccountId::new(key_pair.public_key().clone())
}

fn sample_route(seed: u8) -> StreamingPrivacyRoute {
    StreamingPrivacyRoute::new(
        sample_hash(seed).into(),
        StreamingPrivacyRelay::new(
            sample_hash(seed.wrapping_add(1)).into(),
            "/ip4/127.0.0.1/udp/9101/quic".to_owned(),
            sample_hash(seed.wrapping_add(2)).into(),
            0b001,
        ),
        StreamingPrivacyRelay::new(
            sample_hash(seed.wrapping_add(3)).into(),
            "/ip4/127.0.0.1/udp/9201/quic".to_owned(),
            sample_hash(seed.wrapping_add(4)).into(),
            0b010,
        ),
        vec![0xA1, 0xA2, 0xA3],
        vec![0xB1, 0xB2, 0xB3],
        1_024,
    )
    .with_soranet(StreamingSoranetRoute::new(
        sample_hash(seed.wrapping_add(5)).into(),
        "/dns/exit.torii.example/tcp/8080".to_owned(),
        Some(25),
        StreamingSoranetAccessKind::Authenticated,
        StreamingSoranetStreamTag::NoritoStream,
    ))
}

fn sample_ticket_envelope() -> TicketEnvelopeV1 {
    let body = TicketBodyV1 {
        blinded_cid: [0xAA; 32],
        scope: TicketScopeV1::Read,
        max_uses: 10,
        valid_after: 1_701_000_000,
        valid_until: 1_701_086_400,
        issuer_id: seeded_account("wonderland", 0x01),
        salt_epoch: 42,
        policy_flags: 0,
        metadata: Metadata::default(),
    };
    let commitment = body.compute_commitment();
    TicketEnvelopeV1 {
        body,
        commitment,
        zk_proof: Vec::new(),
        signature: Signature::from_bytes(&[0u8; 64]),
        nullifier: [0xBB; 32],
    }
}

fn sample_ticket_record() -> StreamingTicketRecord {
    StreamingTicketRecord::new(
        sample_hash(0x02),
        seeded_account("streaming", 0x02),
        DataSpaceId::new(7),
        LaneId::new(5),
        2_048,
        21_000,
        24_000,
        120_000,
        64,
        12,
        sample_hash(0x33),
        42,
        [0x66; 64],
        sample_hash(0x44),
        sample_hash(0x55),
        [0x77; 32],
        1_701_234_567,
        1_701_834_567,
        Some(StreamingTicketPolicy::new(
            4,
            vec!["us".into(), "jp".into()],
            Some(15_000),
        )),
        StreamingTicketCapabilities::from_bits(
            StreamingTicketCapabilities::LIVE | StreamingTicketCapabilities::HDR,
        ),
    )
}

#[test]
fn ticket_record_roundtrip() {
    let record = sample_ticket_record();
    let (payload, flags) = encode_with_header_flags(&record);
    let framed = frame_bare_with_header_flags::<StreamingTicketRecord>(&payload, flags)
        .expect("frame ticket record payload");
    let decoded =
        decode_from_bytes::<StreamingTicketRecord>(&framed).expect("decode framed ticket record");
    assert_eq!(record, decoded);
}

#[test]
fn privacy_route_with_ticket_roundtrip() {
    let envelope = sample_ticket_envelope();
    let route = sample_route(0x21).with_ticket(envelope.clone());
    let bytes = route.encode();
    let decoded =
        StreamingPrivacyRoute::decode(&mut bytes.as_slice()).expect("decode route with ticket");
    let decoded_ticket = decoded.ticket_envelope().expect("ticket envelope present");
    assert_eq!(decoded_ticket, &envelope);
}

#[test]
fn ticket_ready_roundtrip() {
    let ticket_envelope = sample_ticket_envelope();
    let route = sample_route(0x11).with_ticket(ticket_envelope.clone());
    assert!(route.ticket_envelope().is_some());
    let binding = StreamingRouteBinding::new(route.clone(), 10, 42, true);
    let domain_id = DomainId::from_str("nsc").expect("domain id");
    let ticket = sample_ticket_record();
    let event = DomainEvent::StreamingTicketReady(StreamingTicketReady::new(
        domain_id.clone(),
        sample_hash(0x01),
        ticket,
        vec![binding],
    ));

    let bytes = event.encode();
    let result = std::panic::catch_unwind(|| DomainEvent::decode(&mut bytes.as_slice()));
    if let Err(panic) = result {
        eprintln!("decode panic: {panic:?}");
        panic!("DomainEvent::decode panicked");
    }
    let decoded = result.unwrap().expect("decode ticket ready");
    assert_eq!(event, decoded);
}

#[test]
fn ticket_revoked_roundtrip() {
    let event = DomainEvent::StreamingTicketRevoked(StreamingTicketRevoked::new(
        DomainId::from_str("nsc").expect("domain id"),
        sample_hash(0x10),
        sample_hash(0x20),
        sample_hash(0x21),
        17,
        [0xCC; 64],
    ));

    let bytes = event.encode();
    let decoded = DomainEvent::decode(&mut bytes.as_slice()).expect("decode ticket revoked");
    assert_eq!(event, decoded);
}

#[test]
fn soranet_route_mutators() {
    let mut route = StreamingSoranetRoute::new(
        sample_hash(0x31).into(),
        "/dns/exit.soranet/quic".to_owned(),
        Some(30),
        StreamingSoranetAccessKind::ReadOnly,
        StreamingSoranetStreamTag::NoritoStream,
    );
    assert_eq!(route.padding_budget_ms(), Some(30));
    route.set_padding_budget_ms(None);
    assert_eq!(route.padding_budget_ms(), None);

    route.set_exit_multiaddr("/dns/exit.updated/quic".to_owned());
    assert_eq!(route.exit_multiaddr(), "/dns/exit.updated/quic");

    assert_eq!(*route.access_kind(), StreamingSoranetAccessKind::ReadOnly);
    route.set_access_kind(StreamingSoranetAccessKind::Authenticated);
    assert_eq!(
        *route.access_kind(),
        StreamingSoranetAccessKind::Authenticated
    );
}

#[test]
fn privacy_route_soranet_and_ticket_setters() {
    let mut route = StreamingPrivacyRoute::new(
        sample_hash(0x41).into(),
        StreamingPrivacyRelay::new(
            sample_hash(0x42).into(),
            "/ip4/10.0.0.1/udp/9101/quic".to_owned(),
            sample_hash(0x43).into(),
            0b011,
        ),
        StreamingPrivacyRelay::new(
            sample_hash(0x44).into(),
            "/ip4/10.0.0.2/udp/9201/quic".to_owned(),
            sample_hash(0x45).into(),
            0b101,
        ),
        vec![0xC1, 0xC2],
        vec![0xD1, 0xD2],
        512,
    );

    assert!(route.soranet().is_none());
    let soranet = StreamingSoranetRoute::new(
        sample_hash(0x46).into(),
        "/dns/soranet.entry/quic".to_owned(),
        Some(12),
        StreamingSoranetAccessKind::Authenticated,
        StreamingSoranetStreamTag::NoritoStream,
    );
    route.set_soranet(Some(soranet.clone()));
    assert_eq!(route.soranet(), Some(&soranet));

    assert!(route.ticket_envelope().is_none());
    let envelope = sample_ticket_envelope();
    route.set_ticket(Some(envelope.clone()));
    assert_eq!(route.ticket_envelope(), Some(&envelope));

    route.set_soranet(None);
    assert!(route.soranet().is_none());
    route.set_ticket(None);
    assert!(route.ticket_envelope().is_none());
}
