//! Taikai publisher tooling.
//!
//! Provides helpers for bundling broadcast segments into deterministic CAR
//! archives and Norito envelopes so downstream services can index and publish
//! them without re-deriving ingest metadata.

use super::da_common::{
    DaPublisher, metadata_map_to_extra, parse_blob_class, parse_fec_scheme, parse_storage_class,
};
use crate::cli_output::print_with_optional_text;
use crate::{CliOutputFormat, Run, RunContext};
use blake3::Hasher;
use clap::ValueEnum;
use eyre::{Result, WrapErr, eyre};
use iroha::{
    config::Config,
    da::{self, DaIngestParams},
    data_model::{
        da::types::{
            BlobCodec, BlobDigest, ErasureProfile, ExtraMetadata, GovernanceTag, RetentionPolicy,
            StorageTicketId,
        },
        name::Name,
        nexus::LaneId,
        taikai::{
            CEK_ROTATION_RECEIPT_VERSION_V1, CekRotationReceiptV1,
            REPLICATION_PROOF_TOKEN_VERSION_V1, ReplicationProofTokenV1, TaikaiAudioLayout,
            TaikaiCodec, TaikaiEventId, TaikaiRenditionId, TaikaiResolution, TaikaiStreamId,
            TaikaiTrackMetadata,
        },
    },
};
use norito::{
    NoritoSerialize,
    json::{self, JsonSerialize, Map, Value},
};
use rand::{Rng, RngCore, SeedableRng, rngs::StdRng};
use sorafs_car::taikai::{BundleRequest, BundleSummary, bundle_segment, load_extra_metadata};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    env,
    fmt::Write as _,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const DEFAULT_LADDER_PRESETS_JSON: &str =
    include_str!("../../../../fixtures/taikai/ladder_presets.json");

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Bundle a Taikai segment into a CAR archive and Norito envelope.
    Bundle(BundleArgs),
    /// Emit a CEK rotation receipt for a Taikai stream.
    CekRotate(CekRotateArgs),
    /// Generate a replication proof token (RPT) attestation.
    RptAttest(RptAttestArgs),
    /// Taikai ingest helpers (watchers, automation).
    #[command(subcommand)]
    Ingest(IngestCommand),
}

impl Run for Command {
    #[allow(clippy::too_many_lines)]
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Bundle(args) => args.run(context),
            Command::CekRotate(args) => args.run(context),
            Command::RptAttest(args) => args.run(context),
            Command::Ingest(cmd) => cmd.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum IngestCommand {
    /// Watch a directory for CMAF fragments and bundle them into CAR + Norito artifacts.
    Watch(IngestWatchArgs),
    /// Prototype edge receiver that emits CMAF fragments and drift logs for the watcher.
    Edge(IngestEdgeArgs),
}

impl Run for IngestCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Watch(args) => args.run(context),
            Self::Edge(args) => args.run(context),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum CliTrackKind {
    Video,
    Audio,
    Data,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum EdgeProtocol {
    Srt,
    Rtmp,
}

impl EdgeProtocol {
    fn as_str(self) -> &'static str {
        match self {
            EdgeProtocol::Srt => "srt",
            EdgeProtocol::Rtmp => "rtmp",
        }
    }

    fn slug(self) -> &'static str {
        self.as_str()
    }
}

#[derive(clap::Args, Debug)]
pub struct BundleArgs {
    /// Path to the CMAF fragment or segment payload to ingest.
    #[arg(long, value_name = "PATH")]
    pub payload: PathBuf,
    /// Where to write the generated `CARv2` archive.
    #[arg(long, value_name = "PATH")]
    pub car_out: PathBuf,
    /// Where to write the Norito-encoded Taikai segment envelope.
    #[arg(long, value_name = "PATH")]
    pub envelope_out: PathBuf,
    /// Optional path for a JSON file containing the time/CID index keys.
    #[arg(long, value_name = "PATH")]
    pub indexes_out: Option<PathBuf>,
    /// Optional path for the ingest metadata JSON map consumed by `/v1/da/ingest`.
    #[arg(long, value_name = "PATH")]
    pub ingest_metadata_out: Option<PathBuf>,
    /// Identifier of the Taikai event.
    #[arg(long, value_name = "NAME")]
    pub event_id: String,
    /// Logical stream identifier within the event.
    #[arg(long, value_name = "NAME")]
    pub stream_id: String,
    /// Rendition identifier (ladder rung).
    #[arg(long, value_name = "NAME")]
    pub rendition_id: String,
    /// Track kind carried by the segment.
    #[arg(long, value_enum)]
    pub track_kind: CliTrackKind,
    /// Codec identifier (`avc-high`, `hevc-main10`, `av1-main`, `aac-lc`, `opus`, or `custom:<name>`).
    #[arg(long)]
    pub codec: String,
    /// Average bitrate in kilobits per second.
    #[arg(long, value_name = "KBPS")]
    pub bitrate_kbps: u32,
    /// Video resolution (`WIDTHxHEIGHT`). Required for `video` tracks.
    #[arg(long)]
    pub resolution: Option<String>,
    /// Audio layout (`mono`, `stereo`, `5.1`, `7.1`, or `custom:<channels>`). Required for `audio` tracks.
    #[arg(long)]
    pub audio_layout: Option<String>,
    /// Monotonic segment sequence number.
    #[arg(long)]
    pub segment_sequence: u64,
    /// Presentation timestamp (start) in microseconds since stream origin.
    #[arg(long)]
    pub segment_start_pts: u64,
    /// Presentation duration in microseconds.
    #[arg(long)]
    pub segment_duration: u32,
    /// Wall-clock reference (Unix milliseconds) when the segment was finalised.
    #[arg(long)]
    pub wallclock_unix_ms: u64,
    /// Deterministic manifest hash emitted by the ingest pipeline (hex).
    #[arg(long, value_name = "HEX")]
    pub manifest_hash: String,
    /// Storage ticket identifier assigned by the orchestrator (hex).
    #[arg(long, value_name = "HEX")]
    pub storage_ticket: String,
    /// Optional encoder-to-ingest latency in milliseconds.
    #[arg(long)]
    pub ingest_latency_ms: Option<u32>,
    /// Optional live-edge drift measurement in milliseconds (negative = stream ahead of ingest).
    #[arg(long)]
    pub live_edge_drift_ms: Option<i32>,
    /// Optional identifier for the ingest node that sealed the segment.
    #[arg(long)]
    pub ingest_node_id: Option<String>,
    /// Optional JSON file describing additional metadata entries.
    #[arg(long, value_name = "PATH")]
    pub metadata_json: Option<PathBuf>,
}

impl Run for BundleArgs {
    #[allow(clippy::too_many_lines)]
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let manifest_hash = parse_blob_digest(&self.manifest_hash, "manifest-hash")?;
        let storage_ticket =
            StorageTicketId::new(parse_hex_32(&self.storage_ticket, "storage-ticket")?);
        let event_name = parse_name(&self.event_id, "event-id")?;
        let stream_name = parse_name(&self.stream_id, "stream-id")?;
        let rendition_name = parse_name(&self.rendition_id, "rendition-id")?;
        let track = build_track_metadata(&self)?;
        let extra_metadata = match &self.metadata_json {
            Some(path) => Some(load_extra_metadata(path)?),
            None => None,
        };

        let summary = bundle_segment(&BundleRequest {
            payload_path: &self.payload,
            payload_bytes: None,
            car_out: &self.car_out,
            envelope_out: &self.envelope_out,
            indexes_out: self.indexes_out.as_deref(),
            ingest_metadata_out: self.ingest_metadata_out.as_deref(),
            manifest_hash,
            storage_ticket,
            event_id: TaikaiEventId::new(event_name),
            stream_id: TaikaiStreamId::new(stream_name),
            rendition_id: TaikaiRenditionId::new(rendition_name),
            track,
            segment_sequence: self.segment_sequence,
            segment_start_pts: self.segment_start_pts,
            segment_duration: self.segment_duration,
            wallclock_unix_ms: self.wallclock_unix_ms,
            ingest_latency_ms: self.ingest_latency_ms,
            live_edge_drift_ms: self.live_edge_drift_ms,
            ingest_node_id: self.ingest_node_id.clone(),
            extra_metadata,
        })?;

        let summary_value = build_bundle_summary_value(&summary);
        let text = render_bundle_summary_text(&summary);
        print_with_optional_text(context, Some(text), &summary_value)
    }
}

#[derive(clap::Args, Debug)]
pub struct IngestEdgeArgs {
    /// Path to a sample fragment payload (treated as CMAF bytes).
    #[arg(long, value_name = "PATH")]
    pub payload: PathBuf,
    /// Optional output root; defaults to `./artifacts/taikai/ingest_edge_run_<timestamp>/`.
    #[arg(long, value_name = "PATH")]
    pub output_root: Option<PathBuf>,
    /// Number of fragments to emit into the watcher source directory.
    #[arg(long, default_value_t = 4)]
    pub segments: u64,
    /// Presentation timestamp (start) in microseconds for the first emitted segment.
    #[arg(long, value_name = "MICROS", default_value_t = 0)]
    pub first_segment_pts: u64,
    /// Interval between segments in milliseconds (controls PTS and wallclock spacing).
    #[arg(long, value_name = "MILLIS", default_value_t = 2_000)]
    pub segment_interval_ms: u64,
    /// Base drift in milliseconds applied to every segment (positive = ingest behind live edge).
    #[arg(long, value_name = "MILLIS", default_value_t = 0)]
    pub drift_ms: i32,
    /// Jitter window in milliseconds applied around the base drift.
    #[arg(long, value_name = "MILLIS", default_value_t = 0)]
    pub drift_jitter_ms: u32,
    /// Optional RNG seed for drift jitter so CI runs stay deterministic.
    #[arg(long, value_name = "SEED")]
    pub drift_seed: Option<u64>,
    /// Optional Unix timestamp for the first emitted segment; defaults to now.
    #[arg(long, value_name = "UNIX_MS")]
    pub start_unix_ms: Option<u64>,
    /// Optional identifier for the ingest edge node recorded in drift logs.
    #[arg(long)]
    pub ingest_node_id: Option<String>,
    /// Protocol label attached to the emitted fragments.
    #[arg(long, value_enum, default_value_t = EdgeProtocol::Srt)]
    pub protocol: EdgeProtocol,
}

