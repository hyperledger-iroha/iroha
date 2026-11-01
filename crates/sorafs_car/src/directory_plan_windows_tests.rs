//! Real Windows directory planning, retained custody and fixed public bounds.

use super::*;
use std::{
    ffi::{OsString, c_void},
    fs::{self, OpenOptions},
    mem::size_of,
    os::windows::{ffi::OsStringExt as _, io::AsRawHandle as _},
    process::Command,
};

fn fixture(private: bool) -> (tempfile::TempDir, PathBuf) {
    let temporary = tempfile::tempdir().expect("native directory fixture");
    let root = temporary.path().join("source");
    if private {
        drop(iroha_fs::PrivateDirectory::open_or_create(&root).expect("private source"));
    } else {
        fs::create_dir(&root).expect("ordinary owner source");
    }
    (temporary, root)
}

fn write_file(root: &Path, parts: &[&str], bytes: &[u8]) {
    let mut path = root.to_path_buf();
    for part in parts {
        path.push(part);
    }
    fs::create_dir_all(path.parent().expect("file parent")).expect("parent directories");
    fs::write(path, bytes).expect("fixture bytes");
}

fn acl(path: &Path) -> Vec<u8> {
    let result = Command::new("icacls.exe")
        .arg(path)
        .output()
        .expect("read ACL");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}

