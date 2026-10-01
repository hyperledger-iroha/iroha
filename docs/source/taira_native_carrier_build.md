# Native Taira dev carrier producer

`scripts/taira_native_carrier_build.py` owns a build-only native Linux AArch64
lane for `irohad/iroha3d_taira` and `iroha_cli/iroha`. It is distinct from
`taira_release.py prepare` (four cross-compiled release-profile artifacts) and
from the independently owned native finality-helper producer. It runs no tests,
performs no deployment, and never signs an input.

The caller supplies a canonical owner-only JSON plan with schema
`taira.native-carrier-build.plan.v1`. `plan-only --plan /absolute/plan.json`
validates and prints its exact bytes; `build --plan /absolute/plan.json` executes
it. The signed source checkout must contain this producer and its maintained
helper closure, have the selected clean HEAD and tree on `optimizations`, and
remain unchanged through final artifact capture. The controller executes source
bytes, excluding cached bytecode. The existing checkout branch is never changed.

The closed plan contains:

- `source`: `repo_root`, full lowercase `commit` and `tree`, full uppercase
  OpenPGP signing-key `signer`, `cargo_lock_sha256`, and `public_key` with
  `path`, `sha256`, `size`. GPG classifies the key export as public-only before
  Python reads its bytes; an isolated public keyring verifies the exact commit
  signature. The original public key and plan are retained.
- `target_dir`: an existing owner-held 0700 warm target, separate from the routine
  development lane. `lane_owner_repo_root` preserves the exact existing
  `.taira-build-lane/role.json` repository binding. Its `release` role denotes
  authenticated lane custody; the actual carrier profile remains `dev`.
  The selected signed checkout may differ from this retained lane owner path.
- `output_dir`: a fresh absolute attempt directory, separate from the source
  and target, below an existing owner-held 0700 parent. Attempts cannot be
  overwritten or resumed; retry uses a new output and the same warm target.
- `tools`: exact `invocation`, canonical `path`, SHA256 and byte `size` for
  `git`, `gpg`, `python`, `cargo`, `rustc`, `rustdoc`, `compiler`, `linker`.
  The actual interpreter and PATH-selected capture Git must match. Tool bytes,
  resolutions and metadata are rechecked before and after work.
- `environment`: exactly the public variables in `ENV_NAMES` in the producer.
  Required choices include six jobs, offline Cargo, incremental dev output,
  unpacked dev/test split debug metadata, exact source commit variables,
  Rust tool paths/channel, canonical target and isolated Cargo home. LLVM driver
  and linker arguments are derived from the pinned tool paths and encoded with
  U+001F. `HOME` stays the native user's home. Runtime secrets, wrappers and
  ambient compiler overrides are not forwarded. The captured source path for
  `CARGO_ZIGBUILD_ZIG_PATH` is
  `<target>/taira-release-sources/<sha256(target-path-bytes)[:24]>/source/scripts/zig_linux_gnu.py`.
  This wrapper is retained as a source-bound environment input; Cargo invokes
  native `build` and never invokes Zig in this producer.
- `capacity`: explicit nonnegative `cargo_additional_bytes` and
  `capture_additional_bytes`, selected for this warm build. Preflight records
  free bytes on each actual filesystem and includes the signed-source capture
  size. Capture additionally requires the two actual output sizes. There is no
  release-profile 8 GiB floor or regression gate.

The producer holds the maintained Cargo lane, fixed source lane and attempt
flocks. Git-object capture creates read-only source files and directories and
binds its sole `target` symlink to the approved warm target. Maintained Cargo
fingerprint admission retains foreign local-source metadata outside Cargo's
lookup namespace; compiled outputs and dependency caches remain available.
The isolated Cargo home admits only caches and cache locks, excluding Cargo
configuration and credentials. Rust's reported host and release must match the
native ARM target and pinned captured toolchain.

The actual command uses `cwd=/`, the explicit captured `.cargo/config.toml` and
manifest, the warm `--target-dir`, `--locked --offline`, and just the two
package/bin pairs. Cargo's default profile is `dev`; no cross target or release
profile is requested. Original JSON stdout and diagnostic stderr are retained
separately. Before compilation, the retained offline Cargo metadata selects one
unique local workspace package ID for each carrier. The producer binds its
manifest, inherited workspace version/edition, explicit bin declaration and bin
source bytes to the authenticated frozen snapshot. Ownership follows this graph
instead of an assumed package-directory layout. `cargo-owner-graph.json` binds
the original metadata SHA256 and signed source hashes. Successful bin emissions
must match those exact package IDs, manifests, target source paths and features,
and both approved executable paths, with one successful build-finished record.
Library emissions sharing the CLI name are skipped. No artifact selector can
run without this source-bound graph; each retained binary must be an
AArch64 ELF executable with stable bytes and custody. Existing Cargo hardlink
aliases remain untouched.

`request.json`, `started.json`, `cargo-exit.json`, and the original streams record
actual inputs and outcomes. Native key, signature, toolchain and package probes
retain their own exact request, start, streams and exit records. A successful
`manifest.json` records the source/tree/signer/Cargo.lock/controller joins,
nonsecret environment, tool identities, actual command/exit, source invariance
and artifact paths/SHA256/sizes. `profile=dev`, `checks_run=false`,
`native_release_qualified=false`, `application_ready=false`, and `deployed=false`
are unconditional. These observations confer no rollout or update authority.

The current attestation body requires a nonzero `observed_at_unix_ms`, sampled
from the reporting node's software wall clock immediately before signing. Its
signature binds that reading separately from the certified block timestamp.
An older carrier that omits this field cannot supply current helper health
observations. A source capture or completed build does not establish that the
installed validators serve the new body; actual deployment and fresh signed
observations must establish that separately. There is no missing-field decoder
or clock value inferred from a retained block.

Failures retain `failure.json`, original streams and any partial capture, with
no artifact manifest. An interrupted observer never signals Cargo or rustc;
the actual child inherits its lane locks and original log descriptors. Its
unknown exit cannot be relabelled as completion. Wait for that child to finish
before retrying the warm lane; do not delete locks or kill another build.
