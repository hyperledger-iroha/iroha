//! ISO 20022 reference-data ingestion and telemetry helpers.
//!
//! This module loads regulated identifier crosswalks (ISIN↔CUSIP, BIC↔LEI, MIC)
//! from operator-provided snapshots, captures provenance metadata, and exposes
//! ready-to-query maps for the Torii ISO bridge runtime. Each dataset is tagged
//! with refresh metadata and emits Prometheus metrics so operators can monitor
//! staleness or ingestion failures.

use core::convert::TryFrom;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

use eyre::{self, WrapErr as _};
use iroha_config::parameters::actual;
use iroha_logger::{error, info, warn};
use iroha_telemetry::metrics;
use ivm::iso20022::{self, IdentifierKind};
use norito::json::{self, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

/// Dataset kinds tracked by the ISO bridge reference-data loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatasetKind {
    /// ANNA / CUSIP crosswalk.
    IsinCusip,
    /// BIC to LEI mapping.
    BicLei,
    /// MIC directory.
    MicDirectory,
}

impl DatasetKind {
    /// Human-readable label used in logs and metrics.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            DatasetKind::IsinCusip => "isin_cusip",
            DatasetKind::BicLei => "bic_lei",
            DatasetKind::MicDirectory => "mic_directory",
        }
    }
}

/// Snapshot state capturing whether a dataset was ingested successfully.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotState {
    /// Dataset not provided (no path configured).
    Missing,
    /// Dataset loaded and parsed successfully.
    Loaded,
    /// Dataset ingestion failed due to IO/parse errors.
    Failed,
}

impl SnapshotState {
    /// Map the state to a numeric gauge value for telemetry.
    #[must_use]
    pub fn as_gauge(self) -> i64 {
        match self {
            SnapshotState::Missing => 0,
            SnapshotState::Loaded => 1,
            SnapshotState::Failed => -1,
        }
    }
}

/// Outcome of a reference-data validation attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationOutcome {
    /// Validation executed because the dataset was available.
    Enforced,
    /// Validation skipped due to the dataset not being configured.
    Skipped,
}

impl ValidationOutcome {
    /// Returns `true` when the validation was performed.
    #[must_use]
    pub fn is_enforced(self) -> bool {
        matches!(self, ValidationOutcome::Enforced)
    }
}

/// Errors that occur while validating ISO reference data records.
#[derive(Debug, Error)]
pub enum ReferenceDataError {
    #[error("{kind_label} dataset failed to load reference data: {diagnostics}", kind_label = .kind.label(), diagnostics = .diagnostics.as_deref().unwrap_or("unknown error"))]
    /// Reference data loader was unable to ingest the dataset.
    DatasetFailed {
        /// Dataset kind that failed.
        kind: DatasetKind,
        /// Loader diagnostics when available.
        diagnostics: Option<String>,
    },
    #[error("{kind_label} reference `{value}` not found in snapshot", kind_label = .kind.label())]
    /// Lookup for the requested reference value failed.
    NotFound {
        /// Dataset kind used for lookup.
        kind: DatasetKind,
        /// Identifier value that was queried.
        value: String,
    },
    #[error("MIC `{mic}` is inactive (status: {status:?})")]
    /// Market identifier code exists but is not currently active.
    MicInactive {
        /// MIC identifier considered inactive.
        mic: String,
        /// Optional upstream status string.
        status: Option<String>,
    },
}

/// Provenance metadata describing a reference-data snapshot.
#[derive(Debug, Clone)]
pub struct SnapshotMetadata {
    /// Upstream dataset version or publication identifier.
    pub version: String,
    /// Upstream data source (e.g., ANNA DSB, GLEIF).
    pub source: String,
    /// Upstream fetch timestamp (RFC3339). Optional when not supplied.
    pub fetched_at: Option<OffsetDateTime>,
    /// Number of records ingested for the dataset.
    pub record_count: usize,
}

impl SnapshotMetadata {
    fn age_seconds(&self) -> Option<u64> {
        self.fetched_at.map(|ts| {
            let now = OffsetDateTime::now_utc();
            let delta = now - ts;
            if delta.is_negative() {
                0
            } else {
                u64::try_from(delta.whole_seconds()).unwrap_or(0)
            }
        })
    }
}

/// Snapshot container describing the state of a particular dataset.
#[derive(Debug, Clone)]
pub struct DatasetSnapshot<T> {
    kind: DatasetKind,
    state: SnapshotState,
    metadata: Option<SnapshotMetadata>,
    records: Option<T>,
    diagnostics: Option<String>,
    configured_path: Option<PathBuf>,
}

impl<T> DatasetSnapshot<T> {
    fn missing(kind: DatasetKind) -> Self {
        Self {
            kind,
            state: SnapshotState::Missing,
            metadata: None,
            records: None,
            diagnostics: None,
            configured_path: None,
        }
    }

    fn failed(kind: DatasetKind, path: &Path, err: &eyre::Report) -> Self {
        Self {
            kind,
            state: SnapshotState::Failed,
            metadata: None,
            records: None,
            diagnostics: Some(err.to_string()),
            configured_path: Some(path.to_path_buf()),
        }
    }

    fn loaded(kind: DatasetKind, path: &Path, metadata: SnapshotMetadata, records: T) -> Self {
        Self {
            kind,
            state: SnapshotState::Loaded,
            metadata: Some(metadata),
            records: Some(records),
            diagnostics: None,
            configured_path: Some(path.to_path_buf()),
        }
    }

