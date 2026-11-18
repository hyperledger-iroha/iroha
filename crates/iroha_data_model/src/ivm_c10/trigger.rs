//! Triggers of C10 executions.
//!
//! A trigger's public closure reaches a transaction status through its action's event filter, so
//! [`ActionC10V1`] and [`TriggerC10V1`] are new definitions with the C10 filter family
//! ([`EventFilterBoxC10V1`]). They keep every invariant of the baseline `Action`: a
//! trigger-completed filter cannot drive an action, a retry policy needs a scheduled time
//! filter, and an execute-trigger filter is bound to the action's authority. Decoding enforces
//! them exactly as the baseline decoder does, so no invalid action exists in any encoding.

use iroha_model_base::metadata::Metadata;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::events::{EventFilterBoxC10V1, c10_json_via_norito_bytes};
use crate::{
    account::AccountId,
    events::time::{ExecutionTime, TimeEventFilter},
    transaction::Executable,
    trigger::{
        TriggerId,
        action::{ActionValidationError, Repeats, TimeTriggerRetryPolicy},
    },
};

/// The action of a C10 trigger: what it executes, how often, as whom and on which events.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::ivm_c10::ActionC10V1")]
pub struct ActionC10V1 {
    /// The executable linked to this trigger: C10 bytecode or instructions.
    executable: Executable,
    /// How many times this trigger may fire.
    repeats: Repeats,
    /// Account invoking the executable.
    authority: AccountId,
    /// Condition defining which events invoke the executable, with the C10 status family.
    filter: EventFilterBoxC10V1,
    /// Optional retry policy for scheduled time triggers.
    retry_policy: Option<TimeTriggerRetryPolicy>,
    /// Arbitrary metadata stored for this trigger.
    metadata: Metadata,
}

/// The encoded fields of [`ActionC10V1`], in order; decoding validates them into an action.
#[derive(Encode, Decode)]
struct ActionCandidateC10V1 {
    executable: Executable,
    repeats: Repeats,
    authority: AccountId,
    filter: EventFilterBoxC10V1,
    retry_policy: Option<TimeTriggerRetryPolicy>,
    metadata: Metadata,
}

impl ActionCandidateC10V1 {
    /// The baseline `Action` invariants, applied to the C10 filter family.
    fn validate(self) -> Result<ActionC10V1, ActionValidationError> {
        if matches!(self.filter, EventFilterBoxC10V1::TriggerCompleted(_)) {
            return Err(ActionValidationError::TriggerCompletedFilter);
        }
        if self.retry_policy.is_some()
            && !matches!(
                self.filter,
                EventFilterBoxC10V1::Time(TimeEventFilter(ExecutionTime::Schedule(_)))
            )
        {
            return Err(ActionValidationError::RetryPolicyRequiresScheduledTimeTrigger);
        }
        let filter = match self.filter {
            EventFilterBoxC10V1::ExecuteTrigger(filter) => EventFilterBoxC10V1::ExecuteTrigger(
                filter
                    .ensure_authority(&self.authority)
                    .map_err(|_| ActionValidationError::ExecuteTriggerAuthorityMismatch)?,
            ),
            other => other,
        };
        Ok(ActionC10V1 {
            executable: self.executable,
            repeats: self.repeats,
            authority: self.authority,
            filter,
            retry_policy: self.retry_policy,
            metadata: self.metadata,
        })
    }
}

impl ActionC10V1 {
    /// Construct an action, checking the baseline `Action` invariants.
    ///
    /// # Errors
    /// The filter is a trigger-completed filter, or an execute-trigger filter names another
    /// authority.
    pub fn new(
        executable: impl Into<Executable>,
        repeats: impl Into<Repeats>,
        authority: AccountId,
        filter: impl Into<EventFilterBoxC10V1>,
    ) -> Result<Self, ActionValidationError> {
        ActionCandidateC10V1 {
            executable: executable.into(),
            repeats: repeats.into(),
            authority,
            filter: filter.into(),
            retry_policy: None,
            metadata: Metadata::default(),
        }
        .validate()
    }

    /// Replace the action's metadata.
    #[must_use]
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Attach a retry policy; only a scheduled time trigger accepts one.
    ///
    /// # Errors
    /// The action's filter is not a scheduled time filter.
    pub fn with_retry_policy(
        self,
        retry_policy: TimeTriggerRetryPolicy,
    ) -> Result<Self, ActionValidationError> {
        ActionCandidateC10V1 {
            executable: self.executable,
            repeats: self.repeats,
            authority: self.authority,
            filter: self.filter,
            retry_policy: Some(retry_policy),
            metadata: self.metadata,
        }
        .validate()
    }

    /// The executable linked to this action.
    #[must_use]
    pub fn executable(&self) -> &Executable {
        &self.executable
    }

    /// How many times this action may fire.
    #[must_use]
    pub fn repeats(&self) -> Repeats {
        self.repeats
    }

