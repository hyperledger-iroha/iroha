Tools to work with file- and environment-based configuration.

The main tool here is [`read::ConfigReader`]. It is built around these key concepts:

- Read config from TOML files;
- Identify each configuration parameter by its path in the file;
- Parameter might have an environment variable alias, which overwrites value from files;
- Parameter might have a default value, applied if nothing was found in files/env.

The reader's goal is to:

- Give an exhaustive error report if something fails;
- Give origins of values for later use in error reports (see [`WithOrigin`]);
- Gives traces for debugging purposes (by [`log`] crate).

File-backed TOML loading is fail-closed and allocation-bounded. Each source must be a stable
regular file, and `extends` traversal rejects cycles, duplicate diamond loads, excessive
depth/source fanout, and excessive aggregate encoded bytes. The exact first-release ceilings are
exposed by [`toml::MAX_TOML_SOURCE_BYTES`] and the `MAX_TOML_EXTENDS_*` constants in [`read`].

## Example: raw usage

Let's say we want to read the following config:

```toml
[foo]
bar = "example" # has env alias BAR
baz = 42
more = { foo = 24 }
```

The reading and manual implementation of [`read::ReadConfig`] might look like:

```
use iroha_config_base::{
    WithOrigin,
    read::{ConfigReader, FinalWrap, ReadConfig},
    toml::TomlSource,
};
use norito::derive::JsonDeserialize;
use toml::toml;

struct Config {
    foo_bar: String,
    foo_baz: WithOrigin<u8>,
    more: Option<More>,
}

#[derive(JsonDeserialize)]
struct More {
    foo: u8,
}

impl ReadConfig for Config {
    fn read(reader: &mut ConfigReader) -> FinalWrap<Self> {
        let foo_bar = reader
            .read_parameter(["foo", "bar"])
            .env("BAR")
            .value_required()
            .finish();

        let foo_baz = reader
            .read_parameter(["foo", "baz"])
            .value_or_else(|| 100)
            .finish_with_origin();

        let more = reader
            .read_parameter(["foo", "more"])
            .value_optional()
            .finish();

        FinalWrap::value_fn(|| Self {
            foo_bar: foo_bar.unwrap(),
            foo_baz: foo_baz.unwrap(),
            more: more.unwrap(),
        })
    }
}

let _config = ConfigReader::new()
    .with_toml_source(TomlSource::inline(toml! {
        [foo]
        bar = "example"
        baz = 42
        more = { foo = 24 }
    }))
    .read_and_complete::<Config>()
    .expect("config is valid");
```

## Example: using macro

[`iroha_derive::ReadConfig`] macro simplifies manual work.
The previous example might be simplified as follows:

```
use iroha_config_base::{
    ReadConfig, WithOrigin,
    read::{ConfigReader, ReadConfig},
    toml::TomlSource,
};
use norito::derive::JsonDeserialize;
use toml::toml;

#[derive(ReadConfig)]
struct Config {
    #[config(nested)]
    foo: Foo,
}

#[derive(ReadConfig)]
struct Foo {
    #[config(env = "BAR")]
    bar: String,
    #[config(default = "100")]
    baz: WithOrigin<u8>,
    more: Option<More>,
}

#[derive(JsonDeserialize)]
struct More {
    foo: u8,
}

let config = ConfigReader::new()
    .with_toml_source(TomlSource::inline(toml! {
        [foo]
        bar = "bar"
    }))
    .read_and_complete::<Config>()
    .expect("config is valid");

assert_eq!(config.foo.bar, "bar");
assert_eq!(*config.foo.baz.value(), 100);
assert!(config.foo.more.is_none());
```

Here we also use nesting.

See macro documentation for details.
