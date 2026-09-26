//! Full fixed-profile producer for the bounded DEEP verifier.
//!
//! Commitments precede their challenges; the quotient uses complete polynomial
//! evaluation and exact division, and the DEEP polynomial is formed before its
//! LDE. Every opening comes from the retained committed arrays and minimal tree
//! frontiers. Private arrays use fixed allocations erased on drop.
//! TODO: Qualify witness hiding, concrete hash security, execution quotas and
//! hardware/resource performance before production admission.

use fastpq_isi::GoldilocksDigest384V1 as Digest;
use iroha_data_model::privacy::GoldilocksDigest384V1 as WireDigest;
use rayon::prelude::*;
#[cfg(test)]
use std::time::Instant;

use super::{
    compact_public_columns::COMMITTED_COLUMN_COUNT,
    compact_v1::{MAX_PREPARED_HASH_FRAME_BYTES, PreparedHashFrame},
    deep_binding::{BindingError, Context, Message, Oracle, Transcript},
    deep_composition::OodPair,
    deep_geometry::{CONSTRAINTS, DeepGeometry, FRI_ARITIES, FRI_LENGTHS, LDE_ROWS, TRACE_ROWS},
    deep_polynomial::DeepPolynomialSource,
    deep_proof::{
        self, DeepProof, FriGroup, FriRound, FriValues, OodAnswers, OpeningPlans, QuotientOpening,
        RowOpening, RowValues,
    },
    deep_quotient::{DeepQuotientLimits, PreparedDeepTrace},
    deep_relation::DeepRelation,
    fri_fold::FriFoldPlan,
    merkle_multiproof::MultiproofPlan,
    mul_mod,
    polynomial_field::PolynomialField,
    polynomial_transform::PolynomialDomain,
    secret_polynomial::SecretPolynomial,
};
use crate::digest384_batch::{
    Digest384LastFieldJob, execute_last_fields_with_cpu, last_fields_payload_charge,
};
use crate::{DigestExecutionV1, Error, Result, cyclotomic, field::GoldilocksFp4V1 as F};
use zeroize::Zeroizing;

// Bounded per-tree preparation; exactly the same charge on every backend.
const HASH_BATCH_FRAMES: usize = 256;

// Test qualification reports phase names and elapsed time only. The normal
// producer has no stderr side effect or environment-based diagnostic switch.
#[cfg(test)]
struct PhaseTimer {
    name: &'static str,
    round: Option<usize>,
    started: Instant,
}

#[cfg(test)]
impl PhaseTimer {
    fn start(name: &'static str, round: Option<usize>) -> Self {
        let timer = Self {
            name,
            round,
            started: Instant::now(),
        };
        timer.report("start");
        timer
    }

    fn report(&self, event: &str) {
        use std::io::Write;
        // Logging must not cause a second panic while a phase unwinds.
        let _ = writeln!(
            std::io::stderr(),
            "deep_prover_phase={} round={:?} event={} elapsed_ms={}",
            self.name,
            self.round,
            event,
            self.started.elapsed().as_millis()
        );
    }
}

#[cfg(test)]
impl Drop for PhaseTimer {
    fn drop(&mut self) {
        self.report("end");
    }
}

#[cfg(not(test))]
struct PhaseTimer;

#[cfg(not(test))]
impl PhaseTimer {
    fn start(_name: &'static str, _round: Option<usize>) -> Self {
        Self
    }
}

#[cfg(not(test))]
impl Drop for PhaseTimer {
    fn drop(&mut self) {}
}

/// Caller-owned local prover ceilings; these do not grant ledger admission.
#[derive(Clone, Copy, Debug)]
pub(super) struct ProverLimits {
    /// Explicit local commitment executor, independent of protocol identity.
    pub(super) digest_execution: DigestExecutionV1,
    /// Conservative live and temporary array payload charge, not process RSS.
    pub(super) max_payload_bytes: usize,
    /// Existing exact quotient arithmetic and inspection-work ceiling.
    pub(super) quotient: DeepQuotientLimits,
    /// Complete canonical proof-frame ceiling.
    pub(super) max_proof_bytes: usize,
}

