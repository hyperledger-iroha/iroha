//! Explicit full-route source construction from a pinned complete finality metadata snapshot.
//! This diagnostic emits unsigned originals only. It proves no receipt or payment,
//! signs no artifact identity, and cannot manufacture a qualified wallet capability.

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use iroha_data_model::{
    block::decode_framed_signed_block,
    sumeragi_finality::{FinalityValidator, SumeragiFinalityVerifier, genesis_epoch},
};
use iroha_kagemusha_proof::finality::{
    catalog::{
        ArtifactRecord, SourceProvenance, VerifierBlobSource, VerifierLimits, qualify_receipt,
    },
    continuity::producer::Error as FinalityError,
    native::Parameters,
};
use iroha_pasta::msm::MemoryBudget;
use iroha_plonk_gadgets::p256::native::{Affine, words_from_be};
use p256::ecdsa::SigningKey;

use super::*;

const RECORDS: usize = 4_096;
const INVENTORY_BYTES: usize = RECORDS * 2_048 + 4_096;
const VERIFIER_BYTES: usize = 64 << 20;
const OUTPUT_BYTES: u64 = 128 << 30;
const FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/kagemusha/ordinary_first_load_receipt_v1.json"
));

#[derive(Clone, Copy)]
struct Pins {
    producer: [u8; 32],
    sources: [u8; 32],
    fixture: [u8; 32],
    inventory: [u8; 32],
}
fn pin(name: &str) -> [u8; 32] {
    let encoded = std::env::var(name).expect("independently recorded exact SHA-256 pin");
    let bytes: [u8; 32] = hex::decode(encoded).unwrap().try_into().unwrap();
    assert_ne!(bytes, [0; 32]);
    bytes
}
fn regular_directory(path: &Path) -> Result<(), Error> {
    if !fs::symlink_metadata(path)
        .map_err(|_| Error::Inventory)?
        .file_type()
        .is_dir()
    {
        return Err(Error::Inventory);
    }
    Ok(())
}
fn bounded_file(path: &Path, cap: usize) -> Result<Vec<u8>, Error> {
    let metadata = fs::symlink_metadata(path).map_err(|_| Error::Inventory)?;
    let length = usize::try_from(metadata.len()).map_err(|_| Error::Inventory)?;
    if !metadata.file_type().is_file() || length == 0 || length > cap {
        return Err(Error::Inventory);
    }
    let file = File::open(path).map_err(|_| Error::Inventory)?;
    if file.metadata().map_err(|_| Error::Inventory)?.len() != metadata.len() {
        return Err(Error::Inventory);
    }
    let mut bytes = Vec::with_capacity(length);
    file.take(u64::try_from(cap).map_err(|_| Error::Inventory)? + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Inventory)?;
    if bytes.len() != length {
        return Err(Error::Inventory);
    }
    Ok(bytes)
}
fn pinned_file(path: &Path, cap: usize, expected: [u8; 32]) -> Result<Vec<u8>, Error> {
    if expected == [0; 32] {
        return Err(Error::Inventory);
    }
    let bytes = bounded_file(path, cap)?;
    if BlobV1::of(&bytes).sha256 != expected {
        return Err(Error::Inventory);
    }
    Ok(bytes)
}
fn records(root: &Path, pins: Pins) -> Result<Vec<ArtifactRecord>, Error> {
    regular_directory(root)?;
    // Only a separate immutable metadata snapshot is accepted; never reopen the
    // live streaming catalog, its lock or its regenerable server proving keys.
    if bounded_file(&root.join("snapshot-kind"), 128)?
        != b"KAGEMUSHA completed finality metadata snapshot v1\n"
        || bounded_file(&root.join("binary.sha256"), 32)? != pins.producer
        || pins.producer == [0; 32]
        || pins.sources == [0; 32]
        || pinned_file(&root.join("fixture.json"), 1 << 20, pins.fixture)? != FIXTURE
    {
        return Err(Error::Inventory);
    }
    let provenance = bounded_file(&root.join("provenance.norito"), 8_192)?;
    let provenance: SourceProvenance = norito::decode_canonical_with_limits(
        &provenance,
        norito::canonical_decode_limits(provenance.len()),
    )
    .map_err(|_| Error::Inventory)?;
    if provenance.source_manifest_sha256 != pins.sources {
        return Err(Error::Inventory);
    }
    regular_directory(&root.join("originals"))?;
    let bytes = pinned_file(
        &root.join("originals/inventory.norito"),
        INVENTORY_BYTES,
        pins.inventory,
    )?;
    let records: Vec<ArtifactRecord> =
        norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(|_| Error::Inventory)?;
    if records.is_empty() || records.len() > RECORDS {
        return Err(Error::Inventory);
    }
    Ok(records)
}
struct VerifierFiles {
    files: BTreeMap<[u8; 32], (PathBuf, usize)>,
    reads: usize,
}
impl VerifierFiles {
    fn new(root: &Path, records: &[ArtifactRecord]) -> Result<Self, Error> {
        let mut files = BTreeMap::new();
        let mut total = 0usize;
        let mut previous = None;
        for record in records {
            record.validate_identity().map_err(|_| Error::Inventory)?;
            if previous.is_some_and(|name: &[u8]| name >= record.name.as_slice()) {
                return Err(Error::Inventory);
            }
            previous = Some(record.name.as_slice());
            let stem = hex::encode(Sha256::digest(&record.name));
            for (index, (suffix, cap)) in [("descriptor", 1 << 20), ("vk", 1 << 18)]
                .into_iter()
                .enumerate()
            {
                let blob = BlobV1 {
                    bytes: record.lengths[index],
                    sha256: record.sha256[index],
                };
                let length = blob.length(cap)?;
                total = total
                    .checked_add(length)
                    .filter(|n| *n <= VERIFIER_BYTES)
                    .ok_or(Error::Inventory)?;
                if let Some((_, previous_length)) = files.get(&blob.sha256) {
                    if *previous_length != length {
                        return Err(Error::Inventory);
                    }
                } else {
                    files.insert(blob.sha256, (root.join(format!("{stem}.{suffix}")), length));
                }
            }
        }
        Ok(Self { files, reads: 0 })
    }
}
impl VerifierBlobSource for VerifierFiles {
    fn open(&mut self, digest: &[u8; 32]) -> Result<Box<dyn Read + '_>, FinalityError> {
        let (path, length) = self.files.get(digest).ok_or(FinalityError::Artifact)?;
        let bytes = pinned_file(path, *length, *digest).map_err(|_| FinalityError::Artifact)?;
        if bytes.len() != *length {
            return Err(FinalityError::Artifact);
        }
        self.reads += 1;
        Ok(Box::new(std::io::Cursor::new(bytes)))
    }
}
fn native_finality() -> SumeragiFinalityVerifier {
    let capture: norito::json::Value = norito::json::from_slice(FIXTURE).unwrap();
    let wire = hex::decode(capture["signed_genesis_wire_hex"].as_str().unwrap()).unwrap();
    let genesis = decode_framed_signed_block(&wire).unwrap();
    let epoch = genesis_epoch(&genesis).unwrap();
    let roster = epoch
        .committee
        .iter()
        .map(|member| FinalityValidator {
            public_key: member.validator.public_key().clone(),
            proof_of_possession: member.proof_of_possession.clone(),
        })
        .collect();
    SumeragiFinalityVerifier::new(&genesis, capture["chain_id"].as_str().unwrap(), roster).unwrap()
}
fn fixture_scope() -> SourceScopeV1 {
    // Public engineering root only; the same scalar and compiled provider are
    // used by the test authority owner. No production signing material is read.
    let root = SigningKey::from_bytes((&[0x11; 32]).into()).unwrap();
    let point = root.verifying_key().to_encoded_point(false);
    let sec1 = point.as_bytes();
    let provider = kagemusha_wallet_provider_contract_v1();
    SourceScopeV1::new(
        [
            u128::from_le_bytes(provider[..16].try_into().unwrap()),
            u128::from_le_bytes(provider[16..].try_into().unwrap()),
        ],
        Affine {
            x: words_from_be(sec1[1..33].try_into().unwrap()),
            y: words_from_be(sec1[33..65].try_into().unwrap()),
        },
    )
    .unwrap()
}
fn publish(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = path.parent().ok_or(Error::Inventory)?;
    let temporary = root.join(format!(
        ".pending-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| Error::Unavailable)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| Error::Unavailable)?;
    fs::hard_link(&temporary, path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            Error::Inventory
        } else {
            Error::Unavailable
        }
    })?;
    fs::remove_file(temporary).map_err(|_| Error::Unavailable)?;
    File::open(root)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| Error::Unavailable)
}

