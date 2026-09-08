# Taira CLI release checks

Run `python3 scripts/taira_release_check.py` before the existing Taira four-binary
Linux release build. It compiles the native `iroha_cli` test harness once through
`scripts/cargo_fast.sh`, reuses the warm Cargo target and native jobserver, and
reports build, stage and test durations. Python 3.11+ and the repository Rust
and Cargo toolchain are required. Use an existing `CARGO_TARGET_DIR` only for
an established separate native build lane; no clean or per-run target is needed.

The check runs 33 regressions for secure inherited configuration FDs,
network-369 inventory decoding, aggregate timeout admission before custody,
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
Linux also runs a real OpenSSH
configuration-only check that verifies parent-held descriptor paths survive its
descriptor cleanup and replacement of the original paths. Every selected test
must exist and execute exactly once. Missing,
ignored, failed or empty selections fail the command. Fix the named failure and
rerun the same command to reuse compiled dependencies.

The native compile command is:

```sh
scripts/cargo_fast.sh -- test --locked -p iroha_cli --bin iroha --no-run --message-format=json-render-diagnostics
```

Only existing disposable test fixtures are used. The gate accepts no live
configuration, keys, tokens, SSH, signing or deployment inputs, and writes no
report files. It is an early regression check and does not replace existing
release qualification, exact signed-source and artifact checks, or the offline
probes against actual Linux release binaries. Public readiness still requires
the end-to-end live checks.

The `build` job in `.github/workflows/workspace_release.yml` runs this check
before its full workspace build using the same Cargo cache. The existing local
Taira release caller should run it before cross-compilation. The canonical
release artifact producer remains `scripts/run_release_pipeline.py`; it does
not gain a hidden build step or additional runtime authority.

Validate the runner without Cargo:

```sh
python3 -B -m unittest discover -s pytests/scripts -p test_taira_release_check.py
```

Cargo compiler errors retain their rendered diagnostics while the gate consumes
JSON artifact events; accelerator progress is also preserved.
