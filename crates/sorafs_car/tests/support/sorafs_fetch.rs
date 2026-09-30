//! Shared synthetic payload, signed-advert and private-path fixtures for fetch tests.
use ed25519_dalek::{Signer, SigningKey};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    AdvertEndpoint, AdvertSignature, EndpointKind, EndpointMetadata, EndpointMetadataKey,
    PROVIDER_ADVERT_VERSION_V1, ProviderAdvertBodyV1, ProviderAdvertV1, ProviderCapabilityRangeV1,
    RendezvousTopic, SignatureAlgorithm, StreamBudgetV1, TransportHintV1, TransportProtocol,
    deal::XorQuantity,
};
use std::path::PathBuf;
use tempfile::{TempDir, tempdir};
pub(super) fn canonical_tempdir() -> (TempDir, PathBuf) {
    let temp = tempdir().expect("tempdir");
    let path = temp.path().canonicalize().expect("canonical tempdir");
    (temp, path)
}
pub(super) fn xor_micro(value: u128) -> XorQuantity {
    XorQuantity::try_from_micro(value).expect("test micro-XOR amount is representable")
}
pub(super) fn range_capability_payload() -> Vec<u8> {
    let profile = ChunkProfile::DEFAULT;
    ProviderCapabilityRangeV1 {
        max_chunk_span: profile.max_size as u32,
        min_granularity: profile.min_size as u32,
        supports_sparse_offsets: true,
        requires_alignment: false,
        supports_merkle_proof: true,
    }
    .to_bytes()
    .expect("encode range capability")
}
pub(super) fn sample_stream_budget() -> StreamBudgetV1 {
    StreamBudgetV1 {
        max_in_flight: 4,
        max_bytes_per_sec: 5_000_000,
        burst_bytes: Some(2_500_000),
    }
}
pub(super) fn signed_provider_advert(
    body: ProviderAdvertBodyV1,
    signing_key: &SigningKey,
    issued_at: u64,
    expires_at: u64,
    allow_unknown_capabilities: bool,
) -> ProviderAdvertV1 {
    let mut advert = ProviderAdvertV1 {
        version: PROVIDER_ADVERT_VERSION_V1,
        network_id: [0xA1; 32],
        issued_at,
        expires_at,
        body,
        signature: AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: vec![0; 64],
        },
        signature_strict: true,
        allow_unknown_capabilities,
    };
    let payload = advert
        .signature_payload_bytes()
        .expect("serialize advert signature envelope");
    advert.signature.signature = signing_key.sign(&payload).to_bytes().to_vec();
    advert
}
pub(super) fn sample_endpoint() -> AdvertEndpoint {
    AdvertEndpoint {
        kind: EndpointKind::Torii,
        host_pattern: "torii.example.org".into(),
        metadata: vec![EndpointMetadata {
            key: EndpointMetadataKey::Region,
            value: b"global".to_vec(),
        }],
    }
}
pub(super) fn sample_rendezvous_topics(label: &str) -> Vec<RendezvousTopic> {
    vec![RendezvousTopic {
        topic: format!("sorafs.{label}.primary"),
        region: "global".into(),
    }]
}
pub(super) fn sample_transport_hints() -> Vec<TransportHintV1> {
    vec![TransportHintV1 {
        protocol: TransportProtocol::ToriiHttpRange,
        priority: 0,
    }]
}
