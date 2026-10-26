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

/// One human line per event, shared by the Kagami and Mochi frontends.
impl std::fmt::Display for DeploymentProgress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Prepared(_) => f.write_str("Deployment prepared with exact fee quotes"),
            Self::Submitting(stage) => write!(
                f,
                "Submitting {} ({}/{})",
                stage.name, stage.number, stage.total
            ),
            Self::Recovering(stage) => write!(f, "Recovering {} by its retained hash", stage.name),
            Self::Applied { stage, .. } => write!(f, "{} applied", stage.name),
            Self::ReadingBack { .. } => f.write_str("Verifying deployed code and alias"),
        }
    }
}

pub fn stage(record: &PlanRecord, index: usize) -> DeploymentStage {
    let transaction = &record.transactions[index];
    DeploymentStage {
        number: index + 1,
        total: record.transactions.len(),
        name: transaction.name.clone(),
        hash: transaction.hash.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_reads_as_one_human_line_per_stage() {
        let stage = DeploymentStage {
            number: 2,
            total: 3,
            name: "register_contract".into(),
            hash: "ab".repeat(32),
        };
        assert_eq!(
            DeploymentProgress::Submitting(stage.clone()).to_string(),
            "Submitting register_contract (2/3)"
        );
        assert_eq!(
            DeploymentProgress::Recovering(stage).to_string(),
            "Recovering register_contract by its retained hash"
        );
    }
}
