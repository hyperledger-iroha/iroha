//! Encode a freshly verified KAGEMUSHA evidence projection as an experimental receipt.
//!
//! This tool runs the existing closed-evidence verifier on private copies of pinned
//! inputs, then packages the verified files under SHA-256 names for Kagami's release
//! preparer. It does not generate proving keys, observations, or production qualification.

use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fs::{self, File, OpenOptions},
    io::{self, Read as _, Write as _},
    path::{Path, PathBuf},
    process::Command,
};

use iroha_data_model::kagemusha::{
    KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
    KAGEMUSHA_RELEASE_EVIDENCE_FILE_MAX_BYTES_V1, KagemushaArtifactBindingV1,
    KagemushaArtifactRoleV1, KagemushaEvidenceFileV1, KagemushaInternalValidationReceiptV1,
    kagemusha_artifact_set_digest_v1, kagemusha_vk_set_digest_v1,
};
use norito::json::{Map, Value};
use sha2::{Digest as _, Sha256};

const MAX_PROJECTION_BYTES: usize = 16 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MAX_EVIDENCE_FILES: usize = 65_536;
const STREAM_BUFFER_BYTES: usize = 64 * 1024;
const PROJECTION_SCHEMA: &str = "iroha.kagemusha_v1.testnet_experiment_authority_review_projection";
const MANIFEST_SCHEMA: &str = "iroha.kagemusha_v1.testnet_experiment_evidence_manifest";
const ISOLATED_VERIFIER_ENTRY: &str = "import runpy,sys; sys.path.append(sys.argv[1]); sys.argv=sys.argv[2:]; runpy.run_path(sys.argv[0],run_name='__main__')";

#[derive(Debug)]
struct Arguments {
    python: PathBuf,
    python_sha256: [u8; 32],
    verifier: PathBuf,
    verifier_sha256: [u8; 32],
    artifact_contract: PathBuf,
    artifact_contract_sha256: [u8; 32],
    manifest: PathBuf,
    manifest_sha256: [u8; 32],
    evidence_root: PathBuf,
    observer_policy: PathBuf,
    observer_policy_sha256: [u8; 32],
    output_dir: PathBuf,
}

#[derive(Clone, Debug)]
struct SourceFile {
    path: PathBuf,
    sha256: [u8; 32],
    byte_len: u64,
}

#[derive(Debug)]
struct ManifestFile {
    source: SourceFile,
    kind: String,
}

#[derive(Debug)]
struct HandoffSources {
    artifacts: BTreeMap<[u8; 32], SourceFile>,
    evidence: BTreeMap<[u8; 32], SourceFile>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Arguments::parse(env::args().skip(1))?;
    args.check_inputs()?;
    let projection = args.run_verifier()?;
    args.check_inputs()?;
    let (receipt, inventory) = decode_projection(&projection, args.manifest_sha256)?;
    let receipt_bytes = norito::encode_canonical(&receipt)?;
    if receipt_bytes.len() > KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1 {
        return Err(invalid("typed receipt exceeds its wire limit").into());
    }
    KagemushaInternalValidationReceiptV1::decode_canonical_experimental_exact(&receipt_bytes)?;
    let mut inventory_bytes = norito::json::to_json(&inventory)?.into_bytes();
    inventory_bytes.push(b'\n');
    write_verified_handoff(
        &args,
        &receipt,
        &inventory,
        &receipt_bytes,
        &inventory_bytes,
        &projection,
    )?;
    Ok(())
}

impl Arguments {
    fn parse(raw: impl IntoIterator<Item = String>) -> io::Result<Self> {
        let mut fields = BTreeMap::new();
        let mut raw = raw.into_iter();
        while let Some(flag) = raw.next() {
            if !matches!(
                flag.as_str(),
                "--python"
                    | "--python-sha256"
                    | "--verifier"
                    | "--verifier-sha256"
                    | "--artifact-contract"
                    | "--artifact-contract-sha256"
                    | "--manifest"
                    | "--manifest-sha256"
                    | "--evidence-root"
                    | "--observer-policy"
                    | "--observer-policy-sha256"
                    | "--output-dir"
            ) || fields
                .insert(
                    flag.clone(),
                    raw.next().ok_or_else(|| invalid("missing flag value"))?,
                )
                .is_some()
            {
                return Err(invalid("unknown or repeated flag"));
            }
        }
        if fields.len() != 12 {
            return Err(invalid("all twelve pinned input/output flags are required"));
        }
        let take_path = |key| -> io::Result<PathBuf> {
            fields
                .get(key)
                .map(PathBuf::from)
                .ok_or_else(|| invalid("missing path flag"))
        };
        let take_digest = |key| -> io::Result<[u8; 32]> {
            parse_digest(
                fields
                    .get(key)
                    .ok_or_else(|| invalid("missing digest flag"))?,
            )
        };
        Ok(Self {
            python: take_path("--python")?,
            python_sha256: take_digest("--python-sha256")?,
            verifier: take_path("--verifier")?,
            verifier_sha256: take_digest("--verifier-sha256")?,
            artifact_contract: take_path("--artifact-contract")?,
            artifact_contract_sha256: take_digest("--artifact-contract-sha256")?,
            manifest: take_path("--manifest")?,
            manifest_sha256: take_digest("--manifest-sha256")?,
            evidence_root: take_path("--evidence-root")?,
            observer_policy: take_path("--observer-policy")?,
            observer_policy_sha256: take_digest("--observer-policy-sha256")?,
            output_dir: take_path("--output-dir")?,
        })
    }

