//! Direct canonical parameter and projection ingress into one original-pool graph.
//!
//! The enclosing reader retains source/diagnostic custody and authenticates its
//! scope. This module does not create genesis authority or publish a successor.

use iroha_allocation::{AllocationBudget, ChargedBufferError};
use iroha_data_model::{
    block::consensus::{
        AdmittedPublicLaneStakingPolicy, AdmittedRootFeePolicy, MonetaryJsonAdmissionError,
        PublicLaneStakingPolicy, PublicLaneStakingPolicyError, RootFeePolicy,
    },
    crypto_admission_policy::{
        AdmittedCryptoAdmissionPolicy, CryptoAdmissionPolicyV1, CryptoPolicyJsonAdmissionError,
    },
    fraud::{
        AdmittedFraudAdmissionPolicy, FraudAdmissionPolicyError, FraudAdmissionPolicyV1,
        FraudPolicyJsonAdmissionError,
    },
    parameter::{
        CustomParametersAdmissionError, CustomParametersJsonAdmissionError, Parameters,
        system::{
            AdmittedParameters, ConsensusHandshakeValidationError, HandshakeJsonAdmissionError,
            ParametersJsonAdmissionError, SumeragiNposJsonAdmissionError,
        },
    },
    pipeline_validity_policy::{
        PipelinePolicyJsonAdmissionError, PipelineValidityPolicyError, PipelineValidityPolicyV1,
    },
    zk_validity_policy::{
        AdmittedZkValidityPolicy, ZkPolicyJsonAdmissionError, ZkValidityPolicyError,
        ZkValidityPolicyV1,
    },
};
use iroha_primitives::json::{Json, JsonRetentionError};
use norito::json;

use super::{
    ParameterRetentionError, PreparedParameters, RetainedStateParameters,
    gas_admission::{AdmittedGas, GasAdmissionError},
    retained::{DecodedParameterParts, ReservedParameterGraph},
};

