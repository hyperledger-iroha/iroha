//! Offline integrity and actual source/import tests, without complete-finality claims.

use super::*;
use iroha_pasta::{Fq, msm::MemoryBudget};
use iroha_plonk::{
    ProverConfig, ProverRandomness, keys::pk::artifact::ReadConfig, pcs::ipa::PinnedParams,
};
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _result = fs::remove_dir_all(&self.0);
    }
}
fn directory(limits: ImportLimits) -> (Temporary, DirectoryCatalog) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "kg-finality-catalog-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let store = DirectoryCatalog::create(&path, limits).unwrap();
    (Temporary(path), store)
}
fn limits() -> ImportLimits {
    ImportLimits {
        key: ReadConfig {
            maximum_bytes: 128 << 20,
            maximum_rows: 1 << 16,
            coset_cache: CosetCachePolicy::OnDemand,
            msm_budget: MemoryBudget::DEFAULT,
        },
        maximum_artifacts: 128,
        maximum_original_bytes: 4_usize << 30,
    }
}
fn sample() -> OriginalBytes {
    // Resource/integrity bytes only: no importer or proof ever accepts them.
    OriginalBytes {
        descriptor: vec![1],
        verifying_key: vec![2],
        proving_key: vec![3, 4],
    }
}
fn id() -> ArtifactId {
    ArtifactId::Source(NodeId::Genesis)
}
fn files(path: &std::path::Path, suffix: &str) -> Vec<PathBuf> {
    fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|x| x == suffix))
        .collect()
}

#[test]
fn bounded_original_inventory_roundtrips_without_trust_or_replacement() {
    let (dir, mut store) = directory(limits());
    assert!(DirectoryCatalog::create(&dir.0, limits()).is_err());
    let bytes = sample();
    store.store(&id(), &bytes).unwrap();
    store.store(&id(), &bytes).unwrap();
    assert_eq!(store.original_bytes(), 4);
    let inventory = store.inventory().unwrap();
    drop(store);
    let mut restored = DirectoryCatalog::reopen(&dir.0, &inventory, limits()).unwrap();
    assert_eq!(restored.inventory().unwrap(), inventory);
    let decoded = restored.load(&id()).unwrap();
    assert_eq!(decoded.descriptor, bytes.descriptor);
    assert_eq!(decoded.verifying_key, bytes.verifying_key);
    assert_eq!(decoded.proving_key, bytes.proving_key);
    let mut changed = sample();
    changed.proving_key[0] ^= 1;
    assert!(restored.store(&id(), &changed).is_err());
    assert!(restored.load(&ArtifactId::Source(NodeId::Append)).is_err());
    assert!(
        restored
            .store(
                &ArtifactId::Source(NodeId::ProgramMerge(Program::Bls, 0)),
                &bytes
            )
            .is_err()
    );
}

#[test]
fn canonical_record_identity_distinguishes_receipt_source_and_wrapper() {
    let (_dir, mut store) = directory(limits());
    let source = ArtifactId::Source(NodeId::Composition(Composition::Receipt));
    let wrapper = ArtifactId::Wrapper(NodeId::Composition(Composition::Receipt));
    store.store(&source, &sample()).unwrap();
    store.store(&wrapper, &sample()).unwrap();
    let bytes = store.inventory().unwrap();
    let records: Vec<ArtifactRecord> =
        norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
            .unwrap();
    assert_eq!(
        records
            .iter()
            .filter(|r| r.matches_identity(&source).unwrap())
            .count(),
        1
    );
    assert_eq!(
        records
            .iter()
            .filter(|r| r.matches_identity(&wrapper).unwrap())
            .count(),
        1
    );
    for record in &records {
        assert_ne!(
            record.matches_identity(&source).unwrap(),
            record.matches_identity(&wrapper).unwrap()
        );
        assert!(
            !record
                .matches_identity(&ArtifactId::HistoryWrapper)
                .unwrap()
        );
        let mut malformed = record.clone();
        malformed.name.push(0);
        assert!(malformed.matches_identity(&wrapper).is_err());
        assert!(
            record
                .matches_identity(&ArtifactId::Source(NodeId::ProgramMerge(Program::Bls, 0)))
                .is_err()
        );
    }
}

