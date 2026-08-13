//! Persistence helper for relay incentive snapshots.
//!
//! Writes Norito-encoded `RelayEpochMetricsV1` payloads to a spool directory so
//! offline auditors can replay the incentive pipeline deterministically.
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use iroha_data_model::soranet::incentives::RelayEpochMetricsV1;
use norito::{
    core::to_bytes_bounded,
    derive::{JsonDeserialize, JsonSerialize},
};
use sha2::{Digest, Sha256};
use thiserror::Error;
use crate::{
    error::RelayError,
    incentives::{
        INCENTIVE_DEFAULT_ACTIVE_EPOCHS, INCENTIVE_DEFAULT_MEASUREMENTS_PER_EPOCH,
        INCENTIVE_MAX_ACTIVE_EPOCHS_V1, INCENTIVE_MAX_RETAINED_MEASUREMENTS_V1,
    },
};
/// First-release maximum encoded incentive snapshot size.
pub const INCENTIVE_SNAPSHOT_MAX_BYTES_V1: usize = 4 * 1024 * 1024;
/// Errors surfaced while persisting incentive snapshots.
#[derive(Debug, Error)]
pub enum IncentiveLogError {
    #[error("incentive log I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to encode incentive snapshot: {0}")]
    Encode(String),
    #[error("incentive configuration error: {0}")]
    Config(String),
    #[error("incentive snapshot capacity error: {0}")]
    Capacity(String),
    #[error("incentive snapshot digest cache is poisoned")]
    Poisoned,
}
impl From<IncentiveLogError> for RelayError {
    fn from(err: IncentiveLogError) -> Self {
        RelayError::Logging(err.to_string())
    }
}
/// Configuration for incentive snapshot persistence.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct IncentiveLogConfig {
    /// Whether incentive snapshots should be written to disk.
    pub enable: bool,
    /// Optional spool directory; defaults to `artifacts/incentives` when enabled.
    pub spool_dir: Option<PathBuf>,
    /// Maximum simultaneously retained accumulator epochs and logger digests.
    #[norito(default = "IncentiveLogConfig::default_max_active_epochs")]
    pub max_active_epochs: usize,
    /// Maximum distinct measurement IDs retained in any one epoch.
    #[norito(default = "IncentiveLogConfig::default_max_measurements_per_epoch")]
    pub max_measurements_per_epoch: usize,
}
impl Default for IncentiveLogConfig {
    fn default() -> Self {
        Self {
            enable: false,
            spool_dir: None,
            max_active_epochs: Self::default_max_active_epochs(),
            max_measurements_per_epoch: Self::default_max_measurements_per_epoch(),
        }
    }
}
impl IncentiveLogConfig {
    const DEFAULT_SPOOL_DIR: &'static str = "artifacts/incentives";
    const fn default_max_active_epochs() -> usize {
        INCENTIVE_DEFAULT_ACTIVE_EPOCHS
    }
    const fn default_max_measurements_per_epoch() -> usize {
        INCENTIVE_DEFAULT_MEASUREMENTS_PER_EPOCH
    }
    pub fn new() -> Self {
        Self::default()
    }
    pub fn apply_defaults(&mut self) {
        if self.enable && self.spool_dir.is_none() {
            self.spool_dir = Some(PathBuf::from(Self::DEFAULT_SPOOL_DIR));
        }
        if self.max_active_epochs == 0 {
            self.max_active_epochs = Self::default_max_active_epochs();
        }
        if self.max_measurements_per_epoch == 0 {
            self.max_measurements_per_epoch = Self::default_max_measurements_per_epoch();
        }
    }
    /// Applies defaults and validates the first-release retained-memory
    /// geometry.
    pub fn validate(&mut self) -> Result<(), IncentiveLogError> {
        self.apply_defaults();
        if self.max_active_epochs > INCENTIVE_MAX_ACTIVE_EPOCHS_V1 {
            return Err(IncentiveLogError::Config(format!(
                "incentives.max_active_epochs ({}) exceeds the first-release limit of {INCENTIVE_MAX_ACTIVE_EPOCHS_V1}",
                self.max_active_epochs
            )));
        }
        if self.max_measurements_per_epoch > INCENTIVE_MAX_RETAINED_MEASUREMENTS_V1 {
            return Err(IncentiveLogError::Config(format!(
                "incentives.max_measurements_per_epoch ({}) exceeds the first-release limit of {INCENTIVE_MAX_RETAINED_MEASUREMENTS_V1}",
                self.max_measurements_per_epoch
            )));
        }
        let retained = self
            .max_active_epochs
            .checked_mul(self.max_measurements_per_epoch)
            .ok_or_else(|| {
                IncentiveLogError::Config(
                    "incentive retained-measurement geometry overflowed".to_owned(),
                )
            })?;
        if retained > INCENTIVE_MAX_RETAINED_MEASUREMENTS_V1 {
            return Err(IncentiveLogError::Config(format!(
                "incentives.max_active_epochs * incentives.max_measurements_per_epoch ({retained}) exceeds the aggregate first-release limit of {INCENTIVE_MAX_RETAINED_MEASUREMENTS_V1}"
            )));
        }
        Ok(())
    }
    pub fn as_logger(
        &self,
        relay_id_hex: &str,
    ) -> Result<Option<IncentiveLogger>, IncentiveLogError> {
        let mut config = self.clone();
        config.validate()?;
        if !config.enable {
            return Ok(None);
        }
        let dir = config
            .spool_dir
            .ok_or_else(|| IncentiveLogError::Config("incentive spool path is missing".into()))?;
        Ok(Some(IncentiveLogger::new(
            dir,
            relay_id_hex,
            config.max_active_epochs,
            config.max_measurements_per_epoch,
        )?))
    }
}
/// Writes Norito-encoded incentive snapshots to disk.
#[derive(Debug)]
pub struct IncentiveLogger {
    spool_dir: PathBuf,
    relay_id_hex: String,
    max_seen_epochs: usize,
    max_measurements_per_epoch: usize,
    /// Sorted, fully preallocated `(epoch, digest)` cache.
    seen: Mutex<Vec<(u32, [u8; 32])>>,
}
impl IncentiveLogger {
    fn new(
        spool_dir: PathBuf,
        relay_id_hex: &str,
        max_seen_epochs: usize,
        max_measurements_per_epoch: usize,
    ) -> Result<Self, IncentiveLogError> {
        if relay_id_hex.len() != 64
            || !relay_id_hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(IncentiveLogError::Config(
                "relay incentive log identifier must be exactly 64 lowercase hexadecimal bytes"
                    .to_owned(),
            ));
        }
        let max_seen_epochs = max_seen_epochs.clamp(1, INCENTIVE_MAX_ACTIVE_EPOCHS_V1);
        let mut seen = Vec::new();
        seen.try_reserve_exact(max_seen_epochs).map_err(|_| {
            IncentiveLogError::Capacity("digest cache allocation failed".to_owned())
        })?;
        let mut relay_id = String::new();
        relay_id
            .try_reserve_exact(relay_id_hex.len())
            .map_err(|_| {
                IncentiveLogError::Capacity("relay identifier allocation failed".into())
            })?;
        relay_id.push_str(relay_id_hex);
        fs::create_dir_all(&spool_dir)?;
        Ok(Self {
            spool_dir,
            relay_id_hex: relay_id,
            max_seen_epochs,
            max_measurements_per_epoch: max_measurements_per_epoch
                .clamp(1, INCENTIVE_MAX_RETAINED_MEASUREMENTS_V1),
            seen: Mutex::new(seen),
        })
    }
    /// Persist a snapshot if it has changed since the last write.
    pub fn write_snapshot(&self, metrics: &RelayEpochMetricsV1) -> Result<(), IncentiveLogError> {
        if metrics.measurement_ids.len() > self.max_measurements_per_epoch {
            return Err(IncentiveLogError::Capacity(format!(
                "epoch {} contains {} measurement IDs; maximum is {}",
                metrics.epoch,
                metrics.measurement_ids.len(),
                self.max_measurements_per_epoch
            )));
        }
        let payload = to_bytes_bounded(metrics, INCENTIVE_SNAPSHOT_MAX_BYTES_V1)
            .map_err(|error| IncentiveLogError::Encode(error.to_string()))?;
        let digest_bytes: [u8; 32] = Sha256::digest(&payload).into();
        let mut guard = self.seen.lock().map_err(|_| IncentiveLogError::Poisoned)?;
        let cached_position = guard.binary_search_by_key(&metrics.epoch, |(epoch, _)| *epoch);
        if let Ok(index) = cached_position
            && guard[index].1 == digest_bytes
        {
            return Ok(());
        }
        if cached_position.is_err()
            && guard.len() == self.max_seen_epochs
            && metrics.epoch <= guard[0].0
        {
            return Err(IncentiveLogError::Capacity(format!(
                "epoch {} is outside the newest {} cached epochs",
                metrics.epoch, self.max_seen_epochs
            )));
        }
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros();
        let mut temporary = tempfile::Builder::new()
            .prefix(".iroha-incentive-snapshot-")
            .tempfile_in(&self.spool_dir)?;
        temporary.write_all(&payload)?;
        temporary.as_file().sync_all()?;
        const MAX_CREATE_ATTEMPTS: u32 = 1_024;
        for attempt in 0..MAX_CREATE_ATTEMPTS {
            let mut candidate = self.spool_dir.clone();
            let suffix = if attempt == 0 {
                format!(
                    "relay-{}-epoch-{}-{}.to",
                    self.relay_id_hex, metrics.epoch, timestamp
                )
            } else {
                format!(
                    "relay-{}-epoch-{}-{}-{}.to",
                    self.relay_id_hex, metrics.epoch, timestamp, attempt
                )
            };
            candidate.push(suffix);
            match temporary.persist_noclobber(&candidate) {
                Ok(_) => {
                    #[cfg(unix)]
                    fs::File::open(&self.spool_dir)?.sync_all()?;
                    match cached_position {
                        Ok(index) => guard[index].1 = digest_bytes,
                        Err(mut index) => {
                            if guard.len() == self.max_seen_epochs {
                                guard.remove(0);
                                index = guard
                                    .binary_search_by_key(&metrics.epoch, |(epoch, _)| *epoch)
                                    .unwrap_or_else(|index| index);
                            }
                            guard.insert(index, (metrics.epoch, digest_bytes));
                        }
                    }
                    return Ok(());
                }
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                    temporary = error.file;
                    continue;
                }
                Err(error) => return Err(IncentiveLogError::Io(error.error)),
            }
        }
        Err(IncentiveLogError::Capacity(format!(
            "failed to allocate a unique incentive snapshot name after {MAX_CREATE_ATTEMPTS} attempts"
        )))
    }
}
#[cfg(test)]
mod tests {
    use iroha_data_model::{
        metadata::Metadata,
        soranet::incentives::{RelayComplianceStatusV1, RelayEpochMetricsV1},
    };
    use tempfile::TempDir;
    use super::*;
    fn sample_metrics(epoch: u32) -> RelayEpochMetricsV1 {
        RelayEpochMetricsV1 {
            relay_id: [0x11; 32],
            epoch,
            uptime_seconds: 90,
            scheduled_uptime_seconds: 120,
            verified_bandwidth_bytes: 1_024,
            compliance: RelayComplianceStatusV1::Clean,
            reward_score: 0,
            confidence_floor_per_mille: 875,
            measurement_ids: vec![[0xAA; 32]],
            metadata: Metadata::default(),
        }
    }
    fn logger(
        dir: &TempDir,
        max_seen_epochs: usize,
        max_measurements_per_epoch: usize,
    ) -> IncentiveLogger {
        IncentiveLogger::new(
            dir.path().to_path_buf(),
            &"ab".repeat(32),
            max_seen_epochs,
            max_measurements_per_epoch,
        )
        .expect("create logger")
    }
    #[test]
    fn deduplicates_snapshots() {
        let dir = TempDir::new().expect("spool dir");
        let logger = logger(&dir, 2, 2);
        let metrics = sample_metrics(1);
        logger.write_snapshot(&metrics).expect("first write");
        logger.write_snapshot(&metrics).expect("deduplicated write");
        let files: Vec<_> = fs::read_dir(dir.path())
            .expect("read dir")
            .collect::<Result<_, _>>()
            .expect("entries");
        assert_eq!(files.len(), 1);
    }
    #[test]
    fn digest_cache_retains_the_newest_exact_epoch_window() {
        let dir = TempDir::new().expect("spool dir");
        let logger = logger(&dir, 2, 2);
        for epoch in 1..=3 {
            logger
                .write_snapshot(&sample_metrics(epoch))
                .expect("newer epoch must rotate bounded digest cache");
        }
        let guard = logger.seen.lock().expect("digest cache");
        assert_eq!(
            guard.iter().map(|(epoch, _)| *epoch).collect::<Vec<_>>(),
            vec![2, 3]
        );
        drop(guard);
        assert!(matches!(
            logger.write_snapshot(&sample_metrics(1)),
            Err(IncentiveLogError::Capacity(message)) if message.contains("newest 2")
        ));
    }
    #[test]
    fn measurement_overflow_rejects_before_file_creation() {
        let dir = TempDir::new().expect("spool dir");
        let logger = logger(&dir, 2, 1);
        let mut metrics = sample_metrics(1);
        metrics.measurement_ids.push([0xBB; 32]);
        assert!(matches!(
            logger.write_snapshot(&metrics),
            Err(IncentiveLogError::Capacity(message)) if message.contains("maximum is 1")
        ));
        assert_eq!(fs::read_dir(dir.path()).expect("read spool").count(), 0);
    }
}
