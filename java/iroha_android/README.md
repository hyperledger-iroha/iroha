# IrohaAndroid

`IrohaAndroid` provides a native Android library that wraps Hyperledger Iroha
capabilities for Kotlin/Java mobile applications. The library will expose
key management (including secure-element backed keys), transaction building and
signing, Norito serialization helpers, and networking clients for interacting
with Iroha nodes.

This snapshot covers the offline key management façade, Norito encoding backed
by the shared `norito-java` implementation, the Android Keystore/StrongBox
backend (with cached attestations + deterministic software fallbacks), and
scaffolding for network clients.

## Gradle quickstart

Point Gradle at the repository that hosts the SDK (the default
`ci/publish_android_sdk.sh` output lives under `artifacts/android/maven`) and
depend on the surface you need:

```kotlin
repositories {
    google()
    mavenCentral()
    maven { url = uri("../../artifacts/android/maven") } // or your Nexus
}

dependencies {
    implementation("org.hyperledger.iroha:iroha-android:<version>") // Android AAR
    implementation("org.hyperledger.iroha:iroha-android-jvm:<version>")     // JVM tooling
}
```

The sample app (`:samples-android`) can validate AAR consumption from a local
repository with:

```bash
./gradlew -p java/iroha_android :samples-android:assembleDebug \
  -PirohaAndroidUsePublished=true \
  -PirohaAndroidRepoDir=$PWD/../artifacts/android/maven
```

It defaults to the local snapshot repository when it exists and falls back to
the in-repo project dependency otherwise. Set `irohaAndroidVersion` to match the
published coordinates when consuming from Maven.

## Account addresses

```java
import org.hyperledger.iroha.android.address.AccountAddress;

byte[] key = new byte[32];
AccountAddress address = AccountAddress.fromAccount("default", key, "ed25519");
System.out.println(address.canonicalHex());
System.out.println(address.toIH58(753));
System.out.println(address.toCompressedSora());

AccountAddress.DisplayFormats formats = address.displayFormats();
System.out.println(formats.ih58);
System.out.println(formats.compressed);
System.out.println(formats.compressedWarning);
```

Use `displayFormats()` whenever UI layers need to render or copy addresses so the warning text and
network prefix stay aligned with `docs/source/sns/address_display_guidelines.md`.

## Multisig specs and TTL preview

```java
import org.hyperledger.iroha.android.multisig.MultisigProposalTtlPreview;
import org.hyperledger.iroha.android.multisig.MultisigSpec;

MultisigSpec spec =
    MultisigSpec.builder()
        .setQuorum(3)
        .setTransactionTtlMs(86_400_000L)
        .addSignatory("alice@wonderland", 2)
        .addSignatory("bob@wonderland", 1)
        .build();

MultisigProposalTtlPreview preview = spec.enforceProposalTtl(90_000L, System.currentTimeMillis());
System.out.println("effective ttl: " + preview.effectiveTtlMs());
System.out.println("expires at: " + preview.expiresAtMs());
```

`enforceProposalTtl` rejects TTL overrides above the policy cap (`transaction_ttl_ms`) before
submission so apps can surface the same error Torii would return. Use
`previewProposalExpiry` when you only need a preview (cap + expiry) without throwing.
When registering a multisig controller, supply an explicit account id in the same domain as the
signatories (random keys are fine—the controller must never be used for direct signing). Nodes now
quarantine deterministically derived controller ids and will reject registration and subsequent
propose/approve attempts that use them.

```java
import java.util.Base64;
import org.hyperledger.iroha.android.model.InstructionBox;

InstructionBox registerMultisig =
    InstructionBox.fromWirePayload(
        "<WIRE_NAME_REGISTER_MULTISIG>",
        Base64.getDecoder().decode("<WIRE_PAYLOAD_BASE64>")); // Replace with wire payload bytes.
```

## Subscriptions

```java
import java.net.URI;
import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.client.SubscriptionToriiClient;
import org.hyperledger.iroha.android.subscriptions.SubscriptionCreateRequest;
import org.hyperledger.iroha.android.subscriptions.SubscriptionCreateResponse;
import org.hyperledger.iroha.android.subscriptions.SubscriptionPlanCreateRequest;
import org.hyperledger.iroha.android.subscriptions.SubscriptionPlanCreateResponse;
import org.hyperledger.iroha.android.subscriptions.SubscriptionUsageRequest;

SubscriptionToriiClient client =
    SubscriptionToriiClient.builder()
        .baseUri(URI.create("https://example.com"))
        .build();

Map<String, Object> plan = new LinkedHashMap<>();
plan.put("kind", "fixed");
plan.put("price", "120");
plan.put("period", "month");

SubscriptionPlanCreateResponse planResponse =
    client.createSubscriptionPlan(
            SubscriptionPlanCreateRequest.builder()
                .authority("aws@commerce")
                .privateKey("<hex>")
                .planId("aws_compute#commerce")
                .plan(plan)
                .build())
        .join();

SubscriptionCreateResponse subscriptionResponse =
    client.createSubscription(
            SubscriptionCreateRequest.builder()
                .authority("alice@wonderland")
                .privateKey("<hex>")
                .subscriptionId("sub-001$subscriptions")
                .planId("aws_compute#commerce")
                .build())
        .join();

client.recordSubscriptionUsage(
        "sub-001$subscriptions",
        SubscriptionUsageRequest.builder()
            .authority("aws@commerce")
            .privateKey("<hex>")
            .unitKey("compute_ms")
            .delta("3600000")
            .build())
    .join();
```

## Layout

