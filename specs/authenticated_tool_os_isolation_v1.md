# Authenticated tool OS isolation v1

`iroha_authenticated_tool_controller` is the repository-owned implementation
of `iroha.authenticated-tool-os-isolation.v1`. It is an explicit `dev-tools`
binary target in the existing `iroha_kagami` crate; production packaging must
build it with `--locked --release --features dev-tools`, install it as a
root-custodied executable, and pin its SHA-256 in the release-controller trust
record. A checkout path or an unreviewed locally rebuilt binary is not a
production trust root.

The production execution surface is `run-v1`. The Kagemusha readiness gate is
its sole current caller and emits the exact admitted option set.
`qualify-host-v1` accepts no arguments and runs the controller's built-in
hostile host suite. Its internal adversarial payload, `qualification-probe-v1`,
must not be granted as a standalone privileged sudo command. Unknown, duplicate,
inapplicable, missing, non-canonical, or oversized inputs fail with status
`125`. Policy limits fail with status `124`; otherwise the tool's status and
separate stdout/stderr byte streams are forwarded exactly. Tool stdin is
always a fresh `/dev/null`, never an inherited caller data channel.

The request protocol intentionally does not carry an expected tool digest.
The trusted caller must authenticate the digest and provide a private snapshot,
as the current Kagemusha readiness gate does. The controller then validates the
complete runtime/root-custodied parent chain and proves that snapshot's identity
and SHA-256 remain unchanged across execution. This divides trust without
letting an untrusted request choose its own supposedly expected digest.
The request does carry the launcher's attested numeric UID and GID; the
controller requires exact equality with both the real and effective runtime
credentials before it creates the job.

## macOS backend

The qualified macOS backend requires the root-owned immutable
`/usr/bin/sandbox-exec`. It authenticates the tool's canonical path, safe
ownership/mode/link identity, and SHA-256 before and after execution; starts it
at the exact canonical working directory with the caller's fixed sanitized
environment; requires `LANG=LC_ALL=C`, `PATH=/usr/bin:/bin`, a canonical
runtime/root-custodied `TMPDIR`, optional canonical `HOME`, and, when present,
`PYTHONDONTWRITEBYTECODE=1`; refuses inherited descriptors above stderr; applies a private
umask, zero core-dump limit, and file-size resource limit; and executes it as
the leader of a fresh session/process group under a generated Seatbelt profile.
The executable and every parent directory must also have no extended macOS ACL;
mode bits alone are not accepted as proof of custody.

Darwin has no `PR_SET_NO_NEW_PRIVS`. The v1 no-new-privileges invariant is
therefore discharged compositionally: the controller rejects a set-id origin,
requires equal real/effective user and group identities, rejects setuid/setgid
tool bits, denies tool forks, and permits `exec` only of that same authenticated
non-setid image. A non-root hostile qualification also proves that the image
cannot acquire uid 0. A root runtime already holds the contract's maximum
attested identity and cannot acquire a higher Unix identity.

The generated profile starts from `deny default`, so ambient Mach services are
not an escape hatch. Every request must select
`--deny-read-outside-allowlist`; there is no ambient filesystem-read mode. The
controller grants file data only for the authenticated executable, explicit
`--readable-file` paths, exact `--readable-directory` entries, writable output
files, and the fixed immutable Apple runtime roots under `/usr/lib`,
`/System/Library`, and the OS Cryptex System Library. Exact ancestor directories
receive metadata-only access for pathname lookup. Darwin additionally requires
data access to the filesystem-root directory during `execve`, which can expose
top-level names but not the contents or metadata of unlisted files below them.
`/dev/null`, `/dev/random`, and `/dev/urandom` are the only device read grants.
Operator homes, SSH material, key stores, `/etc`, `/private`, `/var`, and other
ambient paths remain denied unless the trusted caller names one exact input.
The only sysctl grants are `hw.memsize`, required by the verifier's native
memory guard, plus `hw.pagesize` and `hw.pagesize_compat`, required by the Rust
runtime's stack-guard setup; process-table, hostname, and other ambient sysctl
data remain denied.

Seatbelt also denies network operations, tool forks, execution of any
different image, every write outside the exact direct-child allowlist,
unlink/rename, hard links, clones, symlinks, directories, sockets, FIFOs, and
device nodes.
The deny-all mode permits no filesystem writes. Allowlisted outputs must remain
private, single-link regular files owned by the runtime identity. The
controller continuously validates each per-file ceiling, the combined output
ceiling, the complete live root, and protected entry identities. Because
unlink is denied in the kernel, an isolated tool cannot hide charged bytes in
an open-unlinked file.

