//! Exact string JSON for unit-only compute configuration policies.
//!
//! These policies carry no payload. Their canonical string form is shared by
//! manifests, SDK JSON and TOML configuration; object envelopes are rejected.

use norito::json;

use super::{
    ComputeAuthPolicy, ComputePriceRiskClass, ComputeRandomnessPolicy, ComputeStorageAccess,
};

macro_rules! policy_json {
    ($policy:ty { $($variant:ident),+ $(,)? }) => {
        impl json::JsonSerialize for $policy {
            fn json_serialize(&self, out: &mut String) {
                json::write_json_string(
                    match self {
                        $(Self::$variant => stringify!($variant),)+
                    },
                    out,
                );
            }

            fn json_serialize_to(
                &self,
                out: &mut dyn json::JsonWriteSink,
            ) -> Result<(), json::BoundedJsonError> {
                json::write_json_string_to(
                    match self {
                        $(Self::$variant => stringify!($variant),)+
                    },
                    out,
                )
            }
        }

        impl json::JsonDeserialize for $policy {
            fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
                let value = parser.parse_string()?;
                match value.as_str() {
                    $(stringify!($variant) => Ok(Self::$variant),)+
                    _ => Err(json::Error::Message(format!(
                        "unknown {} variant: {value}",
                        stringify!($policy),
                    ))),
                }
            }
        }
    };
}

policy_json!(ComputeRandomnessPolicy {
    None,
    SeededFromRequest
});
policy_json!(ComputeStorageAccess {
    ReadOnly,
    ReadWrite
});
policy_json!(ComputeAuthPolicy {
    PublicOnly,
    AuthenticatedOnly,
    Either
});
policy_json!(ComputePriceRiskClass {
    Low,
    Balanced,
    High
});

#[cfg(test)]
mod tests;
