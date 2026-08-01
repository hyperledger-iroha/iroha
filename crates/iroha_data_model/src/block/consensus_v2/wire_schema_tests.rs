// Wire-schema regression tests included in the parent consensus-v2 test module.

#[test]
fn global_phase_wire_tags_are_explicit_and_schema_aligned() {
    let prepare = GlobalPhase::Prepare.encode();
    let commit = GlobalPhase::Commit.encode();
    assert_eq!(prepare, u32::from(GlobalPhase::Prepare as u8).to_le_bytes());
    assert_eq!(commit, u32::from(GlobalPhase::Commit as u8).to_le_bytes());
    assert_eq!(prepare, 1_u32.to_le_bytes());
    assert_eq!(commit, 2_u32.to_le_bytes());

    let mut prepare_cursor = prepare.as_slice();
    let mut commit_cursor = commit.as_slice();
    assert_eq!(
        GlobalPhase::decode_all(&mut prepare_cursor).expect("decode Prepare"),
        GlobalPhase::Prepare
    );
    assert_eq!(
        GlobalPhase::decode_all(&mut commit_cursor).expect("decode Commit"),
        GlobalPhase::Commit
    );
    let legacy_implicit_zero_bytes = 0_u32.to_le_bytes();
    let mut legacy_implicit_zero = legacy_implicit_zero_bytes.as_slice();
    assert!(GlobalPhase::decode_all(&mut legacy_implicit_zero).is_err());
}
