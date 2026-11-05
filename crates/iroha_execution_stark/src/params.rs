//! Proof parameters and their soundness accounting.
//!
//! Security uses the Johnson-regime bound for DEEP-FRI, never the "up to capacity" conjecture,
//! which Diamond–Gruen (ePrint 2025/2010) and Crites–Stewart (ePrint 2025/2046) refuted in 2025.
//!
//! - Query phase (BCIKS20 Thm 8.3, ethSTARK v1.2 eq. 10; Haböck ePrint 2022/1216 Thm 8): each
//!   query lets a cheating prover through with probability at most `sqrt(rho+) * (1 + 1/(2m))`,
//!   where `rho+ = (rows + 2) / lde_size` accounts for the DEEP quotients.
//! - Commit phase: base term `(m + 1/2)^7 N^2 / (2 rho^(3/2) |F|)` (BCIKS20; the linear-`N`
//!   bound of BCHKS25, ePrint 2025/2055, is tighter). At the standard blowup 8 and `N <= 2^25`,
//!   this base term is below `2^-153`. The estimator also accounts for DEEP batch width, folding
//!   arity and round count; the base-term example is not a bound for every supported parameter set.
//! - DEEP/ALI: `L+ (C + d (k+2) + k) / |F|` with list size `L+ = (m + 1/2)/sqrt(rho+)`.
//! - LogUp (Haböck ePrint 2022/1530): `(lookups * rows + table) / |F|`.
//!
//! Each round must stay below `2^-(lambda+1)` for `lambda` bits after Fiat–Shamir (ethSTARK v1.2
//! Cor. 3), so queries are sized for 129 bits. Native 384-bit BLAKE2b digests give the BCS hash
//! term `3(T^2+1)/2^384`, imposing a 190.5-bit hash ceiling; the full budget is the minimum of
//! that ceiling and the circuit-specific algebraic terms. This is a classical random-oracle bound.
//! Grinding is applied before query sampling but never counted.

/// Proof-system parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Params {
    /// `log2` of the low-degree-extension blowup (rate `2^-log_blowup`).
    pub log_blowup: u32,
    /// FRI queries.
    pub queries: usize,
    /// `log2` of the FRI folding arity.
    pub fri_log_arity: u32,
    /// Fold until the degree bound is at most `2^fri_log_final_degree`, then send coefficients.
    pub fri_log_final_degree: u32,
    /// Proof-of-work bits before query sampling (not counted toward security).
    pub grinding_bits: u32,
}

/// Target security level in bits.
pub const TARGET_SECURITY_BITS: f64 = 128.0;
/// Hash output bits used as `kappa` in ethSTARK v1.2 Corollary 3, not full-proof security bits.
/// Native BLAKE2b-384 uses a 512-bit chaining state and has a 192-bit collision-security target.
pub const HASH_SECURITY_BITS: usize = crate::hash::DIGEST_BYTES * 8;
/// Proximity parameter `m` of the Johnson-regime bound (`eta = sqrt(rho)/(2m)`).
pub const JOHNSON_M: f64 = 128.0;
/// Per-round target so that Fiat–Shamir keeps `TARGET_SECURITY_BITS` (ethSTARK v1.2 Cor. 3).
pub const PER_ROUND_TARGET_BITS: f64 = TARGET_SECURITY_BITS + 1.0;
/// `log2 |Fp4|`.
pub const EXTENSION_FIELD_BITS: f64 = 255.99;

/// Why a parameter set cannot be used with a circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParamsError {
    /// A parameter value is outside the supported range.
    #[error("parameter value outside the supported range")]
    Unsupported,
    /// The circuit's trace height, table width or helper width is outside the supported range, or
    /// it has too many lookups for exact multiplicities below the field characteristic.
    #[error("circuit dimensions outside the supported range")]
    Circuit,
    /// More queries than points in the low-degree extension.
    #[error("more queries than evaluation points")]
    Queries,
    /// The blowup is smaller than the quotient domain the constraint degree needs.
    #[error("blowup too small for the constraint degree")]
    QuotientBlowup,
    /// A FRI fold needs more points than its layer's codeword has.
    #[error("FRI folding exceeds a layer's domain")]
    FoldGeometry,
    /// Opened rows or the worst-case decoded element count exceed the proof codec's limits.
    #[error("proof dimensions exceed the codec's limits")]
    Encoding,
    /// A size or count does not fit the platform's integers.
    #[error("proof size or lookup count overflow")]
    Overflow,
}