#[test]
fn changed_truncated_and_oversized_original_files_fail_before_import() {
    for change in 0..3 {
        let (dir, mut store) = directory(limits());
        store.store(&id(), &sample()).unwrap();
        let path = files(&dir.0, "pk").pop().unwrap();
        let bytes = match change {
            0 => vec![9, 4],
            1 => vec![3],
            _ => vec![3, 4, 5],
        };
        fs::write(path, bytes).unwrap();
        assert!(store.load(&id()).is_err());
    }
}

#[test]
fn inventory_canonicality_duplicates_names_and_limits_are_checked() {
    let (dir, mut store) = directory(limits());
    store.store(&id(), &sample()).unwrap();
    let original = store.inventory().unwrap();
    let records: Vec<ArtifactRecord> = norito::decode_canonical_with_limits(
        &original,
        norito::canonical_decode_limits(original.len()),
    )
    .unwrap();
    for change in 0..5 {
        let mut changed = records.clone();
        match change {
            0 => changed.push(changed[0].clone()),
            1 => changed[0].name.push(0),
            2 => changed[0].lengths[2] = u64::MAX,
            3 => changed[0].lengths[0] = 0,
            _ => changed[0].name[0] ^= 1,
        }
        let bytes = norito::to_bytes(&changed).unwrap();
        assert!(DirectoryCatalog::reopen(&dir.0, &bytes, limits()).is_err());
    }
    let mut trailing = original.clone();
    trailing.push(0);
    assert!(DirectoryCatalog::reopen(&dir.0, &trailing, limits()).is_err());
    assert!(
        DirectoryCatalog::reopen(
            &dir.0,
            &original,
            ImportLimits {
                maximum_original_bytes: 3,
                ..limits()
            }
        )
        .is_err()
    );
    assert!(
        store::check_limits(ImportLimits {
            maximum_artifacts: usize::MAX,
            ..limits()
        })
        .is_err()
    );
    assert!(
        store::check_limits(ImportLimits {
            maximum_original_bytes: 0,
            ..limits()
        })
        .is_err()
    );
}

#[test]
fn finite_distinct_inventory_limit_never_resets_on_duplicate_writes() {
    let (_dir, mut store) = directory(ImportLimits {
        maximum_artifacts: 1,
        maximum_original_bytes: 4,
        ..limits()
    });
    store.store(&id(), &sample()).unwrap();
    store.store(&id(), &sample()).unwrap();
    assert!(
        store
            .store(&ArtifactId::Source(NodeId::Append), &sample())
            .is_err()
    );
    assert_eq!(store.original_bytes(), 4);
    let mut oversized = sample();
    oversized.descriptor.resize((1 << 20) + 1, 0);
    assert!(store::check_original(&oversized, limits()).is_err());
}

fn parameters() -> Parameters {
    Parameters {
        pallas: PinnedParams::derive(16).unwrap(),
        vesta: PinnedParams::derive(16).unwrap(),
    }
}
fn compiler(sink: &mut dyn ArtifactSink, params: Parameters, limits: ImportLimits) -> Compiler<'_> {
    Compiler {
        sink,
        params,
        limits,
        cache: BTreeMap::new(),
        emitted: BTreeMap::new(),
        total: 0,
    }
}
fn anchor() -> HistoryAnchor {
    // Constructor-only policy; no signed genesis or live receipt is claimed.
    HistoryAnchor {
        network: [1; 32],
        instance: [2; 32],
        initial_context: [3; 32],
        initial_epoch: 0,
        parameters: [1000, 2000, 3000, 4000, 1 << 20, 100],
    }
}
fn randomness(seed: u8) -> ProverRandomness<'static> {
    ProverRandomness::recovery(move |_context: &[u8; 32]| {
        Ok::<_, core::convert::Infallible>(ChaCha20Rng::from_seed([seed; 32]))
    })
}

