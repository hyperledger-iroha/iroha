# CI image contract

All managed Rust CI containers receive `IROHA_CI_IMAGE` through the hosted
`ci_image_preflight.yml` job. The variable must contain an immutable
`repository@sha256:<64 lowercase hex digits>` reference. Missing or mutable
references fail before the dependent container starts. There is no tag fallback.
Digest validation establishes identity, not vulnerability status.

`ci_image.yml` builds `Dockerfile.build` from the separately reviewed,
digest-pinned `IROHA_CI_BUILDER_IMAGE`. It emits a commit-tagged candidate, its
digest, provenance and SBOM. The recipe pins Rust 1.99.1, verifies the official
grcov 0.10.8 Linux archives, and installs Poetry 2.5.1 from the Linux,
hash-locked `scripts/ci_image_poetry.lock` in an isolated Python 3.14 environment.
Arch packages are updated during the build and their exact versions are retained
in `/usr/local/share/iroha-ci/arch-packages.txt`; rebuilding is not claimed to
reproduce those rolling repository inputs.

Regenerate the Poetry lock with the command recorded in its header. Qualify its
binary-wheel resolution for both supported Linux architectures and scan every
locked package. Linux-targeted resolution excludes browser/Wasm adapters;
the x86_64 and aarch64 package/version sets must match.

Before promoting a candidate digest to `IROHA_CI_IMAGE`, the image owner must
build and scan those exact Linux image bytes and run the repository's CI jobs
against them. A recipe edit or successful digest preflight does not establish
Linux runtime qualification. The historical May 2025 image is not represented
as a current rebuilt image, and this change does not publish or select a new
image on the owner's behalf.
