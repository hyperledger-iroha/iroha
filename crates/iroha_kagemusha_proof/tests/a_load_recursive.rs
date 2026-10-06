//! Genuine rooted Bootstrap predecessor and Load recursive partition inventory.
//! A passing diagnostic is not a final Load lineage acceptance or size gate.

// Both included suites also run independently; each keeps its own deterministic
// helper/cache module when composed into this complete-chain test.
#![allow(clippy::duplicate_mod)]

#[path = "common/bootstrap.rs"]
mod bootstrap;
#[path = "common/bootstrap_objects.rs"]
#[allow(dead_code)]
mod bootstrap_objects;
/// Shared genuine outer-proof fixture, including its component regressions.
#[path = "bootstrap_omega.rs"]
pub mod bootstrap_outer;
mod common;
#[path = "common/consuming_proof.rs"]
mod consuming_proof;
/// Shared genuine Load sigma, map and signed-object fixtures.
#[path = "a_load.rs"]
pub mod load_components;
#[path = "common/load_objects.rs"]
#[allow(dead_code)]
mod load_objects;

use ff::{Field, PrimeField};
use iroha_kagemusha_proof::{
    a_relation::{
        AProofPlan, ProofMessageCells, QProofPlan, SigmaBindingCells, VestaClaimCells,
        context::{ContextInputs, ContextPlan, ContextPredecessor, ContextState},
        load::{LoadInputs, LoadObjects, LoadStagePlan},
        schedule::OperationTask,
        split::close_first,
        verify_predecessor, verify_sigma,
    },
    admin_sigma::LoadCircuit,
    operation_relation::{map_effects::MapState, statement::StatementCells},
    q_sigma::{QSigmaPlan, SigmaClass, SigmaSlotWitness, native::QSigmaProver},
};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaAffine, msm::MemoryBudget, poseidon::hash_with_domain};
use iroha_plonk::{
    ProverConfig, VerifyingKey, Witness,
    check::{CheckMode, check},
    create_proof_owned_with_claim,
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value, synthesize},
    keys::{KeygenConfigV2, keygen_pk_v2},
    pcs::ipa::PinnedParams,
    verifier::accumulate_generator,
};
use iroha_plonk_gadgets::{
    bytes::{
        element::le_message_segments,
        tape::{BytesChip, BytesConfig, SegmentSpec},
    },
    statement::foreign_limbs,
};
use iroha_plonk_recursion::{
    AccumulatorT, FoldConfig, FoldInput,
    accumulation_circuit::FoldInputCells,
    codec::ScalarCells,
    create_fold,
    obligation::ledger::Variant,
    verifier::{VerifierChip, VerifierConfig, VerifierPlan},
};
use std::sync::Arc;

