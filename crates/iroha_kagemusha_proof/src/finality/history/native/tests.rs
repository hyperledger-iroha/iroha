//! Genuine finite catalog import and genesis proof, without block-finality claims.
//!
//! The body key here belongs to a real Genesis source deliberately incapable of
//! proving a `HistoryStep`. This tests closed catalog mounting and actual genesis
//! proof production; no complete signed block or receipt authorization is claimed.
use super::*;
use crate::{
    finality::continuity::{test_support::qualified, tree::OriginalBytes},
    omega::{OmegaCircuit, OmegaPlan, OmegaWitness},
};
use ff::{Field, PrimeField};
use iroha_pasta::{Fp, Fq, msm::MemoryBudget};
use iroha_plonk::{
    ProverRandomness, ProvingKey,
    cs::InstanceType,
    frontend::{Circuit, synthesize},
    keys::{CosetCachePolicy, KeygenConfigV2, keygen_pk_v2},
};
use iroha_plonk_recursion::FOLD_WITNESS_BYTES;
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};

fn artifact<C: iroha_pasta::PastaCurve>(key: &ProvingKey<C>) -> OriginalBytes {
    OriginalBytes {
        descriptor: key.binding().encoded().to_vec(),
        verifying_key: key.vk().to_bytes().to_vec(),
        proving_key: key.artifact_bytes_v2().unwrap(),
    }
}
fn borrowed(bytes: &OriginalBytes) -> OriginalArtifact<'_> {
    OriginalArtifact {
        descriptor: &bytes.descriptor,
        verifying_key: &bytes.verifying_key,
        proving_key: &bytes.proving_key,
    }
}
fn randomness(seed: u8) -> ProverRandomness<'static> {
    ProverRandomness::recovery(move |_context: &[u8; 32]| {
        Ok::<_, core::convert::Infallible>(ChaCha20Rng::from_seed([seed; 32]))
    })
}

#[test]
#[ignore = "actual original Genesis/Append/shared-wrapper imports and genuine genesis proof"]
fn finite_catalog_imports_both_original_sources_and_proves_actual_genesis() {
    let started = std::time::Instant::now();
    let anchor = HistoryAnchor {
        network: [1; 32],
        instance: [2; 32],
        initial_context: [3; 32],
        initial_epoch: 0,
        parameters: [1000, 2000, 3000, 4000, 1 << 20, 100],
    };
    let pallas = PinnedParams::<Ep>::derive(16).unwrap();
    let vesta = PinnedParams::<Eq>::derive(16).unwrap();
    let budget = MemoryBudget::DEFAULT;
    let genesis = GenesisSourceCircuit::for_source(anchor);
    let body = qualified(&genesis, &pallas, &vesta);
    let append_plan =
        HistoryAppendPlan::new(anchor, body.binding().clone(), body.clone(), pallas.clone())
            .unwrap();
    let append = HistoryAppendCircuit::for_source(append_plan).unwrap();
    let compiled = synthesize(&append, 16, None)
        .expect("canonical dynamic predecessor verifier and body verifier fit k16");
    let rows = compiled
        .tables
        .advice_assigned()
        .iter()
        .filter_map(|column| column.iter().rposition(|v| *v))
        .max()
        .map_or(0, |last| last + 1);
    eprintln!("finite history append: {rows} advice rows at k16; original layout only");
    drop(compiled);
    let mut source_config = KeygenConfigV2::pipa_r(vec![InstanceType::Bounded]);
    source_config.compress_selectors = false;
    let genesis_key = keygen_pk_v2(&vesta, &genesis, &source_config).unwrap();
    let append_key = keygen_pk_v2(&vesta, &append, &source_config).unwrap();
    assert_eq!(
        genesis_key.binding().encoded(),
        append_key.binding().encoded()
    );
    let keys = [genesis_key.vk().clone(), append_key.vk().clone()];
    let digests = keys
        .iter()
        .map(|key| key.kagemusha_digest(genesis_key.binding()).unwrap())
        .collect();
    let plan = OmegaPlan::new(genesis_key.binding().clone(), vesta.clone(), digests)
        .unwrap()
        .with_key_catalog(keys.to_vec())
        .unwrap();
    let length = plan.verifier().proof_length();
    let wrapper = OmegaCircuit::new(
        plan,
        OmegaWitness {
            key: keys[0].clone(),
            instances: vec![Fp::ZERO; 69],
            proof: vec![0; length],
            length: u32::try_from(length).unwrap(),
            fold: [0; FOLD_WITNESS_BYTES],
        },
    )
    .unwrap()
    .without_witnesses();
    let wrapper_key = keygen_pk_v2(
        &pallas,
        &wrapper,
        &KeygenConfigV2::pipa_r(OmegaPlan::instance_types().to_vec()),
    )
    .unwrap();
    assert_eq!(
        wrapper_key.binding().encoded(),
        body.binding().encoded(),
        "two-key wrapper shape is independent of future original key values"
    );
    let originals = [
        artifact(&genesis_key),
        artifact(&append_key),
        artifact(&wrapper_key),
    ];
    drop((genesis_key, append_key, wrapper_key));
    let config = ReadConfig {
        maximum_bytes: originals
            .iter()
            .map(|key| key.proving_key.len())
            .max()
            .unwrap(),
        maximum_rows: 1 << 16,
        coset_cache: CosetCachePolicy::OnDemand,
        msm_budget: budget,
    };
    let artifacts = HistoryArtifacts {
        genesis: borrowed(&originals[0]),
        append: borrowed(&originals[1]),
        wrapper: borrowed(&originals[2]),
    };
    assert!(
        HistoryProver::from_original_artifacts(
            anchor,
            body.clone(),
            HistoryArtifacts {
                genesis: artifacts.append,
                append: artifacts.genesis,
                ..artifacts
            },
            pallas.clone(),
            vesta.clone(),
            config
        )
        .is_err(),
        "catalog entries must implement their exact source owners"
    );
    let producer = HistoryProver::from_original_artifacts(
        anchor,
        body.clone(),
        artifacts,
        pallas.clone(),
        vesta.clone(),
        config,
    )
    .unwrap();
    let proof = producer
        .genesis(
            NodeRandomness {
                inner_salt: Fp::from(1),
                outer_salt: Fq::from(2).to_repr(),
                source: randomness(31),
                wrapper: randomness(32),
            },
            ProverConfig::default(),
        )
        .unwrap();
    let selected = producer.qualified_source().unwrap();
    assert_eq!(
        proof.endpoints,
        GenesisSourceCircuit::new(anchor, selected.key_digest().unwrap()).endpoints()
    );
    let opening = selected.verify_native(&proof, &vesta, budget).unwrap();
    assert_eq!(opening.source_k(), 16);
    proof.pallas.decide(&pallas, budget).unwrap();
    proof.vesta.decide(&vesta, budget).unwrap();
    let mut changed = proof.clone();
    changed.endpoints[1] += Fp::ONE;
    assert!(selected.verify_native(&changed, &vesta, budget).is_err());
    assert!(
        producer
            .append(
                proof,
                body.blank_evidence().unwrap(),
                NodeRandomness {
                    inner_salt: Fp::from(3),
                    outer_salt: Fq::from(4).to_repr(),
                    source: randomness(33),
                    wrapper: randomness(34)
                },
                ProverConfig::default(),
                &FoldConfig::default()
            )
            .is_err(),
        "a genesis-only or placeholder body grants no block step"
    );
    eprintln!(
        "finite two-source catalog and genuine genesis proof passed in {:?}; no block-finality proof",
        started.elapsed()
    );
}