/// Why a circuit and parameter set do not meet a requested soundness budget.
///
/// Compatibility and release qualification remain separate: passing this check is necessary for
/// a caller's security target, but does not certify the circuit, implementation or release evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SecurityError {
    /// The parameters and circuit fail the ordinary compatibility checks.
    #[error(transparent)]
    Parameters(#[from] ParamsError),
    /// The requested target is not finite and strictly positive.
    #[error("security target must be finite and positive")]
    InvalidTarget,
    /// The circuit's computed non-interactive budget is below the target or is not finite.
    #[error("proof parameters do not meet the requested security target")]
    InsufficientSecurity,
}

/// Lookup occurrences over every row, padding included (the exact sum of the multiplicities).
///
/// # Errors
/// Rejects counts that overflow or reach the field characteristic.
pub fn lookup_occurrences(circuit: &crate::Circuit) -> Result<u64, ParamsError> {
    let rows = 1u64
        .checked_shl(circuit.log_rows)
        .filter(|_| circuit.log_rows < 64)
        .ok_or(ParamsError::Overflow)?;
    let lookups = u64::try_from(circuit.lookups.len()).map_err(|_| ParamsError::Overflow)?;
    let total = rows.checked_mul(lookups).ok_or(ParamsError::Overflow)?;
    if total >= crate::field::MODULUS {
        return Err(ParamsError::Circuit);
    }
    Ok(total)
}

/// The dimensions of a circuit that enter its soundness bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SecurityShape {
    /// `log2` of the trace height.
    pub log_rows: u32,
    /// Maximum identity degree (circuit and LogUp helper groups).
    pub max_degree: u32,
    /// Identities combined in the composition (circuit, boundary and LogUp).
    pub identities: usize,
    /// Lookups per row.
    pub lookups: usize,
    /// `log2` of the range-table size.
    pub table_bits: u32,
    /// Polynomials combined in the DEEP batch.
    pub batched: usize,
}

impl SecurityShape {
    /// The shape of a circuit.
    pub fn of(circuit: &crate::Circuit) -> Self {
        let shape = crate::air::quotient_shape(circuit);
        let has_lookups = !circuit.lookups.is_empty();
        let aux = if has_lookups {
            circuit.helper_count() + 1
        } else {
            0
        };
        Self {
            log_rows: circuit.log_rows,
            max_degree: shape.low_degree.max(shape.high_degree),
            identities: circuit.constraints.len()
                + 3 * circuit.registers.len()
                + if has_lookups {
                    circuit.helper_count() + 1
                } else {
                    0
                },
            lookups: circuit.lookups.len(),
            table_bits: circuit.range_bits,
            batched: circuit.columns.len()
                + aux
                + shape.chunks()
                + circuit.next_columns().len()
                + usize::from(has_lookups),
        }
    }
}

