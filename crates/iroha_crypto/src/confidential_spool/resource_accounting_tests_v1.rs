//! Named confidential-spool payload accounting and exact AAD ownership.
use super::*;
use core::mem::size_of_val;

#[test]
fn named_spool_payloads_cover_real_handles_and_record_workspace() {
    let layout = ConfidentialSpoolLayoutV1::new_v1(2, 16_384, [17; 32]).unwrap();
    let retained = layout.named_retained_bytes_v1();
    assert!(retained >= size_of::<ConfidentialSpoolWriterV1>() + KEY_BYTES_V1);
    assert!(retained >= size_of::<ConfidentialSpoolSnapshotV1>() + KEY_BYTES_V1);
    let smaller = ConfidentialSpoolLayoutV1::new_v1(1, 1, [18; 32]).unwrap();
    assert_eq!(retained, smaller.named_retained_bytes_v1());
    assert_eq!(
        layout.named_operation_workspace_bytes_v1() - smaller.named_operation_workspace_bytes_v1(),
        16_383
    );
    let chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(layout.plaintext_len).unwrap();
    let aad = allocate_aad_v1(&layout).unwrap();
    assert_eq!(aad.capacity(), layout.aad_len);
    assert!(
        layout.named_operation_workspace_bytes_v1()
            >= chunk.len_v1() as usize
                + size_of_val(&chunk)
                + aad.capacity()
                + size_of_val(&aad)
                + size_of::<XChaCha20Poly1305>()
                + 2 * size_of::<blake3::Hasher>()
    );
}

#[test]
fn named_spool_aad_rejects_larger_or_smaller_actual_capacity_before_filling() {
    let layout = ConfidentialSpoolLayoutV1::new_v1(2, 16_384, [19; 32]).unwrap();
    for requested in [layout.aad_len - 1, layout.aad_len + 1] {
        let mut aad = Vec::new();
        aad.try_reserve_exact(requested).unwrap();
        assert_ne!(aad.capacity(), layout.aad_len);
        assert!(aad.is_empty());
        assert_eq!(
            require_exact_aad_capacity_v1(&layout, &aad),
            Err(ConfidentialSpoolErrorV1::Allocation("record AAD capacity"))
        );
        assert!(aad.is_empty());
    }
    let mut admitted = allocate_aad_v1(&layout).unwrap();
    let pointer = admitted.as_ptr();
    let coordinate = derived_coordinate_v1(&layout, 0);
    fill_aad_v1(&mut admitted, &layout, &[23; 16], 0, &coordinate);
    assert_eq!(admitted.len(), layout.aad_len);
    assert_eq!(admitted.capacity(), layout.aad_len);
    assert_eq!(admitted.as_ptr(), pointer);
}

#[cfg(unix)]
#[test]
fn named_spool_workspace_keeps_existing_real_file_ciphertext_roundtrip() {
    let directory = tempfile::tempdir().unwrap();
    let layout = ConfidentialSpoolLayoutV1::new_v1(2, 16_384, [31; 32]).unwrap();
    let retained = layout.named_retained_bytes_v1();
    let scratch = layout.named_operation_workspace_bytes_v1();
    let mut writer = ConfidentialSpoolWriterV1::create_in_v1(directory.path(), layout).unwrap();
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    assert_eq!(writer.resources.as_ref().unwrap().key.len(), KEY_BYTES_V1);
    for slot in 0..2 {
        let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
        chunk.as_mut_slice_v1().fill(41 + slot as u8);
        writer.write_slot_v1(slot, chunk).unwrap();
    }
    let mut snapshot = writer.seal_v1().unwrap();
    assert!(retained >= size_of_val(&snapshot) + KEY_BYTES_V1);
    for slot in 0..2 {
        let chunk = snapshot.read_slot_v1(slot, [31; 32]).unwrap();
        assert!(chunk.as_slice_v1().iter().all(|b| *b == 41 + slot as u8));
        assert!(scratch >= size_of_val(&chunk) + chunk.len_v1() as usize);
    }
    drop(snapshot);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}
