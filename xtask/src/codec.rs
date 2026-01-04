use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use iroha_crypto::{Algorithm, KeyPair, PrivateKey, PublicKey, Signature, SignatureOf};
use norito::{
    core::to_bytes,
    json::{self, Map as NoritoMap, Number as NoritoNumber, Value as NoritoValue},
    streaming::{
        RansGroupTableV1, RansTablesBodyV1, RansTablesSignatureV1, RansTablesV1,
        SignatureAlgorithm, SignedRansTablesV1,
    },
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::workspace_root;

const DEFAULT_OUTPUT_SUBDIR: &str = "artifacts/nsc";
const DEFAULT_OUTPUT_BASENAME: &str = "rans_tables";
const DEFAULT_PRECISION_BITS: u8 = 12;
pub const MIN_BUNDLE_WIDTH: u8 = 2;
pub const MAX_BUNDLE_WIDTH: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Toml,
    Csv,
}

impl OutputFormat {
    fn from_str(input: &str) -> Result<Self, Box<dyn Error>> {
        match input.to_ascii_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "toml" => Ok(Self::Toml),
            "csv" => Ok(Self::Csv),
            other => Err(format!("unsupported rans-tables format: {other}").into()),
        }
    }
}

#[derive(Clone)]
pub struct RansTablesOptions {
    pub seed: u64,
    pub bundle_width: u8,
    pub output_base: PathBuf,
    pub formats: Vec<OutputFormat>,
    pub signing_key: Option<PathBuf>,
    pub verify_paths: Vec<PathBuf>,
}

impl Default for RansTablesOptions {
    fn default() -> Self {
        Self {
            seed: 0,
            bundle_width: MAX_BUNDLE_WIDTH,
            output_base: default_output_base(),
            formats: Vec::new(),
            signing_key: None,
            verify_paths: Vec::new(),
        }
    }
}

impl RansTablesOptions {
    pub fn add_format(&mut self, value: &str) -> Result<(), Box<dyn Error>> {
        for item in value.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            self.formats.push(OutputFormat::from_str(item)?);
        }
        Ok(())
    }

    pub fn finalize_formats(&mut self) {
        if self.formats.is_empty() && self.verify_paths.is_empty() {
            self.formats.push(OutputFormat::Json);
            self.formats.push(OutputFormat::Toml);
        }
        self.formats.sort_by_key(|format| match format {
            OutputFormat::Json => 0,
            OutputFormat::Toml => 1,
            OutputFormat::Csv => 2,
        });
        self.formats.dedup();
    }

    pub fn default_output_path(&self, format: OutputFormat) -> PathBuf {
        let base = self.output_base.clone();
        let ext = match format {
            OutputFormat::Json => "json",
            OutputFormat::Toml => "toml",
            OutputFormat::Csv => "csv",
        };
        if base.extension().is_some() {
            base
        } else {
            base.with_extension(ext)
        }
    }
}

pub fn execute_rans_tables(options: RansTablesOptions) -> Result<(), Box<dyn Error>> {
    for path in &options.verify_paths {
        verify_artifact(path)?;
    }

    if options.formats.is_empty() {
        return Ok(());
    }

    let mut payload = build_payload(options.seed, options.bundle_width)?;
    let checksum = compute_checksum(&payload.body)?;
    payload.checksum_sha256 = checksum;

    let signature = if let Some(path) = options.signing_key.as_ref() {
        Some(sign_payload(&payload, path)?)
    } else {
        None
    };
    let signed = SignedRansTablesV1 { payload, signature };

    for format in &options.formats {
        let path = options.default_output_path(*format);
        write_artifact(&signed, *format, &path)?;
        println!(
            "wrote {} ({format:?})",
            path.strip_prefix(workspace_root())
                .unwrap_or(&path)
                .display()
        );
    }

    Ok(())
}

pub fn verify_tables(paths: &[PathBuf]) -> Result<(), Box<dyn Error>> {
    if paths.is_empty() {
        return Ok(());
    }
    for path in paths {
        verify_artifact(path)?;
    }
    Ok(())
}

fn build_payload(seed: u64, bundle_width: u8) -> Result<RansTablesV1, Box<dyn Error>> {
    let body = generate_body(seed, bundle_width)?;
    let generated_at = OffsetDateTime::now_utc()
        .unix_timestamp()
        .try_into()
        .unwrap_or_else(|_| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        });
    let commit = current_commit().unwrap_or_else(|_| "unknown".to_string());
    Ok(RansTablesV1 {
        version: 1,
        generated_at,
        generator_commit: commit,
        checksum_sha256: [0u8; 32],
        body,
    })
}

