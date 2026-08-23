Macro which controls how to export item's API. The behaviour is controlled with `transparent_api`
feature flag. If the flag is active, item's public fields will be exposed as public, however, if
it's not active, item will be exposed as opaque, i.e. no fields will be visible. This enables
internal libraries of Iroha to see and destructure data model items. On the other hand,
client libraries will only see opaque items and can be dynamically linked.

Additionally, this macro will rewrite private items as public when `transparent_api` is active.
If an item should remain private regardless of consumer library, just don't wrap it in this macro.

Should be used only on public module named `model`.
Macro will modify only structs, enums and unions. Other items will be left as is.

# Example

```
use iroha_data_model_derive::model;

#[model]
mod model {
    pub struct DataModel1 {
        pub item1: u32,
        item2: u64,
    }

    pub(crate) struct DataModel2 {
        pub item1: u32,
        item2: u64,
    }
}

/* will produce:
mod model {
    pub struct DataModel1 {
        #[cfg(feature = "transparent_api")]
        pub item1: u32,
        #[cfg(not(feature = "transparent_api"))]
        pub(crate) item1: u32,
        item2: u64
    }

    #[cfg(not(feature = "transparent_api"))]
    pub struct DataModel2 {
        pub item1: u32,
        item2: u64
    }

    #[cfg(feature = "transparent_api")]
    struct DataModel2 {
        pub item1: u32,
        item2: u64
    }
}
*/
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
