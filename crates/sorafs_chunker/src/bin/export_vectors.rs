use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use blake3::Hash;
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, PublicKey, Signature};
use norito::json::{self, Map, Value};
use sorafs_chunker::{
    ChunkProfile, Chunker,
    fixtures::{FixtureProfile, FixtureVectors, to_hex},
};

#[derive(Default)]
struct CliOptions {
    signing_key_hex: Option<String>,
    signer_hex: Option<String>,
    signature_out: Option<PathBuf>,
    allow_unsigned: bool,
}

enum CliError {
    Help,
    Message(String),
}

const USAGE: &str = "\
Usage: export_vectors [OPTIONS]

Options:
    --signing-key <hex>         Ed25519 private key (32- or 64-byte hex) for signing the manifest
    --signer <hex>              Expected public key (32-byte hex). Defaults to the key derived from --signing-key
    --signature-out <path>      Output path for council signature JSON (defaults to fixtures/sorafs_chunker/manifest_signatures.json)
    --allow-unsigned            Permit regeneration without signatures (local development only; CI MUST sign)
    -h, --help                  Show this help message
";

const CANONICAL_PROFILE_HANDLE: &str = "sorafs.sf1@1.0.0";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = match parse_cli() {
        Ok(cli) => cli,
        Err(CliError::Help) => {
            println!("{USAGE}");
            return Ok(());
        }
        Err(CliError::Message(err)) => {
            eprintln!("error: {err}");
            eprintln!("{USAGE}");
            std::process::exit(1);
        }
    };

    let repo_root = repo_root()?;
    let output_dir = repo_root.join("fixtures").join("sorafs_chunker");
    fs::create_dir_all(&output_dir)?;

    let vectors = FixtureProfile::SF1_V1.generate_vectors();

    write_json(&output_dir, &vectors)?;
    write_rust(&output_dir, &vectors)?;
    write_typescript(&output_dir, &vectors)?;
    write_go(&output_dir, &vectors)?;
    let manifest_digest = write_manifest(&output_dir, &vectors)?;
    write_manifest_signatures(&output_dir, &vectors, manifest_digest, &cli)?;
    write_fuzz_corpora(&repo_root, &vectors)?;

    println!("Generated fixtures in {}", output_dir.display());
    Ok(())
}

fn parse_cli() -> Result<CliOptions, CliError> {
    let mut options = CliOptions::default();
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Err(CliError::Help),
            value if value.starts_with("--signing-key=") => {
                let cleaned = clean_hex(&value["--signing-key=".len()..], "--signing-key")?;
                set_option(&mut options.signing_key_hex, cleaned, "--signing-key")?;
            }
            "--signing-key" => {
                let raw = take_value(&mut args, "--signing-key")?;
                let cleaned = clean_hex(&raw, "--signing-key")?;
                set_option(&mut options.signing_key_hex, cleaned, "--signing-key")?;
            }
            value if value.starts_with("--signer=") => {
                let cleaned = clean_hex(&value["--signer=".len()..], "--signer")?;
                validate_signer_len(&cleaned)?;
                set_option(&mut options.signer_hex, cleaned, "--signer")?;
            }
            "--signer" => {
                let raw = take_value(&mut args, "--signer")?;
                let cleaned = clean_hex(&raw, "--signer")?;
                validate_signer_len(&cleaned)?;
                set_option(&mut options.signer_hex, cleaned, "--signer")?;
            }
            value if value.starts_with("--signature-out=") => {
                let path = PathBuf::from(&value["--signature-out=".len()..]);
                set_option(&mut options.signature_out, path, "--signature-out")?;
            }
            "--signature-out" => {
                let raw = take_value(&mut args, "--signature-out")?;
                set_option(
                    &mut options.signature_out,
                    PathBuf::from(raw),
                    "--signature-out",
                )?;
            }
            "--allow-unsigned" => {
                options.allow_unsigned = true;
            }
            other => return Err(CliError::Message(format!("unknown argument {other}"))),
        }
    }

    if options.signer_hex.is_some() && options.signing_key_hex.is_none() {
        return Err(CliError::Message(
            "--signer requires --signing-key".to_owned(),
        ));
    }

    Ok(options)
}

fn take_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, CliError> {
    args.next()
        .ok_or_else(|| CliError::Message(format!("{flag} requires a value")))
}

fn set_option<T>(slot: &mut Option<T>, value: T, flag: &str) -> Result<(), CliError> {
    if slot.is_some() {
        return Err(CliError::Message(format!(
            "{flag} specified multiple times"
        )));
    }
    *slot = Some(value);
    Ok(())
}

