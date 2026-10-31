//! Retained native webhook storage; shared registry and delivery policy remain in the parent.

use super::{
    BoundedWebhookFile, OsStr, OsString, QueueScanBatch, WEBHOOK_QUEUE_ADMISSION_SCAN_WORK_ITEMS,
    WEBHOOK_QUEUE_SCAN_BATCH_SIZE, WEBHOOK_QUEUE_SCAN_WORK_ITEMS, WEBHOOK_TEMP_FILE_RETRIES,
    WebhookDirectory, WebhookFileIdentity, WebhookPolicy, WebhookPublication,
    effective_queue_capacity, filename_in_webhook_directory, invalid_webhook_storage,
    is_webhook_temporary_name, lock_unpoisoned, open_webhook_queue_directory, publication_matches,
    queue_dir,
};
use iroha_fs::{AtomicWriteStep, FileIdentity, FileSnapshot, OwnerDirectory, PrivateDirectory};
use std::{
    io::{self, Read as _, Write as _},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

fn finish<T>(result: io::Result<T>, final_check: io::Result<()>) -> io::Result<T> {
    result.and_then(|value| final_check.map(|()| value))
}

impl WebhookDirectory {
    fn native_revalidate(&self) -> io::Result<()> {
        let original = self.native.revalidate();
        let private = self
            .private
            .as_ref()
            .map_or(Ok(()), PrivateDirectory::revalidate);
        original.and(private)
    }

    pub(super) fn native_sync(&self) -> io::Result<()> {
        let result = self.native.sync();
        let final_check = self.native_revalidate();
        result.and(final_check)
    }
}

pub(super) fn open_webhook_directory_chain(
    path: &Path,
    create: bool,
    private_final: bool,
) -> io::Result<Option<WebhookDirectory>> {
    let opened = if create {
        OwnerDirectory::open_or_create(path)
    } else {
        OwnerDirectory::open(path)
    };
    let native = match opened {
        Ok(directory) => directory,
        Err(error) if !create && error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let admitted = (|| {
        // Existing ACLs are admitted, never tightened. Keep both originals: the project
        // root may permit readers, while the queue must remain owner-private.
        let private = if private_final {
            let private = PrivateDirectory::open(native.path())?;
            if private.identity()? != native.identity()? {
                return Err(invalid_webhook_storage("webhook queue identity changed"));
            }
            Some(private)
        } else {
            None
        };
        Ok(private)
    })();
    let final_check = native.revalidate();
    let private = finish(admitted, final_check)?;
    let directory = WebhookDirectory {
        path: native.path().to_path_buf(),
        native,
        private,
    };
    directory.native_revalidate()?;
    Ok(Some(directory))
}

pub(super) fn read_private_webhook_file_bounded(
    directory: &WebhookDirectory,
    path: &Path,
    maximum_bytes: usize,
) -> io::Result<Option<BoundedWebhookFile>> {
    let mut original = None;
    let mut snapshot = None;
    let result = (|| {
        directory.native_revalidate()?;
        let name = filename_in_webhook_directory(directory, path)?;
        original = directory.native.open_retained_regular_optional(name)?;
        let Some(original) = original.as_ref() else {
            return Ok(None);
        };
        let before = FileSnapshot::of(original.file(), true)?;
        snapshot = Some(before);
        let length = usize::try_from(original.file().metadata()?.len())
            .map_err(|_| invalid_webhook_storage("webhook record extent does not fit"))?;
        if length > maximum_bytes {
            return Err(invalid_webhook_storage(
                "webhook record exceeds its byte bound",
            ));
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(io::Error::other)?;
        original
            .file()
            .take(
                u64::try_from(length)
                    .map_err(io::Error::other)?
                    .checked_add(1)
                    .ok_or_else(|| invalid_webhook_storage("webhook record bound overflows"))?,
            )
            .read_to_end(&mut bytes)?;
        if bytes.len() != length || FileSnapshot::of(original.file(), true)? != before {
            return Err(invalid_webhook_storage(
                "webhook record changed during bounded read",
            ));
        }
        Ok(Some(BoundedWebhookFile {
            bytes,
            identity: WebhookFileIdentity(before),
        }))
    })();
    let file_final = original.as_ref().map_or(Ok(()), |original| {
        let private = FileSnapshot::of(original.file(), true).and_then(|after| {
            if snapshot.is_some_and(|before| before != after) {
                Err(invalid_webhook_storage("webhook private original changed"))
            } else {
                Ok(())
            }
        });
        let retained = original.revalidate();
        private.and(retained)
    });
    let directory_final = directory.native_revalidate();
    finish(result, file_final.and(directory_final))
}

pub(super) fn write_private_webhook_file_atomic(
    directory: &WebhookDirectory,
    path: &Path,
    bytes: &[u8],
    maximum_bytes: usize,
    publication: WebhookPublication,
) -> io::Result<()> {
    write_observed(directory, path, bytes, maximum_bytes, publication, |_| {
        Ok(())
    })
}

fn write_observed(
    directory: &WebhookDirectory,
    path: &Path,
    bytes: &[u8],
    maximum_bytes: usize,
    publication: WebhookPublication,
    mut observe: impl FnMut(AtomicWriteStep) -> io::Result<()>,
) -> io::Result<()> {
    use rand::TryRngCore as _;

    let mut guarded_observe = |step| {
        let result = observe(step);
        let final_check = directory.native_revalidate();
        result.and(final_check)
    };

    let mut predecessor = None;
    let mut predecessor_snapshot = None;
    let mut predecessor_identity = None;
    let mut original = None;
    let mut temporary_name = None;
    let mut created_identity = None;
    let mut published = false;
    let name = filename_in_webhook_directory(directory, path)?;
    let result = (|| {
        directory.native_revalidate()?;
        if bytes.len() > maximum_bytes {
            return Err(invalid_webhook_storage(
                "webhook persistence payload exceeds its byte bound",
            ));
        }
        predecessor = directory.native.open_private_journal_optional(name)?;
        predecessor_snapshot = predecessor
            .as_ref()
            .map(FileSnapshot::private_journal)
            .transpose()?;
        predecessor_identity = predecessor.as_ref().map(FileIdentity::of).transpose()?;
        if !publication_matches(publication, predecessor_snapshot.map(WebhookFileIdentity)) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "webhook destination changed",
            ));
        }
        for _ in 0..WEBHOOK_TEMP_FILE_RETRIES {
            let mut nonce = [0_u8; 16];
            rand::rngs::OsRng
                .try_fill_bytes(&mut nonce)
                .map_err(io::Error::other)?;
            temporary_name = Some(OsString::from(format!(
                ".webhook-{}.tmp",
                hex::encode(nonce)
            )));
            let temporary = temporary_name
                .as_ref()
                .ok_or_else(|| io::Error::other("missing webhook staging name"))?;
            guarded_observe(AtomicWriteStep::CreateTemp)?;
            match directory
                .native
                .create_private_journal_into(temporary, &mut original)
            {
                Ok(()) => break,
                Err(error)
                    if original.is_none() && error.kind() == io::ErrorKind::AlreadyExists =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
        let original = original.as_mut().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "webhook staging names exhausted",
            )
        })?;
        created_identity = Some(FileIdentity::of(original)?);
        let middle = bytes.len() / 2;
        guarded_observe(AtomicWriteStep::WriteHead)?;
        original.write_all(&bytes[..middle])?;
        guarded_observe(AtomicWriteStep::WriteTail)?;
        original.write_all(&bytes[middle..])?;
        guarded_observe(AtomicWriteStep::SyncFile)?;
        original.sync_all()?;
        if original.metadata()?.len() != bytes.len() as u64 {
            return Err(invalid_webhook_storage("webhook staging extent differs"));
        }
        let temporary = temporary_name
            .as_ref()
            .ok_or_else(|| io::Error::other("missing webhook staging name"))?;
        directory.native.publish_private_journal_observed(
            temporary,
            name,
            original,
            predecessor.as_ref(),
            &mut published,
            &mut guarded_observe,
        )?;
        guarded_observe(AtomicWriteStep::SyncDirectory)?;
        directory.native_sync()
    })();
    // A failed attempt retains its exact originals through all final checks. Before
    // publication a private named temporary may remain for the existing bounded recovery;
    // after publication the complete destination remains visible with uncertain durability.
    let original_final = (|| {
        if let Some(original) = original.as_ref() {
            FileSnapshot::private_journal(original)?;
            if let Some(identity) = created_identity {
                if FileIdentity::of(original)? != identity {
                    return Err(invalid_webhook_storage("webhook staging original changed"));
                }
            }
            let current_name = if published {
                name
            } else {
                temporary_name
                    .as_deref()
                    .ok_or_else(|| io::Error::other("missing webhook staging name"))?
            };
            directory.native.require_same_lock(current_name, original)?;
        }
        Ok(())
    })();
    let predecessor_final = (|| {
        if let Some(predecessor) = predecessor.as_ref() {
            if published {
                directory
                    .native
                    .require_retired_private_journal(predecessor)?;
                if let Some(identity) = predecessor_identity {
                    if FileIdentity::of(predecessor)? != identity {
                        return Err(invalid_webhook_storage(
                            "webhook retired predecessor changed",
                        ));
                    }
                }
            } else {
                directory.native.require_same_lock(name, predecessor)?;
                if let Some(snapshot) = predecessor_snapshot {
                    if FileSnapshot::private_journal(predecessor)? != snapshot {
                        return Err(invalid_webhook_storage("webhook predecessor changed"));
                    }
                }
            }
        }
        Ok(())
    })();
    let directory_final = directory.native_revalidate();
    result
        .and(original_final)
        .and(predecessor_final)
        .and(directory_final)
}

