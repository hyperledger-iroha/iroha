//! Original successor-history preparation retained outside disposable World overlays.
//!
//! The same canonical acquisition, receipt verifier and signed-genesis authenticator serve
//! both the header check and pristine schedule capture. Current State and source membership
//! are rechecked whenever the completed preparation is lent; it is not imported authority.
//! TODO: ordinary nested result/epoch graphs and source-metadata scratch retain their existing
//! funding boundaries. This owner retains the actual admitted frame/decoder/receipt prefixes.

use super::*;
use crate::state::{CanonicalHistorySource, NativeExecutionTip};
use iroha_allocation::AllocationBudget;
use iroha_data_model::{
    block::SharedSignedBlock,
    consensus::GlobalThresholdBeaconPulseContextV1,
    sumeragi_finality::{OriginalGenesisRead, OriginalGenesisReadError},
};

type Attempt<T> = Result<T, ExecutionAttemptError<schedule::ScheduleError>>;

/// Pending original history of one native successor, shared by its two authentication sites.
/// No source or completed owner is replaced after an admission or physical refusal.
#[derive(Default)]
pub(crate) struct SuccessorContextPreparation {
    parent: Option<Parent>,
    genesis_authentication: Option<OriginalGenesisRead>,
    genesis: Option<Carrier>,
    authenticated: Option<(Hash32, SumeragiRootScope)>,
    original: Option<Original>,
    continuation: schedule::continuation::ContinuationPreparation,
}

struct Original {
    kura: Arc<Kura>,
    budget: AllocationBudget,
    tip: NativeExecutionTip,
    network: NetworkId,
    chain: [u8; iroha_model_base::chain::MAX_CHAIN_ID_BYTES],
    chain_len: usize,
    genesis: HashOf<IrohaHeader>,
}

struct Parent {
    frame: AdmittedFrameRead,
    complete: bool,
    carrier: Carrier,
}

struct Carrier {
    body: Option<SharedSignedBlock>,
    acquisition: NativeCarrierAcquisition<'static>,
}

fn source(
    error: ExecutionAttemptError<ChainReadError>,
) -> ExecutionAttemptError<schedule::ScheduleError> {
    error.map_rejection(schedule::ScheduleError::from)
}

fn unavailable(height: u64) -> ExecutionAttemptError<schedule::ScheduleError> {
    schedule::ScheduleError::from(ChainReadError::NotInView { height }).into()
}

fn authentication_error(error: OriginalGenesisReadError) -> ExecutionAttemptError<ChainReadError> {
    match error {
        OriginalGenesisReadError::Validation(original) => {
            crate::execution_attempt::genesis_read_attempt_error(original, |_| {
                ChainReadError::ForeignGenesis
            })
        }
        OriginalGenesisReadError::Allocation(original) => {
            ExecutionAttemptError::Deferred(retained_query_authority::buffer_refusal(original))
        }
        OriginalGenesisReadError::ForeignPool => {
            ExecutionAttemptError::Deferred(retained_query_authority::invariant())
        }
    }
}

impl Original {
    fn capture(view: &(impl StateReadOnly + ?Sized)) -> Attempt<Self> {
        let height = view.height() as u64;
        let tip = view
            .native_execution_tip()
            .ok_or_else(|| unavailable(height))?;
        if tip.height() != height
            || view.block_hashes().get(view.height().saturating_sub(1)) != Some(&tip.iroha_hash())
        {
            return Err(unavailable(height));
        }
        let genesis = *view.block_hashes().get(0).ok_or_else(|| unavailable(1))?;
        let chain_bytes = view.chain_id().as_str().as_bytes();
        let mut chain = [0; iroha_model_base::chain::MAX_CHAIN_ID_BYTES];
        chain[..chain_bytes.len()].copy_from_slice(chain_bytes);
        Ok(Self {
            kura: view.kura_handle(),
            budget: view.execution_budget(),
            tip,
            network: *view.network_id(),
            chain,
            chain_len: chain_bytes.len(),
            genesis,
        })
    }