fn clean_hex(value: &str, flag: &str) -> Result<String, CliError> {
    let trimmed = value.trim();
    let trimmed = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    if trimmed.is_empty() {
        return Err(CliError::Message(format!(
            "{flag} requires a non-empty hex value"
        )));
    }
    if !trimmed.len().is_multiple_of(2) {
        return Err(CliError::Message(format!(
            "{flag} value must contain an even number of hex digits"
        )));
    }
    if !trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(CliError::Message(format!(
            "{flag} value must be hex-encoded"
        )));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn validate_signer_len(cleaned: &str) -> Result<(), CliError> {
    if cleaned.len() != 64 {
        return Err(CliError::Message(
            "--signer must be a 32-byte hex string (64 hex characters)".to_owned(),
        ));
    }
    Ok(())
}

fn repo_root() -> Result<PathBuf, std::io::Error> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("..").join("..").canonicalize()
}

fn write_json(dir: &Path, vectors: &FixtureVectors) -> Result<(), Box<dyn std::error::Error>> {
    let mut prng = Map::new();
    prng.insert(
        "multiplier".to_owned(),
        Value::from(vectors.prng.multiplier),
    );
    prng.insert("increment".to_owned(), Value::from(vectors.prng.increment));

    let chunk_lengths = Value::Array(
        vectors
            .chunk_lengths
            .iter()
            .map(|len| Value::from(*len as u64))
            .collect(),
    );
    let chunk_offsets = Value::Array(
        vectors
            .chunk_offsets
            .iter()
            .map(|offset| Value::from(*offset as u64))
            .collect(),
    );
    let chunk_digests = Value::Array(
        vectors
            .blake3_digest_hexes()
            .into_iter()
            .map(Value::from)
            .collect(),
    );

    let mut root = Map::new();
    let profile_aliases = vec![Value::from(CANONICAL_PROFILE_HANDLE)];
    root.insert("profile".to_owned(), Value::from(CANONICAL_PROFILE_HANDLE));
    root.insert("profile_aliases".to_owned(), Value::Array(profile_aliases));
    root.insert("input_seed".to_owned(), Value::from(vectors.input_seed_hex));
    root.insert(
        "input_length".to_owned(),
        Value::from(vectors.input_length as u64),
    );
    root.insert("prng".to_owned(), Value::Object(prng));
    root.insert(
        "chunk_count".to_owned(),
        Value::from(vectors.chunk_count() as u64),
    );
    root.insert("chunk_lengths".to_owned(), chunk_lengths);
    root.insert("chunk_offsets".to_owned(), chunk_offsets);
    root.insert(
        "chunk_digest_sha3_256".to_owned(),
        Value::from(vectors.sha3_digest_hex()),
    );
    root.insert("chunk_digests_blake3".to_owned(), chunk_digests);

    let json_bytes = json::to_vec_pretty(&Value::Object(root))?;
    fs::write(dir.join("sf1_profile_v1.json"), json_bytes)?;
    Ok(())
}

fn write_rust(dir: &Path, vectors: &FixtureVectors) -> Result<(), std::io::Error> {
    let mut file = fs::File::create(dir.join("sf1_profile_v1.rs"))?;
    writeln!(
        file,
        "// @generated by `cargo run -p sorafs_chunker --bin export_vectors`\n\
         // Canonical fixture constants for the SoraFS chunker."
    )?;
    writeln!(
        file,
        "pub const PROFILE: &str = \"{CANONICAL_PROFILE_HANDLE}\";"
    )?;
    writeln!(
        file,
        "pub const PROFILE_ALIASES: &[&str] = &[\"{CANONICAL_PROFILE_HANDLE}\"];"
    )?;
    writeln!(
        file,
        "pub const INPUT_SEED: &str = \"{}\";",
        vectors.input_seed_hex
    )?;
    writeln!(
        file,
        "pub const INPUT_LENGTH: usize = {};",
        vectors.input_length
    )?;
    writeln!(
        file,
        "pub const PRNG_MULTIPLIER: u64 = {}u64;",
        vectors.prng.multiplier
    )?;
    writeln!(
        file,
        "pub const PRNG_INCREMENT: u64 = {}u64;",
        vectors.prng.increment
    )?;
    writeln!(
        file,
        "pub const CHUNK_COUNT: usize = {};",
        vectors.chunk_count()
    )?;
    write_array(&mut file, "CHUNK_LENGTHS", "usize", &vectors.chunk_lengths)?;
    write_array(&mut file, "CHUNK_OFFSETS", "usize", &vectors.chunk_offsets)?;
    writeln!(
        file,
        "pub const CHUNK_DIGEST_SHA3_256: &str = \"{}\";",
        vectors.sha3_digest_hex()
    )?;
    write_str_array(
        &mut file,
        "CHUNK_DIGESTS_BLAKE3",
        &vectors.blake3_digest_hexes(),
    )?;
    Ok(())
}

