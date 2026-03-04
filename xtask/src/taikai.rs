use std::{
    borrow::Cow,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use blake3::Hasher;
use eyre::{Context as _, Result, ensure, eyre};
use iroha_data_model::taikai::{REPLICATION_PROOF_TOKEN_VERSION_V1, ReplicationProofTokenV1};
use norito::{codec::Decode, derive::JsonSerialize, json};

use crate::{JsonTarget, write_json_output};

#[derive(Debug)]
pub struct RptVerifyOptions {
    pub envelope_path: PathBuf,
    pub gar_path: Option<PathBuf>,
    pub cek_receipt_path: Option<PathBuf>,
    pub bundle_path: Option<PathBuf>,
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
    let envelope_path = options.envelope_path.clone();
    let rpt = load_rpt(&envelope_path)?;
    ensure!(
        rpt.schema_version == REPLICATION_PROOF_TOKEN_VERSION_V1,
        "rpt schema version {} is unsupported (expected {})",
        rpt.schema_version,
        REPLICATION_PROOF_TOKEN_VERSION_V1
    );
    let gar_verified = if let Some(path) = options.gar_path {
        let digest = compute_file_digest(&path)
            .wrap_err_with(|| format!("failed to hash GAR `{}`", path.display()))?;
        ensure!(
            digest == rpt.gar_digest,
            "GAR digest mismatch for `{}` (expected {}, got {})",
            path.display(),
            to_hex(&rpt.gar_digest),
            to_hex(&digest)
        );
        Some(path)
    } else {
        None
    };
    let cek_verified = if let Some(path) = options.cek_receipt_path {
        let digest = compute_file_digest(&path)
            .wrap_err_with(|| format!("failed to hash CEK receipt `{}`", path.display()))?;
        ensure!(
            digest == rpt.cek_receipt_digest,
            "CEK receipt digest mismatch for `{}` (expected {}, got {})",
            path.display(),
            to_hex(&rpt.cek_receipt_digest),
            to_hex(&digest)
        );
        Some(path)
    } else {
        None
    };
    let bundle_verified = if let Some(path) = options.bundle_path {
        let digest = compute_bundle_digest(&path)
            .wrap_err_with(|| format!("failed to hash bundle `{}`", path.display()))?;
        ensure!(
            digest == rpt.distribution_bundle_digest,
            "bundle digest mismatch for `{}` (expected {}, got {})",
            path.display(),
            to_hex(&rpt.distribution_bundle_digest),
            to_hex(&digest)
        );
        Some(path)
    } else {
        None
    };

    let report = RptVerificationReport {
        envelope_path: envelope_path.display().to_string(),
        schema_version: rpt.schema_version,
        event_id: rpt.event_id.as_name().to_string(),
        stream_id: rpt.stream_id.as_name().to_string(),
        rendition_id: rpt.rendition_id.as_name().to_string(),
        gar: DigestCheck {
            expected: to_hex(&rpt.gar_digest),
            verified_from: gar_verified.as_ref().map(|path| path.display().to_string()),
        },
        cek_receipt: DigestCheck {
            expected: to_hex(&rpt.cek_receipt_digest),
            verified_from: cek_verified.as_ref().map(|path| path.display().to_string()),
        },
        distribution_bundle: DigestCheck {
            expected: to_hex(&rpt.distribution_bundle_digest),
            verified_from: bundle_verified
                .as_ref()
                .map(|path| path.display().to_string()),
        },
        policy_labels: rpt.policy_labels.clone(),
        valid_from_unix: rpt.valid_from_unix,
        valid_until_unix: rpt.valid_until_unix,
        notes: rpt.notes.clone(),
    };

    if let Some(target) = options.output {
        let value = json::to_value(&report)?;
        write_json_output(&value, target).map_err(|err| eyre!(err.to_string()))?;
    } else {
        print_report(&report);
    }
    Ok(())
}

fn load_rpt(path: &Path) -> Result<ReplicationProofTokenV1> {
    let bytes = fs::read(path).wrap_err_with(|| format!("failed to read `{}`", path.display()))?;
    {
        let mut cursor = bytes.as_slice();
        if let Ok(rpt) = ReplicationProofTokenV1::decode(&mut cursor) {
            return Ok(rpt);
        }
    }
    let text = String::from_utf8(bytes).map_err(|err| {
        eyre!(
            "failed to decode `{}` as Norito or UTF-8 JSON: {err}",
            path.display()
        )
    })?;
    json::from_str(&text).wrap_err_with(|| format!("failed to parse RPT JSON `{}`", path.display()))
}

fn compute_file_digest(path: &Path) -> Result<[u8; 32]> {
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to stat `{}`", path.display()))?;
    if !metadata.is_file() {
        return Err(eyre!("`{}` is not a regular file", path.display()));
    }
    let mut hasher = Hasher::new();
    let relative = path
        .file_name()
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    hash_file_entry(path, &relative, &mut hasher)?;
    Ok(*hasher.finalize().as_bytes())
}

fn compute_bundle_digest(path: &Path) -> Result<[u8; 32]> {
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to stat `{}`", path.display()))?;
    let mut hasher = Hasher::new();
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
    update_path_marker(relative, b'F', hasher);
    let mut file =
        fs::File::open(path).wrap_err_with(|| format!("failed to open `{}`", path.display()))?;
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
    update_path_marker(relative, b'D', hasher);
    let mut entries = Vec::new();
    for entry in fs::read_dir(path)
        .wrap_err_with(|| format!("failed to read directory `{}`", path.display()))?
    {
        entries.push(
            entry.wrap_err_with(|| format!("failed to iterate directory `{}`", path.display()))?,
        );
    }
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let child_path = entry.path();
        let mut child_relative = if relative.as_os_str().is_empty() {
            PathBuf::new()
        } else {
            relative.to_path_buf()
        };
        child_relative.push(entry.file_name());
        hash_path_entry(&child_path, &child_relative, hasher)?;
    }
    Ok(())
}

