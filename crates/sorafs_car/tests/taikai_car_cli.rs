#![cfg(feature = "cli")]

use assert_cmd::cargo::cargo_bin_cmd;
use blake3::hash as blake3_hash;
use iroha_data_model::taikai::{
    SegmentDuration, SegmentTimestamp, TaikaiEnvelopeIndexes, TaikaiSegmentEnvelopeV1,
    TaikaiTrackKind,
};
use norito::{decode_from_bytes, json};
use std::{env, error::Error, fs, path::PathBuf, process::Command};
use tempfile::tempdir;

#[test]
fn taikai_car_cli_emits_bundle_indexes_and_metadata() -> Result<(), Box<dyn Error>> {
    let tempdir = tempdir()?;
    let payload_path = tempdir.path().join("segment.bin");
    let payload: Vec<u8> = (0..2048).map(|i| i as u8 ^ 0b1010_1010).collect();
    fs::write(&payload_path, payload)?;

    let car_out = tempdir.path().join("segment.car");
    let envelope_out = tempdir.path().join("segment.to");
    let indexes_out = tempdir.path().join("segment.indexes.json");
    let ingest_out = tempdir.path().join("segment.ingest.json");
    let summary_out = tempdir.path().join("segment.summary.json");

    let manifest_arg = format!("0x{}", "11".repeat(32));
    let ticket_arg = format!("0x{}", "ab".repeat(32));
    let segment_sequence = 42u64;
    let segment_start_pts = 3_600_000u64;
    let segment_duration = 2_000_000u32;
    let wallclock_ms = 1_702_560_000_000u64;
    let ingest_latency = 150u32;
    let ingest_latency_str = ingest_latency.to_string();
    let live_edge_drift = -25i32;

    let mut cmd = cargo_bin_cmd!("taikai_car");
    cmd.arg("--payload")
        .arg(&payload_path)
        .arg("--car-out")
        .arg(&car_out)
        .arg("--envelope-out")
        .arg(&envelope_out)
        .arg("--indexes-out")
        .arg(&indexes_out)
        .arg("--ingest-metadata-out")
        .arg(&ingest_out)
        .arg("--summary-out")
        .arg(&summary_out)
        .arg("--event-id")
        .arg("globalkeynote")
        .arg("--stream-id")
        .arg("stagea")
        .arg("--rendition-id")
        .arg("r1080p")
        .arg("--track-kind")
        .arg("video")
        .arg("--codec")
        .arg("av1-main")
        .arg("--bitrate-kbps")
        .arg("6000")
        .arg("--resolution")
        .arg("1920x1080")
        .arg("--segment-sequence")
        .arg(segment_sequence.to_string())
        .arg("--segment-start-pts")
        .arg(segment_start_pts.to_string())
        .arg("--segment-duration")
        .arg(segment_duration.to_string())
        .arg("--wallclock-unix-ms")
        .arg(wallclock_ms.to_string())
        .arg("--manifest-hash")
        .arg(&manifest_arg)
        .arg("--storage-ticket")
        .arg(&ticket_arg)
        .arg("--ingest-latency-ms")
        .arg(&ingest_latency_str)
        .arg(format!("--live-edge-drift-ms={}", live_edge_drift))
        .arg("--ingest-node-id")
        .arg("edge-a");
    cmd.assert().success();

    assert!(car_out.exists());
    assert!(envelope_out.exists());
    assert!(indexes_out.exists());
    assert!(ingest_out.exists());
    assert!(summary_out.exists());

    let envelope_bytes = fs::read(&envelope_out)?;
    let envelope: TaikaiSegmentEnvelopeV1 = decode_from_bytes(&envelope_bytes)?;
    assert_eq!(envelope.event_id.as_name().as_ref(), "globalkeynote");
    assert_eq!(envelope.stream_id.as_name().as_ref(), "stagea");
    assert_eq!(envelope.rendition_id.as_name().as_ref(), "r1080p");
    assert_eq!(envelope.track.kind, TaikaiTrackKind::Video);
    let resolution = envelope.track.resolution.expect("video resolution");
    assert_eq!((resolution.width, resolution.height), (1920, 1080));
    assert_eq!(envelope.track.average_bitrate_kbps, 6000);
    assert_eq!(envelope.segment_sequence, segment_sequence);
    assert_eq!(
        envelope.segment_start_pts.as_micros(),
        SegmentTimestamp::new(segment_start_pts).as_micros()
    );
    assert_eq!(
        envelope.segment_duration.as_micros(),
        SegmentDuration::new(segment_duration).as_micros()
    );
    assert_eq!(envelope.wallclock_unix_ms, wallclock_ms);
    assert_eq!(
        envelope.instrumentation.encoder_to_ingest_latency_ms,
        Some(ingest_latency)
    );
    assert_eq!(
        envelope.instrumentation.live_edge_drift_ms,
        Some(live_edge_drift)
    );
    assert_eq!(
        envelope.instrumentation.ingest_node_id.as_deref(),
        Some("edge-a")
    );

    let car_bytes = fs::read(&car_out)?;
    let digest = blake3_hash(&car_bytes);
    assert_eq!(envelope.ingest.car.car_size_bytes, car_bytes.len() as u64);
    assert_eq!(envelope.ingest.car.car_digest.as_bytes(), digest.as_bytes());

    let indexes_raw = fs::read(&indexes_out)?;
    let indexes: TaikaiEnvelopeIndexes = json::from_slice(&indexes_raw)?;
    assert_eq!(
        indexes.time_key.segment_start_pts.as_micros(),
        envelope.segment_start_pts.as_micros()
    );
    assert_eq!(
        indexes.cid_key.cid_multibase,
        envelope.ingest.car.cid_multibase
    );

    let ingest_raw = fs::read(&ingest_out)?;
    let ingest_value: json::Value = json::from_slice(&ingest_raw)?;
    let ingest_obj = ingest_value.as_object().expect("ingest metadata object");
    assert_eq!(
        ingest_obj
            .get("taikai.event_id")
            .and_then(json::Value::as_str),
        Some("globalkeynote")
    );
    assert_eq!(
        ingest_obj
            .get("taikai.instrumentation.ingest_latency_ms")
            .and_then(json::Value::as_str),
        Some(ingest_latency_str.as_str())
    );
    assert_eq!(
        ingest_obj
            .get("taikai.track.codec")
            .and_then(json::Value::as_str),
        Some("av1-main")
    );

    let summary_raw = fs::read(&summary_out)?;
    let summary_value: json::Value = json::from_slice(&summary_raw)?;
    let summary_obj = summary_value.as_object().expect("summary object");
    let car_obj = summary_obj
        .get("car")
        .and_then(json::Value::as_object)
        .expect("car summary");
    assert_eq!(
        car_obj.get("cid_multibase").and_then(json::Value::as_str),
        Some(envelope.ingest.car.cid_multibase.as_str())
    );
    let chunk_obj = summary_obj
        .get("chunk")
        .and_then(json::Value::as_object)
        .expect("chunk summary");
    assert_eq!(
        chunk_obj.get("count").and_then(json::Value::as_u64),
        Some(u64::from(envelope.ingest.chunk_count))
    );
    let expected_chunk_root = hex::encode(envelope.ingest.chunk_root.as_bytes());
    assert_eq!(
        chunk_obj
            .get("root_blake3_hex")
            .and_then(json::Value::as_str),
        Some(expected_chunk_root.as_str())
    );
    let outputs_obj = summary_obj
        .get("outputs")
        .and_then(json::Value::as_object)
        .expect("outputs summary");
    assert_eq!(
        outputs_obj.get("car").and_then(json::Value::as_str),
        Some(car_out.to_string_lossy().as_ref())
    );
    let indexes_obj = summary_obj
        .get("indexes")
        .and_then(json::Value::as_object)
        .expect("indexes summary");
    let time_key = indexes_obj
        .get("time_key")
        .and_then(json::Value::as_object)
        .expect("time_key");
    assert_eq!(
        time_key
            .get("segment_start_pts")
            .and_then(json::Value::as_u64),
        Some(segment_start_pts)
    );
    assert_eq!(
        time_key.get("event_id").and_then(json::Value::as_str),
        Some("globalkeynote")
    );
    let cid_key = indexes_obj
        .get("cid_key")
        .and_then(json::Value::as_object)
        .expect("cid_key");
    assert_eq!(
        cid_key.get("cid_multibase").and_then(json::Value::as_str),
        Some(envelope.ingest.car.cid_multibase.as_str())
    );
    assert_eq!(
        cid_key.get("rendition_id").and_then(json::Value::as_str),
        Some("r1080p")
    );

    Ok(())
}