    fn check_inputs(&self) -> io::Result<()> {
        for (path, digest) in [
            (&self.python, self.python_sha256),
            (&self.verifier, self.verifier_sha256),
            (&self.artifact_contract, self.artifact_contract_sha256),
            (&self.manifest, self.manifest_sha256),
            (&self.observer_policy, self.observer_policy_sha256),
        ] {
            check_pinned_file(path, digest)?;
        }
        check_canonical_dir(&self.evidence_root)?;
        if self
            .verifier
            .parent()
            .and_then(|parent| {
                parent
                    .join("release_artifact_contract.py")
                    .canonicalize()
                    .ok()
            })
            .as_deref()
            != Some(self.artifact_contract.as_path())
        {
            return Err(invalid(
                "artifact contract must be the verifier's pinned sibling",
            ));
        }
        let parent = self
            .output_dir
            .parent()
            .ok_or_else(|| invalid("output has no parent"))?;
        check_canonical_dir(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            if fs::metadata(parent)?.permissions().mode() & 0o077 != 0 {
                return Err(invalid("output parent must be owner-only"));
            }
        }
        if self.output_dir.starts_with(&self.evidence_root) {
            return Err(invalid(
                "output directory must be outside the closed evidence root",
            ));
        }
        if self.output_dir.exists() {
            return Err(invalid("output directory already exists"));
        }
        Ok(())
    }

    fn run_verifier(&self) -> io::Result<Vec<u8>> {
        let output_parent = self
            .output_dir
            .parent()
            .ok_or_else(|| invalid("output has no parent"))?;
        let staged = tempfile::Builder::new()
            .prefix(".kagemusha-pinned-verifier-")
            .tempdir_in(output_parent)?;
        let staged_python = staged.path().join(
            self.python
                .file_name()
                .ok_or_else(|| invalid("Python executable has no filename"))?,
        );
        let staged_verifier = staged
            .path()
            .join("verify_kagemusha_v1_release_evidence.py");
        let staged_contract = staged.path().join("release_artifact_contract.py");
        stage_pinned_file(&self.python, &staged_python, self.python_sha256, true)?;
        stage_pinned_file(
            &self.verifier,
            &staged_verifier,
            self.verifier_sha256,
            false,
        )?;
        stage_pinned_file(
            &self.artifact_contract,
            &staged_contract,
            self.artifact_contract_sha256,
            false,
        )?;
        let mut command = isolated_verifier_command(&staged_python, &staged_verifier)?;
        let output = command
            .arg("--manifest")
            .arg(&self.manifest)
            .arg("--manifest-sha256")
            .arg(hex::encode(self.manifest_sha256))
            .arg("--evidence-root")
            .arg(&self.evidence_root)
            .arg("--observer-policy")
            .arg(&self.observer_policy)
            .arg("--observer-policy-sha256")
            .arg(hex::encode(self.observer_policy_sha256))
            .arg("--testnet-experiment")
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "pinned release verifier rejected evidence: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        if output.stdout.len() > MAX_PROJECTION_BYTES {
            return Err(invalid("verified projection exceeds its input limit"));
        }
        Ok(output.stdout)
    }
}

fn isolated_verifier_command(python: &Path, verifier: &Path) -> io::Result<Command> {
    let sibling_dir = verifier
        .parent()
        .ok_or_else(|| invalid("verifier has no parent"))?;
    let mut command = Command::new(python);
    // -I ignores Python environment/current-directory injection; -S disables
    // sitecustomize and third-party site imports. Append only the pinned sibling
    // directory after stdlib paths so it cannot shadow hashlib/json imports.
    command
        .arg("-I")
        .arg("-S")
        .arg("-c")
        .arg(ISOLATED_VERIFIER_ENTRY)
        .arg(sibling_dir)
        .arg(verifier)
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .env("PYTHONDONTWRITEBYTECODE", "1");
    Ok(command)
}

fn decode_projection(
    bytes: &[u8],
    expected_manifest_digest: [u8; 32],
) -> Result<
    (
        KagemushaInternalValidationReceiptV1,
        Vec<KagemushaArtifactBindingV1>,
    ),
    Box<dyn Error>,
> {
    if bytes.is_empty() || bytes.len() > MAX_PROJECTION_BYTES {
        return Err(invalid("verified projection is empty or oversized").into());
    }
    let projection: Value = norito::json::from_slice(bytes)?;
    let object = projection
        .as_object()
        .ok_or_else(|| invalid("projection is not an object"))?;
    if object.get("schema").and_then(Value::as_str) != Some(PROJECTION_SCHEMA)
        || object.get("schema_version").and_then(Value::as_u64) != Some(1)
        || object.get("manifest_sha256").and_then(Value::as_str)
            != Some(hex::encode(expected_manifest_digest).as_str())
    {
        return Err(invalid("projection schema or manifest pin differs").into());
    }
    let receipt: KagemushaInternalValidationReceiptV1 =
        norito::json::from_value(restore_rust_json_shape(
            object
                .get("receipt_projection")
                .ok_or_else(|| invalid("projection has no receipt"))?
                .clone(),
        )?)?;
    let inventory: Vec<KagemushaArtifactBindingV1> =
        norito::json::from_value(restore_rust_json_shape(
            object
                .get("artifact_inventory")
                .ok_or_else(|| invalid("projection has no inventory"))?
                .clone(),
        )?)?;
    receipt.validate_experimental()?;
    if kagemusha_artifact_set_digest_v1(&inventory)? != receipt.artifact_set_digest {
        return Err(invalid("inventory differs from the structural receipt").into());
    }
    let vk_digest = kagemusha_vk_set_digest_v1(
        &inventory,
        receipt.eq_protocol_digest,
        receipt.ep_protocol_digest,
        &receipt.helper_protocols,
    )?;
    if receipt
        .profile_qualifications
        .iter()
        .any(|qualification| qualification.profile.vk_digest != vk_digest)
    {
        return Err(invalid("profile verifier set differs from the artifact inventory").into());
    }
    Ok((receipt, inventory))
}

