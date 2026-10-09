//! Genuine two-curve source composition and adversarial endpoint binding.

use super::super::tree::{IntervalTree, NodeRandomness, TreeImportConfig};
use super::*;
use crate::finality::continuity::producer::{self, OriginalArtifact, SourceCircuit};
use crate::omega::{OmegaCircuit, OmegaWitness};
use iroha_pasta::Fq;
use iroha_pasta::msm::MemoryBudget;
use iroha_plonk::verifier::verify_full;
use iroha_plonk::{
    ProverConfig, ProverRandomness, ProvingKey, Witness,
    check::{CheckMode, check_circuit},
    create_proof_owned_with_claim,
    cs::InstanceType,
    frontend::{Value, synthesize},
    keys::{CosetCachePolicy, KeygenConfigV2, keygen_pk_v2, pk::artifact::ReadConfig},
};
use iroha_plonk_recursion::{FoldInput, create_fold};
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};

// This deliberately small source program increments both its cursor and state.
// It qualifies composition only, and does not assert validator finality.
#[derive(Clone)]
struct Increment {
    endpoints: [Fp; 6],
    known: bool,
    step: u64,
}
impl producer::sealed::Source for Increment {
    fn exports(
        &self,
        pallas: &PinnedParams<Ep>,
        vesta: &PinnedParams<Eq>,
        budget: MemoryBudget,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<producer::Exports, producer::Error> {
        producer::leaf_exports(self.endpoints, pallas, vesta, budget, cancellation)
    }
}
impl SourceCircuit for Increment {}
impl Circuit<Fp> for Increment {
    type Config = SourcePairConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        SourceMergeCircuit::configure(meta)
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let output = layouter.assign_region(
            || "increment source program",
            |mut region| {
                let values = self.endpoints.map(|x| {
                    if self.known {
                        Value::known(x)
                    } else {
                        Value::unknown()
                    }
                });
                let words = chip.uint().glue().witnesses(&mut region, &values)?;
                let words: [_; 6] = words.try_into().map_err(|_| Error::Synthesis)?;
                chip.uint()
                    .glue()
                    .enforce_constant(&mut region, &words[0], Fp::from(901))?;
                let endpoints = SourceEndpoints::from_words(&mut chip, &mut region, &words)?;
                for (before, after) in [(2, 3), (4, 5)] {
                    let plus_one = chip.uint().glue().linear(
                        &mut region,
                        &[(Fp::ONE, &words[before])],
                        Fp::from(self.step),
                    )?;
                    GlueChip::assert_equal(&mut region, &plus_one, &words[after])?;
                }
                SourceCheckpoint::leaf(&mut chip, &mut region, endpoints)?
                    .frame(&mut chip, &mut region)
            },
        )?;
        for (index, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, index)?;
        }
        Ok(())
    }
}

fn randomness(seed: u8) -> ProverRandomness<'static> {
    ProverRandomness::recovery(move |_context: &[u8; 32]| {
        Ok::<_, core::convert::Infallible>(ChaCha20Rng::from_seed([seed; 32]))
    })
}

struct Wrapped {
    evidence: SourceNodeEvidence,
    key: ProvingKey<Ep>,
    source: SourceVerifier,
    originals: super::super::tree::OriginalPair,
}

