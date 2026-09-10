//! Daemon dependency admission simulations; no metadata value qualifies physical hardware.
use super::*;
use iroha_torii::sorafs::{
    StreamTokenApprovedCustodyAnchorV1, StreamTokenHardwareCallErrorV1,
    StreamTokenHardwareClientV1, StreamTokenHardwareReceiptV1, StreamTokenObserverReplyV1,
    StreamTokenStateObserverClientV1,
};
use sorafs_manifest::{
    StreamTokenBodyV1,
    signer::{
        custody::SignerCustodyAnchorV1, stream_token::SignerStreamTokenExpectedV1,
        stream_token_evidence::SignerStreamTokenObservationRequestV1,
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

type Error = IrohaRuntimeProviderRegistryErrorV1;

struct Route {
    handles: [String; 2],
    calls: AtomicUsize,
}
impl Route {
    fn exact(handle: &str) -> Arc<Self> {
        Self::changing(handle, handle)
    }
    fn changing(first: &str, second: &str) -> Arc<Self> {
        Arc::new(Self {
            handles: [first.to_owned(), second.to_owned()],
            calls: AtomicUsize::new(0),
        })
    }
    fn route(&self) -> &str {
        &self.handles[self.calls.fetch_add(1, Ordering::SeqCst).min(1)]
    }
}
impl StreamTokenHardwareClientV1 for Route {
    fn handle(&self) -> &str {
        self.route()
    }
    fn sign(
        &self,
        _: &SignerStreamTokenExpectedV1,
        _: &StreamTokenBodyV1,
    ) -> Result<StreamTokenHardwareReceiptV1, StreamTokenHardwareCallErrorV1> {
        panic!("metadata admission must never sign")
    }
    fn recover(
        &self,
        _: &SignerStreamTokenExpectedV1,
        _: &StreamTokenBodyV1,
    ) -> Result<StreamTokenHardwareReceiptV1, StreamTokenHardwareCallErrorV1> {
        panic!("metadata admission must never recover")
    }
}
impl StreamTokenStateObserverClientV1 for Route {
    fn handle(&self) -> &str {
        self.route()
    }
    fn observe(
        &self,
        _: &SignerStreamTokenObservationRequestV1,
    ) -> Result<StreamTokenObserverReplyV1, StreamTokenHardwareCallErrorV1> {
        panic!("registry has no independently authenticated local finalized view")
    }
}

fn fixture() -> (
    IrohaRuntimeProviderBindingsV1,
    StreamTokenHardwareRuntimeBindingV1,
) {
    let metadata = super::super::stream_token_hardware_binding::tests::fixture();
    let catalog = IrohaRuntimeProviderBindingsV1 {
        chain_id: metadata.custody().chain_id.clone(),
        network_id: crate::runtime_provider_registry::runtime_provider_test_network_id(),
        bindings: vec![
            IrohaRuntimeProviderBindingV1::try_new_stream_token_signer(metadata.clone()).unwrap(),
        ],
    };
    (catalog, metadata)
}
fn approved(metadata: &StreamTokenHardwareRuntimeBindingV1) -> StreamTokenApprovedCustodyAnchorV1 {
    StreamTokenApprovedCustodyAnchorV1::new(
        metadata.trust_pins_digest(),
        SignerCustodyAnchorV1 {
            height: 7,
            block_hash: [0x61; 32],
            state_digest: [0x62; 32],
        },
    )
    .unwrap()
}
fn dependencies(metadata: &StreamTokenHardwareRuntimeBindingV1) -> IrohaRuntimeDeps {
    IrohaRuntimeDeps::default()
        .with_sorafs_stream_token_hardware_client(Route::exact(&metadata.custody().runtime_handle))
        .with_sorafs_stream_token_state_observer(Route::exact(metadata.observer_handle()))
}

#[test]
fn exact_dependency_handles_are_observed_twice_without_signing_or_qualification() {
    let (catalog, metadata) = fixture();
    let hardware = Route::exact(&metadata.custody().runtime_handle);
    let observer = Route::exact(metadata.observer_handle());
    let deps = IrohaRuntimeDeps::default()
        .with_sorafs_stream_token_hardware_client(hardware.clone())
        .with_sorafs_stream_token_state_observer(observer.clone())
        .with_sorafs_stream_token_approved_anchor(approved(&metadata));
    validate_dependency_bindings(&catalog, &deps).unwrap();
    assert_eq!(hardware.calls.load(Ordering::SeqCst), 2);
    assert_eq!(observer.calls.load(Ordering::SeqCst), 2);
    assert!(!deps.is_empty());
    assert_eq!(
        deps.sorafs_stream_token_approved_anchor
            .unwrap()
            .config_digest(),
        metadata.trust_pins_digest()
    );
}

#[test]
fn substituted_identity_drift_and_test_markers_are_rejected() {
    let (catalog, metadata) = fixture();
    for mutate_hardware in [false, true] {
        for first_only in [false, true] {
            for (route, error) in [
                (
                    "hsm://sorafs/stream-token/foreign-a",
                    Error::BindingMismatch,
                ),
                (
                    "mock://sorafs/stream-token/primary-a",
                    Error::TestProviderRejected,
                ),
            ] {
                let mut deps = dependencies(&metadata);
                let exact = if mutate_hardware {
                    &metadata.custody().runtime_handle
                } else {
                    metadata.observer_handle()
                };
                let changed = Route::changing(if first_only { exact } else { route }, route);
                if mutate_hardware {
                    deps.sorafs_stream_token_hardware_client = Some(changed);
                } else {
                    deps.sorafs_stream_token_state_observer = Some(changed);
                }
                assert_eq!(validate_dependency_bindings(&catalog, &deps), Err(error));
            }
        }
    }
}

#[test]
fn all_dependency_subsets_are_exactly_scoped_and_missing_clients_fail_closed() {
    let (catalog, metadata) = fixture();
    let empty = IrohaRuntimeProviderBindingsV1 {
        bindings: vec![],
        ..catalog.clone()
    };
    let slot = IrohaRuntimeProviderSlotV1::StreamTokenSigner;
    for mask in 0_u8..8 {
        let mut deps = IrohaRuntimeDeps::default();
        if mask & 1 != 0 {
            deps = deps.with_sorafs_stream_token_hardware_client(Route::exact(
                &metadata.custody().runtime_handle,
            ));
        }
        if mask & 2 != 0 {
            deps = deps
                .with_sorafs_stream_token_state_observer(Route::exact(metadata.observer_handle()));
        }
        if mask & 4 != 0 {
            deps = deps.with_sorafs_stream_token_approved_anchor(approved(&metadata));
        }
        assert_eq!(deps.is_empty(), mask == 0);
        assert_eq!(dependency_is_present(&deps, slot), mask & 3 == 3);
        assert_eq!(has_unrequested_dependency(&empty, &deps), mask != 0);
        assert!(!has_unrequested_dependency(&catalog, &deps));
        assert_eq!(
            validate_dependency_bindings(&catalog, &deps),
            if mask & 3 == 3 {
                Ok(())
            } else {
                Err(Error::IncompleteResolution)
            }
        );
    }
}

#[test]
fn independent_anchor_must_match_complete_pins_but_never_establishes_finality_here() {
    let (catalog, metadata) = fixture();
    let deps = dependencies(&metadata);
    validate_dependency_bindings(&catalog, &deps).unwrap();
    assert!(
        deps.sorafs_stream_token_approved_anchor.is_none(),
        "broker-only metadata resolution may not manufacture the startup floor"
    );
    let wrong =
        StreamTokenApprovedCustodyAnchorV1::new([0x66; 32], approved(&metadata).anchor()).unwrap();
    assert_eq!(
        validate_dependency_bindings(
            &catalog,
            &deps.with_sorafs_stream_token_approved_anchor(wrong)
        ),
        Err(Error::BindingMismatch)
    );
    let deps =
        dependencies(&metadata).with_sorafs_stream_token_approved_anchor(approved(&metadata));
    validate_dependency_bindings(&catalog, &deps).unwrap();
    let mut wrong_chain = catalog.clone();
    wrong_chain.chain_id.push_str("-foreign");
    assert!(validate_dependency_bindings(&wrong_chain, &deps).is_err());
    let mut wrong_network = catalog;
    wrong_network.network_id =
        iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            iroha_data_model::block::BlockHeader,
        >::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x6f; 32])
        ));
    assert!(validate_dependency_bindings(&wrong_network, &deps).is_err());
}
