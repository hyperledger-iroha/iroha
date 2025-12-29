// Copyright (c) 2022 vergen developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! # vergen-git2 - Emit cargo instructions from a build script
//! `vergen-git2` uses the [`git2`](https://docs.rs/git2) library to generate the git instructions.
//!
//! `vergen-git2`, when used in conjunction with cargo [build scripts] can emit the following:
//!
//! - Will emit [`cargo:rustc-env=VAR=VALUE`](https://doc.rust-lang.org/cargo/reference/build-scripts.html#cargorustc-envvarvalue)
//!   for each feature you have enabled.  These can be referenced with the [`env`!](std::env!) or [`option_env`!](std::option_env!) macro in your code.
//! - Can emit [`cargo:warning`](https://doc.rust-lang.org/cargo/reference/build-scripts.html#cargo-warning) outputs if the
//!   [`fail_on_error`](Emitter::fail_on_error) feature is not enabled and the requested variable is defaulted through error or
//!   the [`idempotent`](Emitter::idempotent) flag.
//! - Will emit [`cargo:rerun-if-changed=.git/HEAD`](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed)
//!   if git instructions are emitted.  This is done to ensure any git instructions are regenerated when commits are made.
//! - Will emit [`cargo:rerun-if-changed=.git/<path_to_ref>`](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed)
//!   if git instructions are emitted.  This is done to ensure any git instructions are regenerated when commits are made.
//! - Will emit [`cargo:rerun-if-changed=build.rs`](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed)
//!   to rerun instruction emission if the `build.rs` file changed.
//! - Will emit [`cargo:rerun-if-env-changed=VERGEN_IDEMPOTENT`](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed)
//!   to rerun instruction emission if the `VERGEN_IDEMPOTENT` environment variable has changed.
//! - Will emit [`cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH`](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed)
//!   to rerun instruction emission if the `SOURCE_DATE_EPOCH` environment variable has changed.
//!
//! ## Usage
//!
//! 1. Ensure you have build scripts enabled via the `build` configuration in your `Cargo.toml`
//!
//! ```toml
//! [package]
//! #..
//! build = "build.rs"
//! ```
//!
//! 2. Add `vergen-git2` as a build dependency in `Cargo.toml`, specifying the features you wish to enable.
//!
//! ```toml
//! [dependencies]
//! #..
//!
//! [build-dependencies]
//! # All features enabled
//! vergen-git2 = { version = "1.0.0", features = ["build", "cargo", "rustc", "si"] }
//! # or
//! vergen-git2 = { version = "1.0.0", features = ["build"] }
//! # if you wish to disable certain features
//! ```
//!
//! 3. Create a `build.rs` file that uses `vergen-git2` to emit cargo instructions.  Configuration
//!    starts with [`Emitter`].  Eventually you will call [`emit`](Emitter::emit) to output the
//!    cargo instructions. See the [`emit`](Emitter::emit) documentation for more robust examples.
//!
//! #### Generate all output
//!
//! ```
//! # use anyhow::Result;
//! # use vergen_git2::{Emitter, Git2Builder};
#![cfg_attr(feature = "build", doc = r"# use vergen_git2::BuildBuilder;")]
#![cfg_attr(feature = "cargo", doc = r"# use vergen_git2::CargoBuilder;")]
#![cfg_attr(feature = "rustc", doc = r"# use vergen_git2::RustcBuilder;")]
#![cfg_attr(feature = "si", doc = r"# use vergen_git2::SysinfoBuilder;")]
#![cfg_attr(feature = "cargo", doc = r"# use test_util::with_cargo_vars;")]
//! #
//! # pub fn main() -> Result<()> {
#![cfg_attr(feature = "cargo", doc = r"# let result = with_cargo_vars(|| {")]
//! // NOTE: This will output everything, and requires all features enabled.
//! // NOTE: See the specific builder documentation for configuration options.
#![cfg_attr(feature = "build", doc = r"let build = BuildBuilder::all_build()?;")]
#![cfg_attr(feature = "cargo", doc = r"let cargo = CargoBuilder::all_cargo()?;")]
//! let git2 = Git2Builder::all_git()?;
#![cfg_attr(feature = "rustc", doc = r"let rustc = RustcBuilder::all_rustc()?;")]
#![cfg_attr(feature = "si", doc = r"let si = SysinfoBuilder::all_sysinfo()?;")]
//!
//! Emitter::default()
#![cfg_attr(feature = "build", doc = r"    .add_instructions(&build)?")]
#![cfg_attr(feature = "cargo", doc = r"    .add_instructions(&cargo)?")]
//!     .add_instructions(&git2)?
#![cfg_attr(feature = "rustc", doc = r"    .add_instructions(&rustc)?")]
#![cfg_attr(feature = "si", doc = r"    .add_instructions(&si)?")]
//!     .emit()?;
#![cfg_attr(
    feature = "cargo",
    doc = r"
