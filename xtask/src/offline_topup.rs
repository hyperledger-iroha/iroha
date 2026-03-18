use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use blake2::{Blake2b512, Digest};
use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT, ristretto::RistrettoPoint, scalar::Scalar,
};
use eyre::{Context as _, Result, eyre};
use hex::encode_upper;
use iroha::{
    client::Client,
    config::{Config, LoadPath},
    data_model::{
        account::AccountId,
        asset::AssetId,
        isi::offline::RegisterOfflineAllowance,
        metadata::Metadata,
        offline::{
            AndroidIntegrityMetadata, AndroidProvisionedProof, OfflineAllowanceCommitment,
            OfflineWalletCertificate, OfflineWalletPolicy,
        },
        prelude::Numeric,
    },
};
use iroha_crypto::{Hash, PrivateKey, PublicKey, Signature};
use norito::{
    NoritoDeserialize,
    derive::{JsonDeserialize, JsonSerialize},
    from_bytes, json as norito_json,
    json::{self as serde_json, Map as JsonMap, Value as JsonValue},
    to_bytes,
};
use once_cell::sync::Lazy;
use rand::RngCore;
use sha2::Sha512;

use crate::offline_tooling::{
    build_metadata, parse_private_key, parse_public_key, write_json, write_norito_bytes,
};

const H_GENERATOR_LABEL: &[u8] = b"iroha.offline.balance.generator.H.v1";
const BLINDING_DERIVE_LABEL: &[u8] = b"iroha.offline.balance.blind.v1";

static PEDERSEN_H: Lazy<RistrettoPoint> =
    Lazy::new(|| RistrettoPoint::hash_from_bytes::<Sha512>(H_GENERATOR_LABEL));

#[derive(Debug)]
pub(crate) struct OfflineTopupOptions {
    pub spec_path: PathBuf,
    pub output_root: PathBuf,
    pub operator_key_override: Option<String>,
    pub register: Option<RegisterOptions>,
}

