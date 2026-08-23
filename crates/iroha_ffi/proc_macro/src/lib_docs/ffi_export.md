Generate FFI functions

When placed on a structure, it integrates with `getset` to export derived getter/setter methods.
To be visible this attribute must be placed before/on top of any `getset` derive macro attributes

It also works on impl blocks (by visiting all methods in the impl block) and on enums and unions (as a no-op)

# Example:
```rust
use std::alloc::alloc;

use getset::Getters;

// For a struct such as:
#[iroha_ffi::ffi_export]
#[derive(iroha_ffi::FfiType, Clone, Getters)]
#[getset(get = "pub")]
pub struct Foo {
    /// Id of the struct
    id: u8,
    #[getset(skip)]
    bar: Vec<u8>,
}

#[iroha_ffi::ffi_export]
impl Foo {
    /// Construct new type
    pub fn new(id: u8) -> Self {
        Self {
            id,
            bar: Vec::new(),
        }
    }
    /// Return bar
    pub fn bar(&self) -> &[u8] {
        &self.bar
    }
}

/* The following functions will be derived:
extern "C" fn Foo__new(id: u8, output: *mut Foo) -> FfiReturn {
    /* function implementation */
    FfiReturn::Ok
}
extern "C" fn Foo__bar(handle: *const Foo, output: *mut RefSlice<u8>) -> FfiReturn {
    /* function implementation */
    FfiReturn::Ok
}
extern "C" fn Foo__id(handle: *const Foo, output: *mut u8) -> FfiReturn {
    /* function implementation */
    FfiReturn::Ok
} */
```

## A note on `#[derive(...)]` limitations

This proc-macro crate parses the `#[derive(...)]` attributes.
Due to technical limitations of proc macros, it does not have access to the resolved path of the macro, only to what is written in the derive.
As such, it cannot support derives that are used through aliases, such as

```ignore
use getset::Getters as GettersAlias;
#[derive(GettersAlias)]
pub struct Hello {
    // ...
}
```

It assumes that the derive is imported and referred to by its original name.