```
java/iroha_android
├── src
│   ├── main/java
│   │   └── org/hyperledger/iroha/android
│   │       ├── IrohaKeyManager.java
│   │       ├── SigningException.java
│   │       ├── client
│   │       │   ├── ClientConfig.java
│   │       │   ├── ClientResponse.java
│   │       │   ├── HttpClientTransport.java
│   │       │   └── IrohaClient.java
│   │       ├── crypto
│   │       │   ├── KeyProviderMetadata.java
│   │       │   ├── Ed25519Signer.java
│   │       │   ├── SoftwareKeyProvider.java
│   │       │   └── keystore
│   │       │       ├── KeystoreBackend.java
│   │       │       ├── KeystoreKeyProvider.java
│   │       │       └── KeyGenParameters.java
│   │       ├── gpu
│   │       │   └── CudaAccelerators.java
│   │       ├── model
│   │       │   ├── Executable.java
│   │       │   └── TransactionPayload.java
│   │       ├── norito
│   │       │   ├── NoritoCodecAdapter.java
│   │       │   ├── NoritoException.java
│   │       │   ├── NoritoJavaCodecAdapter.java
│   │       │   └── TransactionPayloadAdapter.java
│   │       ├── offline
│   │       │   ├── OfflineToriiClient.java
│   │       │   ├── OfflineListParams.java
│   │       │   ├── OfflineAllowanceList.java
│   │       │   ├── OfflineTransferList.java
│   │       │   ├── OfflineAuditLogger.java
│   │       │   └── OfflineWallet.java
│   │       ├── subscriptions
│   │       │   ├── SubscriptionPlanCreateRequest.java
│   │       │   ├── SubscriptionCreateRequest.java
│   │       │   ├── SubscriptionListResponse.java
│   │       │   └── SubscriptionToriiException.java
│   │       └── tx
│   │           ├── SignedTransaction.java
│   │           └── TransactionBuilder.java
│   └── test/java
│       └── org/hyperledger/iroha/android
│           ├── IrohaKeyManagerTests.java
│           ├── client/HttpClientTransportTests.java
│           ├── crypto/keystore/KeystoreKeyProviderTests.java
│           ├── gpu/CudaAcceleratorsTests.java
│           ├── norito/NoritoCodecAdapterTests.java
│           └── tx/TransactionBuilderTests.java
├── src/test/resources
│   └── transaction_payloads.json
├── schemas
│   └── norito_schema_manifest.json
```

## Modules & dependencies

- `:core` — shared Java sources used by both targets.
- `:android` — Android AAR surface (OkHttp transport, keystore helpers) published as
  `org.hyperledger.iroha:iroha-android`.
- `:jvm` — JVM jar (java.net.http transport) published as `org.hyperledger.iroha:iroha-android-jvm`.
- `:samples-android` — launchable demo that exercises Norito encoding and address helpers against the
  AAR; build with `gradle -p java/iroha_android :samples-android:assembleDebug`.

When consuming the workspace via an included build:

```kotlin
dependencies {
    implementation(project(":android")) // Android apps
    implementation(project(":jvm"))     // JVM tooling
}
```

Published artefacts can be pulled directly when the composite build is not in play:

```kotlin
dependencies {
    implementation("org.hyperledger.iroha:iroha-android:${IROHA_ANDROID_VERSION}") // Android (AAR)
    implementation("org.hyperledger.iroha:iroha-android-jvm:${IROHA_ANDROID_VERSION}") // JVM
}
```

The `:samples-android` app defaults to the in-repo `:android` project but can
consume a published Maven repo by setting `irohaAndroidUsePublished=true`
(or `ANDROID_SAMPLE_USE_PUBLISHED=1`) and pointing `irohaAndroidRepoDir` at
`artifacts/android/maven/<version>` from `ci/publish_android_sdk.sh`.
`MainActivity` renders an IH58 address from the AAR, and `SampleAddressTest`
keeps the wiring green for the published-vs-project toggle.

## Build & Test

The Gradle harness targets JDK 21 and wires the included `norito-java` build automatically. From the
repository root:

```bash
bash ci/run_android_tests.sh
# or
make android-tests
# or run a subset of tasks
ANDROID_GRADLE_TASKS=":core:check :android:testDebugUnitTest" bash ci/run_android_tests.sh
```

Use `ANDROID_HARNESS_MAINS` (comma-separated class names) to filter the main-based harnesses, and
`ANDROID_GRADLE_TASKS` to override the Gradle task list.

`gradle -p java/iroha_android :core:check` runs the shared JUnit/parameterised harnesses with
assertions enabled, enforces the pinned Norito schema manifest (`verifyNoritoSchemas`), and calls
`checkAndroidFixtures` (formerly `scripts/check_android_fixtures.py`) so the fixture manifest stays
in sync. Lint and JVM publishing are covered by the default CI task list above, and
`:android:testDebugUnitTest` exercises the Android-only harness alongside the shared tests.

Set `NORITO_JAVA_VERSION=<version>` to exercise a different Norito drop; the manifest guard fails if
it diverges from `schemas/norito_schema_manifest.json`.

`PlatformHttpTransportExecutor` now prefers the Android OkHttp factory when present (stubbed in core
tests), falling back to the JDK client elsewhere. The Android module exposes
`OkHttpTransportExecutorFactory` for callers that want to reuse a shared `OkHttpClient`.

To keep JVM-only transports out of the Android artefacts, run the guard after producing an AAR:

```bash
make android-transport-guard
# or provide a custom classes.jar/aar
ANDROID_TRANSPORT_GUARD_AAR=java/iroha_android/android/build/outputs/aar/android-release.aar \
  bash ci/check_android_transport_guard.sh
# or
bash ci/check_android_transport_guard.sh /path/to/classes.jar
```

### Transport defaults and troubleshooting

- Android builds default to OkHttp transports backed by a shared connection pool; JVM builds default
  to the JDK executor shipped in the `:jvm` artefact. `PlatformHttpTransportExecutor` selects OkHttp
  when the Android factory is on the classpath and falls back to `JavaHttpExecutorFactory`
  otherwise. JVM callers that want an explicit JDK transport can use
  `JavaHttpExecutorFactory.createTransport(...)`.
- Builders now auto-wire platform defaults: `HttpClientTransport.withDefaultExecutor(...)`,
  `ToriiEventStreamClient.builder()` (without `setTransportExecutor(...)`),
  `SorafsGatewayClient.builder()`, and `HttpSafetyDetectService.createDefault(...)` all pick the
  platform executor so Android apps land on the shared OkHttp client without extra wiring.
- WebSockets follow the same split: `PlatformWebSocketConnector` prefers
  `OkHttpWebSocketConnectorFactory` when present and falls back to
  `JdkWebSocketConnectorFactory` on JVM builds. Inject `setTransportExecutor`/`setWebSocketConnector`
  in client builders when you want to reuse a shared OkHttp client.
