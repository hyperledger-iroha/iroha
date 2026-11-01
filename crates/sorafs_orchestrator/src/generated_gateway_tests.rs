//! Genuine signed provider and custody proofs over explicitly synthetic World preimages.
//!
//! The real finality verifier joins original generated TLS intent to current account-read
//! authority. This component fixture executes no World transition and qualifies no deployment.

use super::{OrchestratorConfig, fetch_via_gateway_with_context};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model::{
    account::AccountId,
    sorafs::{
        capacity::ProviderId,
        provider_admission::{
            ProviderAdmissionCouncilPolicyV1,
            discovery::{ProviderAdmissionHeadProofV1, ProviderDiscoveryProofV1},
            history::{
                AdmissionHistoryPathV1, AdmissionHistoryRecordV1, GenesisAdmissionOriginV1,
                admission_history_path, encode_history_value,
            },
        },
        stream_token_custody::{
            StreamTokenCustodyControlRecordV1, history::StreamTokenCustodyControlIndexV1,
        },
    },
    sumeragi_finality::{
        VerifiedSumeragiBlock, WorldStateElementKindV1, WorldStateSnapshotEntryV1,
        WorldStateSnapshotV1, test_fixtures::NativeFinalityFixture, world_state_value_hash_v1,
    },
};
use rcgen::{
    BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyUsagePurpose,
};
use sorafs_car::{
    CarBuildPlan, CarWriter,
    gateway::{
        AuthenticatedGeneratedLocalProviderTransportV1, GatewayFetchConfig, GatewayFetchContext,
        GatewayProviderInput, GeneratedLocalProviderTransportV1,
    },
};
use sorafs_manifest::{
    CouncilSignature, ManifestBuilder, ManifestV1, StreamTokenBodyV1, StreamTokenV1,
    deal::XorQuantity,
    provider_admission::{
        EndpointAdmissionV1, EndpointAttestationKind, EndpointAttestationV1,
        ProviderAdmissionGenesisMaterialV1, ProviderAdmissionProposalV1, ProviderVrfPublicKeyV1,
    },
    provider_advert::{
        AdvertEndpoint, AvailabilityTier, CapabilityTlv, CapabilityType, EndpointKind,
        PathDiversityPolicy, ProviderAdvertBodyV1, ProviderAdvertV1, QosHints, RendezvousTopic,
        StakePointer, account_read::RegisteredAccountReadV1,
    },
    signer::{custody::*, custody_control::*, protocol::*},
};
use std::{
    collections::BTreeMap,
    net::{Ipv4Addr, TcpListener},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::oneshot,
};
use tokio_rustls::{
    TlsAcceptor,
    rustls::{
        self,
        pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
    },
};

