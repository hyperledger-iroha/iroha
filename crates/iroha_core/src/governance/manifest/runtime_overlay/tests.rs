//! Deterministic additive manifest derivation and immutable baseline regression tests.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::nexus::{DataSpaceMetadata, LaneLifecyclePlan};
use iroha_primitives::json::Json;
use nonzero_ext::nonzero;

fn manifest(alias: &str, count: u8, quorum: u32) -> ManifestFile {
    ManifestFile {
        lane: Some(alias.to_owned()),
        version: Some(1),
        validators: Some(
            (1..=count)
                .map(|seed| {
                    let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic public manifest fixture");
                    ManifestValidatorBindingFile {
                        validator: Some(AccountId::new(key.public_key().clone()).to_string()),
                        peer_id: Some(PeerId::new(key.public_key().clone()).to_string()),
                        torii_url: None,
                    }
                })
                .collect(),
        ),
        quorum: Some(quorum),
        ..ManifestFile::default()
    }
}

fn addition(lane_id: u32, alias: &str) -> RuntimeLaneManifestV1 {
    RuntimeLaneManifestV1 {
        lane_id: LaneId::new(lane_id),
        manifest: Json::new(manifest(alias, 4, 3)),
    }
}

fn fixture() -> (
    LaneManifestRegistry,
    LaneCatalog,
    DataSpaceCatalog,
    GovernanceCatalog,
) {
    let baseline_catalog = LaneCatalog::new(
        nonzero!(2_u32),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: LaneId::new(1),
                alias: "existing".to_owned(),
                ..LaneConfig::default()
            },
        ],
    )
    .expect("baseline catalog");
    let governance = GovernanceCatalog::default();
    let baseline =
        LaneManifestRegistry::from_config(&baseline_catalog, &governance, &LaneRegistry::default());
    let effective = baseline_catalog
        .apply_lifecycle(&LaneLifecyclePlan {
            additions: vec![LaneConfig {
                id: LaneId::new(5),
                alias: "bpng".to_owned(),
                dataspace_id: DataSpaceId::new(42),
                ..LaneConfig::default()
            }],
            retire: vec![],
        })
        .expect("additive lane catalog");
    let dataspaces = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: DataSpaceId::new(42),
            alias: "bpng".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .expect("physical dataspace catalog");
    (baseline, effective, dataspaces, governance)
}

#[test]
fn runtime_manifest_overlay_preserves_baseline_and_rebuilds_cumulatively() {
    let (baseline, catalog, dataspaces, governance) = fixture();
    let original_digest = baseline.consensus_policy_digest();
    let additions = vec![addition(5, "bpng")];
    let effective = baseline
        .with_runtime_additions(&additions, &catalog, &dataspaces, &governance)
        .expect("authenticated runtime source");
    assert_eq!(baseline.consensus_policy_digest(), original_digest);
    assert_eq!(
        effective.baseline_consensus_policy_digest(),
        original_digest
    );
    assert_ne!(effective.consensus_policy_digest(), original_digest);
    assert!(effective.has_manifest(LaneId::new(5)));
    assert!(
        effective
            .status(LaneId::new(5))
            .expect("runtime status")
            .manifest_path
            .is_none()
    );
    effective
        .ensure_lane_ready(LaneId::new(5))
        .expect("runtime source needs no fake path");
    assert!(!baseline.has_manifest(LaneId::new(5)));
    let repeated = effective
        .with_runtime_additions(&additions, &catalog, &dataspaces, &governance)
        .expect("full cumulative reconstruction is idempotent");
    assert_eq!(
        repeated.consensus_policy_digest(),
        effective.consensus_policy_digest()
    );
    assert_eq!(repeated.baseline_consensus_policy_digest(), original_digest);
    let rebound = repeated.rebind(&catalog, &governance);
    assert_eq!(rebound.baseline_consensus_policy_digest(), original_digest);
    assert!(Arc::ptr_eq(
        rebound
            .baseline_source_snapshot
            .as_ref()
            .expect("original baseline retained"),
        baseline.source_snapshot.as_ref().expect("startup source"),
    ));
    let expanded_catalog = catalog
        .apply_lifecycle(&LaneLifecyclePlan {
            additions: vec![LaneConfig {
                id: LaneId::new(6),
                alias: "bpng-extra".to_owned(),
                dataspace_id: DataSpaceId::new(42),
                ..LaneConfig::default()
            }],
            retire: vec![],
        })
        .expect("second additive lane");
    let cumulative = rebound
        .with_runtime_additions(
            &[addition(5, "bpng"), addition(6, "bpng-extra")],
            &expanded_catalog,
            &dataspaces,
            &governance,
        )
        .expect("rebuild both runtime additions from original source");
    assert!(cumulative.has_manifest(LaneId::new(5)) && cumulative.has_manifest(LaneId::new(6)));
    assert_eq!(
        cumulative.baseline_consensus_policy_digest(),
        original_digest
    );
}

