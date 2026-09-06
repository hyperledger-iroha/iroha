# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-a2dcf7ae394650a0f8348b15e2fbeaa7b7f986c19d700ce099caaab18f396b89"></a>

<!-- Original context: Roadmap / Iroha configuration first-release closure -->
- After the current shared-tree integration edits and unrelated SoraCloud/Nexus
  golden failures are resolved, run the complete `iroha_config_base`,
  `iroha_config`, generator, SDK, daemon, strict Clippy, and workspace test
  matrices from one settled release candidate.



<a id="record-e40b44d28d1d582a3532db61744ee4a7fcde0598f8d32405ae923c858f940ad1"></a>

<!-- Original context: Roadmap / Taikai first-release closure -->
## Taikai first-release closure



<a id="record-6b1e092858aa216877ec7d134779b305694ae4b1dcc0a6881cefa7b8e810a8a3"></a>

- Persist a versioned ingest-proposal journal before the first spool side
  effect. Key it by the replay coordinates and fingerprint, authenticate it
  against the signed request, and reuse the exact server queue time, PDP
  commitment, receipt signature, and commitment record on interrupted retries.


<a id="record-8e672d20ecb435d234f1f1ce526afbaa7d4cfb9d2f4a93726bad49f763d7cd90"></a>

- Retain authenticated TRM lineage history by artifact identity instead of only
  the latest alias head, so an exact staged retry remains recoverable after a
  later routing window advances. Historical retries must never emit a second
  alias-rotation event.


<a id="record-e9e3449fa22b99241113c7b4cf7c7aa01acaf0b0a7ed3764169a62a4ffcd8c1a"></a>

- Provide one canonical publisher-side builder for the exact Taikai
  `DaManifestV1`, envelope, CAR commitment, alias proof input, and SSM preimage,
  with byte-parity tests against Torii's enforced retention and rent policy.
  Publishers should not have to duplicate private admission algorithms.


<a id="record-7410583ed738faab14e3979b9b1c63d38c0f128cce8d9d6ab4b4f8d7993aea7d"></a>

- Consolidate the remaining repeated TRM, SSM, envelope, and track checks into
  checked V1 constructors shared by publishers and Torii. Add a production
  reachability guard requiring every future signed field to have an explicit
  admission, routing, persistence, or viewer owner before changing the
  first-release wire shape.


<a id="record-4cd0049a10e47b39ce1dc8a03e9b76d6c76f74f93c5c6303b6362cb7f37208bb"></a>

- Extend the borrowed-section CAR verifier to file-backed rehydration with an
  explicit configuration-sourced archive bound, then benchmark retained and
  peak bytes so the CLI does not need to materialize an entire archive at once.


<a id="record-a8c633efbf26a4e966853545e72183a3033fc9910f292d0d9237086afa5967e7"></a>

- Extend the CAR output prepare/commit boundary to the optional summary output,
  then rerun the focused Torii, CLI, xtask, viewer, CAR, integration, and
  full-workspace matrices from a settled candidate.


<a id="record-60e4521038895a9c654b3e35345d52d896ec0880ae7da6cb48c4b2aa3e1b08e1"></a>

- Remediate and verify the mutable-path TOCTOU in the Taikai evidence collector
  reported as the sole Medium-severity collateral finding by sealed Codex
  Security scan `6ab84cf2-4c1f-47e4-9cb0-92d126eb333e`. Receipt validation,
  digesting, copying, and optional signing must consume one immutable artifact
  snapshot rather than reopening attacker-replaceable pathnames.



<a id="record-6d9d42188493cb4ab5bdfe603b979335145cc5b3ba39276bfba069ab2219f238"></a>

<!-- Original context: Roadmap / ZK algorithm release qualification -->
- Keep Kaigi `ZkRosterV1` join admission unavailable until a versioned roster
  circuit binds the canonical signed participant authority (and, if required by
  the final replay policy, the Kaigi action/session context) as authenticated
  public input. Regenerate the deterministic verifier key, exact schema, JS/SDK
  builders, registry fixtures, and front-run/cross-authority negative tests as
  one profile change. Do not treat the existing arbitrary account witness or a
  copied NIZK as bearer authorization. Usage proofs and host-signed lifecycle
  operations are separate and remain valid.


<a id="record-d379d00d95ccf470c0d9df39a584e4e0c58fa1f5a0a24377fed871d25616b53e"></a>

