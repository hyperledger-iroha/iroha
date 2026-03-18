//! Helpers for inspecting manifest metadata and provider capabilities.

use crate::{
    ChunkingProfileV1, ManifestV1,
    provider_advert::{CapabilityType, ProviderAdvertBodyV1, ProviderCapabilityRangeV1},
};

/// Summary of manifest chunker metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkProfileSummary {
    /// Governance-assigned profile identifier.
    pub profile_id: u32,
    /// Registry namespace.
    pub namespace: String,
    /// Registry name.
    pub name: String,
    /// Semantic version string.
    pub semver: String,
    /// Minimum chunk size (bytes).
    pub min_size: u32,
    /// Target chunk size (bytes).
    pub target_size: u32,
    /// Maximum chunk size (bytes).
    pub max_size: u32,
    /// Optional aliases published in the registry.
    pub aliases: Vec<String>,
    /// Multihash code baked into the profile.
    pub multihash_code: u64,
}

impl From<&ChunkingProfileV1> for ChunkProfileSummary {
    fn from(profile: &ChunkingProfileV1) -> Self {
        Self {
            profile_id: profile.profile_id.0,
            namespace: profile.namespace.clone(),
            name: profile.name.clone(),
            semver: profile.semver.clone(),
            min_size: profile.min_size,
            target_size: profile.target_size,
            max_size: profile.max_size,
            aliases: profile.aliases.clone(),
            multihash_code: profile.multihash_code,
        }
    }
}

/// Summary of manifest transport capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestCapabilitySummary {
    /// Chunker metadata baked into the manifest.
    pub chunk_profile: Option<ChunkProfileSummary>,
    /// Raw metadata entries bundled in the manifest.
    pub metadata_pairs: Vec<(String, String)>,
    /// Whether clients must attach the manifest envelope.
    pub requires_manifest_envelope: bool,
    /// Whether direct-CAR streaming is explicitly advertised by manifest metadata.
    pub direct_car_supported: bool,
    /// Provider advertises Torii gateway support.
    pub supports_torii_gateway: bool,
    /// Provider advertises QUIC + Noise transport.
    pub supports_quic_noise: bool,
    /// Provider advertises SoraNet transport support.
    pub supports_soranet: bool,
    /// Provider explicitly advertises SoraNet PQ hybrid capability.
    pub supports_soranet_hybrid_pq: bool,
    /// Provider advertises ranged chunk fetch capability.
    pub supports_chunk_range_fetch: bool,
    /// Parsed range capability payload when advertised.
    pub range_capability: Option<ProviderCapabilityRangeV1>,
    /// Raw capability types observed in the provider advert.
    pub advertised_capabilities: Vec<CapabilityType>,
}

impl Default for ManifestCapabilitySummary {
    fn default() -> Self {
        Self {
            chunk_profile: None,
            metadata_pairs: Vec::new(),
            requires_manifest_envelope: true,
            direct_car_supported: false,
            supports_torii_gateway: false,
            supports_quic_noise: false,
            supports_soranet: false,
            supports_soranet_hybrid_pq: false,
            supports_chunk_range_fetch: false,
            range_capability: None,
            advertised_capabilities: Vec::new(),
        }
    }
}