#[derive(Debug)]
pub(crate) struct RegisterOptions {
    pub config_path: PathBuf,
    pub mode: RegisterMode,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RegisterMode {
    Blocking,
    Immediate,
}

#[derive(Debug, JsonDeserialize)]
struct SpecFile {
    operator: Option<OperatorSpec>,
    allowances: Vec<AllowanceSpec>,
}

#[derive(Debug, JsonDeserialize)]
struct OperatorSpec {
    account: Option<String>,
    private_key: Option<String>,
}

#[derive(Debug, JsonDeserialize)]
struct AllowanceSpec {
    label: String,
    controller: String,
    operator: Option<String>,
    allowance_asset: String,
    amount: String,
    issued_at_ms: u64,
    expires_at_ms: u64,
    policy: PolicySpec,
    spend_public_key: String,
    metadata: Option<JsonValue>,
    metadata_file: Option<PathBuf>,
    provisioned_proof_file: Option<PathBuf>,
    attestation_report_file: Option<PathBuf>,
    attestation_report_hex: Option<String>,
    attestation_report_base64: Option<String>,
    blinding_hex: Option<String>,
    operator_key: Option<String>,
    verdict_id_hex: Option<String>,
    attestation_nonce_hex: Option<String>,
    refresh_at_ms: Option<u64>,
}

#[derive(Debug, JsonDeserialize)]
struct PolicySpec {
    max_balance: String,
    max_tx_value: String,
    expires_at_ms: u64,
}

#[derive(Debug, JsonSerialize)]
struct AllowanceSummary {
    label: String,
    certificate_id_hex: String,
    controller: String,
    allowance_asset: String,
    allowance_amount: String,
    commitment_hex: String,
    blinding_hex: String,
    attestation_bytes: usize,
    metadata_keys: Vec<String>,
    verdict_id_hex: Option<String>,
    attestation_nonce_hex: Option<String>,
    refresh_at_ms: Option<u64>,
    certificate_json: String,
    certificate_norito: String,
    register_instruction_json: String,
    register_instruction_norito: String,
    submitted: bool,
}

#[derive(Debug, JsonSerialize)]
struct AllowanceFixturesManifest {
    allowances: Vec<AllowanceSummary>,
}

fn write_manifest(output_root: &Path, allowances: Vec<AllowanceSummary>) -> Result<()> {
    let manifest = AllowanceFixturesManifest { allowances };
    let manifest_path = output_root.join("allowance_fixtures.manifest.json");
    write_json(&manifest_path, &manifest)
        .wrap_err_with(|| format!("failed to write manifest at {}", manifest_path.display()))
}

pub(crate) fn run(options: OfflineTopupOptions) -> Result<()> {
    let spec_bytes = fs::read(&options.spec_path)
        .wrap_err_with(|| format!("failed to read spec file {}", options.spec_path.display()))?;
    let spec: SpecFile = serde_json::from_slice(&spec_bytes)
        .wrap_err_with(|| format!("failed to parse JSON spec {}", options.spec_path.display()))?;
    if spec.allowances.is_empty() {
        return Err(eyre!(
            "spec `{}` contains no allowances",
            options.spec_path.display()
        ));
    }

    fs::create_dir_all(&options.output_root).wrap_err_with(|| {
        format!(
            "failed to create output directory {}",
            options.output_root.display()
        )
    })?;

    let base_dir = options
        .spec_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let register_opts = options.register.as_ref();
    let client = if let Some(opts) = register_opts {
        Some(init_client(opts)?)
    } else {
        None
    };

    let mut summaries = Vec::with_capacity(spec.allowances.len());
    for allowance in &spec.allowances {
        let summary = handle_allowance(
            allowance,
            spec.operator.as_ref(),
            options.operator_key_override.as_deref(),
            &options.output_root,
            &base_dir,
            register_opts,
            client.as_ref(),
        )?;
        summaries.push(summary);
    }

    write_manifest(&options.output_root, summaries)?;

    Ok(())
}

fn handle_allowance(
    allowance: &AllowanceSpec,
    operator_from_spec: Option<&OperatorSpec>,
    operator_override: Option<&str>,
    output_root: &Path,
    spec_root: &Path,
    register_opts: Option<&RegisterOptions>,
    client: Option<&Client>,
) -> Result<AllowanceSummary> {
    let label = allowance.label.trim();
    if label.is_empty() {
        return Err(eyre!("allowance entry is missing a non-empty `label`"));
    }

    let operator_key_spec = operator_override
        .or(allowance.operator_key.as_deref())
        .or_else(|| operator_from_spec.and_then(|op| op.private_key.as_deref()))
        .ok_or_else(|| eyre!("operator private key not provided for allowance `{label}`"))?;
    let operator_key = parse_private_key(operator_key_spec)
        .wrap_err_with(|| format!("invalid operator private key for allowance `{label}`"))?;

    let controller = AccountId::parse_encoded(&allowance.controller)
        .map_err(|err| {
            eyre!(
                "invalid `controller` value `{}` for allowance `{label}`: {}",
                allowance.controller,
                err.reason()
            )
        })?
        .into_account_id();
    let operator_account = allowance
        .operator
        .as_deref()
        .or_else(|| operator_from_spec.and_then(|op| op.account.as_deref()))
        .ok_or_else(|| eyre!("operator account not provided for allowance `{label}`"))?;
    let operator_account = AccountId::parse_encoded(operator_account)
        .map_err(|err| {
            eyre!(
                "invalid `operator` value `{}` for allowance `{label}`: {}",
                operator_account,
                err.reason()
            )
        })?
        .into_account_id();
    let asset = AssetId::from_str(&allowance.allowance_asset).wrap_err_with(|| {
        format!(
            "invalid `allowance_asset` `{}` for allowance `{label}`",
            allowance.allowance_asset
        )
    })?;
    if asset.account() != &controller {
        return Err(eyre!(
            "allowance `{label}` asset account must match controller ({} != {})",
            asset.account(),
            controller
        ));
    }
    let allowance_amount = parse_numeric(&allowance.amount)
        .wrap_err_with(|| format!("invalid `amount` for allowance `{label}`"))?;
    if allowance_amount <= Numeric::zero() {
        return Err(eyre!(
            "allowance `{label}` amount must be positive, got {allowance_amount}"
        ));
    }
    let policy = build_policy(&allowance.policy)
        .wrap_err_with(|| format!("invalid policy block for allowance `{label}`"))?;
    if policy.max_tx_value > policy.max_balance {
        return Err(eyre!(
            "allowance `{label}` policy max_tx_value exceeds max_balance"
        ));
    }
    if policy.max_balance < allowance_amount {
        return Err(eyre!(
            "allowance `{label}` policy max_balance ({}) is smaller than allowance amount ({allowance_amount})",
            policy.max_balance
        ));
    }
    if policy.expires_at_ms < allowance.expires_at_ms {
        return Err(eyre!(
            "allowance `{label}` policy expiry must cover certificate expiry ({} < {})",
            policy.expires_at_ms,
            allowance.expires_at_ms
        ));
    }
    let spend_public_key = parse_public_key(&allowance.spend_public_key).wrap_err_with(|| {
        format!(
            "invalid spend public key `{}` for allowance `{label}`",
            allowance.spend_public_key
        )
    })?;
    let metadata = build_metadata(
        allowance.metadata.as_ref(),
        allowance.metadata_file.as_ref().map(|p| spec_root.join(p)),
    )
    .wrap_err_with(|| format!("invalid metadata for allowance `{label}`"))?;
    let android_policy = validate_android_policy(&metadata)
        .wrap_err_with(|| format!("invalid android metadata for allowance `{label}`"))?;
    let provisioned_proof = allowance
        .provisioned_proof_file
        .as_ref()
        .map(|relative| {
            let resolved = spec_root.join(relative);
            load_provisioned_proof(&resolved).wrap_err_with(|| {
                format!(
                    "failed to load provisioned proof for allowance `{label}` from {}",
                    resolved.display()
                )
            })
        })
        .transpose()?;
    let verdict_id = parse_optional_hash("verdict_id_hex", allowance.verdict_id_hex.as_deref())?;
    let attestation_nonce = parse_optional_hash(
        "attestation_nonce_hex",
        allowance.attestation_nonce_hex.as_deref(),
    )?;
    let refresh_at_ms = allowance.refresh_at_ms;
    let verdict_id_hex = verdict_id.as_ref().map(|hash| encode_upper(hash.as_ref()));
    let attestation_nonce_hex = attestation_nonce
        .as_ref()
        .map(|hash| encode_upper(hash.as_ref()));
    if let Some(proof) = provisioned_proof.as_ref() {
        ensure_provisioned_metadata_matches(label, android_policy.as_ref(), proof)?;
    }
    let attestation_report = if let Some(proof) = provisioned_proof.as_ref() {
        to_bytes(proof).map_err(|err| {
            eyre!("failed to encode provisioned proof for allowance `{label}`: {err}")
        })?
    } else {
        build_attestation_bytes(allowance, spec_root)
            .wrap_err_with(|| format!("invalid attestation info for allowance `{label}`"))?
    };
    let blinding = resolve_blinding_bytes(allowance.blinding_hex.as_deref())?;
    let commitment = derive_commitment(&asset, &allowance_amount, &blinding)
        .wrap_err_with(|| format!("failed to derive commitment for allowance `{label}`"))?;

    let certificate = build_certificate(
        controller.clone(),
        operator_account.clone(),
        asset.clone(),
        allowance_amount,
        allowance.issued_at_ms,
        allowance.expires_at_ms,
        policy,
        spend_public_key,
        attestation_report.clone(),
        metadata.clone(),
        commitment.clone(),
        &operator_key,
        verdict_id,
        attestation_nonce,
        refresh_at_ms,
    )?;

    let certificate_id = certificate.certificate_id();
    let register_instruction = RegisterOfflineAllowance {
        certificate: certificate.clone(),
    };

    let allowance_dir = output_root.join(label);
    fs::create_dir_all(&allowance_dir).wrap_err_with(|| {
        format!(
            "failed to create output directory for allowance `{label}`: {}",
            allowance_dir.display()
        )
    })?;

    let certificate_json_path = allowance_dir.join("certificate.json");
    let certificate_norito_path = allowance_dir.join("certificate.norito");
    let register_json_path = allowance_dir.join("register_instruction.json");
    let register_norito_path = allowance_dir.join("register_instruction.norito");
    write_json(&certificate_json_path, &certificate_to_json(&certificate))?;
    write_norito_bytes(&certificate_norito_path, &certificate)?;
    write_json(
        &register_json_path,
        &register_instruction_to_json(&register_instruction),
    )?;
    write_norito_bytes(&register_norito_path, &register_instruction)?;

    let mut submitted = false;
    if let (Some(client), Some(opts)) = (client, register_opts) {
        submit_allowance(client, opts.mode, &register_instruction)?;
        submitted = true;
        println!(
            "registered allowance `{label}` (cert id {}) via {}",
            encode_upper(certificate_id.as_ref()),
            opts.config_path.display()
        );
    } else {
        println!(
            "generated allowance `{label}` (cert id {}) → {}",
            encode_upper(certificate_id.as_ref()),
            allowance_dir.display()
        );
    }

    let summary = AllowanceSummary {
        label: label.to_string(),
        certificate_id_hex: encode_upper(certificate_id.as_ref()),
        controller: controller.to_string(),
        allowance_asset: asset.to_string(),
        allowance_amount: allowance.amount.clone(),
        commitment_hex: encode_upper(&commitment),
        blinding_hex: encode_upper(&blinding),
        attestation_bytes: attestation_report.len(),
        metadata_keys: metadata.iter().map(|(name, _)| name.to_string()).collect(),
        verdict_id_hex,
        attestation_nonce_hex,
        refresh_at_ms,
        certificate_json: path_name(&certificate_json_path, output_root),
        certificate_norito: path_name(&certificate_norito_path, output_root),
        register_instruction_json: path_name(&register_json_path, output_root),
        register_instruction_norito: path_name(&register_norito_path, output_root),
        submitted,
    };
    let summary_path = allowance_dir.join("summary.json");
    write_json(&summary_path, &summary).wrap_err_with(|| {
        format!(
            "failed to write summary for allowance `{label}` at {}",
            summary_path.display()
        )
    })?;

    Ok(summary)
}

fn init_client(options: &RegisterOptions) -> Result<Client> {
    let config = Config::load(LoadPath::Explicit(&options.config_path)).map_err(|err| {
        eyre!(
            "failed to load client config {}: {err}",
            options.config_path.display()
        )
    })?;
    Ok(Client::new(config))
}

fn submit_allowance(
    client: &Client,
    mode: RegisterMode,
    instruction: &RegisterOfflineAllowance,
) -> Result<()> {
    match mode {
        RegisterMode::Blocking => {
            client.submit_blocking(instruction.clone())?;
        }
        RegisterMode::Immediate => {
            client.submit(instruction.clone())?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_certificate(
    controller: AccountId,
    operator: AccountId,
    asset: AssetId,
    amount: Numeric,
    issued_at_ms: u64,
    expires_at_ms: u64,
    policy: OfflineWalletPolicy,
    spend_public_key: PublicKey,
    attestation_report: Vec<u8>,
    metadata: Metadata,
    commitment: Vec<u8>,
    operator_key: &PrivateKey,
    verdict_id: Option<Hash>,
    attestation_nonce: Option<Hash>,
    refresh_at_ms: Option<u64>,
) -> Result<OfflineWalletCertificate> {
    let allowance = OfflineAllowanceCommitment {
        asset,
        amount,
        commitment,
    };
    let mut certificate = OfflineWalletCertificate {
        controller,
        operator,
        allowance,
        spend_public_key,
        attestation_report,
        issued_at_ms,
        expires_at_ms,
        policy,
        operator_signature: Signature::from_bytes(&[0; 64]),
        metadata,
        verdict_id,
        attestation_nonce,
        refresh_at_ms,
    };
    let payload = certificate
        .operator_signing_bytes()
        .wrap_err("failed to encode certificate payload")?;
    certificate.operator_signature = Signature::new(operator_key, &payload);
    Ok(certificate)
}

fn build_policy(spec: &PolicySpec) -> Result<OfflineWalletPolicy> {
    let max_balance = parse_numeric(&spec.max_balance)?;
    let max_tx = parse_numeric(&spec.max_tx_value)?;
    Ok(OfflineWalletPolicy {
        max_balance,
        max_tx_value: max_tx,
        expires_at_ms: spec.expires_at_ms,
    })
}

fn parse_numeric(input: &str) -> Result<Numeric> {
    Numeric::from_str(input).map_err(|err| eyre!(err))
}

fn validate_android_policy(metadata: &Metadata) -> Result<Option<AndroidIntegrityMetadata>> {
    AndroidIntegrityMetadata::from_metadata(metadata)
        .map_err(|err| eyre!("android metadata invalid: {err}"))
}

fn build_attestation_bytes(spec: &AllowanceSpec, base: &Path) -> Result<Vec<u8>> {
    if let Some(hex_blob) = spec.attestation_report_hex.as_deref() {
        return hex::decode(hex_blob)
            .map_err(|err| eyre!("invalid attestation hex payload: {err}"));
    }
    if let Some(b64) = spec.attestation_report_base64.as_deref() {
        return BASE64
            .decode(b64.as_bytes())
            .map_err(|err| eyre!("invalid attestation base64 payload: {err}"));
    }
    if let Some(path) = spec.attestation_report_file.as_ref() {
        let resolved = base.join(path);
        return fs::read(&resolved)
            .wrap_err_with(|| format!("failed to read attestation file {}", resolved.display()));
    }
    Ok(Vec::new())
}

fn resolve_blinding_bytes(source: Option<&str>) -> Result<Vec<u8>> {
    if let Some(hex_seed) = source {
        return hex::decode(hex_seed)
            .map_err(|err| eyre!("invalid blinding hex `{hex_seed}`: {err}"));
    }
    let mut buf = vec![0u8; 32];
    let mut rng = rand::rng();
    rng.fill_bytes(&mut buf);
    Ok(buf)
}

fn parse_optional_hash(field: &str, value: Option<&str>) -> Result<Option<Hash>> {
    value
        .map(|hex| Hash::from_str(hex).map_err(|err| eyre!("invalid {field} `{hex}`: {err}")))
        .transpose()
}

fn derive_commitment(asset: &AssetId, amount: &Numeric, blinding: &[u8]) -> Result<Vec<u8>> {
    let value_scalar = numeric_to_scalar(amount)
        .wrap_err("failed to map allowance amount into Pedersen scalar")?;
    let blind_scalar = derive_blinding_scalar(asset, blinding);
    let commitment =
        RISTRETTO_BASEPOINT_POINT * value_scalar + pedersen_generator_h() * blind_scalar;
    Ok(commitment.compress().as_bytes().to_vec())
}

fn pedersen_generator_h() -> &'static RistrettoPoint {
    &PEDERSEN_H
}

fn numeric_to_scalar(value: &Numeric) -> Result<Scalar> {
    let signed = value
        .try_mantissa_i128()
        .ok_or_else(|| eyre!("numeric mantissa exceeds range"))?;
    Ok(scalar_from_i128(signed))
}

fn scalar_from_i128(value: i128) -> Scalar {
    let magnitude = value.unsigned_abs();
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(&magnitude.to_le_bytes());
    let scalar = Scalar::from_bytes_mod_order(bytes);
    if value.is_negative() { -scalar } else { scalar }
}

fn derive_blinding_scalar(asset: &AssetId, seed: &[u8]) -> Scalar {
    let mut hasher = Blake2b512::new();
    hasher.update(BLINDING_DERIVE_LABEL);
    hasher.update(asset.to_string().as_bytes());
    hasher.update(seed);
    let digest = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&digest[..32]);
    Scalar::from_bytes_mod_order(bytes)
}

fn certificate_to_json(certificate: &OfflineWalletCertificate) -> JsonValue {
    let mut allowance = JsonMap::new();
    allowance.insert(
        "asset".to_string(),
        JsonValue::String(certificate.allowance.asset.to_string()),
    );
    allowance.insert(
        "amount".to_string(),
        JsonValue::String(certificate.allowance.amount.to_string()),
    );
    allowance.insert(
        "commitment_hex".to_string(),
        JsonValue::String(encode_upper(&certificate.allowance.commitment)),
    );

    let mut policy = JsonMap::new();
    policy.insert(
        "max_balance".to_string(),
        JsonValue::String(certificate.policy.max_balance.to_string()),
    );
    policy.insert(
        "max_tx_value".to_string(),
        JsonValue::String(certificate.policy.max_tx_value.to_string()),
    );
    policy.insert(
        "expires_at_ms".to_string(),
        JsonValue::from(certificate.policy.expires_at_ms),
    );

    let mut map = JsonMap::new();
    map.insert(
        "controller".to_string(),
        JsonValue::String(certificate.controller.to_string()),
    );
    map.insert(
        "operator".to_string(),
        JsonValue::String(certificate.operator.to_string()),
    );
    map.insert("allowance".to_string(), JsonValue::Object(allowance));
    map.insert(
        "spend_public_key".to_string(),
        JsonValue::String(certificate.spend_public_key.to_string()),
    );
    map.insert(
        "attestation_report_b64".to_string(),
        JsonValue::String(BASE64.encode(&certificate.attestation_report)),
    );
    map.insert(
        "issued_at_ms".to_string(),
        JsonValue::from(certificate.issued_at_ms),
    );
    map.insert(
        "expires_at_ms".to_string(),
        JsonValue::from(certificate.expires_at_ms),
    );
    map.insert("policy".to_string(), JsonValue::Object(policy));
    map.insert(
        "operator_signature_hex".to_string(),
        JsonValue::String(encode_upper(certificate.operator_signature.payload())),
    );
    map.insert(
        "metadata".to_string(),
        metadata_to_json(&certificate.metadata),
    );
    map.insert(
        "verdict_id_hex".to_string(),
        certificate
            .verdict_id
            .as_ref()
            .map(|hash| JsonValue::String(encode_upper(hash.as_ref())))
            .unwrap_or(JsonValue::Null),
    );
    map.insert(
        "attestation_nonce_hex".to_string(),
        certificate
            .attestation_nonce
            .as_ref()
            .map(|hash| JsonValue::String(encode_upper(hash.as_ref())))
            .unwrap_or(JsonValue::Null),
    );
    map.insert(
        "refresh_at_ms".to_string(),
        certificate
            .refresh_at_ms
            .map(JsonValue::from)
            .unwrap_or(JsonValue::Null),
    );
    JsonValue::Object(map)
}

fn register_instruction_to_json(isi: &RegisterOfflineAllowance) -> JsonValue {
    let mut map = JsonMap::new();
    map.insert(
        "instruction".to_string(),
        JsonValue::String("RegisterOfflineAllowance".to_string()),
    );
    map.insert(
        "certificate".to_string(),
        certificate_to_json(&isi.certificate),
    );
    JsonValue::Object(map)
}

fn metadata_to_json(metadata: &Metadata) -> JsonValue {
    let mut map = JsonMap::new();
    for (name, value) in metadata.iter() {
        let parsed = serde_json::from_str::<JsonValue>(value.get()).unwrap_or(JsonValue::Null);
        map.insert(name.to_string(), parsed);
    }
    JsonValue::Object(map)
}

#[allow(dead_code)]
fn metadata_key_list(metadata: &Metadata) -> Vec<String> {
    let mut keys = metadata
        .iter()
        .map(|(name, _)| name.to_string())
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

fn path_name(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

fn load_provisioned_proof(path: &Path) -> Result<AndroidProvisionedProof> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read provisioned proof {}", path.display()))?;
    let try_json = || {
        norito_json::from_slice::<AndroidProvisionedProof>(&bytes).map_err(|err| {
            eyre!(
                "failed to parse provisioned proof JSON {}: {err}",
                path.display()
            )
        })
    };
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
    {
        return try_json();
    }
    match from_bytes::<AndroidProvisionedProof>(&bytes) {
        Ok(archived) => AndroidProvisionedProof::try_deserialize(archived).map_err(|err| {
            eyre!(
                "failed to deserialize provisioned proof {}: {err}",
                path.display()
            )
        }),
        Err(err) => {
            eprintln!(
                "failed to decode Norito proof {} ({err}); attempting JSON fallback",
                path.display()
            );
            try_json()
        }
    }
}

fn ensure_provisioned_metadata_matches(
    label: &str,
    android_policy: Option<&AndroidIntegrityMetadata>,
    proof: &AndroidProvisionedProof,
) -> Result<()> {
    let config = match android_policy {
        Some(AndroidIntegrityMetadata::Provisioned(cfg)) => cfg,
        Some(_) => {
            return Err(eyre!(
                "allowance `{label}` supplies a provisioned proof but metadata policy is not `provisioned`"
            ));
        }
        None => {
            return Err(eyre!(
                "allowance `{label}` supplies a provisioned proof but does not declare android.integrity.policy"
            ));
        }
    };

    if proof.manifest_schema.trim() != config.manifest_schema {
        return Err(eyre!(
            "allowance `{label}` proof manifest_schema `{}` does not match metadata value `{}`",
            proof.manifest_schema,
            config.manifest_schema
        ));
    }
    if let Some(expected) = config.manifest_version {
        match proof.manifest_version {
            Some(actual) if actual == expected => {}
            Some(actual) => {
                return Err(eyre!(
                    "allowance `{label}` proof manifest_version {actual} does not match metadata value {expected}"
                ));
            }
            None => {
                return Err(eyre!(
                    "allowance `{label}` proof omits manifest_version but metadata requires {expected}"
                ));
            }
        }
    }
    if let Some(expected_digest) = config.manifest_digest {
        let actual_digest = proof
            .manifest_digest()
            .map_err(|err| eyre!("failed to hash provisioned manifest for `{label}`: {err}"))?;
        if actual_digest != expected_digest {
            return Err(eyre!(
                "allowance `{label}` proof manifest digest {} does not match metadata digest {}",
                encode_upper(actual_digest.as_ref()),
                encode_upper(expected_digest.as_ref())
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use curve25519_dalek::ristretto::CompressedRistretto;
    use iroha::data_model::{asset::AssetDefinitionId, name::Name};
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_primitives::json::Json;
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn validate_android_policy_accepts_marker_key() {
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("android.integrity.policy").expect("key"),
            Json::new("marker_key"),
        );
        metadata.insert(
            Name::from_str("android.attestation.package_names").expect("key"),
            Json::new(vec!["tech.app".to_string()]),
        );
        metadata.insert(
            Name::from_str("android.attestation.signing_digests_sha256").expect("key"),
            Json::new(vec![
                "11223344556677889900AABBCCDDEEFF11223344556677889900AABBCCDDEEFF".to_string(),
            ]),
        );
        let policy = validate_android_policy(&metadata).expect("android metadata valid");
        assert!(matches!(
            policy,
            Some(AndroidIntegrityMetadata::MarkerKey(_))
        ));
    }

    #[test]
    fn validate_android_policy_rejects_missing_entries() {
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("android.integrity.policy").expect("key"),
            Json::new("marker_key"),
        );
        let err = validate_android_policy(&metadata).expect_err("metadata invalid");
        assert!(
            err.to_string()
                .contains("android.attestation.package_names"),
            "error should mention missing package names"
        );
    }

    #[test]
    fn derive_commitment_is_deterministic() {
        let asset = sample_asset();
        let amount = Numeric::from_str("100").expect("numeric");
        let blinding = vec![0xAA; 32];
        let first = derive_commitment(&asset, &amount, &blinding).expect("commitment");
        let second = derive_commitment(&asset, &amount, &blinding).expect("commitment");
        assert_eq!(first, second);
    }

    #[test]
    fn derive_commitment_emits_valid_point() {
        let asset = sample_asset();
        let amount = Numeric::from_str("42.50").expect("numeric");
        let blinding = vec![0x42; 32];
        let bytes = derive_commitment(&asset, &amount, &blinding).expect("commitment");
        let compressed =
            CompressedRistretto::from_slice(&bytes).expect("compressed commitment bytes");
        assert!(
            compressed.decompress().is_some(),
            "commitment must be on curve"
        );
    }

    #[test]
    fn provisioned_proof_must_match_metadata() {
        let proof = sample_provisioned_proof();
        let digest = proof.manifest_digest().expect("digest");
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("android.integrity.policy").expect("key"),
            Json::new("provisioned"),
        );
        metadata.insert(
            Name::from_str("android.provisioned.inspector_public_key").expect("key"),
            Json::new("ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4"),
        );
        metadata.insert(
            Name::from_str("android.provisioned.manifest_schema").expect("key"),
            Json::new("offline_provisioning_v1"),
        );
        metadata.insert(
            Name::from_str("android.provisioned.manifest_version").expect("key"),
            Json::new(1),
        );
        metadata.insert(
            Name::from_str("android.provisioned.max_manifest_age_ms").expect("key"),
            Json::new(604_800_000u64),
        );
        metadata.insert(
            Name::from_str("android.provisioned.manifest_digest").expect("key"),
            Json::new(encode_upper(digest.as_ref())),
        );
        let policy = validate_android_policy(&metadata).expect("metadata valid");
        ensure_provisioned_metadata_matches("demo", policy.as_ref(), &proof)
            .expect("metadata and proof should align");
    }

    #[test]
    fn provisioned_metadata_mismatch_is_reported() {
        let proof = sample_provisioned_proof();
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("android.integrity.policy").expect("key"),
            Json::new("provisioned"),
        );
        metadata.insert(
            Name::from_str("android.provisioned.inspector_public_key").expect("key"),
            Json::new("ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4"),
        );
        metadata.insert(
            Name::from_str("android.provisioned.manifest_schema").expect("key"),
            Json::new("different_schema"),
        );
        let policy = validate_android_policy(&metadata).expect("metadata valid");
        let err = ensure_provisioned_metadata_matches("demo", policy.as_ref(), &proof)
            .expect_err("schema mismatch should be reported");
        assert!(
            err.to_string().contains("manifest_schema"),
            "error should mention manifest schema mismatch"
        );
    }

    #[test]
    fn load_provisioned_proof_supports_json() {
        let proof = sample_provisioned_proof();
        let mut file = NamedTempFile::new().expect("temp file");
        let mut rendered = norito_json::to_json_pretty(&proof).expect("serialize proof");
        rendered.push('\n');
        file.write_all(rendered.as_bytes()).expect("write proof");
        let decoded = load_provisioned_proof(file.path()).expect("load proof");
        assert_eq!(decoded.manifest_schema, proof.manifest_schema);
        assert_eq!(decoded.counter, proof.counter);
    }

    fn sample_asset() -> AssetId {
        let definition = AssetDefinitionId::from_str("usd#wonderland").expect("definition id");
        let key_pair = KeyPair::from_seed(vec![0xAB; 32], Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        AssetId::new(definition, account)
    }

    fn sample_provisioned_proof() -> AndroidProvisionedProof {
        let mut manifest = Metadata::default();
        manifest.insert(
            Name::from_str("android.provisioned.device_id").expect("key"),
            Json::new("retail-device-001"),
        );
        AndroidProvisionedProof {
            manifest_schema: "offline_provisioning_v1".to_string(),
            manifest_version: Some(1),
            manifest_issued_at_ms: 1_730_314_876_000,
            challenge_hash: Hash::new(b"demo-challenge"),
            counter: 7,
            device_manifest: manifest,
            inspector_signature: Signature::from_bytes(&[0u8; 64]),
        }
    }
}
