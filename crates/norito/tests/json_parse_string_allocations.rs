//! Allocation and escape-parity checks for owned JSON string parsing.
#![cfg(feature = "json")]
use norito::json::Parser;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering},
};
struct CountingAllocator;
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static TRACK_CURRENT_THREAD: Cell<bool> = const { Cell::new(false) };
}
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        TRACK_CURRENT_THREAD.with(|tracking| {
            if tracking.get() {
                ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator;
fn measured_allocations<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    TRACK_CURRENT_THREAD.with(|tracking| tracking.set(true));
    let result = operation();
    TRACK_CURRENT_THREAD.with(|tracking| tracking.set(false));
    (result, ALLOCATIONS.load(Ordering::Relaxed))
}
#[test]
fn escaped_string_reuses_its_unescape_buffer() {
    const SOURCE: &str = r#""a\nb\tc\"d""#;
    let (parsed, allocations) = measured_allocations(|| {
        Parser::new(SOURCE)
            .parse_string()
            .expect("parse escaped string")
    });
    assert_eq!(parsed, "a\nb\tc\"d");
    assert_eq!(
        allocations, 1,
        "the returned String must take ownership of the single unescape buffer"
    );
}
#[test]
fn escaped_string_conversion_preserves_all_escape_forms() {
    let cases = [
        (r#""quote: \"""#, "quote: \""),
        (r#""slash: \\""#, "slash: \\"),
        (r#""solidus: \/""#, "solidus: /"),
        (r#""controls: \b\f\n\r\t""#, "controls: \u{8}\u{c}\n\r\t"),
        (r#""bmp: \u263a""#, "bmp: ☺"),
        (r#""pair: \ud83d\ude00""#, "pair: 😀"),
    ];
    for (source, expected) in cases {
        let actual = Parser::new(source)
            .parse_string()
            .expect("parse JSON escape form");
        assert_eq!(actual, expected, "escape mismatch for {source:?}");
    }
}
