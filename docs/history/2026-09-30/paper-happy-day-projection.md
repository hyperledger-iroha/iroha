# SORA Nexus happy-day projection and observation costs

The 16-validator diagnostic completed naturally with native exit 101 after
653.795 seconds. Signed RS16 row acquisition progressed through setup, then
publication correctly rejected an executed lane-merge block because the borrowed
writer included execution-only inputs in the original proposal bytes. All owned
processes closed; source and image integrity passed. This is a failed diagnostic,
with zero accepted financial samples.

One checked borrowed projection now owns original-proposal counting, writing,
hashing, comparison and materialization. The two owned proposal APIs return
`Result`; impossible merge counts and misaligned contexts are errors. Consumers
propagate errors directly. Raw proposal ingress remains exact, and full execution
commitments still bind the appended lane inputs and outputs. There is no legacy
projection or compatibility wrapper. The Core payload encoder writes this borrowed
view without cloning the proposal graph.

The actual baseline model fails three of six paired controls; the correction
passes all six and nine extended controls. Compiled mutations that retain the
merged suffix or drop the global beacon are rejected. An actual production
storage-adapter harness reproduces publication and cold-read failures before the
fix, then passes all 61 controls, including substituted execution and source
negatives. Its explicit in-memory backend does not establish full-Core durability.

Each attached test peer also owns one reusable Alice client context. Clones share
its connection pools and blocking runtime; explicit custom-account construction
uses the same immutable network, operator and deadline policy. Both startup
status paths use that captured policy. Three isolated actual-production ownership
controls pass. Twelve alternating factory/clone component pairs report medians
297.682 ms and 2.854 microseconds, respectively, with all identity and policy
assertions retained. Cold initialization is 286.911 ms. This sends no requests and
starts no validators; background runtime teardown and unrelated host compilation
are explicit limits. It is not payment latency.

Evidence is retained under `dist/paper-performance-20260929/` in
`merged-proposal-projection1`, `merged-proposal-kura17`, `peer-client-owner17` and
`happy-day-profile2/diagnostic9-review`. The source join incorporates the latest
canonical merge; a fresh joined build, full-Core selected tests and successful
native happy-day execution remain required. Historical component evidence does
not qualify that new source population automatically.

## First-release status boundary

The latest-source release daemon build stopped at the telemetry producer: it
omitted 27 retired consensus fields that the shared status wire type still
required. The build exited 101 after 309.214 seconds with unchanged source and
HEAD; no harness or network was launched. The correction removes those fields
and their unused metrics, and keeps the 12 live node-wide observations. Detailed
consensus state uses the existing per-instance status endpoint. The changed DTO
has one canonical owner identity and rejects removed JSON fields. Its actual
codec regenerates both standalone and enclosing status frames. The metric
catalog retains checked row counts, registered counts, byte length and checksum,
regenerated from the new catalog. The shared library, current wire fixtures,
identity checks and schema projection pass
focused qualification. An actual producer probe first reproduced a catalog
length assertion, then passed unchanged after the four integrity constants were
regenerated. It checks live values, retired-field absence, the enclosing status
roundtrip and the governance bound. The full telemetry library compiles without
warnings. Both retired standalone and nested wire layouts are rejected.
Full daemon/harness and happy-day execution remain
pending; this change supplies no financial measurement.

The successor daemon build passed that boundary and exposed a missing parent
module declaration for the existing per-instance Sumeragi metrics implementation.
It exited 101 after 222.148 seconds with unchanged inputs; no network launched.
The parent now declares that implementation directly for its driver/node callers.
Fresh whole-node qualification remains pending.
