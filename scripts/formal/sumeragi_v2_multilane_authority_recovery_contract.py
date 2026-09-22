"""Authenticated State, Native predecessor and canonical recovery source contracts.

These structural obligations bind actual owner bodies. They are not executed
consensus evidence or a replacement for the complete reviewed include closure.
"""
from __future__ import annotations

from pathlib import Path
from typing import Any, Callable

from sumeragi_v2_multilane_geometry_evidence_contract import _code

AUTHORITY_RECOVERY_BINDINGS = (('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/sumeragi/v2_lane_work.rs',
  'method',
  'V2LaneWorkAdapter::native_body_matches_context',
  ('body.authority_context_height == self.context.height',
   'self.native_participant_predecessor_is_current(body)',
   'body.coordinator_lane_incarnation',
   'body.participant_lane_incarnation',
   'self.globally_locked_body.is_none()',
   '!self.decision_pending()',
   '.native_coordinator_height_is_current(body)\n                .unwrap_or(false)',
   '== Some(body.coordinator_lane_incarnation)',
   '== Some(body.participant_lane_incarnation)'),
  ('self.globally_locked_body.is_none()',
   '.native_coordinator_height_is_current(body)\n                .unwrap_or(false)',
   'self.native_participant_predecessor_is_current(body)',
   '== Some(body.coordinator_lane_incarnation)',
   '== Some(body.participant_lane_incarnation)')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/sumeragi/v2_lane_work.rs',
  'method',
  'V2LaneWorkAdapter::native_request_matches_context',
  ('request.validate_plan_binding().is_ok()',
   'self.native_body_matches_context(&request.body, active_view)',
   'request.participant_settlement.source_ids().len() <= source_capacity',
   '.native_coordinator_predecessor_is_current(request)\n                .unwrap_or(false)',
   'self.native_control_predecessor_is_current(request)'),
  ('request.validate_plan_binding().is_ok()',
   'self.native_body_matches_context(&request.body, active_view)',
   '.native_coordinator_predecessor_is_current(request)\n                .unwrap_or(false)',
   'self.native_control_predecessor_is_current(request)')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/sumeragi/v2_lane_work.rs',
  'fn',
  'native_coordinator_tip',
  ('if !self.lane_route_active(\n'
   '            body.coordinator_lane_id,\n'
   '            body.coordinator_dataspace_id,\n'
   '            body.coordinator_lane_incarnation,\n'
   '            body.authority_context_height,\n'
   '        ) {\n'
   '            return Ok(None);\n'
   '        }',
   'let pending = self.consensus_storage_read(',
   '.unapplied_lane_block_artifact_heights_snapshot_cached(),\n        )?;',
   'if pending.contains_key(&(body.coordinator_lane_id, body.coordinator_dataspace_id)) {\n'
   '            return Ok(None);\n'
   '        }',
   'self.consensus_storage_read(v2_known_lane_tip_for_route(\n'
   '            self.state.as_ref(),\n'
   '            self.kura.as_ref(),\n'
   '            body.authority_context_height,\n'
   '            body.coordinator_lane_id,\n'
   '            body.coordinator_dataspace_id,\n'
   '            body.coordinator_lane_incarnation,\n'
   '        ))'),
  ('if !self.lane_route_active(',
   'let pending = self.consensus_storage_read(',
   'if pending.contains_key(&(body.coordinator_lane_id, body.coordinator_dataspace_id))',
   'self.consensus_storage_read(v2_known_lane_tip_for_route(')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/sumeragi/v2_lane_work.rs',
  'fn',
  'native_coordinator_height_is_current',
  ('let Some((height, descriptor_hash)) = self.native_coordinator_tip(body)? else {\n'
   '            return Ok(false);\n'
   '        }',
   '(height == 0) == descriptor_hash.is_none()',
   'height.checked_add(1) == Some(body.planned_coordinator_block_height)'),
  ('self.native_coordinator_tip(body)?',
   'return Ok(false);',
   '(height == 0) == descriptor_hash.is_none()',
   'height.checked_add(1) == Some(body.planned_coordinator_block_height)')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/sumeragi/v2_lane_work.rs',
  'fn',
  'native_coordinator_predecessor_is_current',
  ('if !self.native_coordinator_height_is_current(&request.body)?\n'
   '            || request.body.planned_coordinator_block_height\n'
   '                != request.coordinator_proposal.descriptor.lane_block_height\n'
   '        {\n'
   '            return Ok(false);\n'
   '        }',
   'let view = self.state.view();',
   'self.consensus_storage_read(State::lane_block_predecessor_is_applied_for_snapshot(\n'
   '            &view,\n'
   '            &request.coordinator_proposal,\n'
   '            crate::state::LanePredecessorApplicationMode::CurrentTip,\n'
   '        ))'),
  ('self.native_coordinator_height_is_current(&request.body)?',
   '!= request.coordinator_proposal.descriptor.lane_block_height',
   'return Ok(false);',
   'let view = self.state.view();',
   'self.consensus_storage_read(State::lane_block_predecessor_is_applied_for_snapshot(',
   'crate::state::LanePredecessorApplicationMode::CurrentTip')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/sumeragi/v2_lane_work.rs',
  'fn',
  'consensus_storage_read',
  ('read.map_err(|error| {',
   'self.output_guard.close_admission_for_restart();',
   'V2LaneWorkError::Persistence(error.to_string())'),
  ('read.map_err(|error| {',
   'self.output_guard.close_admission_for_restart();',
   'V2LaneWorkError::Persistence(error.to_string())')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/state.rs',
  'fn',
  'lane_block_predecessor_is_applied_for_snapshot',
  ('height <= prefix_height\n'
   '                && index.is_some_and(|index| state.block_hashes().get(index) == Some(&hash))',
   'Self::validate_native_amx_participant_shared_frontier(state.world(), &marker)?;',
   '.read_native_amx_participant_application_history(descriptor.lane_id)\n'
   '            .map_err(MergeLedgerCommitError::Persistence)?;',
   'if application_height > prefix_height {\n'
   '                if mode == LanePredecessorApplicationMode::CurrentTip {\n'
   '                    return Ok(false);\n'
   '                }\n'
   '                continue;\n'
   '            }',
   'if !prefix_contains(application_height, application_hash)',
   'route_matches &= same_route;',
   'pending |= !matches!(observation, Observation::Applied(_));',
   'if pending || !route_matches {\n            return Ok(false);\n        }',
   '(Some(_), None) | (None, Some(_)) => return Ok(false),',
   'if !Self::native_amx_participant_receipt_matches_frontier(&receipt, marker)',
   '&& (mode == LanePredecessorApplicationMode::CurrentTip\n'
   '                        || artifact.ownership.proposal_height <= prefix_height)',
   'if ownership.proposal_height > prefix_height {\n'
   '                return Ok(false);\n'
   '            }',
   'Self::lane_block_artifact_matches_certified_proposal(&artifact, &receipt.proposal)\n'
   '                    && prefix_contains(receipt.application_block_height, '
   'receipt.application_block_hash)',
   'if !state.kura().is_audited_snapshot_import_height(height) {\n'
   '                        return Ok(false);\n'
   '                    }',
   'if mode != LanePredecessorApplicationMode::OrdinaryBodyStatePrefix {\n'
   '                        return Ok(false);\n'
   '                    }',
   'if canonical.as_slice() != [artifact.clone()] {\n'
   '                        return Ok(false);\n'
   '                    }',
   'if ownership.proposal_height >= descriptor.proposal_height {\n'
   '                return Ok(false);\n'
   '            }',
   'if height == latest_height && hash != latest_hash =>',
   'descriptor.previous_lane_block_height == 0\n'
   '                    && descriptor.previous_lane_block_descriptor_hash.is_none()\n'
   '                    && descriptor.lane_block_height == 1',
   'descriptor.previous_lane_block_height == height\n'
   '                    && descriptor.previous_lane_block_descriptor_hash == Some(hash)\n'
   '                    && height.checked_add(1) == Some(descriptor.lane_block_height)'),
  ('Self::validate_native_amx_participant_shared_frontier(state.world(), &marker)?;',
   '.read_native_amx_participant_application_history(descriptor.lane_id)',
   'for (height, observation) in history.entries()',
   'if application_height > prefix_height',
   'if !prefix_contains(application_height, application_hash)',
   'if pending || !route_matches',
   'match (native_marker, native_receipt)',
   '.latest_lane_block_artifact_matching(descriptor.lane_id, |artifact|',
   'if !receipt_applies && !merge_applies',
   'if !state.kura().is_audited_snapshot_import_height(height)',
   'if mode != LanePredecessorApplicationMode::OrdinaryBodyStatePrefix',
   'if canonical.as_slice() != [artifact.clone()]',
   'for (height, hash) in tips',
   'Ok(match latest')),
 ('SumeragiV2NativeApplicationEvidence',
 'crates/iroha_core/src/sumeragi/lane_planner.rs',
 'fn',
 'v2_known_lane_tip_for_route',
 ('let mut matching = v2_known_lane_tips(state, proposal_height)?',
  'tip.lane_id == lane_id\n'
  '                && tip.dataspace_id == dataspace_id\n'
  '                && tip.lane_incarnation == lane_incarnation',
  'if !kura.emergency_fast_startup_enabled()',
  '.read_native_amx_participant_application_history(lane_id)\n'
  '            .map_err(crate::state::MergeLedgerCommitError::Persistence)?;',
  '.entries()\n            .next_back()\n            .map(|(_, observation)| observation)',
  'crate::kura::NativeAmxParticipantApplicationObservation::PendingTipMetadata(_)',
  'crate::kura::NativeAmxParticipantApplicationObservation::PendingManifestRepair(_)',
  'crate::kura::NativeAmxParticipantApplicationObservation::PendingReceiptRepair(_)',
  'Some(crate::kura::NativeAmxParticipantApplicationObservation::Applied(',
  'if descriptor.dataspace_id != dataspace_id\n'
  '                    || descriptor.lane_incarnation != lane_incarnation\n'
  '                    || latest_receipt.application_block_height >= proposal_height',
  '} else if matching.is_empty() {',
  'if matching.is_empty() {\n        return Ok(Some((0, None)));\n    }',
  'if hashes.len() > 1 {\n        return Ok(None);\n    }'),
 ('let mut matching = v2_known_lane_tips(state, proposal_height)?',
  '.read_native_amx_participant_application_history(lane_id)\n'
  '            .map_err(crate::state::MergeLedgerCommitError::Persistence)?;',
  '.entries()\n            .next_back()\n            .map(|(_, observation)| observation)',
  'crate::kura::NativeAmxParticipantApplicationObservation::PendingTipMetadata(_)',
  'crate::kura::NativeAmxParticipantApplicationObservation::PendingManifestRepair(_)',
  'crate::kura::NativeAmxParticipantApplicationObservation::PendingReceiptRepair(_)',
  'return Ok(None);',
  'Some(crate::kura::NativeAmxParticipantApplicationObservation::Applied(',
  'if descriptor.dataspace_id != dataspace_id',
  'return Ok(None);',
  '} else if matching.is_empty() {',
  'return Ok(None);',
  'if matching.is_empty() {\n        return Ok(Some((0, None)));\n    }',
  'if hashes.len() > 1')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/sumeragi/v2_lane_work.rs',
  'fn',
  'lane_route_active',
  ('self.state.lane_route_and_incarnation_active_at_height(\n'
   '            lane_id,\n'
   '            dataspace_id,\n'
   '            lane_incarnation,\n'
   '            proposal_height,\n'
   '        )',),
  ()),
 ('SumeragiV2AutoscaleLifecycle', 'crates/iroha_core/src/state.rs', 'fn', 'lane_has_drain_blocking_evidence', ('    pub(crate) fn lane_has_drain_blocking_evidence(\n        &self,\n        lane_id: LaneId,\n        dataspace_id: DataSpaceId,\n        lane_incarnation: Hash,\n    ) -> Result<bool, MergeLedgerCommitError> {\n        let mut releases = LaneLifecycleReleases::new(self);\n        self.lane_has_drain_blocking_evidence_with_releases(\n            lane_id,\n            dataspace_id,\n            lane_incarnation,\n            &mut releases,\n        )\n    }',), ()),
 ('SumeragiV2NativeApplicationEvidence', 'crates/iroha_core/src/state.rs', 'fn', 'native_amx_participant_application_snapshot', ('    fn native_amx_participant_application_snapshot(\n        &self,\n    ) -> Result<NativeAmxParticipantApplicationSnapshot, MergeLedgerCommitError> {\n        let lifecycle = self.lane_consensus_lifecycle_snapshot();\n        self.native_amx_participant_application_snapshot_with_lifecycle(&lifecycle)\n    }',), ()),
 ('SumeragiV2NativeApplicationEvidence', 'crates/iroha_core/src/state.rs', 'fn', 'native_amx_participant_frontiers_pending_durable_evidence_snapshot', ('    pub(crate) fn native_amx_participant_frontiers_pending_durable_evidence_snapshot(\n        &self,\n    ) -> Result<Vec<AppliedNativeAmxParticipantFrontierMarker>, MergeLedgerCommitError> {\n        let lifecycle = self.lane_consensus_lifecycle_snapshot();\n        self.native_amx_participant_frontiers_pending_durable_evidence_snapshot_with_lifecycle(\n            &lifecycle,\n        )\n    }',), ()),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/sumeragi/v2_lane_work/canonical_executed_block_application_repair.rs',
  'fn',
  'plan_lane_application_evidence_repair',
  ('lane_application_certified_repair_snapshot_cached',
   'certified_snapshot.pair_repairs',
   'certified_snapshot.earliest_unapplied',
   'ordinary_pairs.len().saturating_add(ordinary_sessions.len())',
   'preflight_finalized_merge_carrier_repairs',
   'missing_merge_carrier_bodies',
   'into_parts()',
   'planned_merge_entries_by_carrier(&merge_carriers)',
   'preflight_lane_block_application_receipt_repair',
   'LaneBlockApplicationReceiptRepairPreflight::MissingCanonicalBody',
   'canonical_executed_block_need_for_height',
   'collect_lane_application_repair_need',
   'BTreeMap',
   'read_block_body(height)',
   'map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?',
   '.get(&(application_block_height, application_block_hash))',
   'preflight_native_amx_participant_application_evidence_repair',
   'needs.into_values().collect()',
   'LaneApplicationEvidenceRepairPlanning::Ready',
   'merge_carriers',
   'repair_capacity',
   '.native_amx_participant_frontiers_pending_durable_evidence_snapshot()\n'
   '        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;'),
  ()),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/sumeragi/v2_lane_work/canonical_executed_block_application_repair.rs',
  'fn',
  'apply_lane_application_evidence_repair',
  ('state.committed_height() != plan.state_tip_height',
   'state.latest_block_hash_fast() != plan.state_tip_hash',
   'lane_application_certified_repair_snapshot_cached',
   'current_certified.pair_repairs != plan.ordinary_pairs',
   'current_certified.earliest_unapplied',
   'preflight_lane_block_application_receipt_repair',
   'planned_merge_entries_by_carrier(&plan.merge_carriers)',
   'preflight_native_amx_participant_application_evidence_repair',
   'planned_merge_entries',
   'preflight_finalized_merge_carrier_repairs',
   'current_merge_carriers != plan.merge_carriers',
   'for artifact in &plan.ordinary_pairs',
   'persist_committed_lane_block_session_lifecycle_bound',
   'summary.ordinary_pairs',
   'apply_finalized_merge_carrier_repairs',
   'persist_preflighted_lane_block_application_receipt',
   'lane_block_application_receipt_available',
   'repair_native_amx_participant_application_evidence_for_markers',
   '&carrier.markers',
   'repaired_routes != carrier.markers.len()',
   'failed exact readback',
   '.native_amx_participant_frontiers_pending_durable_evidence_snapshot()\n'
   '        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;'),
  ()),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/state.rs',
  'method',
  'State::lane_application_certified_repair_snapshot_cached',
  ('capacity == 0',
   'lane_consensus_lifecycle_snapshot',
   'preflight_latest_certified_lane_block_frontier_with_authority',
   'pair_repair_required',
   'lane_route_and_incarnation_matches',
   'previous_lane_block_height.checked_add(1)',
   'read_certified_lane_block_artifact_read_only',
   'previous_descriptor.lane_incarnation != descriptor.lane_incarnation',
   'traversed_total',
   'earliest.sort_by_key',
   'pair_repairs.sort_by_key',
   'self.certified_lane_block_session_is_applied_or_snapshot_anchored(&session)?',
   'self.certified_lane_block_predecessor_is_applied_or_snapshot_anchored(\n'
   '                    &session.proposal,\n'
   '                )?'),
  ('let _state_write_lock',
   'preflight_latest_certified_lane_block_frontier_with_authority',
   'for mut session in frontiers',
   'self.certified_lane_block_predecessor_is_applied_or_snapshot_anchored(',
   'read_certified_lane_block_artifact_read_only',
   'earliest.sort_by_key',
   'pair_repairs.sort_by_key')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/state.rs',
  'method',
  'State::certified_lane_block_session_is_applied_or_snapshot_anchored',
  ('certified_lane_block_proposal_has_hash_only_snapshot_anchor',
   'if session.prepare_qc.payload_availability_qc.is_some() {',
   'return self.certified_autonomous_lane_block_is_globally_applied(&session.proposal);',
   'self.certified_lane_block_proposal_has_authenticated_application_receipt(\n'
   '                &session.proposal,\n'
   '            )?',
   '.certified_lane_block_proposal_has_hash_only_snapshot_anchor(&session.proposal)?'),
  ('if session.prepare_qc.payload_availability_qc.is_some()',
   'return self.certified_autonomous_lane_block_is_globally_applied',
   'self.certified_lane_block_proposal_has_authenticated_application_receipt(')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/state.rs',
  'method',
  'State::certified_lane_block_predecessor_is_applied_or_snapshot_anchored',
  ('previous_height == 0',
   'return Ok(false)',
   'previous_height.checked_add(1) != Some(descriptor.lane_block_height)',
   'let native_snapshot = self.native_amx_participant_application_snapshot()?;',
   '.contains_key(&(descriptor.lane_id, descriptor.dataspace_id))',
   'let Some(previous_descriptor_hash) = descriptor.previous_lane_block_descriptor_hash else {\n'
   '            return Ok(true);\n'
   '        };',
   '.read_lane_block_artifact_read_only(descriptor.lane_id, previous_height)',
   'artifact.ownership.lane_incarnation == descriptor.lane_incarnation',
   'artifact.ownership.lane_block_descriptor_hash == Some(previous_descriptor_hash)',
   'self.lane_block_predecessor_has_authenticated_receipt(proposal, &native_snapshot)',
   'if native_snapshot\n'
   '            .blocked\n'
   '            .contains_key(&(descriptor.lane_id, descriptor.dataspace_id))\n'
   '        {\n'
   '            return Ok(false);\n'
   '        }'),
  ('let native_snapshot = self.native_amx_participant_application_snapshot()?;',
   '.contains_key(&(descriptor.lane_id, descriptor.dataspace_id))',
   'let Some(previous_descriptor_hash)',
   '.read_lane_block_artifact_read_only(',
   'self.lane_block_predecessor_has_authenticated_receipt(proposal, &native_snapshot)')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/state.rs',
  'fn',
  'certified_lane_block_proposal_has_authenticated_application_receipt',
  ('.read_lane_application_receipt(descriptor.lane_id, descriptor.lane_block_height)',
   '})?;',
   'Ok(receipt.is_some_and(|receipt| receipt.proposal == *proposal))'),
  ()),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/state.rs',
  'fn',
  'certified_lane_block_proposal_has_hash_only_snapshot_anchor',
  ('.read_lane_block_artifact_read_only(descriptor.lane_id, descriptor.lane_block_height)',
   '})?',
   'if !Self::lane_block_artifact_matches_certified_proposal(&artifact, proposal) {\n'
   '            return Ok(false);\n'
   '        }',
   'Ok(self.lane_block_artifact_has_hash_only_snapshot_anchor(&artifact))'),
  ('.read_lane_block_artifact_read_only(',
   'if !Self::lane_block_artifact_matches_certified_proposal',
   'Ok(self.lane_block_artifact_has_hash_only_snapshot_anchor(&artifact))')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/state/autonomous_predecessor_application.rs',
  'fn',
  'lane_block_predecessor_has_authenticated_receipt',
  ('previous_height.checked_add(1) != Some(descriptor.lane_block_height)',
   '.read_lane_application_receipt(descriptor.lane_id, previous_height)',
   '.read_native_amx_participant_application_history(descriptor.lane_id)',
   'Some(crate::kura::NativeAmxParticipantApplicationObservation::Applied(receipt))',
   'previous.lane_incarnation == descriptor.lane_incarnation',
   'previous.descriptor_hash == previous_descriptor_hash',
   'receipt.application_block_height < descriptor.proposal_height'),
  ()),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/state/autonomous_predecessor_application.rs',
  'fn',
  'certified_autonomous_lane_block_is_globally_applied',
  ('Self::canonical_merged_lane_frontier_from_world(',
   '.read_lane_application_receipt(descriptor.lane_id, descriptor.lane_block_height)',
   '})?;',
   'crate::kura::LaneBlockApplicationReceiptArtifactFormat::MergeExecution',
   'receipt.proposal == *proposal'),
  ()),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/sumeragi/tests/v2_lane_work_native_body_recovery.rs',
  'fn',
  'native_participant_missing_carrier_uses_generic_chunk_recovery_then_repairs_receipt',
  ('plan_lane_application_evidence_repair',
   'LaneApplicationEvidenceRepairPlanning::RecoverCanonicalBodies',
   'needs.len()',
   'needs[0].finality_artifact_hash',
   'needs[0].executed_block_wire_hash',
   'kura.cache_block_body',
   'LaneApplicationEvidenceRepairPlanning::Ready',
   'apply_lane_application_evidence_repair',
   'summary.native_carriers',
   'summary.native_routes',
   'read_native_amx_participant_application_receipt',
   'receipt.executed_block_wire_hash',
   'repair_native_amx_participant_application_evidence_for_markers',
   'preflight_native_amx_participant_application_evidence_repair',
   '[drifted_marker]',
   'absent from its authenticated carrier manifest',
   'Some(receipt)',
   'native_amx_participant_frontiers_pending_durable_evidence_snapshot'),
  ()),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/sumeragi/v2_lane_work.rs',
  'method',
  'V2LaneWorkAdapter::ensure_globally_applied_lane_receipts_durable',
  ('lane_application_certified_repair_snapshot_cached(self.limits.session_capacity.get())',
   '!ordinary.pair_repairs.is_empty()',
   '!ordinary.earliest_unapplied.is_empty()',
   '!native.is_empty()',
   'return Err(V2LaneWorkError::Persistence(',
   'not completed by startup repair',
   '.native_amx_participant_frontiers_pending_durable_evidence_snapshot()\n'
   '            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;'),
  ()),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/sumeragi/v2_runner/canonical_recovery_ingress.rs',
  'fn',
  'dispatch_canonical_executed_block_recovery_effects',
  ('recovery.effect_count()',
   'recovery.drain_effects(1)',
   'V2LaneWorkEffect::PostLaneBlock',
   'can_retain_lane_work_effect',
   'dispatch_lane_work_effect',
   'LaneWorkEffectDispatch::Complete',
   'LaneWorkEffectDispatch::SourceRetained',
   'let Some(effect) = recovery.next_effect() else {',
   'if is_request && !is_current_request {',
   'require_peeked_lane_work_effect(recovery.drain_effects(1).pop())?',
   '.can_retain_lane_work_effect(&effect)\n            .map_err(V2RunnerError::Service)?',
   'LaneWorkEffectDispatch::SourceRetained(_) => break,'),
  ('let scan_limit = recovery.effect_count();',
   'let Some(effect) = recovery.next_effect()',
   'if is_request && !is_current_request',
   'continue;',
   '.can_retain_lane_work_effect(&effect)',
   'match dispatch_lane_work_effect(services, effect)?',
   'LaneWorkEffectDispatch::Complete => {',
   'require_peeked_lane_work_effect(recovery.drain_effects(1).pop())?',
   'LaneWorkEffectDispatch::SourceRetained(_) => break,')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/sumeragi/v2_lane_work/canonical_executed_block_application_repair.rs',
  'fn',
  'next_effect',
  ('self.output_guard.acquire()?;', 'self.effects.front().cloned()'),
  ('self.output_guard.acquire()?;', 'self.effects.front().cloned()')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/sumeragi/v2_lane_work/canonical_executed_block_application_repair.rs',
  'fn',
  'is_current_request_effect',
  ('let Some(outstanding) = self.outstanding.as_ref() else {',
   'peer == &outstanding.responder.peer',
   'request.as_ref() == &outstanding.request'),
  ()),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/sumeragi/v2_lane_work.rs',
  'method',
  'V2LaneWorkAdapter::has_pending_historical_recovery',
  ('!self.historical_recovery_sessions.is_empty()',
   '.proposals_without_commit_qc()',
   '.qcs_for_incomplete_sessions()',
   'qc.body.proposal_height < self.context.height',
   'self.historical_proposal_still_needs_recovery(&proposal)?',
   'let Some(proposal) = self.available_proposal_for_vote_body(&qc.body)?'),
  ('.proposals_without_commit_qc()',
   'self.historical_proposal_still_needs_recovery(&proposal)?',
   '.qcs_for_incomplete_sessions()',
   'let Some(proposal) = self.available_proposal_for_vote_body(&qc.body)?')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/sumeragi/v2_lane_work.rs',
  'fn',
  'available_proposal_for_vote_body',
  ('.lane_sessions\n            .proposal_for_vote_body(body)',
   'self.historical_autonomous_recovery_proposal_for_vote_body(body)',
   'return Ok(Some(proposal));',
   'self.canonical_proposal_for_vote_body(body)'),
  ('.lane_sessions\n            .proposal_for_vote_body(body)',
   'self.historical_autonomous_recovery_proposal_for_vote_body(body)',
   'return Ok(Some(proposal));',
   'self.canonical_proposal_for_vote_body(body)')),
 ('SumeragiV2AutonomousReservationCarrier',
  'crates/iroha_core/src/block.rs',
  'fn',
  'validate_autonomous_lane_payload_slot',
  ('latest_certified_lane_block_artifact_matching',
   'latest_lane_block_artifact_matching',
   'block.hash()',
   '.read_current_autonomous_lane_block_artifact(',
   'expected_network_id,',
   'expected_epoch,',
   '.read_lane_completion_certificate(',
   '.read_lane_block_artifact_read_only(',
   '.read_latest_native_amx_participant_application_receipt(',
   'crate::kura::NativeAmxLatestReceiptObservation::PendingTipMetadata(_)',
   'slot_error(&format!("local autonomous slot is unreadable: {error}"))\n                })?',
   'slot_error(&format!("local certified slot is unreadable: {error}"))\n                })?',
   'slot_error(&format!("local Native AMX slot is unreadable: {error}"))\n                })?;',
   'return Err(slot_error("awaits exact Native AMX application metadata"));',
   '&& stored.lane_block_height >= lane_block_height'),
  ('.read_current_autonomous_lane_block_artifact(',
   '.read_lane_completion_certificate(',
   '.latest_certified_lane_block_artifact_matching(',
   '.read_lane_block_artifact_read_only(',
   '.latest_lane_block_artifact_matching(',
   '.read_latest_native_amx_participant_application_receipt(',
   'Ok(exact_current_slot)')),
 ('SumeragiV2QueuePlanAdmissionRegistry',
  'crates/iroha_core/src/sumeragi/v2_lane_work.rs',
  'method',
  'V2LaneWorkAdapter::bind_locked_global_body_from_origin',
  ('!origin_matches || block.header().height().get() != self.context.height',
   'if crate::block::external_queue_plan_synced_entrypoint_index(block).is_some()',
   'retain_pending_certified_merge_entry_for_locked_carrier(',
   'retire_autonomous_payload_batch(&losing_pending)',
   'let canonical_recovery = (|| -> crate::kura::Result<bool> {',
   'Ok(canonical_v2_lane_payload_matches_kura(',
   'let canonical_recovery = match self.consensus_storage_read(canonical_recovery) {'),
  ('if !origin_matches || block.header().height().get() != self.context.height',
   'external_queue_plan_synced_entrypoint_index(block).is_some()',
   'return V2LaneIngressOutcome::Rejected;',
   'let canonical_recovery = (|| -> crate::kura::Result<bool> {',
   'let canonical_recovery = match self.consensus_storage_read(canonical_recovery) {',
   'retain_pending_certified_merge_entry_for_locked_carrier(',
   'retire_autonomous_payload_batch(&losing_pending)')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/kura/consensus_storage_reads.rs',
  'fn',
  'read_lane_completion_certificate',
  ('if self.emergency_fast_startup_enabled()',
   'return Err(Error::EmergencyFastAuxiliaryUnavailable',
   'self.ensure_prune_recovery_not_required()?;',
   'self.read_certified_lane_block_artifact_read_only_under_prune_and_canonical_guards(\n'
   '            lane_id,\n'
   '            lane_block_height,\n'
   '            true,\n'
   '        )'),
  ('self.prune_lock.lock()',
   'self.ensure_prune_recovery_not_required()?;',
   'self.canonical_chain_lock.lock()',
   'self.read_certified_lane_block_artifact_read_only_under_prune_and_canonical_guards(')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/kura/consensus_storage_reads.rs',
  'fn',
  'read_current_autonomous_lane_block_artifact',
  ('self.ensure_prune_recovery_not_required()?;',
   'self.read_current_autonomous_lane_block_artifact_under_guards(\n'
   '            lane_id,\n'
   '            lane_block_height,\n'
   '            network_id,\n'
   '            epoch,\n'
   '        )'),
  ('self.prune_lock.lock()',
   'self.ensure_prune_recovery_not_required()?;',
   'self.canonical_chain_lock.lock()',
   'self.read_current_autonomous_lane_block_artifact_under_guards(')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/kura/consensus_storage_reads.rs',
  'fn',
  'read_current_autonomous_lane_block_artifact_under_guards',
  ('pointer.lane_id != lane_id',
   'pointer.dataspace_id != entry.dataspace_id',
   'pointer.lane_block_height != lane_block_height',
   'pointer.lane_incarnation != marker.0',
   'pointer.proposal_height <= marker.1',
   'AutonomousLaneBlockViewStateReadMode::LatestReadOnly',
   'record.retirement.is_none().then_some(record.artifact)',
   'if self.active_lane_incarnation_marker(&entry)? != marker',
   ')? != bytes'),
  ('self.lane_geometry_lock.lock()',
   'self.active_lane_incarnation_marker(&entry)?',
   'self.sidecar_lock.lock()',
   'self.read_regular_sidecar_bytes(',
   'Self::decode_autonomous_lane_block_latest_attempt(&path, bytes)?',
   'AutonomousLaneBlockViewStateReadMode::LatestReadOnly',
   'if self.active_lane_incarnation_marker(&entry)? != marker',
   'Ok(artifact)')),
 ('SumeragiV2NativeApplicationEvidence',
  'crates/iroha_core/src/kura.rs',
  'fn',
  'read_latest_native_amx_participant_application_receipt',
  ('let mut history = self.read_native_amx_participant_application_history(lane_id)?;',
   '.pop_last()',
   'None => NativeAmxLatestReceiptObservation::Absent,',
   'NativeAmxLatestReceiptObservation::PendingTipMetadata(receipt)',
   'Some(NativeAmxParticipantApplicationObservation::PendingManifestRepair(_))',
   'Some(NativeAmxParticipantApplicationObservation::PendingReceiptRepair(_))',
   'return Err(Self::invalid_lane_artifact_error('),
  ()),
 ('SumeragiV2NativeApplicationEvidence',
 'crates/iroha_core/src/kura.rs',
 'fn',
 'read_native_amx_participant_application_history_with_lease',
 ('if lease.is_some_and(|lease| !lease.belongs_to(self)) {',
  "lease: Option<&KuraPublicationLease<'_>>",
  'lease.is_none().then(|| self.prune_lock.lock())',
  'lease.is_none().then(|| self.canonical_chain_lock.lock())',
  'lease.is_none().then(|| self.lane_geometry_lock.lock())',
  'lease.is_none().then(|| self.sidecar_lock.lock())',
  'self.ensure_prune_recovery_not_required()?;',
  'self.require_native_amx_evidence_prune_intent_absent_locked(&namespace)?;',
  'self.require_native_amx_latest_index_temp_absent_locked(&namespace)?;',
  'self.inventory_native_amx_evidence_files_locked(&namespace, true)?',
  'if !inventory.temporaries.is_empty()',
  'Self::validate_native_amx_retained_history_continuity(',
  'self.read_block_body_under_prune_and_canonical_guards(*height)?;',
  '|| self.active_lane_incarnation_marker(&current)? != marker',
  '|| !Self::progress_mutation_namespace_unchanged(&namespace)',
  '|| u64::try_from(self.exact_durable_blocks_count()?)? != exact_tip',
  'if !confirmed_inventory.temporaries.is_empty()',
  'if !Self::stable_sidecar_metadata_unchanged(metadata, &confirmed)',
  'if retained_manifests.get(height) != Some(&confirmed)',
  'if retained_receipts.get(height) != Some(&confirmed)',
  'if metadata.get(height) != Some(&confirmed)',
  'Ok(NativeAmxParticipantApplicationHistory {'),
 ('if lease.is_some_and(|lease| !lease.belongs_to(self)) {',
  'lease.is_none().then(|| self.prune_lock.lock())',
  'lease.is_none().then(|| self.canonical_chain_lock.lock())',
  'lease.is_none().then(|| self.lane_geometry_lock.lock())',
  'lease.is_none().then(|| self.sidecar_lock.lock())',
  'self.inventory_native_amx_evidence_files_locked(&namespace, true)?',
  'drop(sidecar);',
  'drop(geometry);',
  'self.read_block_body_under_prune_and_canonical_guards(*height)?;',
  'let geometry = lease.is_none().then(|| self.lane_geometry_lock.lock());',
  'if !confirmed_inventory.temporaries.is_empty()',
  'if retained_manifests.get(height) != Some(&confirmed)',
  'if retained_receipts.get(height) != Some(&confirmed)',
  'if metadata.get(height) != Some(&confirmed)',
  'drop(sidecar);',
  'drop(geometry);',
  'self.read_block_body_under_prune_and_canonical_guards(*height)?;',
  'Ok(NativeAmxParticipantApplicationHistory {')),
 ('SumeragiV2NativeApplicationEvidence',
 'crates/iroha_core/src/kura.rs',
 'fn',
 'read_native_amx_participant_application_history',
 ('self.read_native_amx_participant_application_history_with_lease(lane_id, None)',),
 ('self.read_native_amx_participant_application_history_with_lease(lane_id, None)',)))


