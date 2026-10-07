//! Bounded offline driver over the exact independently generated native receipt capture.
//!
//! The fixture anchor is a selected test policy, not a production genesis admission.
//! Its receipt has ordinal seven; this driver never relabels it as a first Load.

use ff::{Field, PrimeField};
use iroha_kagemusha_proof::finality::{
    catalog::{self, ArtifactSink, OriginalRecipe, SourceProvenance, StreamingCatalog},
    continuity::{
        SourceNodeEvidence, SourceVerifier,
        checkpoint::{ProofCheckpointStore, ProofIdentity},
        producer::Error,
        tree::{NodeRandomness, OriginalBytes},
    },
    history::HistoryAnchor,
    native::{
        ArtifactId, ArtifactSource, BlockWitnessInput, ImportLimits, LoadWitnessInput, Parameters,
        ProvingContext,
    },
};
use iroha_pasta::{Ep, Eq, Fp, Fq, msm::MemoryBudget};
use iroha_plonk::{
    ProverConfig, ProverRandomness,
    keys::{CosetCachePolicy, pk::artifact::ReadConfig},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_recursion::{ACCUMULATOR_BYTES, AccumulatorT, FoldConfig};
use norito::{Decode, Encode, NoritoSchema, json::Value};
use rand_chacha::rand_core::OsRng;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// Bounded verifier-only reuse of a completed immutable receipt capture.
#[path = "finality_driver/restore.rs"]
pub mod restore;

const FIXTURE: &str = include_str!("../../../../fixtures/kagemusha/ordinary_load_receipt_v1.json");
const CHECKPOINT_LIMIT: usize = 1 << 20;
const CHECKPOINT_TOTAL: usize = 256 << 20;
const MAX_CHECKPOINTS: usize = 16_384;
const WORKING_PK_BYTES: usize = 512 << 20;

fn value<'a>(json: &'a Value, name: &str) -> Result<&'a Value, Error> {
    json.get(name).ok_or(Error::Input)
}
fn bytes(json: &Value, name: &str) -> Result<Vec<u8>, Error> {
    hex(value(json, name)?.as_str().ok_or(Error::Input)?)
}
fn fixed<const N: usize>(json: &Value, name: &str) -> Result<[u8; N], Error> {
    bytes(json, name)?.try_into().map_err(|_| Error::Input)
}
fn hex(text: &str) -> Result<Vec<u8>, Error> {
    if text.len() > 2 * (1 << 20)
        || !text.len().is_multiple_of(2)
        || !text.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(Error::Input);
    }
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = core::str::from_utf8(pair).map_err(|_| Error::Input)?;
            u8::from_str_radix(pair, 16).map_err(|_| Error::Input)
        })
        .collect()
}
fn hex_out(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(2 * bytes.len());
    for byte in bytes {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 15)]));
    }
    out
}
struct Fixture {
    anchor: HistoryAnchor,
    block: BlockWitnessInput,
    load: LoadWitnessInput,
    receipt_digest: Fp,
}
fn fixture(capture: &str) -> Result<Fixture, Error> {
    if capture.len() > 1 << 20 {
        return Err(Error::Input);
    }
    let json: Value = norito::json::from_str(capture).map_err(|_| Error::Input)?;
    let anchor = value(&json, "history_anchor")?;
    let anchor = HistoryAnchor {
        network: fixed(anchor, "network_hex")?,
        instance: fixed(anchor, "instance_hex")?,
        initial_context: fixed(anchor, "initial_context_hex")?,
        initial_epoch: value(anchor, "initial_epoch")?
            .as_u64()
            .ok_or(Error::Input)?,
        parameters: value(anchor, "parameters")?
            .as_array()
            .ok_or(Error::Input)?
            .iter()
            .map(|value| value.as_u64().ok_or(Error::Input))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Error::Input)?,
    };
    let schedule = value(&json, "authenticated_schedule")?;
    let current_context = fixed(value(schedule, "current")?, "context_id_hex")?;
    if !value(schedule, "boundary")?.is_null() {
        return Err(Error::Input);
    }
    let roster = value(&json, "committee_public_keys_hex")?
        .as_array()
        .ok_or(Error::Input)?;
    if !(4..=31).contains(&roster.len()) || roster.len() % 3 != 1 {
        return Err(Error::Input);
    }
    let result_frame = bytes(&json, "result_preimage_hex")?;
    let block = BlockWitnessInput {
        result_frame: result_frame.clone(),
        message: fixed(&json, "commit_vote_preimage_hex")?,
        roster: roster
            .iter()
            .map(|value| {
                hex(value.as_str().ok_or(Error::Input)?)?
                    .try_into()
                    .map_err(|_| Error::Input)
            })
            .collect::<Result<Vec<_>, _>>()?,
        bitmap: bytes(&json, "qc_bitmap_hex")?,
        signature: fixed(&json, "qc_aggregate_signature_hex")?,
        current_context,
        authorized_context: current_context,
    };
    let event_count = value(&json, "event_commitment_count")?
        .as_u64()
        .ok_or(Error::Input)?;
    if event_count != 1 {
        return Err(Error::Input);
    }
    let load = LoadWitnessInput {
        result_frame,
        receipt: fixed(&json, "receipt_transcript_hex")?,
        event_root: fixed(&json, "event_commitment_root_hex")?,
        event_count,
        event_index: 0,
        siblings: [[0; 32]; 32],
    };
    let recorded = Option::<Fp>::from(Fp::from_repr(fixed(&json, "receipt_digest_hex")?))
        .ok_or(Error::Input)?;
    let receipt_digest = iroha_plonk_gadgets::bytes::p_bytes_native(
        iroha_kagemusha_proof::finality::LoadReceiptCells::DOMAIN,
        &load.receipt,
    );
    if recorded != receipt_digest {
        return Err(Error::Input);
    }
    Ok(Fixture {
        anchor,
        block,
        load,
        receipt_digest,
    })
}