fn write_typescript(dir: &Path, vectors: &FixtureVectors) -> Result<(), std::io::Error> {
    let mut file = fs::File::create(dir.join("sf1_profile_v1.ts"))?;
    writeln!(
        file,
        "// @generated by `cargo run -p sorafs_chunker --bin export_vectors`\n\
         // Canonical fixture constants for the SoraFS chunker.\n"
    )?;
    writeln!(
        file,
        "export interface ChunkerFixture {{\n    profile: string;\n    inputSeed: string;\n    inputLength: number;\n    prngMultiplier: string;\n    prngIncrement: string;\n    chunkCount: number;\n    chunkLengths: readonly number[];\n    chunkOffsets: readonly number[];\n    chunkDigestSha3_256: string;\n    chunkDigestsBlake3: readonly string[];\n}}\n"
    )?;
    writeln!(file, "export const sf1ProfileV1: ChunkerFixture = {{")?;
    writeln!(file, "    profile: \"{CANONICAL_PROFILE_HANDLE}\",")?;
    writeln!(
        file,
        "    profileAliases: [\"{CANONICAL_PROFILE_HANDLE}\"] as const,"
    )?;
    writeln!(file, "    inputSeed: \"{}\",", vectors.input_seed_hex)?;
    writeln!(file, "    inputLength: {},", vectors.input_length)?;
    writeln!(file, "    prngMultiplier: \"{}\",", vectors.prng.multiplier)?;
    writeln!(file, "    prngIncrement: \"{}\",", vectors.prng.increment)?;
    writeln!(file, "    chunkCount: {},", vectors.chunk_count())?;
    write_ts_number_array(&mut file, "chunkLengths", &vectors.chunk_lengths)?;
    write_ts_number_array(&mut file, "chunkOffsets", &vectors.chunk_offsets)?;
    writeln!(
        file,
        "    chunkDigestSha3_256: \"{}\",",
        vectors.sha3_digest_hex()
    )?;
    write_ts_string_array(
        &mut file,
        "chunkDigestsBlake3",
        &vectors.blake3_digest_hexes(),
    )?;
    writeln!(file, "}} as const;")?;
    Ok(())
}

fn write_go(dir: &Path, vectors: &FixtureVectors) -> Result<(), std::io::Error> {
    let mut file = fs::File::create(dir.join("sf1_profile_v1.go"))?;
    writeln!(
        file,
        "// Code generated by `cargo run -p sorafs_chunker --bin export_vectors`; DO NOT EDIT.\n\
         package sorafsfixtures\n"
    )?;
    writeln!(
        file,
        "type ChunkerFixture struct {{\n    Profile string\n    ProfileAliases []string\n    InputSeed string\n    InputLength int\n    PRNGMultiplier uint64\n    PRNGIncrement uint64\n    ChunkCount int\n    ChunkLengths []int\n    ChunkOffsets []int\n    ChunkDigestSHA3_256 string\n    ChunkDigestsBLAKE3 []string\n}}\n"
    )?;
    writeln!(file, "var SF1ProfileV1 = ChunkerFixture{{")?;
    writeln!(file, "    Profile: \"{CANONICAL_PROFILE_HANDLE}\",")?;
    writeln!(
        file,
        "    ProfileAliases: []string{{\"{CANONICAL_PROFILE_HANDLE}\"}},"
    )?;
    writeln!(file, "    InputSeed: \"{}\",", vectors.input_seed_hex)?;
    writeln!(file, "    InputLength: {},", vectors.input_length)?;
    writeln!(file, "    PRNGMultiplier: {},", vectors.prng.multiplier)?;
    writeln!(file, "    PRNGIncrement: {},", vectors.prng.increment)?;
    writeln!(file, "    ChunkCount: {},", vectors.chunk_count())?;
    write_go_int_slice(&mut file, "ChunkLengths", &vectors.chunk_lengths)?;
    write_go_int_slice(&mut file, "ChunkOffsets", &vectors.chunk_offsets)?;
    writeln!(
        file,
        "    ChunkDigestSHA3_256: \"{}\",",
        vectors.sha3_digest_hex()
    )?;
    write_go_string_slice(
        &mut file,
        "ChunkDigestsBLAKE3",
        &vectors.blake3_digest_hexes(),
    )?;
    writeln!(file, "}}")?;
    Ok(())
}

