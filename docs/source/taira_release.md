# Local Taira release preparation

Run the native CLI checks, build the four AArch64 Linux executables, and capture
read-only copies through one maintained command. This replaces per-release local
build and capture scripts. Python 3.11+, Git, the repository Rust toolchain,
cargo-zigbuild, Zig and an existing warm Cargo target directory are required.

Run only the early native gate:

    python3 scripts/taira_release.py check

Prepare binaries from the exact signed HEAD on optimizations:

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
Git replacement refs are disabled. Every `BUILD_SOURCES` controller file is checked
against its signed Git blob and executable mode, independently of the mutable
index and flags that conceal local edits. Imported controller modules must come
from those checked paths; additional checkout modules cannot enter the controller.
Unrelated staged, unstaged and untracked work stays untouched and is never copied
into the build. Gitlinks retain their signed mode and commit in the capture and
remain empty there; worktree submodule contents are excluded. There are no
embedded tool digests or release-number-specific paths to update.

Before fresh preparation starts, the optimizations HEAD, its signature and the
controller sources are checked.
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
commit, the controller files against that commit, and the captured bytes. Unrelated
HEAD advancement is allowed on optimizations; changes to controller files require
a new preparation for their signed commit.

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

`check` defaults to the existing sibling `.taira-testnet-build-targets/routine`
development lane. Set `TAIRA_TESTNET_CARGO_TARGET_DIR` or `--target-dir` to select
another established development lane; when both are set, they must agree.
`prepare` defaults to the checkout's existing `target/` release lane and ignores
the development-only environment override. Its `--target-dir` must select an
established release lane. The commands enforce separate lane ownership: mutable
development checks cannot use a captured release lane, and authenticated
preparation cannot use the routine development lane. Preparation uses the
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

After verifying the complete native Cargo output set, a bounded owner-private
ledger records only final test executables and retires recorded superseded outputs
before checking copy capacity. Retirement runs under Cargo's locks after exact
inode checks and an OS open-file check; a later copy failure leaves the current
verified Cargo outputs intact.
The first run only records current outputs; unrecorded files, production binaries,
libraries, object files and warm compiler caches are retained. Busy files or an
unavailable/inconclusive `lsof` check cause retention. A private quarantine closes
the old pathname before the final open-file check; interrupted retirement remains
recorded. Retries recover both rename windows using the recorded inode and stable
metadata, recheck open-file status, and never adopt an unrelated replacement. A
full ledger retries pending cleanup before admitting successors, so closing an
old reader restores progress without manual ledger edits.

Temporary executable copies, including non-CLI native network binaries, are
released after their last child exits, on success or failure. The published native
`iroha` CLI snapshot remains available to operator custody and deployment consumers;
the `cli` test harness is temporary. Exact identity and closed-file checks preserve
replaced or busy copies. If isolation fails partway through a batch, only already
verified copies are cleanup-owned, including an unpublished CLI; incomplete or
unrecorded files are retained. Observations and fixture logs remain. Cargo producers
and Linux release artifacts are outside temporary-copy cleanup.

Before Cargo, the gate compiles the dependency-free consensus reducers and the
shared lifecycle source assertions directly with the pinned Rust compiler. Both
must execute every listed test without skips. Lifecycle mutation controls check
that removing or reordering required retries still fails. The gate also reconciles
the shipping binary table with Cargo manifests and the early compilation targets.
Configuration library and integration tests, CLI, SDK, Torii, crypto, P2P, Core, proof and fixture harnesses,
including all four shipping entry points, then share one Cargo invocation,
resolving the union of their existing default features. Configuration runs first
and fails immediately, including when an independent-test checkpoint can be reused.
CLI and the canonical Kagami projection checks precede the proof, crypto,
transport, consensus and fixture selections. Every independent failure stops
before daemon startup. Shipping targets without selected tests provide actual
compilation evidence, with no invented test passes. The native production build
uses the same four shipping packages and binaries, plus the ordinary `iroha3d`
fixture launcher; the four-peer test continues to launch that ordinary binary.
The Linux release command and its production features remain unchanged. Proof
harnesses are not rebuilt separately after the network test. This reduces repeated
dependency work; it does not promise a fixed build duration.
The fixture checks read the public Taira Nexus profile without runtime inputs
and verify that collection decoding preserves declared configuration defaults
while rejecting malformed values.

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

Before initial source capture, local admission counts the signed Git blobs' exact
byte sizes without reading unrelated worktree files.
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

    PYTHONPATH=scripts python3 -B -m unittest discover -s pytests/scripts -p 'test_taira_release*.py'

The gate's existing selection and diagnostics are documented in
[Taira CLI release checks](taira_release_check.md).

## Updating an initialized testnet

For a routine update of the existing four-validator Taira installation, use the
completed basic preparation directly:

    python3 scripts/taira_update.py \
      --deployment /absolute/owner-private/taira/deployment.json \
      --prepared-result /absolute/completed-preparation/result.json \
      --output /absolute/owner-private/taira/update-output

The deployment record contains the approved SSH route and public host-key pins,
network and directory identities, and the exact completed predecessor receipt.
Keep it outside Git. The updater transfers the prepared daemon and matching CLI,
preserves configuration, signer custody and ledger state, and verifies native
Strict snapshot restoration and public basic health. It does not invoke Cargo.
`--plan-only` writes the concrete plan locally without contacting the host.

Local and guest locks serialize updates. Each operation retains its own staging
and evidence paths, so a failed transfer or lock conflict can use the same
completed binaries in a fresh operation after inspection. Failed stages remain
on disk. An interrupted runtime mutation requires recovery before another update;
there is no automatic rollback to earlier execution rules after candidate start.
A successful update emits `next-deployment.json` for the next invocation. Confirm
an application transaction as state-resolved Applied after installation.

Validate this controller without Cargo or network:

    python3 -B scripts/tests/taira_update_test.py

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
