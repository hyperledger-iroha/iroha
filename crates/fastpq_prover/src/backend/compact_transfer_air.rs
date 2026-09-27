//! Private complete one-delta compact hash/SMT relation and prover acceleration.
//!
//! The fixed schema is 65,536 rows, 342 committed cells and 923 independently
//! mixed constraints: hash locals, hash transitions, then SMT semantic slots.
//! Exact full-width public leaves, paths and roots are Norito-bound before any
//! protocol challenge. Optional caller context is bound as opaque bytes inside
//! that canonical envelope; this helper neither interprets nor authenticates it.
//! The caller must establish the public transfer arithmetic, identities, native
//! leaf hashes and collision-resolved paths, and authenticate external roots.
//!
//! The complete arithmetic evaluator accepts canonical base or quartic-extension
//! points and cells, preserving all 923 slots. The existing compact proof still
//! uses its fixed base-field openings and unchanged transcript/profile.
//! Verification evaluates only the two complete openings and bounded public
//! polynomials. It never constructs a witness, FFT or LDE. Prover preparation
//! expands exactly 49 fixed public columns once and shares those immutable LDEs
//! plus the periodic phase/hash masks across worker-local arithmetic scratch.
//! Under the existing unmasked column bound <N, hash numerators have degree <3N
//! and SMT numerators degree <2N. Exact division of a vanishing combined numerator
//! then yields quotient degree <2N. Explicit masked column degrees require the
//! separate full-polynomial degree calculation and new degree authentication.
//!
//! TODO: Qualify the complete protocol's soundness and public resource envelope
//! before changing production admission. This internal prototype proves one
//! declared two-update SMT statement; it changes no query/profile/default limit
//! and does not replace the production verifier's mandatory replay.

use super::{
    air_degree::{AirDegreeBounds, PolynomialDegree},
    compact_hash_quotient::PolynomialHashEvaluator,
    masked_quotient::{checked_add, checked_mul, transform_work},
    polynomial_transform::{PolynomialDomain, PolynomialLanes, reserved},
    secret_polynomial::SecretPolynomial,
};
use crate::field::GoldilocksFp4V1;
use fastpq_isi::FASTPQ_FINAL_V1;
use norito::{NoritoSerialize, codec::Encode as NoritoEncode};

use super::{
    compact_hash_quotient::{CompactHashQuotient, HashNumerators, LOCAL_SLOTS, TRANSITION_SLOTS},
    compact_protocol::{FixedAir, FixedAirSchema},
    compact_smt_quotient::{CompactSmtFixedColumns, CompactSmtQuotient, RESIDUE_COUNT},
    polynomial_field::PolynomialField,
};
use crate::{
    Error, Result,
    gadgets::{
        compact_smt_air::{COLUMN_COUNT, DigestLimbs, PHYSICAL_ROW_COUNT, PublicStatement},
        compact_trace_columns::smt_row_from_cells,
    },
    proof::VerifyLimits,
};

use super::compact_smt_quotient::{CompactSmtFixedValues, FIXED_COLUMN_COUNT, FIXED_ROW_COUNT};
#[cfg(test)]
use super::{
    FriDomain, GOLDILOCKS_MODULUS,
    compact_hash_quotient::ProverMaskCycle,
    compact_protocol::{PreparedAir, ProverEvaluator},
    fixed_schedule::PeriodicSelectors,
};
#[cfg(test)]
use crate::gadgets::compact_trace_columns::decode_smt_row;
use crate::{fft::Planner, gadgets::compact_smt_air::PHYSICAL_HASH_ROWS};

const CONSTRAINT_COUNT: usize = LOCAL_SLOTS + TRANSITION_SLOTS + RESIDUE_COUNT;
const IDENTITY: &str =
    "fastpq:compact:v1:compact-transfer:v1:342cols:597local+83edge+243smt:65536rows";
const MASK_CYCLE_ROWS: usize = 4096;
#[cfg(test)]
const LDE_ROWS: usize = 524_288;
#[cfg(test)]
const FIXED_LDE_BYTES: usize = 205_520_896;
#[cfg(test)]
const FIXED_COEFFICIENT_BYTES: usize = 25_690_112;
#[cfg(test)]
const PHASE_CYCLE_BYTES: usize = 16_777_216;

#[derive(NoritoSerialize, norito::NoritoSchema)]
#[norito_schema(
    name = "fastpq_prover::backend::compact_transfer_air::BoundUpdate",
    frame = "fastpq_prover::compact_v1::TransferUpdateV1"
)]
struct BoundUpdate {
    old_leaf: [u8; 32],
    new_leaf: [u8; 32],
    path: u32,
}

#[derive(NoritoSerialize, norito::NoritoSchema)]
#[norito_schema(
    name = "fastpq_prover::backend::compact_transfer_air::BoundStatement",
    frame = "fastpq_prover::compact_v1::TransferStatementV1"
)]
struct BoundStatement {
    version: u16,
    updates: [BoundUpdate; 2],
    old_root: [u8; 32],
    new_root: [u8; 32],
    caller_context: Option<Vec<u8>>,
}

fn digest_bytes(limbs: DigestLimbs) -> [u8; 32] {
    core::array::from_fn(|byte| limbs[byte / 4].to_le_bytes()[byte % 4])
}

fn bound_statement(statement: &PublicStatement, caller_context: Option<&[u8]>) -> BoundStatement {
    BoundStatement {
        version: 1,
        updates: statement.updates.map(|update| BoundUpdate {
            old_leaf: digest_bytes(update.old_leaf),
            new_leaf: digest_bytes(update.new_leaf),
            path: update.path,
        }),
        old_root: digest_bytes(statement.old_root),
        new_root: digest_bytes(statement.new_root),
        caller_context: caller_context.map(<[u8]>::to_vec),
    }
}

/// Complete fixed one-delta relation with an owned bounded public statement.
pub(super) struct CompactTransferAir {
    statement_bytes: Vec<u8>,
    fixed: CompactSmtFixedColumns,
    hash: CompactHashQuotient,
}

impl CompactTransferAir {
    /// Bind complete public ports and optional caller-authenticated context.
    ///
    /// Context is preserved byte-for-byte, including absent versus empty. The
    /// enclosing Norito layout always uses canonical codec flags. Supplying bytes
    /// establishes no account authority, permission or external-root provenance.
    pub(super) fn new(statement: &PublicStatement, caller_context: Option<&[u8]>) -> Result<Self> {
        let maximum = VerifyLimits::default().max_batch_bytes;
        check_limit(
            "max_compact_statement_bytes",
            caller_context.map_or(0, <[u8]>::len),
            maximum,
        )?;
        let fixed = CompactSmtFixedColumns::new(statement)?;
        // Validate the exact sparse schema/geometry now; evaluation below creates
        // a borrowed bounded view, avoiding a self-referential owned relation.
        CompactSmtQuotient::new(&FASTPQ_FINAL_V1, &fixed)?;
        let binding = bound_statement(statement, caller_context);
        // Encode pins canonical flags and restores the caller's ambient codec
        // state. The input preflight already bounded the only variable payload.
        let statement_bytes = binding.encode();
        check_limit(
            "max_compact_statement_bytes",
            statement_bytes.len(),
            maximum,
        )?;
        Ok(Self {
            statement_bytes,
            fixed,
            hash: CompactHashQuotient::new(&FASTPQ_FINAL_V1, PHYSICAL_ROW_COUNT)?,
        })
    }

    /// Count the exact canonical statement without constructing fixed AIR data.
    ///
    /// The optional public context is bounded before the small binding clone;
    /// this performs no witness, polynomial, FFT, LDE or proof operation.
    pub(super) fn encoded_statement_len(
        statement: &PublicStatement,
        caller_context: Option<&[u8]>,
    ) -> Result<usize> {
        let maximum = VerifyLimits::default().max_batch_bytes;
        check_limit(
            "max_compact_statement_bytes",
            caller_context.map_or(0, <[u8]>::len),
            maximum,
        )?;
        let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let bytes = norito::core::encoded_payload_len(&bound_statement(statement, caller_context))?;
        check_limit("max_compact_statement_bytes", bytes, maximum)?;
        Ok(bytes)
    }