// The independent verifier's canonical JSON uses lowercase hex and bare enum names.
// Norito's typed JSON derives use fixed-byte arrays and tagged unit enums. Only the
// known representation changes below are allowed; semantic validation runs afterward.
fn restore_rust_json_shape(value: Value) -> io::Result<Value> {
    match value {
        Value::Array(values) => values
            .into_iter()
            .map(restore_rust_json_shape)
            .collect::<io::Result<Vec<_>>>()
            .map(Value::Array),
        Value::Object(values) => {
            let mut restored = Map::new();
            for (field, value) in values {
                let value = if is_digest_field(&field) {
                    hex_array(value, 32)?
                } else if matches!(
                    field.as_str(),
                    "governance_credential_public_key" | "issuer_signature"
                ) {
                    let width = if field == "issuer_signature" { 64 } else { 65 };
                    Value::Array(vec![checked_hex_string(value, width)?])
                } else if let Some(tag) = unit_enum_tag(&field) {
                    let Value::String(name) = value else {
                        return Err(invalid("projection unit enum is not a string"));
                    };
                    let mut tagged = Map::new();
                    tagged.insert(tag.into(), Value::String(name));
                    tagged.insert("value".into(), Value::Null);
                    Value::Object(tagged)
                } else {
                    restore_rust_json_shape(value)?
                };
                restored.insert(field, value);
            }
            Ok(Value::Object(restored))
        }
        value => Ok(value),
    }
}

fn hex_array(value: Value, width: usize) -> io::Result<Value> {
    let Value::String(raw) = checked_hex_string(value, width)? else {
        unreachable!("checked hex always returns a string")
    };
    let bytes = hex::decode(raw).map_err(|_| invalid("projection fixed bytes are malformed"))?;
    Ok(Value::Array(bytes.into_iter().map(Value::from).collect()))
}

fn checked_hex_string(value: Value, width: usize) -> io::Result<Value> {
    let Value::String(raw) = value else {
        return Err(invalid("projection fixed bytes are not lowercase hex"));
    };
    if raw.len() != width * 2
        || !raw
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(
            "projection fixed bytes have the wrong width or alphabet",
        ));
    }
    Ok(Value::String(raw))
}

fn is_digest_field(field: &str) -> bool {
    matches!(
        field,
        "sha256"
            | "source_tree_digest"
            | "cargo_lock_digest"
            | "profile_digest"
            | "native_profile_digest"
            | "eq_protocol_digest"
            | "ep_protocol_digest"
            | "artifact_set_digest"
            | "hardware_policy_digest"
            | "provider_policy_root"
            | "provider_authority_commitment"
            | "verification_records_digest"
            | "candidate_context_digest"
            | "hardware_profile_id"
            | "suite_id"
            | "vk_digest"
            | "qualification_digest"
            | "builder_id"
            | "provider_id"
            | "product_class_digest"
            | "firmware_policy_digest"
            | "app_attestation_authority_policy_digest"
            | "enrollment_attestation_verifier_digest"
            | "attestation_trust_roots_digest"
            | "allowed_suite_commitment"
            | "qualification_report_digest"
    )
}

fn unit_enum_tag(field: &str) -> Option<&'static str> {
    match field {
        "role" => Some("role"),
        "relation" => Some("relation"),
        "helper" => Some("helper"),
        "platform_class" => Some("class"),
        "case" => Some("case"),
        _ => None,
    }
}

fn check_pinned_file(path: &Path, expected: [u8; 32]) -> io::Result<()> {
    let mut input = open_pinned_input(path)?;
    let before = input.metadata()?;
    let actual = hash_reader(&mut input)?;
    if actual != expected || !pinned_file_unchanged(path, &before, &input.metadata()?)? {
        return Err(invalid("pinned input SHA-256 changed"));
    }
    Ok(())
}

fn stage_pinned_file(
    source: &Path,
    target: &Path,
    expected: [u8; 32],
    executable: bool,
) -> io::Result<()> {
    let mut input = open_pinned_input(source)?;
    let before = input.metadata()?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(target)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; STREAM_BUFFER_BYTES];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])?;
        hasher.update(&buffer[..count]);
    }
    if <[u8; 32]>::from(hasher.finalize()) != expected
        || !pinned_file_unchanged(source, &before, &input.metadata()?)?
    {
        return Err(invalid("pinned source changed before private staging"));
    }
    file.sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(
            target,
            fs::Permissions::from_mode(if executable { 0o500 } else { 0o400 }),
        )?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    check_pinned_file(target, expected)
}

fn open_pinned_input(path: &Path) -> io::Result<File> {
    #[cfg(unix)]
    {
        open_nofollow_regular(path)
    }
    #[cfg(not(unix))]
    {
        if !path.is_absolute()
            || path.canonicalize()? != path
            || !path.is_file()
            || fs::symlink_metadata(path)?.file_type().is_symlink()
        {
            return Err(invalid(
                "pinned input must be a canonical absolute regular file",
            ));
        }
        File::open(path)
    }
}

fn hash_reader(reader: &mut impl io::Read) -> io::Result<[u8; 32]> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; STREAM_BUFFER_BYTES];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().into())
}

fn pinned_file_unchanged(
    path: &Path,
    before: &fs::Metadata,
    after: &fs::Metadata,
) -> io::Result<bool> {
    #[cfg(unix)]
    {
        Ok(same_file_metadata(before, after)
            && same_file_metadata(before, &open_nofollow_regular(path)?.metadata()?))
    }
    #[cfg(not(unix))]
    {
        Ok(before.len() == after.len()
            && before.modified()? == after.modified()?
            && fs::metadata(path)?.len() == before.len())
    }
}

fn read_pinned_manifest(path: &Path, expected: [u8; 32]) -> io::Result<Vec<u8>> {
    let mut input = open_pinned_input(path)?;
    let before = input.metadata()?;
    if before.len() > MAX_MANIFEST_BYTES {
        return Err(invalid("verified evidence manifest exceeds its byte limit"));
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    (&mut input)
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES
        || bytes.len() as u64 != before.len()
        || <[u8; 32]>::from(Sha256::digest(&bytes)) != expected
        || !pinned_file_unchanged(path, &before, &input.metadata()?)?
    {
        return Err(invalid(
            "evidence manifest changed after independent verification",
        ));
    }
    Ok(bytes)
}

fn check_canonical_dir(path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path.canonicalize()? != path
        || !path.is_dir()
        || fs::symlink_metadata(path)?.file_type().is_symlink()
    {
        return Err(invalid(
            "directory must be canonical, absolute, and non-symlink",
        ));
    }
    Ok(())
}

fn parse_digest(raw: &str) -> io::Result<[u8; 32]> {
    if raw.len() != 64
        || !raw
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(
            "digest must be 64 lowercase hexadecimal characters",
        ));
    }
    let mut digest = [0_u8; 32];
    hex::decode_to_slice(raw, &mut digest).map_err(|_| invalid("digest is not hexadecimal"))?;
    Ok(digest)
}

