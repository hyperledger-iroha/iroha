# IVM CUDA PTX artifacts

The CUDA source kernels in this directory are optional acceleration paths. Their
outputs must remain bit-identical to the scalar implementation; the PTX build
path must never silently substitute a placeholder.
The build script pins exactly ten production `.cu` families and rejects missing
or extra sources. A deleted kernel cannot silently shrink the bundled artifact
inventory.

`IVM_CUDA_PTX_MODE` selects the build-time artifact policy:

- `bundled` (default) verifies the signed ten-family manifest and copies its
  already-verified `cuda/<kernel>.ptx` bytes into Cargo's output directory.
  Missing, altered, or structurally invalid PTX fails the build. This path
  does not invoke `nvcc` or require a CUDA driver.
- `generate` explicitly invokes `nvcc` for every `.cu` source and writes PTX
  only into Cargo's output directory. This mode is for qualification runners,
  not ordinary or release builds; every non-debug profile, including `deploy`,
  rejects it.
- `check` verifies the signed bundle, checks the exact `nvcc --version` output,
  flags and target against the manifest, invokes `nvcc`, requires every
  generated file to be byte-identical to its signed counterpart, then installs
  the verified bytes.

Generation and checking use `IVM_CUDA_NVCC` (or `NVCC`),
`IVM_CUDA_GENCODE`, and `IVM_CUDA_NVCC_EXTRA`. The default generation target is
`arch=compute_86,code=sm_86`, matching the current CUDA hardware lane. A release
artifact still requires a pinned CUDA image and toolchain; the default alone is
not provenance.

`bundled` and `check` also require `IVM_CUDA_TRUSTED_KEY_SHA256`: a reviewed,
lowercase SHA-256 fingerprint of the raw Ed25519 public key. This is a release
build trust input, not a runtime feature switch or a value copied from the
bundle. The fixed-order LF-terminated `cuda/provenance.v1` contains the pinned
CUDA image SHA-256, SHA-256 of exact `nvcc --version` output, exact compiler
flags, target profile, two independent-generation digests, and source/PTX
SHA-256 pairs for all ten families in the build-script order. Its signature is
`cuda/provenance.v1.sig` (64 raw bytes); the corresponding public key is
`cuda/provenance.v1.pub` (32 raw bytes). The build verifies that signature and
fingerprint before reading artifacts. Symlinks, extra manifest fields, changed
source/PTX bytes and divergent generation digests are rejected.
The build environment receives this fingerprint from the release producer.
The release pipeline requires its independently reviewed value explicitly via
`--trusted-cuda-key-sha256`, and binds it plus the exact source manifest digest
in the single authenticated prebuilt V1 `acceleration` record. Setting an
environment value alone does not qualify a signer or artifact.
The build exposes the signed `cuda_image_sha256` claim as
`IVM_CUDA_SIGNED_IMAGE_SHA256` to the compiled IVM crate. A release runner must
compare it with an independently measured, pinned toolkit image; this build
value alone is not an image attestation.

The generation digest is SHA-256 over
`ivm-cuda-ptx-generation-v1\0` followed, in pinned family order, by each
stem's little-endian `u16` byte length, stem bytes, little-endian `u64` PTX byte
length, and PTX bytes. The signed two-run values must both equal this digest.
This checks the signed claim against the bundle; the release runner must also
retain evidence that two clean builds used the pinned image and really produced
those bytes. The `check` mode verifies one local regeneration; it is not a
substitute for two-run evidence or GPU parity.

Examples:

```sh
IVM_CUDA_PTX_MODE=generate cargo build -p ivm --features cuda
IVM_CUDA_TRUSTED_KEY_SHA256=<reviewed-fingerprint> IVM_CUDA_PTX_MODE=check cargo build -p ivm --features cuda
```

## Reproducible candidate generation