fn taikai_publisher_example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("sdk/examples/taikai_publisher")
}

#[test]
fn taikai_publisher_sample_bundle_matches_config() -> Result<(), Box<dyn Error>> {
    let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());
    let python_check = Command::new(&python).arg("--version").output();
    if python_check.is_err() {
        eprintln!("skipping sample bundle test because `{python}` is unavailable");
        return Ok(());
    }

    let taikai_car_path = assert_cmd::cargo::cargo_bin!("taikai_car");
    let taikai_car = taikai_car_path
        .to_str()
        .expect("valid taikai_car path")
        .to_string();
    let sample_dir = taikai_publisher_example_dir();
    let script = sample_dir.join("bundle_sample.py");
    let config_primary = sample_dir.join("sample_config.json");
    let config_secondary = sample_dir.join("sample_config_720p.json");
    let config_audio = sample_dir.join("sample_config_audio.json");
    let tempdir = tempdir()?;
    let out_dir = tempdir.path().join("sample");
    let summary_path = out_dir.join("batch_summary.json");

    let output = Command::new(python)
        .arg(&script)
        .arg("--config")
        .arg(&config_primary)
        .arg("--config")
        .arg(&config_secondary)
        .arg("--config")
        .arg(&config_audio)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--print-command")
        .arg("--summary-json")
        .arg(&summary_path)
        .env("TAIKAI_CAR_BIN", &taikai_car)
        .output()?;
    assert!(
        output.status.success(),
        "bundle_sample.py failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let ingest_path = out_dir.join("sample_segment_0001.ingest.json");
    assert!(
        ingest_path.exists(),
        "expected ingest metadata at {}",
        ingest_path.display()
    );
    let ladder_ingest_path = out_dir.join("sample_segment_0001_720p.ingest.json");
    assert!(
        ladder_ingest_path.exists(),
        "expected second ingest metadata at {}",
        ladder_ingest_path.display()
    );
    let envelope_path = out_dir.join("sample_segment_0001.norito");
    assert!(
        envelope_path.exists(),
        "missing {}",
        envelope_path.display()
    );
    let car_path = out_dir.join("sample_segment_0001.car");
    assert!(car_path.exists(), "missing {}", car_path.display());
    let ladder_car_path = out_dir.join("sample_segment_0001_720p.car");
    assert!(
        ladder_car_path.exists(),
        "missing {}",
        ladder_car_path.display()
    );
    let audio_car_path = out_dir.join("sample_segment_audio_0001.car");
    assert!(
        audio_car_path.exists(),
        "missing {}",
        audio_car_path.display()
    );

    let ingest_raw = fs::read(&ingest_path)?;
    let ingest_value: json::Value = json::from_slice(&ingest_raw)?;
    let ingest_obj = ingest_value
        .as_object()
        .expect("ingest metadata JSON object");
    assert_eq!(
        ingest_obj
            .get("taikai.event_id")
            .and_then(json::Value::as_str),
        Some("global-keynote")
    );
    assert_eq!(
        ingest_obj
            .get("taikai.stream_id")
            .and_then(json::Value::as_str),
        Some("stage-a")
    );
    assert_eq!(
        ingest_obj
            .get("taikai.rendition_id")
            .and_then(json::Value::as_str),
        Some("1080p")
    );

    let ladder_ingest_raw = fs::read(&ladder_ingest_path)?;
    let ladder_ingest: json::Value = json::from_slice(&ladder_ingest_raw)?;
    let ladder_obj = ladder_ingest
        .as_object()
        .expect("ingest metadata JSON object");
    assert_eq!(
        ladder_obj
            .get("taikai.rendition_id")
            .and_then(json::Value::as_str),
        Some("720p")
    );

    let audio_ingest_path = out_dir.join("sample_segment_audio_0001.ingest.json");
    assert!(
        audio_ingest_path.exists(),
        "expected audio ingest metadata at {}",
        audio_ingest_path.display()
    );
    let audio_ingest_raw = fs::read(&audio_ingest_path)?;
    let audio_ingest: json::Value = json::from_slice(&audio_ingest_raw)?;
    let audio_obj = audio_ingest
        .as_object()
        .expect("ingest metadata JSON object");
    assert_eq!(
        audio_obj
            .get("taikai.track.kind")
            .and_then(json::Value::as_str),
        Some("audio")
    );
    assert_eq!(
        audio_obj
            .get("taikai.track.audio_layout")
            .and_then(json::Value::as_str),
        Some("stereo")
    );

    assert!(
        summary_path.exists(),
        "missing summary JSON at {}",
        summary_path.display()
    );
    let summary_value: json::Value = json::from_slice(&fs::read(&summary_path)?)?;
    let summary_array = summary_value.as_array().expect("summary array");
    assert_eq!(summary_array.len(), 3);
    let ladder_summary = summary_array
        .iter()
        .find(|entry| {
            entry
                .get("ingest")
                .and_then(json::Value::as_object)
                .and_then(|obj| obj.get("rendition_id"))
                .and_then(json::Value::as_str)
                == Some("720p")
        })
        .expect("summary entry for 720p ladder");
    assert!(
        ladder_summary
            .get("outputs")
            .and_then(json::Value::as_object)
            .is_some(),
        "summary should contain output paths"
    );
    let audio_summary = summary_array
        .iter()
        .find(|entry| {
            entry
                .get("ingest")
                .and_then(json::Value::as_object)
                .and_then(|obj| obj.get("rendition_id"))
                .and_then(json::Value::as_str)
                == Some("audio-stereo")
        })
        .expect("summary entry for audio track");
    assert!(
        audio_summary
            .get("outputs")
            .and_then(json::Value::as_object)
            .is_some(),
        "audio summary should contain output paths"
    );

    Ok(())
}

