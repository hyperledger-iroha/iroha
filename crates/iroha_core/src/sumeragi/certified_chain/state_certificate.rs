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
        self.verify_executed_successor(&parent, current)
            .map_err(query_failure)
    }

    /// Verify one target and an optional older certificate in one original-tip walk.
    ///
    /// The selector runs only after the target's parent, exact quorum and signed
    /// availability are authenticated. It may select a strictly older non-genesis
    /// height. The same captured source, source-admission callback and allocation
    /// scope cover the whole walk; the ancestry walk never restarts. The constructor's
    /// signed-genesis read remains separately admitted.
    /// An immediately preceding selection reuses the original owned parent receipt.
    ///
    /// # Errors
    /// Rejects unavailable or changed ancestry, invalid certificates, exhausted
    /// source admission, genesis selections and selections at/above the target.
    pub fn certified_with_ancestor_from_execution(
        &self,
        height: NonZeroUsize,
        before_read: impl FnMut(u64, u64) -> Result<(), QueryExecutionFail>,
        select: impl FnOnce(&CertifiedBlock) -> Result<Option<NonZeroUsize>, QueryExecutionFail>,
    ) -> Result<(CertifiedBlock, Option<CertifiedBlock>), QueryExecutionFail> {
        let invalid = |message: &str| QueryExecutionFail::Conversion(message.into());
        if height.get() == 1 {
            return Err(invalid("genesis alone has no native CommitQC"));
        }
        let ChainSource::State(view) = &self.source else {
            return Err(invalid(
                "certificate read requires original State authority",
            ));
        };
        let mut select = Some(select);
        let mut target_height = height.get() as u64;
        let mut target = None;
        let mut latest = None;
        let mut ancestor = None;
        view.canonical_history().visit_executed_backwards_until(
            NonZeroUsize::new(1).expect("genesis height is nonzero"),
            height,
            before_read,
            |receipt| {
                if let Some(current) = target.take() {
                    let certified = self
                        .verify_executed_successor(&receipt, current)
                        .map_err(query_failure)?;
                    if let Some(select) = select.take() {
                        let selected = select(&certified)?;
                        latest = Some(certified);
                        let Some(selected) = selected else {
                            return Ok(core::ops::ControlFlow::Break(()));
                        };
                        if selected.get() <= 1 || selected >= height {
                            return Err(invalid(
                                "selected ancestor must precede the target and follow genesis",
                            ));
                        }
                        target_height = selected.get() as u64;
                    } else {
                        ancestor = Some(certified);
                        return Ok(core::ops::ControlFlow::Break(()));
                    }
                }
                if receipt.height() == target_height {
                    target = Some(receipt);
                }
                Ok(core::ops::ControlFlow::Continue(()))
            },
        )?;
        let latest = latest.ok_or_else(|| invalid("authenticated target is absent"))?;
        if select.is_none() && target_height != height.get() as u64 && ancestor.is_none() {
            return Err(invalid("authenticated selected ancestor is absent"));
        }
        Ok((latest, ancestor))
    }

    pub(super) fn verify_executed_successor(
        &self,
        parent: &CommittedBlock,
        current: CommittedBlock,
    ) -> Result<CertifiedBlock, VerificationReadError> {
        let height = current.height();
        let malformed = |reason: String| ChainReadError::Committee { height, reason };
        if !current.extends(parent) {
            return Err(ChainReadError::Discontinuous { height }.into());
        }
        parent
            .commitment
            .schedule
            .validate_successor(&current.commitment.schedule)
            .map_err(|error| malformed(error.to_string()))?;
        let schedule::ScheduledSlot::Ready(scheduled) = &parent.commitment.schedule.next else {
            return Err(malformed("parent has no authorized successor".into()).into());
        };
        let config = scheduled
            .height_config()
            .map_err(|error| malformed(error.to_string()))?;
        // Resource admission precedes the boolean signature relation and keeps
        // the original inherited cumulative allowance. No refusal is converted
        // to an invalid certificate or hidden in a new per-frame scope.
        let scratch = iroha_crypto::BlsNormalAggregateScratch::new(|bytes| {
            #[cfg(all(test, sumeragi_core_mutation = "HC13"))]
            {
                let _ = bytes;
                Ok(())
            }
            #[cfg(not(all(test, sumeragi_core_mutation = "HC13")))]
            query_scratch_admission(bytes)
        })
        .map_err(VerificationReadError::Resource)?;
        let authority = VerifiedAuthority::with_crypto(
            scheduled.epoch.clone(),
            height,
            BlsCrypto::with_aggregate_scratch(scratch),
        )?;
        let certified = self
            .verification_context()
            .verify_certificate_with_scratch_admission(
                current,
                &authority,
                Some(&config),
                None,
                &mut query_scratch_admission,
            )?;
        verify_boundary_source(&certified, parent, &authority)?;
        Ok(certified)
    }
}

/// Charge the inherited request allowance before allocating verification scratch.
pub(super) fn query_scratch_admission(
    bytes: usize,
) -> Result<(), norito::core::DecodeResourceError> {
    #[cfg(all(test, sumeragi_core_mutation = "HC10"))]
    {
        let _ = bytes;
        return Ok(());
    }
    #[cfg(not(all(test, sumeragi_core_mutation = "HC10")))]
    norito::core::reserve_decode_allocation(bytes).map_err(|error| {
        error
            .decode_resource_error()
            .expect("allocation admission is a resource error")
    })
}

fn query_failure(error: VerificationReadError) -> QueryExecutionFail {
    match error {
        VerificationReadError::Resource(_) => QueryExecutionFail::GasBudgetExceeded,
        error => QueryExecutionFail::Conversion(error.to_string()),
    }
}

#[cfg(test)]
mod allocation_tests {
    use super::*;

    #[test]
    fn state_certificate_maps_local_scratch_refusal_without_source_invalidity() {
        let error = VerificationReadError::Resource(
            norito::core::DecodeResourceError::TotalAllocationExceeded {
                attempted: 17,
                limit: 16,
            },
        );
        assert_eq!(query_failure(error), QueryExecutionFail::GasBudgetExceeded);
    }
}