- Profile startup rebuild cost for the derived Kaigi signal locator index on a
  release-scale archive. Add a durable, version-bound rebuild checkpoint only if
  that evidence warrants it; preserve fail-closed reads for incomplete,
  poisoned, or pruning-recovery state and never restore the ledger-history scan.


<a id="record-21a9ae61f55b1437ad0c20471561e9fb2d7cf64378483029aa1ad2146de438e9"></a>

- Replace the first-release 4,096-entry retained Kaigi private-usage log with a
  versioned rolling accumulator before supporting longer fine-grained sessions.
  Preserve deterministic replay protection, bind the accumulator into the
  privacy proof statement, and migrate only through an explicit protocol
  version rather than weakening the V1 record bound.


<a id="record-1ee601b6288a8185089327dbc6e6e01721b05c461bd27f699bde892427b3e238"></a>

- Finish non-Rust Kaigi parity against one Rust-generated nine-instruction
  Norito fixture set. Wire the Kotlin/Java typed templates into canonical
  `WirePayload` encoding and consume the same complete fixture set from
  JavaScript, Python, Swift, Kotlin, and Java rather than maintaining
  handwritten wire expectations.


<a id="record-aec7dd1861c854955de13983ac1c408b96b13970a7535e891a2e27b5cfa8db39"></a>

- Replace the opaque bounded Kaigi HPKE byte vector with a suite-tagged bounded
  descriptor before adding another cipher suite. Preserve the current 4 KiB
  protocol ceiling and 3–8-hop manifest bound, and add suite-specific exact-
  length validation without changing observable results across peers.


<a id="record-8f404caa97ef74d270b01c4789fab39964aba9dd364ce5fdff66a637626cc71e"></a>

<!-- Original context: Roadmap / Musubi first-release registry and developer ecosystem reset -->
## Musubi first-release registry and developer ecosystem reset

The V1 contract, typed registry/Core storage, Cargo-style developer path,
authenticated cache and publication client/service core, Torii/MCP surface,
and Kotlin/Java/Swift package-model and exact twelve-query parity, including
archive retention, are implemented without a migration or compatibility layer.
Historical publication proofs now bind to an unpruned, consensus-persisted,
revision-keyed sparse checkpoint history: genesis and each changed block retain
only their block-final revision, proof admission exact-loads the claimed
activation interval and canonical block anchors, and rollback/replay uses the
ordinary MV state lifecycle. The stale Kotodama/IVM references to the retired
generic confidential instructions are removed; the final focused Core
test-target build remains part of the coordinated validation lane.
The custom daemon launch path is now late-bound: a one-shot factory receives
the exact live chain/genesis, state, queue, and SoraFS handles after trusted
startup replay, while stock `irohad` remains fail-closed. The provider-grade
bundle verifier also has a fresh-reader three-pass path that avoids CAR
materialization and rejects pass substitution and non-exact EOF. Mandatory
bundle metadata now crosses shared exact-slice, canonical, resource-limited
data-model decoders; semantic-release and lock decoding each has a 48 MiB
cumulative allocation ceiling that includes nested field and realignment
charges. Both provider entry points drop the 32 MiB PoR store before metadata
parsing. Supported-target whole-process RSS qualification against the 64 MiB
target remains outstanding, and the separate generic chunk-store ceiling still
needs reduction or measured qualification. Payload-only verification is deliberately limited to authenticated
extractions or admitted chunk storage; raw provider CARs still require the full
canonical-container boundary.
The embedded SoraFS node now also exposes a digest-selected, callback-scoped
payload lease with exactly three scheduled fresh readers and eviction safety;
lookup is keyed rather than scanned, acquisition failures are redacted, and
chunk-boundary short reads preserve `Read` semantics when a later chunk fails.
An exclusive transient retirement intent prevents steady new leases from
starving eviction, and verified-read failures retain their admitted byte charge.
Completed provider-ingest and post-completion claim/request primitives are
recorded in `status.md`. The completed claim is now accepted only by the
lifecycle-leased fresh-reader verifier. The only downstream-visible minting
boundary is the doc-hidden `NodeHandle` method that checks its process-local
marker before and after the crate-private lease verifier; the raw
verifier-evidence request constructor is crate-private. The inert provider-
attestation core now covers a stable cursor-independent completed-row identity
with later-head rebasing, the approval-only replay-stable signer contract, and
a bounded CAS journal with restart-ready pages, UNIX-time and claim fencing,
dead-letter recovery, and opaque inventory acknowledgements. The outstanding
gate is production activation. An effect-free bounded scanner with private
claim minting and a dedicated request-bound signed archive reader now validates
completed-Musubi pages without mutating an external service. The reader lazily
owns one ephemeral Ed25519 session, signs a bounded canonical transcript over
the exact request and every projected field, and caches one exact generation
response so cancellation is replay-safe even as the archive head advances. The
reader receives no claim factory or opaque claim; the scanner pins the session,
verifies the signature before privately sealing the projection, and commits its
generation only after full validation. It retains separate continuation,
finalized-high-water, and completed-head state so same-head suppression remains
deterministic for one scanner lifetime; restart creates a fresh scanner and
rescans. A crate-private one-page reconciler rederives each request under the
admitted-payload lifecycle lease, idempotently enqueues it while the key remains
  retained in the bounded journal, and uses a drop guard to roll all scanner
  progress back on cancellation and on every returned failure.
