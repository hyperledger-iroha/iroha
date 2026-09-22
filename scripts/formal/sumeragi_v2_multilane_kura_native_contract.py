"""Current Kura Native owners and branch-specific durable publication order.

These are structural source contracts, not native execution or release evidence.
The completed-retry branch is admitted only by the original all-route durable
join; fresh publication consumes each reservation after its durable writer.
"""
from __future__ import annotations

import re

from sumeragi_v2_multilane_geometry_evidence_contract import _code

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

    for path, kind, symbol, expected in REPAIR_PREFIX_EXACT:
        current = owner(path, kind, symbol)
        if current is not None and _code(current) != _code(expected):
            errors.append(f'{path}: repair-prefix executable owner {symbol} changed its authenticated cleanup relation')
    for path, kind, symbol, tokens in REPAIR_PREFIX_ORDER:
        current = owner(path, kind, symbol)
        if current is not None:
            value = _code(current)
            cursor = -1
            for token in tokens:
                token = _code(token)
                position = value.find(token, cursor + 1)
                if value.count(token) != 1 or position < 0:
                    errors.append(f'{path}: repair-prefix ordered owner {symbol} lost original preflight/cleanup ordering')
                    break
                cursor = position

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
                                                                                'NativeAmxParticipantApplicationPrepublicationToken::from_plan( self.instance_identity(), plan, '
                                                                                'identities, ) '
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