fn update_path_marker(relative: &Path, kind: u8, hasher: &mut Hasher) {
    let label: Cow<'_, str> = if relative.as_os_str().is_empty() {
        Cow::Borrowed(".")
    } else {
        Cow::Owned(relative.to_string_lossy().into_owned())
    };
    hasher.update(label.as_bytes());
    hasher.update(&[0xFF, kind]);
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
    use std::str::FromStr;

    use iroha_data_model::{
        name::Name,
        taikai::{TaikaiEventId, TaikaiRenditionId, TaikaiStreamId},
    };
    use norito::codec::Encode;
    use tempfile::tempdir;

    use super::*;

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

    #[test]
    fn rpt_verify_accepts_matching_inputs() {
        let dir = tempdir().unwrap();
        let gar_path = dir.path().join("gar.json");
        fs::write(&gar_path, b"{\"gar\":\"v2\"}").unwrap();
        let cek_path = dir.path().join("cek_receipt.to");
        fs::write(&cek_path, b"{\"cek\":\"receipt\"}").unwrap();
        let bundle_dir = dir.path().join("bundle");
        fs::create_dir_all(&bundle_dir).unwrap();
        fs::write(bundle_dir.join("artifact.bin"), b"bundle-bytes").unwrap();

        let gar_digest = compute_file_digest(&gar_path).unwrap();
        let cek_digest = compute_file_digest(&cek_path).unwrap();
        let bundle_digest = compute_bundle_digest(&bundle_dir).unwrap();
        let rpt = build_rpt(gar_digest, cek_digest, bundle_digest);
        let envelope_path = dir.path().join("attestation.to");
        fs::write(&envelope_path, rpt.encode()).unwrap();

        run_rpt_verify(RptVerifyOptions {
            envelope_path,
            gar_path: Some(gar_path),
            cek_receipt_path: Some(cek_path),
            bundle_path: Some(bundle_dir),
            output: None,
        })
        .expect("verification should pass");
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
        fs::write(&envelope_path, rpt.encode()).unwrap();

        fs::write(&gar_path, b"{\"gar\":\"drift\"}").unwrap();
        let err = run_rpt_verify(RptVerifyOptions {
            envelope_path,
            gar_path: Some(gar_path.clone()),
            cek_receipt_path: Some(cek_path),
            bundle_path: Some(bundle_path),
            output: None,
        })
        .expect_err("mismatched digest must fail");
        assert!(err.to_string().contains("GAR digest mismatch"),);
    }

    #[test]
    fn rpt_verify_accepts_json_input() {
        let dir = tempdir().unwrap();
        let gar_path = dir.path().join("gar.json");
        fs::write(&gar_path, b"gar-json").unwrap();
        let cek_path = dir.path().join("cek.json");
        fs::write(&cek_path, b"cek-json").unwrap();
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
            gar_path: None,
            cek_receipt_path: None,
            bundle_path: None,
            output: None,
        })
        .expect("json verification should pass");
    }
}
