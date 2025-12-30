use std::{
    env,
    io::{self, Write},
};

use blake3::{Hash, Hasher};
use sorafs_chunker::{Chunk, ChunkProfile, Chunker, fixtures::FixtureProfile};

const TOTAL_LEN: usize = 1 << 30; // 1 GiB

fn collect_chunks(template: &[u8]) -> Vec<Chunk> {
    let mut chunker = Chunker::new();
    let mut chunks = Vec::new();
    let repeat = TOTAL_LEN / template.len();
    if repeat * template.len() != TOTAL_LEN {
        panic!("template length does not evenly divide 1 GiB");
    }
    for _ in 0..repeat {
        chunker.feed(template, |chunk| chunks.push(chunk));
    }
    chunker.finish(|chunk| chunks.push(chunk));
    chunks
}

fn replay_chunks(chunks: &[Chunk], template: &[u8]) -> (Hash, Vec<Hash>) {
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

    if offset != TOTAL_LEN {
        panic!("replay covered {offset} bytes, expected {TOTAL_LEN}");
    }
    (overall.finalize(), per_chunk)
}

fn write_json(profile_handle: &str, chunks: &[Chunk], overall: &Hash, per_chunk: &[Hash]) {
    let stdout = io::stdout();
    let mut writer = io::BufWriter::new(stdout.lock());
    let sample_indices = match per_chunk.len() {
        0 => Vec::new(),
        1 => vec![0],
        _ => vec![0, per_chunk.len() / 2, per_chunk.len().saturating_sub(1)],
    };
    write!(
        writer,
        "{{\"profile\":\"{profile_handle}\",\"total_bytes\":{TOTAL_LEN},\"chunk_count\":{},\"overall_digest\":\"{}\",\"samples\":[",
        per_chunk.len(),
        overall.to_hex()
    )
    .expect("write report header");
    for (idx, sample) in sample_indices.iter().enumerate() {
        if idx > 0 {
            writer.write_all(b",").expect("write comma");
        }
        write!(
            writer,
            "{{\"index\":{},\"digest\":\"{}\"}}",
            sample,
            per_chunk[*sample].to_hex()
        )
        .expect("write sample");
    }
    writer
        .write_all(b"],\"chunk_lengths\":{")
        .expect("write lengths header");
    write!(
        writer,
        "\"min\":{},\"max\":{},\"profile_min\":{},\"profile_max\":{}",
        chunks.iter().map(|chunk| chunk.length).min().unwrap_or(0),
        chunks.iter().map(|chunk| chunk.length).max().unwrap_or(0),
        ChunkProfile::DEFAULT.min_size,
        ChunkProfile::DEFAULT.max_size
    )
    .expect("write length stats");
    writer.write_all(b"}}").expect("finish report");
    writer.flush().expect("flush report");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        eprintln!(
            "usage: {}   # no arguments; emits JSON with digest statistics",
            args[0]
        );
        std::process::exit(1);
    }
    let template = FixtureProfile::SF1_V1.generate_input();
    let chunks = collect_chunks(&template);
    let (overall_digest, per_chunk_digests) = replay_chunks(&chunks, &template);
    write_json(
        "sorafs.sf1@1.0.0",
        &chunks,
        &overall_digest,
        &per_chunk_digests,
    );
}
