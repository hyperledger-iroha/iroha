use std::{
    borrow::Cow,
    fs::{self, File},
    path::{Path, PathBuf},
};

use eyre::{Result, WrapErr, eyre};
use iroha_data_model::{
    da::types::{BlobDigest, ExtraMetadata, StorageTicketId},
    taikai::{
        SegmentDuration, SegmentTimestamp, TaikaiCarPointer, TaikaiEnvelopeIndexes, TaikaiEventId,
        TaikaiIngestPointer, TaikaiRenditionId, TaikaiSegmentEnvelopeV1, TaikaiStreamId,
        TaikaiTrackKind, TaikaiTrackMetadata,
    },
};
use norito::json::{self, Map, Value};

use crate::{CarWriter, RAW_CODEC, ingest_single_file, verifier::ParsedCar};

/// Request describing a Taikai segment bundle operation.
pub struct BundleRequest<'a> {
    pub payload_path: &'a Path,
    pub payload_bytes: Option<&'a [u8]>,
    pub car_out: &'a Path,
    pub envelope_out: &'a Path,
    pub indexes_out: Option<&'a Path>,
    pub ingest_metadata_out: Option<&'a Path>,
    pub manifest_hash: BlobDigest,
    pub storage_ticket: StorageTicketId,
    pub event_id: TaikaiEventId,
    pub stream_id: TaikaiStreamId,
    pub rendition_id: TaikaiRenditionId,
    pub track: TaikaiTrackMetadata,
    pub segment_sequence: u64,
    pub segment_start_pts: u64,
    pub segment_duration: u32,
    pub wallclock_unix_ms: u64,
    pub ingest_latency_ms: Option<u32>,
    pub live_edge_drift_ms: Option<i32>,
    pub ingest_node_id: Option<String>,
    pub extra_metadata: Option<ExtraMetadata>,
}

/// Summary describing the bundle artifacts that were generated.
#[derive(Debug, Clone)]
pub struct BundleSummary {
    pub car_pointer: TaikaiCarPointer,
    pub chunk_root: BlobDigest,
    pub chunk_count: u32,
    pub car_out: PathBuf,
    pub envelope_out: PathBuf,
    pub indexes: TaikaiEnvelopeIndexes,
    pub indexes_out: Option<PathBuf>,
    pub ingest_metadata_out: Option<PathBuf>,
    pub ingest_metadata: Map,
}

struct SegmentDetails<'a> {
    event_id: &'a TaikaiEventId,
    stream_id: &'a TaikaiStreamId,
    rendition_id: &'a TaikaiRenditionId,
    track: &'a TaikaiTrackMetadata,
    segment_sequence: u64,
    segment_start_pts: u64,
    segment_duration: u32,
    wallclock_unix_ms: u64,
    ingest_latency_ms: Option<u32>,
    live_edge_drift_ms: Option<i32>,
    ingest_node_id: Option<&'a str>,
    extra_metadata: Option<ExtraMetadata>,
}

/// Request describing how to regenerate Taikai bundle metadata from an existing CAR.
pub struct RehydrateRequest<'a> {
    pub car_in: &'a Path,
    pub car_out: &'a Path,
    pub envelope_out: &'a Path,
    pub indexes_out: Option<&'a Path>,
    pub ingest_metadata_out: Option<&'a Path>,
    pub manifest_hash: BlobDigest,
    pub storage_ticket: StorageTicketId,
    pub event_id: TaikaiEventId,
    pub stream_id: TaikaiStreamId,
    pub rendition_id: TaikaiRenditionId,
    pub track: TaikaiTrackMetadata,
    pub segment_sequence: u64,
    pub segment_start_pts: u64,
    pub segment_duration: u32,
    pub wallclock_unix_ms: u64,
    pub ingest_latency_ms: Option<u32>,
    pub live_edge_drift_ms: Option<i32>,
    pub ingest_node_id: Option<String>,
    pub extra_metadata: Option<ExtraMetadata>,
}