impl Run for IngestEdgeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if self.segments == 0 {
            return Err(eyre!("--segments must be greater than zero"));
        }
        if self.segment_interval_ms == 0 {
            return Err(eyre!("--segment-interval-ms must be greater than zero"));
        }
        let payload_path = self
            .payload
            .canonicalize()
            .wrap_err_with(|| format!("failed to access payload `{}`", self.payload.display()))?;
        let payload = fs::read(&payload_path).wrap_err_with(|| {
            format!(
                "failed to read payload bytes from `{}`",
                payload_path.display()
            )
        })?;
        if payload.is_empty() {
            return Err(eyre!(
                "payload `{}` is empty; provide a CMAF fragment to seed the watcher",
                payload_path.display()
            ));
        }
        let layout = EdgeOutputLayout::new(self.output_root.clone())?;
        let mut rng = build_edge_rng(self.drift_seed);
        let start_unix_ms = match self.start_unix_ms {
            Some(value) => value,
            None => unix_timestamp_ms()?,
        };
        let mut drift_stats = DriftAccumulator::new();
        let mut log_writer = EdgeLogWriter::create(&layout.log_path)?;
        for sequence in 0..self.segments {
            let fragment = layout.fragment_path(self.protocol, sequence);
            fs::write(&fragment, &payload)
                .wrap_err_with(|| format!("failed to write fragment `{}`", fragment.display()))?;
            let pts_increment = self
                .segment_interval_ms
                .checked_mul(1_000)
                .ok_or_else(|| eyre!("segment interval overflowed when deriving PTS"))?;
            let pts = self
                .first_segment_pts
                .checked_add(
                    sequence
                        .checked_mul(pts_increment)
                        .ok_or_else(|| eyre!("segment PTS calculation overflowed"))?,
                )
                .ok_or_else(|| eyre!("segment PTS overflowed"))?;
            let wallclock_unix_ms = start_unix_ms
                .checked_add(
                    sequence
                        .checked_mul(self.segment_interval_ms)
                        .ok_or_else(|| eyre!("wallclock calculation overflowed"))?,
                )
                .ok_or_else(|| eyre!("wallclock calculation overflowed"))?;
            let drift_ms = jittered_drift(self.drift_ms, self.drift_jitter_ms, &mut rng);
            drift_stats.record(drift_ms);
            let entry = EdgeLogEntry {
                fragment,
                protocol: self.protocol,
                sequence,
                segment_start_pts: pts,
                wallclock_unix_ms,
                drift_ms,
                ingest_node_id: self.ingest_node_id.clone(),
            };
            log_writer.write_entry(&entry)?;
        }
        let summary = drift_stats
            .into_summary(self.protocol, self.segments, payload.len(), start_unix_ms)
            .with_output(&layout, self.first_segment_pts, self.segment_interval_ms)
            .with_node(self.ingest_node_id.as_deref());
        summary.write(&layout.drift_summary_path)?;
        let summary_value = build_edge_output_value(&summary, &layout.drift_summary_path);
        let text = render_edge_summary_text(
            &summary,
            &layout.drift_summary_path,
            self.drift_ms,
            self.drift_jitter_ms,
        );
        print_with_optional_text(context, Some(text), &summary_value)
    }
}

#[derive(clap::Args, Debug)]
pub struct IngestWatchArgs {
    /// Directory that receives CMAF fragments (e.g., `.m4s` files).
    #[arg(long, value_name = "PATH")]
    pub source_dir: PathBuf,
    /// Optional output root; defaults to `./artifacts/taikai/ingest_run_<timestamp>/`.
    #[arg(long, value_name = "PATH")]
    pub output_root: Option<PathBuf>,
    /// Optional NDJSON summary file containing one entry per processed segment.
    #[arg(long, value_name = "PATH")]
    pub summary_out: Option<PathBuf>,
    /// Identifier of the Taikai event.
    #[arg(long, value_name = "NAME")]
    pub event_id: String,
    /// Logical stream identifier within the event.
    #[arg(long, value_name = "NAME")]
    pub stream_id: String,
    /// Rendition identifier (ladder rung).
    #[arg(long, value_name = "NAME")]
    pub rendition_id: String,
    /// CMAF segment duration in microseconds (defaults to 2 s).
    #[arg(long, value_name = "MICROS", default_value_t = 2_000_000)]
    pub segment_duration: u32,
    /// Presentation timestamp (start) in microseconds for the first processed segment.
    #[arg(long, value_name = "MICROS", default_value_t = 0)]
    pub first_segment_pts: u64,
    /// Sequence number to use for the first processed segment.
    #[arg(long, default_value_t = 0)]
    pub sequence_start: u64,
    /// Optional ladder preset identifier (see `fixtures/taikai/ladder_presets.json`).
    #[arg(long)]
    pub ladder_preset: Option<String>,
    /// Optional override path for the ladder preset JSON catalog.
    #[arg(long, value_name = "PATH")]
    pub ladder_presets: Option<PathBuf>,
    /// Override for the track kind when not using a preset.
    #[arg(long, value_enum)]
    pub track_kind: Option<CliTrackKind>,
    /// Override for the codec identifier.
    #[arg(long)]
    pub codec: Option<String>,
    /// Override for the average bitrate in kilobits per second.
    #[arg(long)]
    pub bitrate_kbps: Option<u32>,
    /// Override for the video resolution (`WIDTHxHEIGHT`).
    #[arg(long)]
    pub resolution: Option<String>,
    /// Override for the audio layout (`mono`, `stereo`, etc.).
    #[arg(long)]
    pub audio_layout: Option<String>,
    /// Optional encoder-to-ingest latency in milliseconds (computed from file timestamps when omitted).
    #[arg(long)]
    pub ingest_latency_ms: Option<u32>,
    /// Optional identifier for the ingest node that sealed the segment.
    #[arg(long)]
    pub ingest_node_id: Option<String>,
    /// Optional JSON file describing additional metadata entries to attach to each envelope.
    #[arg(long, value_name = "PATH")]
    pub metadata_json: Option<PathBuf>,
    /// File extensions to watch (repeat the flag to add more).
    #[arg(long = "match-ext", value_name = "EXT", default_value = "m4s")]
    pub match_ext: Vec<String>,
    /// Optional limit on the number of processed segments before exiting.
    #[arg(long, value_name = "COUNT")]
    pub max_segments: Option<u64>,
    /// Poll interval in milliseconds between directory scans.
    #[arg(long, value_name = "MILLIS", default_value_t = 1000)]
    pub poll_interval_ms: u64,
    /// Drift warning threshold in milliseconds.
    #[arg(long, value_name = "MILLIS", default_value_t = 1500)]
    pub drift_warn_ms: u64,
    /// Lane identifier supplied in DA ingest requests (default: 0 / single-lane).
    #[arg(long = "da-lane", default_value_t = 0)]
    pub da_lane: u32,
    /// Epoch identifier for DA ingest requests.
    #[arg(long = "da-epoch", default_value_t = 0)]
    pub da_epoch: u64,
    /// Blob-class label (`taikai_segment`, `nexus_lane_sidecar`, `governance_artifact`, `custom:<id>`).
    #[arg(long = "da-blob-class", default_value = "taikai_segment")]
    pub da_blob_class: String,
    /// Codec label recorded in DA ingest requests (default `taikai.cmaf`).
    #[arg(long = "da-blob-codec", default_value = "taikai.cmaf")]
    pub da_blob_codec: String,
    /// Chunk size in bytes used for DA ingest requests.
    #[arg(
        long = "da-chunk-size",
        value_name = "BYTES",
        default_value_t = 262_144
    )]
    pub da_chunk_size: u32,
    /// Number of data shards for the erasure profile (default 10).
    #[arg(long = "da-data-shards", default_value_t = 10)]
    pub da_data_shards: u16,
    /// Number of parity shards for the erasure profile (default 4).
    #[arg(long = "da-parity-shards", default_value_t = 4)]
    pub da_parity_shards: u16,
    /// Chunk alignment (chunks per availability slice).
    #[arg(long = "da-chunk-alignment", default_value_t = 10)]
    pub da_chunk_alignment: u16,
    /// FEC scheme label (`rs12_10`, `rswin14_10`, `rs18_14`, `custom:<id>`).
    #[arg(long = "da-fec-scheme", default_value = "rs12_10")]
    pub da_fec_scheme: String,
    /// Hot-retention period in seconds.
    #[arg(long = "da-hot-retention-secs", default_value_t = 604_800)]
    pub da_hot_retention_secs: u64,
    /// Cold-retention period in seconds.
    #[arg(long = "da-cold-retention-secs", default_value_t = 7_776_000)]
    pub da_cold_retention_secs: u64,
    /// Required replica count for DA retention.
    #[arg(long = "da-required-replicas", default_value_t = 3)]
    pub da_required_replicas: u16,
    /// Storage class label for DA retention (`hot`, `warm`, `cold`).
    #[arg(long = "da-storage-class", default_value = "hot")]
    pub da_storage_class: String,
    /// Governance tag recorded in the retention policy (default `da.taikai.live`).
    #[arg(long = "da-governance-tag", default_value = "da.taikai.live")]
    pub da_governance_tag: String,
    /// Toggle automatic publishing to `/v1/da/ingest` using the CLI config.
    #[arg(long = "publish-da")]
    pub publish_da: bool,
    /// Override the Torii DA ingest endpoint (defaults to `$TORII/v1/da/ingest`).
    #[arg(long = "da-endpoint", value_name = "URL")]
    pub da_endpoint: Option<String>,
}

