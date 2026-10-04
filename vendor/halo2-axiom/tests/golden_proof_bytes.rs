//! Golden proof-byte tests for the Pasta IPA prover.
//!
//! Every case proves a fixed circuit with a fixed ChaCha20 seed through the
//! Blake2b transcript and compares the SHA-256 of the proof bytes with a
//! constant recorded from the unmodified prover on 2026-10-04 (aarch64-apple-darwin,
//! confirmed on x86_64-apple-darwin). Each proof is created inside Rayon pools of 1, 2,
//! 4 and 7 threads and the bytes must not depend on the pool.
//!
//! Prover kernel work (generator folding, MSM, quotient evaluation) must keep
//! every constant unchanged. A changed constant means a changed proof format,
//! not a test to update.
//!
//! Two circuit shapes are covered over both Pasta cycles (Eq/Fp and Ep/Fq):
//! - `sigma`: eight advice columns: two Pow5-style width-3 lanes, an equality
//!   bus and a byte-lookup base column. It has a degree-7 round gate (a
//!   quadratic mode indicator times `x^5`), copy constraints on the bus and one
//!   instance column. It runs at k = 6 and 9, plus k = 11 as an ignored release case.
//! - `wide`: twelve advice columns with degree-4 product gates, two lookups
//!   and equality on every column, so the permutation argument splits into
//!   several chunks. It runs at k = 8 and 10.
//!
//! The debug and release builds, and aarch64 and x86_64, must produce the same constants.
//!
//! TODO: no reviewed command runs this file from the Iroha workspace yet. Cargo refuses
//! `cargo test -p halo2-axiom` because this crate is a patched non-member with
//! dev-dependencies. A standalone build of this manifest also cannot resolve offline,
//! because some dev and optional dependencies are absent from the workspace lock. Until a
//! runner is reviewed, compile this file as the integration test of a package that
//! path-depends on `vendor/halo2-axiom`, patches `halo2curves-axiom` and `num-bigint` to
//! their vendored copies, and starts from the workspace `Cargo.lock`.

use halo2_axiom::{
    arithmetic::CurveAffine,
    circuit::{Cell, Layouter, SimpleFloorPlanner, Value},
    halo2curves::{
        ff::{Field, FromUniformBytes, PrimeField, WithSmallOrderMulGroup},
        pasta::{EpAffine, EqAffine},
    },
    plonk::{
        Advice, Circuit, Column, ConstraintSystem, Error, Expression, Fixed, Instance, ProvingKey,
        TableColumn, create_proof, keygen_pk, keygen_vk, verify_proof,
    },
    poly::{
        Rotation, VerificationStrategy,
        commitment::ParamsProver,
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            multiopen::{ProverIPA, VerifierIPA},
            strategy::SingleStrategy,
        },
    },
    transcript::{
        Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer, TranscriptWriterBuffer,
    },
};
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};
use sha2::{Digest, Sha256};

/// Prover seeds; each case is proved once per seed.
const PROVER_SEEDS: [[u8; 32]; 2] = [[42; 32], [43; 32]];
/// Rayon pool sizes every proof is repeated in.
const THREAD_POOLS: [usize; 4] = [1, 2, 4, 7];
/// Rows left unassigned at the end of the domain; covers every case's blinding rows.
const RESERVED_ROWS: usize = 16;
/// Width of one Pow5-style lane.
const LANE_WIDTH: usize = 3;
/// Number of Pow5-style lanes in the sigma circuit.
const LANES: usize = 2;
/// Rows between two consecutive bus copies of the same lane.
const BUS_STRIDE: usize = 16;
/// Advice columns of the wide circuit.
const WIDE_ADVICE: usize = 12;
/// Product groups `(x, y, z)` of the wide circuit.
const WIDE_GROUPS: usize = WIDE_ADVICE / 3;

/// Constraint-system shape a golden case must keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CircuitShape {
    /// Maximum constraint degree.
    degree: usize,
    /// Columns in the permutation argument; with degree `d` each chunk holds `d - 2`.
    permutation_columns: usize,
}

/// Degree-7 round gate; bus, base, both lane heads and the instance are permuted.
const SIGMA_SHAPE: CircuitShape = CircuitShape {
    degree: 7,
    permutation_columns: 5,
};
/// Degree-4 products; every advice column and the instance are permuted (seven chunks).
const WIDE_SHAPE: CircuitShape = CircuitShape {
    degree: 4,
    permutation_columns: WIDE_ADVICE + 1,
};

