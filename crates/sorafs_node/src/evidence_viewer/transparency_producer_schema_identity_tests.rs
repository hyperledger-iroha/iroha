// Canonical frame identities are explicit; these are local fixture checks, not runtime qualification.

#[test]
fn declared_frame_names_are_explicit_and_nominal_in_generic_parents() {
    crate::schema_identity_test_support::assert_identity::<EvidenceViewerTransparencyHeadBodyV1>(
        "sorafs_node::evidence_viewer::transparency_producer::EvidenceViewerTransparencyHeadBodyV1",
    );
}

#[test]
fn transparency_published_body_advertises_explicit_schema_identity() {
    let publisher = Arc::new(FakePublisher::new(PUBLISHER_HANDLE));
    let producer = EvidenceViewerTransparencyProducerV1::try_new(config(), publisher)
        .expect("qualified fixture producer");
    let source = projection(signed_anchor(1, None, [0x81; 32], None), None, Vec::new());
    let head = match producer
        .publish_projection(&source)
        .expect("actual publication")
    {
        EvidenceViewerTransparencyProducerOutcomeV1::Published(head) => head,
        EvidenceViewerTransparencyProducerOutcomeV1::AlreadyCurrent(_) => panic!("fresh fixture"),
    };
    head.verify(&config()).expect("real signed head");
    crate::schema_identity_test_support::assert_canonical_frame(
        &head.body,
        "sorafs_node::evidence_viewer::transparency_producer::EvidenceViewerTransparencyHeadBodyV1",
    );
}
