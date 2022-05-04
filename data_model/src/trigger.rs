//! Structures traits and impls related to `Trigger`s.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};
use core::{cmp::Ordering, fmt, str::FromStr};

use iroha_schema::IntoSchema;
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{
    events::prelude::*, metadata::Metadata, transaction::Executable, Identifiable, Name, ParseError,
};

/// Type which is used for registering a `Trigger`.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, Deserialize, Serialize, IntoSchema,
)]
pub struct Trigger<F: Filter> {
    /// [`Id`] of the [`Trigger`].
    pub id: Id,
    /// Action to be performed when the trigger matches.
    pub action: Action<F>,
}

impl<F: Filter> Trigger<F> {
    /// Construct trigger, given name action and signatories.
    pub fn new(
        id: <Self as Identifiable>::Id,
        action: Action<F>,
    ) -> <Self as Identifiable>::RegisteredWith {
        Self { id, action }
    }
}

impl TryFrom<Trigger<FilterBox>> for Trigger<DataEventFilter> {
    type Error = &'static str;

    fn try_from(boxed: Trigger<FilterBox>) -> Result<Self, Self::Error> {
        if let FilterBox::Data(data_filter) = boxed.action.filter {
            let action = Action::new(
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
            let action = Action::new(
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
            let action = Action::new(
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
            let action = Action::new(
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

impl Identifiable for Trigger<FilterBox> {
    type Id = Id;
    type RegisteredWith = Self;
}

/// Trait for common methods for all [`Action`]'s
pub trait ActionTrait {
    /// Get action executable
    fn executable(&self) -> &Executable;

    /// Get action repeats enum
    fn repeats(&self) -> Repeats;

    /// Set action repeats
    fn set_repeats(&mut self, repeats: Repeats);

    /// Get action technical account
    fn technical_account(&self) -> &super::account::Id;

    /// Check if action is mintable.
    fn mintable(&self) -> bool;
}

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
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize, IntoSchema)]
pub struct Action<F: Filter> {
    /// The executable linked to this action
    pub executable: Executable,
    /// The repeating scheme of the action. It's kept as part of the
    /// action and not inside the [`Trigger`] type, so that further
    /// sanity checking can be done.
    pub repeats: Repeats,
    /// Technical account linked to this trigger. The technical
    /// account must already exist in order for `Register<Trigger>` to
    /// work.
    pub technical_account: super::account::Id,
    /// Defines events which trigger the `Action`
    pub filter: F,
    /// Metadata used as persistent storage for trigger data.
    pub metadata: Metadata,
}

impl<F: Filter> Action<F> {
    /// Construct an action given `executable`, `repeats`, `technical_account` and `filter`.
    pub fn new(
        executable: impl Into<Executable>,
        repeats: impl Into<Repeats>,
        technical_account: super::account::Id,
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

impl<F: Filter> ActionTrait for Action<F> {
    fn executable(&self) -> &Executable {
        &self.executable
    }

    fn repeats(&self) -> Repeats {
        self.repeats
    }

    fn set_repeats(&mut self, repeats: Repeats) {
        self.repeats = repeats;
    }

    fn technical_account(&self) -> &crate::account::Id {
        &self.technical_account
    }

    fn mintable(&self) -> bool {
        self.filter.mintable()
    }
}

impl<F: Filter + PartialEq> PartialOrd for Action<F> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Exclude the executable. When debugging and replacing
        // the trigger, its position in Hash and Tree maps should
        // not change depending on the content.
        match self.repeats.cmp(&other.repeats) {
            Ordering::Equal => {}
            ord => return Some(ord),
        }
        Some(self.technical_account.cmp(&other.technical_account))
    }
}

#[allow(clippy::expect_used)]
impl<F: Filter + Eq> Ord for Action<F> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other)
            .expect("`PartialCmp::partial_cmp()` for `Action` should never return `None`")
    }
}

/// Enumeration of possible repetitions schemes.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialOrd,
    Ord,
    PartialEq,
    Eq,
    Encode,
    Decode,
    Serialize,
    Deserialize,
    IntoSchema,
)]
pub enum Repeats {
    /// Repeat indefinitely, until the trigger is unregistered.
    Indefinitely,
    /// Repeat a set number of times
    Exactly(u32), // If you need more, use `Indefinitely`.
}

impl From<u32> for Repeats {
    fn from(num: u32) -> Self {
        Repeats::Exactly(num)
    }
}

/// Identification of a `Trigger`.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
)]
pub struct Id {
    /// Name given to trigger by its creator.
    pub name: Name,
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.fmt(f)
    }
}

impl Id {
    /// Construct [`Id`], while performing lenght checks and acceptable character validation.
    ///
    /// # Errors
    /// If name contains invalid characters.
    pub fn new(name: Name) -> Self {
        Self { name }
    }
}

impl FromStr for Id {
    type Err = ParseError;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            name: Name::from_str(name)?,
        })
    }
}

pub mod prelude {
    //! Re-exports of commonly used types.
    pub use super::{Action, ActionTrait, Id as TriggerId, Repeats, Trigger};
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