#[derive(Clone)]
struct Predecessor {
    key: VerifyingKey<Ep>,
    program: VerifierPlan<Ep>,
    proof: Vec<u8>,
    pallas: AccumulatorT<Ep>,
    opening: FoldInput<Ep>,
    vesta: AccumulatorT<Eq>,
}
#[derive(Clone)]
struct Sources {
    maps: load_components::LoadMaps,
    sigma: Vec<u8>,
    plan: AProofPlan,
    q_instances: Vec<Vec<Vec<Fq>>>,
    q_proofs: Vec<Vec<u8>>,
    q_openings: Vec<FoldInput<Ep>>,
    signature_schema: Vec<iroha_kagemusha_proof::q_signature::QSignaturePlan>,
    part: FoldInput<Eq>,
    predecessor: Predecessor,
    params: PinnedParams<Ep>,
}
fn sources(rooted: &bootstrap_outer::RootedBootstrapOmega, q_buses: Option<usize>) -> Sources {
    let (maps, sigma) = load_components::fixture_from(&rooted.source.state);
    let leaf = LoadCircuit::new(&maps.witness);
    let leaf_params = common::vesta_params(12);
    let leaf_key = keygen_pk_v2(
        &leaf_params,
        &leaf,
        &KeygenConfigV2::pipa_r(LoadCircuit::instance_types().to_vec()),
    )
    .unwrap();
    let vparams = common::vesta_params(16);
    let params = PinnedParams::<Ep>::derive(16).unwrap();
    let sigma_plan = QSigmaPlan::new(
        SigmaClass::new(
            VerifierPlan::new(leaf_key.binding().clone(), leaf_params).unwrap(),
            vec![(
                iroha_kagemusha_proof::a_relation::schedule::sigma_selector(2, 0).unwrap(),
                leaf_key.vk().kagemusha_digest(leaf_key.binding()).unwrap(),
            )],
        )
        .unwrap(),
        None,
        &vparams,
    )
    .unwrap();
    let prepared = sigma_plan
        .prepare(
            SigmaSlotWitness {
                key: leaf_key.vk().clone(),
                statement: leaf.instances()[0][0],
                length: sigma.len().try_into().unwrap(),
                proof: sigma.clone(),
            },
            None,
            &vparams,
            Fq::from(71),
            &FoldConfig::default(),
        )
        .unwrap();
    let part = prepared.part().clone();
    let q = q_buses
        .map_or_else(
            || QSigmaProver::keygen(&prepared, params.clone()),
            |buses| QSigmaProver::keygen_serialized_foreign(&prepared, params.clone(), buses),
        )
        .unwrap();
    let qproof = q
        .prove(&prepared, common::recovery(193), ProverConfig::default())
        .unwrap();
    let qclaim = accumulate_generator(
        &params,
        q.binding(),
        q.verifying_key(),
        &qproof.instances,
        &qproof.bytes,
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    qclaim.decide(&params, MemoryBudget::DEFAULT).unwrap();
    let sigma_q = QProofPlan::new(
        VerifierPlan::new(q.binding().clone(), params.clone()).unwrap(),
        q.verifying_key().clone(),
    )
    .unwrap();
    let (signature, signature_instances) =
        load_components::bootstrap_objects::signatures(&core::array::from_fn(|i| {
            maps.objects[i].clone()
        }));
    let signature_key = keygen_pk_v2(
        &params,
        &signature,
        &KeygenConfigV2::pipa_r(
            iroha_kagemusha_proof::q_signature::QSignaturePlan::instance_types().to_vec(),
        ),
    )
    .unwrap();
    let signature_proof = create_proof_owned_with_claim(
        &params,
        &signature_key,
        Witness::from_circuit(&signature_key, &signature, &signature_instances).unwrap(),
        common::recovery(194),
        ProverConfig::default(),
    )
    .unwrap();
    signature_proof
        .opening
        .decide(&params, MemoryBudget::DEFAULT)
        .unwrap();
    let signature_q = QProofPlan::new(
        VerifierPlan::new(signature_key.binding().clone(), params.clone()).unwrap(),
        signature_key.vk().clone(),
    )
    .unwrap();
    let (current, current_instances) =
        load_objects::current_signatures(&[maps.objects[4].signature, maps.objects[3].signature]);
    let current_key = keygen_pk_v2(
        &params,
        &current,
        &KeygenConfigV2::pipa_r(
            iroha_kagemusha_proof::q_signature::QSignaturePlan::instance_types().to_vec(),
        ),
    )
    .unwrap();
    let current_proof = create_proof_owned_with_claim(
        &params,
        &current_key,
        Witness::from_circuit(&current_key, &current, &current_instances).unwrap(),
        common::recovery(195),
        ProverConfig::default(),
    )
    .unwrap();
    current_proof
        .opening
        .decide(&params, MemoryBudget::DEFAULT)
        .unwrap();
    let current_q = QProofPlan::new(
        VerifierPlan::new(current_key.binding().clone(), params.clone()).unwrap(),
        current_key.vk().clone(),
    )
    .unwrap();
    let predecessor = Predecessor {
        key: rooted.key.clone(),
        program: VerifierPlan::new(rooted.binding.clone(), params.clone()).unwrap(),
        proof: rooted.proof.clone(),
        pallas: rooted.source.pallas.clone(),
        opening: rooted.opening.clone(),
        vesta: rooted.vesta.clone(),
    };
    let plan = AProofPlan::new(
        Variant::Load,
        sigma_plan,
        vec![sigma_q, signature_q, current_q],
        Some(predecessor.program.clone()),
        &params,
    )
    .unwrap();
    for (name, groups, pred) in [
        ("missing Q", vec![vec![], vec![0], vec![]], Some(0)),
        ("duplicate Q", vec![vec![], vec![0], vec![0, 1]], Some(0)),
        ("reordered Q", vec![vec![], vec![1, 0], vec![]], Some(0)),
        ("unknown Q", vec![vec![], vec![0], vec![2]], Some(0)),
        (
            "wrong predecessor stage",
            vec![vec![], vec![0], vec![1]],
            Some(3),
        ),
        ("dropped predecessor", vec![vec![], vec![0], vec![1]], None),
        (
            "empty first obligation",
            vec![vec![], vec![0], vec![1]],
            Some(1),
        ),
        ("missing continuation", vec![vec![0, 1]], Some(0)),
    ] {
        assert!(
            ContextPlan::with_schedule(
                plan.clone(),
                groups,
                pred,
                LoadObjects::context_specs().unwrap().to_vec()
            )
            .is_err(),
            "{name}"
        );
    }
    Sources {
        maps,
        sigma,
        plan,
        q_instances: vec![
            qproof.instances,
            signature_instances.to_vec(),
            current_instances.to_vec(),
        ],
        q_proofs: vec![qproof.bytes, signature_proof.proof, current_proof.proof],
        q_openings: vec![
            FoldInput::from_opening(*qclaim.g(), qclaim.challenges()).unwrap(),
            FoldInput::from_opening(
                *signature_proof.opening.g(),
                signature_proof.opening.challenges(),
            )
            .unwrap(),
            FoldInput::from_opening(
                *current_proof.opening.g(),
                current_proof.opening.challenges(),
            )
            .unwrap(),
        ],
        signature_schema: vec![signature.plan().clone(), current.plan().clone()],
        part,
        predecessor,
        params,
    }
}
#[derive(Clone)]
struct First {
    source: Arc<Sources>,
    range_buses: usize,
    plan: ContextPlan,
    pallas: AccumulatorT<Ep>,
    fold: Vec<u8>,
    known: bool,
}
#[derive(Clone, Debug)]
struct Config {
    verifier: VerifierConfig<Ep>,
    bytes: BytesConfig,
    public: Column<Instance>,
}
impl First {
    fn value<T: Copy>(&self, value: T) -> Value<T> {
        if self.known {
            Value::known(value)
        } else {
            Value::unknown()
        }
    }
    fn scalar(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        v: Fq,
    ) -> Result<ScalarCells<Ep>, Error> {
        let [lo, hi] = foreign_limbs(&v);
        let lo = chip.uint().assign::<128>(region, self.value(lo))?;
        let hi = chip.uint().assign::<127>(region, self.value(hi))?;
        ScalarCells::from_limbs(&mut chip.uint(), region, &lo, &hi)
    }
    fn pallas(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        input: &FoldInput<Ep>,
    ) -> Result<FoldInputCells<Ep>, Error> {
        let g = chip.witness_point(region, self.value(Ep::from(*input.g())))?;
        let challenges = input
            .challenges()
            .iter()
            .map(|v| self.scalar(chip, region, *v))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        FoldInputCells::from_normalized(chip, region, 16, g, challenges)
    }
    fn vesta(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        input: &AccumulatorT<Eq>,
    ) -> Result<VestaClaimCells, Error> {
        let (x, y) = Option::from(input.g().coordinates()).ok_or(Error::Synthesis)?;
        let coordinates = [self.scalar(chip, region, x)?, self.scalar(chip, region, y)?];
        let challenges = chip
            .uint()
            .glue()
            .witnesses(region, &input.challenges().map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        VestaClaimCells::constrain(chip, region, 16, coordinates, challenges)
    }
    fn carrier(
        &self,
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
        body: &[u8],
    ) -> Result<ProofMessageCells, Error> {
        let length = u32::try_from(body.len()).map_err(|_| Error::BoundsFailure)?;
        let source = length
            .to_le_bytes()
            .into_iter()
            .chain(body.iter().copied())
            .map(|v| self.value(v))
            .collect::<Vec<_>>();
        let mut segments = vec![SegmentSpec::little(0, 4)];
        segments.extend(le_message_segments(4, body.len() / 32));
        let run = bytes.run(
            region,
            &source,
            &source
                .chunks(31)
                .map(<[Value<u8>]>::len)
                .collect::<Vec<_>>(),
            &segments,
        )?;
        ProofMessageCells::from_run(chip, region, &run, 0, body.len())
    }
    fn sigma(
        &self,
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
        statement: &StatementCells,
    ) -> Result<SigmaBindingCells, Error> {
        let length = u32::try_from(self.source.sigma.len()).map_err(|_| Error::BoundsFailure)?;
        let source = length
            .to_le_bytes()
            .into_iter()
            .chain(self.source.sigma.iter().copied())
            .map(|v| self.value(v))
            .collect::<Vec<_>>();
        let run = bytes.run(
            region,
            &source,
            &source
                .chunks(31)
                .map(<[Value<u8>]>::len)
                .collect::<Vec<_>>(),
            &[SegmentSpec::little(0, 4)],
        )?;
        let index = chip.uint().glue().constant(
            region,
            Fp::from(u64::from(
                iroha_kagemusha_proof::a_relation::schedule::sigma_selector(2, 0).unwrap(),
            )),
        )?;
        SigmaBindingCells::from_run(chip, region, statement, index, &run)
    }
}
impl Circuit<Fp> for First {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = usize;
    fn params(&self) -> usize {
        self.range_buses
    }
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Config {
        Self::configure_with_params(meta, 0)
    }
    fn configure_with_params(meta: &mut ConstraintSystem<Fp>, range_buses: usize) -> Config {
        let verifier = if range_buses == 0 {
            VerifierConfig::configure(meta)
        } else {
            VerifierConfig::configure_serialized_foreign(meta, range_buses).unwrap()
        };
        let a = meta.advice_column();
        let b = meta.advice_column();
        let bytes = BytesConfig::configure(meta, a, b);
        let public = meta.instance_column(69);
        meta.enable_equality(public);
        Config {
            verifier,
            bytes,
            public,
        }
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let out = layouter.assign_region(
            || "genuine Load predecessor-first relation",
            |mut region| {
                let mut maps = self.source.maps.clone();
                maps.known = self.known;
                let (old, pred_public) =
                    maps.state(&mut chip, &mut region, &maps.witness.predecessor)?;
                let (new, next_public) =
                    maps.state(&mut chip, &mut region, &maps.witness.successor)?;
                let fields = chip
                    .uint()
                    .glue()
                    .witnesses(&mut region, &maps.witness.statement.map(|v| self.value(v)))?
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                let statement = StatementCells::constrain_with_verifier(
                    &mut chip,
                    &mut region,
                    Variant::Load,
                    &fields,
                )?;
                let sigma = self.sigma(&mut chip, &mut bytes, &mut region, &statement)?;
                let objects = maps
                    .objects
                    .each_ref()
                    .map(|o| o.bytes.iter().map(|v| self.value(*v)).collect::<Vec<_>>());
                let objects = LoadObjects::decode(
                    &mut chip,
                    &mut bytes,
                    &mut region,
                    objects.each_ref().map(Vec::as_slice),
                )?;
                let insertion = maps.insertion(&mut chip.uint(), &mut region)?;
                LoadStagePlan::new(self.plan.clone())?.constrain_stage(
                    &mut chip,
                    &mut region,
                    0,
                    &objects,
                    load_objects::policy(),
                    LoadInputs {
                        predecessor: MapState {
                            state: &old,
                            lineage: &pred_public,
                        },
                        successor: MapState {
                            state: &new,
                            lineage: &next_public,
                        },
                        sigma: &sigma,
                        signatures: &[],
                    },
                    Some(&insertion),
                    None,
                )?;
                let q_instances = self
                    .source
                    .q_instances
                    .iter()
                    .map(|cols| {
                        cols.iter()
                            .map(|column| {
                                column
                                    .iter()
                                    .map(|v| self.scalar(&mut chip, &mut region, *v))
                                    .collect()
                            })
                            .collect()
                    })
                    .collect::<Result<Vec<Vec<Vec<_>>>, _>>()?;
                let pp = self.pallas(
                    &mut chip,
                    &mut region,
                    &self.source.predecessor.pallas.as_input(),
                )?;
                let pv = self.vesta(&mut chip, &mut region, &self.source.predecessor.vesta)?;
                let input = ContextInputs {
                    own_statement: &statement,
                    incoming_statement: None,
                    predecessor: Some(ContextPredecessor {
                        state: &old,
                        public: &pred_public,
                        pallas: &pp,
                        vesta: &pv,
                    }),
                    successor: ContextState {
                        state: &new,
                        public: &next_public,
                    },
                    incoming: None,
                    q_instances: &q_instances,
                    objects: objects.context(),
                    modes: &[],
                    pallas_corrections: &[],
                    vesta_corrections: &[],
                    receive_results: None,
                };
                let vk = &self.source.predecessor.key;
                let iroha_plonk::transcript::TranscriptRepr::Base(repr) = *vk.transcript_repr()
                else {
                    return Err(Error::Synthesis);
                };
                let fixed = vk
                    .fixed_commitments()
                    .iter()
                    .map(|p| self.value(Ep::from(*p)))
                    .collect::<Vec<_>>();
                let permutation = vk
                    .permutation_commitments()
                    .iter()
                    .map(|p| self.value(Ep::from(*p)))
                    .collect::<Vec<_>>();
                let key = chip.witness_key(&mut region, self.value(repr), &fixed, &permutation)?;
                let proof = self.carrier(
                    &mut chip,
                    &mut bytes,
                    &mut region,
                    &self.source.predecessor.proof,
                )?;
                let pred = verify_predecessor(
                    &mut chip,
                    &mut region,
                    self.plan.operation(),
                    &key,
                    &pred_public,
                    &next_public,
                    &pp,
                    &pv,
                    &proof,
                )?;
                let mut verified = Vec::new();
                for (index, columns) in q_instances
                    .iter()
                    .enumerate()
                    .take(self.plan.first_q_count())
                {
                    let proof = self.carrier(
                        &mut chip,
                        &mut bytes,
                        &mut region,
                        &self.source.q_proofs[index],
                    )?;
                    let (q, _) = verify_sigma(
                        &mut chip,
                        &mut region,
                        self.plan.operation(),
                        columns,
                        &proof,
                        core::slice::from_ref(&sigma),
                    )?;
                    verified.push(q);
                }
                let fold = self.carrier(&mut chip, &mut bytes, &mut region, &self.fold)?;
                let first = close_first(
                    &mut chip,
                    &mut region,
                    &self.plan,
                    &input,
                    Some(&pred),
                    &verified,
                    core::slice::from_ref(&sigma),
                    Some(&fold),
                    &self.source.params,
                )?;
                first.words(&mut chip, &mut region)
            },
        )?;
        for (row, word) in out.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, row)?;
        }
        Ok(())
    }
}
#[derive(Clone)]
struct Authorization {
    first: First,
    current: bool,
    context_mutation: u8,
    order: [usize; 3],
    wrong_policy: bool,
}
impl Circuit<Fp> for Authorization {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            first: self.first.without_witnesses(),
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Config {
        let verifier = VerifierConfig::configure(meta);
        let a = meta.advice_column();
        let b = meta.advice_column();
        let bytes = BytesConfig::configure(meta, a, b);
        let public = meta.instance_column(5);
        meta.enable_equality(public);
        Config {
            verifier,
            bytes,
            public,
        }
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let first = &self.first;
        let q_index = if self.current { 2 } else { 1 };
        let mut chip = VerifierChip::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let out = layouter.assign_region(
            || "actual Load Q signature to objects",
            |mut region| {
                let mut maps = first.source.maps.clone();
                maps.known = first.known;
                let (old, pred) = maps.state(&mut chip, &mut region, &maps.witness.predecessor)?;
                let (new, next) = maps.state(&mut chip, &mut region, &maps.witness.successor)?;
                let fields = chip
                    .uint()
                    .glue()
                    .witnesses(&mut region, &maps.witness.statement.map(|v| first.value(v)))?
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                let statement = StatementCells::constrain_with_verifier(
                    &mut chip,
                    &mut region,
                    Variant::Load,
                    &fields,
                )?;
                let sigma = first.sigma(&mut chip, &mut bytes, &mut region, &statement)?;
                let source = maps
                    .objects
                    .each_ref()
                    .map(|o| o.bytes.iter().map(|v| first.value(*v)).collect::<Vec<_>>());
                let objects = LoadObjects::decode(
                    &mut chip,
                    &mut bytes,
                    &mut region,
                    source.each_ref().map(Vec::as_slice),
                )?;
                let instances = first.source.q_instances[q_index]
                    .iter()
                    .map(|column| {
                        column
                            .iter()
                            .map(|v| first.scalar(&mut chip, &mut region, *v))
                            .collect()
                    })
                    .collect::<Result<Vec<Vec<_>>, _>>()?;
                let proof = first.carrier(
                    &mut chip,
                    &mut bytes,
                    &mut region,
                    &first.source.q_proofs[q_index],
                )?;
                let q = iroha_kagemusha_proof::a_relation::verify_q(
                    &mut chip,
                    &mut region,
                    first.plan.operation(),
                    q_index,
                    &instances,
                    &proof,
                )?;
                let slots = iroha_kagemusha_proof::a_relation::bind_signature_q(
                    &mut chip,
                    &mut region,
                    first.plan.operation(),
                    q_index,
                    &first.source.signature_schema[q_index - 1],
                    &q,
                )?;
                let mut exact_instances = instances.clone();
                if self.context_mutation == 1 {
                    exact_instances[0][0] = first.scalar(
                        &mut chip,
                        &mut region,
                        first.source.q_instances[q_index][0][0] + Fq::ONE,
                    )?;
                } else if self.context_mutation == 3 {
                    exact_instances[0].pop();
                }
                slots.bind_context(
                    &mut region,
                    first.plan.operation(),
                    if self.context_mutation == 2 {
                        3 - q_index
                    } else {
                        q_index
                    },
                    &exact_instances,
                )?;
                let slots = self
                    .order
                    .iter()
                    .take(if self.current { 2 } else { 3 })
                    .map(|i| slots.slots()[*i].clone())
                    .collect::<Vec<_>>();
                let policy = if self.wrong_policy {
                    iroha_kagemusha_proof::a_relation::own::OwnPolicy::new(
                        [1, 2],
                        [31, 32],
                        iroha_plonk_gadgets::p256::native::Affine::GENERATOR,
                    )?
                } else {
                    load_objects::policy()
                };
                let input = LoadInputs {
                    predecessor: MapState {
                        state: &old,
                        lineage: &pred,
                    },
                    successor: MapState {
                        state: &new,
                        lineage: &next,
                    },
                    sigma: &sigma,
                    signatures: &slots,
                };
                if self.current {
                    objects.authenticate_current(&mut chip, &mut region, policy, input)?;
                } else {
                    objects.authenticate(&mut chip, &mut region, policy, input)?;
                }
                Ok(objects
                    .context()
                    .iter()
                    .map(|o| o.authenticated_digest().clone())
                    .collect::<Vec<_>>())
            },
        )?;
        for (row, word) in out.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, row)?;
        }
        Ok(())
    }
}
#[derive(Clone)]
struct Continuation {
    first: First,
    plan: iroha_kagemusha_proof::a_relation::split::SplitPlan,
    wrapper: Vec<u8>,
    vesta: AccumulatorT<Eq>,
    fold: Vec<u8>,
    pallas: AccumulatorT<Ep>,
    carried: AccumulatorT<Ep>,
    history: Vec<(AccumulatorT<Ep>, AccumulatorT<Eq>)>,
    omit_q: Option<usize>,
}
impl Circuit<Fp> for Continuation {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = usize;
    fn params(&self) -> usize {
        self.first.range_buses
    }
    fn without_witnesses(&self) -> Self {
        Self {
            first: self.first.without_witnesses(),
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Config {
        First::configure(meta)
    }
    fn configure_with_params(meta: &mut ConstraintSystem<Fp>, range_buses: usize) -> Config {
        First::configure_with_params(meta, range_buses)
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let first = &self.first;
        let mut chip = VerifierChip::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let out = layouter.assign_region(
            || "Load deferred Q continuation",
            |mut region| {
                let mut maps = first.source.maps.clone();
                maps.known = first.known;
                let (old, pred_public) =
                    maps.state(&mut chip, &mut region, &maps.witness.predecessor)?;
                let (new, next_public) =
                    maps.state(&mut chip, &mut region, &maps.witness.successor)?;
                let fields = chip
                    .uint()
                    .glue()
                    .witnesses(&mut region, &maps.witness.statement.map(|v| first.value(v)))?
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                let statement = StatementCells::constrain_with_verifier(
                    &mut chip,
                    &mut region,
                    Variant::Load,
                    &fields,
                )?;
                let sigma = first.sigma(&mut chip, &mut bytes, &mut region, &statement)?;
                let sources = maps
                    .objects
                    .each_ref()
                    .map(|o| o.bytes.iter().map(|v| first.value(*v)).collect::<Vec<_>>());
                let objects = LoadObjects::decode(
                    &mut chip,
                    &mut bytes,
                    &mut region,
                    sources.each_ref().map(Vec::as_slice),
                )?;
                let q_instances = first
                    .source
                    .q_instances
                    .iter()
                    .map(|cols| {
                        cols.iter()
                            .map(|column| {
                                column
                                    .iter()
                                    .map(|v| first.scalar(&mut chip, &mut region, *v))
                                    .collect()
                            })
                            .collect()
                    })
                    .collect::<Result<Vec<Vec<Vec<_>>>, _>>()?;
                let pp = first.pallas(
                    &mut chip,
                    &mut region,
                    &first.source.predecessor.pallas.as_input(),
                )?;
                let pv = first.vesta(&mut chip, &mut region, &first.source.predecessor.vesta)?;
                let input = ContextInputs {
                    own_statement: &statement,
                    incoming_statement: None,
                    predecessor: Some(ContextPredecessor {
                        state: &old,
                        public: &pred_public,
                        pallas: &pp,
                        vesta: &pv,
                    }),
                    successor: ContextState {
                        state: &new,
                        public: &next_public,
                    },
                    incoming: None,
                    q_instances: &q_instances,
                    objects: objects.context(),
                    modes: &[],
                    pallas_corrections: &[],
                    vesta_corrections: &[],
                    receive_results: None,
                };
                let cp = first.pallas(&mut chip, &mut region, &self.carried.as_input())?;
                let history = self
                    .history
                    .iter()
                    .map(|(p, v)| {
                        Ok(iroha_kagemusha_proof::a_relation::split::ContextLinkCells {
                            pallas: first.pallas(&mut chip, &mut region, &p.as_input())?,
                            vesta: first.vesta(&mut chip, &mut region, v)?,
                        })
                    })
                    .collect::<Result<Vec<_>, Error>>()?;
                let cv = first.vesta(&mut chip, &mut region, &self.vesta)?;
                let proof = first.carrier(&mut chip, &mut bytes, &mut region, &self.wrapper)?;
                let resumed = iroha_kagemusha_proof::a_relation::split::resume_context(
                    &mut chip,
                    &mut region,
                    &self.plan,
                    &input,
                    &history,
                    &cp,
                    &cv,
                    &proof,
                    core::slice::from_ref(&sigma),
                )?;
                let mut verified = Vec::new();
                for index in first
                    .plan
                    .q_partition(self.plan.stage())
                    .ok_or(Error::Synthesis)?
                    .iter()
                    .copied()
                {
                    let columns = &q_instances[index];
                    if self.omit_q == Some(index) {
                        continue;
                    }
                    let proof = first.carrier(
                        &mut chip,
                        &mut bytes,
                        &mut region,
                        &first.source.q_proofs[index],
                    )?;
                    let q = if index == 0 {
                        verify_sigma(
                            &mut chip,
                            &mut region,
                            first.plan.operation(),
                            columns,
                            &proof,
                            core::slice::from_ref(&sigma),
                        )?
                        .0
                    } else {
                        let q = iroha_kagemusha_proof::a_relation::verify_q(
                            &mut chip,
                            &mut region,
                            first.plan.operation(),
                            index,
                            columns,
                            &proof,
                        )?;
                        let slots = iroha_kagemusha_proof::a_relation::bind_signature_q(
                            &mut chip,
                            &mut region,
                            first.plan.operation(),
                            index,
                            &first.source.signature_schema[index - 1],
                            &q,
                        )?;
                        LoadStagePlan::new(first.plan.clone())?.constrain_stage(
                            &mut chip,
                            &mut region,
                            self.plan.stage(),
                            &objects,
                            load_objects::policy(),
                            LoadInputs {
                                predecessor: MapState {
                                    state: &old,
                                    lineage: &pred_public,
                                },
                                successor: MapState {
                                    state: &new,
                                    lineage: &next_public,
                                },
                                sigma: &sigma,
                                signatures: slots.slots(),
                            },
                            None,
                            Some(iroha_kagemusha_proof::a_relation::SignatureQContext {
                                bundle: &slots,
                                instances: columns,
                            }),
                        )?;
                        slots.verified().clone()
                    };
                    verified.push(q);
                }
                let fold = first.carrier(&mut chip, &mut bytes, &mut region, &self.fold)?;
                let closed = iroha_kagemusha_proof::a_relation::split::close_stage(
                    &mut chip,
                    &mut region,
                    &self.plan,
                    &resumed,
                    None,
                    None,
                    None,
                    &verified,
                    &fold,
                )?;
                if self.plan.is_terminal() {
                    closed.words(&mut chip, &mut region, &next_public)
                } else {
                    closed.continuation()?.words(&mut chip, &mut region)
                }
            },
        )?;
        for (row, word) in out.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, row)?;
        }
        Ok(())
    }
}
impl Continuation {
    fn public(&self) -> Vec<Vec<Fp>> {
        if !self.plan.is_terminal() {
            let mut digest = context_digest(&self.first);
            for (index, (p, v)) in self.history.iter().enumerate() {
                let next = self
                    .history
                    .get(index + 1)
                    .map_or(&self.carried, |(p, _)| p);
                digest = continued_digest(&self.first.plan, index + 1, digest, p, v, next);
            }
            digest = continued_digest(
                &self.first.plan,
                self.plan.stage(),
                digest,
                &self.carried,
                &self.vesta,
                &self.pallas,
            );
            return internal_public(digest, &self.vesta.as_input());
        }
        let mut lineage = self.first.source.maps.witness.successor.lineage.to_vec();
        let (x, y) = self.pallas.g().coordinates().unwrap();
        lineage.extend([x, y]);
        for u in self.pallas.challenges() {
            lineage.extend(foreign_limbs(u).map(Fp::from_u128));
        }
        let digest = hash_with_domain(u64::from_le_bytes(*b"kgwomg_1"), &lineage);
        let mut out = vec![digest, Fp::from(16)];
        out.extend(vesta_words(&self.vesta.as_input()));
        out.extend(vesta_words(&self.first.source.predecessor.vesta.as_input()));
        let trivial =
            AccumulatorT::trivial(&common::vesta_params(16), MemoryBudget::DEFAULT).unwrap();
        let words = vesta_words(&trivial.as_input());
        out.extend(&words);
        out.extend([Fp::ZERO, Fp::ONE, Fp::ZERO]);
        out.extend(&words[..4]);
        assert_eq!(out.len(), 69);
        vec![out]
    }
}
fn continued_digest(
    plan: &ContextPlan,
    stage: usize,
    previous: Fp,
    pallas: &AccumulatorT<Ep>,
    vesta: &AccumulatorT<Eq>,
    current: &AccumulatorT<Ep>,
) -> Fp {
    let mut words = vec![
        Fp::ONE,
        plan.schema()[1],
        Fp::from(u64::try_from(stage + 1).unwrap()),
        previous,
    ];
    push_pallas(&mut words, &pallas.as_input());
    words.extend(vesta_words(&vesta.as_input()));
    push_pallas(&mut words, &current.as_input());
    hash_with_domain(u64::from_le_bytes(*b"kgwctx_1"), &words)
}
fn internal_public(digest: Fp, part: &FoldInput<Eq>) -> Vec<Vec<Fp>> {
    let mut words = vec![digest, Fp::from(u64::from(part.source_k()))];
    words.extend(vesta_words(part));
    let trivial =
        AccumulatorT::<Eq>::trivial(&common::vesta_params(16), MemoryBudget::DEFAULT).unwrap();
    let trivial = vesta_words(&trivial.as_input());
    words.extend(&trivial);
    words.extend(&trivial);
    words.extend([Fp::ZERO, Fp::ONE, Fp::ZERO]);
    words.extend(&trivial[..4]);
    assert_eq!(words.len(), 69);
    vec![words]
}
/// Genuine authenticated Load terminal artifacts for outer/next-operation tests.
/// The supplied predecessor's catalog/profile provenance is retained unchanged;
/// this fixture does not declare the resulting lineage admitted by a final catalog.
#[derive(Clone)]
#[allow(dead_code)] // Outer and next-operation tests consume all carried artifacts.
pub(crate) struct AuthenticatedLoad {
    pub(crate) key: VerifyingKey<Eq>,
    pub(crate) binding: iroha_plonk::DescriptorBinding,
    pub(crate) proof: Vec<u8>,
    pub(crate) instances: Vec<Fp>,
    pub(crate) opening: FoldInput<Eq>,
    pub(crate) pallas: AccumulatorT<Ep>,
    pub(crate) vesta_part: AccumulatorT<Eq>,
    pub(crate) predecessor_vesta: AccumulatorT<Eq>,
    pub(crate) state: iroha_kagemusha_proof::admin_sigma::StateWitness,
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut text, "{byte:02x}").unwrap();
    }
    text
}
fn proof_identity(bytes: &[u8]) -> Fp {
    let mut framed = u32::try_from(bytes.len()).unwrap().to_le_bytes().to_vec();
    framed.extend(bytes);
    let words = framed
        .chunks(31)
        .map(|chunk| {
            let mut repr = [0; 32];
            repr[..chunk.len()].copy_from_slice(chunk);
            Fp::from_repr(repr).unwrap()
        })
        .collect::<Vec<_>>();
    hash_with_domain(u64::from_le_bytes(*b"kgwtstp1"), &words)
}
fn artifact_diagnostics(
    stage: usize,
    key: &iroha_plonk::ProvingKey<Eq>,
    proof: &[u8],
    range_buses: usize,
) {
    eprintln!(
        "LOAD_A_ARTIFACT stage={} range_buses={} shape={:?} descriptor_personalized_blake2b={} vk_kgwvkey1={} proof_pbytes_kgwtstp1={} proof_bytes={} diagnostic_only=true",
        stage + 1,
        range_buses,
        iroha_plonk::protocol::Protocol::new(key.binding().descriptor())
            .unwrap()
            .shape(),
        hex(key.binding().digest()),
        hex(&key.vk().kagemusha_digest(key.binding()).unwrap().to_repr()),
        hex(&proof_identity(proof).to_repr()),
        proof.len()
    );
}
fn continue_schedule(
    first: &First,
    first_key: &iroha_plonk::ProvingKey<Eq>,
    first_proof: iroha_plonk::ProverOutput<Eq>,
    first_public: &[Vec<Fp>],
    adversarial: bool,
) -> Option<AuthenticatedLoad> {
    use iroha_kagemusha_proof::a_relation::split::{SplitPlan, WCircuit, WKey};
    let params = &first.source.params;
    let vparams = common::vesta_params(16);
    let mut source_key = first_key.clone();
    let mut source_proof = first_proof;
    let mut source_public = first_public.to_vec();
    let mut carried = first.pallas.clone();
    let mut part = first.source.part.clone();
    let mut history = Vec::new();
    let mut intermediate_keys = vec![first_key.vk().clone()];
    for stage in 1..first.plan.stage_count() {
        let own =
            FoldInput::from_opening(*source_proof.opening.g(), source_proof.opening.challenges())
                .unwrap();
        let trivial = AccumulatorT::trivial(&vparams, MemoryBudget::DEFAULT)
            .unwrap()
            .as_input();
        let (vfold, vesta) = create_fold(
            &vparams,
            &[part, own, trivial.clone(), trivial],
            Fq::from(123 + u64::try_from(stage).unwrap()).to_repr(),
            &FoldConfig::default(),
        )
        .unwrap();
        vesta.decide(&vparams, MemoryBudget::DEFAULT).unwrap();
        let witness = iroha_kagemusha_proof::omega::OmegaWitness {
            key: source_key.vk().clone(),
            instances: source_public[0].clone(),
            length: source_proof.proof.len().try_into().unwrap(),
            proof: source_proof.proof,
            fold: vfold.to_bytes(),
        };
        assert!(
            WCircuit::new(
                &first.plan,
                first.plan.stage_count() - 1,
                source_key.binding().clone(),
                vparams.clone(),
                vec![
                    source_key
                        .vk()
                        .kagemusha_digest(source_key.binding())
                        .unwrap()
                ],
                witness.clone()
            )
            .is_err(),
            "terminal A cannot be wrapped as an intermediate stage"
        );
        let circuit = WCircuit::new(
            &first.plan,
            stage - 1,
            source_key.binding().clone(),
            vparams.clone(),
            vec![
                source_key
                    .vk()
                    .kagemusha_digest(source_key.binding())
                    .unwrap(),
            ],
            witness,
        )
        .unwrap();
        let (wkey, wprover) = WKey::keygen(&circuit, params).unwrap();
        for wrong_stage in 0..=first.plan.stage_count() {
            if wrong_stage != stage {
                assert!(
                    SplitPlan::new(first.plan.clone(), wrong_stage, wkey.clone(), params).is_err(),
                    "wrong-stage W must not resume at {wrong_stage}"
                );
            }
        }
        let (x, y) = vesta.g().coordinates().unwrap();
        let wpublic = vec![
            vec![Fq::from_repr(source_public[0][0].to_repr()).unwrap()],
            vec![x, y],
            vesta
                .challenges()
                .iter()
                .map(|v| Fq::from_repr(v.to_repr()).unwrap())
                .collect(),
        ];
        let wrapper = create_proof_owned_with_claim(
            params,
            &wprover,
            Witness::from_circuit(&wprover, &circuit, &wpublic).unwrap(),
            common::recovery(196 + u8::try_from(stage).unwrap()),
            ProverConfig::default(),
        )
        .unwrap();
        let wopening = accumulate_generator(
            params,
            wprover.binding(),
            wprover.vk(),
            &wpublic,
            &wrapper.proof,
            MemoryBudget::DEFAULT,
        )
        .unwrap();
        wopening.decide(params, MemoryBudget::DEFAULT).unwrap();
        let indices = first.plan.q_partition(stage).unwrap();
        let mut claims = vec![
            carried.as_input(),
            FoldInput::from_opening(*wopening.g(), wopening.challenges()).unwrap(),
        ];
        claims.extend(indices.iter().map(|i| first.source.q_openings[*i].clone()));
        let salt = Fp::from(124 + u64::try_from(stage).unwrap()).to_repr();
        let (fold, pallas) = create_fold(params, &claims, salt, &FoldConfig::default()).unwrap();
        pallas.decide(params, MemoryBudget::DEFAULT).unwrap();
        let continuation = Continuation {
            first: first.clone(),
            plan: SplitPlan::new(first.plan.clone(), stage, wkey, params).unwrap(),
            wrapper: wrapper.proof,
            vesta: vesta.clone(),
            fold: fold.to_bytes().to_vec(),
            pallas: pallas.clone(),
            carried: carried.clone(),
            history: history.clone(),
            omit_q: None,
        };
        let public = continuation.public();
        let (assigned, k) = match synthesize(&continuation, 16, Some(&public)) {
            Ok(a) => (a, 16),
            Err(error) => {
                eprintln!(
                    "actual Load A{} fails k16 {error:?}; diagnostic k18 only",
                    stage + 1
                );
                (synthesize(&continuation, 18, Some(&public)).unwrap(), 18)
            }
        };
        let report = check(&assigned.cs, &assigned.tables, CheckMode::Strict).unwrap();
        assert!(report.is_satisfied(), "{:?}", report.failures().first());
        let unknown = synthesize(&continuation.without_witnesses(), k, None).unwrap();
        assert_eq!(assigned.tables.fixed(), unknown.tables.fixed());
        assert_eq!(assigned.tables.permutation(), unknown.tables.permutation());
        assert_eq!(
            assigned.tables.advice_assigned(),
            unknown.tables.advice_assigned()
        );
        let lanes: Vec<_> = assigned
            .tables
            .advice_assigned()
            .iter()
            .map(|c| c.iter().rposition(|v| *v).map_or(0, |i| i + 1))
            .collect();
        eprintln!(
            "genuine Load A{} hard W+Q{indices:?}+context+fold range_buses={} lanes={lanes:?}, production_k16_fit={}",
            stage + 1,
            first.range_buses,
            k == 16
        );
        if adversarial {
            for omitted in 0..claims.len() {
                let kept = claims
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != omitted)
                    .map(|(_, c)| c.clone())
                    .collect::<Vec<_>>();
                let (fold, _) = create_fold(params, &kept, salt, &FoldConfig::default()).unwrap();
                let mut bad = continuation.clone();
                bad.fold = fold.to_bytes().to_vec();
                assert!(
                    !iroha_plonk::check::check_circuit(&bad, k, &public, CheckMode::Strict)
                        .is_ok_and(|r| r.is_satisfied()),
                    "dropped stage{stage} claim{omitted}"
                );
            }
            let mut doubled = claims.clone();
            doubled.push(claims[0].clone());
            let (fold, _) = create_fold(params, &doubled, salt, &FoldConfig::default()).unwrap();
            let mut bad = continuation.clone();
            bad.fold = fold.to_bytes().to_vec();
            assert!(
                !iroha_plonk::check::check_circuit(&bad, k, &public, CheckMode::Strict)
                    .is_ok_and(|r| r.is_satisfied()),
                "double-consumed stage{stage} contextP"
            );
            for index in indices {
                let mut bad = continuation.clone();
                bad.omit_q = Some(*index);
                assert!(
                    !iroha_plonk::check::check_circuit(&bad, k, &public, CheckMode::Strict)
                        .is_ok_and(|r| r.is_satisfied()),
                    "partial W cannot omit stage{stage} Q{index}"
                );
                let mut source = (*first.source).clone();
                source.q_proofs[*index][0] ^= 1;
                let mut bad = continuation.clone();
                bad.first.source = Arc::new(source);
                assert!(
                    !iroha_plonk::check::check_circuit(&bad, k, &public, CheckMode::Strict)
                        .is_ok_and(|r| r.is_satisfied()),
                    "wrong deferred proof stage{stage} Q{index}"
                );
            }
            if !history.is_empty() {
                let mut bad = continuation.clone();
                bad.history.clear();
                assert!(
                    !iroha_plonk::check::check_circuit(&bad, k, &public, CheckMode::Strict)
                        .is_ok_and(|r| r.is_satisfied()),
                    "missing fixed-length stage history"
                );
                let mut bad = continuation.clone();
                bad.history[0].0 = pallas.clone();
                assert!(
                    !iroha_plonk::check::check_circuit(&bad, k, &public, CheckMode::Strict)
                        .is_ok_and(|r| r.is_satisfied()),
                    "relabelled prior accumulated P at stage{stage}"
                );
                let mut bad = continuation.clone();
                bad.history[0].1 = vesta.clone();
                assert!(
                    !iroha_plonk::check::check_circuit(&bad, k, &public, CheckMode::Strict)
                        .is_ok_and(|r| r.is_satisfied()),
                    "relabelled prior accumulated V at stage{stage}"
                );
            }
        }
        if k != 16 {
            return None;
        }
        let mut cfg = KeygenConfigV2::pipa_r(vec![iroha_plonk::cs::InstanceType::Bounded]);
        cfg.compress_selectors = false;
        let key = keygen_pk_v2(&vparams, &continuation, &cfg).unwrap();
        assert_eq!(key.binding().descriptor(), first_key.binding().descriptor());
        let proof = create_proof_owned_with_claim(
            &vparams,
            &key,
            Witness::from_circuit(&key, &continuation, &public).unwrap(),
            common::recovery(200 + u8::try_from(stage).unwrap()),
            ProverConfig::default(),
        )
        .unwrap();
        proof
            .opening
            .decide(&vparams, MemoryBudget::DEFAULT)
            .unwrap();
        eprintln!(
            "actual Load A{} proof={}B; terminal={}; final Omega/full catalog still pending",
            stage + 1,
            proof.proof.len(),
            continuation.plan.is_terminal()
        );
        artifact_diagnostics(stage, &key, &proof.proof, first.range_buses);
        if continuation.plan.is_terminal() {
            let final_digest = key.vk().kagemusha_digest(key.binding()).unwrap();
            let final_catalog = iroha_kagemusha_proof::omega::OmegaPlan::new(
                key.binding().clone(),
                vparams.clone(),
                vec![final_digest],
            )
            .unwrap();
            assert!(
                final_catalog
                    .clone()
                    .with_key_catalog(vec![key.vk().clone()])
                    .is_ok()
            );
            for internal in &intermediate_keys {
                assert_ne!(
                    internal.kagemusha_digest(key.binding()).unwrap(),
                    final_digest
                );
                assert!(
                    final_catalog
                        .clone()
                        .with_key_catalog(vec![internal.clone()])
                        .is_err(),
                    "terminal-only catalog rejects every intermediate A key"
                );
            }
            return Some(AuthenticatedLoad {
                key: key.vk().clone(),
                binding: key.binding().clone(),
                opening: FoldInput::from_opening(*proof.opening.g(), proof.opening.challenges())
                    .unwrap(),
                proof: proof.proof,
                instances: public[0].clone(),
                pallas,
                vesta_part: vesta,
                predecessor_vesta: first.source.predecessor.vesta.clone(),
                state: first.source.maps.witness.successor,
            });
        }
        intermediate_keys.push(key.vk().clone());
        history.push((carried, vesta.clone()));
        carried = pallas;
        part = vesta.as_input();
        source_key = key;
        source_proof = proof;
        source_public = public;
    }
    None
}
fn push_pallas(words: &mut Vec<Fp>, claim: &FoldInput<Ep>) {
    words.push(Fp::from(16));
    let (x, y) = claim.g().coordinates().unwrap();
    words.extend([x, y]);
    for u in claim.challenges() {
        words.extend(foreign_limbs(u).map(Fp::from_u128));
    }
}
fn vesta_words(claim: &FoldInput<Eq>) -> Vec<Fp> {
    let (x, y) = claim.g().coordinates().unwrap();
    let mut words = vec![];
    for v in [x, y] {
        words.extend(foreign_limbs(&v).map(Fp::from_u128));
    }
    words.extend(claim.challenges());
    words
}
fn context_digest(first: &First) -> Fp {
    let s = &first.source;
    let mut words = first.plan.schema().to_vec();
    words.extend(s.maps.witness.statement);
    let before = &s.maps.witness.predecessor;
    let after = &s.maps.witness.successor;
    words.extend(before.core);
    words.extend(before.rest);
    words.extend(before.lineage);
    push_pallas(&mut words, &s.predecessor.pallas.as_input());
    words.extend(vesta_words(&s.predecessor.vesta.as_input()));
    words.extend(after.core);
    words.extend(after.rest);
    words.extend(after.lineage);
    for v in s.q_instances.iter().flatten().flatten() {
        words.extend(foreign_limbs(v).map(Fp::from_u128));
    }
    for (object, spec) in s
        .maps
        .objects
        .iter()
        .zip(LoadObjects::context_specs().unwrap())
    {
        let mut bytes = spec.capacity.to_le_bytes().to_vec();
        bytes.extend(&object.bytes);
        let mut tape = vec![
            Fp::from(u64::from(spec.tag)),
            Fp::from(u64::from(spec.capacity)),
        ];
        for chunk in bytes.chunks(31) {
            let mut repr = [0; 32];
            repr[..chunk.len()].copy_from_slice(chunk);
            tape.push(Fp::from_repr(repr).unwrap());
        }
        words.extend([
            object.digest(),
            Fp::from(u64::from(spec.capacity)),
            hash_with_domain(u64::from_le_bytes(*b"kgwctap1"), &tape),
        ]);
    }
    push_pallas(&mut words, &first.pallas.as_input());
    hash_with_domain(u64::from_le_bytes(*b"kgwctx_1"), &words)
}
fn first_public(first: &First) -> Vec<Vec<Fp>> {
    let mut words = vec![context_digest(first), Fp::from(12)];
    words.extend(vesta_words(&first.source.part));
    let trivial =
        AccumulatorT::<Eq>::trivial(&common::vesta_params(16), MemoryBudget::DEFAULT).unwrap();
    let trivial = vesta_words(&trivial.as_input());
    words.extend(&trivial);
    words.extend(&trivial);
    words.extend([Fp::ZERO, Fp::ONE, Fp::ZERO]);
    words.extend(&trivial[..4]);
    assert_eq!(words.len(), 69);
    vec![words]
}
#[test]
#[ignore = "real rooted Bootstrap chain, Load Qs and A predecessor partition; run optimized"]
fn rooted_load_first_partition_preserves_real_predecessor_and_q() {
    let _ = rooted_load_profiles(&[0]);
}
#[test]
#[ignore = "real rooted Bootstrap, Load Qs and shared-range four-stage chain; run optimized"]
fn rooted_load_shared_range_four_stage_inventory() {
    assert!(rooted_load_profiles(&[4, 5]).is_some());
}
#[test]
#[ignore = "canonical selector/task schema and genuine five-bus Load closure; run optimized"]
fn canonical_five_bus_load_tasks_retain_all_openings() {
    assert!(rooted_load_profiles(&[5]).is_some());
}
#[test]
#[ignore = "canonical selector/task schema and genuine four-bus Load closure; run optimized"]
fn canonical_four_bus_load_tasks_retain_all_openings() {
    assert!(rooted_load_profiles(&[4]).is_some());
}
#[test]
#[ignore = "genuine predecessor proof and both hard signature Q leaves; run optimized"]
fn current_authorization_and_transport_reject_cross_tape_substitution() {
    let rooted = bootstrap_outer::rooted_bootstrap_omega(false);
    let _ = load_profiles(&rooted, &[4], true, true);
}
fn rooted_load_profiles(profiles: &[usize]) -> Option<AuthenticatedLoad> {
    let rooted = bootstrap_outer::rooted_bootstrap_omega(false);
    load_profiles(&rooted, profiles, true, false)
}
/// Build actual sigma/Q/A1/W1/A2/W2/A3/W3/A4 proofs for the genuine predecessor.
/// Catalog rebinding/admission is the outer caller's separate responsibility.
#[allow(dead_code)] // Consumed by the outer integration test.
pub(crate) fn authenticated_load(
    rooted: &bootstrap_outer::RootedBootstrapOmega,
    range_buses: usize,
    adversarial: bool,
) -> AuthenticatedLoad {
    load_profiles(rooted, &[range_buses], adversarial, false).expect("Load stages must fit k16")
}

