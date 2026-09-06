# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-18be783fe52db4f02eb18618c7b8966bd924d9bc1252410bf3c1f7b0f4a53748"></a>

<!-- Original context: Roadmap / SoraFS V1 Governance DAG deployment closure -->
## SoraFS V1 Governance DAG deployment closure

The standard outbound request path and exact inbound receiver now have sealed
cross-replica nonce fencing, and bounded signed DAG block-prefix archive/readback
is implemented; completed source details and focused results live in
`status.md`. The standalone validated config view now exposes an exact public
service-only broker catalog: slots 8 and 10 for IPNS, slots 8, 9, and 10 for
signed HTTP, and never producer signer slot 7. Canonical build, deterministic
release-bundle, and generic OCI inventories now include
`sorafs_governance_dag`; this closes source discoverability and package
inventory only. The shared fixed broker endpoint now accepts exact canonical
non-empty client subsets and confines every session to its selected bindings,
so the daemon and standalone service can share one qualified deployment
catalog without exposing producer-only roles.
The sanitized projection now has a bounded, explicitly versioned canonical
Norito export/load handoff that carries only the exact public catalog and
roundtrips all specialized configuration families byte-identically. This
removes any need to pass the daemon's complete configuration to a deployment
broker. The shared executable shell now owns the catalog-only CLI, secure
bounded file loading, redacted errors, readiness/lifecycle bridge, and
SIGINT/SIGTERM shutdown. It does not supply a concrete deployment registry,
vendor-linked binary, credentials, vendor clients, or provider implementations.
The supported V1 packaging model is a thin statically linked deployment binary;
a generic binary would require a separately approved and versioned
authenticated provider-plugin/IPC ABI.

Remaining work is deployment-owned: install the receiver in both Kubo/head
ingress administrations, link and supervise a concrete deployment registry and
broker executable with authenticated signing/custody and sealed-CAS backends,
and qualify two instances through CAS failover, signer rotation, archive
recovery, rollback, public-mirror, corruption, outage, and disaster-recovery
rehearsals.
Clean-source five-target artifacts, binary smokes, SBOM/provenance, L1
qualification, and L2 promotion evidence remain open. None of those external
proofs may be replaced by an in-tree test provider, package inventory, or local
source validation.



<a id="record-98e13eaa065dc4812321ef11d903a353e56e4572d79e2f042e4a9e8d201cb0c1"></a>

<!-- Original context: Roadmap / SoraFS V1 production closure -->
## SoraFS V1 production closure

The canonical first-release implementation, validation, documentation,
authority-removal, and rollout-evidence mapping is
[`specs/sorafs/v1_closure_ledger.md`](../../../../../specs/sorafs/v1_closure_ledger.md).
Canonical implementation-coupled SoraFS plans and fixture notes live in this
repository under `specs/` and `fixtures/`. Public and localized mirrors belong
to the optional sibling `iroha-docs` repository; they are not hashed release
inputs or static-contract scan targets here.
Repository conformance and production promotion are separate: the current
production aggregate remains `status=blocked`, `summary_file_count=0`, and
`recognized_summary_count=0`, with no trusted foundational envelope.
Repository conformance and synthetic fixture aggregates are not promotion
evidence. L1 requires the live four-voting-validator, multi-provider,
dual-gateway, dual-Governance-DAG deployment, resilience/load/24-hour soak
evidence, and exactly 17 fresh payload-free summaries under one deployment
context. L2 requires the ordered `SFM-1`, `SF-1`, `SF-2`, `SF-2c`, `SF-3`,
`SF-4`, `SF-5b`, `SF-6`, and `SF-8a` envelope signed by an independently
administered external software Ed25519 signer. Its binding must pin
`signing_provider=authenticated_external_signer`, `signing_backend=software`,
and `signer_qualification=software-key-qualified`.
Promotion is allowed only when both deterministic runs emit `status=ready`,
`summary_file_count=17`, `recognized_summary_count=17`, every lane is valid,
no critical/high vulnerability remains, and rollback is usable.
The read-only final control is
`scripts/check_sorafs_production_promotion_bundle.py`. It is the conjunctive
consumer of those two-run/22-input replay outputs, the exact six-case negative
archive, and fresh externally authenticated software-Ed25519 plus cosign/OIDC
provenance. A local archive alone remains blocked. Closure still requires an
independent administrator to produce that genuine signed provenance over the
final pinned release bytes; synthetic tests and unsigned local receipts do not
advance L2.
Gateway-compliance trust policy now rejects any catalog-approval and regional-
acknowledgement role reuse by signer identifier or Ed25519 key before provider
or checkpoint access. Deployment closure still requires the audited feed and
ACME adapters, independently administered gateways, atomic promotion/LKG
rollback, live canonical 451 probes, and resilience evidence.
The reference-validator packager has no smoke bypass: advert and closed-bundle
fixture smokes are unconditional archive members with manifest-bound digests;
each must decode as the exact expected successful `ValidationOutcomeV1`, and
the retired `--skip-smoke` spelling fails as an unknown argument. The local
gate is verified by 33 packager, six strict-capture, and 154
release-automation tests. Five-target builds, installation, rollback/yank,
SBOM/provenance, and external signing evidence remain open.
The shared runtime boundary now enforces one canonical WebAuthn RP/origin
policy and exact secret-free catalog relationships and configured bounds. The
provider-ingest launcher also completes a state-free qualification preflight
before opening any archive, consensus, node-handle, or outbox state. The exact
opaque-preflight consumption/revalidation and main daemon startup-order tests
are green; remaining L0 execution includes clean workspace gates and final
artifact provenance. Seven focused Rust `EscrowId` hard-cut
regressions and the latest complete aggregate production-readiness regression
run passed all 465 current tests; earlier complete runs covered the then-current
459-test suite. A second complete 465-test final-tree replay remains required.
Neither local result substitutes for native qualification or genuine L1/L2
evidence.
PDP, PoR, and PoTR production failure handoff uses the exact-chain durable
native transaction forwarder, and storage work is gated by the finalized
native task cursor, revision, lease owner, generation, and expiry. The
competing `sorafs_node` repair manager, checkpoint, event projection, and
process-local terminal authority have been deleted; repair GC and
reconciliation now consume one complete finalized native projection. Source
validation and four-peer exactly-once evidence remain release-blocking for the
repair lane.

