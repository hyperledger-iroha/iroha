"""Scoped source obligations for Native geometry observation and maintenance.

These are structural mutation checks, not proof that retirement admission is
reserved across consensus. The scanner explicitly dispatches observation or
maintenance and durability attestation under its inherited locks.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Callable

from sumeragi_v2_multilane_reviewed_rust_source import _mask_rust_comments


NATIVE_MODULE = "SumeragiV2NativeApplicationEvidence"
GEOMETRY = "crates/iroha_core/src/kura/lane_geometry.rs"
EVIDENCE = "crates/iroha_core/src/kura/lane_geometry/native_evidence.rs"
MAINTENANCE = "crates/iroha_core/src/kura/lane_geometry/retirement_maintenance.rs"
EFFECTS = "crates/iroha_core/src/kura/lane_geometry/retirement_observation.rs"
OBSERVER = "observe_geometry_native_amx_per_height_evidence"
WRAPPER = "read_and_attest_geometry_native_amx_per_height_evidence"
ADMISSION = "ensure_first_release_lane_retirement_admissible_with_certified_locked"
OBSERVED_ADMISSION = "observe_lane_retirement_locked"
SCANNER = "scan_lane_retirement_locked"
ARCHIVED = "ensure_archived_lane_work_released_with_custody"
MAINTAIN = "maintain_lane_retirement_route_locked"
COMMITTED_REWRITE = "certified_history_has_committed_rewrite_locked"

# Exact reviewed ledger obligations: callers retain their existing cross-object
# authentication checks, while file authentication and fsync have distinct owners.
GEOMETRY_EVIDENCE_BINDINGS = (
    (EVIDENCE, "struct", "ObservedNativeAmxFile", (
        "path: PathBuf", "metadata: StableSidecarMetadata", "bytes_hash: Hash",
    )),
    (EVIDENCE, "struct", "ObservedNativeAmxEvidence", (
        "kura: &'kura Kura", "directory: BoundProgressDirectory",
        "inventory: &'kura BoundProgressDirectorySnapshot", "manifests: BTreeMap",
        "receipts: BTreeMap", "files: Vec<ObservedNativeAmxFile>",
        "payload_limit: usize", "context: &'kura str",
    )),
    (EVIDENCE, "fn", OBSERVER, (
        "STRICT_INIT_MAX_BLOCK_BYTES", "parse_native_amx_evidence_path",
        "retained_record_limit", "native_amx_participant_evidence_file_bytes",
        "shared aggregate byte bound", "regular_sidecar_metadata_for",
        "read_regular_sidecar_snapshot", "norito::decode_from_bytes",
        "norito::to_bytes",
        "validate_native_amx_participant_application_manifest_artifact",
        "validate_native_amx_participant_application_receipt_artifact",
        "stable_sidecar_metadata_unchanged", "geometry_bound_progress_directory_snapshot",
        "files.try_reserve(1)", "bytes_hash: before.bytes_hash",
    )),
    (EVIDENCE, "fn", WRAPPER, (OBSERVER, ".attest()")),
    (EVIDENCE, "fn", "into_observed", (
        "fn into_observed(self)", "read_regular_sidecar_snapshot",
        "after.bytes_hash != observed.bytes_hash", "stable_sidecar_metadata_unchanged",
        "geometry_bound_progress_directory_snapshot", "Ok((self.manifests, self.receipts))",
    )),
    (EVIDENCE, "fn", "attest", (
        "fn attest(self)", "geometry_bound_progress_directory_snapshot",
        "sidecar_file_metadata_unchanged", "file.sync_all",
        "read_regular_sidecar_snapshot", "after.bytes_hash != observed.bytes_hash",
        "stable_sidecar_metadata_unchanged", "sync_dir", "Ok((manifests, receipts))",
    )),
    (MAINTENANCE, "fn", COMMITTED_REWRITE, (
        "certified_lane_block_paths_for_entry", "autonomous_lane_merge_bundle_paths_for_entry",
        "bound_progress_sidecar_directory_is_absent", "open_bound_progress_namespace",
        "open_optional_bound_progress_file", 'index.with_extension("index.tmp")',
        "return Ok(true)", "Ok(false)",
    )),
    (MAINTENANCE, "fn", MAINTAIN, (
        "fixed_progress_pairs: &[(&Path, &Path, &str); 7]",
        "read_latest_certified_lane_block_frontier_locked",
        "certified_history_has_committed_rewrite_locked", "AuthenticatedLaneHistoryRetention",
        "first_retained_height", "retention.as_ref()",
        "recover_certified_bundle_history_rewrites_locked",
        "recover_certified_lane_block_pair_from_frontier_locked",
        "confirm_latest_certified_lane_block_frontier_read_locked",
        "note_certified_frontier_artifact_validation",
        "decode_lane_merge_application_frontier",
        "lane_merge_application_frontier_expected_receipt_under_prune_and_canonical_guards",
        "retiring.contains(&frontier_identity)",
        "compact_lane_histories_through_merge_frontier_locked",
        "LaneHistoryCompactionOutcome::CapacityBlocked",
        "recover_geometry_progress_pairs_before_snapshot",
    )),
    (GEOMETRY, "fn", SCANNER, (
        "effects.prepare_route", OBSERVER, "effects.native", "require_active_lane_incarnation",
        "require_active_lane_artifact", "native_amx_participant_receipt_matches_manifest_leaf",
        "native_amx_retained_windows_are_complete",
        "validate_native_amx_retained_history_continuity",
        "decode_native_amx_participant_receipt_latest_index", "latest.matches_receipt",
        "confirmed_snapshot != artifact_snapshot",
    )),
    (GEOMETRY, "fn", ARCHIVED, (
        "custody.authenticate(self)?", "custody.is_none().then(|| self.sidecar_lock.lock())", WRAPPER,
        "native_amx_participant_application_manifest_matches_available_finality_under_prune_and_canonical_guards",
        "native_amx_retained_windows_are_complete",
        "validate_native_amx_retained_history_continuity",
        "native_amx_participant_application_receipt_matches_manifest_and_available_evidence_under_prune_canonical_and_sidecar_guards",
        "decode_native_amx_participant_receipt_latest_index_for_route",
        "latest.matches_receipt", "confirmed_snapshot != artifact_snapshot",
    )),
    (GEOMETRY, "fn", ADMISSION, (
        SCANNER, "RetirementScanEffects::MaintainAndAttest", "pending_canonical_bytes", ".map(drop)",
    )),
    (GEOMETRY, "fn", OBSERVED_ADMISSION, (
        "validate_certified_retirements_against_geometry", SCANNER, "RetirementScanEffects::Observe",
    )),
    (EFFECTS, "fn", "prepare_route", (
        "Self::MaintainAndAttest", MAINTAIN, "Self::Observe", "observe_lane_retirement_route_locked",
    )),
    (EFFECTS, "fn", "native", (
        "ObservedNativeAmxEvidence", "Self::Observe => observation.into_observed()",
        "Self::MaintainAndAttest { .. } => observation.attest()",
    )),
)

GEOMETRY_EVIDENCE_SOURCE_RELATIVES = (
    Path(__file__).relative_to(Path(__file__).resolve().parents[2]),
    Path("pytests/scripts/sumeragi_v2_multilane_geometry_evidence_contract_test.py"),
    *(Path(path) for path in (GEOMETRY, EVIDENCE, MAINTENANCE, EFFECTS)),
)


def _code(source: str) -> str:
    """Ignore layout, comments and literals, preserving Rust token order."""

    compact = re.sub(r"\s+", "", _mask_rust_comments(source))
    return re.sub(r",+(?=[)>\]}])", "", compact)


def validate_geometry_evidence_contract(
    root: Path, models: Any, errors: list[str], rust_binding_item: Callable,
) -> None:
    """Check exact ledger owners and the reviewed executable source relations."""

    native = [model for model in models if isinstance(model, dict)
              and model.get("module") == NATIVE_MODULE]
    bindings = native[0].get("production_symbols", ()) if len(native) == 1 else ()
    items: dict[str, str] = {}
    for relative, kind, symbol, tokens in GEOMETRY_EVIDENCE_BINDINGS:
        matches = [binding for binding in bindings if isinstance(binding, dict)
                   and (binding.get("path"), binding.get("kind"), binding.get("symbol"))
                   == (relative, kind, symbol)]
        if len(matches) != 1:
            errors.append(f"geometry evidence ledger owner {symbol} must occur exactly once")
        elif tuple(matches[0].get("required_tokens", ())) != tokens:
            errors.append(f"geometry evidence reviewed tokens changed for {symbol}")
        item = rust_binding_item(root, relative, kind, symbol, "geometry evidence", errors)
        if item is None:
            continue
        items[symbol] = _code(item)
        for token in tokens:
            if token not in item:
                errors.append(f"geometry evidence {symbol} is missing source token {token!r}")

    def require(symbol: str, *relations: str) -> None:
        code = items.get(symbol)
        if code is None:
            return
        for relation in relations:
            if _code(relation) not in code:
                errors.append(f"geometry evidence {symbol} is missing executable relation {relation!r}")

    def ordered(symbol: str, *relations: str) -> None:
        code = items.get(symbol)
        if code is None:
            return
        cursor = 0
        for relation in relations:
            position = code.find(_code(relation), cursor)
            if position < 0:
                errors.append(f"geometry evidence {symbol} is missing or reorders {relation!r}")
                break
            cursor = position + len(_code(relation))

    def count(symbol: str, relation: str, expected: int) -> None:
        code = items.get(symbol)
        if code is not None and code.count(_code(relation)) != expected:
            errors.append(f"geometry evidence {symbol} must contain {expected} exact {relation!r} checks")

    for symbol in ("ObservedNativeAmxFile", "ObservedNativeAmxEvidence"):
        code = items.get(symbol, "")
        if re.search(r"pub(?:\([^)]*\))?\w+:", code):
            errors.append(f"geometry evidence {symbol} exposes a mutable ownership field")
        if "&mut" in code:
            errors.append(f"geometry evidence {symbol} retains mutable borrowed state")
    require("ObservedNativeAmxEvidence", "kura: &'kura Kura", "inventory: &'kura BoundProgressDirectorySnapshot")

    for forbidden in ("sync_all(", "sync_dir(", ".attest(", "recover_", "compact_",
                      ".write(", ".create(", ".truncate(", "fs::write("):
        for symbol in (OBSERVER, "into_observed"):
            if forbidden in items.get(symbol, ""):
                errors.append(f"geometry evidence observer contains storage effect {forbidden!r} in {symbol}")
    require(OBSERVER,
        "if temporary { return Err(",
        "if retained_count >= retained_record_limit {",
        "if entry_snapshot.kind != BoundProgressDirectoryEntryKind::File { return Err(",
        "evidence_bytes.checked_add(encoded_len).ok_or_else(",
        "if evidence_bytes > self.native_amx_participant_evidence_file_bytes() { return Err(",
        "if !Self::stable_sidecar_metadata_unchanged(&metadata, &before.metadata) { return Err(",
        "kura: self", "inventory: artifact_snapshot",
        "metadata: before.metadata", "bytes_hash: before.bytes_hash",
    )
    for kind, artifact, height, rows in (
        ("Manifest", "NativeAmxParticipantApplicationManifestArtifactV1", "artifact.leaf.participant_height", "manifests"),
        ("Receipt", "NativeAmxParticipantApplicationReceiptArtifact", "artifact.participant_proposal.descriptor.lane_block_height", "receipts"),
    ):
        require(OBSERVER,
            f"norito::decode_from_bytes::<{artifact}>(&before.bytes).map_err(Error::NoritoFrame)?",
            f"if norito::to_bytes(&artifact).map_err(Error::NoritoFrame)? != before.bytes"
            f" || {height} != lane_block_height"
            f" || Self::validate_native_amx_participant_application_{kind.lower()}_artifact(&artifact).is_err()"
            f" || {rows}.insert(lane_block_height, artifact).is_some() {{ return Err(",
        )
    count(OBSERVER, "if &self.geometry_bound_progress_directory_snapshot(&directory, artifact_snapshot.len(), context)? != artifact_snapshot { return Err(", 2)
    require(WRAPPER,
        "self.observe_geometry_native_amx_per_height_evidence(lane_artifacts, artifact_snapshot, retained_record_limit, context)?.attest()",
    )
    require("into_observed", "fn into_observed(self) -> Result<NativeGeometryEvidence>",
        "if after.bytes_hash != observed.bytes_hash || !Kura::stable_sidecar_metadata_unchanged(&observed.metadata, &after.metadata) { return Err(",
        "if &self.kura.geometry_bound_progress_directory_snapshot(&self.directory, self.inventory.len(), self.context)? != self.inventory { return Err(",
    )
    ordered("into_observed", "for observed in &self.files {",
        "read_regular_sidecar_snapshot(&observed.path, &self.directory.expected_path, self.payload_limit)",
        "after.bytes_hash != observed.bytes_hash", "geometry_bound_progress_directory_snapshot(",
        "Ok((self.manifests, self.receipts))",
    )
    require("attest", "fn attest(self) -> Result<NativeGeometryEvidence> {",
        "let Self { kura, directory, inventory, manifests, receipts, files, payload_limit, context } = self;",
        "if !Kura::sidecar_file_metadata_unchanged(&observed.metadata.file, &opened_metadata) { return Err(",
        "if after.bytes_hash != observed.bytes_hash || !Kura::stable_sidecar_metadata_unchanged(&observed.metadata, &after.metadata) { return Err(",
    )
    inventory_check = "if &kura.geometry_bound_progress_directory_snapshot(&directory, inventory.len(), context)? != inventory { return Err("
    count("attest", inventory_check, 2)
    ordered("attest", inventory_check, "for observed in files {",
        "sidecar_file_metadata_unchanged(&observed.metadata.file, &opened_metadata)",
        "file.sync_all()", "read_regular_sidecar_snapshot(&path, lane_artifacts, payload_limit)?",
        "after.bytes_hash != observed.bytes_hash", "sync_dir(lane_artifacts)",
        inventory_check, "Ok((manifests, receipts))",
    )
    require(COMMITTED_REWRITE,
        "for (data, index) in [ Self::certified_lane_block_paths_for_entry(entry, &self.store_root), Self::autonomous_lane_merge_bundle_paths_for_entry(entry, &self.store_root) ] {",
        "if self.bound_progress_sidecar_directory_is_absent(&data, &index)? { continue; }",
        'let namespace = self.open_bound_progress_namespace(&data, &index)?; if self.open_optional_bound_progress_file(&namespace, &index.with_extension("index.tmp"))?.is_some() { return Ok(true); }',
    )
    # Bind the first authentication branch to the committed-rewrite owner. The
    # later compaction check is a separate obligation and cannot stand in for it.
    require(MAINTAIN,
        "if self.certified_history_has_committed_rewrite_locked(entry)? { let retention = self.decode_lane_merge_application_frontier(entry, &merge_application_frontier)?.map(|frontier| { if self.lane_merge_application_frontier_expected_receipt_under_prune_and_canonical_guards(&frontier).is_none() { return Err(",
        "Ok(AuthenticatedLaneHistoryRetention { entry: entry.clone(), first_retained_height: frontier.lane_block_height.saturating_sub(self.lane_history_retention.get() as u64).saturating_add(1), frontier, }) }).transpose()?; self.recover_certified_bundle_history_rewrites_locked(entry, retention.as_ref(), Some(&frontier_read.frontier.artifact))?;",
    )
    require(MAINTAIN,
        "read_latest_certified_lane_block_frontier_locked(entry, true)?",
        "recover_certified_lane_block_pair_from_frontier_locked(entry, &frontier_read.frontier.artifact, None, None)",
        "if let Some(frontier) = self.decode_lane_merge_application_frontier(entry, &merge_application_frontier)? { if self.lane_merge_application_frontier_expected_receipt_under_prune_and_canonical_guards(&frontier).is_none() { return Err(",
        "if retiring.contains(&frontier_identity) { match self.compact_lane_histories_through_merge_frontier_locked(pending_canonical_bytes, entry, &frontier)?",
    )
    ordered(MAINTAIN,
        "read_latest_certified_lane_block_frontier_locked(",
        "certified_history_has_committed_rewrite_locked(",
        "decode_lane_merge_application_frontier(",
        "lane_merge_application_frontier_expected_receipt_under_prune_and_canonical_guards(",
        "AuthenticatedLaneHistoryRetention {",
        "recover_certified_bundle_history_rewrites_locked(",
        "recover_certified_lane_block_pair_from_frontier_locked(",
        "confirm_latest_certified_lane_block_frontier_read_locked(",
        "note_certified_frontier_artifact_validation(",
        "decode_lane_merge_application_frontier(",
        "lane_merge_application_frontier_expected_receipt_under_prune_and_canonical_guards(",
        "compact_lane_histories_through_merge_frontier_locked(",
        "recover_geometry_progress_pairs_before_snapshot(",
    )
    if ".lock(" in items.get(MAINTAIN, ""):
        errors.append("geometry evidence maintenance reacquires an inherited lock")
    require(ADMISSION,
        "self.scan_lane_retirement_locked(retiring, certified_retirements, RetirementScanEffects::MaintainAndAttest { pending_canonical_bytes }).map(drop)",
    )
    require(OBSERVED_ADMISSION,
        "self.validate_certified_retirements_against_geometry(retiring, certified_retirements)?; self.scan_lane_retirement_locked(retiring, certified_retirements, RetirementScanEffects::Observe)",
    )
    require("prepare_route",
        "match self { Self::MaintainAndAttest { pending_canonical_bytes } => kura.maintain_lane_retirement_route_locked(pending_canonical_bytes, lane, entry, retiring, pairs), Self::Observe => kura.observe_lane_retirement_route_locked(entry) }",
    )
    require("native",
        "match self { Self::Observe => observation.into_observed(), Self::MaintainAndAttest { .. } => observation.attest() }",
    )
    require(SCANNER,
        "let fixed_progress_pairs: [(&Path, &Path, &str); 7] = [ (&lane_data, &lane_index, \"\"), (&input_data, &input_index, \"\"), (&preflight_data, &preflight_index, \"\"), (&certified_data, &certified_index, \"\"), (&merge_bundle_data, &merge_bundle_index, \"\"), (&canonical_replica_data, &canonical_replica_index, CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL), (&receipt_data, &receipt_index, \"\") ];",
        "let lane_artifacts_guard = effects.prepare_route(self, storage_lane_id, &entry, &retiring, &fixed_progress_pairs)?; let artifact_snapshot = self.geometry_bound_progress_directory_snapshot(",
        "let (retained_native_manifests, retained_native_receipts) = effects.native(native_observation)?;",
    )
    ordered(SCANNER, "effects.prepare_route(",
        "let artifact_snapshot = self.geometry_bound_progress_directory_snapshot(",
        "self.observe_geometry_native_amx_per_height_evidence(",
        "effects.native(native_observation)?",
    )
    require(ARCHIVED, "if let Some(custody) = custody { custody.authenticate(self)?; }")
    ordered(ARCHIVED, "custody.authenticate(self)?;",
        "let _sidecar_guard = custody.is_none().then(|| self.sidecar_lock.lock());",
        "self.read_and_attest_geometry_native_amx_per_height_evidence(",
        "confirmed_snapshot != artifact_snapshot",
    )
