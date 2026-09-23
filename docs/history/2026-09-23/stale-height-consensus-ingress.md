# Retired-height consensus ingress and observer pressure

This is local development evidence from `/Users/takemiyamakoto/dev/iroha` on
`optimizations`. L1–L6 and release qualification remain open.

## Production failure and correction

The four-validator same-subject locked-reproposal test retained an authenticated
height-2 CommitCertificate while the peers finalized height 2 and activated
height 3. Releasing that exact old envelope made Core return generic
`WrongHeightContext` rejection. The daemon translated rejection into a fatal
downstream-delivery failure and peers exited even though the certificate's
height was already final. The retained red run is
`dist/sumeragi-main-work/generation250-message-control-network/same-subject-accepted-114-local1`.

`FairV2Ingress::try_push_owned_at` now classifies a structurally valid productive
message below the active finalized-height boundary as `Stale` before current
roster and capacity checks. This also covers a former validator after roster
rotation. It preserves the exact inbound owner and authenticated reply route
until the ingress lock drops. Active and future wrong-context messages still
reject; malformed protocol shape and fail-stop behavior remain distinct. The
daemon maps only `Stale` to the nonfatal retired relay outcome.

The Core tests cover old height, former validator, exact owner/reply retention,
and active/future rejection. The daemon test covers stale retirement without
weakening the active rejection outcome.

## Real-network evidence

The controlled daemon and ordinary integration harness were built from matching
captured Rust/manifest/fixture inputs on `optimizations`; each network runner
requires one real-network start attempt and retains peer stores and logs.

- `same-subject-final-local2` passes the four-validator same-subject
  locked-reproposal case (77.23 seconds, one test passed).
- `distinct-subject-admission-local3` passes the four-validator distinct
  PrepareQC case (196.08 seconds, one test passed). The committed height-2
  QueuePlan carries the exact complete admitted transaction as an attachment;
  its account instruction executes in the successor. The test checks both the
  committed attachment and eventual account visibility.
- The earlier nine-peer attempts exposed a fixed-view startup race, held-vote
  quorum timing, and a transient `/status` 503 in the single-read witness.
  Moving absence queries before pausing relays and retrying status reads within
  the original deadline reached the actual recovery boundary. The retained
  `observer-pressure-p2p-debug-local8` run then failed after 371.86 seconds:
  four validators advanced to block 3 while all five observers stayed at block
  1. Their first height-2 CommitQC requests reached validators before finality,
  when no response existed. Later same-request retries were absorbed by the
  incumbent exact-output fanout instead of producing fresh outbound frames.
  Independently, the logs showed a valid complete admission publication rejected
  by Norito's cumulative allocation limit (`2,228,224` bytes) inside its signed
  relay. This run did not establish observer catch-up.
- The discovery source now retires only the prior transport fanout before each
  retry and retains the same signed request for late authenticated responses.
  Admission-publication classification checks the raw byte-sequence count
  against the 1 MiB complete-input cap before decoding, permits the bounded
  owned relay graph in the cumulative allocation budget, and still enforces the
  wire cap. The signed-relay maximum and one-byte-oversize regression passes.
- On one unchanged `optimizations` source and joined daemon/harness binaries,
  `observer-pressure-retry-local9` passes the real four-validator/five-observer
  slow-reader case in 179.03 seconds. The test checks forced later-view
  finality, all nine peers' account visibility and exact committed block hash,
  cryptographic finality proof, and an exact 3-of-4 validator CommitQC excluding
  observers. The same binaries pass `same-subject-retry-local10` in 89.76 seconds
  and `distinct-subject-retry-local11` in 182.60 seconds. Each runner records one
  real-network start attempt, one pass, unchanged source/Git inputs and unchanged
  retained binaries.

The focused Core admission, retry and ingress controls pass; the current
canonical multilane structural gate, Rust formatting and diff checks pass.
These local development checks establish neither general liveness nor clean
release readiness. The full unchanged four/seven-validator loss, reordering,
backpressure, restart and final-transaction campaign remains open, as do two
older NPoS fail-stops with unattributed initiating errors.

The subsequent [Native source-observation refresh](native-source-observation-refresh.md)
records a silent-author fail-stop on this preliminary candidate, its retained
validation correction, a later source-joined network matrix, and one unresolved
controlled-drain timeout. The preliminary passes above do not qualify that later
source by themselves.
