//! Private, non-authorizing SHA-256 helper shape prerequisite.
//!
//! This module deliberately does not expose a digest, proof, verifier, or
//! production-acceptance API. It pins the nine SHA-256 messages already
//! implied by `helper_relation`, models the tenth message as the exact raw DER
//! `TBSCertificate` slice under an externally governed byte cap, and records
//! the smallest reviewed Table16 child shape at `k = 16`.
//!
//! The reviewed Table16 implementation is neither an eight-advice nor a
//! current-query circuit. Even an optimistic, zero-cost current-query
//! projection of one lane has a 5,344-byte augmented proof. The nine fixed
//! jobs require three lanes at `k = 16`, whose optimistic projection is
//! 12,192 bytes. Both exceed the 3,200-byte helper-proof envelope, so this
//! prerequisite remains fail-closed pending a separately reviewed recursive
//! compression circuit. The projection below is evidence, not a circuit.

use std::{collections::BTreeMap, fmt};

const SHA256_BLOCK_BYTES: usize = 64;
const SHA256_LENGTH_BYTES: usize = 8;
const SHA256_MARKER_BYTES: usize = 1;

const TABLE16_ROWS_PER_BLOCK_V1: usize = 2_265;
const TABLE16_IV_ROWS_PER_JOB_V1: usize = 16;
const TABLE16_DIGEST_ROWS_PER_JOB_V1: usize = 4;
const K16_USABLE_ROWS_V1: usize = (1 << 16) - 9;

pub(super) const OFFLINE_CASH_SHA256_K_V1: u32 = 16;
pub(super) const OFFLINE_CASH_SHA256_MAX_ADVICE_V1: usize = 8;
pub(super) const OFFLINE_CASH_SHA256_MAX_AUGMENTED_PROOF_BYTES_V1: usize = 3_200;

const fn framed_len(domain_len: usize, fields: &[usize]) -> usize {
    let mut length = 8 + domain_len;
    let mut index = 0;
    while index < fields.len() {
        length += 8 + fields[index];
        index += 1;
    }
    length
}

const CURRENT_GUARD_BYTES: usize = framed_len(42, &[1, 32, 32, 32, 32, 32, 32, 32, 8]);
const NEXT_GUARD_BYTES: usize = framed_len(39, &[1, 32, 32, 32, 32, 32, 32, 32, 32, 32, 8]);
const PLATFORM_MESSAGE_BYTES: usize =
    framed_len(45, &[1, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 8, 8]);
const GUARD_USE_CLAIM_BYTES: usize =
    framed_len(44, &[1, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 8, 8, 32]);
const PLATFORM_BIND_CLAIM_BYTES: usize = framed_len(48, &[32, 32, 32, 32, 32, 32, 32, 32]);
const ANDROID_KEY_CERT_CLAIM_BYTES: usize =
    framed_len(51, &[32, 32, 32, 32, 32, 32, 32, 32, 17, 29, 4, 7, 4]);
const GUARD_BUNDLE_BYTES: usize = framed_len(
    41,
    &[
        1, 1, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 8, 8, 32, 32, 32,
    ],
);

/// The exact helper SHA jobs in canonical routing order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(super) enum OfflineCashSha256JobV1 {
    CurrentGuardBinding = 0,
    NextGuardBinding = 1,
    PlatformMessage = 2,
    PlatformPublicKeySec1 = 3,
    GuardUseClaim = 4,
    PlatformBindClaim = 5,
    AndroidIssuerPublicKeySec1 = 6,
    AndroidKeyCertClaim = 7,
    GuardBundle = 8,
    RawTbsCertificate = 9,
}

/// A fixed-length SHA job already present in the helper relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OfflineCashFixedSha256JobV1 {
    pub(super) job: OfflineCashSha256JobV1,
    pub(super) exact_message_bytes: usize,
    pub(super) compression_blocks: usize,
}

