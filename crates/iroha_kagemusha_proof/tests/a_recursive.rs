//! Actual Q proof ownership in A's Bootstrap frame. This isolates the recursive
//! frame; Bootstrap's zero-state operation constraints are tested separately.

mod common;
use ff::{Field, PrimeField};
use iroha_kagemusha_proof::{
    Mutation, SigmaRelation,
    a_relation::{
        AFramePlan, AOutputCells, AProofPlan, LINEAGE_DOMAIN, LineagePublicCells,
        ProofMessageCells, QProofPlan, SigmaBindingCells, VestaClaimCells, fold_pallas,
        lineage_digest, verify_predecessor, verify_sigma,
    },
    operation_relation::statement::StatementCells,
    q_sigma::{QSigmaPlan, SigmaClass, SigmaSlotWitness, native::QSigmaProver},
    sample_witness,
};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaAffine, msm::MemoryBudget, poseidon::hash_with_domain};
use iroha_plonk::{
    ProverConfig, VerifyingKey, Witness,
    check::{CheckMode, check_circuit},
    create_proof_owned_with_claim,
    cs::{Column, ConstraintSystem, Instance, InstanceType},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner, Value, synthesize},
    keys::{KeygenConfigV2, keygen_pk_v2},
    pcs::ipa::PinnedParams,
    transcript::TranscriptRepr,
    verifier::accumulate_generator,
};
use iroha_plonk_gadgets::{
    GlueChip, GlueConfig,
    bytes::{
        element::le_message_segments,
        tape::{BytesChip, BytesConfig, SegmentSpec},
    },
    statement::foreign_limbs,
};
use iroha_plonk_recursion::{
    AccumulatorT, FoldConfig, FoldInput, VESTA_TRIVIAL_GENERATOR,
    accumulation_circuit::FoldInputCells,
    codec::ScalarCells,
    create_fold,
    obligation::ledger::Variant,
    verifier::{VerifierChip, VerifierConfig, VerifierPlan},
};

#[derive(Clone, Debug)]
struct Config {
    verifier: VerifierConfig<Ep>,
    bytes: BytesConfig,
    public: Column<Instance>,
}
#[derive(Clone)]
struct Recursive {
    plan: AProofPlan,
    q: Vec<u8>,
    q_length: u32,
    instances: Vec<Vec<Fq>>,
    sigma: Vec<u8>,
    statement: [Fp; 26],
    fields: [Fp; 18],
    known: bool,
    route: u8,
    predecessor: Option<Predecessor>,
}
impl Recursive {
    fn value<T: Copy>(&self, v: T) -> Value<T> {
        if self.known {
            Value::known(v)
        } else {
            Value::unknown()
        }
    }
}
fn carrier(
    chip: &mut VerifierChip<Ep>,
    bytes: &mut BytesChip<Fp>,
    region: &mut iroha_plonk::frontend::Region<'_, Fp>,
    body: &[u8],
    length: u32,
    known: bool,
) -> Result<ProofMessageCells, Error> {
    let raw = length
        .to_le_bytes()
        .into_iter()
        .chain(body.iter().copied())
        .map(|v| {
            if known {
                Value::known(v)
            } else {
                Value::unknown()
            }
        })
        .collect::<Vec<_>>();
    let mut segments = vec![SegmentSpec::little(0, 4)];
    segments.extend(le_message_segments(4, body.len() / 32));
    let run = bytes.run(
        region,
        &raw,
        &raw.chunks(31).map(<[Value<u8>]>::len).collect::<Vec<_>>(),
        &segments,
    )?;
    ProofMessageCells::from_run(chip, region, &run, 0, body.len())
}
#[derive(Clone)]
struct Predecessor {
    key: VerifyingKey<Ep>,
    fields: [Fp; 18],
    pallas: FoldInput<Ep>,
    proof: Vec<u8>,
    length: u32,
    fold: Vec<u8>,
    fold_length: u32,
}
struct Fixture {
    circuit: Recursive,
    values: [Vec<Fp>; 1],
    sigma_plan: QSigmaPlan,
    q_plan: QProofPlan,
    q_opening: FoldInput<Ep>,
    part: FoldInput<Eq>,
    params: PinnedParams<Ep>,
}

