//! Native tree parity and byte-source substitution checks.

use super::*;
use ff::PrimeField;
use iroha_pasta::poseidon::hash_with_domain;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Layouter, SimpleFloorPlanner, Value, synthesize},
};
use iroha_plonk_gadgets::{
    GlueConfig, LimbBits, RunningSumChip, RunningSumConfig, SpongeChip, SpongeConfig,
    blake2b::Blake2bConfig,
    poseidon::{Pow5Columns, RoundConstantColumns},
};

#[derive(Clone)]
struct NativeTape {
    bytes: Vec<u8>,
    levels: Vec<Vec<Fp>>,
}

impl NativeTape {
    fn new(frame: &[u8], bad_padding: bool) -> Self {
        let mut bytes = vec![0; (1 << TAPE_DEPTH) * 32];
        bytes[..RESULT_TAG.len()].copy_from_slice(RESULT_TAG);
        bytes[RESULT_TAG.len()..RESULT_TAG.len() + frame.len()].copy_from_slice(frame);
        if bad_padding {
            bytes[127] = 1;
        }
        let leaves: Vec<_> = bytes
            .chunks_exact(32)
            .enumerate()
            .map(|(index, bytes)| {
                let lo = u128::from_le_bytes(bytes[..16].try_into().unwrap());
                let hi = u128::from_le_bytes(bytes[16..].try_into().unwrap());
                hash_with_domain(
                    TAPE_LEAF_DOMAIN,
                    &[Fp::from(index as u64), Fp::from_u128(lo), Fp::from_u128(hi)],
                )
            })
            .collect();
        let mut levels = vec![leaves];
        while levels.last().unwrap().len() > 1 {
            levels.push(
                levels
                    .last()
                    .unwrap()
                    .chunks_exact(2)
                    .map(|children| hash_with_domain(TAPE_NODE_DOMAIN, children))
                    .collect(),
            );
        }
        Self { bytes, levels }
    }
    fn root(&self) -> Fp {
        self.levels[TAPE_DEPTH][0]
    }
    fn opening(&self, index: usize) -> ([u8; 32], [Fp; TAPE_DEPTH]) {
        let bytes = self.bytes[index * 32..index * 32 + 32].try_into().unwrap();
        let siblings = core::array::from_fn(|level| self.levels[level][(index >> level) ^ 1]);
        (bytes, siblings)
    }
}

fn witness<T>(known: bool, value: T) -> Value<T> {
    if known {
        Value::known(value)
    } else {
        Value::unknown()
    }
}

#[derive(Clone)]
struct Config {
    glue: GlueConfig,
    range: RunningSumConfig,
    hash: SpongeConfig<Fp>,
    blake: Blake2bConfig,
    public: Column<Instance>,
}

fn configure(meta: &mut ConstraintSystem<Fp>, count: usize) -> Config {
    let advice = core::array::from_fn(|_| meta.advice_column());
    let constants = meta.fixed_column();
    let glue = GlueConfig::configure(meta, advice, constants);
    let range_column = meta.advice_column();
    let range = RunningSumConfig::configure(meta, range_column, LimbBits::new(8).unwrap());
    let columns = Pow5Columns::allocate(meta);
    let round_constants = RoundConstantColumns::allocate(meta);
    let hash = SpongeConfig::configure(meta, columns, round_constants, &[]);
    let advice = core::array::from_fn(|_| meta.advice_column());
    let blake = Blake2bConfig::configure(meta, advice, constants);
    let public = meta.instance_column(count);
    meta.enable_equality(public);
    Config {
        glue,
        range,
        hash,
        blake,
        public,
    }
}

fn assign_opening(
    glue: &mut GlueChip<Fp>,
    region: &mut Region<'_, Fp>,
    native: &NativeTape,
    index: usize,
    known: bool,
    attack: usize,
) -> Result<ChunkOpening, Error> {
    let (bytes, mut siblings) = native.opening(index);
    let mut values = bytes.map(|byte| Fp::from(u64::from(byte)));
    match attack {
        1 => values[0] += Fp::ONE,
        2 => siblings[5] += Fp::ONE,
        3 => values[17] = Fp::from(256),
        _ => {}
    }
    Ok(ChunkOpening {
        bytes: glue
            .witnesses(region, &values.map(|value| witness(known, value)))?
            .try_into()
            .unwrap(),
        siblings: glue
            .witnesses(region, &siblings.map(|value| witness(known, value)))?
            .try_into()
            .unwrap(),
    })
}

#[derive(Clone)]
struct ReadCircuit {
    tape: NativeTape,
    frame_len: u128,
    offset: u128,
    attack: usize,
    known: bool,
}

