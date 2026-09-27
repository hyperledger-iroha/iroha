//! SCCP v1 consensus parameters (`specs/sccp.md` §4.1).
//!
//! [`SccpParametersV1`] is one Taira-internal value stored in dedicated world state. It is written
//! only by the genesis-only `InitializeSccpV1` instruction and by the Parliament-enacted
//! `SetParameters` action, and both check every rule of the §4.1 table, including the joint
//! rules, against the complete new value with [`SccpParametersV1::validate`].

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Immutable upper bound on `roster_validity_ms` and on every roster a destination installs.
///
/// This is the contracts' `MAX_ROSTER_VALIDITY_MS` (30 d, §3.9).
pub const SCCP_MAX_ROSTER_VALIDITY_MS_V1: u64 = 2_592_000_000;
/// Grace period during which a destination still accepts the previous roster (24 h, §3.9).
pub const SCCP_PREVIOUS_ROSTER_GRACE_MS_V1: u64 = 86_400_000;
/// Maximum clock skew a destination tolerates on a roster's `valid_from_ms` (1 h, §3.9).
pub const SCCP_MAX_CLOCK_SKEW_MS_V1: u64 = 3_600_000;
/// Maximum SCCP messages (transfers and controls together) committed by one Taira block (§3.4).
pub const SCCP_MESSAGES_MAX_PER_BLOCK_V1: u32 = 512;
/// Smallest admitted `roster_max_age_ms` (1 h).
pub const SCCP_MIN_ROSTER_MAX_AGE_MS_V1: u64 = 3_600_000;
/// Rotation margin in the joint `roster_validity_ms` lower bound (1 d).
///
/// `2 × roster_max_age_ms + attestation_stall_ms + margin ≤ roster_validity_ms` leaves a
/// destination at least `roster_max_age + 1 d` to rotate (§5.1.5).
pub const SCCP_ROSTER_ROTATION_MARGIN_MS_V1: u64 = 86_400_000;
/// Smallest admitted `outbound_ttl_ms` (1 d).
pub const SCCP_MIN_OUTBOUND_TTL_MS_V1: u64 = 86_400_000;
/// Largest admitted `outbound_ttl_ms` (90 d).
pub const SCCP_MAX_OUTBOUND_TTL_MS_V1: u64 = 7_776_000_000;
/// Largest admitted `min_outbound_amount` in Taira units (1 000 000 XOR).
pub const SCCP_MAX_MIN_OUTBOUND_AMOUNT_V1: u128 = 1_000_000_000_000_000;
/// Largest admitted `inbound_self_claim_fee` in Taira units (10 XOR).
pub const SCCP_MAX_INBOUND_SELF_CLAIM_FEE_V1: u128 = 10_000_000_000;
/// Margin in the joint `attestation_retention_ms` lower bound (1 d past `outbound_ttl_ms`).
pub const SCCP_ATTESTATION_RETENTION_MARGIN_MS_V1: u64 = 86_400_000;
/// Largest admitted `attestation_retention_ms` (365 d).
pub const SCCP_MAX_ATTESTATION_RETENTION_MS_V1: u64 = 31_536_000_000;
/// Smallest admitted `attestation_stall_ms` (1 min).
pub const SCCP_MIN_ATTESTATION_STALL_MS_V1: u64 = 60_000;
/// Largest admitted `attestation_stall_ms` (1 d).
pub const SCCP_MAX_ATTESTATION_STALL_MS_V1: u64 = 86_400_000;
/// Smallest admitted `max_attestation_entries_per_instruction`.
///
/// The node attestor's default batch of 64 (§4.9) must always fit.
pub const SCCP_MIN_ATTESTATION_ENTRIES_PER_INSTRUCTION_V1: u32 = 64;
/// Largest admitted `max_attestation_entries_per_instruction`.
pub const SCCP_MAX_ATTESTATION_ENTRIES_PER_INSTRUCTION_V1: u32 = 1_024;
/// Smallest admitted `max_exempt_transactions_per_block`.
///
/// Two attestor keys per validator during a key rotation at the largest roster (2 × 31) plus a
/// reserve of 32 for keeper advances, key registrations, fault evidence and self-claims (§4.1).
pub const SCCP_MIN_EXEMPT_TRANSACTIONS_PER_BLOCK_V1: u32 = 94;
/// Largest admitted `max_exempt_transactions_per_block`.
pub const SCCP_MAX_EXEMPT_TRANSACTIONS_PER_BLOCK_V1: u32 = 1_024;

