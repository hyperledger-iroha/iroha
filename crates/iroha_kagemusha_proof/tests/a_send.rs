//! Genuine Send sigma and same-tape ownership, Request, fee and depth32 maps.
//! These component tests do not substitute for the hard predecessor/Q A schedule.

#[path = "common/bootstrap.rs"]
mod bootstrap;
#[path = "common/bootstrap_objects.rs"]
#[allow(dead_code)] // Shared genuine object signing support.
mod bootstrap_objects;
mod common;
#[path = "common/load_objects.rs"]
#[allow(dead_code)] // The component consumes Load state, not its receipt proof.
mod load_objects;
#[path = "common/send_objects.rs"]
mod send_objects;

use ff::{Field, PrimeField};
use iroha_kagemusha_proof::{
    SigmaProver, SigmaRelation,
    a_relation::{
        LineagePublicCells, SigmaBindingCells,
        send::{SendInputs, SendObjects},
    },
    admin_sigma::StateWitness,
    operation_relation::{
        map_effects::{InsertCells, MapState},
        objects::ObjectKind,
        state::StateCells,
        statement::StatementCells,
    },
    tree::IndexedInsert,
};
use iroha_pasta::{Ep, Eq, Fp, msm::MemoryBudget, poseidon::hash_with_domain};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value, synthesize},
    verifier::accumulate_generator,
};
use iroha_plonk_gadgets::{
    UintChip,
    bytes::{
        p_bytes_native,
        tape::{BytesChip, BytesConfig},
    },
    imt::{LeafCells, OpeningCells, PathCells},
};
use iroha_plonk_recursion::{
    obligation::ledger::Variant,
    verifier::{VerifierChip, VerifierConfig},
};