/// SHA-256 of the proof bytes recorded from the unmodified prover, keyed by case name.
const GOLDEN_SHA256: &[(&str, &str)] = &[
    (
        "sigma/eq/k6/seed42",
        "90e140f85c554ac155c680225133d12d15e63dd9077096d52981ecefa58aa7f2",
    ),
    (
        "sigma/eq/k6/seed43",
        "ea7ade72c220a206bc64cab0389f310515363e2e0655632faae2e404d10aa9a6",
    ),
    (
        "sigma/eq/k9/seed42",
        "6d3df87126fa5fa50fff8c65dd4ce0ab3f5f17a09524165fb00c661e2c477d2f",
    ),
    (
        "sigma/eq/k9/seed43",
        "653c2cb3c9bcb7f0805bdfb782b8759bc2a3118f0d7f550c008518b8616294e5",
    ),
    (
        "sigma/eq/k11/seed42",
        "221e81b72d17d99efe8b2acc24f2872e265ff016b36b105bac2970c9c44623f1",
    ),
    (
        "sigma/eq/k11/seed43",
        "5f8cf8102416bff24187317b6f9d1830b03028091e723a88f74572fa205a97ea",
    ),
    (
        "sigma/ep/k6/seed42",
        "e189647dfa75ccd42ef1f74b740806266a165fe09de41f5fd579c393279f576d",
    ),
    (
        "sigma/ep/k6/seed43",
        "90c2428245029a8ff09916178a38b64af158d8bb1d673dcf0b4387621e6e399c",
    ),
    (
        "sigma/ep/k9/seed42",
        "c8f7e72fb00d0fec5abab7c0f621cf0d7252b1e4e4abbf817bde3493332a492d",
    ),
    (
        "sigma/ep/k9/seed43",
        "264707cb5f70912117500987dc40936cabb32f378fd1ae68cb4e750659be6cc9",
    ),
    (
        "sigma/ep/k11/seed42",
        "d5550ad8b76ab493fbcb77eb146758c6c18d1877acc78b6eae889394189ba22d",
    ),
    (
        "sigma/ep/k11/seed43",
        "5b5c48afd56accbe8b3003acd63c7aa6818332f09aa38c26807612c0c8aa7469",
    ),
    (
        "wide/eq/k8/seed42",
        "d838e4643cce7ced44b9f102d536eb35828ece18d9005a04634b68c7859ba564",
    ),
    (
        "wide/eq/k8/seed43",
        "0e4fc07392ccb6546a39a32dd4a63715bcb09aecfbeed70919a06b65e69832b3",
    ),
    (
        "wide/eq/k10/seed42",
        "abd09095618144fe89407946d046371582a69480eba035edd824c87a7d47c556",
    ),
    (
        "wide/eq/k10/seed43",
        "57de409b4779bdc26801bd029d6640b1570c6a62e53fb3fb4a640945c20542d8",
    ),
    (
        "wide/ep/k8/seed42",
        "9da04008b44a594e9d3a59f0394cd1a8b367c4e802ec86bb222824d24c4f893f",
    ),
    (
        "wide/ep/k8/seed43",
        "6d165875348a6a70636daa1b388614c20431f69a1d7026a5d629ec607f69d706",
    ),
    (
        "wide/ep/k10/seed42",
        "2931f8c816e3090edbe5689c05de983c61f6b424dd0f6d6197120182b09eae62",
    ),
    (
        "wide/ep/k10/seed43",
        "090413c5a7ba2c80054a1690673076615f55caac42f2eda68e1201f4988f72c4",
    ),
];

/// Number of usable circuit rows at `k`.
fn used_rows(k: u32) -> usize {
    (1_usize << k)
        .checked_sub(RESERVED_ROWS)
        .expect("k leaves rows after the reserved tail")
}

/// Bits of the lookup table at `k`: a byte table when it fits, otherwise a smaller one.
fn table_bits(k: u32) -> u32 {
    k.checked_sub(1).expect("k is positive").min(8)
}