/// SCCP consensus parameters (`sccp_parameters` world state, §4.1).
///
/// Every field is Parliament-settable within its rule. Windows are milliseconds of Taira block
/// time, never block counts, because Iroha produces no empty blocks.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::params::SccpParametersV1")]
pub struct SccpParametersV1 {
    /// Value-moving switch: when false, recording fails and inbound settlement and outbound
    /// refunds stay `Pending`; keys, rosters, attestations, light clients and Parliament
    /// enactment keep running.
    pub enabled: bool,
    /// Heartbeat: force a new roster generation once a generation is this old.
    pub roster_max_age_ms: u64,
    /// Validity window of every roster generation (`valid_until = valid_from + validity`).
    pub roster_validity_ms: u64,
    /// Outbound deadline offset from the record block's creation time.
    pub outbound_ttl_ms: u64,
    /// Floor per outbound message, in Taira units (a decimal string in JSON).
    #[norito(json = "crate::json_helpers::u128_string")]
    pub min_outbound_amount: u128,
    /// Fee deducted from the proceeds of a fee-exempt inbound self-claim, in Taira units (a
    /// decimal string in JSON).
    #[norito(json = "crate::json_helpers::u128_string")]
    pub inbound_self_claim_fee: u128,
    /// Signature pruning horizon by block time.
    pub attestation_retention_ms: u64,
    /// Attestation window by block time before outbound records are refused and handoffs stall.
    pub attestation_stall_ms: u64,
    /// Bound on the entries of one `SubmitSccpAttestationsV1`.
    pub max_attestation_entries_per_instruction: u32,
    /// Cap on fee-exempt SCCP transactions per block.
    pub max_exempt_transactions_per_block: u32,
}

/// A violated rule of the §4.1 parameter table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SccpParametersError {
    /// `roster_max_age_ms` is below [`SCCP_MIN_ROSTER_MAX_AGE_MS_V1`].
    #[error("roster_max_age_ms must be at least 3 600 000 ms")]
    RosterMaxAgeTooShort,
    /// `2 × roster_max_age_ms + attestation_stall_ms + 86 400 000` exceeds `roster_validity_ms`.
    #[error(
        "roster_validity_ms must be at least 2 × roster_max_age_ms + attestation_stall_ms + 86 400 000 ms"
    )]
    RosterValidityTooShort,
    /// `roster_validity_ms` exceeds [`SCCP_MAX_ROSTER_VALIDITY_MS_V1`].
    #[error("roster_validity_ms must not exceed 2 592 000 000 ms")]
    RosterValidityTooLong,
    /// `outbound_ttl_ms` is outside 1 d..=90 d.
    #[error("outbound_ttl_ms must be within 86 400 000..=7 776 000 000 ms")]
    OutboundTtlOutOfRange,
    /// `min_outbound_amount` is outside `1..=10^15`.
    #[error("min_outbound_amount must be within 1..=10^15 Taira units")]
    MinOutboundAmountOutOfRange,
    /// `inbound_self_claim_fee` exceeds `10^10`.
    #[error("inbound_self_claim_fee must not exceed 10^10 Taira units")]
    InboundSelfClaimFeeTooLarge,
    /// `outbound_ttl_ms + 86 400 000` exceeds `attestation_retention_ms`.
    #[error("attestation_retention_ms must be at least outbound_ttl_ms + 86 400 000 ms")]
    AttestationRetentionTooShort,
    /// `attestation_retention_ms` exceeds [`SCCP_MAX_ATTESTATION_RETENTION_MS_V1`].
    #[error("attestation_retention_ms must not exceed 31 536 000 000 ms")]
    AttestationRetentionTooLong,
    /// `attestation_stall_ms` is outside `60 000..=86 400 000`.
    #[error("attestation_stall_ms must be within 60 000..=86 400 000 ms")]
    AttestationStallOutOfRange,
    /// `max_attestation_entries_per_instruction` is outside `64..=1 024`.
    #[error("max_attestation_entries_per_instruction must be within 64..=1 024")]
    AttestationEntriesOutOfRange,
    /// `max_exempt_transactions_per_block` is outside `94..=1 024`.
    #[error("max_exempt_transactions_per_block must be within 94..=1 024")]
    ExemptTransactionsOutOfRange,
}