Delivered entries can be capacity-pruned, so this is not durable deduplication.
A private clone-shared `Arc` marker exists only on a `NodeHandle` with storage
and an ingest outbox and identifies that process-local handle incarnation
through completed claims, candidates, and approval requests. Clones share it;
  another handle or restart gets a fresh non-equal marker. Reconciliation rejects
  a foreign scanner before a signed page request; a foreign or generic-unbound
  claim fails before the lifecycle lease or payload-reader I/O. Stable
  completion/approval digests exclude the marker, but it does not authenticate Kura, State, the
signed reader, or provider slots. The generic production builder is removed;
the scanner and reconciler stay crate-private. A non-resetting atomic take guard
is shared across `NodeHandle` clones, and the prepared archive exposes one
movable concrete signed reader with no cloneable accessor. A private daemon
composer consumes both into a doc-hidden non-generic coordinator retained on
`Iroha`; construction performs no reader call and lazy binding retries the same
reader/session. The reconciler now performs a qualified, timed exact slot-59
lookup after request verification and before enqueue: exact valid payload
suppresses, absence proceeds, and conflict or qualification failure fails. The
default-off nested configuration provides
independent 1--4,096 entry and 4--128 MiB checkpoint caps; `enabled = true`
remains an activation request. Stock launch fails earlier during pre-Tokio
broker resolution because slots 57--59 are unsupported; an injected registry
that resolves them still reaches the shared pre-supervisor start gate. The
inert public root-fenced file adapter now provides an exact
chain/genesis/provider-bound two-slot CAS with a fixed 128 MiB
checkpoint/payload ceiling on Linux/macOS with nonblocking normal locks and a
fixed five-second initialization deadline. Its sealed wrapper reuses the exact
initialization-lock identity committed in the immutable two-slot headers as a
nonblocking process/cross-process composite lease spanning external authority
and local cache reconciliation; cancellation releases it and exact retry can
complete a direct-predecessor repair. Raw checkpoint/CAS and checkpoint-head
orchestration, the abstract store, sealed wrapper, transition engine, and
journal runtime constructor are crate-private. The public file-store paths
separate explicit empty-`H0` initialization from ordinary open, which rejects a
missing external head and never promotes local bytes.

Slot 57 now defines one qualified combined durability provider with separate
small monotonic-time and checkpoint-head CAS namespaces plus immutable
content-addressed checkpoint blobs. Canonical domain-separated hashes bind the
checkpoint scope to chain, genesis, provider, and the exact journal-policy
digest. Mutation order is blob put/readback, head CAS/readback, then local
two-slot CAS. The external head/blob is authoritative; a local exact direct
predecessor can be proved from the retained predecessor record/blob and repaired
forward, while deeper rollback, ahead/fork, substitution, or missing state
fails closed. The separate sealed time floor bounds every head timestamp.
Restoring the local cache therefore cannot make an older checkpoint current.
Exact successor replay remains idempotent after cancellation or response loss.

The non-secret activation catalog reserves all-or-none runtime slots 57--59 for
the combined durability seal, approval signer, and authenticated inventory.
Registry resolution performs exact pre/post handle and qualification snapshots,
but the stock broker has no implementations and resolution invokes neither
readiness nor effects. Private daemon wrappers now pin signer calls to the
configured adapter, chain/genesis/provider, and finalized
`State::provider_owners()` value, and inventory calls/results to the configured
adapter and exact local scope. They remain uninstantiated, as do the
crate-private sealed-clock and checkpoint-head effect drivers.