fn grant_write(path: &Path) {
    let result = Command::new("icacls.exe")
        .arg(path)
        .args(["/grant", "*S-1-1-0:(W)"])
        .output()
        .expect("grant unsafe fixture write access");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn junction(path: &Path, target: &Path) {
    let result = Command::new("cmd.exe")
        .current_dir(path.parent().expect("junction parent"))
        .args(["/D", "/C", "mklink", "/J"])
        .arg(path)
        .arg(target)
        .output()
        .expect("create native junction");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn matches_explicit(root: &Path, entries: Vec<FileEntry>, profile: ChunkProfile) {
    let expected =
        CarBuildPlan::from_files_with_profile(entries, profile).expect("explicit oracle");
    let actual = CarBuildPlan::from_directory_with_profile(root, profile).expect("native scanner");
    assert_eq!(actual, expected);
    let mut expected_car = Vec::new();
    CarWriter::new(&expected.0, &expected.1)
        .unwrap()
        .write_to(&mut expected_car)
        .unwrap();
    let mut actual_car = Vec::new();
    CarWriter::new(&actual.0, &actual.1)
        .unwrap()
        .write_to(&mut actual_car)
        .unwrap();
    assert_eq!(actual_car, expected_car);
}

#[test]
fn native_directory_plan_matches_explicit_utf8_order_and_case_spelling() {
    let (_temporary, root) = fixture(false);
    let entries = vec![
        FileEntry {
            path: vec!["\u{10000}.txt".into()],
            data: b"supplementary".to_vec(),
        },
        FileEntry {
            path: vec!["\u{e000}.txt".into()],
            data: b"bmp".to_vec(),
        },
        FileEntry {
            path: vec!["Zed.TXT".into()],
            data: b"case-preserved".to_vec(),
        },
        FileEntry {
            path: vec!["docs".into(), "空.txt".into()],
            data: "SORA 空\n".as_bytes().to_vec(),
        },
        FileEntry {
            path: vec!["empty.txt".into()],
            data: Vec::new(),
        },
    ];
    for file in &entries {
        let parts: Vec<_> = file.path.iter().map(String::as_str).collect();
        write_file(&root, &parts, &file.data);
    }
    let mut opened = Vec::new();
    let gathered = gather_files_secure(&root, &mut |path| {
        opened.push(path.file_name().unwrap().to_os_string());
        Ok(())
    })
    .expect("retained actual inventory");
    // UTF-16 ordinal order puts the surrogate pair before U+E000. Protocol paths
    // instead keep canonical UTF-8 order, independent of native enumeration order.
    let bmp = opened
        .iter()
        .position(|name| name == "\u{e000}.txt")
        .unwrap();
    let supplementary = opened
        .iter()
        .position(|name| name == "\u{10000}.txt")
        .unwrap();
    assert!(bmp < supplementary);
    assert!(gathered.iter().any(|file| file.path == ["Zed.TXT"]));
    let default = CarBuildPlan::from_directory(&root).expect("public default plan");
    assert_eq!(default, CarBuildPlan::from_files(entries.clone()).unwrap());
    matches_explicit(
        &root,
        entries,
        ChunkProfile {
            min_size: 1,
            target_size: 1,
            max_size: 1,
            break_mask: 1,
        },
    );
}

#[test]
fn native_directory_plan_accepts_safe_normal_and_private_acls_without_repair() {
    for private in [false, true] {
        let (_temporary, root) = fixture(private);
        write_file(&root, &["payload"], b"unchanged native source");
        let file = root.join("payload");
        let before = (acl(&root), acl(&file));
        let directory_id = iroha_fs::ReaderDirectory::open(&root)
            .unwrap()
            .identity()
            .unwrap();
        let source = iroha_fs::RetainedFile::open_regular(&file).unwrap();
        let snapshot = source.snapshot().unwrap();
        drop(source);
        let (_, payload) = CarBuildPlan::from_directory(&root).unwrap();
        assert_eq!(payload, b"unchanged native source");
        assert_eq!((acl(&root), acl(&file)), before);
        assert_eq!(
            iroha_fs::ReaderDirectory::open(&root)
                .unwrap()
                .identity()
                .unwrap(),
            directory_id
        );
        assert_eq!(
            iroha_fs::RetainedFile::open_regular(&file)
                .unwrap()
                .snapshot()
                .unwrap(),
            snapshot
        );
    }
}

#[test]
fn native_directory_plan_refuses_unsafe_file_or_directory_acl_without_repair() {
    for directory in [false, true] {
        let (_temporary, root) = fixture(true);
        write_file(&root, &["payload"], b"protected bytes");
        let changed = if directory {
            root.clone()
        } else {
            root.join("payload")
        };
        grant_write(&changed);
        let before = acl(&changed);
        assert!(CarBuildPlan::from_directory(&root).is_err());
        assert_eq!(acl(&changed), before);
        assert_eq!(fs::read(root.join("payload")).unwrap(), b"protected bytes");
    }
}

#[test]
fn native_directory_plan_refuses_live_writer_then_recovers_after_its_close() {
    let (_temporary, root) = fixture(false);
    write_file(&root, &["payload"], b"source bytes");
    let writer = OpenOptions::new()
        .write(true)
        .open(root.join("payload"))
        .unwrap();
    assert!(CarBuildPlan::from_directory(&root).is_err());
    drop(writer);
    assert_eq!(
        CarBuildPlan::from_directory(&root).unwrap().1,
        b"source bytes"
    );
}

#[test]
fn native_directory_scan_originals_exclude_write_delete_and_rename() {
    let (temporary, root) = fixture(false);
    write_file(&root, &["payload"], b"original bytes");
    let mut calls = 0;
    let files = gather_files_secure(&root, &mut |path| {
        calls += 1;
        let write_error = OpenOptions::new().write(true).open(path).unwrap_err();
        assert_eq!(write_error.raw_os_error(), Some(32));
        assert_eq!(fs::remove_file(path).unwrap_err().raw_os_error(), Some(32));
        assert_eq!(
            fs::rename(path, temporary.path().join("renamed-file"))
                .unwrap_err()
                .raw_os_error(),
            Some(32)
        );
        assert_eq!(
            fs::rename(&root, temporary.path().join("renamed-root"))
                .unwrap_err()
                .raw_os_error(),
            Some(32)
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].data, b"original bytes");
    assert!(root.join("payload").is_file());
}

#[test]
fn native_directory_plan_rejects_hardlinks_and_preserves_outside_original() {
    let (temporary, root) = fixture(false);
    let outside = temporary.path().join("outside");
    fs::write(&outside, b"outside sentinel").unwrap();
    fs::hard_link(&outside, root.join("linked")).unwrap();
    assert!(CarBuildPlan::from_directory(&root).is_err());
    assert_eq!(fs::read(&outside).unwrap(), b"outside sentinel");
    assert_eq!(fs::read(root.join("linked")).unwrap(), b"outside sentinel");
}

#[test]
fn native_directory_plan_rejects_root_and_child_junctions_without_following() {
    let (temporary, root) = fixture(false);
    write_file(&root, &["payload"], b"inside");
    let outside = temporary.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sentinel"), b"unchanged outside").unwrap();
    let child = root.join("junction");
    junction(&child, &outside);
    let mut child_opened = 0;
    assert!(
        gather_files_secure(&root, &mut |_| {
            child_opened += 1;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(
        child_opened, 0,
        "junction sorts before payload; no file may be admitted"
    );
    assert!(CarBuildPlan::from_directory(&root).is_err());
    assert_eq!(
        fs::read(outside.join("sentinel")).unwrap(),
        b"unchanged outside"
    );
    fs::remove_dir(&child).expect("remove only owned junction");
    let alias = temporary.path().join("root-junction");
    junction(&alias, &root);
    let mut root_opened = 0;
    assert!(
        gather_files_secure(&alias, &mut |_| {
            root_opened += 1;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(
        root_opened, 0,
        "root junction must refuse before any file admission"
    );
    assert!(CarBuildPlan::from_directory(&alias).is_err());
    fs::remove_dir(&alias).expect("remove only owned root junction");
    let ancestor = temporary.path().join("ancestor-junction");
    junction(&ancestor, temporary.path());
    let mut ancestor_opened = 0;
    assert!(
        gather_files_secure(&ancestor.join("source"), &mut |_| {
            ancestor_opened += 1;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(
        ancestor_opened, 0,
        "ancestor junction must refuse before any file admission"
    );
    assert!(CarBuildPlan::from_directory(&ancestor.join("source")).is_err());
    fs::remove_dir(&ancestor).expect("remove only owned ancestor junction");
    assert_eq!(fs::read(root.join("payload")).unwrap(), b"inside");
}

#[test]
fn native_directory_plan_refuses_unpaired_utf16_and_overlong_utf8_component() {
    for name in [
        OsString::from_wide(&[0xd800]),
        OsString::from("\u{e000}".repeat(86)),
    ] {
        let (_temporary, root) = fixture(false);
        fs::write(root.join(&name), b"invalid logical name").unwrap();
        assert!(matches!(
            CarBuildPlan::from_directory(&root),
            Err(CarPlanError::NonUtf8Path(_) | CarPlanError::InvalidPath(_))
        ));
        assert_eq!(fs::read(root.join(&name)).unwrap(), b"invalid logical name");
    }
}

#[test]
fn native_directory_scan_refuses_midscan_acl_or_inventory_mutation() {
    for change in ["file-acl", "root-acl", "new-entry"] {
        let (_temporary, root) = fixture(true);
        write_file(&root, &["a"], b"first");
        write_file(&root, &["b"], b"second");
        let mut calls = 0;
        let result = gather_files_secure(&root, &mut |path| {
            calls += 1;
            if calls == 1 {
                match change {
                    "file-acl" => grant_write(path),
                    "root-acl" => grant_write(&root),
                    "new-entry" => fs::write(root.join("added"), b"late entry")?,
                    _ => unreachable!(),
                }
            }
            Ok(())
        });
        assert!(
            result.is_err(),
            "mutation {change} returned a partial or complete inventory"
        );
        assert!(calls >= 1);
        assert_eq!(fs::read(root.join("a")).unwrap(), b"first");
        assert_eq!(fs::read(root.join("b")).unwrap(), b"second");
    }
}

#[test]
fn native_directory_scan_name_order_preserves_spelling_and_refuses_duplicates() {
    let path = Path::new("source");
    let mut names = vec![
        "\u{10000}".into(),
        "a".into(),
        "\u{e000}".into(),
        "A".into(),
    ];
    directory_plan_windows::sort_directory_names(&mut names, path).unwrap();
    assert_eq!(names, ["A", "a", "\u{e000}", "\u{10000}"]);
    let mut empty = Vec::new();
    directory_plan_windows::sort_directory_names(&mut empty, path).unwrap();
    assert!(empty.is_empty());
    let mut duplicate = vec!["same".into(), "other".into(), "same".into()];
    assert!(matches!(
        directory_plan_windows::sort_directory_names(&mut duplicate, path),
        Err(CarPlanError::InvalidPath(_))
    ));
}

#[test]
fn native_directory_scan_refuses_removed_or_renamed_visited_names() {
    for change in ["remove", "rename"] {
        let (_temporary, root) = fixture(true);
        write_file(&root, &["a"], b"first");
        write_file(&root, &["b"], b"second");
        let mut calls = 0;
        let result = gather_files_secure(&root, &mut |path| {
            calls += 1;
            if path.file_name() == Some(std::ffi::OsStr::new("b")) {
                // The first leaf's original has genuinely closed; the directory remains held.
                match change {
                    "remove" => fs::remove_file(root.join("a"))?,
                    "rename" => fs::rename(root.join("a"), root.join("renamed"))?,
                    _ => unreachable!(),
                }
            }
            Ok(())
        });
        assert!(
            result.is_err(),
            "mutation {change} returned a partial or complete inventory"
        );
        assert_eq!(
            calls, 2,
            "mutation must follow the first leaf's read and close"
        );
        assert!(!root.join("a").exists());
        assert_eq!(fs::read(root.join("b")).unwrap(), b"second");
        if change == "rename" {
            assert_eq!(fs::read(root.join("renamed")).unwrap(), b"first");
        }
    }
}

#[test]
fn native_directory_scan_refuses_added_name_in_retained_nested_directory() {
    let (_temporary, root) = fixture(true);
    write_file(&root, &["nested", "a"], b"first");
    write_file(&root, &["nested", "b"], b"second");
    let mut calls = 0;
    let result = gather_files_secure(&root, &mut |path| {
        calls += 1;
        if calls == 1 {
            assert_eq!(path.parent(), Some(root.join("nested").as_path()));
            fs::write(path.parent().unwrap().join("added"), b"late nested entry")?;
        }
        Ok(())
    });
    assert!(
        result.is_err(),
        "nested addition returned an incomplete inventory"
    );
    assert!(calls >= 1);
    assert_eq!(fs::read(root.join("nested/a")).unwrap(), b"first");
    assert_eq!(fs::read(root.join("nested/b")).unwrap(), b"second");
    assert_eq!(
        fs::read(root.join("nested/added")).unwrap(),
        b"late nested entry"
    );
}

#[test]
fn native_directory_scan_post_open_error_returns_no_partial_inventory_and_releases_originals() {
    let (temporary, root) = fixture(false);
    write_file(&root, &["a"], b"first");
    write_file(&root, &["b"], b"second");
    let mut calls = 0;
    let error = gather_files_secure(&root, &mut |_path| {
        calls += 1;
        if calls == 2 {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "owned second-file refusal",
            ))
        } else {
            Ok(())
        }
    })
    .expect_err("completed first entry must not escape as a partial inventory");
    assert_eq!(calls, 2);
    assert!(
        matches!(error, CarPlanError::DirectoryIo { source, .. } if source.kind() == io::ErrorKind::Interrupted && source.to_string() == "owned second-file refusal")
    );
    let writer = OpenOptions::new()
        .write(true)
        .open(root.join("a"))
        .expect("first original released");
    drop(writer);
    let moved = temporary.path().join("moved-after-refusal");
    fs::rename(&root, &moved).expect("all directory originals released");
    assert_eq!(
        CarBuildPlan::from_directory(&moved).unwrap().1,
        b"firstsecond"
    );
}

#[test]
fn native_directory_plan_exact_depth_and_component_limits_are_enforced() {
    let (_temporary, root) = fixture(false);
    let exact_component = "\u{e000}".repeat(CAR_LOGICAL_PATH_COMPONENT_MAX_BYTES / 3);
    assert_eq!(exact_component.len(), CAR_LOGICAL_PATH_COMPONENT_MAX_BYTES);
    write_file(&root, &[&exact_component], b"component boundary");
    assert_eq!(
        CarBuildPlan::from_directory(&root).unwrap().0.files[0].path,
        [exact_component]
    );
    let (_deep_temporary, deep_root) = fixture(false);
    let mut current = deep_root.clone();
    for _ in 0..CAR_LOGICAL_PATH_MAX_COMPONENTS - 1 {
        current.push("d");
        fs::create_dir(&current).unwrap();
    }
    fs::write(current.join("payload"), b"depth boundary").unwrap();
    assert_eq!(
        CarBuildPlan::from_directory(&deep_root).unwrap().0.files[0]
            .path
            .len(),
        CAR_LOGICAL_PATH_MAX_COMPONENTS
    );
    fs::remove_file(current.join("payload")).unwrap();
    current.push("d");
    fs::create_dir(&current).unwrap();
    fs::write(current.join("payload"), b"one component too deep").unwrap();
    assert!(matches!(
        CarBuildPlan::from_directory(&deep_root),
        Err(CarPlanError::InvalidPath(_))
    ));
}

#[test]
fn native_directory_counter_helpers_enforce_exact_bounds_and_overflow() {
    use super::directory_plan_windows::{add_payload_bytes, next_entry_count, next_file_count};
    assert_eq!(
        next_entry_count(CAR_DIRECTORY_MAX_ENTRIES - 1).unwrap(),
        CAR_DIRECTORY_MAX_ENTRIES
    );
    assert_eq!(
        next_file_count(CAR_PLAN_MAX_FILES - 1).unwrap(),
        CAR_PLAN_MAX_FILES
    );
    for (current, wanted) in [
        (CAR_DIRECTORY_MAX_ENTRIES, CAR_DIRECTORY_MAX_ENTRIES + 1),
        (usize::MAX, usize::MAX),
    ] {
        assert!(
            matches!(next_entry_count(current), Err(CarPlanError::TooManyDirectoryEntries { count, maximum }) if count == wanted && maximum == CAR_DIRECTORY_MAX_ENTRIES)
        );
    }
    for (current, wanted) in [
        (CAR_PLAN_MAX_FILES, CAR_PLAN_MAX_FILES + 1),
        (usize::MAX, usize::MAX),
    ] {
        assert!(
            matches!(next_file_count(current), Err(CarPlanError::TooManyFiles { count, maximum }) if count == wanted && maximum == CAR_PLAN_MAX_FILES)
        );
    }
    assert_eq!(
        add_payload_bytes(CAR_EAGER_DIRECTORY_MAX_BYTES - 1, 1).unwrap(),
        CAR_EAGER_DIRECTORY_MAX_BYTES
    );
    for (current, additional, wanted) in [
        (
            CAR_EAGER_DIRECTORY_MAX_BYTES,
            1,
            CAR_EAGER_DIRECTORY_MAX_BYTES + 1,
        ),
        (u64::MAX, 1, u64::MAX),
    ] {
        assert!(
            matches!(add_payload_bytes(current, additional), Err(CarPlanError::DirectoryPayloadTooLarge { bytes, maximum }) if bytes == wanted && maximum == CAR_EAGER_DIRECTORY_MAX_BYTES)
        );
    }
}

#[test]
fn native_directory_logical_path_helper_enforces_exact_bytes_and_depth() {
    use super::directory_plan_windows::validate_component;
    let native = Path::new("public-logical-boundary");
    let parents = vec!["p".repeat(240); 16];
    let exact = "n".repeat(240);
    assert_eq!(
        parents.iter().map(|part| part.len() + 1).sum::<usize>() + exact.len(),
        CAR_LOGICAL_PATH_MAX_BYTES
    );
    validate_component(&exact, &parents, native).unwrap();
    assert!(matches!(
        validate_component(&(exact + "n"), &parents, native),
        Err(CarPlanError::InvalidPath(_))
    ));
    validate_component(
        "file",
        &vec!["d".into(); CAR_LOGICAL_PATH_MAX_COMPONENTS - 1],
        native,
    )
    .unwrap();
    assert!(matches!(
        validate_component(
            "file",
            &vec!["d".into(); CAR_LOGICAL_PATH_MAX_COMPONENTS],
            native
        ),
        Err(CarPlanError::InvalidPath(_))
    ));
    for name in ["", ".", "..", "a/b", "a\\b"] {
        assert!(matches!(
            validate_component(name, &[], native),
            Err(CarPlanError::InvalidPath(_))
        ));
    }
}

// Same original-handle sparse setup as the maintained sorafs-node fixture. Mark
// before set_len: ordinary native NTFS extension allocates the full physical size.
#[allow(unsafe_code)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn DeviceIoControl(
        handle: *mut c_void,
        code: u32,
        input: *mut c_void,
        input_size: u32,
        output: *mut c_void,
        output_size: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn GetFileInformationByHandleEx(
        handle: *mut c_void,
        class: i32,
        buffer: *mut c_void,
        size: u32,
    ) -> i32;
}

fn mark_sparse_and_measure(file: &File) -> u64 {
    let mut returned = 0;
    // SAFETY: synchronous no-payload control borrows the live original File.
    #[allow(unsafe_code)]
    let marked = unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            0x0009_00c4,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            &raw mut returned,
            std::ptr::null_mut(),
        )
    };
    assert_ne!(marked, 0, "mark sparse: {}", io::Error::last_os_error());
    file.set_len(CAR_EAGER_DIRECTORY_MAX_BYTES + 1).unwrap();
    #[repr(C)]
    #[derive(Default)]
    struct StandardInfo {
        allocation: i64,
        end_of_file: i64,
        links: u32,
        delete_pending: u8,
        directory: u8,
    }
    let mut info = StandardInfo::default();
    // SAFETY: initialized native FILE_STANDARD_INFO buffer has the exact C layout;
    // the borrowed original stays live throughout this synchronous query.
    #[allow(unsafe_code)]
    let observed = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            1,
            (&raw mut info).cast(),
            u32::try_from(size_of::<StandardInfo>()).unwrap(),
        )
    };
    assert_ne!(observed, 0, "query sparse: {}", io::Error::last_os_error());
    assert_eq!(
        u64::try_from(info.end_of_file).unwrap(),
        CAR_EAGER_DIRECTORY_MAX_BYTES + 1
    );
    u64::try_from(info.allocation).unwrap()
}

#[test]
fn native_directory_plan_refuses_sparse_oversize_before_payload_read() {
    let (_temporary, root) = fixture(false);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(root.join("sparse"))
        .unwrap();
    assert_eq!(
        mark_sparse_and_measure(&file),
        0,
        "no physical NTFS data extent"
    );
    let identity = iroha_fs::FileIdentity::of(&file).unwrap();
    drop(file);
    let mut opened = false;
    let result = gather_files_secure(&root, &mut |_| {
        opened = true;
        Ok(())
    });
    assert!(
        matches!(result, Err(CarPlanError::DirectoryPayloadTooLarge { bytes, maximum }) if bytes == CAR_EAGER_DIRECTORY_MAX_BYTES + 1 && maximum == CAR_EAGER_DIRECTORY_MAX_BYTES)
    );
    assert!(
        !opened,
        "payload limit must reject before the payload-read hook"
    );
    // The hook is not an allocation observer. Payload allocation ordering is
    // separately checked in the production scanner; directory setup may allocate.
    let original = File::open(root.join("sparse")).unwrap();
    assert_eq!(iroha_fs::FileIdentity::of(&original).unwrap(), identity);
    assert_eq!(
        original.metadata().unwrap().len(),
        CAR_EAGER_DIRECTORY_MAX_BYTES + 1
    );
}

#[test]
fn native_directory_plan_validates_profile_before_missing_root() {
    let temporary = tempfile::tempdir().unwrap();
    let missing = temporary.path().join("missing");
    let invalid = ChunkProfile {
        min_size: 2,
        target_size: 1,
        max_size: 4,
        break_mask: 1,
    };
    assert!(matches!(
        CarBuildPlan::from_directory_with_profile(&missing, invalid),
        Err(CarPlanError::Chunking(
            sorafs_chunker::ChunkerError::TargetBeforeMin { .. }
        ))
    ));
    assert!(!missing.exists());
}

#[test]
fn native_directory_plan_empty_directories_and_empty_files_keep_distinct_semantics() {
    let (_temporary, root) = fixture(false);
    assert!(matches!(
        CarBuildPlan::from_directory(&root),
        Err(CarPlanError::EmptyInput)
    ));
    fs::create_dir(root.join("empty-directory")).unwrap();
    assert!(matches!(
        CarBuildPlan::from_directory(&root),
        Err(CarPlanError::EmptyInput)
    ));
    write_file(&root, &["empty-file"], b"");
    let entries = vec![FileEntry {
        path: vec!["empty-file".into()],
        data: Vec::new(),
    }];
    matches_explicit(&root, entries, ChunkProfile::DEFAULT);
    let (plan, payload) = CarBuildPlan::from_directory(&root).unwrap();
    assert!(payload.is_empty());
    assert_eq!(plan.files.len(), 1);
    assert_eq!(plan.files[0].size, 0);
    assert!(plan.chunks.is_empty());
}

#[test]
fn native_directory_plan_refuses_unc_and_device_roots_before_traversal() {
    for root in [r"\\unreachable.invalid\share\payload", r"\\.\C:\payload"] {
        let mut opened = false;
        let result = gather_files_secure(Path::new(root), &mut |_| {
            opened = true;
            Ok(())
        });
        assert!(
            matches!(result, Err(CarPlanError::DirectoryIo { source, .. })
            if source.kind() == io::ErrorKind::InvalidInput)
        );
        assert!(!opened);
    }
}
