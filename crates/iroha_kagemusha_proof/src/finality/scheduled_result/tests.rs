//! Original-cell binding tests, not substitutes for either source proof or history.

use super::*;
use crate::finality::{
    certificate::CertificateStatement, schedule::source::prepare_schedule_source,
};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Layouter, SimpleFloorPlanner, Value, synthesize},
};
use iroha_plonk_recursion::verifier::VerifierConfig;

#[derive(Clone)]
struct Link {
    input: ScheduledResultInput,
    sources: [[Fp; 6]; 2],
    known: bool,
}
impl Link {
    fn value<T: Copy>(&self, value: T) -> Value<T> {
        if self.known {
            Value::known(value)
        } else {
            Value::unknown()
        }
    }
    fn words<const N: usize>(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        values: [Fp; N],
    ) -> Result<[Word<Fp>; N], Error> {
        chip.uint()
            .glue()
            .witnesses(region, &values.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| Error::Synthesis)
    }
}
#[derive(Clone)]
struct Config {
    verifier: VerifierConfig<Ep>,
    public: Column<Instance>,
}
impl Circuit<Fp> for Link {
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
        let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, 3).unwrap();
        let public = meta.instance_column(1);
        meta.enable_equality(public);
        Config { verifier, public }
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let digest = layouter.assign_region(
            || "original scheduled result linkage",
            |mut region| {
                let sources = [
                    self.words(&mut chip, &mut region, self.sources[0])?,
                    self.words(&mut chip, &mut region, self.sources[1])?,
                ];
                let sources = [
                    SourceEndpoints::from_words(&mut chip, &mut region, &sources[0])?,
                    SourceEndpoints::from_words(&mut chip, &mut region, &sources[1])?,
                ];
                let certificate = CertificateStatementCells::assign(
                    &mut chip,
                    &mut region,
                    self.value(self.input.certified.certificate),
                )?;
                let [root] = self.words(&mut chip, &mut region, [self.input.certified.root])?;
                let len = chip.uint().assign::<32>(
                    &mut region,
                    self.value(u128::from(self.input.certified.frame_len)),
                )?;
                let schedule = ScheduleSourceBinding::assign(
                    &mut chip,
                    &mut region,
                    &self.value(self.input.schedule),
                )?;
                let linked = ScheduledResultLinkCells::constrain(
                    &mut chip,
                    &mut region,
                    [&sources[0], &sources[1]],
                    &certificate,
                    &root,
                    &len,
                    &schedule,
                )?;
                // The compact native prediction has independently assigned bounded
                // cells, and must match every field exported by the original link.
                let statement = ScheduledResultStatementCells::assign(
                    &mut chip,
                    &mut region,
                    self.value(self.input.statement()),
                )?;
                GlueChip::assert_equal(&mut region, statement.digest(), linked.digest())?;
                for (actual, expected) in [
                    (statement.network(), linked.statement().network()),
                    (statement.instance(), linked.statement().instance()),
                    (statement.context(), linked.statement().context()),
                    (statement.result(), linked.statement().result()),
                ] {
                    for (actual, expected) in actual.iter().zip(expected) {
                        GlueChip::assert_equal(&mut region, actual, expected)?;
                    }
                }
                for (actual, expected) in [
                    (statement.epoch().word(), linked.statement().epoch().word()),
                    (
                        statement.height().word(),
                        linked.statement().height().word(),
                    ),
                    (statement.tape_root(), linked.statement().tape_root()),
                    (
                        statement.frame_len().word(),
                        linked.statement().frame_len().word(),
                    ),
                ] {
                    GlueChip::assert_equal(&mut region, actual, expected)?;
                }
                Ok(linked.digest().clone())
            },
        )?;
        layouter.constrain_instance(digest.cell(), config.public, 0)
    }
}
fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}
fn fixture() -> Link {
    let json: norito::json::Value = norito::json::from_str(include_str!(
        "../../../../../fixtures/kagemusha/ordinary_load_receipt_v1.json"
    ))
    .unwrap();
    let bytes = |name: &str| hex(json.get(name).unwrap().as_str().unwrap());
    let message: [u8; 165] = bytes("commit_vote_preimage_hex").try_into().unwrap();
    let id = core::array::from_fn(|i| message[53 + i]);
    let source = prepare_schedule_source(bytes("result_preimage_hex"), false, id).unwrap();
    let schedule = *source[0].input();
    let input = ScheduledResultInput {
        certified: CertifiedResultContext {
            certificate: CertificateStatement {
                roster_root: schedule.roster_root,
                members: schedule.projection.members,
                faults: schedule.projection.faults,
                message,
            },
            root: schedule.epoch_hash.tape_root,
            frame_len: schedule.epoch_hash.result_len,
        },
        schedule,
    };
    Link {
        sources: input.source_endpoints(),
        input,
        known: true,
    }
}
fn accepts(circuit: &Link) -> bool {
    accepts_public(circuit, circuit.input.statement().digest())
}
fn accepts_public(circuit: &Link, digest: Fp) -> bool {
    check_circuit(circuit, 16, &[vec![digest]], CheckMode::Strict).is_ok_and(|r| r.is_satisfied())
}

