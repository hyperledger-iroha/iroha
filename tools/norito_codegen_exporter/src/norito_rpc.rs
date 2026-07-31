//! Canonical Norito RPC fixture generation and verification.
//!
//! This module owns the fixture bytes, schema-hash validation, and SDK manifest
//! parity checks. Repository-facing command wrappers should delegate here so
//! every caller exercises the same implementation.

use std::{
    collections::{BTreeMap, HashSet},
    fs,
    fs::File,
    io::Read,
    num::NonZeroU32,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use blake2::{Blake2bVar, digest::VariableOutput};
use eyre::{Context, Result, bail, eyre};
use hex::encode as hex_encode;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    isi::{Instruction, InstructionBox, decode_instruction_from_pair, frame_instruction_payload},
    metadata::Metadata,
    name::Name,
    sns::{NameControllerV1, NameRecordV1, NameSelectorV1, NameStatus, SuffixPolicyV1},
    transaction::{
        Executable, ExecutableBatchItem, FeePaymentIntent, IvmBytecode, SignedTransaction,
        TransactionBuilder, executable::ContractInvocation, signed::TransactionPayload,
    },
};
use iroha_primitives::json::Json;
use norito::{
    NoritoSerialize,
    codec::{Decode, Encode},
    core::DecodeFromSlice,
    json::{self, JsonDeserialize, JsonSerialize, Map, Number, Value},
};
use sha2::{Digest as ShaDigest, Sha256};
use tempfile::tempdir;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const CANONICAL_MANIFEST: &str = "fixtures/norito_rpc/transaction_fixtures.manifest.json";
const SCHEMA_HASH_MANIFEST: &str = "fixtures/norito_rpc/schema_hashes.json";
const SCHEMA_HASH_MANIFEST_BASENAME: &str = "schema_hashes.json";
const MANIFEST_BASENAME: &str = "transaction_fixtures.manifest.json";
const COMPACT_HASH_VECTOR_BASENAME: &str = "iroha_compact_hash_vector.properties";
const COMPACT_HASH_VECTOR_SOURCE: &str = "transfer_asset";
const SIGNED_TRANSACTION_V1: u8 = 1;
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

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("exporter resides under the workspace tools directory")
        .to_path_buf()
}

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

/// Destination for the optional Norito RPC verification report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JsonOutput {
    /// Write the report to standard output.
    Stdout,
    /// Write the report to the provided file, creating parent directories.
    File(PathBuf),
}

/// Inputs and output selection for fixture regeneration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureOptions {
    /// Source JSON containing transaction fixture definitions.
    fixtures_json: Option<PathBuf>,
    /// Deprecated exporter manifest argument retained for CLI compatibility.
    exporter_manifest: Option<PathBuf>,
    /// Directory receiving canonical manifests and encoded payloads.
    output_dir: Option<PathBuf>,
    /// Existing manifest whose entries select the fixtures to retain.
    selection_manifest: Option<PathBuf>,
    /// Emit every generated fixture instead of applying the selection manifest.
    include_all: bool,
    /// Compare generated payload bytes with encoded hints from the source JSON.
    check_encoded: bool,
}

impl FixtureOptions {
    /// Create fixture-generation options from the established `xtask` inputs.
    pub fn new(
        fixtures_json: Option<PathBuf>,
        exporter_manifest: Option<PathBuf>,
        output_dir: Option<PathBuf>,
        selection_manifest: Option<PathBuf>,
        include_all: bool,
        check_encoded: bool,
    ) -> Self {
        Self {
            fixtures_json,
            exporter_manifest,
            output_dir,
            selection_manifest,
            include_all,
            check_encoded,
        }
    }