pub(super) fn unlink_private_webhook_entry(
    directory: &WebhookDirectory,
    path: &Path,
    expected: Option<WebhookFileIdentity>,
    sync: bool,
) -> io::Result<()> {
    unlink_observed(directory, path, expected, sync, |_| Ok(()))
}

fn unlink_observed(
    directory: &WebhookDirectory,
    path: &Path,
    expected: Option<WebhookFileIdentity>,
    sync: bool,
    mut observe: impl FnMut(AtomicWriteStep) -> io::Result<()>,
) -> io::Result<()> {
    let mut original = None;
    let mut disposed = false;
    let mut identity = None;
    let name = filename_in_webhook_directory(directory, path)?;
    let result = (|| {
        directory.native_revalidate()?;
        original = directory.native.open_private_journal_optional(name)?;
        let Some(original) = original.as_ref() else {
            return Ok(());
        };
        let actual = FileSnapshot::private_journal(original)?;
        if expected.is_some_and(|expected| expected.0 != actual) {
            return Err(invalid_webhook_storage(
                "refusing to remove a replaced webhook entry",
            ));
        }
        identity = Some(FileIdentity::of(original)?);
        directory
            .native
            .retire_private_journal(name, original, &mut disposed)
    })();
    let original_final = (|| {
        if let Some(original) = original.as_ref() {
            if disposed {
                directory.native.require_retired_private_journal(original)?;
            } else {
                directory.native.require_same_lock(name, original)?;
            }
            if let Some(identity) = identity {
                if FileIdentity::of(original)? != identity {
                    return Err(invalid_webhook_storage("webhook removal original changed"));
                }
            }
        }
        Ok(())
    })();
    drop(original);
    // A requested barrier remains required when a retry observes initial absence.
    // Also flush every actual disposition, including best-effort batch removals:
    // their shared caller may log an error without counting the removed name.
    let barrier = if disposed || sync {
        observe(AtomicWriteStep::SyncDirectory).and_then(|()| directory.native_sync())
    } else {
        Ok(())
    };
    let directory_final = directory.native_revalidate();
    result.and(original_final).and(barrier).and(directory_final)
}

