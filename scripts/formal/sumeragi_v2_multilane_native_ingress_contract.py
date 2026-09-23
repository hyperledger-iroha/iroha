"""Structural controls for Native physical ingress across global rollover.

Native admission uses the sole bounded fair queue and process-lived consumer.
These controls bind custody across global rollover, not network liveness.
"""
from __future__ import annotations

from pathlib import Path
import re
from typing import Callable

from sumeragi_v2_multilane_geometry_evidence_contract import _code

CORE = "crates/iroha_core/src/sumeragi/mod.rs"
NATIVE = "crates/iroha_core/src/sumeragi/fair_v2_ingress_native.rs"
SELECTOR = "crates/iroha_core/src/sumeragi/fair_v2_ingress_selector.rs"
POSITION = "crates/iroha_core/src/sumeragi/v2_lifecycle_ingress_position.rs"
TESTS = "crates/iroha_core/src/sumeragi/tests/mod_native_ingress_rollover.rs"
SOURCE_RELATIVES = tuple(map(Path, (
    CORE, NATIVE, POSITION, SELECTOR, TESTS,
    "crates/iroha_core/src/sumeragi/tests/mod_authoritative_runtime_gate_03_admission_and_fairness.rs",
    "crates/iroha_core/src/sumeragi/v2_runner/native_process.rs",
    "scripts/formal/sumeragi_v2_multilane_native_ingress_contract.py",
    "pytests/scripts/sumeragi_v2_multilane_native_ingress_contract_test.py",
)))

# Small authority projections are bound in full; larger owners have ordered
# executable obligations and explicit forbidden destructive operations below.
EXACT = (
    (CORE, "enum", "FairV2IngressSource", """
        enum FairV2IngressSource { Validator(PeerId), Authenticated(PeerId), Native(PeerId) }
    """),
    (CORE, "enum", "FairV2IngressSourceClass", """
        enum FairV2IngressSourceClass { Validator, Authenticated }
    """),
    (CORE, "method", "FairV2IngressSource::is_native", """
        const fn is_native(&self) -> bool { matches!(self, Self::Native(_)) }
    """),
    (CORE, "method", "FairV2IngressSource::uses_authenticated_capacity", """
        const fn uses_authenticated_capacity(&self) -> bool {
            matches!(self, Self::Authenticated(_) | Self::Native(_))
        }
    """),
    (CORE, "method", "FairV2IngressSource::class", """
        const fn class(&self) -> FairV2IngressSourceClass {
            match self {
                Self::Validator(_) => FairV2IngressSourceClass::Validator,
                Self::Authenticated(_) | Self::Native(_) => FairV2IngressSourceClass::Authenticated,
            }
        }
    """),
    (CORE, "method", "FairV2Ingress::try_push_at", """
        fn try_push_at(&self, inbound: InboundBlockMessage, enqueued_at: Instant)
        -> Result<FairV2IngressPushDisposition, FairV2IngressPushError> {
            self.try_push_owned_at(inbound, enqueued_at)
        }
    """),
    (CORE, "fn", "fair_v2_ingress_append_source_identity", """
        fn fair_v2_ingress_append_source_identity(
            projection: &mut Vec<u8>, source: &FairV2IngressSource,
            append_peer: &mut impl FnMut(&mut Vec<u8>, &PeerId),
        ) {
            match source {
                FairV2IngressSource::Validator(peer) => { projection.push(0); append_peer(projection, peer); }
                FairV2IngressSource::Authenticated(peer) => { projection.push(1); append_peer(projection, peer); }
                FairV2IngressSource::Native(peer) => { projection.push(2); append_peer(projection, peer); }
            }
        }
    """),
    (NATIVE, "method", "FairV2IngressState::retain_native_ingress", """
        pub(super) fn retain_native_ingress(&mut self) {
            self.lanes.retain(|source, _| source.is_native());
            self.pending_wire_owners.retain(|_, source| source.is_native());
            self.ready.retain(|source| source.is_native());
            self.len = self.lanes.values().map(|lane| lane.entries.len()).sum();
            self.bytes = self.lanes.values().map(|lane| lane.bytes).sum();
            self.nonempty_since = self.lanes.values()
                .flat_map(|lane| lane.entries.iter().map(|entry| entry.enqueued_at)).min();
            if self.len == 0 { self.last_service_attempt_at = None; }
        }
    """),
    (NATIVE, "method", "FairV2IngressState::has_global_ingress", """
        pub(super) fn has_global_ingress(&self) -> bool {
            self.lanes.iter().any(|(source, lane)| {
                !source.is_native() && (!lane.entries.is_empty() || !lane.pending_wire.is_empty() || lane.bytes != 0)
            }) || self.pending_wire_owners.values().any(|source| !source.is_native())
                || self.ready.iter().any(|source| !source.is_native())
        }
    """),
)

