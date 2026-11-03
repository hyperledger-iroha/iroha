//! Centered basis extension using the same private RNS word/gate owner as division.

use core::mem::size_of;

use iroha_fhe::{
    constant_time::FixedModulus,
    modular::mul_mod_u64,
    rns_chain::{BASIS_PRIME_BOUND, ConversionTarget, MAX_BASIS_PRIMES, RnsBasis},
};
use zeroize::Zeroizing;

use super::{
    Builder, Counts, Geometry, Program, RnsChainAirError, RnsChainRequirements, RnsChainWitness,
    RnsKernel, WORD_BITS, add, check_input, evaluate_reference,
    integer::{self, GateKind},
    limit, multiply, semantic_error,
};
use crate::air::{AirSchema, IntegerAirField, PolynomialField, SemanticAir, WorkLimits};

const EXTENSION_IDENTITY: &str = "fastpq:rns-centered-extension:v1";

fn extension_counts(source: usize, targets: usize) -> Result<Counts, RnsChainAirError> {
    let triangle = multiply(source, add(source, 1)?)? / 2;
    let terms = add(triangle, multiply(source, targets)?)?;
    let mut counts = Counts {
        words: add(
            add(multiply(4, terms)?, multiply(3, source)?)?,
            add(multiply(5, targets)?, 1)?,
        )?,
        constants: add(add(terms, source)?, multiply(2, targets)?)?,
        equalities: add(source, targets)?,
        ..Counts::default()
    };
    for (kind, number) in [
        (GateKind::Canonical, add(multiply(2, source)?, targets)?),
        (GateKind::Reduce, terms),
        (GateKind::MulMod, add(terms, targets)?),
        (GateKind::AddMod, terms),
        (GateKind::SubMod, targets),
    ] {
        let layout = kind.layout();
        counts.gates = add(counts.gates, number)?;
        counts.auxiliary = add(
            counts.auxiliary,
            multiply(number, layout.auxiliary_columns)?,
        )?;
        counts.gate_constraints = add(
            counts.gate_constraints,
            multiply(number, layout.constraint_count)?,
        )?;
    }
    Ok(counts)
}

fn compile_extension(
    source: &[u64],
    targets: &[u64],
    counts: Counts,
) -> Result<Program, RnsChainAirError> {
    let mut builder = Builder {
        counts: Counts::default(),
        program: Some(Program {
            gates: Vec::with_capacity(counts.gates),
            constants: Vec::with_capacity(counts.constants),
            equalities: Vec::with_capacity(counts.equalities),
        }),
    };
    for _ in 0..add(source.len(), targets.len())? {
        builder.word()?;
    }
    let first_digit = builder.counts.words;
    for _ in source {
        builder.word()?;
    }
    let negative = builder.word()?;
    for (index, &prime) in source.iter().enumerate() {
        builder.gate(GateKind::Canonical, prime, index, None)?;
        builder.gate(GateKind::Canonical, prime, first_digit + index, None)?;
        let recovered = builder.reconstruct(source, prime, first_digit, index + 1)?;
        builder.equal(recovered, index)?;
    }
    for (index, &target) in targets.iter().enumerate() {
        let output = source.len() + index;
        builder.gate(GateKind::Canonical, target, output, None)?;
        let value = builder.reconstruct(source, target, first_digit, source.len())?;
        let product = source.iter().fold(1_u64, |value, &prime| {
            mul_mod_u64(value, prime % target, target)
        });
        let product_word = builder.constant(product)?;
        let correction = builder.gate(GateKind::MulMod, target, negative, Some(product_word))?;
        let centered = builder.gate(GateKind::SubMod, target, value, Some(correction))?;
        builder.equal(centered, output)?;
    }
    if builder.counts != counts {
        return Err(RnsChainAirError::Shape(
            "extension public compilation count changed",
        ));
    }
    builder
        .program
        .ok_or(RnsChainAirError::Shape("extension program missing"))
}

fn extension_native_heap_bytes(source: usize, targets: usize) -> Result<usize, RnsChainAirError> {
    // Exact native RnsBasis and ConversionTarget Vec extents. Their inline
    // headers belong to the plan or the charged targets Vec elements.
    let triangle = multiply(source, source.saturating_sub(1))? / 2;
    let basis = add(
        multiply(source, size_of::<FixedModulus>() + size_of::<Vec<u64>>())?,
        multiply(add(triangle, multiply(3, source)?)?, size_of::<u64>())?,
    )?;
    let converted = add(
        multiply(targets, size_of::<ConversionTarget>())?,
        multiply(multiply(source, targets)?, size_of::<u64>())?,
    )?;
    add(basis, converted)
}

