# Linux process resource harness validation

The 2026-10-22 Intel i7-11800H/WSL2 run exercised real Linux child-process
`RLIMIT_AS` enforcement. The kernel's `/proc/<pid>/limits`, harness requirement
and emitter record agree at both 8 GiB and 32 GiB. A record claiming 32 GiB
under an 8 GiB applied limit is rejected. These are harness controls using a
native `tee` child and public synthetic records, not a measured prover or
whole-network qualification.

The run exposed a sampling race: a child exiting while the sampler slept was
queried as a live process afterward, and Linux correctly refused its zombie
state. The unmonitored sampling path now reaps again immediately after sleeping.
An attempted observation that fails still rejects the run, and the Darwin
terminal-monitor path keeps its existing ordering and checks.

`ObservationExitRaceTests.test_exit_during_sampling_sleep_is_reaped_before_a_live_observation`
fails against the original loop by attempting to observe the exited child and
passes with the fix. The existing later-exit control still rejects an earlier
observer failure.

Validation:

```sh
python3 -m unittest -v scripts/tests/zk_resource_harness_test.py
```

All 56 tests pass, including real process accounting, enforced-limit mismatch,
recovery of accepted evidence, observation failures and deadline controls.
The same real-child helper was also run with `--address-space-limit-bytes
34359738368`; it returned `accepted` with both kernel limits at that value.
Logs, the original failure and the 32 GiB report are retained untracked under
`dist/intel-platform/` as `linux-resource-harness-unittest.log`,
`linux-resource-harness-final.log`, `linux-exit-race-before-fix.log`,
`linux-rlimit-32g.log` and `linux-rlimit-32g.json`.

The separate `resource_process_exit_test.py` suite now supplies a symbolic kqueue
interface for its mocked cases when the host lacks that API. All 24 mock controls
pass on Linux; the two actual Darwin controls remain skipped and require native
Darwin execution. This Linux run does not establish Darwin terminal behavior.