impl Run for IngestWatchArgs {
    #[allow(clippy::too_many_lines)]
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if self.segment_duration == 0 {
            return Err(eyre!("--segment-duration must be greater than zero"));
        }
        let source_dir = self.source_dir.canonicalize().wrap_err_with(|| {
            format!(
                "failed to access source directory `{}`",
                self.source_dir.display()
            )
        })?;
        if !source_dir.is_dir() {
            return Err(eyre!(
                "source directory `{}` is not a directory",
                source_dir.display()
            ));
        }
        let output_layout = IngestOutputLayout::new(self.output_root.clone())?;
        let mut summary_writer = match &self.summary_out {
            Some(path) => Some(IngestSummaryLog::create(path)?),
            None => None,
        };
        let da_opts = build_da_ingest_opts(&self)?;
        let publisher = if self.publish_da {
            Some(DaPublisher::new(
                context.config(),
                self.da_endpoint.as_deref(),
            )?)
        } else {
            None
        };
        let preset_store = LadderPresetStore::load(self.ladder_presets.as_deref())?;
        let descriptor = resolve_track_descriptor(&self, &preset_store)?;
        let track = descriptor.build_metadata()?;
        let extra_metadata = match &self.metadata_json {
            Some(path) => Some(load_extra_metadata(path)?),
            None => None,
        };
        let event_name = parse_name(&self.event_id, "event-id")?;
        let stream_name = parse_name(&self.stream_id, "stream-id")?;
        let rendition_name = parse_name(&self.rendition_id, "rendition-id")?;
        let event_id = TaikaiEventId::new(event_name);
        let stream_id = TaikaiStreamId::new(stream_name);
        let rendition_id = TaikaiRenditionId::new(rendition_name);
        let poll_interval = Duration::from_millis(self.poll_interval_ms.max(100));
        let matcher = ExtensionMatcher::new(&self.match_ext)?;
        let metadata_template = extra_metadata;
        let anchors = DriftAnchors::new(self.first_segment_pts)?;
        let mut seen = HashSet::new();
        let mut processed = 0_u64;
        let mut sequence = self.sequence_start;
        let mut pts = self.first_segment_pts;
        context.println(format_args!(
            "Taikai ingest watcher: source=`{}` output=`{}` duration={}µs preset={}",
            source_dir.display(),
            output_layout.root.display(),
            self.segment_duration,
            self.ladder_preset
                .as_deref()
                .unwrap_or("<manual configuration>")
        ))?;
        loop {
            let mut batch = Vec::new();
            for entry in fs::read_dir(&source_dir)
                .wrap_err_with(|| format!("failed to read `{}`", source_dir.display()))?
            {
                let entry = entry
                    .wrap_err_with(|| format!("failed to iterate `{}`", source_dir.display()))?;
                let path = entry.path();
                if !path.is_file() || !matcher.matches(&path) {
                    continue;
                }
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                if seen.contains(&canonical) {
                    continue;
                }
                batch.push(canonical);
            }
            batch.sort();
            for path in batch {
                if self.max_segments.is_some_and(|limit| processed >= limit) {
                    context.println("ingest watcher reached --max-segments; exiting")?;
                    return Ok(());
                }
                let summary = process_ingest_file(
                    &path,
                    &output_layout,
                    &da_opts,
                    publisher.as_ref(),
                    context.config(),
                    &event_id,
                    &stream_id,
                    &rendition_id,
                    &track,
                    metadata_template.as_ref(),
                    sequence,
                    pts,
                    self.segment_duration,
                    self.ingest_latency_ms,
                    self.ingest_node_id.as_deref(),
                    &anchors,
                )?;
                emit_ingest_summary(context, &path, &summary, processed, self.drift_warn_ms)?;
                if let Some(writer) = summary_writer.as_mut() {
                    writer.write_entry(&path, &summary)?;
                }
                seen.insert(path);
                processed = processed
                    .checked_add(1)
                    .ok_or_else(|| eyre!("processed segment counter overflowed"))?;
                sequence = sequence
                    .checked_add(1)
                    .ok_or_else(|| eyre!("segment sequence overflowed"))?;
                pts = pts
                    .checked_add(u64::from(self.segment_duration))
                    .ok_or_else(|| eyre!("segment PTS overflowed"))?;
            }
            if self.max_segments.is_some_and(|limit| processed >= limit) {
                context.println("ingest watcher completed requested workload")?;
                return Ok(());
            }
            thread::sleep(poll_interval);
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct CekRotateArgs {
    /// Identifier of the Taikai event.
    #[arg(long, value_name = "NAME")]
    pub event_id: String,
    /// Stream identifier within the event.
    #[arg(long, value_name = "NAME")]
    pub stream_id: String,
    /// Named KMS profile (e.g., `nitro:prod`).
    #[arg(long)]
    pub kms_profile: String,
    /// Label of the new wrap key minted by the KMS.
    #[arg(long)]
    pub new_wrap_key_label: String,
    /// Optional label for the previously active wrap key.
    #[arg(long)]
    pub previous_wrap_key_label: Option<String>,
    /// Segment sequence where the new CEK becomes active.
    #[arg(long, value_name = "SEQ")]
    pub effective_segment: u64,
    /// Optional HKDF salt (hex). Generated randomly when omitted.
    #[arg(long, value_name = "HEX")]
    pub hkdf_salt: Option<String>,
    /// Optional Unix timestamp override for the issued-at field.
    #[arg(long)]
    pub issued_at_unix: Option<u64>,
    /// Optional operator or governance notes.
    #[arg(long)]
    pub notes: Option<String>,
    /// Path to the Norito-encoded receipt output.
    #[arg(long, value_name = "PATH")]
    pub out: PathBuf,
    /// Optional JSON summary output path.
    #[arg(long, value_name = "PATH")]
    pub json_out: Option<PathBuf>,
}

impl Run for CekRotateArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let event_name = parse_name(&self.event_id, "event-id")?;
        let stream_name = parse_name(&self.stream_id, "stream-id")?;
        let hkdf_salt = if let Some(hex) = &self.hkdf_salt {
            parse_hex_32(hex, "hkdf-salt")?
        } else {
            let mut salt = [0u8; 32];
            rand::rng().fill_bytes(&mut salt);
            salt
        };
        let issued_at_unix = match self.issued_at_unix {
            Some(value) => value,
            None => current_unix_timestamp()
                .wrap_err("failed to read system clock for issued-at timestamp")?,
        };
        let receipt = CekRotationReceiptV1 {
            schema_version: CEK_ROTATION_RECEIPT_VERSION_V1,
            event_id: TaikaiEventId::new(event_name),
            stream_id: TaikaiStreamId::new(stream_name),
            kms_profile: self.kms_profile,
            new_wrap_key_label: self.new_wrap_key_label,
            previous_wrap_key_label: self.previous_wrap_key_label,
            hkdf_salt,
            effective_segment_sequence: self.effective_segment,
            issued_at_unix,
            notes: self.notes,
        };
        write_norito_file(&self.out, "cek rotation receipt", &receipt)?;
        if let Some(path) = &self.json_out {
            write_json_file(path, "cek rotation receipt", &receipt)?;
        }
        let output =
            build_receipt_output_value("receipt", &receipt, &self.out, self.json_out.as_deref())?;
        let text = render_receipt_text(
            "Taikai CEK rotation receipt generated",
            "receipt_out",
            &self.out,
            self.json_out.as_deref(),
        );
        print_with_optional_text(context, Some(text), &output)
    }
}

#[derive(clap::Args, Debug)]
pub struct RptAttestArgs {
    /// Identifier of the Taikai event.
    #[arg(long, value_name = "NAME")]
    pub event_id: String,
    /// Stream identifier within the event.
    #[arg(long, value_name = "NAME")]
    pub stream_id: String,
    /// Rendition identifier (ladder rung).
    #[arg(long, value_name = "NAME")]
    pub rendition_id: String,
    /// Path to the GAR JWS payload (used for digest computation).
    #[arg(long, value_name = "PATH")]
    pub gar: PathBuf,
    /// Path to the CEK rotation receipt referenced by the rollout.
    #[arg(long, value_name = "PATH")]
    pub cek_receipt: PathBuf,
    /// Path to the rollout evidence bundle (directory or single archive).
    #[arg(long, value_name = "PATH")]
    pub bundle: PathBuf,
    /// Output path for the Norito-encoded RPT.
    #[arg(long, value_name = "PATH")]
    pub out: PathBuf,
    /// Optional JSON summary output path.
    #[arg(long, value_name = "PATH")]
    pub json_out: Option<PathBuf>,
    /// Optional attestation validity start (Unix seconds).
    #[arg(long)]
    pub valid_from_unix: Option<u64>,
    /// Optional attestation validity end (Unix seconds).
    #[arg(long)]
    pub valid_until_unix: Option<u64>,
    /// Optional telemetry labels to embed in the attestation (repeatable).
    #[arg(long = "policy-label", value_name = "LABEL")]
    pub policy_labels: Vec<String>,
    /// Optional governance notes or ticket reference.
    #[arg(long)]
    pub notes: Option<String>,
}

impl Run for RptAttestArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let event_name = parse_name(&self.event_id, "event-id")?;
        let stream_name = parse_name(&self.stream_id, "stream-id")?;
        let rendition_name = parse_name(&self.rendition_id, "rendition-id")?;
        let gar_digest = compute_file_digest(&self.gar)
            .wrap_err_with(|| format!("failed to hash GAR `{}`", self.gar.display()))?;
        let cek_receipt_digest = compute_file_digest(&self.cek_receipt).wrap_err_with(|| {
            format!(
                "failed to hash CEK receipt `{}`",
                self.cek_receipt.display()
            )
        })?;
        let bundle_digest = compute_bundle_digest(&self.bundle)
            .wrap_err_with(|| format!("failed to hash bundle `{}`", self.bundle.display()))?;
        let policy_labels = normalize_labels(&self.policy_labels, "policy-label")?;
        let now = current_unix_timestamp()
            .wrap_err("failed to read system clock for RPT valid_from/valid_until defaults")?;
        let valid_from = self.valid_from_unix.unwrap_or(now);
        let valid_until = self.valid_until_unix.unwrap_or(valid_from + 86_400);
        if valid_until <= valid_from {
            return Err(eyre!(
                "valid-until ({valid_until}) must be greater than valid-from ({valid_from})"
            ));
        }
        let rpt = ReplicationProofTokenV1 {
            schema_version: REPLICATION_PROOF_TOKEN_VERSION_V1,
            event_id: TaikaiEventId::new(event_name),
            stream_id: TaikaiStreamId::new(stream_name),
            rendition_id: TaikaiRenditionId::new(rendition_name),
            gar_digest,
            cek_receipt_digest,
            distribution_bundle_digest: bundle_digest,
            policy_labels,
            valid_from_unix: valid_from,
            valid_until_unix: valid_until,
            notes: self.notes,
        };
        write_norito_file(&self.out, "replication proof token", &rpt)?;
        if let Some(path) = &self.json_out {
            write_json_file(path, "replication proof token", &rpt)?;
        }
        let output = build_receipt_output_value("rpt", &rpt, &self.out, self.json_out.as_deref())?;
        let text = render_receipt_text(
            "Taikai replication proof token generated",
            "rpt_out",
            &self.out,
            self.json_out.as_deref(),
        );
        print_with_optional_text(context, Some(text), &output)
    }
}

fn parse_name(value: &str, field: &str) -> Result<Name> {
    Name::from_str(value).map_err(|err| eyre!("invalid {field} `{value}`: {err}"))
}

fn parse_blob_digest(value: &str, field: &str) -> Result<BlobDigest> {
    let bytes = parse_hex_32(value, field)?;
    Ok(BlobDigest::new(bytes))
}

fn parse_hex_32(value: &str, field: &str) -> Result<[u8; 32]> {
    let trimmed = value.trim_start_matches("0x");
    let bytes =
        hex::decode(trimmed).map_err(|err| eyre!("invalid {field} hex `{value}`: {err}"))?;
    if bytes.len() != 32 {
        return Err(eyre!(
            "{field} must contain 32 bytes (64 hex chars), got {} bytes",
            bytes.len()
        ));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn write_norito_file<T: NoritoSerialize>(path: &Path, label: &str, value: &T) -> Result<()> {
    let bytes = norito::to_bytes(value)
        .wrap_err_with(|| format!("failed to encode {label} as canonical Norito framing"))?;
    fs::write(path, &bytes)
        .wrap_err_with(|| format!("failed to write {label} `{}`", path.display()))
}

fn write_json_file<T: JsonSerialize>(path: &Path, label: &str, value: &T) -> Result<()> {
    let rendered = norito::json::to_json_pretty(value)
        .map_err(|err| eyre!("failed to render {label} JSON: {err}"))?;
    fs::write(path, rendered.as_bytes())
        .wrap_err_with(|| format!("failed to write {label} `{}`", path.display()))
}

fn current_unix_timestamp() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|err| eyre!("clock drifted before UNIX_EPOCH: {err}"))
}

