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
        let genesis = view
            .canonical_history()
            .block_with_admission(
                NonZeroUsize::new(GENESIS_HEIGHT as usize).expect("genesis height is nonzero"),
                before_read,
            )
            .map_err(crate::smartcontracts::isi::query::query_transport_error)?;
        Self::from_genesis(ChainSource::State(view), genesis).map_err(|error| {
            crate::smartcontracts::isi::query::query_transport_error(
                error.map_rejection(|error| QueryExecutionFail::Conversion(error.to_string())),
            )
        })
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
        view.canonical_history()
            .visit_executed_backwards(parent_height, height, before_read, |receipt| {
                if receipt.height() == height.get() as u64 {
                    current = Some(receipt);
                } else {
                    parent = Some(receipt);
                }
                Ok(())
            })
            .map_err(crate::smartcontracts::isi::query::query_transport_error)?;
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
        view.canonical_history()
            .visit_executed_backwards_until(
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
            )
            .map_err(crate::smartcontracts::isi::query::query_transport_error)?;
        let latest = latest.ok_or_else(|| invalid("authenticated target is absent"))?;
        if select.is_none() && target_height != height.get() as u64 && ancestor.is_none() {
            return Err(invalid("authenticated selected ancestor is absent"));
        }
        Ok((latest, ancestor))
    }

    fn executed_successor_authority(
        &self,
        parent: &CommittedBlock,
        current: &CommittedBlock,
    ) -> Result<(VerifiedAuthority, iroha_sumeragi::types::HeightConfig), VerificationReadError>
    {
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
        let mut validation = EpochValidationScope::new();
        let config = scheduled
            .height_config_with_validation(&mut validation)
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
            &mut validation,
        )?;
        Ok((authority, config))
    }

    pub(super) fn verify_executed_successor(
        &self,
        parent: &CommittedBlock,
        current: CommittedBlock,
    ) -> Result<CertifiedBlock, VerificationReadError> {
        let (authority, config) = self.executed_successor_authority(parent, &current)?;
        let certified = self
            .verification_context()
            .verify_certificate_with_scratch_admission(
                current,
                &authority,
                Some(&config),
                None,
                &mut query_scratch_admission,
            )?;
        verify_boundary_source(&certified.committed, parent, &authority)?;
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
        VerificationReadError::Resource(_) | VerificationReadError::Deferred(_) => {
            QueryExecutionFail::GasBudgetExceeded
        }
        error => QueryExecutionFail::Conversion(error.to_string()),
    }
}

#[cfg(test)]
mod allocation_tests {
    use super::*;

    #[test]
    fn parent_service_verification_retains_original_allocation_release_owner() {
        let budget = iroha_allocation::AllocationBudget::new(1);
        let occupied = budget.try_reserve_bytes(1).unwrap();
        let refusal = budget.try_reserve_bytes(1).unwrap_err();
        let expected: ExecutionDeferred = refusal.clone().into();
        let error = parent_verification_source(VerificationReadError::Deferred(refusal.into()));
        assert!(matches!(error, ParentServiceError::Deferred(actual) if actual == expected));
        assert_eq!(budget.reserved_bytes(), 1);
        drop(occupied);
        let retry = budget
            .try_reserve_bytes(1)
            .expect("the original pool admits a retry after release");
        assert_eq!(budget.reserved_bytes(), 1);
        drop(retry);
        assert_eq!(budget.reserved_bytes(), 0);
    }

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

/// A local original-source refusal or an invalid consensus-carried parent proof.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ParentServiceError {
    #[error("invalid parent service proof: {0}")]
    Invalid(String),
    #[error("parent service original source is unavailable: {0}")]
    Source(QueryExecutionFail),
    #[error("parent service authentication deferred: {0}")]
    Deferred(crate::execution_attempt::ExecutionDeferred),
}

/// An authenticated, proposal-bound parent participation decision.
/// Only the original State-tip reader constructs it; it has no wire constructor.
pub(crate) struct VerifiedParentService {
    proposal: HashOf<IrohaHeader>,
    height: u64,
    timestamp_ms: u64,
    signers: Vec<iroha_model_base::peer::PeerId>,
}