fn write_verified_handoff(
    args: &Arguments,
    receipt_binding: &KagemushaInternalValidationReceiptV1,
    inventory_bindings: &[KagemushaArtifactBindingV1],
    receipt: &[u8],
    inventory: &[u8],
    projection: &[u8],
) -> io::Result<()> {
    let sources = collect_handoff_sources(args, receipt_binding, inventory_bindings, projection)?;
    let root = &args.output_dir;
    let parent = root
        .parent()
        .ok_or_else(|| invalid("handoff output has no parent"))?;
    let staging = private_staging_dir(parent)?;
    let staged_root = staging.path();
    let artifacts = staged_root.join("artifacts");
    let evidence = staged_root.join("evidence");
    create_private_dir(&artifacts)?;
    create_private_dir(&evidence)?;
    for source in sources.artifacts.values() {
        copy_content_addressed(source, &artifacts)?;
    }
    for source in sources.evidence.values() {
        copy_content_addressed(source, &evidence)?;
    }
    File::open(&artifacts)?.sync_all()?;
    File::open(&evidence)?.sync_all()?;
    write_outputs(staged_root, receipt, inventory, projection)?;
    File::open(staged_root)?.sync_all()?;
    publish_staged_handoff(staged_root, root)?;
    // The staging name no longer exists after rename; do not let TempDir's
    // cleanup race a later private directory created with that old name.
    let _published_staging_path = staging.keep();
    Ok(())
}

fn private_staging_dir(parent: &Path) -> io::Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(".kagemusha-handoff-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        builder.permissions(fs::Permissions::from_mode(0o700));
    }
    builder.tempdir_in(parent)
}

#[cfg(any(
    target_vendor = "apple",
    target_os = "linux",
    target_os = "android",
    target_os = "redox"
))]
fn publish_staged_handoff(staging: &Path, root: &Path) -> io::Result<()> {
    let parent_path = root
        .parent()
        .ok_or_else(|| invalid("handoff output has no parent"))?;
    if staging.parent() != Some(parent_path) {
        return Err(invalid("handoff staging and output must be siblings"));
    }
    let parent = open_nofollow_directory(parent_path)?;
    rustix::fs::renameat_with(
        &parent,
        staging
            .file_name()
            .ok_or_else(|| invalid("handoff staging has no name"))?,
        &parent,
        root.file_name()
            .ok_or_else(|| invalid("handoff output has no name"))?,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(io::Error::from)?;
    parent.sync_all()
}

#[cfg(not(any(
    target_vendor = "apple",
    target_os = "linux",
    target_os = "android",
    target_os = "redox"
)))]
fn publish_staged_handoff(_staging: &Path, _root: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic no-replace handoff publication is unavailable on this platform",
    ))
}

