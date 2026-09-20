"""Current Kura Native owners and branch-specific durable publication order.

These are structural source contracts, not native execution or release evidence.
The completed-retry branch is admitted only by the original all-route durable
join; fresh publication consumes each reservation after its durable writer.
"""
from __future__ import annotations

import re

KURA = 'crates/iroha_core/src/kura.rs'
DATA = 'crates/iroha_data_model/src/block/consensus.rs'
CAPACITY = 'crates/iroha_core/src/kura/native_amx_publication_capacity.rs'
LIVE = 'persist_native_amx_participant_application_evidence_under_publication_guard'
REPAIR = 'persist_native_amx_participant_application_repair_targets_under_publication_guard'
BRANCHED_SYMBOLS = (LIVE, REPAIR)


def reconcile_bindings(original):
    """Replace only explicitly reviewed moved owners; retain all other bindings."""
    replacements = {(path, kind, symbol): tokens for path, kind, symbol, tokens in BINDINGS}
    result = []
    for path, kind, symbol, tokens in original:
        result.append((path, kind, symbol, replacements.pop((path, kind, symbol), tokens)))
    result.extend((*key, tokens) for key, tokens in replacements.items())
    return tuple(result)


def _compact(value):
    return ' '.join(value.split())


def _ordered(item, tokens, label, errors):
    """Each reviewed statement must appear once in this exact branch and order."""
    value = _compact(item)
    cursor = -1
    for raw in tokens:
        token = _compact(raw)
        position = value.find(token, cursor + 1)
        if value.count(token) != 1 or position < 0:
            errors.append(f'{KURA}: {label} loses unique ordered statement {token!r}')
            return
        cursor = position


def validate(root, items, errors, read_item, extract):
    """Check current cap/decode, Native chain, resource and branch relations."""
    def owner(path, kind, symbol):
        key = (path, kind, symbol)
        if key not in items:
            items[key] = read_item(root, path, kind, symbol, 'current Native owner', errors)
        return items[key]

    for path, kind, symbol, tokens in BINDINGS:
        item = owner(path, kind, symbol)
        if item is not None:
            for token in tokens:
                if token not in item:
                    errors.append(f'{path}: current Native owner {symbol} lacks {token!r}')

    # The generic method parser intentionally does not infer generic trait
    # ownership. Bind this exact decoder impl and its complete checked method.
    provider = root / DATA
    if not provider.is_file() or provider.is_symlink():
        errors.append(f'{DATA}: missing exact Native settlement decoder provider')
    else:
        source = provider.read_text(encoding='utf-8')
        if source.count('pub const NATIVE_AMX_GROUP_SOURCES_MAX: usize = 4_096;') != 1:
            errors.append(f'{DATA}: Native settlement source cap must remain exactly 4096')
        settlement = owner(DATA, 'struct', 'NativeAmxParticipantSettlement')
        if settlement is not None and re.search(r'\bpub(?:\s|\()', settlement.split('{', 1)[1]):
            errors.append(f'{DATA}: Native settlement fields must remain private constructor invariants')
        matches = list(re.finditer(
            r"(?m)^impl<'de> norito::core::DeserializePayload<'de> for NativeAmxParticipantSettlement\s*", source))
        implementation = extract(source, matches[0]) if len(matches) == 1 else None
        method_matches = [] if implementation is None else list(re.finditer(r'\bfn try_deserialize\s*', implementation))
        method = extract(implementation, method_matches[0]) if len(method_matches) == 1 else None
        if method is None or _compact(method) != BINARY_DECODER:
            errors.append(f'{DATA}: Native settlement binary decoder must use its exact checked constructor chain')

    receipt = owner(KURA, 'fn', 'validate_native_amx_participant_application_receipt_artifact')
    if receipt is not None and RECEIPT_MEMBERSHIP_GUARD not in _compact(receipt):
        errors.append(f'{KURA}: Native receipt must preserve its complete bounded settlement membership disjunction')

    for symbol in BRANCHED_SYMBOLS:
        item = owner(KURA, 'fn', symbol)
        if item is None:
            continue
        matches = list(re.finditer(r'\bif !publication_required\s*', item))
        retry = extract(item, matches[0]) if len(matches) == 1 else None
        if retry is None:
            errors.append(f'{KURA}: {symbol} must have one completed-retry branch')
            continue
        prefix = item[:matches[0].start()]
        fresh = item[matches[0].start() + len(retry):]
        _ordered(prefix, PREFIX_ORDER[symbol], symbol + ' capacity admission', errors)
        if len(re.findall(r'\bpublication_required\b', prefix)) != 1:
            errors.append(f'{KURA}: {symbol} must use the sole capacity result without shadowing or reassignment')
        if symbol == LIVE and len(re.findall(r'\ball_targets\b', prefix)) != 2:
            errors.append(f'{KURA}: live Native admission must retain the exact all-target vector')
        if _compact(retry) != RETRY_BRANCHES[symbol]:
            errors.append(f'{KURA}: ordered Native prepublication item {symbol} completed retry must authenticate the reviewed all-target durable join before its exact return')
        _ordered(fresh, FRESH_ORDER[symbol], 'ordered Native prepublication item ' + symbol + ' fresh publication', errors)
        for call in PUBLICATION_CALLS:
            if fresh.count(call) != 1 or call in retry:
                errors.append(f'{KURA}: {symbol} must perform each durable publication only once in the fresh branch')
        if symbol == LIVE:
            cleanup = FRESH_LOOPS[symbol][-1]
            if _compact('if permit_cleanup { ' + cleanup + ' }') not in _compact(fresh):
                errors.append(f'{KURA}: live Native cleanup-only-after-WSV must remain conditional on PostWsvRepair')

    for symbol, tokens in ORDERED_OWNERS:
        path = CAPACITY if symbol.startswith('Kura::') else KURA
        item = owner(path, 'method' if symbol.startswith('Kura::') else 'fn', symbol)
        if item is not None:
            _ordered(item, tokens, symbol, errors)

    # Completed retries must prove every route's durable join before false is
    # returned. A pending reservation cannot take the already-complete branch.
    capacity = owner(CAPACITY, 'method', 'Kura::ensure_native_amx_publication_capacity_under_publication_guard')
    if capacity is not None and _compact(capacity).count(COMPLETED_CAPACITY_BRANCH) != 1:
        errors.append(f'{CAPACITY}: completed retry lost its no-existing-owner/all-route durable join condition')
    joined = owner(CAPACITY, 'method', 'Kura::native_amx_publication_plan_is_durably_complete_under_prune_and_canonical_guards')
    if joined is not None and _compact(joined) != COMPLETED_JOIN:
        errors.append(f'{CAPACITY}: completed retry must preserve the full all-route pending-index, latest identity, WSV join and sync sequence')


