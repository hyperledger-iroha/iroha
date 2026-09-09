//! Bounded canonical raw-byte verification for the shared-opening candidate.
//!
//! The caller's byte ceiling applies before header parsing or checksum work.
//! Trusted AIR geometry bounds every sequence and the sum of all dynamic
//! sequence elements during decoding. Exact table dimensions and canonical
//! field values remain the shared verifier's preflight responsibility, before
//! Fiat-Shamir processing. A dimension-invalid value may therefore be decoded,
//! but only inside the same finite resource envelope.
//!
//! The allocation ceiling counts cumulative Norito allocation charges; it is
//! not an RSS measurement or an assertion about allocator capacity rounding.
//! TODO: Qualify the complete production profile and caller migration. This
//! candidate changes neither default proof limits nor production admission.

use norito::DecodeLimits;

use super::*;

/// Maximum cumulative allocation requests charged by Norito during one decode.
#[cfg(test)]
const MAX_DECODE_ALLOCATION_CHARGES: usize = 32 * 1024 * 1024;
/// The deepest current field path has nine nested decodes, including arrays.
const MAX_DECODE_DEPTH: usize = 16;

#[derive(Default)]
struct SequenceBudget {
    largest: usize,
    total: usize,
}

impl SequenceBudget {
    fn include(&mut self, elements: usize, occurrences: usize) -> Result<()> {
        if occurrences == 0 {
            return Ok(());
        }
        self.largest = self.largest.max(elements);
        self.total = elements
            .checked_mul(occurrences)
            .and_then(|elements| self.total.checked_add(elements))
            .ok_or_else(|| shape("shared decode element budget overflow"))?;
        Ok(())
    }

    fn frontier(&mut self, leaves: usize, openings: usize) -> Result<()> {
        let elements = openings
            .checked_mul((leaves.ilog2() as usize).max(1))
            .ok_or_else(|| shape("shared decode frontier budget overflow"))?;
        self.include(elements, 1)
    }
}

fn decode_limits_with_allocation(
    relation: &impl FixedAir,
    geometry: &Geometry,
    frame_bytes: usize,
    limits: VerifyLimits,
    allocation_charges: usize,
) -> Result<DecodeLimits> {
    check_limit(
        "max_compact_statement_bytes",
        relation.statement_bytes().len(),
        limits.max_batch_bytes,
    )?;
    check_limit(
        "max_air_row_values",
        geometry.schema.width,
        limits.max_air_row_values,
    )?;
    let layers = geometry.fri_lengths.len();
    check_limit("max_fri_layers", layers, limits.max_fri_layers)?;
    let queries = profile::QUERY_COUNT;
    check_limit("max_queries", queries, limits.max_queries)?;
    check_limit(
        "max_query_path_len",
        (geometry.lde_rows.ilog2() as usize).max(1),
        limits.max_query_path_len,
    )?;
    let terminal = *geometry
        .fri_lengths
        .last()
        .ok_or_else(|| shape("shared decode needs a terminal FRI layer"))?;
    check_limit(
        "max_fri_round_values",
        terminal.max(2),
        limits.max_fri_round_values,
    )?;
    let rounds = layers
        .checked_sub(1)
        .ok_or_else(|| shape("shared decode needs a committed FRI layer"))?;
    let rows = queries
        .checked_mul(2)
        .ok_or_else(|| shape("shared decode row budget overflow"))?
        .min(geometry.lde_rows);

    // Account for every Vec in SharedProof and its nested row/round tables.
    // The three scalar roots and fixed [Fp4; 2]/[u64; 4] arrays have no dynamic
    // count headers; their field bodies still consume the allocation budget.
    let mut budget = SequenceBudget::default();
    budget.include(layers, 1)?; // fri_roots
    budget.include(rows, 1)?;
    budget.include(geometry.schema.width, rows)?;
    budget.include(queries, 1)?;
    budget.frontier(geometry.lde_rows, rows)?;
    budget.frontier(geometry.lde_rows, queries)?;
    budget.frontier(geometry.lde_rows, queries)?;
    budget.include(rounds, 1)?;
    for &length in geometry.fri_lengths.iter().take(rounds) {
        let leaves = length / 2;
        let groups = queries.min(leaves);
        budget.include(groups, 1)?;
        budget.frontier(leaves, groups)?;
    }
    budget.include(terminal, 1)?;
    Ok(DecodeLimits::new(
        budget.largest,
        frame_bytes,
        budget.total,
        allocation_charges,
        MAX_DECODE_DEPTH,
    ))
}

// Deliberately private: this bounded decode alone does not establish exact
// dimensions, field canonicality, the AIR relation or authenticated openings.
#[cfg(test)]
fn decode_bounded(
    relation: &impl FixedAir,
    bytes: &[u8],
    limits: VerifyLimits,
    allocation_charges: usize,
) -> Result<SharedProof> {
    // This must precede even Geometry::new and Norito header/CRC inspection.
    check_limit("max_proof_bytes", bytes.len(), limits.max_proof_bytes)?;
    let geometry = Geometry::new(relation)?;
    let decode_limits = decode_limits_with_allocation(
        relation,
        &geometry,
        bytes.len(),
        limits,
        allocation_charges,
    )?;
    // This API scopes canonical flags, validates the complete frame and
    // compares canonical re-encoding without another frame-sized allocation.
    // Its payload-derived defaults and any outer budgets remain active too.
    Ok(norito::decode_canonical_with_limits(bytes, decode_limits)?)
}

