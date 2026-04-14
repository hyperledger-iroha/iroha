//! Structures traits and impls related to `Trigger`s.

// If editing this file, consider updating `iroha_core/src/smartcontracts/isi/triggers/specialized.rs`
// It mirrors structures from this file.

use std::{
    cmp, format,
    num::{NonZeroU32, NonZeroU64},
    string::String,
    vec::Vec,
};

#[cfg(feature = "json")]
use base64::Engine as _;
#[cfg(feature = "json")]
use base64::engine::general_purpose::STANDARD;
use derive_more::{Constructor, Display, FromStr};
use getset::Getters;
use iroha_data_model_derive::{IdEqOrdHash, model};
use iroha_macro::ffi_impl_opaque;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize};

pub use self::model::*;
use crate::{Identifiable, Name, Registered, metadata::Metadata, transaction::Executable};

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
        IntoSchema,
    )]
    #[display("{name}")]
    #[getset(get = "pub")]
    #[repr(transparent)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct TriggerId {
        /// Name given to trigger by its creator.
        pub name: Name,
    }

    /// Represents a trigger, binding an identifier to its action.
    #[derive(Debug, Display, Clone, IdEqOrdHash, IntoSchema)]
    #[display("{id}")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct Trigger {
        /// Unique identifier of this trigger.
        pub id: TriggerId,
        /// Defines when, who initiates what execution and includes persistent storage.
        pub action: action::Action,
    }
}

string_id!(TriggerId);

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

impl crate::HasMetadata for Trigger {
    fn metadata(&self) -> &crate::metadata::Metadata {
        crate::HasMetadata::metadata(self.action())
    }
}

mod candidate {
    use super::*;

    #[derive(Encode, Decode)]
    pub(super) struct TriggerCandidate {
        pub id: TriggerId,
        pub action: action::Action,
    }

    impl TriggerCandidate {
        fn into_trigger(self) -> Trigger {
            Trigger {
                id: self.id,
                action: self.action,
            }
        }
    }

    impl<'de> norito::core::NoritoDeserialize<'de> for Trigger {
        fn deserialize(archived: &'de norito::core::Archived<Trigger>) -> Self {
            let candidate =
                <TriggerCandidate as norito::core::NoritoDeserialize>::deserialize(archived.cast());
            candidate.into_trigger()
        }
    }

    impl norito::core::NoritoSerialize for Trigger {
        fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
            let candidate = TriggerCandidate {
                id: self.id.clone(),
                action: self.action.clone(),
            };
            norito::core::NoritoSerialize::serialize(&candidate, writer)
        }
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for Trigger {
    fn json_serialize(&self, out: &mut String) {
        let bytes = norito::to_bytes(self).expect("Trigger Norito serialization must succeed");
        let encoded = STANDARD.encode(bytes);
        json::JsonSerialize::json_serialize(&encoded, out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for Trigger {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let encoded = parser.parse_string()?;
        let bytes = STANDARD
            .decode(encoded.as_str())
            .map_err(|err| json::Error::Message(err.to_string()))?;
        let archived = norito::from_bytes::<Trigger>(&bytes)
            .map_err(|err| json::Error::Message(err.to_string()))?;
        norito::core::NoritoDeserialize::try_deserialize(archived)
            .map_err(|err| json::Error::Message(err.to_string()))
    }
}

pub mod action {
    //! Contains trigger action and common trait for all actions

    use iroha_data_model_derive::model;

    pub use self::model::*;
    use super::*;
    use crate::{
        account::AccountId,
        events::{
            EventFilterBox, data::DataEventFilter, execute_trigger::ExecuteTriggerEventFilter,
            pipeline::PipelineEventFilterBox, time::TimeEventFilter,
        },
    };

    /// Ensures that trigger filters enforce the declared action authority when applicable.
    pub trait EnsureTriggerAuthority: Sized {
        /// Returns a filter bound to the provided authority, or an error if the filter already
        /// carries a different authority.
        ///
        /// # Errors
        ///
        /// Returns an error when the filter already specifies an incompatible authority.
        fn ensure_trigger_authority(self, authority: &AccountId) -> Result<Self, &'static str>;
    }

    impl EnsureTriggerAuthority for EventFilterBox {
        fn ensure_trigger_authority(self, authority: &AccountId) -> Result<Self, &'static str> {
            match self {
                EventFilterBox::ExecuteTrigger(filter) => filter
                    .ensure_authority(authority)
                    .map(EventFilterBox::ExecuteTrigger),
                other => Ok(other),
            }
        }
    }

    impl EnsureTriggerAuthority for ExecuteTriggerEventFilter {
        fn ensure_trigger_authority(self, authority: &AccountId) -> Result<Self, &'static str> {
            self.ensure_authority(authority)
        }
    }

    impl EnsureTriggerAuthority for DataEventFilter {
        fn ensure_trigger_authority(self, _authority: &AccountId) -> Result<Self, &'static str> {
            Ok(self)
        }
    }

