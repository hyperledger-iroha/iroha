# SoraFS first-release publication qualification

This source audit and focused validation track the SORA CARS release. Iroha 3 is unreleased;
obsolete formats and publication aliases are rejected, not supported through fallback paths.

## Corrected publisher behavior

`crates/sorafs_orchestrator/src/bin/sorafs_cli.rs` now makes `storage prepare` reproduce the
manifest's exact content length, registered chunk profile, ordered chunk-plan commitment, PoR
root, file paths/root CID, CAR size, and CAR archive digest before writing prepared output.
Previously, its fixture paired an unrelated manifest with a new directory and still passed.
Manifest preparation and submission decode bounded canonical Norito; file payload reads are
bounded and reject symlink substitution. Directory preparation uses the native secure traversal
and eager-payload limits in `sorafs_car`.

Publisher HTTP has a five-second connection deadline and 30-second request deadline. Registration
and discovery responses are capped at one MiB; asset readback streams through a 64 KiB buffer and
rejects data beyond its exact expected length. Expected hashes come from the retained, verified payload, so a file changed after preparation
cannot substitute the expected publication bytes. Every directory asset is checked; the previous
32-file sample could overlook an unavailable or corrupted later asset.

`deploy` currently registers a signed manifest and checks existing gateway bytes. It has no
authenticated publisher-source transfer and no pinned finalized registration/provider-completion
verifier. It therefore records `publication_verified: false`, labels endpoint fee data
`reported_pin_fee`, leaves assignment/completion unknown, and fails qualification even when an
HTTP endpoint returns success and matching bytes. This must be replaced by genuine evidence,
not a flag or a permissive compatibility route.

`sorafs_car/src/gateway.rs` now accepts the native 36-byte CIDv1 root (72 lowercase hex
characters), validates its dag-cbor/BLAKE3-256 structure, and binds the signed stream token to
that exact CID. The old 32-byte assumption rejected every genuine native root. Tokens must
carry canonical CIDs even when the caller omits an optional expected CID. Tests exercise a
native `CarWriter` root and the committed native manifest fixture; raw hashes and other CID
formats are rejected.

## Actual native storage test route

The `publication_roundtrip` module is part of the existing `sorafs_node` `pin_workflows` harness.
It constructs a real multi-file CAR, verifies it, passes the retained authenticated payload reader
to the production persistent `NodeHandle`, and compares every file/chunk plus cross-chunk ranges
and native PoR samples before and after closing/reopening the provider. Negative cases cover
changed/truncated/trailing CAR bytes, changed manifest commitments, changed/truncated source
bytes, duplicate admission, and persisted chunk corruption. Failed ingestion must leave no
manifest and must release reservations so a valid retry succeeds.

Run from the Iroha checkout, sharing the active worktree's warm Cargo lane:

```sh
cargo iroha-fast -- test -p sorafs_node --test pin_workflows publication_roundtrip:: -- --nocapture
cargo iroha-fast -- test -p sorafs_orchestrator --features cli-orchestrator --bin sorafs_cli deployment_integrity_tests:: -- --nocapture
cargo iroha-fast -- test -p sorafs_orchestrator --features cli-orchestrator --test sorafs_cli storage_prepare -- --nocapture
cargo iroha-fast -- test -p sorafs_orchestrator --features cli-orchestrator --test sorafs_cli deploy_ -- --nocapture
cargo iroha-fast -- test -p sorafs_car --features manifest --lib gateway::tests:: -- --nocapture
```

Current Cargo evidence on September 6, 2026: the three native provider tests passed, with the
release-artifact test correctly ignored (3 passed, 1 ignored, 2.16 seconds). The executed binary
was `target/debug/deps/pin_workflows-11af0074c335c138`; the log is retained at
`/tmp/sora-cars-sorafs-provider-roundtrip.log`. The current Cargo gateway suite subsequently passed all 34 tests in 0.04 seconds, with
`/tmp/sora-cars-sorafs-gateway-retry.log` retained. Publisher CLI and full daemon leaf gates
remain separately queued; earlier attempts encountered independent Core compilation errors.

The ignored release-artifact test requires an immutable matching website directory and native
package containing `sora-cars.manifest.to` and `sora-cars.car`. It fails if either input is missing
or if the current directory differs from the CAR; it never silently substitutes a tiny fixture.