    /// Snapshot status.
    #[must_use]
    pub fn state(&self) -> SnapshotState {
        self.state
    }

    /// Snapshot metadata if the dataset loaded successfully.
    #[must_use]
    pub fn metadata(&self) -> Option<&SnapshotMetadata> {
        self.metadata.as_ref()
    }

    /// Loaded records when the dataset is available.
    #[must_use]
    pub fn records(&self) -> Option<&T> {
        self.records.as_ref()
    }

    /// Diagnostics message (error) when loading failed.
    #[must_use]
    pub fn diagnostics(&self) -> Option<&str> {
        self.diagnostics.as_deref()
    }

    /// Configured snapshot path when provided.
    #[must_use]
    pub fn configured_path(&self) -> Option<&Path> {
        self.configured_path.as_deref()
    }

    /// Dataset kind represented by this snapshot.
    #[must_use]
    pub fn kind(&self) -> DatasetKind {
        self.kind
    }

    fn log_status(&self) {
        match self.state {
            SnapshotState::Loaded => {
                if let Some(meta) = &self.metadata {
                    let fetched = meta
                        .fetched_at
                        .map(|ts| ts.format(&Rfc3339).unwrap_or_else(|_| ts.to_string()));
                    let fetched_str = fetched.as_deref().unwrap_or("n/a");
                    info!(
                        dataset = self.kind.label(),
                        version = meta.version.as_str(),
                        source = meta.source.as_str(),
                        records = meta.record_count,
                        fetched_at = fetched_str,
                        "ISO reference dataset loaded"
                    );
                } else {
                    info!(
                        dataset = self.kind.label(),
                        "ISO reference dataset loaded without metadata"
                    );
                }
            }
            SnapshotState::Missing => {
                warn!(
                    dataset = self.kind.label(),
                    "ISO reference dataset not provided; falling back to runtime defaults"
                );
            }
            SnapshotState::Failed => {
                error!(
                    dataset = self.kind.label(),
                    error = self.diagnostics.as_deref().unwrap_or("unknown"),
                    "ISO reference dataset failed to load"
                );
            }
        }
    }

    fn publish_metrics(&self, refresh_interval: Duration) {
        let metrics = metrics::global_or_default();
        let dataset = self.kind.label();
        let status = self.state.as_gauge();
        let (age_seconds, record_count) = match (&self.state, &self.metadata) {
            (SnapshotState::Loaded, Some(meta)) => (meta.age_seconds(), Some(meta.record_count)),
            _ => (None, None),
        };
        metrics.record_iso_reference_dataset(dataset, status, age_seconds, record_count);
        metrics
            .iso_reference_refresh_interval_secs
            .with_label_values(&[dataset])
            .set({
                let secs = refresh_interval.as_secs();
                let clamped = secs.min(i64::MAX as u64);
                i64::try_from(clamped).unwrap_or(i64::MAX)
            });
    }
}

/// In-memory snapshot cache for ISO 20022 reference data.
#[derive(Debug, Clone)]
pub struct ReferenceDataSnapshots {
    /// ISIN ↔ CUSIP crosswalk (ANNA).
    isin_cusip: DatasetSnapshot<InstrumentCrosswalk>,
    /// BIC ↔ LEI mapping (GLEIF).
    bic_lei: DatasetSnapshot<BicLeiCrosswalk>,
    /// MIC directory (SWIFT).
    mic_directory: DatasetSnapshot<MicDirectory>,
    /// Configured refresh interval.
    refresh_interval: Duration,
    /// Timestamp when the loader executed.
    loaded_at: OffsetDateTime,
}

impl ReferenceDataSnapshots {
    /// Build snapshots from the provided configuration.
    pub fn from_config(config: &actual::IsoReferenceData) -> Self {
        let now = OffsetDateTime::now_utc();

        let isin_snapshot = config.isin_crosswalk_path.as_deref().map_or_else(
            || DatasetSnapshot::missing(DatasetKind::IsinCusip),
            |path| match load_isin_crosswalk(path) {
                Ok((meta, records)) => {
                    DatasetSnapshot::loaded(DatasetKind::IsinCusip, path, meta, records)
                }
                Err(err) => DatasetSnapshot::failed(DatasetKind::IsinCusip, path, &err),
            },
        );

        let bic_lei_snapshot = config.bic_lei_path.as_deref().map_or_else(
            || DatasetSnapshot::missing(DatasetKind::BicLei),
            |path| match load_bic_lei_crosswalk(path) {
                Ok((meta, records)) => {
                    DatasetSnapshot::loaded(DatasetKind::BicLei, path, meta, records)
                }
                Err(err) => DatasetSnapshot::failed(DatasetKind::BicLei, path, &err),
            },
        );

        let mic_snapshot = config.mic_directory_path.as_deref().map_or_else(
            || DatasetSnapshot::missing(DatasetKind::MicDirectory),
            |path| match load_mic_directory(path) {
                Ok((meta, records)) => {
                    DatasetSnapshot::loaded(DatasetKind::MicDirectory, path, meta, records)
                }
                Err(err) => DatasetSnapshot::failed(DatasetKind::MicDirectory, path, &err),
            },
        );

        let snapshots = Self {
            isin_cusip: isin_snapshot,
            bic_lei: bic_lei_snapshot,
            mic_directory: mic_snapshot,
            refresh_interval: config.refresh_interval,
            loaded_at: now,
        };

        snapshots.log_statuses();
        snapshots.publish_metrics();
        if let Some(cache_dir) = config.cache_dir.as_deref()
            && let Err(err) = snapshots.persist_cache(cache_dir)
        {
            error!(
                directory = cache_dir.display().to_string().as_str(),
                error = err.to_string().as_str(),
                "failed to cache ISO reference dataset snapshots"
            );
        }
        snapshots
    }

