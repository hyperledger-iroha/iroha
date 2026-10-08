//! The genuine complete finality compiler and descriptor/VK-only wallet export.
//!
//! The server compiler's bounded regenerable PK working set stays in its own
//! directory. Export follows only the completed canonical catalog and rehashes
//! every descriptor/VK; it never discovers readiness by enumerating a directory.

use super::*;
use iroha_kagemusha_proof::finality::{
    catalog::{self, SourceProvenance, StreamingCatalog},
    native::ImportLimits,
};
use sha2::Sha256;

const FINALITY_KEY_MAX: usize = 256 << 20;
const FINALITY_WORKING_MAX: u64 = 16 << 30;
const FINALITY_LOGICAL_MAX: u64 = 512 << 30;

#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct FinalityRequest {
    schema: String,
    chain_id: String,
    network_id_hex: String,
    genesis_public_key: String,
    signed_genesis: OriginalInput,
    source_revision: String,
    source_manifest_sha256: String,
    output_parent: String,
    output_name: String,
    maximum_original_bytes: u64,
    working_proving_key_bytes: u64,
}

fn parse(bytes: &[u8]) -> io::Result<FinalityRequest> {
    let request: FinalityRequest = checked(
        json::from_slice(bytes),
        "closed finality request schema required",
    )?;
    if request.schema != "iroha.kagemusha.finality-artifact-production.v1"
        || request.chain_id.is_empty()
        || request.chain_id.len() > 256
        || request.source_revision.is_empty()
        || request.source_revision.len() > 4_096
        || request.output_name.is_empty()
        || request.output_name.len() > 128
        || !request
            .output_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || request.maximum_original_bytes < FINALITY_KEY_MAX as u64
        || request.maximum_original_bytes > FINALITY_LOGICAL_MAX
        || request.working_proving_key_bytes < FINALITY_KEY_MAX as u64
        || request.working_proving_key_bytes > FINALITY_WORKING_MAX
    {
        return Err(invalid(
            "unsupported finality production request or resource limit",
        ));
    }
    digest(&request.network_id_hex)?;
    digest(&request.signed_genesis.sha256)?;
    digest(&request.source_manifest_sha256)?;
    Ok(request)
}

fn export_verifiers(
    source: &PrivateDirectory,
    records: &[ArtifactRecord],
    output: &mut DirectoryOriginalsV1,
) -> io::Result<()> {
    for record in records {
        checked(
            record.validate_identity(),
            "finality export identity rejected",
        )?;
        let stem = hex::encode(Sha256::digest(&record.name));
        for (index, (suffix, cap)) in [
            ("descriptor", DESCRIPTOR_MAX_BYTES_V1),
            ("vk", VERIFYING_KEY_MAX_BYTES_V1),
        ]
        .into_iter()
        .enumerate()
        {
            let blob = BlobV1 {
                bytes: record.lengths[index],
                sha256: record.sha256[index],
            };
            let bytes = source.read(format!("{stem}.{suffix}"), cap)?;
            if BlobV1::of(&bytes) != blob {
                return Err(invalid(
                    "completed finality export differs from exact source original",
                ));
            }
            output.store_original(blob, &bytes)?;
        }
    }
    source.revalidate()?;
    Ok(())
}

