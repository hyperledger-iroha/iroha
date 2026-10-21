//! Fresh ordered snapshot reads share only borrowed ancestry, never source or codec verdicts.

use super::*;
use crate::managed::{Error, native_operation::attempts::metadata};
use std::io;

fn observed(directory: &Arc<PrivateDirectory>, name: &str, maximum: usize) -> RecordSnapshot {
    let bytes = read_optional(directory, name, maximum).unwrap();
    RecordSnapshot::from_read(Arc::clone(directory), name, maximum, bytes.as_deref())
}

// These native files exercise snapshot byte custody, not protocol/signature admission.
struct Store {
    temporary: tempfile::TempDir,
    root: Arc<PrivateDirectory>,
    container: Arc<PrivateDirectory>,
    first: Arc<PrivateDirectory>,
    reference: Arc<PrivateDirectory>,
    snapshot: Arc<Snapshot>,
}
impl Store {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let root =
            Arc::new(PrivateDirectory::open_or_create(temporary.path().join("purpose")).unwrap());
        root.write_atomic("original.nrt", b"original", PublishMode::CreateNew)
            .unwrap();
        root.write_atomic("anchor.nrt", b"anchor", PublishMode::CreateNew)
            .unwrap();
        let epochs = root.create_child("epochs").unwrap();
        drop(epochs);
        let container = Arc::new(root.create_child("bodies").unwrap());
        let reference =
            Arc::new(PrivateDirectory::open_or_create(temporary.path().join("authority")).unwrap());
        reference
            .write_atomic("enroll-selection.nrt", b"reference", PublishMode::CreateNew)
            .unwrap();
        let base = Arc::new(Snapshot {
            previous: None,
            records: vec![
                observed(&root, "original.nrt", MAX_SELECTION_BYTES),
                observed(&root, "anchor.nrt", MAX_BODY_BYTES),
                observed(&reference, "enroll-selection.nrt", MAX_SELECTION_BYTES),
            ],
            names: Vec::new(),
            root: Some(Arc::clone(&root)),
        });
        let mut rows = Vec::new();
        for ordinal in 1..=2_u8 {
            let row = Arc::new(container.create_child(body_name(ordinal).unwrap()).unwrap());
            row.write_atomic("reserved.nrt", b"reserved", PublishMode::CreateNew)
                .unwrap();
            row.write_atomic(
                metadata::FILE,
                &metadata::reserved_body(b"reserved").unwrap(),
                PublishMode::CreateNew,
            )
            .unwrap();
            rows.push(row);
        }
        let mut snapshot = Arc::new(Snapshot {
            previous: Some(base),
            records: Vec::new(),
            names: vec![NamesSnapshot::from_read(
                Arc::clone(&container),
                usize::from(MAX_BODIES) * 3,
                container.entries(usize::from(MAX_BODIES) * 3).unwrap(),
            )],
            root: None,
        });
        for (index, row) in rows.iter().enumerate() {
            let name = body_name(u8::try_from(index + 1).unwrap()).unwrap();
            snapshot = Arc::new(Snapshot {
                previous: Some(snapshot),
                records: vec![
                    observed(row, "reserved.nrt", MAX_BODY_BYTES),
                    observed(row, "original.nrt", journal::MAX_ORIGINAL_BYTES),
                    observed(
                        &container,
                        &format!("{name}-activation.nrt"),
                        MAX_SELECTION_BYTES,
                    ),
                    observed(
                        &container,
                        &format!("{name}-unused.nrt"),
                        MAX_SELECTION_BYTES,
                    ),
                ],
                names: vec![NamesSnapshot::from_reserved_read(
                    Arc::clone(row),
                    row.entries(2).unwrap(),
                )],
                root: None,
            });
        }
        Self {
            temporary,
            root,
            container,
            first: Arc::clone(&rows[0]),
            reference,
            snapshot,
        }
    }
}

