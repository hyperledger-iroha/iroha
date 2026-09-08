# Local Taira release preparation

Run the native CLI checks, build the four AArch64 Linux executables, and capture
read-only copies through one maintained command. This replaces per-release local
build and capture scripts. Python 3.11+, Git, the repository Rust toolchain,
cargo-zigbuild, Zig and an existing warm Cargo target directory are required.

Run only the early native gate:

    python3 scripts/taira_release.py check

Prepare binaries from an exact signed, clean commit on optimizations:

    python3 scripts/taira_release.py prepare \
      --expected-commit FULL_SIGNED_COMMIT \
      --expected-signer REVIEWED_SIGNING_KEY_FINGERPRINT \
      --output-dir /absolute/private/output-for-this-build \
      --zig /absolute/real/zig \
      --zig-sha256 REVIEWED_ZIG_SHA256 \
      --cargo-zigbuild /absolute/real/cargo-zigbuild \
      --cargo-zigbuild-sha256 REVIEWED_CARGO_ZIGBUILD_SHA256

Use independently reviewed tool digests and the full signing-key fingerprint
(GPG uppercase hexadecimal, or SSH SHA256 form). A valid signature from another
locally known key is rejected. Paths must be absolute and contain no
symlinks; PATH must resolve cargo-zigbuild to the supplied executable. Source
verification binds the maintained helper scripts to the supplied signed commit.
Git replacement refs are disabled. Every tracked file is checked against its
indexed blob and mode even when Git index flags conceal local edits.
Gitlinks retain their exact indexed mode and commit; their worktree paths must
be uninitialized empty directories or absent. Initial admission rejects populated worktree submodules; captured gitlinks always
remain empty. There are no embedded tool digests or release-number-specific paths to update.

Before preparation starts, the clean checkout and its signed commit are checked.
The command then reads that commit's Git objects into a private, read-only source
capture under the selected Cargo target's `taira-release-sources/`. It creates no Git repository,
worktree or branch. Both native checks and the Linux build consume this capture;
One explicit `source/target` binding points to the selected existing warm Cargo
target for native fixture output; source inventories verify this binding without
traversing generated files. Native fixture processes also run from that external
target directory. The snapshot covers signed Git entries; the output binding is
recorded separately as `source_output_target`. Every signed source path remains
read-only. Subsequent
checkout edits, merges or HEAD changes cannot mix source versions into
the build. Native test selection is loaded from the captured gate helper, including
a resumed check after the checkout has changed. Resume authenticates the recorded
commit and captured bytes without
requiring the working checkout to remain unchanged.

Each selected warm Cargo lane has one stable source path. A lane-wide lock covers
capture refresh, native checks, Linux compilation and artifact capture, including
an active child if its launcher exits. Source replacement occurs only between
preparations. Unchanged files retain their timestamps so routine releases can
reuse Cargo's cache. Interrupted source publication is recoverable; previous
source directories and unfinished copies remain retained alongside the current
capture.

Cargo can retain dependency records pointing at an older source directory even
when the selected manifest changes. Before compiling, preparation inspects local
package records under Cargo's profile locks. Native checks inspect only the
debug profile family; Linux release builds inspect only the release profile
family, including their host build-script records. One family's admission never
retires the other's reusable fingerprints. Records for another source tree are
retained under `taira-release-cache-retired/`, outside Cargo's active fingerprint
lookup. Compiled outputs and registry/Git caches remain in place. Current-source
records are reused. After building, the same admission must pass without repairs;
Cargo's profile locks remain held through artifact capture. The regression suite
reproduces the stale host build script with two real source directories and proves
the corrected build executes the new source.

Both commands default to the checkout's existing target/ directory. Supply
--target-dir only to select another established warm lane. Preparation uses the
unchanged release profile and six jobs for exactly iroha3d_taira, iroha,
sorafs-node and kagami. Preparation selects the Rust toolchain from the captured
`rust-toolchain.toml`, then runs Cargo from `/` with the captured manifest and
`.cargo/config.toml` explicitly selected. A persistent config-free Cargo home
under the project target reuses the existing registry and Git caches. Ancestor
and home Cargo configuration cannot override these inputs; a root-level Cargo
config stops preparation with an actionable error. The captured Zig driver and
installed sccache remain in use. No target or cache is cleaned or replaced.
Compiler overrides, interpreter hooks and runtime credentials are not forwarded.
The native gate receives the same source, toolchain and explicit target directory.

Rerun the exact same `prepare` command and output directory after interruption.
The command locks that owner-private preparation directory, checks that its
recorded inputs still match the fixed signed source capture and tools, and resumes locally:

- Completed native checks are reused for those exact inputs unless foreign
  local-package fingerprints had to be retired.
- A failed or interrupted build runs Cargo again in the same warm target. Cargo
  reuses its cache; each attempt gets a fresh private log and capture directory.
- A completed read-only capture is revalidated and reused without running checks
  or Cargo, including a crash before the final result was published.
- Changed input identity or a modified captured binary stops with a specific
  error. Active preparations retain their lock; a second command stops promptly.

