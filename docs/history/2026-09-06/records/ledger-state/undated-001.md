# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-a94faab561091217c5e39b36e2c131484c38bd842ce56a9c1af55a418aba1330"></a>

<!-- Original context: Roadmap / Iroha Core first-release closure -->
## Iroha Core first-release closure



<a id="record-d76628d205e0a4f8603009502b3f53bd0d01343f993c30abd88487f1ce812343"></a>

- Split the remaining `state.rs` monolith along existing ownership boundaries:
  world storage/schema, snapshot restoration, canonical merge admission,
  transaction execution, and derived caches. Keep one atomic block overlay and
  one canonical serialization definition; do not introduce facade layers that
  merely rename storage operations.


<a id="record-a5ff453f5ba3084c6bdbeb89b05a1586d5dc7eca1a47b8ac02ad58164c27eca8"></a>

- Replace the two remaining stored/ephemeral `QueryItemKind` match tables with one typed registry
  declaration that generates both execution modes and an exhaustiveness test. Keep canonical exact
  payload decoding and source-budget admission explicit; do not restore boxed or compatibility
  dispatch.


<a id="record-367615959711323f92e2b7400da63325039c2258d2b1065908e098958714687a"></a>

- Remove the crate-wide Clippy suppression in bounded module groups. Delete or test-gate each
  resulting dead surface, and add a CI target that checks the production library without test-only
  features so unused shipping code cannot accumulate again.


<a id="record-a3f1df1efd2ce1368f44fa1034e343fa8688ba43512e7e6585acd1f9eb8f9e20"></a>

- Move the remaining used invariant-bypassing `*_for_testing` state mutators
  behind `cfg(test)` or the narrow `iroha-core-tests` feature. The strict
  block-commit migration now gives core, CLI, Torii, and daemon fixtures
  explicit world-overlay or empty-block helpers and an explicit build matrix;
  retain that boundary while migrating the remaining mutators. Add a
  production-source reachability gate so definition-only WSV facades and raw
  mutable index accessors cannot return. Preserve historical-format rejection
  and recovery detection that fail closed; those are safety controls, not
  compatibility support.


<a id="record-933c3110e22c459c6f40a5adf0b394afa17ed07ea3f7d0713f11c1f123fd83f5"></a>

- Add deterministic benchmarks for query dispatch/metering, block admission, state transitions,
  and startup indexes. Use the profiles to remove avoidable encoding, cloning, and full-collection
  scans while asserting identical outputs across hardware paths.


<a id="record-55b70b243645aee19325f3f46d035f750aea4203edc656bed242c6f7a13b6134"></a>

- Qualify the canonical-only autonomous lane path on a settled four-validator
  candidate, including compact merge-carrier restart, exact transaction
  membership, rollback/replay, lane reset, and snapshot restore. No direct WSV
  application or legacy receipt/snapshot migration path may be reintroduced.



<a id="record-483409e2328fd132198abb6e070f9e11d71911bc2834d54095d8d09268073197"></a>

<!-- Original context: Roadmap / Kura emergency-start closure -->
## Kura emergency-start closure



<a id="record-48c3449fdc29b5fe2816d6de2a10fb2296202c912bae09c9b4809b4cf019413b"></a>

- Keep merge and autonomous production unavailable in emergency Fast mode. If operators later need
  those write paths before a Strict restart, first add an atomically published merge-log
  count/length/tip marker and durable latest-route summary; Fast must never recover that authority
  by decoding every historical frame. Read-only Fast startup must not require the deferred merge
  log or inspect auxiliary recovery artifacts; it leaves them untouched for Strict. Preserve the
  library-level writer-start, durable-mutation, and sidecar-queue rejection tests so a daemon
  refactor cannot accidentally make Fast producing. Preserve byte-exact recovery-artifact tests
  for public Fast reads and authorization-transition tests proving queued work is never drained
  after Kura becomes poisoned or unauthenticated.


<a id="record-e781c17ff09cd1bef5b7bacf62c13a46d0044883baead5bcd2a8ab7ff8f0f148"></a>