# The actual typed-error and generation/post-generation owners replace the
# former boolean fallback and monolithic lifecycle helper.
AUTHORITY_RECOVERY_BINDINGS += (('SumeragiV2AutoscaleLifecycle',
 'crates/iroha_core/src/sumeragi/v2_lane_work.rs',
 'fn',
 'lane_drain_has_local_blockers',
 ('lane_has_drain_blocking_evidence',
  'lane_has_pending_work',
  'pending_autonomous_anchor_payloads',
  'native_sessions.has_pending_votes_for_lane',
  'PendingMergeStage::Certified',
  'let Some(queue) = self.lane_drain_queue.as_ref() else {\n            return true;\n        };',
  '.lane_has_drain_blocking_evidence(',
  '.unwrap_or(true)',
  '|| queue.lane_has_pending_work(',
  '|| self.lane_sessions.has_undrained_work_for_lane('),
 ()),
 ('SumeragiV2AutoscaleLifecycle',
  'crates/iroha_core/src/state.rs',
  'fn',
  'validate_merge_lane_drain_certificate_payload',
  ('self.lane_has_drain_blocking_evidence(',
   '.map_err(|error| MergeLedgerCommitError::LocalDrainObservation(Box::new(error)))?',
   'Self::queue_plan_pending_route_obligation_blocks_lane_drain_in_world(',
   'if blocked {\n            return Err(MergeLedgerCommitError::ExecutionBatchInvalid('),
  ()),
 ('SumeragiV2AutoscaleLifecycle', 'crates/iroha_core/src/state.rs', 'fn', 'validate_committed_autoscale_lane_lifecycle', ('commitment.carrier_height >= block_height', '.lane_has_drain_blocking_evidence_with_releases(', '.map_err(LaneLifecycleError::DrainObservation)?', 'frontier != commitment.frontier'), ()),
 ('SumeragiV2AutoscaleLifecycle',
  'crates/iroha_core/src/block.rs',
  'method',
  'BlockValidationError::from_autoscale_lifecycle_error',
  ('LaneLifecycleError::DrainObservation(_)', 'Self::LocalStorageRecoveryRequired { reason }'),
  ()),
 ('SumeragiV2AutoscaleLifecycle',
  'crates/iroha_core/src/block.rs',
  'method',
  'BlockValidationError::from_certified_merge_stage_error',
  ('| MergeLedgerCommitError::LocalDrainObservation(_)', 'Self::LocalStorageRecoveryRequired {'),
  ()),
 ('SumeragiV2AutoscaleLifecycle',
  'crates/iroha_core/src/sumeragi/v2_apply.rs',
  'method',
  'V2ApplyService::classify_candidate_validation_error',
  ('if let BlockValidationError::LocalStorageRecoveryRequired { reason } = error {',
   'super::v2_body_store::LocalValidationRefusal::RecoveryRequired(reason.clone())',
   'return V2ApplyError::LocalValidation('),
  ()),
 ('SumeragiV2AutoscaleLifecycle',
  'crates/iroha_core/src/sumeragi/v2_apply.rs',
  'method',
  'V2ApplyService::classify_lane_lifecycle_validation_error',
  ('| crate::state::LaneLifecycleError::DrainObservation(_)',
   'super::v2_body_store::LocalValidationRefusal::RecoveryRequired(',
   'V2ApplyError::LocalValidation('),
  ()),
 ('SumeragiV2AutoscaleLifecycle',
  'crates/iroha_core/src/sumeragi/v2_apply/error_recovery.rs',
  'fn',
  'rejection_identity',
  ('Self::LocalValidation(_)', '=> None,'),
  ()),
 ('SumeragiV2AutoscaleLifecycle',
  'crates/iroha_core/src/state/carrier_lifecycle_effects.rs',
  'struct',
  'PreparedLaneLifecycleEffects',
  ('manifests: LaneManifestRegistryHandle',
   'privacy: LanePrivacyRegistryHandle',
   'lanes_to_reset: BTreeSet<LaneId>',
   'active_reset_lanes: BTreeSet<LaneId>',
   'lane_config: iroha_config::parameters::actual::LaneConfig',
   'transition_height: u64'),
  ()),
 ('SumeragiV2AutoscaleLifecycle',
  'crates/iroha_core/src/state/carrier_lifecycle_effects.rs',
  'method',
  'PreparedLaneLifecycleEffects::prepare',
  ('manifests: Arc::clone(&pending.updated_lane_manifests)',
   'privacy: Arc::new(LanePrivacyRegistry::from_manifest_registry(',
   '&pending.updated_lane_manifests,',
   'lanes_to_reset: pending.catalog_update.lanes_to_reset.clone()',
   'active_reset_lanes: State::active_reset_lanes(',
   '&pending.catalog_update.lanes_to_reset,',
   '&pending.catalog_update.updated_lane_config,',
   'lane_config: pending.catalog_update.updated_lane_config.clone()',
   'transition: pending.transition.clone()',
   'transition_height: pending.transition_height'),
  ()),
 ('SumeragiV2AutoscaleLifecycle', 'crates/iroha_core/src/state/carrier_lifecycle_effects.rs', 'method', 'PreparedLaneLifecycleEffects::publish', ('self,', "indexes: &mut effect_publication::StateEffectLocks<'_>", "_publication: &StateViewGenerationWriteGuard<'_>", 'indexes.install_registries(self.manifests, self.privacy);', '        if !self.lanes_to_reset.is_empty() {\n            indexes\n                .merge_admission\n                .as_mut()\n                .expect("prepared merge admission")\n                .prune_lane_progress(&self.lanes_to_reset);\n            indexes\n                .lane_relays\n                .as_mut()\n                .expect("prepared relays")\n                .prune_lanes(&self.lanes_to_reset);\n            indexes\n                .da_commitments\n                .as_mut()\n                .expect("prepared commitments")\n                .prune_lanes(&self.lanes_to_reset);\n            indexes\n                .da_confidential_compute\n                .as_mut()\n                .expect("prepared confidential compute")\n                .prune_lanes(&self.lanes_to_reset);\n            indexes\n                .da_pin_intents\n                .as_mut()\n                .expect("prepared pins")\n                .prune_lanes(&self.lanes_to_reset);\n            indexes\n                .da_receipt_cursors\n                .as_mut()\n                .expect("prepared receipt cursors")\n                .prune_lanes(&self.lanes_to_reset);\n            indexes\n                .da_shard_cursors\n                .as_mut()\n                .expect("prepared shard cursors")\n                .prune_lanes(&self.lanes_to_reset);\n        }\n', 'if publish_process_runtime {', 'state.publish_lane_scoped_runtime_reset(&self.lanes_to_reset);', 'let records_reset = !self.active_reset_lanes.is_empty() && self.transition_height != 0;', '        if records_reset {\n            indexes\n                .da_shard_cursors\n                .as_mut()\n                .expect("prepared shard cursors")\n                .mark_lanes_canonically_reset(&self.active_reset_lanes, self.transition_height);\n        }\n', '        let persist_cursor_journal = publish_process_runtime\n            && (records_reset\n                || (!self.lanes_to_reset.is_empty()\n                    && indexes\n                        .da_indexes_hydrated\n                        .as_ref()\n                        .expect("prepared hydration status")\n                        .is_some()));\n', 'LaneLifecyclePostPublication {', 'lane_config: self.lane_config,', 'persist_cursor_journal,', 'snapshot: None,', 'captured: false,'), ('indexes.install_registries(', '        if !self.lanes_to_reset.is_empty() {\n            indexes\n                .merge_admission\n                .as_mut()\n                .expect("prepared merge admission")\n                .prune_lane_progress(&self.lanes_to_reset);\n            indexes\n                .lane_relays\n                .as_mut()\n                .expect("prepared relays")\n                .prune_lanes(&self.lanes_to_reset);\n            indexes\n                .da_commitments\n                .as_mut()\n                .expect("prepared commitments")\n                .prune_lanes(&self.lanes_to_reset);\n            indexes\n                .da_confidential_compute\n                .as_mut()\n                .expect("prepared confidential compute")\n                .prune_lanes(&self.lanes_to_reset);\n            indexes\n                .da_pin_intents\n                .as_mut()\n                .expect("prepared pins")\n                .prune_lanes(&self.lanes_to_reset);\n            indexes\n                .da_receipt_cursors\n                .as_mut()\n                .expect("prepared receipt cursors")\n                .prune_lanes(&self.lanes_to_reset);\n            indexes\n                .da_shard_cursors\n                .as_mut()\n                .expect("prepared shard cursors")\n                .prune_lanes(&self.lanes_to_reset);\n        }\n', 'state.publish_lane_scoped_runtime_reset(', '.mark_lanes_canonically_reset(', 'LaneLifecyclePostPublication {')),
 ('SumeragiV2AutoscaleLifecycle', 'crates/iroha_core/src/state/carrier_lifecycle_effects.rs', 'method', 'LaneLifecyclePostPublication::publish', ('fn publish(self, state: &State)', 'self.captured', 'if let Some(snapshot) = self.snapshot', 'snapshot.persist()', 'self.transition.log(self.transition_height);'), ()),
 ('SumeragiV2AutoscaleLifecycle',
  'crates/iroha_core/src/state.rs',
  'fn',
  'active_reset_lanes',
  ('.filter(|lane_id| lane_config.entry(*lane_id).is_some())',),
  ()),
 ('SumeragiV2AutoscaleLifecycle', 'crates/iroha_core/src/state.rs', 'fn', 'install_prepared_lane_manifests_in_publication', ("_publication: &StateViewGenerationWriteGuard<'_>", '*guard = manifests;', '*privacy_guard = privacy;', "releases: &mut LaneLifecycleReleases<'_>", 'let mut guard = releases.manifests.write();', 'let mut privacy_guard = releases.privacy.write();'), ()),
 ('SumeragiV2AutoscaleLifecycle', 'crates/iroha_core/src/state.rs', 'fn', 'reset_lane_scoped_runtime_indexes', ('self.prune_merge_admission_lane_progress(lanes_to_reset, releases);', 'releases.relays.write().prune_lanes(lanes_to_reset);', 'releases.commitments.write().prune_lanes(lanes_to_reset);', 'releases\n            .confidential_compute\n            .write()\n            .prune_lanes(lanes_to_reset);', 'releases.pin_intents.write().prune_lanes(lanes_to_reset);', 'releases.receipt_cursors.write().prune_lanes(lanes_to_reset);', 'let mut cursors = releases.shard_cursors.write();', 'cursors.prune_lanes(lanes_to_reset);'), ()),
 ('SumeragiV2AutoscaleLifecycle',
  'crates/iroha_core/src/state.rs',
  'fn',
  'persist_da_shard_cursor_journal_with_config',
  ('let cursors = self.da_shard_cursors.read().clone();',
   'DaShardCursorJournal::from_index(lane_config, &cursors, &path)',
   'snapshot.persist()'),
  ()),
 ('SumeragiV2AutoscaleLifecycle', 'crates/iroha_core/src/state.rs', 'method', 'StateBlock::commit_inner', ('let _state_commit_lock = commit_fence.lock();', 'Some(lifecycle_fence.lock())', 'carrier_lifecycle_effects::PreparedLaneLifecycleEffects::prepare(pending, &nexus)', 'let _view_generation = publication_notice.begin();', 'canonical_runtime.publish_prepared();', 'Some(prepared.publish(state_ref, &mut effect_locks, &_view_generation, !*replay_prevalidation))', 'if let Some(post) = lifecycle_post_publication.take() {\n            post.publish(state_ref);\n        }', 'drop(autoscale_lifecycle_guard);', 'effect_locks.prepare_blocking();', 'effect_locks.release_writers();'), ('let mut effect_cleanup =', 'let mut lifecycle_post_publication = None;', 'let mut commit_fence =', 'let mut write_fence =', 'let mut lifecycle_fence =', 'hash_budget.with_deferred_refund_notifications(', 'let mut this = self;', 'let _state_commit_lock =', 'Some(lifecycle_fence.lock())', 'carrier_lifecycle_effects::PreparedLaneLifecycleEffects::prepare(', 'canonical_runtime.publish_prepared();', 'world.publish_prepared();', 'block_hashes.publish_prepared();', 'Some(prepared.publish(state_ref, &mut effect_locks, &_view_generation, !*replay_prevalidation))', 'if let Some(post) = lifecycle_post_publication.take() {', 'drop(autoscale_lifecycle_guard);', 'drop(_state_commit_lock);')))

