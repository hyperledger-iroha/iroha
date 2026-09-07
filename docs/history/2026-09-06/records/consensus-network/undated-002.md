# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-35790c77bb95927be924def983fbc2a29416f6a861973c86652fa5304f45abc4"></a>

<!-- Original context: Roadmap / Sumeragi v2 revision-4 release gates -->
- Keep the unified consuming V1 lifecycle-owner factory as the sole serialized
  runner owner for every non-PendingKura height. The retained verified context,
  coordinator, concrete registry, Certified-Serve payload store, and body store
  now move together through startup, live execution, and finalization. Startup
  privately selects exactly one of no WAL
  authority, recovered phase-vote repair, recovered control-Sign repair, or
  recovered Decision-Fetch repair.
  Exact `ProposalIntent -> SignProposal` and
  `TimeoutIntent -> SignTimeoutVote` restart paths authenticate the terminal
  WAL frontier independently, then retain the latest exact frame owning the
  reducer's current signature through checked LedgerV1 admission, concrete
  carrier installation, and the final Fetch/Serve/Producer open. Identical
  repeated intents select the later match, and terminal frames may instead own
  queued Prepare/Commit signatures. A historical Commit remains eligible under
  the current later-view tag only with its exact active-lock `LockAndCommit`
  PrepareQC lineage. A sealed dispatch-only prerequisite now covers all three
  recovered Sign classes. It rejoins PhaseVote to the current terminal Validate
  parent, reserves a class-sensitive worker position before claiming, and parks
  one guarded opaque completion. Vote/Timeout single-Broadcast successors now
  cross one LedgerV1 transaction and have an authenticated cold-open path which
  cryptographically replays `Signed` before installing the live Broadcast
  carrier. A separate typed refanout claims that durable carrier, enters exact
  output, and parks only volatile state; finality must retire the still-live
  durable output debt. The Broadcast-plus-next-Sign prerequisite now has one
  affine body/WAL authority, an executable standalone next-Vote candidate, and
  exact two-child coordinator/registry staging. The source-only WAL-ahead
  Proposal transaction consumes the shared output reservation across one
  LedgerV1 fsync, installs
  both children, parks the Broadcast only in volatile state, leaves the next
  Sign Ready, and commits control plus chunks atomically. Recovered Prepare
  votes use the same two-child fsync without Proposal output ownership, leaving
  both Broadcast and Commit Sign Ready until typed refanout owns the former.
  A frame-bound cold
  classifier now recognizes only the exact Proposal-to-Prepare or
  Prepare-to-Commit pair (even with unrelated later rows), and the WAL seal can
  replay the lost historical signature only to those two children. The control
  Proposal branch now reconstructs canonical chunks from the same revalidated
  body store, installs the linked pair in the complete cold-open census, and
  advances the adapter. The phase Prepare-to-Commit branch now rejoins its
  retained Validate/body authority, installs the linked Broadcast/Commit-Sign
  pair, and advances the adapter to the exact Commit-signature fence. Their
  shared refanout driver retains unrelated Ready work and recognizes a pair
  only through the Broadcast carrier's exact child seal. The shared
  exact-output corridor now preflights and
  commits Proposal control plus chunks as one batch for both ordinary live
  output and recovered restart output; the latter remains bound to the exact
  body-store and output-guard owner and returns its opaque authority on
  capacity/abort. The initial `ProposalPrepareWal` transaction now holds that
  reservation across exact PrepareIntent WAL fsync and the adjacent
  Broadcast/Prepare-Sign LedgerV1 fsync; post-WAL failures are restart-only.
  The unified lifecycle Completion-turn driver keeps both recovered Proposal
  shapes inside the production runner activation transaction; neither shape
  routes through the bounded single-Broadcast cut. The
  recovered Decision-Fetch ingress prerequisite now removes caller-selected
  physical ordinals: ordinary dequeue and lifecycle discovery share the same
  ready-source/lane, strict-before-dependency selector, and the lifecycle path
  returns a recovered selector only when that exact fair winner owns the
  authenticated response family. The non-Pending production runner now uses
  the lifecycle height driver to compose the unified Completion and Ingress owners with the intervening
  serialized Runtime turn around the real borrow-bound cursor. It derives the
  output guard from the retained services and routes an ordinary winner through
  the shared opaque post-dequeue consumer.
  It retains Apply deferral, guarded Sign/Fetch completions, and recovered
  ingress capacity waits internally; takes at most one physical Completion
  head; classifies the complete Ready census; and returns the unchanged cursor
  only when no fair ingress winner exists. Its queue-owned ordinary ingress
  remains inert while an earlier recovered Sign completion is previewed under
  Completion-before-Runtime; pending effect, scheduler, and leader-wire
  mutation debts still exclude that completion transaction. The ordinary
  ingress prerequisite then freezes that exact fair winner. Current-height
  Certified-Serve enters the coordinator-owned selector/admission transaction,
  and the typed capacity/dequeue cut physically removes the same occurrence
  only when its registry-attested scheduler claim is ready.
  Backpressure retains the carrier plus off-queue debt, while recovered
  Decision-Fetch retains its queue witness through Phase A. Mixed
  Apply/Sign/Fetch Ready rows now freeze one composite worker/output-capacity
  census and transfer only the ranked row's typed reservation. A sealed
  preactivation runner key now permits one callback over only the launched
  executor and services while exact
  output ownership, closed ingress, the retained observer, and unarmed clocks
  hold before and after the callback. The CompleteTip H/H+1 wrapper now lends
  that same transaction without releasing its retired predecessor. Recovered
  ProposalIntent ownership is
  retained through cold adapter advancement and can clear its activation
  blocker only by binding the matching reducer directive into a move-only
  prepared runner state which ordinary and CompleteTip activation retain. The
  activated runner-only borrow now supplies that same opaque scheduler owner
  beside owner/executor/services, and the non-Pending production loop uses it
  in place of the legacy local-Proposal scheduler. An
  armed non-permit fail-stop scope closes output on error or unwind without
  retaining a read permit across nested service fail-stop code.
  The ordinary token now enters the same private runner-owned post-dequeue
  consumer used by the legacy loop, and one production-shaped activated fixture
  exercises the complete lifecycle-owned ordinary batch. The opaque `PendingKuraApply`
  Decision-Fetch/WAL/runtime replay join is sealed through preactivation.
  `PendingKuraApply` now uses one dedicated lifecycle-owned no-clock recovery
  height; its verified successor crosses into the ordinary lifecycle loop and
  no legacy owner extends to later heights. The runner's
  freshly opened body store must first enter a
  move-only quarantine that rejects any already promoted, rejected, or retired
  marker, then enter the sole production factory with an adapter-bound
  execution/storage seal. The quarantine's only consuming transition fixes
  finality filtering, WAL-authority filtering, semantic marker replay, and
  sealing with the same `V2ApplyService` the owner retains for launch. It has
  no root-reopen or raw validation-callback alternative. A private runner-only
  permit now seals the prerequisite dependency/local-signer/authenticated-cadence handoff,
  avoiding the uncommitted State placeholder at fresh height one; the
  production lifecycle runner mints and consumes it. Launch rejects a different peer key or incorrect claimed
  validator position before gate/runtime creation. Authenticated height recovery now
  mints one move-only storage authority binding the verified context, exact
  Kura instance, context-addressed ledger/body roots, and authenticated
  genesis-or-rotating signature policy. The same recovery boundary also
  retains the universal genesis account derived from its authenticated genesis
  key; the factory moves that account into its single replay/live service only
  after exact State/Kura Arc, network, storage, and startup-instance checks.
  Verified live successors now retain that State-owned Kura identity and
  consume themselves into context, activation, and a rotating-policy lifecycle
  storage authority at both ordinary lifecycle finalization and the dedicated
  PendingKura lifecycle handoff.
  The same seal now retains the Kura-derived safety-WAL and
  chunk paths; the factory binds the adapter's held WAL before store side effects, and launch
  internally restores one ordinal source, folds producer and leader-wire
  high-watermarks and opens the leader-wire binding from the owner-held body
  store on the same still-closed ingress. Certified-Serve no longer has a
  parallel service gate; it joins only through the lifecycle coordinator after
  selection. The non-PendingKura production runner consumes this path, and one
  RAII owner retires the leader-wire binding atomically. Raw paths,
  ordinals, receipts, Queue/archives/cadence/events, and genesis authority are
  no longer launch inputs. Launch consumes the retained Apply service through
  a parent-sealed move-only worker permit. The authenticated adapter
  startup retains its validation frontier internally and provides in production
  only one consuming WAL-bound leader-wire launch authority, so neither the
  adapter nor replay batch can be extracted.
  Residual and WAL-projection
  mismatches fail before that cut is unsealed and before Serve or Ledger opens,
  while status remains unpublished in the opaque owner. A sealed consuming
  launch now transfers that owner's exact body store through the runtime and
  executor into the I/O worker. Staged-genesis verification seals the exact
  signed body into a move-only recovery token, and launch may install only
  that optional token before worker start. It verifies the store/output identities, and
  leaves only the store-instance seal in the owner; the runner must consume
  this launch and publish status only after clocks and authenticated ingress
  are live. CompleteTip retirement now retains the actual opened canonical H+1
  ledger handle/frame and can consume an unlaunched lifecycle owner into an
  opaque exact join only after rechecking its verified context/parent QC,
  ledger target and projection, body and Serve roots, the exact post-prune
  authenticated Serve cut against a fresh bounded directory census, adapter
  context, and concrete registry. That bound-owner seal now consumes the sole
  generic owner launch and returns another opaque wrapper which retains the
  running H+1 stack together with its retired-H authority. The lifecycle runner
  now consumes that wrapper through its one-shot activation without unwrapping
  it into the generic successor publisher.
  No caller-supplied branch or recovery cut remains. The owner now has a
  sealed scheduler-input factory for the complete directly owned Ready subset:
  closed Validate completions reattest
  through the registry and raw rank construction is unavailable. Certified
  Fetch now also has a consuming locked I/O-capacity transaction joining the
  live executor mode, ingress selector/lane/source, concrete registry carrier,
  and runner-reach observation, followed by the exact `n + 1` Blocked command
  fence and existing `n + 2` durable completion. It currently rejects
  production use only outside the lifecycle-owned current-Ingress cursor. The
  non-PendingKura runner now uses the consuming owner-to-I/O-worker launch and
  calls the planner through that cursor; the PendingKura runner uses its
  narrower decided-lane ingress and exposes no ordinary scheduler turn.
  Certified-Serve startup now installs exact shared-family Serve and
  ProducerTurn carriers, and a selector-bound fresh transaction
  fsyncs its payload before atomically staging LedgerV1 plus both carriers.
  That fresh transaction now uses the exhaustive all-kind
  coordinator/registry oracle, including exact logical indexes, capacity,
  Serve/Producer debt and shared replay-family checks, recovered
  Broadcast/next-Vote pairing, and every concrete carrier variant, while
  retaining the already-wired sealed selector target in the owner. The
  receipt-free terminal owner transaction now closes payload store,
  coordinator, registry, and LedgerV1 ownership and performs the exact
  Serve/Producer carrier replacement or removal; the lifecycle runner transfers
  body-store completion authority to the worker without exposing a receipt or
  raw parts API. ExecuteBody Validate now follows the same lifecycle-owned
  scheduler/worker boundary: an exact registry attestation and I/O/output
  reservation precede the Ready-to-Waiting claim, guarded completion replaces
  that row with the exact next Ready work, and missing merge-sidecar completion
  remains parked under its durable same-row registration for wake or cold-open
  recovery. Selected-Serve predecessor scheduling
  now uses one direct runtime observation plus a move-only, one-turn worker
  admission; the old persistent episode/witness state is gone while immutable
  lifecycle ordinals and the single bounded retry latch remain.
  The phase-vote/control-Sign/Decision-Fetch end-to-end owner-factory and
  empty/two-Fetch/terminal-Validate-plus-Serve startup checks, including steady
  and payload-store-ahead terminal Serve, are represented in the current
  focused fixtures. Decision Fetch now has an exact
  certificate-only V1 origin for its absent manifest and enters the complete
  Ledger/registry/coordinator startup census with exact no-rewrite coalescing;
  quarantined markers cannot cross the revalidated-store seal. A matching
  promoted success detaches into an opaque same-store/full-context cut;
  promoted or quarantined deterministic rejection cannot be treated as refetch
  or Apply authority. The recovered Fetch carrier now also has a sealed
  request driver: it reserves exact-output and executor request ownership
  before claiming the Completion turn, installs a disjoint request/reverse
  census, and emits the exact signed fanout. Its typed response selector
  re-probes the active Ingress cursor and claimed carrier before publishing a
  dedicated body-store command; the guarded durable completion remains parked
  and indexed outside the generic drain. Its restart-closed Fetch-to-Store
  transaction now prelocks the exact ingress occurrence and preflights every
  request, carrier, adapter, registry, storage, and output owner before fsyncing
  the payload-free Fetch `Advanced` plus body-frame Store successor. The
  post-fsync tail is assertion-only and retires the request, ingress, and
  worker index before disarming the output guard. Cold open reconstructs the
  dedicated Store owner, while body-store open separately defers the exact
  validated marker and keeps pending-tip startup storage-only. The production
  pending-tip driver consumes each local Fetch/Store/Validate stage through its
  restricted stage consumer. Successful Validate atomically commits the direct
  reducer update and emits its sole predecessor-derived move-only Apply child;
  the recovery step advances to Apply before releasing that child to executor
  dispatch. No recovered pending-Kura Apply carrier, generic pending-specific
  Apply settlement path, Apply-only gate, compatibility ordinal accessor, or
  runtime ownership sidecar remains. Applied completion commits Kura →
  LedgerV1 → coordinator/registry/adapter/executor → worker acknowledgement in
  durable order. The typed missing-sidecar retry republishes only the unchanged
  task under its existing dedicated key while both fail-stop guards remain
  armed. Settlement first registers the exact round/subject/reference in the
  lane recovery journal. Its stable keyed owner preserves each decided carrier
  across executor cleanup, retains terminal invalid-entry evidence outside the
  generic drain, and dispatches only the next fair matching request through
  the exact paired service/lane endpoints. Retry reauthenticates that local
  sidecar before reserving Consensus capacity; unavailable sidecar or capacity
  returns the whole owner unchanged. Queue-level regressions now pin the exact
  keyed retry transfer and both capacity/barrier non-mutation cases. Layered
  acceptance covers the full crash-window, deterministic rejection/report
  publication, same-row sidecar wake, restart reconstruction, and
  post-LedgerV1-fsync fail-stop in exact four-validator BLS/RS16 contexts, plus
  real four-validator success, view-change, restart, and authenticated
  consensus Hold/Drop convergence. The feature-only controller and exact
  four-validator test cover authenticated, Proposal-bound `PayloadChunk`
  Hold/heal selection, and the final current-tree locked/offline acceptance run
  is green, 1/1 in 84.82 seconds. The other strict live-network gaps are an
  unskipped invalid-
  body-report test, merge-sidecar-recovery test, publication-phase-targeted
  restart matrix, and the ignored strongest observer/body-recovery case. These
  are evidence gaps, not missing production joins. CompleteTip
  recovery now retains its full Kura artifact and receipt instead of a lossy
  predecessor hash projection, and terminal Apply has a separate exact
  four-row ledger oracle which cannot mint a live carrier. CompleteTip recovery
  now retains verified H and derives private context-addressed H/H+1 lifecycle
  targets plus the predecessor body-store root/signature policy from the same
  Kura instance; a copied predecessor frame at another root is rejected. The
  production disk-only retirement transaction is wired before ingress: it
  consumes receipt-refreshed Certified-Serve terminal updates, cancels every
  remaining live row, clears producer debt, fsyncs/reloads H, then initializes
  H+1 at the retained ordinal high-water or authenticates a later exact
  descendant above that floor. A dedicated move-only retired token is retained
  but cannot publish status: runner preflight stays closed until the sealed H+1
  lifecycle owner consumes that canonical successor frame, and that rejection
  precedes every legacy H+1 adapter/worker/output constructor. Its bound launch
  now consumes the generic owner-to-I/O transaction without exposing either
  half, retaining the launched stack and retired-H authority in one typed
  activation prerequisite. That wrapper now has a consuming CompleteTip-only
  activation which keeps retirement sealed through clocks, observer install,
  exact ingress open, and H+1 status publication. The parallel ordinary
  activation returns an opaque owner borrowable only through a private runner
  key. The consuming finalization chain is now present: readiness and exact
  ingress close first, leader-wire ownership retires, executor/Kura finality and
  adapter WAL retirement complete under fail-stop ownership, and the existing
  lane/service output transaction seals its durable handoff while services and
  the lifecycle owner remain joined. A fresh post-handoff Certified-Serve
  census then rejoins ledger rows plus capacity waits, payload retirement
  precedes one opaque all-row LedgerV1 publication, and its coordinator-owning
  publication token consumes the concrete registry. Only the cleanup-ready
  state permits normal finalized-height teardown. Explicit consuming shutdown
  transitions now close runner readiness and the exact queue, release prepared
  local-Proposal state, detach leader-wire ingress ownership, and permit normal
  worker stop for unpublished or active heights without claiming finality;
  CompleteTip keeps its retired predecessor joined through the same boundary.
  The consumed activation authority also lends one borrow-scoped preactivation
  ingress aperture for legacy-parity canonical-body recovery, with RAII closure
  before clocks or status publication. The non-PendingKura runner now consumes
  the landed activation/finalization/shutdown states and has no independent
  adapter/body-store/service startup path. The isolated one-height PendingKura
  path now has its dedicated lifecycle state and an executable Kura-first
  shutdown/finalization fixture with source mutations. The former monolithic
  height body is removed. The current-source runtime slice is 159/159, the
  lifecycle height-driver slice is 12/12, and the production-shaped terminal
  CurrentServe regressions are green; broader formal baseline debt and the
  unrelated all-target workspace errors remain release-wide gates.


