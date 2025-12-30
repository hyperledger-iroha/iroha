//! Debug helper for inspecting persisted RBC sessions.

use std::{env, path::Path};

use iroha_core::sumeragi::rbc_status;

fn main() {
    let mut args = env::args().skip(1);
    let Some(dir) = args.next() else {
        eprintln!("usage: rbc_probe <rbc_dir>");
        std::process::exit(1);
    };

    let dir = Path::new(&dir);
    let mut summaries = rbc_status::read_persisted_snapshot(dir);
    summaries.sort_by_key(|s| (s.height, s.view));
    println!("decoded {} sessions", summaries.len());
    for summary in summaries.iter().take(20) {
        let hash_short = hex::encode(summary.block_hash.as_ref().as_ref());
        let payload_hex = summary
            .payload_hash
            .as_ref()
            .map_or_else(|| "none".to_owned(), |hash| hex::encode(hash.as_ref()));
        println!(
            "{} height={} view={} total_chunks={} received={} ready={} delivered={} recovered={} invalid={} payload_hash={}",
            &hash_short[..8],
            summary.height,
            summary.view,
            summary.total_chunks,
            summary.received_chunks,
            summary.ready_count,
            summary.delivered,
            summary.recovered_from_disk,
            summary.invalid,
            payload_hex
        );
    }
}
