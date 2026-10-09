//! Opt-in selective preparation of the exact Result1 original pair, never proofs.

use super::*;
use sha2::{Digest as _, Sha256};
use std::{
    fs::OpenOptions,
    io::{Read as _, Write as _},
    path::Path,
};

const INVENTORY_SHA: &str = "ad8d0d299181a3033807e8c845bff60735d0ea07439ac41b2495892483c38397";
const PREFIX: &str = "KAGEMUSHA_RESULT1_PREP_";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn path(name: &str) -> PathBuf {
    std::env::var_os(format!("{PREFIX}{name}"))
        .map(PathBuf::from)
        .expect("explicit fixture path")
}
fn bounded(path: &Path, maximum: usize) -> Vec<u8> {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(metadata.file_type().is_file());
    let length = usize::try_from(metadata.len()).unwrap();
    assert!(length > 0 && length <= maximum);
    let file = fs::File::open(path).unwrap();
    assert_eq!(file.metadata().unwrap().len(), metadata.len());
    let mut bytes = Vec::with_capacity(length);
    file.take(length as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes.len(), length);
    bytes
}
fn retain(root: &Path, name: &str, bytes: &[u8]) {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(root.join(name)).unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}
fn same_bytes(record: &ArtifactRecord, bytes: &OriginalBytes) -> Result<(), Error> {
    for (index, value) in [&bytes.descriptor, &bytes.verifying_key, &bytes.proving_key]
        .into_iter()
        .enumerate()
    {
        if value.len() as u64 != record.lengths[index]
            || <[u8; 32]>::from(Sha256::digest(value)) != record.sha256[index]
        {
            return Err(Error::Artifact);
        }
    }
    Ok(())
}
struct PinnedSink {
    originals: DirectoryCatalog,
    expected: Vec<ArtifactRecord>,
    stores: Vec<ArtifactId>,
}
impl ArtifactSource for PinnedSink {
    fn load(&mut self, id: &ArtifactId) -> Result<OriginalBytes, Error> {
        self.originals.load(id)
    }
}
impl ArtifactSink for PinnedSink {
    fn store(&mut self, id: &ArtifactId, bytes: &OriginalBytes) -> Result<(), Error> {
        if self.stores.contains(id) {
            return Err(Error::Artifact);
        }
        let mut matching = None;
        for record in &self.expected {
            if record.matches_identity(id)? && matching.replace(record).is_some() {
                return Err(Error::Artifact);
            }
        }
        same_bytes(matching.ok_or(Error::Artifact)?, bytes)?;
        self.originals.store(id, bytes)?;
        self.stores.push(id.clone());
        Ok(())
    }
}

#[test]
fn selected_original_comparison_binds_every_role_length_and_byte() {
    let bytes = sample();
    let values = [&bytes.descriptor, &bytes.verifying_key, &bytes.proving_key];
    // This checks only the byte-comparison helper. No fake source import occurs.
    let record = ArtifactRecord {
        name: Vec::new(),
        lengths: values.map(|b| b.len() as u64),
        sha256: values.map(|b| Sha256::digest(b).into()),
    };
    same_bytes(&record, &bytes).unwrap();
    for index in 0..3 {
        let mut wrong = record.clone();
        wrong.lengths[index] += 1;
        assert_eq!(same_bytes(&wrong, &bytes), Err(Error::Artifact));
        let mut wrong = record.clone();
        wrong.sha256[index][0] ^= 1;
        assert_eq!(same_bytes(&wrong, &bytes), Err(Error::Artifact));
    }
}