fn write_manifest(
    dir: &Path,
    vectors: &FixtureVectors,
) -> Result<Hash, Box<dyn std::error::Error>> {
    let files = [
        "sf1_profile_v1.json",
        "sf1_profile_v1.rs",
        "sf1_profile_v1.ts",
        "sf1_profile_v1.go",
    ];
    let mut entries = Vec::with_capacity(files.len());
    for &name in &files {
        let path = dir.join(name);
        let bytes = fs::read(&path)?;
        let digest = to_hex(blake3::hash(&bytes).as_bytes());
        let mut entry = Map::new();
        entry.insert("file".to_owned(), Value::from(name));
        entry.insert("size".to_owned(), Value::from(bytes.len() as u64));
        entry.insert("blake3".to_owned(), Value::from(digest));
        entries.push(Value::Object(entry));
    }
    let mut root = Map::new();
    let profile_aliases = vec![Value::from(CANONICAL_PROFILE_HANDLE)];
    root.insert("profile".to_owned(), Value::from(CANONICAL_PROFILE_HANDLE));
    root.insert("profile_aliases".to_owned(), Value::Array(profile_aliases));
    root.insert(
        "chunk_digest_sha3_256".to_owned(),
        Value::from(vectors.sha3_digest_hex()),
    );
    root.insert("files".to_owned(), Value::Array(entries));

    let bytes = json::to_vec_pretty(&Value::Object(root))?;
    let digest = blake3::hash(&bytes);
    fs::write(dir.join("manifest_blake3.json"), bytes)?;
    Ok(digest)
}

fn write_manifest_signatures(
    dir: &Path,
    vectors: &FixtureVectors,
    manifest_digest: Hash,
    cli: &CliOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let out_path = cli
        .signature_out
        .clone()
        .unwrap_or_else(|| dir.join("manifest_signatures.json"));
    let existing_root = load_existing_manifest_signatures(&out_path, vectors, &manifest_digest)?;

    if cli.signing_key_hex.is_none() {
        match existing_root.as_ref() {
            Some(root) => {
                if let Err(err) = ensure_signed(root, &manifest_digest) {
                    if cli.allow_unsigned {
                        eprintln!(
                            "warning: {err}; continuing due to --allow-unsigned (NOT FOR CI)"
                        );
                        return Ok(());
                    }
                    return Err(err);
                }
                return Ok(());
            }
            None => {
                if cli.allow_unsigned {
                    eprintln!(
                        "warning: manifest signatures missing; continuing due to --allow-unsigned \
                         (NOT FOR CI)"
                    );
                    return Ok(());
                }
                return Err("manifest_signatures.json missing; provide --signing-key or explicitly allow unsigned output".into());
            }
        }
    }

    let signing_key_hex = cli
        .signing_key_hex
        .as_ref()
        .expect("signing key checked above");

    let private_key = PrivateKey::from_hex(Algorithm::Ed25519, signing_key_hex)
        .map_err(|err| format!("failed to parse --signing-key: {err}"))?;
    let key_pair = KeyPair::from_private_key(private_key)
        .map_err(|err| format!("failed to derive public key from --signing-key: {err}"))?;
    let public_key = key_pair.public_key();
    let (algorithm, public_bytes) = public_key.to_bytes();
    if algorithm != Algorithm::Ed25519 {
        return Err("signing key must use the Ed25519 algorithm".into());
    }
    let derived_signer_hex = to_hex(public_bytes);
    if cli
        .signer_hex
        .as_ref()
        .is_some_and(|expected| expected != &derived_signer_hex)
    {
        return Err(format!(
            "--signer does not match the public key derived from --signing-key (expected {derived_signer_hex})"
        )
        .into());
    }

    let signature = Signature::new(key_pair.private_key(), manifest_digest.as_bytes());
    let signature_hex = to_hex(signature.payload());
    let manifest_digest_hex = to_hex(manifest_digest.as_bytes());

    let mut entry = Map::new();
    entry.insert("algorithm".to_owned(), Value::from("ed25519"));
    entry.insert("signer".to_owned(), Value::from(derived_signer_hex.clone()));
    entry.insert("signature".to_owned(), Value::from(signature_hex));
    entry.insert(
        "signer_multihash".to_owned(),
        Value::from(public_key.to_string()),
    );

    if let Some(parent) = out_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let mut root = existing_root.unwrap_or_default();

    let mut signatures = match root.remove("signatures") {
        Some(Value::Array(items)) => items,
        Some(_) => {
            return Err("manifest signatures field must be an array"
                .to_owned()
                .into());
        }
        None => Vec::new(),
    };

    let entry_value = Value::Object(entry.clone());
    let mut replaced = false;
    for existing in signatures.iter_mut() {
        if signature_signer(existing)? == derived_signer_hex {
            if *existing != entry_value {
                *existing = entry_value.clone();
            }
            replaced = true;
            break;
        }
    }
    if !replaced {
        signatures.push(entry_value);
    }

    let mut signatures_with_keys = Vec::with_capacity(signatures.len());
    for value in signatures {
        let signer = signature_signer(&value)?.to_owned();
        signatures_with_keys.push((signer, value));
    }
    signatures_with_keys.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
    let signatures: Vec<Value> = signatures_with_keys
        .into_iter()
        .map(|(_, value)| value)
        .collect();

    verify_signatures(&signatures, &manifest_digest)?;

    let profile_aliases = vec![Value::from(CANONICAL_PROFILE_HANDLE)];
    root.insert("profile".to_owned(), Value::from(CANONICAL_PROFILE_HANDLE));
    root.insert("profile_aliases".to_owned(), Value::Array(profile_aliases));
    root.insert("manifest".to_owned(), Value::from("manifest_blake3.json"));
    root.insert(
        "manifest_blake3".to_owned(),
        Value::from(manifest_digest_hex),
    );
    root.insert(
        "chunk_digest_sha3_256".to_owned(),
        Value::from(vectors.sha3_digest_hex()),
    );
    root.insert("signatures".to_owned(), Value::Array(signatures.clone()));

    ensure_signed(&root, &manifest_digest)?;

    let bytes = json::to_vec_pretty(&Value::Object(root))?;
    fs::write(out_path, bytes)?;
    Ok(())
}

