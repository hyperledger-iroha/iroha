//! One bounded candidate worker; every completion returns its original source waits.

use super::native_process::NativeRunnerProcess;
use super::*;
use crate::sumeragi::v2_candidate::{
    CandidateAttachments, CandidateError, NativeCandidateAssembly,
};
use std::{sync::mpsc, thread};

pub(super) struct NativeCandidateJob {
    owner: LocalProposalOwner,
    receive: mpsc::Receiver<Result<NativeCandidateAssembly, CandidateError>>,
    worker: thread::JoinHandle<()>,
}

impl NativeCandidateJob {
    pub(super) fn join(self) -> Result<(), V2RunnerError> {
        // Keep the one result slot alive until the worker stops; no result send
        // blocks and no original source/candidate gets detached from shutdown.
        let joined = self.worker.join();
        drop(self.receive);
        joined.map_err(|_| {
            V2RunnerError::Service("Native candidate worker panicked during shutdown".into())
        })
    }
}

pub(super) struct NativeCandidateResult {
    owner: LocalProposalOwner,
    result: Result<NativeCandidateAssembly, CandidateError>,
}

enum OwnedCandidateParent {
    Block(SignedBlock),
    Snapshot(wire::SnapshotBootstrapAnchor),
}
impl OwnedCandidateParent {
    fn borrow(&self) -> CandidateParent<'_> {
        match self {
            Self::Block(block) => CandidateParent::Block(block),
            Self::Snapshot(anchor) => CandidateParent::Snapshot(anchor),
        }
    }
}

impl NativeRunnerProcess {
    /// Nonblocking completion observation. Join only after the physical worker exited.
    pub(super) fn poll_candidate(&mut self) -> Result<(), V2RunnerError> {
        if self.candidate_result.is_some() {
            return Ok(());
        }
        if !self
            .candidate_job
            .as_ref()
            .is_some_and(|job| job.worker.is_finished())
        {
            return Ok(());
        }
        let job = self
            .candidate_job
            .take()
            .expect("finished candidate worker");
        let result = job.receive.try_recv().map_err(|error| {
            V2RunnerError::Service(format!(
                "Native candidate worker lost original result: {error}"
            ))
        });
        if job.worker.join().is_err() {
            return Err(V2RunnerError::Service(
                "Native candidate worker panicked".into(),
            ));
        }
        self.candidate_result = Some(NativeCandidateResult {
            owner: job.owner,
            result: result?,
        });
        Ok(())
    }

