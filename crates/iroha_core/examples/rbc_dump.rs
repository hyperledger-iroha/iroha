//! Dump RBC session summaries persisted on disk.

use std::{env, path::Path};

use iroha_core::sumeragi::rbc_status;

fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: rbc_dump <rbc_sessions_dir>...");
        std::process::exit(1);
    }

    for path in args {
        let summaries = rbc_status::read_persisted_snapshot(Path::new(&path));
        println!("{path}: {} summaries", summaries.len());
        for summary in summaries {
            println!(
                "  height={} view={} delivered={} ready={} chunks={}/{} block_hash={:?} payload_hash={:?}",
                summary.height,
                summary.view,
                summary.delivered,
                summary.ready_count,
                summary.received_chunks,
                summary.total_chunks,
                summary.block_hash,
                summary.payload_hash
            );
        }
    }
}
