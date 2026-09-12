// Canonical frame identity, archive persistence, and strict decode coverage.
#[test]
fn retention_authority_binding_rejects_test_stale_and_substituted_identity() {
    assert!(matches!(
        ReputationFinalizedArchiveRetentionAuthorityBindingV1::try_new(
            "sealed.reputation.archive.test".to_owned(),
            7,
            [0xA7; 32],
        ),
        Err(ReputationFinalizedArchiveError::InvalidRetentionAuthorityBinding)
    ));
    assert!(matches!(
        ReputationFinalizedArchiveRetentionAuthorityBindingV1::try_new(
            "sealed.reputation.archive.primary".to_owned(),
            0,
            [0xA7; 32],
        ),
        Err(ReputationFinalizedArchiveError::InvalidRetentionAuthorityBinding)
    ));
    let expected = TestRetentionAuthority::new();
    let binding = expected.binding();
    let mut substituted = TestRetentionAuthority::new();
    substituted.handle = "sealed.reputation.archive.secondary".to_owned();
    assert!(matches!(
        assert_retention_authority_identity(&binding, &substituted),
        Err(ReputationFinalizedArchiveError::RetentionAuthoritySubstitution)
    ));
    let mut stale = TestRetentionAuthority::new();
    stale.qualification =
        ReputationFinalizedArchiveRetentionAuthorityQualificationV1::new(8, [0xA7; 32]);
    assert!(matches!(
        assert_retention_authority_identity(&binding, &stale),
        Err(ReputationFinalizedArchiveError::RetentionAuthoritySubstitution)
    ));
}
#[test]
fn retention_approval_codec_and_cas_readback_fail_closed() {
    let authority = TestRetentionAuthority::new();
    let binding = authority.binding();
    let proposal = retention_test_proposal(1, 0x31);
    let approval = ReputationFinalizedArchiveRetentionApprovalRecordV1::try_new(
        1,
        binding.qualification(),
        proposal,
        None,
        None,
    )
    .expect("valid first approval");
    let canonical = approval.to_canonical_bytes().expect("encode approval");
    assert_eq!(
        ReputationFinalizedArchiveRetentionApprovalRecordV1::from_canonical_bytes(&canonical)
            .expect("decode canonical approval"),
        approval
    );
    let mut trailing = canonical;
    trailing.push(0);
    assert!(
        ReputationFinalizedArchiveRetentionApprovalRecordV1::from_canonical_bytes(&trailing)
            .is_err()
    );
    assert!(
        ReputationFinalizedArchiveRetentionApprovalRecordV1::from_canonical_bytes(&vec![
            0;
            RETENTION_APPROVAL_MAX_CANONICAL_BYTES_V1
                + 1
        ])
        .is_err()
    );
    authority.set_behavior(TestRetentionCasBehavior::ApplyAmbiguous);
    compare_and_read_back_retention_approval(
        &binding,
        &authority,
        &proposal_network_id(&approval),
        None,
        &approval,
    )
    .expect("applied ambiguous CAS is proven by exact readback");
    compare_and_read_back_retention_approval(
        &binding,
        &authority,
        &proposal_network_id(&approval),
        None,
        &approval,
    )
    .expect("replica that loses an identical CAS converges by exact readback");
    let unchanged = TestRetentionAuthority::new();
    unchanged.set_behavior(TestRetentionCasBehavior::LeaveUnchanged);
    assert!(matches!(
        compare_and_read_back_retention_approval(
            &unchanged.binding(),
            &unchanged,
            &proposal_network_id(&approval),
            None,
            &approval,
        ),
        Err(ReputationFinalizedArchiveError::RetentionAuthorityCasUnchanged)
    ));
    let equivocation = TestRetentionAuthority::new();
    equivocation.set_behavior(TestRetentionCasBehavior::Equivocate);
    let competing_proposal = retention_test_proposal(1, 0x41);
    let competing = ReputationFinalizedArchiveRetentionApprovalRecordV1::try_new(
        1,
        equivocation.binding().qualification(),
        competing_proposal,
        None,
        None,
    )
    .expect("valid competing approval");
    equivocation.set_competing(competing);
    assert!(matches!(
        compare_and_read_back_retention_approval(
            &equivocation.binding(),
            &equivocation,
            &proposal_network_id(&approval),
            None,
            &approval,
        ),
        Err(ReputationFinalizedArchiveError::RetentionAuthorityEquivocation)
    ));
}
#[test]
fn persisted_record_is_byte_canonical_norito() {
    let directory = tempdir().expect("create archive directory");
    let archive = open_archive(&directory, bounds());
    let projection = sample_projection(7, [0x71; 32]);
    archive
        .insert(projection.clone())
        .expect("insert projection");
    let bytes = fs::read(
        archive
            .record_path(&projection.key)
            .expect("derive record path"),
    )
    .expect("read canonical record");
    let decoded: PersistedReputationFinalizedAnchorV1 =
        decode_from_bytes_with_limits(&bytes, bounds().decode_limits())
            .expect("decode canonical record");
    assert_eq!(
        norito::to_bytes(&decoded).expect("re-encode canonical record"),
        bytes
    );
    assert_eq!(decoded.manifest.key, projection.key);
    assert_eq!(
        decoded.manifest.high_water_marks,
        ReputationFeedHighWaterMarksV1::default()
    );
    assert_eq!(decoded.manifest.journal_source_head_count, 0);
    assert_eq!(
        decoded.manifest.journal_source_head_root,
        journal_prefix_source_head_root(&[]).expect("digest empty source-head set")
    );
    assert_eq!(decoded.delta, ReputationFinalizedAnchorDeltaV1::default());
}