The command reports stage durations and emits elapsed time, compiler-output size,
and the log path every 30 seconds during Linux compilation. It does not repeatedly
hash source or artifacts while reporting progress and does not impose an arbitrary
cold-build deadline. Captured source, tools and artifacts are checked at actual consumption
and reuse boundaries. Runtime secrets remain excluded from the child environment.

Before initial source capture, local admission includes its exact file bytes.
Before compilation, it groups requirements by filesystem and checks an 8 GiB
Cargo working-space floor plus 256 MiB capture headroom. Before capture,
it checks the exact binary-copy bytes plus that headroom. The build floor is an
operational minimum, not a prediction of Cargo's peak use. This local check cannot
observe a remote guest's sparse backing disk. Run the deployment capacity check
below on the guest and its backing host. No cache or output is deleted
automatically, and the warm Cargo target is never replaced with a new lane.

The output directory contains read-only `request.json`, `checks.json`, and
`result.json`, a persistent private `session.lock`, and numbered `attempts/`
directories. Failed attempt logs and partial captures stay available. Successful
captures contain four 0500 executables and a 0400 `capture.json`; its artifact paths
are returned in `result.json`. The successful attempt, capture and top-level output
directories are 0500. The mutable Cargo binaries remain in place. Existing unrelated
output directories cannot be adopted as resumable preparations.

This result remains a local build observation with `release_qualified=false` and
`deployed=false`. It is not an authenticated prebuilt-provenance manifest accepted
by `run_release_pipeline.py`, and local resume does not resume a deployment journal.

Canonical release signing remains in run_release_pipeline.py. Controller import
and native public-reset source-manifest, assemble, authorize, preflight and apply
remain separate operations under their existing authority. This helper accepts
no runtime keys, tokens, SSH, import, activation or publication options.

Validate the local orchestration without Cargo or network:

    python3 -B -m unittest discover -s pytests/scripts -p 'test_taira_release*.py'

The gate's existing selection and diagnostics are documented in
[Taira CLI release checks](taira_release_check.md).

## Deployment capacity and retrying an unchanged release

Keep the build identity separate from the deployment attempt. A storage or host
failure does not require another build, source import, or binary transfer when
the retained artifacts still match their successful preparation and transfer
receipts. After a completed native rollback, retain its terminal record and
create fresh deployment custody and authorization for the same artifacts. Never
replay the failed authorization or edit a terminal journal to resume it.

Use the maintained retry command with the retained preparation, transfer and
runtime evidence:

    python3 scripts/taira_retry.py \
      --plan /private/runtime/taira-retry/operator-plan.json \
      --output-root /private/runtime/taira-retry/local-evidence

It allocates a new attempt automatically, preserves completed rollback history,
runs native assembly and authorization, and completes seed continuity and boot
persistence. Repeating it before apply resumes the same operation; after the
durable apply frontier, a real native rollback terminal is required. See
[the retry command](taira_retry.md) for input custody, capacity admission and
public application validation. No numbered retry adapter needs to be edited.

Before preparing a deployment, and again immediately before apply, run:

    python3 scripts/taira_disk_capacity.py --plan /absolute/public-capacity-plan.json

The public JSON plan uses schema `taira.disk-capacity.plan.v1` and an
`allocations` list. Each allocation has exactly `path`, `label`, `bytes`, and
`inodes`. Quantities describe additional allocated space, including concurrently
live temporary files and explicit operating headroom. The helper resolves paths
without following symlinks, groups requirements by actual filesystem, and checks
both available bytes and inodes. It reads no configuration or credentials and
performs no cleanup. Failure returns exit code 2 with required and available
quantities. Success is an observation, not a disk reservation.

For the current four-validator deployment on one physical guest, a fresh attempt
requires `3*A + 2*S + 4*P + 4*R`, plus operating headroom:

- `A` includes the complete artifact sets for all four validators and the edge.
  The coordinator snapshot, host uploads, and installed releases coexist.
- `S` includes the full Inrou stage tree. Its coordinator snapshot and host upload
  coexist with the already retained source tree.
- `P` includes one complete SoraFS store: all three payloads, chunk allocation
  slack, manifest and PoR metadata, index files, and temporary publication files.
- `R` includes one replica's hydrated guest, independent writable root disk,
  application extraction and publication, data lease, ephemeral storage, and
  runtime metadata. All four replicas keep their own runtime files.

The helper's `allocation_bound` and `cohost_peak_plan` functions construct these
byte and inode bounds. See [the allocation contract](taira_disk_capacity.md) for
the persisted metadata limits and backing-volume requirements. Logical SoraFS quotas are not free-space checks. Existing
files are already charged against available space; do not subtract proposed
cleanup until it has actually completed. Check a sparse VM's physical backing
filesystem separately, including its possible additional allocation.

When reclaiming failed attempts, remove only identified obsolete generated
payloads that have no live references and are outside current deployment or
recovery inputs. Hold the deployment coordinator and physical-host action locks.
Retain terminal journals, manifests, cleanup receipts, runtime custody, and the
current artifact set. Repeatedly retaining every large failed upload eventually
exhausts the guest even when the next store fits its logical quota.

Validate the capacity helper without network access:

    python3 -B -m unittest discover -s scripts/tests -p 'taira_disk_capacity_test.py'
