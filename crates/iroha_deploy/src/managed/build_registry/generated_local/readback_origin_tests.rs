//! Cold origin resolution over real generated genesis, paid custody and native account proofs.
//! These component controls do not claim running-network discovery or TLS readback qualification.
use super::*;
use crate::managed::{
    native_operation::test_support::native_fixture::{NativeFixture, quote_instructions},
    service_policies::GeneratedServicePolicies,
};
use iroha_core::state::{AllocationBudget, State};
use iroha_data_model::{
    asset::AssetDefinitionId,
    isi::{InstructionBox, Log},
    sorafs::provider_admission::discovery::{
        ProviderDiscoveryProofRefV1, ProviderDiscoveryProofV1,
    },
    transaction::FeePaymentIntent,
};
use iroha_primitives::numeric::Quantity;
use iroha_storage_client::musubi_archive_fetch::MusubiArchiveProviderDiscoveryV1;
use iroha_wallet::operations::BoundedTransactionOptions;
use sorafs_manifest::provider_advert::{CapabilityType, account_read::RegisteredAccountReadV1};
use std::{
    collections::BTreeMap,
    net::{Ipv4Addr, TcpListener},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

fn discovery_fixture() -> (
    tempfile::TempDir,
    PreparedLocalnet,
    ServiceAuthority,
    NativeFixture,
) {
    let temporary = tempfile::tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = crate::localnet::prepare_localnet_at(
        "discovery-order",
        &temporary.path().join("generation"),
        &ports,
        LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    let owner =
        ServiceAuthority::open_network(&prepared, NetworkPurpose::ServiceObservation).unwrap();
    let mut native = NativeFixture::from_generated(&prepared, &owner);
    native.retain_http_listeners(&prepared, &ports);
    (temporary, prepared, owner, native)
}

fn commit_discovery_cut(
    native: &mut NativeFixture,
    owner: &ServiceAuthority,
    message: &str,
) -> VerifiedSumeragiBlock {
    let log = quote_instructions(
        native,
        &owner.config,
        [InstructionBox::from(Log::new(
            iroha_data_model::Level::INFO,
            message.into(),
        ))],
    );
    assert_eq!(native.chain.commit(vec![log]), vec![true]);
    native.observe(owner).verified_tip().unwrap()
}

#[test]
fn provider_discovery_starts_at_its_original_validator_and_visits_each_fallback_once() {
    let _resources = crate::managed::native_test_guard();
    let (_tmp, prepared, owner, mut native) = discovery_fixture();
    let block = commit_discovery_cut(&mut native, &owner, "peer order");
    let mut peers = native.unavailable_peers(&prepared);
    let deadline = Instant::now() + Duration::from_secs(30);
    let plans = owner.provider_plans().unwrap();
    for plan in plans {
        assert!(read_discovery_at(&owner, plan.provider_id(), &block, deadline).is_err());
    }
    peers.finish();
    let requests = peers.requests.lock().unwrap();
    for (slot, plan) in plans.iter().enumerate() {
        let path = format!(
            "/v1/sorafs/providers/{}/discovery/{}",
            hex::encode(plan.provider_id().as_bytes()),
            block.height()
        );
        let visited: Vec<_> = requests
            .iter()
            .filter(|r| r.method == "GET" && r.path == path)
            .map(|r| r.peer)
            .collect();
        let expected = (0..4).cycle().skip(slot).take(4).collect::<Vec<_>>();
        assert_eq!(visited, expected);
    }
}

#[test]
fn discovery_and_custody_join_the_same_cut_before_a_row_can_be_used() {
    use std::sync::mpsc;

    let (started, custody_started) = mpsc::sync_channel(0);
    let (read, discovery_read) = mpsc::sync_channel(0);
    let current = read_service_proofs(
        || {
            custody_started
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            read.send(()).unwrap();
            Ok(Some(7))
        },
        move || {
            started.send(()).unwrap();
            discovery_read.recv_timeout(Duration::from_secs(5)).unwrap();
            Ok(Some(()))
        },
    )
    .unwrap();
    assert_eq!(current, Some(7));
    assert_eq!(
        read_service_proofs(|| Ok(Some(7)), || Ok(None)).unwrap(),
        None
    );
    assert_eq!(
        read_service_proofs::<u8>(|| Ok(None), || Ok(None)).unwrap(),
        None
    );
    assert!(read_service_proofs::<u8>(|| Err(invalid("invalid discovery")), || Ok(None)).is_err());
    assert!(read_service_proofs(|| Ok(Some(7)), || Err(invalid("custody refused"))).is_err());
    assert!(
        read_service_proofs::<u8>(|| Err(invalid("discovery refused")), || Ok(Some(()))).is_err()
    );
    assert_eq!(
        read_service_proofs::<u8>(|| Ok(None), || Ok(Some(()))).unwrap(),
        None
    );
}

#[test]
fn unavailable_discovery_and_refused_custody_do_not_request_another_cut() {
    let _resources = crate::managed::native_test_guard();
    let (_tmp, _prepared, mut owner, mut native) = discovery_fixture();
    let first = commit_discovery_cut(&mut native, &owner, "mixed refusal");
    let mut cuts = 0;
    let result = read_current_service_rows(
        &mut owner,
        |_| {
            cuts += 1;
            Ok(first.clone())
        },
        |_, cut| {
            let mixed = read_service_proofs(|| Ok(None), || Err(invalid("custody refused")))?;
            Ok([Some(cut.height()), mixed, Some(cut.height())])
        },
    );
    assert!(matches!(
        result,
        Err(crate::managed::Error::Invalid(message)) if message == "custody refused"
    ));
    assert_eq!(cuts, 1);
}

#[test]
fn active_decoder_keeps_discovery_and_custody_on_its_original_thread() {
    let caller = std::thread::current().id();
    let limits = norito::DecodeLimits::new(1024, 1024, 1024, 0, 8);
    norito::core::DecodeBudgetContext::new(limits).with(|| {
        let current = read_service_proofs(
            || {
                assert_eq!(std::thread::current().id(), caller);
                Ok(Some(3))
            },
            || {
                assert!(norito::core::decode_limits_active());
                assert_eq!(std::thread::current().id(), caller);
                Ok(Some(()))
            },
        )
        .unwrap();
        assert_eq!(current, Some(3));
        assert_eq!(
            read_service_proofs::<u8>(|| Ok(None), || panic!("no serial custody read")).unwrap(),
            None,
        );
    });
}

#[test]
fn provider_discovery_retries_all_rows_once_only_on_a_newer_certified_cut() {
    let _resources = crate::managed::native_test_guard();
    let (_tmp, _prepared, mut owner, mut native) = discovery_fixture();
    let first = commit_discovery_cut(&mut native, &owner, "first cut");
    let next = commit_discovery_cut(&mut native, &owner, "next cut");
    let mut cuts = [first.clone(), next.clone()].into_iter();
    let mut heights = Vec::new();
    let rows = read_current_service_rows(
        &mut owner,
        |_| Ok(cuts.next().unwrap()),
        |_, cut| {
            heights.push(cut.height());
            if cut.height() == first.height() {
                Ok([Some(cut.height()), None, Some(cut.height())])
            } else {
                Ok([Some(cut.height()); 3])
            }
        },
    )
    .unwrap();
    assert_eq!(heights, [first.height(), next.height()]);
    assert_eq!(rows, [next.height(); 3]);

    // An unchanged head does not cause another read; a partial second round stays failed.
    for (second, expected_reads) in [(&first, 1), (&next, 2)] {
        let mut cuts = [&first, second].into_iter();
        let mut reads = 0;
        let result = read_current_service_rows(
            &mut owner,
            |_| Ok(cuts.next().unwrap().clone()),
            |_, cut| {
                reads += 1;
                Ok([None, Some(cut.height()), None])
            },
        );
        assert!(result.is_err());
        assert_eq!(reads, expected_reads);
    }
    // A local policy, invalid proof, or deadline refusal is terminal for this turn.
    let mut cuts = [first].into_iter();
    let result = read_current_service_rows::<u64>(
        &mut owner,
        |_| Ok(cuts.next().unwrap()),
        |_, _| Err(invalid("original scope differs")),
    );
    assert!(result.is_err());
}

#[test]
fn cold_origin_joins_genuine_native_account_proof_before_any_provider_data_request() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = crate::localnet::prepare_localnet_at(
        "readback-origin",
        &temporary.path().join("generation"),
        &ports,
        LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    let authority =
        ServiceAuthority::open_network(&prepared, NetworkPurpose::BuildRegistry).unwrap();
    let config = authority.config.clone();
    let policies = GeneratedServicePolicies::select(&authority).unwrap();
    let policy = &policies.providers[2];
    let provider = policy.provider_id;
    let deadline = Instant::now() + Duration::from_secs(600);
    let now = now_ms().unwrap();
    let interval = policy.initial_enrollment(now, now + 600_000).unwrap();
    let mut native = NativeFixture::from_generated(&prepared, &authority);
    let log = quote_instructions(
        &native,
        &config,
        [InstructionBox::from(Log::new(
            iroha_data_model::Level::INFO,
            "native readback origin prerequisites".into(),
        ))],
    );
    assert_eq!(native.chain.commit(vec![log]), vec![true]);
    drop(ports);
    let options = BoundedTransactionOptions {
        fee_payment: FeePaymentIntent::authority(Vec::new(), None),
        max_total_fees: BTreeMap::from([(
            AssetDefinitionId::parse_address_literal(
                crate::genesis::profile::TAIRA_XOR_ASSET_DEFINITION_ID,
            )
            .unwrap(),
            Quantity::from(1_000u64),
        )]),
        deadline,
    };
    let mut custody = ManagedStreamTokenCustody::open(&prepared, provider).unwrap();
    assert_eq!(
        custody
            .bootstrap_native_configure(
                &mut native,
                &policy.custody,
                interval.deadline_unix_ms,
                &options
            )
            .finalized
            .unwrap()
            .height,
        3
    );
    assert_eq!(
        custody
            .bootstrap_native_enroll(&mut native, &policy.custody, interval, &options)
            .finalized
            .unwrap()
            .height,
        4
    );
    drop(custody);
    let observed = native.observe(&authority);
    let block = observed.verified_tip().unwrap();
    assert_eq!(block.height(), 4);
    let advert = norito::encode_canonical(
        &prepared
            .provider_advert(provider, now_ms().unwrap() / 1000)
            .unwrap(),
    )
    .unwrap();
    let tip = native.chain.committed(native.chain.height());
    let budget = AllocationBudget::new(32 * 1024 * 1024);
    let bytes = native
        .chain
        .state()
        .with_native_provider_admission_snapshot_v1(&tip, provider, &budget, |originals| {
            assert_eq!(
                tip.commitment().execution.world_state_root,
                originals.world.root().unwrap()
            );
            norito::encode_canonical(&ProviderDiscoveryProofRefV1::new(
                originals.world,
                (originals.council_head, originals.council_predecessor),
                (originals.provider_head, originals.provider_predecessor),
                originals.owner,
                &advert,
                originals.stream_token,
            ))
            .map_err(|error| error.to_string())
        })
        .unwrap();
    assert_eq!(budget.reserved_bytes(), 0);
    let proof = ProviderDiscoveryProofV1::decode_frame(&bytes).unwrap();
    let schema = State::native_world_schema_hash_v1().unwrap();
    let verified = proof
        .verify_account_read(
            config.chain.as_str(),
            config.network_id,
            provider,
            schema,
            &block,
            now_ms().unwrap(),
        )
        .unwrap();
    assert_eq!(
        verified.token_public_key(),
        &policy.custody.binding.public_key
    );
    assert_eq!(
        verified.enrollment_expires_at_unix_ms(),
        interval.expires_at_unix_ms
    );
    let plans = prepared.provider_service_plans().unwrap().unwrap();
    let originals = plans.each_ref().map(|plan| {
        GeneratedLocalProviderTransportV1::select(
            plan.network_id(),
            config.chain.as_str(),
            plan.provider_id(),
            &plan.reserve_terms().provider_account,
            plan.admission_material(),
        )
        .unwrap()
    });
    let expected_origin = format!("{}/", plans[2].https_origin());
    let url: url::Url = expected_origin.parse().unwrap();
    assert_ne!(url.port_or_known_default(), Some(443));
    let listener =
        TcpListener::bind((Ipv4Addr::LOCALHOST, url.port_or_known_default().unwrap())).unwrap();
    listener.set_nonblocking(true).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let reject = Arc::new(AtomicBool::new(false));
    let counted = Arc::clone(&calls);
    let refusal = Arc::clone(&reject);
    let registry_config = config.clone();
    let discovery: Arc<MusubiArchiveProviderDiscoveryV1> = Arc::new(move |requested| {
        counted.fetch_add(1, Ordering::SeqCst);
        if refusal.load(Ordering::SeqCst) {
            return Err(MusubiArchiveDiscoveryErrorV1::Rejected);
        }
        proof
            .verify_account_read(
                registry_config.chain.as_str(),
                registry_config.network_id,
                requested,
                schema,
                &block,
                now_ms().unwrap(),
            )
            .map_err(|_| MusubiArchiveDiscoveryErrorV1::Rejected)
    });
    let transport = PreparedMusubiArchiveFetchConfigV1::from_generated_local_account_registry(
        config.clone(),
        Arc::clone(&discovery),
        originals.clone(),
        Duration::from_secs(1),
    )
    .unwrap();
    let client = transport.build_client().unwrap();
    assert!(format!("{client:?}").contains("provider_count: 0"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        client
            .resolve_provider_gateway_origin(ProviderId::new([0; 32]))
            .unwrap_err()
            .code(),
        "MUSUBI_ARCHIVE_LOCAL_TRANSPORT_MISMATCH"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    for count in 1..=2 {
        assert_eq!(
            client.resolve_provider_gateway_origin(provider).unwrap(),
            expected_origin
        );
        assert_eq!(calls.load(Ordering::SeqCst), count);
        assert!(format!("{client:?}").contains("provider_count: 0"));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    reject.store(true, Ordering::SeqCst);
    assert_eq!(
        client
            .resolve_provider_gateway_origin(provider)
            .unwrap_err()
            .code(),
        "MUSUBI_ARCHIVE_PROVIDER_DISCOVERY_INVALID"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    reject.store(false, Ordering::SeqCst);
    // Alter only caller-selected original intent. The actual certified proof stays unchanged;
    // neither a different port nor another original TLS certificate can join it.
    for tls in [false, true] {
        let mut changed = plans[2].admission_material().clone();
        if tls {
            changed.proposal.endpoints[0].attestation.leaf_certificate =
                plans[1].admission_material().proposal.endpoints[0]
                    .attestation
                    .leaf_certificate
                    .clone();
        } else {
            let mut read =
                RegisteredAccountReadV1::from_capabilities(&changed.proposal.capabilities)
                    .unwrap()
                    .unwrap();
            read.https_port = if read.https_port == 8443 { 8444 } else { 8443 };
            let cap = changed
                .proposal
                .capabilities
                .iter_mut()
                .find(|cap| cap.cap_type == CapabilityType::RegisteredAccountRead)
                .unwrap();
            *cap = read.to_capability().unwrap();
            changed.advert_body.capabilities = changed.proposal.capabilities.clone();
        }
        let mut selected = originals.clone();
        selected[2] = GeneratedLocalProviderTransportV1::select(
            config.network_id,
            config.chain.as_str(),
            provider,
            &plans[2].reserve_terms().provider_account,
            &changed,
        )
        .unwrap();
        let wrong = PreparedMusubiArchiveFetchConfigV1::from_generated_local_account_registry(
            config.clone(),
            Arc::clone(&discovery),
            selected,
            Duration::from_secs(1),
        )
        .unwrap()
        .build_client()
        .unwrap();
        assert_eq!(
            wrong
                .resolve_provider_gateway_origin(provider)
                .unwrap_err()
                .code(),
            "MUSUBI_ARCHIVE_LOCAL_TRANSPORT_MISMATCH"
        );
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 5);
    assert_eq!(
        client.resolve_provider_gateway_origin(provider).unwrap(),
        expected_origin
    );
    assert_eq!(calls.load(Ordering::SeqCst), 6);
    // Generic account reads cannot borrow this generated-only loopback exception. Fresh native
    // policy alone also cannot substitute the independently selected registry network/chain.
    for mismatch in 0..3 {
        let mut selected_config = config.clone();
        let expected = match mismatch {
            0 => "MUSUBI_ARCHIVE_FETCH_GATEWAY_URL_INVALID",
            1 => {
                selected_config.chain = "different-registry-chain".parse().unwrap();
                "MUSUBI_ARCHIVE_PROVIDER_DISCOVERY_SCOPE_MISMATCH"
            }
            _ => {
                selected_config.network_id = iroha_data_model::NetworkId::from_genesis_hash(
                    iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
                        b"different-registry-network",
                    )),
                );
                "MUSUBI_ARCHIVE_PROVIDER_DISCOVERY_SCOPE_MISMATCH"
            }
        };
        let remote = PreparedMusubiArchiveFetchConfigV1::from_account_registry(
            selected_config,
            Arc::clone(&discovery),
            Duration::from_secs(1),
        )
        .unwrap()
        .build_client()
        .unwrap();
        assert_eq!(
            remote
                .resolve_provider_gateway_origin(provider)
                .unwrap_err()
                .code(),
            expected
        );
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 9);
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