```sh
SORAFS_QUALIFICATION_DIST=/absolute/matching/dist \
SORAFS_QUALIFICATION_PACKAGE=/absolute/native/package \
cargo iroha-fast -- test -p sorafs_node --test pin_workflows \
  publication_roundtrip::sora_cars_release_car_roundtrips_real_provider_storage -- --exact --ignored --nocapture
```

These tests qualify native storage mechanics. They do not authenticate ledger finality, exercise
the supervised remote source fetch, or establish provider availability over the network.

## Remaining integrated qualification

- Consensus `RegisterPinManifest::execute` in `iroha_core/src/smartcontracts/isi/sorafs.rs`
  creates an automatic replication order once policy permits approval. Registration is therefore
  more than a local manifest cache, but HTTP acceptance is still insufficient finality evidence.
- The supervised provider-ingest runtime requires exact injected authenticated source, governed
  completion-signer, and sealed checkpoint providers (`irohad/src/main.rs` and
  `sorafs_provider_ingest_runtime.rs`). The authenticated Unix broker client/server already
  supplies bounded transport, authenticated source EOF, signer resolution and checkpoint calls.
  The concrete HTTPS leaf is now implemented with a mandatory injected governed grant resolver.
  Catalog-bound backend assembly is implemented; deployment-owned authenticated grant resolution
  remains a prerequisite. No retired `/v1/sorafs/storage/pin` route may be restored.
- Existing `sorafs_node::provider_ingest_runtime` tests mostly use fixture ledger/source/storage
  adapters. `irohad`'s `post_admission_quarantine_survives_restart_with_shared_chunks` does combine
  the production local storage adapter with a durable sealed outbox and explicit fixture ledger;
  run it as additional restart coverage, without calling it a consensus test.
- Torii native tests `site_binding_serves_manifest_and_spa_fallback`,
  `public_site_and_every_cid_read_share_provider_admission_policy`,
  `cid_path_gateway_redirects_active_non_browser_content_to_isolated_origin`, and
  `car_range_rejects_corrupted_payload` cover real storage/route logic but not a live provider
  network. They live under `sorafs::api::tests` in the `iroha_torii` library with `app_api` enabled.
- Final acceptance needs four actual validators, finalized manifest/order/completion readback,
  seeded authenticated source providers, real source-fetch/ingest and restart, CID-host website
  retrieval with every asset hash checked, and negative provider/manifest/token substitution.
  Retain the exact CID, network trust context, finalized transaction/block evidence, provider
  identities, and independent retrieval report. The public Taira prerequisites remain separate.

## Narrow production implementation route

This is an implementation map, not a claim that the following adapters are deployed.

| Work | Source integration point | Exact requirement |
| --- | --- | --- |
| Provider HTTPS source leaf | Implemented in `irohad/src/sorafs_provider_ingest_runtime/https_source.rs`, implements `sorafs_node::ProviderIngestAuthenticatedProviderSourceV1<Fetched = VerifiedProviderIngestPayloadV1>` | Derive only current admitted signed provider adverts from a finalized trust context; pin provider ID, network, endpoint trust policy and bounded stream-grant identity. |
| Source pool and broker injection | `https_source_pool::compose_provider_ingest_https_pool_v1`, `RuntimeProviderBrokerBackendsV1::with_provider_ingest_https_sources`, existing broker launcher | Compose the native source pool from the exact public catalog, explicitly injected governed resolvers and one shared retained-payload/DNS admission budget. Preserve existing pre/post qualification and authenticated streamed EOF checks. |
| Gateway transport | `sorafs_car/src/gateway.rs` | Reuse public-address DNS resolution pinned into the HTTP client, HTTPS-only/no-proxy, no redirects/decompression, deadlines, bounded metadata and canonical token validation. Add bounded full CAR streaming or bounded complete plan/chunk acquisition without an eager whole-CAR buffer. |
| Public policy configuration | `iroha_config/src/parameters/{user,actual,defaults}.rs`, runtime broker catalog | Expose public endpoint/trust, deadlines, maximum metadata/CAR/chunk counts and policy digests through typed configuration. Keep grants, credentials and signer material runtime-only in the deployment launcher. No environment or local-key fallback. |
| Initial publisher seed | Native publisher command plus provider source staging admission | Transfer the public CAR and canonical manifest to at least two governed source providers that can serve the exact assignment. Verify CAR/root/plan/PoR before exposing source bytes. Stage under content identity; staging does not imply finalized storage or authorize a completion signature. |
| Publication receipt | `sorafs_cli.rs` `deploy` | Await authenticated finalized registration, assignment and completion evidence; check exact manifest/CID/provider identities and retention, then verify every asset at its isolated CID origin. Only this complete path can set `publication_verified: true`. |