/// Nine existing jobs. The raw `TBSCertificate` job is intentionally absent:
/// it has no governed byte cap yet and must be supplied through
/// [`OfflineCashRawTbsSha256BoundV1`].
pub(super) const OFFLINE_CASH_FIXED_SHA256_JOBS_V1: [OfflineCashFixedSha256JobV1; 9] = [
    OfflineCashFixedSha256JobV1 {
        job: OfflineCashSha256JobV1::CurrentGuardBinding,
        exact_message_bytes: CURRENT_GUARD_BYTES,
        compression_blocks: 6,
    },
    OfflineCashFixedSha256JobV1 {
        job: OfflineCashSha256JobV1::NextGuardBinding,
        exact_message_bytes: NEXT_GUARD_BYTES,
        compression_blocks: 7,
    },
    OfflineCashFixedSha256JobV1 {
        job: OfflineCashSha256JobV1::PlatformMessage,
        exact_message_bytes: PLATFORM_MESSAGE_BYTES,
        compression_blocks: 8,
    },
    OfflineCashFixedSha256JobV1 {
        job: OfflineCashSha256JobV1::PlatformPublicKeySec1,
        exact_message_bytes: 65,
        compression_blocks: 2,
    },
    OfflineCashFixedSha256JobV1 {
        job: OfflineCashSha256JobV1::GuardUseClaim,
        exact_message_bytes: GUARD_USE_CLAIM_BYTES,
        compression_blocks: 9,
    },
    OfflineCashFixedSha256JobV1 {
        job: OfflineCashSha256JobV1::PlatformBindClaim,
        exact_message_bytes: PLATFORM_BIND_CLAIM_BYTES,
        compression_blocks: 7,
    },
    OfflineCashFixedSha256JobV1 {
        job: OfflineCashSha256JobV1::AndroidIssuerPublicKeySec1,
        exact_message_bytes: 65,
        compression_blocks: 2,
    },
    OfflineCashFixedSha256JobV1 {
        job: OfflineCashSha256JobV1::AndroidKeyCertClaim,
        exact_message_bytes: ANDROID_KEY_CERT_CLAIM_BYTES,
        compression_blocks: 8,
    },
    OfflineCashFixedSha256JobV1 {
        job: OfflineCashSha256JobV1::GuardBundle,
        exact_message_bytes: GUARD_BUNDLE_BYTES,
        compression_blocks: 10,
    },
];

/// Exact fail-closed errors for the private shape prerequisite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OfflineCashCompactSha256ErrorV1 {
    MissingGovernedRawTbsCap,
    EmptyRawTbsCertificate,
    EmptyGovernedRawTbsCap,
    RawTbsCertificateExceedsCap {
        exact_message_bytes: usize,
        governed_max_message_bytes: usize,
    },
    Sha256MessageLengthNotEncodable {
        message_bytes: usize,
    },
    ZeroTable16Lanes,
    ArithmeticOverflow,
}

impl fmt::Display for OfflineCashCompactSha256ErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingGovernedRawTbsCap => {
                formatter.write_str("raw TBSCertificate SHA-256 cap is not governed")
            }
            Self::EmptyRawTbsCertificate => {
                formatter.write_str("raw TBSCertificate cannot be empty")
            }
            Self::EmptyGovernedRawTbsCap => {
                formatter.write_str("raw TBSCertificate governed cap cannot be zero")
            }
            Self::RawTbsCertificateExceedsCap {
                exact_message_bytes,
                governed_max_message_bytes,
            } => write!(
                formatter,
                "raw TBSCertificate length {exact_message_bytes} exceeds governed cap {governed_max_message_bytes}"
            ),
            Self::Sha256MessageLengthNotEncodable { message_bytes } => write!(
                formatter,
                "SHA-256 cannot encode a {message_bytes}-byte message length"
            ),
            Self::ZeroTable16Lanes => {
                formatter.write_str("Table16 shape requires at least one lane")
            }
            Self::ArithmeticOverflow => formatter.write_str("SHA-256 shape arithmetic overflow"),
        }
    }
}

impl std::error::Error for OfflineCashCompactSha256ErrorV1 {}

fn sha256_compression_blocks(
    message_bytes: usize,
) -> Result<usize, OfflineCashCompactSha256ErrorV1> {
    let encoded_bytes = u64::try_from(message_bytes).map_err(|_| {
        OfflineCashCompactSha256ErrorV1::Sha256MessageLengthNotEncodable { message_bytes }
    })?;
    encoded_bytes.checked_mul(8).ok_or(
        OfflineCashCompactSha256ErrorV1::Sha256MessageLengthNotEncodable { message_bytes },
    )?;
    message_bytes
        .checked_add(SHA256_MARKER_BYTES + SHA256_LENGTH_BYTES)
        .map(|padded_minimum| padded_minimum.div_ceil(SHA256_BLOCK_BYTES))
        .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)
}

/// The tenth job's exact raw slice length and externally governed maximum.
///
/// There is no default cap. `None` is rejected so a local implementation
/// cannot silently reuse the helper framing cap or infer a DER limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OfflineCashRawTbsSha256BoundV1 {
    pub(super) exact_message_bytes: usize,
    pub(super) governed_max_message_bytes: usize,
    pub(super) exact_compression_blocks: usize,
    pub(super) maximum_compression_blocks: usize,
}