fn compute_file_digest(path: &Path) -> Result<[u8; 32]> {
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to stat `{}`", path.display()))?;
    if !metadata.is_file() {
        return Err(eyre!("`{}` is not a regular file", path.display()));
    }
    let mut hasher = Hasher::new();
    let relative = path
        .file_name()
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    hash_file_entry(path, &relative, &mut hasher)?;
    Ok(*hasher.finalize().as_bytes())
}

fn compute_bundle_digest(path: &Path) -> Result<[u8; 32]> {
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to stat `{}`", path.display()))?;
    let mut hasher = Hasher::new();
    if metadata.is_file() {
        let relative = path
            .file_name()
            .map_or_else(|| PathBuf::from("."), PathBuf::from);
        hash_file_entry(path, &relative, &mut hasher)?;
    } else if metadata.is_dir() {
        hash_directory_entry(path, Path::new(""), &mut hasher)?;
    } else {
        return Err(eyre!(
            "bundle `{}` must be a regular file or directory",
            path.display()
        ));
    }
    Ok(*hasher.finalize().as_bytes())
}

fn hash_file_entry(path: &Path, relative: &Path, hasher: &mut Hasher) -> Result<()> {
    update_path_marker(relative, b'F', hasher);
    let mut file =
        File::open(path).wrap_err_with(|| format!("failed to open `{}`", path.display()))?;
    let mut buffer = [0u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .wrap_err_with(|| format!("failed to read `{}`", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(())
}

fn hash_directory_entry(path: &Path, relative: &Path, hasher: &mut Hasher) -> Result<()> {
    update_path_marker(relative, b'D', hasher);
    let mut entries = Vec::new();
    for entry in fs::read_dir(path)
        .wrap_err_with(|| format!("failed to read directory `{}`", path.display()))?
    {
        let entry =
            entry.wrap_err_with(|| format!("failed to iterate directory `{}`", path.display()))?;
        entries.push(entry);
    }
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let child_path = entry.path();
        let mut child_relative = if relative.as_os_str().is_empty() {
            PathBuf::new()
        } else {
            relative.to_path_buf()
        };
        child_relative.push(entry.file_name());
        hash_path_entry(&child_path, &child_relative, hasher)?;
    }
    Ok(())
}

fn hash_path_entry(path: &Path, relative: &Path, hasher: &mut Hasher) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to stat `{}`", path.display()))?;
    if metadata.is_file() {
        hash_file_entry(path, relative, hasher)
    } else if metadata.is_dir() {
        hash_directory_entry(path, relative, hasher)
    } else {
        Err(eyre!(
            "unsupported entry type at `{}` (expected file or directory)",
            path.display()
        ))
    }
}

fn update_path_marker(relative: &Path, kind: u8, hasher: &mut Hasher) {
    let label: Cow<'_, str> = if relative.as_os_str().is_empty() {
        Cow::Borrowed(".")
    } else {
        Cow::Owned(relative.to_string_lossy().into_owned())
    };
    hasher.update(label.as_bytes());
    hasher.update(&[0xFF, kind]);
}

fn normalize_labels(values: &[String], field: &str) -> Result<Vec<String>> {
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(eyre!("{field} entries must not be empty"));
        }
        out.push(trimmed.to_string());
    }
    Ok(out)
}

fn build_track_metadata(args: &BundleArgs) -> Result<TaikaiTrackMetadata> {
    track_metadata_from_parts(
        args.track_kind,
        &args.codec,
        args.bitrate_kbps,
        args.resolution.as_deref(),
        args.audio_layout.as_deref(),
    )
}

fn track_metadata_from_parts(
    kind: CliTrackKind,
    codec_label: &str,
    bitrate_kbps: u32,
    resolution: Option<&str>,
    audio_layout: Option<&str>,
) -> Result<TaikaiTrackMetadata> {
    let codec = TaikaiCodec::from_str(codec_label)
        .map_err(|err| eyre!("invalid codec `{codec_label}`: {err}"))?;
    match kind {
        CliTrackKind::Video => {
            ensure_video_codec(&codec)?;
            let resolution_str =
                resolution.ok_or_else(|| eyre!("--resolution is required for video tracks"))?;
            let parsed = TaikaiResolution::from_str(resolution_str)
                .map_err(|err| eyre!("invalid resolution `{resolution_str}`: {err}"))?;
            Ok(TaikaiTrackMetadata::video(
                codec.clone(),
                bitrate_kbps,
                parsed,
            ))
        }
        CliTrackKind::Audio => {
            ensure_audio_codec(&codec)?;
            let layout_str =
                audio_layout.ok_or_else(|| eyre!("--audio-layout is required for audio tracks"))?;
            let layout = TaikaiAudioLayout::from_str(layout_str)
                .map_err(|err| eyre!("invalid audio layout `{layout_str}`: {err}"))?;
            Ok(TaikaiTrackMetadata::audio(
                codec.clone(),
                bitrate_kbps,
                layout,
            ))
        }
        CliTrackKind::Data => Ok(TaikaiTrackMetadata::data(codec.clone(), bitrate_kbps)),
    }
}

#[derive(Clone, Debug)]
struct TrackDescriptor {
    track_kind: CliTrackKind,
    codec: String,
    bitrate_kbps: u32,
    resolution: Option<String>,
    audio_layout: Option<String>,
}

impl TrackDescriptor {
    fn build_metadata(&self) -> Result<TaikaiTrackMetadata> {
        track_metadata_from_parts(
            self.track_kind,
            &self.codec,
            self.bitrate_kbps,
            self.resolution.as_deref(),
            self.audio_layout.as_deref(),
        )
    }
}

fn resolve_track_descriptor(
    args: &IngestWatchArgs,
    store: &LadderPresetStore,
) -> Result<TrackDescriptor> {
    let mut descriptor = if let Some(id) = args.ladder_preset.as_deref() {
        store
            .get(id)
            .cloned()
            .ok_or_else(|| eyre!("unknown ladder preset `{id}`"))?
    } else {
        TrackDescriptor {
            track_kind: args.track_kind.ok_or_else(|| {
                eyre!("--track-kind is required when --ladder-preset is not supplied")
            })?,
            codec: args
                .codec
                .clone()
                .ok_or_else(|| eyre!("--codec is required when --ladder-preset is not supplied"))?,
            bitrate_kbps: args.bitrate_kbps.ok_or_else(|| {
                eyre!("--bitrate-kbps is required when --ladder-preset is not supplied")
            })?,
            resolution: args.resolution.clone(),
            audio_layout: args.audio_layout.clone(),
        }
    };
    if let Some(kind) = args.track_kind {
        descriptor.track_kind = kind;
    }
    if let Some(codec) = &args.codec {
        descriptor.codec.clone_from(codec);
    }
    if let Some(bitrate) = args.bitrate_kbps {
        descriptor.bitrate_kbps = bitrate;
    }
    if let Some(resolution) = &args.resolution {
        descriptor.resolution = Some(resolution.clone());
    }
    if let Some(layout) = &args.audio_layout {
        descriptor.audio_layout = Some(layout.clone());
    }
    Ok(descriptor)
}

struct LadderPresetStore {
    presets: HashMap<String, TrackDescriptor>,
}

impl LadderPresetStore {
    fn load(path: Option<&Path>) -> Result<Self> {
        let contents = if let Some(path) = path {
            fs::read_to_string(path)
                .wrap_err_with(|| format!("failed to read ladder presets `{}`", path.display()))?
        } else {
            DEFAULT_LADDER_PRESETS_JSON.to_owned()
        };
        let presets = parse_ladder_presets_json(&contents)?;
        Ok(Self { presets })
    }

    fn get(&self, id: &str) -> Option<&TrackDescriptor> {
        self.presets.get(id)
    }
}

fn parse_ladder_presets_json(contents: &str) -> Result<HashMap<String, TrackDescriptor>> {
    let value: Value = json::from_str(contents)
        .map_err(|err| eyre!("failed to parse ladder preset JSON: {err}"))?;
    let entries = value
        .get("presets")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("ladder preset JSON missing `presets` array"))?;
    let mut presets = HashMap::with_capacity(entries.len());
    for entry in entries {
        let id = entry
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre!("ladder preset entry missing `id` field"))?;
        let track_kind = entry
            .get("track_kind")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre!("ladder preset `{id}` missing `track_kind`"))?;
        let codec = entry
            .get("codec")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre!("ladder preset `{id}` missing `codec`"))?;
        let bitrate = entry
            .get("bitrate_kbps")
            .and_then(Value::as_u64)
            .ok_or_else(|| eyre!("ladder preset `{id}` missing `bitrate_kbps`"))?;
        let track_kind = CliTrackKind::from_str(track_kind, true).map_err(|err| {
            eyre!("ladder preset `{id}` uses invalid track_kind `{track_kind}`: {err}")
        })?;
        let bitrate_kbps = u32::try_from(bitrate)
            .map_err(|_| eyre!("ladder preset `{id}` bitrate exceeds u32 range"))?;
        let resolution = entry
            .get("resolution")
            .and_then(Value::as_str)
            .map(str::to_string);
        let audio_layout = entry
            .get("audio_layout")
            .and_then(Value::as_str)
            .map(str::to_string);
        presets.insert(
            id.to_string(),
            TrackDescriptor {
                track_kind,
                codec: codec.to_string(),
                bitrate_kbps,
                resolution,
                audio_layout,
            },
        );
    }
    Ok(presets)
}

type DaIngestOpts = DaIngestParams;

fn build_da_ingest_opts(args: &IngestWatchArgs) -> Result<DaIngestOpts> {
    let lane_id = LaneId::new(args.da_lane);
    let blob_class = parse_blob_class(&args.da_blob_class)?;
    let blob_codec = BlobCodec::new(args.da_blob_codec.clone());
    let fec_scheme = parse_fec_scheme(&args.da_fec_scheme)?;
    let erasure_profile = ErasureProfile {
        data_shards: args.da_data_shards,
        parity_shards: args.da_parity_shards,
        row_parity_stripes: 0,
        chunk_alignment: args.da_chunk_alignment,
        fec_scheme,
    };
    let storage_class = parse_storage_class(&args.da_storage_class)?;
    let retention_policy = RetentionPolicy {
        hot_retention_secs: args.da_hot_retention_secs,
        cold_retention_secs: args.da_cold_retention_secs,
        required_replicas: args.da_required_replicas,
        storage_class,
        governance_tag: GovernanceTag::new(args.da_governance_tag.clone()),
    };
    Ok(DaIngestOpts {
        lane_id,
        epoch: args.da_epoch,
        sequence: args.sequence_start,
        blob_class,
        blob_codec,
        erasure_profile,
        retention_policy,
        chunk_size: args.da_chunk_size,
        client_blob_id: None,
    })
}

fn merge_metadata(mut base: ExtraMetadata, overlay: Option<&ExtraMetadata>) -> ExtraMetadata {
    if let Some(extra) = overlay {
        base.items.extend(extra.items.clone());
    }
    base
}

fn ensure_video_codec(codec: &TaikaiCodec) -> Result<()> {
    match codec {
        TaikaiCodec::AvcHigh
        | TaikaiCodec::HevcMain10
        | TaikaiCodec::Av1Main
        | TaikaiCodec::Custom(_) => Ok(()),
        _ => Err(eyre!(
            "codec `{codec:?}` is not valid for a video track; expected AV1/AVC/HEVC or custom video codec"
        )),
    }
}

