// Musubi data-model tests included from the parent module.
use iroha_crypto::{Algorithm, KeyPair};
use norito::codec::DecodeAll as _;

use super::*;
use crate::{
    account::{MultisigMember, MultisigPolicy},
    sorafs::pin_registry::ProviderIngestCompletionSignerPolicyV1,
};

fn account(seed: u8) -> AccountId {
    let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("fixture seed derives a checked keypair");
    AccountId::new(keypair.public_key().clone())
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

fn snapshot() -> MusubiRegistrySnapshotV1 {
    MusubiRegistrySnapshotV1 {
        finalized_height: 42,
        finalized_block_hash: [0x42; 32],
        index_revision: 3,
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

fn seed_ingress_binding(broker: AccountId) -> MusubiSeedIngressReceiptBindingV1 {
    let commitment = archive_commitment();
    MusubiSeedIngressReceiptBindingV1 {
        chain_id: ChainId::from("musubi-publish-test"),
        genesis_block_hash: [0x15; 32],
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
        chain_id: ChainId::from("musubi-publish-test"),
        genesis_block_hash: [0x23; 32],
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
fn seed_ingress_receipt_rejects_expiry_replay_and_commitment_substitution() {
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
    replayed.chain_id = ChainId::from("musubi-other-chain");
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

    let mut location = MusubiArchiveLocationV1 {
        location_id: MusubiArchiveLocationIdV1::new([0x31; 32]),
        archive_id: binding.archive_id,
        pin_manifest: ManifestDigest::new([0x32; 32]),
        replication_order: binding.replication_order,
        providers: vec![binding.provider_id],
        provider_attestations: vec![attestation],
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
    oversized.content_length = MUSUBI_MAX_SOURCE_PAYLOAD_BYTES_V1 + 1;
    assert!(oversized.validate().is_err());
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
    let exact = MusubiExactDependencyEdgeV1 {
        alias: dependency.alias.clone(),
        kind: MusubiDependencyKindV1::Normal,
        package: dependency.package.clone(),
        requirement: dependency.requirement.clone(),
        selected: selected.clone(),
    };
    let mut lock = MusubiVerificationLockV1 {
        schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
        version: MUSUBI_REGISTRY_VERSION_V1,
        root: release("swap-core", "1.2.3"),
        root_dependencies: vec![exact],
        nodes: vec![node(parallel, 20), node(selected, 10)],
    };
    lock.canonicalize();
    lock.validate().expect("exact root selection validates");

    let mut manifest = release_manifest();
    manifest.dependencies = vec![dependency];
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

    publication.manifest.dependencies[0].alias = "renamed".parse().expect("alias");
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
        root_dependencies: Vec::new(),
        nodes: vec![
            node(first.clone(), edge("second", second.clone())),
            node(second, edge("first", first)),
        ],
    };
    lock.canonicalize();
    assert!(lock.validate().is_err());
}

#[test]
fn release_record_keeps_yank_and_takedown_outside_immutable_digest() {
    let manifest = release_manifest();
    let publisher = account(7);
    let record = MusubiReleaseRecordV1 {
        release_digest: manifest.release_digest(),
        yank: MusubiReleaseYankV1 {
            release: manifest.release.clone(),
            yanked: false,
            reason: "initial publication".parse().expect("reason"),
            changed_by: publisher.clone(),
            changed_at_height: 42,
            revision: 1,
        },
        artifact_governance: MusubiArtifactGovernanceStateV1::Available,
        revisions: MusubiReleaseRevisionsV1 {
            yank: 1,
            artifact_governance: 1,
        },
        manifest,
        published_by: publisher,
        published_at_height: 42,
    };
    record.validate().expect("valid record");
}

#[test]
fn release_yank_validation_recurses_into_decoded_release_and_reason() {
    let valid = MusubiReleaseYankV1 {
        release: release("validation", "1.0.0"),
        yanked: true,
        reason: "security response".parse().expect("reason"),
        changed_by: account(8),
        changed_at_height: 42,
        revision: 2,
    };
    valid.validate().expect("valid yank record");

    for raw in [String::new(), "x".repeat(1_025)] {
        let malformed = MusubiReleaseYankV1 {
            reason: MusubiReasonV1(raw),
            ..valid.clone()
        };
        let decoded = MusubiReleaseYankV1::decode_all(&mut malformed.encode().as_slice())
            .expect("malformed reason remains representable on the wire");
        assert!(
            decoded.validate().is_err(),
            "decoded empty or oversized yank reason must fail closed"
        );
    }

    let mut malformed = valid;
    malformed.release.package.name = MusubiPackageNameV1("Upper".to_owned());
    let decoded = MusubiReleaseYankV1::decode_all(&mut malformed.encode().as_slice())
        .expect("malformed release remains representable on the wire");
    assert!(
        decoded.validate().is_err(),
        "decoded yank release identity must be recursively validated"
    );
}

#[test]
fn persisted_records_recursively_validate_decoded_packages_and_takedown_reasons() {
    let mut malformed_package = package("nested");
    malformed_package.name = MusubiPackageNameV1("Upper".to_owned());
    let member = MusubiPackageMemberV1 {
        package: malformed_package.clone(),
        account: account(10),
        role: MusubiPackageRoleV1::Owner,
        accepted_at_height: 1,
        governance_revision: 1,
    };
    let invitation = MusubiMaintainerInvitationV1 {
        invite_id: MusubiInviteIdV1::new([0x41; 32]),
        package: malformed_package.clone(),
        invited_by: account(11),
        invited_account: account(12),
        role: MusubiPackageRoleV1::Owner,
        expected_governance_revision: 1,
        expires_at_height: 10,
        state: MusubiInvitationStateV1::Pending,
    };
    let metadata = MusubiPackageMetadataRecordV1 {
        package: malformed_package,
        metadata: MusubiReleaseMetadataV1::default(),
        revision: 1,
        changed_by: account(13),
        changed_at_height: 1,
    };
    let decoded_member =
        MusubiPackageMemberV1::decode_all(&mut member.encode().as_slice()).expect("member");
    let decoded_invitation =
        MusubiMaintainerInvitationV1::decode_all(&mut invitation.encode().as_slice())
            .expect("invitation");
    let decoded_metadata =
        MusubiPackageMetadataRecordV1::decode_all(&mut metadata.encode().as_slice())
            .expect("metadata");
    assert!(decoded_member.validate().is_err());
    assert!(decoded_invitation.validate().is_err());
    assert!(decoded_metadata.validate().is_err());

    for raw in [String::new(), "x".repeat(1_025)] {
        let governance = MusubiArtifactGovernanceStateV1::TakenDown(MusubiArtifactTakedownV1 {
            action_digest: MusubiGovernanceActionDigestV1::new([0x42; 32]),
            reason: MusubiReasonV1(raw),
            applied_at_height: 2,
        });
        let decoded =
            MusubiArtifactGovernanceStateV1::decode_all(&mut governance.encode().as_slice())
                .expect("malformed takedown remains representable on the wire");
        assert!(
            decoded.validate().is_err(),
            "decoded takedown reason must be recursively validated"
        );

        let manifest = release_manifest();
        let record = MusubiReleaseRecordV1 {
            release_digest: manifest.release_digest(),
            yank: MusubiReleaseYankV1 {
                release: manifest.release.clone(),
                yanked: false,
                reason: "initial publication".parse().expect("reason"),
                changed_by: account(14),
                changed_at_height: 1,
                revision: 1,
            },
            artifact_governance: decoded,
            revisions: MusubiReleaseRevisionsV1 {
                yank: 1,
                artifact_governance: 2,
            },
            manifest,
            published_by: account(14),
            published_at_height: 1,
        };
        assert!(
            record.validate().is_err(),
            "authoritative release record must reject its malformed takedown"
        );
    }
}

#[cfg(feature = "json")]
#[test]
fn governed_takedown_json_is_closed_and_uses_applied_height() {
    let manifest = release_manifest();
    let release = manifest.release.clone();
    let archive_id = manifest.archive_id;
    let publisher = account(15);
    let yank = MusubiReleaseYankV1 {
        release: release.clone(),
        yanked: false,
        reason: "initial publication".parse().expect("reason"),
        changed_by: publisher.clone(),
        changed_at_height: 42,
        revision: 1,
    };
    let governance = MusubiArtifactGovernanceStateV1::TakenDown(MusubiArtifactTakedownV1 {
        action_digest: MusubiGovernanceActionDigestV1::new([0x51; 32]),
        reason: "governed security response".parse().expect("reason"),
        applied_at_height: 50,
    });
    let record = MusubiReleaseRecordV1 {
        release_digest: manifest.release_digest(),
        yank: yank.clone(),
        artifact_governance: governance.clone(),
        revisions: MusubiReleaseRevisionsV1 {
            yank: 1,
            artifact_governance: 2,
        },
        manifest: manifest.clone(),
        published_by: publisher,
        published_at_height: 42,
    };
    record.validate().expect("canonical governed release");
    let row = MusubiResolverReleaseRowV1 {
        release,
        release_digest: manifest.release_digest(),
        archive_id,
        source_digest: MusubiContentDigestV1::new([0x52; 32]),
        interface_digest: manifest.interface_digest,
        abi: manifest.abi,
        dependencies: manifest.dependencies,
        selection: MusubiReleaseSelectionStateV1 {
            yank,
            storage: MusubiArchiveAvailabilityV1 {
                archive_id,
                availability: MusubiStorageAvailabilityV1::Selectable,
                healthy_replicas: MUSUBI_MIN_HEALTHY_REPLICAS_V1,
                active_locations: 1,
                finalized_height: 50,
                finalized_block_hash: [0x53; 32],
                index_revision: 2,
            },
            governance: governance.clone(),
        },
        index_revision: 2,
    };
    row.validate().expect("canonical governed resolver row");

    let governance_json = norito::json::to_json(&governance).expect("governance JSON encodes");
    assert!(governance_json.contains("\"applied_at_height\":50"));
    assert!(!governance_json.contains("enacted_at_height"));
    assert_eq!(
        norito::json::from_json::<MusubiArtifactGovernanceStateV1>(&governance_json)
            .expect("canonical governance JSON decodes"),
        governance
    );
    let legacy_height = governance_json.replace("\"applied_at_height\"", "\"enacted_at_height\"");
    assert!(
        norito::json::from_json::<MusubiArtifactGovernanceStateV1>(&legacy_height).is_err(),
        "the retired enactment-height spelling must not be accepted"
    );
    for (prefix, depth) in [
        ("{", "the governance envelope"),
        ("\"value\":{", "the takedown payload"),
    ] {
        let hostile = governance_json.replacen(prefix, &format!("{prefix}\"legacy\":true,"), 1);
        assert!(
            norito::json::from_json::<MusubiArtifactGovernanceStateV1>(&hostile).is_err(),
            "governance JSON must reject an unknown field at {depth}"
        );
    }

    let record_json = norito::json::to_json(&record).expect("release record JSON encodes");
    for (prefix, depth) in [
        ("{", "the release record"),
        ("\"yank\":{", "the yank projection"),
        ("\"revisions\":{", "the release revisions"),
    ] {
        let hostile = record_json.replacen(prefix, &format!("{prefix}\"legacy\":true,"), 1);
        assert!(
            norito::json::from_json::<MusubiReleaseRecordV1>(&hostile).is_err(),
            "release JSON must reject an unknown field at {depth}"
        );
    }

    let row_json = norito::json::to_json(&row).expect("resolver row JSON encodes");
    for (prefix, depth) in [
        ("{", "the resolver row"),
        ("\"selection\":{", "the selection projection"),
    ] {
        let hostile = row_json.replacen(prefix, &format!("{prefix}\"legacy\":true,"), 1);
        assert!(
            norito::json::from_json::<MusubiResolverReleaseRowV1>(&hostile).is_err(),
            "resolver JSON must reject an unknown field at {depth}"
        );
    }
}

#[test]
fn archive_availability_requires_exact_runtime_classification_and_capacity() {
    let availability = |state, healthy_replicas, active_locations| MusubiArchiveAvailabilityV1 {
        archive_id: ArchiveId::new([0xA7; 32]),
        availability: state,
        healthy_replicas,
        active_locations,
        finalized_height: 9,
        finalized_block_hash: [0xB7; 32],
        index_revision: 3,
    };

    for record in [
        availability(MusubiStorageAvailabilityV1::Unavailable, 0, 0),
        availability(MusubiStorageAvailabilityV1::Unavailable, 0, 2),
        availability(MusubiStorageAvailabilityV1::BelowQuorum, 1, 1),
        availability(MusubiStorageAvailabilityV1::BelowQuorum, 2, 1),
        availability(MusubiStorageAvailabilityV1::Selectable, 3, 1),
    ] {
        record
            .validate()
            .expect("canonical availability projection");
    }

    let mut zero_height = availability(MusubiStorageAvailabilityV1::Unavailable, 0, 0);
    zero_height.finalized_height = 0;
    let invalid = [
        zero_height,
        availability(MusubiStorageAvailabilityV1::Selectable, 3, 0),
        availability(MusubiStorageAvailabilityV1::Selectable, 2, 1),
        availability(MusubiStorageAvailabilityV1::BelowQuorum, 0, 1),
        availability(MusubiStorageAvailabilityV1::BelowQuorum, 3, 1),
        availability(MusubiStorageAvailabilityV1::Unavailable, 1, 1),
        availability(MusubiStorageAvailabilityV1::Selectable, 65, 1),
    ];
    for record in invalid {
        assert!(
            record.validate().is_err(),
            "noncanonical availability projection must fail: {record:?}"
        );
    }
}

#[test]
fn archive_retention_decisions_are_bounded_and_fail_closed() {
    let archive_id = ArchiveId::new([0xC7; 32]);
    let storage = MusubiArchiveAvailabilityV1 {
        archive_id,
        availability: MusubiStorageAvailabilityV1::Unavailable,
        healthy_replicas: 0,
        active_locations: 0,
        finalized_height: 9,
        finalized_block_hash: [0xD7; 32],
        index_revision: 3,
    };
    let referenced = MusubiArchiveRetentionDecisionV1 {
        archive_id,
        disposition: MusubiArchiveRetentionDispositionV1::RetainReferenced,
        active_releases: 1,
        yanked_releases: 2,
        taken_down_releases: 3,
        storage: Some(storage),
    };
    referenced
        .validate()
        .expect("published archives remain retained even without a healthy location");
    assert!(referenced.must_retain());
    let page = MusubiArchiveRetentionPageV1 {
        chain_id: ChainId::from("retention-model-test"),
        genesis_hash: [0xE7; 32],
        items: vec![referenced],
        snapshot: snapshot(),
        finalized_time_ms: 1_700_000_000_000,
    };
    page.validate()
        .expect("storage changes before the query anchor are valid");
    let mut future_storage = page;
    future_storage.items[0]
        .storage
        .as_mut()
        .expect("referenced storage")
        .finalized_height = 43;
    assert!(future_storage.validate().is_err());

    let unknown = MusubiArchiveRetentionDecisionV1 {
        archive_id: ArchiveId::new([0xC8; 32]),
        disposition: MusubiArchiveRetentionDispositionV1::RetainUnknown,
        active_releases: 0,
        yanked_releases: 0,
        taken_down_releases: 0,
        storage: None,
    };
    unknown
        .validate()
        .expect("unknown archives retain fail-closed");
    assert!(unknown.must_retain());

    let mut inconsistent = referenced.clone();
    inconsistent.disposition = MusubiArchiveRetentionDispositionV1::PruneGovernedTakedown;
    assert!(inconsistent.validate().is_err());

    let request = MusubiArchiveRetentionQueryV1 {
        archive_ids: vec![archive_id, ArchiveId::new([0xC8; 32])],
        expected_snapshot: Some(snapshot()),
    };
    request.validate().expect("canonical exact retention batch");
    let mut duplicate = request.clone();
    duplicate.archive_ids[1] = duplicate.archive_ids[0];
    assert!(duplicate.validate().is_err());
    let mut oversized = request;
    oversized.archive_ids = (1..=MUSUBI_MAX_ARCHIVE_RETENTION_BATCH_V1 + 1)
        .map(|index| {
            let mut bytes = [0_u8; 32];
            bytes[..8].copy_from_slice(
                &u64::try_from(index)
                    .expect("bounded fixture index")
                    .to_be_bytes(),
            );
            ArchiveId::new(bytes)
        })
        .collect();
    assert!(oversized.validate().is_err());
}

#[test]
fn parliament_actions_validate_decoded_nested_identifiers() {
    let owner_recovery =
        MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
            package: package("recovery"),
            owners: vec![account(9)],
            expected_revision: 1,
        });
    owner_recovery.validate().expect("valid owner recovery");
    let alias_recovery = MusubiParliamentActionV1::RetargetAlias(MusubiRetargetAliasV1 {
        alias: "stable".parse().expect("alias"),
        target: package("replacement"),
        expected_revision: 1,
    });
    alias_recovery.validate().expect("valid alias recovery");

    let mut malformed_owner = owner_recovery;
    let MusubiParliamentActionV1::RecoverPackageOwners(recovery) = &mut malformed_owner else {
        unreachable!("owner recovery fixture")
    };
    recovery.package.name = MusubiPackageNameV1("Upper".to_owned());

    let mut malformed_alias = alias_recovery.clone();
    let MusubiParliamentActionV1::RetargetAlias(recovery) = &mut malformed_alias else {
        unreachable!("alias recovery fixture")
    };
    recovery.alias = MusubiAliasNameV1("Upper".to_owned());

    let mut malformed_target = alias_recovery;
    let MusubiParliamentActionV1::RetargetAlias(recovery) = &mut malformed_target else {
        unreachable!("alias recovery fixture")
    };
    recovery.target.name = MusubiPackageNameV1("Upper".to_owned());

    for action in [malformed_owner, malformed_alias, malformed_target] {
        let decoded = MusubiParliamentActionV1::decode_all(&mut action.encode().as_slice())
            .expect("malformed nested identity remains representable on the wire");
        assert!(
            decoded.validate().is_err(),
            "decoded Parliament action must validate every nested identity"
        );
    }
}

#[cfg(feature = "json")]
#[test]
fn parliament_action_json_rejects_unknown_fields_recursively() {
    macro_rules! assert_unknown_rejected {
        ($canonical:expr, $prefix:literal, $depth:literal) => {{
            let canonical: &str = $canonical;
            let replacement = format!("{}\"legacy\":true,", $prefix);
            let hostile = canonical.replacen($prefix, &replacement, 1);
            assert_ne!(
                hostile, canonical,
                "canonical Parliament action JSON must contain {}",
                $depth
            );
            assert!(
                norito::json::from_json::<MusubiParliamentActionV1>(&hostile).is_err(),
                "Parliament action JSON must reject an unknown field at {}",
                $depth
            );
        }};
    }

    let owner_recovery =
        MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
            package: package("closed-owner-recovery"),
            owners: vec![account(9)],
            expected_revision: 1,
        });
    let alias_retarget = MusubiParliamentActionV1::RetargetAlias(MusubiRetargetAliasV1 {
        alias: "closed-alias".parse().expect("alias"),
        target: package("closed-alias-target"),
        expected_revision: 1,
    });
    let takedown = MusubiParliamentActionV1::TakedownArtifact(MusubiTakedownArtifactActionV1 {
        release: MusubiReleaseIdV1::new(
            package("closed-takedown"),
            "1.2.3-alpha".parse().expect("release version"),
        ),
        reason: MusubiReasonV1::new("hostile JSON regression test").expect("bounded reason"),
        expected_artifact_governance_revision: 1,
    });
    let set_policy = MusubiParliamentActionV1::SetRegistryPolicy(MusubiSetRegistryPolicyActionV1 {
        policy: MusubiRegistryPolicyV1::default(),
        expected_revision: 1,
    });

    for action in [&owner_recovery, &alias_retarget, &takedown, &set_policy] {
        let canonical =
            norito::json::to_json(action).expect("canonical Parliament action JSON encodes");
        assert_eq!(
            norito::json::from_json::<MusubiParliamentActionV1>(&canonical)
                .expect("canonical Parliament action JSON decodes"),
            *action
        );
        assert_unknown_rejected!(canonical.as_str(), "{", "the tagged action envelope");
        assert_unknown_rejected!(canonical.as_str(), "\"value\":{", "the action payload");
    }

    let owner_json = norito::json::to_json(&owner_recovery).expect("owner action encodes");
    assert_unknown_rejected!(owner_json.as_str(), "\"package\":{", "the package identity");
    assert_unknown_rejected!(owner_json.as_str(), "\"scope\":{", "the package scope");

    let takedown_json = norito::json::to_json(&takedown).expect("takedown action encodes");
    assert_unknown_rejected!(
        takedown_json.as_str(),
        "\"release\":{",
        "the release identity"
    );
    assert_unknown_rejected!(
        takedown_json.as_str(),
        "\"version\":{",
        "the structured version"
    );
    assert_unknown_rejected!(
        takedown_json.as_str(),
        "\"prerelease\":[{",
        "the prerelease identifier envelope"
    );

    let policy_json = norito::json::to_json(&set_policy).expect("policy action encodes");
    assert_unknown_rejected!(policy_json.as_str(), "\"policy\":{", "the registry policy");
    assert_unknown_rejected!(policy_json.as_str(), "\"mode\":{", "the admission mode");
    assert_unknown_rejected!(
        policy_json.as_str(),
        "\"alias_pricing\":{",
        "the alias pricing policy"
    );
}

#[test]
fn governance_decision_consumption_binds_execution_boundary_and_roundtrips() {
    let consumption = MusubiGovernanceDecisionConsumptionV1 {
        decision: MusubiGovernanceDecisionV1 {
            decision_id: [0x31; 32],
            action_digest: MusubiGovernanceActionDigestV1::new([0x42; 32]),
            enacted_at_height: 10,
            execute_after_height: 20,
        },
        minimum_enactment_delay: 10,
        consumed_at_height: 20,
    };
    consumption
        .validate()
        .expect("execution exactly at the decision boundary is valid");

    let decoded =
        MusubiGovernanceDecisionConsumptionV1::decode_all(&mut consumption.encode().as_slice())
            .expect("decision consumption Norito roundtrip");
    assert_eq!(decoded, consumption);
    decoded
        .validate()
        .expect("roundtripped consumption validates");

    let mut premature = consumption;
    premature.consumed_at_height = 19;
    assert!(premature.validate().is_err());

    let mut shortened_delay = consumption;
    shortened_delay.minimum_enactment_delay = 11;
    assert!(shortened_delay.validate().is_err());

    let mut malformed = consumption;
    malformed.decision.decision_id = [0; 32];
    assert!(malformed.validate().is_err());
}

#[cfg(feature = "json")]
#[test]
fn governance_decision_consumption_json_rejects_bare_and_unknown_forms() {
    let consumption = MusubiGovernanceDecisionConsumptionV1 {
        decision: MusubiGovernanceDecisionV1 {
            decision_id: [0x31; 32],
            action_digest: MusubiGovernanceActionDigestV1::new([0x42; 32]),
            enacted_at_height: 10,
            execute_after_height: 20,
        },
        minimum_enactment_delay: 10,
        consumed_at_height: 20,
    };
    let canonical =
        norito::json::to_json(&consumption).expect("canonical decision consumption JSON encodes");
    assert_eq!(
        norito::json::from_json::<MusubiGovernanceDecisionConsumptionV1>(&canonical)
            .expect("canonical decision consumption JSON decodes"),
        consumption
    );

    let bare = norito::json::to_json(&consumption.decision)
        .expect("bare decision JSON remains representable as its own public type");
    assert!(
        norito::json::from_json::<MusubiGovernanceDecisionConsumptionV1>(&bare).is_err(),
        "the persisted consumption store must not accept the old bare-decision shape"
    );

    let unknown = canonical.replacen('{', "{\"legacy\":true,", 1);
    assert!(
        norito::json::from_json::<MusubiGovernanceDecisionConsumptionV1>(&unknown).is_err(),
        "the first-release consumption shape must reject unknown fields"
    );

    let nested_unknown = canonical.replacen("\"decision\":{", "\"decision\":{\"legacy\":true,", 1);
    assert_ne!(nested_unknown, canonical, "decision JSON field is present");
    assert!(
        norito::json::from_json::<MusubiGovernanceDecisionConsumptionV1>(&nested_unknown).is_err(),
        "the nested first-release decision shape must reject unknown fields"
    );
}

#[test]
fn alias_names_and_genesis_prices_are_exact() {
    for (alias, expected) in [
        ("a", 1_000),
        ("ab", 200),
        ("abc", 40),
        ("abcd", 8),
        ("abcde", 1),
    ] {
        let alias: MusubiAliasNameV1 = alias.parse().expect("alias");
        assert_eq!(
            MusubiAliasPricingPolicyV1::GENESIS.price_for(&alias),
            expected
        );
    }
    assert!("Upper".parse::<MusubiAliasNameV1>().is_err());
    assert!("-bad".parse::<MusubiAliasNameV1>().is_err());
    assert!("a".repeat(33).parse::<MusubiAliasNameV1>().is_err());
}

#[test]
fn registry_policy_successors_bind_price_changes_to_pricing_revisions() {
    let current = MusubiRegistryPolicyV1::default();

    let mut mode_only = current.clone();
    mode_only.revision += 1;
    mode_only.mode = MusubiRegistryAdmissionModeV1::Closed;
    mode_only
        .validate_successor(&current)
        .expect("non-price policy changes retain the exact pricing policy");

    let mut unchanged_with_new_pricing_revision = mode_only.clone();
    unchanged_with_new_pricing_revision.alias_pricing.revision += 1;
    assert!(
        unchanged_with_new_pricing_revision
            .validate_successor(&current)
            .is_err(),
        "pricing revision must not advance when prices are unchanged"
    );

    let mut changed_without_new_pricing_revision = mode_only.clone();
    changed_without_new_pricing_revision
        .alias_pricing
        .length_5_to_32_xor += 1;
    assert!(
        changed_without_new_pricing_revision
            .validate_successor(&current)
            .is_err(),
        "changed prices must advance the pricing revision"
    );

    let mut changed = changed_without_new_pricing_revision;
    changed.alias_pricing.revision += 1;
    changed
        .validate_successor(&current)
        .expect("changed prices with the exact successor revision are canonical");

    let mut skipped_pricing_revision = changed.clone();
    skipped_pricing_revision.alias_pricing.revision += 1;
    assert!(
        skipped_pricing_revision
            .validate_successor(&current)
            .is_err(),
        "changed prices must not skip a pricing revision"
    );

    let mut skipped = mode_only;
    skipped.revision += 1;
    assert!(
        skipped.validate_successor(&current).is_err(),
        "registry policy revisions must not skip"
    );

    let mut exhausted_policy = current.clone();
    exhausted_policy.revision = u64::MAX;
    assert!(
        current.validate_successor(&exhausted_policy).is_err(),
        "an exhausted registry revision cannot have a successor"
    );

    let mut exhausted_pricing = current.clone();
    exhausted_pricing.alias_pricing.revision = u64::MAX;
    let mut changed_after_exhausted_pricing = exhausted_pricing.clone();
    changed_after_exhausted_pricing.revision += 1;
    changed_after_exhausted_pricing
        .alias_pricing
        .length_5_to_32_xor += 1;
    assert!(
        changed_after_exhausted_pricing
            .validate_successor(&exhausted_pricing)
            .is_err(),
        "an exhausted pricing revision cannot describe changed prices"
    );
}

#[test]
fn page_and_cursor_bounds_are_enforced() {
    for limit in [
        0,
        u32::try_from(MUSUBI_MAX_PAGE_SIZE_V1).expect("page maximum fits u32"),
    ] {
        MusubiPageRequestV1 {
            limit,
            cursor: None,
        }
        .validate()
        .expect("default and exact-maximum page limits are canonical");
    }
    for limit in [
        u32::try_from(MUSUBI_MAX_PAGE_SIZE_V1 + 1).expect("page overflow fixture fits u32"),
        u32::MAX,
    ] {
        assert!(
            MusubiPageRequestV1 {
                limit,
                cursor: None,
            }
            .validate()
            .is_err(),
            "oversized page limit {limit} must be rejected instead of clamped"
        );
    }

    let ordered = MusubiVersionPageV1 {
        query: MusubiPackagePageQueryV1 {
            package: package("page-bounds"),
            page: MusubiPageRequestV1 {
                limit: 2,
                cursor: None,
            },
        },
        items: vec![
            "1.0.0".parse().expect("version"),
            "2.0.0".parse().expect("version"),
        ],
        next_cursor: None,
        snapshot: snapshot(),
    };
    ordered.validate().expect("strictly ordered version page");
    let mut reversed = ordered.clone();
    reversed.items.reverse();
    assert!(reversed.validate().is_err());
    let mut duplicate = ordered.clone();
    duplicate.items[1] = duplicate.items[0].clone();
    assert!(duplicate.validate().is_err());
    let malformed = MusubiVersionV1 {
        major: 1,
        minor: 0,
        patch: 0,
        prerelease: vec![MusubiPrereleaseIdentifierV1::AlphaNumeric(String::new())],
    };
    let decoded = MusubiVersionPageV1::decode_all(
        &mut MusubiVersionPageV1 {
            query: ordered.query.clone(),
            items: vec![malformed],
            next_cursor: None,
            snapshot: snapshot(),
        }
        .encode()
        .as_slice(),
    )
    .expect("malformed page item remains representable on the wire");
    assert!(
        decoded.validate().is_err(),
        "page validation must recurse into decoded items"
    );

    let page = MusubiVersionPageV1 {
        query: MusubiPackagePageQueryV1 {
            package: package("page-overflow"),
            page: MusubiPageRequestV1 {
                limit: u32::try_from(MUSUBI_MAX_PAGE_SIZE_V1).expect("page maximum fits u32"),
                cursor: None,
            },
        },
        items: vec!["1.0.0".parse().expect("version"); MUSUBI_MAX_PAGE_SIZE_V1 + 1],
        next_cursor: None,
        snapshot: snapshot(),
    };
    assert!(page.validate().is_err());

    let cursor = MusubiFinalizedCursorV1 {
        snapshot: snapshot(),
        query_hash: MusubiQueryHashV1::new([1; 32]),
        last_key: "x".repeat(MUSUBI_MAX_CURSOR_KEY_BYTES_V1 + 1),
        caller: None,
    };
    assert!(cursor.validate().is_err());

    let resolver_page = MusubiResolverIndexPageV1 {
        query: MusubiResolverIndexQueryV1 {
            package: package("resolver-page"),
            requirement: None,
            page: MusubiPageRequestV1 {
                limit: 50,
                cursor: None,
            },
        },
        chain_id: ChainId::from("musubi-test-chain"),
        genesis_hash: [9; 32],
        items: Vec::new(),
        next_cursor: None,
        snapshot: snapshot(),
    };
    resolver_page
        .validate()
        .expect("resolver page has authoritative lock identity");
    assert!(
        MusubiResolverIndexPageV1 {
            genesis_hash: [0; 32],
            ..resolver_page
        }
        .validate()
        .is_err()
    );

    let directory_page = MusubiOrderedPackagePageV1 {
        query: MusubiOrderedPrefixQueryV1 {
            prefix: MusubiOrderedPrefixV1::new("sora/").expect("directory prefix"),
            page: MusubiPageRequestV1 {
                limit: 50,
                cursor: None,
            },
        },
        chain_id: ChainId::from("musubi-test-chain"),
        genesis_hash: [9; 32],
        namespace_binding: MusubiNamespaceBindingV1 {
            namespace: "sora".parse().expect("namespace"),
            home_dataspace: DataSpaceId::new(7),
            scope: MusubiPackageScopeV1::DataspaceRoot,
            generation: 1,
        },
        items: Vec::new(),
        next_cursor: None,
        snapshot: snapshot(),
    };
    directory_page
        .validate()
        .expect("directory page has authoritative lock identity");
    assert!(
        MusubiOrderedPackagePageV1 {
            genesis_hash: [0; 32],
            ..directory_page
        }
        .validate()
        .is_err()
    );
}

#[test]
fn empty_response_pages_retain_their_exact_query_identity() {
    let package_id = package("empty-context");
    let package_query = MusubiPackagePageQueryV1 {
        package: package_id.clone(),
        page: MusubiPageRequestV1 {
            limit: 7,
            cursor: None,
        },
    };
    let versions = MusubiVersionPageV1 {
        query: package_query.clone(),
        items: Vec::new(),
        next_cursor: None,
        snapshot: snapshot(),
    };
    versions
        .validate_for(&package_query)
        .expect("empty version page retains its package and page controls");
    let mut other_package_query = package_query.clone();
    other_package_query.package = package("other-context");
    assert!(versions.validate_for(&other_package_query).is_err());

    let resolver_query = MusubiResolverIndexQueryV1 {
        package: package_id,
        requirement: Some("^1.2.3".parse().expect("requirement")),
        page: MusubiPageRequestV1 {
            limit: 9,
            cursor: None,
        },
    };
    let resolver = MusubiResolverIndexPageV1 {
        query: resolver_query.clone(),
        chain_id: ChainId::from("musubi-test-chain"),
        genesis_hash: [9; 32],
        items: Vec::new(),
        next_cursor: None,
        snapshot: snapshot(),
    };
    resolver
        .validate_for(&resolver_query)
        .expect("empty resolver page retains package, requirement, and page controls");
    let mut other_resolver_query = resolver_query.clone();
    other_resolver_query.requirement = Some("~1.2.3".parse().expect("requirement"));
    assert!(resolver.validate_for(&other_resolver_query).is_err());

    let maintainers = MusubiMaintainerPageV1 {
        query: package_query.clone(),
        items: Vec::new(),
        next_cursor: None,
        snapshot: snapshot(),
    };
    maintainers
        .validate_for(&package_query)
        .expect("empty maintainer page retains its package context");

    let alias_query = MusubiAliasQueryV1 {
        alias: "math".parse().expect("alias"),
        page: MusubiPageRequestV1 {
            limit: 11,
            cursor: None,
        },
    };
    let history = MusubiAliasHistoryPageV1 {
        query: alias_query.clone(),
        items: Vec::new(),
        next_cursor: None,
        snapshot: snapshot(),
    };
    history
        .validate_for(&alias_query)
        .expect("empty alias-history page retains its alias context");

    let prefix_query = MusubiOrderedPrefixQueryV1 {
        prefix: MusubiOrderedPrefixV1::new("sora/math-").expect("prefix"),
        page: MusubiPageRequestV1 {
            limit: 13,
            cursor: None,
        },
    };
    let directory = MusubiOrderedPackagePageV1 {
        query: prefix_query.clone(),
        chain_id: ChainId::from("musubi-test-chain"),
        genesis_hash: [9; 32],
        namespace_binding: MusubiNamespaceBindingV1 {
            namespace: "sora".parse().expect("namespace"),
            home_dataspace: DataSpaceId::new(7),
            scope: MusubiPackageScopeV1::DataspaceRoot,
            generation: 1,
        },
        items: Vec::new(),
        next_cursor: None,
        snapshot: snapshot(),
    };
    directory
        .validate_for(&prefix_query)
        .expect("empty directory page retains its complete prefix context");

    let search_query = MusubiSearchQueryV1 {
        query: "arithmetic math".to_owned(),
        page: MusubiSearchPageRequestV1 {
            limit: 15,
            cursor: None,
        },
    };
    let search = MusubiSearchPageV1 {
        query: search_query.clone(),
        items: Vec::new(),
        next_cursor: None,
        snapshot: MusubiSearchSnapshotV1 {
            finalized_height: 5,
            finalized_block_hash: [7; 32],
            projection_revision: 9,
        },
    };
    search
        .validate_for(&search_query)
        .expect("empty first search page retains its exact terms and page controls");
    let mut other_search_query = search_query.clone();
    other_search_query.query = "math arithmetic".to_owned();
    assert!(search.validate_for(&other_search_query).is_err());
}

#[test]
fn version_page_cursor_advances_by_structured_semver() {
    let snapshot = snapshot();
    let cursor = MusubiFinalizedCursorV1 {
        snapshot,
        query_hash: MusubiQueryHashV1::new([0x31; 32]),
        last_key: "1.2.0".to_owned(),
        caller: None,
    };
    let page = MusubiVersionPageV1 {
        query: MusubiPackagePageQueryV1 {
            package: package("semver-cursor"),
            page: MusubiPageRequestV1 {
                limit: 2,
                cursor: Some(cursor),
            },
        },
        items: vec!["1.10.0".parse().expect("version")],
        next_cursor: None,
        snapshot,
    };
    page.validate()
        .expect("1.10.0 follows 1.2.0 by structured SemVer, not lexical text order");

    let mut prerelease = page;
    prerelease
        .query
        .page
        .cursor
        .as_mut()
        .expect("cursor")
        .last_key = "2.0.0-alpha.10".to_owned();
    prerelease.items = vec!["2.0.0-beta.1".parse().expect("prerelease")];
    prerelease
        .validate()
        .expect("prerelease cursor advancement uses structured SemVer ordering");
}

#[test]
fn finalized_next_cursor_binds_the_exact_full_page_tail() {
    let snapshot = snapshot();
    let query = MusubiPackagePageQueryV1 {
        package: package("next-cursor"),
        page: MusubiPageRequestV1 {
            limit: 1,
            cursor: None,
        },
    };
    let mut page = MusubiVersionPageV1 {
        query,
        items: vec!["1.0.0".parse().expect("version")],
        next_cursor: Some(MusubiFinalizedCursorV1 {
            snapshot,
            query_hash: MusubiQueryHashV1::new([0x41; 32]),
            last_key: "1.0.0".to_owned(),
            caller: None,
        }),
        snapshot,
    };
    page.validate().expect("exact full-page cursor tail");
    page.next_cursor.as_mut().expect("cursor").last_key = "1.0.1".to_owned();
    assert!(page.validate().is_err());
}

#[test]
fn ordered_prefix_requires_canonical_namespace_and_package_prefix() {
    for invalid in ["sora", "sora/-math", "sora/math--", "sora/math/extra"] {
        assert!(
            MusubiOrderedPrefixV1::new(invalid).is_err(),
            "invalid ordered prefix `{invalid}` must be rejected"
        );
    }
    let prefix = MusubiOrderedPrefixV1::new("apps.sora/math-").expect("canonical prefix");
    let (namespace, package_prefix) = prefix.components().expect("prefix components");
    assert_eq!(namespace.as_str(), "apps.sora");
    assert_eq!(package_prefix, "math-");
}

#[test]
fn sorafs_reverse_reference_keys_are_fixed_and_provider_prefix_bounded() {
    let location = MusubiArchiveLocationKeyV1::new(
        ArchiveId::new([7; 32]),
        MusubiArchiveLocationIdV1::new([8; 32]),
    );
    let pin = MusubiPinLocationReferenceV1 {
        pin_manifest: ManifestDigest::new([9; 32]),
        location,
        active: true,
    };
    let order = MusubiReplicationOrderLocationReferenceV1 {
        replication_order: ReplicationOrderId::new([10; 32]),
        location,
        active: false,
    };
    pin.validate().expect("valid pin reverse reference");
    order.validate().expect("valid order reuse tombstone");

    let provider = ProviderId::new([11; 32]);
    let range = MusubiProviderLocationKeyV1::provider_range(provider);
    assert!(range.contains(&MusubiProviderLocationKeyV1::new(provider, location)));
    assert!(!range.contains(&MusubiProviderLocationKeyV1::new(
        ProviderId::new([12; 32]),
        location,
    )));
    MusubiProviderLocationKeyV1::new(provider, location)
        .validate()
        .expect("valid provider reverse key");
}

#[cfg(feature = "json")]
#[test]
fn v1_query_request_json_rejects_unknown_secret_fields() {
    macro_rules! assert_closed_json {
        ($request_type:ty, $request:expr) => {{
            let request: $request_type = $request;
            let canonical = norito::json::to_json(&request)
                .expect("canonical Musubi V1 query request JSON encodes");
            assert_eq!(
                norito::json::from_json::<$request_type>(&canonical)
                    .expect("canonical Musubi V1 query request JSON decodes"),
                request
            );
            let hostile = canonical.replacen('{', "{\"private_key\":\"must-not-be-accepted\",", 1);
            assert!(
                norito::json::from_json::<$request_type>(&hostile).is_err(),
                "Musubi V1 query request JSON must reject unknown secret-bearing fields"
            );
        }};
    }

    let package = package("query-contract");
    let release = MusubiReleaseIdV1::new(
        package.clone(),
        "1.2.3".parse().expect("query release version"),
    );
    let page = MusubiPageRequestV1 {
        limit: MUSUBI_DEFAULT_PAGE_SIZE_V1,
        cursor: None,
    };
    assert_closed_json!(
        MusubiExactPackageQueryV1,
        MusubiExactPackageQueryV1 {
            package: package.clone()
        }
    );
    assert_closed_json!(
        MusubiExactReleaseQueryV1,
        MusubiExactReleaseQueryV1 { release }
    );
    assert_closed_json!(
        MusubiResolverIndexQueryV1,
        MusubiResolverIndexQueryV1 {
            package: package.clone(),
            requirement: Some("^1.0.0".parse().expect("query requirement")),
            page: page.clone(),
        }
    );
    assert_closed_json!(
        MusubiPackagePageQueryV1,
        MusubiPackagePageQueryV1 {
            package: package.clone(),
            page: page.clone(),
        }
    );
    assert_closed_json!(
        MusubiArchiveLocationQueryV1,
        MusubiArchiveLocationQueryV1 {
            archive_id: archive_commitment().archive_id(),
            page: page.clone(),
        }
    );
    assert_closed_json!(
        MusubiArchiveRetentionQueryV1,
        MusubiArchiveRetentionQueryV1 {
            archive_ids: vec![archive_commitment().archive_id()],
            expected_snapshot: Some(snapshot()),
        }
    );
    assert_closed_json!(
        MusubiAliasQueryV1,
        MusubiAliasQueryV1 {
            alias: "query-contract".parse().expect("query alias"),
            page: page.clone(),
        }
    );
    assert_closed_json!(
        MusubiSearchQueryV1,
        MusubiSearchQueryV1 {
            query: "zero-knowledge verifier".to_owned(),
            page: MusubiSearchPageRequestV1 {
                limit: MUSUBI_DEFAULT_PAGE_SIZE_V1,
                cursor: None,
            },
        }
    );
    assert_closed_json!(
        MusubiOrderedPrefixQueryV1,
        MusubiOrderedPrefixQueryV1 {
            prefix: MusubiOrderedPrefixV1::new("query/").expect("query prefix"),
            page,
        }
    );
}

#[cfg(feature = "json")]
#[test]
fn archive_retention_json_rejects_unknown_fields_recursively() {
    macro_rules! assert_unknown_rejected {
        ($type:ty, $canonical:expr, $prefix:literal, $depth:literal) => {{
            let canonical: &str = $canonical;
            let replacement = format!("{}\"legacy\":true,", $prefix);
            let hostile = canonical.replacen($prefix, &replacement, 1);
            assert_ne!(
                hostile, canonical,
                "canonical archive-retention JSON must contain {}",
                $depth
            );
            assert!(
                norito::json::from_json::<$type>(&hostile).is_err(),
                "archive-retention JSON must reject an unknown field at {}",
                $depth
            );
        }};
    }

    let snapshot = snapshot();
    let snapshot_json = norito::json::to_json(&snapshot).expect("registry snapshot JSON encodes");
    assert_unknown_rejected!(
        MusubiRegistrySnapshotV1,
        snapshot_json.as_str(),
        "{",
        "the registry snapshot"
    );

    let storage = MusubiArchiveAvailabilityV1 {
        archive_id: archive_commitment().archive_id(),
        availability: MusubiStorageAvailabilityV1::Selectable,
        healthy_replicas: MUSUBI_MIN_HEALTHY_REPLICAS_V1,
        active_locations: 1,
        finalized_height: snapshot.finalized_height,
        finalized_block_hash: snapshot.finalized_block_hash,
        index_revision: snapshot.index_revision,
    };
    storage.validate().expect("canonical availability fixture");
    let storage_json = norito::json::to_json(&storage).expect("archive availability JSON encodes");
    assert_unknown_rejected!(
        MusubiArchiveAvailabilityV1,
        storage_json.as_str(),
        "{",
        "the availability projection"
    );
    assert_unknown_rejected!(
        MusubiArchiveAvailabilityV1,
        storage_json.as_str(),
        "\"availability\":{",
        "the storage-availability envelope"
    );

    let request = MusubiArchiveRetentionQueryV1 {
        archive_ids: vec![storage.archive_id],
        expected_snapshot: Some(snapshot),
    };
    let request_json =
        norito::json::to_json(&request).expect("archive-retention request JSON encodes");
    assert_unknown_rejected!(
        MusubiArchiveRetentionQueryV1,
        request_json.as_str(),
        "\"expected_snapshot\":{",
        "the request snapshot"
    );

    let decision = MusubiArchiveRetentionDecisionV1 {
        archive_id: storage.archive_id,
        disposition: MusubiArchiveRetentionDispositionV1::RetainReferenced,
        active_releases: 1,
        yanked_releases: 0,
        taken_down_releases: 0,
        storage: Some(storage),
    };
    decision.validate().expect("canonical retention decision");
    let decision_json =
        norito::json::to_json(&decision).expect("archive-retention decision JSON encodes");
    assert_unknown_rejected!(
        MusubiArchiveRetentionDecisionV1,
        decision_json.as_str(),
        "\"disposition\":{",
        "the retention-disposition envelope"
    );
    assert_unknown_rejected!(
        MusubiArchiveRetentionDecisionV1,
        decision_json.as_str(),
        "\"storage\":{",
        "the nested availability projection"
    );
}

#[test]
fn search_terms_are_bounded_exact_and_canonical() {
    let request = MusubiSearchQueryV1 {
        query: "Zero-Knowledge verifier verifier".to_owned(),
        page: MusubiSearchPageRequestV1 {
            limit: 0,
            cursor: None,
        },
    };
    assert_eq!(
        request.normalized_terms().expect("normalized terms"),
        vec![
            "knowledge".to_owned(),
            "verifier".to_owned(),
            "zero".to_owned(),
            "zero-knowledge".to_owned(),
        ]
    );
    assert_eq!(request.page.effective_limit(), 50);

    let mut too_many = request;
    too_many.query = (0..=MUSUBI_MAX_SEARCH_QUERY_TERMS_V1)
        .map(|index| format!("term{index}"))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(too_many.validate().is_err());
}