fn signature_signer(value: &Value) -> Result<&str, Box<dyn std::error::Error>> {
    let signer = value
        .as_object()
        .and_then(|map| map.get("signer"))
        .and_then(Value::as_str)
        .ok_or_else(|| "signature entries must include a signer field".to_owned())?;
    Ok(signer)
}

type JsonMap = Map;

fn load_existing_manifest_signatures(
    path: &Path,
    vectors: &FixtureVectors,
    manifest_digest: &Hash,
) -> Result<Option<JsonMap>, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(None);
    }
    let existing = json::from_slice(&fs::read(path)?)
        .map_err(|err| format!("failed to parse existing manifest signatures file: {err}"))?;
    let root = match existing {
        Value::Object(map) => map,
        _ => return Err("manifest signatures file must contain a JSON object".into()),
    };

    let profile = root
        .get("profile")
        .and_then(Value::as_str)
        .ok_or_else(|| "manifest signatures missing profile field".to_owned())?;
    let mut profile_ok = profile == CANONICAL_PROFILE_HANDLE;
    if !profile_ok && let Some(aliases) = root.get("profile_aliases").and_then(Value::as_array) {
        profile_ok = aliases
            .iter()
            .filter_map(Value::as_str)
            .any(|alias| alias == CANONICAL_PROFILE_HANDLE);
    }
    if !profile_ok {
        return Err(
            format!(
                "existing manifest signatures profile {profile} mismatches expected canonical handle {CANONICAL_PROFILE_HANDLE}"
            )
            .into(),
        );
    }

    match root.get("manifest").and_then(Value::as_str) {
        Some("manifest_blake3.json") => {}
        Some(other) => {
            return Err(format!(
                "existing manifest signatures references unexpected manifest {other}"
            )
            .into());
        }
        None => return Err("manifest signatures missing manifest field".into()),
    }

    let manifest_digest_hex = to_hex(manifest_digest.as_bytes());
    match root.get("manifest_blake3").and_then(Value::as_str) {
        Some(digest) if digest == manifest_digest_hex => {}
        Some(_) => {
            return Err(
                "existing manifest signatures digest mismatches regenerated manifest"
                    .to_owned()
                    .into(),
            );
        }
        None => return Err("manifest signatures missing manifest_blake3 field".into()),
    }

    match root.get("chunk_digest_sha3_256").and_then(Value::as_str) {
        Some(chunk_digest) if chunk_digest == vectors.sha3_digest_hex() => {}
        Some(_) => {
            return Err(
                "existing manifest signatures chunk digest mismatches regenerated vectors"
                    .to_owned()
                    .into(),
            );
        }
        None => return Err("manifest signatures missing chunk_digest_sha3_256 field".into()),
    }

    Ok(Some(root))
}

fn extract_signatures(map: &JsonMap) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    match map.get("signatures") {
        Some(Value::Array(items)) => Ok(items.clone()),
        Some(_) => Err("manifest signatures field must be an array".into()),
        None => Err("manifest signatures missing signatures array".into()),
    }
}

fn ensure_signed(map: &JsonMap, manifest_digest: &Hash) -> Result<(), Box<dyn std::error::Error>> {
    let signatures = extract_signatures(map)?;
    if signatures.is_empty() {
        return Err("manifest signatures file contains no council signatures".into());
    }
    verify_signatures(&signatures, manifest_digest)?;
    Ok(())
}