fn write_outputs(
    root: &Path,
    receipt: &[u8],
    inventory: &[u8],
    projection: &[u8],
) -> io::Result<()> {
    for (name, bytes) in [
        ("artifact_inventory.json", inventory),
        ("authority-review-projection.json", projection),
        ("receipt.norito", receipt),
    ] {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options.open(root.join(name))?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    Ok(())
}

fn create_private_dir(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(path)
}

fn collect_handoff_sources(
    args: &Arguments,
    receipt: &KagemushaInternalValidationReceiptV1,
    inventory: &[KagemushaArtifactBindingV1],
    projection_bytes: &[u8],
) -> io::Result<HandoffSources> {
    let manifest_bytes = read_pinned_manifest(&args.manifest, args.manifest_sha256)?;
    let manifest: Value = norito::json::from_slice(&manifest_bytes)
        .map_err(|_| invalid("verified evidence manifest is malformed"))?;
    let object = manifest
        .as_object()
        .ok_or_else(|| invalid("verified evidence manifest is not an object"))?;
    if object.get("schema").and_then(Value::as_str) != Some(MANIFEST_SCHEMA)
        || object.get("schema_version").and_then(Value::as_u64) != Some(1)
    {
        return Err(invalid("verified evidence manifest has the wrong schema"));
    }
    let rows = object
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("verified evidence manifest has no file list"))?;
    if rows.is_empty() || rows.len() > MAX_EVIDENCE_FILES {
        return Err(invalid("verified evidence manifest file count is invalid"));
    }
    let mut files = BTreeMap::new();
    let mut by_digest = BTreeMap::new();
    for row in rows {
        let row = row
            .as_object()
            .ok_or_else(|| invalid("verified evidence file row is not an object"))?;
        let relative = row
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("verified evidence file has no path"))?;
        let relative = canonical_evidence_relative_path(relative)?;
        let digest = parse_digest(
            row.get("sha256")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("verified evidence file has no SHA-256"))?,
        )?;
        let byte_len = row
            .get("byte_len")
            .and_then(Value::as_u64)
            .filter(|len| *len > 0)
            .ok_or_else(|| invalid("verified evidence file has no positive length"))?;
        let kind = row
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("verified evidence file has no kind"))?;
        let source = SourceFile {
            path: args.evidence_root.join(&relative),
            sha256: digest,
            byte_len,
        };
        let file = ManifestFile {
            source: source.clone(),
            kind: kind.to_owned(),
        };
        if files.insert(relative, file).is_some() {
            return Err(invalid("verified evidence file path is repeated"));
        }
        if let Some(previous) = by_digest.insert(digest, source)
            && previous.byte_len != byte_len
        {
            return Err(invalid("verified evidence digest has conflicting lengths"));
        }
    }

    let artifact_rows = object
        .get("artifacts")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("verified evidence manifest has no artifact list"))?;
    let projection: Value = norito::json::from_slice(projection_bytes)
        .map_err(|_| invalid("verified projection cannot be decoded for handoff"))?;
    let projection_rows = projection
        .get("artifact_inventory")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("verified projection has no artifact inventory"))?;
    if artifact_rows.len() != 50
        || inventory.len() != artifact_rows.len()
        || projection_rows.len() != artifact_rows.len()
    {
        return Err(invalid("handoff requires exactly 50 ordered artifacts"));
    }
    let mut artifacts = BTreeMap::new();
    for (index, ((row, projected), binding)) in artifact_rows
        .iter()
        .zip(projection_rows)
        .zip(inventory)
        .enumerate()
    {
        let row = row
            .as_object()
            .ok_or_else(|| invalid("verified artifact row is not an object"))?;
        let role = row
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("verified artifact has no role"))?;
        let typed_role = norito::json::to_value(&binding.role)
            .map_err(|_| invalid("typed artifact role cannot be projected"))?;
        if binding.role != KagemushaArtifactRoleV1::ALL[index]
            || typed_role.get("role").and_then(Value::as_str) != Some(role)
            || projected.get("role").and_then(Value::as_str) != Some(role)
            || projected.get("sha256").and_then(Value::as_str)
                != Some(hex::encode(binding.sha256).as_str())
            || projected.get("byte_len").and_then(Value::as_u64) != Some(binding.byte_len)
        {
            return Err(invalid(
                "verified artifact role differs from typed inventory",
            ));
        }
        let path = row
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("verified artifact has no path"))?;
        let path = canonical_evidence_relative_path(path)?;
        let file = files
            .get(&path)
            .ok_or_else(|| invalid("verified artifact path is not in evidence files"))?;
        if file.kind != "artifact"
            || file.source.sha256 != binding.sha256
            || file.source.byte_len != binding.byte_len
        {
            return Err(invalid("verified artifact differs from typed inventory"));
        }
        if artifacts
            .insert(binding.sha256, file.source.clone())
            .is_some()
        {
            return Err(invalid("verified artifact digest is repeated"));
        }
    }

    let manifest_binding = receipt.evidence_closure.evidence_manifest;
    let policy_binding = receipt.evidence_closure.observer_policy;
    if manifest_binding.sha256 != args.manifest_sha256
        || policy_binding.sha256 != args.observer_policy_sha256
    {
        return Err(invalid("typed evidence closure differs from pinned inputs"));
    }
    let mut evidence = BTreeMap::new();
    insert_handoff_evidence(
        &mut evidence,
        manifest_binding,
        SourceFile {
            path: args.manifest.clone(),
            sha256: args.manifest_sha256,
            byte_len: manifest_bytes.len() as u64,
        },
    )?;
    insert_handoff_evidence(
        &mut evidence,
        policy_binding,
        SourceFile {
            path: args.observer_policy.clone(),
            sha256: args.observer_policy_sha256,
            byte_len: fs::metadata(&args.observer_policy)?.len(),
        },
    )?;
    for binding in experimental_receipt_evidence_files(receipt)? {
        let source = by_digest
            .get(&binding.sha256)
            .ok_or_else(|| invalid("typed evidence is absent from verified manifest"))?;
        insert_handoff_evidence(&mut evidence, binding, source.clone())?;
    }
    Ok(HandoffSources {
        artifacts,
        evidence,
    })
}

fn canonical_evidence_relative_path(raw: &str) -> io::Result<String> {
    if raw.is_empty()
        || raw.len() > 512
        || raw.starts_with('/')
        || raw.ends_with('/')
        || raw.contains("//")
        || raw
            .bytes()
            .any(|byte| byte < 0x20 || byte == 0x7f || byte == b'\\' || byte == b':')
        || raw
            .split('/')
            .any(|component| component == "." || component == "..")
    {
        return Err(invalid("evidence path is not canonical and relative"));
    }
    Ok(raw.to_owned())
}

fn insert_handoff_evidence(
    selected: &mut BTreeMap<[u8; 32], SourceFile>,
    binding: KagemushaEvidenceFileV1,
    source: SourceFile,
) -> io::Result<()> {
    if binding.sha256 == [0; 32]
        || binding.byte_len == 0
        || binding.byte_len > KAGEMUSHA_RELEASE_EVIDENCE_FILE_MAX_BYTES_V1
        || binding.sha256 != source.sha256
        || binding.byte_len != source.byte_len
    {
        return Err(invalid("typed evidence has no exact verified source"));
    }
    if let Some(previous) = selected.insert(binding.sha256, source)
        && previous.byte_len != binding.byte_len
    {
        return Err(invalid("typed evidence digest has conflicting lengths"));
    }
    Ok(())
}

fn append_optional_evidence(
    files: &mut Vec<KagemushaEvidenceFileV1>,
    binding: KagemushaEvidenceFileV1,
) -> io::Result<()> {
    if binding.sha256 == [0; 32] && binding.byte_len == 0 {
        return Ok(());
    }
    if binding.sha256 == [0; 32]
        || binding.byte_len == 0
        || binding.byte_len > KAGEMUSHA_RELEASE_EVIDENCE_FILE_MAX_BYTES_V1
    {
        return Err(invalid("optional typed evidence is partial or oversized"));
    }
    files.push(binding);
    Ok(())
}

fn experimental_receipt_evidence_files(
    receipt: &KagemushaInternalValidationReceiptV1,
) -> io::Result<Vec<KagemushaEvidenceFileV1>> {
    let mut files = vec![receipt.circuit_shape_report];
    for optional in [
        receipt.security_review_report,
        receipt.kat_report,
        receipt.fuzz_report,
        receipt.resource_report,
    ] {
        append_optional_evidence(&mut files, optional)?;
    }
    for profile in &receipt.profile_qualifications {
        files.push(profile.profile.qualification_report);
        files.extend(profile.relations.iter().map(|row| row.report));
        files.extend(profile.helper_circuits.iter().map(|row| row.report));
        for optional in profile
            .recursive_depths
            .iter()
            .map(|row| row.report)
            .chain([
                profile.aggregate_balance.report,
                profile.thermal.report,
                profile.envelope.report,
            ])
            .chain(profile.acceptance_cases.iter().map(|row| row.report))
        {
            append_optional_evidence(&mut files, optional)?;
        }
    }
    for optional in receipt.reproducible_builds.iter().map(|row| row.report) {
        append_optional_evidence(&mut files, optional)?;
    }
    Ok(files)
}

