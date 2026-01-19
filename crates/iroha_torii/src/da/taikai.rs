//! Taikai ingest helpers and anchor integration for DA.

#![allow(clippy::redundant_pub_crate)]

use std::{
    borrow::Cow,
    fs::{self, OpenOptions},
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use axum::http::StatusCode;
use blake3::{Hasher as Blake3Hasher, hash as blake3_hash};
use iroha_config::parameters::actual::DaTaikaiAnchor;
use iroha_core::da::ReplayFingerprint;
use iroha_data_model::{
    da::prelude::*,
    name::Name,
    nexus::LaneId,
    sorafs::pin_registry::StorageClass,
    taikai::{
        GuardDirectoryId, SegmentDuration, SegmentTimestamp, TaikaiAliasBinding, TaikaiAudioLayout,
        TaikaiAvailabilityClass, TaikaiCarPointer, TaikaiCodec, TaikaiEnvelopeIndexes,
        TaikaiEventId, TaikaiGuardPolicy, TaikaiIngestPointer, TaikaiParseError, TaikaiRenditionId,
        TaikaiRenditionRouteV1, TaikaiResolution, TaikaiRoutingManifestV1, TaikaiSegmentEnvelopeV1,
        TaikaiSegmentSigningManifestV1, TaikaiSegmentWindow, TaikaiStreamId, TaikaiTrackKind,
        TaikaiTrackMetadata,
    },
};
use iroha_futures::supervisor::ShutdownSignal;
use iroha_logger::debug;
use iroha_torii_shared::da::sampling::compute_sample_window;
use norito::{
    decode_from_bytes,
    json::{self, Map, Value},
};
use reqwest::Client;
use sorafs_car::ChunkStore;

use super::{ingest::ManifestArtifacts, storage_class_label};
use crate::{
    routing::MaybeTelemetry,
    sorafs::{AliasCachePolicy, AliasProofEvaluation},
};

pub(crate) const TAIKAI_SPOOL_SUBDIR: &str = "taikai";
pub(crate) const META_TAIKAI_EVENT_ID: &str = "taikai.event_id";
pub(crate) const META_TAIKAI_STREAM_ID: &str = "taikai.stream_id";
pub(crate) const META_TAIKAI_RENDITION_ID: &str = "taikai.rendition_id";
pub(crate) const META_TAIKAI_TRACK_KIND: &str = "taikai.track.kind";
pub(crate) const META_TAIKAI_TRACK_CODEC: &str = "taikai.track.codec";
pub(crate) const META_TAIKAI_TRACK_BITRATE: &str = "taikai.track.bitrate_kbps";
pub(crate) const META_TAIKAI_TRACK_RESOLUTION: &str = "taikai.track.resolution";
pub(crate) const META_TAIKAI_TRACK_AUDIO_LAYOUT: &str = "taikai.track.audio_layout";
pub(crate) const META_TAIKAI_SEGMENT_SEQUENCE: &str = "taikai.segment.sequence";
pub(crate) const META_TAIKAI_SEGMENT_START: &str = "taikai.segment.start_pts";
pub(crate) const META_TAIKAI_SEGMENT_DURATION: &str = "taikai.segment.duration";
pub(crate) const META_TAIKAI_WALLCLOCK_MS: &str = "taikai.wallclock_unix_ms";
pub(crate) const META_TAIKAI_INGEST_LATENCY_MS: &str = "taikai.instrumentation.ingest_latency_ms";
pub(crate) const META_TAIKAI_LIVE_EDGE_DRIFT_MS: &str = "taikai.instrumentation.live_edge_drift_ms";
pub(crate) const META_TAIKAI_INGEST_NODE_ID: &str = "taikai.instrumentation.ingest_node_id";
pub(crate) const META_TAIKAI_SSM: &str = "taikai.ssm";
pub(crate) const META_TAIKAI_TRM: &str = "taikai.trm";
pub(crate) const META_TAIKAI_AVAILABILITY_CLASS: &str = "taikai.availability_class";
pub(crate) const META_TAIKAI_REPLICATION_REPLICAS: &str = "taikai.replication.replicas";
pub(crate) const META_TAIKAI_REPLICATION_STORAGE: &str = "taikai.replication.storage_class";
pub(crate) const META_TAIKAI_REPLICATION_HOT_SECS: &str = "taikai.replication.hot_retention_secs";
pub(crate) const META_TAIKAI_REPLICATION_COLD_SECS: &str = "taikai.replication.cold_retention_secs";
pub(crate) const META_TAIKAI_CACHE_HINT: &str = "taikai.cache_hint";
pub(crate) const META_DA_PROOF_TIER: &str = "da.proof.tier";
pub(crate) const META_DA_PDP_SAMPLE_WINDOW: &str = "da.proof.pdp.sample_window";
pub(crate) const META_DA_POTR_SAMPLE_WINDOW: &str = "da.proof.potr.sample_window";
pub(crate) const TAIKAI_ANCHOR_SENTINEL_PREFIX: &str = "taikai-anchor-";
pub(crate) const TAIKAI_ANCHOR_SENTINEL_SUFFIX: &str = ".ok";
pub(crate) const TAIKAI_ANCHOR_REQUEST_PREFIX: &str = "taikai-anchor-request-";
pub(crate) const TAIKAI_ANCHOR_REQUEST_SUFFIX: &str = ".json";
pub(crate) const TAIKAI_TRM_LINEAGE_PREFIX: &str = "taikai-trm-state-";
pub(crate) const TAIKAI_TRM_LINEAGE_SUFFIX: &str = ".json";
pub(crate) const TAIKAI_TRM_LOCK_PREFIX: &str = "taikai-trm-lock-";
pub(crate) const TAIKAI_TRM_LOCK_SUFFIX: &str = ".lock";
pub(crate) const TAIKAI_TRM_LOCK_STALE_SECS: u64 = 300;
pub(crate) const TAIKAI_LINEAGE_HINT_PREFIX: &str = "taikai-lineage";

#[allow(clippy::redundant_pub_crate)]
pub(crate) mod taikai_ingest {
    use std::{
        io::{self, Write},
        str::FromStr,
    };

    use sorafs_car::{CarBuildPlan, CarWriter};

    use super::*;

    pub(crate) const STREAM_LABEL_FALLBACK: &str = "<unknown>";

    pub(crate) struct EnvelopeArtifacts {
        pub envelope_bytes: Vec<u8>,
        pub indexes_json: Vec<u8>,
        pub telemetry: TaikaiTelemetrySample,
        pub car_digest: BlobDigest,
    }

    #[derive(Clone)]
    pub(crate) struct TaikaiTelemetrySample {
        pub event_id: String,
        pub stream_id: String,
        pub rendition_id: String,
        pub segment_sequence: u64,
        pub wallclock_unix_ms: u64,
        pub ingest_latency_ms: Option<u32>,
        pub live_edge_drift_ms: Option<i32>,
    }

    #[derive(Clone, Debug)]
    struct TrmLineageRecord {
        alias_namespace: String,
        alias_name: String,
        manifest_digest_hex: String,
        window_start_sequence: u64,
        window_end_sequence: u64,
        updated_unix: u64,
    }

    pub(crate) struct TrmLineageGuard {
        manifest_store_dir: PathBuf,
        base_dir: PathBuf,
        alias_namespace: String,
        alias_name: String,
        alias_slug: String,
        lock: TrmAliasLock,
        record_path: PathBuf,
        previous: Option<TrmLineageRecord>,
    }

    impl TrmLineageGuard {
        pub fn new(
            spool_dir: &Path,
            alias: &TaikaiAliasBinding,
        ) -> Result<Option<Self>, (StatusCode, String)> {
            if spool_dir.as_os_str().is_empty() {
                return Ok(None);
            }
            let base_dir = spool_dir.join(TAIKAI_SPOOL_SUBDIR);
            fs::create_dir_all(&base_dir).map_err(|err| {
                internal_error(format!(
                    "failed to prepare Taikai spool directory `{}`: {err}",
                    base_dir.display()
                ))
            })?;
            let alias_slug = alias_slug(&alias.namespace, &alias.name);
            let lock = TrmAliasLock::acquire(&base_dir, &alias_slug)?;
            let record_path = lineage_record_path(&base_dir, &alias_slug);
            let previous = read_lineage_record(&record_path).map_err(|err| {
                internal_error(format!(
                    "failed to read Taikai routing manifest lineage `{}`: {err}",
                    record_path.display()
                ))
            })?;
            Ok(Some(Self {
                manifest_store_dir: spool_dir.to_path_buf(),
                base_dir,
                alias_namespace: alias.namespace.clone(),
                alias_name: alias.name.clone(),
                alias_slug,
                lock,
                record_path,
                previous,
            }))
        }

        pub fn validate(
            &self,
            manifest: &TaikaiRoutingManifestV1,
            manifest_digest_hex: &str,
        ) -> Result<(), (StatusCode, String)> {
            if let Some(previous) = &self.previous {
                if previous.manifest_digest_hex == manifest_digest_hex {
                    return Err(bad_request(
                        META_TAIKAI_TRM,
                        format!(
                            "routing manifest digest `{manifest_digest_hex}` already accepted for alias {}.{}",
                            self.alias_name, self.alias_namespace
                        ),
                    ));
                }
                if manifest.segment_window.start_sequence <= previous.window_end_sequence {
                    return Err(bad_request(
                        META_TAIKAI_TRM,
                        format!(
                            "routing manifest window {}–{} overlaps previously accepted window {}–{} for alias {}.{}",
                            manifest.segment_window.start_sequence,
                            manifest.segment_window.end_sequence,
                            previous.window_start_sequence,
                            previous.window_end_sequence,
                            self.alias_name,
                            self.alias_namespace
                        ),
                    ));
                }
            }
            Ok(())
        }

        pub fn persist_lineage_hint(
            &self,
            lane_id: LaneId,
            epoch: u64,
            sequence: u64,
            storage_ticket: &StorageTicketId,
            fingerprint: &ReplayFingerprint,
        ) -> Result<(), (StatusCode, String)> {
            let bytes = build_lineage_hint_bytes(
                &self.alias_namespace,
                &self.alias_name,
                self.previous.as_ref(),
            )
            .map_err(|err| {
                internal_error(format!(
                    "failed to build Taikai routing manifest lineage hint: {err}"
                ))
            })?;
            persist_artifact(
                &self.manifest_store_dir,
                lane_id,
                epoch,
                sequence,
                storage_ticket,
                fingerprint,
                TAIKAI_LINEAGE_HINT_PREFIX,
                "json",
                &bytes,
            )
            .map_err(|err| {
                internal_error(format!(
                    "failed to persist Taikai routing manifest lineage hint in `{}`: {err}",
                    self.manifest_store_dir.display()
                ))
            })?;
            Ok(())
        }

        pub fn commit(
            &mut self,
            window: TaikaiSegmentWindow,
            manifest_digest_hex: &str,
        ) -> Result<(), (StatusCode, String)> {
            let record = TrmLineageRecord {
                alias_namespace: self.alias_namespace.clone(),
                alias_name: self.alias_name.clone(),
                manifest_digest_hex: manifest_digest_hex.to_owned(),
                window_start_sequence: window.start_sequence,
                window_end_sequence: window.end_sequence,
                updated_unix: current_unix_seconds(),
            };
            write_lineage_record(&self.record_path, &record).map_err(|err| {
                internal_error(format!(
                    "failed to persist Taikai routing manifest lineage `{}`: {err}",
                    self.record_path.display()
                ))
            })?;
            self.previous = Some(record);
            Ok(())
        }
    }

    struct TrmAliasLock {
        path: PathBuf,
    }

    impl TrmAliasLock {
        fn acquire(base_dir: &Path, slug: &str) -> Result<Self, (StatusCode, String)> {
            let path = base_dir.join(format!(
                "{TAIKAI_TRM_LOCK_PREFIX}{slug}{TAIKAI_TRM_LOCK_SUFFIX}"
            ));
            for attempt in 0..=1 {
                match OpenOptions::new().write(true).create_new(true).open(&path) {
                    Ok(mut file) => {
                        let now = current_unix_seconds();
                        let _ = writeln!(file, "{now}");
                        return Ok(Self { path });
                    }
                    Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                        if attempt == 0 && lock_is_stale(&path).unwrap_or(false) {
                            let _ = fs::remove_file(&path);
                            continue;
                        }
                        return Err((
                            StatusCode::SERVICE_UNAVAILABLE,
                            format!(
                                "routing manifest lock busy for alias slug `{slug}`; retry later"
                            ),
                        ));
                    }
                    Err(err) => {
                        return Err(internal_error(format!(
                            "failed to create Taikai routing manifest lock `{}`: {err}",
                            path.display()
                        )));
                    }
                }
            }
            unreachable!("lock acquisition attempts exhausted without returning");
        }
    }

    impl Drop for TrmAliasLock {
        fn drop(&mut self) {
            if let Err(err) = fs::remove_file(&self.path) {
                if err.kind() != ErrorKind::NotFound {
                    iroha_logger::warn!(
                        ?err,
                        path = %self.path.display(),
                        "failed to remove Taikai routing manifest lock"
                    );
                }
            }
        }
    }

    fn alias_slug(namespace: &str, name: &str) -> String {
        let mut hasher = Blake3Hasher::new();
        hasher.update(namespace.as_bytes());
        hasher.update(&[0xFF]);
        hasher.update(name.as_bytes());
        let digest = hasher.finalize();
        let digest_hex = hex::encode(&digest.as_bytes()[..6]);
        format!(
            "{}-{}-{digest_hex}",
            sanitize_alias_component(namespace),
            sanitize_alias_component(name)
        )
    }

    fn sanitize_alias_component(component: &str) -> String {
        component
            .chars()
            .map(|ch| match ch {
                'a'..='z' => ch,
                'A'..='Z' => ch.to_ascii_lowercase(),
                '0'..='9' => ch,
                _ => '-',
            })
            .collect()
    }

    fn lineage_record_path(base_dir: &Path, slug: &str) -> PathBuf {
        base_dir.join(format!(
            "{TAIKAI_TRM_LINEAGE_PREFIX}{slug}{TAIKAI_TRM_LINEAGE_SUFFIX}"
        ))
    }

    fn lock_is_stale(path: &Path) -> io::Result<bool> {
        match fs::metadata(path) {
            Ok(metadata) => {
                let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
                let elapsed = SystemTime::now()
                    .duration_since(modified)
                    .unwrap_or_default();
                Ok(elapsed.as_secs() >= TAIKAI_TRM_LOCK_STALE_SECS)
            }
            Err(err) => {
                if err.kind() == ErrorKind::NotFound {
                    Ok(false)
                } else {
                    Err(err)
                }
            }
        }
    }

    fn current_unix_seconds() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn read_lineage_record(path: &Path) -> io::Result<Option<TrmLineageRecord>> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        let value: Value = json::from_slice(&bytes)
            .map_err(|err| io::Error::new(ErrorKind::Other, err.to_string()))?;
        let map = value.as_object().ok_or_else(|| {
            io::Error::new(
                ErrorKind::Other,
                "Taikai routing manifest lineage record must be a JSON object",
            )
        })?;
        let alias_namespace = map
            .get("alias_namespace")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                io::Error::new(
                    ErrorKind::Other,
                    "Taikai routing manifest lineage record missing alias_namespace",
                )
            })?
            .to_owned();
        let alias_name = map
            .get("alias_name")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                io::Error::new(
                    ErrorKind::Other,
                    "Taikai routing manifest lineage record missing alias_name",
                )
            })?
            .to_owned();
        let manifest_digest_hex = map
            .get("manifest_digest_hex")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                io::Error::new(
                    ErrorKind::Other,
                    "Taikai routing manifest lineage record missing manifest_digest_hex",
                )
            })?
            .to_owned();
        let window_start_sequence = map
            .get("window_start_sequence")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                io::Error::new(
                    ErrorKind::Other,
                    "Taikai routing manifest lineage record missing window_start_sequence",
                )
            })?;
        let window_end_sequence = map
            .get("window_end_sequence")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                io::Error::new(
                    ErrorKind::Other,
                    "Taikai routing manifest lineage record missing window_end_sequence",
                )
            })?;
        let updated_unix = map
            .get("updated_unix")
            .and_then(Value::as_u64)
            .unwrap_or_else(current_unix_seconds);
        Ok(Some(TrmLineageRecord {
            alias_namespace,
            alias_name,
            manifest_digest_hex,
            window_start_sequence,
            window_end_sequence,
            updated_unix,
        }))
    }

    fn write_lineage_record(path: &Path, record: &TrmLineageRecord) -> io::Result<()> {
        let mut map = Map::new();
        map.insert("version".into(), Value::from(1));
        map.insert(
            "alias_namespace".into(),
            Value::from(record.alias_namespace.clone()),
        );
        map.insert("alias_name".into(), Value::from(record.alias_name.clone()));
        map.insert(
            "manifest_digest_hex".into(),
            Value::from(record.manifest_digest_hex.clone()),
        );
        map.insert(
            "window_start_sequence".into(),
            Value::from(record.window_start_sequence),
        );
        map.insert(
            "window_end_sequence".into(),
            Value::from(record.window_end_sequence),
        );
        map.insert("updated_unix".into(), Value::from(record.updated_unix));
        let rendered = json::to_json_pretty(&Value::Object(map))
            .map_err(|err| io::Error::new(ErrorKind::Other, err.to_string()))?;
        let tmp_path = path.with_extension(format!("tmp-{}", std::process::id()));
        match fs::write(&tmp_path, rendered.as_bytes()) {
            Ok(()) => {}
            Err(err) => {
                let _ = fs::remove_file(&tmp_path);
                return Err(err);
            }
        }
        if let Err(err) = fs::rename(&tmp_path, path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(err);
        }
        Ok(())
    }

    fn build_lineage_hint_bytes(
        alias_namespace: &str,
        alias_name: &str,
        previous: Option<&TrmLineageRecord>,
    ) -> Result<Vec<u8>, io::Error> {
        let mut map = Map::new();
        map.insert("version".into(), Value::from(1));
        map.insert(
            "alias_namespace".into(),
            Value::from(alias_namespace.to_owned()),
        );
        map.insert("alias_name".into(), Value::from(alias_name.to_owned()));
        if let Some(previous) = previous {
            map.insert(
                "previous_manifest_digest_hex".into(),
                Value::from(previous.manifest_digest_hex.clone()),
            );
            map.insert(
                "previous_window_start_sequence".into(),
                Value::from(previous.window_start_sequence),
            );
            map.insert(
                "previous_window_end_sequence".into(),
                Value::from(previous.window_end_sequence),
            );
            map.insert(
                "previous_updated_unix".into(),
                Value::from(previous.updated_unix),
            );
        } else {
            map.insert("previous_manifest_digest_hex".into(), Value::Null);
            map.insert("previous_window_start_sequence".into(), Value::Null);
            map.insert("previous_window_end_sequence".into(), Value::Null);
            map.insert("previous_updated_unix".into(), Value::Null);
        }
        let rendered = json::to_json_pretty(&Value::Object(map))
            .map_err(|err| io::Error::new(ErrorKind::Other, err.to_string()))?;
        Ok(rendered.into_bytes())
    }

    pub(crate) fn build_envelope(
        _request: &DaIngestRequest,
        manifest: &ManifestArtifacts,
        chunk_store: &ChunkStore,
        canonical_payload: &[u8],
        chunking_observer: Option<&dyn Fn(Duration)>,
    ) -> Result<EnvelopeArtifacts, (StatusCode, String)> {
        let metadata = &manifest.manifest.metadata;

        let event_id = TaikaiEventId::new(parse_name(metadata, META_TAIKAI_EVENT_ID)?);
        let stream_id = TaikaiStreamId::new(parse_name(metadata, META_TAIKAI_STREAM_ID)?);
        let rendition_id = TaikaiRenditionId::new(parse_name(metadata, META_TAIKAI_RENDITION_ID)?);

        let track_kind = TaikaiTrackKind::from_str(require_utf8(metadata, META_TAIKAI_TRACK_KIND)?)
            .map_err(|err| parse_error(META_TAIKAI_TRACK_KIND, err))?;
        let codec = TaikaiCodec::from_str(require_utf8(metadata, META_TAIKAI_TRACK_CODEC)?)
            .map_err(|err| parse_error(META_TAIKAI_TRACK_CODEC, err))?;
        let bitrate = parse_u32(
            require_utf8(metadata, META_TAIKAI_TRACK_BITRATE)?,
            META_TAIKAI_TRACK_BITRATE,
        )?;

        let track = match track_kind {
            TaikaiTrackKind::Video => {
                let resolution_str = require_utf8(metadata, META_TAIKAI_TRACK_RESOLUTION)?;
                let resolution = TaikaiResolution::from_str(resolution_str)
                    .map_err(|err| parse_error(META_TAIKAI_TRACK_RESOLUTION, err))?;
                if !matches!(
                    codec,
                    TaikaiCodec::AvcHigh
                        | TaikaiCodec::HevcMain10
                        | TaikaiCodec::Av1Main
                        | TaikaiCodec::Custom(_)
                ) {
                    return Err(bad_request(
                        META_TAIKAI_TRACK_CODEC,
                        "codec is not valid for a video track; expected AV1/AVC/HEVC or custom",
                    ));
                }
                TaikaiTrackMetadata::video(codec, bitrate, resolution)
            }
            TaikaiTrackKind::Audio => {
                let layout_str = require_utf8(metadata, META_TAIKAI_TRACK_AUDIO_LAYOUT)?;
                let layout = TaikaiAudioLayout::from_str(layout_str)
                    .map_err(|err| parse_error(META_TAIKAI_TRACK_AUDIO_LAYOUT, err))?;
                if !matches!(
                    codec,
                    TaikaiCodec::AacLc | TaikaiCodec::Opus | TaikaiCodec::Custom(_)
                ) {
                    return Err(bad_request(
                        META_TAIKAI_TRACK_CODEC,
                        "codec is not valid for an audio track; expected AAC/Opus or custom",
                    ));
                }
                TaikaiTrackMetadata::audio(codec, bitrate, layout)
            }
            TaikaiTrackKind::Data => TaikaiTrackMetadata::data(codec, bitrate),
        };

        if matches!(track.kind, TaikaiTrackKind::Video) && track.resolution.is_none() {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "metadata entry `{META_TAIKAI_TRACK_RESOLUTION}` is required for video tracks"
                ),
            ));
        }

        if matches!(track.kind, TaikaiTrackKind::Audio) && track.audio_layout.is_none() {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "metadata entry `{META_TAIKAI_TRACK_AUDIO_LAYOUT}` is required for audio tracks"
                ),
            ));
        }

        let segment_sequence = parse_u64(
            require_utf8(metadata, META_TAIKAI_SEGMENT_SEQUENCE)?,
            META_TAIKAI_SEGMENT_SEQUENCE,
        )?;
        let segment_start_pts = parse_u64(
            require_utf8(metadata, META_TAIKAI_SEGMENT_START)?,
            META_TAIKAI_SEGMENT_START,
        )?;
        let segment_duration = parse_u32(
            require_utf8(metadata, META_TAIKAI_SEGMENT_DURATION)?,
            META_TAIKAI_SEGMENT_DURATION,
        )?;
        let wallclock_unix_ms = parse_u64(
            require_utf8(metadata, META_TAIKAI_WALLCLOCK_MS)?,
            META_TAIKAI_WALLCLOCK_MS,
        )?;

        let chunk_count: u32 = chunk_store
            .chunks()
            .len()
            .try_into()
            .map_err(|_| internal_error("chunk count exceeds supported range".into()))?;

        let chunking_started = Instant::now();
        let plan = CarBuildPlan::single_file(canonical_payload)
            .map_err(|err| internal_error(format!("failed to derive CAR plan: {err}")))?;
        let mut sink = io::sink();
        let stats = CarWriter::new(&plan, canonical_payload)
            .map_err(|err| internal_error(format!("failed to initialise CAR writer: {err}")))?
            .write_to(&mut sink)
            .map_err(|err| internal_error(format!("failed to compute CAR digests: {err}")))?;
        if let Some(observer) = chunking_observer {
            observer(chunking_started.elapsed());
        }

        let car_digest = BlobDigest::from_hash(stats.car_archive_digest);
        let car_pointer = TaikaiCarPointer::new(
            format!("b{}", encode_base32_lower(&stats.car_cid)),
            car_digest,
            stats.car_size,
        );

        let ingest_pointer = TaikaiIngestPointer::new(
            manifest.manifest_hash,
            manifest.storage_ticket,
            manifest.chunk_root,
            chunk_count,
            car_pointer,
        );

        let event_label = event_id.as_name().as_ref().to_owned();
        let stream_label = stream_id.as_name().as_ref().to_owned();
        let rendition_label = rendition_id.as_name().as_ref().to_owned();

        let mut envelope = TaikaiSegmentEnvelopeV1::new(
            event_id,
            stream_id,
            rendition_id,
            track,
            segment_sequence,
            SegmentTimestamp::new(segment_start_pts),
            SegmentDuration::new(segment_duration),
            wallclock_unix_ms,
            ingest_pointer,
        );

        if let Some(latency_str) = optional_utf8(metadata, META_TAIKAI_INGEST_LATENCY_MS)? {
            let latency = parse_u32(latency_str, META_TAIKAI_INGEST_LATENCY_MS)?;
            envelope.instrumentation.encoder_to_ingest_latency_ms = Some(latency);
        }
        let live_edge_drift_ms =
            if let Some(drift_str) = optional_utf8(metadata, META_TAIKAI_LIVE_EDGE_DRIFT_MS)? {
                Some(parse_i32(drift_str, META_TAIKAI_LIVE_EDGE_DRIFT_MS)?)
            } else {
                None
            };
        if let Some(drift) = live_edge_drift_ms {
            envelope.instrumentation.live_edge_drift_ms = Some(drift);
        }
        if let Some(node_id) = optional_utf8(metadata, META_TAIKAI_INGEST_NODE_ID)? {
            if !node_id.trim().is_empty() {
                envelope.instrumentation.ingest_node_id = Some(node_id.trim().to_owned());
            }
        }

        let indexes = envelope.indexes();
        let envelope_bytes = envelope.encode();
        let indexes_json = norito::json::to_json_pretty(&indexes)
            .map_err(|err| internal_error(format!("failed to render Taikai indexes: {err}")))?
            .into_bytes();

        let telemetry = TaikaiTelemetrySample {
            event_id: event_label,
            stream_id: stream_label,
            rendition_id: rendition_label,
            segment_sequence,
            wallclock_unix_ms,
            ingest_latency_ms: envelope.instrumentation.encoder_to_ingest_latency_ms,
            live_edge_drift_ms,
        };

        Ok(EnvelopeArtifacts {
            envelope_bytes,
            indexes_json,
            telemetry,
            car_digest,
        })
    }

    pub(crate) fn persist_envelope(
        spool_dir: &Path,
        lane_id: LaneId,
        epoch: u64,
        sequence: u64,
        storage_ticket: &StorageTicketId,
        fingerprint: &ReplayFingerprint,
        bytes: &[u8],
    ) -> io::Result<Option<PathBuf>> {
        persist_artifact(
            spool_dir,
            lane_id,
            epoch,
            sequence,
            storage_ticket,
            fingerprint,
            "taikai-envelope",
            "norito",
            bytes,
        )
    }

    pub(crate) fn persist_indexes(
        spool_dir: &Path,
        lane_id: LaneId,
        epoch: u64,
        sequence: u64,
        storage_ticket: &StorageTicketId,
        fingerprint: &ReplayFingerprint,
        bytes: &[u8],
    ) -> io::Result<Option<PathBuf>> {
        persist_artifact(
            spool_dir,
            lane_id,
            epoch,
            sequence,
            storage_ticket,
            fingerprint,
            "taikai-indexes",
            "json",
            bytes,
        )
    }

    pub(crate) fn persist_ssm(
        spool_dir: &Path,
        lane_id: LaneId,
        epoch: u64,
        sequence: u64,
        storage_ticket: &StorageTicketId,
        fingerprint: &ReplayFingerprint,
        bytes: &[u8],
    ) -> io::Result<Option<PathBuf>> {
        persist_artifact(
            spool_dir,
            lane_id,
            epoch,
            sequence,
            storage_ticket,
            fingerprint,
            "taikai-ssm",
            "norito",
            bytes,
        )
    }

    pub(crate) fn persist_trm(
        spool_dir: &Path,
        lane_id: LaneId,
        epoch: u64,
        sequence: u64,
        storage_ticket: &StorageTicketId,
        fingerprint: &ReplayFingerprint,
        bytes: &[u8],
    ) -> io::Result<Option<PathBuf>> {
        persist_artifact(
            spool_dir,
            lane_id,
            epoch,
            sequence,
            storage_ticket,
            fingerprint,
            "taikai-trm",
            "norito",
            bytes,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn persist_artifact(
        spool_dir: &Path,
        lane_id: LaneId,
        epoch: u64,
        sequence: u64,
        storage_ticket: &StorageTicketId,
        fingerprint: &ReplayFingerprint,
        prefix: &str,
        extension: &str,
        bytes: &[u8],
    ) -> io::Result<Option<PathBuf>> {
        if spool_dir.as_os_str().is_empty() {
            return Ok(None);
        }

        let base_dir = spool_dir.join(TAIKAI_SPOOL_SUBDIR);
        fs::create_dir_all(&base_dir)?;

        let lane = lane_id.as_u32();
        let ticket_hex = hex::encode(storage_ticket.as_ref());
        let fingerprint_hex = hex::encode(fingerprint.as_bytes());
        let file_name = format!(
            "{prefix}-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.{extension}"
        );
        let target_path = base_dir.join(&file_name);
        if target_path.exists() {
            return Ok(Some(target_path));
        }

        let tmp_name = format!(
            ".{prefix}-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.tmp-{}",
            std::process::id()
        );
        let tmp_path = base_dir.join(tmp_name);

        match fs::write(&tmp_path, bytes) {
            Ok(()) => {}
            Err(err) => {
                let _ = fs::remove_file(&tmp_path);
                return Err(err);
            }
        }

        if let Err(err) = fs::rename(&tmp_path, &target_path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(err);
        }

        debug!(
            path = ?target_path,
            lane = lane,
            epoch,
            sequence,
            ticket = %ticket_hex,
            kind = prefix,
            "queued Taikai artefact for anchoring"
        );

        Ok(Some(target_path))
    }

    fn parse_u64(value: &str, key: &str) -> Result<u64, (StatusCode, String)> {
        value
            .trim()
            .parse::<u64>()
            .map_err(|err| bad_request(key, format!("invalid integer `{value}`: {err}")))
    }

    fn parse_u32(value: &str, key: &str) -> Result<u32, (StatusCode, String)> {
        value
            .trim()
            .parse::<u32>()
            .map_err(|err| bad_request(key, format!("invalid integer `{value}`: {err}")))
    }

    fn parse_i32(value: &str, key: &str) -> Result<i32, (StatusCode, String)> {
        value
            .trim()
            .parse::<i32>()
            .map_err(|err| bad_request(key, format!("invalid integer `{value}`: {err}")))
    }

    pub(crate) fn parse_name(
        metadata: &ExtraMetadata,
        key: &str,
    ) -> Result<Name, (StatusCode, String)> {
        let value = require_utf8(metadata, key)?;
        Name::from_str(value.trim())
            .map_err(|err| bad_request(key, format!("invalid Name `{value}`: {err}")))
    }

    fn require_utf8<'a>(
        metadata: &'a ExtraMetadata,
        key: &str,
    ) -> Result<&'a str, (StatusCode, String)> {
        let entry = metadata_entry(metadata, key)?;
        std::str::from_utf8(&entry.value).map_err(|_| bad_request(key, "value must be valid UTF-8"))
    }

    fn optional_utf8<'a>(
        metadata: &'a ExtraMetadata,
        key: &str,
    ) -> Result<Option<&'a str>, (StatusCode, String)> {
        let Some(entry) = metadata.items.iter().find(|entry| entry.key == key) else {
            return Ok(None);
        };
        validate_metadata_entry(entry).map_err(|message| bad_request(key, message))?;
        let value = std::str::from_utf8(&entry.value)
            .map_err(|_| bad_request(key, "value must be valid UTF-8"))?;
        Ok(Some(value))
    }

    pub(crate) fn take_ssm_entry(
        metadata: &mut ExtraMetadata,
    ) -> Result<Option<Vec<u8>>, (StatusCode, String)> {
        if let Some(index) = metadata
            .items
            .iter()
            .position(|entry| entry.key == META_TAIKAI_SSM)
        {
            let entry = metadata.items.remove(index);
            validate_metadata_entry(&entry)
                .map_err(|message| bad_request(META_TAIKAI_SSM, message))?;
            return Ok(Some(entry.value));
        }
        Ok(None)
    }

    pub(crate) fn take_trm_entry(
        metadata: &mut ExtraMetadata,
    ) -> Result<Option<Vec<u8>>, (StatusCode, String)> {
        if let Some(index) = metadata
            .items
            .iter()
            .position(|entry| entry.key == META_TAIKAI_TRM)
        {
            let entry = metadata.items.remove(index);
            validate_metadata_entry(&entry)
                .map_err(|message| bad_request(META_TAIKAI_TRM, message))?;
            return Ok(Some(entry.value));
        }
        Ok(None)
    }

    fn metadata_entry<'a>(
        metadata: &'a ExtraMetadata,
        key: &str,
    ) -> Result<&'a MetadataEntry, (StatusCode, String)> {
        let entry = metadata
            .items
            .iter()
            .find(|entry| entry.key == key)
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("metadata entry `{key}` is required for Taikai segments"),
                )
            })?;
        validate_metadata_entry(entry).map_err(|message| bad_request(key, message))?;
        Ok(entry)
    }

    fn validate_metadata_entry(entry: &MetadataEntry) -> Result<(), String> {
        if entry.visibility != MetadataVisibility::Public {
            return Err("must use public visibility".into());
        }
        if !matches!(entry.encryption, MetadataEncryption::None) {
            return Err("must not be encrypted".into());
        }
        Ok(())
    }

    pub(crate) fn bad_request(key: &str, message: impl Into<String>) -> (StatusCode, String) {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid Taikai metadata `{key}`: {}", message.into()),
        )
    }

    fn parse_error(key: &str, err: TaikaiParseError) -> (StatusCode, String) {
        bad_request(key, err.to_string())
    }

    pub(crate) fn internal_error(message: String) -> (StatusCode, String) {
        (StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    fn encode_base32_lower(data: &[u8]) -> String {
        const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
        if data.is_empty() {
            return String::new();
        }
        let mut acc = 0u32;
        let mut bits = 0u32;
        let mut out = Vec::with_capacity((data.len() * 8).div_ceil(5));
        for &byte in data {
            acc = (acc << 8) | u32::from(byte);
            bits += 8;
            while bits >= 5 {
                bits -= 5;
                let index = ((acc >> bits) & 0x1f) as usize;
                out.push(ALPHABET[index]);
            }
        }
        if bits > 0 {
            let index = ((acc << (5 - bits)) & 0x1f) as usize;
            out.push(ALPHABET[index]);
        }
        String::from_utf8(out).expect("alphabet contains valid ASCII")
    }

    pub fn spawn_anchor_worker(
        manifest_store_dir: PathBuf,
        anchor_cfg: DaTaikaiAnchor,
        shutdown: ShutdownSignal,
    ) {
        if anchor_cfg.poll_interval.is_zero() {
            iroha_logger::warn!("Taikai anchor poll interval is zero; using 1 second");
        }
        let poll_interval = if anchor_cfg.poll_interval.is_zero() {
            Duration::from_secs(1)
        } else {
            anchor_cfg.poll_interval
        };

        let spool_dir = manifest_store_dir.join(TAIKAI_SPOOL_SUBDIR);
        let sender = match HttpAnchorSender::new() {
            Ok(sender) => sender,
            Err(err) => {
                iroha_logger::error!(?err, "failed to initialise Taikai anchor HTTP client");
                return;
            }
        };

        tokio::spawn(async move {
            if let Err(err) = async_fs::create_dir_all(&spool_dir).await {
                if err.kind() != ErrorKind::AlreadyExists {
                    iroha_logger::warn!(
                        ?err,
                        ?spool_dir,
                        "failed to prepare Taikai spool directory"
                    );
                }
            }

            run_anchor_worker(spool_dir, anchor_cfg, sender, shutdown, poll_interval).await;
        });
    }

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use iroha_data_model::Encode;
    use tokio::{
        fs as async_fs,
        time::{MissedTickBehavior, interval},
    };

    struct HttpAnchorSender {
        client: Client,
    }

    impl HttpAnchorSender {
        fn new() -> Result<Self, reqwest::Error> {
            Ok(Self {
                client: Client::builder().build()?,
            })
        }
    }

    #[async_trait]
    pub(crate) trait AnchorSender: Send + Sync {
        async fn send(
            &self,
            endpoint: &reqwest::Url,
            body: String,
            api_token: Option<&str>,
        ) -> Result<(), reqwest::Error>;
    }

    #[async_trait]
    impl AnchorSender for HttpAnchorSender {
        async fn send(
            &self,
            endpoint: &reqwest::Url,
            body: String,
            api_token: Option<&str>,
        ) -> Result<(), reqwest::Error> {
            let mut request = self
                .client
                .post(endpoint.clone())
                .header("content-type", "application/json");
            if let Some(token) = api_token {
                request = request.header("authorization", token);
            }
            request.body(body).send().await?.error_for_status()?;
            Ok(())
        }
    }

    pub(crate) struct PendingUpload {
        base_id: String,
        body: String,
        sentinel_path: PathBuf,
    }

    impl PendingUpload {
        pub(crate) fn base_id(&self) -> &str {
            &self.base_id
        }

        pub(crate) fn body(&self) -> &str {
            &self.body
        }
    }

    async fn persist_anchor_request_capture(
        spool_dir: &Path,
        base_id: &str,
        body: &str,
    ) -> Result<(), std::io::Error> {
        let request_path = spool_dir.join(format!(
            "{TAIKAI_ANCHOR_REQUEST_PREFIX}{base_id}{TAIKAI_ANCHOR_REQUEST_SUFFIX}"
        ));
        if async_fs::metadata(&request_path).await.is_ok() {
            return Ok(());
        }
        async_fs::write(&request_path, body).await
    }

    async fn run_anchor_worker<S>(
        spool_dir: PathBuf,
        anchor_cfg: DaTaikaiAnchor,
        sender: S,
        shutdown: ShutdownSignal,
        poll_interval: Duration,
    ) where
        S: AnchorSender + 'static,
    {
        let mut ticker = interval(poll_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = shutdown.receive() => break,
                _ = ticker.tick() => {
                    if let Err(err) = process_batch(&spool_dir, &anchor_cfg, &sender).await {
                        iroha_logger::error!(?err, "failed to process Taikai anchor batch");
                    }
                }
            }
        }
    }

    pub(crate) async fn process_batch<S>(
        spool_dir: &Path,
        anchor_cfg: &DaTaikaiAnchor,
        sender: &S,
    ) -> Result<(), String>
    where
        S: AnchorSender + ?Sized,
    {
        let uploads = collect_pending_uploads(spool_dir).await?;
        if uploads.is_empty() {
            return Ok(());
        }

        for upload in uploads {
            match sender
                .send(
                    &anchor_cfg.endpoint,
                    upload.body.clone(),
                    anchor_cfg.api_token.as_deref(),
                )
                .await
            {
                Ok(()) => {
                    let marker = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                        .to_string();
                    if let Err(err) = async_fs::write(&upload.sentinel_path, marker).await {
                        iroha_logger::warn!(
                            ?err,
                            path = ?upload.sentinel_path,
                            "failed to write Taikai anchor sentinel"
                        );
                    }
                }
                Err(err) => {
                    iroha_logger::warn!(
                        ?err,
                        base = upload.base_id,
                        "failed to deliver Taikai envelope to anchor service"
                    );
                }
            }
        }

        Ok(())
    }

    pub(crate) async fn collect_pending_uploads(
        spool_dir: &Path,
    ) -> Result<Vec<PendingUpload>, String> {
        let mut result = Vec::new();
        let mut dir = match async_fs::read_dir(spool_dir).await {
            Ok(dir) => dir,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(result),
            Err(err) => {
                return Err(format!(
                    "failed to read Taikai spool directory `{}`: {err}",
                    spool_dir.display()
                ));
            }
        };

        while let Some(entry) = dir.next_entry().await.map_err(|err| {
            format!(
                "failed to iterate Taikai spool directory `{}`: {err}",
                spool_dir.display()
            )
        })? {
            let file_name = entry.file_name();
            let file_name = match file_name.to_str() {
                Some(value) => value,
                None => continue,
            };

            if !file_name.starts_with("taikai-envelope-") || !file_name.ends_with(".norito") {
                continue;
            }

            let base_id = &file_name["taikai-envelope-".len()..file_name.len() - ".norito".len()];
            let sentinel_path = spool_dir.join(format!(
                "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{base_id}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
            ));
            if async_fs::metadata(&sentinel_path).await.is_ok() {
                continue;
            }

            let envelope_path = entry.path();
            let indexes_name = format!("taikai-indexes-{base_id}.json");
            let indexes_path = spool_dir.join(&indexes_name);
            if async_fs::metadata(&indexes_path).await.is_err() {
                continue;
            }
            let ssm_name = format!("taikai-ssm-{base_id}.norito");
            let ssm_path = spool_dir.join(&ssm_name);
            if async_fs::metadata(&ssm_path).await.is_err() {
                continue;
            }

            let envelope_bytes = match async_fs::read(&envelope_path).await {
                Ok(bytes) => bytes,
                Err(err) => {
                    iroha_logger::warn!(?err, path = ?envelope_path, "failed to read Taikai envelope");
                    continue;
                }
            };

            let indexes_value: Value = match async_fs::read(&indexes_path).await {
                Ok(bytes) => match json::from_slice(&bytes) {
                    Ok(value) => value,
                    Err(err) => {
                        iroha_logger::warn!(?err, path = ?indexes_path, "failed to parse Taikai indexes JSON");
                        continue;
                    }
                },
                Err(err) => {
                    iroha_logger::warn!(?err, path = ?indexes_path, "failed to read Taikai indexes JSON");
                    continue;
                }
            };

            let ssm_bytes = match async_fs::read(&ssm_path).await {
                Ok(bytes) => bytes,
                Err(err) => {
                    iroha_logger::warn!(?err, path = ?ssm_path, "failed to read Taikai signing manifest");
                    continue;
                }
            };

            let trm_name = format!("taikai-trm-{base_id}.norito");
            let trm_path = spool_dir.join(&trm_name);
            let trm_bytes = match async_fs::read(&trm_path).await {
                Ok(bytes) => Some(bytes),
                Err(err) => {
                    if err.kind() != ErrorKind::NotFound {
                        iroha_logger::warn!(?err, path = ?trm_path, "failed to read Taikai routing manifest");
                    }
                    None
                }
            };
            let lineage_name = format!("{TAIKAI_LINEAGE_HINT_PREFIX}-{base_id}.json");
            let lineage_path = spool_dir.join(&lineage_name);
            let lineage_value = match async_fs::read(&lineage_path).await {
                Ok(bytes) => match json::from_slice(&bytes) {
                    Ok(value) => Some(value),
                    Err(err) => {
                        iroha_logger::warn!(
                            ?err,
                            path = ?lineage_path,
                            "failed to parse Taikai lineage hint JSON"
                        );
                        None
                    }
                },
                Err(err) => {
                    if err.kind() != ErrorKind::NotFound {
                        iroha_logger::warn!(
                            ?err,
                            path = ?lineage_path,
                            "failed to read Taikai lineage hint JSON"
                        );
                    }
                    None
                }
            };

            let mut payload = Map::new();
            let envelope_b64 = BASE64.encode(envelope_bytes);
            payload.insert("envelope_base64".to_string(), Value::String(envelope_b64));
            payload.insert("indexes".to_string(), indexes_value);
            let ssm_b64 = BASE64.encode(ssm_bytes);
            payload.insert("ssm_base64".to_string(), Value::String(ssm_b64));
            if let Some(trm_bytes) = trm_bytes {
                let trm_b64 = BASE64.encode(trm_bytes);
                payload.insert("trm_base64".to_string(), Value::String(trm_b64));
            }
            if let Some(value) = lineage_value {
                payload.insert("lineage_hint".to_string(), value);
            }
            let payload = Value::Object(payload);
            let body = match json::to_string(&payload) {
                Ok(body) => body,
                Err(err) => {
                    iroha_logger::warn!(
                        ?err,
                        base = base_id,
                        "failed to encode Taikai anchor payload"
                    );
                    continue;
                }
            };

            if let Err(err) = persist_anchor_request_capture(spool_dir, base_id, &body).await {
                iroha_logger::warn!(
                    ?err,
                    path = ?spool_dir.join(format!(
                        "{TAIKAI_ANCHOR_REQUEST_PREFIX}{base_id}{TAIKAI_ANCHOR_REQUEST_SUFFIX}"
                    )),
                    "failed to persist Taikai anchor request payload for governance bundle"
                );
            }

            result.push(PendingUpload {
                base_id: base_id.to_string(),
                body,
                sentinel_path,
            });
        }

        Ok(result)
    }
}

