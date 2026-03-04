//! Notification events and their filter

use std::{format, string::String, vec::Vec};

use derive_more::Constructor;
use getset::Getters;
use iroha_crypto::HashOf;
use iroha_data_model_derive::model;
use iroha_macro::FromVariant;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

pub use self::model::*;
use crate::{transaction::TransactionEntrypoint, trigger::TriggerId};

#[model]
mod model {
    use super::*;

    /// Event that notifies that a trigger was executed
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Getters,
        Constructor,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[getset(get = "pub")]
    pub struct TriggerCompletedEvent {
        trigger_id: TriggerId,
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        step_index: u32,
        outcome: TriggerCompletedOutcome,
    }

    /// Enum to represent outcome of trigger execution
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, FromVariant, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub enum TriggerCompletedOutcome {
        Success,
        Failure(String),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        any(feature = "ffi_import", feature = "ffi_export"),
        derive(iroha_ffi::FfiType)
    )]
    #[repr(u8)]
    pub enum TriggerCompletedOutcomeType {
        Success,
        Failure,
    }

    impl ::core::fmt::Display for TriggerCompletedOutcomeType {
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.write_str(match self {
                Self::Success => "Success",
                Self::Failure => "Failure",
            })
        }
    }

    impl ::core::convert::TryFrom<u8> for TriggerCompletedOutcomeType {
        type Error = ();

        fn try_from(value: u8) -> Result<Self, Self::Error> {
            match value {
                0 => Ok(Self::Success),
                1 => Ok(Self::Failure),
                _ => Err(()),
            }
        }
    }

    /// Filter [`TriggerCompletedEvent`] by
    /// 1. if `trigger_id` is some filter based on trigger id
    /// 2. if `outcome_type` is some filter based on execution outcome (success/failure)
    /// 3. if both fields are none accept every event of this type
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Getters, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[getset(get = "pub")]
    pub struct TriggerCompletedEventFilter {
        pub(super) trigger_id: Option<TriggerId>,
        pub(super) outcome_type: Option<TriggerCompletedOutcomeType>,
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(
    TriggerCompletedEvent,
    TriggerCompletedOutcome,
    TriggerCompletedOutcomeType,
    TriggerCompletedEventFilter,
);

impl TriggerCompletedEventFilter {
    /// Creates a new [`TriggerCompletedEventFilter`] accepting all [`TriggerCompletedEvent`]s
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            trigger_id: None,
            outcome_type: None,
        }
    }

    /// Modifies a [`TriggerCompletedEventFilter`] to accept only [`TriggerCompletedEvent`]s originating from a specific trigger
    #[must_use]
    #[inline]
    pub fn for_trigger(mut self, trigger_id: TriggerId) -> Self {
        self.trigger_id = Some(trigger_id);
        self
    }

    /// Modifies a [`TriggerCompletedEventFilter`] to accept only [`TriggerCompletedEvent`]s with a specific outcome
    #[must_use]
    #[inline]
    pub const fn for_outcome(mut self, outcome_type: TriggerCompletedOutcomeType) -> Self {
        self.outcome_type = Some(outcome_type);
        self
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for TriggerCompletedEventFilter {
    type Event = TriggerCompletedEvent;

    /// Check if `self` accepts the `event`.
    #[inline]
    fn matches(&self, event: &Self::Event) -> bool {
        let trigger_ok = self
            .trigger_id()
            .as_ref()
            .is_none_or(|id| id == event.trigger_id());

        let outcome_ok = !matches!(
            (self.outcome_type(), event.outcome()),
            (
                Some(TriggerCompletedOutcomeType::Success),
                TriggerCompletedOutcome::Failure(_)
            ) | (
                Some(TriggerCompletedOutcomeType::Failure),
                TriggerCompletedOutcome::Success
            )
        );

        trigger_ok && outcome_ok
    }
}

/// Exports common structs and enums from this module.
pub mod prelude {
    pub use super::{
        TriggerCompletedEvent, TriggerCompletedEventFilter, TriggerCompletedOutcome,
        TriggerCompletedOutcomeType,
    };
}

#[cfg(test)]
#[cfg(feature = "transparent_api")]
mod tests {
    use super::*;
    use crate::events::EventFilter;

    #[test]
    fn trigger_completed_events_filter() {
        let trigger_id_1: TriggerId = "trigger_1".parse().expect("Valid");
        let trigger_id_2: TriggerId = "trigger_2".parse().expect("Valid");
        let dummy_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0; iroha_crypto::Hash::LENGTH]),
        );

        let event_1_failure = TriggerCompletedEvent::new(
            trigger_id_1.clone(),
            dummy_hash,
            0,
            TriggerCompletedOutcome::Failure("Error".to_string()),
        );
        let event_1_success = TriggerCompletedEvent::new(
            trigger_id_1.clone(),
            dummy_hash,
            0,
            TriggerCompletedOutcome::Success,
        );
        let event_2_failure = TriggerCompletedEvent::new(
            trigger_id_2.clone(),
            dummy_hash,
            0,
            TriggerCompletedOutcome::Failure("Error".to_string()),
        );
        let event_2_success = TriggerCompletedEvent::new(
            trigger_id_2.clone(),
            dummy_hash,
            0,
            TriggerCompletedOutcome::Success,
        );

        let filter_accept_all = TriggerCompletedEventFilter::new();
        assert!(filter_accept_all.matches(&event_1_failure));
        assert!(filter_accept_all.matches(&event_1_success));
        assert!(filter_accept_all.matches(&event_2_failure));
        assert!(filter_accept_all.matches(&event_2_success));

        let filter_accept_success =
            TriggerCompletedEventFilter::new().for_outcome(TriggerCompletedOutcomeType::Success);
        assert!(!filter_accept_success.matches(&event_1_failure));
        assert!(filter_accept_success.matches(&event_1_success));
        assert!(!filter_accept_success.matches(&event_2_failure));
        assert!(filter_accept_success.matches(&event_2_success));

        let filter_accept_failure =
            TriggerCompletedEventFilter::new().for_outcome(TriggerCompletedOutcomeType::Failure);
        assert!(filter_accept_failure.matches(&event_1_failure));
        assert!(!filter_accept_failure.matches(&event_1_success));
        assert!(filter_accept_failure.matches(&event_2_failure));
        assert!(!filter_accept_failure.matches(&event_2_success));

        let filter_accept_1 = TriggerCompletedEventFilter::new().for_trigger(trigger_id_1.clone());
        assert!(filter_accept_1.matches(&event_1_failure));
        assert!(filter_accept_1.matches(&event_1_success));
        assert!(!filter_accept_1.matches(&event_2_failure));
        assert!(!filter_accept_1.matches(&event_2_success));

        let filter_accept_1_failure = TriggerCompletedEventFilter::new()
            .for_trigger(trigger_id_1.clone())
            .for_outcome(TriggerCompletedOutcomeType::Failure);
        assert!(filter_accept_1_failure.matches(&event_1_failure));
        assert!(!filter_accept_1_failure.matches(&event_1_success));
        assert!(!filter_accept_1_failure.matches(&event_2_failure));
        assert!(!filter_accept_1_failure.matches(&event_2_success));

        let filter_accept_1_success = TriggerCompletedEventFilter::new()
            .for_trigger(trigger_id_1)
            .for_outcome(TriggerCompletedOutcomeType::Success);
        assert!(!filter_accept_1_success.matches(&event_1_failure));
        assert!(filter_accept_1_success.matches(&event_1_success));
        assert!(!filter_accept_1_success.matches(&event_2_failure));
        assert!(!filter_accept_1_success.matches(&event_2_success));

        let filter_accept_2 = TriggerCompletedEventFilter::new().for_trigger(trigger_id_2.clone());
        assert!(!filter_accept_2.matches(&event_1_failure));
        assert!(!filter_accept_2.matches(&event_1_success));
        assert!(filter_accept_2.matches(&event_2_failure));
        assert!(filter_accept_2.matches(&event_2_success));

        let filter_accept_2_failure = TriggerCompletedEventFilter::new()
            .for_trigger(trigger_id_2.clone())
            .for_outcome(TriggerCompletedOutcomeType::Failure);
        assert!(!filter_accept_2_failure.matches(&event_1_failure));
        assert!(!filter_accept_2_failure.matches(&event_1_success));
        assert!(filter_accept_2_failure.matches(&event_2_failure));
        assert!(!filter_accept_2_failure.matches(&event_2_success));

        let filter_accept_2_success = TriggerCompletedEventFilter::new()
            .for_trigger(trigger_id_2)
            .for_outcome(TriggerCompletedOutcomeType::Success);
        assert!(!filter_accept_2_success.matches(&event_1_failure));
        assert!(!filter_accept_2_success.matches(&event_1_success));
        assert!(!filter_accept_2_success.matches(&event_2_failure));
        assert!(filter_accept_2_success.matches(&event_2_success));
    }
}
