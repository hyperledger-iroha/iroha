//! Committed native private-settlement admission limits and bounded drain scheduling.

use super::*;
use crate::{
    account::{AccountController, AccountId},
    nexus::{
        ATOMIC_PRIVATE_SETTLEMENT_MAX_LEGS_V1, ATOMIC_PRIVATE_SETTLEMENT_MIN_LEGS_V1,
        PRIVATE_SETTLEMENT_MAX_AUDITORS_V1, PRIVATE_SETTLEMENT_MAX_PROOF_BYTES_V1,
        PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1, PrivateSettlementCapsulePaddingV1,
        private_settlement_capsule_canonical_upper_bound_v1,
    },
    parameter::{CustomParameter, CustomParameterId, Parameters},
};
use iroha_primitives::json::Json;

/// Immutable private-payment admission policy stored in `Parameters.custom`.
///
/// Authenticated absence means the fixed native V1 baseline. A present value has exactly
/// this schema; malformed values and unfinished local codec attempts never select defaults.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::privacy::PrivateSettlementPolicyV1")]
pub struct PrivateSettlementPolicyV1 {
    /// Limits effective at this committed cut.
    pub current_limits: PrivateSettlementValidityLimitsV1,
    /// At most one future private-policy replacement.
    pub pending_update: Option<PrivateSettlementPolicyUpdateV1>,
    /// Original horizon of the exact pending ordinary protocol closure, if any.
    pub protocol_closure_drain: Option<PrivateSettlementProtocolClosureDrainV1>,
}

/// A future private admission policy, separate from the frozen general privacy layout.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::privacy::PrivateSettlementPolicyUpdateV1")]
pub struct PrivateSettlementPolicyUpdateV1 {
    /// Exact block admitting this schedule.
    pub scheduled_at_height: u64,
    /// Exact incoming block applying this successor.
    pub effective_at_height: u64,
    /// Complete successor private admission limits.
    pub next_limits: PrivateSettlementValidityLimitsV1,
}

/// Captured original drain bound to one exact ordinary protocol-limit schedule.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::privacy::PrivateSettlementProtocolClosureDrainV1")]
pub struct PrivateSettlementProtocolClosureDrainV1 {
    /// The native relation whose ordinary limit update closes admission.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Exact block which admitted that protocol schedule.
    pub scheduled_at_height: u64,
    /// Exact incoming block applying that protocol successor.
    pub effective_at_height: u64,
    /// Hash of the canonical frame of the exact scheduled protocol limits.
    pub next_limits_hash: iroha_crypto::Hash,
    /// Expiry span captured from the private limits when the schedule was admitted.
    pub original_max_expiry_blocks: u64,
}

