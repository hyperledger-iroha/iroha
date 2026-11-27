//! Certified C10 execution outputs and borrowed result projections.
use super::{
    InvocationCompletionV1, PipelineInvocationV1, TimeInvocationV1, TriggerFailureRootV1,
    TriggerLifecycleEffectsV1,
};
use crate::{
    block::consensus::NexusFeeReceipt,
    events::data::prelude::AssetBatchTransferOutcome,
    ivm_c10::{TransactionRejectionReasonC10V1, TransactionResultC10V1},
    smart_contract::event::ContractEmissionsV1,
    transaction::{TransactionResult, error::TransactionRejectionReason},
    trigger::DataTriggerSequence,
};
use norito::codec::{Decode, Encode};

/// Original execution owner of one C10 output.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::execution_output::ExecutionSourceC10V1")]
#[norito(
    tag = "kind",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ExecutionSourceC10V1 {
    /// Position in the canonical network-input projection.
    Network(u32),
    /// Pipeline source and the diagnostic retained after a real rollback.
    Pipeline {
        /// Pre-body pipeline invocation.
        invocation: PipelineInvocationV1,
        /// Explicit diagnostic presence or absence.
        #[norito(required)]
        failure_root: Option<TriggerFailureRootV1>,
    },
    /// Scheduled source and the diagnostic retained after a real rollback.
    Time {
        /// Pre-body scheduled invocation.
        invocation: TimeInvocationV1,
        /// Explicit diagnostic presence or absence.
        #[norito(required)]
        failure_root: Option<TriggerFailureRootV1>,
    },
}

/// Sole certified result of a source that executed through its selected C10 profile.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::execution_output::ExecutionOutputC10V1")]
#[norito(deny_unknown_fields)]
pub struct ExecutionOutputC10V1 {
    /// Authenticated input or pre-body invocation.
    pub source: ExecutionSourceC10V1,
    /// The complete success or rejection, receipts and emissions.
    pub result: TransactionResultC10V1,
    /// Ordered completion records owned by this call.
    pub completions: Vec<InvocationCompletionV1>,
    /// Coverage from the selected trigger-lifecycle service. Absence grants no coverage.
    #[norito(required)]
    pub trigger_lifecycle: Option<TriggerLifecycleEffectsV1>,
}

/// Borrow the original result, retaining the immutable family of any rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionResultRef<'a> {
    /// Baseline or runtime-lifecycle result.
    Baseline(&'a TransactionResult),
    /// C10 result.
    C10(&'a TransactionResultC10V1),
}

/// Borrow a rejection without converting a profile's closed error set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionRejectionRef<'a> {
    /// Baseline or runtime-lifecycle rejection.
    Baseline(&'a TransactionRejectionReason),
    /// C10 rejection.
    C10(&'a TransactionRejectionReasonC10V1),
}
/// Owned native rejection for diagnostics whose source block can be released.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfiledTransactionRejection {
    /// Original installed result family.
    Baseline(TransactionRejectionReason),
    /// Original C10 rejection family.
    C10(TransactionRejectionReasonC10V1),
}

impl From<TransactionRejectionReason> for ProfiledTransactionRejection {
    fn from(value: TransactionRejectionReason) -> Self {
        Self::Baseline(value)
    }
}
impl ProfiledTransactionRejection {
    /// Borrow the original error and its family.
    pub fn as_borrowed(&self) -> TransactionRejectionRef<'_> {
        match self {
            Self::Baseline(reason) => TransactionRejectionRef::Baseline(reason),
            Self::C10(reason) => TransactionRejectionRef::C10(reason),
        }
    }
}

impl<'a> From<&'a TransactionRejectionReason> for TransactionRejectionRef<'a> {
    fn from(reason: &'a TransactionRejectionReason) -> Self {
        Self::Baseline(reason)
    }
}

impl<'a> From<&'a ProfiledTransactionRejection> for TransactionRejectionRef<'a> {
    fn from(reason: &'a ProfiledTransactionRejection) -> Self {
        reason.as_borrowed()
    }
}
impl<'a> TransactionRejectionRef<'a> {
    /// Borrow the installed baseline family when this is a baseline rejection.
    pub fn baseline(self) -> Option<&'a TransactionRejectionReason> {
        match self {
            Self::Baseline(reason) => Some(reason),
            Self::C10(_) => None,
        }
    }

    /// Keep the exact source rejection after releasing its carrier.
    pub fn into_owned(self) -> ProfiledTransactionRejection {
        match self {
            Self::Baseline(reason) => ProfiledTransactionRejection::Baseline(reason.clone()),
            Self::C10(reason) => ProfiledTransactionRejection::C10(reason.clone()),
        }
    }
}
impl core::fmt::Display for ProfiledTransactionRejection {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.as_borrowed(), f)
    }
}
impl core::fmt::Display for TransactionRejectionRef<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Baseline(reason) => core::fmt::Display::fmt(reason, f),
            Self::C10(reason) => core::fmt::Display::fmt(reason, f),
        }
    }
}

