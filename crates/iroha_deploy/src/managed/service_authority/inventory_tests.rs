//! Real generated private-custody controls for the parent-only absence census.

use super::*;
use iroha_fs::PublishMode;

fn fixture() -> (tempfile::TempDir, ServiceAuthority) {
    let temporary = tempfile::tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = crate::localnet::prepare_localnet_at(
        "inventory-fast-path",
        &temporary.path().join("generation"),
        &ports,
        crate::localnet::LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    let owner =
        ServiceAuthority::open_network(&prepared, NetworkPurpose::ServiceBootstrap).unwrap();
    (temporary, owner)
}

#[test]
fn all_absent_child_purposes_preserve_names_without_reconstructing_authorities() {
    let _guard = crate::managed::native_test_guard();
    let (_temporary, parent) = fixture();
    let providers = parent
        .manifest
        .providers
        .each_ref()
        .map(|value| value.provider_id);
    let before = parent.directory.entries(4).unwrap();
    let (result, opens) = ServiceChildInventory::test_count_authority_opens(|| -> Result<()> {
        let inventory = ServiceChildInventory::begin(&parent)?;
        assert!(
            inventory
                .open_network(NetworkPurpose::InitialReservePolicy)?
                .is_none()
        );
        assert!(
            inventory
                .open_network(NetworkPurpose::InitialReputationPolicy)?
                .is_none()
        );
        for provider in providers {
            for purpose in [
                ProviderPurpose::Custody,
                ProviderPurpose::Custody,
                ProviderPurpose::ReserveAccountRegistration,
                ProviderPurpose::ProviderFundingBootstrap,
                ProviderPurpose::ReserveTopUpRequest,
                ProviderPurpose::ReserveTopUpApproval,
                ProviderPurpose::InitialProviderCredit,
                ProviderPurpose::ProviderCapacityDeclaration,
                ProviderPurpose::InitialProviderIngestAuthority,
                ProviderPurpose::InitialGatewaySetup,
            ] {
                assert!(inventory.open_provider(provider, purpose)?.is_none());
            }
        }
        assert!(
            inventory
                .open_provider(ProviderId::new([0x71; 32]), ProviderPurpose::Custody)
                .is_err()
        );
        inventory.finish()
    });
    result.unwrap();
    assert_eq!(opens, 0);
    assert_eq!(parent.directory.entries(4).unwrap(), before);
    assert!(
        !parent
            .directory
            .path()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("providers")
            .exists()
    );
}

#[test]
fn present_child_preserves_native_lock_empty_prefix_and_dirty_loss_rules() {
    let _guard = crate::managed::native_test_guard();
    let (_temporary, parent) = fixture();
    let provider = parent.manifest.providers[1].provider_id;
    let child =
        ServiceAuthority::open_provider(&parent.prepared, provider, ProviderPurpose::Custody)
            .unwrap();
    let retained = child.directory.retain().unwrap();
    let inventory = ServiceChildInventory::begin(&parent).unwrap();
    let (locked, opens) = ServiceChildInventory::test_count_authority_opens(|| {
        inventory.open_provider(provider, ProviderPurpose::Custody)
    });
    assert!(locked.is_err());
    assert_eq!(opens, 1);
    drop(child);
    let (reopened, opens) = ServiceChildInventory::test_count_authority_opens(|| {
        inventory.open_provider(provider, ProviderPurpose::Custody)
    });
    let reopened = reopened.unwrap().unwrap();
    assert_eq!(opens, 1);
    assert_eq!(
        reopened.directory.identity().unwrap(),
        retained.identity().unwrap()
    );
    assert_eq!(reopened.config.network_id, parent.config.network_id);
    drop(reopened);
    std::fs::remove_file(retained.path().join("operation.lock")).unwrap();
    let (empty, opens) = ServiceChildInventory::test_count_authority_opens(|| {
        inventory.open_provider(provider, ProviderPurpose::Custody)
    });
    assert!(empty.unwrap().is_none());
    assert_eq!(opens, 1);
    assert!(retained.entries(1).unwrap().is_empty());
    super::super::super::native_operation::require_empty(&retained).unwrap();
    retained
        .write_atomic("unknown.nrt", b"presence only", PublishMode::CreateNew)
        .unwrap();
    // The exact snapshot must also be empty if material arrives after the initial empty check.
    assert!(Branch::capture_empty(retained.retain().unwrap()).is_err());
    let (changed, opens) = ServiceChildInventory::test_count_authority_opens(|| {
        inventory.open_provider(provider, ProviderPurpose::Custody)
    });
    assert!(changed.is_err());
    assert_eq!(opens, 0);
    assert!(inventory.finish().is_err());
    let inventory = ServiceChildInventory::begin(&parent).unwrap();
    let (dirty, opens) = ServiceChildInventory::test_count_authority_opens(|| {
        inventory.open_provider(provider, ProviderPurpose::Custody)
    });
    assert!(dirty.is_err());
    assert_eq!(opens, 1);
    assert!(!retained.path().join("operation.lock").exists());
    assert_eq!(
        retained.read("unknown.nrt", 32).unwrap().as_slice(),
        b"presence only"
    );
    inventory.finish().unwrap();
}

#[test]
fn new_earlier_namespace_and_replaced_parent_lock_refuse_before_child_open() {
    let _guard = crate::managed::native_test_guard();
    let (_temporary, parent) = fixture();
    let provider = parent.manifest.providers[0].provider_id;
    let inventory = ServiceChildInventory::begin(&parent).unwrap();
    assert!(
        inventory
            .open_provider(provider, ProviderPurpose::Custody)
            .unwrap()
            .is_none()
    );
    // The same parent-native ancestry now has material where the previous probe observed absence.
    let operations = &inventory.branches[1].directory;
    operations.ensure_child("providers").unwrap();
    let (changed, opens) = ServiceChildInventory::test_count_authority_opens(|| {
        inventory.open_network(NetworkPurpose::InitialReservePolicy)
    });
    assert!(changed.is_err());
    assert_eq!(opens, 0);
    assert!(inventory.finish().is_err());
    let inventory = ServiceChildInventory::begin(&parent).unwrap();
    #[cfg(unix)]
    {
        parent
            .directory
            .write_atomic("operation.lock", b"", PublishMode::Replace)
            .unwrap();
        let (changed, opens) = ServiceChildInventory::test_count_authority_opens(|| {
            inventory.open_provider(provider, ProviderPurpose::Custody)
        });
        assert!(changed.is_err());
        assert_eq!(opens, 0);
        assert!(inventory.finish().is_err());
    }
    #[cfg(windows)]
    {
        // The retained native lock denies deletion/replacement before its identity can change.
        let original = iroha_fs::FileIdentity::of(&parent._lock).unwrap();
        assert!(
            parent
                .directory
                .write_atomic("operation.lock", b"", PublishMode::Replace)
                .is_err()
        );
        assert_eq!(iroha_fs::FileIdentity::of(&parent._lock).unwrap(), original);
        assert_eq!(
            iroha_fs::FileIdentity::of(&parent.directory.open_read("operation.lock").unwrap())
                .unwrap(),
            original
        );
        let (unchanged, opens) = ServiceChildInventory::test_count_authority_opens(|| {
            inventory.open_provider(provider, ProviderPurpose::Custody)
        });
        assert!(unchanged.unwrap().is_none());
        assert_eq!(opens, 0);
        inventory.finish().unwrap();
    }
}

#[test]
fn whole_profile_is_authenticated_before_and_after_the_absence_census() {
    let _guard = crate::managed::native_test_guard();
    let (_temporary, parent) = fixture();
    let inventory = ServiceChildInventory::begin(&parent).unwrap();
    let peer = &parent.prepared.peers[0].config_path;
    let directory = PrivateDirectory::open_exact(peer.parent().unwrap()).unwrap();
    let original = directory
        .read(peer.file_name().unwrap(), 1024 * 1024)
        .unwrap();
    directory
        .write_atomic(
            peer.file_name().unwrap(),
            b"invalid = [",
            PublishMode::Replace,
        )
        .unwrap();
    assert!(inventory.finish().is_err());
    assert!(ServiceChildInventory::begin(&parent).is_err());
    // Ordinary standalone absence callers keep their full original authentication contract.
    assert!(
        ServiceAuthority::open_network_existing(
            &parent.prepared,
            NetworkPurpose::InitialReservePolicy
        )
        .is_err()
    );
    directory
        .write_atomic(peer.file_name().unwrap(), &original, PublishMode::Replace)
        .unwrap();
    let inventory = ServiceChildInventory::begin(&parent).unwrap();
    assert!(
        inventory
            .open_network(NetworkPurpose::InitialReservePolicy)
            .unwrap()
            .is_none()
    );
    inventory.finish().unwrap();
}

#[test]
fn first_reserve_name_probe_refuses_appeared_names_and_retries_fresh_without_child_opens() {
    let _guard = crate::managed::native_test_guard();
    let (_temporary, parent) = fixture();
    let (missing, opens) = ServiceChildInventory::test_count_authority_opens(|| {
        ServiceChildInventory::begin(&parent)?.initial_reserve_policy_absent()
    });
    assert!(missing.unwrap());
    assert_eq!(opens, 0);
    let inventory = ServiceChildInventory::begin(&parent).unwrap();
    let network = inventory.branches[inventory.network]
        .directory
        .retain()
        .unwrap();
    let purpose = network
        .ensure_child(NetworkPurpose::InitialReservePolicy.directory_name())
        .unwrap();
    let (refused, opens) = ServiceChildInventory::test_count_authority_opens(|| {
        inventory.initial_reserve_policy_absent()
    });
    assert!(refused.is_err());
    assert_eq!(opens, 0);
    let (present, opens) = ServiceChildInventory::test_count_authority_opens(|| {
        ServiceChildInventory::begin(&parent)?.initial_reserve_policy_absent()
    });
    assert!(!present.unwrap());
    assert_eq!(opens, 0, "presence observation must not lock a child");
    assert!(purpose.entries(1).unwrap().is_empty());
    std::fs::remove_dir(purpose.path()).unwrap();
    let inventory = ServiceChildInventory::begin(&parent).unwrap();
    let appeared = network.ensure_child("appeared-purpose").unwrap();
    assert!(inventory.initial_reserve_policy_absent().is_err());
    std::fs::remove_dir(appeared.path()).unwrap();
    assert!(
        ServiceChildInventory::begin(&parent)
            .unwrap()
            .initial_reserve_policy_absent()
            .unwrap()
    );
}

#[test]
fn first_reserve_name_probe_checks_original_profile_at_entry_and_exit_then_retries() {
    let _guard = crate::managed::native_test_guard();
    let (_temporary, parent) = fixture();
    let peer = &parent.prepared.peers[0].config_path;
    let generation = PrivateDirectory::open_exact(peer.parent().unwrap()).unwrap();
    let name = peer.file_name().unwrap();
    let original = generation.read(name, 1024 * 1024).unwrap();
    let inventory = ServiceChildInventory::begin(&parent).unwrap();
    generation
        .write_atomic(name, b"invalid = [", PublishMode::Replace)
        .unwrap();
    let ((entry, exit), opens) = ServiceChildInventory::test_count_authority_opens(|| {
        (
            ServiceChildInventory::begin(&parent).is_err(),
            inventory.initial_reserve_policy_absent().is_err(),
        )
    });
    assert!(entry && exit);
    assert_eq!(opens, 0);
    generation
        .write_atomic(name, &original, PublishMode::Replace)
        .unwrap();
    let (retried, opens) = ServiceChildInventory::test_count_authority_opens(|| {
        ServiceChildInventory::begin(&parent)?.initial_reserve_policy_absent()
    });
    assert!(retried.unwrap());
    assert_eq!(opens, 0);
    assert_eq!(generation.read(name, 1024 * 1024).unwrap(), original);
}

#[test]
fn first_reserve_name_probe_retains_parent_lock_and_native_ancestors_before_retry() {
    let _guard = crate::managed::native_test_guard();
    let (_temporary, parent) = fixture();
    let inventory = ServiceChildInventory::begin(&parent).unwrap();
    let lock_identity = iroha_fs::FileIdentity::of(&parent._lock).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let lock_path = parent.directory.path().join("operation.lock");
        let original_path = parent.directory.path().join("original-operation.lock");
        std::fs::rename(&lock_path, &original_path).unwrap();
        parent
            .directory
            .write_atomic("operation.lock", b"", PublishMode::CreateNew)
            .unwrap();
        assert!(inventory.initial_reserve_policy_absent().is_err());
        assert!(ServiceChildInventory::begin(&parent).is_err());
        std::fs::remove_file(&lock_path).unwrap();
        std::fs::rename(&original_path, &lock_path).unwrap();
        assert_eq!(
            iroha_fs::FileIdentity::of(&parent.directory.open_read("operation.lock").unwrap())
                .unwrap(),
            lock_identity
        );
        assert!(
            ServiceChildInventory::begin(&parent)
                .unwrap()
                .initial_reserve_policy_absent()
                .unwrap()
        );

        let inventory = ServiceChildInventory::begin(&parent).unwrap();
        let network_path = parent.directory.path().parent().unwrap();
        let original_mode = std::fs::metadata(network_path).unwrap().permissions();
        std::fs::set_permissions(network_path, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(inventory.initial_reserve_policy_absent().is_err());
        std::fs::set_permissions(network_path, original_mode).unwrap();
        assert!(
            ServiceChildInventory::begin(&parent)
                .unwrap()
                .initial_reserve_policy_absent()
                .unwrap()
        );

        let inventory = ServiceChildInventory::begin(&parent).unwrap();
        let operations = PrivateDirectory::open_exact(network_path.parent().unwrap()).unwrap();
        let saved_network = operations.path().join("original-network");
        std::fs::rename(network_path, &saved_network).unwrap();
        let replaced = operations.ensure_child("network").unwrap();
        assert!(inventory.initial_reserve_policy_absent().is_err());
        std::fs::remove_dir(replaced.path()).unwrap();
        std::fs::rename(&saved_network, network_path).unwrap();
        assert!(
            ServiceChildInventory::begin(&parent)
                .unwrap()
                .initial_reserve_policy_absent()
                .unwrap()
        );
    }
    #[cfg(windows)]
    {
        // Retained native lock denies replacement; no successful mutation is fabricated.
        assert!(
            parent
                .directory
                .write_atomic("operation.lock", b"", PublishMode::Replace)
                .is_err()
        );
        assert_eq!(
            iroha_fs::FileIdentity::of(&parent._lock).unwrap(),
            lock_identity
        );
        assert!(inventory.initial_reserve_policy_absent().unwrap());
    }
    assert_eq!(
        iroha_fs::FileIdentity::of(&parent._lock).unwrap(),
        lock_identity
    );
    assert!(
        ServiceChildInventory::begin(&parent)
            .unwrap()
            .initial_reserve_policy_absent()
            .unwrap()
    );
}
