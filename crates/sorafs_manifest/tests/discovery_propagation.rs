//! Discovery propagation integration tests covering gossip, signature checks, and capability GREASE.

use std::collections::{HashMap, HashSet, VecDeque};

use blake3::hash as blake3_hash;
use ed25519_dalek::{
    PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH, Signature as DalekSignature, Signer, SigningKey, Verifier,
    VerifyingKey,
};
use sorafs_manifest::{
    AdvertEndpoint, AdvertSignature, AvailabilityTier, CapabilityTlv, CapabilityType, EndpointKind,
    EndpointMetadata, EndpointMetadataKey, MAX_ADVERT_TTL_SECS, PROVIDER_ADVERT_VERSION_V1,
    PathDiversityPolicy, ProviderAdvertBodyV1, ProviderAdvertV1, ProviderCapabilityRangeV1,
    QosHints, RendezvousTopic, SignatureAlgorithm, StakePointer,
};

const ISSUED_AT: u64 = 1_700_000_000;
const TTL_SECS: u64 = 3_600;

#[test]
fn dht_gossip_propagates_valid_adverts() {
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let advert = make_signed_advert(
        &signing_key,
        [0x11; 32],
        [0x21; 32],
        vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::ChunkRangeFetch,
                payload: ProviderCapabilityRangeV1 {
                    max_chunk_span: 16,
                    min_granularity: 4,
                    supports_sparse_offsets: true,
                    requires_alignment: false,
                    supports_merkle_proof: false,
                }
                .to_bytes()
                .expect("encode range capability"),
            },
        ],
        false,
    );

    let mut mesh = TestMesh::with_edges(
        vec![
            TestNode::new(
                "alpha",
                &[
                    CapabilityType::ToriiGateway,
                    CapabilityType::ChunkRangeFetch,
                ],
            ),
            TestNode::new(
                "beta",
                &[
                    CapabilityType::ToriiGateway,
                    CapabilityType::ChunkRangeFetch,
                ],
            ),
            TestNode::new(
                "gamma",
                &[
                    CapabilityType::ToriiGateway,
                    CapabilityType::ChunkRangeFetch,
                ],
            ),
        ],
        &[("alpha", "beta"), ("beta", "gamma")],
    );

    mesh.publish("alpha", advert.clone(), ISSUED_AT + 30)
        .expect("propagation succeeds");

    for id in ["alpha", "beta", "gamma"] {
        let node = mesh.node(id);
        assert_eq!(
            node.stored_count(),
            1,
            "{id} should retain the propagated advert"
        );
        assert_eq!(
            node.capabilities_for(&advert.body.provider_id)
                .expect("node stored advert"),
            vec![
                CapabilityType::ToriiGateway,
                CapabilityType::ChunkRangeFetch
            ],
            "{id} should keep both advertised capabilities"
        );
    }
}

#[test]
fn invalid_signature_is_rejected_and_not_gossiped() {
    let signing_key = SigningKey::from_bytes(&[8u8; 32]);
    let mut advert = make_signed_advert(
        &signing_key,
        [0x33; 32],
        [0x44; 32],
        vec![CapabilityTlv {
            cap_type: CapabilityType::ToriiGateway,
            payload: Vec::new(),
        }],
        false,
    );
    advert.signature.signature[0] ^= 0xFF;

    let mut mesh = TestMesh::with_edges(
        vec![
            TestNode::new("alpha", &[CapabilityType::ToriiGateway]),
            TestNode::new("beta", &[CapabilityType::ToriiGateway]),
        ],
        &[("alpha", "beta")],
    );

    let result = mesh.publish("alpha", advert.clone(), ISSUED_AT + 10);
    assert!(
        result
            .err()
            .unwrap_or_default()
            .contains("signature verification failed"),
        "origin should reject invalid signatures"
    );
    assert_eq!(
        mesh.node("alpha").stored_count(),
        0,
        "invalid adverts must not be retained locally"
    );
    assert_eq!(
        mesh.node("beta").stored_count(),
        0,
        "invalid adverts must not be propagated"
    );
}