AUTHORITY_RECOVERY_BINDINGS += (
    ('SumeragiV2AutoscaleLifecycle', 'crates/iroha_core/src/state/carrier_lifecycle_effects.rs', 'method', 'LaneLifecyclePostPublication::capture_snapshot', ('!self.captured', 'self.captured = true;', 'if self.persist_cursor_journal {', 'state.da_shard_cursor_journal_path()', 'if !path.as_os_str().is_empty()', 'DaShardCursorJournal::from_index(\n                    &self.lane_config,\n                    cursors,\n                    &path,\n                )'), ('!self.captured', 'self.captured = true;', 'if self.persist_cursor_journal {', 'DaShardCursorJournal::from_index(')),
)

# Original wrappers stay bound; these are their sole defining evidence kernels.
AUTHORITY_RECOVERY_BINDINGS += (
    ('SumeragiV2AutoscaleLifecycle', 'crates/iroha_core/src/state.rs', 'fn', 'lane_has_drain_blocking_evidence_with_releases', ('Result<bool, MergeLedgerCommitError>', '.unmerged_merge_admissible_relay_progress_with_releases(lane_id, dataspace_id, releases)', '.unapplied_lane_block_artifact_heights_snapshot_cached_with_lifecycle(&lifecycle)?', '.unapplied_certified_lane_block_heights_snapshot_cached_with_lifecycle(&lifecycle)?', '.native_amx_participant_frontiers_pending_durable_evidence_snapshot_with_lifecycle(\n                    &lifecycle,\n                )?', 'marker.lane_id == lane_id', 'marker.dataspace_id == dataspace_id', 'marker.lane_incarnation == lane_incarnation', 'self.pending_queue_plan_admission_blocks_lane_drain_with_releases(', 'self.queue_plan_pending_route_obligation_blocks_lane_drain(', 'self.kura.pending_certified_merge_work_for_lane('), ()),
    ('SumeragiV2NativeApplicationEvidence', 'crates/iroha_core/src/state.rs', 'fn', 'native_amx_participant_application_snapshot_with_lifecycle', ('for lane in lifecycle.nexus.lane_catalog.lanes()', '.read_native_amx_participant_application_history(lane.id),', '.map_err(MergeLedgerCommitError::Persistence)?;', 'Self::canonical_native_amx_participant_frontier_from_world(', 'Self::validate_native_amx_participant_shared_frontier(&world, &marker)?;', 'observed_lane != lane.id', 'observed_dataspace != lane.dataspace_id', 'observed_incarnation != incarnation', 'if !marker_matches {', ') || marker.is_none_or(|marker| height > marker.lane_block_height)', 'snapshot.applied_slots.insert((', 'snapshot.repair.push(marker);', 'Some(NativeAmxParticipantApplicationObservation::Applied(_))', '                if !matches!(\n                    observation,\n                    NativeAmxParticipantApplicationObservation::Applied(_)\n                ) || marker.is_none_or(|marker| height > marker.lane_block_height)\n                {\n                    snapshot\n                        .blocked\n                        .entry(route)\n                        .and_modify(|blocked| *blocked = (*blocked).max(height))\n                        .or_insert(height);\n                } else {\n                    snapshot.applied_slots.insert((\n                        lane.id,\n                        lane.dataspace_id,\n                        incarnation,\n                        height,\n                    ));\n                }', '            if matches!(\n                history.get(marker.lane_block_height),\n                Some(NativeAmxParticipantApplicationObservation::Applied(_))\n            ) {\n                snapshot.applied.push(marker);\n            } else {\n                snapshot.repair.push(marker);\n                snapshot\n                    .blocked\n                    .entry(route)\n                    .and_modify(|blocked| *blocked = (*blocked).max(marker.lane_block_height))\n                    .or_insert(marker.lane_block_height);\n            }'), ('for lane in lifecycle.nexus.lane_catalog.lanes()', '.read_native_amx_participant_application_history(lane.id),', 'Self::canonical_native_amx_participant_frontier_from_world(', 'for (height, observation) in history.entries()', 'if !marker_matches {', 'snapshot.applied_slots.insert((', 'let Some(marker) = marker else {', 'snapshot.repair.push(marker);', 'Ok(snapshot)')),
    ('SumeragiV2NativeApplicationEvidence', 'crates/iroha_core/src/state.rs', 'fn', 'native_amx_participant_frontiers_pending_durable_evidence_snapshot_with_lifecycle', ('Ok(self\n            .native_amx_participant_application_snapshot_with_lifecycle(lifecycle)?\n            .repair)',), ()),
)

