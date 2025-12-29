use std::sync::OnceLock;

use fastpq_isi::poseidon::{
    FIELD_MODULUS, MDS as CPU_MDS, RATE, ROUND_CONSTANTS as CPU_ROUND_CONSTANTS, STATE_WIDTH,
};
use sha2::{Digest, Sha256};

const TOTAL_ROUNDS: usize = CPU_ROUND_CONSTANTS.len();
const SNAPSHOT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../artifacts/offline_poseidon/constants.ron"
));
const EXPECTED_SHA256: &str = "99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21";

static POSEIDON_MANIFEST: OnceLock<PoseidonManifest> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ManifestSection {
    None,
    RoundConstants,
    Mds,
}

/// Canonical Poseidon manifest derived from `artifacts/offline_poseidon/constants.ron`.
#[derive(Clone)]
pub struct PoseidonManifest {
    sha256_hex: String,
    round_constants: [[u64; STATE_WIDTH]; TOTAL_ROUNDS],
    mds: [[u64; STATE_WIDTH]; STATE_WIDTH],
}

impl PoseidonManifest {
    /// Returns the manifest SHA-256 as a lowercase hex string.
    #[must_use]
    pub fn sha256_hex(&self) -> &str {
        &self.sha256_hex
    }

    /// Returns the Poseidon round constants parsed from the manifest.
    #[must_use]
    pub fn round_constants(&self) -> &[[u64; STATE_WIDTH]; TOTAL_ROUNDS] {
        &self.round_constants
    }

    /// Returns the Poseidon MDS matrix parsed from the manifest.
    #[must_use]
    pub fn mds(&self) -> &[[u64; STATE_WIDTH]; STATE_WIDTH] {
        &self.mds
    }
}

/// Returns the canonical manifest snapshot, parsing and validating it on first use.
#[must_use]
pub fn poseidon_manifest() -> &'static PoseidonManifest {
    POSEIDON_MANIFEST.get_or_init(|| load_manifest().unwrap_or_else(|err| panic!("{err}")))
}

/// Returns the manifest SHA-256 digest.
#[must_use]
pub fn poseidon_manifest_sha256() -> &'static str {
    poseidon_manifest().sha256_hex()
}