fn scalar(
    chip: &mut VerifierChip<Ep>,
    region: &mut iroha_plonk::frontend::Region<'_, Fp>,
    v: Value<Fq>,
) -> Result<ScalarCells<Ep>, Error> {
    let lo = chip
        .uint()
        .assign::<128>(region, v.map(|v| foreign_limbs(&v)[0]))?;
    let hi = chip
        .uint()
        .assign::<127>(region, v.map(|v| foreign_limbs(&v)[1]))?;
    ScalarCells::from_limbs(&mut chip.uint(), region, &lo, &hi)
}
impl Circuit<Fp> for Recursive {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Config {
        let verifier = VerifierConfig::configure(meta);
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
            || "A owns real Q opening",
            |mut region| {
                let proof = carrier(
                    &mut chip,
                    &mut bytes,
                    &mut region,
                    &self.q,
                    self.q_length,
                    self.known,
                )?;
                let mut instances = Vec::new();
                for column in &self.instances {
                    let mut values = Vec::new();
                    for value in column {
                        values.push(scalar(&mut chip, &mut region, self.value(*value))?);
                    }
                    instances.push(values);
                }
                let sigma = self
                    .sigma
                    .iter()
                    .map(|v| self.value(*v))
                    .collect::<Vec<_>>();
                let chunks = bytes.run(
                    &mut region,
                    &sigma,
                    &sigma.chunks(31).map(<[Value<u8>]>::len).collect::<Vec<_>>(),
                    &[],
                )?;
                let statement = self
                    .statement
                    .iter()
                    .map(|v| chip.uint().glue().witness(&mut region, self.value(*v)))
                    .collect::<Result<Vec<_>, _>>()?;
                let statement = StatementCells::constrain_with_verifier(
                    &mut chip,
                    &mut region,
                    Variant::Send,
                    &statement.try_into().map_err(|_| Error::Synthesis)?,
                )?;
                let index = chip.uint().glue().constant(&mut region, Fp::from(5))?;
                let binding = SigmaBindingCells::from_statement(
                    &statement,
                    index,
                    chunks.primary().iter().map(|s| s.word().clone()).collect(),
                );
                let (q, sigma) = verify_sigma(
                    &mut chip,
                    &mut region,
                    &self.plan,
                    &instances,
                    &proof,
                    &[binding],
                )?;
                let q = match self.route {
                    0 => vec![q],
                    1 => vec![],
                    _ => vec![q.clone(), q],
                };
                let fields = self
                    .fields
                    .iter()
                    .map(|v| chip.uint().glue().witness(&mut region, self.value(*v)))
                    .collect::<Result<Vec<_>, _>>()?;
                let public = LineagePublicCells::constrain(
                    &mut chip.uint(),
                    &mut region,
                    &fields.try_into().map_err(|_| Error::Synthesis)?,
                )?;
                let mut predecessor = None;
                let mut fold = None;
                if let Some(source) = &self.predecessor {
                    let TranscriptRepr::Base(repr) = *source.key.transcript_repr() else {
                        return Err(Error::Synthesis);
                    };
                    let fixed = source
                        .key
                        .fixed_commitments()
                        .iter()
                        .map(|p| self.value(Ep::from(*p)))
                        .collect::<Vec<_>>();
                    let permutation = source
                        .key
                        .permutation_commitments()
                        .iter()
                        .map(|p| self.value(Ep::from(*p)))
                        .collect::<Vec<_>>();
                    let key =
                        chip.witness_key(&mut region, self.value(repr), &fixed, &permutation)?;
                    let fields = source
                        .fields
                        .iter()
                        .map(|v| chip.uint().glue().witness(&mut region, self.value(*v)))
                        .collect::<Result<Vec<_>, _>>()?;
                    let previous = LineagePublicCells::constrain(
                        &mut chip.uint(),
                        &mut region,
                        &fields.try_into().map_err(|_| Error::Synthesis)?,
                    )?;
                    let g = chip.constant_point(&mut region, &Ep::from(*source.pallas.g()))?;
                    let challenges = source
                        .pallas
                        .challenges()
                        .iter()
                        .map(|v| scalar(&mut chip, &mut region, self.value(*v)))
                        .collect::<Result<Vec<_>, _>>()?;
                    let claim = FoldInputCells::from_normalized(
                        &mut chip,
                        &mut region,
                        16,
                        g,
                        challenges.try_into().map_err(|_| Error::Synthesis)?,
                    )?;
                    let vesta = VestaClaimCells::trivial(&mut chip, &mut region)?;
                    let proof = carrier(
                        &mut chip,
                        &mut bytes,
                        &mut region,
                        &source.proof,
                        source.length,
                        self.known,
                    )?;
                    predecessor = Some(verify_predecessor(
                        &mut chip,
                        &mut region,
                        &self.plan,
                        &key,
                        &previous,
                        &public,
                        &claim,
                        &vesta,
                        &proof,
                    )?);
                    fold = Some(carrier(
                        &mut chip,
                        &mut bytes,
                        &mut region,
                        &source.fold,
                        source.fold_length,
                        self.known,
                    )?);
                }
                let pallas = fold_pallas(
                    &mut chip,
                    &mut region,
                    &self.plan,
                    predecessor.as_ref(),
                    None,
                    &q,
                    fold.as_ref(),
                )?;
                let digest = lineage_digest(&mut chip, &mut region, &public, &pallas)?;
                AOutputCells {
                    digest,
                    sigma_part: sigma.part,
                    predecessor: predecessor.map(|p| p.vesta().clone()),
                    incoming: None,
                }
                .words(&mut chip, &mut region, self.plan.frame())
            },
        )?;
        for (row, word) in out.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, row)?;
        }
        Ok(())
    }
}
fn public(
    fields: &[Fp; 18],
    pallas: &FoldInput<Ep>,
    part: &iroha_plonk_recursion::FoldInput<iroha_pasta::Eq>,
) -> Vec<Fp> {
    let mut words = fields.to_vec();
    let (x, y) = pallas.g().coordinates().unwrap();
    words.extend([x, y]);
    for value in pallas.challenges() {
        words.extend(foreign_limbs(value).map(Fp::from_u128));
    }
    let mut out = vec![
        hash_with_domain(LINEAGE_DOMAIN, &words),
        Fp::from(u64::from(part.source_k())),
    ];
    let (x, y) = part.g().coordinates().unwrap();
    for coordinate in [x, y] {
        out.extend(foreign_limbs(&coordinate).map(Fp::from_u128));
    }
    out.extend(part.challenges());
    let trivial =
        iroha_plonk::transcript::decode_point::<iroha_pasta::Eq>(&VESTA_TRIVIAL_GENERATOR).unwrap();
    let (x, y) = trivial.coordinates().unwrap();
    let coordinates = [x, y]
        .into_iter()
        .flat_map(|v| foreign_limbs(&v).map(Fp::from_u128))
        .collect::<Vec<_>>();
    for _ in 0..2 {
        out.extend(coordinates.clone());
        out.extend([Fp::ONE; 16]);
    }
    out.extend([Fp::ZERO, Fp::ONE, Fp::ZERO]);
    out.extend(coordinates);
    out
}
fn satisfied(circuit: &Recursive, instances: &[Vec<Fp>]) -> bool {
    check_circuit(circuit, 16, instances, CheckMode::Strict)
        .map(|r| r.is_satisfied())
        .unwrap_or(false)
}

