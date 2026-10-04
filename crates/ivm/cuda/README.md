# IVM CUDA PTX artifacts

The CUDA source kernels in this directory are optional acceleration paths. Their
outputs must remain bit-identical to the scalar implementation; the PTX build
path must never silently substitute a placeholder.
The build script pins exactly ten production `.cu` families and rejects missing
or extra sources. A deleted kernel cannot silently shrink the bundled artifact
inventory.

The private source-owned `REVIEWED_CUDA_BUNDLE_PINS` declaration in
`src/cuda_build_policy.rs` is the only approval descriptor. Its two public pins
are the independently reviewed nonzero lowercase SHA-256 of the raw Ed25519
public key and exact canonical authenticated V1 manifest. The current descriptor
is `None`; there is no authentic approved production bundle.

Ordinary Cargo validates the ten-source inventory and distinguishes genuine
absence from supplied material. `None` with all thirteen fixed PTX/provenance
paths absent emits an absent owner and preserves CPU-capable builds in every
profile. Any supplied unreviewed, partial, malformed, unexpected or mismatched
material fails integrity. Future genuine approval requires the complete exact
bundle. Cargo and startup never invoke `nvcc` or select trust through environment
values. Linux/Windows daemon dependencies automatically select the existing IVM
CUDA feature; macOS retains its Metal graph.

The sole canonical V1 verifier authenticates held manifest/key/signature before
artifact traversal. The LF-terminated manifest retains the pinned image claim,
exact `nvcc --version` identity, flags, target, two generation digests and all ten
source/PTX hash pairs in fixed order. The raw signature is 64 bytes and public
key 32 bytes. No-follow stable file custody, bounds, canonical fields, actual
hashes and both generation digests remain mandatory. The exact manifest pin is
checked after the original canonical relation. Build and immutable runtime
adapters share this relation; there is no second codec or decoder.

The runtime owner retains the original exact immutable bytes and admits them
before physical discovery, private staging or native completion. A missing bundle
maps to existing unavailable/CPU behavior, with no hardware, self-test or cost
credit. Signature authenticity and the source image claim do not independently
measure a toolkit image or establish device qualification.

Python release owners read only the closed source literal through stable source
custody, derive both expected pins from it and compare retained source/prebuilt
identities. They do not authenticate a second PTX/signature format. Docker and Nix
perform a thirteen-file regular/non-symlink inventory preflight only; Rust owns
cryptographic admission. Shipping CUDA rejects current absence. The closed
prebuilt V1 `acceleration` record retains its exact three fields and macOS
no-CUDA invariant.

The generation digest is SHA-256 over
`ivm-cuda-ptx-generation-v1\0` followed, in pinned family order, by each
stem's little-endian `u16` byte length, stem bytes, little-endian `u64` PTX byte
length, and PTX bytes. The signed two-run values must both equal this digest.
This checks the signed claim against the bundle; the release runner must also
retain evidence that two clean builds used the pinned image and really produced
those bytes. An explicit offline reproduction is not a substitute for the
independent two-run evidence or GPU parity.

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
two-run generation digest. The offline producer retains its explicit Linux
`g++-12` default; ordinary Cargo has no CUDA compiler selection. `--host-compiler` and repeated `--extra-flag` inputs are available
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

Before promoting a candidate, reviewers must independently measure and pin the
actual toolkit image and tools, approve the signer and exact authenticated
manifest, retain two clean-run evidence and qualify every kernel/device profile.
Only a reviewed source change can install genuine `Some(ReviewedCudaBundlePins
{ ... })` approval; copying producer output or its key fingerprint cannot do so.
The source descriptor stays `None` until those gates are satisfied.

## Release blocker

TODO: reproducibly generate and check in all 10 real artifacts:
`aes.ptx`, `bitonic_sort.ptx`, `bn254.ptx`, `poseidon.ptx`,
`sha256.ptx`, `sha256_leaves.ptx`, `sha256_pairs_reduce.ptx`, `sha3.ptx`,
`signature.ptx`, and `vector.ptx`.

TODO: check in the signed manifest, raw signature and reviewed public key
fingerprint and exact authenticated manifest pin from release signing infrastructure. The verifier and exact
source/PTX admission gate are implemented, but no production signature or
two-run evidence exists yet. Ordinary absence remains CPU-capable, while
shipping and required-hardware CUDA qualification fail closed.

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

The nightly workflow qualifies only the source-owned bundle; `None` fails the
required-hardware test. The separate release gate requires the independently
reviewed `IVM_CUDA_QUALIFICATION_IMAGE` digest-pinned reference and
`IVM_CUDA_IMAGE_SHA256`, then binds Docker's actual inspection to the source
approval through `scripts/check_ivm_cuda_release_image.py`. It executes the
inspected image configuration ID. Explicit pinned nvcc/OpenSSL paths and an
external runtime-only signing-key path feed the sole offline producer for two
clean-run reproduction. Its fresh candidate is compared byte-for-byte with the
approved source bundle and never installed or used to approve itself. No ordinary
Cargo compiler mode, ambient trust value or producer receipt substitutes for
source review, actual image measurement or kernel completion. This input
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