fn wrap<C: SourceCircuit>(
    circuit: &C,
    substitution: Option<&C>,
    exports: producer::Exports,
    pparams: &PinnedParams<Ep>,
    vparams: &PinnedParams<Eq>,
    seed: u8,
) -> Wrapped {
    let producer::Exports {
        frame,
        endpoints,
        pallas,
        parts,
    } = exports;
    let budget = MemoryBudget::DEFAULT;
    let public = vec![frame.to_vec()];
    let report = check_circuit(circuit, 16, &public, CheckMode::Strict).unwrap();
    assert!(report.is_satisfied(), "{:?}", report.failures().first());
    let mut config = KeygenConfigV2::pipa_r(vec![InstanceType::Bounded]);
    config.compress_selectors = false;
    let key = keygen_pk_v2(vparams, circuit, &config).unwrap();
    let proof = create_proof_owned_with_claim(
        vparams,
        &key,
        Witness::from_circuit(&key, circuit, &public).unwrap(),
        randomness(seed),
        ProverConfig::default(),
    )
    .unwrap();
    proof.opening.decide(vparams, budget).unwrap();
    verify_full(
        vparams,
        key.binding(),
        key.vk(),
        &public,
        &proof.proof,
        budget,
    )
    .unwrap();
    let opening = FoldInput::from_opening(*proof.opening.g(), proof.opening.challenges()).unwrap();
    let trivial = AccumulatorT::trivial(vparams, budget).unwrap();
    let (fold, vesta) = create_fold(
        vparams,
        &[
            parts[0].as_input(),
            opening,
            parts[1].as_input(),
            trivial.as_input(),
        ],
        Fq::from(u64::from(seed)).to_repr(),
        &FoldConfig::default(),
    )
    .unwrap();
    vesta.decide(vparams, budget).unwrap();
    let wrapper = OmegaCircuit::new(
        OmegaPlan::new(
            key.binding().clone(),
            vparams.clone(),
            vec![key.vk().kagemusha_digest(key.binding()).unwrap()],
        )
        .unwrap()
        .with_key_catalog(vec![key.vk().clone()])
        .unwrap(),
        OmegaWitness {
            key: key.vk().clone(),
            instances: frame.to_vec(),
            length: proof.proof.len().try_into().unwrap(),
            proof: proof.proof,
            fold: fold.to_bytes(),
        },
    )
    .unwrap();
    let evidence = SourceNodeEvidence {
        endpoints,
        proof: Vec::new(),
        pallas,
        vesta,
    };
    let public = evidence.instances().unwrap();
    let report = check_circuit(&wrapper, 16, &public, CheckMode::Strict).unwrap();
    assert!(report.is_satisfied(), "{:?}", report.failures().first());
    let source_descriptor = key.binding().encoded().to_vec();
    let source_verifying = key.vk().to_bytes().to_vec();
    let source_proving = key.artifact_bytes_v2().unwrap();
    drop(key);
    let key = keygen_pk_v2(
        pparams,
        &wrapper,
        &KeygenConfigV2::pipa_r(OmegaPlan::instance_types().to_vec()),
    )
    .unwrap();
    let wrapper_pk = key.artifact_bytes_v2().unwrap();
    let read = ReadConfig {
        maximum_bytes: source_proving.len().max(wrapper_pk.len()),
        maximum_rows: 1 << 16,
        coset_cache: CosetCachePolicy::OnDemand,
        msm_budget: budget,
    };
    let source = OriginalArtifact {
        descriptor: &source_descriptor,
        verifying_key: &source_verifying,
        proving_key: &source_proving,
    };
    let outer = OriginalArtifact {
        descriptor: key.binding().encoded(),
        verifying_key: key.vk().to_bytes(),
        proving_key: &wrapper_pk,
    };
    let cancelled = iroha_pasta::CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        producer::Prover::from_original_artifacts_cancellable(
            circuit,
            source,
            outer,
            pparams.clone(),
            vparams.clone(),
            read,
            Some(&cancelled),
        ),
        Err(producer::Error::Cancelled)
    ));
    let imported = producer::Prover::from_original_artifacts(
        circuit,
        source,
        outer,
        pparams.clone(),
        vparams.clone(),
        read,
    )
    .unwrap();
    assert_eq!(imported.binding(), key.binding());
    assert_eq!(imported.verifying_key().to_bytes(), key.vk().to_bytes());
    if let Some(substitution) = substitution {
        assert!(
            producer::Prover::from_original_artifacts(
                substitution,
                source,
                outer,
                pparams.clone(),
                vparams.clone(),
                read,
            )
            .is_err(),
            "different fixed source must fail original-key import"
        );
        assert!(
            imported
                .prove(
                    substitution,
                    [seed; 32],
                    randomness(seed),
                    randomness(seed + 1),
                    ProverConfig::default(),
                )
                .is_err(),
            "a proof witness cannot replace the imported fixed source"
        );
    }
    let evidence = imported
        .prove(
            circuit,
            Fq::from(u64::from(seed)).to_repr(),
            randomness(seed),
            randomness(seed + 1),
            ProverConfig::default(),
        )
        .unwrap();
    assert_eq!(evidence.instances().unwrap(), public);
    imported.verify_evidence(&evidence, budget).unwrap();
    let mut changed = evidence.clone();
    changed.endpoints[5] += Fp::ONE;
    assert!(imported.verify_evidence(&changed, budget).is_err());
    let mut changed = evidence.clone();
    changed.proof[0] ^= 1;
    assert!(imported.verify_evidence(&changed, budget).is_err());
    let mut changed = evidence.clone();
    changed.proof.push(0);
    assert!(imported.verify_evidence(&changed, budget).is_err());
    let source = imported.qualified_source().unwrap();
    Wrapped {
        evidence,
        originals: super::super::tree::OriginalPair {
            source: super::super::tree::OriginalBytes {
                descriptor: source_descriptor,
                verifying_key: source_verifying,
                proving_key: source_proving,
            },
            wrapper: super::super::tree::OriginalBytes {
                descriptor: key.binding().encoded().to_vec(),
                verifying_key: key.vk().to_bytes().to_vec(),
                proving_key: wrapper_pk,
            },
        },
        key,
        source,
    }
}

