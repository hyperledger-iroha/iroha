"""Exact State wrapper/helper source joins for autonomous merge and lane drain.

These structural checks preserve live/replay authority, canonical ordering,
pre-execution budgets and commit-surface sequencing across extracted helpers.
They do not replace native execution or formal engine evidence.
"""
from __future__ import annotations


def validate_state_merge_source_item(item, symbol, tokens, errors):
    """Require behavior-bearing anchors and the exact current operation order."""
    for token in tokens:
        if token not in item:
            errors.append(f"State merge item {symbol} missing semantic token {token!r}")
    cursor = -1
    for token in STATE_MERGE_ORDERED_CHECKS.get(symbol, ()):
        position = item.find(token, cursor + 1)
        if position < 0:
            errors.append(f"State merge item {symbol} missing or reordered token {token!r}")
            break
        cursor = position


def validate_state_merge_source_contract(root, models, errors, rust_binding_item):
    """Bind every reviewed wrapper and helper to one physical production owner."""
    if not isinstance(models, list):
        errors.append("State merge contract requires the canonical model array")
        return
    for module, path, kind, symbol, tokens in STATE_MERGE_BINDINGS:
        selected = [model for model in models if isinstance(model, dict) and model.get("module") == module]
        rows = selected[0].get("production_symbols", []) if len(selected) == 1 else []
        rows = rows if isinstance(rows, list) else []
        matches = [row for row in rows if isinstance(row, dict)
                   and (row.get("path"), row.get("kind"), row.get("symbol")) == (path, kind, symbol)]
        if len(matches) != 1 or matches[0].get("required_tokens") != list(tokens):
            errors.append(f"{module}: State merge declaration must occur exactly once with current semantic tokens: {path}!{symbol}")
        item = rust_binding_item(root, path, kind, symbol, "State merge wrapper/helper", errors)
        if item is not None:
            validate_state_merge_source_item(item, symbol, tokens, errors)


