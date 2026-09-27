//! Signed committee preparation authority across opaque execution boundaries.

use super::*;
use iroha_executor_data_model::isi::multisig::MultisigApprove;

pub(super) fn reject_opaque_committee_operation(
    instruction: &InstructionBox,
    instruction_index: usize,
) -> Result<(), ValidationFeeAdmissionError> {
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
        return Err(ValidationFeeAdmissionError::OpaqueDeferredCommitteeOperation {
            instruction_index,
        });
    }
    if let Some(instruction_wire_id) = staking_effects::monetary_staking_wire_id(instruction) {
        return Err(
            ValidationFeeAdmissionError::OpaqueDeferredStakingOperation {
                instruction_index,
                instruction_wire_id,
            },
        );
    }
    Ok(())
}

pub(super) fn reject_opaque_committee_operations_with<F>(
    instructions: &[InstructionBox],
    visited: &mut std::collections::BTreeSet<String>,
    depth: usize,
    resolve: &mut F,
) -> Result<(), ValidationFeeAdmissionError>
where
    F: FnMut(&MultisigApprove) -> Option<(AccountId, Vec<InstructionBox>)>,
{
    if depth > MAX_OPAQUE_DEFERRED_PROPOSAL_DEPTH {
        return Err(ValidationFeeAdmissionError::OpaqueDeferredProposalDepthExceeded);
    }
    for (index, instruction) in instructions.iter().enumerate() {
        reject_opaque_committee_operation(instruction, index)?;
        if let Ok(multisig) = MultisigInstructionBox::try_from(instruction) {
            match multisig {
                MultisigInstructionBox::Propose(proposal) => {
                    reject_opaque_committee_operations_with(
                        &proposal.instructions,
                        visited,
                        depth + 1,
                        resolve,
                    )?;
                }
                MultisigInstructionBox::Approve(approval) => {
                    let Some((authority, instructions)) = resolve(&approval) else {
                        return Err(
                            ValidationFeeAdmissionError::UnresolvedOpaqueDeferredMultisigApproval {
                                account_id: approval.account.to_string(),
                                instructions_hash_hex: hex::encode(
                                    approval.instructions_hash.as_ref(),
                                ),
                            },
                        );
                    };
                    let identity = format!(
                        "{}:{}",
                        authority,
                        hex::encode(approval.instructions_hash.as_ref())
                    );
                    if visited.insert(identity) {
                        reject_opaque_committee_operations_with(
                            &instructions,
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
                reject_opaque_committee_operations_with(nested, visited, depth + 1, resolve)?;
            }
            Executable::IvmProved(proved) => {
                reject_opaque_committee_operations_with(
                    &proved.overlay,
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
