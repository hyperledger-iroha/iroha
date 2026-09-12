/// One observed durable inventory routed under one exact consensus authority.
/// The sorted certificate hashes are the pending-admission generation: coalesced
/// wakeups need no independent sequence number or mutable transport authority.
#[derive(Clone, Debug, PartialEq, Eq)]
struct QueuePlanAdmissionHandoffGeneration {
    round: wire::ConsensusRound,
    leader: PeerId,
    pending: BTreeSet<Hash>,
}

impl QueuePlanAdmissionHandoffGeneration {
    fn same_destination(&self, other: &Self) -> bool {
        self.round == other.round && self.leader == other.leader
    }
}

#[derive(Debug)]
struct QueuePlanAdmissionHandoffProgress {
    generation: QueuePlanAdmissionHandoffGeneration,
    enqueued: BTreeSet<Hash>,
}

/// Readiness is scoped to the exact view and durable certificate inventory.
/// `Enqueued` means ownership reached the retained adapter/output corridor; it
/// never means network delivery, canonical admission, or transaction finality.
#[derive(Debug)]
enum QueuePlanAdmissionHandoffState {
    Unobserved,
    Pending(QueuePlanAdmissionHandoffProgress),
    Enqueued(QueuePlanAdmissionHandoffProgress),
}

impl QueuePlanAdmissionHandoffState {
    fn progress(&self) -> Option<&QueuePlanAdmissionHandoffProgress> {
        match self {
            Self::Unobserved => None,
            Self::Pending(progress) | Self::Enqueued(progress) => Some(progress),
        }
    }

    fn needs_refresh(&self, round: wire::ConsensusRound, leader: &PeerId) -> bool {
        match self {
            Self::Enqueued(progress) => {
                progress.generation.round != round || &progress.generation.leader != leader
            }
            Self::Unobserved | Self::Pending(_) => true,
        }
    }

    fn begin(&mut self, generation: QueuePlanAdmissionHandoffGeneration) {
        let enqueued = self
            .progress()
            .filter(|progress| progress.generation.same_destination(&generation))
            .map(|progress| {
                progress
                    .enqueued
                    .intersection(&generation.pending)
                    .copied()
                    .collect()
            })
            .unwrap_or_default();
        *self = Self::Pending(QueuePlanAdmissionHandoffProgress {
            generation,
            enqueued,
        });
    }

    fn is_enqueued(&self) -> bool {
        matches!(self, Self::Enqueued(_))
    }

    fn contains(&self, hash: &Hash) -> bool {
        self.progress()
            .is_some_and(|progress| progress.enqueued.contains(hash))
    }

    fn admit(&mut self, generation: &QueuePlanAdmissionHandoffGeneration, hash: Hash) -> bool {
        match self {
            Self::Pending(progress)
                if progress.generation == *generation && generation.pending.contains(&hash) =>
            {
                progress.enqueued.insert(hash);
                true
            }
            _ => false,
        }
    }

    fn finish(&mut self, generation: &QueuePlanAdmissionHandoffGeneration) -> bool {
        if !matches!(self, Self::Pending(progress) if progress.generation == *generation) {
            return false;
        }
        if let Self::Pending(progress) = std::mem::replace(self, Self::Unobserved) {
            *self = Self::Enqueued(progress);
            true
        } else {
            false
        }
    }
}

impl V2LaneWorkAdapter {
    /// A new destination or a capacity-pending inventory needs another bounded turn.
    pub(crate) fn queue_plan_admission_handoffs_need_refresh(
        &self,
        active_view: wire::View,
    ) -> Result<bool, V2LaneWorkError> {
        let leader = self
            .context
            .roster
            .get(usize::try_from(self.context.leader(active_view)).unwrap_or(usize::MAX))
            .map(|entry| &entry.validator)
            .ok_or_else(|| {
                V2LaneWorkError::InvalidContext(
                    "QueuePlan handoff leader is outside the frozen roster".to_owned(),
                )
            })?;
        Ok(self.queue_plan_admission_handoff.needs_refresh(
            wire::ConsensusRound {
                context_id: self.context.id(),
                height: self.context.height,
                view: active_view,
            },
            leader,
        ))
    }

