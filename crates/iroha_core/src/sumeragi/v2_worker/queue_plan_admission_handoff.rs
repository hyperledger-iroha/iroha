type QueuePlanAdmissionEffectParts = (
    Vec<NetworkMessage>,
    Vec<PeerId>,
    Vec<ExactTargetRoute>,
    Option<NetworkReplyRoutes>,
    Option<FairV2IngressOwnershipEvidence>,
    ExactOutputRolloverClaim,
);
pub(crate) struct QueuePlanBatchSources {
    inventory: HashSet<Hash>,
    bodies: HashMap<Hash, Vec<u8>>,
    validated: HashSet<Hash>,
}

impl QueuePlanBatchSources {
    fn resolve<'a>(&'a mut self, kura: &Kura, hash: Hash) -> Result<Option<&'a [u8]>, String> {
        if !self.inventory.contains(&hash) {
            return Ok(None);
        }
        if !self.bodies.contains_key(&hash) {
            let Some(bytes) = kura
                .pending_queue_plan_admission_certificate(hash)
                .map_err(|error| error.to_string())?
            else {
                return Ok(None);
            };
            self.bodies.insert(hash, bytes);
        }
        Ok(self.bodies.get(&hash).map(Vec::as_slice))
    }

    fn contains_exact(&mut self, kura: &Kura, bytes: &[u8]) -> Result<bool, String> {
        self.resolve(kura, Hash::new(bytes))
            .map(|source| source == Some(bytes))
    }

    #[cfg(test)]
    pub(in crate::sumeragi) fn corrupt_for_test(&mut self, hash: Hash) {
        self.bodies.insert(hash, b"tampered".to_vec());
    }

    fn validate(
        &mut self,
        _kura: &Kura,
        network_id: &NetworkId,
        bytes: &[u8],
    ) -> Result<Hash, String> {
        let hash = Hash::new(bytes);
        if !self.validated.contains(&hash) {
            crate::torii_proxy::decode_and_validate_queue_plan_admission_certificate_v1(
                network_id, bytes,
            )
            .map_err(|error| format!("invalid QueuePlan admission handoff: {error}"))?;
            #[cfg(test)]
            _kura
                .pending_queue_plan_admission_batch_validations
                .fetch_add(1, AtomicOrdering::Relaxed);
            self.validated.insert(hash);
        }
        Ok(hash)
    }
}

fn queue_plan_admission_reconstruction_covers(
    messages: &[NetworkMessage],
    rollover_claim: &ExactOutputRolloverClaim,
    artifact: &wire::finality::V2FinalityArtifact,
    durable_history: Option<&Kura>,
) -> Result<(), String> {
    let ExactOutputRolloverClaim::QueuePlanAdmission {
        target,
        view,
        certificate_hash,
        ..
    } = rollover_claim
    else {
        return Err("QueuePlan reconstruction received another claim kind".to_owned());
    };
    let [NetworkMessage::QueuePlanAdmissionCertificate(certificate)] = messages else {
        return Err("QueuePlan admission rollover lost its exact certificate payload".to_owned());
    };
    let expected_leader = artifact
        .height_context
        .roster
        .get(usize::try_from(artifact.height_context.leader(*view)).unwrap_or(usize::MAX))
        .map(|entry| &entry.validator);
    if expected_leader != Some(target) {
        return Err("QueuePlan admission rollover target is not its frozen view leader".to_owned());
    }
    let durable_history = durable_history.ok_or_else(|| {
        "QueuePlan admission rollover lacks an independently readable Kura source".to_owned()
    })?;
    let source_is_exact = durable_history
        .pending_queue_plan_admission_certificate(*certificate_hash)
        .map_err(|error| error.to_string())?
        .is_some_and(|bytes| bytes == certificate.as_slice());
    source_is_exact
        .then_some(())
        .ok_or_else(|| "QueuePlan admission rollover lost its exact durable Kura source".to_owned())
}

fn rotating_current_archive_targets(
    local_peer: &PeerId,
    cursor: &AtomicUsize,
    limit: usize,
    mut snapshot: impl FnMut(usize, usize) -> ConfiguredPeerBatch,
) -> Vec<PeerId> {
    let mut remaining_in_cycle = None;
    loop {
        let mut start = cursor.load(AtomicOrdering::Relaxed);
        let batch = loop {
            let candidate = snapshot(start, limit);
            match cursor.compare_exchange(
                start,
                candidate.next_start_index,
                AtomicOrdering::Relaxed,
                AtomicOrdering::Relaxed,
            ) {
                Ok(_) => break candidate,
                Err(current) => start = current,
            }
        };
        let remaining = remaining_in_cycle.get_or_insert(batch.total_peer_count);
        if batch.peer_ids.is_empty() || *remaining == 0 {
            return Vec::new();
        }
        *remaining = remaining.saturating_sub(batch.peer_ids.len());
        let mut targets = batch
            .peer_ids
            .into_iter()
            .filter(|peer| peer != local_peer)
            .collect::<Vec<_>>();
        targets.sort();
        targets.dedup();
        if !targets.is_empty() {
            return targets;
        }
        if *remaining == 0 {
            return Vec::new();
        }
    }
}

impl ProductionV2Services {
    #[cfg(test)]
    pub(in crate::sumeragi) fn queue_plan_test_kura(&self) -> &Kura {
        &self.kura
    }

    #[cfg(test)]
    pub(in crate::sumeragi) fn queue_plan_test_route(
        &self,
        view: wire::View,
    ) -> (NetworkId, PeerId) {
        let peer = self.context.roster[usize::try_from(self.context.leader(view)).unwrap()]
            .validator
            .clone();
        (self.context.network_id.clone(), peer)
    }

