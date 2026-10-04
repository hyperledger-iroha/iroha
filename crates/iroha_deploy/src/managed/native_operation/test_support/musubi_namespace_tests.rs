//! Exact generated developer-owned namespace through paid wallet and native execution owners.
//! Component certificates authenticate the original carrier; no live publication is inferred.
use super::native_fixture::{NativeFixture, NativeReadHttp, balance, quote_instructions};
use crate::{
    localnet::{LocalnetServiceProfile, prepare_localnet_at},
    managed::{
        LocalnetPorts,
        native_operation::{now_ms, verify_carrier},
        service_authority::{NetworkPurpose, ProviderPurpose, ServiceAuthority},
    },
};
use iroha_core::state::WorldReadOnly;
use iroha_data_model::{
    asset::{AssetDefinitionId, AssetId},
    isi::{InstructionBox, Log, musubi::RegisterMusubiNamespaceBindingV1},
    transaction::Executable,
};
use iroha_model_base::domain::DomainId;
use iroha_primitives::numeric::Quantity;
use iroha_wallet::operations::{
    AccountService, BoundedTransactionOptions, MusubiNamespaceBindingRequest,
    MusubiNamespaceBindingSelection, NativePreparationPhase,
};
use mv::storage::StorageReadOnly as _;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

#[test]
fn generated_namespace_binding_uses_exact_paid_owner_wire_and_native_current_owner() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let ports = LocalnetPorts::reserve().unwrap();
    let prepared = prepare_localnet_at(
        "native-musubi-namespace",
        &temporary.path().join("generation"),
        &ports,
        LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    drop(ports);
    let intent = prepared.publication_client_config().unwrap().unwrap();
    let (developer, _) = iroha::config::Config::load_bytes_with_musubi_publication(
        intent.client_config_path(),
        intent.client_config_image(),
    )
    .unwrap();
    assert_eq!(intent.publisher(), &developer.account);
    assert_eq!(intent.publication_namespace().as_str(), "dev.universal");
    let authority =
        ServiceAuthority::open_network(&prepared, NetworkPurpose::InitialReservePolicy).unwrap();
    let mut native = NativeFixture::from_generated(&prepared, &authority);
    let domain = DomainId::parse_fully_qualified("dev.universal").unwrap();
    {
        let view = native.chain.state().view();
        assert_eq!(
            view.world().domain(&domain).unwrap().owned_by(),
            &developer.account
        );
        assert_eq!(
            view.world().musubi_registry_policy(),
            intent.registry_policy()
        );
        assert!(
            view.world()
                .musubi_namespace_bindings()
                .get(intent.publication_namespace())
                .is_none()
        );
        assert_eq!(
            view.world().musubi_domain_ownership_generation(&domain),
            intent.namespace_binding().generation
        );
    }
    let log = quote_instructions(
        &native,
        &authority.config,
        [InstructionBox::from(Log::new(
            iroha_data_model::Level::INFO,
            "independently observe generated namespace ownership".into(),
        ))],
    );
    assert_eq!(native.chain.commit(vec![log]), vec![true]);
    let before = native.observe(&authority);
    assert_eq!(before.verified_tip().unwrap().height(), 2);
    let asset = AssetDefinitionId::parse_address_literal(
        crate::genesis::profile::TAIRA_XOR_ASSET_DEFINITION_ID,
    )
    .unwrap();
    let owned_asset = AssetId::new(asset.clone(), developer.account.clone());
    let original_fee = intent.namespace_fee_payment();
    assert_eq!(original_fee.charge_limits().len(), 1);
    assert_eq!(original_fee.charge_limits()[0].asset_definition_id, asset);
    assert_eq!(
        original_fee.charge_limits()[0].kind,
        iroha_data_model::transaction::FeeChargeKind::Nexus
    );
    let request = MusubiNamespaceBindingRequest {
        selection: MusubiNamespaceBindingSelection {
            chain_id: developer.chain.to_string(),
            network_id: developer.network_id,
            owner: developer.account.clone(),
            binding: intent.namespace_binding().clone(),
            expected_policy_revision: intent.registry_policy().revision,
        },
        deadline_unix_ms: now_ms().unwrap() + 1_200_000,
        options: BoundedTransactionOptions {
            fee_payment: intent.namespace_fee_payment(),
            max_total_fees: BTreeMap::from([(
                asset.clone(),
                intent.namespace_fee_payment().charge_limits()[0]
                    .max_amount
                    .clone(),
            )]),
            deadline: Instant::now() + Duration::from_secs(600),
        },
    };
    let path = temporary.path().join("namespace-wallet");
    let wallet = AccountService::new(developer.clone()).unwrap();
    let mut http = NativeReadHttp::start_config(&developer, Arc::clone(native.chain.state()));
    wallet
        .prepare_musubi_namespace_binding(&request, &path)
        .unwrap();
    let signed = wallet
        .verify_musubi_namespace_binding_journal(&path, &request)
        .unwrap();
    let calls = http.requests.lock().unwrap().len();
    let (recovered, phase) = wallet
        .recover_musubi_namespace_binding_request(&path, &request.selection, &request.options)
        .unwrap();
    assert_eq!(phase.phase(), NativePreparationPhase::Signed);
    assert_eq!(recovered.deadline_unix_ms, request.deadline_unix_ms);
    assert_eq!(
        http.requests.lock().unwrap().len(),
        calls,
        "original inspection is offline"
    );
    http.finish();
    let quote = http.quote.lock().unwrap().take().unwrap();
    assert!(!quote.components.is_empty());
    let maximum = quote
        .components
        .iter()
        .fold(Quantity::zero(), |sum, component| {
            assert_eq!(component.asset_definition_id, asset);
            sum.checked_add(&component.max_amount).unwrap()
        });
    assert!(!maximum.is_zero());
    assert_eq!(
        http.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, path)| path == "/v1/fees/quote")
            .count(),
        1
    );
    assert!(
        http.requests
            .lock()
            .unwrap()
            .iter()
            .any(|(_, path)| path == "/v1/query")
    );
    let Executable::Instructions(items) = signed.instructions() else {
        panic!("native Register")
    };
    assert_eq!(items.len(), 1);
    let register = items[0]
        .as_any()
        .downcast_ref::<RegisterMusubiNamespaceBindingV1>()
        .unwrap();
    assert_eq!(register.binding, *intent.namespace_binding());
    assert_eq!(
        register.expected_policy_revision,
        intent.registry_policy().revision
    );
    assert_eq!(signed.authority(), &developer.account);
    let wire = signed.encode_wire_v1().unwrap();
    let original = std::fs::read(path.join("operation.json")).unwrap();
    let owned_before = balance(native.chain.state(), &owned_asset);
    assert_eq!(native.chain.commit(vec![signed.clone()]), vec![true]);
    let committed = native.observe(&authority);
    let carrier = verify_carrier(&committed, &signed).unwrap();
    assert_eq!(carrier.height, 3);
    let tip = committed.verified_tip().unwrap();
    assert_eq!(tip.block().network_entrypoint_count(), 1);
    assert_eq!(
        tip.block()
            .external_transactions()
            .next()
            .unwrap()
            .encode_wire_v1()
            .unwrap(),
        wire
    );
    assert!(verify_carrier(&before, &signed).is_err());
    let fee = owned_before
        .checked_sub(&balance(native.chain.state(), &owned_asset))
        .unwrap();
    assert!(!fee.is_zero() && fee <= maximum);
    assert_eq!(
        native
            .chain
            .state()
            .view()
            .world()
            .musubi_namespace_bindings()
            .get(intent.publication_namespace()),
        Some(intent.namespace_binding())
    );
    drop(wallet);
    let reopened = AccountService::new(developer.clone()).unwrap();
    let (again, phase) = reopened
        .recover_musubi_namespace_binding_request(&path, &request.selection, &request.options)
        .unwrap();
    assert_eq!(phase.phase(), NativePreparationPhase::Signed);
    assert_eq!(again.deadline_unix_ms, request.deadline_unix_ms);
    assert_eq!(
        reopened
            .verify_musubi_namespace_binding_journal(&path, &again)
            .unwrap()
            .encode_wire_v1()
            .unwrap(),
        wire
    );
    assert_eq!(
        std::fs::read(path.join("operation.json")).unwrap(),
        original
    );
    assert!(
        !path.join("submission.json").exists(),
        "native component commit did not impersonate wallet dispatch"
    );
    let selected = ServiceAuthority::open_provider(
        &prepared,
        super::provider_id(&prepared, 0),
        ProviderPurpose::ReserveAccountRegistration,
    )
    .unwrap();
    let foreign = selected.issuer_operator_config().unwrap();
    assert_ne!(foreign.account, developer.account);
    let rejected = quote_instructions(
        &native,
        &foreign,
        [InstructionBox::from(RegisterMusubiNamespaceBindingV1::new(
            intent.namespace_binding().clone(),
            intent.registry_policy().revision,
        ))],
    );
    assert_eq!(native.chain.commit(vec![rejected.clone()]), vec![false]);
    assert!(verify_carrier(&native.observe(&authority), &rejected).is_err());
    assert_eq!(
        native
            .chain
            .state()
            .view()
            .world()
            .musubi_namespace_bindings()
            .get(intent.publication_namespace()),
        Some(intent.namespace_binding())
    );
    let replay = quote_instructions(
        &native,
        &developer,
        [InstructionBox::from(RegisterMusubiNamespaceBindingV1::new(
            intent.namespace_binding().clone(),
            intent.registry_policy().revision,
        ))],
    );
    assert_eq!(native.chain.commit(vec![replay]), vec![true]);
    let mut changed = intent.namespace_binding().clone();
    changed.generation += 1;
    let changed = quote_instructions(
        &native,
        &developer,
        [InstructionBox::from(RegisterMusubiNamespaceBindingV1::new(
            changed,
            intent.registry_policy().revision,
        ))],
    );
    assert_eq!(native.chain.commit(vec![changed]), vec![false]);
    assert_eq!(
        native
            .chain
            .state()
            .view()
            .world()
            .musubi_namespace_bindings()
            .get(intent.publication_namespace()),
        Some(intent.namespace_binding())
    );
    assert_eq!(
        std::fs::read(path.join("operation.json")).unwrap(),
        original
    );
}