The leaf must first check the opaque `FinalizedProviderIngestAuthorizationV1` supplied by the
runtime. The existing public manifest endpoint is `/v1/sorafs/storage/manifest/{manifest_id}`.
The public plan endpoint `/v1/sorafs/storage/plan/{manifest_id}` is paginated: consume both file
and chunk inventories completely, requiring unchanged totals, digest, profile, exact offsets,
progress and bounded page count. Do not mistake a truncated first page for a complete plan.
Preserve file paths and sizes; flattening the directory into one file changes its root CID.
Bind the complete plan, content length, canonical root CID, chunk commitments, PoR root, CAR
size and CAR digest to the finalized manifest, and repeat exact reader validation at EOF.

The existing `/v1/sorafs/storage/token` is operator-authenticated; CAR/chunk routes require the
protocol handshake. Manifest/plan routes are public reads. A source grant must therefore come
from the correctly governed issuer through an authenticated provider capability. Do not pass
publisher wallet/spending keys into a worker, and do not weaken token issuance to make a test
work. Missing/expired/substituted grants must be fixed source errors; retries try only the
bounded canonical source list. Runtime outbox data must never contain endpoint credentials,
stream tokens or payload bytes.

The initial-seed requirement is substantive: current finalized work derives source provider IDs
from the other order assignments. A new manifest on an entirely empty provider set otherwise
has no source to read. Seed acquisition must be a distinct authenticated staging operation,
kept outside the durable admitted manifest store until normal finalized ingest succeeds. The
existing offline `sorafs-node ingest`/preseed tooling can exercise native verification mechanics,
but it is not itself an authenticated production publisher transfer. Do not resurrect the
retired storage-pin HTTP route or use a generic transaction proxy.

## External signer/checkpoint dependencies

The HTTPS leaf alone does not make publication operational. The deployment-owned launcher
must also inject:

- `ProviderIngestGovernedSignerResolverRuntimeV1` resolving a
  `ProviderIngestCompletionSignerV1` against the finalized provider owner, assignment revision,
  signer policy and binding qualification. The native wrapper signs exactly one
  `CompleteReplicationOrder` instruction with the expected finalized anchor. Provider-owner
  rotation or policy changes must fail stale requests; publisher and racer keys are unrelated.
- `ProviderIngestCheckpointRuntimeV1` supplying a sealed, monotonic compare-and-swap checkpoint,
  with exact predecessor revision, externally authenticated qualification, durable restart and
  rollback detection. A local mutable JSON file is not an implementation of that guarantee.
- `ProviderIngestFinalizedArchiveRetentionAuthorityV1` where finalized archive retention is
  enabled, plus the existing native finalized-ledger query/archive integration.
- The stream-token signer and gateway admission/quota backend at the source providers, including
  their sealed sequence/callback state. Provider signing, quota and checkpoint backends remain
  separate from the custody-free source byte reader.

The concrete daemon broker clients for signer resolution/completion and sealed checkpoint are
in `runtime_provider_broker/platform_provider_clients_03.rs`; source client/reader are in
`platform_provider_clients_01.rs`, and source-serving frames are in
`platform_server_transport.rs`. `api.rs` intentionally requires deployment-owned backends;
its typed HTTPS composition method installs no credential loader or authority default. The concrete
`https_source::ProviderIngestHttpsSourceV1` leaf now implements
`ProviderIngestAuthenticatedProviderSourceV1` through that pool. Its new mandatory
`ProviderIngestGovernedHttpsGrantResolverV1` remains a deployment-owned dependency; no default
resolver or fabricated authenticated readiness is installed. KAGEMUSHA platform owners can
supply compatible sealed/checkpoint and signer services,
but the SoraFS contracts above must be implemented and qualified explicitly; shared broker
transport or a passing KAGEMUSHA proof test does not establish them.

Qualify in this order: actual HTTPS leaf against admitted native providers with corruption,
truncation, wrong provider/token/CID, stale advert, timeout and restart cases; actual source pool
through the existing Unix broker and retained-memory quotas; then four validators plus provider
processes producing finalized registration/order/completion and identical restart outcomes.
Finally run the immutable complete SORA CARS package through the same source/ingest path and
independent CID-origin retrieval. Preserve the expected network genesis/trust context and
finalized inclusion evidence together with exact package hashes. Until then, the publisher's
explicit unverified result is the current implementation truth.

## Concrete HTTPS leaf implementation status

