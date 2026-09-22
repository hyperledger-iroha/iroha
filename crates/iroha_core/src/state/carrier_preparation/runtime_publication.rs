//! Acquire all four original runtime owners before publishing any component.

use super::*;
use mv::{
    PublicationPreparationError,
    cell::{PreparedPublication, PublishedPublication},
};
use std::convert::Infallible;

/// A local installation refusal which retains the complete original journals.
#[derive(Debug)]
pub(in crate::state::carrier_preparation::journals) enum RuntimePublicationError<E> {
    /// Complete installation capacity was refused before acquiring any writer.
    Admission(E),
    /// An original component is busy or lost its exact captured predecessor.
    Component {
        /// Exact original runtime field which refused preparation.
        field: &'static str,
        /// Local publication refusal, never a consensus validation result.
        cause: PublicationPreparationError<Infallible>,
    },
}

/// Four prepared current/undo pairs with every original writer retained.
///
/// The enclosing State publisher must own joint visibility and exact finality,
/// and prepare its other components before consuming this owner.
#[must_use = "runtime preparation must remain owned until publication or abort"]
pub(in crate::state::carrier_preparation::journals) struct PreparedRuntimeJournals<
    'target,
    Admission,
    Installation,
> {
    original: Option<AcquiredRuntimeJournals<'target, Admission, Installation>>,
}

// One typed participant inventory defines the exclusive acquiring/prepared phases.
macro_rules! define_runtime_publication_components {
    ($($field:ident: ($($ty:ty),+)),+ $(,)?) => {
        struct AcquiredRuntimeJournals<'target, Admission, Installation> {

            $($field: PreparedPublication<'target, $($ty,)+ (), ()>,)+
            admission: Admission,
            installation: Installation,
        }
        struct RuntimePublicationComponents<'target, Admission> {

            $($field: mv::cell::DetachedPublicationSlot<'target, $($ty,)+ (), ()>,)+
            // Retain the original reservation after every component's cleanup.
            admission: Option<Admission>,
        }
        impl<'target, Admission> RuntimePublicationComponents<'target, Admission> {
            fn new(original: RuntimeJournals<Admission>, target: &'target State) -> Self {
                let RuntimeJournals {  $($field,)+ admission } = original;
                // Every move below is inert: no readiness probe, admission or payload clone.
                Self {  $($field: $field.publication_slot(&target.$field),)+ admission: Some(admission) }
            }
            fn try_prepare<E>(&mut self) -> Result<(), RuntimePublicationError<E>> {
                $(self.$field.try_prepare(|_, _| Ok::<_, Infallible>(()))
                    .map_err(|cause| RuntimePublicationError::Component { field: stringify!($field), cause })?;)+
                Ok(())
            }
            fn release_writers(&mut self) { $(self.$field.release_writers();)+ }
            fn recover_original(&mut self) -> RuntimeJournals<Admission> {
                // Each lower method unlocks and returns its original journal; actual cleanup stays in its slot.
                RuntimeJournals {  $($field: self.$field.recover_original(),)+
                    admission: self.admission.take().expect("original capture admission"), }
            }
            fn into_prepared<Installation>(self, installation: Installation) -> PreparedRuntimeJournals<'target, Admission, Installation> {
                PreparedRuntimeJournals { original: Some(AcquiredRuntimeJournals {
                    $($field: self.$field.into_prepared(),)+
                    admission: self.admission.expect("original capture admission"), installation,
                }) }
            }
            fn into_cleanup(self) -> [Option<mv::PublicationCleanup<()>>; 4] {
                assert!(self.admission.is_none(), "original journals must first be recovered");
                [$(Some(self.$field.into_cleanup()),)+]
            }
        }
    };
}
define_runtime_publication_components! {
    canonical_runtime: (SnapshotNexusRuntime),
    commit_topology: (Vec<PeerId>),
    prev_commit_topology: (Vec<PeerId>),
    lane_consensus_contexts: (LaneConsensusContextsV1),
}

/// Released original runtime components and their resource owners.
/// Collector and wake callbacks remain deferred until the enclosing State unlocks.
pub(in crate::state::carrier_preparation::journals) struct PublishedRuntimeJournals<A, I> {
    _canonical_runtime: PublishedPublication<SnapshotNexusRuntime, (), ()>,
    _commit_topology: PublishedPublication<Vec<PeerId>, (), ()>,
    _prev_commit_topology: PublishedPublication<Vec<PeerId>, (), ()>,
    _lane_consensus_contexts: PublishedPublication<LaneConsensusContextsV1, (), ()>,
    _admission: A,
    _installation: I,
}

/// Original aborted runtime notifications and their installation reservation.
pub(in crate::state::carrier_preparation::journals) struct AbortedRuntimeJournals<I> {
    _components: [Option<mv::PublicationCleanup<()>>; 4],
    _installation: Option<I>,
}

enum RuntimePublicationPhase<'target, Admission> {
    Original(RuntimeJournals<Admission>),
    Components(RuntimePublicationComponents<'target, Admission>),
}

