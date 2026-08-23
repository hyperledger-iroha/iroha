Replace struct/enum/union definition with opaque pointer. This applies to types that
are converted to an opaque pointer when sent across FFI but does not affect any other
item wrapped with this macro (e.g. fieldless enums). This is so that most of the time
users can safely wrap all of their structs with this macro and not be concerned with the
cognitive load of figuring out which structs are converted to opaque pointers.

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
