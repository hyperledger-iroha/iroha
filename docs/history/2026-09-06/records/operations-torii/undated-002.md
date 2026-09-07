# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-c0ffecdc8479092787cffbebf61a5bcfb2e857e319376cb7637e4bb38e2d35f8"></a>

<!-- Original context: Roadmap / SORA Nexus and Taira -->
- Merge replay hardening now includes active-catalog checks during
  exact leader-candidate synthesis and globally carried State publication,
  plus per-lane latest-height tracking across active-only merge entries. Lane lifecycle and
  config-swap resets also prune lane-scoped DA receipt cursors and unshared DA
  shard cursors for fresh lane incarnations and same-shard lane/dataspace
  rebinds, including lifecycle plans that retire and add the same lane id in one
  transaction. Verified-relay cleanup now prunes decoded canonical/map rows and
  undecodable lowercase exact canonical keys for reset lanes while leaving
  arbitrary prefixed siblings and uppercase digest variants inert. Contract-state
  hydration now applies the same lowercase exact-key scan before decode, so
  noncanonical prefixed state cannot drive relay-cache admission attempts.
  Reset pruning now also covers public-lane stake shares and reward records by
  storage-key or embedded lane ownership, while reward-claim cursors remain
  key-owned. Durable DA pin-intent query indexes are pruned for reset lanes by
  embedded ticket lane, lane/epoch key ownership, and stale ticket cross-index
  references, including direct lifecycle/config swaps and autoscale scale-in
  overlays; same-block pin-intent bundles are sanitized after the autoscale
  catalog update so retired-lane intents cannot publish through commit-order
  races, and surviving pins keep their original canonical bundle positions even
  when a hidden retired-lane pin sits between surviving records; Kura DA index
  replay now covers that pin-intent visibility and location boundary after
  restart while preserving committed owner fields without revalidating them
  against the current post-removal account set. DA commitment bundles now apply
  the same post-lifecycle visibility rule: the canonical block bundle and
  committed identities are retained, while
  retired-lane records are withheld from query, cursor, and confidential-compute
  indexes after autoscale scale-in; confidential-compute receipts keep their
  original committed bundle locations even when a hidden retired-lane record
  sits between surviving records, and Kura DA index hydration now replays the
  same visibility rules after restart. Per-lane reset watermarks are now
  persisted with the DA shard cursor journal, so historical records at or below
  a retire/recreate or same-shard lane/dataspace rebind reset height remain
  available as committed block bundles but cannot rehydrate active pin intents,
  query-visible commitments, shard cursors, or committed identity reservations
  for the fresh lane incarnation after rewind or restart. Same-lane DA policy
  changes, including DA shard mapping, visibility, storage profile,
  proof-scheme, manifest availability policy, and confidential-compute
  key/audience policy changes, now use the same reset-watermark path so stale
  Kura records, shard cursors, and confidential receipts cannot rehydrate into
  the new policy epoch after rewind or restart. Same-plan retire+add
  replacements now also apply destructive physical geometry
  semantics: Kura and tiered-state storage archive the old segment before
  provisioning the replacement and reject occupied replacement targets, so a
  fresh lane id cannot inherit stale block, merge-ledger, or cold snapshot
  files through the relabel path. Lifecycle replacement of the configured
  default route is rejected after normal routing-policy validation, keeping the
  base route as an active-lane anchor instead of destructively recreating it
  under Kura's active block store. Staged autoscale lifecycle commits now also
  revalidate the committed Nexus baseline before storage geometry publication,
  including catalog, dataspace, routing, autoscale, and derived lane-config
  inputs, so disabled or retuned autoscale and committed catalog/config drift
  abort before Kura/tiered storage or lane catalog mutation. The code corridor
  now includes a four-peer autoscale/certified-merge recovery fixture with one
  offline sidecar-missing peer and repeated restart proof convergence;
  remaining work is its fresh validation and live rollout evidence rather than
  stale cached-relay, stale DA-cursor, stale storage, or stale public-lane
  economic admission. Focused local validation has also
  rerun the previously pending routing dataspace/default-lane autoscale guards
  and mismatched public-lane validator-row filters, keeping remaining Nexus
  validation debt concentrated on end-to-end rollout evidence. Lane-local
  proposal, vote, and QC messages now also feed a bounded session cache that
  deduplicates replays, rejects conflicts, verifies QC aggregates with signer
  proof-of-possession material, seals prepare/commit QCs from cached vote
  quorums, broadcasts locally sealed QCs once, moves proposal-plus-QC
  committed sessions into a bounded actor-owned staging queue, publishes a
  compact committed-lane-block status surface for rollout evidence, broadcasts
  finalized lane-block proposals plus the local BLS prepare vote after stale
  proposal checks pass, broadcasts the local BLS commit vote only after a
  validated prepare QC is cached, feeds queued committed lane sessions back into
  proposal-planning lane tips, persists certified standalone lane-block
  sessions as Norito-framed Kura sidecars with proposal/prepare-QC/commit-QC
  plus signer-PoP aggregate validation, reloads those certified sidecars into
  proposal-planning lane tips and the bounded committed-session queue/status
  surface after restart, verifies when Kura can recover every accepted
  entrypoint from the anchoring global block body, exposes and persists the
  recovered proposal/artifact/entrypoints as Norito-framed execution-input
  sidecars, advertises certified-only sessions as
  `awaiting_executable_payload`, recoverable sessions as
  `payload_available_awaiting_executor`, and recovered handoffs as
  `payload_recovered_awaiting_state_application`, records canonical
  block-result receipts as durable Norito-framed application sidecars,
  advertises those receipts as `state_applied_by_canonical_block`, defers valid
  artifacts as `payload_unapplied`, and retains recovered executable inputs as
  certified sources until an exact globally ordered merge carrier commits
  their WSV effects. Data-model drift tests also pin every lane
  descriptor/proposal replay, predecessor, ownership, validator-set hash
  version/hash, quorum, and QC-mode field in canonical identities, plus every
  lane vote-body replay, ownership, validator-set, quorum, and QC-mode field in
  the BLS-signed preimage. Four-peer main-loop coverage now proves one final
  proposal emits two lane-block proposal artifacts and two local prepare votes
  for active lanes at different lane-local heights without waiting for an idle
  default lane, mixed-committee coverage proves that local prepare-vote
  broadcast is limited to lanes whose validator set includes the local peer,
  and negative coverage proves a cross-lane batch with one missing lane
  authority is deferred atomically with stale ownership status cleared. Fresh
  active lanes with no lane-local relay history now start predecessor planning
  from lane-local height zero instead of inheriting a global compatibility
  height, while reset watermarks remain the baseline for recreated lanes;
  broader default-soak live multi-peer rollout evidence remains outstanding.
  The 12-peer
  cross-dataspace localnet route-probe corridor now gates DS1 and DS2 planning
  on observable replayable lane-payload ownership with exact dataspace IDs, the
  expected four-validator lane committees, and the deterministic three-vote
  quorum. The route-probe parser now requires those ownership rows to pass the
  canonical replay-material hash validator, treats same-slot ownership identity
  drift or malformed latest ownership rows as non-progress, and keeps
  same-height cross-peer observations from combining into quorum progress
  unless their descriptor, subject, payload-ownership, and RBC-instance
  identities match. Conflicting top committed-lane or ownership identities that
  independently satisfy quorum at the same lane height/view now fail closed
  instead of being selected by iterator order. An extra DS1 route
  probe proves one active lane can advance beyond another in lane-local block
  height without waiting for an idle configured lane. Committed standalone
  lane-block summaries now ride the canonical Norito status payload too, and
  the localnet parser requires each active lane to publish committed rows whose
  embedded prepare and commit QC signer counts satisfy the exact lane quorum.
  The parser also retains committed-lane
  descriptor/proposal hashes, subject hash, payload-ownership hash,
  RBC-instance hash, and QC-mode tag and treats same-height rows with identity
  drift or malformed latest QC metadata as non-progress, so conflicting
  certified payload identities cannot fake scale-out progress, combine into
  cross-peer route progress, or prove safe scale-in contraction.
  Runtime committed-lane status merging now also requires descriptor hash plus
  proposal, prepare-QC, and commit-QC equality before replacing an existing
  row, so malformed in-memory descriptor or QC drift remains visible as
  suspicious evidence instead of overwriting the original certified status.
  That separates independent lane-height planning evidence from standalone
  lane-block QC evidence instead of treating operator status-row fanout as the
  safety proof. The route-probe corridor now also requires an applied
  certified committed-lane row with executable payload material for each active
  dataspace, scans every peer for that self-certified QC evidence, and keeps
  the stricter latest-row application selector under focused parser coverage
  while the end-to-end gate accepts the highest applied certified row when
  later rows are still awaiting predecessor application. The bounded
  committed-session queue now releases capacity only after a durable
  application receipt is validated, and restart hydration skips already
  application-receipted certified sidecars so applied work cannot crowd out
  unapplied committed lane blocks. That hydration now scans every valid
  current-dataspace certified sidecar in deterministic lane-height order,
  rather than only the latest tip, so restart has a complete ordered backlog
  of certified sources for global merge selection. The execution boundary
  gates payload recovery and merge eligibility on a durable application receipt
  for the exact predecessor lane height and descriptor hash, surfacing
  `awaiting_predecessor_application` as a fail-closed status when a later lane
  block has payload material but the prior lane block is not proven applied.
  Recovered execution inputs are revalidated against the current canonical
  lane payload artifact and proposal before merge admission, so stale or
  tampered handoff state cannot survive canonical artifact drift. The global
  leader executes a canonical prefix on one pristine revertible `StateBlock`;
  followers fetch and re-execute that exact body, and result vectors, settlement
  evidence, write-set roots, and base-state commitments must all match before a
  merge signature is possible. The complete entry is stored as a bounded
  sidecar while the global block carries only its certified reference. Startup
  reconciles log, carrier, and block suffixes before replay, and live publication
  emits exactly one merge event. Autoscale rollout evidence now uses the
  shared `iroha_data_model` committed-lane status classifier to allowlist only
  audited execution labels with matching executable-payload flags, so conflict
  or unknown future states cannot be counted as horizontal expansion/progress
  evidence until intentionally reviewed.
  Public-lane economic cleanup and embedded-reset-lane validator exit tests
  have also been rerun across `set_nexus`, manual lifecycle, and autoscale
  scale-in paths, so local reset cleanup validation debt is closed before the
  broader independent-lane rollout evidence pass.
  DA cursor reset, rehydrated merge-history reset, AXT replay ownership, and
  verified-relay stale-state cleanup regressions have likewise been rerun, so
  stale reset-boundary state validation is covered locally before live rollout.
  Reset-selector, lane-state pruning, removed-validator exit, retire preflight
  preservation, and autoscale scale-in height/preflight failure regressions
  have also been rerun, confirming destructive lane cleanup remains
  fail-closed before storage/catalog publication.
  Focused non-localnet multilane integration suites for router behavior,
  pipeline setup, Kura storage layout, cross-lane adversarial isolation,
  global commit behavior, and lane-registry wiring have been refreshed after
  these local hardening passes. Cross-dataspace localnet route evidence now
  passes genesis pre-execution and covers route probes, applied certified
  evidence, stricter committed-history quorum checks, proposal-readiness
  deferral for unapplied lane blocks, Kura sidecar recovery from local canonical
  block bodies, and lane/dataspace-preserving ownership status. The remaining
  historical pre-fix blocker was DvP lane-block application liveness under the
  12-peer localnet: route probes could stall before committed/applied
  lane-block convergence, with one DS lane split between executable-payload
  wait and canonical application while the other DS lane had no committed
  lane-block row. The source-side recovery and bounded stage diagnostics have
  since been corrected. A fresh strict 10/10 deterministic-seed rerun and
  two-hour rotating-fault soak remain mandatory; the pre-fix default run that
  exited `ok` with only 3/10 iterations is not release evidence. Older
  full-soak evidence remains the 2026-07-03 Nexus consensus metadata snapshot
  with a 10/10 paired-swap soak and rollback verification. The
  feature-gated
  STARK cross-dataspace localnet fixture now uses the same canonical dataspace
  catalog and has non-ignored `zk-stark` genesis pre-execution coverage, while
  its native STARK/FRI proof runtime tests remain intentionally ignored until
  AIR openings are implemented. The remaining runtime dataspace-registration
  and wrong-ingress tx/query localnet fixtures now use the same canonical
  dataspace catalog; tx/query runtime evidence also targets the current
  `/v1/pipeline/transactions` submit route and uses production Torii read
  fanout request budgets, with the full 12-peer wrong-ingress flow passing
  locally and refreshed on the same 2026-07-03 Nexus consensus metadata
  snapshot. Autoscale localnet evidence now covers both the canonical
  expand/contract cycle and the public-profile strict transition cycle after
  tightening stdout transition-marker parsing for real
  tracing-target prefixes.
  The canonical 4-peer expand/contract fixture was refreshed on the same
  2026-07-03 snapshot with scale-in transitions observed after contraction on
  every peer. The strict autoscale and public-profile strict transition
  fixtures were also refreshed on that snapshot, with quorum-observed expansion
  status and scale-out/scale-in transitions on all four peers. The repeated
  two-cycle expand/contract fixture was also refreshed on that snapshot,
  reaching deterministic scale-out quorum on both cycles and validating
  contraction profiles across consecutive scale-in passes.
  The autoscale soak harness now also fails closed on stale prior scale-out
  logs and retry cleanup races: strict repeated cycles require fresh
	  post-baseline scale-out quorum, cooldown clearance re-checks contraction
	  before taking the cycle baseline, strict probes stop adding top-up load once
	  transition quorum is observed, and the reporter rejects successful-cycle summaries with
	  scale-out or scale-in quorum misses. The reporter summary now also publishes
	  quorum-required maxima, successful scale-out minimum peer counts, required
	  scale-in cycle counts, required scale-in quorum minima, and optional scale-in
	  cycle counts for rollout review. A fresh 2026-07-07 strict expand/contract
	  localnet rerun now passes after strict expansion readiness was hardened to
	  require fresh deterministic scale-out transition evidence and expanded-lane
	  status evidence in one gate: the canonical strict run observes load
	  application, deterministic scale-out/status quorum, and scale-in quorum in
	  cycle 1, and the public-profile strict run passes the same combined evidence
	  gate. The repeated two-cycle localnet also passes under the stop-top-up-after
	  transition-quorum policy.
  Hardened localnet soak evidence now includes a clean 300-second run refreshed
  on the 2026-07-03 Nexus consensus
  metadata snapshot with 10 cycles, 0 retries, and 0 failures, plus an earlier
  clean 300-second run with 11 cycles, 0 retries, and 0 quorum
  misses, plus a full 30-minute run with 32 cycles, 0 retries, 0 attempt
  failures, and 0 fresh scale-out quorum misses. Public Taira read-only MCP
  rollout evidence now passes against `https://taira.sora.org`, covering native
  MCP negotiation, curated `iroha.*` tool exposure, public status/Sumeragi
  health, and the public SCCP/ZK/validator/public-lane/contract/Musubi/bridge
  routes. Public Taira SoraFS read-only rollout evidence also passes, covering
  the SoraFS route surface and capacity-state read path. The signed SoraFS
  rollout canary now also rejects malformed numeric operator controls before
  transaction submission or capacity-state polling, and requires an explicit
  runtime-only `--write-config`. It no longer creates or funds a signer.
  Public SoraFS rollout HTTP probes now also use
  JSON `Accept` headers plus bounded curl connect/overall timeouts so `/status`
  content negotiation and stalled public edges fail cleanly. The same SoraFS
  rollout smoke now gates read-only promotion on positive `/status.blocks`,
  healthy Sumeragi commit-QC height, and at least four commit-QC validators.
  Local SoraFS rollout mock coverage now exercises
  read-only no-submit behavior, explicit signer-config preservation, missing
  config failure, unfunded-signer diagnostics without an automatic retry,
  stale validator instruction dispatch, missing capacity-state visibility after
  submit, malformed node-health responses, and malformed canary/timeout
  controls. The remaining live Taira
  validation track is signed write evidence, specifically the generic MCP write
  canary and signed SoraFS capacity declaration canary, which intentionally
  still require explicit live-state mutation approval. The current dirty Nexus
  tree has also been refreshed through the focused core and public API
  lifecycle gates: committed-autoscale drift revalidation, the broader core
  autoscale transition suite, the core lane lifecycle suite, and the grouped
  Torii `nexus_lifecycle_endpoint` module all pass on this snapshot.