# Original CompletedRepair authority precedes exact prefix cleanup; ordinary inventory stays strict.
INDEX = 'crates/iroha_core/src/kura/native_amx_publication_index.rs'
PREFIX = 'crates/iroha_core/src/kura/native_amx_repair_prefix.rs'
# One reviewed body supplies both the ledger row and executable equality check.
REPAIR_PREFIX_OWNERS = (('crates/iroha_core/src/kura.rs',
  'fn',
  'inventory_native_amx_evidence_files_locked',
  (),
  '    fn inventory_native_amx_evidence_files_locked(\n'
  '        &self,\n'
  '        namespace: &BoundProgressNamespace,\n'
  '        allow_transient: bool,\n'
  '    ) -> Result<NativeAmxEvidenceInventory> {\n'
  '        self.inventory_native_amx_evidence_with_indexed_prefix_locked(\n'
  '            namespace,\n'
  '            allow_transient,\n'
  '            None,\n'
  '        )\n'
  '    }'),
 ('crates/iroha_core/src/kura.rs',
  'fn',
  'inventory_native_amx_evidence_with_indexed_prefix_locked',
  ('parse_native_amx_evidence_path',
   'regular_sidecar_metadata_for',
   'STRICT_INIT_MAX_BLOCK_BYTES',
   'native_amx_participant_evidence_retention',
   'native_amx_participant_evidence_startup_bytes',
   'native_amx_participant_evidence_file_bytes',
   'native_amx_evidence_total_payload_bytes',
   'shared aggregate byte bound',
   'stable_sidecar_metadata_unchanged',
   'progress_mutation_namespace_unchanged'),
  '    fn inventory_native_amx_evidence_with_indexed_prefix_locked(\n'
  '        &self,\n'
  '        namespace: &BoundProgressNamespace,\n'
  '        allow_transient: bool,\n'
  '        indexed_prefix: Option<&NativeAmxEvidenceFile>,\n'
  '    ) -> Result<NativeAmxEvidenceInventory> {\n'
  '        if !Self::progress_mutation_namespace_unchanged(namespace) {\n'
  '            return Err(Self::invalid_lane_artifact_error(\n'
  '                namespace.data_path.clone(),\n'
  '                "Native AMX evidence namespace changed before inventory",\n'
  '            ));\n'
  '        }\n'
  '        let directory = namespace.data_path.parent().ok_or_else(|| {\n'
  '            Self::invalid_lane_artifact_error(\n'
  '                namespace.data_path.clone(),\n'
  '                "Native AMX evidence namespace has no directory",\n'
  '            )\n'
  '        })?;\n'
  '        let mut inventory = NativeAmxEvidenceInventory::default();\n'
  '        let entries = std::fs::read_dir(directory)\n'
  '            .map_err(|error| Error::IO(error, directory.to_path_buf()))?;\n'
  '        for entry in entries {\n'
  '            let entry = entry.map_err(|error| Error::IO(error, directory.to_path_buf()))?;\n'
  '            let path = entry.path();\n'
  '            let Some((kind, participant_height, temporary)) =\n'
  '                Self::parse_native_amx_evidence_path(&path)?\n'
  '            else {\n'
  '                continue;\n'
  '            };\n'
  '            let metadata = Self::regular_sidecar_metadata_for(&self.store_root, &path, directory)?\n'
  '                .ok_or_else(|| {\n'
  '                    Self::invalid_lane_artifact_error(\n'
  '                        path.clone(),\n'
  '                        "Native AMX evidence disappeared during bounded inventory",\n'
  '                    )\n'
  '                })?;\n'
  '            let len = metadata.file.len();\n'
  '            let owned_prefix = indexed_prefix.is_some_and(|candidate| {\n'
  '                allow_transient\n'
  '                    && temporary\n'
  '                    && candidate.kind == kind\n'
  '                    && candidate.path == path\n'
  '                    && candidate.participant_height == participant_height\n'
  '                    && Self::stable_sidecar_metadata_unchanged(&candidate.metadata, &metadata)\n'
  '            });\n'
  '            if (len == 0 && !owned_prefix)\n'
  '                || len > STRICT_INIT_MAX_BLOCK_BYTES\n'
  '                || len > self.native_amx_participant_evidence_file_bytes()\n'
  '            {\n'
  '                return Err(Self::invalid_lane_artifact_error(\n'
  '                    path,\n'
  '                    format!(\n'
  '                        "{} has an empty or oversized standalone payload for the shared stable budget",\n'
  '                        kind.label()\n'
  '                    ),\n'
  '                ));\n'
  '            }\n'
  '            let file = NativeAmxEvidenceFile {\n'
  '                kind,\n'
  '                participant_height,\n'
  '                path: path.clone(),\n'
  '                metadata,\n'
  '            };\n'
  '            if temporary {\n'
  '                if inventory.temporaries.insert(kind, file).is_some() {\n'
  '                    return Err(Self::invalid_lane_artifact_error(\n'
  '                        directory.to_path_buf(),\n'
  '                        format!(\n'
  '                            "{} retains more than one publication temporary",\n'
  '                            kind.label()\n'
  '                        ),\n'
  '                    ));\n'
  '                }\n'
  '                continue;\n'
  '            }\n'
  '            let (stable, aggregate) = match kind {\n'
  '                NativeAmxEvidenceKind::Manifest => (\n'
  '                    &mut inventory.manifests,\n'
  '                    &mut inventory.manifest_stable_bytes,\n'
  '                ),\n'
  '                NativeAmxEvidenceKind::Receipt => {\n'
  '                    (&mut inventory.receipts, &mut inventory.receipt_stable_bytes)\n'
  '                }\n'
  '            };\n'
  '            if stable.insert(participant_height, file).is_some() {\n'
  '                return Err(Self::invalid_lane_artifact_error(\n'
  '                    path,\n'
  '                    format!(\n'
  '                        "{} repeats participant height {participant_height}",\n'
  '                        kind.label()\n'
  '                    ),\n'
  '                ));\n'
  '            }\n'
  '            *aggregate = aggregate.checked_add(len).ok_or_else(|| {\n'
  '                Self::invalid_lane_artifact_error(\n'
  '                    directory.to_path_buf(),\n'
  '                    format!("{} aggregate byte count overflowed", kind.label()),\n'
  '                )\n'
  '            })?;\n'
  '        }\n'
  '        let stable_entry_limit = self\n'
  '            .native_amx_participant_evidence_retention()\n'
  '            .get()\n'
  '            .checked_add(usize::from(allow_transient))\n'
  '            .ok_or_else(|| {\n'
  '                Self::invalid_lane_artifact_error(\n'
  '                    directory.to_path_buf(),\n'
  '                    "Native AMX evidence entry bound overflowed",\n'
  '                )\n'
  '            })?;\n'
  '        let aggregate_limit = if allow_transient {\n'
  '            self.native_amx_participant_evidence_startup_bytes()?\n'
  '        } else {\n'
  '            self.native_amx_participant_evidence_file_bytes()\n'
  '        };\n'
  '        for kind in [\n'
  '            NativeAmxEvidenceKind::Manifest,\n'
  '            NativeAmxEvidenceKind::Receipt,\n'
  '        ] {\n'
  '            let temporary_count = usize::from(\n'
  '                inventory\n'
  '                    .temporary(kind)\n'
  '                    .is_some_and(|file| file.kind == kind),\n'
  '            );\n'
  '            if inventory\n'
  '                .stable(kind)\n'
  '                .len()\n'
  '                .checked_add(temporary_count)\n'
  '                .is_none_or(|count| count > stable_entry_limit)\n'
  '            {\n'
  '                return Err(Self::invalid_lane_artifact_error(\n'
  '                    directory.to_path_buf(),\n'
  '                    format!("{} exceeds its retained record bound", kind.label()),\n'
  '                ));\n'
  '            }\n'
  '        }\n'
  '        if Self::native_amx_evidence_total_payload_bytes(&inventory)\n'
  '            .is_none_or(|bytes| bytes > aggregate_limit)\n'
  '        {\n'
  '            return Err(Self::invalid_lane_artifact_error(\n'
  '                directory.to_path_buf(),\n'
  '                "Native AMX manifests, receipts, and temporaries exceed their shared aggregate byte '
  'bound",\n'
  '            ));\n'
  '        }\n'
  '        if !allow_transient && !inventory.temporaries.is_empty() {\n'
  '            return Err(Self::invalid_lane_artifact_error(\n'
  '                directory.to_path_buf(),\n'
  '                "Native AMX evidence retains an unresolved publication temporary",\n'
  '            ));\n'
  '        }\n'
  '        for file in inventory\n'
  '            .manifests\n'
  '            .values()\n'
  '            .chain(inventory.receipts.values())\n'
  '            .chain(inventory.temporaries.values())\n'
  '        {\n'
  '            let current =\n'
  '                Self::regular_sidecar_metadata_for(&self.store_root, &file.path, directory)?\n'
  '                    .ok_or_else(|| {\n'
  '                        Self::invalid_lane_artifact_error(\n'
  '                            file.path.clone(),\n'
  '                            "Native AMX evidence disappeared after bounded inventory",\n'
  '                        )\n'
  '                    })?;\n'
  '            if !Self::stable_sidecar_metadata_unchanged(&file.metadata, &current) {\n'
  '                return Err(Self::invalid_lane_artifact_error(\n'
  '                    file.path.clone(),\n'
  '                    "Native AMX evidence changed during bounded inventory",\n'
  '                ));\n'
  '            }\n'
  '        }\n'
  '        if !Self::progress_mutation_namespace_unchanged(namespace) {\n'
  '            return Err(Self::invalid_lane_artifact_error(\n'
  '                directory.to_path_buf(),\n'
  '                "Native AMX evidence namespace changed during inventory",\n'
  '            ));\n'
  '        }\n'
  '        Ok(inventory)\n'
  '    }'),
 ('crates/iroha_core/src/kura/native_amx_publication_index.rs',
  'method',
  'Kura::require_native_amx_completed_repair_receipt_at_target_locked',
  (),
  '    fn require_native_amx_completed_repair_receipt_at_target_locked(\n'
  '        &self,\n'
  '        entry: &impl LaneArtifactStorageView,\n'
  '        receipt: &NativeAmxParticipantApplicationReceiptArtifact,\n'
  '        recovery: Option<(\n'
  '            &NativeAmxPublicationIndexRecord,\n'
  '            &NativeAmxParticipantApplicationManifestArtifactV1,\n'
  '        )>,\n'
  '    ) -> Result<()> {\n'
  '        let descriptor = &receipt.participant_proposal.descriptor;\n'
  '        self.require_active_lane_artifact(entry, descriptor)?;\n'
  '        let namespace = self.native_amx_evidence_namespace_for_entry(entry)?;\n'
  '        self.require_native_amx_evidence_prune_intent_absent_locked(&namespace)?;\n'
  '        let inventory = self.inventory_native_amx_evidence_files_locked(&namespace, true)?;\n'
  '        self.require_native_amx_completed_repair_receipt_with_inventory_locked(\n'
  '            entry, receipt, recovery, &namespace, &inventory,\n'
  '        )\n'
  '    }'),
 ('crates/iroha_core/src/kura/native_amx_publication_index.rs',
  'method',
  'Kura::require_native_amx_completed_repair_receipt_with_inventory_locked',
  (),
  '    fn require_native_amx_completed_repair_receipt_with_inventory_locked(\n'
  '        &self,\n'
  '        entry: &impl LaneArtifactStorageView,\n'
  '        receipt: &NativeAmxParticipantApplicationReceiptArtifact,\n'
  '        recovery: Option<(\n'
  '            &NativeAmxPublicationIndexRecord,\n'
  '            &NativeAmxParticipantApplicationManifestArtifactV1,\n'
  '        )>,\n'
  '        namespace: &BoundProgressNamespace,\n'
  '        inventory: &NativeAmxEvidenceInventory,\n'
  '    ) -> Result<()> {\n'
  '        let descriptor = &receipt.participant_proposal.descriptor;\n'
  '        let file = inventory.receipts.get(&descriptor.lane_block_height)\n'
  '            .ok_or_else(|| Error::PruneIntentConflict("Native unfinished publication lacks its exact '
  'pending index and retained receipt".to_owned()))?;\n'
  '        if self.decode_native_amx_receipt_file_locked(entry, &namespace, file)? != *receipt {\n'
  '            return Err(Error::PruneIntentConflict(\n'
  '                "Native completed repair lacks exact stable receipt custody".to_owned(),\n'
  '            ));\n'
  '        }\n'
  '        if !inventory.temporaries.is_empty() {\n'
  '            let Some((record, manifest)) = recovery else {\n'
  '                return Err(Error::PruneIntentConflict(\n'
  '                    "Native new completed repair cannot adopt unowned publication temporaries"\n'
  '                        .to_owned(),\n'
  '                ));\n'
  '            };\n'
  '            record\n'
  '                .validate()\n'
  '                .map_err(|message| Error::PruneIntentConflict(message.to_owned()))?;\n'
  '            if record.origin != NativeAmxPublicationIndexOriginV1::CompletedRepair\n'
  '                || record.carrier.height != manifest.leaf.application_block_height\n'
  '                || record.carrier.block_hash != manifest.leaf.application_block_hash\n'
  '                || record.carrier.executed_wire_hash != manifest.leaf.executed_block_wire_hash\n'
  '                || HashOf::new(manifest) != receipt.manifest_artifact_hash\n'
  '                || manifest.finality_artifact_hash != receipt.finality_artifact_hash\n'
  '                || !Self::native_amx_participant_receipt_matches_manifest_leaf(\n'
  '                    receipt,\n'
  '                    &manifest.leaf,\n'
  '                )\n'
  '            {\n'
  '                return Err(Error::PruneIntentConflict(\n'
  '                    "Native repair temporary lacks its exact retained index and artifact join"\n'
  '                        .to_owned(),\n'
  '                ));\n'
  '            }\n'
  '            for temporary in inventory.temporaries.values() {\n'
  '                let expected = match temporary.kind {\n'
  '                    NativeAmxEvidenceKind::Manifest => manifest.encode_framed()?,\n'
  '                    NativeAmxEvidenceKind::Receipt => receipt.encode_framed()?,\n'
  '                };\n'
  '                // Inventory binds canonical filename, route and physical identity;\n'
  '                // this read rechecks that same object, not a path-only replacement.\n'
  '                if temporary.participant_height != descriptor.lane_block_height\n'
  '                    || self.read_native_amx_evidence_file_bytes_locked(&namespace, temporary)?\n'
  '                        != expected\n'
  '                {\n'
  '                    return Err(Error::PruneIntentConflict(\n'
  '                        "Native repair temporary differs from its exact authenticated canonical '
  'artifact".to_owned(),\n'
  '                    ));\n'
  '                }\n'
  '            }\n'
  '        }\n'
  '        let latest_path = Self::native_amx_participant_receipt_latest_index_path_for_entry(\n'
  '            entry,\n'
  '            &self.store_root,\n'
  '        );\n'
  '        self.require_native_amx_latest_index_temp_absent_locked(&namespace)?;\n'
  '        let latest = self.decode_bound_native_amx_participant_receipt_latest_index_locked(\n'
  '            entry,\n'
  '            &latest_path,\n'
  '            &namespace,\n'
  '        )?;\n'
  '        if latest\n'
  '            != Some(NativeAmxParticipantReceiptLatestIndexV2::from_receipt(\n'
  '                receipt,\n'
  '            ))\n'
  '        {\n'
  '            return Err(Error::PruneIntentConflict(\n'
  '                "Native completed repair lacks its exact published receipt pointer".to_owned(),\n'
  '            ));\n'
  '        }\n'
  '        Ok(())\n'
  '    }'),
 ('crates/iroha_core/src/kura/native_amx_publication_index.rs',
  'method',
  'Kura::native_amx_indexed_publication_artifacts_under_prune_and_canonical_guards',
  (),
  '    fn native_amx_indexed_publication_artifacts_under_prune_and_canonical_guards(\n'
  '        &self,\n'
  '        block: &SignedBlock,\n'
  '        merge: Option<&MergeLedgerEntry>,\n'
  '        record: &NativeAmxPublicationIndexRecord,\n'
  '    ) -> Result<\n'
  '        Vec<(\n'
  '            NativeAmxParticipantApplicationManifestArtifactV1,\n'
  '            NativeAmxParticipantApplicationReceiptArtifact,\n'
  '        )>,\n'
  '    > {\n'
  '        let carrier = Self::native_amx_publication_carrier(block)?;\n'
  '        record\n'
  '            .validate()\n'
  '            .map_err(|message| Error::PruneIntentConflict(message.to_owned()))?;\n'
  '        if record.carrier != carrier\n'
  '            || record.merge_entry_hash != merge.map(MergeLedgerEntry::canonical_hash)\n'
  '        {\n'
  '            return Err(Error::PruneIntentConflict(\n'
  '                "Native indexed publication startup differs from its retained index".to_owned(),\n'
  '            ));\n'
  '        }\n'
  '        let height = NonZeroUsize::new(usize::try_from(carrier.height)?).ok_or_else(|| {\n'
  '            Error::PruneIntentConflict(\n'
  '                "Native indexed publication has zero startup height".to_owned(),\n'
  '            )\n'
  '        })?;\n'
  '        let selected = self\n'
  '            .read_block_body_under_prune_and_canonical_guards(height)?\n'
  '            .ok_or_else(|| {\n'
  '                Error::PruneIntentConflict(\n'
  '                    "Native indexed publication startup lacks its authenticated canonical body"\n'
  '                        .to_owned(),\n'
  '                )\n'
  '            })?;\n'
  '        if Self::native_amx_publication_carrier(&selected)? != carrier {\n'
  '            return Err(Error::PruneIntentConflict(\n'
  '                "Native indexed publication startup changed canonical executed wire".to_owned(),\n'
  '            ));\n'
  '        }\n'
  '        let (_, finality, _) = self\n'
  '            .v2_finality_artifact_with_archive_under_prune_and_canonical_guards(carrier.height)?\n'
  '            .ok_or(Error::MissingV2FinalityArtifact {\n'
  '                height: carrier.height,\n'
  '            })?;\n'
  '        let manifest = '
  'crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(block, '
  'merge)\n'
  '            .map_err(|error| Error::PruneIntentConflict(format!("Native indexed publication startup '
  'manifest: {error}")))?;\n'
  '        let artifacts =\n'
  '            native_amx_participant_application_artifacts(&manifest, HashOf::new(&finality))\n'
  '                .filter(|artifacts| !artifacts.is_empty())\n'
  '                .ok_or_else(|| {\n'
  '                    Error::PruneIntentConflict(\n'
  '                        "Native indexed publication startup has no exact artifact plan".to_owned(),\n'
  '                    )\n'
  '                })?;\n'
  '        Ok(artifacts)\n'
  '    }'),
 ('crates/iroha_core/src/kura/native_amx_publication_index.rs',
  'method',
  'Kura::authenticate_native_amx_completed_repair_on_startup',
  (),
  '    fn authenticate_native_amx_completed_repair_on_startup(\n'
  '        &self,\n'
  '        block: &SignedBlock,\n'
  '        merge: Option<&MergeLedgerEntry>,\n'
  '        record: &NativeAmxPublicationIndexRecord,\n'
  '    ) -> Result<()> {\n'
  '        if record.origin != NativeAmxPublicationIndexOriginV1::CompletedRepair {\n'
  '            return Err(Error::PruneIntentConflict(\n'
  '                "Native completed repair requires its original repair locator".to_owned(),\n'
  '            ));\n'
  '        }\n'
  '        let artifacts = self\n'
  '            .native_amx_indexed_publication_artifacts_under_prune_and_canonical_guards(\n'
  '                block, merge, record,\n'
  '            )?;\n'
  '        let _geometry = self.lane_geometry_lock.lock();\n'
  '        let _sidecar = self.sidecar_lock.lock();\n'
  '        for (manifest, receipt) in artifacts {\n'
  '            if !self.native_amx_publication_wsv_join_is_complete_locked(&manifest, &receipt)? {\n'
  '                return Err(Error::PruneIntentConflict(\n'
  '                    "Native completed repair startup lost its finalized WSV join".to_owned(),\n'
  '                ));\n'
  '            }\n'
  '            let target = self.native_amx_reservation_physical_target_from_journal(\n'
  '                &receipt.participant_proposal.descriptor,\n'
  '            )?;\n'
  '            // An independently authenticated later completed pointer is an\n'
  '            // existing terminal proof, not authority to republish old evidence.\n'
  '            if self\n'
  '                .native_amx_route_publication_capacity_at_target_locked(\n'
  '                    &target, &manifest, &receipt,\n'
  '                )?\n'
  '                .is_some()\n'
  '            {\n'
  '                self.require_native_amx_completed_repair_receipt_at_target_locked(\n'
  '                    &target,\n'
  '                    &receipt,\n'
  '                    Some((record, &manifest)),\n'
  '                )?;\n'
  '            }\n'
  '            self.require_native_amx_reservation_physical_target(&target)?;\n'
  '        }\n'
  '        Ok(())\n'
  '    }'),
 ('crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'struct',
  'NativeAmxIndexedPublicationPrefix',
  (),
  'struct NativeAmxIndexedPublicationPrefix {\n'
  '    target: lane_geometry::NativeAmxReservationPhysicalTarget,\n'
  '    namespace: BoundProgressNamespace,\n'
  '    file: NativeAmxIndexedPrefixFile,\n'
  '}'),
 ('crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_publication_guard',
  (),
  '    fn recover_native_amx_indexed_publication_prefixes_under_publication_guard(\n'
  '        &self,\n'
  '        block: &SignedBlock,\n'
  '    ) -> Result<()> {\n'
  '        let _canonical = self.canonical_chain_lock.lock();\n'
  '        self.recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards(\n'
  '            &[Self::native_amx_publication_carrier(block)?],\n'
  '            NativeAmxPrefixRecoveryScope::Indexed,\n'
  '        )\n'
  '    }'),
 ('crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards',
  (),
  '    fn recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards(\n'
  '        &self,\n'
  '        carriers: &[NativeAmxPublicationCarrier],\n'
  '        scope: NativeAmxPrefixRecoveryScope,\n'
  '    ) -> Result<()> {\n'
  '        let index = Self::read_native_amx_publication_index_for_store(&self.store_root)?;\n'
  '        let selected_marker = {\n'
  '            let mut store = self.block_store.lock();\n'
  '            let count = store.read_exact_durable_index_count()?;\n'
  '            store.commit_marker_for_count(count)?\n'
  '        };\n'
  '        let mut authenticated = Vec::new();\n'
  '        for carrier in carriers.iter().copied().collect::<BTreeSet<_>>() {\n'
  '            let Some(record) = index.records.get(&carrier) else {\n'
  '                continue;\n'
  '            };\n'
  '            let height = NonZeroUsize::new(usize::try_from(carrier.height)?).ok_or_else(|| {\n'
  '                Error::PruneIntentConflict(\n'
  '                    "Native indexed prefix has zero carrier height".to_owned(),\n'
  '                )\n'
  '            })?;\n'
  '            let block = self\n'
  '                .get_block_without_merge_sidecar(height)\n'
  '                .ok_or_else(|| {\n'
  '                    Error::PruneIntentConflict(\n'
  '                        "Native indexed prefix lost its canonical carrier".to_owned(),\n'
  '                    )\n'
  '                })?;\n'
  '            if record.classify_resolved_carrier(\n'
  '                &selected_marker,\n'
  '                Some(Self::native_amx_publication_carrier(&block)?),\n'
  '            )? != NativeAmxPublicationIndexResolution::Committed\n'
  '            {\n'
  '                return Err(Error::PruneIntentConflict(\n'
  '                    "Native indexed prefix lacks committed selected-wire authority".to_owned(),\n'
  '                ));\n'
  '            }\n'
  '            let merge =\n'
  '                self.native_amx_capacity_merge_entry_under_prune_and_canonical_guards(&block)?;\n'
  '            if record.merge_entry_hash != merge.as_ref().map(MergeLedgerEntry::canonical_hash) {\n'
  '                return Err(Error::PruneIntentConflict(\n'
  '                    "Native indexed prefix changed its original merge association".to_owned(),\n'
  '                ));\n'
  '            }\n'
  '            if record.origin == NativeAmxPublicationIndexOriginV1::CanonicalWrite\n'
  '                && !self\n'
  '                    .native_amx_indexed_publication_has_temporary_under_prune_and_canonical_guards(\n'
  '                        &block,\n'
  '                        merge.as_ref(),\n'
  '                    )?\n'
  '            {\n'
  '                // No derived writer ran: preserve the legitimate pre-finality cut.\n'
  '                continue;\n'
  '            }\n'
  '            let artifacts = self\n'
  '                .native_amx_indexed_publication_artifacts_under_prune_and_canonical_guards(\n'
  '                    &block,\n'
  '                    merge.as_ref(),\n'
  '                    record,\n'
  '                )?;\n'
  '            authenticated.push((record, artifacts));\n'
  '        }\n'
  '        if authenticated.is_empty() && scope == NativeAmxPrefixRecoveryScope::Indexed {\n'
  '            return Ok(());\n'
  '        }\n'
  '        let indexed_routes = authenticated\n'
  '            .iter()\n'
  '            .flat_map(|(_, artifacts)| {\n'
  '                artifacts.iter().map(|(_, receipt)| {\n'
  '                    let descriptor = &receipt.participant_proposal.descriptor;\n'
  '                    (descriptor.lane_id, descriptor.lane_incarnation)\n'
  '                })\n'
  '            })\n'
  '            .collect::<BTreeSet<_>>();\n'
  '        let _geometry = self.lane_geometry_lock.lock();\n'
  '        let _sidecar = self.sidecar_lock.lock();\n'
  '        let mut prefixes = Vec::new();\n'
  '        for (record, artifacts) in authenticated {\n'
  '            for (manifest, receipt) in artifacts {\n'
  '                if record.origin == NativeAmxPublicationIndexOriginV1::CompletedRepair\n'
  '                    && !self\n'
  '                        .native_amx_publication_wsv_join_is_complete_locked(&manifest, &receipt)?\n'
  '                {\n'
  '                    return Err(Error::PruneIntentConflict(\n'
  '                        "Native completed-repair prefix lacks its finalized WSV join".to_owned(),\n'
  '                    ));\n'
  '                }\n'
  '                if '
  '!self.native_amx_participant_application_manifest_matches_available_finality_under_prune_and_canonical_guards(&manifest) '
  '{\n'
  '                    return Err(Error::PruneIntentConflict("Native indexed prefix differs from '
  'authenticated finality".to_owned()));\n'
  '                }\n'
  '                let manifest_bytes = manifest.encode_framed()?;\n'
  '                let receipt_bytes = receipt.encode_framed()?;\n'
  '                if !self.native_amx_participant_evidence_pair_fits_stable_bytes(\n'
  '                    manifest_bytes.len(),\n'
  '                    receipt_bytes.len(),\n'
  '                ) {\n'
  '                    return Err(Error::PruneIntentConflict(\n'
  '                        "Native indexed prefix exceeds the authenticated pair byte bound"\n'
  '                            .to_owned(),\n'
  '                    ));\n'
  '                }\n'
  '                let descriptor = &receipt.participant_proposal.descriptor;\n'
  '                let target =\n'
  '                    self.native_amx_reservation_physical_target_from_journal(descriptor)?;\n'
  '                let namespace = self.native_amx_evidence_namespace_for_entry(&target)?;\n'
  '                self.require_active_lane_artifact(&target, descriptor)?;\n'
  '                self.require_native_amx_evidence_prune_intent_absent_locked(&namespace)?;\n'
  '                let manifest_path = Self::native_amx_application_manifest_path_for_entry(\n'
  '                    &target,\n'
  '                    &self.store_root,\n'
  '                    descriptor.lane_block_height,\n'
  '                );\n'
  '                let receipt_path = Self::native_amx_participant_receipt_path_for_entry(\n'
  '                    &target,\n'
  '                    &self.store_root,\n'
  '                    descriptor.lane_block_height,\n'
  '                );\n'
  '                let latest_path = Self::native_amx_participant_receipt_latest_index_path_for_entry(\n'
  '                    &target,\n'
  '                    &self.store_root,\n'
  '                );\n'
  '                let latest_temp = latest_path\n'
  '                    .parent()\n'
  '                    .expect("bound latest directory")\n'
  '                    .join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE);\n'
  '                let latest_bytes = norito::encode_canonical(\n'
  '                    &NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipt),\n'
  '                )?;\n'
  '                let mut route_prefix = None;\n'
  '                for (component, stable, path, expected) in [\n'
  '                    (\n'
  '                        NativeAmxPublicationComponent::Manifest,\n'
  '                        &manifest_path,\n'
  '                        manifest_path.with_extension("norito.tmp"),\n'
  '                        manifest_bytes.as_slice(),\n'
  '                    ),\n'
  '                    (\n'
  '                        NativeAmxPublicationComponent::Receipt,\n'
  '                        &receipt_path,\n'
  '                        receipt_path.with_extension("norito.tmp"),\n'
  '                        receipt_bytes.as_slice(),\n'
  '                    ),\n'
  '                    (\n'
  '                        NativeAmxPublicationComponent::Latest,\n'
  '                        &latest_path,\n'
  '                        latest_temp,\n'
  '                        latest_bytes.as_slice(),\n'
  '                    ),\n'
  '                ] {\n'
  '                    if let Some(prefix) = self.open_native_amx_indexed_publication_prefix_locked(\n'
  '                        &namespace, component, stable, &path, expected,\n'
  '                    )? {\n'
  '                        if route_prefix.replace(prefix).is_some() {\n'
  '                            return Err(Error::PruneIntentConflict(\n'
  '                                "Native indexed route has ambiguous simultaneous partial writes"\n'
  '                                    .to_owned(),\n'
  '                            ));\n'
  '                        }\n'
  '                    }\n'
  '                }\n'
  '                let evidence_prefix = route_prefix.as_ref().and_then(|prefix| {\n'
  '                    let kind = match prefix.component {\n'
  '                        NativeAmxPublicationComponent::Manifest => NativeAmxEvidenceKind::Manifest,\n'
  '                        NativeAmxPublicationComponent::Receipt => NativeAmxEvidenceKind::Receipt,\n'
  '                        NativeAmxPublicationComponent::Latest => return None,\n'
  '                    };\n'
  '                    Some(NativeAmxEvidenceFile {\n'
  '                        kind,\n'
  '                        participant_height: descriptor.lane_block_height,\n'
  '                        path: prefix.path.clone(),\n'
  '                        metadata: prefix.metadata.clone(),\n'
  '                    })\n'
  '                });\n'
  '                let mut inventory = self.inventory_native_amx_evidence_with_indexed_prefix_locked(\n'
  '                    &namespace,\n'
  '                    true,\n'
  '                    evidence_prefix.as_ref(),\n'
  '                )?;\n'
  '                // Bounds include the original prefix bytes before the strictly\n'
  '                // authenticated incomplete object is omitted from decode planning.\n'
  '                if let Some(temporary) = &evidence_prefix {\n'
  '                    let removed = inventory.temporaries.remove(&temporary.kind);\n'
  '                    if !removed.as_ref().is_some_and(|file| {\n'
  '                        file.path == temporary.path\n'
  '                            && Self::stable_sidecar_metadata_unchanged(\n'
  '                                &file.metadata,\n'
  '                                &temporary.metadata,\n'
  '                            )\n'
  '                    }) {\n'
  '                        return Err(Error::PruneIntentConflict(\n'
  '                            "Native indexed prefix differs from its bounded inventory".to_owned(),\n'
  '                        ));\n'
  '                    }\n'
  '                }\n'
  '                let latest_prefix = route_prefix\n'
  '                    .as_ref()\n'
  '                    .filter(|prefix| prefix.component == NativeAmxPublicationComponent::Latest);\n'
  '                // Use the original incoming-pair validator for every route. It\n'
  '                // checks complete/wrong temporaries and all retained continuity.\n'
  '                if record.origin == NativeAmxPublicationIndexOriginV1::CanonicalWrite\n'
  '                    || route_prefix.is_some()\n'
  '                {\n'
  '                    self.preflight_native_amx_incoming_artifacts_locked(\n'
  '                        &target, &namespace, &inventory, &manifest, &receipt,\n'
  '                    )?;\n'
  '                }\n'
  '                if self\n'
  '                    .native_amx_route_publication_capacity_with_inventory_locked(\n'
  '                        &target,\n'
  '                        &manifest,\n'
  '                        &receipt,\n'
  '                        Some((&namespace, &inventory)),\n'
  '                        latest_prefix,\n'
  '                    )?\n'
  '                    .is_some()\n'
  '                {\n'
  '                    if record.origin == NativeAmxPublicationIndexOriginV1::CompletedRepair {\n'
  '                        self.require_native_amx_completed_repair_receipt_with_inventory_locked(\n'
  '                            &target,\n'
  '                            &receipt,\n'
  '                            Some((record, &manifest)),\n'
  '                            &namespace,\n'
  '                            &inventory,\n'
  '                        )?;\n'
  '                    }\n'
  '                } else if route_prefix.is_some() {\n'
  '                    return Err(Error::PruneIntentConflict(\n'
  '                        "Native indexed prefix cannot rewrite a later published frontier"\n'
  '                            .to_owned(),\n'
  '                    ));\n'
  '                }\n'
  '                self.require_native_amx_reservation_physical_target(&target)?;\n'
  '                if let Some(file) = route_prefix {\n'
  '                    prefixes.push(NativeAmxIndexedPublicationPrefix {\n'
  '                        target,\n'
  '                        namespace,\n'
  '                        file,\n'
  '                    });\n'
  '                }\n'
  '            }\n'
  '        }\n'
  '        if scope == NativeAmxPrefixRecoveryScope::Startup {\n'
  '            self.collect_native_amx_completed_pair_latest_prefixes_locked(\n'
  '                &index,\n'
  '                &indexed_routes,\n'
  '                &mut prefixes,\n'
  '            )?;\n'
  '        }\n'
  '        // All authorities and routes passed without mutation. Retain exact file\n'
  '        // and directory objects through both this pass and every durable unlink.\n'
  '        for prefix in &mut prefixes {\n'
  '            self.require_native_amx_reservation_physical_target(&prefix.target)?;\n'
  '            self.verify_bound_open_regular_file_exact_bytes_locked(\n'
  '                &prefix.namespace,\n'
  '                &prefix.file.path,\n'
  '                &mut prefix.file.opened,\n'
  '                &prefix.file.metadata,\n'
  '                &prefix.file.prefix,\n'
  '                prefix.file.prefix.len(),\n'
  '                "Native indexed publication prefix",\n'
  '            )?;\n'
  '        }\n'
  '        if prefixes.is_empty() {\n'
  '            return Ok(());\n'
  '        }\n'
  '        self.durable_mutation_authorized()?;\n'
  '        let resources = self.begin_total_disk_usage_mutation().with_resource_paths(\n'
  '            prefixes\n'
  '                .iter()\n'
  '                .map(|prefix| prefix.file.path.clone())\n'
  '                .collect(),\n'
  '        );\n'
  '        for prefix in &mut prefixes {\n'
  '            self.verify_bound_open_regular_file_exact_bytes_after_namespace_mutation_locked(\n'
  '                &prefix.namespace,\n'
  '                &prefix.file.path,\n'
  '                &mut prefix.file.opened,\n'
  '                &prefix.file.metadata,\n'
  '                &prefix.file.prefix,\n'
  '                prefix.file.prefix.len(),\n'
  '                "Native indexed publication prefix",\n'
  '            )?;\n'
  '            Self::remove_bound_progress_file_if_matches(\n'
  '                &prefix.namespace,\n'
  '                &prefix.file.path,\n'
  '                &prefix.file.opened,\n'
  '                &prefix.file.metadata,\n'
  '            )\n'
  '            .map_err(|error| Error::IO(error, prefix.file.path.clone()))?;\n'
  '            self.sync_native_amx_evidence_namespace(\n'
  '                &prefix.namespace,\n'
  '                "Native indexed prefix removal",\n'
  '            )?;\n'
  '            self.require_native_amx_reservation_physical_target(&prefix.target)?;\n'
  '        }\n'
  '        resources.finish_resources_before_disk_rescan();\n'
  '        Ok(())\n'
  '    }'),
 ('crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::open_native_amx_indexed_publication_prefix_locked',
  (),
  '    fn open_native_amx_indexed_publication_prefix_locked(\n'
  '        &self,\n'
  '        namespace: &BoundProgressNamespace,\n'
  '        component: NativeAmxPublicationComponent,\n'
  '        stable: &Path,\n'
  '        path: &Path,\n'
  '        expected: &[u8],\n'
  '    ) -> Result<Option<NativeAmxIndexedPrefixFile>> {\n'
  '        let directory = path.parent().ok_or_else(|| {\n'
  '            Error::PruneIntentConflict("Native indexed prefix has no parent".to_owned())\n'
  '        })?;\n'
  '        let Some(metadata) = Self::regular_sidecar_metadata_for(&self.store_root, path, directory)?\n'
  '        else {\n'
  '            return Ok(None);\n'
  '        };\n'
  '        let len = usize::try_from(metadata.file.len())?;\n'
  '        if len >= expected.len() {\n'
  '            return Ok(None);\n'
  '        }\n'
  '        let limit = if component == NativeAmxPublicationComponent::Latest {\n'
  '            u64::try_from(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_MAX_BYTES)?\n'
  '        } else {\n'
  '            self.native_amx_participant_evidence_file_bytes()\n'
  '        };\n'
  '        if metadata.file.len() > STRICT_INIT_MAX_BLOCK_BYTES\n'
  '            || metadata.file.len() > limit\n'
  '            || (component != NativeAmxPublicationComponent::Latest\n'
  '                && Self::regular_sidecar_metadata_for(&self.store_root, stable, directory)?\n'
  '                    .is_some())\n'
  '        {\n'
  '            return Err(Self::invalid_lane_artifact_error(\n'
  '                path.to_path_buf(),\n'
  '                "Native indexed prefix is oversized or overlaps a stable pair file",\n'
  '            ));\n'
  '        }\n'
  '        let mut opened = Self::open_bound_progress_file(namespace, path, &metadata)?;\n'
  '        let mut prefix = Vec::new();\n'
  '        prefix.try_reserve_exact(len)?;\n'
  '        prefix.resize(len, 0);\n'
  '        opened\n'
  '            .read_exact(&mut prefix)\n'
  '            .map_err(|error| Error::IO(error, path.to_path_buf()))?;\n'
  '        if !expected.starts_with(&prefix) {\n'
  '            return Err(Self::invalid_lane_artifact_error(\n'
  '                path.to_path_buf(),\n'
  '                "Native indexed temporary is not an exact canonical prefix",\n'
  '            ));\n'
  '        }\n'
  '        self.verify_bound_open_regular_file_exact_bytes_locked(\n'
  '            namespace,\n'
  '            path,\n'
  '            &mut opened,\n'
  '            &metadata,\n'
  '            &prefix,\n'
  '            len,\n'
  '            "Native indexed publication prefix",\n'
  '        )?;\n'
  '        Ok(Some(NativeAmxIndexedPrefixFile {\n'
  '            component,\n'
  '            path: path.to_path_buf(),\n'
  '            metadata,\n'
  '            opened,\n'
  '            prefix,\n'
  '        }))\n'
  '    }'),
 ('crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'struct',
  'NativeAmxIndexedPrefixFile',
  (),
  'struct NativeAmxIndexedPrefixFile {\n'
  '    component: NativeAmxPublicationComponent,\n'
  '    path: PathBuf,\n'
  '    metadata: StableSidecarMetadata,\n'
  '    opened: std::fs::File,\n'
  '    prefix: Vec<u8>,\n'
  '}'),
 ('crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::native_amx_indexed_publication_has_temporary_under_prune_and_canonical_guards',
  (),
  '    fn native_amx_indexed_publication_has_temporary_under_prune_and_canonical_guards(\n'
  '        &self,\n'
  '        block: &SignedBlock,\n'
  '        merge: Option<&MergeLedgerEntry>,\n'
  '    ) -> Result<bool> {\n'
  '        let manifest = '
  'crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(block, '
  'merge)\n'
  '            .map_err(|error| Error::PruneIntentConflict(format!("Native indexed prefix discovery '
  'manifest: {error}")))?;\n'
  '        let artifacts = native_amx_participant_application_artifacts(\n'
  '            &manifest,\n'
  '            native_amx_participant_application_finality_placeholder_hash(),\n'
  '        )\n'
  '        .ok_or_else(|| {\n'
  '            Error::PruneIntentConflict(\n'
  '                "Native indexed prefix discovery lacks an artifact plan".to_owned(),\n'
  '            )\n'
  '        })?;\n'
  '        let _geometry = self.lane_geometry_lock.lock();\n'
  '        let _sidecar = self.sidecar_lock.lock();\n'
  '        for (_, receipt) in artifacts {\n'
  '            let descriptor = &receipt.participant_proposal.descriptor;\n'
  '            let target = self.native_amx_reservation_physical_target_from_journal(descriptor)?;\n'
  '            let manifest_path = Self::native_amx_application_manifest_path_for_entry(\n'
  '                &target,\n'
  '                &self.store_root,\n'
  '                descriptor.lane_block_height,\n'
  '            );\n'
  '            let receipt_path = Self::native_amx_participant_receipt_path_for_entry(\n'
  '                &target,\n'
  '                &self.store_root,\n'
  '                descriptor.lane_block_height,\n'
  '            );\n'
  '            if self.bound_progress_sidecar_directory_is_absent(&manifest_path, &receipt_path)? {\n'
  '                continue;\n'
  '            }\n'
  '            let directory = manifest_path.parent().ok_or_else(|| {\n'
  '                Error::PruneIntentConflict("Native indexed prefix path has no parent".to_owned())\n'
  '            })?;\n'
  '            for path in [\n'
  '                manifest_path.with_extension("norito.tmp"),\n'
  '                receipt_path.with_extension("norito.tmp"),\n'
  '                directory.join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE),\n'
  '            ] {\n'
  '                if Self::regular_sidecar_metadata_for(&self.store_root, &path, directory)?.is_some()\n'
  '                {\n'
  '                    self.require_native_amx_reservation_physical_target(&target)?;\n'
  '                    return Ok(true);\n'
  '                }\n'
  '            }\n'
  '            self.require_native_amx_reservation_physical_target(&target)?;\n'
  '        }\n'
  '        Ok(false)\n'
  '    }'),
 ('crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::require_native_amx_indexed_latest_prefix_locked',
  (),
  '    fn require_native_amx_indexed_latest_prefix_locked(\n'
  '        &self,\n'
  '        entry: &impl LaneArtifactStorageView,\n'
  '        namespace: &BoundProgressNamespace,\n'
  '        inventory: &NativeAmxEvidenceInventory,\n'
  '        manifest: &NativeAmxParticipantApplicationManifestArtifactV1,\n'
  '        receipt: &NativeAmxParticipantApplicationReceiptArtifact,\n'
  '        prefix: &NativeAmxIndexedPrefixFile,\n'
  '    ) -> Result<()> {\n'
  '        let expected_path = namespace\n'
  '            .data_path\n'
  '            .parent()\n'
  '            .expect("bound Native directory")\n'
  '            .join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE);\n'
  '        let height = receipt.participant_proposal.descriptor.lane_block_height;\n'
  '        if prefix.component != NativeAmxPublicationComponent::Latest\n'
  '            || prefix.path != expected_path\n'
  '            || !inventory.temporaries.is_empty()\n'
  '        {\n'
  '            return Err(Error::PruneIntentConflict(\n'
  '                "Native latest prefix ambiguously overlaps another publication phase".to_owned(),\n'
  '            ));\n'
  '        }\n'
  '        self.require_native_amx_evidence_prune_intent_absent_locked(namespace)?;\n'
  '        let retained_manifest = inventory.manifests.get(&height).ok_or_else(|| {\n'
  '            Error::PruneIntentConflict("Native latest prefix lacks its stable manifest".to_owned())\n'
  '        })?;\n'
  '        let retained_receipt = inventory.receipts.get(&height).ok_or_else(|| {\n'
  '            Error::PruneIntentConflict("Native latest prefix lacks its stable receipt".to_owned())\n'
  '        })?;\n'
  '        if self.decode_native_amx_manifest_file_locked(entry, namespace, retained_manifest)?\n'
  '            != *manifest\n'
  '            || self.decode_native_amx_receipt_file_locked(entry, namespace, retained_receipt)?\n'
  '                != *receipt\n'
  '        {\n'
  '            return Err(Error::PruneIntentConflict(\n'
  '                "Native latest prefix differs from its exact stable pair".to_owned(),\n'
  '            ));\n'
  '        }\n'
  '        Ok(())\n'
  '    }'),
 ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::consume_native_amx_startup_stable_components_locked',
  (),
  '    fn consume_native_amx_startup_stable_components_locked(\n'
  '        &self,\n'
  '        entry: &impl LaneArtifactStorageView,\n'
  '        namespace: &BoundProgressNamespace,\n'
  '        manifest: &NativeAmxParticipantApplicationManifestArtifactV1,\n'
  '        receipt: &NativeAmxParticipantApplicationReceiptArtifact,\n'
  '    ) -> Result<()> {\n'
  '        let descriptor = &receipt.participant_proposal.descriptor;\n'
  '        let carrier = NativeAmxPublicationCarrier {\n'
  '            height: receipt.application_block_height,\n'
  '            block_hash: receipt.application_block_hash,\n'
  '            executed_wire_hash: receipt.executed_block_wire_hash,\n'
  '        };\n'
  '        let route = NativeAmxPublicationRoute {\n'
  '            lane_id: descriptor.lane_id,\n'
  '            dataspace_id: descriptor.dataspace_id,\n'
  '            incarnation: descriptor.lane_incarnation,\n'
  '        };\n'
  '        let Some(original_index) = self\n'
  '            .native_amx_publication_capacity_reservations\n'
  '            .lock()\n'
  '            .get(&carrier)\n'
  '            .filter(|owner| owner.routes.contains_key(&route))\n'
  '            .map(|owner| owner.index_record.clone())\n'
  '        else {\n'
  '            return Ok(());\n'
  '        };\n'
  '        let index = Self::read_native_amx_publication_index_for_store(&self.store_root)?;\n'
  '        if let Some(original) = original_index.as_ref() {\n'
  '            original\n'
  '                .validate()\n'
  '                .map_err(|message| Error::PruneIntentConflict(message.to_owned()))?;\n'
  '            if index.records.get(&carrier) != Some(original) {\n'
  '                return Err(Error::PruneIntentConflict(\n'
  '                    "Native startup component allocation lost or changed its original index"\n'
  '                        .to_owned(),\n'
  '                ));\n'
  '            }\n'
  '        } else if index.records.contains_key(&carrier) {\n'
  '            return Err(Error::PruneIntentConflict(\n'
  '                "Native startup maintenance acquired a foreign publication index".to_owned(),\n'
  '            ));\n'
  '        }\n'
  '        let latest_path = Self::native_amx_participant_receipt_latest_index_path_for_entry(\n'
  '            entry,\n'
  '            &self.store_root,\n'
  '        );\n'
  '        let expected = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(receipt);\n'
  '        if self.decode_bound_native_amx_participant_receipt_latest_index_locked(\n'
  '            entry,\n'
  '            &latest_path,\n'
  '            namespace,\n'
  '        )? != Some(expected)\n'
  '        {\n'
  "            return Ok(()); // A later authoritative frontier is not this route's publication.\n"
  '        }\n'
  '        let height = descriptor.lane_block_height;\n'
  '        let inventory = self.inventory_native_amx_evidence_files_locked(namespace, true)?;\n'
  '        let manifest_file = inventory.manifests.get(&height).ok_or_else(|| {\n'
  '            Error::PruneIntentConflict("Native startup component lost stable manifest".to_owned())\n'
  '        })?;\n'
  '        let receipt_file = inventory.receipts.get(&height).ok_or_else(|| {\n'
  '            Error::PruneIntentConflict("Native startup component lost stable receipt".to_owned())\n'
  '        })?;\n'
  '        if self.decode_native_amx_manifest_file_locked(entry, namespace, manifest_file)?\n'
  '            != *manifest\n'
  '            || self.decode_native_amx_receipt_file_locked(entry, namespace, receipt_file)?\n'
  '                != *receipt\n'
  '            || !expected.matches_manifest(manifest)\n'
  '        {\n'
  '            return Err(Error::PruneIntentConflict(\n'
  '                "Native startup component readback differs from original pair".to_owned(),\n'
  '            ));\n'
  '        }\n'
  '        // Completed-pair maintenance was admitted without a pending index.\n'
  '        // Reauthenticate that same finalized WSV authority before consuming its\n'
  '        // original component allocation; missing indexed ownership never falls\n'
  '        // through to this branch.\n'
  '        if original_index.is_none()\n'
  '            && '
  '(!self.native_amx_participant_application_receipt_matches_manifest_and_available_evidence_under_prune_canonical_and_sidecar_guards(receipt, '
  'manifest)\n'
  '                || !self.native_amx_publication_wsv_join_is_complete_locked(manifest, receipt)?)\n'
  '        {\n'
  '            return Err(Error::PruneIntentConflict(\n'
  '                "Native startup maintenance lost its completed-pair authority".to_owned(),\n'
  '            ));\n'
  '        }\n'
  '        for (component, bytes) in [\n'
  '            (\n'
  '                NativeAmxPublicationComponent::Manifest,\n'
  '                manifest.encode_framed()?.len(),\n'
  '            ),\n'
  '            (\n'
  '                NativeAmxPublicationComponent::Receipt,\n'
  '                receipt.encode_framed()?.len(),\n'
  '            ),\n'
  '            (\n'
  '                NativeAmxPublicationComponent::Latest,\n'
  '                norito::encode_canonical(&expected)?.len(),\n'
  '            ),\n'
  '        ] {\n'
  '            self.consume_native_amx_publication_component_after_durable_publication(\n'
  '                receipt, component, bytes,\n'
  '            )?;\n'
  '        }\n'
  '        Ok(())\n'
  '    }'),
 ('crates/iroha_core/src/kura.rs',
  'fn',
  'rebuild_native_amx_participant_receipt_latest_indexes_on_startup',
  ('native_amx_evidence_namespace_for_entry',
   'native_amx_latest_index_temp_bytes_locked',
   'require_native_amx_latest_index_temp_recovery_unambiguous_locked',
   'complete_native_amx_evidence_prune_intent_locked',
   'recover_native_amx_evidence_publication_temp_locked',
   'inventory_native_amx_evidence_files_locked',
   'decode_native_amx_manifest_file_locked',
   'decode_native_amx_receipt_file_locked',
   'difference(&manifest_payload_heights)',
   'difference(&receipt_payload_heights)',
   'authenticated_complete',
   'reconcile_native_amx_latest_index_temp_locked',
   'NativeAmxLatestIndexTempReconciliation::Promoted',
   'decode_bound_native_amx_participant_receipt_latest_index_locked',
   'current.matches_receipt',
   'current.matches_manifest',
   'is not backed by its exact receipt or QC-authenticated manifest',
   'persist_native_amx_participant_receipt_latest_index_from_reconstructed_inventory_locked',
   'prune_native_amx_evidence_pairs_locked',
   'progress_mutation_namespace_unchanged',
   'update_disk_usage_delta'),
  '    pub(crate) fn rebuild_native_amx_participant_receipt_latest_indexes_on_startup(\n'
  '        &self,\n'
  '    ) -> Result<usize> {\n'
  '        let _prune_guard = self.prune_lock.lock();\n'
  '        self.ensure_prune_recovery_not_required()?;\n'
  '        self.durable_mutation_authorized()?;\n'
  '        let _canonical_chain_guard = self.canonical_chain_lock.lock();\n'
  '        let _geometry_guard = self.lane_geometry_lock.lock();\n'
  '        let entries = self.retained_lane_storage_entries_under_geometry_guard()?;\n'
  '        let _sidecar_guard = self.sidecar_lock.lock();\n'
  '        let exact_durable_tip = u64::try_from(self.exact_durable_blocks_count()?)?;\n'
  '        let mut accounting_mutation = self\n'
  '            .begin_total_disk_usage_mutation()\n'
  '            .with_resource_children(entries.len());\n'
  '        let mut rebuilt = 0_usize;\n'
  '        for entry in entries {\n'
  '            let evidence_directory = Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root));\n'
  '            match std::fs::symlink_metadata(&evidence_directory) {\n'
  '                Ok(_) => {}\n'
  '                Err(error) if error.kind() == ErrorKind::NotFound => {\n'
  '                    accounting_mutation.resource_batch(0).finish();\n'
  '                    continue;\n'
  '                }\n'
  '                Err(error) => return Err(Error::IO(error, evidence_directory)),\n'
  '            }\n'
  '            let mut lane_resources = accounting_mutation.resource_batch(3);\n'
  '            let namespace = self.native_amx_evidence_namespace_for_entry(&entry)?;\n'
  '            let latest_index_path =\n'
  '                Self::native_amx_participant_receipt_latest_index_path_for_entry(\n'
  '                    &entry,\n'
  '                    &self.store_root,\n'
  '                );\n'
  '            if latest_index_path.parent() != Some(evidence_directory.as_path())\n'
  '                || !Self::progress_mutation_namespace_unchanged(&namespace)\n'
  '            {\n'
  '                return Err(Self::invalid_lane_artifact_error(\n'
  '                    evidence_directory,\n'
  '                    "Native AMX startup evidence does not share one descriptor-bound directory",\n'
  '                ));\n'
  '            }\n'
  '            for directory_entry in std::fs::read_dir(&evidence_directory)\n'
  '                .map_err(|error| Error::IO(error, evidence_directory.clone()))?\n'
  '            {\n'
  '                let directory_entry = directory_entry\n'
  '                    .map_err(|error| Error::IO(error, evidence_directory.clone()))?;\n'
  '                if directory_entry\n'
  '                    .file_name()\n'
  '                    .to_string_lossy()\n'
  '                    .starts_with(".kura-sidecar-")\n'
  '                {\n'
  '                    return Err(Self::invalid_lane_artifact_error(\n'
  '                        directory_entry.path(),\n'
  '                        "Native AMX startup reconstruction found an unresolved atomic temporary",\n'
  '                    ));\n'
  '                }\n'
  '            }\n'
  '            let before_bytes = self.native_amx_evidence_tracked_bytes_locked(&namespace)?;\n'
  '            let latest_temp_present = self\n'
  '                .native_amx_latest_index_temp_bytes_locked(&namespace)?\n'
  '                .is_some();\n'
  '            if latest_temp_present {\n'
  '                // Pointer staging follows complete pair publication and\n'
  '                // precedes pruning. Do not consume another recovery journal\n'
  '                // when these mutually exclusive crash shapes overlap.\n'
  '                self.require_native_amx_latest_index_temp_recovery_unambiguous_locked(&namespace)?;\n'
  '                lane_resources.guard().resource_batch(0).finish();\n'
  '            } else {\n'
  '                let mut recovery = lane_resources.guard().resource_batch(2);\n'
  '                self.complete_native_amx_evidence_prune_intent_locked(\n'
  '                    recovery.guard(),\n'
  '                    &entry,\n'
  '                    &namespace,\n'
  '                )?;\n'
  '                self.recover_native_amx_evidence_publication_temp_locked(\n'
  '                    recovery.guard(),\n'
  '                    &entry,\n'
  '                    &namespace,\n'
  '                    NativeAmxEvidenceRecoveryPhase::Startup,\n'
  '                )?;\n'
  '                recovery.finish();\n'
  '            }\n'
  '            let inventory = self.inventory_native_amx_evidence_files_locked(&namespace, true)?;\n'
  '            let receipt_payload_heights =\n'
  '                inventory.receipts.keys().copied().collect::<BTreeSet<_>>();\n'
  '            let manifest_payload_heights =\n'
  '                inventory.manifests.keys().copied().collect::<BTreeSet<_>>();\n'
  '            let latest_height = receipt_payload_heights.last().copied();\n'
  '            let mut validated_manifests = BTreeMap::new();\n'
  '            for manifest_height in &manifest_payload_heights {\n'
  '                let file = inventory\n'
  '                    .manifests\n'
  '                    .get(manifest_height)\n'
  '                    .expect("inventoried Native AMX manifest height exists");\n'
  '                let manifest =\n'
  '                    self.decode_native_amx_manifest_file_locked(&entry, &namespace, file)?;\n'
  '                if !self\n'
  '                    '
  '.native_amx_participant_application_manifest_matches_available_finality_under_prune_and_canonical_guards(\n'
  '                        &manifest,\n'
  '                    )\n'
  '                {\n'
  '                    return Err(Self::invalid_lane_artifact_error(\n'
  '                        file.path.clone(),\n'
  '                        format!(\n'
  '                            "Native AMX participant manifest at retained height {manifest_height} '
  'conflicts with authenticated finality"\n'
  '                        ),\n'
  '                    ));\n'
  '                }\n'
  '                validated_manifests.insert(*manifest_height, manifest);\n'
  '            }\n'
  '            let mut validated_receipts = BTreeMap::new();\n'
  '            let mut startup_evidence = BTreeMap::new();\n'
  '            for receipt_height in &receipt_payload_heights {\n'
  '                let file = inventory\n'
  '                    .receipts\n'
  '                    .get(receipt_height)\n'
  '                    .expect("inventoried Native AMX receipt height exists");\n'
  '                let receipt =\n'
  '                    self.decode_native_amx_receipt_file_locked(&entry, &namespace, file)?;\n'
  '                let evidence = self\n'
  '                    .classify_native_amx_latest_receipt_evidence_under_startup_guards(\n'
  '                        &entry,\n'
  '                        &receipt,\n'
  '                        &namespace,\n'
  '                        &manifest_payload_heights,\n'
  '                        exact_durable_tip,\n'
  '                    )?;\n'
  '                if evidence != NativeAmxParticipantReceiptStartupEvidence::DurablyApplied\n'
  '                    && Some(*receipt_height) != latest_height\n'
  '                {\n'
  '                    return Err(Self::invalid_lane_artifact_error(\n'
  '                        file.path.clone(),\n'
  '                        format!(\n'
  '                            "older Native AMX participant receipt at retained height {receipt_height} has '
  'an unrecoverable partial evidence join"\n'
  '                        ),\n'
  '                    ));\n'
  '                }\n'
  '                validated_receipts.insert(*receipt_height, receipt);\n'
  '                startup_evidence.insert(*receipt_height, evidence);\n'
  '            }\n'
  '            let receipt_without_manifest = receipt_payload_heights\n'
  '                .difference(&manifest_payload_heights)\n'
  '                .copied()\n'
  '                .collect::<Vec<_>>();\n'
  '            let manifest_without_receipt = manifest_payload_heights\n'
  '                .difference(&receipt_payload_heights)\n'
  '                .copied()\n'
  '                .collect::<Vec<_>>();\n'
  '            Self::validate_native_amx_retained_history_continuity(\n'
  '                &validated_manifests,\n'
  '                &validated_receipts,\n'
  '                true,\n'
  '            )\n'
  '            .map_err(|message| {\n'
  '                Self::invalid_lane_artifact_error(\n'
  '                    evidence_directory.clone(),\n'
  '                    format!("Native AMX startup retained history is invalid: {message}"),\n'
  '                )\n'
  '            })?;\n'
  '            let expected_receipt = latest_height\n'
  '                .and_then(|height| validated_receipts.get(&height))\n'
  '                .cloned();\n'
  '            let expected_startup_evidence =\n'
  '                latest_height.and_then(|height| startup_evidence.get(&height).copied());\n'
  '            let expected_can_publish = !matches!(\n'
  '                expected_startup_evidence,\n'
  '                Some(NativeAmxParticipantReceiptStartupEvidence::PendingManifestRepair)\n'
  '            );\n'
  '            let expected = expected_receipt\n'
  '                .as_ref()\n'
  '                .map(NativeAmxParticipantReceiptLatestIndexV2::from_receipt);\n'
  '            let mut authenticated_complete = BTreeMap::new();\n'
  '            for (height, receipt) in &validated_receipts {\n'
  '                let Some(manifest) = validated_manifests.get(height) else {\n'
  '                    continue;\n'
  '                };\n'
  '                let identity = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(receipt);\n'
  '                if !identity.matches_manifest(manifest) {\n'
  '                    return Err(Self::invalid_lane_artifact_error(\n'
  '                        evidence_directory.clone(),\n'
  '                        format!(\n'
  '                            "Native AMX complete startup pair at height {height} has conflicting '
  'latest-index projections"\n'
  '                        ),\n'
  '                    ));\n'
  '                }\n'
  '                authenticated_complete.insert(*height, identity);\n'
  '            }\n'
  '            let latest_child = lane_resources.guard().resource_child(vec![\n'
  '                latest_index_path.clone(),\n'
  '                evidence_directory.join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE),\n'
  '            ]);\n'
  '            if self.reconcile_native_amx_latest_index_temp_locked(\n'
  '                &entry,\n'
  '                &namespace,\n'
  '                &latest_index_path,\n'
  '                &authenticated_complete,\n'
  '                expected,\n'
  '                expected_can_publish,\n'
  '            )? == NativeAmxLatestIndexTempReconciliation::Promoted\n'
  '            {\n'
  '                rebuilt = rebuilt.saturating_add(1);\n'
  '            }\n'
  '            let current = self.decode_bound_native_amx_participant_receipt_latest_index_locked(\n'
  '                &entry,\n'
  '                &latest_index_path,\n'
  '                &namespace,\n'
  '            )?;\n'
  '            let current_manifest_backed = if let Some(current) = current {\n'
  '                let canonical_height = usize::try_from(current.application_block_height)\n'
  '                    .ok()\n'
  '                    .and_then(NonZeroUsize::new);\n'
  '                if self\n'
  '                    .require_active_lane_incarnation(\n'
  '                        &entry,\n'
  '                        current.lane_incarnation,\n'
  '                        current.application_block_height,\n'
  '                    )\n'
  '                    .is_err()\n'
  '                    || canonical_height.and_then(|height| self.get_durable_block_hash(height))\n'
  '                        != Some(current.application_block_hash)\n'
  '                {\n'
  '                    return Err(Self::invalid_lane_artifact_error(\n'
  '                        latest_index_path,\n'
  '                        "Native AMX participant latest index targets a stale incarnation or non-canonical '
  'application block",\n'
  '                    ));\n'
  '                }\n'
  '                let current_receipt_backed = validated_receipts\n'
  '                    .get(&current.lane_block_height)\n'
  '                    .is_some_and(|receipt| current.matches_receipt(receipt));\n'
  '                let current_manifest_backed = validated_manifests\n'
  '                    .get(&current.lane_block_height)\n'
  '                    .is_some_and(|manifest| current.matches_manifest(manifest));\n'
  '                if !current_receipt_backed && !current_manifest_backed {\n'
  '                    return Err(Self::invalid_lane_artifact_error(\n'
  '                        latest_index_path.clone(),\n'
  '                        "Native AMX participant latest index is not backed by its exact receipt or '
  'QC-authenticated manifest",\n'
  '                    ));\n'
  '                }\n'
  '                current_manifest_backed\n'
  '            } else {\n'
  '                false\n'
  '            };\n'
  '            match (expected, current) {\n'
  '                (Some(expected), Some(current)) if current != expected => {\n'
  '                    if current.lane_block_height > expected.lane_block_height\n'
  '                        && current_manifest_backed\n'
  '                        && manifest_without_receipt.contains(&current.lane_block_height)\n'
  '                    {\n'
  '                        iroha_logger::warn!(\n'
  '                            lane = %entry.lane_id.as_u32(),\n'
  '                            dataspace = entry.dataspace_id.as_u64(),\n'
  '                            lane_block_height = current.lane_block_height,\n'
  '                            "Native AMX derived latest pointer is awaiting authoritative receipt repair"\n'
  '                        );\n'
  '                    } else if current.lane_block_height < expected.lane_block_height {\n'
  '                        if expected_can_publish {\n'
  '                            let receipt = expected_receipt.as_ref().ok_or_else(|| {\n'
  '                                Self::invalid_lane_artifact_error(\n'
  '                                    evidence_directory.clone(),\n'
  '                                    "Native AMX participant highest receipt disappeared during startup '
  'reconstruction",\n'
  '                                )\n'
  '                            })?;\n'
  '                            '
  'self.persist_native_amx_participant_receipt_latest_index_from_reconstructed_inventory_locked(\n'
  '                                &entry,\n'
  '                                receipt,\n'
  '                                &latest_index_path,\n'
  '                                &namespace,\n'
  '                            )?;\n'
  '                            rebuilt = rebuilt.saturating_add(1);\n'
  '                        } else {\n'
  '                            iroha_logger::warn!(\n'
  '                                lane = %entry.lane_id.as_u32(),\n'
  '                                dataspace = entry.dataspace_id.as_u64(),\n'
  '                                lane_block_height = expected.lane_block_height,\n'
  '                                "newest Native AMX receipt is awaiting manifest repair before '
  'latest-pointer advancement"\n'
  '                            );\n'
  '                        }\n'
  '                    } else {\n'
  '                        return Err(Self::invalid_lane_artifact_error(\n'
  '                            latest_index_path,\n'
  '                            "Native AMX participant latest index conflicts with exact durable application '
  'evidence",\n'
  '                        ));\n'
  '                    }\n'
  '                }\n'
  '                (Some(expected), None) if expected_can_publish => {\n'
  '                    let receipt = expected_receipt.as_ref().ok_or_else(|| {\n'
  '                        Self::invalid_lane_artifact_error(\n'
  '                            evidence_directory.clone(),\n'
  '                            "Native AMX participant highest receipt disappeared during startup '
  'reconstruction",\n'
  '                        )\n'
  '                    })?;\n'
  '                    debug_assert!(expected.matches_receipt(receipt));\n'
  '                    '
  'self.persist_native_amx_participant_receipt_latest_index_from_reconstructed_inventory_locked(\n'
  '                        &entry,\n'
  '                        receipt,\n'
  '                        &latest_index_path,\n'
  '                        &namespace,\n'
  '                    )?;\n'
  '                    rebuilt = rebuilt.saturating_add(1);\n'
  '                }\n'
  '                (Some(_), None) => {\n'
  '                    iroha_logger::warn!(\n'
  '                        lane = %entry.lane_id.as_u32(),\n'
  '                        dataspace = entry.dataspace_id.as_u64(),\n'
  '                        "Native AMX receipt is awaiting manifest repair before latest-pointer '
  'publication"\n'
  '                    );\n'
  '                }\n'
  '                (Some(_), Some(_)) | (None, None) => {}\n'
  '                (None, Some(current)) => {\n'
  '                    iroha_logger::warn!(\n'
  '                        lane = %entry.lane_id.as_u32(),\n'
  '                        dataspace = entry.dataspace_id.as_u64(),\n'
  '                        lane_block_height = current.lane_block_height,\n'
  '                        "Native AMX derived latest pointer is awaiting authoritative receipt repair"\n'
  '                    );\n'
  '                }\n'
  '            }\n'
  '            if !Self::progress_mutation_namespace_unchanged(&namespace) {\n'
  '                return Err(Self::invalid_lane_artifact_error(\n'
  '                    evidence_directory.clone(),\n'
  '                    "Native AMX startup evidence namespace changed during reconstruction",\n'
  '                ));\n'
  '            }\n'
  '            latest_child.finish();\n'
  '            if expected_can_publish\n'
  '                && let Some(receipt) = expected_receipt.as_ref()\n'
  '                && let Some(manifest) = validated_manifests\n'
  '                    .get(&receipt.participant_proposal.descriptor.lane_block_height)\n'
  '            {\n'
  '                // Durable physical publication consumes its original allocation\n'
  '                // even while post-WSV authority and retention cleanup are pending.\n'
  '                self.consume_native_amx_startup_stable_components_locked(\n'
  '                    &entry, &namespace, manifest, receipt,\n'
  '                )?;\n'
  '            }\n'
  '            // A prepublished tip intentionally has no post-WSV metadata yet.\n'
  '            // Keep the previous complete pair until State replay commits that\n'
  '            // tip and the normal repair path authenticates the full join.\n'
  '            if native_amx_startup_retention_cleanup_authorized(\n'
  '                expected_startup_evidence,\n'
  '                !receipt_without_manifest.is_empty() || !manifest_without_receipt.is_empty(),\n'
  '            ) {\n'
  '                self.prune_native_amx_evidence_pairs_locked(\n'
  '                    lane_resources.guard(),\n'
  '                    &entry,\n'
  '                    &namespace,\n'
  '                )?;\n'
  '            } else {\n'
  '                self.inventory_native_amx_evidence_files_locked(&namespace, true)?;\n'
  '                lane_resources.guard().resource_batch(0).finish();\n'
  '            }\n'
  '            let after_bytes = self.native_amx_evidence_tracked_bytes_locked(&namespace)?;\n'
  '            self.update_disk_usage_delta(before_bytes, after_bytes);\n'
  '            if native_amx_startup_retention_cleanup_authorized(\n'
  '                expected_startup_evidence,\n'
  '                !receipt_without_manifest.is_empty() || !manifest_without_receipt.is_empty(),\n'
  '            ) && let Some(receipt) = expected_receipt.as_ref()\n'
  '            {\n'
  '                let height = receipt.participant_proposal.descriptor.lane_block_height;\n'
  '                let manifest = validated_manifests.get(&height).ok_or_else(|| {\n'
  '                    Error::PruneIntentConflict(\n'
  '                        "Native startup completion lost its authenticated manifest".to_owned(),\n'
  '                    )\n'
  '                })?;\n'
  '                self.validate_native_amx_startup_completed_pair_locked(\n'
  '                    &entry, &namespace, &inventory, manifest, receipt,\n'
  '                )?;\n'
  '                self.complete_native_amx_publication_route_capacity_locked(\n'
  '                    &entry, &namespace, receipt,\n'
  '                )?;\n'
  '            }\n'
  '            lane_resources.finish();\n'
  '        }\n'
  '        accounting_mutation.finish();\n'
  '        if !self.disk_usage_initialized.load(Ordering::Relaxed)\n'
  '            || !self.disk_usage_total_initialized.load(Ordering::Relaxed)\n'
  '        {\n'
  '            // A prior failed recovery attempt deliberately invalidates both\n'
  '            // caches. The successful retry has now released its mutation\n'
  '            // guard, so rebuild the complete enforced/total snapshot instead\n'
  '            // of publishing a delta against stale cached totals.\n'
  '            self.refresh_disk_usage_bytes()?;\n'
  '        }\n'
  '        Ok(rebuilt)\n'
  '    }'),
 ('crates/iroha_core/src/kura.rs',
  'fn',
  'preflight_native_amx_incoming_artifacts_locked',
  ('validate_native_amx_retained_history_continuity',
   'native_amx_participant_application_pair_framed_bytes',
   'let mut additional_bytes = 0_u64;',
   'read_native_amx_evidence_file_bytes_locked',
   '!= expected_bytes.as_slice()',
   'conflicts with the incoming same-height plan before publication',
   'temporary conflicts with the incoming plan before publication',
   '!inventory.stable(*kind).contains_key(&participant_height)',
   'inventory.temporary(*kind).is_none()',
   'additional_bytes = additional_bytes',
   'native_amx_participant_evidence_startup_bytes',
   'native_amx_evidence_total_payload_bytes(inventory)',
   'bytes.checked_add(additional_bytes)',
   'single bounded transient publication window'),
  '    fn preflight_native_amx_incoming_artifacts_locked(\n'
  '        &self,\n'
  '        entry: &impl LaneArtifactStorageView,\n'
  '        namespace: &BoundProgressNamespace,\n'
  '        inventory: &NativeAmxEvidenceInventory,\n'
  '        manifest: &NativeAmxParticipantApplicationManifestArtifactV1,\n'
  '        receipt: &NativeAmxParticipantApplicationReceiptArtifact,\n'
  '    ) -> Result<()> {\n'
  '        let participant_height = manifest.leaf.participant_height;\n'
  '        let mut retained_manifests = BTreeMap::new();\n'
  '        for (height, file) in &inventory.manifests {\n'
  '            retained_manifests.insert(\n'
  '                *height,\n'
  '                self.decode_native_amx_manifest_file_locked(entry, namespace, file)?,\n'
  '            );\n'
  '        }\n'
  '        let mut retained_receipts = BTreeMap::new();\n'
  '        for (height, file) in &inventory.receipts {\n'
  '            retained_receipts.insert(\n'
  '                *height,\n'
  '                self.decode_native_amx_receipt_file_locked(entry, namespace, file)?,\n'
  '            );\n'
  '        }\n'
  '        Self::validate_native_amx_retained_history_continuity(\n'
  '            &retained_manifests,\n'
  '            &retained_receipts,\n'
  '            true,\n'
  '        )\n'
  '        .map_err(|message| {\n'
  '            Self::invalid_lane_artifact_error(\n'
  '                namespace.data_path.clone(),\n'
  '                format!("Native AMX prepublication retained history is invalid: {message}"),\n'
  '            )\n'
  '        })?;\n'
  '        if inventory\n'
  '            .manifests\n'
  '            .keys()\n'
  '            .chain(inventory.receipts.keys())\n'
  '            .any(|height| *height > participant_height)\n'
  '        {\n'
  '            return Err(Self::invalid_lane_artifact_error(\n'
  '                namespace.data_path.clone(),\n'
  '                "Native AMX prepublication would regress behind newer durable route evidence",\n'
  '            ));\n'
  '        }\n'
  '        let (manifest_bytes, receipt_bytes) =\n'
  '            native_amx_participant_application_pair_framed_bytes(manifest, receipt)?;\n'
  '        let incoming = [\n'
  '            (NativeAmxEvidenceKind::Manifest, manifest_bytes),\n'
  '            (NativeAmxEvidenceKind::Receipt, receipt_bytes),\n'
  '        ];\n'
  '        if !self.native_amx_participant_evidence_pair_fits_stable_bytes(\n'
  '            incoming[0].1.len(),\n'
  '            incoming[1].1.len(),\n'
  '        ) {\n'
  '            return Err(Self::invalid_lane_artifact_error(\n'
  '                namespace.data_path.clone(),\n'
  '                "Native AMX incoming manifest/receipt pair exceeds the shared stable aggregate byte '
  'bound",\n'
  '            ));\n'
  '        }\n'
  '        let mut additional_bytes = 0_u64;\n'
  '        for (kind, expected_bytes) in &incoming {\n'
  '            if let Some(existing) = inventory.stable(*kind).get(&participant_height) {\n'
  '                self.validate_native_amx_evidence_file_locked(entry, namespace, existing)?;\n'
  '                if self\n'
  '                    .read_native_amx_evidence_file_bytes_locked(namespace, existing)?\n'
  '                    .as_slice()\n'
  '                    != expected_bytes.as_slice()\n'
  '                {\n'
  '                    return Err(Self::invalid_lane_artifact_error(\n'
  '                        existing.path.clone(),\n'
  '                        format!(\n'
  '                            "{} conflicts with the incoming same-height plan before publication",\n'
  '                            kind.label()\n'
  '                        ),\n'
  '                    ));\n'
  '                }\n'
  '            }\n'
  '            if let Some(temporary) = inventory.temporary(*kind) {\n'
  '                if temporary.participant_height != participant_height {\n'
  '                    return Err(Self::invalid_lane_artifact_error(\n'
  '                        temporary.path.clone(),\n'
  '                        format!(\n'
  '                            "{} temporary targets another participant height",\n'
  '                            kind.label()\n'
  '                        ),\n'
  '                    ));\n'
  '                }\n'
  '                self.validate_native_amx_evidence_file_locked(entry, namespace, temporary)?;\n'
  '                if self\n'
  '                    .read_native_amx_evidence_file_bytes_locked(namespace, temporary)?\n'
  '                    .as_slice()\n'
  '                    != expected_bytes.as_slice()\n'
  '                {\n'
  '                    return Err(Self::invalid_lane_artifact_error(\n'
  '                        temporary.path.clone(),\n'
  '                        format!(\n'
  '                            "{} temporary conflicts with the incoming plan before publication",\n'
  '                            kind.label()\n'
  '                        ),\n'
  '                    ));\n'
  '                }\n'
  '            }\n'
  '            if !inventory.stable(*kind).contains_key(&participant_height)\n'
  '                && inventory.temporary(*kind).is_none()\n'
  '            {\n'
  '                additional_bytes = additional_bytes\n'
  '                    .checked_add(u64::try_from(expected_bytes.len())?)\n'
  '                    .ok_or_else(|| {\n'
  '                        Self::invalid_lane_artifact_error(\n'
  '                            namespace.data_path.clone(),\n'
  '                            "Native AMX incoming pair byte count overflowed",\n'
  '                        )\n'
  '                    })?;\n'
  '            }\n'
  '        }\n'
  '        let transient_limit = self.native_amx_participant_evidence_startup_bytes()?;\n'
  '        if Self::native_amx_evidence_total_payload_bytes(inventory)\n'
  '            .and_then(|bytes| bytes.checked_add(additional_bytes))\n'
  '            .is_none_or(|bytes| bytes > transient_limit)\n'
  '        {\n'
  '            return Err(Self::invalid_lane_artifact_error(\n'
  '                namespace.data_path.clone(),\n'
  '                "Native AMX incoming pair cannot reserve the single bounded transient publication '
  'window",\n'
  '            ));\n'
  '        }\n'
  '        Ok(())\n'
  '    }'),
 ('crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'enum',
  'NativeAmxPrefixRecoveryScope',
  (),
  'enum NativeAmxPrefixRecoveryScope {\n    Indexed,\n    Startup,\n}'),
 ('crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::collect_native_amx_completed_pair_latest_prefixes_locked',
  (),
  '    fn collect_native_amx_completed_pair_latest_prefixes_locked(\n'
  '        &self,\n'
  '        index: &NativeAmxPublicationIndexInventory,\n'
  '        indexed_routes: &BTreeSet<(LaneId, Hash)>,\n'
  '        prefixes: &mut Vec<NativeAmxIndexedPublicationPrefix>,\n'
  '    ) -> Result<()> {\n'
  '        for location in self.native_amx_evidence_physical_locations_from_journal()? {\n'
  '            if indexed_routes.contains(&(location.lane_id(), location.incarnation())) {\n'
  '                continue;\n'
  '            }\n'
  '            let directory = Self::lane_artifact_dir(location.blocks_path());\n'
  '            let latest_path = directory.join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_FILE);\n'
  '            let latest_temp =\n'
  '                directory.join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE);\n'
  '            if Self::regular_sidecar_metadata_for(&self.store_root, &latest_temp, &directory)?\n'
  '                .is_none()\n'
  '            {\n'
  '                continue;\n'
  '            }\n'
  '            self.require_native_amx_evidence_physical_location(&location)?;\n'
  '            let manifest_path = directory.join(Self::native_amx_evidence_file_name(\n'
  '                NativeAmxEvidenceKind::Manifest,\n'
  '                1,\n'
  '            ));\n'
  '            let receipt_path = directory.join(Self::native_amx_evidence_file_name(\n'
  '                NativeAmxEvidenceKind::Receipt,\n'
  '                1,\n'
  '            ));\n'
  '            let namespace = self.open_bound_progress_namespace(&manifest_path, &receipt_path)?;\n'
  '            let inventory = self.inventory_native_amx_evidence_files_locked(&namespace, true)?;\n'
  '            if !inventory.temporaries.is_empty() {\n'
  '                return Err(Error::PruneIntentConflict(\n'
  '                    "Native maintenance latest prefix overlaps unindexed pair publication"\n'
  '                        .to_owned(),\n'
  '                ));\n'
  '            }\n'
  '            let height = inventory\n'
  '                .manifests\n'
  '                .keys()\n'
  '                .chain(inventory.receipts.keys())\n'
  '                .copied()\n'
  '                .max()\n'
  '                .ok_or_else(|| {\n'
  '                    Error::PruneIntentConflict(\n'
  '                        "Native maintenance latest prefix lacks its stable pair".to_owned(),\n'
  '                    )\n'
  '                })?;\n'
  '            let manifest_file = inventory.manifests.get(&height).ok_or_else(|| {\n'
  '                Error::PruneIntentConflict(\n'
  '                    "Native maintenance latest prefix lacks its highest stable manifest".to_owned(),\n'
  '                )\n'
  '            })?;\n'
  '            let receipt_file = inventory.receipts.get(&height).ok_or_else(|| {\n'
  '                Error::PruneIntentConflict(\n'
  '                    "Native maintenance latest prefix lacks its highest stable receipt".to_owned(),\n'
  '                )\n'
  '            })?;\n'
  '            let receipt_bytes =\n'
  '                self.read_native_amx_evidence_file_bytes_locked(&namespace, receipt_file)?;\n'
  '            let observed =\n'
  '                norito::decode_canonical::<NativeAmxParticipantApplicationReceiptArtifact>(\n'
  '                    &receipt_bytes,\n'
  '                )\n'
  '                .map_err(|error| {\n'
  '                    Self::invalid_lane_artifact_error(\n'
  '                        receipt_file.path.clone(),\n'
  '                        format!(\n'
  '                            "Native maintenance receipt discovery failed exact decode: {error}"\n'
  '                        ),\n'
  '                    )\n'
  '                })?;\n'
  '            let target = self.native_amx_reservation_physical_target_from_location(\n'
  '                &location,\n'
  '                observed.participant_proposal.descriptor.dataspace_id,\n'
  '            )?;\n'
  '            let manifest =\n'
  '                self.decode_native_amx_manifest_file_locked(&target, &namespace, manifest_file)?;\n'
  '            let receipt =\n'
  '                self.decode_native_amx_receipt_file_locked(&target, &namespace, receipt_file)?;\n'
  '            let carrier = NativeAmxPublicationCarrier {\n'
  '                height: receipt.application_block_height,\n'
  '                block_hash: receipt.application_block_hash,\n'
  '                executed_wire_hash: receipt.executed_block_wire_hash,\n'
  '            };\n'
  '            if index.records.contains_key(&carrier) {\n'
  '                return Err(Error::PruneIntentConflict(\n'
  '                    "Native maintenance latest prefix cannot borrow indexed publication authority"\n'
  '                        .to_owned(),\n'
  '                ));\n'
  '            }\n'
  '            self.ensure_durable_block_at_height(carrier.height, carrier.block_hash)?;\n'
  '            if '
  '!self.native_amx_participant_application_receipt_matches_manifest_and_available_evidence_under_prune_canonical_and_sidecar_guards(&receipt, '
  '&manifest)\n'
  '                || !self.native_amx_publication_wsv_join_is_complete_locked(&manifest, &receipt)?\n'
  '            {\n'
  '                return Err(Error::PruneIntentConflict(\n'
  '                    "Native maintenance latest prefix lacks completed-pair authority".to_owned(),\n'
  '                ));\n'
  '            }\n'
  '            let expected = norito::encode_canonical(\n'
  '                &NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipt),\n'
  '            )?;\n'
  '            let prefix = self.open_native_amx_indexed_publication_prefix_locked(\n'
  '                &namespace,\n'
  '                NativeAmxPublicationComponent::Latest,\n'
  '                &latest_path,\n'
  '                &latest_temp,\n'
  '                &expected,\n'
  '            )?;\n'
  '            self.preflight_native_amx_incoming_artifacts_locked(\n'
  '                &target, &namespace, &inventory, &manifest, &receipt,\n'
  '            )?;\n'
  '            let Some((_, capacity)) = self\n'
  '                .native_amx_route_publication_capacity_with_inventory_locked(\n'
  '                    &target,\n'
  '                    &manifest,\n'
  '                    &receipt,\n'
  '                    Some((&namespace, &inventory)),\n'
  '                    prefix.as_ref(),\n'
  '                )?\n'
  '            else {\n'
  '                return Err(Error::PruneIntentConflict(\n'
  '                    "Native maintenance latest prefix cannot rewrite a later frontier".to_owned(),\n'
  '                ));\n'
  '            };\n'
  '            if capacity\n'
  '                .outstanding_components\n'
  '                .iter()\n'
  '                .any(|component| *component != NativeAmxPublicationComponent::Latest)\n'
  '            {\n'
  '                return Err(Error::PruneIntentConflict(\n'
  '                    "Native maintenance latest prefix cannot authorize missing pair publication"\n'
  '                        .to_owned(),\n'
  '                ));\n'
  '            }\n'
  '            self.require_native_amx_reservation_physical_target(&target)?;\n'
  '            if let Some(file) = prefix {\n'
  '                prefixes.push(NativeAmxIndexedPublicationPrefix {\n'
  '                    target,\n'
  '                    namespace,\n'
  '                    file,\n'
  '                });\n'
  '            }\n'
  '        }\n'
  '        Ok(())\n'
  '    }'))

