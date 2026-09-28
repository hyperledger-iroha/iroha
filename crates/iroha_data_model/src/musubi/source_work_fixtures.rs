// Existing canonical Musubi fixtures shared by semantic and source-work controls.

pub(super) fn account(seed: u8) -> AccountId {
    let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("fixture seed derives a checked keypair");
    AccountId::new(keypair.public_key().clone())
}

pub(super) fn archive_commitment() -> MusubiArchiveCommitmentV1 {
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

pub(super) fn provider_bundle_binding(
    owner: AccountId,
) -> MusubiProviderBundleVerificationBindingV1 {
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

pub(super) fn release_manifest() -> MusubiReleaseManifestV1 {
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
