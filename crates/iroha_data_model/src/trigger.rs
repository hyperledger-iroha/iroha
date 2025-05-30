//! Structures traits and impls related to `Trigger`s.

// If editing this file, consider updating `iroha_core/src/smartcontracts/isi/triggers/specialized.rs`
// It mirrors structures from this file.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};
use core::cmp;

use derive_more::{Constructor, Display, FromStr};
use getset::Getters;
use iroha_data_model_derive::{model, IdEqOrdHash};
use iroha_macro::ffi_impl_opaque;
use iroha_schema::IntoSchema;
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};

pub use self::model::*;
use crate::{
    events::prelude::*, metadata::Metadata, transaction::Executable, Identifiable, Name, Registered,
};

#[model]
mod model {
    use super::*;

    /// Identification of a `Trigger`.
    #[derive(
        Debug,
        Display,
        FromStr,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Constructor,
        Getters,
        Decode,
        Encode,
        DeserializeFromStr,
        SerializeDisplay,
        IntoSchema,
    )]
    #[display(fmt = "{name}")]
    #[getset(get = "pub")]
    #[repr(transparent)]
    #[ffi_type(opaque)]
    pub struct TriggerId {
        /// Name given to trigger by its creator.
        pub name: Name,
    }

    /// Represents a trigger, binding an identifier to its action.
    #[derive(
        Debug, Display, Clone, IdEqOrdHash, Decode, Encode, Deserialize, Serialize, IntoSchema,
    )]
    #[display(fmt = "{id}")]
    #[ffi_type]
    pub struct Trigger {
        /// Unique identifier of this trigger.
        pub id: TriggerId,
        /// Defines when, who initiates what execution and includes persistent storage.
        pub action: action::Action,
    }
}

#[ffi_impl_opaque]
impl Trigger {
    // we can derive this with `derive_more::Constructor`, but RustRover freaks out and thinks it's signature is (TriggerId, TriggerId, Action, Action), giving bogus errors
    /// Construct a trigger given `id` and `action`.
    pub fn new(id: TriggerId, action: action::Action) -> Trigger {
        Trigger { id, action }
    }

    /// Unique identifier of the [`Trigger`].
    pub fn id(&self) -> &TriggerId {
        &self.id
    }

    /// Action to be performed when the trigger matches.
    pub fn action(&self) -> &action::Action {
        &self.action
    }
}

impl Registered for Trigger {
    type With = Self;
}

pub mod action {
    //! Contains trigger action and common trait for all actions

    use iroha_data_model_derive::model;

    pub use self::model::*;
    use super::*;
    use crate::account::AccountId;

    #[model]
    mod model {
        use super::*;

        /// Core definition of a trigger action, including the executable, firing policy, and persistent storage.
        #[derive(Debug, Clone, PartialEq, Eq, Encode, Serialize, IntoSchema)]
        #[ffi_type]
        pub struct Action {
            /// The executable linked to this trigger.
            pub executable: Executable,
            /// How many times this trigger may fire.
            pub repeats: Repeats,
            /// Account invoking the executable.
            pub authority: AccountId,
            /// Condition defining which events invoke the executable.
            pub filter: EventFilterBox,
            /// Arbitrary metadata stored for this trigger.
            pub metadata: Metadata,
        }

