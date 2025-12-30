//! Shared helpers for serialising and parsing PoR proofs and trees.

use norito::json::{Map, Value};

use crate::{PorChunkTree, PorLeaf, PorMerkleTree, PorProof, PorSegment};

/// Convert the full PoR tree into a Norito JSON value (root → chunks → segments → leaves).
#[must_use]
pub fn tree_to_value(tree: &PorMerkleTree) -> Value {
    let mut root = Map::new();
    root.insert("root_hex".into(), Value::from(to_hex(tree.root())));
    root.insert(
        "chunk_count".into(),
        Value::from(tree.chunks().len() as u64),
    );

    let mut chunk_entries = Vec::with_capacity(tree.chunks().len());
    for chunk in tree.chunks() {
        chunk_entries.push(Value::Object(chunk_to_map(chunk)));
    }

    root.insert("chunks".into(), Value::Array(chunk_entries));
    Value::Object(root)
}

/// Convert a PoR proof into a JSON map.
#[must_use]
pub fn proof_to_map(proof: &PorProof) -> Map {
    let mut map = Map::new();
    map.insert("payload_len".into(), Value::from(proof.payload_len));
    map.insert("chunk_index".into(), Value::from(proof.chunk_index as u64));
    map.insert("chunk_offset".into(), Value::from(proof.chunk_offset));
    map.insert(
        "chunk_length".into(),
        Value::from(proof.chunk_length as u64),
    );
    map.insert(
        "chunk_digest_hex".into(),
        Value::from(to_hex(&proof.chunk_digest)),
    );
    map.insert(
        "chunk_root_hex".into(),
        Value::from(to_hex(&proof.chunk_root)),
    );
    map.insert(
        "segment_index".into(),
        Value::from(proof.segment_index as u64),
    );
    map.insert("segment_offset".into(), Value::from(proof.segment_offset));
    map.insert(
        "segment_length".into(),
        Value::from(proof.segment_length as u64),
    );
    map.insert(
        "segment_digest_hex".into(),
        Value::from(to_hex(&proof.segment_digest)),
    );
    map.insert("leaf_index".into(), Value::from(proof.leaf_index as u64));
    map.insert("leaf_offset".into(), Value::from(proof.leaf_offset));
    map.insert("leaf_length".into(), Value::from(proof.leaf_length as u64));
    map.insert(
        "leaf_bytes_hex".into(),
        Value::from(to_hex(&proof.leaf_bytes)),
    );
    map.insert(
        "leaf_digest_hex".into(),
        Value::from(to_hex(&proof.leaf_digest)),
    );
    map.insert(
        "segment_leaves_hex".into(),
        Value::Array(
            proof
                .segment_leaves
                .iter()
                .map(|digest| Value::from(to_hex(digest)))
                .collect(),
        ),
    );
    map.insert(
        "chunk_segments_hex".into(),
        Value::Array(
            proof
                .chunk_segments
                .iter()
                .map(|digest| Value::from(to_hex(digest)))
                .collect(),
        ),
    );
    map.insert(
        "chunk_roots_hex".into(),
        Value::Array(
            proof
                .chunk_roots
                .iter()
                .map(|digest| Value::from(to_hex(digest)))
                .collect(),
        ),
    );
    map
}

/// Convert a PoR proof into a Norito JSON value.
#[must_use]
pub fn proof_to_value(proof: &PorProof) -> Value {
    Value::Object(proof_to_map(proof))
}

/// Parse a PoR proof from a Norito JSON value.
pub fn proof_from_value(value: &Value) -> Result<PorProof, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "expected PoR proof JSON object".to_string())?;
    let payload_len = expect_u64(obj, "payload_len")?;
    let chunk_index = expect_usize(obj, "chunk_index")?;
    let chunk_offset = expect_u64(obj, "chunk_offset")?;
    let chunk_length = expect_u32(obj, "chunk_length")?;
    let chunk_digest = expect_digest(obj, "chunk_digest_hex")?;
    let chunk_root = expect_digest(obj, "chunk_root_hex")?;
    let segment_index = expect_usize(obj, "segment_index")?;
    let segment_offset = expect_u64(obj, "segment_offset")?;
    let segment_length = expect_u32(obj, "segment_length")?;
    let segment_digest = expect_digest(obj, "segment_digest_hex")?;
    let leaf_index = expect_usize(obj, "leaf_index")?;
    let leaf_offset = expect_u64(obj, "leaf_offset")?;
    let leaf_length = expect_u32(obj, "leaf_length")?;
    let leaf_bytes = hex_to_vec(expect_str(obj, "leaf_bytes_hex")?)?;
    let leaf_digest = expect_digest(obj, "leaf_digest_hex")?;
    let segment_leaves = expect_digest_array(obj, "segment_leaves_hex")?;
    let chunk_segments = expect_digest_array(obj, "chunk_segments_hex")?;
    let chunk_roots = expect_digest_array(obj, "chunk_roots_hex")?;

    Ok(PorProof {
        payload_len,
        chunk_index,
        chunk_offset,
        chunk_length,
        chunk_digest,
        chunk_root,
        segment_index,
        segment_offset,
        segment_length,
        segment_digest,
        leaf_index,
        leaf_offset,
        leaf_length,
        leaf_bytes,
        leaf_digest,
        segment_leaves,
        chunk_segments,
        chunk_roots,
    })
}