<a id="record-4f6e1c82509ce90628fa9282f4d54c65dfb5150a1f600a0bef159b6b8c0b51ad"></a>

- Exercise autonomous carrier terminal gating, bootstrap roll-forward, and
  lifecycle retention with the focused Kura/Sumeragi regressions and a fresh
  disposable four-validator devnet. Require the multi-stage workload to apply,
  all peers to remain healthy, the queue to drain, and committed heights to
  converge without unexpected rejection. Public deployment qualification is a
  separate operator activity and is not a prerequisite for creating this
  throwaway network.


<a id="record-d8481815c20238933463419f0c9c16630eb96ff63bac032239f5d1945d41a0d6"></a>

- Keep the selected-Serve `missing_proposal` repair covered by the focused
  mutation matrix and disposable four-peer smoke. Live evidence showed a
  responsive pacemaker
  rotating views while proposal scheduling stayed behind one selected Serve:
  a passive Fetch could complete after the prior predecessor turn and re-enter
  `BodyAvailable` under an older ordinal. Runtime now directly observes the
  current runnable prefix on each bounded turn. The first observation opens a
  checked worker aperture; later observations open it only for a runnable
  predecessor. A move-only guard binds admitted fanout to one older owner and
  closes the aperture on success, error, or unwind. Passive Fetch remains
  outside the runnable minimum, so an unresponsive source cannot block the
  pacemaker. The earlier focused regression still drives local timeout,
  responsive remote TimeoutVotes, grouped TC, and `EnterView`; the final queue
  handoff still blocks fresh Serve replenishment until one ordinary producer
  turn runs. Keep exact semantic
  retries on their immutable owner, reject owner replacement before refinement,
  and retain only the separately typed monotone Fetch/Store/Validate authority
  lineage.


