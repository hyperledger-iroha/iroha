//! Retained NTFS custody for the gateway compliance checkpoint and its lifetime lease.

use super::*;

impl GatewayComplianceStore for FileGatewayComplianceStore {
    fn try_acquire(&self) -> Result<Box<dyn GatewayComplianceStoreLease>, GatewayComplianceError> {
        self.directory
            .revalidate()
            .map_err(|error| persistence_io("revalidate checkpoint directory", error))?;
        let lock_path = checkpoint_lock_path(&self.path)?;
        let lock_name = lock_path.file_name().ok_or_else(|| {
            GatewayComplianceError::Persistence("checkpoint lease has no filename".into())
        })?;
        let file = self
            .directory
            .open_lock(lock_name)
            .map_err(|error| persistence_io("open private checkpoint lease", error))?;
        let snapshot = iroha_fs::FileSnapshot::of(&file, true)
            .map_err(|error| persistence_io("inspect checkpoint lease custody", error))?;
        match file.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => return Err(GatewayComplianceError::LeaseHeld),
            Err(fs::TryLockError::Error(error)) => {
                return Err(persistence_io("lock checkpoint lease", error));
            }
        }
        let lease = FileGatewayComplianceStoreLease {
            path: self.path.clone(),
            max_bytes: self.max_bytes,
            _lock_file: file,
            directory: Arc::clone(&self.directory),
            lock_snapshot: snapshot,
            #[cfg(test)]
            test_hook: Arc::clone(&self.test_hook),
        };
        lease.validate_custody()?;
        Ok(Box::new(lease))
    }
}

impl FileGatewayComplianceStoreLease {
    fn validate_custody(&self) -> Result<(), GatewayComplianceError> {
        let lock_path = checkpoint_lock_path(&self.path)?;
        self.directory
            .require_same_lock(
                lock_path.file_name().ok_or_else(|| {
                    GatewayComplianceError::Persistence("checkpoint lease has no filename".into())
                })?,
                &self._lock_file,
            )
            .map_err(|error| persistence_io("revalidate checkpoint lease", error))?;
        if iroha_fs::FileSnapshot::of(&self._lock_file, true)
            .map_err(|error| persistence_io("reinspect checkpoint lease", error))?
            != self.lock_snapshot
        {
            return Err(GatewayComplianceError::Persistence(
                "checkpoint lease custody changed".into(),
            ));
        }
        self.directory
            .revalidate()
            .map_err(|error| persistence_io("revalidate checkpoint directory", error))
    }

    fn read_current(&self) -> Result<GatewayComplianceStoreSnapshot, GatewayComplianceError> {
        self.validate_custody()?;
        let name = self.path.file_name().ok_or_else(|| {
            GatewayComplianceError::Persistence("checkpoint has no filename".into())
        })?;
        let result = (|| {
            let Some(mut original) = self
                .directory
                .open_retained_regular_optional(name)
                .map_err(|error| persistence_io("open checkpoint original", error))?
            else {
                return Ok(checkpoint_store_snapshot(None));
            };
            let before = original
                .snapshot()
                .map_err(|error| persistence_io("inspect checkpoint original", error))?;
            let length = usize::try_from(
                original
                    .file()
                    .metadata()
                    .map_err(|error| persistence_io("inspect checkpoint extent", error))?
                    .len(),
            )
            .unwrap_or(usize::MAX);
            if length > self.max_bytes {
                return Err(GatewayComplianceError::ResourceLimit {
                    resource: "compliance checkpoint bytes",
                    found: length,
                    maximum: self.max_bytes,
                });
            }
            let mut bytes = Vec::with_capacity(length);
            Read::by_ref(original.file_mut())
                .take(
                    u64::try_from(self.max_bytes)
                        .unwrap_or(u64::MAX)
                        .saturating_add(1),
                )
                .read_to_end(&mut bytes)
                .map_err(|error| persistence_io("read bounded checkpoint", error))?;
            if bytes.len() > self.max_bytes {
                return Err(GatewayComplianceError::ResourceLimit {
                    resource: "compliance checkpoint bytes",
                    found: bytes.len(),
                    maximum: self.max_bytes,
                });
            }
            if bytes.len() != length
                || before
                    != original
                        .snapshot()
                        .map_err(|error| persistence_io("reinspect checkpoint original", error))?
            {
                return Err(GatewayComplianceError::Persistence(
                    "compliance checkpoint changed during bounded read".into(),
                ));
            }
            self.validate_custody()?;
            Ok(checkpoint_store_snapshot(Some(bytes)))
        })();
        self.validate_custody()?;
        result
    }
}

