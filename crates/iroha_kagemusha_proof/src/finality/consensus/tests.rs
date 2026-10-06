//! Native Commit transcript offsets and exact equal-weight quorum rejection.

use super::*;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Layouter, SimpleFloorPlanner, Value, synthesize},
};
use iroha_plonk_gadgets::{
    GlueConfig, LimbBits, RunningSumChip, RunningSumConfig, tamper::undetected_tampers,
};

#[derive(Clone)]
struct QuorumCircuit {
    n: u64,
    f: u64,
    length: u64,
    bitmap: [u64; 4],
    known: bool,
}
impl QuorumCircuit {
    fn valid(f: u64, offset: u64) -> Self {
        let n = 3 * f + 1;
        let mut bitmap = [0; 4];
        for i in 0..2 * f + 1 {
            let index = (i + offset) % n;
            bitmap[index as usize / 8] |= 1 << (index % 8);
        }
        Self {
            n,
            f,
            length: n.div_ceil(8),
            bitmap,
            known: true,
        }
    }
    fn public(&self) -> Vec<Fp> {
        [self.n, self.f, self.length]
            .into_iter()
            .chain(self.bitmap)
            .chain((0..31).map(|i| (self.bitmap[i / 8] >> (i % 8)) & 1))
            .map(Fp::from)
            .collect()
    }
}
#[derive(Clone)]
struct Config {
    glue: GlueConfig,
    range: RunningSumConfig,
    public: Column<Instance>,
}
fn configure(meta: &mut ConstraintSystem<Fp>, count: usize) -> Config {
    let columns = core::array::from_fn(|_| meta.advice_column());
    let constant = meta.fixed_column();
    let glue = GlueConfig::configure(meta, columns, constant);
    let column = meta.advice_column();
    let range = RunningSumConfig::configure(meta, column, LimbBits::new(4).unwrap());
    let public = meta.instance_column(count);
    meta.enable_equality(public);
    Config {
        glue,
        range,
        public,
    }
}
fn witness(known: bool, value: u64) -> Value<Fp> {
    if known {
        Value::known(Fp::from(value))
    } else {
        Value::unknown()
    }
}
impl Circuit<Fp> for QuorumCircuit {
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
        configure(meta, 38)
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut glue = GlueChip::new(config.glue);
        let mut range = RunningSumChip::new(config.range);
        range.load_table(&mut layouter)?;
        let output = layouter.assign_region(
            || "exact QC signer geometry",
            |mut region| {
                let input = [self.n, self.f, self.length]
                    .into_iter()
                    .chain(self.bitmap)
                    .map(|x| witness(self.known, x))
                    .collect::<Vec<_>>();
                let words = glue.witnesses(&mut region, &input)?;
                let bytes: [Word<Fp>; 4] = words[3..]
                    .to_vec()
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                let mut uint = UintChip::new(&mut glue, &mut range);
                let quorum = QuorumCells::from_bitmap(
                    &mut uint,
                    &mut region,
                    &words[0],
                    &words[1],
                    &words[2],
                    &bytes,
                )?;
                let mut output = vec![
                    quorum.members().word().clone(),
                    quorum.faults().word().clone(),
                    quorum.encoded_len().word().clone(),
                ];
                output.extend_from_slice(&bytes);
                output.extend(quorum.selected().iter().map(|x| x.word().clone()));
                Ok(output)
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}
fn quorum_satisfies(circuit: &QuorumCircuit) -> bool {
    check_circuit(circuit, 11, &[circuit.public()], CheckMode::Strict)
        .unwrap()
        .is_satisfied()
}
#[test]
fn exact_quorums_match_native_lsb_first_bitmap_geometry() {
    for f in 1..=10 {
        for offset in [0, 1, 3 * f] {
            let circuit = QuorumCircuit::valid(f, offset);
            assert!(quorum_satisfies(&circuit), "f={f} offset={offset}");
            let mut forged = circuit.clone();
            forged.length += 1;
            assert!(!quorum_satisfies(&forged));
            forged = circuit.clone();
            forged.bitmap[3] |= 128;
            assert!(!quorum_satisfies(&forged));
            forged = circuit.clone();
            forged.bitmap[0] ^= 1;
            assert!(!quorum_satisfies(&forged));
        }
    }
    for (n, f) in [(1, 0), (3, 1), (5, 1), (30, 10), (31, 9), (34, 11)] {
        let mut circuit = QuorumCircuit::valid(1, 0);
        circuit.n = n;
        circuit.f = f;
        assert!(!quorum_satisfies(&circuit));
    }
    let mut circuit = QuorumCircuit::valid(1, 0);
    circuit.bitmap[0] += 256;
    assert!(!quorum_satisfies(&circuit));
}
#[test]
fn every_quorum_advice_cell_is_constrained() {
    let circuit = QuorumCircuit::valid(4, 3);
    assert!(
        undetected_tampers(&circuit, 11, &[circuit.public()])
            .unwrap()
            .is_empty()
    );
    assert!(synthesize(&circuit.without_witnesses(), 11, None).is_ok());
}

#[derive(Clone)]
struct VoteCircuit {
    bytes: [u64; CommitVoteCells::BYTES],
    known: bool,
}
impl VoteCircuit {
    fn fixture() -> Self {
        let mut bytes = b"sumeragi/sig\x03".to_vec();
        bytes.extend_from_slice(&[11; 32]);
        bytes.extend_from_slice(&0x0102_0304_0506_0708_u64.to_be_bytes());
        bytes.extend_from_slice(&[22; 32]);
        bytes.extend_from_slice(&0x1122_3344_5566_7788_u64.to_be_bytes());
        bytes.extend_from_slice(&0x99aa_bbcc_ddee_ff00_u64.to_be_bytes());
        bytes.extend_from_slice(&[33; 32]);
        bytes.extend_from_slice(&[44; 32]);
        Self {
            bytes: bytes
                .into_iter()
                .map(u64::from)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            known: true,
        }
    }
    fn public(&self) -> Vec<Fp> {
        self.bytes
            .into_iter()
            .chain([
                0x0102_0304_0506_0708,
                0x1122_3344_5566_7788,
                0x99aa_bbcc_ddee_ff00,
            ])
            .map(Fp::from)
            .collect()
    }
}
impl Circuit<Fp> for VoteCircuit {
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
        configure(meta, 168)
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut glue = GlueChip::new(config.glue);
        let mut range = RunningSumChip::new(config.range);
        range.load_table(&mut layouter)?;
        let output = layouter.assign_region(
            || "exact Commit vote transcript",
            |mut region| {
                let bytes =
                    glue.witnesses(&mut region, &self.bytes.map(|x| witness(self.known, x)))?;
                let bytes = bytes.try_into().map_err(|_| Error::Synthesis)?;
                let mut uint = UintChip::new(&mut glue, &mut range);
                let vote = CommitVoteCells::from_bytes(&mut uint, &mut region, &bytes)?;
                let mut out = vote.bytes()[..13].to_vec();
                out.extend_from_slice(vote.instance());
                out.extend_from_slice(&vote.bytes()[45..53]);
                out.extend_from_slice(vote.context());
                out.extend_from_slice(&vote.bytes()[85..101]);
                out.extend_from_slice(vote.block_hash());
                out.extend_from_slice(vote.result());
                out.extend([
                    vote.epoch().word().clone(),
                    vote.height().word().clone(),
                    vote.view().word().clone(),
                ]);
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
fn commit_vote_bytes_bind_every_field_and_reject_wrong_domain() {
    let circuit = VoteCircuit::fixture();
    let check = |circuit: &VoteCircuit, public: Vec<Fp>| {
        check_circuit(circuit, 10, &[public], CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    };
    assert!(check(&circuit, circuit.public()));
    for offset in [0, 11, 12] {
        let mut forged = circuit.clone();
        forged.bytes[offset] ^= 1;
        assert!(!check(&forged, forged.public()));
    }
    for offset in [13, 45, 53, 85, 93, 101, 133, 164] {
        let mut forged = circuit.public();
        forged[offset] += Fp::ONE;
        assert!(!check(&circuit, forged));
    }
    let mut forged = circuit.clone();
    forged.bytes[85..93].fill(0);
    forged.bytes[92] = 1;
    let mut public = forged.public();
    public[166] = Fp::ONE;
    assert!(!check(&forged, public));
    forged = circuit.clone();
    forged.bytes[164] += 256;
    assert!(!check(&forged, forged.public()));
    assert!(
        undetected_tampers(&circuit, 10, &[circuit.public()])
            .unwrap()
            .is_empty()
    );
    assert!(synthesize(&circuit.without_witnesses(), 10, None).is_ok());
}
