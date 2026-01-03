use std::{fs, path::PathBuf};

use iroha_crypto::{Algorithm, PublicKey, Signature};
use norito::json::Value;
use sorafs_chunker::{chunk_bytes, fixtures::FixtureProfile};

const CANONICAL_PROFILE_HANDLE: &str = "sorafs.sf1@1.0.0";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("resolve repository root")
}

fn fixture_path() -> PathBuf {
    repo_root()
        .join("fixtures")
        .join("sorafs_chunker")
        .join("sf1_profile_v1.json")
}

fn manifest_path() -> PathBuf {
    repo_root()
        .join("fixtures")
        .join("sorafs_chunker")
        .join("manifest_blake3.json")
}

fn signature_path() -> PathBuf {
    repo_root()
        .join("fixtures")
        .join("sorafs_chunker")
        .join("manifest_signatures.json")
}

fn typescript_path() -> PathBuf {
    repo_root()
        .join("fixtures")
        .join("sorafs_chunker")
        .join("sf1_profile_v1.ts")
}

fn go_path() -> PathBuf {
    repo_root()
        .join("fixtures")
        .join("sorafs_chunker")
        .join("sf1_profile_v1.go")
}

fn load_fixture_json() -> Value {
    let bytes = fs::read(fixture_path()).expect("read fixtures/sorafs_chunker/sf1_profile_v1.json");
    norito::json::from_slice(&bytes).expect("parse chunker fixture json")
}

fn load_signature_json() -> Value {
    let bytes =
        fs::read(signature_path()).expect("read fixtures/sorafs_chunker/manifest_signatures.json");
    norito::json::from_slice(&bytes).expect("parse manifest signatures json")
}

fn decode_hex(hex: &str) -> Vec<u8> {
    assert!(
        hex.len().is_multiple_of(2),
        "hex string must have an even number of characters"
    );
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for pair in hex.as_bytes().chunks_exact(2) {
        let hi = (pair[0] as char).to_digit(16).expect("hex digit") as u8;
        let lo = (pair[1] as char).to_digit(16).expect("hex digit") as u8;
        bytes.push((hi << 4) | lo);
    }
    bytes
}

fn extract_delimited(content: &str, needle: &str, open: char, close: char) -> Option<String> {
    let start = content.find(needle)?;
    let after = &content[start..];
    let open_pos = after.find(open)?;
    let mut depth = 0i32;
    let mut end_rel = None;
    for (offset, ch) in after[open_pos..].char_indices() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                end_rel = Some(open_pos + offset);
                break;
            }
        }
    }
    let end_rel = end_rel?;
    let inner = &after[(open_pos + 1)..end_rel];
    Some(inner.to_owned())
}

fn parse_number_list(raw: &str) -> Vec<usize> {
    raw.split(',')
        .filter_map(|token| token.trim().parse::<usize>().ok())
        .collect()
}

fn parse_string_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .filter_map(|token| {
            let token = token.trim().trim_matches('"');
            if token.is_empty() {
                None
            } else {
                Some(token.to_owned())
            }
        })
        .collect()
}

#[test]
fn generated_vectors_match_expected_constants() {
    let vectors = FixtureProfile::SF1_V1.generate_vectors();
    assert_eq!(vectors.profile_id, CANONICAL_PROFILE_HANDLE);
    assert_eq!(vectors.input_length, 1 << 20);
    assert_eq!(vectors.chunk_count(), 5);
    assert_eq!(
        vectors.chunk_lengths,
        vec![177_082, 210_377, 403_145, 187_169, 70_803]
    );
    assert_eq!(
        vectors.chunk_offsets,
        vec![0, 177_082, 387_459, 790_604, 977_773]
    );
    assert_eq!(
        vectors.sha3_digest_hex(),
        "13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482"
    );

    let digest_hexes = vectors.blake3_digest_hexes();
    assert_eq!(
        digest_hexes,
        vec![
            "7789b490337d16c51b59a92e354a657ba450da4bab872c31e85e4d4fedcb3a27",
            "56397fe0ff8cc24c790e0719505fff05c49dca09289b595d17455bedcc1f7438",
            "2a2a47eedcc11effdb5d13190fe09143d6b31f959c72980bb0f2b8a5058971fd",
            "8dbb40d7a3439aa57bd69a3adfe98ff43b9043764072bc42d7dc211942e46ef8",
            "194c8180f4b90e021a86199d17092af3b2bd29792dc028e401258d903616b664"
        ]
    );

    let chunks = chunk_bytes(&vectors.input);
    let chunk_lengths: Vec<usize> = chunks.iter().map(|chunk| chunk.length).collect();
    assert_eq!(chunk_lengths, vectors.chunk_lengths);
    let chunk_offsets: Vec<usize> = chunks.iter().map(|chunk| chunk.offset).collect();
    assert_eq!(chunk_offsets, vectors.chunk_offsets);
}

