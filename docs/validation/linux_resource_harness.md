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

The recorded ordinary-native harness suite passed 56 tests, including real
process accounting, enforced-limit mismatch,
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

Current Linux native tests require an explicitly supplied normal-publisher bridge
and independent module, provenance and canonical source pins. Use
[`run_zk_resource_harness_native.py`](../../scripts/tests/run_zk_resource_harness_native.py)
for all `NativeRunTests`, and
[`run_zk_resource_harness_public_delivery.py`](../../scripts/tests/run_zk_resource_harness_public_delivery.py)
for the six public delivery controls; both list their required inputs with
`--help`. A bare unittest invocation cannot qualify the current native path and
fails when these inputs are absent. Keep the canonical source owner retained
through publication and observation; a diagnostic shared library alone is not
an authenticated installed bridge.

The final 2026-10-31 tightened harness passes all 20 native methods and six public
delivery methods, alongside 225 regression controls. The native suite retains
unchanged Python hashes and matching source censuses around its tracked-output
refusal check. Its receipt is
`dist/linux-harness-native20-terminal-fixture-20261031/result.json`
(`6ae37301f3538b398eb5e9310ae627c346ed1b6ab31964b65cd8f0a4ee812c9c`).
The two post-run publication-error controls now wait for the actual original
child's non-reaping terminal event before the unchanged reader runs. This repairs
a separately reproduced fixture sampling race; it does not establish the cause
of an earlier suite's additional timeout reason. Earlier failed attempts remain
retained, and these engineering controls do not repair the rejected maximum proof.

`run --max-peak-rss-bytes 12884901888` states a 12 GiB engineering acceptance
bound applied independently to the original child's kernel child-rusage peak
and every retained RSS sample. The bound is inclusive and must be a positive
unsigned 64-bit integer. Missing, invalid or unavailable kernel accounting
rejects the run; the emitter's own RSS claim cannot substitute for it. This is
an acceptance check after execution, separate from the address-space limit
enforced with `--address-space-limit-bytes`.

Linux lifecycle evidence retains the original `wait4` peak in bytes and must
match the report exactly. Replay recomputes sample metrics from captured journal
bytes; coordinated lower report and window summaries cannot erase a larger
retained sample. A larger initial or later sample independently exceeds the
bound even when the `wait4` peak is below it. The metrics are preserved separately:
`smaps_rollup` walks page tables, while `wait4` high-water accounting uses batched
RSS counters, so neither metric is required to exceed the other. No tolerance
is applied. See the matching published WSL kernel's
[page-table sampling](https://github.com/microsoft/WSL2-Linux-Kernel/blob/d504d40ab83839cb61942a812a99d183c42227b9/fs/proc/task_mmu.c#L1398)
and [RSS/high-water counters](https://github.com/microsoft/WSL2-Linux-Kernel/blob/d504d40ab83839cb61942a812a99d183c42227b9/include/linux/mm.h#L2839).
Journal snapshots spill beyond 8 MiB to an anonymous temporary file and never
reopen evidence paths during replay.

The strict report requirement `max_peak_rss_bytes` is always present (`null`
when no bound was requested). `salvage --max-peak-rss-bytes` and pinned consumers
retain the original requirements and verdict, then separately check the consumer
bound. The original bound must be at least as strict as the consumer's; weaker
or omitted consumer requirements cannot rescue a run that exceeded its original
bound. Synthetic controls cover exact and one-byte-over boundaries and invalid
observations; the native control compares the actual kernel peak against a
one-page emitter claim.