    impl EnsureTriggerAuthority for PipelineEventFilterBox {
        fn ensure_trigger_authority(self, _authority: &AccountId) -> Result<Self, &'static str> {
            Ok(self)
        }
    }

    impl EnsureTriggerAuthority for TimeEventFilter {
        fn ensure_trigger_authority(self, _authority: &AccountId) -> Result<Self, &'static str> {
            Ok(self)
        }
    }

    #[model]
    mod model {
        use super::*;

        /// Core definition of a trigger action, including the executable, firing policy, and persistent storage.
        #[derive(Debug, Clone, PartialEq, Eq, IntoSchema)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct Action {
            /// The executable linked to this trigger.
            pub executable: Executable,
            /// How many times this trigger may fire.
            pub repeats: Repeats,
            /// Account invoking the executable.
            pub authority: AccountId,
            /// Condition defining which events invoke the executable.
            pub filter: EventFilterBox,
            /// Optional retry policy for scheduled time triggers.
            ///
            /// Retries are opt-in, use a fixed delay based on committed block
            /// timestamps, and allow only one outstanding retry stream per
            /// trigger. Exhausting the retry budget unregisters the trigger.
            pub retry_policy: Option<TimeTriggerRetryPolicy>,
            /// Arbitrary metadata stored for this trigger.
            pub metadata: Metadata,
        }

        /// Retry policy for scheduled time triggers.
        ///
        /// The initial scheduled firing does not count toward
        /// [`max_retries`](Self::max_retries). Each failed retry attempt is
        /// delayed by [`retry_after_ms`](Self::retry_after_ms), and when the
        /// budget is exhausted the trigger is unregistered.
        #[derive(
            Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct TimeTriggerRetryPolicy {
            /// Maximum number of automatic retries after the initial failed firing.
            pub max_retries: NonZeroU32,
            /// Delay in milliseconds before the next retry becomes eligible.
            pub retry_after_ms: NonZeroU64,
        }

        /// Repetition policy for a trigger action.
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
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
        /// Optional retry policy for this action.
        pub fn retry_policy(&self) -> Option<TimeTriggerRetryPolicy> {
            self.retry_policy
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
            let filter = filter.into().ensure_trigger_authority(&authority).unwrap();
            let action = candidate::ActionCandidate {
                executable: executable.into(),
                repeats: repeats.into(),
                authority,
                filter,
                retry_policy: None,
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

        /// Attach a retry policy for scheduled time-trigger execution.
        ///
        /// Only scheduled time triggers accept retry policies. Runtime retry
        /// state is tracked internally; query surfaces continue exposing only
        /// the configured policy.
        #[must_use]
        pub fn with_retry_policy(self, retry_policy: TimeTriggerRetryPolicy) -> Self {
            candidate::ActionCandidate {
                executable: self.executable,
                repeats: self.repeats,
                authority: self.authority,
                filter: self.filter,
                retry_policy: Some(retry_policy),
                metadata: self.metadata,
            }
            .validate()
            .unwrap()
        }
    }

    #[cfg(test)]
    mod tests {

        use iroha_crypto::KeyPair;

        use super::*;
        use crate::{
            account::AccountId,
            events::execute_trigger::ExecuteTriggerEventFilter,
            events::time::{ExecutionTime, Schedule, TimeEventFilter},
            transaction::{Executable, IvmBytecode},
            trigger::TriggerId,
        };

        fn sample_executable() -> Executable {
            Executable::Ivm(IvmBytecode::from_compiled(Vec::new()))
        }

        fn account_in(_domain: &str) -> AccountId {
            let kp = KeyPair::random();
            AccountId::new(kp.public_key().clone())
        }

        #[test]
        fn bind_execute_trigger_filter_authority() {
            let authority = account_in("wonderland");
            let trigger_id: TriggerId = "test_trigger".parse().unwrap();
            let filter = ExecuteTriggerEventFilter::new().for_trigger(trigger_id);
            let action = Action::new(
                sample_executable(),
                Repeats::Exactly(1),
                authority.clone(),
                filter,
            );

            let EventFilterBox::ExecuteTrigger(bound) = action.filter().clone() else {
                panic!("expected execute trigger filter");
            };
            assert_eq!(bound.authority(), Some(&authority));
        }

        #[test]
        #[should_panic(expected = "ExecuteTrigger filter authority must match trigger owner")]
        fn reject_mismatched_execute_trigger_authority() {
            let authority = account_in("wonderland");
            let other = account_in("wonderland");
            Action::new(
                sample_executable(),
                Repeats::Exactly(1),
                authority,
                ExecuteTriggerEventFilter::new().under_authority(other),
            );
        }

        #[test]
        fn retry_policy_accepts_scheduled_time_trigger() {
            let authority = account_in("wonderland");
            let retry_policy = TimeTriggerRetryPolicy {
                max_retries: NonZeroU32::new(2).expect("nonzero"),
                retry_after_ms: NonZeroU64::new(500).expect("nonzero"),
            };
            let action = Action::new(
                sample_executable(),
                Repeats::Exactly(1),
                authority,
                TimeEventFilter::new(ExecutionTime::Schedule(Schedule::starting_at(
                    std::time::Duration::from_millis(5),
                ))),
            )
            .with_retry_policy(retry_policy);

            assert_eq!(action.retry_policy(), Some(retry_policy));
        }

        #[test]
        #[should_panic(
            expected = "retry policy is only supported for scheduled time-trigger actions"
        )]
        fn retry_policy_rejects_non_scheduled_trigger() {
            let authority = account_in("wonderland");
            let retry_policy = TimeTriggerRetryPolicy {
                max_retries: NonZeroU32::new(1).expect("nonzero"),
                retry_after_ms: NonZeroU64::new(10).expect("nonzero"),
            };
            let trigger_id: TriggerId = "test_trigger".parse().unwrap();
            let action = Action::new(
                sample_executable(),
                Repeats::Exactly(1),
                authority,
                ExecuteTriggerEventFilter::new().for_trigger(trigger_id),
            );
            let _ = action.with_retry_policy(retry_policy);
        }

        #[test]
        fn action_with_retry_policy_norito_roundtrip() {
            let authority = account_in("wonderland");
            let retry_policy = TimeTriggerRetryPolicy {
                max_retries: NonZeroU32::new(3).expect("nonzero"),
                retry_after_ms: NonZeroU64::new(1_000).expect("nonzero"),
            };
            let action = Action::new(
                sample_executable(),
                Repeats::Exactly(2),
                authority,
                TimeEventFilter::new(ExecutionTime::Schedule(
                    Schedule::starting_at(std::time::Duration::from_secs(1))
                        .with_period(std::time::Duration::from_secs(5)),
                )),
            )
            .with_retry_policy(retry_policy);

            let bytes = norito::to_bytes(&action).expect("serialize action");
            let decoded = norito::from_bytes::<Action>(&bytes).expect("decode action bytes");
            let restored = norito::core::NoritoDeserialize::try_deserialize(decoded)
                .expect("deserialize action");
            assert_eq!(restored, action);
        }

        #[cfg(feature = "json")]
        #[test]
        fn action_with_retry_policy_json_roundtrip() {
            let authority = account_in("wonderland");
            let retry_policy = TimeTriggerRetryPolicy {
                max_retries: NonZeroU32::new(2).expect("nonzero"),
                retry_after_ms: NonZeroU64::new(250).expect("nonzero"),
            };
            let action = Action::new(
                sample_executable(),
                Repeats::Exactly(1),
                authority,
                TimeEventFilter::new(ExecutionTime::Schedule(Schedule::starting_at(
                    std::time::Duration::from_millis(25),
                ))),
            )
            .with_retry_policy(retry_policy);

            let json = norito::json::to_json(&action).expect("serialize action to json");
            let restored: Action = norito::json::from_json(&json).expect("deserialize action");
            assert_eq!(restored, action);
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

    #[cfg(feature = "json")]
    impl JsonSerialize for Repeats {
        fn json_serialize(&self, out: &mut String) {
            out.push('{');
            match self {
                Repeats::Indefinitely => {
                    json::write_json_string("Indefinitely", out);
                    out.push_str(":null");
                }
                Repeats::Exactly(count) => {
                    json::write_json_string("Exactly", out);
                    out.push(':');
                    json::JsonSerialize::json_serialize(count, out);
                }
            }
            out.push('}');
        }
    }

    #[cfg(feature = "json")]
    impl JsonDeserialize for Repeats {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            parser.skip_ws();
            parser.consume_char(b'{')?;
            parser.skip_ws();
            let key = parser.parse_key()?;
            let result = match key.as_str() {
                "Indefinitely" => {
                    parser.parse_null()?;
                    Repeats::Indefinitely
                }
                "Exactly" => {
                    let value = u32::json_deserialize(parser)?;
                    Repeats::Exactly(value)
                }
                other => {
                    return Err(json::Error::unknown_field(other.to_owned()));
                }
            };
            parser.skip_ws();
            parser.consume_char(b'}')?;
            Ok(result)
        }
    }

    #[cfg(feature = "json")]
    impl JsonSerialize for Action {
        fn json_serialize(&self, out: &mut String) {
            let bytes = norito::to_bytes(self).expect("Action Norito serialization must succeed");
            let encoded = STANDARD.encode(bytes);
            json::JsonSerialize::json_serialize(&encoded, out);
        }
    }

    #[cfg(feature = "json")]
    impl JsonDeserialize for Action {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let encoded = parser.parse_string()?;
            let bytes = STANDARD
                .decode(encoded.as_str())
                .map_err(|err| json::Error::Message(err.to_string()))?;
            let archived = norito::from_bytes::<Action>(&bytes)
                .map_err(|err| json::Error::Message(err.to_string()))?;
            norito::core::NoritoDeserialize::try_deserialize(archived)
                .map_err(|err| json::Error::Message(err.to_string()))
        }
    }

    #[cfg(feature = "json")]
    impl JsonSerialize for TimeTriggerRetryPolicy {
        fn json_serialize(&self, out: &mut String) {
            let bytes = norito::to_bytes(self)
                .expect("TimeTriggerRetryPolicy Norito serialization must succeed");
            let encoded = STANDARD.encode(bytes);
            json::JsonSerialize::json_serialize(&encoded, out);
        }
    }

    #[cfg(feature = "json")]
    impl JsonDeserialize for TimeTriggerRetryPolicy {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let encoded = parser.parse_string()?;
            let bytes = STANDARD
                .decode(encoded.as_str())
                .map_err(|err| json::Error::Message(err.to_string()))?;
            let archived = norito::from_bytes::<TimeTriggerRetryPolicy>(&bytes)
                .map_err(|err| json::Error::Message(err.to_string()))?;
            norito::core::NoritoDeserialize::try_deserialize(archived)
                .map_err(|err| json::Error::Message(err.to_string()))
        }
    }

    mod candidate {
        use super::{EnsureTriggerAuthority, *};

        #[derive(Encode, Decode)]
        pub(super) struct ActionCandidate {
            pub executable: Executable,
            pub repeats: Repeats,
            pub authority: AccountId,
            pub filter: EventFilterBox,
            pub retry_policy: Option<TimeTriggerRetryPolicy>,
            pub metadata: Metadata,
        }

        impl ActionCandidate {
            pub(super) fn validate(self) -> Result<Action, &'static str> {
                if matches!(self.filter, EventFilterBox::TriggerCompleted(_)) {
                    return Err("TriggerCompleted cannot be used as filter for triggering actions");
                }

                if self.retry_policy.is_some()
                    && !matches!(
                        self.filter,
                        EventFilterBox::Time(crate::events::time::TimeEventFilter(
                            crate::events::time::ExecutionTime::Schedule(_)
                        ))
                    )
                {
                    return Err(
                        "retry policy is only supported for scheduled time-trigger actions",
                    );
                }

                let Self {
                    executable,
                    repeats,
                    authority,
                    filter,
                    retry_policy,
                    metadata,
                } = self;

                let filter = filter.ensure_trigger_authority(&authority)?;

                Ok(Action {
                    executable,
                    repeats,
                    authority,
                    filter,
                    retry_policy,
                    metadata,
                })
            }
        }

        impl<'de> norito::core::NoritoDeserialize<'de> for Action {
            fn deserialize(archived: &'de norito::core::Archived<Action>) -> Self {
                let candidate = <ActionCandidate as norito::core::NoritoDeserialize>::deserialize(
                    archived.cast(),
                );
                candidate.validate().expect("invalid Action")
            }
        }

        impl norito::core::NoritoSerialize for Action {
            fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
                let candidate = ActionCandidate {
                    executable: self.executable.clone(),
                    repeats: self.repeats,
                    authority: self.authority.clone(),
                    filter: self.filter.clone(),
                    retry_policy: self.retry_policy,
                    metadata: self.metadata.clone(),
                };
                norito::core::NoritoSerialize::serialize(&candidate, writer)
            }
        }
    }

    pub mod prelude {
        //! Re-exports of commonly used types.
        pub use super::{Action, Repeats, TimeTriggerRetryPolicy};
    }
}

pub mod prelude {
    //! Re-exports of commonly used types.

    pub use super::{Trigger, TriggerId, action::prelude::*};
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

    #[cfg(feature = "json")]
    #[test]
    fn repeats_json_roundtrip() {
        let exact = Repeats::Exactly(3);
        let json = norito::json::to_json(&exact).expect("serialize exactly");
        let decoded: Repeats = norito::json::from_str(&json).expect("deserialize exactly");
        assert_eq!(exact, decoded);

        let indefinite = Repeats::Indefinitely;
        let json = norito::json::to_json(&indefinite).expect("serialize indefinitely");
        let decoded: Repeats = norito::json::from_str(&json).expect("deserialize indefinitely");
        assert_eq!(indefinite, decoded);
    }
}