pub(super) fn cleanup_webhook_temporary_files(
    directory: &WebhookDirectory,
    work_limit: usize,
) -> io::Result<usize> {
    cleanup_observed(directory, work_limit, |_| Ok(()))
}

fn cleanup_observed(
    directory: &WebhookDirectory,
    work_limit: usize,
    mut observe: impl FnMut(AtomicWriteStep) -> io::Result<()>,
) -> io::Result<usize> {
    let result = (|| {
        let entries = directory.native.entries(work_limit)?;
        let mut removed = 0;
        for name in entries {
            if is_webhook_temporary_name(&name) {
                unlink_private_webhook_entry(directory, &directory.path.join(name), None, false)?;
                removed += 1;
            }
        }
        Ok(removed)
    })();
    // A prior recovery may have removed its last temporary before a failed barrier.
    // Reissue it even for an empty sweep, and on errors after earlier dispositions.
    let barrier = observe(AtomicWriteStep::SyncDirectory).and_then(|()| directory.native_sync());
    let final_check = directory.native_revalidate();
    finish(result, barrier.and(final_check))
}

pub(super) fn queue_depth_bounded_in(
    directory: &WebhookDirectory,
    maximum: usize,
    work_limit: usize,
) -> io::Result<usize> {
    let result = directory.native.entries(work_limit).map(|entries| {
        entries
            .iter()
            .filter(|name| Path::new(name).extension() == Some(OsStr::new("json")))
            .count()
            .min(maximum)
    });
    let final_check = directory.native_revalidate();
    finish(result, final_check)
}