pub fn run(path: &str) -> io::Result<()> {
    let request_original = Original::open(path, REQUEST_MAX, None)?;
    let request = parse(&request_original.bytes)?;
    let genesis = Original::open(
        &request.signed_genesis.path,
        GENESIS_MAX,
        Some(digest(&request.signed_genesis.sha256)?),
    )?;
    let native = native_finality(
        &request.chain_id,
        &request.network_id_hex,
        &request.genesis_public_key,
        &genesis.bytes,
    )?;
    let anchor = checked(
        derive_history_anchor(&native),
        "finality signed genesis anchor rejected",
    )?;
    let limits = ImportLimits {
        key: ReadConfig {
            maximum_bytes: FINALITY_KEY_MAX,
            maximum_rows: 1 << 16,
            coset_cache: CosetCachePolicy::OnDemand,
            msm_budget: MemoryBudget::DEFAULT,
        },
        maximum_artifacts: RECORDS_MAX,
        maximum_original_bytes: usize::try_from(request.maximum_original_bytes)
            .map_err(|_| invalid("finality logical bound unavailable on this host"))?,
    };
    let working = usize::try_from(request.working_proving_key_bytes)
        .map_err(|_| invalid("finality working bound unavailable on this host"))?;
    let provenance = SourceProvenance {
        revision: request.source_revision.clone(),
        source_manifest_sha256: digest(&request.source_manifest_sha256)?,
    };
    let parent = PrivateDirectory::open_exact(&request.output_parent)?;
    request_original.recheck()?;
    genesis.recheck()?;
    let output = parent.create_child(&request.output_name)?;
    publish(&output, "request.json", &request_original.bytes)?;
    publish(&output, "signed-genesis.norito", &genesis.bytes)?;
    publish(
        &output,
        "source-provenance.norito",
        &checked(
            norito::encode_canonical(&provenance),
            "source provenance encoding",
        )?,
    )?;
    let streaming_path = output.path().join("server-originals");
    let mut originals = checked(
        StreamingCatalog::create(&streaming_path, limits, working),
        "exclusive finality compiler storage",
    )?;
    let held_server = PrivateDirectory::open_exact(&streaming_path)?;
    eprintln!(
        "Compiling the complete ordinary-finality graph with bounded regenerable server PK residency."
    );
    let completed = catalog::compile(
        anchor,
        &mut originals,
        Parameters {
            pallas: checked(PinnedParams::derive(16), "Pallas parameters")?,
            vesta: checked(PinnedParams::derive(16), "Vesta parameters")?,
        },
        limits,
        provenance,
    )
    .map_err(io::Error::other)?;
    let inventory = checked(
        originals.inventory(),
        "completed finality inventory encoding",
    )?;
    let (records, _, _) = records(&inventory)?;
    request_original.recheck()?;
    genesis.recheck()?;
    held_server.revalidate()?;
    let export = output.create_child("finality-originals")?;
    let mut export = DirectoryOriginalsV1::open_existing(export.path(), DESCRIPTOR_MAX_BYTES_V1)?;
    export_verifiers(&held_server, &records, &mut export)?;
    publish(&output, "finality-inventory.norito", &inventory)?;
    let complete = norito::json!({
        "schema": "iroha.kagemusha.finality-artifact-production-result.v1",
        "signed_genesis_sha256": (request.signed_genesis.sha256),
        "source_revision": (completed.provenance.revision),
        "source_manifest_sha256": (hex::encode(completed.provenance.source_manifest_sha256)),
        "inventory_sha256": (hex::encode(BlobV1::of(&inventory).sha256)),
        "terminal_descriptor": (hex::encode(completed.terminal.descriptor)),
        "terminal_key": (hex::encode(completed.terminal.key)),
        "logical_original_bytes": (originals.original_bytes()),
        "resident_server_pk_bytes": (originals.resident_bytes()),
        "scope": "Complete fixed source graph and strict original import; no live finality proof, wallet installation or release admission."
    });
    publish(
        &output,
        "source-complete.json",
        checked(json::to_json(&complete), "completion encoding")?.as_bytes(),
    )?;
    request_original.recheck()?;
    genesis.recheck()?;
    held_server.revalidate()?;
    output.sync()?;
    println!(
        "Complete finality source graph and descriptor/VK export written to {}.",
        output.path().display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> String {
        format!(
            r#"{{"schema":"iroha.kagemusha.finality-artifact-production.v1","chain_id":"test-only","network_id_hex":"{}","genesis_public_key":"unused-before-input-intake","signed_genesis":{{"path":"/private/absent/genesis","sha256":"{}"}},"source_revision":"unit-test-source-only","source_manifest_sha256":"{}","output_parent":"/private/absent/output","output_name":"test-only-output","maximum_original_bytes":549755813888,"working_proving_key_bytes":536870912}}"#,
            "01".repeat(32),
            "02".repeat(32),
            "03".repeat(32)
        )
    }

    #[test]
    fn source_request_refuses_unbounded_memory_unknown_fields_and_absent_genesis() {
        let document = request();
        assert!(parse(document.as_bytes()).is_ok());
        assert!(parse(document.replace("536870912", "1").as_bytes()).is_err());
        assert!(parse(document.replace("549755813888", "549755813889").as_bytes()).is_err());
        assert!(parse(document.replacen('{', "{\"ready\":true,", 1).as_bytes()).is_err());
        let temp = tempfile::tempdir().unwrap();
        let root =
            PrivateDirectory::open_or_create(temp.path().canonicalize().unwrap().join("inputs"))
                .unwrap();
        publish(&root, "request.json", document.as_bytes()).unwrap();
        assert!(run(root.path().join("request.json").to_str().unwrap()).is_err());
        assert_eq!(root.entries(10).unwrap().len(), 1);
    }

    #[test]
    fn export_rejects_invalid_record_identity_before_reading_any_payload() {
        let temp = tempfile::tempdir().unwrap();
        let root =
            PrivateDirectory::open_or_create(temp.path().canonicalize().unwrap().join("sources"))
                .unwrap();
        let out = root.create_child("output").unwrap();
        let mut out =
            DirectoryOriginalsV1::open_existing(out.path(), DESCRIPTOR_MAX_BYTES_V1).unwrap();
        let record = ArtifactRecord {
            name: b"not a canonical source identity".to_vec(),
            lengths: [1, 1, 1],
            sha256: [[1; 32]; 3],
        };
        assert!(export_verifiers(&root, &[record], &mut out).is_err());
        assert_eq!(
            root.open_child("output")
                .unwrap()
                .entries(10)
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn maintained_catalog_export_preserves_actual_metadata_bytes_without_server_pk() {
        use iroha_kagemusha_proof::finality::{
            catalog::{ArtifactSink, DirectoryCatalog},
            continuity::tree::OriginalBytes,
            native::{ArtifactId, NodeId},
        };
        let temp = tempfile::tempdir().unwrap();
        let root = PrivateDirectory::open_or_create(
            temp.path()
                .canonicalize()
                .unwrap()
                .join("metadata-only-test"),
        )
        .unwrap();
        let limits = ImportLimits {
            key: ReadConfig {
                maximum_bytes: 1_024,
                maximum_rows: 1 << 16,
                coset_cache: CosetCachePolicy::OnDemand,
                msm_budget: MemoryBudget::DEFAULT,
            },
            maximum_artifacts: 4,
            maximum_original_bytes: 16_384,
        };
        let path = root.path().join("source");
        let mut catalog = DirectoryCatalog::create(&path, limits).unwrap();
        // Explicit opaque metadata DATA for export/custody, not compiled keys or proof authority.
        let original = OriginalBytes {
            descriptor: b"test descriptor DATA".to_vec(),
            verifying_key: b"test verifier DATA".to_vec(),
            proving_key: b"test server PK DATA stays behind".to_vec(),
        };
        catalog
            .store(&ArtifactId::Source(NodeId::Genesis), &original)
            .unwrap();
        let inventory = catalog.inventory().unwrap();
        let (selected, _, _) = records(&inventory).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            for entry in std::fs::read_dir(&path).unwrap() {
                std::fs::set_permissions(
                    entry.unwrap().path(),
                    std::fs::Permissions::from_mode(0o600),
                )
                .unwrap();
            }
        }
        let held = PrivateDirectory::open_exact(&path).unwrap();
        let output = root.create_child("export").unwrap();
        let mut output =
            DirectoryOriginalsV1::open_existing(output.path(), DESCRIPTOR_MAX_BYTES_V1).unwrap();
        export_verifiers(&held, &selected, &mut output).unwrap();
        output
            .verify_original(BlobV1::of(&original.descriptor))
            .unwrap();
        output
            .verify_original(BlobV1::of(&original.verifying_key))
            .unwrap();
        assert!(
            output
                .open_original(BlobV1::of(&original.proving_key).sha256)
                .is_err()
        );
        assert_eq!(
            root.open_child("export")
                .unwrap()
                .entries(10)
                .unwrap()
                .len(),
            2
        );
        let mut bad = selected.clone();
        bad[0].lengths[0] += 1;
        assert!(export_verifiers(&held, &bad, &mut output).is_err());
        let mut duplicate = selected.clone();
        duplicate.push(selected[0].clone());
        assert!(records(&norito::encode_canonical(&duplicate).unwrap()).is_err());
        bad[0].lengths[0] = DESCRIPTOR_MAX_BYTES_V1 as u64 + 1;
        assert!(records(&norito::encode_canonical(&bad).unwrap()).is_err());
    }
}
