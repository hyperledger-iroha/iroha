#[test]
fn checkpoint_genesis_and_successor_use_cas_with_mandatory_readback() {
    let fixture = EvidenceViewerFixture::new();
    let service = fixture.open();
    let genesis = fixture
        .checkpoint_store
        .current()
        .expect("authoritative genesis");
    assert_eq!(genesis.generation, 1);
    assert_eq!(genesis.predecessor_revision, None);
    assert_eq!(genesis.predecessor_checkpoint_digest, None);
    assert_eq!(
        genesis.checkpoint_store_handle,
        TEST_CHECKPOINT_STORE_HANDLE
    );
    assert_eq!(
        genesis.checkpoint_store_revision,
        TEST_CHECKPOINT_STORE_QUALIFICATION.revision()
    );
    assert_eq!(
        genesis.checkpoint_store_policy_digest,
        TEST_CHECKPOINT_STORE_QUALIFICATION.policy_digest()
    );
    assert_eq!(genesis.revision, checkpoint_store_record_revision(&genesis));
    let (_, genesis_anchor) =
        verify_checkpoint_store_record(&fixture.config, &service.checkpoint_store, &genesis)
            .expect("genesis record signature and canonical checkpoint");
    assert_eq!(genesis_anchor.checkpoint_generation, genesis.generation);
    assert_eq!(genesis_anchor.predecessor_checkpoint_revision, None);
    assert_eq!(genesis_anchor.predecessor_checkpoint_digest, None);
    assert_eq!(
        genesis_anchor.checkpoint_store_handle,
        TEST_CHECKPOINT_STORE_HANDLE
    );
    assert_eq!(
        genesis_anchor.checkpoint_store_revision,
        TEST_CHECKPOINT_STORE_QUALIFICATION.revision()
    );
    assert_eq!(
        genesis_anchor.checkpoint_store_policy_digest,
        TEST_CHECKPOINT_STORE_QUALIFICATION.policy_digest()
    );
    assert_eq!(fixture.checkpoint_store.cas_call_count(), 1);
    assert_eq!(fixture.checkpoint_store.load_call_count(), 4);
    fixture.issue_challenge(
        &service,
        JUROR_ACCOUNT,
        EvidenceViewerRoleV1::Juror,
        [0xC0; 32],
        BASE_UNIX_MS,
    );
    let successor = fixture
        .checkpoint_store
        .current()
        .expect("authoritative successor");
    assert_eq!(successor.generation, 2);
    assert_eq!(successor.predecessor_revision, Some(genesis.revision));
    assert_eq!(
        successor.predecessor_checkpoint_digest,
        Some(genesis.checkpoint_digest)
    );
    let (_, successor_anchor) =
        verify_checkpoint_store_record(&fixture.config, &service.checkpoint_store, &successor)
            .expect("successor record signature and canonical checkpoint");
    assert_eq!(successor_anchor.checkpoint_generation, successor.generation);
    assert_eq!(
        successor_anchor.predecessor_checkpoint_revision,
        Some(genesis.revision)
    );
    assert_eq!(
        successor_anchor.predecessor_checkpoint_digest,
        Some(genesis.checkpoint_digest)
    );
    assert_eq!(fixture.checkpoint_store.cas_call_count(), 2);
    assert_eq!(fixture.checkpoint_store.load_call_count(), 8);
}