# Ok(())
# });
# assert!(result.is_ok());"
)]
//! #    Ok(())
//! # }
//! ```
//! #### Sample Output
//! ```text
//! cargo:rustc-env=VERGEN_BUILD_DATE=2024-01-31
//! cargo:rustc-env=VERGEN_BUILD_TIMESTAMP=2024-01-31T03:26:34.065893658Z
//! cargo:rustc-env=VERGEN_CARGO_DEBUG=true
//! cargo:rustc-env=VERGEN_CARGO_FEATURES=
//! cargo:rustc-env=VERGEN_CARGO_OPT_LEVEL=0
//! cargo:rustc-env=VERGEN_CARGO_TARGET_TRIPLE=x86_64-unknown-linux-gnu
//! cargo:rustc-env=VERGEN_CARGO_DEPENDENCIES=anyhow 1.0.79,vergen-pretty 0.3.2
//! cargo:rustc-env=VERGEN_GIT_BRANCH=master
//! cargo:rustc-env=VERGEN_GIT_COMMIT_AUTHOR_EMAIL=emitter@vergen.com
//! cargo:rustc-env=VERGEN_GIT_COMMIT_AUTHOR_NAME=Jason Ozias
//! cargo:rustc-env=VERGEN_GIT_COMMIT_COUNT=44
//! cargo:rustc-env=VERGEN_GIT_COMMIT_DATE=2024-01-30
//! cargo:rustc-env=VERGEN_GIT_COMMIT_MESSAGE=depsup
//! cargo:rustc-env=VERGEN_GIT_COMMIT_TIMESTAMP=2024-01-30T21:43:43.000000000Z
//! cargo:rustc-env=VERGEN_GIT_DESCRIBE=0.1.0-beta.1-15-g728e25c
//! cargo:rustc-env=VERGEN_GIT_SHA=728e25ca5bb7edbbc505f12b28c66b2b27883cf1
//! cargo:rustc-env=VERGEN_RUSTC_CHANNEL=nightly
//! cargo:rustc-env=VERGEN_RUSTC_COMMIT_DATE=2024-01-29
//! cargo:rustc-env=VERGEN_RUSTC_COMMIT_HASH=5518eaa946291f00471af8b254b2a1715f234882
//! cargo:rustc-env=VERGEN_RUSTC_HOST_TRIPLE=x86_64-unknown-linux-gnu
//! cargo:rustc-env=VERGEN_RUSTC_LLVM_VERSION=17.0
//! cargo:rustc-env=VERGEN_RUSTC_SEMVER=1.77.0-nightly
//! cargo:rustc-env=VERGEN_SYSINFO_NAME=Arch Linux
//! cargo:rustc-env=VERGEN_SYSINFO_OS_VERSION=Linux  Arch Linux
//! cargo:rustc-env=VERGEN_SYSINFO_USER=jozias
//! cargo:rustc-env=VERGEN_SYSINFO_TOTAL_MEMORY=31 GiB
//! cargo:rustc-env=VERGEN_SYSINFO_CPU_VENDOR=AuthenticAMD
//! cargo:rustc-env=VERGEN_SYSINFO_CPU_CORE_COUNT=8
//! cargo:rustc-env=VERGEN_SYSINFO_CPU_NAME=cpu0,cpu1,cpu2,cpu3,cpu4,cpu5,cpu6,cpu7
//! cargo:rustc-env=VERGEN_SYSINFO_CPU_BRAND=AMD Ryzen Threadripper 1900X 8-Core Processor
//! cargo:rustc-env=VERGEN_SYSINFO_CPU_FREQUENCY=3792
//! cargo:rerun-if-changed=/home/jozias/projects/rust-lang/vergen-cl/.git/HEAD
//! cargo:rerun-if-changed=/home/jozias/projects/rust-lang/vergen-cl/.git/refs/heads/master
//! cargo:rerun-if-changed=build.rs
//! cargo:rerun-if-env-changed=VERGEN_IDEMPOTENT
//! cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH
//! ```
//!
//! #### Generate specific output
//!
//! ```
//! # use anyhow::Result;
//! # use vergen_git2::{Emitter, Git2Builder};
#![cfg_attr(feature = "build", doc = r"# use vergen_git2::BuildBuilder;")]
#![cfg_attr(feature = "cargo", doc = r"# use vergen_git2::CargoBuilder;")]
#![cfg_attr(feature = "rustc", doc = r"# use vergen_git2::RustcBuilder;")]
#![cfg_attr(feature = "si", doc = r"# use vergen_git2::SysinfoBuilder;")]
#![cfg_attr(feature = "cargo", doc = r"# use test_util::with_cargo_vars;")]
//! #
//! # pub fn main() -> Result<()> {
#![cfg_attr(feature = "cargo", doc = r"# let result = with_cargo_vars(|| {")]
#![cfg_attr(
    feature = "build",
    doc = r"// NOTE: This will output only the instructions specified.