#[test]
fn generated_paid_body_snapshots_reject_changed_original_and_reference_then_retry_same_sources() {
    use crate::managed::stream_token_custody::renewal_tests::Fixture;
    let _resources = crate::managed::native_test_guard();
    let fixture = Fixture::enrolled(60_000);
    let history = BodyHistory::open(&fixture.owner, CustodyPurpose::InitialEnroll)
        .unwrap()
        .unwrap();
    assert_eq!(history.bodies.len(), 1);
    let body = &history.bodies[0];
    let snapshot = &body.snapshots;
    assert!(snapshot.tree_root().is_some());
    snapshot.revalidate().unwrap();
    snapshot.revalidate_in_tree(None).unwrap();
    let original = body
        .directory
        .read("original.nrt", journal::MAX_ORIGINAL_BYTES)
        .unwrap();
    let reservation = body.directory.read("reserved.nrt", MAX_BODY_BYTES).unwrap();
    let mut changed = original.to_vec();
    changed[0] ^= 1;
    body.directory
        .write_atomic("original.nrt", &changed, PublishMode::Replace)
        .unwrap();
    assert!(
        matches!(snapshot.revalidate(), Err(Error::Invalid(message)) if message == "retained enrollment body material changed")
    );
    assert!(
        matches!(snapshot.revalidate_in_tree(None), Err(Error::Invalid(message)) if message == "retained enrollment body material changed")
    );
    body.directory
        .write_atomic("original.nrt", &original, PublishMode::Replace)
        .unwrap();
    snapshot.revalidate().unwrap();
    let reference = &history.root_snapshots.records[2];
    assert!(!Arc::ptr_eq(&reference.directory, &history.root));
    let reference_bytes = reference
        .directory
        .read(&reference.name, reference.maximum)
        .unwrap();
    let held = reference.directory.path().join("held-selection");
    std::fs::rename(reference.directory.path().join(&reference.name), &held).unwrap();
    assert!(
        matches!(snapshot.revalidate(), Err(Error::Io(error)) if error.kind() == io::ErrorKind::NotFound)
    );
    std::fs::rename(&held, reference.directory.path().join(&reference.name)).unwrap();
    snapshot.revalidate().unwrap();
    assert_eq!(
        reference
            .directory
            .read(&reference.name, reference.maximum)
            .unwrap(),
        reference_bytes
    );
    assert_eq!(
        body.directory.read("reserved.nrt", MAX_BODY_BYTES).unwrap(),
        reservation
    );
    assert_eq!(
        body.directory
            .read("original.nrt", journal::MAX_ORIGINAL_BYTES)
            .unwrap(),
        original
    );
    let restored = BodyHistory::open(&fixture.owner, CustodyPurpose::InitialEnroll)
        .unwrap()
        .unwrap();
    restored.bodies[0].snapshots.revalidate().unwrap();
    assert_eq!(restored.bodies.len(), 1);
    assert_eq!(restored.anchor.highest, history.anchor.highest);
}

#[test]
fn snapshot_tree_keeps_inventory_hash_absence_order_and_late_active_codec_limits() {
    let store = Store::new();
    assert!(store.snapshot.tree_root().is_some());
    store.snapshot.revalidate().unwrap();
    store.snapshot.revalidate_in_tree(None).unwrap();
    store
        .first
        .write_atomic("reserved.nrt", b"modified", PublishMode::Replace)
        .unwrap();
    let second = &store.snapshot.records[0].directory;
    let held_second = store.temporary.path().join("held-second-reservation");
    std::fs::rename(second.path().join("reserved.nrt"), &held_second).unwrap();
    // The previous row's real hash refusal must precede the later row's real missing leaf.
    assert!(
        matches!(store.snapshot.revalidate(), Err(Error::Invalid(message)) if message == "retained enrollment body material changed")
    );
    store
        .first
        .write_atomic("reserved.nrt", b"reserved", PublishMode::Replace)
        .unwrap();
    assert!(
        matches!(store.snapshot.revalidate(), Err(Error::Io(error)) if error.kind() == io::ErrorKind::NotFound)
    );
    std::fs::rename(&held_second, second.path().join("reserved.nrt")).unwrap();
    store.snapshot.revalidate().unwrap();
    store
        .container
        .write_atomic("0001-activation.nrt", b"appeared", PublishMode::CreateNew)
        .unwrap();
    // Container inventory precedes body records, retaining the original first refusal.
    assert!(
        matches!(store.snapshot.revalidate(), Err(Error::Invalid(message)) if message == "retained enrollment namespace changed")
    );
    std::fs::remove_file(store.container.path().join("0001-activation.nrt")).unwrap();
    store
        .first
        .write_atomic("original.nrt", b"appeared", PublishMode::CreateNew)
        .unwrap();
    assert!(
        matches!(store.snapshot.revalidate(), Err(Error::Invalid(message)) if message == "retained enrollment body material changed")
    );
    std::fs::remove_file(store.first.path().join("original.nrt")).unwrap();
    let held = store.first.path().join("held");
    std::fs::rename(store.first.path().join("reserved.nrt"), &held).unwrap();
    assert!(
        matches!(store.snapshot.revalidate(), Err(Error::Io(error)) if error.kind() == io::ErrorKind::NotFound)
    );
    std::fs::rename(&held, store.first.path().join("reserved.nrt")).unwrap();
    let extra = store.container.create_child("0003").unwrap();
    drop(extra);
    assert!(
        matches!(store.snapshot.revalidate(), Err(Error::Invalid(message)) if message == "retained enrollment namespace changed")
    );
    std::fs::remove_dir(store.container.path().join("0003")).unwrap();
    store.snapshot.revalidate().unwrap();
    let encoded = encode(&vec![7_u8; 32], MAX_BODY_BYTES).unwrap();
    for allocation in [0, 1] {
        let limits = norito::DecodeLimits::new(
            MAX_BODY_BYTES,
            MAX_BODY_BYTES,
            MAX_BODY_BYTES,
            allocation,
            48,
        );
        let result = norito::core::with_decode_limits_scope(limits, || {
            store.snapshot.revalidate().unwrap();
            decode::<Vec<u8>>(&encoded, MAX_BODY_BYTES)
        });
        assert!(
            matches!(result, Err(Error::Invalid(message)) if message == "invalid canonical enrollment body record")
        );
    }
    let wide = norito::DecodeLimits::new(
        MAX_BODY_BYTES,
        MAX_BODY_BYTES,
        MAX_BODY_BYTES,
        96 * 1024 * 1024,
        48,
    );
    assert_eq!(
        norito::core::with_decode_limits_scope(wide, || {
            store.snapshot.revalidate().unwrap();
            decode::<Vec<u8>>(&encoded, MAX_BODY_BYTES)
        })
        .unwrap(),
        vec![7_u8; 32]
    );
    let one = norito::DecodeLimits::new(MAX_BODY_BYTES, MAX_BODY_BYTES, MAX_BODY_BYTES, 1, 48);
    norito::core::with_decode_limits_scope(one, || {
        norito::core::reserve_decode_allocation(1).unwrap();
        store.snapshot.revalidate().unwrap();
        assert!(norito::core::reserve_decode_allocation(1).is_err());
    });
    store.snapshot.revalidate().unwrap();
    assert_eq!(
        store
            .first
            .read("reserved.nrt", MAX_BODY_BYTES)
            .unwrap()
            .as_slice(),
        b"reserved"
    );
}

