//! Allocation-free assembly of directly admitted canonical and projection leaves.
//!
//! The caller reserves this shell before parsing and supplies leaves decoded from
//! that same canonical source. No raw graph clone, Value tree, text serialization,
//! second parameter cell, or replacement pool is used by this transition.

use super::{
    ParameterGraph, ParameterGraphCustody, ParameterRetentionError, RetainedStateParameters,
    StateParameters,
};
use crate::pipeline_gas_policy::gas_admission::{AdmittedGas, GasCustody};
use iroha_allocation::{
    AllocationBudget, AllocationCharge, ChargedBuffer, ChargedShared, ReservedChargedShared,
};
use iroha_data_model::{
    block::consensus::{
        AdmittedPublicLaneStakingPolicy, AdmittedRootFeePolicy, PublicLaneStakingPolicyCustody,
        RootFeePolicyCustody,
    },
    crypto_admission_policy::AdmittedCryptoAdmissionPolicy,
    fraud::AdmittedFraudAdmissionPolicy,
    parameter::system::{AdmittedParameters, ParametersCustody},
    pipeline_validity_policy::PipelineValidityPolicyV1,
    zk_validity_policy::AdmittedZkValidityPolicy,
};

/// The sole destination for the parser's already admitted canonical graph and
/// the projections decoded from its exact original JSON leaves.
///
/// This private input never grants scope/genesis authority. The reader must
/// validate policy semantics, scope and original source before assembly.
pub(in crate::pipeline_gas_policy) struct DecodedParameterParts {
    pub(in crate::pipeline_gas_policy) parameters: AdmittedParameters,
    pub(in crate::pipeline_gas_policy) gas: AdmittedGas,
    pub(in crate::pipeline_gas_policy) pipeline: PipelineValidityPolicyV1,
    pub(in crate::pipeline_gas_policy) fraud: AdmittedFraudAdmissionPolicy,
    pub(in crate::pipeline_gas_policy) zk: AdmittedZkValidityPolicy,
    pub(in crate::pipeline_gas_policy) root_fees: Option<AdmittedRootFeePolicy>,
    pub(in crate::pipeline_gas_policy) crypto: Option<AdmittedCryptoAdmissionPolicy>,
    pub(in crate::pipeline_gas_policy) staking: Option<AdmittedPublicLaneStakingPolicy>,
}
impl DecodedParameterParts {
    fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.parameters.belongs_to(budget)
            && self.gas.belongs_to(budget)
            && self.fraud.belongs_to(budget)
            && self.zk.belongs_to(budget)
            && self
                .root_fees
                .as_ref()
                .is_none_or(|value| value.belongs_to(budget))
            && self
                .crypto
                .as_ref()
                .is_none_or(|value| value.belongs_to(budget))
            && self
                .staking
                .as_ref()
                .is_none_or(|value| value.belongs_to(budget))
    }
}

// Nested opaque custody stays inline. Flattening it would allocate another
// ledger or lose the exact Account/Quantity/Custom owner identity.
pub(super) struct DecodedCustody {
    parameters: ParametersCustody,
    gas: GasCustody,
    fraud: ChargedBuffer<AllocationCharge>,
    zk: ChargedBuffer<AllocationCharge>,
    root_fees: Option<RootFeePolicyCustody>,
    crypto: Option<AllocationCharge>,
    staking: Option<PublicLaneStakingPolicyCustody>,
}
impl DecodedCustody {
    pub(super) fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        let ledger_matches = |ledger: &ChargedBuffer<AllocationCharge>| {
            ledger.belongs_to(budget)
                && ledger
                    .as_slice()
                    .iter()
                    .all(|charge| charge.belongs_to(budget))
        };
        self.parameters.belongs_to(budget)
            && self.gas.belongs_to(budget)
            && ledger_matches(&self.fraud)
            && ledger_matches(&self.zk)
            && self
                .root_fees
                .as_ref()
                .is_none_or(|value| value.belongs_to(budget))
            && self
                .crypto
                .as_ref()
                .is_none_or(|charge| charge.belongs_to(budget))
            && self
                .staking
                .as_ref()
                .is_none_or(|custody| custody.belongs_to(budget))
    }
}

