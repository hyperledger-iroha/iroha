//! Unbiased RNS residue sampling from canonical Goldilocks digest words.
//!
//! A six-lane digest word is in `[0, p_G)`, not `[0, 2^64)`. The accepted
//! interval must therefore contain an exact multiple of the requested modulus
//! below `p_G`. Callers retain their exact bounded attempts and independent
//! typed digest domain for every attempt and coefficient.

use fastpq_isi::poseidon::FIELD_MODULUS;

use super::{
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1, ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1,
        ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1, ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1,
    },
    rns_native_proof_hash::{
        RnsNativeProofDigestV1, RnsNativeProofHashContextV1, RnsNativeProofHashPhaseV1,
        RnsNativeProofHashPositionV1, RnsNativeProofHashRoleV1,
    },
    rns_native_qpcs_prefix::Fq2V1,
};

/// The existing native verifier's exact retry bound, including rejected tails.
pub(super) const MAX_CHALLENGE_ATTEMPTS_V1: u16 = 256;

/// A verifier-derived scalar coordinate in the sole native challenge schedule.
///
/// All variants are checked against the current fixed profile before hashing.
/// The enum selects a mathematical role, never a caller-selected domain label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeChallengeCoordinateV1 {
    /// One of the 160 distinct initial half-domain query positions.
    Query { ordinal: u16 },
    /// One of five base-field relation points for one of the forty limbs.
    RelationPoint { limb: u8, repetition: u8 },
    /// One of two distinct nonzero base-field RLWE aggregation coefficients.
    Aggregation {
        limb: u8,
        repetition: u8,
        coefficient: u8,
    },
    /// A base component of one of two Fq2 batching coefficients per row.
    Batch {
        limb: u8,
        row: u8,
        coefficient: u8,
        component: u8,
    },
    /// A base component of the Fq2 folding challenge for one exact layer/row.
    FriFold {
        layer: u8,
        limb: u8,
        row: u8,
        component: u8,
    },
}

impl RnsNativeChallengeCoordinateV1 {
    fn frame(self) -> Result<(&'static [u8], [u8; 5], u64), RnsNativeProofSamplingErrorV1> {
        let invalid = RnsNativeProofSamplingErrorV1::InvalidCoordinate;
        match self {
            Self::Query { ordinal } if ordinal < ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1 => {
                let ordinal = ordinal.to_be_bytes();
                Ok((
                    b"query-index",
                    [ordinal[0], ordinal[1], 0, 0, 0],
                    1_u64 << (ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1 - 1),
                ))
            }
            Self::RelationPoint { limb, repetition } if repetition < 5 => {
                let modulus = *ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1
                    .get(usize::from(limb))
                    .ok_or(invalid)?;
                Ok((b"relation-point", [limb, repetition, 0, 0, 0], modulus))
            }
            Self::Aggregation {
                limb,
                repetition,
                coefficient,
            } if repetition < 5 && coefficient < 2 => {
                let modulus = *ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1
                    .get(usize::from(limb))
                    .ok_or(invalid)?;
                Ok((
                    b"rlwe-aggregation",
                    [limb, repetition, coefficient, 0, 0],
                    modulus,
                ))
            }
            Self::Batch {
                limb,
                row,
                coefficient,
                component,
            } if row < 10 && coefficient < 2 && component < 2 => {
                let modulus = *ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1
                    .get(usize::from(limb))
                    .ok_or(invalid)?;
                Ok((
                    b"batch-coefficient",
                    [limb, row, coefficient, component, 0],
                    modulus,
                ))
            }
            Self::FriFold {
                layer,
                limb,
                row,
                component,
            } if layer < ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1 && row < 10 && component < 2 => {
                let modulus = *ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1
                    .get(usize::from(limb))
                    .ok_or(invalid)?;
                Ok((b"fri-fold", [layer, limb, row, component, 0], modulus))
            }
            _ => Err(invalid),
        }
    }
}

/// Invalid target modulus or noncanonical digest word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeProofSamplingErrorV1 {
    /// The requested residue domain is zero or larger than Goldilocks.
    InvalidModulus,
    /// The purported digest word is outside its canonical Goldilocks field.
    NonCanonicalDigestWord,
    /// An index, coefficient component, or retry is outside the fixed schedule.
    InvalidCoordinate,
    /// The exact shared hash context rejected the field frame.
    InvalidHashFrame,
    /// All 256 independently framed attempts were rejected.
    AttemptsExhausted,
}

