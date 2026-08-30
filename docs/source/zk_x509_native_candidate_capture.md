# Native zk-X509 candidate capture corridor

This corridor produces review material only. It never installs fixtures, edits
the eleven capture-owned source pins, commits, signs, uploads, publishes, or
turns a candidate root into a release root. A candidate is admissible for
review only when it comes from a clean, exact-one SSH-signed source commit and
an authenticated-source Linux AArch64 worker package built from that same
commit with an unchanged tracked `Cargo.lock`.

All executable steps in the packager, capture controller, and IID verifier are
Linux-only. Each bounded command is launched as PID 2 below a trusted,
non-dumpable PID 1 in fresh user and PID namespaces. The target has its
capabilities and capability bounding set cleared and `no_new_privs` locked
before exec. If namespace creation, exact UID/GID mapping, capability removal,
parent-death binding, or the exec handshake cannot be proved, the target is
never executed. There is no macOS, process-group-only, inherited-pipe-only, or
`/proc` descendant-scan fallback. The native-host qualification script probes
the same user/PID namespace and identity-map prerequisites before capture.

## Trust inputs

The operator supplies these values through an authenticated channel outside
the capture host and repository:

- the allowed-signers file and its exact SHA-256;
- the SSH revocation policy and its exact SHA-256;
- the expected signer principal and OpenSSH SHA-256 fingerprint;
- the AWS region and the raw-file SHA-256 of that region's AWS RSA-2048 IID
  certificate;
- the AWS account ID and AMI ID admitted for the capture;
- the exact OpenSSL executable SHA-256.

The AWS certificate must be selected manually from AWS's regional
RSA-2048 instance-identity certificate list. The controller does not download
or update trust roots. Region, account ID, and AMI ID are runtime admission
pins. The authenticated instance ID and availability zone are recorded in the
candidate envelope for manual review; they are deliberately not guessed or
silently admitted as OOB pins.

## Immutable package first

Before capture, build the worker package from the clean signed source using a
fresh owner-private target, cache, and temporary directory outside the checkout.
The directory name should be derived from the reviewed commit, policy digests,
`Cargo.lock` digest, target, and toolchain identity. Then run:

```text
python3 -I -S scripts/package_zk_x509_prover_worker.py build \
  --source-root <canonical-signed-source> \
  --allowed-signers <canonical-oob-file> \
  --allowed-signers-sha256 <oob-sha256> \
  --revocation <canonical-oob-file> \
  --revocation-sha256 <oob-sha256> \
  --signer-principal <oob-principal> \
  --signer-fingerprint <oob-SHA256:fingerprint> \
  --target aarch64-unknown-linux-gnu \
  --external-build-root <fresh-owner-private-build-root> \
  --output-root <fresh-owner-private-package-root>
```

Set `CARGO_HOME` to the owner-controlled offline dependency cache. The builder
creates fresh owner-private `HOME`, Cargo target, sccache, temporary, and
config-free Cargo-home roots beneath `--external-build-root`, exports the exact
signed Git tree read-only, and invokes the authenticated Cargo executable
directly from `/`; it does not dispatch an ambient Cargo subcommand. Do not pass a prebuilt artifact: the candidate
controller accepts only `cargo-direct-frozen-signed-snapshot-v3` build
provenance. With all capture-owned
pins still zero, this package must truthfully report `release_ready=false`.
That is expected and is not a release failure.

Package publication uses the platform's atomic no-replace rename primitive;
an existing destination is never overwritten. The publisher reopens the
published directory, requires the same inode, and repeats the complete member
inventory before reporting success. Package verification holds one private
artifact snapshot through expected-digest admission, strict ELF validation,
authenticated identity execution, manifest binding, and copy, then repeats the
original package inventory. This prevents validation of artifact A followed by
execution or packaging of a path-swapped artifact B.

The package is immutable, content-addressed, mode `0500`, and contains only
`manifest.json` and `iroha_zk_x509_prover_worker`. Its booleans remain claims,
not authority: the controller parses it independently, rehashes the worker and
tool inputs, probes it through the signed package verifier, and independently
enforces AArch64, no `PT_INTERP`, no `DT_NEEDED`, and no writable-executable
program segment.

## Candidate capture

Run `scripts/capture_zk_x509_native_candidate.py` directly on the admitted
Linux AArch64 c7g.4xlarge host. Supply canonical absolute paths for every file
and directory. The two roots below must already exist, be disjoint, be outside
the source checkout, be owned by the process user, and have mode `0700`.

```text
python3 -I -S scripts/capture_zk_x509_native_candidate.py \
  --source-root <canonical-signed-source> \
  --package <immutable-worker-package> \
  --allowed-signers <canonical-oob-file> \
  --allowed-signers-sha256 <oob-sha256> \
  --revocation <canonical-oob-file> \
  --revocation-sha256 <oob-sha256> \
  --signer-principal <oob-principal> \
  --signer-fingerprint <oob-SHA256:fingerprint> \
  --region <oob-aws-region> \
  --expected-account-id <oob-account-id> \
  --expected-image-id <oob-ami-id> \
  --iid-certificate <canonical-regional-rsa2048-certificate> \
  --iid-certificate-sha256 <oob-sha256> \
  --openssl <canonical-openssl> \
  --openssl-sha256 <oob-sha256> \
  --git /usr/bin/git \
  --ssh-keygen /usr/bin/ssh-keygen \
  --ldd /usr/bin/ldd \
  --readelf /usr/bin/readelf \
  --external-build-root <owner-private-external-build-root> \
  --candidate-output-root <owner-private-candidate-root>
```

