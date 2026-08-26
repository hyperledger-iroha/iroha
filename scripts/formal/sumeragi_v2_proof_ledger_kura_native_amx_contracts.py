# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.


def _kura_native_amx_standalone_evidence_production_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Bind retirement/archive GC to the standalone Native AMX namespace."""

    kura_path = repo_root / "crates" / "iroha_core" / "src" / "kura.rs"
    lane_geometry_path = (
        repo_root / "crates" / "iroha_core" / "src" / "kura" / "lane_geometry.rs"
    )
    errors: list[str] = []
    if not kura_path.is_file() or kura_path.is_symlink():
        errors.append(
            f"{kura_path}: Kura Native AMX aggregate-byte source must be a "
            "regular file"
        )
        return errors
    if not lane_geometry_path.is_file() or lane_geometry_path.is_symlink():
        errors.append(
            f"{lane_geometry_path}: Kura standalone Native AMX production "
            "source must be a regular file"
        )
        return errors
    kura_source = kura_path.read_text(encoding="utf-8")
    source = lane_geometry_path.read_text(encoding="utf-8")

    aggregate_byte_source = _require_qualified_rust_item(
        kura_path,
        kura_source,
        "Kura",
        "native_amx_participant_evidence_file_bytes",
        errors,
        "standalone Native AMX configured aggregate-byte source",
    )
    _require_exact_rust_tokens(
        kura_path,
        aggregate_byte_source,
        """
fn native_amx_participant_evidence_file_bytes(&self) -> u64 {
u64::try_from(self.pending_control_sidecar_limits.aggregate_bytes)
    .expect("configured pending-control sidecar bytes fit u64")
}
""",
        "standalone Native AMX configured aggregate byte source must be the "
        "pending-control sidecar geometry",
        errors,
    )

    scanner = _require_qualified_rust_item(
        lane_geometry_path,
        source,
        "Kura",
        "read_geometry_native_amx_per_height_evidence",
        errors,
        "standalone Native AMX per-height evidence scanner",
    )
    scanner_contracts = (
        (
            """
