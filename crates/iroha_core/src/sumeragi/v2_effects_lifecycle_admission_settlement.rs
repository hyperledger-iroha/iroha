/// Exhaustive source inventory of effects which may create pending work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingWorkProducer {
    Sign,
    Fetch,
    Store,
    Validate,
    Apply,
    Output,
}

/// Closed result of servicing exactly one attested post-Apply direct Broadcast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum ProductionApplyTerminalDirectBroadcastSettlementV1 {
    /// The exact output was accepted and its lifecycle row became terminal.
    Completed,
    /// The output source retained the occurrence and the same owner remains parked.
    SourceRetained,
}

/// Exact signed-Proposal replay owner retained beside the ordinary body pipeline.
///
/// The stage changes only after the corresponding runtime or body-store cut
/// commits. No variant exposes replay evidence, pending bindings, or a caller-
/// selected lifecycle address.
#[allow(variant_size_differences)]
enum RemoteProposalReplayStageV1 {
    Fetch {
        work_id: EffectWorkId,
        replay: PreparedRemoteProposalFetchReplayPreAdmission,
    },
    BodyAvailable(PreparedRemoteProposalFetchReplayPreAdmission),
    StoreAdmission(PreparedRemoteProposalStoreReplayPreAdmission),
    Store {
        work_id: EffectWorkId,
        replay: PreparedRemoteProposalStoreReplayPreAdmission,
    },
    Stored {
        replay: PreparedRemoteProposalStoredReplayPreAdmission,
        ownership: RuntimeEffectOwnership,
    },
}

impl RemoteProposalReplayStageV1 {
    /// Recheck one stale ordinary Fetch against the already-installed signed origin.
    fn exactly_authenticates_fetch_rediscovery(&self, effect: &AdapterEffect) -> bool {
        match self {
            Self::Fetch { replay, .. } | Self::BodyAvailable(replay) => {
                replay.exactly_authenticates_fetch_rediscovery(effect)
            }
            Self::StoreAdmission(replay) | Self::Store { replay, .. } => {
                replay.exactly_authenticates_fetch_rediscovery(effect)
            }
            Self::Stored { replay, .. } => replay.exactly_authenticates_fetch_rediscovery(effect),
        }
    }
}

/// Exact authenticated-genesis replay owner retained beside the certified body pipeline.
///
/// Unlike an ordinary certified response, this source is already local and
/// launch-authenticated. The stage still advances through the same Store and
/// Validate ownership cuts, and no variant exposes its replay evidence or a
/// caller-selected lifecycle address.
#[allow(variant_size_differences)]
enum AuthenticatedGenesisReplayStageV1 {
    BodyAvailable(PreparedAuthenticatedGenesisFetchReplayPreAdmission),
    StoreAdmission(PreparedAuthenticatedGenesisStoreReplayPreAdmission),
    Store {
        work_id: EffectWorkId,
        replay: PreparedAuthenticatedGenesisStoreReplayPreAdmission,
    },
    Stored {
        replay: PreparedAuthenticatedGenesisStoredReplayPreAdmission,
        ownership: RuntimeEffectOwnership,
    },
}

impl AuthenticatedGenesisReplayStageV1 {
    fn store_work_id(&self) -> Option<EffectWorkId> {
        match self {
            Self::Store { work_id, .. } => Some(*work_id),
            Self::BodyAvailable(_) | Self::StoreAdmission(_) | Self::Stored { .. } => None,
        }
    }

    fn exactly_authenticates_fetch_rediscovery(&self, effect: &AdapterEffect) -> bool {
        match self {
            Self::BodyAvailable(replay) => replay.exactly_authenticates_fetch_rediscovery(effect),
            Self::StoreAdmission(replay) | Self::Store { replay, .. } => {
                replay.exactly_authenticates_fetch_rediscovery(effect)
            }
            Self::Stored { replay, .. } => replay.exactly_authenticates_fetch_rediscovery(effect),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AuthenticatedGenesisStoreReplayDispositionV1 {
    None,
    Advance,
    Retry(RuntimeEffectOwnership),
}

/// Inert runtime fingerprint for one replay-authorized Validate after its
/// move-only admission owner transfers into the lifecycle registry.
#[derive(Clone, Debug, PartialEq, Eq)]
enum DurableValidateRetrySealV1 {
    /// Live admission retains the original executable runtime owner so later
    /// authority refinement can preserve its one physical lifecycle root.
    Live {
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
        store_terminal: Option<DurableStoreTerminalRetrySealV1>,
        lifecycle_ordinal: Option<u128>,
    },
    /// Cold recovery retains a registry-authenticated inert owner. The `Arc`
    /// permits transactional executor snapshots without making the move-only
    /// registry projection itself cloneable or externally reusable.
    Recovered {
        owner: Arc<RecoveredDurableValidateRetryOwnerV1>,
        frontier: RecoveredDurableValidateRetryFrontierV1,
        lifecycle_ordinal: Option<u128>,
    },
}

struct DurableValidateRetryProjectionV1 {
    seal: DurableValidateRetrySealV1,
    ownership: RuntimeEffectOwnership,
}

impl DurableValidateRetrySealV1 {
    fn seal_exact(
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
        pending: &PendingDurableValidateAdmissionV1,
        store_terminal: Option<DurableStoreTerminalRetrySealV1>,
    ) -> Option<Self> {
        matches!(effect, AdapterEffect::ValidateBody { .. })
            .then_some(())
            .filter(|()| {
                pending.exactly_matches_retry(effect, ownership)
                    && store_terminal
                        .as_ref()
                        .is_none_or(|store| store.exactly_precedes_validate(effect))
            })?;
        Some(Self::Live {
            effect: effect.clone(),
            ownership: ownership.clone(),
            store_terminal,
            lifecycle_ordinal: None,
        })
    }

    /// Return the exact logical row still owned by this retry authority.
    const fn lifecycle_ordinal(&self) -> Option<u128> {
        match self {
            Self::Live {
                lifecycle_ordinal, ..
            }
            | Self::Recovered {
                lifecycle_ordinal, ..
            } => *lifecycle_ordinal,
        }
    }

    /// Bind a just-committed or recovered registry row without permitting a
    /// retry to switch logical ownership.
    fn bind_lifecycle_ordinal(&mut self, ordinal: u128) -> Result<(), String> {
        if ordinal == 0 {
            return Err("durable Validate retry received a zero lifecycle ordinal".to_owned());
        }
        let lifecycle_ordinal = match self {
            Self::Live {
                lifecycle_ordinal, ..
            }
            | Self::Recovered {
                lifecycle_ordinal, ..
            } => lifecycle_ordinal,
        };
        match *lifecycle_ordinal {
            Some(existing) if existing != ordinal => {
                Err("durable Validate retry changed its exact lifecycle ordinal".to_owned())
            }
            Some(_) => Ok(()),
            None => {
                *lifecycle_ordinal = Some(ordinal);
                Ok(())
            }
        }
    }

    /// Convert the exact resolved row into an inert retransmit tombstone.
    fn release_lifecycle_ordinal(&mut self, ordinal: u128) -> Result<(), String> {
        if self.lifecycle_ordinal() != Some(ordinal) {
            return Err("resolved durable Validate changed its exact lifecycle ordinal".to_owned());
        }
        match self {
            Self::Live {
                lifecycle_ordinal, ..
            }
            | Self::Recovered {
                lifecycle_ordinal, ..
            } => *lifecycle_ordinal = None,
        }
        Ok(())
    }

    fn project_retry(
        &self,
        effect: &AdapterEffect,
        incoming: &RuntimeEffectOwnership,
    ) -> Result<DurableValidateRetryProjectionV1, String> {
        match self {
            Self::Live {
                effect: incumbent_effect,
                ownership: incumbent_ownership,
                store_terminal,
                lifecycle_ordinal,
            } => {
                let (
                    AdapterEffect::ValidateBody {
                        tag: incumbent_tag,
                        round: incumbent_round,
                        subject: incumbent_subject,
                    },
                    AdapterEffect::ValidateBody {
                        tag: incoming_tag,
                        round: incoming_round,
                        subject: incoming_subject,
                    },
                ) = (incumbent_effect, effect)
                else {
                    return Err(
                        "durable Validate retry seal received another effect stage".to_owned()
                    );
                };
                if (incoming_tag != incumbent_tag
                    && !incoming_tag.strictly_advances(*incumbent_tag))
                    || incoming_round != incumbent_round
                    || incoming_subject != incumbent_subject
                    || incumbent_ownership
                        .exact_pending_adapter_effect_binding(incumbent_effect)
                        .is_err()
                    || store_terminal
                        .as_ref()
                        .is_some_and(|store| !store.exactly_precedes_validate(effect))
                {
                    return Err(
                        "durable Validate retry changed its exact body, tag, or incumbent owner"
                            .to_owned(),
                    );
                }
                let ownership = incumbent_ownership
                    .adopt_incumbent_body_stage_for_retry_or_authority(incoming, effect)?;
                ownership
                    .exact_pending_adapter_effect_binding(effect)
                    .map_err(|_| {
                        "durable Validate retry lost its exact adopted owner".to_owned()
                    })?;
                Ok(DurableValidateRetryProjectionV1 {
                    seal: Self::Live {
                        effect: effect.clone(),
                        ownership: ownership.clone(),
                        store_terminal: store_terminal.clone(),
                        lifecycle_ordinal: *lifecycle_ordinal,
                    },
                    ownership,
                })
            }
            Self::Recovered {
                owner,
                frontier,
                lifecycle_ordinal,
            } => {
                let (frontier, ownership) =
                    owner.exactly_matches_retry(frontier, effect, incoming)?;
                Ok(DurableValidateRetryProjectionV1 {
                    seal: Self::Recovered {
                        owner: Arc::clone(owner),
                        frontier,
                        lifecycle_ordinal: *lifecycle_ordinal,
                    },
                    ownership,
                })
            }
        }
    }

    /// Return whether a resolved ordinary Validate tombstone must yield to
    /// one exact newer-view protected-Prepare admission.
    ///
    /// An ordinal-bound seal still has a concrete registry row which can
    /// absorb authority refinement. Once that row has terminalized, however,
    /// its ordinal-free tombstone owns neither service work nor a completion
    /// carrier. It therefore cannot satisfy a later protected view's first
    /// Prepare-authorized `ValidationCompleted` transition. This predicate is
    /// deliberately closed over the live ordinary-to-Prepare upgrade; cold
    /// owners, published direct-lifecycle markers, Commit upgrades, and
    /// same/stale retries keep their existing stutter policy.
    fn is_unbound_live_ordinary_to_prepare_upgrade(
        &self,
        projected: &DurableValidateRetryProjectionV1,
    ) -> bool {
        let (
            Self::Live {
                effect: incumbent_effect,
                ownership: incumbent_ownership,
                lifecycle_ordinal: None,
                ..
            },
            Self::Live {
                effect: projected_effect,
                ownership: projected_ownership,
                lifecycle_ordinal: None,
                ..
            },
        ) = (self, &projected.seal)
        else {
            return false;
        };
        let (
            AdapterEffect::ValidateBody {
                tag: incumbent_tag, ..
            },
            AdapterEffect::ValidateBody {
                tag: projected_tag, ..
            },
        ) = (incumbent_effect, projected_effect)
        else {
            return false;
        };
        if !projected_tag.strictly_advances(*incumbent_tag) {
            return false;
        }
        let Some(incumbent_statement) = incumbent_ownership
            .exact_pending_adapter_effect_binding(incumbent_effect)
            .ok()
            .and_then(|pending| pending.candidate_statement())
        else {
            return false;
        };
        let Some(projected_statement) = projected_ownership
            .exact_pending_adapter_effect_binding(projected_effect)
            .ok()
            .and_then(|pending| pending.candidate_statement())
        else {
            return false;
        };
        incumbent_statement.phase().is_none()
            && incumbent_statement.execution_commitment().is_none()
            && projected_statement.phase() == Some(wire::GlobalPhase::Prepare)
            && projected_statement.execution_commitment().is_some()
            && incumbent_statement.body_stage_authority_relation_to(projected_statement)
                == Some(RuntimeFetchAuthorityRelation::Upgrade)
    }

    /// Project one late durable Store carrier through the inert predecessor
    /// owner retained when this live Validate admission consumed its replay.
    fn project_store_terminal_retry(
        &self,
        durable_receipt: &DurableBodyReceipt,
        effect: &AdapterEffect,
        incoming: &RuntimeEffectOwnership,
    ) -> Result<Option<RuntimeEffectOwnership>, String> {
        let Self::Live {
            store_terminal: Some(store_terminal),
            ..
        } = self
        else {
            return Ok(None);
        };
        store_terminal
            .project_retry_ownership(durable_receipt, effect, incoming)
            .map(Some)
            .ok_or_else(|| {
                "post-Validate Store terminal changed its durable body or exact owner".to_owned()
            })
    }

    /// Project a durable commitment join without mutating the current seal.
    fn project_recovered_commitment_ceiling(
        &self,
        commitment: wire::ExecutionCommitment,
    ) -> Result<Option<Self>, String> {
        match self {
            Self::Live { .. } => Ok(None),
            Self::Recovered {
                owner,
                frontier,
                lifecycle_ordinal,
            } => frontier
                .project_commitment_ceiling(commitment)
                .map(|frontier| {
                    Some(Self::Recovered {
                        owner: Arc::clone(owner),
                        frontier,
                        lifecycle_ordinal: *lifecycle_ordinal,
                    })
                })
                .map_err(str::to_owned),
        }
    }
}

/// Atomic executor-side sink for one complete recovered Validate retry census.
///
/// The opaque registry census alone can obtain owners and feeds every one into
/// this preflight. No executor state changes until [`Self::commit`] consumes
/// the complete prepared map.
pub(in crate::sumeragi) struct PreparedRecoveredDurableValidateRetryInstallV1<'a, R> {
    executor: &'a mut V2EffectExecutor<R>,
    runtime_decision: Option<(
        wire::ConsensusRound,
        wire::ConsensusRound,
        wire::BlockSubject,
        wire::ExecutionCommitment,
    )>,
    prepared: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableValidateRetrySealV1>,
}

impl<R: EffectRuntime> PreparedRecoveredDurableValidateRetryInstallV1<'_, R> {
    /// Absorb one owner projected by the private complete registry census.
    pub(in crate::sumeragi) fn absorb(
        &mut self,
        owner: RecoveredDurableValidateRetryOwnerV1,
    ) -> Result<(), EffectExecutorError> {
        let key = owner.key();
        let validation_marker_is_exact = match (
            self.executor.validated_bodies.get(&key),
            self.executor.rejected_bodies.get(&key),
        ) {
            (None, None) => true,
            (Some(validated), None) => owner.exactly_matches_validated_marker(key, validated),
            (None, Some(rejected)) => rejected == owner.durable_receipt(),
            (Some(_), Some(_)) => false,
        };
        if owner
            .expected_decision()
            .is_some_and(|decision| self.runtime_decision != Some(decision))
            || self.executor.durable_bodies.get(&key) != Some(owner.durable_receipt())
            || !validation_marker_is_exact
            || self.executor.retired_rejected_bodies.contains_key(&key)
            || self
                .prepared
                .insert(
                    key,
                    DurableValidateRetrySealV1::Recovered {
                        frontier: owner.initial_retry_frontier().ok_or_else(|| {
                            EffectExecutorError::Contract(
                                "cold Validate retry owner omitted its initial frontier".to_owned(),
                            )
                        })?,
                        lifecycle_ordinal: Some(owner.lifecycle_ordinal()),
                        owner: Arc::new(owner),
                    },
                )
                .is_some()
        {
            return Err(EffectExecutorError::Contract(
                "cold Validate retry owner disagreed with runtime or body-store recovery"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    /// Publish the fully preflighted census in one map replacement.
    pub(in crate::sumeragi) fn commit(self) -> Result<(), EffectExecutorError> {
        self.executor.durable_validate_retry_seals = self.prepared;
        Ok(())
    }
}

/// Inert exact Store owner retained while a direct lifecycle Store row is
/// active and later nested as the predecessor of its published Validate row.
///
/// The opaque pending binding has no runtime ordinal and cannot execute. It
/// proves that an exact durable Store stage is already lifecycle-owned,
/// allowing a later compatible Store carrier to stutter before it can query a
/// queued `BodyStored` terminal under a fresh lifecycle owner.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PublishedLifecycleStoreTerminalRetrySealV1 {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectFingerprintV1,
    /// Strongest same-body authority observed since the immutable Store row
    /// crossed publication. The exact published statement remains sealed in
    /// `pending` for Store-to-Validate reverse projection.
    statement: RuntimeCandidateSemanticStatement,
    durable_receipt: DurableBodyReceipt,
}

/// Fully preflighted disposition for one physical Store completion which may
/// arrive after the same body stage crossed direct lifecycle publication.
enum PublishedLifecycleStoreCompletionPlanV1 {
    /// No direct lifecycle row owns this Store terminal.
    NoPublishedMarker,
    /// The active Store row absorbs the completion and commits this monotonic
    /// comparison-only authority overlay.
    ActiveStore(PublishedLifecycleStoreTerminalRetrySealV1),
    /// The successor Validate row already owns the Store terminal. Its marker
    /// is immutable, so successful preflight carries no replacement value.
    PublishedValidate,
}

impl PublishedLifecycleStoreCompletionPlanV1 {
    const fn coalesces_terminal(&self) -> bool {
        !matches!(self, Self::NoPublishedMarker)
    }
}

/// Comparison-only immutable publication identity for one executable
/// lifecycle Store row.
///
/// Finalized rollover compares this value against the authenticated lifecycle
/// registry before retiring all remaining rows. The monotonic in-process
/// authority overlay is intentionally absent: only the exact published Store
/// effect, its ordinal-free pending fingerprint, and durable receipt identify
/// the registry owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct PublishedLifecycleStoreRetryCensusEntryV1 {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectFingerprintV1,
    durable_receipt: DurableBodyReceipt,
}

impl PublishedLifecycleStoreTerminalRetrySealV1 {
    fn seal_published_store(
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
        durable_receipt: &DurableBodyReceipt,
    ) -> Option<Self> {
        let AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        } = effect
        else {
            return None;
        };
        let statement = pending.candidate_statement()?;
        let pending = pending.published_store_retry_fingerprint(effect)?;
        if tag.height() != round.height
            || durable_receipt.context_id() != round.context_id
            || durable_receipt.round() != *round
            || durable_receipt.subject() != *subject
            || !pending.exactly_binds_adapter_effect(effect)
            || statement.context_id() != round.context_id
            || statement.proposal_round() != *round
            || statement.subject() != Some(*subject)
        {
            return None;
        }
        Some(Self {
            effect: effect.clone(),
            pending,
            statement,
            durable_receipt: durable_receipt.clone(),
        })
    }

    fn seal_exact(
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
        durable_receipt: &DurableBodyReceipt,
    ) -> Option<Self> {
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = validate_effect
        else {
            return None;
        };
        let effect = AdapterEffect::StoreBody {
            tag: *tag,
            round: *round,
            subject: *subject,
        };
        let pending =
            validate_pending.project_validate_store_predecessor(validate_effect, &effect)?;
        if pending
            .project_store_validate_successor(&effect, validate_effect)
            .as_ref()
            != Some(validate_pending)
        {
            return None;
        }
        Self::seal_published_store(&effect, &pending, durable_receipt)
    }

    fn validates(&self) -> bool {
        let AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        } = &self.effect
        else {
            return false;
        };
        tag.height() == round.height
            && self.durable_receipt.context_id() == round.context_id
            && self.durable_receipt.round() == *round
            && self.durable_receipt.subject() == *subject
            && self.pending.exactly_binds_adapter_effect(&self.effect)
            && self.pending.candidate_statement().is_some_and(|published| {
                matches!(
                    published.body_stage_authority_relation_to(self.statement),
                    Some(
                        RuntimeFetchAuthorityRelation::Same
                            | RuntimeFetchAuthorityRelation::Upgrade
                    )
                )
            })
    }

    fn publication_census_entry(&self) -> Option<PublishedLifecycleStoreRetryCensusEntryV1> {
        self.validates()
            .then(|| PublishedLifecycleStoreRetryCensusEntryV1 {
                effect: self.effect.clone(),
                pending: self.pending.clone(),
                durable_receipt: self.durable_receipt.clone(),
            })
    }

    fn exactly_precedes_validate_marker(
        &self,
        validate_effect: &AdapterEffect,
        validate_statement: RuntimeCandidateSemanticStatement,
    ) -> bool {
        let (
            AdapterEffect::StoreBody {
                tag: store_tag,
                round: store_round,
                subject: store_subject,
            },
            AdapterEffect::ValidateBody {
                tag: validate_tag,
                round: validate_round,
                subject: validate_subject,
            },
        ) = (&self.effect, validate_effect)
        else {
            return false;
        };
        self.validates()
            && store_tag.height() == validate_tag.height()
            && (store_tag == validate_tag || validate_tag.strictly_advances(*store_tag))
            && store_round == validate_round
            && store_subject == validate_subject
            && matches!(
                self.statement
                    .body_stage_authority_relation_to(validate_statement),
                Some(RuntimeFetchAuthorityRelation::Same | RuntimeFetchAuthorityRelation::Upgrade)
            )
    }

    fn project_retry(
        &self,
        durable_receipt: &DurableBodyReceipt,
        effect: &AdapterEffect,
        incoming: &RuntimeEffectOwnership,
    ) -> Result<RuntimeCandidateSemanticStatement, String> {
        let (
            AdapterEffect::StoreBody {
                tag: incumbent_tag,
                round: incumbent_round,
                subject: incumbent_subject,
            },
            AdapterEffect::StoreBody {
                tag: incoming_tag,
                round: incoming_round,
                subject: incoming_subject,
            },
        ) = (&self.effect, effect)
        else {
            return Err(
                "published lifecycle Store terminal received another effect stage".to_owned(),
            );
        };
        if !self.validates()
            || self.durable_receipt != *durable_receipt
            || incumbent_tag.height() != incoming_tag.height()
            || (incoming_tag != incumbent_tag && !incoming_tag.strictly_advances(*incumbent_tag))
            || incoming_round != incumbent_round
            || incoming_subject != incumbent_subject
        {
            return Err(
                "published lifecycle Store retry changed its durable body or regressed its tag"
                    .to_owned(),
            );
        }
        let pending = incoming
            .exact_pending_adapter_effect_binding(effect)
            .map_err(|_| {
                "published lifecycle Store retry lost its exact runtime binding".to_owned()
            })?;
        let incoming_statement = pending.candidate_statement().ok_or_else(|| {
            "published lifecycle Store retry omitted its candidate statement".to_owned()
        })?;
        self.statement
            .body_stage_authority_relation_to(incoming_statement)
            .ok_or_else(|| {
                "published lifecycle Store retry changed its body or authority commitment"
                    .to_owned()
            })?;
        Ok(incoming_statement)
    }

    fn project_active_store_retry(
        &self,
        durable_receipt: &DurableBodyReceipt,
        effect: &AdapterEffect,
        incoming: &RuntimeEffectOwnership,
    ) -> Result<Self, String> {
        let incoming_statement = self.project_retry(durable_receipt, effect, incoming)?;
        let relation = self
            .statement
            .body_stage_authority_relation_to(incoming_statement)
            .ok_or_else(|| {
                "published lifecycle Store retry changed its body or authority commitment"
                    .to_owned()
            })?;
        let statement = match relation {
            RuntimeFetchAuthorityRelation::Upgrade => incoming_statement,
            RuntimeFetchAuthorityRelation::Same | RuntimeFetchAuthorityRelation::Stale => {
                self.statement
            }
        };
        let projected = Self {
            statement,
            ..self.clone()
        };
        if !projected.validates() {
            return Err(
                "published lifecycle Store retry broke its immutable published binding".to_owned(),
            );
        }
        Ok(projected)
    }

    /// Compare an already-running physical Store task with a row published
    /// while that task was in flight.
    ///
    /// Unlike live retry admission, a completion may carry an older tag. The
    /// task must nevertheless be the exact immutable Store predecessor, and a
    /// task newer than the published marker is rejected. This method returns
    /// only an inert marker projection; it never rebinds the historical task
    /// into an executable capability at the marker tag.
    fn project_historical_store_completion(
        &self,
        durable_receipt: &DurableBodyReceipt,
        effect: &AdapterEffect,
        task: &RuntimeEffectOwnership,
    ) -> Result<Self, String> {
        let (
            AdapterEffect::StoreBody {
                tag: marker_tag,
                round: marker_round,
                subject: marker_subject,
            },
            AdapterEffect::StoreBody {
                tag: task_tag,
                round: task_round,
                subject: task_subject,
            },
        ) = (&self.effect, effect)
        else {
            return Err(
                "published lifecycle Store completion received another effect stage".to_owned(),
            );
        };
        if !self.validates()
            || self.durable_receipt != *durable_receipt
            || marker_tag.height() != task_tag.height()
            || (task_tag != marker_tag && !marker_tag.strictly_advances(*task_tag))
            || task_round != marker_round
            || task_subject != marker_subject
        {
            return Err(
                "published lifecycle Store completion changed its durable body or outran its marker"
                    .to_owned(),
            );
        }
        let pending = task
            .exact_pending_adapter_effect_binding(effect)
            .map_err(|_| {
                "published lifecycle Store completion lost its exact task binding".to_owned()
            })?;
        let task_statement = pending.candidate_statement().ok_or_else(|| {
            "published lifecycle Store completion omitted its task statement".to_owned()
        })?;
        let relation = self
            .statement
            .body_stage_authority_relation_to(task_statement)
            .ok_or_else(|| {
                "published lifecycle Store completion changed its body or authority commitment"
                    .to_owned()
            })?;
        let statement = match relation {
            RuntimeFetchAuthorityRelation::Upgrade => task_statement,
            RuntimeFetchAuthorityRelation::Same | RuntimeFetchAuthorityRelation::Stale => {
                self.statement
            }
        };
        let projected = Self {
            statement,
            ..self.clone()
        };
        if !projected.validates() {
            return Err(
                "published lifecycle Store completion broke its immutable published binding"
                    .to_owned(),
            );
        }
        Ok(projected)
    }
}

impl PublishedLifecycleStoreRetryCensusEntryV1 {
    /// Reconstruct the exact immutable publication identity from one
    /// authenticated lifecycle registry Store row.
    pub(in crate::sumeragi) fn from_exact_published_store(
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
        durable_receipt: &DurableBodyReceipt,
    ) -> Option<Self> {
        PublishedLifecycleStoreTerminalRetrySealV1::seal_published_store(
            effect,
            pending,
            durable_receipt,
        )?
        .publication_census_entry()
    }