fn verify_signatures(
    signatures: &[Value],
    manifest_digest: &Hash,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in signatures {
        let map = entry
            .as_object()
            .ok_or_else(|| "signature entry must be an object".to_owned())?;

        let algorithm = map
            .get("algorithm")
            .and_then(Value::as_str)
            .ok_or_else(|| "signature entry missing algorithm".to_owned())?;
        if algorithm != "ed25519" {
            return Err(format!("unsupported signature algorithm {algorithm}").into());
        }

        let signer_hex = map
            .get("signer")
            .and_then(Value::as_str)
            .ok_or_else(|| "signature entry missing signer".to_owned())?;
        let signature_hex = map
            .get("signature")
            .and_then(Value::as_str)
            .ok_or_else(|| "signature entry missing signature".to_owned())?;

        let signer_bytes =
            hex::decode(signer_hex).map_err(|err| format!("failed to decode signer hex: {err}"))?;
        let signature_bytes = hex::decode(signature_hex)
            .map_err(|err| format!("failed to decode signature hex: {err}"))?;

        let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &signer_bytes)
            .map_err(|err| format!("invalid signer public key: {err}"))?;

        let expected_multihash = public_key.to_string();
        if let Some(multihash) = map.get("signer_multihash").and_then(Value::as_str)
            && multihash != expected_multihash.as_str()
        {
            return Err("signer_multihash does not match encoded public key".into());
        }

        let signature = Signature::from_bytes(&signature_bytes);
        signature
            .verify(&public_key, manifest_digest.as_bytes())
            .map_err(|err| format!("signature verification failed: {err}"))?;
    }
    Ok(())
}

fn write_fuzz_corpora(
    repo_root: &Path,
    vectors: &FixtureVectors,
) -> Result<(), Box<dyn std::error::Error>> {
    let fuzz_dir = repo_root.join("fuzz").join("sorafs_chunker");
    fs::create_dir_all(&fuzz_dir)?;

    // Raw input corpus for fuzzers/back-pressure harnesses.
    fs::write(fuzz_dir.join("sf1_profile_v1_input.bin"), &vectors.input)?;

    let scenarios = generate_backpressure_scenarios(vectors);
    let mut scenario_array = Vec::with_capacity(scenarios.len());
    for scenario in scenarios {
        let mut map = Map::new();
        map.insert("name".to_owned(), Value::from(scenario.name));
        map.insert(
            "feed_sizes".to_owned(),
            Value::Array(
                scenario
                    .feed_sizes
                    .iter()
                    .map(|&size| Value::from(size as u64))
                    .collect(),
            ),
        );
        map.insert(
            "expected_chunk_lengths".to_owned(),
            Value::Array(
                scenario
                    .expected_chunk_lengths
                    .iter()
                    .map(|&len| Value::from(len as u64))
                    .collect(),
            ),
        );
        map.insert(
            "chunk_count".to_owned(),
            Value::from(scenario.expected_chunk_lengths.len() as u64),
        );
        map.insert(
            "max_feed_size".to_owned(),
            Value::from(scenario.max_feed_size as u64),
        );
        map.insert(
            "min_feed_size".to_owned(),
            Value::from(scenario.min_feed_size as u64),
        );
        scenario_array.push(Value::Object(map));
    }

    let mut root = Map::new();
    let profile_aliases = vec![Value::from(CANONICAL_PROFILE_HANDLE)];
    root.insert("profile".to_owned(), Value::from(CANONICAL_PROFILE_HANDLE));
    root.insert("profile_aliases".to_owned(), Value::Array(profile_aliases));
    root.insert(
        "input_length".to_owned(),
        Value::from(vectors.input_length as u64),
    );
    root.insert(
        "chunk_digest_sha3_256".to_owned(),
        Value::from(vectors.sha3_digest_hex()),
    );
    root.insert("scenarios".to_owned(), Value::Array(scenario_array));

    let bytes = json::to_vec_pretty(&Value::Object(root))?;
    fs::write(fuzz_dir.join("sf1_profile_v1_backpressure.json"), bytes)?;

    Ok(())
}

struct BackpressureScenario {
    name: &'static str,
    feed_sizes: Vec<usize>,
    expected_chunk_lengths: Vec<usize>,
    max_feed_size: usize,
    min_feed_size: usize,
}

fn generate_backpressure_scenarios(vectors: &FixtureVectors) -> Vec<BackpressureScenario> {
    let total = vectors.input.len();
    let profile = vectors.chunk_profile;

    let patterns: [(&str, Vec<usize>); 3] = [
        ("uniform_4k", vec![4 * 1024]),
        (
            "burst_64k",
            vec![64 * 1024, 3 * 1024, 8 * 1024, 2 * 1024, 48 * 1024],
        ),
        (
            "jitter_prime",
            vec![
                1_537, 3_073, 5_003, 7_013, 11_143, 17_147, 19_313, 23_477, 29_597, 31_337,
            ],
        ),
    ];

    patterns
        .iter()
        .map(|(name, pattern)| {
            let feed_sizes = partition_input(total, pattern);
            let expected_chunk_lengths =
                capture_stream_chunks(profile, &vectors.input, &feed_sizes);
            let max_feed_size = feed_sizes.iter().copied().max().unwrap_or(0);
            let min_feed_size = feed_sizes.iter().copied().min().unwrap_or(0);
            BackpressureScenario {
                name,
                feed_sizes,
                expected_chunk_lengths,
                max_feed_size,
                min_feed_size,
            }
        })
        .collect()
}

