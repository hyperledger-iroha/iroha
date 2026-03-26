---
lang: ka
direction: ltr
source: docs/source/sdk/android/index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6e075c326db3e81bd69c992f0d95922e430ca756b324db7928f706d9879df33e
source_last_modified: "2026-01-30T18:06:03.307531+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Iroha Android SDK

The Android SDK (Java/Kotlin bindings located under `java/iroha_android`) wraps
the Norito codec, deterministic signing helpers, and Torii HTTP clients so
mobile apps can build and submit transactions without shelling out to Rust CLIs.
It currently targets the standard library only; Android platform bindings are
layered on top via the keystore abstraction when running on devices.

## Components

- **Key management:** `IrohaKeyManager` combines hardware-backed providers
  (Android Keystore/StrongBox once available) with the deterministic software
  fallback so aliases can be generated on emulators and desktop JVMs. The
  manager exports/imports HKDF-derived bundles for offline tooling.
- **Norito codec:** `NoritoJavaCodecAdapter` bridges to the in-repo Norito
  implementation (`java/norito_java`) providing canonical transaction encoding
  for IVM bytecode and wire-framed instruction payloads. Fixtures in
  `src/test/resources` mirror the Rust canonical payloads.
- **Transaction builder:** `TransactionBuilder` encodes payloads, signs with the
  requested key alias, and optionally emits `OfflineSigningEnvelope` records for
  later submission or key export workflows.
- **Multisig signatures:** `SignedTransaction` can attach optional
  `MultisigSignatures` bundles (made up of `MultisigSignature` entries) when
  the authority uses a multisig controller; set them via
  `SignedTransaction.Builder#setMultisigSignatures(...)` before submission.