struct Originals {
    root: PathBuf,
    bytes: u64,
    count: usize,
}
impl OriginalSourceV1 for Originals {
    fn open(&mut self, digest: [u8; 32]) -> Result<Box<dyn Read + '_>, Error> {
        let path = self.root.join(hex::encode(digest));
        if !fs::symlink_metadata(&path)
            .map_err(|_| Error::Inventory)?
            .file_type()
            .is_file()
        {
            return Err(Error::Inventory);
        }
        Ok(Box::new(File::open(path).map_err(|_| Error::Inventory)?))
    }
}
impl OriginalSinkV1 for Originals {
    fn store(&mut self, identity: BlobV1, bytes: &[u8]) -> Result<(), Error> {
        if BlobV1::of(bytes) != identity {
            return Err(Error::Inventory);
        }
        let path = self.root.join(hex::encode(identity.sha256));
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                if pinned_file(&path, bytes.len(), identity.sha256)? != bytes {
                    return Err(Error::Inventory);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let total = self
                    .bytes
                    .checked_add(identity.bytes)
                    .filter(|n| *n <= OUTPUT_BYTES)
                    .ok_or(Error::Inventory)?;
                // Durable hard-link publication never replaces an existing address.
                publish(&path, bytes)?;
                self.bytes = total;
                self.count += 1;
                if bytes.len() > 1 << 20 {
                    eprintln!(
                        "WALLET_SOURCE_ORIGINAL count={} stored_bytes={}",
                        self.count, self.bytes
                    );
                }
            }
            Err(_) => return Err(Error::Inventory),
        }
        Ok(())
    }
}

