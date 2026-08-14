# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

@pytest.mark.parametrize(
    ("relative_path", "region_marker", "old", "new", "error_fragment"),
    (
        (
            "crates/iroha_core/src/lib.rs",
            "pub enum NetworkMessage",
            "CertifiedMergeSidecar(Arc<CertifiedMergeSidecarMessage>),",
            "CertifiedMergeSidecar(Box<CertifiedMergeSidecarMessage>),",
            "every exact-output network payload class must use an immutable shared carrier",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct OutboundTransfer",
            "chunks: Vec<Arc<CertifiedMergeSidecarMessage>>",
            "chunks: Vec<CertifiedMergeSidecarMessage>",
            "sidecar responses must cache each immutable fixed-boundary payload once for every source cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn drain_outbound_chunks_inner(",
            "let message = Arc::clone(\n"
            "                            transfer\n"
            "                                .chunks\n"
            "                                .get(index)\n"
            "                                .expect(\"bounded sidecar cursor names a cached chunk\"),\n"
            "                        );",
            "let message = Arc::new(\n"
            "                            transfer\n"
            "                                .chunks\n"
            "                                .get(index)\n"
            "                                .expect(\"bounded sidecar cursor names a cached chunk\")\n"
            "                                .as_ref()\n"
            "                                .clone(),\n"
            "                        );",
            "sidecar drainage must clone only the cached Arc",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "let projection = admission.projection();",
            "let _rebuilt_payload = Vec::<u8>::new().to_vec();\n"
            "        let projection = admission.projection();",
            "per-source sidecar drainage and acknowledgement must never reconstruct cached payload bytes",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) enum V2LaneWorkEffect",
            "message: Arc<CertifiedMergeSidecarMessage>,",
            "message: CertifiedMergeSidecarMessage,",
            "the lane effect must preserve the exact immutable sidecar carrier",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn post_certified_merge_sidecar_with_reply_routes(",
            "let data = NetworkMessage::CertifiedMergeSidecar(message);",
            "let data = NetworkMessage::CertifiedMergeSidecar(Arc::new((*message).clone()));",
            "worker sidecar dispatch must install the existing Arc without reconstruction",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effect(",
            "Arc::clone(&message),",
            "Arc::new((*message).clone()),",
            "runner sidecar dispatch must preserve the exact peer, complete route set, and immutable message pointer",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "V2_MERGE_SIDECAR_OUTBOUND_SESSIONS_PER_SOURCE",
            "nonzero!(2_usize)",
            "nonzero!(3_usize)",
            "certified sidecar per-source sessions must remain exactly two",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "V2_MERGE_SIDECAR_OUTBOUND_BYTES_PER_SOURCE",
            "nonzero!(16_usize * 1024 * 1024)",
            "nonzero!(17_usize * 1024 * 1024)",
            "certified sidecar per-source bytes must remain exactly 16 MiB",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "V2_MERGE_SIDECAR_SERVER_REQUEST_GATES_PER_SOURCE",
            "nonzero!(4_usize)",
            "nonzero!(5_usize)",
            "certified sidecar per-source request gates must remain exactly four",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn from_admitted_reply(",
            "semantic_target: flush_identity.semantic_target().clone(),",
            "semantic_target: chunk.responder.clone(),",
            "sidecar writer-flush admission must bind the opaque source, exact route, actor ticket and clone-shared claim with immutable payload and cursors",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn poll_reply_flushes(",
            "check_production_reliable_flush_worker_transition(flush_trace)\n"
            "                            .ok_or_else",
            "Some(flush_trace)\n"
            "                            .ok_or_else",
            "writer-flush ownership must consume the checked transition token before removing its target-local witness",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn poll_reply_flushes(",
            ".bind_confirmed_worker_trace(flush_trace)",
            ".bind_confirmed_worker_trace(ProductionReliableFlushTraceProjection::default())",
            "a successful writer occurrence must bind its exact confirmed worker trace before lane admission",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn poll_reply_flushes(",
            "if let Some(admission) = pending_flush.sidecar_admission.take() {\n"
            "                        self.admitted_sidecar_chunks.push_back(admission);\n"
            "                    }",
            "let _ = pending_flush.sidecar_admission.take();",
            "only a successful peer-writer flush may create a sidecar cursor receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn poll_reply_flushes(",
            "let route_state = self\n"
            "                        .fanouts",
            "if let Some(admission) = pending_flush.sidecar_admission.take() {\n"
            "                        self.admitted_sidecar_chunks.push_back(admission);\n"
            "                    }\n"
            "                    let route_state = self\n"
            "                        .fanouts",
            "closed writer ownership must not manufacture a sidecar cursor receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn project_sidecar_receipt_completions(",
            "admission.matches_materialized_chunk(message) && admission.is_bound_to_source(route)",
            "admission.matches_materialized_chunk(message)",
            "retained sidecar flush completion must match the immutable chunk and exact authenticated source before advancing only that route",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "CertifiedMergeSidecarMessage::Request(_)\n"
            "                    | CertifiedMergeSidecarMessage::Close(_)\n"
            "                    | CertifiedMergeSidecarMessage::CloseAck(_)\n"
            "                    | CertifiedMergeSidecarMessage::GenerationHint(_) => None,",
            "CertifiedMergeSidecarMessage::Request(_) => Some((\n"
            "                        post.clone(),\n"
            "                        reply_route.clone(),\n"
            "                        message_cursor_before,\n"
            "                        message_cursor_after,\n"
            "                    )),\n"
            "                    CertifiedMergeSidecarMessage::Close(_)\n"
            "                    | CertifiedMergeSidecarMessage::CloseAck(_)\n"
            "                    | CertifiedMergeSidecarMessage::GenerationHint(_) => None,",
            "only an immutable certified response chunk may create a writer-flush receipt from its exact route",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "self.sidecar_control_units() >= self.sidecar_admission_capacity",
            "self.sidecar_control_units() > self.sidecar_admission_capacity",
            "sidecar receipt capacity must reject at the exact full boundary",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn admit_network_exact_output(",
            ".post_reply_recoverable_with_flush_ack_at_attempt(\n"
            "                        post,\n"
            "                        reply_route,\n"
            "                        ticket,\n"
            "                        reply_writer_timeout_attempt,\n"
            "                    )?",
            ".post_reply_recoverable(post, reply_route, ticket)\n"
            "                .map(|()| None)?",
            "production reply output must retain every exact writer-flush witness",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn admit_network_exact_output(",
            "Ok(ExactOutputAttemptOutcome::SidecarFlush(flush_ack))",
            "{ drop(flush_ack); Ok(ExactOutputAttemptOutcome::Admitted) }",
            "production reply output must retain every exact writer-flush witness",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn drain_certified_merge_sidecar_chunk_admissions(",
            "limit.min(pending.admitted_sidecar_chunks.len())",
            "limit.min(pending.flushing_sidecar_chunks.len())",
            "receipt drainage may consume only successfully flushed sidecar admissions",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "check_production_reliable_flush_link_transition(worker_trace, occurrence).ok_or(",
            "check_production_reliable_flush_link_transition(worker_trace, worker_trace).ok_or(",
            "lane application must consume checked worker/link tokens for the exact accepted occurrence before inspecting mutable transport state",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "if !admission.projection_matches_identity(&admission.flush_identity) {",
            "if false {",
            "lane application must consume checked worker/link tokens for the exact accepted occurrence before inspecting mutable transport state",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "|| projection.chunk_cursor_before != chunk_index",
            "|| false",
            "lane application must validate the immutable message and chunk cursors before transport preflight",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "preflight_reliable_flush_outbound(self, admission, &gate, chunk_index, count)?",
            "preflight_reliable_flush_outbound(self, admission, &gate, 0, count)?",
            "the exact gate, source route, shared bytes and cursors must preflight into one immutable application plan before claiming completion",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct ServerRequestGateAttempt {",
            "cursor: ServerResponseCursor,",
            "cursor: usize,",
            "sidecar request gates must retain exact materialization authority, retry state, and a source-local pending-or-terminal cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "enum ServerResponseCursor {",
            "Complete,",
            "PendingZero,",
            "sidecar gate history must preserve terminal completion across exact, later-delivery, and reconnected observations",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn prune_server_gates(",
            "let reclaimed = self.reclaim_inactive_outbound_attempts(now)?;",
            "let reclaimed = 0;",
            "sidecar gate pruning must preserve semantic ownership until an authenticated close floor retires it",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn route_update(",
            ".source_update_from(prior)",
            ".source_update_from(candidate)",
            "same-source sidecar route update must use the canonical monotonic update kernel",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "if prior.cursor == ServerResponseCursor::Complete {",
            "if false && prior.cursor == ServerResponseCursor::Complete {",
            "an exact, later-delivery, or reconnected completed source must remain terminal while only its observed route may update",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            "let ServerResponseCursor::Pending(resume_chunk) = attempt.cursor else {\n"
            "                continue;\n"
            "            };",
            "let resume_chunk = match attempt.cursor {\n"
            "                ServerResponseCursor::Pending(chunk) => chunk,\n"
            "                ServerResponseCursor::Complete => 0,\n"
            "            };",
            "completed sidecar sources must never regain materialized output",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn drain_outbound_chunks_inner(",
            "cursor = ServerResponseCursor::Complete;",
            "cursor = ServerResponseCursor::Pending(0);",
            "sidecar drainage must persist terminal completion rather than a replayable chunk-zero cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "if !admission.flush_identity.claim_writer_flush_once() {",
            "if false {",
            "the clone-shared writer claim must be the sole linearization point before the exact checked application is compared and durably published",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "check_production_reliable_flush_application_transition(prospective_application)",
            "check_production_reliable_flush_worker_transition(worker_trace)",
            "the clone-shared writer claim must remain behind the checked application/link gates and opaque-token consumption",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "check_production_reliable_flush_link_transition(worker_trace, prospective_application)",
            "check_production_reliable_flush_link_transition(worker_trace, occurrence)",
            "the clone-shared writer claim must remain behind the checked application/link gates and opaque-token consumption",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "let prospective_application = checked_application.into_projection();",
            "let prospective_application = prospective_application;",
            "the clone-shared writer claim must remain behind the checked application/link gates and opaque-token consumption",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "attempt.materialization_retryable = false;\n"
            "                    return Ok(ServerRequestAdmission::Existing);",
            "attempt.materialization_retryable = true;\n"
            "                    return Ok(ServerRequestAdmission::Existing);",
            "an exact, later-delivery, or reconnected completed source must remain terminal while only its observed route may update",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "let retry_chunk = attempt.in_flight_chunk.unwrap_or(attempt.next_chunk);",
            "let retry_chunk = 0;",
            "a replacement writer tenure must retry the source's current chunk",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "if attempt.in_flight_chunk.is_none() && !attempt.queued {",
            "if !attempt.queued {",
            "a later delivery with an in-flight chunk must refresh only its source route without queueing a concurrent copy",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "attempt.cursor = ServerResponseCursor::Pending(retry_chunk);",
            "attempt.cursor = ServerResponseCursor::Pending(0);",
            "an observed source update must never reset a retained sidecar cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            "candidate.same_delivery(admitted)",
            "candidate.same_tenure(admitted)",
            "materialization must consume the exact admitted delivery capability",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            "let mut remaining_global_sessions = self\n"
            "            .outbound_session_capacity\n"
            "            .saturating_sub(self.outbound_attempt_count());",
            "let mut remaining_global_sessions = usize::MAX;",
            "sidecar materialization must preflight global and per-source session and byte bounds",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            "self.source_outbound_count(source) >= self.limits.outbound_sessions_per_source",
            "self.source_outbound_count(source) > self.limits.outbound_sessions_per_source",
            "sidecar materialization must preflight global and per-source session and byte bounds",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            ".saturating_add(response_len)\n"
            "                    > self.limits.outbound_bytes_per_source",
            ".saturating_add(response_len)\n                    > usize::MAX",
            "sidecar materialization must preflight global and per-source session and byte bounds",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if admitted_attempts.is_empty()",
            "> self.outbound_byte_capacity",
            "> usize::MAX",
            "sidecar materialization must preflight global and per-source session and byte bounds",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if remaining_global_sessions == 0",
            "capacity_rejected_attempts.push(source.clone());",
            "return Err(MergeSidecarError::Capacity(\"outbound response budget\"));",
            "one saturated sidecar source must not erase independently admissible same-request sources",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if !Self::alternate_source_is_authorized",
            "next_chunk: 0,",
            "next_chunk: prior.resume_chunk,",
            "a newly observed alternate sidecar source must begin at chunk zero",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn drain_outbound_chunks_inner(",
            "attempt.in_flight_chunk = Some(index);",
            "attempt.next_chunk = index.saturating_add(1);",
            "preserve the exact source route, and mark only an in-flight cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "!existing.request.same_occurrence_except_close_floor(request)",
            "false && !existing.request.same_occurrence_except_close_floor(request)",
            "duplicate sidecar admission must preserve canonical occurrence identity",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn alternate_source_is_authorized(",
            "match candidate {",
            "return true; match candidate {",
            "alternate sidecar sources must retain live route authority without treating recovered peer ownership as a process-local capability",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "if &request.requester != sender || &request.responder != local_peer {",
            "if false && (&request.requester != sender || &request.responder != local_peer) {",
            "sidecar request admission must bind authenticated sender, responder, semantic target, and active route",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn cancel_unmaterialized_server_request(",
            "Self::release_authorized_server_request_attempts(gate);",
            "let _ = gate;",
            "failed sidecar materialization must preserve route/cursor history",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "if !prior.materialization_retryable {",
            "if false && !prior.materialization_retryable {",
            "an exact failed-materialization retry must preserve only its source-local retryability and re-enter durable fair selection",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn release_unsent_request(",
            "let attempt = assembly\n"
            "            .current\n"
            "            .take()",
            "return; let attempt = assembly\n            .current\n            .take()",
            "an unsent sidecar request must restore the exact holder cursor, close its durable sequence, and persist before retry",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn acknowledge_certified_merge_sidecar_chunk_admission(",
            "if acknowledged {",
            "if true {",
            "lane work may schedule the next chunk only after the exact receipt and next pending writer identity are durable",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn acknowledge_certified_merge_sidecar_chunk_admission(",
            "operation.complete();",
            "drop(operation);",
            "lane sidecar ACK application may complete only after every successor post is retained",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn effect_count(",
            "self.effects\n"
            "            .len()\n"
            "            .saturating_add(self.sidecar_effects.len())",
            "0",
            "lane scan rank must count both ordinary and source-owned sidecar effects",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn requeue_effect(",
            "match effect {",
            "drop(effect); return true; match effect {",
            "lane requeue must return the exact unserviceable occurrence to its bounded owner lane",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn push_merge_sidecar_post(",
            "let MergeSidecarPost {\n"
            "            peer,\n"
            "            reply_route,\n"
            "            message,\n"
            "        } = post;",
            "let MergeSidecarPost {\n"
            "            peer: _,\n"
            "            reply_route,\n"
            "            message,\n"
            "        } = post;\n"
            "        let peer = self.local_peer.clone();",
            "lane sidecar post conversion must preserve the exact peer and message while stripping only GenerationHint reply-route ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn prune_finalized_merge_sidecars(",
            ".map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;",
            ".ok();",
            "finalized sidecar pruning must remain fail-stop and Kura-bound",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn is_pending(",
            "fanout.has_dispatchable_target()",
            "false",
            "pending exact output must include dispatchable fanouts, writer flushes, and undrained receipts without spinning on parked ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn handoff_applied_height_to_durable_reconstruction(",
            "self.admitted_sidecar_chunks.clear();",
            "let _ = &self.admitted_sidecar_chunks;",
            "applied-height handoff must retire every volatile sidecar completion state",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn retain_returned(",
            "if HashOf::new(&post.data) != *expected_hash {",
            "if false && HashOf::new(&post.data) != *expected_hash {",
            "returned actor post must retain the exact pinned payload identity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn handoff_applied_height_to_durable_reconstruction(",
            ".any(|(message, expected_hash)| HashOf::new(message) != *expected_hash)",
            ".any(|(message, expected_hash)| false && HashOf::new(message) != *expected_hash)",
            "applied-height handoff must preflight every pinned payload before classification",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "blocked_sources.insert(attempted_source);",
            "let _ = attempted_source;",
            "exact-output drive_with_budget_ack declaration and complete control flow",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn apply_reply_route_update(",
            "self.current = None;",
            "self.message_index = 0; self.current = None;",
            "a same-source reconnect must not reset its retained exact-output cursor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn coalesce_reservation_additions_for_plan(",
            "ReplyTargetMerge::Update { .. } => 0,",
            "ReplyTargetMerge::Update { .. } => full_mask,",
            "ordinary same-source updates retain reservation ownership while only a new source charges the candidate cursor suffix",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn preview_coalesce_plan(",
            "target.2 = false;",
            "target.1 = 0; target.2 = false;",
            "the coalesce preview must preserve the retained message cursor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn outstanding_sources_excluding(",
            "for (target_index, target) in self.targets.iter().enumerate() {",
            "for (target_index, target) in self.targets.iter().enumerate().filter(|(_, target)| !target.parked) {",
            "parked attempts must retain every outstanding source/FIFO class",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn outstanding_reservation_counts(",
            "for (target_index, target) in self.targets.iter().enumerate() {",
            "for (target_index, target) in self.targets.iter().enumerate().filter(|(_, target)| !target.parked) {",
            "parked attempts must retain their reservation ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan(&self, candidate: &Self)",
            "self.reply_target_merge_plan_with_hooks(candidate, |_| {}, || {})",
            "self.reply_target_merge_plan_after_candidate_prune(candidate, |_| {})",
            "the no-hook production coalescing wrapper must delegate to the receipt-bound route-history kernel",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            ".project_retained_reply_routes(prune_receipt)",
            ".project_retained_reply_routes(prune_receipt.clone())",
            "candidate pruning must retain its ownership receipt while strict or explicitly authorized superseded history produces a typed merge receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            ".merge_with_receipt(&candidate_routes)",
            ".merge(&candidate_routes)",
            "candidate pruning must retain its ownership receipt while strict or explicitly authorized superseded history produces a typed merge receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            ".any(|route| route.same_delivery(candidate_route))",
            ".any(|route| route.same_source(candidate_route))",
            "the authoritative merged route snapshot must reuse the immutable joint tenure/delivery freshness kernel",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "                    update,\n                });",
            "                    update: NetworkReplyRouteSourceUpdate::Exact,\n"
            "                });",
            "same-source coalescing must preserve the retained source cursor while updating only its authenticated route capability",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn commit_coalesce_plan(",
            "message_index: candidate_target.message_index,\n"
            "                        reply_writer_timeout_attempt: candidate_target.reply_writer_timeout_attempt,\n"
            "                        current: None,",
            "message_index: 0,\n"
            "                        reply_writer_timeout_attempt: candidate_target.reply_writer_timeout_attempt,\n"
            "                        current: None,",
            "an appended source must preserve its candidate cursor and parked state while starting without actor-post, admission-ticket, or writer-flush ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn capacity_available_for(",
            "pending.coalesce_reservation_additions_for_plan(fanout, &plan.targets)?",
            "fanout.admission_reservation_counts()?",
            "capacity preflight must enforce route-source geometry before charging only newly appended source ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn coalesced_target_geometry_available(",
            "&& target_count <= plan.reply_routes.source_capacity()",
            "&& true",
            "coalesced reply attempts must fit both the configured fanout bound and the actor-derived source-capacity geometry",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            ".source_update_from_snapshot(prior_route)",
            ".source_update_from(prior_route)",
            "the authoritative merged route snapshot must reuse the immutable joint tenure/delivery freshness kernel",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn classified_with_reply_routes(",
            "Self::classified_with_route_history(messages, peers, routes, Some(reply_routes))",
            "Self::classified_with_route_history(messages, peers, routes, None)",
            "reply fanout construction must preserve the complete bounded route history",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn classified_with_route_history(",
            "            reply_routes,\n"
            "            ingress_ownership: None,\n"
            "            current_source_targets: BTreeMap::new(),",
            "            reply_routes: None,\n"
            "            ingress_ownership: None,\n"
            "            current_source_targets: BTreeMap::new(),",
            "fanout construction must store the complete authoritative live-and-tombstone reply history",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "let retained_routes = self.reply_routes.clone().ok_or_else",
            "let retained_routes = candidate.reply_routes.clone().ok_or_else",
            "candidate pruning must retain its ownership receipt while strict or explicitly authorized superseded history produces a typed merge receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "plan.push(ReplyTargetMerge::Update {",
            "plan.push(ReplyTargetMerge::Reactivate {",
            "no coalescing path may reset a retained terminal reply cursor from a newly materialized candidate",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "if plan.targets.is_empty()",
            ".commit_coalesce_plan(&fanout, &plan, preview.current_source_targets);",
            ".coalesce_retry(&fanout)?;",
            "a route-history-only update must atomically commit its previewed cursor and FIFO ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "if plan.targets.is_empty()",
            "self.source_fifo_owners = next_source_fifo_owners;",
            "let _ = next_source_fifo_owners;",
            "a route-history-only update must atomically commit its previewed cursor and FIFO ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn commit_coalesce_plan(",
            "self.reply_routes = Some(plan.reply_routes.clone());",
            "self.reply_routes = None;",
            "atomic coalesce commit must install the complete route and fair-ingress histories",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn exact_target_geometry(",
            "Some(reply_routes.clone()),",
            "None,",
            "lane preflight expands every authenticated source into an independent exact target",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn advance_after_attempt(",
            "self.unregister_source_fifo_owner(fifo_id, source)?;",
            "let _ = (fifo_id, source);",
            "admission advances only the completed class/source ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn start_inner(",
            ".map(|entry| entry.validator.clone())",
            ".filter(|_| false).map(|entry| entry.validator.clone())",
            "production bounds protocol fanout by roster and source geometry while charging the shared pool only for the independently reserved authenticated reply sources",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn enqueue_owned_exact_reply_routes_while_guarded(",
            "PendingExactFanout::claimed_with_reply_routes_and_ingress_ownership(",
            "PendingExactFanout::claimed_with_routes(",
            "exact replies expand all authenticated sources without changing semantic identity and preserve bounded route",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn applied_height_reconstruction_covers(",
            "rollover_claim.validate_fanout(messages, peers)?;",
            "let _ = (messages, peers);",
            "durable rollover requires a validated typed claim in the exact creation scope",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn applied_height_reconstruction_covers(",
            "if !scope.covers(artifact) {",
            "if false && !scope.covers(artifact) {",
            "durable rollover requires a validated typed claim in the exact creation scope",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn enqueue_exact_fanout_while_guarded(",
            "PendingExactFanout::claimed(messages, peers, rollover_claim)?",
            "PendingExactFanout::claimed(messages, peers, ExactOutputRolloverClaim::Exact)?",
            "every production exact fanout must enter the corridor with its typed claim",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn applied_height_reconstruction_covers(",
            "durable_history.ok_or_else(|| {",
            "Some(durable_history.unwrap()).ok_or_else(|| {",
            "applied-height handoff must independently reread every durable Kura response source",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn durable_history_source_covers(",
            "|| response.certificate != source.commit_qc",
            "|| false",
            "durable CommitQC response must match its exact Kura finality source",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn durable_history_source_covers(",
            "|| canonical_wire != response.body",
            "|| false",
            "durable body response must match its exact canonical Kura block",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn durable_history_source_covers(",
            "|| certificate.commit_qc != source.commit_qc",
            "|| false",
            "durable lane certificate must match its exact certified Kura source",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn post_durable_history_response_with_routes(",
            "durable_history_source_covers(",
            "durable_history_source_covers_unchecked(",
            "global historical response must validate Kura before exact-output admission",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn post_durable_lane_certificate_with_routes(",
            "durable_history_source_covers(",
            "durable_history_source_covers_unchecked(",
            "historical lane response must validate Kura before exact-output admission",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn handoff_applied_height_output_to_durable_reconstruction(",
            "Some(self.kura.as_ref()),",
            "None,",
            "production handoff must pass exact lane and Kura authorities into retirement",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn applied_height_reconstruction_covers(",
            "round.context_id == context_id && round.height == height",
            "round.context_id == context_id",
            "durable rollover classification must bind the exact artifact context and height",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn applied_height_reconstruction_covers(",
            "ProgressReconstruction::Retransmit",
            "ProgressReconstruction::Exact",
            "exact-output applied_height_reconstruction_covers declaration",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn validate_applied_height_output_handoff_authority(",
            "|| receipt.artifact_hash() != HashOf::new(artifact)",
            "|| false",
            "applied-height handoff requires the exact Kura receipt and finality artifact",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn covered_source_hash(",
            "self.finality_artifact_hash != HashOf::new(finality_artifact)",
            "false && self.finality_artifact_hash != HashOf::new(finality_artifact)",
            "lane rollover authority must bind the exact finality artifact and consult its proposal-keyed durable source before height supersession",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn covered_source_hash(",
            "self.durable_sessions.get(&proposal_hash)",
            "self.durable_sessions.values().next()",
            "lane rollover authority must bind the exact finality artifact and consult its proposal-keyed durable source before height supersession",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn covered_source_hash(",
            "validate_superseded_lane_output(message)?;",
            "let _ = message;",
            "non-winning lane output must be validated before artifact-bound supersession",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn persistent(",
            "application_receipt_hash.as_ref(),",
            "durable_artifact_hash.as_ref(),",
            "lane durable source must commit finality, certificate, and application receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn durable_lane_rollover_authority(",
            "block.header().height().get() != finality_artifact.height\n"
            "            || block.hash() != finality_artifact.block_hash",
            "false",
            "lane authority builder must derive its bounded ordinary and autonomous "
            "winner set from the exact canonical block",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn durable_lane_rollover_authority(",
            "|| receipt.application_block_hash != finality_artifact.block_hash",
            "|| false",
            "lane authority builder must bind every winner to the exact applied artifact",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn reconstruct_durable_lane_certificate(",
            "self.kura.read_certified_lane_block_artifact(",
            "self.kura.read_certified_lane_block_artifact_unchecked(",
            "lane recovery reconstruction must begin from the exact certified Kura artifact",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn serve_durable_lane_certificate(",
            "reply_routes: Some(reply_routes),",
            "reply_routes: None,",
            "lane recovery emitter retains every authenticated source route",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn merge_optional_reply_routes(",
            "let mut merged = retained.clone();",
            "let mut merged = candidate.clone();",
            "lane effect coalescence atomically commits canonical history maintenance and reports success only for a retained live candidate delivery",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn merge_optional_reply_routes(",
            "merged.merge_observed_with_receipt(candidate)",
            "merged.merge(candidate)",
            "lane effect coalescence atomically commits canonical history maintenance and reports success only for a retained live candidate delivery",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn merge_lane_work_effect_reply_routes_after_route_merge<AfterRouteMerge>(",
            "if !lane_work_effect_reply_routes_have_valid_shape(candidate) {",
            "if !lane_work_effect_reply_routes_are_valid(candidate) {",
            "inactive duplicates still reach maintenance",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn lane_work_effect_key(",
            "encoded.push(4);",
            "encoded.push(0);",
            "durable lane response effect identity must include its distinct tag, peer, and certificate",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn drain_v2_ingress(",
            "services.post_durable_history_response_on_reply_routes_with_permit(",
            "services.post_durable_history_response_with_permit(",
            "historical global responses preserve the complete prevalidated route set",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effect(",
            ".post_durable_lane_certificate_on_reply_routes(\n"
            "                    peer,\n"
            "                    reply_routes,\n"
            "                    ingress_ownership,\n"
            "                    certificate,\n"
            "                )",
            ".post_lane_block(peer, BlockMessage::LaneBlockCertificate(Box::new(certificate)))",
            "historical lane dispatch preserves every authenticated source route",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effects(",
            "let scan_limit = lane_work.effect_count();",
            "let scan_limit = limit.max(1);",
            "lane scheduler must scan past unserviceable heads without losing ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effects(",
            "continue;",
            "break;",
            "lane scheduler must scan past unserviceable heads without losing ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effects(",
            "apply_certified_merge_sidecar_chunk_admissions(lane_work, services, limit)?;",
            "let _ = (lane_work, services, limit);",
            "runner lane dispatch must apply writer receipts before selecting owned work",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "match dispatch_lane_work_effect(services, next_effect)? {",
            "apply_certified_merge_sidecar_chunk_admissions(lane_work, services, limit)?;",
            "let _ = (lane_work, services, limit);",
            "runner lane dispatch must apply writer receipts after complete and source-retained exact handoffs",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/finalized_output_rollover.rs",
            "fn rollover_finalized_height_outputs(",
            "let _ = retry_exact_output_and_apply_sidecar_admissions(\n"
            "        &mut lane_work,\n"
            "        services,\n"
            "        control_queue_capacity,\n"
            "    )?;",
            "let _ = services.retry_pending_exact_output();",
            "durable finalization must retry and apply sidecar receipts before reconstructing rollover authority",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn persist_anchored_sessions(",
            "self.hydrate_canonical_lane_artifacts()?;",
            "let _ = &self.lane_sessions;",
            "late canonical lane hydration must precede committed-session collection",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn new_with_output_guard_and_transport_inner(",
            "adapter.ensure_globally_applied_lane_receipts_durable()?;\n"
            "        construction.complete();",
            "adapter.ensure_globally_applied_lane_receipts_durable()?;\n"
            "        adapter.hydrate_canonical_lane_artifacts()?;\n"
            "        construction.complete();",
            "production constructor must remain carrier-silent before exact Queue installation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn activate_after_lane_drain_queue_install(",
            "self.revalidate_hydrated_autonomous_queue_owners(installed_queue.as_ref())?;",
            "let _ = installed_queue;",
            "one-shot startup activation must authenticate the installed Queue",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn claim_runner_lifecycle_process_generation(",
            "NodeRole::Validator => kura\n"
            "            .claim_autonomous_lifecycle_process_generation(",
            "NodeRole::Validator => kura\n"
            "            .read_autonomous_lifecycle_process_generation(",
            "the configured-role process generation helper must durably claim validator ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn claim_runner_lifecycle_process_generation(",
            "match role {",
            "if local_validator_index(context, local_peer, role)?.is_none() {\n"
            "        return Ok(None);\n"
            "    }\n"
            "    match role {",
            "the configured-role process generation helper must not consult height-local roster membership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn run_inner(",
            "lane_work.activate_after_lane_drain_queue_install(&queue)?;",
            "let _ = &queue;",
            "runner startup must install the exact Queue before the one-shot carrier activation seam",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn hydrate_canonical_lane_artifacts(",
            "let _ = self\n"
            "                .lane_sessions\n"
            "                .insert_recovered_proposal_replacing_uncommitted_conflict(proposal);",
            "let _ = proposal;",
            "late canonical lane hydration must retain the exact proposal as bounded recovery work",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/finalized_output_rollover.rs",
            "fn rollover_finalized_height_outputs(",
            "lane_work.persist_anchored_sessions()?;",
            "let _ = &lane_work;",
            "finalized output rollover must durably reconstruct every predecessor owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/finalized_output_rollover.rs",
            "fn rollover_finalized_height_outputs(",
            ".durable_lane_rollover_authority(artifact)?",
            "None",
            "finalized output rollover must durably reconstruct every predecessor owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/finalized_output_rollover.rs",
            "fn rollover_finalized_height_outputs(",
            "lane_work.retain_successor_owned_rollover_effects(artifact, &durable_lane_authority)?;",
            "lane_work.retain_successor_owned_rollover_effects(artifact, &durable_lane_authority.clone())?;",
            "finalized output rollover must durably reconstruct every predecessor owner",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn with_limits_and_server_stream_capacity(",
            "reply_source_capacity,\n"
            "            server_roster_digest,\n"
            "            server_stream_capacity,\n"
            "            outbound_session_capacity,",
            "reply_source_capacity,\n"
            "            server_roster_digest,\n"
            "            server_stream_capacity,\n"
            "            outbound_session_capacity: 0,",
            "sidecar source geometry must reject zero and install every checked corridor bound",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn with_limits_and_server_stream_capacity(",
            "tick_close_next: false,",
            "tick_close_next: true,",
            "sidecar request/close fairness must service an initial progress-bearing request before alternating closes",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn tick_bounded(",
            "if self.timeout_retry_close_deferred {",
            "if false {",
            "a newly timed-out sidecar fetch may preempt administrative closure only once before retained Close debt wins the bounded service slot",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn begin_request_or_close(",
            "self.timeout_retry_close_deferred = false;",
            "let _ = &self.timeout_retry_close_deferred;",
            "servicing a sidecar Close must discharge retained timeout-preemption debt",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn release_authorized_server_request_attempts(",
            "matches!(attempt.cursor, ServerResponseCursor::Pending(_));",
            "false;",
            "transiently rejected response work must release materialization authority",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn park_authorized_server_request_attempts(",
            "for attempt in gate\n            .attempts",
            "return; for attempt in gate\n            .attempts",
            "parking a materialized response must consume retryability while retaining each source route and resume cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if admitted_attempts.is_empty() && capacity_rejected_attempts.is_empty()",
            "Self::park_authorized_server_request_attempts(gate, now);",
            "let _ = (gate, now);",
            "completed-race and admitted materialized response work must pass through terminal parking",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            "attempt.materialization_retryable =\n"
            "                matches!(attempt.cursor, ServerResponseCursor::Pending(_));",
            "attempt.materialization_retryable = false;",
            "partial materialization must keep capacity-partitioned pending sources retryable after shared bytes retire",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn reclaim_inactive_outbound_attempts(",
            "gate_attempt.cursor = ServerResponseCursor::Pending(resume_chunk);",
            "let _ = resume_chunk;",
            "inactive sidecar parking must remain pending at the exact unacknowledged source cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn reclaim_inactive_outbound_attempts(",
            "self.persist_lifecycle_projection(projected)?;",
            "drop(projected);",
            "inactive sidecar reclamation must publish every projected durable cursor before removing ephemeral writers or shared bytes",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn with_limits_and_server_stream_capacity(",
            ".checked_mul(limits.outbound_sessions_per_source)",
            ".saturating_mul(limits.outbound_sessions_per_source)",
            "sidecar global capacity must be checked from the configured authenticated-source geometry",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn derive_server_request_capacities(",
            ".checked_mul(reply_source_capacity)",
            ".saturating_mul(reply_source_capacity)",
            "sidecar responder gates and per-source attempts must use checked products of the immutable roster and authenticated-source geometry",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn next_server_request_materialization(",
            "self.persist_lifecycle_projection(projected)?;",
            "drop(projected);",
            "fair sidecar materialization must persist the selected requester cursor before granting any live terminating lookup authority",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "next_chunk: resume_chunk,",
            "next_chunk: 0,",
            "merge-sidecar source-isolated production seam",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn apply_reliable_flush_application(",
            "attempt.queued = true;\n            transport\n"
            "                .outbound_order\n"
            "                .push_back((plan.gate.key.clone(), plan.gate.source.clone()));",
            "let _ = &attempt.queued;",
            "merge-sidecar source-isolated production seam",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn drain_outbound_chunks_durable(",
            "self.persist_lifecycle_state()?;",
            "let _ = &lifecycle_changed;",
            "production sidecar drainage must durably publish every changed pending identity",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "#[cfg(test)]\n    fn drain_outbound_chunks(",
            "#[cfg(test)]",
            "#[cfg(any())]",
            "raw non-durable sidecar drainage must remain exactly #[cfg(test)]",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "const MAX_CERTIFIED_MERGE_SEMANTIC_PEERS: usize",
            "MAX_VALIDATORS_PER_HEIGHT;",
            "1;",
            "certified merge semantic request history must have exactly one top-level protocol-roster bound",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct MergeSidecarRuntimeGeometryV3",
            "semantic_peer_capacity: u64,",
            "semantic_peer_capacity: u32,",
            "durable sidecar geometry must advertise its validator-scoped semantic-peer bound",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn lifecycle_runtime_geometry_v3(",
            "semantic_peer_capacity: as_u64(MAX_CERTIFIED_MERGE_SEMANTIC_PEERS)?,",
            "semantic_peer_capacity: as_u64(self.reply_source_capacity)?,",
            "lifecycle geometry must fingerprint the validator-scoped semantic-peer bound independently of concurrent reply sources",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn advance_piggybacked_close_floor(",
            "!retained_request.same_occurrence_except_close_floor(request)",
            "retained_request.same_occurrence_except_close_floor(request)",
            "accept only the same immutable occurrence and reject regression",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn advance_piggybacked_close_floor(",
            "self.persist_lifecycle_projection(projected)?;",
            "drop(projected);",
            "publish the sole V2 projection before updating live cancellation, gate, or transfer state without rematerializing",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn restore_lifecycle_snapshot(",
            "snapshot.request_streams.len() > MAX_CERTIFIED_MERGE_SEMANTIC_PEERS",
            "snapshot.request_streams.len() > self.reply_source_capacity",
            "lifecycle restoration must bound both semantic stream maps independently from concurrent authenticated-source gates",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn allocate_request_sequence(",
            "&& reclaim.is_none()",
            "&& reclaim.is_some()",
            "requester-side holder rotation must reclaim only quiescent roster-bounded streams",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn ensure_server_stream_slot(",
            "self.server_streams.len() < self.server_stream_capacity",
            "self.server_streams.len() < self.server_request_gate_capacity",
            "stream-slot helper must reject immutable-capacity exhaustion without independently rolling the generation",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn server_generation_is_terminal(",
            "&& self.server_request_gates.is_empty()",
            "|| self.server_request_gates.is_empty()",
            "every stream terminal and every gate, transfer, flush-order, and pending-closure owner empty",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn server_generation_is_terminal(",
            "&& self.pending_server_closures.is_empty()",
            "|| self.pending_server_closures.is_empty()",
            "every stream terminal and every gate, transfer, flush-order, and pending-closure owner empty",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn transition_server_service_generation(",
            "if !self.server_generation_is_terminal() {",
            "if false && !self.server_generation_is_terminal() {",
            (
                "ordinary responder generation rollover must occur only for a full terminal table",
                "ordinary responder generation transition must prepare without mutation, reject nonterminal state, and only then commit the prepared fence",
            ),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "self.ensure_server_stream_slot(sender)?;",
            "let _ = sender;",
            "full current-generation responder table rejects before pruning or mutating lifecycle state",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_close(",
            "if !self.server_streams.contains_key(sender) {\n"
            "            return Ok(close_ack());\n"
            "        }",
            "if !self.server_streams.contains_key(sender) {\n"
            "            let _ = close_ack();\n"
            "        }",
            "unknown current-generation Close must be acknowledged without allocating responder stream geometry",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn new(payload: MergeSidecarLifecyclePayloadV3)",
            "let payload_hash = HashOf::new(&payload);",
            "let payload_hash = HashOf::new(&());",
            "merge-sidecar crash-safe lifecycle production seam",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn integrity_is_valid(&self)",
            "self.payload_hash == HashOf::new(&self.payload)",
            "self.payload_hash != HashOf::new(&self.payload)",
            "the durable sidecar snapshot integrity check must bind the complete canonical payload",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn decode_snapshot(",
            "if !snapshot.integrity_is_valid() {",
            "if false && !snapshot.integrity_is_valid() {",
            "lifecycle recovery must accept only canonical integrity-bound V3 state",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn decode_snapshot(",
            "decode_from_bytes::<MergeSidecarLifecycleSnapshotV3>",
            "decode_from_bytes::<UnsupportedMergeSidecarLifecycleSnapshotV1>",
            "production lifecycle recovery must never decode the legacy V1 negative-test fixture",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn legacy_lifecycle_v1_snapshot_is_rejected_without_migration(",
            'error.contains("migration is not supported")',
            'error.contains("payload digest mismatch") '
            '/* error.contains("migration is not supported") */',
            "the retired-layout regression must require an explicit no-migration recovery failure",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn source_gate_count(",
            "retained.shares_budget_with(source)",
            "!retained.shares_budget_with(source)",
            "per-source sidecar gate accounting must share one stable authenticated-peer budget across every semantic origin",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn source_gate_count_after_close(",
            "&key.0 != sender",
            "&key.0 == sender",
            "close-aware per-source gate accounting must retain every gate from another semantic origin",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn restore_lifecycle_snapshot(",
            "BTreeMap::<ServerRequestBudgetSource, usize>::new()",
            "BTreeMap::<(PeerId, ServerRequestBudgetSource), usize>::new()",
            "durable recovery must aggregate gate ownership by stable authenticated source rather than by semantic requester/source pairs",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "self.source_gate_count(&source)",
            "self.server_request_gates.len()",
            "alternate-source sidecar admission must retain route-set, global-gate, and per-source-gate bounds",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn fifth_gate_from_one_hub_is_rejected_while_another_hub_progresses(",
            "for index in 0..MAX_SERVER_REQUEST_GATES_PER_SOURCE {",
            "for index in 0..1 {",
            "the exact source-cap regression must fill all four gates through one authenticated hub while varying semantic origins",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn restore_lifecycle_snapshot(",
            "if !snapshot.integrity_is_valid() {",
            "if false && !snapshot.integrity_is_valid() {",
            "lifecycle restoration must independently reject a stale typed payload digest before interpreting any semantic floor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn persist_next(",
            "snapshot.payload_hash = HashOf::new(&snapshot.payload);",
            "snapshot.payload_hash = HashOf::new(&());",
            "V3 lifecycle publication must recompute the typed payload digest before each state-slot publication",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn persist_next(",
            "Self::sync_directory(&self.directory)?;",
            "let _ = &self.directory;",
            "V3 lifecycle publication must sync the replaced state slot before publishing its root",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub enum CertifiedMergeSidecarMessage",
            "GenerationHint(CertifiedMergeSidecarGenerationHintV1),",
            "GenerationHint(CertifiedMergeSidecarCloseAckV1),",
            "the certified sidecar wire enum must expose request, close, close acknowledgement, generation fence, and chunk as distinct exhaustive variants",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) enum ServerRequestAdmission",
            "GenerationHint(MergeSidecarPost),",
            "GenerationHint,",
            "server request admission must explicitly distinguish materialization, existing ownership, and a stateless generation hint",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn canonical_hint_id(&self)",
            "self.observed_message_hash.as_ref(),",
            "&version,",
            "the generation hint identity must bind both generations, the exact observed message, and both authenticated peers",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub struct CertifiedMergeSidecarSemanticSequenceV1",
            "pub struct CertifiedMergeSidecarSemanticSequenceV1(pub NonZeroU64);",
            "pub struct CertifiedMergeSidecarSemanticSequenceV1(pub u64);",
            "every exact semantic occurrence coordinate must use a nonzero typed wire value",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub struct CertifiedMergeSidecarRequestV1",
            "pub semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,",
            "pub semantic_sequence: u64,",
            "Request wire occurrence must carry typed nonzero generation, epoch, and semantic sequence",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub struct CertifiedMergeSidecarChunkV1",
            "pub semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,",
            "pub semantic_sequence: u64,",
            "Chunk wire occurrence must copy the typed nonzero generation, epoch, and semantic sequence",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct ServerPendingChunkIdentity",
            "semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,",
            "semantic_sequence: u64,",
            "process-local pending flush identity must retain the typed nonzero request occurrence",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct ServerPendingChunkLifecycleV3",
            "semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,",
            "semantic_sequence: u64,",
            "the durable pending marker must bind the complete generation-scoped request, response, payload, and chunk identity",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct ServerRequestGate",
            "semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,",
            "semantic_sequence: u64,",
            "live responder gate must retain the full canonical request and every generation-scoped occurrence coordinate",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct ServerRequestGateLifecycleV3",
            "semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,",
            "semantic_sequence: u64,",
            "each durable responder gate must retain the full canonical request and every generation, epoch, sequence, source, and pending-marker coordinate",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "struct CertifiedSidecarTransferIdentity",
            "semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,",
            "semantic_sequence: u64,",
            "worker exact-transfer identity must retain the typed nonzero sidecar occurrence",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct MergeSidecarLifecyclePayloadV3",
            "server_service_generation: CertifiedMergeSidecarServiceGenerationV1,",
            "server_service_generation: u64,",
            "the sole V3 durable sidecar snapshot must bind canonical runtime, root generation, and roster geometry",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn open(",
            "for legacy in LEGACY_LIFECYCLE_JOURNAL_DIRS {",
            "for legacy in [] {",
            "lifecycle startup must fail closed on every legacy directory before opening or creating sole V3 state",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn open(",
            "journal.publish_bootstrap_marker()?;",
            "let _ = &journal;",
            "lifecycle startup must durably publish the generation-zero root before creating the V3 state directory",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn open(",
            "let marker = journal.decode_root_high_water(&journal.root_high_water_path())?;",
            "let marker = MergeSidecarLifecycleRootHighWaterV3::bootstrap();",
            "existing V3 lifecycle state must be selected by the exact durable root high-water",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn decode_snapshot(",
            "snapshot.payload.version != LIFECYCLE_JOURNAL_VERSION_V3",
            "snapshot.payload.version == LIFECYCLE_JOURNAL_VERSION_V3",
            "lifecycle recovery must accept only canonical integrity-bound V3 state",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn restore_lifecycle_snapshot(",
            "gate.request.request_id != gate.request.canonical_request_id()",
            "false",
            "responder recovery must recompute the full canonical request and bind its generation, epoch, and semantic sequence to the durable gate",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn restore_lifecycle_snapshot(",
            "gate.attempts.len() > source_capacity.unwrap_or(1)",
            "false",
            "responder recovery must reject a gate whose durable attempts exceed its authenticated route-set capacity",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn restore_lifecycle_snapshot(",
            "if source_capacity.is_none() && peer == gate.requester",
            "if source_capacity.is_some() && peer == gate.requester",
            "responder recovery must reject synthetic/authenticated source-kind drift and synthetic requester impersonation",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn restore_lifecycle_snapshot(",
            "usize::try_from(pending.chunk_count).ok() != Some(expected_chunk_count)",
            "usize::try_from(pending.chunk_count).ok() != Some(index)",
            "responder recovery must reject any pending marker whose generation-scoped request metadata or exact chunk geometry differs from its gate",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_generation_hint(",
            "self.persist_lifecycle_projection(snapshot)?;",
            "drop(snapshot);",
            "requester generation replacement must persist the new generation and unique epoch before retiring any process-local old-generation attempt",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn commit_server_service_generation_transition(",
            "self.persist_lifecycle_projection(snapshot)?;",
            "drop(snapshot);",
            "publish the incremented generation and empty responder tables in the root-anchored V3 snapshot before mutating memory",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "request.service_generation > self.server_service_generation",
            "request.service_generation < self.server_service_generation",
            "a canonical future-generation request must reject atomically, while stale input or terminal full-table compaction returns an exact route-free hint before ordinary request-state mutation",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "request.service_generation < self.server_service_generation",
            "request.service_generation > self.server_service_generation",
            "a canonical future-generation request must reject atomically, while stale input or terminal full-table compaction returns an exact route-free hint before ordinary request-state mutation",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_close(",
            "close.service_generation > self.server_service_generation",
            "close.service_generation < self.server_service_generation",
            "a canonical future-generation Close must be rejected while a stale-generation Close returns a stateless exact hint before allocating or mutating a responder stream",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_close(",
            "close.service_generation < self.server_service_generation",
            "close.service_generation > self.server_service_generation",
            "a canonical future-generation Close must be rejected while a stale-generation Close returns a stateless exact hint before allocating or mutating a responder stream",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn sidecar_effect_slots(",
            ".filter(|effect| retryable_sidecar_server_control_peer(effect).is_none())",
            ".filter(|effect| retryable_sidecar_server_control_peer(effect).is_some())",
            "reproducible responder controls must not consume progress reservations while all physical sidecar effects remain relay bounded",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn next_sidecar_effect_selection(",
            ".position(|effect| retryable_sidecar_server_control_peer(effect).is_none())",
            ".position(|effect| retryable_sidecar_server_control_peer(effect).is_some())",
            "sidecar scheduling must prioritize progress while granting retryable responder control a bounded weighted turn",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "const SIDECAR_PROGRESS_DRAIN_WEIGHT: u8 = 3;",
            "const SIDECAR_PROGRESS_DRAIN_WEIGHT: u8 = 3;",
            "const SIDECAR_PROGRESS_DRAIN_WEIGHT: u8 = 0;",
            "the sidecar scheduler must give retryable responder control one bounded turn after exactly three progress-bearing drains",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn new_with_output_guard_and_transport_inner(",
            "MergeSidecarTransport::open_durable_with_server_stream_capacity(",
            "MergeSidecarTransport::open_durable(",
            "lane construction must derive the canonical responder roster and restore or open only its exact durable source and stream geometry",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn rehydrate_for_successor(",
            "self.successor_context_id != successor.id()",
            "self.successor_context_id == successor.id()",
            "retained sidecar ownership must bind the exact successor context and consume its durable output handoff before roster-aware rehydration",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn new_with_output_guard_and_transport_inner(",
            "sidecar_progress_drain_credit: SIDECAR_PROGRESS_DRAIN_WEIGHT,",
            "sidecar_progress_drain_credit: 0,",
            "lane construction must initialize the bounded sidecar progress/control drain credit",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn next_sidecar_effect_selection(",
            "if self.sidecar_progress_drain_credit == 0 {",
            "if self.sidecar_progress_drain_credit > 0 {",
            "sidecar scheduling must prioritize progress while granting retryable responder control a bounded weighted turn",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn retryable_sidecar_server_control_peer(",
            "CertifiedMergeSidecarMessage::GenerationHint(_) if reply_routes.is_some()",
            "CertifiedMergeSidecarMessage::GenerationHint(_) if reply_routes.is_none()",
            "lane retryable responder controls must retain exact reply ownership for CloseAck and GenerationHint during per-peer coalescing",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn retryable_sidecar_server_control_peer(",
            "CertifiedMergeSidecarMessage::CloseAck(_) if reply_routes.is_some()",
            "CertifiedMergeSidecarMessage::CloseAck(_) if reply_routes.is_none()",
            "lane retryable responder controls must retain exact reply ownership for CloseAck and GenerationHint during per-peer coalescing",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn push_merge_sidecar_post_or_restart(",
            "if retryable_server_control {",
            "if false && retryable_server_control {",
            "reserved sidecar handoff may nonfatally drop only reproducible responder control or an inactive response, and must roll back an unsent request before fail-stop",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn push_merge_sidecar_effect(",
            ".any(|queued| retryable_sidecar_server_control_peer(queued) == Some(peer))",
            ".any(|queued| retryable_sidecar_server_control_peer(queued).is_some())",
            "sidecar effect admission must preserve full identity and routes, coalesce retryable controls per peer inside the physical relay bound, and evict a retryable control before rejecting progress",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn push_merge_sidecar_effect(",
            "if self.sidecar_effects.len() >= self.limits.relay_capacity.get() {",
            "if self.sidecar_effects.len() > self.limits.relay_capacity.get() {",
            "sidecar effect admission must preserve full identity and routes, coalesce retryable controls per peer inside the physical relay bound, and evict a retryable control before rejecting progress",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn push_merge_sidecar_effect(",
            "if self.sidecar_effect_slots() == 0 {",
            "if false && self.sidecar_effect_slots() == 0 {",
            "sidecar effect admission must preserve full identity and routes, coalesce retryable controls per peer inside the physical relay bound, and evict a retryable control before rejecting progress",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn push_merge_sidecar_effect(",
            ".rposition(|queued| retryable_sidecar_server_control_peer(queued).is_some())",
            ".rposition(|queued| retryable_sidecar_server_control_peer(queued).is_none())",
            "sidecar effect admission must preserve full identity and routes, coalesce retryable controls per peer inside the physical relay bound, and evict a retryable control before rejecting progress",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn push_merge_sidecar_effect(",
            "self.sidecar_effect_keys\n"
            "                    .remove(&lane_work_effect_key(&evicted));",
            "let _ = evicted;",
            "sidecar effect admission must preserve full identity and routes, coalesce retryable controls per peer inside the physical relay bound, and evict a retryable control before rejecting progress",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn next_effect(",
            "self.next_sidecar_effect_selection()\n"
            "                .and_then(|(index, _)| self.sidecar_effects.get(index))\n"
            "                .cloned()",
            "self.sidecar_effects.front().cloned()",
            "lane effect peek must clone the exact weighted progress/control sidecar selection without consuming its credit",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn drain_effects(",
            "let (index, retryable_control) = self\n"
            "                    .next_sidecar_effect_selection()\n"
            "                    .expect(\"sidecar effect selected only when present\");",
            "let (index, retryable_control) = (0, false);",
            "lane effect drain must transfer the same weighted selection as peek, retire its key, and update progress/control credit only after ownership transfer",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn drain_effects(",
            "self.sidecar_progress_drain_credit = SIDECAR_PROGRESS_DRAIN_WEIGHT;",
            "self.sidecar_progress_drain_credit = 0;",
            "lane effect drain must transfer the same weighted selection as peek, retire its key, and update progress/control credit only after ownership transfer",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn drain_effects(",
            "self.sidecar_progress_drain_credit.saturating_sub(1);",
            "self.sidecar_progress_drain_credit.saturating_add(1);",
            "lane effect drain must transfer the same weighted selection as peek, retire its key, and update progress/control credit only after ownership transfer",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn accept_certified_merge_sidecar_request(",
            "return Ok(if self.push_merge_sidecar_post(post) {",
            "self.push_merge_sidecar_post_or_restart(post)?;\n"
            "            return Ok(if true {",
            "lane ingress must treat a stale-generation hint as bounded retryable output while persisting every materialize/existing gate before fair service",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn accept_certified_merge_sidecar_close(",
            "let _ = self.service_next_certified_merge_sidecar_materialization(now)?;",
            "let _ = now;",
            "an authenticated Close must expose its durable prefix, give fair pending materialization one turn, and preserve both progress and bounded retryable control outcomes",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn from_admitted_reply(",
            "reply_writer_timeout_attempt: flush_identity.reply_writer_timeout_attempt(),",
            "reply_writer_timeout_attempt: 0,",
            "sidecar writer-flush admission must bind the opaque source, exact route, actor ticket and clone-shared claim with immutable payload and cursors",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "match attempt(post, ticket, &route, reply_writer_timeout_attempt) {",
            "match attempt(post, ticket, &route, 0) {",
            "worker dispatch must pass the target-local adaptive timeout attempt into actor admission",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "|| flush_ack.identity().reply_writer_timeout_attempt()\n"
            "                            != reply_writer_timeout_attempt",
            "|| flush_ack.identity().reply_writer_timeout_attempt()\n"
            "                            != 0",
            "ordinary reply cursor must remain unchanged while retaining its exact admission and writer-flush witness",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "if flush_ack.identity().reply_writer_timeout_attempt()\n"
            "                        != reply_writer_timeout_attempt",
            "if flush_ack.identity().reply_writer_timeout_attempt()\n"
            "                        != 0",
            "sidecar cursor may advance only after retaining its exact admission and writer-flush witness",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn poll_reply_flushes(",
            "pending_flush.reply_writer_timeout_attempt != current_timeout_attempt",
            "pending_flush.reply_writer_timeout_attempt != 0",
            "terminal reply-flush polling must bind the mutable target, retained writer occurrence, and actor acknowledgement to one adaptive timeout attempt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn poll_reply_flushes(",
            "!= pending_flush.reply_writer_timeout_attempt",
            "!= current_timeout_attempt",
            "terminal reply-flush polling must bind the mutable target, retained writer occurrence, and actor acknowledgement to one adaptive timeout attempt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn handoff_applied_height_to_durable_reconstruction(",
            "!= target.reply_writer_timeout_attempt",
            "!= 0",
            "finality handoff must revalidate target, retained writer occurrence, and actor acknowledgement against the same adaptive timeout attempt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn handoff_applied_height_to_durable_reconstruction(",
            "!= pending_flush.reply_writer_timeout_attempt",
            "!= target.reply_writer_timeout_attempt",
            "finality handoff must revalidate target, retained writer occurrence, and actor acknowledgement against the same adaptive timeout attempt",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn covers(&self, other: &Self)",
            "self.requester == other.requester",
            "self.requester != other.requester",
            "sidecar close-prefix dominance must bind the requester",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_close(",
            "return Ok(false);",
            "return Err(MergeSidecarError::UnsolicitedResponse);",
            "a canonical duplicate CloseAck may be a bounded no-op",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_close(",
            "if ack.version != CERTIFIED_MERGE_SIDECAR_VERSION_V1 {",
            "if ack.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1 {",
            "merge-sidecar crash-safe lifecycle production seam MergeSidecarTransport::acknowledge_close declaration and complete control flow must match the exact reviewed token digest",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn drain_closed_server_prefixes(",
            "std::mem::take(&mut self.pending_server_closures)",
            "self.pending_server_closures.clone()",
            "merge transport must move every coalesced authenticated close prefix exactly once",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn accept_certified_merge_sidecar_request(",
            "let _ = self.apply_closed_server_prefixes();",
            "let _ = false;",
            "lane ingress must treat a stale-generation hint as bounded retryable output while persisting every materialize/existing gate before fair service",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn accept_certified_merge_sidecar_close(",
            "let close_progress = self.apply_closed_server_prefixes();",
            "let close_progress = false;",
            "an authenticated Close must expose its durable prefix",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn drain_closed_sidecar_prefixes(",
            "std::mem::take(&mut self.closed_sidecar_prefixes)",
            "self.closed_sidecar_prefixes.clone()",
            "lane work must move each dominant close prefix exactly once",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn close_certified_sidecar_prefix(",
            "&transfer.requester,",
            "&prefix.requester,",
            "worker close-prefix projection must bind the exact requester",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn close_certified_merge_sidecar_prefix(",
            "pending.close_certified_sidecar_prefix(prefix)",
            "Ok(0)",
            "production close-prefix bridge must serialize",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn apply_certified_merge_sidecar_closed_prefixes(",
            ".close_certified_merge_sidecar_prefix(prefix)",
            ".retry_pending_exact_output()",
            "runner must move every lane close prefix into the worker exact-output owner before later dispatch",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn service_next_certified_merge_sidecar_materialization(",
            ".next_server_request_materialization(now)",
            ".authorized_server_request_materialization()",
            "only the transport's durable fair materialization selection may cross into the terminating Kura lookup",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn retryable_certified_sidecar_responder_control_target(",
            "CertifiedMergeSidecarMessage::CloseAck(_)\n"
            "                | CertifiedMergeSidecarMessage::GenerationHint(_) => self",
            "CertifiedMergeSidecarMessage::CloseAck(_) => self",
            "worker retryable responder control must use exact reply ownership for CloseAck and GenerationHint",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn retryable_certified_sidecar_responder_control_target(",
            ".all(|route| matches!(&route.route, ExactTargetRoute::Reply(_))),",
            ".all(|route| matches!(&route.route, ExactTargetRoute::Topology)),",
            "worker retryable responder control must use exact reply ownership for CloseAck and GenerationHint",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn retains_retryable_sidecar_responder_control_for(",
            "retained.retryable_certified_sidecar_responder_control_target()\n"
            "                        == Some(candidate_target)",
            "retained.retryable_certified_sidecar_responder_control_target()\n"
            "                        .is_some()",
            "worker responder-control suppression must require an already-retained retryable control for the same semantic target",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn retains_retryable_sidecar_responder_control_for(",
            "== Some(candidate_target)",
            "!= Some(candidate_target)",
            "worker responder-control suppression must require an already-retained retryable control for the same semantic target",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn can_enqueue(&self, fanout: &PendingExactFanout)",
            "if self.retains_retryable_sidecar_responder_control_for(fanout) {",
            "if false && self.retains_retryable_sidecar_responder_control_for(fanout) {",
            "lane-effect preflight must validate geometry, consume only a same-target duplicate responder control, and otherwise charge reservation capacity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn can_enqueue_owned_reply_transfer(",
            "if self.retains_retryable_sidecar_responder_control_for(&fanout) {",
            "if false && self.retains_retryable_sidecar_responder_control_for(&fanout) {",
            "owned reply capacity preflight must consume only a same-target duplicate responder control without charging capacity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn enqueue_validated(",
            "if self.retains_retryable_sidecar_responder_control_for(&fanout) {",
            "if false && self.retains_retryable_sidecar_responder_control_for(&fanout) {",
            "worker exact output may retain at most one retryable responder control per semantic target while preserving independent controls and ordinary progress for other targets",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn accept_certified_merge_sidecar(",
            "self.accept_certified_merge_sidecar_generation_hint(sender, reply_route, &hint)",
            "Ok(V2LaneIngressOutcome::Rejected)",
            "lane sidecar ingress must exhaustively route the authenticated generation hint alongside every request, close, acknowledgement, and chunk variant",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "| CertifiedMergeSidecarMessage::GenerationHint(_) => None,",
            "=> None,",
            "only an immutable certified response chunk may create a writer-flush receipt from its exact route",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn post_certified_merge_sidecar_with_reply_routes(",
            "CertifiedMergeSidecarMessage::CloseAck(_)\n"
            "            | CertifiedMergeSidecarMessage::GenerationHint(_)\n"
            "            | CertifiedMergeSidecarMessage::Chunk(_) => reply_routes.is_some(),",
            "CertifiedMergeSidecarMessage::CloseAck(_)\n"
            "            | CertifiedMergeSidecarMessage::GenerationHint(_)\n"
            "            | CertifiedMergeSidecarMessage::Chunk(_) => reply_routes.is_none(),",
            "worker sidecar dispatch must keep Request and Close on topology while CloseAck, GenerationHint, and Chunk retain exact reply routes",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effect(",
            "| CertifiedMergeSidecarMessage::Chunk(_) => reply_routes.is_some(),",
            "| CertifiedMergeSidecarMessage::Chunk(_) => reply_routes.is_none(),",
            "runner sidecar dispatch must reject missing or extraneous route ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn next_effect(",
            "if take_sidecar {",
            "if !take_sidecar {",
            "lane effect peek must clone the exact fairly selected queue head",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn apply_bounded_sidecar_admissions<T, Error>(",
            "let mut applied = 0usize;",
            "return Ok(0); let mut applied = 0usize;",
            "runner exact-output ownership/ACK production seam",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn has_pending_exact_output(",
            "self.lock_pending_exact_output()",
            "return Ok(false); self.lock_pending_exact_output()",
            "worker exact-output ownership/ACK production seam",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "after_candidate_prune(merge_attempt);",
            "let _ = merge_attempt;",
            "candidate pruning must retain its ownership receipt while strict or explicitly authorized superseded history produces a typed merge receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "if candidate_routes.len() >= live_before_merge {",
            "if false && candidate_routes.len() >= live_before_merge {",
            "candidate pruning must retain its ownership receipt while strict or explicitly authorized superseded history produces a typed merge receipt",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "pub const V2_EXACT_OUTPUT_CLASS_COUNT: usize = 3;",
            "pub const V2_EXACT_OUTPUT_CLASS_COUNT: usize = 3;",
            "pub const V2_EXACT_OUTPUT_CLASS_COUNT: usize = 2;",
            "exact-output defaults must retain the reviewed completion divisor, reducer batch, and three-class geometry",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "pub const MAX_EFFECTS_PER_STEP: usize = 8;",
            "pub const MAX_EFFECTS_PER_STEP: usize = 8;",
            "pub const MAX_EFFECTS_PER_STEP: usize = 7;",
            "the dependency-free reducer refinement must retain the reviewed maximum effect batch",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_core.rs",
            "const _: [(); refinement::MAX_EFFECTS_PER_STEP]",
            "[(); iroha_config::parameters::defaults::sumeragi::V2_MAX_EFFECTS_PER_STEP]",
            "[(); iroha_config::parameters::defaults::sumeragi::V2_EXACT_OUTPUT_CLASS_COUNT]",
            "the production embedded reducer must bind its dependency-free batch to configured exact-output geometry",
        ),
        (
            "crates/iroha_config/src/parameters/actual.rs",
            "pub fn sumeragi_v2_exact_output_shared_ownership_capacity(",
            ".checked_add(certified_request_capacity)",
            ".saturating_add(certified_request_capacity)",
            "the shared exact-output owner must reserve both bounded producers and one complete reducer batch with checked arithmetic",
        ),
        (
            "crates/iroha_config/src/parameters/actual.rs",
            "pub fn validate_sumeragi_v2_exact_output_geometry(",
            ".checked_mul(defaults::sumeragi::V2_EXACT_OUTPUT_CLASS_COUNT)",
            ".saturating_mul(defaults::sumeragi::V2_EXACT_OUTPUT_CLASS_COUNT)",
            "the geometry kernel must reject zero, multiplication overflow, and any corridor smaller than source-count times exact classes",
        ),
        (
            "crates/iroha_config/src/parameters/user.rs",
            "pub fn parse(self) -> Result<actual::Root, ParseError> {",
            ".max_total_connections\n",
            ".max_connections_per_peer\n",
            "root configuration must derive the authenticated-source bound from network geometry and fail parsing",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn start_inner(",
            "validate_shared_ownership_geometry(\n"
            "            shared_pending_ownership_unit_capacity,\n"
            "            reply_route_source_capacity,\n"
            "        )?;",
            "validate_shared_ownership_geometry(\n"
            "            shared_pending_ownership_unit_capacity,\n"
            "            max_peers_per_fanout,\n"
            "        )?;",
            "production bounds protocol fanout by roster and source geometry while charging the shared pool only for the independently reserved authenticated reply sources",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "pub(crate) fn matches_semantic_origin(",
            "self.validate_exact() && self.first.semantic_origin.as_ref() == origin",
            "self.validate_exact()",
            "semantic-origin validation must compare the independently retained canonical request origin",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "pub(crate) fn advance_reply_cursors(",
            "if message_cursor < attempt.message_cursor || chunk_cursor < attempt.chunk_cursor {",
            "if false && (message_cursor < attempt.message_cursor || chunk_cursor < attempt.chunk_cursor) {",
            "a source attempt may advance but never reset either exact-output cursor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(crate) fn accept_payload_chunk_with_ingress_ownership",
            "|| !ingress_ownership.matches_semantic_origin(Some(authenticated_sender))",
            "|| false",
            "payload chunk effect consumption must reject a changed envelope or semantic origin before mutation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(crate) fn accept_certified_body_response_with_ingress_ownership",
            "|| !ingress_ownership.matches_semantic_origin(Some(authenticated_responder))",
            "|| false",
            "certified body response effect consumption must reject a changed envelope or semantic origin before mutation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn claimed_with_reply_routes_and_ingress_ownership(",
            "if !ownership.validate_exact() || !ownership.matches_reply_routes(Some(routes)) {",
            "if false {",
            "exact reply construction must attach only a validated fair-ingress carrier matching the complete per-source route set",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn serve_certified_request_on_routes(",
            "|| !ingress_ownership.matches_semantic_origin(Some(&admission.request.requester))",
            "|| false",
            "certified request service must bind canonical request, immutable requester origin, and every requester-targeted return source before queued local work",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn serve_certified_request_on_routes(",
            "|| reply_routes.semantic_target() != &admission.request.requester",
            "|| false",
            "certified request service must bind canonical request, immutable requester origin, and every requester-targeted return source before queued local work",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn commit_serve(",
            ".merge_downstream_with_observed_receipt(ingress_ownership, receipt)",
            ".merge_downstream(ingress_ownership)\n"
            "                .then_some(route_candidate)",
            "exact Serve retries must consume one observed-route receipt into cloned ingress ownership and atomically install the resulting route/ownership pair",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn commit_serve(\n"
            "        &self,\n"
            "        admission: &CertifiedServeAdmission,\n"
            "        reply_routes: NetworkReplyRoutes,\n"
            "        ingress_ownership: FairV2IngressOwnershipEvidence,\n"
            "    ) -> Result<CertifiedServeCommit, String> {\n"
            "        self.queue",
            "self.queue\n"
            "            .commit_serve(admission, reply_routes, ingress_ownership)",
            "let _ = reply_routes.clone().merge_observed_with_receipt(&reply_routes);\n"
            "        self.queue\n"
            "            .commit_serve(admission, reply_routes, ingress_ownership)",
            "the exact Serve retry and reply-target plan must be the worker-side observed-history reconciliation seams",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn post_to_peer_on_reply_routes(",
            "if reply_routes.semantic_target() != &peer",
            "if false",
            "certified response emission must validate the semantic target and exact route history under one fail-stop output operation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn post_to_peer_on_reply_routes(",
            "|| !ingress_ownership.matches_reply_routes(Some(&reply_routes))",
            "|| false",
            "certified response emission must validate the semantic target and exact route history under one fail-stop output operation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn post_to_peer_on_reply_routes(",
            "operation.complete();\n"
            "        Ok(())",
            "drop(operation);\n"
            "        Ok(())",
            "certified response emission preserves the complete authenticated route set until the guarded enqueue has completed",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn route_payload_chunk<R: EffectRuntime>(",
            "|| !ingress_ownership.matches_semantic_origin(Some(&sender))",
            "|| false",
            "payload chunk routing must bind canonical bytes and semantic sender before buffering or effect mutation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn buffer_orphan_payload_chunk_inner(",
            "if !retained.merge_downstream(candidate) {",
            "drop(candidate); if false {",
            "orphan chunk duplicates must merge alternate source ownership without replacing canonical semantic identity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn accept_lane_message_owned(",
            "|| !ownership.matches_semantic_origin(sender.as_ref())",
            "|| false",
            "lane ingress must bind semantic origin, canonical message, and the complete source route set before service",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn drain_v2_ingress(",
            "|| !ingress_ownership.matches_semantic_origin(inbound.sender())",
            "|| false",
            "runner ingress must retain canonical message, semantic origin, and source-isolated routes in one exact ownership carrier",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn drain_v2_ingress(",
            "if turn == OuterIngressTurn::Runtime {",
            "if false && turn == OuterIngressTurn::Runtime {",
            "an admitted or provisional exact Serve must suppress every later runtime-producer turn",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn drain_v2_ingress(",
            ".try_recv_if_checked_retiring_obsolete_with_barrier_bypass(barrier_bypass, |inbound| {",
            ".try_recv_if_at_checked_classified(|inbound| {",
            "ingress drain must use the gate-bound checked selector",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn drain_v2_ingress(",
            "|| !ingress_ownership.matches_semantic_origin(Some(sender))",
            "|| false",
            "exact Serve ingress must validate complete transport ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn drain_v2_ingress(",
            "Err(CertifiedServePrepareError::Backpressure) => {\n"
            "                        // `prepare_certified_request` installs the off-queue debt",
            "Err(CertifiedServePrepareError::Backpressure) => {\n"
            "                        return true;\n"
            "                        // `prepare_certified_request` installs the off-queue debt",
            "exact Serve ingress must validate complete transport ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn drain_v2_ingress(",
            "prepared_serve = Some(PreparedCertifiedServe::Admitted(admission));",
            "drop(admission);",
            (
                "exact Serve ingress must validate identity and reserve/coalesce its lifecycle atomically before the selected head can drain",
                "exact Serve ingress must validate complete transport ownership",
            ),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn drain_v2_ingress(",
            "None => {\n"
            "                            return Err(V2RunnerError::Service(",
            "None => {\n"
            "                            continue;\n"
            "                            return Err(V2RunnerError::Service(",
            "a current-height exact request may cross ingress removal only with its already-prepared lifecycle admission",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn bind(\n"
            "        ingress_ready: Arc<AtomicBool>,\n"
            "        block_ingress: Arc<FairV2Ingress>,\n"
            "        gate: CertifiedServeIngressGate,\n"
            "    ) -> Result<Self, V2RunnerError> {",
            ".bind_certified_serve_gate(gate.clone())",
            ".bind_certified_serve_gate_for_test(gate.clone())",
            "the per-height ingress owner must bind the exact Serve reservation gate before becoming live",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn certified_serve_ingress_gate(&self) -> CertifiedServeIngressGate {",
            "queue: Arc::clone(&self.command_tx.queue),",
            "queue: Arc::new(V2IoCommandQueue::default()),",
            "the I/O handle must expose a gate over its exact command queue rather than a detached reservation owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn certified_serve_ingress_gate(&self) -> Result<CertifiedServeIngressGate, String> {",
            ".map(V2IoHandle::certified_serve_ingress_gate)",
            ".map(|_| panic!(\"detached gate\"))",
            "production services must bind ingress to the live I/O handle's exact Serve queue",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn retire(&mut self) -> Result<(), V2RunnerError>",
            "close_ingress_for_rollover(&self.ingress_ready, &self.block_ingress);",
            "let _ = (&self.ingress_ready, &self.block_ingress);",
            "ingress rollover must close selection before unbinding the exact Serve reservation gate",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "impl Drop for CertifiedServeIngressBinding {\n"
            "    fn drop(&mut self) {",
            "if let Err(error) = self.retire() {",
            "if let Err(error) = Ok::<(), V2RunnerError>(()) {",
            "every abnormal binding drop must attempt the same ordered ingress retirement",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn run_inner(",
            "HeightIngressBindings::new(",
            "HeightIngressBindings::new_for_test(",
            "the runner must bind Serve and leader-wire ingress to one joint per-height queue owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn run_inner(",
            "height_ingress_bindings.retire()?;",
            "let _ = height_ingress_bindings.retire();",
            "every clean shutdown and finality path must atomically retire both per-height ingress bindings",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "fn v2_ingress_head_can_drain<R: EffectRuntime>(",
            "executor.can_admit_network_message_with_ingress_ownership(message, ingress_ownership)",
            "executor.can_admit_network_message(message)",
            "effect-executor preflight must preserve the exact fair-ingress carrier into owned runtime capacity admission",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn target_reservation(",
            "ExactTargetReservationKind::SidecarTopologyProgress",
            "ExactTargetReservationKind::Reliable",
            "reservation identity must isolate requester-owned topology progress from reliable reply-source ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn seal(&self) -> Result<(), String>",
            ".compare_exchange(false, true, AtomicOrdering::AcqRel, AtomicOrdering::Acquire)",
            ".compare_exchange(false, false, AtomicOrdering::AcqRel, AtomicOrdering::Acquire)",
            "durable exact-output handoff must seal its unique service owner exactly once",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn seal_applied_height_output_handoff(",
            "if pending.is_pending() {",
            "if false && pending.is_pending() {",
            "final exact-output handoff must validate durable authority, atomically empty the corridor, and one-shot seal every later enqueue",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn into_retained_merge_sidecars(",
            ".is_bound_to_transport_owner(&self.exact_output_handoff_owner)",
            ".matches_predecessor_context(&self.context)",
            "lane rollover must consume only its paired service receipt for the exact predecessor artifact and immediate successor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn require_peeked_lane_work_effect(",
            "drained.ok_or(V2RunnerError::RestartRequired)",
            "drained.ok_or(V2RunnerError::Service(\"lost peek\".to_owned()))",
            "runner lane dispatch must fail stop if its guarded peek loses the exact queued owner before drain",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "pub(crate) fn matches_body_coordinates(",
            "            && self.identity.view == round.view\n",
            "",
            "exact-output ingress seam ingress::leader_wire_token_matches_body_coordinates",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(crate) fn classify_payload_chunk_lifecycle(",
            "            return Ok(PayloadChunkLifecycleDisposition::Retain);\n",
            "            return Ok(PayloadChunkLifecycleDisposition::Volatile);\n",
            "a live exact fetch must retain its productive chunk lifecycle",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "fn begin_fetch<S: V2EffectServices>(",
            "        if round.context_id != self.context.id()\n",
            "        self.finality_completion = None;\n        if round.context_id != self.context.id()\n",
            "the durable Apply completion tombstone must have exactly the runtime and recovered authenticated installation paths",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(crate) fn complete_application<S: V2EffectServices>(",
            "        self.finality_completion = Some(FinalityCompletion {\n            tag,\n",
            "        self.finality_completion = Some(FinalityCompletion {\n            tag: EventTag::new(tag.height(), 0, tag.generation()),\n",
            "durable Apply completion must retain the exact tag in its non-resurrecting tombstone",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "fn begin_apply<S: V2EffectServices>(",
            "        if let Some(existing) = self.pending_applications.values().next() {\n",
            "        if false && let Some(existing) = self.pending_applications.values().next() {\n",
            "exact Apply retransmission must retain the incumbent authority and coalesce every later periodic lifecycle",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn route_payload_chunk<R: EffectRuntime>(",
            "            if self.has_exact_reconstructed_completion(manifest_hash, &ingress_ownership)? {\n",
            "            if false && self.has_exact_reconstructed_completion(manifest_hash, &ingress_ownership)? {\n",
            "an unmatched productive chunk must consult exact reconstructed and executor-owned lifecycle authority before buffering",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn replay_buffered_chunks<R: EffectRuntime>(",
            "        self.sweep_buffered_payload_chunk_lifecycles(executor)?;\n",
            "",
            "every orphan replay turn must sweep terminal exact chunk owners before selecting live fetch work",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn retire_buffered_payload_chunk_tail(",
            "                .mark_leader_wire_volatile_terminal(runtime)\n",
            "                .mark_leader_wire_volatile_terminal(runtime).and(Ok(()))\n",
            "exact-output ingress seam worker::retire_buffered_payload_chunk_tail",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn rehydrate_with_exact_geometry_after_durable_handoff(",
            "self.requeue_retained_outbound_after_height_rollover();",
            "let _ = &self;",
            "durable sidecar rehydration must preserve and requeue retained exact outbound ownership after validating the rollover authority",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/reply_route_retention.rs",
            "fn retain_active_owned_reply_routes_with_snapshot_hook<AfterSnapshot>(",
            "let (retained, receipt) = routes.retain_active_with_receipt();",
            "let retained = routes.retain_active();\n"
            "    let receipt = NetworkReplyRouteHistoryReceipt::default();",
            "runner pruning must retain every live source attempt and its tombstones",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn drain_v2_ingress(",
            "        if matches!(inbound.message(), BlockMessage::KuraReplicaAdvert(_)) {\n"
            "            admit_kura_replica_advert_ingress(receiver, kura, inbound)?;\n"
            "            continue;\n"
            "        }\n",
            "",
            "KuraReplicaAdvert ingress must bypass both consensus reducers",
        ),
    ),
)
def test_exact_output_production_source_mutations_fail_closed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    relative_path: str,
    region_marker: str,
    old: str,
    new: str,
    error_fragment: str | tuple[str, ...],
) -> None:
    module = load_checker()
    exact_output_production_fixture(tmp_path)

    path = tmp_path / relative_path
    source = path.read_text(encoding="utf-8")
    region_start = source.find(region_marker)
    assert region_start >= 0
    mutation = source.find(old, region_start)
    assert mutation >= 0
    next_item = re.search(
        r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?(?:async[ \t]+)?fn[ \t]+",
        source[region_start + len(region_marker) :],
    )
    if next_item is not None:
        next_item_start = region_start + len(region_marker) + next_item.start()
        assert mutation < next_item_start, (
            "mutation escaped the production Rust item selected by its region marker",
            relative_path,
            region_marker,
            old,
        )
    path.write_text(
        source[:mutation] + new + source[mutation + len(old) :],
        encoding="utf-8",
    )

    expected_errors = (
        [error_fragment]
        if isinstance(error_fragment, str)
        else list(error_fragment)
    )
    if region_marker == "fn lifecycle_max_snapshot_bytes_for_attempt_capacity(":
        expected_errors.extend(
            _apply_exact_output_non_runtime_extended_mutations(
                tmp_path, module, monkeypatch
            )
        )

    errors = module._exact_output_production_source_fidelity_errors(tmp_path)
    assert all(
        any(expected_error in error for error in errors)
        for expected_error in expected_errors
    ), errors


def _apply_exact_output_non_runtime_extended_mutations(
    tmp_path: Path, module, monkeypatch: pytest.MonkeyPatch
) -> list[str]:
    mutations = (
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn derive_server_request_capacities(",
            "|| server_stream_capacity > MAX_CERTIFIED_MERGE_SERVER_STREAMS",
            "|| false",
            "must enforce the protocol roster bound",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn lifecycle_protocol_max_snapshot_bytes(",
            "MAX_CERTIFIED_MERGE_SERVER_STREAMS,",
            "self.server_stream_capacity,",
            "must reserve the maximum bounded responder roster",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn rehydrate_after_lifecycle_restore(",
            "if server_stream_capacity < self.server_stream_capacity {",
            "if false && server_stream_capacity < self.server_stream_capacity {",
            "permit only monotonic same-roster capacity expansion",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "        Self {\n            session_capacity,",
            "historical_recovery_retry_floor,",
            "historical_recovery_retry_ceiling,",
            "lane limits must retain the exact configured reply-source geometry",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn new_with_output_guard_and_transport(",
            ".zip(verified_context.proofs_of_possession())",
            ".zip(std::iter::empty())",
            "must derive its context and frozen validator proofs from one verified carrier",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn new_with_output_guard_and_transport_inner(",
            "let sidecar_server_stream_capacity =\n"
            "            merge_sidecar_server_stream_capacity(sidecar_server_roster.len());",
            "let sidecar_server_stream_capacity = 1;",
            "must derive the canonical responder roster",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn hydrate_canonical_lane_artifacts(",
            "&& !self.lane_sessions.contains_proposal(proposal)",
            "&& false",
            "must bound and authorize every exact historical recovery proposal",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn validate_winning_lane_output(",
            "validate_lane_vote_for_proposal(vote, proposal)?;",
            "let _ = (vote, proposal);",
            "must authenticate both standalone votes and aggregate certificates",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn durable_lane_rollover_authority(",
            "if retained_proposal == proposal",
            "if true",
            "must require either an exact durable certificate or the bounded retained successor owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn durable_lane_rollover_authority(",
            "durable_sessions.insert(proposal.proposal_hash, source);",
            "let _ = source;",
            "must preserve one exact retained or persistent witness per winner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "struct FinalityCompletion {",
            "ownership: FinalityCompletionOwner,",
            "ownership: Option<FinalityCompletionOwner>,",
            "durable Apply tombstone must retain the exact reducer incarnation tag",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(crate) fn complete_application<S: V2EffectServices>(",
            "            ownership: FinalityCompletionOwner::Runtime(ownership),\n        });",
            "            ownership: FinalityCompletionOwner::RecoveredDecisionApply(\n"
            "                RecoveredDecisionApplyDispatchKeyV1::new_for_test(),\n            ),\n        });",
            "durable Apply completion must retain the exact tag",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "fn begin_apply<S: V2EffectServices>(",
            ".same_commit_decision(certificate.as_ref())",
            ".same_commit_decision(existing.task.certificate.as_ref())",
            "exact Apply retransmission must retain the incumbent authority",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn sweep_buffered_payload_chunk_lifecycles<R: EffectRuntime>(",
            "                Err(error) => {\n"
            "                    self.orphan_lifecycle_sweep_cursor = Some(OrphanPayloadLifecycleSweepCursor {\n"
            "                        manifest_hash: cursor.manifest_hash,\n"
            "                        chunk_offset: cursor.chunk_offset.saturating_add(1),\n"
            "                    });",
            "                Err(error) => {\n"
            "                    self.orphan_lifecycle_sweep_cursor = None;",
            "buffered lifecycle sweep must preserve every nonterminal or unclassifiable exact owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "if !self.rollover_claim.accepts_superseded_reply_delivery() {",
            "if false {",
            "explicitly authorized superseded history produces a typed merge receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            ".merge_observed_with_receipt(&candidate_routes)",
            ".merge_with_receipt(&candidate_routes)",
            "worker-side observed-history reconciliation seams",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            ".merge_downstream_with_observed_receipt(candidate, receipt)",
            ".merge_downstream_with_strict_receipt(candidate, receipt)",
            "must consume the typed strict or superseded receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn applied_height_reconstruction_covers(",
            "if proposal_height >= artifact.height {",
            "if false {",
            "lane output retirement must require exact typed durable or independently readable historical lane authority",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "let covered = match envelope.as_message() {",
            "lane_output_is_covered(lane_message)?",
            "false",
            "lane output retirement must route every typed lane class through the reviewed authority classifier",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn run_inner(",
            "            leader_wire_recovery_authority,\n"
            "            exact_output_service_owner,",
            "            None,\n            exact_output_service_owner,",
            "runner construction must move the unique service owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "let lane_work_limits = lane_work_limits(",
            "            retransmit_interval,\n            round_timeout,",
            "            Duration::ZERO,\n            Duration::ZERO,",
            "runner construction must pass the exact P2P source geometry into lane work",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "V2LaneWorkAdapter::new_with_output_guard_and_transport(",
            "                    &verified_context,",
            "                    &context,",
            "lane adapter must be constructed from the same configured role after startup reconciliation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "V2LaneWorkAdapter::new_with_output_guard_and_transport(",
            "                    _lifecycle_process_generation.clone(),",
            "                    None,",
            "lane adapter must receive the same process-lifetime generation claim",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn drain_v2_ingress(",
            "executor.can_admit_timeout_vote_recovery_episode(message, ownership)",
            "executor.can_admit_timeout_vote_recovery_episode(message, ownership,)",
            "ingress drain must use the gate-bound checked selector, keep the TimeoutVote bypass explicit, and revalidate runtime capacity before physical removal",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker/exact_output_rollover_claim.rs",
            "const fn accepts_superseded_reply_delivery(&self) -> bool {",
            "| Self::DurableCertifiedBodyResponse { .. }",
            "| Self::DurableLaneCertificateResponse { .. }",
            "superseded reply history must be limited to durable global response claims",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "fn matches_apply(",
            "matches!(&self.ownership, FinalityCompletionOwner::Runtime(retained) if retained == ownership)",
            "true",
            "durable Apply tombstone equality must bind the runtime incarnation, tag, finality decision, and Kura receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn durable_historical_lane_output_source_hash(",
            "if durable.proposal.proposal_hash != proposal_hash {",
            "if false {",
            "historical lane retirement must authenticate the exact durable proposal and bind its output hash",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn durable_historical_lane_verification_pops(",
            "|| hint.proposal_block_hash != finality.block_hash",
            "|| false",
            "historical lane verification must source alternate signer PoPs from the frozen finality roster",
        ),
    )
    diagnostics = []
    for relative_path, marker, old, new, diagnostic in mutations:
        path = tmp_path / relative_path
        source = path.read_text(encoding="utf-8")
        region = source.find(marker)
        assert region >= 0, (relative_path, marker)
        offset = source.find(old, region)
        assert offset >= 0, (relative_path, marker, old)
        path.write_text(
            source[:offset] + new + source[offset + len(old) :],
            encoding="utf-8",
        )
        diagnostics.append(diagnostic)
    monkeypatch.setattr(module, "_require_rust_item_token_sha256", lambda *args: None)
    return diagnostics
