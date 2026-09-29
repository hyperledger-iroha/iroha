//! Generate canonical IVM replay binding keys and a public CLI registry submission.

use base64::Engine as _;
use clap::Parser;
use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

#[path = "../zk_registry_json.rs"]
mod registry_json;
use registry_json::VkSubmissionJson;

#[derive(Debug, Parser)]
#[command(about = "Generate canonical IVM replay binding registry artifacts")]
struct Args {
    /// Write the canonical verifying-key container to a new file.
    #[arg(long, value_name = "PATH")]
    vk_out: PathBuf,
    /// Write public JSON accepted by `iroha app zk vk register --json` to a new file.
    #[arg(long, value_name = "PATH")]
    template_out: PathBuf,
    /// Export an optional offline proving-key archive; Torii derives its own key.
    #[arg(long, value_name = "PATH")]
    pk_out: Option<PathBuf>,
    /// Registry name. Signing credentials come from the CLI client configuration.
    #[arg(long, default_value = "ivm_replay_binding", value_parser = registry_name)]
    name: String,
}

fn registry_name(value: &str) -> Result<String, String> {
    if !iroha_data_model::proof::verifying_key_id_field_is_portable(value) {
        return Err(format!(
            "name must use portable verifier-key registry syntax (1..={} bytes)",
            iroha_data_model::proof::VERIFYING_KEY_ID_MAX_FIELD_BYTES
        ));
    }
    Ok(value.to_owned())
}

/// Resolve output parents before key generation; refuse aliases and overwrites.
fn prepare_output_paths(args: &Args) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::with_capacity(3);
    for (flag, path) in [
        ("--vk-out", Some(args.vk_out.as_path())),
        ("--template-out", Some(args.template_out.as_path())),
        ("--pk-out", args.pk_out.as_deref()),
    ] {
        let Some(path) = path else { continue };
        let name = path
            .file_name()
            .ok_or_else(|| format!("{flag} must name a file: {}", path.display()))?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|error| {
            format!("cannot create {flag} parent {}: {error}", parent.display())
        })?;
        let resolved = fs::canonicalize(parent)
            .map_err(|error| format!("cannot resolve {flag} parent {}: {error}", parent.display()))?
            .join(name);
        if paths.contains(&resolved) {
            return Err(format!(
                "output paths must be distinct: {}",
                resolved.display()
            ));
        }
        match fs::symlink_metadata(&resolved) {
            Ok(_) => {
                return Err(format!(
                    "{flag} output already exists: {}",
                    resolved.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "cannot inspect {flag} output {}: {error}",
                    resolved.display()
                ));
            }
        }
        paths.push(resolved);
    }
    Ok(paths)
}

struct Artifacts {
    vk: Vec<u8>,
    template: String,
    pk: Option<Vec<u8>>,
}

fn generate_artifacts(name: &str, include_pk: bool) -> Result<Artifacts, String> {
    let name = registry_name(name)?;
    let mut record = iroha_core::zk::halo2_ipa_ivm_replay_binding_vk_record("core", 1)
        .map_err(|error| format!("cannot build canonical replay binding VK record: {error}"))?;
    let vk = record
        .key
        .take()
        .ok_or("canonical VK record has no inline key")?;
    let pk = if include_pk {
        Some(
            iroha_core::zk::derive_halo2_ipa_ivm_replay_binding_proving_key_bytes(&vk)
                .map_err(|error| format!("cannot derive optional proving-key archive: {error}"))?,
        )
    } else {
        None
    };
    let submission = VkSubmissionJson {
        backend: vk.backend,
        name,
        version: record.version,
        circuit_id: record.circuit_id,
        public_inputs_schema_hash_hex: hex::encode(record.public_inputs_schema_hash),
        curve: Some(record.curve),
        gas_schedule_id: record.gas_schedule_id,
        vk_len: Some(record.vk_len),
        max_proof_bytes: Some(record.max_proof_bytes),
        metadata_uri_cid: record.metadata_uri_cid,
        vk_bytes_cid: record.vk_bytes_cid,
        activation_height: record.activation_height,
        withdraw_height: record.withdraw_height,
        commitment_hex: Some(hex::encode(record.commitment)),
        vk_bytes: Some(base64::engine::general_purpose::STANDARD.encode(&vk.bytes)),
        status: Some(record.status),
        namespace: Some(record.namespace),
    };
    let mut template = norito::json::to_json_pretty(&submission)
        .map_err(|error| format!("cannot serialize public registry submission: {error}"))?;
    template.push('\n');
    Ok(Artifacts {
        vk: vk.bytes,
        template,
        pk,
    })
}