# Exact reviewed values are declared below. They are ordinary source declarations
# so the model and tests authenticate their full content instead of trusting a
# runtime file or a caller-supplied token list.

BINDINGS = (('crates/iroha_core/src/kura.rs',
  'fn',
  'validate_native_amx_participant_application_receipt_artifact',
  ('manifest_artifact_hash',
   'participant_settlement_hash',
   'result_hashes',
   'let settlement_source_ids = settlement.source_ids();',
   'settlement_source_ids != artifact.source_ids',
   'artifact.source_ids.len() != artifact.entrypoint_indices.len()')),
 ('crates/iroha_core/src/kura.rs',
  'fn',
  'validate_native_amx_evidence_prune_intent_locked',
  ('NativeAmxEvidencePruneIntentV2::VERSION',
 'active_lane_incarnation_marker',
 'native_amx_evidence_prune_intent_max_entries',
 'derive_native_amx_evidence_prune_protected_latest_locked',
 'protected_latest != intent.protected_latest',
 'native_amx_evidence_prune_entry_kind',
 'participant_height == 0',
 'participant_height >= protected_height',
 'artifact_hash',
 'entries are not strictly ordered',
 'complete manifest/receipt pairs',
 'if preimage_heights != removal_heights',
 'height <= highest_removal && !removal_heights.contains(height)',
 'original_links.keys().copied().collect::<BTreeSet<_>>() != original_heights',
 'Self::validate_native_amx_settlement_chain_links(&original_links)',
 'receipt.participant_settlement != *settlement',
 'one oldest contiguous prefix',
 'validate_native_amx_retained_history_continuity',
 'validate_native_amx_evidence_prune_protected_latest_locked')),
 ('crates/iroha_core/src/kura.rs',
  'fn',
  'plan_native_amx_evidence_pair_prune_locked',
  ('decode_native_amx_manifest_file_locked(entry, namespace, file)?',
 'decode_native_amx_receipt_file_locked(entry, namespace, file)?',
 'Self::plan_native_amx_evidence_prune_intent_from_artifacts(\n'
 '            self.native_amx_participant_evidence_retention(),\n'
 '            self.native_amx_participant_evidence_file_bytes(),\n'
 '            self.native_amx_evidence_prune_intent_max_bytes(),\n'
 '            &manifests,\n'
 '            &receipts,\n'
 '        )?',
 'derive_native_amx_evidence_prune_protected_latest_locked',
 'intent.protected_latest != protected_latest',
 'for removal in &intent.entries',
 'read_native_amx_evidence_file_bytes_locked(namespace, file)?',
 'if Hash::new(bytes) != removal.artifact_hash')),
 ('crates/iroha_core/src/kura.rs',
  'fn',
  'prune_native_amx_evidence_pairs_locked',
  ('complete_native_amx_evidence_prune_intent_locked(batch.guard(), entry, namespace)?',
 'recover_native_amx_evidence_publication_temp_locked(',
 'NativeAmxEvidenceRecoveryPhase::Startup',
 'self.plan_native_amx_evidence_pair_prune_locked(entry, namespace, &inventory)?',
 'self.validate_native_amx_evidence_prune_intent_locked(entry, namespace, &intent)?',
 'bytes.is_empty() || bytes.len() > self.native_amx_evidence_prune_intent_max_bytes()',
 'if !self.publish_bound_noclobber_file_locked(',
 'Native AMX evidence prune intent appeared concurrently',
 'self.complete_native_amx_evidence_prune_intent_locked(\n'
 '            publication.guard(),\n'
 '            entry,\n'
 '            namespace,\n'
 '        )?',
 'self.inventory_native_amx_evidence_files_locked(namespace, false)?')),
 ('crates/iroha_core/src/kura.rs',
  'fn',
  'validate_native_amx_retained_history_continuity',
  ('retained_heights.windows(2)',
   'allow_highest_partial',
   'partial_count',
   'retained_heights.last()',
   'manifest_artifact_hash',
   'native_amx_participant_receipt_matches_manifest_leaf',
   'previous_lane_block_height',
   'predecessor.leaf.descriptor_hash',
   'Self::validate_native_amx_settlement_chain_links(&links)?;',
   'previous_height.checked_add(1) != Some(successor_height)',
   'previous_height == predecessor_height',
   'application_height < predecessor.leaf.application_block_height')),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::try_new',
  ('participant_lane_block_height == 0',
   'authority_context_height == 0',
   'participant_lane_block_height == 1 && previous_native_settlement_hash.is_some()',
   'source_ids.is_empty() || source_ids.len() > NATIVE_AMX_GROUP_SOURCES_MAX',
   '!native_amx_nonzero(source)',
   'collect::<std::collections::BTreeSet<_>>()',
   '!= source_ids.len()')),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::try_from',
  ('Self::try_new(',
 'wire.lane_id',
 'wire.dataspace_id',
 'wire.lane_incarnation',
 'wire.participant_lane_block_height',
 'wire.authority_context_height',
 'wire.previous_native_settlement_hash',
 'wire.source_ids')),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::json_deserialize',
  ('NativeAmxParticipantSettlementWire as norito::json::JsonDeserialize',
 'json_deserialize(parser)?',
 'Self::try_from(wire).map_err')),
 ('crates/iroha_core/src/kura.rs',
  'fn',
  'validate_native_amx_settlement_chain_links',
  ('height == 0',
   'height == 1 && previous_native_hash.is_some()',
   'previous_native_hash != Some(previous)',
   'predecessor = Some(settlement_hash)')),
 ('crates/iroha_core/src/kura.rs',
  'fn',
  'plan_native_amx_evidence_prune_intent_from_artifacts',
  ('retention.get().checked_add(1)',
 'if manifests.len() > count_limit || receipts.len() > count_limit',
 'Self::validate_native_amx_retained_history_continuity(manifests, receipts, false)',
 'NativeAmxEvidencePruneProtectedLatestV2::from_artifacts(',
 'for height in complete.iter().rev()',
 'if pair_len > stable_byte_limit',
 'let fits = !stopped',
 'kept_complete.len() < retention.get()',
 '.checked_add(pair_len)',
 '.is_some_and(|bytes| bytes <= stable_byte_limit)',
 'if !fits {\n                stopped = true;\n                continue;\n            }',
 'if !kept_complete.contains(&protected_height)',
 'for height in complete.difference(&kept_complete)',
 'NativeAmxEvidencePruneIntentV2::MANIFEST_KIND',
 'NativeAmxEvidencePruneIntentV2::RECEIPT_KIND',
 'artifact_hash: Hash::new(bytes)',
 'NativeAmxEvidencePruneIntentV2::VERSION',
 'Self::collect_native_amx_prune_settlement_preimages(',
 'bytes.is_empty() || bytes.len() > journal_byte_limit')),
 ('crates/iroha_core/src/kura.rs',
  'fn',
  'collect_native_amx_prune_settlement_preimages',
  ('if !intent.removed_settlements.is_empty()',
 'let mut retained_bytes = norito::encode_canonical(intent)?.len()',
 'if retained_bytes == 0 || retained_bytes > byte_limit',
 'if removal.kind != NativeAmxEvidencePruneIntentV2::RECEIPT_KIND',
 'let settlement = load(removal.participant_height)?',
 'let settlement_bytes = norito::encode_canonical(&settlement)?.len()',
 '.checked_add(settlement_bytes)',
 '.and_then(|bytes| bytes.checked_add(PREFIX_HEADROOM))',
 'if next_bytes > byte_limit',
 'removed_settlements.try_reserve_exact(1)?',
 'removed_settlements.push(settlement)',
 'retained_bytes = next_bytes')),
 ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::native_amx_publication_plan_is_durably_complete_under_prune_and_canonical_guards',
  ('!route.outstanding_components.is_empty() || route.prune_journal_bytes != 0',
   'require_native_amx_evidence_prune_intent_absent_locked',
   'require_native_amx_latest_index_temp_absent_locked',
   'derive_native_amx_evidence_prune_protected_latest_locked',
   'latest.participant_settlement_hash != capacity.settlement_hash',
   'native_amx_publication_wsv_join_is_complete_locked',
   'sync_native_amx_evidence_namespace',
   'inventory_native_amx_evidence_files_locked',
   'for (route, capacity) in &plan.routes',
   '.contains_key(&carrier)',
   'latest.application_block_height != carrier.height',
   'latest.application_block_hash != carrier.block_hash',
   'latest.executed_block_wire_hash != carrier.executed_wire_hash',
   'latest.lane_incarnation != route.incarnation',
   'latest.lane_block_height != capacity.participant_height',
   'latest.participant_proposal_hash != capacity.proposal_hash')),
 ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::consume_native_amx_publication_component_after_durable_publication',
  ('capacity.participant_height != descriptor.lane_block_height',
   'capacity.settlement_hash != receipt.participant_settlement_hash',
   'Some(u64::try_from(encoded_len)?)',
   '*allocation = 0;',
   'capacity.outstanding_components.remove(&component);')))