- Android artefacts must not contain `java.net.http` bytecode. The `android-and6` workflow now runs a
  `transport-guard` job that assembles the release AAR and executes
  `ci/check_android_transport_guard.sh` (also available locally via `make android-transport-guard`)
  to fail when JVM-only classes leak into the Android bundle.
- Guard failures usually mean the wrong artefact was scanned (set `ANDROID_TRANSPORT_GUARD_AAR` to
  the release bundle if you used a custom output path) or a
  JVM-only module was added as an Android dependency. Rebuild with
  `gradle -p java/iroha_android :android:assembleRelease` and rerun the guard before publishing.
- Consumers can supply a custom `OkHttpClient` via `OkHttpTransportExecutorFactory` on Android (the
  default factory uses the shared client provider); JVM callers can opt into
  `JavaHttpExecutorFactory` when `java.net.http` is available on the module path.
- When using a custom `OkHttpClient` (for example with certificate pinning) and you need to release
  resources, call `HttpClientTransport.invalidateAndCancel()` (or
  `HttpTransportExecutor.invalidateAndCancel()`) to cancel in-flight requests and clean up the
  underlying dispatcher/connection pool.
- For Android-first apps that want a single transport surface, `AndroidClientFactory` constructs
  HTTP/Norito RPC/SSE/WebSocket/Safety Detect/SoraFS clients around a shared `OkHttpClient`,
  threading through `ClientConfig` headers/observers (including telemetry).

Tests rely on Java assertions (enabled in the Gradle test tasks). Make sure `JAVA_HOME` points to a
JDK 21 or newer installation before running the harness.

Android Foundations pins this workspace to **JDK 21 LTS**. Possible upgrades are only evaluated after
Oracle’s quarterly CPU releases: stage the candidate by setting `ANDROID_JDK_NEXT=1` in Buildkite
so the Gradle harness and `scripts/android_fixture_regen.sh` soak the alternate toolchain. Capture
the CI logs and record the soak decision in `artifacts/android/fixture_runs/` and `status.md` before
promoting a new JDK.

`scripts/check_android_fixtures.py` keeps the checked-in Norito fixtures and
manifest in sync with the canonical hash metadata. Verify schema pinning with:

```bash
gradle -p java/iroha_android :core:verifyNoritoSchemas
```

Refresh the manifest after Norito schema updates via:

```bash
gradle -p java/iroha_android :core:regenNoritoSchemaManifest
```

Commit the updated `schemas/norito_schema_manifest.json` alongside any Norito changes.

To produce a combined CI-friendly report that exercises both the test harness
and the fixture parity gate, run:

```bash
python3 scripts/android_test_report.py --run-tests --output artifacts/android/test_report.json
```

The helper executes `ci/run_android_tests.sh`, writes the per-step summaries
under `artifacts/android/`, and exits non-zero if either the tests or fixture
parity fail (set `--allow-failures` only when you need a report without failing
the shell).

### Deterministic export & recovery

- `SoftwareKeyProvider.exportDeterministic(...)` emits v3 bundles with per-export salt/nonce, `kdf_kind`,
  and `kdf_work_factor`. Argon2id (64 MiB, 3 iterations, parallelism = 2) is preferred, with a
  PBKDF2-HMAC-SHA256 fallback at 350 k iterations. Passphrases must be ≥12 characters and the importer
  rejects all-zero salt/nonce seeds.
- `SoftwareKeyProvider` can persist deterministic exports by wiring a `KeyExportStore` plus
  `KeyPassphraseProvider` (for example, `FileKeyExportStore` on Android/JVM, or
  `InMemoryKeyExportStore` in tests). The provider rehydrates keys from the store before generating
  new material, keeping software-backed accounts stable across app restarts.
- `KeyExportBundle.decode(Base64|bytes)` accepts the v3 payload only. Treat
  salt/nonce/ciphertext errors as tampering and capture a fresh bundle rather than reusing an old
  export between devices.
- Regression coverage in `DeterministicKeyExporterTests` includes wrong passphrases and tampered
  salt/nonce/ciphertext, and all-zero seed rejection. Clear passphrase char arrays after use in
  application code.
- `KeystoreKeyProviderTests` exercises empty vs challenged attestation regeneration and the
  `android.keystore.attestation.failure` telemetry path; run
  `ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.crypto.keystore.KeystoreKeyProviderTests \
  bash ci/run_android_tests.sh` to spot-check cache and challenge matrices without rebuilding the
  full suite.

### Lint & dependency scanning (AND6 prototype)

Run

```bash
make android-lint
```

from the repository root to execute the AND6 static-analysis prototype
(`ci/check_android_javac_lint.sh`). The script reuses the lightweight javac
target used by `ci/run_android_tests.sh`, enables `-Xlint:all` with warnings treated as
errors, and then invokes `jdeps --summary` to ensure the Android surface only
depends on the approved JDK modules (`java.base`, `java.net.http`,
`jdk.httpserver`). Any new module dependency causes the lint run to fail so
publish jobs can gate on the same policy.

Every run copies the generated `jdeps` summary to
`artifacts/android/lint/jdeps-summary.txt` so CI/release artefacts always have
an up-to-date module list. Set
`ANDROID_LINT_KEEP_WORKDIR=1 make android-lint` to preserve the workspace for
manual inspection and/or provide
`ANDROID_LINT_SUMMARY_OUT=artifacts/android/lint/<tag>/jdeps-summary.txt make android-lint`
when you need an additional, versioned copy for compliance packets.

### Norito fixture rotation

Android fixtures follow the shared bi-weekly sync with the Rust maintainers and
carry a 48 hour regeneration SLA whenever discriminants or ABI hashes move.
Run

```bash
ANDROID_FIXTURE_ROTATION_OWNER="<name>" \
ANDROID_FIXTURE_CADENCE="twice-weekly-tue-fri-0900utc" \
make android-fixtures
make android-fixtures-check
```

to regenerate fixtures, record the run in
`artifacts/android/fixture_runs/`, and commit the updated fixtures plus
`artifacts/android_fixture_regen_state.json`. Tuesday and Friday windows fire at
09:00 UTC; keep the cadence label set to
`twice-weekly-tue-fri-0900utc` unless governance explicitly schedules a
one-off run. When the review requests a fixed identifier, provide
`ANDROID_FIXTURE_RUN_LABEL=YYYY-MM-DDThhmmssZ`. No-op reviews should add
`ANDROID_FIXTURE_RUN_NOTE="No upstream discriminator change"` and optionally set
`ANDROID_FIXTURE_RUN_RESULT_LABEL=noop` so the summary renders with the expected
suffix.