pub use taikai_ingest::spawn_anchor_worker;

/// Extract the Taikai stream label from metadata for telemetry tagging.
pub(crate) fn stream_label_from_metadata(metadata: &ExtraMetadata) -> Option<String> {
    metadata
        .items
        .iter()
        .find(|entry| entry.key == META_TAIKAI_STREAM_ID)
        .and_then(|entry| std::str::from_utf8(&entry.value).ok())
        .map(|value| value.trim().to_owned())
}

/// Validate the proof tier metadata against the enforced storage class.
pub(crate) fn validate_da_proof_tier(
    metadata: &ExtraMetadata,
    expected_storage_class: StorageClass,
) -> Result<(), (StatusCode, String)> {
    let entry = metadata
        .items
        .iter()
        .find(|entry| entry.key == META_DA_PROOF_TIER)
        .ok_or_else(|| {
            taikai_ingest::bad_request(
                META_DA_PROOF_TIER,
                "metadata entry is required for Taikai segments",
            )
        })?;
    if entry.visibility != MetadataVisibility::Public
        || !matches!(entry.encryption, MetadataEncryption::None)
    {
        return Err(taikai_ingest::bad_request(
            META_DA_PROOF_TIER,
            "metadata entry must be public and unencrypted",
        ));
    }
    let value = std::str::from_utf8(&entry.value)
        .map_err(|_| taikai_ingest::bad_request(META_DA_PROOF_TIER, "value must be UTF-8"))?
        .trim();
    let expected = storage_class_label(expected_storage_class);
    if value != expected {
        return Err(taikai_ingest::bad_request(
            META_DA_PROOF_TIER,
            format!("tier `{value}` does not match enforced storage class `{expected}`"),
        ));
    }
    Ok(())
}