`scripts/build_ivm_cuda_bundle.py` is the offline candidate producer. It reads
the ten pinned `.cu` files into one source snapshot, compiles every family in
two clean output directories with identical exact flags, rejects divergent PTX,
and publishes a fresh candidate directory only after signing the fixed-order
manifest and checking the retained records. The original private staging and
parent directory descriptors remain held throughout generation. Publication
uses Linux/macOS atomic no-replace rename; an existing destination, including an
empty directory, is never replaced. Unsupported hosts fail closed. It does not
run during Cargo builds or node startup. The signer key
must be supplied explicitly as a PEM file outside the repository and is never
copied into the candidate. The tool requires Python 3.10+, a pinned `nvcc`, and
OpenSSL with Ed25519 support. It never downloads a toolkit or a kernel.

```sh
python3 scripts/build_ivm_cuda_bundle.py \
  --nvcc /absolute/path/to/pinned/nvcc \
  --openssl /absolute/path/to/openssl \
  --cuda-image-sha256 <independently-measured-image-sha256> \
  --signing-key /private/path/outside/repository/ed25519.pem \
  --output-dir /private/staging/ivm-cuda-candidate
```

An absent checkout-local path such as `target/ivm-cuda-candidate` is also
supported; source-directory destinations and symlink ancestors are rejected.
Failed runs retain their private `.ivm-cuda-incomplete-*` staging directory for
inspection and do not publish the requested candidate path.

The script prints the candidate path, raw-public-key fingerprint, and common
two-run generation digest. On Linux it mirrors `build.rs`'s default `g++-12`
selection; `--host-compiler` and repeated `--extra-flag` inputs are available
when the pinned qualification image uses a different exact compiler command
(`--extra-flag=--fmad=false` for a flag beginning with `-`).
The output contains the ten source snapshots, ten PTX files,
`provenance.v1`, `provenance.v1.sig`, `provenance.v1.pub`, and `evidence/`.
The evidence retains both clean output trees, exact compiler commands and
stdout/stderr, source/output hashes, executable hashes, and the exact version
output. Every child retains the original launcher/tool file descriptors and
no-follow ancestor descriptors, including directory change timestamps; swapping
and restoring a compiler ancestor causes publication to fail even when the
original executable bytes are unchanged. Each tool has a twenty-minute deadline
and a combined 4 MiB log bound;
each source snapshot is limited to 16 MiB and each PTX to 8 MiB. The evidence
record is bounded to 128 KiB. Empty stdout/stderr retain normal descriptor custody
and exact zero-byte hashes. The private key is never copied; signer command
records redact its runtime path. Executable/source/output records are local
observations; the supplied CUDA image digest remains explicitly an unverified
claim until the independent runner check. The Ed25519
signature covers the exact LF-terminated manifest bytes, while the manifest
binds the independently supplied image digest, exact `nvcc --version` bytes,
flags, target, two PTX-generation digests, and every source/PTX digest. The
signing preimage is inspectable directly as `provenance.v1`; the generation
preimage is specified above and reproduced in the script tests.

Before promoting a candidate, reviewers must measure and pin the actual CUDA
image independently, approve the signer fingerprint as a release build input,
retain the two clean-run logs and candidate digest inventory, and qualify each
kernel on the intended GPU/driver profiles. Copying the candidate into this
directory and setting `IVM_CUDA_TRUSTED_KEY_SHA256` before those checks would
only make the build admit a self-consistent signature; it would not establish
release qualification. Use `IVM_CUDA_PTX_MODE=check` to reproduce the reviewed
PTX bytes with the same toolkit and flags, followed by bundled-mode hardware
tests. Canonical Linux shipping producers require the CUDA feature and signed bundle;
they fail closed until these inputs and tests exist. Ordinary development Cargo
builds without CUDA remain independent of the toolkit and driver.

## Release blocker

TODO: reproducibly generate and check in all 10 real artifacts:
`aes.ptx`, `bitonic_sort.ptx`, `bn254.ptx`, `poseidon.ptx`,
`sha256.ptx`, `sha256_leaves.ptx`, `sha256_pairs_reduce.ptx`, `sha3.ptx`,
`signature.ptx`, and `vector.ptx`.

TODO: check in the signed manifest, raw signature and reviewed public key
fingerprint from release signing infrastructure. The verifier and exact
source/PTX admission gate are implemented, but no production signature or
two-run evidence exists yet. Until both TODOs are closed, the default `bundled`
CUDA build intentionally fails closed.

