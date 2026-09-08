# Taira CLI release checks

Run either `python3 scripts/taira_release.py check` or
`python3 scripts/taira_release_check.py` before the Taira four-binary Linux
release build. Both compile one native `iroha_cli` harness with six Cargo jobs
and report build, stage and test durations. Python 3.11+, the repository Rust
toolchain, and previously fetched dependencies are required; Cargo runs offline.

Ordinary checks use the existing sibling `.taira-testnet-build-targets/routine`
directory. `--target-dir` or `TAIRA_TESTNET_CARGO_TARGET_DIR` can select another
existing development lane; both must agree if supplied. Ambient
`CARGO_TARGET_DIR` does not select this lane. Neither command creates or cleans
a Cargo target. Keep a stable lane for repeated checks.

Authenticated `prepare` retains the repository's `target/` lane and its fixed
Git-object source capture. Its explicit `--target-dir` override remains available;
the development environment selector does not affect preparation. Checks refuse
the exact repository `target/`, existing source capture lanes, and lanes marked
for release. Preparation refuses the routine lane and lanes marked for development.
A stable `.taira-build-lane/` owner-private directory records the role and canonical
repository root and holds a nonblocking mode lock through Cargo and every harness
child. Another checkout must select its own stable lane, so alternating checkouts
cannot churn the same lane's source paths. Preparation also
retains its original source-custody and output locks. A busy lane fails before
compilation. Existing target permissions remain unchanged.

Both modes use the same isolated `target/taira-release-cargo-home`, including the
same lexical registry paths, selected Rust toolchain, explicit repository Cargo
configuration, optional persistent sccache and the existing linker selection.
Cargo starts from `/` to exclude ancestor configuration. Ambient Cargo settings
and runtime credentials are not inherited. Diagnostic checks still compile the
mutable working tree and verify HEAD continuity; they never qualify a release.
The mode locks coordinate these entry points only: arbitrary direct Cargo commands
do not acquire them. Keep those commands out of an active preparation lane.

The check runs focused regressions for secure inherited configuration and signing FDs,
network-369 inventory decoding, aggregate timeout admission before custody,
required preseed budgets, per-host carrier verification and the exact action ledger,
native genesis-path rebasing with secret-free errors and owner-only output,
configuration artifact custody and signed genesis startup,
generated stages frozen to 0400 and their native consumers, preseed receipt
ordering, KVM ioctl error preservation on a regular file, and all five read-only
host preflights. The process-stream checks send an exact-digest 64 MiB closure
through real pipes with both outputs backpressured, bound each output turn,
retry interruptions, and verify absolute deadlines, early stdin closure, and
cleanup of a descendant retaining the output pipes. They also preserve a remote
rejection and its bounded stderr after early stdin closure, drain diagnostics
larger than pipe capacity, reject a successful exit with an incomplete frame,
and kill and reap a child that closes stdin then hangs. They use the system
`/usr/bin/python3` only for harmless disposable child fixtures. The KVM regression needs no KVM device or root access and runs
on macOS and Linux. The receipt namespace checks cover every host action and
canonical artifact role, including underscore-bearing labels, and retain path,
control-byte and Unicode rejection. Systemd checks use captured numeric
`ExecMainCode=1` evidence for a completed manager operation and require
`active/running` for the long-running validator. They cover terminal failures,
pending jobs, malformed evidence and read-only recovery after a deadline.
Start and restart also wait for the signed Python launcher to execute the exact
daemon within the original deadline, without submitting another manager job.
Changed launcher commands remain immediate failures.
The active journaled restart path also waits for four actual Torii backends before
onboarding, using the deadline captured before its one restart submission.
Convergence treats a valid zero-height commit frontier as pending within that
deadline; identity mismatches and restart requirements fail immediately. Tests
keep pending startup status out of retained proof and preserve public progress
fields in deadline errors.
Candidate tests exercise the real HTTP producer and strict host receipt consumer,
direct signed probe origins, private signer descriptor lifetime, ordered recovery
and failure before edge cutover. Stopped-owner tests preserve the slot lock while
releasing exact empty worker cgroups, reject live or substituted runtime state,
and verify idempotent cleanup before a stop receipt can be reused. Firewall
fixtures admit only exact slot-owned rules and real construction/deletion cuts;
foreign references or changed rules keep the barrier in place. Cleanup removes
each admitted rule explicitly and never flushes a chain.
Linux also runs a real OpenSSH
configuration-only check that verifies parent-held descriptor paths survive its
descriptor cleanup and replacement of the original paths. Every selected test
must exist and execute exactly once. Missing,
ignored, failed or empty selections fail the command. Fix the named failure and
rerun the same command to reuse compiled dependencies.

The selected toolchain's Cargo executes this command from `/` with the isolated
environment and selected `CARGO_TARGET_DIR`:

```sh
cargo --config /absolute/repo/.cargo/config.toml test --manifest-path /absolute/repo/Cargo.toml --locked --offline -p iroha_cli --bin iroha --no-run --message-format=json-render-diagnostics
```

Only existing disposable test fixtures are used. The gate accepts no live
configuration, keys, tokens, SSH, signing or deployment inputs, and writes no
report files. It is an early regression check and does not replace existing
release qualification, exact signed-source and artifact checks, or the offline
probes against actual Linux release binaries. Public readiness still requires
the end-to-end live checks.

The `build` job in `.github/workflows/workspace_release.yml` runs this check
before its full workspace build. CI provisions the stable `target/taira-native-checks`
subdirectory and passes it explicitly; the existing root target cache includes that
independent diagnostic target. CI runs `cargo fetch --locked` first to initialize
the same isolated registry cache, using the selected toolchain, explicit source
configuration and mode lock. Only this fetch sets `CARGO_NET_OFFLINE=false`; the
gate itself remains offline. The subsequent
full workspace build acquires the same development lane lock and uses the same
target, isolated Cargo home and explicit configuration. It explicitly preserves
CI's `CARGO_INCREMENTAL=0` after sanitizing the environment. Matching dependencies can
therefore reuse the gate's artifacts instead of being rebuilt into a second tree. The existing local Taira release caller should run
it before cross-compilation. The canonical
release artifact producer remains `scripts/run_release_pipeline.py`; it does
not gain a hidden build step or additional runtime authority.

Validate the runner without Cargo:

```sh
python3 -B -m unittest discover -s pytests/scripts -p test_taira_release_check.py
```

Cargo compiler errors retain their rendered diagnostics while the gate consumes
JSON artifact events; accelerator progress is also preserved.
