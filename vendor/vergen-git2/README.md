# vergen
### `vergen`, `vergen-git2`, `vergen-gitcl`, `vergen-gix`, `vergen-pretty`
The `vergen` suite of tools allow you to embed environment variables generated at build time into your code.  For example,
I may care about the last git commit number and need to reference it in my code.  You can configure one of the `vergen` git tools in cargo [build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html) and can emit a `VERGEN_GIT_SHA` environment variable for use in your code.

The `vergen` suite of tools can emit the following [output]((https://doc.rust-lang.org/cargo/reference/build-scripts.html#outputs-of-the-build-script)):

- Will emit [`cargo:rustc-env=VAR=VALUE`](https://doc.rust-lang.org/cargo/reference/build-scripts.html#cargorustc-envvarvalue)
for each feature you have enabled.  These can be referenced with the [`env!`](https://doc.rust-lang.org/std/macro.env.html) or [`option_env!`](https://doc.rust-lang.org/std/macro.option_env.html) macro in your code.
- If using one of the git enabled libraries, will emit [`cargo:rerun-if-changed=.git/HEAD`](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed).  This is done to ensure any git instructions are regenerated when commits are made.
- If using one of the git enabled libraries, will emit [`cargo:rerun-if-changed=.git/<path_to_ref>`](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed).  This is done to ensure any git instructions are regenerated when commits are made.
- Can emit [`cargo:warning`](https://doc.rust-lang.org/cargo/reference/build-scripts.html#cargo-warning) outputs if the
[`fail_on_error`](EmitBuilder::fail_on_error) feature is not enabled and the requested variable is defaulted through error or
the [`idempotent`](EmitBuilder::idempotent) flag.
- Will emit [`cargo:rerun-if-changed=build.rs`](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed)
to rerun instruction emission if the `build.rs` file changed.
- Will emit [`cargo:rerun-if-env-changed=VERGEN_IDEMPOTENT`](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed)
to rerun instruction emission if the `VERGEN_IDEMPOTENT` environment variable has changed.
- Will emit [`cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH`](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed)
to rerun instruction emission if the `SOURCE_DATE_EPOCH` environment variable has changed.

`vergen-pretty` is a macro and pretty printer for `vergen` based cargo instruction output.

## Current Releases
### vergen
[![docs.rs](https://docs.rs/vergen/badge.svg)](https://docs.rs/vergen)
[![Crates.io](https://img.shields.io/crates/v/vergen.svg)](https://crates.io/crates/vergen)
[![Crates.io](https://img.shields.io/crates/l/vergen.svg)](https://crates.io/crates/vergen)
[![Crates.io](https://img.shields.io/crates/d/vergen.svg)](https://crates.io/crates/vergen)
[![codecov](https://codecov.io/gh/rustyhorde/vergen/branch/master/graph/badge.svg?token=cBXro7o2UN)](https://codecov.io/gh/rustyhorde/vergen)
[![CI](https://github.com/rustyhorde/vergen/actions/workflows/vergen.yml/badge.svg)](https://github.com/rustyhorde/vergen/actions)
[![sponsor](https://img.shields.io/github/sponsors/crazysacx?logo=github-sponsors)](https://github.com/sponsors/CraZySacX)

### vergen-git2
[![docs.rs](https://docs.rs/vergen-git2/badge.svg)](https://docs.rs/vergen-git2)
[![Crates.io](https://img.shields.io/crates/v/vergen-git2.svg)](https://crates.io/crates/vergen-git2)
[![Crates.io](https://img.shields.io/crates/l/vergen-git2.svg)](https://crates.io/crates/vergen-git2)
[![Crates.io](https://img.shields.io/crates/d/vergen-git2.svg)](https://crates.io/crates/vergen-git2)

### vergen-gitcl
[![docs.rs](https://docs.rs/vergen-gitcl/badge.svg)](https://docs.rs/vergen-gitcl)
[![Crates.io](https://img.shields.io/crates/v/vergen-gitcl.svg)](https://crates.io/crates/vergen-gitcl)
[![Crates.io](https://img.shields.io/crates/l/vergen-gitcl.svg)](https://crates.io/crates/vergen-gitcl)
[![Crates.io](https://img.shields.io/crates/d/vergen-gitcl.svg)](https://crates.io/crates/vergen-gitcl)

### vergen-gix
[![docs.rs](https://docs.rs/vergen-gix/badge.svg)](https://docs.rs/vergen-gix)
[![Crates.io](https://img.shields.io/crates/v/vergen-gix.svg)](https://crates.io/crates/vergen-gix)
[![Crates.io](https://img.shields.io/crates/l/vergen-gix.svg)](https://crates.io/crates/vergen-gix)
[![Crates.io](https://img.shields.io/crates/d/vergen-gix.svg)](https://crates.io/crates/vergen-gix)

### vergen-pretty
[![docs.rs](https://docs.rs/vergen-pretty/badge.svg)](https://docs.rs/vergen-pretty)
[![Crates.io](https://img.shields.io/crates/v/vergen-pretty.svg)](https://crates.io/crates/vergen-pretty)
[![Crates.io](https://img.shields.io/crates/l/vergen-pretty.svg)](https://crates.io/crates/vergen-pretty)
[![Crates.io](https://img.shields.io/crates/d/vergen-pretty.svg)](https://crates.io/crates/vergen-pretty)

## MSRV
The current minimum supported rust version is 1.81.0

## ⚠️ Notes on version 9 ⚠️
* Version 9 introduces 3 new libraries, `vergen-git2`, `vergen-gitcl`, and `vergen-gix` that will be versioned independently from `vergen`.
* The 3 new libraries are intended to be drop in replacements for `vergen` when you need to generate git based cargo build script instructions.
* The git based features have been removed from the base `vergen` library.
* `vergen` now contains the `build`, `cargo`, `rustc`, and `sysinfo` feature implementations.   These features are re-exported by the new libraries allowing you to configure the output as you have previously.
* Version 9 introduces the `AddCustomEntries` trait.  Implementing this trait allows you to include your own custom Cargo instructions, using `vergen` as the engine to generate them. See the [`AddCustomEntries`](https://docs.rs/vergen/9.0.0/vergen/trait.AddCustomEntries.html) docs for more information.
* The [version 8 branch](https://github.com/rustyhorde/vergen/tree/legacy/v8) will be maintained for some time.

### Why?
This was done to resolve issues with [Cargo feature unification](https://doc.rust-lang.org/cargo/reference/features.html#mutually-exclusive-features) and mutually exclusive features.  Previous versions of `vergen` had 3 mutually exclusive features (`git2`, `gitcl`, and `gix`).  Feature unification could cause compilation issues if you had included a dependency that also used `vergen` but had configured a different git feature.  Splitting the git backends into separate libraries helps alleviate this issue.

## Migration from version 8
See the documentation at [MIGRATING_v8_to_v9.md](MIGRATING_v8_to_v9.md)

## Documentation
* [vergen](https://docs.rs/vergen/latest)
* [vergen-git2](https://docs.rs/vergen-git2/latest)
* [vergen-gitcl](https://docs.rs/vergen-gitcl/latest)
* [vergen-gix](https://docs.rs/vergen-gix/latest)
* [vergen-pretty](https://docs.rs/vergen-pretty/latest)

## Contributing
See the documentation at [CONTRIBUTING.md](CONTRIBUTING.md)

## License

Licensed under either of
 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)
at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
