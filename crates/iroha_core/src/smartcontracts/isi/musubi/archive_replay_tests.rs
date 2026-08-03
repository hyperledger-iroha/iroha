// Archive and retention tests included from the parent module.
use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
use mv::cell::Cell;
use mv::storage::StorageReadOnly as _;

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{GovernancePipeline, GovernanceProposalRecord, State, World},
};

const GOVERNANCE_EXECUTION_HEIGHT: u64 = 42;

fn location_fixture(
    archive_byte: u8,
    pin: iroha_data_model::sorafs::pin_registry::ManifestDigest,
    order: iroha_data_model::sorafs::pin_registry::ReplicationOrderId,
) -> MusubiArchiveLocationV1 {
    MusubiArchiveLocationV1 {
        location_id: MusubiArchiveLocationIdV1::new([archive_byte; 32]),
        archive_id: ArchiveId::new([archive_byte; 32]),
        pin_manifest: pin,
        replication_order: order,
        providers: Vec::new(),
        provider_attestations: Vec::new(),
        renew_after_epoch: 1,
        expires_at_epoch: 2,
        finalized_height: 1,
        revision: 1,
        state: MusubiArchiveLocationStateV1::Healthy,
    }
}

fn account(seed: u8) -> AccountId {
    let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("fixture seed derives an account");
    AccountId::new(keypair.public_key().clone())
}

#[cfg(feature = "telemetry")]
fn governance_rejection_counts(
    metrics: &crate::telemetry::Metrics,
    action: &str,
    reason: &str,
) -> (u64, u64) {
    let exposition = metrics.try_to_string().expect("encode metrics");
    let action_label = format!("action=\"{action}\"");
    let reason_label = format!("reason=\"{reason}\"");
    exposition
        .lines()
        .filter(|line| line.starts_with("musubi_governance_rejections_total{"))
        .fold((0_u64, 0_u64), |(total, exact), line| {
            let (labels, value) = line
                .rsplit_once(' ')
                .expect("Prometheus counter sample has a value");
            let value = value.parse::<u64>().expect("counter sample is an integer");
            let exact = if labels.contains(&action_label) && labels.contains(&reason_label) {
                exact + value
            } else {
                exact
            };
            (total + value, exact)
        })
}

#[cfg(feature = "telemetry")]
#[test]
fn governance_rejections_are_counted_once_at_the_authoritative_isi_boundary() {
    use std::sync::Arc;

    let metrics = Arc::new(crate::telemetry::Metrics::default());
    let telemetry = crate::telemetry::StateTelemetry::new(Arc::clone(&metrics), true);
    let state = State::with_telemetry(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        telemetry,
    );
    let before_unauthorized = governance_rejection_counts(&metrics, "remove", "unauthorized");
    let before_stale = governance_rejection_counts(&metrics, "remove", "stale_revision");
    let before_last_owner = governance_rejection_counts(&metrics, "remove", "last_owner");

    {
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("nonzero block height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let owner = account(11);
        let stranger = account(12);
        let package = package("telemetry-last-owner");
        seed_package_owner(&package, &owner, 1, &mut transaction);
        let remove = |expected_governance_revision| RemoveMusubiPackageMaintainerV1 {
            package: package.clone(),
            account: owner.clone(),
            expected_governance_revision,
        };

        remove(1)
            .execute(&stranger, &mut transaction)
            .expect_err("a non-owner must be rejected");
        remove(2)
            .execute(&owner, &mut transaction)
            .expect_err("a stale governance revision must be rejected");
        let error = remove(1)
            .execute(&owner, &mut transaction)
            .expect_err("the sole owner cannot be removed");
        assert!(error.to_string().contains("retain its last owner"));
    }

    let after_unauthorized = governance_rejection_counts(&metrics, "remove", "unauthorized");
    let after_stale = governance_rejection_counts(&metrics, "remove", "stale_revision");
    let after_last_owner = governance_rejection_counts(&metrics, "remove", "last_owner");
    assert_eq!(after_unauthorized.1, before_unauthorized.1 + 1);
    assert_eq!(after_stale.1, before_stale.1 + 1);
    assert_eq!(after_last_owner.1, before_last_owner.1 + 1);
    assert_eq!(after_last_owner.0, before_last_owner.0 + 3);
}

#[test]
fn publication_snapshot_accepts_a_canonical_finalized_ancestor() {
    let snapshot = MusubiRegistrySnapshotV1 {
        finalized_height: 2,
        finalized_block_hash: [0x22; 32],
        index_revision: 7,
    };
    validate_publication_snapshot_anchor(&snapshot, 5, Some([0x22; 32]), 9)
        .expect("a canonical ancestor remains valid while publication evidence finalizes");
}

#[test]
fn publication_snapshot_rejects_future_or_noncanonical_anchors() {
    let snapshot = MusubiRegistrySnapshotV1 {
        finalized_height: 3,
        finalized_block_hash: [0x33; 32],
        index_revision: 4,
    };
    assert!(validate_publication_snapshot_anchor(&snapshot, 2, Some([0x33; 32]), 4).is_err());
    assert!(validate_publication_snapshot_anchor(&snapshot, 3, Some([0x44; 32]), 4).is_err());
    assert!(validate_publication_snapshot_anchor(&snapshot, 3, Some([0x33; 32]), 3).is_err());
}

#[test]
fn archive_registration_replay_requires_the_exact_original_receipt() {
    let mut world = World::new();
    let publisher_key =
        KeyPair::try_from_seed(vec![0x31; 32], Algorithm::Ed25519).expect("publisher key");
    let publisher = AccountId::new(publisher_key.public_key().clone());
    let broker_key =
        KeyPair::try_from_seed(vec![0x32; 32], Algorithm::Ed25519).expect("broker key");
    let broker = AccountId::new(broker_key.public_key().clone());
    let provider = iroha_data_model::sorafs::capacity::ProviderId::new([0x33; 32]);
    world.provider_owners.insert(provider, broker.clone());
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let genesis = iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(1).expect("genesis height"),
        None,
        None,
        None,
        500,
        0,
    );
    let genesis_hash = genesis.hash();
    {
        let mut block_hashes = state.block_hashes.block();
        block_hashes.push_for_tests(genesis_hash);
        block_hashes.commit_for_tests();
    }
    let header = iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(2).expect("replay height"),
        Some(genesis_hash),
        None,
        None,
        1_500,
        0,
    );
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    let commitment = retention_archive(0x34).commitment;
    let binding = MusubiSeedIngressReceiptBindingV1 {
        chain_id: transaction.chain_id().clone(),
        genesis_block_hash: *genesis_hash.as_ref(),
        publisher: publisher.clone(),
        ingress_broker: broker,
        seed_provider: provider,
        semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new([0x35; 32]),
        archive_id: commitment.archive_id(),
        car_body_digest: commitment.car_digest,
        car_body_length: commitment.car_size,
        nonce: [0x36; 32],
    };
    let signed_receipt =
        |binding: MusubiSeedIngressReceiptBindingV1, issued_at_ms, expires_at_ms| {
            let payload = MusubiSeedIngressReceiptPayloadV1 {
                version: MUSUBI_REGISTRY_VERSION_V1,
                binding,
                issued_at_ms,
                expires_at_ms,
            };
            MusubiSeedIngressReceiptV1 {
                approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
                    public_key: broker_key.public_key().clone(),
                    signature: SignatureOf::try_from_hash(
                        broker_key.private_key(),
                        payload.signing_hash(),
                    )
                    .expect("receipt signature"),
                }],
                payload,
            }
        };
    let registered_receipt = signed_receipt(binding.clone(), 500, 1_000);
    let archive_id = commitment.archive_id();
    transaction.world.musubi_archives.insert(
        archive_id,
        MusubiArchiveRecordV1 {
            archive_id,
            commitment: commitment.clone(),
            staging_receipt: registered_receipt.clone(),
            registered_by: publisher.clone(),
            registered_at_height: 1,
            location_revision: 1,
            location_ids: Vec::new(),
        },
    );

    RegisterMusubiArchiveV1::new(commitment.clone(), registered_receipt.clone(), 1)
        .execute(&publisher, &mut transaction)
        .expect("the exact registered receipt remains idempotent after expiry");
    assert_eq!(
        transaction
            .world
            .musubi_archives
            .get(&archive_id)
            .expect("registered archive")
            .staging_receipt,
        registered_receipt,
        "the first authoritative receipt remains immutable"
    );

    let refreshed_receipt = signed_receipt(binding.clone(), 1_400, 2_000);
    let error = RegisterMusubiArchiveV1::new(commitment.clone(), refreshed_receipt, 1)
        .execute(&publisher, &mut transaction)
        .expect_err("a refreshed receipt must not replace the registered receipt");
    assert!(
        error
            .to_string()
            .contains("different commitment or staging receipt")
    );

    let mut different_binding = binding;
    different_binding.nonce = [0xee; 32];
    let different_receipt = signed_receipt(different_binding, 1_400, 2_000);
    let error = RegisterMusubiArchiveV1::new(commitment, different_receipt, 1)
        .execute(&publisher, &mut transaction)
        .expect_err("a different operation nonce must not cross archive registration");
    assert!(
        error
            .to_string()
            .contains("different commitment or staging receipt")
    );
}