#[derive(Clone)]
struct SendMaps {
    witness: send_objects::SendFixture,
    known: bool,
}
#[derive(Clone, Debug)]
struct Config {
    verifier: VerifierConfig<Ep>,
    bytes: BytesConfig,
    public: Column<Instance>,
}
impl SendMaps {
    fn value<T: Copy>(&self, v: T) -> Value<T> {
        if self.known {
            Value::known(v)
        } else {
            Value::unknown()
        }
    }
    fn state(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        witness: &StateWitness,
    ) -> Result<(StateCells, LineagePublicCells), Error> {
        let core = chip
            .uint()
            .glue()
            .witnesses(region, &witness.core.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let rest = chip
            .uint()
            .glue()
            .witnesses(region, &witness.rest.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let state = StateCells::constrain_with_verifier(chip, region, &core, &rest)?;
        let public = chip
            .uint()
            .glue()
            .witnesses(region, &witness.lineage.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let public = LineagePublicCells::constrain(&mut chip.uint(), region, &public)?;
        Ok((state, public))
    }
    fn insertion(
        &self,
        uint: &mut UintChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        witness: &IndexedInsert<Fp>,
    ) -> Result<InsertCells, Error> {
        let leaf = witness.leaf;
        let words = uint
            .glue()
            .witnesses(
                region,
                &[leaf.key, leaf.value, leaf.next_key].map(|v| self.value(v)),
            )?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let leaf = LeafCells::from_words(words);
        let mut paths = Vec::new();
        for (index, siblings) in [
            (witness.leaf_slot, witness.leaf_siblings),
            (witness.slot, witness.slot_siblings),
        ] {
            let index = uint
                .glue()
                .witness(region, self.value(Fp::from(u64::from(index))))?;
            let siblings = uint
                .glue()
                .witnesses(region, &siblings.map(|v| self.value(v)))?
                .try_into()
                .map_err(|_| Error::Synthesis)?;
            paths.push(PathCells::from_words(uint, region, &index, siblings)?);
        }
        let [low, slot] = paths.try_into().map_err(|_| Error::Synthesis)?;
        Ok(InsertCells {
            low: OpeningCells { leaf, path: low },
            slot,
        })
    }
    fn public(&self) -> Vec<Vec<Fp>> {
        vec![
            [
                ObjectKind::Credential,
                ObjectKind::Request,
                ObjectKind::FeeSchedule,
            ]
            .into_iter()
            .zip(&self.witness.objects)
            .map(|(kind, bytes)| {
                let end = kind.body_len();
                let mut fields = vec![p_bytes_native(kind.signing_domain(), &bytes[..end])];
                for offset in [16, 0, 48, 32] {
                    fields.push(Fp::from_u128(u128::from_be_bytes(
                        bytes[end + offset..end + offset + 16].try_into().unwrap(),
                    )));
                }
                hash_with_domain(kind.object_domain(), &fields)
            })
            .collect(),
        ]
    }
}
impl Circuit<Fp> for SendMaps {
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
        let verifier = VerifierConfig::configure_serialized_foreign(meta, 5).unwrap();
        let a = meta.advice_column();
        let b = meta.advice_column();
        let bytes = BytesConfig::configure(meta, a, b);
        let public = meta.instance_column(3);
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
            || "Send same tape objects and maps",
            |mut region| {
                let (before, pred) = self.state(&mut chip, &mut region, &self.witness.before)?;
                let (after, next) = self.state(&mut chip, &mut region, &self.witness.after)?;
                let fields = chip
                    .uint()
                    .glue()
                    .witnesses(&mut region, &self.witness.statement.map(|v| self.value(v)))?
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                let statement = StatementCells::constrain_with_verifier(
                    &mut chip,
                    &mut region,
                    Variant::Send,
                    &fields,
                )?;
                // The actual recursive Q binder derives this selector from the same
                // state. This component only consumes the sigma's statement.
                let index = chip.uint().glue().constant(&mut region, Fp::ZERO)?;
                let sigma = SigmaBindingCells::from_statement(&statement, index, vec![]);
                let sources = self
                    .witness
                    .objects
                    .each_ref()
                    .map(|o| o.iter().map(|v| self.value(*v)).collect::<Vec<_>>());
                let objects = SendObjects::decode(
                    &mut chip,
                    &mut bytes,
                    &mut region,
                    sources.each_ref().map(Vec::as_slice),
                )?;
                let pending =
                    self.insertion(&mut chip.uint(), &mut region, &self.witness.pending)?;
                let fee = self.insertion(&mut chip.uint(), &mut region, &self.witness.fee)?;
                objects.constrain(
                    &mut chip,
                    &mut region,
                    SendInputs {
                        predecessor: MapState {
                            state: &before,
                            lineage: &pred,
                        },
                        successor: MapState {
                            state: &after,
                            lineage: &next,
                        },
                        sigma: &sigma,
                    },
                    &pending,
                    &fee,
                )?;
                Ok(objects
                    .context()
                    .iter()
                    .map(|o| o.authenticated_digest().clone())
                    .collect::<Vec<_>>())
            },
        )?;
        for (i, value) in out.iter().enumerate() {
            layouter.constrain_instance(value.cell(), config.public, i)?;
        }
        Ok(())
    }
}
fn fixture() -> SendMaps {
    let (bootstrap, _, _) = bootstrap_objects::enrollment();
    let (load, _, _, _) = load_objects::authorized(&bootstrap);
    SendMaps {
        witness: send_objects::from_load(&load.successor),
        known: true,
    }
}
#[test]
fn genuine_send_sigma_and_same_tape_depth32_maps() {
    let circuit = fixture();
    let shape = common::pinned_shape(common::folded(SigmaRelation::SEND), (12, 1));
    let prover = SigmaProver::<Eq>::keygen_with_params(shape, common::vesta_params(12)).unwrap();
    let sigma = prover
        .prove(&circuit.witness.step, common::recovery(221))
        .unwrap();
    let opening = accumulate_generator(
        prover.params(),
        prover.proving_key().binding(),
        prover.proving_key().vk(),
        &[sigma.public.instance()],
        &sigma.bytes,
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    opening
        .decide(prover.params(), MemoryBudget::DEFAULT)
        .unwrap();
    let report = check_circuit(&circuit, 16, &circuit.public(), CheckMode::Strict).unwrap();
    assert!(report.is_satisfied(), "{report:?}");
    let known = synthesize(&circuit, 16, None).unwrap();
    let unknown = synthesize(&circuit.without_witnesses(), 16, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(
        known.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
    let lanes: Vec<_> = known
        .tables
        .advice_assigned()
        .iter()
        .map(|c| c.iter().rposition(|v| *v).map_or(0, |i| i + 1))
        .collect();
    eprintln!(
        "actual Send mask0 sigma={}B, five-bus objects/D32 lanes={lanes:?}; hard predecessor/Q closure not in this component",
        sigma.bytes.len()
    );
    for mutation in 0..8 {
        let mut wrong = circuit.clone();
        match mutation {
            0 => wrong.witness.pending.slot_siblings[31] += Fp::ONE,
            1 => wrong.witness.pending.leaf_siblings[0] += Fp::ONE,
            2 => wrong.witness.pending.slot = 0,
            3 => wrong.witness.after.core[16] += Fp::ONE,
            4 => wrong.witness.statement[23] += Fp::ONE,
            5 => wrong.witness.objects[0][98] ^= 1, // Payer account, with recomputed public digest.
            6 => wrong.witness.objects[1][98] ^= 1, // Payer wallet in the Request.
            _ => wrong.witness.objects[1][ObjectKind::Request.body_len()] ^= 1,
        }
        assert!(
            !check_circuit(&wrong, 16, &wrong.public(), CheckMode::Strict)
                .is_ok_and(|r| r.is_satisfied()),
            "Send mutation{mutation}"
        );
    }
    // A zero held fee digest deliberately permits arbitrary bytes in its fixed
    // total dummy slot, while their exact object commitment remains public.
    let mut unused = circuit;
    unused.witness.objects[2].fill(255);
    assert!(
        check_circuit(&unused, 16, &unused.public(), CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
}

#[test]
fn send_held_fee_cannot_skip_or_replace_either_map_obligation() {
    let (bootstrap, _, _) = bootstrap_objects::enrollment();
    let (load, _, _, _) = load_objects::authorized(&bootstrap);
    let circuit = SendMaps {
        witness: send_objects::with_held_fee(&load.successor),
        known: true,
    };
    assert!(
        common::check_witness(
            &common::pinned_shape(common::folded(SigmaRelation::SEND), (12, 1)),
            &circuit.witness.step
        )
        .is_satisfied()
    );
    let report = check_circuit(&circuit, 16, &circuit.public(), CheckMode::Strict).unwrap();
    assert!(report.is_satisfied(), "{report:?}");
    for mutation in 0..7 {
        let mut wrong = circuit.clone();
        match mutation {
            0 => wrong.witness.fee = send_objects::from_load(&load.successor).fee,
            1 => wrong.witness.fee.slot_siblings[31] += Fp::ONE,
            2 => wrong.witness.fee.slot = wrong.witness.fee.leaf_slot,
            3 => wrong.witness.objects[2][ObjectKind::FeeSchedule.body_len()] ^= 1,
            4 => wrong.witness.objects[1][258] ^= 1, // Different historical schedule.
            5 => wrong.witness.after.core[19] = wrong.witness.before.core[19],
            _ => wrong.witness.after.core[17] = wrong.witness.before.core[17],
        }
        assert!(
            !check_circuit(&wrong, 16, &wrong.public(), CheckMode::Strict)
                .is_ok_and(|r| r.is_satisfied()),
            "Send fee mutation{mutation}"
        );
    }
}
