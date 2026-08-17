impl V2LaneWorkAdapter {
    #[cfg(test)]
    pub(in crate::sumeragi) fn set_queue_plan_test_network_id(
        &mut self,
        network_id: iroha_data_model::NetworkId,
    ) {
        self.context.network_id = network_id;
    }

    fn reconcile_pending_queue_plan_admissions(
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
                PendingQueuePlanAdmissionDisposition::Exact => self
                    .kura
                    .remove_pending_queue_plan_admission_certificate(certificate_hash)
                    .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?,
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
                    self.kura
                        .remove_pending_queue_plan_admission_certificate(certificate_hash)
                        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
                }
                PendingQueuePlanAdmissionDisposition::EligibleAbsent if local_is_leader => {
                    admissions.push(certificate_bytes);
                }
                PendingQueuePlanAdmissionDisposition::EligibleAbsent => {
                    let effect = V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
                        peer: leader_peer.clone(),
                        view: active_view,
                        certificate: Arc::new(certificate_bytes),
                    };
                    let queued = self.effect_keys.contains(&lane_work_effect_key(&effect));
                    if !queued && !self.push_effect(effect) {
                        self.queue_plan_admission_handoff_retry_required = true;
                        self.queue_plan_admission_handoff_cursor = (start + offset) % count;
                        completed = false;
                        break;
                    }
                }
                PendingQueuePlanAdmissionDisposition::Future => {}
            }
        }
        if !local_is_leader && count != 0 && completed {
            self.queue_plan_admission_handoff_cursor = (start + 1) % count;
        }
        Ok(admissions)
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
        let Ok((_, disposition)) = self
            .state
            .classify_pending_queue_plan_admission(certificate.as_slice(), self.context.height)
        else {
            return Ok(V2LaneIngressOutcome::Rejected);
        };
        match disposition {
            PendingQueuePlanAdmissionDisposition::Exact => {
                return Ok(V2LaneIngressOutcome::Duplicate);
            }
            PendingQueuePlanAdmissionDisposition::EligibleAbsent => {}
            PendingQueuePlanAdmissionDisposition::DefinitiveConflict
            | PendingQueuePlanAdmissionDisposition::Stale
            | PendingQueuePlanAdmissionDisposition::Future => {
                return Ok(V2LaneIngressOutcome::Rejected);
            }
        }
        let certificate_hash = Hash::new(certificate.as_slice());
        if self
            .kura
            .pending_queue_plan_admission_certificate(certificate_hash)
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?
            .is_some()
        {
            return Ok(V2LaneIngressOutcome::Duplicate);
        }
        self.kura
            .persist_pending_queue_plan_admission_certificate(certificate.as_slice())
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        self.refresh_merge_candidates(active_view)?;
        Ok(V2LaneIngressOutcome::Inserted)
    }
}