/// Private-wire centered basis extension with public dimensions and caller caps.
///
/// Source residues must be canonical. Ordered source primes are admitted by
/// [`RnsBasis`]. Targets may repeat or overlap the source, and may be composite,
/// but must be odd and below `2^62` for the shared bounded integer gates. Native
/// conversion additionally supports unreduced inputs and targets up to `2^63`;
/// those larger domains are outside this relation.
#[derive(Debug)]
pub struct RnsCenteredExtensionPlan {
    source: Vec<u64>,
    targets: Vec<u64>,
    native: RnsBasis,
    native_targets: Vec<ConversionTarget>,
    program: Program,
    counts: Counts,
    requirements: RnsChainRequirements,
    limits: WorkLimits,
    comparator: usize,
    auxiliary: usize,
}

impl RnsCenteredExtensionPlan {
    /// Prepare the exact centered conversion relation before any coefficient read.
    ///
    /// Target cardinality is governed by checked geometry and caller limits.
    /// Source cardinality follows the native owner's maximum of 64 primes.
    ///
    /// # Errors
    /// Rejects empty bases/targets, zero coefficient count, invalid parameters,
    /// checked geometry overflow or any exceeded public caller ceiling.
    pub fn new(
        source: &[u64],
        targets: &[u64],
        coefficients: usize,
        limits: WorkLimits,
    ) -> Result<Self, RnsChainAirError> {
        if source.is_empty()
            || source.len() > MAX_BASIS_PRIMES
            || targets.is_empty()
            || coefficients == 0
        {
            return Err(RnsChainAirError::Shape(
                "nonempty bounded source basis, targets and coefficient rows required",
            ));
        }
        let counts = extension_counts(source.len(), targets.len())?;
        let endpoints = add(source.len(), targets.len())?;
        let geometry = Geometry::new(counts, source.len(), endpoints, endpoints, coefficients)?;
        let native_outputs = multiply(multiply(coefficients, targets.len())?, size_of::<u64>())?;
        let scratch = add(
            multiply(add(counts.words, MAX_BASIS_PRIMES)?, size_of::<u64>())?,
            add(
                add(native_outputs, multiply(2, size_of::<Vec<u64>>())?)?,
                integer::NATIVE_SCRATCH_BYTES,
            )?,
        )?;
        let parameters = multiply(endpoints, size_of::<u64>())?;
        let diagnostic_scratch = add(size_of::<ExtensionStatement>(), parameters)?;
        let payload = add(
            diagnostic_scratch,
            add(
                add(
                    add(
                        size_of::<RnsCenteredExtensionAir>(),
                        size_of::<RnsChainWitness>(),
                    )?,
                    geometry.program_bytes,
                )?,
                add(
                    add(
                        parameters,
                        extension_native_heap_bytes(source.len(), targets.len())?,
                    )?,
                    add(
                        add(multiply(geometry.trace_cells, size_of::<u64>())?, scratch)?,
                        add(
                            add(
                                geometry.statement_bytes,
                                multiply(geometry.endpoint_words, size_of::<u64>())?,
                            )?,
                            geometry.public_columns,
                        )?,
                    )?,
                )?,
            )?,
        )?;
        let requirements = geometry.admit(payload, scratch, limits)?;
        let native = RnsBasis::new(source)?;
        let mut native_targets = Vec::with_capacity(targets.len());
        for &target in targets {
            if target >= BASIS_PRIME_BOUND {
                return Err(RnsChainAirError::Shape(
                    "centered target must be below the shared integer bound 2^62",
                ));
            }
            native_targets.push(native.conversion_target(target)?);
        }
        let program = compile_extension(source, targets, counts)?;
        Ok(Self {
            source: source.to_vec(),
            targets: targets.to_vec(),
            native,
            native_targets,
            program,
            counts,
            requirements,
            limits,
            comparator: geometry.comparator,
            auxiliary: geometry.auxiliary,
        })
    }

    /// Exact shared geometry and explicitly scoped structural reservations.
    #[must_use]
    pub const fn requirements(&self) -> RnsChainRequirements {
        self.requirements
    }

    /// Trusted geometry of the separately bound diagnostic semantic relation.
    #[must_use]
    pub fn schema(&self) -> AirSchema {
        AirSchema {
            identity: EXTENSION_IDENTITY,
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
            source_primes: &self.source,
            first_digit: self.source.len() + self.targets.len(),
            comparator: self.comparator,
            auxiliary: self.auxiliary,
        }
    }

