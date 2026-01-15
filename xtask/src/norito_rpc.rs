use std::{
    collections::{BTreeMap, HashSet},
    fs,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use blake2::{Blake2bVar, digest::VariableOutput};
use eyre::{Context, Result, bail, eyre};
use hex::encode as hex_encode;
use iroha_data_model::{
    sns::{
        FreezeNameRequestV1, GovernanceHookV1, NameControllerV1, NameRecordV1, NameSelectorV1,
        NameStatus, PaymentProofV1, RegisterNameRequestV1, RegisterNameResponseV1,
        RenewNameRequestV1, ReservedAssignmentRequestV1, SuffixPolicyV1, TransferNameRequestV1,
        UpdateControllersRequestV1,
    },
    transaction::{SignedTransaction, signed::TransactionPayload},
};
use norito::{
    NoritoSerialize,
    json::{self, JsonDeserialize, JsonSerialize},
};
use sha2::{Digest as ShaDigest, Sha256};
use tempfile::tempdir;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{JsonTarget, workspace_root, write_json_output};

const CANONICAL_MANIFEST: &str = "fixtures/norito_rpc/transaction_fixtures.manifest.json";
const SCHEMA_HASH_MANIFEST: &str = "fixtures/norito_rpc/schema_hashes.json";
const SCHEMA_HASH_MANIFEST_BASENAME: &str = "schema_hashes.json";
const MANIFEST_BASENAME: &str = "transaction_fixtures.manifest.json";
const SDK_MANIFESTS: &[(&str, &str, bool)] = &[
    (
        "python",
        "python/iroha_python/tests/fixtures/transaction_fixtures.manifest.json",
        false,
    ),
    (
        "java",
        "java/iroha_android/src/test/resources/transaction_fixtures.manifest.json",
        true,
    ),
    (
        "swift",
        "IrohaSwift/Fixtures/transaction_fixtures.manifest.json",
        false,
    ),
];

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct NoritoRpcVerificationReport {
    generated_at: String,
    fixture_count: usize,
    canonical_manifest: ManifestDigestReport,
    schema_manifest: ManifestDigestReport,
    sdk_manifests: Vec<SdkManifestReport>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct ManifestDigestReport {
    path: String,
    sha256: String,
    blake3: String,
    bytes: u64,
}

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct SdkManifestReport {
    sdk: String,
    manifest: ManifestDigestReport,
}

pub struct FixtureOptions {
    pub fixtures_json: Option<PathBuf>,
    pub exporter_manifest: Option<PathBuf>,
    pub output_dir: Option<PathBuf>,
    pub selection_manifest: Option<PathBuf>,
    pub include_all: bool,
    pub check_encoded: bool,
}

impl FixtureOptions {
    fn resolve_paths(&self) -> Result<ResolvedFixtureOptions> {
        let root = workspace_root();
        let fixtures = self.fixtures_json.clone().unwrap_or_else(|| {
            root.join("java/iroha_android/src/test/resources/transaction_payloads.json")
        });
        if !fixtures.is_file() {
            return Err(eyre!(
                "fixtures JSON missing: {} (override with --fixtures)",
                fixtures.display()
            ));
        }
        let exporter = self
            .exporter_manifest
            .clone()
            .unwrap_or_else(|| root.join("scripts/export_norito_fixtures/Cargo.toml"));
        if !exporter.is_file() {
            return Err(eyre!(
                "exporter manifest missing: {} (override with --exporter)",
                exporter.display()
            ));
        }
        let output = self
            .output_dir
            .clone()
            .unwrap_or_else(|| root.join("fixtures/norito_rpc"));
        let selection = self
            .selection_manifest
            .clone()
            .unwrap_or_else(|| output.join(MANIFEST_BASENAME));
        Ok(ResolvedFixtureOptions {
            fixtures_json: fixtures,
            exporter_manifest: exporter,
            output_dir: output,
            manifest_path: selection,
        })
    }
}

struct ResolvedFixtureOptions {
    fixtures_json: PathBuf,
    exporter_manifest: PathBuf,
    output_dir: PathBuf,
    manifest_path: PathBuf,
}

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct SchemaHashManifest {
    version: u32,
    generated_at: String,
    entries: Vec<SchemaHashEntry>,
}

impl SchemaHashManifest {
    fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)?;
        Ok(json::from_slice(&bytes)?)
    }

    fn new_current() -> Self {
        let timestamp = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .expect("timestamp format must succeed");
        Self {
            version: 1,
            generated_at: timestamp,
            entries: schema_targets()
                .into_iter()
                .map(|target| SchemaHashEntry {
                    type_name: target.type_name.to_string(),
                    alias: target.alias.to_string(),
                    schema_hash: format_schema_hash(target.schema_hash),
                })
                .collect(),
        }
    }

    fn validate(&self) -> Result<()> {
        verify_schema_hash_manifest(self)
    }
}