    fn log_statuses(&self) {
        self.isin_cusip.log_status();
        self.bic_lei.log_status();
        self.mic_directory.log_status();
    }

    fn publish_metrics(&self) {
        self.isin_cusip.publish_metrics(self.refresh_interval);
        self.bic_lei.publish_metrics(self.refresh_interval);
        self.mic_directory.publish_metrics(self.refresh_interval);
    }

    fn persist_cache(&self, root: &Path) -> eyre::Result<()> {
        fs::create_dir_all(root).wrap_err_with(|| {
            format!("failed to create ISO cache directory at {}", root.display())
        })?;
        self.persist_dataset(root, self.isin_cusip())?;
        self.persist_dataset(root, self.bic_lei())?;
        self.persist_dataset(root, self.mic_directory())?;
        Ok(())
    }

    fn persist_dataset<T>(&self, root: &Path, snapshot: &DatasetSnapshot<T>) -> eyre::Result<()> {
        let dataset_dir = root.join(snapshot.kind().label());
        fs::create_dir_all(&dataset_dir).wrap_err_with(|| {
            format!(
                "failed to create cache directory for dataset {}",
                snapshot.kind().label()
            )
        })?;

        let status_path = dataset_dir.join("status.json");
        match snapshot.state() {
            SnapshotState::Loaded => {
                let meta = snapshot.metadata().ok_or_else(|| {
                    eyre::eyre!(
                        "dataset {} missing metadata despite Loaded state",
                        snapshot.kind().label()
                    )
                })?;
                self.persist_loaded_dataset(&dataset_dir, &status_path, snapshot, meta)?;
            }
            SnapshotState::Missing => {
                Self::persist_missing_dataset(&status_path, snapshot.kind().label())?;
            }
            SnapshotState::Failed => {
                Self::persist_failed_dataset(&status_path, snapshot)?;
            }
        }

        Ok(())
    }

    fn persist_loaded_dataset<T>(
        &self,
        dataset_dir: &Path,
        status_path: &Path,
        snapshot: &DatasetSnapshot<T>,
        meta: &SnapshotMetadata,
    ) -> eyre::Result<()> {
        let version = if meta.version.trim().is_empty() {
            format!("snapshot_{}", self.loaded_at.unix_timestamp())
        } else {
            meta.version.clone()
        };
        let sanitized_version = sanitize_path_component(&version);
        let data_filename = format!("{sanitized_version}.json");
        let metadata_filename = format!("{sanitized_version}.metadata.json");
        let cached_data_path = dataset_dir.join(&data_filename);

        let cached_sha256 = if let Some(source_path) = snapshot.configured_path() {
            if source_path != cached_data_path {
                fs::copy(source_path, &cached_data_path).wrap_err_with(|| {
                    format!(
                        "failed to copy {} to {}",
                        source_path.display(),
                        cached_data_path.display()
                    )
                })?;
            }
            Some(compute_sha256_hex(&cached_data_path)?)
        } else {
            None
        };

        let fetched_at = meta
            .fetched_at
            .map(|ts| ts.format(&Rfc3339).unwrap_or_else(|_| ts.to_string()));
        let mut metadata_map = json::Map::new();
        metadata_map.insert("status".to_owned(), Value::String("loaded".to_owned()));
        metadata_map.insert("version".to_owned(), Value::String(meta.version.clone()));
        metadata_map.insert("source".to_owned(), Value::String(meta.source.clone()));
        metadata_map.insert(
            "fetched_at".to_owned(),
            fetched_at.map_or(Value::Null, Value::String),
        );
        metadata_map.insert(
            "record_count".to_owned(),
            Value::from(meta.record_count as u64),
        );
        metadata_map.insert(
            "original_path".to_owned(),
            snapshot.configured_path().map_or(Value::Null, |path| {
                Value::String(path.display().to_string())
            }),
        );
        metadata_map.insert(
            "cached_path".to_owned(),
            cached_sha256.as_ref().map_or(Value::Null, |_| {
                Value::String(cached_data_path.display().to_string())
            }),
        );
        metadata_map.insert(
            "cached_sha256".to_owned(),
            cached_sha256
                .as_ref()
                .map_or(Value::Null, |sha| Value::String(sha.clone())),
        );
        let metadata_payload = Value::Object(metadata_map);
        fs::write(
            dataset_dir.join(metadata_filename),
            json::to_string_pretty(&metadata_payload)?,
        )
        .wrap_err_with(|| {
            format!(
                "failed to write metadata for dataset {}",
                snapshot.kind().label()
            )
        })?;

        let mut status_map = json::Map::new();
        status_map.insert("status".to_owned(), Value::String("loaded".to_owned()));
        status_map.insert(
            "latest_version".to_owned(),
            Value::String(meta.version.clone()),
        );
        status_map.insert(
            "cached_file".to_owned(),
            cached_sha256
                .as_ref()
                .map_or(Value::Null, |_| Value::String(data_filename)),
        );
        let status_payload = Value::Object(status_map);
        fs::write(status_path, json::to_string_pretty(&status_payload)?).wrap_err_with(|| {
            format!(
                "failed to write status file for dataset {}",
                snapshot.kind().label()
            )
        })?;

        Ok(())
    }