#[test]
fn original_commit_current_schedule_and_compact_statement_match() {
    let honest = fixture();
    assert!(accepts(&honest));
    let statement = honest.input.statement();
    assert_eq!(statement.network, honest.input.schedule.projection.network);
    assert_eq!(
        statement.context,
        honest.input.schedule.epoch_hash.context_id
    );
    assert_eq!(statement.epoch, honest.input.schedule.projection.epoch);
    assert_eq!(statement.height, honest.input.schedule.height);
    let known = synthesize(&honest, 16, None).unwrap();
    let unknown = synthesize(&honest.without_witnesses(), 16, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.selectors(), unknown.tables.selectors());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
    assert_eq!(
        known.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
}

#[test]
fn only_both_exact_complete_source_endpoints_are_accepted() {
    let honest = fixture();
    for side in 0..2 {
        for field in 0..6 {
            let mut changed = honest.clone();
            changed.sources[side][field] += Fp::ONE;
            assert!(!accepts(&changed), "source {side} field {field}");
        }
    }
}

#[test]
fn consistent_child_commitments_cannot_change_role_tape_or_current_authority() {
    let honest = fixture();
    for mutation in 0..11 {
        let mut changed = honest.clone();
        match mutation {
            0 => changed.input.schedule.authorized = true,
            1 => changed.input.certified.root += Fp::ONE,
            2 => changed.input.certified.frame_len += 1,
            3 => changed.input.certified.certificate.roster_root += Fp::ONE,
            4 => {
                changed.input.certified.certificate.members = 7;
                changed.input.certified.certificate.faults = 2;
            }
            5 => changed.input.certified.certificate.message[52] ^= 1,
            6 => changed.input.certified.certificate.message[53] ^= 1,
            7 => changed.input.certified.certificate.message[92] ^= 1,
            8 => changed.input.schedule.projection.first = changed.input.schedule.height + 1,
            9 => changed.input.schedule.projection.last = changed.input.schedule.height - 1,
            _ => changed.input.schedule.height += 1,
        }
        changed.sources = changed.input.source_endpoints();
        assert!(!accepts(&changed), "cross-source substitution {mutation}");
    }
}

#[test]
fn compact_output_retains_network_instance_epoch_height_context_result_and_tape() {
    let honest = fixture();
    let original = honest.input.statement().digest();
    for mutation in 0..8 {
        let mut changed = honest.clone();
        match mutation {
            0 => changed.input.schedule.projection.network[0] ^= 1,
            1 => changed.input.certified.certificate.message[13] ^= 1,
            2 => {
                changed.input.certified.certificate.message[52] ^= 1;
                changed.input.schedule.projection.epoch ^= 1;
            }
            3 => {
                let height = changed.input.schedule.height + 1;
                changed.input.certified.certificate.message[85..93]
                    .copy_from_slice(&height.to_be_bytes());
                changed.input.schedule.height = height;
                changed.input.schedule.projection.last = height;
            }
            4 => {
                changed.input.certified.certificate.message[53] ^= 1;
                changed.input.schedule.epoch_hash.context_id[0] ^= 1;
            }
            5 => changed.input.certified.certificate.message[133] ^= 1,
            6 => {
                changed.input.certified.root += Fp::ONE;
                changed.input.schedule.epoch_hash.tape_root = changed.input.certified.root;
            }
            _ => {
                changed.input.certified.frame_len += 1;
                changed.input.schedule.epoch_hash.result_len += 1;
            }
        }
        changed.sources = changed.input.source_endpoints();
        assert_ne!(changed.input.statement().digest(), original);
        assert!(
            !accepts_public(&changed, original),
            "output identity {mutation}"
        );
    }
}
