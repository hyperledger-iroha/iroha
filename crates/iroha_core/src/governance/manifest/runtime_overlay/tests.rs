//! Deterministic additive manifest derivation and immutable baseline regression tests.

use super::*;
use iroha_data_model::nexus::{DataSpaceMetadata, LaneLifecyclePlan};
use nonzero_ext::nonzero;

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

fn native_runtime_manifest() -> RuntimeLaneManifestV1 {
    let validators: Vec<_> = (1_u8..=4)
        .map(|seed| {
            let key = iroha_crypto::KeyPair::try_from_seed(
                vec![seed; 32],
                iroha_crypto::Algorithm::Ed25519,
            )
            .expect("deterministic native validator key");
            let validator = AccountId::new(key.public_key().clone()).to_string();
            let peer_id = PeerId::new(key.public_key().clone()).to_string();
            norito::json!({ "validator": validator, "peer_id": peer_id })
        })
        .collect();
    RuntimeLaneManifestV1 {
        lane_id: LaneId::new(5),
        manifest: iroha_primitives::json::Json::new(norito::json!({
            "lane": "bpng", "version": 1, "validators": validators, "quorum": 3,
        })),
    }
}

#[test]
fn runtime_manifest_enclosing_limits_preserve_original_cause_and_same_input_retry() {
    use norito::core::{DecodeAttemptErrorKind, DecodeLocalRefusalKind, DecodeResourceError};

    let (baseline, catalog, dataspaces, governance) = fixture();
    let addition = native_runtime_manifest();
    let lane = catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == addition.lane_id)
        .unwrap();
    let dataspace = dataspaces.by_id(lane.dataspace_id).unwrap();
    let source = addition.manifest.get().to_owned();
    let source_pointer = addition.manifest.get().as_ptr();
    let baseline_digest = baseline.consensus_policy_digest();
    let expected = baseline
        .with_runtime_additions(
            std::slice::from_ref(&addition),
            &catalog,
            &dataspaces,
            &governance,
        )
        .expect("native runtime addition succeeds before limiting local resources");
    LaneManifestRegistry::validate_runtime_manifest(&addition, lane, dataspace, &governance)
        .expect("native standalone validation succeeds");

    for install in [false, true] {
        for (limits, work) in [
            (
                norito::DecodeLimits::new(usize::MAX, usize::MAX, 0, usize::MAX, usize::MAX),
                true,
            ),
            (
                norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
                false,
            ),
        ] {
            // The fourth constructor argument owns allocated bytes; depth is fifth.
            assert_eq!(
                limits.max_total_allocated_bytes(),
                if work { usize::MAX } else { 0 }
            );
            assert_eq!(limits.max_nesting_depth(), usize::MAX);
            let mut original_identity = None;
            let result = norito::with_decode_limits_scope(limits, || {
                norito::core::classify_decode_attempt(|| {
                    let error = if install {
                        baseline
                            .with_runtime_additions(
                                std::slice::from_ref(&addition),
                                &catalog,
                                &dataspaces,
                                &governance,
                            )
                            .map(|_| ())
                    } else {
                        LaneManifestRegistry::validate_runtime_manifest(
                            &addition,
                            lane,
                            dataspace,
                            &governance,
                        )
                    }
                    .expect_err("the actual native parser must refuse its enclosing scope");
                    let LaneManifestError::Json(json::Error::ScopedDecodeResource(original)) =
                        &error
                    else {
                        panic!("keep the original JSON resource cause instead of a diagnostic");
                    };
                    original_identity = Some(original.clone());
                    Err::<(), _>(error.into_decode_error().expect("move original JSON cause"))
                })
            });
            let error = result.expect_err("original actual-operation resource refusal");
            assert_eq!(error.kind(), DecodeAttemptErrorKind::EnclosingLimit);
            let norito::Error::ScopedDecodeResource(actual) = error.error() else {
                panic!("retain the original opaque scope identity");
            };
            assert_eq!(Some(actual), original_identity.as_ref());
            let retained = error.local_refusal().expect("original local refusal owner");
            assert_eq!(retained.kind(), DecodeLocalRefusalKind::EnclosingLimit);
            if work {
                assert!(matches!(
                    retained.resource_error(),
                    DecodeResourceError::TotalElementsExceeded { limit: 0, .. }
                ));
            } else {
                assert!(matches!(
                    retained.resource_error(),
                    DecodeResourceError::TotalAllocationExceeded { limit: 0, .. }
                ));
            }
            assert_eq!(addition.manifest.get(), &source);
            assert_eq!(addition.manifest.get().as_ptr(), source_pointer);
            assert_eq!(baseline.consensus_policy_digest(), baseline_digest);
            assert!(!baseline.has_manifest(addition.lane_id));
            let retry = baseline
                .with_runtime_additions(
                    std::slice::from_ref(&addition),
                    &catalog,
                    &dataspaces,
                    &governance,
                )
                .expect("retry exactly the same source after the enclosing limit retires");
            assert!(retry.has_same_authority_as(&expected));
            assert_eq!(
                retry.consensus_policy_digest(),
                expected.consensus_policy_digest()
            );
            LaneManifestRegistry::validate_runtime_manifest(
                &addition,
                lane,
                dataspace,
                &governance,
            )
            .expect("standalone retry uses the same native source");
        }
    }
}