    fn persist_missing_dataset(status_path: &Path, label: &str) -> eyre::Result<()> {
        let mut status_map = json::Map::new();
        status_map.insert("status".to_owned(), Value::String("missing".to_owned()));
        let status_payload = Value::Object(status_map);
        fs::write(status_path, json::to_string_pretty(&status_payload)?)
            .wrap_err_with(|| format!("failed to write status file for dataset {label}"))?;
        Ok(())
    }

    fn persist_failed_dataset<T>(
        status_path: &Path,
        snapshot: &DatasetSnapshot<T>,
    ) -> eyre::Result<()> {
        let mut status_map = json::Map::new();
        status_map.insert("status".to_owned(), Value::String("failed".to_owned()));
        status_map.insert(
            "diagnostics".to_owned(),
            snapshot
                .diagnostics()
                .map_or(Value::Null, |diag| Value::String(diag.to_owned())),
        );
        status_map.insert(
            "path".to_owned(),
            snapshot.configured_path().map_or(Value::Null, |path| {
                Value::String(path.display().to_string())
            }),
        );
        let status_payload = Value::Object(status_map);
        fs::write(status_path, json::to_string_pretty(&status_payload)?).wrap_err_with(|| {
            format!(
                "failed to write status file for dataset {}",
                snapshot.kind().label()
            )
        })?;
        Ok(())
    }

    /// Access the ISIN ↔ CUSIP crosswalk snapshot.
    #[must_use]
    pub fn isin_cusip(&self) -> &DatasetSnapshot<InstrumentCrosswalk> {
        &self.isin_cusip
    }

    /// Access the BIC ↔ LEI crosswalk snapshot.
    #[must_use]
    pub fn bic_lei(&self) -> &DatasetSnapshot<BicLeiCrosswalk> {
        &self.bic_lei
    }

    /// Access the MIC directory snapshot.
    #[must_use]
    pub fn mic_directory(&self) -> &DatasetSnapshot<MicDirectory> {
        &self.mic_directory
    }

    /// Configured refresh interval.
    #[must_use]
    pub fn refresh_interval(&self) -> Duration {
        self.refresh_interval
    }

    /// Timestamp when the snapshots were last loaded.
    #[must_use]
    pub fn loaded_at(&self) -> OffsetDateTime {
        self.loaded_at
    }

    fn dataset_records_or_skip<T>(
        snapshot: &DatasetSnapshot<T>,
    ) -> Result<Option<&T>, ReferenceDataError> {
        match snapshot.state() {
            SnapshotState::Loaded => {
                let records = snapshot.records().expect("records present when loaded");
                Ok(Some(records))
            }
            SnapshotState::Missing => Ok(None),
            SnapshotState::Failed => Err(ReferenceDataError::DatasetFailed {
                kind: snapshot.kind,
                diagnostics: snapshot.diagnostics().map(ToOwned::to_owned),
            }),
        }
    }

    /// Validate that an ISIN appears in the crosswalk snapshot.
    ///
    /// # Errors
    /// Returns [`ReferenceDataError`] if the dataset is unavailable or the ISIN is unknown.
    pub fn validate_isin(&self, isin: &str) -> Result<ValidationOutcome, ReferenceDataError> {
        Self::dataset_records_or_skip(&self.isin_cusip)?.map_or_else(
            || Ok(ValidationOutcome::Skipped),
            |records| {
                if records.by_isin(isin).is_some() {
                    Ok(ValidationOutcome::Enforced)
                } else {
                    Err(ReferenceDataError::NotFound {
                        kind: DatasetKind::IsinCusip,
                        value: normalise_upper_ascii(isin),
                    })
                }
            },
        )
    }

    /// Validate that a CUSIP maps to a known ISIN.
    ///
    /// # Errors
    /// Returns [`ReferenceDataError`] if the dataset is unavailable or the CUSIP is unknown.
    pub fn validate_cusip(&self, cusip: &str) -> Result<ValidationOutcome, ReferenceDataError> {
        Self::dataset_records_or_skip(&self.isin_cusip)?.map_or_else(
            || Ok(ValidationOutcome::Skipped),
            |records| {
                if records.by_cusip(cusip).is_some() {
                    Ok(ValidationOutcome::Enforced)
                } else {
                    Err(ReferenceDataError::NotFound {
                        kind: DatasetKind::IsinCusip,
                        value: normalise_upper_ascii(cusip),
                    })
                }
            },
        )
    }

    /// Validate that a BIC is registered in the BIC↔LEI dataset.
    ///
    /// # Errors
    /// Returns [`ReferenceDataError`] if the dataset is unavailable or the BIC has no mapping.
    pub fn validate_bic(&self, bic: &str) -> Result<ValidationOutcome, ReferenceDataError> {
        Self::dataset_records_or_skip(&self.bic_lei)?.map_or_else(
            || Ok(ValidationOutcome::Skipped),
            |records| {
                if records.lei_by_bic(bic).is_some() {
                    Ok(ValidationOutcome::Enforced)
                } else {
                    Err(ReferenceDataError::NotFound {
                        kind: DatasetKind::BicLei,
                        value: normalise_upper_ascii(bic),
                    })
                }
            },
        )
    }

