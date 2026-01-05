//! Helpers to generate predecoder golden fixtures (JSON/bin) for tests and tools.
//!
//! Exposed so unit tests can generate fixtures on demand without shelling out to Cargo.

use std::{
    fs,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use crate::{ProgramMetadata, encoding, instruction, ivm_cache::IvmCache, kotodama::wide as kwide};

fn build_mixed_code() -> Vec<u8> {
    let mut code = Vec::new();
    // 1) Wide ADD r3 = r1 + r2
    code.extend_from_slice(&kwide::encode_add(3, 1, 2).to_le_bytes());
    // 2) Wide SUB r6 = r4 - r5
    code.extend_from_slice(&kwide::encode_sub(6, 4, 5).to_le_bytes());
    // 3) Wide XOR r9 = r7 ^ r8
    code.extend_from_slice(&kwide::encode_xor(9, 7, 8).to_le_bytes());
    // 4) Branch if r1 == r1 to skip the next instruction (offset = 2 words)
    let beq = kwide::encode_branch_checked(instruction::wide::control::BEQ, 1, 1, 2)
        .expect("branch offset fits in imm8");
    code.extend_from_slice(&beq.to_le_bytes());
    // 5) Filler instruction (will be skipped)
    code.extend_from_slice(&kwide::encode_add(15, 15, 0).to_le_bytes());
    // 6) HALT
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    code
}

fn json_object<I>(entries: I) -> norito::json::Value
where
    I: IntoIterator<Item = (String, norito::json::Value)>,
{
    let mut map = norito::json::Map::new();
    for (key, value) in entries {
        map.insert(key, value);
    }
    norito::json::Value::Object(map)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out {
        use std::fmt::Write as _;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

/// Generate the default mixed predecoder fixtures under the given root directory.
///
/// Layout (created if missing):
/// root/
///   code.bin
///   decoded.json
///   index.json
///   artifacts/
///     artifact_*.to
pub fn generate_predecoder_mixed_fixtures(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let artifacts_dir = root.join("artifacts");
    fs::create_dir_all(&artifacts_dir)?;

    // 1) Build code and decoded op list
    let code = build_mixed_code();
    let decoded = IvmCache::decode_stream(&code)?;

    // 2) Write code.bin
    fs::write(root.join("code.bin"), &code)?;

    // 3) Write decoded.json
    let decoded_items: Vec<_> = decoded
        .iter()
        .map(|op| {
            json_object([
                ("pc".to_owned(), norito::json::Value::from(op.pc)),
                ("len".to_owned(), norito::json::Value::from(op.len)),
                ("inst".to_owned(), norito::json::Value::from(op.inst)),
                (
                    "inst_hex".to_owned(),
                    norito::json::Value::from(format!("0x{inst:08x}", inst = op.inst)),
                ),
            ])
        })
        .collect();
    let decoded_doc = json_object([
        (
            "format".to_owned(),
            norito::json::Value::from("ivm.predecoder.v1"),
        ),
        (
            "code_sha256".to_owned(),
            norito::json::Value::from(sha256_hex(&code)),
        ),
        (
            "decoded".to_owned(),
            norito::json::Value::Array(decoded_items),
        ),
    ]);
    fs::write(
        root.join("decoded.json"),
        norito::json::to_json_pretty(&decoded_doc)?,
    )?;

    // 4) Header variants and artifacts
    let variants: Vec<ProgramMetadata> = vec![
        ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0x00,
            vector_length: 0,
            max_cycles: 0,
            abi_version: 1,
        },
        ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0x03,
            vector_length: 8,
            max_cycles: 1_000,
            abi_version: 1,
        },
    ];

    let mut index_entries = vec![];
    for m in variants {
        let fname = format!(
            "artifact_v{}_{}_mode{:02x}_vlen{}_cycles{}_abi{}.to",
            m.version_major, m.version_minor, m.mode, m.vector_length, m.max_cycles, m.abi_version
        );
        let mut artifact = m.encode();
        artifact.extend_from_slice(&code);
        fs::write(artifacts_dir.join(&fname), &artifact)?;
        let meta = json_object([
            (
                "version_major".to_owned(),
                norito::json::Value::from(m.version_major),
            ),
            (
                "version_minor".to_owned(),
                norito::json::Value::from(m.version_minor),
            ),
            ("mode".to_owned(), norito::json::Value::from(m.mode)),
            (
                "vector_length".to_owned(),
                norito::json::Value::from(m.vector_length),
            ),
            (
                "max_cycles".to_owned(),
                norito::json::Value::from(m.max_cycles),
            ),
            (
                "abi_version".to_owned(),
                norito::json::Value::from(m.abi_version),
            ),
        ]);
        index_entries.push(json_object([
            ("file".to_owned(), norito::json::Value::from(fname.clone())),
            ("meta".to_owned(), meta),
            (
                "artifact_sha256".to_owned(),
                norito::json::Value::from(sha256_hex(&artifact)),
            ),
        ]));
    }

    // 5) Write index.json summarizing artifacts
    let index_doc = json_object([
        (
            "format".to_owned(),
            norito::json::Value::from("ivm.predecoder.v1/index"),
        ),
        (
            "code_file".to_owned(),
            norito::json::Value::from("code.bin"),
        ),
        (
            "decoded_file".to_owned(),
            norito::json::Value::from("decoded.json"),
        ),
        (
            "artifacts_dir".to_owned(),
            norito::json::Value::from("artifacts"),
        ),
        (
            "artifacts".to_owned(),
            norito::json::Value::Array(index_entries),
        ),
    ]);
    fs::write(
        root.join("index.json"),
        norito::json::to_json_pretty(&index_doc)?,
    )?;

    Ok(())
}

/// Convenience: default fixtures path used by tests.
pub fn default_predecoder_mixed_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/predecoder/mixed")
}