BINARY_DECODER = ("fn try_deserialize( archived: &'de norito::core::Archived<Self>, ) -> Result<Self, "
 'norito::core::Error> { let wire = <NativeAmxParticipantSettlementWire as '
 'norito::core::DeserializePayload>::try_deserialize( archived.cast())?; '
 'Self::try_from(wire).map_err(|error| norito::core::Error::Message(error.to_owned())) }')

RETRY_BRANCHES = {'persist_native_amx_participant_application_evidence_under_publication_guard': 'if '
                                                                                '!publication_required '
                                                                                '{ '
                                                                                'self.read_back_native_amx_plan_manifests_under_publication_guard(plan)?; '
                                                                                'let mut '
                                                                                'identities = '
                                                                                'Vec::with_capacity(plan.artifacts.len()); '
                                                                                'for (manifest, '
                                                                                'receipt) in '
                                                                                '&plan.artifacts { '
                                                                                'identities.push(self.authenticate_native_amx_participant_application_prepublication_under_publication_guard(manifest, '
                                                                                'receipt, '
                                                                                'mode.requires_post_apply_metadata())?); '
                                                                                '} let token = '
                                                                                'NativeAmxParticipantApplicationPrepublicationToken::from_plan(plan, '
                                                                                'identities) '
                                                                                '.ok_or_else(|| { '
                                                                                'Self::invalid_lane_artifact_error( '
                                                                                'self.store_root.clone(), '
                                                                                '"Native AMX '
                                                                                'completed token '
                                                                                'does not cover '
                                                                                'the exact '
                                                                                'manifest", ) })?; '
                                                                                'if permit_cleanup '
                                                                                '{ for (_, '
                                                                                'receipt) in '
                                                                                '&plan.artifacts { '
                                                                                'self.cleanup_native_amx_participant_application_evidence_under_publication_guard(receipt)?; '
                                                                                '} } return '
                                                                                'Ok(token); }',
 'persist_native_amx_participant_application_repair_targets_under_publication_guard': 'if '
                                                                                      '!publication_required '
                                                                                      '{ '
                                                                                      'self.read_back_native_amx_repair_target_manifests_under_publication_guard( '
                                                                                      'plan, '
                                                                                      'target_indices, '
                                                                                      ')?; for '
                                                                                      '&index in '
                                                                                      'target_indices '
                                                                                      '{ let '
                                                                                      '(manifest, '
                                                                                      'receipt) = '
                                                                                      '&plan.artifacts[index]; '
                                                                                      'let _ = '
                                                                                      'self.authenticate_native_amx_participant_application_prepublication_under_publication_guard(manifest, '
                                                                                      'receipt, '
                                                                                      'true)?; '
                                                                                      'self.cleanup_native_amx_participant_application_evidence_under_publication_guard( '
                                                                                      'receipt, '
                                                                                      ')?; } '
                                                                                      'return '
                                                                                      'Ok(target_indices.len()); '
                                                                                      '}'}