#[test]
fn mixed_capability_peers_honour_grease_policy() {
    let signing_key = SigningKey::from_bytes(&[9u8; 32]);
    let grease_cap = CapabilityTlv {
        cap_type: CapabilityType::VendorReserved,
        payload: vec![0xAA, 0xBB],
    };

    let modern_advert = make_signed_advert(
        &signing_key,
        [0x55; 32],
        [0x65; 32],
        vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            grease_cap.clone(),
        ],
        true,
    );

    let mut mesh = TestMesh::with_edges(
        vec![
            TestNode::new(
                "modern",
                &[CapabilityType::ToriiGateway, CapabilityType::VendorReserved],
            ),
            TestNode::new(
                "relay",
                &[CapabilityType::ToriiGateway, CapabilityType::VendorReserved],
            ),
            TestNode::new("strict", &[CapabilityType::ToriiGateway]),
        ],
        &[("modern", "relay"), ("relay", "strict")],
    );

    mesh.publish("modern", modern_advert.clone(), ISSUED_AT + 90)
        .expect("GREASE-allowed advert should propagate");

    let strict_caps = mesh
        .node("strict")
        .capabilities_for(&modern_advert.body.provider_id)
        .expect("strict node should accept GREASE advert");
    assert_eq!(
        strict_caps,
        vec![CapabilityType::ToriiGateway],
        "strict node keeps only known capabilities"
    );
    assert!(
        mesh.node("strict").rejection_reasons().is_empty(),
        "GREASE-enabled advert should not record rejections"
    );

    let strict_advert = make_signed_advert(
        &signing_key,
        [0x77; 32],
        [0x88; 32],
        vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            grease_cap,
        ],
        false,
    );

    let mut mesh = TestMesh::with_edges(
        vec![
            TestNode::new(
                "modern",
                &[CapabilityType::ToriiGateway, CapabilityType::VendorReserved],
            ),
            TestNode::new(
                "relay",
                &[CapabilityType::ToriiGateway, CapabilityType::VendorReserved],
            ),
            TestNode::new("strict", &[CapabilityType::ToriiGateway]),
        ],
        &[("modern", "relay"), ("relay", "strict")],
    );

    mesh.publish("modern", strict_advert.clone(), ISSUED_AT + 120)
        .expect("origin accepts advert it can verify");

    assert!(
        mesh.node("strict")
            .capabilities_for(&strict_advert.body.provider_id)
            .is_none(),
        "strict node must drop adverts with unknown capabilities when GREASE is disabled"
    );
    let rejection_messages = mesh.node("strict").rejection_reasons();
    assert_eq!(rejection_messages.len(), 1);
    assert!(
        rejection_messages[0].contains("VendorReserved"),
        "rejection should mention the unknown capability"
    );
    assert_eq!(
        mesh.node("relay").stored_count(),
        1,
        "intermediate peers with knowledge retain the advert"
    );
}

#[test]
fn stale_advert_is_rejected() {
    let signing_key = SigningKey::from_bytes(&[0xAB; 32]);
    let advert = make_signed_advert(
        &signing_key,
        [0x90; 32],
        [0xA0; 32],
        vec![CapabilityTlv {
            cap_type: CapabilityType::ToriiGateway,
            payload: Vec::new(),
        }],
        false,
    );

    let mut mesh = TestMesh::with_edges(
        vec![TestNode::new(
            "alpha",
            &[
                CapabilityType::ToriiGateway,
                CapabilityType::ChunkRangeFetch,
            ],
        )],
        &[],
    );

    let expired_at = advert.expires_at.saturating_add(1);
    let err = mesh
        .publish("alpha", advert.clone(), expired_at)
        .expect_err("expired advert must be rejected");
    assert!(
        err.contains("expired"),
        "error should mention expiration, got: {err}"
    );
    assert_eq!(
        mesh.node("alpha").stored_count(),
        0,
        "stale advert must not be retained locally"
    );
}

#[test]
fn duplicate_advert_is_ignored_without_recounting() {
    let signing_key = SigningKey::from_bytes(&[0xBC; 32]);
    let advert = make_signed_advert(
        &signing_key,
        [0x10; 32],
        [0x20; 32],
        vec![CapabilityTlv {
            cap_type: CapabilityType::ToriiGateway,
            payload: Vec::new(),
        }],
        false,
    );

    let mut mesh = TestMesh::with_edges(
        vec![
            TestNode::new("alpha", &[CapabilityType::ToriiGateway]),
            TestNode::new("beta", &[CapabilityType::ToriiGateway]),
        ],
        &[("alpha", "beta")],
    );

    mesh.publish("alpha", advert.clone(), ISSUED_AT + 5)
        .expect("initial gossip succeeds");
    assert_eq!(
        mesh.node("beta").stored_count(),
        1,
        "neighbor must store first copy"
    );

    mesh.publish("alpha", advert.clone(), ISSUED_AT + 6)
        .expect("duplicate gossip treated as no-op");
    assert_eq!(
        mesh.node("alpha").stored_count(),
        1,
        "origin retains exactly one advert after duplicate gossip"
    );
    assert_eq!(
        mesh.node("beta").stored_count(),
        1,
        "neighbor store count remains unchanged after duplicate gossip"
    );
}