/// Complete checked proof result; its root is copied from the same bounded decode.
/// Private fields prevent a decoded or partially verified proof from constructing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend) struct VerifiedSharedProof {
    work: SharedVerificationWork,
    row_root: WireDigest,
}

impl VerifiedSharedProof {
    /// Full six-field AIR row commitment authenticated by complete verification.
    pub(in crate::backend) const fn row_root(&self) -> WireDigest {
        self.row_root
    }
    /// Measured work from the same successful verification.
    pub(in crate::backend) const fn work(&self) -> SharedVerificationWork {
        self.work
    }
}

#[cfg(test)]
fn decode_and_verify_recorded(
    relation: &impl FixedAir,
    bytes: &[u8],
    limits: VerifyLimits,
    work: &mut SharedVerificationWork,
    allocation_charges: usize,
) -> Result<()> {
    let proof = decode_bounded(relation, bytes, limits, allocation_charges)?;
    // Keep the existing typed preflight, transcript, Merkle and FRI checks in
    // one implementation; no successful raw decode bypasses any of them.
    verify_shared_recorded(relation, &proof, limits, work)
}

/// Decode and verify the distinct candidate frame with explicit allocation policy.
///
/// The raw cap precedes geometry/header work. Sequence limits come from the
/// caller-fixed 375-query descriptor, never the encoded counts. The allocation
/// cap is diagnostic caller policy; no production default is raised here.
#[cfg(test)]
pub(in crate::backend) fn decode_and_verify_with_allocation(
    relation: &impl FixedAir,
    bytes: &[u8],
    limits: VerifyLimits,
    allocation_charges: usize,
) -> Result<SharedVerificationWork> {
    Ok(
        decode_and_verify_with_allocation_committed(relation, bytes, limits, allocation_charges)?
            .work(),
    )
}

