// Closed Proposal/Prepare/Commit recovery after a next-WAL Sign has advanced.
// This is included in wal_recovery so no raw effect or body authority escapes
// the existing private recovery permits.

/// Exact cold continuation of a standalone Sign whose next Vote has advanced.
///
/// The complete ledger frame is retained until the registry and coordinator
/// agree. Historical Sign rows remain untouched; only their live Broadcasts
/// and an optional final Sign become executable again.
pub(super) struct RecoveredControlContinuationV1 {
    ledger: super::ledger::LifecycleLedgerV1,
    control: AuthenticatedRecoveredWalStandaloneSignProjection,
    parent_ordinal: u128,
    broadcast_ordinal: u128,
    broadcast: RecoveredLifecycleSignedBroadcastProjectionV1,
    votes: Vec<RecoveredControlVoteContinuationV1>,
}

/// One body/WAL-authenticated next Vote and its optional durable signed child.
pub(super) struct RecoveredControlVoteContinuationV1 {
    pub(super) ordinal: u128,
    pub(super) vote: super::replay_authority::RecoveredLifecycleNextWalVoteCandidateProjectionV1,
    pub(super) broadcast: Option<(u128, RecoveredLifecycleSignedBroadcastProjectionV1)>,
}

impl AuthenticatedRecoveredWalStandaloneSignProjection {
    /// Bound the exact remaining vote chain by its closed original WAL source.
    /// Linked Validate repairs never enter this standalone corridor.
    fn advanced_vote_continuation_limit(&self) -> Option<usize> {
        match (&self.origin, &self.effect) {
            (
                RecoveredStandaloneSignOriginV1::Control(_),
                AdapterEffect::Sign {
                    request: crate::sumeragi::v2::SignRequest::Proposal(_),
                    ..
                },
            ) => Some(2),
            (
                RecoveredStandaloneSignOriginV1::ResolvedPhaseVote { .. },
                AdapterEffect::Sign {
                    request: crate::sumeragi::v2::SignRequest::Vote(vote),
                    ..
                },
            ) if vote.phase == wire::GlobalPhase::Prepare => Some(1),
            _ => None,
        }
    }

    /// Select the advanced tail without treating a row hint as authority.
    /// Proposal always publishes its next Prepare with the Broadcast. Prepare
    /// can publish only a Broadcast while it still awaits the PrepareQC.
    pub(super) fn has_advanced_vote_continuation(
        &self,
        ledger: &super::ledger::LifecycleLedgerV1,
        broadcast_ordinal: u128,
    ) -> bool {
        match self.advanced_vote_continuation_limit() {
            Some(2) => true,
            Some(1) => broadcast_ordinal.checked_add(1).is_some_and(|ordinal| {
                ledger.records().iter().any(|record| {
                    record.ordinal() == ordinal
                        && record.work_class() == Some(LifecycleWorkClass::SignVote)
                        && record
                            .key()
                            .is_some_and(|key| key.phase() == super::LifecyclePhase::Commit)
                        && record
                            .stage()
                            .is_some_and(|stage| stage.kind() == LifecycleStageKind::SignCommitVote)
                })
            }),
            _ => false,
        }
    }