/// Validate the Taikai cache hint metadata against the canonical payload.
pub(crate) fn validate_taikai_cache_hint(
    metadata: &ExtraMetadata,
    payload_digest: &BlobDigest,
    expected_payload_len: u64,
) -> Result<(), (StatusCode, String)> {
    let entry = metadata
        .items
        .iter()
        .find(|entry| entry.key == META_TAIKAI_CACHE_HINT)
        .ok_or_else(|| {
            taikai_ingest::bad_request(
                META_TAIKAI_CACHE_HINT,
                "metadata entry is required for Taikai segments",
            )
        })?;
    if entry.visibility != MetadataVisibility::Public
        || !matches!(entry.encryption, MetadataEncryption::None)
    {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_CACHE_HINT,
            "metadata entry must be public and unencrypted",
        ));
    }
    let hint_value: Value = json::from_slice(&entry.value).map_err(|err| {
        taikai_ingest::bad_request(
            META_TAIKAI_CACHE_HINT,
            format!("failed to parse cache hint JSON: {err}"),
        )
    })?;
    let hint = hint_value.as_object().ok_or_else(|| {
        taikai_ingest::bad_request(META_TAIKAI_CACHE_HINT, "cache hint must be a JSON object")
    })?;

    let event = taikai_ingest::parse_name(metadata, META_TAIKAI_EVENT_ID)?;
    let stream = taikai_ingest::parse_name(metadata, META_TAIKAI_STREAM_ID)?;
    let rendition = taikai_ingest::parse_name(metadata, META_TAIKAI_RENDITION_ID)?;
    let sequence_entry = metadata
        .items
        .iter()
        .find(|entry| entry.key == META_TAIKAI_SEGMENT_SEQUENCE)
        .ok_or_else(|| {
            taikai_ingest::bad_request(
                META_TAIKAI_CACHE_HINT,
                "cache hint validation requires taikai.segment.sequence metadata",
            )
        })?;
    if sequence_entry.visibility != MetadataVisibility::Public
        || !matches!(sequence_entry.encryption, MetadataEncryption::None)
    {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SEGMENT_SEQUENCE,
            "metadata entry must be public and unencrypted",
        ));
    }
    let sequence_str = std::str::from_utf8(&sequence_entry.value).map_err(|_| {
        taikai_ingest::bad_request(META_TAIKAI_SEGMENT_SEQUENCE, "value must be UTF-8")
    })?;
    let sequence = sequence_str.parse::<u64>().map_err(|err| {
        taikai_ingest::bad_request(
            META_TAIKAI_SEGMENT_SEQUENCE,
            format!("invalid integer `{sequence_str}`: {err}"),
        )
    })?;

    let expect_str = |key: &str| -> Result<&str, (StatusCode, String)> {
        hint.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                taikai_ingest::bad_request(
                    META_TAIKAI_CACHE_HINT,
                    format!("{key} is required in cache hint"),
                )
            })
    };
    let expect_u64 = |key: &str| -> Result<u64, (StatusCode, String)> {
        hint.get(key).and_then(Value::as_u64).ok_or_else(|| {
            taikai_ingest::bad_request(
                META_TAIKAI_CACHE_HINT,
                format!("{key} is required in cache hint"),
            )
        })
    };

    let event_value = expect_str("event")?;
    let stream_value = expect_str("stream")?;
    let rendition_value = expect_str("rendition")?;
    if event_value != event.as_ref()
        || stream_value != stream.as_ref()
        || rendition_value != rendition.as_ref()
    {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_CACHE_HINT,
            "cache hint identifiers must match segment metadata",
        ));
    }

    let sequence_value = expect_u64("sequence")?;
    if sequence_value != sequence {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_CACHE_HINT,
            "cache hint sequence must match segment metadata",
        ));
    }

    let payload_len = expect_u64("payload_len")?;
    if payload_len != expected_payload_len {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_CACHE_HINT,
            "cache hint payload_len must match canonical payload size",
        ));
    }

    let digest_hex = expect_str("payload_blake3_hex")?;
    let digest_bytes = hex::decode(digest_hex).map_err(|err| {
        taikai_ingest::bad_request(
            META_TAIKAI_CACHE_HINT,
            format!("payload_blake3_hex must be valid hex: {err}"),
        )
    })?;
    if digest_bytes.as_slice() != payload_digest.as_ref() {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_CACHE_HINT,
            "cache hint payload digest does not match canonical payload",
        ));
    }

    Ok(())
}