#[test]
#[ignore = "selective actual Result1 source/wrapper key compilation and strict import, zero proofs; independent original inventory and fresh output required"]
fn prepare_exact_result1_originals_without_full_catalog_or_proofs() {
    let inventory_path = path("INVENTORY");
    let inventory = bounded(&inventory_path, 2 << 20);
    assert_eq!(hex(&Sha256::digest(&inventory)), INVENTORY_SHA);
    let provenance = bounded(&path("PROVENANCE"), 64 << 10);
    assert_eq!(
        hex(&Sha256::digest(&provenance)),
        std::env::var(format!("{PREFIX}PROVENANCE_SHA256"))
            .expect("caller independently selected capture provenance pin")
    );
    let records: Vec<ArtifactRecord> = norito::decode_canonical_with_limits(
        &inventory,
        norito::canonical_decode_limits(inventory.len()),
    )
    .unwrap();
    assert_eq!(records.len(), 2495);
    let node = NodeId::Leaf(Program::Result, 1);
    let mut expected = Vec::new();
    for id in [
        ArtifactId::Source(node.clone()),
        ArtifactId::Wrapper(node.clone()),
    ] {
        let matches: Vec<_> = records
            .iter()
            .filter(|record| record.matches_identity(&id).unwrap())
            .collect();
        assert_eq!(matches.len(), 1);
        expected.push((*matches[0]).clone());
    }
    expected.sort_by(|left, right| left.name.cmp(&right.name));
    assert_eq!(
        expected.iter().map(|record| record.lengths[2]).sum::<u64>(),
        283_357_580
    );
    let output = path("OUTPUT");
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder
        .create(&output)
        .expect("fresh fixture output; never a live cache");
    retain(&output, "inventory.norito", &inventory);
    retain(&output, "caller-provenance.json", &provenance);
    retain(
        &output,
        "expected-records.norito",
        &norito::to_bytes(&expected).unwrap(),
    );
    let read = ImportLimits {
        key: ReadConfig {
            maximum_bytes: 256 << 20,
            ..limits().key
        },
        maximum_artifacts: 2,
        maximum_original_bytes: 512 << 20,
    };
    let mut sink = PinnedSink {
        originals: DirectoryCatalog::create(output.join("originals"), read).unwrap(),
        expected: expected.clone(),
        stores: Vec::new(),
    };
    let params = parameters();
    let (class, layout) = crate::finality::native::source_layout::result(1).unwrap();
    assert_eq!(class, 1);
    eprintln!("RESULT1_FIXTURE selective_compile=true source_classes=1 original_pairs=1 proofs=0");
    // Exactly one existing sealed source owner. Compiler::source already strictly
    // reimports the emitted source and wrapper; no whole graph is assembled.
    let qualified = compiler(&mut sink, params, read)
        .source(node.clone(), &layout)
        .unwrap();
    assert_eq!(
        sink.stores,
        [ArtifactId::Source(node.clone()), ArtifactId::Wrapper(node)]
    );
    assert_eq!(
        sink.originals.inventory().unwrap(),
        norito::to_bytes(&expected).unwrap()
    );
    retain(
        &output,
        "selected-records.norito",
        &sink.originals.inventory().unwrap(),
    );
    assert_eq!(bounded(&inventory_path, inventory.len()), inventory);
    let identity = identity(&qualified).unwrap();
    retain(&output, "result.json", &norito::json::to_vec(&norito::json!({
        "schema": "kagemusha.result1.selective-original-preparation.v1", "actual_source_classes": 1,
        "actual_original_pairs": 1, "actual_proofs": 0, "pk_bytes": 283357580,
        "inventory_sha256": INVENTORY_SHA, "caller_provenance_sha256": (hex(&Sha256::digest(&provenance))),
        "qualified_wrapper_descriptor": (hex(&identity.descriptor)), "qualified_wrapper_key": (hex(&identity.key)),
        "all_six_originals_match_independent_inventory": true, "strict_compiled_source_import": true,
        "scope": "offline selective fixture only; caller provenance needs independent source/binary admission; no whole catalog, native history, monetary or release qualification"
    })).unwrap());
    eprintln!("RESULT1_FIXTURE complete=true exact_six_originals=true strict_import=true proofs=0");
}