FRESH_LOOPS = {'persist_native_amx_participant_application_evidence_under_publication_guard': ('for (manifest, '
                                                                                 'receipt) in '
                                                                                 '&plan.artifacts '
                                                                                 '{ '
                                                                                 'self.write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard( '
                                                                                 'manifest, '
                                                                                 'permit_cleanup, '
                                                                                 ')?; '
                                                                                 'self.consume_native_amx_publication_component_after_durable_publication( '
                                                                                 'receipt, '
                                                                                 'NativeAmxPublicationComponent::Manifest, '
                                                                                 'manifest.encode_framed()?.len(), '
                                                                                 ')?; }',
                                                                                 'for (manifest, '
                                                                                 'receipt) in '
                                                                                 '&plan.artifacts '
                                                                                 '{ '
                                                                                 'self.write_native_amx_participant_application_receipt_artifact_only_with_retention_policy_under_publication_guard( '
                                                                                 'receipt, '
                                                                                 'manifest, '
                                                                                 'permit_cleanup, '
                                                                                 ')?; '
                                                                                 'self.consume_native_amx_publication_component_after_durable_publication( '
                                                                                 'receipt, '
                                                                                 'NativeAmxPublicationComponent::Receipt, '
                                                                                 'receipt.encode_framed()?.len(), '
                                                                                 ')?; }',
                                                                                 'for ((manifest, '
                                                                                 'receipt), '
                                                                                 'preflight) in '
                                                                                 'plan.artifacts.iter().zip(route_preflights.iter()) '
                                                                                 '{ '
                                                                                 'self.write_native_amx_participant_receipt_latest_index_for_prepublication_under_publication_guard( '
                                                                                 'receipt, '
                                                                                 'manifest, '
                                                                                 'permit_cleanup, '
                                                                                 'preflight, )?; '
                                                                                 'self.consume_native_amx_publication_component_after_durable_publication( '
                                                                                 'receipt, '
                                                                                 'NativeAmxPublicationComponent::Latest, '
                                                                                 'norito::encode_canonical(&NativeAmxParticipantReceiptLatestIndexV2::from_receipt( '
                                                                                 'receipt, ))? '
                                                                                 '.len(), )?; }',
                                                                                 'for (manifest, '
                                                                                 'receipt) in '
                                                                                 '&plan.artifacts '
                                                                                 '{ '
                                                                                 'identities.push( '
                                                                                 'self.authenticate_native_amx_participant_application_prepublication_under_publication_guard( '
                                                                                 'manifest, '
                                                                                 'receipt, '
                                                                                 'mode.requires_post_apply_metadata(), '
                                                                                 ')?, ); }',
                                                                                 'for (_, receipt) '
                                                                                 'in '
                                                                                 '&plan.artifacts '
                                                                                 '{ '
                                                                                 'self.cleanup_native_amx_participant_application_evidence_under_publication_guard( '
                                                                                 'receipt, )?; }'),
 'persist_native_amx_participant_application_repair_targets_under_publication_guard': ('for &index '
                                                                                       'in '
                                                                                       'target_indices '
                                                                                       '{ let '
                                                                                       '(manifest, '
                                                                                       'receipt) = '
                                                                                       '&plan.artifacts[index]; '
                                                                                       'self.write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard( '
                                                                                       'manifest, '
                                                                                       'true, )?; '
                                                                                       'self.consume_native_amx_publication_component_after_durable_publication( '
                                                                                       'receipt, '
                                                                                       'NativeAmxPublicationComponent::Manifest, '
                                                                                       'manifest.encode_framed()?.len(), '
                                                                                       ')?; }',
                                                                                       'for &index '
                                                                                       'in '
                                                                                       'target_indices '
                                                                                       '{ let '
                                                                                       '(manifest, '
                                                                                       'receipt) = '
                                                                                       '&plan.artifacts[index]; '
                                                                                       'self.write_native_amx_participant_application_receipt_artifact_only_with_retention_policy_under_publication_guard( '
                                                                                       'receipt, '
                                                                                       'manifest, '
                                                                                       'true, )?; '
                                                                                       'self.consume_native_amx_publication_component_after_durable_publication( '
                                                                                       'receipt, '
                                                                                       'NativeAmxPublicationComponent::Receipt, '
                                                                                       'receipt.encode_framed()?.len(), '
                                                                                       ')?; }',
                                                                                       'for '
                                                                                       '(&index, '
                                                                                       'preflight) '
                                                                                       'in '
                                                                                       'target_indices.iter().zip(route_preflights.iter()) '
                                                                                       '{ let '
                                                                                       '(manifest, '
                                                                                       'receipt) = '
                                                                                       '&plan.artifacts[index]; '
                                                                                       'self.write_native_amx_participant_receipt_latest_index_for_prepublication_under_publication_guard( '
                                                                                       'receipt, '
                                                                                       'manifest, '
                                                                                       'true, '
                                                                                       'preflight, '
                                                                                       ')?; '
                                                                                       'self.consume_native_amx_publication_component_after_durable_publication( '
                                                                                       'receipt, '
                                                                                       'NativeAmxPublicationComponent::Latest, '
                                                                                       'norito::encode_canonical(&NativeAmxParticipantReceiptLatestIndexV2::from_receipt( '
                                                                                       'receipt, '
                                                                                       '))? '
                                                                                       '.len(), '
                                                                                       ')?; }',
                                                                                       'for &index '
                                                                                       'in '
                                                                                       'target_indices '
                                                                                       '{ let '
                                                                                       '(manifest, '
                                                                                       'receipt) = '
                                                                                       '&plan.artifacts[index]; '
                                                                                       'let _ = '
                                                                                       'self.authenticate_native_amx_participant_application_prepublication_under_publication_guard( '
                                                                                       'manifest, '
                                                                                       'receipt, '
                                                                                       'true, )?; '
                                                                                       '}',
                                                                                       'for &index '
                                                                                       'in '
                                                                                       'target_indices '
                                                                                       '{ let (_, '
                                                                                       'receipt) = '
                                                                                       '&plan.artifacts[index]; '
                                                                                       'self.cleanup_native_amx_participant_application_evidence_under_publication_guard( '
                                                                                       'receipt, '
                                                                                       ')?; }')}

