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
Linux runtime qualification. Changing the recipe does not itself qualify or
select a rebuilt image.

The image workflow passes the hosted runner's actual non-root UID/GID into the
existing builder arguments. This keeps GitHub's mounted workspace and command
files writable without running the build or custody tests as root. A self-hosted
consumer must use matching mount ownership, as described in
[`specs/docker_build.md`](../specs/docker_build.md).

The image workflow tests each validated candidate digest before promotion. A
separate Grype job rejects high or critical known vulnerabilities and retains its
scan. A native Linux container job uses that same digest to exercise installed
tools, filesystem custody tests and strict Clippy, primitive tests, and workspace
formatting. These checks establish component/tooling evidence; full release and
network qualification remain separate. Both jobs must pass before the owner
selects the candidate, and the workflow never changes `IROHA_CI_IMAGE` itself.

The final CI image contains Docker client tools only; the mounted host socket owns
engine execution. Buildx 0.38.2 is rebuilt from its hash-pinned upstream archive
with Go 1.27.3 and the reviewed module locks in `scripts/ci_image_buildx.go.*`.
The locks update `golang.org/x/net` to 0.60.0 and `moby/go-archive` to 0.3.0.
The adjacent test-only patch requires invalid IDNA input to be rejected under the
updated library. The build runs upstream unit tests and retains the compiled
module versions and package import graph in the image. Docker CLI and Compose
come from an isolated Arch package stage; the engine and containerd never enter
the final image.

Scanning retains both raw and qualified JSON reports. The latter applies only a
candidate-digest-bound VEX generated after checking the image's exact inventory:

| Finding | Exact component | Evidence |
| --- | --- | --- |
| AVG-2901 / CVE-2025-6020 | Arch pam 1.7.3-1 | [Linux-PAM 1.7.1 fixed it](https://github.com/linux-pam/linux-pam/releases/tag/v1.7.1); [Arch's group](https://security.archlinux.org/AVG-2901) still has an unknown fixed version. |
| AVG-2898 | Arch libxml2 2.15.4-1 | [2.14.5 fixed the Schematron and shell issues](https://raw.githubusercontent.com/GNOME/libxml2/v2.14.5/NEWS) collected by [Arch's group](https://security.archlinux.org/AVG-2898). |
| GO-2026-4887 | Buildx's docker/docker 28.5.2 module | The retained compiled import graph must contain only `pkg/namesgenerator` from this module. The [vulnerable Engine AuthZ handler](https://github.com/moby/moby/security/advisories/GHSA-x744-4wpc-v9h2) is absent. |

A package, toolchain or import change refuses VEX generation and requires renewed
review. These statements do not waive vulnerable versions or apply to the host
Docker engine. Unknown findings still face the unchanged high/critical gate.