/// A deterministic private-policy or captured schedule inconsistency.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PrivateSettlementPolicyValidationErrorV1 {
    /// A current or successor private limit is malformed.
    #[error("invalid private-settlement limits: {0}")]
    Limits(#[from] PrivateSettlementValidityErrorV1),
    /// A schedule has invalid or missed block heights.
    #[error("invalid private-settlement schedule: {0}")]
    Schedule(#[from] PrivacyPolicyValidationErrorV1),
    /// A private update must change the effective limits.
    #[error("private-settlement policy update is a no-op")]
    NoChange,
    /// A captured drain does not identify the exact native pending closure.
    #[error("private-settlement drain differs from the pending protocol closure")]
    ProtocolClosureMismatch,
}

/// A parameter failure retaining the original local codec refusal, when present.
#[derive(Debug, thiserror::Error)]
pub enum PrivateSettlementPolicyParameterErrorV1 {
    /// Deterministic limits, schedule or closure inconsistency.
    #[error(transparent)]
    Invalid(#[from] PrivateSettlementPolicyValidationErrorV1),
    /// Wrong identity, excessive payload or noncanonical JSON.
    #[error("invalid private-settlement policy parameter: {0}")]
    CustomParameter(&'static str),
    /// Original decoding or encoding attempt, including its enforcing scope identity.
    #[error(transparent)]
    Codec(#[from] norito::core::DecodeAttemptError),
}

impl PrivateSettlementPolicyV1 {
    /// Largest canonical parameter payload.
    pub const MAX_PAYLOAD_BYTES: usize = 4_096;

    /// Fixed native V1 meaning of authenticated parameter absence.
    #[must_use]
    pub const fn protocol_default() -> Self {
        Self {
            current_limits: PrivateSettlementValidityLimitsV1::protocol_default(),
            pending_update: None,
            protocol_closure_drain: None,
        }
    }

    /// Sole identity of the private admission extension.
    #[must_use]
    pub fn parameter_id() -> &'static CustomParameterId {
        static ID: std::sync::OnceLock<CustomParameterId> = std::sync::OnceLock::new();
        ID.get_or_init(|| {
            "private_settlement_policy_v1"
                .parse()
                .expect("static identity")
        })
    }

    /// Select the explicit parameter or the immutable native absence meaning.
    /// # Errors
    /// A present parameter must decode and validate without any substitution.
    pub fn from_parameters(
        parameters: &Parameters,
    ) -> Result<Self, PrivateSettlementPolicyParameterErrorV1> {
        parameters
            .custom()
            .get(Self::parameter_id())
            .map_or_else(|| Ok(Self::protocol_default()), Self::from_custom_parameter)
    }

    /// Decode the sole canonical payload, preserving original local codec failures.
    /// # Errors
    /// Wrong identity, excessive size, malformed/noncanonical JSON or invalid policy.
    pub fn from_custom_parameter(
        parameter: &CustomParameter,
    ) -> Result<Self, PrivateSettlementPolicyParameterErrorV1> {
        if parameter.id() != Self::parameter_id() {
            return Err(PrivateSettlementPolicyParameterErrorV1::CustomParameter(
                "another parameter identity",
            ));
        }
        let payload = parameter.payload();
        if payload.get().len() > Self::MAX_PAYLOAD_BYTES {
            return Err(PrivateSettlementPolicyParameterErrorV1::CustomParameter(
                "payload exceeds 4096 bytes",
            ));
        }
        let value: Self = norito::core::classify_decode_attempt(|| {
            norito::json::from_str(payload.get()).map_err(norito::json::Error::into_core_error)
        })?;
        let canonical = norito::core::classify_decode_attempt(|| Json::try_new(value))?;
        if &canonical != payload {
            return Err(PrivateSettlementPolicyParameterErrorV1::CustomParameter(
                "payload is not canonical JSON",
            ));
        }
        value.validate()?;
        Ok(value)
    }

    /// Encode a valid policy without losing original local codec failures.
    /// # Errors
    /// Invalid policy, excessive payload or original encoding failure.
    pub fn to_custom_parameter(
        &self,
    ) -> Result<CustomParameter, PrivateSettlementPolicyParameterErrorV1> {
        self.validate()?;
        let payload = norito::core::classify_decode_attempt(|| Json::try_new(self))?;
        if payload.get().len() > Self::MAX_PAYLOAD_BYTES {
            return Err(PrivateSettlementPolicyParameterErrorV1::CustomParameter(
                "payload exceeds 4096 bytes",
            ));
        }
        Ok(CustomParameter::new(Self::parameter_id().clone(), payload))
    }

    /// Validate private limits and each retained schedule's intrinsic original drain.
    /// # Errors
    /// Malformed limits, no-op update, invalid schedule or original drain.
    pub fn validate(&self) -> Result<(), PrivateSettlementPolicyValidationErrorV1> {
        self.current_limits.validate()?;
        if let Some(pending) = self.pending_update {
            pending.validate_against(&self.current_limits)?;
        }
        if let Some(drain) = self.protocol_closure_drain {
            drain.validate()?;
        }
        Ok(())
    }

    /// Validate both schedules against the exact committed cut.
    /// # Errors
    /// Invalid policy, future claimed admission or a due transition left unapplied.
    pub fn validate_at_committed_height(
        &self,
        committed_height: u64,
    ) -> Result<(), PrivateSettlementPolicyValidationErrorV1> {
        self.validate()?;
        for (scheduled, effective) in self
            .pending_update
            .iter()
            .map(|pending| (pending.scheduled_at_height, pending.effective_at_height))
            .chain(
                self.protocol_closure_drain
                    .iter()
                    .map(|drain| (drain.scheduled_at_height, drain.effective_at_height)),
            )
        {
            if scheduled > committed_height {
                return Err(
                    PrivacyPolicyValidationErrorV1::PendingScheduledAfterCommitted {
                        scheduled_at_height: scheduled,
                        committed_height,
                    }
                    .into(),
                );
            }
            if effective <= committed_height {
                return Err(PrivacyPolicyValidationErrorV1::PendingNotFuture {
                    effective_at_height: effective,
                    committed_height,
                }
                .into());
            }
        }
        Ok(())
    }

    /// Bind the captured original horizon to the exact optional native protocol record.
    ///
    /// The caller also validates that protocol record at the same committed height. A
    /// closing update needs one matching capture; every other state forbids a capture.
    /// # Errors
    /// Missing, unrelated or substituted capture, or an original hashing refusal.
    pub fn validate_protocol_policy(
        &self,
        protocol: Option<&PrivacyProtocolPolicyV1>,
    ) -> Result<(), PrivateSettlementPolicyParameterErrorV1> {
        self.validate()?;
        let closing = protocol.and_then(|policy| {
            policy
                .pending_update
                .as_ref()
                .filter(|pending| {
                    policy.protocol_id == PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1
                        && private_settlement_protocol_closes_v1(
                            &policy.current_limits,
                            &pending.next_limits,
                        )
                })
                .map(|pending| (policy.protocol_id, pending))
        });
        match (closing, self.protocol_closure_drain) {
            (Some((protocol_id, pending)), Some(drain)) => {
                let expected = PrivateSettlementProtocolClosureDrainV1::capture(
                    protocol_id,
                    pending,
                    drain.original_max_expiry_blocks,
                )?;
                if drain != expected {
                    return Err(
                        PrivateSettlementPolicyValidationErrorV1::ProtocolClosureMismatch.into(),
                    );
                }
            }
            (None, None) => {}
            _ => {
                return Err(
                    PrivateSettlementPolicyValidationErrorV1::ProtocolClosureMismatch.into(),
                );
            }
        }
        Ok(())
    }
}

impl Default for PrivateSettlementPolicyV1 {
    fn default() -> Self {
        Self::protocol_default()
    }
}

impl PrivateSettlementPolicyUpdateV1 {
    /// Validate a successor against the original current admission horizon.
    /// # Errors
    /// Invalid limits, no-op, non-future activation or insufficient original drain.
    pub fn validate_against(
        &self,
        current: &PrivateSettlementValidityLimitsV1,
    ) -> Result<(), PrivateSettlementPolicyValidationErrorV1> {
        super::policy::validate_privacy_policy_schedule_heights_v1(
            self.scheduled_at_height,
            self.effective_at_height,
        )?;
        current.validate()?;
        self.next_limits.validate()?;
        if *current == self.next_limits {
            return Err(PrivateSettlementPolicyValidationErrorV1::NoChange);
        }
        validate_private_settlement_drain_v1(
            self.scheduled_at_height,
            current.max_expiry_blocks,
            self.effective_at_height,
        )?;
        Ok(())
    }
}

impl PrivateSettlementProtocolClosureDrainV1 {
    /// Capture the exact original schedule and admission span before either owner changes.
    /// # Errors
    /// Wrong native protocol, invalid original drain or original canonical encoding refusal.
    pub fn capture(
        protocol_id: PrivacyProtocolIdV1,
        pending: &PrivacyProtocolLimitsUpdateV1,
        original_max_expiry_blocks: u64,
    ) -> Result<Self, PrivateSettlementPolicyParameterErrorV1> {
        if pending.next_limits.protocol_id() != protocol_id {
            return Err(PrivateSettlementPolicyValidationErrorV1::ProtocolClosureMismatch.into());
        }
        let frame = norito::core::classify_decode_attempt(|| {
            norito::encode_canonical(&pending.next_limits)
        })?;
        let capture = Self {
            protocol_id,
            scheduled_at_height: pending.scheduled_at_height,
            effective_at_height: pending.effective_at_height,
            next_limits_hash: iroha_crypto::Hash::new(frame),
            original_max_expiry_blocks,
        };
        capture.validate()?;
        Ok(capture)
    }

    /// Validate the immutable native identity and retained original expiry horizon.
    /// # Errors
    /// Another relation or activation before every previously admissible expiry.
    pub fn validate(&self) -> Result<(), PrivateSettlementPolicyValidationErrorV1> {
        if self.protocol_id != PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 {
            return Err(PrivateSettlementPolicyValidationErrorV1::ProtocolClosureMismatch);
        }
        validate_private_settlement_drain_v1(
            self.scheduled_at_height,
            self.original_max_expiry_blocks,
            self.effective_at_height,
        )?;
        Ok(())
    }
}

/// Maximum signatures in every native V1 private-settlement carrier, including cleanup.
///
/// The full original account controller remains bound; this limits the number of
/// signatures satisfying that controller, not the number of non-signing members.
pub const PRIVATE_SETTLEMENT_MAX_CARRIER_SIGNERS_V1: usize = 16;

/// Hard canonical capsule ceiling of the fixed native V1 relation.
pub const PRIVATE_SETTLEMENT_MAX_CAPSULE_BYTES_V1: u64 = 16 * 1024 * 1024;

/// Fixed canonical signed-transaction frame envelope available to original authorized cleanup.
///
/// A sponsor's complete multisig roster can dominate its signed carrier size.
/// Cleanup therefore preserves the same full-carrier ceiling as initial native
/// admission, rather than assuming every account has only sixteen members.
pub const PRIVATE_SETTLEMENT_MAX_CLEANUP_TRANSACTION_BYTES_V1: u64 =
    PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1 as u64;

/// The selectable consensus limits for the single native V1 private-payment relation.
///
/// Worker, cache and local sidecar storage capacities do not belong to this record.
/// Profile and audit-policy versions are fixed by the native model, not selectable.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::privacy::PrivateSettlementValidityLimitsV1")]
pub struct PrivateSettlementValidityLimitsV1 {
    /// Maximum ordered participant data spaces in one bundle.
    pub max_participants: u16,
    /// Maximum span from the original selected context to signed expiry.
    pub max_expiry_blocks: u64,
    /// Maximum auditor-approval collection duration after sidecar admission.
    pub audit_timeout_blocks: u64,
    /// Maximum additional duration for all Prepare certificates.
    pub prepare_timeout_blocks: u64,
    /// Maximum additional duration for all Commit certificates.
    pub commit_timeout_blocks: u64,
    /// Nonzero mask of the fixed `KiB4`, `KiB16`, `KiB64` and `KiB256` classes, in bit order.
    pub capsule_padding_classes: u8,
    /// Maximum proof payload bytes per leg, excluding sidecar framing.
    pub max_proof_bytes: u64,
    /// Maximum complete canonical audit capsule bytes per leg.
    pub max_capsule_bytes: u64,
    /// Maximum complete canonical sponsor-signed transaction frame bytes for new carriers.
    pub max_carrier_bytes: u64,
    /// Minimum signed durable sidecar retention after admission.
    pub sidecar_retention_blocks: u64,
    /// Minimum governed auditor threshold for newly admitted material.
    pub min_auditor_approvals: u16,
}

/// Exact refusal of a native private-payment policy or its admission window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PrivateSettlementValidityErrorV1 {
    /// A positive limit or duration was zero.
    #[error("private-settlement limit {0} must be positive")]
    Zero(&'static str),
    /// A field is outside its fixed native range.
    #[error("private-settlement limit {0} exceeds its native range")]
    NativeBound(&'static str),
    /// Padding selection is empty or contains an unknown native class.
    #[error("private-settlement padding selection is invalid")]
    Padding,
    /// Phase durations overflow or exceed the allowed expiry span.
    #[error("private-settlement phase durations exceed the expiry span")]
    PhaseDurations,
    /// Retention is shorter than the admitted expiry span.
    #[error("private-settlement retention is shorter than its expiry span")]
    Retention,
    /// A selected padding class cannot hold its complete minimum-auditor capsule.
    #[error("private-settlement capsule limit cannot hold a selected class")]
    CapsuleFraming,
    /// The complete original sponsor needs more than the fixed signing bound.
    #[error("private-settlement sponsor has no quorum within the native signing bound")]
    SponsorQuorum,
    /// The original context or expiry is not live at the selected height.
    #[error("private-settlement context or expiry is outside its admission window")]
    AdmissionHeight,
    /// Expiry reaches an already committed exclusive drain boundary.
    #[error("private-settlement expiry reaches a committed policy drain boundary")]
    AdmissionDrain,
    /// No representable height can satisfy the original obligation drain.
    #[error("private-settlement drain horizon overflows")]
    DrainOverflow,
    /// A relation-closing update omitted its governed original drain horizon.
    #[error("private-settlement relation closure requires its original drain horizon")]
    DrainRequired,
    /// A non-closing update carries an unrelated drain horizon.
    #[error("private-settlement drain horizon is not applicable to this update")]
    UnexpectedDrain,
    /// Activation does not follow every previously admissible original expiry.
    #[error(
        "private-settlement activation {effective_at_height} must follow drain horizon {last_expiry_height}"
    )]
    DrainTooEarly {
        /// Proposed exact activation height.
        effective_at_height: u64,
        /// Largest expiry admissible before the schedule was committed.
        last_expiry_height: u64,
    },
}

impl PrivateSettlementValidityLimitsV1 {
    /// Canonical first-release contract, identical on every node.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the const assertion below keeps the protocol leg limit within u16"
    )]
    pub const fn protocol_default() -> Self {
        const { assert!(ATOMIC_PRIVATE_SETTLEMENT_MAX_LEGS_V1 <= u16::MAX as usize) };
        Self {
            max_participants: ATOMIC_PRIVATE_SETTLEMENT_MAX_LEGS_V1 as u16,
            max_expiry_blocks: 7_200,
            audit_timeout_blocks: 1_200,
            prepare_timeout_blocks: 1_200,
            commit_timeout_blocks: 1_200,
            capsule_padding_classes: 0b1111,
            max_proof_bytes: PRIVATE_SETTLEMENT_MAX_PROOF_BYTES_V1 as u64,
            max_capsule_bytes: 1024 * 1024,
            max_carrier_bytes: PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1 as u64,
            sidecar_retention_blocks: 1_000_000,
            min_auditor_approvals: 1,
        }
    }

    /// Whether this contract permits the selected fixed native padding class.
    #[must_use]
    pub const fn permits_padding(&self, padding: PrivateSettlementCapsulePaddingV1) -> bool {
        let bit = match padding {
            PrivateSettlementCapsulePaddingV1::KiB4 => 0,
            PrivateSettlementCapsulePaddingV1::KiB16 => 1,
            PrivateSettlementCapsulePaddingV1::KiB64 => 2,
            PrivateSettlementCapsulePaddingV1::KiB256 => 3,
        };
        self.capsule_padding_classes & (1 << bit) != 0
    }

    /// Check positive fields, native ceilings and complete capsule/phase relationships.
    /// # Errors
    /// Returns the first invalid field in deterministic declaration order.
    pub fn validate(&self) -> Result<(), PrivateSettlementValidityErrorV1> {
        use PrivateSettlementValidityErrorV1 as Error;
        if !(ATOMIC_PRIVATE_SETTLEMENT_MIN_LEGS_V1..=ATOMIC_PRIVATE_SETTLEMENT_MAX_LEGS_V1)
            .contains(&usize::from(self.max_participants))
        {
            return Err(Error::NativeBound("max_participants"));
        }
        for (name, value) in [
            ("max_expiry_blocks", self.max_expiry_blocks),
            ("audit_timeout_blocks", self.audit_timeout_blocks),
            ("prepare_timeout_blocks", self.prepare_timeout_blocks),
            ("commit_timeout_blocks", self.commit_timeout_blocks),
            ("max_proof_bytes", self.max_proof_bytes),
            ("max_capsule_bytes", self.max_capsule_bytes),
            ("max_carrier_bytes", self.max_carrier_bytes),
            ("sidecar_retention_blocks", self.sidecar_retention_blocks),
            (
                "min_auditor_approvals",
                u64::from(self.min_auditor_approvals),
            ),
        ] {
            if value == 0 {
                return Err(Error::Zero(name));
            }
        }
        for (name, value, maximum) in [
            (
                "max_proof_bytes",
                self.max_proof_bytes,
                PRIVATE_SETTLEMENT_MAX_PROOF_BYTES_V1 as u64,
            ),
            (
                "max_capsule_bytes",
                self.max_capsule_bytes,
                PRIVATE_SETTLEMENT_MAX_CAPSULE_BYTES_V1,
            ),
            (
                "max_carrier_bytes",
                self.max_carrier_bytes,
                PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1 as u64,
            ),
            (
                "min_auditor_approvals",
                u64::from(self.min_auditor_approvals),
                PRIVATE_SETTLEMENT_MAX_AUDITORS_V1 as u64,
            ),
        ] {
            if value > maximum {
                return Err(Error::NativeBound(name));
            }
        }
        if self.capsule_padding_classes == 0 || self.capsule_padding_classes & !0b1111 != 0 {
            return Err(Error::Padding);
        }
        if self
            .audit_timeout_blocks
            .checked_add(self.prepare_timeout_blocks)
            .and_then(|sum| sum.checked_add(self.commit_timeout_blocks))
            .is_none_or(|sum| sum > self.max_expiry_blocks)
        {
            return Err(Error::PhaseDurations);
        }
        if self.sidecar_retention_blocks < self.max_expiry_blocks {
            return Err(Error::Retention);
        }
        for padding in [
            PrivateSettlementCapsulePaddingV1::KiB4,
            PrivateSettlementCapsulePaddingV1::KiB16,
            PrivateSettlementCapsulePaddingV1::KiB64,
            PrivateSettlementCapsulePaddingV1::KiB256,
        ] {
            if self.permits_padding(padding)
                && private_settlement_capsule_canonical_upper_bound_v1(
                    padding.plaintext_bytes() as u64,
                    u64::from(self.min_auditor_approvals),
                ) > self.max_capsule_bytes
            {
                return Err(Error::CapsuleFraming);
            }
        }
        Ok(())
    }

    /// Validate a new original intent against this contract and an exclusive pending boundary.
    /// # Errors
    /// Invalid limits, participant count, original height, span, expiry or drain boundary.
    pub fn validate_admission(
        &self,
        participants: usize,
        context_height: u64,
        expiry_height: u64,
        at_height: u64,
        exclusive_deadline: Option<u64>,
    ) -> Result<(), PrivateSettlementValidityErrorV1> {
        self.validate()?;
        if !(ATOMIC_PRIVATE_SETTLEMENT_MIN_LEGS_V1..=usize::from(self.max_participants))
            .contains(&participants)
        {
            return Err(PrivateSettlementValidityErrorV1::NativeBound(
                "participants",
            ));
        }
        if context_height == 0
            || at_height < context_height
            || at_height > expiry_height
            || expiry_height
                .checked_sub(context_height)
                .is_none_or(|span| span == 0 || span > self.max_expiry_blocks)
        {
            return Err(PrivateSettlementValidityErrorV1::AdmissionHeight);
        }
        if exclusive_deadline.is_some_and(|deadline| expiry_height >= deadline) {
            return Err(PrivateSettlementValidityErrorV1::AdmissionDrain);
        }
        Ok(())
    }
}