    /// Validate that a LEI is present in the BIC↔LEI dataset.
    ///
    /// # Errors
    /// Returns [`ReferenceDataError`] if the dataset is unavailable or the LEI is unknown.
    pub fn validate_lei(&self, lei: &str) -> Result<ValidationOutcome, ReferenceDataError> {
        Self::dataset_records_or_skip(&self.bic_lei)?.map_or_else(
            || Ok(ValidationOutcome::Skipped),
            |records| {
                if records.bics_by_lei(lei).is_some() {
                    Ok(ValidationOutcome::Enforced)
                } else {
                    Err(ReferenceDataError::NotFound {
                        kind: DatasetKind::BicLei,
                        value: normalise_upper_ascii(lei),
                    })
                }
            },
        )
    }

    /// Validate that a MIC exists and is active in the MIC directory.
    ///
    /// # Errors
    /// Returns [`ReferenceDataError`] if the dataset is unavailable or the MIC is unknown.
    pub fn validate_mic(&self, mic: &str) -> Result<ValidationOutcome, ReferenceDataError> {
        Self::dataset_records_or_skip(&self.mic_directory)?.map_or_else(
            || Ok(ValidationOutcome::Skipped),
            |records| {
                records.by_mic(mic).map_or_else(
                    || {
                        Err(ReferenceDataError::NotFound {
                            kind: DatasetKind::MicDirectory,
                            value: normalise_upper_ascii(mic),
                        })
                    },
                    |record| {
                        if mic_is_active(record.status.as_deref()) {
                            Ok(ValidationOutcome::Enforced)
                        } else {
                            Err(ReferenceDataError::MicInactive {
                                mic: normalise_upper_ascii(mic),
                                status: record.status.clone(),
                            })
                        }
                    },
                )
            },
        )
    }

    /// Lookup an instrument record by ISIN when the dataset is loaded.
    ///
    /// # Errors
    /// Returns [`ReferenceDataError`] if the dataset failed to load.
    pub fn instrument_by_isin(
        &self,
        isin: &str,
    ) -> Result<Option<&InstrumentRecord>, ReferenceDataError> {
        Self::dataset_records_or_skip(&self.isin_cusip)?
            .map_or_else(|| Ok(None), |records| Ok(records.by_isin(isin)))
    }

    /// Lookup an instrument record by CUSIP when the dataset is loaded.
    ///
    /// # Errors
    /// Returns [`ReferenceDataError`] if the dataset failed to load.
    pub fn instrument_by_cusip(
        &self,
        cusip: &str,
    ) -> Result<Option<&InstrumentRecord>, ReferenceDataError> {
        Self::dataset_records_or_skip(&self.isin_cusip)?
            .map_or_else(|| Ok(None), |records| Ok(records.by_cusip(cusip)))
    }

    /// Lookup MIC record if the directory is loaded.
    ///
    /// # Errors
    /// Returns [`ReferenceDataError`] if the dataset failed to load.
    pub fn mic_record(&self, mic: &str) -> Result<Option<&MicRecord>, ReferenceDataError> {
        Self::dataset_records_or_skip(&self.mic_directory)?
            .map_or_else(|| Ok(None), |records| Ok(records.by_mic(mic)))
    }
}

/// Crosswalk entry describing an instrument record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentRecord {
    /// ISIN identifier (key).
    pub isin: String,
    /// Optional CUSIP identifier.
    pub cusip: Option<String>,
    /// Optional asset definition identifier associated with the instrument.
    pub asset_definition_id: Option<String>,
    /// Optional asset identifier associated with the instrument.
    pub asset_id: Option<String>,
}

/// ISIN ↔ CUSIP crosswalk lookup structure.
#[derive(Debug, Clone, Default)]
pub struct InstrumentCrosswalk {
    by_isin: BTreeMap<String, InstrumentRecord>,
    by_cusip: BTreeMap<String, String>,
}

impl InstrumentCrosswalk {
    fn insert(&mut self, record: InstrumentRecord) -> eyre::Result<()> {
        let isin_key = normalise_upper_ascii(&record.isin);
        if self.by_isin.contains_key(&isin_key) {
            eyre::bail!("duplicate ISIN entry encountered: {isin_key}");
        }
        if let Some(cusip) = record.cusip.as_ref() {
            let cusip_key = normalise_upper_ascii(cusip);
            if let Some(existing) = self.by_cusip.insert(cusip_key.clone(), isin_key.clone()) {
                eyre::bail!(
                    "CUSIP {cusip_key} mapped to multiple ISINs ({existing} vs {isin_key})"
                );
            }
        }
        self.by_isin.insert(isin_key, record);
        Ok(())
    }

    /// Number of instrument records ingested.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_isin.len()
    }

    /// Returns true when no instrument records are loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_isin.is_empty()
    }

    /// Lookup by ISIN.
    #[must_use]
    pub fn by_isin(&self, isin: &str) -> Option<&InstrumentRecord> {
        let key = normalise_upper_ascii(isin);
        self.by_isin.get(&key)
    }

    /// Lookup by CUSIP.
    #[must_use]
    pub fn by_cusip(&self, cusip: &str) -> Option<&InstrumentRecord> {
        let key = normalise_upper_ascii(cusip);
        let isin = self.by_cusip.get(&key)?;
        self.by_isin.get(isin)
    }
}

/// BIC ↔ LEI crosswalk lookup structure.
#[derive(Debug, Clone, Default)]
pub struct BicLeiCrosswalk {
    bic_to_lei: BTreeMap<String, String>,
    lei_to_bic: BTreeMap<String, Vec<String>>,
}