/// Deterministic data stream for circuit constants and witnesses (never the prover RNG).
fn data_rng(label: u8, k: u32) -> ChaCha20Rng {
    let mut seed = [0_u8; 32];
    seed[0] = label;
    seed[1] = u8::try_from(k).expect("k fits a byte");
    seed[31] = 0x5a;
    ChaCha20Rng::from_seed(seed)
}

/// A small nonzero value below `2^bits`.
fn small_nonzero(rng: &mut ChaCha20Rng, bits: u32) -> u64 {
    let mask = (1_u64 << bits) - 1;
    (rng.next_u64() & mask).max(1)
}

/// Fixed lane MDS matrix, shared by `configure` and the witness.
fn lane_mds<F: Field>() -> [[F; LANE_WIDTH]; LANE_WIDTH] {
    let mut rng = data_rng(0xd5, 0);
    std::array::from_fn(|_| std::array::from_fn(|_| F::random(&mut rng)))
}

/// One Pow5 round: `next = M * (state + constants)^5`.
fn lane_round<F: Field>(
    mds: &[[F; LANE_WIDTH]; LANE_WIDTH],
    state: &[F; LANE_WIDTH],
    constants: &[F; LANE_WIDTH],
) -> [F; LANE_WIDTH] {
    let powers: [F; LANE_WIDTH] = std::array::from_fn(|i| {
        let shifted = state[i] + constants[i];
        shifted.square().square() * shifted
    });
    std::array::from_fn(|i| (0..LANE_WIDTH).fold(F::ZERO, |sum, j| sum + mds[i][j] * powers[j]))
}

/// Sigma-shaped circuit: two Pow5-style lanes, an equality bus and a byte-lookup column.
#[derive(Clone)]
struct SigmaCircuit<F: PrimeField> {
    k: u32,
    witness: bool,
    round_constants: Vec<[F; LANE_WIDTH]>,
    states: Vec<[[F; LANE_WIDTH]; LANES]>,
    bytes: Vec<F>,
}

/// Columns of [`SigmaCircuit`].
#[derive(Clone, Copy)]
struct SigmaConfig {
    lanes: [[Column<Advice>; LANE_WIDTH]; LANES],
    bus: Column<Advice>,
    base: Column<Advice>,
    constants: [Column<Fixed>; LANE_WIDTH],
    mode: Column<Fixed>,
    lookup_enable: Column<Fixed>,
    table: TableColumn,
    instance: Column<Instance>,
}

impl<F: PrimeField> SigmaCircuit<F> {
    /// Build the fixed data and witness for `k`.
    fn new(k: u32) -> Self {
        let rows = used_rows(k);
        let mds = lane_mds::<F>();
        let mut fixed_rng = data_rng(0xc0, k);
        let round_constants = (0..rows)
            .map(|_| std::array::from_fn(|_| F::random(&mut fixed_rng)))
            .collect::<Vec<[F; LANE_WIDTH]>>();
        let mut witness_rng = data_rng(0x5e, k);
        let mut states = Vec::with_capacity(rows);
        states.push(std::array::from_fn(|_| {
            std::array::from_fn(|_| F::random(&mut witness_rng))
        }));
        for row in 1..rows {
            let previous: &[[F; LANE_WIDTH]; LANES] = &states[row - 1];
            let next = std::array::from_fn(|lane| {
                lane_round(&mds, &previous[lane], &round_constants[row - 1])
            });
            states.push(next);
        }
        let bits = table_bits(k);
        let bytes = (0..rows)
            .map(|_| F::from(witness_rng.next_u64() & ((1_u64 << bits) - 1)))
            .collect();
        Self {
            k,
            witness: true,
            round_constants,
            states,
            bytes,
        }
    }

    /// The single public input: the first base-column value.
    fn public(&self) -> Vec<F> {
        vec![self.bytes[0]]
    }

    fn value(&self, value: F) -> Value<F> {
        if self.witness {
            Value::known(value)
        } else {
            Value::unknown()
        }
    }
}