#[test]
#[ignore = "actual k16 source/wrapper compilation, original import and two genuine proofs"]
fn compiled_original_genesis_pair_proves_and_rejects_changed_source_tables() {
    use crate::finality::history::GenesisSourceCircuit;
    let (_dir, mut store) = directory(limits());
    let params = parameters();
    let source = GenesisSourceCircuit::for_source(anchor());
    let qualified = compiler(&mut store, params.clone(), limits())
        .source(NodeId::Genesis, &source)
        .unwrap();
    let mut original = store.load(&id()).unwrap();
    let wrapper = store.load(&ArtifactId::Wrapper(NodeId::Genesis)).unwrap();
    let mount = |original: &OriginalBytes, source: &GenesisSourceCircuit| {
        Prover::from_original_artifacts(
            source,
            borrowed(original),
            borrowed(&wrapper),
            params.pallas.clone(),
            params.vesta.clone(),
            limits().key,
        )
    };
    let producer = mount(&original, &source).unwrap();
    assert_eq!(
        identity(&producer.qualified_source().unwrap()).unwrap(),
        identity(&qualified).unwrap()
    );
    let mut changed = anchor();
    changed.parameters[0] += 1;
    assert!(mount(&original, &GenesisSourceCircuit::for_source(changed)).is_err());
    let byte = original.proving_key.last_mut().unwrap();
    *byte ^= 1;
    assert!(mount(&original, &source).is_err());
    let live = GenesisSourceCircuit::new(anchor(), Fp::from(7));
    let proof = producer
        .prove(
            &live,
            Fq::from(11).to_repr(),
            randomness(51),
            randomness(52),
            ProverConfig::default(),
        )
        .unwrap();
    assert_eq!(proof.endpoints, live.endpoints());
    let _opening = qualified
        .verify_native(&proof, &params.vesta, MemoryBudget::DEFAULT)
        .unwrap();
    crate::finality::continuity::checkpoint::exercise_actual_checkpoint(
        &qualified,
        &proof,
        &params.vesta,
    );
    let mut changed = proof.clone();
    changed.endpoints[5] += Fp::ONE;
    assert!(
        qualified
            .verify_native(&changed, &params.vesta, MemoryBudget::DEFAULT)
            .is_err()
    );
    eprintln!(
        "FINALITY_CATALOG_GENESIS_PAIR original_bytes={} wrapper_proof_bytes={} strict_original_import=true live_proof=true complete_history=false",
        store.original_bytes(),
        proof.proof.len()
    );
}

