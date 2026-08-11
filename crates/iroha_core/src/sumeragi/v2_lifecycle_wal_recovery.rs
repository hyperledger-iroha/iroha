//! Sealed restart join for WAL-ahead Validate vote continuations.

use super::{
    CandidateAdmission, CapacityClass, DurablePayloadReference, DurableValidateReplayEvidenceV1,
    InitialLifecycleState, LifecycleStageKind, LifecycleWorkClass, PredecessorScope,
    body_pipeline_transition::{
        durable_continuation_successor_is_exact, durable_validate_payload_is_exact,
    },
    ledger::{AuthenticatedRecoveredWalValidateLedgerParent, DurableWalVoteLedgerRepairReceipt},
    projection,
    schema::DurableContinuationEdge,
};
use crate::sumeragi::{
    v2::{AdapterEffect, VerifiedHeightContext},
    v2_body_store::{DurableBodyReceipt, ValidatedBodyReceipt},
    v2_runtime::{
        RecoveredWalCandidateProjectionPermit, RecoveredWalVoteProjectionFailure,
        RecoveredWalVoteSuccessor,
    },
};

/// Why one recovered WAL vote could not join its exact Validate predecessor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecoveredWalVoteLifecycleRepairErrorKind {
    ParentProjection,
    ChildProjection,
    InvalidWalIdentity,
    InvalidReplayEvidence,
    InvalidParent,
    InvalidChild,
    ForeignOwner,
    ForeignLineage,
}

/// Drop-safe failure which returns every move-only recovery input.
///
/// The caller may retry after rebuilding the surrounding startup cut. No
/// ledger, coordinator, registry, adapter, or WAL state is changed while this
/// value is produced.
#[must_use = "failed WAL lifecycle recovery retains all move-only inputs"]
pub(super) struct RecoveredWalVoteLifecycleRepairError {
    kind: RecoveredWalVoteLifecycleRepairErrorKind,
    _retained: RecoveredWalVoteLifecycleRepairRetained,
}

enum RecoveredWalVoteLifecycleRepairRetained {
    Successor {
        _successor: RecoveredWalVoteSuccessor,
    },
    Projection {
        _projection: AuthenticatedRecoveredWalVoteProjection,
    },
}

/// Consuming projection retaining the recovered successor beside both candidates.
///
/// Construction requires a runtime-private permit and the wrapper has no
/// parts API outside this lifecycle-repair module.
#[must_use = "a recovered WAL candidate projection must enter lifecycle repair"]
pub(in crate::sumeragi) struct AuthenticatedRecoveredWalVoteProjection {
    successor: RecoveredWalVoteSuccessor,
    parent: CandidateAdmission,
    child: CandidateAdmission,
}

impl AuthenticatedRecoveredWalVoteProjection {
    /// Assemble the one successful result of the consuming runtime projection.
    pub(in crate::sumeragi) const fn from_runtime_projection(
        _permit: RecoveredWalCandidateProjectionPermit,
        successor: RecoveredWalVoteSuccessor,
        parent: CandidateAdmission,
        child: CandidateAdmission,
    ) -> Self {
        Self {
            successor,
            parent,
            child,
        }
    }

    const fn parent(&self) -> &CandidateAdmission {
        &self.parent
    }

    const fn child(&self) -> &CandidateAdmission {
        &self.child
    }

    fn concrete_pair_is_exact(&self) -> bool {
        self.successor.replay_evidence_is_exact() && self.successor.concrete_pair_is_exact()
    }

    fn concrete_pair_matches_validation(&self, validated: &ValidatedBodyReceipt) -> bool {
        self.successor.concrete_pair_matches_validation(validated)
    }