#[test]
fn invalid_path_policy_is_rejected() {
    let signing_key = SigningKey::from_bytes(&[0xCD; 32]);
    let mut advert = make_signed_advert(
        &signing_key,
        [0x31; 32],
        [0x41; 32],
        vec![CapabilityTlv {
            cap_type: CapabilityType::ToriiGateway,
            payload: Vec::new(),
        }],
        false,
    );

    advert.body.path_policy.min_guard_weight = 0;

    let mut mesh = TestMesh::with_edges(
        vec![TestNode::new("alpha", &[CapabilityType::ToriiGateway])],
        &[],
    );

    let err = mesh
        .publish("alpha", advert, ISSUED_AT + 10)
        .expect_err("invalid path policy must be rejected");
    assert!(
        err.contains("path diversity"),
        "error should mention path diversity violation, got: {err}"
    );
    assert_eq!(
        mesh.node("alpha").stored_count(),
        0,
        "invalid adverts must not be persisted"
    );
}

#[allow(clippy::assertions_on_constants)]
fn make_signed_advert(
    signing_key: &SigningKey,
    provider_id: [u8; 32],
    stake_pool_id: [u8; 32],
    capabilities: Vec<CapabilityTlv>,
    allow_unknown_capabilities: bool,
) -> ProviderAdvertV1 {
    assert!(
        TTL_SECS <= MAX_ADVERT_TTL_SECS,
        "test TTL must respect advert bounds"
    );

    let body = ProviderAdvertBodyV1 {
        provider_id,
        profile_id: "sorafs.sf1@1.0.0".to_owned(),
        profile_aliases: Some(vec!["sorafs.sf1@1.0.0".to_owned(), "sorafs-sf1".to_owned()]),
        stake: StakePointer {
            pool_id: stake_pool_id,
            stake_amount: 5_000_000,
        },
        qos: QosHints {
            availability: AvailabilityTier::Hot,
            max_retrieval_latency_ms: 1_200,
            max_concurrent_streams: 32,
        },
        capabilities,
        endpoints: vec![AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: "storage.example.com".to_owned(),
            metadata: vec![EndpointMetadata {
                key: EndpointMetadataKey::Region,
                value: b"global".to_vec(),
            }],
        }],
        rendezvous_topics: vec![RendezvousTopic {
            topic: "sorafs.sf1.primary".to_owned(),
            region: "global".to_owned(),
        }],
        path_policy: PathDiversityPolicy {
            min_guard_weight: 5,
            max_same_asn_per_path: 1,
            max_same_pool_per_path: 1,
        },
        notes: None,
        stream_budget: None,
        transport_hints: None,
    };
    body.validate()
        .expect("test vectors must build valid advert bodies");

    let body_bytes =
        norito::to_bytes(&body).expect("advert body must encode deterministically for signing");
    let signature = signing_key.sign(&body_bytes);

    ProviderAdvertV1 {
        version: PROVIDER_ADVERT_VERSION_V1,
        issued_at: ISSUED_AT,
        expires_at: ISSUED_AT
            .checked_add(TTL_SECS)
            .expect("ttl addition must not overflow"),
        body,
        signature: AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: signature.to_bytes().to_vec(),
        },
        signature_strict: true,
        allow_unknown_capabilities,
    }
}

struct TestMesh {
    nodes: HashMap<&'static str, TestNode>,
    adjacency: HashMap<&'static str, Vec<&'static str>>,
}

impl TestMesh {
    fn with_edges(nodes: Vec<TestNode>, edges: &[(&'static str, &'static str)]) -> Self {
        let mut adjacency: HashMap<&'static str, Vec<&'static str>> = HashMap::new();
        for &(a, b) in edges {
            adjacency.entry(a).or_default().push(b);
            adjacency.entry(b).or_default().push(a);
        }

        let mut node_map = HashMap::new();
        for node in nodes {
            adjacency.entry(node.id).or_default();
            node_map.insert(node.id, node);
        }

        Self {
            nodes: node_map,
            adjacency,
        }
    }

    fn publish(
        &mut self,
        origin: &'static str,
        advert: ProviderAdvertV1,
        now: u64,
    ) -> Result<(), String> {
        let mut queue = VecDeque::new();
        match self.node_mut(origin)?.accept(&advert, now)? {
            Acceptance::Stored => queue.push_back((origin, advert.clone())),
            Acceptance::AlreadyKnown => return Ok(()),
            Acceptance::Rejected(reason) => {
                return Err(format!("origin {origin} rejected advert: {reason}"));
            }
        }

        let mut visited: HashSet<&'static str> = HashSet::new();
        visited.insert(origin);

        while let Some((current, advert)) = queue.pop_front() {
            if let Some(neighbors) = self.adjacency.get(current).cloned() {
                for neighbor in neighbors {
                    let acceptance = self.node_mut(neighbor)?.accept(&advert, now)?;
                    match acceptance {
                        Acceptance::Stored => {
                            if visited.insert(neighbor) {
                                queue.push_back((neighbor, advert.clone()));
                            }
                        }
                        Acceptance::AlreadyKnown => {}
                        Acceptance::Rejected(_) => {}
                    }
                }
            }
        }

        Ok(())
    }

