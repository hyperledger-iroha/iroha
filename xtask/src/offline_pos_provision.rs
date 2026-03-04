use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use eyre::{Context as _, Result, eyre};
use iroha::data_model::{
    account::AccountId,
    offline::{
        OfflinePosBackendRoot, OfflinePosProvisionManifest, OfflineVerdictRevocation,
        OfflineVerdictRevocationBundle, OfflineVerdictRevocationReason,
        ParseOfflineVerdictRevocationReasonError,
    },
};
use iroha_crypto::{Hash, PrivateKey, Signature};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json::{self as serde_json, Value as JsonValue},
};

use crate::offline_tooling::{
    build_metadata, parse_private_key, parse_public_key, write_json, write_norito_bytes,
    write_norito_json,
};

#[derive(Debug)]
pub(crate) struct OfflinePosProvisionOptions {
    pub spec_path: PathBuf,
    pub output_root: PathBuf,
    pub operator_key_override: Option<String>,
}

pub(crate) fn run(options: OfflinePosProvisionOptions) -> Result<()> {
    let spec_bytes = fs::read(&options.spec_path)
        .wrap_err_with(|| format!("failed to read spec file {}", options.spec_path.display()))?;
    let spec: SpecFile = serde_json::from_slice(&spec_bytes)
        .wrap_err_with(|| format!("failed to parse JSON spec {}", options.spec_path.display()))?;
    if spec.manifests.is_empty() {
        return Err(eyre!(
            "spec `{}` contains no manifests",
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

    let operator_account = AccountId::from_str(&spec.operator.account)
        .wrap_err_with(|| format!("invalid operator account `{}`", spec.operator.account))?;
    let mut summaries = Vec::with_capacity(spec.manifests.len());
    for manifest_spec in &spec.manifests {
        let summary = handle_manifest(
            manifest_spec,
            &spec.operator,
            &operator_account,
            options.operator_key_override.as_deref(),
            &base_dir,
            &options.output_root,
        )?;
        summaries.push(summary);
    }

    let mut revocation_summaries = Vec::with_capacity(spec.revocation_bundles.len());
    for bundle_spec in &spec.revocation_bundles {
        let summary = handle_revocation_bundle(
            bundle_spec,
            &spec.operator,
            &operator_account,
            options.operator_key_override.as_deref(),
            &base_dir,
            &options.output_root,
        )?;
        revocation_summaries.push(summary);
    }

    let manifest = ManifestFixturesManifest {
        manifests: summaries,
        revocation_bundles: revocation_summaries,
    };
    let manifest_path = options
        .output_root
        .join("pos_provision_fixtures.manifest.json");
    write_json(&manifest_path, &manifest)
        .wrap_err_with(|| format!("failed to write {}", manifest_path.display()))?;

    Ok(())
}

#[derive(Debug, JsonDeserialize)]
struct SpecFile {
    operator: OperatorSpec,
    manifests: Vec<ManifestSpec>,
    #[norito(default)]
    revocation_bundles: Vec<RevocationBundleSpec>,
}

#[derive(Debug, JsonDeserialize)]
struct OperatorSpec {
    account: String,
    #[norito(default)]
    private_key: Option<String>,
}

#[derive(Debug, JsonDeserialize)]
struct ManifestSpec {
    label: String,
    manifest_id: String,
    sequence: u32,
    published_at_ms: u64,
    valid_from_ms: u64,
    valid_until_ms: u64,
    #[norito(default)]
    rotation_hint_ms: Option<u64>,
    #[norito(default)]
    operator_account: Option<String>,
    #[norito(default)]
    operator_key: Option<String>,
    #[norito(default)]
    metadata: Option<JsonValue>,
    #[norito(default)]
    metadata_file: Option<PathBuf>,
    roots: Vec<RootSpec>,
}

#[derive(Debug, JsonDeserialize)]
struct RevocationBundleSpec {
    label: String,
    bundle_id: String,
    sequence: u32,
    published_at_ms: u64,
    #[norito(default)]
    operator_account: Option<String>,
    #[norito(default)]
    operator_key: Option<String>,
    #[norito(default)]
    metadata: Option<JsonValue>,
    #[norito(default)]
    metadata_file: Option<PathBuf>,
    #[norito(default)]
    revocations: Vec<RevocationEntrySpec>,
    #[norito(default)]
    revocations_file: Option<PathBuf>,
}

#[derive(Debug, Clone, JsonDeserialize)]
struct RevocationEntrySpec {
    verdict_id_hex: String,
    issuer: String,
    revoked_at_ms: u64,
    #[norito(default)]
    reason: Option<String>,
    #[norito(default)]
    note: Option<String>,
    #[norito(default)]
    metadata: Option<JsonValue>,
    #[norito(default)]
    metadata_file: Option<PathBuf>,
}

#[derive(Debug, JsonDeserialize)]
struct RootSpec {
    label: String,
    role: String,
    #[norito(default)]
    public_key: Option<String>,
    #[norito(default)]
    public_key_file: Option<PathBuf>,
    valid_from_ms: u64,
    valid_until_ms: u64,
    #[norito(default)]
    metadata: Option<JsonValue>,
    #[norito(default)]
    metadata_file: Option<PathBuf>,
}

#[derive(Debug, JsonSerialize, Clone)]
struct ManifestSummary {
    label: String,
    manifest_id: String,
    sequence: u32,
    published_at_ms: u64,
    valid_from_ms: u64,
    valid_until_ms: u64,
    rotation_hint_ms: Option<u64>,
    operator: String,
    backend_roots: Vec<BackendRootSummary>,
    files: ManifestFileSummary,
}

#[derive(Debug, JsonSerialize, Clone)]
struct ManifestFileSummary {
    manifest_json: String,
    manifest_norito: String,
    summary_json: String,
}

#[derive(Debug, JsonSerialize, Clone)]
struct BackendRootSummary {
    label: String,
    role: String,
    public_key: String,
    valid_from_ms: u64,
    valid_until_ms: u64,
}

#[derive(Debug, JsonSerialize)]
struct ManifestFixturesManifest {
    manifests: Vec<ManifestSummary>,
    #[norito(default)]
    revocation_bundles: Vec<RevocationBundleSummary>,
}

#[derive(Debug, JsonSerialize, Clone)]
struct RevocationBundleSummary {
    label: String,
    bundle_id: String,
    sequence: u32,
    published_at_ms: u64,
    operator: String,
    total_revocations: usize,
    files: RevocationBundleFileSummary,
}

#[derive(Debug, JsonSerialize, Clone)]
struct RevocationBundleFileSummary {
    bundle_json: String,
    bundle_norito: String,
    summary_json: String,
}

fn handle_manifest(
    spec: &ManifestSpec,
    operator_spec: &OperatorSpec,
    default_operator_account: &AccountId,
    operator_key_override: Option<&str>,
    spec_root: &Path,
    output_root: &Path,
) -> Result<ManifestSummary> {
    if spec.roots.is_empty() {
        return Err(eyre!(
            "manifest `{}` must contain at least one backend root",
            spec.label
        ));
    }
    let manifest_dir = output_root.join(&spec.label);
    fs::create_dir_all(&manifest_dir).wrap_err_with(|| {
        format!(
            "failed to create output directory {}",
            manifest_dir.display()
        )
    })?;

    let operator_account = if let Some(account) = spec.operator_account.as_deref() {
        AccountId::from_str(account)
            .wrap_err_with(|| format!("invalid operator account `{account}`"))?
    } else {
        default_operator_account.clone()
    };
    let operator_key = resolve_operator_key(
        operator_key_override,
        spec.operator_key.as_deref(),
        operator_spec.private_key.as_deref(),
        &spec.label,
    )?;

    let manifest_metadata = build_metadata(
        spec.metadata.as_ref(),
        spec.metadata_file.as_ref().map(|path| spec_root.join(path)),
    )
    .wrap_err_with(|| format!("invalid metadata for manifest `{}`", spec.label))?;

    let mut backend_roots = Vec::with_capacity(spec.roots.len());
    for root_spec in &spec.roots {
        let backend_root = build_backend_root(root_spec, spec_root)?;
        backend_roots.push(backend_root);
    }

    let mut manifest = OfflinePosProvisionManifest {
        manifest_id: spec.manifest_id.clone(),
        sequence: spec.sequence,
        published_at_ms: spec.published_at_ms,
        valid_from_ms: spec.valid_from_ms,
        valid_until_ms: spec.valid_until_ms,
        rotation_hint_ms: spec.rotation_hint_ms,
        operator: operator_account.clone(),
        backend_roots: backend_roots.clone(),
        metadata: manifest_metadata,
        operator_signature: Signature::from_bytes(&[0; 64]),
    };
    let payload = manifest
        .operator_signing_bytes()
        .wrap_err("failed to encode provisioning manifest payload")?;
    manifest.operator_signature = Signature::new(&operator_key, &payload);

    let manifest_json_path = manifest_dir.join("manifest.json");
    let manifest_norito_path = manifest_dir.join("manifest.norito");
    write_manifest_json_with_payload(&manifest_json_path, &manifest, &payload).wrap_err_with(
        || {
            format!(
                "failed to write provisioning manifest JSON {}",
                manifest_json_path.display()
            )
        },
    )?;
    write_norito_bytes(&manifest_norito_path, &manifest).wrap_err_with(|| {
        format!(
            "failed to write provisioning manifest Norito {}",
            manifest_norito_path.display()
        )
    })?;

    let local_summary = build_manifest_summary(
        spec,
        &operator_account,
        &backend_roots,
        ManifestFileSummary {
            manifest_json: "manifest.json".into(),
            manifest_norito: "manifest.norito".into(),
            summary_json: "summary.json".into(),
        },
    );
    let summary_path = manifest_dir.join("summary.json");
    write_json(&summary_path, &local_summary).wrap_err_with(|| {
        format!(
            "failed to write provisioning summary {}",
            summary_path.display()
        )
    })?;

    Ok(build_manifest_summary(
        spec,
        &operator_account,
        &backend_roots,
        ManifestFileSummary {
            manifest_json: path_name(&manifest_json_path, output_root),
            manifest_norito: path_name(&manifest_norito_path, output_root),
            summary_json: path_name(&summary_path, output_root),
        },
    ))
}

fn handle_revocation_bundle(
    spec: &RevocationBundleSpec,
    operator_spec: &OperatorSpec,
    default_operator_account: &AccountId,
    operator_key_override: Option<&str>,
    spec_root: &Path,
    output_root: &Path,
) -> Result<RevocationBundleSummary> {
    let bundle_dir = output_root.join(&spec.label);
    fs::create_dir_all(&bundle_dir)
        .wrap_err_with(|| format!("failed to create output directory {}", bundle_dir.display()))?;

    let operator_account = if let Some(account) = spec.operator_account.as_deref() {
        AccountId::from_str(account)
            .wrap_err_with(|| format!("invalid operator account `{account}`"))?
    } else {
        default_operator_account.clone()
    };
    let operator_key = resolve_operator_key(
        operator_key_override,
        spec.operator_key.as_deref(),
        operator_spec.private_key.as_deref(),
        &spec.label,
    )?;
    let bundle_metadata = build_metadata(
        spec.metadata.as_ref(),
        spec.metadata_file.as_ref().map(|path| spec_root.join(path)),
    )
    .wrap_err_with(|| format!("invalid metadata for revocation bundle `{}`", spec.label))?;

    let entry_specs = load_revocation_specs(spec, spec_root)?;
    if entry_specs.is_empty() {
        return Err(eyre!(
            "revocation bundle `{}` must contain at least one entry",
            spec.label
        ));
    }
    let mut revocations = Vec::with_capacity(entry_specs.len());
    for entry in &entry_specs {
        revocations.push(build_revocation_entry(entry, spec_root)?);
    }

    let mut bundle = OfflineVerdictRevocationBundle {
        bundle_id: spec.bundle_id.clone(),
        sequence: spec.sequence,
        published_at_ms: spec.published_at_ms,
        operator: operator_account.clone(),
        metadata: bundle_metadata,
        revocations: revocations.clone(),
        operator_signature: Signature::from_bytes(&[0; 64]),
    };
    let payload = bundle
        .operator_signing_bytes()
        .wrap_err("failed to encode revocation bundle payload")?;
    bundle.operator_signature = Signature::new(&operator_key, &payload);

    let bundle_json_path = bundle_dir.join("revocations.json");
    let bundle_norito_path = bundle_dir.join("revocations.norito");
    write_norito_json(&bundle_json_path, &bundle).wrap_err_with(|| {
        format!(
            "failed to write revocation bundle JSON {}",
            bundle_json_path.display()
        )
    })?;
    write_norito_bytes(&bundle_norito_path, &bundle).wrap_err_with(|| {
        format!(
            "failed to write revocation bundle Norito {}",
            bundle_norito_path.display()
        )
    })?;

    let local_summary = build_revocation_summary(
        spec,
        &operator_account,
        revocations.len(),
        RevocationBundleFileSummary {
            bundle_json: "revocations.json".into(),
            bundle_norito: "revocations.norito".into(),
            summary_json: "summary.json".into(),
        },
    );
    let summary_path = bundle_dir.join("summary.json");
    write_json(&summary_path, &local_summary).wrap_err_with(|| {
        format!(
            "failed to write revocation bundle summary {}",
            summary_path.display()
        )
    })?;

    Ok(build_revocation_summary(
        spec,
        &operator_account,
        revocations.len(),
        RevocationBundleFileSummary {
            bundle_json: path_name(&bundle_json_path, output_root),
            bundle_norito: path_name(&bundle_norito_path, output_root),
            summary_json: path_name(&summary_path, output_root),
        },
    ))
}

fn write_manifest_json_with_payload(
    path: &Path,
    manifest: &OfflinePosProvisionManifest,
    payload: &[u8],
) -> Result<()> {
    let encoded = BASE64.encode(payload);
    let rendered = norito::json::to_json_pretty(manifest)
        .wrap_err("failed to serialize provisioning manifest to JSON")?;
    let mut value: serde_json::Value =
        serde_json::from_str(&rendered).wrap_err("failed to parse provisioning manifest JSON")?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| eyre!("provisioning manifest JSON is not an object"))?;
    object.insert(
        "payload_base64".to_string(),
        serde_json::Value::String(encoded),
    );
    write_json(path, &value)
}

fn build_manifest_summary(
    spec: &ManifestSpec,
    operator: &AccountId,
    roots: &[OfflinePosBackendRoot],
    files: ManifestFileSummary,
) -> ManifestSummary {
    let backend_roots = roots
        .iter()
        .map(|root| BackendRootSummary {
            label: root.label.clone(),
            role: root.role.clone(),
            public_key: root.public_key.to_string(),
            valid_from_ms: root.valid_from_ms,
            valid_until_ms: root.valid_until_ms,
        })
        .collect();
    ManifestSummary {
        label: spec.label.clone(),
        manifest_id: spec.manifest_id.clone(),
        sequence: spec.sequence,
        published_at_ms: spec.published_at_ms,
        valid_from_ms: spec.valid_from_ms,
        valid_until_ms: spec.valid_until_ms,
        rotation_hint_ms: spec.rotation_hint_ms,
        operator: operator.to_string(),
        backend_roots,
        files,
    }
}

fn build_revocation_summary(
    spec: &RevocationBundleSpec,
    operator: &AccountId,
    total: usize,
    files: RevocationBundleFileSummary,
) -> RevocationBundleSummary {
    RevocationBundleSummary {
        label: spec.label.clone(),
        bundle_id: spec.bundle_id.clone(),
        sequence: spec.sequence,
        published_at_ms: spec.published_at_ms,
        operator: operator.to_string(),
        total_revocations: total,
        files,
    }
}

fn resolve_operator_key(
    cli_override: Option<&str>,
    manifest_override: Option<&str>,
    operator_default: Option<&str>,
    label: &str,
) -> Result<PrivateKey> {
    let key_spec = cli_override
        .or(manifest_override)
        .or(operator_default)
        .ok_or_else(|| {
            eyre!(
                "no operator private key supplied for manifest `{}`; provide --operator-key or populate the spec",
                label
            )
        })?;
    parse_private_key(key_spec).wrap_err("invalid operator private key")
}

fn build_backend_root(spec: &RootSpec, base_dir: &Path) -> Result<OfflinePosBackendRoot> {
    let key_literal = read_key_source(
        spec.public_key.as_deref(),
        spec.public_key_file
            .as_ref()
            .map(|path| base_dir.join(path)),
    )?;
    let public_key = parse_public_key(&key_literal)
        .wrap_err_with(|| format!("invalid backend public key for `{}`", spec.label))?;
    let metadata = build_metadata(
        spec.metadata.as_ref(),
        spec.metadata_file.as_ref().map(|path| base_dir.join(path)),
    )
    .wrap_err_with(|| format!("invalid metadata for backend root `{}`", spec.label))?;
    Ok(OfflinePosBackendRoot {
        label: spec.label.clone(),
        role: spec.role.clone(),
        public_key,
        valid_from_ms: spec.valid_from_ms,
        valid_until_ms: spec.valid_until_ms,
        metadata,
    })
}

fn read_key_source(inline: Option<&str>, file: Option<PathBuf>) -> Result<String> {
    match (inline, file) {
        (Some(value), None) => Ok(value.to_string()),
        (None, Some(path)) => {
            let contents = fs::read_to_string(&path)
                .wrap_err_with(|| format!("failed to read public key file {}", path.display()))?;
            Ok(contents.trim().to_string())
        }
        (Some(_), Some(_)) => Err(eyre!(
            "public_key and public_key_file cannot be supplied at the same time"
        )),
        (None, None) => Err(eyre!(
            "backend root must supply either `public_key` or `public_key_file`"
        )),
    }
}

fn load_revocation_specs(
    spec: &RevocationBundleSpec,
    spec_root: &Path,
) -> Result<Vec<RevocationEntrySpec>> {
    let inline = !spec.revocations.is_empty();
    let file = spec.revocations_file.is_some();
    if inline && file {
        return Err(eyre!(
            "revocation bundle `{}` cannot specify both inline entries and `revocations_file`",
            spec.label
        ));
    }
    if let Some(path) = spec.revocations_file.as_ref() {
        let resolved = spec_root.join(path);
        let bytes = fs::read(&resolved)
            .wrap_err_with(|| format!("failed to read revocations file {}", resolved.display()))?;
        let parsed: Vec<RevocationEntrySpec> =
            serde_json::from_slice(&bytes).wrap_err_with(|| {
                format!(
                    "revocations file {} must be a JSON array of entries",
                    resolved.display()
                )
            })?;
        Ok(parsed)
    } else {
        Ok(spec.revocations.clone())
    }
}

fn build_revocation_entry(
    entry: &RevocationEntrySpec,
    spec_root: &Path,
) -> Result<OfflineVerdictRevocation> {
    let verdict_id = parse_verdict_hash(&entry.verdict_id_hex)?.ok_or_else(|| {
        eyre!(
            "revocation entry must supply `verdict_id_hex` (received `{}`)",
            entry.verdict_id_hex
        )
    })?;
    let issuer = AccountId::from_str(&entry.issuer)
        .wrap_err_with(|| format!("invalid issuer account `{}`", entry.issuer))?;
    let reason = parse_revocation_reason(entry.reason.as_deref()).wrap_err_with(|| {
        format!(
            "invalid revocation reason for verdict `{}`",
            entry.verdict_id_hex
        )
    })?;
    let metadata = build_metadata(
        entry.metadata.as_ref(),
        entry
            .metadata_file
            .as_ref()
            .map(|path| spec_root.join(path)),
    )
    .wrap_err_with(|| {
        format!(
            "invalid metadata for revocation entry `{}`",
            entry.verdict_id_hex
        )
    })?;
    Ok(OfflineVerdictRevocation {
        verdict_id,
        issuer,
        revoked_at_ms: entry.revoked_at_ms,
        reason,
        note: entry.note.clone(),
        metadata,
    })
}

fn parse_verdict_hash(value: &str) -> Result<Option<Hash>> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Ok(None);
    }
    let upper = normalized.to_ascii_uppercase();
    Hash::from_str(&upper)
        .map(Some)
        .map_err(|err| eyre!("invalid verdict_id_hex `{value}`: {err}"))
}

fn parse_revocation_reason(
    value: Option<&str>,
) -> Result<OfflineVerdictRevocationReason, ParseOfflineVerdictRevocationReasonError> {
    if let Some(reason) = value {
        OfflineVerdictRevocationReason::from_str(&reason.trim().to_ascii_lowercase())
    } else {
        Ok(OfflineVerdictRevocationReason::Unspecified)
    }
}

fn path_name(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn read_key_errors_on_duplicates() {
        let path = NamedTempFile::new().expect("temp file");
        fs::write(path.path(), "ed25519:00").expect("write key");
        let err = read_key_source(Some("ed25519:aa"), Some(path.path().into()))
            .expect_err("duplicate sources must error");
        assert!(
            err.to_string()
                .contains("public_key and public_key_file cannot be supplied"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_key_supports_inline_value() {
        let value = read_key_source(Some("ed25519:aa"), None).expect("inline key");
        assert_eq!(value, "ed25519:aa");
    }

    #[test]
    fn read_key_supports_files() {
        let path = NamedTempFile::new().expect("temp file");
        fs::write(path.path(), "ed25519:bb\n").expect("write key");
        let value = read_key_source(None, Some(path.path().into())).expect("key");
        assert_eq!(value, "ed25519:bb");
    }
}