`make android-fixtures` wraps `scripts/android_fixture_regen.sh`, which writes a
raw log (`artifacts/android/fixture_runs/<label>-run.log`) and a Markdown
summary (`<label>-success.md`, `<label>-failure.md`, or the custom suffix when
`ANDROID_FIXTURE_RUN_RESULT_LABEL` is set). Leave `ANDROID_FIXTURE_CAPTURE_LOG=1`
and `ANDROID_FIXTURE_CAPTURE_SUMMARY=1` so evidence bundles always have both
files; override the flags only when running local diagnostics. If no changes
occur during the scheduled sync, emit a no-op summary in
`artifacts/android/fixture_runs/` so governance reviewers can confirm the
cadence was honoured.

`make android-fixtures-check` now routes through `ci/check_android_fixtures.sh`,
which emits a parity summary JSON under `artifacts/android/parity/<timestamp>/`
and copies it to `artifacts/android/parity/latest/summary.json` for dashboards
and release gates; set `ANDROID_PARITY_SUMMARY=<path>` to override the target
location or `ANDROID_PARITY_PIPELINE_METADATA=<file>` to embed pipeline/test
metadata in the summary when running under CI.

The test harness currently executes the key manager, keystore scaffolding (with
the Android Keystore stub backend), Norito codec round-trips that verify typed
instruction decoding, transaction builder signing, and HTTP client serialization
stubs to keep the Java pathways aligned.

### Publishing snapshots (AND9)

Run

```bash
make android-publish-snapshot \
  ANDROID_PUBLISH_VERSION=0.2.0-dev.1 \
  ANDROID_PUBLISH_REPO_DIR=$PWD/artifacts/android/maven
```

(or call `./gradlew -p java/iroha_android publish -PirohaAndroidVersion=... -PirohaAndroidRepoDir=...`)
to build/publish both SDK targets (JVM jar and Android AAR), generate the
CycloneDX SBOMs, and emit runtime evidence. The helper stages outputs under
`artifacts/android/reports/<version>/{android,jvm}/` with optional Sigstore
bundles (`ANDROID_PUBLISH_SIGN=1`) and assembles the sample app against the
published AAR unless `ANDROID_PUBLISH_SKIP_SAMPLE=1` is set. Override the
staging root via `ANDROID_PUBLISH_REPORT_DIR` and Maven destinations via
`ANDROID_PUBLISH_REPO_URL`/`ANDROID_PUBLISH_REPO_DIR`.

Key evidence paths (also mirrored under `artifacts/android/reports/<version>`):

- JVM deps: `jvm/build/reports/publishing/iroha-android-jvm-<version>-runtimeClasspath.json`
- JVM runtime manifest/checksum:
  `jvm/build/reports/publishing/iroha-android-jvm-runtime-manifest.json` and
  `jvm/build/reports/publishing/iroha-android-jvm-<version>-runtime.sha256`
- Android deps:
  `android/build/reports/publishing/iroha-android-android-<version>-releaseRuntimeClasspath.json`
- Android runtime manifest/checksum:
  `android/build/reports/publishing/iroha-android-android-runtime-manifest.json` and
  `android/build/reports/publishing/iroha-android-android-<version>-runtime.sha256`
- SBOMs: recorded under `build/reports/` (JSON and XML) with the chosen path
  surfaced via the `sbom.path` field in each runtime manifest.

Every publish invocation automatically runs the manifest/SBOM tasks so
governance packets can attach the resulting JSON alongside the Maven repository
snapshot. See `docs/source/sdk/android/publishing_plan.md` for the full release
checklist.

### Observability Hooks


> **Note:** Instruction lists hydrate strongly typed builders for register,
> transfer (asset/domain/asset-definition), mint/burn asset, and
> grant/revoke permission and role instructions
> and fall back to key/value payloads for families that do not yet have Java
> bindings.
> Additional instruction variants will be added alongside upcoming code
> generation work.

`ClientConfig` now exposes request-scoped instrumentation, static header support,
and deterministic retry policies. Applications can attach `ClientObserver`
implementations to capture metrics or send tracing data, register default headers
(for example, API tokens or `User-Agent` values), and configure `RetryPolicy`
instances to automatically retry transient network failures or 5xx responses with
predictable backoff.

### Crash telemetry

Enable crash telemetry by installing the built-in handler when configuring the
client:

```java
TelemetryOptions telemetryOptions = TelemetryOptions.builder()
    .setTelemetryRedaction(TelemetryOptions.Redaction.builder()
        .setSaltHex("<hex-salt>")
        .setSaltVersion("2026Q1")
        .setRotationId("rot-7")
        .build())
    .build();

ClientConfig clientConfig =
    ClientConfig.builder()
        .setBaseUri(new URI("https://torii.devnet.example"))
        .setTelemetryOptions(telemetryOptions)
        .setTelemetrySink(myTelemetrySink)
        .enableCrashTelemetryHandler()
        .build();
```

The handler records `android.crash.report.capture` automatically when uncaught
exceptions reach the process boundary. Upload pipelines can reuse the telemetry
configuration to emit `android.crash.report.upload` counters:

```java
clientConfig
    .crashTelemetryReporter()
    .ifPresent(reporter -> reporter.recordUpload(crashId, "sorafs", "success", retryCount));
```

Pass a custom `CrashTelemetryHandler.MetadataProvider` to `setCrashTelemetryMetadataProvider`
when additional crash context (e.g., watchdog buckets) is required.

### Torii streaming (SSE)

`HttpClientTransport.newEventStreamClient()` exposes the shared
`ToriiEventStreamClient`, which reuses the same base URI, telemetry observers, and
auth headers as the HTTP transport. Streaming clients consume Torii’s
server-sent event feeds and surface parsed frames via the listener interface:

```java
ToriiEventStreamClient streams = httpTransport.newEventStreamClient();
ToriiEventStream stream =
    streams.openSseStream(
        "/v1/pipeline/events",
        ToriiEventStreamOptions.builder()
            .putQueryParameter("selector", "blocks")
            .build(),
        new ToriiEventStreamListener() {
          @Override
          public void onEvent(ServerSentEvent event) {
            System.out.println(event.event() + ": " + event.data());
          }
        });

// Remember to close the stream when your component is torn down.
stream.close();
```

