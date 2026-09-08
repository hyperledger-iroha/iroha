//! Validate canonical V1 multisig identities at every binary and JSON decode boundary.

use super::PublicKey;
use super::{MultisigMember, MultisigPolicy};

#[derive(crate::DeriveJsonDeserialize)]
#[norito(deny_unknown_fields, no_fast_from_json)]
struct PolicyFields {
    version: u8,
    threshold: u16,
    members: Vec<MultisigMember>,
}

#[derive(crate::DeriveJsonDeserialize)]
#[norito(deny_unknown_fields, no_fast_from_json)]
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

pub(super) fn validate_policy(value: MultisigPolicy) -> Result<MultisigPolicy, norito::Error> {
    MultisigPolicy::from_serialized(value.version, value.threshold, value.members)
        .map_err(|error| norito::Error::Message(error.to_string()))
}

pub(super) fn validate_member(value: MultisigMember) -> Result<MultisigMember, norito::Error> {
    MultisigMember::new(value.public_key, value.weight)
        .map_err(|error| norito::Error::Message(error.to_string()))
}

macro_rules! validated_decode {
    ($target:ty, $fields:ty) => {
        impl<'de> norito::core::DecodeFromSlice<'de> for $target {
            fn decode_from_slice(bytes: &'de [u8]) -> Result<(Self, usize), norito::Error> {
                // The common field decoder honors the declared packed-struct layout
                // and calls the validating archived decoder derived on the owner.
                norito::core::decode_field_canonical::<Self>(bytes)
            }
        }

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