fn write_artifact(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("cannot create output {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("cannot write output {}: {error}", path.display()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let paths = prepare_output_paths(&args)?;
    let artifacts = generate_artifacts(&args.name, args.pk_out.is_some())?;
    write_artifact(&paths[0], &artifacts.vk)?;
    write_artifact(&paths[1], artifacts.template.as_bytes())?;
    println!(
        "wrote vk={} template={}",
        paths[0].display(),
        paths[1].display()
    );
    if let Some(pk) = artifacts.pk {
        write_artifact(&paths[2], &pk)?;
        println!(
            "wrote optional pk={} ({} bytes)",
            paths[2].display(),
            pk.len()
        );
    }
    println!("vk_len={}", artifacts.vk.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_require_public_outputs_and_make_pk_optional() {
        let args =
            Args::try_parse_from(["keygen", "--vk-out", "key.vk", "--template-out", "key.json"])
                .expect("public outputs suffice");
        assert!(args.pk_out.is_none());
        assert_eq!(args.name, "ivm_replay_binding");
        let explicit = Args::try_parse_from([
            "keygen",
            "--vk-out",
            "key.vk",
            "--template-out",
            "key.json",
            "--pk-out",
            "key.pk",
        ])
        .expect("optional export");
        assert_eq!(explicit.pk_out, Some(PathBuf::from("key.pk")));
        for tail in [
            vec!["--unknown", "value"],
            vec!["--name"],
            vec!["--vk-out", "again"],
            vec!["--name", " "],
            vec!["--name", " key"],
        ] {
            let mut values = vec!["keygen", "--vk-out", "key.vk", "--template-out", "key.json"];
            values.extend(tail);
            assert!(Args::try_parse_from(values).is_err());
        }
        assert!(Args::try_parse_from(["keygen", "--vk-out", "key.vk"]).is_err());
        for name in [
            "quoted\"name".to_owned(),
            "back\\slash".to_owned(),
            "a".repeat(iroha_data_model::proof::VERIFYING_KEY_ID_MAX_FIELD_BYTES + 1),
        ] {
            assert!(
                Args::try_parse_from([
                    "keygen",
                    "--vk-out",
                    "key.vk",
                    "--template-out",
                    "key.json",
                    "--name",
                    &name
                ])
                .is_err()
            );
            assert!(generate_artifacts(&name, false).is_err());
        }
    }

    #[test]
    fn outputs_reject_existing_aliasing_and_invalid_parent_paths() {
        let dir = tempfile::tempdir().expect("output directory");
        let mut args = Args {
            vk_out: dir.path().join("key.vk"),
            template_out: dir.path().join("./key.vk"),
            pk_out: None,
            name: "key".into(),
        };
        assert!(
            prepare_output_paths(&args)
                .expect_err("same output")
                .contains("distinct")
        );
        args.template_out = dir.path().join("key.json");
        let paths = prepare_output_paths(&args).expect("new distinct outputs");
        write_artifact(&paths[0], b"preserve existing").expect("write public fixture");
        assert!(
            prepare_output_paths(&args)
                .expect_err("existing output")
                .contains("already exists")
        );
        assert!(write_artifact(&paths[0], b"replacement").is_err());
        assert_eq!(
            fs::read(&paths[0]).expect("read existing"),
            b"preserve existing"
        );
        args.vk_out = paths[0].join("child");
        assert!(
            prepare_output_paths(&args)
                .expect_err("parent is file")
                .contains("parent")
        );
        args.vk_out = PathBuf::new();
        assert!(
            prepare_output_paths(&args)
                .expect_err("empty path")
                .contains("must name a file")
        );
    }

    #[test]
    fn registry_json_escapes_name_and_contains_only_canonical_public_fields() {
        let name = "canonical_key";
        let artifacts = generate_artifacts(name, false).expect("canonical public artifacts");
        assert!(artifacts.pk.is_none());
        let payload: VkSubmissionJson =
            norito::json::from_str(&artifacts.template).expect("same strict DTO as CLI");
        let expected = iroha_core::zk::halo2_ipa_ivm_replay_binding_vk_record("core", 1)
            .expect("canonical record");
        assert_eq!(payload.name, name);
        assert_eq!(payload.backend, iroha_core::zk::ZK_BACKEND_HALO2_IPA);
        assert_eq!(payload.circuit_id, expected.circuit_id);
        assert_eq!(
            payload.public_inputs_schema_hash_hex,
            hex::encode(expected.public_inputs_schema_hash)
        );
        assert_eq!(
            payload.commitment_hex,
            Some(hex::encode(expected.commitment))
        );
        assert_eq!(payload.vk_len, Some(expected.vk_len));
        assert_eq!(payload.max_proof_bytes, Some(expected.max_proof_bytes));
        assert_eq!(payload.curve.as_deref(), Some("pallas"));
        assert_eq!(payload.namespace.as_deref(), Some("core"));
        // Escaping is a serializer property; registry names are validated separately.
        let mut escaped = payload.clone();
        escaped.name = "quoted\"\\name".to_owned();
        let escaped_json = norito::json::to_json(&escaped).expect("escaped DTO");
        let decoded: VkSubmissionJson =
            norito::json::from_str(&escaped_json).expect("escaped DTO roundtrip");
        assert_eq!(decoded.name, escaped.name);
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(payload.vk_bytes.expect("inline VK"))
                .expect("base64"),
            artifacts.vk
        );
        let value: norito::json::Value =
            norito::json::from_str(&artifacts.template).expect("JSON object");
        for field in [
            "authority",
            "private_key",
            "public_inputs_schema_hex",
            "unexpected",
        ] {
            assert!(!value.as_object().expect("object").contains_key(field));
            let mut invalid = value.clone();
            invalid.as_object_mut().expect("object").insert(
                field.to_owned(),
                norito::json::Value::String("forbidden".into()),
            );
            assert!(
                norito::json::from_value::<VkSubmissionJson>(invalid).is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn optional_proving_key_matches_the_canonical_core_archive() {
        let artifacts = generate_artifacts("key", true).expect("explicit offline export");
        let vk = iroha_core::zk::halo2_ipa_ivm_replay_binding_vk_box().expect("canonical VK");
        assert_eq!(artifacts.vk, vk.bytes);
        assert_eq!(
            artifacts.pk.expect("requested archive"),
            iroha_core::zk::derive_halo2_ipa_ivm_replay_binding_proving_key_bytes(&vk)
                .expect("canonical archive")
        );
    }
}