#[test]
fn snapshot_tree_eligibility_keeps_rootless_custom_multiple_root_and_singleton_fallbacks() {
    let store = Store::new();
    let rootless = Snapshot {
        previous: None,
        records: vec![observed(
            &store.reference,
            "enroll-selection.nrt",
            MAX_SELECTION_BYTES,
        )],
        names: Vec::new(),
        root: None,
    };
    assert!(rootless.tree_root().is_none());
    rootless.revalidate().unwrap();
    let custom = Snapshot {
        previous: Some(Arc::clone(&store.snapshot)),
        records: vec![observed(&store.first, "reserved.nrt", MAX_BODY_BYTES)],
        names: Vec::new(),
        root: None,
    };
    assert!(custom.tree_root().is_none());
    custom.revalidate().unwrap();
    let multiple = Snapshot {
        previous: Some(Arc::clone(&store.snapshot)),
        records: Vec::new(),
        names: Vec::new(),
        root: Some(Arc::clone(&store.root)),
    };
    assert!(multiple.tree_root().is_none());
    multiple.revalidate().unwrap();
    store
        .first
        .write_atomic("reserved.nrt", b"modified", PublishMode::Replace)
        .unwrap();
    for snapshot in [&custom, &multiple] {
        assert!(
            matches!(snapshot.revalidate(), Err(Error::Invalid(message)) if message == "retained enrollment body material changed")
        );
    }
    store
        .first
        .write_atomic("reserved.nrt", b"reserved", PublishMode::Replace)
        .unwrap();
    store
        .reference
        .write_atomic("enroll-selection.nrt", b"different", PublishMode::Replace)
        .unwrap();
    assert!(
        matches!(rootless.revalidate(), Err(Error::Invalid(message)) if message == "retained enrollment body material changed")
    );
    store
        .reference
        .write_atomic("enroll-selection.nrt", b"reference", PublishMode::Replace)
        .unwrap();
    rootless.revalidate().unwrap();
    custom.revalidate().unwrap();
    multiple.revalidate().unwrap();
    assert_eq!(store.root.entries(4).unwrap().len(), 4);
}

