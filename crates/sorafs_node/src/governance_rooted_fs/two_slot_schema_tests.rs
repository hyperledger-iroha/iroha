// Explicit framed roots and actual persistence codec regressions.

#[test]
fn declared_schema_roots_are_distinct_and_explicit() {
    let rows = [
        (
            "sorafs_node::governance_rooted_fs::TwoSlotBindingMaterialV1",
            <TwoSlotBindingMaterialV1 as norito::NoritoSchema>::nominal_name(),
            <TwoSlotBindingMaterialV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance_rooted_fs::TwoSlotCommitTrailerRegionV1",
            <TwoSlotCommitTrailerRegionV1 as norito::NoritoSchema>::nominal_name(),
            <TwoSlotCommitTrailerRegionV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance_rooted_fs::TwoSlotHeaderRegionV1",
            <TwoSlotHeaderRegionV1 as norito::NoritoSchema>::nominal_name(),
            <TwoSlotHeaderRegionV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance_rooted_fs::TwoSlotRecordHeaderRegionV1",
            <TwoSlotRecordHeaderRegionV1 as norito::NoritoSchema>::nominal_name(),
            <TwoSlotRecordHeaderRegionV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance_rooted_fs::TwoSlotRecordHeaderV1",
            <TwoSlotRecordHeaderV1 as norito::NoritoSchema>::nominal_name(),
            <TwoSlotRecordHeaderV1 as norito::NoritoSchema>::frame_name(),
        ),
    ];
    let mut identities = std::collections::BTreeSet::new();
    for (expected, nominal, frame) in rows {
        assert_eq!(nominal, expected);
        assert_eq!(frame, expected);
        assert!(
            identities.insert(frame),
            "different roots must remain distinct"
        );
    }
}

fn assert_declared_persistence_frame<T>(value: &T, expected_root: &str) -> Vec<u8>
where
    T: norito::NoritoSerialize + std::fmt::Debug + PartialEq,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let frame = norito::encode_canonical(value).expect("canonical typed frame");
    let view = norito::core::from_bytes_view(&frame).expect("validated frame");
    assert_eq!(
        view.schema(),
        norito::core::schema_hash_for_name(expected_root)
    );
    assert_eq!(
        norito::canonical_frame_len(value).expect("exact frame length"),
        frame.len()
    );
    assert_eq!(
        &norito::decode_canonical::<T>(&frame).expect("typed recovery"),
        value
    );
    let mut substituted = frame.clone();
    // The canonical header puts its 16-byte schema after magic and two version bytes.
    substituted[6..22].copy_from_slice(&norito::core::schema_hash_for_name(
        "sorafs_node::different.persistence.root",
    ));
    assert!(matches!(
        norito::decode_canonical::<T>(&substituted),
        Err(norito::Error::SchemaMismatch)
    ));
    let mut suffixed = frame.clone();
    suffixed.push(0);
    assert!(norito::decode_canonical::<T>(&suffixed).is_err());
    assert!(norito::decode_canonical::<T>(&frame[..frame.len() - 1]).is_err());
    frame
}

#[test]
fn fixed_two_slot_regions_keep_distinct_headers_and_exact_layout_lengths() {
    let binding = zero_two_slot_binding_material();
    assert_declared_persistence_frame(
        &binding,
        "sorafs_node::governance_rooted_fs::TwoSlotBindingMaterialV1",
    );
    let header = TwoSlotHeaderRegionV1 {
        header: TwoSlotHeaderV1 {
            binding,
            binding_digest: [0; 32],
            slot_id: 0,
        },
        reserved: [0; TWO_SLOT_HEADER_RESERVED_BYTES_V1],
    };
    let record_header = TwoSlotRecordHeaderV1 {
        format_version: TWO_SLOT_FORMAT_VERSION_V1,
        binding_digest: [0; 32],
        slot_id: 0,
        generation: 0,
        predecessor_digest: [0; 32],
        payload_len: 0,
        payload_digest: [0; 32],
    };
    assert_declared_persistence_frame(
        &record_header,
        "sorafs_node::governance_rooted_fs::TwoSlotRecordHeaderV1",
    );
    let record = TwoSlotRecordHeaderRegionV1 {
        header: record_header,
        reserved: [0; TWO_SLOT_RECORD_HEADER_RESERVED_BYTES_V1],
    };
    let trailer = TwoSlotCommitTrailerRegionV1 {
        trailer: TwoSlotCommitTrailerV1 {
            format_version: TWO_SLOT_FORMAT_VERSION_V1,
            binding_digest: [0; 32],
            slot_id: 0,
            generation: 0,
            record_digest: [0; 32],
            commit_marker: TWO_SLOT_COMMIT_MARKER_V1,
        },
        reserved: [0; TWO_SLOT_COMMIT_TRAILER_RESERVED_BYTES_V1],
    };
    let header_bytes = assert_declared_persistence_frame(
        &header,
        "sorafs_node::governance_rooted_fs::TwoSlotHeaderRegionV1",
    );
    let record_bytes = assert_declared_persistence_frame(
        &record,
        "sorafs_node::governance_rooted_fs::TwoSlotRecordHeaderRegionV1",
    );
    let trailer_bytes = assert_declared_persistence_frame(
        &trailer,
        "sorafs_node::governance_rooted_fs::TwoSlotCommitTrailerRegionV1",
    );
    let layout = two_slot_layout(1024).expect("fixed store layout");
    assert_eq!(layout.header_region_bytes, header_bytes.len());
    assert_eq!(layout.record_header_region_bytes, record_bytes.len());
    assert_eq!(layout.commit_trailer_region_bytes, trailer_bytes.len());
    assert_eq!(
        decode_two_slot_value::<TwoSlotHeaderRegionV1>(&header_bytes, "header").unwrap(),
        header
    );
    assert_eq!(
        decode_two_slot_value::<TwoSlotRecordHeaderRegionV1>(&record_bytes, "record").unwrap(),
        record
    );
    assert_eq!(
        decode_two_slot_value::<TwoSlotCommitTrailerRegionV1>(&trailer_bytes, "trailer").unwrap(),
        trailer
    );
    assert!(decode_two_slot_value::<TwoSlotHeaderRegionV1>(&record_bytes, "wrong region").is_err());
}
