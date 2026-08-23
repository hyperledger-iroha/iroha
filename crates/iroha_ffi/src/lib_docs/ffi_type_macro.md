Macro for defining FFI types of a known category ([`ir::Robust`] or [`ir::Transmute`]).

The implementation for an FFI type of one of the categories incurs a lot of bloat that
is reduced by the use of this macro

# Safety

* If the type is [`ir::Robust`], it derives [`ReprC`]. Check safety invariants for [`ReprC`]
* If the type is [`ir::Transparent`], it derives [`ir::Transmute`. Check safety invariants for [`ir::Transmute`]

# Example

```
use iroha_ffi::{ReprC, ffi_type};

// Always use a type alias for inner types of transparent items so that if you make
// a change the unsafe code in [`iroha_ffi::ffi_type!`] will not compile, thus preventing UB
type NonNullInner<T> = *mut T;
type WrapperInner = u32;

#[repr(transparent)]
struct NonNull<T>(NonNullInner<T>);

#[repr(transparent)]
struct Wrapper(WrapperInner);

#[derive(Clone, Copy)]
#[repr(C)]
struct RobustStruct(u64, i32);

// SAFETY: Type is robust #[repr(C)]
unsafe impl ReprC for RobustStruct {}
iroha_ffi::ffi_type! { impl Robust for RobustStruct {} }

iroha_ffi::ffi_type! {
    unsafe impl<T> Transparent for NonNull<T> {
        type Target = NonNullInner<T>;

        validation_fn=unsafe {|target: &NonNullInner<T>| !target.is_null()},
        niche_value=core::ptr::null_mut(),
    }
}

// Validation function is `|_| true` implicitly indicating
// this type is robust with respect to the wrapped type
iroha_ffi::ffi_type! {
    unsafe impl Transparent for Wrapper {
        type Target = WrapperInner;
    }
}
```