#[cfg(unix)]
#[test]
fn snapshot_tree_exit_overrides_real_body_results_and_restores_original_ancestor_and_suffix() {
    use std::{fs, os::unix::fs::PermissionsExt as _};
    for body in ["accepted_absence", "hash_refusal", "missing_required"] {
        let store = Store::new();
        let held = store.root.path().join("held-anchor");
        if body == "hash_refusal" {
            store
                .first
                .write_atomic("reserved.nrt", b"modified", PublishMode::Replace)
                .unwrap();
        } else if body == "missing_required" {
            // Move outside the root census so the real original leaf read reaches NotFound.
            fs::rename(store.root.path().join("anchor.nrt"), &held).unwrap();
            fs::rename(&held, store.temporary.path().join("held-anchor")).unwrap();
        }
        let refused = store.root.read_tree_scope(|tree| {
            let result = store.snapshot.revalidate_in_tree(Some(tree));
            match body {
                "accepted_absence" => assert!(result.is_ok()),
                "hash_refusal" => assert!(matches!(&result, Err(Error::Invalid(message)) if *message == "retained enrollment body material changed")),
                "missing_required" => assert!(matches!(&result, Err(Error::Io(error)) if error.kind() == io::ErrorKind::NotFound)),
                _ => unreachable!(),
            }
            fs::set_permissions(store.root.path(), fs::Permissions::from_mode(0o755)).unwrap();
            result
        });
        assert!(
            matches!(refused, Err(Error::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied)
        );
        fs::set_permissions(store.root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        if body == "hash_refusal" {
            store
                .first
                .write_atomic("reserved.nrt", b"reserved", PublishMode::Replace)
                .unwrap();
        } else if body == "missing_required" {
            fs::rename(
                store.temporary.path().join("held-anchor"),
                store.root.path().join("anchor.nrt"),
            )
            .unwrap();
        }
        store.snapshot.revalidate().unwrap();
        let identity = store.first.identity().unwrap();
        fs::set_permissions(store.first.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            matches!(store.snapshot.revalidate(), Err(Error::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied)
        );
        fs::set_permissions(store.first.path(), fs::Permissions::from_mode(0o700)).unwrap();
        store.snapshot.revalidate().unwrap();
        assert_eq!(store.first.identity().unwrap(), identity);
    }
    let store = Store::new();
    store.root.read_tree_scope(|tree| {
        // The root's own full checks and external singleton must still see current custody.
        fs::set_permissions(store.reference.path(), fs::Permissions::from_mode(0o755)).unwrap();
        let result = store.snapshot.revalidate_in_tree(Some(tree));
        assert!(matches!(result, Err(Error::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied));
        fs::set_permissions(store.reference.path(), fs::Permissions::from_mode(0o700)).unwrap();
        store.snapshot.revalidate_in_tree(Some(tree))
    }).unwrap();
    store
        .root
        .read_tree_scope(|tree| {
            store.snapshot.revalidate_in_tree(Some(tree))?;
            fs::set_permissions(store.root.path(), fs::Permissions::from_mode(0o755)).unwrap();
            assert_eq!(
                store.root.revalidate().unwrap_err().kind(),
                io::ErrorKind::PermissionDenied
            );
            fs::set_permissions(store.root.path(), fs::Permissions::from_mode(0o700)).unwrap();
            Ok::<_, Error>(())
        })
        .unwrap();
    // Fully restored common-prefix changes inside the bracket have the documented temporal limit.
    store.snapshot.revalidate().unwrap();
}

#[test]
fn snapshot_closed_reader_keeps_native_count_bound_equal_count_census_and_original_retry() {
    let store = Store::new();
    assert!(store.snapshot.tree_root().is_some());
    let original = store.first.read("reserved.nrt", MAX_BODY_BYTES).unwrap();
    store
        .first
        .write_atomic("extra.nrt", b"extra", PublishMode::CreateNew)
        .unwrap();
    let native = store.first.entries(2).unwrap_err();
    assert_eq!(native.kind(), io::ErrorKind::InvalidInput);
    for result in [
        store.snapshot.revalidate(),
        store.snapshot.revalidate_in_tree(None),
    ] {
        match result {
            Err(Error::Io(error)) => {
                assert_eq!(error.kind(), native.kind());
                assert_eq!(error.to_string(), native.to_string());
            }
            other => panic!("native inventory cap refusal changed: {other:?}"),
        }
    }
    std::fs::remove_file(store.first.path().join("extra.nrt")).unwrap();
    store.snapshot.revalidate().unwrap();
    #[cfg(unix)]
    {
        let first_name = body_name(1).unwrap();
        let moved = store.container.path().join("different-body");
        // Keep the count unchanged. The earlier container census must still reject these names
        // before a later body's retained suffix or required record is visited.
        std::fs::rename(store.first.path(), &moved).unwrap();
        for result in [
            store.snapshot.revalidate(),
            store.snapshot.revalidate_in_tree(None),
        ] {
            assert!(
                matches!(result, Err(Error::Invalid(message)) if message == "retained enrollment namespace changed")
            );
        }
        std::fs::rename(&moved, store.container.path().join(first_name)).unwrap();
    }
    #[cfg(windows)]
    {
        let moved = store.container.path().join("different-body");
        // Retained native directory handles deny DELETE sharing on Windows.
        assert!(std::fs::rename(store.first.path(), &moved).is_err());
        assert!(!moved.exists());
    }
    store.snapshot.revalidate().unwrap();
    store.snapshot.revalidate_in_tree(None).unwrap();
    assert_eq!(
        store.first.read("reserved.nrt", MAX_BODY_BYTES).unwrap(),
        original
    );
}

#[cfg(unix)]
#[test]
fn snapshot_closed_reader_suffix_exit_keeps_typed_body_refusal_and_restored_boundary() {
    use std::{fs, os::unix::fs::PermissionsExt as _};
    let store = Store::new();
    let first = &store.snapshot.previous.as_ref().unwrap().records[0];
    assert!(Arc::ptr_eq(&first.directory, &store.first));
    let failure = store
        .root
        .read_tree_scope(|tree| {
            tree.read_scope(&store.first, |reader| {
                first.revalidate_in_scope(reader)?;
                fs::set_permissions(store.first.path(), fs::Permissions::from_mode(0o755)).unwrap();
                Err::<(), _>(invalid("typed snapshot consumer refusal"))
            })
        })
        .unwrap_err();
    assert!(matches!(failure, Error::Io(error) if error.kind() == io::ErrorKind::PermissionDenied));
    fs::set_permissions(store.first.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let typed = store
        .root
        .read_tree_scope(|tree| {
            tree.read_scope(&store.first, |reader| {
                first.revalidate_in_scope(reader)?;
                Err::<(), _>(invalid("typed snapshot consumer refusal"))
            })
        })
        .unwrap_err();
    assert!(
        matches!(typed, Error::Invalid(message) if message == "typed snapshot consumer refusal")
    );
    store
        .root
        .read_tree_scope(|tree| {
            tree.read_scope(&store.first, |reader| {
                fs::set_permissions(store.first.path(), fs::Permissions::from_mode(0o755)).unwrap();
                assert_eq!(
                    store.first.revalidate().unwrap_err().kind(),
                    io::ErrorKind::PermissionDenied
                );
                first.revalidate_in_scope(reader)?;
                fs::set_permissions(store.first.path(), fs::Permissions::from_mode(0o700)).unwrap();
                // This fully restored intermediate suffix change has the documented endpoint limit.
                Ok::<_, Error>(())
            })
        })
        .unwrap();
    store.snapshot.revalidate().unwrap();
    store.snapshot.revalidate_in_tree(None).unwrap();
}

// Observe the actual shared producers. These hooks never select production validity,
// replace native reads, or carry a verdict across Snapshot::revalidate calls.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ContainerPoint {
    Record(usize, String, Option<(usize, [u8; 32])>),
    Names(usize, Vec<std::ffi::OsString>),
    NodeClosed(usize),
}
#[derive(Debug, Default, PartialEq, Eq)]
struct ContainerCounts {
    entered: usize,
    points: Vec<ContainerPoint>,
}
type ContainerHook = Box<dyn FnMut(&ContainerPoint) -> Result<()>>;
struct ContainerState {
    original: bool,
    counts: ContainerCounts,
    hook: Option<ContainerHook>,
}
thread_local! {
    static CONTAINER_STATE: std::cell::RefCell<Option<ContainerState>> = const {
        std::cell::RefCell::new(None)
    };
}
pub(super) fn original_container_recipe() -> bool {
    CONTAINER_STATE.with(|state| state.borrow().as_ref().is_some_and(|state| state.original))
}
pub(super) fn container_entered() {
    CONTAINER_STATE.with(|state| {
        if let Some(state) = state.borrow_mut().as_mut() {
            state.counts.entered += 1;
        }
    });
}
fn container_point(make: impl FnOnce() -> ContainerPoint) -> Result<()> {
    let Some((point, mut hook)) = CONTAINER_STATE.with(|state| {
        state.borrow_mut().as_mut().map(|state| {
            let point = make();
            state.counts.points.push(point.clone());
            (point, state.hook.take())
        })
    }) else {
        return Ok(());
    };
    let result = hook.as_mut().map_or(Ok(()), |hook| hook(&point));
    CONTAINER_STATE.with(|state| {
        if let Some(state) = state.borrow_mut().as_mut() {
            state.hook = hook;
        }
    });
    result
}
pub(super) fn after_container_record(
    record: &RecordSnapshot,
    observed: Option<(usize, [u8; 32])>,
) -> Result<()> {
    container_point(|| {
        ContainerPoint::Record(
            Arc::as_ptr(&record.directory).addr(),
            record.name.clone(),
            observed,
        )
    })
}
pub(super) fn after_container_names(
    names: &NamesSnapshot,
    observed: &[std::ffi::OsString],
) -> Result<()> {
    container_point(|| {
        ContainerPoint::Names(Arc::as_ptr(&names.directory).addr(), observed.to_vec())
    })
}
pub(super) fn after_container_node(snapshot: &Snapshot) -> Result<()> {
    container_point(|| ContainerPoint::NodeClosed(std::ptr::from_ref(snapshot).addr()))
}
fn observe_container<T>(
    original: bool,
    hook: Option<ContainerHook>,
    read: impl FnOnce() -> T,
) -> (T, ContainerCounts, iroha_fs::CustodyWorkV1) {
    struct Restore(Option<ContainerState>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CONTAINER_STATE.with(|state| state.replace(self.0.take()));
        }
    }
    let _restore = Restore(CONTAINER_STATE.with(|state| {
        state.replace(Some(ContainerState {
            original,
            counts: ContainerCounts::default(),
            hook,
        }))
    }));
    let before = iroha_fs::CustodyWorkV1::current_thread();
    let result = read();
    let work = iroha_fs::CustodyWorkV1::current_thread().since(before);
    let counts = CONTAINER_STATE.with(|state| state.take().unwrap().counts);
    (result, counts, work)
}
fn expected_container_order(snapshot: &Snapshot, points: &mut Vec<ContainerPoint>) {
    if let Some(previous) = &snapshot.previous {
        expected_container_order(previous, points);
    }
    for record in &snapshot.records {
        points.push(ContainerPoint::Record(
            Arc::as_ptr(&record.directory).addr(),
            record.name.clone(),
            record.observed,
        ));
    }
    for names in &snapshot.names {
        points.push(ContainerPoint::Names(
            Arc::as_ptr(&names.directory).addr(),
            names.names.clone(),
        ));
    }
    points.push(ContainerPoint::NodeClosed(
        std::ptr::from_ref(snapshot).addr(),
    ));
}
fn container_outcome(result: Result<()>) -> std::result::Result<(), String> {
    result.map_err(|error| error.to_string())
}
fn container_limits(allocation: usize) -> norito::DecodeLimits {
    norito::DecodeLimits::new(
        MAX_BODY_BYTES,
        MAX_ALL_BODY_BYTES,
        MAX_ALL_BODY_BYTES,
        allocation,
        64,
    )
}

#[test]
fn snapshot_container_shares_actual_native_work_with_all_ordered_bytes_and_inventories() {
    let store = Store::new();
    let read = |original| {
        observe_container(original, None, || {
            shared_snapshot_tests::snapshot_work(|| store.snapshot.revalidate())
        })
    };
    let ((old, old_visits), old_order, old_work) = read(true);
    let ((new, new_visits), new_order, new_work) = read(false);
    old.unwrap();
    new.unwrap();
    assert_eq!(old_visits, (11, 4)); // Three base records plus four per body; base/container counted.
    assert_eq!(old_visits, new_visits);
    assert_eq!((old_order.entered, new_order.entered), (0, 1));
    assert_eq!(old_order.points, new_order.points);
    let mut expected = Vec::new();
    expected_container_order(&store.snapshot, &mut expected);
    assert_eq!(new_order.points, expected);
    // Both unsigned bodies retain two independent inventories and the real metadata decoder.
    // Only the two activation/unused container brackets are removed; body suffixes stay intact.
    assert_eq!(old_work.ancestry_walks, new_work.ancestry_walks + 4);
    assert_eq!(old_work.ancestry_opens, new_work.ancestry_opens + 4);
    let names = new_order
        .points
        .iter()
        .filter(|point| matches!(point, ContainerPoint::Names(..)))
        .count();
    assert_eq!(names, 3);
    metadata::require_reserved(&store.first).unwrap();
    store.snapshot.revalidate().unwrap();

    let _resources = crate::managed::native_test_guard();
    let (_fixture, history) = shared_snapshot_tests::history_with_bodies(3);
    let head = &history.bodies.last().unwrap().snapshots;
    let read = |original| {
        observe_container(original, None, || {
            shared_snapshot_tests::snapshot_work(|| head.revalidate())
        })
    };
    let ((old, old_visits), old_order, old_work) = read(true);
    let ((new, new_visits), new_order, new_work) = read(false);
    old.unwrap();
    new.unwrap();
    assert_eq!(old_visits, (15, 5));
    assert_eq!(old_visits, new_visits);
    assert_eq!((old_order.entered, new_order.entered), (0, 1));
    assert_eq!(old_order.points, new_order.points);
    assert_eq!(old_work.ancestry_walks, new_work.ancestry_walks + 6);
    assert_eq!(old_work.ancestry_opens, new_work.ancestry_opens + 6);
    history
        .current_history()
        .unwrap()
        .test_require_current(6)
        .0
        .unwrap();
}

#[test]
fn snapshot_container_keeps_custom_owners_and_inherited_or_late_decoder_recipes() {
    let store = Store::new();
    let cloned = || Snapshot {
        previous: store.snapshot.previous.clone(),
        records: store.snapshot.records.clone(),
        names: store.snapshot.names.clone(),
        root: store.snapshot.root.clone(),
    };
    for change in ["rootless", "bound", "foreign"] {
        let mut snapshot = cloned();
        match change {
            "rootless" => snapshot.previous = None,
            "bound" => snapshot.records[0].maximum -= 1,
            "foreign" => {
                let foreign =
                    Arc::new(PrivateDirectory::open_exact(store.container.path()).unwrap());
                snapshot.records[2].directory = Arc::clone(&foreign);
                snapshot.records[3].directory = foreign;
            }
            _ => unreachable!(),
        }
        assert!(snapshot.tree_root().is_none());
        let read = |original| {
            observe_container(original, None, || container_outcome(snapshot.revalidate()))
        };
        let old = read(true);
        assert_eq!(old, read(false), "{change}");
        assert!(old.0.is_ok());
        assert_eq!(old.1.entered, 0);
    }
    let encoded = encode(&vec![7_u8; 32], MAX_BODY_BYTES).unwrap();
    let read = |original, allocation| {
        norito::core::with_decode_limits_measured(container_limits(allocation), || {
            observe_container(original, None, || {
                container_outcome(store.snapshot.revalidate().and_then(|()| {
                    assert_eq!(decode::<Vec<u8>>(&encoded, MAX_BODY_BYTES)?, vec![7_u8; 32]);
                    Ok(())
                }))
            })
        })
    };
    let wide = read(true, usize::MAX);
    assert_eq!(wide, read(false, usize::MAX));
    assert!(wide.0.0.is_ok());
    let exact = wide.1.total_allocated_bytes();
    assert!(exact >= 32); // The actual owning Vec control, not an empty metadata image assumption.
    for allocation in [0, 1, exact - 1, exact] {
        let old = read(true, allocation);
        assert_eq!(old, read(false, allocation));
        assert_eq!(old.0.1.entered, 0);
        assert_eq!(old.0.0.is_ok(), allocation == exact);
    }
    // Install the real decoder owner after the outer tree has already admitted its root.
    for allocation in [0, 1, usize::MAX] {
        let late = |original| {
            store
                .root
                .read_tree_scope(|tree| {
                    Ok::<_, Error>(norito::core::with_decode_limits_measured(
                        container_limits(allocation),
                        || {
                            observe_container(original, None, || {
                                container_outcome(
                                    store.snapshot.revalidate_with_container(tree, &store.root),
                                )
                            })
                        },
                    ))
                })
                .unwrap()
        };
        let old = late(true);
        assert_eq!(old, late(false));
        assert!(old.0.0.is_ok());
        assert_eq!(old.0.1.entered, 0);
        assert_eq!(old.1.total_allocated_bytes(), 0);
    }
    // A decoder arriving after the container reader exists still disables every reader reuse.
    let container = store
        .snapshot
        .previous
        .as_ref()
        .unwrap()
        .previous
        .as_ref()
        .unwrap();
    let base = container.previous.as_ref().unwrap();
    for allocation in [0, 1, usize::MAX] {
        let late = |original| {
            store
                .root
                .read_tree_scope(|tree| {
                    base.revalidate_in_tree_with_root(Some(tree), Some(&store.root))?;
                    tree.read_scope_with_tree(&store.container, |tree, reader| {
                        let mut shared = SnapshotContainerRead {
                            directory: &store.container,
                            reader,
                        };
                        Ok::<_, Error>(norito::core::with_decode_limits_measured(
                            container_limits(allocation),
                            || {
                                assert!(!shared.covers(&store.container));
                                observe_container(original, None, || {
                                    container_outcome(store.snapshot.revalidate_in_scope(
                                        Some(tree),
                                        Some(&store.root),
                                        Some(base),
                                        if original { None } else { Some(&mut shared) },
                                    ))
                                })
                            },
                        ))
                    })
                })
                .unwrap()
        };
        let old = late(true);
        assert_eq!(old, late(false));
        assert!(old.0.0.is_ok());
        assert_eq!(old.1.total_allocated_bytes(), 0);
    }
    store.snapshot.revalidate().unwrap();
}

#[test]
fn snapshot_container_keeps_fresh_inventory_absence_metadata_and_base_error_order() {
    let store = Store::new();
    let assert_same_refusal = |expected: &str| {
        let read = |original| {
            observe_container(original, None, || {
                shared_snapshot_tests::snapshot_work(|| {
                    container_outcome(store.snapshot.revalidate())
                })
            })
        };
        let old = read(true);
        let new = read(false);
        assert_eq!(old.0, new.0);
        assert_eq!(old.1.points, new.1.points);
        assert!(new.0.0.unwrap_err().contains(expected));
    };
    store
        .first
        .write_atomic("reserved.nrt", b"modified", PublishMode::Replace)
        .unwrap();
    assert_same_refusal("retained enrollment body material changed");
    store
        .first
        .write_atomic("reserved.nrt", b"reserved", PublishMode::Replace)
        .unwrap();
    store
        .first
        .write_atomic("original.nrt", b"appeared", PublishMode::CreateNew)
        .unwrap();
    assert_same_refusal("retained enrollment body material changed");
    std::fs::remove_file(store.first.path().join("original.nrt")).unwrap();
    store
        .container
        .write_atomic("0001-activation.nrt", b"appeared", PublishMode::CreateNew)
        .unwrap();
    assert_same_refusal("retained enrollment namespace changed");
    std::fs::remove_file(store.container.path().join("0001-activation.nrt")).unwrap();
    let image = store
        .first
        .read(metadata::FILE, metadata::MAX_IMAGE_BYTES)
        .unwrap();
    store
        .first
        .write_atomic(metadata::FILE, b"broken", PublishMode::Replace)
        .unwrap();
    assert_same_refusal("invalid canonical operation metadata image");
    store
        .first
        .write_atomic(metadata::FILE, &image, PublishMode::Replace)
        .unwrap();
    store.snapshot.revalidate().unwrap();
    #[cfg(unix)]
    {
        use std::{fs, os::unix::fs::PermissionsExt as _};
        // The base external reference must refuse before entry notices a bad container.
        fs::set_permissions(store.container.path(), fs::Permissions::from_mode(0o755)).unwrap();
        store
            .reference
            .write_atomic("enroll-selection.nrt", b"changed", PublishMode::Replace)
            .unwrap();
        for original in [true, false] {
            let (result, counts, _) = observe_container(original, None, || {
                container_outcome(store.snapshot.revalidate())
            });
            assert!(
                result
                    .unwrap_err()
                    .contains("retained enrollment body material changed")
            );
            assert_eq!(counts.entered, 0);
            assert!(
                !counts
                    .points
                    .iter()
                    .any(|point| matches!(point, ContainerPoint::Names(..)))
            );
        }
        store
            .reference
            .write_atomic("enroll-selection.nrt", b"reference", PublishMode::Replace)
            .unwrap();
        fs::set_permissions(store.container.path(), fs::Permissions::from_mode(0o700)).unwrap();
        store.snapshot.revalidate().unwrap();
    }
}

#[cfg(unix)]
#[test]
fn snapshot_container_closes_persistent_mutation_after_inner_error_and_retries_same_owners() {
    use std::{fs, os::unix::fs::PermissionsExt as _};
    let store = Store::new();
    let head = Arc::as_ptr(&store.snapshot).addr();
    let identity = store.container.identity().unwrap();
    for original in [true, false] {
        let path = store.container.path().to_owned();
        let hook = Box::new(move |point: &ContainerPoint| {
            if *point == ContainerPoint::NodeClosed(head) {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
                return Err(invalid("ordinary completed body refusal"));
            }
            Ok(())
        });
        let (result, _, _) =
            observe_container(original, Some(hook), || store.snapshot.revalidate());
        fs::set_permissions(store.container.path(), fs::Permissions::from_mode(0o700)).unwrap();
        if original {
            assert!(
                matches!(result, Err(Error::Invalid(message)) if message == "ordinary completed body refusal")
            );
        } else {
            assert!(
                matches!(result, Err(Error::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied)
            );
        }
        assert_eq!(store.container.identity().unwrap(), identity);
        store.snapshot.revalidate().unwrap();
    }
    // Rename the original container after its last inner suffix has closed. The new outer
    // container fence must reject that different directory even when the inner result failed.
    let held = store.temporary.path().join("held-snapshot-container");
    let path = store.container.path().to_owned();
    let hook_path = path.clone();
    let hook_held = held.clone();
    let hook = Box::new(move |point: &ContainerPoint| {
        if *point == ContainerPoint::NodeClosed(head) {
            fs::rename(&hook_path, &hook_held)?;
            fs::create_dir(&hook_path)?;
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o700))?;
            return Err(invalid("ordinary completed body refusal"));
        }
        Ok(())
    });
    let (result, _, _) = observe_container(false, Some(hook), || store.snapshot.revalidate());
    fs::remove_dir(&path).unwrap();
    fs::rename(&held, &path).unwrap();
    assert!(matches!(result, Err(Error::Io(_))));
    store.snapshot.revalidate().unwrap();
    // Fully restored changes within the retained bracket have the documented endpoint limit.
    let path = store.container.path().to_owned();
    let hook = Box::new(move |point: &ContainerPoint| {
        if *point == ContainerPoint::NodeClosed(head) {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    });
    observe_container(false, Some(hook), || store.snapshot.revalidate())
        .0
        .unwrap();
    assert_eq!(store.container.identity().unwrap(), identity);
    store.snapshot.revalidate().unwrap();
}