A controller-owned watchdog is a sibling of the isolated job, not a child of
the tool. It receives no release data. Unexpected controller death closes its
private pipe, causing it to terminate both the original isolated process group
and the still-unreaped leader PID. Signaling both closes a process-group-change
escape; the tool starts as a session leader and therefore cannot move itself to
another group, while the kernel-level fork denial prevents an untracked descendant.
Normal return requires the leader reaped, the original group empty, and the
watchdog reaped.

`run-v1` retains a generic direct-output isolation contract for a future trusted
caller. Before Seatbelt starts, the controller securely pre-creates every absent
allowlisted output as a private regular file; the profile then grants data
writes but denies all file creation. An isolated tool must truncate/write those
files and must not require create-new or temporary rename/unlink publication.
The caller remains responsible for validating the results and performing any
authenticated atomic publication after the controller returns. No current
first-release production caller uses this writable-output mode.

The current Kagemusha readiness caller instead denies all writes and grants its
one pinned policy, the exact fixed release inventory, and the exact release
directory entry only; a digest-pinned but compromised verifier cannot read root
SSH or signing secrets and reflect them through stdout. Kagemusha's macOS memory
guard uses native `sysctlbyname` and `proc_pid_rusage` queries so the verifier
does not need a helper process.

## Linux backend

Linux requests currently fail closed with status `125`. Do not deploy the
macOS binary on a Linux promotion host or substitute a Bubblewrap-only wrapper:
the Linux backend still requires independently qualified Landlock path rules,
seccomp process/network/link denial, a delegated cgroup-v2 job with kill and
storage accounting, and hostile forced-controller-death evidence. This is an
explicit remaining production gap, not a developer fallback.

## Qualification and provisioning

Run the dependency-free unit and hostile host suite on every controller build:

```bash
ci/check_authenticated_tool_controller.sh
```

On macOS this first runs the same built-in `qualify-host-v1` entry point used by
protected hosts and then exercises the external hostile suite. Together they
cover successful exact write/output/status forwarding;
exact-input reads plus denial of an unlisted same-directory file,
`/etc/passwd`, and an ambient sysctl; direct writes plus explicit rejection of
create-new signer semantics; network, spawn, fork, `setsid`, ambient write, unlink,
rename, hard-link, symlink and FIFO denial; output and wall-time bounds; and watchdog cleanup
after forced controller death. They also force both the cumulative writable-file
quota and the complete live write-root quota past their limits and require an
exact status-`124` refusal. The protected Kagemusha qualification job
authenticates the source-built image, installs that exact byte string as root
mode `0555`, verifies its post-install identity/digest/byte equality, and runs
`qualify-host-v1` on the actual protected kernel before readiness validation.
Read access is intentional: the readiness gate must hash and copy the root-owned
image into its owner-private execution snapshot. A non-root identity cannot
modify the installed image or its parent chain.

`.github/workflows/promote_kagemusha_v4.yml` is the repository-owned protected
readiness verifier; it is not yet a publisher or activation workflow. Its
untrusted job builds an inert controller image. Its separately protected macOS
job checks out the exact GitHub workflow SHA, binds that SHA and canonical
workflow identity to the root-custodied reviewed-source checkout, checks the
image against the independent digest pin, and installs it only at
`/Library/SORA/Kagemusha/bin/iroha_authenticated_tool_controller`. It then
qualifies the image, authenticates the root-custodied readiness gate and Python
interpreter/runtime tree, and invokes the gate in strict `promotion`-validation
mode under `env -i` with every policy, catalog, Kagami, source-authority,
sealed-build-report, and physical-iOS trust pin. Missing identities, paths,
pins, exact sudo grants, or host capabilities stop the workflow. Verification
of a pre-existing promotion record does not publish a release, qualify a
validator catalog, submit an activation, or create operator trust records.
The workflow derives a domain-separated promotion id from the immutable GitHub
repository, workflow ref/SHA, run id, and run attempt, and admits the current
catalog revalidation receipt only at
`/Library/SORA/Kagemusha/catalog-revalidation/<promotion-id>.json`. An
independent authority must create that root-custodied receipt after dispatch
and before protected-environment approval; authority signing material and
DeviceCheck credentials never enter this verification workflow.