#[test]
fn runtime_manifest_overlay_rejects_takeover_duplicates_and_schema_drift_atomically() {
    let (baseline, catalog, dataspaces, governance) = fixture();
    let digest = baseline.consensus_policy_digest();
    let mut duplicate_account = manifest("bpng", 4, 3);
    let rows = duplicate_account.validators.as_mut().expect("bindings");
    rows[1].validator = rows[0].validator.clone();
    let mut duplicate_peer = manifest("bpng", 4, 3);
    let rows = duplicate_peer.validators.as_mut().expect("bindings");
    rows[1].peer_id = rows[0].peer_id.clone();
    let mut unnamed = manifest("bpng", 4, 3);
    unnamed.lane = None;
    let invalid = [
        vec![addition(1, "existing")],
        vec![addition(5, "bpng"), addition(5, "bpng")],
        vec![addition(6, "absent")],
        vec![addition(5, "wrong-alias")],
        vec![RuntimeLaneManifestV1 {
            lane_id: LaneId::new(5),
            manifest: Json::new(duplicate_account),
        }],
        vec![RuntimeLaneManifestV1 {
            lane_id: LaneId::new(5),
            manifest: Json::new(duplicate_peer),
        }],
        vec![RuntimeLaneManifestV1 {
            lane_id: LaneId::new(5),
            manifest: Json::new(unnamed),
        }],
        vec![RuntimeLaneManifestV1 {
            lane_id: LaneId::new(5),
            manifest: Json::new(manifest("bpng", 3, 3)),
        }],
        vec![RuntimeLaneManifestV1 {
            lane_id: LaneId::new(5),
            manifest: Json::new(manifest("bpng", 4, 2)),
        }],
        vec![RuntimeLaneManifestV1 {
            lane_id: LaneId::new(5),
            manifest: Json::from(norito::json!({"lane":"bpng","unexpected":true})),
        }],
    ];
    for entries in invalid {
        assert!(
            baseline
                .with_runtime_additions(&entries, &catalog, &dataspaces, &governance)
                .is_err()
        );
        assert_eq!(baseline.consensus_policy_digest(), digest);
        assert!(!baseline.has_manifest(LaneId::new(5)));
    }
    assert!(
        baseline
            .with_runtime_additions(
                &[addition(5, "bpng")],
                &catalog,
                &DataSpaceCatalog::default(),
                &governance
            )
            .is_err()
    );
}

#[test]
fn runtime_manifest_overlay_rejects_private_torii_urls_before_publication() {
    let (baseline, catalog, dataspaces, governance) = fixture();
    let original_digest = baseline.consensus_policy_digest();
    for url in [
        "https://operator:REJECTED_INPUT_MARKER@validator.example",
        "https://REJECTED_INPUT_MARKER@validator.example",
        "https://validator.example?access_token=REJECTED_INPUT_MARKER",
        "https://validator.example#REJECTED_INPUT_MARKER",
        "not-a-url-REJECTED_INPUT_MARKER",
        "https://validator.example/torii",
        "http://127.0.0.1:8080",
    ] {
        let mut parsed = manifest("bpng", 4, 3);
        parsed.validators.as_mut().expect("bindings")[0].torii_url = Some(url.to_owned());
        let additions = [RuntimeLaneManifestV1 {
            lane_id: LaneId::new(5),
            manifest: Json::new(parsed),
        }];
        let result =
            baseline.with_runtime_additions(&additions, &catalog, &dataspaces, &governance);
        if url.contains("REJECTED_INPUT_MARKER") {
            let error = result.expect_err("private URL cannot enter the committed registry");
            assert!(error.contains("torii_url"));
            assert!(!error.contains("REJECTED_INPUT_MARKER"));
            assert!(!error.contains(url));
        } else {
            let registry = result.expect("public Torii base URL is accepted");
            let rules = registry.lane_rules(LaneId::new(5)).expect("runtime rules");
            assert_eq!(rules.validator_bindings[0].torii_url.as_deref(), Some(url));
            assert!(registry.has_manifest(LaneId::new(5)));
        }
        assert_eq!(baseline.consensus_policy_digest(), original_digest);
        assert!(!baseline.has_manifest(LaneId::new(5)));
    }
}

