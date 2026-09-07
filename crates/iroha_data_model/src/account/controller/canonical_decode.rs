//! Validate canonical V1 multisig identities at every binary and JSON decode boundary.

use super::{MultisigMember, MultisigPolicy, PublicKey};

#[derive(norito::Decode)]
#[cfg_attr(feature = "json", derive(crate::DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields, no_fast_from_json))]
struct PolicyFields {
    version: u8,
    threshold: u16,
    members: Vec<MultisigMember>,
}

#[derive(norito::Decode)]
#[cfg_attr(feature = "json", derive(crate::DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields, no_fast_from_json))]
struct MemberFields {
    public_key: PublicKey,
    weight: u16,
}

impl PolicyFields {
    fn validate(self) -> Result<MultisigPolicy, super::MultisigPolicyError> {
        MultisigPolicy::from_serialized(self.version, self.threshold, self.members)
    }
}

impl MemberFields {
    fn validate(self) -> Result<MultisigMember, super::MultisigPolicyError> {
        MultisigMember::new(self.public_key, self.weight)
    }
}

macro_rules! validated_decode {
    ($target:ty, $fields:ty) => {
        impl<'de> norito::NoritoDeserialize<'de> for $target {
            fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
                Self::try_deserialize(archived).expect("canonical V1 multisig archive")
            }

            fn try_deserialize(
                archived: &'de norito::core::Archived<Self>,
            ) -> Result<Self, norito::Error> {
                let fields =
                    <$fields as norito::NoritoDeserialize>::try_deserialize(archived.cast())?;
                fields
                    .validate()
                    .map_err(|error| norito::Error::Message(error.to_string()))
            }
        }

        impl<'de> norito::core::DecodeFromSlice<'de> for $target {
            fn decode_from_slice(bytes: &'de [u8]) -> Result<(Self, usize), norito::Error> {
                // The common field decoder honors the declared packed-struct layout
                // and calls the validating archived decoder above.
                norito::core::decode_field_canonical::<Self>(bytes)
            }
        }

        #[cfg(feature = "json")]
        impl norito::json::JsonDeserialize for $target {
            fn json_deserialize(
                parser: &mut norito::json::Parser<'_>,
            ) -> Result<Self, norito::json::Error> {
                let fields = <$fields as norito::json::JsonDeserialize>::json_deserialize(parser)?;
                fields
                    .validate()
                    .map_err(|error| norito::json::Error::Message(error.to_string()))
            }
        }
    };
}

validated_decode!(MultisigPolicy, PolicyFields);
validated_decode!(MultisigMember, MemberFields);

#[cfg(test)]
mod tests;