fn generate_body(seed: u64, bundle_width: u8) -> Result<RansTablesBodyV1, Box<dyn Error>> {
    let normalized_width = clamp_bundle_width(bundle_width);
    let group_size = 1usize << normalized_width;
    let index = normalized_width.saturating_sub(MIN_BUNDLE_WIDTH) as u64;
    let rng_seed = seed ^ (index << 32) ^ (group_size as u64);
    let base_group = generate_group(group_size, rng_seed)?;
    let mut groups = Vec::new();
    for width in MIN_BUNDLE_WIDTH..normalized_width {
        groups.push(derive_marginal_group(&base_group, width));
    }
    groups.push(base_group);
    Ok(RansTablesBodyV1 {
        seed,
        bundle_width: normalized_width,
        groups,
    })
}

fn generate_group(group_size: usize, seed: u64) -> Result<RansGroupTableV1, Box<dyn Error>> {
    if group_size == 0 {
        return Err("group_size must be non-zero".into());
    }
    let scale_total = 1u32 << DEFAULT_PRECISION_BITS;
    let width_bits = width_bits_from_group_size(group_size)?;
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let mut weights: Vec<u32> = (0..group_size)
        .map(|_| rng.random_range(1..=10_000))
        .collect();
    let weight_sum: u32 = weights.iter().sum();
    let mut frequencies = Vec::with_capacity(group_size);
    for weight in &mut weights {
        let scaled = ((*weight as u64) * (scale_total as u64)) / (weight_sum as u64);
        let freq = scaled.max(1).min(scale_total as u64) as u32;
        frequencies.push(freq);
    }
    normalize_frequencies(&mut frequencies, scale_total);
    let mut cumulative = Vec::with_capacity(group_size + 1);
    let mut running = 0u32;
    cumulative.push(running);
    for &freq in &frequencies {
        running = running.checked_add(freq).ok_or("frequency overflow")?;
        cumulative.push(running);
    }

    Ok(RansGroupTableV1 {
        width_bits,
        group_size: group_size as u16,
        precision_bits: DEFAULT_PRECISION_BITS,
        frequencies: frequencies.into_iter().map(|f| f as u16).collect(),
        cumulative,
    })
}

fn clamp_bundle_width(width: u8) -> u8 {
    width.clamp(MIN_BUNDLE_WIDTH, MAX_BUNDLE_WIDTH)
}

fn derive_marginal_group(base: &RansGroupTableV1, target_width: u8) -> RansGroupTableV1 {
    let base_width = (usize::from(base.group_size).trailing_zeros()) as u8;
    assert!(
        target_width <= base_width,
        "cannot derive marginal width {target_width} from base width {base_width}"
    );
    let stride = 1usize << (base_width - target_width);
    let mut cumulative = Vec::with_capacity((1usize << target_width) + 1);
    let mut frequencies = Vec::with_capacity(1usize << target_width);
    let mut running: u32 = 0;
    cumulative.push(0);
    for chunk in base.frequencies.chunks(stride) {
        let sum: u32 = chunk.iter().map(|&value| u32::from(value)).sum();
        running = running.checked_add(sum).expect("cdf overflow");
        frequencies.push(u16::try_from(sum).expect("marginal frequencies stay within u16 range"));
        cumulative.push(running);
    }
    RansGroupTableV1 {
        width_bits: target_width,
        group_size: 1u16 << target_width,
        precision_bits: base.precision_bits,
        frequencies,
        cumulative,
    }
}

fn width_bits_from_group_size(group_size: usize) -> Result<u8, Box<dyn Error>> {
    if group_size == 0 {
        return Err("group_size must be non-zero".into());
    }
    if group_size & (group_size - 1) != 0 {
        return Err("group_size must be a power of two".into());
    }
    Ok(group_size.trailing_zeros() as u8)
}

fn normalize_frequencies(frequencies: &mut [u32], target_sum: u32) {
    if frequencies.is_empty() {
        return;
    }
    let mut sum: i64 = frequencies.iter().map(|&v| v as i64).sum();
    let target = target_sum as i64;
    let mut index = 0usize;

    while sum < target {
        frequencies[index % frequencies.len()] += 1;
        sum += 1;
        index += 1;
    }
    while sum > target {
        let pos = index % frequencies.len();
        if frequencies[pos] > 1 {
            frequencies[pos] -= 1;
            sum -= 1;
        }
        index += 1;
    }
    if frequencies.contains(&0) {
        for value in frequencies.iter_mut() {
            if *value == 0 {
                *value = 1;
            }
        }
        normalize_frequencies(frequencies, target_sum);
    }
}