    /// Evaluate all 923 fixed slots at the exact canonical field point.
    ///
    /// Current and next values are evaluations at x and g*x supplied by the
    /// surrounding polynomial protocol. This arithmetic authenticates neither
    /// those evaluations nor a proof. All dimensions and coefficients are
    /// checked before constructing public fixed tables or evaluating selectors.
    pub(super) fn evaluate_at<F: PolynomialField>(
        &self,
        point: F,
        current: &[F],
        next: &[F],
    ) -> Result<Vec<F>> {
        let current: &[F; COLUMN_COUNT] = current
            .try_into()
            .map_err(|_| shape("compact_smt_opening needs exactly 342 cells"))?;
        let next: &[F; COLUMN_COUNT] = next
            .try_into()
            .map_err(|_| shape("compact_smt_opening needs exactly 342 cells"))?;
        for row in [current, next] {
            for (column, &value) in row.iter().enumerate() {
                value.validate("compact_smt_opening", &[column])?;
            }
        }
        point.validate("fixed_schedule_evaluation_point", &[])?;
        let current = smt_row_from_cells(current);
        let next = smt_row_from_cells(next);
        let hash = self.hash.evaluate(point, &current.hash, &next.hash)?;
        let smt = CompactSmtQuotient::new(&FASTPQ_FINAL_V1, &self.fixed)?;
        let fixed = smt.evaluate_fixed(point)?;
        Ok(combine(hash, smt.residues(&fixed, &current, &next)))
    }

    /// Bound all full polynomial numerators under explicit per-column degrees.
    ///
    /// Bounds are exclusive and must be positive for all 342 columns. They are
    /// caller obligations, not authenticated by this arithmetic. In particular,
    /// a padded coefficient extent does not prove a tighter column degree.
    /// Rotation by the nonzero trace generator preserves these degrees. This
    /// follows the exact compiled hash and SMT slot order without evaluating a
    /// trace, selecting mask entropy or interpolating subgroup residues.
    pub(super) fn numerator_degree_bounds(&self, columns: &[usize]) -> Result<AirDegreeBounds> {
        let columns: &[usize; COLUMN_COUNT] = columns
            .try_into()
            .map_err(|_| shape("compact degree declaration needs exactly 342 columns"))?;
        if columns.contains(&0) {
            return Err(shape("compact column degree bounds must be positive"));
        }
        let columns = columns.map(PolynomialDegree::from_exclusive);
        let row = smt_row_from_cells(&columns);
        let hash = self.hash.numerator_degree_bounds(
            &crate::gadgets::compact_trace_columns::hash_row_cells(&row.hash),
        )?;
        let smt = self.fixed.numerator_degree_bounds(&columns)?;
        AirDegreeBounds::new(PHYSICAL_ROW_COUNT, combine(hash, smt))
    }

    /// Bound the actual fixed-column/cycle preparation without allocating it.
    pub(super) fn polynomial_preparation_cost(
        &self,
        domain: PolynomialDomain,
    ) -> Result<PolynomialPreparationCost> {
        domain.numerator_rotation(PHYSICAL_ROW_COUNT)?;
        let cycle = domain.rows() / (PHYSICAL_ROW_COUNT / PHYSICAL_HASH_ROWS);
        check_limit("max_compact_polynomial_phase_cycle", cycle, MASK_CYCLE_ROWS)?;
        let graph = self.hash.metrics();
        let fixed = checked_mul(FIXED_COLUMN_COUNT, domain.rows())?;
        let phases = checked_mul(cycle, PHYSICAL_HASH_ROWS)?;
        let masks = checked_mul(cycle, graph.selector_masks)?;
        // Retained fixed lanes, public phases/masks, private graph scratch, one
        // temporary public coefficient conversion and selector/mask scratch.
        let cells = checked_add(
            checked_add(fixed, phases)?,
            checked_add(masks, graph.nodes)?,
        )?;
        let transient = checked_add(
            PHYSICAL_ROW_COUNT,
            checked_add(2 * PHYSICAL_HASH_ROWS, graph.selector_masks)?,
        )?;
        let payload_bytes = checked_add(
            checked_mul(checked_add(cells, transient)?, GoldilocksFp4V1::BYTES)?,
            checked_mul(
                PHYSICAL_ROW_COUNT + FIXED_COLUMN_COUNT * FIXED_ROW_COUNT,
                core::mem::size_of::<u64>(),
            )?,
        )?;
        let fixed_work = checked_mul(
            FIXED_COLUMN_COUNT,
            checked_add(
                transform_work(PHYSICAL_ROW_COUNT)?,
                transform_work(domain.rows())?,
            )?,
        )?;
        // Each selector phase uses at most eight field operations after one
        // bounded inversion/exponentiation. Prefix sums and every mask run are
        // counted separately; this is structural work, not CPU instructions.
        let phase_work = checked_add(
            checked_add(
                9 * PHYSICAL_HASH_ROWS + 4096,
                checked_mul(3, graph.selector_runs)?,
            )?,
            graph.selector_masks,
        )?;
        let work_units = checked_add(
            checked_add(fixed_work, checked_mul(cycle, phase_work)?)?,
            graph.nodes,
        )?;
        // The direct SMT owner has 243 slots; its largest port slot contains a
        // 32-bit pack (96 field operations) plus selector/port arithmetic. 256
        // operations per slot also cover all shorter equations and writes. The
        // executing selector sum and complete canonical opening checks are extra.
        // Do not substitute a CSE graph size for the direct SMT evaluator's work.
        let point_work_units = checked_add(
            checked_add(graph.nodes, checked_mul(2, graph.output_terms)?)?,
            RESIDUE_COUNT * 256
                + 24 * COLUMN_COUNT
                + 5 * (PHYSICAL_HASH_ROWS + FIXED_COLUMN_COUNT)
                + CONSTRAINT_COUNT,
        )?;
        Ok(PolynomialPreparationCost {
            payload_bytes,
            work_units,
            point_work_units,
            scratch_cells: graph.nodes,
        })
    }

