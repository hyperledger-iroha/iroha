//! Native4/31-seat source fixtures across preboundary, boundary and successor R.

use super::{
    authorization::AuthorizationRanges,
    decode::ScheduleReader,
    epoch::EpochRanges,
    graph::{BoundaryRanges, ScheduleRanges, SlotCells},
    tape::{ResultTape, ResultTapeWitness},
};
use ff::Field;
use iroha_pasta::Fp;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner, Value},
};
use iroha_plonk_gadgets::{
    GlueChip, GlueConfig, LimbBits, RunningSumChip, RunningSumConfig, SpongeChip, SpongeConfig,
    UintChip,
    poseidon::{Pow5Columns, RoundConstantColumns},
};

const OUTPUTS: usize = 256;

#[derive(Clone)]
struct ScheduleCircuit {
    frame: Vec<u8>,
    phase: u8,
    seat: u8,
    known: bool,
}
#[derive(Clone)]
struct Config {
    glue: GlueConfig,
    range: RunningSumConfig,
    hash: SpongeConfig<Fp>,
    public: Column<Instance>,
}
fn witness<T>(known: bool, value: T) -> Value<T> {
    if known {
        Value::known(value)
    } else {
        Value::unknown()
    }
}

impl Circuit<Fp> for ScheduleCircuit {
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
        let advice = core::array::from_fn(|_| meta.advice_column());
        let constants = meta.fixed_column();
        let glue = GlueConfig::configure(meta, advice, constants);
        let range_column = meta.advice_column();
        let range = RunningSumConfig::configure(meta, range_column, LimbBits::new(8).unwrap());
        let columns = Pow5Columns::allocate(meta);
        let constants = RoundConstantColumns::allocate(meta);
        let hash = SpongeConfig::configure(meta, columns, constants, &[]);
        let public = meta.instance_column(OUTPUTS);
        meta.enable_equality(public);
        Config {
            glue,
            range,
            hash,
            public,
        }
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut glue = GlueChip::new(config.glue);
        let mut range = RunningSumChip::new(config.range);
        range.load_table(&mut layouter)?;
        let mut hash = SpongeChip::new(config.hash);
        let source = ResultTapeWitness::from_frame(&witness(self.known, self.frame.clone()))?;
        let output = layouter.assign_region(
            || "native schedule parser stages",
            |mut region| {
                let root = glue.witness(&mut region, source.root())?;
                let mut uint = UintChip::new(&mut glue, &mut range);
                let length =
                    uint.assign::<32>(&mut region, witness(self.known, self.frame.len() as u128))?;
                let tape = ResultTape::new(&mut uint, &mut region, &root, &length)?;
                let mut reader = ScheduleReader {
                    tape: &tape,
                    witness: &source,
                    uint: &mut uint,
                    hash: &mut hash,
                };
                let schedule = ScheduleRanges::parse(&mut reader, &mut region)?;
                let mut out = vec![root, schedule.height().word().clone()];
                if self.phase == 0 {
                    let boundary = BoundaryRanges::parse(&mut reader, &mut region, &schedule)?;
                    out.push(boundary.present().word().clone());
                    out.push(boundary.height().word().clone());
                    out.extend_from_slice(boundary.predecessor());
                    out.extend_from_slice(boundary.selection_anchor());
                    let next = SlotCells::parse(&mut reader, &mut region, schedule.next())?;
                    let after_next =
                        SlotCells::parse(&mut reader, &mut region, schedule.after_next())?;
                    schedule.bind_slots(&mut reader, &mut region, &next, &after_next)?;
                    for slot in [&next, &after_next] {
                        out.extend([
                            slot.pending().word().clone(),
                            slot.height().word().clone(),
                            slot.boundary_height().word().clone(),
                        ]);
                        out.extend_from_slice(slot.predecessor());
                        out.extend(
                            slot.params()
                                .values()
                                .iter()
                                .map(|value| value.word().clone()),
                        );
                    }
                } else {
                    let body = if self.phase == 2 {
                        let boundary = BoundaryRanges::parse(&mut reader, &mut region, &schedule)?;
                        boundary.authorized(&mut reader, &mut region, &schedule)?
                    } else {
                        schedule.current().clone()
                    };
                    let epoch = EpochRanges::parse(&mut reader, &mut region, &body)?;
                    let header = epoch.header(&mut reader, &mut region)?;
                    let authorization =
                        AuthorizationRanges::parse(&mut reader, &mut region, &epoch)?;
                    let scalars = authorization.scalars(&mut reader, &mut region)?;
                    out.extend([
                        header.count().word().clone(),
                        header.faults().word().clone(),
                        header.mode().word().clone(),
                        scalars.epoch().word().clone(),
                        scalars.first().word().clone(),
                        scalars.last().word().clone(),
                        scalars.generation().word().clone(),
                        scalars.decision().word().clone(),
                    ]);
                    out.extend_from_slice(header.network());
                    out.extend_from_slice(header.seed());
                    if self.phase == 1 {
                        let seat = reader
                            .uint
                            .assign::<5>(&mut region, witness(self.known, u128::from(self.seat)))?;
                        let member = epoch.member(&mut reader, &mut region, &header, &seat)?;
                        out.push(member.active().word().clone());
                        out.push(seat.word().clone());
                        out.extend_from_slice(member.key());
                        out.extend_from_slice(member.proof_of_possession());
                    } else {
                        let identities =
                            authorization.identities(&mut reader, &mut region, &epoch, &header)?;
                        out.extend_from_slice(identities.authority());
                        out.extend_from_slice(identities.previous());
                        out.extend_from_slice(identities.transition());
                        let beacon = authorization.beacon(&mut reader, &mut region)?;
                        out.push(beacon.installed().word().clone());
                        out.extend_from_slice(beacon.session());
                        out.extend_from_slice(beacon.transcript());
                    }
                }
                if out.len() > OUTPUTS {
                    return Err(Error::Synthesis);
                }
                let zero = reader.uint.glue().constant(&mut region, Fp::ZERO)?;
                out.resize(OUTPUTS, zero);
                Ok(out)
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}
fn bytes(value: &norito::json::Value, key: &str) -> Vec<u8> {
    hex(value.get(key).unwrap().as_str().unwrap())
}
fn num(value: &norito::json::Value, key: &str) -> u64 {
    value.get(key).unwrap().as_u64().unwrap()
}
fn fixture() -> norito::json::Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/kagemusha/ordinary_load_npos_schedule_v1.json");
    norito::json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

// Independent test-only canonical field splitter for the production-captured
// authorization/parameter payloads. It supplies expected values, never witnesses
// to the circuit parser or a native acceptance verdict.
fn fields(bytes: &[u8]) -> Vec<&[u8]> {
    let mut pos = 0;
    let mut out = Vec::new();
    while pos < bytes.len() {
        let mut len = 0;
        let mut shift = 0;
        loop {
            let byte = bytes[pos];
            pos += 1;
            len |= usize::from(byte & 127) << shift;
            if byte < 128 {
                break;
            }
            shift += 7;
        }
        out.push(&bytes[pos..pos + len]);
        pos += len;
    }
    out
}
fn le(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .rev()
        .fold(0, |sum, b| sum * 256 + u64::from(*b))
}
fn append_bytes(out: &mut Vec<Fp>, bytes: &[u8]) {
    out.extend(bytes.iter().map(|b| Fp::from(u64::from(*b))));
}

fn expected(circuit: &ScheduleCircuit, schedule: &norito::json::Value) -> Vec<Vec<Fp>> {
    let witness = ResultTapeWitness::from_frame(&Value::known(circuit.frame.clone())).unwrap();
    let mut out = Vec::new();
    let _ = witness.root().map(|root| out.push(root));
    out.push(Fp::from(num(schedule, "height")));
    let boundary = schedule.get("boundary").unwrap();
    let has_boundary = !matches!(boundary, norito::json::Value::Null);
    if circuit.phase == 0 {
        out.extend([
            Fp::from(u64::from(has_boundary)),
            Fp::from(if has_boundary {
                num(schedule, "height")
            } else {
                0
            }),
        ]);
        for key in ["predecessor_context_id_hex", "selection_anchor_hex"] {
            if has_boundary {
                append_bytes(&mut out, &bytes(boundary, key));
            } else {
                out.extend([Fp::ZERO; 32]);
            }
        }
        for slot in schedule.get("successor_slots").unwrap().as_array().unwrap() {
            let pending = slot.get("kind").unwrap().as_str().unwrap() == "pending_boundary";
            out.extend([
                Fp::from(u64::from(pending)),
                Fp::from(num(slot, "height")),
                Fp::from(if pending {
                    num(schedule.get("current").unwrap(), "last_height")
                } else {
                    0
                }),
            ]);
            if pending {
                append_bytes(&mut out, &bytes(slot, "context_id_hex"));
            } else {
                out.extend([Fp::ZERO; 32]);
            }
            let payload = bytes(slot.get("params").unwrap(), "payload_hex");
            out.extend(
                fields(&payload)
                    .into_iter()
                    .map(|field| Fp::from(le(field))),
            );
        }
    } else {
        let epoch = if circuit.phase == 2 && has_boundary {
            boundary.get("successor").unwrap()
        } else {
            schedule.get("current").unwrap()
        };
        let members = epoch.get("members").unwrap().as_array().unwrap();
        let payload = bytes(epoch.get("authorization").unwrap(), "payload_hex");
        let authorization = fields(&payload);
        out.extend(
            [
                members.len() as u64,
                ((members.len() - 1) / 3) as u64,
                1,
                num(epoch, "epoch"),
                num(epoch, "first_height"),
                num(epoch, "last_height"),
                le(authorization[5]),
                le(authorization[10]),
            ]
            .into_iter()
            .map(Fp::from),
        );
        append_bytes(&mut out, &bytes(epoch, "network_hex"));
        append_bytes(&mut out, &bytes(epoch, "leader_seed_hex"));
        if circuit.phase == 1 {
            let member = members.get(usize::from(circuit.seat));
            out.extend([
                Fp::from(u64::from(member.is_some())),
                Fp::from(u64::from(circuit.seat)),
            ]);
            if let Some(member) = member {
                append_bytes(&mut out, &bytes(member, "public_key_compressed_hex"));
                append_bytes(&mut out, &bytes(member, "proof_of_possession_hex"));
            } else {
                out.extend([Fp::ZERO; 144]);
            }
        } else {
            for index in [6, 8, 9] {
                append_bytes(&mut out, authorization[index]);
            }
            let installed = le(&authorization[7][..4]) == 1;
            out.push(Fp::from(u64::from(installed)));
            if installed {
                let inner = fields(&authorization[7][4..])[0];
                for field in fields(inner) {
                    append_bytes(&mut out, field);
                }
            } else {
                out.extend([Fp::ZERO; 64]);
            }
        }
    }
    assert!(out.len() <= OUTPUTS);
    out.resize(OUTPUTS, Fp::ZERO);
    vec![out]
}

#[test]
fn native_preboundary_boundary_successor_ranges_and_slots_are_source_bound() {
    let fixtures = fixture();
    for case in fixtures.get("cases").unwrap().as_array().unwrap() {
        for block in &case.get("blocks").unwrap().as_array().unwrap()[1..] {
            let circuit = ScheduleCircuit {
                frame: bytes(block, "result_preimage_hex"),
                phase: 0,
                seat: 0,
                known: true,
            };
            let public = expected(&circuit, block.get("schedule").unwrap());
            assert!(
                check_circuit(&circuit, 19, &public, CheckMode::Strict)
                    .unwrap()
                    .is_satisfied(),
                "seats{} height{}",
                num(case, "seats"),
                num(block, "height")
            );
        }
    }
}

#[test]
fn native_current_and_authorized_epoch_members_include31_seats_and_zero_inactive_keys() {
    let fixtures = fixture();
    for case in fixtures.get("cases").unwrap().as_array().unwrap() {
        let blocks = case.get("blocks").unwrap().as_array().unwrap();
        for (height, phase, seat) in [
            (1, 1, u8::try_from(num(case, "seats") - 1).unwrap()),
            (2, 1, 30),
            (2, 2, 0),
            (3, 2, 0),
        ] {
            let block = &blocks[height];
            let circuit = ScheduleCircuit {
                frame: bytes(block, "result_preimage_hex"),
                phase,
                seat,
                known: true,
            };
            let public = expected(&circuit, block.get("schedule").unwrap());
            assert!(
                check_circuit(&circuit, 19, &public, CheckMode::Strict)
                    .unwrap()
                    .is_satisfied(),
                "seats{} height{} phase{phase}",
                num(case, "seats"),
                num(block, "height")
            );
        }
    }
}

/// Exercise the production verifier's shared lanes, rather than the independent
/// sponge configuration used by the larger parser fixture checks above.
mod shared_lanes {
    use super::*;
    use iroha_pasta::{Ep, poseidon::hash_with_domain};
    use iroha_plonk::frontend::synthesize;
    use iroha_plonk_gadgets::WordHasher;
    use iroha_plonk_recursion::verifier::{VerifierChip, VerifierConfig};

    const BEFORE: u64 = u64::from_le_bytes(*b"kgwtstb1");
    const AFTER: u64 = u64::from_le_bytes(*b"kgwtsta1");
    const PUBLIC_WORDS: usize = 35;

    #[derive(Clone)]
    struct SharedTape {
        frame: Vec<u8>,
        known: bool,
    }

    #[derive(Clone)]
    struct SharedConfig {
        verifier: VerifierConfig<Ep>,
        public: Column<Instance>,
    }

    impl Circuit<Fp> for SharedTape {
        type Config = SharedConfig;
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self {
                known: false,
                ..self.clone()
            }
        }

        fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
            let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, 3).unwrap();
            let public = meta.instance_column(PUBLIC_WORDS);
            meta.enable_equality(public);
            SharedConfig { verifier, public }
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<Fp>,
        ) -> Result<(), Error> {
            let mut chip = VerifierChip::new(config.verifier);
            chip.load_tables(&mut layouter)?;
            let source = ResultTapeWitness::from_frame(&witness(self.known, self.frame.clone()))?;
            let output = layouter.assign_region(
                || "tape windows retain shared verifier lanes",
                |mut region| {
                    let root = chip.uint().glue().witness(&mut region, source.root())?;
                    let before =
                        chip.hash_words(&mut region, BEFORE, std::slice::from_ref(&root))?;
                    let (mut uint, hash) = chip.uint_and_hasher()?;
                    let length = uint
                        .assign::<32>(&mut region, witness(self.known, self.frame.len() as u128))?;
                    let tape = ResultTape::new(&mut uint, &mut region, &root, &length)?;
                    // Adding the 24-byte result tag makes both reads cross an
                    // authenticated 32-byte chunk boundary at byte 31.
                    let first_offset = uint.constant::<32>(&mut region, 7)?;
                    let first =
                        source.read::<16>(&tape, &mut uint, hash, &mut region, &first_offset)?;
                    let between = hash.hash_words(&mut region, BEFORE, &first)?;
                    let (mut uint, hash) = chip.uint_and_hasher()?;
                    let second_offset = uint.constant::<32>(&mut region, 39)?;
                    let second =
                        source.read::<16>(&tape, &mut uint, hash, &mut region, &second_offset)?;
                    let mut final_words = vec![before.clone(), between];
                    final_words.extend(second.iter().cloned());
                    let after = chip.hash_words(&mut region, AFTER, &final_words)?;
                    let mut out = vec![root, before];
                    out.extend(first);
                    out.extend(second);
                    out.push(after);
                    // Publicly reachable active-transcript state must refuse a
                    // new source borrow. Missing-duplex state is private to the
                    // verifier and is not exposed just to manufacture a test.
                    chip.uint_and_hasher()?.1.absorb_constant(Fp::ONE);
                    assert!(chip.uint_and_hasher().is_err());
                    Ok(out)
                },
            )?;
            for (index, word) in output.iter().enumerate() {
                layouter.constrain_instance(word.cell(), config.public, index)?;
            }
            Ok(())
        }
    }

    #[test]
    fn verifier_shared_lanes_bind_cross_chunk_windows_and_refuse_active_transcripts() {
        let circuit = SharedTape {
            frame: (0..80)
                .map(|index| u8::try_from((index * 17 + 3) % 256).unwrap())
                .collect(),
            known: true,
        };
        let source = ResultTapeWitness::from_frame(&Value::known(circuit.frame.clone())).unwrap();
        let mut expected = Vec::new();
        let _ = source.root().map(|root| {
            let first = circuit.frame[7..23]
                .iter()
                .map(|byte| Fp::from(u64::from(*byte)))
                .collect::<Vec<_>>();
            let second = circuit.frame[39..55]
                .iter()
                .map(|byte| Fp::from(u64::from(*byte)))
                .collect::<Vec<_>>();
            let before = hash_with_domain(BEFORE, &[root]);
            let between = hash_with_domain(BEFORE, &first);
            let mut final_words = vec![before, between];
            final_words.extend_from_slice(&second);
            expected.extend([root, before]);
            expected.extend(first);
            expected.extend(second);
            expected.push(hash_with_domain(AFTER, &final_words));
        });
        assert_eq!(expected.len(), PUBLIC_WORDS);
        assert!(
            check_circuit(&circuit, 16, &[expected.clone()], CheckMode::Strict)
                .unwrap()
                .is_satisfied()
        );
        expected[2] += Fp::ONE;
        assert!(
            !check_circuit(&circuit, 16, &[expected], CheckMode::Strict)
                .unwrap()
                .is_satisfied()
        );
        let known = synthesize(&circuit, 16, None).unwrap();
        let unknown = synthesize(&circuit.without_witnesses(), 16, None).unwrap();
        assert_eq!(known.tables.fixed(), unknown.tables.fixed());
        assert_eq!(
            known.tables.advice_assigned(),
            unknown.tables.advice_assigned()
        );
    }
}