#[test]
#[ignore = "real k16 keys, bounded eviction/regeneration, strict import and genuine proofs"]
fn streaming_original_genesis_regenerates_exact_keys_and_restores_inventory() {
    use crate::finality::history::GenesisSourceCircuit;
    use std::time::Instant;
    let path = std::env::temp_dir().join(format!(
        "kg-finality-streaming-genuine-{}",
        std::process::id()
    ));
    let dir = Temporary(path);
    let cap = limits().key.maximum_bytes;
    let mut store = StreamingCatalog::create(&dir.0, limits(), cap).unwrap();
    let params = parameters();
    let source = GenesisSourceCircuit::for_source(anchor());
    let start = Instant::now();
    let qualified = compiler(&mut store, params.clone(), limits())
        .source(NodeId::Genesis, &source)
        .unwrap();
    let compile_seconds = start.elapsed().as_secs_f64();
    let inventory = store.inventory().unwrap();
    let records: Vec<ArtifactRecord> = norito::decode_canonical(&inventory).unwrap();
    assert_eq!(records.len(), 2);
    let total_pk: u64 = records.iter().map(|record| record.lengths[2]).sum();
    assert!(total_pk > u64::try_from(cap).unwrap());
    let resident = || {
        files(&dir.0, "pk")
            .iter()
            .map(|path| fs::metadata(path).unwrap().len())
            .sum::<u64>()
    };
    assert!(resident() <= u64::try_from(cap).unwrap());
    let start = Instant::now();
    let original = store.load(&id()).unwrap();
    let source_regeneration_seconds = start.elapsed().as_secs_f64();
    assert_eq!(files(&dir.0, "pk").len(), 1);
    let start = Instant::now();
    let wrapper_id = ArtifactId::Wrapper(NodeId::Genesis);
    let outer = store.load(&wrapper_id).unwrap();
    let wrapper_regeneration_seconds = start.elapsed().as_secs_f64();
    assert_eq!(files(&dir.0, "pk").len(), 1);
    assert!(resident() <= u64::try_from(cap).unwrap());
    assert_eq!(store.inventory().unwrap(), inventory);
    let start = Instant::now();
    let producer = Prover::from_original_artifacts(
        &source,
        borrowed(&original),
        borrowed(&outer),
        params.pallas.clone(),
        params.vesta.clone(),
        limits().key,
    )
    .unwrap();
    let import_seconds = start.elapsed().as_secs_f64();
    drop(original);
    drop(outer);
    let start = Instant::now();
    let live = GenesisSourceCircuit::new(anchor(), Fp::from(7));
    let proof = producer
        .prove(
            &live,
            Fq::from(11).to_repr(),
            randomness(71),
            randomness(72),
            ProverConfig::default(),
        )
        .unwrap();
    let _opening = qualified
        .verify_native(&proof, &params.vesta, MemoryBudget::DEFAULT)
        .unwrap();
    let proof_seconds = start.elapsed().as_secs_f64();
    drop(producer);
    drop(store);
    let mut resumed = StreamingCatalog::reopen(&dir.0, limits(), cap).unwrap();
    assert_eq!(resident(), 0);
    assert!(
        resumed.load(&id()).is_err(),
        "no recipe is restored from disk"
    );
    let start = Instant::now();
    let recompiled = compiler(&mut resumed, params.clone(), limits())
        .source(NodeId::Genesis, &source)
        .unwrap();
    let resume_seconds = start.elapsed().as_secs_f64();
    assert_eq!(
        identity(&recompiled).unwrap(),
        identity(&qualified).unwrap()
    );
    assert_eq!(resumed.inventory().unwrap(), inventory);
    let _opening = recompiled
        .verify_native(&proof, &params.vesta, MemoryBudget::DEFAULT)
        .unwrap();
    let mut changed = anchor();
    changed.parameters[0] += 1;
    resumed
        .register_recipe(
            &id(),
            OriginalRecipe::source(
                &GenesisSourceCircuit::for_source(changed),
                &params,
                limits(),
            ),
        )
        .unwrap();
    assert!(
        resumed.load(&id()).is_err(),
        "changed compiled source must not regenerate an old key"
    );
    assert_eq!(resumed.inventory().unwrap(), inventory);
    eprintln!(
        "FINALITY_STREAMING_GENESIS compile_s={compile_seconds:.3} source_regeneration_s={source_regeneration_seconds:.3} wrapper_regeneration_s={wrapper_regeneration_seconds:.3} import_s={import_seconds:.3} proof_and_decide_s={proof_seconds:.3} resume_compile_s={resume_seconds:.3} pk_cap={cap} logical_original_bytes={} exact_inventory=true actual_proof=true full_finality=false",
        resumed.original_bytes()
    );
}

#[test]
#[ignore = "six actual initial source layouts, twelve original keys and strict imports; no live finality"]
fn actual_initial_sources_compile_and_import_all_six_profiles() {
    use crate::finality::native::source_layout as s;
    // The retained 128 MiB diagnostic rejected Result0's measured 165,677,622 B
    // original. This explicit offline read cap is not a production RSS limit.
    let survey_limits = ImportLimits {
        key: ReadConfig {
            maximum_bytes: 256 << 20,
            ..limits().key
        },
        ..limits()
    };
    let (_dir, mut store) = directory(survey_limits);
    let params = parameters();
    let mut compiler = compiler(&mut store, params, survey_limits);
    macro_rules! source {
        ($program:ident, $factory:ident) => {{
            let (class, source) = s::$factory(0).unwrap();
            let key = compiler.source(NodeId::Leaf(Program::$program, class), &source).unwrap();
            let originals = compiler.emitted[&ArtifactId::Source(NodeId::Leaf(Program::$program, class))];
            let wrapper = compiler.emitted[&ArtifactId::Wrapper(NodeId::Leaf(Program::$program, class))];
            eprintln!("FINALITY_CATALOG_SOURCE program={:?} class={} descriptor={:?} source_original_bytes={} wrapper_original_bytes={} per_original_read_cap={} original_import=true full_program=false", Program::$program, class, key.binding().digest(), originals, wrapper, survey_limits.key.maximum_bytes);
        }};
    }
    source!(Aggregation, aggregation);
    source!(Bls, bls);
    source!(Result, result);
    source!(Schedule, schedule);
    source!(Context, context);
    source!(Load, load);
    assert_eq!(compiler.emitted.len(), 12);
}