impl VerifiedParentService {
    pub(crate) fn require_proposal(
        &self,
        proposal: &SignedBlock,
    ) -> Result<(), ParentServiceError> {
        if self.proposal != proposal.hash()
            || self.height.checked_add(1) != Some(proposal.header().height().get())
        {
            return Err(ParentServiceError::Invalid(
                "verified participation belongs to another proposal".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn height(&self) -> u64 {
        self.height
    }

    pub(crate) fn timestamp_ms(&self) -> u64 {
        self.timestamp_ms
    }

    pub(crate) fn signers(&self) -> &[iroha_model_base::peer::PeerId] {
        &self.signers
    }
}

fn parent_source_failure(error: impl std::fmt::Display) -> ParentServiceError {
    ParentServiceError::Source(QueryExecutionFail::Conversion(error.to_string()))
}

fn parent_proof_decode_failure(error: norito::Error) -> ParentServiceError {
    match crate::execution_attempt::norito_decode_attempt_error(error, |error| error.to_string()) {
        crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
            ParentServiceError::Deferred(reason)
        }
        crate::execution_attempt::ExecutionAttemptError::Rejected(reason) => {
            ParentServiceError::Invalid(reason)
        }
    }
}

fn parent_source_attempt(
    error: crate::execution_attempt::ExecutionAttemptError<QueryExecutionFail>,
) -> ParentServiceError {
    match error {
        crate::execution_attempt::ExecutionAttemptError::Rejected(error) => {
            ParentServiceError::Source(error)
        }
        crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
            ParentServiceError::Deferred(reason)
        }
    }
}

fn parent_verification_source(error: VerificationReadError) -> ParentServiceError {
    match error {
        VerificationReadError::Resource(error) => parent_proof_decode_failure(error.into()),
        VerificationReadError::Deferred(reason) => ParentServiceError::Deferred(reason),
        error => parent_source_failure(error),
    }
}

impl<'v, V: StateReadOnly + ?Sized> CertifiedChain<'v, V> {
    /// Signed genesis source and its resource refusal remain in the same attempt.
    pub(crate) fn new_for_parent_service(view: &'v V) -> Result<Self, ParentServiceError> {
        let genesis = view
            .canonical_history()
            .block_with_admission(NonZeroUsize::new(1).expect("genesis is nonzero"), |_, _| {
                Ok(())
            })
            .map_err(parent_source_attempt)?;
        Self::from_genesis(ChainSource::State(view), genesis).map_err(|error| {
            parent_source_attempt(
                error.map_rejection(|error| QueryExecutionFail::Conversion(error.to_string())),
            )
        })
    }

    fn executed_parent_pair(
        &self,
        target: NonZeroUsize,
        before_read: impl FnMut(u64, u64) -> Result<(), QueryExecutionFail>,
    ) -> Result<(CommittedBlock, CommittedBlock), ParentServiceError> {
        let predecessor = target
            .get()
            .checked_sub(1)
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| ParentServiceError::Invalid("genesis has no native quorum".into()))?;
        let ChainSource::State(view) = &self.source else {
            return Err(parent_source_failure(
                "parent proof requires original State authority",
            ));
        };
        let mut parent = None;
        let mut current = None;
        view.canonical_history()
            .visit_executed_backwards_until(predecessor, target, before_read, |receipt| {
                if receipt.height() == target.get() as u64 {
                    current = Some(receipt);
                } else {
                    parent = Some(receipt);
                }
                Ok(core::ops::ControlFlow::Continue(()))
            })
            .map_err(parent_source_attempt)?;
        let parent =
            parent.ok_or_else(|| parent_source_failure("authenticated predecessor absent"))?;
        let current =
            current.ok_or_else(|| parent_source_failure("authenticated parent absent"))?;
        Ok((parent, current))
    }

    /// Select one genuinely verified local certificate as a proposal input.
    /// Different valid subsets produce different next proposals; execution must
    /// authenticate the offered bytes and never reread this local certificate.
    pub(crate) fn parent_service_proposal_original(
        &self,
        parent: &SignedBlock,
    ) -> Result<Vec<u8>, ParentServiceError> {
        let height = usize::try_from(parent.header().height().get())
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| {
                ParentServiceError::Invalid("parent height is not addressable".into())
            })?;
        let (predecessor, original_parent) = self.executed_parent_pair(height, |_, _| Ok(()))?;
        let certified = self
            .verify_executed_successor(&predecessor, original_parent)
            .map_err(parent_verification_source)?;
        if certified.block_hash() != parent.hash() {
            return Err(ParentServiceError::Invalid(
                "assembly parent differs from original executed parent".into(),
            ));
        }
        let original = certified
            .certificate()
            .ok_or_else(|| parent_source_failure("executed parent has no local certificate"))?
            .commit_qc();
        if original.is_empty()
            || original.len() > iroha_data_model::consensus::PARENT_SERVICE_COMMIT_QC_MAX_BYTES
        {
            return Err(ParentServiceError::Invalid(
                "parent service original exceeds the protocol bound".into(),
            ));
        }
        norito::core::reserve_decode_allocation(original.len())
            .map_err(parent_proof_decode_failure)?;
        let mut owned = Vec::new();
        owned.try_reserve_exact(original.len()).map_err(|_| {
            ParentServiceError::Deferred(
                ivm::error::ExecutionDeferral::AllocationUnavailable.into(),
            )
        })?;
        owned.extend_from_slice(original);
        Ok(owned)
    }

