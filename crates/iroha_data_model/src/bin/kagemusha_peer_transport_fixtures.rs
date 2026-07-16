//! Generate canonical, serialization-derived Kagemusha peer-transport measurements.
//!
//! Run with `cargo run -p iroha_data_model --features test-fixtures --bin
//! kagemusha_peer_transport_fixtures -- --write`. Use `--check` in CI.

use std::{env, error::Error, fs, path::Path};

use iroha_data_model::offline::kagemusha_peer_transport_fixture_records_v1;
use sha2::{Digest as _, Sha256};

const FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/kagemusha/peer_transport_measurements_v1.json"
);
const GENERATOR_PATH: &str = "crates/iroha_data_model/src/bin/kagemusha_peer_transport_fixtures.rs";
const FACTORY_PATH: &str = "crates/iroha_data_model/src/offline/peer_transport_fixtures.rs";
const GENERATOR_SOURCE: &[u8] = include_bytes!("kagemusha_peer_transport_fixtures.rs");
const FACTORY_SOURCE: &[u8] = include_bytes!("../offline/peer_transport_fixtures.rs");
const PROOF_BYTES: usize = 4_096;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let (mode, fixture_path) = match arguments.as_slice() {
        [mode] if matches!(mode.as_str(), "--write" | "--check") => {
            (mode.as_str(), Path::new(FIXTURE_PATH))
        }
        [mode, path] if mode == "--check" => (mode.as_str(), Path::new(path)),
        _ => {
            return Err(
                "usage: kagemusha_peer_transport_fixtures --write | --check [fixture-path]".into(),
            );
        }
    };
    let rendered = render_fixture()?;
    if mode == "--check" {
        let existing = fs::read_to_string(fixture_path)?;
        if existing != rendered {
            return Err(format!(
                "fixture {} is stale; regenerate it with --write",
                fixture_path.display()
            )
            .into());
        }
    } else {
        if let Some(parent) = fixture_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(fixture_path, rendered)?;
    }
    Ok(())
}

fn render_fixture() -> Result<String, Box<dyn Error>> {
    let records = kagemusha_peer_transport_fixture_records_v1()?;
    let mut source_hasher = Sha256::new();
    source_hasher.update(GENERATOR_SOURCE);
    source_hasher.update([0]);
    source_hasher.update(FACTORY_SOURCE);
    let generator_sha256 = hex::encode(source_hasher.finalize());
    let mut output = String::new();
    output.push_str("{\n");
    output.push_str("  \"schema\": \"iroha.kagemusha.peer_transport_measurements.v1\",\n");
    output.push_str(&format!("  \"generator\": \"{GENERATOR_PATH}\",\n"));
    output.push_str(&format!(
        "  \"generator_dependency\": \"{FACTORY_PATH}\",\n"
    ));
    output.push_str(&format!(
        "  \"generator_sha256\": \"{generator_sha256}\",\n"
    ));
    output.push_str(&format!("  \"proof_bytes\": {PROOF_BYTES},\n"));
    output.push_str("  \"records\": [\n");
    for (index, record) in records.iter().enumerate() {
        let archive_sha256 = hex::encode(Sha256::digest(&record.archive));
        output.push_str("    {\n");
        output.push_str(&format!("      \"label\": \"{}\",\n", record.label));
        output.push_str(&format!("      \"kind\": \"{}\",\n", record.kind));
        output.push_str(&format!(
            "      \"branch_depth\": {},\n",
            record.branch_depth
        ));
        output.push_str(&format!("      \"peer_hops\": {},\n", record.peer_hops));
        output.push_str(&format!(
            "      \"archive_bytes\": {},\n",
            record.archive.len()
        ));
        output.push_str(&format!(
            "      \"archive_sha256\": \"{archive_sha256}\",\n"
        ));
        output.push_str(&format!(
            "      \"archive_hex\": \"{}\"\n",
            hex::encode(&record.archive)
        ));
        output.push_str(if index + 1 == records.len() {
            "    }\n"
        } else {
            "    },\n"
        });
    }
    output.push_str("  ]\n}\n");
    Ok(output)
}