// NOTE: See the specific builder documentation for configuration options. 
let build = BuildBuilder::default().build_timestamp(true).build()?;"
)]
#![cfg_attr(
    feature = "cargo",
    doc = r"let cargo = CargoBuilder::default().opt_level(true).build()?;"
)]
//! let git2 = Git2Builder::default().commit_timestamp(true).build()?;
#![cfg_attr(
    feature = "rustc",
    doc = r"let rustc = RustcBuilder::default().semver(true).build()?;"
)]
#![cfg_attr(
    feature = "si",
    doc = r"let si = SysinfoBuilder::default().cpu_core_count(true).build()?;"
)]
//!
//! Emitter::default()
#![cfg_attr(feature = "build", doc = r"    .add_instructions(&build)?")]
#![cfg_attr(feature = "cargo", doc = r"    .add_instructions(&cargo)?")]
//!     .add_instructions(&git2)?
#![cfg_attr(feature = "rustc", doc = r"    .add_instructions(&rustc)?")]
#![cfg_attr(feature = "si", doc = r"    .add_instructions(&si)?")]
//!     .emit()?;
#![cfg_attr(
    feature = "cargo",
    doc = r"
#   Ok(())
# });
# assert!(result.is_ok());"
)]
//! #     Ok(())
//! # }
//! ```
//! #### Sample Output
//! ```text
//! cargo:rustc-env=VERGEN_BUILD_TIMESTAMP=2024-01-31T03:26:34.065893658Z
//! cargo:rustc-env=VERGEN_CARGO_OPT_LEVEL=0
//! cargo:rustc-env=VERGEN_GIT_COMMIT_TIMESTAMP=2024-01-30T21:43:43.000000000Z
//! cargo:rustc-env=VERGEN_RUSTC_SEMVER=1.77.0-nightly
//! cargo:rustc-env=VERGEN_SYSINFO_CPU_CORE_COUNT=8
//! cargo:rerun-if-changed=/home/jozias/projects/rust-lang/vergen-cl/.git/HEAD
//! cargo:rerun-if-changed=/home/jozias/projects/rust-lang/vergen-cl/.git/refs/heads/master
//! cargo:rerun-if-changed=build.rs
//! cargo:rerun-if-env-changed=VERGEN_IDEMPOTENT
//! cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH
//! ```
//!
//! 4. Use the [`env!`](std::env!) or [`option_env!`](std::option_env!) macro in your code to read the environment variables.
//!
//! ```
//! if let Some(timestamp) = option_env!("VERGEN_BUILD_TIMESTAMP") {
//!     println!("Build Timestamp: {timestamp}");
//! }
//! if let Some(describe) = option_env!("VERGEN_GIT_DESCRIBE") {
//!     println!("git describe: {describe}");
//! }
//! ```
//!
//! ## Features
//! `vergen-git2` has four main feature toggles allowing you to customize your output. No features are enabled by default.
//! You **must** specifically enable the features you wish to use.
//!
//! | Feature | Enables |
//! | ------- | ------- |
//! |  build  | `VERGEN_BUILD_*` instructions |
//! |  cargo  | `VERGEN_CARGO_*` instructions |
//! |  rustc  | `VERGEN_RUSTC_*` instructions |
//! |   si    | `VERGEN_SYSINFO_*` instructions |
//!
//! ## Environment Variables
//! `vergen-git2` currently recognizes the following environment variables. The full list of the environment variable names can be
//! found as [constants here](https://docs.rs/vergen-lib/latest/vergen_lib/constants/features/index.html)
//!
//! | Variable | Functionality |
//! | -------- | ------------- |
//! | `VERGEN_IDEMPOTENT` | If this environment variable is set `vergen` will use the idempotent output feature regardless of the configuration set in `build.rs`.  This exists mainly to allow package maintainers to force idempotent output to generate deterministic binary output. |
//! | `SOURCE_DATE_EPOCH` | If this environment variable is set `vergen` will use the value (unix time since epoch) as the basis for a time based instructions.  This can help emit deterministic instructions. |
//! | `VERGEN_BUILD_*` | If this environment variable is set `vergen` will use the value you specify for the output rather than generating it. |
//! | `VERGEN_CARGO_*` | If this environment variable is set `vergen` will use the value you specify for the output rather than generating it. |
//! | `VERGEN_GIT_*` | If this environment variable is set `vergen` will use the value you specify for the output rather than generating it. |
//! | `VERGEN_RUSTC_*` | If this environment variable is set `vergen` will use the value you specify for the output rather than generating it. |
//! | `VERGEN_SYSINFO_*` | If this environment variable is set `vergen` will use the value you specify for the output rather than generating it. |
//!
//! [build scripts]: https://doc.rust-lang.org/cargo/reference/build-scripts.html#outputs-of-the-build-script
//! [cargo:rustc-env]: https://doc.rust-lang.org/cargo/reference/build-scripts.html#rustc-env
//! [cargo:rerun-if-changed]: https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed
//!