Listeners receive retry hints (via `retry:` frames) so applications can reuse
Torii’s back-off guidance, and telemetry observers attached to the transport
emit the same hashed-authority metadata recorded for HTTP submissions. When the
transport supports streaming responses (OkHttp/JDK/URLConnection), frames are
parsed as they arrive; other executors buffer the response before parsing.

Use `ToriiEventStreamSubscription` when a long-lived component needs automatic
reconnects:

```java
ToriiEventStreamSubscription subscription =
    ToriiEventStreamSubscription.builder(
            streams, "/v1/pipeline/events", ToriiEventStreamOptions.defaultOptions(), listener)
        .setInitialBackoff(Duration.ofSeconds(1))
        .setMaxBackoff(Duration.ofSeconds(30))
        .addObserver(new ToriiEventStreamObserver() {
          @Override
          public void onReconnectScheduled(Duration delay, ReconnectReason reason) {
            telemetry.incrementReconnects(reason.name(), delay);
          }

          @Override
          public void onStreamFailure(Throwable error) {
            telemetry.recordFailure(error);
          }
        })
        .build()
        .start();

// Later
subscription.close();
```

The helper honours server-provided retry hints and falls back to exponential
backoff when the stream fails before emitting one. Observers registered via
`addObserver` receive structured lifecycle notifications (`streamOpened`,
`streamClosed`, `streamFailure`, and `onReconnectScheduled`) so telemetry
pipelines can tag reconnect attempts, failure causes, and delay budgets without
mutating the primary listener.

### Torii streaming (WebSocket)

The WebSocket surface now rides on the transport abstractions
(`TransportRequest`/`TransportWebSocket`) so JVM and Android can inject platform
connectors. The default connector uses the JDK client; Android apps should pass
the OkHttp connector to keep `java.net.http` out of the AAR:

```java
import okhttp3.OkHttpClient;
import org.hyperledger.iroha.android.client.okhttp.OkHttpWebSocketConnector;

ToriiWebSocketClient wsClient =
    ToriiWebSocketClient.builder()
        .setBaseUri(URI.create("https://torii.devnet.example"))
        .setWebSocketConnector(new OkHttpWebSocketConnector(new OkHttpClient()))
        .build();

ToriiWebSocketSession session =
    wsClient.connect(
        "/ws/telemetry",
        ToriiWebSocketOptions.builder()
            .addSubprotocol("norito-stream")
            .build(),
        new ToriiWebSocketListener() {
          @Override
          public void onText(ToriiWebSocketSession session, CharSequence data, boolean last) {
            System.out.println("payload: " + data);
          }
        });
```

Sessions expose async send helpers (`sendText`, `sendBinary`, `sendPing`, `sendClose`)
and share the same observer instrumentation used by HTTP/SSE clients. Ping/pong
support depends on the connector: the JDK connector honours both, while the
OkHttp connector returns a failed future for ping/pong because OkHttp does not
expose control frames. Subprotocol negotiation is surfaced via
`ToriiWebSocketSession.subprotocol()`, and reconnect helpers remain available
through `ToriiWebSocketSubscription`.

### Connect retry policy

The Connect stack uses the dedicated `org.hyperledger.iroha.android.connect.ConnectRetryPolicy`
helper to mirror the Rust reference implementation (`connect_retry::policy`). It applies
exponential back-off with full jitter (base 5 s, capped at 60 s) and derives jitter deterministically
from the Connect session identifier so Android, Swift, and JavaScript clients wait for the exact
same delay sequence:

```java
import org.hyperledger.iroha.android.connect.ConnectRetryPolicy;

byte[] sessionId = new byte[32]; // use the Connect session id bytes
ConnectRetryPolicy policy = new ConnectRetryPolicy();
for (int attempt = 0; attempt < 5; attempt++) {
    long delayMs = policy.delayMillis(attempt, sessionId);
    Thread.sleep(delayMs);
    // reconnect logic here
}
```

The deterministic seed/attempt mapping ensures reconnect telemetry stays aligned across SDKs.

### Connect error taxonomy

`org.hyperledger.iroha.android.connect.error.ConnectError` mirrors the shared taxonomy
(`docs/source/connect_error_taxonomy.md`) so Android apps emit the same `category`/`code`
pairs as the Swift and JavaScript SDKs. Wrap every transport, codec, or queue failure via
`ConnectErrors.from(Throwable)` (or manually create a `ConnectError` using the builder)
before forwarding attributes to OpenTelemetry:

```java
import org.hyperledger.iroha.android.connect.error.ConnectError;
import org.hyperledger.iroha.android.connect.error.ConnectErrors;

try {
    connectClient.send(frame);
} catch (Exception ex) {
    ConnectError error = ConnectErrors.from(ex);
    telemetry.emit("connect.error", error.telemetryAttributes());
    throw error;
}
```

Queue back-pressure helpers such as `ConnectQueueError.overflow(limit)` and
`ConnectQueueError.expired(ttlMillis)` already implement `ConnectErrorConvertible`, so they
map to the `queueOverflow` and `timeout` categories automatically. Use
`ConnectError.telemetryAttributes(...)` to project overrides (fatal flag, HTTP status, or
custom `underlying` context) when surfacing the events.

### Canonical request signing

Torii app endpoints accept optional `X-Iroha-Account` / `X-Iroha-Signature`
headers. Use `CanonicalRequestSigner` when calling account-scoped helpers or
building ad-hoc HTTP requests:

```java
import java.net.URI;
import org.hyperledger.iroha.android.client.CanonicalRequestSigner;

URI uri = URI.create("https://torii.example/v1/accounts/alice@wonderland/assets?limit=10");
Map<String, String> headers =
    CanonicalRequestSigner.buildHeaders("get", uri, new byte[0], "alice@wonderland", keyPair.getPrivate());
```

Signatures cover the canonical method/path/query/body layout, matching the Rust
verifier Torii uses on app-facing endpoints.

### Pipeline Hashes

