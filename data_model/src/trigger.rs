//! Structures traits and impls related to `Trigger`s.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};
use core::{cmp, str::FromStr};

use derive_more::{Constructor, Display};
use getset::Getters;
use iroha_data_model_derive::IdEqOrdHash;
use iroha_schema::IntoSchema;
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::{
    events::prelude::*, metadata::Metadata, model, prelude::Domain, transaction::Executable,
    Identifiable, Name, ParseError, Registered,
};

model! {
    /// Identification of a `Trigger`.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Constructor, Getters, Decode, Encode, DeserializeFromStr, SerializeDisplay, IntoSchema)]
    #[getset(get = "pub")]
    #[ffi_type]
    pub struct TriggerId {
        /// Name given to trigger by its creator.
        pub name: Name,
        /// DomainId of domain of the trigger.
        pub domain_id: Option<<Domain as Identifiable>::Id>,
    }

    /// Type which is used for registering a `Trigger`.
    #[derive(Debug, Display, Clone, IdEqOrdHash, Constructor, Getters, Decode, Encode, Deserialize, Serialize, IntoSchema)]
    #[display(fmt = "@@{id}")]
    #[getset(get = "pub")]
    #[ffi_type]
    pub struct Trigger<F> {
        /// [`Id`] of the [`Trigger`].
        pub id: TriggerId,
        /// Action to be performed when the trigger matches.
        pub action: action::Action<F>,
    }
}

impl Registered for Trigger<FilterBox> {
    type With = Self;
}

impl TryFrom<Trigger<FilterBox>> for Trigger<DataEventFilter> {
    type Error = &'static str;

    fn try_from(boxed: Trigger<FilterBox>) -> Result<Self, Self::Error> {
        if let FilterBox::Data(data_filter) = boxed.action.filter {
            let action = action::Action::new(
                boxed.action.executable,
                boxed.action.repeats,
                boxed.action.technical_account,
                data_filter,
            );
            Ok(Self {
                id: boxed.id,
                action,
            })
        } else {
            Err("Expected `FilterBox::Data`, but another variant found")
        }
    }
}

impl TryFrom<Trigger<FilterBox>> for Trigger<PipelineEventFilter> {
    type Error = &'static str;

    fn try_from(boxed: Trigger<FilterBox>) -> Result<Self, Self::Error> {
        if let FilterBox::Pipeline(pipeline_filter) = boxed.action.filter {
            let action = action::Action::new(
                boxed.action.executable,
                boxed.action.repeats,
                boxed.action.technical_account,
                pipeline_filter,
            );
            Ok(Self {
                id: boxed.id,
                action,
            })
        } else {
            Err("Expected `FilterBox::Pipeline`, but another variant found")
        }
    }
}

impl TryFrom<Trigger<FilterBox>> for Trigger<TimeEventFilter> {
    type Error = &'static str;

    fn try_from(boxed: Trigger<FilterBox>) -> Result<Self, Self::Error> {
        if let FilterBox::Time(time_filter) = boxed.action.filter {
            let action = action::Action::new(
                boxed.action.executable,
                boxed.action.repeats,
                boxed.action.technical_account,
                time_filter,
            );
            Ok(Self {
                id: boxed.id,
                action,
            })
        } else {
            Err("Expected `FilterBox::Time`, but another variant found")
        }
    }
}

impl TryFrom<Trigger<FilterBox>> for Trigger<ExecuteTriggerEventFilter> {
    type Error = &'static str;

    fn try_from(boxed: Trigger<FilterBox>) -> Result<Self, Self::Error> {
        if let FilterBox::ExecuteTrigger(execute_trigger_filter) = boxed.action.filter {
            let action = action::Action::new(
                boxed.action.executable,
                boxed.action.repeats,
                boxed.action.technical_account,
                execute_trigger_filter,
            );
            Ok(Self {
                id: boxed.id,
                action,
            })
        } else {
            Err("Expected `FilterBox::ExecuteTrigger`, but another variant found")
        }
    }
}

