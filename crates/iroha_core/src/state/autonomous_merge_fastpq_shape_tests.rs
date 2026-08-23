#[test]
fn merge_execution_rejects_every_malformed_fastpq_bundle_shape() {
    let (state, entry, _) = autonomous_merge_transfer_commit_authorization_fixture();
    let batch = entry
        .execution_batch
        .as_ref()
        .expect("fixture carries one FASTPQ execution batch");
    let expected_reason = "FASTPQ transcript bundle is empty or not bound to its lane entrypoint";
    let canonical_bundle = batch.lanes[0].fastpq_transcripts[0].clone();
    let mut empty = batch.clone();
    empty.lanes[0].fastpq_transcripts.fastpq_transcripts[0]
        .transcripts
        .clear();
    assert_fastpq_batch_rejected(&state, &entry.active_lanes, empty, expected_reason);
    let mut nonmember = batch.clone();
    let nonmember_hash = Hash::new(b"nonmember FASTPQ entrypoint");
    let nonmember_bundle = &mut nonmember.lanes[0].fastpq_transcripts.fastpq_transcripts[0];
    nonmember_bundle.entry_hash = nonmember_hash;
    for transcript in &mut nonmember_bundle.transcripts {
        transcript.batch_hash = nonmember_hash;
    }
    assert_fastpq_batch_rejected(&state, &entry.active_lanes, nonmember, expected_reason);
    let mut mismatched = batch.clone();
    mismatched.lanes[0].fastpq_transcripts.fastpq_transcripts[0].transcripts[0].batch_hash =
        Hash::new(b"mismatched FASTPQ call hash");
    assert_fastpq_batch_rejected(&state, &entry.active_lanes, mismatched, expected_reason);
    let mut duplicate = batch.clone();
    duplicate.lanes[0]
        .fastpq_transcripts
        .fastpq_transcripts
        .push(canonical_bundle.clone());
    assert_fastpq_batch_rejected(
        &state,
        &entry.active_lanes,
        duplicate,
        "FASTPQ transcript bundles are not unique and canonically ordered",
    );
    let mut unordered = batch.clone();
    let mut later = canonical_bundle.clone();
    later.entry_hash = Hash::prehashed([0xFF; Hash::LENGTH]);
    for transcript in &mut later.transcripts {
        transcript.batch_hash = later.entry_hash;
    }
    let mut earlier = canonical_bundle;
    earlier.entry_hash = Hash::prehashed([0x00; Hash::LENGTH]);
    for transcript in &mut earlier.transcripts {
        transcript.batch_hash = earlier.entry_hash;
    }
    unordered.lanes[0].fastpq_transcripts = vec![later, earlier].into();
    assert_fastpq_batch_rejected(
        &state,
        &entry.active_lanes,
        unordered,
        "FASTPQ transcript bundles are not unique and canonically ordered",
    );
}