<a id="record-28b51aef2578ec298da08223797f0b3ac43f361b066bd962e7874645a8002c92"></a>

- Preserve the final-seam targeted lifecycle and exact `iroha_core`
  compilation receipts while closing release evidence. The current-source
  enlarged-stack coordinator sweep is green 510/510. Resolve or explicitly
  scope all 66 broader source-budget findings before release evidence is
  sealed. Run the
  remaining workspace tests, strict Clippy, and the revision-4 formal syntax,
  invariant, and mutation corridor. Focused
  consensus validation must include pre-Decision cache rejection,
  disjoint-roster historical lane signing/recovery, exact-predecessor sidecar
  reservation under outsider pressure, DA resource-cap boundaries, volatile
  shard reacquisition followed by one durable canonical-body boundary, restart
  hydration, live/replay timeout-fenced PrepareQC-to-lock fallthrough, fresh
  generated four-peer genesis startup, and nonblocking cleanup saturation.


<a id="record-68fbc908ce79d236e8195f8a936648383bff53d702cba7cd16cbc5fdf871b328"></a>

- Run unskipped chaos on representative networks of at least four validators,
  covering faulty/withholding leaders and proxy tails, Set A and Set B loss,
  asymmetric partitions, RS16 reconstruction, restart, and catch-up.


