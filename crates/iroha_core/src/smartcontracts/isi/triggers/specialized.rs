//! Contains trigger-related types that are specialized for core-specific needs.

use derive_more::Constructor;
use iroha_crypto::HashOf;
use iroha_data_model::{
    account::AccountId,
    events::{EventFilter, EventFilterBox},
    metadata::Metadata,
    prelude::*,
    trigger::action::EnsureTriggerAuthority,
};
use iroha_logger::trace;
#[cfg(feature = "json")]
use norito::json::native::Number as JsonNumber;
#[cfg(feature = "json")]
use norito::json::{self, JsonSerialize as JsonSerializeTrait, Value as JsonValue};

use crate::smartcontracts::triggers::set::ExecutableRef;

/// Same as [`iroha_data_model::trigger::action::Action`] but generic over the filter type
///
/// This is used to split different action types to different collections
#[derive(Debug, Clone)]
pub struct SpecializedAction<F> {
    /// The executable linked to this action
    pub executable: Executable,
    /// The repeating scheme of the action. It's kept as part of the
    /// action and not inside the [`iroha_data_model::trigger::Trigger`] type, so that further
    /// sanity checking can be done.
    pub repeats: Repeats,
    /// Account executing this action
    pub authority: AccountId,
    /// Defines events which trigger the `Action`
    pub filter: F,
    /// Metadata used as persistent storage for trigger data.
    pub metadata: Metadata,
}

impl<F> SpecializedAction<F> {
    /// Construct a specialized action given `executable`, `repeats`, `authority` and `filter`.
    pub fn new(
        executable: impl Into<Executable>,
        repeats: impl Into<Repeats>,
        authority: AccountId,
        filter: F,
    ) -> Self
    where
        F: EnsureTriggerAuthority,
    {
        let executable = executable.into();
        let repeats = repeats.into();
        let filter = filter.ensure_trigger_authority(&authority).unwrap();
        Self {
            executable,
            repeats,
            authority,
            filter,
            metadata: Metadata::default(),
        }
    }
}

#[cfg(feature = "json")]
fn parse_repeats(value: JsonValue) -> Result<Repeats, json::Error> {
    match value {
        JsonValue::Object(map) => {
            if map.len() != 1 {
                return Err(json::Error::InvalidField {
                    field: "repeats".to_owned(),
                    message: "expected exactly one variant field".to_owned(),
                });
            }
            let (key, inner) = map.into_iter().next().expect("checked map length");
            match key.as_str() {
                "Indefinitely" => match inner {
                    JsonValue::Null => Ok(Repeats::Indefinitely),
                    other => Err(json::Error::InvalidField {
                        field: "repeats".to_owned(),
                        message: format!(
                            "expected null payload for `Indefinitely`, found {other:?}"
                        ),
                    }),
                },
                "Exactly" => match inner {
                    JsonValue::Number(number) => {
                        let value = match number {
                            JsonNumber::I64(v) => {
                                u32::try_from(v).map_err(|_| json::Error::InvalidField {
                                    field: "repeats".to_owned(),
                                    message: "Exactly variant requires non-negative u32".to_owned(),
                                })?
                            }
                            JsonNumber::U64(v) => {
                                u32::try_from(v).map_err(|_| json::Error::InvalidField {
                                    field: "repeats".to_owned(),
                                    message: "Exactly variant requires value within u32 range"
                                        .to_owned(),
                                })?
                            }
                            JsonNumber::F64(_) => {
                                return Err(json::Error::InvalidField {
                                    field: "repeats".to_owned(),
                                    message: "Exactly variant expects integer payload".to_owned(),
                                });
                            }
                        };
                        Ok(Repeats::Exactly(value))
                    }
                    other => Err(json::Error::InvalidField {
                        field: "repeats".to_owned(),
                        message: format!("expected numeric payload for `Exactly`, found {other:?}"),
                    }),
                },
                other => Err(json::Error::unknown_field(other)),
            }
        }
        other => Err(json::Error::InvalidField {
            field: "repeats".to_owned(),
            message: format!("expected JSON object, found {other:?}"),
        }),
    }
}