impl Default for PrivateSettlementValidityLimitsV1 {
    fn default() -> Self {
        Self::protocol_default()
    }
}

/// Whether the ordinary protocol limits admit the fixed native private-payment relation.
///
/// This checks only governed resource counts. Native relation verification, committee authority,
/// pool policy and the exact signed payment still have their existing independent owners.
#[must_use]
pub const fn private_settlement_protocol_admission_open_v1(
    limits: &PrivacyProtocolLimitsV1,
) -> bool {
    match limits {
        PrivacyProtocolLimitsV1::IrohaIvmPrivateNoteStarkV1(limits) => {
            limits.max_input_count >= IVM_PRIVATE_NOTE_MAX_INPUTS_V1
                && limits.max_output_count >= IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1
        }
        _ => false,
    }
}

/// Whether a protocol update closes an already available native private-payment relation.
#[must_use]
pub const fn private_settlement_protocol_closes_v1(
    current: &PrivacyProtocolLimitsV1,
    next: &PrivacyProtocolLimitsV1,
) -> bool {
    private_settlement_protocol_admission_open_v1(current)
        && !private_settlement_protocol_admission_open_v1(next)
}

/// The earliest committed exclusive boundary for a newly admitted private payment.
///
/// Both supplied records must already be authenticated and validated at the same World cut.
/// The result is independent of today's maximum expiry: a pending generic relation closure
/// keeps its original deadline even if a separate private-policy change lengthens that span.
#[must_use]
pub fn private_settlement_admission_deadline_v1(
    consensus: &PrivateSettlementPolicyV1,
    protocol: Option<&PrivacyProtocolPolicyV1>,
) -> Option<u64> {
    let private = consensus
        .pending_update
        .map(|pending| pending.effective_at_height);
    let protocol = protocol.and_then(|policy| {
        policy.pending_update.and_then(|pending| {
            private_settlement_protocol_closes_v1(&policy.current_limits, &pending.next_limits)
                .then_some(pending.effective_at_height)
        })
    });
    match (private, protocol) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (left, right) => left.or(right),
    }
}

