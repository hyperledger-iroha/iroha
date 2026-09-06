# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-2e45762d75c460dfc4c47930448e31813f7972c57effed7942c6318c8a4cf902"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- Sumeragi prepare-quorum phase-gating validation is closed for the current
  formal slice: the dedicated 2026-06-03 Apalache fast run reached `NoError`
  up to computation length `10` with `CommitPhasesNeverBypassPrepareQuorum`
	  loaded, and the formal coverage guard now reports `505` PR modes,
	  `9873` expected-failure modes, and `10379` documented modes.


<a id="record-ac8a1310aa613f949fbccc4677360e01d256d8b5008337aba5eadade28b702f1"></a>

<!-- Original context: Roadmap / SORA Nexus and Taira -->
- Use the public Taira testnet to harden consensus, routing, lane-aware
  execution, data availability, operator workflows, and SDK integration.


<a id="record-c77bb27dad21c524dbc264df2134da2968ccf3cb3aefbeaa9cffb21f9768e338"></a>

- Validate and roll out the completed independent-lane consensus, DA/RBC,
  autoscale lifecycle, and globally ordered cross-lane merge corridor needed
  for the first public Nexus release.


<a id="record-b66d0e5ea666bb8cc9ba73096aa3eac050ea4f4a1b562e8588ca19d2888bfb6c"></a>

