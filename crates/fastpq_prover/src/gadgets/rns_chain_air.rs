//! Canonical-input RNS centered conversion and division polynomial constraints.
//!
//! The arithmetic plans use private shared word columns. They do not publish
//! endpoints or authenticate a profile, key, ciphertext or program. Separate
//! [`RnsScaleDownAir`] and [`RnsCenteredExtensionAir`] wrappers bind public endpoints
//! for the existing uncommitted algebraic reference. It exposes its trace and is
//! not a STARK proof. A wide row per coefficient is a bounded component relation;
//! this layout establishes no viable maximum-size committed bootstrap trace.
//!
//! The native owner accepts unreduced words; this relation deliberately accepts
//! canonical residues only. The full public `u64` scale is bound through Norito,
//! never reduced into one Goldilocks element. BGV callers use the master scale.
//! TODO(C.8, B.2): compose the remaining key-switch/NTT/bootstrap relations and the
//! committed shared engine, including hiding, security, custody and maximum work.

mod centered_extension;
mod integer;

pub use centered_extension::{RnsCenteredExtensionAir, RnsCenteredExtensionPlan};

use core::mem::size_of;

use iroha_fhe::{
    constant_time::FixedModulus,
    modular::{mod_inv_prime_u64, mul_mod_u64},
    rns_chain::{CongruentScaleDown, ConversionTarget, MAX_BASIS_PRIMES, RnsChainError},
};
use zeroize::Zeroizing;

use self::integer::{GateKind, WordViews};
use crate::air::{AirSchema, IntegerAirField, PolynomialField, SemanticAir, WorkLimits};

const WORD_BITS: usize = 64;
const IDENTITY: &str = "fastpq:rns-congruent-scale-down:v1";

/// Failure of public preparation, canonical input generation or row geometry.
#[derive(Clone, Copy, Debug, thiserror::Error)]
pub enum RnsChainAirError {
    /// The shared FHE owner refused the public parameters.
    #[error(transparent)]
    Parameters(#[from] RnsChainError),
    /// A public dimension or slice does not have the required shape.
    #[error("RNS AIR shape is invalid: {0}")]
    Shape(&'static str),
    /// An input coefficient is outside its canonical residue interval.
    #[error("RNS AIR input is not a canonical residue")]
    NonCanonical,
    /// Checked public size arithmetic overflowed.
    #[error("RNS AIR public geometry overflows usize")]
    Overflow,
    /// A public requirement exceeds its caller-provided ceiling.
    #[error("RNS AIR limit {limit} exceeded: {actual} > {max}")]
    Limit {
        /// Name of the exceeded quantity.
        limit: &'static str,
        /// Required quantity.
        actual: usize,
        /// Caller ceiling.
        max: usize,
    },
    /// The bounded diagnostic statement could not be serialized.
    #[error("RNS AIR diagnostic statement encoding failed")]
    Encoding,
}

/// Checked structural requirements, not process RSS or committed proof costs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RnsChainRequirements {
    /// Real coefficient rows before zero padding.
    pub coefficients: usize,
    /// Power-of-two row count including padding.
    pub rows: usize,
    /// Complete field-column width.
    pub width: usize,
    /// Exact number of numerators per row.
    pub constraints: usize,
    /// Exclusive numerator degree bound, including the active column.
    pub numerator_degree_bound: usize,
    /// Number of cells in the complete owned witness.
    pub trace_cells: usize,
    /// Conservative public reservation for the complete Norito statement.
    pub statement_bytes: usize,
    /// Declared owned native buffers and fixed guarded fill scratch.
    pub native_scratch_bytes: usize,
    /// Declared plan, witness, native scratch and diagnostic payload reservation.
    /// Caller-owned numerator sinks and the separate reference owner are excluded.
    pub payload_bytes: usize,
    /// Structural row-cell plus numerator admission units.
    /// This is not measured native arithmetic, executed writes or a CPU bound.
    pub work_units: usize,
}

#[derive(Clone, Copy, Debug)]
struct Gate {
    kind: GateKind,
    modulus: u64,
    left: usize,
    right: Option<usize>,
    output: Option<usize>,
    auxiliary: usize,
    residue: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Counts {
    words: usize,
    gates: usize,
    auxiliary: usize,
    gate_constraints: usize,
    constants: usize,
    equalities: usize,
}

#[derive(Debug)]
struct Program {
    gates: Vec<Gate>,
    constants: Vec<(usize, u64)>,
    equalities: Vec<(usize, usize)>,
}

struct Builder {
    counts: Counts,
    program: Option<Program>,
}

fn add(left: usize, right: usize) -> Result<usize, RnsChainAirError> {
    left.checked_add(right).ok_or(RnsChainAirError::Overflow)
}

fn multiply(left: usize, right: usize) -> Result<usize, RnsChainAirError> {
    left.checked_mul(right).ok_or(RnsChainAirError::Overflow)
}

impl Builder {
    fn word(&mut self) -> Result<usize, RnsChainAirError> {
        let word = self.counts.words;
        self.counts.words = add(word, 1)?;
        Ok(word)
    }

    fn constant(&mut self, value: u64) -> Result<usize, RnsChainAirError> {
        let word = self.word()?;
        self.counts.constants = add(self.counts.constants, 1)?;
        if let Some(program) = &mut self.program {
            program.constants.push((word, value));
        }
        Ok(word)
    }

