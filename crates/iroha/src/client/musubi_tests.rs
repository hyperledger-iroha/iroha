use super::public_musubi::MUSUBI_PUBLIC_QUERY_MAX_RESPONSE_BYTES;
use iroha_data_model::{
    isi::musubi::PublishMusubiReleaseV1,
    musubi::{
        ArchiveId, MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1, MUSUBI_MAX_DEPENDENCIES_V1,
        MUSUBI_MAX_EXPORTS_V1, MUSUBI_MAX_KEYWORDS_V1, MUSUBI_MAX_PRERELEASE_IDENTIFIER_BYTES_V1,
        MUSUBI_MAX_PRERELEASE_IDENTIFIERS_V1, MUSUBI_MAX_VERSION_COMPARATORS_V1,
        MUSUBI_REGISTRY_VERSION_V1, MUSUBI_RESOLVER_PAGE_JSON_ITEMS_BUDGET_BYTES_V1,
        MusubiAbiBindingV1, MusubiArchiveAvailabilityV1, MusubiArtifactGovernanceStateV1,
        MusubiArtifactTakedownV1, MusubiComparatorOpV1, MusubiContentDigestV1,
        MusubiDependencyKindV1, MusubiDependencyReqV1, MusubiDescriptionV1, MusubiDocumentRefV1,
        MusubiExactDependencyEdgeV1, MusubiExactReleaseSnapshotV1, MusubiGovernanceActionDigestV1,
        MusubiKeywordV1, MusubiKotodamaEditionV1, MusubiNamespaceV1, MusubiPackageIdV1,
        MusubiPackageScopeV1, MusubiPrereleaseIdentifierV1, MusubiPublicationV1, MusubiReasonV1,
        MusubiRegistrySnapshotV1, MusubiReleaseIdV1, MusubiReleaseManifestV1,
        MusubiReleaseMetadataV1, MusubiReleaseRecordV1, MusubiReleaseRevisionsV1,
        MusubiReleaseSelectionStateV1, MusubiReleaseYankV1, MusubiResolutionProofV1,
        MusubiResolverReleaseRowV1, MusubiStorageAvailabilityV1, MusubiVerificationLockV1,
        MusubiVerificationNodeV1, MusubiVersionComparatorV1, MusubiVersionReqV1, MusubiVersionV1,
        validate_musubi_account_id_v1,
    },
};
#[test]
fn provider_bundle_attestation_uses_dedicated_public_musubi_route() {
    assert_eq!(
        PublicMusubiQueryPathV1::ProviderBundleAttestation.path(),
        "/v1/musubi/queries/provider-bundle-attestation"
    );
}
#[test]
fn public_musubi_query_signs_the_exact_fixed_route_and_body() {
    let response = json_response(StatusCode::OK, r#"{"result":"finalized"}"#);
    let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let query = norito::json!({"package": "apps.sora/demo"});
    let client = client_with_base_url(base_url());
    let result: PublicMusubiQueryResultV1<Value> =
        with_mock_http(respond_with(&snapshots, response), || {
            post_public_musubi_query_v1(
                &client,
                PublicMusubiQueryPathV1::ExactPackage,
                &query,
                Duration::from_secs(1),
            )
        })
        .expect("public Musubi query");
    assert!(matches!(result, PublicMusubiQueryResultV1::Found(_)));
    let snapshot = snapshots.lock().expect("snapshot lock")[0].clone();
    assert_eq!(snapshot.method, HttpMethod::POST);
    assert_eq!(snapshot.url.path(), "/v1/musubi/queries/exact-package");
    assert_eq!(
        snapshot.max_response_bytes,
        MUSUBI_PUBLIC_QUERY_MAX_RESPONSE_BYTES
    );
    assert_canonical_account_signed_json_request(&client, &snapshot);
}
#[test]
fn public_musubi_query_rejects_legacy_witness_injection_before_dispatch() {
    let mut client = client_with_base_url(base_url());
    client
        .headers
        .insert("x-IROHA-witness".to_owned(), "legacy-witness".to_owned());
    let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let stored = Arc::clone(&snapshots);
    let error = with_mock_http(
        move |snapshot| {
            stored.lock().expect("snapshot lock").push(snapshot);
            Ok(empty_response(StatusCode::OK))
        },
        || {
            post_public_musubi_query_v1::<_, Value>(
                &client,
                PublicMusubiQueryPathV1::ExactPackage,
                &norito::json!({"package": "apps.sora/demo"}),
                Duration::from_secs(1),
            )
        },
    )
    .expect_err("legacy witness headers must fail before dispatch");
    assert!(error.to_string().contains("authenticated Musubi client"));
    assert!(snapshots.lock().expect("snapshot lock").is_empty());
}
#[test]
#[allow(clippy::too_many_lines)]
fn transaction_boundary_exact_release_json_fits_the_musubi_query_cap() {
    const RETIRED_QUERY_CAP_BYTES: usize = 8 * 1024 * 1024;
    fn padded_name(prefix: &str) -> Name {
        assert!(prefix.len() <= MAX_NAME_BYTES);
        format!("{prefix}{}", "\\".repeat(MAX_NAME_BYTES - prefix.len()))
            .parse()
            .expect("maximal fixture name")
    }
    fn near_limit_account() -> (AccountId, KeyPair) {
        let members = (0_u16..256)
            .map(|index| {
                let mut seed = [0xC7; 32];
                seed[..2].copy_from_slice(&index.to_le_bytes());
                let keypair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519)
                    .expect("near-limit account keypair");
                MultisigMember::new(keypair.public_key().clone(), 1)
                    .expect("near-limit account member")
            })
            .collect::<Vec<_>>();
        for count in (1..=members.len()).rev() {
            let account = AccountId::new_multisig(
                MultisigPolicy::new(1, members[..count].to_vec())
                    .expect("near-limit account policy"),
            );
            let encoded =
                norito::to_bytes(&account).expect("near-limit account canonical encoding");
            if encoded.len() <= MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1 {
                assert!(
                    encoded.len() > MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1 - 256,
                    "fixture must exercise the account bound"
                );
                validate_musubi_account_id_v1(&account)
                    .expect("near-limit account is legal in Musubi");
                let mut signer_seed = [0xC7; 32];
                signer_seed[..2].copy_from_slice(&0_u16.to_le_bytes());
                let signer = KeyPair::try_from_seed(signer_seed.to_vec(), Algorithm::Ed25519)
                    .expect("near-limit account signer");
                return (account, signer);
            }
        }
        panic!("at least one multisig member must fit the Musubi account bound");
    }
    let maximal_prerelease = (0..MUSUBI_MAX_PRERELEASE_IDENTIFIERS_V1)
        .map(|_| {
            MusubiPrereleaseIdentifierV1::AlphaNumeric(
                "z".repeat(MUSUBI_MAX_PRERELEASE_IDENTIFIER_BYTES_V1),
            )
        })
        .collect::<Vec<_>>();
    let comparator_major_floor = u64::MAX
        - u64::try_from(MUSUBI_MAX_VERSION_COMPARATORS_V1).expect("comparator maximum fits u64");
    let requirement = MusubiVersionReqV1::Comparators(
        (0..MUSUBI_MAX_VERSION_COMPARATORS_V1)
            .map(|index| MusubiVersionComparatorV1 {
                op: MusubiComparatorOpV1::GreaterOrEqual,
                version: MusubiVersionV1::new(
                    comparator_major_floor
                        + u64::try_from(index).expect("comparator index fits u64"),
                    u64::MAX,
                    u64::MAX,
                    maximal_prerelease.clone(),
                )
                .expect("maximal comparator version"),
            })
            .collect(),
    );
    requirement.validate().expect("maximal comparator AST");
    let dependency_domain: Name = "\\"
        .repeat(MAX_NAME_BYTES)
        .parse()
        .expect("maximal dependency domain");
    let selected_version = MusubiVersionV1::new(u64::MAX, u64::MAX, u64::MAX, Vec::new())
        .expect("stable selected version");
    let mut dependencies = Vec::with_capacity(MUSUBI_MAX_DEPENDENCIES_V1);
    let mut root_dependencies = Vec::with_capacity(MUSUBI_MAX_DEPENDENCIES_V1);
    let mut nodes = Vec::with_capacity(MUSUBI_MAX_DEPENDENCIES_V1);
    for index in 0..MUSUBI_MAX_DEPENDENCIES_V1 {
        let alias = padded_name(&format!("dep-{index:03}-"));
        let package = MusubiPackageIdV1::new(
            DataSpaceId::new(
                u64::MAX
                    - u64::try_from(MUSUBI_MAX_DEPENDENCIES_V1 - index)
                        .expect("dependency index fits u64"),
            ),
            MusubiPackageScopeV1::Domain(dependency_domain.clone()),
            "p".repeat(64).parse().expect("maximal package name"),
        );
        let selected = MusubiReleaseIdV1::new(package.clone(), selected_version.clone());
        dependencies.push(MusubiDependencyReqV1 {
            alias: alias.clone(),
            package: package.clone(),
            requirement: requirement.clone(),
        });
        root_dependencies.push(MusubiExactDependencyEdgeV1 {
            alias,
            kind: MusubiDependencyKindV1::Normal,
            package,
            requirement: requirement.clone(),
            selected: selected.clone(),
        });
        nodes.push(MusubiVerificationNodeV1 {
            release: selected,
            release_digest: iroha_data_model::musubi::MusubiReleaseDigestV1::new([0xFF; 32]),
            archive_id: ArchiveId::new([0xFF; 32]),
            source_digest: MusubiContentDigestV1::new([0xFF; 32]),
            interface_digest: MusubiContentDigestV1::new([0xFF; 32]),
            abi: MusubiAbiBindingV1::new([0xFF; 32]).expect("node ABI"),
            dependencies: Vec::new(),
        });
    }
    let exports = (0..MUSUBI_MAX_EXPORTS_V1)
        .map(|index| padded_name(&format!("export-{index:04}-")))
        .collect::<Vec<_>>();
    let keywords = (0..MUSUBI_MAX_KEYWORDS_V1)
        .map(|index| {
            format!("k{index:02}-{}", "k".repeat(60))
                .parse::<MusubiKeywordV1>()
                .expect("maximal keyword")
        })
        .collect::<Vec<_>>();
    let metadata = MusubiReleaseMetadataV1 {
        description: Some(
            MusubiDescriptionV1::new(&"\\".repeat(4_096)).expect("maximal description"),
        ),
        readme: Some(MusubiDocumentRefV1::new(&"\\".repeat(2_048)).expect("maximal readme")),
        license: Some(MusubiDocumentRefV1::new(&"\\".repeat(2_048)).expect("maximal license")),
        repository: Some(
            MusubiDocumentRefV1::new(&"\\".repeat(2_048)).expect("maximal repository"),
        ),
        keywords,
    };
    let root_domain_text = "\\".repeat(253);
    let root_package = MusubiPackageIdV1::new(
        DataSpaceId::new(u64::MAX),
        MusubiPackageScopeV1::Domain(root_domain_text.parse().expect("maximal namespace domain")),
        "r".repeat(64).parse().expect("maximal root package name"),
    );
    let root_release = MusubiReleaseIdV1::new(
        root_package,
        MusubiVersionV1::new(u64::MAX, u64::MAX, u64::MAX, maximal_prerelease)
            .expect("maximal root release version"),
    );
    let resolution_snapshot = MusubiRegistrySnapshotV1 {
        finalized_height: 1,
        finalized_block_hash: [0x31; 32],
        index_revision: 1,
    };
    let manifest_abi = MusubiAbiBindingV1::new([0xFF; 32]).expect("manifest ABI");
    let archive_id = ArchiveId::new([0xFF; 32]);
    let interface_digest = MusubiContentDigestV1::new([0xFF; 32]);
    let publication_for = |dependency_count: usize| {
        let lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: root_release.clone(),
            root_dependencies: root_dependencies[..dependency_count].to_vec(),
            nodes: nodes[..dependency_count].to_vec(),
        };
        let manifest = MusubiReleaseManifestV1 {
            release: root_release.clone(),
            edition: MusubiKotodamaEditionV1::V1,
            abi: manifest_abi,
            dependencies: dependencies[..dependency_count].to_vec(),
            exports: exports.clone(),
            interface_digest,
            metadata: metadata.clone(),
            archive_id,
            verification_lock_digest: lock.digest(),
        };
        MusubiPublicationV1 {
            manifest,
            resolution: MusubiResolutionProofV1 {
                snapshot: resolution_snapshot,
                lock,
            },
        }
    };
    let network_id = test_network_id();
    let namespace = MusubiNamespaceV1::new(&format!("{root_domain_text}.n"))
        .expect("maximal domain-qualified namespace");
    let (publisher, publisher_signer) = near_limit_account();
    let transaction_frame_len = |dependency_count: usize| {
        let publication = publication_for(dependency_count);
        publication
            .validate()
            .expect("bounded publication fixture is structurally valid");
        let instruction =
            PublishMusubiReleaseV1::new(namespace.clone(), publication, None, 1, None);
        let mut builder = TransactionBuilder::new(
            network_id,
            publisher.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction]);
        builder.set_creation_time(Duration::from_millis(1));
        let signed = builder.sign_multisig([publisher_signer.private_key()]);
        norito::encode_canonical(&signed)
            .expect("canonical signed publication transaction frame")
            .len()
    };
    let max_transaction_bytes =
        usize::try_from(Parameters::default().transaction().max_tx_bytes().get())
            .expect("transaction limit fits usize");
    let mut admitted_dependency_count = 0_usize;
    let mut rejected_dependency_count = MUSUBI_MAX_DEPENDENCIES_V1 + 1;
    while admitted_dependency_count + 1 < rejected_dependency_count {
        let candidate =
            admitted_dependency_count + (rejected_dependency_count - admitted_dependency_count) / 2;
        if transaction_frame_len(candidate) <= max_transaction_bytes {
            admitted_dependency_count = candidate;
        } else {
            rejected_dependency_count = candidate;
        }
    }
    assert!(admitted_dependency_count > 0);
    assert!(
        transaction_frame_len(admitted_dependency_count) <= max_transaction_bytes,
        "selected publication must fit the default consensus transaction corridor"
    );
    if admitted_dependency_count < MUSUBI_MAX_DEPENDENCIES_V1 {
        assert!(
            transaction_frame_len(admitted_dependency_count + 1) > max_transaction_bytes,
            "selected dependency prefix must be maximal for this bounded fixture"
        );
    }
    let exact_snapshot_for = |manifest: MusubiReleaseManifestV1| {
        let snapshot = MusubiRegistrySnapshotV1 {
            finalized_height: u64::MAX,
            finalized_block_hash: [0xFF; 32],
            index_revision: u64::MAX,
        };
        let yank = MusubiReleaseYankV1 {
            release: manifest.release.clone(),
            yanked: false,
            reason: MusubiReasonV1::new(&"\\".repeat(1_024)).expect("maximal yank reason"),
            changed_by: publisher.clone(),
            changed_at_height: u64::MAX - 2,
            revision: u64::MAX,
        };
        let governance = MusubiArtifactGovernanceStateV1::TakenDown(MusubiArtifactTakedownV1 {
            action_digest: MusubiGovernanceActionDigestV1::new([0xFF; 32]),
            reason: MusubiReasonV1::new(&"\\".repeat(1_024)).expect("maximal takedown reason"),
            applied_at_height: u64::MAX - 1,
        });
        let release_digest = manifest.release_digest();
        let universal_release = MusubiResolverReleaseRowV1 {
            release: manifest.release.clone(),
            release_digest,
            archive_id: manifest.archive_id,
            source_digest: MusubiContentDigestV1::new([0xFF; 32]),
            interface_digest: manifest.interface_digest,
            abi: manifest.abi,
            dependencies: manifest.dependencies.clone(),
            selection: MusubiReleaseSelectionStateV1 {
                yank: yank.clone(),
                storage: MusubiArchiveAvailabilityV1 {
                    archive_id: manifest.archive_id,
                    availability: MusubiStorageAvailabilityV1::Selectable,
                    healthy_replicas: 256,
                    active_locations: 4,
                    finalized_height: snapshot.finalized_height,
                    finalized_block_hash: snapshot.finalized_block_hash,
                    index_revision: snapshot.index_revision,
                },
                governance: governance.clone(),
            },
            index_revision: snapshot.index_revision,
        };
        let home_release = MusubiReleaseRecordV1 {
            manifest,
            release_digest,
            published_by: publisher.clone(),
            published_at_height: u64::MAX - 3,
            yank,
            artifact_governance: governance,
            revisions: MusubiReleaseRevisionsV1 {
                yank: u64::MAX,
                artifact_governance: u64::MAX,
            },
        };
        MusubiExactReleaseSnapshotV1 {
            network_id,
            snapshot,
            home_release,
            universal_release,
        }
    };
    let admitted_publication = publication_for(admitted_dependency_count);
    let admitted_snapshot = exact_snapshot_for(admitted_publication.manifest);
    admitted_snapshot
        .validate()
        .expect("consensus-legal transaction-boundary exact release snapshot");
    let admitted_json = norito::json::to_vec(&admitted_snapshot)
        .expect("encode consensus-legal exact release JSON");
    eprintln!(
        "Musubi exact-release boundary: dependencies={admitted_dependency_count}, transaction_bytes={}, response_bytes={}",
        transaction_frame_len(admitted_dependency_count),
        admitted_json.len()
    );
    assert!(
        admitted_json.len() > RETIRED_QUERY_CAP_BYTES,
        "the retired 8 MiB cap must be proven insufficient"
    );
    assert!(
        admitted_json.len() <= MUSUBI_PUBLIC_QUERY_MAX_RESPONSE_BYTES,
        "the selected transaction-boundary exact-release response must fit the Musubi-only cap"
    );
    drop(admitted_json);
    drop(admitted_snapshot);
    let full_dependency_count = publication_for(MUSUBI_MAX_DEPENDENCIES_V1);
    full_dependency_count
        .validate()
        .expect("adversarial full-dependency publication is structurally valid");
    let full_dependency_snapshot = exact_snapshot_for(full_dependency_count.manifest);
    full_dependency_snapshot
        .validate()
        .expect("adversarial full-dependency exact release snapshot");
    let full_resolver_row_json = norito::json::to_vec(&full_dependency_snapshot.universal_release)
        .expect("encode adversarial full-dependency resolver row JSON");
    assert!(
        full_resolver_row_json.len() <= MUSUBI_RESOLVER_PAGE_JSON_ITEMS_BUDGET_BYTES_V1,
        "one adversarial full-dependency resolver row must fit the resolver items budget"
    );
    let full_dependency_json = norito::json::to_vec(&full_dependency_snapshot)
        .expect("encode adversarial full-dependency exact release JSON");
    eprintln!(
        "Musubi exact-release full-dependency fixture: dependencies={}, response_bytes={}",
        MUSUBI_MAX_DEPENDENCIES_V1,
        full_dependency_json.len()
    );
    assert!(
        full_dependency_json.len() <= MUSUBI_PUBLIC_QUERY_MAX_RESPONSE_BYTES,
        "the fixed cap must retain headroom above this full-dependency exact-release fixture"
    );
}
#[test]
fn public_musubi_query_surfaces_missing_and_stale_cursor() {
    let query = norito::json!({"limit": 1_u64});
    let client = client_with_base_url(base_url());
    let missing: PublicMusubiQueryResultV1<Value> = with_mock_http(
        respond_with(
            &Arc::new(Mutex::new(Vec::new())),
            empty_response(StatusCode::NOT_FOUND),
        ),
        || {
            post_public_musubi_query_v1(
                &client,
                PublicMusubiQueryPathV1::Versions,
                &query,
                Duration::from_secs(1),
            )
        },
    )
    .expect("missing query result");
    assert!(matches!(missing, PublicMusubiQueryResultV1::NotFound));
    let stale: PublicMusubiQueryResultV1<Value> = with_mock_http(
        respond_with(
            &Arc::new(Mutex::new(Vec::new())),
            empty_response(StatusCode::GONE),
        ),
        || {
            post_public_musubi_query_v1(
                &client,
                PublicMusubiQueryPathV1::OrderedPrefix,
                &query,
                Duration::from_secs(1),
            )
        },
    )
    .expect("stale query result");
    assert!(matches!(stale, PublicMusubiQueryResultV1::StaleCursor));
}
