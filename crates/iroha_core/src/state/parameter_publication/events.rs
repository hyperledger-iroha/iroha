//! Closed original-pool ParameterChanged backing retained by every event reader.

use crate::pipeline_gas_policy::{ParameterIngressError, ParameterRetentionError};
use iroha_allocation::{
    AllocationBudget, AllocationCharge, AllocationRefusal, ChargedBuffer, ChargedShared,
    RetainedPayload,
};
use iroha_data_model::{
    events::{
        SharedDataEvent,
        data::{
            DataEvent,
            prelude::{ConfigurationEvent, ParameterChanged},
        },
    },
    parameter::{CustomParameter, CustomParameterId, Parameter},
};
use std::alloc::Layout;

type Graph = RetainedPayload<Vec<DataEvent>>;

/// Borrow custom backing; every other parameter family is entirely Copy.
pub(super) enum ParameterValue<'a> {
    Custom(&'a CustomParameter),
    Scalar(Parameter),
}
impl<'a> ParameterValue<'a> {
    pub(super) fn borrowed(parameter: &'a Parameter) -> Self {
        match parameter {
            Parameter::Custom(custom) => Self::Custom(custom),
            Parameter::Sumeragi(value) => Self::Scalar(Parameter::Sumeragi(*value)),
            Parameter::Block(value) => Self::Scalar(Parameter::Block(*value)),
            Parameter::Transaction(value) => Self::Scalar(Parameter::Transaction(*value)),
            Parameter::SmartContract(value) => Self::Scalar(Parameter::SmartContract(*value)),
            Parameter::Executor(value) => Self::Scalar(Parameter::Executor(*value)),
        }
    }
    fn name_layout(&self) -> Option<Layout> {
        match self {
            Self::Custom(value) => value.id().name().admission_clone_layout(),
            Self::Scalar(_) => None,
        }
    }
    fn copy(
        self,
        budget: &AllocationBudget,
        reservation: &mut iroha_allocation::AllocationReservation,
        ledger: &mut ChargedBuffer<AllocationCharge>,
    ) -> Result<Parameter, ParameterIngressError> {
        match self {
            Self::Scalar(value) => Ok(value),
            Self::Custom(value) => {
                // Json carries its own original text/control charges through every
                // raw Clone. The outer ledger must not count those allocations again.
                let payload = value.payload().try_retain_in(budget)?;
                if let Some(layout) = value.id().name().admission_clone_layout() {
                    ledger.push_reserved(
                        reservation
                            .try_split(layout)
                            .map_err(|_| ParameterRetentionError::PlanMismatch)?,
                    );
                }
                let id = CustomParameterId(value.id().name().try_clone_for_admission()?);
                Ok(Parameter::Custom(CustomParameter::new(id, payload)))
            }
        }
    }
}

/// Fund the event vector, exact optional identifier strings and shared shell
/// before copying leaves. Json backing is self-custodial in the same pool.
#[allow(unsafe_code)]
pub(super) fn prepare(
    old: ParameterValue<'_>,
    new: ParameterValue<'_>,
    pool: &AllocationBudget,
) -> Result<SharedDataEvent, ParameterIngressError> {
    let overflow = || ParameterRetentionError::Admission(AllocationRefusal::DemandOverflow);
    let entries =
        1 + usize::from(old.name_layout().is_some()) + usize::from(new.name_layout().is_some());
    let vector = Layout::array::<DataEvent>(1).map_err(|_| overflow())?;
    let ledger_layout = Layout::array::<AllocationCharge>(entries).map_err(|_| overflow())?;
    let shell_layout = ChargedShared::<Graph>::allocation_layout();
    let mut bytes = vector
        .size()
        .checked_add(ledger_layout.size())
        .and_then(|n| n.checked_add(shell_layout.size()))
        .ok_or_else(overflow)?;
    for layout in [old.name_layout(), new.name_layout()].into_iter().flatten() {
        bytes = bytes.checked_add(layout.size()).ok_or_else(overflow)?;
    }
    let mut reservation = pool
        .try_reserve_bytes(bytes)
        .map_err(ParameterRetentionError::from)?;
    // Declaration/drop order keeps all values ahead of their original charges.
    let mut ledger = ChargedBuffer::from_reservation(entries, &mut reservation)
        .map_err(ParameterRetentionError::from)?;
    let shell = ChargedShared::<Graph>::reserve_from(&mut reservation)
        .map_err(ParameterRetentionError::from)?;
    let mut events = ChargedBuffer::from_reservation(1, &mut reservation)
        .map_err(ParameterRetentionError::from)?;
    let old_value = old.copy(pool, &mut reservation, &mut ledger)?;
    let new_value = new.copy(pool, &mut reservation, &mut ledger)?;
    events.push_reserved(DataEvent::Configuration(ConfigurationEvent::Changed(
        ParameterChanged {
            old_value,
            new_value,
        },
    )));
    // SAFETY: the immutable batch moves the exact vector and Name allocations
    // with their original ledger. Each Json owns its same-pool backing itself;
    // no mutable access or extracted value can separate these ownership chains.
    let (events, charge) = unsafe { events.into_allocation_parts() };
    ledger.push_reserved(charge);
    let graph = unsafe { RetainedPayload::try_new(events, ledger, pool) }.map_err(
        |(events, ledger, _)| {
            drop(events);
            drop(ledger);
            ParameterRetentionError::PlanMismatch
        },
    )?;
    let graph = shell.initialize(graph);
    SharedDataEvent::from_funded_batch(&graph, 0, pool)
        .ok_or_else(|| ParameterRetentionError::PlanMismatch.into())
}