    /// Return the unique body key fixed by the complete Store publication.
    pub(in crate::sumeragi) fn key(&self) -> (wire::ConsensusRound, wire::BlockSubject) {
        let AdapterEffect::StoreBody { round, subject, .. } = &self.effect else {
            unreachable!("published Store census entries are StoreBody-only");
        };
        (*round, *subject)
    }
}

/// Inert fingerprint for one Validate row published by the direct lifecycle
/// Store-to-Validate transaction.
///
/// The lifecycle registry owns the executable pending binding. This marker
/// owns no service work and exists only so periodic reducer retransmission can
/// prove that the same physical Validate stage is already durable.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PublishedLifecycleValidateRetryMarkerV1 {
    effect: AdapterEffect,
    statement: RuntimeCandidateSemanticStatement,
    durable_receipt: DurableBodyReceipt,
    store_terminal: PublishedLifecycleStoreTerminalRetrySealV1,
    lifecycle_ordinal: Option<u128>,
}

impl PublishedLifecycleValidateRetryMarkerV1 {
    fn prepare(
        effect: &AdapterEffect,
        durable_receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Option<Self> {
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = effect
        else {
            return None;
        };
        let statement = pending.candidate_statement()?;
        if !pending.exactly_binds_adapter_effect(effect)
            || tag.height() != round.height
            || durable_receipt.context_id() != round.context_id
            || durable_receipt.round() != *round
            || durable_receipt.subject() != *subject
            || statement.context_id() != round.context_id
            || statement.proposal_round() != *round
            || statement.subject() != Some(*subject)
        {
            return None;
        }
        let store_terminal = PublishedLifecycleStoreTerminalRetrySealV1::seal_exact(
            effect,
            pending,
            durable_receipt,
        )?;
        Some(Self {
            effect: effect.clone(),
            statement,
            durable_receipt: durable_receipt.clone(),
            store_terminal,
            lifecycle_ordinal: None,
        })
    }

    fn prepare_from_published_store(
        effect: &AdapterEffect,
        durable_receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
        store_terminal: PublishedLifecycleStoreTerminalRetrySealV1,
    ) -> Option<Self> {
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = effect
        else {
            return None;
        };
        let published_statement = pending.candidate_statement()?;
        let projected_store =
            pending.project_validate_store_predecessor(effect, &store_terminal.effect)?;
        let projected_store =
            projected_store.published_store_retry_fingerprint(&store_terminal.effect)?;
        if !pending.exactly_binds_adapter_effect(effect)
            || tag.height() != round.height
            || durable_receipt.context_id() != round.context_id
            || durable_receipt.round() != *round
            || durable_receipt.subject() != *subject
            || published_statement.context_id() != round.context_id
            || published_statement.proposal_round() != *round
            || published_statement.subject() != Some(*subject)
            || store_terminal.durable_receipt != *durable_receipt
            || projected_store != store_terminal.pending
        {
            return None;
        }
        let relation = store_terminal
            .statement
            .body_stage_authority_relation_to(published_statement)?;
        let statement = match relation {
            RuntimeFetchAuthorityRelation::Upgrade => published_statement,
            RuntimeFetchAuthorityRelation::Same | RuntimeFetchAuthorityRelation::Stale => {
                store_terminal.statement
            }
        };
        if !store_terminal.exactly_precedes_validate_marker(effect, statement) {
            return None;
        }
        Some(Self {
            effect: effect.clone(),
            statement,
            durable_receipt: durable_receipt.clone(),
            store_terminal,
            lifecycle_ordinal: None,
        })
    }

    /// Return whether the concrete registry still owns this Validate row.
    const fn owns_live_lifecycle_row(&self) -> bool {
        self.lifecycle_ordinal.is_some()
    }

    fn bind_lifecycle_ordinal(&mut self, ordinal: u128) -> Result<(), String> {
        if ordinal == 0 {
            return Err("published lifecycle Validate received a zero ordinal".to_owned());
        }
        match self.lifecycle_ordinal {
            Some(existing) if existing != ordinal => {
                Err("published lifecycle Validate changed its exact lifecycle ordinal".to_owned())
            }
            Some(_) => Ok(()),
            None => {
                self.lifecycle_ordinal = Some(ordinal);
                Ok(())
            }
        }
    }

    fn release_lifecycle_ordinal(&mut self, ordinal: u128) -> Result<(), String> {
        if self.lifecycle_ordinal != Some(ordinal) {
            return Err(
                "resolved published lifecycle Validate changed its exact ordinal".to_owned(),
            );
        }
        self.lifecycle_ordinal = None;
        Ok(())
    }

    fn project_retry(
        &self,
        effect: &AdapterEffect,
        incoming: &RuntimeEffectOwnership,
    ) -> Result<Self, String> {
        let (
            AdapterEffect::ValidateBody {
                tag: incumbent_tag,
                round: incumbent_round,
                subject: incumbent_subject,
            },
            AdapterEffect::ValidateBody {
                tag: incoming_tag,
                round: incoming_round,
                subject: incoming_subject,
            },
        ) = (&self.effect, effect)
        else {
            return Err(
                "published lifecycle Validate marker received another effect stage".to_owned(),
            );
        };
        if (incoming_tag != incumbent_tag && !incoming_tag.strictly_advances(*incumbent_tag))
            || incoming_round != incumbent_round
            || incoming_subject != incumbent_subject
            || self.durable_receipt.round() != *incumbent_round
            || self.durable_receipt.subject() != *incumbent_subject
        {
            return Err(
                "published lifecycle Validate retry changed its exact body or regressed its tag"
                    .to_owned(),
            );
        }
        let pending = incoming
            .exact_pending_adapter_effect_binding(effect)
            .map_err(|_| {
                "published lifecycle Validate retry lost its exact runtime binding".to_owned()
            })?;
        let incoming_statement = pending.candidate_statement().ok_or_else(|| {
            "published lifecycle Validate retry omitted its candidate statement".to_owned()
        })?;
        let relation = self
            .statement
            .body_stage_authority_relation_to(incoming_statement)
            .ok_or_else(|| {
                "published lifecycle Validate retry changed its body or authority commitment"
                    .to_owned()
            })?;
        let statement = match relation {
            RuntimeFetchAuthorityRelation::Upgrade => incoming_statement,
            RuntimeFetchAuthorityRelation::Same | RuntimeFetchAuthorityRelation::Stale => {
                self.statement
            }
        };
        if !self
            .store_terminal
            .exactly_precedes_validate_marker(effect, statement)
        {
            return Err(
                "published lifecycle Validate retry lost its exact Store predecessor".to_owned(),
            );
        }
        Ok(Self {
            effect: effect.clone(),
            statement,
            durable_receipt: self.durable_receipt.clone(),
            store_terminal: self.store_terminal.clone(),
            lifecycle_ordinal: self.lifecycle_ordinal,
        })
    }

    /// Return whether this terminal marker is the exact Commit-authorized
    /// Validate owner for one already-fsynced successful validation.
    ///
    /// This is comparison-only. It cannot create work and deliberately rejects
    /// an ordinal-bound marker because that marker still has a concrete row
    /// which must settle under its original lifecycle owner.
    fn is_unbound_exact_decision_owner(
        &self,
        decision: DurableDecision,
        validated_receipt: &ValidatedBodyReceipt,
    ) -> bool {
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = &self.effect
        else {
            return false;
        };
        !self.owns_live_lifecycle_row()
            && tag.height() == decision.0.height
            && *round == decision.1
            && *subject == decision.2
            && self.durable_receipt == *validated_receipt.durable()
            && validated_receipt.execution_commitment() == decision.3
            && self.statement.context_id() == decision.0.context_id
            && self.statement.round() == decision.0
            && self.statement.proposal_round() == decision.1
            && self.statement.subject() == Some(decision.2)
            && self.statement.phase() == Some(wire::GlobalPhase::Commit)
            && self.statement.execution_commitment() == Some(decision.3)
            && self
                .store_terminal
                .exactly_precedes_validate_marker(&self.effect, self.statement)
    }

    /// Return whether a resolved direct-lifecycle marker must redispatch one
    /// exact newer-tag Commit refinement into normal Validate admission.
    ///
    /// Same/stale retries remain inert. The strict authority and tag advance
    /// ensure that a marker already projected to the Decision cannot repeatedly
    /// mint replacement lifecycle rows.
    fn is_unbound_exact_decision_upgrade(
        &self,
        projected: &Self,
        decision: DurableDecision,
        validated_receipt: &ValidatedBodyReceipt,
    ) -> bool {
        let (
            AdapterEffect::ValidateBody {
                tag: incumbent_tag,
                ..
            },
            AdapterEffect::ValidateBody {
                tag: projected_tag, ..
            },
        ) = (&self.effect, &projected.effect)
        else {
            return false;
        };
        !self.owns_live_lifecycle_row()
            && !projected.owns_live_lifecycle_row()
            && projected_tag.strictly_advances(*incumbent_tag)
            && self.durable_receipt == projected.durable_receipt
            && self.store_terminal == projected.store_terminal
            && self
                .statement
                .body_stage_authority_relation_to(projected.statement)
                == Some(RuntimeFetchAuthorityRelation::Upgrade)
            && projected.is_unbound_exact_decision_owner(decision, validated_receipt)
    }

    fn project_store_retry(
        &self,
        durable_receipt: &DurableBodyReceipt,
        effect: &AdapterEffect,
        incoming: &RuntimeEffectOwnership,
    ) -> Result<(), String> {
        if !self
            .store_terminal
            .exactly_precedes_validate_marker(&self.effect, self.statement)
        {
            return Err(
                "published lifecycle Validate marker lost its exact Store predecessor".to_owned(),
            );
        }
        let incoming_statement =
            self.store_terminal
                .project_retry(durable_receipt, effect, incoming)?;
        if !matches!(
            incoming_statement.body_stage_authority_relation_to(self.statement),
            Some(RuntimeFetchAuthorityRelation::Same | RuntimeFetchAuthorityRelation::Upgrade)
        ) {
            return Err(
                "published lifecycle Store retry outran its published Validate authority"
                    .to_owned(),
            );
        }
        Ok(())
    }

    /// Prove that an older physical Store completion is already represented
    /// by this immutable published Validate successor.
    fn project_historical_store_completion(
        &self,
        durable_receipt: &DurableBodyReceipt,
        effect: &AdapterEffect,
        task: &RuntimeEffectOwnership,
    ) -> Result<(), String> {
        if !self
            .store_terminal
            .exactly_precedes_validate_marker(&self.effect, self.statement)
        {
            return Err(
                "published lifecycle Validate marker lost its exact Store predecessor".to_owned(),
            );
        }
        let projected = self.store_terminal.project_historical_store_completion(
            durable_receipt,
            effect,
            task,
        )?;
        if !matches!(
            projected
                .statement
                .body_stage_authority_relation_to(self.statement),
            Some(RuntimeFetchAuthorityRelation::Same | RuntimeFetchAuthorityRelation::Upgrade)
        ) {
            return Err(
                "published lifecycle Store completion outran its published Validate authority"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

/// Move-only preflight for installing one exact Store marker after a direct
/// Fetch-to-Store transaction crosses durable publication.
#[must_use = "the direct lifecycle Store marker has not crossed publication"]
pub(in crate::sumeragi) struct PreparedPublishedLifecycleStoreRetryMarkerV1 {
    durable_receipt: DurableBodyReceipt,
    marker: Option<PublishedLifecycleStoreTerminalRetrySealV1>,
}

impl PreparedPublishedLifecycleStoreRetryMarkerV1 {
    /// Bind the preflighted executor catalog slot to the exact Store child
    /// sealed by the registry after the adapter preview.
    pub(in crate::sumeragi) fn bind_store_successor(
        mut self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<Self, String> {
        let marker = PublishedLifecycleStoreTerminalRetrySealV1::seal_published_store(
            effect,
            pending,
            &self.durable_receipt,
        )
        .ok_or_else(|| {
            "direct lifecycle Store marker changed its exact Fetch successor".to_owned()
        })?;
        self.marker = Some(marker);
        Ok(self)
    }
}

/// Move-only preflight for installing one direct lifecycle Validate marker
/// after its LedgerV1 successor is durable.
#[must_use = "the direct lifecycle Validate marker has not crossed publication"]
pub(in crate::sumeragi) struct PreparedPublishedLifecycleValidateRetryMarkerV1 {
    durable_receipt: DurableBodyReceipt,
    store_terminal: PublishedLifecycleStoreTerminalRetrySealV1,
    marker: Option<PublishedLifecycleValidateRetryMarkerV1>,
}

impl PreparedPublishedLifecycleValidateRetryMarkerV1 {
    /// Bind the preflighted executor catalog slot to the exact Validate
    /// successor sealed by the registry after the adapter preview.
    pub(in crate::sumeragi) fn bind_validate_successor(
        mut self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<Self, String> {
        let marker = PublishedLifecycleValidateRetryMarkerV1::prepare_from_published_store(
            effect,
            &self.durable_receipt,
            pending,
            self.store_terminal.clone(),
        )
        .ok_or_else(|| {
            "direct lifecycle Validate marker changed its exact Store successor".to_owned()
        })?;
        self.marker = Some(marker);
        Ok(self)
    }
}

impl<R: EffectRuntime> V2EffectExecutor<R> {
    /// Begin an atomic sink for the complete storage-authenticated census.
    pub(in crate::sumeragi) fn prepare_recovered_durable_validate_retry_install(
        &mut self,
    ) -> Result<PreparedRecoveredDurableValidateRetryInstallV1<'_, R>, EffectExecutorError> {
        self.ensure_open()?;
        let runtime_decision = self
            .runtime
            .decided_body()
            .map_err(EffectExecutorError::Runtime)?;
        if self.protected_decision.is_some()
            || !self.pending_durable_validate_admissions.is_empty()
            || !self.durable_validate_retry_seals.is_empty()
        {
            return Err(EffectExecutorError::Contract(
                "cold Validate retry census collided with live executor ownership".to_owned(),
            ));
        }
        Ok(PreparedRecoveredDurableValidateRetryInstallV1 {
            executor: self,
            runtime_decision,
            prepared: BTreeMap::new(),
        })
    }

    /// Return whether every inert Validate retry tombstone is scoped to the
    /// sole decided body which may survive until this per-height executor is
    /// consumed at rollover.
    fn durable_validate_retry_seals_are_finalization_inert(&self) -> bool {
        self.published_lifecycle_store_retry_markers
            .iter()
            .all(|(key, marker)| {
                marker
                    .publication_census_entry()
                    .is_some_and(|entry| entry.key() == *key)
            })
            && self.durable_validate_retry_seals.iter().all(|(key, seal)| {
                seal.lifecycle_ordinal().is_none()
                    && self
                        .protected_decision
                        .is_some_and(|(_, round, subject, _)| *key == (round, subject))
            })
            && self
                .published_lifecycle_validate_retry_markers
                .iter()
                .all(|(key, marker)| {
                    !marker.owns_live_lifecycle_row()
                        && self
                            .protected_decision
                            .is_some_and(|(_, round, subject, _)| *key == (round, subject))
                })
            && self
                .durable_validate_retry_seals
                .keys()
                .chain(self.published_lifecycle_validate_retry_markers.keys())
                .all(|key| {
                    self.protected_decision
                        .is_some_and(|(_, round, subject, _)| *key == (round, subject))
                })
    }

    /// Project a bounded, payload-free census for a finalization stall.
    ///
    /// The ordinary readiness predicate deliberately collapses every retry
    /// ownership failure into one blocker label. This diagnostic keeps the
    /// operator record useful without exposing block bytes: it identifies the
    /// retained owner kind, its logical ordinal, and whether its body key is
    /// the exact protected Decision selected for this height.
    pub(in crate::sumeragi) fn durable_validate_retry_finalization_diagnostic(&self) -> String {
        let selected = self
            .protected_decision
            .map(|(_, round, subject, _)| (round, subject));
        let seals = self
            .durable_validate_retry_seals
            .iter()
            .take(8)
            .map(|(key, seal)| {
                let kind = match seal {
                    DurableValidateRetrySealV1::Live { .. } => "Live",
                    DurableValidateRetrySealV1::Recovered { .. } => "Recovered",
                };
                format!(
                    "{kind}:key={key:?}:ordinal={:?}:selected={}",
                    seal.lifecycle_ordinal(),
                    selected == Some(*key)
                )
            })
            .collect::<Vec<_>>();
        let validate_markers = self
            .published_lifecycle_validate_retry_markers
            .iter()
            .take(8)
            .map(|(key, marker)| {
                format!(
                    "key={key:?}:ordinal={:?}:selected={}",
                    marker.lifecycle_ordinal,
                    selected == Some(*key)
                )
            })
            .collect::<Vec<_>>();
        let store_markers = self
            .published_lifecycle_store_retry_markers
            .iter()
            .take(8)
            .map(|(key, marker)| {
                format!(
                    "key={key:?}:valid={}",
                    marker
                        .publication_census_entry()
                        .is_some_and(|entry| entry.key() == *key)
                )
            })
            .collect::<Vec<_>>();
        format!(
            "selected={selected:?} decision_body_drained={} seals_total={} seals={seals:?} \
             validate_markers_total={} validate_markers={validate_markers:?} \
             store_markers_total={} store_markers={store_markers:?}",
            self.decision_body_drained,
            self.durable_validate_retry_seals.len(),
            self.published_lifecycle_validate_retry_markers.len(),
            self.published_lifecycle_store_retry_markers.len(),
        )
    }

    /// Project the complete immutable executor-side Store publication census.
    ///
    /// The lifecycle owner compares this map byte-for-byte with its
    /// authenticated registry census immediately before all-row retirement.
    pub(in crate::sumeragi) fn published_lifecycle_store_retry_census(
        &self,
    ) -> Result<
        BTreeMap<
            (wire::ConsensusRound, wire::BlockSubject),
            PublishedLifecycleStoreRetryCensusEntryV1,
        >,
        String,
    > {
        let mut census = BTreeMap::new();
        for (key, marker) in &self.published_lifecycle_store_retry_markers {
            let entry = marker.publication_census_entry().ok_or_else(|| {
                "executor published Store marker lost its immutable publication identity".to_owned()
            })?;
            if entry.key() != *key || census.insert(*key, entry).is_some() {
                return Err(
                    "executor published Store marker changed or duplicated its body key".to_owned(),
                );
            }
        }
        Ok(census)
    }

    /// Preflight one inert retry marker before the direct Fetch-to-Store
    /// transaction takes the runtime borrow and publishes its Ledger row.
    pub(in crate::sumeragi) fn prepare_published_lifecycle_store_retry_marker(
        &self,
        durable_receipt: &DurableBodyReceipt,
    ) -> Result<PreparedPublishedLifecycleStoreRetryMarkerV1, EffectExecutorError> {
        let key = (durable_receipt.round(), durable_receipt.subject());
        let retained_body_is_exact =
            self.recovered_bodies
                .get(&key)
                .is_some_and(|(manifest, retained)| {
                    retained == durable_receipt
                        && manifest.round == durable_receipt.round()
                        && manifest.subject == durable_receipt.subject()
                        && HashOf::new(manifest) == durable_receipt.manifest_hash()
                });
        if !retained_body_is_exact
            || self.durable_bodies.get(&key) != Some(durable_receipt)
            || self.pending_durable_validate_admissions.contains_key(&key)
            || self.durable_validate_retry_seals.contains_key(&key)
            || self
                .published_lifecycle_store_retry_markers
                .contains_key(&key)
            || self
                .published_lifecycle_validate_retry_markers
                .contains_key(&key)
        {
            return Err(EffectExecutorError::Contract(
                "direct lifecycle Store marker overlaps a foreign executor body owner".to_owned(),
            ));
        }
        Ok(PreparedPublishedLifecycleStoreRetryMarkerV1 {
            durable_receipt: durable_receipt.clone(),
            marker: None,
        })
    }

    /// Install the preflighted Store marker only after its exact lifecycle row
    /// has crossed durable publication.
    pub(in crate::sumeragi) fn commit_published_lifecycle_store_retry_marker(
        &mut self,
        prepared: PreparedPublishedLifecycleStoreRetryMarkerV1,
    ) {
        let marker = prepared
            .marker
            .expect("published direct lifecycle Store retains its sealed marker");
        assert_eq!(marker.durable_receipt, prepared.durable_receipt);
        let key = (
            marker.durable_receipt.round(),
            marker.durable_receipt.subject(),
        );
        assert_eq!(self.durable_bodies.get(&key), Some(&marker.durable_receipt));
        assert!(!self.pending_durable_validate_admissions.contains_key(&key));
        assert!(!self.durable_validate_retry_seals.contains_key(&key));
        assert!(
            !self
                .published_lifecycle_validate_retry_markers
                .contains_key(&key)
        );
        let previous = self
            .published_lifecycle_store_retry_markers
            .insert(key, marker);
        assert!(previous.is_none());
    }

    /// Preflight one inert retry marker before the direct Store-to-Validate
    /// transaction takes the runtime borrow and publishes its Ledger row.
    pub(in crate::sumeragi) fn prepare_published_lifecycle_validate_retry_marker(
        &self,
        durable_receipt: &DurableBodyReceipt,
    ) -> Result<PreparedPublishedLifecycleValidateRetryMarkerV1, EffectExecutorError> {
        let key = (durable_receipt.round(), durable_receipt.subject());
        let retained_body_is_exact =
            self.recovered_bodies
                .get(&key)
                .is_some_and(|(manifest, retained)| {
                    retained == durable_receipt
                        && manifest.round == durable_receipt.round()
                        && manifest.subject == durable_receipt.subject()
                        && HashOf::new(manifest) == durable_receipt.manifest_hash()
                });
        if !retained_body_is_exact
            || self.durable_bodies.get(&key) != Some(durable_receipt)
            || self.pending_durable_validate_admissions.contains_key(&key)
            || self.durable_validate_retry_seals.contains_key(&key)
            || self
                .published_lifecycle_validate_retry_markers
                .contains_key(&key)
        {
            return Err(EffectExecutorError::Contract(
                "direct lifecycle Validate marker overlaps a foreign executor body owner"
                    .to_owned(),
            ));
        }
        let store_terminal = self
            .published_lifecycle_store_retry_markers
            .get(&key)
            .cloned()
            .ok_or_else(|| {
                EffectExecutorError::Contract(
                    "direct lifecycle Validate marker omitted its published Store predecessor"
                        .to_owned(),
                )
            })?;
        if store_terminal.durable_receipt != *durable_receipt || !store_terminal.validates() {
            return Err(EffectExecutorError::Contract(
                "direct lifecycle Validate marker changed its published Store predecessor"
                    .to_owned(),
            ));
        }
        Ok(PreparedPublishedLifecycleValidateRetryMarkerV1 {
            durable_receipt: durable_receipt.clone(),
            store_terminal,
            marker: None,
        })
    }

    /// Install the preflighted marker after the direct successor and reducer
    /// transition have both crossed durable publication.
    pub(in crate::sumeragi) fn commit_published_lifecycle_validate_retry_marker(
        &mut self,
        prepared: PreparedPublishedLifecycleValidateRetryMarkerV1,
        lifecycle_ordinal: u128,
    ) {
        let mut marker = prepared
            .marker
            .expect("published direct lifecycle Validate retains its sealed marker");
        marker
            .bind_lifecycle_ordinal(lifecycle_ordinal)
            .expect("published direct lifecycle Validate binds its committed child ordinal");
        assert_eq!(marker.durable_receipt, prepared.durable_receipt);
        let key = (
            marker.durable_receipt.round(),
            marker.durable_receipt.subject(),
        );
        assert_eq!(self.durable_bodies.get(&key), Some(&marker.durable_receipt));
        assert!(!self.pending_durable_validate_admissions.contains_key(&key));
        assert!(!self.durable_validate_retry_seals.contains_key(&key));
        let store_terminal = self
            .published_lifecycle_store_retry_markers
            .remove(&key)
            .expect("published Validate consumes its exact active Store marker");
        assert_eq!(store_terminal, marker.store_terminal);
        let previous = self
            .published_lifecycle_validate_retry_markers
            .insert(key, marker);
        assert!(previous.is_none());
    }

    /// Reinstall the same inert marker from any authenticated recovered
    /// durable Validate row before live clocks are armed.
    pub(in crate::sumeragi) fn install_recovered_published_lifecycle_validate_retry_marker(
        &mut self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
        durable_receipt: &DurableBodyReceipt,
        lifecycle_ordinal: u128,
    ) -> Result<(), EffectExecutorError> {
        self.ensure_open()?;
        if self.runtime.lifecycle_live_clocks_are_armed() {
            return Err(EffectExecutorError::Contract(
                "recovered lifecycle Validate marker installation followed live clock activation"
                    .to_owned(),
            ));
        }
        let key = (durable_receipt.round(), durable_receipt.subject());
        let retained_body_is_exact =
            self.recovered_bodies
                .get(&key)
                .is_some_and(|(manifest, retained)| {
                    retained == durable_receipt
                        && manifest.round == durable_receipt.round()
                        && manifest.subject == durable_receipt.subject()
                        && HashOf::new(manifest) == durable_receipt.manifest_hash()
                });
        if !retained_body_is_exact
            || self.durable_bodies.get(&key) != Some(durable_receipt)
            || self.pending_durable_validate_admissions.contains_key(&key)
            || self.durable_validate_retry_seals.contains_key(&key)
            || self
                .published_lifecycle_store_retry_markers
                .contains_key(&key)
            || self
                .published_lifecycle_validate_retry_markers
                .contains_key(&key)
        {
            return Err(EffectExecutorError::Contract(
                "recovered lifecycle Validate marker overlaps a foreign executor body owner"
                    .to_owned(),
            ));
        }
        let mut marker =
            PublishedLifecycleValidateRetryMarkerV1::prepare(effect, durable_receipt, pending)
                .ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "recovered lifecycle Validate marker changed its exact Store predecessor"
                            .to_owned(),
                    )
                })?;
        marker
            .bind_lifecycle_ordinal(lifecycle_ordinal)
            .map_err(EffectExecutorError::Contract)?;
        let previous = self
            .published_lifecycle_validate_retry_markers
            .insert(key, marker);
        debug_assert!(previous.is_none());
        Ok(())
    }

    /// Reinstall the same inert Store marker from any authenticated recovered
    /// durable Store row before live clocks are armed.
    pub(in crate::sumeragi) fn install_recovered_published_lifecycle_store_retry_marker(
        &mut self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
        durable_receipt: &DurableBodyReceipt,
    ) -> Result<(), EffectExecutorError> {
        self.ensure_open()?;
        if self.runtime.lifecycle_live_clocks_are_armed() {
            return Err(EffectExecutorError::Contract(
                "recovered lifecycle Store marker installation followed live clock activation"
                    .to_owned(),
            ));
        }
        let prepared = self
            .prepare_published_lifecycle_store_retry_marker(durable_receipt)?
            .bind_store_successor(effect, pending)
            .map_err(EffectExecutorError::Contract)?;
        self.commit_published_lifecycle_store_retry_marker(prepared);
        Ok(())
    }

    /// Recheck the sole retry authority for one exact live Validate row.
    fn exactly_owns_validate_retry_lifecycle_ordinal(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        lifecycle_ordinal: u128,
    ) -> bool {
        if lifecycle_ordinal == 0 {
            return false;
        }
        match (
            self.durable_validate_retry_seals.get(&key),
            self.published_lifecycle_validate_retry_markers.get(&key),
        ) {
            (Some(seal), None) => seal.lifecycle_ordinal() == Some(lifecycle_ordinal),
            (None, Some(marker)) => marker.lifecycle_ordinal == Some(lifecycle_ordinal),
            (Some(_), Some(_)) | (None, None) => false,
        }
    }

    /// Preflight the sole Validate retry row authenticated as one recovered
    /// Decision Apply's durable predecessor.
    fn preflight_recovered_apply_validate_retry_predecessor(
        &self,
        dispatch_key: LifecycleDecisionApplyDispatchKeyV1,
        key: (wire::ConsensusRound, wire::BlockSubject),
        validate_predecessor_ordinal: u128,
    ) -> Result<Option<u128>, EffectExecutorError> {
        if dispatch_key.lineage() != LifecycleDecisionApplyLineageV1::Recovered
            || !dispatch_key.matches_height_context(&self.context)
        {
            return Err(EffectExecutorError::Contract(
                "recovered Apply retry cleanup changed its exact carrier lineage".to_owned(),
            ));
        }
        if validate_predecessor_ordinal == 0
            || validate_predecessor_ordinal >= dispatch_key.lifecycle_ordinal()
        {
            return Err(EffectExecutorError::Contract(
                "recovered Apply retry cleanup omitted its Validate predecessor".to_owned(),
            ));
        }
        match (
            self.durable_validate_retry_seals.get(&key),
            self.published_lifecycle_validate_retry_markers.get(&key),
        ) {
            (None, None) => Ok(None),
            (Some(seal), None) => match seal.lifecycle_ordinal() {
                None => Ok(None),
                Some(ordinal) if ordinal == validate_predecessor_ordinal => {
                    Ok(Some(validate_predecessor_ordinal))
                }
                Some(_) => Err(EffectExecutorError::Contract(
                    "recovered Apply changed its exact durable Validate predecessor ordinal"
                        .to_owned(),
                )),
            },
            (None, Some(marker)) => match marker.lifecycle_ordinal {
                None => Ok(None),
                Some(ordinal) if ordinal == validate_predecessor_ordinal => {
                    Ok(Some(validate_predecessor_ordinal))
                }
                Some(_) => Err(EffectExecutorError::Contract(
                    "recovered Apply changed its exact published Validate predecessor ordinal"
                        .to_owned(),
                )),
            },
            (Some(_), Some(_)) => Err(EffectExecutorError::Contract(
                "recovered Apply retained two Validate predecessor retry authorities".to_owned(),
            )),
        }
    }

    /// Release only the preflighted recovered Apply predecessor, leaving an
    /// already-inert tombstone or an absent retry authority unchanged.
    fn release_recovered_apply_validate_retry_predecessor(
        &mut self,
        dispatch_key: LifecycleDecisionApplyDispatchKeyV1,
        key: (wire::ConsensusRound, wire::BlockSubject),
        validate_predecessor_ordinal: u128,
    ) -> Result<bool, EffectExecutorError> {
        let Some(predecessor_ordinal) = self.preflight_recovered_apply_validate_retry_predecessor(
            dispatch_key,
            key,
            validate_predecessor_ordinal,
        )?
        else {
            return Ok(false);
        };
        if !self.release_validate_retry_lifecycle_ordinal(key, predecessor_ordinal)? {
            return Err(EffectExecutorError::Contract(
                "recovered Apply lost its preflighted Validate predecessor retry authority"
                    .to_owned(),
            ));
        }
        Ok(true)
    }

    /// Release only the retry authority bound to the lifecycle row that just
    /// durably terminalized or advanced to its successor.
    pub(in crate::sumeragi) fn release_validate_retry_lifecycle_ordinal(
        &mut self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        lifecycle_ordinal: u128,
    ) -> Result<bool, EffectExecutorError> {
        let has_seal = self.durable_validate_retry_seals.contains_key(&key);
        let has_marker = self
            .published_lifecycle_validate_retry_markers
            .contains_key(&key);
        if has_seal && has_marker {
            return Err(EffectExecutorError::Contract(
                "resolved Validate retained two retry authorities".to_owned(),
            ));
        }
        let selected_body = self
            .protected_decision
            .map(|(_, round, subject, _)| (round, subject));
        let discard =
            selected_body.is_some_and(|selected| self.decision_body_drained || selected != key);
        if has_seal {
            self.durable_validate_retry_seals
                .get_mut(&key)
                .expect("checked Validate seal remains serialized")
                .release_lifecycle_ordinal(lifecycle_ordinal)
                .map_err(EffectExecutorError::Contract)?;
            if discard {
                self.durable_validate_retry_seals.remove(&key);
            }
            return Ok(true);
        }
        if has_marker {
            self.published_lifecycle_validate_retry_markers
                .get_mut(&key)
                .expect("checked published Validate marker remains serialized")
                .release_lifecycle_ordinal(lifecycle_ordinal)
                .map_err(EffectExecutorError::Contract)?;
            if discard {
                self.published_lifecycle_validate_retry_markers.remove(&key);
            }
            return Ok(true);
        }
        Ok(false)
    }

    /// Exact carrier block hashes still owned by retained missing-sidecar work.
    pub(crate) fn deferred_merge_sidecar_blocks(&self) -> BTreeSet<HashOf<BlockHeader>> {
        self.deferred_merge_work
            .keys()
            .filter_map(|work_id| {
                self.pending_applications
                    .get(work_id)
                    .map(|pending| pending.task.subject().block_hash)
            })
            .collect()
    }

    fn diagnostic_pending_work_is_exact(effect: &AdapterEffect) -> bool {
        Self::restart_effect_source(effect) != RestartEffectSource::DiagnosticOnly
            || matches!(
                Self::pending_work_producer(effect),
                None | Some(PendingWorkProducer::Output)
            )
    }

    /// Park one signed or diagnostic output before any external service I/O.
    fn park_lifecycle_output_admission(
        &mut self,
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
    ) -> Result<(), EffectExecutorError> {
        if let Some(existing) = self
            .pending_lifecycle_output_admissions
            .values()
            .find(|pending| pending.exactly_matches_retry(&effect, &ownership))
        {
            let _ = existing;
            return Ok(());
        }
        self.ensure_pending_slot()?;
        let pending =
            PendingLifecycleOutputAdmissionV1::seal_exact(effect, ownership).map_err(|_| {
                EffectExecutorError::Contract(
                    "signed lifecycle output omitted its exact runtime binding".to_owned(),
                )
            })?;
        let key = pending.key();
        match self.pending_lifecycle_output_admissions.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(pending);
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(_) => Err(EffectExecutorError::Contract(
                "lifecycle output admission key collided with a foreign owner".to_owned(),
            )),
        }
    }

    fn pending_work_producer(effect: &AdapterEffect) -> Option<PendingWorkProducer> {
        match effect {
            AdapterEffect::Sign { .. } => Some(PendingWorkProducer::Sign),
            AdapterEffect::FetchBody { .. } => Some(PendingWorkProducer::Fetch),
            AdapterEffect::StoreBody { .. } => Some(PendingWorkProducer::Store),
            AdapterEffect::ValidateBody { .. } => Some(PendingWorkProducer::Validate),
            AdapterEffect::Apply { .. } => Some(PendingWorkProducer::Apply),
            AdapterEffect::Broadcast(_)
            | AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. } => Some(PendingWorkProducer::Output),
            AdapterEffect::EnterView { .. } => None,
        }
    }

    fn allocate_work_id(&mut self) -> Result<EffectWorkId, EffectExecutorError> {
        let id = EffectWorkId(self.next_work_id);
        self.next_work_id = self
            .next_work_id
            .checked_add(1)
            .ok_or(EffectExecutorError::WorkIdExhausted)?;
        Ok(id)
    }

    /// Count live service/admission work. Validate retry authorities own no
    /// service slot even while an ordinal binds them to a live registry row;
    /// cleanup preserves those rows until exact lifecycle settlement.
    fn pending_work(&self) -> usize {
        self.pending_signatures
            .len()
            .checked_add(self.pending_fetches.len())
            .and_then(|total| total.checked_add(self.pending_stores.len()))
            .and_then(|total| total.checked_add(self.pending_durable_validate_admissions.len()))
            .and_then(|total| {
                total.checked_add(usize::from(self.pending_live_wal_sign_admission.is_some()))
            })
            .and_then(|total| total.checked_add(self.pending_lifecycle_output_admissions.len()))
            .and_then(|total| total.checked_add(self.pending_applications.len()))
            .unwrap_or(usize::MAX)
    }

    fn install_pending_durable_validate_admission(
        &mut self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
        pending: PendingDurableValidateAdmissionV1,
        store_terminal: Option<DurableStoreTerminalRetrySealV1>,
    ) -> Result<(), EffectExecutorError> {
        if self.pending_durable_validate_admissions.contains_key(&key)
            || self.durable_validate_retry_seals.contains_key(&key)
            || self
                .published_lifecycle_store_retry_markers
                .contains_key(&key)
            || self
                .published_lifecycle_validate_retry_markers
                .contains_key(&key)
        {
            return Err(EffectExecutorError::Contract(
                "durable Validate duplicated its exact lifecycle admission owner".to_owned(),
            ));
        }
        let seal =
            DurableValidateRetrySealV1::seal_exact(effect, ownership, &pending, store_terminal)
                .ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "durable Validate could not seal its exact post-admission retry owner"
                            .to_owned(),
                    )
                })?;
        let previous = self
            .pending_durable_validate_admissions
            .insert(key, pending);
        debug_assert!(previous.is_none());
        let previous = self.durable_validate_retry_seals.insert(key, seal);
        debug_assert!(previous.is_none());
        Ok(())
    }

    /// Atomically replace one terminal direct-lifecycle Validate marker with
    /// the exact pending admission which will replay its missing successor.
    ///
    /// Every fallible proof is completed while the immutable marker remains in
    /// the catalog. Once the seal exists, the map swap consists only of exact
    /// removals and infallible inserts, so a partial owner transition cannot be
    /// observed or survive an error.
    fn replace_terminal_published_validate_with_pending_admission(
        &mut self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        expected_marker: PublishedLifecycleValidateRetryMarkerV1,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
        pending: PendingDurableValidateAdmissionV1,
        store_terminal: DurableStoreTerminalRetrySealV1,
    ) -> Result<(), EffectExecutorError> {
        if expected_marker.owns_live_lifecycle_row()
            || self
                .published_lifecycle_validate_retry_markers
                .get(&key)
                != Some(&expected_marker)
            || self.pending_durable_validate_admissions.contains_key(&key)
            || self.durable_validate_retry_seals.contains_key(&key)
            || self
                .published_lifecycle_store_retry_markers
                .contains_key(&key)
        {
            return Err(EffectExecutorError::Contract(
                "terminal published Validate changed before lifecycle readmission".to_owned(),
            ));
        }
        let seal = DurableValidateRetrySealV1::seal_exact(
            effect,
            ownership,
            &pending,
            Some(store_terminal),
        )
        .ok_or_else(|| {
            EffectExecutorError::Contract(
                "terminal published Validate could not seal its replacement retry owner"
                    .to_owned(),
            )
        })?;

        let removed = self
            .published_lifecycle_validate_retry_markers
            .remove(&key);
        assert_eq!(removed, Some(expected_marker));
        let previous = self
            .pending_durable_validate_admissions
            .insert(key, pending);
        assert!(previous.is_none());
        let previous = self.durable_validate_retry_seals.insert(key, seal);
        assert!(previous.is_none());
        Ok(())
    }

    fn bind_validate_retry_lifecycle_ordinal(
        &mut self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        lifecycle_ordinal: u128,
    ) -> Result<(), EffectExecutorError> {
        self.durable_validate_retry_seals
            .get_mut(&key)
            .ok_or_else(|| {
                EffectExecutorError::Contract(
                    "committed Validate admission lost its retry authority".to_owned(),
                )
            })?
            .bind_lifecycle_ordinal(lifecycle_ordinal)
            .map_err(EffectExecutorError::Contract)
    }

    /// Execute the exact output service callback after lifecycle ordering grants the row.
    fn execute_lifecycle_output_service<S: V2EffectServices>(
        &mut self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
        services: &mut S,
    ) -> Result<LifecycleOutputServiceDispositionV1, EffectExecutorError> {
        match effect {
            AdapterEffect::Broadcast(message) => {
                message
                    .validate_version()
                    .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
                let proposal_round = match &message.payload {
                    wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                        if proposal.round.context_id != self.context.id()
                            || proposal.round.height != self.context.height
                        {
                            return Err(EffectExecutorError::Contract(
                                "outbound Proposal changed the frozen height context".to_owned(),
                            ));
                        }
                        Some(proposal.round)
                    }
                    _ => None,
                };
                let disposition = services
                    .broadcast_consensus(message.clone())
                    .map_err(service_error)?;
                if let Some(proposal_round) = proposal_round
                    && disposition == ConsensusBroadcastDisposition::ExactServiceAccepted
                {
                    self.runtime
                        .complete_active_view_producer_after_proposal_fanout(
                            proposal_round,
                            ownership,
                        )
                        .map_err(EffectExecutorError::Runtime)?;
                }
                Ok(match disposition {
                    ConsensusBroadcastDisposition::ExactServiceAccepted => {
                        LifecycleOutputServiceDispositionV1::Accepted
                    }
                    ConsensusBroadcastDisposition::SourceRetained => {
                        LifecycleOutputServiceDispositionV1::SourceRetained
                    }
                })
            }
            AdapterEffect::ReportEquivocation { evidence } => {
                evidence
                    .validate_structure(&self.context)
                    .map_err(|reason| {
                        EffectExecutorError::Contract(format!(
                            "ReportEquivocation carried invalid evidence: {reason}"
                        ))
                    })?;
                services
                    .report_equivocation(evidence.to_wire())
                    .map_err(service_error)?;
                Ok(LifecycleOutputServiceDispositionV1::Accepted)
            }
            AdapterEffect::ReportInvalidCertifiedBody {
                subject,
                certificate,
            } => {
                services
                    .report_invalid_certified_body(*subject, certificate.clone())
                    .map_err(service_error)?;
                Ok(LifecycleOutputServiceDispositionV1::Accepted)
            }
            AdapterEffect::Sign { .. }
            | AdapterEffect::FetchBody { .. }
            | AdapterEffect::StoreBody { .. }
            | AdapterEffect::ValidateBody { .. }
            | AdapterEffect::Apply { .. }
            | AdapterEffect::EnterView { .. } => Err(EffectExecutorError::Contract(
                "non-output effect crossed the lifecycle output settlement seam".to_owned(),
            )),
        }
    }
}