<a id="record-1e4c8090e1daffc91033d679c527530cdae304999759eeb2fc170f54bcaecf57"></a>

- NPoS lane-scope inference now uses each public-lane validator's exact retained
  `[activation_height, deactivation_height)` tenure when deriving live recovery
  candidates and active topologies, so lifecycle labels cannot prematurely
  revoke authority and stale records from a retired or rebound lane cannot pin
  independent-lane recovery to a dead scope.
  Public-lane validator rows must now also have storage key `(lane_id,
  validator)` fields that match the embedded `PublicLaneValidatorRecord` before
  live topology, stake snapshots, validator-election profiles, due activation,
  penalty locators, staking admission checks, Soracloud runtime-authority
  checks, or host-finance stake accounting consume them, preventing malformed
  stale rows from auto-promoting to active, inflating quorum weight, joining an
  NPoS roster, reserving validator capacity or peer bindings, granting runtime
  authority, or redirecting penalties to a mismatched validator slot.
  Lane reset paths now also bound and retire revivable public-lane validator
  records for reset lanes, covering
  direct config swaps, manual lifecycle retirement, and autoscale scale-in.
  Authoritative lane validator and peer resolution now also rejects lanes
  absent from the active lane config, or whose dataspace is absent from the
  active dataspace catalog, so stale manifest bindings or active public
  validator records cannot revive a removed or rebound lane committee. The
  global NPoS epoch stake snapshot now uses the same active lane/dataspace
  guard before public validator records can influence topology scope, council
  member mapping, or stake-ranked candidates. When Nexus is enabled, live NPoS
  active-topology derivation, roster-unavailability recovery candidate
  selection, block-sync sender-lane roster caching, and block-apply peer
  reconciliation now also intersect validator-derived lane scopes with the
  active lane/dataspace catalogs. State-backed commit stake snapshot
  construction and roster-validation cache refreshes now filter stake maps to
  active Nexus lanes, so stale higher-stake records on unknown or retired lanes
  cannot override a validator's active-lane weight. State-backed QC and
  block-sync validation fallbacks now also recompute missing NPoS stake
  snapshots with the active-lane filter. Live NPoS commit quorum status,
  local quorum-completion checks, commit-root signer selection, NEW_VIEW
  aggregation, and repair fanout/coverage telemetry now feed the same
  active-lane set into world-backed stake quorum math, leaving remaining work
  focused on end-to-end independent-lane rollout evidence rather than stale
  unknown-lane stake admission.


