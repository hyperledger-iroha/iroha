//! Necessary decode-policy minima checked before private quantity proving.
//!
//! These are lower bounds, not a sufficient budget or an estimate of the final
//! decoder's charges. Every child opens 375 distinct current rows of 342 fixed
//! eight-byte values. Their payload alone costs at least 1,026,000 bytes both in
//! the canonical frame and in the decoded row vectors. The transport owns the
//! complete carrier as a `Vec<u8>`, so that raw row payload also supplies a lower
//! bound on its sequence, field, cumulative-element and allocation requirements.
//! Other fields, repeated decodes, scratch and codec charges only increase work.
//!
//! This preflight does not enter or consume a Norito scope. The final verifier
//! must still enforce actual counts, deeper nesting, cumulative charges and any
//! stricter or partially consumed inherited budget.

use super::{VerificationLimits, check, invalid, mul};
use crate::Result;

const DISTINCT_CURRENT_ROWS: usize = 375;
const ROW_VALUES: usize = 342;
const VALUE_BYTES: usize = core::mem::size_of::<u64>();
const CHILD_ROW_PAYLOAD_BYTES: usize = DISTINCT_CURRENT_ROWS * ROW_VALUES * VALUE_BYTES;

/// Reject explicit policy that cannot decode even the mandatory row payloads.
/// Passing these necessary minima does not guarantee the final decode succeeds.
pub(super) fn preflight_decode_policy(
    count: usize,
    verification: VerificationLimits,
) -> Result<()> {
    if count == 0 {
        return Err(invalid("quantity decode policy requires a nonempty bundle"));
    }
    let total_rows = mul(count, CHILD_ROW_PAYLOAD_BYTES)?;
    check(
        "max_compact_producer_segment_decode_allocation_charges",
        CHILD_ROW_PAYLOAD_BYTES,
        verification.max_segment_decode_allocation_charges,
    )?;
    check(
        "max_compact_producer_bundle_decode_allocation_charges",
        total_rows,
        verification.bundle.max_total_decode_allocation_charges,
    )?;

    // The outer transport and enclosing whole-request scope both observe the
    // complete owned raw bundle byte vector. One byte is one sequence element;
    // neither scope can decode a successful artifact below these same minima.
    for (limits, names) in [
        (
            verification.transport.norito,
            [
                "max_compact_producer_transport_decode_sequence_elements",
                "max_compact_producer_transport_decode_field_bytes",
                "max_compact_producer_transport_decode_total_elements",
                "max_compact_producer_transport_decode_allocation_charges",
                "max_compact_producer_transport_decode_nesting_depth",
            ],
        ),
        (
            verification.total_decode,
            [
                "max_compact_producer_total_decode_sequence_elements",
                "max_compact_producer_total_decode_field_bytes",
                "max_compact_producer_total_decode_total_elements",
                "max_compact_producer_total_decode_allocation_charges",
                "max_compact_producer_total_decode_nesting_depth",
            ],
        ),
    ] {
        for (name, actual, maximum) in [
            (names[0], total_rows, limits.max_sequence_elements()),
            (names[1], total_rows, limits.max_field_bytes()),
            (names[2], total_rows, limits.max_total_elements()),
            (names[3], total_rows, limits.max_total_allocated_bytes()),
            // One nested field is unavoidable. Exact deeper requirements stay
            // with the canonical decoder, rather than being duplicated here.
            (names[4], 1, limits.max_nesting_depth()),
        ] {
            check(name, actual, maximum)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Error, VerifyLimits, gadgets::public_transfer_statement::PublicTransferLimits,
        offline_compact::BundleVerificationLimits,
    };
    use iroha_data_model::fastpq::FastpqCompactArtifactDecodeLimits;
    use norito::core::DecodeLimits;

    fn policy(count: usize) -> VerificationLimits {
        let bytes = count * CHILD_ROW_PAYLOAD_BYTES;
        let decode = DecodeLimits::new(bytes, bytes, bytes, bytes, 1);
        VerificationLimits {
            transport: FastpqCompactArtifactDecodeLimits {
                max_wire_bytes: usize::MAX,
                max_bundle_frame_bytes: usize::MAX,
                norito: decode,
            },
            public_statement: PublicTransferLimits::default(),
            bundle: BundleVerificationLimits {
                max_segments: count,
                max_wire_bytes: usize::MAX,
                max_total_segment_bytes: usize::MAX,
                max_total_statement_bytes: usize::MAX,
                max_total_queries: count * DISTINCT_CURRENT_ROWS,
                max_total_decode_allocation_charges: bytes,
                segment: VerifyLimits::default(),
            },
            max_segment_decode_allocation_charges: CHILD_ROW_PAYLOAD_BYTES,
            total_decode: decode,
        }
    }

    fn assert_limit(result: Result<()>, name: &str, minimum: usize, cap: usize) {
        assert!(matches!(
            result,
            Err(Error::VerifierLimitExceeded { limit, actual, max })
                if limit == name && actual == minimum && max == cap
        ));
    }

    #[test]
    fn decode_policy_necessary_minima_are_inclusive_and_scale_with_all_children() {
        assert_eq!(CHILD_ROW_PAYLOAD_BYTES, 1_026_000);
        for count in [1, 2, 128] {
            let exact = policy(count);
            preflight_decode_policy(count, exact).unwrap();
            for (name, child) in [
                (
                    "max_compact_producer_segment_decode_allocation_charges",
                    true,
                ),
                (
                    "max_compact_producer_bundle_decode_allocation_charges",
                    false,
                ),
            ] {
                let minimum = if child {
                    CHILD_ROW_PAYLOAD_BYTES
                } else {
                    count * CHILD_ROW_PAYLOAD_BYTES
                };
                for cap in [0, minimum - 1] {
                    let mut limited = exact;
                    if child {
                        limited.max_segment_decode_allocation_charges = cap;
                    } else {
                        limited.bundle.max_total_decode_allocation_charges = cap;
                    }
                    assert_limit(preflight_decode_policy(count, limited), name, minimum, cap);
                }
            }
        }
    }

    #[test]
    fn decode_policy_checks_all_five_transport_and_total_budget_dimensions() {
        let count = 2;
        let minimum_bytes = count * CHILD_ROW_PAYLOAD_BYTES;
        let names = [
            [
                "max_compact_producer_transport_decode_sequence_elements",
                "max_compact_producer_transport_decode_field_bytes",
                "max_compact_producer_transport_decode_total_elements",
                "max_compact_producer_transport_decode_allocation_charges",
                "max_compact_producer_transport_decode_nesting_depth",
            ],
            [
                "max_compact_producer_total_decode_sequence_elements",
                "max_compact_producer_total_decode_field_bytes",
                "max_compact_producer_total_decode_total_elements",
                "max_compact_producer_total_decode_allocation_charges",
                "max_compact_producer_total_decode_nesting_depth",
            ],
        ];
        for (scope, names) in names.into_iter().enumerate() {
            for (dimension, name) in names.into_iter().enumerate() {
                let minimum = if dimension == 4 { 1 } else { minimum_bytes };
                for cap in [0, minimum - 1] {
                    let mut values = [minimum_bytes; 5];
                    values[4] = 1;
                    values[dimension] = cap;
                    let decode =
                        DecodeLimits::new(values[0], values[1], values[2], values[3], values[4]);
                    let mut limited = policy(count);
                    if scope == 0 {
                        limited.transport.norito = decode;
                    } else {
                        limited.total_decode = decode;
                    }
                    assert_limit(preflight_decode_policy(count, limited), name, minimum, cap);
                }
            }
        }
    }

    #[test]
    fn decode_policy_rejects_empty_or_overflowing_cumulative_shapes() {
        assert!(matches!(
            preflight_decode_policy(0, policy(1)),
            Err(Error::TransferInvariant { .. })
        ));
        assert!(matches!(
            preflight_decode_policy(usize::MAX / CHILD_ROW_PAYLOAD_BYTES + 1, policy(1)),
            Err(Error::TransferInvariant { .. })
        ));
    }

    #[test]
    fn decode_policy_does_not_consume_or_relax_inherited_norito_budgets() {
        let raw = norito::encode_canonical(&vec![1_u8]).unwrap();
        let deny = DecodeLimits::new(0, 0, 0, 0, 0);
        let (result, measured) = norito::core::with_decode_limits_measured(deny, || {
            preflight_decode_policy(1, policy(1))
        });
        result.unwrap();
        assert_eq!(measured.total_allocated_bytes(), 0);
        norito::core::with_decode_limits_scope(deny, || {
            preflight_decode_policy(1, policy(1)).unwrap();
            assert!(norito::decode_canonical::<Vec<u8>>(&raw).is_err());
        });
        assert_eq!(norito::decode_canonical::<Vec<u8>>(&raw).unwrap(), vec![1]);
    }
}
