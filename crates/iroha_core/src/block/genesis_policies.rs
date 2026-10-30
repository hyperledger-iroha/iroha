//! Atomic installation of both original signed-genesis policy capabilities.
//!
//! Validate the pristine State once for the complete bootstrap operation. Each
//! source-specific validator runs before either projection changes. Decode into
//! a disposable candidate, then move both capabilities and the candidate together.
//! Every unpublished parameter graph is funded from the original World pool.
//! TODO(original-source-admission): original signed-input retention and capability-selection backing remain with
//! the authenticated source owner; they are not retroactively admitted here.

use iroha_data_model::{
    block::{SignedBlock, consensus::PublicLaneStakingPolicy},
    crypto_admission_policy::CryptoAdmissionPolicyV1,
    isi::SetParameter,
    parameter::Parameter,
    transaction::Executable,
};

use super::{
    AuthenticatedGenesisOutputSource, BlockValidationError, InvalidGenesisError,
    genesis_crypto_policy::GenesisCryptoPolicy, genesis_staking_policy::GenesisStakingPolicy,
};
use crate::state::{State, StateBlock};

/// Both policies selected from the same original authenticated signed source.
#[derive(Debug)]
pub(crate) struct GenesisPolicies {
    crypto: GenesisCryptoPolicy,
    staking: GenesisStakingPolicy,
}

impl AuthenticatedGenesisOutputSource {
    /// Select both mandatory policies without giving an ordinary caller bootstrap authority.
    ///
    /// `None` is a configured pre-policy genesis: one that signs neither policy, on a node that
    /// installed that network's legacy policies. Such a node refuses a signed policy.
    pub(crate) fn initial_policies(
        &self,
        block: &SignedBlock,
    ) -> Result<Option<GenesisPolicies>, BlockValidationError> {
        if crate::pipeline_gas_policy::legacy_genesis_policies().is_some() {
            self.account_for(block).map_err(|error| {
                BlockValidationError::from(InvalidGenesisError::CryptoPolicy(error.into()))
            })?;
            if signs_policy(block) {
                return Err(InvalidGenesisError::CryptoPolicy(
                    "configured legacy genesis policies cannot accompany a genesis that signs \
                     its own policy"
                        .into(),
                )
                .into());
            }
            return Ok(None);
        }
        Ok(Some(GenesisPolicies {
            crypto: self.initial_crypto_policy(block)?,
            staking: self.initial_staking_policy(block)?,
        }))
    }
}

/// Whether any original instruction assigns the crypto or staking policy parameter.
fn signs_policy(block: &SignedBlock) -> bool {
    let crypto = CryptoAdmissionPolicyV1::parameter_id();
    let staking = PublicLaneStakingPolicy::parameter_id();
    block.external_transactions().any(|transaction| {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return false;
        };
        instructions.iter().any(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<SetParameter>()
                .is_some_and(|set| {
                    matches!(set.inner(), Parameter::Custom(custom)
                        if custom.id() == &crypto || custom.id() == &staking)
                })
        })
    })
}

impl GenesisPolicies {
    /// Install both projections at one verified original pristine State cut.
    /// Any source or decoder refusal leaves State and both source capabilities intact.
    pub(crate) fn install(
        original: &mut Option<Self>,
        block: &SignedBlock,
        overlay: &mut StateBlock<'_>,
        state: &State,
        generation: u64,
    ) -> Result<(), BlockValidationError> {
        let invalid = |message: &str| {
            BlockValidationError::from(InvalidGenesisError::StakingPolicy(message.into()))
        };
        if overlay.genesis_crypto_policy.is_some() || overlay.genesis_staking_policy.is_some() {
            return Err(invalid(
                "genesis overlay already retains an initial policy capability",
            ));
        }
        let selected = original
            .as_ref()
            .ok_or_else(|| invalid("combined initial policy capability was already consumed"))?;
        // Both checks still require an empty execution write-set. Neither is
        // weakened to permit the writes of a previously installed policy.
        selected
            .crypto
            .validate_installation(block, overlay, state, generation)?;
        selected
            .staking
            .validate_installation(block, overlay, state, generation)?;
        let predecessor = overlay.world.parameters.get().clone();
        let budget = overlay.world.sccp_committee_generations.allocation_budget();
        let crypto = selected
            .crypto
            .initialize_candidate(&predecessor, block, budget)?;
        let candidate = selected
            .staking
            .initialize_candidate(&crypto, block, budget)?;
        selected
            .crypto
            .policy_for_parameters(&block.header(), &candidate)
            .map_err(|_| invalid("combined candidate lost its original crypto policy"))?;
        selected
            .staking
            .policy_for_parameters(&block.header(), &candidate)
            .map_err(|_| invalid("combined candidate lost its original staking policy"))?;
        if !predecessor.same_version(overlay.world.parameters.get()) {
            return Err(BlockValidationError::ExecutionDeferred(
                ivm::error::ExecutionDeferral::LocalInvariantViolation.into(),
            ));
        }
        // No fallible decoder or source check follows the first overlay write.
        // Both retained source owners move with their exact validated projections.
        *overlay.world.parameters.get_mut() = candidate;
        let selected = original
            .take()
            .expect("checked original combined capability");
        overlay.genesis_crypto_policy = Some(selected.crypto);
        overlay.genesis_staking_policy = Some(selected.staking);
        Ok(())
    }
}