impl Params {
    /// Checks these parameters against a circuit before any proving or verification work. Scalar
    /// ranges come first, so no later step shifts or allocates with an unchecked value; then the
    /// evaluation domain against the queries, the quotient domain against the blowup, every FRI fold
    /// against its codeword, opened-row widths and the worst-case element count against the codec,
    /// and the lookup total against the characteristic. Every supported query layout must fit the
    /// decoding budget, even if some more compact individual proofs could otherwise fit.
    ///
    /// The circuit itself is a trusted, builder-produced AIR; this does not re-check its compiler
    /// assertions or variable indices.
    ///
    /// # Errors
    /// Returns the first violated condition.
    pub fn check(&self, circuit: &crate::Circuit) -> Result<(), ParamsError> {
        if !self.is_supported() {
            return Err(ParamsError::Unsupported);
        }
        if !(8..=22).contains(&circuit.log_rows)
            || !(8..=16).contains(&circuit.range_bits)
            || !(1..=4).contains(&circuit.lookups_per_helper)
        {
            return Err(ParamsError::Circuit);
        }
        let log_lde = circuit.log_rows + self.log_blowup;
        if log_lde > crate::field::TWO_ADICITY {
            return Err(ParamsError::Unsupported);
        }
        if self.queries > 1usize << log_lde {
            return Err(ParamsError::Queries);
        }
        let shape = crate::air::quotient_shape(circuit);
        let quotient_log =
            crate::air::quotient_log_blowup(shape.low_degree).max(if shape.high_chunks() > 0 {
                crate::air::quotient_log_blowup(shape.high_degree)
            } else {
                0
            });
        if quotient_log > self.log_blowup {
            return Err(ParamsError::QuotientBlowup);
        }
        // Track the codeword size separately from the degree bound: a last fold may round the
        // degree down to a constant but still needs `arity` codeword entries.
        let (mut log_degree, mut log_codeword) = (circuit.log_rows, log_lde);
        while log_degree > self.fri_log_final_degree {
            log_codeword = log_codeword
                .checked_sub(self.fri_log_arity)
                .ok_or(ParamsError::FoldGeometry)?;
            log_degree = log_degree.saturating_sub(self.fri_log_arity);
        }
        let aux_width = if circuit.lookups.is_empty() {
            0
        } else {
            (circuit.helper_count() + 1) * 4
        };
        if [circuit.columns.len(), aux_width, shape.chunks() * 4]
            .into_iter()
            .any(|width| width > crate::proof::MAX_WIDTH)
        {
            return Err(ParamsError::Encoding);
        }
        lookup_occurrences(circuit)?;
        if crate::proof::max_proof_elements(circuit, self) > u128::from(crate::proof::MAX_ELEMENTS)
        {
            return Err(ParamsError::Encoding);
        }
        Ok(())
    }

    /// Checks compatibility and the circuit-specific classical soundness budget against a target.
    ///
    /// Production callers should pass their immutable profile's target (for RaceV1,
    /// [`TARGET_SECURITY_BITS`]). This does not change [`Self::check`], which also supports
    /// explicitly chosen low-security parameters for development, and does not grant release
    /// qualification.
    ///
    /// # Errors
    /// Rejects incompatible parameters, a nonfinite or nonpositive target, and a budget below the
    /// target. A nonfinite computed budget is rejected rather than treated as passing.
    pub fn check_security(
        &self,
        circuit: &crate::Circuit,
        target_bits: f64,
    ) -> Result<(), SecurityError> {
        self.check(circuit)?;
        if !target_bits.is_finite() || target_bits <= 0.0 {
            return Err(SecurityError::InvalidTarget);
        }
        let bits = self.hash_limited_security_bits(&SecurityShape::of(circuit));
        if !bits.is_finite() || bits < target_bits {
            return Err(SecurityError::InsufficientSecurity);
        }
        Ok(())
    }

    /// Blowup 8, arity-8 folding, queries targeting 129 raw query-phase bits, 20 grinding bits.
    /// The full soundness budget also depends on the circuit's [`SecurityShape`].
    pub fn standard() -> Self {
        let mut params = Self {
            log_blowup: 3,
            queries: 0,
            fri_log_arity: 3,
            fri_log_final_degree: 4,
            grinding_bits: 20,
        };
        params.queries = params.queries_for(PER_ROUND_TARGET_BITS, 8);
        params
    }

    /// Whether prover, verifier and the proof codec all support these parameters: blowup 2..=64,
    /// arity 2..=16, 1..=1024 queries, final degree at most 2^10 and at most 32 grinding bits.
    pub fn is_supported(&self) -> bool {
        (1..=6).contains(&self.log_blowup)
            && (1..=4).contains(&self.fri_log_arity)
            && (1..=crate::proof::MAX_ROWS).contains(&self.queries)
            && self.fri_log_final_degree <= 10
            && self.grinding_bits <= 32
    }

    /// `rho+ = (rows + 2) / lde_size` for a trace of `2^log_rows` rows.
    pub fn effective_rate(&self, log_rows: u32) -> f64 {
        let rows = 2f64.powi(i32::try_from(log_rows).expect("small"));
        (rows + 2.0) / (rows * 2f64.powi(i32::try_from(self.log_blowup).expect("small")))
    }

