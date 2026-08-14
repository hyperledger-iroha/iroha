//! Musubi V1 registry public-contract integration coverage.
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::BlockHeader,
    isi::musubi::{
        PublishMusubiReleaseV1, RegisterMusubiArchiveV1, RegisterMusubiNamespaceBindingV1,
        RemoveMusubiPackageMaintainerV1, SetMusubiReleaseYankV1,
    },
    musubi::{
        ArchiveId, MUSUBI_REGISTRY_VERSION_V1, MusubiAbiBindingV1, MusubiAliasNameV1,
        MusubiAliasQueryV1, MusubiArchiveCommitmentV1, MusubiArchiveLocationQueryV1,
        MusubiContentDigestV1, MusubiDependencyKindV1, MusubiDependencyReqV1,
        MusubiExactDependencyEdgeV1, MusubiExactPackageQueryV1, MusubiExactReleaseQueryV1,
        MusubiFinalizedCursorV1, MusubiKotodamaEditionV1, MusubiNamespaceBindingV1,
        MusubiNamespaceDelegationApprovalV1, MusubiNamespaceDelegationPayloadV1,
        MusubiNamespaceDelegationV1, MusubiOrderedPrefixQueryV1, MusubiOrderedPrefixV1,
        MusubiPackageIdV1, MusubiPackagePageQueryV1, MusubiPackageRecordV1,
        MusubiPackageRevisionsV1, MusubiPackageScopeV1, MusubiPageRequestV1, MusubiPublicationV1,
        MusubiQueryHashV1, MusubiReasonV1, MusubiRegistrySnapshotV1, MusubiReleaseIdV1,
        MusubiReleaseManifestV1, MusubiReleaseMetadataV1, MusubiResolutionProofV1,
        MusubiResolverIndexQueryV1, MusubiSeedIngressReceiptApprovalV1,
        MusubiSeedIngressReceiptBindingV1, MusubiSeedIngressReceiptPayloadV1,
        MusubiSeedIngressReceiptV1, MusubiVerificationLockV1, MusubiVerificationNodeV1,
        MusubiVersionReqV1,
    },
    nexus::DataSpaceId,
    query::musubi::prelude::{
        FindMusubiAliasHistoryV1, FindMusubiAliasV1, FindMusubiArchiveLocationsV1,
        FindMusubiExactPackageV1, FindMusubiExactReleaseV1, FindMusubiMaintainersV1,
        FindMusubiOrderedPrefixV1, FindMusubiResolverIndexV1, FindMusubiVersionsV1,
    },
    sorafs::{
        capacity::ProviderId,
        pin_registry::{ChunkerProfileHandle, ManifestRootCid},
    },
};
use norito::codec::{DecodeAll, Encode};
fn keypair(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("fixture seed derives an Ed25519 keypair")
}
fn account(seed: u8) -> AccountId {
    AccountId::new(keypair(seed).public_key().clone())
}
fn package(name: &str) -> MusubiPackageIdV1 {
    MusubiPackageIdV1::new(
        DataSpaceId::new(7),
        MusubiPackageScopeV1::Domain("dex".parse().expect("domain")),
        name.parse().expect("package name"),
    )
}
fn release(name: &str, version: &str) -> MusubiReleaseIdV1 {
    MusubiReleaseIdV1::new(package(name), version.parse().expect("version"))
}
fn namespace_binding() -> MusubiNamespaceBindingV1 {
    MusubiNamespaceBindingV1 {
        namespace: "dex.universal".parse().expect("namespace"),
        home_dataspace: DataSpaceId::new(7),
        scope: MusubiPackageScopeV1::Domain("dex".parse().expect("domain")),
        generation: 4,
    }
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
fn snapshot() -> MusubiRegistrySnapshotV1 {
    MusubiRegistrySnapshotV1 {
        finalized_height: 42,
        finalized_block_hash: [0x42; 32],
        index_revision: 3,
    }
}
fn staging_receipt(
    commitment: &MusubiArchiveCommitmentV1,
    semantic_release_manifest_digest: iroha_data_model::musubi::MusubiSemanticReleaseDigestV1,
) -> MusubiSeedIngressReceiptV1 {
    let broker_keypair = keypair(70);
    let binding = MusubiSeedIngressReceiptBindingV1 {
        network_id: NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0x71; 32]),
        )),
        publisher: account(71),
        ingress_broker: AccountId::new(broker_keypair.public_key().clone()),
        seed_provider: ProviderId::new([0x72; 32]),
        semantic_release_manifest_digest,
        archive_id: commitment.archive_id(),
        car_body_digest: commitment.car_digest,
        car_body_length: commitment.car_size,
        nonce: [0x73; 32],
    };
    let payload = MusubiSeedIngressReceiptPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        binding,
        issued_at_ms: 1_000,
        expires_at_ms: 2_000,
    };
    MusubiSeedIngressReceiptV1 {
        approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
            public_key: broker_keypair.public_key().clone(),
            signature: SignatureOf::try_from_hash(
                broker_keypair.private_key(),
                payload.signing_hash(),
            )
            .expect("sign staging receipt"),
        }],
        payload,
    }
}
fn publication() -> MusubiPublicationV1 {
    let root = release("swap", "1.2.3");
    let dependency_release = release("math", "1.1.0");
    let requirement: MusubiVersionReqV1 = "^1.0.0".parse().expect("requirement");
    let exact = MusubiExactDependencyEdgeV1 {
        alias: "math".parse().expect("alias"),
        kind: MusubiDependencyKindV1::Normal,
        package: dependency_release.package.clone(),
        requirement: requirement.clone(),
        selected: dependency_release.clone(),
    };
    let dependency_node = MusubiVerificationNodeV1 {
        release: dependency_release,
        release_digest: iroha_data_model::musubi::MusubiReleaseDigestV1::new([8; 32]),
        archive_id: ArchiveId::new([9; 32]),
        source_digest: MusubiContentDigestV1::new([10; 32]),
        interface_digest: MusubiContentDigestV1::new([11; 32]),
        abi: MusubiAbiBindingV1::new([12; 32]).expect("ABI"),
        dependencies: Vec::new(),
    };
    let lock = MusubiVerificationLockV1 {
        schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
        version: MUSUBI_REGISTRY_VERSION_V1,
        root: root.clone(),
        root_dependencies: vec![exact],
        nodes: vec![dependency_node],
    };
    let manifest = MusubiReleaseManifestV1 {
        release: root,
        edition: MusubiKotodamaEditionV1::V1,
        abi: MusubiAbiBindingV1::new([13; 32]).expect("ABI"),
        dependencies: vec![MusubiDependencyReqV1 {
            alias: "math".parse().expect("alias"),
            package: package("math"),
            requirement,
        }],
        exports: vec!["quote".parse().expect("export")],
        interface_digest: MusubiContentDigestV1::new([14; 32]),
        metadata: MusubiReleaseMetadataV1::default(),
        archive_id: archive_commitment().archive_id(),
        verification_lock_digest: lock.digest(),
    };
    MusubiPublicationV1 {
        manifest,
        resolution: MusubiResolutionProofV1 {
            snapshot: snapshot(),
            lock,
        },
    }
}
fn roundtrip<T>(value: &T) -> T
where
    T: Encode + DecodeAll,
{
    T::decode_all(&mut value.encode().as_slice()).expect("Norito roundtrip")
}
#[test]
fn namespace_claim_uses_current_signed_owner_generation() {
    let binding = namespace_binding();
    binding
        .validate_authority_generation(4)
        .expect("current namespace ownership generation");
    let owner_keypair = keypair(41);
    let owner = AccountId::new(owner_keypair.public_key().clone());
    let delegate = account(42);
    let payload = MusubiNamespaceDelegationPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        namespace_binding: binding.digest(),
        owner_generation: 4,
        owner: owner.clone(),
        delegate: delegate.clone(),
        expires_at_height: 100,
    };
    let delegation = MusubiNamespaceDelegationV1 {
        approvals: vec![MusubiNamespaceDelegationApprovalV1 {
            public_key: owner_keypair.public_key().clone(),
            signature: SignatureOf::try_from_hash(
                owner_keypair.private_key(),
                payload.signing_hash(),
            )
            .expect("sign delegation"),
        }],
        payload,
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
    let register = RegisterMusubiNamespaceBindingV1::new(binding, 7);
    assert_eq!(roundtrip(&register), register);
    let publish = PublishMusubiReleaseV1::new(
        namespace_binding().namespace,
        publication(),
        Some(delegation),
        7,
        None,
    );
    assert_eq!(roundtrip(&publish), publish);
}
#[test]
fn archive_registration_and_publication_bind_the_exact_graph() {
    let commitment = archive_commitment();
    commitment.validate().expect("bounded archive commitment");
    let publication = publication();
    publication
        .validate()
        .expect("manifest and exact verification graph bind");
    assert_eq!(publication.manifest.archive_id, commitment.archive_id());
    let receipt = staging_receipt(&commitment, publication.manifest.semantic_digest());
    receipt
        .verify(&receipt.payload.binding, 1_500)
        .expect("signed staging receipt");
    let register = RegisterMusubiArchiveV1::new(commitment.clone(), receipt, 9);
    assert_eq!(roundtrip(&register), register);
    let mut mismatched = publication.clone();
    mismatched.resolution.lock.root_dependencies[0].requirement =
        "^2.0.0".parse().expect("requirement");
    assert!(mismatched.validate().is_err());
    let assertion = iroha_data_model::isi::musubi::AssertMusubiReleaseDigestV1::new(
        publication.manifest.release.clone(),
        publication.manifest.release_digest(),
    );
    assert_eq!(roundtrip(&assertion), assertion);
}
#[test]
fn governance_yank_and_cursor_requests_are_revision_bound() {
    let owner = account(51);
    let package = package("swap");
    let record = MusubiPackageRecordV1 {
        package: package.clone(),
        claimed_namespace: namespace_binding().namespace,
        claimed_namespace_binding: namespace_binding().digest(),
        owners: vec![owner.clone()],
        member_accounts: vec![owner.clone()],
        claimed_at_height: 42,
        revisions: MusubiPackageRevisionsV1 {
            governance: 5,
            metadata: 3,
            archive_locations: 2,
        },
    };
    record
        .validate()
        .expect("single owner is a valid terminal owner set");
    let mut ownerless = record.clone();
    ownerless.owners.clear();
    assert!(ownerless.validate().is_err());
    let remove_last_owner = RemoveMusubiPackageMaintainerV1 {
        package: package.clone(),
        account: owner.clone(),
        expected_governance_revision: record.revisions.governance,
    };
    assert_eq!(roundtrip(&remove_last_owner), remove_last_owner);
    assert_eq!(remove_last_owner.expected_governance_revision, 5);
    let release = release("swap", "1.2.3");
    let yank = SetMusubiReleaseYankV1::new(
        release.clone(),
        true,
        MusubiReasonV1::new("withdrawn pending review").expect("reason"),
        8,
    );
    let unyank = SetMusubiReleaseYankV1::new(
        release.clone(),
        false,
        MusubiReasonV1::new("review complete").expect("reason"),
        9,
    );
    assert_eq!(roundtrip(&yank), yank);
    assert_eq!(roundtrip(&unyank), unyank);
    assert_eq!(
        unyank.expected_yank_revision,
        yank.expected_yank_revision + 1
    );
    let cursor = MusubiFinalizedCursorV1 {
        snapshot: snapshot(),
        query_hash: MusubiQueryHashV1::new([0x55; 32]),
        last_key: release.to_string(),
        caller: Some(owner),
    };
    cursor.validate().expect("fully bound cursor");
    let page = MusubiPageRequestV1 {
        limit: 50,
        cursor: Some(cursor.clone()),
    };
    page.validate().expect("page cursor");
    assert_eq!(page.effective_limit(), 50);
    let stale_snapshot = MusubiRegistrySnapshotV1 {
        index_revision: snapshot().index_revision + 1,
        ..snapshot()
    };
    assert_ne!(cursor.snapshot, stale_snapshot);
    let alias: MusubiAliasNameV1 = "swap".parse().expect("alias");
    let exact_package = FindMusubiExactPackageV1::new(MusubiExactPackageQueryV1 {
        package: package.clone(),
    });
    let exact_release = FindMusubiExactReleaseV1::new(MusubiExactReleaseQueryV1 {
        release: release.clone(),
    });
    let resolver = FindMusubiResolverIndexV1::new(MusubiResolverIndexQueryV1 {
        package: package.clone(),
        requirement: Some("^1.0.0".parse().expect("requirement")),
        page: page.clone(),
    });
    let versions = FindMusubiVersionsV1::new(MusubiPackagePageQueryV1 {
        package: package.clone(),
        page: page.clone(),
    });
    let maintainers = FindMusubiMaintainersV1::new(MusubiPackagePageQueryV1 {
        package: package.clone(),
        page: page.clone(),
    });
    let locations = FindMusubiArchiveLocationsV1::new(MusubiArchiveLocationQueryV1 {
        archive_id: archive_commitment().archive_id(),
        page: page.clone(),
    });
    let alias_request = MusubiAliasQueryV1 {
        alias,
        page: page.clone(),
    };
    let alias = FindMusubiAliasV1::new(alias_request.clone());
    let history = FindMusubiAliasHistoryV1::new(alias_request);
    let prefix = FindMusubiOrderedPrefixV1::new(MusubiOrderedPrefixQueryV1 {
        prefix: MusubiOrderedPrefixV1::new("dex.universal/").expect("prefix"),
        page,
    });
    assert_eq!(roundtrip(&exact_package), exact_package);
    assert_eq!(roundtrip(&exact_release), exact_release);
    assert_eq!(roundtrip(&resolver), resolver);
    assert_eq!(roundtrip(&versions), versions);
    assert_eq!(roundtrip(&maintainers), maintainers);
    assert_eq!(roundtrip(&locations), locations);
    assert_eq!(roundtrip(&alias), alias);
    assert_eq!(roundtrip(&history), history);
    assert_eq!(roundtrip(&prefix), prefix);
}