    fn require_view(&self, view: &(impl StateReadOnly + ?Sized)) -> Attempt<()> {
        if !Arc::ptr_eq(&self.kura, &view.kura_handle())
            || !self.budget.same_pool(&view.execution_budget())
            || view.native_execution_tip() != Some(self.tip)
            || usize::try_from(self.tip.height()).ok() != Some(view.height())
            || view.block_hashes().get(view.height().saturating_sub(1))
                != Some(&self.tip.iroha_hash())
            || view.block_hashes().get(0) != Some(&self.genesis)
            || *view.network_id() != self.network
            || view.chain_id().as_str().as_bytes() != &self.chain[..self.chain_len]
        {
            return Err(unavailable(self.tip.height()));
        }
        Ok(())
    }
}

impl Carrier {
    fn new(original: &Original, height: u64, hash: HashOf<IrohaHeader>) -> Attempt<Self> {
        let index = usize::try_from(height)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| unavailable(height))?;
        Ok(Self {
            body: None,
            acquisition: NativeCarrierAcquisition::new_owned(
                Arc::clone(&original.kura),
                index,
                hash,
                original.budget.clone(),
                usize::MAX,
            ),
        })
    }

    fn complete(&mut self) -> Attempt<&SharedSignedBlock> {
        if self.body.is_none() {
            self.body = Some(self.acquisition.complete().map_err(source)?);
        } else {
            self.recheck()?;
        }
        Ok(self.body.as_ref().expect("original carrier completed"))
    }

    fn recheck(&self) -> Attempt<()> {
        self.acquisition.recheck_original_source().map_err(source)
    }
}

impl SuccessorContextPreparation {
    /// Complete original parent and signed-genesis work, then check the current proposal context.
    /// Every local refusal leaves successful physical stages attached to this same attempt.
    pub(crate) fn authenticate(
        &mut self,
        state: &(impl StateReadOnly + ?Sized),
        header: &IrohaHeader,
        expected: &GlobalThresholdBeaconPulseContextV1,
    ) -> Attempt<()> {
        schedule::require_successor_cut(state, header)?;
        let budget = self.original.as_ref().map_or_else(
            || state.execution_budget(),
            |original| original.budget.clone(),
        );
        budget.with_deferred_refund_notifications(|_| {
            self.prepare(state)?;
            self.recheck(state, header, expected)
        })
    }

    fn prepare(&mut self, state: &(impl StateReadOnly + ?Sized)) -> Attempt<()> {
        if self.original.is_none() {
            self.original = Some(Original::capture(state)?);
        }
        let original = self.original.as_ref().expect("original State cut captured");
        original.require_view(state)?;
        let height = original.tip.height();
        if self.parent.is_none() {
            self.parent = Some(Parent {
                frame: AdmittedFrameRead::default(),
                complete: false,
                carrier: Carrier::new(original, height, original.tip.iroha_hash())?,
            });
        }
        let parent = self.parent.as_mut().expect("original parent acquisition");
        let body = parent.carrier.complete()?;
        if !parent.complete {
            parent
                .frame
                .initialize(
                    body.clone(),
                    height,
                    original.tip.root_scope(),
                    &mut EpochValidationScope::new(),
                    &original.budget,
                )
                .map_err(source)?;
            require_parent_link(parent.frame.committed(), original.tip)?;
            parent.complete = true;
        }
        // At the genesis successor, authenticate the signed body independently of its
        // execution receipt, but retain the identical raw frame and shared body allocation.
        let genesis = if height == GENESIS_HEIGHT {
            parent.frame.committed().block().clone()
        } else {
            if self.genesis.is_none() {
                self.genesis = Some(Carrier::new(original, GENESIS_HEIGHT, original.genesis)?);
            }
            self.genesis
                .as_mut()
                .expect("original signed genesis acquisition")
                .complete()?
                .clone()
        };
        if self.genesis_authentication.is_none() {
            self.genesis_authentication = Some(
                OriginalGenesisRead::new(genesis.clone(), &original.budget)
                    .map_err(authentication_error)
                    .map_err(source)?,
            );
        }
        if self.authenticated.is_none() {
            let authentication = self
                .genesis_authentication
                .as_mut()
                .expect("original genesis policy owner");
            let (_, instance, scope) =
                authenticate_genesis_with(&genesis, state.network_id(), state.chain_id(), || {
                    authentication.authenticate().map_err(authentication_error)
                })
                .map_err(source)?;
            // SPEC: HC139 restores the unnecessary executed-genesis ancestry walk on
            // the native retained path as well as on the standalone context reader.
            #[cfg(all(test, sumeragi_core_mutation = "HC139"))]
            let instance = {
                let _ = instance;
                let executed = committed_block(state, GENESIS_HEIGHT).map_err(source)?;
                super::super::node::root_instance(executed.block(), &state.chain_id().to_string())
                    .map_err(|error| error.map_rejection(schedule::ScheduleError::Epoch))?
            };
            self.authenticated = Some((instance, scope));
        }
        Ok(())
    }