/// Candidate Q/source profile with the same hard operation and obligation schedule.
#[allow(dead_code)] // Consumed by candidate outer/catalog integration tests.
pub(crate) fn authenticated_load_with_q_layout(
    rooted: &bootstrap_outer::RootedBootstrapOmega,
    range_buses: usize,
    q_buses: Option<usize>,
    adversarial: bool,
) -> AuthenticatedLoad {
    load_profiles_with_q(rooted, &[range_buses], q_buses, adversarial, false)
        .expect("candidate Load stages must fit k16")
}

#[test]
#[ignore = "genuine candidate Q2/A3 Bootstrap predecessor and complete Load stages"]
fn reduced_q_two_bus_three_bus_load_capacity() {
    let rooted = bootstrap_outer::rooted_bootstrap_omega_with_q_layout(false, 3, Some(2));
    assert!(load_profiles_with_q(&rooted, &[3], Some(2), false, false).is_some());
}

fn load_profiles(
    rooted: &bootstrap_outer::RootedBootstrapOmega,
    profiles: &[usize],
    adversarial: bool,
    components_only: bool,
) -> Option<AuthenticatedLoad> {
    load_profiles_with_q(rooted, profiles, None, adversarial, components_only)
}
fn load_profiles_with_q(
    rooted: &bootstrap_outer::RootedBootstrapOmega,
    profiles: &[usize],
    q_buses: Option<usize>,
    adversarial: bool,
    components_only: bool,
) -> Option<AuthenticatedLoad> {
    let source = Arc::new(sources(rooted, q_buses));
    for range_buses in profiles.iter().copied() {
        let initial_counts: &[usize] = &[0];
        for first_q in initial_counts.iter().copied() {
            let objects = LoadObjects::context_specs().unwrap().to_vec();
            let plan = if first_q == 0 {
                ContextPlan::with_schedule(
                    source.plan.clone(),
                    vec![vec![], vec![0], vec![1], vec![2]],
                    Some(0),
                    objects,
                )
            } else {
                ContextPlan::with_predecessor_first(source.plan.clone(), first_q, objects)
            }
            .unwrap();
            let mut tasks = vec![Vec::new(); plan.stage_count()];
            tasks[0].push(OperationTask::LoadRecovery);
            for (stage, group) in tasks.iter_mut().enumerate() {
                let qs = plan.q_partition(stage).unwrap();
                if qs.contains(&1) {
                    group.push(OperationTask::LoadAuthorization);
                }
                if qs.contains(&2) {
                    group.push(OperationTask::LoadCurrentAuthorization);
                }
            }
            let plan = plan.with_operation_tasks(tasks).unwrap();
            LoadStagePlan::new(plan.clone()).unwrap();
            if adversarial {
                for mutation in 0..3 {
                    let mut specs = LoadObjects::context_specs().unwrap().to_vec();
                    match mutation {
                        0 => specs[0].tag = 77,
                        1 => specs[1].capacity += 1,
                        _ => {
                            specs.pop();
                        }
                    }
                    let bad = ContextPlan::with_schedule(
                        source.plan.clone(),
                        (0..plan.stage_count())
                            .map(|i| plan.q_partition(i).unwrap().to_vec())
                            .collect(),
                        plan.predecessor_stage(),
                        specs,
                    )
                    .unwrap()
                    .with_operation_tasks(
                        (0..plan.stage_count())
                            .map(|i| plan.operation_tasks(i).unwrap().to_vec())
                            .collect(),
                    )
                    .unwrap();
                    assert!(
                        LoadStagePlan::new(bad).is_err(),
                        "wrong Load object schema{mutation}"
                    );
                }
            }

            let mut claims = vec![
                source.predecessor.pallas.as_input(),
                source.predecessor.opening.clone(),
            ];
            claims.extend(source.q_openings[..first_q].iter().cloned());
            let (fold, pallas) = create_fold(
                &source.params,
                &claims,
                Fp::from(122).to_repr(),
                &FoldConfig::default(),
            )
            .unwrap();
            pallas
                .decide(&source.params, MemoryBudget::DEFAULT)
                .unwrap();
            let first = First {
                source: source.clone(),
                range_buses,
                plan,
                pallas,
                fold: fold.to_bytes().to_vec(),
                known: true,
            };
            if adversarial {
                consuming_proof::check(&first);
                for current in [false, true] {
                    let authorization = Authorization {
                        first: first.clone(),
                        current,
                        context_mutation: 0,
                        order: [0, 1, 2],
                        wrong_policy: false,
                    };
                    let public = vec![
                        source
                            .maps
                            .objects
                            .iter()
                            .map(load_components::bootstrap_objects::Signed::digest)
                            .collect::<Vec<_>>(),
                    ];
                    assert!(
                        iroha_plonk::check::check_circuit(
                            &authorization,
                            16,
                            &public,
                            CheckMode::Strict
                        )
                        .unwrap()
                        .is_satisfied()
                    );
                    let mut bad = authorization.clone();
                    bad.order = [1, 0, 2];
                    assert!(
                        !iroha_plonk::check::check_circuit(&bad, 16, &public, CheckMode::Strict)
                            .is_ok_and(|r| r.is_satisfied())
                    );
                    let mut bad = authorization.clone();
                    bad.wrong_policy = true;
                    assert!(
                        !iroha_plonk::check::check_circuit(&bad, 16, &public, CheckMode::Strict)
                            .is_ok_and(|r| r.is_satisfied())
                    );
                    for context_mutation in 1..=3 {
                        let bad = Authorization {
                            context_mutation,
                            ..authorization.clone()
                        };
                        assert!(
                            !iroha_plonk::check::check_circuit(
                                &bad,
                                16,
                                &public,
                                CheckMode::Strict
                            )
                            .is_ok_and(|r| r.is_satisfied()),
                            "signature bundle context/index/shape substitution{context_mutation}"
                        );
                    }
                    if current {
                        for (object, offset) in [
                            (3, 35),
                            (4, 226),
                            (4, source.maps.objects[4].bytes.len() - 1),
                        ] {
                            let mut bad_source = (*source).clone();
                            bad_source.maps.objects[object].bytes[offset] ^= 1;
                            let mut bad = authorization.clone();
                            bad.first.source = Arc::new(bad_source);
                            let changed = vec![
                                bad.first
                                    .source
                                    .maps
                                    .objects
                                    .iter()
                                    .map(load_components::bootstrap_objects::Signed::digest)
                                    .collect::<Vec<_>>(),
                            ];
                            assert!(
                                !iroha_plonk::check::check_circuit(
                                    &bad,
                                    16,
                                    &changed,
                                    CheckMode::Strict
                                )
                                .is_ok_and(|r| r.is_satisfied()),
                                "current authorization substitution object={object} byte={offset}"
                            );
                        }
                    }
                    let known = synthesize(&authorization, 16, None).unwrap();
                    let unknown = synthesize(&authorization.without_witnesses(), 16, None).unwrap();
                    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
                    assert_eq!(
                        known.tables.advice_assigned(),
                        unknown.tables.advice_assigned()
                    );
                    eprintln!(
                        "actual Load signature Q/object/receipt authorization component PASS; retained opening closed by subsequent complete A relation only"
                    );
                }
            }
            if components_only {
                return None;
            }
            let public = first_public(&first);
            let (assigned, k) = match synthesize(&first, 16, Some(&public)) {
                Ok(a) => (a, 16),
                Err(error) => {
                    eprintln!(
                        "Load predecessor-first Qcount{first_q} k16 fails {error:?}; diagnostic k18 only"
                    );
                    (synthesize(&first, 18, Some(&public)).unwrap(), 18)
                }
            };
            let report = check(&assigned.cs, &assigned.tables, CheckMode::Strict).unwrap();
            assert!(report.is_satisfied(), "{:?}", report.failures().first());
            let lanes: Vec<_> = assigned
                .tables
                .advice_assigned()
                .iter()
                .map(|c| c.iter().rposition(|v| *v).map_or(0, |i| i + 1))
                .collect();
            eprintln!(
                "genuine Load first predecessor+Q{first_q}+map+context range_buses={range_buses} lanes={lanes:?}, production_k16_fit={}",
                k == 16
            );
            if adversarial {
                // Every predecessor obligation must survive even when no Q is yet
                // authenticated. The native proof of a smaller fold is not interchangeable.
                for omitted in 0..claims.len() {
                    let retained = claims
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i != omitted)
                        .map(|(_, c)| c.clone())
                        .collect::<Vec<_>>();
                    let (bad_fold, _) = create_fold(
                        &source.params,
                        &retained,
                        Fp::from(122).to_repr(),
                        &FoldConfig::default(),
                    )
                    .unwrap();
                    let mut bad = first.clone();
                    bad.fold = bad_fold.to_bytes().to_vec();
                    assert!(
                        !iroha_plonk::check::check_circuit(&bad, k, &public, CheckMode::Strict)
                            .is_ok_and(|r| r.is_satisfied()),
                        "dropped first-stage obligation{omitted} with Qcount{first_q}"
                    );
                }
                let mut relabelled = first.clone();
                relabelled.plan = ContextPlan::with_predecessor_first(
                    source.plan.clone(),
                    1 - first_q,
                    LoadObjects::context_specs().unwrap().to_vec(),
                )
                .unwrap();
                assert_ne!(first.plan.schema(), relabelled.plan.schema());
                assert!(
                    !iroha_plonk::check::check_circuit(&relabelled, k, &public, CheckMode::Strict)
                        .is_ok_and(|r| r.is_satisfied()),
                    "relabelled Q partition"
                );
                if first_q == 1 {
                    let mut bad_source = (*source).clone();
                    bad_source.q_proofs[0][0] ^= 1;
                    let mut bad = first.clone();
                    bad.source = Arc::new(bad_source);
                    assert!(
                        !iroha_plonk::check::check_circuit(&bad, k, &public, CheckMode::Strict)
                            .is_ok_and(|r| r.is_satisfied()),
                        "wrong hard Q proof"
                    );
                }
            }
            if k == 16 {
                let mut cfg = KeygenConfigV2::pipa_r(vec![iroha_plonk::cs::InstanceType::Bounded]);
                cfg.compress_selectors = false;
                let a_params = common::vesta_params(16);
                let key = keygen_pk_v2(&a_params, &first, &cfg).unwrap();
                let proof = create_proof_owned_with_claim(
                    &a_params,
                    &key,
                    Witness::from_circuit(&key, &first, &public).unwrap(),
                    common::recovery(195),
                    ProverConfig::default(),
                )
                .unwrap();
                proof
                    .opening
                    .decide(&a_params, MemoryBudget::DEFAULT)
                    .unwrap();
                eprintln!(
                    "actual Load A1 proof={}B, no final continuation claimed",
                    proof.proof.len()
                );
                artifact_diagnostics(0, &key, &proof.proof, range_buses);
                if let Some(result) = continue_schedule(&first, &key, proof, &public, adversarial) {
                    eprintln!(
                        "genuine Load four-stage C4 closure native proofs PASS source_range_buses={range_buses}; final full-catalog Omega still pending"
                    );
                    return Some(result);
                }
            }
        }
    }
    None
}