#[derive(Debug)]
/// Result details from validating a Taikai signing manifest.
pub(crate) struct TaikaiSsmOutcome {
    /// Namespaced alias label referenced by the SSM.
    pub(crate) alias_label: String,
    /// Digest of the signing manifest payload.
    pub(crate) ssm_digest: BlobDigest,
    /// Alias proof evaluation metadata.
    pub(crate) evaluation: AliasProofEvaluation,
}

/// Validate a Taikai signing manifest payload against ingest artifacts.
pub(crate) fn validate_taikai_ssm(
    ssm_bytes: &[u8],
    manifest_hash: &BlobDigest,
    car_digest: &BlobDigest,
    envelope_bytes: &[u8],
    expected_sequence: u64,
    alias_policy: &AliasCachePolicy,
    telemetry: &MaybeTelemetry,
) -> Result<TaikaiSsmOutcome, (StatusCode, String)> {
    let signing_manifest: TaikaiSegmentSigningManifestV1 =
        decode_from_bytes(ssm_bytes).map_err(|err| {
            taikai_ingest::bad_request(
                META_TAIKAI_SSM,
                format!("failed to decode signing manifest: {err}"),
            )
        })?;

    signing_manifest
        .signature
        .verify(&signing_manifest.body.publisher_key, &signing_manifest.body)
        .map_err(|err| {
            taikai_ingest::bad_request(
                META_TAIKAI_SSM,
                format!("publisher signature verification failed: {err}"),
            )
        })?;

    if &signing_manifest.body.manifest_hash != manifest_hash {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SSM,
            "manifest hash mismatch between SSM and ingest artefact",
        ));
    }
    if &signing_manifest.body.car_digest != car_digest {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SSM,
            "CAR digest mismatch between SSM and ingest artefact",
        ));
    }

    let envelope_hash = BlobDigest::from_hash(blake3_hash(envelope_bytes));
    if signing_manifest.body.segment_envelope_hash != envelope_hash {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SSM,
            "envelope hash mismatch between SSM and ingest artefact",
        ));
    }
    if signing_manifest.body.segment_sequence != expected_sequence {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SSM,
            "segment sequence mismatch between SSM and ingest metadata",
        ));
    }

    let alias_binding = &signing_manifest.body.alias_binding;
    if alias_binding.proof.is_empty() {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SSM,
            "alias proof payload must not be empty",
        ));
    }

    let alias_label = format!("{}/{}", alias_binding.namespace, alias_binding.name);
    let alias_proof = crate::sorafs::decode_alias_proof(&alias_binding.proof).map_err(|err| {
        taikai_ingest::bad_request(
            META_TAIKAI_SSM,
            format!("alias proof failed validation for `{alias_label}`: {err}"),
        )
    })?;
    if alias_proof.binding.alias != alias_label {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SSM,
            format!(
                "alias proof binding `{}` does not match signing manifest alias `{alias_label}`",
                alias_proof.binding.alias
            ),
        ));
    }

    let now_secs = crate::sorafs::unix_now_secs();
    let evaluation = alias_policy.evaluate(&alias_proof, now_secs);
    let status_label = evaluation.status_label();
    let result = if evaluation.state.is_servable() {
        "success"
    } else {
        "error"
    };
    telemetry.with_metrics(|metrics| {
        metrics.record_sorafs_alias_cache(result, status_label, evaluation.age.as_secs_f64());
    });

    if !evaluation.state.is_servable() {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SSM,
            format!("alias proof for `{alias_label}` expired ({status_label})"),
        ));
    }

    let ssm_digest = BlobDigest::from_hash(blake3_hash(ssm_bytes));
    Ok(TaikaiSsmOutcome {
        alias_label,
        ssm_digest,
        evaluation,
    })
}