/// Bounded outcome census for one lifecycle-output settlement pass.
///
/// Only a newly completed output crossed service I/O and the terminal durability
/// boundary. Exact terminal duplicates stutter before service I/O, so they must
/// not by themselves preempt the following serialized ingress turn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[must_use = "the lifecycle-output settlement census controls executor yielding"]
pub(in crate::sumeragi) struct LifecycleOutputAdmissionSettlementSummaryV1 {
    newly_completed: usize,
    already_completed: usize,
}

impl LifecycleOutputAdmissionSettlementSummaryV1 {
    #[cfg(test)]
    pub(in crate::sumeragi) const fn newly_completed(self) -> usize {
        self.newly_completed
    }

    #[cfg(test)]
    pub(in crate::sumeragi) const fn already_completed(self) -> usize {
        self.already_completed
    }

    pub(in crate::sumeragi) const fn requires_outer_executor_yield(self) -> bool {
        self.newly_completed > 0
    }
}

impl V2EffectExecutor<SerializedV2Runtime> {
    /// Return whether a signed/diagnostic output is parked at the lifecycle cut.
    pub(in crate::sumeragi) fn has_pending_lifecycle_output_admissions(&self) -> bool {
        !self.pending_lifecycle_output_admissions.is_empty()
    }

    /// Install one exact pending output for a production-seam regression.
    #[cfg(test)]
    pub(in crate::sumeragi) fn install_pending_lifecycle_output_for_test(
        &mut self,
        pending: PendingLifecycleOutputAdmissionV1,
    ) -> bool {
        let key = pending.key();
        match self.pending_lifecycle_output_admissions.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(pending);
                true
            }
            std::collections::btree_map::Entry::Occupied(_) => false,
        }
    }

    /// Settle exactly one registry-attested direct Broadcast after Apply terminal settlement.
    ///
    /// Unlike the ordinary Runtime drain, this consumes only the pending-map key
    /// sealed by the Ready `PendingAdapter` carrier. Later Broadcasts and all
    /// diagnostic outputs remain untouched under the terminal-Apply fence.
    pub(in crate::sumeragi) fn settle_apply_terminal_direct_broadcast<S: V2EffectServices>(
        &mut self,
        owner: &mut ProductionLifecycleOwnerV1,
        services: &mut S,
        prepared: crate::sumeragi::v2_lifecycle_coordinator::PreparedApplyTerminalDirectBroadcastV1,
    ) -> Result<ProductionApplyTerminalDirectBroadcastSettlementV1, EffectExecutorError> {
        self.ensure_open()?;
        let key = prepared.pending_key();
        if self
            .lifecycle_decision_apply_successor_outputs
            .as_ref()
            .is_some_and(|attestation| {
                !attestation.exactly_matches_terminal_preparation(&prepared)
            })
        {
            let error = EffectExecutorError::Contract(format!(
                "post-Apply direct Broadcast ordinal {} changed its attested pending owner",
                prepared.ordinal()
            ));
            return Err(self.close(error, services));
        }
        let Some(pending) = self.pending_lifecycle_output_admissions.remove(&key) else {
            let error = EffectExecutorError::Contract(format!(
                "post-Apply direct Broadcast ordinal {} lost its exact pending owner",
                prepared.ordinal()
            ));
            return Err(self.close(error, services));
        };
        let settlement =
            owner.settle_apply_terminal_direct_broadcast(prepared, pending, |effect, ownership| {
                self.execute_lifecycle_output_service(effect, ownership, services)
            });
        match settlement {
            ProductionLifecycleOutputAdmissionSettlementV1::Completed => {
                // The sealed proof is singleton and was checked before output
                // service. Consume it only after the terminal publication and
                // service transaction have completed.
                self.lifecycle_decision_apply_successor_outputs = None;
                Ok(ProductionApplyTerminalDirectBroadcastSettlementV1::Completed)
            }
            ProductionLifecycleOutputAdmissionSettlementV1::Deferred(pending) => {
                if pending.key() != key
                    || self
                        .pending_lifecycle_output_admissions
                        .insert(key, pending)
                        .is_some()
                {
                    let error = EffectExecutorError::Contract(
                        "post-Apply direct Broadcast retry changed or collided with its pending key"
                            .to_owned(),
                    );
                    return Err(self.close(error, services));
                }
                Ok(ProductionApplyTerminalDirectBroadcastSettlementV1::SourceRetained)
            }
            ProductionLifecycleOutputAdmissionSettlementV1::AlreadyCompleted => {
                let error = EffectExecutorError::Contract(
                    "Ready post-Apply direct Broadcast unexpectedly terminal-stuttered".to_owned(),
                );
                Err(self.close(error, services))
            }
            ProductionLifecycleOutputAdmissionSettlementV1::Failed { failure, pending } => {
                let pending_key = pending.key();
                let collision = pending_key != key
                    || self
                        .pending_lifecycle_output_admissions
                        .insert(key, pending)
                        .is_some();
                let error = if collision {
                    EffectExecutorError::Contract(
                        "failed post-Apply direct Broadcast changed or collided with its pending key"
                            .to_owned(),
                    )
                } else {
                    match failure {
                        ProductionLifecycleOutputAdmissionFailureV1::Service(error) => error,
                        ProductionLifecycleOutputAdmissionFailureV1::Projection(error) => {
                            EffectExecutorError::Contract(format!(
                                "post-Apply direct Broadcast projection failed: {error:?}"
                            ))
                        }
                        ProductionLifecycleOutputAdmissionFailureV1::Registry(reason) => {
                            EffectExecutorError::Contract(format!(
                                "post-Apply direct Broadcast attestation failed: {reason:?}"
                            ))
                        }
                        ProductionLifecycleOutputAdmissionFailureV1::Durability => {
                            EffectExecutorError::Contract(
                                "post-Apply direct Broadcast terminal publication failed"
                                    .to_owned(),
                            )
                        }
                    }
                };
                Err(self.close(error, services))
            }
        }
    }

    /// Settle each initially parked lifecycle output once in binding-key order.
    pub(in crate::sumeragi) fn settle_pending_lifecycle_output_admissions<S: V2EffectServices>(
        &mut self,
        owner: &mut ProductionLifecycleOwnerV1,
        services: &mut S,
    ) -> Result<LifecycleOutputAdmissionSettlementSummaryV1, EffectExecutorError> {
        self.ensure_open()?;
        let keys = self
            .pending_lifecycle_output_admissions
            .keys()
            .copied()
            .collect::<Vec<_>>();
        let mut summary = LifecycleOutputAdmissionSettlementSummaryV1::default();
        for key in keys {
            let Some(pending) = self.pending_lifecycle_output_admissions.remove(&key) else {
                continue;
            };
            let settlement =
                owner.settle_lifecycle_output_admission(pending, |effect, ownership| {
                    self.execute_lifecycle_output_service(effect, ownership, services)
                });
            match settlement {
                ProductionLifecycleOutputAdmissionSettlementV1::Completed => {
                    summary.newly_completed = summary.newly_completed.saturating_add(1);
                }
                ProductionLifecycleOutputAdmissionSettlementV1::AlreadyCompleted => {
                    summary.already_completed = summary.already_completed.saturating_add(1);
                }
                ProductionLifecycleOutputAdmissionSettlementV1::Deferred(pending) => {
                    let previous = self
                        .pending_lifecycle_output_admissions
                        .insert(key, pending);
                    debug_assert!(previous.is_none());
                }
                ProductionLifecycleOutputAdmissionSettlementV1::Failed { failure, pending } => {
                    let previous = self
                        .pending_lifecycle_output_admissions
                        .insert(key, pending);
                    debug_assert!(previous.is_none());
                    let error = match failure {
                        ProductionLifecycleOutputAdmissionFailureV1::Service(error) => error,
                        ProductionLifecycleOutputAdmissionFailureV1::Projection(error) => {
                            EffectExecutorError::Contract(format!(
                                "lifecycle output admission projection failed: {error:?}"
                            ))
                        }
                        ProductionLifecycleOutputAdmissionFailureV1::Registry(reason) => {
                            EffectExecutorError::Contract(format!(
                                "lifecycle output registry settlement failed: {reason:?}"
                            ))
                        }
                        ProductionLifecycleOutputAdmissionFailureV1::Durability => {
                            EffectExecutorError::Contract(
                                "lifecycle output terminal publication failed".to_owned(),
                            )
                        }
                    };
                    return Err(self.close(error, services));
                }
            }
        }
        Ok(summary)
    }

    /// Return whether an exact durable Validate owner is parked at lifecycle admission.
    pub(in crate::sumeragi) fn has_pending_durable_validate_admissions(&self) -> bool {
        !self.pending_durable_validate_admissions.is_empty()
    }

    /// Return whether one exact post-fsync live-WAL Sign is parked at lifecycle admission.
    pub(in crate::sumeragi) fn has_pending_live_wal_sign_admission(&self) -> bool {
        self.pending_live_wal_sign_admission.is_some()
    }

    /// Settle the exact post-fsync live-WAL Sign before generic signed-output dispatch.
    pub(in crate::sumeragi) fn settle_pending_live_wal_sign_admission<S: V2EffectServices>(
        &mut self,
        owner: &mut ProductionLifecycleOwnerV1,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        self.ensure_open()?;
        let Some(pending) = self.pending_live_wal_sign_admission.take() else {
            return Ok(0);
        };
        match owner.settle_live_wal_sign_admission(pending) {
            ProductionLiveWalSignAdmissionSettlementV1::Admitted(AdmissionDecision::Admitted {
                ..
            })
            | ProductionLiveWalSignAdmissionSettlementV1::Rebound(AdmissionDecision::Retry {
                ..
            }) => Ok(1),
            ProductionLiveWalSignAdmissionSettlementV1::Admitted(decision)
            | ProductionLiveWalSignAdmissionSettlementV1::Rebound(decision) => Err(self.close(
                EffectExecutorError::Contract(format!(
                    "live WAL Sign settlement committed an invalid logical decision: {decision:?}"
                )),
                services,
            )),
            ProductionLiveWalSignAdmissionSettlementV1::Returned {
                decision: AdmissionDecision::WaitForCapacity(_),
                pending,
            } => {
                self.pending_live_wal_sign_admission = Some(pending);
                Ok(0)
            }
            ProductionLiveWalSignAdmissionSettlementV1::Returned {
                decision:
                    AdmissionDecision::Retry { .. }
                    | AdmissionDecision::ReplayTerminal { .. }
                    | AdmissionDecision::StutterTerminal { .. },
                pending: _,
            } => Ok(0),
            ProductionLiveWalSignAdmissionSettlementV1::Returned { decision, pending } => {
                self.pending_live_wal_sign_admission = Some(pending);
                Err(self.close(
                    EffectExecutorError::Contract(format!(
                        "live WAL Sign admission returned a terminally invalid decision: {decision:?}"
                    )),
                    services,
                ))
            }
            ProductionLiveWalSignAdmissionSettlementV1::Failed { failure, pending } => {
                self.pending_live_wal_sign_admission = Some(pending);
                let error = match failure {
                    ProductionLiveWalSignAdmissionFailureV1::Projection(error) => {
                        EffectExecutorError::Contract(format!(
                            "live WAL Sign admission projection failed: {error:?}"
                        ))
                    }
                    ProductionLiveWalSignAdmissionFailureV1::Registry => {
                        EffectExecutorError::Contract(
                            "live WAL Sign registry settlement failed".to_owned(),
                        )
                    }
                    ProductionLiveWalSignAdmissionFailureV1::Durability => {
                        EffectExecutorError::Contract(
                            "live WAL Sign admission publication failed".to_owned(),
                        )
                    }
                };
                Err(self.close(error, services))
            }
        }
    }

    /// Settle each currently pending durable Validate owner once, in body-key order.
    ///
    /// Capacity waits restore the exact move-only owner. An exact logical
    /// `Retry` consumes the duplicate because the incumbent ordinal and
    /// registry carrier already own execution. Any other non-committing
    /// decision is a production invariant violation and closes the shared
    /// output gate while preserving the pending owner for restart diagnosis.
    pub(in crate::sumeragi) fn settle_pending_durable_validate_admissions<S: V2EffectServices>(
        &mut self,
        owner: &mut ProductionLifecycleOwnerV1,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        self.ensure_open()?;
        let pending_keys = self
            .pending_durable_validate_admissions
            .keys()
            .copied()
            .collect::<Vec<_>>();
        let mut made_ready = 0usize;
        for key in pending_keys {
            let Some(pending) = self.pending_durable_validate_admissions.remove(&key) else {
                continue;
            };
            match owner.settle_durable_validate_admission(pending) {
                ProductionDurableValidateAdmissionSettlementV1::Admitted(
                    AdmissionDecision::Admitted { ordinal, .. },
                )
                | ProductionDurableValidateAdmissionSettlementV1::Rebound(
                    AdmissionDecision::Retry { ordinal, .. },
                ) => {
                    if let Err(error) = self.bind_validate_retry_lifecycle_ordinal(key, ordinal) {
                        return Err(self.close(error, services));
                    }
                    made_ready = made_ready.saturating_add(1);
                }
                ProductionDurableValidateAdmissionSettlementV1::Admitted(decision)
                | ProductionDurableValidateAdmissionSettlementV1::Rebound(decision) => {
                    return Err(self.close(
                        EffectExecutorError::Contract(format!(
                            "durable Validate settlement committed an invalid logical decision: {decision:?}"
                        )),
                        services,
                    ));
                }
                ProductionDurableValidateAdmissionSettlementV1::Returned {
                    decision: AdmissionDecision::WaitForCapacity(_),
                    pending,
                } => {
                    let previous = self
                        .pending_durable_validate_admissions
                        .insert(key, pending);
                    debug_assert!(previous.is_none());
                }
                ProductionDurableValidateAdmissionSettlementV1::Returned {
                    decision: AdmissionDecision::Retry { ordinal, .. },
                    pending: _,
                } => {
                    if let Err(error) = self.bind_validate_retry_lifecycle_ordinal(key, ordinal) {
                        return Err(self.close(error, services));
                    }
                }
                ProductionDurableValidateAdmissionSettlementV1::Returned {
                    decision:
                        AdmissionDecision::ReplayTerminal { .. }
                        | AdmissionDecision::StutterTerminal { .. },
                    pending: _,
                } => {
                    if self.durable_validate_retry_seals.remove(&key).is_none() {
                        return Err(self.close(
                            EffectExecutorError::Contract(
                                "terminal Validate admission lost its transient retry authority"
                                    .to_owned(),
                            ),
                            services,
                        ));
                    }
                }
                ProductionDurableValidateAdmissionSettlementV1::Returned { decision, pending } => {
                    let previous = self
                        .pending_durable_validate_admissions
                        .insert(key, pending);
                    debug_assert!(previous.is_none());
                    return Err(self.close(
                        EffectExecutorError::Contract(format!(
                            "durable Validate admission returned a terminally invalid decision: {decision:?}"
                        )),
                        services,
                    ));
                }
                ProductionDurableValidateAdmissionSettlementV1::Failed { failure, pending } => {
                    let previous = self
                        .pending_durable_validate_admissions
                        .insert(key, pending);
                    debug_assert!(previous.is_none());
                    return Err(self.close(
                        EffectExecutorError::Contract(format!(
                            "durable Validate admission failed before commit: {failure:?}"
                        )),
                        services,
                    ));
                }
            }
        }
        Ok(made_ready)
    }
}