#[test]
#[ignore = "explicit full52 source construction from independently pinned completed finality metadata; unsigned engineering catalog only"]
fn complete_wallet_catalog_from_pinned_finality_metadata() {
    let snapshot = PathBuf::from(
        std::env::var_os("KAGEMUSHA_FINALITY_METADATA_SNAPSHOT")
            .expect("separate immutable metadata snapshot"),
    );
    let output = PathBuf::from(
        std::env::var_os("KAGEMUSHA_WALLET_CATALOG_OUTPUT")
            .expect("fresh exclusive compiler output"),
    );
    let pins = Pins {
        producer: pin("KAGEMUSHA_FINALITY_PRODUCER_SHA256"),
        sources: pin("KAGEMUSHA_FINALITY_SOURCE_SHA256"),
        fixture: pin("KAGEMUSHA_FINALITY_FIXTURE_SHA256"),
        inventory: pin("KAGEMUSHA_FINALITY_INVENTORY_SHA256"),
    };
    let compiler_sources = pin("KAGEMUSHA_WALLET_SOURCE_SHA256");
    let records = records(&snapshot, pins).unwrap();
    let native = native_finality();
    let anchor = crate::kagemusha_wallet_finality_v1::derive_history_anchor(&native).unwrap();
    let mut metadata = VerifierFiles::new(&snapshot.join("originals"), &records).unwrap();
    eprintln!(
        "WALLET_SOURCE_PHASE complete_receipt_metadata records={} pk_reads=0",
        records.len()
    );
    let receipt = qualify_receipt(
        anchor,
        &records,
        &mut metadata,
        Parameters {
            pallas: PinnedParams::derive(16).unwrap(),
            vesta: PinnedParams::derive(16).unwrap(),
        },
        VerifierLimits {
            maximum_artifacts: RECORDS,
            maximum_verifier_bytes: VERIFIER_BYTES,
            msm_budget: MemoryBudget::DEFAULT,
        },
    )
    .unwrap();
    fs::create_dir(&output).expect("fresh output; no replacement or implicit resume");
    fs::create_dir(output.join("originals")).unwrap();
    let executable = std::env::current_exe().unwrap();
    let mut reader = File::open(executable).unwrap();
    let mut hash = Sha256::new();
    let mut buffer = [0; 16_384];
    loop {
        let length = reader.read(&mut buffer).unwrap();
        if length == 0 {
            break;
        }
        hash.update(&buffer[..length]);
    }
    publish(&output.join("binary.sha256"), &hash.finalize()).unwrap();
    publish(&output.join("source.sha256"), &compiler_sources).unwrap();
    publish(
        &output.join("input-pins.norito"),
        &norito::to_bytes(&(pins.producer, pins.sources, pins.fixture, pins.inventory)).unwrap(),
    )
    .unwrap();
    let scope = fixture_scope();
    publish(
        &output.join("source-policy.norito"),
        &norito::to_bytes(&(scope.provider(), scope.root().x, scope.root().y)).unwrap(),
    )
    .unwrap();
    let mut originals = Originals {
        root: output.join("originals"),
        bytes: 0,
        count: 0,
    };
    let config = ReadConfig {
        maximum_bytes: PROVING_KEY_MAX_BYTES_V1,
        maximum_rows: 1 << 16,
        coset_cache: CosetCachePolicy::OnDemand,
        msm_budget: MemoryBudget::DEFAULT,
    };
    eprintln!("WALLET_SOURCE_PHASE all52_compiled_routes signed=false wallet_grant=false");
    let inventory = OfflineCompilerV1::new(scope, &mut originals, config, OUTPUT_BYTES)
        .unwrap()
        .wallet(
            ReceiptSourceRecipeV1::new(receipt.source(), &anchor),
            FinalityV1 {
                network: anchor.network,
                instance: anchor.instance,
                initial_context: anchor.initial_context,
                initial_epoch: anchor.initial_epoch,
                parameters: anchor.parameters,
                originals: records,
            },
        )
        .unwrap();
    let bytes = inventory.to_canonical_bytes().unwrap();
    publish(&output.join("producer-inventory.norito"), &bytes).unwrap();
    publish(
        &output.join("producer-inventory.sha256"),
        &BlobV1::of(&bytes).sha256,
    )
    .unwrap();
    assert_eq!(inventory.routes.len(), compiled_routes().len());
    eprintln!(
        "WALLET_SOURCE_CATALOG routes={} programs={} terminal_keys={} originals={} stored_bytes={} receipt_metadata_reads={} signed=false qualified_wallet=false actual_receipt_proof=false",
        inventory.routes.len(),
        inventory.operations.len(),
        inventory.terminals.len(),
        inventory.originals.len(),
        originals.bytes,
        metadata.reads
    );
}