#[allow(clippy::too_many_lines)]
fn load_manifest() -> Result<PoseidonManifest, String> {
    let sha256 = Sha256::digest(SNAPSHOT_BYTES);
    let sha256_hex = hex_encode(&sha256);
    if sha256_hex != EXPECTED_SHA256 {
        return Err(format!(
            "Poseidon manifest hash mismatch: expected {EXPECTED_SHA256}, got {sha256_hex}"
        ));
    }

    let text = std::str::from_utf8(SNAPSHOT_BYTES)
        .map_err(|err| format!("manifest is not valid UTF-8: {err}"))?;
    let mut width = None;
    let mut rate = None;
    let mut full_rounds = None;
    let mut partial_rounds = None;
    let mut field_modulus = None;
    let mut round_constants = Vec::with_capacity(TOTAL_ROUNDS);
    let mut mds_rows = Vec::with_capacity(STATE_WIDTH);
    let mut section = ManifestSection::None;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        match section {
            ManifestSection::RoundConstants => {
                if trimmed.starts_with('[') {
                    round_constants.push(parse_row(trimmed)?);
                } else if trimmed.starts_with("],") {
                    section = ManifestSection::None;
                }
                continue;
            }
            ManifestSection::Mds => {
                if trimmed.starts_with('[') {
                    mds_rows.push(parse_row(trimmed)?);
                } else if trimmed.starts_with("],") {
                    section = ManifestSection::None;
                }
                continue;
            }
            ManifestSection::None => {}
        }

        if trimmed.starts_with("round_constants:") {
            section = ManifestSection::RoundConstants;
            continue;
        }
        if trimmed.starts_with("mds:") {
            section = ManifestSection::Mds;
            continue;
        }
        if let Some(value) = parse_usize_field(trimmed, "width:")? {
            width = Some(value);
            continue;
        }
        if let Some(value) = parse_usize_field(trimmed, "rate:")? {
            rate = Some(value);
            continue;
        }
        if let Some(value) = parse_usize_field(trimmed, "full_rounds:")? {
            full_rounds = Some(value);
            continue;
        }
        if let Some(value) = parse_usize_field(trimmed, "partial_rounds:")? {
            partial_rounds = Some(value);
            continue;
        }
        if let Some(hex_value) = parse_string_field(trimmed, "field_modulus:") {
            field_modulus = Some(parse_hex_u64(hex_value)?);
        }
    }

    if round_constants.len() != TOTAL_ROUNDS {
        return Err(format!(
            "expected {TOTAL_ROUNDS} round constants, found {}",
            round_constants.len()
        ));
    }
    if mds_rows.len() != STATE_WIDTH {
        return Err(format!(
            "expected {STATE_WIDTH} MDS rows, found {}",
            mds_rows.len()
        ));
    }
    let width = width.ok_or("missing width field")?;
    let rate = rate.ok_or("missing rate field")?;
    let full_rounds = full_rounds.ok_or("missing full_rounds field")?;
    let partial_rounds = partial_rounds.ok_or("missing partial_rounds field")?;
    let field_modulus = field_modulus.ok_or("missing field_modulus field")?;

    if width != STATE_WIDTH {
        return Err(format!(
            "width mismatch: manifest {width}, expected {STATE_WIDTH}"
        ));
    }
    if rate != RATE {
        return Err(format!("rate mismatch: manifest {rate}, expected {RATE}"));
    }
    if full_rounds * 2 + partial_rounds != TOTAL_ROUNDS {
        return Err(format!(
            "round configuration mismatch: manifest {full_rounds} full/{partial_rounds} partial"
        ));
    }
    if field_modulus != FIELD_MODULUS {
        return Err(format!(
            "field modulus mismatch: manifest {field_modulus:#x}, expected {FIELD_MODULUS:#x}"
        ));
    }
    if round_constants != CPU_ROUND_CONSTANTS {
        return Err("round constants in manifest differ from CPU reference".to_owned());
    }
    if mds_rows != CPU_MDS {
        return Err("MDS matrix in manifest differs from CPU reference".to_owned());
    }

    let round_constants_array: [[u64; STATE_WIDTH]; TOTAL_ROUNDS] = round_constants
        .try_into()
        .map_err(|_| "round constants size mismatch".to_owned())?;
    let mds_array: [[u64; STATE_WIDTH]; STATE_WIDTH] = mds_rows
        .try_into()
        .map_err(|_| "MDS size mismatch".to_owned())?;

    Ok(PoseidonManifest {
        sha256_hex,
        round_constants: round_constants_array,
        mds: mds_array,
    })
}

fn parse_row(line: &str) -> Result<[u64; STATE_WIDTH], String> {
    let content = line
        .trim_start_matches('[')
        .trim_end_matches(',')
        .trim_end_matches(']')
        .trim();
    let mut values = [0u64; STATE_WIDTH];
    let mut count = 0;
    for token in content.split(',') {
        if token.trim().is_empty() {
            continue;
        }
        if count >= STATE_WIDTH {
            return Err(format!("row has more than {STATE_WIDTH} entries: {line}"));
        }
        values[count] = parse_hex_u64(token.trim())?;
        count += 1;
    }
    if count != STATE_WIDTH {
        return Err(format!(
            "row expected {STATE_WIDTH} entries, found {count}: {line}"
        ));
    }
    Ok(values)
}

fn parse_usize_field(line: &str, prefix: &str) -> Result<Option<usize>, String> {
    if !line.starts_with(prefix) {
        return Ok(None);
    }
    let mut value = line[prefix.len()..].trim();
    value = value.trim_end_matches(',');
    value
        .parse::<usize>()
        .map(Some)
        .map_err(|err| format!("invalid numeric literal `{value}`: {err}"))
}

fn parse_string_field<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    if !line.starts_with(prefix) {
        return None;
    }
    let mut value = line[prefix.len()..].trim();
    value = value.trim_end_matches(',');
    value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
}

fn parse_hex_u64(literal: &str) -> Result<u64, String> {
    let trimmed = literal.trim_end_matches(|c: char| c == ',' || c.is_ascii_whitespace());
    let value = trimmed
        .strip_prefix("0x")
        .ok_or_else(|| format!("expected hex literal, found `{literal}`"))?;
    u64::from_str_radix(&value.replace('_', ""), 16)
        .map_err(|err| format!("invalid hex literal `{literal}`: {err}"))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}