impl GatewayComplianceStoreLease for FileGatewayComplianceStoreLease {
    fn load(&self) -> Result<GatewayComplianceStoreSnapshot, GatewayComplianceError> {
        self.read_current()
    }

    fn compare_and_store(
        &self,
        expected: GatewayComplianceStoreGeneration,
        bytes: &[u8],
    ) -> Result<GatewayComplianceStoreGeneration, GatewayComplianceError> {
        if bytes.len() > self.max_bytes {
            return Err(GatewayComplianceError::ResourceLimit {
                resource: "compliance checkpoint bytes",
                found: bytes.len(),
                maximum: self.max_bytes,
            });
        }
        if self.read_current()?.generation != expected {
            return Err(GatewayComplianceError::CheckpointConflict);
        }
        let name = self.path.file_name().ok_or_else(|| {
            GatewayComplianceError::Persistence("checkpoint has no filename".into())
        })?;
        let mut published = false;
        let mut conflict = false;
        let result = self.directory.write_atomic_observed(
            name,
            bytes,
            iroha_fs::PublishMode::Replace,
            32,
            |step, _| {
                if step == iroha_fs::AtomicWriteStep::Published {
                    published = true;
                }
                self.validate_custody().map_err(io::Error::other)?;
                if step == iroha_fs::AtomicWriteStep::Publish {
                    #[cfg(test)]
                    if let Some(replacement) = self
                        .test_hook
                        .before_persist_replacement
                        .lock()
                        .map_err(|_| io::Error::other("checkpoint test hook poisoned"))?
                        .take()
                    {
                        fs::write(&self.path, replacement)?;
                    }
                    if self.read_current().map_err(io::Error::other)?.generation != expected {
                        conflict = true;
                        return Err(io::Error::other("checkpoint generation changed"));
                    }
                }
                Ok(())
            },
        );
        if let Err(error) = result {
            return Err(if published || conflict {
                GatewayComplianceError::CheckpointConflict
            } else {
                persistence_io("publish private checkpoint", error)
            });
        }
        let durable = self
            .read_current()
            .map_err(|_| GatewayComplianceError::CheckpointConflict)?;
        let generation = checkpoint_store_generation(Some(bytes));
        if durable.generation != generation || durable.bytes.as_deref() != Some(bytes) {
            return Err(GatewayComplianceError::CheckpointConflict);
        }
        Ok(generation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let temporary = tempfile::tempdir().unwrap();
        let directory =
            iroha_fs::PrivateDirectory::open_or_create(temporary.path().join("state")).unwrap();
        let path = directory.path().join("checkpoint.to");
        (temporary, path)
    }

    fn change_everyone_access(path: &Path, access: Option<&str>) {
        let mut command = std::process::Command::new("icacls");
        command.arg(path);
        if let Some(access) = access {
            command.arg("/grant").arg(format!("*S-1-1-0:{access}"));
        } else {
            command.arg("/remove:g").arg("*S-1-1-0");
        }
        assert!(command.status().unwrap().success());
    }

    #[test]
    fn native_checkpoint_preserves_exact_cas_and_retained_parent() {
        let (temporary, path) = fixture();
        let store = FileGatewayComplianceStore::new(path.clone()).unwrap();
        let lease = store.try_acquire().unwrap();
        assert!(matches!(
            store.try_acquire(),
            Err(GatewayComplianceError::LeaseHeld)
        ));
        let first = lease
            .compare_and_store(GatewayComplianceStoreGeneration::Absent, b"first")
            .unwrap();
        assert_eq!(
            lease.load().unwrap().bytes.as_deref(),
            Some(b"first".as_slice())
        );
        assert!(fs::rename(path.parent().unwrap(), temporary.path().join("moved")).is_err());
        store.replace_before_next_persist(b"external".to_vec());
        assert!(matches!(
            lease.compare_and_store(first, b"next"),
            Err(GatewayComplianceError::CheckpointConflict)
        ));
        assert_eq!(fs::read(&path).unwrap(), b"external");
        drop(lease);
        let reopened = store.try_acquire().unwrap();
        assert_eq!(
            reopened.load().unwrap().bytes.as_deref(),
            Some(b"external".as_slice())
        );
    }

    #[test]
    fn native_checkpoint_rejects_lock_acl_and_new_hardlinks() {
        let (_temporary, path) = fixture();
        let store = FileGatewayComplianceStore::new(path.clone()).unwrap();
        let lease = store.try_acquire().unwrap();
        let lock_path = checkpoint_lock_path(&path).unwrap();
        change_everyone_access(&lock_path, Some("R"));
        assert!(matches!(
            lease.load(),
            Err(GatewayComplianceError::Persistence(_))
        ));
        drop(lease);
        assert!(matches!(
            store.try_acquire(),
            Err(GatewayComplianceError::Persistence(_))
        ));
        change_everyone_access(&lock_path, None);
        let lease = store.try_acquire().unwrap();
        let alias = path.with_file_name("lock-alias");
        fs::hard_link(&lock_path, &alias).unwrap();
        assert!(matches!(
            lease.load(),
            Err(GatewayComplianceError::Persistence(_))
        ));
        fs::remove_file(alias).unwrap();
    }

    #[test]
    fn native_checkpoint_accepts_readers_and_rejects_foreign_mutation() {
        let (_temporary, path) = fixture();
        let store = FileGatewayComplianceStore::new(path.clone()).unwrap();
        let lease = store.try_acquire().unwrap();
        lease
            .compare_and_store(GatewayComplianceStoreGeneration::Absent, b"catalog")
            .unwrap();
        change_everyone_access(&path, Some("R"));
        assert_eq!(
            lease.load().unwrap().bytes.as_deref(),
            Some(b"catalog".as_slice())
        );
        change_everyone_access(&path, Some("W"));
        assert!(matches!(
            lease.load(),
            Err(GatewayComplianceError::Persistence(_))
        ));
        change_everyone_access(&path, None);
        change_everyone_access(path.parent().unwrap(), Some("W"));
        assert!(matches!(
            lease.load(),
            Err(GatewayComplianceError::Persistence(_))
        ));
        change_everyone_access(path.parent().unwrap(), None);
    }

    #[test]
    fn native_checkpoint_read_pins_original_against_concurrent_writers_and_delete() {
        let (_temporary, path) = fixture();
        let store = FileGatewayComplianceStore::new(path.clone()).unwrap();
        let lease = store.try_acquire().unwrap();
        lease
            .compare_and_store(GatewayComplianceStoreGeneration::Absent, b"catalog")
            .unwrap();
        let original = store
            .directory
            .open_retained_regular_optional("checkpoint.to")
            .unwrap()
            .unwrap();
        assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
        assert!(fs::rename(&path, path.with_file_name("displaced")).is_err());
        assert!(fs::remove_file(&path).is_err());
        assert_eq!(
            lease.load().unwrap().bytes.as_deref(),
            Some(b"catalog".as_slice())
        );
        drop(original);
        fs::write(&path, b"updated").unwrap();
        assert_eq!(
            lease.load().unwrap().bytes.as_deref(),
            Some(b"updated".as_slice())
        );
    }

    #[test]
    fn native_checkpoint_bounds_before_allocation_and_rejects_equal_byte_substitution() {
        let (_temporary, path) = fixture();
        let store = FileGatewayComplianceStore::new(path.clone()).unwrap();
        let lease = store.try_acquire().unwrap();
        lease
            .compare_and_store(GatewayComplianceStoreGeneration::Absent, b"catalog")
            .unwrap();
        let file = store
            .directory
            .open_regular_optional("checkpoint.to")
            .unwrap()
            .unwrap();
        fs::rename(&path, path.with_file_name("displaced")).unwrap();
        store
            .directory
            .write_atomic(
                "checkpoint.to",
                b"catalog",
                iroha_fs::PublishMode::CreateNew,
            )
            .unwrap();
        assert!(
            store
                .directory
                .require_same_regular_file("checkpoint.to", &file)
                .is_err()
        );
        drop(file);
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(u64::try_from(store.max_bytes).unwrap() + 1)
            .unwrap();
        assert!(matches!(
            lease.load(),
            Err(GatewayComplianceError::ResourceLimit { .. })
        ));
    }
}
