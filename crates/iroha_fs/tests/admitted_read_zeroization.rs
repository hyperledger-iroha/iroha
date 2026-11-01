//! Observe the original admitted allocation immediately before its real deallocation.
//!
//! The meter inspects only a registered, initialized byte extent while System still owns
//! the live allocation. It neither reads freed memory nor retains a plaintext copy.

use iroha_fs::{PrivateDirectory, PublishMode};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    io, ptr,
    sync::atomic::{AtomicPtr, AtomicU8, AtomicUsize, Ordering},
};
use zeroize::{Zeroize as _, Zeroizing};

static WATCHED: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());
static LENGTH: AtomicUsize = AtomicUsize::new(0);
static TRACKED: AtomicUsize = AtomicUsize::new(0);
static OBSERVED: AtomicU8 = AtomicU8::new(0);
const CLEARED: u8 = 1;
const UNCLEARED: u8 = 2;
const REALLOCATED: u8 = 3;
const INVALID_EXTENT: u8 = 4;

struct Observer;

// SAFETY: Every operation forwards unchanged allocator arguments to System. Only dealloc
// inspects a registered initialized extent, before releasing its still-live allocation.
#[allow(
    unsafe_code,
    reason = "test observes cleanup before the real allocation is freed"
)]
unsafe impl GlobalAlloc for Observer {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: preserve System's allocation contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: preserve System's zeroed allocation contract.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if WATCHED
            .compare_exchange(
                pointer,
                ptr::null_mut(),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            OBSERVED.store(REALLOCATED, Ordering::Release);
        }
        // SAFETY: forward the original live allocation and requested new size.
        unsafe { System.realloc(pointer, layout, size) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if WATCHED
            .compare_exchange(
                pointer,
                ptr::null_mut(),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            let length = LENGTH.load(Ordering::Acquire);
            let observed = if length > layout.size() {
                INVALID_EXTENT
            } else {
                // SAFETY: track registers this original Vec's initialized length; its
                // allocation is still live and at least that long until System.dealloc.
                let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
                if bytes.iter().all(|byte| *byte == 0) {
                    CLEARED
                } else {
                    UNCLEARED
                }
            };
            OBSERVED.store(observed, Ordering::Release);
        }
        // SAFETY: release the same original live allocation exactly once.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Observer = Observer;

fn begin_case() {
    assert!(WATCHED.load(Ordering::Acquire).is_null());
    OBSERVED.store(0, Ordering::Release);
    TRACKED.store(0, Ordering::Release);
}

fn track(mut bytes: Vec<u8>) -> Vec<u8> {
    assert!(!bytes.is_empty());
    assert!(WATCHED.load(Ordering::Acquire).is_null());
    assert_eq!(OBSERVED.load(Ordering::Acquire), 0);
    assert_eq!(TRACKED.fetch_add(1, Ordering::AcqRel), 0);
    LENGTH.store(bytes.len(), Ordering::Release);
    WATCHED.store(bytes.as_mut_ptr(), Ordering::Release);
    bytes
}

fn assert_dropped(expected: u8) {
    assert_eq!(TRACKED.load(Ordering::Acquire), 1);
    assert!(WATCHED.load(Ordering::Acquire).is_null());
    assert_eq!(OBSERVED.load(Ordering::Acquire), expected);
}

fn fixture() -> (tempfile::TempDir, PrivateDirectory) {
    let temporary = tempfile::tempdir().unwrap();
    let directory = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
    directory
        .write_atomic("record", b"original plaintext", PublishMode::CreateNew)
        .unwrap();
    (temporary, directory)
}

fn admit(extent: u64) -> io::Result<usize> {
    usize::try_from(extent).map_err(io::Error::other)
}

fn changed(named: bool) -> io::Error {
    io::Error::other(if named {
        "named original changed"
    } else {
        "read original changed"
    })
}

#[cfg(unix)]
fn private_policy(directory: &PrivateDirectory, private: bool) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(
        directory.path(),
        std::fs::Permissions::from_mode(if private { 0o700 } else { 0o755 }),
    )
}

#[cfg(windows)]
fn private_policy(directory: &PrivateDirectory, private: bool) -> io::Result<()> {
    let mut command = std::process::Command::new("icacls");
    command.arg(directory.path());
    if private {
        command.args(["/remove:g", "*S-1-1-0"]);
    } else {
        command.args(["/grant", "*S-1-1-0:(R)"]);
    }
    let output = command.output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "native test ACL operation failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

#[test]
fn admitted_buffer_policy_preserves_zeroization_through_native_and_outer_errors() {
    for boundary in [
        "allocation",
        "partial-read",
        "eof",
        "consumer",
        "snapshot",
        "outer",
    ] {
        begin_case();
        let (_temporary, directory) = fixture();
        let reached = std::cell::Cell::new(false);
        let outer_error_kind = std::cell::Cell::new(None);
        let result = directory.read_scope(|reader| {
            let body = reader.read_admitted_zeroizing(
                "record",
                64,
                (admit, |length| {
                    if boundary == "partial-read" {
                        directory.open_existing_lock("record")?.set_len(4)?;
                    } else if boundary == "eof" {
                        directory
                            .open_existing_lock("record")?
                            .set_len(u64::try_from(length).unwrap() + 1)?;
                    }
                    let length = if boundary == "allocation" {
                        length - 1
                    } else {
                        length
                    };
                    if matches!(boundary, "allocation" | "partial-read" | "eof") {
                        reached.set(true);
                    }
                    Ok(Zeroizing::new(track(vec![0xa5; length])))
                }),
                |_| {
                    if boundary == "consumer" {
                        reached.set(true);
                        return Err(io::Error::new(io::ErrorKind::NotFound, "consumer refusal"));
                    }
                    if boundary == "snapshot" {
                        directory.open_existing_lock("record")?.set_len(1)?;
                        reached.set(true);
                    }
                    Ok(())
                },
                (|error| error, changed),
            );
            if boundary == "outer" {
                assert_eq!(
                    body.as_ref().unwrap().as_ref().unwrap().as_slice(),
                    b"original plaintext"
                );
                private_policy(&directory, false).expect("apply real private-policy violation");
                outer_error_kind.set(Some(
                    directory.revalidate().expect_err("policy changed").kind(),
                ));
                reached.set(true);
            }
            body
        });
        assert!(reached.get(), "{boundary} injection must actually complete");
        let error = result.expect_err("injected boundary must refuse");
        match boundary {
            "allocation" => assert_eq!(
                error.to_string(),
                "record allocation differs from its admitted length"
            ),
            "partial-read" => assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof),
            "eof" => assert_eq!(error.to_string(), "read original changed"),
            "consumer" => {
                assert_eq!(error.kind(), io::ErrorKind::NotFound);
                assert_eq!(error.to_string(), "consumer refusal");
            }
            "snapshot" => assert_eq!(error.to_string(), "named original changed"),
            "outer" => assert_eq!(Some(error.kind()), outer_error_kind.get()),
            _ => unreachable!(),
        }
        // Exact real allocation, before deallocation: partially read, unread tail,
        // complete but rejected record, and outer-fence result all contain zeros.
        assert_dropped(CLEARED);
        if boundary == "outer" {
            private_policy(&directory, true).unwrap();
            directory.revalidate().unwrap();
        }
    }

    #[cfg(unix)]
    for substitute in [false, true] {
        begin_case();
        let reached = std::cell::Cell::new(false);
        let (_temporary, directory) = fixture();
        let result = directory.read_scope(|reader| {
            reader.read_admitted_zeroizing(
                "record",
                64,
                (admit, |length| {
                    Ok(Zeroizing::new(track(vec![0xa5; length])))
                }),
                |_| {
                    std::fs::rename(
                        directory.path().join("record"),
                        directory.path().join("saved"),
                    )?;
                    if substitute {
                        directory.write_atomic(
                            "record",
                            b"original plaintext",
                            PublishMode::CreateNew,
                        )?;
                    }
                    reached.set(true);
                    Ok(())
                },
                (|error| error, changed),
            )
        });
        assert!(reached.get(), "native namespace injection must complete");
        let error = result.expect_err("late native name check must refuse");
        if substitute {
            assert_eq!(error.to_string(), "named original changed");
        } else {
            assert_eq!(error.kind(), io::ErrorKind::NotFound);
        }
        assert_dropped(CLEARED);
    }

    // Successful transfer keeps one original allocation through the outer fence.
    begin_case();
    let (_temporary, directory) = fixture();
    let selected = std::cell::Cell::new(ptr::null());
    let mut bytes = directory
        .read_scope(|reader| {
            reader.read_admitted_zeroizing(
                "record",
                64,
                (admit, |length| {
                    let bytes = track(vec![0xa5; length]);
                    selected.set(bytes.as_ptr());
                    Ok(Zeroizing::new(bytes))
                }),
                |_| Ok(()),
                (|error| error, changed),
            )
        })
        .unwrap()
        .unwrap();
    assert_eq!(bytes.as_ptr(), selected.get());
    let mut output = std::mem::take(&mut *bytes);
    drop(bytes);
    assert_eq!(output.as_ptr(), selected.get());
    assert_eq!(output, b"original plaintext");
    assert_eq!(OBSERVED.load(Ordering::Acquire), 0);
    output.zeroize();
    drop(output);
    assert_dropped(CLEARED);

    // Positive meter control and unchanged plain-Vec policy: complete rejected bytes
    // from the existing API are not silently assigned a new clearing policy.
    begin_case();
    let result = directory.read_scope(|reader| {
        reader.read_admitted(
            "record",
            64,
            (admit, |length| Ok(track(vec![0xa5; length]))),
            |_| Err(io::Error::other("plain owner refusal")),
            (|error| error, changed),
        )
    });
    assert_eq!(result.unwrap_err().to_string(), "plain owner refusal");
    assert_dropped(UNCLEARED);
}