/// Bundle a Taikai segment into deterministic CAR + Norito artifacts.
pub fn bundle_segment(request: &BundleRequest<'_>) -> Result<BundleSummary> {
    let payload_cow: Cow<'_, [u8]> = if let Some(bytes) = request.payload_bytes {
        Cow::Borrowed(bytes)
    } else {
        Cow::Owned(fs::read(request.payload_path).wrap_err_with(|| {
            format!(
                "failed to read payload `{}`",
                request.payload_path.display()
            )
        })?)
    };

    if payload_cow.is_empty() {
        return Err(eyre!(
            "payload `{}` is empty; segments must contain data",
            request.payload_path.display()
        ));
    }

    let summary = ingest_single_file(payload_cow.as_ref()).map_err(|err| {
        eyre!(
            "failed to build CAR plan for `{}`: {err}",
            request.payload_path.display()
        )
    })?;
    let chunk_count: u32 = summary
        .chunk_store
        .chunks()
        .len()
        .try_into()
        .map_err(|_| eyre!("chunk count exceeds u32::MAX"))?;
    let chunk_root = BlobDigest::new(*summary.chunk_store.por_tree().root());

    let writer = CarWriter::new(&summary.plan, payload_cow.as_ref())
        .map_err(|err| eyre!("failed to initialise CAR writer: {err}"))?;
    let mut car_file = File::create(request.car_out)
        .wrap_err_with(|| format!("failed to create `{}`", request.car_out.display()))?;
    let car_stats = writer.write_to(&mut car_file).map_err(|err| {
        eyre!(
            "failed to write CAR archive `{}`: {err}",
            request.car_out.display()
        )
    })?;

    let car_digest = BlobDigest::from_hash(car_stats.car_archive_digest);
    let cid_multibase = format!("b{}", encode_base32_lower(&car_stats.car_cid));
    let car_pointer = TaikaiCarPointer::new(cid_multibase.clone(), car_digest, car_stats.car_size);

    let ingest_pointer = TaikaiIngestPointer::new(
        request.manifest_hash,
        request.storage_ticket,
        chunk_root,
        chunk_count,
        car_pointer.clone(),
    );

    let details = SegmentDetails {
        event_id: &request.event_id,
        stream_id: &request.stream_id,
        rendition_id: &request.rendition_id,
        track: &request.track,
        segment_sequence: request.segment_sequence,
        segment_start_pts: request.segment_start_pts,
        segment_duration: request.segment_duration,
        wallclock_unix_ms: request.wallclock_unix_ms,
        ingest_latency_ms: request.ingest_latency_ms,
        live_edge_drift_ms: request.live_edge_drift_ms,
        ingest_node_id: request.ingest_node_id.as_deref(),
        extra_metadata: request.extra_metadata.clone(),
    };

    let envelope = build_envelope(&details, ingest_pointer);
    let ingest_metadata = build_ingest_metadata_from_details(&details)?;

    write_outputs(
        &envelope,
        ingest_metadata,
        request.envelope_out,
        request.indexes_out,
        request.ingest_metadata_out,
        request.car_out,
    )
}

fn build_envelope(
    details: &SegmentDetails<'_>,
    ingest_pointer: TaikaiIngestPointer,
) -> TaikaiSegmentEnvelopeV1 {
    let mut envelope = TaikaiSegmentEnvelopeV1::new(
        details.event_id.clone(),
        details.stream_id.clone(),
        details.rendition_id.clone(),
        details.track.clone(),
        details.segment_sequence,
        SegmentTimestamp::new(details.segment_start_pts),
        SegmentDuration::new(details.segment_duration),
        details.wallclock_unix_ms,
        ingest_pointer,
    );
    if let Some(latency) = details.ingest_latency_ms {
        envelope.instrumentation.encoder_to_ingest_latency_ms = Some(latency);
    }
    if let Some(drift) = details.live_edge_drift_ms {
        envelope.instrumentation.live_edge_drift_ms = Some(drift);
    }
    if let Some(node_id) = details.ingest_node_id {
        envelope.instrumentation.ingest_node_id = Some(node_id.to_owned());
    }
    if let Some(extra) = &details.extra_metadata {
        envelope.metadata = extra.clone();
    }
    envelope
}

