"""Exact source bindings for Native AMX merge-manifest projection."""

from __future__ import annotations

import re
from pathlib import Path


NATIVE_MERGE_MANIFEST_CONTRACT_RELATIVE = Path(
    "scripts/formal/sumeragi_v2_multilane_native_merge_manifest_contract.py"
)
NATIVE_MERGE_MANIFEST_TEST_RELATIVE = Path(
    "pytests/scripts/sumeragi_v2_multilane_native_merge_manifest_test.py"
)
NATIVE_MERGE_MANIFEST_CORRIDOR_RELATIVE = Path(
    "crates/iroha_core/src/sumeragi/tests/"
    "v2_apply_unsealed_01c_historical_recovery.rs"
)

NATIVE_MERGE_SOURCE_BINDINGS = (
    (
        "crates/iroha_core/src/kura/prune_commit_merge_support.rs",
        "enum",
        "NativeAmxMergeAssociation",
        (
            "Live(Option<&'a MergeLedgerEntry>)",
            "Startup(Option<&'a MergeLedgerEntry>)",
            "CommittedOnly",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/exec.rs",
        "fn",
        "merge_native_amx_application_sources",
        (
            "reference.matches_entry(entry)",
            "entry.merge_qc.carrier_height != block.header().height().get()",
            "block.header().prev_block_hash() != Some(entry.merge_qc.carrier_parent_hash)",
            "entry.merge_qc.view != block.header().view_change_index()",
            "let Some(batch) = entry.execution_batch.as_ref() else",
            "return ordinary_native_amx_application_sources(block);",
            "!bundle.external.is_empty()",
            "block.external_entrypoints_cloned().next().is_some()",
            "merge_application_header_from_carrier(&block.header())",
            "merge_execution_batch_commitments_match(batch)",
            "execution.native_amx_receipts.len() != execution.entrypoints.len()",
            "Hash::from(canonical_entrypoint_hash) != *expected_entrypoint_hash",
            "Hash::from(result.hash()) != *expected_result_hash",
            "finality_bound_merge: true",
            "entrypoint_index != batch.entrypoint_count",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/exec.rs",
        "fn",
        "canonical_native_amx_application_sources",
        (
            "merge_entry: Option<&MergeLedgerEntry>",
            "ordinary_native_amx_application_sources(block)",
            "merge_native_amx_application_sources(block, entry)",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "canonical_executed_block_application_repair.rs",
        "fn",
        "planned_merge_entries_by_carrier",
        (
            "repairs: &[FinalizedMergeCarrierRepair]",
            "BTreeMap<(u64, HashOf<BlockHeader>), &MergeLedgerEntry>",
            "let key = (carrier_height, carrier_hash)",
            "entries.insert(key, repair.entry()).is_some()",
            "more than one finalized merge repair names carrier",
        ),
    ),
)

NATIVE_APPLICATION_MANIFEST_BINDING = (
    "crates/iroha_core/src/sumeragi/exec.rs",
    "fn",
    "from_result_bearing_block_and_merge_entry",
    (
        "merge_entry: Option<&MergeLedgerEntry>",
        "executed_block_wire_len",
        "executed_block_wire_hash",
        "canonical_native_amx_application_sources(block, merge_entry)?",
        "native_amx_participant_application_role",
        "NativeAmxParticipantApplicationRole::Coordinator",
        "NativeAmxParticipantApplicationRole::SeparateParticipant",
        "!source.finality_bound_merge",
        "source.finality_bound_merge",
        "authority_context_height > application_block_height",
        "prepare.source_id != source.receipt.source_id",
        "commit.source_id != source.receipt.source_id",
        "prepare.tx_entrypoint_hash != source.entrypoint_hash",
        "commit.tx_entrypoint_hash != source.entrypoint_hash",
        "settlement.tx_count",
        "settlement.receipts.len()",
        "receipt.timestamp_ms != authority_context_height",
        "settlement.nexus_fee_receipts.is_empty()",
        "settlement.native_amx_receipts.is_empty()",
        "BTreeMap::<(LaneId, DataSpaceId, Hash), u64>",
        "BTreeMap::<(LaneId, DataSpaceId, Hash, u64)",
        "route_heights",
        "descriptor.lane_block_height",
        "Native AMX participant route carries more than one height in one application block",
        "source_ids != group.settlement_source_ids",
        "MAX_NATIVE_AMX_APPLICATION_MANIFEST_LEAVES",
        "NativeAmxApplicationManifestLeafV1",
    ),
)

NATIVE_APPLICATION_MANIFEST_CLASSIFIER_MATCH_RELATION = (
    "crates/iroha_core/src/sumeragi/exec.rs",
    "fn",
    "from_result_bearing_block_and_merge_entry",
    (
        "match crate::native_amx::native_amx_participant_application_role( "
        "&source.receipt, leg, ) { "
        "Ok(crate::native_amx::NativeAmxParticipantApplicationRole::Coordinator) "
        "=> { continue; } "
        "Ok( crate::native_amx::"
        "NativeAmxParticipantApplicationRole::SeparateParticipant, ) => {} "
        "Err(error) => { return Err(format!( "
        '"Native AMX participant application identity is invalid: {error}" '
        ")); } }"
    ),
)

NATIVE_APPLICATION_MANIFEST_CLASSIFIER_ORDERED_SOURCE_CHECK = (
    "crates/iroha_core/src/sumeragi/exec.rs",
    "fn",
    "from_result_bearing_block_and_merge_entry",
    (
        "canonical_native_amx_application_sources(block, merge_entry)?",
        "for source in sources",
        "for leg in &source.receipt.legs",
        "match crate::native_amx::native_amx_participant_application_role(",
        "validate_lane_block_proposal(&leg.participant_proposal)",
        ".entry(key)",
        "source_ids != group.settlement_source_ids",
    ),
)

NATIVE_PARTICIPANT_APPLICATION_CLASSIFIER_MATCH_RE = re.compile(
    r"match\s+crate::native_amx::"
    r"native_amx_participant_application_role\s*"
    r"\(\s*(?:receipt|&source\.receipt)\s*,\s*leg\s*,?\s*\)"
)
NATIVE_PARTICIPANT_APPLICATION_ROLE_TOKENS = (
    "NativeAmxParticipantApplicationRole::Coordinator",
    "NativeAmxParticipantApplicationRole::SeparateParticipant",
)

NATIVE_MERGE_MANIFEST_CALLER_BINDINGS = (
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "method",
        "V2ApplyService::validate_candidate",
        (
            "from_result_bearing_block_and_merge_entry",
            "state_block.staged_merge_entry()",
            "execution_commitment_from_validated_block",
            "validate_native_amx_participant_application_evidence_byte_budget",
        ),
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "native_amx_participant_frontier_markers_and_merge_entry",
        (
            "merge_entry: Option<&MergeLedgerEntry>",
            "from_result_bearing_block_and_merge_entry",
            "block,",
            "merge_entry,",
            ".entries()",
            "application_block_hash: leaf.application_block_hash",
        ),
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "stage_native_amx_participant_frontiers",
        (
            "native_amx_participant_frontier_markers_and_merge_entry",
            "self.staged_merge_entry()",
            "encode_native_amx_participant_frontier_marker",
            "stage_merge_lane_frontier_markers",
        ),
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "replay_blocks_from_kura_range_inner",
        (
            "from_result_bearing_block_and_merge_entry",
            "state_block.staged_merge_entry()",
            "execution_commitment_from_validated_block",
            "replayed_execution_commitment != finality.commit_qc.execution_commitment",
        ),
    ),
    (
        "crates/iroha_core/src/kura/lane_artifact_budget.rs",
        "fn",
        "lane_artifact_required_bytes_for_block",
        (
            "merge_entry: Option<&MergeLedgerEntry>",
            "merge_lane_application_artifact_required_bytes_for_block(block, merge_entry)?",
            "from_result_bearing_block_and_merge_entry",
            "block,",
            "merge_entry,",
            "native_amx_participant_application_artifacts",
            "NativeAmxParticipantReceiptLatestIndexV2::from_receipt",
            "native_prune_intent_routes.insert",
            "native_amx_evidence_prune_intent_max_bytes",
        ),
    ),
    (
        "crates/iroha_core/src/kura/lane_artifact_budget.rs",
        "fn",
        "native_amx_manifest_for_committed_block",
        (
            "merge_association: NativeAmxMergeAssociation<'_>",
            "finality: &V2FinalityArtifact",
            "associated_merge_entry_for_block(block)?",
            "let planned_merge_entry = match merge_association",
            "NativeAmxMergeAssociation::Live(staged)",
            "NativeAmxMergeAssociation::Startup(staged)",
            "NativeAmxMergeAssociation::CommittedOnly => None",
            "if let Some(planned) = planned_merge_entry",
            "carrier_record_for_block_entry(block, planned)?",
            "validate_merge_carrier_finality_projection(",
            "committed != planned",
            "planned merge entry differs from its committed association",
            "let merge_entry = match merge_association",
            "live Native AMX merge publication lacks its staged association witness",
            "live Native AMX merge publication lacks its committed association",
            "live Native AMX staged merge entry differs from its committed association",
            "NativeAmxMergeAssociation::Startup(planned)",
            "committed_merge_entry.as_ref().or(planned)",
            "Self::block_merge_reference(block).is_some() && merge_entry.is_none()",
            "lacks its committed merge association",
            "from_result_bearing_block_and_merge_entry",
            "merge_entry,",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "native_amx_participant_application_evidence_for_block_under_publication_guard",
        (
            "merge_association: NativeAmxMergeAssociation<'_>",
            "v2_finality_artifact_with_archive_under_prune_guard",
            "native_amx_manifest_for_committed_block(",
            "merge_association",
            "&finality",
            "native_amx_application_manifest_root",
            "executed_block_wire_hash",
            "finality_artifact_hash",
            "native_amx_participant_receipt_matches_manifest_leaf",
        ),
    ),
)

NATIVE_MERGE_MANIFEST_NORMALIZED_RELATIONS = (
    (
        "crates/iroha_core/src/sumeragi/exec.rs",
        "fn",
        "canonical_native_amx_application_sources",
        "merge_entry.map_or_else( || ordinary_native_amx_application_sources(block), "
        "|entry| merge_native_amx_application_sources(block, entry), )",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "method",
        "V2ApplyService::validate_candidate",
        "let native_amx_manifest = crate::sumeragi::exec::"
        "NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry( "
        "valid.as_ref(), state_block.staged_merge_entry(), )",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "method",
        "V2ApplyService::validate_and_apply",
        "let native_amx_manifest = crate::sumeragi::exec::"
        "NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry( "
        "valid_block.as_ref(), state_block.staged_merge_entry(), )",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "method",
        "V2ApplyService::validate_and_apply",
        "self.kura .prepublish_native_amx_participant_application_evidence( "
        "committed_block.as_ref(), state_block.staged_merge_entry(), )",
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "native_amx_participant_frontier_markers_and_merge_entry",
        "let manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::"
        "from_result_bearing_block_and_merge_entry( block, merge_entry, )",
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "stage_native_amx_participant_frontiers",
        "let markers = State::native_amx_participant_frontier_markers_and_merge_entry( "
        "block, self.staged_merge_entry(), )?;",
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "replay_blocks_from_kura_range_inner",
        "let native_amx_manifest = crate::sumeragi::exec::"
        "NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry( "
        "valid_block.as_ref(), state_block.staged_merge_entry(), )",
    ),
    (
        "crates/iroha_core/src/kura/lane_artifact_budget.rs",
        "fn",
        "lane_artifact_required_bytes_for_block",
        "let native_manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::"
        "from_result_bearing_block_and_merge_entry( block, merge_entry, )",
    ),
    (
        "crates/iroha_core/src/kura/lane_artifact_budget.rs",
        "fn",
        "native_amx_manifest_for_committed_block",
        "let committed_merge_entry = self.associated_merge_entry_for_block(block)?;",
    ),
    (
        "crates/iroha_core/src/kura/lane_artifact_budget.rs",
        "fn",
        "native_amx_manifest_for_committed_block",
        "if let Some(planned) = planned_merge_entry { let record = "
        "Self::carrier_record_for_block_entry(block, planned)?; "
        "Self::validate_merge_carrier_finality_projection( record, planned, "
        "&block.header(), finality, )?;",
    ),
    (
        "crates/iroha_core/src/kura/lane_artifact_budget.rs",
        "fn",
        "native_amx_manifest_for_committed_block",
        "NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry( "
        "block, merge_entry, )",
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "prepublish_native_amx_participant_application_evidence",
        "let plan = self "
        ".native_amx_participant_application_evidence_for_block_under_publication_guard( "
        "block, false, NativeAmxMergeAssociation::Live(staged_merge_entry), )?;",
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "native_amx_participant_application_evidence_for_block_under_publication_guard",
        "let native_manifest = self.native_amx_manifest_for_committed_block("
        "block, merge_association, &finality)?;",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "canonical_executed_block_application_repair.rs",
        "fn",
        "plan_lane_application_evidence_repair",
        "let planned_merge_entries = "
        "planned_merge_entries_by_carrier(&merge_carriers)?;",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "canonical_executed_block_application_repair.rs",
        "fn",
        "apply_lane_application_evidence_repair",
        "summary.merge_carriers = kura .apply_finalized_merge_carrier_repairs( "
        "&plan.merge_carriers, plan.merge_carrier_repair_authorizations, )",
    ),
)

NATIVE_MERGE_MANIFEST_ORDERED_RELATIONS = (
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "canonical_executed_block_application_repair.rs",
        "fn",
        "plan_lane_application_evidence_repair",
        (
            "let planned_merge_entries = "
            "planned_merge_entries_by_carrier(&merge_carriers)?;",
            "let Some(block) = kura.get_block_without_merge_sidecar(height) else",
            "let planned_merge_entry = planned_merge_entries",
            ".get(&(application_block_height, application_block_hash))",
            "preflight_native_amx_participant_application_evidence_repair(",
            "planned_merge_entry,",
            "drop(planned_merge_entries);",
            "if !needs.is_empty()",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "canonical_executed_block_application_repair.rs",
        "fn",
        "apply_lane_application_evidence_repair",
        (
            "let planned_merge_entries = "
            "planned_merge_entries_by_carrier(&plan.merge_carriers)?;",
            "preflight_native_amx_participant_application_evidence_repair(",
            "planned_merge_entries",
            "drop(planned_merge_entries);",
            "preflight_finalized_merge_carrier_repairs(",
            "summary.merge_carriers = kura",
            ".apply_finalized_merge_carrier_repairs(",
            "for carrier in &plan.native_carriers",
            ".repair_native_amx_participant_application_evidence_for_markers(",
        ),
    ),
)

NATIVE_MERGE_MANIFEST_RAW_TEST_CHECKS = (
    (
        NATIVE_MERGE_MANIFEST_CORRIDOR_RELATIVE,
        "historical_autonomous_recovery_reaches_exactly_once_canonical_merge_application",
        (
            "fail_next_native_amx_prepublication_for_tests",
            '"pre-WSV Native AMX participant evidence publication"',
            '"failed live Native prepublication must not stage WSV"',
            "prepublish_native_amx_participant_application_evidence(",
            "durable_carrier.as_ref(), None)",
            '"live merge prepublication requires its exact staged witness"',
            "durable_carrier.as_ref(),\n                Some(&entry),",
            "live_prepublication.authenticates_state_frontiers",
            "remove_latest_native_amx_participant_manifest_for_testing",
            '"remove only the exact latest Native manifest"',
            "remove_merge_carrier_record_for_testing",
            "read_structural_native_amx_participant_application_receipt(",
            '"manifest loss must retain the exact structural Native receipt"',
            "read_native_amx_participant_application_receipt(",
            ".is_none()",
            '"the authoritative reader must reject a receipt without its manifest"',
            "preflight_native_amx_participant_application_evidence_repair(",
            "std::slice::from_ref(&native_marker),\n                None,",
            '"startup Native repair requires a committed or planned association"',
            "std::slice::from_ref(&native_marker),\n                Some(&entry),",
            '"planned merge association authorizes exact Native startup repair"',
            "plan_lane_application_evidence_repair(",
            "apply_lane_application_evidence_repair(",
            "native_carriers: 1",
            "native_routes: 1",
            "merge_carriers: 1",
            '"startup repair must reproduce the exact retained receipt bytes"',
            '"startup evidence repair must not mutate canonical WSV"',
            "assert!(empty_plan.is_empty())",
        ),
    ),
)

NATIVE_MERGE_MANIFEST_SOURCE_RELATIVES = (
    NATIVE_MERGE_MANIFEST_CONTRACT_RELATIVE,
    NATIVE_MERGE_MANIFEST_TEST_RELATIVE,
    NATIVE_MERGE_MANIFEST_CORRIDOR_RELATIVE,
)


def _validate_native_merge_manifest_raw_tests(
    root: Path, errors: list[str]
) -> None:
    for relative, test_name, required_tokens in NATIVE_MERGE_MANIFEST_RAW_TEST_CHECKS:
        path = root / relative
        try:
            source = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(f"{path}: cannot read Native corridor macro test: {error}")
            continue
        declaration = re.compile(
            r"v2_apply_test!\(\s*" + re.escape(test_name) + r"\s*,"
        )
        matches = list(declaration.finditer(source))
        if len(matches) != 1:
            errors.append(
                f"{path}: Native corridor macro test {test_name} must occur "
                f"exactly once, found {len(matches)}"
            )
            continue
        start = matches[0].start()
        next_test = re.search(r"v2_apply_test!\(", source[matches[0].end() :])
        end = (
            matches[0].end() + next_test.start()
            if next_test is not None
            else len(source)
        )
        item = source[start:end]
        cursor = -1
        for token in required_tokens:
            position = item.find(token, cursor + 1)
            if position < 0:
                errors.append(
                    f"{path}: Native corridor macro test {test_name} is "
                    f"missing or reorders token {token!r}"
                )
                break
            cursor = position


def validate_native_merge_manifest_relations(
    root: Path,
    binding_items: dict[tuple[str, str, str], str],
    errors: list[str],
    rust_binding_item=None,
) -> None:
    """Require every consumer to use its staged, durable, or planned entry."""

    _validate_native_merge_manifest_raw_tests(root, errors)

    for relative, kind, symbol, expected_relation in (
        NATIVE_MERGE_MANIFEST_NORMALIZED_RELATIONS
    ):
        item = binding_items.get((relative, kind, symbol))
        if item is None and rust_binding_item is not None:
            item = rust_binding_item(
                root,
                relative,
                kind,
                symbol,
                "Native merge-manifest relation",
                errors,
            )
        if item is None:
            continue
        normalized = " ".join(item.split())
        count = normalized.count(expected_relation)
        if count != 1:
            errors.append(
                f"{root / relative}: Native merge-manifest relation {symbol} "
                "must bind the exact staged or committed merge entry once, "
                f"found {count}"
            )

    for relative, kind, symbol, ordered_tokens in (
        NATIVE_MERGE_MANIFEST_ORDERED_RELATIONS
    ):
        item = binding_items.get((relative, kind, symbol))
        if item is None and rust_binding_item is not None:
            item = rust_binding_item(
                root,
                relative,
                kind,
                symbol,
                "Native merge-manifest corridor",
                errors,
            )
        if item is None:
            continue
        cursor = -1
        for token in ordered_tokens:
            position = item.find(token, cursor + 1)
            if position < 0:
                errors.append(
                    f"{root / relative}: Native merge-manifest corridor "
                    f"{symbol} is missing or reorders token {token!r}"
                )
                break
            cursor = position