/// Caller-owned original participant group, installed before any admission or readiness work.
/// Normal refusal recovers exact journals while keeping every original deferred release here.
/// Caught unwind and terminal release allow cleanup only; this grants no State authority.
#[must_use = "retain this slot until every enclosing physical writer releases"]
pub(in crate::state::carrier_preparation::journals) struct RuntimePublicationSlot<
    'target,
    Admission,
    Installation,
> {
    target: &'target State,
    phase: Option<RuntimePublicationPhase<'target, Admission>>,
    attempted: bool,
    complete: bool,
    retryable: bool,
    released: bool,
    recovered: bool,
    // Must follow the complete original phase so reservation cleanup cannot precede it.
    installation: Option<Installation>,
}

impl<Admission> RuntimeJournals<Admission> {
    /// Inertly retain the original aggregate before any callback or physical acquisition.
    pub(in crate::state::carrier_preparation::journals) fn publication_slot<Installation>(
        self,
        target: &State,
    ) -> RuntimePublicationSlot<'_, Admission, Installation> {
        RuntimePublicationSlot {
            target,
            phase: Some(RuntimePublicationPhase::Original(self)),
            attempted: false,
            complete: false,
            retryable: true,
            released: false,
            recovered: false,
            installation: None,
        }
    }

    /// Standalone preparation delegates to the same caller-owned slot engine.
    /// An enclosing aggregate must retain its slot before calling the borrowed method
    /// if its own physical siblings also need to survive a callee unwind.
    pub(in crate::state::carrier_preparation::journals) fn try_prepare_publication<
        'target,
        Installation,
        E,
    >(
        self,
        target: &'target State,
        admit: impl FnOnce(&Self, &State) -> Result<Installation, E>,
    ) -> Result<
        PreparedRuntimeJournals<'target, Admission, Installation>,
        (
            Self,
            RuntimePublicationError<E>,
            AbortedRuntimeJournals<Installation>,
        ),
    > {
        let mut slot = self.publication_slot(target);
        match slot.try_prepare(admit) {
            Ok(()) => Ok(slot.into_prepared()),
            Err(error) => {
                let original = slot.recover_original();
                Err((original, error, slot.into_cleanup()))
            }
        }
    }
}

impl<'target, Admission, Installation> RuntimePublicationSlot<'target, Admission, Installation> {
    /// Borrow the original aggregate once, with every partial native owner remaining in this slot.
    pub(in crate::state::carrier_preparation::journals) fn try_prepare<E>(
        &mut self,
        admit: impl FnOnce(&RuntimeJournals<Admission>, &State) -> Result<Installation, E>,
    ) -> Result<(), RuntimePublicationError<E>> {
        assert!(
            !self.attempted && !self.released,
            "original aggregate preparation is one-shot"
        );
        self.attempted = true;
        self.retryable = false;
        let result = self.prepare_inner(admit);
        // A caught callee panic never reaches this normal-return recovery grant.
        self.retryable = true;
        self.complete = result.is_ok();
        result
    }
    fn prepare_inner<E>(
        &mut self,
        admit: impl FnOnce(&RuntimeJournals<Admission>, &State) -> Result<Installation, E>,
    ) -> Result<(), RuntimePublicationError<E>> {
        let Some(RuntimePublicationPhase::Original(original)) = &self.phase else {
            unreachable!("original group before preparation")
        };
        self.installation =
            Some(admit(original, self.target).map_err(RuntimePublicationError::Admission)?);
        let Some(RuntimePublicationPhase::Original(original)) = self.phase.take() else {
            unreachable!()
        };
        self.phase = Some(RuntimePublicationPhase::Components(
            RuntimePublicationComponents::new(original, self.target),
        ));
        let Some(RuntimePublicationPhase::Components(components)) = &mut self.phase else {
            unreachable!()
        };
        components.try_prepare()
    }
    /// Terminal physical-only pass. Payloads, admission and all callbacks remain caller-owned.
    pub(in crate::state::carrier_preparation::journals) fn release_writers(&mut self) {
        self.released = true;
        self.retryable = false;
        self.complete = false;
        if let Some(RuntimePublicationPhase::Components(components)) = &mut self.phase {
            components.release_writers();
        }
    }
    /// Return the same original journals after normal refusal or complete prepared abort.
    /// The empty lower slots keep actual release events until this aggregate is retired.
    pub(in crate::state::carrier_preparation::journals) fn recover_original(
        &mut self,
    ) -> RuntimeJournals<Admission> {
        assert!(
            self.retryable && !self.released,
            "unwound/released group grants no journal"
        );
        self.released = true;
        self.complete = false;
        self.recovered = true;
        match self.phase.as_mut().expect("original participant phase") {
            RuntimePublicationPhase::Components(components) => components.recover_original(),
            RuntimePublicationPhase::Original(_) => {
                let Some(RuntimePublicationPhase::Original(original)) = self.phase.take() else {
                    unreachable!()
                };
                original
            }
        }
    }
    /// Transfer actual released cleanup only after original journals have been recovered.
    pub(in crate::state::carrier_preparation::journals) fn into_cleanup(
        mut self,
    ) -> AbortedRuntimeJournals<Installation> {
        assert!(
            self.released && self.recovered,
            "normal original recovery required"
        );
        let components = match self.phase.take() {
            Some(RuntimePublicationPhase::Components(components)) => components.into_cleanup(),
            None => std::array::from_fn(|_| None),
            Some(RuntimePublicationPhase::Original(_)) => {
                unreachable!("original journal was recovered")
            }
        };
        AbortedRuntimeJournals {
            _components: components,
            _installation: self.installation.take(),
        }
    }
    /// Inertly transfer only a fully prepared original group to its existing publisher.
    pub(in crate::state::carrier_preparation::journals) fn into_prepared(
        mut self,
    ) -> PreparedRuntimeJournals<'target, Admission, Installation> {
        assert!(
            self.complete && !self.released,
            "all original components prepared"
        );
        self.released = true;
        let Some(RuntimePublicationPhase::Components(components)) = self.phase.take() else {
            unreachable!()
        };
        components.into_prepared(
            self.installation
                .take()
                .expect("original installation admission"),
        )
    }
}
impl<Admission, Installation> Drop for RuntimePublicationSlot<'_, Admission, Installation> {
    fn drop(&mut self) {
        self.release_writers();
    }
}