#[derive(Debug, JsonSerialize, JsonDeserialize, PartialEq, Eq)]
struct SchemaHashEntry {
    type_name: String,
    alias: String,
    schema_hash: String,
}

struct SchemaTarget {
    type_name: &'static str,
    alias: &'static str,
    schema_hash: [u8; 16],
}

impl SchemaTarget {
    fn of<T: NoritoSerialize>() -> Self {
        let type_name = std::any::type_name::<T>();
        let alias = type_name.rsplit("::").next().unwrap_or(type_name);
        Self {
            type_name,
            alias,
            schema_hash: T::schema_hash(),
        }
    }
}

fn schema_targets() -> Vec<SchemaTarget> {
    let mut targets = vec![
        SchemaTarget::of::<SignedTransaction>(),
        SchemaTarget::of::<TransactionPayload>(),
        SchemaTarget::of::<RegisterNameRequestV1>(),
        SchemaTarget::of::<RegisterNameResponseV1>(),
        SchemaTarget::of::<RenewNameRequestV1>(),
        SchemaTarget::of::<TransferNameRequestV1>(),
        SchemaTarget::of::<UpdateControllersRequestV1>(),
        SchemaTarget::of::<FreezeNameRequestV1>(),
        SchemaTarget::of::<ReservedAssignmentRequestV1>(),
        SchemaTarget::of::<NameRecordV1>(),
        SchemaTarget::of::<NameControllerV1>(),
        SchemaTarget::of::<NameSelectorV1>(),
        SchemaTarget::of::<NameStatus>(),
        SchemaTarget::of::<SuffixPolicyV1>(),
        SchemaTarget::of::<PaymentProofV1>(),
        SchemaTarget::of::<GovernanceHookV1>(),
    ];
    targets.sort_by(|a, b| a.alias.cmp(b.alias));
    targets
}