`HttpClientTransport.submitTransaction(...)` computes the canonical BLAKE2b-256
hash for every signed transaction via `SignedTransactionHasher` and surfaces it
through `ClientResponse.hashHex()`. Callers can forward the returned hash to
`waitForTransactionStatus(...)` (or other Torii polling helpers) without
reimplementing the hashing logic, and the same canonical value is preserved when
pending transactions are replayed from `PendingTransactionQueue`.

Torii returns a Norito-encoded transaction submission receipt (payload +
signature) on `/transaction` (alias `/v1/pipeline/transactions`). The Android SDK surfaces the raw
receipt bytes via `ClientResponse.body()` so callers can decode them with their
Norito tooling when they need the receipt fields.

### Torii Reject Codes

Torii attaches an `x-iroha-reject-code` header when admission fails. Both the
JDK and Android/OkHttp transports keep that header intact via
`ClientResponse.rejectCode()` so apps can surface the precise Torii error without
re-parsing the response body:

```java
ClientResponse response = transport.submitTransaction(transaction).join();
response.rejectCode().ifPresent(code -> {
  // e.g., PRTRY:TX_SIGNATURE_MISSING — surface to the user/telemetry
});
```

### Key Manager Defaults

`IrohaKeyManager.withDefaultProviders()` constructs a manager that prefers
hardware-backed keystore providers when available and falls back to the software
provider on emulators or desktop JVMs. Pass custom `KeyGenParameters` when you
need to enforce StrongBox-only keys or user-authentication requirements while
retaining a deterministic software fallback for local testing.
If your desktop JVM lacks built-in Ed25519 support, drop in BouncyCastle (the test harness ships a stub provider) so the software fallback can generate keys without the Android keystore.
Hardware-backed keys remain non-extractable; for user-scoped accounts that must
roam across devices, prefer `SOFTWARE_ONLY` (or `withSoftwareFallback`) and use
`exportDeterministicKey(...)` / `importDeterministicKey(...)` to move key
material between devices securely. When you need fully exportable keys, build
the software provider with BouncyCastle enforced and a persistent export store:

```java
KeyExportStore store = new FileKeyExportStore(new File(filesDir, "keys.properties"));
KeyPassphraseProvider passphraseProvider = () -> "export-passphrase".toCharArray();
SoftwareKeyProvider provider =
    new SoftwareKeyProvider(
        SoftwareKeyProvider.ProviderPolicy.BOUNCY_CASTLE_REQUIRED,
        store,
        passphraseProvider);
IrohaKeyManager manager = IrohaKeyManager.fromProviders(List.of(provider));
```

Or use the convenience helper:

```java
IrohaKeyManager manager =
    IrohaKeyManager.withExportableSoftwareKeys(store, passphraseProvider);
```

The manager validates Ed25519 SPKI output and skips providers that return a
different algorithm (common on emulators), falling back to the next configured
provider.

`generateOrLoad(alias, preference)` accepts a `KeySecurityPreference` that
describes the required hardware tier:

- `STRONGBOX_REQUIRED` — only StrongBox-backed providers are consulted; the call
  fails if no StrongBox backend is registered or the target device cannot
  satisfy the request.
- `STRONGBOX_PREFERRED` — StrongBox providers are tried first, then other
  hardware-backed providers, and finally software fallbacks.
- `HARDWARE_REQUIRED`/`HARDWARE_PREFERRED` — retain the previous semantics for
  “any hardware” while allowing deterministic fallback to software.
- `SOFTWARE_ONLY` — bypass hardware providers entirely (useful for emulator or
  deterministic testing scenarios).

Aliases follow a predictable lifecycle: the manager looks up existing keys
across all providers in priority order, reuses the first match, and only
generates a new key when no provider has material for the alias. Hardware-backed
aliases remain pinned to the provider that created them, while software
providers are consulted only when the chosen preference allows a downgrade (for
example, `STRONGBOX_PREFERRED` on a device without StrongBox support). When an
alias migrates from hardware to software fallback, the manager records the
software copy so future lookups remain deterministic.

Call `IrohaKeyManager.providerMetadata()` to inspect the registered providers
(name, hardware capability, attestation support) before deciding which
alias/security preference to use, and use the new
`IrohaKeyManager.verifyAttestation(...)` helper to validate the attestation
chain exported by hardware providers without wiring keystore internals through
application code.

`IrohaKeyManager.hasStrongBoxProvider()` is a convenience check that reports
whether a StrongBox-backed provider is registered, enabling applications to
surface hardware posture in diagnostics or gate user-facing flows before
attempting StrongBox-only operations.

```java
IrohaKeyManager manager = IrohaKeyManager.withDefaultProviders();
for (KeyProviderMetadata meta : manager.providerMetadata()) {
    System.out.printf("%s hardware=%s level=%s attestation=%s%n",
        meta.name(), meta.hardwareBacked(), meta.securityLevel(), meta.supportsAttestationCertificates());
}
```

Use `IrohaKeyManager.verifyAttestation(...)` (or the underlying
`KeystoreKeyProvider.verifyAttestation(...)`) alongside `AttestationVerifier`
when you need to validate the StrongBox/TEE attestation chain exported by the
Android backend. The verifier checks the certificate path, decoded challenge,
and security level while surfacing parsed metadata so applications can enforce
hardware policies or forward the attestation bundle to remote services. For
lab automation run `scripts/android_keystore_attestation.sh --bundle-dir <path>
--trust-root <root.pem> [--trust-root-dir <directory>]` — it compiles the same
verifier and produces a JSON summary that should be archived with each
attestation bundle.

Need fresh attestation material? Call
`IrohaKeyManager.generateAttestation(alias, challenge)` – it walks the
registered providers (StrongBox/TEE first) and returns a `KeyAttestation`
bundle when the hardware can satisfy the request, storing the artefact in the
backing provider for subsequent verification. Pass a non-empty `challenge` to
force fresh material (cache entries are keyed by `(alias, challenge)`), and set
`KeyGenParameters.Builder.setAttestationChallenge(...)` when generating keys if
you need the challenge embedded at creation time. StrongBox preferences are
propagated to keystore parameters (`STRONGBOX_REQUIRED` forces StrongBox,
`STRONGBOX_PREFERRED` attempts StrongBox then falls back to TEE), and hardware
fallbacks are surfaced via telemetry so apps can respond accordingly.