    fn gate(
        &mut self,
        kind: GateKind,
        modulus: u64,
        left: usize,
        right: Option<usize>,
    ) -> Result<usize, RnsChainAirError> {
        let output = if matches!(kind, GateKind::Canonical) {
            None
        } else {
            Some(self.word()?)
        };
        let layout = kind.layout();
        let gate = Gate {
            kind,
            modulus,
            left,
            right,
            output,
            auxiliary: self.counts.auxiliary,
            residue: self.counts.gate_constraints,
        };
        self.counts.gates = add(self.counts.gates, 1)?;
        self.counts.auxiliary = add(self.counts.auxiliary, layout.auxiliary_columns)?;
        self.counts.gate_constraints = add(self.counts.gate_constraints, layout.constraint_count)?;
        if let Some(program) = &mut self.program {
            program.gates.push(gate);
        }
        Ok(output.unwrap_or(left))
    }

    fn equal(&mut self, left: usize, right: usize) -> Result<(), RnsChainAirError> {
        self.counts.equalities = add(self.counts.equalities, 1)?;
        if let Some(program) = &mut self.program {
            program.equalities.push((left, right));
        }
        Ok(())
    }

    fn reconstruct(
        &mut self,
        primes: &[u64],
        target: u64,
        first_digit: usize,
        terms: usize,
    ) -> Result<usize, RnsChainAirError> {
        let mut accumulator = self.constant(0)?;
        let mut weight = 1_u64;
        for (index, &prime) in primes.iter().take(terms).enumerate() {
            let digit = self.gate(GateKind::Reduce, target, first_digit + index, None)?;
            let constant = self.constant(weight)?;
            let term = self.gate(GateKind::MulMod, target, digit, Some(constant))?;
            accumulator = self.gate(GateKind::AddMod, target, accumulator, Some(term))?;
            weight = mul_mod_u64(weight, prime % target, target);
        }
        Ok(accumulator)
    }
}

fn compile(
    kept: &[u64],
    dropped: &[u64],
    scale: u64,
    reserve: Option<Counts>,
) -> Result<Builder, RnsChainAirError> {
    let mut builder = Builder {
        counts: Counts::default(),
        program: reserve.map(|counts| Program {
            gates: Vec::with_capacity(counts.gates),
            constants: Vec::with_capacity(counts.constants),
            equalities: Vec::with_capacity(counts.equalities),
        }),
    };
    // These endpoints occupy the initial contiguous words in the diagnostic view.
    for _ in 0..(2 * kept.len() + dropped.len()) {
        builder.word()?;
    }
    let first_digit = builder.counts.words;
    for _ in dropped {
        builder.word()?;
    }
    let negative = builder.word()?;
    for (index, &prime) in kept.iter().enumerate() {
        builder.gate(GateKind::Canonical, prime, index, None)?;
        builder.gate(
            GateKind::Canonical,
            prime,
            kept.len() + dropped.len() + index,
            None,
        )?;
    }
    for (index, &prime) in dropped.iter().enumerate() {
        builder.gate(GateKind::Canonical, prime, kept.len() + index, None)?;
        builder.gate(GateKind::Canonical, prime, first_digit + index, None)?;
        let value = builder.reconstruct(dropped, prime, first_digit, index + 1)?;
        let factor = builder.constant(scale % prime)?;
        let recovered = builder.gate(GateKind::MulMod, prime, value, Some(factor))?;
        builder.equal(recovered, kept.len() + index)?;
    }
    for (index, &prime) in kept.iter().enumerate() {
        let value = builder.reconstruct(dropped, prime, first_digit, dropped.len())?;
        let product = dropped.iter().fold(1_u64, |value, &factor| {
            mul_mod_u64(value, factor % prime, prime)
        });
        let product_word = builder.constant(product)?;
        let correction = builder.gate(GateKind::MulMod, prime, negative, Some(product_word))?;
        let centered = builder.gate(GateKind::SubMod, prime, value, Some(correction))?;
        let factor = builder.constant(scale % prime)?;
        let output = kept.len() + dropped.len() + index;
        let scaled_output = builder.gate(GateKind::MulMod, prime, output, Some(product_word))?;
        let scaled_center = builder.gate(GateKind::MulMod, prime, centered, Some(factor))?;
        let recovered =
            builder.gate(GateKind::AddMod, prime, scaled_output, Some(scaled_center))?;
        builder.equal(recovered, index)?;
    }
    Ok(builder)
}

fn limit(name: &'static str, actual: usize, max: usize) -> Result<(), RnsChainAirError> {
    if actual > max {
        Err(RnsChainAirError::Limit {
            limit: name,
            actual,
            max,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Geometry {
    coefficients: usize,
    rows: usize,
    width: usize,
    constraints: usize,
    comparator: usize,
    auxiliary: usize,
    trace_cells: usize,
    endpoint_words: usize,
    statement_bytes: usize,
    program_bytes: usize,
    public_columns: usize,
}

impl Geometry {
    fn new(
        counts: Counts,
        source_primes: usize,
        endpoint_words_per_row: usize,
        parameter_words: usize,
        coefficients: usize,
    ) -> Result<Self, RnsChainAirError> {
        let rows = coefficients
            .checked_next_power_of_two()
            .ok_or(RnsChainAirError::Overflow)?;
        let comparator = add(1, multiply(WORD_BITS, counts.words)?)?;
        let comparison_bits = multiply(WORD_BITS, source_primes)?;
        let auxiliary = add(comparator, multiply(2, add(comparison_bits, 1)?)?)?;
        let width = add(auxiliary, counts.auxiliary)?;
        let constraints = add(
            add(
                counts.gate_constraints,
                multiply(WORD_BITS, add(counts.constants, counts.equalities)?)?,
            )?,
            add(multiply(4, comparison_bits)?, 67)?,
        )?;
        let trace_cells = multiply(rows, width)?;
        let endpoint_words = multiply(coefficients, endpoint_words_per_row)?;
        // Explicit conservative reservation, checked again against the actual
        // bounded Norito result at bind.
        let statement_bytes = add(4096, multiply(64, add(endpoint_words, parameter_words)?)?)?;
        let program_bytes = add(
            add(
                multiply(counts.gates, size_of::<Gate>())?,
                multiply(counts.constants, size_of::<(usize, u64)>())?,
            )?,
            multiply(counts.equalities, size_of::<(usize, usize)>())?,
        )?;
        let public_columns = multiply(
            add(1, multiply(WORD_BITS, endpoint_words_per_row)?)?,
            size_of::<usize>(),
        )?;
        Ok(Self {
            coefficients,
            rows,
            width,
            constraints,
            comparator,
            auxiliary,
            trace_cells,
            endpoint_words,
            statement_bytes,
            program_bytes,
            public_columns,
        })
    }

    fn admit(
        self,
        payload_bytes: usize,
        scratch: usize,
        limits: WorkLimits,
    ) -> Result<RnsChainRequirements, RnsChainAirError> {
        let work_units = multiply(self.rows, add(self.width, self.constraints)?)?;
        let numerator_degree_bound = add(multiply(2, self.rows - 1)?, 1)?;
        let requirements = RnsChainRequirements {
            coefficients: self.coefficients,
            rows: self.rows,
            width: self.width,
            constraints: self.constraints,
            numerator_degree_bound,
            trace_cells: self.trace_cells,
            statement_bytes: self.statement_bytes,
            native_scratch_bytes: scratch,
            payload_bytes,
            work_units,
        };
        for (name, actual, max) in [
            ("trace_rows", self.rows, limits.max_trace_rows),
            ("trace_cells", self.trace_cells, limits.max_trace_cells),
            ("constraints", self.constraints, limits.max_constraints),
            (
                "statement_bytes",
                self.statement_bytes,
                limits.max_statement_bytes,
            ),
            ("payload_bytes", payload_bytes, limits.max_payload_bytes),
            ("work_units", work_units, limits.max_work_units),
        ] {
            limit(name, actual, max)?;
        }
        Ok(requirements)
    }
}

fn native_heap_bytes(kept: usize, dropped: usize) -> Result<usize, RnsChainAirError> {
    // Exact declared Vec capacities of the shared native constructor. Inline
    // headers are already inside size_of::<CongruentScaleDown>() in the plan.
    let triangle = multiply(dropped, dropped.saturating_sub(1))? / 2;
    let basis = add(
        multiply(dropped, size_of::<FixedModulus>() + size_of::<Vec<u64>>())?,
        multiply(add(triangle, multiply(3, dropped)?)?, size_of::<u64>())?,
    )?;
    let targets = multiply(
        kept,
        size_of::<FixedModulus>() + size_of::<ConversionTarget>(),
    )?;
    let words = add(add(dropped, multiply(2, kept)?)?, multiply(kept, dropped)?)?;
    add(add(basis, targets)?, multiply(words, size_of::<u64>())?)
}

/// Immutable public arithmetic schedule with checked caller ceilings.
///
/// Construction reads only parameters and public dimensions. No coefficient
/// reader is accepted until all structural limits have been checked.
#[derive(Debug)]
pub struct RnsScaleDownPlan {
    kept: Vec<u64>,
    dropped: Vec<u64>,
    scale: u64,
    scale_inverses: Vec<u64>,
    native: CongruentScaleDown,
    program: Program,
    counts: Counts,
    requirements: RnsChainRequirements,
    limits: WorkLimits,
    comparator: usize,
    auxiliary: usize,
}

impl RnsScaleDownPlan {
    /// Prepare exact shared-wire geometry and validate through the native owner.
    ///
    /// # Errors
    /// Rejects empty/oversized prime lists, zero coefficients, overflow, caller
    /// ceilings, or any parameter rejected by [`CongruentScaleDown::new`].
    pub fn new(
        kept: &[u64],
        dropped: &[u64],
        scale: u64,
        coefficients: usize,
        limits: WorkLimits,
    ) -> Result<Self, RnsChainAirError> {
        if kept.is_empty()
            || dropped.is_empty()
            || kept.len() > MAX_BASIS_PRIMES
            || dropped.len() > MAX_BASIS_PRIMES
            || coefficients == 0
        {
            return Err(RnsChainAirError::Shape(
                "nonempty bounded bases and coefficient rows required",
            ));
        }
        let dummy = [3_u64; MAX_BASIS_PRIMES];
        let counts = compile(&dummy[..kept.len()], &dummy[..dropped.len()], 1, None)?.counts;
        let geometry = Geometry::new(
            counts,
            dropped.len(),
            add(multiply(2, kept.len())?, dropped.len())?,
            add(kept.len(), dropped.len())?,
            coefficients,
        )?;
        let Geometry {
            comparator,
            auxiliary,
            trace_cells,
            endpoint_words,
            statement_bytes,
            program_bytes,
            public_columns,
            ..
        } = geometry;
        let parameters = multiply(
            add(kept.len(), multiply(2, dropped.len())?)?,
            size_of::<u64>(),
        )?;
        let native_outputs = multiply(multiply(coefficients, kept.len())?, size_of::<u64>())?;
        let scratch = add(
            multiply(add(counts.words, MAX_BASIS_PRIMES)?, size_of::<u64>())?,
            add(
                add(native_outputs, multiply(2, size_of::<Vec<u64>>())?)?,
                integer::NATIVE_SCRATCH_BYTES,
            )?,
        )?;
        let diagnostic_scratch = add(
            size_of::<ScaleDownStatement>(),
            multiply(add(kept.len(), dropped.len())?, size_of::<u64>())?,
        )?;
        let payload_bytes = add(
            diagnostic_scratch,
            add(
                add(
                    add(
                        add(size_of::<RnsScaleDownAir>(), size_of::<RnsChainWitness>())?,
                        program_bytes,
                    )?,
                    add(parameters, native_heap_bytes(kept.len(), dropped.len())?)?,
                )?,
                add(
                    add(multiply(trace_cells, size_of::<u64>())?, scratch)?,
                    add(
                        add(statement_bytes, multiply(endpoint_words, size_of::<u64>())?)?,
                        public_columns,
                    )?,
                )?,
            )?,
        )?;
        let requirements = geometry.admit(payload_bytes, scratch, limits)?;
        let native = CongruentScaleDown::new(kept, dropped, scale)?;
        let compiled = compile(kept, dropped, scale, Some(counts))?;
        if compiled.counts != counts {
            return Err(RnsChainAirError::Shape("public compilation count changed"));
        }
        let mut scale_inverses = Vec::with_capacity(dropped.len());
        for &prime in dropped {
            scale_inverses.push(
                mod_inv_prime_u64(scale % prime, prime)
                    .ok_or(RnsChainAirError::Shape("validated scale inverse missing"))?,
            );
        }
        Ok(Self {
            kept: kept.to_vec(),
            dropped: dropped.to_vec(),
            scale,
            scale_inverses,
            native,
            program: compiled
                .program
                .ok_or(RnsChainAirError::Shape("compiled program missing"))?,
            counts,
            requirements,
            limits,
            comparator,
            auxiliary,
        })
    }

    /// Exact geometry and explicitly scoped payload/statement reservations.
    #[must_use]
    pub const fn requirements(&self) -> RnsChainRequirements {
        self.requirements
    }

    /// The same geometry exposed by the diagnostic semantic AIR wrapper.
    #[must_use]
    pub fn schema(&self) -> AirSchema {
        AirSchema {
            identity: IDENTITY,
            trace_rows: self.requirements.rows,
            width: self.requirements.width,
            constraints: self.requirements.constraints,
            numerator_degree_bound: self.requirements.numerator_degree_bound,
        }
    }

    fn kernel(&self) -> RnsKernel<'_> {
        RnsKernel {
            program: &self.program,
            counts: self.counts,
            requirements: self.requirements,
            source_primes: &self.dropped,
            first_digit: 2 * self.kept.len() + self.dropped.len(),
            comparator: self.comparator,
            auxiliary: self.auxiliary,
        }
    }

    fn check_input(&self, values: &[u64], primes: &[u64]) -> Result<(), RnsChainAirError> {
        check_input(values, primes, self.requirements.coefficients)
    }

    /// Build a clearing arithmetic witness using the shared native divide owner.
    ///
    /// Inputs are canonical and limb-major. The native output and mixed-radix
    /// digits are generated independently of the polynomial gate evaluator.
    /// This does not authenticate input ciphertexts or produce a committed proof.
    ///
    /// # Errors
    /// Rejects incorrect lengths, noncanonical coefficients or an internal public
    /// layout inconsistency. Partially filled owned buffers clear on any return.
    pub fn build_witness(
        &self,
        kept_input: &[u64],
        dropped_input: &[u64],
    ) -> Result<RnsChainWitness, RnsChainAirError> {
        self.check_input(kept_input, &self.kept)?;
        self.check_input(dropped_input, &self.dropped)?;
        let coefficients = self.requirements.coefficients;
        let mut output = Zeroizing::new(kept_input.to_vec());
        self.native
            .apply(&mut output, dropped_input, coefficients)?;
        let mut witness = RnsChainWitness {
            cells: Zeroizing::new(vec![0; self.requirements.trace_cells]),
            rows: self.requirements.rows,
            width: self.requirements.width,
        };
        let mut values = Zeroizing::new(vec![0_u64; self.counts.words]);
        let mut digits = Zeroizing::new([0_u64; MAX_BASIS_PRIMES]);
        let first_digit = 2 * self.kept.len() + self.dropped.len();
        let negative_word = first_digit + self.dropped.len();
        for coefficient in 0..coefficients {
            values.fill(0);
            for (limb, _) in self.kept.iter().enumerate() {
                values[limb] = kept_input[limb * coefficients + coefficient];
                values[self.kept.len() + self.dropped.len() + limb] =
                    output[limb * coefficients + coefficient];
            }
            for (limb, &prime) in self.dropped.iter().enumerate() {
                let value = dropped_input[limb * coefficients + coefficient];
                values[self.kept.len() + limb] = value;
                let fixed = FixedModulus::derive(prime)
                    .ok_or(RnsChainAirError::Shape("validated fixed modulus"))?;
                digits[limb] = fixed.multiply(value, self.scale_inverses[limb]);
            }
            self.native
                .dropped()
                .mixed_radix_in_place(&mut digits[..self.dropped.len()])?;
            for (index, &digit) in digits[..self.dropped.len()].iter().enumerate() {
                values[first_digit + index] = digit;
            }
            values[negative_word] = self
                .native
                .dropped()
                .exceeds_half_bit(&digits[..self.dropped.len()]);
            let row = witness
                .row_mut(coefficient)
                .ok_or(RnsChainAirError::Shape("owned row missing"))?;
            self.kernel().fill_row(&mut values, row)?;
        }
        Ok(witness)
    }

    /// Evaluate the reusable private-wire relation without publishing endpoints.
    ///
    /// Values must be canonical field openings. The diagnostic [`SemanticAir`]
    /// implementation checks this representation before calling the kernel.
    /// All arithmetic is polynomial in row cells, including active/padding cells.
    ///
    /// # Errors
    /// Rejects incorrect public slice geometry. Private cell values never choose
    /// an operation or trigger a host comparison in this evaluator.
    pub fn evaluate_row<F: IntegerAirField>(
        &self,
        row: &[F],
        residues: &mut [F],
    ) -> Result<(), RnsChainAirError> {
        self.kernel().evaluate_row(row, residues)
    }

    /// Bind public endpoints for the uncommitted diagnostic reference only.
    ///
    /// This publishes every supplied endpoint in the diagnostic statement. Future
    /// private composition must use [`Self::evaluate_row`] directly with its own
    /// authenticated private-wire bindings instead of this wrapper.
    ///
    /// # Errors
    /// Rejects noncanonical endpoints, incorrect lengths or bounded Norito errors.
    pub fn bind(
        self,
        kept_input: &[u64],
        dropped_input: &[u64],
        kept_output: &[u64],
    ) -> Result<RnsScaleDownAir, RnsChainAirError> {
        self.check_input(kept_input, &self.kept)?;
        self.check_input(dropped_input, &self.dropped)?;
        self.check_input(kept_output, &self.kept)?;
        let statement_fields = ScaleDownStatement {
            version: 1,
            kept_primes: self.kept.clone(),
            dropped_primes: self.dropped.clone(),
            scale: self.scale,
            coefficients: u64::try_from(self.requirements.coefficients)
                .map_err(|_| RnsChainAirError::Overflow)?,
            kept_input: kept_input.to_vec(),
            dropped_input: dropped_input.to_vec(),
            kept_output: kept_output.to_vec(),
        };
        let statement =
            norito::core::to_bytes_bounded(&statement_fields, self.requirements.statement_bytes)
                .map_err(|_| RnsChainAirError::Encoding)?;
        limit(
            "statement_bytes",
            statement.len(),
            self.limits.max_statement_bytes,
        )?;
        let ScaleDownStatement {
            kept_input,
            dropped_input,
            kept_output,
            kept_primes,
            dropped_primes,
            ..
        } = statement_fields;
        drop((kept_primes, dropped_primes));
        let endpoint_words = 2 * self.kept.len() + self.dropped.len();
        let public_columns = (0..=WORD_BITS * endpoint_words).collect();
        Ok(RnsScaleDownAir {
            plan: self,
            statement,
            public_columns,
            kept_input,
            dropped_input,
            kept_output,
        })
    }
}

/// Borrowed shared-word evaluator used by both active RNS relation plans.
struct RnsKernel<'a> {
    program: &'a Program,
    counts: Counts,
    requirements: RnsChainRequirements,
    source_primes: &'a [u64],
    first_digit: usize,
    comparator: usize,
    auxiliary: usize,
}

impl RnsKernel<'_> {
    fn fill_row(&self, values: &mut [u64], row: &mut [u64]) -> Result<(), RnsChainAirError> {
        if values.len() != self.counts.words || row.len() != self.requirements.width {
            return Err(RnsChainAirError::Shape("native shared row geometry"));
        }
        for &(word, value) in &self.program.constants {
            values[word] = value;
        }
        row[0] = 1;
        for (word, &value) in values.iter().enumerate() {
            put_word(row, word, value);
        }
        let mut equal = 1_u64;
        let mut greater = 0_u64;
        row[self.comparator] = equal;
        row[self.comparator + 1] = greater;
        let mut position = 0;
        for digit in (0..self.source_primes.len()).rev() {
            let half = (self.source_primes[digit] - 1) / 2;
            for bit in (0..WORD_BITS).rev() {
                let value_bit = (values[self.first_digit + digit] >> bit) & 1;
                let half_bit = (half >> bit) & 1;
                greater += equal * value_bit * (1 - half_bit);
                equal *= u64::from(value_bit == half_bit);
                position += 1;
                row[self.comparator + 2 * position] = equal;
                row[self.comparator + 2 * position + 1] = greater;
            }
        }
        for gate in &self.program.gates {
            let layout = gate.kind.layout();
            let start = self.auxiliary + gate.auxiliary;
            let value = integer::fill(
                gate.kind,
                gate.modulus,
                values[gate.left],
                gate.right.map(|word| values[word]),
                &mut row[start..start + layout.auxiliary_columns],
            )
            .map_err(|_| RnsChainAirError::Shape("native gate construction"))?;
            if let Some(word) = gate.output {
                values[word] = value;
                put_word(row, word, value);
            }
        }
        Ok(())
    }

    fn evaluate_row<F: IntegerAirField>(
        &self,
        row: &[F],
        residues: &mut [F],
    ) -> Result<(), RnsChainAirError> {
        if row.len() != self.requirements.width || residues.len() != self.requirements.constraints {
            return Err(RnsChainAirError::Shape("row or numerator extent"));
        }
        let active = row[0];
        for gate in &self.program.gates {
            let layout = gate.kind.layout();
            let auxiliary = self.auxiliary + gate.auxiliary;
            integer::evaluate(
                gate.kind,
                gate.modulus,
                active,
                WordViews {
                    left: word_view(row, gate.left)?,
                    right: gate.right.map(|word| word_view(row, word)).transpose()?,
                    output: gate.output.map(|word| word_view(row, word)).transpose()?,
                },
                &row[auxiliary..auxiliary + layout.auxiliary_columns],
                &mut residues[gate.residue..gate.residue + layout.constraint_count],
            )
            .map_err(|_| RnsChainAirError::Shape("polynomial gate geometry"))?;
        }
        let mut cursor = self.counts.gate_constraints;
        for &(word, value) in &self.program.constants {
            for (bit, &cell) in word_view(row, word)?.iter().enumerate() {
                residues[cursor] =
                    cell.sub(active.mul(F::from_u32(u32::from(((value >> bit) & 1) != 0))));
                cursor += 1;
            }
        }
        for &(left, right) in &self.program.equalities {
            for (&left, &right) in word_view(row, left)?.iter().zip(word_view(row, right)?) {
                residues[cursor] = left.sub(right);
                cursor += 1;
            }
        }
        residues[cursor] = active.mul(active.sub(F::ONE));
        residues[cursor + 1] = row[self.comparator].sub(active);
        residues[cursor + 2] = row[self.comparator + 1];
        cursor += 3;
        let mut position = 0;
        for digit in (0..self.source_primes.len()).rev() {
            let half = (self.source_primes[digit] - 1) / 2;
            for bit in (0..WORD_BITS).rev() {
                let old_equal = row[self.comparator + 2 * position];
                let old_greater = row[self.comparator + 2 * position + 1];
                position += 1;
                let equal = row[self.comparator + 2 * position];
                let greater = row[self.comparator + 2 * position + 1];
                let value = word_view(row, self.first_digit + digit)?[bit];
                let (match_bit, first_greater) = if (half >> bit) & 1 == 0 {
                    (F::ONE.sub(value), old_equal.mul(value))
                } else {
                    (value, F::ZERO)
                };
                residues[cursor] = equal.sub(old_equal.mul(match_bit));
                residues[cursor + 1] = greater.sub(old_greater.add(first_greater));
                residues[cursor + 2] = equal.mul(equal.sub(active));
                residues[cursor + 3] = greater.mul(greater.sub(active));
                cursor += 4;
            }
        }
        let negative = word_view(row, self.first_digit + self.source_primes.len())?;
        residues[cursor] = negative[0].sub(row[self.comparator + 2 * position + 1]);
        cursor += 1;
        for &cell in &negative[1..] {
            residues[cursor] = cell;
            cursor += 1;
        }
        if cursor != residues.len() {
            return Err(RnsChainAirError::Shape("polynomial numerator count"));
        }
        Ok(())
    }
}

fn check_input(
    values: &[u64],
    primes: &[u64],
    coefficients: usize,
) -> Result<(), RnsChainAirError> {
    if values.len() != multiply(primes.len(), coefficients)? {
        return Err(RnsChainAirError::Shape("limb-major coefficient extent"));
    }
    for (&prime, limb) in primes.iter().zip(values.chunks_exact(coefficients)) {
        if limb.iter().any(|&value| value >= prime) {
            return Err(RnsChainAirError::NonCanonical);
        }
    }
    Ok(())
}

fn put_word(row: &mut [u64], word: usize, value: u64) {
    let start = 1 + word * WORD_BITS;
    for (bit, cell) in row[start..start + WORD_BITS].iter_mut().enumerate() {
        *cell = (value >> bit) & 1;
    }
}

fn word_view<F>(row: &[F], word: usize) -> Result<&[F; WORD_BITS], RnsChainAirError> {
    let start = add(1, multiply(word, WORD_BITS)?)?;
    row.get(start..add(start, WORD_BITS)?)
        .and_then(|slice| slice.try_into().ok())
        .ok_or(RnsChainAirError::Shape("word column extent"))
}

/// Non-cloneable owned row buffer, cleared on success, error and unwind drops.
///
/// Borrowed rows remain caller-managed after copying. This owner makes no claim
/// about compiler/register copies or the separate clear reference artifact.
pub struct RnsChainWitness {
    cells: Zeroizing<Vec<u64>>,
    rows: usize,
    width: usize,
}

impl core::fmt::Debug for RnsChainWitness {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("RnsChainWitness")
            .field("rows", &self.rows)
            .field("width", &self.width)
            .finish_non_exhaustive()
    }
}

impl RnsChainWitness {
    /// Number of complete rows, including canonical zero padding.
    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }
    /// Exact columns per row.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }
    /// Borrow a complete row without transferring its clearing custody.
    #[must_use]
    pub fn row(&self, index: usize) -> Option<&[u64]> {
        if index >= self.rows {
            return None;
        }
        self.cells.get(index * self.width..(index + 1) * self.width)
    }
    /// Borrow an untrusted witness row for construction or adversarial checks.
    #[must_use]
    pub fn row_mut(&mut self, index: usize) -> Option<&mut [u64]> {
        if index >= self.rows {
            return None;
        }
        self.cells
            .get_mut(index * self.width..(index + 1) * self.width)
    }
}

impl Drop for RnsChainWitness {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(self);
        #[cfg(test)]
        retirement_tests::observe_retired(&self.cells);
    }
}

