//! Telemetry for development rather than production purposes
use crate::integrity::ChainState;
#[cfg(unix)]
use crate::integrity::{TELEMETRY_O_NOFOLLOW_FLAG, process_uid_in_directory};
use chrono::Utc;
use eyre::{Result, WrapErr, eyre};
use iroha_config::parameters::actual::TelemetryIntegrity;
use iroha_futures::FuturePollTelemetry;
use iroha_logger::telemetry::Event as Telemetry;
use std::path::{Path, PathBuf};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    sync::broadcast::Receiver,
    task::{self, JoinHandle},
};
/// Starts telemetry writing to a file. Will create all parent directories.
///
/// # Errors
/// Fails if unable to open the file
pub async fn start_file_output(
    path: PathBuf,
    integrity: TelemetryIntegrity,
    mut telemetry: Receiver<Telemetry>,
) -> Result<JoinHandle<()>> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    tokio::fs::create_dir_all(parent).await.wrap_err_with(|| {
        eyre!(
            "failed to recursively create directories for the dev telemetry output file: {}",
            path.display()
        )
    })?;
    let (mut file, canonical_path) = open_dev_output(&path)?;
    let mut chain = ChainState::new_with_kind(
        integrity,
        "dev",
        canonical_path.as_os_str().as_encoded_bytes(),
    )?;
    let join_handle = task::spawn(async move {
        loop {
            if chain.pending_record().is_some()
                && let Err(error) = resolve_pending(&mut file, &mut chain).await
            {
                iroha_logger::error!(%error, "failed to resolve pending dev telemetry record");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
            let event = match telemetry.recv().await {
                Ok(event) => event,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    iroha_logger::warn!(%skipped, "dev telemetry channel lagged; dropped events");
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };
            let Ok(mut item) = FuturePollTelemetry::try_from(event) else {
                continue;
            };
            item.name.retain(|character| !character.is_whitespace());
            if let Err(error) = write_telemetry(&mut file, &item, &mut chain).await {
                iroha_logger::error!(%error, "failed to write telemetry");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    });
    Ok(join_handle)
}

#[cfg(unix)]
fn open_dev_output(path: &Path) -> Result<(File, PathBuf)> {
    use std::{
        fs::{self, OpenOptions},
        os::unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    };

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let canonical_parent = fs::canonicalize(parent)
        .wrap_err_with(|| format!("canonicalize dev telemetry directory {}", parent.display()))?;
    let parent_metadata =
        fs::metadata(&canonical_parent).wrap_err("inspect telemetry directory")?;
    if !parent_metadata.is_dir() || parent_metadata.mode() & 0o022 != 0 {
        return Err(eyre!(
            "dev telemetry directory must be owned by the process user and not writable by group or others: {}",
            canonical_parent.display()
        ));
    }
    let effective_uid =
        process_uid_in_directory(&canonical_parent).map_err(|message| eyre!(message))?;
    if parent_metadata.uid() != effective_uid {
        return Err(eyre!(
            "dev telemetry directory must be owned by the process user and not writable by group or others: {}",
            canonical_parent.display()
        ));
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| eyre!("dev telemetry output path must name a file"))?;
    let canonical_path = canonical_parent.join(file_name);
    let named_before = match fs::symlink_metadata(&canonical_path) {
        Ok(metadata) => {
            validate_dev_output_metadata(&canonical_path, &metadata, effective_uid)?;
            Some(metadata)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error).wrap_err("inspect dev telemetry output"),
    };

    let mut options = OpenOptions::new();
    options
        .read(true)
        .append(true)
        .create(true)
        .mode(0o600)
        .custom_flags(TELEMETRY_O_NOFOLLOW_FLAG);
    let file = options
        .open(&canonical_path)
        .wrap_err_with(|| format!("open dev telemetry output {}", canonical_path.display()))?;
    let opened = file
        .metadata()
        .wrap_err("inspect opened dev telemetry output")?;
    validate_dev_output_metadata(&canonical_path, &opened, effective_uid)?;
    let named_after =
        fs::symlink_metadata(&canonical_path).wrap_err("reinspect dev telemetry output")?;
    validate_dev_output_metadata(&canonical_path, &named_after, effective_uid)?;
    if opened.dev() != named_after.dev() || opened.ino() != named_after.ino() {
        return Err(eyre!(
            "dev telemetry output changed while it was being opened"
        ));
    }
    if let Some(before) = named_before
        && (before.dev() != opened.dev() || before.ino() != opened.ino())
    {
        return Err(eyre!(
            "dev telemetry output changed while it was being opened"
        ));
    }
    file.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => {
            eyre!(
                "dev telemetry output is already in use: {}",
                canonical_path.display()
            )
        }
        std::fs::TryLockError::Error(error) => {
            eyre!(
                "failed to lock dev telemetry output {}: {error}",
                canonical_path.display()
            )
        }
    })?;
    Ok((File::from_std(file), canonical_path))
}