REPAIR_PREFIX_INTEGRATION_BINDINGS = (('crates/iroha_core/src/kura.rs',
  'fn',
  'persist_native_amx_participant_application_evidence_under_publication_guard',
  ('get_durable_block_hash',
   'plan.application_block_height',
   'plan.application_block_hash',
   'plan.manifest_leaf_count',
   'mode.permits_retention_cleanup()',
   'preflight_native_amx_participant_application_plan_under_publication_guard',
   'write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard',
   'read_back_native_amx_plan_manifests_under_publication_guard',
   'manifest_readback.authenticates',
   'write_native_amx_participant_application_receipt_artifact_only_with_retention_policy_under_publication_guard',
   'write_native_amx_participant_receipt_latest_index_for_prepublication_under_publication_guard',
   'authenticate_native_amx_participant_application_prepublication_under_publication_guard',
   'mode.requires_post_apply_metadata()',
   'NativeAmxParticipantApplicationPrepublicationToken::from_plan',
   'if permit_cleanup',
   'cleanup_native_amx_participant_application_evidence_under_publication_guard',
   'self.recover_native_amx_indexed_publication_prefixes_under_publication_guard(block)?;')),
 ('crates/iroha_core/src/kura.rs',
  'fn',
  'persist_native_amx_participant_application_repair_targets_under_publication_guard',
  ('preflight_native_amx_participant_application_repair_targets_under_publication_guard',
   'write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard',
   'read_back_native_amx_repair_target_manifests_under_publication_guard',
   'write_native_amx_participant_application_receipt_artifact_only_with_retention_policy_under_publication_guard',
   'write_native_amx_participant_receipt_latest_index_for_prepublication_under_publication_guard',
   'authenticate_native_amx_participant_application_prepublication_under_publication_guard',
   'cleanup_native_amx_participant_application_evidence_under_publication_guard',
   'self.recover_native_amx_indexed_publication_prefixes_under_publication_guard(block)?;')),
 ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::rebuild_native_amx_publication_capacity_on_startup',
  ('record.classify_resolved_carrier(&selected_marker, selected)?',
   'let incomplete_carriers = committed_index_carriers;',
   'self.recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards(',
   '&incomplete_carriers.iter().copied().collect::<Vec<_>>()',
   'self.inventory_native_amx_evidence_files_locked(&namespace, true)?',
   'NativeAmxPrefixRecoveryScope::Startup')))