#[cfg(unix)]
fn copy_content_addressed(source: &SourceFile, target_root: &Path) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};

    let mut input = open_nofollow_regular(&source.path)?;
    let before = input.metadata()?;
    if before.len() != source.byte_len || source.byte_len == 0 {
        return Err(invalid("verified source length changed before handoff"));
    }
    let target_path = target_root.join(hex::encode(source.sha256));
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true).mode(0o600);
    let mut output = options.open(&target_path)?;
    let mut hasher = Sha256::new();
    let mut remaining = source.byte_len;
    let mut buffer = [0_u8; STREAM_BUFFER_BYTES];
    while remaining > 0 {
        let limit = usize::try_from(remaining.min(buffer.len() as u64))
            .map_err(|_| invalid("verified source length is unsupported"))?;
        let count = input.read(&mut buffer[..limit])?;
        if count == 0 {
            return Err(invalid("verified source ended before its typed length"));
        }
        output.write_all(&buffer[..count])?;
        hasher.update(&buffer[..count]);
        remaining -= count as u64;
    }
    if input.read(&mut buffer[..1])? != 0
        || <[u8; 32]>::from(hasher.finalize()) != source.sha256
        || !same_file_metadata(&before, &input.metadata()?)
        || !same_file_metadata(&before, &open_nofollow_regular(&source.path)?.metadata()?)
    {
        return Err(invalid(
            "verified source changed during content-addressed copy",
        ));
    }
    output.sync_all()?;
    let target_before = output.metadata()?;
    if target_before.len() != source.byte_len || target_before.mode() & 0o077 != 0 {
        return Err(invalid("content-addressed target has the wrong identity"));
    }
    let mut reopened = open_nofollow_regular(&target_path)?;
    if !same_file_metadata(&target_before, &reopened.metadata()?) {
        return Err(invalid("content-addressed target path changed"));
    }
    let mut target_hasher = Sha256::new();
    let mut observed = 0_u64;
    loop {
        let count = reopened.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        observed = observed
            .checked_add(count as u64)
            .ok_or_else(|| invalid("content-addressed target length overflow"))?;
        if observed > source.byte_len {
            return Err(invalid("content-addressed target is oversized"));
        }
        target_hasher.update(&buffer[..count]);
    }
    if observed != source.byte_len
        || <[u8; 32]>::from(target_hasher.finalize()) != source.sha256
        || !same_file_metadata(&target_before, &reopened.metadata()?)
    {
        return Err(invalid(
            "content-addressed target differs from its typed binding",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn copy_content_addressed(_source: &SourceFile, _target_root: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-pinned handoff is unavailable on this platform",
    ))
}

#[cfg(unix)]
fn open_nofollow_regular(path: &Path) -> io::Result<File> {
    use rustix::fs::OFlags;
    use std::os::unix::fs::MetadataExt as _;

    if !path.is_absolute() || path.canonicalize()? != path {
        return Err(invalid(
            "handoff source or target must be canonical and absolute",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| invalid("handoff file has no parent"))?;
    let directory = open_nofollow_directory(parent)?;
    let name = path
        .file_name()
        .ok_or_else(|| invalid("handoff file has no name"))?;
    let file = openat_nofollow(
        &directory,
        name,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC | OFlags::NOFOLLOW,
    )?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.nlink() != 1 || metadata.mode() & 0o022 != 0 {
        return Err(invalid(
            "handoff file is not an owner-controlled regular file",
        ));
    }
    Ok(file)
}

#[cfg(unix)]
fn open_nofollow_directory(path: &Path) -> io::Result<File> {
    use rustix::fs::OFlags;
    use std::path::Component;

    if !path.is_absolute() || path.canonicalize()? != path {
        return Err(invalid("handoff directory must be canonical and absolute"));
    }
    let mut directory = File::open("/")?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                directory = openat_nofollow(
                    &directory,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                )?;
                if !directory.metadata()?.is_dir() {
                    return Err(invalid("handoff path component is not a directory"));
                }
            }
            _ => return Err(invalid("handoff directory path is not canonical")),
        }
    }
    Ok(directory)
}

#[cfg(unix)]
fn openat_nofollow(
    parent: &File,
    name: &std::ffi::OsStr,
    flags: rustix::fs::OFlags,
) -> io::Result<File> {
    use rustix::fs::{Mode, openat};

    Ok(File::from(
        openat(parent, name, flags, Mode::empty()).map_err(io::Error::from)?,
    ))
}

