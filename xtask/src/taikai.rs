use crate::{JsonTarget, write_json_output};
use blake3::Hasher;
use eyre::{Context as _, Result, ensure, eyre};
use iroha_data_model::taikai::{
    CekRotationReceiptV1, ReplicationProofTokenV1, TaikaiEventId, TaikaiStreamId,
};
use norito::{derive::JsonSerialize, json};
use sorafs_car::taikai::validate_distinct_artifact_paths;
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
const TAIKAI_BUNDLE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.taikai.bundle.v1";
const MAX_TAIKAI_POLICY_DOCUMENT_BYTES: u64 = 1024 * 1024;
#[derive(Debug)]
pub struct RptVerifyOptions {
    pub envelope_path: PathBuf,
    pub gar_path: PathBuf,
    pub cek_receipt_path: PathBuf,
    pub bundle_path: PathBuf,
    pub output: Option<JsonTarget>,
}
#[derive(Debug, JsonSerialize)]
struct DigestCheck {
    expected: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    verified_from: Option<String>,
}
#[derive(Debug, JsonSerialize)]
struct RptVerificationReport {
    envelope_path: String,
    schema_version: u16,
    event_id: String,
    stream_id: String,
    rendition_id: String,
    gar: DigestCheck,
    cek_receipt: DigestCheck,
    distribution_bundle: DigestCheck,
    policy_labels: Vec<String>,
    valid_from_unix: u64,
    valid_until_unix: u64,
    #[norito(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
}
pub fn run_rpt_verify(options: RptVerifyOptions) -> Result<()> {
    validate_report_output_path(&options)?;
    let envelope_path = options.envelope_path.clone();
    let rpt = load_rpt(&envelope_path)?;
    rpt.validate().wrap_err_with(|| {
        format!(
            "RPT `{}` violates replication proof token invariants",
            envelope_path.display()
        )
    })?;
    let gar_digest = compute_file_digest(&options.gar_path)
        .wrap_err_with(|| format!("failed to hash GAR `{}`", options.gar_path.display()))?;
    ensure!(
        gar_digest == rpt.gar_digest,
        "GAR digest mismatch for `{}` (expected {}, got {})",
        options.gar_path.display(),
        to_hex(&rpt.gar_digest),
        to_hex(&gar_digest)
    );
    let cek_digest =
        validate_cek_receipt_binding(&options.cek_receipt_path, &rpt.event_id, &rpt.stream_id)?;
    ensure!(
        cek_digest == rpt.cek_receipt_digest,
        "CEK receipt digest mismatch for `{}` (expected {}, got {})",
        options.cek_receipt_path.display(),
        to_hex(&rpt.cek_receipt_digest),
        to_hex(&cek_digest)
    );
    let bundle_digest = compute_bundle_digest(&options.bundle_path)
        .wrap_err_with(|| format!("failed to hash bundle `{}`", options.bundle_path.display()))?;
    ensure!(
        bundle_digest == rpt.distribution_bundle_digest,
        "bundle digest mismatch for `{}` (expected {}, got {})",
        options.bundle_path.display(),
        to_hex(&rpt.distribution_bundle_digest),
        to_hex(&bundle_digest)
    );
    let report = RptVerificationReport {
        envelope_path: envelope_path.display().to_string(),
        schema_version: rpt.schema_version,
        event_id: rpt.event_id.as_name().to_string(),
        stream_id: rpt.stream_id.as_name().to_string(),
        rendition_id: rpt.rendition_id.as_name().to_string(),
        gar: DigestCheck {
            expected: to_hex(&rpt.gar_digest),
            verified_from: Some(options.gar_path.display().to_string()),
        },
        cek_receipt: DigestCheck {
            expected: to_hex(&rpt.cek_receipt_digest),
            verified_from: Some(options.cek_receipt_path.display().to_string()),
        },
        distribution_bundle: DigestCheck {
            expected: to_hex(&rpt.distribution_bundle_digest),
            verified_from: Some(options.bundle_path.display().to_string()),
        },
        policy_labels: rpt.policy_labels.clone(),
        valid_from_unix: rpt.valid_from_unix,
        valid_until_unix: rpt.valid_until_unix,
        notes: rpt.notes.clone(),
    };
    if let Some(target) = options.output {
        let value = json::to_value(&report)?;
        write_report_output(&value, target)?;
    } else {
        print_report(&report);
    }
    Ok(())
}
fn validate_report_output_path(options: &RptVerifyOptions) -> Result<()> {
    let Some(JsonTarget::File(output)) = options.output.as_ref() else {
        return Ok(());
    };
    validate_direct_report_output_path(output)?;
    let inputs = [
        ("RPT envelope", options.envelope_path.as_path()),
        ("GAR input", options.gar_path.as_path()),
        ("CEK receipt input", options.cek_receipt_path.as_path()),
        ("distribution bundle input", options.bundle_path.as_path()),
    ];
    for (input_label, input) in inputs {
        validate_distinct_artifact_paths(&[
            ("RPT verification report output", output.as_path()),
            (input_label, input),
        ])?;
    }
    Ok(())
}
fn validate_direct_report_output_path(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(eyre!(
                "RPT verification report output `{}` must not be a symlink",
                path.display()
            ));
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err(eyre!(
                "RPT verification report output `{}` must be a regular file",
                path.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(eyre!(
                "failed to inspect RPT verification report output `{}`: {error}",
                path.display()
            ));
        }
    }
    if let Some(parent) = path.parent() {
        for ancestor in parent.ancestors() {
            if ancestor.as_os_str().is_empty() {
                continue;
            }
            match fs::symlink_metadata(ancestor) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(eyre!(
                        "RPT verification report output parent `{}` must not be a symlink",
                        ancestor.display()
                    ));
                }
                Ok(metadata) if !metadata.is_dir() => {
                    return Err(eyre!(
                        "RPT verification report output parent `{}` must be a directory",
                        ancestor.display()
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(eyre!(
                        "failed to inspect RPT verification report output parent `{}`: {error}",
                        ancestor.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

fn write_report_output(value: &json::Value, target: JsonTarget) -> Result<()> {
    match target {
        JsonTarget::Stdout => {
            write_json_output(value, JsonTarget::Stdout).map_err(|err| eyre!(err.to_string()))
        }
        JsonTarget::File(path) => {
            let mut rendered = json::to_string_pretty(value)
                .map_err(|err| eyre!("failed to render RPT verification report: {err}"))?;
            rendered.push('\n');
            publish_report_file_with_hook(&path, rendered.as_bytes(), || Ok(()))
        }
    }
}

fn publish_report_file_with_hook<F>(path: &Path, bytes: &[u8], before_publish: F) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    let parent = report_output_parent(path);
    fs::create_dir_all(parent).wrap_err_with(|| {
        format!(
            "failed to create RPT verification report parent `{}`",
            parent.display()
        )
    })?;
    validate_direct_report_output_path(path)?;

    let mut builder = tempfile::Builder::new();
    builder.prefix(".taikai-rpt-report-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        builder.permissions(fs::Permissions::from_mode(0o666));
    }
    let mut staged = builder.tempfile_in(parent).wrap_err_with(|| {
        format!(
            "failed to create staging file for RPT verification report `{}`",
            path.display()
        )
    })?;
    staged.write_all(bytes).wrap_err_with(|| {
        format!(
            "failed to stage RPT verification report `{}`",
            path.display()
        )
    })?;
    before_publish()?;
    validate_direct_report_output_path(path)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            fs::set_permissions(staged.path(), metadata.permissions()).wrap_err_with(|| {
                format!(
                    "failed to preserve permissions for RPT verification report `{}`",
                    path.display()
                )
            })?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(eyre!(
                "failed to inspect RPT verification report `{}` before publication: {error}",
                path.display()
            ));
        }
    }
    staged.as_file_mut().sync_all().wrap_err_with(|| {
        format!(
            "failed to sync staged RPT verification report `{}`",
            path.display()
        )
    })?;
    staged.persist(path).map_err(|error| {
        eyre!(
            "failed to atomically publish RPT verification report `{}`: {}",
            path.display(),
            error.error
        )
    })?;
    sync_report_directory_chain(parent)?;
    Ok(())
}

fn report_output_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

#[cfg(unix)]
fn sync_report_directory_chain(parent: &Path) -> Result<()> {
    let canonical_parent = fs::canonicalize(parent).wrap_err_with(|| {
        format!(
            "failed to resolve RPT verification report parent `{}` for directory sync",
            parent.display()
        )
    })?;
    for directory in canonical_parent.ancestors() {
        fs::File::open(directory)
            .and_then(|file| file.sync_all())
            .wrap_err_with(|| {
                format!(
                    "failed to sync RPT verification report directory `{}`",
                    directory.display()
                )
            })?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn sync_report_directory_chain(_parent: &Path) -> Result<()> {
    Ok(())
}

fn open_regular_input(path: &Path, label: &str) -> Result<fs::File> {
    let path_metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to inspect {label} `{}`", path.display()))?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(eyre!(
            "{label} `{}` must be a regular file and must not be a symlink",
            path.display()
        ));
    }

    let mut options = OpenOptions::new();
    options.read(true);
    set_input_no_follow(&mut options);
    let file = options
        .open(path)
        .wrap_err_with(|| format!("failed to open {label} `{}`", path.display()))?;
    let opened_metadata = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect opened {label} `{}`", path.display()))?;
    if !opened_metadata.is_file() {
        return Err(eyre!(
            "{label} `{}` changed to a non-regular file while opening it",
            path.display()
        ));
    }
    ensure_same_file(&path_metadata, &opened_metadata, path, label)?;
    Ok(file)
}

fn read_policy_document(file: &mut fs::File, path: &Path, label: &str) -> Result<Vec<u8>> {
    let advertised_len = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect opened {label} `{}`", path.display()))?
        .len();
    if advertised_len > MAX_TAIKAI_POLICY_DOCUMENT_BYTES {
        return Err(eyre!(
            "{label} `{}` exceeds the {}-byte policy document limit",
            path.display(),
            MAX_TAIKAI_POLICY_DOCUMENT_BYTES
        ));
    }
    let capacity = usize::try_from(advertised_len).expect("bounded document length fits usize");
    let mut bytes = Vec::with_capacity(capacity);
    file.take(MAX_TAIKAI_POLICY_DOCUMENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .wrap_err_with(|| format!("failed to read {label} `{}`", path.display()))?;
    if u64::try_from(bytes.len()).expect("bounded document length fits u64")
        > MAX_TAIKAI_POLICY_DOCUMENT_BYTES
    {
        return Err(eyre!(
            "{label} `{}` grew beyond the {}-byte policy document limit while reading",
            path.display(),
            MAX_TAIKAI_POLICY_DOCUMENT_BYTES
        ));
    }
    Ok(bytes)
}

#[cfg(unix)]
fn ensure_same_file(
    expected: &fs::Metadata,
    opened: &fs::Metadata,
    path: &Path,
    label: &str,
) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;
    ensure!(
        expected.dev() == opened.dev() && expected.ino() == opened.ino(),
        "{label} `{}` changed while it was being opened",
        path.display()
    );
    Ok(())
}

#[cfg(not(unix))]
fn ensure_same_file(
    _expected: &fs::Metadata,
    _opened: &fs::Metadata,
    _path: &Path,
    _label: &str,
) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_input_no_follow(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt as _;
    options.custom_flags(input_no_follow_flag());
}

#[cfg(not(unix))]
fn set_input_no_follow(_options: &mut OpenOptions) {}

#[cfg(any(target_os = "linux", target_os = "android"))]
const fn input_no_follow_flag() -> i32 {
    0o400000
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
const fn input_no_follow_flag() -> i32 {
    0x100
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
const fn input_no_follow_flag() -> i32 {
    0
}

fn load_rpt(path: &Path) -> Result<ReplicationProofTokenV1> {
    let mut file = open_regular_input(path, "RPT envelope")?;
    let bytes = read_policy_document(&mut file, path, "RPT envelope")?;
    if let Ok(rpt) = norito::decode_from_bytes::<ReplicationProofTokenV1>(&bytes) {
        return Ok(rpt);
    }
    let text = String::from_utf8(bytes).map_err(|err| {
        eyre!(
            "failed to decode `{}` as Norito or UTF-8 JSON: {err}",
            path.display()
        )
    })?;
    json::from_str(&text).wrap_err_with(|| format!("failed to parse RPT JSON `{}`", path.display()))
}
fn validate_cek_receipt_binding(
    path: &Path,
    event_id: &TaikaiEventId,
    stream_id: &TaikaiStreamId,
) -> Result<[u8; 32]> {
    let mut file = open_regular_input(path, "CEK receipt")?;
    let bytes = read_policy_document(&mut file, path, "CEK receipt")?;
    let receipt = norito::decode_from_bytes::<CekRotationReceiptV1>(&bytes).map_err(|err| {
        eyre!(
            "failed to decode CEK receipt `{}` as canonical framed Norito: {err}",
            path.display()
        )
    })?;
    receipt
        .validate()
        .wrap_err_with(|| format!("invalid CEK receipt `{}`", path.display()))?;
    ensure!(
        &receipt.event_id == event_id && &receipt.stream_id == stream_id,
        "CEK receipt `{}` scope {}/{} does not match RPT scope {}/{}",
        path.display(),
        receipt.event_id,
        receipt.stream_id,
        event_id,
        stream_id
    );
    Ok(*blake3::hash(&bytes).as_bytes())
}
fn compute_file_digest(path: &Path) -> Result<[u8; 32]> {
    let mut file = open_regular_input(path, "policy input")?;
    let mut hasher = Hasher::new();
    hash_file_contents(&mut file, path, &mut hasher)?;
    Ok(*hasher.finalize().as_bytes())
}
fn compute_bundle_digest(path: &Path) -> Result<[u8; 32]> {
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to stat `{}`", path.display()))?;
    let mut hasher = Hasher::new();
    hasher.update(TAIKAI_BUNDLE_DIGEST_DOMAIN_V1);
    if metadata.is_file() {
        let relative = path
            .file_name()
            .map_or_else(|| PathBuf::from("."), PathBuf::from);
        hash_file_entry(path, &relative, &mut hasher)?;
    } else if metadata.is_dir() {
        hash_directory_entry(path, Path::new(""), &mut hasher)?;
    } else {
        return Err(eyre!(
            "bundle `{}` must be a regular file or directory",
            path.display()
        ));
    }
    Ok(*hasher.finalize().as_bytes())
}
fn hash_path_entry(path: &Path, relative: &Path, hasher: &mut Hasher) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to stat `{}`", path.display()))?;
    if metadata.is_file() {
        hash_file_entry(path, relative, hasher)
    } else if metadata.is_dir() {
        hash_directory_entry(path, relative, hasher)
    } else {
        Err(eyre!(
            "unsupported entry type at `{}` (expected file or directory)",
            path.display()
        ))
    }
}
fn hash_file_entry(path: &Path, relative: &Path, hasher: &mut Hasher) -> Result<()> {
    update_path_marker(relative, b'F', hasher)?;
    let mut file = open_regular_input(path, "bundle file")?;
    let expected_len = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect `{}`", path.display()))?
        .len();
    hasher.update(&expected_len.to_le_bytes());
    let mut buffer = [0u8; 8192];
    let mut actual_len = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .wrap_err_with(|| format!("failed to read `{}`", path.display()))?;
        if read == 0 {
            break;
        }
        actual_len = actual_len
            .checked_add(u64::try_from(read).expect("read buffer length fits u64"))
            .ok_or_else(|| {
                eyre!(
                    "bundle file length overflowed while reading `{}`",
                    path.display()
                )
            })?;
        hasher.update(&buffer[..read]);
    }
    ensure!(
        actual_len == expected_len,
        "bundle file `{}` changed length while hashing (expected {expected_len}, read {actual_len})",
        path.display()
    );
    Ok(())
}
fn hash_file_contents(file: &mut fs::File, path: &Path, hasher: &mut Hasher) -> Result<()> {
    let mut buffer = [0u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .wrap_err_with(|| format!("failed to read `{}`", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(())
}
fn hash_directory_entry(path: &Path, relative: &Path, hasher: &mut Hasher) -> Result<()> {
    update_path_marker(relative, b'D', hasher)?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(path)
        .wrap_err_with(|| format!("failed to read directory `{}`", path.display()))?
    {
        let entry =
            entry.wrap_err_with(|| format!("failed to iterate directory `{}`", path.display()))?;
        let child_path = entry.path();
        let file_name = entry
            .file_name()
            .into_string()
            .map_err(|_| eyre!("bundle entry `{}` is not valid UTF-8", child_path.display()))?;
        entries.push((file_name, child_path));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    for (file_name, child_path) in entries {
        let mut child_relative = if relative.as_os_str().is_empty() {
            PathBuf::new()
        } else {
            relative.to_path_buf()
        };
        child_relative.push(file_name);
        hash_path_entry(&child_path, &child_relative, hasher)?;
    }
    Ok(())
}
fn update_path_marker(relative: &Path, kind: u8, hasher: &mut Hasher) -> Result<()> {
    let label = canonical_bundle_relative_path(relative)?;
    let label_len = u64::try_from(label.len())
        .map_err(|_| eyre!("bundle path is too long to hash canonically"))?;
    hasher.update(&[kind]);
    hasher.update(&label_len.to_le_bytes());
    hasher.update(label.as_bytes());
    Ok(())
}
fn canonical_bundle_relative_path(relative: &Path) -> Result<String> {
    if relative.as_os_str().is_empty() {
        return Ok(".".to_string());
    }
    let mut label = String::new();
    for component in relative.components() {
        let std::path::Component::Normal(component) = component else {
            return Err(eyre!(
                "bundle path `{}` is not a canonical relative path",
                relative.display()
            ));
        };
        let component = component
            .to_str()
            .ok_or_else(|| eyre!("bundle path `{}` is not valid UTF-8", relative.display()))?;
        if !label.is_empty() {
            label.push('/');
        }
        label.push_str(component);
    }
    Ok(label)
}
fn to_hex(digest: &[u8; 32]) -> String {
    hex::encode_upper(digest)
}
fn print_report(report: &RptVerificationReport) {
    println!("Taikai replication proof token verified");
    println!("  envelope: {}", report.envelope_path);
    println!("  schema_version: {}", report.schema_version);
    println!(
        "  scope: event={} stream={} rendition={}",
        report.event_id, report.stream_id, report.rendition_id
    );
    println!(
        "  valid_unix: {} -> {}",
        report.valid_from_unix, report.valid_until_unix
    );
    print_digest("GAR digest", &report.gar);
    print_digest("CEK receipt digest", &report.cek_receipt);
    print_digest("bundle digest", &report.distribution_bundle);
    if report.policy_labels.is_empty() {
        println!("  policy_labels: <none>");
    } else {
        println!("  policy_labels: {}", report.policy_labels.join(", "));
    }
    if let Some(notes) = &report.notes {
        println!("  notes: {notes}");
    }
}
fn print_digest(label: &str, digest: &DigestCheck) {
    println!("  {label}: {}", digest.expected);
    if let Some(source) = &digest.verified_from {
        println!("    verified_from: {source}");
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{
        name::Name,
        taikai::{
            CEK_ROTATION_RECEIPT_VERSION_V1, CekRotationReceiptV1,
            REPLICATION_PROOF_TOKEN_VERSION_V1, TaikaiEventId, TaikaiRenditionId, TaikaiStreamId,
        },
    };
    use std::str::FromStr;
    use tempfile::tempdir;
    fn sample_name(raw: &str) -> Name {
        Name::from_str(raw).expect("valid name")
    }
    fn build_rpt(
        gar_digest: [u8; 32],
        cek_digest: [u8; 32],
        bundle_digest: [u8; 32],
    ) -> ReplicationProofTokenV1 {
        ReplicationProofTokenV1 {
            schema_version: REPLICATION_PROOF_TOKEN_VERSION_V1,
            event_id: TaikaiEventId::new(sample_name("global-keynote")),
            stream_id: TaikaiStreamId::new(sample_name("stage-a")),
            rendition_id: TaikaiRenditionId::new(sample_name("primary")),
            gar_digest,
            cek_receipt_digest: cek_digest,
            distribution_bundle_digest: bundle_digest,
            policy_labels: vec!["docs-portal".to_string()],
            valid_from_unix: 1_700_000_000,
            valid_until_unix: 1_700_086_400,
            notes: Some("test-attestation".to_string()),
        }
    }
    fn write_cek_receipt(path: &Path, event_id: &str, stream_id: &str) {
        let receipt = CekRotationReceiptV1 {
            schema_version: CEK_ROTATION_RECEIPT_VERSION_V1,
            event_id: TaikaiEventId::new(sample_name(event_id)),
            stream_id: TaikaiStreamId::new(sample_name(stream_id)),
            kms_profile: "kms/default".to_string(),
            new_wrap_key_label: "wrap-v2".to_string(),
            previous_wrap_key_label: Some("wrap-v1".to_string()),
            hkdf_salt: [0xA5; 32],
            effective_segment_sequence: 42,
            issued_at_unix: 1_700_000_000,
            notes: None,
        };
        fs::write(path, norito::to_bytes(&receipt).unwrap()).unwrap();
    }
    #[test]
    fn rpt_verify_accepts_matching_inputs() {
        let dir = tempdir().unwrap();
        let gar_path = dir.path().join("gar.json");
        fs::write(&gar_path, b"{\"gar\":\"v2\"}").unwrap();
        let cek_path = dir.path().join("cek_receipt.to");
        write_cek_receipt(&cek_path, "global-keynote", "stage-a");
        let bundle_dir = dir.path().join("bundle");
        fs::create_dir_all(&bundle_dir).unwrap();
        fs::write(bundle_dir.join("artifact.bin"), b"bundle-bytes").unwrap();
        let gar_digest = compute_file_digest(&gar_path).unwrap();
        let cek_digest = compute_file_digest(&cek_path).unwrap();
        let bundle_digest = compute_bundle_digest(&bundle_dir).unwrap();
        let rpt = build_rpt(gar_digest, cek_digest, bundle_digest);
        let envelope_path = dir.path().join("attestation.to");
        fs::write(&envelope_path, norito::to_bytes(&rpt).unwrap()).unwrap();
        let report_path = dir.path().join("verification.json");
        fs::write(&report_path, b"old report").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&report_path, fs::Permissions::from_mode(0o640)).unwrap();
        }
        run_rpt_verify(RptVerifyOptions {
            envelope_path,
            gar_path,
            cek_receipt_path: cek_path,
            bundle_path: bundle_dir.clone(),
            output: Some(JsonTarget::File(report_path.clone())),
        })
        .expect("verification should pass");
        let report: json::Value =
            json::from_slice(&fs::read(&report_path).unwrap()).expect("report JSON");
        assert_eq!(
            report
                .get("distribution_bundle")
                .and_then(|value| value.get("verified_from"))
                .and_then(json::Value::as_str),
            Some(bundle_dir.to_str().expect("UTF-8 fixture path"))
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                fs::metadata(&report_path).unwrap().permissions().mode() & 0o777,
                0o640
            );
        }
    }
    #[test]
    fn rpt_verify_rejects_mismatch() {
        let dir = tempdir().unwrap();
        let gar_path = dir.path().join("gar.json");
        fs::write(&gar_path, b"{\"gar\":\"v2\"}").unwrap();
        let cek_path = dir.path().join("cek_receipt.to");
        fs::write(&cek_path, b"{\"cek\":\"receipt\"}").unwrap();
        let bundle_path = dir.path().join("bundle.tar");
        fs::write(&bundle_path, b"bundle").unwrap();
        let gar_digest = compute_file_digest(&gar_path).unwrap();
        let cek_digest = compute_file_digest(&cek_path).unwrap();
        let bundle_digest = compute_bundle_digest(&bundle_path).unwrap();
        let rpt = build_rpt(gar_digest, cek_digest, bundle_digest);
        let envelope_path = dir.path().join("attestation.to");
        fs::write(&envelope_path, norito::to_bytes(&rpt).unwrap()).unwrap();
        fs::write(&gar_path, b"{\"gar\":\"drift\"}").unwrap();
        let err = run_rpt_verify(RptVerifyOptions {
            envelope_path,
            gar_path: gar_path.clone(),
            cek_receipt_path: cek_path,
            bundle_path,
            output: None,
        })
        .expect_err("mismatched digest must fail");
        assert!(err.to_string().contains("GAR digest mismatch"),);
    }
    #[test]
    fn file_digest_is_independent_of_local_filename() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("gar-original.json");
        let renamed = dir.path().join("gar-renamed.json");
        fs::write(&first, b"identical GAR payload").unwrap();
        fs::write(&renamed, b"identical GAR payload").unwrap();

        assert_eq!(
            compute_file_digest(&first).unwrap(),
            compute_file_digest(&renamed).unwrap()
        );
    }
    #[test]
    fn bundle_digest_length_frames_file_contents() {
        let dir = tempdir().unwrap();
        let forged = dir.path().join("forged");
        let structured = dir.path().join("structured");
        fs::create_dir_all(&forged).unwrap();
        fs::create_dir_all(&structured).unwrap();
        let mut forged_contents = b"b".to_vec();
        forged_contents.extend_from_slice(b"c");
        forged_contents.extend_from_slice(&[0xFF, b'F']);
        forged_contents.extend_from_slice(b"d");
        fs::write(forged.join("a"), forged_contents).unwrap();
        fs::write(structured.join("a"), b"b").unwrap();
        fs::write(structured.join("c"), b"d").unwrap();

        assert_ne!(
            compute_bundle_digest(&forged).unwrap(),
            compute_bundle_digest(&structured).unwrap(),
            "file length framing must distinguish bytes that imitate a second entry"
        );
    }
    #[test]
    fn bundle_digest_matches_canonical_v1_test_vector() {
        let dir = tempdir().unwrap();
        let bundle = dir.path().join("bundle");
        fs::create_dir_all(bundle.join("nested")).unwrap();
        fs::write(bundle.join("nested/beta"), b"BC").unwrap();
        fs::write(bundle.join("alpha"), b"A").unwrap();

        assert_eq!(
            hex::encode(compute_bundle_digest(&bundle).unwrap()),
            "32b42aff6303e492d041c7620f8b98f3dc1ee1f613de002a35c51b428d940846"
        );
    }
    #[test]
    fn rpt_verify_rejects_cek_receipt_for_another_scope() {
        let dir = tempdir().unwrap();
        let cek_path = dir.path().join("cek_receipt.to");
        write_cek_receipt(&cek_path, "other-event", "stage-a");
        let gar_path = dir.path().join("gar.json");
        fs::write(&gar_path, b"gar").unwrap();
        let bundle_path = dir.path().join("bundle.bin");
        fs::write(&bundle_path, b"bundle").unwrap();
        let gar_digest = compute_file_digest(&gar_path).unwrap();
        let cek_digest = compute_file_digest(&cek_path).unwrap();
        let bundle_digest = compute_bundle_digest(&bundle_path).unwrap();
        let rpt = build_rpt(gar_digest, cek_digest, bundle_digest);
        let envelope_path = dir.path().join("attestation.to");
        fs::write(&envelope_path, norito::to_bytes(&rpt).unwrap()).unwrap();

        let err = run_rpt_verify(RptVerifyOptions {
            envelope_path,
            gar_path,
            cek_receipt_path: cek_path,
            bundle_path,
            output: None,
        })
        .expect_err("cross-event CEK receipt must fail");

        assert!(err.to_string().contains("does not match RPT scope"));
    }
    #[test]
    fn rpt_verify_accepts_json_input() {
        let dir = tempdir().unwrap();
        let gar_path = dir.path().join("gar.json");
        fs::write(&gar_path, b"gar-json").unwrap();
        let cek_path = dir.path().join("cek.to");
        write_cek_receipt(&cek_path, "global-keynote", "stage-a");
        let bundle_path = dir.path().join("bundle.bin");
        fs::write(&bundle_path, b"bytes").unwrap();
        let gar_digest = compute_file_digest(&gar_path).unwrap();
        let cek_digest = compute_file_digest(&cek_path).unwrap();
        let bundle_digest = compute_bundle_digest(&bundle_path).unwrap();
        let rpt = build_rpt(gar_digest, cek_digest, bundle_digest);
        let envelope_path = dir.path().join("attestation.json");
        let json_text = norito::json::to_json_pretty(&rpt).expect("render JSON");
        fs::write(&envelope_path, json_text).unwrap();
        run_rpt_verify(RptVerifyOptions {
            envelope_path,
            gar_path,
            cek_receipt_path: cek_path,
            bundle_path,
            output: None,
        })
        .expect("json verification should pass");
    }

    #[test]
    fn rpt_verify_rejects_invalid_validity_window() {
        let dir = tempdir().unwrap();
        let mut rpt = build_rpt([0x11; 32], [0x22; 32], [0x33; 32]);
        rpt.valid_until_unix = rpt.valid_from_unix;
        let envelope_path = dir.path().join("attestation.to");
        fs::write(&envelope_path, norito::to_bytes(&rpt).unwrap()).unwrap();

        let err = run_rpt_verify(RptVerifyOptions {
            envelope_path,
            gar_path: dir.path().join("unused-gar"),
            cek_receipt_path: dir.path().join("unused-cek"),
            bundle_path: dir.path().join("unused-bundle"),
            output: None,
        })
        .expect_err("empty validity windows must fail");

        assert!(err.to_string().contains("validity window is invalid"));
    }

    #[test]
    fn rpt_verify_rejects_report_output_aliases_before_reading_inputs() {
        let dir = tempdir().unwrap();
        let envelope_path = dir.path().join("attestation.to");
        fs::write(&envelope_path, b"preserve-envelope").unwrap();
        let gar_path = dir.path().join("gar.json");
        let cek_path = dir.path().join("cek.to");
        let bundle_input = dir.path().join("bundle-input");

        let err = run_rpt_verify(RptVerifyOptions {
            envelope_path: envelope_path.clone(),
            gar_path: gar_path.clone(),
            cek_receipt_path: cek_path.clone(),
            bundle_path: bundle_input,
            output: Some(JsonTarget::File(envelope_path.clone())),
        })
        .expect_err("report must not overwrite its RPT envelope");
        assert!(err.to_string().contains("distinct paths"));
        assert_eq!(fs::read(&envelope_path).unwrap(), b"preserve-envelope");

        let bundle = dir.path().join("bundle");
        fs::create_dir(&bundle).unwrap();
        let nested_output = bundle.join("verification.json");
        let err = run_rpt_verify(RptVerifyOptions {
            envelope_path,
            gar_path,
            cek_receipt_path: cek_path,
            bundle_path: bundle,
            output: Some(JsonTarget::File(nested_output.clone())),
        })
        .expect_err("report must not be written inside an attested bundle");
        assert!(err.to_string().contains("nested paths"));
        assert!(!nested_output.exists());
    }

    #[cfg(unix)]
    #[test]
    fn rpt_verify_rejects_report_output_through_symlinked_parent() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let envelope_path = dir.path().join("attestation.to");
        fs::write(&envelope_path, b"preserve-envelope").unwrap();
        let gar_path = dir.path().join("gar.json");
        let cek_path = dir.path().join("cek.to");
        let bundle = dir.path().join("bundle");
        let bundle_link = dir.path().join("bundle-link");
        fs::create_dir(&bundle).unwrap();
        symlink(&bundle, &bundle_link).unwrap();
        let output = bundle_link.join("verification.json");

        let err = run_rpt_verify(RptVerifyOptions {
            envelope_path,
            gar_path,
            cek_receipt_path: cek_path,
            bundle_path: bundle,
            output: Some(JsonTarget::File(output.clone())),
        })
        .expect_err("symlinked report parent must fail before reading the envelope");

        assert!(err.to_string().contains("parent"));
        assert!(err.to_string().contains("must not be a symlink"));
        assert!(!output.exists());
    }

    #[cfg(unix)]
    #[test]
    fn rpt_verify_rejects_symlinked_envelope() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let gar_path = dir.path().join("gar.json");
        fs::write(&gar_path, b"gar").unwrap();
        let cek_path = dir.path().join("cek.to");
        write_cek_receipt(&cek_path, "global-keynote", "stage-a");
        let bundle_path = dir.path().join("bundle.bin");
        fs::write(&bundle_path, b"bundle").unwrap();
        let rpt = build_rpt(
            compute_file_digest(&gar_path).unwrap(),
            compute_file_digest(&cek_path).unwrap(),
            compute_bundle_digest(&bundle_path).unwrap(),
        );
        let envelope_target = dir.path().join("attestation-target.to");
        fs::write(&envelope_target, norito::to_bytes(&rpt).unwrap()).unwrap();
        let envelope_link = dir.path().join("attestation.to");
        symlink(&envelope_target, &envelope_link).unwrap();

        let error = run_rpt_verify(RptVerifyOptions {
            envelope_path: envelope_link,
            gar_path,
            cek_receipt_path: cek_path,
            bundle_path,
            output: None,
        })
        .expect_err("RPT envelope symlinks must fail closed");

        assert!(error.to_string().contains("RPT envelope"));
        assert!(error.to_string().contains("must not be a symlink"));
    }

    #[test]
    fn rpt_loader_rejects_oversized_policy_document() {
        let dir = tempdir().unwrap();
        let envelope_path = dir.path().join("oversized.to");
        fs::File::create(&envelope_path)
            .unwrap()
            .set_len(MAX_TAIKAI_POLICY_DOCUMENT_BYTES + 1)
            .unwrap();

        let error = load_rpt(&envelope_path).expect_err("oversized RPT must fail before decoding");

        assert!(error.to_string().contains("policy document limit"));
    }

    #[cfg(unix)]
    #[test]
    fn report_writer_rechecks_late_symlink_without_touching_victim() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let output = dir.path().join("report.json");
        let victim = dir.path().join("victim.json");
        fs::write(&victim, b"preserve-victim").unwrap();

        let error = publish_report_file_with_hook(&output, b"{\"verified\":true}\n", || {
            symlink(&victim, &output).wrap_err("create late report symlink")?;
            Ok(())
        })
        .expect_err("late output symlink must fail before publication");

        assert!(error.to_string().contains("must not be a symlink"));
        assert_eq!(fs::read(&victim).unwrap(), b"preserve-victim");
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            2,
            "failed publication must clean its staging file"
        );
    }
}
