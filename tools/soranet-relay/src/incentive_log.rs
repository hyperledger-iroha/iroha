//! Persistence helper for relay incentive snapshots.
//!
//! Writes Norito-encoded `RelayEpochMetricsV1` payloads to a spool directory so
//! offline auditors can replay the incentive pipeline deterministically.

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use iroha_data_model::soranet::incentives::RelayEpochMetricsV1;
use norito::{
    codec::Encode,
    derive::{JsonDeserialize, JsonSerialize},
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::error::RelayError;

/// Errors surfaced while persisting incentive snapshots.
#[derive(Debug, Error)]
pub enum IncentiveLogError {
    #[error("incentive log I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to encode incentive snapshot: {0}")]
    Encode(String),
}

impl From<IncentiveLogError> for RelayError {
    fn from(err: IncentiveLogError) -> Self {
        RelayError::Logging(err.to_string())
    }
}

/// Configuration for incentive snapshot persistence.
#[derive(Debug, Clone, PartialEq, Eq, Default, JsonDeserialize, JsonSerialize)]
pub struct IncentiveLogConfig {
    /// Whether incentive snapshots should be written to disk.
    pub enable: bool,
    /// Optional spool directory; defaults to `artifacts/incentives` when enabled.
    pub spool_dir: Option<PathBuf>,
}

impl IncentiveLogConfig {
    const DEFAULT_SPOOL_DIR: &'static str = "artifacts/incentives";

    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_defaults(&mut self) {
        if self.enable && self.spool_dir.is_none() {
            self.spool_dir = Some(PathBuf::from(Self::DEFAULT_SPOOL_DIR));
        }
    }

    pub fn as_logger(
        &self,
        relay_id_hex: &str,
    ) -> Result<Option<IncentiveLogger>, IncentiveLogError> {
        if !self.enable {
            return Ok(None);
        }
        let dir = self
            .spool_dir
            .clone()
            .expect("defaults ensure spool_dir when enabled");
        Ok(Some(IncentiveLogger::new(dir, relay_id_hex)?))
    }
}

/// Writes Norito-encoded incentive snapshots to disk.
#[derive(Debug)]
pub struct IncentiveLogger {
    spool_dir: PathBuf,
    relay_id_hex: String,
    seen: Mutex<BTreeMap<u32, [u8; 32]>>,
}

impl IncentiveLogger {
    fn new(spool_dir: PathBuf, relay_id_hex: &str) -> Result<Self, IncentiveLogError> {
        fs::create_dir_all(&spool_dir)?;
        Ok(Self {
            spool_dir,
            relay_id_hex: relay_id_hex.to_owned(),
            seen: Mutex::new(BTreeMap::new()),
        })
    }

    /// Persist a snapshot if it has changed since the last write.
    pub fn write_snapshot(&self, metrics: &RelayEpochMetricsV1) -> Result<(), IncentiveLogError> {
        let payload = metrics.encode();
        let digest_bytes: [u8; 32] = Sha256::digest(&payload).into();

        {
            let mut guard = self.seen.lock().expect("incentive log cache poisoned");
            if guard
                .get(&metrics.epoch)
                .map(|d| d == &digest_bytes)
                .unwrap_or(false)
            {
                return Ok(());
            }
            guard.insert(metrics.epoch, digest_bytes);
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros();

        let mut attempt = 0u32;
        loop {
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

            match OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&candidate)
            {
                Ok(mut file) => {
                    file.write_all(&payload)?;
                    return Ok(());
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    attempt = attempt.saturating_add(1);
                    continue;
                }
                Err(err) => return Err(IncentiveLogError::Io(err)),
            }
        }
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

    #[test]
    fn deduplicates_snapshots() {
        let dir = TempDir::new().expect("spool dir");
        let logger = IncentiveLogger::new(dir.path().to_path_buf(), "abcd").expect("create logger");
        let metrics = sample_metrics(1);

        logger.write_snapshot(&metrics).expect("first write");
        logger.write_snapshot(&metrics).expect("deduplicated write");

        let files: Vec<_> = fs::read_dir(dir.path())
            .expect("read dir")
            .collect::<Result<_, _>>()
            .expect("entries");
        assert_eq!(files.len(), 1);
    }
}