impl zeroize::Zeroize for RnsChainWitness {
    fn zeroize(&mut self) {
        self.cells.iter_mut().for_each(zeroize::Zeroize::zeroize);
    }
}

#[derive(norito::NoritoSerialize, norito::NoritoSchema)]
#[norito_schema(name = "fastpq_prover::rns_chain_air::ScaleDownStatementV1")]
struct ScaleDownStatement {
    version: u8,
    kept_primes: Vec<u64>,
    dropped_primes: Vec<u64>,
    scale: u64,
    coefficients: u64,
    kept_input: Vec<u64>,
    dropped_input: Vec<u64>,
    kept_output: Vec<u64>,
}

/// Public-endpoint wrapper for the existing non-hiding algebraic reference.
///
/// It is neither a committed proof nor a production input admission API. The
/// private arithmetic plan remains independently reusable without this wrapper.
#[derive(Debug)]
pub struct RnsScaleDownAir {
    plan: RnsScaleDownPlan,
    statement: Vec<u8>,
    public_columns: Vec<usize>,
    kept_input: Vec<u64>,
    dropped_input: Vec<u64>,
    kept_output: Vec<u64>,
}

impl RnsScaleDownAir {
    /// Borrow the reusable arithmetic plan.
    #[must_use]
    pub const fn plan(&self) -> &RnsScaleDownPlan {
        &self.plan
    }
}