/// Sample one exact coordinate and attempt from its complete 48-byte seed.
///
/// The candidate is the first canonical Goldilocks word of the shared hash,
/// not a big-endian interpretation of bytes or a truncated proof commitment.
/// The complete seed, role, coordinates, target modulus, and attempt are bound.
pub(super) fn sample_challenge_attempt_v1(
    context: &RnsNativeProofHashContextV1,
    seed: RnsNativeProofDigestV1,
    coordinate: RnsNativeChallengeCoordinateV1,
    attempt: u16,
) -> Result<Option<u64>, RnsNativeProofSamplingErrorV1> {
    if attempt >= MAX_CHALLENGE_ATTEMPTS_V1 {
        return Err(RnsNativeProofSamplingErrorV1::InvalidCoordinate);
    }
    let (role, coordinates, modulus) = coordinate.frame()?;
    let digest = context
        .hash(
            RnsNativeProofHashRoleV1::Transcript,
            RnsNativeProofHashPhaseV1::Challenge,
            RnsNativeProofHashPositionV1 {
                level: 0,
                index: 0,
                counter: u64::from(attempt),
            },
            &[
                role,
                &coordinates,
                &modulus.to_be_bytes(),
                &seed.to_le_bytes(),
            ],
        )
        .map_err(|_| RnsNativeProofSamplingErrorV1::InvalidHashFrame)?;
    sample_goldilocks_word_modulus_v1(digest.words()[0], modulus)
}

/// Derive both separately framed components at the same bounded attempt.
///
/// `coordinate` must identify component zero of one exact Batch or FriFold
/// challenge. A zero pair and either rejected tail consume the next attempt.
pub(super) fn derive_fq2_challenge_v1(
    context: &RnsNativeProofHashContextV1,
    seed: RnsNativeProofDigestV1,
    coordinate: RnsNativeChallengeCoordinateV1,
) -> Result<Fq2V1, RnsNativeProofSamplingErrorV1> {
    let second = match coordinate {
        RnsNativeChallengeCoordinateV1::Batch {
            limb,
            row,
            coefficient,
            component: 0,
        } => RnsNativeChallengeCoordinateV1::Batch {
            limb,
            row,
            coefficient,
            component: 1,
        },
        RnsNativeChallengeCoordinateV1::FriFold {
            layer,
            limb,
            row,
            component: 0,
        } => RnsNativeChallengeCoordinateV1::FriFold {
            layer,
            limb,
            row,
            component: 1,
        },
        _ => return Err(RnsNativeProofSamplingErrorV1::InvalidCoordinate),
    };
    coordinate.frame()?;
    derive_fq2_with_v1(|component, attempt| {
        sample_challenge_attempt_v1(
            context,
            seed,
            if component == 0 { coordinate } else { second },
            attempt,
        )
    })
}

fn derive_fq2_with_v1(
    mut sample: impl FnMut(u8, u16) -> Result<Option<u64>, RnsNativeProofSamplingErrorV1>,
) -> Result<Fq2V1, RnsNativeProofSamplingErrorV1> {
    for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V1 {
        let c0 = sample(0, attempt)?;
        let c1 = sample(1, attempt)?;
        if let (Some(c0), Some(c1)) = (c0, c1) {
            let value = Fq2V1 { c0, c1 };
            if value != Fq2V1::ZERO {
                return Ok(value);
            }
        }
    }
    Err(RnsNativeProofSamplingErrorV1::AttemptsExhausted)
}

/// One canonical RLWE aggregation seed shared by source and direct verifiers.
///
/// The public formula/mapping identities and full prior transcript seed are
/// separately framed once; both consumers derive identical current coordinates.
#[derive(Clone, Copy)]
pub(super) struct RnsNativeAggregationSamplerV1 {
    context: RnsNativeProofHashContextV1,
    seed: RnsNativeProofDigestV1,
}