#[derive(Clone, Debug, Encode, Decode, NoritoSchema)]
#[norito_schema(name = "kagemusha_test::FinalityCheckpointV1")]
struct Record {
    version: u8,
    descriptor: [u8; 32],
    key: [u8; 32],
    endpoints: [[u8; 32]; 6],
    proof: Vec<u8>,
    pallas: [u8; ACCUMULATOR_BYTES],
    vesta: [u8; ACCUMULATOR_BYTES],
}
/// Advisory directory custody for this offline test driver. The open descriptor
/// holds the lock through compile/prove/recovery and the OS releases it on exit.
struct RunLock {
    _directory: File,
}
fn directory_exists(path: &Path) -> Result<bool, Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Ok(_) | Err(_) => Err(Error::Artifact),
    }
}
fn ensure_directory(path: &Path) -> Result<(), Error> {
    if !directory_exists(path)? {
        fs::create_dir(path).map_err(|_| Error::Artifact)?;
    }
    Ok(())
}
impl RunLock {
    fn acquire(root: &Path) -> Result<Self, Error> {
        ensure_directory(root)?;
        let directory = File::open(root).map_err(|_| Error::Artifact)?;
        directory.try_lock().map_err(|_| Error::Artifact)?;
        Ok(Self {
            _directory: directory,
        })
    }
}
struct Checkpoints {
    root: PathBuf,
    entries: usize,
    bytes: usize,
}
fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, Error> {
    if !fs::symlink_metadata(path)
        .map_err(|_| Error::Artifact)?
        .file_type()
        .is_file()
    {
        return Err(Error::Artifact);
    }
    let file = File::open(path).map_err(|_| Error::Artifact)?;
    let length = usize::try_from(file.metadata().map_err(|_| Error::Artifact)?.len())
        .map_err(|_| Error::Artifact)?;
    if length > maximum {
        return Err(Error::Artifact);
    }
    let mut out = Vec::with_capacity(length);
    file.take(
        u64::try_from(length)
            .map_err(|_| Error::Artifact)?
            .checked_add(1)
            .ok_or(Error::Artifact)?,
    )
    .read_to_end(&mut out)
    .map_err(|_| Error::Artifact)?;
    if out.len() != length {
        return Err(Error::Artifact);
    }
    Ok(out)
}
fn publish(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = path.parent().ok_or(Error::Artifact)?;
    let temporary = root.join(format!(
        ".pending-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| Error::Artifact)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| Error::Artifact)?;
    fs::hard_link(&temporary, path).map_err(|_| Error::Artifact)?;
    fs::remove_file(temporary).map_err(|_| Error::Artifact)?;
    File::open(root)
        .and_then(|f| f.sync_all())
        .map_err(|_| Error::Artifact)
}
impl Checkpoints {
    fn open(root: PathBuf) -> Result<Self, Error> {
        ensure_directory(&root)?;
        let mut result = Self {
            root,
            entries: 0,
            bytes: 0,
        };
        for entry in fs::read_dir(&result.root).map_err(|_| Error::Artifact)? {
            let entry = entry.map_err(|_| Error::Artifact)?;
            let name = entry.file_name();
            let name = name.to_str().ok_or(Error::Artifact)?;
            if name.starts_with(".pending-") {
                if !entry.file_type().map_err(|_| Error::Artifact)?.is_file() {
                    return Err(Error::Artifact);
                }
                // Root custody is held by run(): this is interrupted publication,
                // never an authenticated checkpoint or an externally owned file.
                fs::remove_file(entry.path()).map_err(|_| Error::Artifact)?;
            } else if name.len() == 71
                && name.ends_with(".norito")
                && name[..64].bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                let size = read_bounded(&entry.path(), CHECKPOINT_LIMIT)?.len();
                result.entries += 1;
                result.bytes = result.bytes.checked_add(size).ok_or(Error::Artifact)?;
            } else {
                return Err(Error::Artifact);
            }
        }
        if result.entries > MAX_CHECKPOINTS || result.bytes > CHECKPOINT_TOTAL {
            return Err(Error::Artifact);
        }
        Ok(result)
    }
    fn path(&self, id: &ProofIdentity) -> PathBuf {
        let mut h = Sha256::new();
        h.update(b"kg-finality-checkpoint1");
        h.update(id.source.descriptor);
        h.update(id.source.key);
        for word in id.endpoints {
            h.update(word);
        }
        self.root.join(format!("{}.norito", hex_out(&h.finalize())))
    }
}
impl ProofCheckpointStore for Checkpoints {
    fn load(&mut self, id: &ProofIdentity) -> Result<Option<SourceNodeEvidence>, Error> {
        let path = self.path(id);
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(Error::Artifact),
            Ok(_) => {}
        }
        let bytes = read_bounded(&path, CHECKPOINT_LIMIT)?;
        let record: Record = norito::decode_canonical_with_limits(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| Error::Artifact)?;
        if record.version != 1
            || record.descriptor != id.source.descriptor
            || record.key != id.source.key
            || record.endpoints != id.endpoints
        {
            return Err(Error::Artifact);
        }
        let endpoints = record
            .endpoints
            .map(|v| Option::<Fp>::from(Fp::from_repr(v)).ok_or(Error::Input))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Error::Input)?;
        Ok(Some(SourceNodeEvidence {
            endpoints,
            proof: record.proof,
            pallas: AccumulatorT::<Ep>::from_bytes(&record.pallas).map_err(|_| Error::Input)?,
            vesta: AccumulatorT::<Eq>::from_bytes(&record.vesta).map_err(|_| Error::Input)?,
        }))
    }
    fn store(&mut self, id: &ProofIdentity, proof: &SourceNodeEvidence) -> Result<(), Error> {
        if proof.endpoints.map(|v| v.to_repr()) != id.endpoints {
            return Err(Error::Input);
        }
        let record = Record {
            version: 1,
            descriptor: id.source.descriptor,
            key: id.source.key,
            endpoints: id.endpoints,
            proof: proof.proof.clone(),
            pallas: proof.pallas.to_bytes(),
            vesta: proof.vesta.to_bytes(),
        };
        let bytes = norito::to_bytes(&record).map_err(|_| Error::Artifact)?;
        let path = self.path(id);
        if path.try_exists().map_err(|_| Error::Artifact)? {
            return if read_bounded(&path, CHECKPOINT_LIMIT)? == bytes {
                Ok(())
            } else {
                Err(Error::Artifact)
            };
        }
        let total = self.bytes.checked_add(bytes.len()).ok_or(Error::Artifact)?;
        if bytes.len() > CHECKPOINT_LIMIT
            || self.entries >= MAX_CHECKPOINTS
            || total > CHECKPOINT_TOTAL
        {
            return Err(Error::Artifact);
        }
        publish(&path, &bytes)?;
        self.entries += 1;
        self.bytes = total;
        eprintln!(
            "FINALITY_CHECKPOINT durable_nodes={} bytes={} source={} interval={:?}..{:?}",
            self.entries,
            self.bytes,
            hex_out(&id.source.key),
            proof.endpoints[2],
            proof.endpoints[3]
        );
        Ok(())
    }
}