const WAIT: Duration = Duration::from_secs(5);
fn provider() -> ProviderId {
    ProviderId::new([0x11; 32])
}
fn host() -> String {
    format!("{}.{}.localhost", "11".repeat(16), "11".repeat(16))
}
fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}
fn raw(key: &KeyPair) -> [u8; 32] {
    key.public_key().to_bytes().1.try_into().unwrap()
}
fn owner() -> AccountId {
    AccountId::new(key(1).public_key().clone())
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .try_into()
        .unwrap()
}
struct Identity {
    root: Vec<u8>,
    leaf: Vec<u8>,
    key: Vec<u8>,
}
impl Identity {
    fn new(name: &str) -> Self {
        let root_key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
        let mut root_params = CertificateParams::new(Vec::<String>::new()).unwrap();
        root_params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        root_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        let root = root_params.self_signed(&root_key).unwrap();
        let issuer = Issuer::from_params(&root_params, root_key);
        let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
        let mut params = CertificateParams::new(vec![name.to_owned()]).unwrap();
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let leaf = params.signed_by(&key, &issuer).unwrap();
        Self {
            root: root.der().to_vec(),
            leaf: leaf.der().to_vec(),
            key: key.serialize_der(),
        }
    }
    fn server(&self) -> Arc<rustls::ServerConfig> {
        let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(self.leaf.clone())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key.clone())),
        )
        .unwrap();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Arc::new(config)
    }
}
fn material(identity: &Identity, port: u16, now: u64) -> ProviderAdmissionGenesisMaterialV1 {
    let read = RegisteredAccountReadV1 {
        https_host: host(),
        https_port: port,
        ttl_secs: 60,
        max_streams: 1,
        rate_limit_bytes: 8 * 1024 * 1024,
        requests_per_minute: 60,
    };
    let capabilities = vec![
        CapabilityTlv {
            cap_type: CapabilityType::ToriiGateway,
            payload: vec![],
        },
        read.to_capability().unwrap(),
    ];
    let endpoint = AdvertEndpoint {
        kind: EndpointKind::Torii,
        host_pattern: host(),
        metadata: vec![],
    };
    let stake = StakePointer {
        pool_id: [2; 32],
        stake_amount: XorQuantity::try_from_micro(1).unwrap(),
    };
    let vrf = KeyPair::from_seed(vec![3; 32], Algorithm::BlsNormal);
    let advert = KeyPair::from_seed(vec![4; 32], Algorithm::Ed25519);
    ProviderAdmissionGenesisMaterialV1 {
        proposal: ProviderAdmissionProposalV1 {
            version: 1,
            provider_id: *provider().as_bytes(),
            profile_id: "sorafs.sf1@1.0.0".into(),
            profile_aliases: Some(
                sorafs_manifest::chunker_registry::default_descriptor()
                    .aliases
                    .iter()
                    .map(|alias| (*alias).to_owned())
                    .collect(),
            ),
            stake: stake.clone(),
            capabilities: capabilities.clone(),
            endpoints: vec![EndpointAdmissionV1 {
                endpoint: endpoint.clone(),
                attestation: EndpointAttestationV1 {
                    version: 1,
                    kind: EndpointAttestationKind::Tls,
                    attested_at: (now - 10_000) / 1000,
                    expires_at: now / 1000 + 3600,
                    leaf_certificate: identity.leaf.clone(),
                    intermediate_certificates: vec![identity.root.clone()],
                    alpn_ids: vec!["http/1.1".into()],
                    report: vec![],
                },
            }],
            advert_key: advert.public_key().to_bytes().1.try_into().unwrap(),
            por_vrf_key: ProviderVrfPublicKeyV1::BlsNormal(
                vrf.public_key().to_bytes().1.try_into().unwrap(),
            ),
            jurisdiction_code: "ZZ".into(),
            contact_uri: None,
            stream_budget: None,
            transport_hints: None,
        },
        advert_body: ProviderAdvertBodyV1 {
            provider_id: *provider().as_bytes(),
            profile_id: "sorafs.sf1@1.0.0".into(),
            profile_aliases: Some(
                sorafs_manifest::chunker_registry::default_descriptor()
                    .aliases
                    .iter()
                    .map(|alias| (*alias).to_owned())
                    .collect(),
            ),
            stake,
            qos: QosHints {
                availability: AvailabilityTier::Hot,
                max_retrieval_latency_ms: 1,
                max_concurrent_streams: 1,
            },
            capabilities,
            endpoints: vec![endpoint],
            rendezvous_topics: vec![RendezvousTopic {
                topic: "sorafs.sf1.primary".into(),
                region: "local".into(),
            }],
            path_policy: PathDiversityPolicy {
                min_guard_weight: 1,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: None,
            stream_budget: None,
            transport_hints: None,
        },
        issued_at: (now - 10_000) / 1000,
        retention_epoch: now / 1000 + 3600,
    }
}
fn row<K: norito::codec::Encode, V: norito::codec::Encode>(
    field: &str,
    key: &K,
    value: &V,
) -> WorldStateSnapshotEntryV1 {
    WorldStateSnapshotEntryV1 {
        field_id: field.into(),
        kind: WorldStateElementKindV1::Table,
        key_hash: Some(world_state_value_hash_v1(key).unwrap()),
        value_hash: world_state_value_hash_v1(value).unwrap(),
    }
}
fn rebuild(proof: &mut ProviderDiscoveryProofV1, provider: ProviderId) {
    let mut entries = vec![row("world.provider_owners", &provider, &proof.owner)];
    for (subject, original) in [(None, &proof.council), (Some(provider), &proof.provider)] {
        let record = AdmissionHistoryRecordV1::decode_frame(&original.head).unwrap();
        entries.push(row(
            "world.smart_contract_state",
            &admission_history_path(subject, AdmissionHistoryPathV1::Head),
            &original.head,
        ));
        entries.push(row(
            "world.smart_contract_state",
            &admission_history_path(subject, AdmissionHistoryPathV1::Revision(record.revision)),
            &original.head,
        ));
        if let Some(previous) = &original.predecessor {
            entries.push(row(
                "world.smart_contract_state",
                &admission_history_path(
                    subject,
                    AdmissionHistoryPathV1::Revision(record.revision - 1),
                ),
                previous,
            ));
        }
    }
    if let Some(token) = &proof.stream_token {
        use iroha_data_model::sorafs::stream_token_custody::history::{
            StreamTokenCustodyControlIndexV1, head_key, height_key, record_key,
        };
        let index: StreamTokenCustodyControlIndexV1 =
            norito::decode_canonical(&token.head).unwrap();
        entries.push(row(
            "world.smart_contract_state",
            &head_key(provider),
            &token.head,
        ));
        entries.push(row(
            "world.smart_contract_state",
            &record_key(provider, index.revision),
            &token.record,
        ));
        entries.push(row(
            "world.smart_contract_state",
            &height_key(provider, index.height, index.ordinal),
            &token.head,
        ));
    }
    entries
        .sort_by(|a, b| (&a.field_id, a.kind, a.key_hash).cmp(&(&b.field_id, b.kind, b.key_hash)));
    proof.world.entries = entries;
}
fn certify(
    native: &mut NativeFinalityFixture,
    proof: &ProviderDiscoveryProofV1,
    now: u64,
) -> VerifiedSumeragiBlock {
    let mut header = native.next_header();
    header.creation_time_ms = header.creation_time_ms.max(now);
    let block = native.block_with_submitted_work(header);
    let cert = native.certify_with_world_root(block, proof.world.root().unwrap());
    native.verifier().verify_retained_decision(&cert).unwrap()
}

struct NativeProvider {
    native: NativeFinalityFixture,
    proof: ProviderDiscoveryProofV1,
    block: VerifiedSumeragiBlock,
    selected: GeneratedLocalProviderTransportV1,
    token_key: KeyPair,
    now: u64,
}
impl NativeProvider {
    fn new(material: &ProviderAdmissionGenesisMaterialV1, now: u64) -> Self {
        let genesis_time = now - 5_000;
        let mut native =
            NativeFinalityFixture::start_at("generated-gateway-synthetic-world", genesis_time);
        assert_eq!(native.genesis().header().creation_time_ms, genesis_time);
        let genesis = native
            .verifier()
            .verify_retained_decision(native.genesis_proof())
            .unwrap();
        assert_eq!(genesis.header().creation_time_ms, genesis_time);
        let mut altered_clock = native.genesis_proof().clone();
        altered_clock.block_header.creation_time_ms += 1;
        assert!(
            native
                .verifier()
                .verify_retained_decision(&altered_clock)
                .is_err()
        );
        assert_eq!(
            native
                .genesis()
                .external_transactions()
                .next()
                .unwrap()
                .creation_time(),
            Duration::from_millis(genesis_time - 1)
        );
        let provider = provider();
        let policy = ProviderAdmissionCouncilPolicyV1 {
            network_id: *native.network_id().as_bytes(),
            policy_id: [0xC1; 32],
            version: 1,
            revision: 1,
            predecessor_policy_digest: None,
            trusted_signers: vec![raw(&key(0x44))],
            signature_threshold: 1,
            paused: false,
        };
        let envelope = material
            .project(
                policy.network_id,
                policy.policy_id,
                policy.canonical_digest().unwrap(),
            )
            .unwrap();
        policy.validate().expect("fixture council policy");
        let admission = sorafs_manifest::AdmissionRecord::from_genesis_material(
            material,
            policy.network_id,
            policy.policy_id,
            policy.canonical_digest().unwrap(),
        )
        .expect("fixture original genesis material");
        let head = |material, owner| AdmissionHistoryRecordV1 {
            network_id: policy.network_id,
            // This history is a synthetic World preimage, certified by the genuine fixture quorum.
            genesis_origin: Some(GenesisAdmissionOriginV1 {
                entrypoint_index: 0,
                instruction_digest: [7; 32],
            }),
            revision: 1,
            predecessor: None,
            height: 1,
            recorded_at_unix_ms: genesis_time,
            owner,
            revoked: false,
            material,
        };
        let mut advert: ProviderAdvertV1 = norito::decode_from_bytes(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/sorafs_manifest/provider_admission/advert_v1.to"
        )))
        .unwrap();
        advert.network_id = policy.network_id;
        advert.body = material.advert_body.clone();
        advert.issued_at = now / 1000;
        advert.expires_at = now / 1000 + 60;
        advert.signature_strict = true;
        advert.signature.public_key = raw(&key(4)).to_vec();
        advert.signature.signature = Signature::new(
            key(4).private_key(),
            &advert.signature_payload_bytes().unwrap(),
        )
        .payload()
        .to_vec();
        advert
            .validate_with_body(now / 1000)
            .expect("fresh fixture advert");
        advert.verify_signature().expect("fixture advert signature");
        sorafs_manifest::provider_admission::verify_advert_against_record(&advert, &admission)
            .expect("fixture advert matches original admission");
        let mut proof = ProviderDiscoveryProofV1 {
            world: WorldStateSnapshotV1 {
                schema_hash: Hash::new(b"synthetic provider schema"),
                entries: vec![],
            },
            council: ProviderAdmissionHeadProofV1 {
                head: encode_history_value(&head(norito::encode_canonical(&policy).unwrap(), None))
                    .unwrap(),
                predecessor: None,
            },
            provider: ProviderAdmissionHeadProofV1 {
                head: encode_history_value(&head(
                    norito::encode_canonical(&envelope).unwrap(),
                    Some(owner()),
                ))
                .unwrap(),
                predecessor: None,
            },
            owner: owner(),
            advert: norito::encode_canonical(&advert).unwrap(),
            stream_token: None,
        };
        rebuild(&mut proof, provider);
        let approval = certify(&mut native, &proof, now - 1_000);
        assert_eq!(approval.header().creation_time_ms, now - 1_000);
        let signer = key(0x61);
        let attester = key(0x62);
        let binding = SignerCustodyBindingV1 {
            chain_id: native.chain_id().into(),
            network_id: *native.network_id().as_bytes(),
            runtime_handle: "software://storage/token".into(),
            key_handle: "software://storage/token/key".into(),
            service_id: "storage-token-signer".into(),
            administrator_id: "storage-token-admin".into(),
            role: SignerRoleV1::StreamToken,
            purpose: SignerPurposeBindingV1::StreamToken {
                provider_id: *provider.as_bytes(),
            },
            algorithm: SignerKeyAlgorithmV1::Ed25519,
            public_key: signer.public_key().clone(),
            key_revision: 7,
            policy_revision: 1,
            policy_digest: [9; 32],
        };
        let authority = SignerCustodyAuthorityV1 {
            service_id: "independent-attester".into(),
            administrator_id: "independent-admin".into(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [10; 32],
        };
        let approved_anchor = SignerCustodyAnchorV1 {
            height: approval.height(),
            block_hash: *approval.header().hash().as_ref(),
            state_digest: [11; 32],
        };
        let statement = SignerCustodyStatementV1 {
            magic: SIGNER_CUSTODY_MAGIC_V1,
            version: SIGNER_CUSTODY_VERSION_V1,
            binding: binding.clone(),
            authority: authority.clone(),
            anchor: approved_anchor,
            sequence: 1,
            predecessor_digest: [0; 32],
            issued_at_unix_ms: now,
            expires_at_unix_ms: now + 60_000,
            evidence_digest: [12; 32],
            revoked: false,
        };
        let signature = Signature::new(
            attester.private_key(),
            &statement.signing_payload().unwrap(),
        );
        let enrollment = SignerCustodyRecordV1 {
            statement,
            attestation: signature.payload().try_into().unwrap(),
        };
        let digest = enrollment.canonical_digest().unwrap();
        let control = SignerCustodyControlStateV1 {
            policy: SignerCustodyPolicyV1 {
                binding,
                attester_authority: authority,
                attester_public_key: attester.public_key().clone(),
                active_from_unix_ms: now - 1_000,
                active_until_unix_ms: now + 120_000,
                max_validity_ms: 60_000,
                max_anchor_age_ms: 60_000,
            },
            next_sequence: 2,
            predecessor_digest: digest,
            active_head: Some(SignerCustodyActiveHeadV1 {
                record_digest: digest,
                sequence: 1,
                approved_anchor,
                key_revision: 7,
                policy_revision: 1,
                policy_digest: [9; 32],
            }),
            signer_revoked: false,
            attester_revoked: false,
        };
        let record = StreamTokenCustodyControlRecordV1 {
            provider_id: provider,
            revision: 1,
            predecessor_digest: [0; 32],
            request_digest: [13; 32],
            execution_height: 3,
            ordinal: 0,
            recorded_at_unix_ms: now,
            authority: proof.owner.clone(),
            control_state: norito::encode_canonical(&control).unwrap(),
            active_enrollment: Some(norito::encode_canonical(&enrollment).unwrap()),
        };
        install(&mut proof, &record);
        rebuild(&mut proof, provider);
        let block = certify(&mut native, &proof, now);
        assert_eq!(block.header().creation_time_ms, now);
        let selected = GeneratedLocalProviderTransportV1::select(
            native.network_id(),
            native.chain_id(),
            provider,
            &proof.owner,
            material,
        )
        .unwrap();
        Self {
            native,
            proof,
            block,
            selected,
            token_key: signer,
            now,
        }
    }
    fn authenticate(&self) -> AuthenticatedGeneratedLocalProviderTransportV1 {
        let current = self
            .proof
            .verify_account_read(
                self.native.chain_id(),
                self.native.network_id(),
                provider(),
                self.proof.world.schema_hash,
                &self.block,
                self.now,
            )
            .unwrap();
        assert_eq!(current.token_public_key(), self.token_key.public_key());
        self.selected.authenticate_current(&current).unwrap()
    }
}
fn install(proof: &mut ProviderDiscoveryProofV1, record: &StreamTokenCustodyControlRecordV1) {
    let index = StreamTokenCustodyControlIndexV1 {
        revision: record.revision,
        digest: record.canonical_digest().unwrap(),
        height: record.execution_height,
        ordinal: record.ordinal,
    };
    proof.stream_token = Some(
        iroha_data_model::sorafs::stream_token_custody::proof::StreamTokenCustodyRecordProofV1 {
            head: norito::encode_canonical(&index).unwrap(),
            record: norito::encode_canonical(record).unwrap(),
        },
    );
}