#[cfg(feature = "json")]
impl<F> json::FastJsonWrite for SpecializedAction<F>
where
    F: json::JsonSerialize,
{
    fn write_json(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        let mut write = |name: &str, serialize: &dyn Fn(&mut String)| {
            if first {
                first = false;
            } else {
                out.push(',');
            }
            json::write_json_string(name, out);
            out.push(':');
            serialize(out);
        };
        write("executable", &|out| {
            JsonSerializeTrait::json_serialize(&self.executable, out);
        });
        write("repeats", &|out| {
            JsonSerializeTrait::json_serialize(&self.repeats, out);
        });
        write("authority", &|out| {
            JsonSerializeTrait::json_serialize(&self.authority, out);
        });
        write("filter", &|out| {
            JsonSerializeTrait::json_serialize(&self.filter, out);
        });
        write("metadata", &|out| {
            JsonSerializeTrait::json_serialize(&self.metadata, out);
        });
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl<F> json::JsonDeserialize for SpecializedAction<F>
where
    F: json::JsonDeserialize + EnsureTriggerAuthority,
{
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let mut visitor = json::MapVisitor::new(parser)?;
        let mut executable: Option<Executable> = None;
        let mut repeats: Option<Repeats> = None;
        let mut authority: Option<AccountId> = None;
        let mut filter: Option<F> = None;
        let mut metadata: Option<Metadata> = None;

        while let Some(key) = visitor.next_key()? {
            eprintln!("LoadedAction::json_deserialize field {}", key.as_str());
            match key.as_str() {
                "executable" => {
                    if executable.is_some() {
                        return Err(json::MapVisitor::duplicate_field("executable"));
                    }
                    executable = Some(visitor.parse_value()?);
                }
                "repeats" => {
                    if repeats.is_some() {
                        return Err(json::MapVisitor::duplicate_field("repeats"));
                    }
                    repeats = Some(visitor.parse_value()?);
                }
                "authority" => {
                    if authority.is_some() {
                        return Err(json::MapVisitor::duplicate_field("authority"));
                    }
                    authority = Some(visitor.parse_value()?);
                }
                "filter" => {
                    if filter.is_some() {
                        return Err(json::MapVisitor::duplicate_field("filter"));
                    }
                    filter = Some(visitor.parse_value()?);
                }
                "metadata" => {
                    if metadata.is_some() {
                        return Err(json::MapVisitor::duplicate_field("metadata"));
                    }
                    metadata = Some(visitor.parse_value()?);
                }
                other => {
                    visitor.skip_value()?;
                    trace!(%other, "ignoring unknown trigger action field");
                }
            }
        }

        visitor.finish()?;

        let executable = executable.ok_or_else(|| json::MapVisitor::missing_field("executable"))?;
        let repeats = repeats.ok_or_else(|| json::MapVisitor::missing_field("repeats"))?;
        let authority = authority.ok_or_else(|| json::MapVisitor::missing_field("authority"))?;
        let filter = filter.ok_or_else(|| json::MapVisitor::missing_field("filter"))?;
        let metadata = metadata.ok_or_else(|| json::MapVisitor::missing_field("metadata"))?;

        let mut action = Self::new(executable, repeats, authority, filter);
        action.metadata = metadata;
        Ok(action)
    }
}
#[allow(clippy::fallible_impl_from)] // `SpecializedAction::new` already ensures the filter authority.
impl<F> From<SpecializedAction<F>> for Action
where
    F: Into<EventFilterBox> + EnsureTriggerAuthority,
{
    fn from(value: SpecializedAction<F>) -> Self {
        let filter = value
            .filter
            .ensure_trigger_authority(&value.authority)
            .unwrap()
            .into();
        Action {
            executable: value.executable,
            repeats: value.repeats,
            authority: value.authority,
            filter,
            metadata: value.metadata,
        }
    }
}

/// Same as [`iroha_data_model::trigger::Trigger`] but generic over the filter type
#[derive(Constructor)]
pub struct SpecializedTrigger<F> {
    /// Unique identifier of the [`Trigger`].
    pub id: TriggerId,
    /// Action to be performed when the trigger matches.
    pub action: SpecializedAction<F>,
}

macro_rules! impl_try_from_box {
    ($($variant:ident => $filter_type:ty),+ $(,)?) => {
        $(
            impl TryFrom<Trigger> for SpecializedTrigger<$filter_type> {
                type Error = &'static str;

                fn try_from(boxed: Trigger) -> Result<Self, Self::Error> {
                    if let EventFilterBox::$variant(concrete_filter) = boxed.action().filter().clone() {
                        let iroha_data_model::trigger::action::Action {
                            executable,
                            repeats,
                            authority,
                            metadata,
                            ..
                        } = boxed.action().clone();
                        let action = SpecializedAction {
                            executable,
                            repeats,
                            authority,
                            filter: concrete_filter,
                            metadata,
                        };
                        Ok(Self {
                            id: boxed.id().clone(),
                            action,
                        })
                    } else {
                        Err(concat!("Expected `EventFilterBox::", stringify!($variant),"`, but another variant found"))
                    }
                }
            }
        )+
    };
}

impl_try_from_box! {
    Data => DataEventFilter,
    Pipeline => PipelineEventFilterBox,
    Time => TimeEventFilter,
    ExecuteTrigger => ExecuteTriggerEventFilter,
}

/// Same as [`iroha_data_model::trigger::action::Action`] but with
/// a reference to a pre-loaded executable.
#[derive(Clone, Debug)]
pub struct LoadedAction<F> {
    /// Reference to the pre-loaded executable.
    pub(super) executable: ExecutableRef,
    /// How many times this trigger may fire.
    pub repeats: Repeats,
    /// Account invoking the executable.
    pub authority: AccountId,
    /// Condition defining which events invoke the executable.
    pub filter: F,
    /// Arbitrary metadata stored for this trigger.
    pub metadata: Metadata,
}

impl<F> LoadedAction<F> {
    pub(super) fn extract_blob_hash(&self) -> Option<HashOf<IvmBytecode>> {
        match self.executable {
            ExecutableRef::Ivm(blob_hash) => Some(blob_hash),
            ExecutableRef::Instructions(_) => None,
        }
    }
}

#[cfg(feature = "json")]
impl<F> json::FastJsonWrite for LoadedAction<F>
where
    F: json::JsonSerialize,
{
    fn write_json(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        let mut write = |name: &str, serialize: &dyn Fn(&mut String)| {
            if first {
                first = false;
            } else {
                out.push(',');
            }
            json::write_json_string(name, out);
            out.push(':');
            serialize(out);
        };
        write("executable", &|out| {
            JsonSerializeTrait::json_serialize(&self.executable, out);
        });
        write("repeats", &|out| {
            JsonSerializeTrait::json_serialize(&self.repeats, out);
        });
        write("authority", &|out| {
            JsonSerializeTrait::json_serialize(&self.authority, out);
        });
        write("filter", &|out| {
            JsonSerializeTrait::json_serialize(&self.filter, out);
        });
        write("metadata", &|out| {
            JsonSerializeTrait::json_serialize(&self.metadata, out);
        });
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl<F> json::JsonDeserialize for LoadedAction<F>
where
    F: json::JsonDeserialize,
{
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = JsonValue::json_deserialize(parser)?;
        let mut map = match value {
            JsonValue::Object(map) => map,
            other => {
                return Err(json::Error::InvalidField {
                    field: "LoadedAction".to_owned(),
                    message: format!(
                        "expected JSON object, found {other:?}. This is a bug in trigger serialization."
                    ),
                });
            }
        };

        let mut take_field = |name: &str| -> Result<JsonValue, json::Error> {
            map.remove(name)
                .ok_or_else(|| json::Error::missing_field(name))
        };

        let executable = json::from_value(take_field("executable")?)?;
        let repeats = parse_repeats(take_field("repeats")?)?;
        let authority = json::from_value(take_field("authority")?)?;
        let filter = json::from_value(take_field("filter")?)?;
        let metadata = json::from_value(take_field("metadata")?)?;

        for (key, _) in map {
            trace!(%key, "ignoring unknown loaded trigger action field");
        }

        Ok(Self {
            executable,
            repeats,
            authority,
            filter,
            metadata,
        })
    }
}

/// Trait common for all `LoadedAction`s
pub trait LoadedActionTrait {
    /// Get action executable
    fn executable(&self) -> &ExecutableRef;

    /// Get action repeats enum
    fn repeats(&self) -> &Repeats;

    /// Set action repeats
    fn set_repeats(&mut self, repeats: Repeats);

    /// Get action technical account
    fn authority(&self) -> &AccountId;

    /// Get action metadata
    fn metadata(&self) -> &Metadata;

    /// Get action metadata
    fn metadata_mut(&mut self) -> &mut Metadata;

    /// Check if action is mintable.
    fn mintable(&self) -> bool;

    /// Convert action to a boxed representation
    fn into_boxed(self) -> LoadedAction<EventFilterBox>;

    /// Same as [`into_boxed()`](LoadedActionTrait::into_boxed) but clones `self`
    fn clone_and_box(&self) -> LoadedAction<EventFilterBox>;
}

impl<F: EventFilter + Into<EventFilterBox> + Clone> LoadedActionTrait for LoadedAction<F> {
    fn executable(&self) -> &ExecutableRef {
        &self.executable
    }

    fn repeats(&self) -> &iroha_data_model::trigger::action::Repeats {
        &self.repeats
    }

    fn set_repeats(&mut self, repeats: iroha_data_model::trigger::action::Repeats) {
        self.repeats = repeats;
    }

    fn authority(&self) -> &AccountId {
        &self.authority
    }

    fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    fn metadata_mut(&mut self) -> &mut Metadata {
        &mut self.metadata
    }

    fn mintable(&self) -> bool {
        self.filter.mintable()
    }

    fn into_boxed(self) -> LoadedAction<EventFilterBox> {
        let Self {
            executable,
            repeats,
            authority,
            filter,
            metadata,
        } = self;

        LoadedAction {
            executable,
            repeats,
            authority,
            filter: filter.into(),
            metadata,
        }
    }

    fn clone_and_box(&self) -> LoadedAction<EventFilterBox> {
        self.clone().into_boxed()
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "json")]
    use iroha_crypto::KeyPair;
    #[cfg(feature = "json")]
    use iroha_data_model::prelude::{
        AccountId, DataEventFilter, InstructionBox, Level, Log, Metadata, Repeats,
    };
    #[cfg(feature = "json")]
    use iroha_primitives::const_vec::ConstVec;

    use super::*;

    #[test]
    fn trigger_with_filterbox_can_be_unboxed() {
        /// Should fail to compile if a new variant will be added to `EventFilterBox`
        #[allow(dead_code)]
        fn compile_time_check(boxed: Trigger) {
            match boxed.action().filter().clone() {
                EventFilterBox::Data(_) => SpecializedTrigger::<DataEventFilter>::try_from(boxed)
                    .map(|_| ())
                    .unwrap(),
                EventFilterBox::Pipeline(_) => {
                    SpecializedTrigger::<PipelineEventFilterBox>::try_from(boxed)
                        .map(|_| ())
                        .unwrap()
                }
                EventFilterBox::Time(_) => SpecializedTrigger::<TimeEventFilter>::try_from(boxed)
                    .map(|_| ())
                    .unwrap(),
                EventFilterBox::ExecuteTrigger(_) => {
                    SpecializedTrigger::<ExecuteTriggerEventFilter>::try_from(boxed)
                        .map(|_| ())
                        .unwrap()
                }
                EventFilterBox::TriggerCompleted(_) => {
                    unreachable!("Disallowed during deserialization")
                }
            }
        }
    }

    #[cfg(feature = "json")]
    #[test]
    fn loaded_action_json_roundtrip() {
        let instruction = InstructionBox::from(Log::new(Level::INFO, "ping".to_owned()));
        let instructions = ConstVec::from(vec![instruction]);
        let executable = ExecutableRef::Instructions(instructions.clone());
        let action = LoadedAction {
            executable,
            repeats: Repeats::Exactly(3),
            authority: AccountId::new(KeyPair::random().public_key().clone()),
            filter: DataEventFilter::Any,
            metadata: Metadata::default(),
        };

        let json = norito::json::to_json(&action).expect("serialize LoadedAction");
        let reparsed: LoadedAction<DataEventFilter> =
            norito::json::from_json(&json).expect("deserialize LoadedAction");

        assert_eq!(reparsed.repeats, action.repeats);
        assert_eq!(
            reparsed.authority.subject_id(),
            action.authority.subject_id()
        );
        assert_eq!(reparsed.filter, action.filter);
        assert_eq!(reparsed.metadata, action.metadata);

        match reparsed.executable {
            ExecutableRef::Instructions(restored) => assert_eq!(restored, instructions),
            ExecutableRef::Ivm(_) => panic!("expected instruction executable"),
        }
    }
}
