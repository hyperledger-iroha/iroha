// Core Musubi identity, archive, verification, and publication tests.
use super::*;
use crate::sorafs::pin_registry::ProviderIngestCompletionSignerPolicyV1;
use iroha_crypto::{Algorithm, KeyPair, Signature};
use norito::codec::DecodeAll as _;
#[derive(Encode)]
struct UncheckedMultisigMemberWire {
    public_key: PublicKey,
    weight: u16,
}
#[derive(Encode)]
struct UncheckedMultisigPolicyWire {
    version: u8,
    threshold: u16,
    members: Vec<UncheckedMultisigMemberWire>,
}
#[test]
fn streamed_musubi_domain_hashes_preserve_exact_legacy_bytes() {
    let value = vec![3_u64, 5, 8, 13];
    let encoded = value.encode();
    let domain = b"iroha.musubi.streaming-hash.test.v1";
    assert_eq!(
        domain_hash_value(domain, &value),
        domain_hash(domain, &encoded)
    );
    let domain_len = u64::try_from(domain.len())
        .expect("test domain length fits u64")
        .to_le_bytes();
    let encoded_len = u64::try_from(encoded.len())
        .expect("test payload length fits u64")
        .to_le_bytes();
    let legacy = HashOf::<Vec<u64>>::from_untyped_unchecked(Hash::new_from_chunks(&[
        &domain_len,
        domain,
        &encoded_len,
        &encoded,
    ]));
    assert_eq!(domain_signing_hash(domain, &value), legacy);
}
#[allow(dead_code)]
#[derive(Encode)]
enum UncheckedAccountControllerWire {
    Single(PublicKey),
    Multisig(UncheckedMultisigPolicyWire),
}
#[cfg(feature = "json")]
#[test]
fn every_named_musubi_json_model_rejects_unknown_fields() {
    for (path, source) in [
        ("musubi.rs", include_str!("musubi.rs")),
        (
            "musubi/query_models.rs",
            include_str!("musubi/query_models.rs"),
        ),
        (
            "musubi/replication_order_lifecycle.rs",
            include_str!("musubi/replication_order_lifecycle.rs"),
        ),
        ("isi/musubi.rs", include_str!("isi/musubi.rs")),
        (
            "query/musubi_queries.rs",
            include_str!("query/musubi_queries.rs"),
        ),
    ] {
        let lines = source.lines().collect::<Vec<_>>();
        for (derive_index, derive) in lines.iter().enumerate() {
            if !derive.contains("derive(DeriveJsonSerialize, DeriveJsonDeserialize)") {
                continue;
            }
            let declaration_index = (derive_index + 1..lines.len().min(derive_index + 15))
                .find(|index| {
                    let line = lines[*index].trim_start();
                    line.starts_with("pub struct ")
                        || line.starts_with("pub enum ")
                        || line.starts_with("struct ")
                        || line.starts_with("enum ")
                })
                .unwrap_or_else(|| panic!("{path}: JSON derive lacks a nearby model declaration"));
            let declaration = lines[declaration_index].trim();
            if declaration.contains("struct ") && declaration.contains('(') {
                continue;
            }
            assert!(
                lines[derive_index..declaration_index]
                    .iter()
                    .any(|line| line.contains("deny_unknown_fields")),
                "{path}: {declaration} must reject unknown JSON fields"
            );
        }
    }
}
fn account(seed: u8) -> AccountId {
    let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("fixture seed derives a checked keypair");
    AccountId::new(keypair.public_key().clone())
}
fn unchecked_multisig_cursor_key(
    version: u8,
    threshold: u16,
    members: Vec<(PublicKey, u16)>,
) -> String {
    let controller = UncheckedAccountControllerWire::Multisig(UncheckedMultisigPolicyWire {
        version,
        threshold,
        members: members
            .into_iter()
            .map(|(public_key, weight)| UncheckedMultisigMemberWire { public_key, weight })
            .collect(),
    });
    maintainer_cursor_key_label_v1(&controller.encode(), None)
}
fn structurally_oversized_account() -> AccountId {
    let members = (0_u16..256)
        .map(|index| {
            let mut seed = [0xA5; 32];
            seed[..2].copy_from_slice(&index.to_le_bytes());
            let keypair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519)
                .expect("oversized-account fixture seed derives a checked keypair");
            MultisigMember::new(keypair.public_key().clone(), 1)
                .expect("oversized-account fixture member")
        })
        .collect();
    let policy = MultisigPolicy::new(1, members).expect("oversized-account fixture policy");
    let account = AccountId::new_multisig(policy);
    assert!(
        norito::encode_canonical(&account)
            .expect("oversized account has canonical Norito bytes")
            .len()
            > MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1
    );
    account
}
fn package(name: &str) -> MusubiPackageIdV1 {
    MusubiPackageIdV1::new(
        DataSpaceId::new(7),
        MusubiPackageScopeV1::Domain("dex".parse().expect("domain")),
        name.parse().expect("package name"),
    )
}
#[test]
fn portable_path_set_accepts_canonical_bundle_paths_in_any_order() {
    let paths = [
        vec!["src".to_owned(), "caf\u{e9}.ko".to_owned()],
        vec!["Musubi.toml".to_owned()],
        vec![".musubi".to_owned(), "semantic-release.norito".to_owned()],
    ];
    validate_musubi_portable_path_set_v1(paths.iter().map(Vec::as_slice))
        .expect("canonical unordered bundle paths must validate");
}
#[test]
fn portable_path_set_rejects_noncanonical_and_unsafe_components() {
    for component in [
        "cafe\u{301}.ko",
        "CON.ko",
        "trailing.",
        "bidirectional\u{202e}name.ko",
        "colon:name.ko",
    ] {
        let paths = [vec!["src".to_owned(), component.to_owned()]];
        assert!(
            validate_musubi_portable_path_set_v1(paths.iter().map(Vec::as_slice)).is_err(),
            "unsafe component was accepted: {component:?}"
        );
    }
    let oversized_component = "a".repeat(MUSUBI_MAX_PORTABLE_PATH_COMPONENT_BYTES_V1 + 1);
    let paths = [vec![oversized_component]];
    assert!(validate_musubi_portable_path_set_v1(paths.iter().map(Vec::as_slice)).is_err());
    let overdeep = vec!["a".to_owned(); MUSUBI_MAX_PORTABLE_PATH_COMPONENTS_V1 + 1];
    let paths = [overdeep];
    assert!(validate_musubi_portable_path_set_v1(paths.iter().map(Vec::as_slice)).is_err());
}
#[test]
fn portable_path_set_rejects_exact_and_casefolded_aliases_and_prefixes() {
    for paths in [
        vec![vec!["a".to_owned()], vec!["a".to_owned()]],
        vec![vec!["a".to_owned()], vec!["a".to_owned(), "z".to_owned()]],
        vec![
            vec!["src".to_owned(), "Foo.ko".to_owned()],
            vec!["src".to_owned(), "foo.ko".to_owned()],
        ],
        vec![
            vec!["src".to_owned(), "Stra\u{df}e.ko".to_owned()],
            vec!["src".to_owned(), "STRASSE.ko".to_owned()],
        ],
        vec![
            vec!["Foo".to_owned()],
            vec!["foo".to_owned(), "z".to_owned()],
        ],
    ] {
        assert!(validate_musubi_portable_path_set_v1(paths.iter().map(Vec::as_slice)).is_err());
    }
}
fn release(name: &str, version: &str) -> MusubiReleaseIdV1 {
    MusubiReleaseIdV1::new(package(name), version.parse().expect("version"))
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one closed test covers every number-encoded Musubi Parliament role"
)]
fn parliament_actions_check_every_number_encoded_u64_role() {
    let maximum = crate::parliament_types::FIRST_RELEASE_MAX_EXACT_JSON_U64;
    let hostile = maximum + 1;
    let recover = |dataspace, expected_revision| {
        let mut package = package("recoverable");
        package.home_dataspace = DataSpaceId::new(dataspace);
        MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
            package,
            owners: vec![account(1)],
            expected_revision,
        })
    };
    let retarget = |dataspace, expected_revision| {
        let mut target = package("retargeted");
        target.home_dataspace = DataSpaceId::new(dataspace);
        MusubiParliamentActionV1::RetargetAlias(MusubiRetargetAliasV1 {
            alias: "stable".parse().expect("alias"),
            target,
            expected_revision,
        })
    };
    let takedown = |package_dataspace,
                    major,
                    minor,
                    patch,
                    prerelease,
                    expected_artifact_governance_revision| {
        let mut package = package("takedown");
        package.home_dataspace = DataSpaceId::new(package_dataspace);
        MusubiParliamentActionV1::TakedownArtifact(MusubiTakedownArtifactActionV1 {
            release: MusubiReleaseIdV1::new(
                package,
                MusubiVersionV1::new(major, minor, patch, prerelease).expect("version fixture"),
            ),
            reason: "governed-takedown".parse().expect("reason"),
            expected_artifact_governance_revision,
        })
    };
    let policy = |policy: MusubiRegistryPolicyV1, expected_revision| {
        MusubiParliamentActionV1::SetRegistryPolicy(MusubiSetRegistryPolicyActionV1 {
            policy,
            expected_revision,
        })
    };

    let baseline_policy = MusubiRegistryPolicyV1 {
        revision: maximum,
        allowlisted_dataspaces: vec![DataSpaceId::new(maximum)],
        alias_pricing: MusubiAliasPricingPolicyV1 {
            revision: maximum,
            length_1_xor: maximum,
            length_2_xor: maximum,
            length_3_xor: maximum,
            length_4_xor: maximum,
            length_5_to_32_xor: maximum,
        },
        ..MusubiRegistryPolicyV1::default()
    };
    for within in [
        recover(maximum, maximum),
        retarget(maximum, maximum),
        takedown(
            maximum,
            maximum,
            maximum,
            maximum,
            vec![MusubiPrereleaseIdentifierV1::Numeric(maximum)],
            maximum,
        ),
        policy(baseline_policy.clone(), maximum),
    ] {
        assert_eq!(
            within.first_release_exact_json_u64_invariant_error(maximum),
            None
        );
    }

    let mut hostile_cases = vec![
        recover(hostile, maximum),
        recover(maximum, hostile),
        retarget(hostile, maximum),
        retarget(maximum, hostile),
        takedown(maximum, hostile, maximum, maximum, Vec::new(), maximum),
        takedown(maximum, maximum, hostile, maximum, Vec::new(), maximum),
        takedown(maximum, maximum, maximum, hostile, Vec::new(), maximum),
        takedown(
            maximum,
            maximum,
            maximum,
            maximum,
            vec![MusubiPrereleaseIdentifierV1::Numeric(hostile)],
            maximum,
        ),
        takedown(maximum, maximum, maximum, maximum, Vec::new(), hostile),
        policy(baseline_policy.clone(), hostile),
    ];
    let mut release_dataspace = takedown(1, 1, 0, 0, Vec::new(), 1);
    let MusubiParliamentActionV1::TakedownArtifact(payload) = &mut release_dataspace else {
        unreachable!()
    };
    payload.release.package.home_dataspace = DataSpaceId::new(hostile);
    hostile_cases.push(release_dataspace);

    let mut policy_revision = baseline_policy.clone();
    policy_revision.revision = hostile;
    hostile_cases.push(policy(policy_revision, maximum));
    let mut allowlisted = baseline_policy.clone();
    allowlisted.allowlisted_dataspaces = vec![DataSpaceId::new(hostile)];
    hostile_cases.push(policy(allowlisted, maximum));
    for role in 0..6 {
        let mut pricing = baseline_policy.clone();
        match role {
            0 => pricing.alias_pricing.revision = hostile,
            1 => pricing.alias_pricing.length_1_xor = hostile,
            2 => pricing.alias_pricing.length_2_xor = hostile,
            3 => pricing.alias_pricing.length_3_xor = hostile,
            4 => pricing.alias_pricing.length_4_xor = hostile,
            5 => pricing.alias_pricing.length_5_to_32_xor = hostile,
            _ => unreachable!(),
        }
        hostile_cases.push(policy(pricing, maximum));
    }
    for hostile_action in hostile_cases {
        assert!(
            hostile_action
                .first_release_exact_json_u64_invariant_error(maximum)
                .is_some()
        );
    }
}
fn snapshot() -> MusubiRegistrySnapshotV1 {
    MusubiRegistrySnapshotV1 {
        finalized_height: 42,
        finalized_block_hash: [0x42; 32],
        index_revision: 3,
    }
}
fn test_network_id(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(
        Hash::new([seed; Hash::LENGTH]),
    ))
}
fn archive_commitment() -> MusubiArchiveCommitmentV1 {
    MusubiArchiveCommitmentV1 {
        root_cid: ManifestRootCid::from_blake3_digest([1; 32]).expect("root CID"),
        chunker: ChunkerProfileHandle {
            profile_id: 1,
            namespace: "sorafs".to_owned(),
            name: "sf1".to_owned(),
            semver: "1.0.0".to_owned(),
            multihash_code: 0x1f,
        },
        chunk_plan_digest: MusubiContentDigestV1::new([2; 32]),
        por_root: MusubiContentDigestV1::new([3; 32]),
        content_length: 1_024,
        car_digest: MusubiContentDigestV1::new([4; 32]),
        car_size: 2_048,
        bundle_digest: MusubiContentDigestV1::new([5; 32]),
        source_tree_digest: MusubiContentDigestV1::new([6; 32]),
        descriptor_digest: MusubiContentDigestV1::new([7; 32]),
        file_count: 2,
        chunk_count: 4,
    }
}
fn seed_ingress_binding(broker: AccountId) -> MusubiSeedIngressReceiptBindingV1 {
    let commitment = archive_commitment();
    MusubiSeedIngressReceiptBindingV1 {
        network_id: test_network_id(0x15),
        publisher: account(20),
        ingress_broker: broker,
        seed_provider: ProviderId::new([0x16; 32]),
        semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new([0x17; 32]),
        archive_id: commitment.archive_id(),
        car_body_digest: commitment.car_digest,
        car_body_length: commitment.car_size,
        nonce: [0x18; 32],
    }
}
fn provider_completion_authority(owner: AccountId) -> ProviderIngestCompletionAuthorityV1 {
    ProviderIngestCompletionAuthorityV1::new(
        owner,
        ProviderIngestCompletionSignerPolicyV1 {
            policy_id: [0x21; 32],
            revision: 1,
            predecessor_digest: None,
            policy_digest: [0x22; 32],
        },
    )
}
fn provider_bundle_binding(owner: AccountId) -> MusubiProviderBundleVerificationBindingV1 {
    MusubiProviderBundleVerificationBindingV1 {
        network_id: test_network_id(0x23),
        provider_id: ProviderId::new([0x24; 32]),
        completed_by: owner.clone(),
        completion_authority: provider_completion_authority(owner),
        replication_order: ReplicationOrderId::new([0x25; 32]),
        assignment_revision: 3,
        completion_epoch: 9,
        finalized_anchor: ProviderIngestFinalizedAnchorV1 {
            height: 77,
            block_hash: [0x26; 32],
        },
        archive_id: archive_commitment().archive_id(),
        bundle_digest: MusubiContentDigestV1::new([0x27; 32]),
        descriptor_digest: MusubiContentDigestV1::new([0x28; 32]),
        semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new([0x29; 32]),
        verification_lock_digest: MusubiVerificationLockDigestV1::new([0x2A; 32]),
        source_tree_digest: MusubiContentDigestV1::new([0x2B; 32]),
    }
}
fn verification_lock(root: MusubiReleaseIdV1) -> MusubiVerificationLockV1 {
    MusubiVerificationLockV1 {
        schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
        version: MUSUBI_REGISTRY_VERSION_V1,
        root,
        root_dependencies: Vec::new(),
        nodes: Vec::new(),
    }
}
fn canonical_bundle_metadata() -> (
    MusubiArtifactDescriptorV1,
    MusubiSemanticReleaseManifestV1,
    MusubiVerificationLockV1,
) {
    let manifest = release_manifest();
    let semantic = manifest.semantic_manifest();
    let lock = verification_lock(semantic.release.clone());
    assert_eq!(semantic.verification_lock_digest, lock.digest());
    let descriptor = MusubiArtifactDescriptorV1::new(
        semantic.semantic_digest(),
        MusubiContentDigestV1::new([0x44; 32]),
        lock.digest(),
        128,
        1,
    )
    .expect("canonical descriptor");
    (descriptor, semantic, lock)
}
fn dense_verification_lock(fanout: usize) -> MusubiVerificationLockV1 {
    const LAYER_WIDTH: usize = MUSUBI_MAX_DEPENDENCIES_V1;
    const LAYER_COUNT: usize = 4;
    let releases = (0..LAYER_WIDTH * LAYER_COUNT)
        .map(|index| release(&format!("node-{index:04}"), "1.0.0"))
        .collect::<Vec<_>>();
    let exact_edge = |alias: String, selected: MusubiReleaseIdV1| MusubiExactDependencyEdgeV1 {
        alias: alias.parse().expect("dense-lock dependency alias"),
        kind: MusubiDependencyKindV1::Normal,
        package: selected.package.clone(),
        requirement: "^1.0.0".parse().expect("dense-lock requirement"),
        selected,
    };
    let root_dependencies = releases[..LAYER_WIDTH]
        .iter()
        .enumerate()
        .map(|(index, selected)| exact_edge(format!("node-{index:04}"), selected.clone()))
        .collect();
    let nodes = releases
        .iter()
        .enumerate()
        .map(|(index, release)| {
            let layer = index / LAYER_WIDTH;
            let column = index % LAYER_WIDTH;
            let dependencies = if layer + 1 == LAYER_COUNT {
                Vec::new()
            } else {
                (0..fanout)
                    .map(|offset| {
                        let target_column = (column + offset) % LAYER_WIDTH;
                        let target = (layer + 1) * LAYER_WIDTH + target_column;
                        exact_edge(format!("dep-{target:04}"), releases[target].clone())
                    })
                    .collect()
            };
            let fill = u8::try_from(index % 250 + 1).expect("bounded dense-lock fill");
            MusubiVerificationNodeV1 {
                release: release.clone(),
                release_digest: MusubiReleaseDigestV1::new([fill; 32]),
                archive_id: ArchiveId::new([fill.saturating_add(1); 32]),
                source_digest: MusubiContentDigestV1::new([fill.saturating_add(2); 32]),
                interface_digest: MusubiContentDigestV1::new([fill.saturating_add(3); 32]),
                abi: MusubiAbiBindingV1::new([fill.saturating_add(4); 32]).expect("ABI"),
                dependencies,
            }
        })
        .collect();
    let mut lock = MusubiVerificationLockV1 {
        schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
        version: MUSUBI_REGISTRY_VERSION_V1,
        root: release("dense-root", "1.0.0"),
        root_dependencies,
        nodes,
    };
    lock.canonicalize();
    lock
}
fn release_manifest() -> MusubiReleaseManifestV1 {
    let release = release("swap-core", "1.2.3");
    let lock = verification_lock(release.clone());
    MusubiReleaseManifestV1 {
        release,
        edition: MusubiKotodamaEditionV1::V1,
        abi: MusubiAbiBindingV1::new([8; 32]).expect("ABI"),
        dependencies: Vec::new(),
        exports: vec!["quote".parse().expect("export")],
        interface_digest: MusubiContentDigestV1::new([9; 32]),
        metadata: MusubiReleaseMetadataV1::default(),
        archive_id: archive_commitment().archive_id(),
        verification_lock_digest: lock.digest(),
    }
}
fn resolver_row(version: &str) -> MusubiResolverReleaseRowV1 {
    let mut manifest = release_manifest();
    manifest.release = release("swap-core", version);
    let release_digest = manifest.release_digest();
    let release = manifest.release.clone();
    let archive_id = manifest.archive_id;
    MusubiResolverReleaseRowV1 {
        release: release.clone(),
        release_digest,
        archive_id,
        source_digest: MusubiContentDigestV1::new([0x61; 32]),
        interface_digest: manifest.interface_digest,
        abi: manifest.abi,
        dependencies: manifest.dependencies,
        selection: MusubiReleaseSelectionStateV1 {
            yank: MusubiReleaseYankV1 {
                release,
                yanked: false,
                reason: "initial publication".parse().expect("yank reason"),
                changed_by: account(17),
                changed_at_height: 42,
                revision: 1,
            },
            storage: MusubiArchiveAvailabilityV1 {
                archive_id,
                availability: MusubiStorageAvailabilityV1::Selectable,
                healthy_replicas: MUSUBI_MIN_HEALTHY_REPLICAS_V1,
                active_locations: 1,
                finalized_height: 42,
                finalized_block_hash: [0x42; 32],
                index_revision: 3,
            },
            governance: MusubiArtifactGovernanceStateV1::Available,
        },
        index_revision: 3,
    }
}
#[cfg(feature = "json")]
#[test]
fn resolver_json_counting_preserves_exact_wire_without_output_scratch() {
    let member_a = KeyPair::try_from_seed(vec![0x21; 32], Algorithm::Ed25519)
        .expect("fixture seed derives a checked keypair");
    let member_b = KeyPair::try_from_seed(vec![0x22; 32], Algorithm::Ed25519)
        .expect("fixture seed derives a checked keypair");
    let multisig = AccountId::new_multisig(
        MultisigPolicy::new(
            1,
            vec![
                MultisigMember::new(member_a.public_key().clone(), 1).expect("valid member"),
                MultisigMember::new(member_b.public_key().clone(), 1).expect("valid member"),
            ],
        )
        .expect("valid policy"),
    );
    for discriminant in [0x02f1, 0x0171, 0, 42] {
        let _guard = crate::account::address::ChainDiscriminantGuard::enter(discriminant);
        for changed_by in [account(17), multisig.clone()] {
            let mut row = resolver_row("1.0.0");
            row.selection.yank.changed_by = changed_by;
            let counted_row_len = row
                .canonical_json_len_bounded(usize::MAX)
                .expect("count resolver row JSON before the ordinary encoder can populate caches");
            let bounded_row_json = norito::json::to_json_bounded(&row, counted_row_len)
                .expect("bounded resolver row JSON before the ordinary encoder populates caches");
            let row_json = norito::json::to_json(&row).expect("encode resolver row JSON");
            assert_eq!(counted_row_len, row_json.len());
            assert_eq!(bounded_row_json, row_json);
            assert_eq!(
                row.canonical_json_len_bounded(row_json.len() - 1),
                Err(norito::json::BoundedJsonError::BodyTooLarge),
            );
            let page = MusubiResolverIndexPageV1 {
                query: MusubiResolverIndexQueryV1 {
                    package: row.release.package.clone(),
                    requirement: None,
                    page: MusubiPageRequestV1 {
                        limit: 1,
                        cursor: None,
                    },
                },
                network_id: test_network_id(0x15),
                items: vec![row],
                next_cursor: None,
                snapshot: snapshot(),
            };
            let counted_page_len = streaming::musubi_json_len_bounded(&page, usize::MAX)
                .expect("count resolver page JSON");
            let bounded_page_json = norito::json::to_json_bounded(&page, counted_page_len)
                .expect("bounded resolver page JSON");
            let page_json = norito::json::to_json(&page).expect("encode resolver page JSON");
            assert_eq!(bounded_page_json, page_json);
            assert_eq!(counted_page_len, page_json.len());
        }
    }
}
#[test]
fn resolver_row_rejects_availability_newer_than_its_row() {
    let mut row = resolver_row("1.0.0");
    row.selection.storage.index_revision = row.index_revision + 1;
    assert!(row.validate().is_err());
}
#[test]
fn namespace_binding_uses_stable_dataspace_scope_and_generation() {
    let binding = MusubiNamespaceBindingV1 {
        namespace: "dex.universal".parse().expect("namespace"),
        home_dataspace: DataSpaceId::new(7),
        scope: MusubiPackageScopeV1::Domain("dex".parse().expect("domain")),
        generation: 4,
    };
    binding.validate().expect("valid binding");
    assert!(!binding.digest().is_zero());
    let mut invalid = binding.clone();
    invalid.generation = 0;
    assert!(invalid.validate().is_err());
    invalid.generation = 1;
    invalid.scope = MusubiPackageScopeV1::DataspaceRoot;
    assert!(invalid.validate().is_err());
    let selector: MusubiPackageSelectorV1 = "dex.universal/swap-core".parse().expect("selector");
    assert_eq!(selector.to_string(), "dex.universal/swap-core");
}
#[test]
fn account_identity_bound_is_exact_and_recursive() {
    assert!(
        validate_musubi_account_id_canonical_bytes_v1(&vec![
            0xA5;
            MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1
        ])
        .is_ok()
    );
    assert!(
        validate_musubi_account_id_canonical_bytes_v1(&vec![
            0xA5;
            MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1
                + 1
        ])
        .is_err()
    );
    validate_musubi_account_id_v1(&account(39)).expect("ordinary account fits the bound");
    let oversized = structurally_oversized_account();
    assert!(validate_musubi_account_id_v1(&oversized).is_err());
    let package_record = MusubiPackageRecordV1 {
        package: package("bounded-accounts"),
        claimed_namespace: "dex.universal".parse().expect("namespace"),
        claimed_namespace_binding: MusubiNamespaceBindingDigestV1::new([0xA6; 32]),
        owners: vec![oversized.clone()],
        member_accounts: vec![oversized.clone()],
        claimed_at_height: 1,
        revisions: MusubiPackageRevisionsV1 {
            governance: 1,
            metadata: 1,
            archive_locations: 1,
        },
    };
    assert!(
        package_record.validate().is_err(),
        "account vectors must enforce the shared canonical bound"
    );
    let cursor = MusubiFinalizedCursorV1 {
        snapshot: snapshot(),
        query_hash: MusubiQueryHashV1::new([0xA7; 32]),
        last_key: "bounded-caller".to_owned(),
        caller: Some(oversized.clone()),
    };
    assert!(
        cursor.validate().is_err(),
        "optional caller bindings must enforce the shared canonical bound"
    );
    let provider_binding = provider_bundle_binding(oversized);
    assert!(
        provider_binding.validate().is_err(),
        "nested provider completion authorities must enforce the shared canonical bound"
    );
}
#[test]
fn account_and_provider_attestation_admission_ignore_and_restore_ambient_flags() {
    let account = account(39);
    let provider_keypair = KeyPair::try_from_seed(vec![63; 32], Algorithm::Ed25519)
        .expect("provider owner fixture keypair");
    let binding = provider_bundle_binding(AccountId::new(provider_keypair.public_key().clone()));
    let payload = MusubiProviderBundleVerificationPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        binding,
    };
    let attestation = MusubiProviderBundleVerificationAttestationV1 {
        approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
            public_key: provider_keypair.public_key().clone(),
            signature: SignatureOf::try_from_hash(
                provider_keypair.private_key(),
                payload.signing_hash(),
            )
            .expect("provider approval"),
        }],
        payload,
    };
    let canonical_account =
        norito::encode_canonical(&account).expect("encode canonical Musubi account");
    let canonical_attestation = norito::encode_canonical(&attestation)
        .expect("encode canonical Musubi provider attestation");
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let (alternate_account, alternate_attestation) = {
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        (
            norito::to_bytes(&account).expect("encode alternate-layout Musubi account"),
            norito::to_bytes(&attestation)
                .expect("encode alternate-layout Musubi provider attestation"),
        )
    };
    assert_ne!(alternate_account, canonical_account);
    assert_ne!(alternate_attestation, canonical_attestation);
    validate_musubi_account_id_v1(&account).expect("baseline account admission");
    attestation
        .validate()
        .expect("baseline provider attestation admission");
    {
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        let before_account =
            norito::to_bytes(&account).expect("encode account under caller ambient flags");
        let before_attestation = norito::to_bytes(&attestation)
            .expect("encode provider attestation under caller ambient flags");
        validate_musubi_account_id_v1(&account).expect("ambient account admission");
        attestation
            .validate()
            .expect("ambient provider attestation admission");
        assert_eq!(
            norito::encode_canonical(&account).expect("canonicalize ambient account"),
            canonical_account
        );
        assert_eq!(
            norito::encode_canonical(&attestation)
                .expect("canonicalize ambient provider attestation"),
            canonical_attestation
        );
        assert_eq!(
            norito::to_bytes(&account).expect("re-encode account under caller ambient flags"),
            before_account
        );
        assert_eq!(
            norito::to_bytes(&attestation)
                .expect("re-encode provider attestation under caller ambient flags"),
            before_attestation
        );
    }
}
#[test]
fn approval_sets_reject_wrong_signature_payload_lengths() {
    const WRONG_SIGNATURE_BYTES: [u8; 63] = [0xA8; 63];
    const LENGTH_ERROR: &str = "Musubi approval signature payload length is invalid";
    let owner_keypair = KeyPair::try_from_seed(vec![44; 32], Algorithm::Ed25519)
        .expect("namespace owner fixture keypair");
    let owner = AccountId::new(owner_keypair.public_key().clone());
    let delegation = MusubiNamespaceDelegationV1 {
        payload: MusubiNamespaceDelegationPayloadV1 {
            version: MUSUBI_REGISTRY_VERSION_V1,
            namespace_binding: MusubiNamespaceBindingDigestV1::new([0xA9; 32]),
            owner_generation: 1,
            owner,
            delegate: account(45),
            expires_at_height: 10,
        },
        approvals: vec![MusubiNamespaceDelegationApprovalV1 {
            public_key: owner_keypair.public_key().clone(),
            signature: SignatureOf::from_signature(Signature::from_bytes(&WRONG_SIGNATURE_BYTES)),
        }],
    };
    assert_eq!(
        delegation
            .validate()
            .expect_err("short signature must fail")
            .reason(),
        LENGTH_ERROR
    );
    let broker_keypair = KeyPair::try_from_seed(vec![53; 32], Algorithm::Ed25519)
        .expect("ingress broker fixture keypair");
    let receipt = MusubiSeedIngressReceiptV1 {
        payload: MusubiSeedIngressReceiptPayloadV1 {
            version: MUSUBI_REGISTRY_VERSION_V1,
            binding: seed_ingress_binding(AccountId::new(broker_keypair.public_key().clone())),
            issued_at_ms: 1_000,
            expires_at_ms: 2_000,
        },
        approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
            public_key: broker_keypair.public_key().clone(),
            signature: SignatureOf::from_signature(Signature::from_bytes(&WRONG_SIGNATURE_BYTES)),
        }],
    };
    assert_eq!(
        receipt
            .validate()
            .expect_err("short signature must fail")
            .reason(),
        LENGTH_ERROR
    );
    let provider_keypair = KeyPair::try_from_seed(vec![63; 32], Algorithm::Ed25519)
        .expect("provider owner fixture keypair");
    let attestation = MusubiProviderBundleVerificationAttestationV1 {
        payload: MusubiProviderBundleVerificationPayloadV1 {
            version: MUSUBI_REGISTRY_VERSION_V1,
            binding: provider_bundle_binding(AccountId::new(provider_keypair.public_key().clone())),
        },
        approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
            public_key: provider_keypair.public_key().clone(),
            signature: SignatureOf::from_signature(Signature::from_bytes(&WRONG_SIGNATURE_BYTES)),
        }],
    };
    assert_eq!(
        attestation
            .validate()
            .expect_err("short signature must fail")
            .reason(),
        LENGTH_ERROR
    );
}
#[test]
fn namespace_delegation_authenticates_owner_generation_and_delegate() {
    let owner_keypair =
        KeyPair::try_from_seed(vec![41; 32], Algorithm::Ed25519).expect("owner fixture keypair");
    let owner = AccountId::new(owner_keypair.public_key().clone());
    let delegate = account(42);
    let binding = MusubiNamespaceBindingV1 {
        namespace: "dex.universal".parse().expect("namespace"),
        home_dataspace: DataSpaceId::new(7),
        scope: MusubiPackageScopeV1::Domain("dex".parse().expect("domain")),
        generation: 4,
    };
    binding
        .validate_authority_generation(4)
        .expect("binding generation is current at registration");
    let payload = MusubiNamespaceDelegationPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        namespace_binding: binding.digest(),
        owner_generation: 4,
        owner: owner.clone(),
        delegate: delegate.clone(),
        expires_at_height: 100,
    };
    let approval = MusubiNamespaceDelegationApprovalV1 {
        public_key: owner_keypair.public_key().clone(),
        signature: SignatureOf::try_from_hash(owner_keypair.private_key(), payload.signing_hash())
            .expect("sign delegation"),
    };
    let delegation = MusubiNamespaceDelegationV1 {
        payload,
        approvals: vec![approval],
    };
    delegation
        .verify(&binding, &owner, 4, &delegate, 100)
        .expect("current signed delegation verifies");
    assert!(
        delegation
            .verify(&binding, &owner, 5, &delegate, 100)
            .is_err()
    );
    assert!(
        delegation
            .verify(&binding, &owner, 4, &account(43), 100)
            .is_err()
    );
    assert!(
        delegation
            .verify(&binding, &owner, 4, &delegate, 101)
            .is_err()
    );
}
#[test]
fn seed_ingress_receipt_rejects_same_label_foreign_genesis_and_commitment_substitution() {
    let broker_keypair =
        KeyPair::try_from_seed(vec![51; 32], Algorithm::Ed25519).expect("broker fixture keypair");
    let broker = AccountId::new(broker_keypair.public_key().clone());
    let binding = seed_ingress_binding(broker);
    let payload = MusubiSeedIngressReceiptPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        binding: binding.clone(),
        issued_at_ms: 1_000,
        expires_at_ms: 2_000,
    };
    let receipt = MusubiSeedIngressReceiptV1 {
        approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
            public_key: broker_keypair.public_key().clone(),
            signature: SignatureOf::try_from_hash(
                broker_keypair.private_key(),
                payload.signing_hash(),
            )
            .expect("sign seed-ingress receipt"),
        }],
        payload,
    };
    receipt
        .verify(&binding, 1_500)
        .expect("current exact receipt verifies");
    assert!(receipt.verify(&binding, 999).is_err());
    assert!(receipt.verify(&binding, 2_001).is_err());
    let mut replayed = binding.clone();
    // A deployment may reuse the same human-facing ChainName; the exact genesis-derived
    // NetworkId still makes its receipt a different signing domain.
    replayed.network_id = test_network_id(0x19);
    assert!(receipt.verify(&replayed, 1_500).is_err());
    let mut substituted = binding.clone();
    substituted.archive_id = ArchiveId::new([0xEE; 32]);
    assert!(receipt.verify(&substituted, 1_500).is_err());
    let mut tampered = receipt.clone();
    tampered.payload.binding.car_body_digest = MusubiContentDigestV1::new([0xEF; 32]);
    let tampered_binding = tampered.payload.binding.clone();
    assert!(tampered.verify(&tampered_binding, 1_500).is_err());
    let decoded = MusubiSeedIngressReceiptV1::decode_all(&mut receipt.encode().as_slice())
        .expect("receipt Norito roundtrip");
    assert_eq!(decoded, receipt);
}
#[test]
fn archive_registration_projection_excludes_mutable_location_state() {
    let broker_keypair =
        KeyPair::try_from_seed(vec![52; 32], Algorithm::Ed25519).expect("broker fixture keypair");
    let broker = AccountId::new(broker_keypair.public_key().clone());
    let binding = seed_ingress_binding(broker);
    let payload = MusubiSeedIngressReceiptPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        binding: binding.clone(),
        issued_at_ms: 1_000,
        expires_at_ms: 2_000,
    };
    let receipt = MusubiSeedIngressReceiptV1 {
        approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
            public_key: broker_keypair.public_key().clone(),
            signature: SignatureOf::try_from_hash(
                broker_keypair.private_key(),
                payload.signing_hash(),
            )
            .expect("sign seed-ingress receipt"),
        }],
        payload,
    };
    let mut archive = MusubiArchiveRecordV1 {
        archive_id: binding.archive_id,
        commitment: archive_commitment(),
        staging_receipt: receipt,
        registered_by: binding.publisher,
        registered_at_height: 7,
        location_revision: 1,
        location_ids: Vec::new(),
    };
    archive.validate().expect("canonical archive record");
    let projection = archive.registration_projection();
    projection
        .validate()
        .expect("canonical immutable registration projection");
    let page = MusubiArchiveLocationPageV1 {
        network_id: test_network_id(0x15),
        archive: archive.clone(),
        items: Vec::new(),
        next_cursor: None,
        snapshot: snapshot(),
    };
    page.validate()
        .expect("archive page binds its exact receipt network identity");
    let mut wrong_network = page;
    wrong_network
        .archive
        .staging_receipt
        .payload
        .binding
        .network_id = test_network_id(0x19);
    assert!(wrong_network.validate().is_err());
    archive.location_revision = 9;
    archive.location_ids = vec![MusubiArchiveLocationIdV1::new([0x31; 32])];
    assert_eq!(
        archive.registration_projection(),
        projection,
        "renewable location state must not enter historical registration evidence"
    );
    let decoded =
        MusubiArchiveRegistrationProjectionV1::decode_all(&mut projection.encode().as_slice())
            .expect("registration projection Norito roundtrip");
    assert_eq!(decoded, projection);
    let mut zero_height = projection;
    zero_height.registered_at_height = 0;
    assert!(zero_height.validate().is_err());
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one provider-attestation scenario covers quorum, replay, substitution, retention, and wire invariants"
)]
fn provider_bundle_attestation_requires_controller_quorum_and_exact_finalized_completion() {
    let first =
        KeyPair::try_from_seed(vec![61; 32], Algorithm::Ed25519).expect("first provider keypair");
    let second =
        KeyPair::try_from_seed(vec![62; 32], Algorithm::Ed25519).expect("second provider keypair");
    let policy = MultisigPolicy::new(
        2,
        vec![
            MultisigMember::new(first.public_key().clone(), 1).expect("first member"),
            MultisigMember::new(second.public_key().clone(), 1).expect("second member"),
        ],
    )
    .expect("provider owner policy");
    let owner = AccountId::new_multisig(policy);
    let binding = provider_bundle_binding(owner);
    let payload = MusubiProviderBundleVerificationPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        binding: binding.clone(),
    };
    let signing_hash = payload.signing_hash();
    let mut approvals = vec![
        MusubiProviderBundleVerificationApprovalV1 {
            public_key: first.public_key().clone(),
            signature: SignatureOf::try_from_hash(first.private_key(), signing_hash)
                .expect("first provider approval"),
        },
        MusubiProviderBundleVerificationApprovalV1 {
            public_key: second.public_key().clone(),
            signature: SignatureOf::try_from_hash(second.private_key(), signing_hash)
                .expect("second provider approval"),
        },
    ];
    approvals.sort_by(|left, right| left.public_key.cmp(&right.public_key));
    let attestation = MusubiProviderBundleVerificationAttestationV1 { payload, approvals };
    attestation
        .verify(&binding)
        .expect("provider controller quorum verifies exact bundle and completion");
    let mut below_quorum = attestation.clone();
    below_quorum.approvals.pop();
    assert!(below_quorum.verify(&binding).is_err());
    let mut replayed_completion = binding.clone();
    replayed_completion.finalized_anchor.block_hash = [0xED; 32];
    assert!(attestation.verify(&replayed_completion).is_err());
    let mut foreign_genesis = binding.clone();
    // Reusing a display label does not make another genesis lineage authoritative.
    foreign_genesis.network_id = test_network_id(0x29);
    assert!(attestation.verify(&foreign_genesis).is_err());
    let mut substituted = binding.clone();
    substituted.verification_lock_digest = MusubiVerificationLockDigestV1::new([0xEC; 32]);
    assert!(attestation.verify(&substituted).is_err());
    let mut tampered = attestation.clone();
    tampered.payload.binding.source_tree_digest = MusubiContentDigestV1::new([0xEB; 32]);
    let tampered_binding = tampered.payload.binding.clone();
    assert!(tampered.verify(&tampered_binding).is_err());
    let decoded = MusubiProviderBundleVerificationAttestationV1::decode_all(
        &mut attestation.encode().as_slice(),
    )
    .expect("provider attestation Norito roundtrip");
    assert_eq!(decoded, attestation);
    let reference = attestation.reference();
    let attestation_set_digest = musubi_provider_bundle_attestation_set_digest_v1(
        binding.archive_id,
        binding.replication_order,
        &[reference],
    )
    .expect("archive/order-bound provider attestation set digest");
    assert_ne!(
        attestation_set_digest,
        musubi_provider_bundle_attestation_set_digest_v1(
            ArchiveId::new([0xFA; 32]),
            binding.replication_order,
            &[reference],
        )
        .expect("different archive remains a valid commitment")
    );
    assert_ne!(
        attestation_set_digest,
        musubi_provider_bundle_attestation_set_digest_v1(
            binding.archive_id,
            ReplicationOrderId::new([0xFB; 32]),
            &[reference],
        )
        .expect("different order remains a valid commitment")
    );
    assert!(
        musubi_provider_bundle_attestation_set_digest_v1(
            ArchiveId::new([0; 32]),
            binding.replication_order,
            &[reference],
        )
        .is_err()
    );
    let record = MusubiProviderBundleAttestationRecordV1 {
        key: attestation.key(),
        attestation_digest: attestation.digest(),
        attestation: attestation.clone(),
        registered_by: binding.completed_by.clone(),
        registered_at_height: 78,
    };
    record
        .validate()
        .expect("exact provider attestation record");
    let mut mismatched_record = record.clone();
    mismatched_record.attestation_digest = MusubiProviderBundleAttestationDigestV1::new([0xFC; 32]);
    assert!(mismatched_record.validate().is_err());
    let mut location = MusubiArchiveLocationV1 {
        location_id: MusubiArchiveLocationIdV1::new([0x31; 32]),
        archive_id: binding.archive_id,
        pin_manifest: ManifestDigest::new([0x32; 32]),
        replication_order: binding.replication_order,
        providers: vec![binding.provider_id],
        provider_attestation_set_digest: attestation_set_digest,
        renew_after_epoch: 10,
        expires_at_epoch: 20,
        finalized_height: 30,
        revision: 1,
        state: MusubiArchiveLocationStateV1::Healthy,
    };
    location.validate().expect("valid archive location");
    location.pin_manifest = ManifestDigest::new([0; 32]);
    let decoded = MusubiArchiveLocationV1::decode_all(&mut location.encode().as_slice())
        .expect("zero pin digest remains representable on the wire");
    assert!(
        decoded.validate().is_err(),
        "decoded archive location must reject a zero pin manifest"
    );
}
#[test]
fn structured_version_rejects_build_metadata_overflow_and_leading_zeroes() {
    assert!("1.2.3-alpha.1".parse::<MusubiVersionV1>().is_ok());
    assert!("1.2.3+local".parse::<MusubiVersionV1>().is_err());
    assert!(
        "18446744073709551616.0.0"
            .parse::<MusubiVersionV1>()
            .is_err()
    );
    assert!("1.02.3".parse::<MusubiVersionV1>().is_err());
    assert!("1.2.3-alpha.01".parse::<MusubiVersionV1>().is_err());
    let too_many =
        vec![MusubiPrereleaseIdentifierV1::Numeric(1); MUSUBI_MAX_PRERELEASE_IDENTIFIERS_V1 + 1];
    assert!(MusubiVersionV1::new(1, 0, 0, too_many).is_err());
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one cursor-ceiling matrix pins every structured Musubi V1 key family"
)]
fn finalized_cursor_ceiling_covers_every_structured_v1_key_family() {
    let maximum_version = MusubiVersionV1::new(
        u64::MAX,
        u64::MAX,
        u64::MAX,
        vec![
            MusubiPrereleaseIdentifierV1::AlphaNumeric(
                "a".repeat(MUSUBI_MAX_PRERELEASE_IDENTIFIER_BYTES_V1),
            );
            MUSUBI_MAX_PRERELEASE_IDENTIFIERS_V1
        ],
    )
    .expect("maximum bounded semantic version");
    let maximum_version_text = maximum_version.to_string();
    assert_eq!(
        maximum_version_text.len(),
        MUSUBI_MAX_VERSION_CURSOR_KEY_BYTES_V1
    );
    assert_eq!(
        maximum_version_text
            .parse::<MusubiVersionV1>()
            .expect("maximum semantic-version text reparses"),
        maximum_version
    );
    assert_eq!(
        MUSUBI_MAX_CURSOR_KEY_BYTES_V1,
        2 * MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1 + 1 + "pending-".len() + 64
    );
    assert_eq!(MUSUBI_MAX_MAINTAINER_CURSOR_KEY_BYTES_V1, 16_457);
    // This synthetic bare payload exercises the deliberately conservative
    // ceiling; it is not claimed to be an attainable AccountId encoding.
    let ceiling_sized_bare_account = vec![0xA5; MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1];
    let ceiling_pending_label = maintainer_cursor_key_label_v1(
        &ceiling_sized_bare_account,
        Some(&MusubiInviteIdV1::new([0x5A; 32])),
    );
    assert_eq!(
        ceiling_pending_label.len(),
        MUSUBI_MAX_MAINTAINER_CURSOR_KEY_BYTES_V1
    );
    assert!(ceiling_pending_label.ends_with(&format!("|pending-{}", "5a".repeat(32))));
    assert_eq!(
        maintainer_cursor_key_label_v1(&ceiling_sized_bare_account, None,).len(),
        2 * MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1 + 1 + "accepted".len()
    );
    let cursor_account = account(44);
    let accepted = MusubiMaintainerDirectoryEntryV1::Accepted(MusubiPackageMemberV1 {
        package: package("cursor-bound"),
        account: cursor_account.clone(),
        role: MusubiPackageRoleV1::Owner,
        accepted_at_height: 1,
        governance_revision: 1,
    });
    let pending =
        MusubiMaintainerDirectoryEntryV1::PendingInvitation(MusubiMaintainerInvitationV1 {
            invite_id: MusubiInviteIdV1::new([0x5B; 32]),
            package: package("cursor-bound"),
            invited_by: account(45),
            invited_account: cursor_account,
            role: MusubiPackageRoleV1::Owner,
            expected_governance_revision: 1,
            expires_at_height: 2,
            state: MusubiInvitationStateV1::Pending,
        });
    accepted.validate().expect("accepted cursor fixture");
    pending.validate().expect("pending cursor fixture");
    let accepted_key = accepted.cursor_key();
    let pending_key = pending.cursor_key();
    assert!(accepted_key.ends_with("|accepted"));
    assert!(pending_key.ends_with(&format!("|pending-{}", "5b".repeat(32))));
    assert_ne!(accepted_key, pending_key);
    assert!(maintainer_cursor_key_is_canonical_v1(&accepted_key));
    assert!(maintainer_cursor_key_is_canonical_v1(&pending_key));
    assert!(!maintainer_cursor_key_is_canonical_v1(&format!(
        "aa|pending-{}",
        "00".repeat(32)
    )));
    assert!(!maintainer_cursor_key_is_canonical_v1("AA|accepted"));
    let repeated_boundary = MusubiMaintainerPageV1 {
        query: MusubiPackagePageQueryV1 {
            package: package("cursor-bound"),
            page: MusubiPageRequestV1 {
                limit: 1,
                cursor: Some(MusubiFinalizedCursorV1 {
                    snapshot: snapshot(),
                    query_hash: MusubiQueryHashV1::new([0x74; 32]),
                    last_key: accepted_key,
                    caller: None,
                }),
            },
        },
        items: vec![accepted],
        next_cursor: None,
        snapshot: snapshot(),
    };
    assert!(
        repeated_boundary.validate().is_err(),
        "a maintainer response may not repeat its opaque request boundary"
    );
    for producer_bound in [
        MUSUBI_MAX_VERSION_CURSOR_KEY_BYTES_V1,
        MUSUBI_MAX_ORDERED_PREFIX_BYTES_V1,
        MUSUBI_MAX_ARCHIVE_LOCATION_CURSOR_KEY_BYTES_V1,
        MUSUBI_MAX_ALIAS_HISTORY_CURSOR_KEY_BYTES_V1,
        MUSUBI_MAX_MAINTAINER_CURSOR_KEY_BYTES_V1,
    ] {
        assert!(producer_bound <= MUSUBI_MAX_CURSOR_KEY_BYTES_V1);
    }
    MusubiFinalizedCursorV1 {
        snapshot: snapshot(),
        query_hash: MusubiQueryHashV1::new([0x71; 32]),
        last_key: maximum_version_text,
        caller: None,
    }
    .validate()
    .expect("the longest canonical semantic version is a valid cursor tail");
    MusubiFinalizedCursorV1 {
        snapshot: snapshot(),
        query_hash: MusubiQueryHashV1::new([0x72; 32]),
        last_key: "x".repeat(MUSUBI_MAX_CURSOR_KEY_BYTES_V1),
        caller: None,
    }
    .validate()
    .expect("the exact generic cursor boundary is accepted");
    assert!(
        MusubiFinalizedCursorV1 {
            snapshot: snapshot(),
            query_hash: MusubiQueryHashV1::new([0x73; 32]),
            last_key: "x".repeat(MUSUBI_MAX_CURSOR_KEY_BYTES_V1 + 1),
            caller: None,
        }
        .validate()
        .is_err()
    );
}
#[test]
fn maintainer_cursor_requires_an_exact_canonical_account_payload() {
    let entry = MusubiMaintainerDirectoryEntryV1::Accepted(MusubiPackageMemberV1 {
        package: package("canonical-cursor"),
        account: account(46),
        role: MusubiPackageRoleV1::Owner,
        accepted_at_height: 1,
        governance_revision: 1,
    });
    let canonical = entry.cursor_key();
    assert!(maintainer_cursor_key_is_canonical_v1(&canonical));
    let (encoded_account, suffix) = canonical
        .split_once('|')
        .expect("producer cursor contains its suffix separator");
    let truncated = format!(
        "{}|{suffix}",
        &encoded_account[..encoded_account.len().saturating_sub(2)]
    );
    let trailing_bytes = format!("{encoded_account}00|{suffix}");
    for invalid in ["00|accepted".to_owned(), truncated, trailing_bytes] {
        assert!(
            !maintainer_cursor_key_is_canonical_v1(&invalid),
            "malformed or noncanonical account payload survived: {invalid}"
        );
    }
}
#[test]
fn maintainer_cursor_rejects_noncanonical_multisig_wire() {
    let first = account(49)
        .controller()
        .single_signatory()
        .expect("single-key fixture")
        .clone();
    let second = account(50)
        .controller()
        .single_signatory()
        .expect("single-key fixture")
        .clone();
    let policy = MultisigPolicy::new(
        1,
        vec![
            MultisigMember::new(first.clone(), 1).expect("valid member"),
            MultisigMember::new(second.clone(), 1).expect("valid member"),
        ],
    )
    .expect("valid policy");
    let canonical_members = policy
        .members()
        .iter()
        .map(|member| (member.public_key().clone(), member.weight()))
        .collect::<Vec<_>>();
    assert!(maintainer_cursor_key_is_canonical_v1(
        &unchecked_multisig_cursor_key(1, 1, canonical_members.clone())
    ));
    let mut reversed_members = canonical_members.clone();
    reversed_members.reverse();
    let invalid = [
        (
            "unsupported version",
            unchecked_multisig_cursor_key(2, 1, vec![(first.clone(), 1)]),
        ),
        (
            "zero threshold",
            unchecked_multisig_cursor_key(1, 0, vec![(first.clone(), 1)]),
        ),
        (
            "zero weight",
            unchecked_multisig_cursor_key(1, 1, vec![(first.clone(), 0)]),
        ),
        (
            "threshold overflow",
            unchecked_multisig_cursor_key(1, 2, vec![(first.clone(), 1)]),
        ),
        (
            "duplicate key",
            unchecked_multisig_cursor_key(1, 1, vec![(first.clone(), 1), (first.clone(), 1)]),
        ),
        (
            "reversed member order",
            unchecked_multisig_cursor_key(1, 1, reversed_members),
        ),
    ];
    for (case, cursor) in invalid {
        assert!(
            !maintainer_cursor_key_is_canonical_v1(&cursor),
            "semantically noncanonical multisig wire survived: {case}"
        );
    }
}
#[test]
fn maintainer_page_rejects_opaque_boundary_repeated_after_first_item() {
    let package_id = package("repeated-cursor");
    let accepted = |seed| {
        MusubiMaintainerDirectoryEntryV1::Accepted(MusubiPackageMemberV1 {
            package: package_id.clone(),
            account: account(seed),
            role: MusubiPackageRoleV1::Owner,
            accepted_at_height: 1,
            governance_revision: 1,
        })
    };
    let mut items = vec![accepted(47), accepted(48)];
    items.sort_by_key(MusubiMaintainerDirectoryEntryV1::key);
    let repeated_boundary = items[1].cursor_key();
    assert_ne!(items[0].cursor_key(), repeated_boundary);
    let page = MusubiMaintainerPageV1 {
        query: MusubiPackagePageQueryV1 {
            package: package_id,
            page: MusubiPageRequestV1 {
                limit: 2,
                cursor: Some(MusubiFinalizedCursorV1 {
                    snapshot: snapshot(),
                    query_hash: MusubiQueryHashV1::new([0x75; 32]),
                    last_key: repeated_boundary,
                    caller: None,
                }),
            },
        },
        items,
        next_cursor: None,
        snapshot: snapshot(),
    };
    assert!(
        page.validate().is_err(),
        "an opaque request boundary may not recur later in the response page"
    );
}
#[test]
fn version_order_is_semver_order() {
    let alpha: MusubiVersionV1 = "1.0.0-alpha.2".parse().expect("alpha");
    let beta: MusubiVersionV1 = "1.0.0-alpha.10".parse().expect("beta");
    let release: MusubiVersionV1 = "1.0.0".parse().expect("release");
    assert!(alpha < beta);
    assert!(beta < release);
}
#[test]
fn requirements_parse_to_one_canonical_ast() {
    let bare: MusubiVersionReqV1 = "1.2.3".parse().expect("bare");
    assert_eq!(bare.to_string(), "^1.2.3");
    assert!(matches!(
        "=1.2.3".parse::<MusubiVersionReqV1>().expect("exact"),
        MusubiVersionReqV1::Exact(_)
    ));
    let ordered: MusubiVersionReqV1 = "<2.0.0, >=1.0.0,>=1.0.0".parse().expect("range");
    assert_eq!(ordered.to_string(), ">=1.0.0,<2.0.0");
    assert!(" ^1.2.3 ".parse::<MusubiVersionReqV1>().is_err());
    assert!(
        ">=1.0.0,=1.0.0,=1.1.0"
            .parse::<MusubiVersionReqV1>()
            .is_err()
    );
    let duplicate_exact: MusubiVersionReqV1 = "=1.2.3,=1.2.3".parse().expect("duplicate exact");
    assert!(matches!(duplicate_exact, MusubiVersionReqV1::Exact(_)));
    assert_eq!(duplicate_exact.to_string(), "=1.2.3");
    for raw in [
        "*",
        "1.2.3",
        "^0.2.3-alpha.1",
        "~1.2.3",
        "1.*",
        "1.2.*",
        "=1.2.3,=1.2.3",
        ">=1.2.3,<2.0.0,>=1.2.3",
    ] {
        let requirement: MusubiVersionReqV1 = raw.parse().expect("valid requirement");
        assert_eq!(
            requirement
                .to_string()
                .parse::<MusubiVersionReqV1>()
                .expect("canonical display reparses"),
            requirement,
            "requirement display must be a canonical AST fixed point for {raw}",
        );
    }
}
#[test]
fn requirements_apply_cargo_prerelease_eligibility() {
    let prerelease: MusubiVersionV1 = "1.2.3-beta.1".parse().expect("prerelease");
    let stable: MusubiVersionV1 = "1.2.3".parse().expect("stable");
    assert!(
        !"*".parse::<MusubiVersionReqV1>()
            .expect("any")
            .matches(&prerelease)
    );
    assert!(
        !"^1.2.0"
            .parse::<MusubiVersionReqV1>()
            .expect("caret")
            .matches(&prerelease)
    );
    assert!(
        "^1.2.3-alpha.1"
            .parse::<MusubiVersionReqV1>()
            .expect("prerelease caret")
            .matches(&prerelease)
    );
    assert!(
        "^1.2.3-alpha.1"
            .parse::<MusubiVersionReqV1>()
            .expect("prerelease caret")
            .matches(&stable)
    );
}
#[test]
fn requirements_keep_cargo_upper_bounds_at_u64_component_limits() {
    let maximum = u64::MAX;
    let zero_major: MusubiVersionReqV1 = format!("^0.{maximum}.0")
        .parse()
        .expect("zero-major caret at the minor limit");
    assert!(
        zero_major.matches(
            &format!("0.{maximum}.1")
                .parse()
                .expect("same compatible minor"),
        )
    );
    assert!(!zero_major.matches(&"1.0.0".parse().expect("next major")));
    let zero_minor: MusubiVersionReqV1 = format!("^0.0.{maximum}")
        .parse()
        .expect("zero-minor caret at the patch limit");
    assert!(
        zero_minor.matches(
            &format!("0.0.{maximum}")
                .parse()
                .expect("exact maximum patch"),
        )
    );
    assert!(!zero_minor.matches(&"0.1.0".parse().expect("next minor")));
    assert!(!zero_minor.matches(&"1.0.0".parse().expect("next major")));
    let tilde: MusubiVersionReqV1 = format!("~0.{maximum}.0")
        .parse()
        .expect("tilde at the minor limit");
    assert!(tilde.matches(&format!("0.{maximum}.1").parse().expect("same tilde minor"),));
    assert!(!tilde.matches(&"1.0.0".parse().expect("next tilde major")));
    let maximum_major: MusubiVersionReqV1 = format!("^{maximum}.0.0")
        .parse()
        .expect("caret at the major limit");
    assert!(
        maximum_major.matches(
            &format!("{maximum}.{maximum}.{maximum}")
                .parse()
                .expect("same maximum major"),
        )
    );
}
#[test]
fn requirement_validation_recurses_into_decoded_fields() {
    let invalid = MusubiVersionReqV1::Caret(MusubiVersionV1 {
        major: 1,
        minor: 0,
        patch: 0,
        prerelease: vec![MusubiPrereleaseIdentifierV1::AlphaNumeric("01".to_owned())],
    });
    assert!(invalid.validate().is_err());
    let noncanonical_exact = MusubiVersionReqV1::Comparators(vec![MusubiVersionComparatorV1 {
        op: MusubiComparatorOpV1::Equal,
        version: "1.0.0".parse().expect("exact comparator version"),
    }]);
    assert!(
        noncanonical_exact.validate().is_err(),
        "decoded singleton equality comparators must use the Exact variant",
    );
}
#[test]
fn archive_id_binds_every_canonical_commitment_field() {
    let archive = archive_commitment();
    archive.validate().expect("valid archive");
    let original = archive.archive_id();
    let mut changed = archive.clone();
    changed.car_size += 1;
    assert_ne!(original, changed.archive_id());
    let mut oversized = archive;
    oversized.content_length = MUSUBI_MAX_BUNDLE_PAYLOAD_BYTES_V1 + 1;
    assert!(oversized.validate().is_err());
    let mut source_boundary_plus_metadata = archive_commitment();
    source_boundary_plus_metadata.content_length = MUSUBI_MAX_SOURCE_PAYLOAD_BYTES_V1 + 1;
    source_boundary_plus_metadata
        .validate()
        .expect("bundle metadata fits above the source-only payload ceiling");
}
#[test]
fn archive_commitment_roundtrips_through_norito() {
    let archive = archive_commitment();
    let bytes = archive.encode();
    let mut cursor = bytes.as_slice();
    let decoded = MusubiArchiveCommitmentV1::decode(&mut cursor).expect("decode archive");
    assert!(cursor.is_empty());
    assert_eq!(decoded, archive);
    decoded.validate().expect("decoded archive validates");
}
#[test]
fn canonical_bundle_file_decoders_accept_exact_valid_metadata() {
    let (descriptor, semantic, lock) = canonical_bundle_metadata();
    assert_eq!(
        MusubiArtifactDescriptorV1::decode_canonical_bundle_file(&descriptor.encode())
            .expect("exact canonical descriptor"),
        descriptor
    );
    assert_eq!(
        MusubiSemanticReleaseManifestV1::decode_canonical_bundle_file(&semantic.encode())
            .expect("exact canonical semantic release"),
        semantic
    );
    assert_eq!(
        MusubiVerificationLockV1::decode_canonical_bundle_file(&lock.encode())
            .expect("exact canonical verification lock"),
        lock
    );
}
#[test]
fn canonical_bundle_file_size_gate_is_inclusive() {
    let (descriptor, semantic, lock) = canonical_bundle_metadata();
    let descriptor_bytes = descriptor.encode();
    let semantic_bytes = semantic.encode();
    let lock_bytes = lock.encode();
    assert_eq!(
        decode_canonical_bundle_file_v1(
            &descriptor_bytes,
            descriptor_bytes.len() as u64,
            MUSUBI_ARTIFACT_DESCRIPTOR_DECODE_LIMITS_V1,
            MusubiArtifactDescriptorV1::validate,
            "descriptor boundary fixture",
        )
        .expect("descriptor exactly at a caller-supplied byte cap"),
        descriptor
    );
    assert!(
        decode_canonical_bundle_file_v1(
            &descriptor_bytes,
            descriptor_bytes.len() as u64 - 1,
            MUSUBI_ARTIFACT_DESCRIPTOR_DECODE_LIMITS_V1,
            MusubiArtifactDescriptorV1::validate,
            "descriptor boundary fixture",
        )
        .is_err(),
        "descriptor one byte above a caller-supplied cap must fail"
    );
    assert_eq!(
        decode_canonical_bundle_file_v1(
            &semantic_bytes,
            semantic_bytes.len() as u64,
            MUSUBI_SEMANTIC_RELEASE_DECODE_LIMITS_V1,
            MusubiSemanticReleaseManifestV1::validate,
            "semantic boundary fixture",
        )
        .expect("semantic release exactly at a caller-supplied byte cap"),
        semantic
    );
    assert!(
        decode_canonical_bundle_file_v1(
            &semantic_bytes,
            semantic_bytes.len() as u64 - 1,
            MUSUBI_SEMANTIC_RELEASE_DECODE_LIMITS_V1,
            MusubiSemanticReleaseManifestV1::validate,
            "semantic boundary fixture",
        )
        .is_err(),
        "semantic release one byte above a caller-supplied cap must fail"
    );
    assert_eq!(
        decode_canonical_bundle_file_v1(
            &lock_bytes,
            lock_bytes.len() as u64,
            MUSUBI_VERIFICATION_LOCK_DECODE_LIMITS_V1,
            MusubiVerificationLockV1::validate,
            "verification-lock boundary fixture",
        )
        .expect("verification lock exactly at a caller-supplied byte cap"),
        lock
    );
    assert!(
        decode_canonical_bundle_file_v1(
            &lock_bytes,
            lock_bytes.len() as u64 - 1,
            MUSUBI_VERIFICATION_LOCK_DECODE_LIMITS_V1,
            MusubiVerificationLockV1::validate,
            "verification-lock boundary fixture",
        )
        .is_err(),
        "verification lock one byte above a caller-supplied cap must fail"
    );
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one large-lock regression covers aligned, misaligned, measured-allocation, and byte-limit decoding"
)]
fn canonical_lock_decoder_accepts_large_aligned_and_misaligned_metadata() {
    let maximum = usize::try_from(MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_V1)
        .expect("Musubi metadata byte cap fits usize");
    let mut lower_fanout = 1;
    let mut upper_fanout = 64;
    let mut largest = None;
    while lower_fanout <= upper_fanout {
        let fanout = lower_fanout + (upper_fanout - lower_fanout) / 2;
        let lock = dense_verification_lock(fanout);
        let bytes = lock.encode();
        if bytes.len() <= maximum {
            largest = Some((lock, bytes));
            lower_fanout = fanout + 1;
        } else {
            upper_fanout = fanout - 1;
        }
    }
    let (lock, bytes) = largest.expect("at least the one-edge dense lock fits the file cap");
    assert_eq!(bytes.len(), 2_056_570, "pin the reviewed dense-lock shape");
    assert!(
        bytes.len() >= maximum * 3 / 4,
        "dense producer-reachable fixture should exercise the actual 2 MiB corridor"
    );
    lock.validate().expect("large producer-reachable lock");
    let alignment = norito::core::archived_payload_align::<MusubiVerificationLockV1>();
    assert!(
        alignment > 1,
        "the lock type must have a testable alignment"
    );
    let mut aligned_storage = vec![0_u8; bytes.len() + alignment];
    let aligned_offset = (0..alignment)
        .find(|offset| (aligned_storage.as_ptr() as usize + offset).is_multiple_of(alignment))
        .expect("one offset within an alignment span is aligned");
    aligned_storage[aligned_offset..aligned_offset + bytes.len()].copy_from_slice(&bytes);
    let aligned = &aligned_storage[aligned_offset..aligned_offset + bytes.len()];
    assert_eq!(aligned.as_ptr() as usize % alignment, 0);
    assert_eq!(
        MusubiVerificationLockV1::decode_canonical_bundle_file(aligned)
            .expect("decode large aligned lock"),
        lock
    );
    let mut misaligned_storage = vec![0_u8; bytes.len() + alignment];
    let misaligned_offset = (0..alignment)
        .find(|offset| !(misaligned_storage.as_ptr() as usize + offset).is_multiple_of(alignment))
        .expect("one offset within an alignment span is misaligned");
    misaligned_storage[misaligned_offset..misaligned_offset + bytes.len()].copy_from_slice(&bytes);
    let misaligned = &misaligned_storage[misaligned_offset..misaligned_offset + bytes.len()];
    assert_ne!(misaligned.as_ptr() as usize % alignment, 0);
    let measurement_limits =
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, usize::MAX);
    let (decoded, usage) = norito::core::with_decode_limits_measured(measurement_limits, || {
        decode_canonical_bundle_file_v1(
            misaligned,
            MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_V1,
            MUSUBI_VERIFICATION_LOCK_DECODE_LIMITS_V1,
            MusubiVerificationLockV1::validate,
            "dense-lock measured allocation fixture",
        )
    });
    assert_eq!(
        decoded.expect("measure the large lock's exact allocation charge"),
        lock
    );
    let exact_allocation = usage.total_allocated_bytes();
    assert!(
        exact_allocation > 0,
        "the dense lock must charge allocations"
    );
    let exact_limits = norito::DecodeLimits::new(1_024, maximum, 8_000_000, exact_allocation, 64);
    assert_eq!(
        decode_canonical_bundle_file_v1(
            misaligned,
            MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_V1,
            exact_limits,
            MusubiVerificationLockV1::validate,
            "dense-lock measured allocation fixture",
        )
        .expect("the exact measured allocation budget decodes the large lock"),
        lock
    );
    let below_measured_minimum_limits =
        norito::DecodeLimits::new(1_024, maximum, 8_000_000, exact_allocation - 1, 64);
    assert!(
        decode_canonical_bundle_file_v1(
            misaligned,
            MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_V1,
            below_measured_minimum_limits,
            MusubiVerificationLockV1::validate,
            "dense-lock measured allocation fixture",
        )
        .is_err(),
        "one byte below the measured allocation requirement must fail"
    );
    assert_eq!(
        MUSUBI_BUNDLE_METADATA_DECODE_MAX_ALLOCATED_BYTES_V1,
        48 * 1024 * 1024,
        "pin the reviewed production allocation corridor"
    );
    assert!(
        MUSUBI_BUNDLE_METADATA_DECODE_MAX_ALLOCATED_BYTES_V1 - exact_allocation >= 5 * 1024 * 1024,
        "the production corridor retains at least 5 MiB above the measured minimum"
    );
    assert_eq!(
        MusubiVerificationLockV1::decode_canonical_bundle_file(misaligned)
            .expect("production corridor decodes the misaligned dense lock"),
        lock
    );
}
#[test]
fn canonical_bundle_file_decoders_restore_ambient_norito_state() {
    let (descriptor, semantic, lock) = canonical_bundle_metadata();
    let descriptor_bytes = descriptor.encode();
    let semantic_bytes = semantic.encode();
    let lock_bytes = lock.encode();
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let alternate_semantic = {
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        let mut bytes = Vec::new();
        norito::core::serialize_to_buffer(&semantic, &mut bytes)
            .expect("encode alternate bare semantic fixture");
        bytes
    };
    assert_ne!(alternate_semantic, semantic_bytes);
    let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
    let ambient_payload = b"ambient Musubi bundle payload";
    let _ambient_payload = norito::core::PayloadCtxGuard::enter(ambient_payload);
    let flags_before = norito::core::effective_decode_flags();
    let payload_context_before = norito::core::payload_ctx();
    let ambient_probe = vec!["first".to_owned(), "second".to_owned()];
    let mut encoding_before = Vec::new();
    norito::core::serialize_to_buffer(&ambient_probe, &mut encoding_before)
        .expect("encode bare ambient probe");
    assert_eq!(
        MusubiArtifactDescriptorV1::decode_canonical_bundle_file(&descriptor_bytes)
            .expect("decode canonical descriptor under alternate ambient state"),
        descriptor
    );
    assert_eq!(
        MusubiSemanticReleaseManifestV1::decode_canonical_bundle_file(&semantic_bytes)
            .expect("decode canonical semantic release under alternate ambient state"),
        semantic
    );
    assert_eq!(
        MusubiVerificationLockV1::decode_canonical_bundle_file(&lock_bytes)
            .expect("decode canonical verification lock under alternate ambient state"),
        lock
    );
    assert!(
        MusubiSemanticReleaseManifestV1::decode_canonical_bundle_file(&alternate_semantic).is_err(),
        "the caller's alternate flags must not make alternate-layout bytes canonical"
    );
    assert_eq!(norito::core::effective_decode_flags(), flags_before);
    assert_eq!(norito::core::payload_ctx(), payload_context_before);
    let mut encoding_after = Vec::new();
    norito::core::serialize_to_buffer(&ambient_probe, &mut encoding_after)
        .expect("re-encode bare ambient probe");
    assert_eq!(encoding_after, encoding_before);
}
#[test]
fn canonical_bundle_file_decoders_reject_empty_trailing_and_oversized_inputs() {
    let (descriptor, semantic, lock) = canonical_bundle_metadata();
    let cases = [
        (
            MusubiArtifactDescriptorV1::decode_canonical_bundle_file(&[])
                .expect_err("empty descriptor must fail")
                .reason(),
            "Musubi artifact descriptor bundle file is invalid or out of bounds",
        ),
        (
            MusubiSemanticReleaseManifestV1::decode_canonical_bundle_file(&[])
                .expect_err("empty semantic release must fail")
                .reason(),
            "Musubi semantic release bundle file is invalid or out of bounds",
        ),
        (
            MusubiVerificationLockV1::decode_canonical_bundle_file(&[])
                .expect_err("empty verification lock must fail")
                .reason(),
            "Musubi verification lock bundle file is invalid or out of bounds",
        ),
    ];
    for (actual, expected) in cases {
        assert_eq!(actual, expected);
    }
    let mut trailing_descriptor = descriptor.encode();
    trailing_descriptor.push(0);
    assert!(
        MusubiArtifactDescriptorV1::decode_canonical_bundle_file(&trailing_descriptor).is_err()
    );
    let mut trailing_semantic = semantic.encode();
    trailing_semantic.push(0);
    assert!(
        MusubiSemanticReleaseManifestV1::decode_canonical_bundle_file(&trailing_semantic).is_err()
    );
    let mut trailing_lock = lock.encode();
    trailing_lock.push(0);
    assert!(MusubiVerificationLockV1::decode_canonical_bundle_file(&trailing_lock).is_err());
    let oversized_descriptor = vec![
        0;
        usize::try_from(MUSUBI_MAX_ARTIFACT_DESCRIPTOR_BYTES_V1)
            .expect("Musubi artifact descriptor byte cap fits usize")
            + 1
    ]
    .into_boxed_slice();
    assert!(
        MusubiArtifactDescriptorV1::decode_canonical_bundle_file(&oversized_descriptor).is_err()
    );
    let oversized_metadata = vec![
        0;
        usize::try_from(MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_V1)
            .expect("Musubi metadata byte cap fits usize")
            + 1
    ]
    .into_boxed_slice();
    assert!(
        MusubiSemanticReleaseManifestV1::decode_canonical_bundle_file(&oversized_metadata).is_err()
    );
    assert!(MusubiVerificationLockV1::decode_canonical_bundle_file(&oversized_metadata).is_err());
}
#[test]
fn canonical_bundle_file_decoders_reject_noncanonical_values_and_length_bombs() {
    let (_, mut semantic, _) = canonical_bundle_metadata();
    semantic.exports = vec![
        "zeta".parse().expect("export"),
        "alpha".parse().expect("export"),
    ];
    assert!(
        MusubiSemanticReleaseManifestV1::decode_canonical_bundle_file(&semantic.encode()).is_err(),
        "a structurally decoded but noncanonical semantic value must fail validation"
    );
    let first = release("alpha-dependency", "1.0.0");
    let second = release("zeta-dependency", "1.0.0");
    let edge = |alias: &str, selected: MusubiReleaseIdV1| MusubiExactDependencyEdgeV1 {
        alias: alias.parse().expect("dependency alias"),
        kind: MusubiDependencyKindV1::Normal,
        package: selected.package.clone(),
        requirement: "^1.0.0".parse().expect("dependency requirement"),
        selected,
    };
    let node = |release: MusubiReleaseIdV1, fill: u8| MusubiVerificationNodeV1 {
        release,
        release_digest: MusubiReleaseDigestV1::new([fill; 32]),
        archive_id: ArchiveId::new([fill.saturating_add(1); 32]),
        source_digest: MusubiContentDigestV1::new([fill.saturating_add(2); 32]),
        interface_digest: MusubiContentDigestV1::new([fill.saturating_add(3); 32]),
        abi: MusubiAbiBindingV1::new([fill.saturating_add(4); 32]).expect("ABI"),
        dependencies: Vec::new(),
    };
    let mut lock = MusubiVerificationLockV1 {
        schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
        version: MUSUBI_REGISTRY_VERSION_V1,
        root: release("root", "1.0.0"),
        root_dependencies: vec![edge("alpha", first.clone()), edge("zeta", second.clone())],
        nodes: vec![node(first, 10), node(second, 20)],
    };
    lock.canonicalize();
    lock.validate().expect("canonical two-node lock");
    lock.nodes.reverse();
    assert!(
        MusubiVerificationLockV1::decode_canonical_bundle_file(&lock.encode()).is_err(),
        "a structurally decoded lock with noncanonical typed node order must fail validation"
    );
    // A tiny hostile payload consisting of maximal length words must be rejected under the
    // payload-derived and schema-specific limits without honoring any declared allocation.
    let declared_length_bomb = [u8::MAX; 32];
    assert!(
        MusubiArtifactDescriptorV1::decode_canonical_bundle_file(&declared_length_bomb).is_err()
    );
    assert!(
        MusubiSemanticReleaseManifestV1::decode_canonical_bundle_file(&declared_length_bomb)
            .is_err()
    );
    assert!(MusubiVerificationLockV1::decode_canonical_bundle_file(&declared_length_bomb).is_err());
}
#[test]
fn release_manifest_and_publication_proof_are_bound() {
    let manifest = release_manifest();
    manifest.validate().expect("valid manifest");
    let lock = verification_lock(manifest.release.clone());
    let publication = MusubiPublicationV1 {
        manifest: manifest.clone(),
        resolution: MusubiResolutionProofV1 {
            snapshot: snapshot(),
            lock,
        },
    };
    publication.validate().expect("valid publication");
    let bytes = manifest.encode();
    let mut cursor = bytes.as_slice();
    let decoded = MusubiReleaseManifestV1::decode(&mut cursor).expect("decode release");
    assert!(cursor.is_empty());
    assert_eq!(decoded, manifest);
    assert_eq!(decoded.release_digest(), manifest.release_digest());
    let semantic = manifest.semantic_manifest();
    semantic.validate().expect("semantic projection validates");
    assert_eq!(semantic.semantic_digest(), manifest.semantic_digest());
    let mut different_archive = manifest.clone();
    different_archive.archive_id = ArchiveId::new([0xFE; 32]);
    assert_eq!(
        different_archive.semantic_digest(),
        manifest.semantic_digest()
    );
    assert_ne!(
        different_archive.release_digest(),
        manifest.release_digest()
    );
}
#[test]
fn publication_binds_each_root_requirement_to_one_exact_node() {
    let dependency_package = package("codec");
    let selected = MusubiReleaseIdV1::new(
        dependency_package.clone(),
        "1.2.0".parse().expect("selected version"),
    );
    let parallel = MusubiReleaseIdV1::new(
        dependency_package.clone(),
        "1.3.0".parse().expect("parallel version"),
    );
    let node = |release: MusubiReleaseIdV1, fill: u8| MusubiVerificationNodeV1 {
        release,
        release_digest: MusubiReleaseDigestV1::new([fill; 32]),
        archive_id: ArchiveId::new([fill.saturating_add(1); 32]),
        source_digest: MusubiContentDigestV1::new([fill.saturating_add(2); 32]),
        interface_digest: MusubiContentDigestV1::new([fill.saturating_add(3); 32]),
        abi: MusubiAbiBindingV1::new([fill.saturating_add(4); 32]).expect("ABI"),
        dependencies: Vec::new(),
    };
    let dependency = MusubiDependencyReqV1 {
        alias: "codec".parse().expect("alias"),
        package: dependency_package.clone(),
        requirement: "^1.0.0".parse().expect("requirement"),
    };
    let parallel_dependency = MusubiDependencyReqV1 {
        alias: "codec-next".parse().expect("parallel alias"),
        package: dependency_package.clone(),
        requirement: "^1.3.0".parse().expect("parallel requirement"),
    };
    let exact = MusubiExactDependencyEdgeV1 {
        alias: dependency.alias.clone(),
        kind: MusubiDependencyKindV1::Normal,
        package: dependency.package.clone(),
        requirement: dependency.requirement.clone(),
        selected: selected.clone(),
    };
    let parallel_exact = MusubiExactDependencyEdgeV1 {
        alias: parallel_dependency.alias.clone(),
        kind: MusubiDependencyKindV1::Normal,
        package: parallel_dependency.package.clone(),
        requirement: parallel_dependency.requirement.clone(),
        selected: parallel.clone(),
    };
    let mut lock = MusubiVerificationLockV1 {
        schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
        version: MUSUBI_REGISTRY_VERSION_V1,
        root: release("swap-core", "1.2.3"),
        root_dependencies: vec![exact, parallel_exact],
        nodes: vec![node(parallel, 20), node(selected, 10)],
    };
    lock.canonicalize();
    lock.validate().expect("exact root selection validates");
    let mut manifest = release_manifest();
    manifest.dependencies = vec![dependency, parallel_dependency];
    manifest.verification_lock_digest = lock.digest();
    let mut publication = MusubiPublicationV1 {
        manifest,
        resolution: MusubiResolutionProofV1 {
            snapshot: snapshot(),
            lock,
        },
    };
    publication
        .validate()
        .expect("one exact direct selection is unambiguous");
    publication.manifest.dependencies[0].requirement =
        "^1.1.0".parse().expect("different compatible requirement");
    publication
        .manifest
        .validate()
        .expect("the changed manifest remains independently valid");
    publication
        .resolution
        .validate()
        .expect("the exact lock remains independently valid");
    assert!(publication.validate().is_err());
}
#[test]
fn exact_graph_rejects_cycles() {
    let first = release("first", "1.0.0");
    let second = release("second", "1.0.0");
    let edge = |alias: &str, selected: MusubiReleaseIdV1| MusubiExactDependencyEdgeV1 {
        alias: alias.parse().expect("alias"),
        kind: MusubiDependencyKindV1::Normal,
        package: selected.package.clone(),
        requirement: "^1.0.0".parse().expect("requirement"),
        selected,
    };
    let node = |release: MusubiReleaseIdV1, dependency: MusubiExactDependencyEdgeV1| {
        MusubiVerificationNodeV1 {
            release,
            release_digest: MusubiReleaseDigestV1::new([1; 32]),
            archive_id: ArchiveId::new([2; 32]),
            source_digest: MusubiContentDigestV1::new([3; 32]),
            interface_digest: MusubiContentDigestV1::new([4; 32]),
            abi: MusubiAbiBindingV1::new([5; 32]).expect("ABI"),
            dependencies: vec![dependency],
        }
    };
    let mut lock = MusubiVerificationLockV1 {
        schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
        version: 1,
        root: release("root", "1.0.0"),
        root_dependencies: vec![edge("first", first.clone())],
        nodes: vec![
            node(first.clone(), edge("second", second.clone())),
            node(second, edge("first", first)),
        ],
    };
    lock.canonicalize();
    assert!(lock.validate().is_err());
}
#[test]
fn exact_graph_rejects_unreachable_nodes() {
    let orphan = release("orphan", "1.0.0");
    let lock = MusubiVerificationLockV1 {
        schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
        version: MUSUBI_REGISTRY_VERSION_V1,
        root: release("root", "1.0.0"),
        root_dependencies: Vec::new(),
        nodes: vec![MusubiVerificationNodeV1 {
            release: orphan,
            release_digest: MusubiReleaseDigestV1::new([1; 32]),
            archive_id: ArchiveId::new([2; 32]),
            source_digest: MusubiContentDigestV1::new([3; 32]),
            interface_digest: MusubiContentDigestV1::new([4; 32]),
            abi: MusubiAbiBindingV1::new([5; 32]).expect("ABI"),
            dependencies: Vec::new(),
        }],
    };
    let error = lock
        .validate()
        .expect_err("unreachable exact nodes must be rejected");
    assert!(error.to_string().contains("unreachable exact nodes"));
}
#[test]
fn verification_lock_rejects_root_in_exact_nodes() {
    let root = release("root", "1.0.0");
    let mut lock = MusubiVerificationLockV1 {
        schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
        version: MUSUBI_REGISTRY_VERSION_V1,
        root: root.clone(),
        root_dependencies: vec![MusubiExactDependencyEdgeV1 {
            alias: "root".parse().expect("alias"),
            kind: MusubiDependencyKindV1::Normal,
            package: root.package.clone(),
            requirement: "^1.0.0".parse().expect("requirement"),
            selected: root.clone(),
        }],
        nodes: vec![MusubiVerificationNodeV1 {
            release: root,
            release_digest: MusubiReleaseDigestV1::new([1; 32]),
            archive_id: ArchiveId::new([2; 32]),
            source_digest: MusubiContentDigestV1::new([3; 32]),
            interface_digest: MusubiContentDigestV1::new([4; 32]),
            abi: MusubiAbiBindingV1::new([5; 32]).expect("ABI"),
            dependencies: Vec::new(),
        }],
    };
    lock.canonicalize();
    let error = lock
        .validate()
        .expect_err("the verification root cannot also be an exact node");
    assert!(error.to_string().contains("invalid or noncanonical"));
}
#[test]
fn verification_nodes_reject_development_dependencies() {
    let parent = release("parent", "1.0.0");
    let child = release("child", "1.0.0");
    let edge = |alias: &str, kind: MusubiDependencyKindV1, selected: MusubiReleaseIdV1| {
        MusubiExactDependencyEdgeV1 {
            alias: alias.parse().expect("alias"),
            kind,
            package: selected.package.clone(),
            requirement: "^1.0.0".parse().expect("requirement"),
            selected,
        }
    };
    let node = |release: MusubiReleaseIdV1,
                dependencies: Vec<MusubiExactDependencyEdgeV1>,
                fill: u8| MusubiVerificationNodeV1 {
        release,
        release_digest: MusubiReleaseDigestV1::new([fill; 32]),
        archive_id: ArchiveId::new([fill.saturating_add(1); 32]),
        source_digest: MusubiContentDigestV1::new([fill.saturating_add(2); 32]),
        interface_digest: MusubiContentDigestV1::new([fill.saturating_add(3); 32]),
        abi: MusubiAbiBindingV1::new([fill.saturating_add(4); 32]).expect("ABI"),
        dependencies,
    };
    let mut lock = MusubiVerificationLockV1 {
        schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
        version: MUSUBI_REGISTRY_VERSION_V1,
        root: release("root", "1.0.0"),
        root_dependencies: vec![edge(
            "parent",
            MusubiDependencyKindV1::Normal,
            parent.clone(),
        )],
        nodes: vec![
            node(
                parent,
                vec![edge(
                    "child",
                    MusubiDependencyKindV1::Development,
                    child.clone(),
                )],
                10,
            ),
            node(child, Vec::new(), 20),
        ],
    };
    lock.canonicalize();
    let error = lock
        .validate()
        .expect_err("transitive development edges must be rejected");
    assert!(error.to_string().contains("verification node"));
}
#[test]
fn parent_local_dependency_aliases_are_unique_across_wire_surfaces() {
    let first_package = package("first-dependency");
    let second_package = package("second-dependency");
    let first_release = MusubiReleaseIdV1::new(
        first_package.clone(),
        "1.1.0".parse().expect("first dependency version"),
    );
    let second_release = MusubiReleaseIdV1::new(
        second_package.clone(),
        "1.2.0".parse().expect("second dependency version"),
    );
    let requirement: MusubiVersionReqV1 = "^1.0.0".parse().expect("dependency requirement");
    let dependency = |package: MusubiPackageIdV1| MusubiDependencyReqV1 {
        alias: "shared".parse().expect("shared dependency alias"),
        package,
        requirement: requirement.clone(),
    };
    let exact = |release: MusubiReleaseIdV1| MusubiExactDependencyEdgeV1 {
        alias: "shared".parse().expect("shared dependency alias"),
        kind: MusubiDependencyKindV1::Normal,
        package: release.package.clone(),
        requirement: requirement.clone(),
        selected: release,
    };
    let node = |release: MusubiReleaseIdV1, fill: u8| MusubiVerificationNodeV1 {
        release,
        release_digest: MusubiReleaseDigestV1::new([fill; 32]),
        archive_id: ArchiveId::new([fill.saturating_add(1); 32]),
        source_digest: MusubiContentDigestV1::new([fill.saturating_add(2); 32]),
        interface_digest: MusubiContentDigestV1::new([fill.saturating_add(3); 32]),
        abi: MusubiAbiBindingV1::new([fill.saturating_add(4); 32]).expect("ABI"),
        dependencies: Vec::new(),
    };
    let mut semantic = release_manifest().semantic_manifest();
    semantic.dependencies = vec![
        dependency(first_package.clone()),
        dependency(second_package.clone()),
    ];
    semantic.dependencies.sort();
    assert!(
        semantic.validate().is_err(),
        "semantic dependencies must not reuse a parent-local alias"
    );
    let mut parent_node = node(release("parent", "1.0.0"), 10);
    parent_node.dependencies = vec![exact(first_release.clone()), exact(second_release.clone())];
    parent_node.dependencies.sort();
    assert!(
        parent_node.validate().is_err(),
        "transitive exact edges must not reuse a parent-local alias"
    );
    let mut lock = MusubiVerificationLockV1 {
        schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
        version: MUSUBI_REGISTRY_VERSION_V1,
        root: release("root", "1.0.0"),
        root_dependencies: vec![exact(first_release.clone()), exact(second_release.clone())],
        nodes: vec![node(first_release, 20), node(second_release, 30)],
    };
    lock.canonicalize();
    assert!(
        lock.validate().is_err(),
        "verification roots must not reuse a parent-local alias"
    );
    let mut row = resolver_row("2.0.0");
    row.dependencies = vec![dependency(first_package), dependency(second_package)];
    row.dependencies.sort();
    assert!(
        row.validate().is_err(),
        "resolver rows must not retain an ambiguous dependency alias"
    );
}
