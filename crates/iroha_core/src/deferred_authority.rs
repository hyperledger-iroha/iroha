//! Signed committee and monetary staking authority across opaque execution boundaries.
//!
//! Validator committee preparation commands and monetary staking plans must be signed
//! instructions. An opaque deferred executable (a trigger body, an IVM-proved overlay or a
//! resolved multisig approval) may not derive them. This authority check is independent of
//! native fee accounting and runs before any deferred group executes.

use crate::{
    execution_attempt::ExecutionAttemptError as Attempt,
    smartcontracts::isi::multisig::{
        live_proposal_instructions_for_approval, multisig_instruction_decode_attempt,
    },
    state::StateTransaction,
    tx::TransactionRejectionReason,
};
use core::fmt;
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    isi::{InstructionBox, RegisterBox, SetParameter},
    parameter::Parameter,
    transaction::{Executable, ExecutableBatchItem},
};
use iroha_executor_data_model::isi::multisig::{MultisigApprove, MultisigInstructionBox};

/// Maximum nested proposal/trigger depth traversed for one deferred group.
const MAX_OPAQUE_DEFERRED_PROPOSAL_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
enum OpaqueDeferredAuthorityError {
    CommitteeOperation {
        instruction_index: usize,
    },
    StakingOperation {
        instruction_index: usize,
        instruction_wire_id: &'static str,
    },
    UnresolvedMultisigApproval {
        account_id: String,
        instructions_hash_hex: String,
    },
    ProposalDepthExceeded,
    ProposalReadFailed(String),
}

impl fmt::Display for OpaqueDeferredAuthorityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommitteeOperation { instruction_index } => write!(
                f,
                "opaque deferred executable derived a validator committee operation at instruction {instruction_index}; the complete preparation command must be a signed instruction"
            ),
            Self::StakingOperation {
                instruction_index,
                instruction_wire_id,
            } => write!(
                f,
                "opaque deferred executable derived monetary staking operation `{instruction_wire_id}` at instruction {instruction_index}; the exact monetary plan must be a signed instruction"
            ),
            Self::UnresolvedMultisigApproval {
                account_id,
                instructions_hash_hex,
            } => write!(
                f,
                "opaque deferred executable contains a multisig approval that cannot be resolved before execution for account {account_id} and instructions hash {instructions_hash_hex}"
            ),
            Self::ProposalDepthExceeded => write!(
                f,
                "opaque deferred proposal graph exceeds the maximum traversal depth"
            ),
            Self::ProposalReadFailed(reason) => {
                write!(f, "live multisig proposal read failed: {reason}")
            }
        }
    }
}

/// Reject committee preparation and monetary staking derived by opaque deferred groups.
///
/// Deferred multisig approvals are resolved against this execution overlay, so a live
/// proposal cannot smuggle a command that its signers never reviewed as an instruction.
///
/// # Errors
/// Any nested committee or monetary staking operation, an unresolved approval, or an
/// excessively deep proposal graph.
pub(crate) fn reject_opaque_deferred_authority(
    instruction_groups: &std::collections::BTreeMap<AccountId, Vec<InstructionBox>>,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Attempt<TransactionRejectionReason>> {
    reject_opaque_instruction_authority(
        instruction_groups
            .values()
            .flat_map(|instructions| instructions.iter()),
        state_transaction,
    )
    .map_err(|error| error.map_rejection(TransactionRejectionReason::Validation))
}

/// Validate the actual effects produced by an opaque host or verified replay.
///
/// Borrow the original ordered effects so every consumption boundary applies the
/// same recursive signed-plan rule before executing its first instruction.
///
/// # Errors
/// Rejects nested committee or monetary staking instructions, unresolved live
/// multisig approvals, and proposal graphs exceeding the traversal bound.
pub(crate) fn reject_opaque_instruction_authority<'a>(
    instructions: impl IntoIterator<Item = &'a InstructionBox>,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Attempt<ValidationFail>> {
    if cfg!(all(test, sumeragi_core_mutation = "HC66")) {
        return Ok(());
    }
    let mut visited = std::collections::BTreeSet::new();
    reject_opaque_committee_operations_with(instructions, &mut visited, 0, &mut |approve| {
        live_proposal_instructions_for_approval(state_transaction, approve)
    })
    .map_err(|error| {
        error.map_rejection(|error| {
            ValidationFail::NotPermitted(format!(
                "deferred execution authority rejected transaction: {error}"
            ))
        })
    })
}

