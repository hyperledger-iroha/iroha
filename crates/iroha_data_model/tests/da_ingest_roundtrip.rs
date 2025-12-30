//! Round-trip tests for DA ingest/manifest Norito types.

use std::{convert::TryFrom, str::FromStr};

use iroha_crypto::{PublicKey, Signature};
use iroha_data_model::{da::prelude::*, nexus::LaneId, sorafs::pin_registry::StorageClass};
use norito::{core::NoritoDeserialize, from_bytes};

fn sample_digest(seed: u8) -> BlobDigest {
    let mut bytes = [0u8; 32];
    for (idx, byte) in bytes.iter_mut().enumerate() {
        let offset = u8::try_from(idx).expect("digest index fits in u8");
        *byte = seed.wrapping_add(offset);
    }
    BlobDigest::new(bytes)
}

fn sample_signature(seed: u8) -> Signature {
    let mut payload = [0u8; 64];
    for (idx, byte) in payload.iter_mut().enumerate() {
        let offset = u8::try_from(idx).expect("signature index fits in u8");
        *byte = seed.wrapping_add(offset);
    }
    Signature::from_bytes(&payload)
}

fn sample_public_key() -> PublicKey {
    PublicKey::from_str("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03")
        .expect("ed25519 test key")
}

fn sample_ticket(seed: u8) -> StorageTicketId {
    let digest = sample_digest(seed);
    StorageTicketId::new(*digest.as_bytes())
}

fn sample_pdp_commitment_bytes() -> Vec<u8> {
    (0..96)
        .map(|idx| {
            let offset = u8::try_from(idx).expect("commitment index fits in u8");
            0xA0u8.wrapping_add(offset)
        })
        .collect()
}

#[test]
fn da_ingest_request_norito_roundtrip() {
    let request = DaIngestRequest {
        client_blob_id: sample_digest(0x11),
        lane_id: LaneId::new(2),
        epoch: 42,
        sequence: 7,
        blob_class: BlobClass::TaikaiSegment,
        codec: BlobCodec::new("cmaf"),
        erasure_profile: ErasureProfile {
            data_shards: 8,
            parity_shards: 4,
            row_parity_stripes: 2,
            chunk_alignment: 12,
            fec_scheme: FecScheme::Rs12_10,
        },
        retention_policy: RetentionPolicy {
            hot_retention_secs: 86_400,
            cold_retention_secs: 30 * 86_400,
            required_replicas: 4,
            storage_class: StorageClass::Hot,
            governance_tag: GovernanceTag::new("da.test"),
        },
        chunk_size: 1 << 20,
        total_size: 5_242_880,
        compression: Compression::Identity,
        norito_manifest: Some(vec![0xAA, 0xBB, 0xCC]),
        payload: b"hello data availability".to_vec(),
        metadata: ExtraMetadata {
            items: vec![
                MetadataEntry::new(
                    "content_type",
                    b"video/mp4".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    "operator_notes",
                    b"ingest-test".to_vec(),
                    MetadataVisibility::GovernanceOnly,
                ),
            ],
        },
        submitter: sample_public_key(),
        signature: sample_signature(0x42),
    };

    let buf = norito::to_bytes(&request).expect("serialize ingest request");
    let archived = from_bytes::<DaIngestRequest>(&buf).expect("decode request");
    let decoded = DaIngestRequest::deserialize(archived);
    assert_eq!(decoded, request);
}

#[test]
fn da_manifest_roundtrip() {
    let manifest = DaManifestV1 {
        version: DaManifestV1::VERSION,
        client_blob_id: sample_digest(0x21),
        lane_id: LaneId::new(7),
        epoch: 777,
        blob_class: BlobClass::GovernanceArtifact,
        codec: BlobCodec::new("norito-batch"),
        blob_hash: sample_digest(0x22),
        chunk_root: sample_digest(0x23),
        storage_ticket: sample_ticket(0x24),
        total_size: 9_437_184,
        chunk_size: 512 * 1024,
        total_stripes: 6,
        shards_per_stripe: 16,
        erasure_profile: ErasureProfile {
            data_shards: 10,
            parity_shards: 6,
            row_parity_stripes: 2,
            chunk_alignment: 10,
            fec_scheme: FecScheme::Rs18_14,
        },
        retention_policy: RetentionPolicy {
            hot_retention_secs: 48 * 3_600,
            cold_retention_secs: 120 * 86_400,
            required_replicas: 5,
            storage_class: StorageClass::Warm,
            governance_tag: GovernanceTag::new("da.governance"),
        },
        rent_quote: DaRentQuote::default(),
        chunks: vec![
            ChunkCommitment::new_with_role(
                0,
                0,
                512 * 1024,
                sample_digest(0x30),
                ChunkRole::Data,
                0,
            ),
            ChunkCommitment::new_with_role(
                1,
                512 * 1024,
                512 * 1024,
                sample_digest(0x31),
                ChunkRole::LocalParity,
                0,
            ),
            ChunkCommitment::new_with_role(
                10,
                10 * 512 * 1024,
                512 * 1024,
                sample_digest(0x32),
                ChunkRole::StripeParity,
                4,
            ),
        ],
        ipa_commitment: sample_digest(0x33),
        metadata: ExtraMetadata {
            items: vec![MetadataEntry::new(
                "category",
                b"governance".to_vec(),
                MetadataVisibility::Public,
            )],
        },
        issued_at_unix: 1_707_000_000,
    };

    let buf = norito::to_bytes(&manifest).expect("serialize manifest");
    let archived = from_bytes::<DaManifestV1>(&buf).expect("decode manifest");
    let decoded = DaManifestV1::deserialize(archived);
    assert_eq!(decoded, manifest);
}

#[test]
fn da_ingest_receipt_roundtrip() {
    let receipt = DaIngestReceipt {
        client_blob_id: sample_digest(0x51),
        lane_id: LaneId::new(3),
        epoch: 1024,
        blob_hash: sample_digest(0x52),
        chunk_root: sample_digest(0x53),
        manifest_hash: sample_digest(0x54),
        storage_ticket: sample_ticket(0x55),
        pdp_commitment: Some(sample_pdp_commitment_bytes()),
        stripe_layout: DaStripeLayout {
            total_stripes: 4,
            shards_per_stripe: 12,
            row_parity_stripes: 1,
        },
        queued_at_unix: 1_707_100_000,
        rent_quote: DaRentQuote::default(),
        operator_signature: sample_signature(0x99),
    };

    let buf = norito::to_bytes(&receipt).expect("serialize receipt");
    let archived = from_bytes::<DaIngestReceipt>(&buf).expect("decode receipt");
    let decoded = DaIngestReceipt::deserialize(archived);
    assert_eq!(decoded, receipt);
}
