#[test]
fn retention_checkpoint_inventory_accepts_only_exact_crash_boundary_sets() {
    let directory = physical_tempdir().expect("archive tempdir");
    let archive = ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds())
        .expect("open archive");
    let first = projection(7);
    let second = advance_projection(&first, 8);
    let third = advance_projection(&second, 9);
    let fourth = advance_projection(&third, 10);
    archive.insert(first).expect("insert first");
    archive.insert(second.clone()).expect("insert second");
    archive.insert(third.clone()).expect("insert third");

    let (_prior_fence, _prior_prepared, prior_proposal) =
        prepared_compaction_for_test(&archive, second.key.clone());
    let authority = TestRetentionAuthority::new();
    let binding = authority.binding();
    let prior_approval = ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::try_new(
        1,
        binding.qualification(),
        prior_proposal,
        None,
        None,
    )
    .expect("construct predecessor approval");
    let prior_outcome = compact_for_test(&archive, second.key);
    archive.insert(fourth.clone()).expect("insert fourth");

    let (_approved_fence, approved_prepared, approved_proposal) =
        prepared_compaction_for_test(&archive, third.key.clone());
    let (_alternate_fence, alternate_prepared, _alternate_proposal) =
        prepared_compaction_for_test(&archive, fourth.key);
    let approval = ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::try_new(
        2,
        binding.qualification(),
        approved_proposal,
        Some(prior_approval.revision()),
        Some(prior_outcome.checkpoint_digest()),
    )
    .expect("construct successor approval");
    let predecessor = archive
        .read_index()
        .expect("read archive")
        .virtual_bases
        .get(&third.key.network_id)
        .expect("predecessor checkpoint")
        .clone();
    let approved = candidate_for_prepared_compaction(&archive, &approved_prepared);
    let alternate = candidate_for_prepared_compaction(&archive, &alternate_prepared);

    validate_retention_checkpoint_candidate_inventory(
        std::slice::from_ref(&approved),
        &approval,
        &third.key.network_id,
    )
    .expect("completed cleanup retains only the approved checkpoint");
    validate_retention_checkpoint_candidate_inventory(
        &[predecessor.clone(), approved.clone()],
        &approval,
        &third.key.network_id,
    )
    .expect("publication crash window retains predecessor and approved checkpoint");
    validate_retention_checkpoint_candidate_inventory(
        std::slice::from_ref(&predecessor),
        &approval,
        &third.key.network_id,
    )
    .expect("pre-publication crash retains only the predecessor checkpoint");

    assert!(matches!(
        validate_retention_checkpoint_candidate_inventory(
            &[approved.clone(), approved.clone()],
            &approval,
            &third.key.network_id,
        ),
        Err(ProviderIngestFinalizedArchiveErrorV1::UnapprovedRetentionCheckpoint)
    ));
    assert!(matches!(
        validate_retention_checkpoint_candidate_inventory(
            &[predecessor, approved, alternate],
            &approval,
            &third.key.network_id,
        ),
        Err(ProviderIngestFinalizedArchiveErrorV1::UnapprovedRetentionCheckpoint)
    ));
    assert!(matches!(
        validate_retention_checkpoint_candidate_inventory(&[], &approval, &third.key.network_id,),
        Err(ProviderIngestFinalizedArchiveErrorV1::RetentionAuthorityRollback)
    ));
}