struct Progress {
    inner: StreamingCatalog,
    writes: usize,
}
impl ArtifactSource for Progress {
    fn load(&mut self, id: &ArtifactId) -> Result<OriginalBytes, Error> {
        self.inner.load(id)
    }
}
impl ArtifactSink for Progress {
    fn register_recipe(&mut self, id: &ArtifactId, recipe: OriginalRecipe) -> Result<(), Error> {
        self.inner.register_recipe(id, recipe)
    }
    fn store(&mut self, id: &ArtifactId, bytes: &OriginalBytes) -> Result<(), Error> {
        self.inner.store(id, bytes)?;
        self.writes += 1;
        eprintln!(
            "FINALITY_ARTIFACT checkpoint={} logical_bytes={} resident_pk_bytes={} node={id:?}",
            self.writes,
            self.inner.original_bytes(),
            self.inner.resident_bytes()
        );
        Ok(())
    }
}

/// Exact receipt proved by the complete compiled finality graph. The Load
/// producer must still independently verify this proof and its selected source.
#[derive(Clone)]
#[allow(dead_code)] // The ordinary-Load driver consumes these fields in its separate test binary.
pub struct FinalizedReceipt {
    /// Complete explicitly selected signed-fixture genesis anchor.
    pub anchor: HistoryAnchor,
    /// Exact terminal key qualified by the complete compiled source graph.
    pub source: SourceVerifier,
    /// Unchanged native ordinary receipt transcript.
    pub receipt: [u8; 282],
    /// Real terminal proof and both deciding carried claims.
    pub evidence: SourceNodeEvidence,
}