- **Torii clients:** `HttpClientTransport` implements `/v1/pipeline/*`
  submission/polling with deterministic retries and pending-queue persistence,
  while `NoritoRpcClient` surfaces `application/x-norito` RPC calls using the
  same configuration (headers, observers, HTTP client). UAID helpers
  (`getUaidPortfolio`, `getUaidBindings`, `getUaidManifests`) reuse the same
  transport so wallets can query Space Directory bindings without reimplementing
  the REST surface (see [UAID portfolio & manifests](#uaid-portfolio--manifests)).
- **Fixtures & tooling:** `scripts/android_fixture_regen.sh` regenerates the
  Norito fixtures from the shared Rust exporter, and
  `scripts/check_android_fixtures.py` guards parity during CI.

## Quickstart

```java
import java.net.URI;
import java.net.http.HttpClient;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.norito.NoritoCodecAdapter;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.tx.TransactionBuilder;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.client.ClientConfig;
import org.hyperledger.iroha.android.client.HttpClientTransport;

NoritoCodecAdapter codec = new NoritoJavaCodecAdapter();
IrohaKeyManager keyManager = IrohaKeyManager.withDefaultProviders();
TransactionBuilder builder = new TransactionBuilder(codec, keyManager);

TransactionPayload payload = TransactionPayload.builder()
    .setChainId("00000000")
    .setAuthority("soraカタカナ...")
    .setCreationTimeMs(System.currentTimeMillis())
    .setExecutable(Executable.ivm(new byte[] { /* Kotodama bytecode */ }))
    .build();

SignedTransaction tx = builder.encodeAndSign(
    payload,
    "demo-alice",
    IrohaKeyManager.KeySecurityPreference.HARDWARE_PREFERRED);

ClientConfig config = ClientConfig.builder()
    .setBaseUri(URI.create("http://127.0.0.1:8080"))
    .build();

HttpClientTransport transport =
    new HttpClientTransport(HttpClient.newHttpClient(), config);

transport.submitTransaction(tx).join();
```

- `IrohaKeyManager.withDefaultProviders()` prefers hardware-backed providers
  when present and falls back to the software signer on emulators/desktop JVMs.
- For instruction lists, populate `TransactionPayload.setInstructions(...)`
  with `InstructionBox.fromWirePayload(...)` entries. Each payload must already
  include the Norito header/checksum; legacy argument-map instruction payloads
  are rejected by the encoder. Trigger registration instructions also require
  wire-framed nested instructions (`wire_name` + `payload_base64` only).
- `HttpClientTransport.submitTransaction` returns a `CompletableFuture`; use
  `waitForTransactionStatus` to poll `/v1/pipeline/transactions/status` for the
  resulting hash, or configure a `PendingTransactionQueue` so failed submissions
  replay deterministically. Torii returns a Norito-encoded submission receipt in
  the response body; `ClientResponse.body()` exposes the raw bytes and
  `ClientResponse.hashHex()` provides the canonical hash for polling.
  A `404` response indicates Torii has no cached status yet (for example after a
  restart), so the client keeps polling until a terminal state arrives.

## UAID portfolio & manifests

Space Directory APIs surface UAID bindings, balances, and manifest lifecycles so
wallets can explain which dataspace granted access to a controller. The Android
transport exposes typed helpers in `org.hyperledger.iroha.android.nexus`:

- `HttpClientTransport.getUaidPortfolio(String uaid)` returns a
  `CompletableFuture<UaidPortfolioResponse>` with per-dataspace totals and
  account labels pulled from `/v1/accounts/{uaid}/portfolio`. Use the overload
  that accepts a `UaidPortfolioQuery` when you need to filter by `asset_id`.
- `HttpClientTransport.getUaidBindings(String uaid)` hits
  `/v1/space-directory/uaids/{uaid}` when only the account bindings are needed.
  Supply a `UaidBindingsQuery` for forward-compatible query options (the endpoint currently returns canonical Katakana i105 literals only).
- `HttpClientTransport.getUaidManifests(String uaid, UaidManifestQuery query)`
  fetches `/v1/space-directory/uaids/{uaid}/manifests`; the query builder lets
  you filter by dataspace, status (`active`, `inactive`, `all`), and paging offsets.
- `UaidLiteral.canonicalize(literal, context)` normalises user input with the
  `uaid:` prefix and 64‑hex (LSB=1) enforcement so controllers can be pasted in any
  casing.

Example:

```java
import org.hyperledger.iroha.android.nexus.UaidLiteral;
import org.hyperledger.iroha.android.nexus.UaidManifestQuery;
import org.hyperledger.iroha.android.nexus.UaidPortfolioQuery;
import org.hyperledger.iroha.android.nexus.UaidPortfolioResponse;

String uaid = UaidLiteral.canonicalize("  UAID:DEADBEEF...  ", "lookup uaid");
UaidPortfolioQuery portfolioQuery =
    UaidPortfolioQuery.builder().setAssetId("61CtjvNd9T3THAR65GsMVHr82Bjc#soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ").build();
UaidPortfolioResponse portfolio = transport.getUaidPortfolio(uaid, portfolioQuery).join();
portfolio.dataspaces().forEach(ds -> {
    String alias = ds.dataspaceAlias();
    System.out.printf("Dataspace %d (%s) exposes %d account(s)%n",
        ds.dataspaceId(),
        alias == null ? "<unnamed>" : alias,
        ds.accounts().size());
});

UaidManifestQuery query = UaidManifestQuery.builder()
    .setDataspaceId(42L)
    .setStatus(UaidManifestQuery.UaidManifestStatusFilter.ACTIVE)
    .setLimit(10)
    .build();

transport.getUaidManifests(uaid, query).join()
    .manifests()
    .forEach(manifest ->
        System.out.println(manifest.manifestHash() + " => " + manifest.status()));
```

Errors bubble up through the returned `CompletableFuture` with the same taxonomy
as Torii (invalid account literals, malformed UAIDs, etc.), so reuse the
helpers before submitting values to ledger instructions. For the full JSON
schema, see [`docs/source/torii/portfolio_api.md`](../../torii/portfolio_api.md).

## Build & Test

```bash
make android-tests            # compiles Java sources and runs assertion-based tests
make android-fixtures         # regenerates Norito fixtures + manifest
make android-fixtures-check   # validates fixtures against canonical hashes
```

Requirements:

- JDK 21+ (`javac`/`java` must be on `PATH` or reachable via `JAVA_HOME`)
- Python 3.11+ for fixture checks
- Access to the Rust workspace when regenerating fixtures

`ci/run_android_tests.sh` mirrors CI: it compiles the Norito codec and
SDK sources together, executes the test mains listed in the script, and finally
invokes `scripts/check_android_fixtures.py` to guard parity.

## CUDA acceleration

CUDA helpers are shipped behind the deterministic `CudaAccelerators` facade
with a Kotlin-friendly wrapper in `CudaAcceleratorsKotlin`. The native backend
remains disabled by default; enable it with `-Diroha.cuda.enableNative=true`
and consult the CUDA operator guide (`gpu_operator_guide.md`) for setup and the
manual smoke harness (`IROHA_CUDA_SELFTEST=1 ...run_tests.sh --tests
org.hyperledger.iroha.android.gpu.CudaAcceleratorsNativeSmokeTests`).

## Pending Work

Roadmap items tracked under `AND2`–`AND5` extend this snapshot with platform
keystore bindings (StrongBox attestation), generated instruction builders,
telemetry hooks, and distribution artifacts. See `roadmap.md` (Android section)
and `status.md` for the latest progress notes. For the upcoming observability
milestone (`AND7`), refer to the companion
[`Android Telemetry Redaction Plan`](telemetry_redaction.md) for signal
inventory, policy deltas, and readiness deliverables ahead of the SRE
governance review. Enablement artefacts live under
`docs/source/sdk/android/readiness/`.

For the AND3 documentation automation work, see the
[`Android Codegen Documentation Tooling Scope`](codegen_doc_tooling_scope.md).
It captures the manifest formats, CLI plan, and CI wiring required to keep the
generated instruction builder docs in sync with Norito schema changes.
Generated references live under `docs/source/sdk/android/generated/` and are
regenerated via `make android-codegen-docs`. The same pipeline now emits
`manifest_catalog.md`, which cross-references every `InstructionBox`
discriminant with its Rust type, schema hash, and generated builder metadata so
Docs/DevRel have a single index when reviewing the AND3 roadmap evidence.

For the AND3 parity dashboard rollout (fixture drift evidence + Grafana plan),
see the [`Android Norito Parity Dashboard Plan`](parity_dashboard_plan.md). It
details how `scripts/check_android_fixtures.py --json-out …` feeds the parity
JSON artefacts, how Buildkite promotes them into
`artifacts/android/parity/<stamp>/summary.json`, and how the resulting feed
becomes a release gate plus the source for the Grafana panels referenced in the
roadmap.

When `InstructionBox` discriminants change, record the delta in the
[`Android Norito Instruction Change Log`](norito_instruction_changes.md). Each
row ties the exported manifest hash to the `iroha_data_model` commit,
Android-specific follow-up, and the parity evidence bundle so governance can
confirm AND3’s determinism guarantees.

- StrongBox attestation capture guidelines:
  `readiness/android_strongbox_attestation_bundle.md` breaks down the bundle
  layout (`chain.pem`, `challenge.hex`, `alias.txt`) and ties into the
  `scripts/android_keystore_attestation.sh` verifier harness for lab workflows.
- `IrohaKeyManager.generateAttestation(alias, challenge)` requests fresh
  hardware-backed attestation for an existing alias and surfaces the resulting
  `KeyAttestation` bundle when the active provider supports it (StrongBox/TEE).
- `TransactionBuilder.encodeAndSignEnvelopeWithAttestation(...)` returns an
  `OfflineTransactionBundle` combining the offline envelope and any generated
  attestation so mobile apps can forward both artifacts together.

For a deeper dive into alias policies, deterministic exports, StrongBox
attestation, and integration hooks, read the
[`Android Key Management & Attestation Guide`](key_management.md); it captures
the AND2/AND5 requirements, sample app expectations, and compliance links
highlighted in the roadmap.

Need end-to-end instructions for offline workflows? See the companion
[`Android Offline Signing & Envelope Guide`](offline_signing.md) for envelope
format details, queue integration, attestation pairing, and operational
checklists referenced by AND5 deliverables.

Need deeper coverage of Torii HTTP configuration, Norito RPC, retry policies,
and telemetry observers? The
[`Android Networking & Telemetry Guide`](networking.md) maps `ClientConfig`,
`HttpClientTransport`, pending queues, and `ClientObserver` hooks to the AND4 /
AND7 roadmap so mobile builds track the same behaviour as Rust nodes and the
Android runbook.

Looking for the NRPC-3 architecture reference (flow control, fallback, codec
reuse, and readiness evidence)? See the
[`Norito RPC Android Client Architecture`](norito_rpc_client_architecture.md)
write-up for the detailed component map, lifecycle, and testing checklist that
back the AND4/NRPC deliverables.

Hardware-security acceptance criteria for AND2/AND6 (attestation bundles,
device-matrix coverage, tamper-proof logs, and disclosure packs) now live in
the [`Android Security & Compliance Evidence`](security.md) guide. Reference it
whenever StrongBox harness updates land or when assembling compliance evidence
for partner releases.

Preparing the AND8 partner pilot and support rollout? The
[`Android Partner SLA Discovery Plan`](partner_sla_discovery.md) explains how to
run the SLA discovery cadence, capture blackout windows, and archive evidence
before the governance gate. Pair it with the notes template under
`docs/examples/android_partner_sla_notes_template.md` when recording sessions.

## Connect Sessions (AND7 Preview)

Android wallets share the same Connect contract documented in
`docs/source/connect_architecture_strawman.md`. The Java SDK already ships the
building blocks:

- `ConnectRetryPolicy` (`java/iroha_android/src/main/java/.../connect/ConnectRetryPolicy.java`)
  implements the agreed exponential back-off (base 5 s, cap 60 s, full jitter) so
  reconnects match Swift/JS behaviour.
- `ConnectQueueConfig` + `ConnectQueueError` persist the bounded FIFO queue as Norito `.to`
  blobs (encrypted via EncryptedSharedPrefs/SQLCipher) and surface deterministic overflow /
  expiry diagnostics. The queue journal schema mirrors `{sid, direction, sequence,
  timestamp_ms, payload_hash}` from the strawman.
- `ConnectError` + `ConnectErrors` emit the shared taxonomy
  (`Transport|Codec|Authorization|Timeout|QueueOverflow|Internal`) with telemetry hooks so
  `connect.queue_depth`, `connect.queue_dropped_total`, and `connect.replay_success_total`
  feed the OpenTelemetry exporter described in the roadmap.
- Flow-control windows arrive as `ConnectEvent.FlowControl` instances. Dequeue ciphertext
  only when a window token is available, and call `ConnectEvent.Resume` after the wallet
  replays buffered frames so dashboards can plot `{seqAppMax, seqWalletMax, queueDepths}`.
- Attestations (`{platform, evidence_b64, statement_hash}`) are generated whenever keys live
  in StrongBox; wallet approvals pass them through automatically so dApps can verify device
  posture.

Sample code lives in `java/iroha_android/src/test/java/org/hyperledger/iroha/android/connect/`
(`ConnectErrorTests`, `ConnectRetryPolicyTests`). Once AND7 lands, the retail-wallet
sample will light up the Connect UI, showing SID generation, resume summaries, and queue
depth telemetry end-to-end.

## Sample Apps (AND5 Complete)

- **Operator Console:** `examples/android/operator-console` — governance/operator
  flows that display StrongBox posture, Torii health, queue depth, harness
  artefact paths (SoraFS scoreboard/summary/receipts + SHA-256 digests), and
  telemetry/handoff hints exported by `android_sample_env.sh`.
- **Retail Wallet:** `examples/android/retail-wallet` — offline
  envelope/recovery demo wired to the SDK. It previews attested envelopes,
  renders address QR codes, surfaces revocation/policy snapshots, and mirrors the
  POS manifest/policy assets shipped in the APK (with hashes recorded in the
  sample manifest) so OA12 drills and recovery flows are reproducible.

Build either sample from the repo root:

```bash
cd examples/android
./gradlew :operator-console:assembleDebug
./gradlew :retail-wallet:assembleDebug
```

Or run `scripts/check_android_samples.sh` (from the repo root) to assemble both apps plus the shared Java SDK module in a single step—ideal for CI smoke coverage.

The samples link directly against the in-repo Java SDK and surface live data
(StrongBox posture, sample transaction hashes, offline envelope previews) and
their manifests record sandbox provenance: Torii endpoints, feature flags,
SoraFS artefacts/digests, telemetry logs, policy overrides, POS asset hashes,
and localization coverage (`en`, `ja`, `he`). Use the manifests inside release
bundles to prove which inputs a given demo used.