<a id="record-fd0e89ecc5321193d1debb13d6e606b40164a2981e92be602d95b86cf2f94faf"></a>

- Autoscale-managed elastic lane authority is now pinned before the new catalog
  and incarnation commitments are derived. Creation-time selection gives an
  explicit manifest precedence over the live commit topology and rejects
  undeclared, duplicate, missing, pending, future-active, disabled, expired, or
  under-quorum candidates without topping up an explicit manifest. The exact
  sorted BLS peer set and aligned proofs of possession then remain immutable for
  the whole incarnation: proposal, availability, prepare/commit QC, NewView,
  relay, Native AMX, and drain verification do not fall back to later manifests,
  global rosters, or live-key records. Adversarial coverage rotates to a
  disjoint current roster, removes live keys, injects stale/under-quorum
  manifests, and misaligns pinned PoPs; historical certificates remain valid
  only under the committed pin, while forged authority is rejected without
  populating the merge-relay cache. Operators must drain and retire affected
  lanes before rotating away the pinned quorum.


<a id="record-0e9a6ea4828a44d757ba06b11a1bb3ac01c3e3675d953ae7c1e2d204d8242cb9"></a>

- Autoscale scale-out eligibility now also requires an actually free elastic
  lane id in `autoscale.min_lane_id..autoscale.max_lane_id_exclusive`, so public-profile
  catalogs whose default-route capacity is below `max_lane_id_exclusive` but whose elastic
  id range is full fail closed without recording a transition.