    /// Prepare ordinary schedule custody before World acquisition, within the same observed
    /// State generation as native header authentication. This grants no cached State authority.
    pub(crate) fn prepare_continuation(
        &mut self,
        state: &(impl StateReadOnly + ?Sized),
        header: &IrohaHeader,
        supplied: Option<iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1>,
        expected: GlobalThresholdBeaconPulseContextV1,
    ) -> Attempt<()> {
        self.recheck(state, header, &expected)?;
        self.continuation.prepare(state, header, supplied, expected)
    }

    /// Borrow the exact ordinary capture after the pristine request repeated successor and
    /// pulse authentication. The lease returns its unchanged nested graph on World rollback.
    pub(crate) fn lend_continuation(
        &self,
        state: &(impl StateReadOnly + ?Sized),
        header: &IrohaHeader,
        scope: SumeragiRootScope,
        pulse: &crate::sumeragi::epoch_beacon::VerifiedEpochPulse,
    ) -> Attempt<schedule::continuation::ContinuationLease> {
        self.continuation.lend(state, header, scope, pulse)
    }

    /// Lend completed original history only after the current State and physical slots recheck.
    /// This repeats current epoch/context comparisons without allocating replacement frames.
    pub(crate) fn recheck(
        &self,
        state: &(impl StateReadOnly + ?Sized),
        header: &IrohaHeader,
        expected: &GlobalThresholdBeaconPulseContextV1,
    ) -> Attempt<()> {
        schedule::require_successor_cut(state, header)?;
        let original = self
            .original
            .as_ref()
            .ok_or_else(|| unavailable(header.height().get() - 1))?;
        original.require_view(state)?;
        let parent = self
            .parent
            .as_ref()
            .filter(|parent| parent.complete)
            .ok_or_else(|| unavailable(original.tip.height()))?;
        parent.carrier.recheck()?;
        if original.tip.height() > GENESIS_HEIGHT {
            self.genesis
                .as_ref()
                .ok_or_else(|| unavailable(GENESIS_HEIGHT))?
                .recheck()?;
        }
        require_parent_link(parent.frame.committed(), original.tip)?;
        let (instance, scope) = self
            .authenticated
            .ok_or_else(|| unavailable(GENESIS_HEIGHT))?;
        if scope != original.tip.root_scope() {
            return Err(source(ChainReadError::ForeignGenesis.into()));
        }
        schedule::compare_successor_context(
            state,
            header,
            expected,
            parent.frame.committed(),
            instance,
        )
    }
}

fn require_parent_link(parent: &CommittedBlock, tip: NativeExecutionTip) -> Attempt<()> {
    CanonicalHistorySource::require_native_execution_link(parent, tip.core_hash(), tip.result())
        .map_err(|error| {
            source(error.map_rejection(|error| ChainReadError::Malformed {
                height: tip.height(),
                reason: error.to_string(),
            }))
        })
}

#[cfg(test)]
mod tests;