macro_rules! assert_reputation_query_frame {
    ($value:expr, $type:ident) => {{
        let value: &$type = &$value;
        let bytes = norito::to_bytes(value).expect("encode declared query frame");
        assert_eq!(
            norito::core::Header::read(bytes.as_slice())
                .expect("read frame header")
                .schema,
            norito::core::schema_hash_for_name(concat!(
                "iroha_core::query::reputation_finalized::",
                stringify!($type)
            ))
        );
        let decoded =
            norito::decode_from_bytes::<$type>(&bytes).expect("decode declared query frame");
        assert_eq!(&decoded, value);
        assert_eq!(
            norito::to_bytes(&decoded).expect("re-encode canonical frame"),
            bytes
        );
        assert!(matches!(
            norito::decode_from_bytes::<u64>(&bytes),
            Err(norito::Error::SchemaMismatch)
        ));
        bytes
    }};
}

#[test]
fn persisted_anchor_policy_and_projection_frames_use_exact_logical_identities() {
    let directory = tempdir().expect("archive tempdir");
    let archive = open_archive(&directory, bounds());
    let projection = sample_projection(7, [0x71; 32]);
    archive
        .insert(projection.clone())
        .expect("persist projection");
    let path = archive.record_path(&projection.key).expect("anchor path");
    let anchor = archive
        .load_anchor_at(&path, Some(&projection.key))
        .expect("load persisted anchor");
    let bytes = assert_reputation_query_frame!(anchor, PersistedReputationFinalizedAnchorV1);
    assert_eq!(bytes, fs::read(&path).expect("read actual anchor frame"));
    let manifest_bytes =
        assert_reputation_query_frame!(anchor.manifest, ReputationFinalizedAnchorManifestV1);
    assert_reputation_query_frame!(anchor.delta, ReputationFinalizedAnchorDeltaV1);
    assert_reputation_query_frame!(projection.key, ReputationFinalizedArchiveKeyV1);
    assert_reputation_query_frame!(projection, ReputationFinalizedProjectionV1);
    let digest_material = ReputationFinalizedAnchorDigestMaterialV1 {
        version: anchor.version,
        manifest_digest: anchor.manifest_digest,
        delta_digest: anchor.delta_digest,
    };
    let material_bytes =
        norito::to_bytes(&digest_material).expect("encode actual anchor commitment material");
    assert_eq!(
        norito::core::Header::read(material_bytes.as_slice())
            .expect("read commitment frame")
            .schema,
        norito::core::schema_hash_for_name(
            "iroha_core::query::reputation_finalized::ReputationFinalizedAnchorDigestMaterialV1"
        )
    );
    assert_eq!(
        anchor.anchor_digest().expect("anchor digest"),
        canonical_domain_digest(ANCHOR_DIGEST_DOMAIN_V1, &digest_material)
            .expect("digest exact material")
    );
    assert!(matches!(
        norito::decode_from_bytes::<PersistedReputationFinalizedAnchorV1>(&material_bytes),
        Err(norito::Error::SchemaMismatch)
    ));
    let policy_path = archive
        .policies
        .join(policy_file_name(anchor.manifest.policy_record_digest));
    let policy = archive
        .load_policy_at(&policy_path, Some(anchor.manifest.policy_record_digest))
        .expect("load persisted policy");
    let policy_bytes = assert_reputation_query_frame!(policy, PersistedReputationAuthorityPolicyV1);
    assert_eq!(
        policy_bytes,
        fs::read(&policy_path).expect("read actual policy frame")
    );
    drop(archive);
    let reopened = open_archive(&directory, bounds());
    assert!(
        reopened
            .get_exact(&projection.key)
            .expect("reconstruct canonical anchor")
            .is_some()
    );
    fs::write(&policy_path, &bytes).expect("substitute valid anchor frame for policy");
    assert!(matches!(
        reopened.load_policy_at(&policy_path, None),
        Err(ReputationFinalizedArchiveError::Decode {
            source: norito::Error::SchemaMismatch,
            ..
        })
    ));
    fs::write(&path, manifest_bytes).expect("substitute valid manifest frame for anchor");
    assert!(matches!(
        reopened.load_anchor_at(&path, Some(&projection.key)),
        Err(ReputationFinalizedArchiveError::Decode {
            source: norito::Error::SchemaMismatch,
            ..
        })
    ));
    drop(reopened);
    assert!(ReputationFinalizedArchive::try_open(archive_root(&directory), bounds()).is_err());
}

