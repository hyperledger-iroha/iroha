//! Acquire all four original runtime owners before publishing any component.

use super::*;
use mv::{PublicationPreparationError, cell::PreparedPublication};
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
    canonical_runtime: PreparedPublication<'target, SnapshotNexusRuntime, (), ()>,
    commit_topology: PreparedPublication<'target, Vec<PeerId>, (), ()>,
    prev_commit_topology: PreparedPublication<'target, Vec<PeerId>, (), ()>,
    lane_consensus_contexts: PreparedPublication<'target, LaneConsensusContextsV1, (), ()>,
    // Fields drop in declaration order: both guards outlive every writer/value.
    admission: Admission,
    installation: Installation,
}

macro_rules! prepare_components {
    ($target:ident, $admission:ident, $installation:ident;
        [$($done:ident,)*]; [$next:ident, $($rest:ident,)*]) => {{
        let $next = match $next.try_prepare_publication(&$target.$next, |_, _| Ok::<_, Infallible>(())) {
            Ok(prepared) => prepared,
            Err(($next, cause)) => {
                $(let $done = $done.abort();)*
                drop($installation);
                return Err((RuntimeJournals {
                    $($done,)* $next, $($rest,)* admission: $admission,
                }, RuntimePublicationError::Component { field: stringify!($next), cause }));
            }
        };
        prepare_components!($target, $admission, $installation;
            [$($done,)* $next,]; [$($rest,)*])
    }};
    ($target:ident, $admission:ident, $installation:ident; [$($done:ident,)*]; []) => {
        Ok(PreparedRuntimeJournals { $($done,)* admission: $admission, installation: $installation })
    };
}

impl<Admission> RuntimeJournals<Admission> {
    /// Admit the complete installation before acquiring any original writer.
    ///
    /// Admission must cover all four current/undo COW and staging copies,
    /// publication identities and retained-reader installation peaks. Every
    /// refusal returns the same typed journals and capture reservation after
    /// aborting earlier acquisitions. This grants no State/finality authority.
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
        (Self, RuntimePublicationError<E>),
    > {
        let installation = match admit(&self, target) {
            Ok(installation) => installation,
            Err(error) => return Err((self, RuntimePublicationError::Admission(error))),
        };
        let Self {
            canonical_runtime,
            commit_topology,
            prev_commit_topology,
            lane_consensus_contexts,
            admission,
        } = self;
        prepare_components!(target, admission, installation; []; [
            canonical_runtime, commit_topology, prev_commit_topology, lane_consensus_contexts,
        ])
    }
}

impl<Admission, Installation> PreparedRuntimeJournals<'_, Admission, Installation> {
    /// Release every writer and return the original journals and capture guard.
    pub(in crate::state::carrier_preparation::journals) fn abort(
        self,
    ) -> RuntimeJournals<Admission> {
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
        drop(installation);
        RuntimeJournals {
            canonical_runtime,
            commit_topology,
            prev_commit_topology,
            lane_consensus_contexts,
            admission,
        }
    }

    /// Publish each prepared pair once and return both actual resource guards.
    ///
    /// The caller must hold the complete State publication and finality owner.
    /// This component does not supply aggregate atomic visibility by itself.
    pub(in crate::state::carrier_preparation::journals) fn publish(
        self,
    ) -> (Admission, Installation) {
        let Self {
            canonical_runtime,
            commit_topology,
            prev_commit_topology,
            lane_consensus_contexts,
            admission,
            installation,
        } = self;
        canonical_runtime.publish();
        commit_topology.publish();
        prev_commit_topology.publish();
        lane_consensus_contexts.publish();
        (admission, installation)
    }
}