/// Derive the Taikai availability class from the routing manifest metadata.
pub(crate) fn taikai_availability_from_metadata(
    metadata: &ExtraMetadata,
    trm_payload: Option<&[u8]>,
) -> Result<Option<TaikaiAvailabilityClass>, (StatusCode, String)> {
    let Some(bytes) = trm_payload else {
        return Ok(None);
    };
    let manifest: TaikaiRoutingManifestV1 = decode_from_bytes(bytes).map_err(|err| {
        taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!("failed to decode routing manifest: {err}"),
        )
    })?;
    let event_id = taikai_ingest::parse_name(metadata, META_TAIKAI_EVENT_ID)?;
    if manifest.event_id.as_name() != &event_id {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!(
                "manifest event_id `{}` does not match segment metadata `{}`",
                manifest.event_id.as_name(),
                event_id.as_ref()
            ),
        ));
    }
    let stream_id = taikai_ingest::parse_name(metadata, META_TAIKAI_STREAM_ID)?;
    if manifest.stream_id.as_name() != &stream_id {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!(
                "manifest stream_id `{}` does not match segment metadata `{}`",
                manifest.stream_id.as_name(),
                stream_id.as_ref()
            ),
        ));
    }
    let rendition_name = taikai_ingest::parse_name(metadata, META_TAIKAI_RENDITION_ID)?;
    let Some(route) = manifest
        .renditions
        .iter()
        .find(|route| route.rendition_id.as_name() == &rendition_name)
    else {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!(
                "manifest missing rendition `{}` required by this segment",
                rendition_name.as_ref()
            ),
        ));
    };
    Ok(Some(route.availability_class))
}