    /// Start or take exactly one original candidate; disk preparation and signing
    /// run off the serialized control thread with the process output guard.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn assemble_candidate(
        &mut self,
        owner: LocalProposalOwner,
        assembler: V2CandidateAssembler,
        context: &wire::HeightContext,
        directive: LocalProposalDirective,
        local_validator: wire::ValidatorIndex,
        parent: CandidateParent<'_>,
        queue: &Arc<Queue>,
        attachments: CandidateAttachments,
    ) -> Result<Option<Result<NativeCandidateAssembly, CandidateError>>, V2RunnerError> {
        self.poll_candidate()?;
        if let Some(completed) = self.candidate_result.take() {
            if completed.owner == owner {
                return Ok(Some(completed.result));
            }
            // Stale signed candidates release only their queue lease. Their exact
            // source waits remain independently owned by this Native process.
            if let Ok(assembly) = completed.result {
                self.retain_candidate_source(assembly.source);
            }
        }
        if self.candidate_job.is_some() {
            return Ok(None);
        }
        let Some(decisions) = self.capture_decisions()? else {
            return Ok(None);
        };
        let decisions = decisions.with_recovered_sources(self.recovered_sources.clone());
        let state = Arc::clone(&self.state);
        let guard = Arc::clone(&self.guard);
        let key = self.key.clone();
        let queue = Arc::clone(queue);
        let context = context.clone();
        let parent = match parent {
            CandidateParent::Block(block) => OwnedCandidateParent::Block(block.clone()),
            CandidateParent::Snapshot(anchor) => OwnedCandidateParent::Snapshot(anchor.clone()),
        };
        let (send, receive) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("sumeragi-native-candidate".into())
            .spawn(move || {
                let result = assembler.assemble_native(CandidateRequest {
                    context: &context,
                    directive,
                    local_validator,
                    parent: parent.borrow(),
                    state: &state,
                    queue: &queue,
                    key_pair: &key,
                    output_guard: &guard,
                    attachments,
                    work_provider: &decisions,
                });
                // At most one candidate is outstanding. Disconnect means runner
                // teardown; the process guard must already forbid canonical output.
                if send.send(result).is_err() {
                    guard.close_admission_for_restart();
                }
            })
            .map_err(|error| V2RunnerError::Service(error.to_string()))?;
        self.candidate_job = Some(NativeCandidateJob {
            owner,
            receive,
            worker,
        });
        Ok(None)
    }

    /// Always retain source custody before interpreting assembly refusal or delivery.
    pub(super) fn retain_candidate_source(
        &mut self,
        source: super::super::v2_lane_driver::NativeLaneCandidatePreparation,
    ) {
        self.candidate_source = Some(source);
    }

    pub(super) fn candidate_source_requirement(
        &self,
    ) -> Option<Arc<crate::state::AuthenticatedLaneAdmittedInputSourceV1>> {
        let completed = self
            .candidate_result
            .as_ref()
            .and_then(|completed| completed.result.as_ref().ok())
            .map(|assembly| &assembly.source);
        self.candidate_source
            .as_ref()
            .into_iter()
            .chain(completed)
            .find_map(|source| {
                source.waits.iter().find_map(|wait| match wait {
                    crate::state::LaneDecisionGroupPreparationV1::CanonicalBodyRecoveryRequired(
                        source,
                    ) if !self.recovered_sources.values().any(|recovered| {
                        recovered.carrier_hash() == source.carrier_hash()
                            && recovered.priority() == source.priority()
                    }) =>
                    {
                        Some(Arc::new(source.clone()))
                    }
                    _ => None,
                })
            })
    }

    /// Candidate results retain historical waits, but only currently open
    /// lanes may occupy the process's one network recovery slot. The caller
    /// must first confirm this complete observation is still current.
    pub(super) fn prune_closed_candidate_source_waits(
        &mut self,
        observed: &crate::state::VerifiedLaneContexts,
    ) {
        let retain_current = |wait: &crate::state::LaneDecisionGroupPreparationV1| {
            !matches!(wait, crate::state::LaneDecisionGroupPreparationV1::CanonicalBodyRecoveryRequired(source)
                if !source.is_current_in(observed))
        };
        if let Some(source) = self.candidate_source.as_mut() {
            source.waits.retain(retain_current);
        }
        if let Some(Ok(assembly)) = self
            .candidate_result
            .as_mut()
            .map(|completed| &mut completed.result)
        {
            assembly.source.waits.retain(retain_current);
        }
    }

    #[cfg(all(test, feature = "bls"))]
    pub(crate) fn assert_candidate_source_pruning_for_test(
        state: Arc<State>,
        source: Arc<crate::state::AuthenticatedLaneAdmittedInputSourceV1>,
        current: &crate::state::VerifiedLaneContexts,
        closed: &crate::state::VerifiedLaneContexts,
        key: &KeyPair,
    ) {
        let mut process = Self::new(
            state,
            ConsensusOutputGuard::isolated(),
            PeerId::new(key.public_key().clone()),
            key.clone(),
            true,
            &crate::sumeragi::v2::SumeragiV2Adapter::native_source_lifecycle_config_for_test(),
            32 * 1024 * 1024,
            Duration::from_secs(10),
            Duration::from_secs(1),
        )
        .expect("actual Native candidate source owner");
        process.candidate_source = Some(
            super::super::v2_lane_driver::NativeLaneCandidatePreparation {
                work: None,
                waits: vec![
                    crate::state::LaneDecisionGroupPreparationV1::CanonicalBodyRecoveryRequired(
                        source.as_ref().clone(),
                    ),
                ],
            },
        );
        assert!(process.candidate_source_requirement().is_some());
        process.prune_closed_candidate_source_waits(current);
        assert!(process.candidate_source_requirement().is_some());
        process.prune_closed_candidate_source_waits(closed);
        assert!(process.candidate_source_requirement().is_none());
        process.shutdown().join().unwrap();
    }
}