RETAINED_CAPACITY = """
    let required = required.max(state.len.saturating_add(
        fair_v2_ingress_current_protected_slots(&state, self.authenticated_non_validator_source_capacity),
    ));
    if required > self.capacity {
        return Err(FairV2IngressCapacityError {
            configured: self.capacity, required, kind: FairV2IngressCapacityKind::Messages,
        });
    }
"""
ORDERED = (
    (SELECTOR, "fn", "fair_v2_ingress_queue_gate_verdict", (
        "let entry = &lane.entries[index];",
        "let leader_wire_barrier = leader.selected_barrier.as_ref();",
        """let ingress_barrier_allows = source.is_native()
            || leader_wire_barrier.is_none_or(|owner| {
                index < owner.ingress_predecessors.get(source).copied().unwrap_or(0)
                    || (owner.ingress_predecessors.iter()
                        .all(|(predecessor, count)| predecessor.is_native() || *count == 0)
                        && entry.leader_wire_token.as_ref() == Some(&owner.token))
            });""",
        "let dependency_bypass = !ingress_barrier_allows",
        """if has_live_control_predecessor || (!ingress_barrier_allows && !dependency_bypass) {
            FairV2IngressQueueGateVerdict::Blocked
        } else if dependency_bypass || (source.is_native() && leader_wire_barrier.is_some()) {
            FairV2IngressQueueGateVerdict::Dependency
        } else { FairV2IngressQueueGateVerdict::Strict }""",
    )),
    (CORE, "method", "FairV2Ingress::try_push", (
        "self.try_push_at(inbound, Instant::now())",
    )),
    (CORE, "method", "FairV2Ingress::try_push_owned_at", (
        "message if message.is_lane_local() || message.is_native_lane() =>",
        "if encoded_len > lane_limit",
        "let _producer_publication_guard = self.producer_publication_lock.lock();",
        "let mut state = self.state.lock();",
        "if !state.open { return Err(FairV2IngressPushError::Closed(inbound)); }",
        """let source = if inbound.message().is_native_lane() {
            FairV2IngressSource::Native(inbound.via.clone())
        } else if state.roster.contains(inbound.via()) {
            FairV2IngressSource::Validator(inbound.via.clone())
        } else { FairV2IngressSource::Authenticated(inbound.via.clone()) };""",
        "let authenticated_via_is_validator = matches!(&source, FairV2IngressSource::Validator(_));",
        "state.pending_wire_owners.get(key).cloned().map(|owner| (key, owner))",
        "return Ok(FairV2IngressPushDisposition::Coalesced);",
        "let source_lane_is_new = !state.lanes.contains_key(&source);",
        "if source_lane_is_new && !source.uses_authenticated_capacity()",
        """let retained_authenticated_non_validator_sources = state.lanes.keys()
            .filter(|source| source.uses_authenticated_capacity()).count();""",
        """if self.authenticated_non_validator_source_capacity
            .is_some_and(|capacity| retained_authenticated_non_validator_sources >= capacity) {
            return Err(FairV2IngressPushError::Full(inbound));
        }""",
        """occurrence.lifecycle_ordinal = if source.is_native() { None }
            else if let Some(token) = leader_wire_token.as_ref()""",
        "if !occurrence.validate_exact()",
        "state.last_admission_ordinal = admission_ordinal;",
        "lane.entries.push_back(FairV2IngressEntry",
    )),
    (CORE, "fn", "fair_v2_ingress_current_protected_slots", (
        ".filter(|source| source.uses_authenticated_capacity()).count();",
        "capacity.checked_sub(materialized_authenticated)",
        ".and_then(|latent| latent.checked_mul(3))",
    )),
    (CORE, "method", "FairV2Ingress::configure_roster_with_byte_requirements", (
        "let _service_guard = self.service_lock.lock();",
        "let mut state = self.state.lock();", "state.open = false;",
        "state.retain_native_ingress();", "state.roster = roster;",
        "FairV2IngressSource::Validator(peer)", RETAINED_CAPACITY,
    )),
    (CORE, "method", "FairV2Ingress::retire_leader_wire_lifecycle_gate", (
        "let _service_guard = self.service_lock.lock();",
        "let mut state = self.state.lock();",
        "bound.park_sealed_ingress(carriers)?;", "state.retain_native_ingress();",
        "state.leader_wire_lifecycle_gate = None;",
        "state.leader_wire_lifecycle_ordinals = None;", "state.leader_wire_context = None;",
        "retirement.complete();",
    )),
    (CORE, "method", "FairV2Ingress::open", (RETAINED_CAPACITY, "state.open = true;")),
    (CORE, "method", "FairV2Ingress::bind_leader_wire_lifecycle_gate", (
        "if state.open || state.has_global_ingress()",
        "if gate.restore()? != restore", "if !gate.matches_geometry(",
    )),
    (CORE, "method", "FairV2Ingress::dequeue_selected_locked", (
        "state.pending_wire_owners.remove(key)",
        "else if source.uses_authenticated_capacity()",
        "state.lanes.remove(&source)",
    )),
    (CORE, "method", "FairV2IngressOwnershipOccurrence::validate_exact", (
        """let native = matches!(self.message_kind,
            FairV2IngressMessageKind::NativeLane | FairV2IngressMessageKind::NativeLaneDecision);""",
        """let source_exact = native == self.authenticated_source.is_native()
            && (!native || self.lifecycle_ordinal.is_none())""",
        """FairV2IngressSource::Authenticated(source) | FairV2IngressSource::Native(source) => {
            !self.authenticated_via_is_validator && source == &self.authenticated_via
        }""",
    )),
    (POSITION, "fn", "entry_storage_is_exact", (
        """let expected_source = if entry.inbound.message().is_native_lane() {
            FairV2IngressSource::Native(entry.inbound.via().clone())
        } else if state.roster.contains(entry.inbound.via())""",
        "&& *source == expected_source",
        "&& Arc::ptr_eq(&ownership.first.encoded_bytes, &entry.encoded_bytes)",
    )),
    (CORE, "method", "FairV2Ingress::ensure_closed_drained_cut", (
        "let has_lane_physical_ownership = state.lanes.values().any(|lane|",
        "if state.len != 0 || state.bytes != 0",
        "|| !state.pending_wire_owners.is_empty() || has_lane_physical_ownership",
    )),
    (POSITION, "method", "FairV2Ingress::ensure_closed_global_drained_cut", (
        "let _service_guard = self.service_lock.lock();",
        "let _publication_guard = self.producer_publication_lock.lock();",
        "let state = self.state.lock();",
        'if state.open { return Err("finalized global ingress cut remained open".to_owned()); }',
        'validate_live_queue_structure(&state).map_err(|error| { format!("finalized global ingress cut changed accounting: {error:?}") })?;',
        """if state.has_global_ingress()
            || (state.len == 0 && state.last_service_attempt_at.is_some())
            || state.leader_wire_lifecycles.values().any(|record| {
                matches!(record.status,
                    super::super::FairV2IngressLeaderWireStatus::Ingress
                        | super::super::FairV2IngressLeaderWireStatus::Runtime)
            }) {
                return Err("finalized global ingress cut retained global ownership".to_owned());
            }""",
        """let retained = state.lanes.values().flat_map(|lane| lane.entries.iter())
            .map(|entry| { (Arc::clone(&entry.inbound), Arc::clone(&entry.ownership_snapshot)) })
            .collect::<Vec<_>>();""",
        "drop(state);",
        "let mut peer_encodings = super::super::FairV2IngressPeerIdentityEncodings::default();",
        "for (inbound, snapshot) in retained",
        'let live = inbound.ingress_ownership().ok_or_else(|| "finalized global ingress cut lost Native ownership".to_owned())?;',
        """if !inbound.message().is_native_lane()
            || !snapshot.validate_exact()
            || !live.validate_exact()
            || snapshot.process_local_projection_hash_with_peer_encodings(&mut peer_encodings)
                != live.process_local_projection_hash_with_peer_encodings(&mut peer_encodings)
            || !live.matches_message(inbound.message())
            || !live.matches_semantic_origin(inbound.sender())
            || !live.matches_reply_routes(inbound.reply_routes())
            || live.leader_wire_token().is_some()
            || live.leader_wire_runtime_receipt().is_some()
            || live.runtime_lifecycle_ordinal().is_some() {
                return Err("finalized global ingress cut changed Native ownership".to_owned());
            }""",
        "Ok(())",
    )),
    (POSITION, "fn", "validate_live_queue_structure", (
        """if state.ready.iter().any(|source| !ready_sources.insert(source.clone())) {
            return Err(FairIngressQueueCutError::DuplicateReadySource);
        }""",
        """let nonempty_sources = state.lanes.iter()
            .filter(|(_, lane)| !lane.entries.is_empty())
            .map(|(source, _)| source.clone()).collect::<BTreeSet<_>>();""",
        """if ready_sources != nonempty_sources {
            return Err(FairIngressQueueCutError::ForeignReadyLane);
        }""",
        "for (source, lane) in &state.lanes",
        "for entry in &lane.entries",
        """if !entry_storage_is_exact(state, source, entry)
            || !admission_ordinals.insert(entry.admission_ordinal) {
                return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
            }""",
        "let key = entry.wire_key.as_ref().ok_or(FairIngressQueueCutError::InvalidOccurrenceIdentity)?;",
        """if !pending_wire.insert(key.clone())
            || pending_wire_owners.insert(key.clone(), source.clone()).is_some() {
                return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
            }""",
        "total_len = total_len.checked_add(1).ok_or(FairIngressQueueCutError::PositionOverflow)?;",
        "lane_bytes = lane_bytes.checked_add(entry.encoded_len).ok_or(FairIngressQueueCutError::PositionOverflow)?;",
        "if entry.class == FairV2IngressClass::Progress",
        "progress_len = progress_len.checked_add(1).ok_or(FairIngressQueueCutError::PositionOverflow)?;",
        "if super::super::fair_v2_ingress_is_certified_fence_escape(&entry.inbound)",
        "certified_fence_escape_len = certified_fence_escape_len.checked_add(1).ok_or(FairIngressQueueCutError::PositionOverflow)?;",
        "certified_fence_escape_bytes = certified_fence_escape_bytes.checked_add(entry.encoded_len).ok_or(FairIngressQueueCutError::PositionOverflow)?;",
        "if super::super::fair_v2_ingress_is_timeout_vote(&entry.inbound)",
        "timeout_vote_len = timeout_vote_len.checked_add(1).ok_or(FairIngressQueueCutError::PositionOverflow)?;",
        "if matches!(source, FairV2IngressSource::Validator(_))",
        "timeout_vote_bytes = timeout_vote_bytes.checked_add(entry.encoded_len).ok_or(FairIngressQueueCutError::PositionOverflow)?;",
        "if entry.class == FairV2IngressClass::TransportCompletion",
        "transport_completion_len = transport_completion_len.checked_add(1).ok_or(FairIngressQueueCutError::PositionOverflow)?;",
        "transport_completion_bytes = transport_completion_bytes.checked_add(entry.encoded_len).ok_or(FairIngressQueueCutError::PositionOverflow)?;",
        "total_bytes = total_bytes.checked_add(lane_bytes).ok_or(FairIngressQueueCutError::PositionOverflow)?;",
        """if lane.pending_wire != pending_wire
            || lane.progress_len != progress_len
            || lane.certified_fence_escape_len != certified_fence_escape_len
            || lane.timeout_vote_len != timeout_vote_len
            || lane.transport_completion_len != transport_completion_len
            || lane.bytes != lane_bytes
            || lane.certified_fence_escape_bytes != certified_fence_escape_bytes
            || lane.timeout_vote_bytes != timeout_vote_bytes
            || lane.transport_completion_bytes != transport_completion_bytes {
                return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
            }""",
        """if state.pending_wire_owners != pending_wire_owners
            || state.len != total_len || state.bytes != total_bytes
            || state.nonempty_since.is_some() != (total_len != 0)
            || admission_ordinals.last().is_some_and(|ordinal| *ordinal > state.last_admission_ordinal) {
                return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
            }""",
        "Ok(())",
    )),
)