/// Convert a sampled PoR proof into a JSON map.
#[must_use]
pub fn sample_to_map(flat_index: usize, proof: &PorProof) -> Map {
    let mut map = Map::new();
    map.insert("leaf_index_flat".into(), Value::from(flat_index as u64));
    map.insert("chunk_index".into(), Value::from(proof.chunk_index as u64));
    map.insert(
        "segment_index".into(),
        Value::from(proof.segment_index as u64),
    );
    map.insert("leaf_index".into(), Value::from(proof.leaf_index as u64));
    map.insert("proof".into(), proof_to_value(proof));
    map
}

/// Parse a `chunk:segment:leaf` specification used by CLI flags.
pub fn parse_proof_spec(spec: &str) -> Result<(usize, usize, usize), String> {
    let mut parts = spec.split(':');
    let chunk = parts
        .next()
        .ok_or_else(|| "missing chunk index in --por-proof".to_string())?
        .parse::<usize>()
        .map_err(|err| format!("invalid chunk index: {err}"))?;
    let segment = parts
        .next()
        .ok_or_else(|| "missing segment index in --por-proof".to_string())?
        .parse::<usize>()
        .map_err(|err| format!("invalid segment index: {err}"))?;
    let leaf = parts
        .next()
        .ok_or_else(|| "missing leaf index in --por-proof".to_string())?
        .parse::<usize>()
        .map_err(|err| format!("invalid leaf index: {err}"))?;
    if parts.next().is_some() {
        return Err("unexpected extra components in --por-proof".to_string());
    }
    Ok((chunk, segment, leaf))
}

/// SplitMix64 PRNG helper used for deterministic sampling.
#[must_use]
pub fn splitmix64(state: u64) -> u64 {
    let mut z = state.wrapping_add(0x9e3779b97f4a7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

fn chunk_to_map(chunk: &PorChunkTree) -> Map {
    let mut obj = Map::new();
    obj.insert("chunk_index".into(), Value::from(chunk.chunk_index as u64));
    obj.insert("offset".into(), Value::from(chunk.offset));
    obj.insert("length".into(), Value::from(chunk.length as u64));
    obj.insert(
        "chunk_digest_hex".into(),
        Value::from(to_hex(&chunk.chunk_digest)),
    );
    obj.insert("root_hex".into(), Value::from(to_hex(&chunk.root)));

    let mut segments = Vec::with_capacity(chunk.segments.len());
    for segment in &chunk.segments {
        segments.push(Value::Object(segment_to_map(segment)));
    }
    obj.insert("segments".into(), Value::Array(segments));
    obj
}

fn segment_to_map(segment: &PorSegment) -> Map {
    let mut obj = Map::new();
    obj.insert("offset".into(), Value::from(segment.offset));
    obj.insert("length".into(), Value::from(segment.length as u64));
    obj.insert("digest_hex".into(), Value::from(to_hex(&segment.digest)));

    let mut leaves = Vec::with_capacity(segment.leaves.len());
    for leaf in &segment.leaves {
        leaves.push(Value::Object(leaf_to_map(leaf)));
    }
    obj.insert("leaves".into(), Value::Array(leaves));
    obj
}

fn leaf_to_map(leaf: &PorLeaf) -> Map {
    let mut obj = Map::new();
    obj.insert("offset".into(), Value::from(leaf.offset));
    obj.insert("length".into(), Value::from(leaf.length as u64));
    obj.insert("digest_hex".into(), Value::from(to_hex(&leaf.digest)));
    obj
}

fn expect_u64(map: &Map, key: &str) -> Result<u64, String> {
    map.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing or invalid {key} in PoR proof"))
}

fn expect_usize(map: &Map, key: &str) -> Result<usize, String> {
    let value = expect_u64(map, key)?;
    usize::try_from(value).map_err(|_| format!("{key} value {value} does not fit into usize"))
}

fn expect_u32(map: &Map, key: &str) -> Result<u32, String> {
    let value = expect_u64(map, key)?;
    u32::try_from(value).map_err(|_| format!("{key} value {value} exceeds u32 range"))
}

fn expect_str<'a>(map: &'a Map, key: &str) -> Result<&'a str, String> {
    map.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing or invalid {key} string in PoR proof"))
}

fn expect_digest(map: &Map, key: &str) -> Result<[u8; 32], String> {
    parse_digest_hex(expect_str(map, key)?)
}

fn expect_digest_array(map: &Map, key: &str) -> Result<Vec<[u8; 32]>, String> {
    let values = map
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing or invalid {key} array in PoR proof"))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("expected string in {key} array"))
                .and_then(parse_digest_hex)
        })
        .collect()
}

fn parse_digest_hex(hex: &str) -> Result<[u8; 32], String> {
    let bytes = hex_to_vec(hex)?;
    if bytes.len() != 32 {
        return Err(format!(
            "expected 32-byte digest, found {} bytes for hex {hex}",
            bytes.len()
        ));
    }
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&bytes);
    Ok(digest)
}

fn hex_to_vec(hex: &str) -> Result<Vec<u8>, String> {
    if !hex.len().is_multiple_of(2) {
        return Err(format!("odd-length hex string: {hex}"));
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    let bytes = hex.as_bytes();
    for idx in (0..bytes.len()).step_by(2) {
        let hi = from_hex_digit(bytes[idx])?;
        let lo = from_hex_digit(bytes[idx + 1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn from_hex_digit(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!("invalid hex digit: {byte}")),
    }
}

fn to_hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(TABLE[(byte >> 4) as usize] as char);
        out.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    out
}
