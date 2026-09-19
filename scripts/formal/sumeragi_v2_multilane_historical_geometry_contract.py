"""Structural obligations for historical geometry observation and attestation.

This binds the extracted owners and their actual callers; it does not establish
that the whole retirement scanner is pure or reserved across consensus.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Callable

from sumeragi_v2_multilane_geometry_evidence_contract import _code


NATIVE_MODULE = "SumeragiV2NativeApplicationEvidence"
GEOMETRY = "crates/iroha_core/src/kura/lane_geometry.rs"
HISTORICAL = "crates/iroha_core/src/kura/lane_geometry/historical_evidence.rs"
RECOVERY = "crates/iroha_core/src/kura/historical_autonomous_recovery.rs"
EFFECTS = "crates/iroha_core/src/kura/lane_geometry/retirement_observation.rs"
OBSERVER = "observe_geometry_historical_autonomous_recovery_records"
WRAPPER = "read_and_attest_geometry_historical_autonomous_recovery_records"
EXACT_READ = "read_historical_autonomous_recovery_record_with_identity"
BOUNDED = "bounded_historical_autonomous_recovery_entries"
SCANNER = "scan_lane_retirement_locked"
ARCHIVED = "ensure_archived_lane_work_released_with_custody"

HISTORICAL_GEOMETRY_BINDINGS = (
    (HISTORICAL, "struct", "ObservedHistoricalRecoveryFile", (
        "path: PathBuf", "metadata: StableSidecarMetadata", "bytes_hash: Hash",
    )),
    (HISTORICAL, "struct", "ObservedHistoricalRecoveryNamespace", (
        "directory: BoundProgressDirectory", "files: Vec<ObservedHistoricalRecoveryFile>",
    )),
    (HISTORICAL, "struct", "ObservedHistoricalRecoveryEvidence", (
        "kura: &'kura Kura", "outer: BoundProgressDirectory",
        "outer_inventory: &'kura BoundProgressDirectorySnapshot",
        "namespace: Option<ObservedHistoricalRecoveryNamespace>",
        "records: Vec<HistoricalAutonomousLaneRecoveryRecordV1>",
        "encoded_bytes: u64", "context: &'kura str",
    )),
    (HISTORICAL, "fn", OBSERVER, (
        "open_bound_progress_directory", "observed.ensure_unchanged()?",
        "geometry_file_identity(&before) != snapshot.identity", "canonical_sidecar_directory",
        "entry_limit.min(HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS)", BOUNDED,
        "records.try_reserve_exact(entries.len())", "files.try_reserve_exact(entries.len())",
        EXACT_READ, "descriptor.lane_incarnation != expected_incarnation",
        "descriptor.proposal_height <= activation_height", "expected_dataspace_id",
        "v2_finality_artifact_with_archive_under_prune_and_canonical_guards",
        "finality.commit_qc.execution_commitment", "finality.height_context",
        "finality.verify().is_err()", "finality.validate_for_header(&retained_header).is_err()",
        "validate_historical_autonomous_recovery_inventory_collisions(&records)?",
        "metadata: read.metadata", "bytes_hash: read.bytes_hash",
    )),
    (HISTORICAL, "fn", "ensure_unchanged", (
        "geometry_bound_progress_directory_snapshot", "self.outer_inventory.len()",
        "geometry_bound_progress_directory_unchanged", BOUNDED, "namespace.files.len()",
        "self.encoded_bytes", "encoded_bytes != self.encoded_bytes",
        "path != &observed.path", "sidecar_file_metadata_unchanged",
    )),
    (HISTORICAL, "fn", "attest", (
        "fn attest(self)", "self.ensure_unchanged()?", "sidecar_file_metadata_unchanged",
        "file.sync_all()", "read_regular_sidecar_snapshot",
        "HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES",
        "after.bytes_hash != observed.bytes_hash", "stable_sidecar_metadata_unchanged",
        "sync_dir(&namespace.directory.expected_path)", "Ok((self.records, self.encoded_bytes))",
    )),
    (HISTORICAL, "fn", WRAPPER, (OBSERVER, ".attest()")),
    (HISTORICAL, "fn", "into_observed", (
        "self.ensure_unchanged()?", "Ok((self.records, self.encoded_bytes))",
    )),
    (EFFECTS, "fn", "historical", (
        "ObservedHistoricalRecoveryEvidence", "Self::Observe => observation.into_observed()",
        "Self::MaintainAndAttest { .. } => observation.attest()",
    )),
    (RECOVERY, "fn", EXACT_READ, (
        "read_regular_sidecar_snapshot", "HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES",
        "historical_autonomous_recovery_read_matches_accounting",
        "HistoricalAutonomousLaneRecoveryRecordV1::decode_all",
        "historical_autonomous_recovery_record_bytes(&record) != snapshot.bytes",
        "hex::encode(record.recovery_id.as_ref())", "Some(expected_name.as_str())",
        "validate_historical_autonomous_recovery_record_shape(&record, path)?",
        "Ok(Some((record, snapshot)))",
    )),
    (RECOVERY, "fn", BOUNDED, (
        "HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS",
        "HISTORICAL_AUTONOMOUS_RECOVERY_HARD_MAX_AGGREGATE_BYTES",
        "bounded.len() >= record_limit", "historical_autonomous_recovery_record_name_is_canonical",
        "sidecar_is_single_link", "HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES",
        "checked_add(metadata.len())", "*bytes <= aggregate_byte_limit",
        "sidecar_directory_metadata_unchanged", "sidecar_file_metadata_unchanged",
        "bounded.sort_by", "encoded_bytes",
    )),
    (RECOVERY, "fn", "historical_autonomous_recovery_read_matches_accounting", (
        "u64::try_from(read.bytes.len()).ok() == Some(accounted.len())",
        "Kura::sidecar_file_metadata_unchanged(accounted, &read.metadata.file)",
    )),
)

HISTORICAL_GEOMETRY_SOURCE_RELATIVES = (
    Path("scripts/formal/sumeragi_v2_multilane_historical_geometry_contract.py"),
    Path("pytests/scripts/sumeragi_v2_multilane_historical_geometry_contract_test.py"),
    *(Path(path) for path in (GEOMETRY, HISTORICAL, RECOVERY, EFFECTS)),
)


def validate_historical_geometry_contract(
    root: Path, models: Any, errors: list[str], rust_binding_item: Callable,
) -> None:
    """Require the reviewed immutable owner, exact read, and consuming barriers."""

    native = [m for m in models if isinstance(m, dict) and m.get("module") == NATIVE_MODULE]
    bindings = native[0].get("production_symbols", ()) if len(native) == 1 else ()
    items: dict[str, str] = {}
    for path, kind, symbol, tokens in HISTORICAL_GEOMETRY_BINDINGS:
        matches = [b for b in bindings if isinstance(b, dict)
                   and (b.get("path"), b.get("kind"), b.get("symbol")) == (path, kind, symbol)]
        if len(matches) != 1:
            errors.append(f"historical geometry ledger owner {symbol} must occur exactly once")
        elif tuple(matches[0].get("required_tokens", ())) != tokens:
            errors.append(f"historical geometry reviewed tokens changed for {symbol}")
        item = rust_binding_item(root, path, kind, symbol, "historical geometry", errors)
        if item is not None:
            items[symbol] = _code(item)
            for token in tokens:
                if token not in item:
                    errors.append(f"historical geometry {symbol} is missing source token {token!r}")
    for symbol in (SCANNER, ARCHIVED):
        item = rust_binding_item(root, GEOMETRY, "fn", symbol, "historical geometry caller", errors)
        if item is not None:
            items[symbol] = _code(item)

    def require(symbol: str, *relations: str) -> None:
        if symbol in items:
            for relation in relations:
                if _code(relation) not in items[symbol]:
                    errors.append(f"historical geometry {symbol} is missing executable relation {relation!r}")

    def count(symbol: str, relation: str, expected: int) -> None:
        if symbol in items and items[symbol].count(_code(relation)) != expected:
            errors.append(f"historical geometry {symbol} must retain {expected} exact {relation!r} checks")

    def ordered(symbol: str, *relations: str) -> None:
        if symbol not in items:
            return
        cursor = 0
        for relation in relations:
            offset = items[symbol].find(_code(relation), cursor)
            if offset < 0:
                errors.append(f"historical geometry {symbol} is missing or reorders {relation!r}")
                break
            cursor = offset + len(_code(relation))

    for symbol in ("ObservedHistoricalRecoveryFile", "ObservedHistoricalRecoveryNamespace", "ObservedHistoricalRecoveryEvidence"):
        code = items.get(symbol, "")
        if re.search(r"pub(?:\([^)]*\))?\w+:", code) or "&mut" in code:
            errors.append(f"historical geometry {symbol} exposes mutable ownership")
    require("ObservedHistoricalRecoveryEvidence", "kura: &'kura Kura", "outer_inventory: &'kura BoundProgressDirectorySnapshot")
    for symbol in (OBSERVER, "ensure_unchanged", "into_observed", EXACT_READ, BOUNDED):
        for forbidden in ("sync_all(", "sync_dir(", ".attest(", ".write(", ".create(", ".truncate(", "fs::write(", "fs::remove", "fs::rename(", ".lock("):
            if forbidden in items.get(symbol, ""):
                errors.append(f"historical geometry {symbol} contains forbidden observation effect {forbidden!r}")
    require(OBSERVER,
        "kura: self", "outer_inventory: artifact_snapshot",
        "let Some(snapshot) = artifact_snapshot.get(raw_name) else { return Ok(observed); };",
        "if snapshot.kind != BoundProgressDirectoryEntryKind::Directory { return Err(",
        "if before.file_type().is_symlink() || !before.file_type().is_dir() || geometry_file_identity(&before) != snapshot.identity || self.canonical_sidecar_directory(&directory)?.is_none() { return Err(",
        "read_historical_autonomous_recovery_record_with_identity(&path, &directory, Some(&accounted))?",
        "if descriptor.lane_id != lane_id || descriptor.lane_incarnation != expected_incarnation || descriptor.proposal_height <= activation_height || expected_dataspace_id.is_some_and(|dataspace_id| descriptor.dataspace_id != dataspace_id) { return Err(",
        "if retained_header.hash() != record.canonical_body.block_hash || retained_header.height().get() != record.canonical_body.height || retained_header.view_change_index() != record.carrier_view || finality.height != record.canonical_body.height || finality.block_hash != record.canonical_body.block_hash || HashOf::new(&finality) != record.canonical_body.finality_artifact_hash || finality.commit_qc.execution_commitment != record.canonical_body.execution_commitment || finality.height_context != record.historical_context || finality.verify().is_err() || finality.validate_for_header(&retained_header).is_err() { return Err(",
    )
    count(OBSERVER, "observed.ensure_unchanged()?;", 2)
    ordered(OBSERVER, "observed.ensure_unchanged()?;", "artifact_snapshot.get(raw_name)",
        "entry_limit.min(HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS)",
        "bounded_historical_autonomous_recovery_entries(&directory, entry_limit, aggregate_byte_limit,",
        "read_historical_autonomous_recovery_record_with_identity(",
        "v2_finality_artifact_with_archive_under_prune_and_canonical_guards(",
        "validate_historical_autonomous_recovery_inventory_collisions(&records)?;",
        "observed.encoded_bytes = encoded_bytes;", "observed.ensure_unchanged()?;", "Ok(observed)",
    )
    require("ensure_unchanged",
        "if &kura.geometry_bound_progress_directory_snapshot(&self.outer, self.outer_inventory.len(), self.context)? != self.outer_inventory { return Err(",
        "let Some(namespace) = &self.namespace else { return Ok(()); };",
        "if !kura.geometry_bound_progress_directory_unchanged(&namespace.directory) { return Err(",
        "bounded_historical_autonomous_recovery_entries(&namespace.directory.expected_path, namespace.files.len(), self.encoded_bytes,",
        "if entries.len() != namespace.files.len() || encoded_bytes != self.encoded_bytes || entries.iter().zip(&namespace.files).any(|((path, metadata), observed)| { path != &observed.path || !Kura::sidecar_file_metadata_unchanged(&observed.metadata.file, metadata) }) { return Err(",
    )
    require("attest", "fn attest(self) -> Result<(Vec<HistoricalAutonomousLaneRecoveryRecordV1>, u64)> {",
        "if !Kura::sidecar_file_metadata_unchanged(&observed.metadata.file, &opened) { return Err(",
        "read_regular_sidecar_snapshot(path, &namespace.directory.expected_path, HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES)?",
        "if after.bytes_hash != observed.bytes_hash || !Kura::stable_sidecar_metadata_unchanged(&observed.metadata, &after.metadata) { return Err(",
    )
    count("attest", "self.ensure_unchanged()?;", 2)
    ordered("attest", "self.ensure_unchanged()?;", "for observed in &namespace.files {",
        "sidecar_file_metadata_unchanged(&observed.metadata.file, &opened)", "file.sync_all()",
        "read_regular_sidecar_snapshot(", "after.bytes_hash != observed.bytes_hash",
        "sync_dir(&namespace.directory.expected_path)", "self.ensure_unchanged()?;",
        "Ok((self.records, self.encoded_bytes))",
    )
    require(WRAPPER,
        "self.observe_geometry_historical_autonomous_recovery_records(lane_artifacts, artifact_snapshot, lane_id, expected_dataspace_id, expected_incarnation, activation_height, entry_limit, aggregate_byte_limit, context)?.attest()",
    )
    require("into_observed",
        "fn into_observed(self) -> Result<(Vec<HistoricalAutonomousLaneRecoveryRecordV1>, u64)> { self.ensure_unchanged()?; Ok((self.records, self.encoded_bytes)) }",
    )
    require(EXACT_READ,
        "if accounted.is_some_and(|accounted| { !historical_autonomous_recovery_read_matches_accounting(accounted, &snapshot) }) { return Err(",
        "if historical_autonomous_recovery_record_bytes(&record) != snapshot.bytes || path.file_name().and_then(std::ffi::OsStr::to_str) != Some(expected_name.as_str()) { return Err(",
        "self.validate_historical_autonomous_recovery_record_shape(&record, path)?; Ok(Some((record, snapshot)))",
    )
    require("historical_autonomous_recovery_read_matches_accounting",
        "u64::try_from(read.bytes.len()).ok() == Some(accounted.len()) && Kura::sidecar_file_metadata_unchanged(accounted, &read.metadata.file)",
    )
    require(BOUNDED,
        "if record_limit > HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS || aggregate_byte_limit > HISTORICAL_AUTONOMOUS_RECOVERY_HARD_MAX_AGGREGATE_BYTES { return Err(",
        "if bounded.len() >= record_limit { return Err(",
        "if path.parent() != Some(directory) || !historical_autonomous_recovery_record_name_is_canonical(&entry.file_name()) { return Err(",
        "if metadata.file_type().is_symlink() || !metadata.file_type().is_file() || !Kura::sidecar_is_single_link(&metadata) || metadata.len() == 0 || metadata.len() > u64::try_from(HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES)? { return Err(",
        "checked_add(metadata.len()).filter(|bytes| *bytes <= aggregate_byte_limit).ok_or_else(",
    )
    require("historical",
        "match self { Self::Observe => observation.into_observed(), Self::MaintainAndAttest { .. } => observation.attest() }",
    )
    count(SCANNER, f"self.{OBSERVER}(", 2)
    count(SCANNER, "effects.historical(", 2)
    ordered(SCANNER, f"self.{OBSERVER}(",
        "effects.historical(historical_observation)?",
        f"self.{OBSERVER}(",
        "effects.historical(confirmed_historical_observation)?",
        "confirmed_route_historical_recoveries != expected_route_historical_recoveries",
    )
    count(ARCHIVED, f"self.{WRAPPER}(", 2)
    require(SCANNER,
        "if confirmed_route_historical_recoveries != expected_route_historical_recoveries || confirmed_historical_recovery_bytes != historical_recovery_bytes { return Err(",
    )
    require(ARCHIVED,
        "if confirmed_historical_recoveries != historical_recoveries || confirmed_historical_recovery_bytes != historical_recovery_bytes { return Err(",
    )
