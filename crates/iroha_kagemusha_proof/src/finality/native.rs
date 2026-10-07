//! Installed, bounded production of the complete ordinary Load finality graph.
//!
//! Every original key is qualified against its compiled owner at mount. Only
//! verifier metadata is retained; each active node reimports its original tables
//! and checks the mounted identity before proving. Native inputs are untrusted
//! witness proposals, never authorization verdicts. The application must also
//! perform its independent native genesis, custody and execution verification.

use iroha_pasta::{Ep, Eq, Fp, msm::MemoryBudget};
use iroha_plonk::{ProverConfig, keys::pk::artifact::ReadConfig, pcs::ipa::PinnedParams};
use iroha_plonk_recursion::FoldConfig;

use super::continuity::checkpoint::{self, ProofCheckpointStore};
pub use super::continuity::producer::Error;
use super::continuity::{SourceNodeEvidence, SourceVerifier};
use super::history::{HistoryAnchor, HistoryState};

mod artifacts;
mod graph;
mod program;
mod session;
pub(crate) mod source_layout;
mod source_policy;

pub use artifacts::{ArtifactId, ArtifactSource, Composition, NodeId, Program};
pub use graph::InstalledFinality;
pub use session::{BlockWitnessInput, HistoryPrefix, LoadWitnessInput};
pub use source_policy::compiled_leaf_schedule_transcript;

/// Explicit finite limits for the complete installed source graph.
#[derive(Clone, Copy, Debug)]
pub struct ImportLimits {
    /// Bound and cache policy for every original key.
    pub key: ReadConfig,
    /// Maximum original artifact entries loaded during installation.
    /// Repeated requests for the same identifier count once.
    pub maximum_artifacts: usize,
    /// Sum of original descriptor, VK and PK bytes for those entries.
    pub maximum_original_bytes: usize,
}

/// Both exact k16 generator parameter sets, shared by every installed node.
#[derive(Clone, Debug)]
pub struct Parameters {
    /// Source wrappers and Pallas obligations.
    pub pallas: PinnedParams<Ep>,
    /// Source circuits and Vesta obligations.
    pub vesta: PinnedParams<Eq>,
}

/// One actual proof invocation. Repeated program uses receive distinct sequence numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofRequest {
    /// Exact original source node being proved.
    pub node: NodeId,
    /// Monotone invocation number within this proving context.
    pub sequence: u64,
}

/// Explicit artifact, randomness and resource inputs for real proof production.
/// The randomness provider must supply independent proof randomness and fold salts.
pub struct ProvingContext<'a, 'r> {
    artifacts: &'a mut dyn ArtifactSource,
    randomness: &'a mut dyn FnMut(
        ProofRequest,
    )
        -> Result<super::continuity::tree::NodeRandomness<'r>, Error>,
    proof: ProverConfig,
    fold: &'a FoldConfig,
    sequence: u64,
    checkpoints: Option<&'a mut dyn ProofCheckpointStore>,
}
impl<'a, 'r> ProvingContext<'a, 'r> {
    /// Construct one finite working context. This grants no source or proof authority.
    pub fn new(
        artifacts: &'a mut dyn ArtifactSource,
        randomness: &'a mut dyn FnMut(
            ProofRequest,
        )
            -> Result<super::continuity::tree::NodeRandomness<'r>, Error>,
        proof: ProverConfig,
        fold: &'a FoldConfig,
    ) -> Self {
        Self {
            artifacts,
            randomness,
            proof,
            fold,
            sequence: 0,
            checkpoints: None,
        }
    }
    /// Attach bounded durable checkpoint storage. Every restored node is fully
    /// reverified under its installed key and exact expected source endpoints.
    #[must_use]
    pub fn with_checkpoints(mut self, store: &'a mut dyn ProofCheckpointStore) -> Self {
        self.checkpoints = Some(store);
        self
    }
    fn restore_source(
        &mut self,
        source: &SourceVerifier,
        endpoints: [Fp; 6],
        params: &PinnedParams<Eq>,
    ) -> Result<Option<SourceNodeEvidence>, Error> {
        match &mut self.checkpoints {
            Some(store) => {
                checkpoint::restore(*store, source, endpoints, params, self.proof.msm_budget)
            }
            None => Ok(None),
        }
    }
    fn retain_source(
        &mut self,
        source: &SourceVerifier,
        proof: &SourceNodeEvidence,
        params: &PinnedParams<Eq>,
    ) -> Result<(), Error> {
        match &mut self.checkpoints {
            Some(store) => checkpoint::retain(*store, source, proof, params, self.proof.msm_budget),
            None => Ok(()),
        }
    }
    fn entropy(
        &mut self,
        node: NodeId,
    ) -> Result<super::continuity::tree::NodeRandomness<'r>, Error> {
        let sequence = self.sequence;
        self.sequence = sequence.checked_add(1).ok_or(Error::Input)?;
        (self.randomness)(ProofRequest { node, sequence })
    }
}

fn circuit<T>(result: Result<T, iroha_plonk::frontend::Error>) -> Result<T, Error> {
    result.map_err(|_| Error::Input)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::finality::continuity::tree::OriginalBytes;

    struct NoArtifacts;
    impl ArtifactSource for NoArtifacts {
        fn load(&mut self, _: &ArtifactId) -> Result<OriginalBytes, Error> {
            panic!("entropy selection must not access original artifacts")
        }
    }

    #[test]
    fn entropy_requests_preserve_node_identity_and_consume_failed_sequences() {
        let requests = RefCell::new(Vec::new());
        let mut provider = |request| {
            requests.borrow_mut().push(request);
            Err(Error::Input)
        };
        let mut artifacts = NoArtifacts;
        let fold = FoldConfig::default();
        let mut context = ProvingContext::new(
            &mut artifacts,
            &mut provider,
            ProverConfig::default(),
            &fold,
        );
        let nodes = [
            NodeId::Leaf(Program::Bls, 7),
            NodeId::ProgramMerge(Program::Context, 11),
            NodeId::Composition(Composition::Receipt),
        ];
        for node in &nodes {
            assert!(context.entropy(node.clone()).is_err());
        }
        let expected: Vec<_> = nodes
            .into_iter()
            .zip(0_u64..)
            .map(|(node, sequence)| ProofRequest { node, sequence })
            .collect();
        assert_eq!(*requests.borrow(), expected);
    }

    #[test]
    fn exhausted_entropy_sequence_does_not_call_provider_or_wrap() {
        let requests = RefCell::new(Vec::new());
        let mut provider = |request| {
            requests.borrow_mut().push(request);
            Err(Error::Input)
        };
        let mut artifacts = NoArtifacts;
        let fold = FoldConfig::default();
        let mut context = ProvingContext::new(
            &mut artifacts,
            &mut provider,
            ProverConfig::default(),
            &fold,
        );
        context.sequence = u64::MAX - 1;
        assert!(context.entropy(NodeId::Append).is_err());
        assert!(context.entropy(NodeId::Genesis).is_err());
        assert!(context.entropy(NodeId::Genesis).is_err());
        assert_eq!(context.sequence, u64::MAX);
        assert_eq!(
            *requests.borrow(),
            vec![ProofRequest {
                node: NodeId::Append,
                sequence: u64::MAX - 1,
            }]
        );
    }
}