fn compute_checksum(body: &RansTablesBodyV1) -> Result<[u8; 32], Box<dyn Error>> {
    let bytes = to_bytes(body)?;
    let digest = Sha256::digest(bytes);
    Ok(digest.into())
}

fn sign_payload(
    payload: &RansTablesV1,
    key_path: &Path,
) -> Result<RansTablesSignatureV1, Box<dyn Error>> {
    let key_hex = fs::read_to_string(key_path)?;
    let cleaned = key_hex
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect::<String>();
    let private_key = PrivateKey::from_hex(Algorithm::Ed25519, &cleaned)?;
    let key_pair: KeyPair = private_key.clone().into();

    let signature = SignatureOf::new(key_pair.private_key(), payload);
    let signature_raw: Signature = signature.clone().into();
    let signature_bytes = signature_raw
        .payload()
        .try_into()
        .map_err(|_| "expected 64-byte Ed25519 signature payload")?;

    let (alg, public_bytes) = key_pair.public_key().to_bytes();
    if alg != Algorithm::Ed25519 {
        return Err("only Ed25519 signing keys are supported".into());
    }
    let public_key = public_bytes
        .try_into()
        .map_err(|_| "expected 32-byte Ed25519 public key payload")?;

    Ok(RansTablesSignatureV1 {
        algorithm: SignatureAlgorithm::Ed25519,
        public_key,
        signature: signature_bytes,
    })
}

fn signed_tables_to_value(signed: &SignedRansTablesV1) -> NoritoValue {
    let mut map = NoritoMap::new();
    map.insert("payload".to_string(), payload_to_value(&signed.payload));
    map.insert(
        "signature".to_string(),
        signed
            .signature
            .as_ref()
            .map(signature_to_value)
            .unwrap_or(NoritoValue::Null),
    );
    NoritoValue::Object(map)
}

fn payload_to_value(payload: &RansTablesV1) -> NoritoValue {
    let mut map = NoritoMap::new();
    map.insert(
        "version".to_string(),
        NoritoValue::Number(NoritoNumber::from(u64::from(payload.version))),
    );
    map.insert(
        "generated_at".to_string(),
        NoritoValue::Number(NoritoNumber::from(payload.generated_at)),
    );
    map.insert(
        "generator_commit".to_string(),
        NoritoValue::String(payload.generator_commit.clone()),
    );
    map.insert(
        "checksum_sha256".to_string(),
        NoritoValue::String(hex::encode_upper(payload.checksum_sha256)),
    );
    map.insert("body".to_string(), body_to_value(&payload.body));
    NoritoValue::Object(map)
}

fn body_to_value(body: &RansTablesBodyV1) -> NoritoValue {
    let mut map = NoritoMap::new();
    map.insert(
        "seed".to_string(),
        NoritoValue::Number(NoritoNumber::from(body.seed)),
    );
    map.insert(
        "bundle_width".to_string(),
        NoritoValue::Number(NoritoNumber::from(u64::from(body.bundle_width))),
    );
    let groups = body.groups.iter().map(group_to_value).collect::<Vec<_>>();
    map.insert("groups".to_string(), NoritoValue::Array(groups));
    NoritoValue::Object(map)
}

fn group_to_value(group: &RansGroupTableV1) -> NoritoValue {
    let mut map = NoritoMap::new();
    map.insert(
        "width_bits".to_string(),
        NoritoValue::Number(NoritoNumber::from(u64::from(group.width_bits))),
    );
    map.insert(
        "group_size".to_string(),
        NoritoValue::Number(NoritoNumber::from(u64::from(group.group_size))),
    );
    map.insert(
        "precision_bits".to_string(),
        NoritoValue::Number(NoritoNumber::from(u64::from(group.precision_bits))),
    );
    let freqs = group
        .frequencies
        .iter()
        .map(|&f| NoritoValue::Number(NoritoNumber::from(u64::from(f))))
        .collect();
    map.insert("frequencies".to_string(), NoritoValue::Array(freqs));
    let cumulative = group
        .cumulative
        .iter()
        .map(|&c| NoritoValue::Number(NoritoNumber::from(u64::from(c))))
        .collect();
    map.insert("cumulative".to_string(), NoritoValue::Array(cumulative));
    NoritoValue::Object(map)
}