#[test]
#[ignore = "three fixed context batch sources, strict original imports and six genuine proofs"]
fn compiled_context_batches_prove_exact_spans_and_reject_foreign_class() {
    use crate::finality::{
        native::source_layout,
        schedule::context_hash::{
            BATCH_LENGTH, CRC_LEAVES, ContextHashBatchCircuit, ContextHashBatchPlan,
            EPOCH_CODEC_ID, EPOCH_TAG, prepare_context_batches,
        },
    };
    let payload = (0..289)
        .map(|i| u8::try_from(i % 251).unwrap())
        .collect::<Vec<_>>();
    let mut preimage = EPOCH_TAG.to_vec();
    preimage.extend_from_slice(b"NRT0\0\0");
    preimage.extend_from_slice(&EPOCH_CODEC_ID);
    preimage.push(0);
    preimage.extend_from_slice(&u64::try_from(payload.len()).unwrap().to_le_bytes());
    preimage.extend_from_slice(&norito::core::hardware_crc64(&payload).to_le_bytes());
    preimage.push(2);
    preimage.extend_from_slice(&payload);
    let id = iroha_crypto::Hash::new(preimage).into();
    let sources = prepare_context_batches(payload, 0, 289, id).unwrap();
    let read = ImportLimits {
        key: ReadConfig {
            maximum_bytes: 256 << 20,
            ..limits().key
        },
        ..limits()
    };
    let (_dir, mut store) = directory(read);
    let params = parameters();
    for (index, expected_start, expected_end) in [
        (0, 0_u32, 2_u32),
        (CRC_LEAVES / 2, CRC_LEAVES, CRC_LEAVES + 2),
        (BATCH_LENGTH - 1, 2_560, 2_561),
    ] {
        let (class, layout) = source_layout::context(index).unwrap();
        let node = NodeId::Leaf(Program::Context, class);
        let qualified = compiler(&mut store, params.clone(), read)
            .source(node.clone(), &layout)
            .unwrap();
        let original = store.load(&ArtifactId::Source(node.clone())).unwrap();
        let wrapper = store.load(&ArtifactId::Wrapper(node)).unwrap();
        let mount = |source: &ContextHashBatchCircuit| {
            Prover::from_original_artifacts(
                source,
                borrowed(&original),
                borrowed(&wrapper),
                params.pallas.clone(),
                params.vesta.clone(),
                read.key,
            )
        };
        let producer = mount(&layout).unwrap();
        assert_eq!(
            identity(&producer.qualified_source().unwrap()).unwrap(),
            identity(&qualified).unwrap()
        );
        let foreign = if index == 0 {
            ContextHashBatchPlan::BlakePair
        } else {
            ContextHashBatchPlan::CrcPair
        };
        assert!(mount(&ContextHashBatchCircuit::for_source(foreign)).is_err());
        let source = &sources[usize::try_from(index).unwrap()];
        assert_eq!(source.endpoints()[2], Fp::from(u64::from(expected_start)));
        assert_eq!(source.endpoints()[3], Fp::from(u64::from(expected_end)));
        let proof = producer
            .prove(
                source,
                Fq::from(11).to_repr(),
                randomness(71),
                randomness(72),
                ProverConfig::default(),
            )
            .unwrap();
        assert_eq!(proof.endpoints, source.endpoints());
        let _opening = qualified
            .verify_native(&proof, &params.vesta, MemoryBudget::DEFAULT)
            .unwrap();
        for endpoint in 0..6 {
            let mut changed = proof.clone();
            changed.endpoints[endpoint] += Fp::ONE;
            assert!(
                qualified
                    .verify_native(&changed, &params.vesta, MemoryBudget::DEFAULT)
                    .is_err()
            );
        }
        eprintln!(
            "FINALITY_CONTEXT_BATCH_PROOF class={class} semantic_start={expected_start} semantic_end={expected_end} descriptor={:?} source_original_bytes={} wrapper_original_bytes={} proof_bytes={} strict_original_import=true foreign_class_rejected=true all_endpoint_mutations_rejected=true complete_context=false",
            qualified.binding().digest(),
            original.proving_key.len(),
            wrapper.proving_key.len(),
            proof.proof.len()
        );
    }
}

