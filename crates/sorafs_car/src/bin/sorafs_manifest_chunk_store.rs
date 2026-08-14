//! CLI helper for ingesting payloads with the canonical SoraFS chunk store.
fn main() {
    if let Err(error) = sorafs_car::chunk_store_cli::run_manifest_chunk_store() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