    /// Prepare actual public fixed polynomials on the checked full Fp4 coset.
    pub(super) fn prepare_polynomial_evaluator(
        &self,
        domain: PolynomialDomain,
    ) -> Result<PreparedPolynomialAir<'_>> {
        // The outer masked plan applies its explicit limits before reaching here.
        // Rechecking geometry keeps this internal preparation independently bound.
        self.polynomial_preparation_cost(domain)?;
        let (phases, hash) = self.hash.polynomial_evaluator(domain)?;
        let mut fixed = reserved(FIXED_COLUMN_COUNT)?;
        let planner = Planner::new(&FASTPQ_FINAL_V1);
        for column in 0..FIXED_COLUMN_COUNT {
            // These coefficients are public statement data, never witness or mask
            // storage. The existing N-point base IFFT is their interpolation owner.
            let mut base = reserved(PHYSICAL_ROW_COUNT)?;
            base.resize(PHYSICAL_ROW_COUNT, 0);
            for (&position, row) in self.fixed.positions().iter().zip(self.fixed.rows()) {
                base[position] = row[column];
            }
            planner.ifft_columns(core::slice::from_mut(&mut base));
            let mut coefficients = reserved(PHYSICAL_ROW_COUNT)?;
            coefficients.extend(base.iter().copied().map(GoldilocksFp4V1::embed_base));
            fixed.push(domain.evaluate(&coefficients, PHYSICAL_ROW_COUNT)?);
        }
        Ok(PreparedPolynomialAir {
            domain,
            smt: CompactSmtQuotient::new(&FASTPQ_FINAL_V1, &self.fixed)?,
            phases,
            fixed,
            hash,
        })
    }

    #[cfg(test)]
    fn prepare(&self) -> Result<PreparedTransferAir<'_>> {
        let lde_rows = PHYSICAL_ROW_COUNT
            .checked_mul(FASTPQ_FINAL_V1.fri.blowup_factor as usize)
            .ok_or_else(|| shape("compact fixed LDE size overflow"))?;
        if PHYSICAL_ROW_COUNT != 65_536
            || COLUMN_COUNT != 342
            || CONSTRAINT_COUNT != 923
            || lde_rows != LDE_ROWS
            || PHYSICAL_HASH_ROWS * FASTPQ_FINAL_V1.fri.blowup_factor as usize != MASK_CYCLE_ROWS
            || self.fixed.positions().len() != FIXED_ROW_COUNT
            || self.fixed.rows().len() != FIXED_ROW_COUNT
        {
            return Err(shape("unsupported compact transfer preparation schema"));
        }
        checked_matrix_bytes(FIXED_COLUMN_COUNT, lde_rows, FIXED_LDE_BYTES)?;
        checked_matrix_bytes(
            FIXED_COLUMN_COUNT,
            PHYSICAL_ROW_COUNT,
            FIXED_COEFFICIENT_BYTES,
        )?;
        checked_matrix_bytes(MASK_CYCLE_ROWS, PHYSICAL_HASH_ROWS, PHASE_CYCLE_BYTES)?;
        let smt = CompactSmtQuotient::new(&FASTPQ_FINAL_V1, &self.fixed)?;
        let domain = FriDomain::from_lde_parameters(
            FASTPQ_FINAL_V1.lde_root,
            FASTPQ_FINAL_V1.lde_log_size,
            lde_rows,
            FASTPQ_FINAL_V1.omega_coset,
        )?;
        // These 49 columns are public polynomials, never proof-supplied masks.
        // Exactly one fixed-size expansion/FFT/LDE is retained per preparation.
        let mut coefficients = vec![vec![0; PHYSICAL_ROW_COUNT]; FIXED_COLUMN_COUNT];
        for (&position, row) in self.fixed.positions().iter().zip(self.fixed.rows()) {
            for (column, &value) in coefficients.iter_mut().zip(row) {
                column[position] = value;
            }
        }
        let planner = Planner::new(&FASTPQ_FINAL_V1);
        planner.ifft_columns(&mut coefficients);
        let fixed_lde = planner.lde_columns(&coefficients);
        drop(coefficients);
        if fixed_lde.len() != FIXED_COLUMN_COUNT
            || fixed_lde.iter().any(|column| column.len() != lde_rows)
        {
            return Err(shape("compact fixed-column LDE has an unexpected shape"));
        }
        let selectors =
            PeriodicSelectors::new(&FASTPQ_FINAL_V1, PHYSICAL_ROW_COUNT, PHYSICAL_HASH_ROWS)?;
        let mut phases = Vec::with_capacity(MASK_CYCLE_ROWS);
        for index in 0..MASK_CYCLE_ROWS {
            phases.push(
                selectors
                    .evaluate(domain.point(index))?
                    .try_into()
                    .map_err(|_| shape("compact phase cycle has an unexpected width"))?,
            );
        }
        let hash_masks = self.hash.prepare_prover_masks()?;
        Ok(PreparedTransferAir {
            air: self,
            smt,
            hash_masks,
            fixed_lde,
            phases,
            domain,
        })
    }
}

impl FixedAir for CompactTransferAir {
    fn schema(&self) -> FixedAirSchema {
        FixedAirSchema {
            trace_rows: PHYSICAL_ROW_COUNT,
            width: COLUMN_COUNT,
            constraints: CONSTRAINT_COUNT,
            identity: IDENTITY,
        }
    }

    fn statement_bytes(&self) -> &[u8] {
        &self.statement_bytes
    }

    #[cfg(test)]
    fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
        self.evaluate_at(point, current, next)
    }

    #[cfg(test)]
    fn prepare_prover(&self) -> Result<Box<dyn PreparedAir + '_>> {
        Ok(Box::new(self.prepare()?))
    }
}

/// Exact immutable public preparation, borrowed by every per-proof worker.
#[cfg(test)]
struct PreparedTransferAir<'a> {
    air: &'a CompactTransferAir,
    smt: CompactSmtQuotient<'a>,
    hash_masks: ProverMaskCycle<'a>,
    fixed_lde: Vec<Vec<u64>>,
    phases: Vec<[u64; PHYSICAL_HASH_ROWS]>,
    domain: FriDomain,
}

#[cfg(test)]
impl PreparedTransferAir<'_> {
    fn fixed_at(&self, index: usize, point: u64) -> Result<CompactSmtFixedValues<u64>> {
        if index >= LDE_ROWS {
            return Err(Error::QueryIndexOutOfRange {
                index,
                len: LDE_ROWS,
            });
        }
        if point >= GOLDILOCKS_MODULUS {
            return Err(Error::NonCanonicalGoldilocksElement {
                context: "compact_transfer_prepared_point",
                indices: Vec::new(),
            });
        }
        if point != self.domain.point(index) {
            return Err(shape(
                "compact fixed cache requires its exact indexed coset point",
            ));
        }
        CompactSmtFixedValues::new(
            self.phases[index % MASK_CYCLE_ROWS],
            core::array::from_fn(|column| self.fixed_lde[column][index]),
        )
    }
}

#[cfg(test)]
impl PreparedAir for PreparedTransferAir<'_> {
    fn evaluator(&self) -> ProverEvaluator<'_> {
        let mut scratch = self.air.hash.evaluation_scratch::<u64>();
        Box::new(move |index, point, current, next| {
            let fixed = self.fixed_at(index, point)?;
            let current = decode_smt_row(current)?;
            let next = decode_smt_row(next)?;
            let hash = self.hash_masks.evaluate_with_scratch(
                index,
                &current.hash,
                &next.hash,
                &mut scratch,
            )?;
            Ok(combine(hash, self.smt.residues(&fixed, &current, &next)))
        })
    }
}

/// Public geometry-only costs; no witness or mask values are retained.
pub(super) struct PolynomialPreparationCost {
    pub(super) payload_bytes: usize,
    pub(super) work_units: usize,
    pub(super) point_work_units: usize,
    /// One graph workspace is included above; parallel callers charge each extra copy.
    pub(super) scratch_cells: usize,
}

/// Immutable exact public polynomial caches shared by bounded arithmetic workers.
pub(super) struct PreparedPolynomialAir<'a> {
    domain: PolynomialDomain,
    smt: CompactSmtQuotient<'a>,
    phases: Vec<[GoldilocksFp4V1; PHYSICAL_HASH_ROWS]>,
    fixed: Vec<PolynomialLanes>,
    hash: PolynomialHashEvaluator<'a>,
}

impl<'source> PreparedPolynomialAir<'source> {
    /// Borrow these exact caches with one separately guarded arithmetic workspace.
    pub(super) fn evaluator(&self) -> Result<PolynomialAirEvaluator<'_, 'source>> {
        Ok(PolynomialAirEvaluator {
            prepared: self,
            scratch: self.hash.scratch()?,
        })
    }
}

/// One zeroizing hash workspace tied to its immutable public polynomial owner.
pub(super) struct PolynomialAirEvaluator<'cache, 'source> {
    prepared: &'cache PreparedPolynomialAir<'source>,
    scratch: SecretPolynomial<GoldilocksFp4V1>,
}

