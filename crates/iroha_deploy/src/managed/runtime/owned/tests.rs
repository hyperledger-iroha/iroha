//! Actual owned handles and genuine renderer provenance; no test process is treated as a daemon.

use super::*;
use crate::{
    localnet::LocalnetServiceProfile, managed::service_bootstrap::ManagedServiceBootstrap,
};

fn launch() -> (tempfile::TempDir, PreparedLocalnet, Arc<GeneratedLaunch>) {
    let temporary = tempfile::tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = crate::localnet::prepare_localnet_at(
        "owned-launch",
        &temporary.path().join("generation"),
        &ports,
        LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut parent = ManagedServiceBootstrap::open(&prepared).unwrap();
    parent
        .authorize_generated_startup(deadline, Arc::new(AtomicBool::new(false)))
        .unwrap()
        .unwrap();
    drop(parent);
    let owner = Arc::new(GeneratedServiceRuntime::open(&prepared).unwrap());
    let revision = owner.prepare_catalog(deadline).unwrap();
    let launch = GeneratedLaunch::new(owner, revision, &prepared).unwrap();
    (temporary, prepared, launch)
}

#[test]
fn generated_command_requires_exact_revision_and_invalidation_prevents_reuse() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared, launch) = launch();
    let daemon = std::path::Path::new("iroha3d");
    let command = launch.command(daemon, 0).unwrap();
    let peer = launch.revision.peer(0).unwrap();
    let digest = hex::encode(peer.blake3());
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            std::ffi::OsStr::new("--sora"),
            "--config".as_ref(),
            peer.path().as_os_str(),
            "--config-blake3".as_ref(),
            digest.as_ref()
        ]
    );
    assert_eq!(
        command.get_current_dir(),
        prepared.context.client_config.parent()
    );
    assert!(launch.command(daemon, 4).is_err());
    let mut owner = PeerProcesses {
        children: Vec::new(),
        launch: Some(Arc::clone(&launch)),
    };
    for plan in prepared.provider_service_plans().unwrap().unwrap() {
        assert!(
            owner.gateway(plan.provider_id()).is_err(),
            "rendered config without owned children is not a live gateway"
        );
    }
    assert!(owner.gateways().is_err());
    owner.stop().unwrap();
    assert!(!launch.active.load(Ordering::Acquire));
    assert!(launch.command(daemon, 0).is_err());
    assert!(launch.validate().is_err());
}

#[cfg(unix)]
#[test]
fn original_guard_dies_on_owned_stop_and_cannot_be_reused_for_another_launch() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared, launch) = launch();
    let children = (0..4)
        .map(|_| Command::new("/bin/sleep").arg("30").spawn().unwrap())
        .collect();
    let mut processes = PeerProcesses::from_children(children);
    processes.launch = Some(Arc::clone(&launch));
    let mut gateways = processes.gateways().unwrap();
    let plans = prepared.provider_service_plans().unwrap().unwrap();
    for (index, gateway) in gateways.iter_mut().enumerate() {
        assert_eq!(gateway.provider(), plans[index].provider_id());
        let plan = prepared
            .gateway_compliance_plan(gateway.provider())
            .unwrap()
            .unwrap();
        gateway.validate(&prepared, &plan).unwrap();
        for other in &plans {
            if other.provider_id() != gateway.provider() {
                let foreign = prepared
                    .gateway_compliance_plan(other.provider_id())
                    .unwrap()
                    .unwrap();
                assert!(gateway.validate(&prepared, &foreign).is_err());
            }
        }
    }
    let unknown = iroha_data_model::sorafs::capacity::ProviderId::new([0xAB; 32]);
    assert!(processes.gateway(unknown).is_err());
    let comparison = crate::managed::native_operation::ManagedTransactionFinality {
        transaction_hash: iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
            b"catalog-has-no-transaction",
        )),
        height: 13,
        block_hash: iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
            b"catalog-has-no-carrier",
        )),
        block_time_ms: 1,
    };
    for gateway in &mut gateways {
        assert!(
            gateway.selected_enrollment(comparison).is_err(),
            "Catalog launch cannot supply enrollment authority"
        );
        let plan = prepared
            .gateway_compliance_plan(gateway.provider())
            .unwrap()
            .unwrap();
        let mut foreign = prepared.clone();
        foreign.context.name.push_str("-foreign");
        assert!(gateway.validate(&foreign, &plan).is_err());
    }
    processes.stop().unwrap();
    assert!(processes.children.is_empty());
    for gateway in &mut gateways {
        let plan = prepared
            .gateway_compliance_plan(gateway.provider())
            .unwrap()
            .unwrap();
        assert!(gateway.require_running().is_err());
        assert!(gateway.selected_enrollment(comparison).is_err());
        assert!(gateway.validate(&prepared, &plan).is_err());
        assert!(processes.gateway(gateway.provider()).is_err());
    }
    assert!(processes.gateways().is_err());
    assert!(processes.generated().unwrap().is_none());
    assert!(!launch.active.load(Ordering::Acquire));
}