impl RnsNativeAggregationSamplerV1 {
    pub(super) fn new(
        parameter_digest: [u8; 32],
        aggregation_seed: RnsNativeProofDigestV1,
        formula_digest: [u8; 32],
        mapping_digest: [u8; 32],
    ) -> Result<Self, RnsNativeProofSamplingErrorV1> {
        let context = RnsNativeProofHashContextV1::canonical()
            .map_err(|_| RnsNativeProofSamplingErrorV1::InvalidHashFrame)?;
        if parameter_digest != context.parameter_digest()
            || aggregation_seed == RnsNativeProofDigestV1::ZERO
            || formula_digest == [0; 32]
            || mapping_digest == [0; 32]
        {
            return Err(RnsNativeProofSamplingErrorV1::InvalidCoordinate);
        }
        let seed = context
            .hash(
                RnsNativeProofHashRoleV1::Transcript,
                RnsNativeProofHashPhaseV1::Binding,
                RnsNativeProofHashPositionV1 {
                    level: 3,
                    index: 0,
                    counter: 0,
                },
                &[
                    b"native-rlwe-aggregation-seed",
                    aggregation_seed.as_bytes(),
                    &formula_digest,
                    &mapping_digest,
                ],
            )
            .map_err(|_| RnsNativeProofSamplingErrorV1::InvalidHashFrame)?;
        if seed == RnsNativeProofDigestV1::ZERO {
            return Err(RnsNativeProofSamplingErrorV1::InvalidHashFrame);
        }
        Ok(Self { context, seed })
    }

    pub(super) fn derive(
        &self,
        limb: usize,
        repetition: usize,
        coefficient: u8,
        used: &[u64],
    ) -> Result<u64, RnsNativeProofSamplingErrorV1> {
        let coordinate = RnsNativeChallengeCoordinateV1::Aggregation {
            limb: u8::try_from(limb)
                .map_err(|_| RnsNativeProofSamplingErrorV1::InvalidCoordinate)?,
            repetition: u8::try_from(repetition)
                .map_err(|_| RnsNativeProofSamplingErrorV1::InvalidCoordinate)?,
            coefficient,
        };
        let (_, _, modulus) = coordinate.frame()?;
        if used.len() >= 10
            || used.iter().enumerate().any(|(index, value)| {
                *value == 0 || *value >= modulus || used[..index].contains(value)
            })
        {
            return Err(RnsNativeProofSamplingErrorV1::InvalidCoordinate);
        }
        derive_nonzero_distinct_with_v1(used, |attempt| {
            sample_challenge_attempt_v1(&self.context, self.seed, coordinate, attempt)
        })
    }
}

fn derive_nonzero_distinct_with_v1(
    used: &[u64],
    mut sample: impl FnMut(u16) -> Result<Option<u64>, RnsNativeProofSamplingErrorV1>,
) -> Result<u64, RnsNativeProofSamplingErrorV1> {
    for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V1 {
        if let Some(value) = sample(attempt)? {
            if value != 0 && !used.contains(&value) {
                return Ok(value);
            }
        }
    }
    Err(RnsNativeProofSamplingErrorV1::AttemptsExhausted)
}

/// Derive the exact distinct query set with a bounded attempt counter per query.
pub(super) fn derive_query_indices_v1(
    context: &RnsNativeProofHashContextV1,
    seed: RnsNativeProofDigestV1,
) -> Result<[u32; ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1 as usize], RnsNativeProofSamplingErrorV1> {
    derive_query_indices_with_v1(|ordinal, attempt| {
        sample_challenge_attempt_v1(
            context,
            seed,
            RnsNativeChallengeCoordinateV1::Query { ordinal },
            attempt,
        )
    })
}

pub(super) fn derive_query_indices_with_v1(
    mut sample: impl FnMut(u16, u16) -> Result<Option<u64>, RnsNativeProofSamplingErrorV1>,
) -> Result<[u32; ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1 as usize], RnsNativeProofSamplingErrorV1> {
    let mut queries = [0_u32; ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1 as usize];
    let bound = 1_u64 << (ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1 - 1);
    for ordinal in 0..queries.len() {
        let mut accepted = None;
        for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V1 {
            let Some(value) = sample(ordinal as u16, attempt)? else {
                continue;
            };
            if value >= bound {
                return Err(RnsNativeProofSamplingErrorV1::InvalidCoordinate);
            }
            let value = u32::try_from(value)
                .map_err(|_| RnsNativeProofSamplingErrorV1::InvalidCoordinate)?;
            if !queries[..ordinal].contains(&value) {
                accepted = Some(value);
                break;
            }
        }
        queries[ordinal] = accepted.ok_or(RnsNativeProofSamplingErrorV1::AttemptsExhausted)?;
    }
    Ok(queries)
}