/// Check the original, bounded obligation horizon carried by a committed schedule.
/// # Errors
/// Zero original span, overflow, or activation at/before the last original expiry.
pub fn validate_private_settlement_drain_v1(
    scheduled_at_height: u64,
    original_max_expiry_blocks: u64,
    effective_at_height: u64,
) -> Result<(), PrivateSettlementValidityErrorV1> {
    if scheduled_at_height == 0 {
        return Err(PrivateSettlementValidityErrorV1::Zero(
            "scheduled_at_height",
        ));
    }
    if original_max_expiry_blocks == 0 {
        return Err(PrivateSettlementValidityErrorV1::Zero(
            "original_max_expiry_blocks",
        ));
    }
    // H1 is the signed initial policy, before any private monetary obligation can exist.
    // Core unconditionally forbids Register/Finalize/Abort carriers in genesis. Pool bootstrap
    // remains allowed. The schedule still activates strictly after genesis, never within it.
    if scheduled_at_height == 1 {
        return if effective_at_height > scheduled_at_height {
            Ok(())
        } else {
            Err(PrivateSettlementValidityErrorV1::DrainTooEarly {
                effective_at_height,
                last_expiry_height: scheduled_at_height,
            })
        };
    }
    let last_expiry_height = scheduled_at_height
        .checked_add(original_max_expiry_blocks)
        .ok_or(PrivateSettlementValidityErrorV1::DrainOverflow)?;
    if effective_at_height <= last_expiry_height {
        return Err(PrivateSettlementValidityErrorV1::DrainTooEarly {
            effective_at_height,
            last_expiry_height,
        });
    }
    Ok(())
}