impl<R: EffectRuntime> V2EffectExecutor<R> {
    fn payload_chunk_reconstruction_task<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        services: &mut S,
    ) -> Result<BodyFetchTask, EffectTransportError> {
        let task = self
            .pending_fetches
            .get(&work_id)
            .ok_or(EffectTransportError::UnknownWork(work_id))?
            .task
            .clone();
        if task.certified_request.is_none() {
            return Ok(task);
        }
        let key = (task.round, task.subject);
        match self.remote_proposal_replay.get(&key) {
            Some(RemoteProposalReplayStageV1::Fetch {
                work_id: replay_work_id,
                ..
            }) if *replay_work_id == work_id => Ok(task),
            Some(_) => Err(self.fail_closed_transport(
                "certificate-backed payload chunks conflict with the retained Proposal replay stage",
                services,
            )),
            None => {
                // A certificate-only Fetch gets its body-frame replay
                // authority from the authenticated CertifiedBodyResponse
                // lifecycle. Generic chunk reconstruction has no equivalent
                // authority to carry through Store and Validate, so leave
                // the exact request/fetch live for that typed response path.
                Err(EffectTransportError::WrongFetchKind)
            }
        }
    }

    fn accept_payload_chunk_inner<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        chunk: wire::PayloadChunk,
        authenticated_sender: &PeerId,
        services: &mut S,
    ) -> Result<(), EffectTransportError> {
        if self.output_guard.restart_required() {
            return Err(EffectTransportError::FailClosed(
                "process restart is required after a fatal consensus failure".to_owned(),
            ));
        }
        if let Some(reason) = &self.fatal_reason {
            return Err(EffectTransportError::FailClosed(reason.clone()));
        }
        let task = self.payload_chunk_reconstruction_task(work_id, services)?;
        let manifest = task
            .manifest
            .as_ref()
            .ok_or(EffectTransportError::WrongFetchKind)?;
        let authenticated =
            authenticate_payload_chunk(&self.context, manifest, chunk, authenticated_sender)?;
        match services.accept_authenticated_chunk(&task, authenticated) {
            Ok(AuthenticatedChunkDisposition::Accepted) => {}
            Ok(AuthenticatedChunkDisposition::Rejected) => {
                self.reject_noncanonical_reconstruction(work_id, services)?;
                return Err(EffectTransportError::BodyMismatch(
                    "authenticated chunks reconstructed invalid or noncanonical body data",
                ));
            }
            Err(error) => {
                let reason = EffectExecutorError::Service(error.to_string()).to_string();
                self.fatal_reason = Some(reason.clone());
                services.fail_closed(&reason);
                return Err(EffectTransportError::FailClosed(reason));
            }
        }
        Ok(())
    }

    /// Retain the exact resultless wire of the already-authenticated staged
    /// genesis as a process-local acquisition source.
    ///
    /// Genesis is signed once with a fixed view-zero header, while its
    /// consensus Proposal may be reissued in later views with a new manifest.
    /// The opaque installed authority, canonical bytes, and subject remain one
    /// value until the certified Fetch projects its Store replay lineage.
    pub(in crate::sumeragi) fn install_authenticated_genesis_body(
        &mut self,
        authenticated_genesis: &super::v2_context::AuthenticatedGenesisBodyV1,
    ) -> Result<(), EffectExecutorError> {
        self.ensure_open()?;
        let installed = InstalledAuthenticatedGenesisReplayAuthorityV1::install(
            authenticated_genesis,
            &self.context,
        )
        .map_err(|reason| EffectExecutorError::Contract(reason.to_owned()))?;
        let subject = installed.subject();
        let canonical_wire = Arc::clone(installed.canonical_wire());
        let genesis_round = wire::ConsensusRound {
            context_id: self.context.id(),
            height: self.context.height,
            view: 0,
        };
        ReadyBody::derive(
            &self.context,
            genesis_round,
            subject,
            Arc::clone(&canonical_wire),
        )
        .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
        if let Some(retained) = self.authenticated_genesis_body.as_ref() {
            if retained.subject() == subject
                && retained.canonical_wire().as_ref() == canonical_wire.as_ref()
            {
                return Ok(());
            }
            return Err(EffectExecutorError::Contract(
                "authenticated staged genesis changed after executor construction".to_owned(),
            ));
        }
        self.authenticated_genesis_body = Some(installed);
        Ok(())
    }

    /// Install synthetic authenticated-genesis authority for executor fixtures.
    #[cfg(test)]
    fn install_authenticated_genesis_body_for_test(
        &mut self,
        authenticated_genesis: &iroha_data_model::block::SignedBlock,
    ) -> Result<(), EffectExecutorError> {
        self.ensure_open()?;
        let installed = InstalledAuthenticatedGenesisReplayAuthorityV1::for_test(
            authenticated_genesis,
            &self.context,
        )
        .ok_or_else(|| {
            EffectExecutorError::Contract(
                "synthetic authenticated staged genesis is not canonical".to_owned(),
            )
        })?;
        if let Some(retained) = self.authenticated_genesis_body.as_ref() {
            return (retained.subject() == installed.subject()
                && retained.canonical_wire().as_ref() == installed.canonical_wire().as_ref())
            .then_some(())
            .ok_or_else(|| {
                EffectExecutorError::Contract(
                    "authenticated staged genesis changed after executor construction".to_owned(),
                )
            });
        }
        self.authenticated_genesis_body = Some(installed);
        Ok(())
    }

    /// Rebind a later Store carrier to the immutable owner retained by its
    /// exact durable Proposal/genesis replay or by the inert Store terminal
    /// seal which survives that replay's Validate handoff.
    ///
    /// The Store handler repeats this projection when the effect dispatches,
    /// but a queued `BodyStored` terminal is queried while the adapter batch is
    /// still being retained.  Performing the same typed, read-only projection
    /// here prevents that earlier query from presenting a fresh weaker owner
    /// to the runtime without weakening the foreign-stale-owner guard.
    fn stored_replay_incumbent_store_ownership(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        effect: &AdapterEffect,
        incoming: &RuntimeEffectOwnership,
    ) -> Result<Option<RuntimeEffectOwnership>, EffectExecutorError> {
        if self.authenticated_genesis_replay.contains_key(&key)
            && self.remote_proposal_replay.contains_key(&key)
        {
            return Err(EffectExecutorError::Contract(
                "one stored body retained two replay lineages".to_owned(),
            ));
        }
        let replay_retained = match (
            self.remote_proposal_replay.get(&key),
            self.authenticated_genesis_replay.get(&key),
        ) {
            (Some(RemoteProposalReplayStageV1::Stored { replay, ownership }), None) => {
                let receipt = self.durable_bodies.get(&key).ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "stored Proposal replay lost its durable receipt during retention"
                            .to_owned(),
                    )
                })?;
                Some(
                    replay
                        .project_retry_ownership(receipt, ownership, effect, incoming)
                        .ok_or_else(|| {
                            EffectExecutorError::Contract(
                                "stored body replay could not project its incumbent Store owner"
                                    .to_owned(),
                            )
                        })?,
                )
            }
            (None, Some(AuthenticatedGenesisReplayStageV1::Stored { replay, ownership })) => {
                let receipt = self.durable_bodies.get(&key).ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "stored authenticated-genesis replay lost its durable receipt during retention"
                            .to_owned(),
                    )
                })?;
                Some(
                    replay
                        .project_retry_ownership(receipt, ownership, effect, incoming)
                        .ok_or_else(|| {
                            EffectExecutorError::Contract(
                                "stored body replay could not project its incumbent Store owner"
                                    .to_owned(),
                            )
                        })?,
                )
            }
            _ => None,
        };
        let sealed_retained = self
            .durable_validate_retry_seals
            .get(&key)
            .map(|seal| {
                let receipt = self.durable_bodies.get(&key).ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "post-Validate Store terminal lost its durable receipt".to_owned(),
                    )
                })?;
                seal.project_store_terminal_retry(receipt, effect, incoming)
                    .map_err(EffectExecutorError::Contract)
            })
            .transpose()?
            .flatten();
        if sealed_retained.is_some()
            && (self.remote_proposal_replay.contains_key(&key)
                || self.authenticated_genesis_replay.contains_key(&key))
        {
            return Err(EffectExecutorError::Contract(
                "one durable Store retained both a replay phase and post-Validate terminal owner"
                    .to_owned(),
            ));
        }
        Ok(replay_retained.or(sealed_retained))
    }

    fn stored_replay_incumbent_validate_ownership(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        effect: &AdapterEffect,
    ) -> Result<Option<RuntimeEffectOwnership>, EffectExecutorError> {
        if self.authenticated_genesis_replay.contains_key(&key)
            && self.remote_proposal_replay.contains_key(&key)
        {
            return Err(EffectExecutorError::Contract(
                "one stored body retained two replay lineages".to_owned(),
            ));
        }
        let Some(receipt) = self.durable_bodies.get(&key) else {
            if self.authenticated_genesis_replay.contains_key(&key)
                || self.remote_proposal_replay.contains_key(&key)
            {
                return Err(EffectExecutorError::Contract(
                    "stored body replay lost its durable receipt during retention".to_owned(),
                ));
            }
            return Ok(None);
        };
        let incumbent = match (
            self.remote_proposal_replay.get(&key),
            self.authenticated_genesis_replay.get(&key),
        ) {
            (Some(RemoteProposalReplayStageV1::Stored { replay, ownership }), None) => {
                replay.project_incumbent_validate_ownership(receipt, ownership, effect)
            }
            (None, Some(AuthenticatedGenesisReplayStageV1::Stored { replay, ownership })) => {
                replay.project_incumbent_validate_ownership(receipt, ownership, effect)
            }
            _ => return Ok(None),
        };
        incumbent.map(Some).ok_or_else(|| {
            EffectExecutorError::Contract(
                "stored body replay could not project its incumbent Validate owner".to_owned(),
            )
        })
    }

    /// Rejoin a refined Proposal body owner to the complete durable QC behind it.
    ///
    /// Runtime candidate statements are intentionally process-local. They may
    /// prove positional refinement, but only the reducer-authenticated QC may
    /// become replay authority in a published lifecycle row.
    fn exact_remote_proposal_validate_authority_certificate(
        &self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<Option<wire::QuorumCertificate>, EffectExecutorError> {
        let pending = ownership
            .exact_pending_adapter_effect_binding(effect)
            .map_err(|_| {
                EffectExecutorError::Contract(
                    "Proposal Validate lost its exact runtime binding before replay refinement"
                        .to_owned(),
                )
            })?;
        let statement = pending.candidate_statement().ok_or_else(|| {
            EffectExecutorError::Contract(
                "Proposal Validate omitted its route-neutral candidate statement".to_owned(),
            )
        })?;
        let Some(phase) = statement.phase() else {
            return statement
                .execution_commitment()
                .is_none()
                .then_some(None)
                .ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "ordinary Proposal Validate carried a commitment without quorum authority"
                            .to_owned(),
                    )
                });
        };
        let certificate = self
            .runtime
            .durable_body_authority_certificate()
            .map_err(EffectExecutorError::Runtime)?
            .ok_or_else(|| {
                EffectExecutorError::Contract(
                    "authority-refined Proposal Validate omitted its durable QC".to_owned(),
                )
            })?;
        self.runtime
            .verify_certificate(&self.context, &certificate)
            .map_err(|reason| {
                EffectExecutorError::Contract(format!(
                    "authority-refined Proposal Validate carried an invalid durable QC: {reason}"
                ))
            })?;
        if certificate.phase != phase
            || certificate.round != statement.round()
            || certificate.proposal_round != statement.proposal_round()
            || certificate.subject != statement.subject().expect("body statement has one subject")
            || Some(certificate.execution_commitment) != statement.execution_commitment()
        {
            return Err(EffectExecutorError::Contract(
                "authority-refined Proposal Validate changed its durable QC coordinates".to_owned(),
            ));
        }
        let protected = match phase {
            wire::GlobalPhase::Prepare => {
                self.protected_decision.is_none()
                    && self.protected_lock
                        == Some((certificate.proposal_round, certificate.subject))
            }
            wire::GlobalPhase::Commit => {
                self.protected_decision
                    == Some((
                        certificate.round,
                        certificate.proposal_round,
                        certificate.subject,
                        certificate.execution_commitment,
                    ))
            }
        };
        if !protected {
            return Err(EffectExecutorError::Contract(
                "authority-refined Proposal Validate is not the protected durable body".to_owned(),
            ));
        }
        Ok(Some(certificate))
    }

    fn preflight_authenticated_genesis_store_completion(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        pending: &PendingStore,
        work_id: EffectWorkId,
    ) -> Result<bool, EffectExecutorError> {
        match self.authenticated_genesis_replay.get(&key) {
            Some(AuthenticatedGenesisReplayStageV1::Store {
                work_id: retained,
                replay,
            }) => {
                let effect = AdapterEffect::StoreBody {
                    tag: pending.task.tag(),
                    round: key.0,
                    subject: key.1,
                };
                if *retained != work_id
                    || !replay.exactly_matches_retry(&effect, pending.task.ownership())
                {
                    return Err(EffectExecutorError::Contract(
                        "authenticated-genesis Store completion changed its replay owner"
                            .to_owned(),
                    ));
                }
                Ok(true)
            }
            Some(AuthenticatedGenesisReplayStageV1::BodyAvailable(_))
            | Some(AuthenticatedGenesisReplayStageV1::StoreAdmission(_)) => {
                Err(EffectExecutorError::Contract(
                    "authenticated-genesis Store completion preceded its retained stage".to_owned(),
                ))
            }
            Some(AuthenticatedGenesisReplayStageV1::Stored { .. }) | None => Ok(false),
        }
    }

    fn commit_authenticated_genesis_store_completion(
        &mut self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        work_id: EffectWorkId,
        receipt: DurableBodyReceipt,
        ownership: RuntimeEffectOwnership,
    ) -> Result<(), EffectExecutorError> {
        let Some(AuthenticatedGenesisReplayStageV1::Store {
            work_id: retained,
            replay,
        }) = self.authenticated_genesis_replay.remove(&key)
        else {
            return Err(EffectExecutorError::Contract(
                "preflighted authenticated-genesis Store replay disappeared".to_owned(),
            ));
        };
        if retained != work_id {
            let previous = self.authenticated_genesis_replay.insert(
                key,
                AuthenticatedGenesisReplayStageV1::Store {
                    work_id: retained,
                    replay,
                },
            );
            debug_assert!(previous.is_none());
            return Err(EffectExecutorError::Contract(
                "authenticated-genesis Store work ID changed before commit".to_owned(),
            ));
        }
        let stored = match replay.bind_durable_body(receipt.clone()) {
            Ok(stored) => stored,
            Err(error) => {
                let previous = self.authenticated_genesis_replay.insert(
                    key,
                    AuthenticatedGenesisReplayStageV1::Store {
                        work_id,
                        replay: error.into_store(),
                    },
                );
                debug_assert!(previous.is_none());
                return Err(EffectExecutorError::Contract(
                    "authenticated-genesis Store completion changed its durable body".to_owned(),
                ));
            }
        };
        if !stored.exactly_retains_owned_store(&receipt, &ownership) {
            return Err(EffectExecutorError::Contract(
                "authenticated-genesis Store completion changed its runtime owner".to_owned(),
            ));
        }
        let previous = self.authenticated_genesis_replay.insert(
            key,
            AuthenticatedGenesisReplayStageV1::Stored {
                replay: stored,
                ownership,
            },
        );
        debug_assert!(previous.is_none());
        Ok(())
    }

    fn prepare_authenticated_genesis_store_replay(
        &mut self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<AuthenticatedGenesisStoreReplayDispositionV1, EffectExecutorError> {
        if self.authenticated_genesis_replay.contains_key(&key)
            && self.remote_proposal_replay.contains_key(&key)
        {
            return Err(EffectExecutorError::Contract(
                "one body stage retained both Proposal and authenticated-genesis replay".to_owned(),
            ));
        }
        match self.authenticated_genesis_replay.get(&key) {
            Some(AuthenticatedGenesisReplayStageV1::StoreAdmission(replay)) => {
                if !replay.exactly_matches_retry(effect, ownership) {
                    return Err(EffectExecutorError::Contract(
                        "authenticated-genesis Store retry changed its projected replay owner"
                            .to_owned(),
                    ));
                }
                return Ok(AuthenticatedGenesisStoreReplayDispositionV1::Advance);
            }
            Some(AuthenticatedGenesisReplayStageV1::Store { work_id, replay }) => {
                let adopted = self
                    .pending_stores
                    .get(work_id)
                    .filter(|pending| {
                        (pending.task.manifest.round, pending.task.manifest.subject) == key
                    })
                    .and_then(|pending| {
                        replay.project_retry_ownership(pending.task.ownership(), effect, ownership)
                    })
                    .ok_or_else(|| {
                        EffectExecutorError::Contract(
                            "authenticated-genesis Store retry changed its exact replay owner"
                                .to_owned(),
                        )
                    })?;
                return Ok(AuthenticatedGenesisStoreReplayDispositionV1::Retry(adopted));
            }
            Some(AuthenticatedGenesisReplayStageV1::Stored {
                replay,
                ownership: stored_ownership,
            }) => {
                let receipt = self.durable_bodies.get(&key).ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "durable authenticated-genesis replay lost its body receipt".to_owned(),
                    )
                })?;
                let adopted = replay
                    .project_retry_ownership(receipt, stored_ownership, effect, ownership)
                    .ok_or_else(|| {
                        EffectExecutorError::Contract(
                            "durable authenticated-genesis Store retry changed its replay owner"
                                .to_owned(),
                        )
                    })?;
                return Ok(AuthenticatedGenesisStoreReplayDispositionV1::Retry(adopted));
            }
            Some(AuthenticatedGenesisReplayStageV1::BodyAvailable(_)) | None => {}
        }
        let Some(AuthenticatedGenesisReplayStageV1::BodyAvailable(fetch)) =
            self.authenticated_genesis_replay.remove(&key)
        else {
            return Ok(AuthenticatedGenesisStoreReplayDispositionV1::None);
        };
        let store = match fetch.project_store(effect.clone(), ownership.clone()) {
            Ok(store) => store,
            Err(error) => {
                let previous = self.authenticated_genesis_replay.insert(
                    key,
                    AuthenticatedGenesisReplayStageV1::BodyAvailable(error.into_fetch()),
                );
                debug_assert!(previous.is_none());
                return Err(EffectExecutorError::Contract(
                    "authenticated-genesis Fetch could not project its exact Store successor"
                        .to_owned(),
                ));
            }
        };
        let previous = self.authenticated_genesis_replay.insert(
            key,
            AuthenticatedGenesisReplayStageV1::StoreAdmission(store),
        );
        debug_assert!(previous.is_none());
        Ok(AuthenticatedGenesisStoreReplayDispositionV1::Advance)
    }

    fn commit_authenticated_genesis_store_replay(
        &mut self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        ownership: RuntimeEffectOwnership,
    ) -> Result<(), EffectExecutorError> {
        let Some(AuthenticatedGenesisReplayStageV1::StoreAdmission(store)) =
            self.authenticated_genesis_replay.remove(&key)
        else {
            return Err(EffectExecutorError::Contract(
                "serialized Store lost its authenticated-genesis replay stage".to_owned(),
            ));
        };
        let stage = if let Some(receipt) = self.durable_bodies.get(&key).cloned() {
            let stored = match store.bind_durable_body(receipt.clone()) {
                Ok(stored) => stored,
                Err(error) => {
                    let previous = self.authenticated_genesis_replay.insert(
                        key,
                        AuthenticatedGenesisReplayStageV1::StoreAdmission(error.into_store()),
                    );
                    debug_assert!(previous.is_none());
                    return Err(EffectExecutorError::Contract(
                        "authenticated-genesis Store could not bind its durable body".to_owned(),
                    ));
                }
            };
            if !stored.exactly_retains_owned_store(&receipt, &ownership) {
                return Err(EffectExecutorError::Contract(
                    "authenticated-genesis Store changed its retained runtime owner".to_owned(),
                ));
            }
            AuthenticatedGenesisReplayStageV1::Stored {
                replay: stored,
                ownership,
            }
        } else {
            let Some(work_id) = self.pending_stores.iter().find_map(|(work_id, pending)| {
                (pending.task.manifest.round == key.0 && pending.task.manifest.subject == key.1)
                    .then_some(*work_id)
            }) else {
                let previous = self.authenticated_genesis_replay.insert(
                    key,
                    AuthenticatedGenesisReplayStageV1::StoreAdmission(store),
                );
                debug_assert!(previous.is_none());
                return Err(EffectExecutorError::Contract(
                    "authenticated-genesis Store installed neither durable nor pending work"
                        .to_owned(),
                ));
            };
            AuthenticatedGenesisReplayStageV1::Store {
                work_id,
                replay: store,
            }
        };
        let previous = self.authenticated_genesis_replay.insert(key, stage);
        debug_assert!(previous.is_none());
        Ok(())
    }

    fn commit_remote_proposal_body_available_replay(
        &mut self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        replay: Option<PreparedRemoteProposalFetchReplayPreAdmission>,
    ) {
        if let Some(replay) = replay {
            let previous = self
                .remote_proposal_replay
                .insert(key, RemoteProposalReplayStageV1::BodyAvailable(replay));
            assert!(
                previous.is_none(),
                "remote Proposal replay preflight keeps its body key vacant"
            );
        }
    }

    /// Prove every signed-Proposal replay token is attached to its exact
    /// executor-owned physical stage before a view or Decision retires it.
    fn preflight_remote_proposal_replay_indexes(&self) -> Result<(), EffectExecutorError> {
        if self
            .authenticated_genesis_replay
            .keys()
            .any(|key| self.remote_proposal_replay.contains_key(key))
        {
            return Err(EffectExecutorError::Contract(
                "one physical body stage retained two replay lineages".to_owned(),
            ));
        }
        for (key, stage) in &self.remote_proposal_replay {
            let exact = match stage {
                RemoteProposalReplayStageV1::Fetch { work_id, .. } => self
                    .pending_fetches
                    .get(work_id)
                    .is_some_and(|pending| (pending.task.round, pending.task.subject) == *key),
                RemoteProposalReplayStageV1::BodyAvailable(_) => {
                    self.body_pipeline_owners.contains_key(key)
                        && self.retained_body_manifest_hash(*key)?.is_some()
                }
                // StoreAdmission exists only inside one serialized StoreBody
                // call. Observing it at a later control boundary would mean
                // the move-only projection escaped that transaction.
                RemoteProposalReplayStageV1::StoreAdmission(_) => false,
                RemoteProposalReplayStageV1::Store { work_id, .. } => {
                    self.pending_stores.get(work_id).is_some_and(|pending| {
                        (pending.task.manifest.round, pending.task.manifest.subject) == *key
                    })
                }
                RemoteProposalReplayStageV1::Stored { replay, ownership } => {
                    self.durable_bodies.get(key).is_some_and(|receipt| {
                        (receipt.round(), receipt.subject()) == *key
                            && replay.exactly_retains_owned_store(receipt, ownership)
                    })
                }
            };
            if !exact {
                return Err(EffectExecutorError::Contract(
                    "remote Proposal replay is detached from its exact physical body stage"
                        .to_owned(),
                ));
            }
        }
        for (key, stage) in &self.authenticated_genesis_replay {
            let exact = match stage {
                AuthenticatedGenesisReplayStageV1::BodyAvailable(_) => {
                    self.body_pipeline_owners.contains_key(key)
                        && self.retained_body_manifest_hash(*key)?.is_some()
                }
                AuthenticatedGenesisReplayStageV1::StoreAdmission(_) => false,
                AuthenticatedGenesisReplayStageV1::Store { work_id, replay } => {
                    self.pending_stores.get(work_id).is_some_and(|pending| {
                        let effect = AdapterEffect::StoreBody {
                            tag: pending.task.tag(),
                            round: key.0,
                            subject: key.1,
                        };
                        (pending.task.manifest.round, pending.task.manifest.subject) == *key
                            && replay.exactly_matches_retry(&effect, pending.task.ownership())
                    })
                }
                AuthenticatedGenesisReplayStageV1::Stored { replay, ownership } => {
                    self.durable_bodies.get(key).is_some_and(|receipt| {
                        (receipt.round(), receipt.subject()) == *key
                            && replay.exactly_retains_owned_store(receipt, ownership)
                    })
                }
            };
            if !exact {
                return Err(EffectExecutorError::Contract(
                    "authenticated-genesis replay is detached from its physical body stage"
                        .to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn validate_body<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        ownership: RuntimeEffectOwnership,
        _services: &mut S,
    ) -> Result<Option<super::v2::PendingKuraValidatedApplySuccessorV1>, EffectExecutorError> {
        let key = (round, subject);
        let effect = AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        };
        if let Some(pending) = self.pending_durable_validate_admissions.get(&key) {
            if !pending.exactly_matches_retry(&effect, &ownership) {
                return Err(EffectExecutorError::Contract(
                    "ValidateBody retry changed its exact pending lifecycle owner".to_owned(),
                ));
            }
            return Ok(None);
        }
        if let Some(marker) = self
            .published_lifecycle_validate_retry_markers
            .get(&key)
            .cloned()
        {
            let projected = marker
                .project_retry(&effect, &ownership)
                .map_err(EffectExecutorError::Contract)?;
            let decision = self.protected_decision;
            let validated_receipt = self.validated_bodies.get(&key).cloned();
            let readmit_protected_decision = decision.is_some_and(|decision| {
                self.published_lifecycle_validate_retry_markers
                    .get(&key)
                    == Some(&projected)
                    && self
                        .runtime
                        .has_exact_pending_live_decision_apply(tag, decision)
                    && validated_receipt.as_ref().is_some_and(|validated| {
                        projected.is_unbound_exact_decision_owner(decision, validated)
                    })
            });
            if !readmit_protected_decision {
                let retained = self
                    .published_lifecycle_validate_retry_markers
                    .get_mut(&key)
                    .expect("projected published Validate marker remains installed");
                *retained = projected;
                return Ok(None);
            }

            let decision = decision.expect("readmission checked one exact protected Decision");
            let validated_receipt = validated_receipt
                .expect("readmission checked one exact cached validation receipt");
            if self.rejected_bodies.contains_key(&key)
                || self.retired_rejected_bodies.contains_key(&key)
                || self.remote_proposal_replay.contains_key(&key)
                || self.authenticated_genesis_replay.contains_key(&key)
                || self.live_lifecycle_validate_successor.is_some()
                || self.live_lifecycle_decision_apply.is_some()
                || !self.pending_applications.is_empty()
                || self.pending_tip_recovery.is_some()
                || self.finality_completion.is_some()
                || self.decision_body_drained
                || self.protected_lock != Some(key)
            {
                return Err(EffectExecutorError::Contract(
                    "terminal published Validate Decision readmission overlapped another outcome or Apply lineage"
                        .to_owned(),
                ));
            }
            let receipt = self.durable_bodies.get(&key).cloned().ok_or_else(|| {
                EffectExecutorError::Contract(
                    "terminal published Validate Decision readmission lost its durable body"
                        .to_owned(),
                )
            })?;
            if receipt != projected.durable_receipt
                || validated_receipt.durable() != &receipt
                || validated_receipt.execution_commitment() != decision.3
            {
                return Err(EffectExecutorError::Contract(
                    "terminal published Validate Decision readmission changed its validation receipt"
                        .to_owned(),
                ));
            }
            let certificate = self
                .exact_remote_proposal_validate_authority_certificate(&effect, &ownership)?
                .filter(|certificate| {
                    certificate.phase == wire::GlobalPhase::Commit
                        && certificate.round == decision.0
                        && certificate.proposal_round == decision.1
                        && certificate.subject == decision.2
                        && certificate.execution_commitment == decision.3
                })
                .ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "terminal published Validate Decision readmission lost its exact CommitQC"
                            .to_owned(),
                    )
                })?;
            let (manifest, recovered_receipt) =
                self.recovered_bodies.get(&key).cloned().ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "terminal published Validate Decision readmission lost its recovered body frame"
                            .to_owned(),
                    )
                })?;
            if recovered_receipt != receipt {
                return Err(EffectExecutorError::Contract(
                    "terminal published Validate Decision readmission changed its recovered body receipt"
                        .to_owned(),
                ));
            }
            let store_terminal = DurableStoreTerminalRetrySealV1::seal_validate_predecessor(
                &effect,
                &ownership,
                &receipt,
            )
            .ok_or_else(|| {
                EffectExecutorError::Contract(
                    "terminal published Validate Decision readmission could not seal its Store predecessor"
                        .to_owned(),
                )
            })?;
            let prepared =
                PreparedLocalBodyValidateReplayPreAdmission::seal_exact_protected_decision_validate(
                    effect.clone(),
                    ownership.clone(),
                    manifest,
                    receipt,
                    &validated_receipt,
                    certificate,
                )
                .map_err(|_| {
                    EffectExecutorError::Contract(
                        "terminal published Validate Decision readmission could not seal exact lifecycle replay"
                            .to_owned(),
                    )
                })?;
            self.replace_terminal_published_validate_with_pending_admission(
                key,
                projected,
                &effect,
                &ownership,
                prepared.into_pending_durable_validate_admission(),
                store_terminal,
            )?;
            return Ok(None);
        }
        if let Some(seal) = self.durable_validate_retry_seals.get_mut(&key) {
            let projected = seal
                .project_retry(&effect, &ownership)
                .map_err(EffectExecutorError::Contract)?;
            *seal = projected.seal;
            return Ok(None);
        }
        let receipt = self.durable_bodies.get(&key).cloned().ok_or_else(|| {
            EffectExecutorError::Contract(
                "ValidateBody has no matching durable body receipt".to_owned(),
            )
        })?;
        if let Some(recovery) = self.pending_tip_recovery.as_ref() {
            if recovery.stage() != PendingKuraApplyRecoveryStage::DeterministicValidation
                || recovery.replay_tag() != tag
                || recovery.durable_round() != round
                || recovery.durable_subject() != subject
                || recovery.durable_receipt() != &receipt
                || self.validated_bodies.get(&key) != Some(recovery.validated_receipt())
            {
                return Err(EffectExecutorError::Contract(
                    "PendingKura ValidateBody changed its exact recovered validation owner"
                        .to_owned(),
                ));
            }
            self.ensure_pending_slot()?;
            let _next_apply_work = self.plan_work_id()?;
            let marker = self
                .pending_tip_recovery
                .as_mut()
                .expect("pending-Kura validation was checked above")
                .take_deferred_validated_marker()?;
            let successor = match self
                .runtime
                .commit_pending_kura_validated_apply(marker, &effect, &ownership)
            {
                Ok(successor) => successor,
                Err((marker, error)) => {
                    self.pending_tip_recovery
                        .as_mut()
                        .expect("pending-Kura validation still owns its recovery evidence")
                        .restore_deferred_validated_marker(marker);
                    return Err(EffectExecutorError::PendingApplyRecoveryMismatch(error));
                }
            };
            // The independently fsynced marker now enters the reducer through
            // its real direct successful-validation transition. The returned
            // Apply is the sole predecessor-projected child and is consumed by
            // the outer recovery step only after it records the Apply stage.
            return Ok(Some(successor));
        }
        if self.authenticated_genesis_replay.contains_key(&key)
            && self.remote_proposal_replay.contains_key(&key)
        {
            return Err(EffectExecutorError::Contract(
                "ValidateBody retained two incompatible replay authorities".to_owned(),
            ));
        }
        match self.authenticated_genesis_replay.get(&key) {
            Some(AuthenticatedGenesisReplayStageV1::BodyAvailable(_))
            | Some(AuthenticatedGenesisReplayStageV1::StoreAdmission(_))
            | Some(AuthenticatedGenesisReplayStageV1::Store { .. }) => {
                return Err(EffectExecutorError::Contract(
                    "authenticated-genesis ValidateBody preceded its durable Store replay"
                        .to_owned(),
                ));
            }
            Some(AuthenticatedGenesisReplayStageV1::Stored {
                replay,
                ownership: store_ownership,
            }) => {
                if !replay.exactly_retains_owned_store(&receipt, store_ownership) {
                    return Err(EffectExecutorError::Contract(
                        "authenticated-genesis ValidateBody changed its Store lineage".to_owned(),
                    ));
                }
            }
            None => {}
        }
        if self.authenticated_genesis_replay.contains_key(&key) {
            let Some(AuthenticatedGenesisReplayStageV1::Stored {
                replay: stored,
                ownership: store_ownership,
            }) = self.authenticated_genesis_replay.remove(&key)
            else {
                unreachable!("preflighted authenticated-genesis Store replay remains installed")
            };
            let store_terminal = stored
                .seal_store_terminal_retry(&receipt, &store_ownership)
                .ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "authenticated-genesis Validate could not seal its durable Store terminal"
                            .to_owned(),
                    )
                })?;
            let validate_ownership = ownership;
            let validate = match self.protected_decision {
                Some((decision_round, proposal_round, decision_subject, execution_commitment))
                    if proposal_round == round && decision_subject == subject =>
                {
                    stored.project_validate_after_durable_decision(
                        effect.clone(),
                        validate_ownership.clone(),
                        decision_round,
                        proposal_round,
                        decision_subject,
                        execution_commitment,
                    )
                }
                Some(_) => {
                    unreachable!("a retained Decision Validate has the protected genesis body key")
                }
                None => stored.project_validate(effect.clone(), validate_ownership.clone()),
            };
            let validate = match validate {
                Ok(validate) => validate,
                Err(error) => {
                    let previous = self.authenticated_genesis_replay.insert(
                        key,
                        AuthenticatedGenesisReplayStageV1::Stored {
                            replay: error.into_stored(),
                            ownership: store_ownership,
                        },
                    );
                    debug_assert!(previous.is_none());
                    return Err(EffectExecutorError::Contract(
                        "authenticated-genesis Store could not project its Validate successor"
                            .to_owned(),
                    ));
                }
            };
            self.install_pending_durable_validate_admission(
                key,
                &effect,
                &validate_ownership,
                validate.into_pending_durable_validate_admission(),
                Some(store_terminal),
            )?;
            return Ok(None);
        }
        match self.remote_proposal_replay.get(&key) {
            Some(RemoteProposalReplayStageV1::Fetch { .. })
            | Some(RemoteProposalReplayStageV1::BodyAvailable(_))
            | Some(RemoteProposalReplayStageV1::StoreAdmission(_))
            | Some(RemoteProposalReplayStageV1::Store { .. }) => {
                return Err(EffectExecutorError::Contract(
                    "Proposal ValidateBody preceded its exact durable Store replay stage"
                        .to_owned(),
                ));
            }
            Some(RemoteProposalReplayStageV1::Stored {
                replay,
                ownership: store_ownership,
            }) => {
                if !replay.exactly_retains_owned_store(&receipt, store_ownership) {
                    return Err(EffectExecutorError::Contract(
                        "Proposal ValidateBody changed its durable Store lineage".to_owned(),
                    ));
                }
            }
            None => {
                // A TC may promote an older PrepareQC after a newer durable
                // Prepare high-water mark caused ordinary Proposal replay to
                // retire. Rejoin only the currently protected full PrepareQC
                // to this exact recovered manifest/receipt and runtime
                // statement. This remains a normal LocalBody lifecycle
                // admission; it neither synthesizes certified-Fetch authority
                // nor bypasses the registry transaction.
                let authority_certificate =
                    self.exact_remote_proposal_validate_authority_certificate(&effect, &ownership)?;
                let Some(certificate) = authority_certificate
                    .filter(|certificate| certificate.phase == wire::GlobalPhase::Prepare)
                else {
                    return Err(EffectExecutorError::Contract(
                        "ValidateBody omitted its mandatory lifecycle replay owner".to_owned(),
                    ));
                };
                let (manifest, recovered_receipt) =
                    self.recovered_bodies.get(&key).cloned().ok_or_else(|| {
                        EffectExecutorError::Contract(
                            "protected-lock ValidateBody omitted its exact recovered body frame"
                                .to_owned(),
                        )
                    })?;
                if recovered_receipt != receipt {
                    return Err(EffectExecutorError::Contract(
                        "protected-lock ValidateBody changed its durable body receipt".to_owned(),
                    ));
                }
                let validate_ownership = ownership;
                let store_terminal = DurableStoreTerminalRetrySealV1::seal_validate_predecessor(
                    &effect,
                    &validate_ownership,
                    &receipt,
                )
                .ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "protected-lock ValidateBody could not seal its exact Store predecessor"
                            .to_owned(),
                    )
                })?;
                let prepared = PreparedLocalBodyValidateReplayPreAdmission::seal_exact_protected_lock_validate(
                    effect.clone(),
                    validate_ownership.clone(),
                    manifest,
                    receipt,
                    certificate,
                )
                .map_err(|_| {
                    EffectExecutorError::Contract(
                        "protected-lock ValidateBody could not reseal exact lifecycle replay"
                            .to_owned(),
                    )
                })?;
                self.install_pending_durable_validate_admission(
                    key,
                    &effect,
                    &validate_ownership,
                    prepared.into_pending_durable_validate_admission(),
                    Some(store_terminal),
                )?;
                return Ok(None);
            }
        }
        let authority_certificate =
            self.exact_remote_proposal_validate_authority_certificate(&effect, &ownership)?;
        let Some(RemoteProposalReplayStageV1::Stored {
            replay: stored,
            ownership: store_ownership,
        }) = self.remote_proposal_replay.remove(&key)
        else {
            unreachable!("preflighted Proposal Store replay remains installed")
        };
        let store_terminal = stored
            .seal_store_terminal_retry(&receipt, &store_ownership)
            .ok_or_else(|| {
                EffectExecutorError::Contract(
                    "Proposal Validate could not seal its durable Store terminal".to_owned(),
                )
            })?;
        let validate_ownership = ownership;
        let validate = match self.protected_decision {
            Some((decision_round, proposal_round, decision_subject, execution_commitment))
                if proposal_round == round && decision_subject == subject =>
            {
                let decision_certificate = authority_certificate.as_ref().filter(|certificate| {
                    certificate.phase == wire::GlobalPhase::Commit
                        && certificate.round == decision_round
                        && certificate.proposal_round == proposal_round
                        && certificate.subject == decision_subject
                        && certificate.execution_commitment == execution_commitment
                });
                let Some(decision_certificate) = decision_certificate else {
                    let previous = self.remote_proposal_replay.insert(
                        key,
                        RemoteProposalReplayStageV1::Stored {
                            replay: stored,
                            ownership: store_ownership,
                        },
                    );
                    debug_assert!(previous.is_none());
                    return Err(EffectExecutorError::Contract(
                        "Decision-refined Proposal Validate lost its exact CommitQC".to_owned(),
                    ));
                };
                stored.project_validate_after_durable_decision(
                    effect.clone(),
                    validate_ownership.clone(),
                    decision_certificate,
                )
            }
            Some(_) => unreachable!("a retained Decision Validate has the protected body key"),
            None => stored.project_validate(
                effect.clone(),
                validate_ownership.clone(),
                authority_certificate.as_ref(),
            ),
        };
        let validate = match validate {
            Ok(validate) => validate,
            Err(error) => {
                let previous = self.remote_proposal_replay.insert(
                    key,
                    RemoteProposalReplayStageV1::Stored {
                        replay: error.into_stored(),
                        ownership: store_ownership,
                    },
                );
                debug_assert!(previous.is_none());
                return Err(EffectExecutorError::Contract(
                    "Proposal Store replay could not project its exact Validate successor"
                        .to_owned(),
                ));
            }
        };
        self.install_pending_durable_validate_admission(
            key,
            &effect,
            &validate_ownership,
            validate.into_pending_durable_validate_admission(),
            Some(store_terminal),
        )?;
        Ok(None)
    }
}