/// Return candidate work and the complete authenticated AIR row commitment.
/// The result exists only after every transcript, opening, AIR and terminal check.
pub(in crate::backend) fn decode_and_verify_with_allocation_committed(
    relation: &impl FixedAir,
    bytes: &[u8],
    limits: VerifyLimits,
    allocation_charges: usize,
) -> Result<VerifiedSharedProof> {
    check_limit("max_proof_bytes", bytes.len(), limits.max_proof_bytes)?;
    let geometry = Geometry::new(relation)?;
    let decode_limits = decode_limits_with_allocation(
        relation,
        &geometry,
        bytes.len(),
        limits,
        allocation_charges,
    )?;
    let proof: SharedProof = norito::decode_canonical_with_limits(bytes, decode_limits)?;
    let row_root = proof.row_root;
    let work = verify_shared(relation, &proof, limits)?;
    Ok(VerifiedSharedProof { work, row_root })
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use super::super::tests::{air, fixture};
    use super::*;

    // Full 375-query frames use the same explicit diagnostic policy as the
    // complete-transfer tests. The 32 MiB baseline remains a rejection control.
    const FIXTURE_ALLOCATION_CHARGES: usize = 64 * 1024 * 1024;

    fn diagnostic_limits() -> VerifyLimits {
        super::super::super::test_fixture::limits()
    }

    fn encode(proof: &SharedProof) -> Vec<u8> {
        norito::encode_canonical(proof).unwrap()
    }

    fn assert_before_transcript(bytes: &[u8], limits: VerifyLimits) -> Error {
        let mut work = SharedVerificationWork::default();
        let error = decode_and_verify_recorded(
            &air(),
            bytes,
            limits,
            &mut work,
            FIXTURE_ALLOCATION_CHARGES,
        )
        .expect_err("malformed raw proof must be rejected");
        assert_eq!(work, SharedVerificationWork::default());
        error
    }

    fn bare(proof: &SharedProof) -> Vec<u8> {
        let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let (bytes, flags) = norito::codec::encode_with_header_flags(proof);
        assert_eq!(flags, norito::core::default_encode_flags());
        bytes
    }

    fn frame(payload: &[u8]) -> Vec<u8> {
        norito::core::frame_bare_with_header_flags::<SharedProof>(
            payload,
            norito::core::default_encode_flags(),
        )
        .unwrap()
    }

    // Test-only byte locations in the canonical sequential layout. The actual
    // admission path above has no schema-specific byte parser.
    fn field(bytes: &[u8], index: usize) -> Range<usize> {
        let mut offset = 0;
        for position in 0..=index {
            let (length, prefix) = norito::core::read_len_from_slice_with_flags(
                &bytes[offset..],
                norito::core::default_encode_flags(),
            )
            .unwrap();
            let start = offset + prefix;
            let end = start + length;
            assert!(end <= bytes.len());
            if position == index {
                return start..end;
            }
            offset = end;
        }
        unreachable!()
    }

    fn first_element(bytes: &[u8]) -> Range<usize> {
        let (count, prefix) = norito::core::inspect_seq_len_slice(bytes).unwrap();
        assert!(count > 0);
        let element = field(&bytes[prefix..], 0);
        prefix + element.start..prefix + element.end
    }

    #[test]
    fn canonical_raw_admission_matches_typed_verification_and_measures_headroom() {
        let relation = air();
        let proof = &fixture().shared;
        let bytes = encode(proof);
        let limits = diagnostic_limits();
        let expected = verify_shared(&relation, proof, limits).unwrap();
        assert!(matches!(
            decode_bounded(&relation, &bytes, limits, MAX_DECODE_ALLOCATION_CHARGES),
            Err(Error::Encode(norito::Error::TotalAllocationExceeded { limit, .. }))
                if limit == MAX_DECODE_ALLOCATION_CHARGES as u64
        ));
        assert_eq!(
            decode_and_verify_with_allocation(
                &relation,
                &bytes,
                limits,
                FIXTURE_ALLOCATION_CHARGES
            )
            .unwrap(),
            expected
        );
        let geometry = Geometry::new(&relation).unwrap();
        let budget = decode_limits_with_allocation(
            &relation,
            &geometry,
            bytes.len(),
            limits,
            FIXTURE_ALLOCATION_CHARGES,
        )
        .unwrap();
        let (decoded, usage) = norito::core::with_decode_limits_measured(budget, || {
            decode_bounded(&relation, &bytes, limits, FIXTURE_ALLOCATION_CHARGES)
        });
        assert_eq!(&decoded.unwrap(), proof);
        assert!(usage.total_elements() <= budget.max_total_elements());
        assert!(usage.total_allocated_bytes() < FIXTURE_ALLOCATION_CHARGES);
        eprintln!(
            "full_geometry_fixture_bytes={}; allocation_charges={}; diagnostic_allocation_limit={}; default_allocation_limit={}",
            bytes.len(),
            usage.total_allocated_bytes(),
            FIXTURE_ALLOCATION_CHARGES,
            MAX_DECODE_ALLOCATION_CHARGES,
        );
        // A caller-owned subslice may require different alignment copies.
        let mut unaligned = vec![0; bytes.len() + 8];
        for offset in 0..8 {
            unaligned[offset..offset + bytes.len()].copy_from_slice(&bytes);
            let (decoded, usage) = norito::core::with_decode_limits_measured(budget, || {
                decode_bounded(
                    &relation,
                    &unaligned[offset..offset + bytes.len()],
                    limits,
                    FIXTURE_ALLOCATION_CHARGES,
                )
            });
            assert_eq!(&decoded.unwrap(), proof);
            assert!(usage.total_allocated_bytes() < FIXTURE_ALLOCATION_CHARGES);
        }
    }

    #[test]
    fn incoming_byte_cap_precedes_even_header_and_geometry_validation() {
        let limits = VerifyLimits {
            max_proof_bytes: 0,
            max_air_row_values: 0,
            ..diagnostic_limits()
        };
        assert!(matches!(
            assert_before_transcript(&[0xff], limits),
            Error::VerifierLimitExceeded {
                limit: "max_proof_bytes",
                actual: 1,
                max: 0,
            }
        ));
        let bytes = encode(&fixture().shared);
        assert_before_transcript(
            &bytes,
            VerifyLimits {
                max_proof_bytes: bytes.len() - 1,
                ..diagnostic_limits()
            },
        );
        assert!(
            decode_bounded(
                &air(),
                &bytes,
                VerifyLimits {
                    max_proof_bytes: bytes.len(),
                    ..diagnostic_limits()
                },
                FIXTURE_ALLOCATION_CHARGES
            )
            .is_ok()
        );
    }

    #[test]
    fn every_top_level_count_and_nested_row_count_bomb_is_bounded() {
        let relation = air();
        let original = bare(&fixture().shared);
        let geometry = Geometry::new(&relation).unwrap();
        for index in 3..11 {
            let mut payload = original.clone();
            let count = field(&payload, index).start;
            payload[count..count + 8].copy_from_slice(&u64::MAX.to_le_bytes());
            let bytes = frame(&payload); // Valid schema, flags, length and CRC.
            let budget = decode_limits_with_allocation(
                &relation,
                &geometry,
                bytes.len(),
                diagnostic_limits(),
                FIXTURE_ALLOCATION_CHARGES,
            )
            .unwrap();
            let (error, usage) = norito::core::with_decode_limits_measured(budget, || {
                assert_before_transcript(&bytes, diagnostic_limits())
            });
            assert!(matches!(
                error,
                Error::Encode(norito::Error::SequenceLengthExceeded { .. })
            ));
            assert!(usage.total_elements() <= budget.max_total_elements());
            assert!(usage.total_allocated_bytes() <= budget.max_total_allocated_bytes());
        }
        let mut payload = original;
        let rows = field(&payload, 4);
        let row = first_element(&payload[rows.clone()]);
        let values = field(&payload[rows.start + row.start..rows.start + row.end], 1);
        let count = rows.start + row.start + values.start;
        payload[count..count + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(matches!(
            assert_before_transcript(&frame(&payload), diagnostic_limits()),
            Error::Encode(norito::Error::SequenceLengthExceeded { .. })
        ));
        for nested in 0..2 {
            let mut payload = bare(&fixture().shared);
            let rounds = field(&payload, 9);
            let round = first_element(&payload[rounds.clone()]);
            let values = field(
                &payload[rounds.start + round.start..rounds.start + round.end],
                nested,
            );
            let count = rounds.start + round.start + values.start;
            payload[count..count + 8].copy_from_slice(&u64::MAX.to_le_bytes());
            assert!(matches!(
                assert_before_transcript(&frame(&payload), diagnostic_limits()),
                Error::Encode(norito::Error::SequenceLengthExceeded { .. })
            ));
        }
        // The first field claims u64::MAX body bytes, using a canonical varint.
        let mut huge_field = vec![0xff; 9];
        huge_field.push(1);
        assert!(matches!(
            assert_before_transcript(&frame(&huge_field), diagnostic_limits()),
            Error::Encode(norito::Error::FieldLengthExceeded { .. })
        ));
    }

    #[test]
    fn cumulative_nested_elements_and_allocation_scopes_cannot_be_relaxed() {
        let relation = air();
        let geometry = Geometry::new(&relation).unwrap();
        let budget = decode_limits_with_allocation(
            &relation,
            &geometry,
            diagnostic_limits().max_proof_bytes,
            diagnostic_limits(),
            FIXTURE_ALLOCATION_CHARGES,
        )
        .unwrap();
        let mut proof = fixture().shared.clone();
        let count = budget.max_total_elements() / budget.max_sequence_elements() + 1;
        proof.rows = (0..count)
            .map(|index| SharedRow {
                index: index as u32,
                values: vec![0; budget.max_sequence_elements()],
            })
            .collect();
        assert!(proof.rows.len() <= budget.max_sequence_elements());
        let bytes = encode(&proof);
        assert!(bytes.len() < diagnostic_limits().max_proof_bytes);
        assert!(matches!(
            assert_before_transcript(&bytes, diagnostic_limits()),
            Error::Encode(norito::Error::TotalElementsExceeded { .. })
        ));
        let bytes = encode(&fixture().shared);
        let stricter = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, MAX_DECODE_DEPTH);
        let (error, usage) = norito::core::with_decode_limits_measured(stricter, || {
            assert_before_transcript(&bytes, diagnostic_limits())
        });
        assert!(matches!(
            error,
            Error::Encode(norito::Error::TotalAllocationExceeded { .. })
        ));
        assert_eq!(usage.total_allocated_bytes(), 0);
        // The prior budget is restored after the terminal failure.
        assert!(
            decode_bounded(
                &relation,
                &bytes,
                diagnostic_limits(),
                FIXTURE_ALLOCATION_CHARGES
            )
            .is_ok()
        );
        let shallow = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 1);
        let (error, _) = norito::core::with_decode_limits_measured(shallow, || {
            assert_before_transcript(&bytes, diagnostic_limits())
        });
        assert!(matches!(
            error,
            Error::Encode(norito::Error::NestingDepthExceeded { limit: 1, .. })
        ));
        assert!(
            decode_bounded(
                &relation,
                &bytes,
                diagnostic_limits(),
                FIXTURE_ALLOCATION_CHARGES
            )
            .is_ok()
        );
        let baseline_budget = decode_limits_with_allocation(
            &relation,
            &geometry,
            bytes.len(),
            diagnostic_limits(),
            MAX_DECODE_ALLOCATION_CHARGES,
        )
        .unwrap();
        let error = norito::with_decode_limits(baseline_budget, || {
            norito::core::reserve_decode_allocation(MAX_DECODE_ALLOCATION_CHARGES + 1)
        })
        .unwrap_err();
        assert!(
            matches!(error, norito::Error::TotalAllocationExceeded { limit, .. }
            if limit == MAX_DECODE_ALLOCATION_CHARGES as u64)
        );
    }

    #[test]
    fn header_corruption_compression_truncation_and_trailing_bytes_fail_closed() {
        let baseline = encode(&fixture().shared);
        for offset in [0, 4, 5, 6, 23, 31] {
            let mut bytes = baseline.clone();
            bytes[offset] ^= 1; // magic/version/schema/payload length/checksum
            assert_before_transcript(&bytes, diagnostic_limits());
        }
        let mut compressed = baseline.clone();
        compressed[22] = norito::Compression::Zstd as u8;
        assert!(matches!(
            assert_before_transcript(&compressed, diagnostic_limits()),
            Error::Encode(norito::Error::NonCanonicalEncoding)
        ));
        let mut reserved_flags = baseline.clone();
        reserved_flags[39] |= norito::core::header_flags::VARINT_OFFSETS;
        assert_before_transcript(&reserved_flags, diagnostic_limits());
        for length in [0, 1, 39, 40, baseline.len() - 1] {
            assert_before_transcript(&baseline[..length], diagnostic_limits());
        }
        let mut trailing_frame = baseline;
        trailing_frame.push(0);
        assert_before_transcript(&trailing_frame, diagnostic_limits());
        let mut trailing_payload = bare(&fixture().shared);
        trailing_payload.push(0);
        assert_before_transcript(&frame(&trailing_payload), diagnostic_limits());
        let mut corrupt_payload = encode(&fixture().shared);
        *corrupt_payload.last_mut().unwrap() ^= 1;
        assert!(matches!(
            assert_before_transcript(&corrupt_payload, diagnostic_limits()),
            Error::Encode(norito::Error::ChecksumMismatch)
        ));
    }

    #[test]
    fn canonical_admission_is_independent_of_every_supported_ambient_layout() {
        let proof = &fixture().shared;
        let canonical = encode(proof);
        for flags in
            (u8::MIN..=u8::MAX).filter(|&flags| norito::core::validate_header_flags(flags).is_ok())
        {
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            assert_eq!(
                decode_bounded(
                    &air(),
                    &canonical,
                    diagnostic_limits(),
                    FIXTURE_ALLOCATION_CHARGES
                )
                .unwrap(),
                *proof
            );
            assert_eq!(norito::core::get_decode_flags(), flags);
            let alternate = norito::to_bytes(proof).unwrap();
            if alternate != canonical {
                assert_before_transcript(&alternate, diagnostic_limits());
            }
            assert_eq!(norito::core::get_decode_flags(), flags);
            assert_eq!(encode(proof), canonical);
            assert_eq!(norito::core::get_decode_flags(), flags);
        }
    }

    #[test]
    fn exact_dimensions_and_noncanonical_fields_reject_before_transcript() {
        let baseline = &fixture().shared;
        let mut width = baseline.clone();
        width.rows[0].values.push(0);
        let bytes = encode(&width);
        // The uniform ceiling deliberately permits this small malformed row.
        assert!(
            decode_bounded(
                &air(),
                &bytes,
                diagnostic_limits(),
                FIXTURE_ALLOCATION_CHARGES
            )
            .is_ok()
        );
        assert_before_transcript(&bytes, diagnostic_limits());
        let mut base = baseline.clone();
        base.rows[0].values[0] = GOLDILOCKS_MODULUS;
        assert_before_transcript(&encode(&base), diagnostic_limits());
        for lane in 0..4 {
            for position in 0..5 {
                let mut proof = baseline.clone();
                let mut coefficients = [0; 4];
                coefficients[lane] = GOLDILOCKS_MODULUS;
                let invalid = GoldilocksFp4V1::from_coefficients_unchecked_for_test(coefficients);
                match position {
                    0 => proof.queries[0].mixed = invalid,
                    1 => proof.queries[0].quotient = invalid,
                    2 => proof.rounds[0].groups[0].values[0] = invalid,
                    3 => proof.rounds[0].groups[0].values[1] = invalid,
                    _ => proof.terminal_values[0] = invalid,
                }
                assert_before_transcript(&encode(&proof), diagnostic_limits());
            }
        }
        for lane in 0..6 {
            let mut payload = bare(baseline);
            let root = field(&payload, 0).start;
            payload[root + lane * 8..root + (lane + 1) * 8]
                .copy_from_slice(&GOLDILOCKS_MODULUS.to_le_bytes());
            assert_before_transcript(&frame(&payload), diagnostic_limits());
        }
    }

    // This adapter is solely for resource geometry. It is never used to prove
    // or verify an AIR; all successful cryptographic tests reuse the explicit full-geometry columns fixture.
    struct SizingAir {
        schema: FixedAirSchema,
    }

    impl FixedAir for SizingAir {
        fn schema(&self) -> FixedAirSchema {
            self.schema
        }
        fn statement_bytes(&self) -> &[u8] {
            b"shared-codec-sizing"
        }
        fn evaluate(&self, _: u64, _: &[u64], _: &[u64]) -> Result<Vec<u64>> {
            Err(shape("a codec sizing fixture must not evaluate the AIR"))
        }
    }

    fn largest_shape(geometry: &Geometry) -> SharedProof {
        let queries = profile::QUERY_COUNT;
        let rows = (queries * 2).min(geometry.lde_rows);
        let siblings = |leaves: usize, openings: usize| {
            vec![WireDigest::default(); openings * (leaves.ilog2() as usize).max(1)]
        };
        SharedProof {
            row_root: WireDigest::default(),
            mixed_root: WireDigest::default(),
            quotient_root: WireDigest::default(),
            fri_roots: vec![WireDigest::default(); geometry.fri_lengths.len()],
            rows: (0..rows)
                .map(|index| SharedRow {
                    index: index as u32,
                    values: vec![0; geometry.schema.width],
                })
                .collect(),
            queries: (0..queries)
                .map(|index| SharedQuery {
                    index: index as u32,
                    mixed: GoldilocksFp4V1::ZERO,
                    quotient: GoldilocksFp4V1::ZERO,
                })
                .collect(),
            row_siblings: siblings(geometry.lde_rows, rows),
            mixed_siblings: siblings(geometry.lde_rows, queries),
            quotient_siblings: siblings(geometry.lde_rows, queries),
            rounds: geometry.fri_lengths[..geometry.fri_lengths.len() - 1]
                .iter()
                .map(|&length| {
                    let leaves = length / 2;
                    let groups = queries.min(leaves);
                    SharedRound {
                        groups: (0..groups)
                            .map(|index| SharedGroup {
                                index: index as u32,
                                values: [GoldilocksFp4V1::ZERO; 2],
                            })
                            .collect(),
                        siblings: siblings(leaves, groups),
                    }
                })
                .collect(),
            terminal_values: vec![GoldilocksFp4V1::ZERO; *geometry.fri_lengths.last().unwrap()],
        }
    }

    #[test]
    fn final_geometry_has_exact_element_budget_and_rejects_other_widths() {
        let relation = air();
        let geometry = Geometry::new(&relation).unwrap();
        let proof = largest_shape(&geometry);
        let bytes = encode(&proof);
        assert_eq!(bytes.len(), 6_713_525);
        assert!(bytes.len() > VerifyLimits::default().max_proof_bytes);
        let budget = decode_limits_with_allocation(
            &relation,
            &geometry,
            bytes.len(),
            diagnostic_limits(),
            MAX_DECODE_ALLOCATION_CHARGES,
        )
        .unwrap();
        assert_eq!(budget.max_sequence_elements(), 14_250);
        assert_eq!(budget.max_total_elements(), 344_631);
        assert_eq!(budget.max_total_allocated_bytes(), 32 * 1024 * 1024);
        assert_eq!(budget.max_nesting_depth(), 16);
        // This loose shape exceeds the unchanged 32 MiB default allocation
        // policy. Keep that rejection explicit before isolating element counts.
        assert!(matches!(
            decode_bounded(&relation, &bytes, diagnostic_limits(), MAX_DECODE_ALLOCATION_CHARGES),
            Err(Error::Encode(norito::Error::TotalAllocationExceeded {
                attempted,
                limit,
            })) if limit == MAX_DECODE_ALLOCATION_CHARGES as u64 && attempted > limit
        ));
        // The separate largest-shape diagnostic measures about 52.3 MiB of
        // cumulative charges. This explicit 64 MiB test scope only permits
        // structural decoding; the dummy roots do not make a valid proof.
        let element_budget = decode_limits_with_allocation(
            &relation,
            &geometry,
            bytes.len(),
            diagnostic_limits(),
            64 * 1024 * 1024,
        )
        .unwrap();
        let (decoded, usage) = norito::core::with_decode_limits_measured(element_budget, || {
            norito::decode_canonical_with_limits::<SharedProof>(&bytes, element_budget)
        });
        assert_eq!(decoded.unwrap(), proof);
        assert_eq!(usage.total_elements(), 344_631);
        assert!(usage.total_allocated_bytes() > MAX_DECODE_ALLOCATION_CHARGES);
        assert!(usage.total_allocated_bytes() < element_budget.max_total_allocated_bytes());
        assert_eq!(
            preflight_shared(&relation, &proof, diagnostic_limits(), &geometry).unwrap(),
            bytes.len()
        );
        assert!(matches!(
            decode_bounded(
                &relation,
                &bytes,
                VerifyLimits::default(),
                MAX_DECODE_ALLOCATION_CHARGES
            ),
            Err(Error::VerifierLimitExceeded {
                limit: "max_proof_bytes",
                ..
            })
        ));
        for width in [1, 341, 343, 512] {
            let invalid = SizingAir {
                schema: FixedAirSchema {
                    width,
                    ..relation.schema()
                },
            };
            assert!(Geometry::new(&invalid).is_err());
        }
        let mut overflow = SequenceBudget::default();
        assert!(overflow.include(usize::MAX, 2).is_err());
        overflow.include(usize::MAX, 1).unwrap();
        assert!(overflow.include(1, 1).is_err());
    }

    struct CandidateShape;
    impl FixedAir for CandidateShape {
        fn schema(&self) -> FixedAirSchema {
            FixedAirSchema {
                trace_rows: 65_536,
                width: 342,
                constraints: 923,
                identity: "candidate-codec-shape-only:v1",
            }
        }
        fn statement_bytes(&self) -> &[u8] {
            b"public codec boundary fixture"
        }
        fn evaluate(&self, _: u64, _: &[u64], _: &[u64]) -> Result<Vec<u64>> {
            panic!("codec failures must precede AIR evaluation")
        }
    }

    fn candidate_limits() -> VerifyLimits {
        VerifyLimits {
            max_proof_bytes: 4_326_227,
            max_queries: 375,
            ..VerifyLimits::default()
        }
    }

    #[test]
    fn final_shared_schema_preserves_exact_payload_and_rejects_both_retired_frames() {
        let shape_policy = VerifyLimits {
            max_proof_bytes: 8 * 1024 * 1024,
            ..candidate_limits()
        };
        macro_rules! retired_frame {
            ($name:ident, $schema:literal) => {
                #[derive(NoritoSerialize, norito::NoritoSchema)]
                #[norito_schema(name = $schema)]
                struct $name {
                    row_root: WireDigest,
                    mixed_root: WireDigest,
                    quotient_root: WireDigest,
                    fri_roots: Vec<WireDigest>,
                    rows: Vec<SharedRow>,
                    queries: Vec<SharedQuery>,
                    row_siblings: Vec<WireDigest>,
                    mixed_siblings: Vec<WireDigest>,
                    quotient_siblings: Vec<WireDigest>,
                    rounds: Vec<SharedRound>,
                    terminal_values: Vec<GoldilocksFp4V1>,
                }
            };
        }
        retired_frame!(
            RetiredPrototype,
            "fastpq_prover::compact_prototype::SharedProofV1"
        );
        retired_frame!(
            RetiredShake,
            "fastpq_prover::compact_candidate::ShakeSharedProofV1"
        );
        let geometry = Geometry::new(&CandidateShape).unwrap();
        let expected = largest_shape(&geometry);
        let row_pointer = expected.rows.as_ptr();
        let query_pointer = expected.queries.as_ptr();
        let bytes = norito::encode_canonical(&expected).unwrap();
        assert_eq!(
            norito::decode_canonical::<SharedProof>(&bytes).unwrap(),
            expected
        );
        assert_eq!(row_pointer, expected.rows.as_ptr());
        assert_eq!(query_pointer, expected.queries.as_ptr());
        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let (payload, _) = norito::codec::encode_with_header_flags(&expected);
        let old_prototype = RetiredPrototype {
            row_root: expected.row_root.clone(),
            mixed_root: expected.mixed_root.clone(),
            quotient_root: expected.quotient_root.clone(),
            fri_roots: expected.fri_roots.clone(),
            rows: expected.rows.clone(),
            queries: expected.queries.clone(),
            row_siblings: expected.row_siblings.clone(),
            mixed_siblings: expected.mixed_siblings.clone(),
            quotient_siblings: expected.quotient_siblings.clone(),
            rounds: expected.rounds.clone(),
            terminal_values: expected.terminal_values.clone(),
        };
        let old_shake = RetiredShake {
            row_root: expected.row_root.clone(),
            mixed_root: expected.mixed_root.clone(),
            quotient_root: expected.quotient_root.clone(),
            fri_roots: expected.fri_roots.clone(),
            rows: expected.rows.clone(),
            queries: expected.queries.clone(),
            row_siblings: expected.row_siblings.clone(),
            mixed_siblings: expected.mixed_siblings.clone(),
            quotient_siblings: expected.quotient_siblings.clone(),
            rounds: expected.rounds.clone(),
            terminal_values: expected.terminal_values.clone(),
        };
        for (old_bytes, old_payload) in [
            (
                norito::encode_canonical(&old_prototype).unwrap(),
                norito::codec::encode_with_header_flags(&old_prototype).0,
            ),
            (
                norito::encode_canonical(&old_shake).unwrap(),
                norito::codec::encode_with_header_flags(&old_shake).0,
            ),
        ] {
            assert_eq!(old_payload, payload);
            assert_eq!(old_bytes.len(), bytes.len());
            assert!(norito::decode_canonical::<SharedProof>(&old_bytes).is_err());
            assert!(
                decode_and_verify_with_allocation(
                    &CandidateShape,
                    &old_bytes,
                    shape_policy,
                    128 * 1024 * 1024
                )
                .is_err()
            );
        }
        let mut wrong_count = expected;
        wrong_count.queries.pop();
        let wrong_bytes = norito::encode_canonical(&wrong_count).unwrap();
        assert!(matches!(
            decode_and_verify_with_allocation(
                &CandidateShape,
                &wrong_bytes,
                shape_policy,
                128 * 1024 * 1024
            ),
            Err(Error::InvalidTraceShape { .. })
        ));
    }

    #[test]
    fn candidate_raw_cap_precedes_geometry_and_schema_and_policy_is_fixed() {
        struct NoCalls;
        impl FixedAir for NoCalls {
            fn schema(&self) -> FixedAirSchema {
                panic!("raw byte cap must run first")
            }
            fn statement_bytes(&self) -> &[u8] {
                panic!("raw byte cap must run first")
            }
            fn evaluate(&self, _: u64, _: &[u64], _: &[u64]) -> Result<Vec<u64>> {
                panic!("no AIR")
            }
        }
        let zero = VerifyLimits {
            max_proof_bytes: 0,
            ..candidate_limits()
        };
        assert!(matches!(
            decode_and_verify_with_allocation(&NoCalls, &[255], zero, 0),
            Err(Error::VerifierLimitExceeded {
                limit: "max_proof_bytes",
                actual: 1,
                max: 0
            })
        ));
        let restricted = VerifyLimits {
            max_queries: 374,
            ..candidate_limits()
        };
        assert!(matches!(
            decode_and_verify_with_allocation(&CandidateShape, &[255], restricted, usize::MAX),
            Err(Error::VerifierLimitExceeded {
                limit: "max_queries",
                actual: 375,
                max: 374
            })
        ));
        assert!(Geometry::new(&air()).is_ok());
        let invalid = SizingAir {
            schema: FixedAirSchema {
                width: 1,
                ..air().schema()
            },
        };
        assert!(Geometry::new(&invalid).is_err());
        let geometry = Geometry::new(&CandidateShape).unwrap();
        let budget = decode_limits_with_allocation(
            &CandidateShape,
            &geometry,
            4_326_227,
            candidate_limits(),
            123456,
        )
        .unwrap();
        assert_eq!(budget.max_total_allocated_bytes(), 123456);
        assert_eq!(budget.max_sequence_elements(), 750 * 19);
        assert!(budget.max_total_elements() > 750 * 342);
        assert!(budget.max_total_elements() < 375_000);
    }

    #[test]
    fn same_final_schema_requires_the_complete_original_context() {
        // Same final schema decodes, but the complete different relation/context
        // cannot verify the columns fixture's roots and transcript.
        let fixture_bytes = encode(&fixture().shared);
        assert!(norito::decode_canonical::<SharedProof>(&fixture_bytes).is_ok());
        assert!(
            decode_and_verify_with_allocation(
                &CandidateShape,
                &fixture_bytes,
                candidate_limits(),
                usize::MAX
            )
            .is_err()
        );
    }

    #[test]
    fn candidate_counts_and_nested_allocation_cannot_escape_caller_budgets() {
        let shape_policy = VerifyLimits {
            max_proof_bytes: 8 * 1024 * 1024,
            ..candidate_limits()
        };
        let geometry = Geometry::new(&CandidateShape).unwrap();
        let candidate = largest_shape(&geometry);
        let bytes = norito::encode_canonical(&candidate).unwrap();
        let budget = decode_limits_with_allocation(
            &CandidateShape,
            &geometry,
            bytes.len(),
            shape_policy,
            128 * 1024 * 1024,
        )
        .unwrap();
        let (decoded, usage) = norito::core::with_decode_limits_measured(budget, || {
            norito::decode_canonical_with_limits::<SharedProof>(&bytes, budget)
        });
        assert_eq!(decoded.unwrap(), candidate);
        let charges = usage.total_allocated_bytes();
        assert!(charges > 0);
        let tighter =
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, charges - 1, 16);
        let (error, usage) = norito::core::with_decode_limits_measured(tighter, || {
            decode_and_verify_with_allocation(
                &CandidateShape,
                &bytes,
                shape_policy,
                128 * 1024 * 1024,
            )
        });
        assert!(matches!(
            error,
            Err(Error::Encode(norito::Error::TotalAllocationExceeded { .. }))
        ));
        assert!(usage.total_allocated_bytes() < charges);
        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let (original, _) = norito::codec::encode_with_header_flags(&candidate);
        for field_index in 3..=10 {
            let mut payload = original.clone();
            let range = field(&payload, field_index);
            payload[range.start..range.start + 8].copy_from_slice(&u64::MAX.to_le_bytes());
            let bytes = norito::core::frame_bare_with_header_flags::<SharedProof>(
                &payload,
                norito::core::default_encode_flags(),
            )
            .unwrap();
            assert!(matches!(
                decode_and_verify_with_allocation(
                    &CandidateShape,
                    &bytes,
                    shape_policy,
                    128 * 1024 * 1024
                ),
                Err(Error::Encode(norito::Error::SequenceLengthExceeded { .. }))
            ));
        }
    }

    #[test]
    fn candidate_largest_decode_shape_matches_wire_bound_and_measures_charges() {
        let geometry = Geometry::new(&CandidateShape).unwrap();
        // Every table takes its independent loose pre-transcript maximum.
        // These dummy roots/indices do not constitute a valid proof or AIR.
        let proof = largest_shape(&geometry);
        let bytes = norito::encode_canonical(&proof).unwrap();
        // The 9,270 Fp4 values each use the canonical 32-byte carrier.
        assert_eq!(bytes.len(), 6_713_525);
        assert!(bytes.len() > candidate_limits().max_proof_bytes);
        let limits = VerifyLimits {
            max_proof_bytes: 8 * 1024 * 1024,
            ..candidate_limits()
        };
        let budget = decode_limits_with_allocation(
            &CandidateShape,
            &geometry,
            bytes.len(),
            limits,
            64 * 1024 * 1024,
        )
        .unwrap();
        let (decoded, usage) = norito::core::with_decode_limits_measured(budget, || {
            norito::decode_canonical_with_limits::<SharedProof>(&bytes, budget)
        });
        assert_eq!(decoded.unwrap(), proof);
        assert!(usage.total_elements() <= budget.max_total_elements());
        assert!(usage.total_allocated_bytes() < 64 * 1024 * 1024);
        assert!(matches!(
            codec_reject_before_geometry(&bytes),
            Error::VerifierLimitExceeded {
                limit: "max_proof_bytes",
                ..
            }
        ));
        eprintln!(
            "candidate_loose_shape_bytes={}; elements={}; allocation_charges={}; valid_proof=false; production_default_changed=false",
            bytes.len(),
            usage.total_elements(),
            usage.total_allocated_bytes()
        );
    }

    fn codec_reject_before_geometry(bytes: &[u8]) -> Error {
        struct NoGeometry;
        impl FixedAir for NoGeometry {
            fn schema(&self) -> FixedAirSchema {
                panic!("oversized bytes must precede geometry")
            }
            fn statement_bytes(&self) -> &[u8] {
                panic!("oversized bytes must precede public context")
            }
            fn evaluate(&self, _: u64, _: &[u64], _: &[u64]) -> Result<Vec<u64>> {
                panic!("no AIR")
            }
        }
        decode_and_verify_with_allocation(&NoGeometry, bytes, candidate_limits(), usize::MAX)
            .unwrap_err()
    }

    #[test]
    fn committed_decode_returns_full_root_only_after_complete_verification() {
        let relation = air();
        let proof = &fixture().shared;
        let bytes = encode(proof);
        let limits = diagnostic_limits();
        let verified = decode_and_verify_with_allocation_committed(
            &relation,
            &bytes,
            limits,
            FIXTURE_ALLOCATION_CHARGES,
        )
        .unwrap();
        assert_eq!(verified.row_root(), proof.row_root);
        assert_eq!(verified.row_root().to_le_bytes().len(), 48);
        assert_eq!(
            verified.work(),
            decode_and_verify_with_allocation(
                &relation,
                &bytes,
                limits,
                FIXTURE_ALLOCATION_CHARGES
            )
            .unwrap()
        );
        let mut changed = proof.clone();
        let mut words = changed.row_root.words();
        words[5] = (words[5] + 1) % GOLDILOCKS_MODULUS;
        changed.row_root = WireDigest::new(words).unwrap();
        assert!(
            decode_and_verify_with_allocation_committed(
                &relation,
                &encode(&changed),
                limits,
                FIXTURE_ALLOCATION_CHARGES
            )
            .is_err()
        );
        changed = proof.clone();
        changed.terminal_values[0] = changed.terminal_values[0].add(GoldilocksFp4V1::ONE);
        assert!(
            decode_and_verify_with_allocation_committed(
                &relation,
                &encode(&changed),
                limits,
                FIXTURE_ALLOCATION_CHARGES
            )
            .is_err()
        );
    }

    #[test]
    fn committed_candidate_decode_preserves_raw_and_query_preflight() {
        let zero = VerifyLimits {
            max_proof_bytes: 0,
            ..candidate_limits()
        };
        assert!(matches!(
            decode_and_verify_with_allocation_committed(&air(), &[0xff], zero, usize::MAX),
            Err(Error::VerifierLimitExceeded {
                limit: "max_proof_bytes",
                ..
            })
        ));
        let restricted = VerifyLimits {
            max_queries: 374,
            ..candidate_limits()
        };
        assert!(
            decode_and_verify_with_allocation_committed(
                &CandidateShape,
                &[0xff],
                restricted,
                usize::MAX
            )
            .is_err()
        );
    }
}
