#[test]
fn stable_bounded_sidecar_read_rejects_post_admission_growth() {
    let root = tempfile::tempdir().expect("create bounded-read root");
    let path = root.path().join("bounded.norito");
    std::fs::write(&path, [0_u8; 8]).expect("write admitted sidecar");
    let error = super::Kura::read_regular_sidecar_snapshot_for_with_admission_hook(
        root.path(),
        &path,
        root.path(),
        8,
        || {
            use std::io::Write as _;
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("open admitted sidecar for growth")
                .write_all(&[1])
                .expect("grow admitted sidecar by one byte");
        },
    )
    .expect_err("post-admission growth must invalidate the bounded read");
    assert!(matches!(
        error,
        super::Error::IO(ref source, _) if source.kind() == std::io::ErrorKind::InvalidData
    ));
    assert_eq!(
        std::fs::metadata(&path)
            .expect("inspect grown sidecar")
            .len(),
        9
    );
}

#[test]
fn stable_bounded_sidecar_read_allows_sibling_publication() {
    let root = tempfile::tempdir().expect("create bounded-read root");
    let path = root.path().join("bounded.norito");
    let bytes = [7_u8; 8];
    std::fs::write(&path, bytes).expect("write admitted sidecar");
    let sibling = root.path().join("concurrent-sibling.norito");
    let snapshot = super::Kura::read_regular_sidecar_snapshot_for_with_admission_hook(
        root.path(),
        &path,
        root.path(),
        bytes.len(),
        || std::fs::write(&sibling, [9_u8]).expect("publish sibling sidecar"),
    )
    .expect("sibling publication must not invalidate the bounded file read")
    .expect("admitted sidecar remains present");
    assert_eq!(snapshot.bytes, bytes);
    assert_eq!(std::fs::read(sibling).expect("read sibling sidecar"), [9]);
}