    /// Account executing this action.
    #[must_use]
    pub fn authority(&self) -> &AccountId {
        &self.authority
    }

    /// Events that invoke this action.
    #[must_use]
    pub fn filter(&self) -> &EventFilterBoxC10V1 {
        &self.filter
    }

    /// Retry policy of a scheduled time trigger.
    #[must_use]
    pub fn retry_policy(&self) -> Option<TimeTriggerRetryPolicy> {
        self.retry_policy
    }

    /// Metadata stored for this action.
    #[must_use]
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// The encoded fields, borrowed: encoding never clones the executable.
    fn candidate(&self) -> ActionCandidateRefC10V1<'_> {
        ActionCandidateRefC10V1 {
            executable: BorrowedValue(&self.executable),
            repeats: self.repeats,
            authority: BorrowedValue(&self.authority),
            filter: BorrowedValue(&self.filter),
            retry_policy: self.retry_policy,
            metadata: BorrowedValue(&self.metadata),
        }
    }
}

/// A borrowed field that encodes exactly as the owned value.
struct BorrowedValue<'a, T>(&'a T);

impl<T: norito::core::SerializePayload> norito::core::SerializePayload for BorrowedValue<'_, T> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.0.serialize(writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}

/// [`ActionCandidateC10V1`] over borrowed fields, with the same encoding.
#[derive(norito::derive::SerializePayload)]
struct ActionCandidateRefC10V1<'a> {
    executable: BorrowedValue<'a, Executable>,
    repeats: Repeats,
    authority: BorrowedValue<'a, AccountId>,
    filter: BorrowedValue<'a, EventFilterBoxC10V1>,
    retry_policy: Option<TimeTriggerRetryPolicy>,
    metadata: BorrowedValue<'a, Metadata>,
}

impl<'de> norito::core::DeserializePayload<'de> for ActionC10V1 {
    fn deserialize(archived: &'de norito::core::Archived<ActionC10V1>) -> Self {
        Self::try_deserialize(archived).expect("invalid ActionC10V1")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<ActionC10V1>,
    ) -> Result<Self, norito::core::Error> {
        let candidate =
            <ActionCandidateC10V1 as norito::core::DeserializePayload>::try_deserialize(
                archived.cast(),
            )?;
        // Exactly the baseline `Action` decoder's rules and errors.
        if crate::isi::lifecycle_instruction_validation_active() {
            // Persistent canonical evidence cannot normalize a missing by-call authority.
            if matches!(&candidate.filter, EventFilterBoxC10V1::ExecuteTrigger(filter)
                if filter.authority().is_none())
            {
                return Err(norito::core::Error::NonCanonicalEncoding);
            }
            return candidate
                .validate()
                .map_err(|_| norito::core::Error::InvalidValue {
                    context: "C10 trigger action constructor invariant",
                });
        }
        candidate
            .validate()
            .map_err(|error| norito::core::Error::Message(error.to_string()))
    }
}

impl norito::core::SerializePayload for ActionC10V1 {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        norito::core::SerializePayload::serialize(&self.candidate(), writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        norito::core::SerializePayload::encoded_len_hint(&self.candidate())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        norito::core::SerializePayload::encoded_len_exact(&self.candidate())
    }
}

/// A C10 trigger: an identifier bound to its action.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::ivm_c10::TriggerC10V1")]
pub struct TriggerC10V1 {
    /// Unique identifier of this trigger.
    pub id: TriggerId,
    /// What the trigger executes, and when.
    pub action: ActionC10V1,
}

impl TriggerC10V1 {
    /// Category in the bounded event-scanning index. Explicit-call actions use an exact ID lookup.
    pub fn event_scan_kind(&self) -> Option<crate::events::TriggeringEventType> {
        use crate::events::TriggeringEventType as Kind;
        match self.action.filter() {
            EventFilterBoxC10V1::Data(_) => Some(Kind::Data),
            EventFilterBoxC10V1::Time(_) => Some(Kind::Time),
            EventFilterBoxC10V1::Pipeline(_) => Some(Kind::Pipeline),
            EventFilterBoxC10V1::ExecuteTrigger(_) | EventFilterBoxC10V1::TriggerCompleted(_) => {
                None
            }
        }
    }
}

/// The C10 trigger and its currently scheduled retry. The retry cursor belongs to
/// execution state and cannot be supplied in a registration.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[norito(validate = "Self::checked")]
#[norito_schema(name = "iroha_data_model::ivm_c10::StoredC10TriggerV1")]
pub struct StoredC10TriggerV1 {
    trigger: TriggerC10V1,
    retry_state: Option<crate::block::execution_output::TriggerLifecycleRetryStateV1>,
}

impl StoredC10TriggerV1 {
    /// A newly registered action has no pending retry.
    pub fn new(trigger: TriggerC10V1) -> Self {
        Self {
            trigger,
            retry_state: None,
        }
    }

    /// Persistent action definition including the remaining repetitions and internal metadata.
    pub fn trigger(&self) -> &TriggerC10V1 {
        &self.trigger
    }