impl OfflineCashRawTbsSha256BoundV1 {
    pub(super) fn new(
        exact_message_bytes: usize,
        governed_max_message_bytes: Option<usize>,
    ) -> Result<Self, OfflineCashCompactSha256ErrorV1> {
        if exact_message_bytes == 0 {
            return Err(OfflineCashCompactSha256ErrorV1::EmptyRawTbsCertificate);
        }
        let governed_max_message_bytes = governed_max_message_bytes
            .ok_or(OfflineCashCompactSha256ErrorV1::MissingGovernedRawTbsCap)?;
        if governed_max_message_bytes == 0 {
            return Err(OfflineCashCompactSha256ErrorV1::EmptyGovernedRawTbsCap);
        }
        if exact_message_bytes > governed_max_message_bytes {
            return Err(
                OfflineCashCompactSha256ErrorV1::RawTbsCertificateExceedsCap {
                    exact_message_bytes,
                    governed_max_message_bytes,
                },
            );
        }
        let exact_compression_blocks = sha256_compression_blocks(exact_message_bytes)?;
        let maximum_compression_blocks = sha256_compression_blocks(governed_max_message_bytes)?;
        Ok(Self {
            exact_message_bytes,
            governed_max_message_bytes,
            exact_compression_blocks,
            maximum_compression_blocks,
        })
    }
}

/// All ten jobs after an external TBS cap has been supplied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OfflineCashSha256InventoryV1 {
    pub(super) raw_tbs: OfflineCashRawTbsSha256BoundV1,
}

impl OfflineCashSha256InventoryV1 {
    pub(super) const fn new(raw_tbs: OfflineCashRawTbsSha256BoundV1) -> Self {
        Self { raw_tbs }
    }

    pub(super) fn exact_block_counts(self) -> [usize; 10] {
        let mut counts = [0; 10];
        let mut index = 0;
        while index < OFFLINE_CASH_FIXED_SHA256_JOBS_V1.len() {
            counts[index] = OFFLINE_CASH_FIXED_SHA256_JOBS_V1[index].compression_blocks;
            index += 1;
        }
        counts[9] = self.raw_tbs.exact_compression_blocks;
        counts
    }

    pub(super) fn maximum_block_counts(self) -> [usize; 10] {
        let mut counts = self.exact_block_counts();
        counts[9] = self.raw_tbs.maximum_compression_blocks;
        counts
    }

    pub(super) fn exact_total_blocks(self) -> Result<usize, OfflineCashCompactSha256ErrorV1> {
        checked_block_sum(&self.exact_block_counts())
    }

    pub(super) fn maximum_total_blocks(self) -> Result<usize, OfflineCashCompactSha256ErrorV1> {
        checked_block_sum(&self.maximum_block_counts())
    }
}

fn checked_block_sum(block_counts: &[usize]) -> Result<usize, OfflineCashCompactSha256ErrorV1> {
    block_counts.iter().try_fold(0_usize, |total, blocks| {
        total
            .checked_add(*blocks)
            .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)
    })
}

fn add_lane_overhead(
    overhead: &mut BTreeMap<usize, usize>,
    lane: usize,
    rows: usize,
) -> Result<(), OfflineCashCompactSha256ErrorV1> {
    let prior = overhead.get(&lane).copied().unwrap_or(0);
    overhead.insert(
        lane,
        prior
            .checked_add(rows)
            .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)?,
    );
    Ok(())
}

/// Exact maximum advice-row load for the reviewed round-robin Table16 router.
///
/// Each block consumes 2,265 rows. A job assigns its 16-row IV on the first
/// block's lane and its four-row digest on the final block's lane.
fn table16_max_lane_rows(
    block_counts: &[usize],
    lanes: usize,
) -> Result<usize, OfflineCashCompactSha256ErrorV1> {
    if lanes == 0 {
        return Err(OfflineCashCompactSha256ErrorV1::ZeroTable16Lanes);
    }
    let mut global_blocks = 0_usize;
    let mut overhead = BTreeMap::new();
    for blocks in block_counts {
        if *blocks == 0 {
            return Err(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow);
        }
        add_lane_overhead(
            &mut overhead,
            global_blocks % lanes,
            TABLE16_IV_ROWS_PER_JOB_V1,
        )?;
        global_blocks = global_blocks
            .checked_add(*blocks)
            .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)?;
        add_lane_overhead(
            &mut overhead,
            (global_blocks - 1) % lanes,
            TABLE16_DIGEST_ROWS_PER_JOB_V1,
        )?;
    }

    let full_rounds = global_blocks / lanes;
    let remainder = global_blocks % lanes;
    let maximum_blocks = full_rounds + usize::from(remainder != 0);
    let mut maximum_rows = maximum_blocks
        .checked_mul(TABLE16_ROWS_PER_BLOCK_V1)
        .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)?;
    for (lane, extra_rows) in overhead {
        let lane_blocks = full_rounds + usize::from(lane < remainder);
        let lane_rows = lane_blocks
            .checked_mul(TABLE16_ROWS_PER_BLOCK_V1)
            .and_then(|rows| rows.checked_add(extra_rows))
            .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)?;
        maximum_rows = maximum_rows.max(lane_rows);
    }
    Ok(maximum_rows)
}