/// Analyse manifest metadata and optional provider advert for capability hints.
#[must_use]
pub fn detect_manifest_capabilities(
    manifest: Option<&ManifestV1>,
    advert: Option<&ProviderAdvertBodyV1>,
) -> ManifestCapabilitySummary {
    let mut summary = ManifestCapabilitySummary::default();

    if let Some(manifest) = manifest {
        summary.chunk_profile = Some(ChunkProfileSummary::from(&manifest.chunking));
        for entry in &manifest.metadata {
            summary
                .metadata_pairs
                .push((entry.key.clone(), entry.value.clone()));
            let key = entry.key.to_ascii_lowercase();
            let value = entry.value.trim();
            if matches!(
                key.as_str(),
                "manifest.requires_envelope" | "manifest:requires_envelope"
            ) {
                summary.requires_manifest_envelope =
                    parse_bool(value).unwrap_or(summary.requires_manifest_envelope);
            }
            if matches!(
                key.as_str(),
                "capability.direct_car" | "capability:direct_car"
            ) {
                summary.direct_car_supported |= parse_bool(value).unwrap_or(false);
            }
        }
    }

    if let Some(advert) = advert {
        for capability in &advert.capabilities {
            summary.advertised_capabilities.push(capability.cap_type);
            match capability.cap_type {
                CapabilityType::ToriiGateway => summary.supports_torii_gateway = true,
                CapabilityType::QuicNoise => summary.supports_quic_noise = true,
                CapabilityType::SoraNetHybridPq => {
                    summary.supports_soranet = true;
                    summary.supports_soranet_hybrid_pq = true;
                }
                CapabilityType::ChunkRangeFetch => {
                    summary.supports_chunk_range_fetch = true;
                    if summary.range_capability.is_none()
                        && let Ok(range) =
                            ProviderCapabilityRangeV1::from_bytes(&capability.payload)
                    {
                        summary.range_capability = Some(range);
                    }
                }
                CapabilityType::VendorReserved => {}
            }
        }
    }

    summary
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "y" => Some(true),
        "false" | "0" | "no" | "n" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ChunkingProfileV1, ManifestBuilder, PinPolicy, StorageClass,
        provider_advert::{
            CapabilityTlv, ProviderAdvertBodyV1, ProviderCapabilityRangeV1,
            ProviderCapabilitySoranetPqV1,
        },
    };

    fn sample_manifest() -> ManifestV1 {
        ManifestBuilder::new()
            .root_cid(vec![0x01, 0x02, 0x03])
            .dag_codec(crate::DagCodecId(0x71))
            .chunking_profile(ChunkingProfileV1 {
                profile_id: crate::ProfileId(7),
                namespace: "sorafs".into(),
                name: "sf1".into(),
                semver: "1.0.0".into(),
                min_size: 4096,
                target_size: 131_072,
                max_size: 262_144,
                break_mask: 0,
                multihash_code: crate::BLAKE3_256_MULTIHASH_CODE,
                aliases: vec!["sf1".into()],
            })
            .content_length(1_048_576)
            .car_digest([0xAA; 32])
            .car_size(1_090_000)
            .pin_policy(PinPolicy {
                min_replicas: 3,
                storage_class: StorageClass::Hot,
                retention_epoch: 0,
            })
            .add_metadata("manifest.requires_envelope", "true")
            .add_metadata("capability.direct_car", "yes")
            .build()
            .expect("build manifest")
    }

    fn sample_advert(range_payload: Vec<u8>) -> ProviderAdvertBodyV1 {
        use crate::provider_advert::{
            AdvertEndpoint, AvailabilityTier, EndpointKind, PathDiversityPolicy, QosHints,
            RendezvousTopic, StakePointer, TransportHintV1, TransportProtocol,
        };
        ProviderAdvertBodyV1 {
            provider_id: [0x11; 32],
            profile_id: "sorafs.sf1@1.0.0".into(),
            profile_aliases: Some(vec!["sf1".into()]),
            stake: StakePointer {
                pool_id: [0x22; 32],
                stake_amount: 1,
            },
            qos: QosHints {
                availability: AvailabilityTier::Hot,
                max_retrieval_latency_ms: 500,
                max_concurrent_streams: 16,
            },
            capabilities: vec![
                CapabilityTlv {
                    cap_type: CapabilityType::ToriiGateway,
                    payload: Vec::new(),
                },
                CapabilityTlv {
                    cap_type: CapabilityType::ChunkRangeFetch,
                    payload: range_payload,
                },
            ],
            endpoints: vec![AdvertEndpoint {
                kind: EndpointKind::Torii,
                host_pattern: "gateway.example.com".into(),
                metadata: Vec::new(),
            }],
            rendezvous_topics: vec![RendezvousTopic {
                topic: "sorafs.sf1.primary".into(),
                region: "global".into(),
            }],
            path_policy: PathDiversityPolicy {
                min_guard_weight: 1,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: None,
            stream_budget: None,
            transport_hints: Some(vec![TransportHintV1 {
                protocol: TransportProtocol::ToriiHttpRange,
                priority: 0,
            }]),
        }
    }

    #[test]
    fn detect_manifest_metadata_capabilities() {
        let manifest = sample_manifest();
        let summary = detect_manifest_capabilities(Some(&manifest), None);
        assert!(summary.requires_manifest_envelope);
        assert!(summary.direct_car_supported);
        let chunk_profile = summary.chunk_profile.expect("chunk profile summary");
        assert_eq!(chunk_profile.namespace, "sorafs");
        assert_eq!(chunk_profile.name, "sf1");
        assert_eq!(chunk_profile.semver, "1.0.0");
    }

    #[test]
    fn detect_advert_capabilities() {
        let range = ProviderCapabilityRangeV1 {
            max_chunk_span: 4,
            min_granularity: 1,
            supports_sparse_offsets: true,
            requires_alignment: false,
            supports_merkle_proof: true,
        }
        .to_bytes()
        .expect("encode range capability");
        let advert = sample_advert(range);
        let summary = detect_manifest_capabilities(None, Some(&advert));
        assert!(summary.supports_torii_gateway);
        assert!(summary.supports_chunk_range_fetch);
        assert!(summary.range_capability.is_some());
        assert!(
            summary
                .advertised_capabilities
                .contains(&CapabilityType::ToriiGateway)
        );
    }

    #[test]
    fn detect_soranet_pq_capability_marks_flags() {
        let mut advert = sample_advert(Vec::new());
        let pq_payload = ProviderCapabilitySoranetPqV1 {
            supports_guard: true,
            supports_majority: true,
            supports_strict: false,
        }
        .to_bytes()
        .expect("encode PQ capability");
        advert.capabilities.push(CapabilityTlv {
            cap_type: CapabilityType::SoraNetHybridPq,
            payload: pq_payload,
        });
        let summary = detect_manifest_capabilities(None, Some(&advert));
        assert!(summary.supports_soranet);
        assert!(summary.supports_soranet_hybrid_pq);
    }

    #[test]
    fn parse_bool_accepts_variants() {
        assert_eq!(parse_bool("YES"), Some(true));
        assert_eq!(parse_bool(" no "), Some(false));
        assert_eq!(parse_bool("2"), None);
    }
}