        /// Repetition policy for a trigger action.
        #[derive(
            Debug, Copy, Clone, PartialEq, Eq, Decode, Encode, Deserialize, Serialize, IntoSchema,
        )]
        #[ffi_type]
        pub enum Repeats {
            /// Repeat the trigger indefinitely until it is unregistered.
            Indefinitely,
            /// Repeat the trigger a fixed number of times.
            Exactly(u32),
        }
    }

    impl crate::HasMetadata for Action {
        fn metadata(&self) -> &crate::metadata::Metadata {
            &self.metadata
        }
    }

    #[ffi_impl_opaque]
    impl Action {
        /// The executable linked to this action
        pub fn executable(&self) -> &Executable {
            &self.executable
        }
        /// The repeating scheme of the action. It's kept as part of the
        /// action and not inside the [`Trigger`] type, so that further
        /// sanity checking can be done.
        pub fn repeats(&self) -> Repeats {
            self.repeats
        }
        /// Account executing this action
        pub fn authority(&self) -> &AccountId {
            &self.authority
        }
        /// Defines events which trigger the `Action`
        pub fn filter(&self) -> &EventFilterBox {
            &self.filter
        }
    }

    impl Action {
        /// Construct an action given `executable`, `repeats`, `authority` and `filter`.
        /// Filter of type [`EventFilterBox::TriggerCompleted`] is not allowed.
        ///
        /// # Panics
        ///
        /// - if filter matches [`EventFilterBox::TriggerCompleted`]
        pub fn new(
            executable: impl Into<Executable>,
            repeats: impl Into<Repeats>,
            authority: AccountId,
            filter: impl Into<EventFilterBox>,
        ) -> Self {
            let action = candidate::ActionCandidate {
                executable: executable.into(),
                repeats: repeats.into(),
                // TODO: At this point the authority is meaningless.
                authority,
                filter: filter.into(),
                metadata: Metadata::default(),
            };

            action.validate().unwrap()
        }

        /// Add [`Metadata`] to the trigger replacing previously defined
        #[must_use]
        pub fn with_metadata(mut self, metadata: Metadata) -> Self {
            self.metadata = metadata;
            self
        }
    }

    impl PartialOrd for Action {
        fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for Action {
        fn cmp(&self, other: &Self) -> cmp::Ordering {
            // Exclude the executable. When debugging and replacing
            // the trigger, its position in Hash and Tree maps should
            // not change depending on the content.
            match self.repeats.cmp(&other.repeats) {
                cmp::Ordering::Equal => {}
                ord => return ord,
            }

            self.authority.cmp(&other.authority)
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
            Repeats::Exactly(num)
        }
    }

    impl Repeats {
        /// Returns `true` if this repeat policy has no remaining executions.
        pub fn is_depleted(&self) -> bool {
            matches!(self, Repeats::Exactly(0))
        }
    }

    mod candidate {
        use parity_scale_codec::Input;

        use super::*;

        #[derive(Decode, Deserialize)]
        pub(super) struct ActionCandidate {
            pub executable: Executable,
            pub repeats: Repeats,
            pub authority: AccountId,
            pub filter: EventFilterBox,
            pub metadata: Metadata,
        }

        impl ActionCandidate {
            pub(super) fn validate(self) -> Result<Action, &'static str> {
                if matches!(self.filter, EventFilterBox::TriggerCompleted(_)) {
                    return Err("TriggerCompleted cannot be used as filter for triggering actions");
                }

                Ok(Action {
                    executable: self.executable,
                    repeats: self.repeats,
                    authority: self.authority,
                    filter: self.filter,
                    metadata: self.metadata,
                })
            }
        }

        impl Decode for Action {
            fn decode<I: Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
                ActionCandidate::decode(input)?
                    .validate()
                    .map_err(Into::into)
            }
        }
        impl<'de> Deserialize<'de> for Action {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                use serde::de::Error as _;

                ActionCandidate::deserialize(deserializer)?
                    .validate()
                    .map_err(D::Error::custom)
            }
        }
    }

    pub mod prelude {
        //! Re-exports of commonly used types.
        pub use super::{Action, Repeats};
    }
}

pub mod prelude {
    //! Re-exports of commonly used types.

    pub use super::{action::prelude::*, Trigger, TriggerId};
}

#[cfg(test)]
mod tests {
    use crate::prelude::Repeats;

    #[test]
    fn repeats_is_depleted() {
        assert!(!Repeats::Indefinitely.is_depleted());
        assert!(!Repeats::Exactly(1).is_depleted());
        assert!(Repeats::Exactly(0).is_depleted());
    }
}