fn partition_input(total: usize, pattern: &[usize]) -> Vec<usize> {
    assert!(!pattern.is_empty(), "partition pattern may not be empty");
    let mut feed = Vec::with_capacity(total / pattern.iter().sum::<usize>().max(1) + 1);
    let mut remaining = total;
    let mut idx = 0;
    while remaining > 0 {
        let candidate = pattern[idx % pattern.len()];
        let size = candidate.min(remaining);
        if size == 0 {
            break;
        }
        feed.push(size);
        remaining -= size;
        idx += 1;
    }
    if remaining > 0 {
        feed.push(remaining);
    }
    feed
}

fn capture_stream_chunks(profile: ChunkProfile, input: &[u8], feed_sizes: &[usize]) -> Vec<usize> {
    let mut chunker = Chunker::with_profile(profile);
    let mut emitted = Vec::new();
    let mut offset = 0usize;
    for &feed in feed_sizes {
        let end = (offset + feed).min(input.len());
        let slice = &input[offset..end];
        chunker.feed(slice, |chunk| emitted.push(chunk.length));
        offset = end;
    }
    chunker.finish(|chunk| emitted.push(chunk.length));
    // If finish emitted the sentinel zero-length chunk (only when no data),
    // filter it out to keep parity with batch chunking fixtures.
    if input.is_empty() {
        emitted
    } else {
        emitted.retain(|len| *len > 0);
        emitted
    }
}

fn write_array<T: std::fmt::Display>(
    file: &mut fs::File,
    name: &str,
    ty: &str,
    values: &[T],
) -> Result<(), std::io::Error> {
    writeln!(file, "pub const {name}: [{ty}; {}] = [", values.len())?;
    for value in values {
        writeln!(file, "    {value},")?;
    }
    writeln!(file, "];")?;
    Ok(())
}

fn write_str_array(
    file: &mut fs::File,
    name: &str,
    values: &[String],
) -> Result<(), std::io::Error> {
    writeln!(file, "pub const {name}: [&str; {}] = [", values.len())?;
    for value in values {
        writeln!(file, "    \"{value}\",")?;
    }
    writeln!(file, "];")?;
    Ok(())
}

fn write_ts_number_array(
    file: &mut fs::File,
    name: &str,
    values: &[usize],
) -> Result<(), std::io::Error> {
    writeln!(file, "    {name}: [")?;
    for value in values {
        writeln!(file, "        {value},")?;
    }
    writeln!(file, "    ] as const,")?;
    Ok(())
}

fn write_ts_string_array(
    file: &mut fs::File,
    name: &str,
    values: &[String],
) -> Result<(), std::io::Error> {
    writeln!(file, "    {name}: [")?;
    for value in values {
        writeln!(file, "        \"{value}\",")?;
    }
    writeln!(file, "    ] as const,")?;
    Ok(())
}

fn write_go_int_slice(
    file: &mut fs::File,
    name: &str,
    values: &[usize],
) -> Result<(), std::io::Error> {
    writeln!(file, "    {name}: []int{{")?;
    for value in values {
        writeln!(file, "        {value},")?;
    }
    writeln!(file, "    }},")?;
    Ok(())
}