/// Monetary staking instructions whose exact plan must be signed.
fn monetary_staking_wire_id(instruction: &InstructionBox) -> Option<&'static str> {
    use iroha_data_model::isi::staking::{
        BondPublicLaneStake, ClaimPublicLaneRewards, FinalizePublicLaneUnbond,
        RecordPublicLaneRewards, RegisterPublicLaneCandidate, RegisterPublicLaneValidator,
        SlashPublicLaneValidator,
    };
    macro_rules! classify {
        ($($ty:ty),+ $(,)?) => {$(
            if instruction.as_any().downcast_ref::<$ty>().is_some() {
                return iroha_data_model::isi::instruction_wire_id(instruction);
            }
        )+};
    }
    classify!(
        RegisterPublicLaneCandidate,
        RegisterPublicLaneValidator,
        BondPublicLaneStake,
        FinalizePublicLaneUnbond,
        SlashPublicLaneValidator,
        RecordPublicLaneRewards,
        ClaimPublicLaneRewards,
    );
    None
}

fn reject_opaque_committee_operation(
    instruction: &InstructionBox,
    instruction_index: usize,
) -> Result<(), OpaqueDeferredAuthorityError> {
    if instruction
        .as_any()
        .downcast_ref::<SetParameter>()
        .is_some_and(|set| {
            matches!(set.inner(), Parameter::Custom(custom)
                if custom.id() == &iroha_data_model::nexus::ValidatorCommitteeOperationV1::parameter_id())
        })
    {
        // Match the reserved identifier before decoding: malformed commands may
        // not evade the signed-envelope requirement by failing deserialization.
        return Err(OpaqueDeferredAuthorityError::CommitteeOperation { instruction_index });
    }
    if let Some(instruction_wire_id) = monetary_staking_wire_id(instruction) {
        return Err(OpaqueDeferredAuthorityError::StakingOperation {
            instruction_index,
            instruction_wire_id,
        });
    }
    Ok(())
}

