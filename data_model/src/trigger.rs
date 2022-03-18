//! Structures traits and impls related to `Trigger`s.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};
use core::cmp::Ordering;

use iroha_schema::IntoSchema;
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{
    metadata::Metadata, prelude::EventFilter, transaction::Executable, Identifiable, Name,
    ParseError,
};

/// Type which is used for registering a `Trigger`.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, Deserialize, Serialize, IntoSchema,
)]
pub struct Trigger {
    /// [`Id`] of the [`Trigger`].
    pub id: Id,
    /// Action to be performed when the trigger matches.
    pub action: Action,
    /// Metadata of this account as a key-value store.
    pub metadata: Metadata,
}

impl Trigger {
    /// Construct trigger, given name action and signatories.
    ///
    /// # Errors
    /// - Name is malformed
    pub fn new(name: &str, action: Action) -> Result<Self, ParseError> {
        let id = Id {
            name: Name::new(name)?,
        };
        Ok(Trigger {
            id,
            action,
            metadata: Metadata::new(),
        })
    }
}

/// Designed to differentiate between oneshot and unlimited
/// triggers. If the trigger must be run a limited number of times,
/// it's the end-user's responsibility to either unregister the
/// `Unlimited` variant.
///
/// # Considerations
///
/// - The granularity might not be sufficient to run an action exactly `n` times
/// - To be able to register an action at exact time --
/// action `repeats` field should have `Repeats::Exactly(1)` value
/// - Time-based actions are executed relative to the block creation time stamp,
/// as such the execution relative to real time is not exact,
/// and heavily depends on the amount of transactions in the current block.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize, IntoSchema)]
pub struct Action {
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
    /// Event filter to identify events on which this action should be performed.
    pub filter: EventFilter,
}

impl Action {
    /// Construct an action given `executable`, `repeats`, `technical_account` and `filter`.
    pub fn new(
        executable: impl Into<Executable>,
        repeats: impl Into<Repeats>,
        technical_account: super::account::Id,
        filter: EventFilter,
    ) -> Action {
        Action {
            executable: executable.into(),
            repeats: repeats.into(),
            // TODO: At this point the technical account is meaningless.
            technical_account,
            filter,
        }
    }
}

impl PartialOrd for Action {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Action {
    fn cmp(&self, other: &Self) -> Ordering {
        // Exclude the executable. When debugging and replacing
        // the trigger, its position in Hash and Tree maps should
        // not change depending on the content.
        match self.repeats.cmp(&other.repeats) {
            Ordering::Equal => self.technical_account.cmp(&other.technical_account),
            ord => ord,
        }
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

impl Identifiable for Trigger {
    type Id = Id;
}

impl Id {
    /// Construct [`Id`], while performing lenght checks and acceptable character validation.
    ///
    /// # Errors
    /// If name contains invalid characters.
    pub fn new(name: &str) -> Result<Self, ParseError> {
        Ok(Self {
            name: Name::new(name)?,
        })
    }
}

pub mod prelude {
    //! Re-exports of commonly used types.
    pub use super::{Action, Id as TriggerId, Repeats, Trigger};
}