Stream-token issuance is now controlled only by node TOML and binds one
runtime-injected Ed25519 signing/custody provider by its non-secret handle, exact
public key, non-zero adapter revision, and non-zero public-policy digest. The
former file-seed loader, environment enablement, standard-launcher node-key
derivation, and internal seed-signing API are deleted. Enabled startup probes the public
identity twice, and Torii revalidates it before and after every signature before
strict verification and token release. Missing, substituted, stale, drifting,
revoked, or test-marked providers fail closed. Focused Cargo/workspace
validation and reviewed deployment-selected provider evidence remain open.

The mandatory SoraFS ABI-23 Python native reference lane is now pinned to exact
Python 3.12. The obsolete tracked `_crypto.cpython-39-darwin.so` is removed, and
the runner rejects any tracked package `.so`, `.so.*`, `.dylib`, `.pyd`, or
`.dll`, activates its selected virtual environment, covers the
cancel-asset-lock, reference-validation, and provider-ingest suites, and rejects
JUnit skips. The complete pinned-Python-3.12 fixture/workflow contract is green
at 36/36. The closed
reference inventory currently binds 82 payload artifacts, 32 exact
`ValidationOutcomeV1` files, and 38 negative payload vectors; all eight
appeal-finance `CancelAssetLock` files are mandatory. The current non-native
hard-cut slice passes 85 focused Python tests, seven static guards, 36
fixture-workflow trigger tests, and 53 native-artifact contract cases; managed
Kotlin/JVM, mirrored Java Android, and C# parity is green. A settled,
source-fingerprint-bound integration snapshot now produces all five Apple
ABI-23 slices and locally qualifies the full Swift, Darwin JavaScript, Python
ABI3, and host C# suites. Those results are local host evidence only: no
checked-in or signed five-target inventory, published package set,
authenticated cross-platform replay record, or release-target attestation
exists. Rebuild and replay every SDK from one signed candidate across the full
release matrix before promotion. Kotlin/JVM and mirrored
Java Android now require both exact bridge ABI 23 and `NativeSignerBridge` JNI
contract revision 5 before
making any native signer call; the Android artifact gate requires both
revision-probe exports, preventing a stale same-ABI JNI descriptor from passing
package qualification. Swift package admission now requires an embedded
`NoritoBridge.artifacts.json` declaring exact ABI 23, and its runtime loader
rejects missing or stale manifest ABI metadata before accepting a matching
artifact hash. Ignored or locally rebuilt artifacts can qualify the exact host
source they bind, but do not constitute a checked-in, published, or
cross-platform release inventory. The separate
SoraFS pin-register SDK workflow, runner, and guard are also exact Python 3.12
and install only the hash-locked, binary-only `requirements-ci.lock`; a fresh
isolated CPython 3.12.13 venv is green at 3/3, including positive static
coverage and version/resolver/major/workflow/lock negative controls.
Kotlin/JVM contract-manifest and SCCP protocol-V4 source drift is closed at
26/26 focused tests, the mirrored Java SCCP harness passes, and the complete
51-test Android module now exercises the mandatory fresh transaction
compatibility probe. The remaining Kotlin/JVM and Java core failures are
explicit stale-ABI-22 native failures; rebuild and rerun them before `G-FINAL`.
Seven focused Rust `EscrowId` hard-cut regressions are green across lowercase
checksum parsing, canonical JSON/Norito/schema handling, noncanonical query
rejection, and typed public-selector roundtrip. The source-fingerprint-bound ABI-23 native rebuild
is locally qualified for Apple, JavaScript, Python, and host C#; Kotlin/JVM and
Java/Android native replay plus the signed full release matrix remain required
before the wire/SDK cut can be closed.

