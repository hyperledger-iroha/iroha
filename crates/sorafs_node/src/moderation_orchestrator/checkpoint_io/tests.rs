//! Portable and native custody controls for the actual moderation checkpoint cache owner.

use super::*;
use std::fs;

fn fixture() -> (tempfile::TempDir, CheckpointCache) {
    let temporary = tempfile::tempdir().unwrap();
    let cache = CheckpointCache::open(&temporary.path().join("cache/checkpoint")).unwrap();
    (temporary, cache)
}

fn path(cache: &CheckpointCache) -> std::path::PathBuf {
    cache.directory.path().join(&cache.name)
}

#[test]
fn checkpoint_cache_roundtrip_absence_and_exact_bounds() {
    let (_temporary, cache) = fixture();
    assert_eq!(cache.read(8).unwrap(), None);
    cache.write(b"original", 8).unwrap();
    assert_eq!(
        cache.read(8).unwrap().as_deref(),
        Some(b"original".as_slice())
    );
    assert_eq!(cache.read(7), Err(checkpoint_bound(7)));
    assert_eq!(cache.write(b"oversized", 8), Err(checkpoint_bound(8)));
    assert_eq!(fs::read(path(&cache)).unwrap(), b"original");
    cache.write(b"", 0).unwrap();
    assert_eq!(cache.read(0).unwrap(), Some(Vec::new()));
}

#[test]
fn checkpoint_cache_rejects_hardlinks_without_repair_or_replacement() {
    let (_temporary, cache) = fixture();
    cache.write(b"original", 16).unwrap();
    let alias = cache.directory.path().join("alias");
    fs::hard_link(path(&cache), &alias).unwrap();
    assert!(matches!(
        cache.read(16),
        Err(ModerationOrchestratorError::CheckpointIo(_))
    ));
    assert!(matches!(
        cache.write(b"changed", 16),
        Err(ModerationOrchestratorError::CheckpointIo(_))
    ));
    assert_eq!(fs::read(path(&cache)).unwrap(), b"original");
    assert_eq!(fs::read(&alias).unwrap(), b"original");
    fs::remove_file(alias).unwrap();
    assert_eq!(cache.read(16).unwrap().unwrap(), b"original");
}

#[test]
fn checkpoint_cache_errors_before_publication_preserve_the_previous_file() {
    let (_temporary, cache) = fixture();
    cache.write(b"original", 16).unwrap();
    for failed in [
        AtomicWriteStep::CreateTemp,
        AtomicWriteStep::WriteHead,
        AtomicWriteStep::WriteTail,
        AtomicWriteStep::SyncFile,
        AtomicWriteStep::Publish,
    ] {
        let result = cache.write_observed(b"successor", 16, |step, _| {
            if step == failed {
                Err(io::Error::other("injected pre-publication failure"))
            } else {
                Ok(())
            }
        });
        assert!(
            matches!(result, Err(ModerationOrchestratorError::CheckpointIo(_))),
            "{failed:?}: {result:?}"
        );
        assert_eq!(cache.read(16).unwrap().unwrap(), b"original");
        assert_eq!(
            cache.directory.entries(2).unwrap(),
            vec![cache.name.clone()]
        );
    }
}

#[test]
fn checkpoint_cache_errors_after_publication_retain_complete_uncertain_bytes() {
    for failed in [AtomicWriteStep::Published, AtomicWriteStep::SyncDirectory] {
        let (_temporary, cache) = fixture();
        cache.write(b"original", 16).unwrap();
        let result = cache.write_observed(b"successor", 16, |step, _| {
            if step == failed {
                Err(io::Error::other("injected post-publication failure"))
            } else {
                Ok(())
            }
        });
        assert!(
            matches!(
                result,
                Err(ModerationOrchestratorError::CheckpointDurabilityUncertain(
                    _
                ))
            ),
            "{failed:?}: {result:?}"
        );
        assert_eq!(cache.read(16).unwrap().unwrap(), b"successor");
        assert_eq!(
            cache.directory.entries(2).unwrap(),
            vec![cache.name.clone()]
        );
    }
}

#[test]
fn checkpoint_cache_refuses_a_competing_equal_byte_publication_at_commit() {
    let (_temporary, cache) = fixture();
    cache.write(b"original", 16).unwrap();
    let before = FileMetadata::symlink_metadata(path(&cache))
        .unwrap()
        .identity();
    let mut competing = None;
    let result = cache.write_observed(b"successor", 16, |step, _| {
        if step == AtomicWriteStep::Publish {
            cache
                .directory
                .write_atomic(&cache.name, b"original", PublishMode::Replace)?;
            competing = Some(FileMetadata::symlink_metadata(path(&cache))?.identity());
        }
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ModerationOrchestratorError::CheckpointIo(_))
    ));
    assert_ne!(competing, Some(before));
    assert_eq!(
        Some(
            FileMetadata::symlink_metadata(path(&cache))
                .unwrap()
                .identity()
        ),
        competing
    );
    assert_eq!(cache.read(16).unwrap().unwrap(), b"original");
    assert_eq!(
        cache.directory.entries(2).unwrap(),
        vec![cache.name.clone()]
    );
}

