//! Signed source indices and actual authenticated tiny-file read controls.
use super::*;

#[test]
fn signed_source_coordinates_match_frozen_same_opening_mapping() {
    use crate::vega::zk_ams::mkhe::rns_native_source_packing_same_opening::signed_source_index_v1;
    for unit in 0..1_032_u16 {
        for first in [7_224, 8_256] {
            let coordinate = signed_source_coordinate_v1(first + unit).unwrap();
            assert_eq!(coordinate.slot, unit);
            assert_eq!(usize::from(coordinate.record), usize::from(unit) / 24);
            for k in [0, 63, 64, 1_023, 1_024, 8_191, 16_383] {
                let source = signed_source_index_v1(usize::from(unit), k).unwrap();
                assert_eq!(
                    source.source_slot as usize,
                    usize::from(coordinate.record) * 896
                        + 512
                        + coordinate.role.index_v1() * 128
                        + usize::from(coordinate.first_source_block)
                        + k / 1_024
                );
                assert_eq!(usize::from(source.byte_offset), 8 * (k % 1_024));
            }
        }
    }
    for ordinal in [0, 687, 688, 7_223, 9_288, u16::MAX] {
        assert!(signed_source_coordinate_v1(ordinal).is_err());
    }
}

#[cfg(unix)]
#[test]
fn original_compact_slots_are_authenticated_for_both_purposes() {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let directory = Directory(std::env::temp_dir().join(format!(
        "iroha-signed-source-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    fs::create_dir(&directory.0).unwrap();
    fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o700)).unwrap();
    let context = [0x5b; 32];
    // Tiny file tests the actual authenticated reader only. It does not mint
    // the complete 1,032-slot production replay evidence or a proof authority.
    let layout = ConfidentialSpoolLayoutV1::new_v1(4, 16_384, context).unwrap();
    let mut writer = ConfidentialSpoolWriterV1::create_in_v1(&directory.0, layout).unwrap();
    for slot in 0..4_u64 {
        let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
        chunk.as_mut_slice_v1()[0] = [0, 1, 255, 1][slot as usize];
        chunk.as_mut_slice_v1()[16_383] = [255, 0, 1, 255][slot as usize];
        writer.write_slot_v1(slot, chunk).unwrap();
    }
    let mut snapshot = writer.seal_v1().unwrap();
    for unit in 0..4_u16 {
        let signed = read_signed_compact_slot_v1(&mut snapshot, 7_224 + unit, context).unwrap();
        let negative = read_signed_compact_slot_v1(&mut snapshot, 8_256 + unit, context).unwrap();
        assert_eq!(signed.as_slice_v1(), negative.as_slice_v1());
        assert_eq!(signed.as_slice_v1()[0], [0, 1, 255, 1][unit as usize]);
        assert_eq!(
            signed.as_slice_v1()[16_383],
            [255, 0, 1, 255][unit as usize]
        );
    }
    assert!(read_signed_compact_slot_v1(&mut snapshot, 7_224, [0x5c; 32]).is_err());
    assert!(read_signed_compact_slot_v1(&mut snapshot, 7_228, context).is_err());
    // The crypto leaf deliberately exposes no descriptor or path for its
    // already-unlinked file. Raw ciphertext/tag tampering remains covered by
    // that leaf's own tests, without adding a descriptor escape here.
    drop(snapshot);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[test]
fn replay_lineage_and_original_stage_are_checked_before_signed_source_io() {
    let source = include_str!("prepared_small_signed_source_v1.rs");
    let read = source
        .split("fn read_small_signed_plane_v1(")
        .nth(1)
        .unwrap()
        .split("fn commit_prepared_small_signed_v1(")
        .next()
        .unwrap();
    assert!(
        read.find("self.validate_radix_materialization_source_v1(")
            .unwrap()
            < read
                .find("self.openings.require_small_signed_position_v1(ordinal)")
                .unwrap()
    );
    assert!(
        read.find("self.openings.require_small_signed_position_v1(ordinal)")
            .unwrap()
            < read.find("read_signed_compact_slot_v1(").unwrap()
    );
    let admit = source
        .split("fn commit_prepared_small_signed_v1(")
        .nth(1)
        .unwrap();
    assert!(
        admit.find("validate_replay_evidence_v1(self)").unwrap()
            < admit.find(".validate_origin_v1(").unwrap()
    );
    assert!(
        admit.find(".validate_origin_v1(").unwrap()
            < admit
                .find("self.openings.commit_prepared_small_signed_v1(statement)")
                .unwrap()
    );
}