impl Circuit<Fp> for ReadCircuit {
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
        configure(meta, 35)
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut glue = GlueChip::new(config.glue);
        let mut range = RunningSumChip::new(config.range);
        range.load_table(&mut layouter)?;
        let mut hash = SpongeChip::new(config.hash);
        let output = layouter.assign_region(
            || "authenticated R window",
            |mut region| {
                let root = glue.witness(&mut region, witness(self.known, self.tape.root()))?;
                let index = (self.offset as usize + RESULT_TAG.len()) / 32;
                let index = index + usize::from(self.attack == 4);
                let openings = [
                    assign_opening(
                        &mut glue,
                        &mut region,
                        &self.tape,
                        index,
                        self.known,
                        self.attack,
                    )?,
                    assign_opening(&mut glue, &mut region, &self.tape, index + 1, self.known, 0)?,
                ];
                let mut uint = UintChip::new(&mut glue, &mut range);
                let length = uint.assign::<32>(&mut region, witness(self.known, self.frame_len))?;
                let offset = uint.assign::<32>(&mut region, witness(self.known, self.offset))?;
                let tape = ResultTape::new(&mut uint, &mut region, &root, &length)?;
                let bytes =
                    tape.read_window::<32>(&mut uint, &mut hash, &mut region, &offset, &openings)?;
                let mut out = vec![root, length.word().clone(), offset.word().clone()];
                out.extend(bytes);
                Ok(out)
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}

fn read_public(circuit: &ReadCircuit) -> Vec<Vec<Fp>> {
    let mut words = vec![
        circuit.tape.root(),
        Fp::from_u128(circuit.frame_len),
        Fp::from_u128(circuit.offset),
    ];
    let start = circuit.offset as usize + RESULT_TAG.len();
    words.extend(
        circuit.tape.bytes[start..start + 32]
            .iter()
            .map(|byte| Fp::from(u64::from(*byte))),
    );
    vec![words]
}

#[test]
fn authenticated_windows_cross_chunks_at_every_possible_byte_offset() {
    let frame: Vec<_> = (0..256).map(|i| i as u8).collect();
    let tape = NativeTape::new(&frame, false);
    for offset in 0..32 {
        let circuit = ReadCircuit {
            tape: tape.clone(),
            frame_len: 256,
            offset,
            attack: 0,
            known: true,
        };
        assert!(
            check_circuit(&circuit, 13, &read_public(&circuit), CheckMode::Strict)
                .unwrap()
                .is_satisfied(),
            "offset {offset}"
        );
    }
}

#[test]
fn authenticated_windows_reject_substituted_bytes_paths_indices_and_out_of_frame_reads() {
    let frame: Vec<_> = (0..256).map(|i| i as u8).collect();
    let circuit = ReadCircuit {
        tape: NativeTape::new(&frame, false),
        frame_len: 256,
        offset: 31,
        attack: 0,
        known: true,
    };
    let public = read_public(&circuit);
    for attack in 1..=4 {
        let forged = ReadCircuit {
            attack,
            ..circuit.clone()
        };
        assert!(
            !check_circuit(&forged, 13, &public, CheckMode::Strict)
                .unwrap()
                .is_satisfied(),
            "attack {attack}"
        );
    }
    let outside = ReadCircuit {
        offset: 225,
        ..circuit.clone()
    };
    assert!(
        !check_circuit(&outside, 13, &read_public(&outside), CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    let known = synthesize(&circuit, 13, Some(&public)).unwrap();
    let unknown = synthesize(&circuit.without_witnesses(), 13, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.selectors(), unknown.tables.selectors());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
}

#[derive(Clone)]
struct ScanCircuit {
    tape: NativeTape,
    bad_index: bool,
    known: bool,
}

impl Circuit<Fp> for ScanCircuit {
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
        configure(meta, 33)
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut glue = GlueChip::new(config.glue);
        let mut range = RunningSumChip::new(config.range);
        range.load_table(&mut layouter)?;
        let mut hash = SpongeChip::new(config.hash);
        let mut blake = Blake2bChip::new(&config.blake);
        let output = layouter.assign_region(
            || "tape scan",
            |mut region| {
                let root = glue.witness(&mut region, witness(self.known, self.tape.root()))?;
                let mut openings = Vec::with_capacity(4);
                for index in 0..4 {
                    openings.push(assign_opening(
                        &mut glue,
                        &mut region,
                        &self.tape,
                        index + usize::from(self.bad_index),
                        self.known,
                        0,
                    )?);
                }
                let openings = openings.try_into().unwrap();
                let mut uint = UintChip::new(&mut glue, &mut range);
                let length = uint.constant::<32>(&mut region, 1)?;
                let tape = ResultTape::new(&mut uint, &mut region, &root, &length)?;
                let stream =
                    TapeResultHashStream::start(&mut uint, &mut blake, &mut region, &tape)?;
                let next =
                    stream.absorb(&mut uint, &mut hash, &mut blake, &mut region, &openings)?;
                let digest = next.finish(&mut blake, &mut region)?;
                let mut out = vec![root];
                out.extend_from_slice(digest.bytes());
                Ok(out)
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}

#[test]
fn tape_scan_binds_membership_counter_and_final_padding_to_native_hash() {
    // Same independent hashlib vector as result/tests.rs (synthetic frame [0]).
    let digest = "f768b3cf4ab52bdb0d0cba74798820c1aec6d6b409bea8d504783cbd7b97bb4f";
    for (bad_padding, bad_index) in [(false, false), (true, false), (false, true)] {
        let circuit = ScanCircuit {
            tape: NativeTape::new(&[0], bad_padding),
            bad_index,
            known: true,
        };
        let mut public = vec![circuit.tape.root()];
        public.extend((0..digest.len()).step_by(2).map(|i| {
            Fp::from(u64::from(
                u8::from_str_radix(&digest[i..i + 2], 16).unwrap(),
            ))
        }));
        assert_eq!(
            check_circuit(&circuit, 15, &[public], CheckMode::Strict)
                .unwrap()
                .is_satisfied(),
            !bad_padding && !bad_index
        );
    }
}
