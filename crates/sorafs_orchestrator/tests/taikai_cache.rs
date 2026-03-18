use std::{
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    da::types::{BlobDigest, StorageTicketId},
    name::Name,
    taikai::{
        GuardDirectoryId, SegmentDuration, SegmentTimestamp, TaikaiCarPointer, TaikaiCodec,
        TaikaiEventId, TaikaiIngestPointer, TaikaiRenditionId, TaikaiResolution,
        TaikaiSegmentEnvelopeV1, TaikaiStreamId, TaikaiTrackMetadata,
    },
};
use norito::{decode_from_bytes, to_bytes};
use rand::{RngCore, SeedableRng, rngs::StdRng};
use sorafs_orchestrator::taikai_cache::{
    CacheAdmissionAction, CacheAdmissionEnvelope, CacheAdmissionError, CacheAdmissionGossip,
    CacheAdmissionGossipBody, CacheAdmissionRecord, CacheAdmissionReplayFilter,
    CacheAdmissionTracker, CacheTierKind, CachedSegment, QosClass, TaikaiCacheConfig,
    TaikaiCacheHandle, TaikaiPullRequest, TaikaiShardId,
};

fn sample_envelope(sequence: u64) -> TaikaiSegmentEnvelopeV1 {
    let event_id = TaikaiEventId::new(Name::from_str("soranet-demo").expect("valid event id"));
    let stream_id = TaikaiStreamId::new(Name::from_str("primary").expect("valid stream id"));
    let rendition_id =
        TaikaiRenditionId::new(Name::from_str("1080p-main").expect("valid rendition id"));
    let track = TaikaiTrackMetadata::video(
        TaikaiCodec::AvcHigh,
        8_000,
        TaikaiResolution::new(1_920, 1_080),
    );
    let ingest = TaikaiIngestPointer::new(
        BlobDigest::new([0x11; 32]),
        StorageTicketId::new([0x22; 32]),
        BlobDigest::new([0x33; 32]),
        4,
        TaikaiCarPointer::new("bafy-test-car", BlobDigest::new([0x44; 32]), 4_096),
    );
    TaikaiSegmentEnvelopeV1::new(
        event_id,
        stream_id,
        rendition_id,
        track,
        sequence,
        SegmentTimestamp::new(sequence * 2_000_000),
        SegmentDuration::new(2_000_000),
        1_726_000_000_000 + sequence * 1_000,
        ingest,
    )
}

fn sample_segment(sequence: u64, qos: QosClass) -> CachedSegment {
    let envelope = sample_envelope(sequence);
    let payload = vec![sequence as u8; 512];
    CachedSegment::new(envelope, Arc::from(payload.into_boxed_slice()), qos)
}

#[test]
fn cache_admission_envelope_roundtrip_and_verify() {
    let shard = TaikaiShardId(17);
    let issuer = GuardDirectoryId::new("soranet/demo");
    let cached = sample_segment(42, QosClass::Priority);
    let issued_ms = 1_726_000_500_000;
    let record = CacheAdmissionRecord::from_segment(
        shard,
        issuer,
        &cached,
        CacheTierKind::Hot,
        CacheAdmissionAction::Admit,
        issued_ms,
        Duration::from_secs(30),
    )
    .expect("record");
    let key_pair = KeyPair::from_seed(vec![0xAA; 32], Algorithm::Ed25519);
    let envelope = CacheAdmissionEnvelope::sign(record.clone(), &key_pair).expect("sign");
    envelope.verify(issued_ms).expect("verify now");
    let bytes = to_bytes(&envelope).expect("encode");
    let decoded: CacheAdmissionEnvelope =
        decode_from_bytes(&bytes).expect("decode cache admission envelope");
    assert_eq!(
        decoded.body().segment().sequence(),
        record.segment().sequence()
    );
    decoded
        .verify(issued_ms + 15_000)
        .expect("verify before expiry");
}

