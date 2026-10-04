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