The SoraFS monitoring source slice is green under checksum-verified
`promtool` 3.13.1: all 30 alert files pass rule validation, all 26 alert unit
suites pass individually, all 50 dashboard JSON files parse, all 56 alert/test
YAML files parse, and the wrapper regressions pass `3/3`. `G-FINAL` still
requires the deployment-owned scrape, alert-delivery, and soak evidence; no
historical source-suite mismatch remains open.

Provider ingest is now an opt-in supervised `irohad` worker over one immutable,
bounded finalized replication-order snapshot. Its durable single-writer
outbox, monotonic finalized high-water, bounded claims, retention-only terminal
pruning, retry/dead-letter path, exact fee-quoted completion transactions,
committed reconciliation, and payload-free liveness/readiness are wired from
`iroha_config`. Enabled startup requires separately identified
authenticated-source and governed completion-signer providers. The new
authenticated source-pool coordinator pins at least two distinct non-local
provider identities and independently identified child transports, rejects
noncanonical or incomplete finalized source lists before I/O, rechecks each
source around fetch, and fails over only in canonical provider order. Standard
daemon startup freezes the exact bounded provider inventory across readiness
probing and requires the same slice on every supervised tick. The worker also
rechecks the current committed provider owner before signer resolution and
immediately before and after signing; a newer finalized assignment snapshot
invalidates retained `Signing`, `Signed`, `Ambiguous`, and `Submitted` material
when that owner changes or is removed. The exact governed signer-policy
identity, monotonic revision, and digest are persisted with prepared material;
a newer finalized policy rotation or revocation invalidates it before
observation or resubmission, including after restart. The durable policy floor
rejects revision rollback, same-revision digest equivocation, identity
substitution, and reuse after revocation unless a strict canonical successor is
observed.
Before any supervisor, archive, Sumeragi, node-handle, or completion-outbox
state is created, startup now performs a state-free exact qualification of the
source pool, signer resolver, signer, and sealed checkpoint provider, retaining
the exact provider in an opaque token for revalidation during assembly. Four
focused preflight tests and the exact opaque-token consumption/revalidation
and main daemon startup-order regressions pass.
Remaining gaps are concrete governance-advert/stream-grant/pinned-HTTPS child
transports, deployment-owned source qualification pins, a qualified
deployment-selected governance-aware completion signer that enforces the
now-configured public handle/revision/policy/algorithm/key binding atomically through
rotation/revocation, and a sealed-CAS retention coordinator for the implemented
daemon-owned Kura-authenticated provider-indexed archive and its explicit
content-addressed compaction fence.
The remaining focused tests, full workspace validation, and reviewed four-peer
restart/duplicate-submission evidence remain open under
`V1-BLOCK-PROVIDER-INGEST-RUNTIME-01`.

PoTR finalization no longer authorizes a provider from Torii's immutable
startup admission registry. The injected runtime boundary requires a separate
live finalized-policy reader, resolves and rechecks the exact admission around
the two independently administered signatures, and atomically persists the
accepted policy identity, digest, sequence, finalized cursor, provider, and
envelope digest as a restart-safe monotonic floor. Torii now constructs
`PotrFinalizedAdmissionReaderV1` from
`PotrStateFinalizedPolicySourceV1` and the council-verified admission registry
only after enabled `[sorafs.por.potr_runtime]` public pins exactly match the
injected signer roles. The strict optional binding contains both signer
handles/identities/qualifications, the gateway key, distinct
reader/source/resolver identities, and the complete baseline finalized
admission anchor; provider qualification is fixed to that anchor's
sequence/digest. Partial or disabled-stale configuration, test-marked/shared
handles, identity reuse, missing injection, unconfigured injection, and any
substitution fail closed. This configuration/startup boundary is
source-complete; focused/workspace Rust validation, deployment-selected signing
provider qualification, and four-peer
rotation/revocation/replay/crash evidence remain open under
`V1-BLOCK-POTR-DUAL-SIGNER-01`.