    fn node(&self, id: &'static str) -> &TestNode {
        self.nodes.get(id).expect("node registered in mesh")
    }

    fn node_mut(&mut self, id: &'static str) -> Result<&mut TestNode, String> {
        self.nodes
            .get_mut(id)
            .ok_or_else(|| format!("node {id} not registered"))
    }
}

#[derive(Clone)]
struct AdvertRecord {
    provider_id: [u8; 32],
    known_capabilities: Vec<CapabilityType>,
}

struct TestNode {
    id: &'static str,
    known_capabilities: Vec<CapabilityType>,
    adverts: Vec<AdvertRecord>,
    seen: HashSet<[u8; 32]>,
    rejections: Vec<String>,
}

impl TestNode {
    fn new(id: &'static str, known_capabilities: &[CapabilityType]) -> Self {
        Self {
            id,
            known_capabilities: known_capabilities.to_vec(),
            adverts: Vec::new(),
            seen: HashSet::new(),
            rejections: Vec::new(),
        }
    }

    fn stored_count(&self) -> usize {
        self.adverts.len()
    }

    fn capabilities_for(&self, provider_id: &[u8; 32]) -> Option<Vec<CapabilityType>> {
        self.adverts
            .iter()
            .find(|record| &record.provider_id == provider_id)
            .map(|record| record.known_capabilities.clone())
    }

    fn rejection_reasons(&self) -> &[String] {
        &self.rejections
    }

    fn accept(&mut self, advert: &ProviderAdvertV1, now: u64) -> Result<Acceptance, String> {
        let fingerprint = advert_fingerprint(advert)?;
        if !self.seen.insert(fingerprint) {
            return Ok(Acceptance::AlreadyKnown);
        }

        advert
            .validate_with_body(now)
            .map_err(|err| format!("validation failed: {err}"))?;
        if advert.signature_strict {
            verify_signature(advert)?;
        }

        let mut known_caps = Vec::new();
        let mut unknown_caps = Vec::new();
        for capability in &advert.body.capabilities {
            if self.known_capabilities.contains(&capability.cap_type) {
                known_caps.push(capability.cap_type);
            } else {
                unknown_caps.push(capability.cap_type);
            }
        }

        if !unknown_caps.is_empty() && !advert.allow_unknown_capabilities {
            let message = format!(
                "unknown capabilities {:?} rejected on {}",
                unknown_caps, self.id
            );
            self.rejections.push(message.clone());
            return Ok(Acceptance::Rejected(message));
        }

        self.adverts.push(AdvertRecord {
            provider_id: advert.body.provider_id,
            known_capabilities: known_caps,
        });

        Ok(Acceptance::Stored)
    }
}

enum Acceptance {
    Stored,
    AlreadyKnown,
    Rejected(String),
}

fn advert_fingerprint(advert: &ProviderAdvertV1) -> Result<[u8; 32], String> {
    let bytes = norito::to_bytes(advert).map_err(|err| format!("encode advert: {err}"))?;
    Ok(blake3_hash(&bytes).into())
}

fn verify_signature(advert: &ProviderAdvertV1) -> Result<(), String> {
    match advert.signature.algorithm {
        SignatureAlgorithm::Ed25519 => {}
        other => {
            return Err(format!("unsupported signature algorithm: {other:?}"));
        }
    }

    if advert.signature.public_key.len() != PUBLIC_KEY_LENGTH {
        return Err(format!(
            "unexpected public key length: {}",
            advert.signature.public_key.len()
        ));
    }
    if advert.signature.signature.len() != SIGNATURE_LENGTH {
        return Err(format!(
            "unexpected signature length: {}",
            advert.signature.signature.len()
        ));
    }

    let mut pk = [0u8; PUBLIC_KEY_LENGTH];
    pk.copy_from_slice(&advert.signature.public_key);
    let verifying_key =
        VerifyingKey::from_bytes(&pk).map_err(|err| format!("invalid public key: {err}"))?;

    let mut sig_bytes = [0u8; SIGNATURE_LENGTH];
    sig_bytes.copy_from_slice(&advert.signature.signature);
    let signature = DalekSignature::from_bytes(&sig_bytes);

    let body_bytes =
        norito::to_bytes(&advert.body).map_err(|err| format!("encode advert body: {err}"))?;
    verifying_key
        .verify(&body_bytes, &signature)
        .map_err(|err| format!("signature verification failed: {err}"))
}
