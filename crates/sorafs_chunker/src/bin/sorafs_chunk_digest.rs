use blake3::{Hash, Hasher};
use sorafs_chunker::{Chunk, ChunkProfile, Chunker, fixtures::FixtureProfile};
use std::{
    env,
    io::{self, Write},
    process,
};
const TOTAL_LEN: usize = 1 << 30; // 1 GiB
fn collect_chunks(template: &[u8]) -> Result<Vec<Chunk>, String> {
    if template.is_empty() {
        return Err("template must not be empty".to_owned());
    }
    let mut chunker = Chunker::new();
    let mut chunks = Vec::new();
    let repeat = TOTAL_LEN / template.len();
    if repeat * template.len() != TOTAL_LEN {
        return Err(format!(
            "template length {} does not evenly divide {TOTAL_LEN} bytes",
            template.len()
        ));
    }
    for _ in 0..repeat {
        chunker.feed(template, |chunk| chunks.push(chunk));
    }
    chunker.finish(|chunk| chunks.push(chunk));
    Ok(chunks)
}
fn replay_chunks(chunks: &[Chunk], template: &[u8]) -> Result<(Hash, Vec<Hash>), String> {
    if template.is_empty() {
        return Err("template must not be empty".to_owned());
    }
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
        return Err(format!(
            "replay covered {offset} bytes, expected {TOTAL_LEN}"
        ));
    }
    Ok((overall.finalize(), per_chunk))
}
fn write_json(
    profile_handle: &str,
    chunks: &[Chunk],
    overall: &Hash,
    per_chunk: &[Hash],
) -> io::Result<()> {
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
    )?;
    for (idx, sample) in sample_indices.iter().enumerate() {
        if idx > 0 {
            writer.write_all(b",")?;
        }
        write!(
            writer,
            "{{\"index\":{},\"digest\":\"{}\"}}",
            sample,
            per_chunk[*sample].to_hex()
        )?;
    }
    writer.write_all(b"],\"chunk_lengths\":{")?;
    write!(
        writer,
        "\"min\":{},\"max\":{},\"profile_min\":{},\"profile_max\":{}",
        chunks.iter().map(|chunk| chunk.length).min().unwrap_or(0),
        chunks.iter().map(|chunk| chunk.length).max().unwrap_or(0),
        ChunkProfile::DEFAULT.min_size,
        ChunkProfile::DEFAULT.max_size
    )?;
    writer.write_all(b"}}")?;
    writer.flush()
}
fn main() {
    if let Err(err) = run(env::args()) {
        eprintln!("error: {err}");
        process::exit(1);
    }
}
fn run<I>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let program = args
        .next()
        .unwrap_or_else(|| "sorafs_chunk_digest".to_owned());
    if args.next().is_some() {
        return Err(format!(
            "usage: {program}   # no arguments; emits JSON with digest statistics"
        ));
    }
    let template = FixtureProfile::SF1_V1.generate_input();
    let chunks = collect_chunks(&template)?;
    let (overall_digest, per_chunk_digests) = replay_chunks(&chunks, &template)?;
    write_json(
        "sorafs.sf1@1.0.0",
        &chunks,
        &overall_digest,
        &per_chunk_digests,
    )
    .map_err(|err| format!("failed to write digest report: {err}"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collect_chunks_rejects_empty_template() {
        let err = collect_chunks(&[]).expect_err("empty template must fail");
        assert!(
            err.contains("must not be empty"),
            "unexpected error message: {err}"
        );
    }
    #[test]
    fn collect_chunks_rejects_uneven_template() {
        let err = collect_chunks(b"abc").expect_err("uneven template must fail");
        assert!(
            err.contains("does not evenly divide"),
            "unexpected error message: {err}"
        );
    }
    #[test]
    fn replay_chunks_rejects_empty_template() {
        let err = replay_chunks(&[], &[]).expect_err("empty template must fail");
        assert!(
            err.contains("must not be empty"),
            "unexpected error message: {err}"
        );
    }
    #[test]
    fn replay_chunks_reports_length_mismatch() {
        let chunks = [Chunk {
            offset: 0,
            length: 4,
        }];
        let err = replay_chunks(&chunks, b"abcd").expect_err("short replay must fail");
        assert!(
            err.contains("replay covered 4 bytes"),
            "unexpected error message: {err}"
        );
    }
    #[test]
    fn run_rejects_extra_arguments() {
        let err = run(["sorafs_chunk_digest".to_owned(), "--unexpected".to_owned()])
            .expect_err("extra arguments must fail");
        assert!(err.contains("usage:"), "unexpected error message: {err}");
    }
}
