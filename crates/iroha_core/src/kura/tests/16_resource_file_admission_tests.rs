// Actual opened-descriptor flags and post-admission FIFO replacement controls.

#[cfg(unix)]
fn resource_file_fifo_with_live_peer(path: &Path) -> std::fs::File {
    let status = std::process::Command::new("mkfifo")
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
    // The peer prevents even a regression to blocking open from stranding this
    // test. Positive controls independently inspect NONBLOCK on the real observer FD.
    std::fs::File::from(
        rustix::fs::open(
            path,
            rustix::fs::OFlags::RDWR | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )
        .unwrap(),
    )
}

#[cfg(unix)]
fn resource_file_assert_nonblocking_descriptor(file: &std::fs::File) {
    assert!(
        rustix::fs::fcntl_getfl(file)
            .unwrap()
            .contains(rustix::fs::OFlags::NONBLOCK)
    );
}

#[cfg(unix)]
#[test]
fn storage_resource_observer_opens_actual_regular_descriptor_nonblocking() {
    let directory = TempDir::new().unwrap();
    let path = fs::canonicalize(directory.path()).unwrap().join("data");
    fs::write(&path, [0x51_u8; 37]).unwrap();
    let admitted = std::cell::Cell::new(false);
    let opened = std::cell::Cell::new(false);
    let actual = storage_resource_file_bytes_with_admission_hooks(
        &path,
        || admitted.set(true),
        |file| {
            opened.set(true);
            resource_file_assert_nonblocking_descriptor(file);
        },
    )
    .unwrap();
    assert!(admitted.get() && opened.get());
    assert_eq!(actual, 37);
    assert_eq!(storage_resource_file_bytes(&path).unwrap(), actual);
}

#[cfg(unix)]
#[test]
fn index_and_evidence_resource_observers_keep_exact_formats_on_nonblocking_descriptors() {
    let directory = TempDir::new().unwrap();
    let path = fs::canonicalize(directory.path()).unwrap().join("index");
    for (format, temporary, bytes, expected_entries) in [
        (IndexResourceFormat::Fixed(16), false, vec![0_u8; 32], 2),
        (IndexResourceFormat::Singleton(64), false, vec![0_u8; 23], 1),
        (IndexResourceFormat::TemporarySingleton(64), true, vec![], 1),
        (
            IndexResourceFormat::SidecarV1,
            false,
            SidecarIndexLayout::base_header(7).to_vec(),
            0,
        ),
    ] {
        fs::write(&path, &bytes).unwrap();
        let opened = std::cell::Cell::new(false);
        let actual = index_resource_file_usage_with_admission_hooks(
            &path,
            format,
            temporary,
            || {},
            |file| {
                opened.set(true);
                resource_file_assert_nonblocking_descriptor(file);
            },
        )
        .unwrap();
        assert!(opened.get());
        assert_eq!(actual.persisted_entries, expected_entries);
        assert_eq!(
            actual.index_bytes + actual.temporary_index_bytes,
            bytes.len() as u64
        );
        assert_eq!(
            index_resource_file_usage(&path, format, temporary).unwrap(),
            actual
        );
    }
}

#[cfg(unix)]
#[test]
fn storage_resource_observer_rejects_fifo_replacement_after_regular_admission() {
    let directory = TempDir::new().unwrap();
    let path = fs::canonicalize(directory.path()).unwrap().join("data");
    fs::write(&path, [0x52_u8; 37]).unwrap();
    assert_eq!(storage_resource_file_bytes(&path).unwrap(), 37);
    let peer = std::cell::RefCell::new(None);
    let admitted = std::cell::Cell::new(false);
    let result = storage_resource_file_bytes_with_admission_hooks(
        &path,
        || {
            admitted.set(true);
            fs::remove_file(&path).unwrap();
            *peer.borrow_mut() = Some(resource_file_fifo_with_live_peer(&path));
        },
        resource_file_assert_nonblocking_descriptor,
    );
    assert!(admitted.get());
    assert!(result.is_err());
}

#[cfg(unix)]
#[test]
fn index_and_evidence_resource_observers_reject_fifo_before_reading_admitted_formats() {
    for (format, bytes) in [
        (IndexResourceFormat::Fixed(16), vec![0_u8; 32]),
        (IndexResourceFormat::Singleton(64), vec![0_u8; 23]),
        (
            IndexResourceFormat::SidecarV1,
            SidecarIndexLayout::base_header(7).to_vec(),
        ),
    ] {
        let directory = TempDir::new().unwrap();
        let path = fs::canonicalize(directory.path()).unwrap().join("index");
        fs::write(&path, &bytes).unwrap();
        assert!(index_resource_file_usage(&path, format, false).is_ok());
        let peer = std::cell::RefCell::new(None);
        let admitted = std::cell::Cell::new(false);
        let result = index_resource_file_usage_with_admission_hooks(
            &path,
            format,
            false,
            || {
                admitted.set(true);
                fs::remove_file(&path).unwrap();
                *peer.borrow_mut() = Some(resource_file_fifo_with_live_peer(&path));
            },
            resource_file_assert_nonblocking_descriptor,
        );
        assert!(admitted.get());
        assert!(result.is_err());
    }
}