The controller authenticates the raw Git commit object and requires exactly
one SSH signature. It records the signer principal, fingerprint, raw commit
digest, allowed-signers digest, revocation-policy digest, signed controller
blob hashes, package root, unchanged `Cargo.lock`, workspace manifest,
toolchain tree, compiler/tool hashes, OpenSSL runtime closure, signed EC2 IID,
kernel/CPU/resource checks, exact command profiles, two byte-identical fresh
runner builds, and the four create-new candidate files:

- `native_release_expectations_v1.norito`
- `native_release_expectations_v1.json`
- `zk_x509_native_resource_v1.norito`
- `zk_x509_native_resource_v1.json`

The capture uses fixed ceilings of 300,000 milliseconds, 12 GiB peak RSS, and
32 GiB address space. A separately built runner reruns all 48 native stages.
The controller then independently validates the JSON projections and recomputes
the 60-field resource-certificate digest. The output directory and files are
owner-private and read-only, and the envelope is explicitly unsigned with
`promotion_authorized=false`.

Every captured subprocess is started below a trusted PID-namespace init.
Standard output and standard error are drained concurrently while their byte
ceilings are enforced. On normal target exit, timeout, overflow, controller
death, or runner error, the trusted init exits or is killed; Linux then kills
every remaining member of that PID namespace before the outer supervisor is
reaped and status is released. This remains true when a target or descendant
changes session/process group, closes inherited descriptors, clears its own
parent-death signal, or double-forks. A reverse launch-authorization pipe and a
short catchable-signal mask prevent target exec until the controller has
retained the outer-supervisor handle; signals raised during process creation
therefore still trigger synchronous teardown. OpenSSL and `ldd` remain open
through held descriptors for both runtime-closure passes, and their bytes and
durable path identities are checked before and after use. Cargo cache
provenance similarly binds each durable cache path to the held root descriptor
before and after sealing and immediately before durable-link materialization.
Final candidate publication is atomic no-replace and is accepted only after a
reopened same-inode, complete post-publication inventory matches the staged
inventory exactly.

The wall ceiling bounds ordinary execution, but teardown is intentionally
fail-closed rather than universally wall-clock bounded. After teardown starts,
the controller releases neither success nor error control until the trusted
supervisor is actually reaped. An exceptional uninterruptible kernel task can
therefore stall cleanup past the configured command timeout; returning while
the namespace might still contain a live process is forbidden.

The non-dumpable claim above applies to the trusted namespace init, not to an
arbitrary bounded target: Linux resets dumpability when an ordinary image is
executed. The qualified zk-X509 worker has a separate, source-closed policy. Its
`main` entry point immediately re-establishes non-dumpability, does so again
after the launcher's internal sealed static-image exec, and refuses qualified
identity unless a post-exec `PR_GET_DUMPABLE` check still reports false. The
packaged policy digest binds those exact worker and launcher source bytes; Git,
Cargo, OpenSSL, and other generic corridor tools make no `dumpable=false`
claim.

## Review and later release pinning

The candidate root and derived values are not source pins. Review the source
authentication, IID identity, host metadata, TCB closures, command records,
four artifacts, repeat validation, and all trust limits out of band. Only a
later clean repair commit may add the four fixture files and replace exactly
the eleven all-zero capture-owned constants in `profile.rs` and
`readiness_certificates.rs`. That repair must preserve the exact tracked
`Cargo.lock` bytes and contain no unrelated changes, and it must itself carry
exactly one approved SSH signature.

On that future pinned source, build the worker package again in a new external
provenance lane without `--require-release-ready`. Print and manually approve
its package root first:

```text
python3 -I -S scripts/package_zk_x509_prover_worker.py verify \
  --package <new-package> --print-package-root
```

Both verification commands must be executed by the packager from that exact
signed checkout. The default verifier authenticates its own packager,
workspace-identity helper, and exact-inode launch helper against the package's
`source_commit` before it executes the worker, and repeats that authentication
after the native identity probe. A copied script, dirty helper, or verifier
from another commit fails before the worker is launched.

Then perform the fail-closed release check with both the readiness requirement
and the independently approved root:

```text
python3 -I -S scripts/package_zk_x509_prover_worker.py verify \
  --package <new-package> \
  --require-release-ready \
  --trusted-package-root-sha256 <independently-approved-root>
```

Use the pinned runner's ordinary `generate` followed by `verify` for final
release evidence. Do not use `validate-captured-fixtures` after pinning: that
mode intentionally accepts only the all-zero, capture-open source state.

## Trust limits

RSA-2048 IID verification proves that AWS signed the recovered metadata bytes;
it is not a TPM, Nitro Enclave, measured-boot, process, or filesystem
attestation. It also has no verifier nonce or current-host freshness proof;
`instanceId` and `pendingTime` are signed review fields, not out-of-band
admission pins. A privileged host can still falsify local command execution and
resource observations. Hashing Git, SSH, Python, Cargo, rustc, the linker,
OpenSSL, `ldd`, the resolved OpenSSL loader/shared-library closure,
`/etc/ld.so.cache`, and the OpenSSL modules directory measures those bytes but
does not prove faithful execution. Except for inputs explicitly executed through
held descriptors, path-dispatched build tools also assume that no concurrent
same-owner process performs a swap-use-restore race between measurement and
invocation. The controller disables OpenSSL configuration loading, yet the
kernel, dynamic loader, libc, Python runtime, firmware, and hardware remain in
the trusted computing base. The clean-source manifest covers tracked and
non-ignored checkout state; ignored filesystem state is not part of that
identity. These limits are why this corridor can emit candidates but can never
authorize release pinning by itself.
