const CANONICAL_EXECUTED_BLOCK_CHUNK_BYTES: usize = MAX_CERTIFIED_MERGE_CHUNK_BYTES;
const CANONICAL_EXECUTED_BLOCK_MAX_CHUNKS: usize =
    (STRICT_INIT_MAX_BLOCK_BYTES as usize).div_ceil(CANONICAL_EXECUTED_BLOCK_CHUNK_BYTES);
fn canonical_executed_block_request_fits_frame(
    limits: V2LaneWorkLimits,
    request: &LaneHistoricalRecoveryRequestV1,
) -> bool {
    BlockMessage::LaneHistoricalRecoveryRequest(Box::new(request.clone()))
        .encode()
        .len()
        <= limits
            .merge_share_frame_capacity
            .get()
            .min(MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES)
}
fn canonical_executed_block_response_fits_frame(
    limits: V2LaneWorkLimits,
    response: &LaneHistoricalRecoveryResponseV1,
) -> bool {
    let bytes = BlockMessage::LaneHistoricalRecoveryResponse(Box::new(response.clone()))
        .encode()
        .len();
    super::fair_v2_ingress_required_lane_p2p_frame_bytes(bytes)
        <= limits.historical_recovery_response_frame_capacity.get()
}
/// Build one exact chunk-recovery dependency from locally verified durable
/// State, Kura finality, and the consensus-signed canonical wire length.
pub(crate) fn canonical_executed_block_need_for_height(
    context: &wire::HeightContext,
    state: &State,
    kura: &Kura,
    height: u64,
    expected_hash: HashOf<BlockHeader>,
) -> Result<CanonicalExecutedBlockNeedV1, V2LaneWorkError> {
    let (header, finality) = kura
        .v2_finality_artifact_with_header(height)
        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?
        .ok_or_else(|| {
            V2LaneWorkError::Persistence(format!(
                "canonical executed-block repair lacks durable finality at height {height}"
            ))
        })?;
    let execution_commitment = finality.commit_qc.execution_commitment;
    let need = CanonicalExecutedBlockNeedV1 {
        height,
        block_hash: expected_hash,
        finality_artifact_hash: HashOf::new(&finality),
        execution_commitment,
        executed_block_wire_len: execution_commitment.executed_block_wire_len,
        executed_block_wire_hash: execution_commitment.executed_block_wire_hash,
    };
    if header.hash() != expected_hash
        || kura.durable_block_payload_len_by_hash(expected_hash)
            != Some((height, need.executed_block_wire_len))
    {
        return Err(V2LaneWorkError::Persistence(format!(
            "canonical executed-block repair has conflicting durable identity at height {height}"
        )));
    }
    validate_canonical_executed_block_need(context, state, kura, need)
        .map_err(V2LaneWorkError::Persistence)?;
    Ok(need)
}
fn validate_canonical_executed_block_need(
    context: &wire::HeightContext,
    state: &State,
    kura: &Kura,
    need: CanonicalExecutedBlockNeedV1,
) -> Result<wire::finality::V2FinalityArtifact, String> {
    if need.height == 0
        || need.height > u64::try_from(state.committed_height()).unwrap_or(u64::MAX)
        || need.height > context.height
        || need.execution_commitment.validate().is_err()
        || need.executed_block_wire_len == 0
        || need.executed_block_wire_len > STRICT_INIT_MAX_BLOCK_BYTES
        || need.executed_block_wire_len != need.execution_commitment.executed_block_wire_len
        || need.executed_block_wire_hash != need.execution_commitment.executed_block_wire_hash
        || state.committed_block_hash_at_height(need.height) != Some(need.block_hash)
    {
        return Err(
            "canonical executed-block need has an invalid State, height, or execution binding"
                .to_owned(),
        );
    }
    if kura.durable_block_payload_len_by_hash(need.block_hash)
        != Some((need.height, need.executed_block_wire_len))
    {
        return Err(
            "canonical executed-block need differs from durable Kura length authority".to_owned(),
        );
    }
    let (header, finality) = kura
        .v2_finality_artifact_with_header(need.height)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical executed-block need lacks durable finality".to_owned())?;
    if HashOf::new(&finality) != need.finality_artifact_hash
        || finality.verify().is_err()
        || finality.validate_for_header(&header).is_err()
        || finality.height != need.height
        || header.height().get() != need.height
        || finality.block_hash != need.block_hash
        || header.hash() != need.block_hash
        || (need.height > 1
            && state.committed_block_hash_at_height(need.height - 1) != header.prev_block_hash())
        || (need.height == 1 && header.prev_block_hash().is_some())
        || finality.commit_qc.execution_commitment != need.execution_commitment
        || finality.height_context.network_id != context.network_id
    {
        return Err("canonical executed-block need differs from exact local finality".to_owned());
    }
    Ok(finality)
}
fn validate_canonical_executed_block_request(
    context: &wire::HeightContext,
    state: &State,
    kura: &Kura,
    limits: V2LaneWorkLimits,
    request: &LaneHistoricalRecoveryRequestV1,
    sender: &PeerId,
) -> Result<
    (
        CanonicalExecutedBlockNeedV1,
        wire::finality::V2FinalityArtifact,
        u32,
    ),
    String,