/// Original parsing, allocation or policy failure from the complete graph reader.
#[derive(Debug, thiserror::Error)]
pub enum ParameterIngressError {
    /// Exact original decoder cause, including its current observer identity.
    #[error(transparent)]
    Codec(#[from] norito::Error),
    /// Exact physical or finite-pool backing refusal.
    #[error(transparent)]
    Allocation(#[from] ChargedBufferError),
    /// Original shared-shell or retained-map refusal.
    #[error(transparent)]
    Retention(ParameterRetentionError),
    /// Original self-custodial JSON backing refusal.
    #[error(transparent)]
    Json(JsonRetentionError),
    /// An executor declaration used a map key different from its value identity.
    #[error("executor custom parameter key differs from its value identity")]
    ExecutorParameterIdentity,
    /// Intrinsic signed handshake/context validation without intermediate text.
    #[error(transparent)]
    Handshake(#[from] ConsensusHandshakeValidationError),
    /// The original NPoS instruction explicitly zeroes one governed bound.
    #[error("SumeragiNposParameters.reconfig.{0} must be greater than zero")]
    NposZeroField(&'static str),
    /// Intrinsic pipeline validity rule; no policy text is reformatted.
    #[error(transparent)]
    Pipeline(#[from] PipelineValidityPolicyError),
    /// Intrinsic fraud validity rule.
    #[error(transparent)]
    Fraud(#[from] FraudAdmissionPolicyError),
    /// Intrinsic ZK validity rule.
    #[error(transparent)]
    Zk(#[from] ZkValidityPolicyError),
    /// Intrinsic staking validity rule.
    #[error(transparent)]
    Staking(#[from] PublicLaneStakingPolicyError),
}
impl From<json::Error> for ParameterIngressError {
    fn from(error: json::Error) -> Self {
        Self::Codec(error.into_core_error())
    }
}
impl From<ParameterRetentionError> for ParameterIngressError {
    fn from(error: ParameterRetentionError) -> Self {
        match error {
            ParameterRetentionError::Codec(error) => Self::Codec(error),
            ParameterRetentionError::Custom(error) => error.into(),
            error => Self::Retention(error),
        }
    }
}
impl From<JsonRetentionError> for ParameterIngressError {
    fn from(error: JsonRetentionError) -> Self {
        match error {
            JsonRetentionError::Decode(error) => error.into(),
            error => Self::Json(error),
        }
    }
}
impl From<CustomParametersAdmissionError> for ParameterIngressError {
    fn from(error: CustomParametersAdmissionError) -> Self {
        match error {
            CustomParametersAdmissionError::Codec(error) => Self::Codec(error),
            CustomParametersAdmissionError::Json(error) => error.into(),
            error => Self::Retention(ParameterRetentionError::Custom(error)),
        }
    }
}
impl From<CustomParametersJsonAdmissionError> for ParameterIngressError {
    fn from(error: CustomParametersJsonAdmissionError) -> Self {
        match error {
            CustomParametersJsonAdmissionError::Json(error) => error.into(),
            CustomParametersJsonAdmissionError::Admission(error) => {
                ParameterRetentionError::Admission(error).into()
            }
            CustomParametersJsonAdmissionError::Allocation(error) => error.into(),
            CustomParametersJsonAdmissionError::RetainedJson(error) => error.into(),
        }
    }
}
impl From<ParametersJsonAdmissionError> for ParameterIngressError {
    fn from(error: ParametersJsonAdmissionError) -> Self {
        match error {
            ParametersJsonAdmissionError::Json(error) => error.into(),
            ParametersJsonAdmissionError::Allocation(error) => error.into(),
            ParametersJsonAdmissionError::Custom(error) => error.into(),
            ParametersJsonAdmissionError::RetainCustom(error) => error.into(),
        }
    }
}
macro_rules! json_leaf_error {
    ($($error:ident),+ $(,)?) => {$ (
        impl From<$error> for ParameterIngressError {
            fn from(error: $error) -> Self {
                match error {
                    $error::Json(error) => error.into(),
                    $error::Allocation(error) => error.into(),
                }
            }
        }
    )+};
}
json_leaf_error!(
    CryptoPolicyJsonAdmissionError,
    ZkPolicyJsonAdmissionError,
    MonetaryJsonAdmissionError,
    PipelinePolicyJsonAdmissionError,
    SumeragiNposJsonAdmissionError,
    HandshakeJsonAdmissionError
);
impl From<FraudPolicyJsonAdmissionError> for ParameterIngressError {
    fn from(error: FraudPolicyJsonAdmissionError) -> Self {
        match error {
            FraudPolicyJsonAdmissionError::Json(error) => error.into(),
            FraudPolicyJsonAdmissionError::Codec(error) => error.into(),
            FraudPolicyJsonAdmissionError::Allocation(error) => error.into(),
        }
    }
}
impl From<GasAdmissionError> for ParameterIngressError {
    fn from(error: GasAdmissionError) -> Self {
        match error {
            GasAdmissionError::Decode(error) => error.into(),
            GasAdmissionError::Allocation(error) => error.into(),
        }
    }
}

/// The result of observing this actual graph operation in the current decoder.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ParameterIngressAttemptError {
    /// The original codec attempt, never an error replayed after the operation.
    #[error(transparent)]
    Codec(norito::core::DecodeAttemptError),
    /// The original typed pool, physical allocator, or semantic failure.
    #[error(transparent)]
    Original(ParameterIngressError),
}
impl ParameterIngressAttemptError {
    /// Observe before invoking the real canonical/projection reader.
    pub(crate) fn observe<T>(
        operation: impl FnOnce() -> Result<T, ParameterIngressError>,
    ) -> Result<T, Self> {
        match norito::core::classify_decode_attempt(|| match operation() {
            Ok(value) => Ok(Ok(value)),
            Err(ParameterIngressError::Codec(error)) => Err(error),
            Err(error) => Ok(Err(error)),
        }) {
            Ok(result) => result.map_err(Self::Original),
            Err(error) => Err(Self::Codec(error)),
        }
    }
}

impl RetainedStateParameters {
    /// Build and bind a successor before opening any mutable MV access.
    /// The execution owner checks authorization, monetary scope, retained
    /// liabilities and prepared event storage before installing this value.
    pub(crate) fn prepare_parameter(
        &self,
        parameter: &iroha_data_model::parameter::Parameter,
        budget: &AllocationBudget,
    ) -> Result<PreparedParameters, ParameterIngressError> {
        self.prepare_parameter_with_cadence(parameter, None, budget)
    }

    /// Prepare one custom handshake and its authenticated Copy cadence together.
    /// The caller must validate the signed genesis source and immutability first.
    pub(crate) fn prepare_parameter_with_cadence(
        &self,
        parameter: &iroha_data_model::parameter::Parameter,
        cadence: Option<std::num::NonZeroU64>,
        budget: &AllocationBudget,
    ) -> Result<PreparedParameters, ParameterIngressError> {
        self.require_original_pool(budget)?;
        let shell = ReservedParameterGraph::try_reserve(budget)?;
        let canonical =
            AdmittedParameters::try_with_parameter(&self.get().parameters, parameter, budget)?;
        let next = project_with_cadence(shell, canonical, cadence, budget)?;
        self.prepare_admitted(next, budget)
            .map_err(|(_next, error)| error.into())
    }

    /// Prepare one borrowed executor default without an unadmitted ID clone.
    pub(crate) fn prepare_custom_parameter(
        &self,
        parameter: &iroha_data_model::parameter::CustomParameter,
        budget: &AllocationBudget,
    ) -> Result<PreparedParameters, ParameterIngressError> {
        self.require_original_pool(budget)?;
        let shell = ReservedParameterGraph::try_reserve(budget)?;
        let canonical =
            AdmittedParameters::try_with_custom(&self.get().parameters, parameter, budget)?;
        let next = project_with_cadence(shell, canonical, None, budget)?;
        self.prepare_admitted(next, budget)
            .map_err(|(_next, error)| error.into())
    }

    /// Prepare exact custom removal without losing mandatory initialized policy.
    pub(crate) fn prepare_custom_removal(
        &self,
        id: &iroha_data_model::parameter::CustomParameterId,
        budget: &AllocationBudget,
    ) -> Result<PreparedParameters, ParameterIngressError> {
        self.require_original_pool(budget)?;
        let shell = ReservedParameterGraph::try_reserve(budget)?;
        let canonical = AdmittedParameters::try_without_custom(&self.get().parameters, id, budget)?;
        let next = project(shell, canonical, budget)?;
        self.prepare_admitted(next, budget)
            .map_err(|(_next, error)| error.into())
    }

    /// Prepare a full consensus-family replacement with exact array custody.
    pub(crate) fn prepare_sumeragi(
        &self,
        family: &iroha_data_model::parameter::SumeragiParameters,
        budget: &AllocationBudget,
    ) -> Result<PreparedParameters, ParameterIngressError> {
        self.require_original_pool(budget)?;
        let shell = ReservedParameterGraph::try_reserve(budget)?;
        let canonical =
            AdmittedParameters::try_with_sumeragi(&self.get().parameters, family, budget)?;
        let next = project(shell, canonical, budget)?;
        self.prepare_admitted(next, budget)
            .map_err(|(_next, error)| error.into())
    }

    fn require_original_pool(
        &self,
        budget: &AllocationBudget,
    ) -> Result<(), ParameterIngressError> {
        if self.belongs_to(budget) {
            Ok(())
        } else {
            Err(ParameterRetentionError::ForeignPool.into())
        }
    }
    /// Read the canonical source directly into this original pool and one graph.
    /// The caller preserves its parser/context and checks enclosing completion.
    pub(crate) fn try_decode_json(
        parser: &mut json::Parser<'_>,
        budget: &AllocationBudget,
    ) -> Result<Self, ParameterIngressError> {
        let shell = ReservedParameterGraph::try_reserve(budget)?;
        let canonical = AdmittedParameters::try_decode_json(parser, budget)?;
        project(shell, canonical, budget)
    }

    /// Fund empty pre-genesis State directly without an ordinary default graph.
    /// Mandatory monetary/crypto policies remain absent until explicit genesis.
    pub(crate) fn try_protocol_default(
        budget: &AllocationBudget,
    ) -> Result<Self, ParameterIngressError> {
        let shell = ReservedParameterGraph::try_reserve(budget)?;
        let canonical = AdmittedParameters::try_protocol_default(budget)?;
        project(shell, canonical, budget)
    }
}

fn payload<'a>(parameters: &'a Parameters, id: &str) -> Option<&'a Json> {
    parameters
        .custom()
        .values()
        .find(|value| value.id().name().as_ref() == id)
        .map(|value| value.payload())
}
fn document<T, E: Into<ParameterIngressError>>(
    source: &Json,
    decode: impl FnOnce(&mut json::Parser<'_>) -> Result<T, E>,
) -> Result<T, ParameterIngressError> {
    let mut parser = json::Parser::new(source.get());
    parser.preflight_document()?;
    parser.skip_ws();
    let value = decode(&mut parser).map_err(Into::into)?;
    parser.finish_document()?;
    Ok(value)
}

// Canonical source remains owned throughout projection decoding. The order is
// the original StateParameters constructor's order, including validation between
// families. Defaults exist only for the optional protocol-default families.
fn project(
    shell: ReservedParameterGraph,
    canonical: AdmittedParameters,
    budget: &AllocationBudget,
) -> Result<RetainedStateParameters, ParameterIngressError> {
    project_with_cadence(shell, canonical, None, budget)
}
fn project_with_cadence(
    shell: ReservedParameterGraph,
    canonical: AdmittedParameters,
    cadence: Option<std::num::NonZeroU64>,
    budget: &AllocationBudget,
) -> Result<RetainedStateParameters, ParameterIngressError> {
    let parameters = canonical.get();
    let gas = AdmittedGas::try_from_parameters(parameters, budget)?;
    let pipeline = match payload(parameters, PipelineValidityPolicyV1::PARAMETER_ID_STR) {
        Some(source) => document(source, |p| {
            PipelineValidityPolicyV1::try_decode_json(p, budget)
        })?,
        None => PipelineValidityPolicyV1::protocol_default(),
    };
    pipeline.validate()?;
    let fraud = match payload(parameters, FraudAdmissionPolicyV1::PARAMETER_ID_STR) {
        Some(source) => document(source, |p| {
            AdmittedFraudAdmissionPolicy::try_decode_json(p, budget)
        })?,
        None => AdmittedFraudAdmissionPolicy::try_protocol_default(budget)?,
    };
    fraud.get().validate()?;
    let zk = match payload(parameters, ZkValidityPolicyV1::PARAMETER_ID_STR) {
        Some(source) => document(source, |p| {
            AdmittedZkValidityPolicy::try_decode_json(p, budget)
        })?,
        None => AdmittedZkValidityPolicy::try_protocol_default(budget)?,
    };
    zk.get().validate()?;
    let root_fees = payload(parameters, RootFeePolicy::PARAMETER_ID_STR)
        .map(|source| {
            document(source, |p| {
                AdmittedRootFeePolicy::try_decode_json(p, budget)
            })
        })
        .transpose()?;
    let crypto = payload(parameters, CryptoAdmissionPolicyV1::PARAMETER_ID_STR)
        .map(|source| {
            document(source, |p| {
                AdmittedCryptoAdmissionPolicy::try_decode_json(p, budget)
            })
        })
        .transpose()?;
    let staking = payload(parameters, PublicLaneStakingPolicy::PARAMETER_ID_STR)
        .map(|source| {
            document(source, |p| {
                AdmittedPublicLaneStakingPolicy::try_decode_json(p, budget)
            })
        })
        .transpose()?;
    if let Some(staking) = &staking {
        staking.get().validate()?;
    }
    let parts = DecodedParameterParts {
        parameters: canonical,
        gas,
        pipeline,
        fraud,
        zk,
        root_fees,
        crypto,
        staking,
    };
    shell
        .finish_with_cadence(parts, cadence, budget)
        .map_err(|(_shell, _parts, error)| error.into())
}

#[cfg(test)]
mod tests;