impl<F: PrimeField> Circuit<F> for SigmaCircuit<F> {
    type Config = SigmaConfig;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            witness: false,
            ..self.clone()
        }
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> SigmaConfig {
        let lanes: [[Column<Advice>; LANE_WIDTH]; LANES] =
            std::array::from_fn(|_| std::array::from_fn(|_| meta.advice_column()));
        let bus = meta.advice_column();
        let base = meta.advice_column();
        let constants = std::array::from_fn(|_| meta.fixed_column());
        let mode = meta.fixed_column();
        let lookup_enable = meta.fixed_column();
        let table = meta.lookup_table_column();
        let instance = meta.instance_column();
        for column in [bus, base, lanes[0][0], lanes[1][0]] {
            meta.enable_equality(column);
        }
        meta.enable_equality(instance);
        let mds = lane_mds::<F>();
        for lane in lanes {
            meta.create_gate("sigma lane round", |cells| {
                let mode = cells.query_fixed(mode, Rotation::cur());
                // Mode 2 marks an active round; mode 0 disables the row.
                let active = mode.clone() * (mode - Expression::Constant(F::ONE));
                let powers: [Expression<F>; LANE_WIDTH] = std::array::from_fn(|i| {
                    let shifted = cells.query_advice(lane[i], Rotation::cur())
                        + cells.query_fixed(constants[i], Rotation::cur());
                    let square = shifted.clone() * shifted.clone();
                    square.clone() * square * shifted
                });
                (0..LANE_WIDTH)
                    .map(|i| {
                        let mixed = (0..LANE_WIDTH)
                            .fold(Expression::Constant(F::ZERO), |sum, j| {
                                sum + Expression::Constant(mds[i][j]) * powers[j].clone()
                            });
                        active.clone() * (cells.query_advice(lane[i], Rotation::next()) - mixed)
                    })
                    .collect::<Vec<_>>()
            });
        }
        meta.lookup("sigma byte", |cells| {
            let enable = cells.query_fixed(lookup_enable, Rotation::cur());
            let value = cells.query_advice(base, Rotation::cur());
            vec![(enable * value, table)]
        });
        SigmaConfig {
            lanes,
            bus,
            base,
            constants,
            mode,
            lookup_enable,
            table,
            instance,
        }
    }

    fn synthesize(&self, config: SigmaConfig, mut layouter: impl Layouter<F>) -> Result<(), Error> {
        let rows = used_rows(self.k);
        let table_size = 1_u64 << table_bits(self.k);
        layouter.assign_table(
            || "sigma byte table",
            |mut table| {
                for value in 0..table_size {
                    let offset = usize::try_from(value).expect("table offset fits usize");
                    table.assign_cell(
                        || "byte",
                        config.table,
                        offset,
                        || Value::known(F::from(value)),
                    )?;
                }
                Ok(())
            },
        )?;
        let public_cell = layouter.assign_region(
            || "sigma rows",
            |mut region| {
                let mut public_cell: Option<Cell> = None;
                for row in 0..rows {
                    for (column, constant) in config.constants.iter().zip(self.round_constants[row])
                    {
                        region.assign_fixed(*column, row, constant);
                    }
                    let mode = if row + 1 < rows { 2_u64 } else { 0 };
                    region.assign_fixed(config.mode, row, F::from(mode));
                    region.assign_fixed(config.lookup_enable, row, F::ONE);
                    let mut heads = [None; LANES];
                    for (lane, columns) in config.lanes.iter().enumerate() {
                        for (i, column) in columns.iter().enumerate() {
                            let value = self.value(self.states[row][lane][i]);
                            let cell = region.assign_advice(*column, row, value).cell();
                            if i == 0 {
                                heads[lane] = Some(cell);
                            }
                        }
                    }
                    // The bus carries lane 0 except at the mid-stride row, which carries lane 1.
                    let bus_lane = usize::from(row % BUS_STRIDE == BUS_STRIDE / 2);
                    let bus_value = self.value(self.states[row][bus_lane][0]);
                    let bus_cell = region.assign_advice(config.bus, row, bus_value).cell();
                    if row % (BUS_STRIDE / 2) == 0 {
                        region.constrain_equal(
                            bus_cell,
                            heads[bus_lane].expect("every lane assigns its head"),
                        );
                    }
                    let byte_cell = region
                        .assign_advice(config.base, row, self.value(self.bytes[row]))
                        .cell();
                    if row == 0 {
                        public_cell = Some(byte_cell);
                    }
                }
                Ok(public_cell.expect("row 0 is assigned"))
            },
        )?;
        layouter.constrain_instance(public_cell, config.instance, 0);
        Ok(())
    }
}

