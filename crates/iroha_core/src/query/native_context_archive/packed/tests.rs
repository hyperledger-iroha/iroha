//! Packed record layout, publication and bounded rebuild regressions.

use super::super::{
    CastingBindingsRef, LaneStateRef, NativeContextArchive, OrdinaryWritesRef,
    PreparedNativeContext, Projection, open_directory, pack_existing_records,
};
use super::*;
use crate::query::native_context_archive::PackMode;
use iroha_allocation::{AllocationBudget, AllocationRefusal, ChargedBufferError};
use iroha_crypto::HashOf;
use iroha_data_model::{
    block::{BlockHeader, consensus::ExecKv},
    parameter::system::SumeragiParameters,
    parliament_casting::ParliamentTimedOvnCastingContextBindingV1,
    sumeragi_lanes::{
        SumeragiLaneFrontier, SumeragiLaneRecord, SumeragiLaneSample, SumeragiLaneState,
    },
};
use iroha_model_base::topology::{DataSpaceId, LaneId};
use std::{
    fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

static EMPTY_WRITES: Vec<ExecKv> = Vec::new();
static EMPTY_CASTING: Vec<ParliamentTimedOvnCastingContextBindingV1> = Vec::new();
/// Anchors whose encoding fills more than two complete pieces.
const ANCHORS: usize = 130;
/// Lane records whose encoding fills more than one filesystem block.
const LANE_RECORDS: u32 = 24;

fn carrier(height: u64) -> HashOf<BlockHeader> {
    HashOf::from_untyped_unchecked(Hash::new(height.to_le_bytes()))
}
/// Lane records filling more than one filesystem block, and `samples` autoscale load samples,
/// which change every carrier.
fn lanes(samples: u64) -> SumeragiLaneState {
    SumeragiLaneState {
        lanes: (1..=LANE_RECORDS)
            .map(|lane| SumeragiLaneRecord {
                da_layout: iroha_sumeragi::availability::recommended_data_availability_layout(),
                lane: LaneId::new(lane),
                dataspace: DataSpaceId::new(0),
                incarnation: [u8::try_from(lane).unwrap(); 32],
                params: SumeragiParameters::default(),
                committee: Vec::new(),
                created_at: 10,
                active_from: 12,
                closing: None,
                anchor_freshness: 16,
                merged: SumeragiLaneFrontier::default(),
                merged_at: 12,
                rescued: 0,
            })
            .collect(),
        samples: (0..samples)
            .map(|height| SumeragiLaneSample {
                height,
                time_ms: height * 1_000,
                transactions: height % 3,
                lanes: 1,
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        ..SumeragiLaneState::default()
    }
}
fn component(out: &mut Vec<u8>, name: &[u8], value: &[u8]) {
    out.extend_from_slice(&(name.len() as u64).to_le_bytes());
    out.extend_from_slice(name);
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value);
}
/// An SCCP delta with an earlier cell and an open anchor chunk of `count` anchors.
fn delta(count: usize) -> Vec<u8> {
    let mut anchors = (count as u64).to_le_bytes().to_vec();
    for index in 0..count as u64 {
        anchors.extend_from_slice(&[0x42, 0x20]);
        anchors.extend_from_slice(Hash::new(index.to_le_bytes()).as_ref());
        anchors.push(0x20);
        anchors.extend_from_slice(Hash::new((index << 32).to_le_bytes()).as_ref());
    }
    let mut out = Vec::new();
    component(&mut out, b"sccp_liability_clock", &7_u64.to_le_bytes());
    component(&mut out, b"sccp_anchor_open", &anchors);
    out
}
fn archive(root: &Path, budget: AllocationBudget) -> NativeContextArchive {
    let (root, directory) = open_directory(root, true).unwrap();
    NativeContextArchive {
        root,
        directory,
        budget,
        maximum: NonZeroUsize::new(1 << 20).unwrap(),
        writable: true,
        namespace: "native-contexts",
        fastpq_maximum: None,
        fastpq_replay_target: None,
    }
}
fn record(
    archive: &NativeContextArchive,
    height: u64,
    lanes: &SumeragiLaneState,
    delta: &[u8],
) -> PreparedNativeContext {
    archive
        .encode_projection(&Projection {
            carrier_height: height,
            carrier_hash: carrier(height),
            lanes: LaneStateRef(lanes),
            ordinary_writes: OrdinaryWritesRef(&EMPTY_WRITES),
            casting_bindings: CastingBindingsRef(&EMPTY_CASTING),
            sccp_delta: delta,
        })
        .unwrap()
}
fn record_path(root: &Path, height: u64, staged: bool) -> PathBuf {
    root.join("native-contexts")
        .join(RecordName::new(height, carrier(height), staged).as_str())
}
fn pieces(root: &Path) -> Vec<PathBuf> {
    let mut pieces = fs::read_dir(root.join("native-contexts"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ncs"))
        .collect::<Vec<_>>();
    pieces.sort();
    pieces
}
fn no_stages(root: &Path) -> bool {
    fs::read_dir(root.join("native-contexts"))
        .unwrap()
        .all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with('.')
        })
}

#[test]
fn layout_shares_the_lane_records_and_complete_anchor_pieces() {
    let dir = tempfile::tempdir().unwrap();
    let writer = archive(dir.path(), AllocationBudget::new(1 << 20));
    let state = lanes(3);
    let original = record(&writer, 2, &state, &delta(ANCHORS));
    let canonical = original.canonical_bytes();
    let layout = Layout::of(canonical, 2, carrier(2).as_ref());
    let (start, end) = layout.lanes.expect("lane records are shared");
    let records = norito::core::SerializePayload::encoded_len_exact(&state.lanes).unwrap();
    let whole = norito::core::SerializePayload::encoded_len_exact(&LaneStateRef(&state)).unwrap();
    assert!(
        end - start >= records && end - start < whole,
        "lane records and custody, without the load samples"
    );
    assert!(end - start >= LANES_SHARED_MIN);
    // Another autoscale load sample changes no shared byte.
    let next = record(&writer, 2, &lanes(4), &delta(ANCHORS));
    let (next_start, next_end) = Layout::of(next.canonical_bytes(), 2, carrier(2).as_ref())
        .lanes
        .unwrap();
    assert_eq!(
        canonical[start..end],
        next.canonical_bytes()[next_start..next_end]
    );
    let (start, end) = layout.anchors.expect("complete anchor pieces are shared");
    assert_eq!(end - start, 2 * ANCHOR_PIECE_LEN);
    assert_eq!(canonical[start - 8..start], (ANCHORS as u64).to_le_bytes());
    assert_eq!(canonical[start..start + 2], [0x42, 0x20]);
    let mut cursor = 0;
    let mut shared = 0;
    for piece in layout.pieces(canonical.len()) {
        assert_eq!(piece.start, cursor, "pieces partition the canonical record");
        assert!(piece.end > piece.start);
        cursor = piece.end;
        shared += usize::from(piece.shared);
    }
    assert_eq!(cursor, canonical.len());
    assert_eq!(shared, 3);
    assert!(
        Layout::of(canonical, 3, carrier(2).as_ref()).is_inline(),
        "a carrier mismatch shares nothing"
    );
    let small = record(&writer, 2, &SumeragiLaneState::default(), &delta(3));
    assert!(Layout::of(small.canonical_bytes(), 2, carrier(2).as_ref()).is_inline());
}

#[test]
fn packed_record_rebuilds_the_exact_original_in_bounded_steps() {
    let dir = tempfile::tempdir().unwrap();
    let budget = AllocationBudget::new(1 << 20);
    let writer = archive(dir.path(), budget.clone());
    let original = record(&writer, 2, &lanes(3), &delta(ANCHORS));
    let canonical = original.canonical_bytes().to_vec();
    writer.publish(&original).unwrap();
    let stored = fs::read(record_path(dir.path(), 2, false)).unwrap();
    assert_eq!(stored[..4], MAGIC);
    assert!(
        stored.len() * 4 < canonical.len(),
        "{} of {}",
        stored.len(),
        canonical.len()
    );
    assert_eq!(pieces(dir.path()).len(), 3);
    assert!(no_stages(dir.path()));
    drop(original);
    assert_eq!(budget.reserved_bytes(), 0);

    let mut read = writer.read_job(2, carrier(2));
    let mut acquired = 0;
    let bytes = loop {
        if let Some(bytes) = read.poll().unwrap() {
            break bytes;
        }
        let prefix = read.acquired_prefix().map_or(0, <[u8]>::len);
        assert!(prefix - acquired <= STEP, "one chunk per poll");
        acquired = prefix;
        assert_eq!(
            budget.reserved_bytes(),
            canonical.len(),
            "one exact allocation"
        );
    };
    assert_eq!(bytes.as_slice(), canonical);
    assert!(bytes.belongs_to(&budget));
    assert!(read.poll().is_err(), "a completed read cannot restart");
    drop(bytes);
    assert_eq!(budget.reserved_bytes(), 0);
    let reader = NativeContextArchive::open_read_only(
        dir.path(),
        AllocationBudget::new(canonical.len()),
        NonZeroUsize::new(canonical.len()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        reader.read_exact(2, carrier(2)).unwrap().as_slice(),
        canonical
    );
}

#[test]
fn consecutive_records_reuse_unchanged_pieces() {
    let dir = tempfile::tempdir().unwrap();
    let writer = archive(dir.path(), AllocationBudget::new(1 << 20));
    let mut originals = Vec::new();
    for (height, anchors) in (2..).zip([ANCHORS, ANCHORS + 1, ANCHORS + 2]) {
        // Autoscale appends a load sample every carrier; the lane records stay the same.
        let original = record(&writer, height, &lanes(height), &delta(anchors));
        writer.publish(&original).unwrap();
        originals.push(original);
        assert_eq!(
            pieces(dir.path()).len(),
            3,
            "lane records and two anchor pieces"
        );
    }
    // A third complete anchor piece appears once the open chunk grows past it.
    let grown = record(&writer, 5, &lanes(5), &delta(190));
    writer.publish(&grown).unwrap();
    originals.push(grown);
    assert_eq!(pieces(dir.path()).len(), 4);
    for original in &originals {
        let bytes = writer
            .read_exact(original.height(), original.carrier_hash())
            .unwrap();
        assert_eq!(bytes.as_slice(), original.canonical_bytes());
    }
}

#[test]
fn canonical_record_and_stage_from_an_earlier_release_are_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let writer = archive(dir.path(), AllocationBudget::new(1 << 20));
    let original = record(&writer, 2, &lanes(3), &delta(ANCHORS));
    let target = record_path(dir.path(), 2, false);
    fs::write(&target, original.canonical_bytes()).unwrap();
    fs::write(record_path(dir.path(), 2, true), original.canonical_bytes()).unwrap();
    writer.publish(&original).unwrap();
    assert_eq!(fs::read(&target).unwrap(), original.canonical_bytes());
    assert!(no_stages(dir.path()));
    assert_eq!(
        pieces(dir.path()).len(),
        0,
        "an existing canonical record needs no pieces"
    );
    assert_eq!(
        writer.read_exact(2, carrier(2)).unwrap().as_slice(),
        original.canonical_bytes()
    );
    // A new publication is packed; only context records are checked as packed.
    fs::remove_file(&target).unwrap();
    writer.publish(&original).unwrap();
    assert_eq!(fs::read(&target).unwrap()[..4], MAGIC);
    let directory = File::open(dir.path().join("native-contexts")).unwrap();
    verify_original(
        &directory,
        File::open(&target).unwrap(),
        original.canonical_bytes(),
        true,
    )
    .unwrap();
    assert!(
        verify_original(
            &directory,
            File::open(&target).unwrap(),
            original.canonical_bytes(),
            false,
        )
        .is_err(),
        "only context records may be stored packed"
    );
}

#[test]
fn changed_or_missing_pieces_fail_closed_without_overwriting() {
    let dir = tempfile::tempdir().unwrap();
    let writer = archive(dir.path(), AllocationBudget::new(1 << 20));
    let original = record(&writer, 2, &lanes(3), &delta(ANCHORS));
    writer.publish(&original).unwrap();
    writer.publish(&original).unwrap();
    let piece = pieces(dir.path()).remove(0);
    let kept = fs::read(&piece).unwrap();
    let mut changed = kept.clone();
    changed[100] ^= 1;
    fs::write(&piece, &changed).unwrap();
    assert!(matches!(
        writer.read_exact(2, carrier(2)),
        Err(NativeContextArchiveError::Source(_))
    ));
    assert!(writer.publish(&original).is_err());
    assert_eq!(fs::read(&piece).unwrap(), changed, "never overwritten");

    fs::remove_file(&piece).unwrap();
    assert!(matches!(
        writer.read_exact(2, carrier(2)),
        Err(NativeContextArchiveError::Io(error)) if error.kind() == io::ErrorKind::NotFound
    ));
    // Republication restores a missing piece from the original bytes.
    writer.publish(&original).unwrap();
    assert_eq!(fs::read(&piece).unwrap(), kept);

    let target = record_path(dir.path(), 2, false);
    let mut record_bytes = fs::read(&target).unwrap();
    let last = record_bytes.len() - 1;
    record_bytes[last] ^= 1;
    fs::write(&target, &record_bytes).unwrap();
    assert!(matches!(
        writer.read_exact(2, carrier(2)),
        Err(NativeContextArchiveError::Source(_))
    ));
    assert!(writer.publish(&original).is_err());
    assert_eq!(fs::read(&target).unwrap(), record_bytes);
}

#[test]
fn refused_rebuild_keeps_the_selected_record_and_retries_exactly() {
    let dir = tempfile::tempdir().unwrap();
    let writer = archive(dir.path(), AllocationBudget::new(1 << 20));
    let original = record(&writer, 2, &lanes(3), &delta(ANCHORS));
    writer.publish(&original).unwrap();
    let canonical = original.canonical_bytes().to_vec();
    drop(original);
    let budget = AllocationBudget::new(canonical.len());
    let (root, directory) = open_directory(dir.path(), false).unwrap();
    let reader = NativeContextArchive {
        root,
        directory,
        budget: budget.clone(),
        maximum: NonZeroUsize::new(canonical.len()).unwrap(),
        writable: false,
        namespace: "native-contexts",
        fastpq_maximum: None,
        fastpq_replay_target: None,
    };
    let blocker = ChargedBuffer::<u8>::new(1, &budget).unwrap();
    let mut read = reader.read_job(2, carrier(2));
    assert!(matches!(
        read.poll(),
        Err(NativeContextArchiveError::Allocation(
            ChargedBufferError::Admission(AllocationRefusal::Capacity { .. })
        ))
    ));
    assert!(read.has_pinned_source());
    // The pinned descriptor survives replacement of the record's path.
    let target = record_path(dir.path(), 2, false);
    fs::rename(&target, dir.path().join("held-original")).unwrap();
    fs::write(&target, b"substituted").unwrap();
    drop(blocker);
    let bytes = loop {
        if let Some(bytes) = read.poll().unwrap() {
            break bytes;
        }
    };
    assert_eq!(bytes.as_slice(), canonical);
}

#[test]
fn existing_canonical_records_are_packed_offline_under_the_store_lock() {
    let dir = tempfile::tempdir().unwrap();
    let writer = archive(dir.path(), AllocationBudget::new(1 << 20));
    let state = lanes(3);
    let mut originals = (2..)
        .zip([ANCHORS, ANCHORS + 1, 190])
        .map(|(height, anchors)| record(&writer, height, &state, &delta(anchors)))
        .collect::<Vec<_>>();
    originals.push(record(&writer, 5, &SumeragiLaneState::default(), &delta(3)));
    // An earlier release wrote every record in canonical form.
    for original in &originals {
        fs::write(
            record_path(dir.path(), original.height(), false),
            original.canonical_bytes(),
        )
        .unwrap();
    }
    let unrelated = dir.path().join("native-contexts").join("unrelated");
    fs::write(&unrelated, b"kept").unwrap();
    assert!(
        pack_existing_records(dir.path(), PackMode::Pack).is_err(),
        "a store without its Kura lock file is not packed"
    );
    fs::write(dir.path().join(".kura.lock"), b"").unwrap();
    let measured = pack_existing_records(dir.path(), PackMode::Measure).unwrap();
    assert_eq!((measured.records, measured.packed), (4, 3));
    assert!(measured.stored_bytes * 4 < measured.canonical_bytes);
    assert_eq!(pieces(dir.path()).len(), 0, "a dry run writes nothing");
    let running = File::open(dir.path().join(".kura.lock")).unwrap();
    running.try_lock().unwrap();
    assert!(
        pack_existing_records(dir.path(), PackMode::Pack).is_err(),
        "a running node's store is never rewritten"
    );
    drop(running);
    assert_eq!(
        pack_existing_records(dir.path(), PackMode::Pack).unwrap(),
        measured
    );
    assert_eq!(pieces(dir.path()).len(), 4);
    assert!(no_stages(dir.path()));
    let again = pack_existing_records(dir.path(), PackMode::Pack).unwrap();
    assert_eq!((again.packed, again.canonical_bytes), (3, 0));
    for original in &originals {
        assert_eq!(
            writer
                .read_exact(original.height(), original.carrier_hash())
                .unwrap()
                .as_slice(),
            original.canonical_bytes()
        );
    }
    assert_eq!(
        fs::read(record_path(dir.path(), 5, false)).unwrap(),
        originals[3].canonical_bytes(),
        "records with nothing to share stay canonical"
    );
    assert_eq!(fs::read(&unrelated).unwrap(), b"kept");
    // A release that cannot read packed records gets every original back.
    let restored = pack_existing_records(dir.path(), PackMode::Unpack).unwrap();
    assert_eq!(
        (restored.records, restored.packed, restored.canonical_bytes),
        (4, 0, measured.canonical_bytes)
    );
    for original in &originals {
        assert_eq!(
            fs::read(record_path(dir.path(), original.height(), false)).unwrap(),
            original.canonical_bytes()
        );
    }
    assert_eq!(pieces(dir.path()).len(), 0);
    assert!(no_stages(dir.path()));
    assert_eq!(fs::read(&unrelated).unwrap(), b"kept");
}

#[test]
fn certified_chain_packs_records_once_the_open_anchor_chunk_fills_a_piece() {
    let (mut chain, _) = crate::smartcontracts::isi::sccp::forgery::tests::active_member_chain();
    // A1 appends one 67-byte anchor per height; a complete 4 KiB piece needs about 62.
    for _ in 0..70 {
        chain.commit(Vec::new());
    }
    let root = chain.kura().store_root();
    let packed = fs::read_dir(root.join("native-contexts"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "nrt"))
        .filter(|path| fs::read(path).unwrap().starts_with(&MAGIC))
        .count();
    assert!(packed > 0, "later records share complete anchor pieces");
    assert_ne!(pieces(&root).len(), 0);
    let reader = NativeContextArchive::open_read_only(
        &root,
        AllocationBudget::new(1 << 24),
        NonZeroUsize::new(1 << 22).unwrap(),
    )
    .unwrap();
    for height in 1..=chain.height() {
        let bytes = reader
            .read_exact(height, chain.committed(height).block_hash())
            .unwrap();
        let projection: crate::state::NativeExecutionProjectionV1 =
            norito::decode_canonical(bytes.as_slice()).unwrap();
        assert_eq!(projection.carrier_height, height);
    }
}

#[test]
fn offline_rewrite_clears_node_stages_and_its_own_interrupted_temporaries() {
    let dir = tempfile::tempdir().unwrap();
    let writer = archive(dir.path(), AllocationBudget::new(1 << 20));
    fs::write(dir.path().join(".kura.lock"), b"").unwrap();
    let originals = (2..5)
        .map(|height| record(&writer, height, &lanes(height), &delta(ANCHORS)))
        .collect::<Vec<_>>();
    for original in &originals {
        fs::write(
            record_path(dir.path(), original.height(), false),
            original.canonical_bytes(),
        )
        .unwrap();
    }
    // A node interrupted after linking its stage leaves a second link to the record.
    fs::hard_link(
        record_path(dir.path(), 2, false),
        record_path(dir.path(), 2, true),
    )
    .unwrap();
    // An interrupted rewrite leaves a partial temporary, which node republication ignores.
    let temporary = dir
        .path()
        .join("native-contexts")
        .join(RecordName::rewrite(3, carrier(3).as_ref()).as_str());
    fs::write(&temporary, b"partial").unwrap();
    writer.publish(&originals[1]).unwrap();
    assert_eq!(fs::read(&temporary).unwrap(), b"partial");
    assert_eq!(
        pack_existing_records(dir.path(), PackMode::Pack)
            .unwrap()
            .packed,
        3
    );
    assert!(
        no_stages(dir.path()),
        "matching stages and temporaries are gone"
    );
    // A packed stage beside a packed record never reaches a release without packed records.
    fs::hard_link(
        record_path(dir.path(), 4, false),
        record_path(dir.path(), 4, true),
    )
    .unwrap();
    assert_eq!(
        pack_existing_records(dir.path(), PackMode::Unpack)
            .unwrap()
            .packed,
        0
    );
    assert!(no_stages(dir.path()));
    assert_eq!(pieces(dir.path()).len(), 0);
    for original in &originals {
        assert_eq!(
            fs::read(record_path(dir.path(), original.height(), false)).unwrap(),
            original.canonical_bytes()
        );
    }
}
