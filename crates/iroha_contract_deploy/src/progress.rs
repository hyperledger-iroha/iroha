//! Typed, observational deployment progress tied to exact durable native stages.
use super::{AppliedEvidence, ContractAddress, ContractAlias, DeploymentPreflight, PlanRecord};

/// Exact native transaction stage being submitted, recovered, or confirmed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeploymentStage {
    /// One-based stage position in the exact signed plan.
    pub number: usize,
    /// Total number of native transactions in the plan.
    pub total: usize,
    /// Stable native stage name.
    pub name: String,
    /// Exact canonical signed transaction hash.
    pub hash: String,
}

/// Observational progress from the canonical deployment implementation.
///
/// `Applied` is emitted only after canonical finality and durable Applied evidence agree.
/// Observers do not control signing, dispatch, recovery, or receipt validity.
#[derive(Clone, Debug)]
pub enum DeploymentProgress {
    /// Authenticated exact plan review, emitted before any transaction dispatch or recovery.
    Prepared(Box<DeploymentPreflight>),
    /// The attempt record is durable and this exact transaction is about to be submitted.
    Submitting(DeploymentStage),
    /// An existing attempt will be recovered by exact hash without another submission.
    Recovering(DeploymentStage),
    /// The exact transaction has canonical, durably recorded Applied evidence.
    Applied {
        /// Stage whose exact hash was confirmed.
        stage: DeploymentStage,
        /// Canonical global, state-resolved finality evidence.
        evidence: AppliedEvidence,
    },
    /// Every native stage is Applied; authenticated alias and stored artifact readback begins.
    ReadingBack {
        /// Exact alias whose new binding must agree with the atomic commit.
        alias: ContractAlias,
        /// Exact committed contract address expected by readback.
        address: ContractAddress,
    },
}

pub(super) fn stage(record: &PlanRecord, index: usize) -> DeploymentStage {
    let transaction = &record.transactions[index];
    DeploymentStage {
        number: index + 1,
        total: record.transactions.len(),
        name: transaction.name.clone(),
        hash: transaction.hash.clone(),
    }
}