STATE_MERGE_BINDINGS = (('SumeragiV2AutoscaleLifecycle',
  'crates/iroha_core/src/state.rs',
  'fn',
  'pending_autoscale_lane_drain_body',
  ('self.pending_autoscale_lane_drain_body_with_frontier(|lane, dataspace, incarnation| {',
   'Self::evidence_aware_lane_drain_frontier_from_world(',
   '&self.world.view(),\n'
   '                &self.kura,\n'
   '                lane,\n'
   '                dataspace,\n'
   '                incarnation,',
   '.ok()')),
 ('SumeragiV2AutoscaleLifecycle',
  'crates/iroha_core/src/state.rs',
  'fn',
  'pending_autoscale_lane_drain_body_with_frontier',
  ('frontier: impl FnOnce(LaneId, DataSpaceId, Hash) -> Option<LaneDrainFrontierV1>',
   'if !nexus.autoscale.enabled',
   'pending.is_some() || state.commitment.is_some() || lane.id != expected_lane',
   'if !autoscale_lane_drain_state_matches_context(',
   '&self.network_id,\n                incarnation,',
   'state.intent.close_global_height > committed_height',
   '!nexus_autoscale_lane_active_for_authority(',
   'state.intent.validator_set_hash != HashOf::new(&committee)',
   'state.intent.validator_count != validator_count',
   'state.intent.min_quorum != min_quorum',
   'let final_frontier = frontier(lane.id, lane.dataspace_id, incarnation)?;',
   'validate_lane_drain_certificate_body(&body)',
   '.map(|()| (body, committee))')),
 ('SumeragiV2AutonomousReservationCarrier',
  'crates/iroha_core/src/state.rs',
  'fn',
  'build_merge_execution_candidate_for_consensus',
  ('canonical_merged_lane_frontier_from_world',
   'validate_merge_execution_predecessor_against_frontier',
   'durable_autonomous_merge_source_for_lane_slot',
   'MergeExecutionSource::from_durable',
   'build_merge_execution_batch_from_source_prefix',
   '!consensus.is_current(self)',
   'epoch_id != admission.expected_epoch()',
   'consensus.committed_height.checked_add(1)?',
   'application_block_header.prev_block_hash() != consensus.latest_block_hash',
   'nexus.lane_catalog.lanes().len() > MAX_ACTIVE_EXECUTION_LANES',
   'lane_authority::authenticated_committee_for_descriptor(',
   'if descriptor.validator_set != authoritative',
   '.merge_active_lane_authority_snapshot(application_block_header.height().get())',
   'let candidate_template = crate::merge::MergeLedgerCandidate {',
   'lane_authority_catalog,',
   'Self::select_merge_execution_candidate_prefix(',
   'MAX_MERGE_LEDGER_ENTRY_BYTES.saturating_sub(MAX_MERGE_QC_BYTES)',
   'sources[..prefix_len].to_vec()',
   'consensus.is_current(self).then_some(selected).flatten()',
   'select_merge_execution_source_budget(\n'
   '            sources,\n'
   '            gas_limit_from_parameters(world.parameters()),',
   'if sources.is_empty()',
   'self.build_merge_execution_batch_from_source_prefix(\n'
   '                    epoch_id,\n'
   '                    application_block_header.clone(),\n'
   '                    sources[..prefix_len].to_vec(),')),
 ('SumeragiV2AutonomousReservationCarrier',
  'crates/iroha_core/src/state.rs',
  'fn',
  'select_merge_execution_source_budget',
  ('sources.sort_by_key(|source| {',
   'source.origin_proposal.descriptor.proposal_height,',
   'merge_execution_canonical_order_key(&source.certified.proposal)',
   'selected_entrypoints.checked_add(source.input.entrypoints.len())',
   'let gas = merge_execution_proposal_gas(&source.input.entrypoints)?;',
   'next_entrypoints > MAX_MERGE_EXECUTION_ENTRYPOINTS',
   '!crate::gas::gas_components_fit_block_limit(gas_limit, [selected_gas, gas])',
   'selected_gas.checked_add(gas)',
   'selected_entrypoints = next_entrypoints;',
   'selected_count += 1;',
   'sources.truncate(selected_count);')),
 ('SumeragiV2AutonomousReservationCarrier',
  'crates/iroha_core/src/state.rs',
  'fn',
  'merge_execution_proposal_gas',
  ('entrypoints.into_iter().try_fold(0u64, |total, entrypoint| {',
   'std::borrow::Cow::Borrowed(entrypoint)',
   'crate::queue::Queue::compute_proposal_gas_cost(&accepted)',
   'total.checked_add(gas)',
   'autonomous source proposal gas overflows u64')),
 ('SumeragiV2AutonomousReservationCarrier',
  'crates/iroha_core/src/state.rs',
  'fn',
  'build_merge_execution_batch_from_source_prefix',
  ('base_state_height',
   'base_state_hash',
   'preexecute_merge_execution_sources',
   'application_write_set_root',
   'merge_execution_write_set_root',
   'expected_post_state_hash',
   'merge_execution_batch_hash',
   'sources.is_empty() || total_entrypoints > MAX_MERGE_EXECUTION_ENTRYPOINTS',
   '.sort_by_key(|source| merge_execution_canonical_order_key(&source.certified.proposal))',
   '.preexecute_merge_execution_sources(application_block_header.clone(), sources)',
   'validate_merge_execution_commit_surface(MergeExecutionCommitSurface::Pristine)')),
 ('SumeragiV2AutonomousReservationCarrier',
  'crates/iroha_core/src/state.rs',
  'fn',
  'validate_merge_execution_predecessor_against_frontier',
  ('Self::validate_lane_frontier_successor(',
   'world,',
   'lane_id: descriptor.lane_id,',
   'dataspace_id: descriptor.dataspace_id,',
   'lane_incarnation: descriptor.lane_incarnation,',
   'lane_block_height: descriptor.lane_block_height,',
   'lane_block_descriptor_hash: descriptor.descriptor_hash,',
   'descriptor.previous_lane_block_height,\n'
   '            descriptor.previous_lane_block_descriptor_hash,')),
 ('SumeragiV2AutonomousReservationCarrier',
  'crates/iroha_core/src/state.rs',
  'fn',
  'validate_lane_frontier_successor',
  ('Self::canonical_merged_lane_frontier_from_world(',
   'descriptor.lane_id,\n            descriptor.dataspace_id,\n            descriptor.lane_incarnation,',
   'let actual_predecessor = (previous_height, previous_hash);',
   'if actual_predecessor != expected_predecessor',
   'expected_predecessor.0.checked_add(1)',
   'if descriptor.lane_block_height != expected_height',
   'MergeLedgerCommitError::NonContiguousLaneSnapshot')),
 ('SumeragiV2AutonomousReservationCarrier',
  'crates/iroha_core/src/state.rs',
  'fn',
  'preexecute_merge_execution_sources',
  ('self.merge_preexecution_block(application_block_header)',
   'Self::preexecute_merge_execution_sources_into(&mut state_block, sources)?',
   'Ok((state_block, executions))')),
 ('SumeragiV2AutonomousReservationCarrier',
  'crates/iroha_core/src/state.rs',
  'fn',
  'preexecute_merge_execution_sources_into',
  ('Self::preexecute_merge_execution_sources_into_with_replay(state_block, sources, None)',)),
 ('SumeragiV2AutonomousReservationCarrier',
  'crates/iroha_core/src/state.rs',
  'fn',
  'preexecute_merge_execution_sources_into_with_replay',
  ('decode_autonomous_lane_merge_bundle',
   'validate_lane_block_execution_input_with_routing_context',
   'stage_merge_carrier_entrypoints',
   'drain_merge_lane_settlement_commitment',
   'seen_entrypoints',
   'seen_reservations',
   'replay: Option<&crate::block::VerifiedReplayProposal>',
   'sources.iter().flat_map(|source| &source.input.entrypoints)',
   'state_block.gas_limit_per_block,\n            [state_block.gas_used_in_block, reserved_gas],',
   'authenticated_bundle.certified != source.certified',
   'authenticated_bundle.bundle_hash().ok() != Some(source.bundle_hash)',
   'source.input.reservation_keys != authenticated_payload.reservation_keys',
   'source.input.routing_plans != authenticated_payload.routing_plans',
   '!= iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced',
   '!seen_entrypoints.insert(*hash)',
   '!seen_reservations.insert(reservation.digest())',
   '.map(|token| token.native_amx_authority(&*state_block))',
   'Some(authority) => authority,',
   'None => &*state_block,')),
 ('SumeragiV2AutonomousReservationCarrier',
  'crates/iroha_core/src/state.rs',
  'fn',
  'stage_certified_merge_entry',
  ('self.stage_certified_merge_entry_with_replay(entry, frozen_mode, None)',)),
 ('SumeragiV2AutonomousReservationCarrier',
  'crates/iroha_core/src/state.rs',
  'fn',
  'stage_certified_merge_entry_with_replay',
  ('merge_execution_already_applied',
   'base_state_hash',
   'preexecute_merge_execution_sources_into',
   'validate_merge_execution_commit_surface',
   'MergeExecutionCommitSurface::Pristine',
   'application_write_set_root',
   'stage_merge_execution_markers',
   'expected_post_state_hash',
   'let external_event_count = self.world.external_event_buf.len()',
   'let external_event_bytes = Self::merge_execution_external_event_bytes(&self.world)',
   'validated_publication_event_bytes: None',
   'replay: Option<&crate::block::VerifiedReplayProposal>',
   'self.ensure_pristine_execution_control_stage()?;',
   '.validate_merge_stage(&self._curr_block, &*self, entry)',
   '.validate_certified_merge_entry_for_global_order_with_replay(\n'
   '                entry,\n'
   '                frozen_mode,\n'
   '                replay,',
   'State::preexecute_merge_execution_sources_into_with_replay(self, sources, replay)',
   'self._curr_block = carrier_header;',
   'let actual_lanes = execution?;',
   'if actual_lanes != batch.lanes',
   'self.canonical_wsv_merge_commit_authorization =')),
 ('SumeragiV2AutonomousReservationCarrier',
  'crates/iroha_core/src/state.rs',
  'fn',
  'validate_merge_execution_batch',
  ('self.validate_merge_execution_batch_with_replay(\n'
   '            active_lanes,\n'
   '            batch,\n'
   '            validation_authority,\n'
   '            None,',)),
 ('SumeragiV2AutonomousReservationCarrier',
  'crates/iroha_core/src/state.rs',
  'fn',
  'validate_merge_execution_batch_with_replay',
  ('reservation_keys',
   'routing_plans',
   'merge_execution_batch_commitments_match',
   'entrypoint_hash',
   'LaneIncarnationMismatch',
   'validate_merge_execution_predecessor_against_frontier',
   'if !validate_live_authority',
   'validate_historical_native_amx_source_bundle',
   'HistoricalNativeAmxSourceAuthority::MergeQcActiveLanes',
   'merge_execution_canonical_order_key',
   'previous_order',
   'strict canonical total order',
   'replay: Option<&crate::block::VerifiedReplayProposal>',
   'MergeExecutionValidationAuthority::Live(mode) => Some(*mode)',
   'authority.entry.active_lanes != active_lanes',
   'authority.entry.execution_batch.as_ref() != Some(batch)',
   'let validate_live_authority = frozen_mode.is_some();',
   'if previous_order.is_some_and(|previous| order <= previous)',
   'previous_order = Some(order);',
   'if validate_live_authority {\n'
   '                Self::validate_merge_execution_predecessor_against_frontier(world, descriptor)?;',
   'reservation.routing_plan_digest != routing_plan.digest()',
   'reservation.coordinator_leg != routing_plan.coordinator_leg()')))