fn signature_to_value(signature: &RansTablesSignatureV1) -> NoritoValue {
    let mut map = NoritoMap::new();
    map.insert(
        "algorithm".to_string(),
        NoritoValue::String(match signature.algorithm {
            SignatureAlgorithm::Ed25519 => "Ed25519".to_string(),
        }),
    );
    map.insert(
        "public_key".to_string(),
        NoritoValue::String(hex::encode_upper(signature.public_key)),
    );
    map.insert(
        "signature".to_string(),
        NoritoValue::String(hex::encode_upper(signature.signature)),
    );
    NoritoValue::Object(map)
}

fn signed_tables_from_value(value: &NoritoValue) -> Result<SignedRansTablesV1, Box<dyn Error>> {
    let map = value
        .as_object()
        .ok_or_else(|| "signed rANS tables must be an object".to_string())?;
    let payload_value = map
        .get("payload")
        .ok_or_else(|| "payload missing from signed tables".to_string())?;
    let signature_value = map.get("signature");
    let payload = payload_from_value(payload_value)?;
    let signature = match signature_value {
        Some(NoritoValue::Null) | None => None,
        Some(other) => Some(signature_from_value(other)?),
    };
    Ok(SignedRansTablesV1 { payload, signature })
}

fn payload_from_value(value: &NoritoValue) -> Result<RansTablesV1, Box<dyn Error>> {
    let map = value
        .as_object()
        .ok_or_else(|| "payload must be an object".to_string())?;
    let version = map
        .get("version")
        .and_then(NoritoValue::as_u64)
        .ok_or_else(|| "payload.version must be a u64".to_string())?;
    let generated_at = map
        .get("generated_at")
        .and_then(NoritoValue::as_u64)
        .ok_or_else(|| "payload.generated_at must be a u64".to_string())?;
    let generator_commit = map
        .get("generator_commit")
        .and_then(NoritoValue::as_str)
        .ok_or_else(|| "payload.generator_commit must be a string".to_string())?
        .to_string();
    let checksum_hex = map
        .get("checksum_sha256")
        .and_then(NoritoValue::as_str)
        .ok_or_else(|| "payload.checksum_sha256 must be a hex string".to_string())?;
    let checksum = decode_hex_array::<32>(checksum_hex, "checksum_sha256")?;
    let body_value = map
        .get("body")
        .ok_or_else(|| "payload.body missing".to_string())?;
    let body = body_from_value(body_value)?;
    Ok(RansTablesV1 {
        version: u16::try_from(version)
            .map_err(|_| "payload.version must fit into u16".to_string())?,
        generated_at,
        generator_commit,
        checksum_sha256: checksum,
        body,
    })
}

fn body_from_value(value: &NoritoValue) -> Result<RansTablesBodyV1, Box<dyn Error>> {
    let map = value
        .as_object()
        .ok_or_else(|| "body must be an object".to_string())?;
    let seed = map
        .get("seed")
        .and_then(NoritoValue::as_u64)
        .ok_or_else(|| "body.seed must be a u64".to_string())?;
    let bundle_width = map
        .get("bundle_width")
        .and_then(NoritoValue::as_u64)
        .map(|value| u8::try_from(value).unwrap_or(MAX_BUNDLE_WIDTH))
        .unwrap_or(MAX_BUNDLE_WIDTH);
    let groups_value = map
        .get("groups")
        .and_then(NoritoValue::as_array)
        .ok_or_else(|| "body.groups must be an array".to_string())?;
    let mut groups = Vec::with_capacity(groups_value.len());
    for value in groups_value {
        groups.push(group_from_value(value)?);
    }
    Ok(RansTablesBodyV1 {
        seed,
        bundle_width,
        groups,
    })
}

