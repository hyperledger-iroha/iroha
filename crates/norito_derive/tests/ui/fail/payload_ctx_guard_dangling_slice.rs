//! A leaked or out-of-order payload guard could keep a context installed after
//! its buffer died, and the raw-pointer accessor handed out `&'static` bytes.
//! The guard is crate-private and the pointer accessor is gone, so this former
//! dangling-slice program no longer compiles.
#![forbid(unsafe_code)]
fn main() {
    let dangling: &'static [u8] = {
        let buffer = vec![1_u8, 2, 3];
        let _context = norito::core::PayloadCtxGuard::enter(&buffer);
        norito::core::payload_slice_from_ptr(buffer.as_ptr()).unwrap()
    };
    assert_eq!(dangling, [1, 2, 3]);
}