    /// Replay a standalone Proposal or Prepare and its bounded Vote continuation.
    ///
    /// When a next vote is produced, its Sign and the preceding Broadcast
    /// share one exact LedgerV1 successor after the vote's WAL fsync. The caller
    /// keeps a standalone Prepare Broadcast without a next vote on the existing
    /// Broadcast-only path. Proposal has at most two remaining vote phases;
    /// standalone Prepare has only Commit.
    /// Each signature is roster-authenticated, each next Vote rejoins the
    /// semantically revalidated body store and exact WAL frame, and every live
    /// output is compared with its original ledger row before returning.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(super) fn recover_advanced_standalone_vote_continuation(
        self,
        verified: &VerifiedHeightContext,
        ledger: &super::ledger::LifecycleLedgerV1,
        parent_ordinal: u128,
        broadcast_ordinal: u128,
        broadcast: RecoveredLifecycleSignedBroadcastProjectionV1,
        startup: crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1,
        body_store: &crate::sumeragi::v2_body_store::V2BodyStore,
    ) -> Result<
        (
            crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1,
            RecoveredControlContinuationV1,
        ),
        &'static str,
    > {
        let limit = self
            .advanced_vote_continuation_limit()
            .ok_or("cold continuation has no standalone Proposal or Prepare source")?;
        if !self.is_exact(verified) || !self.source_matches_ledger(ledger) {
            return Err("cold standalone continuation changed its exact WAL or terminal source");
        }
        let mut preview =
            self.prepare_cold_signed_broadcast_and_sign(verified, startup, &broadcast)?;
        let body = body_store
            .authenticate_recovered_lifecycle_next_vote_body(&mut preview)
            .map_err(|_| "cold Proposal continuation lost its exact body-store authority")?;
        let seal = preview.seal_recovered_lifecycle_next_wal_vote(body)?;
        let (startup, mut combined) = self
            .project_authenticated_cold_signed_broadcast_and_sign(verified, seal)
            .ok_or("cold Proposal continuation changed its WAL/body authority")?;
        if !combined
            .broadcast
            .exactly_matches_durable_projection(&broadcast)
        {
            return Err("cold Proposal continuation changed its signed Broadcast");
        }
        let authority = combined
            .project_cold_adapter_replay_authority(verified)
            .ok_or("cold Proposal continuation cannot replay its historical signature")?;
        let mut startup =
            startup.advance_recovered_lifecycle_signed_broadcast_and_sign(verified, authority)?;
        let mut next_vote = combined.next_sign;
        let mut ordinal = broadcast_ordinal
            .checked_add(1)
            .ok_or("cold Proposal continuation ordinal overflow")?;
        let mut votes = Vec::new();
        let record_at = |ordinal| {
            ledger
                .records()
                .binary_search_by_key(&ordinal, |row| row.ordinal())
                .ok()
                .and_then(|index| ledger.records().get(index))
        };
        loop {
            if votes.len() >= limit {
                return Err("cold standalone continuation exceeds its remaining vote phases");
            }
            let record =
                record_at(ordinal).ok_or("cold Proposal continuation lost its next Vote row")?;
            if next_vote.exactly_matches_fresh_record(ledger.context(), record) {
                votes.push(RecoveredControlVoteContinuationV1 {
                    ordinal,
                    vote: next_vote,
                    broadcast: None,
                });
                break;
            }
            let (_, child_ordinal) = record
                .continuation()
                .and_then(super::schema::DurableContinuation::successor_parts)
                .ok_or("cold Proposal next Vote is neither live nor an exact advanced Sign")?;
            if !next_vote.exactly_matches_advanced_broadcast_parent(
                ledger.context(),
                record,
                child_ordinal,
            ) {
                return Err("cold Proposal next Vote changed its exact advanced parent");
            }
            let child = record_at(child_ordinal)
                .ok_or("cold Proposal next Vote lost its signed Broadcast")?;
            let durable = child
                .project_recovered_signed_broadcast_child(ledger.context())
                .ok_or("cold Proposal next Vote has no canonical live Broadcast")?;
            let effect =
                durable.consume_for_recovered_wal(
                    RecoveredLifecycleSignBroadcastProjectionPermitV1::new(),
                );
            let projected = next_vote
                .project_authenticated_signed_broadcast(verified, effect)
                .ok_or("cold Proposal next Vote Broadcast failed WAL and roster authentication")?;
            let (effect, pending, candidate) =
                projected.consume_for_recovered_wal(
                    RecoveredLifecycleSignBroadcastProjectionPermitV1::new(),
                );
            let signed = RecoveredLifecycleSignedBroadcastProjectionV1 {
                effect,
                pending,
                candidate,
                cold_proposal_output: None,
            };
            if child.owner() != record.owner()
                || !signed.exactly_matches_record(child, record.owner())
            {
                return Err("cold Proposal next Vote Broadcast changed its durable owner");
            }
            let next_ordinal = child_ordinal
                .checked_add(1)
                .ok_or("cold Proposal continuation ordinal overflow")?;
            let has_next_sign = record_at(next_ordinal).is_some_and(|row| {
                row.work_class() == Some(LifecycleWorkClass::SignVote)
                    && row
                        .stage()
                        .is_some_and(|stage| stage.kind() == LifecycleStageKind::SignCommitVote)
            });
            let sign = next_vote
                .project_cold_adapter_next_sign(
                    verified,
                    RecoveredLifecycleSignBroadcastProjectionPermitV1::new(),
                )
                .ok_or("cold Proposal next Vote lost its exact WAL request")?;
            let AdapterEffect::Sign { tag, request } = sign else {
                return Err("cold Proposal next Vote is not a Sign");
            };
            if has_next_sign {
                let authority = crate::sumeragi::v2::RecoveredLifecycleSignedBroadcastColdPreviewAuthorityV1::from_recovered_wal(
                    RecoveredLifecycleSignBroadcastProjectionPermitV1::new(), tag, request, signed.effect.clone())
                    .ok_or("cold Proposal Prepare Broadcast is not an exact signed successor")?;
                let mut preview = startup
                    .prepare_recovered_lifecycle_signed_broadcast_and_sign(verified, authority)?;
                let body = body_store
                    .authenticate_recovered_lifecycle_next_vote_body(&mut preview)
                    .map_err(
                        |_| "cold Proposal Commit continuation lost its exact body-store authority",
                    )?;
                let seal = preview.seal_recovered_lifecycle_next_wal_vote(body)?;
                let (cold, replayed, next, output) = seal.consume_for_recovered_wal(
                    RecoveredLifecycleSignBroadcastProjectionPermitV1::new(),
                );
                if replayed != signed.effect || output.is_some() {
                    return Err("cold Proposal Prepare continuation changed its signed output");
                }
                let next = project_recovered_lifecycle_next_wal_vote_candidate(verified, next)
                    .map_err(
                        |_| "cold Proposal Commit continuation changed its exact WAL candidate",
                    )?;
                let next_effect = next
                    .project_cold_adapter_next_sign(
                        verified,
                        RecoveredLifecycleSignBroadcastProjectionPermitV1::new(),
                    )
                    .ok_or("cold Proposal Commit continuation lost its replay request")?;
                let authority = crate::sumeragi::v2::RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1::from_recovered_wal(
                    RecoveredLifecycleSignBroadcastProjectionPermitV1::new(), signed.effect.clone(), next_effect)
                    .ok_or("cold Proposal Prepare/Commit adapter relation is inconsistent")?;
                startup = cold
                    .advance_recovered_lifecycle_signed_broadcast_and_sign(verified, authority)?;
                votes.push(RecoveredControlVoteContinuationV1 {
                    ordinal,
                    vote: next_vote,
                    broadcast: Some((child_ordinal, signed)),
                });
                next_vote = next;
                ordinal = next_ordinal;
            } else {
                let authority = crate::sumeragi::v2::RecoveredLifecycleSignColdAdapterAuthorityV1::from_recovered_wal(
                    RecoveredLifecycleSignBroadcastProjectionPermitV1::new(), tag, request, signed.effect.clone())
                    .ok_or("cold Proposal Vote Broadcast lost its exact signature")?;
                startup =
                    startup.advance_recovered_lifecycle_signed_broadcast(verified, authority)?;
                votes.push(RecoveredControlVoteContinuationV1 {
                    ordinal,
                    vote: next_vote,
                    broadcast: Some((child_ordinal, signed)),
                });
                break;
            }
        }
        let continuation = RecoveredControlContinuationV1 {
            ledger: ledger.clone(),
            control: self,
            parent_ordinal,
            broadcast_ordinal,
            broadcast: combined.broadcast,
            votes,
        };
        if !continuation.exactly_matches_ledger(ledger) {
            return Err("cold Proposal continuation does not own its complete durable lineage");
        }
        Ok((startup, continuation))
    }
}

