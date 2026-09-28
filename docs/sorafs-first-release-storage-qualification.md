# SoraFS first-release publication qualification

This record describes the source as of September 26, 2026 and retains earlier SORA CARS
validation evidence separately. Iroha 3 is unreleased; obsolete formats and publication aliases
are rejected. Current integrated compilation and four-validator publication qualification remain
open; historical binaries below do not validate the new source delta.

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

`deploy` requires an independently supplied canonical `SumeragiFinalityCheckpoint`,
bound to the configured genesis-derived network and chain label. The shared current-consensus
verifier resumes from its signed genesis, selected roster, certified tip and at most two
predecessor decisions; a queried proof cannot select its own trust root. The checkpoint is
bounded to 68 MiB before canonical decoding. It verifies challenged
`AssertSorafsPublicationV1` transaction inclusion and embedded-certificate finality for
the approved manifest and automatic assignment, then uploads bounded, authenticated source
chunks to `/v1/sorafs/publish/source` for every incomplete assigned provider. Staging verifies
the manifest and native plan and checks each content-addressed chunk; ordinary finalized ingest
must still reproduce complete payload, CAR and proof commitments before admitting the replica.
Staging neither completes the ledger order nor writes a directly admitted pin.

The publisher awaits the same assignment's native completion evidence, verifies a second
challenged finality proof extending the trusted floor, and retains assignment/completion proofs
and the new finality checkpoint. Only verified execution advances that checkpoint through
a private same-directory file, synced atomic replacement and directory sync. It then checks
every asset at the CID origin. The receipt sets
`publication_verified` only when this evidence and all gateway reads succeed. Endpoint success
or bytes alone are insufficient. The implementation is present; live four-validator acceptance
and the frozen package run through this complete path remain qualification gates.

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

Historical Cargo evidence recorded September 6, 2026: the three native provider tests passed, with the
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
- The supervised provider-ingest runtime supports explicit native software custody through
  `native_completion_credential`, or qualified external adapters. Native assembly uses the
  daemon's State, governed completion authority, typed assignment source authorization and a
  durable local checkpoint CAS; external injection cannot replace a selected native adapter.
  Configured source origins provide remote acquisition after authenticated publisher staging.
  No retired `/v1/sorafs/storage/pin` route is restored.
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

## Current native implementation boundaries

| Component | Source | Behavior and limit |
| --- | --- | --- |
| Publisher source | `sorafs_cli/deploy_publication.rs`, Torii publisher routes, `sorafs_node` staging | Authenticated assignment-bound chunk transfer and bounded staging; completion requires normal verified ingest. |
| Assigned provider source | `sorafs_provider_ingest_runtime/native_source.rs`, Core `query/provider_ingest_source.rs` | Requester signatures bind the exact network and typed source request. Same-State durable finality, owner permission, both provider admissions, assignment revision and retained pin authorize reads. The reader fetches and verifies one chunk at a time under one operation deadline. Configured origins use HTTPS, with explicit loopback HTTP for local networks. |
| Repair source | `irohad/src/sorafs_repair_source.rs`, Core `query/repair_source.rs`, Torii repair route | Origins must match current admitted signed adverts. HTTPS is required except for explicitly configured numeric loopback HTTP origins. A typed request requires the exact active repair lease, requester permission, source/target admission and pin. The worker consumes one verified chunk at a time, rechecks live authority before mutation and proof release, and preserves quarantine on failure. |
| Native completion signer | `sorafs_provider_ingest_runtime/native_software.rs` | An explicit owner-only credential must match the public binding. The signer rechecks native authority, assignment, manifest and exact completion payload. It cannot publish a transaction itself. |
| Native transaction roles | `irohad/src/sorafs_native_software_signers.rs` | Separate explicit `software_credential` selection for proof-outcome, repair, reserve and orderbook. Credentials are loaded after State exists; validator-key reuse, shared roles and native/external ambiguity are rejected. Existing qualification probes and forwarder permission checks remain mandatory. |
| Local checkpoint | `sorafs_provider_ingest_runtime/native_software.rs` | Owner-only files, process lock and in-process mutex, bounded canonical decoding, exact predecessor/sequence CAS, file sync, atomic rename and directory sync. This is crash-durable software state, not a hardware rollback seal. Finalized ledger replay independently reconciles restored local state. |
| Publication proof | `sorafs_cli/deploy_publication.rs`, native publication query/ISI | Challenge-bound transaction inclusion, exact manifest/order/completion, pinned network/finality ancestry and every-asset retrieval. Current live qualification remains open. |

