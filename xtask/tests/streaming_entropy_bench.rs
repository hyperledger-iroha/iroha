use std::{fs, io::Write, path::PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::{self, Value};
use tempfile::TempDir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn streaming_entropy_bench_emits_metrics() {
    let temp = TempDir::new().expect("temp dir");
    let json_out = temp.path().join("entropy_bench.json");
    let bundle_out = temp.path().join("bundle.norito");
    let reference_y4m = temp.path().join("baseline.y4m");

    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.args([
        "streaming-entropy-bench",
        "--frames",
        "2",
        "--segments",
        "2",
        "--width",
        "3",
        "--psnr-mode",
        "y",
        "--json-out",
        json_out.to_str().expect("utf8 path"),
        "--chunk-out",
        bundle_out.to_str().expect("utf8 path"),
        "--y4m-out",
        reference_y4m.to_str().expect("utf8 path"),
    ]);
    cmd.assert().success();

    let raw = fs::read_to_string(json_out).expect("bench json");
    let report: Value = json::from_str(&raw).expect("parse bench json");
    let root = report.as_object().expect("bench root object");
    assert_eq!(
        root.get("frames_per_segment").and_then(Value::as_u64),
        Some(2),
        "frames_per_segment should reflect CLI override"
    );
    assert_eq!(
        root.get("segments").and_then(Value::as_u64),
        Some(2),
        "segments should reflect CLI override"
    );
    assert_eq!(
        root.get("psnr_mode").and_then(Value::as_str),
        Some("y"),
        "psnr_mode should be recorded in the output"
    );

    let quantizer_runs = root
        .get("quantizer_runs")
        .and_then(Value::as_array)
        .expect("quantizer_runs array");
    assert!(
        !quantizer_runs.is_empty(),
        "quantizer_runs should include at least one run"
    );
    assert_eq!(
        root.get("tiny_clip_preset").and_then(Value::as_bool),
        Some(false),
        "tiny_clip_preset should default to false"
    );
    let bench = root
        .get("bench")
        .and_then(Value::as_object)
        .expect("bench metrics");
    let bench_bytes = bench
        .get("chunk_bytes")
        .and_then(Value::as_u64)
        .expect("bench chunk bytes");
    assert!(bench_bytes > 0, "bench chunk bytes should be recorded");
    let bench_encode = bench
        .get("encode_ms")
        .and_then(Value::as_f64)
        .expect("bench encode_ms");
    assert!(
        bench_encode >= 0.0,
        "bench encode_ms should be non-negative"
    );

    let targets = root
        .get("bitrate_targets")
        .and_then(Value::as_array)
        .expect("bitrate_targets array");
    assert!(
        targets.is_empty(),
        "bitrate_targets should be empty when no ladder is requested"
    );

    assert_eq!(
        root.get("bundled_available").and_then(Value::as_bool),
        Some(true),
        "bundled builds should advertise bundled_available = true"
    );

    assert!(
        bundle_out.exists(),
        "streaming-entropy-bench should emit bundle output when requested"
    );
    assert!(
        reference_y4m.exists(),
        "streaming-entropy-bench should emit decoded Y4M when requested"
    );

    let decoded_y4m = temp.path().join("decoded.y4m");
    let mut decode_cmd = cargo_bin_cmd!("xtask");
    decode_cmd.current_dir(workspace_root());
    decode_cmd.args([
        "streaming-decode",
        "--bundle",
        bundle_out.to_str().expect("utf8 path"),
        "--y4m-out",
        decoded_y4m.to_str().expect("utf8 path"),
        "--psnr-ref",
        reference_y4m.to_str().expect("utf8 path"),
        "--json-out",
        "-",
    ]);
    let output = decode_cmd.assert().success().get_output().stdout.clone();
    let decode_report: Value = json::from_slice(&output).expect("decode json");
    assert_eq!(
        decode_report
            .get("frames")
            .and_then(Value::as_u64)
            .expect("decoded frames count"),
        4,
        "decoded output should include all frames across segments"
    );
    assert!(
        decode_report.get("psnr_y").is_some(),
        "decoder output should report PSNR when reference provided"
    );
    assert!(decoded_y4m.exists(), "decoder should write Y4M output");
}

#[test]
fn streaming_entropy_bench_roundtrips_chroma_and_yuv_psnr() {
    let temp = TempDir::new().expect("temp dir");
    let y4m_input = temp.path().join("input.y4m");
    let bench_json = temp.path().join("bench.json");
    let bundle_out = temp.path().join("bundle.norito");
    let bench_y4m_out = temp.path().join("bench_ref.y4m");
    let decoded_out = temp.path().join("decoded.y4m");

    write_test_y4m(&y4m_input);

    let mut bench_cmd = cargo_bin_cmd!("xtask");
    bench_cmd.current_dir(workspace_root());
    bench_cmd.args([
        "streaming-entropy-bench",
        "--y4m-in",
        y4m_input.to_str().expect("utf8 path"),
        "--json-out",
        bench_json.to_str().expect("utf8 path"),
        "--chunk-out",
        bundle_out.to_str().expect("utf8 path"),
        "--y4m-out",
        bench_y4m_out.to_str().expect("utf8 path"),
        "--psnr-mode",
        "yuv",
    ]);
    bench_cmd.assert().success();
    assert!(bench_json.exists(), "bench json output should be written");
    assert!(
        bench_y4m_out.exists(),
        "bench reference Y4M should be written"
    );

    let mut decode_cmd = cargo_bin_cmd!("xtask");
    decode_cmd.current_dir(workspace_root());
    decode_cmd.args([
        "streaming-decode",
        "--bundle",
        bundle_out.to_str().expect("utf8 path"),
        "--y4m-out",
        decoded_out.to_str().expect("utf8 path"),
        "--psnr-ref",
        y4m_input.to_str().expect("utf8 path"),
        "--psnr-mode",
        "yuv",
        "--json-out",
        "-",
    ]);
    let output = decode_cmd.assert().success().get_output().stdout.clone();
    let decode_report: Value = json::from_slice(&output).expect("decode json");
    assert_eq!(
        decode_report
            .get("psnr_mode")
            .and_then(Value::as_str)
            .expect("psnr_mode"),
        "yuv",
        "decoder should record psnr_mode=yuv"
    );
    let psnr_yuv = decode_report
        .get("psnr_yuv")
        .and_then(Value::as_f64)
        .expect("psnr_yuv present");
    assert!(
        psnr_yuv > 0.0,
        "expected finite PSNR when chroma is preserved, got {psnr_yuv}"
    );

    let original_chroma = read_y4m_chroma(&y4m_input);
    let decoded_chroma = read_y4m_chroma(&decoded_out);
    assert_eq!(
        original_chroma.len(),
        decoded_chroma.len(),
        "decoded Y4M should preserve chroma frame count"
    );
    let chroma_differs = original_chroma
        .iter()
        .zip(decoded_chroma.iter())
        .any(|((orig_u, orig_v), (dec_u, dec_v))| orig_u != dec_u || orig_v != dec_v);
    assert!(
        chroma_differs,
        "decoded chroma should reflect encoded output rather than a copied sidecar"
    );
}

#[test]
fn streaming_entropy_bench_respects_quantizer_override() {
    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.args([
        "streaming-entropy-bench",
        "--quantizer",
        "12",
        "--frames",
        "1",
        "--segments",
        "1",
        "--json-out",
        "-",
    ]);
    let output = cmd.assert().success().get_output().stdout.clone();
    let report: Value = json::from_slice(&output).expect("parse bench json");
    let baseline = report
        .get("bench")
        .and_then(Value::as_object)
        .expect("bench metrics");
    assert_eq!(
        baseline.get("quantizer").and_then(Value::as_u64),
        Some(12),
        "bench quantizer should respect override"
    );
}

#[test]
fn streaming_entropy_bench_supports_quantizer_ladder_and_tiny_preset() {
    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.args([
        "streaming-entropy-bench",
        "--quantizer",
        "8",
        "--quantizer",
        "14",
        "--target-bitrate-mbps",
        "0.5",
        "--tiny-clip-preset",
        "--frames",
        "1",
        "--segments",
        "1",
        "--json-out",
        "-",
    ]);
    let output = cmd.assert().success().get_output().stdout.clone();
    let report: Value = json::from_slice(&output).expect("bench json");
    let root = report.as_object().expect("bench root object");
    assert_eq!(
        root.get("tiny_clip_preset").and_then(Value::as_bool),
        Some(true),
        "tiny_clip_preset should be recorded when requested"
    );
    let runs = root
        .get("quantizer_runs")
        .and_then(Value::as_array)
        .expect("quantizer_runs array");
    assert!(
        runs.len() >= 2,
        "quantizer_runs should include each provided quantizer"
    );
    let targets = root
        .get("bitrate_targets")
        .and_then(Value::as_array)
        .expect("bitrate_targets array");
    assert_eq!(
        targets.len(),
        1,
        "single target bitrate should produce one ladder entry"
    );
    let target_entry = targets[0]
        .as_object()
        .expect("bitrate target should be object");
    assert!(
        target_entry.get("bench").is_some(),
        "ladder entry should be recorded"
    );
}

fn write_test_y4m(path: &std::path::Path) {
    let mut file = std::fs::File::create(path).expect("create y4m");
    writeln!(file, "YUV4MPEG2 W2 H2 F30:1 Ip A0:0 C420mpeg2").expect("header");
    for frame_idx in 0..2u8 {
        file.write_all(b"FRAME\n").expect("frame tag");
        let luma = [
            10 + frame_idx,
            20 + frame_idx,
            30 + frame_idx,
            40 + frame_idx,
        ];
        let u = [50 + frame_idx];
        let v = [200 - frame_idx];
        file.write_all(&luma).expect("luma");
        file.write_all(&u).expect("u plane");
        file.write_all(&v).expect("v plane");
    }
}

fn read_y4m_chroma(path: &std::path::Path) -> Vec<(Vec<u8>, Vec<u8>)> {
    use std::io::{BufRead, BufReader, Read};

    let file = std::fs::File::open(path).expect("open y4m");
    let mut reader = BufReader::new(file);
    let mut header = String::new();
    reader.read_line(&mut header).expect("read header");
    let mut width = None;
    let mut height = None;
    for token in header.split_ascii_whitespace() {
        if let Some(rest) = token.strip_prefix('W') {
            width = rest.parse::<usize>().ok();
        } else if let Some(rest) = token.strip_prefix('H') {
            height = rest.parse::<usize>().ok();
        }
    }
    let width = width.expect("width");
    let height = height.expect("height");
    let luma_len = width * height;
    let chroma_len = luma_len / 4;

    let mut chroma = Vec::new();
    loop {
        let mut tag = String::new();
        if reader.read_line(&mut tag).expect("frame tag") == 0 {
            break;
        }
        if !tag.starts_with("FRAME") {
            break;
        }
        let mut buf = vec![0u8; luma_len + chroma_len * 2];
        reader.read_exact(&mut buf).expect("frame body");
        let u = buf[luma_len..(luma_len + chroma_len)].to_vec();
        let v = buf[(luma_len + chroma_len)..].to_vec();
        chroma.push((u, v));
    }
    chroma
}