/// The concrete shared graph shell is physically funded before leaf decoding.
pub(in crate::pipeline_gas_policy) struct ReservedParameterGraph(
    ReservedChargedShared<ParameterGraph>,
);
impl ReservedParameterGraph {
    pub(in crate::pipeline_gas_policy) fn try_reserve(
        budget: &AllocationBudget,
    ) -> Result<Self, ParameterRetentionError> {
        let mut original = budget
            .try_reserve_bytes(ChargedShared::<ParameterGraph>::allocation_layout().size())?;
        Ok(Self(ChargedShared::reserve_from(&mut original)?))
    }

    /// Move exact values and ledgers after checking every original pool.
    /// Refusal returns every original leaf and the unused shell intact.
    pub(in crate::pipeline_gas_policy) fn finish(
        self,
        parts: DecodedParameterParts,
        budget: &AllocationBudget,
    ) -> Result<RetainedStateParameters, (Self, DecodedParameterParts, ParameterRetentionError)>
    {
        self.finish_with_cadence(parts, None, budget)
    }

    /// Apply only the authenticated handshake's scalar cadence while closing the
    /// same immutable graph. This Copy field cannot change allocation custody.
    pub(in crate::pipeline_gas_policy) fn finish_with_cadence(
        self,
        parts: DecodedParameterParts,
        cadence: Option<std::num::NonZeroU64>,
        budget: &AllocationBudget,
    ) -> Result<RetainedStateParameters, (Self, DecodedParameterParts, ParameterRetentionError)>
    {
        if !self.0.belongs_to(budget) || !parts.belongs_to(budget) {
            return Err((self, parts, ParameterRetentionError::ForeignPool));
        }
        // SAFETY: each input owns these exact immutable allocations. After the
        // first extraction there are no calls that allocate, validate, fail or
        // invoke user code. Pure moves immediately put the complete value before
        // all opaque custody in the single closed ParameterGraph. No raw leaves
        // escape and no Clone is used. Initialization consumes the prepaid shell.
        #[allow(unsafe_code)]
        let graph = unsafe {
            let (mut parameters, parameter_custody) = parts.parameters.into_allocation_parts();
            if let Some(cadence) = cadence {
                parameters.sumeragi.block_cadence_ms = cadence;
            }
            let (gas, gas_custody) = parts.gas.into_allocation_parts();
            let (fraud, fraud_custody) = parts.fraud.into_allocation_parts();
            let (zk, zk_custody) = parts.zk.into_allocation_parts();
            let (root_fees, root_fee_custody) = match parts.root_fees {
                Some(value) => {
                    let (value, custody) = value.into_allocation_parts();
                    (Some(value), Some(custody))
                }
                None => (None, None),
            };
            let (crypto, crypto_custody) = match parts.crypto {
                Some(value) => {
                    let (value, custody) = value.into_allocation_parts();
                    (Some(value), Some(custody))
                }
                None => (None, None),
            };
            let (staking, staking_custody) = match parts.staking {
                Some(value) => {
                    let (value, custody) = value.into_allocation_parts();
                    (Some(value), Some(custody))
                }
                None => (None, None),
            };
            ParameterGraph {
                value: StateParameters {
                    parameters,
                    gas,
                    pipeline: parts.pipeline,
                    fraud,
                    zk,
                    root_fees,
                    crypto,
                    staking,
                },
                custody: ParameterGraphCustody::Decoded(DecodedCustody {
                    parameters: parameter_custody,
                    gas: gas_custody,
                    fraud: fraud_custody,
                    zk: zk_custody,
                    root_fees: root_fee_custody,
                    crypto: crypto_custody,
                    staking: staking_custody,
                }),
            }
        };
        Ok(RetainedStateParameters(self.0.initialize(graph)))
    }
}

#[cfg(test)]
mod tests;
