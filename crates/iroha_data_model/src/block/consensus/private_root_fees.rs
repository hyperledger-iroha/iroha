//! Genesis-committed transaction charges for an independent private root.

use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize,
    asset::AssetDefinitionId,
    parameter::{CustomParameter, CustomParameterId, Parameters},
};

/// Immutable private-ledger fee currency and rates, installed by signed genesis.
///
/// Execution additionally requires the currency to be restricted to the exact signed root
/// dataspace. Fees burn that dataspace's balance; they never debit a global XOR account or
/// confer a claim on the parent network. Node-local configuration cannot override this policy.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::block::consensus::PrivateRootFeePolicy")]
#[norito(deny_unknown_fields)]
pub struct PrivateRootFeePolicy {
    /// Canonical asset whose definition is restricted to the private root's dataspace.
    pub asset_definition_id: AssetDefinitionId,
    /// Positive charge applied to every ordinary transaction.
    pub base_fee: Quantity,
    /// Charge for each byte of the signed transaction payload.
    pub per_byte_fee: Quantity,
    /// Charge for each native instruction in the transaction.
    pub per_instruction_fee: Quantity,
    /// Positive multiplier applied to measured execution gas.
    pub per_gas_unit_fee: Quantity,
}

/// Why the signed private-root fee parameter cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PrivateRootFeePolicyError {
    /// Required base or gas charge was zero.
    #[error("private-root base and gas fees must be positive")]
    ZeroRequiredRate,
    /// The supplied custom parameter identifies another policy.
    #[error("wrong private-root fee parameter identity")]
    WrongParameter,
    /// The parameter did not contain the strict typed policy.
    #[error("malformed private-root fee parameter")]
    MalformedParameter,
}

impl PrivateRootFeePolicy {
    /// Canonical identity of the immutable signed-genesis parameter.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        "private_root_fees_v1"
            .parse()
            .expect("static parameter identifier")
    }

    /// Check positive mandatory rates without consulting node-local settings.
    ///
    /// # Errors
    /// Rejects zero base or per-gas charge. Asset scope is checked against committed state.
    pub fn validate(&self) -> Result<(), PrivateRootFeePolicyError> {
        if self.base_fee.is_zero() || self.per_gas_unit_fee.is_zero() {
            return Err(PrivateRootFeePolicyError::ZeroRequiredRate);
        }
        Ok(())
    }

    /// Encode the validated policy as its canonical custom parameter.
    ///
    /// # Errors
    /// Rejects invalid rates or an unrepresentable parameter payload.
    pub fn into_custom_parameter(self) -> Result<CustomParameter, PrivateRootFeePolicyError> {
        self.validate()?;
        // The typed constructor canonicalizes object-key order. Parsing serializer text
        // would require declaration order to happen to match canonical lexical order.
        let payload = iroha_primitives::json::Json::try_new(self)
            .map_err(|_| PrivateRootFeePolicyError::MalformedParameter)?;
        Ok(CustomParameter::new(Self::parameter_id(), payload))
    }

    /// Decode only the exact, strictly validated policy parameter.
    ///
    /// # Errors
    /// Rejects a different identity, malformed payload, unknown fields or invalid rates.
    pub fn from_custom_parameter(
        parameter: &CustomParameter,
    ) -> Result<Self, PrivateRootFeePolicyError> {
        if parameter.id() != &Self::parameter_id() {
            return Err(PrivateRootFeePolicyError::WrongParameter);
        }
        let policy: Self = parameter
            .payload()
            .try_into_any()
            .map_err(|_| PrivateRootFeePolicyError::MalformedParameter)?;
        policy.validate()?;
        Ok(policy)
    }

    /// Read committed parameters, distinguishing an absent policy from a malformed one.
    ///
    /// # Errors
    /// A present policy must decode and validate. Private execution must reject `None`.
    pub fn from_parameters(
        parameters: &Parameters,
    ) -> Result<Option<Self>, PrivateRootFeePolicyError> {
        parameters
            .custom()
            .get(&Self::parameter_id())
            .map(Self::from_custom_parameter)
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> PrivateRootFeePolicy {
        PrivateRootFeePolicy {
            asset_definition_id: AssetDefinitionId::derive_from_components(
                iroha_model_base::domain::DomainId::parse_fully_qualified("app.acme").unwrap(),
                "gas".parse().unwrap(),
            ),
            base_fee: "0.01".parse().unwrap(),
            per_byte_fee: Quantity::zero(),
            per_instruction_fee: "0.002".parse().unwrap(),
            per_gas_unit_fee: "0.000001".parse().unwrap(),
        }
    }

    #[test]
    fn signed_policy_preserves_currency_and_decimal_rates_in_all_codecs() {
        let expected = policy();
        expected.validate().unwrap();
        let bytes = norito::encode_canonical(&expected).unwrap();
        assert_eq!(
            norito::decode_canonical::<PrivateRootFeePolicy>(&bytes).unwrap(),
            expected
        );
        let parameter = expected.clone().into_custom_parameter().unwrap();
        assert_eq!(parameter.id(), &PrivateRootFeePolicy::parameter_id());
        assert_eq!(
            PrivateRootFeePolicy::from_custom_parameter(&parameter).unwrap(),
            expected
        );
        assert_eq!(
            PrivateRootFeePolicy::from_parameters(&Parameters::default()).unwrap(),
            None
        );
        let mut parameters = Parameters::default();
        parameters.set_parameter(crate::parameter::Parameter::Custom(parameter));
        assert_eq!(
            PrivateRootFeePolicy::from_parameters(&parameters).unwrap(),
            Some(expected)
        );
    }

    #[test]
    fn private_policy_rejects_free_execution_and_malformed_parameter() {
        for gas in [false, true] {
            let mut invalid = policy();
            if gas {
                invalid.per_gas_unit_fee = Quantity::zero();
            } else {
                invalid.base_fee = Quantity::zero();
            }
            assert_eq!(
                invalid.validate(),
                Err(PrivateRootFeePolicyError::ZeroRequiredRate)
            );
            assert_eq!(
                invalid.into_custom_parameter(),
                Err(PrivateRootFeePolicyError::ZeroRequiredRate)
            );
        }
        let valid = policy().into_custom_parameter().unwrap();
        let wrong = CustomParameter::new("other".parse().unwrap(), valid.payload().clone());
        assert_eq!(
            PrivateRootFeePolicy::from_custom_parameter(&wrong),
            Err(PrivateRootFeePolicyError::WrongParameter)
        );
        let malformed = CustomParameter::new(
            PrivateRootFeePolicy::parameter_id(),
            "{}".parse::<iroha_primitives::json::Json>().unwrap(),
        );
        assert_eq!(
            PrivateRootFeePolicy::from_custom_parameter(&malformed),
            Err(PrivateRootFeePolicyError::MalformedParameter)
        );
        let mut parameters = Parameters::default();
        parameters.set_parameter(crate::parameter::Parameter::Custom(malformed));
        assert_eq!(
            PrivateRootFeePolicy::from_parameters(&parameters),
            Err(PrivateRootFeePolicyError::MalformedParameter)
        );
        let mut value = norito::json::to_value(&policy()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("node_override".into(), true.into());
        assert!(norito::json::from_value::<PrivateRootFeePolicy>(value).is_err());
    }
}