/// Apply Taikai-specific metadata tags for ingest and proof policy enforcement.
pub(crate) fn apply_taikai_ingest_tags(
    metadata: &mut ExtraMetadata,
    availability: Option<TaikaiAvailabilityClass>,
    retention: &RetentionPolicy,
    payload_digest: BlobDigest,
    payload_len: u64,
) -> Result<(), (StatusCode, String)> {
    let availability_class =
        availability.unwrap_or_else(|| TaikaiAvailabilityClass::from(retention.storage_class));
    upsert_metadata(
        metadata,
        META_TAIKAI_AVAILABILITY_CLASS,
        availability_label(availability_class),
    );
    upsert_metadata(
        metadata,
        META_DA_PROOF_TIER,
        storage_class_label(retention.storage_class),
    );
    upsert_metadata(
        metadata,
        META_TAIKAI_REPLICATION_REPLICAS,
        retention.required_replicas.to_string(),
    );
    upsert_metadata(
        metadata,
        META_TAIKAI_REPLICATION_STORAGE,
        storage_class_label(retention.storage_class),
    );
    upsert_metadata(
        metadata,
        META_TAIKAI_REPLICATION_HOT_SECS,
        retention.hot_retention_secs.to_string(),
    );
    upsert_metadata(
        metadata,
        META_TAIKAI_REPLICATION_COLD_SECS,
        retention.cold_retention_secs.to_string(),
    );

    let sample_window = compute_sample_window(payload_len);
    upsert_metadata(
        metadata,
        META_DA_PDP_SAMPLE_WINDOW,
        sample_window.to_string(),
    );
    upsert_metadata(
        metadata,
        META_DA_POTR_SAMPLE_WINDOW,
        sample_window.to_string(),
    );

    let event = taikai_ingest::parse_name(metadata, META_TAIKAI_EVENT_ID)?;
    let stream = taikai_ingest::parse_name(metadata, META_TAIKAI_STREAM_ID)?;
    let rendition = taikai_ingest::parse_name(metadata, META_TAIKAI_RENDITION_ID)?;
    let sequence_entry = metadata
        .items
        .iter()
        .find(|entry| entry.key == META_TAIKAI_SEGMENT_SEQUENCE)
        .ok_or_else(|| {
            taikai_ingest::bad_request(
                META_TAIKAI_SEGMENT_SEQUENCE,
                "metadata entry `taikai.segment.sequence` is required for Taikai segments",
            )
        })?;
    if sequence_entry.visibility != MetadataVisibility::Public {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SEGMENT_SEQUENCE,
            "metadata visibility must be public for Taikai segments",
        ));
    }
    if sequence_entry.encryption != MetadataEncryption::None {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_SEGMENT_SEQUENCE,
            "metadata encryption is not supported for Taikai sequence fields",
        ));
    }
    let sequence_raw = std::str::from_utf8(&sequence_entry.value).map_err(|_| {
        taikai_ingest::bad_request(META_TAIKAI_SEGMENT_SEQUENCE, "value must be UTF-8")
    })?;
    let sequence = sequence_raw.parse::<u64>().map_err(|err| {
        taikai_ingest::bad_request(
            META_TAIKAI_SEGMENT_SEQUENCE,
            format!("invalid u64 `{sequence_raw}`: {err}"),
        )
    })?;

    let mut hint = Map::new();
    hint.insert("event".into(), Value::from(event.as_ref()));
    hint.insert("stream".into(), Value::from(stream.as_ref()));
    hint.insert("rendition".into(), Value::from(rendition.as_ref()));
    hint.insert("sequence".into(), Value::from(sequence));
    hint.insert("payload_len".into(), Value::from(payload_len));
    hint.insert(
        "payload_blake3_hex".into(),
        Value::from(hex::encode(payload_digest.as_ref())),
    );
    let rendered_hint = json::to_vec(&Value::Object(hint)).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to encode Taikai cache hint: {err}"),
        )
    })?;
    upsert_metadata(metadata, META_TAIKAI_CACHE_HINT, rendered_hint);

    Ok(())
}

