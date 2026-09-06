# SCCP release-validator build boundary

`scripts/sccp_validator_builder.py` is the final-V1 production path for the
Rust `sccp_release_evidence` executable. It accepts no signing private key and
does not infer a tool, image, source revision, feature, or expected digest from
the ambient host.

## Immutable external policy

The release authority supplies a canonical
`iroha.sccp.validator-builder-policy.final-v1` document and communicates its
SHA-256 independently. The policy binds all of the following before a release
build is accepted:

- one clean signed Git commit, its approved signer fingerprint, and the commit
  time used as `SOURCE_DATE_EPOCH`;
- one digest-addressed Linux/amd64 container image;
- the reviewed container driver and exact Python, Cargo, rustc, linker, Cargo
  cache, target triple, and version reports;
- SHA-256 identities for the host Python, Git, and Docker executables, the
  reviewed host orchestrator, and `sccp_release_common.py`;
- a canonical SHA-256 commitment to the Linux/amd64 Docker daemon's version,
  kernel, storage and cgroup drivers, default runtime, runtime component
  commits, security options, isolation flags, and empty server-error state;
- the SHA-256 identity of a network-inert OpenPGP commit verifier (for example,
  an audited `gpgv` build); Git is forced to use it with system/global config,
  replacement objects, hooks, and filesystem monitors disabled;
- the expected vendored dependency inventory, Cargo metadata graph, SBOM,
  toolchain inventory, sysroot inventory, linker, build recipe, and build
  environment; and
- distinct Ed25519 public keys for `release-engineering` and
  `release-security`, plus explicit file-count, byte, log, and timeout limits.

Guardian keys, builder/image identities, expected closure hashes, and signer
identities are external release inputs. They must not be replaced with example
or locally generated production values.

## Exact build

Each release role performs `prepare` independently, preferably on separately
administered hosts:

```text
python3 scripts/sccp_validator_builder.py prepare \
  --role release-engineering \
  --policy /secure/public/validator-builder-policy.json \
  --trusted-policy-sha256 $TRUSTED_POLICY_SHA256 \
  --git /approved/bin/git \
  --docker /approved/bin/docker \
  --commit-verifier /approved/bin/gpgv \
  --output-dir /secure/rebuilds/release-engineering
```

The security role repeats the command with `--role release-security` and a
different output directory. The orchestrator verifies the signed commit and a
clean tracked/nonignored tree, streams a deterministic `git archive`,
adds a canonical inventory binding every archived path, Git mode, blob object
ID, and unexpanded gitlink to the signed tree. Git, Docker, and the commit
verifier are copied from their authenticated inodes into an owner-only run
directory before first use. The image runs with `--pull=never`,
`--platform=linux/amd64`, `--network=none`, a read-only root, no capabilities,
`no-new-privileges`, fixed PID/CPU/memory/swap/file-descriptor/file-size limits,
and private bounded tmpfs. Docker uses an empty per-run configuration and the
explicit local Unix socket; no ambient context is accepted.
The container uses an SCCP-owned, hash-bound seccomp profile derived from the
tagged Moby `seccomp/v0.2.1` baseline. Its default action is `EPERM`; only the
reviewed Linux/amd64 build syscall set is allowed, `socket` is restricted to
`AF_UNIX`, `clone` rejects every namespace flag, and `clone3` returns `ENOSYS`
for the libc fallback. Unknown syscalls, `io_uring`, SysV IPC, POSIX message
queues, Linux AIO, BPF, ptrace, keyrings, and mount/namespace operations are
therefore unavailable.

The reviewed driver is baked into the image at
`/opt/iroha/sccp_validator_builder_driver.py`; it is never host-mounted, and it
authenticates its own bytes against the policy before doing work. Inside the
image it rejects Cargo credentials and ambient Cargo source configuration. It
vendors offline and performs exactly:

```text
cargo build --release --locked --frozen --offline \
  --no-default-features --features dev-tools \
  -p iroha_sccp --bin sccp_release_evidence \
  --jobs 1 --target x86_64-unknown-linux-gnu
```

Immediately before Cargo builds, Linux Landlock denies the compiler and every
build-script descendant writes outside fresh target and home scratch trees.
After each tool invocation the PID-1 driver kills and reaps any residual
process before inspecting closure bytes. Absence of Landlock ABI v3, a private
PID namespace, or the required syscalls fails closed.