#[test]
fn checkpoint_cache_readback_rejects_equal_bytes_from_a_different_original() {
    let (_temporary, cache) = fixture();
    cache.write(b"original", 16).unwrap();
    let old = cache.open_original().unwrap().unwrap().identity().unwrap();
    // A separately published original has equal content, but cannot close an earlier write.
    cache.write(b"original", 16).unwrap();
    assert!(matches!(
        cache.read_observed(16, Some(old), |_| Ok(())),
        Err(ModerationOrchestratorError::CheckpointIo(_))
    ));
}

#[cfg(unix)]
#[test]
fn checkpoint_cache_preserves_safe_parent_readers_and_rejects_public_leaf() {
    use std::os::unix::fs::PermissionsExt as _;
    let temporary = tempfile::tempdir().unwrap();
    fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o755)).unwrap();
    let cache = CheckpointCache::open(&temporary.path().join("checkpoint")).unwrap();
    cache.write(b"private", 16).unwrap();
    assert_eq!(
        fs::metadata(temporary.path()).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert_eq!(
        fs::metadata(path(&cache)).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::set_permissions(path(&cache), fs::Permissions::from_mode(0o644)).unwrap();
    assert!(cache.read(16).is_err());
    assert!(cache.write(b"changed", 16).is_err());
    assert_eq!(
        fs::metadata(path(&cache)).unwrap().permissions().mode() & 0o777,
        0o644
    );
    assert_eq!(fs::read(path(&cache)).unwrap(), b"private");
}

#[cfg(unix)]
#[test]
fn checkpoint_cache_rejects_displaced_parent_instead_of_admitting_absence() {
    let (temporary, cache) = fixture();
    cache.write(b"original", 16).unwrap();
    let displaced = temporary.path().join("displaced");
    fs::rename(cache.directory.path(), &displaced).unwrap();
    fs::create_dir(cache.directory.path()).unwrap();
    assert!(matches!(
        cache.read(16),
        Err(ModerationOrchestratorError::CheckpointIo(_))
    ));
    assert!(matches!(
        cache.write(b"changed", 16),
        Err(ModerationOrchestratorError::CheckpointIo(_))
    ));
    assert!(!path(&cache).exists());
    assert_eq!(fs::read(displaced.join(&cache.name)).unwrap(), b"original");
}

#[cfg(unix)]
#[test]
fn checkpoint_cache_rejects_replaced_original_during_read() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_temporary, cache) = fixture();
    cache.write(b"original", 16).unwrap();
    let result = cache.read_observed(16, None, |_| {
        fs::rename(path(&cache), cache.directory.path().join("preserved"))?;
        fs::write(path(&cache), b"original")?;
        fs::set_permissions(path(&cache), fs::Permissions::from_mode(0o600))
    });
    assert!(matches!(
        result,
        Err(ModerationOrchestratorError::CheckpointIo(_))
    ));
    assert_eq!(
        fs::read(cache.directory.path().join("preserved")).unwrap(),
        b"original"
    );
}

#[cfg(windows)]
fn everyone_access(path: &Path, permission: Option<&str>) {
    let mut command = std::process::Command::new("icacls");
    command.arg(path);
    match permission {
        Some(permission) => {
            command.arg("/grant").arg(format!("*S-1-1-0:{permission}"));
        }
        None => {
            command.args(["/remove:g", "*S-1-1-0"]);
        }
    }
    assert!(command.status().unwrap().success());
}

#[cfg(windows)]
#[test]
fn checkpoint_cache_windows_rejects_leaf_acl_and_parent_write_drift_without_repair() {
    let (_temporary, cache) = fixture();
    cache.write(b"original", 16).unwrap();
    let leaf = path(&cache);
    for (changed, permission) in [(leaf.as_path(), "R"), (cache.directory.path(), "W")] {
        everyone_access(changed, Some(permission));
        assert!(cache.read(16).is_err());
        assert!(cache.write(b"changed", 16).is_err());
        assert_eq!(fs::read(&leaf).unwrap(), b"original");
        everyone_access(changed, None);
    }
    assert_eq!(cache.read(16).unwrap().unwrap(), b"original");
}

#[cfg(windows)]
#[test]
fn checkpoint_cache_windows_retains_directory_and_excludes_original_writers() {
    let (temporary, cache) = fixture();
    cache.write(b"original", 16).unwrap();
    assert!(fs::rename(cache.directory.path(), temporary.path().join("displaced")).is_err());
    let leaf = path(&cache);
    let bytes = cache
        .read_observed(16, None, |_| {
            assert!(fs::OpenOptions::new().write(true).open(&leaf).is_err());
            assert!(fs::remove_file(&leaf).is_err());
            assert!(fs::rename(&leaf, leaf.with_extension("moved")).is_err());
            Ok(())
        })
        .unwrap()
        .unwrap();
    assert_eq!(bytes, b"original");
    fs::write(&leaf, b"changed").unwrap();
    assert_eq!(cache.read(16).unwrap().unwrap(), b"changed");
}

#[cfg(windows)]
#[test]
fn checkpoint_cache_windows_rejects_junction_parent_without_touching_target() {
    let (temporary, cache) = fixture();
    cache.write(b"original", 16).unwrap();
    let alias = temporary.path().join("alias");
    assert!(
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&alias)
            .arg(cache.directory.path())
            .status()
            .unwrap()
            .success()
    );
    assert!(CheckpointCache::open(&alias.join("checkpoint")).is_err());
    assert_eq!(cache.read(16).unwrap().unwrap(), b"original");
}