#[test]
fn runtime_manifest_overlay_quorum_tracks_dataspace_fault_tolerance() {
    let (baseline, catalog, _, governance) = fixture();
    let dataspaces = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: DataSpaceId::new(42),
            alias: "bpng".to_owned(),
            description: None,
            fault_tolerance: 2,
        },
    ])
    .expect("f=2 dataspace");
    assert!(
        baseline
            .with_runtime_additions(&[addition(5, "bpng")], &catalog, &dataspaces, &governance)
            .is_err()
    );
    let entries = [RuntimeLaneManifestV1 {
        lane_id: LaneId::new(5),
        manifest: Json::new(manifest("bpng", 7, 5)),
    }];
    let registry = baseline
        .with_runtime_additions(&entries, &catalog, &dataspaces, &governance)
        .expect("exact f=2 seven-member committee");
    assert_eq!(
        registry
            .lane_validators(LaneId::new(5))
            .expect("committee")
            .len(),
        7
    );
}

#[test]
fn runtime_manifest_overlay_never_loads_deferred_paths_and_preserves_manual_rebind() {
    let (baseline, catalog, dataspaces, governance) = fixture();
    let deferred = LaneManifestRegistry {
        source_snapshot: Some(Arc::new(LaneManifestSourceSnapshot::load(
            &LaneRegistry::default(),
        ))),
        ..LaneManifestRegistry::default()
    };
    assert!(
        deferred
            .with_runtime_additions(&[addition(5, "bpng")], &catalog, &dataspaces, &governance)
            .is_err()
    );
    let after_retirement = catalog
        .apply_lifecycle(&LaneLifecyclePlan {
            additions: vec![],
            retire: vec![LaneId::new(1)],
        })
        .expect("ordinary authorized manual retirement");
    let rebound = baseline
        .with_runtime_additions(&[], &after_retirement, &dataspaces, &governance)
        .expect("no runtime overlay does not reject manual lifecycle changes");
    assert!(rebound.status(LaneId::new(1)).is_none());
    assert_eq!(
        rebound.consensus_policy_digest(),
        baseline.consensus_policy_digest()
    );
    let with_overlay = rebound
        .with_runtime_additions(
            &[addition(5, "bpng")],
            &after_retirement,
            &dataspaces,
            &governance,
        )
        .expect("runtime source preserves already-authorized baseline retirement");
    assert!(with_overlay.status(LaneId::new(1)).is_none());

    let baseline_catalog = LaneCatalog::new(nonzero!(1_u32), vec![LaneConfig::default()])
        .expect("empty-source startup catalog");
    let initially_empty = LaneManifestRegistry::empty().rebind(&baseline_catalog, &governance);
    let from_empty = initially_empty
        .with_runtime_additions(&[addition(5, "bpng")], &catalog, &dataspaces, &governance)
        .expect("first empty-source bind preserves initial catalog authority");
    assert!(from_empty.has_manifest(LaneId::new(5)));
}

#[test]
fn manifest_catalog_binding_rejects_stale_refresh_after_source_preserving_lifecycle() {
    let (baseline, catalog, _, governance) = fixture();
    let candidate = baseline.rebind(&catalog, &governance);
    let retired = catalog
        .apply_lifecycle(&LaneLifecyclePlan {
            additions: vec![],
            retire: vec![LaneId::new(1)],
        })
        .expect("ordinary manual lifecycle");
    let current = candidate.rebind(&retired, &governance);
    assert_eq!(
        candidate.consensus_policy_digest(),
        current.consensus_policy_digest()
    );
    assert_eq!(
        candidate.baseline_consensus_policy_digest(),
        current.baseline_consensus_policy_digest()
    );
    assert!(candidate.is_bound_to_catalog(&catalog));
    assert!(!candidate.is_bound_to_catalog(&retired));
    assert!(current.is_bound_to_catalog(&retired));
    assert!(!LaneManifestRegistry::empty().is_bound_to_catalog(&catalog));
    let status_only = LaneManifestRegistry::from_statuses(candidate.statuses.clone());
    assert!(!status_only.is_bound_to_catalog(&catalog));
    assert!(
        !status_only
            .rebind(&catalog, &governance)
            .is_bound_to_catalog(&catalog)
    );
}