fn semantic_error(error: RnsChainAirError) -> crate::Error {
    crate::Error::InvalidTraceShape {
        details: error.to_string(),
    }
}

impl SemanticAir for RnsScaleDownAir {
    fn schema(&self) -> AirSchema {
        self.plan.schema()
    }
    fn statement_bytes(&self) -> &[u8] {
        &self.statement
    }
    fn public_columns(&self) -> &[usize] {
        &self.public_columns
    }
    fn public_value(&self, ordinal: usize, row: usize) -> crate::Result<u64> {
        let column = *self
            .public_columns
            .get(ordinal)
            .ok_or_else(|| semantic_error(RnsChainAirError::Shape("public column ordinal")))?;
        if row >= self.plan.requirements.rows {
            return Err(semantic_error(RnsChainAirError::Shape(
                "public row ordinal",
            )));
        }
        if row >= self.plan.requirements.coefficients {
            return Ok(0);
        }
        if column == 0 {
            return Ok(1);
        }
        let word = (column - 1) / WORD_BITS;
        let bit = (column - 1) % WORD_BITS;
        let kept = self.plan.kept.len();
        let dropped = self.plan.dropped.len();
        let count = self.plan.requirements.coefficients;
        let value = if word < kept {
            self.kept_input[word * count + row]
        } else if word < kept + dropped {
            self.dropped_input[(word - kept) * count + row]
        } else {
            self.kept_output[(word - kept - dropped) * count + row]
        };
        Ok((value >> bit) & 1)
    }
    fn evaluate<F: PolynomialField>(
        &self,
        point: F,
        current: &[F],
        next: &[F],
    ) -> crate::Result<Vec<F>> {
        evaluate_reference(&self.plan.kernel(), point, current, next)
    }
}

