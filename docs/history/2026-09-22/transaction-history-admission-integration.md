# Transaction-history integration checkpoint

Work is confined to `/Users/takemiyamakoto/devstuff/iroha` on `optimizations`,
observed source commit `1b8e5f92b6dadb1dcdd16f945286560e31b347ff`. No branch,
worktree, commit or release candidate was created. All fourteen release goals
remain open; no compatibility path or HSM prerequisite was added.

The integrated history pool uses the existing admitted native B+tree and exact
charged generation identities. Kura owns one configured finite pool, wired as
`kura.transaction_history_bytes` through user, actual and default configuration.
Logical preparation retains its original owner during local refusal/retry;
physical attachment precedes publication. The current membership-root and
record implementation is preserved, including refusal of a matching but
physically unprepared publisher before changing visibility.

Fresh State and snapshot recovery use the original Kura pool. Restore preserves
local admission errors as typed State admission failures; daemon policy refuses
to convert them into recoverable serialization failures or empty-State replay.
Snapshot serialization uses the already captured membership view.

The existing native release mechanism now supports retaining the actual read
unlock notification. StateBlock keeps original member/hash/index notices until
all enclosing execution writers retire, and carries the same owner through
merge, drain, autoscale and journal-capture paths. Snapshot and replay scopes
also retain original allocation refunds outside physical fences. Replay keeps
the replaced State outside its commit guard on normal return and unwind.
Native default poison semantics remain conservative; retained acquisitions use
the reviewed explicit same-kernel path and never reset a poison verdict.

Seven reviewed packets compose into 90 changed files plus four unchanged
membership context files. The sole replay conflict was resolved by retaining
both the original notice owner and the replaced State's lifetime. Exact applied
patch: `5b223e3ff3af552a71bf01d7f5511f1934f0d304938b9c87c2609df99ec011ee`.
Application receipt: `target/first-release-history-application-20260922.json`,
SHA-256 `7d3c9286f9d58f91cb1f8d3d78db9eb10a245d6018b4b4504a1ff75d943101a8`.

The actual locked/offline Core library check passed in 279.83 seconds. Every
observed source input was unchanged and no new input appeared. Its log is
`target/first-release-core-history-check-20260922/cargo.log`, SHA-256
`f2d1c4279ad5177b802b941beb35304e35850aa0c86186ebb821a176f6483172`.
The five unused-code warnings were then corrected by narrowing test-only
imports/methods and preserving the original prior-tip owner under an explicit
retirement field. These follow-ups and two actual replay success/unwind
regressions are recorded in
`target/first-release-history-followup-application-20260922.json`
(`73e69e8d687524354cfb97b875dc53b9307f0107e32c982bce7ec78380a87d21`).
The first Core test build exposed seven test-only integration errors. Their
fixture, import and observer repairs are recorded in
`target/first-release-history-test-fixes-application-20260922.json` and
`target/first-release-identity-observer-application-20260922.json`.
The next locked/offline feature test build passed in 407.59 seconds with no
compiler diagnostics. Its 763-test selection aborted after 347 passes when
`retained_execution_phases_survive_marker_reproposal_and_publication_refusals`
overflowed the ordinary test-worker stack. The isolated captured binary
reproduces the abort. Fourteen required reader, replay, drain and transitive
ownership controls pass separately on that same binary. A diagnostic 4 MiB
stack run passes the failing test, establishing finite stack pressure; it is
not ordinary-stack qualification or a production limit change. Stage tracing
and an ordinary-stack correction remain in progress.

All 897 configuration package tests passed (651 unit, 245 declared integration
and one documentation test), with zero failures, ignores or filtered cases.
The 212.55-second run observed no changed or added source inputs. Its log is
`target/first-release-config-history-tests-20260922/cargo.log`, SHA-256
`4b28c3c6334f884baa673e431cdf7dcc5661bbe587f218eeb9a680b04ed7fbd7`.
The daemon snapshot policy selection passed 17 tests and failed two whose
fixtures no longer modeled the authenticated geometry baseline and immutable
configured catalog. Reviewed test-only corrections are applied and await
execution. Formatting, retired-codec, scoped diff and history-archive checks
pass after the current integration. The complete Core/daemon runtime suites
and strict lint remain pending.
The earlier 473 passing native component tests have their own source/lock
scope and do not replace actual integrated Core tests.

This slice does not complete aggregate memory admission: hot-tip membership,
snapshot scratch, cold-store and external resource owners still need their
full admission and qualification. It does not prove every standalone State
view caller is free of enclosing fences. Process-lived Native cutover,
network/restart qualification and all privacy/SoraFS/multilane release gates
remain open. No release-source seal or production ceiling was relaxed.