fn write_go_string_slice(
    file: &mut fs::File,
    name: &str,
    values: &[String],
) -> Result<(), std::io::Error> {
    writeln!(file, "    {name}: []string{{")?;
    for value in values {
        writeln!(file, "        \"{value}\",")?;
    }
    writeln!(file, "    }},")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
    use sorafs_chunker::fixtures::{FixtureProfile, FixtureVectors, to_hex};

    use super::*;

    const SIGNING_KEY_1: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    const SIGNING_KEY_2: &str = "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f";

    fn temp_dir() -> PathBuf {
        let mut dir = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        dir.push(format!(
            "sorafs_chunker_test_{:x}_{}",
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn prepare_fixture_files(dir: &Path, vectors: &FixtureVectors) {
        write_json(dir, vectors).expect("write json fixture");
        write_rust(dir, vectors).expect("write rust fixture");
        write_typescript(dir, vectors).expect("write ts fixture");
        write_go(dir, vectors).expect("write go fixture");
    }

    fn derive_public_hex(secret_hex: &str) -> String {
        let private_key =
            PrivateKey::from_hex(Algorithm::Ed25519, secret_hex).expect("valid private key");
        let key_pair = KeyPair::from_private_key(private_key).expect("derive public key");
        let (_, public_bytes) = key_pair.public_key().to_bytes();
        to_hex(public_bytes)
    }

    fn read_signers(path: &Path) -> Vec<String> {
        let bytes = fs::read(path).expect("read manifest signatures");
        let value: Value = json::from_slice(&bytes).expect("parse manifest signatures json");
        let signatures = value
            .get("signatures")
            .and_then(Value::as_array)
            .expect("signatures array");
        signatures
            .iter()
            .map(|entry| {
                entry
                    .get("signer")
                    .and_then(Value::as_str)
                    .expect("signer field")
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn manifest_signatures_merge_without_duplicates() {
        let dir = temp_dir();
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        prepare_fixture_files(&dir, &vectors);
        let manifest_digest = write_manifest(&dir, &vectors).expect("write manifest");

        let cli = CliOptions {
            signing_key_hex: Some(SIGNING_KEY_1.to_owned()),
            signature_out: Some(dir.join("signatures.json")),
            ..CliOptions::default()
        };

        write_manifest_signatures(&dir, &vectors, manifest_digest, &cli)
            .expect("initial signature write");
        let out_path = cli.signature_out.as_ref().expect("signature output path");
        let first_signer = derive_public_hex(cli.signing_key_hex.as_deref().unwrap());
        assert_eq!(read_signers(out_path), vec![first_signer.clone()]);

        // Re-sign with the same key; should not create duplicates.
        write_manifest_signatures(&dir, &vectors, manifest_digest, &cli)
            .expect("idempotent signature write");
        assert_eq!(read_signers(out_path), vec![first_signer.clone()]);

        let cli_second = CliOptions {
            signing_key_hex: Some(SIGNING_KEY_2.to_owned()),
            signature_out: Some(out_path.clone()),
            ..CliOptions::default()
        };
        let second_signer = derive_public_hex(cli_second.signing_key_hex.as_deref().unwrap());

        write_manifest_signatures(&dir, &vectors, manifest_digest, &cli_second)
            .expect("append second signature");
        let mut expected = vec![first_signer, second_signer];
        expected.sort();
        assert_eq!(read_signers(out_path), expected);

        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }

    #[test]
    fn unsigned_regeneration_without_signatures_is_rejected() {
        let dir = temp_dir();
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        prepare_fixture_files(&dir, &vectors);
        let manifest_digest = write_manifest(&dir, &vectors).expect("write manifest");

        let cli = CliOptions {
            signature_out: Some(dir.join("manifest_signatures.json")),
            ..CliOptions::default()
        };

        let err = write_manifest_signatures(&dir, &vectors, manifest_digest, &cli)
            .expect_err("missing signatures must fail");
        assert!(
            err.to_string().contains("manifest_signatures.json missing"),
            "unexpected error: {err}"
        );

        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }

    #[test]
    fn allow_unsigned_flag_skips_enforcement() {
        let dir = temp_dir();
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        prepare_fixture_files(&dir, &vectors);
        let manifest_digest = write_manifest(&dir, &vectors).expect("write manifest");

        let cli = CliOptions {
            signature_out: Some(dir.join("manifest_signatures.json")),
            allow_unsigned: true,
            ..CliOptions::default()
        };

        write_manifest_signatures(&dir, &vectors, manifest_digest, &cli)
            .expect("allow unsigned should bypass enforcement");
        assert!(
            !cli.signature_out.as_ref().unwrap().exists(),
            "manifest signatures file should not be created"
        );

        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }

    #[test]
    fn existing_signed_manifest_passes_verification() {
        let dir = temp_dir();
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        prepare_fixture_files(&dir, &vectors);
        let manifest_digest = write_manifest(&dir, &vectors).expect("write manifest");
        let signature_path = dir.join("manifest_signatures.json");

        let signer_cli = CliOptions {
            signature_out: Some(signature_path.clone()),
            signing_key_hex: Some(SIGNING_KEY_1.to_owned()),
            ..CliOptions::default()
        };
        write_manifest_signatures(&dir, &vectors, manifest_digest, &signer_cli)
            .expect("signing should succeed");
        assert!(signature_path.exists(), "signature file should be created");

        let verify_cli = CliOptions {
            signature_out: Some(signature_path.clone()),
            ..CliOptions::default()
        };
        write_manifest_signatures(&dir, &vectors, manifest_digest, &verify_cli)
            .expect("verification should succeed with existing signature");

        // Ensure additional signer can still be added afterwards.
        let second_cli = CliOptions {
            signature_out: Some(signature_path.clone()),
            signing_key_hex: Some(SIGNING_KEY_2.to_owned()),
            ..CliOptions::default()
        };
        write_manifest_signatures(&dir, &vectors, manifest_digest, &second_cli)
            .expect("appending second signature must succeed");

        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }
}