/// Check that the exact sponsor controller can be satisfied within the native signing bound.
///
/// This proves only possible quorum geometry. Actual carrier admission still verifies every
/// original signature, key algorithm permission, account authorization and fee intent.
/// # Errors
/// A multisig threshold requires more than the fixed maximum number of signatures.
pub fn validate_private_settlement_sponsor_v1(
    sponsor: &AccountId,
) -> Result<(), PrivateSettlementValidityErrorV1> {
    let AccountController::Multisig(policy) = sponsor.controller() else {
        return Ok(());
    };
    let mut greatest_weights = [0_u16; PRIVATE_SETTLEMENT_MAX_CARRIER_SIGNERS_V1];
    for member in policy.members() {
        let weight = member.weight();
        if weight > greatest_weights[0] {
            greatest_weights[0] = weight;
            greatest_weights.sort_unstable();
        }
    }
    if greatest_weights.into_iter().map(u32::from).sum::<u32>() < u32::from(policy.threshold()) {
        return Err(PrivateSettlementValidityErrorV1::SponsorQuorum);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_absence_preserves_parameters_and_present_values_never_default() {
        let mut parameters = Parameters::default();
        let original_frame = norito::encode_canonical(&parameters).unwrap();
        let baseline = PrivateSettlementPolicyV1::from_parameters(&parameters).unwrap();
        assert_eq!(baseline, PrivateSettlementPolicyV1::protocol_default());
        assert_eq!(
            norito::encode_canonical(&parameters).unwrap(),
            original_frame
        );
        assert!(
            parameters
                .custom()
                .get(PrivateSettlementPolicyV1::parameter_id())
                .is_none()
        );

        let parameter = baseline.to_custom_parameter().unwrap();
        assert_eq!(
            PrivateSettlementPolicyV1::from_custom_parameter(&parameter).unwrap(),
            baseline
        );
        parameters.custom.insert(parameter);
        assert_eq!(
            PrivateSettlementPolicyV1::from_parameters(&parameters).unwrap(),
            baseline
        );
        let frame = norito::encode_canonical(&baseline).unwrap();
        assert_eq!(
            norito::decode_canonical::<PrivateSettlementPolicyV1>(&frame).unwrap(),
            baseline
        );

        let wrong = CustomParameter::new(
            "unrelated_policy".parse().unwrap(),
            Json::try_new(baseline).unwrap(),
        );
        assert!(PrivateSettlementPolicyV1::from_custom_parameter(&wrong).is_err());
        for raw in [
            "{}".to_owned(),
            "null".to_owned(),
            format!("\"{}\"", "x".repeat(4096)),
        ] {
            let malformed = CustomParameter::new(
                PrivateSettlementPolicyV1::parameter_id().clone(),
                Json::from_raw_json(raw).unwrap(),
            );
            parameters.custom.insert(malformed);
            assert!(PrivateSettlementPolicyV1::from_parameters(&parameters).is_err());
        }
        let mut invalid = baseline;
        invalid.current_limits.max_expiry_blocks = 0;
        parameters.custom.insert(CustomParameter::new(
            PrivateSettlementPolicyV1::parameter_id().clone(),
            Json::try_new(invalid).unwrap(),
        ));
        assert!(PrivateSettlementPolicyV1::from_parameters(&parameters).is_err());
        assert!(invalid.to_custom_parameter().is_err());
    }

    #[test]
    fn extension_codec_refusals_retain_original_scope_owners() {
        let policy = PrivateSettlementPolicyV1::protocol_default();
        let parameter = policy.to_custom_parameter().unwrap();
        let limits = |allocated| {
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, allocated, usize::MAX)
        };
        let (decoded, usage) =
            norito::core::with_decode_limits_measured(limits(usize::MAX), || {
                norito::json::from_str::<PrivateSettlementPolicyV1>(parameter.payload().get())
            });
        assert_eq!(decoded.unwrap(), policy);
        let demand = usage.total_allocated_bytes();
        assert!(demand > 0);
        for allocated in [0, demand] {
            let error = norito::with_decode_limits_scope(limits(allocated), || {
                PrivateSettlementPolicyV1::from_custom_parameter(&parameter)
            })
            .expect_err("decode or canonical rewrite must retain its local refusal");
            let PrivateSettlementPolicyParameterErrorV1::Codec(original) = error else {
                panic!("lost refusal: {error:?}");
            };
            assert_eq!(
                original.kind(),
                norito::core::DecodeAttemptErrorKind::EnclosingLimit
            );
            assert!(original.local_refusal().is_some());
        }
        let error = norito::with_decode_limits_scope(limits(0), || policy.to_custom_parameter())
            .unwrap_err();
        let PrivateSettlementPolicyParameterErrorV1::Codec(original) = error else {
            panic!("lost encoder refusal: {error:?}");
        };
        assert_eq!(
            original.kind(),
            norito::core::DecodeAttemptErrorKind::EnclosingLimit
        );
        assert!(original.local_refusal().is_some());
        assert_eq!(
            PrivateSettlementPolicyV1::from_custom_parameter(&parameter).unwrap(),
            policy
        );
    }

    #[test]
    fn extension_schedules_reject_no_op_short_drain_and_missed_boundaries() {
        let mut policy = PrivateSettlementPolicyV1::protocol_default();
        let mut pending = PrivateSettlementPolicyUpdateV1 {
            scheduled_at_height: 10,
            effective_at_height: 10 + policy.current_limits.max_expiry_blocks + 1,
            next_limits: policy.current_limits,
        };
        assert_eq!(
            pending.validate_against(&policy.current_limits),
            Err(PrivateSettlementPolicyValidationErrorV1::NoChange)
        );
        pending.next_limits.max_carrier_bytes -= 1;
        pending.validate_against(&policy.current_limits).unwrap();
        pending.effective_at_height -= 1;
        assert!(pending.validate_against(&policy.current_limits).is_err());
        pending.effective_at_height += 1;
        policy.pending_update = Some(pending);
        policy.validate_at_committed_height(10).unwrap();
        assert!(policy.validate_at_committed_height(9).is_err());
        assert!(
            policy
                .validate_at_committed_height(pending.effective_at_height)
                .is_err()
        );
    }

    #[test]
    fn original_privacy_layouts_keep_their_exact_canonical_frames() {
        // Independently pinned from the original 90ea8ea33b layouts: ten framed u32
        // fields, and two framed u64 heights followed by the closed unit protocol enum.
        let limits = PrivacyConsensusLimitsV1 {
            max_actions_per_transaction: 1,
            max_actions_per_block: 2,
            max_proof_bytes_per_action: 3,
            max_action_bytes: 4,
            max_privacy_bytes_per_transaction: 5,
            max_privacy_bytes_per_block: 6,
            max_statement_and_encrypted_output_bytes_per_transaction: 7,
            max_nullifiers_per_action: 8,
            max_commitments_per_action: 9,
            retained_root_count: 10,
        };
        let limits_frame = hex::decode("4E525430000090FB35B4CE6B716EF181B8C8B4FC228300320000000000000045A4DDFD313A48AB02040100000004020000000403000000040400000004050000000406000000040700000004080000000409000000040A000000").unwrap();
        assert_eq!(norito::encode_canonical(&limits).unwrap(), limits_frame);
        assert_eq!(
            norito::decode_canonical::<PrivacyConsensusLimitsV1>(&limits_frame).unwrap(),
            limits
        );
        let pending = PrivacyProtocolLimitsUpdateV1 {
            scheduled_at_height: 7,
            effective_at_height: 8,
            next_limits: PrivacyProtocolLimitsV1::ZkAcePqAuthorizationV1,
        };
        let pending_frame = hex::decode("4E5254300000F1E86538130DAD59DCBA9FF37F1B9F990017000000000000000C36AFF85B9286D9020807000000000000000808000000000000000400000000").unwrap();
        assert_eq!(norito::encode_canonical(&pending).unwrap(), pending_frame);
        assert_eq!(
            norito::decode_canonical::<PrivacyProtocolLimitsUpdateV1>(&pending_frame).unwrap(),
            pending
        );
    }

    #[test]
    fn native_policy_roundtrip_and_padding_are_exact() {
        let policy = PrivateSettlementValidityLimitsV1::protocol_default();
        policy.validate().unwrap();
        let frame = norito::encode_canonical(&policy).unwrap();
        assert_eq!(
            norito::decode_canonical::<PrivateSettlementValidityLimitsV1>(&frame).unwrap(),
            policy
        );
        let json = norito::json::to_vec(&policy).unwrap();
        assert_eq!(
            norito::json::from_slice::<PrivateSettlementValidityLimitsV1>(&json).unwrap(),
            policy
        );
        let selected = PrivateSettlementValidityLimitsV1 {
            capsule_padding_classes: 0b0101,
            ..policy
        };
        selected.validate().unwrap();
        assert!(selected.permits_padding(PrivateSettlementCapsulePaddingV1::KiB4));
        assert!(!selected.permits_padding(PrivateSettlementCapsulePaddingV1::KiB16));
        assert!(selected.permits_padding(PrivateSettlementCapsulePaddingV1::KiB64));
        assert!(!selected.permits_padding(PrivateSettlementCapsulePaddingV1::KiB256));
        for mask in [0, 0b1_0000, u8::MAX] {
            assert_eq!(
                PrivateSettlementValidityLimitsV1 {
                    capsule_padding_classes: mask,
                    ..policy
                }
                .validate(),
                Err(PrivateSettlementValidityErrorV1::Padding)
            );
        }
    }

    #[test]
    fn every_selected_padding_reserves_complete_capsule_at_auditor_boundaries() {
        for padding in [
            PrivateSettlementCapsulePaddingV1::KiB4,
            PrivateSettlementCapsulePaddingV1::KiB16,
            PrivateSettlementCapsulePaddingV1::KiB64,
            PrivateSettlementCapsulePaddingV1::KiB256,
        ] {
            for auditors in [
                1,
                u16::try_from(PRIVATE_SETTLEMENT_MAX_AUDITORS_V1).unwrap(),
            ] {
                let mut policy = PrivateSettlementValidityLimitsV1::protocol_default();
                policy.capsule_padding_classes = match padding {
                    PrivateSettlementCapsulePaddingV1::KiB4 => 1,
                    PrivateSettlementCapsulePaddingV1::KiB16 => 2,
                    PrivateSettlementCapsulePaddingV1::KiB64 => 4,
                    PrivateSettlementCapsulePaddingV1::KiB256 => 8,
                };
                policy.min_auditor_approvals = auditors;
                policy.max_capsule_bytes = private_settlement_capsule_canonical_upper_bound_v1(
                    padding.plaintext_bytes() as u64,
                    u64::from(auditors),
                );
                policy.validate().unwrap();
                policy.max_capsule_bytes -= 1;
                assert_eq!(
                    policy.validate(),
                    Err(PrivateSettlementValidityErrorV1::CapsuleFraming)
                );
            }
        }
        let mut policy = PrivateSettlementValidityLimitsV1::protocol_default();
        policy.min_auditor_approvals =
            u16::try_from(PRIVATE_SETTLEMENT_MAX_AUDITORS_V1).unwrap() + 1;
        assert_eq!(
            policy.validate(),
            Err(PrivateSettlementValidityErrorV1::NativeBound(
                "min_auditor_approvals"
            ))
        );
        policy = PrivateSettlementValidityLimitsV1::protocol_default();
        policy.max_participants = 1;
        assert_eq!(
            policy.validate(),
            Err(PrivateSettlementValidityErrorV1::NativeBound(
                "max_participants"
            ))
        );
        policy.max_participants = 2;
        policy.validate().unwrap();
    }

    #[test]
    fn signed_genesis_initialization_does_not_waive_a_later_obligation_drain() {
        let span = PrivateSettlementValidityLimitsV1::protocol_default().max_expiry_blocks;
        validate_private_settlement_drain_v1(1, span, 2).unwrap();
        assert!(validate_private_settlement_drain_v1(1, span, 1).is_err());
        assert!(validate_private_settlement_drain_v1(2, span, 3).is_err());
        validate_private_settlement_drain_v1(2, span, 3 + span).unwrap();
        let mut next = PrivateSettlementValidityLimitsV1::protocol_default();
        next.max_participants = 16;
        let current = PrivateSettlementValidityLimitsV1::protocol_default();
        let update = PrivateSettlementPolicyUpdateV1 {
            scheduled_at_height: 1,
            effective_at_height: 2,
            next_limits: next,
        };
        update.validate_against(&current).unwrap();
        let policy = PrivateSettlementPolicyV1 {
            current_limits: current,
            pending_update: Some(update),
            protocol_closure_drain: None,
        };
        policy.validate_at_committed_height(1).unwrap();
        assert_eq!(
            private_settlement_admission_deadline_v1(&policy, None),
            Some(2)
        );
        assert!(current.validate_admission(2, 1, 2, 1, Some(2)).is_err());
    }

    #[test]
    fn original_drain_is_checked_and_admission_deadline_is_exclusive() {
        let policy = PrivateSettlementValidityLimitsV1::protocol_default();
        let last = 10 + policy.max_expiry_blocks;
        assert!(validate_private_settlement_drain_v1(10, policy.max_expiry_blocks, last).is_err());
        validate_private_settlement_drain_v1(10, policy.max_expiry_blocks, last + 1).unwrap();
        assert_eq!(
            validate_private_settlement_drain_v1(u64::MAX, 1, u64::MAX),
            Err(PrivateSettlementValidityErrorV1::DrainOverflow)
        );
        policy.validate_admission(2, 20, 50, 21, Some(51)).unwrap();
        assert_eq!(
            policy.validate_admission(2, 20, 50, 21, Some(50)),
            Err(PrivateSettlementValidityErrorV1::AdmissionDrain)
        );
        assert!(policy.validate_admission(2, 20, 50, 19, None).is_err());
        assert!(policy.validate_admission(2, 20, 50, 51, None).is_err());
    }

    #[test]
    fn native_policy_ranges_accept_exact_and_refuse_first_over() {
        let exact = PrivateSettlementValidityLimitsV1 {
            max_participants: u16::try_from(ATOMIC_PRIVATE_SETTLEMENT_MAX_LEGS_V1).unwrap(),
            max_capsule_bytes: PRIVATE_SETTLEMENT_MAX_CAPSULE_BYTES_V1,
            max_carrier_bytes: PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1 as u64,
            ..PrivateSettlementValidityLimitsV1::protocol_default()
        };
        exact.validate().unwrap();
        for first_over in [
            PrivateSettlementValidityLimitsV1 {
                max_participants: exact.max_participants + 1,
                ..exact
            },
            PrivateSettlementValidityLimitsV1 {
                max_capsule_bytes: exact.max_capsule_bytes + 1,
                ..exact
            },
            PrivateSettlementValidityLimitsV1 {
                max_carrier_bytes: exact.max_carrier_bytes + 1,
                ..exact
            },
        ] {
            assert!(matches!(
                first_over.validate(),
                Err(PrivateSettlementValidityErrorV1::NativeBound(_))
            ));
        }
    }

    #[test]
    fn private_policy_rejects_invalid_native_and_phase_relationships() {
        let original = PrivateSettlementValidityLimitsV1::protocol_default();
        for invalid in [
            PrivateSettlementValidityLimitsV1 {
                max_participants: 1,
                ..original
            },
            PrivateSettlementValidityLimitsV1 {
                max_proof_bytes: PRIVATE_SETTLEMENT_MAX_PROOF_BYTES_V1 as u64 + 1,
                ..original
            },
            PrivateSettlementValidityLimitsV1 {
                min_auditor_approvals: u16::try_from(PRIVATE_SETTLEMENT_MAX_AUDITORS_V1).unwrap()
                    + 1,
                ..original
            },
            PrivateSettlementValidityLimitsV1 {
                audit_timeout_blocks: u64::MAX,
                ..original
            },
            PrivateSettlementValidityLimitsV1 {
                sidecar_retention_blocks: original.max_expiry_blocks - 1,
                ..original
            },
            PrivateSettlementValidityLimitsV1 {
                max_capsule_bytes: 1,
                ..original
            },
            PrivateSettlementValidityLimitsV1 {
                max_carrier_bytes: 0,
                ..original
            },
        ] {
            assert!(invalid.validate().is_err());
        }
    }

    #[test]
    fn later_private_policy_preserves_original_pending_protocol_drain() {
        let protocol_id = PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1;
        let mut protocol = PrivacyProtocolPolicyV1::canonical_default_v1(protocol_id);
        let PrivacyProtocolLimitsV1::IrohaIvmPrivateNoteStarkV1(mut closed) =
            protocol.current_limits
        else {
            unreachable!()
        };
        closed.max_input_count = IVM_PRIVATE_NOTE_MAX_INPUTS_V1 - 1;
        let closed = PrivacyProtocolLimitsV1::IrohaIvmPrivateNoteStarkV1(closed);
        assert!(private_settlement_protocol_closes_v1(
            &protocol.current_limits,
            &closed
        ));
        let mut original = PrivateSettlementPolicyV1::protocol_default();
        let original_span = original.current_limits.max_expiry_blocks;
        let activation = 10 + original_span + 1;
        let pending = PrivacyProtocolLimitsUpdateV1 {
            scheduled_at_height: 10,
            effective_at_height: activation,
            next_limits: closed,
        };
        protocol.pending_update = Some(pending);
        original.protocol_closure_drain = Some(
            PrivateSettlementProtocolClosureDrainV1::capture(protocol_id, &pending, original_span)
                .unwrap(),
        );
        protocol.validate_at_committed_height(20).unwrap();
        original.validate_protocol_policy(Some(&protocol)).unwrap();
        let frame = norito::encode_canonical(&original).unwrap();
        let mut later: PrivateSettlementPolicyV1 = norito::decode_canonical(&frame).unwrap();
        later.current_limits.max_expiry_blocks *= 2;
        later.validate_protocol_policy(Some(&protocol)).unwrap();
        assert_eq!(
            private_settlement_admission_deadline_v1(&later, Some(&protocol)),
            Some(activation)
        );
        assert_eq!(
            later.current_limits.validate_admission(
                2,
                20,
                activation,
                20,
                private_settlement_admission_deadline_v1(&later, Some(&protocol))
            ),
            Err(PrivateSettlementValidityErrorV1::AdmissionDrain)
        );
        let mut next = later.current_limits;
        next.max_participants -= 1;
        later.pending_update = Some(PrivateSettlementPolicyUpdateV1 {
            scheduled_at_height: 20,
            effective_at_height: 20 + later.current_limits.max_expiry_blocks + 1,
            next_limits: next,
        });
        later.validate_at_committed_height(20).unwrap();
        assert_eq!(
            private_settlement_admission_deadline_v1(&later, Some(&protocol)),
            Some(activation)
        );
        let mut malformed = later;
        malformed.protocol_closure_drain = None;
        assert!(malformed.validate_protocol_policy(Some(&protocol)).is_err());
        let mut overflow = later;
        overflow
            .protocol_closure_drain
            .as_mut()
            .unwrap()
            .original_max_expiry_blocks = u64::MAX;
        assert!(overflow.validate().is_err());
        for mutation in 0..4 {
            let mut malformed = later;
            let drain = malformed.protocol_closure_drain.as_mut().unwrap();
            match mutation {
                0 => drain.protocol_id = PrivacyProtocolIdV1::ZkAcePqAuthorizationV1,
                1 => drain.scheduled_at_height -= 1,
                2 => drain.effective_at_height += 1,
                _ => drain.next_limits_hash = iroha_crypto::Hash::new(b"substituted successor"),
            }
            assert!(malformed.validate_protocol_policy(Some(&protocol)).is_err());
        }
        assert!(later.validate_protocol_policy(None).is_err());
    }

    #[test]
    fn original_sponsor_quorum_retains_large_rosters_with_bounded_signers() {
        use crate::account::{MultisigMember, MultisigPolicy};
        use iroha_crypto::{Algorithm, KeyPair};
        let members: Vec<_> = (0..=PRIVATE_SETTLEMENT_MAX_CARRIER_SIGNERS_V1)
            .map(|index| {
                let key = KeyPair::from_seed(
                    vec![u8::try_from(index + 1).unwrap(); 32],
                    Algorithm::Ed25519,
                );
                MultisigMember::new(key.public_key().clone(), 1).unwrap()
            })
            .collect();
        let sponsor = |threshold| {
            AccountId::new_multisig(MultisigPolicy::new(threshold, members.clone()).unwrap())
        };
        validate_private_settlement_sponsor_v1(&sponsor(
            u16::try_from(PRIVATE_SETTLEMENT_MAX_CARRIER_SIGNERS_V1).unwrap(),
        ))
        .unwrap();
        assert_eq!(
            validate_private_settlement_sponsor_v1(&sponsor(
                u16::try_from(PRIVATE_SETTLEMENT_MAX_CARRIER_SIGNERS_V1).unwrap() + 1
            )),
            Err(PrivateSettlementValidityErrorV1::SponsorQuorum)
        );
        validate_private_settlement_sponsor_v1(&sponsor(2)).unwrap();
    }
}
