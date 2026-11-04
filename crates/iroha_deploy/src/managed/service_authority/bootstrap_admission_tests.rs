//! Native bootstrap admission preserves absence, locks, parent exits and physical decoding.

use super::creating_original_tests::{fixture, limits, parsed};
use super::*;
use crate::localnet::service_authorities::count_profile_images;
use iroha_fs::{FileIdentity, PublishMode};
use norito::core::DecodeBudgetContext;
use std::cell::Cell;

fn path(parent: &ServiceAuthority, provider: ProviderId) -> std::path::PathBuf {
    parent
        .profile
        .runtime()
        .path()
        .join("service-operations/providers")
        .join(parent.manifest.provider(provider).unwrap().slot.to_string())
        .join(ProviderPurpose::Custody.directory_name())
}

fn combined(
    parent: &ServiceAuthority,
    provider: ProviderId,
    authorize: &dyn Fn() -> Result<bool>,
) -> Result<Option<ServiceAuthority>> {
    ServiceAuthority::open_provider_for_bootstrap(
        parent,
        provider,
        ProviderPurpose::Custody,
        authorize,
    )
}

fn original_recipe(parent: &ServiceAuthority, provider: ProviderId) -> Result<ServiceAuthority> {
    if let Some(child) = ServiceAuthority::open_provider_existing_from_original(
        parent,
        provider,
        ProviderPurpose::Custody,
        None,
    )? {
        return Ok(child);
    }
    // Match the full profile fence in the real startup permission callback. The actual
    // epoch/deadline/cancellation controls live in the bootstrap phase tests below.
    parent.validate_profile()?;
    ServiceAuthority::open_provider_from_original(parent, provider, ProviderPurpose::Custody)
}

#[test]
fn bootstrap_admission_removes_only_duplicate_parent_images_and_keeps_exact_child_lock() {
    let _guard = crate::managed::native_test_guard();
    let (_temporary, parent) = fixture();
    let provider = parent.manifest.providers[0].provider_id;
    let path = path(&parent, provider);
    drop(PrivateDirectory::open_or_create(path.parent().unwrap()).unwrap());
    let ((baseline, parses), baseline_images) =
        count_profile_images(|| parsed(|| original_recipe(&parent, provider)));
    let baseline = baseline.unwrap();
    assert_eq!(parses, 0);
    assert!(baseline.checkpoint_import_scope().is_none());
    drop(baseline);
    std::fs::remove_dir_all(&path).unwrap();
    let calls = Cell::new(0);
    let ((actual, parses), images) = count_profile_images(|| {
        parsed(|| {
            combined(&parent, provider, &|| {
                calls.set(calls.get() + 1);
                parent.validate_profile()?;
                Ok(true)
            })
        })
    });
    let actual = actual.unwrap().unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(parses, 0);
    assert_eq!((baseline_images, images), (6, 4));
    eprintln!("bootstrap native child image passes: original={baseline_images} combined={images}");
    assert!(actual.checkpoint_import_scope().is_none());
    assert_eq!(actual.provider_id().unwrap(), provider);
    assert!(combined(&parent, provider, &|| panic!("locked child is not absence")).is_err());
    let directory = actual.directory.retain().unwrap();
    let identity = directory.identity().unwrap();
    let lock = FileIdentity::of(&actual.lock).unwrap();
    drop(actual);
    let (existing, images) = count_profile_images(|| {
        combined(&parent, provider, &|| {
            panic!("existing recovery must not require live creation permission")
        })
    });
    let existing = existing.unwrap().unwrap();
    assert_eq!(images, 3);
    assert_eq!(existing.directory.identity().unwrap(), identity);
    assert_eq!(FileIdentity::of(&existing.lock).unwrap(), lock);
    assert_eq!(
        directory.entries(2).unwrap(),
        vec![std::ffi::OsString::from("operation.lock")]
    );
}

#[test]
fn bootstrap_admission_refuses_nonempty_missing_lock_before_authorization() {
    let _guard = crate::managed::native_test_guard();
    let (_temporary, parent) = fixture();
    let provider = parent.manifest.providers[0].provider_id;
    let path = path(&parent, provider);
    assert!(
        combined(&parent, provider, &|| Ok(false))
            .unwrap()
            .is_none()
    );
    assert!(!path.exists());
    let directory = PrivateDirectory::open_or_create(&path).unwrap();
    directory
        .write_atomic("unexpected.nrt", b"retained", PublishMode::CreateNew)
        .unwrap();
    let result = combined(&parent, provider, &|| {
        panic!("corrupt child is not absence")
    });
    assert!(result.is_err());
    assert_eq!(
        directory.entries(2).unwrap(),
        vec![std::ffi::OsString::from("unexpected.nrt")]
    );
    assert!(!path.join("operation.lock").exists());
    std::fs::remove_file(path.join("unexpected.nrt")).unwrap();
    // An exact empty crash prefix remains genuine absence and can be resumed.
    let child = combined(&parent, provider, &|| Ok(true)).unwrap().unwrap();
    assert_eq!(
        child.directory.identity().unwrap(),
        directory.identity().unwrap()
    );
}

