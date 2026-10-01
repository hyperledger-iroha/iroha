//! Native certificates anchored in the captured original execution tip.

use super::*;
use iroha_data_model::query::error::QueryExecutionFail;

impl<'v, V: StateReadOnly + ?Sized> CertifiedChain<'v, V> {
    /// Authenticate signed genesis after admitting its exact canonical source frame.
    ///
    /// Use the same admission callback and allocation scope for later certificate reads
    /// so genesis cannot escape the request's cumulative source allowance. This performs
    /// no execution authentication: native reads still require the original State tip.
    ///
    /// # Errors
    /// Refuses missing or changed canonical genesis, source/decode resource exhaustion,
    /// and genesis that does not authenticate this State's network and chain instance.
    pub fn new_with_source_admission(
        view: &'v V,
        before_read: impl FnMut(u64, u64) -> Result<(), QueryExecutionFail>,
    ) -> Result<Self, QueryExecutionFail> {
        let genesis = view.canonical_history().block_with_admission(
            NonZeroUsize::new(GENESIS_HEIGHT as usize).expect("genesis height is nonzero"),
            before_read,
        )?;
        Self::from_genesis(ChainSource::State(view), genesis)
            .map_err(|error| QueryExecutionFail::Conversion(error.to_string()))
    }

    /// Authenticate a local certificate through this view's original execution tip.
    ///
    /// The exact parent and target are authenticated by the reverse native hash/result
    /// chain. The parent's executed schedule then supplies the target's authority and
    /// parameters. BLS quorum, paired-Pasta and signed availability checks still run.
    /// A recent read therefore does not repeatedly decode every block since genesis.
    /// The caller admits each source frame before I/O and retains its allocation scope
    /// across the entire operation. No fresh per-frame decode budget is installed.
    ///
    /// # Errors
    /// Refuses genesis, non-State sources, missing original tip authority, exhausted
    /// source admission, changed execution ancestry or invalid native certificates.
    pub fn certified_from_execution(
        &self,
        height: NonZeroUsize,
        before_read: impl FnMut(u64, u64) -> Result<(), QueryExecutionFail>,
    ) -> Result<CertifiedBlock, QueryExecutionFail> {
        let invalid = |message: String| QueryExecutionFail::Conversion(message);
        let ChainSource::State(view) = &self.source else {
            return Err(invalid(
                "certificate read requires original State authority".into(),
            ));
        };
        let parent_height = height
            .get()
            .checked_sub(1)
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| invalid("genesis alone has no native CommitQC".into()))?;
        let mut parent = None;
        let mut current = None;
        view.canonical_history().visit_executed_backwards(
            parent_height,
            height,
            before_read,
            |receipt| {
                if receipt.height() == height.get() as u64 {
                    current = Some(receipt);
                } else {
                    parent = Some(receipt);
                }
                Ok(())
            },
        )?;
        let parent = parent.ok_or_else(|| invalid("authenticated parent is absent".into()))?;
        let current = current.ok_or_else(|| invalid("authenticated target is absent".into()))?;
        self.verify_executed_successor(parent, current)
            .map_err(|error| invalid(error.to_string()))
    }

    pub(super) fn verify_executed_successor(
        &self,
        parent: CommittedBlock,
        current: CommittedBlock,
    ) -> Result<CertifiedBlock, ChainReadError> {
        let height = current.height();
        let malformed = |reason: String| ChainReadError::Committee { height, reason };
        if !current.extends(&parent) {
            return Err(ChainReadError::Discontinuous { height });
        }
        parent
            .commitment
            .schedule
            .validate_successor(&current.commitment.schedule)
            .map_err(|error| malformed(error.to_string()))?;
        let schedule::ScheduledSlot::Ready(scheduled) = &parent.commitment.schedule.next else {
            return Err(malformed("parent has no authorized successor".into()));
        };
        let config = scheduled
            .height_config()
            .map_err(|error| malformed(error.to_string()))?;
        let authority = VerifiedAuthority::new(scheduled.epoch.clone(), height)?;
        let certified = self.verification_context().verify_certificate(
            current,
            &authority,
            Some(&config),
            None,
        )?;
        verify_boundary_source(&certified, &parent, &authority)?;
        Ok(certified)
    }
}
