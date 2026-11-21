//! Explicit signed-genesis recipe for a new independently administered child root.
//!
//! The recipe names a compiled foundation selection and its initial administrator policy. It
//! deliberately contains no eventual child genesis hash, avoiding a self-referential signature.
//! Reading this value from an ordinary parameter or snapshot never creates foundation authority.

use iroha_primitives::json::Json;

use super::{ChildAdminErrorV1, ChildRuntimeAdminPolicyV1};
use crate::{
    parameter::{CustomParameter, CustomParameterId},
    runtime::definition::SelectionId,
};

/// Maximum canonical recipe JSON retained by the authenticated genesis source.
pub const MAX_CHILD_FOUNDATION_BYTES_V1: usize = 128 * 1024;

/// The one explicit recipe signed by a fresh child's original genesis authority.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(
    name = "iroha_data_model::runtime::child_admin::foundation::ChildRuntimeFoundationV1"
)]
#[norito(deny_unknown_fields)]
pub struct ChildRuntimeFoundationV1 {
    /// Exact immutable foundation selection expected by the genesis author.
    pub selection: SelectionId,
    /// Explicit initial administrators, threshold, notice and provenance policy.
    pub policy: ChildRuntimeAdminPolicyV1,
}

impl ChildRuntimeFoundationV1 {
    /// Sole signed custom-parameter identity of the fresh-child recipe.
    #[must_use]
    pub fn parameter_id() -> &'static CustomParameterId {
        static ID: std::sync::OnceLock<CustomParameterId> = std::sync::OnceLock::new();
        ID.get_or_init(|| {
            "runtime_child_foundation_v1"
                .parse()
                .expect("static recipe identity")
        })
    }

    /// Validate shape. Only Core's original signed-genesis capability can establish authority.
    ///
    /// # Errors
    /// Rejects an absent selection identity, noninitial revision or invalid administrator policy.
    pub fn validate(&self) -> Result<(), ChildAdminErrorV1> {
        if self.selection.0 == [0; 32] || self.policy.revision != 1 {
            return Err(ChildAdminErrorV1::Shape(
                "invalid child foundation selection or initial revision",
            ));
        }
        self.policy.validate()
    }

    /// Read exact canonical recipe JSON from its original signed parameter.
    ///
    /// # Errors
    /// Rejects another identity, excessive bytes, malformed or noncanonical JSON, or invalid policy.
    pub fn from_custom_parameter(parameter: &CustomParameter) -> Result<Self, ChildAdminErrorV1> {
        if parameter.id() != Self::parameter_id()
            || parameter.payload().get().len() > MAX_CHILD_FOUNDATION_BYTES_V1
        {
            return Err(ChildAdminErrorV1::Shape(
                "invalid child foundation parameter identity or size",
            ));
        }
        let value: Self = norito::core::classify_decode_attempt(|| {
            norito::json::from_str(parameter.payload().get())
                .map_err(norito::json::Error::into_core_error)
        })
        .map_err(ChildAdminErrorV1::Resource)?;
        value.validate()?;
        let canonical = norito::core::classify_decode_attempt(|| Json::try_new(&value))
            .map_err(ChildAdminErrorV1::Resource)?;
        if &canonical != parameter.payload() {
            return Err(ChildAdminErrorV1::Shape(
                "noncanonical child foundation recipe",
            ));
        }
        Ok(value)
    }

    /// Encode the complete explicit recipe for signing inside a new genesis.
    ///
    /// # Errors
    /// Rejects invalid policy or an oversized/unencodable canonical recipe.
    pub fn to_custom_parameter(&self) -> Result<CustomParameter, ChildAdminErrorV1> {
        self.validate()?;
        let payload = norito::core::classify_decode_attempt(|| Json::try_new(self))
            .map_err(ChildAdminErrorV1::Resource)?;
        if payload.get().len() > MAX_CHILD_FOUNDATION_BYTES_V1 {
            return Err(ChildAdminErrorV1::Shape(
                "child foundation recipe exceeds byte bound",
            ));
        }
        Ok(CustomParameter::new(Self::parameter_id().clone(), payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe() -> ChildRuntimeFoundationV1 {
        ChildRuntimeFoundationV1 {
            selection: SelectionId([3; 32]),
            policy: ChildRuntimeAdminPolicyV1 {
                revision: 1,
                admins: vec![
                    iroha_crypto::KeyPair::from_seed(vec![8; 32], iroha_crypto::Algorithm::Ed25519)
                        .public_key()
                        .clone(),
                ],
                threshold: 1,
                notice: Default::default(),
                provenance: None,
            },
        }
    }

    #[test]
    fn recipe_round_trips_exactly_without_child_genesis_identity() {
        let expected = recipe();
        let parameter = expected.to_custom_parameter().unwrap();
        assert_eq!(
            ChildRuntimeFoundationV1::from_custom_parameter(&parameter).unwrap(),
            expected
        );
        let frame = norito::to_bytes(&expected).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<ChildRuntimeFoundationV1>(&frame).unwrap(),
            expected
        );
    }

    #[test]
    fn recipe_refuses_noninitial_authority_and_foreign_or_malformed_parameters() {
        let mut value = recipe();
        value.policy.revision = 2;
        assert!(value.to_custom_parameter().is_err());
        value = recipe();
        value.selection = SelectionId([0; 32]);
        assert!(value.validate().is_err());
        let parameter = recipe().to_custom_parameter().unwrap();
        let foreign =
            CustomParameter::new("unrelated".parse().unwrap(), parameter.payload().clone());
        assert!(ChildRuntimeFoundationV1::from_custom_parameter(&foreign).is_err());
        let malformed = CustomParameter::new(
            ChildRuntimeFoundationV1::parameter_id().clone(),
            Json::try_new(norito::json!({"selection": "invalid"})).unwrap(),
        );
        assert!(ChildRuntimeFoundationV1::from_custom_parameter(&malformed).is_err());
    }
}
