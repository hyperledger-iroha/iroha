# TON SCCP release-builder boundary

`scripts/ton_sccp_builder.py` is the only repository-owned path that can emit
the three TON build identities accepted by final-V1 SCCP release evidence. It
does not download Acton, use `ACTON_BIN`, resolve a compiler from `PATH`, or
hold a signing key.

## Development smoke

Local contract checks are explicitly non-release:

```text
scripts/sccp_ton_contract_build.sh development-local --acton /absolute/path/to/acton
```

The executable must report exact Acton 1.1.0 and embedded Tolk 1.4.1. This mode
runs format, check, build, and test, then states that no release receipt was
emitted. Its output is never valid destination-build evidence.

## Externally approved production inputs

Production operators supply one canonical
`iroha.sccp.ton-builder-policy.final-v1` document and its independently trusted
SHA-256. The policy binds:

- a clean, full, signed Git commit, its signer fingerprint, and its exact
  `SOURCE_DATE_EPOCH`;
- one nonzero digest-addressed Linux/amd64 builder image and fixed entrypoint;
- the reviewed official Acton 1.1.0 Linux archive digest, exact version output,
  and exact Tolk 1.4.1 version;
- the SHA-256 of the host Python, Git, Docker, and OpenPGP commit-verifier
  executables (`host_commit_verifier_sha256` binds the last one);
- a sorted, byte-exact Acton/builder/Tolk-stdlib inventory; and
- finite file-count, byte, log, and execution-time limits.

The policy also pins distinct `release-engineering` and `release-security`
Ed25519 public keys. Private keys and signing commands stay outside this
repository.

Both production commands require `--commit-verifier /absolute/approved/verifier`.
The verifier must be a direct, bounded executable whose bytes match the policy;
its path must contain only canonical shell-inert components. Git signature
verification and fingerprint readback use this same pinned helper for every
signature format, so repository configuration cannot select another program.
The approved helper must implement offline OpenPGP verification; signing keys
are never supplied to the builder. Policies missing the verifier digest fail
validation.

Every Git operation, including archival, disables system/global configuration,
replacement objects, lazy fetching, terminal prompts, filesystem-monitor
commands, and hooks. The archive and signed-source checks therefore read the
same original Git objects without ambient command helpers.

After read-only repository discovery, status, signature checks, and archival
run against fresh private Git metadata with a fixed source commit and direct
access to the original object store. Source configuration, refs,
`.git/info/attributes`, global/system attributes, and Git templates are not
copied. Signed tracked attributes retain their archive behavior; filter names
cannot select host commands, and signature substitutions use the same pinned
verifier. Status reads the original worktree and index with optional locks
disabled, preserving staged and unstaged checks without modifying the index.
Optional submodule worktrees are not traversed; a separate cached raw diff
checks staged files and gitlink identities against the approved commit.
Both SHA-1 and SHA-256 Git object formats are supported.

The digest-addressed builder image must implement the policy-pinned entrypoint.
It receives only a read-only `git archive`, an empty output mount, and fixed
source identities. It runs with `--pull=never`, `--platform=linux/amd64`,
`--network=none`, a read-only root filesystem, no Linux capabilities,
`no-new-privileges`, a finite PID limit, and a private temporary work
filesystem. Its output contains exactly `artifacts/`, `toolchain/`, and one
canonical `builder-report.json`. The report and the actual inodes must agree on
every path, size, executable bit, and SHA-256.

## Two-step offline approval

`production-prepare` verifies the signed source commit and pinned host tools,
performs two isolated container builds, and requires byte-identical artifact
and toolchain inventories. It publishes an unsigned output lock and the exact
domain-separated signing payload to a new owner-only directory outside the
repository.

The two policy-pinned roles independently sign that payload. An operator then
constructs canonical `iroha.sccp.ton-output-lock.final-v1` with the two ordered
detached signatures and invokes `production-release`. Release mode repeats both
isolated builds; stale or substituted signatures, sources, policies,
toolchains, images, or output bytes fail closed.

Artifacts are copied through descriptor-relative, exclusive, no-follow writes
into a new owner-only directory and hashed back from the opened inodes. The
receipt is published last and contains the exact lower-hex values:

- `ton_builder_policy_sha256`
- `ton_source_closure_sha256`
- `ton_output_lock_sha256`

These values are the inputs required by `DestinationBuildPolicyV1`. A receipt
from `development-local`, a single build, an unsigned lock, or a caller-chosen
digest is not defined.