/// Map a canonical digest word uniformly into a requested residue domain.
///
/// `None` requests a new independently framed attempt. It never reduces the
/// incomplete tail, and it does not claim a whole proof is verified.
pub(super) fn sample_goldilocks_word_modulus_v1(
    word: u64,
    modulus: u64,
) -> Result<Option<u64>, RnsNativeProofSamplingErrorV1> {
    if modulus == 0 || modulus > FIELD_MODULUS {
        return Err(RnsNativeProofSamplingErrorV1::InvalidModulus);
    }
    if word >= FIELD_MODULUS {
        return Err(RnsNativeProofSamplingErrorV1::NonCanonicalDigestWord);
    }
    let exclusive_limit = FIELD_MODULUS - FIELD_MODULUS % modulus;
    Ok((word < exclusive_limit).then(|| word % modulus))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_current_coordinate_has_a_distinct_bounded_frame() {
        use std::collections::BTreeSet;
        let mut frames = BTreeSet::new();
        let mut insert = |coordinate: RnsNativeChallengeCoordinateV1| {
            assert!(frames.insert(coordinate.frame().unwrap()));
        };
        for ordinal in 0..ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1 {
            insert(RnsNativeChallengeCoordinateV1::Query { ordinal });
        }
        for limb in 0..40 {
            for repetition in 0..5 {
                insert(RnsNativeChallengeCoordinateV1::RelationPoint { limb, repetition });
                for coefficient in 0..2 {
                    insert(RnsNativeChallengeCoordinateV1::Aggregation {
                        limb,
                        repetition,
                        coefficient,
                    });
                }
            }
            for row in 0..10 {
                for component in 0..2 {
                    for coefficient in 0..2 {
                        insert(RnsNativeChallengeCoordinateV1::Batch {
                            limb,
                            row,
                            coefficient,
                            component,
                        });
                    }
                    for layer in 0..ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1 {
                        insert(RnsNativeChallengeCoordinateV1::FriFold {
                            layer,
                            limb,
                            row,
                            component,
                        });
                    }
                }
            }
        }
        assert_eq!(
            frames.len(),
            160 + 40 * 5 + 40 * 5 * 2 + 40 * 10 * 2 * 2 + 18 * 40 * 10 * 2
        );
        for coordinate in [
            RnsNativeChallengeCoordinateV1::Query { ordinal: 160 },
            RnsNativeChallengeCoordinateV1::RelationPoint {
                limb: 40,
                repetition: 0,
            },
            RnsNativeChallengeCoordinateV1::RelationPoint {
                limb: 0,
                repetition: 5,
            },
            RnsNativeChallengeCoordinateV1::Batch {
                limb: 40,
                row: 0,
                coefficient: 0,
                component: 0,
            },
            RnsNativeChallengeCoordinateV1::Batch {
                limb: 0,
                row: 10,
                coefficient: 0,
                component: 0,
            },
            RnsNativeChallengeCoordinateV1::Batch {
                limb: 0,
                row: 0,
                coefficient: 2,
                component: 0,
            },
            RnsNativeChallengeCoordinateV1::Batch {
                limb: 0,
                row: 0,
                coefficient: 0,
                component: 2,
            },
            RnsNativeChallengeCoordinateV1::FriFold {
                layer: 18,
                limb: 0,
                row: 0,
                component: 0,
            },
            RnsNativeChallengeCoordinateV1::FriFold {
                layer: 0,
                limb: 40,
                row: 0,
                component: 0,
            },
            RnsNativeChallengeCoordinateV1::FriFold {
                layer: 0,
                limb: 0,
                row: 10,
                component: 0,
            },
            RnsNativeChallengeCoordinateV1::FriFold {
                layer: 0,
                limb: 0,
                row: 0,
                component: 2,
            },
        ] {
            assert_eq!(
                coordinate.frame(),
                Err(RnsNativeProofSamplingErrorV1::InvalidCoordinate)
            );
        }
    }

    #[test]
    fn attempts_use_canonical_shared_words_and_bind_the_whole_seed() {
        let context = RnsNativeProofHashContextV1::canonical().unwrap();
        let coordinate = RnsNativeChallengeCoordinateV1::FriFold {
            layer: 17,
            limb: 39,
            row: 9,
            component: 1,
        };
        let (role, coordinates, modulus) = coordinate.frame().unwrap();
        let seed = RnsNativeProofDigestV1::from_shared(
            fastpq_isi::GoldilocksDigest384V1::new([1, 2, 3, 4, 5, 6]).unwrap(),
        );
        for attempt in [0_u16, 1, 255] {
            let expected = context
                .hash(
                    RnsNativeProofHashRoleV1::Transcript,
                    RnsNativeProofHashPhaseV1::Challenge,
                    RnsNativeProofHashPositionV1 {
                        level: 0,
                        index: 0,
                        counter: u64::from(attempt),
                    },
                    &[
                        role,
                        &coordinates,
                        &modulus.to_be_bytes(),
                        &seed.to_le_bytes(),
                    ],
                )
                .unwrap();
            assert_eq!(
                sample_challenge_attempt_v1(&context, seed, coordinate, attempt),
                sample_goldilocks_word_modulus_v1(expected.words()[0], modulus)
            );
        }
        assert_eq!(
            sample_challenge_attempt_v1(&context, seed, coordinate, 256),
            Err(RnsNativeProofSamplingErrorV1::InvalidCoordinate)
        );
        // Only the final seed word changes: there is no 32-byte seed projection.
        let other_seed = RnsNativeProofDigestV1::from_shared(
            fastpq_isi::GoldilocksDigest384V1::new([1, 2, 3, 4, 5, 7]).unwrap(),
        );
        let queries = derive_query_indices_v1(&context, seed).unwrap();
        let other_queries = derive_query_indices_v1(&context, other_seed).unwrap();
        assert_ne!(queries, other_queries);
        assert_eq!(queries.len(), 160);
        for (index, query) in queries.iter().enumerate() {
            assert!(*query < 1 << 18);
            assert!(!queries[..index].contains(query));
        }
    }

    #[test]
    fn query_tail_and_duplicate_exhaustion_are_exact_and_fail_closed() {
        let mut attempts = Vec::new();
        let queries = derive_query_indices_with_v1(|ordinal, attempt| {
            if ordinal == 0 {
                attempts.push(attempt);
            }
            Ok((ordinal != 0 || attempt == 255).then_some(u64::from(ordinal)))
        })
        .unwrap();
        assert_eq!(attempts, (0..256).collect::<Vec<_>>());
        assert_eq!(queries, core::array::from_fn(|index| index as u32));
        let mut attempts = Vec::new();
        assert_eq!(
            derive_query_indices_with_v1(|ordinal, attempt| {
                attempts.push((ordinal, attempt));
                Ok(None)
            }),
            Err(RnsNativeProofSamplingErrorV1::AttemptsExhausted)
        );
        assert_eq!(
            attempts,
            (0..256).map(|attempt| (0, attempt)).collect::<Vec<_>>()
        );
        let mut attempts = Vec::new();
        assert_eq!(
            derive_query_indices_with_v1(|ordinal, attempt| {
                attempts.push((ordinal, attempt));
                Ok(Some(0))
            }),
            Err(RnsNativeProofSamplingErrorV1::AttemptsExhausted)
        );
        assert_eq!(attempts[0], (0, 0));
        assert_eq!(
            &attempts[1..],
            (0..256).map(|attempt| (1, attempt)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn query_sampler_propagates_invalid_values_and_hash_failures() {
        assert_eq!(
            derive_query_indices_with_v1(|_, _| Ok(Some(1 << 18))),
            Err(RnsNativeProofSamplingErrorV1::InvalidCoordinate)
        );
        let mut calls = 0;
        assert_eq!(
            derive_query_indices_with_v1(|_, _| {
                calls += 1;
                Err(RnsNativeProofSamplingErrorV1::InvalidHashFrame)
            }),
            Err(RnsNativeProofSamplingErrorV1::InvalidHashFrame)
        );
        assert_eq!(calls, 1);
    }

    #[test]
    fn every_native_rns_prime_rejects_the_incomplete_goldilocks_tail() {
        for modulus in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1 {
            let multiplicity = FIELD_MODULUS / modulus;
            let limit = multiplicity * modulus;
            assert_eq!(multiplicity, 15);
            assert_eq!(sample_goldilocks_word_modulus_v1(0, modulus), Ok(Some(0)));
            assert_eq!(
                sample_goldilocks_word_modulus_v1(limit - 1, modulus),
                Ok(Some(modulus - 1))
            );
            assert_eq!(sample_goldilocks_word_modulus_v1(limit, modulus), Ok(None));
            assert_eq!(
                sample_goldilocks_word_modulus_v1(FIELD_MODULUS - 1, modulus),
                Ok(None)
            );
            // Every residue has exactly the same fifteen accepted preimages.
            for residue in [0, 1, modulus / 2, modulus - 1] {
                for copy in 0..multiplicity {
                    assert_eq!(
                        sample_goldilocks_word_modulus_v1(copy * modulus + residue, modulus),
                        Ok(Some(residue))
                    );
                }
            }
            let retired_u64_limit = u64::MAX - u64::MAX % modulus;
            assert!(FIELD_MODULUS - 1 < retired_u64_limit);
        }
    }

    #[test]
    fn power_of_two_queries_reject_the_single_extra_goldilocks_element() {
        let query_bound = 1_u64 << 18;
        assert_eq!(FIELD_MODULUS % query_bound, 1);
        assert_eq!(
            sample_goldilocks_word_modulus_v1(FIELD_MODULUS - 2, query_bound),
            Ok(Some(query_bound - 1))
        );
        assert_eq!(
            sample_goldilocks_word_modulus_v1(FIELD_MODULUS - 1, query_bound),
            Ok(None)
        );
    }

    #[test]
    fn malformed_sampler_inputs_never_alias_canonical_values() {
        assert_eq!(
            sample_goldilocks_word_modulus_v1(FIELD_MODULUS, 17),
            Err(RnsNativeProofSamplingErrorV1::NonCanonicalDigestWord)
        );
        assert_eq!(
            sample_goldilocks_word_modulus_v1(u64::MAX, 17),
            Err(RnsNativeProofSamplingErrorV1::NonCanonicalDigestWord)
        );
        assert_eq!(
            sample_goldilocks_word_modulus_v1(0, 0),
            Err(RnsNativeProofSamplingErrorV1::InvalidModulus)
        );
        assert_eq!(
            sample_goldilocks_word_modulus_v1(0, FIELD_MODULUS + 1),
            Err(RnsNativeProofSamplingErrorV1::InvalidModulus)
        );
        assert_eq!(
            sample_goldilocks_word_modulus_v1(FIELD_MODULUS - 1, FIELD_MODULUS),
            Ok(Some(FIELD_MODULUS - 1))
        );
        assert_eq!(
            sample_goldilocks_word_modulus_v1(FIELD_MODULUS - 1, 1),
            Ok(Some(0))
        );
    }
    #[test]
    fn fq2_retries_both_components_at_one_attempt_and_rejects_zero_pairs() {
        let mut seen = Vec::new();
        let actual = derive_fq2_with_v1(|component, attempt| {
            seen.push((component, attempt));
            Ok(match (attempt, component) {
                (0, 0) | (1, 1) => None,
                (0, 1) | (1, 0) => Some(17),
                (2, _) => Some(0),
                (3, 0) => Some(0),
                (3, 1) => Some(23),
                _ => panic!("accepted nonzero pair must end retries"),
            })
        })
        .unwrap();
        assert_eq!(actual, Fq2V1 { c0: 0, c1: 23 });
        assert_eq!(
            seen,
            [
                (0, 0),
                (1, 0),
                (0, 1),
                (1, 1),
                (0, 2),
                (1, 2),
                (0, 3),
                (1, 3)
            ]
        );
    }

    #[test]
    fn fq2_exact_exhaustion_and_hash_errors_never_yield_a_challenge() {
        for sampled in [None, Some(0)] {
            let mut seen = Vec::new();
            assert_eq!(
                derive_fq2_with_v1(|component, attempt| {
                    seen.push((component, attempt));
                    Ok(sampled)
                }),
                Err(RnsNativeProofSamplingErrorV1::AttemptsExhausted)
            );
            assert_eq!(seen.len(), 512);
            for (ordinal, actual) in seen.into_iter().enumerate() {
                assert_eq!(actual, ((ordinal % 2) as u8, (ordinal / 2) as u16));
            }
        }
        for failed_component in 0..2 {
            let mut seen = Vec::new();
            assert_eq!(
                derive_fq2_with_v1(|component, attempt| {
                    seen.push((component, attempt));
                    if component == failed_component {
                        Err(RnsNativeProofSamplingErrorV1::InvalidHashFrame)
                    } else {
                        Ok(Some(0))
                    }
                }),
                Err(RnsNativeProofSamplingErrorV1::InvalidHashFrame)
            );
            assert_eq!(seen.len(), usize::from(failed_component) + 1);
            assert!(seen.iter().all(|&(_, attempt)| attempt == 0));
        }
    }

    #[test]
    fn fq2_joint_sampler_replays_actual_shared_hash_for_boundary_coordinates() {
        let context = RnsNativeProofHashContextV1::canonical().unwrap();
        let seed =
            super::super::rns_native_proof_hash::test_proof_digest_v1(b"joint-fq2-sampling", 0);
        for coordinate in [
            RnsNativeChallengeCoordinateV1::Batch {
                limb: 0,
                row: 0,
                coefficient: 0,
                component: 0,
            },
            RnsNativeChallengeCoordinateV1::Batch {
                limb: 39,
                row: 9,
                coefficient: 1,
                component: 0,
            },
            RnsNativeChallengeCoordinateV1::FriFold {
                layer: 0,
                limb: 0,
                row: 0,
                component: 0,
            },
            RnsNativeChallengeCoordinateV1::FriFold {
                layer: 17,
                limb: 39,
                row: 9,
                component: 0,
            },
        ] {
            let (role, mut axes, modulus) = coordinate.frame().unwrap();
            let component_index = 3;
            let mut expected = None;
            for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V1 {
                let mut values = [None; 2];
                for component in 0..2 {
                    axes[component_index] = component as u8;
                    let output = context
                        .hash(
                            RnsNativeProofHashRoleV1::Transcript,
                            RnsNativeProofHashPhaseV1::Challenge,
                            RnsNativeProofHashPositionV1 {
                                level: 0,
                                index: 0,
                                counter: u64::from(attempt),
                            },
                            &[role, &axes, &modulus.to_be_bytes(), seed.as_bytes()],
                        )
                        .unwrap();
                    let word = output.words()[0];
                    let limit = FIELD_MODULUS - FIELD_MODULUS % modulus;
                    values[component] = (word < limit).then_some(word % modulus);
                }
                if let [Some(c0), Some(c1)] = values {
                    if c0 != 0 || c1 != 0 {
                        expected = Some(Fq2V1 { c0, c1 });
                        break;
                    }
                }
            }
            let actual = derive_fq2_challenge_v1(&context, seed, coordinate).unwrap();
            assert_eq!(Some(actual), expected);
            assert!(actual.c0 < modulus && actual.c1 < modulus);
            assert_ne!(actual, Fq2V1::ZERO);
        }
        for coordinate in [
            RnsNativeChallengeCoordinateV1::Query { ordinal: 0 },
            RnsNativeChallengeCoordinateV1::RelationPoint {
                limb: 0,
                repetition: 0,
            },
            RnsNativeChallengeCoordinateV1::Batch {
                limb: 0,
                row: 0,
                coefficient: 0,
                component: 1,
            },
            RnsNativeChallengeCoordinateV1::FriFold {
                layer: 0,
                limb: 0,
                row: 0,
                component: 1,
            },
            RnsNativeChallengeCoordinateV1::Batch {
                limb: 40,
                row: 0,
                coefficient: 0,
                component: 0,
            },
            RnsNativeChallengeCoordinateV1::FriFold {
                layer: 18,
                limb: 0,
                row: 0,
                component: 0,
            },
        ] {
            assert_eq!(
                derive_fq2_challenge_v1(&context, seed, coordinate),
                Err(RnsNativeProofSamplingErrorV1::InvalidCoordinate)
            );
        }
    }
    #[test]
    fn aggregation_coordinates_reject_noncanonical_inputs_and_replay_shared_owner() {
        let context = RnsNativeProofHashContextV1::canonical().unwrap();
        let parameter = context.parameter_digest();
        let seed = super::super::rns_native_proof_hash::test_proof_digest_v1(b"rlwe-gamma-beta", 0);
        let sampler =
            RnsNativeAggregationSamplerV1::new(parameter, seed, [1; 32], [2; 32]).unwrap();
        let expected_seed = context
            .hash(
                RnsNativeProofHashRoleV1::Transcript,
                RnsNativeProofHashPhaseV1::Binding,
                RnsNativeProofHashPositionV1 {
                    level: 3,
                    index: 0,
                    counter: 0,
                },
                &[
                    b"native-rlwe-aggregation-seed",
                    seed.as_bytes(),
                    &[1; 32],
                    &[2; 32],
                ],
            )
            .unwrap();
        assert_eq!(sampler.seed, expected_seed);
        for limb in [0, 39] {
            let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb];
            let mut used = Vec::new();
            for repetition in 0..5 {
                for coefficient in 0..2 {
                    let actual = sampler
                        .derive(limb, repetition, coefficient, &used)
                        .unwrap();
                    let coordinate = RnsNativeChallengeCoordinateV1::Aggregation {
                        limb: limb as u8,
                        repetition: repetition as u8,
                        coefficient,
                    };
                    let expected = (0..MAX_CHALLENGE_ATTEMPTS_V1)
                        .find_map(|attempt| {
                            sample_challenge_attempt_v1(
                                &context,
                                expected_seed,
                                coordinate,
                                attempt,
                            )
                            .unwrap()
                            .filter(|value| *value != 0 && !used.contains(value))
                        })
                        .unwrap();
                    assert_eq!(actual, expected);
                    assert!(actual > 0 && actual < modulus);
                    used.push(actual);
                }
            }
            assert_eq!(
                sampler.derive(limb, 0, 0, &used),
                Err(RnsNativeProofSamplingErrorV1::InvalidCoordinate)
            );
        }
        for (limb, repetition, coefficient) in [(40, 0, 0), (0, 5, 0), (0, 0, 2), (256, 0, 0)] {
            assert_eq!(
                sampler.derive(limb, repetition, coefficient, &[]),
                Err(RnsNativeProofSamplingErrorV1::InvalidCoordinate)
            );
        }
        for invalid in [
            vec![0],
            vec![ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0]],
            vec![1, 1],
        ] {
            assert_eq!(
                sampler.derive(0, 0, 0, &invalid),
                Err(RnsNativeProofSamplingErrorV1::InvalidCoordinate)
            );
        }
        assert!(RnsNativeAggregationSamplerV1::new([0; 32], seed, [1; 32], [2; 32]).is_err());
        assert!(
            RnsNativeAggregationSamplerV1::new(
                parameter,
                RnsNativeProofDigestV1::ZERO,
                [1; 32],
                [2; 32]
            )
            .is_err()
        );
        assert!(RnsNativeAggregationSamplerV1::new(parameter, seed, [0; 32], [2; 32]).is_err());
        assert!(RnsNativeAggregationSamplerV1::new(parameter, seed, [1; 32], [0; 32]).is_err());
        for other in [
            RnsNativeAggregationSamplerV1::new(parameter, seed, [3; 32], [2; 32]).unwrap(),
            RnsNativeAggregationSamplerV1::new(parameter, seed, [1; 32], [3; 32]).unwrap(),
        ] {
            assert_ne!(sampler.seed, other.seed);
        }
    }

    #[test]
    fn nonzero_distinct_sampling_preserves_exact_attempts_and_error_propagation() {
        let mut seen = Vec::new();
        assert_eq!(
            derive_nonzero_distinct_with_v1(&[7], |attempt| {
                seen.push(attempt);
                Ok(match attempt {
                    0 => None,
                    1 => Some(0),
                    2 => Some(7),
                    3 => Some(9),
                    _ => panic!("accepted"),
                })
            }),
            Ok(9)
        );
        assert_eq!(seen, [0, 1, 2, 3]);
        for rejected in [None, Some(0), Some(7)] {
            let mut calls = 0;
            assert_eq!(
                derive_nonzero_distinct_with_v1(&[7], |attempt| {
                    assert_eq!(usize::from(attempt), calls);
                    calls += 1;
                    Ok(rejected)
                }),
                Err(RnsNativeProofSamplingErrorV1::AttemptsExhausted)
            );
            assert_eq!(calls, 256);
        }
        assert_eq!(
            derive_nonzero_distinct_with_v1(&[], |_| Err(
                RnsNativeProofSamplingErrorV1::InvalidHashFrame
            )),
            Err(RnsNativeProofSamplingErrorV1::InvalidHashFrame)
        );
    }
}