fn evaluate_reference<F: PolynomialField>(
    kernel: &RnsKernel<'_>,
    point: F,
    current: &[F],
    next: &[F],
) -> crate::Result<Vec<F>> {
    if current.len() != kernel.requirements.width || next.len() != kernel.requirements.width {
        return Err(semantic_error(RnsChainAirError::Shape(
            "semantic row extent",
        )));
    }
    point.validate("rns_scale_down_point", &[])?;
    for (index, &value) in current.iter().enumerate() {
        value.validate("rns_scale_down_current", &[index])?;
    }
    for (index, &value) in next.iter().enumerate() {
        value.validate("rns_scale_down_next", &[index])?;
    }
    let mut result = vec![F::ZERO; kernel.requirements.constraints];
    kernel
        .evaluate_row(current, &mut result)
        .map_err(semantic_error)?;
    Ok(result)
}

#[cfg(test)]
mod retirement_tests {
    use super::*;
    use std::cell::Cell;

    #[derive(Clone, Copy, Default)]
    struct Observation {
        enabled: bool,
        owners: usize,
        all_zero: bool,
    }

    thread_local! {
        static OBSERVATION: Cell<Observation> = const { Cell::new(Observation { enabled: false, owners: 0, all_zero: false }) };
    }

    pub(super) fn observe_retired(cells: &[u64]) {
        OBSERVATION.with(|slot| {
            let mut observation = slot.get();
            if observation.enabled {
                observation.owners += 1;
                observation.all_zero &= cells.iter().all(|&cell| cell == 0);
                slot.set(observation);
            }
        });
    }