    fn resolve_paths(&self) -> Result<ResolvedFixtureOptions> {
        let root = workspace_root();
        if self.exporter_manifest.is_some() {
            eprintln!(
                "[norito-rpc] warning: --exporter is deprecated and ignored; fixture exporter is built into xtask"
            );
        }
        let fixtures = self.fixtures_json.clone().unwrap_or_else(|| {
            root.join("java/iroha_android/src/test/resources/transaction_payloads.json")
        });
        if !fixtures.is_file() {
            return Err(eyre!(
                "fixtures JSON missing: {} (override with --fixtures)",
                fixtures.display()
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
            output_dir: output,
            manifest_path: selection,
        })
    }
}

struct ResolvedFixtureOptions {
    fixtures_json: PathBuf,
    output_dir: PathBuf,
    manifest_path: PathBuf,
}

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct SchemaHashManifest {
    version: u32,
    entries: Vec<SchemaHashEntry>,
}

impl SchemaHashManifest {
    fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)?;
        Ok(json::from_slice(&bytes)?)
    }

    fn new_current() -> Self {
        Self {
            version: 1,
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
        SchemaTarget::of::<NameRecordV1>(),
        SchemaTarget::of::<NameControllerV1>(),
        SchemaTarget::of::<NameSelectorV1>(),
        SchemaTarget::of::<NameStatus>(),
        SchemaTarget::of::<SuffixPolicyV1>(),
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

/// Verify canonical fixture bytes, schema hashes, and SDK manifest parity.
pub fn run_verify(json_out: Option<JsonOutput>) -> Result<()> {
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

fn write_json_output(value: &Value, target: JsonOutput) -> Result<()> {
    let mut json_text = json::to_string_pretty(value)?;
    json_text.push('\n');
    match target {
        JsonOutput::Stdout => print!("{json_text}"),
        JsonOutput::File(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, json_text)?;
        }
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
    let compact_hash_vector_path = canonical_path
        .parent()
        .expect("manifest file should have parent directory")
        .join(COMPACT_HASH_VECTOR_BASENAME);
    verify_compact_hash_vector(&compact_hash_vector_path, &canonical.fixtures)?;

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

/// Regenerate canonical Norito RPC fixtures from the configured source JSON.
pub fn generate_fixtures(options: FixtureOptions) -> Result<()> {
    let resolved = options.resolve_paths()?;
    fs::create_dir_all(&resolved.output_dir)
        .with_context(|| format!("failed to create {}", resolved.output_dir.display()))?;
    let temp_dir = tempdir().context("failed to create temporary directory")?;
    generate_fixture_artifacts(&resolved, temp_dir.path(), options.check_encoded)?;
    let generated_manifest_path = temp_dir.path().join(MANIFEST_BASENAME);
    let generated = Manifest::load(&generated_manifest_path).with_context(|| {
        format!(
            "failed to read generated manifest {}",
            generated_manifest_path.display()
        )
    })?;
    generated
        .validate(Some(temp_dir.path()))
        .map_err(|err| eyre!("generated manifest failed validation: {err}"))?;

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
    let compact_hash_vector = render_compact_hash_vector(&selected)?;
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
    let compact_hash_vector_path = resolved.output_dir.join(COMPACT_HASH_VECTOR_BASENAME);
    fs::write(&compact_hash_vector_path, compact_hash_vector)
        .with_context(|| format!("failed to generate {}", compact_hash_vector_path.display()))?;

    println!(
        "norito-rpc fixtures regenerated: {} entries written to {}",
        filtered_manifest.fixtures.len(),
        resolved.manifest_path.display()
    );
    Ok(())
}

fn generate_fixture_artifacts(
    resolved: &ResolvedFixtureOptions,
    out_dir: &Path,
    check_encoded: bool,
) -> Result<()> {
    let fixtures_text = fs::read_to_string(&resolved.fixtures_json)
        .with_context(|| format!("failed to read {}", resolved.fixtures_json.display()))?;
    let fixtures_value: Value =
        json::from_str(&fixtures_text).context("invalid transaction_payloads fixtures JSON")?;
    let raw_fixtures = parse_payload_fixtures(&fixtures_value)?;
    let keypair = signing_keypair()?;

    let mut fixtures = Vec::with_capacity(raw_fixtures.len());
    for raw in &raw_fixtures {
        fixtures.push(raw.generate_fixture(&keypair, check_encoded)?);
    }

    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;
    for fixture in &fixtures {
        let norito_path = out_dir.join(format!("{}.norito", fixture.name));
        fs::write(&norito_path, &fixture.payload_bytes)
            .with_context(|| format!("failed to write {}", norito_path.display()))?;
    }

    let manifest = Manifest {
        fixtures: fixtures.iter().map(Fixture::to_entry).collect(),
    };
    let manifest_json = json::to_json_pretty(&manifest)?;
    fs::write(
        out_dir.join(MANIFEST_BASENAME),
        format!("{manifest_json}\n"),
    )
    .context("failed to write generated fixture manifest")?;

    // Refresh the selected source file's generated hints.
    let updated_payloads = build_payload_fixtures_json(&raw_fixtures, &fixtures)?;
    let payloads_json = json::to_json_pretty(&updated_payloads)?;
    fs::write(&resolved.fixtures_json, format!("{payloads_json}\n"))
        .with_context(|| format!("failed to write {}", resolved.fixtures_json.display()))?;

    Ok(())
}

const SIGNING_SEED_HEX: &str = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";

fn signing_keypair() -> Result<KeyPair> {
    let seed = hex::decode(SIGNING_SEED_HEX).context("invalid signing seed hex")?;
    KeyPair::try_from_seed(seed, Algorithm::Ed25519)
        .map_err(|err| eyre!("failed to derive Norito RPC fixture signing key: {err}"))
}

#[derive(Clone)]
struct RawPayloadFixture {
    name: String,
    payload: RawPayload,
    payload_json: Value,
    encoded_hint: Option<String>,
    chain_hint: Option<String>,
    authority_hint: Option<String>,
    creation_time_ms_hint: Option<u64>,
    ttl_ms_hint: u64,
    nonce_hint: Option<u32>,
}

#[derive(Clone)]
struct RawPayload {
    chain: String,
    authority: String,
    creation_time_ms: u64,
    executable: RawExecutable,
    ttl_ms: u64,
    nonce: Option<u32>,
    fee_payment: FeePaymentIntent,
    metadata: Vec<(Name, Json)>,
}

#[derive(Clone)]
enum RawExecutable {
    Ivm(Vec<u8>),
    Instructions(Vec<RawInstruction>),
    Batch(Vec<RawBatchItem>),
}

#[derive(Clone)]
enum RawBatchItem {
    Instruction(RawInstruction),
    ContractCall(ContractInvocation),
}

#[derive(Clone)]
struct RawInstruction {
    wire_name: String,
    payload_base64: String,
}

struct Fixture {
    name: String,
    payload_bytes: Vec<u8>,
    signed_bytes: Vec<u8>,
    summary: PayloadSummary,
}

struct PayloadSummary {
    chain: String,
    authority: String,
    creation_time_ms: u64,
    ttl_ms: u64,
    nonce: Option<u32>,
    payload_base64: String,
    signed_base64: String,
    payload_hash_hex: String,
    signed_hash_hex: String,
}

struct WireInstructionPayload {
    wire_name: String,
    payload_base64: String,
}

impl RawPayloadFixture {
    fn generate_fixture(&self, keypair: &KeyPair, check_encoded: bool) -> Result<Fixture> {
        if let Some(chain_hint) = &self.chain_hint
            && chain_hint != &self.payload.chain
        {
            bail!(
                "fixture '{}' chain mismatch: expected {}, got {}",
                self.name,
                chain_hint,
                self.payload.chain
            );
        }
        if let Some(authority_hint) = &self.authority_hint {
            let expected = normalize_authority_hint(authority_hint);
            let actual = normalize_authority_hint(&self.payload.authority);
            if expected != actual {
                bail!(
                    "fixture '{}' authority mismatch: expected {}, got {}",
                    self.name,
                    authority_hint,
                    self.payload.authority
                );
            }
        }
        if let Some(creation_hint) = self.creation_time_ms_hint
            && creation_hint != self.payload.creation_time_ms
        {
            bail!(
                "fixture '{}' creation_time_ms mismatch: expected {}, got {}",
                self.name,
                creation_hint,
                self.payload.creation_time_ms
            );
        }
        if self.ttl_ms_hint != self.payload.ttl_ms {
            bail!(
                "fixture '{}' time_to_live_ms mismatch: expected {}, got {}",
                self.name,
                self.ttl_ms_hint,
                self.payload.ttl_ms
            );
        }
        if let Some(nonce_hint) = self.nonce_hint
            && Some(nonce_hint) != self.payload.nonce
        {
            bail!(
                "fixture '{}' nonce mismatch: expected {}, got {:?}",
                self.name,
                nonce_hint,
                self.payload.nonce
            );
        }

        let builder = self
            .payload
            .to_builder()
            .with_context(|| format!("failed to build Norito RPC fixture '{}'", self.name))?;
        let signed = builder.try_sign(keypair.private_key()).map_err(|err| {
            eyre!(
                "failed to sign Norito RPC transaction fixture '{}': {err}",
                self.name
            )
        })?;
        let payload_value = signed.payload().clone();
        let actual_ttl = payload_value.time_to_live().ok_or_else(|| {
            eyre!(
                "fixture '{}' is missing time_to_live_ms after construction",
                self.name
            )
        })?;
        let actual_ttl_ms = u64::try_from(actual_ttl.as_millis()).map_err(|_| {
            eyre!(
                "fixture '{}' time_to_live_ms exceeds u64 after construction",
                self.name
            )
        })?;
        if actual_ttl_ms != self.payload.ttl_ms {
            bail!(
                "fixture '{}' time_to_live_ms changed during construction: expected {}, got {}",
                self.name,
                self.payload.ttl_ms,
                actual_ttl_ms
            );
        }
        let payload_bytes = payload_value.encode();
        let payload_base64 = BASE64.encode(&payload_bytes);
        if check_encoded
            && let Some(expected) = &self.encoded_hint
            && expected != &payload_base64
        {
            bail!(
                "encoded payload mismatch for '{}': expected {}, got {}",
                self.name,
                expected,
                payload_base64
            );
        }

        let signed_bytes = signed.encode();
        let signed_base64 = BASE64.encode(&signed_bytes);
        let payload_hash_hex = blake2b256_hex(&payload_bytes);
        let signed_hash_hex = signed_transaction_entrypoint_hash_hex(&signed_bytes)?;

        Ok(Fixture {
            name: self.name.clone(),
            payload_bytes,
            signed_bytes,
            summary: PayloadSummary {
                chain: self.payload.chain.clone(),
                authority: payload_value.authority().to_string(),
                creation_time_ms: self.payload.creation_time_ms,
                ttl_ms: actual_ttl_ms,
                nonce: self.payload.nonce,
                payload_base64,
                signed_base64,
                payload_hash_hex,
                signed_hash_hex,
            },
        })
    }
}

impl RawPayload {
    fn to_builder(&self) -> Result<TransactionBuilder> {
        let chain_id = ChainId::from_str(&self.chain)
            .with_context(|| format!("invalid canonical chain id '{}'", self.chain))?;
        let authority = parse_account_id(&self.authority)
            .with_context(|| format!("invalid authority id '{}'", self.authority))?;

        let mut builder = TransactionBuilder::new(chain_id, authority, self.fee_payment.clone());
        builder.set_creation_time(Duration::from_millis(self.creation_time_ms));
        builder.set_ttl(Duration::from_millis(self.ttl_ms));
        if let Some(nonce) = self.nonce {
            let nz = NonZeroU32::new(nonce).ok_or_else(|| eyre!("nonce must be > 0"))?;
            builder.set_nonce(nz);
        }

        let mut metadata = Metadata::default();
        for (key, value) in &self.metadata {
            metadata.insert(key.clone(), value.clone());
        }
        builder = builder.with_metadata(metadata);

        builder = match &self.executable {
            RawExecutable::Ivm(bytes) => {
                builder.with_executable(Executable::Ivm(IvmBytecode::from_compiled(bytes.clone())))
            }
            RawExecutable::Instructions(raws) => {
                let instructions = raws
                    .iter()
                    .map(build_instruction)
                    .collect::<Result<Vec<_>>>()?;
                builder.with_instructions(instructions)
            }
            RawExecutable::Batch(raws) => {
                let items = raws
                    .iter()
                    .map(|raw| match raw {
                        RawBatchItem::Instruction(raw) => {
                            build_instruction(raw).map(ExecutableBatchItem::Instruction)
                        }
                        RawBatchItem::ContractCall(invocation) => {
                            Ok(ExecutableBatchItem::ContractCall(invocation.clone()))
                        }
                    })
                    .collect::<Result<Vec<_>>>()?;
                if items.is_empty() {
                    bail!("mixed executable batch must not be empty");
                }
                builder.with_executable_batch(items)
            }
        };

        Ok(builder)
    }
}

impl Fixture {
    fn to_entry(&self) -> FixtureEntry {
        FixtureEntry {
            name: self.name.clone(),
            authority: self.summary.authority.clone(),
            chain: self.summary.chain.clone(),
            creation_time_ms: self.summary.creation_time_ms,
            encoded_file: format!("{}.norito", self.name),
            encoded_len: self.payload_bytes.len() as u64,
            signed_len: self.signed_bytes.len() as u64,
            payload_base64: self.summary.payload_base64.clone(),
            payload_hash: self.summary.payload_hash_hex.clone(),
            signed_base64: self.summary.signed_base64.clone(),
            signed_hash: self.summary.signed_hash_hex.clone(),
            nonce: self.summary.nonce,
            time_to_live_ms: self.summary.ttl_ms,
        }
    }
}

fn parse_payload_fixtures(value: &Value) -> Result<Vec<RawPayloadFixture>> {
    let arr = value
        .as_array()
        .ok_or_else(|| eyre!("fixture root must be an array"))?;
    let fixtures = arr
        .iter()
        .map(parse_payload_fixture)
        .collect::<Result<Vec<_>>>()?;
    let mut names = HashSet::with_capacity(fixtures.len());
    for fixture in &fixtures {
        if !names.insert(fixture.name.as_str()) {
            bail!("duplicate fixture name '{}'", fixture.name);
        }
    }
    Ok(fixtures)
}

fn parse_payload_fixture(value: &Value) -> Result<RawPayloadFixture> {
    let obj = value
        .as_object()
        .ok_or_else(|| eyre!("fixture entries must be objects"))?;
    let name = expect_string(obj, "name")?.to_owned();
    let payload_value = obj
        .get("payload")
        .ok_or_else(|| eyre!("fixture '{name}' missing payload"))?;
    let payload_json = payload_value.clone();
    let payload = parse_payload(payload_value)
        .with_context(|| format!("invalid payload for fixture '{name}'"))?;

    let encoded_hint = obj
        .get("payload_base64")
        .or_else(|| obj.get("encoded"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let chain_hint = obj.get("chain").and_then(Value::as_str).map(str::to_owned);
    let authority_hint = obj
        .get("authority")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let creation_time_ms_hint = obj.get("creation_time_ms").and_then(Value::as_u64);
    let ttl_ms_hint = expect_nonzero_u64(obj, "time_to_live_ms")
        .with_context(|| format!("invalid top-level lifetime for fixture '{name}'"))?;
    let nonce_hint = obj.get("nonce").and_then(Value::as_u64).map(|n| n as u32);

    Ok(RawPayloadFixture {
        name,
        payload,
        payload_json,
        encoded_hint,
        chain_hint,
        authority_hint,
        creation_time_ms_hint,
        ttl_ms_hint,
        nonce_hint,
    })
}

fn parse_payload(value: &Value) -> Result<RawPayload> {
    let obj = value
        .as_object()
        .ok_or_else(|| eyre!("payload entries must be objects"))?;
    let chain = expect_string(obj, "chain")?.to_owned();
    let authority = expect_string(obj, "authority")?.to_owned();
    let creation_time_ms = expect_u64(obj, "creation_time_ms")?;
    let executable_value = obj
        .get("executable")
        .ok_or_else(|| eyre!("missing executable"))?;
    let executable = parse_executable(executable_value)?;
    let ttl_ms = expect_nonzero_u64(obj, "time_to_live_ms")?;
    let nonce = parse_optional_u32(obj, "nonce")?;
    let fee_payment = obj
        .get("fee_payment")
        .ok_or_else(|| eyre!("missing fee_payment"))
        .and_then(|value| {
            json::from_value::<FeePaymentIntent>(value.clone())
                .map_err(|err| eyre!(err.to_string()))
        })?;
    fee_payment
        .validate()
        .map_err(|err| eyre!(err.to_string()))?;
    let metadata = match obj.get("metadata") {
        Some(value) => parse_metadata_object(value)?,
        None => Vec::new(),
    };

    Ok(RawPayload {
        chain,
        authority,
        creation_time_ms,
        executable,
        ttl_ms,
        nonce,
        fee_payment,
        metadata,
    })
}

fn parse_executable(value: &Value) -> Result<RawExecutable> {
    let obj = value
        .as_object()
        .ok_or_else(|| eyre!("executable must be an object"))?;
    if let Some(ivm) = obj.get("Ivm") {
        let bytes = ivm
            .as_str()
            .ok_or_else(|| eyre!("Ivm value must be base64 string"))?;
        let decoded = BASE64
            .decode(bytes)
            .with_context(|| format!("failed to decode Ivm base64 for payload {bytes:?}"))?;
        return Ok(RawExecutable::Ivm(decoded));
    }
    if let Some(instr) = obj.get("Instructions") {
        let arr = instr
            .as_array()
            .ok_or_else(|| eyre!("Instructions must be an array"))?;
        let mut entries = Vec::with_capacity(arr.len());
        for entry in arr {
            entries.push(parse_instruction(entry)?);
        }
        return Ok(RawExecutable::Instructions(entries));
    }
    if let Some(batch) = obj.get("Batch") {
        let arr = batch
            .as_array()
            .ok_or_else(|| eyre!("Batch must be an array"))?;
        if arr.is_empty() {
            bail!("Batch must contain at least one item");
        }
        let mut entries = Vec::with_capacity(arr.len());
        for entry in arr {
            let item = entry
                .as_object()
                .ok_or_else(|| eyre!("Batch items must be externally tagged objects"))?;
            if item.len() != 1 {
                bail!("Batch items must contain exactly one variant");
            }
            if let Some(instruction) = item.get("Instruction") {
                entries.push(RawBatchItem::Instruction(parse_instruction(instruction)?));
                continue;
            }
            if let Some(invocation) = item.get("ContractCall") {
                let invocation = json::from_value::<ContractInvocation>(invocation.clone())
                    .map_err(|err| eyre!(err.to_string()))
                    .context("invalid Batch ContractCall")?;
                entries.push(RawBatchItem::ContractCall(invocation));
                continue;
            }
            bail!("unknown Batch item variant");
        }
        return Ok(RawExecutable::Batch(entries));
    }
    bail!("unknown executable variant")
}

fn parse_instruction(value: &Value) -> Result<RawInstruction> {
    let obj = value
        .as_object()
        .ok_or_else(|| eyre!("instruction entries must be objects"))?;
    let wire_name = obj
        .get("wire_name")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| eyre!("instruction wire payload requires wire_name"))?;
    let payload_base64 = obj
        .get("payload_base64")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| eyre!("instruction wire payload requires payload_base64"))?;
    if obj.contains_key("kind") || obj.contains_key("arguments") {
        bail!("legacy instruction fields are not supported; use wire_name/payload_base64");
    }
    Ok(RawInstruction {
        wire_name,
        payload_base64,
    })
}

fn parse_metadata_object(value: &Value) -> Result<Vec<(Name, Json)>> {
    let obj = value
        .as_object()
        .ok_or_else(|| eyre!("metadata must be an object"))?;
    let mut entries = Vec::with_capacity(obj.len());
    for (key, value) in obj {
        let name: Name = key.parse().context("invalid metadata key")?;
        let json_value = Json::from_norito_value_ref(value)
            .map_err(|err| eyre!("invalid metadata json value for '{key}': {err}"))?;
        entries.push((name, json_value));
    }
    Ok(entries)
}

fn build_instruction(raw: &RawInstruction) -> Result<InstructionBox> {
    let payload_bytes = BASE64
        .decode(raw.payload_base64.as_bytes())
        .with_context(|| format!("invalid instruction payload_base64 for {}", raw.wire_name))?;
    if payload_bytes.is_empty() {
        bail!("instruction payload_base64 must not decode to empty bytes");
    }
    decode_instruction_from_pair(&raw.wire_name, &payload_bytes)
        .map_err(|err| eyre!(err.to_string()))
        .with_context(|| format!("failed to decode wire instruction '{}'", raw.wire_name))
}

fn normalize_authority_hint(authority: &str) -> String {
    let trimmed = authority.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    AccountId::parse_encoded(trimmed)
        .map(|parsed| parsed.into_account_id().to_string())
        .unwrap_or_else(|_| trimmed.to_string())
}

fn parse_account_id(value: &str) -> Result<AccountId> {
    AccountId::parse_encoded(value.trim())
        .map(|parsed| parsed.into_account_id())
        .with_context(|| format!("account id '{value}' must be a canonical I105-encoded literal"))
}

fn optional_u32_value(value: Option<u32>) -> Value {
    match value {
        Some(v) => Value::Number(Number::U64(v as u64)),
        None => Value::Null,
    }
}

fn build_payload_fixtures_json(
    raw_fixtures: &[RawPayloadFixture],
    fixtures: &[Fixture],
) -> Result<Value> {
    let fixtures_by_name: BTreeMap<&str, &Fixture> = fixtures
        .iter()
        .map(|fixture| (fixture.name.as_str(), fixture))
        .collect();

    let mut out = Vec::with_capacity(raw_fixtures.len());
    for raw in raw_fixtures {
        let fixture = fixtures_by_name
            .get(raw.name.as_str())
            .copied()
            .ok_or_else(|| eyre!("fixture '{}' missing generated payload", raw.name))?;

        let mut entry = Map::new();
        entry.insert("name".to_owned(), Value::String(fixture.name.clone()));
        entry.insert(
            "chain".to_owned(),
            Value::String(fixture.summary.chain.clone()),
        );
        entry.insert(
            "authority".to_owned(),
            Value::String(fixture.summary.authority.clone()),
        );
        entry.insert(
            "creation_time_ms".to_owned(),
            Value::Number(Number::U64(fixture.summary.creation_time_ms)),
        );
        entry.insert(
            "time_to_live_ms".to_owned(),
            Value::Number(Number::U64(fixture.summary.ttl_ms)),
        );
        entry.insert(
            "nonce".to_owned(),
            optional_u32_value(fixture.summary.nonce),
        );
        entry.insert(
            "payload_base64".to_owned(),
            Value::String(fixture.summary.payload_base64.clone()),
        );
        entry.insert(
            "signed_base64".to_owned(),
            Value::String(fixture.summary.signed_base64.clone()),
        );
        entry.insert(
            "payload_hash".to_owned(),
            Value::String(fixture.summary.payload_hash_hex.clone()),
        );
        entry.insert(
            "signed_hash".to_owned(),
            Value::String(fixture.summary.signed_hash_hex.clone()),
        );
        entry.insert(
            "encoded".to_owned(),
            Value::String(fixture.summary.payload_base64.clone()),
        );

        let mut payload = raw.payload_json.clone();
        if let Some(payload_obj) = payload.as_object_mut() {
            payload_obj.insert(
                "authority".to_owned(),
                Value::String(fixture.summary.authority.clone()),
            );
        }
        let wire_payloads = wire_payloads_from_encoded(&fixture.payload_bytes)?;
        if !wire_payloads.is_empty() {
            apply_wire_payloads_to_payload_json(&mut payload, &wire_payloads)?;
        }
        entry.insert("payload".to_owned(), payload);

        out.push(Value::Object(entry));
    }

    Ok(Value::Array(out))
}

fn wire_payloads_from_encoded(encoded: &[u8]) -> Result<Vec<WireInstructionPayload>> {
    let mut cursor = encoded;
    let payload = TransactionPayload::decode(&mut cursor).context("decode TransactionPayload")?;
    if !cursor.is_empty() {
        bail!("payload contains trailing bytes");
    }
    let registry = iroha_data_model::instruction_registry::default();
    let mut out = Vec::new();
    for instruction in payload.instructions().explicit_instructions() {
        let type_name = Instruction::id(&**instruction);
        let wire_name = registry.wire_id(type_name).unwrap_or(type_name);
        let payload = Instruction::dyn_encode(&**instruction);
        let framed =
            frame_instruction_payload(type_name, &payload).map_err(|err| eyre!(err.to_string()))?;
        out.push(WireInstructionPayload {
            wire_name: wire_name.to_owned(),
            payload_base64: BASE64.encode(framed),
        });
    }
    Ok(out)
}

fn apply_wire_payloads_to_payload_json(
    payload: &mut Value,
    wire_payloads: &[WireInstructionPayload],
) -> Result<()> {
    let payload_obj = payload
        .as_object_mut()
        .ok_or_else(|| eyre!("payload must be an object"))?;
    let executable_value = payload_obj
        .get_mut("executable")
        .ok_or_else(|| eyre!("payload missing executable"))?;
    let executable_obj = executable_value
        .as_object_mut()
        .ok_or_else(|| eyre!("payload executable must be an object"))?;
    if let Some(instructions_value) = executable_obj.get_mut("Instructions") {
        let instructions = instructions_value
            .as_array_mut()
            .ok_or_else(|| eyre!("payload Instructions must be an array"))?;
        if instructions.len() != wire_payloads.len() {
            bail!(
                "payload instructions length mismatch: expected {}, got {}",
                wire_payloads.len(),
                instructions.len()
            );
        }
        for (entry, wire) in instructions.iter_mut().zip(wire_payloads) {
            apply_wire_payload_to_instruction(entry, wire)?;
        }
        return Ok(());
    }

    let items = executable_obj
        .get_mut("Batch")
        .ok_or_else(|| eyre!("payload executable missing Instructions or Batch"))?
        .as_array_mut()
        .ok_or_else(|| eyre!("payload Batch must be an array"))?;
    let mut wires = wire_payloads.iter();
    let mut instruction_count = 0_usize;
    for item in items {
        let item = item
            .as_object_mut()
            .ok_or_else(|| eyre!("batch entries must be objects"))?;
        let Some(instruction) = item.get_mut("Instruction") else {
            continue;
        };
        let wire = wires
            .next()
            .ok_or_else(|| eyre!("batch contains more instructions than decoded payload"))?;
        apply_wire_payload_to_instruction(instruction, wire)?;
        instruction_count += 1;
    }
    if wires.next().is_some() || instruction_count != wire_payloads.len() {
        bail!(
            "payload batch instruction length mismatch: expected {}, got {}",
            wire_payloads.len(),
            instruction_count
        );
    }
    Ok(())
}

fn apply_wire_payload_to_instruction(
    entry: &mut Value,
    wire: &WireInstructionPayload,
) -> Result<()> {
    let obj = entry
        .as_object_mut()
        .ok_or_else(|| eyre!("instruction entries must be objects"))?;
    if obj.contains_key("kind") || obj.contains_key("arguments") {
        bail!("instruction entries must not include legacy kind/arguments fields");
    }
    obj.insert("wire_name".into(), Value::String(wire.wire_name.clone()));
    obj.insert(
        "payload_base64".into(),
        Value::String(wire.payload_base64.clone()),
    );
    Ok(())
}

fn expect_string<'a>(obj: &'a Map, key: &str) -> Result<&'a str> {
    obj.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("missing '{key}' string"))
}

fn expect_u64(obj: &Map, key: &str) -> Result<u64> {
    obj.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("missing '{key}' integer"))
}