    /// Bits contributed by each query: `-log2(sqrt(rho+) * (1 + 1/(2m)))`.
    pub fn bits_per_query(&self, log_rows: u32) -> f64 {
        -(self.effective_rate(log_rows).sqrt() * (1.0 + 1.0 / (2.0 * JOHNSON_M))).log2()
    }

    /// Queries needed for `bits` of query-phase security at `2^log_rows` rows (the smallest
    /// supported trace is the worst case).
    pub fn queries_for(&self, bits: f64, log_rows: u32) -> usize {
        (bits / self.bits_per_query(log_rows)).ceil() as usize
    }

    /// The reported soundness budget `lambda` (in bits) for a circuit of this shape: the weakest
    /// per-round term minus the one-bit margin that ethSTARK v1.2 Cor. 3 requires (each round below
    /// `2^-(lambda+1)`). At the smallest trace the standard query term is 129.522 bits before
    /// this margin, so it contributes 128.522 bits; another term can lower the reported budget.
    ///
    /// The commit term carries the DEEP batch width (Haböck 2022/1216 Thm 2: factor `L - 1/2`) and
    /// the LogUp term counts every lookup of every row plus the table.
    #[allow(clippy::cast_precision_loss)] // counts are far below 2^52
    pub fn proven_security_bits(&self, shape: &SecurityShape) -> f64 {
        let log_rows = shape.log_rows;
        let log_lde = f64::from(log_rows + self.log_blowup);
        let rho = 2f64.powi(-i32::try_from(self.log_blowup).expect("small"));
        let rho_plus = self.effective_rate(log_rows);
        let query = self.bits_per_query(log_rows) * self.queries as f64;
        // Commit phase: per folding round, (m + 1/2)^7 / (2 rho^1.5) * N^2 / |F|, times arity - 1
        // for power batching, the number of rounds and the batch width.
        let rounds = f64::from(log_rows.saturating_sub(self.fri_log_final_degree))
            / f64::from(self.fri_log_arity.max(1))
            + 1.0;
        let arity = 2f64.powi(i32::try_from(self.fri_log_arity).expect("small"));
        let gap_log = 7.0f64.mul_add((JOHNSON_M + 0.5).log2(), -(2.0 * rho.powf(1.5)).log2());
        let commit_log = 2.0f64.mul_add(log_lde, gap_log)
            + (arity - 1.0).max(1.0).log2()
            + rounds.log2()
            + (shape.batched as f64).max(1.0).log2();
        let commit = EXTENSION_FIELD_BITS - commit_log;
        // DEEP/ALI: L+ (C + d (k+2) + k) / |F| with L+ = (m + 1/2)/sqrt(rho+).
        let rows = 2f64.powi(i32::try_from(log_rows).expect("small"));
        let list_log = ((JOHNSON_M + 0.5) / rho_plus.sqrt()).log2();
        let ali_log = (f64::from(shape.max_degree).mul_add(rows + 2.0, shape.identities as f64)
            + rows)
            .log2()
            + list_log;
        let ali = EXTENSION_FIELD_BITS - ali_log;
        // LogUp: (lookups * rows + table) / |F|.
        let table = 2f64.powi(i32::try_from(shape.table_bits).expect("small"));
        let logup = EXTENSION_FIELD_BITS - (shape.lookups as f64).mul_add(rows, table).log2();
        query.min(commit).min(ali).min(logup) - 1.0
    }