fn package(name: &str) -> MusubiPackageIdV1 {
    MusubiPackageIdV1::new(
        iroha_data_model::nexus::DataSpaceId::new(7),
        MusubiPackageScopeV1::DataspaceRoot,
        name.parse().expect("package name"),
    )
}

fn seed_package_owner(
    package: &MusubiPackageIdV1,
    owner: &AccountId,
    governance_revision: u64,
    transaction: &mut StateTransaction<'_, '_>,
) {
    transaction.world.musubi_packages.insert(
        package.clone(),
        MusubiPackageRecordV1 {
            package: package.clone(),
            claimed_namespace: "sora".parse().expect("namespace"),
            claimed_namespace_binding: MusubiNamespaceBindingDigestV1::new([1; 32]),
            owners: vec![owner.clone()],
            member_accounts: vec![owner.clone()],
            claimed_at_height: 1,
            revisions: MusubiPackageRevisionsV1 {
                governance: governance_revision,
                metadata: 1,
                archive_locations: 1,
            },
        },
    );
    let member = MusubiPackageMemberV1 {
        package: package.clone(),
        account: owner.clone(),
        role: MusubiPackageRoleV1::Owner,
        accepted_at_height: 1,
        governance_revision,
    };
    transaction
        .world
        .musubi_package_members
        .insert(member.key(), member.clone());
    upsert_maintainer_directory(
        MusubiMaintainerDirectoryEntryV1::Accepted(member),
        transaction,
    );
}

fn seed_pending_invitation(
    invitation: MusubiMaintainerInvitationV1,
    transaction: &mut StateTransaction<'_, '_>,
) {
    transaction
        .world
        .musubi_package_invitations
        .insert(invitation.invite_id, invitation.clone());
    upsert_maintainer_directory(
        MusubiMaintainerDirectoryEntryV1::PendingInvitation(invitation),
        transaction,
    );
}