    const fn installed_child_effect(&self) -> &AdapterEffect {
        self.successor.installed_child_effect()
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredWalVoteLifecycleRepairError {
    /// Return a stable diagnostic classification without exposing authority.
    pub(super) const fn reason(&self) -> &'static str {
        match self.kind {
            RecoveredWalVoteLifecycleRepairErrorKind::ParentProjection => {
                "recovered Validate projection failed"
            }
            RecoveredWalVoteLifecycleRepairErrorKind::ChildProjection => {
                "recovered Sign projection failed"
            }
            RecoveredWalVoteLifecycleRepairErrorKind::InvalidWalIdentity => {
                "recovered WAL identity is inconsistent"
            }
            RecoveredWalVoteLifecycleRepairErrorKind::InvalidReplayEvidence => {
                "recovered WAL replay evidence is inconsistent"
            }
            RecoveredWalVoteLifecycleRepairErrorKind::InvalidParent => {
                "recovered Validate parent is invalid"
            }
            RecoveredWalVoteLifecycleRepairErrorKind::InvalidChild => {
                "recovered Sign child is invalid"
            }
            RecoveredWalVoteLifecycleRepairErrorKind::ForeignOwner => {
                "recovered WAL continuation changed causal owner"
            }
            RecoveredWalVoteLifecycleRepairErrorKind::ForeignLineage => {
                "recovered WAL continuation changed body lineage"
            }
        }
    }
}

/// Authenticated, move-only WAL-ahead parent/child lifecycle repair.
///
/// Both logical candidates are projected from sealed runtime bindings. The
/// child binding was itself minted only by consuming an adapter-authenticated
/// final WAL vote seal, including the full PrepareQC for a recovered Commit.
/// This value is inert: it exposes no ledger persistence, coordinator
/// mutation, registry installation, or adapter commit surface.
#[must_use = "an authenticated WAL lifecycle repair has not been staged or published"]
pub(super) struct AuthenticatedWalVoteLifecycleRepair {
    projection: AuthenticatedRecoveredWalVoteProjection,
    edge: DurableContinuationEdge,
}

/// Post-fsync WAL recovery authority bound to one exact LedgerV1 replacement.
///
/// The token still retains the concrete Validate parent and Sign successor.
/// It exposes no effect/binding extraction or registry mutation; the future
/// startup transaction must consume it directly into the exact child address.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "a durable WAL repair still owns its concrete lifecycle handoff"]
pub(super) struct DurableAuthenticatedWalVoteLifecycleRepair {
    repair: AuthenticatedWalVoteLifecycleRepair,
    receipt: DurableWalVoteLedgerRepairReceipt,
}

#[cfg_attr(not(test), allow(dead_code))]
impl AuthenticatedWalVoteLifecycleRepair {
    /// Borrow the exact recovered Validate admission projection.
    pub(super) const fn parent(&self) -> &CandidateAdmission {
        self.projection.parent()
    }

    /// Borrow the exact recovered Sign admission projection.
    pub(super) const fn child(&self) -> &CandidateAdmission {
        self.projection.child()
    }

    /// Return the typed durable Validate-to-Sign continuation edge.
    pub(super) const fn edge(&self) -> DurableContinuationEdge {
        self.edge
    }

    /// Revalidate both retained concrete effects against their sealed bindings.
    pub(super) fn concrete_pair_is_exact(&self) -> bool {
        self.projection.concrete_pair_is_exact()
    }

    /// Return whether one durable validation is the exact outcome carried by
    /// this concrete Validate-to-Sign recovery pair.
    ///
    /// This equality oracle deliberately exposes neither concrete effect nor
    /// pending binding. The registry recovery token uses it to keep the body
    /// receipt tied to the authenticated WAL vote after detaching the parent
    /// row.
    pub(super) fn concrete_pair_matches_validation(
        &self,
        validated: &ValidatedBodyReceipt,
    ) -> bool {
        let active_context = super::LifecycleContext::new(
            self.parent().key.context(),
            self.parent().key.round().height(),
        );
        let expected_payload =
            projection::durable_body_frame_reference(active_context, validated.durable())
                .map(DurablePayloadReference::BodyFrame);
        self.concrete_pair_is_exact()
            && Some(self.parent().payload) == expected_payload
            && self.projection.concrete_pair_matches_validation(validated)
    }