#[test]
fn runtime_manifest_malformed_and_protocol_bounds_remain_invalid() {
    let (baseline, catalog, dataspaces, governance) = fixture();
    let valid = native_runtime_manifest();
    let lane = catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == valid.lane_id)
        .unwrap();
    let dataspace = dataspaces.by_id(lane.dataspace_id).unwrap();
    let malformed = norito::core::classify_decode_attempt(|| {
        let error = LaneManifestRegistry::parse_bounded_manifest_json(
            br#"{"lane":}"#,
            &mut ManifestSourceLoadBudget::default(),
        )
        .expect_err("real malformed source");
        assert!(matches!(&error, LaneManifestError::Json(_)));
        Ok::<_, norito::Error>(
            error
                .into_decode_error()
                .expect_err("syntax is not admission"),
        )
    })
    .expect("no local refusal was created for malformed syntax");
    assert!(matches!(malformed, LaneManifestError::Json(_)));
    let preflight = LaneManifestRegistry::parse_bounded_manifest_json(
        b"{",
        &mut ManifestSourceLoadBudget::default(),
    )
    .expect_err("existing lexical preflight rejects an unterminated container");
    assert!(matches!(
        preflight.into_decode_error(),
        Err(LaneManifestError::Invalid(_))
    ));

    let oversized_alias = "x".repeat(LANE_MANIFEST_MAX_BYTES_V1);
    let oversized = RuntimeLaneManifestV1 {
        lane_id: valid.lane_id,
        manifest: iroha_primitives::json::Json::new(norito::json!({
            "lane": oversized_alias,
        })),
    };
    let protocol = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, 0, 0, usize::MAX),
        || {
            norito::core::classify_decode_attempt(|| {
                let error = LaneManifestRegistry::validate_runtime_manifest(
                    &oversized,
                    lane,
                    dataspace,
                    &governance,
                )
                .expect_err("the fixed raw-source cap rejects before decoding");
                Ok::<_, norito::Error>(
                    error
                        .into_decode_error()
                        .expect_err("fixed bounds are invalid input"),
                )
            })
        },
    )
    .expect("a protocol bound cannot manufacture local resource ownership");
    assert!(matches!(protocol, LaneManifestError::Invalid(_)));

    let wrong_alias = RuntimeLaneManifestV1 {
        lane_id: valid.lane_id,
        manifest: iroha_primitives::json::Json::new(norito::json!({"lane": "other"})),
    };
    let invalid = baseline
        .with_runtime_additions(
            std::slice::from_ref(&wrong_alias),
            &catalog,
            &dataspaces,
            &governance,
        )
        .expect_err("semantic alias mismatch stays invalid");
    let mut moved = None;
    assert_eq!(
        crate::test_allocations::allocations_during(|| {
            moved = Some(invalid.into_decode_error());
        }),
        0,
        "rejected error transfer must not format or allocate"
    );
    assert!(matches!(moved.unwrap(), Err(LaneManifestError::Invalid(_))));
    baseline
        .with_runtime_additions(
            std::slice::from_ref(&valid),
            &catalog,
            &dataspaces,
            &governance,
        )
        .expect("invalid attempts do not change the immutable baseline");
}

#[test]
fn runtime_manifest_sibling_and_stale_causes_cannot_borrow_fresh_refusal_authority() {
    use norito::core::DecodeAttemptErrorKind;

    let (_, catalog, dataspaces, governance) = fixture();
    let addition = native_runtime_manifest();
    let lane = catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == addition.lane_id)
        .unwrap();
    let dataspace = dataspaces.by_id(lane.dataspace_id).unwrap();
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
    assert_eq!(limits.max_total_allocated_bytes(), 0);
    assert_eq!(limits.max_nesting_depth(), usize::MAX);
    let original = || {
        norito::core::classify_decode_attempt(|| {
            let error = LaneManifestRegistry::validate_runtime_manifest(
                &addition,
                lane,
                dataspace,
                &governance,
            )
            .expect_err("actual native parser refusal");
            Err::<(), _>(error.into_decode_error().expect("own original JSON cause"))
        })
        .expect_err("actual observer captures the original cause")
    };
    let reject_foreign = |retained: LaneManifestError| {
        let rejected = norito::core::classify_decode_attempt(|| {
            let fresh = LaneManifestRegistry::validate_runtime_manifest(
                &addition,
                lane,
                dataspace,
                &governance,
            )
            .expect_err("fresh actual parser refusal in this observer");
            assert!(fresh.into_decode_error().is_ok());
            Err::<(), _>(
                retained
                    .into_decode_error()
                    .expect("move prior original cause"),
            )
        })
        .expect_err("a sibling or retired cause is not this attempt's local refusal");
        assert_eq!(rejected.kind(), DecodeAttemptErrorKind::Invalid);
        assert!(rejected.local_refusal().is_none());
    };
    norito::with_decode_limits_scope(limits, || {
        let sibling = original();
        assert_eq!(sibling.kind(), DecodeAttemptErrorKind::EnclosingLimit);
        reject_foreign(LaneManifestError::Codec(sibling.into_error()));
    });
    let stale = norito::with_decode_limits_scope(limits, original);
    assert_eq!(stale.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    norito::with_decode_limits_scope(limits, || {
        reject_foreign(LaneManifestError::Codec(stale.into_error()));
    });
    LaneManifestRegistry::validate_runtime_manifest(&addition, lane, dataspace, &governance)
        .expect("same source validates after resource scopes retire");
}