// rustc lints
#![cfg_attr(
    all(feature = "unstable", nightly),
    feature(
        multiple_supertrait_upcastable,
        must_not_suspend,
        non_exhaustive_omitted_patterns_lint,
        rustdoc_missing_doc_code_examples,
        strict_provenance_lints,
    )
)]
#![cfg_attr(nightly, allow(single_use_lifetimes, unexpected_cfgs))]
#![cfg_attr(
    nightly,
    deny(
        absolute_paths_not_starting_with_crate,
        ambiguous_glob_imports,
        ambiguous_glob_reexports,
        ambiguous_wide_pointer_comparisons,
        anonymous_parameters,
        array_into_iter,
        asm_sub_register,
        async_fn_in_trait,
        bad_asm_style,
        bare_trait_objects,
        break_with_label_and_loop,
        clashing_extern_declarations,
        coherence_leak_check,
        confusable_idents,
        const_evaluatable_unchecked,
        const_item_mutation,
        dangling_pointers_from_temporaries,
        dead_code,
        deprecated,
        deprecated_in_future,
        deprecated_where_clause_location,
        deref_into_dyn_supertrait,
        deref_nullptr,
        drop_bounds,
        dropping_copy_types,
        dropping_references,
        duplicate_macro_attributes,
        dyn_drop,
        elided_lifetimes_in_associated_constant,
        elided_lifetimes_in_paths,
        ellipsis_inclusive_range_patterns,
        explicit_outlives_requirements,
        exported_private_dependencies,
        ffi_unwind_calls,
        forbidden_lint_groups,
        forgetting_copy_types,
        forgetting_references,
        for_loops_over_fallibles,
        function_item_references,
        hidden_glob_reexports,
        improper_ctypes,
        improper_ctypes_definitions,
        inline_no_sanitize,
        internal_features,
        invalid_from_utf8,
        invalid_macro_export_arguments,
        invalid_nan_comparisons,
        invalid_value,
        irrefutable_let_patterns,
        keyword_idents_2018,
        keyword_idents_2024,
        large_assignments,
        late_bound_lifetime_arguments,
        legacy_derive_helpers,
        let_underscore_drop,
        macro_use_extern_crate,
        map_unit_fn,
        meta_variable_misuse,
        missing_abi,
        missing_copy_implementations,
        missing_debug_implementations,
        missing_docs,
        mixed_script_confusables,
        named_arguments_used_positionally,
        never_type_fallback_flowing_into_unsafe,
        no_mangle_generic_items,
        non_ascii_idents,
        non_camel_case_types,
        non_contiguous_range_endpoints,
        non_fmt_panics,
        non_local_definitions,
        non_shorthand_field_patterns,
        non_snake_case,
        non_upper_case_globals,
        noop_method_call,
        opaque_hidden_inferred_bound,
        overlapping_range_endpoints,
        path_statements,
        private_bounds,
        private_interfaces,
        redundant_lifetimes,
        redundant_semicolons,
        refining_impl_trait_internal,
        refining_impl_trait_reachable,
        renamed_and_removed_lints,
        repr_transparent_external_private_fields,
        rust_2021_incompatible_closure_captures,
        rust_2021_incompatible_or_patterns,
        rust_2021_prefixes_incompatible_syntax,
        rust_2021_prelude_collisions,
        semicolon_in_expressions_from_macros,
        special_module_name,
        stable_features,
        static_mut_refs,
        suspicious_double_ref_op,
        trivial_bounds,
        trivial_casts,
        trivial_numeric_casts,
        type_alias_bounds,
        tyvar_behind_raw_pointer,
        uncommon_codepoints,
        unconditional_recursion,
        uncovered_param_in_projection,
        undefined_naked_function_abi,
        ungated_async_fn_track_caller,
        uninhabited_static,
        unit_bindings,
        unknown_lints,
        unknown_or_malformed_diagnostic_attributes,
        unnameable_test_items,
        unnameable_types,
        unreachable_code,
        unreachable_patterns,
        unreachable_pub,
        unsafe_code,
        unsafe_op_in_unsafe_fn,
        unstable_name_collisions,
        unstable_syntax_pre_expansion,
        unused_allocation,
        unused_assignments,
        unused_associated_type_bounds,
        unused_attributes,
        unused_braces,
        unused_comparisons,
        unused_crate_dependencies,
        unused_doc_comments,
        unused_extern_crates,
        unused_features,
        unused_import_braces,
        unused_imports,
        unused_labels,
        unused_lifetimes,
        unused_macro_rules,
        unused_macros,
        unused_must_use,
        unused_mut,
        unused_parens,
        unused_qualifications,
        unused_results,
        unused_unsafe,
        unused_variables,
        useless_ptr_null_checks,
        variant_size_differences,
        while_true,
    )
)]
#![cfg_attr(all(nightly), allow(unstable_features))]
// If nightly and unstable, allow `incomplete_features` and `unstable_features`
#![cfg_attr(all(feature = "unstable", nightly), allow(incomplete_features))]
// If nightly and not unstable, deny `incomplete_features` and `unstable_features`
#![cfg_attr(
    all(not(feature = "unstable"), nightly),
    deny(incomplete_features, unstable_features)
)]
// The unstable lints
#![cfg_attr(
    all(feature = "unstable", nightly),
    deny(
        fuzzy_provenance_casts,
        lossy_provenance_casts,
        multiple_supertrait_upcastable,
        must_not_suspend,
        non_exhaustive_omitted_patterns,
        unfulfilled_lint_expectations,
    )
)]
// clippy lints
#![cfg_attr(nightly, deny(clippy::all, clippy::pedantic))]
// rustdoc lints
#![cfg_attr(
    nightly,
    deny(
        rustdoc::bare_urls,
        rustdoc::broken_intra_doc_links,
        rustdoc::invalid_codeblock_attributes,
        rustdoc::invalid_html_tags,
        rustdoc::missing_crate_level_docs,
        rustdoc::private_doc_tests,
        rustdoc::private_intra_doc_links,
    )
)]
#![cfg_attr(
    all(nightly, feature = "unstable"),
    deny(rustdoc::missing_doc_code_examples)
)]
#![cfg_attr(all(doc, nightly), feature(doc_auto_cfg))]
#![cfg_attr(all(docsrs, nightly), feature(doc_cfg))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod git2;

