use super::*;

fn identity(value: u8) -> [u8; 32] {
    [value; 32]
}

fn valid_axes() -> DirectRkgOnePublicationAxesV1 {
    DirectRkgOnePublicationAxesV1 {
        publication: [identity(1), identity(1)],
        staging: [identity(2), identity(3)],
        seal: [identity(4), identity(5)],
        object: [identity(6), identity(7)],
        provider: [identity(8), identity(8)],
        snapshot: [identity(9), identity(9)],
    }
}

#[test]
fn publication_pair_accepts_only_shared_publication_provider_and_snapshot_axes() {
    assert!(validate_publication_axes_v1(valid_axes()).is_ok());

    let mutations: [fn(&mut DirectRkgOnePublicationAxesV1); 9] = [
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.publication[0] = [0; 32],
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.publication[1] = [0; 32],
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.publication[1] = identity(10),
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.provider[0] = [0; 32],
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.provider[1] = [0; 32],
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.provider[1] = identity(10),
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.snapshot[0] = [0; 32],
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.snapshot[1] = [0; 32],
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.snapshot[1] = identity(10),
    ];
    for mutate in mutations {
        let mut axes = valid_axes();
        mutate(&mut axes);
        assert!(validate_publication_axes_v1(axes).is_err());
    }
}

#[test]
fn publication_pair_rejects_zero_or_reused_per_object_axes() {
    let mutations: [fn(&mut DirectRkgOnePublicationAxesV1); 9] = [
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.staging[0] = [0; 32],
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.staging[1] = [0; 32],
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.staging[1] = axes.staging[0],
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.seal[0] = [0; 32],
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.seal[1] = [0; 32],
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.seal[1] = axes.seal[0],
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.object[0] = [0; 32],
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.object[1] = [0; 32],
        |axes: &mut DirectRkgOnePublicationAxesV1| axes.object[1] = axes.object[0],
    ];
    for mutate in mutations {
        let mut axes = valid_axes();
        mutate(&mut axes);
        assert!(validate_publication_axes_v1(axes).is_err());
    }
}

#[test]
fn typed_owner_keeps_stream_and_move_only_cas_receipts() {
    let source = include_str!("../direct_rkg_one_publication_v1.rs");
    assert!(source.contains("stream: ZkAmsMkheDirectPolynomialStreamReceiptV1"));
    assert!(source.contains("publication: ZkAmsMkheDirectObjectPublicationReceiptV1"));
    assert_eq!(source.matches("publish_direct_rkg_one_h0_h1_v1").count(), 1);
    assert!(source.find("publish_h1_v1").unwrap() > source.find("finish_h0_v1").unwrap());
}

#[test]
fn publication_area_and_tests_stay_within_review_caps() {
    let production = include_str!("../direct_rkg_one_publication_v1.rs");
    let tests = include_str!("direct_rkg_one_publication_v1_tests.rs");
    assert!(production.lines().count() <= 500 && production.len() <= 24 * 1024);
    assert!(tests.lines().count() <= 500 && tests.len() <= 24 * 1024);
}