/// Prove a caller-prepared fixed transfer AIR from its 301 base coefficients.
///
/// Coefficient slices may be short, including empty zero columns; their exact
/// degree must be below N. Public columns are always derived independently.
pub(super) fn prove(
    relation: &impl DeepRelation,
    coefficients: &[&[u64]],
    limits: ProverLimits,
) -> Result<Vec<u8>> {
    let geometry = DeepGeometry::new()?;
    let binding = preflight(relation, coefficients, limits)?;
    let phase = PhaseTimer::start("coefficient_preparation", None);
    let prepared_trace = PreparedDeepTrace::prepare(coefficients, limits.quotient)?;
    let quotient_payload = prepared_trace
        .plan(relation.deep_relation(), limits.quotient)?
        .payload_bytes();
    limit(
        "max_deep_prover_payload_bytes",
        payload_charge(quotient_payload)?,
        limits.max_payload_bytes,
    )?;
    let transform = PolynomialDomain::for_deep(&geometry, limits.max_payload_bytes)?;
    drop(phase);

    let phase = PhaseTimer::start("base_lde", None);
    let columns = coefficients
        .par_iter()
        .map(|values| base_lde(&geometry, values))
        .collect::<Result<Vec<_>>>()?;
    drop(phase);
    let phase = PhaseTimer::start("row_tree", None);
    let row_tree = Tree::build(
        &binding,
        Oracle::Row,
        LDE_ROWS,
        limits.digest_execution,
        |index| {
            let mut bytes = Zeroizing::new([0; COMMITTED_COLUMN_COUNT * 8]);
            for (slot, column) in columns.iter().enumerate() {
                bytes[slot * 8..slot * 8 + 8].copy_from_slice(&column[index].to_le_bytes());
            }
            binding.prepare_leaf(Oracle::Row, index as u32, &bytes[..])
        },
    )?;
    drop(phase);
    let mut transcript = Transcript::new(binding.clone());
    if transcript.challenge().map_err(binding_error)? != Message::Dummy {
        return Err(invalid("DEEP producer requires the initial dummy message"));
    }
    transcript
        .commit_root(Oracle::Row, row_tree.root())
        .map_err(binding_error)?;
    let alphas = fields(&mut transcript, CONSTRAINTS)?;
    let phase = PhaseTimer::start("quotient_build", None);
    let quotient = prepared_trace.build(relation.deep_relation(), &alphas, limits.quotient)?;
    drop(prepared_trace);
    drop(phase);
    let phase = PhaseTimer::start("quotient_lde", None);
    let source = DeepPolynomialSource::new(coefficients, &quotient)?;
    let halves = source.quotient_halves();
    let low = transform.evaluate(halves[0], halves[0].len())?;
    let high = transform.evaluate(halves[1], halves[1].len())?;
    drop(phase);
    let phase = PhaseTimer::start("quotient_tree", None);
    let quotient_tree = Tree::build(
        &binding,
        Oracle::QuotientPair,
        LDE_ROWS,
        limits.digest_execution,
        |index| {
            let mut bytes = Zeroizing::new([0; 64]);
            bytes[..32].copy_from_slice(&low.value(index)?.to_le_bytes());
            bytes[32..].copy_from_slice(&high.value(index)?.to_le_bytes());
            binding.prepare_leaf(Oracle::QuotientPair, index as u32, &bytes[..])
        },
    )?;
    drop(phase);
    transcript
        .commit_root(Oracle::QuotientPair, quotient_tree.root())
        .map_err(binding_error)?;
    let phase = PhaseTimer::start("ood_composition", None);
    let z = fields(&mut transcript, 1)?[0];
    let prepared = source.prepare(OodPair::new(z, geometry.trace_generator())?);
    let answers = prepared.trace_answers();
    let ood = OodAnswers {
        current: answers[0].to_vec(),
        next: answers[1].to_vec(),
        quotient: prepared.quotient_answers().to_vec(),
    };
    geometry.check_ood(
        relation.deep_relation(),
        &alphas,
        z,
        &ood.current,
        &ood.next,
        &ood.quotient,
    )?;
    transcript
        .commit_ood(&ood.current, &ood.next, &ood.quotient)
        .map_err(binding_error)?;
    let lambda = fields(&mut transcript, 1)?[0];
    let polynomial = prepared.compose(lambda, limits.max_payload_bytes)?;
    drop(phase);
    let phase = PhaseTimer::start("composition_lde", None);
    let first = transform.evaluate(polynomial.coefficients(), TRACE_ROWS)?;
    let mut initial = SecretPolynomial::zeroed(LDE_ROWS)?;
    for (index, value) in initial.iter_mut().enumerate() {
        *value = first.value(index)?;
    }
    drop(first);
    drop(phase);
    let mut layers = vec![initial];
    let mut trees = Vec::with_capacity(5);
    let mut domain = geometry.domain();
    for (round, &arity) in FRI_ARITIES.iter().enumerate() {
        let values = &layers[round];
        let groups = FRI_LENGTHS[round + 1];
        let oracle = Oracle::Fri(round as u8);
        let phase = PhaseTimer::start("fri_tree", Some(round));
        let tree = Tree::build(&binding, oracle, groups, limits.digest_execution, |index| {
            let mut bytes = Zeroizing::new([0_u8; 16 * F::BYTES]);
            for coordinate in 0..arity {
                bytes[coordinate * F::BYTES..(coordinate + 1) * F::BYTES]
                    .copy_from_slice(&values[index + coordinate * groups].to_le_bytes());
            }
            binding.prepare_leaf(oracle, index as u32, &bytes[..arity * F::BYTES])
        })?;
        drop(phase);
        transcript
            .commit_root(oracle, tree.root())
            .map_err(binding_error)?;
        let beta = fields(&mut transcript, 1)?[0];
        let phase = PhaseTimer::start("fri_fold", Some(round));
        let fold = FriFoldPlan::new(arity, domain.coset_generator(groups))?;
        let mut next = SecretPolynomial::zeroed(groups)?;
        fold.fold_layer_into(values, beta, domain, &mut next)?;
        domain = domain.folded(arity);
        trees.push(tree);
        layers.push(next);
        drop(phase);
    }
    let phase = PhaseTimer::start("terminal_tree", None);
    let terminal = layers[5].to_vec();
    if terminal.iter().any(|value| *value != terminal[0]) {
        return Err(invalid("DEEP producer terminal is not constant"));
    }
    let terminal_bytes: Vec<_> = terminal
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let terminal_tree = Tree::build(
        &binding,
        Oracle::Terminal,
        1,
        limits.digest_execution,
        |_| binding.prepare_leaf(Oracle::Terminal, 0, &terminal_bytes),
    )?;
    drop(phase);
    transcript
        .commit_root(Oracle::Terminal, terminal_tree.root())
        .map_err(binding_error)?;
    let phase = PhaseTimer::start("wire_assembly", None);
    let Message::Queries(queries) = transcript.challenge().map_err(binding_error)? else {
        return Err(invalid(
            "DEEP producer requires its final whole query message",
        ));
    };
    let queries: Vec<_> = queries.into_iter().map(|index| index as usize).collect();
    let plans = OpeningPlans::new(&queries)?;
    let rows = queries
        .iter()
        .map(|&index| {
            Ok(RowOpening {
                index: index as u32,
                values: RowValues::new(columns.iter().map(|column| column[index]).collect())?,
            })
        })
        .collect::<Result<_>>()?;
    let quotients = queries
        .iter()
        .map(|&index| {
            Ok(QuotientOpening {
                index: index as u32,
                low: low.value(index)?,
                high: high.value(index)?,
            })
        })
        .collect::<Result<_>>()?;
    let rounds = plans
        .round_indices
        .iter()
        .enumerate()
        .map(|(round, indices)| {
            let groups = indices
                .iter()
                .map(|&index| {
                    Ok(FriGroup {
                        index: index as u32,
                        values: FriValues::new(
                            (0..FRI_ARITIES[round])
                                .map(|coordinate| {
                                    layers[round][index + coordinate * FRI_LENGTHS[round + 1]]
                                })
                                .collect(),
                        )?,
                    })
                })
                .collect::<Result<_>>()?;
            Ok(FriRound {
                groups,
                siblings: trees[round].frontier(&plans.rounds[round])?,
            })
        })
        .collect::<Result<_>>()?;
    let mut fri_roots: Vec<_> = trees
        .iter()
        .map(|tree| WireDigest::from(tree.root()))
        .collect();
    fri_roots.push(WireDigest::from(terminal_tree.root()));
    let proof = DeepProof {
        row_root: WireDigest::from(row_tree.root()),
        quotient_root: WireDigest::from(quotient_tree.root()),
        fri_roots,
        ood,
        rows,
        quotients,
        row_siblings: row_tree.frontier(&plans.initial)?,
        quotient_siblings: quotient_tree.frontier(&plans.initial)?,
        rounds,
        terminal,
    };
    deep_proof::preflight(&proof, &queries)?;
    let bytes = norito::encode_canonical(&proof)?;
    limit("max_deep_proof_bytes", bytes.len(), limits.max_proof_bytes)?;
    limit(
        "max_deep_frame_bytes",
        bytes.len(),
        deep_proof::MAX_FRAME_BYTES,
    )?;
    drop(phase);
    Ok(bytes)
}

