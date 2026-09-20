# Canonical pending admission across drain closure — 2026-09-19

All work uses `/Users/takemiyamakoto/dev/iroha` on `optimizations`. No branch,
worktree, commit, compatibility path or sibling repository edit is introduced.

Queue had been applying the fresh-admission close gate to its already accepted,
canonically ranked pending input. On close this could latch a permanent routing
fault, refuse an exact lost-response retry, and prevent cold journal replay.
The same State-owned pending-route projection is consumed by Queue and Native
closed-lane opening. It distinguishes an active route from retained draining
authority. It joins the full immutable binding to its exact
registry, obligation, every atomic route member, canonical predecessor and first
carrier rank. Each closed leg must retain its original incarnation and pinned
committee, rank at or before close, and have no drain commitment. Missing,
terminal, conflicting, stale or corrupt evidence grants no pending continuation.

Queue uses that authority for immutable lookup, routing refresh, exact durable
retry, cold journal replay and canonical pending body handoff. Fresh ingress
continues through the existing close gate. Ordinary global selection, pop and
new autonomous reservation do not execute a draining input; the existing Native
pending owner must finish it. The original claim, timestamp, journal digest and
FIFO cell are retained; no new context or receipt is minted. Uncarried off-chain
receipts do not become canonical authority through this rule.

The Queue component controls cover original lookup/retry without rebinding,
healthy ordinary-selection deferral, cold replay after close, exact canonical
body handoff into a distinct local Queue and refusal of fresh/unranked inputs.
The State controls commit actual admission carriers and cover active, absent and
applied states; closed authority after live-key churn; wrong ranks and missing
markers; wrong network/incarnation/pin/commitment; and every atomic participant.
These are component controls, not a real-peer fault campaign.

Build59 passed. Its expanded runtime selection passed 1,252/1,256 on unchanged
7,060 captured inputs and emitted test binaries. All failures were stack overflows
in three new large State fixtures and one existing consumer of their shared setup.
The high-volume barrier proof control passed in 337.25 seconds. An isolated 16 MiB
stack diagnostic passed the three new cases; this does not qualify runtime59.
Candidate60 uses the existing explicit State test-stack harness only for those
four cases; no assertion or production stack setting changes. It also replaces
the Native closed-loop copy with the shared retained authority while preserving
its staged height, activation, incarnation and committee/PoP guards.

Formal59 initially failed two positives and its gate on a source-token constructor
fragment; all 28 semantic mutations and ledger negatives already passed. The
paired token/ledger correction restored all 51 controls and zero gate diagnostics
on unchanged 8,138 inputs.

Candidate60 passes the combined build and all 1,256 selected runtime controls
(1,203 Core, 53 Torii) on unchanged 7,060 inputs and emitted test binaries. The
selection includes all 362 Queue controls, 30 State QueuePlan and four Native
opening controls, plus the prior worker/work-registry/Apply/body-store/planner/
coordinator/candidate/block-sync/height-driver and ingress/recovery selections.

The combined build command is
`scripts/cargo_fast.sh --stable-local-metadata --incremental --jobs 4 -- test`
with `-p iroha_core -p iroha_torii -p iroha_test_network -p iroha_kagami -p irohad`,
`--lib --bins --test iroha_core_group_02 --test taira_consensus_contracts`
and `--no-run --message-format=json --locked --offline`. Emitted binaries then
run exact selected tests in isolated processes; this is not the full workspace.

The canonical multilane binding gate passes with zero diagnostics. All 117 Python
controls pass on 8,138 unchanged inputs: the 51 QueuePlan checks, 17 new Native
consumer checks and 49 parser/current-source/Validate controls. Mutations require
the shared Native call and its original outer authority, every route, exact rank,
pin/PoPs, closed-commitment exclusion, retained claim and ordinary deferral.
This is structural and mutation evidence, not a complete proof ledger, new
TLC/Verus run or real-peer qualification. Both input captures include the compiled
source-contract asset. Scoped Rustfmt 2024, retired-codec and diff checks pass;
no full workspace formatting claim is made. HEAD/index/branch stayed unchanged.

Raw receipts are under ignored `dist/sumeragi-main-work/`: `validation60.json`,
`build60-source-{before,after}.json`, `local-validation-controls60/summary.json`,
`formal-retained-route60/summary.json`, and `static60.json`. Failed59 and the
stack diagnostic remain separate receipts.

The Torii public/receiver routing preflight remains a separate integration gap:
a peer with an empty local Queue can refuse a closed route before consulting
its exact canonical pending binding. Already-owned durable retries use the
corrected Queue fallback, but the new handoff test proves the Queue API, not that
network ingress path. Canonical submission retries must reach authenticated State
custody before fresh routing is required.

Remaining work includes complete Native production consumption of preserved work,
exact Queue terminal cleanup before drain, and closure of all off-chain receipt
promises. In particular, a released Queue owner may still have a Kura Pending
terminal outcome; an empty Complete authorization list never permits State-only
cleanup of that owner. Preserve the release-to-Complete crash cut and original
reservation/finalization barriers. Per-committee promise custody plus canonical
carry/terminal disposition must settle delayed and partial acknowledgements.
Original Validate-to-Apply prepared ownership, full workspace qualification and
unchanged four/seven-validator fault/restart/final-transaction campaigns remain
open. No liveness milestone is complete.
