//! Typed first rejection retained after a failed genesis output owner is poisoned.

use iroha_data_model::block::execution_output::{ExecutionOutputV1, ProfiledTransactionRejection};

/// The first rejected row in the complete canonical genesis execution-output order.
///
/// The index includes Network, Pipeline, and Time outputs; it is not a transaction
/// input index. Retaining this diagnostic does not attach or authorize failed outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenesisOutputRejection {
    /// Zero-based position in the ordered complete execution outputs.
    pub output_index: usize,
    /// Original typed rejection cause, retained independently of discarded output storage.
    pub reason: Box<ProfiledTransactionRejection>,
}

impl std::fmt::Display for GenesisOutputRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "output {}: {}", self.output_index, self.reason)
    }
}

impl std::error::Error for GenesisOutputRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self.reason.as_ref() {
            ProfiledTransactionRejection::Baseline(r) => r,
            ProfiledTransactionRejection::C10(r) => r,
        })
    }
}

impl GenesisOutputRejection {
    /// Select deterministically before destroying a failed output owner.
    pub(crate) fn first(outputs: &[ExecutionOutputV1]) -> Option<Self> {
        outputs.iter().enumerate().find_map(|(output_index, row)| {
            row.result().as_ref().err().map(|reason| Self {
                output_index,
                reason: Box::new(reason.into_owned()),
            })
        })
    }
}
