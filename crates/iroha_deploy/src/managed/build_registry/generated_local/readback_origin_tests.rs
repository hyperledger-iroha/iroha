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
                originals.world.root().unwrap(),
                tip.commitment().execution.world_state_root
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