fn ensure_audio_codec(codec: &TaikaiCodec) -> Result<()> {
    match codec {
        TaikaiCodec::AacLc | TaikaiCodec::Opus | TaikaiCodec::Custom(_) => Ok(()),
        _ => Err(eyre!(
            "codec `{codec:?}` is not valid for an audio track; expected AAC/Opus or custom audio codec"
        )),
    }
}

struct EdgeOutputLayout {
    fragments_dir: PathBuf,
    log_path: PathBuf,
    drift_summary_path: PathBuf,
}

impl EdgeOutputLayout {
    fn new(explicit: Option<PathBuf>) -> Result<Self> {
        let root = default_ingest_edge_root(explicit)?;
        let fragments_dir = root.join("fragments");
        let logs_dir = root.join("logs");
        let log_path = logs_dir.join("ingest_edge.ndjson");
        let drift_summary_path = logs_dir.join("drift_monitor.json");
        for dir in [&root, &fragments_dir, &logs_dir] {
            fs::create_dir_all(dir)
                .wrap_err_with(|| format!("failed to create `{}`", dir.display()))?;
        }
        Ok(Self {
            fragments_dir,
            log_path,
            drift_summary_path,
        })
    }

    fn fragment_path(&self, protocol: EdgeProtocol, sequence: u64) -> PathBuf {
        self.fragments_dir
            .join(format!("{}_segment_{sequence:06}.m4s", protocol.slug()))
    }
}

fn default_ingest_edge_root(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    let mut root = env::current_dir().wrap_err("failed to determine current directory")?;
    root.push("artifacts");
    root.push("taikai");
    let stamp = unix_timestamp_ms()? / 1_000;
    root.push(format!("ingest_edge_run_{stamp}"));
    Ok(root)
}

struct EdgeLogEntry {
    fragment: PathBuf,
    protocol: EdgeProtocol,
    sequence: u64,
    segment_start_pts: u64,
    wallclock_unix_ms: u64,
    drift_ms: i32,
    ingest_node_id: Option<String>,
}

struct EdgeLogWriter {
    file: File,
}

impl EdgeLogWriter {
    fn create(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).wrap_err_with(|| {
                format!(
                    "failed to create ingest edge log directory `{}`",
                    parent.display()
                )
            })?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .wrap_err_with(|| format!("failed to create ingest edge log `{}`", path.display()))?;
        Ok(Self { file })
    }

    fn write_entry(&mut self, entry: &EdgeLogEntry) -> Result<()> {
        let value = build_edge_log_value(entry);
        let rendered = norito::json::to_json(&value)
            .map_err(|err| eyre!("failed to render ingest edge log JSON: {err}"))?;
        self.file
            .write_all(rendered.as_bytes())
            .wrap_err("failed to write ingest edge log entry")?;
        self.file
            .write_all(b"\n")
            .wrap_err("failed to finalise ingest edge log entry")?;
        Ok(())
    }
}

struct DriftAccumulator {
    count: u64,
    sum: i128,
    min: i32,
    max: i32,
}

impl DriftAccumulator {
    fn new() -> Self {
        Self {
            count: 0,
            sum: 0,
            min: i32::MAX,
            max: i32::MIN,
        }
    }

    fn record(&mut self, value: i32) {
        self.count = self.count.saturating_add(1);
        self.sum = self.sum.saturating_add(i128::from(value));
        self.min = self.min.min(value);
        self.max = self.max.max(value);
    }

    #[allow(clippy::cast_precision_loss)]
    fn into_summary(
        self,
        protocol: EdgeProtocol,
        segments: u64,
        payload_bytes: usize,
        start_unix_ms: u64,
    ) -> EdgeSummary {
        let drift_avg_ms = if self.count == 0 {
            0.0
        } else {
            self.sum as f64 / self.count as f64
        };
        let drift_min_ms = if self.count == 0 { 0 } else { self.min };
        let drift_max_ms = if self.count == 0 { 0 } else { self.max };
        EdgeSummary {
            protocol,
            segments,
            payload_bytes,
            start_unix_ms,
            first_segment_pts: 0,
            segment_interval_ms: 0,
            fragment_dir: PathBuf::new(),
            log_path: PathBuf::new(),
            drift_min_ms,
            drift_max_ms,
            drift_avg_ms,
            ingest_node_id: None,
        }
    }
}

struct EdgeSummary {
    protocol: EdgeProtocol,
    segments: u64,
    payload_bytes: usize,
    start_unix_ms: u64,
    first_segment_pts: u64,
    segment_interval_ms: u64,
    fragment_dir: PathBuf,
    log_path: PathBuf,
    drift_min_ms: i32,
    drift_max_ms: i32,
    drift_avg_ms: f64,
    ingest_node_id: Option<String>,
}

impl EdgeSummary {
    fn with_output(
        mut self,
        layout: &EdgeOutputLayout,
        first_segment_pts: u64,
        segment_interval_ms: u64,
    ) -> Self {
        self.fragment_dir.clone_from(&layout.fragments_dir);
        self.log_path.clone_from(&layout.log_path);
        self.first_segment_pts = first_segment_pts;
        self.segment_interval_ms = segment_interval_ms;
        self
    }

    fn with_node(mut self, node: Option<&str>) -> Self {
        self.ingest_node_id = node.map(ToOwned::to_owned);
        self
    }

    fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).wrap_err_with(|| {
                format!(
                    "failed to create ingest edge summary directory `{}`",
                    parent.display()
                )
            })?;
        }
        let rendered = norito::json::to_json_pretty(&build_edge_summary_value(self))
            .map_err(|err| eyre!("failed to render ingest edge summary JSON: {err}"))?;
        fs::write(path, rendered.as_bytes()).wrap_err_with(|| {
            format!(
                "failed to write ingest edge summary file `{}`",
                path.display()
            )
        })?;
        Ok(())
    }
}

fn build_edge_rng(seed: Option<u64>) -> StdRng {
    seed.map_or_else(StdRng::from_os_rng, StdRng::seed_from_u64)
}

fn jittered_drift<R: Rng + ?Sized>(base: i32, jitter_ms: u32, rng: &mut R) -> i32 {
    if jitter_ms == 0 {
        return base;
    }
    let jitter_clamped = i32::try_from(jitter_ms.min(i32::MAX as u32)).unwrap_or(i32::MAX);
    let offset = rng.random_range(-jitter_clamped..=jitter_clamped);
    base.saturating_add(offset)
}

fn unix_timestamp_ms() -> Result<u64> {
    let now = SystemTime::now();
    let duration = now
        .duration_since(UNIX_EPOCH)
        .map_err(|err| eyre!("clock drifted before UNIX_EPOCH: {err}"))?;
    u64::try_from(duration.as_millis()).map_err(|_| eyre!("wallclock timestamp exceeds u64::MAX"))
}

fn build_edge_log_value(entry: &EdgeLogEntry) -> Value {
    let mut map = Map::new();
    map.insert(
        "fragment".into(),
        Value::from(path_to_string(&entry.fragment)),
    );
    map.insert(
        "protocol".into(),
        Value::from(entry.protocol.as_str().to_string()),
    );
    map.insert("sequence".into(), Value::from(entry.sequence));
    map.insert(
        "segment_start_pts".into(),
        Value::from(entry.segment_start_pts),
    );
    map.insert(
        "wallclock_unix_ms".into(),
        Value::from(entry.wallclock_unix_ms),
    );
    map.insert("drift_ms".into(), Value::from(entry.drift_ms));
    map.insert(
        "ingest_node_id".into(),
        optional_string_value(entry.ingest_node_id.as_deref()),
    );
    Value::Object(map)
}

fn build_edge_summary_value(summary: &EdgeSummary) -> Value {
    let mut map = Map::new();
    map.insert(
        "protocol".into(),
        Value::from(summary.protocol.as_str().to_string()),
    );
    map.insert("segments".into(), Value::from(summary.segments));
    map.insert(
        "payload_bytes".into(),
        Value::from(u64::try_from(summary.payload_bytes).unwrap_or(u64::MAX)),
    );
    map.insert("start_unix_ms".into(), Value::from(summary.start_unix_ms));
    map.insert(
        "first_segment_pts".into(),
        Value::from(summary.first_segment_pts),
    );
    map.insert(
        "segment_interval_ms".into(),
        Value::from(summary.segment_interval_ms),
    );
    map.insert("drift_min_ms".into(), Value::from(summary.drift_min_ms));
    map.insert("drift_max_ms".into(), Value::from(summary.drift_max_ms));
    map.insert("drift_avg_ms".into(), Value::from(summary.drift_avg_ms));
    map.insert(
        "fragment_dir".into(),
        Value::from(path_to_string(&summary.fragment_dir)),
    );
    map.insert(
        "ingest_edge_log".into(),
        Value::from(path_to_string(&summary.log_path)),
    );
    map.insert(
        "ingest_node_id".into(),
        optional_string_value(summary.ingest_node_id.as_deref()),
    );
    Value::Object(map)
}

struct IngestOutputLayout {
    root: PathBuf,
    car_dir: PathBuf,
    envelope_dir: PathBuf,
    indexes_dir: PathBuf,
    metadata_dir: PathBuf,
    da_request_dir: PathBuf,
    da_receipt_dir: PathBuf,
}

impl IngestOutputLayout {
    fn new(explicit: Option<PathBuf>) -> Result<Self> {
        let root = default_ingest_root(explicit)?;
        let car_dir = root.join("car");
        let envelope_dir = root.join("envelopes");
        let indexes_dir = root.join("indexes");
        let metadata_dir = root.join("ingest_metadata");
        let da_request_dir = root.join("da_requests");
        let da_receipt_dir = root.join("da_receipts");
        for dir in [
            &root,
            &car_dir,
            &envelope_dir,
            &indexes_dir,
            &metadata_dir,
            &da_request_dir,
            &da_receipt_dir,
        ] {
            fs::create_dir_all(dir)
                .wrap_err_with(|| format!("failed to create `{}`", dir.display()))?;
        }
        Ok(Self {
            root,
            car_dir,
            envelope_dir,
            indexes_dir,
            metadata_dir,
            da_request_dir,
            da_receipt_dir,
        })
    }

    fn allocate_paths(&self, source: &Path, sequence: u64) -> IngestOutputPaths {
        let stem = source
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .map_or_else(|| "segment".to_string(), sanitize_slug);
        let slug = format!("{stem}_{sequence:020}");
        IngestOutputPaths {
            slug: slug.clone(),
            car: self.car_dir.join(format!("{slug}.car")),
            envelope: self.envelope_dir.join(format!("{slug}.to")),
            indexes: self.indexes_dir.join(format!("{slug}.json")),
            metadata: self.metadata_dir.join(format!("{slug}.da.json")),
        }
    }

