# Executable build identity

Daemon, CLI and PK2 supply immutable `BuildIdentity` values to their runtime or
verifier. Core, Torii and telemetry libraries do not embed a Git revision. This
keeps a tooling-only commit from invalidating those expensive shared libraries.
Actual source changes still rebuild the affected libraries; executable metadata
and dependent executable linking still change for a new revision.

The canonical compiled source is `VERGEN_GIT_SHA`: an exact lowercase 40-digit
commit, or the explicit development label `local-fast-build`. A present
`IROHA_GIT_COMMIT_HASH` is a sealed artifact marker and must equal the exact
canonical commit. It is never an alternative source or fallback. Absent, padded,
malformed or contradictory metadata fails identity construction. The daemon and
CLI build helper discovers Git HEAD only when no explicit override is supplied;
PK2 has no package build script and needs explicit compiled metadata.

The authenticated release producer supplies the signed source commit to both
variables. Native release admission and PK2 release verification reject the
development label. Release admission continues to require the exact signed
source manifest, signer and immutable artifact closure. Valid syntax alone does
not establish provenance. Runtime environment variables and node configuration
cannot replace the compiled identity.

Both Docker source-build definitions require matching full commits in the
`IROHA_GIT_COMMIT_HASH` and `VERGEN_GIT_SHA` build arguments before invoking
Cargo. `scripts/build_release_image.sh` supplies both from the already admitted
source commit when packaging its prebuilt release binaries. Container builds
cannot infer a source identity from an empty build argument or omitted `.git`.

The Sumeragi build fingerprint hashes the package version immediately followed
by the source revision. One immutable daemon identity reaches normal startup,
pending-Kura recovery and the resumed normal loop. The same identity reaches
Torii public status and prover configuration. Prover reservations retain that
identity through reconfiguration; processing contexts bind it so proof results
cannot be reused across executable revisions. Target and feature metadata remain
public reporting fields and do not change this source fingerprint.

Use the existing warm target, native Cargo jobserver and approved linker for
local work:

```sh
cargo iroha-fast --stable-local-metadata -- check -p iroha_core --lib
cargo iroha-fast --stable-local-metadata -- build --profile local-release -p irohad --bin iroha3d
cargo iroha-fast --stable-local-metadata -- build -p iroha_core --features dev-tools --bin pk2_bridge_finality_verify
```

Keep the release-only `IROHA_GIT_COMMIT_HASH` unset in this development
environment. The wrapper supplies `VERGEN_GIT_SHA=local-fast-build`; it never
creates sealed artifact metadata. It rejects an inherited sealed marker before
starting a build, so contradictory metadata cannot waste a build and fail only at
startup. PK2 test fixtures supply explicit identities.

The local wrapper resolves both Cargo target and intermediate build directories
with offline, locked metadata before building. It rejects lanes whose
`.taira-build-lane/role.json` assigns them to authenticated release work, including
default and config-selected targets. Select one stable `--target-slot <name>` for
concurrent development, or an existing external development lane. It creates no
replacement cache. Use explicit Cargo command names; aliases can hide target
selectors and are rejected. Direct Cargo remains outside this advisory wrapper
guard, so authenticated preparation still owns its locks and immutable captures.

Native release checks isolate executables with descriptor-bound APFS clones on
macOS. Each clone has an independent inode and is verified and frozen before
execution; later Cargo writes cannot change its contents. Unsupported clone
filesystems use the streamed copy only when all remaining bytes fit above the
working-space reserve. Cloning avoids allocating a second full set of native
test binaries while preserving the same source and destination checks. Warm
Cargo targets and completed attempt receipts remain retained.

Basic and full Taira checks compile the same native graph and consensus harness.
Basic runs the universal default-route transaction sequence; full also runs the
separate-dataspace sequence. Both retain four validators, the three-dataspace
fixture, mandatory NPoS/DA, actual local/global Applied state and a signed-snapshot
restart. Scope selection changes executed tests, not daemon features or artifacts.

For PK2 release verification, the authenticated source/artifact corridor must
supply its already validated source commit to both build-time variables. Run in
that corridor's captured source checkout through its controlled Cargo invocation,
using its existing warm target; the local accelerator rejects release lanes:

```sh
IROHA_GIT_COMMIT_HASH="$TAIRA_AUTHENTICATED_SOURCE_COMMIT" \
VERGEN_GIT_SHA="$TAIRA_AUTHENTICATED_SOURCE_COMMIT" \
cargo build --locked --release -p iroha_core --features dev-tools --bin pk2_bridge_finality_verify
```

An ordinary PK2 build without canonical metadata can show help, but cannot
verify release finality. A local development build also rejects release
verification; neither missing metadata nor a development label is a fallback
source identity.

Measure actual Cargo `compiler-artifact` freshness in the same warm lane when
changing only the compiled revision. Core, Torii and telemetry must remain
fresh while executable metadata changes. Do not infer cache reuse or a timing
improvement from compiler log wording alone.