#[test]
fn bootstrap_admission_closes_parent_after_denied_permission_and_failed_creation() {
    let _guard = crate::managed::native_test_guard();
    let (_temporary, parent) = fixture();
    let provider = parent.manifest.providers[0].provider_id;
    let path = path(&parent, provider);
    let generation =
        PrivateDirectory::open_exact(parent.prepared.context.client_config.parent().unwrap())
            .unwrap();
    let original = generation.read("peer3.toml", 1024 * 1024).unwrap();
    let mut changed = zeroize::Zeroizing::new(original.to_vec());
    changed.push(0);
    for refusal_is_error in [false, true] {
        let result = combined(&parent, provider, &|| {
            generation.write_atomic("peer3.toml", &changed, PublishMode::Replace)?;
            if refusal_is_error {
                Err(invalid("creation permission refused"))
            } else {
                Ok(false)
            }
        });
        let error = result.err().unwrap();
        assert_ne!(
            error.to_string(),
            invalid("creation permission refused").to_string()
        );
        assert!(!path.exists());
        generation
            .write_atomic("peer3.toml", &original, PublishMode::Replace)
            .unwrap();
    }
    let result = combined(&parent, provider, &|| {
        generation.write_atomic("peer3.toml", &changed, PublishMode::Replace)?;
        Ok(true)
    });
    assert!(
        result.is_err(),
        "positive permission cannot authenticate changed source"
    );
    generation
        .write_atomic("peer3.toml", &original, PublishMode::Replace)
        .unwrap();
    // The ordinary creating producer may retain a native prefix before its full-image
    // refusal, but returns no authority. After restoration it remains safely recoverable.
    drop(combined(&parent, provider, &|| Ok(true)).unwrap().unwrap());
    std::fs::remove_dir_all(&path).unwrap();
    let calls = Cell::new(0);
    let (result, checks) = profile_validation_test_support::count(|| {
        combined(&parent, provider, &|| {
            calls.set(calls.get() + 1);
            let branch = PrivateDirectory::open_or_create(path.parent().unwrap())?;
            branch.write_atomic(
                ProviderPurpose::Custody.directory_name(),
                b"not a directory",
                PublishMode::CreateNew,
            )?;
            Ok(true)
        })
    });
    assert!(result.is_err());
    assert_eq!(calls.get(), 1);
    assert_eq!(
        checks, 2,
        "failed creation still closes the original parent"
    );
    assert!(!path.join("operation.lock").exists());
    std::fs::remove_file(&path).unwrap();
    let hook_generation = generation.retain().unwrap();
    let hook_changed = changed.clone();
    let hook = ServiceAuthority::test_on_creating_child_exit(move || {
        hook_generation
            .write_atomic("peer3.toml", &hook_changed, PublishMode::Replace)
            .unwrap();
    });
    assert!(combined(&parent, provider, &|| Ok(true)).is_err());
    drop(hook);
    generation
        .write_atomic("peer3.toml", &original, PublishMode::Replace)
        .unwrap();
    // Parent-exit refusal drops the independently locked child; a fresh native retry
    // recovers that exact purpose and does not request creation permission again.
    drop(
        combined(&parent, provider, &|| panic!("child already exists"))
            .unwrap()
            .unwrap(),
    );
    parent.validate_profile().unwrap();
}

#[test]
fn bootstrap_admission_keeps_exact_active_decode_and_owned_parent_producers() {
    let _guard = crate::managed::native_test_guard();
    let (_temporary, parent) = fixture();
    let provider = parent.manifest.providers[0].provider_id;
    let path = path(&parent, provider);
    drop(PrivateDirectory::open_or_create(path.parent().unwrap()).unwrap());
    let budget = DecodeBudgetContext::new(limits(64 * 1024 * 1024));
    let (baseline, parses) = parsed(|| budget.with(|| original_recipe(&parent, provider)));
    assert_eq!(parses, 2);
    let baseline = baseline.unwrap();
    assert!(matches!(&baseline.profile, AuthorityProfile::Owned(_)));
    let charge = usize::try_from(budget.consumed_allocated_bytes()).unwrap();
    assert!(charge > 0);
    drop(baseline);
    std::fs::remove_dir_all(&path).unwrap();
    let exact = DecodeBudgetContext::new(limits(charge));
    let (actual, parses) = parsed(|| {
        exact.with(|| {
            combined(&parent, provider, &|| {
                parent.validate_profile()?;
                Ok(true)
            })
        })
    });
    let actual = actual.unwrap().unwrap();
    assert_eq!(parses, 2);
    assert!(matches!(&actual.profile, AuthorityProfile::Owned(_)));
    assert_eq!(exact.consumed_allocated_bytes(), charge as u64);
    drop(actual);
    std::fs::remove_dir_all(&path).unwrap();
    for allocation in [0, charge - 1] {
        let baseline_budget = DecodeBudgetContext::new(limits(allocation));
        let baseline = baseline_budget
            .with(|| original_recipe(&parent, provider))
            .err()
            .unwrap();
        let actual_budget = DecodeBudgetContext::new(limits(allocation));
        let actual = actual_budget
            .with(|| {
                combined(&parent, provider, &|| {
                    parent.validate_profile()?;
                    Ok(true)
                })
            })
            .err()
            .unwrap();
        assert_eq!(actual.to_string(), baseline.to_string());
        assert_eq!(
            actual_budget.consumed_allocated_bytes(),
            baseline_budget.consumed_allocated_bytes()
        );
        // Admission may have created only a native prefix before budget refusal.
        if path.exists() {
            std::fs::remove_dir_all(&path).unwrap();
        }
    }
    let prepared = parent.prepared.clone();
    drop(parent);
    let entry = DecodeBudgetContext::new(limits(64 * 1024 * 1024));
    let parent = entry
        .with(|| {
            ServiceAuthority::open_network_existing(&prepared, NetworkPurpose::ServiceBootstrap)
        })
        .unwrap()
        .unwrap();
    assert!(matches!(&parent.profile, AuthorityProfile::Owned(_)));
    let (child, parses) = parsed(|| combined(&parent, provider, &|| Ok(true)));
    assert_eq!(
        parses, 2,
        "owned absence plus creation keep both standalone captures"
    );
    drop(child.unwrap().unwrap());
}