Keep the pre-supervisor rejection until the retained non-generic daemon
coordinator owns those private effect drivers. It must also
provide a concrete combined time/head/blob durability adapter, signer and
inventory adapters, broker support, bounded readiness and supervision, and
crash/cancellation/revocation/corruption/concurrency/platform chaos evidence.
Deployment must enforce a singleton rooted runtime session for each exact
external provider scope across machines, or equivalent provider-side session
fencing; the implemented OS lease covers only processes sharing one state root.
The provider must not mutate the attestation registry directly.
Remaining release gates, in order, are:



<a id="record-219b43cdb0c1f272cebe08a184e43f0b12fdbf71a42c820ded1871bafed34262"></a>

- Finish focused validation of the occurrence-bound targeted resolver,
  supplied-source test runner, package-invitation rebasing, exact archive
  projection preflight, and runtime publication authorization, signer-clock, retry,
  and dead-letter hardening. Include the real-type exact-release response
  boundary under the new Musubi-only 32 MiB client ceiling, the resolver's
  deterministic 24 MiB JSON-items pagination budget/cursor continuation, the
  1,102-byte maximal SemVer and conservative 16,457-byte maintainer cursor
  boundaries. Kotlin and Java focused fixtures now cover the matching SDK
  boundaries, and Swift bounds the response during transport collection; its
  focused alias and streaming XCTest cases pass under the full Xcode toolchain.
  Regenerate the checked
  exact 31-route OpenAPI surface only from a clean dependency graph whose
  tracked root `Cargo.lock` matches its source-bound exact pin; do not bless the
  present unrelated uncommitted dependency work by changing that pin in
  isolation.


<a id="record-1a0066623d144d7ee94852f512a14034e0bada92f6b11a9c200cc9d210f9ccb0"></a>