#[test]
fn checkpoint_and_validation_frames_are_canonical_and_reject_wrong_roots() {
    let directory = tempdir().expect("archive tempdir");
    let archive = open_archive(&directory, bounds());
    let projection = sample_projection(7, [0x71; 32]);
    archive
        .insert(projection.clone())
        .expect("persist checkpoint source");
    let (persisted, bytes, path) = test_checkpoint_artifact(&archive, &projection.key);
    assert_eq!(
        assert_reputation_query_frame!(
            persisted,
            PersistedReputationFinalizedVirtualBaseCheckpointV1
        ),
        bytes
    );
    let checkpoint_bytes = assert_reputation_query_frame!(
        persisted.checkpoint,
        ReputationFinalizedVirtualBaseCheckpointV1
    );
    assert_reputation_query_frame!(
        persisted.checkpoint.validation_summary,
        ReputationCheckpointValidationSummaryV1
    );
    publish_immutable_bytes(
        &archive.checkpoints,
        archive.checkpoints_identity,
        &path,
        &bytes,
    )
    .expect("publish canonical checkpoint");
    assert_eq!(
        archive
            .load_checkpoint_at(&path, Some(persisted.checkpoint_digest))
            .expect("load persisted checkpoint"),
        persisted
    );
    fs::write(&path, checkpoint_bytes)
        .expect("substitute valid checkpoint payload for persisted root");
    assert!(matches!(
        archive.load_checkpoint_at(&path, None),
        Err(ReputationFinalizedArchiveError::Decode {
            source: norito::Error::SchemaMismatch,
            ..
        })
    ));
}

#[test]
fn retention_frames_bind_proposal_material_and_reject_wrong_approval_root() {
    let proposal = retention_test_proposal(1, 0x31);
    assert_reputation_query_frame!(
        proposal.material,
        ReputationFinalizedArchiveCompactionProposalMaterialV1
    );
    let authority = TestRetentionAuthority::new();
    let approval = ReputationFinalizedArchiveRetentionApprovalRecordV1::try_new(
        1,
        authority.binding().qualification(),
        proposal,
        None,
        None,
    )
    .expect("construct valid approval");
    let bytes = assert_reputation_query_frame!(
        approval,
        ReputationFinalizedArchiveRetentionApprovalRecordV1
    );
    assert_eq!(
        approval
            .to_canonical_bytes()
            .expect("production approval encoder"),
        bytes
    );
    assert_eq!(
        ReputationFinalizedArchiveRetentionApprovalRecordV1::from_canonical_bytes(&bytes)
            .expect("production approval decoder"),
        approval
    );
    let foreign = assert_reputation_query_frame!(
        approval.material,
        ReputationFinalizedArchiveRetentionApprovalMaterialV1
    );
    assert!(matches!(
        norito::decode_from_bytes::<ReputationFinalizedArchiveRetentionApprovalRecordV1>(&foreign),
        Err(norito::Error::SchemaMismatch)
    ));
    assert!(
        ReputationFinalizedArchiveRetentionApprovalRecordV1::from_canonical_bytes(&foreign)
            .is_err()
    );
}