fn preflight(
    relation: &impl DeepRelation,
    coefficients: &[&[u64]],
    limits: ProverLimits,
) -> Result<Context> {
    limit(
        "max_deep_proof_bytes",
        deep_proof::MAX_FRAME_BYTES,
        limits.max_proof_bytes,
    )?;
    limit(
        "max_deep_prover_payload_bytes",
        payload_charge(0)?,
        limits.max_payload_bytes,
    )?;
    if coefficients.len() != COMMITTED_COLUMN_COUNT
        || coefficients.iter().any(|column| column.len() > TRACE_ROWS)
    {
        return Err(invalid(
            "DEEP prover needs exactly 301 degree-below-N coefficient columns",
        ));
    }
    for (column, values) in coefficients.iter().enumerate() {
        for (degree, &value) in values.iter().enumerate() {
            value.validate("deep_prover_coefficients", &[column, degree])?;
        }
    }
    Context::for_relation(relation).map_err(binding_error)
}

// Conservative array charges sum lifetimes instead of discounting overlap.
// Includes input coefficients, full base LDE, all trees and fallible digest
// collection slots, quotient/DEEP lane matrices and retained folding layers.
// Allocator metadata, thread stacks and unrelated process state are excluded.
pub(super) fn payload_charge(quotient_payload: usize) -> Result<usize> {
    let terms = [
        COMMITTED_COLUMN_COUNT * TRACE_ROWS * 8,
        COMMITTED_COLUMN_COUNT * LDE_ROWS * 8,
        8 * LDE_ROWS * core::mem::size_of::<Digest>(),
        LDE_ROWS * core::mem::size_of::<Result<Digest>>(),
        8 * LDE_ROWS * F::BYTES,
        8 * deep_proof::MAX_FRAME_BYTES,
        8 * 256 * 1024,
        quotient_payload,
        hash_batch_payload_charge()?,
    ];
    terms.into_iter().try_fold(0usize, |sum, term| {
        sum.checked_add(term)
            .ok_or_else(|| invalid("DEEP prover payload charge overflow"))
    })
}