    /// Authenticate the common parent participation input against the captured
    /// original execution tip. The offered QC supplies no authority or source.
    pub(crate) fn authenticate_parent_service(
        &self,
        proposal: &SignedBlock,
        before_read: impl FnMut(u64, u64) -> Result<(), QueryExecutionFail>,
    ) -> Result<Option<VerifiedParentService>, ParentServiceError> {
        proposal
            .validate_proposal_commitments()
            .map_err(|error| ParentServiceError::Invalid(error.to_string()))?;
        let height = proposal.header().height().get();
        let offered = proposal
            .npos_consensus_effects()
            .and_then(|effects| effects.parent_service_commit_qc.as_deref());
        if height <= 2 {
            if offered.is_some() {
                return Err(ParentServiceError::Invalid(
                    "genesis has no native parent service certificate".into(),
                ));
            }
            return Ok(None);
        }
        let offered = offered.ok_or_else(|| {
            ParentServiceError::Invalid("proposal omits its parent service original".into())
        })?;
        if offered.is_empty()
            || offered.len() > iroha_data_model::consensus::PARENT_SERVICE_COMMIT_QC_MAX_BYTES
        {
            return Err(ParentServiceError::Invalid(
                "parent service original exceeds the protocol bound".into(),
            ));
        }
        let target = usize::try_from(height - 1)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| {
                ParentServiceError::Invalid("parent height is not addressable".into())
            })?;
        let (parent, current) = self.executed_parent_pair(target, before_read)?;
        self.verify_parent_service_at(proposal, &parent, current, offered)
    }

    /// Private relation after an original-tip walk; tests retain genuine executed receipts.
    pub(super) fn verify_parent_service_at(
        &self,
        proposal: &SignedBlock,
        parent: &CommittedBlock,
        current: CommittedBlock,
        offered: &[u8],
    ) -> Result<Option<VerifiedParentService>, ParentServiceError> {
        if proposal.header().prev_block_hash() != Some(current.block_hash()) {
            return Err(ParentServiceError::Invalid(
                "proposal does not extend the authenticated parent".into(),
            ));
        }
        let qc: Qc = norito::decode_canonical(offered).map_err(parent_proof_decode_failure)?;
        let (authority, _config) = self
            .executed_successor_authority(parent, &current)
            .map_err(parent_verification_source)?;
        self.verification_context()
            .verify_commit_qc_original(&current, &authority, &qc)
            .map_err(|error| ParentServiceError::Invalid(error.to_string()))?;
        verify_boundary_source(&current, parent, &authority).map_err(parent_source_failure)?;
        let committee = &current.commitment.schedule.current.committee;
        let mut signers = Vec::new();
        let count = qc.signers.ones().count();
        let bytes = count
            .checked_mul(std::mem::size_of::<iroha_model_base::peer::PeerId>())
            .ok_or_else(|| ParentServiceError::Invalid("signer count overflow".into()))?;
        norito::core::reserve_decode_allocation(bytes).map_err(parent_proof_decode_failure)?;
        signers.try_reserve_exact(count).map_err(|_| {
            ParentServiceError::Deferred(
                ivm::error::ExecutionDeferral::AllocationUnavailable.into(),
            )
        })?;
        for index in qc.signers.ones() {
            let member = usize::try_from(index)
                .ok()
                .and_then(|index| committee.get(index))
                .ok_or_else(|| {
                    ParentServiceError::Invalid("service signer outside committee".into())
                })?;
            signers.push(member.validator.clone());
        }
        Ok(Some(VerifiedParentService {
            proposal: proposal.hash(),
            height: current.height(),
            timestamp_ms: current.block_time_ms(),
            signers,
        }))
    }
}