    fn allocate_da_paths(&self, slug: &str) -> DaRequestPaths {
        DaRequestPaths {
            request: self.da_request_dir.join(format!("{slug}.request.norito")),
            request_json: self.da_request_dir.join(format!("{slug}.request.json")),
            receipt: self.da_receipt_dir.join(format!("{slug}.receipt.norito")),
            receipt_json: self.da_receipt_dir.join(format!("{slug}.receipt.json")),
        }
    }
}

struct IngestOutputPaths {
    slug: String,
    car: PathBuf,
    envelope: PathBuf,
    indexes: PathBuf,
    metadata: PathBuf,
}

struct DaRequestPaths {
    request: PathBuf,
    request_json: PathBuf,
    receipt: PathBuf,
    receipt_json: PathBuf,
}

fn default_ingest_root(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    let mut root = env::current_dir().wrap_err("failed to determine current directory")?;
    root.push("artifacts");
    root.push("taikai");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| eyre!("clock drifted before UNIX_EPOCH: {err}"))?
        .as_secs();
    root.push(format!("ingest_run_{stamp}"));
    Ok(root)
}

fn sanitize_slug(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

struct DriftAnchors {
    pts_anchor: u64,
    wallclock_anchor_ms: u128,
}

impl DriftAnchors {
    fn new(pts_anchor: u64) -> Result<Self> {
        let wallclock_anchor_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| eyre!("clock drifted before UNIX_EPOCH: {err}"))?
            .as_millis();
        Ok(Self {
            pts_anchor,
            wallclock_anchor_ms,
        })
    }

    fn measure(&self, segment_pts: u64, actual_ms: u128) -> i128 {
        let delta_pts = segment_pts.saturating_sub(self.pts_anchor);
        let expected_ms = self.wallclock_anchor_ms + u128::from(delta_pts) / 1_000;
        let actual_i = i128::try_from(actual_ms).unwrap_or(i128::MAX);
        let expected_i = i128::try_from(expected_ms).unwrap_or(i128::MAX);
        actual_i - expected_i
    }
}

struct IngestSummary {
    bundle: BundleSummary,
    drift_ms: i128,
    live_edge_drift_ms: i32,
    da_request_path: PathBuf,
    da_receipt_path: Option<PathBuf>,
    slug: String,
    manifest_hash: BlobDigest,
    storage_ticket: StorageTicketId,
    sequence: u64,
    segment_start_pts: u64,
    segment_duration: u32,
    wallclock_unix_ms: u64,
    ingest_latency_ms: Option<u32>,
    ingest_node_id: Option<String>,
}

struct IngestSummaryLog {
    file: File,
}

impl IngestSummaryLog {
    fn create(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).wrap_err_with(|| {
                format!("failed to create summary directory `{}`", parent.display())
            })?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .wrap_err_with(|| {
                format!("failed to create ingest summary file `{}`", path.display())
            })?;
        Ok(Self { file })
    }

    fn write_entry(&mut self, source: &Path, summary: &IngestSummary) -> Result<()> {
        let value = build_ingest_summary_value(source, summary);
        let rendered = norito::json::to_json(&value)
            .map_err(|err| eyre!("failed to render ingest summary JSON: {err}"))?;
        self.file
            .write_all(rendered.as_bytes())
            .wrap_err("failed to write ingest summary line")?;
        self.file
            .write_all(b"\n")
            .wrap_err("failed to finalise ingest summary line")?;
        Ok(())
    }
}

fn build_ingest_summary_value(source: &Path, summary: &IngestSummary) -> Value {
    let mut map = Map::new();
    map.insert("payload".into(), Value::from(path_to_string(source)));
    map.insert("slug".into(), Value::from(summary.slug.clone()));
    map.insert(
        "car".into(),
        Value::from(path_to_string(&summary.bundle.car_out)),
    );
    map.insert(
        "envelope".into(),
        Value::from(path_to_string(&summary.bundle.envelope_out)),
    );
    map.insert(
        "indexes".into(),
        optional_path_value(summary.bundle.indexes_out.as_deref()),
    );
    map.insert(
        "ingest_metadata".into(),
        optional_path_value(summary.bundle.ingest_metadata_out.as_deref()),
    );
    map.insert(
        "da_request".into(),
        Value::from(path_to_string(&summary.da_request_path)),
    );
    map.insert(
        "da_receipt".into(),
        optional_path_value(summary.da_receipt_path.as_deref()),
    );
    map.insert(
        "manifest_hash".into(),
        Value::from(hex::encode(summary.manifest_hash.as_bytes())),
    );
    map.insert(
        "storage_ticket".into(),
        Value::from(hex::encode(summary.storage_ticket.as_bytes())),
    );
    map.insert(
        "chunk_root".into(),
        Value::from(hex::encode(summary.bundle.chunk_root.as_bytes())),
    );
    map.insert(
        "chunk_count".into(),
        Value::from(summary.bundle.chunk_count),
    );
    map.insert(
        "car_cid_multibase".into(),
        Value::from(summary.bundle.car_pointer.cid_multibase.clone()),
    );
    map.insert(
        "car_digest".into(),
        Value::from(hex::encode(
            summary.bundle.car_pointer.car_digest.as_bytes(),
        )),
    );
    map.insert(
        "car_size_bytes".into(),
        Value::from(summary.bundle.car_pointer.car_size_bytes),
    );
    map.insert("sequence".into(), Value::from(summary.sequence));
    map.insert(
        "segment_start_pts".into(),
        Value::from(summary.segment_start_pts),
    );
    map.insert(
        "segment_duration".into(),
        Value::from(summary.segment_duration),
    );
    map.insert(
        "wallclock_unix_ms".into(),
        Value::from(summary.wallclock_unix_ms),
    );
    map.insert(
        "ingest_latency_ms".into(),
        optional_u32_value(summary.ingest_latency_ms),
    );
    map.insert(
        "live_edge_drift_ms".into(),
        Value::from(summary.live_edge_drift_ms),
    );
    let drift_clamped = summary
        .drift_ms
        .clamp(i128::from(i64::MIN), i128::from(i64::MAX));
    let drift_i64 =
        i64::try_from(drift_clamped).expect("clamp must keep drift_ms within i64 bounds");
    map.insert("drift_ms".into(), Value::from(drift_i64));
    map.insert(
        "ingest_node_id".into(),
        optional_string_value(summary.ingest_node_id.as_deref()),
    );
    Value::Object(map)
}

fn build_bundle_summary_value(summary: &BundleSummary) -> Value {
    let mut map = Map::new();
    map.insert(
        "car_cid_multibase".into(),
        Value::from(summary.car_pointer.cid_multibase.clone()),
    );
    map.insert(
        "car_digest".into(),
        Value::from(hex::encode(summary.car_pointer.car_digest.as_bytes())),
    );
    map.insert(
        "car_size_bytes".into(),
        Value::from(summary.car_pointer.car_size_bytes),
    );
    map.insert(
        "chunk_root".into(),
        Value::from(hex::encode(summary.chunk_root.as_bytes())),
    );
    map.insert("chunk_count".into(), Value::from(summary.chunk_count));
    map.insert(
        "car_out".into(),
        Value::from(path_to_string(&summary.car_out)),
    );
    map.insert(
        "envelope_out".into(),
        Value::from(path_to_string(&summary.envelope_out)),
    );
    map.insert(
        "indexes_out".into(),
        optional_path_value(summary.indexes_out.as_deref()),
    );
    map.insert(
        "ingest_metadata_out".into(),
        optional_path_value(summary.ingest_metadata_out.as_deref()),
    );
    Value::Object(map)
}

fn build_edge_output_value(summary: &EdgeSummary, drift_summary_path: &Path) -> Value {
    let mut map = match build_edge_summary_value(summary) {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    map.insert(
        "drift_summary".into(),
        Value::from(path_to_string(drift_summary_path)),
    );
    Value::Object(map)
}

fn build_receipt_output_value<T: JsonSerialize>(
    label: &str,
    payload: &T,
    out_path: &Path,
    json_out: Option<&Path>,
) -> Result<Value> {
    let mut map = Map::new();
    let payload_value =
        json::to_value(payload).map_err(|err| eyre!("failed to render {label} JSON: {err}"))?;
    map.insert(label.into(), payload_value);
    map.insert(
        format!("{label}_out"),
        Value::from(path_to_string(out_path)),
    );
    map.insert("json_out".into(), optional_path_value(json_out));
    Ok(Value::Object(map))
}

fn render_bundle_summary_text(summary: &BundleSummary) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Taikai segment bundle generated");
    let _ = writeln!(
        out,
        "car_cid (multibase): {}",
        summary.car_pointer.cid_multibase
    );
    let _ = writeln!(
        out,
        "car_digest (blake3-256 hex): {}",
        hex::encode(summary.car_pointer.car_digest.as_bytes())
    );
    let _ = writeln!(
        out,
        "car_size_bytes: {}",
        summary.car_pointer.car_size_bytes
    );
    let _ = writeln!(
        out,
        "chunk_root (blake3-256 hex): {}",
        hex::encode(summary.chunk_root.as_bytes())
    );
    let _ = writeln!(out, "chunk_count: {}", summary.chunk_count);
    let _ = writeln!(out, "car_out: {}", summary.car_out.display());
    let _ = writeln!(out, "envelope_out: {}", summary.envelope_out.display());
    if let Some(path) = summary.indexes_out.as_ref() {
        let _ = writeln!(out, "indexes_out: {}", path.display());
    }
    if let Some(path) = summary.ingest_metadata_out.as_ref() {
        let _ = writeln!(out, "ingest_metadata_out: {}", path.display());
    }
    out
}

fn render_edge_summary_text(
    summary: &EdgeSummary,
    drift_summary_path: &Path,
    base_drift: i32,
    jitter_ms: u32,
) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Taikai ingest edge emitted {segments} fragments to `{dir}` (protocol={proto}, drift={base}ms±{jitter}ms)",
        segments = summary.segments,
        dir = summary.fragment_dir.display(),
        proto = summary.protocol.as_str(),
        base = base_drift,
        jitter = jitter_ms,
    );
    let _ = writeln!(
        out,
        "drift log: {} (summary: {})",
        summary.log_path.display(),
        drift_summary_path.display()
    );
    out
}

fn render_receipt_text(
    header: &str,
    out_label: &str,
    out_path: &Path,
    json_out: Option<&Path>,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{header}");
    let _ = writeln!(out, "{out_label}: {}", out_path.display());
    if let Some(path) = json_out {
        let _ = writeln!(out, "json_out: {}", path.display());
    }
    out
}

fn optional_path_value(path: Option<&Path>) -> Value {
    path.map_or(Value::Null, |p| Value::from(path_to_string(p)))
}

fn optional_u32_value(value: Option<u32>) -> Value {
    value.map_or(Value::Null, Value::from)
}

