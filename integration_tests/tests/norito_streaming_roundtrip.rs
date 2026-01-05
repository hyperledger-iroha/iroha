//! Integration coverage for the Norito Streaming baseline codec and chunk helpers.

#[path = "streaming/mod.rs"]
mod streaming;

use hex::encode as hex_encode;
use norito::streaming::{
    FecScheme, Hash, Multiaddr, PrivacyCapabilities, PrivacyRelay, PrivacyRoute, Signature,
    StreamMetadata,
    chunk::{self, BaselineDecoder},
    codec::{BaselineEncoderConfig, BaselineManifestParams, EncodedSegment, RawFrame},
};

struct SegmentFixture {
    label: &'static str,
    config: BaselineEncoderConfig,
    segment: EncodedSegment,
    frames: Vec<RawFrame>,
}

fn segment_fixtures() -> Vec<SegmentFixture> {
    let mut fixtures = Vec::new();

    let (config, segment, frames) = streaming::baseline_segment(2);
    fixtures.push(SegmentFixture {
        label: "baseline",
        config,
        segment,
        frames,
    });

    let (config, segment, frames, _telemetry) = streaming::bundled_segment(2, 4);
    fixtures.push(SegmentFixture {
        label: "rans_bundled",
        config,
        segment,
        frames,
    });

    fixtures
}

fn sample_hash(seed: u8) -> Hash {
    let mut bytes = [0u8; 32];
    bytes.fill(seed);
    bytes
}

fn sample_signature(seed: u8) -> Signature {
    let mut bytes = [0u8; 64];
    bytes.fill(seed);
    bytes
}

#[test]
fn segment_manifest_roundtrip() {
    for fixture in segment_fixtures() {
        let manifest = fixture.segment.build_manifest(BaselineManifestParams {
            stream_id: sample_hash(11),
            protocol_version: 1,
            published_at: 1_702_000_000,
            da_endpoint: Multiaddr::from("/dns/publisher.nsc/quic"),
            privacy_routes: vec![PrivacyRoute {
                route_id: sample_hash(12),
                entry: PrivacyRelay {
                    relay_id: sample_hash(13),
                    endpoint: Multiaddr::from("/dns/entry.relay/quic"),
                    key_fingerprint: sample_hash(14),
                    capabilities: PrivacyCapabilities::from_bits(0b001),
                },
                exit: PrivacyRelay {
                    relay_id: sample_hash(15),
                    endpoint: Multiaddr::from("/dns/exit.relay/quic"),
                    key_fingerprint: sample_hash(16),
                    capabilities: PrivacyCapabilities::from_bits(0b010),
                },
                ticket_entry: vec![1, 2, 3, 4],
                ticket_exit: vec![5, 6, 7, 8],
                expiry_segment: 128,
                soranet: None,
            }],
            public_metadata: StreamMetadata {
                title: "NSC Sample Stream".into(),
                description: Some("Roundtrip coverage for baseline codec.".into()),
                access_policy_id: Some(sample_hash(17)),
                tags: vec!["nsc".into(), "baseline".into()],
            },
            capabilities: streaming::BASE_CAPABILITIES,
            signature: sample_signature(21),
            fec_suite: FecScheme::Rs12_10,
            neural_bundle: None,
            transport_capabilities_hash: sample_hash(22),
        });

        fixture
            .segment
            .verify_manifest(&manifest)
            .unwrap_or_else(|err| panic!("manifest mismatch for {}: {err}", fixture.label));
        assert_eq!(
            manifest.segment_number, fixture.segment.header.segment_number,
            "{} segment number mismatch",
            fixture.label
        );
        assert_eq!(
            manifest.chunk_root, fixture.segment.header.chunk_merkle_root,
            "{} manifest advertises incorrect commitment",
            fixture.label
        );
        assert_eq!(
            manifest.transport_capabilities_hash,
            sample_hash(22),
            "{} transport capability hash mismatch",
            fixture.label
        );

        let decoder = BaselineDecoder::new(
            fixture.config.frame_dimensions,
            fixture.config.frame_duration_ns,
        );
        let decoded_frames = decoder
            .decode_segment(&fixture.segment)
            .unwrap_or_else(|err| panic!("decode failed for {}: {err}", fixture.label));
        assert_eq!(
            decoded_frames.len(),
            fixture.frames.len(),
            "{} frame count mismatch",
            fixture.label
        );
        let pts_step = u64::from(fixture.config.frame_duration_ns);
        for (idx, frame) in decoded_frames.iter().enumerate() {
            assert_eq!(
                frame.index as usize, idx,
                "{} frame index mismatch",
                fixture.label
            );
            let expected_pts = fixture.segment.header.timeline_start_ns + pts_step * idx as u64;
            assert_eq!(
                frame.pts_ns, expected_pts,
                "{} PTS mismatch at frame {}",
                fixture.label, idx
            );
            assert_eq!(
                frame.luma.len(),
                fixture.frames[idx].luma.len(),
                "{} decoded frame length mismatch at index {}",
                fixture.label,
                idx
            );
            assert_eq!(
                frame.luma, fixture.frames[idx].luma,
                "{} decoded frame mismatch at index {}",
                fixture.label, idx
            );
        }
    }
}