/// A finite local TLS server whose shutdown does not depend on planned requests arriving.
struct Server {
    port: u16,
    stop: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<(usize, usize)>>,
}
impl Server {
    fn start(identity: &Identity, responses: BTreeMap<String, Vec<u8>>) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        let acceptor = TlsAcceptor::from(identity.server());
        let (stop, mut stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut responses = responses;
            let mut requested_count = 0;
            loop {
                let accepted = tokio::select! {
                    biased;
                    _ = &mut stopped => break,
                    accepted = listener.accept() => accepted,
                };
                let (stream, _) = accepted.unwrap();
                let exchange = async {
                    let mut tls = acceptor.accept(stream).await.unwrap();
                    let mut request = Vec::new();
                    loop {
                        let mut buffer = [0; 1024];
                        let count = tls.read(&mut buffer).await.unwrap();
                        assert_ne!(count, 0, "complete HTTP request required");
                        request.extend_from_slice(&buffer[..count]);
                        assert!(request.len() <= 16 * 1024, "bounded fixture request");
                        if request.windows(4).any(|window| window == b"\r\n\r\n") {
                            break;
                        }
                    }
                    let request = std::str::from_utf8(&request).unwrap();
                    let path = request
                        .lines()
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap();
                    let body = responses
                        .remove(path)
                        .expect("exact planned manifest/chunk route");
                    // Count matched requests before cancellable response I/O. The
                    // caller verifies every body and the complete CAR independently;
                    // HTTP completion need not wait for TLS close_notify to finish.
                    requested_count += 1;
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    tls.write_all(header.as_bytes()).await.unwrap();
                    tls.write_all(&body).await.unwrap();
                    tls.shutdown().await.unwrap();
                };
                tokio::select! {
                    biased;
                    _ = &mut stopped => break,
                    result = tokio::time::timeout(WAIT, exchange) => result.expect("bounded TLS request"),
                }
            }
            (requested_count, responses.len())
        });
        Self {
            port,
            stop: Some(stop),
            task: Some(task),
        }
    }
    async fn finish(mut self) -> (usize, usize) {
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(WAIT, self.task.take().unwrap())
            .await
            .expect("bounded server shutdown")
            .expect("TLS worker")
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        // Closing the sender also interrupts accept/handshake/read on a failed test path.
        self.stop.take();
    }
}

