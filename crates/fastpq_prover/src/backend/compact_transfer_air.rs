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
//! Verification evaluates only the two complete openings and bounded public
//! polynomials. It never constructs a witness, FFT or LDE. Prover preparation
//! expands exactly 49 fixed public columns once and shares those immutable LDEs
//! plus the periodic phase/hash masks across worker-local arithmetic scratch.
//! Fixed hash numerators have degree <3N and SMT numerators degree <2N. A valid
//! combined numerator is divisible by X^N-1, with quotient degree <2N, as required
//! by the private protocol's independently proved trace/quotient degree bounds.
//!
//! TODO: Qualify the complete protocol's soundness and public resource envelope
//! before changing production admission. This internal prototype proves one
//! declared two-update SMT statement; it changes no query/profile/default limit
//! and does not replace the production verifier's mandatory replay.

use fastpq_isi::FASTPQ_FINAL_V1;
use norito::{NoritoSerialize, codec::Encode as NoritoEncode};

use super::{
    compact_hash_quotient::{CompactHashQuotient, HashNumerators, LOCAL_SLOTS, TRANSITION_SLOTS},
    compact_protocol::{FixedAir, FixedAirSchema},
    compact_smt_quotient::{CompactSmtFixedColumns, CompactSmtQuotient, RESIDUE_COUNT},
};
use crate::{
    Error, Result,
    gadgets::{
        compact_smt_air::{COLUMN_COUNT, DigestLimbs, PHYSICAL_ROW_COUNT, PublicStatement},
        compact_trace_columns::decode_smt_row,
    },
    proof::VerifyLimits,
};

#[cfg(test)]
use super::{
    FriDomain, GOLDILOCKS_MODULUS,
    compact_hash_quotient::ProverMaskCycle,
    compact_protocol::{PreparedAir, ProverEvaluator},
    compact_smt_quotient::{CompactSmtFixedValues, FIXED_COLUMN_COUNT, FIXED_ROW_COUNT},
    fixed_schedule::PeriodicSelectors,
};
#[cfg(test)]
use crate::{fft::Planner, gadgets::compact_smt_air::PHYSICAL_HASH_ROWS};

const CONSTRAINT_COUNT: usize = LOCAL_SLOTS + TRANSITION_SLOTS + RESIDUE_COUNT;
const IDENTITY: &str =
    "fastpq:compact:v1:compact-transfer:v1:342cols:597local+83edge+243smt:65536rows";
#[cfg(test)]
const MASK_CYCLE_ROWS: usize = 4096;
#[cfg(test)]
const LDE_ROWS: usize = 524_288;
#[cfg(test)]
const FIXED_LDE_BYTES: usize = 205_520_896;
#[cfg(test)]
const FIXED_COEFFICIENT_BYTES: usize = 25_690_112;
#[cfg(test)]
const PHASE_CYCLE_BYTES: usize = 16_777_216;

#[derive(NoritoSerialize)]
#[norito(schema_name = "fastpq_prover::compact_v1::TransferUpdateV1")]
struct BoundUpdate {
    old_leaf: [u8; 32],
    new_leaf: [u8; 32],
    path: u32,
}

#[derive(NoritoSerialize)]
#[norito(schema_name = "fastpq_prover::compact_v1::TransferStatementV1")]
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

    fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
        let current = decode_smt_row(current)?;
        let next = decode_smt_row(next)?;
        let hash = self.hash.evaluate(point, &current.hash, &next.hash)?;
        let smt = CompactSmtQuotient::new(&FASTPQ_FINAL_V1, &self.fixed)?;
        let fixed = smt.evaluate_fixed(point)?;
        Ok(combine(hash, smt.residues(&fixed, &current, &next)))
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
    fn fixed_at(&self, index: usize, point: u64) -> Result<CompactSmtFixedValues> {
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

fn combine(hash: HashNumerators<u64>, smt: [u64; RESIDUE_COUNT]) -> Vec<u64> {
    let mut result = Vec::with_capacity(CONSTRAINT_COUNT);
    result.extend(hash.local);
    result.extend(hash.transitions);
    result.extend(smt);
    result
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

#[cfg(test)]
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
}
