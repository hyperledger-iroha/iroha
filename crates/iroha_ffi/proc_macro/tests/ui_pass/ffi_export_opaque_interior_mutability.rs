use std::cell::RefCell;
use iroha_ffi::{FfiType, ffi_export};
#[derive(FfiType)]
#[ffi_type(opaque)]
pub struct OpaqueState {
    value: RefCell<u32>,
}
#[ffi_export]
impl OpaqueState {
    pub fn replace(&self, value: u32) -> u32 {
        self.value.replace(value)
    }
}
fn main() {
    let state = OpaqueState {
        value: RefCell::new(1),
    };
    assert_eq!(state.replace(2), 1);
}