fn optional_string_value(value: Option<&str>) -> Value {
    value.map_or(Value::Null, |val| Value::from(val.to_string()))
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod summary_tests {
    use super::*;
    use iroha::data_model::{
        da::types::{BlobDigest, StorageTicketId},
        name::Name,
        taikai::{
            SegmentTimestamp, TaikaiCarPointer, TaikaiCidIndexKey, TaikaiEnvelopeIndexes,
            TaikaiEventId, TaikaiRenditionId, TaikaiStreamId, TaikaiTimeIndexKey,
        },
    };
    use std::{path::Path, str::FromStr};

    fn sample_bundle_summary() -> BundleSummary {
        let time_key = TaikaiTimeIndexKey {
            event_id: TaikaiEventId::new(Name::from_str("event").unwrap()),
            stream_id: TaikaiStreamId::new(Name::from_str("stream").unwrap()),
            rendition_id: TaikaiRenditionId::new(Name::from_str("rend").unwrap()),
            segment_start_pts: SegmentTimestamp::new(7),
        };
        let cid_key = TaikaiCidIndexKey {
            event_id: TaikaiEventId::new(Name::from_str("event").unwrap()),
            stream_id: TaikaiStreamId::new(Name::from_str("stream").unwrap()),
            rendition_id: TaikaiRenditionId::new(Name::from_str("rend").unwrap()),
            cid_multibase: "bafycarcid".to_string(),
        };
        BundleSummary {
            car_pointer: TaikaiCarPointer::new("bafycarcid", BlobDigest::new([0x11; 32]), 1_024),
            chunk_root: BlobDigest::new([0x22; 32]),
            chunk_count: 4,
            car_out: PathBuf::from("/tmp/out.car"),
            envelope_out: PathBuf::from("/tmp/out.to"),
            indexes: TaikaiEnvelopeIndexes { time_key, cid_key },
            indexes_out: Some(PathBuf::from("/tmp/out.json")),
            ingest_metadata_out: Some(PathBuf::from("/tmp/out.da.json")),
            ingest_metadata: Map::new(),
        }
    }

    #[test]
    fn build_ingest_summary_value_tracks_core_fields() {
        let bundle = sample_bundle_summary();
        let summary = IngestSummary {
            bundle,
            drift_ms: 33,
            live_edge_drift_ms: -7,
            da_request_path: PathBuf::from("/tmp/out.request.norito"),
            da_receipt_path: Some(PathBuf::from("/tmp/out.receipt.norito")),
            slug: "segment_00000000000000000000".to_string(),
            manifest_hash: BlobDigest::new([0x33; 32]),
            storage_ticket: StorageTicketId::new([0x44; 32]),
            sequence: 9,
            segment_start_pts: 777,
            segment_duration: 2_000,
            wallclock_unix_ms: 1_234_567,
            ingest_latency_ms: Some(99),
            ingest_node_id: Some("node-a".to_string()),
        };
        let value = build_ingest_summary_value(Path::new("/tmp/source.m4s"), &summary);
        let map = value
            .as_object()
            .expect("summary value should be a JSON object");
        assert_eq!(
            map.get("payload").and_then(Value::as_str),
            Some("/tmp/source.m4s")
        );
        assert_eq!(
            map.get("slug").and_then(Value::as_str),
            Some("segment_00000000000000000000")
        );
        assert_eq!(map.get("car").and_then(Value::as_str), Some("/tmp/out.car"));
        assert_eq!(
            map.get("da_receipt").and_then(Value::as_str),
            Some("/tmp/out.receipt.norito")
        );
        let expected_manifest = hex::encode(summary.manifest_hash.as_bytes());
        assert_eq!(
            map.get("manifest_hash").and_then(Value::as_str),
            Some(expected_manifest.as_str())
        );
        let expected_ticket = hex::encode(summary.storage_ticket.as_bytes());
        assert_eq!(
            map.get("storage_ticket").and_then(Value::as_str),
            Some(expected_ticket.as_str())
        );
        assert_eq!(map.get("chunk_count").and_then(Value::as_u64), Some(4));
        assert_eq!(map.get("sequence").and_then(Value::as_u64), Some(9));
        assert_eq!(
            map.get("segment_start_pts").and_then(Value::as_u64),
            Some(777)
        );
        assert_eq!(
            map.get("ingest_latency_ms").and_then(Value::as_u64),
            Some(99)
        );
        assert_eq!(
            map.get("live_edge_drift_ms").and_then(Value::as_i64),
            Some(-7)
        );
        assert_eq!(map.get("drift_ms").and_then(Value::as_i64), Some(33));
        assert_eq!(
            map.get("ingest_node_id").and_then(Value::as_str),
            Some("node-a")
        );
    }

    #[test]
    fn build_bundle_summary_value_tracks_paths() {
        let summary = sample_bundle_summary();
        let value = build_bundle_summary_value(&summary);
        let map = value
            .as_object()
            .expect("bundle summary should be a JSON object");
        assert_eq!(
            map.get("car_out").and_then(Value::as_str),
            Some("/tmp/out.car")
        );
        assert_eq!(
            map.get("envelope_out").and_then(Value::as_str),
            Some("/tmp/out.to")
        );
        assert_eq!(
            map.get("indexes_out").and_then(Value::as_str),
            Some("/tmp/out.json")
        );
        assert_eq!(
            map.get("ingest_metadata_out").and_then(Value::as_str),
            Some("/tmp/out.da.json")
        );
    }

    #[test]
    fn render_bundle_summary_text_mentions_outputs() {
        let summary = sample_bundle_summary();
        let text = render_bundle_summary_text(&summary);
        assert!(text.contains("car_out: /tmp/out.car"));
        assert!(text.contains("envelope_out: /tmp/out.to"));
        assert!(text.contains("indexes_out: /tmp/out.json"));
    }

    #[test]
    fn render_edge_summary_text_mentions_paths() {
        let summary = EdgeSummary {
            protocol: EdgeProtocol::Srt,
            segments: 2,
            payload_bytes: 100,
            start_unix_ms: 0,
            first_segment_pts: 0,
            segment_interval_ms: 1000,
            fragment_dir: PathBuf::from("/tmp/fragments"),
            log_path: PathBuf::from("/tmp/drift.log"),
            drift_min_ms: -1,
            drift_max_ms: 1,
            drift_avg_ms: 0.0,
            ingest_node_id: None,
        };
        let text = render_edge_summary_text(&summary, Path::new("/tmp/drift.json"), 0, 5);
        assert!(text.contains("fragments"));
        assert!(text.contains("drift log"));
    }

    #[test]
    fn build_receipt_output_value_tracks_paths() {
        let payload = norito::json!({ "ok": true });
        let value = build_receipt_output_value("receipt", &payload, Path::new("/tmp/out.to"), None)
            .expect("receipt output");
        let map = value.as_object().expect("receipt output object");
        assert_eq!(
            map.get("receipt_out").and_then(Value::as_str),
            Some("/tmp/out.to")
        );
    }

    #[test]
    fn render_receipt_text_includes_json_path() {
        let text = render_receipt_text(
            "Receipt saved",
            "receipt_out",
            Path::new("/tmp/out.to"),
            Some(Path::new("/tmp/out.json")),
        );
        assert!(text.contains("receipt_out: /tmp/out.to"));
        assert!(text.contains("json_out: /tmp/out.json"));
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn process_ingest_file(
    payload_path: &Path,
    layout: &IngestOutputLayout,
    da_opts: &DaIngestOpts,
    publisher: Option<&DaPublisher>,
    config: &Config,
    event_id: &TaikaiEventId,
    stream_id: &TaikaiStreamId,
    rendition_id: &TaikaiRenditionId,
    track: &TaikaiTrackMetadata,
    metadata: Option<&ExtraMetadata>,
    sequence: u64,
    segment_pts: u64,
    segment_duration: u32,
    ingest_latency_override: Option<u32>,
    ingest_node_id: Option<&str>,
    anchors: &DriftAnchors,
) -> Result<IngestSummary> {
    let outputs = layout.allocate_paths(payload_path, sequence);
    let slug = outputs.slug.clone();
    let payload_bytes = fs::read(payload_path)
        .wrap_err_with(|| format!("failed to read payload `{}`", payload_path.display()))?;
    if payload_bytes.is_empty() {
        return Err(eyre!(
            "payload `{}` is empty; segments must contain data",
            payload_path.display()
        ));
    }
    let manifest_hash = BlobDigest::from_hash(blake3::hash(&payload_bytes));
    let storage_ticket =
        derive_storage_ticket(&manifest_hash, event_id, stream_id, rendition_id, sequence);
    let now = SystemTime::now();
    let wallclock_duration = now
        .duration_since(UNIX_EPOCH)
        .map_err(|err| eyre!("clock drifted before UNIX_EPOCH: {err}"))?;
    let wallclock_ms = wallclock_duration.as_millis();
    let drift_ms = anchors.measure(segment_pts, wallclock_ms);
    let live_edge_drift_i32 =
        i32::try_from(drift_ms.clamp(i128::from(i32::MIN), i128::from(i32::MAX)))
            .expect("clamp enforces i32 bounds");
    let wallclock_unix_ms =
        u64::try_from(wallclock_ms).map_err(|_| eyre!("wallclock timestamp exceeds u64::MAX"))?;
    let ingest_latency_ms = compute_ingest_latency_ms(ingest_latency_override, payload_path, now);
    let ingest_node_owned = ingest_node_id.map(ToOwned::to_owned);
    let summary = bundle_segment(&BundleRequest {
        payload_path,
        payload_bytes: Some(payload_bytes.as_slice()),
        car_out: &outputs.car,
        envelope_out: &outputs.envelope,
        indexes_out: Some(&outputs.indexes),
        ingest_metadata_out: Some(&outputs.metadata),
        manifest_hash,
        storage_ticket,
        event_id: event_id.clone(),
        stream_id: stream_id.clone(),
        rendition_id: rendition_id.clone(),
        track: track.clone(),
        segment_sequence: sequence,
        segment_start_pts: segment_pts,
        segment_duration,
        wallclock_unix_ms,
        ingest_latency_ms,
        live_edge_drift_ms: Some(live_edge_drift_i32),
        ingest_node_id: ingest_node_owned.clone(),
        extra_metadata: metadata.cloned(),
    })?;

    let ingest_metadata = metadata_map_to_extra(&summary.ingest_metadata)?;
    let combined_metadata = merge_metadata(ingest_metadata, metadata);
    let mut request_params = da_opts.clone();
    request_params.sequence = sequence;
    request_params = request_params.with_client_blob_id(manifest_hash);
    let request = da::build_da_request(
        payload_bytes,
        &request_params,
        combined_metadata,
        &config.key_pair,
        None,
    );
    let request_bytes = norito::to_bytes(&request).wrap_err("failed to encode DA request")?;
    let request_json = norito::json::to_json_pretty(&request)
        .map_err(|err| eyre!("failed to render DA request JSON: {err}"))?;
    let da_paths = layout.allocate_da_paths(&outputs.slug);
    fs::write(&da_paths.request, &request_bytes).wrap_err_with(|| {
        format!(
            "failed to write DA request `{}`",
            da_paths.request.display()
        )
    })?;
    fs::write(&da_paths.request_json, request_json.as_bytes()).wrap_err_with(|| {
        format!(
            "failed to write DA request JSON `{}`",
            da_paths.request_json.display()
        )
    })?;

    let receipt_path = if let Some(publisher) = publisher {
        let receipt = publisher.publish(&request_bytes)?;
        fs::write(&da_paths.receipt, &receipt.bytes).wrap_err_with(|| {
            format!(
                "failed to write DA receipt `{}`",
                da_paths.receipt.display()
            )
        })?;
        fs::write(&da_paths.receipt_json, receipt.json.as_bytes()).wrap_err_with(|| {
            format!(
                "failed to write DA receipt JSON `{}`",
                da_paths.receipt_json.display()
            )
        })?;
        Some(da_paths.receipt.clone())
    } else {
        None
    };

    Ok(IngestSummary {
        bundle: summary,
        drift_ms,
        live_edge_drift_ms: live_edge_drift_i32,
        da_request_path: da_paths.request,
        da_receipt_path: receipt_path,
        slug,
        manifest_hash,
        storage_ticket,
        sequence,
        segment_start_pts: segment_pts,
        segment_duration,
        wallclock_unix_ms,
        ingest_latency_ms,
        ingest_node_id: ingest_node_owned,
    })
}

fn derive_storage_ticket(
    digest: &BlobDigest,
    event_id: &TaikaiEventId,
    stream_id: &TaikaiStreamId,
    rendition_id: &TaikaiRenditionId,
    sequence: u64,
) -> StorageTicketId {
    let mut hasher = Hasher::new();
    hasher.update(digest.as_bytes());
    hasher.update(event_id.as_name().as_ref().as_bytes());
    hasher.update(stream_id.as_name().as_ref().as_bytes());
    hasher.update(rendition_id.as_name().as_ref().as_bytes());
    hasher.update(&sequence.to_le_bytes());
    StorageTicketId::from_hash(hasher.finalize())
}

fn emit_ingest_summary<C: RunContext>(
    context: &mut C,
    source: &Path,
    summary: &IngestSummary,
    processed: u64,
    warn_threshold_ms: u64,
) -> Result<()> {
    let receipt_blurb = summary
        .da_receipt_path
        .as_ref()
        .map(|path| format!(" receipt=`{}`", path.display()))
        .unwrap_or_default();
    match context.output_format() {
        CliOutputFormat::Json => {
            let summary_value = build_ingest_summary_value(source, summary);
            context.print_data(&summary_value)?;
        }
        CliOutputFormat::Text => {
            context.println(format_args!(
                "[{processed}] bundled `{}` -> car=`{}` envelope=`{}` chunks={} drift={:+}ms da_request=`{}`{receipt_blurb}",
                source.display(),
                summary.bundle.car_out.display(),
                summary.bundle.envelope_out.display(),
                summary.bundle.chunk_count,
                summary.drift_ms,
                summary.da_request_path.display(),
            ))?;
        }
    }
    let drift_abs = summary.drift_ms.unsigned_abs();
    if drift_abs > u128::from(warn_threshold_ms) {
        context.println(format_args!(
            "warning: ingest drift {:+}ms exceeds threshold {}ms",
            summary.drift_ms, warn_threshold_ms
        ))?;
    }
    Ok(())
}

fn compute_ingest_latency_ms(
    override_value: Option<u32>,
    path: &Path,
    now: SystemTime,
) -> Option<u32> {
    if override_value.is_some() {
        return override_value;
    }
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let duration = now.duration_since(modified).ok()?;
    let millis = duration.as_millis();
    Some(u32::try_from(millis.min(u128::from(u32::MAX))).unwrap())
}

struct ExtensionMatcher {
    extensions: Vec<String>,
}

impl ExtensionMatcher {
    fn new(values: &[String]) -> Result<Self> {
        let mut extensions = Vec::new();
        if values.is_empty() {
            extensions.push("m4s".to_string());
        } else {
            for value in values {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err(eyre!("match-ext entries must not be empty"));
                }
                extensions.push(trimmed.to_ascii_lowercase());
            }
        }
        Ok(Self { extensions })
    }

    fn matches(&self, path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase);
        ext.is_some_and(|ext| {
            self.extensions
                .iter()
                .any(|candidate| candidate == "*" || candidate == &ext)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::data_model::taikai::TaikaiTrackKind;
    use std::{fs, path::Path};

    #[test]
    fn hex_parser_accepts_32_byte_values() {
        let hex = "aa".repeat(32);
        let digest = parse_blob_digest(&hex, "test").expect("digest");
        assert_eq!(digest.as_bytes(), &[0xaa; 32]);
        let err = parse_blob_digest("ff", "test").unwrap_err();
        assert!(err.to_string().contains("64 hex chars"));
    }

    #[test]
    fn ensure_codec_validation_matches_track_kind() {
        assert!(ensure_video_codec(&TaikaiCodec::Av1Main).is_ok());
        assert!(ensure_video_codec(&TaikaiCodec::AacLc).is_err());
        assert!(ensure_audio_codec(&TaikaiCodec::Opus).is_ok());
        assert!(ensure_audio_codec(&TaikaiCodec::AvcHigh).is_err());
    }

    #[test]
    fn build_track_metadata_enforces_required_fields() {
        let mut base = sample_args();
        let built = build_track_metadata(&base).expect("video track");
        assert_eq!(built.kind, TaikaiTrackKind::Video);
        assert_eq!(built.resolution.unwrap().width, 1920, "resolution width");

        base.resolution = None;
        assert!(build_track_metadata(&base).is_err());

        let mut audio = sample_args();
        audio.track_kind = CliTrackKind::Audio;
        audio.codec = "aac-lc".into();
        audio.resolution = None;
        audio.audio_layout = Some("stereo".into());
        let built_audio = build_track_metadata(&audio).expect("audio track");
        assert_eq!(built_audio.kind, TaikaiTrackKind::Audio);
        assert_eq!(built_audio.audio_layout.unwrap(), TaikaiAudioLayout::Stereo);

        audio.audio_layout = None;
        assert!(build_track_metadata(&audio).is_err());

        let mut data = sample_args();
        data.track_kind = CliTrackKind::Data;
        data.codec = "custom:id3".into();
        data.resolution = None;
        data.audio_layout = None;
        let built_data = build_track_metadata(&data).expect("data track");
        assert_eq!(built_data.kind, TaikaiTrackKind::Data);
        assert!(built_data.resolution.is_none());
    }

    #[test]
    fn ladder_preset_store_exposes_defaults() {
        let store = LadderPresetStore::load(None).expect("store");
        let preset = store
            .get("fhd-1080p-av1")
            .expect("preset missing from catalog");
        assert_eq!(preset.track_kind, CliTrackKind::Video);
        assert_eq!(preset.bitrate_kbps, 8_000);
    }

    #[test]
    fn extension_matcher_filters_extensions() {
        let matcher =
            ExtensionMatcher::new(&["m4s".into(), "cmfv".into()]).expect("matcher config");
        assert!(matcher.matches(Path::new("segment0001.m4s")));
        assert!(matcher.matches(Path::new("demo.cmfv")));
        assert!(!matcher.matches(Path::new("notes.txt")));
    }

    #[test]
    fn sanitize_slug_rewrites_problematic_chars() {
        assert_eq!(sanitize_slug("foo bar#baz"), "foo_bar_baz");
        assert_eq!(sanitize_slug("中継_stream"), "___stream");
    }

    #[test]
    fn ingest_edge_log_writer_emits_ndjson() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let log_path = tmp.path().join("edge.ndjson");
        let fragment = tmp.path().join("segment.m4s");
        let mut writer = EdgeLogWriter::create(&log_path).expect("writer");
        let entry = EdgeLogEntry {
            fragment: fragment.clone(),
            protocol: EdgeProtocol::Rtmp,
            sequence: 3,
            segment_start_pts: 2_000_000,
            wallclock_unix_ms: 1_700_000_123_000,
            drift_ms: -50,
            ingest_node_id: Some("edge-a".to_string()),
        };
        writer.write_entry(&entry).expect("write log");

        let contents = fs::read_to_string(&log_path).expect("log contents");
        let value: Value = json::from_str(contents.trim()).expect("parse log");
        let map = value.as_object().expect("log entry object");
        assert_eq!(
            map.get("fragment").and_then(Value::as_str),
            Some(path_to_string(&fragment).as_str())
        );
        assert_eq!(map.get("protocol").and_then(Value::as_str), Some("rtmp"));
        assert_eq!(map.get("sequence").and_then(Value::as_u64), Some(3));
        assert_eq!(
            map.get("wallclock_unix_ms").and_then(Value::as_u64),
            Some(1_700_000_123_000)
        );
        assert_eq!(map.get("drift_ms").and_then(Value::as_i64), Some(-50));
        assert_eq!(
            map.get("ingest_node_id").and_then(Value::as_str),
            Some("edge-a")
        );
    }

    #[test]
    fn ingest_edge_summary_writes_report() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let layout =
            EdgeOutputLayout::new(Some(tmp.path().join("edge_out"))).expect("layout created");
        let mut acc = DriftAccumulator::new();
        acc.record(-10);
        acc.record(5);
        let summary = acc
            .into_summary(EdgeProtocol::Srt, 2, 11, 1_700_000_000_000)
            .with_output(&layout, 0, 2_000)
            .with_node(Some("node-1"));
        summary
            .write(&layout.drift_summary_path)
            .expect("write summary");
        let contents = fs::read_to_string(&layout.drift_summary_path).expect("summary contents");
        let value: Value = json::from_str(&contents).expect("parse summary");
        let map = value.as_object().expect("summary object");
        assert_eq!(map.get("segments").and_then(Value::as_u64), Some(2));
        assert_eq!(map.get("drift_min_ms").and_then(Value::as_i64), Some(-10));
        assert_eq!(map.get("drift_max_ms").and_then(Value::as_i64), Some(5));
        assert_eq!(
            map.get("ingest_node_id").and_then(Value::as_str),
            Some("node-1")
        );
        assert_eq!(
            map.get("fragment_dir").and_then(Value::as_str),
            Some(path_to_string(&layout.fragments_dir).as_str())
        );
    }

    fn sample_args() -> BundleArgs {
        BundleArgs {
            payload: PathBuf::from("payload"),
            car_out: PathBuf::from("car"),
            envelope_out: PathBuf::from("envelope"),
            indexes_out: None,
            ingest_metadata_out: None,
            event_id: "event".into(),
            stream_id: "stream".into(),
            rendition_id: "rendition".into(),
            track_kind: CliTrackKind::Video,
            codec: "av1-main".into(),
            bitrate_kbps: 2_000,
            resolution: Some("1920x1080".into()),
            audio_layout: None,
            segment_sequence: 1,
            segment_start_pts: 0,
            segment_duration: 1_000_000,
            wallclock_unix_ms: 1_700_000_000_000,
            manifest_hash: "aa".repeat(32),
            storage_ticket: "bb".repeat(32),
            ingest_latency_ms: None,
            live_edge_drift_ms: None,
            ingest_node_id: None,
            metadata_json: None,
        }
    }
}
