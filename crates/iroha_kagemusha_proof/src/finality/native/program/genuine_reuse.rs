//! Opt-in parity of real captured result-source witnesses under original k16 keys.
//!
//! This tests one source class, not consensus, a complete finality interval or
//! deployment authority. Missing original PKs fail; this test never regenerates.

use super::*;
use crate::finality::{
    catalog::{ArtifactRecord, DirectoryCatalog},
    continuity::{
        checkpoint::{ProofCheckpointStore, ProofIdentity},
        tree::{NodeRandomness, OriginalBytes},
    },
    result_scan::{ResultScanBatchPlan, prepare_result_batches},
};
use ff::PrimeField;
use iroha_pasta::Fq;
use iroha_plonk::{ProverRandomness, keys::CosetCachePolicy};
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng as _};
use sha2::{Digest as _, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
};

const FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/kagemusha/ordinary_load_receipt_v1.json"
));
const PREFIX: &str = "KAGEMUSHA_FINALITY_REUSE_";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_hex(text: &str, maximum: usize) -> Vec<u8> {
    assert!(text.len() <= maximum * 2 && text.len().is_multiple_of(2));
    assert!(
        text.bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn pin(name: &str) -> [u8; 32] {
    let text =
        std::env::var(format!("{PREFIX}{name}_SHA256")).expect("explicit independent input pin");
    let value: [u8; 32] = decode_hex(&text, 32).try_into().unwrap();
    assert_ne!(value, [0; 32]);
    value
}

fn path(name: &str) -> PathBuf {
    std::env::var_os(format!("{PREFIX}{name}"))
        .map(PathBuf::from)
        .expect("explicit original/output path; no fallback")
}

fn bounded(path: &Path, maximum: usize) -> Vec<u8> {
    let before = fs::symlink_metadata(path).expect("required immutable original exists");
    assert!(before.file_type().is_file());
    let file = File::open(path).unwrap();
    let length = usize::try_from(file.metadata().unwrap().len()).unwrap();
    assert!(length > 0 && length <= maximum);
    assert_eq!(before.len(), length as u64);
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

fn limits() -> ImportLimits {
    ImportLimits {
        key: ReadConfig {
            maximum_bytes: 256 << 20,
            maximum_rows: 1 << 16,
            coset_cache: CosetCachePolicy::OnDemand,
            msm_budget: MemoryBudget::DEFAULT,
        },
        maximum_artifacts: 4096,
        maximum_original_bytes: 512_usize << 30,
    }
}

fn selected_id() -> NodeId {
    NodeId::Leaf(Program::Result, 1)
}

struct Originals {
    source: DirectoryCatalog,
    loads: Vec<ArtifactId>,
    tamper_source_use: Option<usize>,
    source_uses: usize,
    deny: bool,
}

impl Originals {
    fn open(root: &Path, inventory: &[u8]) -> Self {
        Self {
            source: DirectoryCatalog::reopen(root, inventory, limits()).unwrap(),
            loads: Vec::new(),
            tamper_source_use: None,
            source_uses: 0,
            deny: false,
        }
    }
}

impl ArtifactSource for Originals {
    fn load(&mut self, id: &ArtifactId) -> Result<OriginalBytes, Error> {
        assert!(!self.deny, "verified checkpoints must not load originals");
        assert!(
            *id == ArtifactId::Source(selected_id()) || *id == ArtifactId::Wrapper(selected_id())
        );
        self.loads.push(id.clone());
        let mut bytes = self.source.load(id)?;
        if *id == ArtifactId::Source(selected_id()) {
            self.source_uses += 1;
            if self.tamper_source_use == Some(self.source_uses) {
                // Mutation is in this returned test buffer, never the immutable archive.
                *bytes.proving_key.last_mut().expect("real nonempty PK") ^= 1;
            }
        }
        Ok(bytes)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CheckpointEvent {
    Load(ProofIdentity),
    Store(ProofIdentity),
}

#[derive(Clone, Default)]
struct Checkpoints {
    values: Vec<(ProofIdentity, SourceNodeEvidence)>,
    events: Vec<CheckpointEvent>,
}

fn evidence_equal(left: &SourceNodeEvidence, right: &SourceNodeEvidence) {
    assert_eq!(left.endpoints, right.endpoints);
    assert_eq!(left.proof, right.proof);
    assert_eq!(left.pallas.to_bytes(), right.pallas.to_bytes());
    assert_eq!(left.vesta.to_bytes(), right.vesta.to_bytes());
    assert_eq!(left.instances().unwrap(), right.instances().unwrap());
}

impl ProofCheckpointStore for Checkpoints {
    fn load(&mut self, identity: &ProofIdentity) -> Result<Option<SourceNodeEvidence>, Error> {
        self.events.push(CheckpointEvent::Load(*identity));
        Ok(self
            .values
            .iter()
            .find(|(key, _)| key == identity)
            .map(|(_, proof)| proof.clone()))
    }

    fn store(&mut self, identity: &ProofIdentity, proof: &SourceNodeEvidence) -> Result<(), Error> {
        self.events.push(CheckpointEvent::Store(*identity));
        assert!(self.values.iter().all(|(key, _)| key != identity));
        self.values.push((*identity, proof.clone()));
        Ok(())
    }
}

fn proof_randomness(seed: u8) -> ProverRandomness<'static> {
    // Existing genuine finality tests use this public recovery API; the
    // dependency's cfg(test)-only fixed-seed constructor is not available here.
    ProverRandomness::recovery(move |_context: &[u8; 32]| {
        Ok::<_, core::convert::Infallible>(ChaCha20Rng::from_seed([seed; 32]))
    })
}

fn randomness(
    request: ProofRequest,
    log: &mut Vec<ProofRequest>,
) -> Result<NodeRandomness<'static>, Error> {
    assert_eq!(request.node, selected_id());
    assert!(request.sequence < 3);
    let index = u8::try_from(request.sequence).unwrap();
    log.push(request);
    Ok(NodeRandomness {
        inner_salt: Fp::from(u64::from(index) + 101),
        outer_salt: Fq::from(u64::from(index) + 201).to_repr(),
        source: proof_randomness(10 + index),
        wrapper: proof_randomness(20 + index),
    })
}

fn save_evidence(output: &Path, branch: &str, index: usize, proof: &SourceNodeEvidence) {
    retain(output, &format!("{branch}-{index}.proof"), &proof.proof);
    retain(
        output,
        &format!("{branch}-{index}.pallas"),
        &proof.pallas.to_bytes(),
    );
    retain(
        output,
        &format!("{branch}-{index}.vesta"),
        &proof.vesta.to_bytes(),
    );
    let endpoints: Vec<_> = proof
        .endpoints
        .iter()
        .flat_map(|word| word.to_repr())
        .collect();
    retain(output, &format!("{branch}-{index}.endpoints"), &endpoints);
}

#[test]
#[ignore = "eight real k16 source/wrapper proofs; exact original Result1 D/V/PK archive and exclusive output required"]
fn genuine_result_pair_reuse_preserves_proofs_custody_and_witness_checks() {
    let root = path("ORIGINALS");
    let inventory_path = path("INVENTORY");
    let output = path("OUTPUT");
    let inventory_pin = pin("INVENTORY");
    let fixture_pin = pin("FIXTURE");
    assert_eq!(<[u8; 32]>::from(Sha256::digest(FIXTURE)), fixture_pin);
    assert!(fs::symlink_metadata(&root).unwrap().file_type().is_dir());
    let directory = File::open(&root).unwrap();
    directory
        .try_lock()
        .expect("exclusive immutable artifact fixture, never live server cache");
    let inventory = bounded(&inventory_path, 4096 * 2048 + 4096);
    assert_eq!(<[u8; 32]>::from(Sha256::digest(&inventory)), inventory_pin);
    let records: Vec<ArtifactRecord> = norito::decode_canonical_with_limits(
        &inventory,
        norito::canonical_decode_limits(inventory.len()),
    )
    .unwrap();
    let mut selected = Vec::new();
    for id in [
        ArtifactId::Source(selected_id()),
        ArtifactId::Wrapper(selected_id()),
    ] {
        let matching: Vec<_> = records
            .iter()
            .filter(|record| record.matches_identity(&id).unwrap())
            .collect();
        assert_eq!(
            matching.len(),
            1,
            "exact installed Result1 source/wrapper role"
        );
        selected.push(matching[0].clone());
    }
    let mut probe = Originals::open(&root, &inventory);
    // Fail on absent/tampered PK data before deriving parameters or importing keys.
    for id in [
        ArtifactId::Source(selected_id()),
        ArtifactId::Wrapper(selected_id()),
    ] {
        drop(
            probe
                .load(&id)
                .expect("all six genuine originals required; no keygen fallback"),
        );
    }
    drop(probe);
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder
        .create(&output)
        .expect("fresh exclusively owned output");
    retain(
        &output,
        "selected-records.norito",
        &norito::to_bytes(&selected).unwrap(),
    );
    retain(&output, "fixture.json", FIXTURE);
    retain(&output, "inventory.sha256", &inventory_pin);

    let json: norito::json::Value = norito::json::from_slice(FIXTURE).unwrap();
    let frame = decode_hex(json["result_preimage_hex"].as_str().unwrap(), 65_536);
    let expected: [u8; 32] = decode_hex(json["result_hash_hex"].as_str().unwrap(), 32)
        .try_into()
        .unwrap();
    let witnesses = prepare_result_batches(&frame, expected).unwrap();
    assert_eq!(
        witnesses.len(),
        crate::finality::result_scan::RESULT_BATCH_LENGTH as usize
    );
    assert_eq!(
        ResultScanBatchPlan::at(1),
        Some(ResultScanBatchPlan::AbsorbPair)
    );
    assert_eq!(
        ResultScanBatchPlan::at(2),
        Some(ResultScanBatchPlan::AbsorbPair)
    );
    let first = witnesses[1].endpoints();
    let second = witnesses[2].endpoints();
    assert_eq!(first[..2], second[..2]);
    assert_eq!((first[3], first[5]), (second[2], second[4]));
    assert_ne!(
        first, second,
        "two distinct actual captured witness intervals"
    );

    let params = Parameters {
        pallas: PinnedParams::derive(16).unwrap(),
        vesta: PinnedParams::derive(16).unwrap(),
    };
    let mut mount_originals = Originals::open(&root, &inventory);
    let mounted = Mounted::mount(
        selected_id(),
        &witnesses[1],
        &mut GraphSource::Original(&mut mount_originals),
        &params,
        limits(),
        None,
    )
    .unwrap();
    assert_eq!(mount_originals.loads.len(), 2);
    drop(mount_originals);
    let fold = FoldConfig::default();
    let mut fresh_checks = Checkpoints::default();
    let mut fresh_log = Vec::new();
    let mut fresh_originals = Originals::open(&root, &inventory);
    let mut provider = |request| randomness(request, &mut fresh_log);
    let mut context = ProvingContext::new(
        &mut fresh_originals,
        &mut provider,
        ProverConfig::default(),
        &fold,
    )
    .with_checkpoints(&mut fresh_checks);
    let mut fresh = Vec::new();
    for (index, witness) in witnesses[1..3].iter().enumerate() {
        eprintln!("GENUINE_IMPORT_REUSE branch=fresh leaf={index} start=true");
        let entropy = context.entropy(selected_id()).unwrap();
        let proof = mounted
            .prove_with(witness, &params, limits(), &mut context, entropy, None)
            .unwrap();
        let _opening = mounted
            .source
            .verify_native_cancellable(&proof, &params.vesta, MemoryBudget::DEFAULT, None)
            .unwrap();
        save_evidence(&output, "fresh", index, &proof);
        fresh.push(proof);
        if index == 0 {
            // The second requested witness is another genuine compiled layout.
            // Fresh strict import must reject it before producing a proof.
            let entropy = context.entropy(selected_id()).unwrap();
            assert_eq!(
                mounted
                    .prove_with(
                        &witnesses[0],
                        &params,
                        limits(),
                        &mut context,
                        entropy,
                        None
                    )
                    .err(),
                Some(Error::Artifact)
            );
        }
    }
    drop(context);
    let mut cached_checks = Checkpoints::default();
    let mut cached_log = Vec::new();
    let mut cached_originals = Originals::open(&root, &inventory);
    let mut provider = |request| randomness(request, &mut cached_log);
    let mut cache = LastImported::new(limits().key.maximum_bytes);
    let mut context = ProvingContext::new(
        &mut cached_originals,
        &mut provider,
        ProverConfig::default(),
        &fold,
    )
    .with_checkpoints(&mut cached_checks);
    for (index, witness) in witnesses[1..3].iter().enumerate() {
        eprintln!("GENUINE_IMPORT_REUSE branch=cached leaf={index} start=true");
        let proof = mounted
            .prove(witness, &params, limits(), &mut context, &mut cache)
            .unwrap();
        let _opening = mounted
            .source
            .verify_native_cancellable(&proof, &params.vesta, MemoryBudget::DEFAULT, None)
            .unwrap();
        evidence_equal(&fresh[index], &proof);
        save_evidence(&output, "cached", index, &proof);
        if index == 0 {
            // Same second request and original class, but the cached owner must
            // independently reject this actual witness's different fixed layout.
            assert_eq!(
                mounted
                    .prove(&witnesses[0], &params, limits(), &mut context, &mut cache)
                    .err(),
                Some(Error::Prover)
            );
        }
    }
    drop(context);
    assert_eq!(fresh_log, cached_log);
    assert_eq!(fresh_originals.loads, cached_originals.loads);
    assert_eq!(cached_originals.loads.len(), 6);
    assert_eq!(fresh_log.len(), 3);
    assert_eq!(fresh_checks.events, cached_checks.events);
    let first_id = ProofIdentity::new(&mounted.source, fresh[0].endpoints).unwrap();
    let wrong_id = ProofIdentity::new(&mounted.source, witnesses[0].endpoints()).unwrap();
    let second_id = ProofIdentity::new(&mounted.source, fresh[1].endpoints).unwrap();
    assert_eq!(
        cached_checks.events,
        [
            CheckpointEvent::Load(first_id),
            CheckpointEvent::Store(first_id),
            CheckpointEvent::Load(wrong_id),
            CheckpointEvent::Load(second_id),
            CheckpointEvent::Store(second_id),
        ]
    );
    drop(cache);

    // Prime with a real strict import, then mutate only the second original load.
    // This negative must stop before any second proof or checkpoint publication.
    let mut changed = Originals::open(&root, &inventory);
    changed.tamper_source_use = Some(2);
    let mut cache = LastImported::new(limits().key.maximum_bytes);
    let imported = cache
        .get_or_import(&selected_id(), &mut changed, None, |pair| {
            Prover::from_original_artifacts_cancellable(
                &witnesses[1],
                borrowed(&pair.source),
                borrowed(&pair.wrapper),
                params.pallas.clone(),
                params.vesta.clone(),
                limits().key,
                None,
            )
        })
        .unwrap();
    assert_eq!(
        identity(&imported.qualified_source().unwrap()).unwrap(),
        identity(&mounted.source).unwrap()
    );
    let mut negative_log = Vec::new();
    let mut provider = |request| randomness(request, &mut negative_log);
    let mut refused_checks = Checkpoints::default();
    let mut context =
        ProvingContext::new(&mut changed, &mut provider, ProverConfig::default(), &fold)
            .with_checkpoints(&mut refused_checks);
    assert_eq!(
        mounted
            .prove(&witnesses[2], &params, limits(), &mut context, &mut cache)
            .err(),
        Some(Error::Artifact)
    );
    drop(context);
    drop(cache);
    assert_eq!(changed.source_uses, 2);
    assert!(refused_checks.values.is_empty());
    assert_eq!(refused_checks.events, [CheckpointEvent::Load(second_id)]);
    assert_eq!(
        negative_log,
        [ProofRequest {
            node: selected_id(),
            sequence: 0
        }]
    );

    // Replay uses the actual fully verified retained evidence, never token stubs.
    let previous_events = cached_checks.events.len();
    cached_originals.deny = true;
    let previous_loads = cached_originals.loads.len();
    let mut replay_log = Vec::new();
    let mut provider = |request| randomness(request, &mut replay_log);
    let mut context = ProvingContext::new(
        &mut cached_originals,
        &mut provider,
        ProverConfig::default(),
        &fold,
    )
    .with_checkpoints(&mut cached_checks);
    let mut cache = LastImported::new(limits().key.maximum_bytes);
    for (index, witness) in witnesses[1..3].iter().enumerate() {
        let proof = mounted
            .prove(witness, &params, limits(), &mut context, &mut cache)
            .unwrap();
        evidence_equal(&fresh[index], &proof);
    }
    drop(context);
    assert_eq!(
        replay_log,
        [
            ProofRequest {
                node: selected_id(),
                sequence: 0
            },
            ProofRequest {
                node: selected_id(),
                sequence: 1
            },
        ]
    );
    assert_eq!(cached_originals.loads.len(), previous_loads);
    assert_eq!(cached_checks.events.len(), previous_events + 2);
    let mut corrupt = cached_checks.clone();
    let before_corrupt_events = corrupt.events.len();
    corrupt.values[1].1.proof[0] ^= 1;
    let mut corrupt_log = Vec::new();
    let mut provider = |request| randomness(request, &mut corrupt_log);
    let mut context = ProvingContext::new(
        &mut cached_originals,
        &mut provider,
        ProverConfig::default(),
        &fold,
    )
    .with_checkpoints(&mut corrupt);
    assert_eq!(
        mounted
            .prove(&witnesses[2], &params, limits(), &mut context, &mut cache)
            .err(),
        Some(Error::Proof)
    );
    drop(context);
    assert_eq!(cached_originals.loads.len(), previous_loads);
    assert_eq!(corrupt.values.len(), 2);
    assert_eq!(corrupt.events.len(), before_corrupt_events + 1);
    assert_eq!(
        corrupt_log,
        [ProofRequest {
            node: selected_id(),
            sequence: 0
        }]
    );
    assert_eq!(
        corrupt.events.last(),
        Some(&CheckpointEvent::Load(second_id))
    );
    assert_eq!(bounded(&inventory_path, inventory.len()), inventory);
    // Exact public request/checkpoint sequences are retained with the evidence.
    let requests: Vec<_> = fresh_log.iter().map(|request| request.sequence).collect();
    let replay_requests: Vec<_> = replay_log.iter().map(|request| request.sequence).collect();
    let negative_requests: Vec<_> = negative_log
        .iter()
        .map(|request| request.sequence)
        .collect();
    let corrupt_requests: Vec<_> = corrupt_log.iter().map(|request| request.sequence).collect();
    let events: Vec<_> = fresh_checks
        .events
        .iter()
        .map(|event| {
            let (kind, identity) = match event {
                CheckpointEvent::Load(identity) => ("load", identity),
                CheckpointEvent::Store(identity) => ("store", identity),
            };
            let endpoints: Vec<_> = identity.endpoints.iter().map(|word| hex(word)).collect();
            norito::json!({"kind": kind, "descriptor": (hex(&identity.source.descriptor)),
            "key": (hex(&identity.source.key)), "endpoints": endpoints})
        })
        .collect();
    retain(&output, "sequences.json", &norito::json::to_vec(&norito::json!({
        "node": "Leaf(Result,1)", "fresh_and_cached_requests": requests,
        "fresh_and_cached_checkpoints": events,
        "replay_requests": replay_requests, "second_original_requests": negative_requests,
        "replay_checkpoint_mutation_requests": corrupt_requests,
        "fixed_test_entropy": "request i: inner Fp(i+101), outer Fq(i+201), source ChaCha20 [10+i;32], wrapper [20+i;32]",
        "scope": "provider request sequence, not production entropy or hidden prover tape observation"
    })).unwrap());
    let result = norito::json!({
        "schema": "iroha.kagemusha.genuine-program-import-reuse.v1",
        "inventory_sha256": (hex(&inventory_pin)), "fixture_sha256": (hex(&fixture_pin)),
        "two_distinct_captured_intervals": true, "real_source_and_wrapper_proofs": 8,
        "retained_wrapper_proofs": 4, "exact_evidence_parity": true,
        "entropy_and_checkpoint_sequence_parity": true,
        "second_original_mutation_refused": true, "second_layout_mutation_refused": true,
        "verified_checkpoint_replay_without_original_reads": true,
        "mutated_checkpoint_refused": true,
        "scope": "one genuine Result1 class; no complete finality, ledger, RSS, timing or release qualification"
    });
    retain(
        &output,
        "result.json",
        &norito::json::to_vec(&result).unwrap(),
    );
    eprintln!(
        "GENUINE_IMPORT_REUSE complete=true real_proofs=8 exact_evidence=true component_only=true"
    );
}