- Runtime lane lifecycle plans now reject duplicate addition ids, duplicate
  addition aliases, and duplicate retire ids at the catalog boundary, so
  malformed public lifecycle requests cannot rely on implicit deduplication and
  failed plans leave the active lane catalog unchanged; the Torii
  `/v1/nexus/lifecycle` endpoint covers the same duplicate-addition rejection
  and duplicate-retire destruction rejection paths through signed operator
  requests and now pins direct default-lane retire plus same-plan default-route
  replacement as public API errors without catalog or queue-limit mutation.
  Empty signed lifecycle plans are now rejected before catalog, derived
  lane-config, or queue-local limit mutation, so accepted lifecycle responses
  cannot be produced by no-op operator payloads.
  Accepted signed add/retire plans are also covered at the endpoint wiring
  layer to prove lane-specific queue limits refresh from committed metadata and
  clear back to fallback values after retirement.
  Signed malformed JSON and invalid-topology lifecycle payloads carrying
  lane-specific queue metadata are pinned as public-route failures that leave
  both committed catalogs and queue-local limits unchanged.
  Unsigned and body-mismatched lifecycle requests are pinned as endpoint-level
  auth failures that preserve both the committed catalog and queue limits; the
  same public-route coverage now rejects exact replayed mutations and non-node
  operator keys before they can apply lane catalog or queue changes.
  Core autoscale failure coverage now also proves failed internal scale-out and
  scale-in lifecycle attempts do not leave a pending staged catalog update for
  commit.
  Autoscale ownership markers are now reserved as a pair: operator-authored
  lanes cannot carry either `autoscale.managed` or `autoscale.created_height`
  through signed lifecycle plans, external Nexus config swaps, TOML config
  parsing, runtime autoscale prechecks, capacity accounting, or router
  resolution. Marker-only corrupted lanes can be explicitly retired for repair
  but cannot be preserved as manual lanes, selected as canonical/default/rule
  routes, or counted as router-owned capacity. Core and Torii endpoint
  regressions now also pin the valid-looking spoof shape where an
  operator-authored lane uses the canonical elastic alias plus both reserved
  autoscale markers.
  Runtime autoscale transitions also reject autoscale-managed lanes whose
  `autoscale.created_height` is greater than the current block height before
  capacity, scale-out, or scale-in decisions run, so corrupted future-created
  lanes cannot be counted by the autoscaler. Unrelated lifecycle/config updates
  cannot silently preserve future-created autoscale lanes, while explicit
  operator repair-retire plans can remove them; Torii endpoint coverage now
  also proves that repair-retire calls for invalid-height, future-created,
  out-of-range, off-default, and disabled-autoscale owned lanes clear stale
  lane-specific queue limits while rejected unrelated plans and active-lane
  manual-retire attempts leave the catalog and queue cache untouched.
  Operator-authored additions carrying reserved autoscale markers or ids inside
  the reserved elastic range are also pinned as no-side-effect endpoint
  rejections even when the rejected lane metadata includes scheduler capacity
  overrides.
  Generic malformed lifecycle plans and disabled-Nexus lifecycle attempts,
  including duplicate additions or retires, unknown retire targets, unknown
  dataspace additions, and default-route retire/replacement attempts, are
  likewise pinned as no-side-effect public API failures for both committed
  catalogs and queue-local scheduler limits.
  The core queue lifecycle helper now directly proves rejected autoscale-reserved
  scheduler metadata cannot refresh queue-local lane catalogs or TEU limits
  before the state catalog accepts the plan, and accepted repair-retire plans
  clear stale per-lane TEU overrides after future-created autoscale lanes are
  removed from the state catalog.
  Live-state default-route sharding applies the same committed-height boundary
  before selecting elastic lanes, preventing future-created lanes from receiving
  default-route traffic or
  appearing in route plans. Block execution, static execution-context
  revalidation, transaction validation without embedded routing context, and
  proposal routing refresh now use a block-height-aware Nexus/world routing
  helper so candidate block height, not stale committed view height, decides
  whether an autoscale lane is active. The older heightless Nexus/world routing
  helper now fails closed to the configured default route for no-target default
  traffic instead of sharding over autoscale lanes without a candidate height.
  Deterministic autoscale scale-out now also rejects internally generated lane
  additions unless `autoscale.created_height` equals the current transition
  block height, so stale or future-dated owned lanes cannot be staged by the
  production lifecycle path. Internal autoscale transition metadata is now
  checked against the exact lifecycle plan shape as well, so scale-out cannot
  stage anything except one addition for the logged lane and scale-in cannot
  stage anything except one retirement for the logged lane. The recorded
  `active_lanes` and `autoscale_capacity_lanes` values must also match current
  default-route autoscale capacity before staging, preventing stale capacity
  evidence from being logged with a valid add/retire plan. The pending
  transition height, rederived plan shape, capacity metadata, exact
  previous-catalog lifecycle replay, derived lane config, and scale-out
  creation height are now revalidated again at block commit before autoscale
  storage geometry is published, so tampered staged metadata cannot
  durable-publish a valid catalog delta. Scale-in commit-revalidation coverage
  now proves tampered pending transition metadata, survivor catalog rows,
  preserved retired-lane catalogs, and derived lane configs abort before
  retired-lane Kura/tiered storage or committed catalog publication, matching
  the existing scale-out tamper boundary. Direct queue admission and restart
  queue-plan journal replay now also pin forged future-created autoscale route
  plans against the live height-aware route before they can enter the pending
  queue or local routing ledger. State-backed queue route resolution now also
  checks every resolved coordinator and Native AMX participant leg against the
  active-height Nexus predicate, so stale state-free route hints cannot target
  future-created or otherwise inactive autoscale lanes during admission, gossip
  routing, proposal refresh, pending reroute, journal replay, or requeue, while
  requiring every concrete dataspace to have one exact catalog lane. Dynamically
  discovered SNS dataspaces without an explicit lane now fail closed instead of
  borrowing the universal lane. The queue and transaction-gossip negative
  fixtures use canonical autoscale elastic lane shape and assert activation at
  the declared creation height, so those regressions prove the height gate
  rather than malformed-lane rejection.
  Lane relay authority applies the same activation-height boundary before
  accepting manifest-bound or commit-topology-derived validator sets for
  autoscale elastic lanes, so relays for not-yet-created lanes cannot be
  accepted or cached, and corrupted manual lanes inside the reserved autoscale
  elastic id range are denied relay authority as well; record-level relay
  admission now checks lane activity before stale emergency overrides can fill a
  committee, and block-local autoscale lifecycle cleanup applies the same
  reset-or-inactive-lane pruning before automatic scale-out/scale-in commits can
  preserve stale emergency override rows. The active-lane authority boundary
  now also requires the caller-supplied dataspace to match the lane catalog and
  remain present in the dataspace catalog, so forged dataspace context or
  removed dataspace bindings cannot keep a lane authoritative. Manifest
  validator fallback reads now also
  hide explicit peer bindings whose consensus keys are pending, future-active,
  disabled, or expired while keeping the raw manifest installed to suppress
  unsafe topology fallback. Manifest, commit-topology, and stake-derived
  authoritative lane peer sources, plus account-level authoritative validator
  reads, now also require world peer membership plus a live consensus key before
  exposing a peer or validator account as authoritative. Account-level manifest
  authority now follows explicit validator-to-peer bindings before falling back
  to validator-account signatory inference, so operator BLS peer bindings do
  not require the validator account key to be the consensus peer. Explicit
  manifest validators and bindings are now exposed only when the declared
  validator list is duplicate-free, each binding names a declared validator, and
  the binding list has no duplicate validators or peers before public binding,
  route-authority, peer-authority, or account-authority reads consume them, so
  undeclared or duplicate manifest rows cannot inflate validator or peer weight.
  Protected governance admission and transaction state validation now consume
  the same duplicate-free canonical validator set before authority or quorum
  checks, so duplicate validator rows fail closed instead of being deduplicated
  by one boundary. `gov_manifest_approvers` quorum metadata is now
  duplicate-free too: repeated approver claims reject admission and state
  validation instead of being collapsed into one approval. Manifest loading now
  also rejects duplicate protected namespaces and duplicate runtime-upgrade
  allowlist ids after trimming, so manifests cannot carry shadow duplicate
  policy rows. Duplicate manifest filenames that resolve to the same lane alias
  in one source directory now invalidate that alias and keep the governed lane
  locked until the duplicate source is removed.
  Merge
  candidate synthesis and commit-time merge
  snapshot validation now apply the relay snapshot height before accepting lane
  snapshots, so stale relay-store entries and persisted verified relay
  contract-state records for future-created autoscale lanes cannot hydrate,
  restart-hydrate, or merge; verified relay contract-state cleanup also prunes
  exact canonical reset-lane keys even when the decoded payload claims a
  survivor lane, without using payload-only claims to delete unrelated
  contract-map keys.
  Public manifest validator-binding reads now also use the committed
  lane-authority height, so Torii's manifest-preferred authoritative-peer and
  proxy candidate paths cannot expose future-created autoscale lanes before
  activation.
  Lane-relay emergency override admission now uses the same
  height-aware active-lane dataspace boundary before storing override validator
  sets, so future-created autoscale lanes cannot gain emergency relay authority
  through a catalog-only check. IVM
  AXT policy snapshot derivation, cached-policy reads, and policy cache
  rebuild/prune paths now also filter target lanes through that height-aware
  active-lane boundary, so Space Directory manifests and stale cached policies
  cannot expose or persist future-created autoscale lanes before activation.
  Stake/QC active-lane
  filters now also exclude autoscale-reserved lanes and manual occupants of the
  elastic range before public validator stake can contribute as ordinary active
  lane stake. Public-lane staking ISIs now reject inactive lanes at the
  transaction block height, and Torii public lane validators/stake/rewards reads
  hide stale persisted rows for future-created autoscale lanes at the committed
  lane-authority height. Soracloud runtime validator-authority and HF placement
  stake aggregation now use the same transaction-height active-lane boundary, so
  future-created autoscale validator rows cannot authorize runtime actions or
  inflate host placement weights before activation. Sumeragi NPoS penalty
  derivation, penalty application, and election candidate-profile assembly now
  apply active-lane filters before stale future-created autoscale rows can jail,
  slash, or weight validator candidates. Torii route discovery now applies the
  same active-lane filter before exposing direct dataspace routes,
  all-dataspace fanout, restricted ingress routes, or public-dataspace
  visibility, and explicit public-lane route resolution plus receiver-side
  read/verified-query proxy validation reject future-created autoscale lane
  hints as stale routes. Transaction gossip validation now applies the same
  active-lane boundary to advertised routes and full Native AMX routing plans
  before materialization or local admission. Soracloud status routing now
  separates configured lanes from active lane ids/count and live
  autoscale-capacity lane ids/count, reports sparse configured lane namespaces
  separately from declared metadata entries, and accepted Nexus lifecycle
  responses mirror the active/capacity split while preserving legacy
  `lane_count` as configured count. The generated Torii OpenAPI surface and
  latest/current portal snapshots now list `/v1/soracloud/status` with that
  routing-field split, document `/v1/nexus/lifecycle` as a `202 Accepted`
  `NexusLaneLifecycleResponse` operator action, and the Nexus operator docs
  describe the expanded lifecycle response payload. Autoscale transition
  telemetry now derives `active_lanes` from live default-route autoscale
  capacity rather than raw configured catalog length, preventing unrelated
  configured dataspaces from being reported as active horizontal capacity in
  scale-out/scale-in events. Autoscale localnet transition evidence now rejects
  logs whose `active_lanes` and `autoscale_capacity_lanes` disagree, so stale
  pre-capacity-split output or forged counters cannot satisfy transition
  evidence checks; zero active/capacity lane counters are also rejected as
  non-evidence. Fresh transition deltas now require matching per-peer baseline
  entries, so truncated baseline snapshots cannot make stale scale-out or
  scale-in counters satisfy quorum evidence.
  DA active proof-policy admission
  rejects manual lanes in the autoscale elastic range and malformed
  autoscale-reserved metadata while still
  advertising proof policy for valid autoscale elastic lanes. State and
  consensus DA commitment validation now enforce that same proof policy at the
  candidate block/proposal height, so future-created autoscale lanes cannot
  validate commitments before their declared creation height. DA pin-intent
  sanitization, inbound block validation, state ingestion, Kura replay, and
  feature-gated proposal assembly coverage now apply the same candidate-height
  rule before sealing, hydrating, or accepting pin intents for autoscale lanes.
  Block validation and proposal assembly now compute DA proof-policy header
  hashes against the block/proposal height as well. State DA commitment
  materialization and Kura replay use the commitment block height for
  current-catalog query/identity visibility, while preserving canonical
  historical bundles for removed lanes, and Torii DA ingest selects proof
  schemes from the committed-height policy view.
  Torii DA commitment/proof-policy/pin-intent read APIs now use
  committed-height policy snapshots plus proof-block-height and
  intent-block-height active-lane checks, so corrupted future-created
  autoscale lanes cannot be advertised, listed, proven, or verified through
  public read surfaces.
  Default-route autoscale capacity now counts fixed default-dataspace base
  lanes below `min_lane_id` and the configured default-route anchor itself toward
  the scale-in floor, including valid anchors above the reserved elastic range,
  while still ignoring unrelated manual lanes outside the elastic range. Live
  default-route sharding now has matching high-anchor regression coverage so
  no-target traffic can use the high anchor plus in-range autoscale lanes but
  cannot leak onto unrelated manual sidecars.
  Accepted scale-out plus public-profile scale-in tests pin the staged pending
  lifecycle height, catalog, lane-config, reset lanes, and empty replacement
  set.
  Same-id dataspace rebind pruning coverage now exercises a non-default lane,
  keeping default-route replacement rejection intact while still proving
  lane-scoped state is reset across lifecycle rebinds.
  Same-plan replacement preflight coverage now also includes Kura merge-ledger
  and tiered snapshot target collisions, proving those failures preserve the
  committed catalog, source storage, and untouched conflicting targets instead
  of partially applying physical geometry.
  The routing-policy validator also
  resolves rule lanes without explicit dataspaces against the default dataspace
  and rejects explicit rules that target autoscale-owned lanes, so elastic lanes
  cannot be pinned by policy rules outside the autoscaler. Fallible router
  resolution now enforces the same ownership boundary for corrupted in-memory
  explicit-rule and default-lane policies before returning a route or routing
  plan.