fn expect_nonzero_u64(obj: &Map, key: &str) -> Result<u64> {
    let value = expect_u64(obj, key)?;
    if value == 0 {
        bail!("'{key}' must be greater than zero");
    }
    Ok(value)
}

fn parse_optional_u32(obj: &Map, key: &str) -> Result<Option<u32>> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => {
            let value = number
                .as_u64()
                .ok_or_else(|| eyre!("'{key}' must be an integer or null"))?;
            let value_u32 = u32::try_from(value)
                .with_context(|| format!("'{key}' must fit in u32 (got {value})"))?;
            Ok(Some(value_u32))
        }
        Some(other) => bail!("'{key}' must be an integer or null, got {other:?}"),
    }
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
        let mut names = HashSet::with_capacity(self.fixtures.len());
        let mut encoded_files = HashSet::with_capacity(self.fixtures.len());
        let mut payload_hashes = HashSet::with_capacity(self.fixtures.len());
        let mut payload_bytes_values = HashSet::with_capacity(self.fixtures.len());
        let mut signed_hashes = HashSet::with_capacity(self.fixtures.len());
        let mut signed_bytes_values = HashSet::with_capacity(self.fixtures.len());
        for fixture in &self.fixtures {
            if !names.insert(fixture.name.as_str()) {
                bail!("duplicate fixture name '{}'", fixture.name);
            }
            if !encoded_files.insert(fixture.encoded_file.as_str()) {
                bail!("duplicate fixture encoded_file '{}'", fixture.encoded_file);
            }
            if !payload_hashes.insert(fixture.payload_hash.as_str()) {
                bail!("duplicate fixture payload_hash '{}'", fixture.payload_hash);
            }
            let payload_bytes = BASE64
                .decode(fixture.payload_base64.as_bytes())
                .with_context(|| format!("fixture '{}' payload base64 invalid", fixture.name))?;
            if BASE64.encode(&payload_bytes) != fixture.payload_base64 {
                bail!("fixture '{}' payload base64 is non-canonical", fixture.name);
            }
            if !payload_bytes_values.insert(payload_bytes) {
                bail!("duplicate fixture payload bytes for '{}'", fixture.name);
            }
            if !signed_hashes.insert(fixture.signed_hash.as_str()) {
                bail!("duplicate fixture signed_hash '{}'", fixture.signed_hash);
            }
            let signed_bytes = BASE64
                .decode(fixture.signed_base64.as_bytes())
                .with_context(|| format!("fixture '{}' signed base64 invalid", fixture.name))?;
            if BASE64.encode(&signed_bytes) != fixture.signed_base64 {
                bail!("fixture '{}' signed base64 is non-canonical", fixture.name);
            }
            if !signed_bytes_values.insert(signed_bytes) {
                bail!("duplicate fixture signed bytes for '{}'", fixture.name);
            }
        }
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
    time_to_live_ms: u64,
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
        let signed = decode_canonical_signed_transaction(&signed_bytes)?;
        let signed_hash = hex_encode(signed.hash_as_entrypoint().as_ref());
        if signed_hash != self.signed_hash {
            bail!(
                "fixture '{}' signed hash mismatch (manifest={}, computed={})",
                self.name,
                self.signed_hash,
                signed_hash
            );
        }
        if signed.payload().encode() != payload_bytes {
            bail!(
                "fixture '{}' signed payload differs from the canonical payload bytes",
                self.name
            );
        }

        let actual_creation_time_ms =
            u64::try_from(signed.creation_time().as_millis()).map_err(|_| {
                eyre!(
                    "fixture '{}' creation_time_ms exceeds u64 in signed payload",
                    self.name
                )
            })?;
        let actual_ttl_ms = signed
            .time_to_live()
            .map(|ttl| u64::try_from(ttl.as_millis()))
            .transpose()
            .map_err(|_| {
                eyre!(
                    "fixture '{}' time_to_live_ms exceeds u64 in signed payload",
                    self.name
                )
            })?;
        let actual_nonce = signed.nonce().map(NonZeroU32::get);
        let actual_authority = signed.authority().to_string();
        let actual_chain = signed.chain().to_string();
        if actual_authority != self.authority
            || actual_chain != self.chain
            || actual_creation_time_ms != self.creation_time_ms
            || actual_ttl_ms != Some(self.time_to_live_ms)
            || actual_nonce != self.nonce
        {
            bail!(
                "fixture '{}' manifest summary differs from signed payload",
                self.name
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

fn signed_transaction_entrypoint_hash_hex(
    canonical_bare_signed_transaction: &[u8],
) -> Result<String> {
    let signed = decode_canonical_signed_transaction(canonical_bare_signed_transaction)?;
    Ok(hex_encode(signed.hash_as_entrypoint().as_ref()))
}

fn decode_canonical_signed_transaction(
    canonical_bare_signed_transaction: &[u8],
) -> Result<SignedTransaction> {
    let (signed, used) = SignedTransaction::decode_from_slice(canonical_bare_signed_transaction)
        .map_err(|err| eyre!("invalid canonical SignedTransaction bytes: {err}"))?;
    if used != canonical_bare_signed_transaction.len()
        || signed.encode() != canonical_bare_signed_transaction
    {
        bail!("SignedTransaction bytes are not exact canonical bare encoding");
    }
    Ok(signed)
}

fn render_compact_hash_vector(fixtures: &[FixtureEntry]) -> Result<String> {
    let source = fixtures
        .iter()
        .find(|fixture| fixture.name == COMPACT_HASH_VECTOR_SOURCE)
        .ok_or_else(|| {
            eyre!(
                "compact hash vector source fixture '{}' is missing from the selected manifest",
                COMPACT_HASH_VECTOR_SOURCE
            )
        })?;
    source
        .validate(None)
        .context("compact hash vector source fixture failed validation")?;

    let bare = BASE64
        .decode(&source.signed_base64)
        .context("compact hash vector signed bytes are not valid base64")?;
    if BASE64.encode(&bare) != source.signed_base64 {
        bail!("compact hash vector signed bytes are not canonical base64");
    }
    let signed = decode_canonical_signed_transaction(&bare)?;
    let payload = signed.payload().encode();
    let mut canonical = 0_u32.to_le_bytes().to_vec();
    norito::core::write_len_to_vec(&mut canonical, payload.len() as u64);
    let prefix_len = canonical.len();
    canonical.extend_from_slice(&payload);
    let prefix = &canonical[..prefix_len];
    let (discriminant, compact_length) = prefix
        .split_at_checked(core::mem::size_of::<u32>())
        .ok_or_else(|| eyre!("compact entrypoint prefix is missing its discriminant"))?;
    if discriminant != 0_u32.to_le_bytes() {
        bail!("compact hash vector source is not the External transaction entrypoint");
    }
    if compact_length.is_empty() {
        bail!("compact entrypoint prefix is missing its canonical length");
    }
    if compact_length.len() != 2 {
        bail!(
            "compact hash vector source must exercise a two-byte length, got {} bytes",
            compact_length.len()
        );
    }
    let canonical_hash = blake2b256_hex(&canonical);
    if canonical_hash != source.signed_hash {
        bail!("compact hash vector does not match the manifest signed hash");
    }
    let payload_hash = blake2b256_hex(&payload);
    if payload_hash != source.payload_hash {
        bail!("compact hash vector payload does not match the manifest payload hash");
    }

    let mut versioned = Vec::with_capacity(1 + bare.len());
    versioned.push(SIGNED_TRANSACTION_V1);
    versioned.extend_from_slice(&bare);

    Ok(format!(
        concat!(
            "schema.version=2\n",
            "source.fixture={source_fixture}\n",
            "versioned.bytes={versioned_bytes}\n",
            "versioned.sha256={versioned_sha256}\n",
            "bare.bytes={bare_bytes}\n",
            "compact.length.hex={compact_length}\n",
            "canonical.prefix.hex={canonical_prefix}\n",
            "canonical.hash={canonical_hash}\n",
            "payload.prehash={payload_prehash}\n",
            "versioned.base64={versioned_base64}\n",
        ),
        source_fixture = source.name,
        versioned_bytes = versioned.len(),
        versioned_sha256 = hex_encode(Sha256::digest(&versioned)),
        bare_bytes = bare.len(),
        compact_length = hex_encode(compact_length),
        canonical_prefix = hex_encode(prefix),
        canonical_hash = canonical_hash,
        payload_prehash = payload_hash,
        versioned_base64 = BASE64.encode(versioned),
    ))
}

fn verify_compact_hash_vector(path: &Path, fixtures: &[FixtureEntry]) -> Result<()> {
    let expected = render_compact_hash_vector(fixtures)?;
    let actual = fs::read_to_string(path)
        .with_context(|| format!("failed to read compact hash vector {}", path.display()))?;
    if actual != expected {
        bail!(
            "compact hash vector {} is stale; regenerate Norito RPC fixtures",
            path.display()
        );
    }
    Ok(())
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
    time_to_live_ms: u64,
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

    use norito::core::DecodeFromSlice;

    use super::*;

    fn sample_manifest() -> Manifest {
        Manifest {
            fixtures: vec![fixture("alpha"), fixture("beta"), fixture("gamma")],
        }
    }

    fn fixture(name: &str) -> FixtureEntry {
        FixtureEntry {
            name: name.to_string(),
            authority: "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53".into(),
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
            time_to_live_ms: 1,
        }
    }

    #[test]
    fn mixed_batch_fixture_parser_preserves_item_order() {
        let mut instruction = Map::new();
        instruction.insert(
            "wire_name".to_owned(),
            Value::String("iroha.log".to_owned()),
        );
        instruction.insert(
            "payload_base64".to_owned(),
            Value::String("AA==".to_owned()),
        );
        let mut instruction_item = Map::new();
        instruction_item.insert("Instruction".to_owned(), Value::Object(instruction));
        let mut invocation = Map::new();
        invocation.insert(
            "contract_address".to_owned(),
            Value::String(
                "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8".to_owned(),
            ),
        );
        invocation.insert(
            "expected_code_hash".to_owned(),
            Value::String(
                "hash:0E5751C026E543B2E8AB2EB06099DAA1D1E5DF47778F7787FAAB45CDF12FE3A9#6A22"
                    .to_owned(),
            ),
        );
        invocation.insert("entrypoint".to_owned(), Value::String("run".to_owned()));
        invocation.insert("arguments".to_owned(), Value::Null);
        let mut contract_item = Map::new();
        contract_item.insert("ContractCall".to_owned(), Value::Object(invocation));
        let mut executable = Map::new();
        executable.insert(
            "Batch".to_owned(),
            Value::Array(vec![
                Value::Object(instruction_item),
                Value::Object(contract_item),
            ]),
        );

        let parsed = parse_executable(&Value::Object(executable)).expect("parse mixed batch");
        let RawExecutable::Batch(items) = parsed else {
            panic!("expected mixed batch");
        };
        assert!(matches!(items[0], RawBatchItem::Instruction(_)));
        assert!(matches!(items[1], RawBatchItem::ContractCall(_)));
    }

    #[test]
    fn mixed_batch_fixture_parser_rejects_empty_batch() {
        let mut executable = Map::new();
        executable.insert("Batch".to_owned(), Value::Array(Vec::new()));
        let Err(err) = parse_executable(&Value::Object(executable)) else {
            panic!("empty mixed batch must be rejected");
        };
        assert!(err.to_string().contains("at least one item"));
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
    fn signed_hash_uses_compact_external_entrypoint_domain() {
        let keypair = signing_keypair().expect("fixture signing key");
        let signed = TransactionBuilder::new(
            ChainId::from("fixture-hash-domain"),
            AccountId::new(keypair.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .try_sign(keypair.private_key())
        .expect("sign fixture transaction");
        let signed_bytes = signed.encode();
        let payload_bytes = signed.payload().encode();
        let mut entrypoint = 0_u32.to_le_bytes().to_vec();
        norito::core::write_len_to_vec(&mut entrypoint, payload_bytes.len() as u64);
        entrypoint.extend_from_slice(&payload_bytes);
        assert_eq!(
            signed_transaction_entrypoint_hash_hex(&signed_bytes)
                .expect("hash canonical signed transaction"),
            blake2b256_hex(&entrypoint)
        );
        assert_ne!(
            signed_transaction_entrypoint_hash_hex(&signed_bytes)
                .expect("hash canonical signed transaction"),
            blake2b256_hex(&signed_bytes)
        );
    }

    #[test]
    fn compact_hash_vector_is_descriptor_owned_and_deterministic() {
        let root = workspace_root();
        let manifest =
            Manifest::load(&root.join(CANONICAL_MANIFEST)).expect("canonical manifest loads");
        let first = render_compact_hash_vector(&manifest.fixtures).expect("render compact vector");
        let second = render_compact_hash_vector(&manifest.fixtures).expect("render compact vector");
        assert_eq!(
            first, second,
            "compact vector rendering must be deterministic"
        );
        assert_eq!(
            fs::read_to_string(
                root.join("fixtures/norito_rpc")
                    .join(COMPACT_HASH_VECTOR_BASENAME)
            )
            .expect("read checked-in compact vector"),
            first,
            "checked-in compact vector must be generated from the canonical manifest"
        );

        let properties: BTreeMap<_, _> = first
            .lines()
            .map(|line| line.split_once('=').expect("generated property"))
            .collect();
        assert_eq!(properties["schema.version"], "2");
        assert_eq!(properties["source.fixture"], COMPACT_HASH_VECTOR_SOURCE);
        assert_eq!(
            properties["versioned.bytes"]
                .parse::<usize>()
                .expect("versioned byte count"),
            properties["bare.bytes"]
                .parse::<usize>()
                .expect("bare byte count")
                + 1
        );
        assert_eq!(
            properties["canonical.prefix.hex"],
            format!("00000000{}", properties["compact.length.hex"])
        );
        assert_eq!(properties["versioned.bytes"], "592");
        assert_eq!(
            properties["versioned.sha256"],
            "8c7bd16c4f5bbbbeb67aa0a0e3b2d82e4a2a0f08d2bdc7e48aff6ed8d806b255"
        );
        assert_eq!(properties["bare.bytes"], "591");
        assert_eq!(properties["compact.length.hex"], "bf03");
        assert_eq!(properties["canonical.prefix.hex"], "00000000bf03");
        assert_eq!(
            properties["canonical.hash"],
            "9e4fca2b657ecf1d9c206badf2b2b7511c1cfbc749a778ce792eb31a13ac9927"
        );
        assert_eq!(
            properties["payload.prehash"],
            "ea55e9ccd91a2a4910245a7747b856af27b2262f4278b8c0efd3166603612d71"
        );
    }

    #[test]
    fn compact_hash_vector_requires_the_canonical_source_fixture() {
        let error = render_compact_hash_vector(&sample_manifest().fixtures)
            .expect_err("missing canonical source fixture must fail closed");
        assert!(
            error.to_string().contains(COMPACT_HASH_VECTOR_SOURCE),
            "error must identify the missing source fixture: {error}"
        );
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
    fn manifest_validation_rejects_duplicate_fixture_names() {
        let manifest = Manifest {
            fixtures: vec![fixture("alpha"), fixture("alpha")],
        };
        let err = manifest
            .validate(None)
            .expect_err("duplicate fixture names must fail closed");
        assert!(
            err.to_string().contains("duplicate fixture name 'alpha'"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn manifest_validation_rejects_duplicate_encoded_files() {
        let first = fixture("alpha");
        let mut second = fixture("beta");
        second.encoded_file = first.encoded_file.clone();
        let manifest = Manifest {
            fixtures: vec![first, second],
        };
        let err = manifest
            .validate(None)
            .expect_err("duplicate encoded files must fail closed");
        assert!(
            err.to_string()
                .contains("duplicate fixture encoded_file 'alpha.norito'"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn manifest_validation_rejects_renamed_cloned_payloads() {
        let first = fixture("alpha");
        let mut second = fixture("beta");
        second.payload_hash = first.payload_hash.clone();
        second.payload_base64 = first.payload_base64.clone();
        second.signed_hash = first.signed_hash.clone();
        second.signed_base64 = first.signed_base64.clone();
        let manifest = Manifest {
            fixtures: vec![first, second],
        };
        let err = manifest
            .validate(None)
            .expect_err("renamed cloned fixture payloads must fail closed");
        assert!(
            err.to_string()
                .contains("duplicate fixture payload_hash 'payload-alpha'"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn manifest_validation_rejects_noncanonical_base64() {
        for malformed in ["YQ!!", "Y Q==", "YQ=", "YQ===", "YR=="] {
            let mut entry = fixture("alpha");
            entry.payload_base64 = malformed.to_string();
            let manifest = Manifest {
                fixtures: vec![entry],
            };
            let err = manifest
                .validate(None)
                .expect_err("invalid or non-canonical base64 must fail closed");
            let message = err.to_string();
            assert!(
                message.contains("base64 invalid") || message.contains("base64 is non-canonical"),
                "unexpected error for {malformed:?}: {err}"
            );
        }
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
    fn schema_targets_exclude_retired_sns_mutation_requests() {
        let aliases = schema_targets()
            .into_iter()
            .map(|target| target.alias)
            .collect::<HashSet<_>>();
        for retired in [
            "FreezeNameRequestV1",
            "GovernanceHookV1",
            "PaymentProofV1",
            "RegisterNameRequestV1",
            "RegisterNameResponseV1",
            "RenewNameRequestV1",
            "ReservedAssignmentRequestV1",
            "TransferNameRequestV1",
            "UpdateControllersRequestV1",
        ] {
            assert!(
                !aliases.contains(retired),
                "retired SNS mutation schema `{retired}` must not be advertised"
            );
        }
    }

    #[test]
    fn schema_manifest_round_trip_validates() {
        let manifest = SchemaHashManifest::new_current();
        manifest.validate().expect("generated manifest validates");
    }

    #[test]
    fn schema_manifest_generation_is_deterministic() {
        let first =
            json::to_json_pretty(&SchemaHashManifest::new_current()).expect("serialize manifest");
        let second =
            json::to_json_pretty(&SchemaHashManifest::new_current()).expect("serialize manifest");
        assert_eq!(first, second);
        assert!(
            !first.contains("generated_at"),
            "checked-in schema manifests must not contain wall-clock state"
        );
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
    fn workspace_root_points_to_repository() {
        let root = workspace_root();
        assert!(root.join("Cargo.toml").is_file());
        assert!(root.join("xtask/Cargo.toml").is_file());
        assert!(root.join(CANONICAL_MANIFEST).is_file());
    }

    #[test]
    fn json_report_writer_creates_parent_directory() {
        let temp_dir = tempdir().expect("temp dir");
        let output = temp_dir.path().join("nested/report.json");
        let value = Value::String("fixture-report".to_owned());

        write_json_output(&value, JsonOutput::File(output.clone())).expect("write JSON report");

        assert_eq!(
            fs::read_to_string(output).expect("read JSON report"),
            "\"fixture-report\"\n"
        );
    }

    #[test]
    fn generated_transaction_fixture_uses_checked_signing() {
        let keypair = signing_keypair().expect("checked signing keypair");
        let account_id = AccountId::new(keypair.public_key().clone());
        let raw = RawPayloadFixture {
            name: "checked-signing".to_string(),
            payload: RawPayload {
                chain: "00000001".to_string(),
                authority: account_id.to_string(),
                creation_time_ms: 1_735_000_000_000,
                executable: RawExecutable::Instructions(Vec::new()),
                ttl_ms: 60_000,
                nonce: Some(1),
                fee_payment: FeePaymentIntent::authority(Vec::new(), None),
                metadata: Vec::new(),
            },
            payload_json: Value::Null,
            encoded_hint: None,
            chain_hint: None,
            authority_hint: None,
            creation_time_ms_hint: None,
            ttl_ms_hint: 60_000,
            nonce_hint: None,
        };

        let fixture = raw
            .generate_fixture(&keypair, false)
            .expect("generate checked signed fixture");
        let (signed, used) = SignedTransaction::decode_from_slice(&fixture.signed_bytes)
            .expect("decode checked signed fixture");

        assert_eq!(used, fixture.signed_bytes.len());
        signed
            .verify_signature()
            .expect("checked fixture transaction signature verifies");
    }

    #[test]
    fn raw_payload_rejects_invalid_chain_id() {
        let payload = RawPayload {
            chain: String::new(),
            authority: String::new(),
            creation_time_ms: 0,
            executable: RawExecutable::Instructions(Vec::new()),
            ttl_ms: 1,
            nonce: None,
            fee_payment: FeePaymentIntent::authority(Vec::new(), None),
            metadata: Vec::new(),
        };

        let error = payload
            .to_builder()
            .err()
            .expect("an empty chain id must be rejected");
        assert!(
            error.to_string().contains("invalid canonical chain id ''"),
            "unexpected chain id error: {error}"
        );
    }

    #[test]
    fn payload_ttl_is_required_and_nonzero() {
        for value in [None, Some(Value::Null), Some(Value::from(0_u64))] {
            let mut object = Map::new();
            if let Some(value) = value {
                object.insert("time_to_live_ms".to_owned(), value);
            }
            let error = expect_nonzero_u64(&object, "time_to_live_ms")
                .expect_err("missing, null, and zero lifetimes must be rejected");
            assert!(
                error.to_string().contains("time_to_live_ms"),
                "unexpected lifetime error: {error}"
            );
        }

        let mut object = Map::new();
        object.insert("time_to_live_ms".to_owned(), Value::from(100_000_u64));
        assert_eq!(
            expect_nonzero_u64(&object, "time_to_live_ms").expect("positive lifetime"),
            100_000
        );
    }

    #[test]
    fn fixture_manifest_requires_explicit_ttl() {
        let manifest = Manifest {
            fixtures: vec![fixture("required-ttl")],
        };
        let encoded = json::to_json_pretty(&manifest).expect("serialize fixture manifest");
        let base: Value = json::from_str(&encoded).expect("parse fixture manifest value");

        for replacement in [None, Some(Value::Null)] {
            let mut candidate = base.clone();
            let entry = candidate
                .as_object_mut()
                .and_then(|object| object.get_mut("fixtures"))
                .and_then(Value::as_array_mut)
                .and_then(|fixtures| fixtures.first_mut())
                .and_then(Value::as_object_mut)
                .expect("fixture manifest entry");
            if let Some(value) = replacement {
                entry.insert("time_to_live_ms".to_owned(), value);
            } else {
                entry.remove("time_to_live_ms");
            }

            json::from_value::<Manifest>(candidate)
                .expect_err("missing and null fixture lifetimes must be rejected");
        }
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

    #[test]
    fn manifest_validation_rejects_ttl_summary_drift() {
        let root = workspace_root();
        let canonical_path = root.join(CANONICAL_MANIFEST);
        let canonical = Manifest::load(&canonical_path).expect("canonical manifest loads");
        let mut entry = canonical
            .fixtures
            .first()
            .expect("at least one fixture in canonical manifest")
            .clone();
        entry.time_to_live_ms = entry
            .time_to_live_ms
            .checked_add(1)
            .expect("fixture lifetime can be incremented");

        let error = Manifest {
            fixtures: vec![entry],
        }
        .validate(None)
        .expect_err("manifest lifetime drift must be rejected");
        assert!(
            error
                .to_string()
                .contains("manifest summary differs from signed payload"),
            "unexpected lifetime drift error: {error}"
        );
    }
}
