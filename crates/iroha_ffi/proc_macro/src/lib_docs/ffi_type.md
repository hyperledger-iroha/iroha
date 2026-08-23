Derive implementations of traits required to convert to and from an FFI-compatible type

# Attributes

* `#[ffi_type(opaque)]`

  Serialize the type as opaque. If the automatically derived type does not work,
  attach this attribute to force the type to be serialized as opaque across FFI.

* `#[ffi_type(unsafe {robust})]`

  Serialize the type as transparent with respect to the wrapped type, where every
  valid bit pattern of the underlying type must be valid for the wrapper type.

  This is only applicable to `#[repr(transparent)]` types.

  **Safety:** The type must not have trap representations in the serialized form.

* `#[ffi_type(local)]`

  Mark the type as local, meaning it contains references to the local frame. A type
  containing references to the local frame cannot be returned from an FFI function
  because the frame is destroyed on function return, invalidating those references.

  This is only applicable to data-carrying enums.

  **Note:** This attribute is likely to be removed in future versions.

* `#[ffi_type(unsafe {robust_non_owning})]`

  When a type contains a raw pointer (for example, `*const T` or `*mut T`), it is
  not possible to determine whether it carries ownership of the pointed-to data.
  Place this attribute on the field to indicate that the pointer does not own the
  data and is robust in the type. Alternatively, if the type carries ownership,
  mark the entire type as opaque with `#[ffi_type(opaque)]`. If the type does not
  carry ownership but is not robust, convert it into an equivalent
  `iroha_ffi::ReprC` type that is validated when crossing the FFI boundary. A
  non-owning, non-robust type may also be marked as opaque.

  Safety requires both of the following:

  * The wrapping type must allow all possible pointer values, including `null`
    (it is robust).
  * The wrapping type's pointer field must not carry ownership (it is non-owning).

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