The reputation lane now has a native committed input journal with governed
predecessor-bound recorder policy, one globally contiguous sequence, typed
payload-free committed events, and a fixed-view finalized query. PoR terminals
and stream-token outcomes have exact append instructions; canonical capacity
dispute registration and resolution update the dispute record and journal
atomically. Typed proof-outcome, repair, orderbook, and reserve finalized feeds
already exist. The current-head state adapter was removed because it could not
provide exact-anchor pages. The standard daemon now owns the configured bounded
historical archive and constructs `ReputationFinalizedQueryV1` from that
archive rather than accepting an injected query. Before Sumeragi starts it
reconciles the committed State tip against Kura's authenticated V2 finality
receipt with a zero-gap barrier, retains an explicit activation floor when
first enabled on a nonempty chain, and then applies the configured live-lag
qualification. The same archive is installed at the V2 apply boundary so a
fresh projection is durably captured after Kura finality and the WSV checkpoint
but before live State publication; capture failure is restart-required. Daemon
bootstrap also validates the immutable response against the complete exact
request—chain and height, finalized anchor/time, authority activation,
continuation, row bound, and exact cursor—before opening runtime state.
An externally authenticated journal-transaction submitter handles the exact
PoR and counted stream-token append transactions for supervised committed-event
reconciliation; standard `irohad` no longer constructs a validator-key or
queue-backed fallback. The exported deterministic `sorafs_node` multi-feed projector
persists five finalized-feed cursors, provider accumulators, a bounded
retry/dead-letter signing-material outbox, and acknowledgements that verify the
full threshold-signed snapshot against the anchored trust policy and
authoritative finalized time. Strict standard-daemon policy construction,
trust-policy reuse, supervised fixed-view scheduling, shutdown, status, and
bounded metrics are wired through the daemon-owned finalized archive plus
runtime-only threshold-signer and Governance DAG clients. Enabled startup
rejects an unavailable or inconsistent archive and missing, null/test-marked,
or substituted external clients. The threshold-signer handle is pinned
to the full canonical trust policy and revalidated before and after signing;
the returned envelope is verified against its policy ID/version, quorum,
ordered Ed25519 key set, and revocations. Governance DAG provider qualification
now binds both publisher peer identity and Ed25519 key, rejecting same-key
cross-peer substitution before durable state opens. The authenticated journal
submitter must be supplied through runtime injection. Governance publication reconciliation
now accepts only the canonical versioned signed-head receipt with a bounded
inclusion suffix whose hard limit aliases the manifest checkpoint window. It
verifies the pinned
head publisher/key before traversing every block, requires the exact signed
snapshot once, links each successor suffix to the previously authenticated head
without rollback or fork, and persists/reverifies the head and path before
restart reads. Focused locked `sorafs_node` and `irohad` validation is green.
All seven authenticated committed GET routes, strict JavaScript/TypeScript,
Python, Kotlin/JVM, Java Android, Swift, and C# clients, and the
canonical-account-signed Rust CLI/rollout collector are implemented locally.
Deployment qualification of the daemon-owned immutable historical query,
genuine external threshold-signer and authenticated Governance DAG adapters,
PoR/token callback-owner wiring, complete SDK/native and workspace validation,
reviewed four-peer evidence, and promotion remain open. Latest, provider,
weights, and event reads are gated on the fresh committed projection;
snapshot-id reads resolve the exact authenticated snapshot from a durable
immutable suffix capped at 1,024 entries and the publication-checkpoint byte
ceiling. Unknown or evicted ids return `404` without substituting latest.

