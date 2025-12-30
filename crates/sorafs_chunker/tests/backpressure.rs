use std::{fs, path::PathBuf};

use norito::json::Value;
use sorafs_chunker::{Chunker, fixtures::FixtureProfile};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("resolve repository root")
}

fn fuzz_dir() -> PathBuf {
    repo_root().join("fuzz").join("sorafs_chunker")
}

fn load_backpressure_json() -> Value {
    let path = fuzz_dir().join("sf1_profile_v1_backpressure.json");
    let bytes = fs::read(path).expect("read backpressure corpus json");
    norito::json::from_slice(&bytes).expect("parse backpressure json corpus")
}

fn load_input_bytes() -> Vec<u8> {
    let path = fuzz_dir().join("sf1_profile_v1_input.bin");
    fs::read(path).expect("read backpressure input corpus")
}

fn as_array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .unwrap_or_else(|| panic!("missing key {key}"))
        .as_array()
        .unwrap_or_else(|| panic!("{key} is not an array"))
}

fn as_str<'a>(value: &'a Value, key: &str) -> &'a str {
    value
        .get(key)
        .unwrap_or_else(|| panic!("missing key {key}"))
        .as_str()
        .unwrap_or_else(|| panic!("{key} is not a string"))
}

fn as_usize(value: &Value) -> usize {
    value
        .as_u64()
        .unwrap_or_else(|| panic!("expected positive integer, got {value:?}")) as usize
}

#[test]
fn backpressure_corpus_matches_fixture_vectors() {
    let vectors = FixtureProfile::SF1_V1.generate_vectors();
    let json = load_backpressure_json();

    let profile = json
        .get("profile")
        .and_then(Value::as_str)
        .expect("profile string");
    assert!(
        profile == vectors.profile_id || profile == "sorafs.sf1@1.0.0",
        "unexpected profile {profile}"
    );
    assert_eq!(
        json.get("input_length")
            .and_then(Value::as_u64)
            .expect("input length") as usize,
        vectors.input_length
    );
    assert_eq!(
        json.get("chunk_digest_sha3_256")
            .and_then(Value::as_str)
            .expect("digest string"),
        vectors.sha3_digest_hex()
    );

    let scenarios = json
        .get("scenarios")
        .and_then(Value::as_array)
        .expect("scenarios array");
    assert!(
        !scenarios.is_empty(),
        "backpressure corpus must contain at least one scenario"
    );

    for scenario in scenarios {
        let name = as_str(scenario, "name");
        let feed_sizes: Vec<usize> = as_array(scenario, "feed_sizes")
            .iter()
            .map(as_usize)
            .collect();
        assert!(
            !feed_sizes.is_empty(),
            "scenario {name} must have feed sizes"
        );
        let feed_sum: usize = feed_sizes.iter().sum();
        assert_eq!(
            feed_sum, vectors.input_length,
            "scenario {name} feed sizes must equal input length"
        );

        let expected_lengths: Vec<usize> = as_array(scenario, "expected_chunk_lengths")
            .iter()
            .map(as_usize)
            .collect();
        assert_eq!(
            expected_lengths, vectors.chunk_lengths,
            "scenario {name} expected lengths must match canonical fixture"
        );

        assert_eq!(
            scenario
                .get("chunk_count")
                .and_then(Value::as_u64)
                .expect("chunk count") as usize,
            vectors.chunk_lengths.len(),
            "scenario {name} chunk count mismatch"
        );

        // Ensure feed bounds are consistent.
        let max_feed = scenario
            .get("max_feed_size")
            .and_then(Value::as_u64)
            .expect("max feed size") as usize;
        let min_feed = scenario
            .get("min_feed_size")
            .and_then(Value::as_u64)
            .expect("min feed size") as usize;
        assert!(
            max_feed >= min_feed,
            "scenario {name} max_feed should be >= min_feed"
        );
        assert_eq!(
            max_feed,
            feed_sizes
                .iter()
                .copied()
                .max()
                .expect("scenario {name} max feed non-empty"),
            "scenario {name} max feed mismatch"
        );
        assert_eq!(
            min_feed,
            feed_sizes
                .iter()
                .copied()
                .min()
                .expect("scenario {name} min feed non-empty"),
            "scenario {name} min feed mismatch"
        );
    }
}

#[test]
fn streaming_chunker_matches_backpressure_scenarios() {
    let vectors = FixtureProfile::SF1_V1.generate_vectors();
    let input = load_input_bytes();
    assert_eq!(input.len(), vectors.input_length);
    assert_eq!(
        input, vectors.input,
        "input corpus must match fixture vectors"
    );

    let json = load_backpressure_json();
    let scenarios = as_array(&json, "scenarios");

    for scenario in scenarios {
        let name = as_str(scenario, "name");
        let feed_sizes: Vec<usize> = as_array(scenario, "feed_sizes")
            .iter()
            .map(as_usize)
            .collect();
        assert!(
            !feed_sizes.is_empty(),
            "scenario {name} must have feed sizes"
        );

        let mut chunker = Chunker::new();
        let mut lengths = Vec::new();
        let mut offsets = Vec::new();
        let mut consumed = 0usize;
        for feed in &feed_sizes {
            let end = consumed + feed;
            let slice = &input[consumed..end];
            chunker.feed(slice, |chunk| {
                lengths.push(chunk.length);
                offsets.push(chunk.offset);
            });
            consumed = end;
        }
        chunker.finish(|chunk| {
            if chunk.length > 0 {
                lengths.push(chunk.length);
                offsets.push(chunk.offset);
            }
        });

        assert_eq!(
            consumed,
            input.len(),
            "scenario {name} feed sizes must consume entire input"
        );
        assert_eq!(
            lengths, vectors.chunk_lengths,
            "scenario {name} chunk lengths mismatch"
        );
        assert_eq!(
            offsets, vectors.chunk_offsets,
            "scenario {name} chunk offsets mismatch"
        );
    }
}
