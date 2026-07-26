use std::{
    io::Write,
    process::{Command, Stdio},
};

use blake3::{Hash, Hasher};
use norito::json::Value;
use sorafs_chunker::{Chunk, ChunkProfile, Chunker, fixtures::FixtureProfile};

const TOTAL_LEN: usize = 1 << 30; // 1 GiB
const PROFILE: FixtureProfile = FixtureProfile::SF1_V1;

fn collect_chunks(template: &[u8]) -> Vec<Chunk> {
    let mut chunker = Chunker::new();
    let mut chunks = Vec::new();
    let repeat = TOTAL_LEN / template.len();
    assert_eq!(
        repeat * template.len(),
        TOTAL_LEN,
        "template length does not evenly divide 1 GiB"
    );

    for _ in 0..repeat {
        chunker.feed(template, |chunk| chunks.push(chunk));
    }
    chunker.finish(|chunk| chunks.push(chunk));
    chunks
}

fn replay_chunks(chunks: &[Chunk], template: &[u8]) -> (blake3::Hash, Vec<blake3::Hash>) {
    let mut overall = Hasher::new();
    let mut per_chunk = Vec::with_capacity(chunks.len());
    let mut offset = 0usize;
    let template_len = template.len();

    for chunk in chunks {
        let mut chunk_hasher = Hasher::new();
        let mut remaining = chunk.length;
        while remaining > 0 {
            let template_offset = offset % template_len;
            let take = remaining.min(template_len - template_offset);
            let slice = &template[template_offset..template_offset + take];
            chunk_hasher.update(slice);
            overall.update(slice);
            remaining -= take;
            offset += take;
        }
        per_chunk.push(chunk_hasher.finalize());
    }

    assert_eq!(offset, TOTAL_LEN, "replay covered unexpected length");
    (overall.finalize(), per_chunk)
}

#[test]
#[ignore = "expensive 1 GiB regression; run with `cargo test --test one_gib -- --ignored`"]
fn chunk_profile_default_handles_one_gib_stream() {
    let template = PROFILE.generate_input();
    assert_eq!(
        template.len(),
        1 << 20,
        "fixture must remain a 1 MiB corpus"
    );

    let chunks = collect_chunks(&template);
    assert!(!chunks.is_empty(), "chunker emitted no chunks");

    let profile = ChunkProfile::DEFAULT;
    for window in chunks.windows(2) {
        assert_eq!(
            window[0].offset + window[0].length,
            window[1].offset,
            "chunk offsets drifted"
        );
    }
    let first = chunks.first().expect("first chunk");
    assert_eq!(first.offset, 0);
    for (idx, chunk) in chunks.iter().enumerate() {
        let end = chunk.offset + chunk.length;
        assert!(
            chunk.length <= profile.max_size,
            "chunk {idx} exceeds max size: {} > {}",
            chunk.length,
            profile.max_size
        );
        if end < TOTAL_LEN {
            assert!(
                chunk.length >= profile.min_size,
                "chunk {idx} below min size: {} < {}",
                chunk.length,
                profile.min_size
            );
        }
    }
    let last = chunks.last().expect("last chunk");
    assert_eq!(
        last.offset + last.length,
        TOTAL_LEN,
        "final chunk end did not reach 1 GiB"
    );

    let (overall_digest, chunk_digests) = replay_chunks(&chunks, &template);
    let repeat = TOTAL_LEN / template.len();

    let chunk_dump_path = std::env::var("CARGO_BIN_EXE_sorafs_chunk_dump")
        .expect("sorafs_chunk_dump binary available during tests");
    let mut child = Command::new(&chunk_dump_path)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn sorafs-chunk-dump CLI");
    {
        let mut stdin = child.stdin.take().expect("stdin handle");
        for _ in 0..repeat {
            stdin
                .write_all(&template)
                .expect("write template slice to CLI");
        }
    }
    let output = child
        .wait_with_output()
        .expect("collect sorafs-chunk-dump output");
    assert!(
        output.status.success(),
        "sorafs-chunk-dump exited with {:?}",
        output.status.code()
    );
    let cli_json: Value =
        norito::json::from_slice(&output.stdout).expect("parse chunk dump JSON payload");
    assert_eq!(
        cli_json
            .get("input_length")
            .and_then(Value::as_u64)
            .expect("input_length") as usize,
        TOTAL_LEN,
        "CLI reported input length mismatch"
    );
    let cli_chunks = cli_json
        .get("chunks")
        .and_then(Value::as_array)
        .expect("chunks array from CLI");
    assert_eq!(
        cli_chunks.len(),
        chunks.len(),
        "CLI chunk count differs from Rust chunker"
    );
    let mut chunk_vec_from_cli = Vec::with_capacity(cli_chunks.len());
    let mut digests_from_cli = Vec::with_capacity(cli_chunks.len());
    for (idx, (entry, chunk)) in cli_chunks.iter().zip(chunks.iter()).enumerate() {
        let offset = entry
            .get("offset")
            .and_then(Value::as_u64)
            .expect("chunk offset") as usize;
        let length = entry
            .get("length")
            .and_then(Value::as_u64)
            .expect("chunk length") as usize;
        let digest_hex = entry
            .get("digest_blake3")
            .and_then(Value::as_str)
            .expect("chunk digest hex");
        let digest = Hash::from_hex(digest_hex).expect("valid blake3 digest");
        chunk_vec_from_cli.push(Chunk { offset, length });
        digests_from_cli.push(digest);

        assert_eq!(chunk.offset, offset, "chunk offset mismatch at index {idx}");
        assert_eq!(chunk.length, length, "chunk length mismatch at index {idx}");
    }
    assert_eq!(
        digests_from_cli, chunk_digests,
        "chunk digest list diverged between CLI and library"
    );
    let (cli_overall_digest, _) = replay_chunks(&chunk_vec_from_cli, &template);
    assert_eq!(
        cli_overall_digest, overall_digest,
        "overall digest mismatch between CLI and library"
    );

    let digest_hex = overall_digest.to_hex();
    assert_eq!(
        digest_hex.as_str(),
        "03200fc323fdc649ad7fc5a2d7949247db73a119de2da65971064a5e6b7e8a42",
        "overall digest changed; update fixtures/tests if intentional"
    );
    assert_eq!(
        chunk_digests.len(),
        chunks.len(),
        "chunk digest count mismatch"
    );
    assert!(
        chunk_digests.len() > 4000,
        "expected thousands of chunks for 1 GiB input"
    );
    let sample_indices = [0, chunk_digests.len() / 2, chunk_digests.len() - 1];
    let expected_samples = [
        "7789b490337d16c51b59a92e354a657ba450da4bab872c31e85e4d4fedcb3a27",
        "79f7fbd8c45dac13b1fa0be8ac366586f594b4ea7b079cc04814fabf43797bc4",
        "c35af9444dc6f7374909860b553433c222155d36d3d06c4398570d4a39e72d41",
    ];
    for (idx, expected) in sample_indices.into_iter().zip(expected_samples) {
        assert_eq!(
            chunk_digests[idx].to_hex().as_str(),
            expected,
            "chunk digest mismatch at index {idx}"
        );
    }
}