`irohad/src/sorafs_provider_ingest_runtime/https_source.rs` now implements the existing source
trait. Its public typed config requires the network, source binding, explicit resource/deadline
limits and one to four retained admissions. The mandatory governed resolver must authenticate
current finalized source membership, exact assignment revision, signed advert, grant, signing-key
pin, source endpoint and TLS roots. The leaf checks the returned complete request/manifest,
rejects self-source and cross-network/Musubi substitutions, and rechecks source qualification,
lease expiry and revocation before/after acquisition and at every payload read including EOF.
An admission remains held until the reader is dropped; failed readers cannot later resume.

`GatewayFetchContext::new_with_pinned_tls_roots` replaces platform roots with the exact injected
bounded DER root set. Existing public-address DNS pinning, HTTPS-only transport, no proxy,
no redirect and no implicit decompression apply. DNS resolution runs on a bounded admitted
blocking worker; a timed-out caller cannot accumulate unbounded detached resolver work.
No loopback allowance exists in the production constructor.

`sorafs_car/src/gateway/source.rs` obtains the complete existing paginated Torii plan without
inventing a parallel transfer protocol. Every page repeats identical manifest ID, totals,
profile and payload digest, exact offsets/counts and truncation flags. The complete native plan
then validates file paths and chunk coverage. Chunk responses are individually bounded and
hashed; exact payload, native PoR, root CID, CAR size and complete canonical CAR digest are
reproduced before a payload reader is exposed. It currently retains at most 64 MiB of public
payload rather than exposing a partially verified transport reader. There is no second full CAR
buffer. The existing broker reader still authenticates its exact streamed EOF independently.

Five new CAR tests cover explicit TLS-root bounds and use real loopback HTTP and native multi-file plans to cover complete
pagination, redirect rejection, truncated HTTP bodies, changed chunk bytes, later-page
substitution and malformed resource bounds. Loopback is injected only through the existing
crate-private test engine; it does not qualify TLS, governance, public routing or deployment.
Five daemon leaf tests cover exact request/lease bindings, explicit resolver refusal, retained
admission bounds, sticky reader failure, revocation at EOF and expiry. The current Cargo gateway
suite passed all 34 tests. A historical exact-source harness passed all 38 gateway/leaf tests in
0.04 seconds using the retained coherent native Node dependency closure
`sorafs_node-82f9b21f41cf1f7f`. The production payload type/implementation is copied byte-for-byte;
all gateway and leaf files are exact snapshots of the sources qualified at that time. The later
shared-admission constructor and test changed the leaf; the original evidence does not cover
that delta or the new catalog composition module.
This harness qualifies those source behaviors with native types, and explicitly excludes full
daemon wiring, current Cargo composition, real TLS endpoints and authenticated governance.

The harness evidence is `sora-cars/output/qualification/sorafs-source/report.json`, with exact
source/dependency manifests, compiler command, test log and copied native executable. Its binary
SHA-256 is `689d7d274831356e0565ab7bfb8acdf64180bb6419a5d59ef7ed8c19ceb37c4b`.
The full daemon command remains a separate gate:

```sh
cargo iroha-fast -- test -p irohad --lib sorafs_provider_ingest_runtime::https_source::tests:: -- --nocapture
```

After the shared-admission change, a separate immutable exact-source harness passed all 39 tests
(34 gateway, five leaf) in 0.04 seconds. Its evidence is
`sora-cars/output/qualification/sorafs-source-shared-admission/report.json`; the copied binary's
SHA-256 is `547084195d95bffb4ed7cb5225474125ab276940791b5b82ce9eedac9a0348fd`.
It reuses the retained coherent native dependency closure and verifies exact current leaf/gateway
source and native payload-wrapper bytes. This pass additionally covers a shared admission held
by a pending real leaf acquisition, denial before a second grant can be requested, and release
on cancellation. It explicitly excludes the six new catalog/pool/backend/launcher composition
tests below, full daemon Cargo composition, real governed grants/TLS endpoints and publication.
The original 38-test evidence remains historical and unchanged.

The subsequent evidence-validation adapter changed only the visibility/documentation of the
leaf's shared `validate_grant` helper. The 39-test snapshot is also historical by exact source
hash; it does not include that visibility change or the new evidence module.

The initial positive HTTP test correctly rejected its invalid zero-retention fixture. The
fixture now uses nonzero retention, and every negative mutation is checked against that valid
baseline. An existing future-token timing test now leaves a one-minute margin outside the
allowed skew to avoid a wall-clock second-boundary flake; production token expiry rules did not
change.