STATE_MERGE_ORDERED_CHECKS = {'pending_autoscale_lane_drain_body_with_frontier': ('if !autoscale_lane_drain_state_matches_context(',
                                                     'state.intent.close_global_height > '
                                                     'committed_height',
                                                     'state.intent.validator_set_hash != '
                                                     'HashOf::new(&committee)',
                                                     'let final_frontier = frontier(lane.id, '
                                                     'lane.dataspace_id, incarnation)?;',
                                                     'validate_lane_drain_certificate_body(&body)'),
 'build_merge_execution_candidate_for_consensus': ('let mut sources = Vec::new();',
                                                   'validate_merge_execution_predecessor_against_frontier',
                                                   'if descriptor.validator_set != authoritative',
                                                   'sources.push(MergeExecutionSource::from_durable(durable_source));',
                                                   'let sources = match '
                                                   'select_merge_execution_source_budget(',
                                                   'Self::select_merge_execution_candidate_prefix(',
                                                   'self.build_merge_execution_batch_from_source_prefix(',
                                                   'consensus.is_current(self).then_some(selected).flatten()'),
 'select_merge_execution_source_budget': ('source.origin_proposal.descriptor.proposal_height,',
                                          'merge_execution_canonical_order_key(&source.certified.proposal)',
                                          'selected_entrypoints.checked_add(source.input.entrypoints.len())',
                                          'let gas = '
                                          'merge_execution_proposal_gas(&source.input.entrypoints)?;',
                                          'next_entrypoints > MAX_MERGE_EXECUTION_ENTRYPOINTS',
                                          'selected_gas.checked_add(gas)',
                                          'selected_count += 1;',
                                          'sources.truncate(selected_count);'),
 'build_merge_execution_batch_from_source_prefix': ('sources.is_empty() || total_entrypoints > '
                                                    'MAX_MERGE_EXECUTION_ENTRYPOINTS',
                                                    '.sort_by_key(|source| '
                                                    'merge_execution_canonical_order_key(&source.certified.proposal))',
                                                    '.preexecute_merge_execution_sources(application_block_header.clone(), '
                                                    'sources)',
                                                    'validate_merge_execution_commit_surface(MergeExecutionCommitSurface::Pristine)',
                                                    'let application_write_set_root = '
                                                    'state_block.merge_execution_write_set_root();'),
 'validate_lane_frontier_successor': ('Self::canonical_merged_lane_frontier_from_world(',
                                      'if actual_predecessor != expected_predecessor',
                                      'expected_predecessor.0.checked_add(1)',
                                      'if descriptor.lane_block_height != expected_height',
                                      'MergeLedgerCommitError::NonContiguousLaneSnapshot'),
 'preexecute_merge_execution_sources_into_with_replay': ('let reserved_gas = '
                                                         'merge_execution_proposal_gas(',
                                                         'if '
                                                         '!crate::gas::gas_components_fit_block_limit(',
                                                         'for source in sources {',
                                                         'decode_autonomous_lane_merge_bundle(',
                                                         'authenticated_bundle.certified != '
                                                         'source.certified',
                                                         'source.input.reservation_keys != '
                                                         'authenticated_payload.reservation_keys',
                                                         '!seen_entrypoints.insert(*hash)',
                                                         '!seen_reservations.insert(reservation.digest())',
                                                         '.validate_lane_block_execution_input_with_routing_context(',
                                                         'stage_merge_carrier_entrypoints',
                                                         'drain_merge_lane_settlement_commitment'),
 'stage_certified_merge_entry_with_replay': ('self.ensure_pristine_execution_control_stage()?;',
                                             '.validate_certified_merge_entry_for_global_order_with_replay(',
                                             '.merge_execution_already_applied(entry, batch)?',
                                             'batch.base_state_height != actual_height || '
                                             'batch.base_state_hash != actual_hash',
                                             'State::preexecute_merge_execution_sources_into_with_replay(self, '
                                             'sources, replay)',
                                             'self._curr_block = carrier_header;',
                                             'let actual_lanes = execution?;',
                                             'if actual_lanes != batch.lanes',
                                             'self.validate_merge_execution_commit_surface(MergeExecutionCommitSurface::Pristine)?;',
                                             'self.merge_execution_write_set_root() != '
                                             'batch.application_write_set_root',
                                             'self.stage_merge_execution_markers(entry.epoch_id, '
                                             'batch)?;',
                                             'actual_post_state_hash != batch.expected_post_state_hash',
                                             'if actual_batch_hash != batch.batch_hash',
                                             'let external_event_count = '
                                             'self.world.external_event_buf.len()',
                                             'validated_publication_event_bytes: None'),
 'validate_merge_execution_batch_with_replay': ('authority.entry.active_lanes != active_lanes',
                                                'let validate_live_authority = frozen_mode.is_some();',
                                                'if '
                                                '!crate::merge::merge_execution_batch_commitments_match(batch)',
                                                'let mut previous_order = None;',
                                                'let order = '
                                                'merge_execution_canonical_order_key(&execution.proposal);',
                                                'if previous_order.is_some_and(|previous| order <= '
                                                'previous)',
                                                'previous_order = Some(order);',
                                                'MergeLedgerCommitError::LaneIncarnationMismatch',
                                                'Self::validate_merge_execution_predecessor_against_frontier(world, '
                                                'descriptor)?;',
                                                'reservation.routing_plan_digest != '
                                                'routing_plan.digest()')}