/// Prove the original ordinal-seven capture without altering its ledger terms.
pub fn run(root: &Path, manifest: &str) {
    run_capture(root, manifest, FIXTURE, |_| {});
}

/// Prove one exact native capture, retaining exclusive output custody through the callback.
pub fn run_capture(
    root: &Path,
    manifest: &str,
    capture: &str,
    complete: impl FnOnce(FinalizedReceipt),
) {
    let manifest: [u8; 32] = hex(manifest)
        .expect("source manifest hex")
        .try_into()
        .expect("source manifest SHA256");
    assert_ne!(manifest, [0; 32]);
    let _lock = RunLock::acquire(root).expect("exclusive real driver directory");
    let fixture = fixture(capture).expect("exact native capture");
    let capture_path = root.join("fixture.json");
    match fs::symlink_metadata(&capture_path) {
        Ok(_) => assert_eq!(
            read_bounded(&capture_path, 1 << 20).unwrap(),
            capture.as_bytes()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            publish(&capture_path, capture.as_bytes()).unwrap();
        }
        Err(error) => panic!("unavailable retained fixture: {error}"),
    }
    let executable = std::env::current_exe().unwrap();
    let executable_hash = Sha256::digest(read_bounded(&executable, 256 << 20).unwrap());
    let executable_path = root.join("binary.sha256");
    match fs::symlink_metadata(&executable_path) {
        Ok(_) => assert_eq!(
            read_bounded(&executable_path, 32).unwrap(),
            executable_hash.as_slice()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            publish(&executable_path, executable_hash.as_slice()).unwrap();
        }
        Err(error) => panic!("unavailable executable provenance: {error}"),
    }
    let ordinal = u128::from_le_bytes(fixture.load.receipt[130..146].try_into().unwrap());
    let limits = ImportLimits {
        key: ReadConfig {
            maximum_bytes: 256 << 20,
            maximum_rows: 1 << 16,
            coset_cache: CosetCachePolicy::OnDemand,
            msm_budget: MemoryBudget::DEFAULT,
        },
        maximum_artifacts: 4096,
        maximum_original_bytes: 512_usize << 30,
    };
    let artifact_root = root.join("originals");
    let inner = if directory_exists(&artifact_root).unwrap() {
        StreamingCatalog::reopen(&artifact_root, limits, WORKING_PK_BYTES)
    } else {
        StreamingCatalog::create(&artifact_root, limits, WORKING_PK_BYTES)
    }
    .unwrap();
    let mut artifacts = Progress { inner, writes: 0 };
    let provenance = SourceProvenance {
        revision: "explicit native captured fixture driver; component only".into(),
        source_manifest_sha256: manifest,
    };
    let provenance = norito::to_bytes(&provenance).unwrap();
    let provenance_path = root.join("provenance.norito");
    match fs::symlink_metadata(&provenance_path) {
        Ok(_) => assert_eq!(read_bounded(&provenance_path, 8192).unwrap(), provenance),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            publish(&provenance_path, &provenance).unwrap();
        }
        Err(error) => panic!("unavailable provenance: {error}"),
    }
    let provenance = norito::decode_canonical(&provenance).unwrap();
    eprintln!(
        "FINALITY_PHASE compile current_cold_proofs=8944 retained_pk_cap={} logical_inventory_cap={} fixture_ordinal={ordinal} no_wallet_admission=true",
        WORKING_PK_BYTES, limits.maximum_original_bytes
    );
    let compilation = catalog::compile(
        fixture.anchor,
        &mut artifacts,
        Parameters {
            pallas: PinnedParams::derive(16).unwrap(),
            vesta: PinnedParams::derive(16).unwrap(),
        },
        limits,
        provenance,
    )
    .unwrap();
    let mut checkpoints = Checkpoints::open(root.join("proofs")).unwrap();
    // Independent OS entropy for every proof and both salts, including after restart
    // or cache-skips. Invocation ordinals are progress labels, never entropy keys.
    let mut entropy = |request| {
        eprintln!("FINALITY_NODE {request:?}");
        Ok(NodeRandomness {
            inner_salt: Fp::random(OsRng),
            outer_salt: Fq::random(OsRng).to_repr(),
            source: ProverRandomness::hedged(),
            wrapper: ProverRandomness::hedged(),
        })
    };
    let fold = FoldConfig::default();
    let mut context =
        ProvingContext::new(&mut artifacts, &mut entropy, ProverConfig::default(), &fold)
            .with_checkpoints(&mut checkpoints);
    eprintln!("FINALITY_PHASE genesis");
    let prefix = compilation.graph.genesis(&mut context).unwrap();
    eprintln!("FINALITY_PHASE append_height2");
    let prefix = compilation
        .graph
        .append_block(&prefix, &fixture.block, &mut context)
        .unwrap();
    assert_eq!(prefix.state().next_height, 3);
    eprintln!("FINALITY_PHASE receipt");
    let evidence = compilation
        .graph
        .prove_receipt(&prefix, &fixture.load, &mut context)
        .unwrap();
    compilation
        .graph
        .verify_receipt_evidence(fixture.receipt_digest, &evidence, MemoryBudget::DEFAULT)
        .unwrap();
    eprintln!(
        "FINALITY_COMPLETE genuine_receipt=true current_cold_proofs=8944 proof_bytes={} exact_originals=true ordinal={ordinal} full_wallet_catalog=false",
        evidence.proof.len()
    );
    complete(FinalizedReceipt {
        anchor: fixture.anchor,
        source: compilation.graph.qualified_source(),
        receipt: fixture.load.receipt,
        evidence,
    });
}