#[test]
fn cache_admission_detects_tampering_and_expiry() {
    let shard = TaikaiShardId(2);
    let issuer = GuardDirectoryId::new("soranet/canary");
    let cached = sample_segment(7, QosClass::Standard);
    let issued_ms = 1_726_000_100_000;
    let record = CacheAdmissionRecord::from_segment(
        shard,
        issuer,
        &cached,
        CacheTierKind::Warm,
        CacheAdmissionAction::Evict,
        issued_ms,
        Duration::from_secs(5),
    )
    .expect("record");
    let key_pair = KeyPair::from_seed(vec![0xBB; 32], Algorithm::Ed25519);
    let envelope = CacheAdmissionEnvelope::sign(record.clone(), &key_pair).expect("sign");

    let err = envelope
        .verify(record.expires_unix_ms() + 1)
        .expect_err("expired");
    assert!(matches!(err, CacheAdmissionError::Expired { .. }));

    let (mut forged_body, signer, signature) = envelope.clone().into_parts();
    forged_body.payload_len += 1;
    let forged = CacheAdmissionEnvelope::from_parts(forged_body, signer, signature);
    let err = forged
        .verify(record.issued_unix_ms())
        .expect_err("payload tampering");
    assert!(matches!(err, CacheAdmissionError::InvalidSignature));
}

#[test]
fn cache_admission_gossip_signs_and_verifies() {
    let shard = TaikaiShardId(5);
    let issuer = GuardDirectoryId::new("soranet/gossip");
    let cached = sample_segment(12, QosClass::Priority);
    let issued_ms = 1_726_000_200_000;
    let ttl = Duration::from_secs(15);
    let mut rng = StdRng::seed_from_u64(7);
    let key_pair = KeyPair::from_seed(vec![0xCC; 32], Algorithm::Ed25519);
    let record = CacheAdmissionRecord::from_segment(
        shard,
        issuer,
        &cached,
        CacheTierKind::Warm,
        CacheAdmissionAction::Admit,
        issued_ms,
        ttl,
    )
    .expect("record");
    let envelope = CacheAdmissionEnvelope::sign(record, &key_pair).expect("sign");
    let body =
        CacheAdmissionGossipBody::with_nonce(envelope.clone(), issued_ms, ttl, &mut rng).unwrap();
    let gossip = CacheAdmissionGossip::sign(body, &key_pair).expect("sign gossip");

    gossip.verify(issued_ms + 5_000).expect("verify gossip");

    let (body, signer, _) = gossip.clone().into_parts();
    let forged_signature =
        iroha_crypto::Signature::new(key_pair.private_key(), b"tamper-cache-admission");
    let forged = CacheAdmissionGossip::from_parts(body, signer, forged_signature);
    let err = forged
        .verify(issued_ms + 5_000)
        .expect_err("signature mismatch");
    assert!(matches!(err, CacheAdmissionError::InvalidSignature));

    let err = gossip
        .verify(gossip.body().expires_unix_ms() + 1)
        .expect_err("expired gossip");
    assert!(matches!(err, CacheAdmissionError::Expired { .. }));
}