    /// [`Self::proven_security_bits`] capped by the hash condition of ethSTARK v1.2 Cor. 3 (digest
    /// bits `>= 2 lambda + 3`). Native 384-bit BLAKE2b digests impose a 190.5-bit ceiling; this
    /// ceiling is not the full-proof security claim, which also depends on the circuit's shape.
    pub fn hash_limited_security_bits(&self, shape: &SecurityShape) -> f64 {
        let hash_bits =
            f64::from(u32::try_from(HASH_SECURITY_BITS).expect("fixed digest bit count"));
        self.proven_security_bits(shape)
            .min((hash_bits - 3.0) / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_parameters_fit_the_total_decoder_budget() {
        // Build only AIR metadata, never a wide trace or proof. Each materialized constant is a
        // valid constrained column, so this is builder-produced and all individual widths fit.
        let mut builder = crate::Builder::new("decoder budget", 8);
        for _ in 0..crate::proof::MAX_WIDTH {
            builder.materialize(&crate::Expr::constant(0), "zero");
        }
        let circuit = builder.finish();
        assert_eq!(circuit.columns.len(), crate::proof::MAX_WIDTH);
        let wide_queries = Params {
            queries: crate::proof::MAX_ROWS,
            grinding_bits: 0,
            ..Params::standard()
        };
        // Main openings alone consume the entire budget; OOD data and FRI need more elements.
        assert_eq!(
            wide_queries.queries as u128 * circuit.columns.len() as u128,
            u128::from(crate::proof::MAX_ELEMENTS)
        );
        assert_eq!(wide_queries.check(&circuit), Err(ParamsError::Encoding));
        assert_eq!(
            crate::max_proof_bytes(&circuit, &wide_queries),
            Err(ParamsError::Encoding)
        );
        // The ordinary profile still supports this width.
        assert_eq!(Params::standard().check(&circuit), Ok(()));
    }

    #[test]
    fn standard_parameters_reach_128_proven_bits() {
        let params = Params::standard();
        // Matches the Johnson-regime table (blowup 8: 87 queries per ethSTARK Cor. 3).
        assert_eq!(params.queries, 87);
        for log_rows in 8..=22 {
            let shape = SecurityShape {
                log_rows,
                max_degree: 5,
                identities: 100_000,
                lookups: 4_000,
                table_bits: 16,
                batched: 10_000,
            };
            let bits = params.proven_security_bits(&shape);
            assert!(bits >= 128.0, "log_rows {log_rows}: {bits}");
            assert!(params.hash_limited_security_bits(&shape) >= TARGET_SECURITY_BITS);
            assert!((params.hash_limited_security_bits(&shape) - bits).abs() < 1e-9);
        }
        let wider = Params {
            log_blowup: 4,
            ..params
        };
        assert_eq!(wider.queries_for(PER_ROUND_TARGET_BITS, 8), 65);
        let fewer = Params {
            queries: 80,
            ..params
        };
        let shape = SecurityShape {
            log_rows: 13,
            max_degree: 3,
            identities: 1_000,
            lookups: 1_000,
            table_bits: 16,
            batched: 2_000,
        };
        assert!(fewer.proven_security_bits(&shape) < 128.0);
    }

    #[test]
    fn every_stock_race_shape_meets_the_security_target() {
        use crate::games::race_v1::{self, Track};

        assert_eq!(crate::hash::DIGEST_BYTES, 48);
        assert_eq!(HASH_SECURITY_BITS, 384);
        let params = Params::standard();
        let weaker = Params {
            queries: 80,
            ..params
        };
        for track in [Track::NeonTokyo, Track::Harbor, Track::Sakura] {
            for players in 1..=race_v1::MAX_PLAYERS {
                let circuit = race_v1::circuit(track, players);
                let shape = SecurityShape::of(&circuit);
                let bits = params.hash_limited_security_bits(&shape);
                assert!(
                    bits >= TARGET_SECURITY_BITS,
                    "{track:?}, {players} players: {bits} bits"
                );
                assert_eq!(
                    params.check_security(&circuit, TARGET_SECURITY_BITS),
                    Ok(())
                );
                // Feasibility alone never silently endorses the weaker profile's security.
                assert_eq!(weaker.check(&circuit), Ok(()));
                assert_eq!(
                    weaker.check_security(&circuit, TARGET_SECURITY_BITS),
                    Err(SecurityError::InsufficientSecurity)
                );
            }
        }
    }

    #[test]
    fn security_target_check_rejects_invalid_targets_and_parameters() {
        let circuit = crate::Builder::new("security target", 8).finish();
        let params = Params::standard();
        for target in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1.0] {
            assert_eq!(
                params.check_security(&circuit, target),
                Err(SecurityError::InvalidTarget)
            );
        }
        assert_eq!(
            params.check_security(&circuit, 191.0),
            Err(SecurityError::InsufficientSecurity)
        );
        let unsupported = Params {
            queries: 0,
            ..params
        };
        assert_eq!(
            unsupported.check_security(&circuit, TARGET_SECURITY_BITS),
            Err(SecurityError::Parameters(ParamsError::Unsupported))
        );
    }
}