fn fixed_job_block_counts() -> [usize; 9] {
    OFFLINE_CASH_FIXED_SHA256_JOBS_V1.map(|job| job.compression_blocks)
}

fn fixed_job_minimum_table16_lanes() -> Result<usize, OfflineCashCompactSha256ErrorV1> {
    let block_counts = fixed_job_block_counts();
    let total_blocks = checked_block_sum(&block_counts)?;
    for lanes in 1..=total_blocks {
        if table16_max_lane_rows(&block_counts, lanes)? <= K16_USABLE_ROWS_V1 {
            return Ok(lanes);
        }
    }
    Err(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)
}

/// Whether the shape uses the reviewed Table16 rotations or the optimistic
/// zero-cost current-query projection used only as a lower bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OfflineCashSha256QueryModelV1 {
    ReviewedTable16,
    OptimisticCurrentOnlyLowerBound,
}

/// Exact direct-instance IPA transcript accounting for the reviewed Table16
/// family plus one public-output binding column/query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OfflineCashSha256ChildShapeV1 {
    pub(super) k: u32,
    pub(super) query_model: OfflineCashSha256QueryModelV1,
    pub(super) degree: usize,
    pub(super) lanes: usize,
    pub(super) advice_columns: usize,
    pub(super) advice_queries: usize,
    pub(super) instance_columns: usize,
    pub(super) instance_queries: usize,
    pub(super) fixed_columns: usize,
    pub(super) fixed_queries: usize,
    pub(super) selectors: usize,
    pub(super) lookup_arguments: usize,
    pub(super) equality_columns: usize,
    pub(super) permutation_chunks: usize,
    pub(super) quotient_pieces: usize,
    pub(super) opening_point_sets: usize,
    pub(super) proof_points: usize,
    pub(super) proof_scalars: usize,
    pub(super) raw_proof_bytes: usize,
    pub(super) augmented_proof_bytes: usize,
}

impl OfflineCashSha256ChildShapeV1 {
    pub(super) fn for_lanes(
        lanes: usize,
        query_model: OfflineCashSha256QueryModelV1,
    ) -> Result<Self, OfflineCashCompactSha256ErrorV1> {
        if lanes == 0 {
            return Err(OfflineCashCompactSha256ErrorV1::ZeroTable16Lanes);
        }
        let degree = 9_usize;
        let advice_columns = lanes
            .checked_mul(11)
            .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)?;
        let advice_queries = lanes
            .checked_mul(30)
            .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)?;
        let selectors = lanes
            .checked_mul(22)
            .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)?;
        let lookup_arguments = lanes
            .checked_mul(4)
            .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)?;
        let equality_columns = lanes
            .checked_mul(8)
            .and_then(|columns| columns.checked_add(1))
            .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)?;
        let permutation_chunk_size = degree - 2;
        let permutation_chunks = equality_columns.div_ceil(permutation_chunk_size);
        let opening_point_sets = match query_model {
            OfflineCashSha256QueryModelV1::ReviewedTable16 => 5,
            OfflineCashSha256QueryModelV1::OptimisticCurrentOnlyLowerBound => 4,
        };
        let ipa_commitments = usize::try_from(OFFLINE_CASH_SHA256_K_V1)
            .ok()
            .and_then(|rounds| rounds.checked_mul(2))
            .and_then(|commitments| commitments.checked_add(1))
            .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)?;
        let proof_points = advice_columns
            .checked_add(
                lookup_arguments
                    .checked_mul(3)
                    .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)?,
            )
            .and_then(|points| points.checked_add(permutation_chunks))
            .and_then(|points| points.checked_add(degree))
            .and_then(|points| points.checked_add(1))
            .and_then(|points| points.checked_add(ipa_commitments))
            .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)?;
        let permutation_evaluations = permutation_chunks
            .checked_mul(3)
            .and_then(|evaluations| evaluations.checked_sub(1))
            .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)?;
        let proof_scalars = advice_queries
            .checked_add(5) // fixed queries, including the output-binding query
            .and_then(|scalars| scalars.checked_add(selectors))
            .and_then(|scalars| scalars.checked_add(lookup_arguments.checked_mul(5)?))
            .and_then(|scalars| scalars.checked_add(permutation_evaluations))
            .and_then(|scalars| scalars.checked_add(equality_columns))
            .and_then(|scalars| scalars.checked_add(1))
            .and_then(|scalars| scalars.checked_add(opening_point_sets))
            .and_then(|scalars| scalars.checked_add(2))
            .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)?;
        let raw_proof_bytes = proof_points
            .checked_add(proof_scalars)
            .and_then(|elements| elements.checked_mul(32))
            .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)?;
        let augmented_proof_bytes = raw_proof_bytes
            .checked_add(32)
            .ok_or(OfflineCashCompactSha256ErrorV1::ArithmeticOverflow)?;
        Ok(Self {
            k: OFFLINE_CASH_SHA256_K_V1,
            query_model,
            degree,
            lanes,
            advice_columns,
            advice_queries,
            instance_columns: 1,
            instance_queries: 1,
            fixed_columns: 5,
            fixed_queries: 5,
            selectors,
            lookup_arguments,
            equality_columns,
            permutation_chunks,
            quotient_pieces: degree,
            opening_point_sets,
            proof_points,
            proof_scalars,
            raw_proof_bytes,
            augmented_proof_bytes,
        })
    }
}