Publisher, assigned-source and repair requests are typed operations, not generic transaction or
storage proxies. Private keys remain runtime-only; configuration contains explicit credential
paths and public bindings. Provider source reads do not require the publisher's wallet key.
The ordinary public gateway continues to require its protocol handshake and governed stream
token. Native assignment/repair routes derive their narrower read authority from their specific
finalized ledger operation.

## Optional external custody and gateway-source composition

Selecting external adapters still requires the exact governed completion resolver, qualified
checkpoint service and authenticated source provider. Existing Unix broker clients retain
bounded transport and streamed EOF authentication. External archive retention authority is
required only when that retention mode is selected; native software mode refuses to pretend it
supplies external retention or Musubi attestation services.

The reusable gateway HTTPS source below has a distinct governed stream-grant resolver contract.
That external composition requires an authenticated issuer, independent TLS/key pins and fresh
revocation evidence; a local advert or endpoint URL cannot manufacture a grant. Native assigned
source acquisition uses the narrower typed native route described above. Neither path implies
hardware custody or platform qualification from shared broker transport.

Qualification must cover corruption, truncation, substituted network/provider/manifest/token,
revoked permissions/admission, expired or superseded assignments/leases, deadline and resource
limits, process interruption, checkpoint restart, and identical full commitment checks. Final
acceptance requires four actual validators and provider processes producing finalized
registration, assignment and completion, followed by independent CID-origin asset retrieval.
Retain exact genesis/trust context, transaction/block evidence, package hashes and provider
identities. Source implementation alone is not this evidence.

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
reproduced before a payload reader is exposed. The consuming scheduler writes ordered verified
chunks into a private temporary file under the configured complete-payload disk bound (at most
8 GiB); dropping the reader removes the file. Payload reservations and metadata inventories are
bounded separately, with no complete payload or CAR buffer. The existing broker reader still
authenticates its exact streamed EOF independently.

Historical gateway/leaf snapshot tests cover explicit TLS-root bounds and use real loopback HTTP and native multi-file plans to cover complete
pagination, redirect rejection, truncated HTTP bodies, changed chunk bytes, later-page
substitution and malformed resource bounds. Loopback is injected only through the existing
crate-private test engine; it does not qualify TLS, governance, public routing or deployment.
Five daemon leaf tests cover exact request/lease bindings, explicit resolver refusal, retained
admission bounds, sticky reader failure, revocation at EOF and expiry. The retained historical Cargo gateway
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
It reuses the retained coherent native dependency closure and verifies the exact leaf/gateway
source and native payload-wrapper bytes in that historical snapshot. This pass additionally covers a shared admission held
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
The source count cannot multiply the concurrent-spool allowance. Public leaf policies all carry
the same pool admission count. Catalog `max_content_bytes` currently comes from total local
storage capacity; the leaf's maximum temporary payload may be smaller (at most 8 GiB), and a
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

The retained historical daemon Cargo gate passed, including all six composition tests. This
supersedes the narrower 39-test exact-source evidence for daemon compilation and constructor
composition; it does not install an authenticated grant resolver or qualify remote publication.

For the external gateway-source composition, provider owners, signed adverts and completion
records alone do not form the exact assignment/advert/key/grant lease required by that contract.
Its deployment registry must supply the authenticated stream-grant resolver and governed TLS/key
pins. The current native assigned-source implementation instead exposes an operation-specific
read route authorized directly from same-State finalized assignment and admission records. This
resolves native provider acquisition without making the worker's claim factory public or
fabricating a stream token.

## External gateway evidence validation and historical qualification

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

The external `ProviderIngestFinalizedLedgerV1` claim factory remains private to the worker;
ordinary authorization values carry no read capability. External stream-grant resolvers still
need their own authenticated authority. The native typed source route uses Core's current
assignment read and account-authenticated request instead; it does not expose that factory or
a completion-signing capability.

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

The retained historical locked `...::https_source` Cargo filter passed all 17 tests against its
`irohad` composition snapshot. The exact copied native binary then passed all 74
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
That historical pass excludes an installed authoritative resolver, real TLS, consensus finality
and live provider publication. It also predates the native producer, typed source/repair routes,
streaming retrieval changes and completed publisher workflow described above.

## Frozen SORA CARS package through native provider storage

The historical frozen `NF89bGDp` frontend passed the ignored native provider test in 32.92 seconds.
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