fn take_musubi_events(transaction: &mut StateTransaction<'_, '_>) -> Vec<MusubiEvent> {
    transaction
        .world
        .take_external_events()
        .into_iter()
        .filter_map(|event| match event {
            iroha_data_model::events::EventBox::Data(data) => match data.as_ref() {
                DataEvent::Musubi(event) => Some(event.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn decision_for_current_block(
    decision_id: [u8; 32],
    action: &MusubiParliamentActionV1,
    transaction: &StateTransaction<'_, '_>,
) -> MusubiGovernanceDecisionV1 {
    let execute_after_height = execution_height(transaction);
    let delay = transaction.gov.min_enactment_delay.max(1);
    let enacted_at_height = execute_after_height
        .checked_sub(delay)
        .filter(|height| *height > 0)
        .expect("fixture block leaves a positive enactment height");
    MusubiGovernanceDecisionV1 {
        decision_id,
        action_digest: action.action_digest(),
        enacted_at_height,
        execute_after_height,
    }
}

fn insert_enacted_proposal(
    decision_id: [u8; 32],
    kind: ProposalKind,
    enacted_at_height: u64,
    transaction: &mut StateTransaction<'_, '_>,
) {
    transaction.world.put_governance_proposal(
        decision_id,
        GovernanceProposalRecord {
            proposer: account(80),
            kind,
            created_height: enacted_at_height.saturating_sub(1).max(1),
            status: GovernanceProposalStatus::Enacted,
            pipeline: GovernancePipeline::default(),
            parliament_snapshot: None,
            finalization_evidence: None,
            enacted_at_height: Some(enacted_at_height),
        },
    );
}

fn seed_enacted_decision(
    action: &MusubiParliamentActionV1,
    transaction: &mut StateTransaction<'_, '_>,
) -> MusubiGovernanceDecisionV1 {
    let kind = ProposalKind::MusubiRegistryGovernance(action.clone());
    let decision_id = kind.fingerprint();
    let decision = decision_for_current_block(decision_id, action, transaction);
    insert_enacted_proposal(decision_id, kind, decision.enacted_at_height, transaction);
    decision
}

fn snapshot(revision: u64) -> MusubiRegistrySnapshotV1 {
    MusubiRegistrySnapshotV1 {
        finalized_height: 7,
        finalized_block_hash: [7; 32],
        index_revision: revision,
    }
}

fn retention_archive(seed: u8) -> MusubiArchiveRecordV1 {
    let commitment = MusubiArchiveCommitmentV1 {
        root_cid: iroha_data_model::sorafs::pin_registry::ManifestRootCid::from_blake3_digest(
            [seed; 32],
        )
        .expect("retention fixture root CID"),
        chunker: iroha_data_model::sorafs::pin_registry::ChunkerProfileHandle {
            profile_id: 1,
            namespace: "sorafs".to_owned(),
            name: "sf1".to_owned(),
            semver: "1.0.0".to_owned(),
            multihash_code: 0x1f,
        },
        chunk_plan_digest: MusubiContentDigestV1::new([seed.wrapping_add(1); 32]),
        por_root: MusubiContentDigestV1::new([seed.wrapping_add(2); 32]),
        content_length: 1,
        car_digest: MusubiContentDigestV1::new([seed.wrapping_add(3); 32]),
        car_size: 1,
        bundle_digest: MusubiContentDigestV1::new([seed.wrapping_add(4); 32]),
        source_tree_digest: MusubiContentDigestV1::new([seed.wrapping_add(5); 32]),
        descriptor_digest: MusubiContentDigestV1::new([seed.wrapping_add(6); 32]),
        file_count: 1,
        chunk_count: 1,
    };
    let archive_id = commitment.archive_id();
    let publisher = account(seed);
    let broker_keypair = KeyPair::try_from_seed(vec![seed.wrapping_add(1); 32], Algorithm::Ed25519)
        .expect("retention fixture broker keypair");
    let broker = AccountId::new(broker_keypair.public_key().clone());
    let receipt_payload = MusubiSeedIngressReceiptPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        binding: MusubiSeedIngressReceiptBindingV1 {
            chain_id: iroha_data_model::ChainId::from("retention-test"),
            genesis_block_hash: [seed.wrapping_add(7); 32],
            publisher: publisher.clone(),
            ingress_broker: broker,
            seed_provider: iroha_data_model::sorafs::capacity::ProviderId::new(
                [seed.wrapping_add(8); 32],
            ),
            semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new(
                [seed.wrapping_add(9); 32],
            ),
            archive_id,
            car_body_digest: commitment.car_digest,
            car_body_length: commitment.car_size,
            nonce: [seed.wrapping_add(10); 32],
        },
        issued_at_ms: 1,
        expires_at_ms: 2,
    };
    let receipt_approval = MusubiSeedIngressReceiptApprovalV1 {
        public_key: broker_keypair.public_key().clone(),
        signature: SignatureOf::try_from_hash(
            broker_keypair.private_key(),
            receipt_payload.signing_hash(),
        )
        .expect("sign retention fixture receipt"),
    };
    MusubiArchiveRecordV1 {
        archive_id,
        commitment: commitment.clone(),
        staging_receipt: MusubiSeedIngressReceiptV1 {
            payload: receipt_payload,
            approvals: vec![receipt_approval],
        },
        registered_by: publisher,
        registered_at_height: 1,
        location_revision: 1,
        location_ids: Vec::new(),
    }
}

fn retention_release(
    archive_id: ArchiveId,
    version: &str,
    yanked: bool,
    artifact_governance: MusubiArtifactGovernanceStateV1,
) -> MusubiReleaseRecordV1 {
    let release = MusubiReleaseIdV1::new(
        package("retention"),
        version.parse().expect("retention release version"),
    );
    let manifest = MusubiReleaseManifestV1 {
        release: release.clone(),
        edition: MusubiKotodamaEditionV1::V1,
        abi: MusubiAbiBindingV1::new([0xA1; 32]).expect("retention ABI"),
        dependencies: Vec::new(),
        exports: Vec::new(),
        interface_digest: MusubiContentDigestV1::new([0xA2; 32]),
        metadata: MusubiReleaseMetadataV1::default(),
        archive_id,
        verification_lock_digest: MusubiVerificationLockDigestV1::new([0xA3; 32]),
    };
    MusubiReleaseRecordV1 {
        release_digest: manifest.release_digest(),
        manifest,
        published_by: account(111),
        published_at_height: 1,
        yank: MusubiReleaseYankV1 {
            release,
            yanked,
            reason: "retention fixture".parse().expect("yank reason"),
            changed_by: account(111),
            changed_at_height: 1,
            revision: 1,
        },
        artifact_governance,
        revisions: MusubiReleaseRevisionsV1 {
            yank: 1,
            artifact_governance: 1,
        },
    }
}

fn seed_retention_archive(
    world: &mut World,
    archive: MusubiArchiveRecordV1,
    releases: Vec<MusubiReleaseRecordV1>,
) -> ArchiveId {
    let archive_id = archive.archive_id;
    let mut release_ids = releases
        .iter()
        .map(|release| release.manifest.release.clone())
        .collect::<Vec<_>>();
    release_ids.sort();
    let release_count = u64::try_from(release_ids.len()).expect("release fixture count fits u64");
    for release in releases {
        world
            .musubi_releases
            .insert(release.manifest.release.clone(), release);
    }
    world.musubi_archives.insert(archive_id, archive);
    world.musubi_archive_availability.insert(
        archive_id,
        MusubiArchiveAvailabilityV1 {
            archive_id,
            availability: MusubiStorageAvailabilityV1::Unavailable,
            healthy_replicas: 0,
            active_locations: 0,
            finalized_height: 1,
            finalized_block_hash: [0xB1; 32],
            index_revision: 1,
        },
    );
    world.musubi_archive_reverse_references.insert(
        archive_id,
        MusubiArchiveReverseReferencesV1 {
            archive_id,
            releases: release_ids,
        },
    );
    let shortfall = *world.musubi_replication_shortfall_releases.view().get();
    world.musubi_replication_shortfall_releases = Cell::new(
        shortfall
            .checked_add(release_count)
            .expect("retention fixture shortfall count fits u64"),
    );
    archive_id
}

fn archive_location_replay_fixture(
    seed: u8,
) -> (
    World,
    AccountId,
    MusubiArchiveLocationKeyV1,
    AddMusubiArchiveLocationV1,
) {
    let mut world = World::new();
    let mut archive = retention_archive(seed);
    let authority = archive.registered_by.clone();
    let location_id = MusubiArchiveLocationIdV1::new([seed.wrapping_add(11); 32]);
    let pin =
        iroha_data_model::sorafs::pin_registry::ManifestDigest::new([seed.wrapping_add(12); 32]);
    let order = iroha_data_model::sorafs::pin_registry::ReplicationOrderId::new(
        [seed.wrapping_add(13); 32],
    );
    archive.location_revision = 7;
    archive.location_ids = vec![location_id];
    let archive_id = archive.archive_id;
    let key = MusubiArchiveLocationKeyV1::new(archive_id, location_id);
    let mut location = location_fixture(seed, pin, order);
    location.archive_id = archive_id;
    location.location_id = location_id;
    location.revision = archive.location_revision;
    location.state = MusubiArchiveLocationStateV1::Degraded;
    let provider_keypair =
        KeyPair::try_from_seed(vec![seed.wrapping_add(14); 32], Algorithm::Ed25519)
            .expect("fixture provider keypair");
    let provider_owner = AccountId::new(provider_keypair.public_key().clone());
    let provider_id =
        iroha_data_model::sorafs::capacity::ProviderId::new([seed.wrapping_add(15); 32]);
    let completion_authority =
        iroha_data_model::sorafs::pin_registry::ProviderIngestCompletionAuthorityV1::new(
            provider_owner.clone(),
            iroha_data_model::sorafs::pin_registry::ProviderIngestCompletionSignerPolicyV1 {
                policy_id: [seed.wrapping_add(16); 32],
                revision: 1,
                predecessor_digest: None,
                policy_digest: [seed.wrapping_add(17); 32],
            },
        );
    let binding = MusubiProviderBundleVerificationBindingV1 {
        chain_id: archive.staging_receipt.payload.binding.chain_id.clone(),
        genesis_block_hash: archive.staging_receipt.payload.binding.genesis_block_hash,
        provider_id,
        completed_by: provider_owner.clone(),
        completion_authority,
        replication_order: order,
        assignment_revision: 1,
        completion_epoch: 1,
        finalized_anchor: iroha_data_model::sorafs::pin_registry::ProviderIngestFinalizedAnchorV1 {
            height: 1,
            block_hash: [seed.wrapping_add(18); 32],
        },
        archive_id,
        bundle_digest: archive.commitment.bundle_digest,
        descriptor_digest: archive.commitment.descriptor_digest,
        semantic_release_manifest_digest: archive
            .staging_receipt
            .payload
            .binding
            .semantic_release_manifest_digest,
        verification_lock_digest: MusubiVerificationLockDigestV1::new([seed.wrapping_add(19); 32]),
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
            .expect("sign fixture provider attestation"),
        }],
        payload,
    };
    attestation
        .verify(&binding)
        .expect("fixture provider attestation is cryptographically valid");
    location.providers = vec![provider_id];
    location.provider_attestations = vec![attestation];
    location
        .validate()
        .expect("fixture archive location is structurally valid");
    let instruction = AddMusubiArchiveLocationV1 {
        archive_id,
        location_id,
        pin_manifest: location.pin_manifest,
        replication_order: location.replication_order,
        provider_attestations: location.provider_attestations.clone(),
        renew_after_epoch: location.renew_after_epoch,
        expires_at_epoch: location.expires_at_epoch,
        expected_location_revision: 1,
    };
    let mut pin_record = iroha_data_model::sorafs::pin_registry::PinManifestRecord::new(
        pin,
        archive.commitment.root_cid.clone(),
        archive.commitment.chunker.clone(),
        *archive.commitment.chunk_plan_digest.as_bytes(),
        *archive.commitment.por_root.as_bytes(),
        archive.commitment.content_length,
        iroha_data_model::sorafs::pin_registry::PinPolicy {
            min_replicas: MUSUBI_MIN_HEALTHY_REPLICAS_V1,
            storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Hot,
            retention_epoch: location.expires_at_epoch,
        },
        authority.clone(),
        1,
        None,
        None,
        iroha_data_model::metadata::Metadata::default(),
    );
    pin_record.approve(1, None);
    world.pin_manifests.insert(pin, pin_record);
    world.replication_orders.insert(
        order,
        iroha_data_model::sorafs::pin_registry::ReplicationOrderRecord {
            order_id: order,
            manifest_digest: pin,
            manifest_root_cid: archive.commitment.root_cid.clone(),
            issued_by: authority.clone(),
            issued_epoch: 1,
            deadline_epoch: location.expires_at_epoch,
            canonical_order: vec![seed],
            assignment_revision: binding.assignment_revision,
            provider_completions: vec![
                iroha_data_model::sorafs::pin_registry::ReplicationOrderCompletionRecord {
                    provider_id,
                    completed_by: provider_owner.clone(),
                    completion_epoch: binding.completion_epoch,
                    assignment_revision: binding.assignment_revision,
                    completion_authority: binding.completion_authority.clone(),
                    finalized_anchor: binding.finalized_anchor,
                },
            ],
            status: ReplicationOrderStatus::Completed(binding.completion_epoch),
        },
    );
    world.provider_owners.insert(provider_id, provider_owner);
    world.musubi_locations_by_pin.insert(
        pin,
        MusubiPinLocationReferenceV1 {
            pin_manifest: pin,
            location: key,
            active: true,
        },
    );
    world.musubi_locations_by_replication_order.insert(
        order,
        MusubiReplicationOrderLocationReferenceV1 {
            replication_order: order,
            location: key,
            active: true,
        },
    );
    world
        .musubi_locations_by_provider
        .insert(MusubiProviderLocationKeyV1::new(provider_id, key), ());
    world.musubi_archives.insert(archive_id, archive);
    world.musubi_archive_locations.insert(key, location);
    (world, authority, key, instruction)
}

fn archive_location_replay_state(world: World) -> State {
    State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

fn archive_location_replay_block(state: &State) -> crate::state::StateBlock<'_> {
    let header = iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(2).expect("nonzero replay height"),
        None,
        None,
        None,
        0,
        0,
    );
    state.block(header)
}

fn assert_exact_archive_location_replay_rejects_corruption(
    seed: u8,
    corrupt: impl FnOnce(&mut World, MusubiArchiveLocationKeyV1, &mut AddMusubiArchiveLocationV1),
    expected_message: &str,
) {
    let (mut world, authority, key, mut instruction) = archive_location_replay_fixture(seed);
    corrupt(&mut world, key, &mut instruction);
    let archive_id = instruction.archive_id;
    let archive_before = world
        .musubi_archives
        .view()
        .get(&archive_id)
        .cloned()
        .expect("fixture archive");
    let location_before = world
        .musubi_archive_locations
        .view()
        .get(&key)
        .cloned()
        .expect("fixture location");
    let state = archive_location_replay_state(world);
    let mut block = archive_location_replay_block(&state);
    let mut transaction = block.transaction();

    let error = instruction
        .execute(&authority, &mut transaction)
        .expect_err("corrupt authoritative replay state must fail closed");

    assert!(
        error.to_string().contains(expected_message),
        "unexpected replay error: {error}"
    );
    assert_eq!(
        transaction.world.musubi_archives.get(&archive_id),
        Some(&archive_before)
    );
    assert_eq!(
        transaction.world.musubi_archive_locations.get(&key),
        Some(&location_before)
    );
    assert!(take_musubi_events(&mut transaction).is_empty());
}

#[test]
fn exact_archive_location_replay_ignores_stale_revision_without_mutation() {
    let seed = 0x41;
    let (world, authority, key, instruction) = archive_location_replay_fixture(seed);
    let archive_id = instruction.archive_id;
    let state = archive_location_replay_state(world);
    let mut block = archive_location_replay_block(&state);
    let mut transaction = block.transaction();
    let archive_before = transaction
        .world
        .musubi_archives
        .get(&archive_id)
        .cloned()
        .expect("fixture archive");
    let location_before = transaction
        .world
        .musubi_archive_locations
        .get(&key)
        .cloned()
        .expect("fixture location");

    instruction
        .execute(&authority, &mut transaction)
        .expect("an exact replay must not require the consumed CAS revision");

    assert_eq!(
        transaction.world.musubi_archives.get(&archive_id),
        Some(&archive_before)
    );
    assert_eq!(
        transaction.world.musubi_archive_locations.get(&key),
        Some(&location_before)
    );
    assert!(take_musubi_events(&mut transaction).is_empty());
}

#[test]
fn exact_archive_location_replay_rejects_a_malformed_stored_location() {
    let seed = 0x44;
    let (mut world, authority, key, instruction) = archive_location_replay_fixture(seed);
    let mut malformed = world
        .musubi_archive_locations
        .view()
        .get(&key)
        .cloned()
        .expect("fixture location");
    malformed.providers.clear();
    world
        .musubi_archive_locations
        .insert(key, malformed.clone());
    let state = archive_location_replay_state(world);
    let mut block = archive_location_replay_block(&state);
    let mut transaction = block.transaction();

    let error = instruction
        .execute(&authority, &mut transaction)
        .expect_err("an exact replay must not bless malformed authoritative state");

    assert!(error.to_string().contains("archive location is invalid"));
    assert_eq!(
        transaction.world.musubi_archive_locations.get(&key),
        Some(&malformed)
    );
    assert!(take_musubi_events(&mut transaction).is_empty());
}

#[test]
fn exact_archive_location_replay_rejects_a_future_location_revision() {
    assert_exact_archive_location_replay_rejects_corruption(
        0x4A,
        |world, key, _| {
            let mut location = world
                .musubi_archive_locations
                .view()
                .get(&key)
                .cloned()
                .expect("fixture location");
            location.revision = world
                .musubi_archives
                .view()
                .get(&key.archive_id)
                .expect("fixture archive")
                .location_revision
                .checked_add(1)
                .expect("fixture revision remains bounded");
            world.musubi_archive_locations.insert(key, location);
        },
        "location revision or finalized height is inconsistent",
    );
}

#[test]
fn exact_archive_location_replay_rejects_mismatched_immutable_attestation_evidence() {
    assert_exact_archive_location_replay_rejects_corruption(
        0x4B,
        |world, key, instruction| {
            let mut location = world
                .musubi_archive_locations
                .view()
                .get(&key)
                .cloned()
                .expect("fixture location");
            location.provider_attestations[0]
                .payload
                .binding
                .bundle_digest = MusubiContentDigestV1::new([0xEE; 32]);
            instruction.provider_attestations = location.provider_attestations.clone();
            world.musubi_archive_locations.insert(key, location);
        },
        "attestation does not match its immutable archive commitments",
    );
}

#[test]
fn exact_archive_location_replay_rejects_an_invalid_stored_attestation_signature() {
    assert_exact_archive_location_replay_rejects_corruption(
        0x4C,
        |world, key, instruction| {
            let mut location = world
                .musubi_archive_locations
                .view()
                .get(&key)
                .cloned()
                .expect("fixture location");
            let foreign_keypair = KeyPair::try_from_seed(vec![0xFD; 32], Algorithm::Ed25519)
                .expect("foreign fixture keypair");
            location.provider_attestations[0].approvals[0].public_key =
                foreign_keypair.public_key().clone();
            instruction.provider_attestations = location.provider_attestations.clone();
            world.musubi_archive_locations.insert(key, location);
        },
        "approval is not a provider-owner key",
    );
}

#[test]
fn exact_archive_location_replay_rejects_a_missing_pin_reverse_reference() {
    assert_exact_archive_location_replay_rejects_corruption(
        0x45,
        |world, _, instruction| {
            let mut locations_by_pin = world.musubi_locations_by_pin.block();
            let _ = locations_by_pin.remove(instruction.pin_manifest);
            locations_by_pin.commit();
        },
        "pin reverse index is inconsistent",
    );
}

#[test]
fn exact_archive_location_replay_rejects_an_inactive_order_reverse_reference() {
    assert_exact_archive_location_replay_rejects_corruption(
        0x46,
        |world, _, instruction| {
            let mut reference = world
                .musubi_locations_by_replication_order
                .view()
                .get(&instruction.replication_order)
                .copied()
                .expect("fixture order reverse reference");
            reference.active = false;
            world
                .musubi_locations_by_replication_order
                .insert(instruction.replication_order, reference);
        },
        "order reverse index is inconsistent",
    );
}

#[test]
fn exact_archive_location_replay_rejects_a_missing_provider_reverse_reference() {
    assert_exact_archive_location_replay_rejects_corruption(
        0x47,
        |world, key, instruction| {
            let provider = instruction.provider_attestations[0]
                .payload
                .binding
                .provider_id;
            let mut locations_by_provider = world.musubi_locations_by_provider.block();
            let _ = locations_by_provider.remove(MusubiProviderLocationKeyV1::new(provider, key));
            locations_by_provider.commit();
        },
        "provider reverse index is inconsistent",
    );
}

#[test]
fn exact_archive_location_replay_ignores_mutable_sorafs_degradation() {
    let seed = 0x48;
    let (mut world, authority, key, instruction) = archive_location_replay_fixture(seed);
    let archive_id = instruction.archive_id;
    let mut pin = world
        .pin_manifests
        .view()
        .get(&instruction.pin_manifest)
        .cloned()
        .expect("fixture pin manifest");
    pin.retire(2, Some("fixture lifecycle degradation".to_owned()));
    world.pin_manifests.insert(instruction.pin_manifest, pin);
    let mut order = world
        .replication_orders
        .view()
        .get(&instruction.replication_order)
        .cloned()
        .expect("fixture replication order");
    order.status = ReplicationOrderStatus::Expired(2);
    world
        .replication_orders
        .insert(instruction.replication_order, order);
    let provider = instruction.provider_attestations[0]
        .payload
        .binding
        .provider_id;
    let mut provider_owners = world.provider_owners.block();
    let _ = provider_owners.remove(provider);
    provider_owners.commit();
    let archive_before = world
        .musubi_archives
        .view()
        .get(&archive_id)
        .cloned()
        .expect("fixture archive");
    let location_before = world
        .musubi_archive_locations
        .view()
        .get(&key)
        .cloned()
        .expect("fixture location");
    let state = archive_location_replay_state(world);
    let mut block = archive_location_replay_block(&state);
    let mut transaction = block.transaction();

    instruction
        .execute(&authority, &mut transaction)
        .expect("later mutable SoraFS degradation must not invalidate an exact replay");

    assert_eq!(
        transaction.world.musubi_archives.get(&archive_id),
        Some(&archive_before)
    );
    assert_eq!(
        transaction.world.musubi_archive_locations.get(&key),
        Some(&location_before)
    );
    assert!(take_musubi_events(&mut transaction).is_empty());
}

#[test]
fn changed_archive_location_replay_still_requires_current_revision() {
    let seed = 0x42;
    let (world, authority, key, mut instruction) = archive_location_replay_fixture(seed);
    let archive_id = instruction.archive_id;
    instruction.expires_at_epoch = instruction
        .expires_at_epoch
        .checked_add(1)
        .expect("fixture expiry remains bounded");
    let state = archive_location_replay_state(world);
    let mut block = archive_location_replay_block(&state);
    let mut transaction = block.transaction();
    let archive_before = transaction
        .world
        .musubi_archives
        .get(&archive_id)
        .cloned()
        .expect("fixture archive");
    let location_before = transaction
        .world
        .musubi_archive_locations
        .get(&key)
        .cloned()
        .expect("fixture location");

    let error = instruction
        .execute(&authority, &mut transaction)
        .expect_err("changed location content must not bypass compare-and-set");

    assert!(
        error
            .to_string()
            .contains("stale Musubi archive location revision")
    );
    assert_eq!(
        transaction.world.musubi_archives.get(&archive_id),
        Some(&archive_before)
    );
    assert_eq!(
        transaction.world.musubi_archive_locations.get(&key),
        Some(&location_before)
    );
    assert!(take_musubi_events(&mut transaction).is_empty());
}

#[test]
fn retired_archive_location_identity_rejects_exact_stale_replay() {
    let seed = 0x43;
    let (mut world, authority, key, instruction) = archive_location_replay_fixture(seed);
    let mut retired = world
        .musubi_archive_locations
        .view()
        .get(&key)
        .cloned()
        .expect("fixture location");
    retired.state = MusubiArchiveLocationStateV1::Retired;
    world.musubi_archive_locations.insert(key, retired.clone());
    let state = archive_location_replay_state(world);
    let mut block = archive_location_replay_block(&state);
    let mut transaction = block.transaction();

    let error = instruction
        .execute(&authority, &mut transaction)
        .expect_err("a retired location identity must never be replayed or reused");

    assert!(
        error
            .to_string()
            .contains("retired archive location identities")
    );
    assert_eq!(
        transaction.world.musubi_archive_locations.get(&key),
        Some(&retired)
    );
    assert!(take_musubi_events(&mut transaction).is_empty());
}

#[test]
fn replication_shortfall_transition_is_checked_and_boundary_scoped() {
    use MusubiStorageAvailabilityV1::{BelowQuorum, Selectable, Unavailable};

    assert_eq!(
        plan_replication_shortfall_transition(5, Selectable, BelowQuorum, 3)
            .expect("selectable-to-shortfall transition"),
        Some(8)
    );
    assert_eq!(
        plan_replication_shortfall_transition(5, Selectable, Unavailable, 3)
            .expect("selectable-to-unavailable transition"),
        Some(8)
    );
    assert_eq!(
        plan_replication_shortfall_transition(5, BelowQuorum, Selectable, 3)
            .expect("shortfall-to-selectable transition"),
        Some(2)
    );
    assert_eq!(
        plan_replication_shortfall_transition(5, Unavailable, Selectable, 3)
            .expect("unavailable-to-selectable transition"),
        Some(2)
    );
    assert_eq!(
        plan_replication_shortfall_transition(5, BelowQuorum, Unavailable, 3)
            .expect("non-selectable transition"),
        None
    );
    assert_eq!(
        plan_replication_shortfall_transition(5, Selectable, BelowQuorum, 0)
            .expect("empty reverse-reference transition"),
        None
    );
    assert!(
        plan_replication_shortfall_transition(u64::MAX, Selectable, BelowQuorum, 1).is_err(),
        "consensus aggregate overflow must fail closed"
    );
    assert!(
        plan_replication_shortfall_transition(0, Unavailable, Selectable, 1).is_err(),
        "consensus aggregate underflow must fail closed"
    );
}

#[test]
fn availability_refresh_preflights_resolver_rows_and_packages_before_mutation() {
    let mut world = World::new();
    let archive = retention_archive(17);
    let archive_id = archive.archive_id;
    let source_digest = archive.commitment.source_tree_digest;
    let release = retention_release(
        archive_id,
        "1.0.0",
        false,
        MusubiArtifactGovernanceStateV1::Available,
    );
    let release_id = release.manifest.release.clone();
    let resolver_release = release.clone();
    seed_retention_archive(&mut world, archive, vec![release]);
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(2).expect("nonzero fixture height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    let availability_before = *transaction
        .world
        .musubi_archive_availability
        .get(&archive_id)
        .expect("fixture archive has availability");
    let index_revision_before = transaction.world.musubi_resolver_index_revision.get().get();
    let shortfall_before = *transaction
        .world
        .musubi_replication_shortfall_releases
        .get();

    let error = refresh_archive_availability(archive_id, &mut transaction)
        .expect_err("a reverse-referenced release must have an exact resolver row");

    assert!(error.to_string().contains("missing its exact resolver row"));
    assert!(
        transaction
            .world
            .musubi_resolver_index
            .get(&release_id)
            .is_none()
    );
    assert_eq!(
        transaction
            .world
            .musubi_archive_availability
            .get(&archive_id),
        Some(&availability_before)
    );
    assert_eq!(
        transaction.world.musubi_resolver_index_revision.get().get(),
        index_revision_before
    );
    assert_eq!(
        *transaction
            .world
            .musubi_replication_shortfall_releases
            .get(),
        shortfall_before
    );

    let row = MusubiResolverReleaseRowV1 {
        release: release_id.clone(),
        release_digest: resolver_release.release_digest,
        archive_id,
        source_digest,
        interface_digest: resolver_release.manifest.interface_digest,
        abi: resolver_release.manifest.abi,
        dependencies: resolver_release.manifest.dependencies.clone(),
        selection: MusubiReleaseSelectionStateV1 {
            yank: resolver_release.yank,
            storage: availability_before,
            governance: resolver_release.artifact_governance,
        },
        index_revision: index_revision_before,
    };
    row.validate().expect("fixture resolver row is canonical");
    transaction
        .world
        .musubi_resolver_index
        .insert(release_id, row);

    let error = refresh_archive_availability(archive_id, &mut transaction)
        .expect_err("a reverse-referenced release must retain its package record");
    assert!(error.to_string().contains("missing package record"));
    assert_eq!(
        transaction
            .world
            .musubi_archive_availability
            .get(&archive_id),
        Some(&availability_before)
    );
    assert_eq!(
        transaction.world.musubi_resolver_index_revision.get().get(),
        index_revision_before
    );
    assert_eq!(
        *transaction
            .world
            .musubi_replication_shortfall_releases
            .get(),
        shortfall_before
    );
}

#[test]
fn availability_refresh_rejects_an_invalid_archive_before_mutation() {
    let mut world = World::new();
    let mut archive = retention_archive(18);
    let archive_id = archive.archive_id;
    archive.location_revision = 0;
    seed_retention_archive(&mut world, archive.clone(), Vec::new());
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(2).expect("nonzero fixture height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    let availability_before = *transaction
        .world
        .musubi_archive_availability
        .get(&archive_id)
        .expect("fixture archive has availability");
    let index_revision_before = transaction.world.musubi_resolver_index_revision.get().get();

    let error = refresh_archive_availability(archive_id, &mut transaction)
        .expect_err("an invalid authoritative archive must fail closed");

    assert!(error.to_string().contains("archive record"));
    assert_eq!(
        transaction.world.musubi_archives.get(&archive_id),
        Some(&archive)
    );
    assert_eq!(
        transaction
            .world
            .musubi_archive_availability
            .get(&archive_id),
        Some(&availability_before)
    );
    assert_eq!(
        transaction.world.musubi_resolver_index_revision.get().get(),
        index_revision_before
    );
    assert!(take_musubi_events(&mut transaction).is_empty());
}

#[test]
fn availability_refresh_rejects_a_mismatched_archive_identity_before_mutation() {
    let mut world = World::new();
    let canonical = retention_archive(19);
    let archive_id = canonical.archive_id;
    let mismatched = retention_archive(20);
    mismatched
        .validate()
        .expect("mismatched archive fixture is structurally valid");
    seed_retention_archive(&mut world, canonical, Vec::new());
    world.musubi_archives.insert(archive_id, mismatched.clone());
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(2).expect("nonzero fixture height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    let availability_before = *transaction
        .world
        .musubi_archive_availability
        .get(&archive_id)
        .expect("fixture archive has availability");
    let index_revision_before = transaction.world.musubi_resolver_index_revision.get().get();

    let error = refresh_archive_availability(archive_id, &mut transaction)
        .expect_err("an archive stored under another identity must fail closed");

    assert!(
        error
            .to_string()
            .contains("wrong embedded archive identity")
    );
    assert_eq!(
        transaction.world.musubi_archives.get(&archive_id),
        Some(&mismatched)
    );
    assert_eq!(
        transaction
            .world
            .musubi_archive_availability
            .get(&archive_id),
        Some(&availability_before)
    );
    assert_eq!(
        transaction.world.musubi_resolver_index_revision.get().get(),
        index_revision_before
    );
    assert!(take_musubi_events(&mut transaction).is_empty());
}

#[test]
fn availability_refresh_preflights_location_validation_and_identity() {
    let mut world = World::new();
    let mut archive = retention_archive(21);
    let archive_id = archive.archive_id;
    let location_id = MusubiArchiveLocationIdV1::new([0x51; 32]);
    archive.location_ids = vec![location_id];
    archive
        .validate()
        .expect("archive with one location identity is valid");
    seed_retention_archive(&mut world, archive.clone(), Vec::new());
    let key = MusubiArchiveLocationKeyV1::new(archive_id, location_id);
    let mut location = location_fixture(
        0x51,
        iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0x52; 32]),
        iroha_data_model::sorafs::pin_registry::ReplicationOrderId::new([0x53; 32]),
    );
    location.archive_id = archive_id;
    location.location_id = location_id;
    world.musubi_archive_locations.insert(key, location.clone());
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(2).expect("nonzero fixture height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    let availability_before = *transaction
        .world
        .musubi_archive_availability
        .get(&archive_id)
        .expect("fixture archive has availability");
    let index_revision_before = transaction.world.musubi_resolver_index_revision.get().get();

    let invalid = refresh_archive_availability(archive_id, &mut transaction)
        .expect_err("a malformed location must fail before availability changes");
    assert!(invalid.to_string().contains("archive location is invalid"));
    assert_eq!(
        transaction.world.musubi_archive_locations.get(&key),
        Some(&location)
    );
    assert_eq!(
        transaction
            .world
            .musubi_archive_availability
            .get(&archive_id),
        Some(&availability_before)
    );
    assert_eq!(
        transaction.world.musubi_resolver_index_revision.get().get(),
        index_revision_before
    );
    assert!(take_musubi_events(&mut transaction).is_empty());

    location.location_id = MusubiArchiveLocationIdV1::new([0x54; 32]);
    transaction
        .world
        .musubi_archive_locations
        .insert(key, location.clone());
    let mismatched = refresh_archive_availability(archive_id, &mut transaction)
        .expect_err("a location stored under another identity must fail closed");
    assert!(mismatched.to_string().contains("wrong embedded identity"));
    assert_eq!(
        transaction.world.musubi_archive_locations.get(&key),
        Some(&location)
    );
    assert_eq!(
        transaction
            .world
            .musubi_archive_availability
            .get(&archive_id),
        Some(&availability_before)
    );
    assert_eq!(
        transaction.world.musubi_resolver_index_revision.get().get(),
        index_revision_before
    );
    assert!(take_musubi_events(&mut transaction).is_empty());
}

#[test]
fn archive_retention_finalized_time_requires_the_exact_snapshot_block() {
    let snapshot = MusubiRegistrySnapshotV1 {
        finalized_height: 7,
        finalized_block_hash: [0x71; 32],
        index_revision: 9,
    };
    assert_eq!(
        validated_finalized_block_time(&snapshot, 7, [0x71; 32], 1_700_000_000_000)
            .expect("exact finalized block"),
        1_700_000_000_000
    );
    assert!(validated_finalized_block_time(&snapshot, 8, [0x71; 32], 1).is_err());
    assert!(validated_finalized_block_time(&snapshot, 7, [0x72; 32], 1).is_err());
}

#[test]
fn archive_retention_point_lookups_keep_active_yanked_and_unknown_archives() {
    let mut world = World::new();
    let unreferenced = seed_retention_archive(&mut world, retention_archive(11), Vec::new());
    let referenced_archive = retention_archive(21);
    let referenced = referenced_archive.archive_id;
    let takedown = |seed| {
        MusubiArtifactGovernanceStateV1::TakenDown(MusubiArtifactTakedownV1 {
            action_digest: MusubiGovernanceActionDigestV1::new([seed; 32]),
            reason: "Parliament fixture".parse().expect("takedown reason"),
            applied_at_height: 1,
        })
    };
    seed_retention_archive(
        &mut world,
        referenced_archive,
        vec![
            retention_release(
                referenced,
                "1.0.0",
                false,
                MusubiArtifactGovernanceStateV1::Available,
            ),
            retention_release(
                referenced,
                "1.1.0",
                true,
                MusubiArtifactGovernanceStateV1::Available,
            ),
            retention_release(referenced, "1.2.0", false, takedown(31)),
        ],
    );
    let taken_down_archive = retention_archive(41);
    let taken_down = taken_down_archive.archive_id;
    seed_retention_archive(
        &mut world,
        taken_down_archive,
        vec![retention_release(taken_down, "2.0.0", true, takedown(42))],
    );
    let view = world.view();

    let unknown =
        archive_retention_decision(ArchiveId::new([0xF1; 32]), &view).expect("unknown decision");
    assert_eq!(
        unknown.disposition,
        MusubiArchiveRetentionDispositionV1::RetainUnknown
    );
    assert!(unknown.must_retain());

    let unreferenced =
        archive_retention_decision(unreferenced, &view).expect("unreferenced decision");
    assert_eq!(
        unreferenced.disposition,
        MusubiArchiveRetentionDispositionV1::PruneUnreferenced
    );

    let referenced = archive_retention_decision(referenced, &view).expect("referenced decision");
    assert_eq!(
        referenced.disposition,
        MusubiArchiveRetentionDispositionV1::RetainReferenced
    );
    assert_eq!(referenced.active_releases, 1);
    assert_eq!(referenced.yanked_releases, 1);
    assert_eq!(referenced.taken_down_releases, 1);
    assert!(referenced.must_retain());

    let taken_down = archive_retention_decision(taken_down, &view).expect("taken-down decision");
    assert_eq!(
        taken_down.disposition,
        MusubiArchiveRetentionDispositionV1::PruneGovernedTakedown
    );
    assert_eq!(taken_down.taken_down_releases, 1);
    assert!(!taken_down.must_retain());
}

#[test]
fn archive_retention_point_lookups_reject_projection_identity_mismatches() {
    let mut world = World::new();
    let archive = retention_archive(51);
    let archive_id = seed_retention_archive(&mut world, archive.clone(), Vec::new());
    let other_archive_id = retention_archive(52).archive_id;
    let valid_storage = world
        .musubi_archive_availability
        .view()
        .get(&archive_id)
        .cloned()
        .expect("seeded archive availability");

    world
        .musubi_archives
        .insert(archive_id, retention_archive(52));
    assert!(archive_retention_decision(archive_id, &world.view()).is_err());
    world.musubi_archives.insert(archive_id, archive.clone());

    world.musubi_archive_reverse_references.insert(
        archive_id,
        MusubiArchiveReverseReferencesV1 {
            archive_id: other_archive_id,
            releases: Vec::new(),
        },
    );
    assert!(archive_retention_decision(archive_id, &world.view()).is_err());
    world.musubi_archive_reverse_references.insert(
        archive_id,
        MusubiArchiveReverseReferencesV1 {
            archive_id,
            releases: Vec::new(),
        },
    );

    let mut mismatched_storage = valid_storage.clone();
    mismatched_storage.archive_id = other_archive_id;
    world
        .musubi_archive_availability
        .insert(archive_id, mismatched_storage);
    assert!(archive_retention_decision(archive_id, &world.view()).is_err());
    world
        .musubi_archive_availability
        .insert(archive_id, valid_storage);

    let missing_release = retention_release(
        archive_id,
        "1.0.0",
        false,
        MusubiArtifactGovernanceStateV1::Available,
    )
    .manifest
    .release;
    world.musubi_archive_reverse_references.insert(
        archive_id,
        MusubiArchiveReverseReferencesV1 {
            archive_id,
            releases: vec![missing_release],
        },
    );
    assert!(archive_retention_decision(archive_id, &world.view()).is_err());

    let mut mismatched_release = retention_release(
        archive_id,
        "2.0.0",
        false,
        MusubiArtifactGovernanceStateV1::Available,
    );
    let referenced_release = mismatched_release.manifest.release.clone();
    mismatched_release.manifest.release = MusubiReleaseIdV1::new(
        package("retention"),
        "2.1.0".parse().expect("mismatched release version"),
    );
    mismatched_release.yank.release = mismatched_release.manifest.release.clone();
    mismatched_release.release_digest = mismatched_release.manifest.release_digest();
    world
        .musubi_releases
        .insert(referenced_release.clone(), mismatched_release);
    world.musubi_archive_reverse_references.insert(
        archive_id,
        MusubiArchiveReverseReferencesV1 {
            archive_id,
            releases: vec![referenced_release],
        },
    );
    assert!(archive_retention_decision(archive_id, &world.view()).is_err());

    let mut wrong_archive_release = retention_release(
        archive_id,
        "3.0.0",
        true,
        MusubiArtifactGovernanceStateV1::Available,
    );
    let referenced_release = wrong_archive_release.manifest.release.clone();
    wrong_archive_release.manifest.archive_id = other_archive_id;
    wrong_archive_release.release_digest = wrong_archive_release.manifest.release_digest();
    world
        .musubi_releases
        .insert(referenced_release.clone(), wrong_archive_release);
    world.musubi_archive_reverse_references.insert(
        archive_id,
        MusubiArchiveReverseReferencesV1 {
            archive_id,
            releases: vec![referenced_release],
        },
    );
    assert!(archive_retention_decision(archive_id, &world.view()).is_err());
}

#[test]
fn pagination_continues_after_exact_last_key() {
    let request = MusubiPageRequestV1 {
        limit: 2,
        cursor: None,
    };
    let hash = query_hash(b"test", b"request");
    let rows = vec![
        ("a".to_owned(), 1_u8),
        ("b".to_owned(), 2_u8),
        ("c".to_owned(), 3_u8),
    ];
    let (first, cursor) = paginate(rows.clone(), &request, hash, snapshot(1)).expect("first page");
    assert_eq!(first, vec![1, 2]);
    let cursor = cursor.expect("continuation");
    assert_eq!(cursor.last_key, "b");

    let request = MusubiPageRequestV1 {
        limit: 2,
        cursor: Some(cursor),
    };
    let (second, cursor) = paginate(rows, &request, hash, snapshot(1)).expect("second page");
    assert_eq!(second, vec![3]);
    assert!(cursor.is_none());
}

#[test]
fn pagination_preserves_exact_cursor_failure_reasons() {
    fn assert_reason<T>(
        result: Result<(Vec<T>, Option<MusubiFinalizedCursorV1>), MusubiQueryExecutionErrorV1>,
        expected: MusubiCursorFailureV1,
    ) {
        let error = match result {
            Ok(_) => panic!("cursor must fail"),
            Err(error) => error,
        };
        assert_eq!(error.cursor_failure(), Some(expected));
        assert_eq!(error.into_query_error(), QueryExecutionFail::Expired);
    }

    let hash = query_hash(b"test", b"request");
    let cursor = |cursor_snapshot, query_hash, last_key: &str, caller| MusubiFinalizedCursorV1 {
        snapshot: cursor_snapshot,
        query_hash,
        last_key: last_key.to_owned(),
        caller,
    };
    let page = |cursor| MusubiPageRequestV1 {
        limit: 1,
        cursor: Some(cursor),
    };

    let mut changed_anchor = snapshot(1);
    changed_anchor.finalized_height += 1;
    assert_reason(
        paginate(
            vec![("a".to_owned(), 1_u8)],
            &page(cursor(snapshot(1), hash, "a", None)),
            hash,
            changed_anchor,
        ),
        MusubiCursorFailureV1::FinalizedAnchorMismatch,
    );
    assert_reason(
        paginate(
            vec![("a".to_owned(), 1_u8)],
            &page(cursor(snapshot(1), hash, "a", None)),
            hash,
            snapshot(2),
        ),
        MusubiCursorFailureV1::IndexRevisionMismatch,
    );
    assert_reason(
        paginate(
            vec![("a".to_owned(), 1_u8)],
            &page(cursor(
                snapshot(1),
                query_hash(b"other", b"request"),
                "a",
                None,
            )),
            hash,
            snapshot(1),
        ),
        MusubiCursorFailureV1::QueryMismatch,
    );
    let expected_caller = account(1);
    assert_reason(
        paginate_for_caller(
            vec![("a".to_owned(), 1_u8)],
            &page(cursor(snapshot(1), hash, "a", Some(account(2)))),
            hash,
            snapshot(1),
            Some(&expected_caller),
        ),
        MusubiCursorFailureV1::CallerMismatch,
    );
    assert_reason(
        paginate(
            vec![("a".to_owned(), 1_u8)],
            &page(cursor(snapshot(1), hash, "missing", None)),
            hash,
            snapshot(1),
        ),
        MusubiCursorFailureV1::LastKeyStale,
    );

    let invalid_version_cursor = page(cursor(snapshot(1), hash, "01.0.0", None));
    let error = package_release_page_start(&package("cursor-test"), &invalid_version_cursor)
        .expect_err("noncanonical version boundary must fail");
    assert_eq!(
        error.cursor_failure(),
        Some(MusubiCursorFailureV1::LastKeyStale)
    );
}

#[test]
fn query_hash_is_domain_separated() {
    assert_ne!(
        query_hash(b"versions", b"same"),
        query_hash(b"maintainers", b"same")
    );
    assert_ne!(
        query_hash(b"versions", b"same"),
        query_hash(b"versions", b"different")
    );
}
