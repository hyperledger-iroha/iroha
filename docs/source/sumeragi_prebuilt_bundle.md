# Source-bound release binary bundles

`scripts/sumeragi_prebuilt_bundle.sh` builds, publishes and re-verifies a
private, read-only bundle of release executables for network tests that must
not build their own binaries. The shell functions call
`scripts/sumeragi_prebuilt_bundle.py` (`prepare-cache`, `create`, `validate`)
for every filesystem and manifest check; `iroha_test_network` consumes the
result.

## Bundle contents

| Manifest role | Bundle path | Export |
| --- | --- | --- |
| `irohad` | `release/iroha3d` | `TEST_NETWORK_BIN_IROHAD` |
| `iroha` | `release/iroha` | `TEST_NETWORK_BIN_IROHA` |
| `kagami` | `release/kagami` | `KAGAMI_BIN` |
| `irohad_taira` | `release/iroha3d_taira` | `TEST_NETWORK_BIN_IROHAD_TAIRA` |

All four come from one `--locked --offline --release` build in the default
feature graph; the Taira launcher and the standard daemon are distinct
executables of the `irohad` package. The bundle directory is
`<IROHA_RELEASE_ARTIFACT_ROOT>/sumeragi-release/<source-manifest-sha256>/programs/invocation.<token>`.
It holds the four executables (mode `0500`) and the schema-2 manifest
`.sumeragi-prebuilt-binaries.tsv` (mode `0400`) with exactly 25 ordered
tab-separated fields: schema version, source manifest, `Cargo.lock`, Cargo and
rustc version digests, host and target triples, profile, bundle directory, and
the relative path, SHA-256, size and mode of each executable. Every directory is
closed to mode `0500` after publication.

## Usage

The caller sources `scripts/sumeragi_release_process_policy.sh` (its root checks
and Cargo wrapper) and `scripts/sumeragi_prebuilt_bundle.sh`, sets
`CARGO_TARGET_DIR` and `IROHA_RELEASE_ARTIFACT_ROOT` to private owner-only
directories outside the repository, and passes the repository root and the
workspace source digest (`scripts/compute_workspace_source_manifest.py`):

- `sumeragi_ensure_source_bound_localnet_binaries <repo> <source-sha256>`
  builds a fresh bundle and exports `IROHA_TEST_TARGET_DIR` and
  `IROHA_RELEASE_PREBUILT_MANIFEST_SHA256`. When
  `IROHA_RELEASE_PREBUILT_MANIFEST_SHA256` is already set, the inherited bundle
  is validated and reused; it is never rebuilt or replaced.
- `sumeragi_export_source_bound_localnet_binaries <repo> <source-sha256>`
  re-validates the bundle and exports the four binary paths above.
- `sumeragi_localnet_binary_attestation_valid <repo> <source-sha256>` only
  validates.

Validation rejects a bundle whose manifest digest, source digest, profile,
bundle path or `Cargo.lock` digest differs, whose directory tree holds anything
beyond the published entries, or whose executables differ from the manifest in
path, digest, size or mode, are not single-link regular files, or change while
they are read.

## Consumer

With `IROHA_RELEASE_SOURCE_MANIFEST_SHA256` set, `iroha_test_network` resolves
binaries only from the bundle. It requires `IROHA_TEST_SKIP_BUILD=1`,
`IROHA_TEST_TARGET_DIR` naming an immediate `invocation.<token>` directory under
the manifest-addressed programs root of `IROHA_RELEASE_ARTIFACT_ROOT`, and a
manifest whose SHA-256 equals `IROHA_RELEASE_PREBUILT_MANIFEST_SHA256`; it then
checks every executable against the manifest. The BPNG alias-registry network
test (`integration_tests/tests/alias_registry_bootstrap_network.rs`) resolves
its binaries this way.

TODO(release-runner): no maintained runner currently provides that test's full
sealed release environment (`IROHA_RELEASE_SEALED_WORKTREE`,
`IROHA_RELEASE_EXPECTED_IDENTITY_PATH` and the other `IROHA_RELEASE_*`
identities), so it fails closed until one is added.
