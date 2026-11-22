//! Process-local evidence for repeated native signer history reads.

use super::{CertifiedChain, Hash, StateReadOnly, current_prefix_source};
use iroha_data_model::{ChainId, NetworkId};

/// A completed native prefix bound to its certificate-inclusive bytes and pinned header journal.
/// It carries no custody, registration, time, observation or operation decision.
pub struct CertifiedHistorySource {
    network: NetworkId,
    chain: ChainId,
    height: u64,
    wire: Hash,
    headers: Hash,
}

impl CertifiedHistorySource {
    // Reuse is a bounded optimization. Larger histories follow the ordinary native reader.
    const MAX_HEIGHT: u64 = 256;

    /// Enable source capture before the original certified walk starts.
    pub fn track<V: StateReadOnly>(reader: &mut CertifiedChain<'_, V>, through: u64) {
        if (2..=Self::MAX_HEIGHT).contains(&through) && !norito::core::decode_limits_active() {
            reader.enable_proof_source_cut();
        }
    }

    /// Retain the source of the reader's actual completed prefix. The native digest includes
    /// signed genesis and each authenticated successor, including the local commit certificate.
    /// A checkpoint without those original bytes cannot create this evidence.
    pub fn from_verified<V: StateReadOnly>(
        reader: &CertifiedChain<'_, V>,
        through: u64,
    ) -> Option<Self> {
        if norito::core::decode_limits_active() || !(2..=Self::MAX_HEIGHT).contains(&through) {
            return None;
        }
        let view = reader.proof_source_view()?;
        let (height, wire) = reader.proof_source_cut()?;
        if height != through || current_prefix_source(view, height)? != wire {
            return None;
        }
        Some(Self {
            network: *view.network_id(),
            chain: view.chain_id().clone(),
            height,
            wire,
            headers: header_digest(view, height)?,
        })
    }

    /// Check every original frame and the pinned header journal again on this fresh view.
    /// The caller still checks its requested header hashes and all current policy predicates.
    pub fn covers(&self, view: &impl StateReadOnly, through: u64) -> bool {
        !norito::core::decode_limits_active()
            && through >= 2
            && through <= self.height
            && view.network_id() == &self.network
            && view.chain_id() == &self.chain
            && header_digest(view, self.height) == Some(self.headers)
            && current_prefix_source(view, self.height) == Some(self.wire)
    }
}

fn header_digest(view: &impl StateReadOnly, height: u64) -> Option<Hash> {
    let hashes = view.block_hashes();
    let count = usize::try_from(height).ok()?;
    if hashes.len() < count {
        return None;
    }
    Hash::new_from_writer(|writer| {
        for hash in hashes.iter().take(count) {
            writer.write_all(hash.as_ref())?;
        }
        Ok(())
    })
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        state::World,
        sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig},
    };

    #[test]
    fn retained_history_checks_the_original_interior_certificate_and_inherited_budget() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        chain.commit_at(2_000, Vec::new());
        chain.commit_at(3_000, Vec::new());
        let source = {
            let view = chain.state().view();
            let mut reader = CertifiedChain::new(&view).unwrap();
            assert!(CertifiedHistorySource::from_verified(&reader, 3).is_none());
            CertifiedHistorySource::track(&mut reader, 3);
            for block in reader.walk(1, 3) {
                block.unwrap();
            }
            CertifiedHistorySource::from_verified(&reader, 3).unwrap()
        };
        let view = chain.state().view();
        assert!(source.covers(&view, 2));
        assert!(source.covers(&view, 3));
        assert!(!source.covers(&view, 4));
        norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(1024, 1024, 1024, 0, 8))
            .with(|| assert!(!source.covers(&view, 3)));
        drop(view);
        chain.corrupt_local_quorum_for_test(2, Signers::BelowQuorum);
        assert!(!source.covers(&chain.state().view(), 3));
    }
}
