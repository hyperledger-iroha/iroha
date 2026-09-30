//! Fixed-size reverse branch selection beneath an independently authenticated global frontier.
//!
//! This cursor does not authenticate the global lane record or grant monetary authority. The
//! caller retains that original authority and every source read owner. Advancement borrows
//! complete available custody and never clones, extracts or replaces it on failure.

use iroha_data_model::sumeragi_lanes::SumeragiLaneFrontier;
use iroha_sumeragi::{
    availability::AvailableBody,
    crypto::{AttestationVerifier, Crypto, Verifier},
    message::Qc,
    topology::demotion_window,
    types::{Hash32, HeightConfig},
};

/// Why an independently anchored lane history cannot cover a signed report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LaneAncestryError {
    /// The supplied pinned lane authority or genesis is inconsistent.
    #[error("invalid independent lane ancestry authority")]
    Authority,
    /// The requested native parent is beyond the globally merged frontier.
    #[error("native evidence parent lacks globally anchored coverage")]
    Uncovered,
    /// The exact requested native interval has already been authenticated.
    #[error("lane ancestry interval is already complete")]
    Complete,
    /// The frame belongs to a different source, branch, result or predecessor.
    #[error("lane frame differs from the globally anchored branch")]
    Branch,
    /// The original exact commit quorum did not verify under pinned authority.
    #[error("lane ancestry commit certificate did not verify")]
    Certificate,
}

/// Reverse hash-and-result authentication from one globally committed native lane frontier.
///
/// Construction consumes the caller's original pinned configuration without cloning it. This
/// cursor selects a branch but supplies no global occurrence height. The enclosing reader must
/// independently authenticate its creation record, frontier and original staking provenance.
pub struct LaneAncestry {
    instance: Hash32,
    config: HeightConfig,
    genesis: SumeragiLaneFrontier,
    expected: SumeragiLaneFrontier,
    parent_height: u64,
    interval: Option<(u64, u64)>,
    first: u64,
}

impl LaneAncestry {
    /// Select the complete native parent/demotion interval beneath an original global anchor.
    /// `genesis`, `frontier`, `instance` and `config` must all be independently authenticated;
    /// an inspected frame or local QC cannot provide any of them.
    ///
    /// # Errors
    /// Inconsistent pinned authority, a zero subject, or a subject whose parent is unanchored.
    pub fn new(
        instance: Hash32,
        config: HeightConfig,
        genesis: SumeragiLaneFrontier,
        frontier: SumeragiLaneFrontier,
        subject_height: u64,
        window: u64,
    ) -> Result<Self, LaneAncestryError> {
        let parent_height = subject_height
            .checked_sub(1)
            .ok_or(LaneAncestryError::Uncovered)?;
        if parent_height > frontier.height {
            return Err(LaneAncestryError::Uncovered);
        }
        if genesis.height != 0
            || (frontier.height == 0 && frontier != genesis)
            || config.epoch.id.epoch != 0
            || config.epoch.id.context != Hash32(genesis.result)
            || config.epoch.authority_generation != Hash32(genesis.result)
            || config.epoch.first_height != 0
            || config.epoch.last_height != u64::MAX
            || !iroha_data_model::block::consensus::is_valid_committee_size(config.committee.n())
        {
            return Err(LaneAncestryError::Authority);
        }
        let interval = demotion_window(subject_height, 0, window);
        let first = interval.map_or(parent_height, |(first, _)| first);
        Ok(Self {
            instance,
            config,
            genesis,
            // Subject one has the independently derived genesis parent and no demotion window.
            // No local frame, including an arbitrarily high frontier, can change that context.
            expected: if parent_height == 0 {
                genesis
            } else {
                frontier
            },
            parent_height,
            interval,
            first,
        })
    }

    /// Next exact native height to read. Every frame above the requested interval also counts.
    #[must_use]
    pub fn next_height(&self) -> Option<u64> {
        (self.expected.height > 0 && self.expected.height >= self.first)
            .then_some(self.expected.height)
    }

    /// Native parent height; never a global evidence-age or stake-tenure boundary.
    #[must_use]
    pub fn parent_height(&self) -> u64 {
        self.parent_height
    }

    /// Exact inclusive demotion interval, which the consumer must retain in ascending order.
    #[must_use]
    pub fn demotion_interval(&self) -> Option<(u64, u64)> {
        self.interval
    }

    /// Borrow the original pinned configuration without cloning its backing.
    #[must_use]
    pub fn config(&self) -> &HeightConfig {
        &self.config
    }

    /// Verify one full original frame and advance only along its authenticated parent link.
    /// A failure leaves both this cursor and the borrowed frame owners unchanged. A caller
    /// retaining a refused `LaneFrameRead` must retry that same read, not reopen another source.
    ///
    /// # Errors
    /// Already complete, wrong source/hash/result/ancestry, or an invalid original commit QC.
    pub fn advance(
        &mut self,
        crypto: &dyn Crypto,
        attestations: &dyn AttestationVerifier,
        body: &AvailableBody,
        qc: &Qc,
    ) -> Result<(), LaneAncestryError> {
        let height = self.next_height().ok_or(LaneAncestryError::Complete)?;
        let header = body.header();
        let source = body.source();
        if source.instance() != self.instance
            || source.height() != height
            || source.block_hash() != Hash32(self.expected.block_hash)
            || source.config() != &self.config
            || header.height != height
            || header.instance != self.instance
            || header.epoch != self.config.epoch.id
            || header.attest
            || !header.control_witness.is_empty()
            || body.hash(crypto) != Hash32(self.expected.block_hash)
            || qc.result != Hash32(self.expected.result)
            || (height == 1
                && (header.parent_hash != Hash32(self.genesis.block_hash)
                    || header.parent_result != Hash32(self.genesis.result)))
        {
            return Err(LaneAncestryError::Branch);
        }
        if !Verifier::new(
            crypto,
            &self.instance,
            &self.config.epoch.id,
            &self.config.committee,
        )
        .verify_commit_qc(attestations, qc, Some(header))
        {
            return Err(LaneAncestryError::Certificate);
        }
        self.expected = SumeragiLaneFrontier {
            height: height - 1,
            block_hash: header.parent_hash.0,
            result: header.parent_result.0,
        };
        Ok(())
    }
}