FRESH_ORDER = {'persist_native_amx_participant_application_evidence_under_publication_guard': ('for (manifest, '
                                                                                 'receipt) in '
                                                                                 '&plan.artifacts '
                                                                                 '{ '
                                                                                 'self.write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard( '
                                                                                 'manifest, '
                                                                                 'permit_cleanup, '
                                                                                 ')?; '
                                                                                 'self.consume_native_amx_publication_component_after_durable_publication( '
                                                                                 'receipt, '
                                                                                 'NativeAmxPublicationComponent::Manifest, '
                                                                                 'manifest.encode_framed()?.len(), '
                                                                                 ')?; }',
                                                                                 'let '
                                                                                 'manifest_readback '
                                                                                 '= '
                                                                                 'self.read_back_native_amx_plan_manifests_under_publication_guard(plan)?;',
                                                                                 '!manifest_readback.authenticates(plan, '
                                                                                 'manifest)',
                                                                                 'for (manifest, '
                                                                                 'receipt) in '
                                                                                 '&plan.artifacts '
                                                                                 '{ '
                                                                                 'self.write_native_amx_participant_application_receipt_artifact_only_with_retention_policy_under_publication_guard( '
                                                                                 'receipt, '
                                                                                 'manifest, '
                                                                                 'permit_cleanup, '
                                                                                 ')?; '
                                                                                 'self.consume_native_amx_publication_component_after_durable_publication( '
                                                                                 'receipt, '
                                                                                 'NativeAmxPublicationComponent::Receipt, '
                                                                                 'receipt.encode_framed()?.len(), '
                                                                                 ')?; }',
                                                                                 'for ((manifest, '
                                                                                 'receipt), '
                                                                                 'preflight) in '
                                                                                 'plan.artifacts.iter().zip(route_preflights.iter()) '
                                                                                 '{ '
                                                                                 'self.write_native_amx_participant_receipt_latest_index_for_prepublication_under_publication_guard( '
                                                                                 'receipt, '
                                                                                 'manifest, '
                                                                                 'permit_cleanup, '
                                                                                 'preflight, )?; '
                                                                                 'self.consume_native_amx_publication_component_after_durable_publication( '
                                                                                 'receipt, '
                                                                                 'NativeAmxPublicationComponent::Latest, '
                                                                                 'norito::encode_canonical(&NativeAmxParticipantReceiptLatestIndexV2::from_receipt( '
                                                                                 'receipt, ))? '
                                                                                 '.len(), )?; }',
                                                                                 'for (manifest, '
                                                                                 'receipt) in '
                                                                                 '&plan.artifacts '
                                                                                 '{ '
                                                                                 'identities.push( '
                                                                                 'self.authenticate_native_amx_participant_application_prepublication_under_publication_guard( '
                                                                                 'manifest, '
                                                                                 'receipt, '
                                                                                 'mode.requires_post_apply_metadata(), '
                                                                                 ')?, ); }',
                                                                                 'let token = '
                                                                                 'NativeAmxParticipantApplicationPrepublicationToken::from_plan',
                                                                                 'for (_, receipt) '
                                                                                 'in '
                                                                                 '&plan.artifacts '
                                                                                 '{ '
                                                                                 'self.cleanup_native_amx_participant_application_evidence_under_publication_guard( '
                                                                                 'receipt, )?; }',
                                                                                 'Ok(token)'),
 'persist_native_amx_participant_application_repair_targets_under_publication_guard': ('for &index '
                                                                                       'in '
                                                                                       'target_indices '
                                                                                       '{ let '
                                                                                       '(manifest, '
                                                                                       'receipt) = '
                                                                                       '&plan.artifacts[index]; '
                                                                                       'self.write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard( '
                                                                                       'manifest, '
                                                                                       'true, )?; '
                                                                                       'self.consume_native_amx_publication_component_after_durable_publication( '
                                                                                       'receipt, '
                                                                                       'NativeAmxPublicationComponent::Manifest, '
                                                                                       'manifest.encode_framed()?.len(), '
                                                                                       ')?; }',
                                                                                       'self.read_back_native_amx_repair_target_manifests_under_publication_guard( '
                                                                                       'plan, '
                                                                                       'target_indices, '
                                                                                       ')?;',
                                                                                       'for &index '
                                                                                       'in '
                                                                                       'target_indices '
                                                                                       '{ let '
                                                                                       '(manifest, '
                                                                                       'receipt) = '
                                                                                       '&plan.artifacts[index]; '
                                                                                       'self.write_native_amx_participant_application_receipt_artifact_only_with_retention_policy_under_publication_guard( '
                                                                                       'receipt, '
                                                                                       'manifest, '
                                                                                       'true, )?; '
                                                                                       'self.consume_native_amx_publication_component_after_durable_publication( '
                                                                                       'receipt, '
                                                                                       'NativeAmxPublicationComponent::Receipt, '
                                                                                       'receipt.encode_framed()?.len(), '
                                                                                       ')?; }',
                                                                                       'for '
                                                                                       '(&index, '
                                                                                       'preflight) '
                                                                                       'in '
                                                                                       'target_indices.iter().zip(route_preflights.iter()) '
                                                                                       '{ let '
                                                                                       '(manifest, '
                                                                                       'receipt) = '
                                                                                       '&plan.artifacts[index]; '
                                                                                       'self.write_native_amx_participant_receipt_latest_index_for_prepublication_under_publication_guard( '
                                                                                       'receipt, '
                                                                                       'manifest, '
                                                                                       'true, '
                                                                                       'preflight, '
                                                                                       ')?; '
                                                                                       'self.consume_native_amx_publication_component_after_durable_publication( '
                                                                                       'receipt, '
                                                                                       'NativeAmxPublicationComponent::Latest, '
                                                                                       'norito::encode_canonical(&NativeAmxParticipantReceiptLatestIndexV2::from_receipt( '
                                                                                       'receipt, '
                                                                                       '))? '
                                                                                       '.len(), '
                                                                                       ')?; }',
                                                                                       'for &index '
                                                                                       'in '
                                                                                       'target_indices '
                                                                                       '{ let '
                                                                                       '(manifest, '
                                                                                       'receipt) = '
                                                                                       '&plan.artifacts[index]; '
                                                                                       'let _ = '
                                                                                       'self.authenticate_native_amx_participant_application_prepublication_under_publication_guard( '
                                                                                       'manifest, '
                                                                                       'receipt, '
                                                                                       'true, )?; '
                                                                                       '}',
                                                                                       'for &index '
                                                                                       'in '
                                                                                       'target_indices '
                                                                                       '{ let (_, '
                                                                                       'receipt) = '
                                                                                       '&plan.artifacts[index]; '
                                                                                       'self.cleanup_native_amx_participant_application_evidence_under_publication_guard( '
                                                                                       'receipt, '
                                                                                       ')?; }',
                                                                                       'Ok(target_indices.len())')}

