use crate::{JsonTarget, write_json_output};
use eyre::{Context as _, Result, ensure, eyre};
use iroha_data_model::taikai::{
    CekRotationReceiptV1, ReplicationProofTokenV1, TaikaiEventId, TaikaiStreamId,
};
use norito::{derive::JsonSerialize, json};
use sorafs_car::{
    taikai::validate_distinct_artifact_paths,
    taikai_bundle::{self, bundle_digest_v1, open_regular_input, read_policy_document},
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
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
    let gar_digest = taikai_bundle::file_digest(&options.gar_path, "policy input")
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
    let bundle_digest = bundle_digest_v1(&options.bundle_path)
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
        Ok(metadata) if metadata_is_symlink_or_reparse(&metadata) => {
            return Err(eyre!(
                "RPT verification report output `{}` must not be a symlink or reparse point",
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
    let parent = report_output_parent(path);
    let mut ancestors = parent
        .ancestors()
        .filter(|ancestor| !ancestor.as_os_str().is_empty())
        .collect::<Vec<_>>();
    ancestors.reverse();
    for ancestor in ancestors {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata_is_symlink_or_reparse(&metadata) => {
                return Err(eyre!(
                    "RPT verification report output parent `{}` must not be a symlink or reparse point",
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
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(eyre!(
                    "RPT verification report output parent `{}` must already exist as a directory",
                    ancestor.display()
                ));
            }
            Err(error) => {
                return Err(eyre!(
                    "failed to inspect RPT verification report output parent `{}`: {error}",
                    ancestor.display()
                ));
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
            staged
                .as_file_mut()
                .set_permissions(metadata.permissions())
                .wrap_err_with(|| {
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
    sync_report_parent_directory(parent)?;
    Ok(())
}

fn report_output_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

#[cfg(unix)]
fn sync_report_parent_directory(parent: &Path) -> Result<()> {
    fs::File::open(parent)
        .and_then(|file| file.sync_all())
        .wrap_err_with(|| {
            format!(
                "failed to sync RPT verification report directory `{}`",
                parent.display()
            )
        })
}

#[cfg(not(unix))]
fn sync_report_parent_directory(_parent: &Path) -> Result<()> {
    Ok(())
}

fn metadata_is_symlink_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
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
    use iroha_data_model::taikai::{
        CEK_ROTATION_RECEIPT_VERSION_V1, CekRotationReceiptV1, REPLICATION_PROOF_TOKEN_VERSION_V1,
        TaikaiEventId, TaikaiRenditionId, TaikaiStreamId,
    };
    use iroha_model_base::name::Name;
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
        let gar_digest = taikai_bundle::file_digest(&gar_path, "policy input").unwrap();
        let cek_digest = taikai_bundle::file_digest(&cek_path, "policy input").unwrap();
        let bundle_digest = bundle_digest_v1(&bundle_dir).unwrap();
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
        let gar_digest = taikai_bundle::file_digest(&gar_path, "policy input").unwrap();
        let cek_digest = taikai_bundle::file_digest(&cek_path, "policy input").unwrap();
        let bundle_digest = bundle_digest_v1(&bundle_path).unwrap();
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
    fn rpt_verify_rejects_cek_receipt_for_another_scope() {
        let dir = tempdir().unwrap();
        let cek_path = dir.path().join("cek_receipt.to");
        write_cek_receipt(&cek_path, "other-event", "stage-a");
        let gar_path = dir.path().join("gar.json");
        fs::write(&gar_path, b"gar").unwrap();
        let bundle_path = dir.path().join("bundle.bin");
        fs::write(&bundle_path, b"bundle").unwrap();
        let gar_digest = taikai_bundle::file_digest(&gar_path, "policy input").unwrap();
        let cek_digest = taikai_bundle::file_digest(&cek_path, "policy input").unwrap();
        let bundle_digest = bundle_digest_v1(&bundle_path).unwrap();
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
        let gar_digest = taikai_bundle::file_digest(&gar_path, "policy input").unwrap();
        let cek_digest = taikai_bundle::file_digest(&cek_path, "policy input").unwrap();
        let bundle_digest = bundle_digest_v1(&bundle_path).unwrap();
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
    fn report_publish_does_not_create_directories_through_symlinked_parent() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let victim = dir.path().join("victim");
        let parent_link = dir.path().join("report-link");
        fs::create_dir(&victim).unwrap();
        symlink(&victim, &parent_link).unwrap();
        let output = parent_link.join("new").join("report.json");

        let error = publish_report_file_with_hook(&output, b"{}\n", || Ok(()))
            .expect_err("a symlinked missing output parent must fail without side effects");

        assert!(error.to_string().contains("must not be a symlink"));
        assert!(!victim.join("new").exists());
    }

    #[cfg(windows)]
    #[test]
    fn report_publish_rejects_windows_reparse_target() {
        use std::os::windows::fs::symlink_file;

        let dir = tempdir().unwrap();
        let victim = dir.path().join("victim");
        let output = dir.path().join("report.json");
        fs::write(&victim, b"victim").unwrap();
        match symlink_file(&victim, &output) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(error) => panic!("create report reparse point: {error}"),
        }

        let error = publish_report_file_with_hook(&output, b"{}\n", || Ok(()))
            .expect_err("a reparse report target must fail closed");

        assert!(error.to_string().contains("reparse point"));
        assert_eq!(fs::read(&victim).unwrap(), b"victim");
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
            taikai_bundle::file_digest(&gar_path, "policy input").unwrap(),
            taikai_bundle::file_digest(&cek_path, "policy input").unwrap(),
            bundle_digest_v1(&bundle_path).unwrap(),
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
            .set_len(taikai_bundle::MAX_TAIKAI_POLICY_DOCUMENT_BYTES + 1)
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
