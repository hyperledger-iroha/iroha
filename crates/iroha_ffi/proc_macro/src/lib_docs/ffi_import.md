Replace the function's body with a call to FFI function. Counterpart of [`macro@ffi_export`]

When placed on a structure, it integrates with `getset` to import derived getter/setter methods.

# Example:
```rust
#[iroha_ffi::ffi_import]
pub fn return_first_elem_from_arr(arr: [u8; 8]) -> u8 {
    // The body of this function is replaced with something like the following:
    // let mut store = Default::default();
    // let arr = iroha_ffi::FfiConvert::into_ffi(arr, &mut store);
    // let output = MaybeUninit::uninit();
    //
    // let call_res = __return_first_elem_from_arr(arr, output.as_mut_ptr());
    // if iroha_ffi::FfiReturn::Ok != call_res {
    //     panic!("Function call failed");
    // }
    //
    // iroha_ffi::FfiOutPtrRead::try_read_out(output.assume_init()).expect("Invalid type")
}

/* The following functions will be declared:
extern {
    fn __return_first_elem_from_arr(arr: *const [u8; 8]) -> u8;
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