fn build_ingest_metadata_from_details(details: &SegmentDetails<'_>) -> Result<Map> {
    let params = IngestMetadataParams {
        event_id: details.event_id,
        stream_id: details.stream_id,
        rendition_id: details.rendition_id,
        track: details.track,
        segment_sequence: details.segment_sequence,
        segment_start_pts: details.segment_start_pts,
        segment_duration: details.segment_duration,
        wallclock_unix_ms: details.wallclock_unix_ms,
        ingest_latency_ms: details.ingest_latency_ms,
        live_edge_drift_ms: details.live_edge_drift_ms,
        ingest_node_id: details.ingest_node_id,
    };
    build_ingest_metadata_inner(&params)
}

fn write_outputs(
    envelope: &TaikaiSegmentEnvelopeV1,
    ingest_metadata: Map,
    envelope_out: &Path,
    indexes_out: Option<&Path>,
    ingest_metadata_out: Option<&Path>,
    car_out: &Path,
) -> Result<BundleSummary> {
    let envelope_bytes =
        norito::to_bytes(envelope).wrap_err("failed to encode Taikai envelope payload")?;
    fs::write(envelope_out, envelope_bytes).wrap_err_with(|| {
        format!(
            "failed to write envelope output `{}`",
            envelope_out.display()
        )
    })?;

    let indexes = envelope.indexes();
    let indexes_out_paths = if let Some(path) = indexes_out {
        let rendered = json::to_json_pretty(&indexes)
            .map_err(|err| eyre!("failed to render Taikai index JSON: {err}"))?;
        fs::write(path, rendered.as_bytes())
            .wrap_err_with(|| format!("failed to write index output `{}`", path.display()))?;
        Some(path.to_path_buf())
    } else {
        None
    };

    let ingest_metadata_out_paths = if let Some(path) = ingest_metadata_out {
        let rendered = json::to_json_pretty(&Value::Object(ingest_metadata.clone()))
            .map_err(|err| eyre!("failed to render ingest metadata JSON: {err}"))?;
        fs::write(path, rendered.as_bytes()).wrap_err_with(|| {
            format!(
                "failed to write ingest metadata output `{}`",
                path.display()
            )
        })?;
        Some(path.to_path_buf())
    } else {
        None
    };

    Ok(BundleSummary {
        car_pointer: envelope.ingest.car.clone(),
        chunk_root: envelope.ingest.chunk_root,
        chunk_count: envelope.ingest.chunk_count,
        car_out: car_out.to_path_buf(),
        envelope_out: envelope_out.to_path_buf(),
        indexes,
        indexes_out: indexes_out_paths,
        ingest_metadata_out: ingest_metadata_out_paths,
        ingest_metadata,
    })
}

