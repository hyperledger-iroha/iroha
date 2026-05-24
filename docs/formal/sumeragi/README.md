# Sumeragi Formal Model (TLA+ / Apalache)

This directory contains bounded formal models for Sumeragi safety and liveness.

## Scope

`Sumeragi.tla` captures the commit path:
- phase progression (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- vote and quorum thresholds (`CommitQuorum`, `ViewQuorum`),
- weighted stake quorum (`StakeQuorum`) for NPoS-style commit guards,
- RBC causality (`Init -> Chunk -> Ready -> Deliver`) with header/digest evidence,
- GST and weak fairness assumptions over honest progress actions.

`SumeragiForkSafety.tla` captures same-height fork safety with two conflicting
branches:
- honest and Byzantine commit signer sets,
- permissioned count quorum and optional stake quorum,
- locked-QC gating for same-height branch replacement,
- honest single-vote discipline across branches,
- commit-certificate formation for each branch, plus a mutation that disables
  the single-vote/locked-QC guards and must produce a counterexample.

`SumeragiQuorumPolicy.tla` captures fail-closed quorum-policy arithmetic:
- permissioned count quorum requires a strict two-thirds supermajority plus
  one and rejects signer counts above the active validator count,
- NPoS stake quorum requires signed stake to strictly exceed two thirds of
  total stake,
- missing/negative stake, zero/negative total stake, over-total stake, exact
  two-thirds stake, and overflow all fail closed.

`SumeragiRbcDeliverQuorum.tla` captures the RBC deliver-quorum gate:
- the default deliver threshold equals the commit quorum over the deduplicated
  validator topology,
- topologies of one to three validators require all validators; larger
  topologies require `floor(2 * validators / 3) + 1`,
- the debug force-one path uses threshold one,
- READY counting uses distinct senders, so duplicate READY observations cannot
  inflate the deliver decision,
- deliver is impossible before the distinct READY count reaches the required
  threshold.

`SumeragiRbcCausalityGate.tla` captures implementation-side RBC message
causality:
- accepted INIT binds epoch, non-empty chunk metadata, roster hash, derived
  roster, existing-session evidence, header hash, header height/view, leader
  signature, chunk-digest root, layout, payload hash, chunk digests, and chunk
  root before a session and roster are recorded,
- chunks before INIT are stashed, bad-digest chunks are dropped, and local READY
  emission requires complete payload evidence plus a matching chunk root,
- remote READY before INIT is stashed, and recorded READY messages require
  roster, signature, and chunk-root validation; conflicting READY evidence
  invalidates the session and clears pending RBC state,
- DELIVER before INIT is stashed, duplicate DELIVER is ignored, accepted
  DELIVER requires signature and chunk-root validation, and embedded READY
  bundle entries seed state only after independent READY-signature validation.

`SumeragiPendingRbcStashGate.tla` captures the bounded pending-RBC stash used
before INIT/session context is available:
- chunk, READY, and DELIVER stashes respect per-session chunk and byte caps,
  and rejected/evicted frames are counted without making them replayable,
- new traffic refreshes the last-seen timestamp so TTL is measured from the
  latest pending frame rather than the first observation,
- TTL and session-limit housekeeping evict only inactive pending sessions,
  retain active-session stashes, and reject a new slot when only active slots
  fill the configured cap,
- eviction releases block-payload dedup entries, records metrics/drop counts,
  requests missing-block repair, and publishes the backlog snapshot,
- flushing after INIT replays only retained chunk/READY/DELIVER frames and
  removes the pending wrapper.

`SumeragiRbcSigningPreimageGate.tla` captures RBC READY/DELIVER signing
preimage construction:
- READY and DELIVER preimages bind chain id, Sumeragi mode tag, v1 message
  domain, block hash, height, view, epoch, roster hash, chunk root, and sender,
- READY and DELIVER domains cannot be interchanged,
- the mutable READY/DELIVER self-signature bytes stay outside their own
  preimages,
- DELIVER preimages bind the embedded READY-signature count and, when present,
  each entry's order, sender, signature length, and signature bytes.

`SumeragiClassicSigningPreimageGate.tla` captures classic Vote/VRF signing
preimage construction:
- Vote, VRF commit, and VRF reveal preimages bind the consensus domain
  protocol, chain id, Sumeragi mode tag, protocol version, v1 preimage version,
  and their message type tags,
- votes bind block hash, parent and post-state roots, height, view, epoch,
  chain-order hash, rechain sequence, and phase,
- votes bind either the highest-QC absence flag or the full highest-QC
  reference; the two encodings cannot be interchanged,
- VRF commits and reveals bind epoch, signer index, and commitment/reveal bytes,
- mutable vote, VRF, aggregate-signature, and signer-bitmap bytes stay outside
  the signing preimages.

`SumeragiClassicSignatureGate.tla` captures classic Vote/QC signature
verification:
- QC admission binds the expected mode tag and validator-set hash/body before
  signer or aggregate verification can succeed,
- signer bitmaps must have canonical length, stay inside the active roster,
  select a non-empty signer set, and satisfy count or strict NPoS stake quorum,
- aggregate verification requires a non-empty aggregate signature, available
  signer PoPs, and a valid BLS aggregate over the canonical QC preimage,
- permissioned QCs require all bitmap-selected votes locally, while NPoS QCs
  may tolerate missing local vote bodies after aggregate/stake verification,
- local votes must match the QC subject and state roots, have valid signatures,
  map from canonical to view-specific signer indices, and agree with NewView
  highest-QC context,
- accepted QCs return exactly the bitmap-selected voting signers, and rejected
  QCs return no signers.

`SumeragiVrfMessageAdmissionGate.tla` captures VRF commit/reveal admission:
- VRF commit and reveal handlers require a supported consensus mode, an active
  epoch manager, a signer inside the current topology, a non-empty signature,
  and successful verification over the canonical VRF preimage,
- accepted commits/reveals must match the epoch, active roster, and
  commit/reveal windows before staging a VRF epoch snapshot,
- duplicate same-value observations are accepted, but commitment, reveal, and
  late-reveal rewrites are rejected,
- reveals require a prior matching commitment; late reveals may be accepted as
  penalty-clearing observations without refreshing the current PRF context,
- only externally originated accepted observations are rebroadcast, and local
  VRF state is updated only for the local validator's accepted observation.

`SumeragiVoteAdmissionGate.tla` captures classic inbound vote admission:
- early height/view, lock, and missing-roster gates fail closed before vote
  evidence, QC aggregation, or pending-progress state can mutate,
- duplicate votes, non-NEW_VIEW highest-QC references, chain-order mismatches,
  invalid signatures, and malformed NEW_VIEW highest-QC references are
  rejected without recording the incoming vote,
- same-signer same-slot conflicts are rejected with double-vote evidence,
  deferred without side effects while supersession context is missing, or
  accepted only when a newer QC/local quorum proves supersession,
- cross-phase conflicts can record the new vote but must also persist
  double-vote evidence,
- accepted PREPARE/COMMIT votes cache rosters and request commit-pipeline
  progress, while NEW_VIEW votes do not cache rosters; stale NEW_VIEW votes can
  aggregate a QC but must not update the proposal tracker or request the commit
  pipeline.

`SumeragiProposalHintAdmissionGate.tla` captures inbound proposal-hint
admission:
- stale height/view hints, malformed highest-QC references, cached duplicates,
  committed-edge conflicts, local metadata mismatches, and locked-QC conflicts
  fail closed,
- missing future highest-QC dependencies are dropped as accepted metadata while
  exact block repair is requested; cross-view hints may be cached only as
  dependency context and same-view hints are not cached,
- accepted hints update PRF context, cache the hint, mark the slot observed,
  replay deferred votes, and prune old observed slots,
- highest-QC state changes only for newer references or same-slot Commit phase
  promotion; lock-lag catchup defers that update while preserving accepted hint
  side effects.

`SumeragiProposalAdmissionGate.tla` captures inbound proposal metadata
admission:
- stale height/view proposals, mismatched proposal epochs, malformed highest-QC
  references, parent/highest-QC mismatches, committed-edge conflicts, local
  metadata mismatches, and locked-QC conflicts fail closed,
- missing future highest-QC dependencies arm exact repair and a defer marker
  without caching the proposal or marking the slot observed,
- accepted proposals update PRF context, sample leader context, mark the slot
  observed, cache the proposal, replay deferred votes, and prune old observed
  slots,
- proposal metadata alone must not wake the commit pipeline or record
  payload-phase progress.

`SumeragiBlockCreatedAdmissionGate.tla` captures direct `BlockCreated` payload
admission:
- local-removed, stale-height/view, lock-rejected sink, authoritative-owner
  conflict, empty-payload, hint-mismatch, locked-QC, proposal-preserve, and
  RBC-payload mismatch gates fail closed before pending payload mutation,
- duplicates refresh payload state without taking ownership, missing-highest
  hints preserve replay material while arming dependency repair, and
  pending-processing or commit-inflight slots preserve the body for replay,
- future-height payloads request parent or gap repair before admission
  continues,
- accepted payloads update pending state, mark the payload phase, clear
  relevant missing-block requests, and wake the commit pipeline; proposal
  context is cached only for accepted inline proposal material.

`SumeragiQcSignerBitmap.tla` captures QC signer-bitmap admission:
- bitmap length must match the topology-derived byte length,
- signer bits outside the topology are rejected,
- only signer indices inside the voting validator set count toward quorum,
- observer or padding indices cannot satisfy quorum on behalf of voting
  validators,
- accepted QC signer evidence must match the voting-set quorum predicate.

`SumeragiCommitRootConsistency.tla` captures commit-QC execution-root
consistency:
- commit votes are filtered into one same-root group before quorum is
  evaluated,
- permissioned mode selects the largest same-root signer group with a
  deterministic low-root tie-break,
- NPoS mode selects the heaviest same-root stake group with the same tie-break,
- wrong-context votes cannot help satisfy quorum,
- QC validation rejects signer votes whose roots do not match the QC roots.

`SumeragiCommitPipelineRecoveryGate.tla` captures adapter-side commit-pipeline
recovery ordering:
- cached commit-vote quorums must be aggregated into a local commit QC before
  peer missing-QC recovery is armed,
- the locally formed commit-QC marker must stay attached to the pending block,
- missing commit-QC recovery is armed only for a valid, payload-local, stale,
  locally voted pending block that still extends the committed tip,
- fresh local votes, existing commit QCs, missing local DA payloads, invalid
  pending blocks, missing local votes, and off-tip candidates do not arm peer
  recovery,
- cached near-quorum commit votes are rebroadcast to quorum missing-signer
  targets, not the proposal collector subset, and empty/committed vote sets do
  not rebroadcast.

`SumeragiCommitPipelineSchedulingGate.tla` captures commit-pipeline scheduling
before recovery/finalization candidates are processed:
- ticks enter the commit pipeline only when active candidates, commit inflight
  work, or an explicit commit wakeup exists; queue saturation alone does not
  run the pipeline,
- event-triggered entry is unconditional unless the local budget is already
  exhausted, and backlog observation cannot suppress or invent candidate work,
- recovery candidates are included only for explicit commit wakeups, queue
  saturation, or active candidates with commit-certificate evidence,
- the shared tick budget is bypassed only for active pending work under commit
  wakeup or queue saturation, and budget exhaustion re-arms the commit wakeup
  without processing candidates,
- idle-view repair preserves its budget only when a woken commit pipeline has
  active pending candidates.

`SumeragiCommitResultDrainGate.tla` captures asynchronous commit-result
draining:
- worker results are applied only when their result id matches the active
  inflight commit,
- id-mismatched results are ignored while restoring the real inflight job, and
  ownerless results are ignored without recording progress,
- disconnected result channels clear worker state and run inline fallback only
  when an inflight commit exists,
- inline signature-index recovery is allowed only when the local peer is
  outside the commit topology and a commit QC is available,
- summaries, progress, and pacemaker kickstart are recorded only after an
  accepted result is applied, and kickstart is limited to durable commit
  outcomes.

`SumeragiCommitJobDispatchGate.tla` captures commit-job dispatch ownership:
- a duplicate finalize for the same inflight block is suppressed without
  requeueing pending work,
- a different block encountered while another commit is inflight is retained
  in the pending map without replacing the current inflight owner,
- a ready commit worker receives ownership without inline execution on the
  actor thread,
- a full worker queue returns without blocking, keeps the block pending, and
  leaves no inflight marker behind,
- missing worker channels and disconnected sends fall back to inline execution,
  with disconnected sends also clearing commit worker state,
- every dispatch path leaves the block recoverable through exactly one owner:
  existing inflight, worker queue, pending retry, or inline commit.

`SumeragiCommitInflightTimeoutGate.tla` captures commit-inflight timeout
reporting:
- a timeout reports only when a nonzero timeout is configured, an inflight job
  exists, elapsed time is at or beyond the boundary, and the job has not
  already been reported,
- already-reported jobs keep their timeout marker but do not duplicate status
  or warning diagnostics,
- reporting preserves the inflight owner so a late worker result remains
  attachable,
- timeout reporting does not requeue or abort pending work, prune proposal
  state, force or advance a view, record a commit failure, apply an outcome, or
  kickstart the pacemaker.

`SumeragiPostCommitPacemakerKickGate.tla` captures post-commit pacemaker
kickstart gating:
- durable commits kick the pacemaker only when queued transaction work remains,
- healthy proposal backpressure and pacing-only pressure from queue saturation
  or consensus ingress backlog still allow the kick,
- active pending blocks, RBC backlog, and relay backpressure suppress the kick,
  even when pacing pressure is also present,
- the helper returns whether it attempted the kick, not the callback's result.

`SumeragiIdleViewProposalBudgetGate.tla` captures proposal-side idle-view
budget preservation:
- due proposals with queued work can preserve the idle-view repair budget only
  when no mode flip or commit job is in flight,
- healthy proposal state and pacing-only queue/consensus pressure preserve the
  budget for proposal work,
- active pending blocks, RBC backlog, and relay backpressure keep idle-view
  repair available, even when pacing pressure is also present,
- if idle repair was skipped for a due proposal, the post-proposal retry runs
  only while queued work remains, the frontier is empty, and no commit job owns
  the frontier.

`SumeragiPacemakerEvaluationGate.tla` captures pacemaker evaluation:
- initial deferral logging fires only on the first backpressure transition and
  is not repeated for subsequent deferring ticks,
- `Pacemaker::should_fire(now)` is checked before backpressure branching, so a
  due deadline advances even when hard backpressure suppresses proposal work,
- pacing-only backpressure attempts a due proposal and logs the fire deferral,
  while before-deadline pacing only logs the first deferral transition,
- active pending blocks, RBC backlog, and relay pressure suppress proposal
  attempts even when the deadline is due,
- recovered pressure clears the backpressure tracker without deferral logging.

`SumeragiCachedSlotTimeoutGate.tla` captures cached proposal-slot timeout
selection:
- near-commit-quorum payload repair may shorten the cached-slot timeout only
  when one more precommit would satisfy quorum, local data is missing, and
  consensus queue/RBC backlog are both absent,
- the shortened timeout is capped by the ordinary quorum timeout,
- zero votes, far-from-quorum votes, already-satisfied quorum, missing-data
  absence, consensus backlog, and RBC incompleteness keep the ordinary timeout,
- repeated NPoS cached-slot timeouts compute a next streak from same-height
  newer-view history and cap the hysteresis factor,
- permissioned mode, zero quorum timeout, absent history, height mismatch,
  non-advancing views, and elapsed boundary/after-boundary cases do not wait.

`SumeragiProposalParentResolutionGate.tla` captures proposal parent resolution
and inline frontier backup transport:
- proposal heights zero and one do not resolve a previous block or defer for a
  missing parent,
- Kura-backed previous blocks take precedence over pending parent material,
- pending fallback is used only when the pending block is keyed by the
  highest-QC subject and its height is exactly one below the proposal height,
- `usize` conversion overflow during Kura lookup logs the overflow but still
  permits a matching pending-parent fallback,
- missing parents above genesis defer proposal assembly without draining the
  transaction queue,
- DA inline backup transport is seeded only when DA is enabled, the frontier
  `BlockCreated` frame is inline, and inline backup is configured; RBC body
  transport is used for DA primary RBC or that inline backup path.

`SumeragiPrecommitQcViewChangeGate.tla` captures the precommit-QC selector
used by pacemaker view-change handling:
- the local highest QC is filtered to Commit phase before it can seed NewView
  context,
- non-Commit highest QCs fall back to the committed QC when one exists and
  otherwise select no precommit QC,
- with only a Commit-phase highest QC, that highest QC is selected,
- with only a committed QC, the committed QC is selected,
- when both Commit-phase candidates exist, `(height, view)` ordering selects
  the local highest QC exactly when it is lexicographically greater than or
  equal to the committed QC, including equal-slot ties.

`SumeragiCommitEvidenceReplayGate.tla` captures known-block commit-evidence
replay pacing:
- inactive pending blocks, wrong-round calls, aborted pending blocks, cooldown
  hits, zero-evidence states, and local-only target sets cannot emit replay
  traffic,
- first evidence, vote-count progress, commit-QC progress, view progress, and
  stalled positive evidence after cooldown may replay,
- vote evidence is replayed as `QcVote` and commit-QC evidence as `CommitCert`,
- replay never falls back to `BlockCreated` payload broadcasts or
  `BlockSyncUpdate` hydration,
- explicit replay targets exclude the local peer and are deduplicated before
  outbound work is scheduled.

`SumeragiBlockSyncRecoveryGate.tla` captures BlockSyncUpdate recovery
admission into the BlockCreated owner path:
- stale-view updates require a missing-block request, cached commit evidence,
  or an explicit commit-QC repair mode,
- payload-only recovery may hydrate or retain a branch, but cannot steal
  authoritative same-height/frontier ownership or clear stale commit inflight,
- commit-QC/certified recovery revives aborted placeholders, keeps commit-QC
  evidence attached, supersedes stale same-height owners, and clears stale
  commit inflight,
- sparse next-height payloads and vote-only unknown-frontier updates track
  missing commit-QC repair,
- unvalidated commit-QC sidecars cannot promote lock or highest-QC state.

`SumeragiCertifiedBlockFetchGate.tla` captures direct certified-block fetch
recovery:
- only commit-QC evidence can request an exact certified block, and request
  targets are selected from QC signers with topology fallback, local-peer
  removal, sorting, and deduplication,
- direct certified fetch uses consensus high priority and does not regress to
  generic missing-block fetch traffic,
- service-side requests reject forged requester identities, missing local
  blocks, mismatched local subjects, missing commit QCs, and mismatched commit
  QCs,
- NPoS certified responses carry a stake snapshot aligned to the certified
  validator set,
- oversized responses split into proof/body companions or bounded fallbacks
  without sending an oversized full response,
- full responses and proof companions self-validate block height/view, QC
  subject, QC height/view, certification fields, and validator checkpoint
  metadata before the QC is cached,
- body companions materialize only after a matching accepted proof, invalid
  inflight or pending owners remain rejected, retry-aborted pending blocks are
  revived, and successful materialization clears recovery deferrals before
  waking the commit pipeline.

`SumeragiMissingBlockFetchGate.tla` captures QC-first missing-block fetch
planning:
- missing-block request state is keyed by block hash, and conflicting heights
  for the same hash cannot rewrite the original request identity,
- views and phases only advance, while stale-view updates are ignored,
- consensus priority upgrades retry immediately and own the recovery windows;
  equal-priority retries use the most aggressive window before the first fetch
  and preserve widened backoff after attempts begin,
- explicit view-change deferral can clear an armed same-priority view-change
  window,
- attempts increment only when a fetch is emitted, and known local blocks do not
  defer QC aggregation,
- default mode prefers signer targets before the fallback limit, falls back to
  topology targets afterward or when no signers are usable, strict-signer mode
  fails closed without signer targets, aggressive mode uses topology targets,
  and out-of-range signer indexes do not produce targets,
- final send helpers remove the local peer and fetch-request builders preserve
  requester/hash/height/view/priority/roster-proof/commit-QC-only fields.

`SumeragiMissingBlockHardCapGate.tla` captures missing-block height hard-cap
recovery escalation:
- no view change is emitted before the hard cap, while an active contiguous
  consensus-priority request rotates once the hard-cap guard and stall-window
  reservation allow it,
- recent dependency progress, recent RBC progress, in-flight range-pull
  progress, deterministic tier advancement, and explicit view-change deferral
  suppress hard-cap rotation while recovery can still converge,
- already-triggered, non-active-height, non-contiguous-height,
  background-priority, advanced-current-view, and already-escalated-view states
  do not rotate,
- the lock-lag override can rotate without the ordinary stall-window
  reservation but still respects priority, current-view, duplicate-trigger,
  and already-escalated guards,
- trigger paths record the escalated view, latch the request view, and use
  `MissingPayload` as the view-change cause,
- no-actionable recovery clears the height budget and returns no progress, and
  range-pull-only progress cannot become a view change before the hard cap.

`SumeragiMissingBlockHardCapCleanupGate.tla` captures missing-block hard-cap
cleanup preservation:
- hard-cap cleanup preserves the contiguous frontier only when the height is
  exactly `committed + 1` and live material is present,
- live material can be active frontier ownership, a valid pending/inflight block
  extending the tip, a valid RBC session, or pending RBC work,
- invalid or non-tip pending material cannot keep the cleanup in preserve mode,
- live hard-cap cleanup skips same-height pruning, keeps same-height
  hint/proposal/seen metadata, keeps frontier recovery state, and preserves
  valid same-height RBC sessions and pacing metadata,
- invalid same-height RBC state and all future pending/missing/RBC state are
  still pruned,
- quorum-backed missing-payload repair keeps same-height missing requests and
  all same-height RBC state, while no-live cleanup prunes stale same-height
  pending state and clears recovery windows,
- hard-cap cleanup keeps frontier NewView evidence and same-view authoritative
  ownership, while quorum-timeout cleanup without live evidence drops stale
  same-view ownership.

`SumeragiMissingBlockViewChangeGate.tla` captures missing-block view-change
escalation:
- only consensus-priority missing-block requests can arm a view change,
  missing or zero dwell windows fail closed, and boundary dwell/last-trigger
  times are accepted,
- a current-view latch suppresses repeated escalation while a prior-view latch
  does not block the new view,
- `mark_view_change_if_due` records the current view and trigger timestamp only
  when the request is due,
- `clear_missing_block_view_change` clears the window and triggered-view latch
  without removing the tracked request,
- scheduler deadlines include armed untriggered view-change windows and skip
  current-view latches, missing windows, and zero windows,
- recent dependency progress, recent RBC progress, in-flight range-pull
  progress, and active backlog windows defer escalation, while stale progress
  and expired backlog windows do not.

`SumeragiNativeAmxAttestationGate.tla` captures native AMX proposer-side
prepare/commit attestation gating:
- non-AMX plans return no receipt, and native AMX plans without a BLS-capable
  roster fail closed,
- prepare quorum must exist before commit requests are broadcast,
- every participant leg must have both prepare and commit QCs before proposal
  assembly seals a native AMX receipt,
- invalid duplicate, wrong-body, or outsider vote sets cannot build QCs,
- vote projection is deterministic in validator-set order, and retried bodies
  plus distinct participant legs stay separate in the vote cache.

`SumeragiNativeAmxJournalReplay.tla` captures native AMX queue-plan journal
replay across restart:
- full native AMX routing plans, entrypoints, and gossip payloads are replayed
  rather than collapsed into single-lane records,
- tombstones are scoped by `(signed_transaction_hash, plan_digest)`, so removing
  one digest cannot delete a re-admitted transaction with a new plan,
- unsupported journal record versions are ignored,
- duplicate puts for the same key keep the last record,
- compaction preserves exactly the live records,
- torn payload or length tails are repaired while preserving the last complete
  native AMX record.

`SumeragiNativeAmxRoutingPlanGate.tla` captures native AMX routing-plan
canonicalization and execution-context projection:
- multi-dataspace and universal-coordinator targets remain native AMX plans,
  while single-route targets remain single-route plans,
- participant legs are sorted by `(dataspace_id, lane_id)`, exact duplicate
  legs are deduplicated, and distinct lanes in one dataspace remain distinct,
- coordinator and participant roles are forced before durable block projection,
- single and native plan digests use separate domains, and native digests bind
  the coordinator plus canonical participants while ignoring input order,
- execution contexts project coordinator route, plan digest, and coordinator-
  first route legs, and block validation rejects mutated coordinator, digest, or
  leg contexts,
- native AMX contexts require receipts, single-route contexts reject receipts,
  and unresolved participant lanes fail closed.

`SumeragiNativeAmxReceiptValidation.tla` captures native AMX receipt admission
inside block execution-context validation:
- native AMX contexts must carry a receipt, single-route contexts must not, and
  receipts must be attached to signed transaction entrypoints,
- receipt version, source hash, coordinator route, block height, and plan digest
  must match the transaction and routing plan,
- participant legs must match the native AMX plan exactly, with no missing,
  unexpected, or duplicate participant records,
- prepare/commit QC bodies must match the receipt, participant leg, expected
  phase, entrypoint hash, plan digest, coordinator, and planned height,
- validator-set hash/version, dataspace committee size, signer bitmap bounds,
  BLS signer eligibility, live proof-of-possession state, quorum, and aggregate
  BLS signature validation all fail closed.

`SumeragiNativeAmxIngressGate.tla` captures native AMX control-plane ingress:
- prepare/commit attestation requests reply only when the body phase matches the
  request kind, the local consensus key is BLS-normal, and the local key has a
  live proof-of-possession at the planned coordinator height,
- request replies are addressed back to the sender, use the requested phase,
  are signed by the local peer, and sign the exact request body,
- inbound votes are cached only when the signer is BLS-normal, has a live and
  valid proof-of-possession, and the BLS signature verifies over the canonical
  body preimage,
- duplicate same-body signer votes do not create duplicate cache entries, while
  retried bodies and distinct participant legs stay separately cacheable.

`SumeragiVNextChainOrderGate.tla` captures vNext chain-order helper
construction:
- `ChainOrder::new(...)` rejects empty orders, zero critical prefixes,
  critical prefixes longer than the order, and quarantine tails outside the
  validated `[critical_prefix_len, order_len]` range,
- accepted orders expose exactly the critical prefix and never include the
  quarantine tail in `critical_path()`,
- `successor_of(...)` returns only the next critical-path validator and returns
  none for critical-tail, quarantine-tail, and unknown peers,
- `QuorumPolicy::smallest_satisfying_prefix_len(...)` returns the first count
  or strict-stake prefix that satisfies quorum and returns none for impossible,
  missing-weight, or zero-total-stake inputs,
- `build_signer_bitmap(...)` uses the canonical byte length, rejects duplicate
  signer indices, and rejects out-of-range signer indices.

`SumeragiVNextRechainGate.tla` captures the quarantined vNext re-chain helper:
- suspicion evidence must match the current slot, chain-order hash, and
  re-chain sequence before it can affect the deterministic chain order,
- only successor-scoped accusations are admitted; tail accusers,
  non-successor accusations, duplicates, and evidence that stops being
  successor-scoped after earlier canonical evidence fail closed,
- accepted evidence moves both accuser and accused validators to the quarantine
  tail, keeps the critical path free of tainted validators, increments the
  re-chain sequence once per applied suspicion, and changes the chain-order
  hash,
- count and stake quorum policies are rechecked after quarantine, with exact
  two-thirds stake still rejected by the strict NPoS comparison.

`SumeragiVNextSignatureGate.tla` captures vNext aggregate certificate
verification:
- re-chain certificates must have slot, chain-order hash, and re-chain sequence
  fields consistent with their embedded `ChainOrder` before aggregate
  verification runs,
- both re-chain and view-change certificates reject missing aggregate
  signatures, empty signer rosters, malformed signer bitmaps, out-of-range
  bitmap bits, empty signer sets, and signer proof-of-possession length
  mismatches,
- selected signers must satisfy the configured count or stake quorum, and exact
  two-thirds stake remains insufficient,
- aggregate verification accepts only BLS-normal signer keys with a valid
  aggregate signature over the canonical certificate preimage,
- accepted certificates return exactly the bitmap-selected signer set, and
  rejected certificates return no signers.

`SumeragiVNextSigningPreimageGate.tla` captures vNext signing preimage
construction:
- re-chain votes and re-chain certificates share the same certificate-body
  preimage, and view-change votes and view-change certificates share the same
  view-change body preimage,
- every aggregate preimage binds chain id, message type, vNext version, mode
  tag, and the complete certificate body,
- aggregate signatures, vote signatures, and signer bitmaps are excluded from
  the signed body,
- unsigned vote helpers project only certificate body fields, signer, and empty
  signature state,
- suspicion signing-body hashes include the canonical suspicion evidence fields
  while excluding the signature.

`SumeragiVNextControlIngressGate.tla` captures actor-level vNext control
certificate ingress:
- re-chain certificates received before a round is installed are retained
  without mutating live round state or requiring a view change,
- already-current re-chain certificates are side-effect-free no-ops,
- re-chain certificates with mismatched previous chain-order hash or evidence
  are rejected without installation, round mutation, or escalation,
- deterministic re-chain certificates within the configured taint bound update
  the round chain order, record re-chain progress, and install the certificate,
- deterministic re-chain certificates that exceed the taint bound or would
  weaken quorum require a live view change instead of installing or updating
  the round,
- required view changes clear vNext validation worker ownership, sign a local
  view-change vote when possible, and trigger live view-change handling,
- view-change certificates always install through the non-canonical diagnostic
  path, abort only an installed highest slot, and trigger only nonzero new
  views.

`SumeragiVNextSlotLifecycleGate.tla` captures actor-owned vNext slot lifecycle:
- proposal, availability, validation, and commit-persisted events cannot
  install or progress a slot without an installed round,
- proposal and availability events preserve committed slots,
- validation dispatch requires an installed, non-committed, unqueued slot and
  records the running validation owner,
- matching worker-start and queue-full events mutate only their owned
  validation state, while stale events are side-effect free,
- matching valid results prepare and accept the slot, and matching invalid
  results abort and reject the slot,
- stale or terminal validation results are ignored,
- deferred validation resets only non-committed slots,
- recovery starts only for due, unprotected running or backpressured timeouts,
  and recovery does not emit validation result side effects,
- commit-persisted events make the slot sticky-committed and record progress.

`SumeragiVNextValidationGate.tla` captures vNext validation ownership:
- unqueued validation dispatches a worker, queued validation awaits a worker,
  and terminal valid/invalid states accept or reject without dispatching or
  raising suspicion,
- running and backpressured states raise suspicion exactly at or after the
  configured suspicion timeout, while pre-timeout states await/backpressure,
- elapsed-time calculation saturates when the sampled time is before the
  recorded worker start time, avoiding underflow-driven suspicion,
- worker start records the running owner identity, and only matching
  `(id, generation)` worker results may apply,
- wrong-id, wrong-generation, and non-running worker results are ignored
  without mutating validation state.

`SumeragiVoteVerifyAsyncGate.tla` captures actor-side async vote verification:
- no-worker dispatch falls back to inline signature verification instead of
  queueing unowned work,
- duplicate votes already in flight or pending are dropped without adding a
  second worker owner,
- successful dispatch and pending retry install exactly one in-flight owner,
  while full or unavailable worker lanes keep work pending for a later retry,
- worker results apply votes only for matching in-flight ids with valid
  signatures,
- no-in-flight, mismatched-id, stale-view, locked-precommit, penalized-signer,
  and invalid-signature results cannot mutate consensus state,
- a disconnected result channel clears worker senders plus in-flight and
  pending work and does not retain the dead receiver.

`SumeragiQcVerifyAsyncGate.tla` captures actor-side async QC aggregate
verification:
- verified-cache hits, small committees, forced inline checks, missing aggregate
  inputs, full worker queues, and unavailable workers do not create worker
  ownership and continue through inline QC handling,
- successful consensus-QC and known-block QC dispatches install exactly one
  in-flight aggregate-verification owner and do not mutate consensus state
  before the worker result returns,
- duplicate in-flight QCs are suppressed without adding another worker owner,
- known-block recovery drops stale locked QCs before aggregate-verification
  dispatch,
- worker results apply only when the in-flight key and id match, and they route
  consensus QCs back through `handle_qc_with_aggregate(...)` while known-block
  QCs re-enter `apply_known_block_qc_work(...)`,
- disconnected worker senders or result receivers clear worker-owned state and
  fall back to inline verification rather than keeping stale ownership.

`SumeragiWorkerDrainSchedulerGate.tla` captures worker-loop drain scheduling:
- vote queues stay ahead of payload tiers while the bounded vote burst remains
  available, and payload tiers regain service once the burst is exhausted,
- frontier body-repair work preempts ordinary vote preference, while
  quorum-recovery vote drain can override starved payload work but not block
  backlog escape,
- a vote-only pre-tick drain that reaches the time budget grants one non-vote
  payload/RBC turn before breaking,
- block-only urgent backlog and starved block work escape vote bias,
- low-priority consensus/control work is selected after high-priority queues are
  empty,
- every handled envelope records queue drain, consumes budget, and marks phase
  progress,
- result polling and external-hint sync run before tick decisions, busy ticks
  use the busy gap, explicit wakeups bypass the tick gap, and budget-exhausted
  pre-tick drains suppress post-tick work.

`SumeragiWorkerBudgetAdaptiveGate.tla` captures worker-loop budget and
adaptive-cap helpers:
- worker iteration time budgets are anchored to the block/commit cadence,
  floor at a bounded minimum, and respect global/configured caps,
- vote drain budgets may use DA quorum windows and multipliers, but still
  respect per-iteration and configured caps and never collapse to zero,
- generic drain-budget and tick-gap helpers preserve floor/max relationships,
- block backlog depths map deterministically to zero/small/medium/large/huge
  cap tiers,
- vote backlog throttles block-payload work without reducing RBC ingress, and
  block backlog throttles blocks plus payload/RBC repair caps with a minimum
  payload/RBC floor.

`SumeragiWorkerIngressRoutingGate.tla` captures worker ingress routing and
parallel worker execution envelopes:
- inbound block/control/lane/background messages are routed to the intended
  worker queues with matching enqueue metadata and queue accounting,
- blocking and nonblocking enqueue paths wake, record enqueue/drop status, and
  account blocking sends consistently,
- each queue worker uses the intended actor-gate priority, worker stage, and
  handler family for its queue,
- vote and RBC workers retain their bounded parallel batch limits while other
  queue workers drain one message at a time,
- queue workers enter the gate before actor handling, set the stage before
  handling, poll worker results after each handled message, record the drain,
  stop on empty queues, and restore idle only when the last active worker
  leaves.

`SumeragiNposVrfEpochSealGate.tla` captures NPoS VRF epoch-seal staging and
committed-effect reconciliation:
- epoch records must retain immutable header fields, canonical commitments,
  reveal values, penalty heights, penalty markers, and finalized offender
  state when compatible records are merged,
- finalized offenders are sticky, unfinalized offender candidates are stripped
  from merged records, and offender overlap with the epoch roster is rejected,
- record update heights advance monotonically while existing observations are
  preserved and incoming observations are added,
- validator-election outcomes are sticky once present and cannot be rewritten,
- pending staged records are dropped when already covered by committed state,
  extended only with compatible committed progress, and replaced by better
  committed snapshots when pending state is incompatible,
- committed NPoS VRF block effects reconcile pending state with canonical
  committed records and reject stale, regressive, or conflicting effects,
- elected rosters activate only when the configured activation margin has
  elapsed,
- block-level NPoS VRF effect validation rejects penalty heights without
  markers, duplicate participants, duplicate offenders, offenders outside the
  epoch roster, and finalized offender sets that retain active validators.

`SumeragiKuraCommitRetryGate.tla` captures Kura durability and commit retry
handling:
- Kura/state alignment succeeds only when Kura has the block, the state tip hash
  matches the pending block, and the committed state height covers the pending
  height,
- retry backoff keeps the pending block, records a scheduled retry, and avoids
  block/evidence cleanup,
- retry exhaustion and Kura-aborted pending blocks remove unsafe pending state,
  clean block-scoped consensus evidence, reset lock/highest-QC anchors, and
  trigger a commit-failure view change,
- already durable blocks are marked persisted and reset retry state before the
  commit replay continues,
- already committed duplicates are dropped while only settled RBC and parent-QC
  evidence are cleaned,
- state-commit height mismatches distinguish duplicate aligned state from
  conflicting advanced state, requeueing transactions and clearing proposal
  cache state only for the conflict branch,
- non-height state-commit failures keep the block pending with Kura persistence
  recorded so replay does not append duplicate durable bytes,
- missing commit QCs, non-extending tips, and uncertified aborted or retired
  blocks defer finalization, while QC-certified aborted/retired blocks can
  proceed.

`SumeragiRestartReplayGate.tla` captures restarted-peer replay and
snapshot/Kura consistency:
- snapshot digest, signature, Merkle metadata, and chain id must verify before
  a snapshot state can be accepted,
- snapshot height cannot exceed the durable Kura height, and nonempty snapshots
  must carry durable Offline Note V2 replay keys,
- normal restart requires local Kura block bodies for every snapshot hash,
  while hard-fork bootstrap may use the durable hash journal but still rejects
  missing or mismatched hashes,
- non-tip hash mismatches fail closed, while the latest-block mismatch path is
  accepted only after reverting the inconsistent latest snapshot changes,
- legacy snapshots missing the Space Directory manifest section replay
  manifests from Kura or reject if replay fails,
- snapshot writes require the state height and latest block hash to be backed
  by Kura and publish through the temporary file/digest/signature/Merkle
  promotion path,
- canonical replay checkpoints redact consensus sidecars and normalize MV-cell
  history and set-like key-policy fields while preserving committed ledger WSV
  mutations.

`SumeragiPostCommitCleanupGate.tla` captures post-commit cleanup and
stale-evidence pruning:
- undelivered DA-backed RBC sessions for the committed block remain retained
  while settled, invalid, or no-DA committed RBC runtime state is drained,
- only pending descendants that extend the committed tip survive cleanup;
  divergent and unknown-parent descendants are dropped and requeued,
- already committed pending duplicates, including Kura-backed duplicates, are
  dropped without requeueing transactions,
- stale pending, validation, RBC, QC, proposal, missing-block, slot, forced
  view, vote, and recovery state at or below the committed height is pruned,
- committed-hash QC evidence and active vote windows are preserved,
- missing-block clears are fail-closed for non-obsolete payload-unavailable
  requests while obsolete clears are allowed,
- committed-edge conflict cleanup preserves canonical frontier evidence but
  prunes recovery/cooldown state when no such evidence exists.

`SumeragiFrontierGapRealignGate.tla` captures post-commit frontier-gap
realignment and committed-anchor range-pull pacing:
- post-commit realignment requires future recovery evidence strictly beyond
  the contiguous frontier and skips when the tip-extending frontier payload is
  already local,
- exact-body frontier repair suppresses generic range pulls unless the deep
  catch-up gate admits a broader reanchor,
- canonical frontier reanchors use the previous/latest committed anchor pair
  when possible, while non-canonical pulls use the latest/latest anchor,
- range-pull targets fall back from voting roster to commit topology to
  trusted peers, then remove the local peer and sort/deduplicate the result,
- empty targets, per-peer cooldown hits, zero-send outcomes, already-emitted
  shared windows, recovery-FSM suppression, and stride-mismatched windows
  suppress emission,
- successful emissions record direct response permits, per-peer cooldowns,
  canonical window marks, dependency watermarks, metrics, and high-priority
  canonical-next-height recovery traffic.

`SumeragiPrecommitVoteGate.tla` captures local precommit vote emission:
- pending blocks must be validated before the local node signs a precommit,
- observers and peers outside the view-aligned voting topology cannot sign,
- duplicate same-slot votes and unsuperseded same-height conflicts are
  rejected,
- a newer conflicting branch may be signed only when it is superseded by
  accepted new-view evidence or the local vote completes the newer-view quorum,
- older conflicting branches cannot use quorum-completion as an escape hatch,
- locked-QC conflicts, missing locked payloads at the same or older view, and
  non-extending locked-chain candidates fail closed.

`SumeragiProposalAssemblyGate.tla` captures local proposal assembly before
prepare voting:
- observers and non-leaders cannot assemble fresh proposals,
- active local same-height vote conflicts and pending same-height vote
  verification defer proposal assembly without mutating proposal-cache state,
- missing highest-QC payloads and non-extending highest-QC ancestry defer
  proposal assembly,
- split same-height vote locks and committed-edge highest-QC conflicts do not
  produce fresh proposals,
- stale retired prior-view vote history, accepted new-view supersession,
  locked-QC fallback, and locked-chain extension remain permitted liveness
  cases.

`SumeragiEngineTickGate.tla` captures the pure engine pacemaker tick gate:
- every tick advances the local view by one saturating step,
- ticks return the engine to proposal phase and emit both a `NewView` vote and
  an `AdvanceView` output,
- any in-flight proposal validation is cleared before late callbacks arrive,
- highest-QC state is bound into the `NewView` vote subject and highest-QC
  field when present; otherwise the vote uses the zero subject with no highest
  QC,
- pending finality is preserved across view changes so exact payload recovery
  can still complete.

`SumeragiEngineNewViewSubjectGate.tla` captures pure-engine NewView vote
subject projection:
- highest-QC ticks and invalid-validation failures project
  `qc.subject_block_hash` into both the parent and block fields,
- highest-QC NewView votes carry the same highest-QC reference,
- no-highest ticks use the zero subject and bind no highest-QC reference,
- no-highest invalid-validation failures use the rejected block hash as both
  parent and block and bind no highest-QC reference,
- every emitted NewView subject uses the zero payload hash.

`SumeragiEngineHandleDispatchGate.tla` captures top-level pure-engine input
dispatch:
- each `ConsensusInput` variant dispatches to exactly one matching handler,
- `Tick` and `Proposal` inputs cannot be dropped or cross-routed,
- all certificate phase variants dispatch only to `on_certificate(...)`,
- payload availability, validation-result, and committed-block inputs dispatch
  only to their matching handlers.

`SumeragiEngineCertificateDispatchGate.tla` captures pure-engine certificate
prefilter dispatch:
- already committed heights, wrong height/epoch/validator-set context, and
  wrong quorum policy are rejected before any phase handler runs,
- stale-view Prepare and Commit certificates are rejected by the shared
  prefilter,
- matching Prepare and Commit certificates dispatch only to their corresponding
  phase handlers,
- matching NewView certificates dispatch to `on_new_view_qc(...)` regardless
  of lower, same, or higher view, leaving the strict newer-view check to the
  NewView handler,
- accepted certificates cannot be cross-dispatched to the wrong phase handler.

`SumeragiEngineCertificatePrefilterStateGate.tla` captures pure-engine
certificate prefilter state handoff:
- rejected certificates return without mutating phase, round, lock, highest-QC,
  pending-finality, or validation-owner state,
- rejected certificates emit no output because no phase handler runs,
- accepted certificates reach the correct phase handler with the original
  prefilter-visible state unchanged,
- phase-specific handlers, not the shared prefilter, own all certificate-driven
  state mutation.

`SumeragiEngineViewAdvanceSaturationGate.tla` captures the shared pure-engine
view-advance boundary used by pacemaker ticks and invalid validation results:
- ordinary views advance by exactly one,
- maximum view values saturate instead of wrapping to zero,
- `NewView` and `AdvanceView` outputs bind the same saturated view stored in
  engine state,
- valid, stale, wrong-block, and no-in-flight validation callbacks do not
  advance the view.

`SumeragiEngineNewViewQcGate.tla` captures the pure engine NewView-QC gate:
- NewView certificates must match the current height, epoch, validator set, and
  quorum policy,
- accepted NewView certificates must carry a strictly newer view,
- carried highest-QC evidence must be compatible with the certificate round,
- accepted NewView QCs emit `AdvanceView`, return to proposal phase, clear
  in-flight validation, and preserve pending finality,
- accepted highest-QC evidence updates local highest-QC state only when it
  improves the existing reference.

`SumeragiEngineNewViewHighestQcGate.tla` captures exact highest-QC state after
NewView-QC handling:
- accepted NewView certificates without carried highest-QC evidence preserve
  the stored highest-QC reference,
- accepted compatible carried highest-QC evidence records exactly that QC when
  no current QC exists or when it improves the stored QC,
- accepted lower or equal carried highest-QC evidence preserves the stored QC,
- stale/same-view, incompatible-highest, wrong-context, and wrong-quorum
  NewView certificates preserve the stored highest-QC reference exactly.

`SumeragiEngineNewViewAdvanceGate.tla` captures the exact round/output fields
for accepted NewView QCs:
- accepted NewView QCs set the stored engine round to `certificate.round`,
- the emitted `AdvanceView` output carries the exact certificate round,
- accepted NewView QCs clear validation, preserve pending finality, and enter
  proposal phase,
- rejected NewView QCs do not update the stored round or emit `AdvanceView`.

`SumeragiEngineProposalGate.tla` captures the pure engine proposal-ingress
gate:
- proposals are accepted only while the engine is in proposal phase,
- proposal rounds must match the current height, epoch, validator set, and
  view,
- carried highest-QC evidence must be compatible with the proposal round,
- locked conflicting proposals require a strictly higher compatible QC, while
  unlocked proposals and proposals for the locked subject remain safe,
- accepted proposals must request validation, sign a prepare vote, and enter
  prepare phase.

`SumeragiEngineProposalOutputGate.tla` captures exact proposal output fields:
- accepted proposals emit `ValidateBlock` for the exact proposal subject,
- accepted proposals then emit one prepare `SignVote` for the exact proposal
  round and subject,
- prepare votes emitted for proposals never carry a highest-QC reference, even
  when the accepted proposal carried a highest QC to unlock a conflicting
  proposal,
- rejected proposals emit no validation or prepare-vote outputs.

`SumeragiEngineProposalStateGate.tla` captures exact proposal state mutation:
- accepted proposals move the phase from Proposal to Prepare,
- accepted proposals preserve the current round, locked QC, highest QC, and
  pending-finality marker exactly,
- rejected proposals preserve the whole modeled state exactly, including
  wrong-phase inputs that started outside Proposal phase,
- a proposal-carried highest QC may unlock a conflicting proposal but must not
  be recorded as the engine's highest QC by proposal ingress.

`SumeragiEngineProposalValidationOwnerGate.tla` captures the exact validation
owner side effect for accepted proposals:
- accepted proposals set `self.validating` to exactly the accepted proposal
  subject,
- accepted proposals overwrite any stale validation owner from earlier work,
- rejected proposals preserve the previous validation owner exactly, including
  preserving `None` for rejected candidates that started without an owner.

`SumeragiEngineProposalLockGate.tla` captures the pure-engine proposal lock
predicate:
- proposals are accepted when no locked QC exists,
- proposals for the locked subject are accepted without extra evidence,
- conflicting proposals without a highest QC are rejected,
- conflicting proposals with equal or lower QCs are rejected,
- conflicting proposals with strictly greater QCs are accepted.

`SumeragiQcRoundCompatibilityGate.tla` captures the pure-engine helper that
checks whether carried highest-QC evidence is compatible with a candidate
round:
- QC epoch must match the candidate round epoch,
- lower-height QCs are compatible regardless of their view,
- same-height QCs are compatible only when their view is no greater than the
  candidate round view,
- future-height QCs, same-height future-view QCs, and wrong-epoch QCs are
  rejected before proposal or NewView admission can use them.

`SumeragiEngineQcRefProjectionGate.tla` captures the pure-engine helper that
projects certificates into QC references:
- projected QC height, view, and epoch match the certificate round exactly,
- projected subject uses the certified block hash, not the parent hash or a
  synthesized zero hash,
- projected phase matches the certificate phase for Prepare, Commit, and
  NewView certificates,
- projection does not advance height or collapse round metadata before
  lock/highest-QC state is recorded.

`SumeragiEngineHighestQcRecordGate.tla` captures the pure-engine helper that
records local highest-QC state:
- empty highest-QC state records the candidate,
- existing highest-QC state updates only for candidates strictly greater under
  height, then view, then phase rank, then subject hash ordering,
- equal candidates and candidates lower on any decisive comparator do not
  overwrite state,
- height ordering dominates view, view dominates phase and subject, Commit
  phase outranks NewView/Prepare at the same slot, and subject hash tie-breaks
  make same-rank records deterministic.

`SumeragiEngineCommitSubjectGate.tla` captures the pure-engine helper that
applies finality side effects in `commit_subject(...)`:
- a conflicting already-committed height returns no output and preserves
  committed state, pending finality, validation ownership, and phase,
- fresh or matching committed subjects record or retain the subject hash,
- successful commits clear pending finality and validation ownership,
- successful commits return to proposal phase and emit exactly one
  `CommitBlock`.

`SumeragiEnginePayloadLookupGate.tla` captures the pure-engine helper that
checks local payload availability in `has_payload(...)`:
- availability is keyed by the exact `(block_hash, payload_hash)` pair,
- a matching block hash with a different payload hash is insufficient,
- a matching payload hash for another block is insufficient,
- unrelated recorded payloads and empty availability stores cannot satisfy the
  commit-QC immediate-finality guard.

`SumeragiEnginePrepareQcGate.tla` captures the pure engine prepare-QC to
commit-vote transition:
- prepare certificates must match the current height, epoch, validator set,
  view, and quorum policy before they can make the engine sign a commit vote,
- prepare certificates for already committed heights are ignored,
- replayed same-subject and conflicting prepare QCs for a round do not emit
  additional commit votes,
- prepare QCs cannot emit commit votes while the engine is waiting for pending
  finality payload recovery,
- accepted prepare QCs must record both the locked QC and highest QC.

`SumeragiEnginePrepareLockHighestGate.tla` captures exact lock/highest-QC state
after Prepare-QC handling:
- accepted Prepare QCs derive the stored lock/highest candidate from the exact
  certificate round, `Prepare` phase, and subject block hash,
- every accepted Prepare QC writes the exact derived QC to `state.locked_qc`,
- accepted Prepare QCs record the exact derived QC as highest only when no
  current highest QC exists or when the derived QC improves the stored highest,
- accepted lower/equal derived Prepare QCs preserve the stored highest QC while
  still updating the lock,
- shared-prefilter rejections, replayed/conflicting same-round Prepare QCs, and
  pending-finality returns preserve stored lock and highest-QC state exactly.

`SumeragiEnginePreparePhaseGate.tla` captures exact phase state after
Prepare-QC handling:
- every accepted fresh Prepare QC moves the pure engine to `Commit` phase,
  whether the previous non-pending phase was `Proposal`, `Prepare`, or
  already `Commit`,
- shared-prefilter rejections preserve the current phase,
- replayed or conflicting same-round Prepare QCs preserve the current phase,
- pending-finality Prepare QCs preserve `PendingFinality` rather than
  regressing or advancing phase state.

`SumeragiEnginePrepareVoteCacheGate.tla` captures the exact cache/output side
effects in the accepted `on_prepare_qc(...)` branch:
- an accepted safe Prepare QC must insert exactly
  `commit_votes[certificate.round] = certificate.subject`,
- the emitted commit vote must use phase `Commit`, the certificate round and
  subject, and no carried highest-QC reference,
- rejected, pending-finality, replayed, and conflicting Prepare QCs must not
  emit a commit vote,
- replayed and conflicting same-round Prepare QCs must preserve the existing
  cached subject.

`SumeragiEngineCommitQcGate.tla` captures the pure engine commit-QC finality
gate:
- commit certificates must match the current height, epoch, validator set,
  view, and quorum policy before they can affect finality,
- commit QCs for already committed heights, pending-finality replays, and
  conflicting pending-finality subjects are ignored,
- payload-available commit QCs finalize immediately,
- missing-payload commit QCs request exact payload recovery instead of
  finalizing,
- accepted commit QCs must record highest-QC state.

`SumeragiEngineCommitQcHighestRecordGate.tla` captures exact highest-QC state
after Commit-QC handling:
- accepted Commit QCs derive the stored candidate from the exact certificate
  round, phase, and subject block hash,
- payload-available and missing-payload Commit QCs record exactly the derived
  Commit QC when no current highest QC exists or when it improves the stored
  highest QC,
- accepted lower or equal derived Commit QCs preserve the stored highest QC,
- shared-prefilter rejections and pending-finality replay/conflict returns
  preserve the stored highest-QC reference exactly.

`SumeragiEngineCommitQcAvailableCommitGate.tla` captures the exact
payload-available Commit-QC finality side effects:
- a payload-available Commit QC commits the certified block hash at the current
  height,
- it emits exactly one `CommitBlock` for the certificate subject,
- it clears validation ownership, returns to proposal phase, and does not
  create pending-finality state or a fetch request,
- shared-prefilter rejections, already committed heights, and
  pending-finality replay/conflict returns do not commit or emit finality.

`SumeragiEngineCommitQcPendingFetchGate.tla` captures the pure engine
missing-payload Commit-QC pending/fetch boundary:
- missing-payload Commit QCs set `state.pending_finality` to the certified
  subject, insert the cloned certificate into the pending certificate map under
  the certified block hash, and emit `FetchPayload` with the exact certificate
  round, block hash, and payload hash,
- payload-available Commit QCs do not create pending state or fetch requests,
- shared-prefilter rejections and pending-finality replay/conflict returns do
  not create new pending entries or fetch requests, and replay/conflict returns
  preserve the already pending subject and certificate-map entry.

`SumeragiEngineCommitQcValidationCleanupGate.tla` captures the pure engine
Commit-QC validation-owner cleanup boundary:
- every current-context Commit QC that reaches `on_commit_qc(...)` clears
  in-flight validation ownership before payload-available, missing-payload,
  pending-finality replay, or pending-finality conflict returns,
- Commit QCs rejected by the shared certificate prefilter preserve validation
  ownership and do not synthesize cleanup side effects,
- late invalid validation callbacks cannot advance the view after a handler
  reached Commit QC has superseded the validation owner.

`SumeragiEnginePayloadAvailabilityGate.tla` captures the pure engine
payload-availability gate:
- payload availability alone cannot finalize a block,
- when a commit QC is pending, only the exact certified subject can commit,
- payload hash mismatches, parent mismatches, and unrelated block hashes are
  ignored without dropping pending finality,
- the exact matching payload clears pending finality and returns the engine to
  proposal phase.

`SumeragiEnginePayloadAvailabilityRecordGate.tla` captures the exact
availability-store mutation in `on_payload_available(...)`:
- every payload-availability input records exactly
  `(subject.block_hash, subject.payload_hash)` before any pending-finality
  lookup,
- no-pending, matching-pending, mismatched-pending, and duplicate
  notifications all use the same exact block/payload key,
- parent mismatches do not change the availability key because the store is
  keyed only by block hash and payload hash,
- previously recorded unrelated availability pairs are preserved.

`SumeragiEngineValidationResultGate.tla` captures the pure engine
validation-result gate:
- only the exact current in-flight validation result can mutate consensus
  state,
- valid current results clear validation ownership without emitting consensus
  outputs,
- invalid current results clear ownership, advance the view, emit a `NewView`
  vote and `AdvanceView`, and bind the correct highest-QC or fallback subject,
- wrong-round, wrong-block, replayed, no-in-flight, commit-superseded, and
  storage-committed callbacks are ignored without dropping pending finality or
  overwriting committed state.

`SumeragiEngineValidationOwnershipGate.tla` captures exact validation-owner
cleanup:
- matching valid and invalid current validation callbacks clear
  `self.validating`,
- wrong-round callbacks preserve the existing validation owner because the
  round guard runs before owner lookup,
- wrong-block callbacks preserve the existing validation owner after lookup,
- no-in-flight, replayed, and superseded callbacks preserve `None` exactly and
  never synthesize a new owner.

`SumeragiEngineValidationInvalidAdvanceGate.tla` captures exact invalid
validation-result round advancement:
- invalid current results store the exact next round obtained by saturating
  the view while preserving height, epoch, and validator set,
- the NewView vote and `AdvanceView` output both carry that same exact next
  round,
- max-view invalid results saturate instead of wrapping,
- valid current results and ignored callbacks preserve the current round and
  emit no view-advance outputs.

`SumeragiEngineCommittedBlockGate.tla` captures the pure engine committed-block
notification gate:
- fresh committed-block notifications record the height,
- only a fresh boundary reconfiguration notification emits validator-set
  activation,
- duplicate same-height notifications are idempotent,
- conflicting same-height notifications cannot overwrite the committed hash or
  activate a validator set.

`SumeragiEngineCommittedBlockRecordGate.tla` captures exact committed-map
recording for pure engine committed-block notifications:
- fresh notifications insert exactly
  `committed[round.height] = block_hash`,
- fresh notifications preserve already committed unrelated heights,
- duplicate notifications preserve the existing committed map,
- conflicting notifications preserve the existing same-height block hash and do
  not write any spurious committed height.

`SumeragiEngineReconfigurationStagingGate.tla` captures pure-engine
committed-block reconfiguration staging:
- fresh boundary reconfiguration notifications stage exactly the same
  `ValidatorSetChange` that they emit through `ActivateValidatorSet`,
- boundary reconfiguration replaces any previously staged change,
- plain commits, non-boundary reconfigurations, duplicate notifications, and
  conflicting notifications preserve existing staging and emit no activation.

`SumeragiEngineCommittedBlockCleanupGate.tla` captures committed-block cleanup
side effects:
- fresh current-height finality clears in-flight validation ownership,
- fresh current-height finality clears pending-finality state and removes the
  pending certificate map entry, whether the storage commit matches or
  supersedes the pending QC subject,
- fresh other-height notifications record finality without clearing current
  validation or pending-finality ownership,
- duplicate or conflicting already-committed-height notifications are no-ops,
- storage finality notifications never emit a `CommitBlock` output back to the
  adapter.

`SumeragiValidatorSetTransition.tla` captures the validator-set activation
gate for one scheduled reconfiguration:
- old-set finality at the activation boundary,
- staged activation only after that old-set certificate,
- new-set certificates only after activation,
- old-set certificates stopping before the activation height,
- rejection of mixed-set certificates and multiple validator-set certificates
  for one height.

`SumeragiCertifiedRecovery.tla` captures certified block recovery when a commit
QC arrives before the matching payload:
- pending finality is anchored to an observed commit QC,
- exact payload recovery is required before state application,
- mismatched certified block responses are rejected without dropping the
  pending QC,
- a same-height conflicting subject cannot finalize after another subject is
  already committed.

`SumeragiViewChangeSafety.tla` captures the view-change and locked-proposal
gate:
- accepted new-view certificates move the local view forward only,
- highest-QC tracking is monotonic over accepted evidence,
- locked validators reject conflicting proposals unless the proposal carries a
  strictly higher QC,
- conflicting prepare evidence cannot overwrite an existing same-height lock at
  the same or lower QC rank.

`SumeragiValidationGate.tla` captures asynchronous proposal-validation callback
ownership:
- only the current in-flight validation result may advance the view on failure,
- unknown validation results are ignored,
- completed validation result replays are ignored,
- timeout-stale validation failures cannot advance the view after timeout
  already cleared the in-flight proposal,
- one invalid validation result cannot advance the same proposal twice.

`SumeragiCertificateAdmission.tla` captures certificate admission before
evidence mutates consensus state:
- wrong height, epoch, validator-set, or quorum-policy certificates are ignored,
- stale prepare/commit certificates after view advance are ignored,
- future-height certificates are ignored,
- certificates for already committed heights are ignored.

`SumeragiHighestQcSelection.tla` captures deterministic highest-QC selection
from new-view evidence:
- only `NewView` certificates contribute embedded highest-QC evidence,
- QCs are ordered by height, then view, then phase rank, then subject hash,
- two replicas observing the same certificate set in different orders must
  select the same QC,
- mutations that ignore height priority, phase rank, subject tie-breaking, or
  certificate phase must produce counterexamples.

`SumeragiFrontierRecovery.tla` captures the focused Taira hang class around one
active pending contiguous frontier block plus one concrete future frontier slot:
- commit-vote evidence below or at quorum,
- vote queue backlog and local drain,
- missing vs. local payload state,
- fresh vs. stale frontier recovery ownership,
- quorum-reschedule marker/window pacing,
- future slot presence, contiguity, vote evidence, payload state, and recovery
  ownership,
- future frontier/new-view evidence derived from that future slot and consumed
  through a two-step reanchor/promotion path,
- late arrival of future frontier evidence after GST,
- promotion freshness, so a promoted second slot cannot inherit stale active
  payload recovery, retransmit, quorum-window, or view-rotation progress,
- active pending progress age/event tracking, so validation, local commit-vote,
  commit-QC, payload recovery, retransmit, reanchor, promotion, and rotation
  progress must explicitly touch the pending block progress marker,
- same-height stale recovery unlocks scoped to the subject view that was
  rotated, not just to the block height,
- deterministic post-GST commit, retransmit, bounded view-rotation, and
  zero-evidence drop outcomes.

All models intentionally abstract away wire formats, ECDSA/signature
verification, and full networking details.

## Files

- `Sumeragi.tla`: protocol model and properties.
- `Sumeragi_fast.cfg`: smaller CI-friendly parameter set.
- `Sumeragi_deep.cfg`: larger stress parameter set.
- `SumeragiForkSafety.tla`: same-height conflicting-branch commit-certificate safety model.
- `SumeragiForkSafety_fast.cfg`: permissioned count-quorum fork-safety check.
- `SumeragiForkSafety_npos.cfg`: NPoS-style stake-quorum fork-safety check.
- `SumeragiForkSafety_bug_double_sign.cfg`: expected-failure double-sign/lock-gate mutation.
- `SumeragiQuorumPolicy.tla`: fail-closed quorum-policy arithmetic model.
- `SumeragiQuorumPolicy_fast.cfg`: CI-friendly quorum-policy arithmetic check.
- `SumeragiQuorumPolicy_bug_count_under_threshold.cfg`: expected-failure under-threshold count mutation.
- `SumeragiQuorumPolicy_bug_count_over_validators.cfg`: expected-failure over-validator count mutation.
- `SumeragiQuorumPolicy_bug_stake_exact_two_thirds.cfg`: expected-failure exact two-thirds stake mutation.
- `SumeragiQuorumPolicy_bug_stake_over_total.cfg`: expected-failure over-total stake mutation.
- `SumeragiQuorumPolicy_bug_stake_invalid_input.cfg`: expected-failure invalid stake input mutation.
- `SumeragiQuorumPolicy_bug_stake_overflow.cfg`: expected-failure stake overflow mutation.
- `SumeragiRbcDeliverQuorum.tla`: RBC deliver-quorum gate model.
- `SumeragiRbcDeliverQuorum_fast.cfg`: CI-friendly RBC deliver-quorum check.
- `SumeragiRbcDeliverQuorum_bug_duplicate_ready_count.cfg`: expected-failure duplicate READY counting mutation.
- `SumeragiRbcDeliverQuorum_bug_under_quorum_deliver.cfg`: expected-failure under-quorum delivery mutation.
- `SumeragiRbcDeliverQuorum_bug_wrong_commit_formula.cfg`: expected-failure commit-quorum arithmetic mutation.
- `SumeragiRbcDeliverQuorum_bug_force_one_ignored.cfg`: expected-failure force-one debug path mutation.
- `SumeragiRbcCausalityGate.tla`: RBC INIT/chunk/READY/DELIVER causality model.
- `SumeragiRbcCausalityGate_fast.cfg`: CI-friendly RBC causality check.
- `SumeragiRbcCausalityGate_bug_*.cfg`: expected-failure INIT evidence, chunk integrity, READY validation, DELIVER validation, stash, duplicate, and commit-wakeup mutations.
- `SumeragiPendingRbcStashGate.tla`: pending-RBC stash cap/TTL/replay model.
- `SumeragiPendingRbcStashGate_fast.cfg`: CI-friendly pending-RBC stash check.
- `SumeragiPendingRbcStashGate_bug_*.cfg`: expected-failure chunk/byte cap, drop accounting, last-seen TTL, active-session retention, session-limit, flush, dedup release, metrics, repair, backlog, and evicted-frame replay mutations.
- `SumeragiRbcSigningPreimageGate.tla`: RBC READY/DELIVER signing-preimage construction model.
- `SumeragiRbcSigningPreimageGate_fast.cfg`: CI-friendly RBC signing-preimage check.
- `SumeragiRbcSigningPreimageGate_bug_*.cfg`: expected-failure domain, subject-field, self-signature, and embedded READY-bundle mutations.
- `SumeragiClassicSigningPreimageGate.tla`: classic Vote/VRF signing-preimage construction model.
- `SumeragiClassicSigningPreimageGate_fast.cfg`: CI-friendly classic signing-preimage check.
- `SumeragiClassicSigningPreimageGate_bug_*.cfg`: expected-failure domain, type, vote-subject, highest-QC, VRF body, and mutable-signature mutations.
- `SumeragiClassicSignatureGate.tla`: classic Vote/QC signature-verification model.
- `SumeragiClassicSignatureGate_fast.cfg`: CI-friendly classic Vote/QC signature-verification check.
- `SumeragiClassicSignatureGate_bug_*.cfg`: expected-failure mode, roster, bitmap, quorum, aggregate, vote-body, vote-signature, NewView highest-QC, and return-contract mutations.
- `SumeragiVrfMessageAdmissionGate.tla`: VRF commit/reveal message-admission model.
- `SumeragiVrfMessageAdmissionGate_fast.cfg`: CI-friendly VRF commit/reveal message-admission check.
- `SumeragiVrfMessageAdmissionGate_bug_*.cfg`: expected-failure mode, manager, topology, signature, epoch/window, rewrite, broadcast, local-state, and PRF-refresh mutations.
- `SumeragiVoteAdmissionGate.tla`: classic inbound vote-admission model.
- `SumeragiVoteAdmissionGate_fast.cfg`: CI-friendly classic inbound vote-admission check.
- `SumeragiVoteAdmissionGate_bug_*.cfg`: expected-failure early-drop, roster/defer, duplicate, chain-order, signature, NEW_VIEW highest-QC, conflict, evidence, QC, roster-cache, tracker, pipeline, and progress mutations.
- `SumeragiProposalHintAdmissionGate.tla`: inbound proposal-hint admission model.
- `SumeragiProposalHintAdmissionGate_fast.cfg`: CI-friendly proposal-hint admission check.
- `SumeragiProposalHintAdmissionGate_bug_*.cfg`: expected-failure stale/malformed hint, duplicate, committed-edge conflict, missing-highest repair, local metadata, lock, cache/observe, highest-QC update, dependency, PRF, replay, prune, and conflict-suppression mutations.
- `SumeragiProposalAdmissionGate.tla`: inbound proposal metadata admission model.
- `SumeragiProposalAdmissionGate_fast.cfg`: CI-friendly proposal metadata admission check.
- `SumeragiProposalAdmissionGate_bug_*.cfg`: expected-failure stale/malformed proposal, committed-edge conflict, missing-highest repair, local metadata, lock, cache/observe, highest-QC update, dependency, PRF, leader-context, replay, prune, conflict-suppression, and no-commit-wakeup mutations.
- `SumeragiBlockCreatedAdmissionGate.tla`: direct `BlockCreated` payload admission model.
- `SumeragiBlockCreatedAdmissionGate_fast.cfg`: CI-friendly direct `BlockCreated` payload admission check.
- `SumeragiBlockCreatedAdmissionGate_bug_*.cfg`: expected-failure hard-reject, duplicate, replay-preserve, dependency, parent/gap repair, stale cleanup, evidence, lock-reject, proposal-context, phase, commit-wakeup, missing-request, and payload-mismatch recovery mutations.
- `SumeragiQcSignerBitmap.tla`: QC signer-bitmap admission model.
- `SumeragiQcSignerBitmap_fast.cfg`: CI-friendly QC signer-bitmap admission check.
- `SumeragiQcSignerBitmap_bug_count_observers.cfg`: expected-failure observer-counting mutation.
- `SumeragiQcSignerBitmap_bug_ignore_bitmap_length.cfg`: expected-failure bitmap-length mutation.
- `SumeragiQcSignerBitmap_bug_ignore_out_of_bounds.cfg`: expected-failure out-of-bounds signer mutation.
- `SumeragiQcSignerBitmap_bug_under_quorum_accept.cfg`: expected-failure under-quorum acceptance mutation.
- `SumeragiCommitRootConsistency.tla`: commit-QC execution-root consistency model.
- `SumeragiCommitRootConsistency_fast.cfg`: CI-friendly commit-root consistency check.
- `SumeragiCommitRootConsistency_bug_mix_root_signers.cfg`: expected-failure mixed-root quorum mutation.
- `SumeragiCommitRootConsistency_bug_count_wrong_context.cfg`: expected-failure wrong-context vote counting mutation.
- `SumeragiCommitRootConsistency_bug_tie_high_root.cfg`: expected-failure nondeterministic/high-root tie mutation.
- `SumeragiCommitRootConsistency_bug_stake_ignores_weight.cfg`: expected-failure NPoS stake root-selection mutation.
- `SumeragiCommitRootConsistency_bug_under_quorum_accept.cfg`: expected-failure under-quorum root group mutation.
- `SumeragiCommitRootConsistency_bug_validate_mismatched_roots.cfg`: expected-failure root-mismatch validation mutation.
- `SumeragiCommitPipelineRecoveryGate.tla`: commit-pipeline recovery ordering model.
- `SumeragiCommitPipelineRecoveryGate_fast.cfg`: CI-friendly commit-pipeline recovery gate check.
- `SumeragiCommitPipelineRecoveryGate_bug_skip_local_qc_formation.cfg`: expected-failure missing local commit-QC aggregation mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_recover_despite_local_quorum.cfg`: expected-failure peer recovery before using local quorum mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_request_recovery_before_timeout.cfg`: expected-failure fresh local-vote recovery mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_request_recovery_without_local_vote.cfg`: expected-failure no-local-vote recovery mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_request_recovery_with_commit_qc.cfg`: expected-failure recovery despite observed commit QC mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_request_recovery_with_missing_data.cfg`: expected-failure missing-local-data recovery mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_request_recovery_invalid_pending.cfg`: expected-failure invalid-pending recovery mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_request_recovery_off_tip.cfg`: expected-failure off-tip recovery mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_skip_missing_qc_request.cfg`: expected-failure missing peer recovery mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_drop_commit_qc_marker.cfg`: expected-failure dropped commit-QC marker mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_skip_quorum_retransmit.cfg`: expected-failure missing near-quorum retransmit mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_use_collector_targets.cfg`: expected-failure collector-target retransmit mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_rebroadcast_without_votes.cfg`: expected-failure empty-vote rebroadcast mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_rebroadcast_after_qc.cfg`: expected-failure post-QC rebroadcast mutation.
- `SumeragiCommitPipelineSchedulingGate.tla`: commit-pipeline scheduling gate model.
- `SumeragiCommitPipelineSchedulingGate_fast.cfg`: CI-friendly commit-pipeline scheduling gate check.
- `SumeragiCommitPipelineSchedulingGate_bug_*.cfg`: expected-failure tick/event entry, deadline, recovery-candidate inclusion, budget exhaustion, backlog, last-run, wakeup, idle-budget, and candidate-processing mutations.
- `SumeragiCommitResultDrainGate.tla`: commit-result drain gate model.
- `SumeragiCommitResultDrainGate_fast.cfg`: CI-friendly commit-result drain gate check.
- `SumeragiCommitResultDrainGate_bug_*.cfg`: expected-failure result id, inflight ownership, worker disconnect, inline fallback, signature-recovery, summary/progress, kickstart, and loop-stop mutations.
- `SumeragiCommitJobDispatchGate.tla`: commit-job dispatch ownership gate model.
- `SumeragiCommitJobDispatchGate_fast.cfg`: CI-friendly commit-job dispatch gate check.
- `SumeragiCommitJobDispatchGate_bug_*.cfg`: expected-failure duplicate suppression, pending retention, worker enqueue, queue-full, inline fallback, worker disconnect, return-value, and ownership-exclusivity mutations.
- `SumeragiCommitInflightTimeoutGate.tla`: commit-inflight timeout reporting gate model.
- `SumeragiCommitInflightTimeoutGate_fast.cfg`: CI-friendly commit-inflight timeout gate check.
- `SumeragiCommitInflightTimeoutGate_bug_*.cfg`: expected-failure timeout-boundary, one-shot reporting, inflight preservation, late-result attachability, and no-consensus-mutation mutations.
- `SumeragiPostCommitPacemakerKickGate.tla`: post-commit pacemaker kickstart gate model.
- `SumeragiPostCommitPacemakerKickGate_fast.cfg`: CI-friendly post-commit pacemaker kickstart gate check.
- `SumeragiPostCommitPacemakerKickGate_bug_*.cfg`: expected-failure queue, pacing-only backpressure, hard-backpressure, callback-result, return, and timestamp-capture mutations.
- `SumeragiIdleViewProposalBudgetGate.tla`: proposal-side idle-view budget preservation gate model.
- `SumeragiIdleViewProposalBudgetGate_fast.cfg`: CI-friendly proposal idle-view budget preservation gate check.
- `SumeragiIdleViewProposalBudgetGate_bug_*.cfg`: expected-failure queue, mode-flip, commit-inflight, deadline, pacing-only backpressure, hard-backpressure, idle-repair deferral, proposal reservation, and post-proposal retry mutations.
- `SumeragiPacemakerEvaluationGate.tla`: pacemaker evaluation gate model.
- `SumeragiPacemakerEvaluationGate_fast.cfg`: CI-friendly pacemaker evaluation gate check.
- `SumeragiPacemakerEvaluationGate_bug_*.cfg`: expected-failure deferral logging, pacing-only, hard-backpressure, recovery, deadline, and proposal-attempt mutations.
- `SumeragiCachedSlotTimeoutGate.tla`: cached proposal-slot timeout gate model.
- `SumeragiCachedSlotTimeoutGate_fast.cfg`: CI-friendly cached proposal-slot timeout gate check.
- `SumeragiCachedSlotTimeoutGate_bug_*.cfg`: expected-failure near-quorum fast-timeout, min-boundary, backlog, NPoS hysteresis, streak, and factor-cap mutations.
- `SumeragiProposalParentResolutionGate.tla`: proposal parent resolution and inline backup transport gate model.
- `SumeragiProposalParentResolutionGate_fast.cfg`: CI-friendly proposal parent resolution and inline backup transport check.
- `SumeragiProposalParentResolutionGate_bug_*.cfg`: expected-failure parent-height, Kura precedence, pending fallback, parent-deferral, overflow, backup-seeding, and RBC-transport mutations.
- `SumeragiPrecommitQcViewChangeGate.tla`: precommit-QC view-change selector gate model.
- `SumeragiPrecommitQcViewChangeGate_fast.cfg`: CI-friendly precommit-QC view-change selector check.
- `SumeragiPrecommitQcViewChangeGate_bug_*.cfg`: expected-failure phase filtering, committed fallback, height/view comparison, tie, and selection mutations.
- `SumeragiCommitEvidenceReplayGate.tla`: known-block commit-evidence replay gate model.
- `SumeragiCommitEvidenceReplayGate_fast.cfg`: CI-friendly commit-evidence replay gate check.
- `SumeragiCommitEvidenceReplayGate_bug_replay_inactive.cfg`: expected-failure inactive pending replay mutation.
- `SumeragiCommitEvidenceReplayGate_bug_ignore_cooldown.cfg`: expected-failure cooldown bypass mutation.
- `SumeragiCommitEvidenceReplayGate_bug_replay_without_targets.cfg`: expected-failure local-only/no-target replay mutation.
- `SumeragiCommitEvidenceReplayGate_bug_skip_first_evidence.cfg`: expected-failure first-evidence replay drop mutation.
- `SumeragiCommitEvidenceReplayGate_bug_skip_progress.cfg`: expected-failure progress replay drop mutation.
- `SumeragiCommitEvidenceReplayGate_bug_skip_stalled_retry.cfg`: expected-failure stalled positive-evidence retry drop mutation.
- `SumeragiCommitEvidenceReplayGate_bug_replay_no_evidence.cfg`: expected-failure zero-evidence replay mutation.
- `SumeragiCommitEvidenceReplayGate_bug_votes_use_payload_fallback.cfg`: expected-failure vote replay payload-fallback mutation.
- `SumeragiCommitEvidenceReplayGate_bug_commit_qc_uses_votes.cfg`: expected-failure commit-QC replay as votes mutation.
- `SumeragiCommitEvidenceReplayGate_bug_drop_commit_qc_replay.cfg`: expected-failure dropped commit-QC replay mutation.
- `SumeragiCommitEvidenceReplayGate_bug_use_local_targets.cfg`: expected-failure local-target replay mutation.
- `SumeragiCommitEvidenceReplayGate_bug_use_duplicate_targets.cfg`: expected-failure duplicate-target replay mutation.
- `SumeragiBlockSyncRecoveryGate.tla`: block-sync recovery admission model.
- `SumeragiBlockSyncRecoveryGate_fast.cfg`: CI-friendly block-sync recovery gate check.
- `SumeragiBlockSyncRecoveryGate_bug_accept_stale_without_request.cfg`: expected-failure stale update accepted without request/evidence mutation.
- `SumeragiBlockSyncRecoveryGate_bug_drop_requested_stale.cfg`: expected-failure requested stale payload drop mutation.
- `SumeragiBlockSyncRecoveryGate_bug_accept_future_unrequested.cfg`: expected-failure unrequested future-height acceptance mutation.
- `SumeragiBlockSyncRecoveryGate_bug_revive_aborted_without_commit_qc.cfg`: expected-failure payload-only aborted revival mutation.
- `SumeragiBlockSyncRecoveryGate_bug_keep_aborted_with_commit_qc.cfg`: expected-failure commit-QC aborted placeholder retention mutation.
- `SumeragiBlockSyncRecoveryGate_bug_skip_vote_backed_owner.cfg`: expected-failure vote-backed stale owner drop mutation.
- `SumeragiBlockSyncRecoveryGate_bug_steal_owner_with_payload_only.cfg`: expected-failure payload-only owner steal mutation.
- `SumeragiBlockSyncRecoveryGate_bug_skip_certified_owner.cfg`: expected-failure certified recovery owner drop mutation.
- `SumeragiBlockSyncRecoveryGate_bug_activate_uncertified_conflict.cfg`: expected-failure raw same-height conflict activation mutation.
- `SumeragiBlockSyncRecoveryGate_bug_drop_commit_qc_marker.cfg`: expected-failure commit-QC marker loss mutation.
- `SumeragiBlockSyncRecoveryGate_bug_skip_missing_commit_qc_request.cfg`: expected-failure missing commit-QC repair tracking mutation.
- `SumeragiBlockSyncRecoveryGate_bug_keep_missing_request.cfg`: expected-failure missing-block request retention mutation.
- `SumeragiBlockSyncRecoveryGate_bug_clear_inflight_for_payload_only.cfg`: expected-failure payload-only stale inflight clear mutation.
- `SumeragiBlockSyncRecoveryGate_bug_keep_inflight_for_certified.cfg`: expected-failure certified stale inflight retention mutation.
- `SumeragiBlockSyncRecoveryGate_bug_promote_unvalidated_qc.cfg`: expected-failure unvalidated commit-QC promotion mutation.
- `SumeragiCertifiedBlockFetchGate.tla`: direct certified-block fetch recovery model.
- `SumeragiCertifiedBlockFetchGate_fast.cfg`: CI-friendly direct certified-block fetch check.
- `SumeragiCertifiedBlockFetchGate_bug_*.cfg`: expected-failure request-targeting, service-side admission, response splitting, response/proof/body validation, proof pairing, invalid-owner, retry-revival, and materialization-cleanup mutations.
- `SumeragiMissingBlockFetchGate.tla`: QC-first missing-block fetch planner model.
- `SumeragiMissingBlockFetchGate_fast.cfg`: CI-friendly missing-block fetch planner check.
- `SumeragiMissingBlockFetchGate_bug_*.cfg`: expected-failure missing-block request identity, retry/backoff, target selection, send-filter, and request-field mutations.
- `SumeragiMissingBlockHardCapGate.tla`: missing-block hard-cap recovery escalation model.
- `SumeragiMissingBlockHardCapGate_fast.cfg`: CI-friendly missing-block hard-cap recovery check.
- `SumeragiMissingBlockHardCapGate_bug_*.cfg`: expected-failure hard-cap trigger/suppress, lock-lag override, side-effect, no-actionable cleanup, and range-pull-only mutations.
- `SumeragiMissingBlockHardCapCleanupGate.tla`: missing-block hard-cap cleanup preservation model.
- `SumeragiMissingBlockHardCapCleanupGate_fast.cfg`: CI-friendly missing-block hard-cap cleanup check.
- `SumeragiMissingBlockHardCapCleanupGate_bug_*.cfg`: expected-failure live-frontier preservation, metadata/RBC retention, future-pruning, quorum-backed repair, no-live cleanup, and owner/evidence mutations.
- `SumeragiMissingBlockViewChangeGate.tla`: missing-block view-change escalation model.
- `SumeragiMissingBlockViewChangeGate_fast.cfg`: CI-friendly missing-block view-change escalation check.
- `SumeragiMissingBlockViewChangeGate_bug_*.cfg`: expected-failure priority/window/latch, mark, clear, scheduler, progress-deferral, and backlog-deferral mutations.
- `SumeragiNativeAmxAttestationGate.tla`: native AMX attestation gate model.
- `SumeragiNativeAmxAttestationGate_fast.cfg`: CI-friendly native AMX attestation gate check.
- `SumeragiNativeAmxAttestationGate_bug_seal_non_native_plan.cfg`: expected-failure non-AMX receipt mutation.
- `SumeragiNativeAmxAttestationGate_bug_seal_empty_roster.cfg`: expected-failure empty-roster receipt mutation.
- `SumeragiNativeAmxAttestationGate_bug_skip_prepare_request.cfg`: expected-failure missing prepare-request mutation.
- `SumeragiNativeAmxAttestationGate_bug_skip_commit_request.cfg`: expected-failure missing commit-request mutation.
- `SumeragiNativeAmxAttestationGate_bug_request_commit_before_prepare.cfg`: expected-failure commit-before-prepare mutation.
- `SumeragiNativeAmxAttestationGate_bug_retry_prepare_after_quorum.cfg`: expected-failure redundant prepare retry mutation.
- `SumeragiNativeAmxAttestationGate_bug_seal_with_prepare_only.cfg`: expected-failure prepare-only receipt mutation.
- `SumeragiNativeAmxAttestationGate_bug_seal_with_commit_only.cfg`: expected-failure commit-only receipt mutation.
- `SumeragiNativeAmxAttestationGate_bug_seal_partial_multi_leg.cfg`: expected-failure partial multi-leg receipt mutation.
- `SumeragiNativeAmxAttestationGate_bug_accept_duplicate_prepare.cfg`: expected-failure duplicate prepare signer mutation.
- `SumeragiNativeAmxAttestationGate_bug_accept_duplicate_commit.cfg`: expected-failure duplicate commit signer mutation.
- `SumeragiNativeAmxAttestationGate_bug_accept_wrong_prepare_body.cfg`: expected-failure wrong prepare-body mutation.
- `SumeragiNativeAmxAttestationGate_bug_accept_wrong_commit_body.cfg`: expected-failure wrong commit-body mutation.
- `SumeragiNativeAmxAttestationGate_bug_accept_outsider_signer.cfg`: expected-failure outsider signer mutation.
- `SumeragiNativeAmxAttestationGate_bug_use_arrival_order_bitmap.cfg`: expected-failure nondeterministic signer projection mutation.
- `SumeragiNativeAmxAttestationGate_bug_collapse_retry_bodies.cfg`: expected-failure retried-body cache collision mutation.
- `SumeragiNativeAmxAttestationGate_bug_collapse_participant_legs.cfg`: expected-failure participant-leg cache collision mutation.
- `SumeragiNativeAmxJournalReplay.tla`: native AMX queue-plan journal replay model.
- `SumeragiNativeAmxJournalReplay_fast.cfg`: CI-friendly native AMX journal replay check.
- `SumeragiNativeAmxJournalReplay_bug_drop_native_plan.cfg`: expected-failure native plan drop mutation.
- `SumeragiNativeAmxJournalReplay_bug_collapse_native_to_single.cfg`: expected-failure native plan collapsed to single-route mutation.
- `SumeragiNativeAmxJournalReplay_bug_single_plan_as_native.cfg`: expected-failure single-route plan replayed as native AMX mutation.
- `SumeragiNativeAmxJournalReplay_bug_drop_participants.cfg`: expected-failure participant-leg drop mutation.
- `SumeragiNativeAmxJournalReplay_bug_reorder_participants.cfg`: expected-failure participant ordering mutation.
- `SumeragiNativeAmxJournalReplay_bug_keep_duplicate_participant.cfg`: expected-failure participant deduplication mutation.
- `SumeragiNativeAmxJournalReplay_bug_recompute_digest_wrong.cfg`: expected-failure plan digest corruption mutation.
- `SumeragiNativeAmxJournalReplay_bug_drop_gossip_payload.cfg`: expected-failure gossip payload loss mutation.
- `SumeragiNativeAmxJournalReplay_bug_drop_entrypoint.cfg`: expected-failure entrypoint loss mutation.
- `SumeragiNativeAmxJournalReplay_bug_remove_by_hash_only.cfg`: expected-failure hash-only tombstone mutation.
- `SumeragiNativeAmxJournalReplay_bug_ignore_exact_remove.cfg`: expected-failure ignored exact tombstone mutation.
- `SumeragiNativeAmxJournalReplay_bug_replay_unsupported_version.cfg`: expected-failure unsupported-version replay mutation.
- `SumeragiNativeAmxJournalReplay_bug_first_put_wins.cfg`: expected-failure first-put-wins replacement mutation.
- `SumeragiNativeAmxJournalReplay_bug_compaction_drops_live.cfg`: expected-failure compaction live-record drop mutation.
- `SumeragiNativeAmxJournalReplay_bug_compaction_keeps_removed.cfg`: expected-failure compaction removed-record retention mutation.
- `SumeragiNativeAmxJournalReplay_bug_keep_torn_tail.cfg`: expected-failure torn-tail retention mutation.
- `SumeragiNativeAmxJournalReplay_bug_drop_prior_on_tail_repair.cfg`: expected-failure complete-prefix loss during tail repair mutation.
- `SumeragiNativeAmxReceiptValidation.tla`: native AMX receipt validation model.
- `SumeragiNativeAmxReceiptValidation_fast.cfg`: CI-friendly native AMX receipt validation check.
- `SumeragiNativeAmxReceiptValidation_bug_accept_missing_receipt.cfg`: expected-failure missing native receipt acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_reject_valid_single.cfg`: expected-failure single-route no-receipt rejection mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_single_receipt.cfg`: expected-failure single-route receipt acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_unsigned_entrypoint.cfg`: expected-failure unsigned-entrypoint receipt acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_unsupported_version.cfg`: expected-failure unsupported receipt version acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_source_mismatch.cfg`: expected-failure source hash mismatch acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_coordinator_mismatch.cfg`: expected-failure coordinator route mismatch acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_height_mismatch.cfg`: expected-failure block-height mismatch acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_plan_digest_mismatch.cfg`: expected-failure plan digest mismatch acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_missing_participant.cfg`: expected-failure missing participant leg acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_unexpected_participant.cfg`: expected-failure unexpected participant leg acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_duplicate_participant.cfg`: expected-failure duplicate participant leg acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_qc_source_mismatch.cfg`: expected-failure QC source mismatch acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_qc_entrypoint_mismatch.cfg`: expected-failure QC entrypoint mismatch acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_qc_plan_digest_mismatch.cfg`: expected-failure QC plan digest mismatch acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_qc_wrong_phase.cfg`: expected-failure QC phase mismatch acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_qc_coordinator_mismatch.cfg`: expected-failure QC coordinator mismatch acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_qc_participant_mismatch.cfg`: expected-failure QC participant mismatch acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_qc_height_mismatch.cfg`: expected-failure QC height mismatch acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_validator_hash_version.cfg`: expected-failure validator-set hash version acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_validator_set_hash.cfg`: expected-failure validator-set hash mismatch acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_unknown_dataspace.cfg`: expected-failure unknown participant dataspace acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_small_validator_set.cfg`: expected-failure undersized validator set acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_bad_bitmap_length.cfg`: expected-failure signer bitmap length acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_bitmap_oob.cfg`: expected-failure out-of-bounds bitmap signer acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_non_bls_signer.cfg`: expected-failure non-BLS signer acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_missing_pop.cfg`: expected-failure missing proof-of-possession acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_under_quorum.cfg`: expected-failure under-quorum receipt acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_missing_signature.cfg`: expected-failure missing aggregate signature acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_accept_invalid_signature.cfg`: expected-failure invalid aggregate signature acceptance mutation.
- `SumeragiNativeAmxReceiptValidation_bug_reject_valid_native.cfg`: expected-failure valid native receipt rejection mutation.
- `SumeragiNativeAmxIngressGate.tla`: native AMX control-plane ingress model.
- `SumeragiNativeAmxIngressGate_fast.cfg`: CI-friendly native AMX ingress check.
- `SumeragiNativeAmxIngressGate_bug_reply_wrong_prepare_phase.cfg`: expected-failure wrong prepare-request phase reply mutation.
- `SumeragiNativeAmxIngressGate_bug_reply_wrong_commit_phase.cfg`: expected-failure wrong commit-request phase reply mutation.
- `SumeragiNativeAmxIngressGate_bug_reply_local_non_bls.cfg`: expected-failure local non-BLS request reply mutation.
- `SumeragiNativeAmxIngressGate_bug_reply_local_missing_pop.cfg`: expected-failure local missing-PoP request reply mutation.
- `SumeragiNativeAmxIngressGate_bug_drop_valid_prepare_request.cfg`: expected-failure valid prepare-request drop mutation.
- `SumeragiNativeAmxIngressGate_bug_drop_valid_commit_request.cfg`: expected-failure valid commit-request drop mutation.
- `SumeragiNativeAmxIngressGate_bug_wrong_reply_peer.cfg`: expected-failure wrong reply target mutation.
- `SumeragiNativeAmxIngressGate_bug_wrong_reply_phase.cfg`: expected-failure wrong reply phase mutation.
- `SumeragiNativeAmxIngressGate_bug_wrong_reply_signer.cfg`: expected-failure wrong reply signer mutation.
- `SumeragiNativeAmxIngressGate_bug_wrong_reply_body.cfg`: expected-failure wrong reply body mutation.
- `SumeragiNativeAmxIngressGate_bug_cache_non_bls_vote.cfg`: expected-failure non-BLS vote cache mutation.
- `SumeragiNativeAmxIngressGate_bug_cache_missing_pop_vote.cfg`: expected-failure missing-PoP vote cache mutation.
- `SumeragiNativeAmxIngressGate_bug_cache_invalid_pop_vote.cfg`: expected-failure invalid-PoP vote cache mutation.
- `SumeragiNativeAmxIngressGate_bug_cache_invalid_signature_vote.cfg`: expected-failure invalid-signature vote cache mutation.
- `SumeragiNativeAmxIngressGate_bug_drop_valid_prepare_vote.cfg`: expected-failure valid prepare-vote drop mutation.
- `SumeragiNativeAmxIngressGate_bug_drop_valid_commit_vote.cfg`: expected-failure valid commit-vote drop mutation.
- `SumeragiNativeAmxIngressGate_bug_cache_duplicate_signer_twice.cfg`: expected-failure duplicate same-body signer cache mutation.
- `SumeragiNativeAmxIngressGate_bug_drop_retried_body.cfg`: expected-failure retried-body cache drop mutation.
- `SumeragiNativeAmxIngressGate_bug_drop_different_participant.cfg`: expected-failure distinct participant-leg cache drop mutation.
- `SumeragiVNextChainOrderGate.tla`: vNext chain-order helper construction model.
- `SumeragiVNextChainOrderGate_fast.cfg`: CI-friendly vNext chain-order helper check.
- `SumeragiVNextChainOrderGate_bug_accept_empty_order.cfg`: expected-failure empty-order acceptance mutation.
- `SumeragiVNextChainOrderGate_bug_accept_zero_critical.cfg`: expected-failure zero critical-prefix acceptance mutation.
- `SumeragiVNextChainOrderGate_bug_accept_critical_after_end.cfg`: expected-failure overlong critical-prefix mutation.
- `SumeragiVNextChainOrderGate_bug_accept_quarantine_before_critical.cfg`: expected-failure early quarantine-tail mutation.
- `SumeragiVNextChainOrderGate_bug_accept_quarantine_after_end.cfg`: expected-failure out-of-range quarantine-tail mutation.
- `SumeragiVNextChainOrderGate_bug_critical_path_includes_tail.cfg`: expected-failure critical-path tail inclusion mutation.
- `SumeragiVNextChainOrderGate_bug_critical_path_drops_last.cfg`: expected-failure critical-path truncation mutation.
- `SumeragiVNextChainOrderGate_bug_successor_off_by_one.cfg`: expected-failure successor off-by-one mutation.
- `SumeragiVNextChainOrderGate_bug_tail_has_successor.cfg`: expected-failure critical-tail successor mutation.
- `SumeragiVNextChainOrderGate_bug_unknown_has_successor.cfg`: expected-failure unknown-peer successor mutation.
- `SumeragiVNextChainOrderGate_bug_quarantine_has_successor.cfg`: expected-failure quarantine-peer successor mutation.
- `SumeragiVNextChainOrderGate_bug_count_prefix_off_by_one.cfg`: expected-failure count-prefix minimality mutation.
- `SumeragiVNextChainOrderGate_bug_count_prefix_accepts_impossible.cfg`: expected-failure impossible count-prefix mutation.
- `SumeragiVNextChainOrderGate_bug_stake_uses_non_strict.cfg`: expected-failure exact-two-thirds stake mutation.
- `SumeragiVNextChainOrderGate_bug_stake_missing_weight_accepted.cfg`: expected-failure missing-weight stake mutation.
- `SumeragiVNextChainOrderGate_bug_stake_zero_total_accepted.cfg`: expected-failure zero-total stake mutation.
- `SumeragiVNextChainOrderGate_bug_bitmap_wrong_length_for_nine.cfg`: expected-failure non-canonical nine-signer bitmap length mutation.
- `SumeragiVNextChainOrderGate_bug_bitmap_allows_duplicate.cfg`: expected-failure duplicate signer bitmap mutation.
- `SumeragiVNextChainOrderGate_bug_bitmap_allows_out_of_range.cfg`: expected-failure out-of-range signer bitmap mutation.
- `SumeragiVNextRechainGate.tla`: quarantined vNext re-chain helper model.
- `SumeragiVNextRechainGate_fast.cfg`: CI-friendly vNext re-chain helper check.
- `SumeragiVNextRechainGate_bug_accept_empty_evidence.cfg`: expected-failure empty-evidence acceptance mutation.
- `SumeragiVNextRechainGate_bug_ignore_slot_mismatch.cfg`: expected-failure slot-mismatch acceptance mutation.
- `SumeragiVNextRechainGate_bug_ignore_order_hash_mismatch.cfg`: expected-failure chain-order hash mismatch acceptance mutation.
- `SumeragiVNextRechainGate_bug_ignore_sequence_mismatch.cfg`: expected-failure re-chain sequence mismatch acceptance mutation.
- `SumeragiVNextRechainGate_bug_accept_non_successor.cfg`: expected-failure non-successor accusation mutation.
- `SumeragiVNextRechainGate_bug_allow_tail_accuser.cfg`: expected-failure tail accuser mutation.
- `SumeragiVNextRechainGate_bug_skip_sequential_scope.cfg`: expected-failure no-longer-successor sequential-scope mutation.
- `SumeragiVNextRechainGate_bug_allow_duplicate_evidence.cfg`: expected-failure duplicate-evidence mutation.
- `SumeragiVNextRechainGate_bug_ignore_untainted_limit.cfg`: expected-failure insufficient-untainted-validator mutation.
- `SumeragiVNextRechainGate_bug_ignore_count_quorum.cfg`: expected-failure count-quorum mutation.
- `SumeragiVNextRechainGate_bug_use_non_strict_stake.cfg`: expected-failure exact-two-thirds stake mutation.
- `SumeragiVNextRechainGate_bug_drop_accuser_taint.cfg`: expected-failure accuser-taint drop mutation.
- `SumeragiVNextRechainGate_bug_drop_accused_taint.cfg`: expected-failure accused-taint drop mutation.
- `SumeragiVNextRechainGate_bug_keep_tainted_in_critical.cfg`: expected-failure tainted-critical-path mutation.
- `SumeragiVNextRechainGate_bug_do_not_increment_sequence.cfg`: expected-failure re-chain sequence mutation.
- `SumeragiVNextRechainGate_bug_mutate_certificate_slot.cfg`: expected-failure certificate slot mutation.
- `SumeragiVNextRechainGate_bug_reuse_previous_hash.cfg`: expected-failure unchanged chain-order hash mutation.
- `SumeragiVNextSignatureGate.tla`: vNext aggregate certificate verification model.
- `SumeragiVNextSignatureGate_fast.cfg`: CI-friendly vNext aggregate certificate verification check.
- `SumeragiVNextSignatureGate_bug_accept_missing_signature.cfg`: expected-failure missing aggregate signature acceptance mutation.
- `SumeragiVNextSignatureGate_bug_allow_empty_roster.cfg`: expected-failure empty signer roster acceptance mutation.
- `SumeragiVNextSignatureGate_bug_ignore_bitmap_length.cfg`: expected-failure non-canonical bitmap length mutation.
- `SumeragiVNextSignatureGate_bug_ignore_bitmap_out_of_range.cfg`: expected-failure out-of-range bitmap signer mutation.
- `SumeragiVNextSignatureGate_bug_allow_empty_signer_set.cfg`: expected-failure empty signer-set acceptance mutation.
- `SumeragiVNextSignatureGate_bug_ignore_pop_length.cfg`: expected-failure signer PoP length mismatch mutation.
- `SumeragiVNextSignatureGate_bug_ignore_count_quorum.cfg`: expected-failure under-quorum count mutation.
- `SumeragiVNextSignatureGate_bug_use_non_strict_stake.cfg`: expected-failure exact-two-thirds stake mutation.
- `SumeragiVNextSignatureGate_bug_allow_non_bls_signer.cfg`: expected-failure non-BLS signer acceptance mutation.
- `SumeragiVNextSignatureGate_bug_accept_bad_aggregate_signature.cfg`: expected-failure bad aggregate signature acceptance mutation.
- `SumeragiVNextSignatureGate_bug_ignore_rechain_slot_mismatch.cfg`: expected-failure re-chain certificate slot mismatch mutation.
- `SumeragiVNextSignatureGate_bug_ignore_rechain_hash_mismatch.cfg`: expected-failure re-chain certificate hash mismatch mutation.
- `SumeragiVNextSignatureGate_bug_ignore_rechain_sequence_mismatch.cfg`: expected-failure re-chain certificate sequence mismatch mutation.
- `SumeragiVNextSignatureGate_bug_return_full_roster.cfg`: expected-failure return-full-roster mutation.
- `SumeragiVNextSignatureGate_bug_drop_returned_signer.cfg`: expected-failure returned-signer drop mutation.
- `SumeragiVNextSignatureGate_bug_return_signers_on_reject.cfg`: expected-failure rejected-certificate signer leak mutation.
- `SumeragiVNextSigningPreimageGate.tla`: vNext signing-preimage construction model.
- `SumeragiVNextSigningPreimageGate_fast.cfg`: CI-friendly vNext signing-preimage construction check.
- `SumeragiVNextSigningPreimageGate_bug_*.cfg`: expected-failure domain-separation, body-field, signature-material, vote-projection, and suspicion-hash mutations.
- `SumeragiVNextControlIngressGate.tla`: vNext control-certificate ingress model.
- `SumeragiVNextControlIngressGate_fast.cfg`: CI-friendly vNext control-certificate ingress check.
- `SumeragiVNextControlIngressGate_bug_*.cfg`: expected-failure missing-round, already-current, re-chain rejection, valid re-chain, taint-bound escalation, view-change requirement, and view-certificate side-effect mutations.
- `SumeragiVNextSlotLifecycleGate.tla`: vNext actor-owned slot lifecycle model.
- `SumeragiVNextSlotLifecycleGate_fast.cfg`: CI-friendly vNext slot-lifecycle check.
- `SumeragiVNextSlotLifecycleGate_bug_*.cfg`: expected-failure no-base, committed-stickiness, validation-dispatch, worker-owner, queue-full, validation-result, deferral, timeout, commit, and recovery side-effect mutations.
- `SumeragiVNextValidationGate.tla`: vNext validation ownership model.
- `SumeragiVNextValidationGate_fast.cfg`: CI-friendly vNext validation ownership check.
- `SumeragiVNextValidationGate_bug_dispatch_queued.cfg`: expected-failure queued-dispatch mutation.
- `SumeragiVNextValidationGate_bug_raise_running_before_timeout.cfg`: expected-failure early running-suspicion mutation.
- `SumeragiVNextValidationGate_bug_miss_running_at_timeout.cfg`: expected-failure missed running timeout-boundary mutation.
- `SumeragiVNextValidationGate_bug_backpressure_before_timeout_raises.cfg`: expected-failure early backpressure-suspicion mutation.
- `SumeragiVNextValidationGate_bug_miss_backpressure_at_timeout.cfg`: expected-failure missed backpressure timeout-boundary mutation.
- `SumeragiVNextValidationGate_bug_accept_valid_as_await.cfg`: expected-failure valid-terminal await mutation.
- `SumeragiVNextValidationGate_bug_reject_invalid_as_await.cfg`: expected-failure invalid-terminal await mutation.
- `SumeragiVNextValidationGate_bug_underflow_elapsed.cfg`: expected-failure elapsed-underflow mutation.
- `SumeragiVNextValidationGate_bug_worker_started_keeps_queued.cfg`: expected-failure worker-start ownership mutation.
- `SumeragiVNextValidationGate_bug_apply_wrong_id.cfg`: expected-failure wrong-worker-id application mutation.
- `SumeragiVNextValidationGate_bug_apply_wrong_generation.cfg`: expected-failure wrong-generation application mutation.
- `SumeragiVNextValidationGate_bug_apply_not_running.cfg`: expected-failure non-running result application mutation.
- `SumeragiVNextValidationGate_bug_ignore_matching_valid.cfg`: expected-failure matching valid-result ignore mutation.
- `SumeragiVNextValidationGate_bug_ignore_matching_invalid.cfg`: expected-failure matching invalid-result ignore mutation.
- `SumeragiVNextValidationGate_bug_stale_mutates_state.cfg`: expected-failure stale-result state mutation.
- `SumeragiVoteVerifyAsyncGate.tla`: actor-side async vote-verification ownership model.
- `SumeragiVoteVerifyAsyncGate_fast.cfg`: CI-friendly async vote-verification ownership check.
- `SumeragiVoteVerifyAsyncGate_bug_*.cfg`: expected-failure no-worker fallback, duplicate suppression, backpressure, pending retry, worker-result ownership, rejection, and channel-disconnect mutations.
- `SumeragiQcVerifyAsyncGate.tla`: actor-side async QC aggregate-verification ownership model.
- `SumeragiQcVerifyAsyncGate_fast.cfg`: CI-friendly async QC aggregate-verification ownership check.
- `SumeragiQcVerifyAsyncGate_bug_*.cfg`: expected-failure cache, inline fallback, worker dispatch, duplicate suppression, known-block stale-lock, worker-result ownership, and disconnect-cleanup mutations.
- `SumeragiWorkerDrainSchedulerGate.tla`: worker-loop drain scheduler model.
- `SumeragiWorkerDrainSchedulerGate_fast.cfg`: CI-friendly worker-loop drain scheduler check.
- `SumeragiWorkerDrainSchedulerGate_bug_*.cfg`: expected-failure vote-priority, frontier repair, quorum-recovery drain, overtime payload, block backlog, low-priority service, accounting, polling, tick, and budget mutations.
- `SumeragiWorkerBudgetAdaptiveGate.tla`: worker-loop budget/adaptive-cap model.
- `SumeragiWorkerBudgetAdaptiveGate_fast.cfg`: CI-friendly worker-loop budget/adaptive-cap check.
- `SumeragiWorkerBudgetAdaptiveGate_bug_*.cfg`: expected-failure time-budget, vote-budget, drain-budget, tick-gap, block-backlog tier, and adaptive-cap mutations.
- `SumeragiWorkerIngressRoutingGate.tla`: worker ingress routing and parallel worker execution-envelope model.
- `SumeragiWorkerIngressRoutingGate_fast.cfg`: CI-friendly worker ingress routing check.
- `SumeragiWorkerIngressRoutingGate_bug_*.cfg`: expected-failure message routing, enqueue accounting, gate/stage/handler mapping, batch-limit, and drain-sequencing mutations.
- `SumeragiNposVrfEpochSealGate.tla`: NPoS VRF epoch-seal staging and committed-effect reconciliation model.
- `SumeragiNposVrfEpochSealGate_fast.cfg`: CI-friendly NPoS VRF epoch-seal staging check.
- `SumeragiNposVrfEpochSealGate_bug_*.cfg`: expected-failure merge, staging, committed-effect, activation, and effect-admission mutations.
- `SumeragiKuraCommitRetryGate.tla`: Kura durability and commit retry gate model.
- `SumeragiKuraCommitRetryGate_fast.cfg`: CI-friendly Kura durability commit retry check.
- `SumeragiKuraCommitRetryGate_bug_*.cfg`: expected-failure alignment, backoff, abort, cleanup, replay, and state-commit failure mutations.
- `SumeragiRestartReplayGate.tla`: restarted-peer replay and snapshot/Kura consistency model.
- `SumeragiRestartReplayGate_fast.cfg`: CI-friendly restarted-peer replay check.
- `SumeragiRestartReplayGate_bug_*.cfg`: expected-failure metadata verification, Kura parity, legacy replay, write-back, and canonical-checkpoint mutations.
- `SumeragiPostCommitCleanupGate.tla`: post-commit cleanup and stale-evidence pruning model.
- `SumeragiPostCommitCleanupGate_fast.cfg`: CI-friendly post-commit cleanup check.
- `SumeragiPostCommitCleanupGate_bug_*.cfg`: expected-failure RBC retention/drain, descendant pruning, duplicate-drop, missing-request, vote-window, and frontier-evidence cleanup mutations.
- `SumeragiFrontierGapRealignGate.tla`: post-commit frontier-gap realignment and committed-anchor range-pull pacing model.
- `SumeragiFrontierGapRealignGate_fast.cfg`: CI-friendly frontier-gap realignment check.
- `SumeragiFrontierGapRealignGate_bug_*.cfg`: expected-failure future-evidence, exact-body suppression, anchor selection, target fallback, cooldown, shared-window, stride, priority, and send-accounting mutations.
- `SumeragiPrecommitVoteGate.tla`: local precommit vote-emission gate model.
- `SumeragiPrecommitVoteGate_fast.cfg`: CI-friendly precommit vote-emission check.
- `SumeragiPrecommitVoteGate_bug_invalid_validation.cfg`: expected-failure invalid-validation emission mutation.
- `SumeragiPrecommitVoteGate_bug_observer.cfg`: expected-failure observer/out-of-topology emission mutation.
- `SumeragiPrecommitVoteGate_bug_duplicate.cfg`: expected-failure duplicate same-slot emission mutation.
- `SumeragiPrecommitVoteGate_bug_unsuperseded_conflict.cfg`: expected-failure unsuperseded same-height conflict mutation.
- `SumeragiPrecommitVoteGate_bug_older_quorum_completion.cfg`: expected-failure older-branch quorum-completion mutation.
- `SumeragiPrecommitVoteGate_bug_locked_conflict.cfg`: expected-failure locked same-height conflict mutation.
- `SumeragiPrecommitVoteGate_bug_missing_locked_payload.cfg`: expected-failure missing locked-payload mutation.
- `SumeragiPrecommitVoteGate_bug_non_extending_lock.cfg`: expected-failure non-extending locked-chain mutation.
- `SumeragiPrecommitVoteGate_bug_reject_safe.cfg`: expected-failure safe-candidate rejection mutation.
- `SumeragiProposalAssemblyGate.tla`: local proposal assembly gate model.
- `SumeragiProposalAssemblyGate_fast.cfg`: CI-friendly proposal assembly gate check.
- `SumeragiProposalAssemblyGate_bug_observer.cfg`: expected-failure observer/non-leader assembly mutation.
- `SumeragiProposalAssemblyGate_bug_active_vote_conflict.cfg`: expected-failure active same-height vote conflict mutation.
- `SumeragiProposalAssemblyGate_bug_pending_vote_verification.cfg`: expected-failure pending vote-verification mutation.
- `SumeragiProposalAssemblyGate_bug_missing_highest_qc.cfg`: expected-failure missing highest-QC mutation.
- `SumeragiProposalAssemblyGate_bug_non_extending_highest.cfg`: expected-failure non-extending highest-QC mutation.
- `SumeragiProposalAssemblyGate_bug_split_vote_lock.cfg`: expected-failure split-vote lock mutation.
- `SumeragiProposalAssemblyGate_bug_committed_edge_conflict.cfg`: expected-failure committed-edge highest-QC mutation.
- `SumeragiProposalAssemblyGate_bug_reject_safe.cfg`: expected-failure safe proposal rejection mutation.
- `SumeragiProposalAssemblyGate_bug_reject_stale_retired.cfg`: expected-failure stale retired vote-history rejection mutation.
- `SumeragiProposalAssemblyGate_bug_reject_locked_fallback.cfg`: expected-failure locked fallback rejection mutation.
- `SumeragiEngineTickGate.tla`: pure engine pacemaker tick gate model.
- `SumeragiEngineTickGate_fast.cfg`: CI-friendly engine tick gate check.
- `SumeragiEngineTickGate_bug_skip_round_advance.cfg`: expected-failure missing round-advance mutation.
- `SumeragiEngineTickGate_bug_skip_new_view_vote.cfg`: expected-failure missing NewView vote mutation.
- `SumeragiEngineTickGate_bug_skip_advance_output.cfg`: expected-failure missing AdvanceView output mutation.
- `SumeragiEngineTickGate_bug_wrong_phase.cfg`: expected-failure wrong post-tick phase mutation.
- `SumeragiEngineTickGate_bug_keep_validation.cfg`: expected-failure retained validation-in-flight mutation.
- `SumeragiEngineTickGate_bug_drop_pending_finality.cfg`: expected-failure dropped pending-finality mutation.
- `SumeragiEngineTickGate_bug_use_zero_despite_highest.cfg`: expected-failure highest-QC subject loss mutation.
- `SumeragiEngineTickGate_bug_use_highest_without_highest.cfg`: expected-failure false highest-QC subject mutation.
- `SumeragiEngineTickGate_bug_omit_highest_binding.cfg`: expected-failure missing highest-QC binding mutation.
- `SumeragiEngineTickGate_bug_bind_highest_without_highest.cfg`: expected-failure spurious highest-QC binding mutation.
- `SumeragiEngineNewViewSubjectGate.tla`: pure engine NewView subject projection helper model.
- `SumeragiEngineNewViewSubjectGate_fast.cfg`: CI-friendly NewView subject projection helper check.
- `SumeragiEngineNewViewSubjectGate_bug_*.cfg`: expected-failure highest-QC, fallback-subject, field-projection, payload, and highest-binding mutations.
- `SumeragiEngineHandleDispatchGate.tla`: pure engine top-level input dispatch model.
- `SumeragiEngineHandleDispatchGate_fast.cfg`: CI-friendly top-level input dispatch check.
- `SumeragiEngineHandleDispatchGate_bug_*.cfg`: expected-failure dropped input, cross-routed input, and double-dispatch mutations.
- `SumeragiEngineCertificateDispatchGate.tla`: pure engine certificate prefilter dispatch model.
- `SumeragiEngineCertificateDispatchGate_fast.cfg`: CI-friendly certificate prefilter dispatch check.
- `SumeragiEngineCertificateDispatchGate_bug_*.cfg`: expected-failure committed-height, wrong-context, wrong-quorum, stale Prepare/Commit, safe-certificate rejection, NewView prefilter rejection, and cross-phase dispatch mutations.
- `SumeragiEngineCertificatePrefilterStateGate.tla`: pure engine certificate prefilter state-handoff model.
- `SumeragiEngineCertificatePrefilterStateGate_fast.cfg`: CI-friendly certificate prefilter state-handoff check.
- `SumeragiEngineCertificatePrefilterStateGate_bug_*.cfg`: expected-failure accepted-handoff, rejected-state, rejected-output, and dropped-dispatch mutation configs.
- `SumeragiEngineViewAdvanceSaturationGate.tla`: pure engine view-advance saturation model.
- `SumeragiEngineViewAdvanceSaturationGate_fast.cfg`: CI-friendly view-advance saturation check.
- `SumeragiEngineViewAdvanceSaturationGate_bug_*.cfg`: expected-failure tick, invalid-validation, non-advancing validation, wraparound, and output-binding mutations.
- `SumeragiEngineNewViewQcGate.tla`: pure engine NewView-QC gate model.
- `SumeragiEngineNewViewQcGate_fast.cfg`: CI-friendly engine NewView-QC gate check.
- `SumeragiEngineNewViewQcGate_bug_accept_wrong_context.cfg`: expected-failure wrong round-context NewView-QC mutation.
- `SumeragiEngineNewViewQcGate_bug_accept_wrong_quorum.cfg`: expected-failure wrong quorum-policy NewView-QC mutation.
- `SumeragiEngineNewViewQcGate_bug_accept_stale_view.cfg`: expected-failure stale or same-view NewView-QC mutation.
- `SumeragiEngineNewViewQcGate_bug_accept_incompatible_highest.cfg`: expected-failure incompatible highest-QC mutation.
- `SumeragiEngineNewViewQcGate_bug_reject_safe_no_highest.cfg`: expected-failure safe no-highest NewView-QC rejection mutation.
- `SumeragiEngineNewViewQcGate_bug_reject_safe_improving_highest.cfg`: expected-failure safe improving-highest NewView-QC rejection mutation.
- `SumeragiEngineNewViewQcGate_bug_reject_safe_lower_highest.cfg`: expected-failure safe lower-highest NewView-QC rejection mutation.
- `SumeragiEngineNewViewQcGate_bug_skip_advance_output.cfg`: expected-failure missing AdvanceView output mutation.
- `SumeragiEngineNewViewQcGate_bug_wrong_phase.cfg`: expected-failure wrong post-NewView phase mutation.
- `SumeragiEngineNewViewQcGate_bug_keep_validation.cfg`: expected-failure retained validation-in-flight mutation.
- `SumeragiEngineNewViewQcGate_bug_drop_pending_finality.cfg`: expected-failure dropped pending-finality mutation.
- `SumeragiEngineNewViewQcGate_bug_overwrite_lower_highest.cfg`: expected-failure lower highest-QC overwrite mutation.
- `SumeragiEngineNewViewQcGate_bug_skip_highest_record.cfg`: expected-failure missing improving highest-QC record mutation.
- `SumeragiEngineNewViewHighestQcGate.tla`: pure engine exact NewView-QC highest-QC record model.
- `SumeragiEngineNewViewHighestQcGate_fast.cfg`: CI-friendly exact NewView highest-QC record check.
- `SumeragiEngineNewViewHighestQcGate_bug_*.cfg`: expected-failure no-current record, improving record, wrong-QC record, lower overwrite, no-highest mutation, and rejected-certificate record mutations.
- `SumeragiEngineNewViewAdvanceGate.tla`: pure engine exact NewView-QC round/output model.
- `SumeragiEngineNewViewAdvanceGate_fast.cfg`: CI-friendly exact NewView-QC advance check.
- `SumeragiEngineNewViewAdvanceGate_bug_*.cfg`: expected-failure round-field, output-field, cleanup, phase, rejected-round-update, and rejected-output mutations.
- `SumeragiEngineProposalGate.tla`: pure engine proposal-ingress gate model.
- `SumeragiEngineProposalGate_fast.cfg`: CI-friendly engine proposal-ingress gate check.
- `SumeragiEngineProposalGate_bug_wrong_phase.cfg`: expected-failure wrong phase proposal mutation.
- `SumeragiEngineProposalGate_bug_wrong_round.cfg`: expected-failure wrong round-context proposal mutation.
- `SumeragiEngineProposalGate_bug_incompatible_highest.cfg`: expected-failure incompatible highest-QC mutation.
- `SumeragiEngineProposalGate_bug_locked_conflict_no_qc.cfg`: expected-failure locked conflict without QC mutation.
- `SumeragiEngineProposalGate_bug_locked_conflict_equal_qc.cfg`: expected-failure locked conflict with equal-QC mutation.
- `SumeragiEngineProposalGate_bug_locked_conflict_lower_qc.cfg`: expected-failure locked conflict with lower-QC mutation.
- `SumeragiEngineProposalGate_bug_reject_unlocked.cfg`: expected-failure unlocked safe proposal rejection mutation.
- `SumeragiEngineProposalGate_bug_reject_locked_subject.cfg`: expected-failure locked-subject safe proposal rejection mutation.
- `SumeragiEngineProposalGate_bug_reject_higher_qc.cfg`: expected-failure higher-QC safe proposal rejection mutation.
- `SumeragiEngineProposalGate_bug_skip_validation_request.cfg`: expected-failure missing validation output mutation.
- `SumeragiEngineProposalGate_bug_skip_prepare_vote.cfg`: expected-failure missing prepare-vote output mutation.
- `SumeragiEngineProposalGate_bug_skip_prepare_phase.cfg`: expected-failure missing prepare-phase transition mutation.
- `SumeragiEngineProposalOutputGate.tla`: pure engine exact proposal output-field model.
- `SumeragiEngineProposalOutputGate_fast.cfg`: CI-friendly exact proposal output-field check.
- `SumeragiEngineProposalOutputGate_bug_*.cfg`: expected-failure missing output, swapped output order, wrong validation subject, wrong prepare-vote field, and rejected-output mutation configs.
- `SumeragiEngineProposalStateGate.tla`: pure engine exact proposal state-mutation model.
- `SumeragiEngineProposalStateGate_fast.cfg`: CI-friendly exact proposal state-mutation check.
- `SumeragiEngineProposalStateGate_bug_*.cfg`: expected-failure accepted-phase, accepted-state, rejected-phase, and rejected-state mutation configs.
- `SumeragiEngineProposalValidationOwnerGate.tla`: pure engine exact proposal validation-owner model.
- `SumeragiEngineProposalValidationOwnerGate_fast.cfg`: CI-friendly exact proposal validation-owner check.
- `SumeragiEngineProposalValidationOwnerGate_bug_*.cfg`: expected-failure missing owner record, stale owner retention, wrong-subject record, locked-subject record, and rejected-owner mutation configs.
- `SumeragiEngineProposalLockGate.tla`: pure engine proposal lock predicate helper model.
- `SumeragiEngineProposalLockGate_fast.cfg`: CI-friendly proposal lock predicate helper check.
- `SumeragiEngineProposalLockGate_bug_*.cfg`: expected-failure unlocked, locked-subject, no-QC, equality, lower-QC, and higher-QC strictness mutations.
- `SumeragiQcRoundCompatibilityGate.tla`: pure engine QC-round compatibility helper model.
- `SumeragiQcRoundCompatibilityGate_fast.cfg`: CI-friendly QC-round compatibility helper check.
- `SumeragiQcRoundCompatibilityGate_bug_*.cfg`: expected-failure epoch, lower-height, same-height view, future-height, and height/view ordering mutations.
- `SumeragiEngineQcRefProjectionGate.tla`: pure engine certificate-to-QC reference projection helper model.
- `SumeragiEngineQcRefProjectionGate_fast.cfg`: CI-friendly QC reference projection helper check.
- `SumeragiEngineQcRefProjectionGate_bug_*.cfg`: expected-failure height, view, epoch, subject, and phase projection mutations.
- `SumeragiEngineHighestQcRecordGate.tla`: pure engine highest-QC record helper model.
- `SumeragiEngineHighestQcRecordGate_fast.cfg`: CI-friendly highest-QC record helper check.
- `SumeragiEngineHighestQcRecordGate_bug_*.cfg`: expected-failure empty-state, height, view, phase-rank, subject tie-break, and equal-overwrite mutations.
- `SumeragiEngineCommitSubjectGate.tla`: pure engine commit-subject finality side-effect helper model.
- `SumeragiEngineCommitSubjectGate_fast.cfg`: CI-friendly commit-subject helper check.
- `SumeragiEngineCommitSubjectGate_bug_*.cfg`: expected-failure fresh-record, matching-commit, cleanup, phase, output, conflict-overwrite, conflict-output, and conflict-mutation mutations.
- `SumeragiEnginePayloadLookupGate.tla`: pure engine payload lookup helper model.
- `SumeragiEnginePayloadLookupGate_fast.cfg`: CI-friendly payload lookup helper check.
- `SumeragiEnginePayloadLookupGate_bug_*.cfg`: expected-failure block-hash, payload-hash, any-recorded-payload, empty-store, exact-rejection, and inverted-lookup mutations.
- `SumeragiEnginePrepareQcGate.tla`: pure engine prepare-QC commit-vote gate model.
- `SumeragiEnginePrepareQcGate_fast.cfg`: CI-friendly engine prepare-QC gate check.
- `SumeragiEnginePrepareQcGate_bug_wrong_context.cfg`: expected-failure wrong round-context mutation.
- `SumeragiEnginePrepareQcGate_bug_wrong_quorum_policy.cfg`: expected-failure wrong quorum-policy mutation.
- `SumeragiEnginePrepareQcGate_bug_stale_view.cfg`: expected-failure stale prepare-view mutation.
- `SumeragiEnginePrepareQcGate_bug_committed_height.cfg`: expected-failure committed-height prepare mutation.
- `SumeragiEnginePrepareQcGate_bug_replay_prepare.cfg`: expected-failure prepare-QC replay mutation.
- `SumeragiEnginePrepareQcGate_bug_conflicting_prepare.cfg`: expected-failure conflicting prepare-QC mutation.
- `SumeragiEnginePrepareQcGate_bug_pending_finality.cfg`: expected-failure pending-finality prepare mutation.
- `SumeragiEnginePrepareQcGate_bug_reject_safe.cfg`: expected-failure safe prepare-QC rejection mutation.
- `SumeragiEnginePrepareQcGate_bug_missing_lock_record.cfg`: expected-failure missing lock/highest-QC record mutation.
- `SumeragiEnginePrepareLockHighestGate.tla`: pure engine exact Prepare-QC lock/highest-QC record model.
- `SumeragiEnginePrepareLockHighestGate_fast.cfg`: CI-friendly exact Prepare-QC lock/highest-QC record check.
- `SumeragiEnginePrepareLockHighestGate_bug_*.cfg`: expected-failure lock record, wrong-QC record, rejected-QC mutation, replay/conflict/pending mutation, no-current record, improving record, and lower-overwrite mutations.
- `SumeragiEnginePreparePhaseGate.tla`: pure engine exact Prepare-QC phase-transition model.
- `SumeragiEnginePreparePhaseGate_fast.cfg`: CI-friendly exact Prepare-QC phase-transition check.
- `SumeragiEnginePreparePhaseGate_bug_*.cfg`: expected-failure accepted-phase, rejected-QC phase, replay/conflict phase, and pending-finality phase mutations.
- `SumeragiEnginePrepareVoteCacheGate.tla`: pure engine prepare-QC commit-vote cache/output side-effect model.
- `SumeragiEnginePrepareVoteCacheGate_fast.cfg`: CI-friendly prepare-QC commit-vote cache/output check.
- `SumeragiEnginePrepareVoteCacheGate_bug_*.cfg`: expected-failure cache insert, cache field, output field, rejection, replay/conflict output, conflict overwrite, and replay/conflict cleanup mutations.
- `SumeragiEngineCommitQcGate.tla`: pure engine commit-QC finality gate model.
- `SumeragiEngineCommitQcGate_fast.cfg`: CI-friendly engine commit-QC gate check.
- `SumeragiEngineCommitQcGate_bug_wrong_context.cfg`: expected-failure wrong round-context mutation.
- `SumeragiEngineCommitQcGate_bug_wrong_quorum_policy.cfg`: expected-failure wrong quorum-policy mutation.
- `SumeragiEngineCommitQcGate_bug_stale_view.cfg`: expected-failure stale commit-view mutation.
- `SumeragiEngineCommitQcGate_bug_committed_height.cfg`: expected-failure committed-height commit-QC mutation.
- `SumeragiEngineCommitQcGate_bug_pending_replay.cfg`: expected-failure pending-finality replay mutation.
- `SumeragiEngineCommitQcGate_bug_pending_conflict.cfg`: expected-failure pending-finality conflict mutation.
- `SumeragiEngineCommitQcGate_bug_commit_without_payload.cfg`: expected-failure missing-payload commit mutation.
- `SumeragiEngineCommitQcGate_bug_fetch_despite_payload.cfg`: expected-failure payload-available fetch mutation.
- `SumeragiEngineCommitQcGate_bug_reject_available.cfg`: expected-failure payload-available commit-QC rejection mutation.
- `SumeragiEngineCommitQcGate_bug_reject_missing_payload.cfg`: expected-failure missing-payload commit-QC rejection mutation.
- `SumeragiEngineCommitQcGate_bug_missing_highest_record.cfg`: expected-failure missing highest-QC record mutation.
- `SumeragiEngineCommitQcHighestRecordGate.tla`: pure engine exact Commit-QC highest-QC record model.
- `SumeragiEngineCommitQcHighestRecordGate_fast.cfg`: CI-friendly exact Commit-QC highest-QC record check.
- `SumeragiEngineCommitQcHighestRecordGate_bug_*.cfg`: expected-failure no-current record, improving record, wrong-QC record, lower overwrite, rejected-QC mutation, and pending replay/conflict mutation.
- `SumeragiEngineCommitQcAvailableCommitGate.tla`: pure engine payload-available Commit-QC exact finality side-effect model.
- `SumeragiEngineCommitQcAvailableCommitGate_fast.cfg`: CI-friendly payload-available Commit-QC exact finality check.
- `SumeragiEngineCommitQcAvailableCommitGate_bug_*.cfg`: expected-failure commit-record, output-field, cleanup, fetch/pending, rejected-QC, replay/conflict, and committed-height overwrite mutations.
- `SumeragiEngineCommitQcPendingFetchGate.tla`: pure engine missing-payload Commit-QC pending/fetch model.
- `SumeragiEngineCommitQcPendingFetchGate_fast.cfg`: CI-friendly missing-payload Commit-QC pending/fetch check.
- `SumeragiEngineCommitQcPendingFetchGate_bug_*.cfg`: expected-failure pending-state, pending-map key/certificate, fetch-field, payload-available, rejected-QC, and replay/conflict mutations.
- `SumeragiEngineCommitQcValidationCleanupGate.tla`: pure engine Commit-QC validation cleanup model.
- `SumeragiEngineCommitQcValidationCleanupGate_fast.cfg`: CI-friendly Commit-QC validation cleanup check.
- `SumeragiEngineCommitQcValidationCleanupGate_bug_*.cfg`: expected-failure accepted-QC cleanup, pending replay/conflict cleanup, rejected-prefilter cleanup, and late invalid callback mutations.
- `SumeragiEnginePayloadAvailabilityRecordGate.tla`: pure engine exact payload-availability record model.
- `SumeragiEnginePayloadAvailabilityRecordGate_fast.cfg`: CI-friendly exact payload-availability record check.
- `SumeragiEnginePayloadAvailabilityRecordGate_bug_*.cfg`: expected-failure skipped record, conditional record, wrong-key record, pending-subject substitution, and existing-availability mutation.
- `SumeragiEnginePayloadAvailabilityGate.tla`: pure engine payload-availability gate model.
- `SumeragiEnginePayloadAvailabilityGate_fast.cfg`: CI-friendly engine payload-availability gate check.
- `SumeragiEnginePayloadAvailabilityGate_bug_skip_available_record.cfg`: expected-failure missing payload-availability record mutation.
- `SumeragiEnginePayloadAvailabilityGate_bug_commit_without_pending.cfg`: expected-failure payload-only commit mutation.
- `SumeragiEnginePayloadAvailabilityGate_bug_commit_mismatched_payload.cfg`: expected-failure mismatched-payload commit mutation.
- `SumeragiEnginePayloadAvailabilityGate_bug_drop_pending_on_mismatch.cfg`: expected-failure pending-finality drop on mismatch mutation.
- `SumeragiEnginePayloadAvailabilityGate_bug_reject_matching_payload.cfg`: expected-failure exact matching payload rejection mutation.
- `SumeragiEnginePayloadAvailabilityGate_bug_keep_pending_after_commit.cfg`: expected-failure stale pending-finality after commit mutation.
- `SumeragiEnginePayloadAvailabilityGate_bug_wrong_phase_after_commit.cfg`: expected-failure wrong post-commit phase mutation.
- `SumeragiEngineValidationResultGate.tla`: pure engine validation-result gate model.
- `SumeragiEngineValidationResultGate_fast.cfg`: CI-friendly engine validation-result gate check.
- `SumeragiEngineValidationResultGate_bug_accept_wrong_round.cfg`: expected-failure wrong-round validation callback mutation.
- `SumeragiEngineValidationResultGate_bug_accept_wrong_block_hash.cfg`: expected-failure wrong-block validation callback mutation.
- `SumeragiEngineValidationResultGate_bug_accept_no_inflight.cfg`: expected-failure no-in-flight/replayed validation callback mutation.
- `SumeragiEngineValidationResultGate_bug_accept_superseded.cfg`: expected-failure commit-superseded validation callback mutation.
- `SumeragiEngineValidationResultGate_bug_reject_current_valid.cfg`: expected-failure current valid-result rejection mutation.
- `SumeragiEngineValidationResultGate_bug_reject_current_invalid.cfg`: expected-failure current invalid-result rejection mutation.
- `SumeragiEngineValidationResultGate_bug_keep_validation.cfg`: expected-failure retained validation owner mutation.
- `SumeragiEngineValidationResultGate_bug_valid_emits_output.cfg`: expected-failure valid-result output mutation.
- `SumeragiEngineValidationResultGate_bug_skip_round_advance.cfg`: expected-failure invalid-result round-advance mutation.
- `SumeragiEngineValidationResultGate_bug_skip_new_view_vote.cfg`: expected-failure invalid-result missing NewView vote mutation.
- `SumeragiEngineValidationResultGate_bug_skip_advance_output.cfg`: expected-failure invalid-result missing AdvanceView mutation.
- `SumeragiEngineValidationResultGate_bug_wrong_phase.cfg`: expected-failure invalid-result wrong phase mutation.
- `SumeragiEngineValidationResultGate_bug_use_invalid_subject_despite_highest.cfg`: expected-failure highest-QC subject loss mutation.
- `SumeragiEngineValidationResultGate_bug_use_highest_without_highest.cfg`: expected-failure false highest-QC subject mutation.
- `SumeragiEngineValidationResultGate_bug_omit_highest_binding.cfg`: expected-failure missing highest-QC binding mutation.
- `SumeragiEngineValidationResultGate_bug_bind_highest_without_highest.cfg`: expected-failure spurious highest-QC binding mutation.
- `SumeragiEngineValidationResultGate_bug_drop_pending_finality.cfg`: expected-failure superseded-callback pending-finality drop mutation.
- `SumeragiEngineValidationResultGate_bug_overwrite_committed.cfg`: expected-failure superseded-callback committed-state overwrite mutation.
- `SumeragiEngineValidationOwnershipGate.tla`: pure engine exact validation-owner cleanup model.
- `SumeragiEngineValidationOwnershipGate_fast.cfg`: CI-friendly exact validation-owner cleanup check.
- `SumeragiEngineValidationOwnershipGate_bug_*.cfg`: expected-failure current-owner retention, ignored-callback clear/replace, and no-owner synthesis mutations.
- `SumeragiEngineValidationInvalidAdvanceGate.tla`: pure engine exact invalid-validation round/output advance model.
- `SumeragiEngineValidationInvalidAdvanceGate_fast.cfg`: CI-friendly exact invalid-validation round/output advance check.
- `SumeragiEngineValidationInvalidAdvanceGate_bug_*.cfg`: expected-failure state-round, output-round, saturating-view, valid-callback, and ignored-callback advance mutations.
- `SumeragiEngineCommittedBlockGate.tla`: pure engine committed-block notification gate model.
- `SumeragiEngineCommittedBlockGate_fast.cfg`: CI-friendly engine committed-block gate check.
- `SumeragiEngineCommittedBlockGate_bug_skip_fresh_record.cfg`: expected-failure missing committed-height record mutation.
- `SumeragiEngineCommittedBlockGate_bug_reject_boundary_activation.cfg`: expected-failure boundary reconfiguration rejection mutation.
- `SumeragiEngineCommittedBlockGate_bug_activate_without_boundary.cfg`: expected-failure plain commit activation mutation.
- `SumeragiEngineCommittedBlockGate_bug_activate_non_boundary.cfg`: expected-failure non-boundary reconfiguration activation mutation.
- `SumeragiEngineCommittedBlockGate_bug_record_duplicate.cfg`: expected-failure duplicate committed-height record mutation.
- `SumeragiEngineCommittedBlockGate_bug_activate_duplicate.cfg`: expected-failure duplicate activation mutation.
- `SumeragiEngineCommittedBlockGate_bug_record_conflict.cfg`: expected-failure conflicting committed-height record mutation.
- `SumeragiEngineCommittedBlockGate_bug_activate_conflict.cfg`: expected-failure conflicting reconfiguration activation mutation.
- `SumeragiEngineCommittedBlockGate_bug_overwrite_conflict.cfg`: expected-failure conflicting committed-height overwrite mutation.
- `SumeragiEngineCommittedBlockRecordGate.tla`: pure engine exact committed-map record model.
- `SumeragiEngineCommittedBlockRecordGate_fast.cfg`: CI-friendly exact committed-map record check.
- `SumeragiEngineCommittedBlockRecordGate_bug_*.cfg`: expected-failure fresh-record key/value, unrelated-entry preservation, duplicate no-op, conflict no-op, and spurious committed-height mutations.
- `SumeragiEngineReconfigurationStagingGate.tla`: pure engine committed-block reconfiguration staging model.
- `SumeragiEngineReconfigurationStagingGate_fast.cfg`: CI-friendly reconfiguration staging check.
- `SumeragiEngineReconfigurationStagingGate_bug_*.cfg`: expected-failure boundary staging/activation, non-boundary/plain staging or activation, duplicate/conflict mutation, wrong-change, stale-stage preservation, and no-op clearing mutations.
- `SumeragiEngineCommittedBlockCleanupGate.tla`: pure engine committed-block cleanup side-effect model.
- `SumeragiEngineCommittedBlockCleanupGate_fast.cfg`: CI-friendly committed-block cleanup side-effect check.
- `SumeragiEngineCommittedBlockCleanupGate_bug_*.cfg`: expected-failure fresh-record, current-cleanup, other-height preservation, duplicate/conflict no-op, and spurious CommitBlock-output mutations.
- `SumeragiValidatorSetTransition.tla`: validator-set activation safety model.
- `SumeragiValidatorSetTransition_fast.cfg`: CI-friendly reconfiguration check.
- `SumeragiValidatorSetTransition_bug_premature_activation.cfg`: expected-failure activation-without-boundary-finality mutation.
- `SumeragiValidatorSetTransition_bug_premature_new_cert.cfg`: expected-failure new-set-before-activation mutation.
- `SumeragiValidatorSetTransition_bug_mixed_cert.cfg`: expected-failure mixed-set certificate mutation.
- `SumeragiCertifiedRecovery.tla`: certified commit-QC payload recovery safety model.
- `SumeragiCertifiedRecovery_fast.cfg`: CI-friendly certified-recovery check.
- `SumeragiCertifiedRecovery_bug_commit_without_payload.cfg`: expected-failure commit-without-payload mutation.
- `SumeragiCertifiedRecovery_bug_mismatched_payload.cfg`: expected-failure mismatched-payload mutation.
- `SumeragiCertifiedRecovery_bug_conflicting_finality.cfg`: expected-failure conflicting-finality mutation.
- `SumeragiViewChangeSafety.tla`: view-change/highest-QC/locked-proposal safety model.
- `SumeragiViewChangeSafety_fast.cfg`: CI-friendly view-change safety check.
- `SumeragiViewChangeSafety_bug_stale_new_view.cfg`: expected-failure stale-new-view mutation.
- `SumeragiViewChangeSafety_bug_unsafe_proposal.cfg`: expected-failure unsafe-proposal mutation.
- `SumeragiViewChangeSafety_bug_lock_overwrite.cfg`: expected-failure lock-overwrite mutation.
- `SumeragiViewChangeSafety_bug_highest_regression.cfg`: expected-failure highest-QC regression mutation.
- `SumeragiValidationGate.tla`: asynchronous proposal-validation callback ownership model.
- `SumeragiValidationGate_fast.cfg`: CI-friendly validation-callback safety check.
- `SumeragiValidationGate_bug_unknown_result.cfg`: expected-failure unknown-validation-result mutation.
- `SumeragiValidationGate_bug_completed_replay.cfg`: expected-failure completed-result-replay mutation.
- `SumeragiValidationGate_bug_timeout_inflight.cfg`: expected-failure timeout-retains-in-flight mutation.
- `SumeragiValidationGate_bug_invalid_replay.cfg`: expected-failure duplicate-invalid-result mutation.
- `SumeragiCertificateAdmission.tla`: fail-closed certificate-admission safety model.
- `SumeragiCertificateAdmission_fast.cfg`: CI-friendly certificate-admission safety check.
- `SumeragiCertificateAdmission_bug_wrong_context.cfg`: expected-failure wrong-context certificate mutation.
- `SumeragiCertificateAdmission_bug_stale_prepare_commit.cfg`: expected-failure stale prepare/commit certificate mutation.
- `SumeragiCertificateAdmission_bug_future_height.cfg`: expected-failure future-height certificate mutation.
- `SumeragiCertificateAdmission_bug_committed_height.cfg`: expected-failure committed-height certificate mutation.
- `SumeragiHighestQcSelection.tla`: deterministic highest-QC selection model.
- `SumeragiHighestQcSelection_fast.cfg`: CI-friendly highest-QC selection check.
- `SumeragiHighestQcSelection_bug_height_priority.cfg`: expected-failure height-priority comparator mutation.
- `SumeragiHighestQcSelection_bug_phase_rank.cfg`: expected-failure phase-rank comparator mutation.
- `SumeragiHighestQcSelection_bug_subject_tie.cfg`: expected-failure missing subject tie-break mutation.
- `SumeragiHighestQcSelection_bug_non_new_view.cfg`: expected-failure non-new-view inclusion mutation.
- `SumeragiFrontierRecovery.tla`: focused frontier recovery model.
- `SumeragiFrontierRecovery_fast.cfg`: smaller CI-friendly frontier parameter set.
- `SumeragiFrontierRecovery_deep.cfg`: larger frontier backlog/window/view bound set.
- `SumeragiFrontierRecovery_wide.cfg`: wider frontier bound set used by formal CI.
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: expected-failure stale-owner mutation.
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: expected-failure vote-queue mutation.
- `SumeragiFrontierRecovery_bug_payload_recovery.cfg`: expected-failure payload-recovery mutation.
- `SumeragiFrontierRecovery_bug_retransmit_followthrough.cfg`: expected-failure retransmit-follow-through mutation.
- `SumeragiFrontierRecovery_bug_future_promotion.cfg`: expected-failure future-promotion mutation.
- `SumeragiFrontierRecovery_bug_future_reanchor_clear.cfg`: expected-failure reanchor-clear mutation.
- `SumeragiFrontierRecovery_bug_future_evidence_drop.cfg`: expected-failure future-evidence drop mutation.
- `SumeragiFrontierRecovery_bug_promotion_reset.cfg`: expected-failure promotion-reset mutation.
- `SumeragiFrontierRecovery_bug_future_stale_owner.cfg`: expected-failure future stale-owner mutation.
- `SumeragiFrontierRecovery_bug_progress_touch.cfg`: expected-failure pending progress-touch mutation.
- `SumeragiFrontierRecovery_bug_height_only_recovery.cfg`: expected-failure height-only stale recovery mutation.
- `SumeragiFrontierRecovery_tlc_small.cfg`: small TLC cross-check config.
- `.github/workflows/nightly_sumeragi_formal.yml`: scheduled/manual longer-bound
  frontier check using `frontier-nightly`.

## Properties

Invariants:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

Fork-safety invariants:
- `TypeInvariant`
- `HonestCommitVotesSingleBranch`
- `CommitCertificateImpliesCountQuorum`
- `CommitCertificateImpliesStakeQuorum`
- `CommitCertificateImpliesHonestSupport`, which requires every modeled commit
  certificate to contain enough honest support after discounting the Byzantine
  budget.
- `NoConflictingCommitCertificates`, which is the direct same-height finality
  property for the two modeled branches.

Quorum-policy invariants:
- `TypeInvariant`
- `CountMatchesStrictSupermajority`
- `CountRejectsOverValidatorCount`
- `StakeMatchesStrictSupermajority`
- `ExactTwoThirdsStakeRejected`
- `StakeRejectsInvalidInputs`
- `StakeRejectsOverTotal`

RBC causality gate invariants:
- `TypeInvariant`
- `ActionsMatchSpec`
- `AcceptedInitBindsEvidence`
- `DroppedMessagesDoNotMutateSession`
- `StashedMessagesDoNotMutateConsensus`
- `LocalReadyRequiresPayloadEvidence`
- `RemoteReadyRequiresRosterSigRoot`
- `ReadyConflictInvalidatesAndClearsPending`
- `DeliverRequiresSignatureAndRoot`
- `DeliverReadyBundleSeedsOnlyAfterValidation`
- `DeliverWakeRequiresFirstDeliver`
- `DuplicateDeliverDoesNotMutate`

RBC signing-preimage gate invariants:
- `TypeInvariant`
- `FieldsMatchSpec`
- `PreimagesBindDomain`
- `ReadyUsesReadyTypeOnly`
- `DeliverUsesDeliverTypeOnly`
- `PreimagesBindSubject`
- `PreimagesExcludeSelfSignatures`
- `DeliverBindsReadyCount`
- `EmptyDeliverHasNoReadyEntries`
- `BundledDeliverBindsReadyEntries`

Classic signing-preimage gate invariants:
- `TypeInvariant`
- `FieldsMatchSpec`
- `PreimagesBindDomain`
- `VoteUsesVoteTypeOnly`
- `VrfCommitUsesCommitTypeOnly`
- `VrfRevealUsesRevealTypeOnly`
- `VoteBindsSubject`
- `VoteWithoutHighestBindsAbsenceOnly`
- `VoteWithHighestBindsReference`
- `VrfCommitBindsBody`
- `VrfRevealBindsBody`
- `PreimagesExcludeMutableSignatures`

Classic signature gate invariants:
- `TypeInvariant`
- `AcceptMatchesSpec`
- `AcceptedReturnsBitmapSigners`
- `ReturnedSignersWithinRoster`
- `RejectedReturnsNoSigners`
- `ValidCasesAccepted`
- `ModeAndRosterFailuresFailClosed`
- `BitmapFailuresFailClosed`
- `QuorumFailuresFailClosed`
- `AggregateFailuresFailClosed`
- `VoteFailuresFailClosed`
- `HighestFailuresFailClosed`
- `NposAggregateMayTolerateMissingVotes`

VRF message-admission gate invariants:
- `TypeInvariant`
- `AcceptMatchesSpec`
- `LateMatchesSpec`
- `StagingMatchesSpec`
- `BroadcastMatchesSpec`
- `LocalUpdateMatchesSpec`
- `PrfRefreshMatchesSpec`
- `ValidCasesAccepted`
- `InvalidCasesRejected`
- `RejectedHasNoSideEffects`
- `NetworkOriginDoesNotBroadcast`
- `ExternalAcceptedBroadcasts`
- `LocalStateOnlyForLocalSigner`
- `LateRevealDoesNotRefreshPrf`
- `NormalRevealRefreshesPrf`
- `CommitDoesNotRefreshPrf`

Vote-admission gate invariants:
- `TypeInvariant`
- `AcceptMatchesSpec`
- `RecordMatchesSpec`
- `DeferredMatchesSpec`
- `DroppedMatchesSpec`
- `EvidenceMatchesSpec`
- `QcAttemptMatchesSpec`
- `RosterCacheMatchesSpec`
- `NewViewTrackingMatchesSpec`
- `PipelineRequestMatchesSpec`
- `ProgressTouchMatchesSpec`
- `AcceptedCasesAccepted`
- `InvalidCasesRejected`
- `DeferredCasesDeferredOnly`
- `DroppedVotesHaveNoSideEffects`
- `RejectedConflictsPersistEvidence`
- `SupersededConflictRecordsWithoutEvidence`
- `DeferredConflictDoesNotRecordEvidence`
- `CrossPhaseConflictRecordsAndPersistsEvidence`
- `NewViewVotesNeverCacheRoster`
- `AcceptedPrepareCommitCachesRoster`
- `StaleNewViewAggregatesOnly`
- `ValidNewViewTracked`
- `AcceptedVotesAttemptQc`
- `AcceptedVotesTouchProgress`
- `AcceptedVotesRequestPipelineExceptStaleNewView`

Commit-root consistency invariants:
- `TypeInvariant`
- `SelectedRootMatchesSpec`
- `SelectedEvidenceMatchesSpecRoot`
- `AcceptedMatchesSpec`
- `MixedRootsCannotSatisfyPermissionedQuorum`
- `MixedRootsCannotSatisfyStakeQuorum`
- `WrongContextCannotSatisfyRootQuorum`
- `ValidationRootMismatchRejected`
- `ValidatedMatchesSpec`

Commit-pipeline recovery-gate invariants:
- `TypeInvariant`
- `LocalCommitQcFormationMatchesSpec`
- `LocalQuorumFormsBeforePeerRecovery`
- `CommitQcObservationIsPreserved`
- `MissingCommitQcRecoveryMatchesSpec`
- `FreshLocalVoteDoesNotRecover`
- `RecoveryRequiresLocalVote`
- `RecoveryRequiresCommitQcAbsent`
- `RecoveryRequiresPayloadLocal`
- `RecoveryRequiresValidPending`
- `RecoveryRequiresTipExtension`
- `QuorumRetransmitMatchesSpec`
- `QuorumRetransmitUsesMissingSignerTargets`
- `CollectorSubsetNeverOverridesQuorumTargets`
- `EmptyVoteSetNeverRebroadcasts`
- `CachedCommitQcSkipsRebroadcast`

Commit-evidence replay-gate invariants:
- `TypeInvariant`
- `ReplayMatchesSpec`
- `InactivePendingNeverReplays`
- `CooldownSuppressesReplay`
- `NoEvidenceNeverReplays`
- `RemoteTargetsRequired`
- `FirstEvidenceReplays`
- `ProgressReplays`
- `StalledPositiveEvidenceRetries`
- `VoteEvidenceUsesVoteReplay`
- `CommitQcUsesCommitCertReplay`
- `PayloadFallbackNeverUsed`
- `ReplayTargetsExcludeLocal`
- `DuplicateExplicitTargetsAreDeduped`

vNext re-chain gate invariants:
- `TypeInvariant`
- `AcceptMatchesSpec`
- `AcceptedTaintSetMatchesSpec`
- `AcceptedCriticalPathMatchesSpec`
- `AcceptedCriticalPathExcludesTainted`
- `AcceptedSequenceIncrements`
- `AcceptedCertificateBodyConsistent`
- `RejectedHasNoCertificate`
- `InvalidEvidenceFailsClosed`
- `QuarantineAndQuorumFailClosed`
- `ValidEvidenceCanRechain`

vNext signature gate invariants:
- `TypeInvariant`
- `AcceptMatchesSpec`
- `AcceptedReturnsBitmapSigners`
- `ReturnedSignersWithinRoster`
- `RejectedReturnsNoSigners`
- `ValidCertificatesAccepted`
- `MalformedBitmapFailsClosed`
- `QuorumFailuresFailClosed`
- `SignatureFailuresFailClosed`
- `RechainBodyMismatchesFailClosed`
- `EmptyRosterFailsClosed`
- `PopLengthMismatchFailsClosed`

vNext signing-preimage gate invariants:
- `TypeInvariant`
- `FieldsMatchSpec`
- `PreimageBindsDomain`
- `RechainPreimageUsesRechainTypeOnly`
- `ViewPreimageUsesViewTypeOnly`
- `RechainPreimageBindsBody`
- `ViewPreimageBindsBody`
- `PreimagesExcludeMutableSignatureMaterial`
- `RechainVoteAndCertificatePreimagesAgree`
- `ViewVoteAndCertificatePreimagesAgree`
- `UnsignedVotesProjectBodyAndSigner`
- `UnsignedViewVotesProjectBodyAndSigner`
- `UnsignedVotesStartWithoutSignature`
- `SuspectHashBindsBody`
- `SuspectHashExcludesSignature`

vNext control-ingress gate invariants:
- `TypeInvariant`
- `MatchesSpec`
- `RechainNoRoundDoesNotMutateLiveRound`
- `CurrentRechainCertificateIsNoOp`
- `RejectedRechainDoesNotInstallOrEscalate`
- `ValidRechainInstallsAndUpdates`
- `RequireViewChangeCasesDoNotInstallOrUpdate`
- `RequireViewChangeClearsVotesAndTriggers`
- `LastRechainOnlyWhenChainUpdated`
- `ViewCertificateAlwaysInstalls`
- `ViewHighestAbortRequiresInstalledHighestSlot`
- `ViewMissingRoundOrNoHighestDoesNotAbort`
- `ZeroNewViewDoesNotTrigger`
- `NonzeroViewTriggers`

vNext slot-lifecycle gate invariants:
- `TypeInvariant`
- `MatchesSpec`
- `NoBaseNeverInstallsOrProgresses`
- `CommittedSlotsAreSticky`
- `ValidationDispatchRequiresInstalledNonCommittedSlot`
- `MatchingWorkerStartOnlyMutatesQueued`
- `StaleWorkerEventsAreSideEffectFree`
- `QueueFullMatchingBackpressures`
- `ValidResultPreparesAndAccepts`
- `InvalidResultAbortsAndRejects`
- `DeferResetsOnlyNonCommittedSlots`
- `RecoveryRequiresDueUnprotectedTimeout`
- `DueUnprotectedTimeoutsRecover`
- `ProtectedTimeoutsDoNotRecover`
- `TerminalTicksDoNotRecover`
- `CommitPersistedCommits`
- `RecoveryDoesNotEmitValidationResultEffects`

vNext chain-order helper invariants:
- `TypeInvariant`
- `OrderOkMatchesSpec`
- `InvalidOrdersFailClosed`
- `ValidOrderCriticalPrefixMatches`
- `CriticalPathExcludesTail`
- `RejectedOrdersExposeNoCriticalPath`
- `SuccessorMatchesSpec`
- `TailPeerHasNoCriticalSuccessor`
- `PrefixMatchesSpec`
- `CountPrefixMinimal`
- `ImpossibleCountPrefixReturnsNone`
- `StrictStakeBoundaryNeedsMoreThanExact`
- `StakeFailuresFailClosed`
- `BitmapOkMatchesSpec`
- `BitmapLengthMatchesSpec`
- `BitmapFailuresFailClosed`

vNext validation gate invariants:
- `TypeInvariant`
- `DecisionMatchesSpec`
- `RunningTimeoutBoundaryMatchesSpec`
- `BackpressureTimeoutBoundaryMatchesSpec`
- `SaturatingElapsedDoesNotRaiseEarly`
- `TerminalStatesNeverDispatchOrSuspect`
- `WorkerStartedRecordsOwner`
- `MatchingWorkerResultsApply`
- `StaleWorkerResultsIgnored`
- `IgnoredResultsPreserveState`
- `AppliedResultsReachTerminalState`

Precommit vote-gate invariants:
- `TypeInvariant`
- `EmittedMatchesSpec`
- `RejectedMatchesSpec`
- `SafeCandidatesAreAccepted`
- `UnsafeCandidatesAreRejected`
- `InvalidValidationNeverEmits`
- `ObserversNeverEmit`
- `DuplicateSameSlotNeverEmits`
- `UnsupersededConflictNeverEmits`
- `OlderConflictCannotUseQuorumCompletion`
- `LockedConflictsNeverEmit`
- `PermittedConflictCasesCanEmit`
- `PermittedLockCasesCanEmit`

Proposal assembly-gate invariants:
- `TypeInvariant`
- `AssembledMatchesSpec`
- `DeferredMatchesSpec`
- `SafeCandidatesAreAssembled`
- `UnsafeCandidatesAreDeferred`
- `ObserversNeverAssemble`
- `ActiveLocalVoteConflictNeverAssembles`
- `PendingVoteVerificationNeverAssembles`
- `MissingHighestQcNeverAssembles`
- `NonExtendingHighestQcNeverAssembles`
- `SplitVoteLockNeverAssembles`
- `CommittedEdgeConflictNeverAssembles`
- `PermittedVoteHistoryCasesAssemble`
- `PermittedLockedParentCasesAssemble`

Engine tick gate invariants:
- `TypeInvariant`
- `EveryTickAdvancesView`
- `EveryTickSignsNewView`
- `EveryTickEmitsAdvanceView`
- `EveryTickEntersProposalPhase`
- `TicksClearInflightValidation`
- `TicksPreservePendingFinality`
- `HighestTicksUseHighestSubject`
- `NoHighestTicksUseZeroSubject`
- `HighestTicksBindHighestQc`
- `NoHighestTicksDoNotBindHighestQc`
- `SignedTicksHaveConsistentOutputs`

Engine NewView-QC exact advance invariants:
- `TypeInvariant`
- `RoundHeightMatchesSpec`
- `RoundViewMatchesSpec`
- `RoundEpochMatchesSpec`
- `RoundValidatorSetMatchesSpec`
- `OutputHeightMatchesSpec`
- `OutputViewMatchesSpec`
- `OutputEpochMatchesSpec`
- `OutputValidatorSetMatchesSpec`
- `ValidationMatchesSpec`
- `PendingFinalityMatchesSpec`
- `PhaseMatchesSpec`
- `AcceptedNewViewUpdatesStoredRoundExactly`
- `AcceptedNewViewEmitsExactAdvanceView`
- `AcceptedNewViewClearsValidation`
- `AcceptedNewViewPreservesPendingFinality`
- `AcceptedNewViewEntersProposalPhase`
- `RejectedNewViewDoesNotUpdateStoredRound`
- `RejectedNewViewEmitsNoAdvanceView`
- `RejectedNewViewPreservesOwnershipAndPhase`
- `ValuesStayInDomain`

Engine proposal-ingress gate invariants:
- `TypeInvariant`
- `AcceptedMatchesSpec`
- `IgnoredMatchesSpec`
- `SafeProposalsValidate`
- `SafeProposalsSignPrepare`
- `SafeProposalsEnterPreparePhase`
- `UnsafeProposalsAreIgnored`
- `WrongPhaseNeverAccepted`
- `WrongRoundNeverAccepted`
- `IncompatibleHighestNeverAccepted`
- `LockedConflictWithoutUnlockNeverAccepted`
- `AcceptedProposalsRequestValidation`
- `AcceptedProposalsSignPrepareVote`
- `AcceptedProposalsEnterPrepare`
- `IgnoredProposalsDoNotEmit`
- `OutputsStayTogether`

Engine prepare-QC gate invariants:
- `TypeInvariant`
- `SignedMatchesSpec`
- `IgnoredMatchesSpec`
- `SafePrepareQcsSign`
- `UnsafePrepareQcsAreIgnored`
- `WrongContextNeverSigns`
- `WrongQuorumPolicyNeverSigns`
- `StaleViewNeverSigns`
- `CommittedHeightNeverSigns`
- `ReplayPrepareNeverSigns`
- `ConflictingPrepareNeverSigns`
- `PendingFinalityNeverSigns`
- `SignedPrepareRecordsLock`
- `SignedPrepareRecordsHighest`
- `IgnoredPrepareDoesNotMutateLock`
- `LockAndHighestFollowSigned`

Engine prepare-vote cache/output invariants:
- `TypeInvariant`
- `CacheKeyMatchesSpec`
- `CacheSubjectMatchesSpec`
- `OutputPhaseMatchesSpec`
- `OutputRoundMatchesSpec`
- `OutputSubjectMatchesSpec`
- `OutputHighestMatchesSpec`
- `SafePrepareCachesRoundSubject`
- `SafePrepareEmitsExactCommitVote`
- `RejectedPrepareDoesNotCacheOrVote`
- `ReplayConflictPreservesCache`
- `ReplayConflictDoesNotVote`
- `OutputOnlyForSafePrepare`
- `CacheNeverUsesWrongRoundForSafePrepare`
- `CacheNeverUsesWrongSubjectForSafePrepare`
- `ValuesStayInDomain`

Engine commit-QC gate invariants:
- `TypeInvariant`
- `CommittedMatchesSpec`
- `FetchedMatchesSpec`
- `IgnoredMatchesSpec`
- `SafeAvailableCommitQcsCommit`
- `SafeMissingPayloadCommitQcsFetch`
- `UnsafeCommitQcsAreIgnored`
- `WrongContextNeverAccepted`
- `WrongQuorumPolicyNeverAccepted`
- `StaleViewNeverAccepted`
- `CommittedHeightNeverAccepted`
- `PendingReplayNeverAccepted`
- `PendingConflictNeverAccepted`
- `NoCommitWithoutPayload`
- `NoFetchWhenPayloadAvailable`
- `AcceptedCommitQcsRecordHighest`
- `IgnoredCommitQcsDoNotRecordHighest`
- `HighestFollowsAcceptedCommitQcs`

Engine payload-available Commit-QC exact finality invariants:
- `TypeInvariant`
- `CommittedHeightMatchesSpec`
- `CommittedBlockMatchesSpec`
- `ValidationMatchesSpec`
- `PhaseMatchesSpec`
- `PendingSubjectMatchesSpec`
- `PendingMapKeyMatchesSpec`
- `PendingMapCertMatchesSpec`
- `OutputParentMatchesSpec`
- `OutputBlockMatchesSpec`
- `OutputPayloadMatchesSpec`
- `FetchBlockMatchesSpec`
- `FetchPayloadMatchesSpec`
- `SafeAvailableCommitsExactSubject`
- `SafeAvailableClearsOwnershipAndReturnsProposal`
- `SafeAvailableDoesNotFetch`
- `RejectedCommitQcsDoNotCommitOrOutput`
- `ReplayConflictCommitQcsDoNotCommitOrOutput`
- `ReplayConflictClearsValidationAndPreservesPending`
- `CommittedHeightPreserved`
- `NoCommitWithoutPayloadAvailable`
- `ValuesStayInDomain`

Engine payload-availability gate invariants:
- `TypeInvariant`
- `EveryPayloadIsRecordedAvailable`
- `CommittedMatchesSpec`
- `IgnoredMatchesSpec`
- `PayloadOnlyNeverCommits`
- `MismatchedPayloadsNeverCommit`
- `MatchingPayloadCommits`
- `MismatchedPayloadsPreservePending`
- `MatchingPayloadClearsPending`
- `CommitClearsPending`
- `CommitEntersProposalPhase`
- `IgnoredPayloadsDoNotClearPending`

Engine committed-block gate invariants:
- `TypeInvariant`
- `RecordedMatchesSpec`
- `ActivatedMatchesSpec`
- `IgnoredMatchesSpec`
- `FreshCommitNotificationsRecord`
- `FreshBoundaryReconfigurationActivates`
- `PlainCommitNotificationsNeverActivate`
- `NonBoundaryReconfigurationNeverActivates`
- `DuplicateNotificationsAreIdempotent`
- `ConflictingNotificationsAreIgnored`
- `ConflictsNeverOverwrite`
- `ActivationRequiresFreshBoundaryRecord`
- `NoDuplicateOrConflictRecord`

Validator-set transition invariants:
- `TypeInvariant`
- `ActivationRequiresOldBoundaryFinality`
- `NewCertificatesStartAtActivationHeight`
- `NewCertificatesRequireActivation`
- `OldCertificatesStopBeforeActivationHeight`
- `NoMixedValidatorSetCertificates`
- `NoHeightCommittedByMultipleValidatorSets`

Certified recovery invariants:
- `TypeInvariant`
- `PendingFinalityRequiresCommitQc`
- `CommitRequiresCommitQc`
- `NoCommitWithoutPayload`
- `CommitRequiresMatchingPayload`
- `NoMismatchedPayloadAccepted`
- `NoConflictingFinality`

View-change safety invariants:
- `TypeInvariant`
- `CurrentViewNeverRewinds`
- `StaleNewViewCertificatesRejected`
- `HighestQcDominatesAcceptedEvidence`
- `HighestQcNeverRegresses`
- `UnsafeProposalsRejected`
- `ConflictingLockOverwritesRejected`

Validation-gate invariants:
- `TypeInvariant`
- `UnknownValidationDoesNotAdvance`
- `CompletedValidationReplayDoesNotAdvance`
- `LateValidationAfterTimeoutDoesNotAdvance`
- `TimeoutClearsInflight`
- `InvalidValidationAdvancesAtMostOnce`
- `NoStaleInflightAfterViewAdvance`
- `CompletedValidationClearsInflight`

Certificate-admission invariants:
- `TypeInvariant`
- `WrongContextCertificatesIgnored`
- `StalePrepareCommitCertificatesIgnored`
- `FutureHeightCertificatesIgnored`
- `CommittedHeightCertificatesIgnored`
- `LockedCertificateMatchesCurrentView`
- `CommittedHeightHasNoPendingFinality`

Highest-QC selection invariants:
- `TypeInvariant`
- `SelectedAEqualsSpecMax`
- `SelectedBEqualsSpecMax`
- `SelectedOnlyFromNewViewCertificates`
- `EqualObservedSelectsEqualQc`
- `HeightPriorityDominatesView`
- `PhaseRankDominatesSubject`
- `SubjectTieBreakDominatesArrivalOrder`

Temporal property:
- `EventuallyCommit` (`[] (gst => <> committed)`), with post-GST fairness encoded
  operationally in `Next` (timeout/fault preemption guards on enabled
  progress actions). This keeps the model checkable with Apalache 0.52.x, which
  does not support `WF_` fairness operators inside checked temporal properties.

Frontier recovery invariants:
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`, which rules out a terminal
  post-GST state where `pending /\ voteBacked /\ ~committed` has no recovery,
  commit, retransmit, rotation, or bounded-drop transition.
- `FuturePromotionReadyHasProgress`, which rules out a terminal post-GST
  state where the current pending wrapper has cleared for future evidence but
  the future slot cannot be promoted.
- `StaleRecoveryOwnerHasClearProgress`, which requires stale current frontier
  ownership to expose a clear transition once the relevant subject view has
  rotated.
- `VoteQueueBacklogHasDrainProgress`, which requires a queued-vote backlog on
  a fresh active frontier to expose a real drain transition instead of being
  masked by unrelated progress bookkeeping.
- `MissingPayloadHasRecoveryProgress`, which requires a vote-backed active
  frontier with a drained vote queue and missing payload to expose a payload
  recovery transition.
- `QuorumWindowHasRetransmitProgress`, which requires an expired
  quorum-reschedule window to expose a quorum retransmit transition before
  bounded rotation/drop can follow.
- `RetransmitHasFollowthroughProgress`, which requires a vote-backed frontier
  that already retransmitted quorum evidence to expose the deterministic
  rotation or view-bound clear follow-through.
- `FutureEvidenceHasReanchorProgress`, which requires concrete future frontier
  evidence to expose the current-wrapper clear step before promotion.
- `FutureEvidencePreservedUntilPromotion`, which requires observed future
  frontier evidence to remain represented by the concrete future slot until it
  is promoted.
- `FuturePromotionResetsActiveProgress`, which requires a freshly promoted
  second slot to start with cleared active progress, validation, vote, QC,
  recovery, quorum-window, and view flags.
- `PendingProgressEventsTouchAge`, which requires every modeled pending
  progress event to reset the abstract progress age.
- `StaleRecoveryUnlockIsViewScoped`, which requires any stale recovery unlock
  to have rotated at least the pending block's subject view.

Frontier recovery temporal property:
- `PostGstVoteBackedFrontierEventuallyResolves`: after GST, every unresolved
  active vote-backed pending frontier state eventually clears its pending
  wrapper.
- `RecoveredPayloadEventuallyAdvances`: a vote-backed frontier state that has
  recovered the payload cannot remain pending forever without commit,
  retransmit, reanchor, or rotation.
- `QuorumRetransmitEventuallyLeavesPending`: once quorum retransmit has fired
  for a vote-backed frontier state, the pending wrapper must eventually clear.
- `FutureFrontierEvidenceEventuallyReanchors`: later frontier/new-view evidence
  must be consumed through reanchor and future-slot promotion.
- `FuturePromotionReadyEventuallyPromotes`: a cleared current wrapper with
  promotion-ready future evidence must eventually promote that future slot.
- `PromotedSecondSlotEventuallyClears`: after promotion, the second slot must
  satisfy the same vote-backed pending-clear property as the original active
  slot.

## Assumption map

The fork-safety model is intentionally finite. These are the implementation
surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `honestCommitA`, `honestCommitB`, `byzCommitA`, `byzCommitB` | Commit-vote signer tracking, duplicate/conflicting vote rejection, and double-vote evidence in `crates/iroha_core/src/sumeragi/main_loop/votes.rs` plus coverage such as `conflicting_vote_does_not_override_first` and `conflicting_commit_vote_across_views_is_dropped_for_same_signer_peer` in `main_loop/tests.rs`. |
| `CommitQuorum`, `UseStakeQuorum`, `StakeQuorum` | Strict permissioned and NPoS quorum policy in `crates/iroha_data_model/src/block/consensus.rs` and the live commit-certificate aggregation/validation path. |
| `lockedBranch`, `lockView`, `PrepareQc` | Locked-QC acceptance rules in `crates/iroha_core/src/sumeragi/main_loop/locked_qc.rs` and the pure engine's `proposal_satisfies_lock(...)`. |
| `commitCerts` | Commit-certificate formation and finality conflict rejection in the collector/receiver path; the pure engine bridge coverage includes `conflicting_blocks_cannot_both_commit_at_same_height` and `committed_block_notifications_do_not_overwrite_conflicting_height`. |

The quorum-policy model is intentionally finite. These are the implementation
surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `PermissionedThreshold`, `CountSpecSatisfied` | `QuorumPolicy::permissioned_threshold(...)` and `QuorumPolicy::is_satisfied_by_count(...)` in `crates/iroha_data_model/src/block/consensus.rs`. |
| `CountRejectsOverValidatorCount` | Count quorum must reject signer counts above the active validator count even if they exceed the threshold. Bridge coverage includes `quorum_policy_enforces_strict_supermajority_boundaries`. |
| `StakeSpecSatisfied` | `QuorumPolicy::is_satisfied_by_stake(...)` accepts only signed stake strictly greater than two thirds of total stake. |
| `StakeRejectsInvalidInputs`, `StakeRejectsOverTotal` | NPoS stake quorum rejects missing/negative stake, zero/negative total stake, signed stake above total stake, and checked-multiply overflow. The same bridge test exercises these fail-closed boundaries. |

The RBC causality model is intentionally finite. These are the implementation
surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `valid_init`, `invalid_init_rejected` | `handle_rbc_init(...)` in `crates/iroha_core/src/sumeragi/main_loop/rbc.rs` rejects stale/epoch-mismatched/zero-chunk/oversized/digest-count/roster/header/signature/root/layout failures before creating `RbcSession`, recording the session roster, and binding block header, leader signature, payload hash, chunk digests, and chunk root. |
| `chunk_before_init`, `valid_chunk_recorded`, `chunk_bad_digest`, `complete_chunks_emit_ready`, `chunk_root_mismatch_blocks_ready` | `handle_rbc_chunk(...)`, `RbcSession::ingest_chunk_with_outcome(...)`, and `maybe_emit_rbc_ready(...)` stash chunks before INIT, reject digest mismatches, require complete payload evidence and matching chunk root before local READY, and mark mismatched roots invalid instead of signing. |
| `ready_before_init`, `valid_ready_recorded`, `ready_bad_signature`, `ready_roster_mismatch`, `ready_root_mismatch`, `ready_conflict` | `handle_rbc_ready(...)` stashes READY before session/roster availability, validates roster hash, sender signature, and chunk root before recording, and marks conflicting same-sender READY evidence invalid while clearing pending RBC state. |
| `deliver_before_init`, `valid_deliver`, `deliver_bad_signature`, `deliver_root_mismatch`, `deliver_ready_bundle`, `deliver_invalid_ready_bundle`, `deliver_duplicate` | `handle_rbc_deliver(...)` stashes DELIVER before session/roster availability, validates DELIVER signature and chunk root before recording delivery, validates embedded READY signatures independently before seeding the ready set, ignores invalid READY bundle entries, ignores duplicate DELIVER, and wakes the commit pipeline only after first delivery. |

The pending-RBC stash model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `chunk_insert_*`, `chunk_drop_*` | `PendingRbcMessages::push_chunk_capped(...)` enforces per-session chunk and byte caps, evicts oldest retained chunks when needed, drops new frames that still cannot fit, updates pending byte counts, and records evicted/dropped chunk counts. |
| `ready_accept`, `ready_drop_over_cap`, `deliver_accept`, `deliver_drop_over_cap` | `push_ready_capped(...)`, `push_deliver_capped(...)`, `rbc_ready_stash_bytes(...)`, and `rbc_deliver_stash_bytes(...)` admit READY/DELIVER stashes only when byte accounting fits; DELIVER byte accounting includes embedded READY signatures. |
| `touch_extends_ttl`, `ttl_expired_*`, `ttl_disabled` | `PendingRbcMessages::touch(...)` and `expired(...)` measure TTL from `last_seen`; `apply_pending_rbc_housekeeping(...)` skips TTL eviction when TTL is disabled and suppresses TTL eviction for keys with active sessions. |
| `session_cap_*` | `apply_pending_rbc_housekeeping(...)` evicts the oldest inactive pending slot when the cap is full, never evicts active sessions for capacity, allows an existing key, and rejects a new slot if only active sessions fill the cap. |
| `flush_after_init` | `flush_pending_rbc(...)` removes the pending wrapper and replays retained chunks, READY, and DELIVER frames through the normal handlers; evicted or dropped frames cannot reappear during flush. |
| `eviction_cleanup` | `pending_rbc_slot(...)` releases block-payload dedup entries, records pending-drop metrics, requests missing-block repair, and publishes the RBC backlog snapshot after TTL or session-limit eviction. |

The RBC signing-preimage model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `ready_preimage` | `rbc_ready_preimage(...)` in `crates/iroha_core/src/sumeragi/consensus.rs` constructs the domain-separated READY signing body from `consensus_domain(chain_id, "RbcReady", b"v1", mode_tag)` plus block hash, height, view, epoch, roster hash, chunk root, and sender. |
| `deliver_empty`, `deliver_bundle` | `rbc_deliver_preimage(...)` constructs the DELIVER signing body from `consensus_domain(chain_id, "RbcDeliver", b"v1", mode_tag)` plus the same RBC subject fields and the embedded READY-signature count. |
| `ready_entry_order`, `ready_entry_sender`, `ready_entry_sig_len`, `ready_entry_signature` | DELIVER preimages iterate over `deliver.ready_signatures` in vector order and bind each entry's sender, signature length, and signature bytes. |
| `ready_signature`, `deliver_signature` | `rbc_ready_preimage(...)` excludes `ready.signature`, and `rbc_deliver_preimage(...)` excludes `deliver.signature`; bridge coverage includes `preimages_use_current_domain_tags` and RBC READY/DELIVER handler tests that sign and verify these preimages. |

The classic signing-preimage model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `vote_no_highest`, `vote_with_highest` | `vote_preimage(...)` in `crates/iroha_core/src/sumeragi/consensus.rs` constructs the domain-separated vote signing body from `consensus_domain(chain_id, "Vote", b"v1", mode_tag)` plus block hash, parent state root, post-state root, height, view, epoch, chain-order hash, rechain sequence, and phase. |
| `highest_absent_flag`, `highest_present_flag`, `highest_*` | `vote_preimage(...)` appends a one-byte highest-QC presence flag and, when present, binds highest-QC height, view, epoch, subject block hash, and phase. |
| `vrf_commit` | `vrf_commit_preimage(...)` constructs the domain-separated VRF commit signing body from `consensus_domain(chain_id, "VrfCommit", b"v1", mode_tag)` plus epoch, signer index, and commitment bytes. |
| `vrf_reveal` | `vrf_reveal_preimage(...)` constructs the domain-separated VRF reveal signing body from `consensus_domain(chain_id, "VrfReveal", b"v1", mode_tag)` plus epoch, signer index, and reveal bytes. |
| `vote_signature`, `vrf_commit_signature`, `vrf_reveal_signature`, `aggregate_signature`, `signer_bitmap` | The preimage helpers exclude mutable signature and aggregate certificate material; bridge coverage includes `preimages_use_current_domain_tags` and `vote_preimage_binds_chain_order`. |

The classic signature model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `mode_tag_mismatch`, `validator_set_mismatch` | `validate_qc_against_votes(...)` rejects QCs whose `mode_tag` differs from the runtime mode or whose validator-set hash/body/version do not match the canonical topology. |
| `wrong_bitmap_length`, `bitmap_oob`, `empty_signer_set`, `under_count_quorum`, `stake_boundary`, `missing_stake_snapshot` | `qc_signer_indices(...)`, `parse_signers_bitmap(...)`, `signer_peers_for_topology(...)`, `Topology::min_votes_for_commit()`, and `stake_quorum_reached_for_snapshot(...)` enforce canonical bitmaps and fail-closed count/stake quorum semantics. |
| `aggregate_missing_signature`, `aggregate_missing_pop`, `aggregate_bad_signature` | `qc_aggregate_inputs(...)` requires a non-empty aggregate signature, signer PoPs aligned with selected signers, and successful `bls_normal_verify_preaggregated_same_message(...)` over `qc_bls_preimage(...)`. |
| `missing_vote_permissioned`, `valid_npos_stake_missing_vote` | Permissioned validation rejects missing bitmap-selected votes; NPoS validation may accept missing local vote bodies only after aggregate and stake checks succeed. |
| `subject_mismatch`, `roots_mismatch`, `vote_invalid_signature`, `view_mapping_missing` | Bitmap-selected local votes must map from canonical signer index to view-specific topology, match the QC subject and state roots, and pass `vote_signature_check(...)` unless the NPoS aggregate fast path already authenticated the QC. |
| `non_new_view_highest_present`, `new_view_*`, `new_view_vote_highest_mismatch` | `validate_new_view_qc_highest(...)` rejects highest-QC context on non-NewView QCs, requires NewView QCs to carry a matching Prepare/Commit highest-QC reference for the previous height, and requires selected votes to bind the same highest-QC reference. |
| `returned` | `validate_qc_against_votes(...)` returns the parsed bitmap voting signers on success and no signer set on rejection. |

The VRF message-admission model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `unsupported_mode`, `missing_manager` | `handle_vrf_commit(...)` and `handle_vrf_reveal(...)` accept only permissioned/NPoS modes and require an active `EpochManager`; unsupported modes and missing managers drop the message before staging or rebroadcast. |
| `signer_oob`, `missing_signature`, `bad_signature` | `verify_vrf_commit_signature(...)` and `verify_vrf_reveal_signature(...)` require a signer index inside the current topology, non-empty BLS signature bytes, and verification over `vrf_commit_preimage(...)` or `vrf_reveal_preimage(...)`. |
| `epoch_mismatch`, `unknown_signer`, `commit_out_of_window` | `EpochManager::try_note_commit_at_height(...)` rejects observations whose epoch does not match, whose signer is outside the epoch roster, or whose committed height is outside the commit window. |
| `commit_rewrite` | `try_note_commit_at_height(...)` accepts duplicate same-value commitments but rejects conflicting same-signer commitment rewrites. |
| `reveal_in_commit_window`, `reveal_without_commit`, `reveal_commit_mismatch`, `reveal_rewrite` | `EpochManager::try_note_reveal_at_height(...)` requires reveal-window timing, a prior commitment, matching reveal hash, and no conflicting same-signer reveal rewrite. |
| `valid_late_reveal_*` | Late reveals after the reveal window require a matching prior commitment and are accepted as late observations without refreshing the current PRF context. |
| `broadcast`, `local_updated`, `prf_refreshed` | `process_vrf_commit(...)`, `process_vrf_reveal(...)`, and `broadcast_external_vrf_metadata(...)` stage accepted snapshots, rebroadcast only externally sourced accepted observations, update local VRF state only for the local validator, and refresh PRF context only for normal accepted reveals. |

The vote-admission model is intentionally finite. These are the implementation
surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `height_or_view_drop`, `locked_conflict` | `handle_vote(...)` applies `drop_vote_for_height_or_view(...)` and `drop_precommit_vote_for_lock(...)` before roster lookup, signature validation, vote recording, or QC aggregation. |
| `roster_missing_prepare` | Missing PREPARE/COMMIT vote rosters call `maybe_request_missing_block_for_unresolved_roster(...)`, record a roster-missing drop, and defer through `defer_vote_for_roster(...)` without recording the vote. |
| `duplicate_vote`, `non_new_view_highest`, `chain_order_mismatch`, `bad_signature` | `validate_and_record_vote_with_signature_result(...)` rejects duplicate votes, non-NEW_VIEW highest-QC references, mismatched vNext chain-order bindings, and invalid signatures before touching vote logs or QC formation. |
| `new_view_*` | NEW_VIEW votes must carry a highest-QC reference with matching epoch, valid Prepare/Commit phase, matching block hash, matching next height, and local block metadata consistency before they can record. |
| `same_slot_conflict`, `same_slot_conflict_superseded`, `same_slot_conflict_deferred` | `conflicting_slot_vote_for_peer(...)` rejects same-signer same-slot conflicts with double-vote evidence, accepts only conflicts superseded by a newer QC/local quorum, or defers through `defer_vote_for_missing_highest_qc_context(...)` while supersession context is missing. |
| `same_key_conflict`, `cross_phase_conflict` | Raw-key conflicts are rejected with double-vote evidence; cross-phase PREPARE/COMMIT conflicts may record the new vote but still call `note_double_vote(...)`. |
| `recorded`, `qc_attempted`, `roster_cached`, `new_view_tracked`, `pipeline_requested`, `progress_touched` | `apply_validated_vote(...)` touches pending progress, caches rosters only for PREPARE/COMMIT votes, runs `try_form_qc_from_votes(...)` for accepted votes, records non-stale NEW_VIEW votes in the proposal tracker, and requests the commit pipeline except for stale NEW_VIEW aggregation-only votes. |

The proposal-hint admission model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `stale_height`, `stale_view` | `handle_proposal_hint(...)` applies committed-height and stale-view gates before caching, observing, PRF updates, replay, or highest-QC mutation; stale-height drops also prune old proposal-cache entries. |
| `highest_height_mismatch`, `highest_epoch_mismatch` | The hint highest-QC must point to `height - 1` and to the epoch computed for that parent height before any accepted side effects run. |
| `cached_conflict`, `cached_conflict_committed_replacement` | A conflicting cached hint for the same slot is rejected unless the cached hint's parent conflicts with the committed edge, in which case the replacement path may continue. |
| `stored_height_mismatch`, `committed_conflict`, `missing_committed_highest` | Locally stored parent blocks must match the highest-QC height and committed edge; committed-edge conflicts are suppressed and dropped rather than cached or observed. |
| `missing_future_highest_*` | Missing future highest-QC parents arm exact repair and a deferral marker without marking the slot observed; cross-view hints may be cached only as dependency context. |
| `local_height_mismatch`, `local_view_mismatch` | Local block metadata for the highest-QC hash must match the QC's height/view before PRF, cache, observe, replay, or highest-QC side effects. |
| `locked_qc_reject` | PRF context is updated before `ensure_highest_qc_extends_locked(...)`; locked-QC rejection then drops the hint without caching or observing it. |
| `valid_*`, `cache`, `observed`, `replayed`, `pruned` | Accepted hints update PRF context, cache the hint, mark the slot observed, replay deferred votes, and prune the observed-slot horizon. |
| `highest_updated`, `valid_lock_lag_update_defer` | `should_update_highest` mutates `highest_qc` only for a newer parent or same-slot Commit promotion; lock-lag catchup defers the highest-QC update while still accepting and caching the hint. |

The proposal admission model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `stale_height`, `stale_view` | `handle_proposal(...)` applies committed-height and stale-view gates before PRF, leader-context, cache, observe, replay, or highest-QC mutation; stale-height drops also prune old proposal-cache entries. |
| `proposal_epoch_mismatch` | Proposal headers must match `epoch_for_height(height)` before any accepted side effects run. |
| `highest_height_mismatch`, `highest_epoch_mismatch`, `parent_hash_mismatch` | The proposal highest-QC must point to `height - 1`, use the parent-height epoch, and match the proposal parent hash. |
| `stored_height_mismatch`, `committed_conflict`, `missing_committed_highest` | Locally stored parent blocks must match the highest-QC height and committed edge; committed-edge conflicts are suppressed and dropped rather than cached or observed. |
| `missing_future_highest` | Missing future highest-QC parents arm exact repair and a deferral marker without caching proposal metadata or marking the slot observed. |
| `local_height_mismatch`, `local_view_mismatch` | Local block metadata for the highest-QC hash must match the QC's height/view before PRF, leader-context, cache, observe, replay, or highest-QC side effects. |
| `locked_qc_reject` | Leader context and PRF context are sampled before `ensure_highest_qc_extends_locked(...)`; locked-QC rejection then drops the proposal without caching or observing it. |
| `valid_*`, `cache`, `observed`, `replayed`, `pruned` | Accepted proposals call `update_prf_context(...)`, `record_phase_sample(PipelinePhase::Propose, ...)`, `note_proposal_seen(...)`, cache the proposal, replay deferred votes, and prune observed-slot history through `note_proposal_seen(...)`. |
| `highest_updated`, `valid_lock_lag_update_defer` | `should_update_highest` mutates `highest_qc` only for a newer parent or same-slot Commit promotion; lock-lag catchup defers the highest-QC update while still accepting and caching the proposal. |
| `commit_pipeline_woken`, `payload_phase_recorded` | Proposal metadata alone does not call commit-pipeline wakeup paths and must not record payload-phase round progress; bridge coverage includes `proposal_does_not_wake_commit_pipeline_without_block_created`. |

The direct `BlockCreated` admission model is intentionally finite. These are
the implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `local_removed`, `stale_height`, `stale_view_without_request`, `lock_rejected_sink` | `handle_block_created_with_preserve_policy(...)` drops locally removed blocks, committed-height payloads, stale-view payloads without explicit recovery evidence, and active lock-rejected sinks before mutating pending state or waking commit. |
| `authoritative_owner_conflict`, `empty_payload_without_triggers`, `hint_mismatch_fatal`, `locked_qc_reject_*` | Same-height authoritative-owner conflicts, empty payloads without trigger context, fatal hint mismatches, and locked-QC failures are rejected with the corresponding cleanup/evidence side effects instead of becoming pending payload owners. |
| `valid_duplicate`, `pending_processing_defer`, `commit_inflight_defer`, `missing_highest_hint` | Duplicate bodies refresh/hydrate payload state without pending mutation; active processing, commit inflight, and missing-highest dependency gaps preserve replay material without waking the commit pipeline. |
| `future_height_request`, `future_height_gap_request` | Future-height payloads request exact parent or gap repair before admission continues so missing ancestors are not skipped. |
| `proposal_mismatch_continue`, `proposal_mismatch_preserve`, `rbc_payload_mismatch` | Proposal/RBC payload mismatch paths emit invalid evidence where proposal context exists, clear stale missing requests when appropriate, and only continue when the local policy allows payload recovery. |
| `payload_accepted`, `pending_updated`, `phase_sampled`, `commit_pipeline_requested` | Accepted direct bodies update pending state, note the payload phase, clear relevant missing-block requests, and request the commit pipeline; bridge coverage includes `block_created_missing_highest_qc_preserves_deferred_body_for_wire_rebuild` and `deferred_block_created_replays_after_missing_highest_qc_repairs`. |
| `proposal_cached`, `proposal_observed` | Inline proposal context is cached and observed only on accepted payload paths, matching the proposal-cache and `note_proposal_seen(...)` ordering inside `handle_block_created_with_preserve_policy(...)`. |

The commit-pipeline recovery-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `localQuorumStale`, `localQuorumFresh` | `process_commit_candidates_with_trigger_inner(...)` calls `try_form_qc_from_votes(...)` before peer recovery when cached commit votes meet `min_votes_for_commit`; bridge coverage includes `commit_pipeline_forms_local_commit_qc_before_missing_commit_qc_recovery`. |
| `stalledLocalVote`, `freshLocalVote` | Missing commit-QC recovery is armed only after the pending fast-path timeout for a locally voted pending block; bridge coverage includes `commit_pipeline_arms_missing_commit_qc_recovery_for_stalled_local_vote`. |
| `commitQcAlreadyObserved` | Existing commit-QC evidence must suppress peer recovery and preserve the pending block's commit-QC marker. |
| `missingLocalData`, `invalidPending`, `noLocalVote`, `offTip` | Recovery requires local DA payload availability, valid pending state, local commit-vote emission, and extension of the committed tip. |
| `nearQuorumRetransmit`, `collectorDecoyRetransmit` | `rebroadcast_block_votes(..., target_missing_only = true)` derives quorum missing-signer targets through `quorum_retransmit_targets_for_missing_votes(...)`; bridge coverage includes `commit_pipeline_rebroadcasts_cached_votes_to_quorum_retransmit_targets`. |
| `noVotesRetransmit`, `hasCommitQcRetransmit` | Empty vote logs and already cached commit QCs skip near-quorum rebroadcast. |

The commit-pipeline scheduling-gate model is intentionally finite. These are
the implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `tick_no_work`, `tick_active_candidate`, `tick_inflight_only`, `tick_wakeup_*`, `tick_queue_saturated_*` | The main tick path calls `should_run_commit_pipeline_on_tick(...)`, also entering the pipeline for commit inflight work or explicit wakeups. Queue saturation expands recovery-candidate scope but does not by itself run the pipeline. |
| `tick_budget_exhausted_before_candidates`, `tick_budget_exhausted_during_candidates` | `commit_pipeline_budget_exhausted(...)` re-arms `commit_pipeline_wakeup` and returns or stops before candidate work, preserving the pending block for a later slice. |
| `event_no_candidate`, `event_backlogged_candidate`, `event_backlogged_no_candidate`, `event_budget_exhausted` | `process_commit_candidates_with_trigger_inner(...)` handles event-triggered entry, reschedules stale pending blocks before candidate processing, observes backlog for diagnostics, and does not let backlog suppress or fabricate candidate work. |
| `candidate_recovery_*` | `commit_candidate_blocks_len(include_recovery_candidates)` includes recovery candidates only when a commit wakeup or queue saturation asks for them, and filters recovery-only candidates when active pending work exists without commit-certificate evidence. |
| `idle_view_*` | `should_preserve_idle_view_budget_for_commit_pipeline(...)` yields idle-view repair budget only to a woken commit pipeline with active candidates. |

The commit-result drain-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `matching_success`, `matching_rejected`, `matching_kura_retry` | `drain_commit_results(...)` takes the active `CommitInFlight` only when the worker result id matches, applies the outcome, records stage timings/progress, and kickstarts the pacemaker only when `apply_commit_outcome(...)` reports a durable commit. |
| `id_mismatch` | A stale result whose id does not match the active inflight commit is ignored and the real inflight commit is restored. |
| `no_inflight_result`, `no_result_rx`, `empty_result` | Ownerless results, absent result receivers, and empty channels do not apply outcomes or record progress; empty channels simply stop the drain loop. |
| `disconnected_no_inflight`, `disconnected_success`, `disconnected_rejected` | A disconnected result channel clears commit worker state, stops the drain loop, and executes inline fallback only when an inflight commit exists. |
| `disconnected_local_outside_with_qc`, `disconnected_local_outside_without_qc`, `disconnected_local_inside_with_qc` | Inline fallback enables signature-index recovery only when the local peer is outside the commit topology and the inflight commit carries a commit QC. |

The commit-job dispatch-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `existing_same_block` | `start_commit_job(...)` suppresses duplicate finalization when the active inflight commit has the same block hash, returning `false` without inserting another pending block. |
| `existing_other_block` | A new block encountered while a different commit is inflight is inserted into `pending.pending_blocks` and the existing inflight marker is preserved. |
| `worker_ready_enqueued` | A live `work_tx` plus `result_rx` hands `CommitWork` to the worker, records commit-inflight start, sets `subsystems.commit.inflight`, returns `true`, and does not run the commit inline on the actor thread. |
| `worker_queue_full` | A full worker queue returns `false`, keeps the pending block queued for retry, and does not set a stale inflight marker. |
| `worker_disconnected_inline_*` | A disconnected send clears commit worker state and falls back to `execute_commit_job_inline(...)`, leaving no worker-owned inflight marker after the inline result is applied. |
| `missing_work_tx_inline_*`, `missing_result_rx_inline_*`, `missing_both_worker_ends_inline_*` | Missing worker channel ends bypass enqueue attempts and execute inline without clearing worker state as disconnected. |

The commit-inflight timeout-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `timeout_zero_with_inflight`, `no_inflight` | `report_inflight_commit_if_timed_out(...)` returns `false` before inspecting elapsed time when timeout reporting is disabled or no commit job is inflight. |
| `clock_before_enqueue`, `below_timeout` | Elapsed time is computed with `saturating_duration_since(...)`; pre-enqueue clock skew and below-boundary elapsed time do not report or mark the job. |
| `at_timeout_unreported`, `above_timeout_unreported` | At or beyond `commit_inflight_timeout`, an unreported inflight job sets `timeout_reported`, records timeout status, warns, returns `true`, and preserves the inflight marker. Bridge coverage includes `commit_inflight_timeout_reports_and_keeps_inflight_result_attachable`. |
| `at_timeout_already_reported`, `above_timeout_already_reported` | A reported inflight job keeps its marker but returns `false` and does not duplicate diagnostics. |
| late-result attachability | The timeout path does not insert the block into `pending.pending_blocks`, prune proposal/view state, force a view change, record commit failure, apply an outcome, or kick the pacemaker. Bridge coverage includes `late_successful_commit_result_after_timeout_is_applied`. |

The post-commit pacemaker-kick model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `no_queue_*` | `kickstart_pacemaker_after_commit(...)` returns `false` without calling the trigger when `queue_len == 0`, regardless of backpressure state. |
| `queued_healthy_callback_true`, `queued_healthy_callback_false` | Queued transaction work without proposal backpressure calls the trigger and returns `true`; the trigger callback result is intentionally ignored. Bridge coverage includes `kickstart_pacemaker_after_commit_triggers_only_when_allowed`. |
| `queued_queue_saturated`, `queued_consensus_pacing`, `queued_combined_pacing` | Pacing-only backpressure still calls the trigger so a durable commit can immediately drain queued transaction work. |
| `queued_active_pending`, `queued_rbc_backlog`, `queued_relay_backpressure` | Hard backpressure from an active pending block, RBC backlog, or relay pressure suppresses the trigger. |
| `queued_active_pending_with_queue_saturated`, `queued_rbc_backlog_with_consensus`, `queued_relay_with_queue_saturated`, `queued_all_backpressure` | Pacing pressure does not override hard backpressure. |

The idle-view proposal-budget model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `preserve_no_queue`, `preserve_mode_flip`, `preserve_commit_inflight`, `preserve_deadline_not_due` | `should_preserve_idle_view_budget_for_proposal(...)` requires queued work, no pending mode flip, no commit inflight job, and a due proposal deadline or queue nudge. |
| `preserve_healthy_due` | A due proposal with queued work and no backpressure preserves the tick budget for proposal handling before idle-view repair. |
| `preserve_queue_saturated_due`, `preserve_consensus_pacing_due`, `preserve_combined_pacing_due` | Pacing-only pressure from queue saturation and/or consensus ingress backlog still preserves proposal budget. Bridge coverage includes `idle_view_budget_is_preserved_for_due_proposal_under_pacing_backpressure`. |
| `preserve_active_pending_due`, `preserve_rbc_backlog_due`, `preserve_relay_backpressure_due` | Hard proposal backpressure keeps idle-view repair available. |
| `preserve_active_pending_with_pacing`, `preserve_rbc_with_consensus`, `preserve_relay_with_queue` | Pacing pressure does not override active pending work, RBC backlog, or relay backpressure. |
| `retry_skipped_frontier_empty`, `retry_not_skipped`, `retry_no_queue`, `retry_pending_blocks`, `retry_commit_inflight` | `should_retry_idle_view_after_proposal(...)` retries idle repair only after a skipped due proposal while queued work remains, no pending blocks own the frontier, and no commit job is inflight. Bridge coverage includes `idle_view_repair_retries_after_skipped_due_proposal_only_when_frontier_empty`. |

The pacemaker evaluation-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `healthy_before_deadline`, `healthy_due` | Without proposal backpressure, `evaluate_pacemaker(...)` never logs deferral, clears the tracker, and attempts a proposal only when `pacemaker.should_fire(now)` fires. |
| `pacing_first_before_deadline`, `pacing_subsequent_before_deadline` | Pacing-only pressure before the deadline suppresses proposal attempts; the first transition logs both initial and fire deferral, while subsequent ticks stay quiet. |
| `pacing_first_due`, `pacing_subsequent_due` | Pacing-only pressure after the deadline advances the pacemaker, logs fire deferral, and still attempts proposal work. Bridge coverage includes `evaluate_pacemaker_fires_after_deadline_when_saturated`. |
| `hard_first_before_deadline`, `hard_subsequent_before_deadline` | Hard backpressure before the deadline records tracker deferral but does not log fire deferral or attempt proposal work. |
| `hard_first_due`, `hard_subsequent_due` | Hard backpressure after the deadline still advances the pacemaker and logs fire deferral, but it suppresses proposal work. Bridge coverage includes `evaluate_pacemaker_fires_after_deadline_under_consensus_queue_backpressure`. |
| `recovered_before_deadline`, `recovered_due` | A cleared backpressure signal resets `PacemakerBackpressure` without deferral logging; due recovered ticks attempt proposal work. |

The cached proposal-slot timeout model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `effective_zero_votes`, `effective_far_from_quorum`, `effective_at_quorum` | `cached_slot_effective_quorum_timeout(...)` requires at least one vote, fewer votes than quorum, and `precommit_votes_at_view + 1 >= quorum` before considering the near-quorum path. |
| `effective_near_fast_shorter` | Near-quorum payload repair with missing local data and no consensus/RBC backlog returns `near_quorum_payload_timeout(rebroadcast_cooldown).min(quorum_timeout)`; when that value is shorter, the cached slot uses the shorter retry. Bridge coverage includes `cached_slot_effective_timeout_uses_near_quorum_payload_window_only_without_backlog`. |
| `effective_near_fast_not_shorter` | The near-quorum path remains capped by the ordinary quorum timeout when the derived near-quorum window is longer. |
| `effective_near_no_missing_data`, `effective_near_consensus_backlog`, `effective_near_rbc_incomplete`, `effective_near_both_backlogs` | Missing local data is required, and consensus queue or RBC backlog disables the fast retry path. |
| `hysteresis_permissioned_mode`, `hysteresis_zero_quorum_timeout`, `hysteresis_no_previous`, `hysteresis_height_mismatch`, `hysteresis_same_view`, `hysteresis_lower_view` | `cached_slot_timeout_hysteresis_remaining(...)` returns `None` unless NPoS mode has a nonzero quorum timeout, prior same-height timeout history, and a strictly newer view. |
| `hysteresis_streak0_before`, `hysteresis_streak1_before`, `hysteresis_streak2_before`, `hysteresis_streak3_before` | `next_cached_slot_timeout_streak(...)` increments the previous streak for same-height newer views, while the hysteresis multiplier is capped at four quorum-timeout windows. |
| `hysteresis_boundary`, `hysteresis_after` | The wait is present only while elapsed time is strictly below the hysteresis window; boundary and later ticks do not delay rotation. Bridge coverage includes `npos_timeout_hysteresis_applies_after_previous_trigger`. |

The proposal parent-resolution model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `height_zero`, `height_one`, `height_one_pending_ignored` | `resolve_prev_block_for_proposal(...)` uses `checked_sub(1)` plus `NonZeroUsize`; heights zero and one have no previous Kura block lookup and cannot use pending fallback because `proposal_height > 1` is required. |
| `kura_parent`, `kura_preferred_over_pending` | A Kura-returned previous block is used directly, and pending fallback is skipped when Kura already supplied the parent. |
| `parent_missing_no_pending` | When no Kura block or matching pending parent exists above genesis, proposal assembly defers on `ParentMissing` without draining the queue. Bridge coverage includes `proposal_assembly_defers_without_draining_queue_and_preserves_view_when_parent_missing`. |
| `pending_parent` | Pending fallback returns the pending block only when it is stored under `highest_qc.subject_block_hash` and `pending.height + 1 == proposal_height`. Bridge coverage includes `resolve_prev_block_for_proposal_uses_pending_parent_when_available`. |
| `pending_wrong_hash`, `pending_wrong_height` | Wrong-subject or wrong-height pending blocks cannot become proposal parents. |
| `usize_overflow_pending_parent`, `usize_overflow_no_pending` | A previous-height conversion overflow skips the Kura lookup and records overflow diagnostics, but it does not prevent a matching pending-parent fallback. |
| `transport_*` | `should_seed_frontier_backup_transport(...)` returns true only for `(da_enabled, inline_frontier_block_created_transport, inline_block_created_backup) == (true, true, true)`, while proposal assembly uses RBC transport when DA is enabled and either primary RBC is required or inline backup is seeded. Bridge coverage includes `inline_frontier_backup_transport_requires_da_inline_and_config`. |

The precommit-QC view-change selector model is intentionally finite. These
are the implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `none_none` | `precommit_qc_for_view_change(None, None)` returns no QC. |
| `highest_prepare_no_committed`, `highest_prepare_committed` | `highest_qc.filter(|qc| qc.phase == Phase::Commit)` rejects non-Commit highest QCs before selection; committed QC is the fallback when present. |
| `highest_commit_no_committed`, `no_highest_committed` | Single-candidate cases return the only Commit-phase candidate. |
| `highest_commit_newer_height`, `highest_commit_higher_height_lower_view`, `highest_commit_same_height_newer_view`, `highest_commit_equal_slot` | A Commit-phase highest QC wins when `(highest.height, highest.view) >= (committed.height, committed.view)`, including equal height/view. |
| `highest_commit_same_height_older_view`, `highest_commit_older_height`, `highest_commit_lower_height_higher_view` | The committed QC wins when the Commit-phase highest QC is lexicographically older, even if its view is numerically higher at a lower height. |

The commit-evidence replay-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `missingPending`, `wrongRound`, `abortedPending` | `maybe_replay_known_block_commit_evidence(...)` returns before replay when no active pending block matches the exact height/view or the pending block is aborted. Bridge coverage includes `known_block_commit_evidence_replay_skips_aborted_pending_tracked`. |
| `cooldownVotes` | `block_sync_rebroadcast_log.allow(...)` suppresses repeated replay during the per-block cooldown. Bridge coverage includes `known_block_commit_evidence_replay_skips_during_cooldown`. |
| `firstVotesRemote`, `firstCommitQcRemote` | The first positive commit evidence snapshot may replay and records the pending block's replay state. Bridge coverage includes `known_block_commit_evidence_replay_skips_payload_fallback_without_roster` and `known_block_commit_qc_replay_targets_snapshot_roster`. |
| `stalledVotesRemote`, `stalledCommitQcRemote` | Stalled positive evidence is allowed to retry once the cooldown expires. Bridge coverage includes `known_block_commit_evidence_replay_retries_stalled_commit_evidence_after_cooldown`. |
| `voteCountProgressRemote`, `commitQcProgressRemote`, `viewProgressRemote` | `PendingBlock::should_replay_commit_evidence(...)` treats higher vote count, newly cached commit QC, or view change as replay progress; unit coverage includes `commit_evidence_replay_advances_on_progress`. |
| `firstNoEvidenceRemote`, `sameZeroNoProgress` | Zero-evidence snapshots must not schedule outbound vote/certificate work or payload fallback. Bridge coverage includes `commit_evidence_replay_cooldown_does_not_fallback_to_payload`. |
| `localOnlyVoteTargets`, `localOnlyCommitQcTargets` | Explicit target sets that collapse to the local peer return `false` without outbound work. Bridge coverage includes `known_block_commit_evidence_replay_returns_false_for_local_only_explicit_targets` and `known_block_commit_qc_replay_returns_false_for_local_only_explicit_targets`. |
| `duplicateVoteTargets` | `rebroadcast_block_votes_to_targets(...)` filters local targets and deduplicates explicit remotes. Bridge coverage includes `known_block_commit_evidence_replay_deduplicates_explicit_vote_targets`. |
| `VoteEvidenceUsesVoteReplay`, `CommitQcUsesCommitCertReplay`, `PayloadFallbackNeverUsed` | Vote replay uses `QcVote`, cached commit-QC replay uses `CommitCert`, and neither path rebuilds `BlockCreated` or `BlockSyncUpdate` payload traffic. Bridge coverage includes `known_block_commit_evidence_replay_uses_explicit_targets`, `known_block_commit_qc_replay_targets_snapshot_roster`, and `commit_evidence_replay_cooldown_does_not_fallback_to_payload`. |

The block-sync recovery-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `requestedStalePayload`, `staleNoRequest` | `handle_block_sync_update(...)` and `handle_block_created_with_preserve_policy(...)` admit stale-view recovery only for missing-block requests or commit evidence; bridge coverage includes `block_sync_update_accepts_stale_view_when_missing_block_requested` and `block_sync_update_drops_stale_view_without_missing_request`. |
| `staleCommitVotes`, `staleCommitQc` | Vote/QC-backed stale contiguous recovery enters `BlockSyncRecoveryMode::CommitEvidenceRepair` and may become the authoritative owner. Bridge coverage includes `block_sync_update_accepts_stale_view_with_commit_votes` and `block_sync_update_accepts_stale_view_with_commit_qc`. |
| `abortedPayloadOnly`, `abortedCommitQc` | Payload-only sparse updates keep aborted placeholders inactive, while commit-QC evidence may revive them and preserve the observed QC epoch. Bridge coverage includes `block_sync_update_keeps_aborted_next_height_payload_sparse_without_commit_evidence` and `block_sync_update_revives_aborted_next_height_payload_with_commit_qc`. |
| `sparseNextHeight`, `unknownFrontierVoteOnly` | Sparse next-height payload repair and unknown-frontier vote-only updates track missing commit-QC repair instead of silently becoming complete. Bridge coverage includes `block_sync_update_tracks_missing_commit_qc_for_next_height_sparse_payload_recovery` and `block_sync_update_tracks_missing_qc_for_unknown_frontier_vote_only_update`. |
| `payloadOnlyStaleInflight`, `certifiedStaleInflight` | Payload-only exact repair does not clear stale commit inflight or steal owner state, but certified repair may bypass stale inflight and clear it. Bridge coverage includes `block_sync_update_accepts_stale_exact_frontier_payload_repair_with_da` and `block_sync_update_commit_qc_bypasses_stale_commit_inflight_frontier_owner`. |
| `sameHeightRawQuorumConflict`, `sameHeightCertifiedConflict` | Raw block-signature quorum can hydrate a passive retained branch, while certified evidence may supersede a stale same-height frontier owner. Bridge coverage includes `block_sync_update_same_height_conflict_with_block_quorum_stays_passive_without_certified_evidence` and `block_sync_update_commit_qc_supersedes_stale_same_height_frontier_owner`. |
| `cachedCommitQcPayload`, `unvalidatedCommitQc`, `unrequestedFuture` | Cached commit-QC payload recovery remains authoritative, unvalidated sidecar QC cannot advance lock/highest-QC state, and unrequested future-height updates are dropped. Bridge coverage includes `block_sync_payload_with_cached_commit_qc_supersedes_lock_conflicting_stale_frontier_owner`, `block_sync_update_does_not_advance_qc_for_unvalidated_payload`, and `block_sync_update_drops_unrequested_future_height_beyond_active_frontier_lanes`. |

The direct certified-block fetch model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `nonCommitQc`, `signerTargets`, `outOfRangeSigners`, `localOnlyTargets`, `remoteTargets` | `request_certified_block_for_header(...)`, `build_fetch_targets(...)`, and `send_certified_block_fetch_request(...)` request exact certified blocks only from commit-QC evidence, prefer QC signer targets with topology fallback, remove the local peer, sort/deduplicate remotes, use `Priority::High`, and avoid generic missing-block fetch fallback. |
| `forgedRequester`, `localBlockMissing`, `localSubjectMismatch`, `localCommitQcMissing`, `localCommitQcMismatch` | `handle_certified_block_fetch_request(...)`, `local_signed_block_for_body_repair(...)`, and `certified_block_fetch_response_for_block(...)` fail closed before serving malformed or uncertified local data. |
| `nposResponse` | `certified_block_fetch_response_for_block(...)` attaches an NPoS stake snapshot only when it matches the certified validator checkpoint. |
| `smallFull`, `oversizedFull`, `oversizedProof`, `oversizedBody`, `oversizedBodyResponse`, `oversizedAll` | `dispatch_certified_block_fetch_response(...)` sends a full response only under the wire cap, otherwise splits proof/body and falls back to `BlockBodyResponse` or `BlockCreated` before dropping oversized body material. |
| `validResponse`, `validProof`, `malformedProof`, `heightMismatch`, `viewMismatch`, `blockHashMismatch`, `qcHeightMismatch`, `qcViewMismatch`, `uncertifiedResponse`, `checkpointMismatch` | `CertifiedBlockFetchResponse::validate_subject(...)`, `CertifiedBlockFetchProof::validate_subject(...)`, and `validate_certified_fetch_proof_parts(...)` self-validate block, QC, certification, and validator-checkpoint metadata before a response can mutate recovery state. |
| `proofAccepted` | `accept_certified_block_fetch_proof(...)` accepts only validated proof companions, feeds commit-QC admission, caches the certified QC, records roster/stake metadata, and clears exact missing commit-QC requests. |
| `bodyWithoutProof`, `bodyMismatchedProof`, `proofThenBody` | `handle_certified_block_fetch_body(...)` admits body companions only after a matching cached proof/QC pair for the same height, view, and block hash. |
| `fullResponse` | `handle_certified_block_fetch_response(...)` validates the response, accepts its proof material, and materializes only after proof admission succeeds. |
| `invalidInflight`, `invalidPending`, `retryAborted`, `materializationDeferrals` | `materialize_certified_block_fetch_response(...)` rejects invalid inflight/pending owners, revives retry-aborted pending blocks, caches the body, clears missing-block/view-change/deferred-QC state, flushes requesters, replays deferred QCs, and wakes the commit pipeline. |

The native AMX attestation-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `nonNativePlan`, `emptyRoster` | `native_amx_receipt_for_plan(...)` returns `Ok(None)` for non-AMX plans and fails closed when `native_amx_vote_roster()` is empty. |
| `noPrepareVotes`, `prepareBelowQuorum`, `commitWithoutPrepare` | Missing prepare quorum schedules `NativeAmxMessage::PrepareRequest` and does not request commit attestations yet. |
| `prepareQuorumNoCommitVotes`, `prepareQuorumCommitBelowQuorum` | Once prepare quorum exists, missing commit quorum schedules `NativeAmxMessage::CommitRequest`. |
| `fullQuorumSingleLeg`, `fullQuorumMultiLeg`, `oneLegPendingMultiLeg` | A receipt is sealed only when every participant leg has both prepare and commit QCs; any pending leg defers the whole native AMX proposal batch without a partial receipt. |
| `duplicatePrepareSigner`, `duplicateCommitSigner`, `wrongPrepareBody`, `wrongCommitBody`, `outsiderPrepareSigner`, `outsiderCommitSigner` | `NativeAmxSessionCache::insert_vote(...)` and `aggregate_votes_to_qc(...)` reject duplicate exact-body signers, wrong attestation bodies, and signers outside the validator set. Bridge coverage includes `session_cache_rejects_duplicate_signer` and `aggregate_votes_to_qc_rejects_bad_vote_sets`. |
| `unsortedQuorumVotes` | `aggregate_votes_to_qc(...)` projects votes into validator-set order before building the bitmap and BLS aggregate. Bridge coverage includes `aggregate_votes_to_qc_orders_votes_by_validator_set`. |
| `retriedHeightSameSigner`, `differentParticipantSameSigner` | The session cache scopes duplicate checks to exact attestation bodies, so retried heights and distinct participant legs do not collide. Bridge coverage includes `session_cache_allows_same_signer_for_retried_body` and `session_cache_allows_same_signer_for_different_participant_legs`. |

The native AMX journal-replay model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `nativePutReplay`, `singlePutReplay` | `QueuePlanJournalRecordV1` stores the full `RoutingPlan`, and replay returns the same plan variant. Bridge coverage includes `native_amx_queue_journal_replays_plan_after_restart`. |
| `participantOrder`, `participantDedup` | `RoutingPlan::native_amx(...)` sorts and deduplicates participant legs by dataspace and lane before computing the plan digest. Bridge coverage includes `mixed_domain_write_targets_across_dataspaces_build_native_amx_plan`. |
| `digestPreserved`, `gossipPayloadPreserved`, `entrypointPreserved` | Journal records persist `routing_plan`, `gossip_payload`, and `entrypoint`; `plan_digest()` is derived from the stored routing plan. Bridge coverage includes `journal_replays_puts_and_removes` plus native AMX routing tests. |
| `removeExactDigest`, `removeOtherDigest`, `readmitSameHashNewDigest` | `QueuePlanJournal::replay()` keys live entries by `(signed_transaction_hash, plan_digest)`, so removes tombstone only the exact plan digest. |
| `duplicateSameKeyLastWins` | Replayed puts are inserted into a `BTreeMap`, so later puts for the same `(hash, digest)` replace earlier records. |
| `unsupportedVersionIgnored` | Replay ignores records whose `version` is not `QUEUE_PLAN_JOURNAL_VERSION`. |
| `compactionKeepsLive`, `compactionDropsRemoved` | `compact_if_needed()` replays the journal and rewrites only live `Put` frames. |
| `tornPayloadTailPreservesPrior`, `tornLengthTailPreservesPrior` | `QueuePlanJournal::open()` calls `repair_incomplete_tail(...)`, preserving complete prefix frames while truncating incomplete tails. Bridge coverage includes `journal_open_truncates_torn_payload_tail_before_append`. |

The vNext chain-order helper model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `empty_order`, `zero_critical`, `critical_after_end` | `ChainOrder::new(...)` rejects empty validator orders, zero critical prefixes, and critical prefixes longer than the order. |
| `quarantine_before_critical`, `quarantine_after_end` | `ChainOrder::new(...)` rejects quarantine starts before the critical prefix and beyond the end of the order. |
| `valid_order` | `critical_path()` returns exactly `ordered_validators[..critical_prefix_len]`, excluding the quarantine tail. |
| `successor_first`, `successor_tail`, `successor_quarantine`, `successor_unknown` | `successor_of(...)` returns the next validator only while the successor remains inside the critical prefix. Critical-tail, quarantine-tail, and unknown peers return `None`. |
| `count_prefix_minimal`, `count_prefix_none` | `QuorumPolicy::smallest_satisfying_prefix_len(...)` scans prefixes in order and returns the first count quorum, or `None` when no prefix can satisfy the required count. |
| `stake_prefix_minimal`, `stake_exact_boundary`, `stake_missing_weight`, `stake_zero_total` | `stake_quorum_satisfied(...)` requires known weights, nonzero total stake, checked arithmetic, and strict greater-than two-thirds stake before a prefix satisfies NPoS quorum. |
| `bitmap_empty_roster`, `bitmap_one_signer`, `bitmap_eight_signers`, `bitmap_nine_signers`, `bitmap_duplicate`, `bitmap_out_of_range` | `build_signer_bitmap(...)` uses `signer_bitmap_len(roster_len)`, rejects duplicate signer indices, and rejects indices outside the roster. |

The vNext re-chain helper model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `slot_mismatch`, `hash_mismatch`, `seq_mismatch` | `ChainOrder::validate_suspicion(...)` rejects suspicions whose slot, chain-order hash, or re-chain sequence does not match the local order. |
| `non_successor`, `tail_accuser` | `ChainOrder::validate_successor_pair(...)` accepts only the accuser's current critical-path successor and rejects accusers with no critical successor. |
| `duplicate`, `multi_no_longer_successor` | `rechain_after_suspicions(...)` canonicalizes by suspicion signing-body hash, rejects duplicate bodies, and revalidates each suspicion against the evolving order before applying the next one. |
| `insufficient_untainted`, `count_quorum_fail`, `stake_boundary` | `rebuild_with_tainted_tail(...)` requires enough untainted validators for the critical prefix and rechecks `QuorumPolicy::satisfied_by(...)`, including strict greater-than two-thirds stake. |
| `success_count`, `multi_success` | Accepted evidence moves each accuser and accused into the quarantine tail, increments `rechain_seq` per applied suspicion, and changes the chain-order hash in the returned `RechainCertificate`. |

The vNext signature gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `rechain_slot_mismatch`, `rechain_hash_mismatch`, `rechain_seq_mismatch` | `RechainCertificate::validate_body_consistency(...)` rejects inconsistent embedded order fields before aggregate verification. |
| `missing_signature`, `bad_aggregate_signature` | `verify_preaggregated_vnext_signature(...)` rejects empty aggregate signatures and maps failed BLS aggregate verification to `BadAggregateSignature`. |
| `empty_roster`, `wrong_bitmap_length`, `bitmap_oob`, `empty_signer_set` | `signer_peers_from_bitmap(...)` requires a non-empty roster, canonical bitmap length, no out-of-range bits, and at least one selected signer. |
| `pop_len_mismatch`, `non_bls_signer` | The verifier requires signer PoP entries to align with the signer roster and rejects non-BLS-normal signer keys before aggregate verification. |
| `under_count_quorum`, `stake_boundary` | `QuorumPolicy::satisfied_by(...)` gates selected bitmap signers for both count and stake policies, preserving strict greater-than two-thirds stake semantics. |
| `valid_rechain_count`, `valid_view_count`, `valid_view_stake` | `RechainCertificate::verify_aggregate_signature(...)` and `ViewChangeCertificate::verify_aggregate_signature(...)` return exactly the bitmap-selected signer set on success. |

The vNext signing-preimage model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `rechain_vote_unsigned`, `view_vote_unsigned` | `RechainVote::unsigned(...)` and `ViewChangeVote::unsigned(...)` project the certificate body plus signer and initialize an empty signature. |
| `rechain_vote_preimage`, `rechain_cert_preimage` | `RechainVote::signing_preimage(...)` and `RechainCertificate::signing_preimage(...)` both call `rechain_certificate_signing_preimage(...)` over the same body fields. |
| `view_vote_preimage`, `view_cert_preimage` | `ViewChangeVote::signing_preimage(...)` and `ViewChangeCertificate::signing_preimage(...)` both call `view_change_certificate_signing_preimage(...)` over the same body fields. |
| `DomainFields` | `vnext_signing_preimage(...)` prefixes the Norito body with consensus domain material derived from chain id, message type, `vnext-v1`, and the mode tag. |
| `SignatureAndBitmapFields` | Vote signatures, aggregate signatures, and signer bitmaps are mutable verification material and are intentionally excluded from aggregate signing bodies. |
| `suspect_hash` | `Suspect::signing_body_hash(...)` hashes the Norito-encoded `Suspect::signing_body()`, including slot, accuser, accused, obligation, chain-order hash, re-chain sequence, and observed delay while excluding the signature. |

The vNext control-ingress model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `rechain_no_round` | `handle_vnext_rechain_certificate_received(...)` calls `install_vnext_rechain_certificate(...)` and returns when no round is installed, so it must not mutate live round state or require view change. |
| `rechain_already_current` | The same handler treats a certificate whose new chain order already matches the current round as a no-op. |
| `rechain_hash_mismatch`, `rechain_evidence_mismatch` | Previous-chain-order mismatches and expected-body mismatches are rejected with only diagnostic logging. |
| `rechain_valid_within_max` | A deterministic re-chain result that matches the certificate and stays within `max_tainted_per_view` updates `round.chain_order`, records `last_rechain_ms`, and installs the certificate. |
| `rechain_valid_exceeds_max`, `rechain_would_weaken_quorum` | Over-taint and quorum-weakening re-chain evidence call `require_vnext_view_change(...)` instead of installing or updating the round. |
| `clearWorkerOwner`, `broadcastViewChangeVote`, `triggerViewChange` | `require_vnext_view_change(...)` clears vNext worker ownership, broadcasts a local view-change vote for `slot.view + 1`, and calls `trigger_view_change_with_cause(..., CensorshipEvidence)`. |
| `view_highest_installed`, `view_highest_missing_round`, `view_no_highest`, `view_zero_highest_installed` | `handle_vnext_view_change_certificate_received(...)` aborts only an installed highest slot before `install_vnext_view_change_certificate(...)`; installing a certificate triggers live view-change handling only when `new_view > 0`. |

The vNext slot-lifecycle model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `proposal_no_base`, `proposal_idle`, `proposal_committed` | `drive_vnext_proposal_accepted_for_block(...)` and `mark_vnext_proposal_accepted(...)` install rounds through `ensure_vnext_round(...)`, preserve committed slots, and request validation only for live slots. |
| `availability_no_base`, `availability_idle`, `availability_committed` | `drive_vnext_availability_ready_for_block(...)` and `mark_vnext_availability_ready(...)` follow the same installed-round and committed-slot guards before driving validation. |
| `validation_no_base`, `validation_unqueued_dispatch`, `validation_committed` | `drive_vnext_validation_for_pending(...)`, `drive_vnext_validation_needed(...)`, and `dispatch_vnext_validation(...)` dispatch only installed, non-committed, unqueued slots and record the running validation owner. |
| `worker_started_matching`, `worker_started_stale` | `mark_vnext_validation_worker_started(...)` mutates only the matching queued owner and leaves stale callbacks side-effect free. |
| `queue_full_matching`, `queue_full_stale` | `handle_vnext_validation_queue_full(...)` records backpressure only for the matching queued owner and ignores stale queue-full callbacks. |
| `result_valid_matching`, `result_invalid_matching`, `result_stale_wrong_owner`, `result_terminal_committed` | `handle_vnext_validation_result(...)` accepts only matching current validation results, prepares valid slots, aborts invalid slots, and ignores stale or terminal callbacks. |
| `defer_running`, `defer_committed` | `mark_vnext_validation_deferred(...)` resets only non-committed validation state and preserves committed slots. |
| `tick_running_*`, `tick_backpressure_*`, `tick_terminal_*` | `tick_vnext_rounds(...)` starts recovery only for due unprotected running or backpressured validation and ignores protected or terminal slots. |
| `commit_persisted_any`, `commit_no_base` | `drive_vnext_commit_persisted_for_block(...)` and `mark_vnext_commit_persisted(...)` make installed slots sticky-committed and do not install missing rounds. |
| `recoveryEffect` | `start_vnext_recovery(...)` is modeled as a recovery-only side effect, not as validation dispatch or result acceptance/rejection. |

The vNext validation gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `unqueued_dispatch`, `queued_await`, `valid_accept`, `invalid_reject` | `ValidationState::decision_at(...)` maps unqueued state to `DispatchWorker`, queued state to `AwaitWorker`, valid state to `Accept`, and invalid state to `Reject`. |
| `running_before_timeout`, `running_at_timeout`, `running_after_timeout` | `decision_at(...)` raises `RaiseSuspicion` only when `elapsed_ms(now_ms, started_at_ms) >= suspicion_timeout_ms`; otherwise it awaits the worker. |
| `backpressured_before_timeout`, `backpressured_at_timeout`, `backpressured_after_timeout` | `decision_at(...)` keeps backpressure before the same timeout boundary and raises suspicion at or after the boundary. |
| `running_now_before_started` | `elapsed_ms(...)` uses saturating subtraction, so a sampled clock before `started_at_ms` has elapsed zero instead of underflowing. |
| `worker_started_records_owner` | `ValidationState::worker_started(...)` replaces the previous state with `Running { id, generation, started_at_ms }`. |
| `worker_result_valid_matching`, `worker_result_invalid_matching` | `apply_worker_result(...)` applies matching owner results and reaches `Valid` or `Invalid`. |
| `worker_result_wrong_id`, `worker_result_wrong_generation`, `worker_result_not_running` | `apply_worker_result(...)` returns `IgnoredStale` without mutating state when ownership no longer matches or the state is not running. |

The vote-verify async-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `dispatch_no_workers_inline`, `dispatch_disconnect_all_inline` | `try_dispatch_vote_verification(...)` returns `false` when no worker sender is usable, so `handle_vote(...)` continues through inline verification; disconnected senders are removed. |
| `dispatch_duplicate_inflight`, `dispatch_duplicate_pending` | `try_dispatch_vote_verification(...)` drops duplicate votes whose `VoteVerifyKey` is already in `inflight` or `pending`. |
| `dispatch_send_success`, `dispatch_queue_full` | Successful sends insert `VoteVerifyInFlight`; full worker queues insert `VoteVerifyPending` without applying the vote. |
| `pending_no_workers`, `pending_success`, `pending_queue_full_keep` | `dispatch_pending_vote_verifications(...)` keeps pending votes when workers are absent or full and moves only successfully sent work into `inflight`. |
| `poll_no_inflight`, `poll_id_mismatch` | `poll_vote_verify_results(...)` ignores worker results without a matching in-flight entry or with a stale id; the id-mismatch path removes the stale owner before continuing. |
| `poll_stale_height_view`, `poll_locked_precommit`, `poll_penalized` | Polling rechecks height/view, locked-precommit, and invalid-signature penalty guards before vote application. |
| `poll_invalid_signature`, `poll_valid_signature` | `validate_and_record_vote_with_signature_result(...)` rejects invalid signatures and `apply_validated_vote(...)` runs only after a matching valid signature result. |
| `poll_channel_disconnected`, `poll_no_rx_dispatch_pending` | A disconnected result channel clears worker senders and owned work; a missing result receiver can still drain pending votes through `dispatch_pending_vote_verifications(...)`. |

The QC-verify async-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `consensus_cached_verified` | `handle_qc_with_aggregate_and_roster_and_stake(...)` consults `qc_verify.verified_cache` and continues with `aggregate_ok = Some(true)` instead of dispatching another worker job. |
| `consensus_no_workers_inline`, `consensus_small_inline`, `consensus_force_inline`, `consensus_missing_inputs_inline`, `consensus_queue_full_inline` | Consensus-QC handling verifies inline when workers are absent, the committee is below `QC_VERIFY_INLINE_ROSTER_MAX`, inline verification is forced, aggregate inputs cannot be prepared, or every worker lane is full. |
| `consensus_send_success`, `consensus_duplicate_inflight` | Successful consensus-QC dispatch inserts `QcVerifyInFlight { target: Consensus }`; duplicate in-flight keys are dropped without a second owner. |
| `known_stale_lock_drop` | `apply_known_block_qc_work(...)` calls `block_sync_qc_is_stale_against_lock(...)` before aggregate-verification dispatch. |
| `known_cached_tally`, `known_no_workers_inline`, `known_missing_inputs_inline`, `known_queue_full_inline` | Known-block QC work skips dispatch when cached tally evidence exists or falls through to inline validation when workers cannot take ownership. |
| `known_send_success`, `known_duplicate_inflight` | Known-block dispatch inserts `QcVerifyInFlight { target: KnownBlock }`; duplicate keys return without applying the work. |
| `poll_no_inflight`, `poll_id_mismatch` | `poll_qc_verify_results(...)` ignores worker results without a matching in-flight entry or with a stale id, removing the stale owner on id mismatch. |
| `poll_consensus_result`, `poll_known_result` | Matching worker results route consensus QCs through `handle_qc_with_aggregate(qc, Some(aggregate_ok))` and known-block QCs through `apply_known_block_qc_work(...)` with `aggregate_ok` attached. |
| `poll_channel_disconnected`, `consensus_disconnect_all_inline`, `known_disconnect_all_inline` | Disconnected worker/result channels clear worker senders, in-flight ownership, and the dead result receiver so later QC handling falls back inline. |

The worker-drain scheduler model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `vote_only`, `vote_with_payload_backlog` | `drain_mailbox(...)` prefers `PriorityTier::Votes` while vote budget remains and no stronger repair priority is active. |
| `after_vote_burst_payload` | Once `stats.votes_handled >= vote_burst`, `select_oldest_pending(...)` can rotate to payload/RBC tiers. |
| `frontier_body_repair_payload`, `frontier_body_repair_block` | `actor.prioritize_frontier_body_repair()` chooses the oldest pending frontier body-repair tier before ordinary vote preference. |
| `quorum_vote_over_payload`, `force_vote_over_starved_payload` | `actor.prioritize_vote_drain()` forces vote handling over starved payload/RBC tiers when quorum recovery is waiting on queued votes. |
| `overtime_payload_turn` | A pre-tick drain that reaches the time budget after vote-only progress grants one non-vote payload turn before marking budget exhaustion. |
| `block_urgent_no_payload`, `starved_block_preempts_vote` | `block_rx_urgent_gap(...)` and starved-tier selection let block backlog escape vote preference when no payload tier is pending or the block tier is oldest-starved. |
| `starved_payload_preempts_after_progress`, `starved_payload_suppressed_first_turn` | Starved payload/RBC tiers can preempt after prior progress, while first-turn payload preemption is suppressed when votes are pending. |
| `low_consensus_after_high_empty` | `select_next_tier(...)` drains low-priority consensus/control tiers only after high-priority tiers have no selectable work. |
| `budget_zero_vote_skips`, `budget_exhausted_pending_vote` | Zero remaining budget prevents vote selection and `refresh_budget_exhaustion_flags(...)` records pending work after budget exhaustion. |
| `pre_tick_deadline_first_turn`, `post_tick_deadline_stops` | Pre-tick deadline handling allows an initial progress turn before breaking, while post-tick deadline handling stops without consuming more work. |
| `result_poll_before_tick`, `tick_due_busy_gap`, `tick_due_bypass_gap`, `post_tick_skipped_when_budget_exceeded` | `run_worker_iteration(...)` polls commit/validation/QC/vote/RBC results before tick, chooses busy tick gaps under backlog, honors explicit tick-gap bypass, and skips post-tick drain after budget exhaustion. |

The actor-gate priority model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `inflight_blocks_all` | The leading `state.in_flight` check in `ActorGate::can_enter(...)` serializes access for every `GatePriority`. |
| `availability_body_first`, `availability_body_defers_to_critical_after_body_cap` | `GatePriority::AvailabilityBody` can take a bounded body burst and then yields to waiting availability-critical work at `MAX_AVAILABILITY_BODY_GATE_STREAK`. |
| `availability_critical_waits_for_body_burst`, `availability_critical_after_body_cap` | `GatePriority::AvailabilityCritical` waits behind body repair while the body burst is below its cap, then can enter. |
| `availability_burst_defers_to_urgent`, `availability_burst_defers_to_da_critical` | Availability work yields to urgent and DA-critical waiters once `availability_streak` reaches `MAX_AVAILABILITY_GATE_STREAK`. |
| `urgent_waits_for_availability_burst`, `urgent_after_availability_cap`, `urgent_before_da_critical_cap`, `urgent_defers_to_da_critical_after_cap` | Urgent work waits for bounded availability bursts and yields to DA-critical work after `max_urgent_before_da_critical`. |
| `da_critical_waits_for_availability_burst`, `da_critical_waits_for_urgent_cap`, `da_critical_after_urgent_cap` | DA-critical work waits for the availability and urgent caps, then enters before lower-priority work. |
| `regular_waits_for_availability`, `regular_waits_for_da_critical`, `regular_waits_for_urgent_until_cap`, `regular_after_urgent_cap` | Regular work waits behind availability, DA-critical, and urgent bursts until the urgent streak reaches `MAX_URGENT_GATE_STREAK`. |
| `availability_body_entry_effects`, `availability_critical_entry_effects`, `urgent_entry_effects`, `da_critical_entry_effects`, `regular_entry_effects` | `ActorGate::enter(...)` sets `in_flight`, decrements the entering waiter counter, and updates availability/body/urgent streaks according to the entered priority. |
| `drop_urgent_keeps_urgent_streak`, `drop_non_urgent_resets_urgent_streak` | `ActorGuard::drop` clears `in_flight`, wakes all waiters, and preserves the urgent streak only for urgent turns. |

The worker-budget adaptive-cap model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `worker_zero_window_floor`, `worker_small_window_floor`, `worker_mid_window_quarter`, `worker_large_window_cap`, `worker_config_cap`, `worker_da_multiplier_ignored` | `worker_time_budget(...)` chooses the smaller block/commit cadence window, divides by four, applies the 50 ms floor and 2 s/config cap, and ignores DA quorum multipliers for per-iteration responsiveness. |
| `vote_da_quorum_window`, `vote_da_multiplier_window`, `vote_max_budget_cap`, `vote_config_cap`, `vote_zero_floor` | `vote_rx_drain_budget(...)` uses the DA quorum window when DA is enabled, applies the DA multiplier, clamps to `max_budget` and the configured cap, and floors at 1 ms. |
| `drain_floor`, `drain_global_cap`, `vote_drain_floor`, `rbc_config_cap` | `cap_drain_budget(...)`, `cap_vote_drain_budget(...)`, and `cap_rbc_drain_budget(...)` preserve floor/cap behavior for non-vote, vote, and RBC drains. |
| `idle_gap_floor`, `idle_gap_max`, `busy_gap_floor`, `busy_gap_idle_cap` | `idle_tick_gap(...)` and `busy_tick_gap(...)` clamp tick cadence between their floors and the configured idle/max gap bounds. |
| `block_depth_zero`, `block_depth_small`, `block_depth_medium`, `block_depth_large`, `block_depth_huge` | `block_backlog_drain_cap(...)` maps queue-depth boundaries to zero, small, medium, large, and huge block backlog caps. |
| `vote_backlog_payload_reduced`, `vote_backlog_rbc_preserved` | `apply_adaptive_drain_caps(...)` reduces block-payload caps under vote backlog but leaves RBC caps alone so RBC repair ingress is not throttled by vote-only pressure. |
| `block_backlog_block_cap`, `block_backlog_payload_min`, `block_backlog_payload_scaled`, `block_backlog_rbc_scaled`, `no_backlog_preserves_caps` | Block backlog caps block drains by the tiered backlog cap, scales payload/RBC caps from the target block cap with a minimum repair floor, and leaves caps unchanged when there is no vote or block backlog. |

The worker-ingress routing model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `route_qc_vote`, `route_proposal_hint`, `route_vrf_commit`, `route_vrf_reveal` | Vote-like block messages route through `SumeragiHandle::incoming_block_message_with_mode(...)` to the `votes` queue with `WorkerQueueKind::Votes`. |
| `route_qc_cert`, `route_block_created`, `route_proposal` | Compact QC/body/proposal messages use the protected block-payload queue with `WorkerQueueKind::BlockPayload`. |
| `route_rbc_init`, `route_rbc_chunk`, `route_rbc_ready`, `route_rbc_deliver`, `route_block_sync_update` | RBC session and block-sync evidence route to the unified RBC session queue with `WorkerQueueKind::RbcChunks`. |
| `route_block_body_response`, `route_fetch_pending_block`, `route_fetch_block_body`, `route_certified_fetch`, `route_other_block` | Exact body repair, fetch requests, certified fetches, and fallback block messages route to the block queue with `WorkerQueueKind::Blocks`. |
| `route_consensus_control`, `route_lane_relay`, `route_merge_signature`, `route_native_amx`, `route_background_*` | Consensus control, lane relay/native AMX/merge messages, and background post/broadcast tasks use the `Consensus`, `LaneRelay`, and `Background` worker queues. |
| `enqueue_blocking_success`, `enqueue_blocking_send_failure`, `enqueue_nonblocking_success`, `enqueue_nonblocking_full`, `enqueue_nonblocking_disconnected` | `enqueue_with_mode(...)` and the control/lane/background enqueue helpers record enqueue/drop status, wake the worker loop on accepted or blocking attempts, attach queue metadata, and account blocking sends. |
| `worker_votes`, `worker_rbc_chunks`, `worker_blocks`, `worker_block_payload`, `worker_consensus`, `worker_lane_relay`, `worker_background` | `run_parallel_worker(...)` maps queues to gate priorities, stages, handler labels, and batch limits when spawning queue workers. |
| `worker_batch_limit_floor`, `worker_batch_limit_respected`, `worker_batch_stops_on_empty` | `drain_queue_batch(...)` floors configured batch limits at one, never drains past the effective limit, and stops when `try_recv` reports an empty queue. |
| `worker_handler_error_keeps_drain`, `worker_last_active_restores_idle`, `worker_not_last_active_keeps_stage` | Queue workers poll worker-result channels after handler errors, record the drain count, and restore the idle stage only when the active-worker counter reaches zero. |

The NPoS VRF epoch-seal model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `MergeCompatible`, `MergeHeaderMismatch` | `merge_vrf_epoch_records(...)` preserves immutable epoch, seed, window, and roster identity fields and rejects mismatched record headers. |
| `MergeCommitmentRewrite`, `MergeRevealRewrite`, `MergeLateRevealRewrite` | The VRF observation merge keeps canonical same-signer commitments, reveals, and late reveals instead of accepting rewrites. |
| `MergePenaltyHeightRewrite`, `MergePenaltyMarkerSticky` | `merge_vrf_penalty_marker(...)` keeps penalty markers and their epoch heights sticky once known. |
| `MergeOffenderOverlap`, `MergeUnfinalizedOffenders`, `MergeFinalizedSticky` | Finalized offenders remain sticky, active roster overlap is rejected, and unfinalized offender candidates are stripped from merged records. |
| `MergeHeightMax`, `MergePreserveExistingObservation`, `MergeAddIncomingObservation` | Merged records use the maximum update height while preserving existing observations and adding incoming compatible observations. |
| `MergeElectionRewrite`, `MergeElectionSticky` | Validator-election outcomes become sticky once present and cannot be rewritten by later compatible records. |
| `StageAlreadyCovered`, `StageExtendCommitted`, `StageReplaceBetterSnapshot`, `StageRejectConflict` | `stage_vrf_epoch_record(...)` and `reconcile_pending_vrf_record_with_committed(...)` drop committed-covered pending state, merge compatible committed progress, replace stale pending state with better committed snapshots, and reject incompatible snapshots when no committed cover exists. |
| `CommittedEffectCoversPending`, `CommittedEffectExtendsPending`, `CommittedEffectConflict` | `note_committed_npos_effects(...)` and `committed_vrf_record_covers_pending(...)` reconcile canonical block effects with pending state instead of keeping covered, regressive, or conflicting pending records. |
| `ActivationEmpty`, `ActivationBeforeMargin`, `ActivationAtMargin` | `activation_plan_from_vrf_record(...)` installs elected rosters only when an election exists and the activation margin has elapsed. |
| `EffectPenaltyHeightNeedsMarker`, `EffectNoDuplicateParticipants`, `EffectNoDuplicateOffenders`, `EffectOffendersInRoster` | `validate_npos_effects_with_state(...)` rejects penalty heights without markers, duplicate participants, duplicate offenders, offenders outside the epoch roster, and finalized offender sets that retain active validators. |

The Kura durability commit retry model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `AlignNoKura`, `AlignMissingStateTip`, `AlignLowerStateHeight`, `AlignWrongStateHash`, `AlignKuraStateTip` | `kura_and_state_aligned_for_block(...)` requires the block to be durable, the state tip hash to exist and match the pending hash, and the state height to cover the pending height. |
| `KuraBackoffDefers` | The commit pipeline checks `PendingBlock::kura_retry_due(...)` and defers finalization while the retry window is still closed. |
| `KuraAbortedCleans` | Kura-aborted pending blocks are removed from the active path, block-scoped evidence/RBC state is cleaned, `reset_qcs_after_kura_abort(...)` restores safe anchors, and a commit-failure view change is triggered. |
| `AlreadyDurableMarksPending`, `MarkPersistedResetsRetry` | Already durable blocks call `PendingBlock::mark_kura_persisted(...)`, which records durability and clears retry attempts/backoff before replaying state commit. |
| `AlreadyCommittedSkips` | Kura/state-aligned duplicates are dropped without reapplying the block while settled RBC and parent-QC evidence are cleaned. |
| `StoreFailureRetry`, `StoreFailureExhausted` | `handle_kura_store_failure(...)` keeps pending state during retry backoff, but after the retry budget is exhausted it requeues transactions, removes unsafe block-scoped evidence, resets consensus anchors, and triggers recovery. |
| `StateHeightMismatchAligned`, `StateHeightMismatchConflict` | State-commit height-mismatch handling distinguishes duplicate already-applied state from conflicting advanced state, requeueing transactions and clearing proposal cache state only for the conflict branch. |
| `StateCommitOtherFailure` | Non-height state-commit failures keep the pending block for retry with Kura persistence marked, avoiding duplicate durable appends. |
| `CommitMissingQcDefers`, `CommitBeforeTipDefers` | Finalization requires observed commit-QC evidence and an extension of the current state tip. |
| `AbortedWithoutQcDefers`, `AbortedWithQcRevives`, `RetiredWithoutQcDefers`, `RetiredWithQcProceed` | Aborted or retired pending blocks remain deferred unless commit-QC evidence plus tip extension makes them safe to revive/finalize. |
| `ResetQcWithFallback`, `ResetQcWithoutFallback` | `reset_qcs_after_kura_abort(...)` restores lock/highest-QC status from the latest committed fallback or clears to the durable state anchor. |

The restarted-peer replay model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `RestoreDigestMismatch`, `RestoreSignatureMismatch`, `RestoreMerkleMismatch` | `select_digest_with_fallback(...)`, `verify_signature_with_fallback(...)`, and `verify_merkle_with_fallback(...)` reject corrupted snapshot payload metadata before deserialization is trusted. |
| `RestoreChainIdMismatch` | `try_read_snapshot_bundle(...)` rejects snapshot state whose chain id differs from the expected node chain id. |
| `RestoreSnapshotAhead` | `try_read_snapshot_bundle(...)` rejects snapshots whose committed hash list is taller than durable Kura. |
| `RestoreMissingOfflineKeys` | Nonempty snapshots must contain the durable `offline_note_v2_replay_keys` world field before restart can proceed. |
| `RestoreNormalMissingBlock`, `RestoreNormalHashOnlyNoBody` | Normal restart calls `Kura::get_block(...)` for every snapshot height, so hash-journal metadata without a local body cannot satisfy replay parity. |
| `RestoreHardForkMissingHash`, `RestoreHardForkMatchingHash`, `RestoreHardForkHashMismatch` | Hard-fork snapshot bootstrap uses `Kura::block_hash_at_height(...)` instead of decoding legacy block bodies, but still requires every durable hash to exist and match the snapshot. |
| `RestoreInteriorHashMismatch`, `RestoreLatestHashMismatch` | Interior hash divergence returns `TryReadError::MismatchedHash`; the latest-height mismatch is handled only by reverting the latest snapshot block changes before accepting state. |
| `RestoreLegacyManifestReplay`, `RestoreLegacyManifestEmptyNoop`, `RestoreManifestReplayFailure` | Legacy snapshots missing the durable Space Directory manifest section replay manifests from Kura for nonempty histories, no-op on empty histories, and reject if replay cannot recover the section. |
| `WriteZeroHeightAllowed`, `WriteStateAhead`, `WriteLatestHashMismatch`, `WriteAlignedPublishes` | `ensure_state_is_backed_by_kura(...)` allows empty state, rejects state height/hash divergence from Kura, and `try_write_snapshot(...)` publishes snapshot bytes plus digest/signature/Merkle files through temporary files. |
| `CanonicalCommitQcIgnored`, `CanonicalConsensusEvidenceIgnored`, `CanonicalVrfEpochIgnored`, `CanonicalTopologyIgnored` | `redact_consensus_sidecars_from_state_value(...)` removes commit-QC, consensus evidence, VRF epoch, and topology sidecars so replay checkpoints depend on block-applied WSV data rather than later recovery cache timing. |
| `CanonicalMvCurrentOnly`, `CanonicalSortsKeyPolicy`, `CanonicalKeepsWsvMutation` | `normalize_mv_cell_fields_in_state_value(...)` serializes current MV-cell values, `normalize_set_like_parameter_fields_in_state_value(...)` sorts/deduplicates Sumeragi key-policy sets, and committed ledger WSV changes remain part of the canonical replay checkpoint. |

The post-commit cleanup model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `CommittedRbcUndeliveredDaRetained`, `CommittedRbcSettledDrained`, `CommittedRbcInvalidDrained`, `CommittedRbcNoDaDrained` | `should_retain_rbc_sessions_after_commit(...)` and `clean_rbc_sessions_for_committed_block_if_settled(...)` retain only non-invalid undelivered DA-backed committed sessions and otherwise drain runtime state while preserving retained summaries. |
| `DescendantExtendsTipKept`, `DescendantDivergesRequeued`, `DescendantUnknownParentRequeued` | `prune_descendants_not_on_tip(...)` keeps only pending blocks whose parent chain extends the committed hash and drops/requeues divergent or unknown-parent descendants. |
| `CommittedDuplicateDroppedNoRequeue`, `CommittedKuraDuplicateDroppedNoRequeue` | `pending_block_already_committed(...)` and `drop_committed_pending_block_without_requeue(...)` remove already committed pending blocks without requeueing transactions. |
| `StalePendingAtOrBelowDropped`, `StalePendingValidationCleared`, `StalePendingRbcCleaned` | Commit cleanup removes stale pending blocks at or below the committed height, clears validation ownership, and cleans block-scoped RBC state. |
| `QcCacheKeepsCommittedHash`, `QcCacheDropsStaleConflict` | Post-commit QC pruning keeps the committed hash at the committed height while dropping stale, conflicting, or unknown-ancestry QC/signature cache entries. |
| `ProposalHintsDropButSeenKept`, `ProposalCachePruneCommitted` | `prune_descendants_not_on_tip(...)` and `proposal_cache.prune_height_leq(...)` remove stale hints/proposals while preserving `proposals_seen` where duplicate suppression must survive. |
| `MissingCommittedPayloadCleared`, `MissingStaleObsoleteCleared`, `MissingUncommittedPayloadClearDenied`, `MissingObsoleteClearAllowedWithoutPayload` | `clear_missing_block_request(...)` requires local payload knowledge for `PayloadAvailable` clears, allows obsolete clears, removes pending fetch state, records recovery success, and clears sidecar mismatch on payload recovery. |
| `VoteCacheDropsCommittedHeight`, `VoteCachePreservesLocalActive`, `VoteCachePreservesActivePending`, `VoteCachePreservesNewViewWindow`, `VoteCacheDropsAncientNewView` | `prune_vote_caches_horizon(...)` prunes committed-height vote evidence while retaining local active votes, active pending-block votes, and bounded active NEW_VIEW windows. |
| `SlotTrackerPrunesCommitted`, `ForcedViewPrunedAtCommitted`, `OnCommitClearsRecoveryForHeight` | `on_block_commit(...)` prunes slot tracker state, clears forced-view markers at committed heights, clears recovery and sidecar mismatch for the committed height, and refreshes frontier cleanup state. |
| `CommittedEdgeKeepsCanonicalFrontierEvidence`, `CommittedEdgeClearsNoEvidenceFrontier` | Committed-edge conflict cleanup preserves canonical frontier evidence when present and otherwise prunes frontier state, recovery windows, and cooldowns. |
| `ValidationWithoutPendingPruned` | `prune_validation_inflight_without_pending(...)` retains validation ownership only for hashes still present in pending blocks. |

The frontier-gap realignment model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `NoFutureEvidenceNoRequest`, `FutureEvidenceAtFrontierNoRequest`, `FutureEvidenceBeyondMissingPayloadRequests`, `LocalTipPayloadSuppresses` | `maybe_request_frontier_gap_realign_after_commit(...)` requires future recovery evidence strictly above the contiguous frontier and skips when `tip_extending_local_payload_known_at_height(...)` already has the next payload. |
| `ExactBodyOwnerSuppressesGenericPull`, `ExactBodyLagExpiredRetriesExactRepair`, `DeepCatchupBypassesExactOwnerSuppress` | `request_range_pull_from_anchor_with_tier(...)` suppresses generic pulls behind active exact frontier body repair, retries exact repair after the lag window, and allows broader range pulls only when `frontier_slot_allows_deep_catchup(...)` admits them. |
| `CanonicalReanchorUsesPrevLatestAnchor`, `NonCanonicalUsesLatestLatestAnchor`, `MissingAnchorSuppresses` | `reason_prefers_prev_committed_anchor(...)` and `range_pull_anchor_hashes(...)` choose previous/latest anchors for canonical frontier reanchors, latest/latest anchors otherwise, and fail closed when no committed anchor exists. |
| `VoteRosterTargetsPreferred`, `CommitTopologyFallbackTargets`, `TrustedPeersFallbackTargets` | `range_pull_targets_for_height(...)` tries live voting roster targets first, then commit topology, then trusted peers. |
| `LocalPeerRemovedFromTargets`, `TargetsSortedDeduped`, `EmptyTargetsSuppress` | Target selection removes the local peer, sorts by public key/peer id, deduplicates, and suppresses emission when no remote target remains. |
| `PerPeerCooldownSkipsDuplicate`, `SentZeroSuppress`, `SuccessfulPullRecordsPermits` | Per-peer range-pull cooldowns prevent duplicate sends; a zero-send pass returns false; successful sends record cooldowns, direct response permits, and `GetBlocksAfter` requests. |
| `SuccessfulPullMarksCanonicalWindow`, `AlreadyEmittedWindowSuppresses`, `CanonicalWindowRecordsDependencyWatermark` | Canonical frontier reanchor window state records emitted windows and dependency-progress watermarks so the same shared window cannot emit repeatedly without new progress. |
| `CanonicalStrideSuppressesNonAligned`, `CanonicalStrideAlignedEmits`, `EveryThirdWindowAllPeers`, `OtherWindowTwoPeerCohort` | Shared-window stride pacing suppresses non-aligned windows, emits on aligned windows, sends two-peer cohorts in ordinary windows, and fans out to all peers every third emission window. |
| `RecoveryFsmSuppressesWindow`, `MissingQcStallMarksWindow` | `step_recovery_fsm(...)` can suppress already-accounted reanchor windows, and missing-QC stall mode records the emitted reacquire window. |
| `HighPriorityForCanonicalNextHeight`, `LockLagFarFutureExtendsCooldown`, `RangePullMetricIncrement` | `recovery_range_pull_priority(...)`, lock-lag cooldown floors, and range-pull metrics preserve pacing and prioritization for canonical next-height recovery. |

The precommit vote-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `invalidValidation` | `emit_precommit_vote(...)` rejects any pending block whose `ValidationStatus` is not `Valid`; bridge coverage includes `emit_precommit_vote_requires_validated_pending`. |
| `observer`, `notInTopology` | Observer and topology-membership guards in `emit_precommit_vote(...)` prevent non-voting peers from signing local precommits. |
| `duplicateSameSlot` | `local_same_slot_vote(...)` prevents duplicate local precommit votes for the same height/view/epoch. |
| `unsupersededConflict`, `supersededConflict` | Same-height local vote history is enforced by `local_conflicting_slot_vote(...)`, `new_view_qc_supersedes_same_height_vote_conflict(...)`, and stale-vote rotation checks; bridge coverage includes `precommit_vote_rejects_newer_view_after_conflict` and the NEW_VIEW retry regressions in `main_loop/tests.rs`. |
| `candidateCompletesNewerQuorum`, `olderConflictCompletesQuorum` | `candidate_commit_quorum_completes_with_local_vote(...)` can unblock only newer conflicting candidates that complete quorum; bridge coverage includes `precommit_vote_allows_newer_conflict_when_local_vote_completes_quorum` and `precommit_vote_rejects_older_conflict_even_when_local_vote_would_complete_quorum`. |
| `lockedSameHeightConflict`, `missingLockedPayloadOldView`, `missingLockedPayloadNewerView`, `nonExtendingLockedChain`, `extendsLockedChain` | Locked-QC checks in `emit_precommit_vote(...)` and `qc_satisfies_locked_with_lookup(...)` reject same-height locked conflicts, require missing locked payload recovery at the same/older view, allow newer-view override, and require chain extension; bridge coverage includes `precommit_vote_skips_when_block_conflicts_with_locked_chain`, `emit_precommit_vote_requests_missing_locked_payload_before_skipping`, `emit_precommit_vote_allows_newer_view_when_locked_payload_missing`, and `precommit_vote_allows_when_block_extends_locked_chain`. |

The proposal assembly-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `observer`, `notLeader` | Local proposer eligibility guards in `assemble_and_broadcast_proposal(...)`; bridge coverage includes `observer_assemble_proposal_returns_false`. |
| `activeLocalVoteConflict` | Same-height local vote history blocks fresh proposal assembly before proposal cache or slot-observed state is mutated; bridge coverage includes `assemble_proposal_defers_when_candidate_conflicts_with_local_vote_history`. |
| `staleRetiredPriorVote`, `newViewSupersedesLocalVote` | Stale retired vote history and accepted new-view supersession unblock fresh proposals; bridge coverage includes `assemble_proposal_allows_stale_retired_prior_view_local_vote_history` and the raw vote-lock supersession regressions in `main_loop/tests.rs`. |
| `pendingVoteVerification` | Pending same-height vote verification defers proposal assembly until the conflict surface is known; bridge coverage includes `assemble_proposal_defers_while_same_height_vote_verification_is_pending`. |
| `missingHighestQc` | Missing highest-QC payloads arm exact frontier repair and suppress proposal messages; bridge coverage includes `assemble_proposal_defers_when_highest_qc_block_missing`. |
| `regressedHighestReplacedByLock`, `lockedChainExtends`, `nonExtendingHighestQc` | Highest-QC and locked-chain compatibility in `highest_qc_extends_locked(...)`, locked fallback, and lock-lag range-pull recovery; bridge coverage includes `pacemaker_uses_locked_qc_when_selected_highest_qc_regresses`, `assemble_proposal_reanchors_lock_lag_highest_qc_catchup`, and `highest_qc_extends_locked_rejects_missing_highest`. |
| `splitSameHeightVotesNonViable` | Split same-height vote locks make the fresh branch non-viable and force recovery/defer instead of proposal assembly; bridge coverage includes `fresh_proposal_defers_when_split_same_height_votes_make_new_branch_non_viable`. |
| `committedEdgeHighestConflict` | Highest-QC evidence conflicting with the committed edge is suppressed instead of producing a fresh proposal; bridge coverage includes `assemble_proposal_suppresses_committed_edge_highest_qc_conflict`. |

The engine tick gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `noHighestIdle` | `ConsensusEngine::on_tick(...)` advances view, signs a `NewView` vote with `zero_subject()`, and emits `AdvanceView` when no highest QC exists. Bridge coverage includes `prepare_and_commit_qcs_from_previous_view_are_ignored_after_timeout`. |
| `highestIdle` | `on_tick(...)` uses `qc_subject(highest_qc)` and carries `highest_qc` when local highest-QC state exists. Bridge coverage includes `pending_finality_survives_timeout_and_view_change_noise`. |
| `validationNoHighest`, `validationWithHighest` | Ticks clear the pure engine's `validating` owner so late validation callbacks cannot force an extra view change. Bridge coverage includes `timeout_clears_inflight_validation_before_late_failure_arrives` and `tick_binds_highest_qc_and_clears_inflight_validation`. |
| `pendingFinalityWithHighest` | Ticks leave `pending_finality` intact across view changes while still binding highest-QC evidence into the `NewView` vote. Bridge coverage includes `pending_finality_survives_timeout_and_view_change_noise`. |

The engine NewView subject projection model is intentionally finite. These
are the implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `tick_no_highest` | `ConsensusEngine::on_tick(...)` calls `zero_subject()` when `state.highest_qc` is absent and emits a NewView vote with no highest-QC binding. Bridge coverage includes `prepare_and_commit_qcs_from_previous_view_are_ignored_after_timeout`. |
| `tick_prepare_highest`, `tick_commit_highest`, `tick_new_view_highest` | `on_tick(...)` maps the current highest QC through `qc_subject(...)`, using the QC subject hash as both parent and block with zero payload, and binds the same highest-QC reference. Bridge coverage includes `tick_binds_highest_qc_and_clears_inflight_validation` and `pending_finality_survives_timeout_and_view_change_noise`. |
| `invalid_no_highest` | The invalid branch of `on_validation_result(...)` falls back to the rejected proposal block hash as both parent and block, with zero payload and no highest-QC binding. Bridge coverage includes `invalid_validation_result_for_current_proposal_advances_view_once`. |
| `invalid_prepare_highest`, `invalid_commit_highest`, `invalid_new_view_highest` | Invalid validation with a current highest QC signs the QC-derived subject instead of the rejected block hash and binds the same highest-QC reference. Bridge coverage includes `invalid_validation_new_view_vote_uses_highest_qc_subject`. |

The engine certificate dispatch model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `current_prepare`, `current_commit` | `ConsensusEngine::on_certificate(...)` dispatches current-context Prepare and Commit certificates only to `on_prepare_qc(...)` and `on_commit_qc(...)`, respectively. Bridge coverage includes `locked_qc_blocks_unsafe_prepare_votes` and `commit_qc_waits_for_payload_before_finality`. |
| `new_view_lower_view`, `new_view_same_view`, `new_view_future_view` | NewView certificates with matching height, epoch, validator set, and quorum policy pass the shared prefilter regardless of view; `on_new_view_qc(...)` owns the strict newer-view decision. Bridge coverage includes `stale_new_view_certificate_cannot_update_highest_qc_or_rewind_round`. |
| `committed_prepare`, `committed_commit`, `committed_new_view` | `committed.contains_key(...)` rejects all certificate phases for finalized heights before dispatch. Bridge coverage includes `prepare_qc_for_committed_height_is_ignored` and `committed_commit_qc_replay_does_not_emit_duplicate_finality`. |
| `wrong_height_*`, `wrong_epoch_*`, `wrong_validator_set_*`, `wrong_quorum_*` | The shared prefilter rejects mismatched round context and quorum policy for every phase. Bridge coverage includes `prepare_qcs_with_wrong_round_context_are_ignored`, `commit_qcs_with_wrong_round_context_are_ignored`, `new_view_certificates_with_wrong_epoch_or_validator_set_are_ignored`, and `certificates_with_wrong_view_or_quorum_policy_are_ignored`. |
| `stale_prepare`, `stale_commit` | Prepare and Commit certificates must match the current view before phase-handler dispatch. Bridge coverage includes `prepare_and_commit_qcs_from_previous_view_are_ignored_after_timeout`. |

The engine view-advance saturation model is intentionally finite. These are
the implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `tick_mid`, `tick_max` | `ConsensusEngine::on_tick(...)` computes the next round with `round.view.saturating_add(1)`, then emits `SignVote { phase: NewView, round: next, ... }` and `AdvanceView { round: next }`. |
| `invalid_mid`, `invalid_max` | `ConsensusEngine::on_validation_result(...)` uses the same saturated increment on the invalid current-validation branch before emitting the NewView vote and `AdvanceView`. |
| `valid_current` | Valid current validation callbacks clear validation ownership but do not advance view or emit consensus outputs. Bridge coverage includes `invalid_validation_result_for_current_proposal_advances_view_once` for the invalid-only branch distinction. |
| `wrong_round_invalid`, `wrong_block_invalid`, `no_inflight_invalid` | Validation callbacks that do not match the current in-flight owner are ignored before any view advance. Bridge coverage includes `validation_results_for_unknown_or_completed_proposals_do_not_force_view_change`. |

The engine NewView-QC gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `safeNoHighest` | `ConsensusEngine::on_new_view_qc(...)` accepts a compatible newer-view certificate without carried highest-QC evidence and emits `AdvanceView`. Bridge coverage includes `stale_new_view_certificate_cannot_update_highest_qc_or_rewind_round` for the accepted advance before the stale replay. |
| `safeImprovingHighest`, `pendingSafeImprovingHighest` | Accepted NewView QCs call `record_highest_qc(...)` when the carried compatible QC improves local state, including while pending finality survives view-change noise. Bridge coverage includes `new_view_certificate_rejects_incompatible_highest_qc` and `pending_finality_survives_timeout_and_view_change_noise`. |
| `safeLowerHighest` | Accepted newer-view certificates with lower carried QC evidence must not regress `highest_qc`. Bridge coverage includes `accepted_new_view_certificate_cannot_downgrade_highest_qc`. |
| `validationSafeNoHighest` | Accepted NewView QCs clear any in-flight proposal validation before late callbacks can mutate the view. Bridge coverage includes `tick_binds_highest_qc_and_clears_inflight_validation` and `invalid_validation_new_view_vote_uses_highest_qc_subject`. |
| `wrongHeight`, `wrongEpoch`, `wrongValidatorSet` | `on_certificate(...)` rejects NewView certificates whose height, epoch, or validator set does not match the engine round before phase-specific handling. Bridge coverage includes `new_view_certificates_with_wrong_epoch_or_validator_set_are_ignored`. |
| `wrongQuorumPolicy` | `on_certificate(...)` rejects certificates whose quorum policy differs from the engine policy before `on_new_view_qc(...)` can mutate state. Bridge coverage is shared with `certificates_with_wrong_view_or_quorum_policy_are_ignored`. |
| `sameView`, `lowerView` | `on_new_view_qc(...)` requires the certificate view to be strictly greater than the current view. Bridge coverage includes `stale_new_view_certificate_cannot_update_highest_qc_or_rewind_round`. |
| `futureHeightHighest`, `futureViewHighest`, `wrongEpochHighest` | `qc_ref_is_compatible_with_round(...)` rejects carried highest-QC evidence from a future height/view or wrong epoch. Bridge coverage includes `new_view_certificate_rejects_incompatible_highest_qc`. |

The engine exact NewView-QC advance model is intentionally finite. These are
the implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `safe_no_highest`, `safe_improving_highest`, `safe_lower_highest`, `validation_safe`, `pending_safe` | `ConsensusEngine::on_new_view_qc(...)` assigns `self.state.round = certificate.round`, returns to proposal phase, clears validation, preserves pending finality, and emits `AdvanceView { round: certificate.round }`. Bridge coverage includes `stale_new_view_certificate_cannot_update_highest_qc_or_rewind_round` and `pending_finality_survives_timeout_and_view_change_noise`. |
| `wrong_height`, `wrong_epoch`, `wrong_validator_set`, `wrong_quorum_policy` | Shared prefilter rejections cannot update the stored round or emit an advance output. Bridge coverage includes `new_view_certificates_with_wrong_epoch_or_validator_set_are_ignored` and `certificates_with_wrong_view_or_quorum_policy_are_ignored`. |
| `same_view`, `lower_view` | NewView certificates that do not strictly advance the view are ignored by `on_new_view_qc(...)`. Bridge coverage includes `stale_new_view_certificate_cannot_update_highest_qc_or_rewind_round`. |
| `future_height_highest`, `future_view_highest`, `wrong_epoch_highest` | Incompatible carried highest-QC evidence rejects the NewView certificate before any round/output mutation. Bridge coverage includes `new_view_certificate_rejects_incompatible_highest_qc`. |

The engine proposal-ingress gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `safeUnlocked` | `ConsensusEngine::on_proposal(...)` accepts a current-round proposal without a lock and emits both `ValidateBlock` and a prepare `SignVote`. Bridge coverage includes `proposals_are_ignored_outside_proposal_phase` for the accepted first proposal. |
| `safeLockedSubject` | `proposal_satisfies_lock(...)` accepts a proposal whose block hash matches the current locked QC subject. Bridge coverage is shared with `locked_qc_blocks_unsafe_prepare_votes`. |
| `safeConflictHigherQc` | A conflicting proposal can unlock only with a strictly higher compatible QC. Bridge coverage includes `conflicting_proposal_requires_strictly_higher_qc_to_unlock`. |
| `wrongPhase` | `on_proposal(...)` ignores proposals outside `EnginePhase::Proposal`. Bridge coverage includes `proposals_are_ignored_outside_proposal_phase`. |
| `wrongHeight`, `wrongEpoch`, `wrongValidatorSet`, `wrongView` | `on_proposal(...)` requires exact round equality before requesting validation or signing prepare. Bridge coverage includes `proposals_with_wrong_round_context_are_ignored`. |
| `futureHeightHighest`, `futureViewHighest`, `wrongEpochHighest` | `qc_ref_is_compatible_with_round(...)` rejects proposal highest-QC evidence from a future height/view or wrong epoch. Bridge coverage includes `proposal_with_incompatible_highest_qc_cannot_unlock_conflicting_lock`. |
| `lockedConflictNoQc`, `lockedConflictEqualQc`, `lockedConflictLowerQc` | `proposal_satisfies_lock(...)` rejects conflicting locked proposals unless the proposal carries a strictly greater compatible QC. Bridge coverage includes `locked_qc_blocks_unsafe_prepare_votes` and `conflicting_proposal_requires_strictly_higher_qc_to_unlock`. |

The engine proposal-lock helper model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `unlocked_no_qc`, `unlocked_with_qc` | `proposal_satisfies_lock(...)` returns true when `state.locked_qc` is absent, regardless of carried highest-QC evidence. |
| `locked_subject_no_qc`, `locked_subject_lower_qc` | A proposal for the locked subject returns true before highest-QC comparison. |
| `conflict_no_qc` | A conflicting subject with no carried highest QC cannot unlock the local lock. |
| `conflict_equal_qc` | Equal QC rank is rejected because the implementation requires `qc_ref_cmp(...).is_gt()`, not greater-than-or-equal. |
| `conflict_lower_height_qc`, `conflict_lower_view_qc` | Lower QC evidence cannot unlock a conflicting proposal. |
| `conflict_higher_height_qc`, `conflict_higher_view_qc`, `conflict_higher_phase_qc`, `conflict_higher_subject_qc` | Any strictly greater QC under `qc_ref_cmp(...)` can unlock the conflicting proposal after the proposal highest-QC compatibility gate has already accepted it. |

The QC-round compatibility helper model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `lowerHeightLowerView`, `lowerHeightEqualView`, `lowerHeightHigherView` | `qc_ref_is_compatible_with_round(...)` accepts same-epoch QCs from lower heights without comparing views. Bridge coverage is shared by `new_view_certificate_rejects_incompatible_highest_qc` and `proposal_with_incompatible_highest_qc_cannot_unlock_conflicting_lock`, which exercise the helper through NewView and proposal admission. |
| `sameHeightPastView`, `sameHeightEqualView` | Same-height QCs are accepted when their view is less than or equal to the candidate round view. |
| `sameHeightFutureView` | Same-height future-view highest-QC evidence is rejected before it can advance NewView context or unlock a proposal. |
| `futureHeightLowerView`, `futureHeightEqualView`, `futureHeightHigherView` | Future-height QCs are rejected even when their view is not ahead of the candidate round view. |
| `wrongEpochLowerHeight`, `wrongEpochSameHeightPastView`, `wrongEpochSameHeightEqualView`, `wrongEpochFutureHeight` | Wrong-epoch QCs are rejected before height/view ordering is considered. |

The engine QC reference projection helper model is intentionally finite. These
are the implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `prepare_base`, `commit_base`, `new_view_base` | `qc_ref_from_certificate(...)` copies the certificate phase into the projected `QcRef` for Prepare, Commit, and NewView certificates. |
| `height_two` | The projected QC height is exactly `certificate.round.height`; it is not advanced or reset during projection. |
| `view_three` | The projected QC view is exactly `certificate.round.view`. |
| `epoch_four` | The projected QC epoch is exactly `certificate.round.epoch`. |
| `subject_b` | The projected QC subject is `certificate.subject.block_hash`; parent hash and synthesized zero-hash substitutions are rejected. |

The engine highest-QC record helper model is intentionally finite. These are
the implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `noCurrent` | `record_highest_qc(...)` records the first candidate when `state.highest_qc` is absent. |
| `higherHeightLowerView`, `lowerHeightHigherView` | `qc_ref_cmp(...)` compares height before view, so a higher-height lower-view QC updates while a lower-height higher-view QC cannot regress state. |
| `sameHeightHigherView`, `sameHeightLowerView` | Same-height candidates use view as the next decisive comparator. |
| `sameSlotCommitOverPrepare`, `sameSlotPrepareUnderCommit` | `phase_rank(...)` orders Prepare, NewView, Commit, so same-slot Commit evidence can improve state while same-slot Prepare evidence cannot replace Commit evidence. |
| `sameSlotSubjectHigh`, `sameSlotSubjectLow` | Subject hash bytes provide the deterministic final tie-break for same-height/view/phase QCs. |
| `equalQc` | `record_highest_qc(...)` uses a strict `is_gt()` check, so equal candidates do not count as state updates. |

The engine commit-subject helper model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `fresh_clean`, `fresh_pending_finality`, `fresh_validating` | `commit_subject(...)` records the subject at the current height, clears pending finality and validation ownership, returns to proposal phase, and emits one `CommitBlock`. |
| `matching_committed` | A direct helper call with an already committed matching hash is idempotent for the committed record but still follows the helper's success branch and emits the commit output. |
| `conflict_clean`, `conflict_pending_finality`, `conflict_validating` | A conflicting committed hash at the current height returns an empty output list without mutating committed state, pending finality, validation ownership, or phase. |
| `BugClearPendingOnConflict`, `BugClearValidationOnConflict` | Conflict handling must be side-effect free even for cleanup-looking state changes; the helper returns before clearing pending finality or validation ownership. |

The engine payload lookup helper model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `exact_pair` | `has_payload(...)` checks `available_payloads.contains(&(subject.block_hash, subject.payload_hash))`, so an exact recorded pair returns true. |
| `same_block_wrong_payload` | A recorded payload for the same block hash but a different payload hash cannot satisfy the lookup. |
| `wrong_block_same_payload` | A recorded payload hash for a different block cannot satisfy the lookup. |
| `wrong_block_wrong_payload` | Unrelated availability entries cannot make another subject appear locally available. |
| `empty_store` | An empty availability set returns false, so commit-QC handling must request payload recovery instead of finalizing. |

The engine prepare-QC gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `safePrepareQc` | `ConsensusEngine::on_certificate(...)` dispatches a current-context `CertPhase::Prepare` certificate into `on_prepare_qc(...)`, which signs one commit vote. Bridge coverage includes `locked_qc_blocks_unsafe_prepare_votes`. |
| `wrongHeight`, `wrongEpoch`, `wrongValidatorSet` | `on_certificate(...)` rejects certificates whose round context does not match the engine round before phase-specific handling. Bridge coverage includes `prepare_qcs_with_wrong_round_context_are_ignored`. |
| `wrongQuorumPolicy` | `on_certificate(...)` rejects certificates whose quorum policy differs from the engine policy. Bridge coverage includes `certificates_with_wrong_view_or_quorum_policy_are_ignored`. |
| `staleView` | Prepare/commit certificates must match the current view after timeout/view advance. Bridge coverage includes `prepare_and_commit_qcs_from_previous_view_are_ignored_after_timeout`. |
| `committedHeight` | `committed.contains_key(...)` blocks certificate handling for finalized heights. Bridge coverage includes `prepare_qc_for_committed_height_is_ignored`. |
| `replaySamePrepareQc`, `conflictingPrepareQc` | The per-round `commit_votes` map suppresses duplicate and conflicting prepare-QC handling after the first commit-vote output. Bridge coverage includes `prepare_qc_replays_and_conflicts_do_not_emit_extra_commit_votes`. |
| `pendingFinality` | `on_prepare_qc(...)` suppresses commit-vote output while a commit QC is waiting for exact payload recovery. Bridge coverage includes `prepare_qc_during_pending_finality_does_not_emit_commit_vote`. |
| `locked`, `highest` | Accepted prepare QCs record the prepare QC as both `locked_qc` and `highest_qc`; bridge coverage includes `locked_qc_blocks_unsafe_prepare_votes` and the replay/conflict test above. |

The engine prepare-vote cache/output model is intentionally finite. These are
the implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `safe_prepare` | `ConsensusEngine::on_prepare_qc(...)` inserts the certificate subject into `commit_votes` under the certificate round and emits one commit `SignVote` with no highest-QC reference. Bridge coverage includes `prepare_qc_replays_and_conflicts_do_not_emit_extra_commit_votes`. |
| `wrong_height`, `wrong_epoch`, `wrong_validator_set`, `wrong_quorum_policy`, `stale_view`, `committed_height` | The shared certificate prefilter rejects unsafe Prepare QCs before they can populate the commit-vote cache or emit a vote. Bridge coverage includes `prepare_qcs_with_wrong_round_context_are_ignored`, `prepare_qc_for_committed_height_is_ignored`, and `certificates_with_wrong_view_or_quorum_policy_are_ignored`. |
| `pending_finality` | `on_prepare_qc(...)` returns before inserting into `commit_votes` or signing while pending finality owns the round. Bridge coverage includes `prepare_qc_during_pending_finality_does_not_emit_commit_vote`. |
| `replay_same_prepare`, `conflicting_prepare` | Existing same-round `commit_votes` entries are preserved and suppress another commit vote, including conflicting subjects. Bridge coverage includes `prepare_qc_replays_and_conflicts_do_not_emit_extra_commit_votes`. |

The engine commit-QC gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `safePayloadAvailable` | `on_commit_qc(...)` commits immediately when `has_payload(...)` is true. Bridge coverage includes `payload_availability_without_commit_qc_never_finalizes`. |
| `safePayloadMissing` | `on_commit_qc(...)` records pending finality and emits `FetchPayload` when the payload is missing. Bridge coverage includes `commit_qc_waits_for_payload_before_finality`. |
| `wrongHeight`, `wrongEpoch`, `wrongValidatorSet` | `on_certificate(...)` rejects certificates whose round context does not match the engine round before commit-QC handling. Bridge coverage includes `commit_qcs_with_wrong_round_context_are_ignored`. |
| `wrongQuorumPolicy` | `on_certificate(...)` rejects certificates whose quorum policy differs from the engine policy. Bridge coverage includes `certificates_with_wrong_view_or_quorum_policy_are_ignored`. |
| `staleView` | Prepare/commit certificates must match the current view after timeout/view advance. Bridge coverage includes `prepare_and_commit_qcs_from_previous_view_are_ignored_after_timeout`. |
| `committedHeight` | `committed.contains_key(...)` blocks certificate handling for finalized heights. Bridge coverage includes `committed_commit_qc_replay_does_not_emit_duplicate_finality` and `conflicting_blocks_cannot_both_commit_at_same_height`. |
| `pendingReplaySameCommitQc`, `pendingConflictingCommitQc` | `pending_finality` suppresses duplicate fetches and conflicting pending-finality subjects until exact payload recovery resolves the current QC. Bridge coverage includes `pending_commit_qc_replays_and_conflicts_do_not_refetch_payload`. |
| `highest` | Accepted commit QCs update `highest_qc` whether they finalize immediately or request payload recovery. Bridge coverage includes `payload_availability_without_commit_qc_never_finalizes` and `commit_qc_waits_for_payload_before_finality`. |

The engine payload-available Commit-QC exact finality model is intentionally
finite. These are the implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `safe_payload_available` | The payload-available branch in `on_commit_qc(...)` calls `commit_subject(certificate.subject)`, records `certificate.subject.block_hash` at the current height, clears validation ownership, returns to proposal phase, emits `CommitBlock { subject: certificate.subject }`, and does not request payload recovery. Bridge coverage includes `payload_availability_without_commit_qc_never_finalizes`. |
| `wrong_height`, `wrong_epoch`, `wrong_validator_set`, `wrong_quorum_policy`, `stale_view` | Shared certificate prefilter rejections return before finality output or committed-height mutation. Bridge coverage includes `commit_qcs_with_wrong_round_context_are_ignored` and `certificates_with_wrong_view_or_quorum_policy_are_ignored`. |
| `committed_height` | `committed.contains_key(...)` rejects a Commit QC for an already finalized height before it can overwrite the recorded block. Bridge coverage includes `committed_commit_qc_replay_does_not_emit_duplicate_finality` and `conflicting_blocks_cannot_both_commit_at_same_height`. |
| `pending_replay`, `pending_conflict` | Commit QCs that reach `on_commit_qc(...)` while pending finality exists clear validation ownership, preserve the existing pending subject/map entry, and emit no finality. Bridge coverage includes `pending_commit_qc_replays_and_conflicts_do_not_refetch_payload` and `pending_finality_rejects_payload_hash_and_subject_replays_without_dropping_qc`. |

The engine Commit-QC pending/fetch model is intentionally finite. These are
the implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `safe_missing_payload` | The missing-payload branch in `on_commit_qc(...)` sets `state.pending_finality = Some(certificate.subject)`, inserts `certificate.clone()` into `pending_finality` keyed by `certificate.subject.block_hash`, and emits `FetchPayload { round: certificate.round, block_hash: certificate.subject.block_hash, payload_hash: certificate.subject.payload_hash }`. Bridge coverage includes `commit_qc_waits_for_payload_before_finality`. |
| `safe_payload_available` | Payload-available Commit QCs call `commit_subject(...)` and must not create pending certificate-map entries or fetch requests. Bridge coverage includes `payload_availability_without_commit_qc_never_finalizes`. |
| `wrongHeight`, `wrongEpoch`, `wrongValidatorSet`, `wrongQuorumPolicy`, `staleView`, `committedHeight` | Shared certificate prefilter rejections return before pending/fetch side effects. Bridge coverage includes `commit_qcs_with_wrong_round_context_are_ignored`, `certificates_with_wrong_view_or_quorum_policy_are_ignored`, and `committed_commit_qc_replay_does_not_emit_duplicate_finality`. |
| `pending_replay`, `pending_conflict` | Existing pending finality returns before refetching or replacing the pending map entry. Bridge coverage includes `pending_commit_qc_replays_and_conflicts_do_not_refetch_payload` and `pending_finality_rejects_payload_hash_and_subject_replays_without_dropping_qc`. |

The engine payload-availability gate model is intentionally finite. These are
the implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `noPendingPayload` | `ConsensusEngine::on_payload_available(...)` records local payload availability but emits no finality without a pending commit QC. Bridge coverage includes `payload_availability_without_commit_qc_never_finalizes`. |
| `matchingPendingPayload` | A payload notification matching the pending commit-QC subject removes the pending certificate and calls `commit_subject(...)`. Bridge coverage includes `commit_qc_waits_for_payload_before_finality`. |
| `payloadHashMismatch` | Same block hash with the wrong payload hash is ignored while preserving pending finality. Bridge coverage includes `pending_finality_ignores_payload_hash_mismatch_until_exact_payload_arrives`. |
| `parentMismatch` | Same block hash and payload hash with a different parent is ignored because the full `BlockSubject` must match the pending certificate. Bridge coverage includes `pending_finality_rejects_payload_hash_and_subject_replays_without_dropping_qc`. |
| `unknownBlockHash` | Payload availability for an unrelated block hash cannot satisfy the pending QC and must not drop it. Bridge coverage includes `pending_commit_qc_replays_and_conflicts_do_not_refetch_payload`. |

The engine committed-block gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `freshPlain` | `ConsensusEngine::on_committed_block(...)` records a freshly finalized height without emitting validator-set activation when no reconfiguration is present. Bridge coverage includes `committed_block_notifications_do_not_overwrite_conflicting_height`. |
| `freshBoundaryReconfiguration` | A reconfiguration activates only when its `activation_height` equals the committed block height plus one. Bridge coverage includes `reconfiguration_activates_only_after_old_set_finality`. |
| `freshNonBoundaryReconfiguration` | Non-boundary reconfiguration notifications are recorded but do not activate. Bridge coverage includes `reconfiguration_with_non_boundary_activation_is_not_activated`. |
| `duplicatePlain`, `duplicateBoundaryReconfiguration` | Duplicate same-height notifications are no-ops and cannot re-emit activation. Bridge coverage includes `duplicate_committed_block_notification_does_not_reactivate_reconfiguration`. |
| `conflictingPlain`, `conflictingBoundaryReconfiguration`, `conflictingNonBoundaryReconfiguration` | A committed height is immutable once recorded, so conflicting same-height notifications cannot overwrite state or activate reconfiguration. Bridge coverage includes `conflicting_committed_block_notification_cannot_activate_reconfiguration`. |

The engine reconfiguration-staging model is intentionally finite. These are
the implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `fresh_boundary_a_none`, `fresh_boundary_b_none` | `ConsensusEngine::on_committed_block(...)` sets `pending_reconfiguration = Some(change.clone())` and emits `ActivateValidatorSet(change)` for fresh boundary reconfiguration notifications. Bridge coverage includes `reconfiguration_activates_only_after_old_set_finality`. |
| `fresh_boundary_a_prior_b` | A fresh boundary reconfiguration replaces any previously staged change with the exact committed-block change before activation is emitted. |
| `fresh_plain_*`, `fresh_non_boundary_*` | Plain commits and non-boundary reconfigurations record the block but preserve existing reconfiguration staging and emit no activation. Bridge coverage includes `reconfiguration_with_non_boundary_activation_is_not_activated`. |
| `duplicate_boundary_*`, `conflict_boundary_*`, `conflict_non_boundary_a_prior_b` | Already committed heights return before staging or activation, so duplicate and conflicting notifications preserve existing staged changes. Bridge coverage includes `duplicate_committed_block_notification_does_not_reactivate_reconfiguration` and `conflicting_committed_block_notification_cannot_activate_reconfiguration`. |

The engine committed-block cleanup model is intentionally finite. These are
the implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `fresh_current_clean`, `fresh_current_validating` | `ConsensusEngine::on_committed_block(...)` records fresh current-height finality, clears in-flight validation, and returns to proposal phase. Bridge coverage includes `committed_block_notification_supersedes_late_invalid_validation_result`. |
| `fresh_current_pending_matching`, `fresh_current_pending_conflicting` | Current-height storage finality clears `state.pending_finality` and removes the pending certificate map entry, including when storage finality supersedes a conflicting pending QC subject. Bridge coverage includes `committed_block_notification_clears_matching_pending_finality` and `conflicting_committed_block_notification_clears_pending_finality`. |
| `fresh_other_validating`, `fresh_other_pending` | Other-height notifications record their height but preserve the current round's validation and pending-finality ownership. Bridge coverage includes `committed_block_notification_for_other_height_does_not_clear_pending_finality`. |
| `duplicate_current_validating`, `duplicate_current_pending`, `conflict_current_validating`, `conflict_current_pending` | Already-committed heights return before cleanup or overwrite side effects, so duplicate and conflicting notifications are no-ops. Bridge coverage includes `committed_block_notifications_do_not_overwrite_conflicting_height`. |

The validator-set transition model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `ActivationHeight`, `BoundaryHeight`, `staged`, `activated` | Epoch-boundary validator-set activation in the pure engine's `CommittedBlock { reconfiguration }` handling and the live pending-roster activation path. |
| `committedOld`, `committedNew` | Commit certificates are accepted only for the validator set active at that height; bridge coverage includes `reconfiguration_activates_only_after_old_set_finality` in `crates/iroha_core/src/sumeragi/engine.rs` and pending-roster activation tests in `main_loop/tests.rs`. |
| `committedMixed` | Mixed-set certificates are invalid: certificate signers are interpreted against exactly one validator-set id/hash in `crates/iroha_data_model/src/block/consensus.rs` and Sumeragi QC validation. |

The certified recovery model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `qcsObserved`, `pendingSubject` | Commit-QC observation and pending-finality state in the pure engine, including `ConsensusInput::CommitQc` / pending-finality handling in `crates/iroha_core/src/sumeragi/engine.rs`. |
| `fetchRequested`, `matchingPayloads`, `mismatchedPayloads` | Certified block fetch request/response validation. Responses must match height, view, block hash, commit-QC subject, payload hash, and checkpoint before materializing local payload state. |
| `rejectedMismatches` | Mismatched payload/hash/subject responses are rejected while keeping the pending QC available for a later exact response. Bridge coverage includes `pending_finality_rejects_payload_hash_and_subject_replays_without_dropping_qc`. |
| `committedSubjects` | State application/finality is gated on both the commit QC and exact matching payload; conflicting same-height finality is rejected by the pure engine and live certified-fetch validation path. |

The view-change safety model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `currentView`, `maxAcceptedView` | Accepted new-view certificates move the pure engine round forward and stale certificates are ignored by `on_new_view_qc(...)`. Bridge coverage includes `stale_new_view_certificate_cannot_update_highest_qc_or_rewind_round`. |
| `highestRank`, `acceptedQcRanks` | Highest-QC monotonicity maps to `record_highest_qc(...)` and deterministic `select_highest_qc(...)` ordering in `crates/iroha_core/src/sumeragi/engine.rs`. |
| `lockedBranch`, `lockRank` | Locked-QC state maps to `locked_qc` and `proposal_satisfies_lock(...)`; bridge coverage includes `locked_qc_blocks_unsafe_prepare_votes`. |
| `unsafeLockOverwrite` | Conflicting prepare-QC replay/overwrite rejection maps to the pure engine's per-round `commit_votes` guard; bridge coverage includes `prepare_qc_replays_and_conflicts_do_not_emit_extra_commit_votes`. |

The validation-gate model is intentionally finite. These are the implementation
surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `validating`, `validationView` | The pure engine's `validating: Option<BlockSubject>` and current `RoundId` checks in `on_validation_result(...)`. |
| `UnknownValidationFailure` | Rejection of validation callbacks whose block hash does not match the in-flight proposal; bridge coverage includes `validation_results_for_unknown_or_completed_proposals_do_not_force_view_change`. |
| `CompletedValidationReplay` | Replayed success/failure callbacks after a proposal's validation state is consumed; the same bridge test proves these do not force a view change. |
| `TimeoutClearsOrRetainsInflight`, `LateValidationAfterTimeout` | Timeout clears the in-flight validation before a late failure arrives; bridge coverage includes `timeout_clears_inflight_validation_before_late_failure_arrives`. |
| `CurrentValidationFails`, `invalidAdvanceSubjects` | An invalid current proposal advances the view once and clears ownership so replayed failures are ignored; bridge coverage includes `invalid_validation_result_for_current_proposal_advances_view_once`. |

The pure engine validation-result model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `validCurrent`, `invalidNoHighest`, `invalidWithHighest` | The pure engine's `on_validation_result(...)` accepts only a result for the current `RoundId` and exact `validating.block_hash`. Bridge coverage includes `validation_results_for_unknown_or_completed_proposals_do_not_force_view_change` and `invalid_validation_result_for_current_proposal_advances_view_once`. |
| `validCurrent` | A successful validation callback clears the in-flight owner and emits no consensus outputs while the engine stays in prepare phase. |
| `invalidNoHighest`, `invalidWithHighest` | A failed current validation clears ownership, emits `NewView` plus `AdvanceView`, and advances to proposal phase. Bridge coverage includes `invalid_validation_new_view_vote_uses_highest_qc_subject`. |
| `supersededByCommit`, `supersededByCommittedBlock` | Commit QCs and committed-block notifications clear stale validation ownership before late invalid callbacks can mutate pending finality or committed state. Bridge coverage includes `commit_qc_supersedes_late_invalid_validation_result`, `conflicting_commit_qc_supersedes_late_invalid_validation_result`, and `committed_block_notification_supersedes_late_invalid_validation_result`. |

The certificate-admission model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `WrongContextCertificate` | `on_certificate(...)` rejects certificates whose height, epoch, validator-set id, or quorum policy do not match the local consensus context. Bridge coverage includes `certificates_with_wrong_view_or_quorum_policy_are_ignored` and `new_view_certificates_with_wrong_epoch_or_validator_set_are_ignored`. |
| `StalePrepareCommitCertificate` | Prepare/commit certificates must match the current view after timeout/view advance. Bridge coverage includes `prepare_and_commit_qcs_from_previous_view_are_ignored_after_timeout`. |
| `FutureHeightCertificate` | Certificate height must match the current height before it can mutate phase, lock, or pending-finality state. Bridge coverage includes `future_round_certificates_do_not_move_local_phase`. |
| `CommittedHeightCertificate` | Already committed heights are immutable through `committed.contains_key(...)` admission checks and `on_committed_block(...)` conflict guards. Bridge coverage includes `conflicting_blocks_cannot_both_commit_at_same_height` and `committed_block_notifications_do_not_overwrite_conflicting_height`. |

The highest-QC selection model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `ObservedNewViewQcs` | `select_highest_qc(...)` filters to `CertPhase::NewView` certificates before considering embedded `highest_qc` values. |
| `SpecGreater` | `qc_ref_cmp(...)` orders QCs by height, view, phase rank, and subject hash bytes. Bridge coverage includes `new_view_certificate_selects_highest_qc_deterministically`. |
| `EqualObservedSelectsEqualQc` | Deterministic aggregation: the selected highest QC is independent of certificate arrival/order. The bridge test checks both input orders for height, phase-rank, and subject tie-break cases. |
| `SelectedOnlyFromNewViewCertificates` | Prepare/commit certificates do not contribute highest-QC evidence to a new-view aggregation; the bridge test asserts a prepare/commit-only input selects no QC. |

The frontier model is intentionally finite. These are the implementation
surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `frontierSlot`, `pending`, `contiguous`, `payloadState` | `PendingBlock` handling and local payload checks in `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`, plus BlockCreated/frontier ownership materialization in `proposal_handlers.rs`. |
| `commitVotes`, `queuedVotes` | Commit-vote counting and vote ingress gating exercised by `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` and `reschedule_ignores_quorum_timeout_vote_queue_backlog` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`. |
| `recoveryOwner` | Active/stale frontier owner state in `frontier_slot_has_active_owner_state_for_view(...)`, stale-owner yield in `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)`, and supersede cleanup in `drop_superseded_contiguous_frontier_owner_state(...)`. |
| `quorumRescheduleArmed`, `quorumWindowAge` | Vote-backed quorum reschedule pacing in `reschedule_stale_pending_blocks_with_now(...)`; regression coverage includes `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned`. |
| `payloadRecovered` | Exact frontier body repair and stale RBC repair admission in `request_frontier_owner_body_repair(...)`, `handle_frontier_body_gap_with_topology(...)`, and `stale_frontier_rbc_repair_is_actionable(...)`. |
| `quorumRetransmitted`, `rotated` | Quorum retransmit target selection, `rebroadcast_pending_block_updates(...)`, and deterministic view-change calls in `reschedule_stale_pending_blocks_with_now(...)`. |
| `subjectView`, `progressAge`, `lastProgressKind`, `validationState`, `localVoteEmitted`, `commitQcObserved` | Pending-block progress age/touch accounting. Validation, local commit-vote emission, and commit-QC observation map to `PendingBlock::touch_progress(...)`, `PendingBlock::note_local_commit_vote_emitted(...)`, and `PendingBlock::note_commit_qc_observed(...)`. |
| `recoveryLastRotationView`, `staleRecoveryUnlocked` | Same-height stale frontier recovery unlocks must be scoped by the vote/view that actually rotated. This maps to `stale_same_height_recovery_age(...)` and the stale-owner quorum-timeout guards in the Sumeragi main loop. |
| `futurePresent`, `futureContiguous`, `futureCommitVotes`, `futureQueuedVotes`, `futurePayloadState`, `futureRecoveryOwner` | One concrete future frontier slot. `FutureFrontierEvidence` is derived from the slot instead of stored as an independent Boolean. |
| `futureEvidenceObserved` | A late or initially present future-evidence obligation. Once observed, the future slot must remain concrete evidence until promotion. |
| `futurePromotionReady`, `futurePromoted`, `promotionFresh` | The two-step future reanchor path: clear the stale/current pending wrapper, then promote the future slot into the active slot with active progress flags reset. This maps to future new-view / higher-frontier quorum handling in `on_pacemaker_propose_ready(...)`, covered by `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists`, `pacemaker_reanchors_future_new_view_quorum_while_vote_queue_backlogged`, and `pacemaker_reanchors_future_new_view_quorum_over_stale_frontier_owner`. |

## Running

From repository root:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh fork-fast
bash scripts/formal/sumeragi_apalache.sh fork-npos
bash scripts/formal/sumeragi_apalache.sh quorum-fast
bash scripts/formal/sumeragi_apalache.sh rbc-fast
bash scripts/formal/sumeragi_apalache.sh rbc-causality-fast
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-fast
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-fast
bash scripts/formal/sumeragi_apalache.sh classic-preimage-fast
bash scripts/formal/sumeragi_apalache.sh classic-signature-fast
bash scripts/formal/sumeragi_apalache.sh vrf-admission-fast
bash scripts/formal/sumeragi_apalache.sh vote-admission-fast
bash scripts/formal/sumeragi_apalache.sh proposal-hint-fast
bash scripts/formal/sumeragi_apalache.sh proposal-admission-fast
bash scripts/formal/sumeragi_apalache.sh block-created-admission-fast
bash scripts/formal/sumeragi_apalache.sh qc-signers-fast
bash scripts/formal/sumeragi_apalache.sh commit-roots-fast
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-fast
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-fast
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-fast
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-fast
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-fast
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-fast
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-fast
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-fast
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-fast
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-fast
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-fast
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-fast
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-fast
bash scripts/formal/sumeragi_apalache.sh certified-fetch-fast
bash scripts/formal/sumeragi_apalache.sh missing-block-fetch-fast
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-fast
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-fast
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-fast
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-fast
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-fast
bash scripts/formal/sumeragi_apalache.sh native-amx-routing-plan-fast
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-fast
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-fast
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-fast
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-fast
bash scripts/formal/sumeragi_apalache.sh vnext-signature-fast
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-fast
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-fast
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-fast
bash scripts/formal/sumeragi_apalache.sh vnext-validation-fast
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-fast
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-fast
bash scripts/formal/sumeragi_apalache.sh worker-drain-fast
bash scripts/formal/sumeragi_apalache.sh actor-gate-fast
bash scripts/formal/sumeragi_apalache.sh worker-budget-fast
bash scripts/formal/sumeragi_apalache.sh worker-ingress-fast
bash scripts/formal/sumeragi_apalache.sh npos-vrf-fast
bash scripts/formal/sumeragi_apalache.sh kura-commit-fast
bash scripts/formal/sumeragi_apalache.sh restart-replay-fast
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-fast
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-fast
bash scripts/formal/sumeragi_apalache.sh precommit-fast
bash scripts/formal/sumeragi_apalache.sh proposal-fast
bash scripts/formal/sumeragi_apalache.sh engine-tick-fast
bash scripts/formal/sumeragi_apalache.sh engine-new-view-subject-fast
bash scripts/formal/sumeragi_apalache.sh engine-handle-dispatch-fast
bash scripts/formal/sumeragi_apalache.sh engine-certificate-dispatch-fast
bash scripts/formal/sumeragi_apalache.sh engine-certificate-prefilter-state-fast
bash scripts/formal/sumeragi_apalache.sh engine-view-advance-saturation-fast
bash scripts/formal/sumeragi_apalache.sh engine-new-view-fast
bash scripts/formal/sumeragi_apalache.sh engine-new-view-highest-qc-fast
bash scripts/formal/sumeragi_apalache.sh engine-new-view-advance-fast
bash scripts/formal/sumeragi_apalache.sh engine-proposal-fast
bash scripts/formal/sumeragi_apalache.sh engine-proposal-output-fast
bash scripts/formal/sumeragi_apalache.sh engine-proposal-state-fast
bash scripts/formal/sumeragi_apalache.sh engine-proposal-validation-owner-fast
bash scripts/formal/sumeragi_apalache.sh engine-proposal-lock-fast
bash scripts/formal/sumeragi_apalache.sh qc-round-compatibility-fast
bash scripts/formal/sumeragi_apalache.sh engine-qc-ref-projection-fast
bash scripts/formal/sumeragi_apalache.sh engine-highest-qc-record-fast
bash scripts/formal/sumeragi_apalache.sh engine-commit-subject-fast
bash scripts/formal/sumeragi_apalache.sh engine-payload-lookup-fast
bash scripts/formal/sumeragi_apalache.sh engine-prepare-fast
bash scripts/formal/sumeragi_apalache.sh engine-prepare-lock-highest-fast
bash scripts/formal/sumeragi_apalache.sh engine-prepare-phase-fast
bash scripts/formal/sumeragi_apalache.sh engine-prepare-vote-cache-fast
bash scripts/formal/sumeragi_apalache.sh engine-commit-fast
bash scripts/formal/sumeragi_apalache.sh engine-commit-highest-qc-fast
bash scripts/formal/sumeragi_apalache.sh engine-commit-available-commit-fast
bash scripts/formal/sumeragi_apalache.sh engine-commit-pending-fetch-fast
bash scripts/formal/sumeragi_apalache.sh engine-commit-validation-cleanup-fast
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-fast
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-record-fast
bash scripts/formal/sumeragi_apalache.sh engine-reconfiguration-staging-fast
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-cleanup-fast
bash scripts/formal/sumeragi_apalache.sh engine-payload-record-fast
bash scripts/formal/sumeragi_apalache.sh engine-payload-fast
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-fast
bash scripts/formal/sumeragi_apalache.sh engine-validation-ownership-fast
bash scripts/formal/sumeragi_apalache.sh engine-validation-invalid-advance-fast
bash scripts/formal/sumeragi_apalache.sh reconfig-fast
bash scripts/formal/sumeragi_apalache.sh recovery-fast
bash scripts/formal/sumeragi_apalache.sh view-change-fast
bash scripts/formal/sumeragi_apalache.sh validation-fast
bash scripts/formal/sumeragi_apalache.sh admission-fast
bash scripts/formal/sumeragi_apalache.sh highest-fast
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
bash scripts/formal/sumeragi_tlc.sh frontier-small
```

The runner sets an explicit Apalache `--length` for each mode:

| Mode | Length | Intended use |
| --- | ---: | --- |
| `fast` | 10 | CI commit-path check |
| `deep` | 10 | Larger commit-path check |
| `fork-fast` | 9 | CI permissioned fork-safety check |
| `fork-npos` | 9 | CI NPoS stake-quorum fork-safety check |
| `quorum-fast` | 2 | CI quorum-policy arithmetic check |
| `rbc-fast` | 2 | CI RBC deliver-quorum gate check |
| `rbc-causality-fast` | 1 | CI RBC causality check |
| `pending-rbc-stash-fast` | 1 | CI pending-RBC stash cap/TTL/replay check |
| `rbc-preimage-fast` | 1 | CI RBC signing-preimage check |
| `classic-preimage-fast` | 1 | CI classic Vote/VRF signing-preimage check |
| `classic-signature-fast` | 1 | CI classic Vote/QC signature-verification check |
| `vrf-admission-fast` | 1 | CI VRF commit/reveal admission check |
| `vote-admission-fast` | 1 | CI classic inbound vote-admission check |
| `proposal-hint-fast` | 1 | CI proposal-hint admission check |
| `proposal-admission-fast` | 1 | CI proposal metadata admission check |
| `block-created-admission-fast` | 1 | CI direct `BlockCreated` payload admission check |
| `qc-signers-fast` | 2 | CI QC signer-bitmap admission check |
| `commit-roots-fast` | 2 | CI commit-root consistency check |
| `commit-pipeline-recovery-fast` | 2 | CI commit-pipeline recovery gate check |
| `commit-pipeline-scheduling-fast` | 1 | CI commit-pipeline scheduling gate check |
| `commit-result-drain-fast` | 1 | CI commit-result drain gate check |
| `commit-job-dispatch-fast` | 1 | CI commit-job dispatch gate check |
| `commit-inflight-timeout-fast` | 1 | CI commit-inflight timeout gate check |
| `post-commit-pacemaker-kick-fast` | 1 | CI post-commit pacemaker kick gate check |
| `idle-view-proposal-budget-fast` | 1 | CI proposal idle-view budget preservation gate check |
| `pacemaker-evaluation-fast` | 1 | CI pacemaker evaluation gate check |
| `cached-slot-timeout-fast` | 1 | CI cached proposal-slot timeout gate check |
| `proposal-parent-resolution-fast` | 1 | CI proposal parent resolution and inline backup transport gate check |
| `precommit-qc-view-change-fast` | 1 | CI precommit-QC view-change selector gate check |
| `commit-evidence-replay-fast` | 2 | CI known-block commit-evidence replay gate check |
| `block-sync-recovery-fast` | 2 | CI block-sync recovery admission gate check |
| `certified-fetch-fast` | 1 | CI direct certified-block fetch check |
| `missing-block-fetch-fast` | 1 | CI missing-block fetch planner check |
| `missing-block-hard-cap-fast` | 1 | CI missing-block hard-cap recovery check |
| `missing-block-hard-cap-cleanup-fast` | 1 | CI missing-block hard-cap cleanup check |
| `missing-block-view-change-fast` | 1 | CI missing-block view-change escalation check |
| `native-amx-attestation-fast` | 2 | CI native AMX attestation gate check |
| `native-amx-journal-fast` | 1 | CI native AMX queue-journal replay check |
| `native-amx-routing-plan-fast` | 1 | CI native AMX routing-plan projection check |
| `native-amx-receipt-fast` | 1 | CI native AMX receipt validation check |
| `native-amx-ingress-fast` | 1 | CI native AMX control-plane ingress check |
| `vnext-chain-order-fast` | 1 | CI vNext chain-order helper check |
| `vnext-rechain-fast` | 1 | CI quarantined vNext re-chain helper check |
| `vnext-signature-fast` | 1 | CI vNext aggregate certificate verification check |
| `vnext-signing-preimage-fast` | 1 | CI vNext signing-preimage construction check |
| `vnext-control-ingress-fast` | 1 | CI vNext control-certificate ingress check |
| `vnext-slot-lifecycle-fast` | 1 | CI vNext slot-lifecycle check |
| `vnext-validation-fast` | 1 | CI vNext validation ownership check |
| `vote-verify-async-fast` | 1 | CI async vote-verification ownership check |
| `qc-verify-async-fast` | 1 | CI async QC aggregate-verification ownership check |
| `worker-drain-fast` | 1 | CI worker-loop drain scheduler check |
| `actor-gate-fast` | 1 | CI actor-gate priority/fairness check |
| `worker-budget-fast` | 1 | CI worker-loop budget/adaptive-cap check |
| `worker-ingress-fast` | 1 | CI worker ingress routing check |
| `npos-vrf-fast` | 1 | CI NPoS VRF epoch-seal staging check |
| `kura-commit-fast` | 1 | CI Kura durability commit retry check |
| `restart-replay-fast` | 1 | CI restarted-peer replay check |
| `post-commit-cleanup-fast` | 1 | CI post-commit cleanup check |
| `frontier-gap-realign-fast` | 1 | CI frontier-gap realignment check |
| `precommit-fast` | 2 | CI precommit vote-emission gate check |
| `proposal-fast` | 2 | CI proposal assembly gate check |
| `engine-tick-fast` | 2 | CI pure engine tick gate check |
| `engine-new-view-subject-fast` | 1 | CI pure engine NewView subject projection helper check |
| `engine-handle-dispatch-fast` | 1 | CI pure engine top-level input dispatch check |
| `engine-certificate-dispatch-fast` | 1 | CI pure engine certificate prefilter dispatch check |
| `engine-certificate-prefilter-state-fast` | 1 | CI pure engine certificate prefilter state-handoff check |
| `engine-view-advance-saturation-fast` | 1 | CI pure engine view-advance saturation check |
| `engine-new-view-fast` | 2 | CI pure engine NewView-QC gate check |
| `engine-new-view-highest-qc-fast` | 1 | CI pure engine exact NewView highest-QC record check |
| `engine-new-view-advance-fast` | 1 | CI pure engine exact NewView-QC advance/output check |
| `engine-proposal-fast` | 2 | CI pure engine proposal-ingress gate check |
| `engine-proposal-output-fast` | 1 | CI pure engine exact proposal output-field check |
| `engine-proposal-state-fast` | 1 | CI pure engine exact proposal state-mutation check |
| `engine-proposal-validation-owner-fast` | 1 | CI pure engine exact proposal validation-owner check |
| `engine-proposal-lock-fast` | 1 | CI pure engine proposal-lock helper check |
| `qc-round-compatibility-fast` | 1 | CI pure engine QC-round compatibility helper check |
| `engine-qc-ref-projection-fast` | 1 | CI pure engine QC reference projection helper check |
| `engine-highest-qc-record-fast` | 1 | CI pure engine highest-QC record helper check |
| `engine-commit-subject-fast` | 1 | CI pure engine commit-subject helper check |
| `engine-payload-lookup-fast` | 1 | CI pure engine payload lookup helper check |
| `engine-prepare-fast` | 2 | CI pure engine prepare-QC gate check |
| `engine-prepare-lock-highest-fast` | 1 | CI pure engine exact Prepare-QC lock/highest-QC record check |
| `engine-prepare-phase-fast` | 1 | CI pure engine exact Prepare-QC phase-transition check |
| `engine-prepare-vote-cache-fast` | 1 | CI pure engine prepare-QC commit-vote cache/output check |
| `engine-commit-fast` | 2 | CI pure engine commit-QC gate check |
| `engine-commit-highest-qc-fast` | 1 | CI pure engine exact Commit-QC highest-QC record check |
| `engine-commit-available-commit-fast` | 1 | CI pure engine payload-available Commit-QC exact finality check |
| `engine-commit-pending-fetch-fast` | 1 | CI pure engine missing-payload Commit-QC pending/fetch check |
| `engine-commit-validation-cleanup-fast` | 1 | CI pure engine Commit-QC validation cleanup check |
| `engine-committed-block-fast` | 2 | CI pure engine committed-block gate check |
| `engine-committed-block-record-fast` | 1 | CI pure engine exact committed-map record check |
| `engine-reconfiguration-staging-fast` | 1 | CI pure engine reconfiguration staging check |
| `engine-committed-block-cleanup-fast` | 1 | CI pure engine committed-block cleanup side-effect check |
| `engine-payload-record-fast` | 1 | CI pure engine exact payload-availability record check |
| `engine-payload-fast` | 2 | CI pure engine payload-availability gate check |
| `engine-validation-result-fast` | 2 | CI pure engine validation-result gate check |
| `engine-validation-ownership-fast` | 1 | CI pure engine exact validation-owner cleanup check |
| `engine-validation-invalid-advance-fast` | 1 | CI pure engine exact invalid-validation round/output advance check |
| `reconfig-fast` | 7 | CI validator-set transition safety check |
| `recovery-fast` | 7 | CI certified payload recovery safety check |
| `view-change-fast` | 6 | CI view-change and lock-safety check |
| `validation-fast` | 6 | CI validation-callback ownership check |
| `admission-fast` | 6 | CI certificate-admission guard check |
| `highest-fast` | 6 | CI deterministic highest-QC selection check |
| `frontier-fast` | 7 | CI frontier check |
| `frontier-deep` | 8 | Larger frontier check |
| `frontier-wide` | 7 | Wider PR formal CI frontier check |
| `frontier-nightly` | 10 | Manual/scheduled wider-bound frontier check |

`APALACHE_LENGTH=<n>` overrides the per-mode default when locally exploring a
counterexample or widening a bounded proof.

`scripts/formal/sumeragi_tlc.sh frontier-small` runs a small exhaustive TLC
cross-check using the same module and TLC-friendly weak-fairness specification.
The TLC config disables generic deadlock rejection because resolved terminal
states, such as a legitimate zero-evidence drop, are valid endpoints; invariants
and temporal properties remain checked.

## Operating Process

Use the expected-failure configs as mutation tests when a formal model changes.
A useful model change should either keep every existing mutation red or add a
new expected-failure config before strengthening the spec.

If a new Taira hang report involves more than one concrete future frontier slot,
do not stretch this two-slot proof by adding more Boolean shortcuts. Add a
three-slot or parameterized follow-up model, then map the new transition back to
focused Rust regression tests.

If a counterexample only relies on abstract evidence predicates, first add or
tighten a Rust bridge test that exercises the corresponding Sumeragi state
transition. Runtime consensus code should change only after the bridge test
shows the abstraction mismatch is real.

The docs metadata job intentionally emits stale `source_hash` warnings for
translated Sumeragi formal READMEs until their bodies are refreshed. PR and
nightly CI upload a JSON metadata report so the translation refresh can be
tracked without pretending stale translations are current.

### Reproducible local setup (no Docker required)

Install the pinned local Apalache toolchain used by this repository:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

The runner auto-detects this install at:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
After installation, `ci/check_sumeragi_formal.sh` should work without extra env vars:

```bash
bash ci/check_sumeragi_formal.sh
```

The expected-failure mutations are part of normal formal CI through
`ci/check_sumeragi_formal.sh`. They should fail under Apalache and are useful
when changing the model:

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

Individual mutation modes are also accepted by the runner:

```bash
bash scripts/formal/sumeragi_apalache.sh frontier-bug-stale-owner
bash scripts/formal/sumeragi_apalache.sh frontier-bug-vote-queue
bash scripts/formal/sumeragi_apalache.sh frontier-bug-payload-recovery
bash scripts/formal/sumeragi_apalache.sh frontier-bug-retransmit-followthrough
bash scripts/formal/sumeragi_apalache.sh frontier-bug-future-promotion
bash scripts/formal/sumeragi_apalache.sh frontier-bug-future-reanchor-clear
bash scripts/formal/sumeragi_apalache.sh frontier-bug-future-evidence-drop
bash scripts/formal/sumeragi_apalache.sh frontier-bug-promotion-reset
bash scripts/formal/sumeragi_apalache.sh frontier-bug-future-stale-owner
bash scripts/formal/sumeragi_apalache.sh frontier-bug-progress-touch
bash scripts/formal/sumeragi_apalache.sh frontier-bug-height-only-recovery
bash scripts/formal/sumeragi_apalache.sh fork-bug-double-sign
bash scripts/formal/sumeragi_apalache.sh quorum-bug-count-under-threshold
bash scripts/formal/sumeragi_apalache.sh quorum-bug-count-over-validators
bash scripts/formal/sumeragi_apalache.sh quorum-bug-stake-exact-two-thirds
bash scripts/formal/sumeragi_apalache.sh quorum-bug-stake-over-total
bash scripts/formal/sumeragi_apalache.sh quorum-bug-stake-invalid-input
bash scripts/formal/sumeragi_apalache.sh quorum-bug-stake-overflow
bash scripts/formal/sumeragi_apalache.sh rbc-bug-duplicate-ready
bash scripts/formal/sumeragi_apalache.sh rbc-bug-under-quorum-deliver
bash scripts/formal/sumeragi_apalache.sh rbc-bug-wrong-commit-formula
bash scripts/formal/sumeragi_apalache.sh rbc-bug-force-one-ignored
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-init-skip-header-hash
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-init-skip-leader-signature
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-init-skip-chunk-root
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-init-skip-roster-hash
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-invalid-init-creates-session
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-drop-mutates-session
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-chunk-before-init-records
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-chunk-bad-digest-records
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-local-ready-before-complete-payload
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-local-ready-without-root-check
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-ready-before-init-records
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-ready-bad-signature-records
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-ready-roster-mismatch-records
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-ready-root-mismatch-records
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-ready-conflict-not-invalidated
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-ready-conflict-keeps-pending
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-deliver-before-init-records
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-deliver-bad-signature-records
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-deliver-root-mismatch-records
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-deliver-unvalidated-ready-bundle-records
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-deliver-invalid-ready-bundle-records
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-deliver-duplicate-records
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-deliver-records-without-wake
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-deliver-wakes-without-record
bash scripts/formal/sumeragi_apalache.sh rbc-causality-bug-stash-wakes-commit
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-drop-insertable-chunk
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-accept-zero-cap-chunk
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-accept-oversize-chunk
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-skip-eviction-for-capped-insert
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-evict-when-not-needed
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-skip-pending-bound
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-cap-drop-skips-counter
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-count-clean-insert-as-drop
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-ignore-ready-byte-cap
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-drop-ready-with-capacity
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-skip-ready-drop-counter
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-ready-counter-on-chunk-drop
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-ignore-deliver-byte-cap
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-drop-deliver-with-capacity
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-skip-deliver-drop-counter
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-deliver-counter-on-chunk-drop
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-no-touch-on-insert
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-no-touch-on-drop
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-no-touch-on-slot
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-spurious-touch-without-traffic
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-skip-ttl-eviction
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-ttl-uses-first-seen
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-ttl-disabled-evicts
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-ttl-evicts-active-session
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-skip-session-cap-eviction
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-session-cap-evicts-active
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-session-cap-rejects-existing-key
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-session-cap-rejects-after-inactive-evict
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-session-cap-ignores-under-limit
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-session-cap-ignores-limit
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-reject-available-slot
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-evict-newest-instead-of-oldest
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-oldest-marker-without-eviction
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-flush-drops-chunks
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-flush-drops-ready
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-flush-drops-deliver
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-flush-keeps-pending
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-replay-on-non-flush
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-eviction-keeps-dedup
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-eviction-skips-metrics
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-eviction-skips-repair
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-eviction-skips-backlog-publish
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-replay-evicted-chunk
bash scripts/formal/sumeragi_apalache.sh pending-rbc-stash-bug-flush-replays-dropped-frame
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-drop-chain-id
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-drop-mode-tag
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-drop-version
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-ready-uses-deliver-type
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-deliver-uses-ready-type
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-drop-block-hash
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-drop-height
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-drop-view
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-drop-epoch
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-drop-roster-hash
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-drop-chunk-root
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-drop-sender
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-ready-includes-signature
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-deliver-includes-signature
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-deliver-omits-ready-count
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-deliver-omits-ready-bundle
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-deliver-omits-entry-order
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-deliver-omits-entry-sender
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-deliver-omits-entry-sig-len
bash scripts/formal/sumeragi_apalache.sh rbc-preimage-bug-deliver-omits-entry-signature
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-chain-id
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-mode-tag
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-proto-version
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-domain-protocol
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-version
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-vote-uses-vrf-commit-type
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-vrf-commit-uses-vote-type
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-vrf-reveal-uses-commit-type
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-block-hash
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-parent-state-root
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-post-state-root
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-height
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-view
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-epoch
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-chain-order-hash
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-rechain-seq
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-phase
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-vote-without-highest-omits-absent-flag
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-vote-without-highest-includes-highest-body
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-vote-with-highest-omits-present-flag
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-highest-height
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-highest-view
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-highest-epoch
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-highest-subject-block-hash
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-drop-highest-phase
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-vrf-commit-drops-signer
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-vrf-commit-drops-commitment
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-vrf-reveal-drops-signer
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-vrf-reveal-drops-reveal
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-vote-includes-signature
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-vrf-commit-includes-signature
bash scripts/formal/sumeragi_apalache.sh classic-preimage-bug-vrf-reveal-includes-signature
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-accept-mode-tag-mismatch
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-accept-validator-set-mismatch
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-allow-empty-roster
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-ignore-bitmap-length
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-ignore-bitmap-out-of-range
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-allow-empty-signer-set
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-ignore-count-quorum
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-use-non-strict-stake
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-allow-missing-stake-snapshot
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-accept-missing-aggregate-signature
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-ignore-missing-pop
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-accept-bad-aggregate-signature
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-ignore-missing-votes
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-ignore-subject-mismatch
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-ignore-roots-mismatch
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-ignore-vote-invalid-signature
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-ignore-view-mapping-failure
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-allow-non-new-view-highest
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-allow-new-view-missing-highest
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-ignore-new-view-highest-subject
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-ignore-new-view-highest-height
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-ignore-new-view-highest-epoch
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-ignore-new-view-highest-phase
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-ignore-vote-highest-mismatch
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-return-full-roster
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-drop-returned-signer
bash scripts/formal/sumeragi_apalache.sh classic-signature-bug-return-signers-on-reject
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-accept-unsupported-mode
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-accept-missing-manager
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-ignore-signer-out-of-topology
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-accept-missing-signature
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-accept-bad-signature
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-accept-epoch-mismatch
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-accept-unknown-signer
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-accept-commit-out-of-window
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-accept-commitment-rewrite
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-accept-reveal-in-commit-window
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-accept-reveal-without-commit
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-accept-reveal-commit-mismatch
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-accept-reveal-rewrite
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-broadcast-on-reject
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-rebroadcast-network-origin
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-skip-external-broadcast
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-skip-stage-on-accept
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-update-local-for-remote-signer
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-skip-local-update
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-skip-prf-refresh-on-reveal
bash scripts/formal/sumeragi_apalache.sh vrf-admission-bug-refresh-prf-on-late-reveal
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-accept-height-or-view-drop
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-accept-locked-conflict
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-accept-roster-missing
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-record-duplicate
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-accept-non-new-view-highest
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-accept-chain-order-mismatch
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-accept-bad-signature
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-accept-new-view-missing-highest
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-accept-new-view-bad-highest-epoch
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-accept-new-view-bad-highest-phase
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-accept-new-view-hash-mismatch
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-accept-new-view-height-mismatch
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-accept-new-view-local-metadata-mismatch
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-record-same-slot-conflict
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-drop-superseded-conflict
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-record-deferred-conflict
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-accept-same-key-conflict
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-skip-defer-missing-context
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-skip-roster-defer
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-skip-double-vote-evidence
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-evidence-on-superseded
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-skip-cross-phase-evidence
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-skip-qc-attempt-on-accept
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-qc-attempt-on-reject
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-cache-new-view-roster
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-skip-roster-cache
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-track-stale-new-view
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-skip-new-view-track
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-request-pipeline-for-stale-new-view
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-skip-commit-pipeline-request
bash scripts/formal/sumeragi_apalache.sh vote-admission-bug-skip-progress-touch
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-drop-accepted-hint
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-accept-stale-height
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-accept-stale-view
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-accept-highest-height-mismatch
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-accept-highest-epoch-mismatch
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-accept-cached-conflict
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-accept-stored-height-mismatch
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-accept-committed-conflict
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-accept-missing-committed-highest
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-accept-missing-future-highest
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-accept-local-height-mismatch
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-accept-local-view-mismatch
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-accept-locked-qc-reject
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-skip-defer-dependency
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-skip-drop-on-reject
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-skip-cache-on-accept
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-skip-deferred-cross-view-cache
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-cache-same-view-deferred
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-cache-rejected-hint
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-skip-observed-on-accept
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-observe-rejected-hint
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-skip-highest-update
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-spurious-highest-update
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-highest-update-on-reject
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-skip-dependency-request
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-request-dependency-for-clean-hint
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-skip-defer-marker
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-marker-for-clean-hint
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-skip-prf-update
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-prf-update-before-admission
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-skip-phase-sample
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-phase-sample-on-reject
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-skip-replay
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-replay-on-reject
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-skip-prune
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-prune-on-reject
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-skip-stale-height-prune
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-prune-committed-on-non-stale
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-skip-committed-conflict-suppression
bash scripts/formal/sumeragi_apalache.sh proposal-hint-bug-suppress-clean-hint
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-drop-accepted-proposal
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-accept-stale-height
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-accept-stale-view
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-accept-proposal-epoch-mismatch
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-accept-highest-height-mismatch
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-accept-highest-epoch-mismatch
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-accept-parent-hash-mismatch
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-accept-stored-height-mismatch
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-accept-committed-conflict
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-accept-missing-committed-highest
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-accept-missing-future-highest
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-accept-local-height-mismatch
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-accept-local-view-mismatch
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-accept-locked-qc-reject
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-skip-defer-dependency
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-skip-drop-on-reject
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-skip-cache-on-accept
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-cache-rejected-proposal
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-skip-observed-on-accept
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-observe-rejected-proposal
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-skip-highest-update
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-spurious-highest-update
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-highest-update-on-reject
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-skip-dependency-request
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-request-dependency-for-clean-proposal
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-skip-defer-marker
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-marker-for-clean-proposal
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-skip-prf-update
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-prf-update-before-admission
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-skip-leader-context
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-leader-context-on-reject
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-skip-phase-sample
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-phase-sample-on-reject
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-skip-replay
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-replay-on-reject
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-skip-prune
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-prune-on-reject
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-skip-stale-height-prune
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-prune-committed-on-non-stale
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-skip-committed-conflict-suppression
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-suppress-clean-proposal
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-wake-commit-pipeline
bash scripts/formal/sumeragi_apalache.sh proposal-admission-bug-record-payload-phase
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-accept-authoritative-owner-conflict
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-accept-empty-payload-without-triggers
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-accept-hint-mismatch
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-accept-local-removed
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-accept-lock-rejected-sink
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-accept-locked-qc-no-hint
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-accept-locked-qc-with-hint
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-accept-missing-highest-hint
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-accept-proposal-mismatch-preserve
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-accept-rbc-payload-mismatch
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-accept-stale-height
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-accept-stale-view-without-request
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-authority-for-passive-or-reject
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-cache-proposal-on-reject
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-clear-missing-dependency-request
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-commit-pipeline-on-reject
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-defer-clean-block
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-drop-clean-block
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-drop-valid-block
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-duplicate-handling-on-clean-block
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-evidence-on-clean-block
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-lock-reject-on-clean-block
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-marker-for-clean-block
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-observe-proposal-on-reject
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-payload-mismatch-recovery-on-clean
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-pending-update-on-reject
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-phase-sample-on-reject
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-preserve-clean-block
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-request-dependency-for-clean-block
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-request-gap-for-current-height
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-request-parent-for-current-height
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-retain-clean-as-passive
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-retain-rejected-as-passive
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-authoritative-owner
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-commit-pipeline-request
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-defer-marker
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-dependency-request
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-drop-on-reject
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-duplicate-drop
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-duplicate-handling
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-gap-request
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-invalid-evidence
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-lock-reject-record
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-missing-request-clear
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-parent-request
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-passive-retained
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-payload-mismatch-recovery
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-pending-update
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-phase-sample
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-proposal-cache
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-proposal-observed
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-replay-preserve
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-skip-stale-cleanup
bash scripts/formal/sumeragi_apalache.sh block-created-admission-bug-stale-cleanup-on-fresh
bash scripts/formal/sumeragi_apalache.sh qc-signers-bug-count-observers
bash scripts/formal/sumeragi_apalache.sh qc-signers-bug-ignore-bitmap-length
bash scripts/formal/sumeragi_apalache.sh qc-signers-bug-ignore-out-of-bounds
bash scripts/formal/sumeragi_apalache.sh qc-signers-bug-under-quorum-accept
bash scripts/formal/sumeragi_apalache.sh commit-roots-bug-mix-root-signers
bash scripts/formal/sumeragi_apalache.sh commit-roots-bug-count-wrong-context
bash scripts/formal/sumeragi_apalache.sh commit-roots-bug-tie-high-root
bash scripts/formal/sumeragi_apalache.sh commit-roots-bug-stake-ignores-weight
bash scripts/formal/sumeragi_apalache.sh commit-roots-bug-under-quorum-accept
bash scripts/formal/sumeragi_apalache.sh commit-roots-bug-validate-mismatched-roots
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-skip-local-qc-formation
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-recover-despite-local-quorum
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-request-recovery-before-timeout
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-request-recovery-without-local-vote
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-request-recovery-with-commit-qc
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-request-recovery-with-missing-data
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-request-recovery-invalid-pending
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-request-recovery-off-tip
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-skip-missing-qc-request
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-drop-commit-qc-marker
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-skip-quorum-retransmit
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-use-collector-targets
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-rebroadcast-without-votes
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-rebroadcast-after-qc
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-run-tick-without-work
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-skip-tick-active-candidate
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-skip-tick-inflight
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-skip-tick-wakeup
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-run-tick-on-saturation-only
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-skip-event-entry
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-event-backlog-suppresses
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-event-backlog-fabricates-candidate
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-skip-event-reschedule
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-reschedule-after-candidate
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-skip-backlog-observation
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-bypass-deadline-without-pressure
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-miss-deadline-bypass-wakeup
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-miss-deadline-bypass-saturation
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-bypass-deadline-without-candidate
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-skip-budget-wakeup
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-process-after-budget-exhausted
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-update-last-run-on-budget-exhausted-before-candidates
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-skip-last-run-with-candidates
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-last-run-without-candidates
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-keep-wakeup-after-tick-entry
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-clear-wakeup-without-entry
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-include-recovery-without-wakeup-or-saturation
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-exclude-recovery-with-wakeup
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-exclude-recovery-with-saturation
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-include-recovery-when-active-without-qc
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-exclude-recovery-with-active-qc
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-preserve-idle-without-wakeup
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-preserve-idle-without-candidate
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-skip-idle-preserve
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-process-candidate-without-pipeline
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-scheduling-bug-skip-candidate-processing
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-apply-without-result-rx
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-apply-on-empty
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-skip-apply-matching
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-apply-id-mismatch
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-drop-inflight-on-id-mismatch
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-skip-ignore-stale-result
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-record-summary-on-ignored-result
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-apply-without-inflight
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-skip-summary-matching
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-skip-progress-matching
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-kickstart-rejected
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-skip-kickstart-committed
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-clear-worker-on-empty
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-keep-worker-on-disconnected
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-inline-without-inflight
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-skip-inline-disconnected
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-skip-apply-disconnected
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-summary-without-apply
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-progress-without-apply
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-kickstart-without-apply
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-allow-recovery-without-local-outside
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-allow-recovery-without-commit-qc
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-deny-recovery-with-local-outside-qc
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-fail-to-clear-inflight-after-apply
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-restore-inflight-after-apply
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-no-loop-stop-on-empty
bash scripts/formal/sumeragi_apalache.sh commit-result-drain-bug-continue-after-disconnected
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-same-block-requeued
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-same-block-starts-second-job
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-other-block-dropped
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-other-block-overwrites-inflight
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-worker-ready-executes-inline
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-worker-ready-skips-enqueue
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-worker-ready-skips-inflight
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-worker-ready-skips-start-record
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-worker-ready-returns-false
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-queue-full-enqueues
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-queue-full-sets-inflight
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-queue-full-drops-pending
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-queue-full-returns-true
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-disconnected-keeps-worker-state
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-disconnected-skips-inline
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-disconnected-sets-worker-inflight
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-disconnected-drops-unrecoverable
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-missing-work-tx-tries-enqueue
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-missing-result-rx-tries-enqueue
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-missing-worker-inline-skipped
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-missing-worker-clears-state
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-inline-sets-worker-inflight
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-inline-drops-unrecoverable
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-enqueue-and-inline-same-job
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-inflight-and-pending-same-job
bash scripts/formal/sumeragi_apalache.sh commit-job-dispatch-bug-start-without-return-true
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-timeout-zero-reports
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-no-inflight-reports
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-below-timeout-reports
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-clock-before-enqueue-reports
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-at-timeout-missed
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-above-timeout-missed
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-already-reported-repeats
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-already-reported-clears-flag
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-non-timeout-sets-flag
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-timeout-without-status-record
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-timeout-without-warning
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-status-without-new-report
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-warning-without-new-report
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-timeout-returns-false
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-non-timeout-returns-true
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-timeout-clears-inflight
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-timeout-requeues-pending
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-timeout-marks-pending-aborted
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-timeout-prunes-proposal
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-timeout-forces-view
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-timeout-triggers-view-change
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-timeout-records-commit-failure
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-timeout-applies-outcome
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-timeout-kickstarts-pacemaker
bash scripts/formal/sumeragi_apalache.sh commit-inflight-timeout-bug-timeout-detaches-late-result
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-trigger-without-queue
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-skip-healthy-queue
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-skip-queue-saturated-pacing
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-skip-consensus-pacing
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-skip-combined-pacing
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-trigger-active-pending
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-trigger-rbc-backlog
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-trigger-relay-backpressure
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-trigger-hard-stop
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-use-callback-result-false
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-return-true-without-trigger
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-trigger-without-return-true
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-capture-time-when-suppressed
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-skip-time-when-triggered
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-ignore-active-pending-with-pacing
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-ignore-rbc-with-pacing
bash scripts/formal/sumeragi_apalache.sh post-commit-pacemaker-kick-bug-ignore-relay-with-pacing
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-preserve-without-queue
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-preserve-during-mode-flip
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-preserve-during-commit-inflight
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-preserve-before-deadline
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-skip-healthy-due
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-skip-queue-saturated-pacing
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-skip-consensus-pacing
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-skip-combined-pacing
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-preserve-active-pending
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-preserve-rbc-backlog
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-preserve-relay-backpressure
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-ignore-active-pending-with-pacing
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-ignore-rbc-with-pacing
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-ignore-relay-with-pacing
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-run-idle-repair-when-preserved
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-reserve-proposal-without-preserve
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-preserve-without-proposal-slot
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-retry-without-skip
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-retry-without-queue
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-retry-with-pending-blocks
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-retry-with-commit-inflight
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-skip-retry-frontier-empty
bash scripts/formal/sumeragi_apalache.sh idle-view-proposal-budget-bug-retry-runs-before-proposal
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-log-initial-missing
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-log-initial-repeats
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-log-initial-without-deferral
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-pacing-first-before-no-fire-log
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-pacing-subsequent-before-logs-fire
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-pacing-due-skips-attempt
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-pacing-due-skips-fire-log
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-pacing-due-no-deadline-advance
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-hard-before-logs-fire
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-hard-due-attempts
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-hard-due-skips-fire-log
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-hard-due-no-deadline-advance
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-healthy-before-attempts
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-healthy-due-skips-attempt
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-healthy-due-logs-deferral
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-recovered-keeps-tracker-deferring
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-recovered-due-skips-attempt
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-deferral-not-tracked
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-deadline-advances-before-due
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-attempt-without-deadline
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-attempt-before-deadline
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-attempt-pacing-before-deadline
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-attempt-recovered-before-deadline
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-attempt-under-hard-backpressure
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-deadline-advanced-before-fire
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-deadline-not-advanced-on-fire
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-log-fire-without-deferral
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-log-hard-before-deadline
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-log-pacing-repeat-before-deadline
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-repeat-initial-deferral-log
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-skip-first-deferral-log
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-skip-hard-deadline-log
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-skip-healthy-deadline-attempt
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-skip-pacing-deadline-attempt
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-skip-pacing-deadline-log
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-skip-pacing-initial-log
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-skip-recovered-deadline-attempt
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-tracker-not-cleared-on-recovery
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-tracker-not-set-on-deferral
bash scripts/formal/sumeragi_apalache.sh pacemaker-evaluation-bug-tracker-set-without-deferral
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-fast-without-votes
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-fast-far-from-quorum
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-fast-at-quorum
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-skip-near-min-path
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-skip-near-fast-shorter
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-fast-without-missing-data
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-fast-with-consensus-backlog
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-fast-with-rbc-incomplete
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-fast-with-both-backlogs
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-return-near-when-min-is-base
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-return-fast-for-non-near
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-hysteresis-in-permissioned
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-hysteresis-zero-timeout
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-hysteresis-without-previous
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-hysteresis-height-mismatch
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-hysteresis-same-view
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-hysteresis-lower-view
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-no-wait-before-boundary
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-boundary-still-waits
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-after-still-waits
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-skip-streak-increment
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-wrong-factor-streak0
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-wrong-factor-streak1
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-wrong-factor-streak2
bash scripts/formal/sumeragi_apalache.sh cached-slot-timeout-bug-streak-not-capped-for-factor
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-return-parent-at-height-zero
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-return-parent-at-height-one
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-skip-kura-parent
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-pending-overrides-kura
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-skip-pending-parent
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-pending-wrong-hash-accepted
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-pending-wrong-height-accepted
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-height-one-checks-pending
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-skip-defer-on-missing-parent
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-defer-when-parent-found
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-skip-kura-lookup-nonzero
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-lookup-kura-height-zero
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-skip-overflow-log
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-overflow-blocks-pending-fallback
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-seed-without-da
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-seed-without-inline
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-seed-without-backup
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-skip-seed-all-enabled
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-rbc-without-da
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-skip-rbc-primary
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-rbc-inline-without-backup
bash scripts/formal/sumeragi_apalache.sh proposal-parent-resolution-bug-skip-rbc-backup
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-select-committed-when-none
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-select-non-commit-highest-without-committed
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-select-non-commit-highest-over-committed
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-skip-highest-without-committed
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-skip-committed-without-highest
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-committed-over-newer-height
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-committed-over-higher-height-lower-view
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-committed-over-same-height-newer-view
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-committed-over-equal-slot-highest
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-highest-over-same-height-older-view
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-highest-over-older-height
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-highest-over-lower-height-higher-view
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-comparison-uses-view-before-height
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-select-none-when-both-commit
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-drop-commit-highest-filter
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-accept-non-commit-filter
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-skip-committed-fallback
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-skip-height-view-comparison
bash scripts/formal/sumeragi_apalache.sh precommit-qc-view-change-bug-tie-does-not-prefer-highest
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-replay-inactive
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-ignore-cooldown
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-replay-without-targets
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-skip-first-evidence
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-skip-progress
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-skip-stalled-retry
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-replay-no-evidence
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-votes-use-payload-fallback
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-commit-qc-uses-votes
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-drop-commit-qc-replay
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-use-local-targets
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-use-duplicate-targets
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-accept-stale-without-request
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-drop-requested-stale
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-accept-future-unrequested
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-revive-aborted-without-commit-qc
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-keep-aborted-with-commit-qc
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-skip-vote-backed-owner
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-steal-owner-with-payload-only
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-skip-certified-owner
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-activate-uncertified-conflict
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-drop-commit-qc-marker
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-skip-missing-commit-qc-request
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-keep-missing-request
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-clear-inflight-for-payload-only
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-keep-inflight-for-certified
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-promote-unvalidated-qc
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-request-non-commit-qc
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-skip-signer-targets
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-no-topology-fallback
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-keep-local-target
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-request-without-remote-targets
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-low-priority-request
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-use-generic-missing-fetch
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-accept-forged-requester
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-serve-missing-local-block
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-serve-mismatched-local-subject
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-serve-without-commit-qc
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-serve-mismatched-commit-qc
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-drop-npos-stake-snapshot
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-split-small-full-response
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-send-oversized-full
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-send-oversized-proof
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-drop-instead-of-body-response-fallback
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-drop-instead-of-block-created-fallback
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-drop-proof-when-body-too-large
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-accept-response-height-mismatch
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-accept-response-view-mismatch
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-accept-response-block-hash-mismatch
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-accept-response-qc-height-mismatch
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-accept-response-qc-view-mismatch
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-accept-uncertified-response
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-accept-checkpoint-mismatch
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-proof-does-not-cache-qc
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-malformed-proof-caches-qc
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-body-without-proof-materializes
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-mismatched-body-materializes
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-proof-only-materializes
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-full-response-skips-proof-admission
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-invalid-inflight-materializes
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-invalid-pending-materializes
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-retry-aborted-dropped
bash scripts/formal/sumeragi_apalache.sh certified-fetch-bug-materialization-leaves-deferrals
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-trigger-before-hard-cap
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-drop-hard-cap-trigger
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-ignore-dependency-progress
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-ignore-rbc-progress
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-ignore-inflight-range-pull
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-retrigger-current-view
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-skip-already-triggered-budget-seal
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-trigger-non-active-height
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-trigger-non-contiguous-height
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-trigger-background-priority
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-ignore-advanced-current-view
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-ignore-escalated-view
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-ignore-tier-deferral
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-ignore-view-change-deferral
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-ignore-stall-window
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-drop-stall-window-trigger
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-require-stall-window-for-lock-lag
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-lock-lag-triggers-background
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-lock-lag-ignores-advanced-view
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-lock-lag-ignores-already-triggered
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-lock-lag-ignores-escalated-view
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-skip-budget-escalated-record
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-skip-request-latch
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-wrong-view-change-cause
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-latch-on-suppressed
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-keep-recovery-without-actionable
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-progress-without-actionable
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-bug-range-pull-triggers-view-change
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-preserve-non-frontier
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-preserve-without-material
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-ignore-owner-material
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-ignore-pending-material
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-ignore-inflight-material
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-ignore-rbc-session-material
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-ignore-rbc-pending-material
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-treat-invalid-pending-live
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-treat-non-tip-pending-live
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-prune-live-same-height
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-drop-live-metadata
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-clear-live-recovery-state
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-drop-valid-rbc
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-drop-rbc-pacing
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-keep-invalid-rbc
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-keep-future-pending
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-keep-future-missing
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-keep-future-rbc
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-drop-quorum-missing-request
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-purge-quorum-same-height-rbc
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-keep-no-live-pending
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-keep-no-live-recovery-state
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-drop-frontier-new-view-evidence
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-drop-same-view-owner
bash scripts/formal/sumeragi_apalache.sh missing-block-hard-cap-cleanup-bug-keep-quorum-timeout-stale-owner
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-background-can-trigger
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-missing-window-triggers
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-zero-window-triggers
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-early-dwell-triggers
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-dwell-boundary-dropped
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-current-view-retriggers
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-old-view-latch-blocks-new-view
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-missing-last-trigger-rejected
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-recent-last-trigger-ignored
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-last-trigger-boundary-dropped
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-mark-due-returns-false
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-skip-trigger-view-record
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-wrong-triggered-view
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-skip-last-trigger-record
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-mark-without-due
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-mark-not-due-mutates
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-clear-keeps-window
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-clear-keeps-trigger
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-clear-removes-request
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-scheduler-ignores-view-window
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-scheduler-reschedules-current-view
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-scheduler-arms-missing-window
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-scheduler-arms-zero-window
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-ignore-dependency-progress
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-ignore-rbc-progress
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-require-view-match-for-progress
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-ignore-range-pull-progress
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-defer-stale-progress
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-ignore-backlog
bash scripts/formal/sumeragi_apalache.sh missing-block-view-change-bug-backlog-sticks-forever
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-seal-non-native-plan
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-seal-empty-roster
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-skip-prepare-request
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-skip-commit-request
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-request-commit-before-prepare
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-retry-prepare-after-quorum
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-seal-with-prepare-only
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-seal-with-commit-only
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-seal-partial-multi-leg
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-accept-duplicate-prepare
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-accept-duplicate-commit
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-accept-wrong-prepare-body
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-accept-wrong-commit-body
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-accept-outsider-signer
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-use-arrival-order-bitmap
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-collapse-retry-bodies
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-collapse-participant-legs
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-drop-native-plan
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-collapse-native-to-single
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-single-plan-as-native
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-drop-participants
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-reorder-participants
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-keep-duplicate-participant
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-recompute-digest-wrong
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-drop-gossip-payload
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-drop-entrypoint
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-remove-by-hash-only
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-ignore-exact-remove
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-replay-unsupported-version
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-first-put-wins
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-compaction-drops-live
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-compaction-keeps-removed
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-keep-torn-tail
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-drop-prior-on-tail-repair
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-missing-receipt
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-reject-valid-single
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-single-receipt
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-unsigned-entrypoint
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-unsupported-version
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-source-mismatch
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-coordinator-mismatch
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-height-mismatch
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-plan-digest-mismatch
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-missing-participant
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-unexpected-participant
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-duplicate-participant
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-qc-source-mismatch
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-qc-entrypoint-mismatch
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-qc-plan-digest-mismatch
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-qc-wrong-phase
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-qc-coordinator-mismatch
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-qc-participant-mismatch
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-qc-height-mismatch
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-validator-hash-version
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-validator-set-hash
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-unknown-dataspace
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-small-validator-set
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-bad-bitmap-length
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-bitmap-oob
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-non-bls-signer
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-missing-pop
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-under-quorum
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-missing-signature
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-accept-invalid-signature
bash scripts/formal/sumeragi_apalache.sh native-amx-receipt-bug-reject-valid-native
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-reply-wrong-prepare-phase
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-reply-wrong-commit-phase
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-reply-local-non-bls
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-reply-local-missing-pop
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-drop-valid-prepare-request
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-drop-valid-commit-request
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-wrong-reply-peer
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-wrong-reply-phase
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-wrong-reply-signer
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-wrong-reply-body
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-cache-non-bls-vote
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-cache-missing-pop-vote
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-cache-invalid-pop-vote
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-cache-invalid-signature-vote
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-drop-valid-prepare-vote
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-drop-valid-commit-vote
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-cache-duplicate-signer-twice
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-drop-retried-body
bash scripts/formal/sumeragi_apalache.sh native-amx-ingress-bug-drop-different-participant
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-accept-empty-evidence
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-ignore-slot-mismatch
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-ignore-order-hash-mismatch
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-ignore-sequence-mismatch
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-accept-non-successor
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-allow-tail-accuser
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-skip-sequential-scope
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-allow-duplicate-evidence
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-ignore-untainted-limit
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-ignore-count-quorum
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-use-non-strict-stake
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-drop-accuser-taint
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-drop-accused-taint
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-keep-tainted-in-critical
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-do-not-increment-sequence
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-mutate-certificate-slot
bash scripts/formal/sumeragi_apalache.sh vnext-rechain-bug-reuse-previous-hash
bash scripts/formal/sumeragi_apalache.sh vnext-signature-bug-accept-missing-signature
bash scripts/formal/sumeragi_apalache.sh vnext-signature-bug-allow-empty-roster
bash scripts/formal/sumeragi_apalache.sh vnext-signature-bug-ignore-bitmap-length
bash scripts/formal/sumeragi_apalache.sh vnext-signature-bug-ignore-bitmap-out-of-range
bash scripts/formal/sumeragi_apalache.sh vnext-signature-bug-allow-empty-signer-set
bash scripts/formal/sumeragi_apalache.sh vnext-signature-bug-ignore-pop-length
bash scripts/formal/sumeragi_apalache.sh vnext-signature-bug-ignore-count-quorum
bash scripts/formal/sumeragi_apalache.sh vnext-signature-bug-use-non-strict-stake
bash scripts/formal/sumeragi_apalache.sh vnext-signature-bug-allow-non-bls-signer
bash scripts/formal/sumeragi_apalache.sh vnext-signature-bug-accept-bad-aggregate-signature
bash scripts/formal/sumeragi_apalache.sh vnext-signature-bug-ignore-rechain-slot-mismatch
bash scripts/formal/sumeragi_apalache.sh vnext-signature-bug-ignore-rechain-hash-mismatch
bash scripts/formal/sumeragi_apalache.sh vnext-signature-bug-ignore-rechain-sequence-mismatch
bash scripts/formal/sumeragi_apalache.sh vnext-signature-bug-return-full-roster
bash scripts/formal/sumeragi_apalache.sh vnext-signature-bug-drop-returned-signer
bash scripts/formal/sumeragi_apalache.sh vnext-signature-bug-return-signers-on-reject
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-drop-chain-id
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-drop-mode-tag
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-drop-vnext-version
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-use-view-type-for-rechain
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-use-rechain-type-for-view
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-drop-rechain-slot
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-drop-rechain-previous-hash
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-drop-rechain-new-hash
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-drop-rechain-new-order
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-drop-rechain-sequence
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-drop-rechain-tainted
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-drop-rechain-suspicions
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-include-rechain-signature
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-include-rechain-bitmap
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-rechain-vote-drops-body
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-rechain-vote-keeps-signature
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-drop-view-new-view
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-drop-view-highest-slot
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-drop-view-chain-order-hash
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-include-view-signature
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-include-view-bitmap
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-view-vote-drops-body
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-view-vote-keeps-signature
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-suspect-hash-drops-accuser
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-suspect-hash-drops-accused
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-suspect-hash-drops-obligation
bash scripts/formal/sumeragi_apalache.sh vnext-signing-preimage-bug-suspect-hash-includes-signature
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-no-round-updates
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-no-round-requires
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-current-rejects
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-current-updates
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-hash-mismatch-installs
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-hash-mismatch-no-reject
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-hash-mismatch-requires
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-valid-no-update
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-valid-no-last-rechain
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-valid-no-install
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-valid-rejects
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-excess-updates
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-excess-no-require
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-weakened-updates
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-weakened-no-require
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-evidence-updates
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-evidence-installs
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-evidence-no-reject
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-require-no-clear
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-require-no-vote
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-require-no-trigger
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-rechain-records-last-without-update
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-view-highest-no-abort
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-view-missing-round-aborts
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-view-no-highest-aborts
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-view-zero-triggers
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-view-nonzero-no-trigger
bash scripts/formal/sumeragi_apalache.sh vnext-control-ingress-bug-view-no-install
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-install-without-base
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-proposal-overwrites-committed
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-availability-overwrites-committed
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-validation-dispatch-without-round
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-validation-dispatch-committed
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-validation-skips-awaiting
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-validation-fails-to-run
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-worker-started-wrong-owner
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-queue-full-wrong-owner
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-queue-full-keeps-queued
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-valid-result-no-prepare
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-valid-result-no-accept
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-invalid-result-no-abort
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-invalid-result-no-reject
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-accept-without-valid
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-reject-without-invalid
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-stale-result-mutates
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-terminal-result-mutates
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-defer-committed-mutates
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-defer-keeps-running
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-timeout-before-due-recovers
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-timeout-due-no-recovery
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-timeout-protected-recovers
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-timeout-committed-recovers
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-timeout-aborted-recovers
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-backpressure-due-no-recovery
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-backpressure-protected-recovers
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-commit-not-sticky
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-commit-missing-progress
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-recovery-dispatches-worker
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-recovery-accepts
bash scripts/formal/sumeragi_apalache.sh vnext-slot-lifecycle-bug-recovery-rejects
bash scripts/formal/sumeragi_apalache.sh vnext-validation-bug-dispatch-queued
bash scripts/formal/sumeragi_apalache.sh vnext-validation-bug-raise-running-before-timeout
bash scripts/formal/sumeragi_apalache.sh vnext-validation-bug-miss-running-at-timeout
bash scripts/formal/sumeragi_apalache.sh vnext-validation-bug-backpressure-before-timeout-raises
bash scripts/formal/sumeragi_apalache.sh vnext-validation-bug-miss-backpressure-at-timeout
bash scripts/formal/sumeragi_apalache.sh vnext-validation-bug-accept-valid-as-await
bash scripts/formal/sumeragi_apalache.sh vnext-validation-bug-reject-invalid-as-await
bash scripts/formal/sumeragi_apalache.sh vnext-validation-bug-underflow-elapsed
bash scripts/formal/sumeragi_apalache.sh vnext-validation-bug-worker-started-keeps-queued
bash scripts/formal/sumeragi_apalache.sh vnext-validation-bug-apply-wrong-id
bash scripts/formal/sumeragi_apalache.sh vnext-validation-bug-apply-wrong-generation
bash scripts/formal/sumeragi_apalache.sh vnext-validation-bug-apply-not-running
bash scripts/formal/sumeragi_apalache.sh vnext-validation-bug-ignore-matching-valid
bash scripts/formal/sumeragi_apalache.sh vnext-validation-bug-ignore-matching-invalid
bash scripts/formal/sumeragi_apalache.sh vnext-validation-bug-stale-mutates-state
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-no-workers-deferred
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-no-workers-no-inline
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-duplicate-inflight-queues
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-duplicate-pending-queues
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-duplicate-not-dropped
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-send-success-no-inflight
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-send-success-applies
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-queue-full-drops
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-queue-full-adds-inflight
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-pending-no-workers-drops
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-pending-success-no-inflight
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-pending-success-keeps-pending
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-pending-queue-full-drops
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-disconnect-all-keeps-workers
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-disconnect-all-no-inline
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-poll-no-inflight-applies
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-poll-id-mismatch-applies
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-poll-id-mismatch-keeps-inflight
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-poll-stale-applies
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-poll-locked-applies
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-poll-penalized-applies
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-poll-invalid-applies
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-poll-invalid-not-rejected
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-poll-valid-no-apply
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-poll-channel-disconnect-keeps-rx
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-poll-channel-disconnect-keeps-workers
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-poll-channel-disconnect-keeps-pending
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-poll-no-rx-skips-dispatch
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-normal-poll-drops-rx
bash scripts/formal/sumeragi_apalache.sh vote-verify-async-bug-poll-drop-missing
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-cache-not-used
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-cache-deferred
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-consensus-no-workers-deferred
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-consensus-inline-deferred
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-consensus-inline-no-verify
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-consensus-inline-no-handler
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-consensus-send-no-inflight
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-consensus-send-applies
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-consensus-queue-full-deferred
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-consensus-queue-full-no-inline
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-consensus-disconnect-deferred
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-known-stale-applies
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-known-stale-dispatches
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-known-cached-deferred
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-known-cached-no-apply
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-known-inline-deferred
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-known-inline-no-verify
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-known-inline-no-apply
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-known-send-no-inflight
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-known-send-applies
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-known-queue-full-deferred
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-known-queue-full-drops
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-known-disconnect-deferred
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-duplicate-queues
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-duplicate-applies
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-duplicate-not-dropped
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-dispatch-disconnect-keeps-inflight
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-dispatch-disconnect-keeps-workers
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-dispatch-disconnect-keeps-rx
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-poll-no-inflight-applies
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-poll-id-mismatch-applies
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-poll-id-mismatch-keeps-inflight
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-poll-consensus-no-handler
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-poll-known-no-apply
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-poll-result-no-aggregate
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-poll-disconnect-keeps-inflight
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-poll-disconnect-keeps-workers
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-poll-disconnect-keeps-rx
bash scripts/formal/sumeragi_apalache.sh qc-verify-async-bug-normal-poll-drops-rx
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-idle-selects-message
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-select-payload-before-vote
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-ignore-vote-burst-limit
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-ignore-frontier-body-repair
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-frontier-body-chooses-wrong-tier
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-ignore-quorum-vote-priority
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-skip-overtime-payload-turn
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-block-urgent-ignored
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-starved-payload-ignored
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-starved-payload-not-suppressed-first
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-force-vote-over-payload-ignored
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-starved-block-overridden-by-vote
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-low-priority-starves
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-budget-zero-votes-selected
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-budget-exhausted-not-flagged
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-pre-tick-deadline-blocks-first-turn
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-post-tick-deadline-processes
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-selected-not-handled
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-spurious-handle-without-selection
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-missing-queue-drain-record
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-missing-budget-consume
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-no-phase-progress
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-skip-commit-result-poll
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-skip-validation-result-poll
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-skip-qc-result-poll
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-skip-vote-result-poll
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-skip-rbc-persist-poll
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-skip-sync-hints
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-tick-busy-due-not-run
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-tick-busy-uses-idle-gap
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-tick-bypass-ignored
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-time-budget-exceeded-not-flagged
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-post-tick-runs-after-budget-exceeded
bash scripts/formal/sumeragi_apalache.sh worker-drain-bug-post-tick-skips-without-budget-exceeded
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-inflight-allows-entry
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-body-ignores-critical-cap
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-critical-skips-body-burst
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-availability-ignores-urgent-cap
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-availability-ignores-da-cap
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-urgent-skips-availability-burst
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-urgent-starves-da-critical
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-da-critical-skips-availability-burst
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-da-critical-skips-urgent-cap
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-regular-skips-availability
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-regular-skips-da-critical
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-regular-skips-urgent-cap
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-entry-does-not-set-inflight
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-entry-does-not-decrement-waiter
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-body-does-not-increment-body-streak
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-body-does-not-increment-availability-streak
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-critical-does-not-reset-body-streak
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-critical-does-not-increment-availability-streak
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-urgent-does-not-increment-urgent-streak
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-urgent-does-not-reset-availability-streak
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-da-critical-keeps-urgent-streak
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-regular-keeps-urgent-streak
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-drop-keeps-inflight
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-drop-urgent-resets-urgent-streak
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-drop-non-urgent-keeps-urgent-streak
bash scripts/formal/sumeragi_apalache.sh actor-gate-bug-drop-skips-notify
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-worker-zero-window-not-floored
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-worker-small-window-not-floored
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-worker-mid-window-not-quartered
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-worker-large-window-ignores-global-cap
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-worker-config-cap-ignored
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-worker-uses-da-multiplier
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-vote-da-window-uses-commit-only
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-vote-da-multiplier-ignored
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-vote-max-budget-ignored
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-vote-config-cap-ignored
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-vote-zero-not-floored
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-drain-floor-ignored
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-drain-global-cap-ignored
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-vote-drain-floor-ignored
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-rbc-cap-ignored
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-idle-gap-floor-ignored
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-idle-gap-max-ignored
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-busy-gap-floor-ignored
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-busy-gap-exceeds-idle
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-block-zero-cap-nonzero
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-block-small-uses-medium
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-block-medium-uses-large
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-block-large-uses-huge
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-block-huge-clamped-large
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-vote-backlog-does-not-reduce-payload
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-vote-backlog-reduces-rbc
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-no-backlog-changes-caps
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-block-backlog-does-not-cap-blocks
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-block-backlog-payload-below-min
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-block-backlog-payload-not-scaled
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-block-backlog-rbc-below-min
bash scripts/formal/sumeragi_apalache.sh worker-budget-bug-block-backlog-rbc-not-scaled
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-route-vote-to-payload
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-route-qc-to-votes
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-route-rbc-to-payload
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-route-block-sync-to-payload
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-route-block-created-to-blocks
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-route-body-to-payload
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-route-consensus-to-background
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-route-lane-to-background
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-route-background-to-consensus
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-missing-metadata
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-missing-enqueue-record
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-accepted-records-drop
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-failed-missing-drop
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-blocking-missing-blocked-record
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-accepted-missing-wake
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-nonblocking-failure-wakes
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-votes-not-urgent
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-rbc-not-critical
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-blocks-not-critical
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-payload-not-body
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-control-not-urgent
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-background-not-regular
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-worker-wrong-stage
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-worker-wrong-handler
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-vote-batch-limit-one
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-rbc-batch-limit-one
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-batch-limit-zero-not-floored
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-batch-ignores-limit
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-batch-continues-on-empty
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-actor-without-gate
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-stage-after-handle
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-missing-poll-after-handle
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-missing-drain-record
bash scripts/formal/sumeragi_apalache.sh worker-ingress-bug-last-active-no-idle
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-accept-header-mismatch
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-accept-commitment-rewrite
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-accept-reveal-rewrite
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-accept-late-reveal-rewrite
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-accept-penalty-height-rewrite
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-allow-offender-overlap
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-keep-unfinalized-offenders
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-lose-finalized-state
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-lower-update-height
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-drop-existing-observation
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-skip-incoming-observation
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-drop-penalty-marker
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-allow-election-rewrite
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-drop-election
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-keep-equal-committed-pending
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-drop-committed-extension
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-damage-compatible-pending
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-keep-bad-pending
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-replace-with-bad-snapshot
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-insert-conflict-without-committed
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-committed-effect-keeps-covered
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-committed-effect-drops-progress
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-committed-effect-keeps-conflict
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-activation-empty-installs
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-activation-before-margin-applies
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-activation-at-margin-defers
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-accept-penalty-height-without-marker
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-accept-duplicate-participants
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-accept-duplicate-offenders
bash scripts/formal/sumeragi_apalache.sh npos-vrf-bug-accept-offender-out-of-roster
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-accept-no-kura-alignment
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-accept-missing-tip-alignment
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-accept-lower-height-alignment
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-accept-wrong-hash-alignment
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-reject-aligned-tip
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-backoff-finalizes
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-aborted-keeps-pending
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-aborted-skips-cleanup
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-already-durable-skips-mark
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-already-committed-keeps-pending
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-store-retry-drops-pending
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-store-retry-cleans-hash
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-store-exhausted-keeps-pending
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-store-exhausted-skips-cleanup
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-state-aligned-keeps-pending
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-state-aligned-cleans-block-hash
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-state-conflict-keeps-pending
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-state-conflict-skips-view-change
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-state-conflict-skips-requeue
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-state-other-drops-pending
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-state-other-forgets-kura-persisted
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-missing-qc-finalizes
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-before-tip-finalizes
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-aborted-without-qc-finalizes
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-aborted-with-qc-stays-aborted
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-retired-without-qc-finalizes
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-retired-with-qc-defers
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-mark-persisted-keeps-retry
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-reset-qc-drops-fallback
bash scripts/formal/sumeragi_apalache.sh kura-commit-bug-reset-qc-retains-stale
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-accept-bad-digest
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-accept-bad-signature
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-accept-bad-merkle
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-accept-wrong-chain
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-accept-ahead-snapshot
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-accept-missing-offline-keys
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-accept-missing-normal-block
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-accept-missing-hard-fork-hash
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-accept-interior-mismatch
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-reject-latest-revert
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-accept-hard-fork-mismatch
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-skip-legacy-manifest-replay
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-reject-empty-legacy-manifest
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-reject-complete-snapshot
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-reject-zero-height-write
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-accept-state-ahead-write
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-accept-latest-hash-mismatch-write
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-publish-without-atomic-tmp
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-canonical-keeps-commit-qc
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-canonical-keeps-consensus-evidence
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-canonical-keeps-vrf-epoch
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-canonical-keeps-topology
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-canonical-keeps-mv-history
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-canonical-key-policy-order-sensitive
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-canonical-drops-wsv-mutation
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-hard-fork-requires-body
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-normal-accepts-hash-only
bash scripts/formal/sumeragi_apalache.sh restart-replay-bug-accept-manifest-replay-failure
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-drop-undelivered-da-rbc
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-purge-settled-rbc-summary
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-retain-invalid-rbc
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-retain-without-da
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-drop-extending-descendant
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-skip-divergent-requeue
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-keep-unknown-parent
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-requeue-committed-duplicate
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-requeue-kura-duplicate
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-keep-stale-pending
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-keep-stale-validation
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-keep-stale-rbc
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-drop-committed-qc
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-keep-conflicting-qc
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-drop-proposals-seen
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-keep-committed-proposal-cache
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-skip-committed-missing-clear
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-skip-obsolete-missing-clear
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-clear-unavailable-nonobsolete
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-block-obsolete-without-payload
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-keep-committed-vote
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-drop-local-active-vote
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-drop-active-pending-vote
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-drop-new-view-window
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-keep-ancient-new-view
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-keep-committed-slot
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-keep-forced-view
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-skip-commit-recovery-clear
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-drop-canonical-frontier-evidence
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-keep-no-evidence-frontier
bash scripts/formal/sumeragi_apalache.sh post-commit-cleanup-bug-keep-validation-without-pending
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-no-future-evidence-requests
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-accept-same-height-future
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-skip-future-evidence-request
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-ignore-local-tip-payload
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-bypass-exact-owner
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-skip-exact-retry
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-deep-catchup-still-suppressed
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-canonical-uses-latest-anchor
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-noncanonical-uses-prev-anchor
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-missing-anchor-requests
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-skip-vote-roster
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-no-commit-topology-fallback
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-no-trusted-fallback
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-send-to-local-peer
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-unstable-target-order
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-empty-targets-request
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-ignore-cooldown
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-zero-sent-returns-success
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-skip-permit
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-skip-canonical-window-mark
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-repeat-already-emitted-window
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-ignore-stride
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-drop-aligned-stride
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-all-peer-cadence-skipped
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-cohort-uses-all-peers
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-ignore-recovery-fsm
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-skip-missing-qc-window-mark
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-low-priority-canonical
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-lock-lag-cooldown-not-extended
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-metric-not-incremented
bash scripts/formal/sumeragi_apalache.sh frontier-gap-realign-bug-drop-dependency-watermark
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-accept-empty-order
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-accept-zero-critical
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-accept-critical-after-end
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-accept-quarantine-before-critical
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-accept-quarantine-after-end
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-critical-path-includes-tail
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-critical-path-drops-last
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-successor-off-by-one
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-tail-has-successor
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-unknown-has-successor
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-quarantine-has-successor
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-count-prefix-off-by-one
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-count-prefix-accepts-impossible
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-stake-uses-non-strict
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-stake-missing-weight-accepted
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-stake-zero-total-accepted
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-bitmap-wrong-length-for-nine
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-bitmap-allows-duplicate
bash scripts/formal/sumeragi_apalache.sh vnext-chain-order-bug-bitmap-allows-out-of-range
bash scripts/formal/sumeragi_apalache.sh precommit-bug-invalid-validation
bash scripts/formal/sumeragi_apalache.sh precommit-bug-observer
bash scripts/formal/sumeragi_apalache.sh precommit-bug-duplicate
bash scripts/formal/sumeragi_apalache.sh precommit-bug-unsuperseded-conflict
bash scripts/formal/sumeragi_apalache.sh precommit-bug-older-quorum-completion
bash scripts/formal/sumeragi_apalache.sh precommit-bug-locked-conflict
bash scripts/formal/sumeragi_apalache.sh precommit-bug-missing-locked-payload
bash scripts/formal/sumeragi_apalache.sh precommit-bug-non-extending-lock
bash scripts/formal/sumeragi_apalache.sh precommit-bug-reject-safe
bash scripts/formal/sumeragi_apalache.sh proposal-bug-observer
bash scripts/formal/sumeragi_apalache.sh proposal-bug-active-vote-conflict
bash scripts/formal/sumeragi_apalache.sh proposal-bug-pending-vote-verification
bash scripts/formal/sumeragi_apalache.sh proposal-bug-missing-highest-qc
bash scripts/formal/sumeragi_apalache.sh proposal-bug-non-extending-highest
bash scripts/formal/sumeragi_apalache.sh proposal-bug-split-vote-lock
bash scripts/formal/sumeragi_apalache.sh proposal-bug-committed-edge-conflict
bash scripts/formal/sumeragi_apalache.sh proposal-bug-reject-safe
bash scripts/formal/sumeragi_apalache.sh proposal-bug-reject-stale-retired
bash scripts/formal/sumeragi_apalache.sh proposal-bug-reject-locked-fallback
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-skip-round-advance
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-skip-new-view-vote
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-skip-advance-output
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-wrong-phase
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-keep-validation
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-drop-pending-finality
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-use-zero-despite-highest
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-use-highest-without-highest
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-omit-highest-binding
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-bind-highest-without-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-subject-bug-use-zero-despite-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-subject-bug-use-invalid-despite-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-subject-bug-use-highest-without-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-subject-bug-tick-no-highest-uses-invalid
bash scripts/formal/sumeragi_apalache.sh engine-new-view-subject-bug-invalid-no-highest-uses-zero
bash scripts/formal/sumeragi_apalache.sh engine-new-view-subject-bug-parent-not-subject-hash
bash scripts/formal/sumeragi_apalache.sh engine-new-view-subject-bug-block-not-subject-hash
bash scripts/formal/sumeragi_apalache.sh engine-new-view-subject-bug-payload-not-zero
bash scripts/formal/sumeragi_apalache.sh engine-new-view-subject-bug-omit-highest-binding
bash scripts/formal/sumeragi_apalache.sh engine-new-view-subject-bug-bind-highest-without-highest
bash scripts/formal/sumeragi_apalache.sh engine-handle-dispatch-bug-drop-tick
bash scripts/formal/sumeragi_apalache.sh engine-handle-dispatch-bug-drop-proposal
bash scripts/formal/sumeragi_apalache.sh engine-handle-dispatch-bug-drop-certificate
bash scripts/formal/sumeragi_apalache.sh engine-handle-dispatch-bug-drop-payload
bash scripts/formal/sumeragi_apalache.sh engine-handle-dispatch-bug-drop-validation
bash scripts/formal/sumeragi_apalache.sh engine-handle-dispatch-bug-drop-committed
bash scripts/formal/sumeragi_apalache.sh engine-handle-dispatch-bug-tick-as-proposal
bash scripts/formal/sumeragi_apalache.sh engine-handle-dispatch-bug-proposal-as-tick
bash scripts/formal/sumeragi_apalache.sh engine-handle-dispatch-bug-certificate-as-payload
bash scripts/formal/sumeragi_apalache.sh engine-handle-dispatch-bug-payload-as-certificate
bash scripts/formal/sumeragi_apalache.sh engine-handle-dispatch-bug-validation-as-committed
bash scripts/formal/sumeragi_apalache.sh engine-handle-dispatch-bug-committed-as-validation
bash scripts/formal/sumeragi_apalache.sh engine-handle-dispatch-bug-dispatch-twice
bash scripts/formal/sumeragi_apalache.sh engine-certificate-dispatch-bug-dispatch-committed-height
bash scripts/formal/sumeragi_apalache.sh engine-certificate-dispatch-bug-dispatch-wrong-height
bash scripts/formal/sumeragi_apalache.sh engine-certificate-dispatch-bug-dispatch-wrong-epoch
bash scripts/formal/sumeragi_apalache.sh engine-certificate-dispatch-bug-dispatch-wrong-validator-set
bash scripts/formal/sumeragi_apalache.sh engine-certificate-dispatch-bug-dispatch-wrong-quorum-policy
bash scripts/formal/sumeragi_apalache.sh engine-certificate-dispatch-bug-dispatch-stale-prepare-commit
bash scripts/formal/sumeragi_apalache.sh engine-certificate-dispatch-bug-reject-safe-prepare
bash scripts/formal/sumeragi_apalache.sh engine-certificate-dispatch-bug-reject-safe-commit
bash scripts/formal/sumeragi_apalache.sh engine-certificate-dispatch-bug-reject-new-view-same-or-past-at-prefilter
bash scripts/formal/sumeragi_apalache.sh engine-certificate-dispatch-bug-reject-new-view-future-at-prefilter
bash scripts/formal/sumeragi_apalache.sh engine-certificate-dispatch-bug-dispatch-prepare-as-commit
bash scripts/formal/sumeragi_apalache.sh engine-certificate-dispatch-bug-dispatch-commit-as-prepare
bash scripts/formal/sumeragi_apalache.sh engine-certificate-dispatch-bug-dispatch-new-view-as-prepare
bash scripts/formal/sumeragi_apalache.sh engine-certificate-prefilter-state-bug-accepted-drops-dispatch
bash scripts/formal/sumeragi_apalache.sh engine-certificate-prefilter-state-bug-accepted-mutates-phase
bash scripts/formal/sumeragi_apalache.sh engine-certificate-prefilter-state-bug-accepted-updates-round
bash scripts/formal/sumeragi_apalache.sh engine-certificate-prefilter-state-bug-accepted-clears-lock
bash scripts/formal/sumeragi_apalache.sh engine-certificate-prefilter-state-bug-accepted-records-highest
bash scripts/formal/sumeragi_apalache.sh engine-certificate-prefilter-state-bug-accepted-clears-pending
bash scripts/formal/sumeragi_apalache.sh engine-certificate-prefilter-state-bug-accepted-clears-validation
bash scripts/formal/sumeragi_apalache.sh engine-certificate-prefilter-state-bug-rejected-mutates-phase
bash scripts/formal/sumeragi_apalache.sh engine-certificate-prefilter-state-bug-rejected-updates-round
bash scripts/formal/sumeragi_apalache.sh engine-certificate-prefilter-state-bug-rejected-clears-lock
bash scripts/formal/sumeragi_apalache.sh engine-certificate-prefilter-state-bug-rejected-records-highest
bash scripts/formal/sumeragi_apalache.sh engine-certificate-prefilter-state-bug-rejected-clears-pending
bash scripts/formal/sumeragi_apalache.sh engine-certificate-prefilter-state-bug-rejected-clears-validation
bash scripts/formal/sumeragi_apalache.sh engine-certificate-prefilter-state-bug-rejected-emits-output
bash scripts/formal/sumeragi_apalache.sh engine-view-advance-saturation-bug-tick-wrap-at-max
bash scripts/formal/sumeragi_apalache.sh engine-view-advance-saturation-bug-tick-stays-before-max
bash scripts/formal/sumeragi_apalache.sh engine-view-advance-saturation-bug-invalid-wrap-at-max
bash scripts/formal/sumeragi_apalache.sh engine-view-advance-saturation-bug-invalid-stays-before-max
bash scripts/formal/sumeragi_apalache.sh engine-view-advance-saturation-bug-valid-advances
bash scripts/formal/sumeragi_apalache.sh engine-view-advance-saturation-bug-stale-validation-advances
bash scripts/formal/sumeragi_apalache.sh engine-view-advance-saturation-bug-no-inflight-advances
bash scripts/formal/sumeragi_apalache.sh engine-view-advance-saturation-bug-output-uses-old-view
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-accept-wrong-context
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-accept-wrong-quorum
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-accept-stale-view
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-accept-incompatible-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-reject-safe-no-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-reject-safe-improving-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-reject-safe-lower-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-skip-advance-output
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-wrong-phase
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-keep-validation
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-drop-pending-finality
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-overwrite-lower-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-skip-highest-record
bash scripts/formal/sumeragi_apalache.sh engine-new-view-highest-qc-bug-skip-no-current-record
bash scripts/formal/sumeragi_apalache.sh engine-new-view-highest-qc-bug-skip-improving-record
bash scripts/formal/sumeragi_apalache.sh engine-new-view-highest-qc-bug-record-wrong-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-highest-qc-bug-overwrite-lower-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-highest-qc-bug-clear-on-no-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-highest-qc-bug-record-without-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-highest-qc-bug-clear-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-new-view-highest-qc-bug-record-on-stale
bash scripts/formal/sumeragi_apalache.sh engine-new-view-highest-qc-bug-record-on-incompatible
bash scripts/formal/sumeragi_apalache.sh engine-new-view-highest-qc-bug-record-on-wrong-context
bash scripts/formal/sumeragi_apalache.sh engine-new-view-highest-qc-bug-record-on-wrong-quorum
bash scripts/formal/sumeragi_apalache.sh engine-new-view-advance-bug-skip-round-update
bash scripts/formal/sumeragi_apalache.sh engine-new-view-advance-bug-round-wrong-height
bash scripts/formal/sumeragi_apalache.sh engine-new-view-advance-bug-round-wrong-view
bash scripts/formal/sumeragi_apalache.sh engine-new-view-advance-bug-round-wrong-epoch
bash scripts/formal/sumeragi_apalache.sh engine-new-view-advance-bug-round-wrong-validator-set
bash scripts/formal/sumeragi_apalache.sh engine-new-view-advance-bug-skip-advance-output
bash scripts/formal/sumeragi_apalache.sh engine-new-view-advance-bug-output-wrong-height
bash scripts/formal/sumeragi_apalache.sh engine-new-view-advance-bug-output-wrong-view
bash scripts/formal/sumeragi_apalache.sh engine-new-view-advance-bug-output-wrong-epoch
bash scripts/formal/sumeragi_apalache.sh engine-new-view-advance-bug-output-wrong-validator-set
bash scripts/formal/sumeragi_apalache.sh engine-new-view-advance-bug-keep-validation-inflight
bash scripts/formal/sumeragi_apalache.sh engine-new-view-advance-bug-wrong-phase-after-accept
bash scripts/formal/sumeragi_apalache.sh engine-new-view-advance-bug-drop-pending-finality
bash scripts/formal/sumeragi_apalache.sh engine-new-view-advance-bug-round-update-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-new-view-advance-bug-output-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-wrong-phase
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-wrong-round
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-incompatible-highest
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-locked-conflict-no-qc
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-locked-conflict-equal-qc
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-locked-conflict-lower-qc
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-reject-unlocked
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-reject-locked-subject
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-reject-higher-qc
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-skip-validation
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-skip-prepare-vote
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-skip-prepare-phase
bash scripts/formal/sumeragi_apalache.sh engine-proposal-output-bug-skip-validate-output
bash scripts/formal/sumeragi_apalache.sh engine-proposal-output-bug-skip-prepare-vote-output
bash scripts/formal/sumeragi_apalache.sh engine-proposal-output-bug-swap-output-order
bash scripts/formal/sumeragi_apalache.sh engine-proposal-output-bug-validate-wrong-subject
bash scripts/formal/sumeragi_apalache.sh engine-proposal-output-bug-vote-wrong-phase
bash scripts/formal/sumeragi_apalache.sh engine-proposal-output-bug-vote-wrong-round
bash scripts/formal/sumeragi_apalache.sh engine-proposal-output-bug-vote-wrong-subject
bash scripts/formal/sumeragi_apalache.sh engine-proposal-output-bug-vote-carries-highest-qc
bash scripts/formal/sumeragi_apalache.sh engine-proposal-output-bug-emit-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-proposal-state-bug-accepted-stays-proposal
bash scripts/formal/sumeragi_apalache.sh engine-proposal-state-bug-accepted-uses-commit-phase
bash scripts/formal/sumeragi_apalache.sh engine-proposal-state-bug-accepted-updates-round
bash scripts/formal/sumeragi_apalache.sh engine-proposal-state-bug-accepted-clears-lock
bash scripts/formal/sumeragi_apalache.sh engine-proposal-state-bug-accepted-records-proposal-highest
bash scripts/formal/sumeragi_apalache.sh engine-proposal-state-bug-accepted-clears-pending
bash scripts/formal/sumeragi_apalache.sh engine-proposal-state-bug-rejected-enters-prepare
bash scripts/formal/sumeragi_apalache.sh engine-proposal-state-bug-rejected-updates-round
bash scripts/formal/sumeragi_apalache.sh engine-proposal-state-bug-rejected-clears-lock
bash scripts/formal/sumeragi_apalache.sh engine-proposal-state-bug-rejected-records-proposal-highest
bash scripts/formal/sumeragi_apalache.sh engine-proposal-state-bug-rejected-clears-pending
bash scripts/formal/sumeragi_apalache.sh engine-proposal-validation-owner-bug-skip-owner-record
bash scripts/formal/sumeragi_apalache.sh engine-proposal-validation-owner-bug-keep-existing-owner
bash scripts/formal/sumeragi_apalache.sh engine-proposal-validation-owner-bug-record-wrong-subject
bash scripts/formal/sumeragi_apalache.sh engine-proposal-validation-owner-bug-record-locked-subject
bash scripts/formal/sumeragi_apalache.sh engine-proposal-validation-owner-bug-clear-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-proposal-validation-owner-bug-replace-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-proposal-validation-owner-bug-set-owner-on-rejected-none
bash scripts/formal/sumeragi_apalache.sh engine-proposal-lock-bug-require-qc-when-unlocked
bash scripts/formal/sumeragi_apalache.sh engine-proposal-lock-bug-reject-locked-subject
bash scripts/formal/sumeragi_apalache.sh engine-proposal-lock-bug-ignore-subject-match
bash scripts/formal/sumeragi_apalache.sh engine-proposal-lock-bug-accept-conflict-without-qc
bash scripts/formal/sumeragi_apalache.sh engine-proposal-lock-bug-accept-equal-qc
bash scripts/formal/sumeragi_apalache.sh engine-proposal-lock-bug-use-non-strict-qc-comparison
bash scripts/formal/sumeragi_apalache.sh engine-proposal-lock-bug-accept-lower-qc
bash scripts/formal/sumeragi_apalache.sh engine-proposal-lock-bug-reject-higher-qc
bash scripts/formal/sumeragi_apalache.sh qc-round-compatibility-bug-ignore-epoch
bash scripts/formal/sumeragi_apalache.sh qc-round-compatibility-bug-reject-lower-height
bash scripts/formal/sumeragi_apalache.sh qc-round-compatibility-bug-require-view-for-lower-height
bash scripts/formal/sumeragi_apalache.sh qc-round-compatibility-bug-accept-same-height-future-view
bash scripts/formal/sumeragi_apalache.sh qc-round-compatibility-bug-accept-future-height
bash scripts/formal/sumeragi_apalache.sh qc-round-compatibility-bug-reject-same-height-past-view
bash scripts/formal/sumeragi_apalache.sh qc-round-compatibility-bug-reject-same-height-equal-view
bash scripts/formal/sumeragi_apalache.sh qc-round-compatibility-bug-use-view-only-comparison
bash scripts/formal/sumeragi_apalache.sh engine-qc-ref-projection-bug-drop-height
bash scripts/formal/sumeragi_apalache.sh engine-qc-ref-projection-bug-advance-height
bash scripts/formal/sumeragi_apalache.sh engine-qc-ref-projection-bug-drop-view
bash scripts/formal/sumeragi_apalache.sh engine-qc-ref-projection-bug-drop-epoch
bash scripts/formal/sumeragi_apalache.sh engine-qc-ref-projection-bug-use-parent-subject
bash scripts/formal/sumeragi_apalache.sh engine-qc-ref-projection-bug-zero-subject
bash scripts/formal/sumeragi_apalache.sh engine-qc-ref-projection-bug-force-prepare-phase
bash scripts/formal/sumeragi_apalache.sh engine-qc-ref-projection-bug-force-commit-phase
bash scripts/formal/sumeragi_apalache.sh engine-qc-ref-projection-bug-force-new-view-phase
bash scripts/formal/sumeragi_apalache.sh engine-highest-qc-record-bug-skip-no-current
bash scripts/formal/sumeragi_apalache.sh engine-highest-qc-record-bug-reject-higher-height
bash scripts/formal/sumeragi_apalache.sh engine-highest-qc-record-bug-accept-lower-height
bash scripts/formal/sumeragi_apalache.sh engine-highest-qc-record-bug-use-view-before-height
bash scripts/formal/sumeragi_apalache.sh engine-highest-qc-record-bug-reject-higher-view
bash scripts/formal/sumeragi_apalache.sh engine-highest-qc-record-bug-accept-lower-view
bash scripts/formal/sumeragi_apalache.sh engine-highest-qc-record-bug-ignore-phase-rank
bash scripts/formal/sumeragi_apalache.sh engine-highest-qc-record-bug-reject-higher-phase
bash scripts/formal/sumeragi_apalache.sh engine-highest-qc-record-bug-accept-lower-phase
bash scripts/formal/sumeragi_apalache.sh engine-highest-qc-record-bug-ignore-subject-tie
bash scripts/formal/sumeragi_apalache.sh engine-highest-qc-record-bug-reject-higher-subject
bash scripts/formal/sumeragi_apalache.sh engine-highest-qc-record-bug-accept-lower-subject
bash scripts/formal/sumeragi_apalache.sh engine-highest-qc-record-bug-overwrite-equal
bash scripts/formal/sumeragi_apalache.sh engine-commit-subject-bug-skip-fresh-record
bash scripts/formal/sumeragi_apalache.sh engine-commit-subject-bug-reject-matching-committed
bash scripts/formal/sumeragi_apalache.sh engine-commit-subject-bug-keep-pending-finality
bash scripts/formal/sumeragi_apalache.sh engine-commit-subject-bug-keep-validation
bash scripts/formal/sumeragi_apalache.sh engine-commit-subject-bug-wrong-phase-after-commit
bash scripts/formal/sumeragi_apalache.sh engine-commit-subject-bug-skip-commit-output
bash scripts/formal/sumeragi_apalache.sh engine-commit-subject-bug-overwrite-conflict
bash scripts/formal/sumeragi_apalache.sh engine-commit-subject-bug-emit-on-conflict
bash scripts/formal/sumeragi_apalache.sh engine-commit-subject-bug-clear-pending-on-conflict
bash scripts/formal/sumeragi_apalache.sh engine-commit-subject-bug-clear-validation-on-conflict
bash scripts/formal/sumeragi_apalache.sh engine-payload-lookup-bug-ignore-block-hash
bash scripts/formal/sumeragi_apalache.sh engine-payload-lookup-bug-ignore-payload-hash
bash scripts/formal/sumeragi_apalache.sh engine-payload-lookup-bug-accept-any-recorded-payload
bash scripts/formal/sumeragi_apalache.sh engine-payload-lookup-bug-accept-empty-store
bash scripts/formal/sumeragi_apalache.sh engine-payload-lookup-bug-reject-exact-pair
bash scripts/formal/sumeragi_apalache.sh engine-payload-lookup-bug-invert-lookup
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-wrong-context
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-wrong-quorum-policy
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-stale-view
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-committed-height
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-replay-prepare
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-conflicting-prepare
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-pending-finality
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-reject-safe
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-missing-lock-record
bash scripts/formal/sumeragi_apalache.sh engine-prepare-lock-highest-bug-skip-lock-record
bash scripts/formal/sumeragi_apalache.sh engine-prepare-lock-highest-bug-record-wrong-lock
bash scripts/formal/sumeragi_apalache.sh engine-prepare-lock-highest-bug-lock-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-prepare-lock-highest-bug-clear-lock-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-prepare-lock-highest-bug-lock-on-replay-conflict-pending
bash scripts/formal/sumeragi_apalache.sh engine-prepare-lock-highest-bug-clear-lock-on-replay-conflict-pending
bash scripts/formal/sumeragi_apalache.sh engine-prepare-lock-highest-bug-skip-no-current-highest
bash scripts/formal/sumeragi_apalache.sh engine-prepare-lock-highest-bug-skip-improving-highest
bash scripts/formal/sumeragi_apalache.sh engine-prepare-lock-highest-bug-record-wrong-highest
bash scripts/formal/sumeragi_apalache.sh engine-prepare-lock-highest-bug-overwrite-lower-highest
bash scripts/formal/sumeragi_apalache.sh engine-prepare-lock-highest-bug-record-highest-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-prepare-lock-highest-bug-clear-highest-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-prepare-lock-highest-bug-record-highest-on-replay-conflict-pending
bash scripts/formal/sumeragi_apalache.sh engine-prepare-lock-highest-bug-clear-highest-on-replay-conflict-pending
bash scripts/formal/sumeragi_apalache.sh engine-prepare-phase-bug-skip-commit-phase
bash scripts/formal/sumeragi_apalache.sh engine-prepare-phase-bug-wrong-accepted-phase
bash scripts/formal/sumeragi_apalache.sh engine-prepare-phase-bug-commit-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-prepare-phase-bug-wrong-phase-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-prepare-phase-bug-commit-on-replay-conflict
bash scripts/formal/sumeragi_apalache.sh engine-prepare-phase-bug-wrong-phase-on-replay-conflict
bash scripts/formal/sumeragi_apalache.sh engine-prepare-phase-bug-commit-on-pending-finality
bash scripts/formal/sumeragi_apalache.sh engine-prepare-phase-bug-wrong-phase-on-pending-finality
bash scripts/formal/sumeragi_apalache.sh engine-prepare-vote-cache-bug-skip-cache-insert
bash scripts/formal/sumeragi_apalache.sh engine-prepare-vote-cache-bug-cache-wrong-round
bash scripts/formal/sumeragi_apalache.sh engine-prepare-vote-cache-bug-cache-wrong-subject
bash scripts/formal/sumeragi_apalache.sh engine-prepare-vote-cache-bug-skip-commit-vote-output
bash scripts/formal/sumeragi_apalache.sh engine-prepare-vote-cache-bug-output-wrong-phase
bash scripts/formal/sumeragi_apalache.sh engine-prepare-vote-cache-bug-output-wrong-round
bash scripts/formal/sumeragi_apalache.sh engine-prepare-vote-cache-bug-output-wrong-subject
bash scripts/formal/sumeragi_apalache.sh engine-prepare-vote-cache-bug-output-carries-highest-qc
bash scripts/formal/sumeragi_apalache.sh engine-prepare-vote-cache-bug-cache-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-prepare-vote-cache-bug-output-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-prepare-vote-cache-bug-overwrite-conflict
bash scripts/formal/sumeragi_apalache.sh engine-prepare-vote-cache-bug-output-replay-conflict
bash scripts/formal/sumeragi_apalache.sh engine-prepare-vote-cache-bug-clear-existing-on-replay-conflict
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-wrong-context
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-wrong-quorum-policy
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-stale-view
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-committed-height
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-pending-replay
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-pending-conflict
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-commit-without-payload
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-fetch-despite-payload
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-reject-available
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-reject-missing-payload
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-missing-highest-record
bash scripts/formal/sumeragi_apalache.sh engine-commit-highest-qc-bug-skip-no-current-record
bash scripts/formal/sumeragi_apalache.sh engine-commit-highest-qc-bug-skip-improving-record
bash scripts/formal/sumeragi_apalache.sh engine-commit-highest-qc-bug-record-wrong-highest
bash scripts/formal/sumeragi_apalache.sh engine-commit-highest-qc-bug-overwrite-lower-highest
bash scripts/formal/sumeragi_apalache.sh engine-commit-highest-qc-bug-record-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-commit-highest-qc-bug-clear-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-commit-highest-qc-bug-record-on-pending-replay
bash scripts/formal/sumeragi_apalache.sh engine-commit-highest-qc-bug-record-on-pending-conflict
bash scripts/formal/sumeragi_apalache.sh engine-commit-highest-qc-bug-clear-on-pending-replay-conflict
bash scripts/formal/sumeragi_apalache.sh engine-commit-available-commit-bug-skip-commit-record
bash scripts/formal/sumeragi_apalache.sh engine-commit-available-commit-bug-commit-wrong-height
bash scripts/formal/sumeragi_apalache.sh engine-commit-available-commit-bug-commit-wrong-block
bash scripts/formal/sumeragi_apalache.sh engine-commit-available-commit-bug-keep-validation-after-commit
bash scripts/formal/sumeragi_apalache.sh engine-commit-available-commit-bug-wrong-phase-after-commit
bash scripts/formal/sumeragi_apalache.sh engine-commit-available-commit-bug-skip-commit-block-output
bash scripts/formal/sumeragi_apalache.sh engine-commit-available-commit-bug-output-wrong-parent
bash scripts/formal/sumeragi_apalache.sh engine-commit-available-commit-bug-output-wrong-block
bash scripts/formal/sumeragi_apalache.sh engine-commit-available-commit-bug-output-wrong-payload
bash scripts/formal/sumeragi_apalache.sh engine-commit-available-commit-bug-fetch-despite-payload-available
bash scripts/formal/sumeragi_apalache.sh engine-commit-available-commit-bug-pending-despite-payload-available
bash scripts/formal/sumeragi_apalache.sh engine-commit-available-commit-bug-commit-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-commit-available-commit-bug-commit-on-replay-conflict
bash scripts/formal/sumeragi_apalache.sh engine-commit-available-commit-bug-overwrite-committed-height
bash scripts/formal/sumeragi_apalache.sh engine-commit-pending-fetch-bug-skip-pending-state
bash scripts/formal/sumeragi_apalache.sh engine-commit-pending-fetch-bug-skip-pending-map-insert
bash scripts/formal/sumeragi_apalache.sh engine-commit-pending-fetch-bug-pending-map-key-uses-payload-hash
bash scripts/formal/sumeragi_apalache.sh engine-commit-pending-fetch-bug-pending-map-key-uses-parent-hash
bash scripts/formal/sumeragi_apalache.sh engine-commit-pending-fetch-bug-pending-map-stores-wrong-certificate
bash scripts/formal/sumeragi_apalache.sh engine-commit-pending-fetch-bug-skip-fetch-request
bash scripts/formal/sumeragi_apalache.sh engine-commit-pending-fetch-bug-fetch-wrong-round
bash scripts/formal/sumeragi_apalache.sh engine-commit-pending-fetch-bug-fetch-wrong-block-hash
bash scripts/formal/sumeragi_apalache.sh engine-commit-pending-fetch-bug-fetch-wrong-payload-hash
bash scripts/formal/sumeragi_apalache.sh engine-commit-pending-fetch-bug-pending-on-payload-available
bash scripts/formal/sumeragi_apalache.sh engine-commit-pending-fetch-bug-fetch-on-payload-available
bash scripts/formal/sumeragi_apalache.sh engine-commit-pending-fetch-bug-pending-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-commit-pending-fetch-bug-fetch-on-rejected
bash scripts/formal/sumeragi_apalache.sh engine-commit-pending-fetch-bug-pending-on-replay-conflict
bash scripts/formal/sumeragi_apalache.sh engine-commit-pending-fetch-bug-fetch-on-replay-conflict
bash scripts/formal/sumeragi_apalache.sh engine-commit-validation-cleanup-bug-skip-accepted-validation-clear
bash scripts/formal/sumeragi_apalache.sh engine-commit-validation-cleanup-bug-skip-pending-replay-validation-clear
bash scripts/formal/sumeragi_apalache.sh engine-commit-validation-cleanup-bug-skip-pending-conflict-validation-clear
bash scripts/formal/sumeragi_apalache.sh engine-commit-validation-cleanup-bug-clear-wrong-context-validation
bash scripts/formal/sumeragi_apalache.sh engine-commit-validation-cleanup-bug-clear-wrong-quorum-policy-validation
bash scripts/formal/sumeragi_apalache.sh engine-commit-validation-cleanup-bug-clear-stale-view-validation
bash scripts/formal/sumeragi_apalache.sh engine-commit-validation-cleanup-bug-clear-committed-height-validation
bash scripts/formal/sumeragi_apalache.sh engine-commit-validation-cleanup-bug-late-invalid-advances-after-commit-qc
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-skip-fresh-record
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-reject-boundary-activation
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-activate-without-boundary
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-activate-non-boundary
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-record-duplicate
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-activate-duplicate
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-record-conflict
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-activate-conflict
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-overwrite-conflict
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-record-bug-skip-fresh-record
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-record-bug-record-wrong-height
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-record-bug-record-wrong-block
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-record-bug-clear-unrelated-entry
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-record-bug-overwrite-unrelated-entry
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-record-bug-overwrite-duplicate
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-record-bug-clear-duplicate
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-record-bug-duplicate-records-wrong-height
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-record-bug-overwrite-conflict
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-record-bug-clear-existing-on-conflict
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-record-bug-conflict-records-wrong-height
bash scripts/formal/sumeragi_apalache.sh engine-reconfiguration-staging-bug-skip-boundary-staging
bash scripts/formal/sumeragi_apalache.sh engine-reconfiguration-staging-bug-skip-boundary-activation
bash scripts/formal/sumeragi_apalache.sh engine-reconfiguration-staging-bug-stage-without-boundary
bash scripts/formal/sumeragi_apalache.sh engine-reconfiguration-staging-bug-activate-without-boundary
bash scripts/formal/sumeragi_apalache.sh engine-reconfiguration-staging-bug-stage-non-boundary
bash scripts/formal/sumeragi_apalache.sh engine-reconfiguration-staging-bug-activate-non-boundary
bash scripts/formal/sumeragi_apalache.sh engine-reconfiguration-staging-bug-stage-duplicate
bash scripts/formal/sumeragi_apalache.sh engine-reconfiguration-staging-bug-activate-duplicate
bash scripts/formal/sumeragi_apalache.sh engine-reconfiguration-staging-bug-stage-conflict
bash scripts/formal/sumeragi_apalache.sh engine-reconfiguration-staging-bug-activate-conflict
bash scripts/formal/sumeragi_apalache.sh engine-reconfiguration-staging-bug-stage-wrong-change
bash scripts/formal/sumeragi_apalache.sh engine-reconfiguration-staging-bug-emit-wrong-change
bash scripts/formal/sumeragi_apalache.sh engine-reconfiguration-staging-bug-preserve-old-on-boundary
bash scripts/formal/sumeragi_apalache.sh engine-reconfiguration-staging-bug-clear-existing-on-noop
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-cleanup-bug-skip-fresh-record
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-cleanup-bug-skip-current-validation-clear
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-cleanup-bug-skip-current-pending-clear
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-cleanup-bug-skip-current-pending-map-remove
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-cleanup-bug-wrong-phase-after-current-commit
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-cleanup-bug-cleanup-other-height
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-cleanup-bug-duplicate-cleans-validation
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-cleanup-bug-duplicate-clears-pending
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-cleanup-bug-conflict-cleans-validation
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-cleanup-bug-conflict-clears-pending
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-cleanup-bug-emit-commit-block
bash scripts/formal/sumeragi_apalache.sh engine-payload-record-bug-skip-record
bash scripts/formal/sumeragi_apalache.sh engine-payload-record-bug-record-only-when-pending
bash scripts/formal/sumeragi_apalache.sh engine-payload-record-bug-record-only-on-match
bash scripts/formal/sumeragi_apalache.sh engine-payload-record-bug-record-wrong-block
bash scripts/formal/sumeragi_apalache.sh engine-payload-record-bug-record-wrong-payload
bash scripts/formal/sumeragi_apalache.sh engine-payload-record-bug-record-parent-as-block
bash scripts/formal/sumeragi_apalache.sh engine-payload-record-bug-record-pending-subject-instead
bash scripts/formal/sumeragi_apalache.sh engine-payload-record-bug-clear-existing-availability
bash scripts/formal/sumeragi_apalache.sh engine-payload-record-bug-drop-unrelated-availability
bash scripts/formal/sumeragi_apalache.sh engine-payload-bug-skip-available-record
bash scripts/formal/sumeragi_apalache.sh engine-payload-bug-commit-without-pending
bash scripts/formal/sumeragi_apalache.sh engine-payload-bug-commit-mismatched-payload
bash scripts/formal/sumeragi_apalache.sh engine-payload-bug-drop-pending-on-mismatch
bash scripts/formal/sumeragi_apalache.sh engine-payload-bug-reject-matching-payload
bash scripts/formal/sumeragi_apalache.sh engine-payload-bug-keep-pending-after-commit
bash scripts/formal/sumeragi_apalache.sh engine-payload-bug-wrong-phase-after-commit
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-accept-wrong-round
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-accept-wrong-block-hash
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-accept-no-inflight
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-accept-superseded
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-reject-current-valid
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-reject-current-invalid
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-keep-validation
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-valid-emits-output
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-skip-round-advance
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-skip-new-view-vote
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-skip-advance-output
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-wrong-phase
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-use-invalid-subject-despite-highest
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-use-highest-without-highest
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-omit-highest-binding
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-bind-highest-without-highest
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-drop-pending-finality
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-overwrite-committed
bash scripts/formal/sumeragi_apalache.sh engine-validation-ownership-bug-keep-valid-owner
bash scripts/formal/sumeragi_apalache.sh engine-validation-ownership-bug-keep-invalid-owner
bash scripts/formal/sumeragi_apalache.sh engine-validation-ownership-bug-clear-wrong-round-owner
bash scripts/formal/sumeragi_apalache.sh engine-validation-ownership-bug-clear-wrong-block-owner
bash scripts/formal/sumeragi_apalache.sh engine-validation-ownership-bug-replace-wrong-round-owner
bash scripts/formal/sumeragi_apalache.sh engine-validation-ownership-bug-replace-wrong-block-owner
bash scripts/formal/sumeragi_apalache.sh engine-validation-ownership-bug-set-owner-on-no-inflight
bash scripts/formal/sumeragi_apalache.sh engine-validation-ownership-bug-set-owner-on-replay
bash scripts/formal/sumeragi_apalache.sh engine-validation-ownership-bug-set-owner-on-superseded
bash scripts/formal/sumeragi_apalache.sh engine-validation-invalid-advance-bug-skip-state-advance
bash scripts/formal/sumeragi_apalache.sh engine-validation-invalid-advance-bug-advance-wrong-height
bash scripts/formal/sumeragi_apalache.sh engine-validation-invalid-advance-bug-advance-wrong-epoch
bash scripts/formal/sumeragi_apalache.sh engine-validation-invalid-advance-bug-advance-wrong-validator-set
bash scripts/formal/sumeragi_apalache.sh engine-validation-invalid-advance-bug-wrap-max-view
bash scripts/formal/sumeragi_apalache.sh engine-validation-invalid-advance-bug-skip-sign-vote
bash scripts/formal/sumeragi_apalache.sh engine-validation-invalid-advance-bug-skip-advance-output
bash scripts/formal/sumeragi_apalache.sh engine-validation-invalid-advance-bug-sign-old-round
bash scripts/formal/sumeragi_apalache.sh engine-validation-invalid-advance-bug-advance-output-old-round
bash scripts/formal/sumeragi_apalache.sh engine-validation-invalid-advance-bug-output-rounds-mismatch
bash scripts/formal/sumeragi_apalache.sh engine-validation-invalid-advance-bug-advance-on-valid
bash scripts/formal/sumeragi_apalache.sh engine-validation-invalid-advance-bug-advance-on-ignored
bash scripts/formal/sumeragi_apalache.sh reconfig-bug-premature-activation
bash scripts/formal/sumeragi_apalache.sh reconfig-bug-premature-new-cert
bash scripts/formal/sumeragi_apalache.sh reconfig-bug-mixed-cert
bash scripts/formal/sumeragi_apalache.sh recovery-bug-commit-without-payload
bash scripts/formal/sumeragi_apalache.sh recovery-bug-mismatched-payload
bash scripts/formal/sumeragi_apalache.sh recovery-bug-conflicting-finality
bash scripts/formal/sumeragi_apalache.sh view-change-bug-stale-new-view
bash scripts/formal/sumeragi_apalache.sh view-change-bug-unsafe-proposal
bash scripts/formal/sumeragi_apalache.sh view-change-bug-lock-overwrite
bash scripts/formal/sumeragi_apalache.sh view-change-bug-highest-regression
bash scripts/formal/sumeragi_apalache.sh validation-bug-unknown-result
bash scripts/formal/sumeragi_apalache.sh validation-bug-completed-replay
bash scripts/formal/sumeragi_apalache.sh validation-bug-timeout-inflight
bash scripts/formal/sumeragi_apalache.sh validation-bug-invalid-replay
bash scripts/formal/sumeragi_apalache.sh admission-bug-wrong-context
bash scripts/formal/sumeragi_apalache.sh admission-bug-stale-prepare-commit
bash scripts/formal/sumeragi_apalache.sh admission-bug-future-height
bash scripts/formal/sumeragi_apalache.sh admission-bug-committed-height
bash scripts/formal/sumeragi_apalache.sh highest-bug-height-priority
bash scripts/formal/sumeragi_apalache.sh highest-bug-phase-rank
bash scripts/formal/sumeragi_apalache.sh highest-bug-subject-tie
bash scripts/formal/sumeragi_apalache.sh highest-bug-non-new-view
```

If Apalache is not in `PATH`, you can:

- set `APALACHE_BIN` to the executable path, or
- use the Docker fallback (enabled by default when `docker` is available):
  - image: `APALACHE_DOCKER_IMAGE` (default `ghcr.io/apalache-mc/apalache:0.52.2`)
  - requires a running Docker daemon
  - disable fallback with `APALACHE_ALLOW_DOCKER=0`.

Examples:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## Notes

- This model complements (does not replace) executable Rust model tests in
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  and
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- The checks are bounded by constant values in the `.cfg` files.
- PR CI runs these checks in `.github/workflows/pr.yml` via
  `ci/check_sumeragi_formal.sh`.
- Scheduled/manual CI runs the same formal baseline plus the longer
  `frontier-nightly` bound in `.github/workflows/nightly_sumeragi_formal.yml`.
- English documentation is authoritative for the current frontier formal slice.
  Translated `docs/formal/sumeragi/README.*.md` files are intentionally not
  refreshed here and may remain source-current stale until a separate
  translation refresh.