impl core::fmt::Display for TriggerId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(ref domain_id) = self.domain_id {
            write!(f, "{}${}", self.name, domain_id)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

impl FromStr for TriggerId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.split('$');
        match (split.next(), split.next(), split.next()) {
            (Some(""), _, _) => Err(ParseError {
                reason: "Trigger ID cannot be empty",
            }),
            (Some(name), None, _) => Ok(Self {
                name: Name::from_str(name)?,
                domain_id: None,
            }),
            (Some(name), Some(domain_id), None) if !domain_id.is_empty() => Ok(Self {
                name: Name::from_str(name)?,
                domain_id: Some(<Domain as Identifiable>::Id::from_str(domain_id)?),
            }),
            _ => Err(ParseError {
                reason: "Trigger ID should have format `name` or `name$domain_id`",
            }),
        }
    }
}

pub mod action {
    //! Contains trigger action and common trait for all actions

    use iroha_primitives::atomic::AtomicU32;

    use super::*;

    /// Trait for common methods for all [`Action`]'s
    #[cfg(feature = "transparent_api")]
    pub trait ActionTrait {
        /// Get action executable
        fn executable(&self) -> &Executable;

        /// Get action repeats enum
        fn repeats(&self) -> &Repeats;

        /// Set action repeats
        fn set_repeats(&mut self, repeats: Repeats);

        /// Get action technical account
        fn technical_account(&self) -> &crate::account::AccountId;

        /// Get action metadata
        fn metadata(&self) -> &Metadata;

        /// Check if action is mintable.
        fn mintable(&self) -> bool;

        /// Convert action to a boxed representation
        fn into_boxed(self) -> Action<FilterBox>;

        /// Same as `into_boxed()` but clones `self`
        fn clone_and_box(&self) -> Action<FilterBox>;
    }

    model! {
        /// Designed to differentiate between oneshot and unlimited
        /// triggers. If the trigger must be run a limited number of times,
        /// it's the end-user's responsibility to either unregister the
        /// `Unlimited` variant.
        ///
        /// # Considerations
        ///
        /// The granularity might not be sufficient to run an action exactly
        /// `n` times. In order to ensure that it is even possible to run the
        /// triggers without gaps, the `Executable` wrapped in the action must
        /// be run before any of the ISIs are pushed into the queue of the
        /// next block.
        #[derive(Debug, Clone, PartialEq, Eq, Getters, Decode, Encode, Deserialize, Serialize, IntoSchema)]
        #[getset(get = "pub")]
        pub struct Action<F> {
            /// The executable linked to this action
            pub executable: Executable,
            /// The repeating scheme of the action. It's kept as part of the
            /// action and not inside the [`Trigger`] type, so that further
            /// sanity checking can be done.
            pub repeats: Repeats,
            /// Technical account linked to this trigger. The technical
            /// account must already exist in order for `Register<Trigger>` to
            /// work.
            pub technical_account: crate::account::AccountId,
            /// Defines events which trigger the `Action`
            pub filter: F,
            /// Metadata used as persistent storage for trigger data.
            pub metadata: Metadata,
        }
    }

    #[cfg(feature = "transparent_api")]
    impl<F: Filter> crate::HasMetadata for Action<F> {
        fn metadata(&self) -> &crate::metadata::Metadata {
            &self.metadata
        }
    }

    impl<F> Action<F> {
        /// Construct an action given `executable`, `repeats`, `technical_account` and `filter`.
        pub fn new(
            executable: impl Into<Executable>,
            repeats: impl Into<Repeats>,
            technical_account: crate::account::AccountId,
            filter: F,
        ) -> Self {
            Self {
                executable: executable.into(),
                repeats: repeats.into(),
                // TODO: At this point the technical account is meaningless.
                technical_account,
                filter,
                metadata: Metadata::new(),
            }
        }

        /// Add [`Metadata`] to the trigger replacing previously defined
        #[must_use]
        pub fn with_metadata(mut self, metadata: Metadata) -> Self {
            self.metadata = metadata;
            self
        }
    }