    pub(crate) fn reconcile_pending_queue_plan_admissions(
        &mut self,
        active_view: wire::View,
    ) -> Result<Vec<Vec<u8>>, V2LaneWorkError> {
        let leader_index = self.context.leader(active_view);
        let leader_peer = self
            .context
            .roster
            .get(usize::try_from(leader_index).unwrap_or(usize::MAX))
            .map(|entry| entry.validator.clone())
            .ok_or_else(|| {
                V2LaneWorkError::InvalidContext(
                    "current global leader is outside the frozen roster".to_owned(),
                )
            })?;
        let local_is_leader = self.local_validator_index() == Some(leader_index);
        let mut admissions = Vec::new();
        let mut pending = self
            .kura
            .pending_queue_plan_admission_certificates_bounded(
                self.kura.pending_queue_plan_admission_capacity(),
            )
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        let generation = QueuePlanAdmissionHandoffGeneration {
            round: wire::ConsensusRound {
                context_id: self.context.id(),
                height: self.context.height,
                view: active_view,
            },
            leader: leader_peer.clone(),
            pending: pending.iter().map(|(hash, _)| *hash).collect(),
        };
        // A certified view transition supersedes only the old adapter-owned
        // handoff. Kura remains the exact source; already transferred worker
        // occurrences cannot complete this new generation.
        self.effects.retain(|effect| match effect {
            V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
                peer,
                view,
                certificate,
            } => {
                *view == active_view
                    && *peer == leader_peer
                    && generation
                        .pending
                        .contains(&Hash::new(certificate.as_slice()))
            }
            _ => true,
        });
        self.effect_keys = self.effects.iter().map(lane_work_effect_key).collect();
        self.queue_plan_admission_handoff.begin(generation.clone());
        let count = pending.len();
        let start = if local_is_leader || count == 0 {
            0
        } else {
            self.queue_plan_admission_handoff_cursor % count
        };
        pending.rotate_left(start);
        let mut completed = true;
        for (offset, (certificate_hash, certificate_bytes)) in pending.into_iter().enumerate() {
            let (admission, disposition) = self
                .state
                .classify_pending_queue_plan_admission(&certificate_bytes, self.context.height)
                .map_err(|error| {
                    V2LaneWorkError::Persistence(format!(
                        "pending QueuePlan admission certificate cannot be reconciled: {error}"
                    ))
                })?;
            match disposition {
                PendingQueuePlanAdmissionDisposition::ExactPending => {
                    // Retain the quorum certificate while the marker is canonical but the body
                    // is not. It remains the authenticated handoff for peers lagging behind the
                    // marker and is retired only with canonical transaction membership.
                }
                PendingQueuePlanAdmissionDisposition::Applied => {
                    self.state
                        .remove_pending_queue_plan_admission_certificate(certificate_hash)
                        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
                }
                PendingQueuePlanAdmissionDisposition::DefinitiveConflict
                | PendingQueuePlanAdmissionDisposition::Stale => {
                    let queue = self.lane_drain_queue.as_ref().ok_or_else(|| {
                        V2LaneWorkError::Persistence(
                            "losing QueuePlan admission cannot be retired without the live queue"
                                .to_owned(),
                        )
                    })?;
                    queue
                        .reject_exact_queue_plan_admission_claim(&admission.certificate.binding)
                        .map_err(|error| {
                            V2LaneWorkError::Persistence(format!(
                                "losing QueuePlan admission queue claim cannot be durably rejected: {error}"
                            ))
                        })?;
                    self.state
                        .remove_pending_queue_plan_admission_certificate(certificate_hash)
                        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
                }
                PendingQueuePlanAdmissionDisposition::EligibleAbsent if local_is_leader => {
                    admissions.push((
                        admission.registry_key,
                        admission.registry_value,
                        certificate_bytes,
                    ));
                }
                PendingQueuePlanAdmissionDisposition::EligibleAbsent => {
                    if self
                        .queue_plan_admission_handoff
                        .contains(&certificate_hash)
                    {
                        continue;
                    }
                    let effect = V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
                        peer: leader_peer.clone(),
                        view: active_view,
                        certificate: Arc::new(certificate_bytes),
                    };
                    let queued = self.effect_keys.contains(&lane_work_effect_key(&effect));
                    if !queued && !self.push_effect(effect) {
                        self.queue_plan_admission_handoff_cursor = (start + offset) % count;
                        completed = false;
                        break;
                    }
                    if !self
                        .queue_plan_admission_handoff
                        .admit(&generation, certificate_hash)
                    {
                        return Err(V2LaneWorkError::InvalidContext(
                            "QueuePlan handoff admission lost its exact generation".to_owned(),
                        ));
                    }
                }
                PendingQueuePlanAdmissionDisposition::Future { .. }
                | PendingQueuePlanAdmissionDisposition::DeferredCarrier => {}
            }
        }
        if !local_is_leader && count != 0 && completed {
            self.queue_plan_admission_handoff_cursor = (start + 1) % count;
        }
        if completed && !self.queue_plan_admission_handoff.finish(&generation) {
            return Err(V2LaneWorkError::InvalidContext(
                "QueuePlan handoff completion lost its exact generation".to_owned(),
            ));
        }
        admissions.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
        });
        let mut selected = Vec::new();
        let mut selected_bytes = 0usize;
        let mut previous_registry_key = None;
        for (registry_key, _, certificate) in admissions {
            if previous_registry_key.as_ref() == Some(&registry_key) {
                continue;
            }
            let Some(next_bytes) = selected_bytes.checked_add(certificate.len()) else {
                break;
            };
            if selected.len() == iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSIONS_PER_BLOCK
                || next_bytes > iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSIONS_BYTES
            {
                break;
            }
            selected_bytes = next_bytes;
            previous_registry_key = Some(registry_key.clone());
            selected.push((registry_key, certificate));
        }
        Ok(selected
            .into_iter()
            .map(|(_, certificate)| certificate)
            .collect())
    }

    fn accept_queue_plan_admission_certificate(
        &mut self,
        _sender: PeerId,
        certificate: Arc<Vec<u8>>,
        active_view: wire::View,
    ) -> Result<V2LaneIngressOutcome, V2LaneWorkError> {
        let local_is_leader = self
            .context
            .roster
            .get(usize::try_from(self.context.leader(active_view)).unwrap_or(usize::MAX))
            .is_some_and(|entry| entry.validator == self.local_peer);
        if !local_is_leader {
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        let Ok(outcome) = self
            .state
            .persist_classified_queue_plan_admission(certificate.as_slice())
        else {
            return Ok(V2LaneIngressOutcome::Rejected);
        };
        match outcome {
            PendingQueuePlanAdmissionPersistenceOutcome::Applied { .. } => {
                Ok(V2LaneIngressOutcome::Duplicate)
            }
            PendingQueuePlanAdmissionPersistenceOutcome::Rejected { .. } => {
                Ok(V2LaneIngressOutcome::Rejected)
            }
            PendingQueuePlanAdmissionPersistenceOutcome::Durable { inserted, .. } => {
                Ok(if inserted {
                    V2LaneIngressOutcome::Inserted
                } else {
                    V2LaneIngressOutcome::Duplicate
                })
            }
        }
    }
}