#[cfg(unix)]
fn validate_dev_output_metadata(
    path: &Path,
    metadata: &std::fs::Metadata,
    effective_uid: u32,
) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;

    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != effective_uid
        || metadata.mode() & 0o777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(eyre!(
            "dev telemetry output must be an owner-held, mode 0600, single-link regular file: {}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn open_dev_output(_path: &Path) -> Result<(File, PathBuf)> {
    Err(eyre!(
        "secure dev telemetry file output is currently supported only on Unix"
    ))
}
async fn write_telemetry(
    file: &mut File,
    item: &FuturePollTelemetry,
    integrity: &mut ChainState,
) -> Result<()> {
    let mut record = match norito::json::to_value(&item)
        .wrap_err("failed to serialize telemetry to JSON value")?
    {
        norito::json::Value::Object(map) => map,
        _ => return Err(eyre!("dev telemetry must serialize to an object")),
    };
    record.insert("ts".into(), Utc::now().to_rfc3339().into());
    integrity.stage_record(record, true).await?;
    resolve_pending(file, integrity).await
}

async fn resolve_pending(file: &mut File, integrity: &mut ChainState) -> Result<()> {
    if !integrity.pending_is_durable() {
        integrity.persist_pending().await?;
    }
    if integrity.pending_output_is_confirmed() {
        integrity.commit_pending().await?;
        return Ok(());
    }
    let bytes = integrity
        .pending_record()
        .ok_or_else(|| eyre!("no dev telemetry record is pending"))?
        .to_vec();
    write_or_recover_record(file, &bytes).await?;
    integrity.confirm_pending_output()?;
    integrity.commit_pending().await?;
    Ok(())
}

async fn write_or_recover_record(file: &mut File, record: &[u8]) -> Result<()> {
    let file_len = file
        .metadata()
        .await
        .wrap_err("failed to inspect telemetry output file")?
        .len();
    let record_len = u64::try_from(record.len()).unwrap_or(u64::MAX);
    let tail_len = usize::try_from(file_len.min(record_len))
        .wrap_err("telemetry output tail length does not fit usize")?;
    let mut tail = vec![0_u8; tail_len];
    if tail_len != 0 {
        let offset = i64::try_from(tail_len).wrap_err("telemetry tail offset does not fit i64")?;
        file.seek(std::io::SeekFrom::End(-offset))
            .await
            .wrap_err("failed to seek telemetry output tail")?;
        file.read_exact(&mut tail)
            .await
            .wrap_err("failed to read telemetry output tail")?;
    }

    if tail.ends_with(record) {
        file.sync_data()
            .await
            .wrap_err("failed to sync recovered telemetry output")?;
        return Ok(());
    }

    let max_partial = record.len().saturating_sub(1).min(tail.len());
    let partial_len = (1..=max_partial)
        .rev()
        .find(|length| tail.ends_with(&record[..*length]))
        .unwrap_or(0);
    if partial_len != 0 {
        let partial_len = u64::try_from(partial_len).unwrap_or(u64::MAX);
        file.set_len(file_len.saturating_sub(partial_len))
            .await
            .wrap_err("failed to truncate partial telemetry record")?;
    }
    file.seek(std::io::SeekFrom::End(0))
        .await
        .wrap_err("failed to seek telemetry output end")?;
    file.write_all(record)
        .await
        .wrap_err("failed to write telemetry record")?;
    file.sync_data()
        .await
        .wrap_err("failed to sync telemetry output")?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use norito::json::Value;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::{fs::OpenOptions, io::AsyncWriteExt};

    fn integrity_config() -> TelemetryIntegrity {
        TelemetryIntegrity {
            enabled: true,
            state_dir: None,
            signing_key: None,
            signing_key_id: None,
        }
    }

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "iroha-dev-telemetry-{label}-{}.log",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }

    fn test_chain() -> ChainState {
        ChainState::new_with_state_path(integrity_config(), None, "dev-test", b"test-output")
            .expect("initialize telemetry integrity chain")
    }

    #[tokio::test]
    async fn dev_output_includes_chain() {
        let path = temp_path("chain");
        let mut file = File::create(&path).await.expect("create file");
        let mut chain = test_chain();
        let item = FuturePollTelemetry {
            id: 1,
            name: "test".to_string(),
            duration: 42,
        };
        write_telemetry(&mut file, &item, &mut chain)
            .await
            .expect("write telemetry");
        file.flush().await.expect("flush file");
        let contents = tokio::fs::read_to_string(&path).await.expect("read file");
        let line = contents.lines().next().expect("telemetry line");
        let value: Value = norito::json::from_str(line).expect("parse JSON");
        let map = value.as_object().expect("object");
        assert!(map.contains_key("chain"));
        assert!(map.contains_key("ts"));
        assert_eq!(map.get("name").and_then(Value::as_str), Some("test"));
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn recovery_does_not_duplicate_complete_pending_record() {
        let path = temp_path("complete-recovery");
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .append(true)
            .create(true)
            .open(&path)
            .await
            .expect("open file");
        let mut chain = test_chain();
        let mut record = norito::json::Map::new();
        record.insert("name".into(), Value::from("recovered"));
        chain
            .stage_record(record, true)
            .await
            .expect("stage record");
        let pending = chain.pending_record().expect("pending").to_vec();
        file.write_all(&pending)
            .await
            .expect("write complete record");
        file.sync_data().await.expect("sync complete record");

        resolve_pending(&mut file, &mut chain)
            .await
            .expect("resolve complete record");
        assert_eq!(tokio::fs::read(&path).await.expect("read file"), pending);
        assert!(chain.pending_record().is_none());
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn recovery_replaces_partial_pending_record() {
        let path = temp_path("partial-recovery");
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .append(true)
            .create(true)
            .open(&path)
            .await
            .expect("open file");
        let existing = b"{\"previous\":true}\n";
        file.write_all(existing)
            .await
            .expect("write existing record");
        let mut chain = test_chain();
        let mut record = norito::json::Map::new();
        record.insert("name".into(), Value::from("partial"));
        chain
            .stage_record(record, true)
            .await
            .expect("stage record");
        let pending = chain.pending_record().expect("pending").to_vec();
        let prefix_len = pending.len() / 2;
        file.write_all(&pending[..prefix_len])
            .await
            .expect("write partial record");
        file.flush().await.expect("flush partial record");

        resolve_pending(&mut file, &mut chain)
            .await
            .expect("resolve partial record");
        let mut expected = existing.to_vec();
        expected.extend_from_slice(&pending);
        assert_eq!(tokio::fs::read(&path).await.expect("read file"), expected);
        assert!(chain.pending_record().is_none());
        let _ = std::fs::remove_file(path);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn output_file_is_private_and_exclusively_locked() {
        use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _};

        let dir = temp_path("secure-output-dir").with_extension("");
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        builder
            .create(&dir)
            .expect("create private output directory");
        let path = dir.join("telemetry.jsonl");
        let (sender, receiver) = tokio::sync::broadcast::channel(1);
        let handle = start_file_output(path.clone(), integrity_config(), receiver)
            .await
            .expect("start first writer");
        assert_eq!(
            std::fs::metadata(&path).expect("output metadata").mode() & 0o777,
            0o600
        );
        let second = start_file_output(path.clone(), integrity_config(), sender.subscribe())
            .await
            .expect_err("second writer must be rejected");
        assert!(second.to_string().contains("already in use"));

        drop(sender);
        handle.await.expect("stop first writer");
        let (sender, receiver) = tokio::sync::broadcast::channel(1);
        let resumed = start_file_output(path, integrity_config(), receiver)
            .await
            .expect("lock must release with writer");
        drop(sender);
        resumed.await.expect("stop resumed writer");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unsafe_output_files_are_rejected() {
        use std::os::unix::fs::{OpenOptionsExt as _, symlink};

        let dir = temp_path("unsafe-output-dir").with_extension("");
        std::fs::create_dir(&dir).expect("create output directory");
        let broad = dir.join("broad.jsonl");
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o644)
            .open(&broad)
            .expect("create broad output");
        let (_sender, receiver) = tokio::sync::broadcast::channel(1);
        assert!(
            start_file_output(broad, integrity_config(), receiver)
                .await
                .is_err()
        );

        let target = dir.join("target.jsonl");
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&target)
            .expect("create symlink target");
        let link = dir.join("link.jsonl");
        symlink(&target, &link).expect("create symlink");
        let (_sender, receiver) = tokio::sync::broadcast::channel(1);
        assert!(
            start_file_output(link, integrity_config(), receiver)
                .await
                .is_err()
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