PREFIX_ORDER = {'persist_native_amx_participant_application_evidence_under_publication_guard': ('self.preflight_native_amx_participant_application_plan_under_publication_guard(plan)?;',
                                                                                 'let all_targets '
                                                                                 '= '
                                                                                 '(0..plan.artifacts.len()).collect::<Vec<_>>();',
                                                                                 'let '
                                                                                 'publication_required '
                                                                                 '= self '
                                                                                 '.ensure_native_amx_publication_capacity_under_publication_guard( '
                                                                                 'block, plan, '
                                                                                 '&all_targets, '
                                                                                 ')?;'),
 'persist_native_amx_participant_application_repair_targets_under_publication_guard': ('preflight_native_amx_participant_application_repair_targets_under_publication_guard( '
                                                                                       'plan, '
                                                                                       'target_indices, '
                                                                                       ')?;',
                                                                                       'let '
                                                                                       'publication_required '
                                                                                       '= self '
                                                                                       '.ensure_native_amx_publication_capacity_under_publication_guard( '
                                                                                       'block, '
                                                                                       'plan, '
                                                                                       'target_indices, '
                                                                                       ')?;')}

PUBLICATION_CALLS = ('write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard(',
 'write_native_amx_participant_application_receipt_artifact_only_with_retention_policy_under_publication_guard(',
 'write_native_amx_participant_receipt_latest_index_for_prepublication_under_publication_guard(')

