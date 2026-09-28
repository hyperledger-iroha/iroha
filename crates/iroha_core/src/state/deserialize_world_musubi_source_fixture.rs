// Shared provider-attestation source fixture for decode and bounded-capture controls.

#[allow(clippy::too_many_lines)]
pub(super) fn seed_provider_attested_location(
    world: &mut World,
    release: &MusubiReleaseIdV1,
    archive_id: ArchiveId,
) -> MusubiProviderBundleAttestationKeyV1 {
    let archive = world
        .musubi_archives
        .view()
        .get(&archive_id)
        .cloned()
        .expect("seeded archive");
    let verification_lock_digest = world
        .musubi_releases
        .view()
        .get(release)
        .map(|record| record.manifest.verification_lock_digest)
        .expect("seeded release");
    let provider_keypair = KeyPair::try_from_seed(vec![70; 32], Algorithm::Ed25519)
        .expect("derive deterministic provider key");
    let provider_owner = AccountId::new(provider_keypair.public_key().clone());
    let provider_id = ProviderId::new([0x31; 32]);
    let replication_order = ReplicationOrderId::new([0x32; 32]);
    let location_id = MusubiArchiveLocationIdV1::new([0x33; 32]);
    let completion_authority = ProviderIngestCompletionAuthorityV1::new(
        provider_owner.clone(),
        ProviderIngestCompletionSignerPolicyV1 {
            policy_id: [0x34; 32],
            revision: 1,
            predecessor_digest: None,
            policy_digest: [0x35; 32],
        },
    );
    let binding = MusubiProviderBundleVerificationBindingV1 {
        network_id: archive.staging_receipt.payload.binding.network_id,
        provider_id,
        completed_by: provider_owner,
        completion_authority,
        replication_order,
        assignment_revision: 1,
        completion_epoch: 1,
        finalized_anchor: ProviderIngestFinalizedAnchorV1 {
            height: 2,
            block_hash: [0x36; 32],
        },
        archive_id,
        bundle_digest: archive.commitment.bundle_digest,
        descriptor_digest: archive.commitment.descriptor_digest,
        semantic_release_manifest_digest: archive
            .staging_receipt
            .payload
            .binding
            .semantic_release_manifest_digest,
        verification_lock_digest,
        source_tree_digest: archive.commitment.source_tree_digest,
    };
    let payload = MusubiProviderBundleVerificationPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        binding: binding.clone(),
    };
    let attestation = MusubiProviderBundleVerificationAttestationV1 {
        approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
            public_key: provider_keypair.public_key().clone(),
            signature: SignatureOf::try_from_hash(
                provider_keypair.private_key(),
                payload.signing_hash(),
            )
            .expect("sign provider bundle attestation"),
        }],
        payload,
    };
    attestation
        .verify(&binding)
        .expect("provider bundle attestation fixture verifies");
    let attestation_key = attestation.key();
    let attestation_reference = attestation.reference();
    let attestation_record = MusubiProviderBundleAttestationRecordV1 {
        key: attestation_key,
        attestation_digest: attestation.digest(),
        attestation,
        registered_by: archive.registered_by.clone(),
        registered_at_height: 2,
    };
    attestation_record
        .validate()
        .expect("valid provider attestation record");
    let provider_attestation_set_digest = musubi_provider_bundle_attestation_set_digest_v1(
        archive_id,
        replication_order,
        &[attestation_reference],
    )
    .expect("valid provider attestation set");
    let location = MusubiArchiveLocationV1 {
        location_id,
        archive_id,
        pin_manifest: ManifestDigest::new([0x37; 32]),
        replication_order,
        providers: vec![provider_id],
        provider_attestation_set_digest,
        renew_after_epoch: 1,
        expires_at_epoch: 2,
        finalized_height: 2,
        revision: 2,
        state: MusubiArchiveLocationStateV1::Degraded,
    };
    location.validate().expect("valid archive location fixture");
    let mut updated_archive = archive;
    updated_archive.location_revision = 2;
    updated_archive.location_ids = vec![location_id];
    updated_archive
        .validate()
        .expect("archive contains the exact current location directory");
    world.musubi_archives.insert(archive_id, updated_archive);
    world
        .musubi_provider_bundle_attestations
        .insert(attestation_key, attestation_record);
    world
        .musubi_archive_locations
        .insert(location.key(), location);
    attestation_key
}