    #[cfg(feature = "transparent_api")]
    impl<F: Filter + Into<FilterBox> + Clone> ActionTrait for Action<F> {
        fn executable(&self) -> &Executable {
            &self.executable
        }

        fn repeats(&self) -> &Repeats {
            &self.repeats
        }

        fn set_repeats(&mut self, repeats: Repeats) {
            self.repeats = repeats;
        }

        fn technical_account(&self) -> &crate::account::AccountId {
            &self.technical_account
        }

        fn metadata(&self) -> &Metadata {
            &self.metadata
        }

        fn mintable(&self) -> bool {
            self.filter.mintable()
        }

        fn into_boxed(self) -> Action<FilterBox> {
            Action::<FilterBox>::new(
                self.executable,
                self.repeats,
                self.technical_account,
                self.filter.into(),
            )
        }

        fn clone_and_box(&self) -> Action<FilterBox> {
            Action::<FilterBox>::new(
                self.executable.clone(),
                self.repeats.clone(),
                self.technical_account.clone(),
                self.filter.clone().into(),
            )
        }
    }

    impl<F: PartialEq> PartialOrd for Action<F> {
        fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
            // Exclude the executable. When debugging and replacing
            // the trigger, its position in Hash and Tree maps should
            // not change depending on the content.
            match self.repeats.cmp(&other.repeats) {
                cmp::Ordering::Equal => {}
                ord => return Some(ord),
            }
            Some(self.technical_account.cmp(&other.technical_account))
        }
    }

    impl<F: Eq> Ord for Action<F> {
        fn cmp(&self, other: &Self) -> cmp::Ordering {
            self.partial_cmp(other)
                .expect("`PartialCmp::partial_cmp()` for `Action` should never return `None`")
        }
    }

    model! {
        /// Enumeration of possible repetitions schemes.
        #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, Deserialize, Serialize, IntoSchema)]
        pub enum Repeats {
            /// Repeat indefinitely, until the trigger is unregistered.
            Indefinitely,
            /// Repeat a set number of times
            Exactly(AtomicU32), // If you need more, use `Indefinitely`.
        }
    }

    impl PartialOrd for Repeats {
        fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for Repeats {
        fn cmp(&self, other: &Self) -> cmp::Ordering {
            match (self, other) {
                (Repeats::Indefinitely, Repeats::Indefinitely) => cmp::Ordering::Equal,
                (Repeats::Indefinitely, Repeats::Exactly(_)) => cmp::Ordering::Greater,
                (Repeats::Exactly(_), Repeats::Indefinitely) => cmp::Ordering::Less,
                (Repeats::Exactly(l), Repeats::Exactly(r)) => l.cmp(r),
            }
        }
    }

    impl From<u32> for Repeats {
        fn from(num: u32) -> Self {
            Repeats::Exactly(AtomicU32::new(num))
        }
    }

    pub mod prelude {
        //! Re-exports of commonly used types.

        #[cfg(feature = "transparent_api")]
        pub use super::action::ActionTrait;
        pub use super::{Action, Repeats};
    }
}

pub mod prelude {
    //! Re-exports of commonly used types.

    pub use super::{action::prelude::*, Trigger, TriggerId};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_with_filterbox_can_be_unboxed() {
        /// Should fail to compile if a new variant will be added to `FilterBox`
        #[allow(dead_code, clippy::unwrap_used)]
        fn compile_time_check(boxed: Trigger<FilterBox>) {
            match &boxed.action.filter {
                FilterBox::Data(_) => Trigger::<DataEventFilter>::try_from(boxed)
                    .map(|_| ())
                    .unwrap(),
                FilterBox::Pipeline(_) => Trigger::<PipelineEventFilter>::try_from(boxed)
                    .map(|_| ())
                    .unwrap(),
                FilterBox::Time(_) => Trigger::<TimeEventFilter>::try_from(boxed)
                    .map(|_| ())
                    .unwrap(),
                FilterBox::ExecuteTrigger(_) => {
                    Trigger::<ExecuteTriggerEventFilter>::try_from(boxed)
                        .map(|_| ())
                        .unwrap()
                }
            }
        }
    }
}
