use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use assert_cmd::cargo::cargo_bin_cmd;
use norito::streaming::{
    EntropyMode,
    chunk::BaselineDecoder,
    codec::{
        BaselineEncoder, BaselineEncoderConfig, FrameDimensions, RawFrame, default_bundle_tables,
    },
};
use tempfile::TempDir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn generate_tables(path: &Path, seed: u64, bundle_width: u8) {
    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.args([
        "codec",
        "rans-tables",
        "--seed",
        &seed.to_string(),
        "--bundle-width",
        &bundle_width.to_string(),
        "--output",
        path.to_str().expect("utf8 output path"),
        "--format",
        "toml",
    ]);
    cmd.assert().success();
}

#[test]
fn rans_tables_generation_is_deterministic() {
    let temp = TempDir::new().expect("temp dir");
    let first = temp.path().join("tables_one.toml");
    let second = temp.path().join("tables_two.toml");

    generate_tables(&first, 7, 3);
    generate_tables(&second, 7, 3);

    let first_text = fs::read_to_string(first).expect("first tables");
    let second_text = fs::read_to_string(second).expect("second tables");
    assert_eq!(
        first_text, second_text,
        "rans table generation must be deterministic for a given seed and width"
    );
}

#[test]
fn verify_tables_detects_tampering() {
    let temp = TempDir::new().expect("temp dir");
    let good_path = temp.path().join("tables.toml");
    generate_tables(&good_path, 13, 4);

    let mut verify_good = cargo_bin_cmd!("xtask");
    verify_good.current_dir(workspace_root());
    verify_good.args([
        "verify-tables",
        "--tables",
        good_path.to_str().expect("utf8 tables path"),
    ]);
    verify_good.assert().success();

    let mut tampered = fs::read_to_string(&good_path).expect("tables text");
    assert!(
        tampered.contains("bundle_width"),
        "expected bundle_width field in generated tables"
    );
    tampered = tampered.replace("bundle_width = 4", "bundle_width = 3");
    let tampered_path = temp.path().join("tables_tampered.toml");
    fs::write(&tampered_path, tampered).expect("write tampered tables");

    let mut verify_bad = cargo_bin_cmd!("xtask");
    verify_bad.current_dir(workspace_root());
    verify_bad.args([
        "verify-tables",
        "--tables",
        tampered_path.to_str().expect("utf8 tampered path"),
    ]);
    verify_bad.assert().failure();
}

#[test]
fn bundled_tables_enable_roundtrip() {
    let tables = default_bundle_tables();

    let dimensions = FrameDimensions::new(8, 8);
    let frame_duration_ns = 25_000_000;
    let frames = vec![
        RawFrame::new(dimensions, vec![0x11; dimensions.pixel_count()]).expect("frame 0"),
        RawFrame::new(dimensions, vec![0x22; dimensions.pixel_count()]).expect("frame 1"),
    ];

    let mut encoder = BaselineEncoder::new(BaselineEncoderConfig {
        frame_dimensions: dimensions,
        frame_duration_ns,
        entropy_mode: EntropyMode::RansBundled,
        bundle_width: 4,
        bundle_tables: Arc::clone(&tables),
        ..BaselineEncoderConfig::default()
    });
    let segment = encoder
        .encode_segment(7, 1_000, 5, &frames, None)
        .expect("encode segment");
    assert_eq!(segment.header.entropy_mode, EntropyMode::RansBundled);

    let decoder = BaselineDecoder::new(dimensions, frame_duration_ns);
    let decoded = decoder
        .decode_segment(&segment)
        .expect("bundled decode path");
    assert_eq!(decoded.len(), frames.len(), "decoded frame count mismatch");
    for (idx, frame) in decoded.iter().enumerate() {
        assert_eq!(
            frame.luma, frames[idx].luma,
            "frame {idx} luma differs after bundled roundtrip"
        );
        assert_eq!(
            frame.pts_ns,
            u64::from(frame_duration_ns) * idx as u64,
            "frame {idx} PTS mismatch"
        );
    }
}