The driver records the normalized Cargo metadata graph, a deterministic SBOM,
every vendored file, every sysroot file, exact tool executables and versions,
the linker, exact generated Cargo configuration, recipe, environment, source
archive, and output executable. Both the driver and host reject metadata whose
workspace, target, manifest, target-source, dependency, licence, or readme paths
escape the inventoried tracked-source and vendor trees. Large source and
executable artifacts are hashed and copied as bounded streams. It streams only
a fixed-inventory, metadata-normalized USTAR archive to the host; there is no
writable host output bind. The host bounds archive and diagnostic streams independently,
force-removes the unpredictable name/CID-tracked container on every exit,
proves it absent, extracts only the exact inventory, and byte-compares a
reconstructed canonical archive before accepting it. The host scans the
bounded Cargo build log; closure documents, the signed Git blobs selected by
the exact source inventory, and the final executable receive bounded recursive
concrete-secret scanning without rejecting source identifiers that merely
discuss key types. An intentional detector fixture can be exempted only by a
policy entry binding both its exact tracked path and its exact Git object ID;
changing either makes the exception inapplicable. The aggregate decode/work
budget is shared across the full scan rather than reset for each chunk or file.

The already-running host Python interpreter, kernel, Docker daemon, filesystem,
and administrator are an explicit release-host trust boundary. The
orchestrator authenticates its own source, `sccp_release_common.py`, and a
canonical security-relevant daemon report against the immutable policy before
starting a build. Production runs must still use a dedicated,
administratively isolated builder host and an externally authenticated,
root-owned non-writable installation of those sources. A digest-pinned image
cannot defend against a malicious daemon that lies consistently about its
state or a compromised interpreter that began executing before the script's
checks.

## Offline signatures and finalization

Each candidate directory contains
`unsigned-rebuild-attestation.json` and the exact domain-separated
`rebuild-signing-payload.bin`. The named offline role signs the payload and
constructs canonical `iroha.sccp.validator-rebuild-attestation.final-v1` by
adding only this exact `provenance` object to the unsigned fields:

```text
role, signer_id, algorithm="ed25519", public_key_hex, signature_b64
```

No repository tool handles the private key. Finalization consumes both
candidates and both signed attestations:

```text
python3 scripts/sccp_validator_builder.py finalize \
  --policy /secure/public/validator-builder-policy.json \
  --trusted-policy-sha256 $TRUSTED_POLICY_SHA256 \
  --engineering-candidate /secure/rebuilds/release-engineering \
  --engineering-signed-rebuild /secure/signatures/release-engineering.json \
  --security-candidate /secure/rebuilds/release-security \
  --security-signed-rebuild /secure/signatures/release-security.json \
  --output-dir /secure/releases/sccp-validator-final-v1
```

Finalization requires distinct nonces, identities, keys, and signatures. It
compares the actual source archive, all eight closure artifacts (including the
exact generated `cargo-config.toml`), and executable
bytes from both rebuilds. Matching summary strings alone are insufficient.
Every file is then published with exclusive descriptor-relative no-follow
writes into a new owner-only directory. The receipt is written last.

The final bundle contains `source.tar`, `closure/`, the executable under
`validator/`, both signed rebuilds, the policy, builder report, output lock, and
`validator-build-receipt.json`. The receipt has exact lower-hex identities:

- `validator_builder_policy_sha256`
- `validator_source_archive_sha256`
- `validator_dependency_inventory_sha256`
- `validator_cargo_metadata_closure_sha256`
- `validator_sbom_sha256`
- `validator_toolchain_inventory_sha256`
- `validator_sysroot_inventory_sha256`
- `validator_linker_sha256`
- `validator_build_recipe_sha256`
- `validator_build_environment_sha256`
- `validator_container_manifest_sha256`
- `validator_builder_report_sha256`
- `validator_executable_sha256`
- `validator_complete_build_closure_sha256`
- `validator_output_lock_sha256`

Every consumer must re-authenticate the published directory before trusting
those identities:

```text
python3 scripts/sccp_validator_builder.py verify \
  --release-dir /secure/releases/sccp-validator-final-v1 \
  --trusted-policy-sha256 $TRUSTED_POLICY_SHA256
```

The read-only verifier requires the exact directory inventory, normalized
policy and externally trusted policy digest, actual report/closure/source and
executable bytes, both independent Ed25519 rebuild attestations, reconstructed
output lock, and reconstructed receipt. Its normalized result contains the
sole executable path and the exact ordered `hashes` object. Release tooling
must equate the executable it stages and runs to
`hashes.validator_executable_sha256` and require all 15 values to agree across
every production proof profile; parsing the receipt alone is not sufficient.

Accordingly, `sccp_all_lanes_evidence.py`, `sccp_release_bundle.py`,
`sccp_verify_release_bundle.py`, and `sccp_release_readiness_report.py` accept
only `--validator-build-release` plus
`--trusted-validator-builder-policy-sha256`. They expose no production option
for an ambient validator executable. Each command replays this directory
verification, then authenticates and privately stages the exact returned
executable bytes immediately before every use. The published path and its
mutable inode are never executed directly after hashing.

It also binds the source commit, source/executable sizes, and both ordered
signed-rebuild identities. A local Cargo executable, a dirty checkout, a
single rebuild, a cached signature, a network-capable container, or a build
with any other feature/target/recipe cannot emit this receipt.