impl BicLeiCrosswalk {
    fn insert(&mut self, bic: &str, lei: &str) {
        let bic_key = normalise_upper_ascii(bic);
        let lei_key = normalise_upper_ascii(lei);
        self.bic_to_lei.insert(bic_key.clone(), lei_key.clone());
        self.lei_to_bic.entry(lei_key).or_default().push(bic_key);
    }

    /// Number of BIC ↔ LEI pairs loaded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bic_to_lei.len()
    }

    /// Returns true when the crosswalk contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bic_to_lei.is_empty()
    }

    /// Lookup LEI by BIC.
    #[must_use]
    pub fn lei_by_bic(&self, bic: &str) -> Option<&str> {
        let key = normalise_upper_ascii(bic);
        self.bic_to_lei.get(&key).map(String::as_str)
    }

    /// Lookup BICs registered under a given LEI.
    #[must_use]
    pub fn bics_by_lei(&self, lei: &str) -> Option<&[String]> {
        let key = normalise_upper_ascii(lei);
        self.lei_to_bic.get(&key).map(Vec::as_slice)
    }
}

/// MIC directory entry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MicRecord {
    /// MIC identifier.
    pub mic: String,
    /// Human-readable market name.
    pub market_name: Option<String>,
    /// Country of operation (ISO 3166 code).
    pub country: Option<String>,
    /// Registration or termination status.
    pub status: Option<String>,
}

/// MIC directory lookup.
#[derive(Debug, Clone, Default)]
pub struct MicDirectory {
    by_mic: BTreeMap<String, MicRecord>,
}

impl MicDirectory {
    fn insert(&mut self, record: MicRecord) -> eyre::Result<()> {
        let key = normalise_upper_ascii(&record.mic);
        if self.by_mic.contains_key(&key) {
            eyre::bail!("duplicate MIC entry encountered: {key}");
        }
        self.by_mic.insert(key, record);
        Ok(())
    }

    /// Number of MIC entries loaded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_mic.len()
    }

    /// Returns true when no MIC entries are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_mic.is_empty()
    }

    /// Lookup a MIC entry by identifier.
    #[must_use]
    pub fn by_mic(&self, mic: &str) -> Option<&MicRecord> {
        let key = normalise_upper_ascii(mic);
        self.by_mic.get(&key)
    }
}

fn load_isin_crosswalk(path: &Path) -> eyre::Result<(SnapshotMetadata, InstrumentCrosswalk)> {
    let root = read_json(path)?;
    let mut metadata = parse_metadata(DatasetKind::IsinCusip, &root)?;
    let entries = root
        .as_object()
        .and_then(|obj| obj.get("entries"))
        .and_then(Value::as_array)
        .ok_or_else(|| eyre::eyre!("isin_cusip snapshot missing `entries` array"))?;

    let mut crosswalk = InstrumentCrosswalk::default();
    for entry in entries {
        let obj = entry
            .as_object()
            .ok_or_else(|| eyre::eyre!("isin_cusip entry must be an object"))?;
        let isin = obj
            .get("isin")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre::eyre!("isin_cusip entry missing `isin`"))?
            .trim()
            .to_ascii_uppercase();
        if !iso20022::validate_identifier(IdentifierKind::Isin, &isin) {
            eyre::bail!("isin_cusip entry contains invalid ISIN `{isin}`");
        }
        let cusip = obj
            .get("cusip")
            .and_then(Value::as_str)
            .map(|s| s.trim().to_ascii_uppercase())
            .filter(|s| !s.is_empty());
        if let Some(ref cusip) = cusip
            && !iso20022::validate_identifier(IdentifierKind::Cusip, cusip)
        {
            eyre::bail!("isin_cusip entry contains invalid CUSIP `{cusip}`");
        }
        let asset_definition_id = obj
            .get("asset_definition_id")
            .and_then(Value::as_str)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let asset_id = obj
            .get("asset_id")
            .and_then(Value::as_str)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let record = InstrumentRecord {
            isin,
            cusip,
            asset_definition_id,
            asset_id,
        };
        crosswalk.insert(record)?;
    }

    metadata.record_count = crosswalk.len();
    Ok((metadata, crosswalk))
}

fn load_bic_lei_crosswalk(path: &Path) -> eyre::Result<(SnapshotMetadata, BicLeiCrosswalk)> {
    let root = read_json(path)?;
    let mut metadata = parse_metadata(DatasetKind::BicLei, &root)?;
    let entries = root
        .as_object()
        .and_then(|obj| obj.get("entries"))
        .and_then(Value::as_array)
        .ok_or_else(|| eyre::eyre!("bic_lei snapshot missing `entries` array"))?;

    let mut crosswalk = BicLeiCrosswalk::default();
    for entry in entries {
        let obj = entry
            .as_object()
            .ok_or_else(|| eyre::eyre!("bic_lei entry must be an object"))?;
        let bic = obj
            .get("bic")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre::eyre!("bic_lei entry missing `bic`"))?
            .trim()
            .to_ascii_uppercase();
        if !iso20022::validate_identifier(IdentifierKind::Bic, &bic) {
            eyre::bail!("bic_lei entry contains invalid BIC `{bic}`");
        }
        let lei = obj
            .get("lei")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre::eyre!("bic_lei entry missing `lei`"))?
            .trim()
            .to_ascii_uppercase();
        if !iso20022::validate_identifier(IdentifierKind::Lei, &lei) {
            eyre::bail!("bic_lei entry contains invalid LEI `{lei}`");
        }
        crosswalk.insert(&bic, &lei);
    }

    metadata.record_count = crosswalk.len();
    Ok((metadata, crosswalk))
}

