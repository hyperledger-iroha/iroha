use std::{
    cmp, fmt, iter,
    num::ParseIntError,
    str::FromStr,
    time::{Duration, Instant},
};

use ff::Field;
use group::{Curve, Group};
use gumdrop::Options;
use blake2b_simd::Params as Blake2bParams;
use halo2_proofs::arithmetic::best_multiexp;
use halo2curves::pasta::pallas;
use rand_core::{OsRng, RngCore};

struct Estimator {
    /// Scalars for estimating multiexp performance.
    multiexp_scalars: Vec<pallas::Scalar>,
    /// Bases for estimating multiexp performance.
    multiexp_bases: Vec<pallas::Affine>,
    /// Pre-sampled input used to benchmark BLAKE2b hashing.
    blake_block: Vec<u8>,
}

impl fmt::Debug for Estimator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Estimator")
    }
}

impl Estimator {
    fn random(k: usize) -> Self {
        let max_size = 1 << (k + 1);
        let mut rng = OsRng;
        let mut blake_block = vec![0u8; 128];
        rng.fill_bytes(&mut blake_block);

        Estimator {
            multiexp_scalars: (0..max_size)
                .map(|_| pallas::Scalar::random(&mut rng))
                .collect(),
            multiexp_bases: (0..max_size)
                .map(|_| pallas::Point::random(&mut rng).to_affine())
                .collect(),
            blake_block,
        }
    }

    fn multiexp(&self, size: usize) -> Duration {
        let start = Instant::now();
        best_multiexp(&self.multiexp_scalars[..size], &self.multiexp_bases[..size]);
        Instant::now().duration_since(start)
    }

    fn blake2b_blocks(&self, blocks: usize) -> Duration {
        if blocks == 0 {
            return Duration::ZERO;
        }
        let params = Blake2bParams::new();
        let mut total = Duration::ZERO;
        for _ in 0..blocks {
            let start = Instant::now();
            let _ = params.hash(&self.blake_block);
            total += start.elapsed();
        }
        total
    }

    fn blake_block_len(&self) -> usize {
        self.blake_block.len()
    }
}

#[derive(Debug, Options)]
struct CostOptions {
    #[options(help = "Print this message.")]
    help: bool,

    #[options(
        help = "An advice column with the given rotations. May be repeated.",
        meta = "R[,R..]"
    )]
    advice: Vec<Poly>,

    #[options(
        help = "An instance column with the given rotations. May be repeated.",
        meta = "R[,R..]"
    )]
    instance: Vec<Poly>,

    #[options(
        help = "A fixed column with the given rotations. May be repeated.",
        meta = "R[,R..]"
    )]
    fixed: Vec<Poly>,

    #[options(help = "Maximum degree of the custom gates.", meta = "D")]
    gate_degree: usize,

    #[options(
        help = "A lookup over N columns with max input degree I and max table degree T. May be repeated.",
        meta = "N,I,T"
    )]
    lookup: Vec<Lookup>,

    #[options(help = "A permutation over N columns. May be repeated.", meta = "N")]
    permutation: Vec<Permutation>,

    #[options(free, required, help = "2^K bound on the number of rows.")]
    k: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Poly {
    rotations: Vec<isize>,
}

impl FromStr for Poly {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut rotations: Vec<isize> =
            s.split(',').map(|r| r.parse()).collect::<Result<_, _>>()?;
        rotations.sort_unstable();
        Ok(Poly { rotations })
    }
}

#[derive(Debug)]
struct Lookup {
    _columns: usize,
    input_deg: usize,
    table_deg: usize,
}

impl FromStr for Lookup {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(',');
        let _columns = parts.next().unwrap().parse()?;
        let input_deg = parts.next().unwrap().parse()?;
        let table_deg = parts.next().unwrap().parse()?;
        Ok(Lookup {
            _columns,
            input_deg,
            table_deg,
        })
    }
}

impl Lookup {
    fn required_degree(&self) -> usize {
        2 + cmp::max(1, self.input_deg) + cmp::max(1, self.table_deg)
    }

    fn queries(&self) -> impl Iterator<Item = Poly> {
        // - product commitments at x and x_inv
        // - input commitments at x and x_inv
        // - table commitments at x
        let product = "0,-1".parse().unwrap();
        let input = "0,-1".parse().unwrap();
        let table = "0".parse().unwrap();

        iter::empty()
            .chain(Some(product))
            .chain(Some(input))
            .chain(Some(table))
    }
}

#[derive(Debug)]
struct Permutation {
    columns: usize,
}

impl FromStr for Permutation {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Permutation {
            columns: s.parse()?,
        })
    }
}

impl Permutation {
    fn required_degree(&self) -> usize {
        cmp::max(self.columns + 1, 2)
    }

