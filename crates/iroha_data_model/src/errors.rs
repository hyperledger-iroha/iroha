//! Canonical error codes surfaced by AMX/DA/settlement pipelines.
//!
//! These types provide a shared, machine-readable schema for the NX-17 error
//! catalog. Each variant maps to a deterministic `reason_code` so operators and
//! SDKs can branch on stable integers instead of parsing strings.

use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::nexus::DataSpaceId;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};

/// Canonical error envelope containing the stable reason code and structured
/// context.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct CanonicalError {
    /// Stable reason code associated with [`detail`].
    pub reason_code: u16,
    /// Structured error payload.
    pub detail: CanonicalErrorKind,
}

impl CanonicalError {
    /// Construct an error using the canonical reason code for the given detail
    /// variant.
    #[must_use]
    pub const fn new(detail: CanonicalErrorKind) -> Self {
        let reason_code = detail.reason_code();
        Self {
            reason_code,
            detail,
        }
    }
}

/// Canonical error variants surfaced by AMX, DA, and settlement components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(DeriveJsonSerialize, DeriveJsonDeserialize),
    norito(tag = "code", content = "context")
)]
pub enum CanonicalErrorKind {
    /// Fewer than `required_receipts` DA attestations arrived before
    /// `deadline_ms`.
    DaDeadlineExceeded(DaDeadlineExceeded),
    /// Oracle data was stale beyond the configured limit for the dataspace.
    OracleStale(OracleStale),
    /// A circuit breaker prevented execution (e.g., liquidity/volatility guard).
    CircuitBreakerActive(CircuitBreakerActive),
    /// Dataspace buffer dropped below configured floor, forcing XOR-only
    /// inclusion.
    BufferDepletedXorOnly(BufferDepletedXorOnly),
    /// Static analysis could not bound the declared read/write set for a
    /// program.
    RwsetUnbounded(RwsetUnbounded),
    /// AMX exceeded its prepare/execute/commit budget for a dataspace slice.
    AmxTimeout(AmxTimeout),
    /// Host detected overlapping write sets or undeclared touches.
    AmxLockConflict(AmxLockConflict),
    /// Referenced PVO handle is missing from cache or expired.
    PvoMissingOrExpired(PvoMissingOrExpired),
    /// Contract executed an instruction that AMX lanes disallow.
    HeavyInstructionDisallowed(HeavyInstructionDisallowed),
    /// Settlement router could not supply a deterministic conversion path.
    SettlementRouterUnavailable(SettlementRouterUnavailable),
}

/// Context for `DA_DEADLINE_EXCEEDED`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaDeadlineExceeded {
    /// Dataspace associated with the DA sample.
    pub dataspace: DataSpaceId,
    /// Receipt quorum required for inclusion.
    pub required_receipts: u16,
    /// Receipts observed before the deadline.
    pub received_receipts: u16,
    /// Millisecond deadline budget that was exceeded.
    pub deadline_ms: u16,
}

/// Context for `ORACLE_STALE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OracleStale {
    /// Dataspace affected by stale inputs.
    pub dataspace: DataSpaceId,
    /// Age of the last oracle update in milliseconds.
    pub last_update_age_ms: u64,
    /// Maximum staleness budget in milliseconds.
    pub max_staleness_ms: u64,
}

/// Context for `CIRCUIT_BREAKER_ACTIVE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct CircuitBreakerActive {
    /// Dataspace affected by the breaker.
    pub dataspace: DataSpaceId,
    /// Kind of breaker that tripped.
    pub breaker: CircuitBreakerKind,
}

/// Context for `BUFFER_DEPLETED_XOR_ONLY`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct BufferDepletedXorOnly {
    /// Dataspace whose buffer was depleted.
    pub dataspace: DataSpaceId,
    /// Current buffer level in basis points.
    pub buffer_level_bp: u16,
    /// Configured floor that triggered XOR-only mode.
    pub floor_bp: u16,
}

/// Context for `RWSET_UNBOUNDED`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct RwsetUnbounded {
    /// Hash of the offending program or descriptor.
    pub program_hash: Hash,
}

/// Context for `AMX_TIMEOUT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AmxTimeout {
    /// Dataspace whose slice exceeded the budget.
    pub dataspace: DataSpaceId,
    /// Stage that timed out.
    pub stage: AmxStage,
    /// Elapsed milliseconds for the stage.
    pub elapsed_ms: u32,
    /// Millisecond budget for the stage.
    pub budget_ms: u32,
}

/// Context for `AMX_LOCK_CONFLICT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AmxLockConflict {
    /// Dataspace where the conflict was detected.
    pub dataspace: DataSpaceId,
    /// Number of conflicting keys or touches observed.
    pub conflicts: u16,
}

/// Context for `PVO_MISSING_OR_EXPIRED`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct PvoMissingOrExpired {
    /// PVO handle digest or identifier.
    pub handle: Hash,
    /// Slot height at which the handle expired.
    pub expiry_slot: u64,
    /// Current slot height when the error was emitted.
    pub current_slot: u64,
}

/// Context for `HEAVY_INSTRUCTION_DISALLOWED`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct HeavyInstructionDisallowed {
    /// Opcode identifier of the disallowed instruction.
    pub opcode: u16,
}

/// Context for `SETTLEMENT_ROUTER_UNAVAILABLE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SettlementRouterUnavailable {
    /// Dataspace whose conversion failed.
    pub dataspace: DataSpaceId,
    /// Reason the router was unavailable.
    pub reason: SettlementRouterOutage,
}