    /// Commit the complete registered C10 action, its identifier and its pending retry.
    /// The canonical framed schema and domain separate this commitment from baseline actions.
    ///
    /// # Errors
    /// The registered value could not be encoded within the active Norito work budget.
    pub fn action_hash(&self) -> Result<iroha_crypto::Hash, norito::Error> {
        use std::io;
        const DOMAIN: &[u8] = b"iroha:trigger:c10:stored-action:v1\0";
        let mut failure = None;
        let captured = iroha_crypto::Hash::new_from_writer(|sink| {
            let mut record = |err| {
                failure = Some(err);
                io::Error::from(io::ErrorKind::InvalidData)
            };
            if norito::core::prepaid_validation_active() {
                norito::core::charge_encode_prewrite(DOMAIN.len()).map_err(&mut record)?;
            }
            sink.write_all(DOMAIN)?;
            norito::core::write_canonical_to_writer(self, sink).map_err(record)
        });
        captured.map_err(|_| {
            failure.unwrap_or(norito::Error::InvalidValue {
                context: "C10 trigger action commitment",
            })
        })
    }

    /// Move account-owned filter selectors and action authority to their new controller.
    /// Returns whether a data predicate changed, invalidating previously captured matches.
    pub fn replace_account_id(&mut self, old: &AccountId, new: &AccountId) -> bool {
        let action = &mut self.trigger.action;
        if &action.authority == old {
            action.authority = new.clone();
        }
        match &mut action.filter {
            EventFilterBoxC10V1::Data(filter) => filter.replace_account_id(old, new),
            EventFilterBoxC10V1::ExecuteTrigger(filter) if filter.authority() == Some(old) => {
                *filter = filter.clone().under_authority(new.clone());
                false
            }
            _ => false,
        }
    }

    /// Replace the remaining count after a permitted invocation or repeat adjustment.
    pub fn set_repeats(&mut self, repeats: Repeats) {
        self.trigger.action.repeats = repeats;
    }

    /// Administrative metadata and consensus policy share the registered action's metadata.
    pub fn metadata_mut(&mut self) -> &mut Metadata {
        &mut self.trigger.action.metadata
    }

    /// Current retry attempt and earliest eligible time.
    pub fn retry_state(
        &self,
    ) -> Option<&crate::block::execution_output::TriggerLifecycleRetryStateV1> {
        self.retry_state.as_ref()
    }

    /// Change the retry cursor after the callback transaction has rolled back.
    ///
    /// # Errors
    /// A cursor requires a scheduled action with a nonzero eligible time and an attempt
    /// within its declared retry allowance.
    pub fn set_retry_state(
        &mut self,
        retry: Option<crate::block::execution_output::TriggerLifecycleRetryStateV1>,
    ) -> Result<(), norito::Error> {
        if let Some(cursor) = &retry
            && (cursor.attempt == 0
                || cursor.next_time_ms == 0
                || self
                    .trigger
                    .action
                    .retry_policy()
                    .is_none_or(|policy| cursor.attempt > policy.max_retries.get()))
        {
            return Err(norito::Error::InvalidValue {
                context: "C10 scheduled trigger retry state",
            });
        }
        self.retry_state = retry;
        Ok(())
    }

    fn checked(mut self) -> Result<Self, norito::Error> {
        let retry = self.retry_state.take();
        self.set_retry_state(retry)?;
        Ok(self)
    }
}

c10_json_via_norito_bytes!(StoredC10TriggerV1);

c10_json_via_norito_bytes!(ActionC10V1, TriggerC10V1);

/// Encodings the C10 family tests compare an action's encoding with.
#[cfg(test)]
pub(super) mod tests_support {
    use norito::codec::Encode as _;

    use super::{ActionC10V1, ActionCandidateC10V1, EventFilterBoxC10V1};
    use crate::events::trigger_completed::TriggerCompletedEventFilter;

    fn owned(action: &ActionC10V1, filter: EventFilterBoxC10V1) -> ActionCandidateC10V1 {
        ActionCandidateC10V1 {
            executable: action.executable.clone(),
            repeats: action.repeats,
            authority: action.authority.clone(),
            filter,
            retry_policy: action.retry_policy,
            metadata: action.metadata.clone(),
        }
    }

    /// Bare encoding of the action's fields as an owned candidate.
    pub(in crate::ivm_c10) fn candidate_bytes(action: &ActionC10V1) -> std::vec::Vec<u8> {
        owned(action, action.filter.clone()).encode()
    }

    /// Bare encoding of the action's fields with a trigger-completed filter, which no action
    /// may carry.
    pub(in crate::ivm_c10) fn invalid_candidate_bytes(action: &ActionC10V1) -> std::vec::Vec<u8> {
        owned(
            action,
            EventFilterBoxC10V1::TriggerCompleted(TriggerCompletedEventFilter::new()),
        )
        .encode()
    }
}