/// Compute the Taikai ingest metadata enrichment applied by Torii.
///
/// # Errors
///
/// Returns a `(StatusCode, String)` when metadata serialization or tag
/// computation fails for the supplied payload.
pub fn compute_taikai_ingest_tags(
    mut metadata: ExtraMetadata,
    availability: Option<TaikaiAvailabilityClass>,
    retention: &RetentionPolicy,
    payload_digest: BlobDigest,
    payload_len: u64,
) -> Result<ExtraMetadata, (StatusCode, String)> {
    apply_taikai_ingest_tags(
        &mut metadata,
        availability,
        retention,
        payload_digest,
        payload_len,
    )?;
    Ok(metadata)
}

fn upsert_metadata(metadata: &mut ExtraMetadata, key: &str, value: impl Into<Vec<u8>>) {
    if let Some(index) = metadata.items.iter().position(|entry| entry.key == key) {
        metadata.items.remove(index);
    }
    metadata.items.push(MetadataEntry::new(
        key,
        value.into(),
        MetadataVisibility::Public,
    ));
}

fn availability_label(class: TaikaiAvailabilityClass) -> &'static str {
    match class {
        TaikaiAvailabilityClass::Hot => "hot",
        TaikaiAvailabilityClass::Warm => "warm",
        TaikaiAvailabilityClass::Cold => "cold",
    }
}