REPAIR_PREFIX_EXACT = tuple(
    (path, kind, symbol, body)
    for path, kind, symbol, _, body in REPAIR_PREFIX_OWNERS
)
REPAIR_PREFIX_BINDINGS = tuple(
    (path, kind, symbol, (*tokens, body))
    for path, kind, symbol, tokens, body in REPAIR_PREFIX_OWNERS
) + REPAIR_PREFIX_INTEGRATION_BINDINGS
BINDINGS += REPAIR_PREFIX_BINDINGS

REPAIR_PREFIX_ORDER = (('crates/iroha_core/src/kura.rs',
  'fn',
  'persist_native_amx_participant_application_evidence_under_publication_guard',
  ('self.recover_native_amx_indexed_publication_prefixes_under_publication_guard(block)?;',
   'self.preflight_native_amx_participant_application_plan_under_publication_guard(plan)?;',
   'self.ensure_native_amx_publication_capacity_under_publication_guard(')),
 ('crates/iroha_core/src/kura.rs',
  'fn',
  'persist_native_amx_participant_application_repair_targets_under_publication_guard',
  ('self.recover_native_amx_indexed_publication_prefixes_under_publication_guard(block)?;',
   'self.preflight_native_amx_participant_application_repair_targets_under_publication_guard(plan, '
   'target_indices,)?;',
   'self.ensure_native_amx_publication_capacity_under_publication_guard(')),
 ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::rebuild_native_amx_publication_capacity_on_startup',
  ('record.classify_resolved_carrier(&selected_marker, selected)?',
   'self.recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards(&incomplete_carriers.iter().copied().collect::<Vec<_>>(),NativeAmxPrefixRecoveryScope::Startup,)?;',
   'self.inventory_native_amx_evidence_files_locked(&namespace, true)?;')),
 ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::ensure_native_amx_publication_capacity_under_publication_guard',
  ('self.ensure_durable_block_at_height(block.header().height().get(), block.hash())?;',
   'self.recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards(&[Self::native_amx_publication_carrier(block)?],NativeAmxPrefixRecoveryScope::Indexed,)?;',
   'self.native_amx_capacity_plan_from_evidence_under_prune_and_canonical_guards(')),
 ('crates/iroha_core/src/kura.rs',
  'fn',
  'rebuild_native_amx_participant_receipt_latest_indexes_on_startup',
  ('latest_child.finish();',
   'self.consume_native_amx_startup_stable_components_locked(&entry, &namespace, manifest, receipt,)?;',
   'self.prune_native_amx_evidence_pairs_locked(')))