<a id="record-35d7981b031a711d3fc7037ca6de40abe9141e8e0bf5290b52f18bd212a623ef"></a>

- Pass a source-sealed 24-hour liveness gate under continuous load and scheduled
  faults, with zero conflicting finality and zero no-progress interval that
  remains after the network and storage assumptions recover.


<a id="record-dfd59fe0e67079893e0943ffa1bd7700c8f94685b3970e6b39f0fb477ec55022"></a>

- Preserve the performance SLO: one-second block cadence, 10,000 TPS target,
  permissioned p95/p99 finality at or below 1.5/2 seconds, and NPoS p95/p99 at
  or below 2/3 seconds.



<a id="record-f85df116a2247f9aa08bd919f56cf51067e5ed98ecbe1e7759b9c6cf63e9825a"></a>

<!-- Original context: Roadmap / Musubi first-release registry and developer ecosystem reset -->
- A four-peer below-quorum queue-journal crash/restart smoke is now present and
  asserts that a replayed unavailable-archive publication is canonically
  rejected once and leaves no package, release, resolver, directory, or archive
  reverse-reference projection on any peer. Execute it, then build on it and the
  deterministic snapshot fault-cut regressions. The feature-isolated daemon
  now provides source-bound, one-shot aborts after PrepareQC, after CommitQC,
  and immediately before world commit, with a durable canonical acknowledgement
  before each process cut. The real-ISI gate now drives a selectable
  three-replica publication through all three cuts in isolated four-peer
  networks, restarts the faulted peer from persistent storage, and checks the
  exact all-peer home/universal tuple plus one publication occurrence after a
  finalized barrier. Execute and archive that gate once the unrelated
  `iroha_data_model` test-target E0603 at `account/recovery.rs:284` is cleared
  before closing the no-half-visible release gate.
  Qualify the one-million-package/twenty-million-row lookup, search, resolver,
  and 64-MiB fetch-memory scale targets. The bounded production CAR bridge now
  has six-frame ownership accounting, terminal worker joins, an isolated
  logical-heap regression, JSON structural/scalar envelopes, and cache
  plan/ingest accounting; the remaining fetch gate is a
  deployment-equivalent HTTP/TLS + JSON-DOM + cache process-RSS or cgroup
  measurement.