/// Validate a Taikai routing manifest payload against the segment envelope.
pub(crate) fn validate_taikai_trm(
    trm_bytes: &[u8],
    envelope: &taikai_ingest::EnvelopeArtifacts,
) -> Result<TaikaiRoutingManifestV1, (StatusCode, String)> {
    let manifest: TaikaiRoutingManifestV1 = decode_from_bytes(trm_bytes).map_err(|err| {
        taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!("failed to decode routing manifest: {err}"),
        )
    })?;

    if manifest.version != TaikaiRoutingManifestV1::VERSION {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!(
                "unsupported manifest version {}; expected {}",
                manifest.version,
                TaikaiRoutingManifestV1::VERSION
            ),
        ));
    }

    if let Err(err) = manifest.validate() {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!("invalid routing manifest: {err}"),
        ));
    }

    if manifest.event_id.as_name().as_ref() != envelope.telemetry.event_id {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!(
                "manifest event_id `{}` does not match segment metadata `{}`",
                manifest.event_id.as_name(),
                envelope.telemetry.event_id
            ),
        ));
    }

    if manifest.stream_id.as_name().as_ref() != envelope.telemetry.stream_id {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!(
                "manifest stream_id `{}` does not match segment metadata `{}`",
                manifest.stream_id.as_name(),
                envelope.telemetry.stream_id
            ),
        ));
    }

    let expected_rendition = envelope.telemetry.rendition_id.as_str();
    if !manifest
        .renditions
        .iter()
        .any(|route| route.rendition_id.as_name().as_ref() == expected_rendition)
    {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            format!("manifest missing rendition `{expected_rendition}` required by this segment"),
        ));
    }

    if !manifest.covers_sequence(envelope.telemetry.segment_sequence) {
        return Err(taikai_ingest::bad_request(
            META_TAIKAI_TRM,
            "manifest window does not cover this segment sequence",
        ));
    }

    Ok(manifest)
}

/// Record Taikai ingest latency/drift metrics for telemetry.
pub(crate) fn record_taikai_ingest_metrics(
    telemetry: &MaybeTelemetry,
    cluster_label: &str,
    sample: &taikai_ingest::TaikaiTelemetrySample,
) {
    if !telemetry.is_enabled() {
        return;
    }
    telemetry.with_metrics(|handle| {
        if let Some(latency) = sample.ingest_latency_ms {
            handle.observe_taikai_ingest_latency(cluster_label, sample.stream_id.as_str(), latency);
        }
        if let Some(drift) = sample.live_edge_drift_ms {
            handle.observe_taikai_live_edge_drift(cluster_label, sample.stream_id.as_str(), drift);
        }
    });
}

/// Record a Taikai alias rotation event in telemetry.
pub(crate) fn record_taikai_alias_rotation_event(
    telemetry: &MaybeTelemetry,
    cluster_label: &str,
    manifest: &TaikaiRoutingManifestV1,
    manifest_digest_hex: &str,
) {
    if !telemetry.is_enabled() {
        return;
    }
    let event_label = manifest.event_id.as_name().as_ref().to_owned();
    let stream_label = manifest.stream_id.as_name().as_ref().to_owned();
    let alias_namespace = manifest.alias_binding.namespace.clone();
    let alias_name = manifest.alias_binding.name.clone();
    let window = manifest.segment_window;
    telemetry.with_metrics(|handle| {
        handle.record_taikai_alias_rotation(
            cluster_label,
            &event_label,
            &stream_label,
            &alias_namespace,
            &alias_name,
            window.start_sequence,
            window.end_sequence,
            manifest_digest_hex,
        );
    });
}

/// Record a Taikai ingest error classified by status code.
pub(crate) fn record_taikai_ingest_error(
    telemetry: &MaybeTelemetry,
    cluster_label: &str,
    stream_label: &str,
    status: StatusCode,
) {
    if !telemetry.is_enabled() {
        return;
    }
    let reason = status
        .canonical_reason()
        .map(Cow::Borrowed)
        .unwrap_or_else(|| Cow::Owned(status.as_u16().to_string()));
    telemetry.with_metrics(|handle| {
        handle.inc_taikai_ingest_error(cluster_label, stream_label, &reason);
    });
}