#[test]
#[ignore = "four actual BLS pair original imports and eight genuine source/wrapper proofs"]
fn compiled_bls_pairs_prove_and_reject_changed_cursor_keys() {
    use crate::finality::{
        bls::{BlsBatchPlan, prepare_bls_batches},
        native::source_layout,
    };
    use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature};
    use iroha_plonk_gadgets::bls12_381::curve::{
        g1_program::G1_SUBGROUP_STEPS, programs::G2_SUBGROUP_STEPS,
    };
    let mut secret = [0; 32];
    secret[0] = 17;
    let key =
        KeyPair::from_private_key(PrivateKey::from_bytes(Algorithm::BlsNormal, &secret).unwrap())
            .unwrap();
    let message = core::array::from_fn(|i| u8::try_from(i).unwrap());
    let signature = Signature::new(key.private_key(), &message);
    let sources = prepare_bls_batches(
        message,
        key.public_key().to_bytes().1.try_into().unwrap(),
        signature.payload().try_into().unwrap(),
    )
    .unwrap();
    let sha_pair =
        u32::try_from(2 + G1_SUBGROUP_STEPS.len() + G2_SUBGROUP_STEPS.len()).unwrap() / 2;
    let mut selected = vec![0, 190, sha_pair, BlsBatchPlan::LENGTH - 1];
    selected.sort_unstable();
    selected.dedup();
    let read = ImportLimits {
        key: ReadConfig {
            maximum_bytes: 256 << 20,
            ..limits().key
        },
        ..limits()
    };
    let (_dir, mut store) = directory(read);
    let params = parameters();
    for index in selected {
        let (class, layout) = source_layout::bls(index).unwrap();
        let node = NodeId::Leaf(Program::Bls, class);
        let qualified = compiler(&mut store, params.clone(), read)
            .source(node.clone(), &layout)
            .unwrap();
        let original = store.load(&ArtifactId::Source(node.clone())).unwrap();
        let wrapper = store.load(&ArtifactId::Wrapper(node)).unwrap();
        let mount = |source: &crate::finality::bls::BlsBatchCircuit| {
            Prover::from_original_artifacts(
                source,
                borrowed(&original),
                borrowed(&wrapper),
                params.pallas.clone(),
                params.vesta.clone(),
                read.key,
            )
        };
        let producer = mount(&layout).unwrap();
        assert_eq!(
            identity(&producer.qualified_source().unwrap()).unwrap(),
            identity(&qualified).unwrap()
        );
        let (_, foreign) = source_layout::bls((index + 1) % BlsBatchPlan::LENGTH).unwrap();
        assert!(mount(&foreign).is_err(), "cursor pair key must be exact");
        let source = &sources[usize::try_from(index).unwrap()];
        let proof = producer
            .prove(
                source,
                Fq::from(19).to_repr(),
                randomness(81),
                randomness(82),
                ProverConfig::default(),
            )
            .unwrap();
        assert_eq!(proof.endpoints, source.endpoints());
        assert_eq!(proof.endpoints[2], Fp::from(2 * u64::from(index)));
        assert_eq!(proof.endpoints[3], Fp::from(2 * u64::from(index) + 2));
        let _opening = qualified
            .verify_native(&proof, &params.vesta, MemoryBudget::DEFAULT)
            .unwrap();
        for endpoint in 0..6 {
            let mut changed = proof.clone();
            changed.endpoints[endpoint] += Fp::ONE;
            assert!(
                qualified
                    .verify_native(&changed, &params.vesta, MemoryBudget::DEFAULT)
                    .is_err()
            );
        }
        eprintln!(
            "FINALITY_BLS_PAIR_PROOF class={class} semantic_start={} semantic_end={} descriptor={:?} source_original_bytes={} wrapper_original_bytes={} proof_bytes={} strict_original_import=true foreign_cursor_rejected=true all_endpoint_mutations_rejected=true complete_signature_program=false",
            2 * index,
            2 * index + 2,
            qualified.binding().digest(),
            original.proving_key.len(),
            wrapper.proving_key.len(),
            proof.proof.len()
        );
    }
}

