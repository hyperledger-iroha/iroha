//! Trigger execution event and filter

use getset::Getters;
use iroha_data_model_derive::model;

pub use self::model::*;
use super::*;
use crate::prelude::*;
#[model]
mod model {
    use super::*;

    /// Trigger execution event. Produced every time the `ExecuteTrigger` instruction is executed.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[getset(get = "pub")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct ExecuteTriggerEvent {
        /// Id of trigger to be executed
        pub trigger_id: TriggerId,
        /// Authority of user who tries to execute trigger
        pub authority: AccountId,
        /// Args to pass for trigger execution
        #[getset(skip)]
        pub args: Json,
    }

    /// Filter for [`ExecuteTriggerEvent`].
    #[derive(
        Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Default, Getters, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct ExecuteTriggerEventFilter {
        /// Id of trigger catch executions of
        pub(super) trigger_id: Option<TriggerId>,
        /// Authority of user who owns trigger
        pub(super) authority: Option<AccountId>,
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(ExecuteTriggerEvent, ExecuteTriggerEventFilter);

impl ExecuteTriggerEvent {
    /// Args to pass for trigger execution
    pub fn args(&self) -> &Json {
        &self.args
    }
}

impl ExecuteTriggerEventFilter {
    /// Creates a new [`ExecuteTriggerEventFilter`] accepting all [`ExecuteTriggerEvent`]s
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            trigger_id: None,
            authority: None,
        }
    }

    /// Modifies a [`ExecuteTriggerEventFilter`] to accept only [`ExecuteTriggerEvent`]s originating from a specific trigger
    #[must_use]
    #[inline]
    pub fn for_trigger(mut self, trigger_id: TriggerId) -> Self {
        self.trigger_id = Some(trigger_id);
        self
    }

    /// Modifies a [`ExecuteTriggerEventFilter`] to accept only [`ExecuteTriggerEvent`]s from triggers executed under specific authority
    #[must_use]
    #[inline]
    pub fn under_authority(mut self, authority: AccountId) -> Self {
        self.authority = Some(authority);
        self
    }

    /// Returns the authority constraint configured for this filter, if any.
    #[must_use]
    #[inline]
    pub fn authority(&self) -> Option<&AccountId> {
        self.authority.as_ref()
    }

    /// Ensures that this filter enforces the provided `authority`.
    ///
    /// If the filter already specifies a different authority, an error is returned.
    ///
    /// # Errors
    ///
    /// Returns an error when the filter already requires a different authority than the one
    /// provided.
    pub fn ensure_authority(mut self, authority: &AccountId) -> Result<Self, &'static str> {
        match self.authority {
            Some(ref existing) if existing != authority => {
                Err("ExecuteTrigger filter authority must match trigger owner")
            }
            Some(_) => Ok(self),
            None => {
                self.authority = Some(authority.clone());
                Ok(self)
            }
        }
    }
}

#[cfg(feature = "transparent_api")]
impl EventFilter for ExecuteTriggerEventFilter {
    type Event = ExecuteTriggerEvent;

    /// Check if `event` matches filter
    ///
    /// Event considered as matched if trigger ids are equal
    fn matches(&self, event: &ExecuteTriggerEvent) -> bool {
        if let Some(trigger_id) = &self.trigger_id
            && trigger_id != &event.trigger_id
        {
            return false;
        }
        if let Some(authority) = &self.authority
            && authority != &event.authority
        {
            return false;
        }

        true
    }
}

/// Exports common structs and enums from this module.
pub mod prelude {
    pub use super::{ExecuteTriggerEvent, ExecuteTriggerEventFilter};
}
