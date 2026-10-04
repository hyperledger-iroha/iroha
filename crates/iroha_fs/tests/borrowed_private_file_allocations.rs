//! Observe Rust heap requests for borrowed retained-file custody on Unix.
//!
//! This meter cannot see native ACL allocation, kernel memory or preexisting directory
//! lineage. It establishes only that the successful borrowed operations below do not
//! request Rust heap storage for retained directory/name copies or other Rust scratch.

#![cfg(unix)]

use iroha_fs::PrivateDirectory;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    ffi::OsStr,
    io::{self, Read as _, Seek as _, SeekFrom, Write as _},
};

thread_local! {
    static REQUESTS: Cell<Option<usize>> = const { Cell::new(None) };
}

struct Observer;

fn record_request() {
    let _ = REQUESTS.try_with(|requests| {
        if let Some(count) = requests.get() {
            requests.set(Some(count + 1));
        }
    });
}

// SAFETY: Every allocator operation forwards the original arguments to System; the
// thread-local counter only observes calls and never owns or alters their allocations.
#[allow(
    unsafe_code,
    reason = "isolated test must observe Rust allocator requests"
)]
unsafe impl GlobalAlloc for Observer {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_request();
        // SAFETY: forwards the caller's unchanged allocation contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_request();
        // SAFETY: forwards the caller's unchanged allocation contract.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_request();
        // SAFETY: forwards the original live allocation and requested new size.
        unsafe { System.realloc(pointer, layout, size) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: returns the original live allocation to its allocator.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Observer = Observer;

struct Observation;

impl Drop for Observation {
    fn drop(&mut self) {
        REQUESTS.with(|requests| requests.set(None));
    }
}

fn observe<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    let previous = REQUESTS.with(|requests| requests.replace(Some(0)));
    assert!(previous.is_none(), "allocation observations cannot nest");
    let scope = Observation;
    let result = operation();
    let requests = REQUESTS.with(|requests| requests.get().unwrap());
    drop(scope);
    (result, requests)
}

fn without_rust_allocations<T>(name: &str, operation: impl FnOnce() -> io::Result<T>) -> T {
    let (result, requests) = observe(operation);
    assert_eq!(requests, 0, "{name} requested Rust heap storage");
    result.unwrap()
}

#[test]
fn borrowed_file_lifecycle_does_not_clone_lineage_names_or_request_rust_scratch() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = PrivateDirectory::open_or_create(temporary.path().join("receipts")).unwrap();
    // Warm the native validation path before observing the Rust allocator. The original
    // directory owner and these static basenames already exist outside the measured work.
    directory.revalidate().unwrap();
    let staged = OsStr::new("staged");
    let published_name = OsStr::new("receipt");
    let mut writer =
        without_rust_allocations("create", || directory.create_borrowed_private(staged, 64));
    without_rust_allocations("write", || writer.write_all(b"original"));
    without_rust_allocations("flush", || writer.flush());
    let position = without_rust_allocations("writer seek", || writer.seek(SeekFrom::Start(0)));
    assert_eq!(position, 0);
    let mut sealed = without_rust_allocations("seal", || writer.seal_read_only());
    let identity = without_rust_allocations("identity", || sealed.identity());
    let snapshot = without_rust_allocations("snapshot", || sealed.snapshot());
    let length = without_rust_allocations("length", || sealed.len());
    assert_eq!(length, 8);
    assert!(!without_rust_allocations("empty check", || sealed.is_empty()));
    let mut bytes = [0; 8];
    without_rust_allocations("read", || sealed.read_exact(&mut bytes));
    assert_eq!(&bytes, b"original");
    assert_eq!(
        without_rust_allocations("stable snapshot", || sealed.snapshot()),
        snapshot
    );
    without_rust_allocations("revalidate", || sealed.revalidate());
    let published = without_rust_allocations("publish", || sealed.publish_new_name(published_name));
    assert_eq!(
        without_rust_allocations("published identity", || published.identity()),
        identity
    );
    let mut reopened = without_rust_allocations("reopen", || {
        directory.open_borrowed_read_only(published_name, 64)
    });
    let position = without_rust_allocations("reader seek", || reopened.seek(SeekFrom::End(-8)));
    assert_eq!(position, 0);
    without_rust_allocations("reopened read", || reopened.read_exact(&mut bytes));
    assert_eq!(&bytes, b"original");
    without_rust_allocations("reopened revalidate", || reopened.revalidate());
    let ((), requests) = observe(|| {
        drop(reopened);
        drop(published);
    });
    assert_eq!(requests, 0);
}

#[test]
fn allocator_observer_detects_owned_directory_and_basename_copies() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = PrivateDirectory::open_or_create(temporary.path().join("receipts")).unwrap();
    directory.revalidate().unwrap();
    // A positive control ensures the meter sees the actual crate's allocations, not only
    // allocations in this integration binary. Owned custody retains both Vec and OsString.
    let (writer, requests) = observe(|| directory.create_retained_private("owned", 64));
    let writer = writer.unwrap();
    assert!(
        requests >= 2,
        "owned lineage and name copies must be observed"
    );
    drop(writer);
}