<a id="record-99ff5da13058bc7b99ca7c12645682c194d134cd5ac10b213b2d7c2b6e34a4e2"></a>

- Autoscale scale-out now treats either sustained p95 latency pressure or
  sustained p95 utilization pressure as enough to add managed capacity, while
  scale-in still requires both latency and utilization to remain cold. Default
  route capacity now matches router-owned candidates exactly: the configured
  default lane plus valid managed elastic lanes. Unrelated public-profile base
  lanes below `autoscale.min_lane_id` no longer dilute utilization or suppress
  scale-in/scale-out decisions, while valid managed elastic lanes can still
  retire down to the fixed default-route floor.


<a id="record-1503092be7d2ba51f456103f57f88d19601d2adc7d714d26284b26252c5c4e91"></a>

- Manual lane additions, full config swaps, and static TOML parsing now reserve
  the enabled autoscale elastic id range for the consensus autoscaler, so
  operator-managed lanes cannot occupy future scale-out ids and silently cap
  default-route horizontal growth. Static TOML parsing also rejects
  `nexus.autoscale.enabled=true` when `nexus.enabled=false`, preventing shadow
  autoscale settings; the `State::set_nexus` runtime boundary now enforces the
  same disabled-profile guard for direct actual-config swaps. Runtime lifecycle
  validation also rejects post-plan catalogs that would preserve a pre-existing
  manual lane in that range, while still allowing an explicit retire plan to
  repair the bad manual lane. The same post-plan scan now rejects unrelated
  lifecycle updates that would preserve an autoscale-owned lane with malformed
  metadata, disabled autoscale, an out-of-range id, or a non-default dataspace
  binding.
  Explicit lifecycle retire plans may remove those invalid autoscale-owned
  lanes for repair, while valid autoscale-managed lanes remain protected from
  manual retirement. The internal autoscale lifecycle path now rejects
  unmanaged/manual additions as well as unmanaged/manual retires, so the
  owner-only flag cannot be used as a generic lane creation bypass outside the
  reserved range. Scale-in transition regression coverage now also exercises
  cold windows with a valid managed retire candidate plus manual, malformed,
  off-default, or out-of-range elastic-range corruption, proving corrupted live
  state cannot retire healthy managed capacity or record a transition.