/// Wide degree-4 circuit: chained products in four column groups and two lookups.
#[derive(Clone)]
struct WideCircuit<F: PrimeField> {
    k: u32,
    witness: bool,
    /// Assigned advice values, one row of all twelve columns per circuit row.
    rows: Vec<[F; WIDE_ADVICE]>,
}

/// Columns of [`WideCircuit`].
#[derive(Clone, Copy)]
struct WideConfig {
    advice: [Column<Advice>; WIDE_ADVICE],
    product: Column<Fixed>,
    range: TableColumn,
    square: TableColumn,
    instance: Column<Instance>,
}

impl<F: PrimeField> WideCircuit<F> {
    /// Build the witness for `k`.
    ///
    /// Group `g` uses columns `(3g, 3g + 1, 3g + 2)` as `(x, y, z)` with
    /// `x_next = x * y * z`. Columns 1, 4, 7 and 10 hold small values, column 8 is
    /// the square of column 4 and the duplicated columns are copy-constrained.
    fn new(k: u32) -> Self {
        let used = used_rows(k);
        let bits = table_bits(k);
        let mut rng = data_rng(0x77, k);
        let start = F::random(&mut rng);
        let mut chains = [start; WIDE_GROUPS];
        let mut rows = Vec::with_capacity(used);
        for _ in 0..used {
            let small = small_nonzero(&mut rng, bits);
            let other_small = small_nonzero(&mut rng, bits);
            let u = F::from(small);
            let v = F::from(other_small);
            let v_squared = v.square();
            let w = F::random(&mut rng);
            let row = [
                chains[0], u, w, // group 0
                chains[1], v, w, // group 1
                chains[2], v, v_squared, // group 2
                chains[3], u, v_squared, // group 3
            ];
            for (group, chain) in chains.iter_mut().enumerate() {
                *chain = row[3 * group] * row[3 * group + 1] * row[3 * group + 2];
            }
            rows.push(row);
        }
        Self {
            k,
            witness: true,
            rows,
        }
    }

    /// The single public input: the last group-3 chain value.
    fn public(&self) -> Vec<F> {
        vec![self.rows.last().expect("wide circuit has rows")[9]]
    }
}

impl<F: PrimeField> Circuit<F> for WideCircuit<F> {
    type Config = WideConfig;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            witness: false,
            ..self.clone()
        }
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> WideConfig {
        let advice: [Column<Advice>; WIDE_ADVICE] = std::array::from_fn(|_| meta.advice_column());
        for column in advice {
            meta.enable_equality(column);
        }
        let instance = meta.instance_column();
        meta.enable_equality(instance);
        let product = meta.fixed_column();
        let range = meta.lookup_table_column();
        let square = meta.lookup_table_column();
        for group in 0..WIDE_GROUPS {
            meta.create_gate("wide product", |cells| {
                let enable = cells.query_fixed(product, Rotation::cur());
                let x = cells.query_advice(advice[3 * group], Rotation::cur());
                let y = cells.query_advice(advice[3 * group + 1], Rotation::cur());
                let z = cells.query_advice(advice[3 * group + 2], Rotation::cur());
                let next = cells.query_advice(advice[3 * group], Rotation::next());
                vec![enable * (x * y * z - next)]
            });
        }
        // Degree-one inputs keep the lookup argument at the circuit's degree 4.
        meta.lookup("wide range", |cells| {
            vec![(cells.query_advice(advice[1], Rotation::cur()), range)]
        });
        meta.lookup("wide square", |cells| {
            vec![
                (cells.query_advice(advice[4], Rotation::cur()), range),
                (cells.query_advice(advice[8], Rotation::cur()), square),
            ]
        });
        WideConfig {
            advice,
            product,
            range,
            square,
            instance,
        }
    }

    fn synthesize(&self, config: WideConfig, mut layouter: impl Layouter<F>) -> Result<(), Error> {
        let table_size = 1_u64 << table_bits(self.k);
        layouter.assign_table(
            || "wide tables",
            |mut table| {
                for value in 0..table_size {
                    let offset = usize::try_from(value).expect("table offset fits usize");
                    let element = F::from(value);
                    table.assign_cell(
                        || "range",
                        config.range,
                        offset,
                        || Value::known(element),
                    )?;
                    table.assign_cell(
                        || "square",
                        config.square,
                        offset,
                        || Value::known(element.square()),
                    )?;
                }
                Ok(())
            },
        )?;
        let used = self.rows.len();
        let public_cell = layouter.assign_region(
            || "wide rows",
            |mut region| {
                let mut first_chains: Vec<Cell> = Vec::with_capacity(WIDE_GROUPS);
                let mut last_chain: Option<Cell> = None;
                for (row, values) in self.rows.iter().enumerate() {
                    let enable = if row + 1 < used { F::ONE } else { F::ZERO };
                    region.assign_fixed(config.product, row, enable);
                    let cells: [Cell; WIDE_ADVICE] = std::array::from_fn(|column| {
                        let value = if self.witness {
                            Value::known(values[column])
                        } else {
                            Value::unknown()
                        };
                        region
                            .assign_advice(config.advice[column], row, value)
                            .cell()
                    });
                    if row == 0 {
                        first_chains.extend((0..WIDE_GROUPS).map(|group| cells[3 * group]));
                    }
                    // Rotate the duplicated pair that is copy-constrained on this row.
                    let (left, right) = match row % 4 {
                        0 => (1, 10),
                        1 => (4, 7),
                        2 => (2, 5),
                        _ => (8, 11),
                    };
                    region.constrain_equal(cells[left], cells[right]);
                    last_chain = Some(cells[9]);
                }
                for pair in first_chains.windows(2) {
                    region.constrain_equal(pair[0], pair[1]);
                }
                Ok(last_chain.expect("wide circuit has rows"))
            },
        )?;
        layouter.constrain_instance(public_cell, config.instance, 0);
        Ok(())
    }
}