impl RecoveredControlContinuationV1 {
    /// Recheck the retained frame and every historical parent/live child.
    pub(super) fn exactly_matches_ledger(&self, ledger: &super::ledger::LifecycleLedgerV1) -> bool {
        let Some(limit) = self.control.advanced_vote_continuation_limit() else {
            return false;
        };
        if &self.ledger != ledger
            || !self.control.source_matches_ledger(ledger)
            || self.votes.is_empty()
            || self.votes.len() > limit
            || self.votes[0].broadcast.is_none()
        {
            return false;
        }
        let row = |ordinal| {
            ledger
                .records()
                .binary_search_by_key(&ordinal, |record| record.ordinal())
                .ok()
                .and_then(|index| ledger.records().get(index))
        };
        let (Some(parent), Some(broadcast)) =
            (row(self.parent_ordinal), row(self.broadcast_ordinal))
        else {
            return false;
        };
        if !self
            .control
            .exactly_matches_advanced_record(parent, self.broadcast_ordinal)
            || !self
                .broadcast
                .exactly_matches_record(broadcast, parent.owner())
            || ledger
                .records()
                .iter()
                .filter(|record| record.owner() == parent.owner())
                .count()
                != 2
        {
            return false;
        }
        let mut expected = self.broadcast_ordinal.checked_add(1);
        let mut owners = std::collections::BTreeSet::from([parent.owner()]);
        for (index, vote) in self.votes.iter().enumerate() {
            let Some(sign) = row(vote.ordinal) else {
                return false;
            };
            let expected_phase = if limit == 2 && index == 0 {
                super::LifecyclePhase::Prepare
            } else {
                super::LifecyclePhase::Commit
            };
            if expected != Some(vote.ordinal)
                || !owners.insert(sign.owner())
                || sign.key().is_none_or(|key| key.phase() != expected_phase)
            {
                return false;
            }
            match &vote.broadcast {
                None => {
                    if index + 1 != self.votes.len()
                        || !vote
                            .vote
                            .exactly_matches_fresh_record(ledger.context(), sign)
                        || ledger
                            .records()
                            .iter()
                            .filter(|record| record.owner() == sign.owner())
                            .count()
                            != 1
                    {
                        return false;
                    }
                }
                Some((ordinal, projected)) => {
                    let Some(child) = row(*ordinal) else {
                        return false;
                    };
                    if !vote.vote.exactly_matches_advanced_broadcast_parent(
                        ledger.context(),
                        sign,
                        *ordinal,
                    ) || !projected.exactly_matches_record(child, sign.owner())
                        || ledger
                            .records()
                            .iter()
                            .filter(|record| record.owner() == sign.owner())
                            .count()
                            != 2
                    {
                        return false;
                    }
                    expected = ordinal.checked_add(1);
                }
            }
        }
        true
    }