All four ledger-authority domains enumerated by V1-C04 now use native committed
state as their sole authority; process-local state is only a rebuildable,
finalized-chain projection.
The process-local reserve runtime, checkpoint, scheduler, mutation API, and
obsolete routes are deleted; reserve mutations now forward exact caller-signed
native transactions and reserve reads use authenticated finalized projections.
Its metrics now rebuild from the typed finalized journal and committed provider
accounts with bounded labels and an explicit reconciled-height readiness bit.
The competing `sorafs_node` orderbook, checkpoint, config, mutation/event API,
and pre-release snapshot wire are also deleted. The reserve/rent and orderbook
lanes remain open for full source validation and reviewed distributed recovery
evidence. The standard daemon now has an opt-in, credential-free finalized
reserve transparency scanner over the daemon-owned immutable archive. It
requires the exact reputation query handle, verifies its restart cursor and
every returned event against fresh committed projections, records through the
durable idempotent source index before advancing its canonical checkpoint, and
uses bounded pagination and exponential retry for normal archive lag. This
closes the local reserve producer gap only; connecting every other finalized
producer and collecting distributed transparency evidence remain open.
Moderation GETs now consume only a fresh supervised finalized
projection, and the signed receipt checkpoint/projection is the sole viewer
audit authority; the retired local audit POSTs are unmounted and unadvertised,
so requests return `404 Not Found` and no scheduler exists. The retained checkpoint now exposes an
Ed25519-signed canonical digest/receipt-count/chain-head anchor; audit pages
require that exact digest and an explicit digest-bound limit, reject alternate
query encodings, and return `409` on checkpoint change. The supervised
moderation worker now durably claims payload-free panel notifications, calls an
independently config-qualified idempotent boundary, and checkpoints exact
receipts or bounded dead letters; standard `irohad` and Torii expose the
all-or-nothing injection path. Moderation remains open for deployment-owned
messaging, settlement, downstream publication, authenticated
access/signing/custody and transparency providers, genuine immutable
archive/checkpoint attestation,
cross-replica operation/checkpoint fencing, and exercise of the shipped signed
terminal archive through rotation, restart, publication readback, incremental
and full-history audit, and failover. A transparency-published monotonic head
that gives first-contact clients freshness and four-peer evidence also remain
open.
Its ballot lifecycle is already chain-authoritative and rebuildable from
finalized events. Taira and Minamoto mutation remains separately authorized
cutover work.



<a id="record-5d04f7cff3584cd07538dd6990311e046abc0bf51f1ced9488d78477841f71f4"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- SoraFS/SoraNet first-release KDF identifier cleanup is complete: SoraFS
  envelopes remain V1/version 1 with the transcript-bound hybrid suite label,
  SoraNet advertises only NK2/NK3 suite IDs `0x04`/`0x05`, and old pre-release
  suite labels/IDs are intentionally rejected. Keep future fixture and SDK work
  aligned with the regenerated `snnet-interop-nk{2,3}-v1.json` contents rather
  than adding compatibility aliases.


<a id="record-b5b2265c54ba5673751bf547667df7b6e8d072469a12b25b28d035e645b83cea"></a>

- The SoraFS gateway DNS owner runbook family now uses governed cutover
  runtime tokens and reviewed sample ticket IDs instead of fixed March 2025
  command examples; the rollout static contract scans the canonical `specs/`
  runbook for stale `OPS-XXXX`/`SNS-DF-XXXX` tickets, dated 2025 cutover
  examples, date-coded DNS tags, and reopened kickoff wording.


<a id="record-3e0ca13134d9670b1c57c9982eaf8653fbbd88c2f698d57091d1cb2f48ecf93a"></a>

- SoraFS gateway direct-mode enable now keeps `require_manifest_envelope`,
  `enforce_admission`, and `enforce_capabilities` enabled in the emitted Torii
  snippet, validates canonical provider/manifest digests, recomputes expected
  hostnames and HTTPS direct-CAR URLs before printing config, rejects plans that
  omit manifest-envelope or direct-CAR capability evidence, and escapes TOML
  strings so tampered plan JSON cannot inject additional settings.


<a id="record-c9624a23386d82cad6c8034da5542e225b74afe9452a06fb74e66b199b275483"></a>

- SoraFS SF1 chunker fixture parity now includes the documented Node helper:
  `scripts/check_sf1_vectors.mjs` compares generated TypeScript, Rust, and Go
  bindings against `sf1_profile_v1.json`, checks manifest metadata/file sizes,
  and verifies the recorded Ed25519 manifest signatures. The helper runs from
  `ci/check_sorafs_fixtures.sh` when Node is available, and the fixture gate's
  canonical-alias JSON probe now reads through no-follow descriptors, closing
  the SF1 determinism report's prior Node-helper gap without reopening fixture
  symlink-following in CI.