<a id="record-bc6160e37faf19d660b31da22d94a214a8b241edc04a3757d9b75bd0e9f2be99"></a>

- State-aware admission, gossip reinsertion, batch admission, consensus requeue,
  and block requeue paths now synchronize queue-local Nexus routing from
  committed state before accepting caller-provided routing plans. Those
  precomputed plans must resolve every coordinator and participant leg against
  the active catalogs and exactly match a freshly recomputed full plan for the
  same transaction, so stale route plans cannot survive policy changes solely
  because their old lane remains catalog-valid. Lane TEU deferral also returns
  full routing plans for consensus requeue, so deferred Native AMX transactions
  keep participant legs instead of requeueing as coordinator-only work. Queue
  reconfiguration after committed Nexus changes refreshes cached full Native
  AMX routing plans for pending transactions through both state- and
  view-backed entry points too, so participant legs cannot remain stale behind
  an unchanged coordinator route. Block requeue now discards stale
  process-global routing-ledger plans after failed ledger-sourced reinsertion,
  so the next recovery pass recomputes Native AMX participant legs from current
  committed state instead of replaying the same stale hint. Torii
  submit-transaction proxy receivers also validate canonical route-leg roles
  and the advertised Native AMX `plan_digest` before comparing ingress hints to
  the receiver-recomputed plan, and route-plan hint conversion is now
  fallible-only so forged proxy hints fail as malformed input instead of being
  normalized into a fresh plan.