    /// Bind this move-only repair to the exact post-fsync ledger receipt.
    #[allow(clippy::result_large_err)]
    pub(super) fn bind_durable_ledger_receipt(
        self,
        receipt: DurableWalVoteLedgerRepairReceipt,
    ) -> Result<DurableAuthenticatedWalVoteLifecycleRepair, (Self, DurableWalVoteLedgerRepairReceipt)>
    {
        if !receipt.matches(&self) {
            return Err((self, receipt));
        }
        Ok(DurableAuthenticatedWalVoteLifecycleRepair {
            repair: self,
            receipt,
        })
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl DurableAuthenticatedWalVoteLifecycleRepair {
    /// Return the durable Sign child ordinal.
    pub(super) const fn child_ordinal(&self) -> u128 {
        self.receipt.child_ordinal()
    }

    /// Return the hash of the complete fsynced LedgerV1 frame.
    pub(super) const fn ledger_frame_hash(&self) -> super::LifecycleDigest {
        self.receipt.ledger_frame_hash()
    }

    /// Borrow the authenticated repair for idempotent post-fsync verification.
    pub(super) const fn repair(&self) -> &AuthenticatedWalVoteLifecycleRepair {
        &self.repair
    }

    /// Borrow only the recovered Sign effect retained by this durable repair.
    ///
    /// This narrow view exists solely so the closed concrete-registry carrier
    /// can satisfy the registry's non-consuming effect-borrow contract. It
    /// exposes neither pending binding nor a consuming effect/authority pair.
    pub(super) const fn installed_child_effect(&self) -> &AdapterEffect {
        self.repair.projection.installed_child_effect()
    }

    /// Bind this authority to one frame already loaded from the exact store.
    pub(super) fn belongs_to_loaded(
        &self,
        store: &super::ledger::LifecycleLedgerStoreV1,
        ledger: &super::ledger::LifecycleLedgerV1,
    ) -> bool {
        self.receipt.belongs_to_loaded(store, ledger)
    }
}

/// Join one recovered Validate binding to the exact vote continuation already
/// authenticated from the final WAL frame.
///
/// Every check is read-only. Success consumes all move-only inputs into one
/// opaque recovery value; failure returns those inputs unchanged.
#[allow(clippy::result_large_err)]
pub(super) fn authenticate_recovered_wal_vote_lifecycle_from_ledger_parent(
    verified: &VerifiedHeightContext,
    parent: &AuthenticatedRecoveredWalValidateLedgerParent,
    successor: RecoveredWalVoteSuccessor,
) -> Result<AuthenticatedWalVoteLifecycleRepair, RecoveredWalVoteLifecycleRepairError> {
    authenticate_recovered_wal_vote_lifecycle(
        verified,
        RecoveredValidatePayloadAuthority::Ledger(parent),
        successor,
    )
}

/// Join one recovered Validate binding to the exact durable body retained by
/// its installed completion carrier.
#[allow(clippy::result_large_err)]
pub(super) fn authenticate_recovered_wal_vote_lifecycle_from_durable_body(
    verified: &VerifiedHeightContext,
    durable: &DurableBodyReceipt,
    replay_evidence: &DurableValidateReplayEvidenceV1,
    successor: RecoveredWalVoteSuccessor,
) -> Result<AuthenticatedWalVoteLifecycleRepair, RecoveredWalVoteLifecycleRepairError> {
    authenticate_recovered_wal_vote_lifecycle(
        verified,
        RecoveredValidatePayloadAuthority::Durable {
            receipt: durable,
            replay_evidence,
        },
        successor,
    )
}

#[allow(variant_size_differences)]
enum RecoveredValidatePayloadAuthority<'a> {
    Ledger(&'a AuthenticatedRecoveredWalValidateLedgerParent),
    Durable {
        receipt: &'a DurableBodyReceipt,
        replay_evidence: &'a DurableValidateReplayEvidenceV1,
    },
}

#[allow(clippy::result_large_err)]
fn authenticate_recovered_wal_vote_lifecycle(
    verified: &VerifiedHeightContext,
    parent_payload: RecoveredValidatePayloadAuthority<'_>,
    successor: RecoveredWalVoteSuccessor,
) -> Result<AuthenticatedWalVoteLifecycleRepair, RecoveredWalVoteLifecycleRepairError> {
    let projected = match parent_payload {
        RecoveredValidatePayloadAuthority::Ledger(parent) => {
            successor.into_ledger_lifecycle_projection(verified, parent)
        }
        RecoveredValidatePayloadAuthority::Durable {
            receipt,
            replay_evidence,
        } => successor.into_durable_lifecycle_projection(verified, receipt, replay_evidence),
    };
    let projection = match projected {
        Ok(projection) => projection,
        Err(failure) => {
            let (kind, successor) = match failure {
                RecoveredWalVoteProjectionFailure::InvalidWalIdentity(successor) => (
                    RecoveredWalVoteLifecycleRepairErrorKind::InvalidWalIdentity,
                    successor,
                ),
                RecoveredWalVoteProjectionFailure::InvalidReplayEvidence(successor) => (
                    RecoveredWalVoteLifecycleRepairErrorKind::InvalidReplayEvidence,
                    successor,
                ),
                RecoveredWalVoteProjectionFailure::Parent(successor) => (
                    RecoveredWalVoteLifecycleRepairErrorKind::ParentProjection,
                    successor,
                ),
                RecoveredWalVoteProjectionFailure::Child(successor) => (
                    RecoveredWalVoteLifecycleRepairErrorKind::ChildProjection,
                    successor,
                ),
            };
            return Err(RecoveredWalVoteLifecycleRepairError {
                kind,
                _retained: RecoveredWalVoteLifecycleRepairRetained::Successor {
                    _successor: successor,
                },
            });
        }
    };

    let structural = (|| {
        let parent = projection.parent();
        let child = projection.child();
        if !candidate_shape_is_exact(parent, LifecycleWorkClass::Validate)
            || parent.key.phase() != super::LifecyclePhase::Validate
            || parent.stage.kind() != LifecycleStageKind::ValidateBody
        {
            Err(RecoveredWalVoteLifecycleRepairErrorKind::InvalidParent)
        } else {
            let edge = match (child.key.phase(), child.stage.kind()) {
                (super::LifecyclePhase::Prepare, LifecycleStageKind::SignPrepareVote) => {
                    Some(DurableContinuationEdge::ValidateToSignPrepare)
                }
                (super::LifecyclePhase::Commit, LifecycleStageKind::SignCommitVote) => {
                    Some(DurableContinuationEdge::ValidateToSignCommit)
                }
                _ => None,
            }
            .ok_or(RecoveredWalVoteLifecycleRepairErrorKind::InvalidChild)?;
            if !candidate_shape_is_exact(child, LifecycleWorkClass::SignVote) {
                Err(RecoveredWalVoteLifecycleRepairErrorKind::InvalidChild)
            } else if parent.causal_root != child.causal_root
                || parent.reconstruction_source != child.reconstruction_source
            {
                Err(RecoveredWalVoteLifecycleRepairErrorKind::ForeignOwner)
            } else if !durable_continuation_successor_is_exact(
                edge,
                parent.work_class,
                parent.key,
                parent.stage,
                child.work_class,
                child.key,
                child.stage,
            ) {
                Err(RecoveredWalVoteLifecycleRepairErrorKind::ForeignLineage)
            } else {
                Ok(edge)
            }
        }
    })();
    match structural {
        Ok(edge) => Ok(AuthenticatedWalVoteLifecycleRepair { projection, edge }),
        Err(kind) => Err(RecoveredWalVoteLifecycleRepairError {
            kind,
            _retained: RecoveredWalVoteLifecycleRepairRetained::Projection {
                _projection: projection,
            },
        }),
    }
}

fn candidate_shape_is_exact(
    candidate: &CandidateAdmission,
    expected_work_class: LifecycleWorkClass,
) -> bool {
    let canonical = candidate.physical_geometry.canonicalized();
    let normalized = candidate.physical_geometry.normalized();
    let payload_is_exact = match expected_work_class {
        LifecycleWorkClass::Validate => {
            durable_validate_payload_is_exact(candidate.key, candidate.payload)
        }
        LifecycleWorkClass::SignVote => candidate.payload == DurablePayloadReference::None,
        _ => false,
    };
    candidate.work_class == expected_work_class
        && candidate.stage.predecessor_scope() == PredecessorScope::Independent
        && candidate.initial_state == InitialLifecycleState::Ready
        && candidate.causal_root.digest() == candidate.reconstruction_source
        && payload_is_exact
        && candidate.producer_turn.is_none()
        && matches!(
            (canonical, normalized),
            (Ok(canonical), Ok((physical, universe, consumed)))
                if canonical == candidate.physical_geometry
                    && physical.len() == 1
                    && universe.len() == 1
                    && consumed == universe
                    && physical.keys().all(|slot| {
                        slot.capacity_class() == Some(CapacityClass::Effect)
                    })
        )
}