for (raw_name, entry_snapshot) in artifact_snapshot {
    let path = lane_artifacts.join(raw_name);
    let Some((kind, lane_block_height, temporary)) =
        Self::parse_native_amx_evidence_path(&path)?
    else {
        continue;
    };
    if temporary {
""",
            "standalone Native AMX scanner must classify only canonical "
            "per-height names and reject temporary evidence",
        ),
        (
            """
let retained_count = match kind {
    NativeAmxEvidenceKind::Manifest => manifests.len(),
    NativeAmxEvidenceKind::Receipt => receipts.len(),
};
if retained_count >= retained_record_limit {
""",
            "standalone Native AMX manifest and receipt counts must be bounded "
            "independently",
        ),
        (
            """
if entry_snapshot.kind != BoundProgressDirectoryEntryKind::File {
""",
            "standalone Native AMX scanner must reject non-regular snapshot "
            "entries",
        ),
        (
            """
let metadata =
    Self::regular_sidecar_metadata_for(&self.store_root, &path, lane_artifacts)?
""",
            "standalone Native AMX scanner must use the bound regular-file "
            "metadata path",
        ),
        (
            """
evidence_bytes = evidence_bytes.checked_add(encoded_len).ok_or_else(|| {
""",
            "standalone Native AMX shared aggregate byte total must be "
            "overflow checked",
        ),
        (
            """
if evidence_bytes > self.native_amx_participant_evidence_file_bytes() {
""",
            "standalone Native AMX shared aggregate byte bound must use the "
            "configured source of truth",
        ),
        (
            """
let before = self
    .read_regular_sidecar_snapshot(&path, lane_artifacts, payload_limit)?
""",
            "standalone Native AMX scanner must use bounded stable snapshot "
            "reads",
        ),
        (
            """
if !Self::stable_sidecar_metadata_unchanged(&metadata, &before.metadata) {
""",
            "standalone Native AMX scanner must bind stable metadata before "
            "decode",
        ),
        (
            """
NativeAmxEvidenceKind::Manifest => {
    let artifact = norito::decode_from_bytes::<
        NativeAmxParticipantApplicationManifestArtifactV1,
    >(&before.bytes)
    .map_err(Error::NoritoFrame)?;
    if norito::to_bytes(&artifact).map_err(Error::NoritoFrame)? != before.bytes
        || artifact.leaf.participant_height != lane_block_height
        || Self::validate_native_amx_participant_application_manifest_artifact(
            &artifact,
        )
        .is_err()
""",
            "standalone Native AMX manifest canonical decode, height binding, "
            "and validation",
        ),
        (
            """
NativeAmxEvidenceKind::Receipt => {
    let artifact = norito::decode_from_bytes::<
        NativeAmxParticipantApplicationReceiptArtifact,
    >(&before.bytes)
    .map_err(Error::NoritoFrame)?;
    if norito::to_bytes(&artifact).map_err(Error::NoritoFrame)? != before.bytes
        || artifact.participant_proposal.descriptor.lane_block_height
            != lane_block_height
        || Self::validate_native_amx_participant_application_receipt_artifact(
            &artifact,
        )
        .is_err()
""",
            "standalone Native AMX receipt canonical decode, height binding, "
            "and validation",
        ),
        (
            """
if !Self::sidecar_file_metadata_unchanged(&before.metadata.file, &opened_metadata) {
""",
            "standalone Native AMX durability open must retain the scanned file "
            "identity",
        ),
        (
            """
file.sync_all()
""",
            "standalone Native AMX files must be durability attested",
        ),
        (
            """
if after.bytes_hash != before.bytes_hash
    || !Self::stable_sidecar_metadata_unchanged(&before.metadata, &after.metadata)
{
""",
            "standalone Native AMX durability sync must preserve exact bytes "
            "and stable metadata",
        ),
        (
            """
sync_dir(lane_artifacts)
""",
            "standalone Native AMX directory must be durability attested",
        ),
    )
    for fragment, description in scanner_contracts:
        _require_rust_token_sequence(
            lane_geometry_path,
            scanner,
            fragment,
            description,
            errors,
        )

    retirement = _require_qualified_rust_item(
        lane_geometry_path,
        source,
        "Kura",
        "ensure_first_release_lane_retirement_admissible_with_certified_locked",
        errors,
        "live standalone Native AMX retirement evidence join",
    )
    retirement_contracts = (
        (
            """
let artifact_snapshot = self.geometry_bound_progress_directory_snapshot(
    &lane_artifacts_guard,
    per_route_artifact_file_limit,
    "first-release lane retirement artifact scan",
)?;
""",
            "live Native AMX retirement must bind an immutable artifact snapshot",
        ),
        (
            """
if snapshot.kind == BoundProgressDirectoryEntryKind::Symlink {
""",
            "live Native AMX retirement must reject symlink artifacts",
        ),
        (
            """
if snapshot.kind != BoundProgressDirectoryEntryKind::File {
""",
            "live Native AMX retirement must reject non-regular artifacts",
        ),
        (
            """
if name.ends_with(".tmp") {
""",
            "live Native AMX retirement must reject temporary artifacts",
        ),
        (
            """
if Self::parse_native_amx_evidence_path(&path)?.is_some() {
    continue;
}
""",
            "live Native AMX retirement allowlist must use the standalone "
            "per-height parser",
        ),
        (
            """
if Self::autonomous_lane_block_attempt_coordinates(name).is_some()
    || Self::autonomous_lifecycle_cursor_coordinates(name).is_some() || Self::autonomous_lifecycle_terminal_outcome_coordinates(name).is_some()
    || Self::autonomous_two_height_coordinates(
        name,
        AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX,
    )
    .is_some()
    || Self::autonomous_one_height_coordinate(
        name,
        AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_PREFIX,
    )
    .is_some()
    || name == AUTONOMOUS_LANE_ROUTE_LATEST_ATTEMPT_FILE
{
    continue;
}
return Err(Error::IO(
    std::io::Error::new(
        ErrorKind::InvalidData,
        "lane retirement scan encountered an unknown artifact filename",
    ),
    path,
));
""",
            "live Native AMX retirement must reject every unexpected or legacy "
            "artifact after the complete allowlist",
        ),
        (
            """
let (retained_native_manifests, retained_native_receipts) = self
    .read_geometry_native_amx_per_height_evidence(
        &lane_artifacts,
        &artifact_snapshot,
        self.native_amx_participant_evidence_retention().get(),
        "lane retirement",
    )?;
""",
            "live Native AMX retirement must scan the exact immutable snapshot",
        ),
        (
            """
if manifest.leaf.lane_id != entry.lane_id
    || manifest.leaf.dataspace_id != entry.dataspace_id
    || self
        .require_active_lane_incarnation(
            &entry,
            manifest.leaf.lane_incarnation,
            manifest.leaf.application_block_height,
        )
        .is_err()
""",
            "live Native AMX manifests must join the active route, incarnation, "
            "and application height",
        ),
        (
            """
if self
    .require_active_lane_artifact(&entry, descriptor)
    .is_err()
    || receipt.manifest_artifact_hash != HashOf::new(manifest)
    || !Self::native_amx_participant_receipt_matches_manifest_leaf(
        &receipt,
        &manifest.leaf,
    )
""",
            "live Native AMX receipts must join the active descriptor and exact "
            "manifest leaf",
        ),
        (
            """
if !native_amx_retained_windows_are_complete(
    &native_manifest_heights,
    &native_receipt_heights,
) {
""",
            "live Native AMX evidence must form a complete retained suffix",
        ),
        (
            """
match native_receipt_heights.last().copied() {
    Some(latest_height) => {
""",
            "live Native AMX latest lookup must select the highest retained "
            "receipt",
        ),
        (
            """
self.require_active_lane_incarnation(
    &entry,
    latest.lane_incarnation,
    latest.application_block_height,
)
.is_ok()
    && latest.matches_receipt(receipt)
""",
            "live Native AMX latest pointer must exactly join the active "
            "incarnation and highest receipt",
        ),
        (
            """
for manifest in native_manifests.values() {
    if !self
        .native_amx_participant_application_manifest_matches_available_finality_under_prune_and_canonical_guards(manifest)
""",
            "live Native AMX manifests must revalidate canonical finality",
        ),
        (
            """
if !self.native_amx_participant_application_receipt_matches_manifest_and_available_evidence_under_prune_canonical_and_sidecar_guards(
    receipt,
    manifest,
)
""",
            "live Native AMX receipts must revalidate exact application "
            "evidence",
        ),
        (
            """
let confirmed_snapshot = self.geometry_bound_progress_directory_snapshot(
    &lane_artifacts_guard,
    per_route_artifact_file_limit,
    "lane retirement artifact rescan",
)?;
if confirmed_snapshot != artifact_snapshot
    || !self.geometry_bound_progress_directory_unchanged(&lane_artifacts_guard)
{
""",
            "live Native AMX retirement must prove the immutable snapshot "
            "unchanged",
        ),
    )
    for fragment, description in retirement_contracts:
        _require_rust_token_sequence(
            lane_geometry_path,
            retirement,
            fragment,
            description,
            errors,
        )

    archive = _require_qualified_rust_item(
        lane_geometry_path,
        source,
        "Kura",
        "ensure_archived_lane_work_released",
        errors,
        "archived standalone Native AMX evidence join",
    )
    archive_contracts = (
        (
            """
let lane_artifacts_guard =
    Self::open_bound_progress_directory(&self.store_root, &lane_artifacts)?;
let artifact_snapshot = self.geometry_bound_progress_directory_snapshot(
    &lane_artifacts_guard,
    MAX_GEOMETRY_ARCHIVE_ENTRIES,
    "retired lane artifact scan",
)?;
""",
            "archived Native AMX GC must bind an immutable artifact snapshot",
        ),
        (
            """
if snapshot.kind == BoundProgressDirectoryEntryKind::Symlink {
""",
            "archived Native AMX GC must reject symlink artifacts",
        ),
        (
            """
if snapshot.kind != BoundProgressDirectoryEntryKind::File {
""",
            "archived Native AMX GC must reject non-regular artifacts",
        ),
        (
            """
if name.ends_with(".tmp") {
""",
            "archived Native AMX GC must reject temporary artifacts",
        ),
        (
            """
if Self::parse_native_amx_evidence_path(&path)?.is_some() {
    continue;
}
""",
            "archived Native AMX allowlist must use the standalone per-height "
            "parser",
        ),
        (
            """
if Self::autonomous_lane_block_attempt_coordinates(name).is_some()
    || Self::autonomous_lifecycle_cursor_coordinates(name).is_some()
    || Self::autonomous_lifecycle_terminal_outcome_coordinates(name).is_some()
    || Self::autonomous_two_height_coordinates(
        name,
        AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX,
    )
    .is_some()
    || Self::autonomous_one_height_coordinate(
        name,
        AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_PREFIX,
    )
    .is_some()
    || name == AUTONOMOUS_LANE_ROUTE_LATEST_ATTEMPT_FILE
{
    continue;
}
return Err(Error::IO(
    std::io::Error::new(
        ErrorKind::InvalidData,
        "lane artifact archive contains an unexpected artifact",
    ),
    path,
));
""",
            "archived Native AMX GC must reject every unexpected or legacy "
            "artifact after the complete allowlist",
        ),
        (
            """
let (retained_native_manifests, retained_native_receipts) = self
    .read_geometry_native_amx_per_height_evidence(
        &lane_artifacts,
        &artifact_snapshot,
        self.native_amx_participant_evidence_retention().get(),
        "retired lane",
    )?;
""",
            "archived Native AMX GC must scan the exact immutable snapshot",
        ),
        (
            """
if manifest.leaf.lane_incarnation != binding.incarnation
    || !self
        .native_amx_participant_application_manifest_matches_available_finality_under_prune_and_canonical_guards(
            &manifest,
        )
""",
            "archived Native AMX manifests must join the archived incarnation "
            "and canonical finality",
        ),
        (
            """
if !native_amx_retained_windows_are_complete(
    &native_manifest_heights,
    &native_receipt_heights,
) {
""",
            "archived Native AMX evidence must form a complete retained suffix",
        ),
        (
            """
if receipt.participant_proposal.descriptor.lane_incarnation != binding.incarnation
    || receipt.manifest_artifact_hash != HashOf::new(manifest)
    || !Self::native_amx_participant_receipt_matches_manifest_leaf(
        &receipt,
        &manifest.leaf,
    )
    || !self.native_amx_participant_application_receipt_matches_manifest_and_available_evidence_under_prune_canonical_and_sidecar_guards(
        &receipt,
        manifest,
    )
""",
            "archived Native AMX receipts must join the incarnation, manifest, "
            "and canonical application",
        ),
        (
            """
if native_receipt_heights.last().copied() == Some(lane_block_height) {
    latest_native_receipt = Some(receipt);
}
""",
            "archived Native AMX latest selection must use the highest retained "
            "receipt",
        ),
        (
            """
decode_native_amx_participant_receipt_latest_index_for_route(
    binding.lane_id,
    receipt.participant_proposal.descriptor.dataspace_id,
    &native_receipt_latest,
)
""",
            "archived Native AMX latest pointer must join the exact route",
        ),
        (
            """
latest.lane_incarnation == binding.incarnation
    && latest.matches_receipt(receipt)
""",
            "archived Native AMX latest pointer must join the archived "
            "incarnation and highest receipt",
        ),
        (
            """
let confirmed_snapshot = self.geometry_bound_progress_directory_snapshot(
    &lane_artifacts_guard,
    MAX_GEOMETRY_ARCHIVE_ENTRIES,
    "retired lane artifact rescan",
)?;
if confirmed_snapshot != artifact_snapshot
    || !self.geometry_bound_progress_directory_unchanged(&lane_artifacts_guard)
{
""",
            "archived Native AMX GC must prove the immutable snapshot unchanged",
        ),
    )
    for fragment, description in archive_contracts:
        _require_rust_token_sequence(
            lane_geometry_path,
            archive,
            fragment,
            description,
            errors,
        )

    obsolete_dense_symbols = (
        "native_amx_application_manifest_paths_for_entry",
        "native_amx_participant_receipt_paths_for_entry",
        "NATIVE_AMX_APPLICATION_MANIFESTS_DATA_FILE",
        "NATIVE_AMX_APPLICATION_MANIFESTS_INDEX_FILE",
        "NATIVE_AMX_PARTICIPANT_RECEIPTS_DATA_FILE",
        "NATIVE_AMX_PARTICIPANT_RECEIPTS_INDEX_FILE",
    )
    obsolete_dense_filenames = (
        "native_amx_application_manifests.norito",
        "native_amx_application_manifests.index",
        "native_amx_participant_receipts.norito",
        "native_amx_participant_receipts.index",
    )
    for item, label in (
        (scanner, "standalone Native AMX scanner"),
        (retirement, "live Native AMX retirement"),
        (archive, "archived Native AMX GC"),
    ):
        if item is None:
            continue
        item_tokens = rust_code_tokens(item.source)
        for symbol in obsolete_dense_symbols:
            observed = _token_sequence_count(
                item_tokens,
                rust_code_tokens(symbol),
            )
            if observed:
                errors.append(
                    f"{lane_geometry_path}:{item.line}: {label} must reject "
                    "obsolete dense Native AMX evidence acceptance; found "
                    f"{symbol} {observed} time(s)"
                )
        for filename in obsolete_dense_filenames:
            observed = item.source.count(filename)
            if observed:
                errors.append(
                    f"{lane_geometry_path}:{item.line}: {label} must reject "
                    "obsolete dense Native AMX evidence acceptance; found "
                    f"{filename} {observed} time(s)"
                )

    return errors