impl SccpParametersV1 {
    /// Return the Taira genesis values of the §4.1 table.
    #[must_use]
    pub const fn taira_default() -> Self {
        Self {
            enabled: true,
            roster_max_age_ms: 86_400_000,
            roster_validity_ms: 1_209_600_000,
            outbound_ttl_ms: 604_800_000,
            min_outbound_amount: 1_000_000_000,
            inbound_self_claim_fee: 10_000_000,
            attestation_retention_ms: 2_592_000_000,
            attestation_stall_ms: 600_000,
            max_attestation_entries_per_instruction: 256,
            max_exempt_transactions_per_block: 128,
        }
    }

    /// Check every rule of the §4.1 table, including the joint rules.
    ///
    /// Joint bounds use checked arithmetic: a lower bound that overflows `u64` can never be met,
    /// so it reports the same violation as any other value below the bound.
    ///
    /// # Errors
    ///
    /// Returns the first violated rule in table order.
    pub fn validate(&self) -> Result<(), SccpParametersError> {
        if self.roster_max_age_ms < SCCP_MIN_ROSTER_MAX_AGE_MS_V1 {
            return Err(SccpParametersError::RosterMaxAgeTooShort);
        }
        let validity_floor = self
            .roster_max_age_ms
            .checked_mul(2)
            .and_then(|value| value.checked_add(self.attestation_stall_ms))
            .and_then(|value| value.checked_add(SCCP_ROSTER_ROTATION_MARGIN_MS_V1));
        if validity_floor.is_none_or(|floor| floor > self.roster_validity_ms) {
            return Err(SccpParametersError::RosterValidityTooShort);
        }
        if self.roster_validity_ms > SCCP_MAX_ROSTER_VALIDITY_MS_V1 {
            return Err(SccpParametersError::RosterValidityTooLong);
        }
        if !(SCCP_MIN_OUTBOUND_TTL_MS_V1..=SCCP_MAX_OUTBOUND_TTL_MS_V1)
            .contains(&self.outbound_ttl_ms)
        {
            return Err(SccpParametersError::OutboundTtlOutOfRange);
        }
        if !(1..=SCCP_MAX_MIN_OUTBOUND_AMOUNT_V1).contains(&self.min_outbound_amount) {
            return Err(SccpParametersError::MinOutboundAmountOutOfRange);
        }
        if self.inbound_self_claim_fee > SCCP_MAX_INBOUND_SELF_CLAIM_FEE_V1 {
            return Err(SccpParametersError::InboundSelfClaimFeeTooLarge);
        }
        let retention_floor = self
            .outbound_ttl_ms
            .checked_add(SCCP_ATTESTATION_RETENTION_MARGIN_MS_V1);
        if retention_floor.is_none_or(|floor| floor > self.attestation_retention_ms) {
            return Err(SccpParametersError::AttestationRetentionTooShort);
        }
        if self.attestation_retention_ms > SCCP_MAX_ATTESTATION_RETENTION_MS_V1 {
            return Err(SccpParametersError::AttestationRetentionTooLong);
        }
        if !(SCCP_MIN_ATTESTATION_STALL_MS_V1..=SCCP_MAX_ATTESTATION_STALL_MS_V1)
            .contains(&self.attestation_stall_ms)
        {
            return Err(SccpParametersError::AttestationStallOutOfRange);
        }
        if !(SCCP_MIN_ATTESTATION_ENTRIES_PER_INSTRUCTION_V1
            ..=SCCP_MAX_ATTESTATION_ENTRIES_PER_INSTRUCTION_V1)
            .contains(&self.max_attestation_entries_per_instruction)
        {
            return Err(SccpParametersError::AttestationEntriesOutOfRange);
        }
        if !(SCCP_MIN_EXEMPT_TRANSACTIONS_PER_BLOCK_V1..=SCCP_MAX_EXEMPT_TRANSACTIONS_PER_BLOCK_V1)
            .contains(&self.max_exempt_transactions_per_block)
        {
            return Err(SccpParametersError::ExemptTransactionsOutOfRange);
        }
        Ok(())
    }