    fn queries(&self) -> impl Iterator<Item = Poly> {
        // - product commitments at x and x_inv
        // - polynomial commitments at x
        let product = "0,-1".parse().unwrap();
        let poly = "0".parse().unwrap();

        iter::empty()
            .chain(Some(product))
            .chain(iter::repeat(poly).take(self.columns))
    }
}

#[derive(Debug)]
struct Circuit {
    /// Power-of-2 bound on the number of rows in the circuit.
    k: usize,
    /// Maximum degree of the circuit.
    max_deg: usize,
    /// Number of advice columns.
    advice_columns: usize,
    /// Number of lookup arguments.
    lookups: usize,
    /// Equality constraint permutation arguments.
    permutations: Vec<Permutation>,
    /// Number of distinct column queries across all gates.
    column_queries: usize,
    /// Number of distinct sets of points in the multiopening argument.
    point_sets: usize,

    estimator: Estimator,
}

impl From<CostOptions> for Circuit {
    fn from(opts: CostOptions) -> Self {
        let max_deg = [1, opts.gate_degree]
            .iter()
            .cloned()
            .chain(opts.lookup.iter().map(|l| l.required_degree()))
            .chain(opts.permutation.iter().map(|p| p.required_degree()))
            .max()
            .unwrap();

        let mut queries: Vec<_> = iter::empty()
            .chain(opts.advice.iter())
            .chain(opts.instance.iter())
            .chain(opts.fixed.iter())
            .cloned()
            .chain(opts.lookup.iter().flat_map(|l| l.queries()))
            .chain(opts.permutation.iter().flat_map(|p| p.queries()))
            .chain(iter::repeat("0".parse().unwrap()).take(max_deg - 1))
            .collect();

        let column_queries = queries.len();
        queries.sort_unstable();
        queries.dedup();
        let point_sets = queries.len();

        Circuit {
            k: opts.k,
            max_deg,
            advice_columns: opts.advice.len(),
            lookups: opts.lookup.len(),
            permutations: opts.permutation,
            column_queries,
            point_sets,
            estimator: Estimator::random(opts.k),
        }
    }
}

impl Circuit {
    fn proof_size(&self) -> usize {
        let size = |points: usize, scalars: usize| points * 32 + scalars * 32;

        // PLONK:
        // - 32 bytes (commitment) per advice column
        // - 3 * 32 bytes (commitments) + 5 * 32 bytes (evals) per lookup argument
        // - 32 bytes (commitment) + 2 * 32 bytes (evals) per permutation argument
        // - 32 bytes (eval) per column per permutation argument
        let plonk = size(1, 0) * self.advice_columns
            + size(3, 5) * self.lookups
            + self
                .permutations
                .iter()
                .map(|p| size(1, 2 + p.columns))
                .sum::<usize>();

        // Vanishing argument:
        // - (max_deg - 1) * 32 bytes (commitments) + (max_deg - 1) * 32 bytes (h_evals)
        //   for quotient polynomial
        // - 32 bytes (eval) per column query
        let vanishing = size(self.max_deg - 1, self.max_deg - 1) + size(0, self.column_queries);

        // Multiopening argument:
        // - f_commitment (32 bytes)
        // - 32 bytes (evals) per set of points in multiopen argument
        let multiopen = size(1, self.point_sets);

        // Polycommit:
        // - s_poly commitment (32 bytes)
        // - inner product argument (k rounds * 2 * 32 bytes)
        // - a (32 bytes)
        // - xi (32 bytes)
        let polycomm = size(1 + 2 * self.k, 2);

        plonk + vanishing + multiopen + polycomm
    }

    fn verification_time(&self) -> Duration {
        // TODO: This isn't accurate; most of these will have zero scalars.
        let g_scalars = 1 << self.k;

        // - f_commitment
        // - q_commitments
        let multiopen = 1 + self.column_queries;

        // - \iota
        // - Rounds
        // - H
        // - U
        let polycomm = 1 + (2 * self.k) + 1 + 1;

        let multiexp_time = self
            .estimator
            .multiexp(g_scalars + multiopen + polycomm);

        // Rough transcript size estimate: proof elements plus challenge material (32 bytes each).
        let transcript_bytes = self.proof_size()
            + 32 * (self.point_sets + self.permutations.len() + self.lookups + 6);
        let block_size = self.estimator.blake_block_len();
        let hash_blocks = if transcript_bytes == 0 {
            0
        } else {
            (transcript_bytes + block_size - 1) / block_size
        };
        let blake_time = self.estimator.blake2b_blocks(hash_blocks);

        blake_time + multiexp_time
    }
}

fn main() {
    let opts = CostOptions::parse_args_default_or_exit();
    let c = Circuit::from(opts);
    println!("{:#?}", c);
    println!("Proof size: {} bytes", c.proof_size());
    println!(
        "Verification: at least {}ms",
        c.verification_time().as_micros() as f64 / 1_000f64
    );
}