To exercise CUDA acceleration on capable devices, launch the JVM with
`-Diroha.cuda.enableNative=true` and ensure `libconnect_norito_bridge` is
available on `java.library.path`. Without the flag the Java fallback remains
active and no native library is loaded (avoiding security warnings in CI).

Kotlin callers should use `CudaAcceleratorsKotlin.*OrNull` helpers to receive
`Long?`/`LongArray?` outputs instead of `Optional` wrappers. See the CUDA
operator guide for native setup and the manual smoke harness
(`docs/source/sdk/android/gpu_operator_guide.md`), which exercises the JNI
bridge on CUDA-capable devices when `IROHA_CUDA_SELFTEST=1` is set.

`SoftwareKeyProvider.exportDeterministic(...)` emits a versioned, AES-GCM
wrapped export bundle (v3) using per-export salt/nonce. The bundle records
`kdf_kind` and work factor; v3 prefers Argon2id (64 MiB, 3 iterations,
parallelism 2) and falls back to PBKDF2-HMAC-SHA256 when Argon2 is unavailable.
A minimum 12 character passphrase is enforced for v3 exports/imports, and only
the v3 payload is accepted. Salt/nonce reuse is rejected and decode guards fail
fast on tampered lengths.
The companion `importDeterministic(...)` helper restores the key pair while
validating the export's public key and authentication tag, ensuring passphrase
mismatches or tampering are rejected.

`IrohaKeyManager.exportDeterministicKey(...)` / `importDeterministicKey(...)`
surface the same functionality through the manager so applications do not need
direct access to the underlying `SoftwareKeyProvider` when marshalling keys for
offline signing tools or recovery flows.

`TransactionBuilder.encodeAndSignEnvelope(...)` produces a
`OfflineSigningEnvelope` that wraps the signed payload, public key, and optional
metadata into a Norito bundle (`OfflineSigningEnvelopeCodec`) suitable for
offline storage or submission on another device. Supply
`OfflineEnvelopeOptions` to control the issued-at timestamp, attach contextual
metadata (for example, audit identifiers), and optionally embed a deterministic
`KeyExportBundle` so recovery tooling can rehydrate the software signing key
after passphrase verification. When hardware attestation is required, call
`encodeAndSignEnvelopeWithAttestation(...)` to receive both the envelope and a
`KeyAttestation` bundle in one shot.

### Offline Transaction Queue

Applications can provide a `PendingTransactionQueue` (the default implementation
`FilePendingTransactionQueue` persists base64-encoded
`OfflineSigningEnvelope` Norito blobs for forward compatibility) via
`ClientConfig`. When Torii submissions exhaust their retry budget, the
transport persists the signed payloads for later replay and automatically
drains the queue before sending new transactions. This keeps the mobile client
resilient to intermittent connectivity without losing deterministic ordering.
For OA2 environments that mandate authenticated WAL storage, call
`ClientConfig.Builder.enableOfflineJournalQueue(...)` (pass either an
`OfflineJournalKey`, raw seed bytes, or a passphrase `char[]`) to swap in
`OfflineJournalPendingTransactionQueue` backed by `OfflineJournal` — it stores
the same Norito envelopes but authenticates each write with BLAKE2b-256 +
HMAC-SHA256 so Android, Swift, and Rust wallets share identical replay logs.
Already have a `ClientConfig`? Call
`HttpClientTransport.withOfflineJournalQueue(config, path, passphraseOrSeed)` to
clone it with the journal queue automatically wired before constructing the
transport.
Set `ClientConfig.ExportOptions` when building the client to attach
deterministic key exports to queued transactions so offline replays can
rehydrate software providers without additional plumbing. `ExportOptions`
supports alias-specific passphrase providers, enabling selective exports when
only a subset of keys should be recoverable offline. Passphrase providers must
return mutable `char[]` instances; the transport zeroes the returned array after
export so secrets are not retained in memory.

### Norito RPC Helper

Use `NoritoRpcClient` when you need to call Torii's Norito RPC endpoints
(`application/x-norito` payloads) alongside the REST pipeline. The helper
wraps the platform HTTP executor (OkHttp on Android, the JDK HTTP client
elsewhere), applies the correct binary content headers, and lets callers
override HTTP method, timeouts, headers, query parameters, and `Accept`
negotiation via `NoritoRpcRequestOptions`. The tests under
`client/NoritoRpcClientTests` demonstrate POST/GET flows, header overrides,
and error propagation, while the client builder accepts default headers (for
example `Authorization`) so instrumentation matches the REST transport. Call
`ClientConfig.toNoritoRpcClient()` for the platform default (OkHttp on Android)
or `ClientConfig.toNoritoRpcClient(HttpTransportExecutor)` when you already
have a custom executor. `HttpClientTransport.newNoritoRpcClient()` reuses
existing client configuration and telemetry hooks when spinning up a Norito
RPC transport.

### SoraFS Gateway Helpers

The `org.hyperledger.iroha.android.sorafs` package provides thin builders that map
directly to the Rust `sorafs_orchestrator` configuration. Use
`GatewayProvider.builder()` to describe gateway endpoints,
`GatewayFetchOptions.builder()` to compose telemetry/retry/transport overrides, and
`GatewayFetchRequest.builder()` to bundle everything into the JSON structure the
orchestrator expects. `TransportPolicy` and `AnonymityPolicy` mirror the CLI/SDK
labels (`soranet-first`, `anon-guard-pq`, etc.), ensuring Android clients participate
in the staged SoraNet anonymity rollout alongside the other SDKs.
The builders validate SoraFS identifiers: `manifest_id_hex` and `provider_id_hex` must
be 32-byte hex strings (an optional `0x` prefix is accepted), and base64 inputs must
decode to non-empty bytes.

`SorafsGatewayClient` wraps the HTTP transport so applications can submit orchestrator
requests without reimplementing header/observer plumbing. Call
`client.fetch(request)` when you only need the raw JSON/string output, or
`client.fetchSummary(request)` to receive a typed `GatewayFetchSummary` that exposes
provider receipts, anonymity ratios, and chunk metadata. The client reuses
`HttpTransportExecutor`, which means tests can provide deterministic stubs and production
code can share the same connection pool as the Torii pipeline transport.
`HttpClientTransport.newSorafsGatewayClient(...)` is a convenience helper that spawns the
gateway client using the same executor, timeout, headers, and observers as the primary Torii
transport so applications can rely on a single HTTP stack.