fn load_mic_directory(path: &Path) -> eyre::Result<(SnapshotMetadata, MicDirectory)> {
    let root = read_json(path)?;
    let mut metadata = parse_metadata(DatasetKind::MicDirectory, &root)?;
    let entries = root
        .as_object()
        .and_then(|obj| obj.get("entries"))
        .and_then(Value::as_array)
        .ok_or_else(|| eyre::eyre!("mic_directory snapshot missing `entries` array"))?;

    let mut directory = MicDirectory::default();
    for entry in entries {
        let obj = entry
            .as_object()
            .ok_or_else(|| eyre::eyre!("mic_directory entry must be an object"))?;
        let mic = obj
            .get("mic")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre::eyre!("mic_directory entry missing `mic`"))?
            .trim()
            .to_ascii_uppercase();
        if !iso20022::validate_identifier(IdentifierKind::Mic, &mic) {
            eyre::bail!("mic_directory entry contains invalid MIC `{mic}`");
        }
        let market_name = obj
            .get("market_name")
            .and_then(Value::as_str)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let country = obj
            .get("country")
            .and_then(Value::as_str)
            .map(|s| s.trim().to_ascii_uppercase())
            .filter(|s| !s.is_empty());
        let status = obj
            .get("status")
            .and_then(Value::as_str)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let record = MicRecord {
            mic,
            market_name,
            country,
            status,
        };
        directory.insert(record)?;
    }

    metadata.record_count = directory.len();
    Ok((metadata, directory))
}

fn read_json(path: &Path) -> eyre::Result<Value> {
    let raw = fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read ISO reference dataset at {}", path.display()))?;
    norito::json::from_str(&raw).wrap_err_with(|| {
        format!(
            "failed to parse ISO reference dataset JSON at {}",
            path.display()
        )
    })
}

fn parse_metadata(kind: DatasetKind, root: &Value) -> eyre::Result<SnapshotMetadata> {
    let obj = root
        .as_object()
        .ok_or_else(|| eyre::eyre!("{} snapshot must be a JSON object", kind.label()))?;
    let version = obj
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(|| eyre::eyre!("{} snapshot missing `version`", kind.label()))?
        .trim()
        .to_string();
    let source = obj
        .get("source")
        .and_then(Value::as_str)
        .ok_or_else(|| eyre::eyre!("{} snapshot missing `source`", kind.label()))?
        .trim()
        .to_string();
    let fetched_at = obj
        .get("fetched_at")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            OffsetDateTime::parse(s, &Rfc3339)
                .wrap_err_with(|| format!("invalid RFC3339 timestamp `{s}`"))
        })
        .transpose()?;

    Ok(SnapshotMetadata {
        version,
        source,
        fetched_at,
        record_count: 0,
    })
}

fn mic_is_active(status: Option<&str>) -> bool {
    match status {
        None => true,
        Some(raw) => {
            let upper = raw.trim().to_ascii_uppercase();
            if upper.is_empty() {
                return true;
            }
            if upper.contains('!') {
                return false;
            }
            !upper.contains("DELETED")
                && !upper.contains("EXPIRED")
                && !upper.contains("INACTIVE")
                && !upper.contains("SUSPENDED")
        }
    }
}

fn normalise_upper_ascii(input: &str) -> String {
    input.trim().to_ascii_uppercase()
}

fn sanitize_path_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() { "_".to_owned() } else { out }
}