impl PolynomialAirEvaluator<'_, '_> {
    /// Write every actual AIR slot into the caller's fixed guarded output slice.
    pub(super) fn evaluate_into(
        &mut self,
        index: usize,
        current: &[GoldilocksFp4V1],
        next: &[GoldilocksFp4V1],
        output: &mut [GoldilocksFp4V1],
    ) -> Result<()> {
        let prepared = self.prepared;
        if index >= prepared.domain.rows() {
            return Err(Error::QueryIndexOutOfRange {
                index,
                len: prepared.domain.rows(),
            });
        }
        let current: &[GoldilocksFp4V1; COLUMN_COUNT] = current
            .try_into()
            .map_err(|_| shape("polynomial AIR needs exactly 342 current cells"))?;
        let next: &[GoldilocksFp4V1; COLUMN_COUNT] = next
            .try_into()
            .map_err(|_| shape("polynomial AIR needs exactly 342 next cells"))?;
        if output.len() != CONSTRAINT_COUNT {
            return Err(shape("polynomial AIR output needs exactly 923 slots"));
        }
        for row in [current, next] {
            for (column, &value) in row.iter().enumerate() {
                value.validate("polynomial_air_opening", &[column])?;
            }
        }
        let mut sparse = [GoldilocksFp4V1::ZERO; FIXED_COLUMN_COUNT];
        for (value, column) in sparse.iter_mut().zip(&prepared.fixed) {
            *value = column.value(index)?;
        }
        let fixed =
            CompactSmtFixedValues::new(prepared.phases[index % prepared.phases.len()], sparse)?;
        let current = smt_row_from_cells(current);
        let next = smt_row_from_cells(next);
        let hash = prepared
            .hash
            .evaluate(index, &current.hash, &next.hash, &mut self.scratch)?;
        combine_into(hash, prepared.smt.residues(&fixed, &current, &next), output)
    }
}

fn combined_slots<F>(hash: HashNumerators<F>, smt: [F; RESIDUE_COUNT]) -> impl Iterator<Item = F> {
    hash.local.into_iter().chain(hash.transitions).chain(smt)
}

fn combine<F>(hash: HashNumerators<F>, smt: [F; RESIDUE_COUNT]) -> Vec<F> {
    combined_slots(hash, smt).collect()
}

fn combine_into<F>(
    hash: HashNumerators<F>,
    smt: [F; RESIDUE_COUNT],
    output: &mut [F],
) -> Result<()> {
    if output.len() != CONSTRAINT_COUNT {
        return Err(shape("polynomial AIR output needs exactly 923 slots"));
    }
    for (output, value) in output.iter_mut().zip(combined_slots(hash, smt)) {
        *output = value;
    }
    Ok(())
}

#[cfg(test)]
fn checked_matrix_bytes(columns: usize, rows: usize, maximum: usize) -> Result<usize> {
    let bytes = columns
        .checked_mul(rows)
        .and_then(|cells| cells.checked_mul(core::mem::size_of::<u64>()))
        .ok_or_else(|| shape("compact fixed preparation byte count overflow"))?;
    check_limit("max_compact_fixed_preparation_bytes", bytes, maximum)?;
    Ok(bytes)
}

fn check_limit(limit: &'static str, actual: usize, max: usize) -> Result<()> {
    if actual > max {
        Err(Error::VerifierLimitExceeded { limit, actual, max })
    } else {
        Ok(())
    }
}