All kernels still require real-hardware scalar parity, malformed input, and
failure-path qualification. The artifact inventory contains only integer and
field kernels.

## Required hardware qualification

Run `cargo test --locked -p ivm --test cuda_hardware --features
cuda-hardware-tests -- --nocapture` on each qualified CUDA runner. This separate
process compares all ten kernel families with scalar references on every usable
discovered device. Missing hardware, unavailable kernels, CPU fallback, and zero
completed kernel batches fail the test. `IVM_CUDA_RECEIPT` lines record completed
kernel batches on the calling thread; admission self-tests and memory transfers
do not count. A receipt must accompany matching output, not replace it.

The nightly workflow first runs this test against a generated diagnostic
candidate. Its separate release gate requires `check` mode to reproduce every
checked-in PTX byte and then reruns hardware qualification in `bundled` mode.
That release gate requires three reviewed repository variables:
`IVM_CUDA_QUALIFICATION_IMAGE` (a repository reference ending in `@sha256:...`),
`IVM_CUDA_IMAGE_SHA256` (the independently reviewed image manifest digest), and
`IVM_CUDA_TRUSTED_KEY_SHA256` (the reviewed raw public-key digest). Missing values
fail before pulling or building. The runner needs Docker with GPU access and a
pinned image containing the matching CUDA, host compiler, and Rust build tools.
It pulls the immutable reference, retains Docker's actual image inspection, and
uses `scripts/check_ivm_cuda_release_image.py` to bind that observation to both
reviewed digests and the source manifest's image claim. Builds and hardware tests
then execute using the inspected image configuration ID. An ambient host key or
the manifest's own image claim cannot supply the reviewed inputs. This input
receipt explicitly does not verify the bundle signature or qualify hardware;
the mandatory Rust build and actual kernel tests still perform those checks.
No reviewed CI values, production bundle, or physical run is supplied by these
source changes.
Missing checked-in artifacts fail that gate. The emitted SHA-256 inventory and
hardware logs are evidence inputs, not a substitute for the signed provenance
manifest above. The `cuda-hardware-tests` feature exposes device-selection hooks
for this dedicated qualification process and must not ship in node builds.

Device discovery skips devices whose context cannot be initialized and applies
`[accel].max_gpus` to usable devices. Each context owns admission and quarantine
state for its production kernels. Golden tests run on the exact selected device
and must complete real kernel work before that kernel is admitted. A parity mismatch
or backend failure during admission or execution excludes that device/kernel pair;
other kernels and devices remain candidates. Local capacity exhaustion, owner-lock
contention, and temporary unavailability leave the exact artifact unqualified and
eligible for another public self-test. No result from a deferred self-test is
published, and a concurrent quarantine always wins over that deferral. Context and
stream faults exclude the affected device.
Task assignment uses operation identity and public workload dimensions only;
operand limbs, digest state, and signature bytes do not select a device.
Task scopes pin their manager generation and device so nested dispatch cannot
switch owners while buffers are live. A timed-out device retains its context and
allocations without retaining healthy device owners. Rebuilt contexts receive
fresh admission state; existing borrowers retain their original state.

TODO: qualify this admission and quarantine matrix on real multi-device hosts,
including injected per-kernel failure, context failure, and stream timeout, before
claiming hardware readiness. Driverless policy tests establish state transitions
and selection behavior, not successful GPU execution.

The patched `cust_raw` boundary resolves driver entry points at runtime and keeps
the library alive for the process lifetime. CUDA-enabled binaries have no native
CUDA link dependency. Linux uses the system `libcuda.so.1` loader path (including
the WSL system directory); Windows restricts `nvcuda.dll` lookup to System32.
Missing drivers and missing symbols return CUDA errors and preserve CPU fallback.
The driverless CI job exercises real dynamic loading with a small test library,
builds `cust`, and proves that public bindings start without a driver. See
[`vendor/cust_raw/UPSTREAM.md`](../../../../vendor/cust_raw/UPSTREAM.md).

TODO: qualify complete ordinary default-target packaging after the PTX and signed
provenance gates pass. Do not enable default CUDA before those artifacts exist.
