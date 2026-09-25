//! Encode a freshly verified KAGEMUSHA evidence projection as an experimental receipt.
//!
//! This tool runs the existing closed-evidence verifier on private copies of pinned
//! inputs. It does not generate proving keys, observations, or production qualification.

use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fs::{self, OpenOptions},
    io::{self, Write as _},
    path::{Path, PathBuf},
    process::Command,
};

use iroha_data_model::kagemusha::{
    KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1, KagemushaArtifactBindingV1,
    KagemushaInternalValidationReceiptV1, kagemusha_artifact_set_digest_v1,
    kagemusha_vk_set_digest_v1,
};
use norito::json::{Map, Value};
use sha2::{Digest as _, Sha256};

const MAX_PROJECTION_BYTES: usize = 16 * 1024 * 1024;
const PROJECTION_SCHEMA: &str = "iroha.kagemusha_v1.testnet_experiment_authority_review_projection";
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
    write_outputs(
        &args.output_dir,
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
    if !path.is_absolute() || path.canonicalize()? != path || !path.is_file() {
        return Err(invalid(
            "pinned input must be a canonical absolute regular file",
        ));
    }
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(invalid("pinned input cannot be a symlink"));
    }
    let actual: [u8; 32] = Sha256::digest(fs::read(path)?).into();
    if actual != expected {
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
    let bytes = fs::read(source)?;
    if <[u8; 32]>::from(Sha256::digest(&bytes)) != expected {
        return Err(invalid("pinned source changed before private staging"));
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(target)?;
    file.write_all(&bytes)?;
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

fn write_outputs(
    root: &Path,
    receipt: &[u8],
    inventory: &[u8],
    projection: &[u8],
) -> io::Result<()> {
    fs::create_dir(root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    }
    for (name, bytes) in [
        ("receipt.norito", receipt),
        ("artifact_inventory.json", inventory),
        ("authority-review-projection.json", projection),
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