def validate_owners(root: Path, errors: list[str], rust_binding_item: Callable) -> None:
    """Bind separate global lifecycle and retained Native physical ownership."""
    for relative, kind, symbol, expected in EXACT:
        item = rust_binding_item(root, relative, kind, symbol, "Native ingress", errors)
        if item is not None and _code(item) != _code(expected):
            errors.append(f"Native ingress exact owner changed for {symbol}")
    for relative, kind, symbol, relations in ORDERED:
        item = rust_binding_item(root, relative, kind, symbol, "Native ingress", errors)
        if item is None:
            continue
        code, cursor = _code(item), 0
        for relation in relations:
            needle = _code(relation)
            position = code.find(needle, cursor)
            if position < 0:
                errors.append(f"Native ingress {symbol} missing or reorders relation {relation!r}")
                break
            cursor = position + len(needle)
        if symbol == "fair_v2_ingress_queue_gate_verdict":
            if "return" in code:
                errors.append("Native ingress queue gate adds an early verdict bypass")
            if any(_code(gate) in code for gate in ("#[cfg(", "#[cfg_attr(", "cfg!(")):
                errors.append("Native ingress queue gate conditionally disables domain ordering")
            if code.count("letingress_barrier_allows=") != 1:
                errors.append("Native ingress queue gate changes its sole domain barrier")
        if symbol in {"FairV2Ingress::configure_roster_with_byte_requirements",
                      "FairV2Ingress::retire_leader_wire_lifecycle_gate"}:
            for destructive in ("state.lanes.clear()", "state.ready.clear()",
                                "state.pending_wire_owners.clear()", "state.len = 0",
                                "state.bytes = 0", "state.last_admission_ordinal = 0"):
                if _code(destructive) in code:
                    errors.append(f"Native ingress {symbol} destroys retained custody: {destructive}")
        if symbol == "FairV2Ingress::ensure_closed_global_drained_cut":
            for forbidden in (
                "drop(_service_guard)", "drop(_publication_guard)",
                "state.lanes.clear()", "state.ready.clear()", "state.pending_wire_owners.clear()",
                "state.retain_native_ingress()",
                "self.try_recv", "self.dequeue", "self.retire_leader_wire_lifecycle_gate",
            ):
                if _code(forbidden) in code:
                    errors.append(f"Native ingress {symbol} releases or mutates original custody: {forbidden}")
            if re.search(r"state\.(?:len|bytes|open)=(?!=)", code):
                errors.append(f"Native ingress {symbol} rewrites original accounting or admission")
        if symbol in {"FairV2Ingress::ensure_closed_global_drained_cut", "validate_live_queue_structure"}:
            if code.count(_code("Ok(())")) != 1:
                errors.append(f"Native ingress {symbol} adds an early success bypass")
            if any(_code(gate) in code for gate in ("#[cfg(", "#[cfg_attr(", "cfg!(")):
                errors.append(f"Native ingress {symbol} conditionally disables release-mode authentication")
