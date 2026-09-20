# Physical Validate retry — 2026-09-19

Changes remain in `/Users/takemiyamakoto/dev/iroha` on `optimizations`; no branch,
worktree, commit, compatibility path or sibling documentation edit is introduced.

The exact original durable Validate dispatch, external lifecycle wait and keyed
worker index now survive a failed physical Queue/lifecycle probe. Guards are
released before parking. The original Queue runner notification is registered
against the actual failed mutex; release before first polling is observed too.
The continuation retries the same key on the same I/O queue. If command admission
is full, actual command release wakes that same runner. Pending completion custody
allows the existing authenticated ordinary-head drain, so full command and
completion queues cannot form the identified retry cycle. No extra lifecycle
ordinal, logical wake, sidecar identity, timer or worker is created.

No prospective retirement binding means no Queue/lifecycle acquisition. Pending
local Queue promises and a configured Native evidence ceiling are typed local
failures and cannot mint invalid-body markers. The latter is fixed configuration,
not occupied capacity with an eventual release. Canonical standalone framing
limits remain deterministic rejection. Apply retains its blocking observation and
final veto because original prepared execution/publication custody is unfinished.
Physical refusal still discards prepared execution and retries validation; preserving
that original execution through decision/publication remains required.

## Exact validation

Combined build58 passes for Core, Torii, test-network, Kagami and daemon library/
binary targets, plus `iroha_core_group_02` and `taira_consensus_contracts`:
`scripts/cargo_fast.sh --stable-local-metadata --incremental --jobs 4 -- test`
with those five packages, `--lib --bins`, the two integration targets and
`--no-run --message-format=json --locked --offline`.

All 871 selected runtime controls pass (818 Core, 53 Torii) on unchanged 7,060
captured inputs and unchanged emitted test binaries. Selection includes all
262 worker, 125 work-registry, 92 Apply, 54 body-store, 79 lane-planner,
71 coordinator, 41 candidate, 31 block-sync and 16 height-driver tests; eleven
Queue retirement, three Native byte-budget, one grouped Native local-capacity,
17 DA hydration, ten complete-input, two certified gossip and three repair
crash-cut tests, plus 53 Torii controls. The manual worker fixture exercises
actual queues and the production driver, not a live network or threaded worker
fault campaign. Physical-release tests cover release before registration,
foreign releases, unchanged waiting authority, full command/completion channels,
exact retry/publication and actual runner wake. The launched fixture follows the
existing 32 MiB test-thread convention; the smaller wait fixtures keep the default.

The canonical multilane binding gate has zero diagnostics. All 49 selected Python
controls pass on 8,138 unchanged inputs: twelve parser controls, five existing
source/preparation/QueuePlan controls and all 32 durable Validate fidelity controls
(including 21 new mutations). Reviewed bindings require retained dispatch/release,
exact retry capacity, publication ordering, original physical incumbent identity,
extracted successor authority and authenticated sidecar cleanup. Compiled source
contracts also check the live no-verdict/retry boundary. These are structural and
mutation checks; no full proof-ledger, new TLC/Verus or production-trace proof is
claimed. Both captures include `source_contracts_v1.txt`. Scoped Rustfmt 2024,
retired-codec and diff checks pass; no whole-workspace formatting pass is claimed.

Build55 passed; runtime55 retained 818 passes and six failures without source or
binary drift. They exposed the new launched fixture's default-stack limit, the
old deterministic-error assertion, and four consumers of two stale history
fixtures. The fixes use the established large-fixture stack, assert typed local
capacity without rejection authority, construct actual payload commitments through
BlockBuilder and initialize authenticated Kura geometry before history writes.
No production guard or behavioral assertion was disabled. Build56 then failed on
the newly introduced wrong BlockBuilder module path; its 53 Torii passes do not
qualify that failed combined build. Build57 corrected the module path and passed
compilation and the formal selection. Runtime57 passed 862 of 871 controls; nine
consumers failed because the history fixture installed active markers before
anchoring configured geometry.
Build58 uses the production fallible State constructor and canonical pre-genesis
setup; no eager-marker convenience constructor enters that fixture. The
initial package-name invocation error and earlier Python positive/mutation-target
failures remain in their separate raw receipts.

Raw evidence is under ignored `dist/sumeragi-main-work/`:
`validation58.json`, `build58-source-{before,after}.json`,
`local-validation-controls58/summary.json`, `formal-local-validation58/summary.json`
and `static58.json`. HEAD and index remained unchanged.

These component controls do not qualify real-peer progress. Retirement still
requires complete canonical drain/carry semantics that preserve admitted promises;
waiting for the blocked height itself would be a semantic cycle. Complete capture
admission, original Validate/cache/reproposal/Apply execution ownership, production
shared-lane cutover and unchanged four/seven-validator fault/restart/final-work
campaigns remain open. All six liveness milestones remain open.