/// Prove `circuit` once with the Blake2b transcript and the ChaCha20 `seed`.
fn prove_once<C, Circ>(
    params: &ParamsIPA<C>,
    pk: &ProvingKey<C>,
    circuit: &Circ,
    public: &[C::Scalar],
    seed: [u8; 32],
) -> Vec<u8>
where
    C: CurveAffine,
    C::Scalar: WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
    Circ: Circuit<C::Scalar> + Clone,
{
    let columns: [&[C::Scalar]; 1] = [public];
    let instances: [&[&[C::Scalar]]; 1] = [&columns];
    let mut transcript = Blake2bWrite::<_, C, Challenge255<C>>::init(Vec::new());
    create_proof::<IPACommitmentScheme<C>, ProverIPA<'_, C>, _, _, _, _>(
        params,
        pk,
        std::slice::from_ref(circuit),
        &instances,
        ChaCha20Rng::from_seed(seed),
        &mut transcript,
    )
    .expect("golden proof generation");
    transcript.finalize()
}

/// Verify one golden proof natively.
fn verify_once<C>(params: &ParamsIPA<C>, pk: &ProvingKey<C>, public: &[C::Scalar], proof: &[u8])
where
    C: CurveAffine,
    C::Scalar: WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let columns: [&[C::Scalar]; 1] = [public];
    let instances: [&[&[C::Scalar]]; 1] = [&columns];
    let mut transcript = Blake2bRead::<_, C, Challenge255<C>>::init(proof);
    let strategy = SingleStrategy::<C>::new(params);
    verify_proof::<IPACommitmentScheme<C>, VerifierIPA<'_, C>, _, _, _>(
        params,
        pk.get_vk(),
        strategy,
        &instances,
        &mut transcript,
    )
    .expect("golden proof verifies");
}