## Catalog-bound daemon composition

`RuntimeProviderBrokerDeploymentV1::try_new` in `runtime_provider_broker/launcher.rs` passes the
sanitized `IrohaRuntimeProviderBindingsV1` to the explicitly supplied deployment registry's
`resolve` method. That registry can now call
`RuntimeProviderBrokerBackendsV1::with_provider_ingest_https_sources(&catalog, sources)` in
`runtime_provider_broker/api.rs`. Each typed source registration carries public policy plus an
opaque `Arc<dyn ProviderIngestGovernedHttpsGrantResolverV1>`. The helper composes the actual
`ProviderIngestAuthenticatedSourcePoolV1<VerifiedProviderIngestPayloadV1>` and installs it through
the existing authenticated-source setter. An already installed source cannot be silently replaced.
No new broker protocol, endpoint override, credential discovery or default authority is added.

`sorafs_provider_ingest_runtime/https_source_pool.rs` derives pool identity, revision and policy
digest directly from the catalog. Before contacting any resolver it checks exact network,
source count, distinct provider identities and handles, handle separation from the pool, native
metadata/payload bounds, and deadlines/concurrency against the catalog. All leaves use one
shared semaphore, retained through blocking DNS work and until the verified reader is dropped.
The source count cannot multiply the retained-payload allowance. Public leaf policies all carry
the same pool admission count. Catalog `max_content_bytes` currently comes from total local
storage capacity; the leaf's maximum retained payload may be smaller (at most 64 MiB), and a
larger requested object fails closed. It does not claim transport support for the whole capacity.

Construction checks public qualifications but never calls readiness or issues a grant. The
existing broker server independently checks live source qualification and authenticated readiness
before announcing readiness. The existing broker client then injects the authenticated source
into `IrohaRuntimeDeps::with_sorafs_provider_ingest_authenticated_source`; daemon startup and the
supervised ingest worker continue through their existing exact qualification gates.

The new composition tests use actual catalog, native pool, backend and deployment constructors.
They cover catalog/network/limit/identity rejection before resolver access, current qualification
mismatch, canonical provider inventory, explicit readiness refusal, and duplicate installation.
They deliberately use refusing fixture authorities and do not start a daemon or bind the broker
socket. Run this filter to include both the leaf and composition tests:

```sh
cargo iroha-fast -- test -p irohad --lib sorafs_provider_ingest_runtime::https_source -- --nocapture
```

The full current daemon Cargo gate now passes, including all six composition tests. This
supersedes the narrower 39-test exact-source evidence for daemon compilation and constructor
composition; it does not install an authenticated grant resolver or qualify remote publication.

The unresolved authority is specific. Native `State` exposes finalized provider owners, manifests,
orders and completion-authority records; the existing finalized ingest archive captures order
assignments. Torii also has real `AdmissionRegistry` council-envelope verification and
`ProviderAdvertCache` signed-advert/admission/replay checks in `sorafs/{admission,discovery}.rs`.
These are useful inputs to a resolver, but neither a local owner lookup nor a cached admitted
advert is the complete fresh, exact-network assignment/advert/key/grant lease required by the
new source contract. No current backend registry implements that contract. In particular the
existing `/v1/sorafs/storage/token` issuer requires an exact-network authenticated operator
signature, qualified stream-token signer and admission service; it cannot be replaced by a
locally fabricated token. The deployment must supply the authenticated resolver that composes
those authorities, governed TLS/key pins, revocation freshness, and custody-free source grants.
Publisher staging, governed completion signing, sealed checkpoints and finalized publication
evidence remain separately required. This code does not enable publication by configuration alone.

## Reusable governed evidence validation and remaining read capability

`sorafs_provider_ingest_runtime/https_source_evidence.rs` now composes existing native
`ProviderIngestSourceRequestV1`, actual Torii `AdmissionRegistry` and `ProviderAdvertCache`, the
exact assignment revision and an already-issued grant. Independent typed transport pins bind
network/provider, current admission and complete advert digests, exact advertised HTTPS origin,
stream-token issuer key and DER roots. It checks source-list membership, request/order/manifest
identity, grant ID, snapshot/grant deadlines, council verification, current admission presence,
cached signed-advert freshness and capability/endpoint membership. Revocation in the current
registry overrides a still-cached historical advert. Debug output excludes authority snapshots,
endpoints and grant material. This is validation of supplied evidence, not an authority or resolver.
The leaf retains canonical token/signature/budget and actual TLS/public-DNS enforcement.