    pub(crate) fn queue_plan_admission_batch_sources(
        &self,
    ) -> Result<QueuePlanBatchSources, String> {
        self.kura
            .pending_queue_plan_admission_hash_inventory()
            .map(|inventory| QueuePlanBatchSources {
                inventory,
                bodies: HashMap::new(),
                validated: HashSet::new(),
            })
            .map_err(|error| error.to_string())
    }

    pub(crate) fn can_retain_lane_work_effect(
        &self,
        effect: &V2LaneWorkEffect,
    ) -> Result<bool, String> {
        let mut sources = if matches!(
            effect,
            V2LaneWorkEffect::PostQueuePlanAdmissionCertificate { .. }
        ) {
            Some(self.queue_plan_admission_batch_sources()?)
        } else {
            None
        };
        self.can_retain_lane_work_effect_from_snapshot(effect, sources.as_mut())
    }

    fn exact_target_geometry(
        peer: &PeerId,
        reply_routes: Option<&NetworkReplyRoutes>,
    ) -> Result<
        (
            Vec<PeerId>,
            Vec<ExactTargetRoute>,
            Option<NetworkReplyRoutes>,
        ),
        String,
    > {
        let Some(reply_routes) = reply_routes else {
            return Ok((vec![peer.clone()], vec![ExactTargetRoute::Topology], None));
        };
        if reply_routes.semantic_target() != peer || reply_routes.is_empty() {
            return Err("Sumeragi v2 effect has invalid reply-route ownership".to_owned());
        }
        let routes = reply_routes
            .iter()
            .cloned()
            .map(ExactTargetRoute::Reply)
            .collect::<Vec<_>>();
        Ok((
            vec![peer.clone(); routes.len()],
            routes,
            Some(reply_routes.clone()),
        ))
    }

    fn remote_voters(&self) -> Vec<PeerId> {
        self.context
            .roster
            .iter()
            .filter(|entry| entry.validator != self.local_peer)
            .map(|entry| entry.validator.clone())
            .collect()
    }

    /// Prefer the live authenticated topology for historical archive traffic.
    ///
    /// The supplied sources remain the deterministic frozen authority retained
    /// in the effect/WAL. They are used only when the live topology has no
    /// remote target, so key rotation cannot strand historical catch-up while
    /// an empty discovery snapshot cannot suppress the frozen fallback.
    fn current_archive_targets_with_frozen_fallback(
        &self,
        frozen_sources: &[PeerId],
    ) -> Vec<PeerId> {
        let limit = self.network.reply_route_source_capacity().max(1);
        let targets = rotating_current_archive_targets(
            &self.local_peer,
            &self.archive_peer_cursor,
            limit,
            |start, limit| self.network.configured_peer_ids_bounded(start, limit),
        );
        if !targets.is_empty() {
            return targets;
        }
        let mut fallback = frozen_sources
            .iter()
            .filter(|peer| *peer != &self.local_peer)
            .cloned()
            .collect::<Vec<_>>();
        fallback.sort();
        fallback.dedup();
        fallback
    }

    fn queue_plan_effect_parts(
        &self,
        peer: &PeerId,
        view: wire::View,
        certificate: &Arc<Vec<u8>>,
        kura_sources: &mut QueuePlanBatchSources,
    ) -> Result<QueuePlanAdmissionEffectParts, String> {
        let expected_leader = self
            .context
            .roster
            .get(usize::try_from(self.context.leader(view)).unwrap_or(usize::MAX))
            .map(|entry| &entry.validator)
            .ok_or_else(|| "QueuePlan admission handoff view has no frozen leader".to_owned())?;
        if peer != expected_leader {
            return Err(
                "QueuePlan admission handoff target is not its frozen view leader".to_owned(),
            );
        }
        let certificate_hash =
            kura_sources.validate(&self.kura, &self.context.network_id, certificate)?;
        if !kura_sources.contains_exact(&self.kura, certificate)? {
            return Err("QueuePlan admission handoff has no exact durable Kura source".to_owned());
        }
        Ok((
            vec![NetworkMessage::QueuePlanAdmissionCertificate(Arc::clone(
                certificate,
            ))],
            vec![peer.clone()],
            vec![ExactTargetRoute::Topology],
            None,
            None,
            ExactOutputRolloverClaim::QueuePlanAdmission {
                scope: self.exact_output_scope(),
                target: peer.clone(),
                view,
                certificate_hash,
            },
        ))
    }

    /// Hand one exact Kura-durable QueuePlan certificate to the current leader.
    pub(crate) fn post_queue_plan_admission_certificate(
        &self,
        peer: PeerId,
        view: wire::View,
        certificate: Arc<Vec<u8>>,
        kura_sources: &mut QueuePlanBatchSources,
    ) {
        let output_guard = Arc::clone(&self.output_guard);
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            return;
        };
        let Ok((messages, peers, _, _, _, rollover_claim)) =
            self.queue_plan_effect_parts(&peer, view, &certificate, kura_sources)
        else {
            iroha_logger::error!(%peer, view, "QueuePlan handoff lost its leader or Kura source");
            return;
        };
        match self.enqueue_exact_fanout_while_guarded(
            messages,
            peers,
            rollover_claim,
            operation.permit(),
        ) {
            Ok(ExactFanoutOwnership::Owned) => operation.complete(),
            Ok(ExactFanoutOwnership::SourceRetained) => iroha_logger::error!(
                "QueuePlan handoff reached an unreserved outbound corridor boundary"
            ),
            Err(error) => iroha_logger::error!(%error, "QueuePlan handoff failed closed"),
        }
    }
}