/// Select the exact previous value without cloning a custom parameter or graph.
/// A newly created custom slot reports the proposed value as its old value,
/// preserving the existing event contract for first insertion.
pub(super) fn for_change(
    current: &crate::pipeline_gas_policy::StateParameters,
    successor: &crate::pipeline_gas_policy::StateParameters,
    next: &Parameter,
    pool: &AllocationBudget,
) -> Result<SharedDataEvent, ParameterIngressError> {
    macro_rules! previous {
        ($($container:ident($family:ident.$field:ident) => $single:ident::$variant:ident),* $(,)?) => {
            match next { $(
                Parameter::$container(iroha_data_model::parameter::$single::$variant(_)) =>
                    ParameterValue::Scalar(Parameter::$container(iroha_data_model::parameter::$single::$variant(current.$family.$field))),
            )*
                Parameter::Custom(offered) => {
                    let new = successor.custom().get(offered.id()).ok_or(ParameterRetentionError::PlanMismatch)?;
                    let old = current.custom().get(offered.id()).unwrap_or(new);
                    return prepare(ParameterValue::Custom(old), ParameterValue::Custom(new), pool);
                }
            }
        }
    }
    let old = previous!(
                Sumeragi(sumeragi.max_clock_drift_ms) => SumeragiParameter::MaxClockDriftMs,
                Sumeragi(sumeragi.due_work_max_lag_ms) => SumeragiParameter::DueWorkMaxLagMs,
                Sumeragi(sumeragi.payload_retry_interval_ms) => SumeragiParameter::PayloadRetryIntervalMs,
                Sumeragi(sumeragi.exec_budget_ms) => SumeragiParameter::ExecBudgetMs,
                Sumeragi(sumeragi.apply_budget_ms) => SumeragiParameter::ApplyBudgetMs,
                Sumeragi(sumeragi.max_block_bytes) => SumeragiParameter::MaxBlockBytes,
                Sumeragi(sumeragi.epoch_length_blocks) => SumeragiParameter::EpochLengthBlocks,
                Sumeragi(sumeragi.demotion_window) => SumeragiParameter::DemotionWindow,
                Block(block.max_transactions) => BlockParameter::MaxTransactions,
                Block(block.max_time_trigger_invocations) => BlockParameter::MaxTimeTriggerInvocations,
                Block(block.execution_output) => BlockParameter::ExecutionOutput,
                Block(block.fastpq_source) => BlockParameter::FastpqSource,
                Transaction(transaction.max_signatures) => TransactionParameter::MaxSignatures,
                Transaction(transaction.max_instructions) => TransactionParameter::MaxInstructions,
                Transaction(transaction.ivm_bytecode_size) => TransactionParameter::IvmBytecodeSize,
                Transaction(transaction.max_tx_bytes) => TransactionParameter::MaxTxBytes,
                Transaction(transaction.max_decompressed_bytes) => TransactionParameter::MaxDecompressedBytes,
                Transaction(transaction.max_metadata_depth) => TransactionParameter::MaxMetadataDepth,
                Transaction(transaction.max_time_to_live_ms) => TransactionParameter::MaxTimeToLiveMs,
                Transaction(transaction.require_height_ttl) => TransactionParameter::RequireHeightTtl,
                Transaction(transaction.require_sequence) => TransactionParameter::RequireSequence,
                SmartContract(smart_contract.fuel) => SmartContractParameter::Fuel,
                SmartContract(smart_contract.memory) => SmartContractParameter::Memory,
                SmartContract(smart_contract.execution_depth) => SmartContractParameter::ExecutionDepth,
                SmartContract(smart_contract.max_output_items) => SmartContractParameter::MaxOutputItems,
                SmartContract(smart_contract.max_output_bytes) => SmartContractParameter::MaxOutputBytes,
                Executor(executor.fuel) => SmartContractParameter::Fuel,
                Executor(executor.memory) => SmartContractParameter::Memory,
                Executor(executor.execution_depth) => SmartContractParameter::ExecutionDepth,
                Executor(executor.max_output_items) => SmartContractParameter::MaxOutputItems,
                Executor(executor.max_output_bytes) => SmartContractParameter::MaxOutputBytes,
    );
    prepare(old, ParameterValue::borrowed(next), pool)
}

#[cfg(test)]
mod tests;