impl CanonicalErrorKind {
    /// Reason code used for `DA_DEADLINE_EXCEEDED`.
    pub const DA_DEADLINE_EXCEEDED_CODE: u16 = 1_000;
    /// Reason code used for `ORACLE_STALE`.
    pub const ORACLE_STALE_CODE: u16 = 1_001;
    /// Reason code used for `CIRCUIT_BREAKER_ACTIVE`.
    pub const CIRCUIT_BREAKER_ACTIVE_CODE: u16 = 1_002;
    /// Reason code used for `BUFFER_DEPLETED_XOR_ONLY`.
    pub const BUFFER_DEPLETED_XOR_ONLY_CODE: u16 = 1_003;
    /// Reason code used for `RWSET_UNBOUNDED`.
    pub const RWSET_UNBOUNDED_CODE: u16 = 1_004;
    /// Reason code used for `AMX_TIMEOUT`.
    pub const AMX_TIMEOUT_CODE: u16 = 1_005;
    /// Reason code used for `AMX_LOCK_CONFLICT`.
    pub const AMX_LOCK_CONFLICT_CODE: u16 = 1_006;
    /// Reason code used for `PVO_MISSING_OR_EXPIRED`.
    pub const PVO_MISSING_OR_EXPIRED_CODE: u16 = 1_007;
    /// Reason code used for `HEAVY_INSTRUCTION_DISALLOWED`.
    pub const HEAVY_INSTRUCTION_DISALLOWED_CODE: u16 = 1_008;
    /// Reason code used for `SETTLEMENT_ROUTER_UNAVAILABLE`.
    pub const SETTLEMENT_ROUTER_UNAVAILABLE_CODE: u16 = 1_009;

    /// Return the deterministic reason code for this error variant.
    #[must_use]
    pub const fn reason_code(&self) -> u16 {
        match self {
            Self::DaDeadlineExceeded(_) => Self::DA_DEADLINE_EXCEEDED_CODE,
            Self::OracleStale(_) => Self::ORACLE_STALE_CODE,
            Self::CircuitBreakerActive(_) => Self::CIRCUIT_BREAKER_ACTIVE_CODE,
            Self::BufferDepletedXorOnly(_) => Self::BUFFER_DEPLETED_XOR_ONLY_CODE,
            Self::RwsetUnbounded(_) => Self::RWSET_UNBOUNDED_CODE,
            Self::AmxTimeout(_) => Self::AMX_TIMEOUT_CODE,
            Self::AmxLockConflict(_) => Self::AMX_LOCK_CONFLICT_CODE,
            Self::PvoMissingOrExpired(_) => Self::PVO_MISSING_OR_EXPIRED_CODE,
            Self::HeavyInstructionDisallowed(_) => Self::HEAVY_INSTRUCTION_DISALLOWED_CODE,
            Self::SettlementRouterUnavailable(_) => Self::SETTLEMENT_ROUTER_UNAVAILABLE_CODE,
        }
    }
}

/// Stage of AMX execution used for timeout reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "stage", content = "value"))]
pub enum AmxStage {
    /// Overlay preparation and lock acquisition.
    Prepare,
    /// IVM overlay execution.
    Execute,
    /// Delta merge and trigger replay.
    Commit,
}

/// Identifier for the breaker that halted execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum CircuitBreakerKind {
    /// Volatility or liquidity guard.
    Liquidity,
    /// Governance-initiated halt.
    Governance,
    /// Oracle confidence or freshness guard.
    Oracle,
    /// Catch-all when the breaker does not map to a dedicated bucket.
    Other,
}

/// Reason why the settlement router could not supply a conversion path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum SettlementRouterOutage {
    /// No valid route in the path registry.
    PathMissing,
    /// Buffer guardrails forced XOR-only mode.
    BufferDepleted,
    /// Router is rate limited or unreachable.
    RateLimited,
    /// Oracle staleness prevented pricing a deterministic path.
    OracleStale,
    /// Catch-all for implementation-defined outages.
    Other,
}

#[cfg(test)]
mod tests {
    use norito::core::NoritoDeserialize;

    use super::*;

    #[test]
    fn reason_codes_are_stable() {
        let ds = DataSpaceId::new(7);
        let detail = CanonicalErrorKind::AmxTimeout(AmxTimeout {
            dataspace: ds,
            stage: AmxStage::Prepare,
            elapsed_ms: 45,
            budget_ms: 30,
        });
        assert_eq!(detail.reason_code(), CanonicalErrorKind::AMX_TIMEOUT_CODE);

        let detail = CanonicalErrorKind::RwsetUnbounded(RwsetUnbounded {
            program_hash: Hash::new(b"rwset-unbounded"),
        });
        assert_eq!(
            detail.reason_code(),
            CanonicalErrorKind::RWSET_UNBOUNDED_CODE
        );
    }

    #[test]
    fn canonical_error_roundtrips() {
        let detail = CanonicalErrorKind::PvoMissingOrExpired(PvoMissingOrExpired {
            handle: Hash::new(b"pvo-handle"),
            expiry_slot: 10,
            current_slot: 12,
        });
        let error = CanonicalError::new(detail);
        let bytes = norito::to_bytes(&error).expect("encode canonical error");
        let archived =
            norito::from_bytes::<CanonicalError>(&bytes).expect("decode canonical error");
        let decoded: CanonicalError =
            NoritoDeserialize::try_deserialize(archived).expect("deserialize canonical error");
        assert_eq!(
            decoded.reason_code,
            CanonicalErrorKind::PVO_MISSING_OR_EXPIRED_CODE
        );
        assert_eq!(decoded.detail, detail);
    }
}