- Qualify the remaining phase-three deployment and long-detach boundaries.
  Archive registration now retains at most eight append-only exact signed
  attempts and rotates only after finalized archive absence plus either
  authoritative `Expired` status or a consensus-committed finalized block time
  strictly beyond the exact transaction/receipt deadline; local or cache-only
  expiry, pending, absent, transport-unknown, and generic rejection do not
  rotate. Applied transaction height must be covered by the authoritative
  archive page. Storage coordination binds the transaction, chain/genesis,
  snapshot, immutable archive-registration projection, verification-lock
  digest, and at least three attestations; the coordinator returns its current
  mutable archive record separately for location CAS.
  Location Add retries use exact Core no-op replay and a complete finalized
  pre/post-query rebase. A bounded eight-generation append-only journal now
  persists each exact signed CAS before submission, retains its applied and
  terminal finalized pages, never reuses a retired stable ID, and recovers a
  replacement after authoritative rejection/rebase, expiry, applied-then-
  retired, or later retirement evidence. Replication, readback, and release
  submission retain and recheck the complete finalized directory, including a
  typed post-rejection check; stale healthy or retirement pages cannot overwrite
  a later journaled renewal. Readback now walks every finalized sorted provider
  until two valid distinct responses succeed, accepts any strictly ordered
  two-provider subset of the location, and leaves the journal and Native AMX
  untouched when the provider set is exhausted. Native AMX evidence retains its
  applied height and final verification must cover it.
  The bounded exact-release journal model now retains a reconstructable signed
  V1 envelope, separate payload and authorization-inclusive wire digests, the
  exact replication/readback floor, append-only outcomes, synchronized
  resolver/retention absence, and derived pre-send capacity reservations.
  The production backend now prepares and persists before send, queries the
  authoritative exact status first, reconstructs byte-identical Torii bytes,
  gates absent submission on the unchanged selected location, and rotates only
  after terminal finalized evidence. An identical observed release is not
  treated as proof that this transaction applied. Restart, lost-response,
  no-resign, renewal, pending-status, and Torii-body oracle regressions are in
  place. Qualify those boundaries against the real fee-quote/status/submission
  transport and crash injection next. Authoritative status currently proves
  payload-hash application while the authorization-inclusive wire digest is a
  local byte-identical replay binding; do not describe it as committed-wire
  evidence. Final verification now validates the complete paired home/universal
  response once and persists only a compact, append-only, self-digested
  checkpoint of its nonce-derived operation identity, immutable release,
  covering snapshot, and two canonical projection digests. A near-limit legal
  governance-account regression proves that mutable projection growth does not
  grow the completion frame. Later paired yank, takedown, and storage-health
  revisions no longer inflate or strand the journal. The public self-digest is
  commitment-only; qualify the trusted rollback-resistant journal boundary
  under crash/corruption injection together with the real submission transport.
  The archive-location wire bound is now corrected: the former
  64-provider, 64-approval aggregate can exceed both the 10 MiB transaction and
  16 MiB block-body corridors, so it is not an admissible consensus shape.
  Each provider's signed parsed-bundle attestation is registered as one
  immutable at-most-1-MiB record; the location Add and public page carry only
  the sorted provider list and archive/order-bound aggregate set digest, which
  Core resolves and recomputes exactly. The publisher installs a compact set
  descriptor plus one no-replace exact signed-transaction sidecar per provider,
  persists compact append-only main-journal anchors in separate advances,
  exact-queries finalized audit records, and replays byte-for-byte before
  preparing the compact Add. Missing or substituted anchored sidecars fail
  permanently, and rejected proof registration rebases only from a covering
  strictly advanced finalized location revision. Qualify the std-only path
  implementation with descriptor-relative primitives and crash injection.
  The daemon now exposes a read-only authoritative archive-registration reader
  bound to one exact `NetworkId` and `Arc<State>`. It consumes Core's
  consensus-owned snapshot-history validator, requires a cryptographically
  verified V2-finality artifact to commit to the exact result-bearing
  registered-height Kura block wire and sole successful native registration,
  and returns only a current record with an equal immutable projection. It has
  no effects and keeps stock activation fail-closed. Retain as the release
  blocker the absent deployment-owned effectful storage coordinator and
  production SoraFS pin/replication backend. Activate the implemented
  provider-attestation foundation through one
  non-generic deployment-owned coordinator: extend its implemented take-once
  `NodeHandle` plus exact prepared signed-reader tenure to the private effect
  drivers and spawn the capture/reconciliation child around the
  crate-private one-page primitive. Rediscover its paged ready work, and after
  every fresh request verification exact-read the authenticated slot-59
  inventory before enqueue; suppress an existing valid exact-payload item,
  proceed on absence, and fail on conflict. Wrap
  concrete slot-58 and slot-59 providers
  in the existing private governed signer and inventory adapters before calling
  the crate-private effect drivers. Provision the implemented root-fenced
  two-slot CAS adapter through its scope/policy-bound consuming constructor and
  bind it to a qualified combined slot-57 provider implementing the separate
  authenticated time/head namespaces and immutable checkpoint blobs. The
  inventory/coordinator must not mutate the registry directly. Keep delivery
  and the stock launch unavailable until concrete slot-57--59 broker support,
  bounded readiness, supervision, and singleton rooted-session or provider-side
  cross-machine session fencing are configured and authenticated. Qualify the
  header-bound composite lease, dead-letter repair, timeout/revocation,
  capacity, corruption, concurrent resume, cancellation after durable successor
  installation, crash-at-every-transition, offline rollback, and
  supported-platform paths.
  Same-ID renewal follows the current finalized pin, order, epochs, and exact
  provider evidence. Exercise the real fee-quote/submission transport and crash
  boundaries before send, after registry commit, after authoritative-record
  persistence, after coordinator response, and after location commit, including
  concurrent resumes and receipt expiry.



<a id="record-164bb3e0d37a19d898e96f86c3fbb2302fb657cf41613b1e1cebe619cfc0a129"></a>

- Supply and qualify the deployment-owned private HTTPS/TLS runner and its
  implementation of the completed late-bound factory, concrete qualified
  deployment-selected signing/custody implementations of the completed
  publisher-request and receipt-approval provider boundaries, and assemble the
  completed bounded durable Unix replay journal with deployment-selected
  operation, authorization, response, and snapshot limits through the
  now-supervised custom launcher path. Supply the
  admitted seed, storage-coordination, and provider-readback adapters. Deploy
  the journal and durable Unix clock only
  below a trusted non-replaceable ancestor with rollback-resistant private
  storage or an external sealed monotonic head. Qualify peak journal memory and
  latency; the current request path validates and replaces a complete snapshot,
  so larger deployments need a bounded transition WAL, small atomic head, and
  off-path checkpoint compaction unless measured selected limits prove that
  unnecessary. Add crash-at-every-write, coordinated-rollback, and
  cross-process lock evidence, and retain fail-closed behavior until equivalent
  non-Unix and descriptor-relative primitives are qualified. Bind configured
  origins, DNS answers, and token-verification keys to finalized provider
  adverts.