/// Check the original fixture structural inputs without granting finality.
pub fn check_fixture() {
    let f = fixture(FIXTURE).expect("exact native capture");
    assert_eq!(f.block.roster.len(), 4);
    assert_eq!(f.block.bitmap, [7]);
    assert_eq!(
        u64::from_be_bytes(f.block.message[85..93].try_into().unwrap()),
        2
    );
    assert_eq!(
        u128::from_le_bytes(f.load.receipt[130..146].try_into().unwrap()),
        7
    );
    assert_eq!(f.load.receipt[2..34], [1; 32]);
    assert_eq!(f.block.result_frame.len(), 10242);
    assert_eq!(f.load.result_frame, f.block.result_frame);
    assert_eq!(f.anchor.initial_context, f.block.current_context);
    let leaves = iroha_kagemusha_proof::finality::load_source::prepare_load_source(
        &f.load.result_frame,
        &f.load.receipt,
        f.load.event_root,
        1,
        0,
        &f.load.siblings,
    )
    .unwrap();
    assert_eq!(leaves.len(), 35);
}

/// Exercise bounded canonical checkpoint files and failed-read distinctions.
pub fn check_checkpoint_files() {
    use iroha_kagemusha_proof::finality::continuity::tree::SourceIdentity;
    let root = std::env::temp_dir().join(format!("kg-finality-proof-files-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let interrupted = root.join(".pending-interrupted");
    fs::write(&interrupted, [1, 2, 3]).unwrap();
    let mut store = Checkpoints::open(root.clone()).unwrap();
    assert!(!interrupted.try_exists().unwrap());
    let id = ProofIdentity {
        source: SourceIdentity {
            descriptor: [1; 32],
            key: [2; 32],
        },
        endpoints: [Fp::ONE.to_repr(); 6],
    };
    assert!(store.load(&id).unwrap().is_none());
    // Codec-only payload. It is never passed to a source verifier or called valid.
    let proof = SourceNodeEvidence {
        endpoints: [Fp::ONE; 6],
        proof: vec![7],
        pallas: AccumulatorT::<Ep>::trivial(
            &PinnedParams::derive(16).unwrap(),
            MemoryBudget::DEFAULT,
        )
        .unwrap(),
        vesta: AccumulatorT::<Eq>::trivial(
            &PinnedParams::derive(16).unwrap(),
            MemoryBudget::DEFAULT,
        )
        .unwrap(),
    };
    store.store(&id, &proof).unwrap();
    drop(store);
    let mut store = Checkpoints::open(root.clone()).unwrap();
    assert_eq!(store.load(&id).unwrap().unwrap().proof, [7]);
    let path = store.path(&id);
    let original = fs::read(&path).unwrap();
    let mut bad = original.clone();
    bad.push(0);
    fs::write(&path, bad).unwrap();
    assert!(store.load(&id).is_err());
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(store.load(&id).is_err());
    fs::remove_dir(&path).unwrap();
    let foreign = root.join("unexpected");
    fs::write(&foreign, []).unwrap();
    assert!(Checkpoints::open(root.clone()).is_err());
    fs::remove_dir_all(root).unwrap();
}

/// Tests only the offline directory custody boundary, with no proof admission.
pub fn check_directory_custody() {
    let root = std::env::temp_dir().join(format!("kg-finality-custody-{}", std::process::id()));
    let lock = RunLock::acquire(&root).unwrap();
    assert!(RunLock::acquire(&root).is_err());
    drop(lock);
    let lock = RunLock::acquire(&root).unwrap();
    let ordinary_file = root.join("file");
    fs::write(&ordinary_file, []).unwrap();
    assert!(RunLock::acquire(&ordinary_file).is_err());
    assert!(Checkpoints::open(ordinary_file).is_err());
    #[cfg(unix)]
    {
        let linked = root.join("linked");
        std::os::unix::fs::symlink(&root, &linked).unwrap();
        assert!(RunLock::acquire(&linked).is_err());
        assert!(Checkpoints::open(linked.clone()).is_err());
        fs::remove_file(linked).unwrap();
    }
    drop(lock);
    fs::remove_dir_all(root).unwrap();
}