/// Hex-encoded SHA-256 of `bytes`.
fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Prove `circuit` for every seed in every pool and return `(case, sha256)` per seed.
///
/// Asserts that every pool produces identical bytes and that the first proof of each seed
/// verifies.
fn golden_digests<C, Circ>(
    name: &str,
    k: u32,
    shape: CircuitShape,
    circuit: &Circ,
    public: &[C::Scalar],
) -> Vec<(String, String)>
where
    C: CurveAffine,
    C::Scalar: WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
    Circ: Circuit<C::Scalar> + Clone + Sync,
{
    let params = ParamsIPA::<C>::new(k);
    let vk = keygen_vk(&params, &circuit.without_witnesses()).expect("golden verifying key");
    let pk = keygen_pk(&params, vk, &circuit.without_witnesses()).expect("golden proving key");
    let cs = pk.get_vk().cs();
    assert_eq!(
        CircuitShape {
            degree: cs.degree(),
            permutation_columns: cs.permutation().get_columns().len(),
        },
        shape,
        "{name}: constraint-system shape"
    );
    let mut digests = Vec::with_capacity(PROVER_SEEDS.len());
    for seed in PROVER_SEEDS {
        let mut reference: Option<Vec<u8>> = None;
        for threads in THREAD_POOLS {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("golden Rayon pool");
            let proof = pool.install(|| prove_once(&params, &pk, circuit, public, seed));
            match &reference {
                None => reference = Some(proof),
                Some(expected) => assert!(
                    expected == &proof,
                    "{name}/seed{}: proof bytes differ between 1 and {threads} threads",
                    seed[0]
                ),
            }
        }
        let proof = reference.expect("at least one pool");
        verify_once(&params, &pk, public, &proof);
        let case = format!("{name}/seed{}", seed[0]);
        let digest = sha256_hex(&proof);
        println!(
            "HALO2_GOLDEN case={case} bytes={} sha256={digest}",
            proof.len()
        );
        digests.push((case, digest));
    }
    digests
}

/// Compare computed digests with [`GOLDEN_SHA256`], reporting every mismatch at once.
fn assert_goldens(digests: &[(String, String)]) {
    let mismatches = digests
        .iter()
        .filter_map(|(case, digest)| {
            let expected = GOLDEN_SHA256
                .iter()
                .find(|(name, _)| name == case)
                .map(|(_, value)| *value);
            (expected != Some(digest.as_str()))
                .then(|| format!("{case}: expected {expected:?}, got {digest}"))
        })
        .collect::<Vec<_>>();
    assert!(
        mismatches.is_empty(),
        "golden proof bytes changed:\n{}",
        mismatches.join("\n")
    );
}

/// Run the sigma circuit at every `k` over curve `C`.
fn sigma_goldens<C>(curve: &str, ks: &[u32])
where
    C: CurveAffine,
    C::Scalar: WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let digests = ks
        .iter()
        .flat_map(|&k| {
            let circuit = SigmaCircuit::<C::Scalar>::new(k);
            let public = circuit.public();
            golden_digests::<C, _>(
                &format!("sigma/{curve}/k{k}"),
                k,
                SIGMA_SHAPE,
                &circuit,
                &public,
            )
        })
        .collect::<Vec<_>>();
    assert_goldens(&digests);
}

/// Run the wide circuit at every `k` over curve `C`.
fn wide_goldens<C>(curve: &str, ks: &[u32])
where
    C: CurveAffine,
    C::Scalar: WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let digests = ks
        .iter()
        .flat_map(|&k| {
            let circuit = WideCircuit::<C::Scalar>::new(k);
            let public = circuit.public();
            golden_digests::<C, _>(
                &format!("wide/{curve}/k{k}"),
                k,
                WIDE_SHAPE,
                &circuit,
                &public,
            )
        })
        .collect::<Vec<_>>();
    assert_goldens(&digests);
}

#[test]
fn sigma_eq_proof_bytes_are_golden() {
    sigma_goldens::<EqAffine>("eq", &[6, 9]);
}

#[test]
fn sigma_ep_proof_bytes_are_golden() {
    sigma_goldens::<EpAffine>("ep", &[6, 9]);
}

#[test]
#[ignore = "k = 11 golden; run in release"]
fn sigma_eq_k11_proof_bytes_are_golden() {
    sigma_goldens::<EqAffine>("eq", &[11]);
}

#[test]
#[ignore = "k = 11 golden; run in release"]
fn sigma_ep_k11_proof_bytes_are_golden() {
    sigma_goldens::<EpAffine>("ep", &[11]);
}

#[test]
fn wide_eq_proof_bytes_are_golden() {
    wide_goldens::<EqAffine>("eq", &[8, 10]);
}

#[test]
fn wide_ep_proof_bytes_are_golden() {
    wide_goldens::<EpAffine>("ep", &[8, 10]);
}

#[test]
fn golden_table_names_are_unique_and_well_formed() {
    let mut names = GOLDEN_SHA256
        .iter()
        .map(|(name, digest)| {
            assert_eq!(digest.len(), 64, "{name}: SHA-256 hex length");
            assert!(
                digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "{name}: SHA-256 hex digits"
            );
            *name
        })
        .collect::<Vec<_>>();
    let count = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), count, "duplicate golden case names");
}