- Add large-history startup benchmarks that separately report marker preflight, hash-journal
  binding, five-artifact metadata binding, bounded-manifest authentication, exact-tip validation,
  minimal-State construction, merge metadata, Sumeragi replay planning, and recursive disk
  accounting. Enforce zero-copy historical-hash, no-historical-body, no-snapshot-payload-read,
  no-Merkle-read, no-current-World-decode, no-derived-index, no-full-StateView,
  no-deep-snapshot-validation, and no-historical-cryptography regressions for Fast mode without
  weakening the signer, network, configured confidential-policy, size, or exact-tip boundary.



<a id="record-88f88c2b5f1861086d33a5ab171c8f50cca9e41069a0a3b38f4ffaaa2f069ea3"></a>

<!-- Original context: Roadmap / Torii first-release closure -->
- Make DA spool recovery transactional: journal server-owned timestamps and
  intended artefact bytes before the first immutable write, then resume or
  quarantine receipt-less partial transactions deterministically after
  restart.


<a id="record-f3ca57ace4700e0fc042cd6ad26e104f00bff30a85dc2bf9724a73b92eeeebb1"></a>

<!-- Original context: Roadmap / SORA Nexus and Taira -->
- Stateful default-route sharding now applies live autoscale enablement and the
  same elastic id range before admitting autoscale-managed candidates, so
  disabled autoscale or corrupted out-of-range managed lanes cannot receive
  ordinary no-target default traffic. Runtime autoscale bounds above the
  compiled cap or a default lane inside the elastic range also disable elastic
  sharding, keeping no-target traffic on the configured default lane until the
  state is repaired. Catalog-only default routing now also stays on the base
  default lane unless a live Nexus state view supplies autoscale enablement and
  bounds, preventing stale router snapshots from selecting elastic lanes after
  scale-in or autoscale disablement. State-free router fast paths now also
  defer unmatched no-target default traffic, including no-target IVM/proved-VM
  traffic, to live-state routing even when unrelated policy rules exist, so
  unmatched rules cannot bypass the autoscale elastic range and pin default
  traffic to the base lane. State-free query routing, non-fallible state-free
  hints, and live non-fallible routing fallbacks now reject autoscale-owned
  default or explicit-rule lanes instead of treating elastic lanes as
  operator-configured anchors.


<a id="record-6c3730ffc2d34d46d2095ba3816e6094512b06184c2d67e73411b4448b4cd251"></a>

- Transaction gossip route hints also resolve against the active dataspace
  catalog before broadcast or reinsertion, so dangling lane bindings left after
  dataspace removal are rejected alongside missing lanes and lane/dataspace
  mismatches. Gossip batch partitioning now falls back to actual Norito length
  for variable-size full routing plans, preserving Native AMX participant legs
  across the gossip plane instead of requeueing them indefinitely. Outgoing
  gossip batch assembly also refreshes cached full routing plans from committed
  Nexus state before emitting route hints, so Native AMX participant drift is
  corrected before serialization. Torii submit-transaction proxy receivers apply
  the same full-plan comparison to ingress hints, so Native AMX participant
  drift is rejected even when the coordinator route is unchanged. Inbound
  transaction gossip now pins the same adversarial case with a stale Native AMX
  participant leg and matching coordinator route, dropping only the stale entry
  while preserving a valid entry in the same batch. Non-empty malformed gossip
  batches with short route or plan metadata now use that same per-entry boundary
  instead of dropping the whole batch, so aligned valid entries still enqueue
  while missing-metadata suffix entries are rejected before semantic
  materialization in both owned and shared/lazy paths. Advertised full routing
  plans are also catalog-resolved and checked for canonical byte-equivalence
  before transaction materialization, so unknown Native AMX participant routes,
  forged digests, duplicate legs, or noncanonical route-leg roles cannot force
  semantic decode before being rejected; direct shared/lazy regressions pin the
  forged-digest and duplicate-participant cases.


<a id="record-0f142160da96d77cedc029ca058a8cdc837e8328eb5d4de947941d81296e1afb"></a>

- Stateful transaction validation without caller-supplied routing context now
  resolves the live Nexus full plan before enforcing lane policies, preventing
  direct validation entrypoints from collapsing autoscaled default-route traffic
  back to the catalog-only base lane.