### Mock Torii Harness

The test suite now includes a lightweight HTTP harness (`src/test/java/org/hyperledger/iroha/android/client/mock/ToriiMockServer.java`)
that mirrors Torii's `/transaction` submission and `/v1/pipeline/transactions/status` routes. Integration tests such as
`HttpClientTransportHarnessTests` spin up the server, interact with it via `HttpClientTransport`, and assert on the recorded
requests/responses, providing end-to-end coverage for retries, headers, and offline queue replays without depending on a real Torii node.

The fixture file under `src/test/resources` documents the expected JSON layout
for transaction payload parity tests (along with matching `.norito` binaries).
Regenerate the encoded blobs via the shared Rust exporter so Android stays in
sync with the canonical Norito toolchain:

```bash
make android-fixtures
make android-fixtures-check
# or run the exporter/check directly:
#   cargo run --manifest-path scripts/export_norito_fixtures/Cargo.toml --release
#   python3 scripts/check_android_fixtures.py
```

The command rewrites `transaction_payloads.json`, refreshes the companion
`.norito` files, and updates `transaction_fixtures.manifest.json` with new
hashes and provenance metadata (including the asset-metadata and
`SetParameter` parity vectors that exercise configuration and key/value
instructions). Always commit the regenerated JSON, `.norito` payloads, and
manifest together.

## Roadmap

- Harden the Android Keystore/StrongBox backend with device-matrix CI coverage,
  alias rotation tooling, and production configuration wrappers surfaced through
  `KeyGenParameters`.
- Integrate Rust parity fixtures for transaction Norito schemas and extend the
  data model beyond raw instruction blobs.
- Expose transaction signing helpers that wrap Iroha manifests and Norito encoders.
- Implement gRPC/HTTP networking clients with deterministic request handling.
- Publish Gradle artifacts and sample applications.

## License

Licensed under the Apache License, Version 2.0. See `LICENSE` for details.

## Offline allowances, transfers, and auditing

The SDK now exposes a lightweight `OfflineToriiClient` plus the model types
required to consume `/v1/offline/allowances` and `/v1/offline/transfers`. The
client reuses the existing `ClientConfig` headers/observers and can be created
from any `HttpClientTransport`:

```java
OfflineListParams params = OfflineListParams.builder()
    .limit(10L)
    .filter("{\"op\":\"eq\",\"args\":[\"controller_id\",\"merchant@wonderland\"]}")
    .build();

transport.offlineToriiClient().listAllowances(params)
    .thenAccept(list -> {
        for (OfflineAllowanceList.OfflineAllowanceItem item : list.items()) {
            System.out.println(item.certificateIdHex());
        }
    });

OfflineQueryEnvelope query = OfflineQueryEnvelope.builder()
    .filterJson("{\"op\":\"eq\",\"args\":[\"receiver_id\",\"merchant@wonderland\"]}")
    .setLimit(25L)
    .build();

transport.offlineToriiClient().queryTransfers(query)
    .thenAccept(list -> System.out.println("Fetched " + list.total() + " transfers"));
```

Register signed certificates on-ledger by posting them to the offline allowances endpoint:

```java
OfflineWalletCertificate certificate =
    new OfflineWalletCertificate(
        controllerId,
        allowanceCommitment,
        spendPublicKeyHex,
        attestationReportBytes,
        issuedAtMs,
        expiresAtMs,
        policy,
        operatorSignatureHex,
        metadata,
        verdictIdHex,
        attestationNonceHex,
        refreshAtMs);

transport.offlineToriiClient().registerAllowance(certificate, authorityId, authorityPrivateKeyHex);
```

If you want to issue and register in one call, use the top-up helpers:

```java
OfflineWalletCertificateDraft draft = /* build draft certificate */;

transport
    .offlineToriiClient()
    .topUpAllowance(draft, authorityId, authorityPrivateKeyHex)
    .thenAccept(topUp -> System.out.println(topUp.registration().certificateIdHex()));

transport
    .offlineToriiClient()
    .topUpAllowanceRenewal(existingCertificateIdHex, draft, authorityId, authorityPrivateKeyHex);
```

For jurisdictions that require offline spend logs, use the shared audit logger
and facade:

```java
OfflineWallet offlineWallet =
    transport.offlineWallet(context.getFilesDir().toPath().resolve("offline_audit.json"), true);

offlineWallet.recordAuditEntry(
    txId,
    senderAccountId,
    receiverAccountId,
    assetId,
    amountDecimalString);

byte[] auditJson = offlineWallet.exportAuditJson(); // forward to regulators when requested
byte[] verdictJournalJson =
    offlineWallet.exportVerdictJournalJson(); // include certificate countdown evidence

// Fetch transfers and automatically append each bundle to the audit log
offlineWallet.fetchTransfersWithAudit(params)
    .thenAccept(list -> {
        System.out.println("Fetched " + list.items().size() + " transfers");
    });
```

`OfflineTransferList.OfflineTransferItem.toJsonMap()` exposes a JSON-ready representation of the
transfer item; encode it with `JsonEncoder.encode(...)` when caching transfer lists locally. The
snapshot payload is included via `PlatformTokenSnapshot.toJsonMap()` when present.

The Android logger mirrors the Swift implementation: logging can be toggled at runtime, entries
use `{tx_id,sender_id,receiver_id,asset_id,amount,timestamp_ms}` fields, and
exports produce deterministic JSON arrays so compliance tooling can compare
SHA-256 or BLAKE3 digests across devices, and the verdict journal export emits the per-certificate
metadata (`refresh_at_ms`, policy countdowns, Safety Detect snapshots) required by OA5 to prove
when allowances were last refreshed. When long-lived audit history is required, use
`OfflineJournal` (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineJournal.java`)
to persist pending bundles with the shared `[kind|timestamp|len|tx_id|payload|chain|hmac]` layout used
by Rust and Swift. The helper exposes append/commit APIs, enforces the hash chain (`BLAKE2b-256` over
the previous chain and `tx_id`), authenticates each record with `HMAC-SHA256`, and exposes pending
entries for replay tooling so OA2’s WAL requirements are satisfied on Android as well.