#[test]
fn taikai_car_cli_rehydrates_from_car_and_summary() -> Result<(), Box<dyn Error>> {
    let tempdir = tempdir()?;
    let payload_path = tempdir.path().join("rehydrate_segment.bin");
    let payload: Vec<u8> = (0..1024).map(|i| (i * 7) as u8).collect();
    fs::write(&payload_path, payload)?;

    let car_out = tempdir.path().join("segment.car");
    let envelope_out = tempdir.path().join("segment.to");
    let indexes_out = tempdir.path().join("segment.indexes.json");
    let ingest_out = tempdir.path().join("segment.ingest.json");
    let summary_out = tempdir.path().join("segment.summary.json");

    let manifest_arg = format!("0x{}", "22".repeat(32));
    let ticket_arg = format!("0x{}", "cd".repeat(32));

    let mut bundle_cmd = cargo_bin_cmd!("taikai_car");
    bundle_cmd
        .arg("--payload")
        .arg(&payload_path)
        .arg("--car-out")
        .arg(&car_out)
        .arg("--envelope-out")
        .arg(&envelope_out)
        .arg("--indexes-out")
        .arg(&indexes_out)
        .arg("--ingest-metadata-out")
        .arg(&ingest_out)
        .arg("--summary-out")
        .arg(&summary_out)
        .arg("--event-id")
        .arg("rehydrate-event")
        .arg("--stream-id")
        .arg("main")
        .arg("--rendition-id")
        .arg("1080p")
        .arg("--track-kind")
        .arg("video")
        .arg("--codec")
        .arg("av1-main")
        .arg("--bitrate-kbps")
        .arg("4500")
        .arg("--resolution")
        .arg("1920x1080")
        .arg("--segment-sequence")
        .arg("7")
        .arg("--segment-start-pts")
        .arg("1230000")
        .arg("--segment-duration")
        .arg("2000000")
        .arg("--wallclock-unix-ms")
        .arg("1702560000000")
        .arg("--manifest-hash")
        .arg(&manifest_arg)
        .arg("--storage-ticket")
        .arg(&ticket_arg);
    bundle_cmd.assert().success();

    let original_envelope = fs::read(&envelope_out)?;
    let original_indexes: json::Value = json::from_slice(&fs::read(&indexes_out)?)?;
    let original_ingest: json::Value = json::from_slice(&fs::read(&ingest_out)?)?;
    let original_car = fs::read(&car_out)?;

    let regen_dir = tempdir.path().join("rehydrated");
    fs::create_dir(&regen_dir)?;
    let car_copy = regen_dir.join("segment_copy.car");
    let envelope_copy = regen_dir.join("segment_copy.to");
    let indexes_copy = regen_dir.join("segment_copy.indexes.json");
    let ingest_copy = regen_dir.join("segment_copy.ingest.json");

    let mut rehydrate_cmd = cargo_bin_cmd!("taikai_car");
    rehydrate_cmd
        .arg("--car-in")
        .arg(&car_out)
        .arg("--car-out")
        .arg(&car_copy)
        .arg("--envelope-out")
        .arg(&envelope_copy)
        .arg("--indexes-out")
        .arg(&indexes_copy)
        .arg("--ingest-metadata-out")
        .arg(&ingest_copy)
        .arg("--summary-in")
        .arg(&summary_out);
    rehydrate_cmd.assert().success();

    assert_eq!(fs::read(&car_copy)?, original_car);
    assert_eq!(fs::read(&envelope_copy)?, original_envelope);

    let regen_indexes: json::Value = json::from_slice(&fs::read(&indexes_copy)?)?;
    assert_eq!(regen_indexes, original_indexes);

    let regen_ingest: json::Value = json::from_slice(&fs::read(&ingest_copy)?)?;
    assert_eq!(regen_ingest, original_ingest);
    Ok(())
}