<a id="record-949a29cb4dee6232a2e7843cfcb3f7f77a658c2097c02c5922df9051382f1665"></a>

- Autoscale configuration, runtime config swaps, lifecycle post-plan
  validation, and block application now reject a `routing_policy.default_lane`
  inside the enabled autoscale elastic id range. The default route remains a
  base-lane anchor and cannot be rebound to an autoscale-owned elastic lane.


<a id="record-018f9144bb6afd03256d2d0ca1121d20c83b0d04878fa4e67e2719dec9fbbcc8"></a>

- Canonical dataspace routing now ignores lanes that claim autoscale ownership,
  including malformed claims, so autoscale elastic lanes cannot become
  dataspace/settlement/permission-scope anchors and dataspace anchors fail
  closed when only autoscale-owned lanes exist.


<a id="record-c01381ce3bbbc23ea49387ad87acc53100f1cbbdff5a011938527e412b0ee294"></a>

- Autoscale transition coverage now also pins fail-closed behavior for
  corrupted default-route bindings and incomplete historical Kura sample
  windows, so hot current-block counters cannot trigger catalog mutation
  without a routable default lane and complete persisted history, and cold
  windows cannot retire managed elastic lanes under those same partial-state
  conditions. Equal or backward block timestamps are now treated as incomplete
  timing evidence too, rather than being clamped into synthetic hot/cold
  samples.