impl<'a> From<&'a TransactionResult> for TransactionResultRef<'a> {
    fn from(value: &'a TransactionResult) -> Self {
        Self::Baseline(value)
    }
}
impl<'a> TransactionResultRef<'a> {
    /// Borrow the installed baseline family when this is a baseline execution.
    pub fn baseline(self) -> Option<&'a TransactionResult> {
        match self {
            Self::Baseline(result) => Some(result),
            Self::C10(_) => None,
        }
    }

    /// Borrow the returned trace or the original family's rejection.
    pub fn as_ref(self) -> Result<&'a DataTriggerSequence, TransactionRejectionRef<'a>> {
        match self {
            Self::Baseline(result) => result.0.as_ref().map_err(TransactionRejectionRef::Baseline),
            Self::C10(result) => result.0.as_ref().map_err(TransactionRejectionRef::C10),
        }
    }
    /// Whether execution committed its business effects.
    pub fn is_ok(self) -> bool {
        self.as_ref().is_ok()
    }
    /// Whether execution rejected and rolled back its business effects.
    pub fn is_err(self) -> bool {
        self.as_ref().is_err()
    }
    /// Borrow the independently retained per-leg transfer receipts.
    pub fn batch_transfer_outcomes(self) -> &'a [AssetBatchTransferOutcome] {
        match self {
            Self::Baseline(result) => result.batch_transfer_outcomes(),
            Self::C10(result) => &result.1,
        }
    }
    /// Borrow the actual fee receipt, including fees retained on rejection.
    pub fn nexus_fee_receipt(self) -> Option<&'a NexusFeeReceipt> {
        match self {
            Self::Baseline(result) => result.nexus_fee_receipt(),
            Self::C10(result) => result.2.as_ref(),
        }
    }
    /// Borrow the contract emissions from this execution.
    pub fn contract_events(self) -> &'a ContractEmissionsV1 {
        match self {
            Self::Baseline(result) => result.contract_events(),
            Self::C10(result) => &result.3,
        }
    }
}

impl super::ExecutionOutputV1 {
    /// Move an already owned bounded terminal into the C10 result family.
    /// The execution owner must authenticate the profile before publishing this row.
    ///
    /// # Errors
    /// Refuses nonterminal rows or a source without an execution result.
    pub fn into_c10_output_limit(self) -> Result<Self, String> {
        if !self.is_output_limit_rejection() {
            return Err("C10 terminal requires an output-limit disposition".into());
        }
        self.into_c10_sizing_terminal()
    }

    pub(in crate::block) fn into_c10_sizing_terminal(self) -> Result<Self, String> {
        use super::{NetworkExecutionOutputV2, PipelineExecutionOutputV2, TimeExecutionOutputV2};
        let (source, result, completions, trigger_lifecycle) = match self {
            Self::Network(row) => (
                ExecutionSourceC10V1::Network(row.input_index),
                row.result,
                row.completions,
                None,
            ),
            Self::NetworkWithLifecycle(NetworkExecutionOutputV2 {
                execution: row,
                trigger_lifecycle,
            }) => (
                ExecutionSourceC10V1::Network(row.input_index),
                row.result,
                row.completions,
                Some(trigger_lifecycle),
            ),
            Self::Pipeline(row) => (
                ExecutionSourceC10V1::Pipeline {
                    invocation: row.invocation,
                    failure_root: row.failure_root,
                },
                row.result,
                row.completions,
                None,
            ),
            Self::PipelineWithLifecycle(PipelineExecutionOutputV2 {
                execution: row,
                trigger_lifecycle,
            }) => (
                ExecutionSourceC10V1::Pipeline {
                    invocation: row.invocation,
                    failure_root: row.failure_root,
                },
                row.result,
                row.completions,
                Some(trigger_lifecycle),
            ),
            Self::Time(row) => (
                ExecutionSourceC10V1::Time {
                    invocation: row.invocation,
                    failure_root: row.failure_root,
                },
                row.result,
                row.completions,
                None,
            ),
            Self::TimeWithLifecycle(TimeExecutionOutputV2 {
                execution: row,
                trigger_lifecycle,
            }) => (
                ExecutionSourceC10V1::Time {
                    invocation: row.invocation,
                    failure_root: row.failure_root,
                },
                row.result,
                row.completions,
                Some(trigger_lifecycle),
            ),
            Self::C10(_) => return Ok(self),
            Self::GovernanceBlockStart(_) => {
                return Err("governance block start has no execution profile".into());
            }
        };
        let TransactionResult(
            Err(TransactionRejectionReason::LimitCheck(reason)),
            transfers,
            fee,
            events,
        ) = result
        else {
            return Err("C10 terminal requires its preowned bounded rejection".into());
        };
        Ok(Self::C10(ExecutionOutputC10V1 {
            source,
            result: TransactionResultC10V1(
                Err(TransactionRejectionReasonC10V1::LimitCheck(reason)),
                transfers,
                fee,
                events,
            ),
            completions,
            trigger_lifecycle,
        }))
    }
}