impl V2EffectExecutor<SerializedV2Runtime> {
    /// Seal the exact pending certified Fetch for its first durable lifecycle admission.
    ///
    /// The authenticated response supplies a canonical manifest even when the
    /// certificate-backed Fetch began manifest-less. No executor owner is
    /// retired here; the returned replay-bound carrier is consumed only by the
    /// coordinator/registry/LedgerV1 admission transaction.
    pub(in crate::sumeragi) fn prepare_lifecycle_certified_fetch_admission(
        &self,
        candidate: &CertifiedResponsePriorityCandidate,
        response: &wire::CertifiedBodyResponse,
        active_context: LifecycleContext,
        verified: &VerifiedHeightContext,
    ) -> Result<
        super::v2_lifecycle_coordinator::PreparedCertifiedFetchAdmissionV1,
        EffectTransportError,
    > {
        self.validate_lifecycle_ingress_selector_authority()?;
        if !candidate.matches_authenticated_response(response, &candidate.authenticated_responder)
            || candidate.response_hash != HashOf::new(response)
            || candidate.canonical_manifest_hash != HashOf::new(&response.manifest)
        {
            return Err(EffectTransportError::BodyMismatch(
                "certified Fetch admission differs from fresh selector authority",
            ));
        }
        let pending = self
            .pending_fetches
            .get(&candidate.work_id)
            .ok_or(EffectTransportError::UnknownWork(candidate.work_id))?;
        if pending.task.id() != candidate.work_id
            || pending.request_hash != Some(candidate.request_hash)
            || pending.task.certified_request().map(HashOf::new) != Some(candidate.request_hash)
            || pending.task.round != candidate.round
            || pending.task.subject != candidate.subject
            || !pending
                .task
                .matches_reconstructed_manifest(&response.manifest)
        {
            return Err(EffectTransportError::BodyMismatch(
                "certified Fetch admission lost its exact pending request owner",
            ));
        }
        let effect = pending.task.adapter_effect();
        let binding = pending
            .task
            .ownership()
            .exact_pending_adapter_effect_binding(&effect)
            .map_err(|_| {
                EffectTransportError::FailClosed(
                    "pending certified Fetch lost its lifecycle binding".to_owned(),
                )
            })?;
        if &binding != candidate.pending_effect_binding() {
            return Err(EffectTransportError::BodyMismatch(
                "certified Fetch admission changed its pending effect binding",
            ));
        }
        super::v2_lifecycle_coordinator::PreparedCertifiedFetchAdmissionV1::prepare(
            active_context,
            verified,
            effect,
            binding,
            response.manifest.clone(),
            candidate.request_hash,
        )
        .map_err(|error| {
            EffectTransportError::FailClosed(format!(
                "pending certified Fetch lifecycle projection failed: {error:?}"
            ))
        })
    }

    /// Preview one ordinary certified Fetch-to-Store reducer transition.
    pub(in crate::sumeragi) fn prepare_certified_fetch_store_adapter(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<super::v2::CertifiedFetchStoreAdapterPreparationV1<'_>, super::v2::AdapterError>
    {
        self.runtime.prepare_certified_fetch_store(tag, manifest)
    }

    /// Preview one ordinary durable Store-to-Validate reducer transition.
    pub(in crate::sumeragi) fn prepare_durable_store_validate_adapter(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: &DurableBodyReceipt,
    ) -> Result<super::v2::DurableStoreValidateAdapterPreparationV1<'_>, super::v2::AdapterError>
    {
        self.runtime
            .prepare_durable_store_validate(tag, round, subject, receipt)
    }
}