fn fixture() -> Fixture {
    let sigma = common::vesta_prover(common::pinned_shape(
        common::folded(SigmaRelation::SEND),
        (12, 1),
    ));
    let witness = sample_witness::<Fp>(common::CHECK_SEED + 2, SigmaRelation::SEND, Mutation::None);
    let proof = sigma.prove(&witness, common::recovery(71)).unwrap();
    let verifier = sigma.verifier();
    let class = SigmaClass::from_verifiers(&[(5, &verifier)]).unwrap();
    let params = common::vesta_params(16);
    let plan = QSigmaPlan::new(class, None, &params).unwrap();
    let slot = SigmaSlotWitness {
        key: verifier.vk().clone(),
        statement: proof.public.statement,
        proof: proof.bytes.clone(),
        length: u32::try_from(proof.bytes.len()).unwrap(),
    };
    let prepared = plan
        .prepare(slot, None, &params, Fq::from(17), &FoldConfig::default())
        .unwrap();
    let pallas_params = PinnedParams::<Ep>::derive(16).unwrap();
    let prover = QSigmaProver::keygen(&prepared, pallas_params.clone()).unwrap();
    let q = prover
        .prove(&prepared, common::recovery(72), ProverConfig::default())
        .unwrap();
    let claim = accumulate_generator(
        &pallas_params,
        prover.binding(),
        prover.verifying_key(),
        &q.instances,
        &q.bytes,
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    let pallas = FoldInput::from_opening(*claim.g(), claim.challenges()).unwrap();
    pallas
        .decide(&pallas_params, MemoryBudget::DEFAULT)
        .unwrap();
    let fixed = QProofPlan::new(
        VerifierPlan::new(prover.binding().clone(), pallas_params.clone()).unwrap(),
        prover.verifying_key().clone(),
    )
    .unwrap();
    let aplan = AProofPlan::new(
        Variant::Bootstrap,
        plan.clone(),
        vec![fixed.clone()],
        None,
        &pallas_params,
    )
    .unwrap();
    assert!(
        AProofPlan::new(
            Variant::Bootstrap,
            plan.clone(),
            vec![fixed.clone(), fixed.clone()],
            None,
            &pallas_params
        )
        .is_err()
    );
    assert!(
        AProofPlan::new(
            Variant::Send,
            plan.clone(),
            vec![fixed.clone()],
            None,
            &pallas_params
        )
        .is_err()
    );
    assert_eq!(aplan.q_count(), 1);
    assert!(aplan.omega().is_none());
    assert!(aplan.q(1).is_none());
    assert_eq!(aplan.frame().variant(), Variant::Bootstrap);
    let mut fields = core::array::from_fn(|i| Fp::from(u64::try_from(i + 1).unwrap()));
    fields[0] = Fp::ONE;
    let native = witness.evaluate(SigmaRelation::SEND);
    let statement = native.statement;
    let sigma_bytes = u32::try_from(proof.bytes.len())
        .unwrap()
        .to_le_bytes()
        .into_iter()
        .chain(proof.bytes)
        .collect();
    let circuit = Recursive {
        plan: aplan,
        q_length: u32::try_from(q.bytes.len()).unwrap(),
        q: q.bytes,
        instances: q.instances,
        sigma: sigma_bytes,
        statement,
        fields,
        known: true,
        route: 0,
        predecessor: None,
    };
    let values = [public(&fields, &pallas, &q.part)];
    Fixture {
        circuit,
        values,
        sigma_plan: plan,
        q_plan: fixed,
        q_opening: pallas,
        part: q.part,
        params: pallas_params,
    }
}
#[test]
#[ignore = "actual k16 Q proof composed into A; run optimized with --include-ignored"]
fn bootstrap_frame_hard_verifies_real_q_and_keeps_its_opening() {
    let Fixture {
        circuit, values, ..
    } = fixture();
    assert_eq!(values[0].len(), AFramePlan::instance_length());
    let honest = check_circuit(&circuit, 16, &values, CheckMode::Strict).unwrap_or_else(|error| {
        let diagnostic = synthesize(&circuit, 17, Some(&values)).expect("diagnostic layout");
        let rows = diagnostic
            .tables
            .advice_assigned()
            .iter()
            .map(|column| column.iter().rposition(|v| *v).map_or(0, |r| r + 1))
            .collect::<Vec<_>>();
        let report =
            iroha_plonk::check::check(&diagnostic.cs, &diagnostic.tables, CheckMode::Strict)
                .unwrap();
        panic!(
            "k16 capacity failure {error:?}; diagnostic k17 rows={rows:?} predicate_pass={}",
            report.is_satisfied()
        );
    });
    assert!(
        honest.is_satisfied(),
        "{:?}",
        &honest.failures()[..honest.failures().len().min(8)]
    );
    let known = synthesize(&circuit, 16, Some(&values)).unwrap();
    let unknown = synthesize(&circuit.without_witnesses(), 16, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(
        known.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
    let rows = known
        .tables
        .advice_assigned()
        .iter()
        .filter_map(|column| column.iter().rposition(|v| *v).map(|r| r + 1))
        .max()
        .unwrap();
    let lanes = known
        .tables
        .advice_assigned()
        .iter()
        .map(|c| c.iter().rposition(|v| *v).map_or(0, |r| r + 1))
        .collect::<Vec<_>>();
    println!("A actualQ Bootstrap frame rows={rows} lanes={lanes:?}");
    for route in [1, 2] {
        let mut bad = circuit.clone();
        bad.route = route;
        assert!(!satisfied(&bad, &values));
    }
    let mut bad = circuit.clone();
    bad.q_length -= 1;
    assert!(!satisfied(&bad, &values));
    let mut bad = circuit.clone();
    bad.q[100] ^= 1;
    assert!(!satisfied(&bad, &values));
    let mut bad = circuit.clone();
    bad.sigma[100] ^= 1;
    assert!(!satisfied(&bad, &values));
    let mut bad = circuit;
    bad.statement[3] += Fp::ONE;
    assert!(!satisfied(&bad, &values));
}

// An actual k16 proof with Omega's typed schema, deliberately isolating the
// consumer contract. This fixture does not assert the production Omega relation.
#[derive(Clone)]
struct OmegaFrame {
    instances: Vec<Vec<Fq>>,
    known: bool,
}
impl Circuit<Fq> for OmegaFrame {
    type Config = (GlueConfig, [Column<Instance>; 3]);
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fq>) -> Self::Config {
        let advice = core::array::from_fn(|_| meta.advice_column());
        let constants = meta.fixed_column();
        let glue = GlueConfig::configure(meta, advice, constants);
        let public = [
            meta.instance_column(1),
            meta.instance_column(2),
            meta.instance_column(16),
        ];
        for column in public {
            meta.enable_equality(column);
        }
        (glue, public)
    }
    fn synthesize(
        &self,
        (config, public): Self::Config,
        mut layouter: impl Layouter<Fq>,
    ) -> Result<(), Error> {
        let mut glue = GlueChip::new(config);
        let words = layouter.assign_region(
            || "Omega consumer fixture",
            |mut region| {
                self.instances
                    .iter()
                    .flatten()
                    .map(|v| {
                        glue.witness(
                            &mut region,
                            if self.known {
                                Value::known(*v)
                            } else {
                                Value::unknown()
                            },
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()
            },
        )?;
        let mut offset = 0;
        for (column, values) in public.iter().zip(&self.instances) {
            for row in 0..values.len() {
                layouter.constrain_instance(words[offset + row].cell(), *column, row)?;
            }
            offset += values.len();
        }
        Ok(())
    }
}
fn digest(fields: &[Fp; 18], pallas: &FoldInput<Ep>) -> Fp {
    let mut words = fields.to_vec();
    let (x, y) = pallas.g().coordinates().unwrap();
    words.extend([x, y]);
    for u in pallas.challenges() {
        words.extend(foreign_limbs(u).map(Fp::from_u128));
    }
    hash_with_domain(LINEAGE_DOMAIN, &words)
}
#[test]
#[ignore = "actual predecessor and Q proofs with hard Pallas fold; run optimized"]
fn send_frame_binds_predecessor_key_and_both_generator_obligations() {
    let mut base = fixture();
    let trivial = AccumulatorT::trivial(&base.params, MemoryBudget::DEFAULT)
        .unwrap()
        .as_input();
    let vparams = common::vesta_params(16);
    let vtrivial = AccumulatorT::trivial(&vparams, MemoryBudget::DEFAULT).unwrap();
    let (x, y) = vtrivial.g().coordinates().unwrap();
    let mut predecessor = OmegaFrame {
        instances: vec![vec![Fq::ZERO], vec![x, y], vec![Fq::ONE; 16]],
        known: true,
    };
    let config = KeygenConfigV2::pipa_r(vec![
        InstanceType::Bounded,
        InstanceType::Field,
        InstanceType::Bounded,
    ]);
    let key = keygen_pk_v2(&base.params, &predecessor, &config).unwrap();
    let key_digest = key.vk().kagemusha_digest(key.binding()).unwrap();
    base.circuit.fields[17] = key_digest;
    let mut fields = base.circuit.fields;
    fields[5] += Fp::ONE;
    predecessor.instances[0][0] = Fq::from_repr(digest(&fields, &trivial).to_repr()).unwrap();
    let witness = Witness::from_circuit(&key, &predecessor, &predecessor.instances).unwrap();
    let proof = create_proof_owned_with_claim(
        &base.params,
        &key,
        witness,
        common::recovery(73),
        ProverConfig::default(),
    )
    .unwrap();
    let opening = FoldInput::from_opening(*proof.opening.g(), proof.opening.challenges()).unwrap();
    let inputs = [trivial.clone(), opening, base.q_opening];
    let (fold, output) = create_fold(
        &base.params,
        &inputs,
        Fp::from(29).to_repr(),
        &FoldConfig::default(),
    )
    .unwrap();
    output.decide(&base.params, MemoryBudget::DEFAULT).unwrap();
    let omega = VerifierPlan::new(key.binding().clone(), base.params.clone()).unwrap();
    base.circuit.plan = AProofPlan::new(
        Variant::Send,
        base.sigma_plan,
        vec![base.q_plan],
        Some(omega),
        &base.params,
    )
    .unwrap();
    base.circuit.predecessor = Some(Predecessor {
        key: key.vk().clone(),
        fields,
        pallas: trivial,
        proof: proof.proof.clone(),
        length: u32::try_from(proof.proof.len()).unwrap(),
        fold: fold.to_bytes().to_vec(),
        fold_length: 1120,
    });
    let values = [public(&base.circuit.fields, &output.as_input(), &base.part)];
    let honest =
        check_circuit(&base.circuit, 16, &values, CheckMode::Strict).unwrap_or_else(|error| {
            let diagnostic =
                synthesize(&base.circuit, 18, Some(&values)).expect("diagnostic Send layout");
            let lanes = diagnostic
                .tables
                .advice_assigned()
                .iter()
                .map(|c| c.iter().rposition(|v| *v).map_or(0, |r| r + 1))
                .collect::<Vec<_>>();
            let report =
                iroha_plonk::check::check(&diagnostic.cs, &diagnostic.tables, CheckMode::Strict)
                    .unwrap();
            panic!(
                "Send k16 capacity failure {error:?}; diagnostic lanes={lanes:?} predicate_pass={}",
                report.is_satisfied()
            );
        });
    assert!(
        honest.is_satisfied(),
        "{:?}",
        &honest.failures()[..honest.failures().len().min(8)]
    );
    for mutation in 0..6 {
        let mut bad = base.circuit.clone();
        match mutation {
            0 => bad.predecessor = None,
            1 => bad.fields[17] += Fp::ONE,
            2 => bad.predecessor.as_mut().unwrap().fields[17] += Fp::ONE,
            3 => bad.predecessor.as_mut().unwrap().proof[100] ^= 1,
            4 => bad.predecessor.as_mut().unwrap().fold[100] ^= 1,
            _ => bad.predecessor.as_mut().unwrap().length -= 1,
        }
        assert!(!satisfied(&bad, &values), "mutation {mutation}");
    }
}