// Includes at most one batch of caller raw rows, fixed encoded bodies,
// preparation result slots, retained records, borrowed jobs and executor pages.
// The backend charge additionally includes returned digest slots and readiness.
fn hash_batch_payload_charge() -> Result<usize> {
    let body_bytes = HASH_BATCH_FRAMES
        .checked_mul(MAX_PREPARED_HASH_FRAME_BYTES)
        .ok_or_else(|| invalid("DEEP hash preparation charge overflow"))?;
    let records = HASH_BATCH_FRAMES
        .checked_mul(
            core::mem::size_of::<Result<PreparedHashFrame>>()
                + core::mem::size_of::<PreparedHashFrame>()
                + core::mem::size_of::<Digest384LastFieldJob<'_>>()
                + 128 * F::BYTES,
        )
        .ok_or_else(|| invalid("DEEP hash preparation charge overflow"))?;
    let backend = last_fields_payload_charge(HASH_BATCH_FRAMES, body_bytes)?;
    body_bytes
        .checked_add(records)
        .and_then(|sum| sum.checked_add(backend))
        .ok_or_else(|| invalid("DEEP hash preparation charge overflow"))
}

fn execute_prepared_frames(
    frames: &[PreparedHashFrame],
    execution: DigestExecutionV1,
) -> Result<Vec<Digest>> {
    if frames.len() > HASH_BATCH_FRAMES {
        return Err(invalid(
            "DEEP hash preparation exceeds its fixed batch count",
        ));
    }
    let bytes = frames.iter().try_fold(0usize, |sum, frame| {
        sum.checked_add(frame.payload_len())
            .ok_or_else(|| invalid("DEEP prepared hash payload overflow"))
    })?;
    let digests = execute_last_fields_with_cpu(
        frames.len(),
        bytes,
        execution,
        |index| frames[index].hash_cpu(),
        || frames.iter().map(PreparedHashFrame::job).collect(),
    )?;
    if digests.len() != frames.len() {
        return Err(invalid("DEEP hash executor returned another digest count"));
    }
    Ok(digests)
}

fn base_lde(geometry: &DeepGeometry, coefficients: &[u64]) -> Result<SecretPolynomial<u64>> {
    let domain = PolynomialDomain::for_deep(geometry, 2 * LDE_ROWS * F::BYTES)?;
    base_lde_on_domain(domain, coefficients)
}

