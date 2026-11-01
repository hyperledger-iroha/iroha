# Source-bound release binary bundles

The structured `scripts/run_release_pipeline.py prepare-sumeragi-programs`
operation owns preparation for network tests that must not build their own binaries.
Its shell adapter is `scripts/sumeragi_prebuilt_bundle.sh`.
`sumeragi_prebuilt_bundle.py` supplies bounded protocol validation and has no
standalone publishing CLI.

## Source owners

| Boundary | Owner |
| --- | --- |
| Canonical manifest, publication and retained executable selection | `crates/iroha_release_bundle` |
| Original filesystem and native process custody | `crates/iroha_fs` |
| Bounded process interoperability | `crates/iroha_process_bridge` |
| Reviewed registry, original source keeper and independent artifact admission | `scripts/run_release_pipeline.py`, `release_entry_owner.py`, `release_bridge_admission.py` |
| Public/cached resolution and complete admission through child lifetime | `iroha_test_network::ResolvedProgram` and its process wrappers |

The entry and shared bridge require explicit independent executable/provenance
pins on every host. The original source keeper, registry, tools and locks remain
held through original child completion. Windows includes the complete Job;
Unix observes the original root. An unresolved ordinary observation keeps the
initiating Python process supervising the same session rather than exiting and
releasing its owners. This does not provide crash-proof interpreter custody.

## Canonical inventory

The flat invocation directory contains `iroha3d`, `iroha`, `kagami` and
`iroha3d_taira`, with `.exe` suffixes on Windows, plus `release-bundle.json`.
The sole Norito JSON schema is `iroha.release-bundle.v1`. It binds the native
target, source and Cargo.lock digests, tool transcripts, exact inventory, content
hashes, sizes, custody policy and original local file/directory identities.
Those local identities bind this invocation; they are not portable consensus IDs.

The namespace is
`<artifact-root>/sumeragi-release/<source-sha256>/programs/invocation.<token>`.
Unix seals directories/executables to owner `0500` and the manifest to `0400`.
Windows uses protected effective-owner DACLs with read/execute for directories
and executables, and read-only access for the manifest. Publication retains the
original creators across sealing; recovery authenticates the exact originals.
Unexpected entries, links, unsafe policies, altered content and substituted
identities refuse admission.

## Consumer contract and qualification

Release admission requires `IROHA_RELEASE_SOURCE_MANIFEST_SHA256`,
`IROHA_RELEASE_PREBUILT_MANIFEST_SHA256`, `IROHA_RELEASE_ARTIFACT_ROOT`,
`IROHA_TEST_TARGET_DIR` and `IROHA_TEST_SKIP_BUILD=1`. If
`IROHA_RELEASE_CARGO_LOCK_SHA256` is supplied, it must also match. The current
original Cargo.lock digest is always compared with the manifest. Invalid partial identities
refuse before developer caches, overrides or builds. Qualified resolution returns
a retained program; `diagnostic_path()` is observational and cannot authorize a
launch. Commands and children retain the complete admission owner through exit.
An inherited bundle is re-admitted and reused; invalid inherited anchors never
trigger a rebuild.

Canonical Rust admission tests and the `pytests/scripts/test_release_*`,
`test_pipeline_preparation`, `test_shell_preparation`, `test_native_supervision`
and `test_bridge_cli_supervision` suites cover these boundaries. The real native
producer/consumer corridor still requires independently trusted artifacts and
matching runtime checks. No maintained runner supplies a complete authenticated
BPNG release environment. Darwin original-image launch and Unix recursive Cargo
selection remain unresolved. See the [platform completion record](../../specs/windows_intel_completion_goals.md).