<a id="record-87736892db47251724036ec2be0bc9b3fff5b7f601514c947f7ccf0e8e7c83d5"></a>

<!-- Original context: Roadmap / Sumeragi V2 production multilane release closure -->
## Sumeragi V2 production multilane release closure

The canonical 54-obligation ledger records 44 `tlaps_proved`, 3
`cross_tool_proved`, 6 `trusted_contract`, and 1 `out_of_scope`, with no
`specified_unproved` rows and `machine_checked_completion: true`. This freezes
the checker-mandated revision-4 status inventory before evidence generation; it
is not proof evidence by itself and is not a deductive proof of revision 4. The
next formal step is to freeze a clean signed tree, run strict
TLAPS and pinned Verus, derive the cross-tool and production-trace evidence,
execute the separate mandatory revision-4 TLC/mutation corridor, and validate
the signed receipts and completion marker against that same commit and ledger
digest.

An earlier mutable-tree static release inventory contained 84 legs, 864/864
production tests across 43 modules, 522/522 G-UNIT rows, and four mandatory
four-peer gates. Its grouped fixture SHA-256 was
`e4fb62addba3c3b8aecdbff55840e21620c770ab96d346ca55b156cf0239942b`.
Its grouped SDK and diagnostics closures contained 1,400 and 1,402 paths at
`fdc4c3fb9192277bc6a80018ecb62ed096f12cb41a4c40bde0681ee8f6478577`,
and
`f2e7d3b79dd311b93f9ade26a846940dcd12b636c551a70800f63817a10bfa4c`.
Those values remain mutable-tree history, not a sealed receipt. The Java
`TransactionAdmissionIntent.java` path is now tracked, so the earlier
untracked-source blocker is closed.

Both reproduced clean-`d24` consensus root causes are closed in source and
their three focused regressions pass. The bounded Kura-backed tag-21 QueuePlan
handoff is independently audited, but no fresh exact four-peer run exists for
this merge. The macOS framework-Python relocation chain also passes its real
copy/rewrite/sign/probe path, exact bootstrap, general runtime population,
validator, and receipt checks on frozen bytes; its independent audit is clear.
Neither focused result substitutes for complete Cargo, network, formal, or
release-proof execution against an immutable candidate. The package-layout
contract still permits only the reviewed test-only
`v2_core/refinement_cases.rs` split and rejects additional, parent-relative,
non-test, or skipped-verifier mutations.