    /// Identify only the live rows owned by this sealed continuation.
    pub(super) fn owns_live_ordinal(&self, ordinal: u128) -> bool {
        ordinal == self.broadcast_ordinal
            || self.votes.iter().any(|vote| {
                vote.broadcast
                    .as_ref()
                    .map_or(vote.ordinal == ordinal, |(child, _)| *child == ordinal)
            })
    }

    /// Splice every live child after complete durable lineage authentication.
    pub(super) fn splice_candidates(
        &self,
        ledger: &super::ledger::LifecycleLedgerV1,
        candidates: &mut std::collections::BTreeMap<super::LifecycleKey, CandidateAdmission>,
    ) -> bool {
        if !self.exactly_matches_ledger(ledger) {
            return false;
        }
        let mut next = candidates.clone();
        for record in ledger
            .records()
            .iter()
            .filter(|row| self.owns_live_ordinal(row.ordinal()))
        {
            let okay = if record.ordinal() == self.broadcast_ordinal {
                self.broadcast
                    .splice_candidate_from_record(record, record.owner(), &mut next)
            } else {
                self.votes.iter().any(|vote| match &vote.broadcast {
                    Some((ordinal, broadcast)) if *ordinal == record.ordinal() => {
                        broadcast.splice_candidate_from_record(record, record.owner(), &mut next)
                    }
                    None if vote.ordinal == record.ordinal() => vote
                        .vote
                        .splice_candidate_from_fresh_record(ledger.context(), record, &mut next),
                    _ => false,
                })
            };
            if !okay {
                return false;
            }
        }
        *candidates = next;
        true
    }

    /// Recheck that no sealed live child was omitted from the storage census.
    pub(super) fn owns_candidates(
        &self,
        candidates: &std::collections::BTreeMap<super::LifecycleKey, CandidateAdmission>,
    ) -> bool {
        self.broadcast.owns_spliced_candidate(candidates)
            && self.votes.iter().all(|vote| match &vote.broadcast {
                Some((_, broadcast)) => broadcast.owns_spliced_candidate(candidates),
                None => vote.vote.owns_spliced_candidate(candidates),
            })
    }

