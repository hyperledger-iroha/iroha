#[allow(unused_imports)]
use std::fmt::Write as _;
use std::{
    error::Error,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use blake3::Hash;
use hex::encode as hex_encode;
use norito::json::{self, JsonDeserialize, Map, Value};
use sha2::{Digest, Sha256};

const RUNBOOK_PATH: &str = "docs/source/runbooks/address_manifest_ops.md";

#[derive(Clone)]
pub struct VerifyOptions {
    pub bundle: PathBuf,
    pub previous_bundle: Option<PathBuf>,
}

#[derive(JsonDeserialize)]
struct ManifestFile {
    version: u32,
    sequence: u64,
    generated_ms: u64,
    ttl_hours: u32,
    previous_digest: String,
    #[norito(default)]
    entries: Vec<Value>,
}

pub fn verify_manifest_bundle(opts: &VerifyOptions) -> Result<(), Box<dyn Error>> {
    let manifest_path = opts.bundle.join("manifest.json");
    let manifest_bytes = read_file_bytes(&manifest_path)?;
    let manifest: ManifestFile = parse_manifest(&manifest_path, &manifest_bytes)?;

    if manifest.version != 1 {
        return Err(format!(
            "address manifest {} uses unsupported version {}; update the tooling or regenerate the bundle",
            manifest_path.display(),
            manifest.version
        )
        .into());
    }
    if manifest.ttl_hours == 0 {
        return Err(format!(
            "address manifest {} declares ttl_hours=0; follow {} to publish a valid bundle",
            manifest_path.display(),
            RUNBOOK_PATH
        )
        .into());
    }
    if manifest.generated_ms == 0 {
        return Err(format!(
            "address manifest {} declares generated_ms=0; follow {} to publish a valid bundle",
            manifest_path.display(),
            RUNBOOK_PATH
        )
        .into());
    }

    validate_entries(&manifest.entries)?;
    verify_checksums(&opts.bundle)?;

    if let Some(previous_dir) = &opts.previous_bundle {
        verify_history(&manifest, previous_dir)?;
    } else if !manifest.previous_digest.is_empty() {
        ensure_hex_string(
            &manifest.previous_digest,
            32,
            "manifest.previous_digest",
            None,
        )?;
    }

    println!(
        "address manifest {} verified (sequence {}, entries {}, ttl {}h)",
        manifest_path.display(),
        manifest.sequence,
        manifest.entries.len(),
        manifest.ttl_hours
    );
    Ok(())
}

fn read_file_bytes(path: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    fs::read(path).map_err(|err| format!("failed to read {}: {err}", path.display()).into())
}

fn parse_manifest(path: &Path, bytes: &[u8]) -> Result<ManifestFile, Box<dyn Error>> {
    json::from_slice(bytes)
        .map_err(|err| format!("failed to parse manifest {}: {err}", path.display()).into())
}

fn verify_history(manifest: &ManifestFile, previous_dir: &Path) -> Result<(), Box<dyn Error>> {
    let prev_manifest_path = previous_dir.join("manifest.json");
    let prev_bytes = read_file_bytes(&prev_manifest_path)?;
    let previous: ManifestFile = parse_manifest(&prev_manifest_path, &prev_bytes)?;
    if manifest.sequence != previous.sequence + 1 {
        return Err(format!(
            "manifest {} sequence {} does not follow previous {} ({}); regenerate the bundle per {}",
            prev_manifest_path.display(),
            manifest.sequence,
            previous.sequence,
            previous_dir.display(),
            RUNBOOK_PATH
        )
        .into());
    }

    let expected_digest = blake3::hash(&prev_bytes);
    ensure_digest_matches(
        &manifest.previous_digest,
        expected_digest,
        &prev_manifest_path,
    )?;
    Ok(())
}

fn ensure_digest_matches(
    recorded: &str,
    expected: Hash,
    previous_manifest_path: &Path,
) -> Result<(), Box<dyn Error>> {
    let expected_hex = expected.to_hex().to_string();
    if recorded.to_ascii_lowercase() != expected_hex {
        return Err(format!(
            "manifest previous_digest `{}` does not match BLAKE3({}); expected {}. Rebuild the bundle per {}",
            recorded,
            previous_manifest_path.display(),
            expected_hex,
            RUNBOOK_PATH
        )
        .into());
    }
    Ok(())
}

fn verify_checksums(bundle: &Path) -> Result<(), Box<dyn Error>> {
    let checksums_path = bundle.join("checksums.sha256");
    let body = fs::read_to_string(&checksums_path).map_err(|err| {
        format!(
            "failed to read checksum file {}: {err}",
            checksums_path.display()
        )
    })?;

    let mut entries = Vec::new();
    for (line_number, raw_line) in body.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let Some(hash) = parts.next() else {
            continue;
        };
        let Some(target) = parts.next() else {
            return Err(format!(
                "invalid checksum entry on line {} in {}; expected `<sha256>  <file>`",
                line_number + 1,
                checksums_path.display()
            )
            .into());
        };
        entries.push((
            hash.to_ascii_lowercase(),
            target.to_string(),
            line_number + 1,
        ));
    }

    if entries.is_empty() {
        return Err(format!(
            "checksum file {} is empty; regenerate the bundle before publishing",
            checksums_path.display()
        )
        .into());
    }

    for (expected, relative_path, line_number) in entries {
        let file_path = bundle.join(&relative_path);
        let actual = sha256_hex(&file_path)?;
        if actual != expected {
            return Err(format!(
                "checksum mismatch for {} (line {} in {}): expected {}, computed {}",
                file_path.display(),
                line_number,
                checksums_path.display(),
                expected,
                actual
            )
            .into());
        }
    }

    Ok(())
}