    /// Build a clearing witness using the native centered conversion owner.
    ///
    /// Inputs and outputs are limb-major. Native outputs are generated before
    /// the shared polynomial-gate trace; no committed proof or input/key
    /// authentication is implied by this local witness.
    ///
    /// # Errors
    /// Rejects wrong source extent, noncanonical residues or internal geometry.
    /// Every owned private buffer clears on error and unwind as well as success.
    pub fn build_witness(&self, source: &[u64]) -> Result<RnsChainWitness, RnsChainAirError> {
        check_input(source, &self.source, self.requirements.coefficients)?;
        let coefficients = self.requirements.coefficients;
        let mut output = Zeroizing::new(vec![0_u64; coefficients * self.targets.len()]);
        self.native
            .convert_centered(source, coefficients, &self.native_targets, &mut output)?;
        let mut witness = RnsChainWitness {
            cells: Zeroizing::new(vec![0; self.requirements.trace_cells]),
            rows: self.requirements.rows,
            width: self.requirements.width,
        };
        let mut values = Zeroizing::new(vec![0_u64; self.counts.words]);
        let mut digits = Zeroizing::new([0_u64; MAX_BASIS_PRIMES]);
        let first_digit = self.source.len() + self.targets.len();
        for coefficient in 0..coefficients {
            values.fill(0);
            for limb in 0..self.source.len() {
                let value = source[limb * coefficients + coefficient];
                values[limb] = value;
                digits[limb] = value;
            }
            for limb in 0..self.targets.len() {
                values[self.source.len() + limb] = output[limb * coefficients + coefficient];
            }
            self.native
                .mixed_radix_in_place(&mut digits[..self.source.len()])?;
            values[first_digit..first_digit + self.source.len()]
                .copy_from_slice(&digits[..self.source.len()]);
            values[first_digit + self.source.len()] =
                self.native.exceeds_half_bit(&digits[..self.source.len()]);
            let row = witness
                .row_mut(coefficient)
                .ok_or(RnsChainAirError::Shape("extension owned row missing"))?;
            self.kernel().fill_row(&mut values, row)?;
        }
        Ok(witness)
    }

    /// Evaluate the shared private-wire polynomial kernel, without publishing inputs.
    ///
    /// # Errors
    /// Rejects a wrong public row or numerator extent before reading cells.
    pub fn evaluate_row<F: IntegerAirField>(
        &self,
        row: &[F],
        residues: &mut [F],
    ) -> Result<(), RnsChainAirError> {
        self.kernel().evaluate_row(row, residues)
    }

    /// Bind public endpoints for the uncommitted reference, separately from the kernel.
    ///
    /// This intentionally publishes the supplied endpoints. Private composition
    /// instead uses shared wire columns with [`Self::evaluate_row`].
    ///
    /// # Errors
    /// Rejects noncanonical endpoints, wrong extents or bounded Norito encoding.
    pub fn bind(
        self,
        source: &[u64],
        output: &[u64],
    ) -> Result<RnsCenteredExtensionAir, RnsChainAirError> {
        check_input(source, &self.source, self.requirements.coefficients)?;
        check_input(output, &self.targets, self.requirements.coefficients)?;
        let fields = ExtensionStatement {
            version: 1,
            source_primes: self.source.clone(),
            target_moduli: self.targets.clone(),
            coefficients: u64::try_from(self.requirements.coefficients)
                .map_err(|_| RnsChainAirError::Overflow)?,
            source: source.to_vec(),
            output: output.to_vec(),
        };
        let statement = norito::core::to_bytes_bounded(&fields, self.requirements.statement_bytes)
            .map_err(|_| RnsChainAirError::Encoding)?;
        limit(
            "statement_bytes",
            statement.len(),
            self.limits.max_statement_bytes,
        )?;
        let ExtensionStatement {
            source,
            output,
            source_primes,
            target_moduli,
            ..
        } = fields;
        drop((source_primes, target_moduli));
        let public_columns = (0..=WORD_BITS * (self.source.len() + self.targets.len())).collect();
        Ok(RnsCenteredExtensionAir {
            plan: self,
            statement,
            public_columns,
            source,
            output,
        })
    }
}

#[derive(norito::NoritoSerialize, norito::NoritoSchema)]
#[norito_schema(name = "fastpq_prover::rns_chain_air::CenteredExtensionStatementV1")]
struct ExtensionStatement {
    version: u8,
    source_primes: Vec<u64>,
    target_moduli: Vec<u64>,
    coefficients: u64,
    source: Vec<u64>,
    output: Vec<u64>,
}

/// Public diagnostic wrapper; it neither hides its trace nor produces a STARK proof.
#[derive(Debug)]
pub struct RnsCenteredExtensionAir {
    plan: RnsCenteredExtensionPlan,
    statement: Vec<u8>,
    public_columns: Vec<usize>,
    source: Vec<u64>,
    output: Vec<u64>,
}

impl RnsCenteredExtensionAir {
    /// Borrow the reusable private arithmetic plan.
    #[must_use]
    pub const fn plan(&self) -> &RnsCenteredExtensionPlan {
        &self.plan
    }
}

impl SemanticAir for RnsCenteredExtensionAir {
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
        let column = *self.public_columns.get(ordinal).ok_or_else(|| {
            semantic_error(RnsChainAirError::Shape("extension public column ordinal"))
        })?;
        if row >= self.plan.requirements.rows {
            return Err(semantic_error(RnsChainAirError::Shape(
                "extension public row ordinal",
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
        let count = self.plan.requirements.coefficients;
        let value = if word < self.plan.source.len() {
            self.source[word * count + row]
        } else {
            self.output[(word - self.plan.source.len()) * count + row]
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