impl<Admission, Installation> AcquiredRuntimeJournals<'_, Admission, Installation> {
    /// Release every writer and return the original journals and capture guard.
    pub(in crate::state::carrier_preparation::journals) fn abort(
        self,
    ) -> (
        RuntimeJournals<Admission>,
        AbortedRuntimeJournals<Installation>,
    ) {
        let Self {
            canonical_runtime,
            commit_topology,
            prev_commit_topology,
            lane_consensus_contexts,
            admission,
            installation,
        } = self;
        let canonical_runtime = canonical_runtime.abort();
        let commit_topology = commit_topology.abort();
        let prev_commit_topology = prev_commit_topology.abort();
        let lane_consensus_contexts = lane_consensus_contexts.abort();
        let retirement = AbortedRuntimeJournals {
            _components: [
                Some(canonical_runtime.1),
                Some(commit_topology.1),
                Some(prev_commit_topology.1),
                Some(lane_consensus_contexts.1),
            ],
            _installation: Some(installation),
        };
        (
            RuntimeJournals {
                canonical_runtime: canonical_runtime.0,
                commit_topology: commit_topology.0,
                prev_commit_topology: prev_commit_topology.0,
                lane_consensus_contexts: lane_consensus_contexts.0,
                admission,
            },
            retirement,
        )
    }

    /// Publish each prepared pair once and return both actual resource guards.
    ///
    /// The caller must hold the complete State publication and finality owner.
    /// This component does not supply aggregate atomic visibility by itself.
    pub(in crate::state::carrier_preparation::journals) fn publish(
        self,
    ) -> PublishedRuntimeJournals<Admission, Installation> {
        let Self {
            canonical_runtime,
            commit_topology,
            prev_commit_topology,
            lane_consensus_contexts,
            admission,
            installation,
        } = self;
        PublishedRuntimeJournals {
            _canonical_runtime: canonical_runtime.publish(),
            _commit_topology: commit_topology.publish(),
            _prev_commit_topology: prev_commit_topology.publish(),
            _lane_consensus_contexts: lane_consensus_contexts.publish(),
            _admission: admission,
            _installation: installation,
        }
    }
}

impl<Admission, Installation> PreparedRuntimeJournals<'_, Admission, Installation> {
    /// Release every physical component before returning the original journals.
    pub(in crate::state::carrier_preparation::journals) fn abort(
        mut self,
    ) -> (
        RuntimeJournals<Admission>,
        AbortedRuntimeJournals<Installation>,
    ) {
        self.original
            .take()
            .expect("original prepared runtime")
            .abort()
    }

    /// Consume the same original components under the enclosing State authority.
    pub(in crate::state::carrier_preparation::journals) fn publish(
        mut self,
    ) -> PublishedRuntimeJournals<Admission, Installation> {
        self.original
            .take()
            .expect("original prepared runtime")
            .publish()
    }
}

impl<Admission, Installation> Drop for PreparedRuntimeJournals<'_, Admission, Installation> {
    fn drop(&mut self) {
        let Some(original) = self.original.take() else {
            return;
        };
        // Declare capacity owners first so they also outlive payload/notification
        // cleanup if a callback itself unwinds. Release every writer before any
        // original journal or deferred notification can be destroyed.
        let admission;
        let installation;
        let AcquiredRuntimeJournals {
            canonical_runtime,
            commit_topology,
            prev_commit_topology,
            lane_consensus_contexts,
            admission: retained_admission,
            installation: retained_installation,
        } = original;
        admission = retained_admission;
        installation = retained_installation;
        let canonical_runtime = canonical_runtime.abort();
        let commit_topology = commit_topology.abort();
        let prev_commit_topology = prev_commit_topology.abort();
        let lane_consensus_contexts = lane_consensus_contexts.abort();
        drop((
            canonical_runtime,
            commit_topology,
            prev_commit_topology,
            lane_consensus_contexts,
        ));
        drop((admission, installation));
    }
}
