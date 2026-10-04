//! Real signed-genesis factory selection without initialized custody or a readiness claim.
use super::*;
use iroha_config::parameters::actual::{MusubiPublicationInstallation, Queue as QueueConfig};
use iroha_core::{
    queue::Queue,
    smartcontracts::isi::sorafs_provider_admission::test_fixture::{
        ProviderAdmissionTestFixtureV1, key,
    },
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::ExposedPrivateKey;
use iroha_data_model::{
    account::{Account, AccountId},
    asset::AssetDefinitionId,
    isi::{Register, sorafs::InitializeSorafsProviderAdmissionV1},
    sorafs::provider_admission::governance::{
        InitialProviderAdmissionCouncilV1, InitialProviderAdmissionV1,
    },
};
use iroha_fs::{PrivateDirectory, PublishMode};
use iroha_primitives::numeric::Quantity;
use rcgen::{
    BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyUsagePurpose,
};
use sorafs_manifest::{
    provider_admission::{EndpointAttestationKind, ProviderAdmissionGenesisMaterialV1},
    provider_advert::{CapabilityType, EndpointKind, account_read::RegisteredAccountReadV1},
};
use std::{
    net::TcpListener,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture {
    _root: tempfile::TempDir,
    _occupied: TcpListener,
    chain: CertifiedTestChain,
    context: MusubiPublicationPrivateServiceContextV1,
    config: MusubiPublication,
    cache: Cache,
    originals: [GeneratedLocalProviderTransportV1; 3],
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix(".stock-publication-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        let directory = PrivateDirectory::open_or_create(root.path().join("keys")).unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let fixture = ProviderAdmissionTestFixtureV1::new_at(now);
        let ca_key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
        let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        ca_params.not_before = time::OffsetDateTime::now_utc() - time::Duration::days(1);
        ca_params.not_after = time::OffsetDateTime::now_utc() + time::Duration::days(1);
        let ca = ca_params.self_signed(&ca_key).unwrap();
        let issuer = Issuer::from_params(&ca_params, ca_key);
        let mut materials = Vec::new();
        for slot in 1..=3u8 {
            let mut material = ProviderAdmissionGenesisMaterialV1 {
                proposal: fixture.envelope.proposal.clone(),
                advert_body: fixture.envelope.advert_body.clone(),
                issued_at: now,
                retention_epoch: now + 3600,
            };
            material.proposal.provider_id = [slot; 32];
            material.advert_body.provider_id = [slot; 32];
            let provider_hex = hex::encode([slot; 32]);
            let host = format!("{}.{}.localhost", &provider_hex[..32], &provider_hex[32..]);
            material
                .proposal
                .capabilities
                .retain(|cap| cap.cap_type != CapabilityType::RegisteredAccountRead);
            material.proposal.capabilities.push(
                RegisteredAccountReadV1 {
                    https_host: host.clone(),
                    https_port: 8443,
                    ttl_secs: 60,
                    max_streams: 1,
                    rate_limit_bytes: 1024,
                    requests_per_minute: 60,
                }
                .to_capability()
                .unwrap(),
            );
            let tls_key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
            let mut params = CertificateParams::new(vec![host.clone()]).unwrap();
            params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
            params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
            params.not_before = ca_params.not_before;
            params.not_after = ca_params.not_after;
            let leaf = params.signed_by(&tls_key, &issuer).unwrap();
            material.proposal.endpoints.truncate(1);
            let endpoint = &mut material.proposal.endpoints[0];
            endpoint.endpoint.kind = EndpointKind::Torii;
            endpoint.endpoint.host_pattern = host;
            endpoint.attestation.kind = EndpointAttestationKind::Tls;
            endpoint.attestation.attested_at = now;
            endpoint.attestation.expires_at = now + 3600;
            endpoint.attestation.alpn_ids = vec!["http/1.1".into()];
            endpoint.attestation.report.clear();
            endpoint.attestation.leaf_certificate = leaf.der().as_ref().to_vec();
            endpoint.attestation.intermediate_certificates = vec![ca.der().as_ref().to_vec()];
            material.advert_body.capabilities = material.proposal.capabilities.clone();
            material.advert_body.endpoints = vec![endpoint.endpoint.clone()];
            material.validate().unwrap();
            if slot == 1 {
                directory
                    .write_atomic("leaf.der", leaf.der(), PublishMode::CreateNew)
                    .unwrap();
                directory
                    .write_atomic("key.der", &tls_key.serialize_der(), PublishMode::CreateNew)
                    .unwrap();
                directory
                    .write_atomic("root.der", ca.der(), PublishMode::CreateNew)
                    .unwrap();
            }
            materials.push((AccountId::new(key(slot).public_key().clone()), material));
        }
        let initializer = InitializeSorafsProviderAdmissionV1 {
            council: InitialProviderAdmissionCouncilV1 {
                policy_id: fixture.policy.policy_id,
                trusted_signers: fixture.policy.trusted_signers.clone(),
                signature_threshold: 1,
            },
            providers: materials
                .iter()
                .map(|(owner, material)| InitialProviderAdmissionV1 {
                    owner: owner.clone(),
                    material: norito::encode_canonical(material).unwrap(),
                })
                .collect(),
        };
        let mut chain_config = TestChainConfig::new(World::new(), now * 1000);
        chain_config.genesis_key = key(1);
        chain_config.genesis_instructions = vec![
            Register::account(Account::new(AccountId::new(key(2).public_key().clone()))).into(),
            Register::account(Account::new(AccountId::new(key(3).public_key().clone()))).into(),
            initializer.into(),
        ];
        let chain = CertifiedTestChain::start(chain_config).unwrap();
        let (events, _) = tokio::sync::broadcast::channel(16);
        let queue = Arc::new(Queue::from_config(QueueConfig::default(), events));
        let node = sorafs_node::NodeHandle::new(
            sorafs_node::config::StorageConfig::builder()
                .data_dir(root.path().join("node-storage"))
                .build(),
        );
        let context = MusubiPublicationPrivateServiceContextV1::new(
            chain.network_id(),
            Arc::clone(chain.state()),
            queue,
            node,
        );
        let originals = materials
            .iter()
            .map(|(owner, material)| {
                GeneratedLocalProviderTransportV1::select(
                    chain.network_id(),
                    chain.state().view().chain_id().as_str(),
                    ProviderId::new(material.proposal.provider_id),
                    owner,
                    material,
                )
                .unwrap()
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let cache = Arc::new(tokio::sync::RwLock::new(
            iroha_torii::sorafs::ProviderAdvertCache::new(
                [],
                Arc::new(iroha_torii::sorafs::AdmissionRegistry::from_state(
                    Arc::clone(chain.state()),
                )),
            ),
        ));
        let broker = key(1);
        directory
            .write_atomic(
                "broker.key",
                format!(
                    "{}\n",
                    ExposedPrivateKey(broker.private_key().clone())
                        .try_to_multihash_string()
                        .unwrap()
                )
                .as_bytes(),
                PublishMode::CreateNew,
            )
            .unwrap();
        directory
            .write_atomic(
                "pin.key",
                format!(
                    "{}\n",
                    ExposedPrivateKey(broker.private_key().clone())
                        .try_to_multihash_string()
                        .unwrap()
                )
                .as_bytes(),
                PublishMode::CreateNew,
            )
            .unwrap();
        let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let config = MusubiPublication {
            private_tls_bind: occupied.local_addr().unwrap(),
            custody_root: root.path().join("uninitialized-custody"),
            installation: Some(MusubiPublicationInstallation {
                network_id: chain.network_id(),
                seed_provider: ProviderId::new([1; 32]),
                ingress_broker: AccountId::new(broker.public_key().clone()),
                pin_session: [0x37; 32],
                broker_key_file: directory.path().join("broker.key"),
                pin_key_file: directory.path().join("pin.key"),
                tls_server_name: materials[0].1.proposal.endpoints[0]
                    .endpoint
                    .host_pattern
                    .clone(),
                tls_certificate_file: directory.path().join("leaf.der"),
                tls_private_key_file: directory.path().join("key.der"),
                tls_root_certificate_file: directory.path().join("root.der"),
                readback_request_timeout_ms: 30_000,
                pin_authorization_window_ms: 600_000,
                pin_max_check_rounds: 16,
                pin_fee_asset: AssetDefinitionId::parse_address_literal(
                    "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
                )
                .unwrap(),
                pin_per_transaction_fee_limit: Quantity::from(1u32),
                pin_total_fee_limit: Quantity::from(64u32),
            }),
            ..MusubiPublication::default()
        };
        Self {
            _root: root,
            _occupied: occupied,
            chain,
            context,
            config,
            cache,
            originals,
        }
    }
}
struct Supplied(Arc<AtomicUsize>);
impl MusubiPublicationPrivateServiceFactoryV1 for Supplied {
    fn build(
        self: Box<Self>,
        _: MusubiPublicationPrivateServiceContextV1,
    ) -> Result<MusubiPublicationPrivateDeploymentV1, Error> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(Error::Unqualified)
    }
}
#[test]
fn standard_and_fast_selection_are_inert_and_explicit_factory_is_preserved() {
    let called = Arc::new(AtomicUsize::new(0));
    assert!(
        select_factory(&MusubiPublication::default(), None, None, None, false)
            .unwrap()
            .is_none()
    );
    let selected = select_factory(
        &MusubiPublication::default(),
        None,
        None,
        Some(Box::new(Supplied(called.clone()))),
        false,
    )
    .unwrap();
    assert!(selected.is_some());
    assert!(
        select_factory(
            &MusubiPublication::default(),
            None,
            None,
            Some(Box::new(Supplied(called.clone()))),
            true
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(called.load(Ordering::SeqCst), 0);
}
#[test]
fn actual_h1_intent_selects_factory_but_missing_original_custody_is_never_initialized() {
    let fixture = Fixture::new();
    assert_eq!(fixture.chain.height(), 1);
    let factory = select_factory(
        &fixture.config,
        Some(&fixture.context),
        Some(fixture.cache.clone()),
        None,
        false,
    )
    .unwrap()
    .unwrap();
    // The configured port is already occupied: original selection must not bind it.
    assert!(!fixture.config.custody_root.exists());
    assert!(factory.build(fixture.context).is_err());
    assert!(!fixture.config.custody_root.exists());
    assert_eq!(fixture.chain.height(), 1);
}
#[test]
fn configured_factory_rejects_ambiguity_foreign_tls_and_missing_dependencies_without_repair() {
    let fixture = Fixture::new();
    let calls = Arc::new(AtomicUsize::new(0));
    assert!(
        select_factory(
            &fixture.config,
            Some(&fixture.context),
            Some(fixture.cache.clone()),
            Some(Box::new(Supplied(calls.clone()))),
            false
        )
        .is_err()
    );
    assert!(select_factory(&fixture.config, None, None, None, false).is_err());
    assert!(
        select_factory(
            &fixture.config,
            None,
            None,
            Some(Box::new(Supplied(calls.clone()))),
            true
        )
        .unwrap()
        .is_none()
    );
    let mutations: [fn(&mut MusubiPublication); 5] = [
        |value| value.private_tls_bind.set_port(8443),
        |value| {
            value
                .installation
                .as_mut()
                .unwrap()
                .tls_server_name
                .push_str(".foreign")
        },
        |value| value.installation.as_mut().unwrap().seed_provider = ProviderId::new([9; 32]),
        |value| {
            value.installation.as_mut().unwrap().tls_certificate_file = value
                .installation
                .as_ref()
                .unwrap()
                .tls_root_certificate_file
                .clone()
        },
        |value| {
            value
                .installation
                .as_mut()
                .unwrap()
                .broker_key_file
                .set_file_name("absent.key")
        },
    ];
    for mutate in mutations {
        let mut config = fixture.config.clone();
        mutate(&mut config);
        assert!(
            select_factory(
                &config,
                Some(&fixture.context),
                Some(fixture.cache.clone()),
                None,
                false
            )
            .is_err()
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(!fixture.config.custody_root.exists());
}
#[test]
fn cold_native_callback_rejects_unknown_scope_before_source_and_h1_before_current_authority() {
    let fixture = Fixture::new();
    let budget = fixture.chain.state().ivm_execution_budget();
    let layout = Layout::array::<u8>(3 * 2 * 1024 * 1024).unwrap();
    let charge = budget
        .try_reserve(layout)
        .unwrap()
        .try_split(layout)
        .unwrap();
    let callback = discovery::prepare(
        Arc::clone(fixture.chain.state()),
        fixture.cache.clone(),
        fixture.originals.clone(),
        Duration::from_secs(30),
        charge,
    )
    .unwrap();
    assert!(matches!(
        callback(ProviderId::new([9; 32])),
        Err(iroha_storage_client::musubi_archive_fetch::MusubiArchiveDiscoveryErrorV1::Rejected)
    ));
    for original in &fixture.originals {
        assert!(matches!(callback(original.provider_id()), Err(iroha_storage_client::musubi_archive_fetch::MusubiArchiveDiscoveryErrorV1::Unavailable)));
    }
    assert_eq!(fixture.chain.height(), 1);
    assert!(!fixture.config.custody_root.exists());
}
