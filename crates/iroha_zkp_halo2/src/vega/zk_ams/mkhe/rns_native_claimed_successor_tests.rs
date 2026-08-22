use super::*;

struct TestParentV1(u8);

#[test]
fn opaque_claim_mints_exact_borrow_without_splitting_parent() {
    let successor = b"claimed-comparator-successor";
    let claim = RnsNativeCrossFieldRlweClaimedSuccessorSliceV1::test_fixture_v1(successor)
        .expect("bounded nonempty claimed successor");
    let carried = RnsNativeClaimedSuccessorV1::from_direct_claim_v1(TestParentV1(7), claim);
    assert_eq!(carried.parent().0, 7);
    assert_eq!(carried.successor(), successor);
}

#[test]
fn carrier_is_move_only_and_has_no_public_or_raw_constructor() {
    let source = include_str!("rns_native_claimed_successor.rs");
    let declaration = source
        .find("pub(super) struct RnsNativeClaimedSuccessorV1")
        .expect("claimed successor declaration");
    let prefix = &source[declaration.saturating_sub(320)..declaration];
    assert!(!prefix.contains("derive(Clone"));
    assert!(!prefix.contains("derive(Copy"));
    assert!(!source.contains("impl Clone for RnsNativeClaimedSuccessorV1"));
    assert!(!source.contains("impl Copy for RnsNativeClaimedSuccessorV1"));
    assert!(!source.contains("pub fn "));
    assert!(!source.contains("fn new("));
    assert!(!source.contains("fn from_raw"));
    let constructor = source
        .split_once("pub(super) fn from_direct_claim_v1(")
        .expect("opaque-claim constructor")
        .1
        .split_once("/// Borrow the retained parent")
        .expect("constructor boundary")
        .0;
    assert!(constructor.contains("RnsNativeCrossFieldRlweClaimedSuccessorSliceV1<'proof>"));
    assert!(!constructor.contains("successor: &'proof [u8]"));

    let parent = include_str!("../mkhe.rs");
    assert_eq!(
        parent.matches("mod rns_native_claimed_successor;").count(),
        1
    );
    assert!(!parent.contains("pub mod rns_native_claimed_successor"));
}
