# Iroha Macros

This crate contains macros and attributes for Iroha projects:

- `FromVariant`, a macro used for implementing `From<Variant> for Enum` and `TryFrom<Enum> for Variant`

## Usage

Add the following to the manifest file of your Rust project:

```toml
iroha_derive = { path = "path/to/iroha_derive" }
```

## Examples

```rust
use iroha_derive::FromVariant;

trait MyTrait {}

// Use derive to derive the implementation of `FromVariant`:
#[derive(FromVariant)]
enum Obj {
    Uint(u32),
    Int(i32),
    String(String),
    // You can also skip implementing `From`
    Vec(#[skip_from] Vec<Obj>),
    // You can also skip implementing `From` for item inside containers such as `Box`
    Box(#[skip_container] Box<dyn MyTrait>)
}

// That would help you avoid doing this:
impl<T: Into<Obj>> From<Vec<T>> for Obj {
    fn from(vec: Vec<T>) -> Self {
        Obj::Vec(vec.into_iter().map(Into::into).collect())
    }
}
```