fn sha256_hex(path: &Path) -> Result<String, Box<dyn Error>> {
    let mut file =
        File::open(path).map_err(|err| format!("failed to open {}: {err}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_encode(hasher.finalize()))
}

fn validate_entries(entries: &[Value]) -> Result<(), Box<dyn Error>> {
    for (index, entry) in entries.iter().enumerate() {
        let Some(obj) = entry.as_object() else {
            return Err(format!(
                "manifest entry #{entry_number} is not an object; follow {}",
                RUNBOOK_PATH,
                entry_number = index + 1
            )
            .into());
        };

        let entry_type = expect_string(obj, "type", index + 1)?;
        match entry_type {
            "global_domain" => validate_global_entry(obj, index + 1)?,
            "local_alias" => validate_local_alias(obj, index + 1)?,
            "tombstone" => validate_tombstone(obj, index + 1)?,
            other => {
                return Err(format!(
                    "manifest entry #{entry_number} has unsupported type `{other}`; manifests must use global_domain, local_alias, or tombstone per {}",
                    RUNBOOK_PATH,
                    entry_number = index + 1
                )
                .into());
            }
        }
    }
    Ok(())
}

fn validate_global_entry(entry: &Map, entry_number: usize) -> Result<(), Box<dyn Error>> {
    expect_string(entry, "domain", entry_number)?;
    expect_string(entry, "chain", entry_number)?;
    let selector = entry.get("selector").ok_or_else(|| {
        format!(
            "manifest entry #{entry_number} global_domain is missing `selector`; see {}",
            RUNBOOK_PATH
        )
    })?;
    if selector.as_str().is_none() && selector.as_object().is_none() {
        return Err(format!(
            "manifest entry #{entry_number} selector must be a string literal or object"
        )
        .into());
    }
    Ok(())
}

fn validate_local_alias(entry: &Map, entry_number: usize) -> Result<(), Box<dyn Error>> {
    expect_string(entry, "domain", entry_number)?;
    let selector = expect_selector(entry, entry_number)?;
    let kind = expect_string(selector, "kind", entry_number)?;
    if kind != "local" {
        return Err(format!(
            "manifest entry #{entry_number} local_alias selector.kind must be `local`, found `{kind}`"
        )
        .into());
    }
    let digest = expect_string(selector, "digest_hex", entry_number)?;
    ensure_hex_string(digest, 12, "selector.digest_hex", Some(entry_number))?;
    Ok(())
}

fn validate_tombstone(entry: &Map, entry_number: usize) -> Result<(), Box<dyn Error>> {
    let selector = expect_selector(entry, entry_number)?;
    let kind = expect_string(selector, "kind", entry_number)?;
    if kind != "local" {
        return Err(format!(
            "manifest entry #{entry_number} tombstone selector.kind must be `local`, found `{kind}`"
        )
        .into());
    }
    let digest = expect_string(selector, "digest_hex", entry_number)?;
    ensure_hex_string(digest, 12, "selector.digest_hex", Some(entry_number))?;

    let reason = expect_string(entry, "reason_code", entry_number)?;
    if reason.is_empty() {
        return Err(format!(
            "manifest entry #{entry_number} tombstone reason_code must be non-empty"
        )
        .into());
    }
    let ticket = expect_string(entry, "ticket", entry_number)?;
    if ticket.is_empty() {
        return Err(
            format!("manifest entry #{entry_number} tombstone ticket must be non-empty").into(),
        );
    }
    entry
        .get("replaces_sequence")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            format!(
                "manifest entry #{entry_number} tombstone must include numeric replaces_sequence"
            )
        })?;

    Ok(())
}