// The fixed DEEP wrapper and small-domain checks share this exact u64
// twist/FFT kernel. PolynomialDomain owns the root/order checks; a base-only
// offset is required before allocating or interpreting its first coordinate.
fn base_lde_on_domain(
    domain: PolynomialDomain,
    coefficients: &[u64],
) -> Result<SecretPolynomial<u64>> {
    if coefficients.len() > TRACE_ROWS || coefficients.len() > domain.rows() {
        return Err(invalid("DEEP base polynomial exceeds its degree bound"));
    }
    for (index, &value) in coefficients.iter().enumerate() {
        value.validate("deep_prover_base_lde", &[index])?;
    }
    let offset = domain.point(0)?.coefficients();
    if offset[1..].iter().any(|&coordinate| coordinate != 0) {
        return Err(invalid("DEEP base polynomial requires a base-field coset"));
    }
    let mut values = SecretPolynomial::zeroed(domain.rows())?;
    let mut twist = 1;
    for (destination, &coefficient) in values.iter_mut().zip(coefficients) {
        *destination = mul_mod(coefficient, twist);
        twist = mul_mod(twist, offset[0]);
    }
    cyclotomic::fft(
        &mut values,
        cyclotomic::Domain {
            log_size: domain.rows().ilog2(),
            generator: domain.generator(),
        },
    );
    Ok(values)
}

struct Tree {
    levels: Vec<Vec<Digest>>,
    binding: Context,
    oracle: Oracle,
}

impl Tree {
    fn build(
        binding: &Context,
        oracle: Oracle,
        leaves: usize,
        execution: DigestExecutionV1,
        leaf: impl Fn(usize) -> Result<PreparedHashFrame> + Sync,
    ) -> Result<Self> {
        let expected = match oracle {
            Oracle::Row | Oracle::QuotientPair => LDE_ROWS,
            Oracle::Fri(round @ 0..=4) => FRI_LENGTHS[round as usize + 1],
            Oracle::Terminal => 1,
            Oracle::Fri(_) => return Err(invalid("DEEP producer tree has an invalid round")),
        };
        if leaves != expected {
            return Err(invalid("DEEP producer tree has another oracle geometry"));
        }
        let mut first = super::polynomial_transform::reserved(leaves.max(2))?;
        for start in (0..leaves).step_by(HASH_BATCH_FRAMES) {
            let end = (start + HASH_BATCH_FRAMES).min(leaves);
            let frames = (start..end)
                .into_par_iter()
                .map(&leaf)
                .collect::<Result<Vec<_>>>()?;
            first.extend(execute_prepared_frames(&frames, execution)?);
        }
        if leaves == 1 {
            first.push(first[0]);
        }
        let mut levels = vec![first];
        while levels.last().expect("tree has leaves").len() > 1 {
            let level = levels.len();
            let children = levels.last().unwrap();
            let mut next = super::polynomial_transform::reserved(children.len() / 2)?;
            for (chunk, pairs) in children.chunks(HASH_BATCH_FRAMES * 2).enumerate() {
                let frames = pairs
                    .par_chunks_exact(2)
                    .enumerate()
                    .map(|(local, pair)| {
                        binding.prepare_parent(
                            oracle,
                            level as u32,
                            (chunk * HASH_BATCH_FRAMES + local) as u32,
                            pair[0],
                            pair[1],
                        )
                    })
                    .collect::<Result<Vec<_>>>()?;
                next.extend(execute_prepared_frames(&frames, execution)?);
            }
            levels.push(next);
        }
        Ok(Self {
            levels,
            binding: binding.clone(),
            oracle,
        })
    }

    fn root(&self) -> Digest {
        self.levels.last().expect("tree has root")[0]
    }

    fn frontier(&self, plan: &MultiproofPlan) -> Result<Vec<WireDigest>> {
        plan.open_with(&self.levels, |level, index, left, right| {
            self.binding
                .hash_parent(self.oracle, level as u32, index as u32, left, right)
                .map_err(binding_error)
        })
        .map(|siblings| siblings.into_iter().map(WireDigest::from).collect())
    }
}

fn fields(transcript: &mut Transcript, expected: usize) -> Result<Vec<F>> {
    match transcript.challenge().map_err(binding_error)? {
        Message::Fields(values) if values.len() == expected => Ok(values),
        _ => Err(invalid("DEEP producer challenge has another dimension")),
    }
}

fn limit(name: &'static str, actual: usize, maximum: usize) -> Result<()> {
    if actual > maximum {
        return Err(Error::VerifierLimitExceeded {
            limit: name,
            actual,
            max: maximum,
        });
    }
    Ok(())
}

fn binding_error(error: BindingError) -> Error {
    Error::InvalidTraceShape {
        details: format!("DEEP producer binding: {error}"),
    }
}
fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
#[path = "deep_prover/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "deep_prover/base_lde_tests.rs"]
mod base_lde_tests;
