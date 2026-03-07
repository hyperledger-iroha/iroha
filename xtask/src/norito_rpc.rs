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
use iroha_crypto::{Algorithm, KeyPair, PublicKey};
use iroha_data_model::{
    ChainId,
    account::{AccountId, address},
    domain::DomainId,
    isi::{Instruction, InstructionBox, decode_instruction_from_pair, frame_instruction_payload},
    metadata::Metadata,
    name::Name,
    sns::{
        FreezeNameRequestV1, GovernanceHookV1, NameControllerV1, NameRecordV1, NameSelectorV1,
        NameStatus, PaymentProofV1, RegisterNameRequestV1, RegisterNameResponseV1,
        RenewNameRequestV1, ReservedAssignmentRequestV1, SuffixPolicyV1, TransferNameRequestV1,
        UpdateControllersRequestV1,
    },
    transaction::{
        Executable, IvmBytecode, SignedTransaction, TransactionBuilder, signed::TransactionPayload,
    },
};
use iroha_primitives::json::Json;
use norito::{
    NoritoSerialize,
    codec::{Decode, Encode},
    json::{self, JsonDeserialize, JsonSerialize, Map, Number, Value},
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

    // Keep the Android/Swift/Python `transaction_payloads.json` source hints in sync.
    let updated_payloads = build_payload_fixtures_json(&raw_fixtures, &fixtures)?;
    let payloads_json = json::to_json_pretty(&updated_payloads)?;
    fs::write(&resolved.fixtures_json, format!("{payloads_json}\n"))
        .with_context(|| format!("failed to write {}", resolved.fixtures_json.display()))?;

    Ok(())
}

const SIGNING_SEED_HEX: &str = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";

fn signing_keypair() -> Result<KeyPair> {
    let seed = hex::decode(SIGNING_SEED_HEX).context("invalid signing seed hex")?;
    Ok(KeyPair::from_seed(seed, Algorithm::Ed25519))
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
    ttl_ms_hint: Option<u64>,
    nonce_hint: Option<u32>,
}

#[derive(Clone)]
struct RawPayload {
    chain: String,
    authority: String,
    creation_time_ms: u64,
    executable: RawExecutable,
    ttl_ms: Option<u64>,
    nonce: Option<u32>,
    metadata: Vec<(Name, Json)>,
}

#[derive(Clone)]
enum RawExecutable {
    Ivm(Vec<u8>),
    Instructions(Vec<RawInstruction>),
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
    ttl_ms: Option<u64>,
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
        if let Some(ttl_hint) = self.ttl_ms_hint
            && Some(ttl_hint) != self.payload.ttl_ms
        {
            bail!(
                "fixture '{}' time_to_live_ms mismatch: expected {}, got {:?}",
                self.name,
                ttl_hint,
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

        let builder = self.payload.to_builder()?;
        let signed = builder.sign(keypair.private_key());
        let payload_value = signed.payload().clone();
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
        let signed_hash_hex = blake2b256_hex(&signed_bytes);

        Ok(Fixture {
            name: self.name.clone(),
            payload_bytes,
            signed_bytes,
            summary: PayloadSummary {
                chain: self.payload.chain.clone(),
                authority: payload_value.authority.to_string(),
                creation_time_ms: self.payload.creation_time_ms,
                ttl_ms: self.payload.ttl_ms,
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
        let chain_id = ChainId::from_str(&self.chain).expect("ChainId parsing must be infallible");
        let authority = parse_account_id(&self.authority)
            .with_context(|| format!("invalid authority id '{}'", self.authority))?;

        let mut builder = TransactionBuilder::new(chain_id, authority);
        builder.set_creation_time(Duration::from_millis(self.creation_time_ms));
        if let Some(ttl) = self.ttl_ms {
            builder.set_ttl(Duration::from_millis(ttl));
        }
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
    arr.iter().map(parse_payload_fixture).collect()
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
    let ttl_ms_hint = obj.get("time_to_live_ms").and_then(Value::as_u64);
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
    let ttl_ms = parse_optional_u64(obj, "time_to_live_ms")?;
    let nonce = parse_optional_u32(obj, "nonce")?;
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
    if let Ok(canonical) = AccountId::canonicalize(trimmed) {
        return canonical;
    }
    if let Some((address_part, _)) = trimmed.rsplit_once('@') {
        if let Ok(canonical) = AccountId::canonicalize(address_part) {
            return canonical;
        }
        return address_part.to_string();
    }
    trimmed.to_string()
}

fn parse_account_id(value: &str) -> Result<AccountId> {
    let (signatory_hint, domain_part) = value
        .split_once('@')
        .ok_or_else(|| eyre!("account id '{value}' must contain '@'"))?;
    let domain: DomainId = domain_part
        .parse()
        .with_context(|| format!("invalid domain id '{domain_part}'"))?;

    if let Ok((address_payload, _)) = address::AccountAddress::parse_any(signatory_hint, None) {
        return address_payload
            .to_account_id(&domain)
            .map_err(|err| eyre!(err.code_str()));
    }

    if let Ok(signatory) = signatory_hint.parse::<PublicKey>() {
        return Ok(AccountId::of(domain, signatory));
    }

    let seed = derive_seed(signatory_hint, domain_part);
    let keypair = KeyPair::from_seed(seed, Algorithm::Ed25519);
    Ok(AccountId::of(domain, keypair.public_key().clone()))
}

fn derive_seed(left: &str, right: &str) -> Vec<u8> {
    let mut seed = [0u8; 32];
    for (index, byte) in left
        .as_bytes()
        .iter()
        .chain(right.as_bytes().iter())
        .enumerate()
    {
        seed[index % seed.len()] ^= *byte;
    }
    seed.to_vec()
}

fn optional_u64_value(value: Option<u64>) -> Value {
    match value {
        Some(v) => Value::Number(Number::U64(v)),
        None => Value::Null,
    }
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
            optional_u64_value(fixture.summary.ttl_ms),
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
    let Executable::Instructions(instructions) = &payload.instructions else {
        return Ok(Vec::new());
    };

    let registry = iroha_data_model::instruction_registry::default();
    let mut out = Vec::with_capacity(instructions.len());
    for instruction in instructions.iter() {
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
    let instructions_value = executable_obj
        .get_mut("Instructions")
        .ok_or_else(|| eyre!("payload executable missing Instructions"))?;
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
    }
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

fn parse_optional_u64(obj: &Map, key: &str) -> Result<Option<u64>> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_u64()
            .ok_or_else(|| eyre!("'{key}' must be an integer or null"))
            .map(Some),
        Some(other) => bail!("'{key}' must be an integer or null, got {other:?}"),
    }
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
            authority: "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn".into(),
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