struct ScanState {
    root: PathBuf,
    capacity: usize,
    directory: Arc<WebhookDirectory>,
    entries: std::vec::IntoIter<OsString>,
    retained: usize,
}

fn cursor() -> &'static Mutex<Option<ScanState>> {
    static CURSOR: OnceLock<Mutex<Option<ScanState>>> = OnceLock::new();
    CURSOR.get_or_init(|| Mutex::new(None))
}

pub(super) fn discover_queue_batch(policy: WebhookPolicy) -> io::Result<QueueScanBatch> {
    let root = queue_dir();
    let capacity = effective_queue_capacity(policy);
    let mut cursor = lock_unpoisoned(cursor());
    if cursor
        .as_ref()
        .is_none_or(|state| state.root != root || state.capacity != capacity)
    {
        if let Some(previous) = cursor.take() {
            previous.directory.native_revalidate()?;
        }
        let directory = Arc::new(
            open_webhook_queue_directory(true)?
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?,
        );
        // The existing recovery/admission ceiling is twice the 10,000-entry hard
        // queue cap. Never silently truncate a larger or malformed population.
        let entries = directory
            .native
            .entries(WEBHOOK_QUEUE_ADMISSION_SCAN_WORK_ITEMS);
        let final_check = directory.native_revalidate();
        let entries = finish(entries, final_check)?;
        *cursor = Some(ScanState {
            root,
            capacity,
            directory,
            entries: entries.into_iter(),
            retained: 0,
        });
    }
    let state = cursor
        .as_mut()
        .ok_or_else(|| io::Error::other("missing webhook scan state"))?;
    let directory = Arc::clone(&state.directory);
    let result = (|| {
        directory.native_revalidate()?;
        let mut paths = Vec::with_capacity(WEBHOOK_QUEUE_SCAN_BATCH_SIZE);
        let mut overflow_paths = Vec::new();
        let mut sweep_complete = false;
        for _ in 0..WEBHOOK_QUEUE_SCAN_WORK_ITEMS {
            if paths.len() + overflow_paths.len() == WEBHOOK_QUEUE_SCAN_BATCH_SIZE {
                break;
            }
            let Some(name) = state.entries.next() else {
                sweep_complete = true;
                break;
            };
            if Path::new(&name).extension() != Some(OsStr::new("json")) {
                continue;
            }
            let path = directory.path.join(name);
            if state.retained < capacity {
                state.retained += 1;
                paths.push(path);
            } else {
                overflow_paths.push(path);
            }
        }
        Ok(QueueScanBatch {
            directory: Some(Arc::clone(&directory)),
            paths,
            overflow_paths,
            sweep_complete,
        })
    })();
    let final_check = directory.native_revalidate();
    let result = finish(result, final_check);
    if match &result {
        Ok(batch) => batch.sweep_complete,
        Err(_) => true,
    } {
        *cursor = None;
    }
    result
}

#[cfg(test)]
mod tests;
