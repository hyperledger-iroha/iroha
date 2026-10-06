//! Genuine two-curve source composition and adversarial endpoint binding.

use super::*;
use crate::omega::{OmegaCircuit, OmegaWitness};
use iroha_pasta::msm::MemoryBudget;
use iroha_plonk::{
    ProverConfig, ProverRandomness, ProvingKey, Witness,
    check::{CheckMode, check_circuit},
    create_proof_owned_with_claim,
    cs::InstanceType,
    keys::{KeygenConfigV2, keygen_pk_v2},
};
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};

// This deliberately small source program increments both its cursor and state.
// It qualifies composition only, and does not assert validator finality.
#[derive(Clone)]
struct Increment {
    endpoints: [Fp; 6],
    known: bool,
}
impl Circuit<Fp> for Increment {
    type Config = SourceMergeConfig;
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
                        Fp::ONE,
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
}

fn wrap<C: Circuit<Fp>>(
    circuit: &C,
    frame: [Fp; 69],
    endpoints: [Fp; 6],
    pallas: AccumulatorT<Ep>,
    parts: [AccumulatorT<Eq>; 2],
    pparams: &PinnedParams<Ep>,
    vparams: &PinnedParams<Eq>,
    seed: u8,
) -> Wrapped {
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
    let mut evidence = SourceNodeEvidence {
        endpoints,
        proof: Vec::new(),
        pallas,
        vesta,
    };
    let public = evidence.instances().unwrap();
    let report = check_circuit(&wrapper, 16, &public, CheckMode::Strict).unwrap();
    assert!(report.is_satisfied(), "{:?}", report.failures().first());
    let key = keygen_pk_v2(
        pparams,
        &wrapper,
        &KeygenConfigV2::pipa_r(OmegaPlan::instance_types().to_vec()),
    )
    .unwrap();
    let output = create_proof_owned_with_claim(
        pparams,
        &key,
        Witness::from_circuit(&key, &wrapper, &public).unwrap(),
        randomness(seed + 1),
        ProverConfig::default(),
    )
    .unwrap();
    output.opening.decide(pparams, budget).unwrap();
    verify_full(
        pparams,
        key.binding(),
        key.vk(),
        &public,
        &output.proof,
        budget,
    )
    .unwrap();
    evidence.proof = output.proof;
    Wrapped { evidence, key }
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
        wrap(
            &Increment {
                endpoints,
                known: true,
            },
            leaf_frame_native(endpoints).unwrap(),
            endpoints,
            p.clone(),
            [v.clone(), v.clone()],
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
    let plan = SourceMergePlan::new(
        wrapped.each_ref().map(|w| {
            (
                VerifierPlan::new(w.key.binding().clone(), pparams.clone()).unwrap(),
                w.key.vk().clone(),
            )
        }),
        &pparams,
    )
    .unwrap();
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
    let report = check_circuit(&circuit, 16, &public, CheckMode::Strict).unwrap();
    assert!(report.is_satisfied(), "{:?}", report.failures().first());
    for (side, field) in [(0, 0), (0, 1), (0, 4), (1, 3), (1, 5)] {
        let mut forged = circuit.clone();
        forged.children[side].endpoints[field] += Fp::ONE;
        assert!(
            !check_circuit(&forged, 16, &public, CheckMode::Strict).is_ok_and(|r| r.is_satisfied()),
            "substituted endpoint {side}/{field}"
        );
    }
    let mut forged = circuit.clone();
    forged.fold[32] ^= 1;
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
        *circuit.instances(),
        *circuit.endpoints(),
        circuit.pallas().clone(),
        children.each_ref().map(|c| c.vesta.clone()),
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
    eprintln!(
        "genuine source merge: k16, wrapper={} bytes, both carried claims decided",
        result.evidence.proof.len()
    );
}
