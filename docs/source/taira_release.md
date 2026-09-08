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
be uninitialized empty directories or absent. Populated submodules are rejected
before checks or compilation. There are no embedded tool digests or release-number-specific paths to update.

Both commands default to the checkout's existing target/ directory. Supply
--target-dir only to select another established warm lane. Preparation uses the
unchanged release profile, six jobs, and the maintained Zig wrapper for exactly
iroha3d_taira, iroha, sorafs-node and kagami. It never cleans the target or changes
source. Compiler overrides, interpreter hooks and runtime credentials are not
forwarded to the child environment; local Cargo/Rustup and sccache paths remain
available. The gate receives the same explicit target directory.

Rerun the exact same `prepare` command and output directory after interruption.
The command locks that owner-private preparation directory, checks that its
recorded inputs still match the signed checkout and tools, and resumes locally:

- Completed native checks are reused for those exact inputs.
- A failed or interrupted build runs Cargo again in the same warm target. Cargo
  reuses its cache; each attempt gets a fresh private log and capture directory.
- A completed read-only capture is revalidated and reused without running checks
  or Cargo, including a crash before the final result was published.
- Changed input identity or a modified captured binary stops with a specific
  error. Active preparations retain their lock; a second command stops promptly.

The command reports stage durations and emits elapsed time, compiler-output size,
and the log path every 30 seconds during Linux compilation. It does not repeatedly
hash source or artifacts while reporting progress and does not impose an arbitrary
cold-build deadline. Source, tools and artifacts are checked at actual consumption
and reuse boundaries. Runtime secrets remain excluded from the child environment.

Before compilation, local admission groups requirements by filesystem and checks
an 8 GiB Cargo working-space floor plus 256 MiB capture headroom. Before capture,
it checks the exact binary-copy bytes plus that headroom. The build floor is an
operational minimum, not a prediction of Cargo's peak use. This local check cannot
observe a remote guest's sparse backing disk; native host capacity admission and
operator backing-volume checks remain necessary. No cache or output is deleted
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