fn group_from_value(value: &NoritoValue) -> Result<RansGroupTableV1, Box<dyn Error>> {
    let map = value
        .as_object()
        .ok_or_else(|| "group entry must be an object".to_string())?;
    let width_bits = map
        .get("width_bits")
        .and_then(NoritoValue::as_u64)
        .ok_or_else(|| "group.width_bits must be a u64".to_string())?;
    let width_bits =
        u8::try_from(width_bits).map_err(|_| "group.width_bits must fit into u8".to_string())?;
    let group_size = map
        .get("group_size")
        .and_then(NoritoValue::as_u64)
        .ok_or_else(|| "group.group_size must be a u64".to_string())?;
    let precision_bits = map
        .get("precision_bits")
        .and_then(NoritoValue::as_u64)
        .ok_or_else(|| "group.precision_bits must be a u64".to_string())?;
    let freqs_value = map
        .get("frequencies")
        .and_then(NoritoValue::as_array)
        .ok_or_else(|| "group.frequencies must be an array".to_string())?;
    let mut frequencies = Vec::with_capacity(freqs_value.len());
    for freq in freqs_value {
        let value = freq
            .as_u64()
            .ok_or_else(|| "frequency entries must be u64".to_string())?;
        frequencies.push(
            u16::try_from(value).map_err(|_| "frequency entries must fit into u16".to_string())?,
        );
    }
    let cum_value = map
        .get("cumulative")
        .and_then(NoritoValue::as_array)
        .ok_or_else(|| "group.cumulative must be an array".to_string())?;
    let mut cumulative = Vec::with_capacity(cum_value.len());
    for entry in cum_value {
        let value = entry
            .as_u64()
            .ok_or_else(|| "cumulative entries must be u64".to_string())?;
        cumulative.push(
            u32::try_from(value).map_err(|_| "cumulative entries must fit into u32".to_string())?,
        );
    }
    let group_size_u16 =
        u16::try_from(group_size).map_err(|_| "group.group_size must fit into u16".to_string())?;
    let expected_width_bits = width_bits_from_group_size(usize::from(group_size_u16))?;
    if width_bits != expected_width_bits {
        return Err(format!(
            "group.width_bits must match group_size (expected {expected_width_bits})"
        )
        .into());
    }
    Ok(RansGroupTableV1 {
        width_bits,
        group_size: group_size_u16,
        precision_bits: u8::try_from(precision_bits)
            .map_err(|_| "group.precision_bits must fit into u8".to_string())?,
        frequencies,
        cumulative,
    })
}

fn signature_from_value(value: &NoritoValue) -> Result<RansTablesSignatureV1, Box<dyn Error>> {
    let map = value
        .as_object()
        .ok_or_else(|| "signature must be an object".to_string())?;
    let algorithm = map
        .get("algorithm")
        .and_then(NoritoValue::as_str)
        .ok_or_else(|| "signature.algorithm must be a string".to_string())?;
    let algorithm = match algorithm {
        "Ed25519" => SignatureAlgorithm::Ed25519,
        other => {
            return Err(format!("unsupported signature algorithm `{other}`").into());
        }
    };
    let public_key_hex = map
        .get("public_key")
        .and_then(NoritoValue::as_str)
        .ok_or_else(|| "signature.public_key must be a hex string".to_string())?;
    let signature_hex = map
        .get("signature")
        .and_then(NoritoValue::as_str)
        .ok_or_else(|| "signature.signature must be a hex string".to_string())?;
    let public_key = decode_hex_array::<32>(public_key_hex, "signature.public_key")?;
    let signature = decode_hex_array::<64>(signature_hex, "signature.signature")?;
    Ok(RansTablesSignatureV1 {
        algorithm,
        public_key,
        signature,
    })
}

fn decode_hex_array<const N: usize>(text: &str, label: &str) -> Result<[u8; N], Box<dyn Error>> {
    let bytes = hex::decode(text)?;
    let array: [u8; N] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| format!("{label} must contain {N} bytes (hex)"))?;
    Ok(array)
}

fn serde_value_to_norito(value: &serde_json::Value) -> Result<NoritoValue, Box<dyn Error>> {
    Ok(match value {
        serde_json::Value::Null => NoritoValue::Null,
        serde_json::Value::Bool(b) => NoritoValue::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                NoritoValue::Number(NoritoNumber::from(u))
            } else if let Some(i) = n.as_i64() {
                NoritoValue::Number(NoritoNumber::from(i))
            } else if let Some(f) = n.as_f64() {
                let num = NoritoNumber::from_f64(f)
                    .ok_or_else(|| "cannot represent NaN/Infinity in Norito JSON".to_string())?;
                NoritoValue::Number(num)
            } else {
                return Err("unsupported numeric representation in serde value".into());
            }
        }
        serde_json::Value::String(s) => NoritoValue::String(s.clone()),
        serde_json::Value::Array(arr) => NoritoValue::Array(
            arr.iter()
                .map(serde_value_to_norito)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        serde_json::Value::Object(map) => {
            let mut out = NoritoMap::new();
            for (k, v) in map {
                out.insert(k.clone(), serde_value_to_norito(v)?);
            }
            NoritoValue::Object(out)
        }
    })
}