/// Rebuild Taikai envelope/index/ingest metadata for an existing CAR archive.
pub fn rehydrate_from_car(request: &RehydrateRequest<'_>) -> Result<BundleSummary> {
    let car_bytes = fs::read(request.car_in)
        .wrap_err_with(|| format!("failed to read CAR `{}`", request.car_in.display()))?;
    let parsed = ParsedCar::parse(&car_bytes)
        .map_err(|err| eyre!("failed to parse CAR `{}`: {err}", request.car_in.display()))?;

    let ingest_summary = ingest_single_file(parsed.payload()).map_err(|err| {
        eyre!(
            "failed to rebuild chunk plan from CAR payload `{}`: {err}",
            request.car_in.display()
        )
    })?;

    if ingest_summary.plan.chunks.len() != parsed.chunk_sections().len() {
        return Err(eyre!(
            "chunk count mismatch between CAR ({}) and rebuilt plan ({})",
            parsed.chunk_sections().len(),
            ingest_summary.plan.chunks.len()
        ));
    }
    for (idx, (plan_chunk, parsed_chunk)) in ingest_summary
        .plan
        .chunks
        .iter()
        .zip(parsed.chunk_sections())
        .enumerate()
    {
        if plan_chunk.length != parsed_chunk.length {
            return Err(eyre!(
                "chunk length mismatch at index {idx} (plan={}, car={})",
                plan_chunk.length,
                parsed_chunk.length
            ));
        }
        if plan_chunk.digest != parsed_chunk.digest {
            return Err(eyre!(
                "chunk digest mismatch at index {idx} for CAR `{}`",
                request.car_in.display()
            ));
        }
    }

    let chunk_count: u32 = ingest_summary
        .chunk_store
        .chunks()
        .len()
        .try_into()
        .map_err(|_| eyre!("chunk count exceeds u32::MAX"))?;
    let chunk_root = BlobDigest::new(*ingest_summary.chunk_store.por_tree().root());

    let car_digest = BlobDigest::from_hash(parsed.car_archive_digest());
    let car_cid = crate::encode_cid(RAW_CODEC, parsed.car_archive_digest().as_bytes());
    let cid_multibase = format!("b{}", encode_base32_lower(&car_cid));
    let car_pointer = TaikaiCarPointer::new(cid_multibase, car_digest, parsed.total_len());

    let ingest_pointer = TaikaiIngestPointer::new(
        request.manifest_hash,
        request.storage_ticket,
        chunk_root,
        chunk_count,
        car_pointer,
    );

    let details = SegmentDetails {
        event_id: &request.event_id,
        stream_id: &request.stream_id,
        rendition_id: &request.rendition_id,
        track: &request.track,
        segment_sequence: request.segment_sequence,
        segment_start_pts: request.segment_start_pts,
        segment_duration: request.segment_duration,
        wallclock_unix_ms: request.wallclock_unix_ms,
        ingest_latency_ms: request.ingest_latency_ms,
        live_edge_drift_ms: request.live_edge_drift_ms,
        ingest_node_id: request.ingest_node_id.as_deref(),
        extra_metadata: request.extra_metadata.clone(),
    };

    if request.car_out != request.car_in {
        fs::write(request.car_out, &car_bytes).wrap_err_with(|| {
            format!("failed to write CAR output `{}`", request.car_out.display())
        })?;
    }

    let envelope = build_envelope(&details, ingest_pointer);
    let ingest_metadata = build_ingest_metadata_from_details(&details)?;

    write_outputs(
        &envelope,
        ingest_metadata,
        request.envelope_out,
        request.indexes_out,
        request.ingest_metadata_out,
        request.car_out,
    )
}

/// Load the optional extra metadata JSON document used by publishers.
pub fn load_extra_metadata(path: &Path) -> Result<ExtraMetadata> {
    let contents = fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read metadata JSON `{}`", path.display()))?;
    json::from_str(&contents)
        .wrap_err_with(|| format!("failed to parse metadata JSON `{}`", path.display()))
}

struct IngestMetadataParams<'a> {
    event_id: &'a TaikaiEventId,
    stream_id: &'a TaikaiStreamId,
    rendition_id: &'a TaikaiRenditionId,
    track: &'a TaikaiTrackMetadata,
    segment_sequence: u64,
    segment_start_pts: u64,
    segment_duration: u32,
    wallclock_unix_ms: u64,
    ingest_latency_ms: Option<u32>,
    live_edge_drift_ms: Option<i32>,
    ingest_node_id: Option<&'a str>,
}

