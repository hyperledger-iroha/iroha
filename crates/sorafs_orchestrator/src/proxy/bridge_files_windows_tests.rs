//! Actual Windows custody, bounded streaming and cancelled blocking-read controls.

use std::{
    fs, future::Future as _, io::Write as _, process::Command, sync::mpsc, task::Poll,
    time::Duration,
};

use tokio::io::AsyncReadExt as _;

use super::*;

fn fixture() -> (tempfile::TempDir, PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("bridge");
    drop(iroha_fs::PrivateDirectory::open_or_create(&root).unwrap());
    (temporary, root)
}

fn payload(root: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = root.join(name);
    let mut writer = RetainedFile::create_new_private(&path).unwrap();
    writer.file_mut().write_all(bytes).unwrap();
    writer.file().sync_all().unwrap();
    drop(writer);
    path
}

fn grant(path: &Path, permission: &str) {
    let result = Command::new("icacls.exe")
        .arg(path)
        .args(["/grant", permission])
        .output()
        .unwrap();
    assert!(result.status.success(), "native ACL fixture failed");
}

#[tokio::test]
async fn native_bridge_streams_multiple_bounded_chunks_and_empty_files() {
    let (_temporary, root) = fixture();
    let bytes: Vec<u8> = (0..COPY_CHUNK_BYTES * 2 + 17)
        .map(|index| (index % 251) as u8)
        .collect();
    let path = payload(&root, "payload", &bytes);
    grant(&path, "*S-1-1-0:(R)");
    let canonical = canonical_bridge_root(&root).unwrap();
    for (name, expected) in [("payload", bytes.as_slice()), ("empty", &[][..])] {
        if name == "empty" {
            payload(&root, name, expected);
        }
        let opened = open_bridge_file(&canonical, canonical.join(name), expected.len() as u64)
            .await
            .unwrap();
        let (mut send, mut receive) = tokio::io::duplex(1031);
        let (copied, read) = tokio::join!(
            async {
                let copied = copy_bridge_file(&opened, &mut send).await;
                drop(send);
                copied
            },
            async {
                let mut actual = Vec::new();
                receive.read_to_end(&mut actual).await.unwrap();
                actual
            }
        );
        assert_eq!(copied.unwrap(), expected.len() as u64);
        assert_eq!(read, expected);
        verify_bridge_file_unchanged(&opened).await.unwrap();
    }
}

#[tokio::test]
async fn native_bridge_refuses_links_writers_nonfiles_and_only_initial_absence_is_missing() {
    let (temporary, root) = fixture();
    let path = payload(&root, "payload", b"original");
    let alias = root.join("alias");
    fs::hard_link(&path, &alias).unwrap();
    assert!(matches!(
        open_bridge_file(&root, path.clone(), 8).await,
        Err(BridgeFileError::Unsafe)
    ));
    fs::remove_file(alias).unwrap();
    let writer = fs::OpenOptions::new().write(true).open(&path).unwrap();
    assert!(matches!(
        open_bridge_file(&root, path.clone(), 8).await,
        Err(BridgeFileError::Unsafe)
    ));
    drop(writer);
    let opened = open_bridge_file(&root, path.clone(), 8).await.unwrap();
    assert!(fs::remove_file(&path).is_err());
    assert!(fs::rename(&root, temporary.path().join("moved")).is_err());
    verify_bridge_file_unchanged(&opened).await.unwrap();
    drop(opened);
    fs::remove_file(&path).unwrap();
    assert!(matches!(
        open_bridge_file(&root, path, 8).await,
        Err(BridgeFileError::NotFound)
    ));
    assert!(matches!(
        open_bridge_file(&root, root.join("missing/leaf"), 8).await,
        Err(BridgeFileError::NotFound)
    ));
    fs::create_dir(root.join("directory")).unwrap();
    assert!(matches!(
        open_bridge_file(&root, root.join("directory"), 8).await,
        Err(BridgeFileError::Unsafe)
    ));
}

#[tokio::test]
async fn native_bridge_rejects_junctions_traversal_and_nonlocal_roots() {
    let (temporary, root) = fixture();
    payload(&root, "payload", b"original");
    for path in [
        root.clone(),
        root.join("../bridge/payload"),
        temporary.path().join("elsewhere"),
    ] {
        assert!(matches!(
            open_bridge_file(&root, path, 8).await,
            Err(BridgeFileError::Unsafe)
        ));
    }
    for root in [
        r"\\server\share\bridge",
        r"\\?\UNC\server\share\bridge",
        r"\\.\C:\bridge",
    ] {
        assert!(canonical_bridge_root(Path::new(root)).is_err());
    }
    let junction = temporary.path().join("junction");
    let result = Command::new("cmd.exe")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&root)
        .output()
        .unwrap();
    assert!(result.status.success(), "native junction fixture failed");
    assert!(canonical_bridge_root(&junction).is_err());
    assert!(matches!(
        open_bridge_file(&junction, junction.join("payload"), 8).await,
        Err(BridgeFileError::Unsafe)
    ));
    fs::remove_dir(junction).unwrap();
}

#[tokio::test]
async fn native_bridge_detects_late_unsafe_acl_without_repair() {
    let (_temporary, root) = fixture();
    let path = payload(&root, "payload", b"original");
    let opened = open_bridge_file(&root, path.clone(), 8).await.unwrap();
    grant(&path, "*S-1-1-0:(W)");
    assert!(matches!(
        verify_bridge_file_unchanged(&opened).await,
        Err(BridgeFileError::Changed)
    ));
    drop(opened);
    assert!(matches!(
        open_bridge_file(&root, path, 8).await,
        Err(BridgeFileError::Unsafe)
    ));
    grant(&root, "*S-1-1-0:(W)");
    assert!(canonical_bridge_root(&root).is_err());
}

#[tokio::test]
async fn native_bridge_preserves_read_and_writer_errors() {
    let (_temporary, root) = fixture();
    let path = payload(&root, "payload", b"original");
    let opened = open_bridge_file(&root, path, 8).await.unwrap();
    assert_eq!(
        read_chunk(Arc::clone(&opened.original), 2, 3)
            .await
            .unwrap(),
        b"igi"
    );
    assert!(matches!(
        read_chunk(Arc::clone(&opened.original), 7, 2).await,
        Err(BridgeFileError::Truncated)
    ));
    let (mut send, receive) = tokio::io::duplex(1);
    drop(receive);
    assert!(matches!(
        copy_bridge_file(&opened, &mut send).await,
        Err(BridgeFileError::Io)
    ));
    verify_bridge_file_unchanged(&opened).await.unwrap();
}

#[test]
fn native_bridge_cancelled_queued_read_retains_original_until_disk_work_ends() {
    let (temporary, root) = fixture();
    let path = payload(&root, "payload", b"original");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let opened = open_bridge_file(&root, path.clone(), 8).await.unwrap();
        let weak = Arc::downgrade(&opened.original);
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        });
        started_rx.await.unwrap();
        let mut read = Box::pin(read_chunk(Arc::clone(&opened.original), 0, 8));
        std::future::poll_fn(|context| {
            assert!(read.as_mut().poll(context).is_pending());
            Poll::Ready(())
        })
        .await;
        // Drop the actual read future while its already-submitted disk job remains queued.
        drop(read);
        drop(opened);
        assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
        assert!(fs::rename(&root, temporary.path().join("moved")).is_err());
        release_tx.send(()).unwrap();
        blocker.await.unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            while weak.upgrade().is_some() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("detached disk job releases its own original");
        fs::write(&path, b"released").unwrap();
        fs::rename(&root, temporary.path().join("moved")).unwrap();
    });
}