The remaining read-capability gap is in the actual APIs: `ProviderIngestFinalizedLedgerV1` requires
`ProviderIngestFinalizedClaimFactoryV1` for assignment reads; that factory's constructor is private
to the native ingest worker. A source resolver cannot mint it or use the public trait to obtain
a separately authenticated current assignment. Ordinary finalized authorization values explicitly
carry no credentials or authentication capability. A qualified read-only source-assignment service
must provide the current request/revision and revocation/freshness context without exposing a
completion-signing capability. Making the claim-factory constructor public would remove this
boundary rather than implement the missing authority.

Admission data does not fill the other missing bindings: provider advert keys sign adverts, while
the stream-token issuer has a separate key; endpoint TLS metadata describes leaf fingerprints,
not a DER root trust policy. Those roles must be bound by a governed transport authority. Existing
operator-authenticated token issuance must supply an already-issued grant. This adapter never
requests or issues one, and cannot turn operator configuration into authenticated readiness.

Six evidence tests use native admission/advert/revocation fixtures and actual Torii
validation/cache operations. They cover valid binding, wrong network/order/source/revision/root,
revocation with a retained cache entry, a newer signed advert, expired/future snapshots, and
substituted endpoint/key/roots. The transport fixture explicitly replaces the shared admission
fixture's tagged `torii:cluster.primary.svc.local` label with a canonical authority, recomputes
native commitments, and authenticates the changed topology with actual provider and council
signatures. Its revocation targets that exact new envelope. A regression verifies that tagged
labels and noncanonical default-port origins remain rejected; no production rewrite is added.
These tests use explicit fixture authority and clock inputs;
they do not qualify finality, revocation activation timing, real grants or TLS.

The locked `...::https_source` Cargo filter passed all 17 tests against current `irohad`
composition. The exact copied native binary then passed all 74
`sorafs_provider_ingest_runtime::` tests, with zero ignored, in 0.53 seconds. This includes
persistent quarantine/restart and preflight validation. The shared preflight fixture now uses
the actual 192-MiB checkpoint default and validates itself before testing identity mutations.
Its earlier 160-MiB bound was too small for the current outbox geometry; production bounds
remain unchanged.

Evidence resides in the sibling SORA CARS checkout under
`output/qualification/sorafs-current-daemon-source/attempt-2/`: `report.json` records the Cargo
run; `all-runtime/report.json` records all 74 exact names; source snapshots and full logs are
retained with `native-daemon-source-tests`. The binary SHA-256 is
`09a1ba710ca814c02d6a61dd1eb817c627a5d862874cc870d4ad22b0b755a12a`.
The source snapshot after the preflight fixture repair matches compilation and execution.
The original queue snapshot remains retained, including the documented fixture change while
Cargo waited for its artifact lock. Attempt 1's five invalid transport-fixture failures and
the earlier `E0463` exact-source attempt remain historical evidence, not passing qualification.
The current pass excludes an installed authoritative resolver, real TLS, consensus finality
and live provider publication.

## Frozen SORA CARS package through native provider storage

The retained frozen `NF89bGDp` frontend passed the ignored native provider test in 32.92 seconds.
The complete directory contained 22 files, 201 chunks and 24,248,940 payload bytes; its canonical
CAR was 24,279,435 bytes. The exact root CID bytes are:

```text
01711f20a3d51650b836f62572c188386715db573edbdff7db1dc209d5405496f9124701
```

Evidence is retained in the SORA CARS checkout at
`output/qualification/sorafs-provider/NF89bGDp/{report.json,native-provider.log}`.
This run reuses the exact immutable executable retained at
`output/qualification/sorafs-provider/GlqbJ_vS/native-provider-tests`.
The copied native test executable SHA-256 is
`63135fb2d714d2faf5f515f03ab50e184da64cde22fe3718c34d4c2ae3a1ed5f`.
The matched inputs are `output/qualification/frontend/NF89bGDp/dist` and
`output/sorafs/first-release-environment-NF89bGDp`.
All file/chunk bytes, ranges, native CAR/plan commitments and provider restart were checked.
This remains native storage evidence; it does not establish remote provider transport, finalized
on-chain completion, CID publication or public availability.

The earlier `GlqbJ_vS` package pass also remains retained as historical evidence. The frontend
has since advanced beyond `NF89bGDp`; a new package requires its own storage qualification.
Storage evidence must not silently mix these directories with another package or CID.
