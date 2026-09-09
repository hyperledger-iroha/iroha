// Included at Kura module scope. File-key counts never confer evidence authority.

/// Immutable limits owned by the configured Kura instance.
#[derive(Clone, Copy)]
struct EvidenceResourceLimits {
    native_record_bytes: u64,
    native_prune_intent_bytes: u64,
    fastpq_artifacts: iroha_config::parameters::actual::KuraFastpqArtifactPolicy,
}

impl Kura {
    fn evidence_resource_limits(&self) -> EvidenceResourceLimits {
        EvidenceResourceLimits {
            native_record_bytes: (self.pending_control_sidecar_limits.aggregate_bytes as u64)
                .min(STRICT_INIT_MAX_BLOCK_BYTES),
            native_prune_intent_bytes: self.native_amx_evidence_prune_intent_max_bytes as u64,
            fastpq_artifacts: self.fastpq_artifact_policy,
        }
    }
}

fn evidence_resource_hex(text: &str, bytes: usize) -> bool {
    text.len() == bytes * 2
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn evidence_resource_height(text: &str) -> bool {
    text.len() == 20
        && text
            .parse::<u64>()
            .is_ok_and(|height| height != 0 && text == format!("{height:020}"))
}

/// Classify exact existing standalone record formats, with no payload decoding.
///
/// The caller separately binds the path to the complete authenticated Kura scope.
/// An occupied reserved namespace with an unknown filename is an error, never a
/// zero contribution. Main and temporary names remain separate physical records.
fn evidence_resource_kind(
    path: &Path,
    limits: EvidenceResourceLimits,
) -> std::result::Result<Option<(IndexResourceFormat, bool)>, resource_inventory::Unavailable> {
    use resource_inventory::Unavailable as Missing;
    let name = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or(Missing::OwnerMismatch)?;
    let directory = path
        .parent()
        .and_then(Path::file_name)
        .and_then(std::ffi::OsStr::to_str);
    let singleton =
        |maximum, temporary| Ok(Some((IndexResourceFormat::Singleton(maximum), temporary)));
    let (stable, temporary) = name
        .strip_suffix(".tmp")
        .map_or((name, false), |stable| (stable, true));
    let stem = stable.strip_suffix(".norito");

    // Reserved namespace ownership wins over generic known marker basenames.
    if directory == Some(fastpq_artifact_store::DIRECTORY) {
        limits
            .fastpq_artifacts
            .validate()
            .map_err(|_| Missing::InvalidInventory)?;
        let maximum = limits.fastpq_artifacts.max_artifact_bytes.get() as u64;
        if name == fastpq_artifact_store::TEMPORARY {
            return Ok(Some((
                IndexResourceFormat::TemporarySingleton(maximum),
                true,
            )));
        }
        if name
            .strip_suffix(".norito")
            .is_some_and(|hash| evidence_resource_hex(hash, Hash::LENGTH))
        {
            return singleton(maximum, false);
        }
        return Err(Missing::OwnerMismatch);
    }

    let by_height = match directory {
        Some(WSV_CHECKPOINTS_DIR_NAME) => Some(MAX_WSV_CHECKPOINT_BYTES as u64),
        Some(COMMIT_MANIFESTS_DIR_NAME) => Some(MAX_COMMIT_MANIFEST_BYTES as u64),
        Some(RETAINED_BLOCKS_DIR_NAME | RETAINED_BLOCK_REWRITE_STAGING_DIR_NAME) => {
            Some(MAX_RETAINED_BLOCK_RECORD_BYTES as u64)
        }
        Some(V2_FINALITY_ARTIFACTS_DIR_NAME) => Some(MAX_KURA_V2_FINALITY_RECORD_BYTES as u64),
        Some(KAGEMUSHA_FINALITY_STAGING_DIR_NAME | KAGEMUSHA_FINALITY_SIDECARS_DIR_NAME) => {
            Some(MAX_KAGEMUSHA_FINALITY_SIDECAR_BYTES as u64)
        }
        _ => None,
    };
    if let Some(maximum) = by_height {
        if stem.is_some_and(evidence_resource_height) {
            return singleton(maximum, temporary);
        }
        // Generic atomic replacement residue cannot identify the intended height.
        // Its actual length is still bounded by this single-format namespace.
        if name.starts_with(".kura-sidecar-") && name.len() > ".kura-sidecar-".len() {
            return singleton(maximum, true);
        }
        return Err(Missing::OwnerMismatch);
    }
    let by_hash = match directory {
        Some(PENDING_MERGE_ENTRIES_DIR) => Some(MAX_MERGE_LEDGER_ENTRY_BYTES as u64),
        Some(PENDING_QUEUE_PLAN_ADMISSIONS_DIR) => {
            Some(MAX_PENDING_QUEUE_PLAN_ADMISSION_CERTIFICATE_BYTES as u64)
        }
        Some(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1) => {
            Some(HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES as u64)
        }
        Some(KAGEMUSHA_MINT_OUTBOX_DIR_NAME) => Some(MAX_KAGEMUSHA_MINT_OUTBOX_ENTRY_BYTES as u64),
        _ => None,
    };
    if let Some(maximum) = by_hash {
        if stem.is_some_and(|stem| evidence_resource_hex(stem, Hash::LENGTH)) {
            return singleton(maximum, temporary);
        }
        if name.starts_with(".kura-sidecar-") && name.len() > ".kura-sidecar-".len() {
            return singleton(maximum, true);
        }
        if directory == Some(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1)
            && Kura::historical_autonomous_recovery_publication_kind(std::ffi::OsStr::new(name))
                == Some(HistoricalAutonomousRecoveryPublicationKind::Temporary)
        {
            return singleton(maximum, true);
        }
        return Err(Missing::OwnerMismatch);
    }
    if directory == Some(KAGEMUSHA_MINT_AUTHORITY_DIR_NAME) {
        if stem
            .and_then(|stem| stem.split_once('-'))
            .is_some_and(|(release, authority)| {
                evidence_resource_hex(release, Hash::LENGTH)
                    && evidence_resource_hex(authority, Hash::LENGTH)
            })
        {
            return singleton(
                MAX_KAGEMUSHA_MINT_AUTHORITY_CHECKPOINT_BYTES as u64,
                temporary,
            );
        }
        if name.starts_with(".kura-sidecar-") && name.len() > ".kura-sidecar-".len() {
            return singleton(MAX_KAGEMUSHA_MINT_AUTHORITY_CHECKPOINT_BYTES as u64, true);
        }
        return Err(Missing::OwnerMismatch);
    }
    if directory.is_some_and(|directory| {
        directory.starts_with(AUTONOMOUS_LANE_ENTRYPOINT_CLAIMS_DIR_PREFIX)
    }) {
        let shard = directory
            .and_then(|directory| {
                directory.strip_prefix(AUTONOMOUS_LANE_ENTRYPOINT_CLAIMS_DIR_PREFIX)
            })
            .and_then(|tail| tail.strip_prefix('_'))
            .ok_or(Missing::OwnerMismatch)?;
        let (network, entrypoint) = stem
            .and_then(|stem| stem.split_once('_'))
            .ok_or(Missing::OwnerMismatch)?;
        if !evidence_resource_hex(shard, 1)
            || !evidence_resource_hex(network, Hash::LENGTH)
            || !evidence_resource_hex(entrypoint, Hash::LENGTH)
            || !entrypoint.starts_with(shard)
        {
            return Err(Missing::OwnerMismatch);
        }
        return singleton(AUTONOMOUS_LANE_ENTRYPOINT_CLAIM_MAX_BYTES as u64, temporary);
    }

    let fixed = match stable {
        COUNT_FILE_NAME => Some(MAX_BLOCK_COMMIT_MARKER_BYTES as u64),
        VERIFIED_SNAPSHOT_TAIL_FILE_NAME => Some(MAX_VERIFIED_SNAPSHOT_TAIL_MARKER_BYTES as u64),
        DA_BLOCK_REWRITE_STAGE_FILE_NAME => Some(MAX_DA_BLOCK_REWRITE_STAGE_BYTES),
        EVICTION_COMPACTION_STAGE_FILE_NAME => Some(MAX_EVICTION_COMPACTION_STAGE_BYTES),
        CANONICAL_ASSOCIATION_STAGE_FILE_NAME => Some(MAX_CANONICAL_ASSOCIATION_STAGE_BYTES),
        AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_FILE => {
            Some(AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_MAX_BYTES as u64)
        }
        PRUNE_INTENT_FILE_NAME => Some(PRUNE_INTENT_MAX_BYTES as u64),
        _ => None,
    };
    if let Some(maximum) = fixed {
        return singleton(maximum, temporary);
    }
    if let Some((maximum, temporary)) = lane_geometry::resource_evidence_file_kind(name) {
        return singleton(maximum, temporary);
    }
    if Kura::is_autonomous_publication_quarantine_name(
        name,
        AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
    ) || Kura::is_unresolved_autonomous_publication_temporary_name(
        name,
        AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
    ) {
        return singleton(
            AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_MAX_BYTES as u64,
            true,
        );
    }
    for (prefix, maximum) in [
        (
            ".verified-snapshot-tail-",
            MAX_VERIFIED_SNAPSHOT_TAIL_MARKER_BYTES as u64,
        ),
        (".kura-eviction-stage-", MAX_EVICTION_COMPACTION_STAGE_BYTES),
        (".kura-da-rewrite-", MAX_DA_BLOCK_REWRITE_STAGE_BYTES),
    ] {
        if name.starts_with(prefix) && name.len() > prefix.len() {
            return singleton(maximum, true);
        }
    }
    for suffix in [".append.build.tmp", ".append.intent.tmp"] {
        if let Some(index_name) = name.strip_suffix(suffix) {
            if matches!(
                index_resource_kind(&path.with_file_name(index_name)),
                Some((_, IndexResourceFormat::SidecarV1, false))
            ) {
                return singleton(BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES as u64, true);
            }
            return Err(Missing::OwnerMismatch);
        }
    }
    if directory == Some(LANE_ARTIFACTS_DIR_NAME) {
        let fixed = match name {
            LATEST_CERTIFIED_LANE_BLOCK_FRONTIER_FILE => Some((STRICT_INIT_MAX_BLOCK_BYTES, false)),
            LATEST_CERTIFIED_LANE_BLOCK_FRONTIER_BUILD_FILE => {
                Some((STRICT_INIT_MAX_BLOCK_BYTES, true))
            }
            LANE_MERGE_APPLICATION_FRONTIER_FILE => {
                Some((LANE_MERGE_APPLICATION_FRONTIER_MAX_BYTES as u64, false))
            }
            AUTONOMOUS_LANE_ROUTE_LATEST_ATTEMPT_FILE => {
                Some((AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_MAX_BYTES as u64, false))
            }
            NATIVE_AMX_EVIDENCE_PRUNE_INTENT_FILE => {
                Some((limits.native_prune_intent_bytes, false))
            }
            NATIVE_AMX_EVIDENCE_PRUNE_INTENT_TEMP_FILE => {
                Some((limits.native_prune_intent_bytes, true))
            }
            _ => None,
        };
        if let Some((maximum, temporary)) = fixed {
            return singleton(maximum, temporary);
        }
        for (prefix, maximum) in [
            (
                AUTONOMOUS_LANE_BLOCK_ATTEMPT_PREFIX,
                MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES,
            ),
            (
                AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX,
                AUTONOMOUS_LANE_BLOCK_VIEW_STATE_MAX_BYTES,
            ),
            (
                AUTONOMOUS_LIFECYCLE_CURSOR_PREFIX,
                AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES,
            ),
            (
                AUTONOMOUS_LIFECYCLE_BOOTSTRAP_PREFIX,
                AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
            ),
            (
                AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_PREFIX,
                AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES,
            ),
        ] {
            if Kura::autonomous_two_height_coordinates(stable, prefix).is_some() {
                return singleton(maximum as u64, temporary);
            }
        }
        if Kura::autonomous_one_height_coordinate(
            stable,
            AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_PREFIX,
        )
        .is_some()
        {
            return singleton(
                AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_MAX_BYTES as u64,
                temporary,
            );
        }
        if let Some((_, _, temporary)) =
            Kura::parse_native_amx_evidence_path(path).map_err(|_| Missing::OwnerMismatch)?
        {
            return singleton(limits.native_record_bytes, temporary);
        }
        if name.starts_with(AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX) {
            return singleton(AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES as u64, true);
        }
        if name.starts_with("autonomous_")
            || name.starts_with("native_amx_")
            || name.starts_with("latest_certified_frontier")
            || name.starts_with("merge_application_frontier")
            || name.starts_with(".kura-sidecar-")
        {
            // The existing V1 ordered pair and Native latest index have other owners.
            if index_resource_kind(path).is_some()
                || matches!(
                    stable,
                    AUTONOMOUS_LANE_MERGE_BUNDLES_DATA_FILE
                        | CANONICAL_AUTONOMOUS_LANE_REPLICAS_DATA_FILE
                )
            {
                return Ok(None);
            }
            return Err(Missing::OwnerMismatch);
        }
    }
    if [
        COUNT_FILE_NAME,
        VERIFIED_SNAPSHOT_TAIL_FILE_NAME,
        DA_BLOCK_REWRITE_STAGE_FILE_NAME,
        EVICTION_COMPACTION_STAGE_FILE_NAME,
        CANONICAL_ASSOCIATION_STAGE_FILE_NAME,
        AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_FILE,
        PRUNE_INTENT_FILE_NAME,
        "lane_geometry_journal.norito",
        ".lane-incarnation.norito",
    ]
    .iter()
    .any(|reserved| name.starts_with(reserved))
    {
        return Err(Missing::OwnerMismatch);
    }
    Ok(None)
}

#[cfg(test)]
mod evidence_resource_tests {
    use super::*;

    fn limits() -> EvidenceResourceLimits {
        EvidenceResourceLimits {
            native_record_bytes: 19,
            native_prune_intent_bytes: 11,
            fastpq_artifacts: iroha_config::parameters::defaults::kura::FASTPQ_ARTIFACT_POLICY,
        }
    }

    #[test]
    fn evidence_namespace_parsers_bind_exact_height_hash_and_claim_shard() {
        let root = Path::new("/unused-evidence-inventory");
        for directory in [
            WSV_CHECKPOINTS_DIR_NAME,
            COMMIT_MANIFESTS_DIR_NAME,
            RETAINED_BLOCKS_DIR_NAME,
            V2_FINALITY_ARTIFACTS_DIR_NAME,
        ] {
            for name in [
                "00000000000000000001.norito",
                "18446744073709551615.norito.tmp",
            ] {
                assert!(
                    evidence_resource_kind(&root.join(directory).join(name), limits())
                        .unwrap()
                        .is_some()
                );
            }
            for name in [
                "1.norito",
                "00000000000000000000.norito",
                "18446744073709551616.norito",
                "00000000000000000001.norito.old",
            ] {
                assert!(
                    evidence_resource_kind(&root.join(directory).join(name), limits()).is_err()
                );
            }
        }
        let network = Kura::fixed_bytes_path_component(Hash::new(b"resource network").as_ref());
        let key = Kura::fixed_bytes_path_component(Hash::new(b"resource entrypoint").as_ref());
        let shard = &key[..2];
        let directory = root.join(format!(
            "{AUTONOMOUS_LANE_ENTRYPOINT_CLAIMS_DIR_PREFIX}_{shard}"
        ));
        let name = format!("{network}_{key}.norito");
        assert!(
            evidence_resource_kind(&directory.join(&name), limits())
                .unwrap()
                .is_some()
        );
        assert!(
            evidence_resource_kind(&directory.join(format!("{name}.tmp")), limits())
                .unwrap()
                .is_some()
        );
        let other_shard = if shard == "00" { "01" } else { "00" };
        assert!(
            evidence_resource_kind(
                &root
                    .join(format!(
                        "{AUTONOMOUS_LANE_ENTRYPOINT_CLAIMS_DIR_PREFIX}_{other_shard}"
                    ))
                    .join(&name),
                limits()
            )
            .is_err()
        );
        for directory in [
            PENDING_MERGE_ENTRIES_DIR,
            PENDING_QUEUE_PLAN_ADMISSIONS_DIR,
            HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1,
            KAGEMUSHA_MINT_OUTBOX_DIR_NAME,
        ] {
            assert!(
                evidence_resource_kind(
                    &root.join(directory).join(format!("{key}.norito")),
                    limits()
                )
                .unwrap()
                .is_some()
            );
            assert!(
                evidence_resource_kind(
                    &root
                        .join(directory)
                        .join(format!("{}.norito", key.to_uppercase())),
                    limits()
                )
                .is_err()
            );
            assert!(
                evidence_resource_kind(&root.join(directory).join("unowned.norito"), limits())
                    .is_err()
            );
        }
    }

    #[test]
    fn real_native_records_preserve_main_temp_bytes_and_configured_bounds() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let lane = root.join(LANE_ARTIFACTS_DIR_NAME);
        std::fs::create_dir(&lane).unwrap();
        let path = lane.join(Kura::native_amx_evidence_file_name(
            NativeAmxEvidenceKind::Receipt,
            1,
        ));
        let temporary = path.with_extension("norito.tmp");
        std::fs::write(&path, [1_u8; 13]).unwrap();
        std::fs::write(&temporary, [2_u8; 17]).unwrap();
        let (format, is_temporary) = evidence_resource_kind(&path, limits()).unwrap().unwrap();
        let main = index_resource_file_usage(&path, format, is_temporary).unwrap();
        let (format, is_temporary) = evidence_resource_kind(&temporary, limits())
            .unwrap()
            .unwrap();
        let staged = index_resource_file_usage(&temporary, format, is_temporary).unwrap();
        let combined = main.checked_add(staged).unwrap();
        assert_eq!(combined.persisted_entries, 2);
        assert_eq!(combined.index_bytes, 13);
        assert_eq!(combined.temporary_index_bytes, 17);
        assert_eq!(combined.resident_associations, 0);
        assert_eq!(combined.storage_bytes, 0);
        std::fs::write(&temporary, [3_u8; 20]).unwrap();
        assert!(index_resource_file_usage(&temporary, format, is_temporary).is_err());
        std::fs::remove_file(&temporary).unwrap();
        assert_eq!(
            index_resource_file_usage(&temporary, format, is_temporary).unwrap(),
            ResourceUsage::default()
        );
        let prune = lane.join(NATIVE_AMX_EVIDENCE_PRUNE_INTENT_FILE);
        let (format, temporary) = evidence_resource_kind(&prune, limits()).unwrap().unwrap();
        std::fs::write(&prune, [4_u8; 12]).unwrap();
        assert!(index_resource_file_usage(&prune, format, temporary).is_err());
    }

    #[test]
    fn evidence_formats_do_not_double_count_index_pairs_or_invent_fastpq_policy() {
        let root = Path::new("/unused-evidence-inventory");
        let lane = root.join(LANE_ARTIFACTS_DIR_NAME);
        for name in [
            NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_FILE,
            NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE,
            CANONICAL_AUTONOMOUS_LANE_REPLICAS_DATA_FILE,
            AUTONOMOUS_LANE_MERGE_BUNDLES_DATA_FILE,
        ] {
            assert!(
                evidence_resource_kind(&lane.join(name), limits())
                    .unwrap()
                    .is_none()
            );
        }
        for name in [LANE_ARTIFACTS_INDEX_FILE, CERTIFIED_LANE_BLOCKS_INDEX_FILE] {
            let intent = lane.join(format!("{name}.append.intent.tmp"));
            assert!(index_resource_kind(&intent).is_none());
            assert!(
                matches!(evidence_resource_kind(&intent, limits()).unwrap(), Some((IndexResourceFormat::Singleton(maximum), true)) if maximum == BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES as u64)
            );
        }
        assert!(
            evidence_resource_kind(&lane.join("autonomous_attempt_v1_bad.norito"), limits())
                .is_err()
        );
        assert!(
            evidence_resource_kind(&lane.join(".kura-sidecar-unknown-owner"), limits()).is_err()
        );
        assert!(matches!(
            evidence_resource_kind(
                &root
                    .join(fastpq_artifact_store::DIRECTORY)
                    .join("pending.tmp"),
                limits()
            ),
            Ok(Some((IndexResourceFormat::TemporarySingleton(_), true)))
        ));
        assert!(
            evidence_resource_kind(
                &root
                    .join(fastpq_artifact_store::DIRECTORY)
                    .join(COUNT_FILE_NAME),
                limits(),
            )
            .is_err(),
            "a known marker basename cannot escape FASTPQ directory ownership"
        );
        assert!(
            evidence_resource_kind(
                &root
                    .join(fastpq_artifact_store::DIRECTORY)
                    .join("unowned.norito"),
                limits(),
            )
            .is_err()
        );
    }

    #[test]
    fn immutable_native_limits_and_geometry_bounds_come_from_the_actual_owners() {
        let kura = Kura::blank_kura_for_testing();
        let observed = kura.evidence_resource_limits();
        assert_eq!(
            observed.native_record_bytes,
            (kura.pending_control_sidecar_limits.aggregate_bytes as u64)
                .min(STRICT_INIT_MAX_BLOCK_BYTES)
        );
        assert_eq!(
            observed.native_prune_intent_bytes,
            kura.native_amx_evidence_prune_intent_max_bytes() as u64
        );
        for (name, temporary) in [
            ("lane_geometry_journal.norito", false),
            ("lane_geometry_journal.norito.tmp", true),
            ("lane_geometry_journal.norito.restore.tmp", true),
            (".lane-incarnation.norito", false),
            (".lane-incarnation.norito.tmp", true),
        ] {
            let (maximum, actual_temporary) =
                lane_geometry::resource_evidence_file_kind(name).unwrap();
            assert_eq!(actual_temporary, temporary);
            assert!(maximum > 0);
            assert!(
                matches!(evidence_resource_kind(&Path::new("/unused").join(name), observed).unwrap(), Some((IndexResourceFormat::Singleton(bound), observed_temporary)) if bound == maximum && observed_temporary == temporary)
            );
        }
        assert!(
            lane_geometry::resource_evidence_file_kind("lane_geometry_journal.norito.compat")
                .is_none()
        );
        assert!(
            evidence_resource_kind(
                &Path::new("/unused").join("lane_geometry_journal.norito.compat"),
                observed,
            )
            .is_err()
        );
        assert!(
            evidence_resource_kind(
                &Path::new("/unused").join(format!("{PRUNE_INTENT_FILE_NAME}.bad")),
                observed,
            )
            .is_err()
        );
        assert!(matches!(evidence_resource_kind(
            &Path::new("/unused").join(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1)
                .join(format!("{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}bounded-crash")),
            observed,
        ).unwrap(), Some((IndexResourceFormat::Singleton(maximum), true))
            if maximum == HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES as u64));
    }

    #[test]
    fn fastpq_reserved_names_and_empty_temporary_preserve_exact_configured_geometry() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let namespace = root.join(fastpq_artifact_store::DIRECTORY);
        std::fs::create_dir(&namespace).unwrap();
        let mut configured = limits();
        configured.fastpq_artifacts = iroha_config::parameters::actual::KuraFastpqArtifactPolicy {
            max_artifact_bytes: std::num::NonZeroUsize::new(8).unwrap(),
            max_artifacts: std::num::NonZeroUsize::new(1).unwrap(),
            max_total_bytes: std::num::NonZeroU64::new(8).unwrap(),
        };
        let temporary = namespace.join(fastpq_artifact_store::TEMPORARY);
        std::fs::write(&temporary, []).unwrap();
        let (format, is_temporary) = evidence_resource_kind(&temporary, configured)
            .unwrap()
            .unwrap();
        assert!(is_temporary);
        let empty = index_resource_file_usage(&temporary, format, is_temporary).unwrap();
        assert_eq!(empty.persisted_entries, 1);
        assert_eq!(empty.index_bytes, 0);
        assert_eq!(empty.temporary_index_bytes, 0);
        assert!(
            index_resource_file_usage(&temporary, format, false).is_err(),
            "emptyable format is restricted to a real temporary owner"
        );
        std::fs::write(&temporary, [1_u8; 8]).unwrap();
        assert_eq!(
            index_resource_file_usage(&temporary, format, true)
                .unwrap()
                .temporary_index_bytes,
            8
        );
        std::fs::write(&temporary, [1_u8; 9]).unwrap();
        assert!(index_resource_file_usage(&temporary, format, true).is_err());
        let hash = Kura::fixed_bytes_path_component(Hash::new(b"FASTPQ resource file").as_ref());
        let stable = namespace.join(format!("{hash}.norito"));
        std::fs::write(&stable, []).unwrap();
        let (stable_format, is_temporary) = evidence_resource_kind(&stable, configured)
            .unwrap()
            .unwrap();
        assert!(!is_temporary);
        assert!(
            index_resource_file_usage(&stable, stable_format, false).is_err(),
            "a stable artifact cannot be empty"
        );
        for name in [
            COUNT_FILE_NAME.to_owned(),
            format!("{hash}.norito.tmp"),
            format!("{}.norito", hash.to_uppercase()),
            "unowned.norito".to_owned(),
        ] {
            assert!(evidence_resource_kind(&namespace.join(name), configured).is_err());
        }
    }
}