#[test]
fn chunk_merkle_proof_roundtrip() {
    for fixture in segment_fixtures() {
        let payload_refs: Vec<(u16, &[u8])> = fixture
            .segment
            .descriptors
            .iter()
            .zip(fixture.segment.chunks.iter())
            .map(|(descriptor, chunk)| (descriptor.chunk_id, chunk.as_slice()))
            .collect();
        let commitments =
            chunk::chunk_commitments(fixture.segment.header.segment_number, &payload_refs);
        assert_eq!(
            commitments.len(),
            fixture.segment.chunks.len(),
            "{} commitment count mismatch",
            fixture.label
        );

        let root = chunk::merkle_root(&commitments).expect("merkle root");
        assert_eq!(
            root, fixture.segment.header.chunk_merkle_root,
            "{} chunk root mismatch",
            fixture.label
        );

        let proof_index = usize::from(commitments.len() > 1);
        let proof = chunk::merkle_proof(&commitments, proof_index, payload_refs[proof_index].0)
            .expect("merkle proof");
        let leaf = commitments[proof_index];
        assert!(
            chunk::verify_merkle_proof(&leaf, &proof, &root),
            "{} merkle proof validation failed",
            fixture.label
        );

        let chunk_ids: Vec<u16> = payload_refs.iter().map(|(id, _)| *id).collect();
        let storage_commitment = chunk::storage_commitment(
            fixture.segment.header.segment_number,
            fixture.segment.header.content_key_id,
            &root,
            &chunk_ids,
        )
        .expect("storage commitment");
        let da_root = chunk::data_availability_root(
            fixture.segment.header.segment_number,
            fixture.segment.header.content_key_id,
            &root,
            &chunk_ids,
        )
        .expect("da root");

        assert_ne!(
            storage_commitment, [0u8; 32],
            "{} storage commitment unexpectedly zeroed",
            fixture.label
        );
        assert_ne!(
            da_root, [0u8; 32],
            "{} data availability root unexpectedly zeroed",
            fixture.label
        );
        assert_ne!(
            storage_commitment, da_root,
            "{} storage/DA roots must use distinct labels",
            fixture.label
        );

        let storage_hex = hex_encode(storage_commitment);
        let da_hex = hex_encode(da_root);
        assert_eq!(
            storage_hex.len(),
            64,
            "{} storage commitment hex length mismatch",
            fixture.label
        );
        assert_eq!(
            da_hex.len(),
            64,
            "{} DA root hex length mismatch",
            fixture.label
        );
    }
}