#[cfg(test)]
use {lazy_static as _, regex as _, temp_env as _};
// This is here to appease the `unused_crate_dependencies` lint
#[cfg(not(any(
    feature = "build",
    feature = "cargo",
    feature = "rustc",
    feature = "si"
)))]
use vergen as _;

pub use crate::git2::Git2;
pub use crate::git2::Git2Builder;
#[cfg(feature = "build")]
pub use vergen::BuildBuilder;
#[cfg(feature = "cargo")]
pub use vergen::CargoBuilder;
#[cfg(feature = "si")]
pub use vergen::CpuRefreshKind;
#[cfg(feature = "cargo")]
pub use vergen::DependencyKind;
#[cfg(feature = "si")]
pub use vergen::MemoryRefreshKind;
#[cfg(feature = "si")]
pub use vergen::ProcessRefreshKind;
#[cfg(feature = "si")]
pub use vergen::RefreshKind;
#[cfg(feature = "rustc")]
pub use vergen::RustcBuilder;
#[cfg(feature = "si")]
pub use vergen::SysinfoBuilder;
pub use vergen_lib::AddCustomEntries;
pub use vergen_lib::CargoRerunIfChanged;
pub use vergen_lib::CargoWarning;
pub use vergen_lib::DefaultConfig;
pub use vergen_lib::Emitter;