fn write_schema_hash_manifest(path: &Path) -> Result<()> {
    let manifest = SchemaHashManifest::new_current();
    let json = json::to_json_pretty(&manifest)?;
    fs::write(path, format!("{json}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn verify_schema_hash_manifest(manifest: &SchemaHashManifest) -> Result<()> {
    if manifest.version != 1 {
        bail!(
            "unsupported schema hash manifest version {}; expected 1",
            manifest.version
        );
    }
    if OffsetDateTime::parse(&manifest.generated_at, &Rfc3339).is_err() {
        bail!("schema hash manifest has invalid generated_at timestamp");
    }

    let expected = schema_targets();
    if expected.len() != manifest.entries.len() {
        bail!(
            "schema hash manifest contains {} entries but {} were expected",
            manifest.entries.len(),
            expected.len()
        );
    }

    for (entry, target) in manifest.entries.iter().zip(expected.iter()) {
        if entry.type_name != target.type_name {
            bail!(
                "schema hash entry order mismatch: expected `{}`, found `{}`",
                target.type_name,
                entry.type_name
            );
        }
        if entry.alias != target.alias {
            bail!(
                "schema hash alias mismatch for `{}`: expected `{}`, found `{}`",
                entry.type_name,
                target.alias,
                entry.alias
            );
        }
        let parsed = parse_schema_hash_hex(&entry.schema_hash)?;
        if parsed != target.schema_hash {
            bail!(
                "schema hash mismatch for `{}`: expected {}, found {}",
                entry.type_name,
                format_schema_hash(target.schema_hash),
                entry.schema_hash
            );
        }
    }

    Ok(())
}

fn format_schema_hash(bytes: [u8; 16]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn parse_schema_hash_hex(input: &str) -> Result<[u8; 16]> {
    let trimmed = input.strip_prefix("0x").unwrap_or(input);
    let bytes = hex::decode(trimmed)?;
    if bytes.len() != 16 {
        bail!(
            "schema hash must be 16 bytes; entry `{input}` decodes to {} bytes",
            bytes.len()
        );
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(&bytes);
    Ok(out)
}

pub fn run_verify(json_out: Option<JsonTarget>) -> Result<()> {
    let report = build_verification_report()?;

    println!(
        "norito-rpc fixtures verified ({} entries)",
        report.fixture_count
    );

    if let Some(target) = json_out {
        let value = json::to_value(&report)
            .map_err(|err| eyre!("failed to encode verification report: {err}"))?;
        write_json_output(&value, target)
            .map_err(|err| eyre!("failed to write verification report: {err}"))?;
    }
    Ok(())
}

fn build_verification_report() -> Result<NoritoRpcVerificationReport> {
    let root = workspace_root();
    let canonical_path = root.join(CANONICAL_MANIFEST);
    let canonical = Manifest::load(&canonical_path)
        .with_context(|| format!("failed to read {}", canonical_path.display()))?;
    canonical
        .validate(Some(
            canonical_path
                .parent()
                .expect("manifest file should have parent directory"),
        ))
        .context("canonical manifest validation failed")?;

    let mut sdk_manifests = Vec::new();
    for (label, rel_path, enforce_parity) in SDK_MANIFESTS {
        let manifest_path = root.join(rel_path);
        let manifest_dir = manifest_path
            .parent()
            .ok_or_else(|| eyre!("manifest path {} has no parent", manifest_path.display()))?;
        let manifest = Manifest::load(&manifest_path)
            .with_context(|| format!("{label} manifest missing at {}", manifest_path.display()))?;
        manifest
            .validate(Some(manifest_dir))
            .with_context(|| format!("{label} manifest failed validation"))?;
        manifest
            .compare_with(&canonical)
            .map_err(|err| eyre!("{label} manifest diverges: {err}"))
            .or_else(|err| {
                if *enforce_parity {
                    Err(err)
                } else {
                    eprintln!("[norito-rpc] warning: {label} manifest parity skipped ({err})");
                    Ok(())
                }
            })?;
        sdk_manifests.push(SdkManifestReport {
            sdk: label.to_string(),
            manifest: manifest_digest(&manifest_path, &root)?,
        });
    }

    let schema_manifest_path = root.join(SCHEMA_HASH_MANIFEST);
    let schema_manifest = SchemaHashManifest::load(&schema_manifest_path)
        .with_context(|| format!("failed to read {}", schema_manifest_path.display()))?;
    schema_manifest
        .validate()
        .context("schema hash manifest validation failed")?;

    let canonical_manifest = manifest_digest(&canonical_path, &root)?;
    let schema_manifest = manifest_digest(&schema_manifest_path, &root)?;
    let timestamp = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("timestamp formatting must succeed");

    Ok(NoritoRpcVerificationReport {
        generated_at: timestamp,
        fixture_count: canonical.fixtures.len(),
        canonical_manifest,
        schema_manifest,
        sdk_manifests,
    })
}

pub fn generate_fixtures(options: FixtureOptions) -> Result<()> {
    let resolved = options.resolve_paths()?;
    fs::create_dir_all(&resolved.output_dir)
        .with_context(|| format!("failed to create {}", resolved.output_dir.display()))?;
    let temp_dir = tempdir().context("failed to create temporary directory")?;
    run_fixture_exporter(&resolved, temp_dir.path(), options.check_encoded)?;
    let generated_manifest_path = temp_dir.path().join(MANIFEST_BASENAME);
    let generated = Manifest::load(&generated_manifest_path).with_context(|| {
        format!(
            "failed to read generated manifest {}",
            generated_manifest_path.display()
        )
    })?;
    generated
        .validate(Some(temp_dir.path()))
        .context("generated manifest failed validation")?;

    let desired_names = if options.include_all {
        None
    } else if resolved.manifest_path.exists() {
        let existing = Manifest::load(&resolved.manifest_path)?;
        Some(
            existing
                .fixtures
                .iter()
                .map(|fixture| fixture.name.clone())
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };

    let selected = filter_fixtures(&generated, desired_names.as_deref())?;
    sync_norito_files(&selected, temp_dir.path(), &resolved.output_dir)?;
    let filtered_manifest = Manifest {
        fixtures: selected.clone(),
    };
    filtered_manifest
        .validate(Some(&resolved.output_dir))
        .context("final manifest validation failed")?;
    let manifest_json = json::to_json_pretty(&filtered_manifest)?;
    fs::write(&resolved.manifest_path, format!("{manifest_json}\n"))
        .with_context(|| format!("failed to write {}", resolved.manifest_path.display()))?;
    let schema_path = resolved.output_dir.join(SCHEMA_HASH_MANIFEST_BASENAME);
    write_schema_hash_manifest(&schema_path)
        .with_context(|| format!("failed to generate {}", schema_path.display()))?;

    println!(
        "norito-rpc fixtures regenerated: {} entries written to {}",
        filtered_manifest.fixtures.len(),
        resolved.manifest_path.display()
    );
    Ok(())
}

fn run_fixture_exporter(
    resolved: &ResolvedFixtureOptions,
    out_dir: &Path,
    check_encoded: bool,
) -> Result<()> {
    let root = workspace_root();
    let mut command = Command::new("cargo");
    command
        .arg("run")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(&resolved.exporter_manifest)
        .arg("--release")
        .arg("--")
        .arg("--fixtures")
        .arg(&resolved.fixtures_json)
        .arg("--write-fixtures")
        .arg("--out-dir")
        .arg(out_dir)
        .arg("--manifest")
        .arg(MANIFEST_BASENAME);
    if !check_encoded {
        command.arg("--check-encoded").arg("false");
    }
    let status = command
        .current_dir(&root)
        .status()
        .with_context(|| format!("failed to run {}", resolved.exporter_manifest.display()))?;
    if !status.success() {
        return Err(eyre!(
            "fixture exporter exited with status {}",
            status.code().unwrap_or(-1)
        ));
    }
    Ok(())
}

fn filter_fixtures(
    manifest: &Manifest,
    desired_names: Option<&[String]>,
) -> Result<Vec<FixtureEntry>> {
    if let Some(names) = desired_names {
        let mut filtered = Vec::with_capacity(names.len());
        for name in names {
            let entry = manifest
                .fixtures
                .iter()
                .find(|fixture| &fixture.name == name)
                .ok_or_else(|| {
                    eyre!(
                        "fixture '{}' missing from regenerated manifest; rerun exporter or update selection",
                        name
                    )
                })?;
            filtered.push(entry.clone());
        }
        Ok(filtered)
    } else {
        Ok(manifest.fixtures.clone())
    }
}

fn sync_norito_files(
    fixtures: &[FixtureEntry],
    source_dir: &Path,
    target_dir: &Path,
) -> Result<()> {
    fs::create_dir_all(target_dir)
        .with_context(|| format!("failed to create {}", target_dir.display()))?;
    let desired: HashSet<String> = fixtures
        .iter()
        .map(|fixture| fixture.name.clone())
        .collect();
    for fixture in fixtures {
        let src = source_dir.join(format!("{}.norito", fixture.name));
        if !src.is_file() {
            return Err(eyre!(
                "fixture '{}' missing generated payload at {}",
                fixture.name,
                src.display()
            ));
        }
        let dst = target_dir.join(format!("{}.norito", fixture.name));
        fs::copy(&src, &dst)
            .with_context(|| format!("failed to copy {} to {}", src.display(), dst.display()))?;
    }

    for entry in fs::read_dir(target_dir)
        .with_context(|| format!("failed to read entries from {}", target_dir.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("norito") {
            continue;
        }
        let stem = entry
            .path()
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(|s| s.to_string())
            .unwrap_or_default();
        if !desired.contains(stem.as_str()) {
            fs::remove_file(entry.path())
                .with_context(|| format!("failed to remove stale fixture {}", stem))?;
        }
    }
    Ok(())
}

fn manifest_digest(path: &Path, root: &Path) -> Result<ManifestDigestReport> {
    let digest = compute_file_digest(path)?;
    Ok(ManifestDigestReport {
        path: relative_path(root, path),
        sha256: digest.sha256,
        blake3: digest.blake3,
        bytes: digest.bytes,
    })
}

struct FileDigest {
    sha256: String,
    blake3: String,
    bytes: u64,
}

fn compute_file_digest(path: &Path) -> Result<FileDigest> {
    let mut file = File::open(path)
        .with_context(|| format!("failed to open {} for digesting", path.display()))?;
    let mut sha = Sha256::new();
    let mut blake = blake3::Hasher::new();
    let mut buf = [0u8; 8192];
    let mut total = 0u64;
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        sha.update(&buf[..read]);
        blake.update(&buf[..read]);
        total = total.saturating_add(read as u64);
    }
    Ok(FileDigest {
        sha256: hex_encode(sha.finalize()),
        blake3: blake.finalize().to_hex().to_string(),
        bytes: total,
    })
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct Manifest {
    fixtures: Vec<FixtureEntry>,
}

impl Manifest {
    fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)?;
        Ok(json::from_slice(&bytes)?)
    }

    fn validate(&self, base_dir: Option<&Path>) -> Result<()> {
        for fixture in &self.fixtures {
            fixture.validate(base_dir)?;
        }
        Ok(())
    }

    fn as_map(&self) -> BTreeMap<&str, FixtureComparable<'_>> {
        self.fixtures
            .iter()
            .map(|fixture| {
                (
                    fixture.name.as_str(),
                    FixtureComparable::from_entry(fixture),
                )
            })
            .collect()
    }

    fn compare_with(&self, canonical: &Manifest) -> Result<()> {
        let expected = canonical.as_map();
        let actual = self.as_map();

        let mut issues = Vec::new();

        for name in actual.keys() {
            if !expected.contains_key(name) {
                issues.push(format!("unexpected fixture '{name}'"));
            }
        }

        for (name, actual_entry) in &actual {
            if let Some(canonical_entry) = expected.get(name)
                && actual_entry != canonical_entry
            {
                issues.push(format!("fixture '{name}' differs from canonical"));
            }
        }

        if issues.is_empty() {
            Ok(())
        } else {
            Err(eyre!(issues.join("; ")))
        }
    }
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
struct FixtureEntry {
    name: String,
    authority: String,
    chain: String,
    creation_time_ms: u64,
    encoded_file: String,
    encoded_len: u64,
    signed_len: u64,
    payload_base64: String,
    payload_hash: String,
    signed_base64: String,
    signed_hash: String,
    #[norito(default)]
    nonce: Option<u32>,
    #[norito(default)]
    time_to_live_ms: Option<u64>,
}

impl FixtureEntry {
    fn validate(&self, base_dir: Option<&Path>) -> Result<()> {
        let payload_bytes = BASE64
            .decode(&self.payload_base64)
            .with_context(|| format!("fixture '{}' payload base64 invalid", self.name))?;
        if payload_bytes.len() != self.encoded_len as usize {
            bail!(
                "fixture '{}' payload length mismatch (manifest={}, decoded={})",
                self.name,
                self.encoded_len,
                payload_bytes.len()
            );
        }
        let payload_hash = blake2b256_hex(&payload_bytes);
        if payload_hash != self.payload_hash {
            bail!(
                "fixture '{}' payload hash mismatch (manifest={}, computed={})",
                self.name,
                self.payload_hash,
                payload_hash
            );
        }

        let signed_bytes = BASE64
            .decode(&self.signed_base64)
            .with_context(|| format!("fixture '{}' signed base64 invalid", self.name))?;
        if signed_bytes.len() != self.signed_len as usize {
            bail!(
                "fixture '{}' signed payload length mismatch (manifest={}, decoded={})",
                self.name,
                self.signed_len,
                signed_bytes.len()
            );
        }
        let signed_hash = blake2b256_hex(&signed_bytes);
        if signed_hash != self.signed_hash {
            bail!(
                "fixture '{}' signed hash mismatch (manifest={}, computed={})",
                self.name,
                self.signed_hash,
                signed_hash
            );
        }

        if let Some(dir) = base_dir {
            let path = dir.join(&self.encoded_file);
            let file_bytes =
                fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
            if file_bytes != payload_bytes {
                bail!(
                    "fixture '{}' encoded file '{}' differs from manifest payload",
                    self.name,
                    path.display()
                );
            }
        }

        Ok(())
    }
}

fn blake2b256_hex(bytes: &[u8]) -> String {
    let mut hasher = Blake2bVar::new(32).expect("32-byte BLAKE2b digest");
    blake2::digest::Update::update(&mut hasher, bytes);
    let mut out = [0_u8; 32];
    hasher
        .finalize_variable(&mut out)
        .expect("finalize BLAKE2b digest");
    out[out.len() - 1] |= 1;
    hex_encode(out)
}

#[derive(Debug, PartialEq, Eq)]
struct FixtureComparable<'a> {
    authority: &'a str,
    chain: &'a str,
    creation_time_ms: u64,
    encoded_file: &'a str,
    encoded_len: u64,
    signed_len: u64,
    payload_base64: &'a str,
    signed_base64: &'a str,
    payload_hash: &'a str,
    signed_hash: &'a str,
    nonce: Option<u32>,
    time_to_live_ms: Option<u64>,
}

impl<'a> FixtureComparable<'a> {
    fn from_entry(entry: &'a FixtureEntry) -> Self {
        Self {
            authority: &entry.authority,
            chain: &entry.chain,
            creation_time_ms: entry.creation_time_ms,
            encoded_file: &entry.encoded_file,
            encoded_len: entry.encoded_len,
            signed_len: entry.signed_len,
            payload_base64: &entry.payload_base64,
            signed_base64: &entry.signed_base64,
            payload_hash: &entry.payload_hash,
            signed_hash: &entry.signed_hash,
            nonce: entry.nonce,
            time_to_live_ms: entry.time_to_live_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn sample_manifest() -> Manifest {
        Manifest {
            fixtures: vec![fixture("alpha"), fixture("beta"), fixture("gamma")],
        }
    }

    fn fixture(name: &str) -> FixtureEntry {
        FixtureEntry {
            name: name.to_string(),
            authority: "alice@wonderland".into(),
            chain: "00000001".into(),
            creation_time_ms: 1_735_000_000_000,
            encoded_file: format!("{name}.norito"),
            encoded_len: 1,
            signed_len: 1,
            payload_base64: "AA==".into(),
            signed_base64: "AA==".into(),
            payload_hash: format!("payload-{name}"),
            signed_hash: format!("signed-{name}"),
            nonce: None,
            time_to_live_ms: None,
        }
    }

    #[test]
    fn filter_fixtures_respects_selection_order() {
        let manifest = sample_manifest();
        let selection = vec!["gamma".to_string(), "alpha".to_string()];
        let filtered = filter_fixtures(&manifest, Some(&selection)).expect("filter succeeds");
        let names: Vec<_> = filtered.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, ["gamma", "alpha"]);
    }

    #[test]
    fn filter_fixtures_errors_on_missing_entries() {
        let manifest = sample_manifest();
        let selection = vec!["delta".to_string()];
        let err = filter_fixtures(&manifest, Some(&selection)).expect_err("missing fixture");
        assert!(
            err.to_string().contains("delta"),
            "error should mention missing fixture"
        );
    }

    #[test]
    fn filter_fixtures_returns_all_when_unfiltered() {
        let manifest = sample_manifest();
        let filtered = filter_fixtures(&manifest, None).expect("all fixtures");
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn compare_with_allows_subset() {
        let canonical = Manifest {
            fixtures: vec![fixture("alpha"), fixture("beta")],
        };
        let subset = Manifest {
            fixtures: vec![fixture("alpha")],
        };
        subset
            .compare_with(&canonical)
            .expect("subset manifests should compare cleanly");
    }

    #[test]
    fn compare_with_rejects_unexpected_entries() {
        let canonical = Manifest {
            fixtures: vec![fixture("alpha")],
        };
        let extra = Manifest {
            fixtures: vec![fixture("alpha"), fixture("gamma")],
        };
        let err = extra
            .compare_with(&canonical)
            .expect_err("extra fixtures should fail comparison");
        assert!(
            err.to_string().contains("unexpected fixture 'gamma'"),
            "error should mention unexpected fixture: {err}"
        );
    }

    #[test]
    fn compare_with_rejects_creation_time_drift() {
        let canonical = Manifest {
            fixtures: vec![fixture("alpha")],
        };
        let mut drift_entry = fixture("alpha");
        drift_entry.creation_time_ms += 1;
        let drift = Manifest {
            fixtures: vec![drift_entry],
        };
        let err = drift
            .compare_with(&canonical)
            .expect_err("creation_time_ms drift should fail comparison");
        assert!(
            err.to_string().contains("fixture 'alpha' differs"),
            "error should mention fixture mismatch: {err}"
        );
    }

    #[test]
    fn schema_targets_are_sorted_and_unique() {
        let targets = schema_targets();
        assert!(
            !targets.is_empty(),
            "expected schema target list to be non-empty"
        );
        for pair in targets.windows(2) {
            assert!(
                pair[0].alias <= pair[1].alias,
                "schema targets must be sorted by alias"
            );
        }
        let mut seen = HashSet::new();
        for target in &targets {
            assert!(
                seen.insert(target.type_name),
                "duplicate schema target `{}` detected",
                target.type_name
            );
        }
    }

    #[test]
    fn schema_manifest_round_trip_validates() {
        let manifest = SchemaHashManifest::new_current();
        manifest.validate().expect("generated manifest validates");
    }

    #[test]
    fn schema_hash_hex_round_trip() {
        let target_binding = schema_targets();
        let target = target_binding.first().expect("at least one schema target");
        let encoded = format_schema_hash(target.schema_hash);
        let decoded = parse_schema_hash_hex(&encoded).expect("decode succeeds");
        assert_eq!(decoded, target.schema_hash);
    }

    #[test]
    fn verification_report_lists_expected_sdks() {
        let report = build_verification_report().expect("report");
        assert!(report.fixture_count > 0);
        let mut labels: Vec<&str> = report
            .sdk_manifests
            .iter()
            .map(|entry| entry.sdk.as_str())
            .collect();
        labels.sort();
        for expected in ["java", "python", "swift"] {
            assert!(
                labels.contains(&expected),
                "expected SDK label {expected:?} to appear in verification report"
            );
        }
    }

    #[test]
    fn manifest_validation_checks_encoded_files_with_base_dir() {
        let root = workspace_root();
        let canonical_path = root.join(CANONICAL_MANIFEST);
        let canonical = Manifest::load(&canonical_path).expect("canonical manifest loads");
        let template = canonical
            .fixtures
            .first()
            .expect("at least one fixture in canonical manifest")
            .clone();

        // Happy path: point the fixture at a custom file in a temporary directory.
        let temp_dir = tempdir().expect("temp dir");
        let mut entry = template.clone();
        entry.encoded_file = "custom_payload.norito".into();
        let payload_bytes = BASE64
            .decode(&entry.payload_base64)
            .expect("payload payload_base64 decodes");
        let encoded_path = temp_dir.path().join(&entry.encoded_file);
        fs::write(&encoded_path, &payload_bytes).expect("write encoded payload");
        let manifest = Manifest {
            fixtures: vec![entry.clone()],
        };
        manifest
            .validate(Some(temp_dir.path()))
            .expect("validation succeeds when encoded file matches payload");

        // Corrupt the file and ensure validation fails.
        fs::write(&encoded_path, b"corrupt-payload").expect("corrupt encoded payload");
        let err = manifest
            .validate(Some(temp_dir.path()))
            .expect_err("corrupted payloads should be rejected");
        assert!(
            err.to_string().contains("differs from manifest payload"),
            "error should mention payload mismatch: {err}"
        );

        // Remove the file entirely to verify missing file errors.
        fs::remove_file(&encoded_path).expect("remove encoded payload");
        let err = manifest
            .validate(Some(temp_dir.path()))
            .expect_err("missing encoded payloads should be rejected");
        assert!(
            err.to_string().contains("custom_payload.norito"),
            "error should mention missing encoded file name: {err}"
        );
    }
}