fn payload_manifest(payload: &[u8], plan: &CarBuildPlan) -> ManifestV1 {
    let stats = CarWriter::new(plan, payload)
        .unwrap()
        .write_to(std::io::sink())
        .unwrap();
    let mut manifest = ManifestBuilder::new()
        .root_cid(stats.root_cids[0].clone())
        .dag_codec(sorafs_manifest::DagCodecId(stats.dag_codec))
        .chunking_from_registry(sorafs_manifest::chunker_registry::default_descriptor().id)
        .chunk_digest_sha3_256(sorafs_car::compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(sorafs_car::compute_por_root(payload, plan).unwrap())
        .content_length(plan.content_length)
        .car_digest(*stats.car_archive_digest.as_bytes())
        .car_size(stats.car_size)
        .pin_policy(sorafs_manifest::PinPolicy {
            retention_epoch: 100,
            ..Default::default()
        })
        .build()
        .unwrap();
    let digest = manifest.digest().unwrap();
    manifest
        .governance
        .council_signatures
        .push(CouncilSignature {
            signer: raw(&key(0x44)),
            signature: Signature::new(key(0x44).private_key(), digest.as_bytes())
                .payload()
                .to_vec(),
        });
    manifest
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fetch_via_gateway_streams_chunks() {
    let (plan, payload) = CarBuildPlan::from_files(
        (0..3)
            .map(|file| sorafs_car::FileEntry {
                path: vec![format!("chunk-{file}.bin")],
                data: (0..8 * 1024)
                    .map(|index| ((index + file) % 251) as u8)
                    .collect(),
            })
            .collect(),
    )
    .unwrap();
    assert!(
        plan.chunks.len() > 1,
        "exercise actual multi-chunk streaming"
    );
    let manifest = payload_manifest(&payload, &plan);
    let manifest_id = hex::encode(manifest.digest().unwrap().as_bytes());
    assert_ne!(manifest_id, hex::encode(plan.payload_digest.as_bytes()));
    let mut response = norito::json::Map::new();
    response.insert(
        "manifest_b64".into(),
        norito::json::Value::from(STANDARD.encode(manifest.encode().unwrap())),
    );
    response.insert(
        "manifest_digest_hex".into(),
        norito::json::Value::from(manifest_id.clone()),
    );
    response.insert(
        "payload_digest_hex".into(),
        norito::json::Value::from(hex::encode(plan.payload_digest.as_bytes())),
    );
    response.insert(
        "content_length".into(),
        norito::json::Value::from(plan.content_length),
    );
    response.insert(
        "chunk_count".into(),
        norito::json::Value::from(plan.chunks.len() as u64),
    );
    response.insert(
        "chunk_profile_handle".into(),
        norito::json::Value::from("sorafs.sf1@1.0.0"),
    );
    let mut responses = BTreeMap::new();
    responses.insert(
        format!("/v1/sorafs/storage/manifest/{manifest_id}"),
        norito::json::to_vec(&norito::json::Value::Object(response)).unwrap(),
    );
    for spec in plan.try_chunk_fetch_specs().unwrap() {
        responses.insert(
            format!(
                "/v1/sorafs/storage/chunk/{manifest_id}/{}",
                hex::encode(spec.digest)
            ),
            payload[spec.offset as usize..spec.offset as usize + spec.length as usize].to_vec(),
        );
    }
    let expected_requests = responses.len();
    let identity = Identity::new(&host());
    let server = Server::start(&identity, responses);
    let now = now_ms();
    let native = NativeProvider::new(&material(&identity, server.port, now), now);
    let selected = native.authenticate();
    let token = StreamTokenV1::sign(
        StreamTokenBodyV1 {
            token_id: "generated-native-gateway-test".into(),
            manifest_cid: manifest.root_cid.clone(),
            provider_id: *provider().as_bytes(),
            profile_handle: "sorafs.sf1@1.0.0".into(),
            max_streams: 1,
            ttl_epoch: now / 1000 + 60,
            rate_limit_bytes: 8 * 1024 * 1024,
            issued_at: now / 1000,
            requests_per_minute: 60,
            token_pk_version: 7,
        },
        &ed25519_dalek::SigningKey::from_bytes(
            &native
                .token_key
                .private_key()
                .to_bytes()
                .1
                .try_into()
                .unwrap(),
        ),
    )
    .unwrap();
    let context = GatewayFetchContext::new_with_generated_local_transport(
        GatewayFetchConfig {
            manifest_id_hex: manifest_id,
            chunker_handle: "sorafs.sf1@1.0.0".into(),
            manifest_envelope_b64: None,
            client_id: Some("native-fixture".into()),
            expected_manifest_cid_hex: Some(hex::encode(&manifest.root_cid)),
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
        },
        GatewayProviderInput {
            name: "alpha".into(),
            provider_id_hex: hex::encode(provider().as_bytes()),
            gateway_public_key_hex: hex::encode(raw(&native.token_key)),
            base_url: selected.base_url().as_str().into(),
            stream_token_b64: STANDARD.encode(norito::encode_canonical(&token).unwrap()),
            privacy_events_url: None,
        },
        WAIT,
        WAIT,
        &selected,
    )
    .unwrap();
    // The context-taking facade must retain every guard before any HTTP request.
    let mut disabled = OrchestratorConfig::default();
    disabled.fetch.verify_digests = false;
    assert!(matches!(
        fetch_via_gateway_with_context(disabled, &plan, context.clone(), None, None).await,
        Err(super::GatewayOrchestratorError::IntegrityVerificationDisabled)
    ));
    let mut unbounded = OrchestratorConfig::default();
    unbounded.fetch.session_timeout = Duration::ZERO;
    assert!(matches!(
        fetch_via_gateway_with_context(unbounded, &plan, context.clone(), None, None).await,
        Err(super::GatewayOrchestratorError::Orchestrator(
            super::OrchestratorError::UnsafeResourceConfig(_)
        ))
    ));
    let mut too_small = OrchestratorConfig::default();
    too_small.fetch.max_payload_bytes = plan.content_length - 1;
    assert!(
        fetch_via_gateway_with_context(too_small, &plan, context.clone(), None, None)
            .await
            .is_err()
    );
    let mut config = OrchestratorConfig::default();
    config.fetch.per_chunk_retry_limit = Some(2);
    let session = fetch_via_gateway_with_context(config, &plan, context, None, Some(2))
        .await
        .unwrap();
    assert_eq!(
        session.outcome.chunks.len(),
        plan.try_chunk_fetch_specs().unwrap().len()
    );
    assert_eq!(session.outcome.chunks.concat(), payload);
    assert!(session.car_verification.is_some());
    assert_eq!(server.finish().await, (expected_requests, 0));
}

#[tokio::test]
async fn generated_tls_server_shutdown_is_bounded_without_consuming_responses() {
    let identity = Identity::new(&host());
    let server = Server::start(
        &identity,
        BTreeMap::from([("/never-requested".into(), vec![1])]),
    );
    assert_eq!(server.finish().await, (0, 1));
}

#[test]
fn current_native_join_rejects_altered_custody_without_http() {
    let identity = Identity::new(&host());
    let now = now_ms();
    let mut native = NativeProvider::new(&material(&identity, 443, now), now);
    native.authenticate();
    let original = native.proof.stream_token.as_mut().unwrap();
    let mut record: StreamTokenCustodyControlRecordV1 =
        norito::decode_canonical(&original.record).unwrap();
    record.active_enrollment.as_mut().unwrap()[0] ^= 1;
    original.record = norito::encode_canonical(&record).unwrap();
    assert!(
        native
            .proof
            .verify_account_read(
                native.native.chain_id(),
                native.native.network_id(),
                provider(),
                native.proof.world.schema_hash,
                &native.block,
                now
            )
            .is_err()
    );
}