fn build_ingest_metadata_inner(params: &IngestMetadataParams<'_>) -> Result<Map> {
    let mut map = Map::new();
    map.insert(
        "taikai.event_id".into(),
        Value::from(params.event_id.as_name().as_ref()),
    );
    map.insert(
        "taikai.stream_id".into(),
        Value::from(params.stream_id.as_name().as_ref()),
    );
    map.insert(
        "taikai.rendition_id".into(),
        Value::from(params.rendition_id.as_name().as_ref()),
    );

    let (kind_label, codec_label, resolution_label, audio_layout_label) =
        track_labels(params.track);
    map.insert("taikai.track.kind".into(), Value::from(kind_label));
    map.insert("taikai.track.codec".into(), Value::from(codec_label));
    map.insert(
        "taikai.track.bitrate_kbps".into(),
        Value::from(params.track.average_bitrate_kbps.to_string()),
    );
    if let Some(resolution) = resolution_label {
        map.insert(
            "taikai.track.resolution".into(),
            Value::from(resolution.to_string()),
        );
    }
    if let Some(layout) = audio_layout_label {
        map.insert(
            "taikai.track.audio_layout".into(),
            Value::from(layout.to_string()),
        );
    }

    map.insert(
        "taikai.segment.sequence".into(),
        Value::from(params.segment_sequence.to_string()),
    );
    map.insert(
        "taikai.segment.start_pts".into(),
        Value::from(params.segment_start_pts.to_string()),
    );
    map.insert(
        "taikai.segment.duration".into(),
        Value::from(params.segment_duration.to_string()),
    );
    map.insert(
        "taikai.wallclock_unix_ms".into(),
        Value::from(params.wallclock_unix_ms.to_string()),
    );
    if let Some(latency) = params.ingest_latency_ms {
        map.insert(
            "taikai.instrumentation.ingest_latency_ms".into(),
            Value::from(latency.to_string()),
        );
    }
    if let Some(drift) = params.live_edge_drift_ms {
        map.insert(
            "taikai.instrumentation.live_edge_drift_ms".into(),
            Value::from(drift.to_string()),
        );
    }
    if let Some(node_id) = params.ingest_node_id {
        map.insert(
            "taikai.instrumentation.ingest_node_id".into(),
            Value::from(node_id.to_owned()),
        );
    }
    Ok(map)
}

fn track_labels(
    track: &TaikaiTrackMetadata,
) -> (&'static str, String, Option<String>, Option<String>) {
    let kind_label = match track.kind {
        TaikaiTrackKind::Video => "video",
        TaikaiTrackKind::Audio => "audio",
        TaikaiTrackKind::Data => "data",
    };
    let codec_label = match &track.codec {
        iroha_data_model::taikai::TaikaiCodec::AvcHigh => "avc-high".to_owned(),
        iroha_data_model::taikai::TaikaiCodec::HevcMain10 => "hevc-main10".to_owned(),
        iroha_data_model::taikai::TaikaiCodec::Av1Main => "av1-main".to_owned(),
        iroha_data_model::taikai::TaikaiCodec::AacLc => "aac-lc".to_owned(),
        iroha_data_model::taikai::TaikaiCodec::Opus => "opus".to_owned(),
        iroha_data_model::taikai::TaikaiCodec::Custom(name) => format!("custom:{name}"),
    };
    let resolution_label = track
        .resolution
        .as_ref()
        .map(|res| format!("{}x{}", res.width, res.height));
    let audio_layout_label = track.audio_layout.as_ref().map(|layout| match layout {
        iroha_data_model::taikai::TaikaiAudioLayout::Mono => "mono".to_owned(),
        iroha_data_model::taikai::TaikaiAudioLayout::Stereo => "stereo".to_owned(),
        iroha_data_model::taikai::TaikaiAudioLayout::FiveOne => "5.1".to_owned(),
        iroha_data_model::taikai::TaikaiAudioLayout::SevenOne => "7.1".to_owned(),
        iroha_data_model::taikai::TaikaiAudioLayout::Custom(channels) => {
            format!("custom:{channels}")
        }
    });
    (
        kind_label,
        codec_label,
        resolution_label,
        audio_layout_label,
    )
}