    fn limits() -> WorkLimits {
        WorkLimits {
            max_trace_rows: 8,
            max_trace_cells: 1_000_000,
            max_constraints: 1_000_000,
            max_statement_bytes: 1_000_000,
            max_payload_bytes: 100_000_000,
            max_work_units: 10_000_000,
        }
    }

    fn fail_with_witness(
        make: impl FnOnce() -> RnsChainWitness,
        failure: &'static str,
    ) -> Result<(), &'static str> {
        let mut witness = make();
        witness.row_mut(0).expect("owned row")[1] = 7;
        Err(failure)
    }

    #[test]
    fn owned_witness_clears_on_success_scope_error_and_unwind() {
        OBSERVATION.with(|slot| {
            slot.set(Observation {
                enabled: true,
                owners: 0,
                all_zero: true,
            })
        });
        let plan = RnsScaleDownPlan::new(&[5], &[3], 2, 1, limits()).expect("plan");
        drop(plan.build_witness(&[4], &[1]).expect("first witness"));
        let result = fail_with_witness(
            || plan.build_witness(&[4], &[1]).expect("error witness"),
            "deliberate scope error",
        );
        assert!(result.is_err());
        let result = std::panic::catch_unwind(|| {
            let mut witness = plan.build_witness(&[4], &[1]).expect("unwind witness");
            witness.row_mut(0).expect("owned row")[2] = 11;
            panic!("deliberate witness-owner unwind");
        });
        assert!(result.is_err());
        let observed = OBSERVATION.with(|slot| {
            let observation = slot.get();
            slot.set(Observation::default());
            observation
        });
        assert_eq!(observed.owners, 3);
        assert!(observed.all_zero);
    }

    #[test]
    fn private_witness_debug_reports_only_geometry() {
        let witness = RnsChainWitness {
            cells: Zeroizing::new(vec![12_345_678; 8]),
            rows: 2,
            width: 4,
        };
        let shown = format!("{witness:?}");
        assert!(shown.contains("rows: 2"));
        assert!(shown.contains("width: 4"));
        assert!(!shown.contains("12345678"));
        assert!(!shown.contains("cells"));
    }

    #[test]
    fn public_geometry_matches_independent_gate_and_shared_word_counts() {
        let small = RnsScaleDownPlan::new(&[5], &[3], 2, 1, limits()).expect("small plan");
        assert_eq!(small.requirements().width, 9_359);
        assert_eq!(small.requirements().constraints, 19_401);
        assert_eq!(small.counts.words, 24);
        let dropped = [
            3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83,
            89, 97, 101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179,
            181, 191, 193, 197, 199, 211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 269, 271,
            277, 281, 283, 293, 307, 311, 313,
        ];
        let large_limits = WorkLimits {
            max_trace_rows: 1,
            max_trace_cells: 5_000_000,
            max_constraints: 10_000_000,
            max_statement_bytes: 100_000,
            max_payload_bytes: 1_000_000_000,
            max_work_units: 20_000_000,
        };
        let large =
            RnsScaleDownPlan::new(&[331], &dropped, 2, 1, large_limits).expect("64-prime plan");
        assert_eq!(large.requirements().width, 4_107_068);
        assert_eq!(large.requirements().constraints, 8_654_559);
        assert_eq!(large.counts.words, 8_907);
    }

    #[test]
    fn centered_owner_reuses_success_error_and_unwind_clearing() {
        OBSERVATION.with(|slot| {
            slot.set(Observation {
                enabled: true,
                owners: 0,
                all_zero: true,
            })
        });
        let plan =
            RnsCenteredExtensionPlan::new(&[3, 5], &[7, 9], 1, limits()).expect("extension plan");
        drop(plan.build_witness(&[1, 4]).expect("extension witness"));
        let error = fail_with_witness(
            || plan.build_witness(&[1, 4]).expect("error owner"),
            "deliberate extension scope error",
        );
        assert!(error.is_err());
        let unwind = std::panic::catch_unwind(|| {
            let mut witness = plan.build_witness(&[1, 4]).expect("unwind owner");
            witness.row_mut(0).expect("row")[2] = 11;
            panic!("deliberate extension unwind");
        });
        assert!(unwind.is_err());
        let observed = OBSERVATION.with(|slot| {
            let found = slot.get();
            slot.set(Observation::default());
            found
        });
        assert_eq!(observed.owners, 3);
        assert!(observed.all_zero);
    }

    #[test]
    fn centered_geometry_matches_independent_counts_and_shared_gate_layouts() {
        let small =
            RnsCenteredExtensionPlan::new(&[3], &[5], 1, limits()).expect("small extension");
        assert_eq!(small.requirements().width, 5_827);
        assert_eq!(small.requirements().constraints, 12_114);
        let source = [
            3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83,
            89, 97, 101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179,
            181, 191, 193, 197, 199, 211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 269, 271,
            277, 281, 283, 293, 307, 311, 313,
        ];
        let large_limits = WorkLimits {
            max_trace_rows: 1,
            max_trace_cells: 5_000_000,
            max_constraints: 10_000_000,
            max_statement_bytes: 100_000,
            max_payload_bytes: 1_000_000_000,
            max_work_units: 20_000_000,
        };
        let large = RnsCenteredExtensionPlan::new(&source, &[331], 1, large_limits)
            .expect("64-prime centered extension");
        assert_eq!(large.requirements().width, 4_037_701);
        assert_eq!(large.requirements().constraints, 8_517_870);
    }
}
