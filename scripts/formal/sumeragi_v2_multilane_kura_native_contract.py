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
  '        self.inventory_native_amx_evidence_with_repair_prefix_locked(\n'
  '            namespace,\n'
  '            allow_transient,\n'
  '            None,\n'
  '        )\n'
  '    }'),
 ('crates/iroha_core/src/kura.rs',
  'fn',
  'inventory_native_amx_evidence_with_repair_prefix_locked',
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
  '    fn inventory_native_amx_evidence_with_repair_prefix_locked(\n'
  '        &self,\n'
  '        namespace: &BoundProgressNamespace,\n'
  '        allow_transient: bool,\n'
  '        repair_prefix: Option<&NativeAmxEvidenceFile>,\n'
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
  '            let metadata = Self::regular_sidecar_metadata_for(&self.store_root, &path, '
  'directory)?\n'
  '                .ok_or_else(|| {\n'
  '                    Self::invalid_lane_artifact_error(\n'
  '                        path.clone(),\n'
  '                        "Native AMX evidence disappeared during bounded inventory",\n'
  '                    )\n'
  '                })?;\n'
  '            let len = metadata.file.len();\n'
  '            let owned_prefix = repair_prefix.is_some_and(|candidate| {\n'
  '                allow_transient\n'
  '                    && temporary\n'
  '                    && kind == NativeAmxEvidenceKind::Manifest\n'
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
  '                        "{} has an empty or oversized standalone payload for the shared stable '
  'budget",\n'
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
  '                "Native AMX manifests, receipts, and temporaries exceed their shared aggregate '
  'byte bound",\n'
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
  '            .ok_or_else(|| Error::PruneIntentConflict("Native unfinished publication lacks its '
  'exact pending index and retained receipt".to_owned()))?;\n'
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
  '                "Native completed repair lacks its exact published receipt '
  'pointer".to_owned(),\n'
  '            ));\n'
  '        }\n'
  '        Ok(())\n'
  '    }'),
 ('crates/iroha_core/src/kura/native_amx_publication_index.rs',
  'method',
  'Kura::native_amx_completed_repair_artifacts_under_prune_and_canonical_guards',
  (),
  '    fn native_amx_completed_repair_artifacts_under_prune_and_canonical_guards(\n'
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
  '        if record.origin != NativeAmxPublicationIndexOriginV1::CompletedRepair\n'
  '            || record.carrier != carrier\n'
  '            || record.merge_entry_hash != merge.map(MergeLedgerEntry::canonical_hash)\n'
  '        {\n'
  '            return Err(Error::PruneIntentConflict(\n'
  '                "Native completed repair startup differs from its retained index".to_owned(),\n'
  '            ));\n'
  '        }\n'
  '        let height = NonZeroUsize::new(usize::try_from(carrier.height)?).ok_or_else(|| {\n'
  '            Error::PruneIntentConflict("Native completed repair has zero startup '
  'height".to_owned())\n'
  '        })?;\n'
  '        let selected = self\n'
  '            .read_block_body_under_prune_and_canonical_guards(height)?\n'
  '            .ok_or_else(|| {\n'
  '                Error::PruneIntentConflict(\n'
  '                    "Native completed repair startup lacks its authenticated canonical body"\n'
  '                        .to_owned(),\n'
  '                )\n'
  '            })?;\n'
  '        if Self::native_amx_publication_carrier(&selected)? != carrier {\n'
  '            return Err(Error::PruneIntentConflict(\n'
  '                "Native completed repair startup changed canonical executed wire".to_owned(),\n'
  '            ));\n'
  '        }\n'
  '        let (_, finality, _) = self\n'
  '            '
  '.v2_finality_artifact_with_archive_under_prune_and_canonical_guards(carrier.height)?\n'
  '            .ok_or(Error::MissingV2FinalityArtifact {\n'
  '                height: carrier.height,\n'
  '            })?;\n'
  '        let manifest = '
  'crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(block, '
  'merge)\n'
  '            .map_err(|error| Error::PruneIntentConflict(format!("Native completed repair '
  'startup manifest: {error}")))?;\n'
  '        let artifacts =\n'
  '            native_amx_participant_application_artifacts(&manifest, HashOf::new(&finality))\n'
  '                .filter(|artifacts| !artifacts.is_empty())\n'
  '                .ok_or_else(|| {\n'
  '                    Error::PruneIntentConflict(\n'
  '                        "Native completed repair startup has no exact artifact '
  'plan".to_owned(),\n'
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
  '        let artifacts = self\n'
  '            .native_amx_completed_repair_artifacts_under_prune_and_canonical_guards(\n'
  '                block, merge, record,\n'
  '            )?;\n'
  '        let _geometry = self.lane_geometry_lock.lock();\n'
  '        let _sidecar = self.sidecar_lock.lock();\n'
  '        for (manifest, receipt) in artifacts {\n'
  '            if !self.native_amx_publication_wsv_join_is_complete_locked(&manifest, &receipt)? '
  '{\n'
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
  'NativeAmxCompletedRepairPrefix',
  (),
  'struct NativeAmxCompletedRepairPrefix {\n'
  '    target: lane_geometry::NativeAmxReservationPhysicalTarget,\n'
  '    namespace: BoundProgressNamespace,\n'
  '    temporary: NativeAmxEvidenceFile,\n'
  '    opened: std::fs::File,\n'
  '    prefix: Vec<u8>,\n'
  '}'),
 ('crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_completed_repair_prefixes_under_publication_guard',
  (),
  '    fn recover_native_amx_completed_repair_prefixes_under_publication_guard(\n'
  '        &self,\n'
  '        block: &SignedBlock,\n'
  '    ) -> Result<()> {\n'
  '        let _canonical = self.canonical_chain_lock.lock();\n'
  '        self.recover_native_amx_completed_repair_prefixes_under_prune_and_canonical_guards(&[\n'
  '            Self::native_amx_publication_carrier(block)?,\n'
  '        ])\n'
  '    }'),
 ('crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::recover_native_amx_completed_repair_prefixes_under_prune_and_canonical_guards',
  (),
  '    fn recover_native_amx_completed_repair_prefixes_under_prune_and_canonical_guards(\n'
  '        &self,\n'
  '        carriers: &[NativeAmxPublicationCarrier],\n'
  '    ) -> Result<()> {\n'
  '        let index = Self::read_native_amx_publication_index_for_store(&self.store_root)?;\n'
  '        let selected_marker = {\n'
  '            let mut store = self.block_store.lock();\n'
  '            let count = store.read_exact_durable_index_count()?;\n'
  '            store.commit_marker_for_count(count)?\n'
  '        };\n'
  '        let mut authenticated = Vec::new();\n'
  '        for carrier in carriers.iter().copied().collect::<BTreeSet<_>>() {\n'
  '            let Some(record) = index.records.get(&carrier).filter(|record| {\n'
  '                record.origin == NativeAmxPublicationIndexOriginV1::CompletedRepair\n'
  '            }) else {\n'
  '                continue;\n'
  '            };\n'
  '            let height = NonZeroUsize::new(usize::try_from(carrier.height)?).ok_or_else(|| {\n'
  '                Error::PruneIntentConflict(\n'
  '                    "Native repair prefix has zero carrier height".to_owned(),\n'
  '                )\n'
  '            })?;\n'
  '            let block = self\n'
  '                .read_block_body_under_prune_and_canonical_guards(height)?\n'
  '                .ok_or_else(|| {\n'
  '                    Error::PruneIntentConflict(\n'
  '                        "Native repair prefix lost its canonical carrier".to_owned(),\n'
  '                    )\n'
  '                })?;\n'
  '            if record.classify_resolved_carrier(\n'
  '                &selected_marker,\n'
  '                Some(Self::native_amx_publication_carrier(&block)?),\n'
  '            )? != NativeAmxPublicationIndexResolution::Committed\n'
  '            {\n'
  '                return Err(Error::PruneIntentConflict(\n'
  '                    "Native repair prefix lacks committed selected-wire authority".to_owned(),\n'
  '                ));\n'
  '            }\n'
  '            let merge =\n'
  '                '
  'self.native_amx_capacity_merge_entry_under_prune_and_canonical_guards(&block)?;\n'
  '            let artifacts = self\n'
  '                .native_amx_completed_repair_artifacts_under_prune_and_canonical_guards(\n'
  '                    &block,\n'
  '                    merge.as_ref(),\n'
  '                    record,\n'
  '                )?;\n'
  '            authenticated.push((record, artifacts));\n'
  '        }\n'
  '        if authenticated.is_empty() {\n'
  '            return Ok(());\n'
  '        }\n'
  '        let _geometry = self.lane_geometry_lock.lock();\n'
  '        let _sidecar = self.sidecar_lock.lock();\n'
  '        let mut prefixes = Vec::new();\n'
  '        for (record, artifacts) in authenticated {\n'
  '            for (manifest, receipt) in artifacts {\n'
  '                if !self.native_amx_publication_wsv_join_is_complete_locked(&manifest, '
  '&receipt)? {\n'
  '                    return Err(Error::PruneIntentConflict(\n'
  '                        "Native repair prefix lacks its finalized WSV join".to_owned(),\n'
  '                    ));\n'
  '                }\n'
  '                if !self.native_amx_participant_evidence_pair_fits_stable_bytes(\n'
  '                    manifest.encode_framed()?.len(),\n'
  '                    receipt.encode_framed()?.len(),\n'
  '                ) {\n'
  '                    return Err(Error::PruneIntentConflict(\n'
  '                        "Native repair prefix exceeds the authenticated pair byte '
  'bound".to_owned(),\n'
  '                    ));\n'
  '                }\n'
  '                let target = self.native_amx_reservation_physical_target_from_journal(\n'
  '                    &receipt.participant_proposal.descriptor,\n'
  '                )?;\n'
  '                let namespace = self.native_amx_evidence_namespace_for_entry(&target)?;\n'
  '                self.require_active_lane_artifact(\n'
  '                    &target,\n'
  '                    &receipt.participant_proposal.descriptor,\n'
  '                )?;\n'
  '                self.require_native_amx_evidence_prune_intent_absent_locked(&namespace)?;\n'
  '                let prefix = self.open_native_amx_completed_repair_prefix_locked(\n'
  '                    &target, &namespace, &manifest,\n'
  '                )?;\n'
  '                let mut inventory = '
  'self.inventory_native_amx_evidence_with_repair_prefix_locked(\n'
  '                    &namespace,\n'
  '                    true,\n'
  '                    prefix.as_ref().map(|(temporary, _, _)| temporary),\n'
  '                )?;\n'
  '                // The full original inventory (including actual prefix bytes) has\n'
  '                // passed every size/count bound. Validate the existing recovery\n'
  '                // plan with only the independently proven incomplete object absent.\n'
  '                if let Some((temporary, _, _)) = &prefix {\n'
  '                    let removed = inventory\n'
  '                        .temporaries\n'
  '                        .remove(&NativeAmxEvidenceKind::Manifest);\n'
  '                    if !removed.as_ref().is_some_and(|file| {\n'
  '                        file.path == temporary.path\n'
  '                            && Self::stable_sidecar_metadata_unchanged(\n'
  '                                &file.metadata,\n'
  '                                &temporary.metadata,\n'
  '                            )\n'
  '                    }) {\n'
  '                        return Err(Error::PruneIntentConflict(\n'
  '                            "Native repair prefix differs from its bounded '
  'inventory".to_owned(),\n'
  '                        ));\n'
  '                    }\n'
  '                }\n'
  '                if self\n'
  '                    .native_amx_route_publication_capacity_with_inventory_locked(\n'
  '                        &target,\n'
  '                        &manifest,\n'
  '                        &receipt,\n'
  '                        Some((&namespace, &inventory)),\n'
  '                    )?\n'
  '                    .is_some()\n'
  '                {\n'
  '                    self.require_native_amx_completed_repair_receipt_with_inventory_locked(\n'
  '                        &target,\n'
  '                        &receipt,\n'
  '                        Some((record, &manifest)),\n'
  '                        &namespace,\n'
  '                        &inventory,\n'
  '                    )?;\n'
  '                } else if prefix.is_some() {\n'
  '                    return Err(Error::PruneIntentConflict(\n'
  '                        "Native repair prefix cannot rewrite a later published '
  'frontier".to_owned(),\n'
  '                    ));\n'
  '                }\n'
  '                self.require_native_amx_reservation_physical_target(&target)?;\n'
  '                if let Some((temporary, opened, prefix)) = prefix {\n'
  '                    prefixes.push(NativeAmxCompletedRepairPrefix {\n'
  '                        target,\n'
  '                        namespace,\n'
  '                        temporary,\n'
  '                        opened,\n'
  '                        prefix,\n'
  '                    });\n'
  '                }\n'
  '            }\n'
  '        }\n'
  '        // No mutation occurred in the preceding all-route pass. Recheck every\n'
  '        // retained descriptor before beginning the exact physical cleanup batch.\n'
  '        for prefix in &mut prefixes {\n'
  '            self.require_native_amx_reservation_physical_target(&prefix.target)?;\n'
  '            self.verify_bound_open_regular_file_exact_bytes_locked(\n'
  '                &prefix.namespace,\n'
  '                &prefix.temporary.path,\n'
  '                &mut prefix.opened,\n'
  '                &prefix.temporary.metadata,\n'
  '                &prefix.prefix,\n'
  '                prefix.prefix.len(),\n'
  '                "Native completed-repair manifest prefix",\n'
  '            )?;\n'
  '        }\n'
  '        if prefixes.is_empty() {\n'
  '            return Ok(());\n'
  '        }\n'
  '        self.durable_mutation_authorized()?;\n'
  '        let resources = self.begin_total_disk_usage_mutation().with_resource_paths(\n'
  '            prefixes\n'
  '                .iter()\n'
  '                .map(|prefix| prefix.temporary.path.clone())\n'
  '                .collect(),\n'
  '        );\n'
  '        for prefix in &mut prefixes {\n'
  '            // Previous unlinks can change a shared parent timestamp. Retain the\n'
  '            // original directory object and the exact file identity and bytes.\n'
  '            self.verify_bound_open_regular_file_exact_bytes_after_namespace_mutation_locked(\n'
  '                &prefix.namespace,\n'
  '                &prefix.temporary.path,\n'
  '                &mut prefix.opened,\n'
  '                &prefix.temporary.metadata,\n'
  '                &prefix.prefix,\n'
  '                prefix.prefix.len(),\n'
  '                "Native completed-repair manifest prefix",\n'
  '            )?;\n'
  '            Self::remove_bound_progress_file_if_matches(\n'
  '                &prefix.namespace,\n'
  '                &prefix.temporary.path,\n'
  '                &prefix.opened,\n'
  '                &prefix.temporary.metadata,\n'
  '            )\n'
  '            .map_err(|error| Error::IO(error, prefix.temporary.path.clone()))?;\n'
  '            self.sync_native_amx_evidence_namespace(\n'
  '                &prefix.namespace,\n'
  '                "Native completed-repair prefix removal",\n'
  '            )?;\n'
  '            self.require_native_amx_reservation_physical_target(&prefix.target)?;\n'
  '        }\n'
  '        // The failed original write may have invalidated cached physical usage.\n'
  '        // Publish the actual removal and require its normal bounded rescan.\n'
  '        resources.finish_resources_before_disk_rescan();\n'
  '        Ok(())\n'
  '    }'),
 ('crates/iroha_core/src/kura/native_amx_repair_prefix.rs',
  'method',
  'Kura::open_native_amx_completed_repair_prefix_locked',
  (),
  '    fn open_native_amx_completed_repair_prefix_locked(\n'
  '        &self,\n'
  '        entry: &impl LaneArtifactStorageView,\n'
  '        namespace: &BoundProgressNamespace,\n'
  '        manifest: &NativeAmxParticipantApplicationManifestArtifactV1,\n'
  '    ) -> Result<Option<(NativeAmxEvidenceFile, std::fs::File, Vec<u8>)>> {\n'
  '        let stable = Self::native_amx_application_manifest_path_for_entry(\n'
  '            entry,\n'
  '            &self.store_root,\n'
  '            manifest.leaf.participant_height,\n'
  '        );\n'
  '        let path = stable.with_extension("norito.tmp");\n'
  '        let directory = path.parent().ok_or_else(|| {\n'
  '            Error::PruneIntentConflict("Native repair prefix has no parent".to_owned())\n'
  '        })?;\n'
  '        let Some(metadata) =\n'
  '            Self::regular_sidecar_metadata_for(&self.store_root, &path, directory)?\n'
  '        else {\n'
  '            return Ok(None);\n'
  '        };\n'
  '        let expected = manifest.encode_framed()?;\n'
  '        let len = usize::try_from(metadata.file.len())?;\n'
  '        if len >= expected.len() {\n'
  '            return Ok(None); // Existing full-frame validation retains its strict rejection.\n'
  '        }\n'
  '        if metadata.file.len() > STRICT_INIT_MAX_BLOCK_BYTES\n'
  '            || metadata.file.len() > self.native_amx_participant_evidence_file_bytes()\n'
  '            || Self::regular_sidecar_metadata_for(&self.store_root, &stable, '
  'directory)?.is_some()\n'
  '        {\n'
  '            return Err(Self::invalid_lane_artifact_error(\n'
  '                path,\n'
  '                "Native repair prefix is oversized or has a stable manifest",\n'
  '            ));\n'
  '        }\n'
  '        let mut opened = Self::open_bound_progress_file(namespace, &path, &metadata)?;\n'
  '        let mut prefix = Vec::new();\n'
  '        prefix.try_reserve_exact(len)?;\n'
  '        prefix.resize(len, 0);\n'
  '        opened\n'
  '            .read_exact(&mut prefix)\n'
  '            .map_err(|error| Error::IO(error, path.clone()))?;\n'
  '        if !expected.starts_with(&prefix) {\n'
  '            return Err(Self::invalid_lane_artifact_error(\n'
  '                path,\n'
  '                "Native repair temporary is not an exact canonical manifest prefix",\n'
  '            ));\n'
  '        }\n'
  '        self.verify_bound_open_regular_file_exact_bytes_locked(\n'
  '            namespace,\n'
  '            &path,\n'
  '            &mut opened,\n'
  '            &metadata,\n'
  '            &prefix,\n'
  '            len,\n'
  '            "Native completed-repair manifest prefix",\n'
  '        )?;\n'
  '        Ok(Some((\n'
  '            NativeAmxEvidenceFile {\n'
  '                kind: NativeAmxEvidenceKind::Manifest,\n'
  '                participant_height: manifest.leaf.participant_height,\n'
  '                path,\n'
  '                metadata,\n'
  '            },\n'
  '            opened,\n'
  '            prefix,\n'
  '        )))\n'
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
   'self.recover_native_amx_completed_repair_prefixes_under_publication_guard(block)?;')),
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
   'self.recover_native_amx_completed_repair_prefixes_under_publication_guard(block)?;')),
 ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::rebuild_native_amx_publication_capacity_on_startup',
  ('record.classify_resolved_carrier(&selected_marker, selected)?',
   'let incomplete_carriers = committed_index_carriers;',
   'self.recover_native_amx_completed_repair_prefixes_under_prune_and_canonical_guards(',
   '&incomplete_carriers.iter().copied().collect::<Vec<_>>()',
   'self.inventory_native_amx_evidence_files_locked(&namespace, true)?')))

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
  ('self.recover_native_amx_completed_repair_prefixes_under_publication_guard(block)?;',
   'self.preflight_native_amx_participant_application_plan_under_publication_guard(plan)?;',
   'self.ensure_native_amx_publication_capacity_under_publication_guard(')),
 ('crates/iroha_core/src/kura.rs',
  'fn',
  'persist_native_amx_participant_application_repair_targets_under_publication_guard',
  ('self.recover_native_amx_completed_repair_prefixes_under_publication_guard(block)?;',
   'self.preflight_native_amx_participant_application_repair_targets_under_publication_guard(plan, '
   'target_indices,)?;',
   'self.ensure_native_amx_publication_capacity_under_publication_guard(')),
 ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::rebuild_native_amx_publication_capacity_on_startup',
  ('record.classify_resolved_carrier(&selected_marker, selected)?',
   'self.recover_native_amx_completed_repair_prefixes_under_prune_and_canonical_guards(&incomplete_carriers.iter().copied().collect::<Vec<_>>(),)?;',
   'self.inventory_native_amx_evidence_files_locked(&namespace, true)?;')),
 ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs',
  'method',
  'Kura::ensure_native_amx_publication_capacity_under_publication_guard',
  ('self.ensure_durable_block_at_height(block.header().height().get(), block.hash())?;',
   'self.recover_native_amx_completed_repair_prefixes_under_prune_and_canonical_guards(&[Self::native_amx_publication_carrier(block)?],)?;',
   'self.native_amx_capacity_plan_from_evidence_under_prune_and_canonical_guards(')))