#[test]
#[ignore = "three fixed result batch sources, strict original imports and six genuine proofs"]
fn compiled_result_batches_prove_exact_spans_and_reject_foreign_class() {
    use crate::finality::{
        native::source_layout,
        result::RESULT_TAG,
        result_scan::{
            RESULT_BATCH_LENGTH, ResultScanBatchCircuit, ResultScanBatchPlan,
            prepare_result_batches,
        },
    };
    let fixture: norito::json::Value = norito::json::from_str(include_str!(
        "../../../../../fixtures/kagemusha/ordinary_load_receipt_v1.json"
    ))
    .unwrap();
    let encoded = fixture
        .get("result_preimage_hex")
        .unwrap()
        .as_str()
        .unwrap();
    let frame = (0..encoded.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&encoded[i..i + 2], 16).unwrap())
        .collect::<Vec<_>>();
    let mut preimage = RESULT_TAG.to_vec();
    preimage.extend_from_slice(&frame);
    let expected = iroha_crypto::Hash::new(preimage).into();
    let sources = prepare_result_batches(&frame, expected).unwrap();
    let read = ImportLimits {
        key: ReadConfig {
            maximum_bytes: 256 << 20,
            ..limits().key
        },
        ..limits()
    };
    let (_dir, mut store) = directory(read);
    let params = parameters();
    for (index, expected_start, expected_end) in [
        (0, 0_u32, 2_u32),
        (1, 2, 4),
        (RESULT_BATCH_LENGTH - 1, 514, 515),
    ] {
        let (class, layout) = source_layout::result(index).unwrap();
        let node = NodeId::Leaf(Program::Result, class);
        let qualified = compiler(&mut store, params.clone(), read)
            .source(node.clone(), &layout)
            .unwrap();
        let original = store.load(&ArtifactId::Source(node.clone())).unwrap();
        let wrapper = store.load(&ArtifactId::Wrapper(node)).unwrap();
        let mount = |source: &ResultScanBatchCircuit| {
            Prover::from_original_artifacts(
                source,
                borrowed(&original),
                borrowed(&wrapper),
                params.pallas.clone(),
                params.vesta.clone(),
                read.key,
            )
        };
        let producer = mount(&layout).unwrap();
        assert_eq!(
            identity(&producer.qualified_source().unwrap()).unwrap(),
            identity(&qualified).unwrap()
        );
        let foreign = if index == 0 {
            ResultScanBatchPlan::AbsorbPair
        } else {
            ResultScanBatchPlan::StartAbsorb
        };
        assert!(mount(&ResultScanBatchCircuit::for_source(foreign).unwrap()).is_err());
        let source = &sources[usize::try_from(index).unwrap()];
        assert_eq!(source.endpoints()[2], Fp::from(u64::from(expected_start)));
        assert_eq!(source.endpoints()[3], Fp::from(u64::from(expected_end)));
        let proof = producer
            .prove(
                source,
                Fq::from(23).to_repr(),
                randomness(91),
                randomness(92),
                ProverConfig::default(),
            )
            .unwrap();
        assert_eq!(proof.endpoints, source.endpoints());
        let _opening = qualified
            .verify_native(&proof, &params.vesta, MemoryBudget::DEFAULT)
            .unwrap();
        for endpoint in 0..6 {
            let mut changed = proof.clone();
            changed.endpoints[endpoint] += Fp::ONE;
            assert!(
                qualified
                    .verify_native(&changed, &params.vesta, MemoryBudget::DEFAULT)
                    .is_err()
            );
        }
        eprintln!(
            "FINALITY_RESULT_BATCH_PROOF class={class} semantic_start={expected_start} semantic_end={expected_end} descriptor={:?} source_original_bytes={} wrapper_original_bytes={} proof_bytes={} strict_original_import=true foreign_class_rejected=true all_endpoint_mutations_rejected=true complete_result=false",
            qualified.binding().digest(),
            original.proving_key.len(),
            wrapper.proving_key.len(),
            proof.proof.len()
        );
    }
}

#[path = "tests/result_reuse_fixture.rs"]
mod result_reuse_fixture;
