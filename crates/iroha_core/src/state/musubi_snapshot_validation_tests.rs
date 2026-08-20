state_test! { sync musubi_snapshot_rejects_dangling_reverse_index_tombstones
    let_row! { location = MusubiArchiveLocationKeyV1::new( ArchiveId::new([1; 32]), MusubiArchiveLocationIdV1::new([2; 32]), ) };
    let locations = Storage::<MusubiArchiveLocationKeyV1, MusubiArchiveLocationV1>::default();
    let_row! { by_pin = Storage::from_iter([( ManifestDigest::new([3; 32]), MusubiPinLocationReferenceV1 { pin_manifest: ManifestDigest::new([3; 32]), location, active: false, }, )]) };
    let_row! { by_order = Storage::<ReplicationOrderId, MusubiReplicationOrderLocationReferenceV1>::default() };
    let by_provider = Storage::<MusubiProviderLocationKeyV1, ()>::default();
    let archives = Storage::default();
    let pin_manifests = Storage::default();
    let replication_orders = Storage::default();
    let_row! { error = super::deserialize::validate_musubi_location_reverse_indices( &archives, &locations, &pin_manifests, &replication_orders, &by_pin, &by_order, &by_provider, ) .expect_err("dangling immutable reuse tombstones must fail snapshot validation") };
    assert!(error.to_string().contains("missing location"));
}