fn write_artifact(
    signed: &SignedRansTablesV1,
    format: OutputFormat,
    path: &Path,
) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    match format {
        OutputFormat::Json => {
            let json_value = signed_tables_to_value(signed);
            let mut text = json::to_json_pretty(&json_value)?;
            text.push('\n');
            fs::write(path, text)?;
        }
        OutputFormat::Toml => {
            let json_value = signed_tables_to_value(signed);
            let mut serde_value = norito_value_to_serde(&json_value);
            strip_nulls(&mut serde_value);
            let toml_value = toml::Value::deserialize(serde_value)?;
            let mut text = toml::to_string_pretty(&toml_value)?;
            text.push('\n');
            fs::write(path, text)?;
        }
        OutputFormat::Csv => {
            let mut rows = String::from("group_size,symbol_index,frequency,cumulative\n");
            for group in &signed.payload.body.groups {
                for (idx, &freq) in group.frequencies.iter().enumerate() {
                    let cumulative =
                        group.cumulative.get(idx + 1).copied().unwrap_or_else(|| {
                            group.cumulative.last().copied().unwrap_or_default()
                        });
                    rows.push_str(&format!(
                        "{},{},{},{}\n",
                        group.group_size, idx, freq, cumulative
                    ));
                }
            }
            fs::write(path, rows)?;
        }
    }

    Ok(())
}

fn verify_artifact(path: &Path) -> Result<(), Box<dyn Error>> {
    let contents = fs::read_to_string(path)?;
    let format = match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("toml") => OutputFormat::Toml,
        Some("csv") => {
            return Err("verification for CSV artefacts is not supported".into());
        }
        _ => OutputFormat::Json,
    };
    let signed = parse_signed(&contents, format)?;
    verify_payload(&signed)?;
    if let Some(signature) = &signed.signature {
        verify_signature(&signed.payload, signature)?;
    }
    println!(
        "verified {}",
        path.strip_prefix(workspace_root())
            .unwrap_or(path)
            .display()
    );
    Ok(())
}

fn verify_payload(signed: &SignedRansTablesV1) -> Result<(), Box<dyn Error>> {
    let expected_checksum = compute_checksum(&signed.payload.body)?;
    if expected_checksum != signed.payload.checksum_sha256 {
        return Err("checksum mismatch".into());
    }
    let regenerated = generate_body(signed.payload.body.seed, signed.payload.body.bundle_width)?;
    if regenerated != signed.payload.body {
        return Err("generated tables do not match deterministic output".into());
    }
    Ok(())
}

fn verify_signature(
    payload: &RansTablesV1,
    signature: &RansTablesSignatureV1,
) -> Result<(), Box<dyn Error>> {
    if signature.algorithm != SignatureAlgorithm::Ed25519 {
        return Err("unsupported signature algorithm in artefact".into());
    }
    let public_key =
        PublicKey::from_bytes(Algorithm::Ed25519, &signature.public_key).map_err(|err| {
            format!("failed to parse Ed25519 public key for artefact verification: {err}")
        })?;
    let sig = Signature::from_bytes(&signature.signature);
    let typed = SignatureOf::<RansTablesV1>::from_signature(sig);
    typed
        .verify(&public_key, payload)
        .map_err(|_| "signature verification failed".into())
}

fn parse_signed(
    contents: &str,
    format: OutputFormat,
) -> Result<SignedRansTablesV1, Box<dyn Error>> {
    match format {
        OutputFormat::Json => {
            let value: NoritoValue = json::from_str(contents)?;
            signed_tables_from_value(&value)
        }
        OutputFormat::Toml => {
            let toml_value: toml::Value = toml::from_str(contents)?;
            let serde_value: serde_json::Value = toml_value.try_into()?;
            let norito_value = serde_value_to_norito(&serde_value)?;
            signed_tables_from_value(&norito_value)
        }
        OutputFormat::Csv => Err("cannot parse CSV artefact into SignedRansTablesV1".into()),
    }
}

fn norito_value_to_serde(value: &NoritoValue) -> serde_json::Value {
    match value {
        NoritoValue::Null => serde_json::Value::Null,
        NoritoValue::Bool(b) => serde_json::Value::Bool(*b),
        NoritoValue::Number(num) => {
            if let Some(v) = num.as_i64() {
                serde_json::Value::Number(v.into())
            } else if let Some(v) = num.as_u64() {
                serde_json::Value::Number(v.into())
            } else if let Some(v) = num.as_f64() {
                serde_json::Number::from_f64(v)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::Value::Null
            }
        }
        NoritoValue::String(s) => serde_json::Value::String(s.clone()),
        NoritoValue::Array(arr) => {
            let converted = arr.iter().map(norito_value_to_serde).collect();
            serde_json::Value::Array(converted)
        }
        NoritoValue::Object(map) => {
            let converted = map
                .iter()
                .map(|(k, v)| (k.clone(), norito_value_to_serde(v)))
                .collect();
            serde_json::Value::Object(converted)
        }
    }
}