fn shape(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        backend::{field_pow, fixed_domain::FixedTraceDomain},
        gadgets::{
            compact_smt_air::{PATH_LEVELS, PhysicalSmtWitness, PublicUpdate, SmtRow, SmtWitness},
            compact_trace_columns::{smt_row_cells, smt_row_from_cells},
        },
    };
    use iroha_crypto::Hash;

    fn digest(seed: u8) -> DigestLimbs {
        let hash = Hash::new([seed; 33]);
        let bytes: &[u8; 32] = hash.as_ref();
        core::array::from_fn(|limb| {
            u32::from_le_bytes(bytes[4 * limb..4 * limb + 4].try_into().unwrap())
        })
    }

    fn statement() -> PublicStatement {
        PublicStatement {
            updates: [
                PublicUpdate {
                    old_leaf: digest(1),
                    new_leaf: digest(2),
                    path: 0xa59c_71e3,
                },
                PublicUpdate {
                    old_leaf: digest(3),
                    new_leaf: digest(4),
                    path: 0x6a35_8e1c,
                },
            ],
            old_root: digest(5),
            new_root: digest(6),
        }
    }

    fn arbitrary_row(seed: u64) -> SmtRow {
        smt_row_from_cells(&core::array::from_fn(|column| seed + 17 * column as u64))
    }

    #[test]
    fn combined_relation_preserves_both_ledgers_and_exact_schema() {
        let statement = statement();
        let air = CompactTransferAir::new(&statement, Some(b"caller-claim")).unwrap();
        assert_eq!(air.schema().trace_rows, 65_536);
        assert_eq!(air.schema().width, 342);
        assert_eq!(air.schema().constraints, 923);
        assert_eq!(
            air.hash.metrics().quotient_degree_bound,
            2 * PHYSICAL_ROW_COUNT
        );
        let smt = CompactSmtQuotient::new(&FASTPQ_FINAL_V1, &air.fixed).unwrap();
        let current = arbitrary_row(3);
        let next = arbitrary_row(71);
        let generator = FixedTraceDomain::new(&FASTPQ_FINAL_V1, PHYSICAL_ROW_COUNT)
            .unwrap()
            .generator;
        for point in [
            0,
            1,
            field_pow(generator, 407),
            field_pow(generator, 408),
            field_pow(generator, 32_768),
            field_pow(generator, 65_535),
            7,
            FASTPQ_FINAL_V1.omega_coset,
        ] {
            let actual = air
                .evaluate(point, &smt_row_cells(&current), &smt_row_cells(&next))
                .unwrap();
            let hash = air.hash.evaluate(point, &current.hash, &next.hash).unwrap();
            assert_eq!(actual[..LOCAL_SLOTS], hash.local);
            assert_eq!(
                actual[LOCAL_SLOTS..LOCAL_SLOTS + TRANSITION_SLOTS],
                hash.transitions
            );
            assert_eq!(
                actual[LOCAL_SLOTS + TRANSITION_SLOTS..],
                smt.residues(&smt.evaluate_fixed(point).unwrap(), &current, &next)
            );
        }
    }

    #[test]
    fn public_leaf_path_root_and_context_mutations_change_exact_binding() {
        let original = statement();
        let air = CompactTransferAir::new(&original, None).unwrap();
        let current = smt_row_cells(&arbitrary_row(13));
        let next = smt_row_cells(&arbitrary_row(79));
        let point = FASTPQ_FINAL_V1.omega_coset;
        let expected = air.evaluate(point, &current, &next).unwrap();
        for mutation in 0..8 {
            let mut changed = original;
            match mutation {
                0 => changed.old_root[0] ^= 1,
                1 => changed.new_root[7] ^= 1 << 31,
                2 => changed.updates[0].old_leaf[7] ^= 1 << 31,
                3 => changed.updates[0].new_leaf[0] ^= 1,
                4 => changed.updates[1].old_leaf[0] ^= 1,
                5 => changed.updates[1].new_leaf[7] ^= 1 << 31,
                6 => changed.updates[0].path ^= 1,
                7 => changed.updates[1].path ^= 1 << 31,
                _ => unreachable!(),
            }
            let changed = CompactTransferAir::new(&changed, None).unwrap();
            assert_ne!(
                changed.statement_bytes(),
                air.statement_bytes(),
                "mutation={mutation}"
            );
            let actual = changed.evaluate(point, &current, &next).unwrap();
            assert_eq!(
                actual[..LOCAL_SLOTS + TRANSITION_SLOTS],
                expected[..LOCAL_SLOTS + TRANSITION_SLOTS]
            );
            assert_ne!(
                actual[LOCAL_SLOTS + TRANSITION_SLOTS..],
                expected[LOCAL_SLOTS + TRANSITION_SLOTS..],
                "mutation={mutation}"
            );
        }
        let empty = CompactTransferAir::new(&original, Some(b"")).unwrap();
        let first = CompactTransferAir::new(&original, Some(b"claim-one")).unwrap();
        let second = CompactTransferAir::new(&original, Some(b"claim-two")).unwrap();
        assert_ne!(air.statement_bytes(), empty.statement_bytes());
        assert_ne!(first.statement_bytes(), second.statement_bytes());
        // Context binding affects the transcript, not the declared AIR equations.
        assert_eq!(first.evaluate(point, &current, &next).unwrap(), expected);
        for update in original.updates {
            let bytes = digest_bytes(update.old_leaf);
            assert_eq!(
                core::array::from_fn::<_, 8, _>(|limb| {
                    u32::from_le_bytes(bytes[4 * limb..4 * limb + 4].try_into().unwrap())
                }),
                update.old_leaf
            );
        }
    }

    #[test]
    fn statement_and_opening_bounds_reject_before_prover_preparation() {
        let original = statement();
        let mut unmarked = original;
        unmarked.updates[1].new_leaf[7] &= !(1 << 24);
        assert!(CompactTransferAir::new(&unmarked, None).is_err());
        let oversized = vec![0; VerifyLimits::default().max_batch_bytes + 1];
        assert!(matches!(
            CompactTransferAir::new(&original, Some(&oversized)),
            Err(Error::VerifierLimitExceeded {
                limit: "max_compact_statement_bytes",
                ..
            })
        ));
        let air = CompactTransferAir::new(&original, None).unwrap();
        let zero = [0; COLUMN_COUNT];
        for len in [0, COLUMN_COUNT - 1, COLUMN_COUNT + 1] {
            assert!(air.evaluate(7, &vec![0; len], &zero).is_err());
            assert!(air.evaluate(7, &zero, &vec![0; len]).is_err());
        }
        for column in 0..COLUMN_COUNT {
            let mut invalid = zero;
            invalid[column] = GOLDILOCKS_MODULUS;
            for (current, next) in [(&invalid, &zero), (&zero, &invalid)] {
                assert!(
                    matches!(air.evaluate(7, current, next), Err(Error::NonCanonicalGoldilocksElement { indices, .. }) if indices == [column])
                );
            }
        }
        assert!(air.evaluate(GOLDILOCKS_MODULUS, &zero, &zero).is_err());
        assert_eq!(
            checked_matrix_bytes(49, LDE_ROWS, FIXED_LDE_BYTES).unwrap(),
            FIXED_LDE_BYTES
        );
        assert!(checked_matrix_bytes(50, LDE_ROWS, FIXED_LDE_BYTES).is_err());
        assert!(checked_matrix_bytes(usize::MAX, 2, usize::MAX).is_err());
    }

    #[test]
    fn counted_statement_length_and_factored_binding_match_original_bare_encoding() {
        let statement = statement();
        let context_bytes = b"\0complete caller claim\xff\0";
        for context in [None, Some(&[][..]), Some(&context_bytes[..])] {
            // Independent inline spelling of the original constructor payload;
            // this must remain byte-identical after factoring its count helper.
            let original = BoundStatement {
                version: 1,
                updates: statement.updates.map(|update| BoundUpdate {
                    old_leaf: digest_bytes(update.old_leaf),
                    new_leaf: digest_bytes(update.new_leaf),
                    path: update.path,
                }),
                old_root: digest_bytes(statement.old_root),
                new_root: digest_bytes(statement.new_root),
                caller_context: context.map(<[u8]>::to_vec),
            }
            .encode();
            assert_eq!(bound_statement(&statement, context).encode(), original);
            assert_eq!(
                CompactTransferAir::new(&statement, context)
                    .unwrap()
                    .statement_bytes(),
                original
            );
            for flags in (u8::MIN..=u8::MAX)
                .filter(|&flags| norito::core::validate_header_flags(flags).is_ok())
            {
                let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
                assert_eq!(
                    CompactTransferAir::encoded_statement_len(&statement, context).unwrap(),
                    original.len()
                );
                assert_eq!(norito::core::get_decode_flags(), flags);
            }
        }
        let oversized = vec![0; VerifyLimits::default().max_batch_bytes + 1];
        assert!(matches!(
            CompactTransferAir::encoded_statement_len(&statement, Some(&oversized)),
            Err(Error::VerifierLimitExceeded {
                limit: "max_compact_statement_bytes",
                ..
            })
        ));
        let fits_context_but_not_envelope = &oversized[..VerifyLimits::default().max_batch_bytes];
        assert!(matches!(
            CompactTransferAir::encoded_statement_len(
                &statement,
                Some(fits_context_but_not_envelope)
            ),
            Err(Error::VerifierLimitExceeded {
                limit: "max_compact_statement_bytes",
                ..
            })
        ));
    }

    #[test]
    fn canonical_statement_binding_restores_every_supported_ambient_layout() {
        let statement = statement();
        let context = b"\0complete caller claim\xff\0";
        let expected = CompactTransferAir::new(&statement, Some(context)).unwrap();
        for flags in
            (u8::MIN..=u8::MAX).filter(|&flags| norito::core::validate_header_flags(flags).is_ok())
        {
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            let actual = CompactTransferAir::new(&statement, Some(context)).unwrap();
            assert_eq!(actual.statement_bytes(), expected.statement_bytes());
            assert_eq!(norito::core::get_decode_flags(), flags);
        }
    }

    #[test]
    #[ignore = "explicit 219 MiB fixed-column FFT/LDE preparation diagnostic"]
    fn prepared_fixed_cache_matches_bounded_verifier_at_cycle_and_domain_boundaries() {
        let air = CompactTransferAir::new(&statement(), Some(b"fixed-cache-diagnostic")).unwrap();
        let start = std::time::Instant::now();
        let prepared = air.prepare().unwrap();
        let preparation = start.elapsed();
        assert!(core::ptr::eq(prepared.air, &air));
        assert_eq!(
            prepared
                .fixed_lde
                .iter()
                .map(|column| column.len() * 8)
                .sum::<usize>(),
            FIXED_LDE_BYTES
        );
        assert_eq!(
            core::mem::size_of_val(prepared.phases.as_slice()),
            PHASE_CYCLE_BYTES
        );
        let first = smt_row_cells(&arbitrary_row(11));
        let second = smt_row_cells(&arbitrary_row(101));
        let zero = [0; COLUMN_COUNT];
        let mut evaluate = prepared.evaluator();
        for index in [0, 1, 407, 408, 4095, 4096, 4097, 32_768, LDE_ROWS - 1, 0] {
            let point = prepared.domain.point(index);
            for (current, next) in [(&first, &second), (&second, &first), (&zero, &zero)] {
                assert_eq!(
                    evaluate(index, point, current, next).unwrap(),
                    air.evaluate(point, current, next).unwrap(),
                    "index={index}"
                );
            }
        }
        assert!(evaluate(LDE_ROWS, 0, &zero, &zero).is_err());
        assert!(evaluate(0, prepared.domain.point(1), &zero, &zero).is_err());
        assert!(evaluate(0, GOLDILOCKS_MODULUS, &zero, &zero).is_err());
        eprintln!(
            "compact_transfer_fixed_preparation={preparation:?}; fixed_lde_bytes={FIXED_LDE_BYTES}; phase_cycle_bytes={PHASE_CYCLE_BYTES}; coefficient_peak_bytes={FIXED_COEFFICIENT_BYTES}"
        );
    }

    fn root(
        mut child: DigestLimbs,
        siblings: &[DigestLimbs; PATH_LEVELS],
        path: u32,
    ) -> DigestLimbs {
        for (level, sibling) in siblings.iter().enumerate() {
            let (left, right) = if path >> level & 1 == 0 {
                (child, *sibling)
            } else {
                (*sibling, child)
            };
            let mut payload = b"fastpq:v1:smt:node|".to_vec();
            payload.extend(digest_bytes(left));
            payload.extend(digest_bytes(right));
            let hash = Hash::new(payload);
            let bytes: &[u8; 32] = hash.as_ref();
            child = core::array::from_fn(|limb| {
                u32::from_le_bytes(bytes[4 * limb..4 * limb + 4].try_into().unwrap())
            });
        }
        child
    }

    /// Prover-test-only complete witness; this native fixture is never a verifier input.
    fn physical_fixture() -> (PublicStatement, PhysicalSmtWitness) {
        let siblings = core::array::from_fn(|level| digest((level + 17) as u8));
        let path = 0xa59c_71e3;
        let first = digest(1);
        let second = digest(2);
        let old_root = root(first, &siblings, path);
        let statement = PublicStatement {
            updates: [
                PublicUpdate {
                    old_leaf: first,
                    new_leaf: second,
                    path,
                },
                PublicUpdate {
                    old_leaf: second,
                    new_leaf: first,
                    path,
                },
            ],
            old_root,
            new_root: old_root,
        };
        let witness = SmtWitness::from_inputs(&statement, &[siblings, siblings])
            .expect("two opposite updates of the same allocated path")
            .into_physical();
        (statement, witness)
    }

    #[test]
    fn polynomial_slot_writer_preserves_all_923_positions() {
        let hash = HashNumerators {
            local: core::array::from_fn(|i| i),
            transitions: core::array::from_fn(|i| LOCAL_SLOTS + i),
        };
        let smt = core::array::from_fn(|i| LOCAL_SLOTS + TRANSITION_SLOTS + i);
        let mut output = [usize::MAX; CONSTRAINT_COUNT];
        combine_into(hash, smt, &mut output).unwrap();
        assert_eq!(output, core::array::from_fn(|i| i));
        let hash = HashNumerators {
            local: [1; LOCAL_SLOTS],
            transitions: [2; TRANSITION_SLOTS],
        };
        let mut short = [7; CONSTRAINT_COUNT - 1];
        assert!(combine_into(hash, [3; RESIDUE_COUNT], &mut short).is_err());
        assert_eq!(short, [7; CONSTRAINT_COUNT - 1]);
    }

    #[test]
    #[ignore = "explicit complete Fp4 public polynomial preparation and actual AIR point oracle"]
    fn full_polynomial_preparation_matches_existing_air_at_cycle_boundaries() {
        let air = CompactTransferAir::new(&statement(), None).unwrap();
        let domain = PolynomialDomain::new(
            262_144,
            GoldilocksFp4V1::new([2, 3, 5, 7]).unwrap(),
            524_288,
            64 * 1024 * 1024,
        )
        .unwrap();
        let cost = air.polynomial_preparation_cost(domain).unwrap();
        assert!(cost.payload_bytes > FIXED_COLUMN_COUNT * domain.rows() * GoldilocksFp4V1::BYTES);
        assert!(cost.work_units > 0 && cost.point_work_units > 0);
        let prepared = air.prepare_polynomial_evaluator(domain).unwrap();
        let mut evaluator = prepared.evaluator().unwrap();
        let current = core::array::from_fn::<_, COLUMN_COUNT, _>(|column| {
            GoldilocksFp4V1::new([3 + column as u64, 5, 7, 11]).unwrap()
        });
        let next = current.map(|value| value.add(GoldilocksFp4V1::new([13, 17, 19, 23]).unwrap()));
        for index in [0, 1, 2047, 2048, 262_143] {
            let mut output = [GoldilocksFp4V1::ZERO; CONSTRAINT_COUNT];
            evaluator
                .evaluate_into(index, &current, &next, &mut output)
                .unwrap();
            assert_eq!(
                &output[..],
                air.evaluate_at(domain.point(index).unwrap(), &current, &next)
                    .unwrap()
            );
        }
        // Reuse the real production range scheduler and immutable cache at
        // phase-cycle, trace rotation and final-domain boundaries. The expected
        // weighted rows come from the independent uncached AIR point evaluator.
        use super::super::masked_quotient::{NUMERATOR_JOBS, evaluate_parallel_rows};
        let indices = [0, 1, 3, 4, 2047, 2048, 65_535, 65_536, 262_142, 262_143];
        let alpha = core::array::from_fn::<_, CONSTRAINT_COUNT, _>(|slot| {
            GoldilocksFp4V1::new([slot as u64 + 1, 3, 5, 7]).unwrap()
        });
        let mix = |values: &[GoldilocksFp4V1]| {
            values
                .iter()
                .zip(&alpha)
                .fold(GoldilocksFp4V1::ZERO, |sum, (&value, &weight)| {
                    sum.add(value.mul(weight))
                })
        };
        let expected = indices.map(|index| {
            mix(&air
                .evaluate_at(domain.point(index).unwrap(), &current, &next)
                .unwrap())
        });
        for threads in [1, 2, 6] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let mut workers = (0..NUMERATOR_JOBS)
                .map(|_| {
                    (
                        prepared.evaluator().unwrap(),
                        SecretPolynomial::zeroed(CONSTRAINT_COUNT).unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            let mut output = SecretPolynomial::zeroed(indices.len()).unwrap();
            pool.install(|| {
                evaluate_parallel_rows(&mut output, &mut workers, |row, worker| {
                    worker
                        .0
                        .evaluate_into(indices[row], &current, &next, &mut worker.1)?;
                    Ok(mix(&worker.1))
                })
            })
            .unwrap();
            assert_eq!(&*output, &expected);
        }
        let mut output = [GoldilocksFp4V1::ONE; CONSTRAINT_COUNT];
        assert!(
            evaluator
                .evaluate_into(domain.rows(), &current, &next, &mut output)
                .is_err()
        );
        assert!(
            evaluator
                .evaluate_into(0, &current[..341], &next, &mut output)
                .is_err()
        );
        assert!(
            evaluator
                .evaluate_into(0, &current, &next, &mut output[..922])
                .is_err()
        );
        let mut malformed = current;
        malformed[341] =
            GoldilocksFp4V1::from_coefficients_unchecked_for_test([0, 0, 0, GOLDILOCKS_MODULUS]);
        assert!(
            evaluator
                .evaluate_into(0, &malformed, &next, &mut output)
                .is_err()
        );
        assert!(output.iter().all(|&value| value == GoldilocksFp4V1::ONE));
    }

    #[test]
    #[ignore = "explicit full 65536x342 masked Fp4 numerator/quotient diagnostic; several GiB, no PCS qualification"]
    fn complete_masked_numerator_divides_exactly_and_matches_actual_air() {
        use super::super::{
            coefficient_masking::MaskingShape,
            masked_quotient::{MaskedQuotientLimits, MaskedQuotientPlan, PreparedMaskedTrace},
            secret_polynomial::SecretPolynomial,
        };
        type F = GoldilocksFp4V1;
        let (statement, witness) = physical_fixture();
        let air = CompactTransferAir::new(&statement, None).unwrap();
        let mut trace: Vec<SecretPolynomial<F>> = (0..COLUMN_COUNT)
            .map(|_| SecretPolynomial::zeroed(PHYSICAL_ROW_COUNT).unwrap())
            .collect();
        for (index, row) in witness.rows().iter().enumerate() {
            for (column, value) in trace.iter_mut().zip(smt_row_cells(row)) {
                column[index] = F::embed_base(value);
            }
        }
        drop(witness);
        // Explicit deterministic arithmetic test coefficients, never production hiding coins.
        let masks: Vec<_> = (0..COLUMN_COUNT)
            .map(|column| [F::new([31 + column as u64, 37, 41, 43]).unwrap()])
            .collect();
        let shape = MaskingShape {
            trace_coefficients: PHYSICAL_ROW_COUNT,
            trace_degree_bound: PHYSICAL_ROW_COUNT,
            mask_coefficients: 1,
            mask_degree_bound: 1,
        };
        let limits = MaskedQuotientLimits {
            max_payload_bytes: usize::try_from(8_u64 * 1024 * 1024 * 1024).unwrap(),
            max_work_units: usize::MAX,
            max_interpolation_rows: 524_288,
            max_mask_coefficients: 1,
            max_masked_coefficients: PHYSICAL_ROW_COUNT + 1,
        };
        let trace_refs: Vec<_> = trace.iter().map(|values| &**values).collect();
        let mask_refs: Vec<_> = masks.iter().map(|values| &values[..]).collect();
        let prepared = PreparedMaskedTrace::prepare(
            &trace_refs,
            &mask_refs,
            &vec![shape; COLUMN_COUNT],
            limits,
        )
        .unwrap();
        drop(trace_refs);
        drop(trace);
        assert!(
            prepared
                .degree_bounds()
                .iter()
                .all(|&degree| degree == PHYSICAL_ROW_COUNT + 1)
        );
        assert_eq!(prepared.column(0).unwrap().len(), PHYSICAL_ROW_COUNT + 1);
        assert!(prepared.column(COLUMN_COUNT).is_err());
        let offset = F::new([2, 3, 5, 7]).unwrap();
        assert!(
            MaskedQuotientPlan::new(&air, &prepared, 131_072, offset, 131_072, limits).is_err()
        );
        assert!(
            MaskedQuotientPlan::new(&air, &prepared, 262_144, F::ONE, 131_072, limits).is_err()
        );
        assert!(MaskedQuotientPlan::new(&air, &prepared, 262_144, offset, 1, limits).is_err());
        let plan =
            MaskedQuotientPlan::new(&air, &prepared, 262_144, offset, 131_072, limits).unwrap();
        let bytes = plan.payload_bytes();
        let work = plan.work_units();
        assert!(
            MaskedQuotientPlan::new(
                &air,
                &prepared,
                262_144,
                offset,
                131_072,
                MaskedQuotientLimits {
                    max_payload_bytes: bytes - 1,
                    ..limits
                }
            )
            .is_err()
        );
        assert!(
            MaskedQuotientPlan::new(
                &air,
                &prepared,
                262_144,
                offset,
                131_072,
                MaskedQuotientLimits {
                    max_work_units: work - 1,
                    ..limits
                }
            )
            .is_err()
        );
        assert!(plan.build(&[F::ONE; 922]).is_err());
        let plan =
            MaskedQuotientPlan::new(&air, &prepared, 262_144, offset, 131_072, limits).unwrap();
        let mut alpha = [F::ONE; CONSTRAINT_COUNT];
        alpha[922] = F::from_coefficients_unchecked_for_test([0, 0, GOLDILOCKS_MODULUS, 0]);
        assert!(plan.build(&alpha).is_err());
        let alpha: Vec<_> = (0..CONSTRAINT_COUNT)
            .map(|i| F::new([i as u64 + 1, 47, 53, 59]).unwrap())
            .collect();
        let result = MaskedQuotientPlan::new(&air, &prepared, 262_144, offset, 131_072, limits)
            .unwrap()
            .build(&alpha)
            .unwrap();
        assert_eq!(result.numerator().len(), 262_144);
        assert_eq!(result.quotient().coefficients().len(), 131_072);
        assert!(
            result.numerator()[result.degrees().combined_numerator()..]
                .iter()
                .all(|&c| c == F::ZERO)
        );
        let horner = |coefficients: &[F], point: F| {
            coefficients
                .iter()
                .rev()
                .fold(F::ZERO, |sum, &value| sum.mul(point).add(value))
        };
        let omega = FixedTraceDomain::new(&FASTPQ_FINAL_V1, PHYSICAL_ROW_COUNT)
            .unwrap()
            .generator;
        for index in [0, 2048, 262_143] {
            let point = result.domain().point(index).unwrap();
            let current: Vec<_> = (0..COLUMN_COUNT)
                .map(|column| horner(prepared.column(column).unwrap(), point))
                .collect();
            let next: Vec<_> = (0..COLUMN_COUNT)
                .map(|column| horner(prepared.column(column).unwrap(), point.mul_base(omega)))
                .collect();
            let expected = air
                .evaluate_at(point, &current, &next)
                .unwrap()
                .iter()
                .zip(&alpha)
                .fold(F::ZERO, |sum, (&value, &weight)| sum.add(value.mul(weight)));
            let numerator = horner(result.numerator(), point);
            assert_eq!(numerator, expected);
            assert_eq!(
                numerator,
                point
                    .power(PHYSICAL_ROW_COUNT as u64)
                    .sub(F::ONE)
                    .mul(horner(result.quotient().coefficients(), point))
            );
        }
    }

    /// Materialise exact base columns only in explicit prover resource tests.
    fn physical_columns(witness: &PhysicalSmtWitness) -> Vec<Vec<u64>> {
        let mut columns = (0..COLUMN_COUNT)
            .map(|_| Vec::with_capacity(PHYSICAL_ROW_COUNT))
            .collect::<Vec<_>>();
        for row in witness.rows() {
            for (column, value) in columns.iter_mut().zip(smt_row_cells(row)) {
                column.push(value);
            }
        }
        columns
    }

    #[test]
    #[ignore = "explicit full 65536x342 compact SMT prover resource diagnostic"]
    fn complete_smt_prover_diagnostic() {
        let start = std::time::Instant::now();
        let (statement, witness) = physical_fixture();
        let columns = physical_columns(&witness);
        drop(witness);
        let construction = start.elapsed();
        let air =
            CompactTransferAir::new(&statement, Some(b"test-only-full-smt-diagnostic")).unwrap();
        let prove_start = std::time::Instant::now();
        let proof = super::super::compact_protocol::prove(&air, &columns).unwrap();
        let proving = prove_start.elapsed();
        drop(columns);
        let proof_bytes = norito::core::to_bytes(&proof).unwrap().len();
        let limits = VerifyLimits::default();
        let verification_start = std::time::Instant::now();
        let verification = super::super::compact_protocol::verify(&air, &proof, limits);
        eprintln!(
            "compact_transfer_construction={construction:?}; prove={proving:?}; verify={:?}; proof_bytes={proof_bytes}; default_proof_limit={}; default_verification={verification:?}",
            verification_start.elapsed(),
            limits.max_proof_bytes
        );
        // Defaults are intentionally never raised by this diagnostic. A proof
        // outside its byte envelope remains a reported production blocker.
        assert!(proof_bytes > limits.max_proof_bytes);
        assert!(matches!(
            verification,
            Err(Error::VerifierLimitExceeded {
                limit: "max_queries",
                actual: 375,
                max: 136
            })
        ));
        let byte_policy = VerifyLimits {
            max_queries: 375,
            ..limits
        };
        assert!(matches!(
            super::super::compact_protocol::verify(&air, &proof, byte_policy),
            Err(Error::VerifierLimitExceeded {
                limit: "max_proof_bytes",
                ..
            })
        ));
        // Independently exercise the complete verifier after all private trace
        // objects have been dropped, even when the default byte gate rejects.
        // This explicit test envelope never alters production policy.
        let diagnostic_limits = VerifyLimits {
            max_proof_bytes: 16 * 1024 * 1024,
            max_queries: 375,
            ..limits
        };
        let diagnostic_start = std::time::Instant::now();
        let work = super::super::compact_protocol::verify(&air, &proof, diagnostic_limits)
            .expect("valid full SMT proof within the explicit diagnostic envelope");
        assert_eq!(work.air_evaluations, 375);
        assert!((375..=750).contains(&work.row_leaves));
        eprintln!(
            "compact_transfer_diagnostic_verify={:?}; work={work:?}; diagnostic_limit={}; production_profile_qualified=false",
            diagnostic_start.elapsed(),
            diagnostic_limits.max_proof_bytes
        );
        let mut changed = statement;
        changed.new_root[0] ^= 1;
        let changed =
            CompactTransferAir::new(&changed, Some(b"test-only-full-smt-diagnostic")).unwrap();
        assert!(
            super::super::compact_protocol::verify(&changed, &proof, diagnostic_limits).is_err()
        );
    }

    #[test]
    fn complete_air_field_points_preserve_all_923_slots_and_base_embeddings() {
        use super::super::polynomial_reference;
        use crate::field::GoldilocksFp4V1 as F;
        let air = CompactTransferAir::new(&statement(), None).unwrap();
        let current = smt_row_cells(&arbitrary_row(13));
        let next = smt_row_cells(&arbitrary_row(73));
        for point in [0, 1, 7, FASTPQ_FINAL_V1.omega_coset] {
            let base = air.evaluate(point, &current, &next).unwrap();
            let extension = air
                .evaluate_at(
                    F::embed_base(point),
                    &current.map(F::embed_base),
                    &next.map(F::embed_base),
                )
                .unwrap();
            assert_eq!(
                extension,
                base.into_iter().map(F::embed_base).collect::<Vec<_>>()
            );
        }
        let current = smt_row_from_cells(&core::array::from_fn(|column| {
            F::new([column as u64 + 1, 3, 5, 7]).unwrap()
        }));
        let next = smt_row_from_cells(&core::array::from_fn(|column| {
            F::new([column as u64 + 11, 13, 17, 19]).unwrap()
        }));
        let smt = CompactSmtQuotient::new(&FASTPQ_FINAL_V1, &air.fixed).unwrap();
        for point in polynomial_reference::points().into_iter().skip(5) {
            let actual = air
                .evaluate_at(point, &smt_row_cells(&current), &smt_row_cells(&next))
                .unwrap();
            let hash = air.hash.evaluate(point, &current.hash, &next.hash).unwrap();
            let semantic = smt.residues(&smt.evaluate_fixed(point).unwrap(), &current, &next);
            assert_eq!(actual.len(), 923);
            assert_eq!(actual[..597], hash.local);
            assert_eq!(actual[597..680], hash.transitions);
            assert_eq!(actual[680..], semantic);
        }
    }

    #[test]
    fn complete_air_rejects_dimensions_and_every_point_or_cell_coordinate() {
        use crate::field::GoldilocksFp4V1 as F;
        let air = CompactTransferAir::new(&statement(), None).unwrap();
        let zero = [F::ZERO; COLUMN_COUNT];
        for width in [0, COLUMN_COUNT - 1, COLUMN_COUNT + 1] {
            let wrong = vec![F::ZERO; width];
            assert!(matches!(
                air.evaluate_at(F::ONE, &wrong, &zero),
                Err(Error::InvalidTraceShape { .. })
            ));
            assert!(matches!(
                air.evaluate_at(F::ONE, &zero, &wrong),
                Err(Error::InvalidTraceShape { .. })
            ));
        }
        for lane in 0..4 {
            let mut words = [0; 4];
            words[lane] = GOLDILOCKS_MODULUS;
            let bad = F::from_coefficients_unchecked_for_test(words);
            assert!(
                matches!(air.evaluate_at(bad, &zero, &zero), Err(Error::NonCanonicalGoldilocksElement { context: "fixed_schedule_evaluation_point", indices }) if indices == [lane])
            );
            for column in 0..COLUMN_COUNT {
                let mut row = zero;
                row[column] = bad;
                for (current, next) in [(&row, &zero), (&zero, &row)] {
                    assert!(
                        matches!(air.evaluate_at(F::ONE, current, next), Err(Error::NonCanonicalGoldilocksElement { context: "compact_smt_opening", indices }) if indices == [column,lane])
                    );
                }
            }
        }
    }
    #[test]
    fn masked_degree_owner_preserves_all_923_slots_and_full_numerator_bounds() {
        let air = CompactTransferAir::new(&statement(), None).unwrap();
        let n = PHYSICAL_ROW_COUNT;
        for d in [1, n, n + 1, n + 65, 2 * n] {
            let bounds = air.numerator_degree_bounds(&[d; COLUMN_COUNT]).unwrap();
            let periodic = n - n / PHYSICAL_HASH_ROWS;
            let hash = 2 * d - 1 + periodic;
            let smt = d + n - 1;
            assert_eq!(
                bounds.numerators()[..LOCAL_SLOTS + TRANSITION_SLOTS]
                    .iter()
                    .copied()
                    .max(),
                Some(hash)
            );
            assert_eq!(
                bounds.numerators()[LOCAL_SLOTS + TRANSITION_SLOTS..]
                    .iter()
                    .copied()
                    .max(),
                Some(smt)
            );
            assert_eq!(bounds.combined_numerator(), hash.max(smt));
            let conditional = bounds.conditional_quotients();
            assert_eq!(conditional.combined, hash.max(smt).saturating_sub(n));
            for (slot, &degree) in bounds.numerators()[LOCAL_SLOTS + TRANSITION_SLOTS..]
                .iter()
                .enumerate()
            {
                assert_eq!(
                    degree,
                    if slot < 155 { periodic + d } else { n + d - 1 },
                    "SMT slot {slot}, d={d}"
                );
            }
        }
        let unmasked = air.numerator_degree_bounds(&[n; COLUMN_COUNT]).unwrap();
        assert_eq!(unmasked.combined_numerator() - 1, 196_478);
        assert_eq!(unmasked.conditional_quotients().combined, 130_943);
        let masked = air
            .numerator_degree_bounds(&[n + 65; COLUMN_COUNT])
            .unwrap();
        assert_eq!(masked.conditional_quotients().combined, 2 * n + 1);
        assert!(masked.conditional_quotients().combined > 2 * n);
    }

    #[test]
    fn heterogeneous_column_degrees_preserve_owner_and_rotation_dependencies() {
        let air = CompactTransferAir::new(&statement(), None).unwrap();
        let baseline = air.numerator_degree_bounds(&[1; COLUMN_COUNT]).unwrap();
        let mut columns = [1; COLUMN_COUNT];
        // Starting root limb zero is column 334; its next-row carry must retain
        // the same bound, while unrelated hash cells and limbs remain unchanged.
        columns[334] = PHYSICAL_ROW_COUNT + 27;
        let changed = air.numerator_degree_bounds(&columns).unwrap();
        let hash_slots = LOCAL_SLOTS + TRANSITION_SLOTS;
        assert_eq!(
            &changed.numerators()[..hash_slots],
            &baseline.numerators()[..hash_slots]
        );
        assert_eq!(
            changed.numerators()[hash_slots + 187],
            PHYSICAL_ROW_COUNT + columns[334] - 1
        );
        assert_eq!(
            changed.numerators()[hash_slots + 211],
            PHYSICAL_ROW_COUNT + columns[334] - 1
        );
        assert_eq!(
            changed.numerators()[hash_slots + 188],
            baseline.numerators()[hash_slots + 188]
        );
        for (&before, &after) in baseline.numerators().iter().zip(changed.numerators()) {
            assert!(after >= before);
        }
        let row = smt_row_from_cells(&columns.map(PolynomialDegree::from_exclusive));
        let hash = air
            .hash
            .numerator_degree_bounds(&crate::gadgets::compact_trace_columns::hash_row_cells(
                &row.hash,
            ))
            .unwrap();
        let smt = air
            .fixed
            .numerator_degree_bounds(&columns.map(PolynomialDegree::from_exclusive))
            .unwrap();
        let expected: Vec<_> = hash
            .local
            .into_iter()
            .chain(hash.transitions)
            .chain(smt)
            .map(PolynomialDegree::exclusive)
            .collect();
        assert_eq!(changed.numerators().as_slice(), expected);
    }

    #[test]
    fn masked_degree_declarations_reject_wrong_shapes_zero_bounds_and_overflow() {
        let air = CompactTransferAir::new(&statement(), None).unwrap();
        for width in [COLUMN_COUNT - 1, COLUMN_COUNT + 1] {
            assert!(air.numerator_degree_bounds(&vec![1; width]).is_err());
        }
        for column in 0..COLUMN_COUNT {
            let mut degrees = [1; COLUMN_COUNT];
            degrees[column] = 0;
            assert!(
                air.numerator_degree_bounds(&degrees).is_err(),
                "column {column}"
            );
        }
        assert!(
            air.numerator_degree_bounds(&[usize::MAX; COLUMN_COUNT])
                .is_err()
        );
        for column in [310, 318, 326, 334] {
            let mut degrees = [1; COLUMN_COUNT];
            degrees[column] = usize::MAX;
            assert!(
                air.numerator_degree_bounds(&degrees).is_err(),
                "column {column}"
            );
        }
    }
}