<a id="record-d3b76e2d2e694bbd4a74e5e4c92c4bf8dcd9648c45da141b4fdb644bb1fd64ae"></a>

- Add a safe stable non-Unix handle abstraction covering identity, single-link,
  no-follow opens, and handle-relative no-replace directory rename/replacement.
  Until then Windows and other non-Unix targets fail with
  `UnsupportedPlatform` before cache-root inspection or creation, package and
  workspace-test reads, platform-config reads, and publication journal/staged
  filesystem access.


<a id="record-d3c75f5b17474cd31dbc037227ca9ba913efd45bad71c81fa62890fb9ea4f016"></a>

- Add an atomic handle-relative compare-and-delete primitive for cache trees on
  every supported platform. Until then dry-run retention classification remains
  available, every non-empty live prune fails before candidate inspection or
  mutation, and install staging/payload residue is retained without automatic
  destructive cleanup.


<a id="record-14b1c28c2f3f3892de68e3d08b0fe714edba9ccdfa35c5e5e0defd73e7970e47"></a>

- Replace the Unix cache's advisory-lock plus ordinary directory rename with a
  safe descriptor-relative no-replace primitive. The current absence check does
  not prevent an uncooperative same-UID process from planting a destination
  immediately before rename, affecting install and repair quarantine.
  The fix requires an approved existing workspace abstraction or dependency
  change; do not weaken the no-clobber contract with another path-only check.


<a id="record-e5b115a6af5de18e9639ad33012cc198135ca42ef3630d5eef2c24a62318307c"></a>

- Replace package collection's path-based ancestor pre/post checks with retained,
  handle-relative no-follow/open-beneath traversal. The opened final-file
  identity is already pinned, but a deliberately timed ancestor ABA replacement
  remains an OS-specific packaging race gate; qualify it with rename-only,
  symlink, and reparse race fuzzing on every supported host before release.


<a id="record-327cc19295f34a498534f624ed42442d92fe9272e5535570d63f3a40d0582b47"></a>

- Replace workspace-manifest, consumer-lock, and declared-test path-based
  ancestor checks with retained, handle-relative no-follow/open-beneath
  traversal. On qualified Unix their final components are now singly linked,
  byte-bounded, and pinned through an architecture-qualified
  nonblocking/no-follow descriptor, so symlink, FIFO, device, and oversize leaf
  substitution cannot supply bytes. Other targets remain fail-closed until a
  stable handle-identity implementation exists. The Unix leaf boundary does
  not close a deliberately timed ancestor-directory ABA around the pathname
  open.


<a id="record-f5659ed8fe1d584aa75236d66d0977bca7588a66be6ea531d12ba51829a94014"></a>

- Wire the remaining metrics only at their authoritative long-lived producers:
  journal phase age, cache corruption/capacity, and selected-root storage
  pressure, plus the injected consumer-fetch integrity observer. The fetch
  adapter now supplies a typed exact-once attempt boundary but the one-shot CLI
  intentionally has no Prometheus producer. For journal phase age, first add an
  atomically persisted phase-entry timestamp supplied by a qualified
  non-regressing clock and a bounded complete active-journal snapshot API; then
  deploy one long-lived owner of both the publisher journal root and exporting
  registry. Pending retries must not reset age, completed operations must be
  excluded, restart must project before exposing any series, and partial scans,
  clock rollback, or ownership loss must never publish healthy zeros. Project
  all seven values atomically with respect to exposition and refresh them at a
  bounded cadence. Core governance rejection telemetry is already emitted once
  at each tracked mutation's authoritative
  error boundary with bounded typed action/reason labels. The persisted
  replication-shortfall aggregate and post-commit gauge synchronization are in
  place; retain alert/rule soak validation as a release gate. The six paged
  Musubi query paths now carry their exact cursor-failure enum through an internal
  Core error and Torii exports the corresponding bounded reason while retaining
  the existing public `Expired` query error.