ORDERED_OWNERS = (('plan_native_amx_evidence_pair_prune_locked',
  ('Self::plan_native_amx_evidence_prune_intent_from_artifacts(',
   'self.derive_native_amx_evidence_prune_protected_latest_locked(',
   'intent.protected_latest != protected_latest',
   'Hash::new(bytes) != removal.artifact_hash',
   'Ok(plan)')),
 ('prune_native_amx_evidence_pairs_locked',
  ('self.complete_native_amx_evidence_prune_intent_locked(batch.guard(), entry, namespace)?;',
   'self.recover_native_amx_evidence_publication_temp_locked(',
   'self.plan_native_amx_evidence_pair_prune_locked(entry, namespace, &inventory)?',
   'self.validate_native_amx_evidence_prune_intent_locked(entry, namespace, &intent)?;',
   'let bytes = norito::encode_canonical(&intent)?;',
   'self.publish_bound_noclobber_file_locked(',
   'self.complete_native_amx_evidence_prune_intent_locked( publication.guard(), entry, namespace, '
   ')?;',
   'publication.finish();')),
 ('rebuild_native_amx_participant_receipt_latest_indexes_on_startup',
  ('let latest_temp_present = self',
   'require_native_amx_latest_index_temp_recovery_unambiguous_locked',
   'self.complete_native_amx_evidence_prune_intent_locked( recovery.guard(), &entry, &namespace, '
   ')?;',
   'self.recover_native_amx_evidence_publication_temp_locked( recovery.guard(), &entry, '
   '&namespace, NativeAmxEvidenceRecoveryPhase::Startup, )?;',
   'let inventory = self.inventory_native_amx_evidence_files_locked',
   'let mut validated_manifests = BTreeMap::new()',
   'let mut validated_receipts = BTreeMap::new()',
   'Self::validate_native_amx_retained_history_continuity(',
   'let mut authenticated_complete = BTreeMap::new()',
   'self.reconcile_native_amx_latest_index_temp_locked(',
   'let current = self.decode_bound_native_amx_participant_receipt_latest_index_locked',
   'match (expected, current)',
   'self.prune_native_amx_evidence_pairs_locked( lane_resources.guard(), &entry, &namespace, )?;')))

