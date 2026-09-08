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

The command reports each stage and its elapsed time. An early-gate failure stops
before Linux compilation. A compiler failure retains cargo.log in the private
output directory. Changed source or tools, missing/wrong-architecture binaries,
and artifacts replaced during capture cannot publish result.json. Existing
output directories are never reused: inspect a failed attempt and choose a fresh
output directory for the next attempt while retaining the same warm Cargo lane.

Successful output contains bin/ with four 0500 executable copies, a 0400 cargo.log,
and a 0400 result.json with source/tool/artifact hashes and stage durations. The
capture and output directories are 0500. The mutable Cargo binaries remain in
place. The local result is explicitly not release qualification and is not an
authenticated prebuilt-provenance manifest accepted by run_release_pipeline.py.

Canonical release signing remains in run_release_pipeline.py. Controller import
and native public-reset source-manifest, assemble, authorize, preflight and apply
remain separate operations under their existing authority. This helper accepts
no runtime keys, tokens, SSH, import, activation or publication options.

Validate the local orchestration without Cargo or network:

    python3 -B -m unittest discover -s pytests/scripts -p 'test_taira_release*.py'

The gate's existing selection and diagnostics are documented in
[Taira CLI release checks](taira_release_check.md).
