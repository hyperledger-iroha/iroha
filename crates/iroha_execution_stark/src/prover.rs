//! The STARK prover.

use rayon::prelude::*;

use crate::air::{BetaPowers, CompiledAir, quotient_log_blowup};
use crate::circuit::Circuit;
use crate::field::{Fp, Fp4, GENERATOR, batch_inverse_ext};
use crate::fri::FriCommitment;
use crate::hash::{DIGEST_BYTES, FramedHash, hash};
use crate::merkle::{Digest, MerkleTree, hash_leaf};
use crate::ntt::{
    CosetPowers, Domain, barycentric_weights, evaluate_coefficients, evaluate_with_weights,
};
use crate::params::Params;
use crate::proof::{Proof, Statement};
use crate::trace::{self, TraceError};
use crate::transcript::Transcript;

/// Proving failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProveError {
    /// Witness generation failed.
    #[error(transparent)]
    Trace(#[from] TraceError),
    /// The statement does not match the supplied states.
    #[error("statement boundary does not match the supplied states")]
    Boundary,
    /// The witness does not satisfy the circuit (a circuit bug).
    #[error("constraint system is not satisfied by the witness")]
    Unsatisfied,
    /// The parameters cannot be used with this circuit.
    #[error(transparent)]
    Parameters(#[from] crate::params::ParamsError),
    /// The out-of-domain point collided with a domain (negligible probability).
    #[error("out-of-domain sample collided with the evaluation domain")]
    BadSample,
}

/// Optional per-phase wall/CPU report on stderr (`IROHA_EXECUTION_STARK_TIMINGS=1`).
struct Phases {
    enabled: bool,
    start: std::time::Instant,
    last: std::time::Instant,
    last_cpu: Option<f64>,
}

#[cfg(unix)]
#[allow(clippy::cast_precision_loss)] // diagnostics only; seconds and nanoseconds are small
fn process_cpu_seconds() -> Option<f64> {
    let t = rustix::time::clock_gettime(rustix::time::ClockId::ProcessCPUTime);
    if t.tv_sec < 0 || !(0..1_000_000_000).contains(&t.tv_nsec) {
        return None;
    }
    Some((t.tv_nsec as f64).mul_add(1e-9, t.tv_sec as f64))
}

#[cfg(not(unix))]
fn process_cpu_seconds() -> Option<f64> {
    None
}

fn cpu_elapsed_label(previous: Option<f64>, current: Option<f64>) -> String {
    match previous.zip(current) {
        Some((previous, current)) if current >= previous => {
            format!("{:>7.3} s", current - previous)
        }
        _ => "unavailable".to_owned(),
    }
}

impl Phases {
    fn new() -> Self {
        let now = std::time::Instant::now();
        let enabled = std::env::var_os("IROHA_EXECUTION_STARK_TIMINGS").is_some();
        Self {
            enabled,
            start: now,
            last: now,
            last_cpu: if enabled { process_cpu_seconds() } else { None },
        }
    }

    fn mark(&mut self, phase: &str) {
        if self.enabled {
            let now = std::time::Instant::now();
            let cpu = process_cpu_seconds();
            eprintln!(
                "  {phase:<28} wall {:>7.3} s  cpu {}   (total wall {:>7.3} s)",
                (now - self.last).as_secs_f64(),
                cpu_elapsed_label(self.last_cpu, cpu),
                (now - self.start).as_secs_f64()
            );
            self.last = now;
            self.last_cpu = cpu;
        }
    }
}

/// Rows per parallel task when evaluating constraints.
const EVAL_BLOCK: usize = 32;

/// Rows per parallel task when walking a matrix by rows.
const ROW_BLOCK: usize = 64;

/// A committed matrix: low-degree extensions (coset-major per column) and its row tree.
struct Committed {
    /// `lde[c][j * n + k]` is column `c` at LDE point `shift * v^(j + blowup*k)`.
    lde: Vec<Vec<Fp>>,
    tree: MerkleTree,
}

fn commit_columns(coefficients: &[Vec<Fp>], domain: &Domain, powers: &CosetPowers) -> Committed {
    let n = domain.size();
    let blowup = powers.powers.len();
    let lde: Vec<Vec<Fp>> = coefficients
        .par_iter()
        .map(|coeffs| {
            let mut flat = vec![Fp::ZERO; n * blowup];
            domain.low_degree_extend_into(coeffs, powers, &mut flat);
            flat
        })
        .collect();
    let width = lde.len();
    let leaves = n * blowup;
    let mut digests = vec![[0u8; DIGEST_BYTES]; leaves];
    // Leaf i is (coset j = i % blowup, position k = i / blowup); hash blocks of positions per coset.
    let blocks: Vec<(usize, usize)> = (0..blowup)
        .flat_map(|j| (0..n).step_by(ROW_BLOCK).map(move |k0| (j, k0)))
        .collect();
    let block_digests: Vec<(usize, usize, Vec<Digest>)> = blocks
        .par_iter()
        .map(|&(j, k0)| {
            let rows = ROW_BLOCK.min(n - k0);
            let mut tile = vec![0u8; rows * width * 8];
            for (c, column) in lde.iter().enumerate() {
                let base = j * n + k0;
                for r in 0..rows {
                    let offset = (r * width + c) * 8;
                    tile[offset..offset + 8].copy_from_slice(&column[base + r].to_le_bytes());
                }
            }
            let digests = (0..rows)
                .map(|r| hash_leaf(&tile[r * width * 8..(r + 1) * width * 8]))
                .collect();
            (j, k0, digests)
        })
        .collect();
    for (j, k0, block) in block_digests {
        for (r, digest) in block.into_iter().enumerate() {
            digests[j + blowup * (k0 + r)] = digest;
        }
    }
    Committed {
        lde,
        tree: MerkleTree::new(digests),
    }
}

impl Committed {
    fn row(&self, index: usize, n: usize, blowup: usize) -> Vec<Fp> {
        let (j, k) = (index % blowup, index / blowup);
        self.lde.iter().map(|column| column[j * n + k]).collect()
    }
}

fn interpolate_columns(columns: &[Vec<Fp>], domain: &Domain) -> Vec<Vec<Fp>> {
    columns
        .par_iter()
        .map(|column| {
            let mut coeffs = column.clone();
            domain.inverse(&mut coeffs);
            coeffs
        })
        .collect()
}

/// Canonical bytes of the range-table multiplicities, absorbed before the LogUp challenge.
pub fn encode_multiplicities(multiplicities: &[u64]) -> Vec<u8> {
    multiplicities
        .iter()
        .flat_map(|m| m.to_le_bytes())
        .collect()
}

/// `1/(beta - v)` for every table value `v < 2^bits`.
pub fn table_inverses(beta: Fp4, bits: u32) -> Vec<Fp4> {
    let mut out: Vec<Fp4> = (0..1u64 << bits)
        .map(|v| beta - Fp4::from_base(Fp::new(v)))
        .collect();
    out.par_chunks_mut(1024).for_each(batch_inverse_ext);
    out
}

/// The table side of the LogUp identity: `sum_v m_v / (beta - v)`.
pub fn table_sum(multiplicities: &[u64], inverses: &[Fp4]) -> Fp4 {
    multiplicities
        .par_iter()
        .zip(inverses)
        .map(|(m, inv)| inv.mul_base(Fp::new(*m)))
        .reduce(|| Fp4::ZERO, |a, b| a + b)
}

/// Digest of the public columns, bound into the transcript.
pub fn public_digest(public: &[Vec<i64>]) -> Digest {
    let mut hasher = FramedHash::new(b"public columns");
    hasher.update(&(public.len() as u64).to_le_bytes());
    for column in public {
        hasher.update(&(column.len() as u64).to_le_bytes());
        for value in column {
            hasher.update(&value.to_le_bytes());
        }
    }
    hasher.finalize()
}

/// Circuit identity: framed BLAKE2b-384 of the circuit's canonical description.
pub fn circuit_id(circuit: &Circuit) -> Digest {
    hash(b"circuit id", &[&circuit.canonical_bytes()])
}

/// Starts the transcript shared by prover and verifier.
pub fn initial_transcript(
    circuit: &Circuit,
    params: &Params,
    statement: &Statement,
    public: &[Vec<i64>],
) -> Transcript {
    let mut transcript = Transcript::new(b"iroha_execution_stark blake2b-384 v1");
    transcript.absorb_digest(b"circuit", &circuit_id(circuit));
    let mut p = Vec::new();
    for v in [
        u64::from(params.log_blowup),
        params.queries as u64,
        u64::from(params.fri_log_arity),
        u64::from(params.fri_log_final_degree),
        u64::from(params.grinding_bits),
    ] {
        p.extend_from_slice(&v.to_le_bytes());
    }
    transcript.absorb(b"params", &p);
    transcript.absorb(b"steps", &(statement.steps as u64).to_le_bytes());
    let encode = |values: &[i64]| {
        values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>()
    };
    transcript.absorb(b"initial", &encode(&statement.initial));
    transcript.absorb(b"final", &encode(&statement.final_state));
    transcript.absorb(b"checkpoint", &statement.checkpoint_bytes());
    transcript.absorb(b"binding", &statement.binding);
    transcript.absorb_digest(b"public", &public_digest(public));
    transcript
}

/// The DEEP combination order: main columns, aux base columns, quotient base columns at `z`, then
/// next-referenced main columns and the accumulator's base columns at `w*z`.
pub(crate) struct DeepLayout {
    pub(crate) main: usize,
    pub(crate) aux_ext: usize,
    pub(crate) quotient_ext: usize,
    pub(crate) next_main: Vec<u32>,
    pub(crate) next_aux: bool,
}

impl DeepLayout {
    /// Coefficients for items at `z` (one per extension-valued item) and at `w*z`.
    pub(crate) fn coefficients(&self, gamma: Fp4) -> (Vec<Fp4>, Vec<Fp4>) {
        let at_z = self.main + self.aux_ext + self.quotient_ext;
        let at_wz = self.next_main.len() + usize::from(self.next_aux);
        let mut powers = Vec::with_capacity(at_z + at_wz);
        let mut acc = Fp4::ONE;
        for _ in 0..at_z + at_wz {
            powers.push(acc);
            acc *= gamma;
        }
        let wz = powers.split_off(at_z);
        (powers, wz)
    }
}

/// Basis element `X^c` of the extension.
pub(crate) fn basis(component: usize) -> Fp4 {
    let mut out = Fp4::ZERO;
    out.0[component] = Fp::ONE;
    out
}

/// Recombines four base coordinates into an extension element.
pub(crate) fn from_components(components: &[Fp]) -> Fp4 {
    Fp4([components[0], components[1], components[2], components[3]])
}

/// Proves a statement.
///
/// The single-letter names follow the protocol notation: `n` rows, OOD point `z`, `v` the LDE
/// generator, `q` quotient-domain blowups, `j`/`k` coset and position.
///
/// `public_inputs[c][row]` are the application's public columns for every row; `states[row]` is the register
/// vector before step `row`, for `row in 0..=steps`.
///
/// # Errors
/// Fails on malformed inputs, simulation divergence or an unsatisfied circuit.
#[allow(clippy::many_single_char_names)]
pub fn prove(
    circuit: &Circuit,
    params: &Params,
    statement: &Statement,
    public_inputs: &[Vec<i64>],
    states: &[Vec<i64>],
) -> Result<Proof, ProveError> {
    params.check(circuit)?;
    if states.first() != Some(&statement.initial)
        || states.get(statement.steps) != Some(&statement.final_state)
    {
        return Err(ProveError::Boundary);
    }
    let mut phases = Phases::new();
    if let Some(checkpoint) = &statement.checkpoint
        && states.get(checkpoint.step) != Some(&checkpoint.registers)
    {
        return Err(ProveError::Boundary);
    }
    let public = trace::public_columns(
        circuit,
        statement.steps,
        statement.checkpoint_step(),
        public_inputs,
    )?;
    let main_trace = trace::generate(circuit, &public, statement.steps, states)?;
    let air = CompiledAir::new(
        circuit,
        &statement.initial,
        &statement.final_state,
        &statement.checkpoint_registers(circuit.registers.len()),
    );
    phases.mark("witness");

    let log_n = circuit.log_rows;
    let n = circuit.rows();
    let blowup = 1usize << params.log_blowup;
    let shift = Fp::new(GENERATOR);
    let domain = Domain::new(log_n);
    let powers = CosetPowers::new(&domain, blowup, shift);
    let mut transcript = initial_transcript(circuit, params, statement, &public);

    // Main trace.
    let main_coefficients = interpolate_columns(&main_trace.columns, &domain);
    let main = commit_columns(&main_coefficients, &domain, &powers);
    transcript.absorb_digest(b"main-root", &main.tree.root());
    phases.mark("main commit (LDE+Merkle)");

    // LogUp: multiplicities are absorbed before the challenge; the verifier sums the table side.
    let has_lookups = !circuit.lookups.is_empty();
    if has_lookups {
        transcript.absorb(
            b"multiplicities",
            &encode_multiplicities(&main_trace.multiplicities),
        );
    }
    let width = air.layout().width();
    let public_fp: Vec<Vec<Fp>> = public
        .iter()
        .map(|c| c.iter().map(|v| Fp::from_i64(*v)).collect())
        .collect();
    let n_inverse = Fp::new(n as u64).inverse().expect("n != 0");
    let (beta, table_per_row, aux_columns_ext) = if has_lookups {
        let beta = transcript.challenge_fp4(b"logup-beta");
        let table_inverses = table_inverses(beta, circuit.range_bits);
        let table_per_row =
            table_sum(&main_trace.multiplicities, &table_inverses).mul_base(n_inverse);
        let helper_count = air.aux_columns() - 1;
        let rows: Vec<Vec<Fp4>> = (0..n)
            .into_par_iter()
            .map(|k| {
                let mut row = vec![Fp::ZERO; width];
                for (c, column) in main_trace.columns.iter().enumerate() {
                    row[c] = column[k];
                }
                for (p, column) in public_fp.iter().enumerate() {
                    row[air.layout().main + p] = column[k];
                }
                let mut values = Vec::new();
                air.lookup_values(&row, &mut values);
                let inverses: Vec<Fp4> = values
                    .iter()
                    .map(|v| table_inverses[usize::try_from(v.as_u64()).expect("lookup in table")])
                    .collect();
                let mut helpers = Vec::with_capacity(helper_count);
                air.helper_values(&inverses, &mut helpers);
                helpers
            })
            .collect();
        // Accumulator: S_0 = 0, S_{k+1} = S_k + sum(helpers_k) - table/n, returning to 0 after n rows.
        let mut columns: Vec<Vec<Fp4>> = vec![vec![Fp4::ZERO; n]; helper_count + 1];
        let mut running = Fp4::ZERO;
        for (k, row) in rows.iter().enumerate() {
            columns[helper_count][k] = running;
            let mut delta = -table_per_row;
            for (c, value) in row.iter().enumerate() {
                columns[c][k] = *value;
                delta += *value;
            }
            running += delta;
        }
        if !running.is_zero() {
            return Err(ProveError::Unsatisfied);
        }
        (beta, table_per_row, columns)
    } else {
        (Fp4::ZERO, Fp4::ZERO, Vec::new())
    };
    let aux_base: Vec<Vec<Fp>> = aux_columns_ext
        .iter()
        .flat_map(|column| (0..4).map(move |c| column.iter().map(|v| v.0[c]).collect::<Vec<Fp>>()))
        .collect();
    let aux_coefficients = interpolate_columns(&aux_base, &domain);
    phases.mark("logup columns");
    let aux = has_lookups.then(|| commit_columns(&aux_coefficients, &domain, &powers));
    if let Some(aux) = &aux {
        transcript.absorb_digest(b"aux-root", &aux.tree.root());
    }
    phases.mark("logup commit (LDE+Merkle)");

    // Composition quotients: the low group on a 2n-sized coset domain, LogUp helpers on their own.
    let alpha = transcript.challenge_fp4(b"composition-alpha");
    let mut alphas = Vec::with_capacity(air.total_constraints);
    let mut acc = Fp4::ONE;
    for _ in 0..air.total_constraints {
        alphas.push(acc);
        acc *= alpha;
    }
    let shape = air.shape();
    let beta_powers = BetaPowers::new(beta);
    let (low_chunks, high_chunks) = (shape.low_chunks(), shape.high_chunks());
    let q_low_log = quotient_log_blowup(shape.low_degree);
    let q_high_log = if high_chunks > 0 {
        quotient_log_blowup(shape.high_degree)
    } else {
        0
    };
    let q_max_log = q_low_log.max(q_high_log);
    debug_assert!(1usize << q_max_log <= blowup, "checked by Params::check");
    let v = Fp::two_adic_generator(log_n + params.log_blowup);
    let public_coefficients = interpolate_columns(&public_fp, &domain);
    let last_point = domain.element(n - 1);
    let mut low_values = vec![Fp4::ZERO; n << q_low_log];
    let mut high_values = vec![Fp4::ZERO; if high_chunks > 0 { n << q_high_log } else { 0 }];
    let low_stride = blowup >> q_low_log;
    let high_stride = blowup >> q_high_log;
    for u in 0..1usize << q_max_log {
        let j = u * (blowup >> q_max_log);
        let in_low = j % low_stride == 0;
        let in_high = high_chunks > 0 && j % high_stride == 0;
        let coset_shift = shift * v.pow(j as u64);
        let public_coset: Vec<Vec<Fp>> = public_coefficients
            .par_iter()
            .map(|coeffs| {
                let mut values = coeffs.clone();
                domain.forward_on_coset(&mut values, coset_shift);
                values
            })
            .collect();
        // x^n - 1 is constant on the coset.
        let zerofier_inverse = (coset_shift.pow(n as u64) - Fp::ONE)
            .inverse()
            .ok_or(ProveError::BadSample)?;
        let layout = air.layout();
        let row_width = layout.row();
        let aux_width = aux_columns_ext.len();
        let block_values: Vec<(usize, Vec<Fp4>, Vec<Fp4>)> = (0..n)
            .step_by(EVAL_BLOCK)
            .collect::<Vec<_>>()
            .par_iter()
            .map(|&k0| {
                let rows = EVAL_BLOCK.min(n - k0);
                // Row-major tile of rows k0..=k0+rows (the last one is the "next" of the block's end).
                let mut tile = vec![Fp::ZERO; (rows + 1) * row_width];
                for (c, column) in main.lde.iter().enumerate() {
                    let coset = &column[j * n..(j + 1) * n];
                    for r in 0..=rows {
                        tile[r * row_width + c] = coset[(k0 + r) % n];
                    }
                }
                for (p, column) in public_coset.iter().enumerate() {
                    for r in 0..=rows {
                        tile[r * row_width + layout.main + p] = column[(k0 + r) % n];
                    }
                }
                let mut aux_tile = vec![Fp4::ZERO; (rows + 1) * aux_width];
                if let Some(aux) = &aux {
                    for a in 0..aux_width {
                        let parts: [&[Fp]; 4] =
                            std::array::from_fn(|c| &aux.lde[4 * a + c][j * n..(j + 1) * n]);
                        for r in 0..=rows {
                            let k = (k0 + r) % n;
                            aux_tile[r * aux_width + a] =
                                Fp4([parts[0][k], parts[1][k], parts[2][k], parts[3][k]]);
                        }
                    }
                }
                let mut low = Vec::with_capacity(if in_low { rows } else { 0 });
                let mut high = Vec::with_capacity(if in_high { rows } else { 0 });
                for r in 0..rows {
                    let k = k0 + r;
                    let row = &tile[r * row_width..(r + 2) * row_width];
                    let aux_row = &aux_tile[r * aux_width..(r + 1) * aux_width];
                    if in_low {
                        let accumulator_next = if aux_width > 0 {
                            aux_tile[(r + 1) * aux_width + aux_width - 1]
                        } else {
                            Fp4::ZERO
                        };
                        let (every, transition) =
                            air.combine_low(row, aux_row, accumulator_next, table_per_row, &alphas);
                        let x = coset_shift * domain.element(k);
                        let numerator = every + transition * Fp4::from_base(x - last_point);
                        low.push(numerator.mul_base(zerofier_inverse));
                    }
                    if in_high {
                        high.push(
                            air.combine_helpers(row, aux_row, &beta_powers, &alphas)
                                .mul_base(zerofier_inverse),
                        );
                    }
                }
                (k0, low, high)
            })
            .collect();
        let (q_low, q_high) = (1usize << q_low_log, 1usize << q_high_log);
        for (k0, low, high) in block_values {
            for (r, value) in low.into_iter().enumerate() {
                low_values[j / low_stride + q_low * (k0 + r)] = value;
            }
            for (r, value) in high.into_iter().enumerate() {
                high_values[j / high_stride + q_high * (k0 + r)] = value;
            }
        }
    }
    phases.mark("constraint evaluation");
    // Interpolate each group on shift * <w_{qn}> and split into chunks of degree < n.
    let mut quotient_polys: Vec<Vec<Fp>> = Vec::new();
    for (values, q_log, chunks) in [
        (&low_values, q_low_log, low_chunks),
        (&high_values, q_high_log, high_chunks),
    ] {
        if chunks == 0 {
            continue;
        }
        let q_domain = Domain::new(log_n + q_log);
        let coefficients: Vec<Vec<Fp>> = (0..4)
            .into_par_iter()
            .map(|c| {
                let mut column: Vec<Fp> = values.iter().map(|v| v.0[c]).collect();
                q_domain.inverse_on_coset(&mut column, shift);
                column
            })
            .collect();
        if coefficients
            .iter()
            .any(|column| column[chunks * n..].iter().any(|v| !v.is_zero()))
        {
            return Err(ProveError::Unsatisfied);
        }
        for chunk in 0..chunks {
            for column in &coefficients {
                quotient_polys.push(column[chunk * n..(chunk + 1) * n].to_vec());
            }
        }
    }
    let chunks = low_chunks + high_chunks;
    let quotient = commit_columns(&quotient_polys, &domain, &powers);
    transcript.absorb_digest(b"quotient-root", &quotient.tree.root());
    phases.mark("quotient commit");

    // Out-of-domain evaluations.
    let z = transcript.challenge_fp4(b"ood-z");
    let omega = domain.generator();
    let wz = z.mul_base(omega);
    let lde_size = (n * blowup) as u64;
    if (z.pow(n as u64) - Fp4::ONE).is_zero()
        || (z.pow(lde_size) - Fp4::from_base(shift.pow(lde_size))).is_zero()
        || (wz.pow(lde_size) - Fp4::from_base(shift.pow(lde_size))).is_zero()
    {
        return Err(ProveError::BadSample);
    }
    let weights_z = barycentric_weights(&domain, z);
    let weights_wz = barycentric_weights(&domain, wz);
    let ood_main: Vec<Fp4> = main_trace
        .columns
        .par_iter()
        .map(|column| evaluate_with_weights(column, &weights_z))
        .collect();
    let next_columns = circuit.next_columns();
    let ood_main_next: Vec<Fp4> = next_columns
        .iter()
        .map(|c| evaluate_with_weights(&main_trace.columns[*c as usize], &weights_wz))
        .collect();
    let ext_eval = |base: &[Vec<Fp>], weights: &[Fp4], index: usize| -> Fp4 {
        (0..4).fold(Fp4::ZERO, |acc, c| {
            acc + basis(c) * evaluate_with_weights(&base[4 * index + c], weights)
        })
    };
    let ood_aux: Vec<Fp4> = (0..aux_columns_ext.len())
        .into_par_iter()
        .map(|a| ext_eval(&aux_base, &weights_z, a))
        .collect();
    let ood_aux_next: Vec<Fp4> = if has_lookups {
        vec![ext_eval(&aux_base, &weights_wz, aux_columns_ext.len() - 1)]
    } else {
        Vec::new()
    };
    let ood_quotient: Vec<Fp4> = (0..chunks)
        .map(|chunk| {
            (0..4).fold(Fp4::ZERO, |acc, c| {
                acc + basis(c) * evaluate_coefficients(&quotient_polys[4 * chunk + c], z)
            })
        })
        .collect();
    for (label, values) in [
        (&b"ood-main"[..], &ood_main),
        (b"ood-main-next", &ood_main_next),
        (b"ood-aux", &ood_aux),
        (b"ood-aux-next", &ood_aux_next),
        (b"ood-quotient", &ood_quotient),
    ] {
        transcript.absorb_fp4s(label, values);
    }

    phases.mark("out-of-domain evaluations");
    // DEEP composition polynomial F, built in coefficient form then evaluated on the LDE domain.
    let gamma = transcript.challenge_fp4(b"deep-gamma");
    let deep = DeepLayout {
        main: main_coefficients.len(),
        aux_ext: aux_columns_ext.len(),
        quotient_ext: chunks,
        next_main: next_columns.clone(),
        next_aux: has_lookups,
    };
    let (gamma_z, gamma_wz) = deep.coefficients(gamma);
    // (base polynomial, multiplier) pairs.
    let mut z_terms: Vec<(&[Fp], Fp4)> = Vec::new();
    for (c, coeffs) in main_coefficients.iter().enumerate() {
        z_terms.push((coeffs, gamma_z[c]));
    }
    for a in 0..aux_columns_ext.len() {
        for c in 0..4 {
            z_terms.push((
                &aux_coefficients[4 * a + c],
                gamma_z[deep.main + a] * basis(c),
            ));
        }
    }
    for chunk in 0..chunks {
        for c in 0..4 {
            z_terms.push((
                &quotient_polys[4 * chunk + c],
                gamma_z[deep.main + deep.aux_ext + chunk] * basis(c),
            ));
        }
    }
    let mut wz_terms: Vec<(&[Fp], Fp4)> = Vec::new();
    for (i, c) in next_columns.iter().enumerate() {
        wz_terms.push((&main_coefficients[*c as usize], gamma_wz[i]));
    }
    if has_lookups {
        let last = aux_columns_ext.len() - 1;
        for c in 0..4 {
            wz_terms.push((
                &aux_coefficients[4 * last + c],
                gamma_wz[next_columns.len()] * basis(c),
            ));
        }
    }
    let combine_coefficients = |terms: &[(&[Fp], Fp4)]| -> Vec<Fp4> {
        (0..n)
            .into_par_iter()
            .with_min_len(256)
            .map(|k| {
                let mut acc = [Fp::ZERO; 4];
                for (coeffs, m) in terms {
                    let v = coeffs[k];
                    acc[0] += m.0[0] * v;
                    acc[1] += m.0[1] * v;
                    acc[2] += m.0[2] * v;
                    acc[3] += m.0[3] * v;
                }
                Fp4(acc)
            })
            .collect()
    };
    let g_z = combine_coefficients(&z_terms);
    let g_wz = combine_coefficients(&wz_terms);
    let claim_z = ood_main
        .iter()
        .chain(&ood_aux)
        .chain(&ood_quotient)
        .zip(&gamma_z)
        .fold(Fp4::ZERO, |acc, (v, g)| acc + *v * *g);
    let claim_wz = ood_main_next
        .iter()
        .chain(&ood_aux_next)
        .zip(&gamma_wz)
        .fold(Fp4::ZERO, |acc, (v, g)| acc + *v * *g);
    let extend = |poly: &[Fp4]| -> Vec<Fp4> {
        let components: Vec<Vec<Vec<Fp>>> = (0..4)
            .into_par_iter()
            .map(|c| {
                let coeffs: Vec<Fp> = poly.iter().map(|v| v.0[c]).collect();
                domain.low_degree_extend(&coeffs, blowup, shift)
            })
            .collect();
        let mut out = vec![Fp4::ZERO; n * blowup];
        for j in 0..blowup {
            for k in 0..n {
                out[j + blowup * k] = Fp4([
                    components[0][j][k],
                    components[1][j][k],
                    components[2][j][k],
                    components[3][j][k],
                ]);
            }
        }
        out
    };
    let g_z_lde = extend(&g_z);
    let g_wz_lde = extend(&g_wz);
    let lde_points: Vec<Fp> = {
        let mut points = Vec::with_capacity(n * blowup);
        let mut x = shift;
        for _ in 0..n * blowup {
            points.push(x);
            x *= v;
        }
        points
    };
    let mut inv_z: Vec<Fp4> = lde_points.iter().map(|x| Fp4::from_base(*x) - z).collect();
    let mut inv_wz: Vec<Fp4> = lde_points.iter().map(|x| Fp4::from_base(*x) - wz).collect();
    inv_z.par_chunks_mut(4096).for_each(batch_inverse_ext);
    inv_wz.par_chunks_mut(4096).for_each(batch_inverse_ext);
    let deep_values: Vec<Fp4> = (0..n * blowup)
        .into_par_iter()
        .map(|i| (g_z_lde[i] - claim_z) * inv_z[i] + (g_wz_lde[i] - claim_wz) * inv_wz[i])
        .collect();

    phases.mark("DEEP composition");
    let fri = FriCommitment::commit(deep_values, shift, log_n, params, &mut transcript);
    phases.mark("FRI commit");
    let pow_nonce = transcript.grind(params.grinding_bits);
    phases.mark("grinding");
    let queries =
        transcript.challenge_unique_indices(b"queries", params.queries, log_n + params.log_blowup);
    let mut sorted = queries.clone();
    sorted.sort_unstable();
    let fri_proof = fri.open(&queries);
    let main_rows = sorted.iter().map(|&i| main.row(i, n, blowup)).collect();
    let (aux_rows, aux_siblings) = match &aux {
        Some(aux) => (
            sorted.iter().map(|&i| aux.row(i, n, blowup)).collect(),
            aux.tree.open_batch(&sorted),
        ),
        None => (Vec::new(), Vec::new()),
    };
    phases.mark("openings");
    Ok(Proof {
        main_root: main.tree.root(),
        multiplicities: main_trace.multiplicities,
        aux_root: aux.as_ref().map(|a| a.tree.root()),
        quotient_root: quotient.tree.root(),
        ood_main,
        ood_main_next,
        ood_aux,
        ood_aux_next,
        ood_quotient,
        fri: fri_proof,
        pow_nonce,
        main_rows,
        main_siblings: main.tree.open_batch(&sorted),
        aux_rows,
        aux_siblings,
        quotient_rows: sorted.iter().map(|&i| quotient.row(i, n, blowup)).collect(),
        quotient_siblings: quotient.tree.open_batch(&sorted),
    })
}

/// Checks every identity on every row of an honest trace (diagnostics; returns the first failure).
///
/// # Errors
/// Returns the label and row of the first violated circuit identity.
pub fn check_trace(
    circuit: &Circuit,
    statement: &Statement,
    public_inputs: &[Vec<i64>],
    states: &[Vec<i64>],
) -> Result<(), String> {
    let public = trace::public_columns(
        circuit,
        statement.steps,
        statement.checkpoint_step(),
        public_inputs,
    )
    .map_err(|e| e.to_string())?;
    let main =
        trace::generate(circuit, &public, statement.steps, states).map_err(|e| e.to_string())?;
    check_main_trace(circuit, statement, &public, &main)
}

/// Checks every circuit and boundary identity of a main trace, returning the first failure.
pub(crate) fn check_main_trace(
    circuit: &Circuit,
    statement: &Statement,
    public: &[Vec<i64>],
    main: &trace::MainTrace,
) -> Result<(), String> {
    let air = CompiledAir::new(
        circuit,
        &statement.initial,
        &statement.final_state,
        &statement.checkpoint_registers(circuit.registers.len()),
    );
    let n = circuit.rows();
    let layout = air.layout();
    for k in 0..n {
        let mut row = vec![Fp::ZERO; layout.width()];
        for (c, column) in main.columns.iter().enumerate() {
            row[c] = column[k];
            row[layout.row() + c] = column[(k + 1) % n];
        }
        for (p, column) in public.iter().enumerate() {
            row[layout.main + p] = Fp::from_i64(column[k]);
            row[layout.row() + layout.main + p] = Fp::from_i64(column[(k + 1) % n]);
        }
        for index in 0..air.base_constraint_count() {
            if air.is_transition(index) && k == n - 1 {
                continue;
            }
            if !air.eval_constraint(index, &row).is_zero() {
                let label = circuit
                    .constraints
                    .get(index)
                    .map_or_else(|| format!("boundary identity {index}"), |c| c.label.clone());
                return Err(format!("row {k}: {label}"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod hash_binding_tests {
    use super::*;

    #[test]
    fn public_digest_binds_values_and_column_boundaries() {
        let public = vec![vec![1, 2], vec![3, 4]];
        let digest = public_digest(&public);
        assert_eq!(digest.len(), DIGEST_BYTES);
        for changed in [
            vec![vec![1, 2], vec![3, 5]],
            vec![vec![1], vec![2, 3, 4]],
            vec![vec![3, 4], vec![1, 2]],
            vec![vec![1, 2], vec![3, 4], vec![]],
        ] {
            assert_ne!(public_digest(&changed), digest);
        }
    }

    #[test]
    fn circuit_digest_binds_name_and_register_bounds() {
        let mut builder = crate::Builder::new("hash binding", 8);
        let value = builder.register("value", 0, 9);
        builder.set_next(&value, &value);
        let circuit = builder.finish();
        let digest = circuit_id(&circuit);
        assert_eq!(digest.len(), DIGEST_BYTES);
        let mut named = circuit.clone();
        named.name.push('x');
        assert_ne!(circuit_id(&named), digest);
        let mut bounded = circuit;
        bounded.registers[0].hi += 1;
        assert_ne!(circuit_id(&bounded), digest);
    }
}

#[cfg(test)]
mod timing_tests {
    use super::*;

    #[test]
    fn missing_cpu_samples_are_unavailable_without_elapsed_subtraction() {
        for (previous, current) in [(None, None), (Some(1.0), None), (None, Some(2.0))] {
            assert_eq!(cpu_elapsed_label(previous, current), "unavailable");
        }
        assert_eq!(cpu_elapsed_label(Some(2.0), Some(1.0)), "unavailable");
    }

    #[test]
    fn measured_cpu_zero_and_elapsed_time_remain_distinct_from_unavailable() {
        assert_eq!(cpu_elapsed_label(Some(1.0), Some(1.0)), "  0.000 s");
        assert_eq!(cpu_elapsed_label(Some(1.25), Some(2.75)), "  1.500 s");
    }

    #[test]
    fn cpu_sample_reports_native_platform_availability() {
        #[cfg(unix)]
        assert!(process_cpu_seconds().is_some_and(|seconds| seconds.is_finite() && seconds >= 0.0));
        #[cfg(not(unix))]
        assert_eq!(process_cpu_seconds(), None);
    }
}