#[test]
fn pinned_reader_rejects_substitution_trailing_oversized_and_absent_originals() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("original");
    assert!(bounded_file(&path, 3).is_err());
    fs::write(&path, [1, 2, 3]).unwrap();
    let pin = BlobV1::of(&[1, 2, 3]).sha256;
    assert_eq!(pinned_file(&path, 3, pin).unwrap(), [1, 2, 3]);
    assert!(pinned_file(&path, 2, pin).is_err());
    assert!(pinned_file(&path, 3, [0; 32]).is_err());
    assert!(pinned_file(&path, 3, [1; 32]).is_err());
    fs::write(&path, [1, 2, 3, 4]).unwrap();
    assert!(pinned_file(&path, 4, pin).is_err());
    assert!(bounded_file(directory.path(), 4).is_err());
}

#[test]
fn output_preserves_exact_content_and_refuses_changed_address_or_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let mut originals = Originals {
        root: directory.path().to_owned(),
        bytes: 0,
        count: 0,
    };
    let original = [1, 2, 3];
    let identity = BlobV1::of(&original);
    originals.store(identity, &original).unwrap();
    originals.store(identity, &original).unwrap();
    assert_eq!((originals.bytes, originals.count), (3, 1));
    assert!(originals.store(identity, &[3, 2, 1]).is_err());
    fs::write(
        directory.path().join(hex::encode(identity.sha256)),
        [3, 2, 1],
    )
    .unwrap();
    assert!(originals.store(identity, &original).is_err());
    assert_eq!(
        fs::read(directory.path().join(hex::encode(identity.sha256))).unwrap(),
        [3, 2, 1]
    );
}
