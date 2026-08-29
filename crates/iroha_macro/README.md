# Iroha Macros

This crate contains macros and attributes for Iroha projects:

- `FromVariant`, a macro used for implementing `From<Variant> for Enum` and `TryFrom<Enum> for Variant`

## Usage

Add the following to the manifest file of your Rust project:

```toml
iroha_macro = { path = "path/to/iroha_macro" }
```

## Examples

```rust
use iroha_macro::FromVariant;

trait MyTrait {}

// Use derive to derive the implementation of `FromVariant`:
#[derive(FromVariant)]
enum Obj {
    Uint(u32),
    Int(i32),
    String(String),
    // You can also skip implementing `From`
    Vec(#[skip_from] Vec<Obj>),
    // Conversions always use the exact field type; no container is allocated implicitly.
    Box(Box<dyn MyTrait>)
}

// That would help you avoid doing this:
impl<T: Into<Obj>> From<Vec<T>> for Obj {
    fn from(vec: Vec<T>) -> Self {
        Obj::Vec(vec.into_iter().map(Into::into).collect())
    }
}
```