COMPLETED_CAPACITY_BRANCH = ('let existing = self .native_amx_publication_capacity_reservations .lock() .get(&carrier) '
 '.cloned(); if let Some(existing) = existing { for (route, capacity) in existing.routes { '
 'plan.routes.entry(route).or_insert(capacity); } } else if !plan.routes.is_empty() && '
 'self.native_amx_publication_plan_is_durably_complete_under_prune_and_canonical_guards(carrier, '
 '&plan)? { return Ok(false); } else if target_indices.len() != evidence.artifacts.len() '
 '&& Self::read_native_amx_publication_index_for_store(&self.store_root)? '
 '.records.get(&carrier).is_some_and(|record| record.origin == NativeAmxPublicationIndexOriginV1::CanonicalWrite) '
 '{ // An unfinished original write still needs its complete // carrier owner. Only separately authenticated completed '
 '// repair may prove every non-target already terminal. return Err(Error::PruneIntentConflict( "Native AMX targeted '
 'repair lacks the complete carrier reservation".to_owned(), )); }')

RECEIPT_MEMBERSHIP_GUARD = 'if settlement_source_ids != artifact.source_ids || artifact .source_ids .iter() .copied() .collect::<BTreeSet<_>>() .len() != artifact.source_ids.len() || artifact.entrypoint_indices.is_empty() || artifact .entrypoint_indices .windows(2) .any(|pair| pair[0] >= pair[1]) || artifact.source_ids.len() != artifact.entrypoint_indices.len() || artifact.entrypoint_hashes.len() != artifact.entrypoint_indices.len() || artifact.result_hashes.len() != artifact.entrypoint_indices.len() || artifact.results.len() != artifact.entrypoint_indices.len() { return Err("Native AMX participant control/result membership is malformed"); }'

COMPLETED_JOIN = ('fn native_amx_publication_plan_is_durably_complete_under_prune_and_canonical_guards( &self, '
 'carrier: NativeAmxPublicationCarrier, plan: &NativeAmxPublicationCapacityReservation, ) -> '
 'Result<bool> { if plan .routes .values() .any(|route| !route.outstanding_components.is_empty() '
 '|| route.prune_journal_bytes != 0) { return Ok(false); } let _geometry = '
 'self.lane_geometry_lock.lock(); let _sidecar = self.sidecar_lock.lock(); if '
 'Self::read_native_amx_publication_index_for_store(&self.store_root)? .records '
 '.contains_key(&carrier) { return Ok(false); } for (route, capacity) in &plan.routes { let entry '
 '= self.lane_storage_entry(route.lane_id)?; if entry.dataspace_id != route.dataspace_id { return '
 'Err(Error::PruneIntentConflict( "Native AMX completed retry changed its dataspace".to_owned(), '
 ')); } let namespace = self.native_amx_evidence_namespace_for_entry(&entry)?; let inventory = '
 'self.inventory_native_amx_evidence_files_locked(&namespace, false)?; '
 'self.require_native_amx_evidence_prune_intent_absent_locked(&namespace)?; '
 'self.require_native_amx_latest_index_temp_absent_locked(&namespace)?; let protected = '
 'self.derive_native_amx_evidence_prune_protected_latest_locked( &entry, &namespace, &inventory, '
 ')?; let latest = protected.identity; if latest.application_block_height != carrier.height || '
 'latest.application_block_hash != carrier.block_hash || latest.executed_block_wire_hash != '
 'carrier.executed_wire_hash || latest.lane_incarnation != route.incarnation || '
 'latest.lane_block_height != capacity.participant_height || latest.participant_proposal_hash != '
 'capacity.proposal_hash || latest.participant_settlement_hash != capacity.settlement_hash { '
 'return Err(Error::PruneIntentConflict( "Native AMX completed retry differs from exact latest '
 'authority".to_owned(), )); } let manifest_file = inventory .manifests '
 '.get(&capacity.participant_height) .ok_or_else(|| { Error::PruneIntentConflict( "Native AMX '
 'completed retry lacks its manifest".to_owned(), ) })?; let receipt_file = inventory .receipts '
 '.get(&capacity.participant_height) .ok_or_else(|| { Error::PruneIntentConflict( "Native AMX '
 'completed retry lacks its receipt".to_owned(), ) })?; let manifest = '
 'self.decode_native_amx_manifest_file_locked(&entry, &namespace, manifest_file)?; let receipt = '
 'self.decode_native_amx_receipt_file_locked(&entry, &namespace, receipt_file)?; if '
 '!self.native_amx_publication_wsv_join_is_complete_locked(&manifest, &receipt)? { return '
 'Ok(false); } self.sync_native_amx_evidence_namespace( &namespace, "Native AMX completed '
 'publication retry", )?; self.inventory_native_amx_evidence_files_locked(&namespace, false)?; } '
 '// An earlier exact unlink may have succeeded before its directory sync // failed. Re-establish '
 'that barrier before releasing any surviving map pin. let record = self '
 '.native_amx_publication_capacity_reservations .lock() .get(&carrier) .and_then(|reservation| '
 'reservation.index_record.clone()); if let Some(record) = record { '
 'self.remove_native_amx_publication_index_exact_locked(&record)?; } '
 'self.native_amx_publication_capacity_reservations .lock() .remove(&carrier); Ok(true) }')