> {
    let LaneHistoricalRecoveryKindV1::CanonicalExecutedBlock { need, chunk_index } = &request.kind
    else {
        return Err("request is not canonical executed-block recovery".to_owned());
    };
    if request.version != LANE_HISTORICAL_RECOVERY_VERSION_V1
        || &request.requester != sender
        || request.certificate.is_some()
        || !request.signer_pops.is_empty()
        || usize::try_from(*chunk_index).ok().is_none_or(|index| {
            usize::try_from(need.executed_block_wire_len)
                .ok()
                .map(|len| len.div_ceil(CANONICAL_EXECUTED_BLOCK_CHUNK_BYTES))
                .is_none_or(|count| index >= count || count > CANONICAL_EXECUTED_BLOCK_MAX_CHUNKS)
        })
        || !canonical_executed_block_request_fits_frame(limits, request)
    {
        return Err(
            "canonical executed-block recovery request has invalid shape, sender, chunk, or size"
                .to_owned(),
        );
    }
    let finality = validate_canonical_executed_block_need(context, state, kura, **need)?;
    let requester_is_authorized = context
        .roster
        .iter()
        .any(|entry| &entry.validator == sender)
        || finality
            .height_context
            .roster
            .iter()
            .any(|entry| &entry.validator == sender);
    if !requester_is_authorized {
        return Err(
            "canonical executed-block requester is outside authenticated rosters".to_owned(),
        );
    }
    Ok((**need, finality, *chunk_index))
}
fn canonical_executed_block_matches_need(
    block: &SignedBlock,
    finality: &wire::finality::V2FinalityArtifact,
    need: CanonicalExecutedBlockNeedV1,
) -> bool {
    let Ok(wire) = block.encode_wire() else {
        return false;
    };
    finality.verify().is_ok()
        && finality.validate_for_header(&block.header()).is_ok()
        && finality.height == need.height
        && finality.block_hash == need.block_hash
        && finality.commit_qc.execution_commitment == need.execution_commitment
        && block.header().height().get() == need.height
        && block.hash() == need.block_hash
        && u64::try_from(wire.len()).ok() == Some(need.executed_block_wire_len)
        && Hash::new(&wire) == need.executed_block_wire_hash
}
fn build_canonical_executed_block_response(
    context: &wire::HeightContext,
    state: &State,
    kura: &Kura,
    limits: V2LaneWorkLimits,
    request: &LaneHistoricalRecoveryRequestV1,
    sender: &PeerId,
) -> Result<LaneHistoricalRecoveryResponseV1, String> {
    let (need, finality, chunk_index) =
        validate_canonical_executed_block_request(context, state, kura, limits, request, sender)?;
    let height = usize::try_from(need.height)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| "canonical executed-block height is invalid".to_owned())?;
    let block = kura
        .get_block_without_merge_sidecar(height)
        .ok_or_else(|| "canonical executed block is unavailable at responder".to_owned())?;
    if !canonical_executed_block_matches_need(&block, &finality, need) {
        return Err("canonical executed-block response differs from finality".to_owned());
    }
    let wire = block.encode_wire().map_err(|error| error.to_string())?;
    if u64::try_from(wire.len()).ok() != Some(need.executed_block_wire_len)
        || Hash::new(&wire) != need.executed_block_wire_hash
    {
        return Err("canonical executed-block wire exceeds the durable block bound".to_owned());
    }
    let chunk_count = wire.len().div_ceil(CANONICAL_EXECUTED_BLOCK_CHUNK_BYTES);
    let chunk_index_usize = usize::try_from(chunk_index).map_err(|error| error.to_string())?;
    if chunk_count == 0
        || chunk_count > CANONICAL_EXECUTED_BLOCK_MAX_CHUNKS
        || chunk_index_usize >= chunk_count
    {
        return Err("canonical executed-block chunk is outside the exact wire".to_owned());
    }
    let start = chunk_index_usize
        .checked_mul(CANONICAL_EXECUTED_BLOCK_CHUNK_BYTES)
        .ok_or_else(|| "canonical executed-block chunk offset overflow".to_owned())?;
    let end = start
        .saturating_add(CANONICAL_EXECUTED_BLOCK_CHUNK_BYTES)
        .min(wire.len());
    let response = LaneHistoricalRecoveryResponseV1 {
        version: LANE_HISTORICAL_RECOVERY_VERSION_V1,
        request_hash: HashOf::new(request),
        payload: LaneHistoricalRecoveryPayloadV1::CanonicalExecutedBlockChunk {
            finality_artifact: finality,
            wire_len: need.executed_block_wire_len,
            chunk_index,
            chunk_count: u32::try_from(chunk_count).map_err(|error| error.to_string())?,
            bytes: wire[start..end].to_vec(),
        },
    };
    if !canonical_executed_block_response_fits_frame(limits, &response) {
        return Err(
            "canonical executed-block chunk exceeds the configured authenticated response frame"
                .to_owned(),
        );
    }
    Ok(response)
}
#[derive(Clone, Debug)]
struct CanonicalExecutedBlockResponder {
    peer: PeerId,
    index: usize,
    count: usize,
}
#[derive(Clone, Debug)]
struct OutstandingCanonicalExecutedBlockRequest {
    request_hash: HashOf<LaneHistoricalRecoveryRequestV1>,
    request: LaneHistoricalRecoveryRequestV1,
    responder: CanonicalExecutedBlockResponder,
    /// The first timeout retransmits the byte-identical request to the pinned
    /// responder. A subsequent timeout refreshes current archive candidates
    /// and necessarily restarts the wire at chunk zero.
    retry_sent: bool,
}
/// Recovery-only, bounded owner for exact canonical executed bodies required
/// by any durable-evidence repair owner.
///
/// This type deliberately has no Queue handle and no lane signing state. It
/// can therefore run while ordinary Queue publication and full lane work are
/// still disabled. At most one canonical wire is assembled at a time; chunks
/// are requested sequentially from one pinned authenticated archive.
pub(crate) struct CanonicalExecutedBlockRecovery {
    context: wire::HeightContext,
    local_peer: PeerId,
    state: Arc<State>,
    kura: Arc<Kura>,
    output_guard: Arc<ConsensusOutputGuard>,
    limits: V2LaneWorkLimits,
    needs: VecDeque<CanonicalExecutedBlockNeedV1>,
    /// Consecutive sends for the current exact chunk without an accepted successor chunk.
    front_attempts: u32,
    /// Whole-wire assemblies abandoned before the current body was authenticated.
    whole_wire_restarts: u32,
    next_peer_index: usize,
    /// Most recently abandoned responder. A fresh candidate snapshot skips
    /// this peer when another current archive is available.
    last_abandoned_peer: Option<PeerId>,
    assembly_responder: Option<CanonicalExecutedBlockResponder>,
    next_chunk_index: u32,
    assembly_wire_len: Option<usize>,
    assembly_chunk_count: Option<u32>,
    assembly: Vec<u8>,
    outstanding: Option<OutstandingCanonicalExecutedBlockRequest>,
    retired_request_hashes: BTreeSet<HashOf<LaneHistoricalRecoveryRequestV1>>,
    effects: VecDeque<V2LaneWorkEffect>,
}
impl CanonicalExecutedBlockRecovery {
    /// Maximum number of ordered needs owned by one recovery-only corridor.
    pub(crate) const fn need_capacity(limits: V2LaneWorkLimits) -> usize {
        limits.session_capacity.get()
    }
    /// Validate and install one canonical, duplicate-free recovery set.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        context: wire::HeightContext,
        local_peer: PeerId,
        state: Arc<State>,
        kura: Arc<Kura>,
        output_guard: Arc<ConsensusOutputGuard>,
        limits: V2LaneWorkLimits,
        needs: Vec<CanonicalExecutedBlockNeedV1>,
    ) -> Result<Self, V2LaneWorkError> {
        if needs.is_empty()
            || needs.len() > Self::need_capacity(limits)
            || needs
                .windows(2)
                .any(|pair| pair[0].height >= pair[1].height)
        {
            return Err(V2LaneWorkError::Persistence(
                "canonical executed-block recovery needs are empty, unordered, duplicated, or over capacity"
                    .to_owned(),
            ));
        }
        context
            .validate()
            .map_err(|error| V2LaneWorkError::InvalidContext(error.to_string()))?;
        for need in &needs {
            validate_canonical_executed_block_need(&context, state.as_ref(), kura.as_ref(), *need)
                .map_err(V2LaneWorkError::Persistence)?;
        }
        Ok(Self {
            context,
            local_peer,
            state,
            kura,
            output_guard,
            limits,
            needs: needs.into(),
            front_attempts: 0,
            whole_wire_restarts: 0,
            next_peer_index: 0,
            last_abandoned_peer: None,
            assembly_responder: None,
            next_chunk_index: 0,
            assembly_wire_len: None,
            assembly_chunk_count: None,
            assembly: Vec::new(),
            outstanding: None,
            retired_request_hashes: BTreeSet::new(),
            effects: VecDeque::new(),
        })
    }
    /// Return whether at least one exact canonical body is still missing.
    pub(crate) fn has_pending(&self) -> bool {
        !self.needs.is_empty()
    }
    /// Number of bounded outbound recovery effects awaiting transport.
    pub(crate) fn effect_count(&self) -> usize {
        self.effects.len()
    }
    /// Drain at most `limit` recovery-only transport effects.
    pub(crate) fn drain_effects(&mut self, limit: usize) -> Vec<V2LaneWorkEffect> {
        let count = limit.min(self.effects.len());
        self.effects.drain(..count).collect()
    }
    /// Restore one source-owned effect after downstream backpressure.
    pub(crate) fn requeue_effect(&mut self, effect: V2LaneWorkEffect) -> bool {
        if self.effects.len() >= self.limits.effect_capacity.get() {
            return false;
        }
        self.effects.push_front(effect);
        true
    }
    fn has_queued_local_request(&self) -> bool {
        self.effects.iter().any(|effect| {
            matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneHistoricalRecoveryRequest(request),
                    ..
                } if request.requester == self.local_peer
                    && matches!(
                        &request.kind,
                        LaneHistoricalRecoveryKindV1::CanonicalExecutedBlock { .. }
                    )
            )
        })
    }
    /// Return whether `effect` is the exact request currently awaiting transport.
    pub(crate) fn is_current_request_effect(&self, effect: &V2LaneWorkEffect) -> bool {
        let Some(outstanding) = self.outstanding.as_ref() else {
            return false;
        };
        matches!(
            effect,
            V2LaneWorkEffect::PostLaneBlock {
                peer,
                message: BlockMessage::LaneHistoricalRecoveryRequest(request),
            } if peer == &outstanding.responder.peer
                && request.as_ref() == &outstanding.request
        )
    }
    fn retire_outstanding_request(&mut self) {
        let Some(outstanding) = self.outstanding.take() else {
            return;
        };
        if self
            .retired_request_hashes
            .contains(&outstanding.request_hash)
            || self.retired_request_hashes.len() < Self::need_capacity(self.limits)
        {
            self.retired_request_hashes.insert(outstanding.request_hash);
        } else {
            self.output_guard.close_admission_for_restart();
        }
    }
    /// Drain canonical request identities superseded by exact chunk progress,
    /// responder abandonment, or local cache completion.
    pub(crate) fn drain_retired_request_hashes(
        &mut self,
    ) -> BTreeSet<HashOf<LaneHistoricalRecoveryRequestV1>> {
        std::mem::take(&mut self.retired_request_hashes)
    }
    /// Restore a cancellation batch after downstream ownership preflight
    /// failed without mutating its exact-output corridor.
    pub(crate) fn requeue_retired_request_hashes(
        &mut self,
        request_hashes: BTreeSet<HashOf<LaneHistoricalRecoveryRequestV1>>,
    ) -> Result<(), V2LaneWorkError> {
        let additional = request_hashes
            .difference(&self.retired_request_hashes)
            .count();
        let next_len = self
            .retired_request_hashes
            .len()
            .checked_add(additional)
            .ok_or_else(|| {
                V2LaneWorkError::Persistence(
                    "retired canonical recovery cancellation capacity overflowed".to_owned(),
                )
            })?;
        if next_len > Self::need_capacity(self.limits) {
            self.output_guard.close_admission_for_restart();
            return Err(V2LaneWorkError::Persistence(
                "retired canonical recovery cancellations exceeded the need bound".to_owned(),
            ));
        }
        self.retired_request_hashes.extend(request_hashes);
        Ok(())
    }
    fn reset_front_assembly(&mut self) {
        self.front_attempts = 0;
        self.assembly_responder = None;
        self.next_chunk_index = 0;
        self.assembly_wire_len = None;
        self.assembly_chunk_count = None;
        // Drop even the allocation: a failed archive must not leave a large
        // poisoned prefix resident while the next archive restarts at zero.
        self.assembly = Vec::new();
        self.retire_outstanding_request();
    }
    fn record_front_attempt(&mut self) -> Result<(), V2LaneWorkError> {
        let limit = self
            .limits
            .historical_recovery_stuck_attempts
            .get()
            .saturating_mul(self.limits.historical_recovery_max_retry_tier.get());
        if self.front_attempts >= limit {
            return Err(V2LaneWorkError::Persistence(format!(
                "canonical executed-block recovery exhausted {limit} bounded attempts without exact chunk progress"
            )));
        }
        self.front_attempts = self.front_attempts.saturating_add(1);
        Ok(())
    }
    fn record_whole_wire_restart(&mut self) -> Result<(), V2LaneWorkError> {
        let limit = self
            .limits
            .historical_recovery_stuck_attempts
            .get()
            .saturating_mul(self.limits.historical_recovery_max_retry_tier.get());
        if self.whole_wire_restarts >= limit {
            return Err(V2LaneWorkError::Persistence(format!(
                "canonical executed-block recovery exhausted {limit} bounded whole-wire restarts without completion"
            )));
        }
        self.whole_wire_restarts = self.whole_wire_restarts.saturating_add(1);
        Ok(())
    }
    fn advance_past_front_responder(&mut self) {
        let responder = self
            .assembly_responder
            .as_ref()
            .or_else(|| {
                self.outstanding
                    .as_ref()
                    .map(|outstanding| &outstanding.responder)
            })
            .cloned();
        if let Some(responder) = responder {
            self.next_peer_index = (responder.index + 1) % responder.count.max(1);
            self.last_abandoned_peer = Some(responder.peer);
        }
        self.reset_front_assembly();
    }
    fn abandon_front_responder(&mut self) -> Result<(), V2LaneWorkError> {
        // Charge the retained assembly before rotating or dropping any of its
        // ownership. Exhaustion therefore leaves the exact failure state intact.
        self.record_whole_wire_restart()?;
        self.advance_past_front_responder();
        Ok(())
    }
    fn abandon_front_responder_after_append(
        &mut self,
        retained_prefix_len: usize,
    ) -> Result<(), V2LaneWorkError> {
        match self.abandon_front_responder() {
            Ok(()) => Ok(()),
            Err(error) => {
                // Authentication needs the complete candidate wire. If the
                // restart budget is already exhausted, roll the speculative
                // suffix back so the fallible abandonment leaves the exact
                // pinned prefix and request authority intact.
                if retained_prefix_len == 0 {
                    self.assembly = Vec::new();
                } else {
                    self.assembly.truncate(retained_prefix_len);
                }
                Err(error)
            }
        }
    }
    fn reconcile_cached_front(&mut self) -> Result<(), V2LaneWorkError> {
        loop {
            let Some(need) = self.needs.front().copied() else {
                self.whole_wire_restarts = 0;
                self.last_abandoned_peer = None;
                self.reset_front_assembly();
                return Ok(());
            };
            let height = usize::try_from(need.height)
                .ok()
                .and_then(NonZeroUsize::new)
                .ok_or_else(|| {
                    V2LaneWorkError::Persistence(
                        "canonical executed-block recovery has an invalid height".to_owned(),
                    )
                })?;
            let Some(block) = self.kura.get_block_without_merge_sidecar(height) else {
                return Ok(());
            };
            let finality = validate_canonical_executed_block_need(
                &self.context,
                self.state.as_ref(),
                self.kura.as_ref(),
                need,
            )
            .map_err(V2LaneWorkError::Persistence)?;
            if !canonical_executed_block_matches_need(&block, &finality, need) {
                return Err(V2LaneWorkError::Persistence(
                    "locally cached canonical executed block conflicts with its exact need"
                        .to_owned(),
                ));
            }
            self.kura
                .preflight_cached_finalized_merge_carrier_reconstruction(&block)
                .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
            self.needs.pop_front();
            self.whole_wire_restarts = 0;
            self.reset_front_assembly();
        }
    }
    /// Queue one bounded retry for the current chunk.
    ///
    /// Returns `true` exactly when a new transport request was retained.
    ///
    /// A body assembly is pinned to one exact authenticated archive. Its
    /// first timeout retransmits the exact request bytes to that archive. A
    /// second timeout abandons every unverified prefix, refreshes from the
    /// supplied current configured-peer snapshot, and restarts at chunk zero.
    #[cfg(test)]
    pub(crate) fn service_next(&mut self) -> Result<bool, V2LaneWorkError> {
        self.service_next_with_archive_targets(&[])
    }
    /// Queue one bounded retry using current configured archives first.
    ///
    /// An empty usable snapshot falls back to the exact historical CommitQC
    /// signers. Once a responder supplies the first exact chunk, the complete
    /// wire remains pinned to it until success or the existing abandonment
    /// transition clears the unverified prefix.
    pub(crate) fn service_next_with_archive_targets(
        &mut self,
        current_archive_targets: &[PeerId],
    ) -> Result<bool, V2LaneWorkError> {
        let guard = Arc::clone(&self.output_guard);
        let Some(_permit) = guard.acquire() else {
            return Err(V2LaneWorkError::RestartRequired);
        };
        self.reconcile_cached_front()?;
        let Some(need) = self.needs.front().copied() else {
            return Ok(false);
        };
        if self.has_queued_local_request() {
            return Ok(false);
        }
        if self.effects.len() >= self.limits.effect_capacity.get() {
            return Ok(false);
        }
        let finality = validate_canonical_executed_block_need(
            &self.context,
            self.state.as_ref(),
            self.kura.as_ref(),
            need,
        )
        .map_err(V2LaneWorkError::Persistence)?;
        if let Some(outstanding) = self.outstanding.as_ref() {
            let responder_is_still_exact = self.assembly_responder.as_ref().is_some_and(|pinned| {
                pinned.peer == outstanding.responder.peer
                    && pinned.index == outstanding.responder.index
                    && pinned.count == outstanding.responder.count
            });
            if !responder_is_still_exact {
                self.abandon_front_responder()?;
            } else if !outstanding.retry_sent {
                let request = outstanding.request.clone();
                let peer = outstanding.responder.peer.clone();
                self.record_front_attempt()?;
                self.effects.push_back(V2LaneWorkEffect::PostLaneBlock {
                    peer,
                    message: BlockMessage::LaneHistoricalRecoveryRequest(Box::new(request)),
                });
                self.outstanding
                    .as_mut()
                    .expect("outstanding request was just observed")
                    .retry_sent = true;
                return Ok(true);
            } else {
                self.abandon_front_responder()?;
            }
        }
        let responder = match self.assembly_responder.as_ref() {
            Some(responder) => responder.clone(),
            None => {
                let mut seen = BTreeSet::new();
                let mut eligible_peers = current_archive_targets
                    .iter()
                    .filter(|peer| *peer != &self.local_peer && seen.insert((*peer).clone()))
                    .cloned()
                    .collect::<Vec<_>>();
                if eligible_peers.is_empty() {
                    eligible_peers = finality
                        .commit_qc
                        .signers
                        .iter()
                        .filter_map(|index| {
                            usize::try_from(*index)
                                .ok()
                                .and_then(|index| finality.height_context.roster.get(index))
                                .map(|entry| entry.validator.clone())
                        })
                        .filter(|peer| peer != &self.local_peer && seen.insert(peer.clone()))
                        .collect::<Vec<_>>();
                }
                if eligible_peers.is_empty() {
                    return Err(V2LaneWorkError::Persistence(
                        "canonical executed-block recovery has no current archive or remote CommitQC signer"
                            .to_owned(),
                    ));
                }
                let mut index = self.next_peer_index % eligible_peers.len();
                if eligible_peers.len() > 1
                    && self.last_abandoned_peer.as_ref() == eligible_peers.get(index)
                {
                    index = (index + 1) % eligible_peers.len();
                }
                self.last_abandoned_peer = None;
                CanonicalExecutedBlockResponder {
                    peer: eligible_peers[index].clone(),
                    index,
                    count: eligible_peers.len(),
                }
            }
        };
        self.assembly_responder = Some(responder.clone());
        let request = LaneHistoricalRecoveryRequestV1 {
            version: LANE_HISTORICAL_RECOVERY_VERSION_V1,
            requester: self.local_peer.clone(),
            certificate: None,
            signer_pops: BTreeMap::new(),
            kind: LaneHistoricalRecoveryKindV1::CanonicalExecutedBlock {
                need: Box::new(need),
                chunk_index: self.next_chunk_index,
            },
        };
        if !canonical_executed_block_request_fits_frame(self.limits, &request) {
            return Err(V2LaneWorkError::Persistence(
                "canonical executed-block recovery request exceeds its authenticated frame"
                    .to_owned(),
            ));
        }
        self.record_front_attempt()?;
        let request_hash = HashOf::new(&request);
        self.effects.push_back(V2LaneWorkEffect::PostLaneBlock {
            peer: responder.peer.clone(),
            message: BlockMessage::LaneHistoricalRecoveryRequest(Box::new(request.clone())),
        });
        self.outstanding = Some(OutstandingCanonicalExecutedBlockRequest {
            request_hash,
            request,
            responder,
            retry_sent: false,
        });
        Ok(true)
    }
    /// Return whether recovery-only ingress owns this exact message.
    pub(crate) fn admits_message(&self, message: &BlockMessage) -> bool {
        match message {
            BlockMessage::LaneHistoricalRecoveryRequest(request) => matches!(
                &request.kind,
                LaneHistoricalRecoveryKindV1::CanonicalExecutedBlock { .. }
            ),
            BlockMessage::LaneHistoricalRecoveryResponse(response) => self
                .outstanding
                .as_ref()
                .is_some_and(|outstanding| outstanding.request_hash == response.request_hash),
            _ => false,
        }
    }
    /// Consume one exact fair-ingress carrier in the recovery-only corridor.
    pub(crate) fn accept_with_ingress_ownership(
        &mut self,
        mut inbound: InboundBlockMessage,
    ) -> Result<V2LaneIngressOutcome, V2LaneWorkError> {
        let guard = Arc::clone(&self.output_guard);
        let Some(_permit) = guard.acquire() else {
            return Err(V2LaneWorkError::RestartRequired);
        };
        let Some(ownership) = inbound.take_ingress_ownership() else {
            return Ok(V2LaneIngressOutcome::Rejected);
        };
        let (message, sender, reply_routes) = inbound.into_message_sender_and_reply_routes();
        if !ownership.validate_exact()
            || !ownership.matches_message(&message)
            || !ownership.matches_semantic_origin(&sender)
            || !ownership.matches_reply_routes(reply_routes.as_ref())
        {
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        match message {
            BlockMessage::LaneHistoricalRecoveryRequest(request) => {
                if self.effects.len() >= self.limits.effect_capacity.get() {
                    return Ok(V2LaneIngressOutcome::Duplicate);
                }
                let response = match build_canonical_executed_block_response(
                    &self.context,
                    self.state.as_ref(),
                    self.kura.as_ref(),
                    self.limits,
                    &request,
                    &sender,
                ) {
                    Ok(response) => response,
                    Err(error) => {
                        iroha_logger::debug!(
                            %error,
                            "rejected canonical executed-block recovery request"
                        );
                        return Ok(V2LaneIngressOutcome::Rejected);
                    }
                };
                self.effects.push_back(V2LaneWorkEffect::PostLaneBlock {
                    peer: sender.clone(),
                    message: BlockMessage::LaneHistoricalRecoveryResponse(Box::new(response)),
                });
                Ok(V2LaneIngressOutcome::Inserted)
            }
            BlockMessage::LaneHistoricalRecoveryResponse(response) => {
                self.accept_response(*response, &sender)
            }
            _ => Ok(V2LaneIngressOutcome::Rejected),
        }
    }
    fn accept_response(
        &mut self,
        response: LaneHistoricalRecoveryResponseV1,
        sender: &PeerId,
    ) -> Result<V2LaneIngressOutcome, V2LaneWorkError> {
        if response.version != LANE_HISTORICAL_RECOVERY_VERSION_V1
            || !canonical_executed_block_response_fits_frame(self.limits, &response)
        {
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        let Some(outstanding) = self.outstanding.as_ref() else {
            return Ok(V2LaneIngressOutcome::Rejected);
        };
        if outstanding.request_hash != response.request_hash
            || HashOf::new(&outstanding.request) != response.request_hash
            || &outstanding.responder.peer != sender
            || self.assembly_responder.as_ref().is_none_or(|pinned| {
                pinned.peer != outstanding.responder.peer
                    || pinned.index != outstanding.responder.index
                    || pinned.count != outstanding.responder.count
            })
        {
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        let LaneHistoricalRecoveryKindV1::CanonicalExecutedBlock {
            need,
            chunk_index: requested_chunk,
        } = &outstanding.request.kind
        else {
            return Ok(V2LaneIngressOutcome::Rejected);
        };
        let need = **need;
        let LaneHistoricalRecoveryPayloadV1::CanonicalExecutedBlockChunk {
            finality_artifact,
            wire_len,
            chunk_index,
            chunk_count,
            bytes,
        } = response.payload
        else {
            return Ok(V2LaneIngressOutcome::Rejected);
        };
        let local_finality = validate_canonical_executed_block_need(
            &self.context,
            self.state.as_ref(),
            self.kura.as_ref(),
            need,
        )
        .map_err(V2LaneWorkError::Persistence)?;
        let signed_wire_len = usize::try_from(need.executed_block_wire_len).ok();
        let chunk_count_usize = usize::try_from(chunk_count).ok();
        let expected_count =
            signed_wire_len.map(|len| len.div_ceil(CANONICAL_EXECUTED_BLOCK_CHUNK_BYTES));
        let chunk_index_usize = usize::try_from(chunk_index).ok();
        let expected_start = chunk_index_usize
            .and_then(|index| index.checked_mul(CANONICAL_EXECUTED_BLOCK_CHUNK_BYTES));
        let expected_len = match (signed_wire_len, expected_start) {
            (Some(signed_wire_len), Some(start)) if start < signed_wire_len => Some(
                CANONICAL_EXECUTED_BLOCK_CHUNK_BYTES.min(signed_wire_len.saturating_sub(start)),
            ),
            _ => None,
        };
        if HashOf::new(&finality_artifact) != need.finality_artifact_hash
            || finality_artifact != local_finality
            || chunk_index != *requested_chunk
            || chunk_index != self.next_chunk_index
            || wire_len != need.executed_block_wire_len
            || signed_wire_len.is_none_or(|len| len == 0)
            || chunk_count_usize != expected_count
            || chunk_count_usize
                .is_none_or(|count| count == 0 || count > CANONICAL_EXECUTED_BLOCK_MAX_CHUNKS)
            || expected_len != Some(bytes.len())
            || self
                .assembly_wire_len
                .is_some_and(|existing| Some(existing) != signed_wire_len)
            || self
                .assembly_chunk_count
                .is_some_and(|existing| existing != chunk_count)
            || expected_start != Some(self.assembly.len())
        {
            self.abandon_front_responder()?;
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        if self.next_chunk_index == 0 {
            let signed_wire_len = signed_wire_len.expect("validated signed wire length");
            if self.assembly.capacity() != 0 || !self.assembly.is_empty() {
                self.abandon_front_responder()?;
                return Ok(V2LaneIngressOutcome::Rejected);
            }
            if self.assembly.try_reserve_exact(signed_wire_len).is_err() {
                self.output_guard.close_admission_for_restart();
                return Err(V2LaneWorkError::Persistence(
                    "canonical executed-block signed wire allocation failed".to_owned(),
                ));
            }
        }
        let retained_prefix_len = self.assembly.len();
        self.assembly.extend_from_slice(&bytes);
        let next_chunk_index = self.next_chunk_index.saturating_add(1);
        if next_chunk_index != chunk_count {
            self.assembly_wire_len = signed_wire_len;
            self.assembly_chunk_count = Some(chunk_count);
            self.retire_outstanding_request();
            self.next_chunk_index = next_chunk_index;
            self.front_attempts = 0;
            return Ok(V2LaneIngressOutcome::Inserted);
        }
        if Some(self.assembly.len()) != signed_wire_len
            || Hash::new(&self.assembly) != need.executed_block_wire_hash
        {
            self.abandon_front_responder_after_append(retained_prefix_len)?;
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        let Ok(block) = decode_versioned_signed_block(&self.assembly) else {
            self.abandon_front_responder_after_append(retained_prefix_len)?;
            return Ok(V2LaneIngressOutcome::Rejected);
        };
        if block
            .encode_wire()
            .map_or(true, |canonical_wire| canonical_wire != self.assembly)
            || !canonical_executed_block_matches_need(&block, &local_finality, need)
        {
            self.abandon_front_responder_after_append(retained_prefix_len)?;
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        self.kura.cache_block_body(&block).map_err(|error| {
            self.output_guard.close_admission_for_restart();
            V2LaneWorkError::Persistence(error.to_string())
        })?;
        self.kura
            .preflight_cached_finalized_merge_carrier_reconstruction(&block)
            .map_err(|error| {
                self.output_guard.close_admission_for_restart();
                V2LaneWorkError::Persistence(error.to_string())
            })?;
        self.needs.pop_front();
        self.whole_wire_restarts = 0;
        self.advance_past_front_responder();
        Ok(V2LaneIngressOutcome::Inserted)
    }
}
/// Read-only startup classification for ordinary and Native application
/// evidence. Missing canonical bodies are recovered by the shared chunked
/// corridor before the complete immutable publication plan is rebuilt.
pub(crate) enum LaneApplicationEvidenceRepairPlanning {
    RecoverCanonicalBodies(Vec<CanonicalExecutedBlockNeedV1>),
    Ready(LaneApplicationEvidenceRepairPlan),
}
pub(crate) struct LaneApplicationEvidenceRepairPlan {
    state_tip_height: usize,
    state_tip_hash: Option<HashOf<BlockHeader>>,
    ordinary_pairs: Vec<crate::kura::CertifiedLaneBlockArtifact>,
    ordinary: Vec<OrdinaryLaneApplicationReceiptRepair>,
    native_carriers: Vec<NativeParticipantCarrierRepair>,
    merge_carriers: Vec<FinalizedMergeCarrierRepair>,
    merge_carrier_repair_authorizations: Vec<Vec<PostCarrierEvidenceRepairAuthorization>>,
    repair_capacity: usize,
}
struct OrdinaryLaneApplicationReceiptRepair {
    session: CommittedLaneBlockSession,
    receipt: LaneBlockApplicationReceiptArtifact,
}
struct NativeParticipantCarrierRepair {
    application_block_height: u64,
    application_block_hash: HashOf<BlockHeader>,
    markers: Vec<crate::state::AppliedNativeAmxParticipantFrontierMarker>,
    block: Arc<SignedBlock>,
}
fn planned_merge_entries_by_carrier(
    repairs: &[FinalizedMergeCarrierRepair],
) -> Result<BTreeMap<(u64, HashOf<BlockHeader>), &MergeLedgerEntry>, V2LaneWorkError> {
    let mut entries = BTreeMap::new();
    for repair in repairs {
        let block = repair.block();
        let carrier_height = block.header().height().get();
        let carrier_hash = block.hash();
        let key = (carrier_height, carrier_hash);
        if entries.insert(key, repair.entry()).is_some() {
            return Err(V2LaneWorkError::Persistence(format!(
                "more than one finalized merge repair names carrier {carrier_height} ({carrier_hash})"
            )));
        }
    }
    Ok(entries)
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LaneApplicationEvidenceRepairSummary {
    pub(crate) ordinary_pairs: usize,
    pub(crate) ordinary_receipts: usize,
    pub(crate) native_carriers: usize,
    pub(crate) native_routes: usize,
    pub(crate) merge_carriers: usize,
}
impl LaneApplicationEvidenceRepairSummary {
    pub(crate) fn publication_count(self) -> usize {
        self.ordinary_pairs
            .saturating_add(self.ordinary_receipts)
            .saturating_add(self.native_carriers)
            .saturating_add(self.merge_carriers)
    }
}
impl LaneApplicationEvidenceRepairPlan {
    pub(crate) fn is_empty(&self) -> bool {
        self.ordinary_pairs.is_empty()
            && self.ordinary.is_empty()
            && self.native_carriers.is_empty()
            && self.merge_carriers.is_empty()
    }
    pub(crate) fn item_count(&self) -> usize {
        self.ordinary_pairs
            .len()
            .saturating_add(self.ordinary.len())
            .saturating_add(self.native_carriers.len())
            .saturating_add(self.merge_carriers.len())
    }
}
fn collect_lane_application_repair_need(
    needs: &mut BTreeMap<u64, CanonicalExecutedBlockNeedV1>,
    need: CanonicalExecutedBlockNeedV1,
) -> Result<(), V2LaneWorkError> {
    match needs.entry(need.height) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(need);
            Ok(())
        }
        std::collections::btree_map::Entry::Occupied(entry) if entry.get() == &need => Ok(()),
        std::collections::btree_map::Entry::Occupied(_) => {
            Err(V2LaneWorkError::Persistence(format!(
                "lane application evidence owners conflict at canonical height {}",
                need.height
            )))
        }
    }
}
/// Preflight every ordinary and Native startup repair owner without publishing
/// any lane evidence. The result is deterministic and predecessor ordered.
pub(crate) fn plan_lane_application_evidence_repair(
    context: &wire::HeightContext,
    state: &State,
    kura: &Kura,
    limits: V2LaneWorkLimits,
) -> Result<LaneApplicationEvidenceRepairPlanning, V2LaneWorkError> {
    let state_tip_height = state.committed_height();
    let state_tip_hash = state.latest_block_hash_fast();
    let certified_snapshot = state
        .lane_application_certified_repair_snapshot_cached(limits.session_capacity.get())
        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
    let ordinary_pairs = certified_snapshot.pair_repairs;
    let ordinary_sessions = certified_snapshot.earliest_unapplied;
    if ordinary_pairs.len().saturating_add(ordinary_sessions.len()) > limits.session_capacity.get()
    {
        return Err(V2LaneWorkError::Persistence(
            "ordinary lane startup repair items exceed startup capacity".to_owned(),
        ));
    }
    let native_markers = state
        .native_amx_participant_frontiers_pending_durable_evidence_snapshot_cached()
        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
    if native_markers.len() > limits.session_capacity.get() {
        return Err(V2LaneWorkError::Persistence(
            "Native AMX application evidence repair exceeds startup capacity".to_owned(),
        ));
    }
    let mut needs = BTreeMap::new();
    let (merge_carriers, missing_merge_carrier_bodies) = kura
        .preflight_finalized_merge_carrier_repairs(
            u64::try_from(state_tip_height).map_err(|error| {
                V2LaneWorkError::Persistence(format!(
                    "State height is not representable for merge-carrier repair: {error}"
                ))
            })?,
            limits.session_capacity.get(),
        )
        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?
        .into_parts();
    let planned_merge_entries = planned_merge_entries_by_carrier(&merge_carriers)?;
    for (height, block_hash) in missing_merge_carrier_bodies {
        let need =
            canonical_executed_block_need_for_height(context, state, kura, height, block_hash)?;
        collect_lane_application_repair_need(&mut needs, need)?;
    }
    let mut ordinary = Vec::new();
    ordinary
        .try_reserve_exact(ordinary_sessions.len())
        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
    for session in ordinary_sessions {
        validate_committed_lane_block_session(&session)
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        let proposal = &session.proposal;
        let descriptor = &proposal.descriptor;
        let hint = proposal.payload_block_hint.ok_or_else(|| {
            V2LaneWorkError::Persistence(
                "ordinary certified lane receipt repair lacks a canonical payload hint".to_owned(),
            )
        })?;
        if hint.proposal_height != descriptor.proposal_height
            || state.committed_block_hash_at_height(hint.proposal_height)
                != Some(hint.proposal_block_hash)
            || !state
                .certified_lane_block_predecessor_is_applied_or_snapshot_anchored_cached(proposal)
        {
            return Err(V2LaneWorkError::Persistence(format!(
                "ordinary certified lane {} changed before receipt preflight",
                descriptor.lane_id.as_u32()
            )));
        }
        match kura
            .preflight_lane_block_application_receipt_repair(proposal)
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?
        {
            LaneBlockApplicationReceiptRepairPreflight::Ready(receipt) => {
                ordinary.push(OrdinaryLaneApplicationReceiptRepair { session, receipt });
            }
            LaneBlockApplicationReceiptRepairPreflight::MissingCanonicalBody => {
                let need = canonical_executed_block_need_for_height(
                    context,
                    state,
                    kura,
                    hint.proposal_height,
                    hint.proposal_block_hash,
                )?;
                collect_lane_application_repair_need(&mut needs, need)?;
            }
        }
    }
    let mut grouped_native = BTreeMap::<
        (u64, HashOf<BlockHeader>),
        Vec<crate::state::AppliedNativeAmxParticipantFrontierMarker>,
    >::new();
    for marker in native_markers {
        if marker.application_block_height == 0
            || marker.application_block_height > u64::try_from(state_tip_height).unwrap_or(u64::MAX)
            || state.committed_block_hash_at_height(marker.application_block_height)
                != Some(marker.application_block_hash)
        {
            return Err(V2LaneWorkError::Persistence(format!(
                "Native AMX route {} names a stale or uncommitted carrier",
                marker.lane_id.as_u32()
            )));
        }
        grouped_native
            .entry((
                marker.application_block_height,
                marker.application_block_hash,
            ))
            .or_default()
            .push(marker);
    }
    let mut native_carriers = Vec::new();
    native_carriers
        .try_reserve_exact(grouped_native.len())
        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
    for ((application_block_height, application_block_hash), mut markers) in grouped_native {
        markers.sort_by_key(|marker| {
            (
                marker.lane_id,
                marker.dataspace_id,
                marker.lane_incarnation,
                marker.lane_block_height,
            )
        });
        let height = usize::try_from(application_block_height)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| {
                V2LaneWorkError::Persistence(
                    "Native AMX carrier height is not representable".to_owned(),
                )
            })?;
        let Some(block) = kura.get_block_without_merge_sidecar(height) else {
            let need = canonical_executed_block_need_for_height(
                context,
                state,
                kura,
                application_block_height,
                application_block_hash,
            )?;
            collect_lane_application_repair_need(&mut needs, need)?;
            continue;
        };
        if block.hash() != application_block_hash {
            return Err(V2LaneWorkError::Persistence(
                "Native AMX recovered carrier conflicts with committed State".to_owned(),
            ));
        }
        let planned_merge_entry = planned_merge_entries
            .get(&(application_block_height, application_block_hash))
            .copied();
        kura.preflight_native_amx_participant_application_evidence_repair(
            &block,
            &markers,
            planned_merge_entry,
        )
        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        native_carriers.push(NativeParticipantCarrierRepair {
            application_block_height,
            application_block_hash,
            markers,
            block,
        });
    }
    drop(planned_merge_entries);
    if !needs.is_empty() {
        return Ok(
            LaneApplicationEvidenceRepairPlanning::RecoverCanonicalBodies(
                needs.into_values().collect(),
            ),
        );
    }
    let network_id = context.network_id;
    let mut merge_carrier_repair_authorizations = Vec::new();
    merge_carrier_repair_authorizations
        .try_reserve_exact(merge_carriers.len())
        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
    for repair in &merge_carriers {
        let block = repair.block();
        let entry = repair.entry();
        let height = block.header().height().get();
        if state.committed_block_hash_at_height(height) != Some(block.hash()) {
            return Err(V2LaneWorkError::Persistence(format!(
                "finalized merge carrier {height} differs from committed State"
            )));
        }
        if entry.execution_batch.is_none() {
            merge_carrier_repair_authorizations.push(Vec::new());
            continue;
        }
        state
            .ensure_committed_merge_execution_applied(entry)
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        let reference = block
            .execution_context()
            .and_then(|bundle| bundle.merge_entry.as_ref())
            .ok_or_else(|| {
                V2LaneWorkError::Persistence(format!(
                    "finalized merge carrier {height} lost its compact reference"
                ))
            })?;
        merge_carrier_repair_authorizations.push(
            post_carrier_evidence_repair_authorizations(
                reference,
                entry,
                network_id,
                height,
                block.hash(),
            )
            .map_err(V2LaneWorkError::Persistence)?,
        );
    }
    ordinary.sort_by_key(|repair| {
        let descriptor = &repair.session.proposal.descriptor;
        (
            descriptor.lane_block_height,
            descriptor.lane_id,
            descriptor.dataspace_id,
            descriptor.lane_block_view,
        )
    });
    Ok(LaneApplicationEvidenceRepairPlanning::Ready(
        LaneApplicationEvidenceRepairPlan {
            state_tip_height,
            state_tip_hash,
            ordinary_pairs,
            ordinary,
            native_carriers,
            merge_carriers,
            merge_carrier_repair_authorizations,
            repair_capacity: limits.session_capacity.get(),
        },
    ))
}
/// Publish one plan only after every item has passed a second read-only
/// preflight. Calls are idempotent; any post-plan drift fails before the first
/// write and every write is followed by exact authoritative readback.
pub(crate) fn apply_lane_application_evidence_repair(
    state: &State,
    kura: &Kura,
    plan: LaneApplicationEvidenceRepairPlan,
) -> Result<LaneApplicationEvidenceRepairSummary, V2LaneWorkError> {
    if state.committed_height() != plan.state_tip_height
        || state.latest_block_hash_fast() != plan.state_tip_hash
    {
        return Err(V2LaneWorkError::Persistence(
            "State changed after lane application evidence startup preflight".to_owned(),
        ));
    }
    let current_certified = state
        .lane_application_certified_repair_snapshot_cached(plan.repair_capacity)
        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
    if current_certified.pair_repairs != plan.ordinary_pairs
        || current_certified.earliest_unapplied
            != plan
                .ordinary
                .iter()
                .map(|repair| repair.session.clone())
                .collect::<Vec<_>>()
    {
        return Err(V2LaneWorkError::Persistence(
            "ordinary certified lane repair owners changed after all-item startup preflight"
                .to_owned(),
        ));
    }
    for repair in &plan.ordinary {
        let current = kura
            .preflight_lane_block_application_receipt_repair(&repair.session.proposal)
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        if current != LaneBlockApplicationReceiptRepairPreflight::Ready(repair.receipt.clone())
            || !state.certified_lane_block_predecessor_is_applied_or_snapshot_anchored_cached(
                &repair.session.proposal,
            )
        {
            return Err(V2LaneWorkError::Persistence(
                "ordinary lane receipt changed after all-item startup preflight".to_owned(),
            ));
        }
    }
    let planned_merge_entries = planned_merge_entries_by_carrier(&plan.merge_carriers)?;
    for carrier in &plan.native_carriers {
        if carrier.block.header().height().get() != carrier.application_block_height
            || carrier.block.hash() != carrier.application_block_hash
            || state.committed_block_hash_at_height(carrier.application_block_height)
                != Some(carrier.application_block_hash)
        {
            return Err(V2LaneWorkError::Persistence(
                "Native AMX carrier changed after all-item startup preflight".to_owned(),
            ));
        }
        kura.preflight_native_amx_participant_application_evidence_repair(
            &carrier.block,
            &carrier.markers,
            planned_merge_entries
                .get(&(
                    carrier.application_block_height,
                    carrier.application_block_hash,
                ))
                .copied(),
        )
        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
    }
    drop(planned_merge_entries);
    let (current_merge_carriers, missing_merge_bodies) = kura
        .preflight_finalized_merge_carrier_repairs(
            u64::try_from(plan.state_tip_height).map_err(|error| {
                V2LaneWorkError::Persistence(format!(
                    "State height changed representation after merge-carrier preflight: {error}"
                ))
            })?,
            plan.repair_capacity,
        )
        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?
        .into_parts();
    if !missing_merge_bodies.is_empty() || current_merge_carriers != plan.merge_carriers {
        return Err(V2LaneWorkError::Persistence(
            "finalized merge-carrier repair changed after all-item startup preflight".to_owned(),
        ));
    }
    if plan.merge_carrier_repair_authorizations.len() != plan.merge_carriers.len() {
        return Err(V2LaneWorkError::Persistence(
            "finalized merge-carrier repair authority cardinality changed after startup preflight"
                .to_owned(),
        ));
    }
    for repair in &plan.merge_carriers {
        if repair.entry().execution_batch.is_some() {
            state
                .ensure_committed_merge_execution_applied(repair.entry())
                .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        }
    }
    let mut summary = LaneApplicationEvidenceRepairSummary::default();
    for artifact in &plan.ordinary_pairs {
        let session = CommittedLaneBlockSession {
            proposal: artifact.proposal.clone(),
            prepare_qc: artifact.prepare_qc.clone(),
            commit_qc: artifact.commit_qc.clone(),
        };
        state
            .persist_committed_lane_block_session_lifecycle_bound(&session, &artifact.signer_pops)
            .map_err(V2LaneWorkError::Persistence)?;
        summary.ordinary_pairs = summary.ordinary_pairs.saturating_add(1);
    }
    summary.merge_carriers = kura
        .apply_finalized_merge_carrier_repairs(
            &plan.merge_carriers,
            plan.merge_carrier_repair_authorizations,
        )
        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
    for repair in &plan.ordinary {
        kura.persist_preflighted_lane_block_application_receipt(&repair.receipt)
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        if !kura.lane_block_application_receipt_available(&repair.session.proposal) {
            return Err(V2LaneWorkError::Persistence(
                "ordinary lane receipt startup publication failed exact readback".to_owned(),
            ));
        }
        summary.ordinary_receipts = summary.ordinary_receipts.saturating_add(1);
    }
    for carrier in &plan.native_carriers {
        let repaired_routes = kura
            .repair_native_amx_participant_application_evidence_for_markers(
                &carrier.block,
                &carrier.markers,
            )
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        if repaired_routes != carrier.markers.len() {
            return Err(V2LaneWorkError::Persistence(
                "Native AMX startup publication did not cover every exact State marker".to_owned(),
            ));
        }
        summary.native_carriers = summary.native_carriers.saturating_add(1);
        summary.native_routes = summary.native_routes.saturating_add(repaired_routes);
    }
    let unresolved_native = state
        .native_amx_participant_frontiers_pending_durable_evidence_snapshot_cached()
        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
    if plan
        .native_carriers
        .iter()
        .flat_map(|carrier| carrier.markers.iter())
        .any(|marker| unresolved_native.contains(marker))
    {
        return Err(V2LaneWorkError::Persistence(
            "Native AMX startup publication failed exact State/Kura readback".to_owned(),
        ));
    }
    Ok(summary)
}