#[test]
fn manifest_signature_matches_fixture_manifest() {
    let vectors = FixtureProfile::SF1_V1.generate_vectors();
    let manifest_bytes =
        fs::read(manifest_path()).expect("read fixtures/sorafs_chunker/manifest_blake3.json");
    let manifest_digest = blake3::hash(&manifest_bytes);

    let signatures = load_signature_json();
    assert_eq!(
        signatures
            .get("profile")
            .and_then(Value::as_str)
            .expect("profile"),
        CANONICAL_PROFILE_HANDLE
    );
    let alias_values = signatures
        .get("profile_aliases")
        .and_then(Value::as_array)
        .expect("profile_aliases array");
    let alias_strings: Vec<&str> = alias_values.iter().filter_map(Value::as_str).collect();
    assert_eq!(
        alias_strings,
        vec![CANONICAL_PROFILE_HANDLE],
        "profile_aliases must include only the canonical handle"
    );
    assert_eq!(
        signatures
            .get("manifest")
            .and_then(Value::as_str)
            .expect("manifest"),
        "manifest_blake3.json"
    );
    assert_eq!(
        signatures
            .get("manifest_blake3")
            .and_then(Value::as_str)
            .expect("manifest digest hex"),
        sorafs_chunker::fixtures::to_hex(manifest_digest.as_bytes())
    );
    assert_eq!(
        signatures
            .get("chunk_digest_sha3_256")
            .and_then(Value::as_str)
            .expect("sha3 digest"),
        vectors.sha3_digest_hex()
    );

    let entries = signatures
        .get("signatures")
        .and_then(Value::as_array)
        .expect("signatures array");
    assert!(
        !entries.is_empty(),
        "manifest signatures array must not be empty"
    );

    for entry in entries {
        let algorithm = entry
            .get("algorithm")
            .and_then(Value::as_str)
            .expect("algorithm string");
        assert_eq!(algorithm, "ed25519");

        let signer_hex = entry
            .get("signer")
            .and_then(Value::as_str)
            .expect("signer hex");
        let signature_hex = entry
            .get("signature")
            .and_then(Value::as_str)
            .expect("signature hex");

        let signer_bytes = decode_hex(signer_hex);
        let signature_bytes = decode_hex(signature_hex);

        let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &signer_bytes)
            .expect("valid signer public key");
        if let Some(multihash) = entry.get("signer_multihash").and_then(Value::as_str) {
            assert_eq!(multihash, public_key.to_string());
        }
        let signature = Signature::from_bytes(&signature_bytes);
        signature
            .verify(&public_key, manifest_digest.as_bytes())
            .expect("signature verifies");
    }
}

#[ignore = "utility for regenerating the fixture digest"]
#[test]
fn print_fixture_digest() {
    let vectors = FixtureProfile::SF1_V1.generate_vectors();
    let chunks = chunk_bytes(&vectors.input);
    println!(
        "chunks={}, digest={}",
        chunks.len(),
        vectors.sha3_digest_hex()
    );
    println!("lengths={:?}", vectors.chunk_lengths);
    let chunk_digests = vectors.blake3_digest_hexes();
    println!("digests={chunk_digests:?}");
}

#[test]
fn typescript_fixture_matches_vectors() {
    let vectors = FixtureProfile::SF1_V1.generate_vectors();
    let content = fs::read_to_string(typescript_path()).expect("read TypeScript fixture");
    assert!(
        content.contains(&format!("profile: \"{CANONICAL_PROFILE_HANDLE}\"")),
        "TypeScript fixture must expose canonical profile handle"
    );
    assert!(
        content.contains(&format!(
            "profileAliases: [\"{CANONICAL_PROFILE_HANDLE}\"] as const"
        )),
        "TypeScript fixture must expose profile aliases"
    );

    let chunk_lengths_block = extract_delimited(&content, "chunkLengths: [", '[', ']')
        .expect("chunkLengths array present");
    let chunk_lengths = parse_number_list(&chunk_lengths_block);
    assert_eq!(chunk_lengths, vectors.chunk_lengths);

    let chunk_offsets_block = extract_delimited(&content, "chunkOffsets: [", '[', ']')
        .expect("chunkOffsets array present");
    let chunk_offsets = parse_number_list(&chunk_offsets_block);
    assert_eq!(chunk_offsets, vectors.chunk_offsets);

    let digest_block = extract_delimited(&content, "chunkDigestsBlake3: [", '[', ']')
        .expect("chunk digests array present");
    let chunk_digests = parse_string_list(&digest_block);
    assert_eq!(chunk_digests, vectors.blake3_digest_hexes());
}