<a id="record-f761c533c2a1971d8347d7ecbbd2b373f0cfba4eab01314b3dfa46560e1794a2"></a>

- Autoscale retire selection and the internal autoscale lifecycle now enforce
  the exclusive `autoscale.max_lane_id_exclusive` bound as well as the lower bound, so
  corrupted managed lanes outside the configured elastic id range cannot be
  silently destroyed by the autoscaler and must be removed through an explicit
  repair retire.


<a id="record-cc2df68fa97da31ce9482344e77f1efc7ab72e359ebaf07d76f93fa12efb1221"></a>

- Autoscale block application now prechecks the active
  `autoscale.min_lane_id..autoscale.max_lane_id_exclusive` range and any autoscale-owned lane
  outside that range before deterministic transitions. Occupied in-range ids
  must already be valid autoscale-managed default-dataspace lanes, and
  autoscale-owned corruption outside the range blocks plan construction until
  an explicit repair retire
  removes it.


<a id="record-d0481601ae52ce0769a7422e9bd84883d298006e7adfb8b746f90c49a67e072f"></a>

- Pending queue-plan journal replay now synchronizes queue-local Nexus routing
  from committed state before comparing persisted route plans, and tombstones
  stale journal records whose lane/dataspace assignment no longer matches
  current policy even when the old lane still exists. Restart coverage now also
  pins the same-lane dataspace rebind case, where a stale journal plan names a
  lane id that remains active but the committed lane binding and dataspace
  catalog have moved to a different dataspace. Restart replay now also
  tombstones stale elastic default-route plans when active elastic-range
  corruption makes live routing fall back to the base lane. Native AMX journal
  replay also compares participant legs from the full recomputed plan, so a
  restart tombstones stale participant routes even when their old lane still
  resolves against the active catalog.