fn encode_base32_lower(data: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    if data.is_empty() {
        return String::new();
    }
    let mut acc = 0u32;
    let mut bits = 0u32;
    let mut out = Vec::with_capacity((data.len() * 8).div_ceil(5));
    for byte in data {
        acc = (acc << 8) | (*byte as u32);
        bits += 8;
        while bits >= 5 {
            let index = ((acc >> (bits - 5)) & 0x1F) as usize;
            out.push(ALPHABET[index]);
            bits -= 5;
        }
    }
    if bits > 0 {
        let index = ((acc << (5 - bits)) & 0x1F) as usize;
        out.push(ALPHABET[index]);
    }
    String::from_utf8(out).expect("base32 alphabet valid")
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use iroha_data_model::{
        name::Name,
        taikai::{TaikaiAudioLayout, TaikaiCodec, TaikaiResolution},
    };
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn bundle_writes_outputs() {
        let tmp = tempdir().expect("tempdir");
        let payload = tmp.path().join("segment.bin");
        fs::write(&payload, b"taikai-payload").expect("write payload");
        let car_out = tmp.path().join("segment.car");
        let envelope_out = tmp.path().join("segment.to");
        let indexes_out = tmp.path().join("segment.indexes.json");
        let ingest_out = tmp.path().join("segment.ingest.json");
        let request = BundleRequest {
            payload_path: &payload,
            payload_bytes: None,
            car_out: &car_out,
            envelope_out: &envelope_out,
            indexes_out: Some(&indexes_out),
            ingest_metadata_out: Some(&ingest_out),
            manifest_hash: BlobDigest::new([0u8; 32]),
            storage_ticket: StorageTicketId::new([1u8; 32]),
            event_id: TaikaiEventId::new(Name::from_str("event").expect("name")),
            stream_id: TaikaiStreamId::new(Name::from_str("stream").expect("name")),
            rendition_id: TaikaiRenditionId::new(Name::from_str("1080p").expect("name")),
            track: TaikaiTrackMetadata::video(
                TaikaiCodec::Av1Main,
                8_000,
                TaikaiResolution {
                    width: 1920,
                    height: 1080,
                },
            ),
            segment_sequence: 42,
            segment_start_pts: 36_000,
            segment_duration: 2_000_000,
            wallclock_unix_ms: 1_702_560_000_000,
            ingest_latency_ms: Some(120),
            live_edge_drift_ms: Some(-45),
            ingest_node_id: Some("node-a".into()),
            extra_metadata: None,
        };

        let summary = bundle_segment(&request).expect("bundle");
        assert_eq!(summary.chunk_count, 1);
        assert!(summary.car_out.exists());
        assert!(summary.envelope_out.exists());
        assert_eq!(summary.indexes.time_key.event_id, request.event_id);
        assert_eq!(summary.indexes.time_key.stream_id, request.stream_id);
        assert_eq!(summary.indexes.time_key.rendition_id, request.rendition_id);
        assert_eq!(
            summary.indexes.time_key.segment_start_pts.as_micros(),
            request.segment_start_pts
        );
        assert_eq!(
            summary.indexes.cid_key.cid_multibase,
            summary.car_pointer.cid_multibase
        );
        assert!(summary.indexes_out.as_ref().unwrap().exists());
        assert!(summary.ingest_metadata_out.as_ref().unwrap().exists());
    }

    #[test]
    fn track_labels_cover_audio() {
        let track =
            TaikaiTrackMetadata::audio(TaikaiCodec::Opus, 192, TaikaiAudioLayout::Custom(6));
        let (_, codec, _, layout) = track_labels(&track);
        assert_eq!(codec, "opus");
        assert_eq!(layout.as_deref(), Some("custom:6"));
    }
}