#[test]
fn runtime_manifest_overlay_retains_frozen_file_source_and_rejects_alias_takeover() {
    let (_, catalog, dataspaces, governance) = fixture();
    let baseline_catalog = catalog
        .apply_lifecycle(&LaneLifecyclePlan {
            additions: vec![],
            retire: vec![LaneId::new(5)],
        })
        .expect("startup lanes");
    let directory = tempfile::tempdir().expect("public manifest fixture directory");
    let path = directory.path().join("existing.manifest.json");
    fs::write(&path, json::to_json(&manifest("existing", 4, 3)).unwrap())
        .expect("public baseline manifest fixture");
    let registry_config = LaneRegistry {
        manifest_directory: Some(directory.path().to_path_buf()),
        ..LaneRegistry::default()
    };
    let baseline =
        LaneManifestRegistry::from_config(&baseline_catalog, &governance, &registry_config);
    let baseline_digest = baseline.consensus_policy_digest();
    let baseline_rules = baseline.lane_rules(LaneId::new(1)).unwrap().clone();
    fs::remove_file(&path).expect("remove the external source after startup capture");
    let effective = baseline
        .with_runtime_additions(&[addition(5, "bpng")], &catalog, &dataspaces, &governance)
        .expect("derivation must use retained parsed source without reopening path");
    assert_eq!(
        effective.baseline_consensus_policy_digest(),
        baseline_digest
    );
    assert_eq!(effective.lane_rules(LaneId::new(1)), Some(&baseline_rules));
    assert!(effective.has_manifest(LaneId::new(1)));
    assert_eq!(
        effective
            .status(LaneId::new(1))
            .unwrap()
            .manifest_path
            .as_ref(),
        Some(&path)
    );

    let reused_alias = catalog
        .apply_lifecycle(&LaneLifecyclePlan {
            additions: vec![LaneConfig {
                id: LaneId::new(6),
                alias: "existing".to_owned(),
                dataspace_id: DataSpaceId::new(42),
                ..LaneConfig::default()
            }],
            retire: vec![LaneId::new(1)],
        })
        .expect("manual lifecycle can move an alias");
    assert!(
        baseline
            .with_runtime_additions(
                &[addition(5, "bpng"), addition(6, "existing")],
                &reused_alias,
                &dataspaces,
                &governance,
            )
            .is_err(),
        "runtime additions cannot replace a frozen source after its old lane retires"
    );
}

#[test]
fn runtime_manifest_overlay_governed_lane_is_ready_without_a_filesystem_path() {
    let (baseline, catalog, dataspaces, mut governance) = fixture();
    governance.modules.insert(
        "council".to_owned(),
        ConfigGovernanceModule {
            module_type: Some("parliament".to_owned()),
            params: BTreeMap::new(),
        },
    );
    let mut lanes = catalog.lanes().to_vec();
    lanes
        .iter_mut()
        .find(|lane| lane.id == LaneId::new(5))
        .unwrap()
        .governance = Some("council".to_owned());
    let catalog = LaneCatalog::new(catalog.lane_count(), lanes).expect("governed runtime lane");
    let mut public_manifest = manifest("bpng", 4, 3);
    public_manifest.governance = Some("council".to_owned());
    let registry = baseline
        .with_runtime_additions(
            &[RuntimeLaneManifestV1 {
                lane_id: LaneId::new(5),
                manifest: Json::new(public_manifest),
            }],
            &catalog,
            &dataspaces,
            &governance,
        )
        .expect("native governed runtime source");
    registry.ensure_lane_ready(LaneId::new(5)).expect("ready");
    assert!(registry.has_manifest(LaneId::new(5)));
    assert!(
        registry
            .status(LaneId::new(5))
            .unwrap()
            .manifest_path
            .is_none()
    );

    #[cfg(feature = "telemetry")]
    {
        use crate::telemetry::{LaneTeuGaugeUpdate, StateTelemetry};
        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        let telemetry = StateTelemetry::new(Arc::clone(&metrics), true);
        telemetry.set_nexus_catalogs(&catalog, &dataspaces);
        telemetry.set_lane_manifest_registry(Arc::new(registry));
        telemetry.record_nexus_scheduler_lane_teu(LaneId::new(5), LaneTeuGaugeUpdate::default());
        assert_eq!(
            metrics
                .nexus_lane_governance_sealed
                .with_label_values(&["bpng"])
                .get(),
            0
        );
        let statuses = metrics.nexus_scheduler_lane_teu_status.read().unwrap();
        let status = statuses.get(&5).expect("runtime lane telemetry");
        assert!(status.manifest_required && status.manifest_ready);
        assert!(status.manifest_path.is_none());
    }
}