#[test]
fn go_fixture_matches_vectors() {
    let vectors = FixtureProfile::SF1_V1.generate_vectors();
    let content = fs::read_to_string(go_path()).expect("read Go fixture");
    assert!(
        content.contains(&format!("Profile: \"{CANONICAL_PROFILE_HANDLE}\"")),
        "Go fixture must expose canonical profile handle"
    );
    assert!(
        content.contains(&format!(
            "ProfileAliases: []string{{\"{CANONICAL_PROFILE_HANDLE}\"}}"
        )),
        "Go fixture must expose profile aliases"
    );

    let chunk_lengths_block =
        extract_delimited(&content, "ChunkLengths:", '{', '}').expect("ChunkLengths slice present");
    let chunk_lengths = parse_number_list(&chunk_lengths_block);
    assert_eq!(chunk_lengths, vectors.chunk_lengths);

    let chunk_offsets_block =
        extract_delimited(&content, "ChunkOffsets:", '{', '}').expect("ChunkOffsets slice present");
    let chunk_offsets = parse_number_list(&chunk_offsets_block);
    assert_eq!(chunk_offsets, vectors.chunk_offsets);

    let digest_block = extract_delimited(&content, "ChunkDigestsBLAKE3:", '{', '}')
        .expect("ChunkDigestsBLAKE3 slice present");
    let chunk_digests = parse_string_list(&digest_block);
    assert_eq!(chunk_digests, vectors.blake3_digest_hexes());
}

#[test]
fn json_fixture_in_sync_with_vectors() {
    let vectors = FixtureProfile::SF1_V1.generate_vectors();
    let json = load_fixture_json();

    assert_eq!(
        json.get("profile")
            .and_then(Value::as_str)
            .expect("profile string"),
        CANONICAL_PROFILE_HANDLE
    );
    let alias_values = json
        .get("profile_aliases")
        .and_then(Value::as_array)
        .expect("profile_aliases array");
    let alias_strings: Vec<&str> = alias_values.iter().filter_map(Value::as_str).collect();
    assert_eq!(
        alias_strings,
        vec![CANONICAL_PROFILE_HANDLE],
        "profile_aliases must include only the canonical handle"
    );
    assert_eq!(
        json.get("input_seed")
            .and_then(Value::as_str)
            .expect("input seed"),
        vectors.input_seed_hex
    );
    assert_eq!(
        json.get("input_length")
            .and_then(Value::as_u64)
            .expect("input length") as usize,
        vectors.input_length
    );
    assert_eq!(
        json.get("chunk_count")
            .and_then(Value::as_u64)
            .expect("chunk count") as usize,
        vectors.chunk_count()
    );

    let prng = json
        .get("prng")
        .and_then(Value::as_object)
        .expect("prng object");
    assert_eq!(
        prng.get("multiplier")
            .and_then(Value::as_u64)
            .expect("multiplier"),
        vectors.prng.multiplier
    );
    assert_eq!(
        prng.get("increment")
            .and_then(Value::as_u64)
            .expect("increment"),
        vectors.prng.increment
    );

    let lengths_json = json
        .get("chunk_lengths")
        .and_then(Value::as_array)
        .expect("chunk lengths array");
    let lengths: Vec<usize> = lengths_json
        .iter()
        .map(|value| value.as_u64().expect("chunk length") as usize)
        .collect();
    assert_eq!(lengths, vectors.chunk_lengths);

    let offsets_json = json
        .get("chunk_offsets")
        .and_then(Value::as_array)
        .expect("chunk offsets array");
    let offsets: Vec<usize> = offsets_json
        .iter()
        .map(|value| value.as_u64().expect("chunk offset") as usize)
        .collect();
    assert_eq!(offsets, vectors.chunk_offsets);

    let digest_hex = json
        .get("chunk_digest_sha3_256")
        .and_then(Value::as_str)
        .expect("digest hex");
    assert_eq!(digest_hex, vectors.sha3_digest_hex());

    let digest_values = json
        .get("chunk_digests_blake3")
        .and_then(Value::as_array)
        .expect("chunk digest array");
    let digest_strings: Vec<String> = digest_values
        .iter()
        .map(|value| value.as_str().expect("digest string").to_owned())
        .collect();
    let expected_digests = vectors.blake3_digest_hexes();
    assert_eq!(digest_strings, expected_digests);
}
