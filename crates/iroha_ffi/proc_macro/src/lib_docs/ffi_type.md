Derive implementations of traits required to convert to and from an FFI-compatible type

# Attributes

* `#[ffi_type(opaque)]`
  serialize the type as opaque. If automatically derived type doesn't work just
  attach this attribute and force the type to be serialized as opaque across FFI

* `#[ffi_type(unsafe {robust})]`
  serialize the type as transparent with respect to the wrapped type where every
  valid bit pattern of the underlying type must be valid for the wrapper type.

Only applicable to `#[repr(transparent)]` types

# Safety

type must not have trap representations in the serialized form

* `#[ffi_type(local)]`
  marks the type as local, meaning it contains references to the local frame. If a type
  contains references to the local frame you won't be able to return it from an FFI function
  because the frame is destroyed on function return which would invalidate your type's references.

Only applicable to data-carrying enums.

NOTE: This attribute is likely to be removed in future versions

* `#[ffi_type(unsafe {robust_non_owning})]`
  when a type contains a raw pointer (e.g. `*const T`/`*mut T`) it's not possible to figure out
  whether it carries ownership of the data pointed to. Place this attribute on the field to
  indicate pointer doesn't own the data and is robust in the type. Alternatively, if the type
  is carrying ownership mark entire type as opaque with `#[ffi_type(opaque)]`. If the type
  is not carrying ownership, but is not robust convert it into an equivalent `iroha_ffi::ReprC`
  type that is validated when crossing the FFI boundary. It is also ok to mark non-owning,
  non-robust type as opaque

# Safety

* wrapping type must allow for all possible values of the pointer including `null` (it's robust)
* the wrapping types's field of the pointer type must not carry ownership (it's non owning)

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