fn compute_sha256_hex(path: &Path) -> eyre::Result<String> {
    let mut file = File::open(path)
        .wrap_err_with(|| format!("failed to open dataset at {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::Write as _,
        sync::{Mutex, OnceLock},
    };

    use iroha_config::parameters::actual::IsoReferenceData;
    use iroha_telemetry::metrics;
    use tempfile::{NamedTempFile, TempDir};

    use super::*;

    fn iso_reference_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("iso reference test lock poisoned")
    }

    fn write_snapshot(contents: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("temp file");
        file.write_all(contents.as_bytes()).expect("write snapshot");
        file
    }

    #[test]
    fn missing_snapshots_default_to_missing_state() {
        let _guard = iso_reference_test_guard();
        let config = IsoReferenceData::default();
        let snapshots = ReferenceDataSnapshots::from_config(&config);
        assert_eq!(snapshots.isin_cusip().state(), SnapshotState::Missing);
        assert_eq!(snapshots.bic_lei().state(), SnapshotState::Missing);
        assert_eq!(snapshots.mic_directory().state(), SnapshotState::Missing);
    }

    #[test]
    fn loads_isin_crosswalk_snapshot() {
        let _guard = iso_reference_test_guard();
        let metrics_handle = metrics::global_or_default();

        let contents = r#"{
            "version":"2024-05-01",
            "source":"ANNA DSB test",
            "fetched_at":"2024-05-01T12:00:00Z",
            "entries":[
                {
                    "isin":"US0378331005",
                    "cusip":"037833100",
                    "asset_definition_id":"usd#test"
                }
            ]
        }"#;
        let file = write_snapshot(contents);
        let config = IsoReferenceData {
            isin_crosswalk_path: Some(file.path().to_path_buf()),
            ..IsoReferenceData::default()
        };

        let snapshots = ReferenceDataSnapshots::from_config(&config);
        let dataset = snapshots.isin_cusip();
        assert_eq!(dataset.state(), SnapshotState::Loaded);
        let metadata = dataset.metadata().expect("metadata present");
        assert_eq!(metadata.version, "2024-05-01");
        assert_eq!(metadata.source, "ANNA DSB test");
        assert_eq!(metadata.record_count, 1);
        let crosswalk = dataset.records().expect("crosswalk loaded");
        let record = crosswalk.by_isin("US0378331005").expect("isin present");
        assert_eq!(record.cusip.as_deref(), Some("037833100"));
        assert_eq!(record.asset_definition_id.as_deref(), Some("usd#test"));

        assert_eq!(
            metrics_handle
                .iso_reference_status
                .with_label_values(&["isin_cusip"])
                .get(),
            1
        );
        assert_eq!(
            metrics_handle
                .iso_reference_records
                .with_label_values(&["isin_cusip"])
                .get(),
            1
        );
        let age = metrics_handle
            .iso_reference_age_seconds
            .with_label_values(&["isin_cusip"])
            .get();
        assert!(age >= 0);
    }

    #[test]
    fn validate_bic_enforces_registered_entries() {
        let _guard = iso_reference_test_guard();
        let bic_snapshot = r#"{
            "version":"2024-05-01",
            "source":"GLEIF sample",
            "entries":[
                {"bic":"DEUTDEFF","lei":"5493001KJTIIGC8Y1R12"}
            ]
        }"#;
        let file = write_snapshot(bic_snapshot);
        let config = IsoReferenceData {
            bic_lei_path: Some(file.path().to_path_buf()),
            ..IsoReferenceData::default()
        };

        let snapshots = ReferenceDataSnapshots::from_config(&config);
        assert_eq!(
            snapshots.validate_bic("DEUTDEFF").expect("validation"),
            ValidationOutcome::Enforced
        );
        let err = snapshots.validate_bic("FOOBARXX").unwrap_err();
        assert!(matches!(err, ReferenceDataError::NotFound { .. }));
    }

    #[test]
    fn validate_bic_skips_when_dataset_missing() {
        let _guard = iso_reference_test_guard();
        let config = IsoReferenceData::default();
        let snapshots = ReferenceDataSnapshots::from_config(&config);
        assert_eq!(
            snapshots.validate_bic("DEUTDEFF").expect("validation"),
            ValidationOutcome::Skipped
        );
    }

    #[test]
    fn persists_loaded_snapshots_to_cache() {
        let _guard = iso_reference_test_guard();
        let contents = r#"{
            "version":"2024-05-01",
            "source":"ANNA test",
            "entries":[{"isin":"US0378331005","cusip":"037833100"}]
        }"#;
        let file = write_snapshot(contents);
        let cache_dir = TempDir::new().expect("cache dir");

        let config = IsoReferenceData {
            isin_crosswalk_path: Some(file.path().to_path_buf()),
            cache_dir: Some(cache_dir.path().to_path_buf()),
            ..IsoReferenceData::default()
        };

        let snapshots = ReferenceDataSnapshots::from_config(&config);
        assert_eq!(snapshots.isin_cusip().state(), SnapshotState::Loaded);

        let dataset_dir = cache_dir.path().join("isin_cusip");
        let status_path = dataset_dir.join("status.json");
        assert!(status_path.exists(), "status file missing");

        let status_value: Value =
            norito::json::from_str(&fs::read_to_string(&status_path).unwrap())
                .expect("status json");
        assert_eq!(
            status_value.get("status").and_then(Value::as_str),
            Some("loaded")
        );
        let cached_file = status_value
            .get("cached_file")
            .and_then(Value::as_str)
            .expect("cached file entry");

        let metadata_path = dataset_dir.join("2024-05-01.metadata.json");
        assert!(metadata_path.exists(), "metadata file missing");
        let metadata_value: Value =
            norito::json::from_str(&fs::read_to_string(&metadata_path).unwrap())
                .expect("metadata json");
        assert_eq!(
            metadata_value.get("version").and_then(Value::as_str),
            Some("2024-05-01")
        );
        let cached_data_path = dataset_dir.join(cached_file);
        assert!(cached_data_path.exists(), "cached dataset missing");

        let expected_sha = compute_sha256_hex(&cached_data_path).expect("sha256");
        assert_eq!(
            metadata_value.get("cached_sha256").and_then(Value::as_str),
            Some(expected_sha.as_str())
        );

        // Metrics are published to a global registry that other tests mutate concurrently,
        // so this test only verifies the cached artifacts.
    }

    #[test]
    fn validate_mic_flags_inactive_entries() {
        let _guard = iso_reference_test_guard();
        let mic_snapshot = r#"{
            "version":"2024-05-01",
            "source":"MIC sample",
            "entries":[
                {"mic":"XNAS","status":"ACTIVE"},
                {"mic":"XTBD","status":"DELETED"}
            ]
        }"#;
        let file = write_snapshot(mic_snapshot);
        let config = IsoReferenceData {
            mic_directory_path: Some(file.path().to_path_buf()),
            ..IsoReferenceData::default()
        };

        let snapshots = ReferenceDataSnapshots::from_config(&config);
        assert_eq!(
            snapshots.validate_mic("XNAS").expect("validation"),
            ValidationOutcome::Enforced
        );
        let err = snapshots.validate_mic("XTBD").unwrap_err();
        assert!(matches!(err, ReferenceDataError::MicInactive { .. }));
    }
}