def validate_authority_recovery_item(item: str, binding: tuple, errors: list[str]) -> None:
    """Require the reviewed predicates and their authority-before-use order."""
    _module, path, _kind, symbol, required, ordered = binding
    for token in required:
        if token not in item:
            errors.append(f"{path}: authority/recovery item {symbol} is missing token {token!r}")
    cursor = 0
    for token in ordered:
        index = item.find(token, cursor)
        if index < 0:
            errors.append(f"{path}: authority/recovery item {symbol} violates order at {token!r}")
            break
        cursor = index + len(token)


    code = _code(item)
    def require(relation: str) -> None:
        if _code(relation) not in code:
            errors.append(f"authority/recovery item {symbol} loses executable relation {relation!r}")

    if symbol == "lane_has_drain_blocking_evidence_with_releases":
        require("self.pending_queue_plan_admission_blocks_lane_drain_with_releases(lane_id, dataspace_id, lane_incarnation, releases,)?")
        for call in ("self.queue_plan_pending_route_obligation_blocks_lane_drain",
                     "self.kura.pending_certified_merge_work_for_lane"):
            require(f"{call}(lane_id, dataspace_id, lane_incarnation,)?")
        if any(token in code for token in (".unwrap_or(", ".unwrap_or_default(", ".map_or(")):
            errors.append("authority/recovery item drain observation must preserve typed local failure")
    if symbol == "lane_drain_has_local_blockers":
        require(""".lane_has_drain_blocking_evidence(
            intent.lane_id, intent.dataspace_id, intent.lane_incarnation,
        ).unwrap_or(true) || queue.lane_has_pending_work(""")
    if symbol == "validate_merge_lane_drain_certificate_payload":
        require("""Self::evidence_aware_lane_drain_frontier_from_world(
            &self.world.view(), &self.kura,
            intent.lane_id, intent.dataspace_id, intent.lane_incarnation,
        ).map_err(|error| MergeLedgerCommitError::LocalDrainObservation(Box::new(error)))?""")
        require("""self.lane_has_drain_blocking_evidence(
            intent.lane_id, intent.dataspace_id, intent.lane_incarnation,
        ).map_err(|error| MergeLedgerCommitError::LocalDrainObservation(Box::new(error)))?""")
    if symbol == "validate_committed_autoscale_lane_lifecycle":
        require("""Self::evidence_aware_lane_drain_frontier_from_world(
            &self.world.view(), &self.kura, *lane, previous_lane.dataspace_id, incarnation,
        ).map_err(LaneLifecycleError::DrainObservation)?""")
        require(""".lane_has_drain_blocking_evidence_with_releases(
            *lane, previous_lane.dataspace_id, incarnation, releases,
        ).map_err(LaneLifecycleError::DrainObservation)?""")
    if symbol == "BlockValidationError::from_autoscale_lifecycle_error":
        require("""LaneLifecycleError::DrainObservation(_)
            | LaneLifecycleError::Storage(_)
            | LaneLifecycleError::GeometryStorage(_)
            | LaneLifecycleError::PublicationBusy { .. } => {
                Self::LocalStorageRecoveryRequired { reason }
            }""")
    if symbol == "BlockValidationError::from_certified_merge_stage_error":
        require("""local @ (MergeLedgerCommitError::Persistence(_)
            | MergeLedgerCommitError::LocalDrainObservation(_)) => {
                Self::LocalStorageRecoveryRequired {
                    reason: format!("certified merge entry could not be staged: {local}"),
                }
            }""")
    if symbol == "V2ApplyService::classify_candidate_validation_error":
        require("""if let BlockValidationError::LocalStorageRecoveryRequired { reason } = error {
            return V2ApplyError::LocalValidation(
                super::v2_body_store::LocalValidationRefusal::RecoveryRequired(reason.clone()),
            );
        }""")
    if symbol == "V2ApplyService::classify_lane_lifecycle_validation_error":
        require("""crate::state::LaneLifecycleError::Storage(_)
            | crate::state::LaneLifecycleError::GeometryStorage(_)
            | crate::state::LaneLifecycleError::DrainObservation(_)
            | crate::state::LaneLifecycleError::PublicationBusy { .. } => {
                V2ApplyError::LocalValidation(
                    super::v2_body_store::LocalValidationRefusal::RecoveryRequired(
                        error.to_string(),
                    ),
                )
            }""")
    if symbol == "rejection_identity":
        # The exhaustive arm may include other local variants, but the actual
        # LocalValidation arm must return None, never a persisted rejection id.
        local = code.find(_code("Self::LocalValidation(_)"))
        arm = code.find("=>", local) if local >= 0 else -1
        if arm < 0 or not code[arm + 2:].startswith("None,"):
            errors.append("authority/recovery item local drain failure cannot acquire rejection identity")
    if symbol == "PreparedLaneLifecycleEffects::publish":
        require("""if publish_process_runtime {
            state.publish_lane_scoped_runtime_reset(&self.lanes_to_reset);
        }""")
        require('        if records_reset {\n            indexes\n                .da_shard_cursors\n                .as_mut()\n                .expect("prepared shard cursors")\n                .mark_lanes_canonically_reset(&self.active_reset_lanes, self.transition_height);\n        }\n')
        require('        let persist_cursor_journal = publish_process_runtime\n            && (records_reset\n                || (!self.lanes_to_reset.is_empty()\n                    && indexes\n                        .da_indexes_hydrated\n                        .as_ref()\n                        .expect("prepared hydration status")\n                        .is_some()));\n')
        if "persist_da_shard_cursor_journal" in code or ".write()" in code or ".read()" in code:
            errors.append("authority/recovery item lifecycle must use acquired indexes and defer disk work")
    if symbol == "LaneLifecyclePostPublication::capture_snapshot":
        require("if self.persist_cursor_journal {")
        require("DaShardCursorJournal::from_index(&self.lane_config, cursors, &path)")
    if symbol == "LaneLifecyclePostPublication::publish":
        require("if let Some(snapshot) = self.snapshot {")
        require("snapshot.persist()")
        if any(token in code for token in ("nexus_snapshot(", "reset_lane_scoped_runtime", "mark_lanes_canonically_reset(", ".read()", ".write()", "persist_da_shard_cursor_journal_with_config")):
            errors.append("authority/recovery item lifecycle post work retargets or repeats published reset")
    if symbol == "StateBlock::commit_inner":
        # Locate the actual final publication block after geometry admission.
        # The preflight membership writer is a separate earlier lexical scope.
        opening = code.find(_code("let autoscale_storage_hold ="))
        first = code.find(_code("{ let state_write_lock_wait_start = Instant::now(); let _state_write_lock = write_fence.lock();"), opening)
        depth = 0
        last = -1
        if first >= 0:
            for index in range(first, len(code)):
                depth += (code[index] == "{") - (code[index] == "}")
                if depth == 0:
                    last = index
                    break
        publication = code[first:last + 1] if last >= 0 else ""
        operations = tuple(map(_code, (
            "let _state_write_lock = write_fence.lock();",
            "block_hashes.try_prepare_publication().map_err(|_| { TransactionsBlockError::SnapshotObservationChanged })?;",
            "world.prepare_publication();", "canonical_runtime.prepare_publication();",
            "lane_consensus_contexts.prepare_publication();", "prev_committed_topology.prepare_publication();",
            "committed_topology.prepare_publication();", "effect_locks.prepare_blocking();", "let _view_generation = publication_notice.begin();",
            "transactions.publish_prepared();", "canonical_runtime.publish_prepared();",
            "world.publish_prepared();", "block_hashes.publish_prepared();",
            "Some(prepared.publish(state_ref, &mut effect_locks, &_view_generation, !*replay_prevalidation))",
            "Some(effects.publish(state_ref, &mut effect_locks, &_view_generation, !*replay_prevalidation))",
        )))
        positions = [publication.find(operation) for operation in operations]
        post = _code("if let Some(post) = lifecycle_post_publication.take() { post.publish(state_ref); }")
        if (not publication or any(index < 0 for index in positions)
                or positions != sorted(positions) or post in publication
                or not code[last + 1:].startswith(_code("""if let Some(post) = lifecycle_post_publication.as_mut() {
                    post.capture_snapshot(state_ref, effect_locks.da_shard_cursors.as_ref().expect("prepared shard cursors"));
                }
                if let Some(post) = da_post_publication.as_mut() {
                    post.capture_snapshot(state_ref, effect_locks.da_shard_cursors.as_ref().expect("prepared shard cursors"));
                }
                effect_locks.release_writers();""") + post)
                or "drop(_view_generation)" in publication
                or "drop(_state_write_lock)" in publication
                or "drop(_state_commit_lock)" in publication
                or code.count("drop(_state_commit_lock)") != 1
                or any(_code(token) in code for token in (
                    "drop(this)", "this.into_fields()", "block_hashes.release_writers()",
                    "drop(write_fence)", "drop(commit_fence)", "drop(lifecycle_fence)",
                ))
                or not 0 <= code.find("letmutwrite_fence=") < code.find("letmutthis=self;") < code.find("let_state_commit_lock=")):
            errors.append("authority/recovery item lifecycle publication loses generation/outer-fence ordering")
    if symbol == "read_native_amx_participant_application_history":
        # The existing caller owns the ordinary locking path. A successful
        # fallback, altered lane, or caller-supplied lease cannot replace it.
        exact_return = """{
            self.read_native_amx_participant_application_history_with_lease(lane_id, None)
        }"""
        if not code.endswith(_code(exact_return)):
            errors.append("authority/recovery item history wrapper must return the exact unleased delegate")
    if symbol == "read_native_amx_participant_application_history_with_lease":
        foreign_lease = """if lease.is_some_and(|lease| !lease.belongs_to(self)) {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(), "Native history lease belongs to another Kura",
            ));
        }"""
        if _code(foreign_lease) not in code:
            errors.append("authority/recovery item history delegate must reject foreign lease before observation")
        # Each existing physical probe is conditional on the same lease, also
        # after the body-read gap. No unconditional relock may hide alongside
        # one still-present required token.
        for mutex, count in (("prune_lock", 1), ("canonical_chain_lock", 1),
                             ("lane_geometry_lock", 2), ("sidecar_lock", 2)):
            probe = f"self.{mutex}.lock()"
            owned = f"lease.is_none().then(|| {probe})"
            if code.count(_code(probe)) != count or code.count(_code(owned)) != count:
                errors.append(f"authority/recovery item history delegate must lease-guard every {mutex} probe")
    if symbol == "v2_known_lane_tip_for_route":
        # Every authenticated occupied but incomplete highest slot must abstain;
        # no pending repair may be filtered out to expose an older Applied slot.
        pending = """Some(
            crate::kura::NativeAmxParticipantApplicationObservation::PendingTipMetadata(_)
            | crate::kura::NativeAmxParticipantApplicationObservation::PendingManifestRepair(_)
            | crate::kura::NativeAmxParticipantApplicationObservation::PendingReceiptRepair(_),
        ) => { return Ok(None); }"""
        current = """if descriptor.dataspace_id != dataspace_id
            || descriptor.lane_incarnation != lane_incarnation
            || latest_receipt.application_block_height >= proposal_height
            { return Ok(None); } matching.push(LaneBlockTip {"""
        for predicate in (pending, current):
            if _code(predicate) not in code:
                errors.append("authority/recovery item Native lane tip must abstain on the highest pending, stale or future application")
        if code.count(_code("read_native_amx_participant_application_history(")) != 1:
            errors.append("authority/recovery item Native lane tip must read one authenticated complete history")
    if symbol == "V2LaneWorkAdapter::bind_locked_global_body_from_origin":
        # The storage failure may be logged, but cannot become a successful
        # canonical match or reach any retention/reservation mutation.
        rejection = """let canonical_recovery = match self.consensus_storage_read(canonical_recovery) {
            Ok(recovered) => recovered,
            Err(error) => {
                if empty_merge_body {
                    iroha_logger::warn!(?subject, ?error,
                        "rejected empty merge carrier after canonical Kura read");
                }
                return V2LaneIngressOutcome::Rejected;
            }
        };"""
        if _code(rejection) not in code:
            errors.append("authority/recovery item canonical recovery storage failure must reject before mutation")


def validate_authority_recovery_contract(
    root: Path, models: Any, errors: list[str], rust_binding_item: Callable,
) -> None:
    """Bind exact declared current owners; no cached-name or token translations."""
    if not isinstance(models, list):
        errors.append("authority/recovery models must be an array")
        return
    for binding in AUTHORITY_RECOVERY_BINDINGS:
        module, path, kind, symbol, required, _ordered = binding
        matching = [m for m in models if isinstance(m, dict) and m.get("module") == module]
        if len(matching) != 1:
            errors.append(f"authority/recovery requires exactly one model {module}")
            continue
        rows = matching[0].get("production_symbols")
        if not isinstance(rows, list):
            errors.append(f"{module}: authority/recovery production symbols must be an array")
            continue
        found = [r for r in rows if isinstance(r, dict) and r.get("path") == path
                 and r.get("kind") == kind and r.get("symbol") == symbol]
        if len(found) != 1 or found[0].get("required_tokens") != list(required):
            errors.append(f"{module}: authority/recovery binding changed for {path}!{symbol}")
        item = rust_binding_item(root, path, kind, symbol, "authority/recovery production binding", errors)
        if item is not None:
            validate_authority_recovery_item(item, binding, errors)
