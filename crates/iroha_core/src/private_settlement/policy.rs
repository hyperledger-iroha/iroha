//! Strict committed private-settlement extension reads, independent of node configuration.

use crate::execution_attempt::{ExecutionAttemptError, canonical_decode_attempt_error};
use iroha_data_model::{
    parameter::Parameters,
    privacy::{
        PrivacyProtocolPolicyV1, PrivateSettlementPolicyParameterErrorV1, PrivateSettlementPolicyV1,
    },
};

/// Preserve the codec attempt before decorating a deterministic policy failure.
pub fn attempt(error: PrivateSettlementPolicyParameterErrorV1) -> ExecutionAttemptError<String> {
    match error {
        PrivateSettlementPolicyParameterErrorV1::Codec(original) => {
            canonical_decode_attempt_error(original, |error| error.to_string())
        }
        error => ExecutionAttemptError::Rejected(error.to_string()),
    }
}

/// Read the immutable baseline only when the exact committed extension is absent.
pub fn read(
    parameters: &Parameters,
    protocol: Option<&PrivacyProtocolPolicyV1>,
    committed_height: u64,
) -> Result<PrivateSettlementPolicyV1, ExecutionAttemptError<String>> {
    if let Some(protocol) = protocol {
        if protocol.protocol_id
            != iroha_data_model::privacy::PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1
        {
            return Err("private-settlement protocol identity differs".into());
        }
        protocol
            .validate_at_committed_height(committed_height)
            .map_err(|error| ExecutionAttemptError::Rejected(error.to_string()))?;
    }
    let policy = PrivateSettlementPolicyV1::from_parameters(parameters).map_err(attempt)?;
    policy
        .validate_at_committed_height(committed_height)
        .map_err(|error| ExecutionAttemptError::Rejected(error.to_string()))?;
    if parameters
        .custom()
        .contains_key(PrivateSettlementPolicyV1::parameter_id())
    {
        policy.validate_protocol_policy(protocol).map_err(attempt)?;
    }
    Ok(policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::parameter::{CustomParameter, Parameter};
    use iroha_primitives::json::Json;

    #[test]
    fn absent_baseline_is_read_only_and_malformed_committed_extension_is_not_defaulted() {
        let mut parameters = Parameters::default();
        let original = norito::encode_canonical(&parameters).unwrap();
        assert_eq!(
            read(&parameters, None, 0).unwrap(),
            PrivateSettlementPolicyV1::protocol_default()
        );
        assert_eq!(norito::encode_canonical(&parameters).unwrap(), original);
        parameters.set_parameter(Parameter::Custom(CustomParameter::new(
            PrivateSettlementPolicyV1::parameter_id().clone(),
            Json::from_str_norito("{}").unwrap(),
        )));
        assert!(matches!(
            read(&parameters, None, 0),
            Err(ExecutionAttemptError::Rejected(_))
        ));
    }

    #[test]
    fn committed_extension_decoder_refusal_retains_the_original_owner() {
        let mut parameters = Parameters::default();
        parameters.set_parameter(Parameter::Custom(
            PrivateSettlementPolicyV1::protocol_default()
                .to_custom_parameter()
                .unwrap(),
        ));
        let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
        let error =
            norito::with_decode_limits_scope(limits, || read(&parameters, None, 0)).unwrap_err();
        let ExecutionAttemptError::Deferred(original) = error else {
            panic!("local read must defer")
        };
        assert!(original.decode_refusal().is_some());
        assert!(read(&parameters, None, 0).is_ok());
    }
}