fn reject_opaque_committee_operations_with<'a, F>(
    instructions: impl IntoIterator<Item = &'a InstructionBox>,
    visited: &mut std::collections::BTreeSet<String>,
    depth: usize,
    resolve: &mut F,
) -> Result<(), Attempt<OpaqueDeferredAuthorityError>>
where
    F: FnMut(
        &MultisigApprove,
    ) -> Result<Option<(AccountId, Vec<InstructionBox>)>, Attempt<ValidationFail>>,
{
    if depth > MAX_OPAQUE_DEFERRED_PROPOSAL_DEPTH {
        return Err(OpaqueDeferredAuthorityError::ProposalDepthExceeded.into());
    }
    for (index, instruction) in instructions.into_iter().enumerate() {
        reject_opaque_committee_operation(instruction, index)?;
        let multisig = match MultisigInstructionBox::try_from(instruction) {
            Ok(multisig) => Some(multisig),
            Err(error) => match multisig_instruction_decode_attempt(error, |_| ()) {
                Attempt::Deferred(reason) => return Err(Attempt::Deferred(reason)),
                Attempt::Rejected(()) => None,
            },
        };
        if let Some(multisig) = multisig {
            match multisig {
                MultisigInstructionBox::Propose(proposal) => {
                    reject_opaque_committee_operations_with(
                        proposal.instructions.iter(),
                        visited,
                        depth + 1,
                        resolve,
                    )?;
                }
                MultisigInstructionBox::Approve(approval) => {
                    let Some((authority, instructions)) = resolve(&approval).map_err(|error| {
                        error.map_rejection(|error| {
                            OpaqueDeferredAuthorityError::ProposalReadFailed(error.to_string())
                        })
                    })?
                    else {
                        return Err(OpaqueDeferredAuthorityError::UnresolvedMultisigApproval {
                            account_id: approval.account.to_string(),
                            instructions_hash_hex: hex::encode(approval.instructions_hash.as_ref()),
                        }
                        .into());
                    };
                    let identity = format!(
                        "{}:{}",
                        authority,
                        hex::encode(approval.instructions_hash.as_ref())
                    );
                    if visited.insert(identity) {
                        reject_opaque_committee_operations_with(
                            instructions.iter(),
                            visited,
                            depth + 1,
                            resolve,
                        )?;
                    }
                }
                MultisigInstructionBox::Register(_)
                | MultisigInstructionBox::Cancel(_)
                | MultisigInstructionBox::InvalidateOutstanding(_) => {}
            }
        }
        let Some(RegisterBox::Trigger(register)) =
            instruction.as_any().downcast_ref::<RegisterBox>()
        else {
            continue;
        };
        match register.object.action().executable() {
            Executable::Instructions(nested) => {
                reject_opaque_committee_operations_with(
                    nested.iter(),
                    visited,
                    depth + 1,
                    resolve,
                )?;
            }
            Executable::IvmProved(proved) => {
                reject_opaque_committee_operations_with(
                    proved.overlay.iter(),
                    visited,
                    depth + 1,
                    resolve,
                )?;
            }
            Executable::Batch(items) => {
                for item in items {
                    if let ExecutableBatchItem::Instruction(instruction) = item {
                        reject_opaque_committee_operations_with(
                            std::slice::from_ref(instruction),
                            visited,
                            depth + 1,
                            resolve,
                        )?;
                    }
                }
            }
            Executable::ContractCall(_) | Executable::Ivm(_) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::isi::staking::{
        BondPublicLaneStake, ClaimPublicLaneRewards, FinalizePublicLaneUnbond,
        RecordPublicLaneRewards, RegisterPublicLaneCandidate, RegisterPublicLaneValidator,
        SlashPublicLaneValidator,
    };
    use iroha_primitives::numeric::Quantity;

    #[test]
    fn staking_wire_ids_match_the_canonical_registry() {
        let registry = iroha_data_model::instruction_registry::default();
        macro_rules! check {
            ($($ty:ty => $wire_id:literal),+ $(,)?) => {$(
                assert_eq!(registry.wire_id(core::any::type_name::<$ty>()), Some($wire_id));
            )+};
        }
        check!(
            RegisterPublicLaneCandidate => "iroha.staking.register_public_lane_candidate",
            RegisterPublicLaneValidator => "iroha.instruction.v1::staking::RegisterPublicLaneValidator",
            BondPublicLaneStake => "iroha.instruction.v1::staking::BondPublicLaneStake",
            FinalizePublicLaneUnbond => "iroha.instruction.v1::staking::FinalizePublicLaneUnbond",
            SlashPublicLaneValidator => "iroha.instruction.v1::staking::SlashPublicLaneValidator",
            RecordPublicLaneRewards => "iroha.instruction.v1::staking::RecordPublicLaneRewards",
            ClaimPublicLaneRewards => "iroha.instruction.v1::staking::ClaimPublicLaneRewards",
        );
    }

    #[test]
    fn opaque_reward_recording_requires_a_signed_instruction() {
        let reward_asset = iroha_data_model::asset::AssetId::of(
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                iroha_model_base::domain::DomainId::try_new("wonderland", "universal")
                    .expect("test domain"),
                "xor".parse().expect("test asset name"),
            ),
            iroha_test_samples::ALICE_ID.clone(),
        );
        let instruction: InstructionBox = RecordPublicLaneRewards {
            lane_id: iroha_model_base::topology::LaneId::SINGLE,
            epoch: 0,
            reward_asset,
            total_reward: Quantity::zero(),
            shares: Vec::new(),
            metadata: iroha_model_base::metadata::Metadata::default(),
        }
        .into();
        assert_eq!(
            reject_opaque_committee_operations_with(
                &[instruction],
                &mut std::collections::BTreeSet::new(),
                0,
                &mut |_| Ok(None),
            ),
            Err(OpaqueDeferredAuthorityError::StakingOperation {
                instruction_index: 0,
                instruction_wire_id: "iroha.instruction.v1::staking::RecordPublicLaneRewards",
            }
            .into())
        );
    }
}