/// Concrete reasons that this prerequisite cannot authorize helper proofs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OfflineCashCompactSha256BlockerV1 {
    MissingGovernedRawTbsCap,
    NoReviewedCurrentQueryCircuit,
    AdviceEnvelopeExceeded { actual: usize, maximum: usize },
    AugmentedProofEnvelopeExceeded { actual: usize, maximum: usize },
}

/// Exact evidence available before the raw TBS cap is governed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OfflineCashCompactSha256EvidenceV1 {
    pub(super) fixed_jobs: usize,
    pub(super) fixed_compression_blocks: usize,
    pub(super) minimum_table16_lanes: usize,
    pub(super) two_lane_max_rows: usize,
    pub(super) three_lane_max_rows: usize,
    pub(super) reviewed_shape: OfflineCashSha256ChildShapeV1,
    pub(super) optimistic_current_only_lower_bound: OfflineCashSha256ChildShapeV1,
}

impl OfflineCashCompactSha256EvidenceV1 {
    pub(super) fn fixed_jobs() -> Result<Self, OfflineCashCompactSha256ErrorV1> {
        let blocks = fixed_job_block_counts();
        let fixed_compression_blocks = checked_block_sum(&blocks)?;
        let minimum_table16_lanes = fixed_job_minimum_table16_lanes()?;
        let reviewed_shape = OfflineCashSha256ChildShapeV1::for_lanes(
            minimum_table16_lanes,
            OfflineCashSha256QueryModelV1::ReviewedTable16,
        )?;
        let optimistic_current_only_lower_bound = OfflineCashSha256ChildShapeV1::for_lanes(
            minimum_table16_lanes,
            OfflineCashSha256QueryModelV1::OptimisticCurrentOnlyLowerBound,
        )?;
        Ok(Self {
            fixed_jobs: OFFLINE_CASH_FIXED_SHA256_JOBS_V1.len(),
            fixed_compression_blocks,
            minimum_table16_lanes,
            two_lane_max_rows: table16_max_lane_rows(&blocks, 2)?,
            three_lane_max_rows: table16_max_lane_rows(&blocks, 3)?,
            reviewed_shape,
            optimistic_current_only_lower_bound,
        })
    }

    pub(super) const fn blockers_without_governed_tbs_cap(
        self,
    ) -> [OfflineCashCompactSha256BlockerV1; 4] {
        [
            OfflineCashCompactSha256BlockerV1::MissingGovernedRawTbsCap,
            OfflineCashCompactSha256BlockerV1::NoReviewedCurrentQueryCircuit,
            OfflineCashCompactSha256BlockerV1::AdviceEnvelopeExceeded {
                actual: self.optimistic_current_only_lower_bound.advice_columns,
                maximum: OFFLINE_CASH_SHA256_MAX_ADVICE_V1,
            },
            OfflineCashCompactSha256BlockerV1::AugmentedProofEnvelopeExceeded {
                actual: self
                    .optimistic_current_only_lower_bound
                    .augmented_proof_bytes,
                maximum: OFFLINE_CASH_SHA256_MAX_AUGMENTED_PROOF_BYTES_V1,
            },
        ]
    }
}

#[cfg(test)]
#[path = "sha256_compact_tests.rs"]
mod tests;