#[test]
#[ignore = "genuine k16 leaf/wrapper/merge/wrapper proofs and two-curve decisions"]
fn genuine_source_merge_retains_both_curves_and_rejects_substitution() {
    let pparams = PinnedParams::<Ep>::derive(16).unwrap();
    let vparams = PinnedParams::<Eq>::derive(16).unwrap();
    let budget = MemoryBudget::DEFAULT;
    let p = AccumulatorT::trivial(&pparams, budget).unwrap();
    let v = AccumulatorT::trivial(&vparams, budget).unwrap();
    let wrapped = [0u64, 1].map(|cursor| {
        let endpoints = [901, 902, cursor, cursor + 1, 10 + cursor, 11 + cursor].map(Fp::from);
        let circuit = Increment {
            endpoints,
            known: true,
            step: 1,
        };
        let wrong = Increment {
            step: 2,
            ..circuit.clone()
        };
        wrap(
            &circuit,
            Some(&wrong),
            producer::Exports {
                frame: leaf_frame_native(endpoints).unwrap(),
                endpoints,
                pallas: p.clone(),
                parts: [v.clone(), v.clone()],
            },
            &pparams,
            &vparams,
            10 + u8::try_from(cursor).unwrap() * 2,
        )
    });
    // Both leaves really use the same relation and exact key, despite different
    // private states, cursors, and proof randomness.
    assert_eq!(
        wrapped[0].key.vk().to_bytes(),
        wrapped[1].key.vk().to_bytes()
    );
    let plan = SourcePairPlan::new(wrapped.each_ref().map(|w| w.source.clone()), &pparams).unwrap();
    let children = wrapped.each_ref().map(|w| w.evidence.clone());
    let circuit = SourceMergeCircuit::prepare(
        plan.clone(),
        children.clone(),
        &vparams,
        Fp::from(911),
        &FoldConfig::default(),
    )
    .unwrap();
    let public = vec![circuit.instances().to_vec()];
    let blank = SourceMergeCircuit::for_source(plan.clone()).unwrap();
    let known = synthesize(&circuit, 16, Some(&public)).unwrap();
    let unknown = synthesize(&blank, 16, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(
        known.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
    drop((known, unknown));
    let report = check_circuit(&circuit, 16, &public, CheckMode::Strict).unwrap();
    assert!(report.is_satisfied(), "{:?}", report.failures().first());
    for (side, field) in [(0, 0), (0, 1), (0, 4), (1, 3), (1, 5)] {
        let mut forged = circuit.clone();
        forged.pair.children[side].endpoints[field] += Fp::ONE;
        assert!(
            !check_circuit(&forged, 16, &public, CheckMode::Strict).is_ok_and(|r| r.is_satisfied()),
            "substituted endpoint {side}/{field}"
        );
    }
    let mut forged = circuit.clone();
    forged.pair.fold[32] ^= 1;
    assert!(
        !check_circuit(&forged, 16, &public, CheckMode::Strict).is_ok_and(|r| r.is_satisfied()),
        "substituted four-obligation fold"
    );
    for mutation in 0..3 {
        let mut changed = children.clone();
        match mutation {
            0 => changed[1].endpoints[2] += Fp::ONE,
            1 => changed[0].proof[0] ^= 1,
            _ => changed.swap(0, 1),
        }
        assert!(
            SourceMergeCircuit::prepare(
                plan.clone(),
                changed,
                &vparams,
                Fp::from(911),
                &FoldConfig::default(),
            )
            .is_err()
        );
    }
    let result = wrap(
        &circuit,
        None,
        producer::Exports {
            frame: *circuit.instances(),
            endpoints: *circuit.endpoints(),
            pallas: circuit.pallas().clone(),
            parts: children.each_ref().map(|c| c.vesta.clone()),
        },
        &pparams,
        &vparams,
        20,
    );
    assert_eq!(
        result.evidence.endpoints,
        [901, 902, 0, 2, 10, 12].map(Fp::from)
    );
    result.evidence.pallas.decide(&pparams, budget).unwrap();
    result.evidence.vesta.decide(&vparams, budget).unwrap();
    // Exercise the production tree owner with the same original merge artifacts.
    // It must re-import the active tables and retain both curves; native topology
    // alone cannot produce this complete interval evidence.
    let originals = &result.originals;
    let total_bytes: usize = [&originals.source, &originals.wrapper]
        .into_iter()
        .map(|a| a.descriptor.len() + a.verifying_key.len() + a.proving_key.len())
        .sum();
    let import = TreeImportConfig {
        key: ReadConfig {
            maximum_bytes: originals
                .source
                .proving_key
                .len()
                .max(originals.wrapper.proving_key.len()),
            maximum_rows: 1 << 16,
            coset_cache: CosetCachePolicy::OnDemand,
            msm_budget: budget,
        },
        maximum_keys: 1,
        maximum_original_bytes: total_bytes,
    };
    let leaf_sources = wrapped.each_ref().map(|w| w.source.clone()).to_vec();
    for bounds in [
        TreeImportConfig {
            maximum_keys: 0,
            ..import
        },
        TreeImportConfig {
            maximum_original_bytes: total_bytes - 1,
            ..import
        },
    ] {
        assert!(
            IntervalTree::from_original_artifacts(
                leaf_sources.clone(),
                |_| Ok(originals.clone()),
                pparams.clone(),
                vparams.clone(),
                bounds
            )
            .is_err()
        );
    }
    let tree = IntervalTree::from_original_artifacts(
        leaf_sources,
        |_| Ok(originals.clone()),
        pparams.clone(),
        vparams.clone(),
        import,
    )
    .unwrap();
    assert_eq!(tree.leaf_count(), 2);
    assert_eq!(
        tree.qualified_source().key.to_bytes(),
        result.source.key.to_bytes()
    );
    assert!(
        tree.prove(
            Vec::new(),
            |_| panic!("missing leaves cannot load keys"),
            |_| panic!("missing leaves cannot draw entropy"),
            ProverConfig::default(),
            &FoldConfig::default(),
            None,
        )
        .is_err()
    );
    let tree_result = tree
        .prove(
            children.to_vec(),
            |_| Ok(originals.clone()),
            |_| {
                Ok(NodeRandomness {
                    inner_salt: Fp::from(919),
                    outer_salt: Fq::from(25).to_repr(),
                    source: randomness(25),
                    wrapper: randomness(26),
                })
            },
            ProverConfig::default(),
            &FoldConfig::default(),
            None,
        )
        .unwrap();
    assert_eq!(tree_result.endpoints, result.evidence.endpoints);
    tree_result.pallas.decide(&pparams, budget).unwrap();
    tree_result.vesta.decide(&vparams, budget).unwrap();
    eprintln!(
        "genuine source merge: k16, wrapper={} bytes, both carried claims decided",
        result.evidence.proof.len()
    );
}

#[test]
fn cancelled_leaf_exports_do_not_derive_or_judge_invalid_parameters() {
    let pallas = PinnedParams::derive(1).unwrap();
    let vesta = PinnedParams::derive(1).unwrap();
    let cancelled = iroha_pasta::CancellationToken::new();
    cancelled.cancel();
    let endpoints = [Fp::ONE; 6];
    assert!(matches!(
        producer::leaf_exports(
            endpoints,
            &pallas,
            &vesta,
            MemoryBudget::DEFAULT,
            Some(&cancelled)
        ),
        Err(producer::Error::Cancelled)
    ));
    assert!(matches!(
        producer::leaf_exports(endpoints, &pallas, &vesta, MemoryBudget::DEFAULT, None),
        Err(producer::Error::Proof)
    ));
}