    /// Return the first `u64` field above `maximum`, naming it for the proposal invariant.
    ///
    /// Parliament proposals must keep every `u64` exact in IEEE-754 binary64 SDK runtimes
    /// (`first_release_exact_json_u64_invariant_error`). The table rules already imply the bound
    /// for valid values; this check is independent of [`Self::validate`].
    #[must_use]
    pub fn first_json_u64_violation(&self, maximum: u64) -> Option<&'static str> {
        [
            (
                self.roster_max_age_ms,
                "SCCP parameters roster_max_age_ms exceeds the exact JSON integer maximum",
            ),
            (
                self.roster_validity_ms,
                "SCCP parameters roster_validity_ms exceeds the exact JSON integer maximum",
            ),
            (
                self.outbound_ttl_ms,
                "SCCP parameters outbound_ttl_ms exceeds the exact JSON integer maximum",
            ),
            (
                self.attestation_retention_ms,
                "SCCP parameters attestation_retention_ms exceeds the exact JSON integer maximum",
            ),
            (
                self.attestation_stall_ms,
                "SCCP parameters attestation_stall_ms exceeds the exact JSON integer maximum",
            ),
        ]
        .into_iter()
        .find_map(|(value, message)| (value > maximum).then_some(message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::codec::DecodeAll as _;

    const DAY_MS: u64 = 86_400_000;
    const JSON_MAX: u64 = (1_u64 << 53) - 1;

    fn with(update: impl FnOnce(&mut SccpParametersV1)) -> SccpParametersV1 {
        let mut parameters = SccpParametersV1::taira_default();
        update(&mut parameters);
        parameters
    }

    #[test]
    fn constants_match_the_spec_constant_summary() {
        assert_eq!(SCCP_MAX_ROSTER_VALIDITY_MS_V1, 30 * DAY_MS);
        assert_eq!(SCCP_PREVIOUS_ROSTER_GRACE_MS_V1, DAY_MS);
        assert_eq!(SCCP_MAX_CLOCK_SKEW_MS_V1, 3_600_000);
        assert_eq!(SCCP_MESSAGES_MAX_PER_BLOCK_V1, 512);
        assert_eq!(SCCP_MAX_OUTBOUND_TTL_MS_V1, 90 * DAY_MS);
        assert_eq!(SCCP_MAX_ATTESTATION_RETENTION_MS_V1, 365 * DAY_MS);
        assert_eq!(SCCP_MIN_EXEMPT_TRANSACTIONS_PER_BLOCK_V1, 2 * 31 + 32);
    }

    #[test]
    fn taira_default_is_the_genesis_table_and_valid() {
        let parameters = SccpParametersV1::taira_default();
        assert_eq!(
            parameters,
            SccpParametersV1 {
                enabled: true,
                roster_max_age_ms: 86_400_000,
                roster_validity_ms: 1_209_600_000,
                outbound_ttl_ms: 604_800_000,
                min_outbound_amount: 10_u128.pow(9),
                inbound_self_claim_fee: 10_u128.pow(7),
                attestation_retention_ms: 2_592_000_000,
                attestation_stall_ms: 600_000,
                max_attestation_entries_per_instruction: 256,
                max_exempt_transactions_per_block: 128,
            }
        );
        assert_eq!(parameters.validate(), Ok(()));
        assert_eq!(with(|p| p.enabled = false).validate(), Ok(()));
    }

    #[test]
    fn roster_max_age_boundary() {
        // Shrinking the heartbeat to its floor keeps the joint validity rule satisfied.
        assert_eq!(with(|p| p.roster_max_age_ms = 3_600_000).validate(), Ok(()));
        assert_eq!(
            with(|p| p.roster_max_age_ms = 3_599_999).validate(),
            Err(SccpParametersError::RosterMaxAgeTooShort)
        );
        assert_eq!(
            with(|p| p.roster_max_age_ms = 0).validate(),
            Err(SccpParametersError::RosterMaxAgeTooShort)
        );
    }

    #[test]
    fn roster_validity_joint_lower_bound() {
        // 2 × 1 d + 10 min + 1 d = 259 800 000.
        let floor = 2 * DAY_MS + 600_000 + DAY_MS;
        assert_eq!(with(|p| p.roster_validity_ms = floor).validate(), Ok(()));
        assert_eq!(
            with(|p| p.roster_validity_ms = floor - 1).validate(),
            Err(SccpParametersError::RosterValidityTooShort)
        );
        // Raising the stall window moves the joint floor with it.
        assert_eq!(
            with(|p| {
                p.roster_validity_ms = floor;
                p.attestation_stall_ms = 600_001;
            })
            .validate(),
            Err(SccpParametersError::RosterValidityTooShort)
        );
        // Raising the heartbeat moves the joint floor twice as fast.
        assert_eq!(
            with(|p| {
                p.roster_validity_ms = floor + 1;
                p.roster_max_age_ms = DAY_MS + 1;
            })
            .validate(),
            Err(SccpParametersError::RosterValidityTooShort)
        );
        assert_eq!(
            with(|p| {
                p.roster_validity_ms = floor + 2;
                p.roster_max_age_ms = DAY_MS + 1;
            })
            .validate(),
            Ok(())
        );
        // The largest heartbeat that still fits under the 30 d cap.
        let largest_age = (SCCP_MAX_ROSTER_VALIDITY_MS_V1 - 600_000 - DAY_MS) / 2;
        assert_eq!(
            with(|p| {
                p.roster_validity_ms = SCCP_MAX_ROSTER_VALIDITY_MS_V1;
                p.roster_max_age_ms = largest_age;
            })
            .validate(),
            Ok(())
        );
        assert_eq!(
            with(|p| {
                p.roster_validity_ms = SCCP_MAX_ROSTER_VALIDITY_MS_V1;
                p.roster_max_age_ms = largest_age + 1;
            })
            .validate(),
            Err(SccpParametersError::RosterValidityTooShort)
        );
    }

    #[test]
    fn roster_validity_joint_bound_overflow_is_a_violation() {
        assert_eq!(
            with(|p| {
                p.roster_max_age_ms = u64::MAX / 2 + 1;
                p.roster_validity_ms = u64::MAX;
            })
            .validate(),
            Err(SccpParametersError::RosterValidityTooShort)
        );
        assert_eq!(
            with(|p| {
                p.roster_max_age_ms = u64::MAX / 2;
                p.attestation_stall_ms = u64::MAX;
                p.roster_validity_ms = u64::MAX;
            })
            .validate(),
            Err(SccpParametersError::RosterValidityTooShort)
        );
        assert_eq!(
            with(|p| {
                p.roster_max_age_ms = (u64::MAX - DAY_MS) / 2;
                p.attestation_stall_ms = 2;
                p.roster_validity_ms = u64::MAX;
            })
            .validate(),
            Err(SccpParametersError::RosterValidityTooShort)
        );
    }

    #[test]
    fn roster_validity_upper_bound() {
        assert_eq!(
            with(|p| p.roster_validity_ms = SCCP_MAX_ROSTER_VALIDITY_MS_V1).validate(),
            Ok(())
        );
        assert_eq!(
            with(|p| p.roster_validity_ms = SCCP_MAX_ROSTER_VALIDITY_MS_V1 + 1).validate(),
            Err(SccpParametersError::RosterValidityTooLong)
        );
    }

    #[test]
    fn outbound_ttl_bounds() {
        assert_eq!(with(|p| p.outbound_ttl_ms = DAY_MS).validate(), Ok(()));
        assert_eq!(
            with(|p| p.outbound_ttl_ms = DAY_MS - 1).validate(),
            Err(SccpParametersError::OutboundTtlOutOfRange)
        );
        assert_eq!(
            with(|p| p.outbound_ttl_ms = 0).validate(),
            Err(SccpParametersError::OutboundTtlOutOfRange)
        );
        assert_eq!(
            with(|p| {
                p.outbound_ttl_ms = 90 * DAY_MS;
                p.attestation_retention_ms = 91 * DAY_MS;
            })
            .validate(),
            Ok(())
        );
        assert_eq!(
            with(|p| {
                p.outbound_ttl_ms = 90 * DAY_MS + 1;
                p.attestation_retention_ms = 92 * DAY_MS;
            })
            .validate(),
            Err(SccpParametersError::OutboundTtlOutOfRange)
        );
    }

    #[test]
    fn min_outbound_amount_bounds() {
        assert_eq!(with(|p| p.min_outbound_amount = 1).validate(), Ok(()));
        assert_eq!(
            with(|p| p.min_outbound_amount = 0).validate(),
            Err(SccpParametersError::MinOutboundAmountOutOfRange)
        );
        assert_eq!(
            with(|p| p.min_outbound_amount = 10_u128.pow(15)).validate(),
            Ok(())
        );
        assert_eq!(
            with(|p| p.min_outbound_amount = 10_u128.pow(15) + 1).validate(),
            Err(SccpParametersError::MinOutboundAmountOutOfRange)
        );
        assert_eq!(
            with(|p| p.min_outbound_amount = u128::MAX).validate(),
            Err(SccpParametersError::MinOutboundAmountOutOfRange)
        );
    }

    #[test]
    fn inbound_self_claim_fee_bound() {
        assert_eq!(with(|p| p.inbound_self_claim_fee = 0).validate(), Ok(()));
        assert_eq!(
            with(|p| p.inbound_self_claim_fee = 10_u128.pow(10)).validate(),
            Ok(())
        );
        assert_eq!(
            with(|p| p.inbound_self_claim_fee = 10_u128.pow(10) + 1).validate(),
            Err(SccpParametersError::InboundSelfClaimFeeTooLarge)
        );
    }

    #[test]
    fn attestation_retention_joint_lower_bound() {
        let floor = 604_800_000 + DAY_MS;
        assert_eq!(
            with(|p| p.attestation_retention_ms = floor).validate(),
            Ok(())
        );
        assert_eq!(
            with(|p| p.attestation_retention_ms = floor - 1).validate(),
            Err(SccpParametersError::AttestationRetentionTooShort)
        );
        // Raising the TTL moves the retention floor with it.
        assert_eq!(
            with(|p| {
                p.attestation_retention_ms = floor;
                p.outbound_ttl_ms = 604_800_001;
            })
            .validate(),
            Err(SccpParametersError::AttestationRetentionTooShort)
        );
    }

    #[test]
    fn attestation_retention_upper_bound() {
        assert_eq!(
            with(|p| p.attestation_retention_ms = SCCP_MAX_ATTESTATION_RETENTION_MS_V1).validate(),
            Ok(())
        );
        assert_eq!(
            with(|p| p.attestation_retention_ms = SCCP_MAX_ATTESTATION_RETENTION_MS_V1 + 1)
                .validate(),
            Err(SccpParametersError::AttestationRetentionTooLong)
        );
    }

    #[test]
    fn attestation_stall_bounds() {
        assert_eq!(with(|p| p.attestation_stall_ms = 60_000).validate(), Ok(()));
        assert_eq!(
            with(|p| p.attestation_stall_ms = 59_999).validate(),
            Err(SccpParametersError::AttestationStallOutOfRange)
        );
        assert_eq!(with(|p| p.attestation_stall_ms = DAY_MS).validate(), Ok(()));
        // Above 1 d with enough validity headroom only the stall rule itself fails.
        assert_eq!(
            with(|p| {
                p.attestation_stall_ms = DAY_MS + 1;
                p.roster_validity_ms = 5 * DAY_MS;
            })
            .validate(),
            Err(SccpParametersError::AttestationStallOutOfRange)
        );
    }

    #[test]
    fn attestation_entries_bounds() {
        for (value, expected) in [
            (64, Ok(())),
            (63, Err(SccpParametersError::AttestationEntriesOutOfRange)),
            (0, Err(SccpParametersError::AttestationEntriesOutOfRange)),
            (1_024, Ok(())),
            (
                1_025,
                Err(SccpParametersError::AttestationEntriesOutOfRange),
            ),
        ] {
            assert_eq!(
                with(|p| p.max_attestation_entries_per_instruction = value).validate(),
                expected,
                "entries = {value}"
            );
        }
    }

    #[test]
    fn exempt_transactions_bounds() {
        for (value, expected) in [
            (94, Ok(())),
            (93, Err(SccpParametersError::ExemptTransactionsOutOfRange)),
            (0, Err(SccpParametersError::ExemptTransactionsOutOfRange)),
            (1_024, Ok(())),
            (
                1_025,
                Err(SccpParametersError::ExemptTransactionsOutOfRange),
            ),
        ] {
            assert_eq!(
                with(|p| p.max_exempt_transactions_per_block = value).validate(),
                expected,
                "exempt = {value}"
            );
        }
    }

    #[test]
    fn json_u64_violation_names_the_first_field_over_the_bound() {
        assert_eq!(
            SccpParametersV1::taira_default().first_json_u64_violation(JSON_MAX),
            None
        );
        let at_bound = with(|p| {
            p.roster_max_age_ms = JSON_MAX;
            p.roster_validity_ms = JSON_MAX;
            p.outbound_ttl_ms = JSON_MAX;
            p.attestation_retention_ms = JSON_MAX;
            p.attestation_stall_ms = JSON_MAX;
        });
        assert_eq!(at_bound.first_json_u64_violation(JSON_MAX), None);
        let fields: [fn(&mut SccpParametersV1); 5] = [
            |p| p.roster_max_age_ms = JSON_MAX + 1,
            |p| p.roster_validity_ms = JSON_MAX + 1,
            |p| p.outbound_ttl_ms = JSON_MAX + 1,
            |p| p.attestation_retention_ms = JSON_MAX + 1,
            |p| p.attestation_stall_ms = JSON_MAX + 1,
        ];
        let names = [
            "roster_max_age_ms",
            "roster_validity_ms",
            "outbound_ttl_ms",
            "attestation_retention_ms",
            "attestation_stall_ms",
        ];
        for (set, name) in fields.into_iter().zip(names) {
            let over = with(set);
            let message = over
                .first_json_u64_violation(JSON_MAX)
                .expect("value above 2^53 - 1 must be reported");
            assert!(message.contains(name), "{message} should name {name}");
        }
    }

    #[test]
    fn binary_and_json_roundtrip() {
        for parameters in [
            SccpParametersV1::taira_default(),
            with(|p| {
                p.enabled = false;
                p.min_outbound_amount = u128::MAX;
                p.roster_validity_ms = u64::MAX;
            }),
        ] {
            let encoded = parameters.encode();
            assert_eq!(
                SccpParametersV1::decode_all(&mut encoded.as_slice()).expect("decode"),
                parameters
            );
            let framed = norito::to_bytes(&parameters).expect("frame");
            assert_eq!(
                norito::decode_from_bytes::<SccpParametersV1>(&framed).expect("decode frame"),
                parameters
            );
            let json = norito::json::to_json(&parameters).expect("serialize");
            assert_eq!(
                norito::json::from_json::<SccpParametersV1>(&json).expect("deserialize"),
                parameters
            );
        }
    }

    #[test]
    fn json_amounts_are_exact_decimal_strings() {
        let parameters = with(|p| p.min_outbound_amount = u128::MAX);
        let json = norito::json::to_json(&parameters).expect("serialize");
        assert!(
            json.contains(r#""min_outbound_amount":"340282366920938463463374607431768211455""#),
            "{json}"
        );
        assert!(
            json.contains(r#""inbound_self_claim_fee":"10000000""#),
            "{json}"
        );
        let numeric = json.replace(
            r#""inbound_self_claim_fee":"10000000""#,
            r#""inbound_self_claim_fee":10000000"#,
        );
        assert!(norito::json::from_json::<SccpParametersV1>(&numeric).is_err());
    }

    #[test]
    fn json_rejects_unknown_and_missing_fields() {
        let mut value =
            norito::json::to_value(&SccpParametersV1::taira_default()).expect("to value");
        let object = value.as_object_mut().expect("object");
        object.insert(
            "min_generation_interval_ms".to_owned(),
            norito::json::Value::from(1_u64),
        );
        let hostile = norito::json::to_json(&value).expect("serialize");
        assert!(norito::json::from_json::<SccpParametersV1>(&hostile).is_err());

        let mut value =
            norito::json::to_value(&SccpParametersV1::taira_default()).expect("to value");
        value
            .as_object_mut()
            .expect("object")
            .remove("attestation_stall_ms");
        let missing = norito::json::to_json(&value).expect("serialize");
        assert!(norito::json::from_json::<SccpParametersV1>(&missing).is_err());
    }
}