#[cfg(unix)]
fn same_file_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mode() == right.mode()
        && left.nlink() == right.nlink()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_flags_and_digest_shape_are_exact() {
        assert!(Arguments::parse(["--manifest".into(), "x".into()]).is_err());
        assert!(
            Arguments::parse([
                "--manifest".into(),
                "x".into(),
                "--manifest".into(),
                "y".into()
            ])
            .is_err()
        );
        assert!(parse_digest(&"ab".repeat(32)).is_ok());
        assert!(parse_digest(&"AB".repeat(32)).is_err());
    }

    #[test]
    fn projections_cannot_change_the_pinned_manifest_or_omit_reports() {
        let pin = [0x42; 32];
        let unpinned = format!(
            r#"{{"schema":"{PROJECTION_SCHEMA}","schema_version":1,"manifest_sha256":"{}"}}"#,
            hex::encode([0x43; 32])
        );
        assert!(decode_projection(unpinned.as_bytes(), pin).is_err());
        let missing = format!(
            r#"{{"schema":"{PROJECTION_SCHEMA}","schema_version":1,"manifest_sha256":"{}"}}"#,
            hex::encode(pin)
        );
        assert!(decode_projection(missing.as_bytes(), pin).is_err());
    }

    #[test]
    fn verified_inventory_json_restores_exact_typed_role_and_digest() {
        let raw: Value = norito::json::from_str(&format!(
            r#"{{"role":"params_eq","sha256":"{}","byte_len":4194372}}"#,
            "ab".repeat(32)
        ))
        .unwrap();
        let typed: KagemushaArtifactBindingV1 =
            norito::json::from_value(restore_rust_json_shape(raw).unwrap()).unwrap();
        assert_eq!(typed.sha256, [0xab; 32]);
        let mut malformed: Value =
            norito::json::from_str(r#"{"role":"params_eq","sha256":"ab","byte_len":4194372}"#)
                .unwrap();
        assert!(restore_rust_json_shape(malformed.clone()).is_err());
        malformed
            .as_object_mut()
            .unwrap()
            .insert("role".into(), Value::Bool(true));
        assert!(restore_rust_json_shape(malformed).is_err());
    }

    #[test]
    fn signed_structural_projection_decodes_as_exact_experimental_receipt() {
        let root = tempfile::tempdir().unwrap();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let fixture = repo.join("pytests/scripts/kagemusha_testnet_experiment_evidence_test.py");
        let script = r#"
import importlib.util, pathlib, sys
spec = importlib.util.spec_from_file_location('receipt_fixture', sys.argv[1])
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
fixture = module._testnet_fixture(pathlib.Path(sys.argv[2]) / 'evidence')
projection = module._verify(fixture, testnet_experiment=True)
sys.stdout.buffer.write(module.VERIFIER.canonical_json_bytes(projection))
"#;
        let output = Command::new("python3")
            .arg("-c")
            .arg(script)
            .arg(fixture)
            .arg(root.path().canonicalize().unwrap())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let projection: Value = norito::json::from_slice(&output.stdout).unwrap();
        let manifest_pin = parse_digest(projection["manifest_sha256"].as_str().unwrap()).unwrap();
        let (receipt, inventory) = decode_projection(&output.stdout, manifest_pin).unwrap();
        assert_eq!(receipt.fuzz_cases, 0);
        assert_eq!(inventory.len(), 50);

        let fixture_parent = root.path().canonicalize().unwrap().join("evidence");
        let policy = fixture_parent.join("trusted-observer-policy.json");
        let mut args = Arguments {
            python: PathBuf::new(),
            python_sha256: [0; 32],
            verifier: PathBuf::new(),
            verifier_sha256: [0; 32],
            artifact_contract: PathBuf::new(),
            artifact_contract_sha256: [0; 32],
            manifest: fixture_parent.join("kagemusha-evidence.json"),
            manifest_sha256: manifest_pin,
            evidence_root: fixture_parent.join("evidence"),
            observer_policy: policy.clone(),
            observer_policy_sha256: Sha256::digest(fs::read(&policy).unwrap()).into(),
            output_dir: root.path().canonicalize().unwrap().join("handoff"),
        };
        let sources = collect_handoff_sources(&args, &receipt, &inventory, &output.stdout).unwrap();
        assert_eq!(sources.artifacts.len(), 50);
        assert!(sources.evidence.len() >= 4);
        let artifact_to_substitute = sources.artifacts[&inventory[0].sha256].path.clone();
        let receipt_bytes = norito::encode_canonical(&receipt).unwrap();
        let mut inventory_bytes = norito::json::to_json(&inventory).unwrap().into_bytes();
        inventory_bytes.push(b'\n');
        write_verified_handoff(
            &args,
            &receipt,
            &inventory,
            &receipt_bytes,
            &inventory_bytes,
            &output.stdout,
        )
        .unwrap();
        assert_eq!(
            fs::read(args.output_dir.join("receipt.norito")).unwrap(),
            receipt_bytes
        );
        assert_eq!(
            fs::read(args.output_dir.join("artifact_inventory.json")).unwrap(),
            inventory_bytes
        );
        assert_eq!(
            fs::read(args.output_dir.join("authority-review-projection.json")).unwrap(),
            output.stdout
        );
        for (digest, source) in sources.artifacts {
            assert_eq!(
                fs::read(args.output_dir.join("artifacts").join(hex::encode(digest))).unwrap(),
                fs::read(source.path).unwrap()
            );
        }
        for (digest, source) in sources.evidence {
            assert_eq!(
                fs::read(args.output_dir.join("evidence").join(hex::encode(digest))).unwrap(),
                fs::read(source.path).unwrap()
            );
        }
        fs::write(&artifact_to_substitute, b"substituted artifact").unwrap();
        args.output_dir = root.path().canonicalize().unwrap().join("rejected-handoff");
        assert!(
            write_verified_handoff(
                &args,
                &receipt,
                &inventory,
                &receipt_bytes,
                &inventory_bytes,
                &output.stdout,
            )
            .is_err()
        );
        assert!(!args.output_dir.join("receipt.norito").exists());
        fs::write(&args.manifest, b"substituted manifest").unwrap();
        assert!(collect_handoff_sources(&args, &receipt, &inventory, &output.stdout).is_err());
    }

    #[test]
    fn altered_report_or_verifier_bytes_break_the_pins() {
        let root = tempfile::tempdir().unwrap();
        let report = root.path().canonicalize().unwrap().join("report.json");
        fs::write(&report, b"real report bytes").unwrap();
        let expected: [u8; 32] = Sha256::digest(b"real report bytes").into();
        assert!(check_pinned_file(&report, expected).is_ok());
        fs::write(&report, b"changed report bytes").unwrap();
        assert!(check_pinned_file(&report, expected).is_err());
    }

    #[test]
    fn private_staged_verifier_keeps_exact_pinned_bytes() {
        let root = tempfile::tempdir().unwrap();
        let root_path = root.path().canonicalize().unwrap();
        let source = root_path.join("verifier.py");
        let staged = root_path.join("private.py");
        fs::write(&source, b"print('pinned')\n").unwrap();
        let pin: [u8; 32] = Sha256::digest(b"print('pinned')\n").into();
        stage_pinned_file(&source, &staged, pin, false).unwrap();
        fs::write(&source, b"print('changed')\n").unwrap();
        assert_eq!(fs::read(&staged).unwrap(), b"print('pinned')\n");
        assert!(stage_pinned_file(&source, &root_path.join("second.py"), pin, false).is_err());
    }

    #[test]
    fn generated_files_are_create_only() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("receipt-output");
        create_private_dir(&output).unwrap();
        write_outputs(&output, b"receipt", b"[]\n", b"{\"verified\":true}\n").unwrap();
        assert_eq!(fs::read(output.join("receipt.norito")).unwrap(), b"receipt");
        assert_eq!(
            fs::read(output.join("artifact_inventory.json")).unwrap(),
            b"[]\n"
        );
        assert_eq!(
            fs::read(output.join("authority-review-projection.json")).unwrap(),
            b"{\"verified\":true}\n"
        );
        assert!(write_outputs(&output, b"replacement", b"[]\n", b"{}").is_err());
        assert_eq!(fs::read(output.join("receipt.norito")).unwrap(), b"receipt");
    }

    #[cfg(any(
        target_vendor = "apple",
        target_os = "linux",
        target_os = "android",
        target_os = "redox"
    ))]
    #[test]
    fn handoff_publication_is_atomic_create_only_and_retry_safe() {
        let parent = tempfile::tempdir().unwrap();
        let parent = parent.path().canonicalize().unwrap();
        let output = parent.join("handoff");

        let interrupted = private_staging_dir(&parent).unwrap();
        let interrupted_path = interrupted.path().to_owned();
        fs::write(interrupted.path().join("receipt.norito"), b"incomplete").unwrap();
        assert!(!output.exists());
        drop(interrupted);
        assert!(!interrupted_path.exists());
        assert!(!output.exists());

        let staging = private_staging_dir(&parent).unwrap();
        let staging_path = staging.path().to_owned();
        fs::write(staging.path().join("receipt.norito"), b"complete").unwrap();
        assert!(!output.exists());
        publish_staged_handoff(staging.path(), &output).unwrap();
        let _published_staging_path = staging.keep();
        assert!(!staging_path.exists());
        assert_eq!(
            fs::read(output.join("receipt.norito")).unwrap(),
            b"complete"
        );

        let replacement = private_staging_dir(&parent).unwrap();
        let replacement_path = replacement.path().to_owned();
        fs::write(replacement.path().join("receipt.norito"), b"replacement").unwrap();
        assert!(publish_staged_handoff(replacement.path(), &output).is_err());
        assert_eq!(
            fs::read(output.join("receipt.norito")).unwrap(),
            b"complete"
        );
        drop(replacement);
        assert!(!replacement_path.exists());
    }

    #[test]
    fn handoff_rejects_noncanonical_relative_paths() {
        for path in [
            "../escape",
            "a/../b",
            "a//b",
            "a/./b",
            "/absolute",
            "back\\slash",
            "a:b",
            "a\0b",
        ] {
            assert!(canonical_evidence_relative_path(path).is_err(), "{path:?}");
        }
        assert_eq!(
            canonical_evidence_relative_path("reports/valid.json").unwrap(),
            "reports/valid.json"
        );
    }

    #[cfg(unix)]
    #[test]
    fn content_addressed_copy_rejects_substitution_and_symlinks() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let source_root = root.join("source");
        let target_root = root.join("target");
        create_private_dir(&source_root).unwrap();
        create_private_dir(&target_root).unwrap();
        let source_path = source_root.join("evidence.bin");
        fs::write(&source_path, b"pinned bytes").unwrap();
        let expected = SourceFile {
            path: source_path.clone(),
            sha256: Sha256::digest(b"pinned bytes").into(),
            byte_len: 12,
        };
        copy_content_addressed(&expected, &target_root).unwrap();
        assert_eq!(
            fs::read(target_root.join(hex::encode(expected.sha256))).unwrap(),
            b"pinned bytes"
        );
        let changed_root = root.join("changed-target");
        create_private_dir(&changed_root).unwrap();
        fs::write(&source_path, b"other bytes!").unwrap();
        assert!(copy_content_addressed(&expected, &changed_root).is_err());

        let link_root = root.join("link-target");
        create_private_dir(&link_root).unwrap();
        let link = source_root.join("link.bin");
        symlink(&source_path, &link).unwrap();
        assert!(
            copy_content_addressed(
                &SourceFile {
                    path: link,
                    ..expected.clone()
                },
                &link_root
            )
            .is_err()
        );
        let parent_link = root.join("linked-source");
        symlink(&source_root, &parent_link).unwrap();
        assert!(
            copy_content_addressed(
                &SourceFile {
                    path: parent_link.join("evidence.bin"),
                    ..expected
                },
                &link_root
            )
            .is_err()
        );
    }

    #[test]
    fn isolated_python_ignores_poisoned_import_path() {
        let root = tempfile::tempdir().unwrap();
        let trusted = root.path().join("trusted");
        let poisoned = root.path().join("poisoned");
        fs::create_dir(&trusted).unwrap();
        fs::create_dir(&poisoned).unwrap();
        let verifier = trusted.join("verifier.py");
        fs::write(
            &verifier,
            b"import hashlib\nfrom release_artifact_contract import marker\nprint(marker)\n",
        )
        .unwrap();
        fs::write(
            trusted.join("release_artifact_contract.py"),
            b"marker='trusted'\n",
        )
        .unwrap();
        fs::write(
            poisoned.join("release_artifact_contract.py"),
            b"marker='poisoned'\n",
        )
        .unwrap();
        fs::write(
            trusted.join("hashlib.py"),
            b"raise RuntimeError('shadowed stdlib')\n",
        )
        .unwrap();
        let output = isolated_verifier_command(Path::new("python3"), &verifier)
            .unwrap()
            .env("PYTHONPATH", &poisoned)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"trusted\n");
    }
}