<a id="record-3c29458df20f2d043960dcdf8612cea8ead92220d91afd93ec27e58a0c296299"></a>

- Proposal routing refresh now resolves full plans from the same live Nexus
  snapshot and autoscale elastic range, so proposal sidecars and execution
  context routes preserve autoscaled default-route assignments instead of
  falling back to catalog-only base-lane routing. The refresh compares full
  routing plans, so Native AMX proposal vectors also replace stale participant
  legs even when the coordinator route is unchanged. Proposal size-cap trimming
  preserves full routing plans for removed transactions too, so overflow requeue
  keeps Native AMX participant metadata. Gas-capped proposal assembly now
  defers an oversized first candidate when a later scanned transaction still
  fits the remaining gas and IVM budgets, so one gas-heavy lane cannot suppress
  fitting cross-lane work under multilane lookahead. Pending queue
  reconfiguration now keeps queued default-route transactions and local
  routing-ledger hints on the active default route until an autoscale elastic
  lane's creation height is committed. Proposal lookahead now gates on
  policy-reachable active lanes at the candidate block height rather than raw
  catalog overrides, so autoscale-owned default anchors, off-default
  autoscale-owned rule targets, unrouted same-dataspace sidecars, and
  future-created autoscale lanes cannot cause scan-budget overfetch while the
  committed routing surface is still effectively single-lane.


<a id="record-e3b6912ebef8bdca0be07193400e3c78c06908bf4c98487cba23deba49f884d6"></a>

- Commit event production now consumes the full routing plan before any legacy
  coordinator-only routing hint, so partial ledger cleanup or stale single-route
  metadata cannot override digest-checked lane/dataspace metadata when both
  ledger shapes exist. Queue-side expiry and unresolved-route rejection events
  now use the same full-plan-first cleanup when both cached plan and legacy
  coordinator metadata exist, so stale route shadows cannot mislabel terminal
  pipeline events after a routed transaction is removed. Shared routing-ledger
  plan discard also clears same-hash legacy shadows after the expected full
  plan is removed.


<a id="record-008bd6f22926047816c4f6b8813341fac92dc537ba3792f431dcf2a7f518ed86"></a>

- Block validation and block execution now recompute execution-context routing
  and per-lane transaction summaries from that same live Nexus autoscale range,
  so validators accept matching elastic default-route contexts and reject stale
  base-lane contexts for transactions routed to elastic lanes. Durable Native
  AMX contexts also compare every committed coordinator and participant leg with
  the recomputed full plan before receipt validation. Per-lane committed TEU
  telemetry is now attributed from the validated block routing vector instead of
  process-global routing-ledger hints, so stale cached routes cannot skew
  scheduler lane-load metrics.