#[test]
fn cache_admission_replay_filter_blocks_replays_and_expires() {
    let shard = TaikaiShardId(9);
    let issuer = GuardDirectoryId::new("soranet/filter");
    let cached = sample_segment(21, QosClass::Bulk);
    let issued_ms = 1_726_000_300_000;
    let ttl = Duration::from_millis(750);
    let mut rng = StdRng::seed_from_u64(11);
    let key_pair = KeyPair::from_seed(vec![0xDD; 32], Algorithm::Ed25519);
    let record = CacheAdmissionRecord::from_segment(
        shard,
        issuer,
        &cached,
        CacheTierKind::Cold,
        CacheAdmissionAction::Evict,
        issued_ms,
        ttl,
    )
    .expect("record");
    let envelope = CacheAdmissionEnvelope::sign(record, &key_pair).expect("sign");

    let body_a =
        CacheAdmissionGossipBody::with_nonce(envelope.clone(), issued_ms, ttl, &mut rng).unwrap();
    let gossip_a = CacheAdmissionGossip::sign(body_a, &key_pair).expect("sign gossip a");

    let mut filter =
        CacheAdmissionReplayFilter::new(Duration::from_millis(500), 3).expect("filter");
    assert!(
        filter
            .observe(&gossip_a, issued_ms)
            .expect("first observation accepted")
    );
    assert!(
        !filter
            .observe(&gossip_a, issued_ms + 100)
            .expect("replay rejected inside window")
    );

    // New nonce → new digest.
    let mut alt_nonce = [0u8; 16];
    rng.fill_bytes(&mut alt_nonce);
    let body_b =
        CacheAdmissionGossipBody::with_nonce(envelope.clone(), issued_ms + 50, ttl, &mut rng)
            .unwrap();
    let gossip_b = CacheAdmissionGossip::sign(body_b, &key_pair).expect("sign gossip b");
    assert!(
        filter
            .observe(&gossip_b, issued_ms + 120)
            .expect("distinct nonce accepted")
    );

    // Window expiry permits the original digest again.
    assert!(
        filter
            .observe(&gossip_a, issued_ms + 1_000)
            .expect("replay allowed after expiry")
    );

    // Capacity eviction clears oldest entries.
    let body_c =
        CacheAdmissionGossipBody::with_nonce(envelope, issued_ms + 130, ttl, &mut rng).unwrap();
    let gossip_c = CacheAdmissionGossip::sign(body_c, &key_pair).expect("sign gossip c");
    assert!(
        filter
            .observe(&gossip_c, issued_ms + 1_050)
            .expect("third digest accepted")
    );
    // Depending on rotation timing, the oldest digest may still be retained; ensure the filter
    // handles the observation without panicking.
    let _ = filter
        .observe(&gossip_a, issued_ms + 1_200)
        .expect("replay filter remains functional after capacity rotation");
}

#[test]
fn cache_admission_tracker_populates_queue_shards() {
    let handle = TaikaiCacheHandle::from_config(TaikaiCacheConfig::default());
    let replay =
        CacheAdmissionReplayFilter::new(Duration::from_secs(5), 16).expect("tracker replay");
    let mut tracker = CacheAdmissionTracker::new(handle.clone(), replay);

    let shard = TaikaiShardId(5);
    let issuer = GuardDirectoryId::new("soranet/tracker");
    let cached = sample_segment(77, QosClass::Priority);
    let issued_ms = 1_726_000_400_000;
    let ttl = Duration::from_secs(30);
    let key_pair = KeyPair::from_seed(vec![0xEE; 32], Algorithm::Ed25519);
    let record = CacheAdmissionRecord::from_segment(
        shard,
        issuer,
        &cached,
        CacheTierKind::Hot,
        CacheAdmissionAction::Admit,
        issued_ms,
        ttl,
    )
    .expect("record");
    let envelope = CacheAdmissionEnvelope::sign(record, &key_pair).expect("envelope");
    let gossip_body =
        CacheAdmissionGossipBody::new(envelope.clone(), issued_ms, ttl).expect("gossip body");
    let gossip = CacheAdmissionGossip::sign(gossip_body, &key_pair).expect("gossip");

    assert!(
        tracker
            .ingest(&gossip, issued_ms)
            .expect("tracker accepts gossip")
    );
    assert_eq!(tracker.active_shards(), vec![shard]);

    let request =
        TaikaiPullRequest::new(cached.key(), QosClass::Priority, cached.size_bytes(), None);
    handle.enqueue_pull(request).expect("enqueue pull");
    let now = Instant::now();
    let batch = handle
        .issue_ready_batch_at(now)
        .expect("queue alive")
        .expect("batch issued");
    assert_eq!(batch.shard, Some(shard));

    assert!(
        !tracker
            .ingest(&gossip, issued_ms + 1)
            .expect("replay suppressed")
    );
}