    /// Retain exact opaque carriers through the exclusive registry installation.
    pub(super) fn into_registry_parts(
        self,
        _permit: super::work_registry::RecoveredLifecycleBroadcastAndSignRegistryCommitPermitV1,
    ) -> (
        AuthenticatedRecoveredWalStandaloneSignProjection,
        u128,
        RecoveredLifecycleSignedBroadcastProjectionV1,
        u128,
        Vec<RecoveredControlVoteContinuationV1>,
    ) {
        (
            self.control,
            self.parent_ordinal,
            self.broadcast,
            self.broadcast_ordinal,
            self.votes,
        )
    }
}

#[cfg(test)]
impl AuthenticatedRecoveredWalStandaloneSignProjection {
    /// Persist a crash frame built from real authenticated WAL frames and signed messages.
    pub(in crate::sumeragi) fn persist_advanced_continuation_for_test(
        &self,
        verified: &VerifiedHeightContext,
        root: &std::path::Path,
        proposal: AdapterEffect,
        votes: Vec<(
            super::replay_authority::RecoveredLifecycleNextWalVoteSealV1,
            Option<AdapterEffect>,
        )>,
    ) {
        use super::{
            OwnerId, TerminalOutcome,
            ledger::{LifecycleLedgerRecordV1, LifecycleLedgerStoreV1, LifecycleLedgerV1},
            schema::DurableContinuation,
        };
        let make = |candidate: &CandidateAdmission,
                    owner,
                    ordinal,
                    successor: Option<(DurableContinuationEdge, u128)>| {
            LifecycleLedgerRecordV1::new(
                candidate.key,
                owner,
                ordinal,
                candidate.work_class,
                candidate.stage,
                successor.map(|_| TerminalOutcome::Advanced),
                candidate.reconstruction_source,
                candidate.payload,
                candidate.replay_authority.clone(),
                successor.map_or(DurableContinuation::None, |(edge, child)| {
                    DurableContinuation::successor(edge, child)
                }),
            )
            .expect("construct exact crash row")
        };
        let proposal =
            project_recovered_signed_broadcast(verified, &self.effect, &self.pending, &proposal)
                .expect("authenticate fixture Proposal signature");
        let owner = OwnerId::new(self.candidate.causal_root, 1);
        let mut records = vec![
            make(
                &self.candidate,
                owner,
                1,
                Some((DurableContinuationEdge::SignProposalToBroadcast, 2)),
            ),
            make(&proposal.candidate, owner, 2, None),
        ];
        let mut ordinal = 3;
        for (vote, broadcast) in votes {
            let vote = project_recovered_lifecycle_next_wal_vote_candidate(verified, vote)
                .unwrap_or_else(|_| panic!("authenticate fixture next-WAL vote"));
            let candidate = vote.candidate_for_continuation_test();
            let owner = OwnerId::new(candidate.causal_root, ordinal);
            let edge = match candidate.stage.kind() {
                LifecycleStageKind::SignPrepareVote => {
                    DurableContinuationEdge::SignPrepareToBroadcast
                }
                LifecycleStageKind::SignCommitVote => {
                    DurableContinuationEdge::SignCommitToBroadcast
                }
                _ => panic!("fixture vote has no broadcast continuation"),
            };
            records.push(make(
                candidate,
                owner,
                ordinal,
                broadcast.as_ref().map(|_| (edge, ordinal + 1)),
            ));
            ordinal += 1;
            if let Some(broadcast) = broadcast {
                let projected = vote
                    .project_authenticated_signed_broadcast(verified, broadcast)
                    .expect("authenticate fixture Vote signature");
                let (_, _, candidate) = projected.consume_for_recovered_wal(
                    RecoveredLifecycleSignBroadcastProjectionPermitV1::new(),
                );
                records.push(make(&candidate, owner, ordinal, None));
                ordinal += 1;
            }
        }
        let context = projection::lifecycle_context(verified.context());
        let ledger = LifecycleLedgerV1::new(context, ordinal - 1, records, Default::default())
            .expect("validate exact crash frame");
        let (store, _) = LifecycleLedgerStoreV1::open(root, context).expect("open fixture ledger");
        store.persist(&ledger).expect("persist exact crash frame");
    }
}