fn expect_string<'a>(
    entry: &'a Map,
    key: &str,
    entry_number: usize,
) -> Result<&'a str, Box<dyn Error>> {
    entry.get(key).and_then(Value::as_str).ok_or_else(|| {
        format!(
            "manifest entry #{entry_number} missing `{key}` string; see {}",
            RUNBOOK_PATH
        )
        .into()
    })
}

fn expect_selector(entry: &Map, entry_number: usize) -> Result<&Map, Box<dyn Error>> {
    entry
        .get("selector")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!("manifest entry #{entry_number} must include selector object").into()
        })
}

fn ensure_hex_string(
    value: &str,
    expected_bytes: usize,
    label: &str,
    entry_number: Option<usize>,
) -> Result<(), Box<dyn Error>> {
    if value.len() != expected_bytes * 2 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        let prefix = entry_number
            .map(|idx| format!("manifest entry #{idx} "))
            .unwrap_or_default();
        return Err(format!(
            "{}{} must be {} bytes of hex ({} characters); found `{value}`",
            prefix,
            label,
            expected_bytes,
            expected_bytes * 2
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn detects_sequence_gap() {
        let previous = tempdir().unwrap();
        write_manifest(&previous, 10, "").unwrap();

        let current = tempdir().unwrap();
        let prev_digest = digest_path(&previous.path().join("manifest.json"));
        write_manifest(&current, 12, &prev_digest).unwrap();

        let result = verify_manifest_bundle(&VerifyOptions {
            bundle: current.path().to_path_buf(),
            previous_bundle: Some(previous.path().to_path_buf()),
        });
        assert!(result.is_err(), "expected sequence gap error");
    }

    #[test]
    fn detects_digest_mismatch() {
        let previous = tempdir().unwrap();
        write_manifest(&previous, 3, "").unwrap();

        let current = tempdir().unwrap();
        write_manifest(&current, 4, "deadbeef").unwrap();

        let err = verify_manifest_bundle(&VerifyOptions {
            bundle: current.path().to_path_buf(),
            previous_bundle: Some(previous.path().to_path_buf()),
        })
        .unwrap_err();
        assert!(
            err.to_string().contains("previous_digest"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn detects_checksum_regression() {
        let previous = tempdir().unwrap();
        write_manifest(&previous, 1, "").unwrap();
        let prev_digest = digest_path(&previous.path().join("manifest.json"));

        let current = tempdir().unwrap();
        write_manifest(&current, 2, &prev_digest).unwrap();

        // Tamper with manifest after checksums were written.
        let manifest_path = current.path().join("manifest.json");
        fs::write(&manifest_path, br#"{"version":1,"sequence":2,"generated_ms":5,"ttl_hours":24,"previous_digest":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","entries":[{"type":"local_alias","domain":"wonderland","selector":{"kind":"local","digest_hex":"0123456789abcdef01234567"}}]}"#).unwrap();

        let err = verify_manifest_bundle(&VerifyOptions {
            bundle: current.path().to_path_buf(),
            previous_bundle: Some(previous.path().to_path_buf()),
        })
        .unwrap_err();
        assert!(
            err.to_string().contains("checksum mismatch"),
            "unexpected error: {err}"
        );
    }

    fn write_manifest(
        dir: &tempfile::TempDir,
        sequence: u64,
        previous_digest: &str,
    ) -> Result<(), Box<dyn Error>> {
        write_manifest_inner(dir.path(), sequence, previous_digest)
    }

    fn write_manifest_inner(
        dir: &Path,
        sequence: u64,
        previous_digest: &str,
    ) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(dir)?;
        let manifest_body = format!(
            r#"{{
    "version": 1,
    "sequence": {sequence},
    "generated_ms": {timestamp},
    "ttl_hours": 24,
    "previous_digest": "{previous_digest}",
    "entries": [
        {{
            "type": "local_alias",
            "domain": "wonderland",
            "selector": {{ "kind": "local", "digest_hex": "0123456789abcdef01234567" }}
        }}
    ]
}}"#,
            timestamp = 1_726_000_000_000_u64
        );
        fs::write(dir.join("manifest.json"), manifest_body)?;
        fs::write(dir.join("manifest.sigstore"), b"stub-signature")?;
        fs::write(dir.join("notes.md"), b"changelog")?;
        write_checksums(dir)?;
        Ok(())
    }

    fn write_checksums(dir: &Path) -> Result<(), Box<dyn Error>> {
        let files = ["manifest.json", "manifest.sigstore", "notes.md"];
        let mut body = String::new();
        for name in &files {
            let hash = sha256_hex(&dir.join(name))?;
            writeln!(&mut body, "{hash}  {name}")?;
        }
        fs::write(dir.join("checksums.sha256"), body)?;
        Ok(())
    }

    fn digest_path(path: &Path) -> String {
        let bytes = fs::read(path).unwrap();
        blake3::hash(&bytes).to_hex().to_string()
    }
}