fn strip_nulls(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Null => {}
        serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
        serde_json::Value::Array(items) => {
            for item in items {
                strip_nulls(item);
            }
        }
        serde_json::Value::Object(map) => {
            let null_keys: Vec<String> = map
                .iter()
                .filter_map(|(key, v)| if v.is_null() { Some(key.clone()) } else { None })
                .collect();
            for key in null_keys {
                map.remove(&key);
            }
            for value in map.values_mut() {
                strip_nulls(value);
            }
        }
    }
}

fn current_commit() -> Result<String, Box<dyn Error>> {
    let output = std::process::Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(workspace_root())
        .output()?;
    if output.status.success() {
        let text = String::from_utf8(output.stdout)?.trim().to_string();
        Ok(text)
    } else {
        Err("git rev-parse HEAD failed".into())
    }
}

fn default_output_base() -> PathBuf {
    let mut base = workspace_root();
    base.push(DEFAULT_OUTPUT_SUBDIR);
    base.push(DEFAULT_OUTPUT_BASENAME);
    base
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write};

    use tempfile::{NamedTempFile, TempDir};

    use super::*;

    fn sample_payload(seed: u64) -> RansTablesV1 {
        let body = generate_body(seed, MAX_BUNDLE_WIDTH).expect("generate deterministic body");
        let checksum = compute_checksum(&body).expect("compute checksum");
        RansTablesV1 {
            version: 1,
            generated_at: 1,
            generator_commit: String::from("test-commit"),
            checksum_sha256: checksum,
            body,
        }
    }

    fn sample_signed(seed: u64) -> SignedRansTablesV1 {
        SignedRansTablesV1 {
            payload: sample_payload(seed),
            signature: None,
        }
    }

    fn write_ed25519_key(seed: [u8; 32]) -> NamedTempFile {
        let key_pair = KeyPair::from_seed(seed.to_vec(), Algorithm::Ed25519);
        let (_, private_key) = key_pair.into_parts();
        let (_, secret_bytes) = private_key.to_bytes();
        let mut file = NamedTempFile::new().expect("private key temp file");
        writeln!(file, "{}", hex::encode(secret_bytes)).expect("write private key");
        file
    }

    #[test]
    fn deterministic_generation() {
        let first = generate_body(42, MAX_BUNDLE_WIDTH).expect("body");
        let second = generate_body(42, MAX_BUNDLE_WIDTH).expect("body");
        assert_eq!(first, second);
    }

    #[test]
    fn width_bits_cover_all_bundle_sizes() {
        for width in MIN_BUNDLE_WIDTH..=MAX_BUNDLE_WIDTH {
            let body = generate_body(17, width).expect("body");
            let mut expected = Vec::new();
            for bits in MIN_BUNDLE_WIDTH..width {
                expected.push(bits);
            }
            expected.push(width);
            let actual: Vec<u8> = body.groups.iter().map(|group| group.width_bits).collect();
            assert_eq!(
                actual, expected,
                "unexpected width_bits sequence for width={width}"
            );
        }
    }

    #[test]
    fn group_from_value_rejects_missing_width_bits() {
        let mut map = NoritoMap::new();
        map.insert(
            "group_size".into(),
            NoritoValue::Number(NoritoNumber::from(4u64)),
        );
        map.insert(
            "precision_bits".into(),
            NoritoValue::Number(NoritoNumber::from(u64::from(DEFAULT_PRECISION_BITS))),
        );
        let freqs = vec![NoritoValue::Number(NoritoNumber::from(1024u64)); 4];
        map.insert("frequencies".into(), NoritoValue::Array(freqs));
        let cumulative = (0..=4)
            .map(|idx| NoritoValue::Number(NoritoNumber::from((idx * 1024) as u64)))
            .collect();
        map.insert("cumulative".into(), NoritoValue::Array(cumulative));
        let group = NoritoValue::Object(map);
        let err = group_from_value(&group).expect_err("missing width_bits should fail");
        assert!(
            err.to_string().contains("group.width_bits"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn marginal_groups_match_base_distribution() {
        let body = generate_body(7, MAX_BUNDLE_WIDTH).expect("body");
        assert_eq!(
            body.groups.len(),
            usize::from(MAX_BUNDLE_WIDTH - MIN_BUNDLE_WIDTH + 1)
        );
        let width2 = &body.groups[0];
        let width3 = &body.groups[1];
        let width4 = &body.groups[2];
        assert_eq!(usize::from(width2.group_size), 1 << 2);
        assert_eq!(usize::from(width3.group_size), 1 << 3);
        assert_eq!(usize::from(width4.group_size), 1 << 4);

        for (chunk, freq2) in width4.frequencies.chunks(4).zip(width2.frequencies.iter()) {
            let sum: u32 = chunk.iter().map(|&value| u32::from(value)).sum();
            assert_eq!(sum, u32::from(*freq2));
        }
        for (chunk, freq3) in width4.frequencies.chunks(2).zip(width3.frequencies.iter()) {
            let sum: u32 = chunk.iter().map(|&value| u32::from(value)).sum();
            assert_eq!(sum, u32::from(*freq3));
        }
    }

    #[test]
    fn frequencies_normalized() {
        let group = generate_group(8, 7).expect("group");
        let total: u32 = group.cumulative.last().copied().unwrap_or_default();
        assert_eq!(total, 1u32 << DEFAULT_PRECISION_BITS);
        assert!(group.frequencies.iter().all(|&f| f > 0));
    }

    #[test]
    fn verify_payload_accepts_generated_body() {
        let signed = sample_signed(99);
        verify_payload(&signed).expect("payload verification");
    }

    #[test]
    fn verify_payload_detects_tampering() {
        let mut signed = sample_signed(11);
        let first_group = signed
            .payload
            .body
            .groups
            .first_mut()
            .expect("group present");
        if let Some(freq) = first_group.frequencies.first_mut() {
            *freq = freq.saturating_add(1);
        }
        assert!(verify_payload(&signed).is_err());
    }

    #[test]
    fn verify_signature_roundtrip() {
        let payload = sample_payload(7);
        let key_file = write_ed25519_key([0x11; 32]);
        let signature = sign_payload(&payload, key_file.path()).expect("sign payload");
        verify_signature(&payload, &signature).expect("signature verification succeeds");

        let mut tampered = signature;
        tampered.signature[0] ^= 0xFF;
        assert!(verify_signature(&payload, &tampered).is_err());
    }

    #[test]
    fn write_and_parse_artifacts() {
        let payload = sample_payload(1234);
        let key_file = write_ed25519_key([0x22; 32]);
        let signature = sign_payload(&payload, key_file.path()).expect("sign payload");
        let signed = SignedRansTablesV1 {
            payload: payload.clone(),
            signature: Some(signature),
        };

        let temp_dir = TempDir::new().expect("temp dir");
        let json_path = temp_dir.path().join("tables.json");
        let toml_path = temp_dir.path().join("tables.toml");
        let csv_path = temp_dir.path().join("tables.csv");
        write_artifact(&signed, OutputFormat::Json, &json_path).expect("write json");
        write_artifact(&signed, OutputFormat::Toml, &toml_path).expect("write toml");
        write_artifact(&signed, OutputFormat::Csv, &csv_path).expect("write csv");

        let json_text = fs::read_to_string(&json_path).expect("read json");
        let parsed_json = parse_signed(&json_text, OutputFormat::Json).expect("parse json");
        assert_eq!(parsed_json, signed);

        let toml_text = fs::read_to_string(&toml_path).expect("read toml");
        let parsed_toml = parse_signed(&toml_text, OutputFormat::Toml).expect("parse toml");
        assert_eq!(parsed_toml, signed);

        verify_artifact(&json_path).expect("verify json artefact");
        verify_artifact(&toml_path).expect("verify toml artefact");

        let csv_text = fs::read_to_string(&csv_path).expect("read csv");
        let mut lines = csv_text.lines();
        assert_eq!(
            lines.next(),
            Some("group_size,symbol_index,frequency,cumulative")
        );
        let row_count = lines.count();
        let expected_rows: usize = signed
            .payload
            .body
            .groups
            .iter()
            .map(|g| g.frequencies.len())
            .sum();
        assert_eq!(row_count, expected_rows);

        let mut tampered = signed.clone();
        tampered.payload.body.groups[0].frequencies[0] ^= 1;
        let tampered_path = temp_dir.path().join("tampered.json");
        write_artifact(&tampered, OutputFormat::Json, &tampered_path).expect("write tampered");
        assert!(verify_artifact(&tampered_path).is_err());
    }

}
