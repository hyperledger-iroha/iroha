package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.atomic.AtomicInteger;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference;
import org.hyperledger.iroha.android.client.queue.FilePendingTransactionQueue;
import org.hyperledger.iroha.android.crypto.SoftwareKeyProvider;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.nexus.AddressFormatOption;
import org.hyperledger.iroha.android.nexus.UaidBindingsQuery;
import org.hyperledger.iroha.android.nexus.UaidBindingsResponse;
import org.hyperledger.iroha.android.nexus.UaidManifestQuery;
import org.hyperledger.iroha.android.nexus.UaidManifestQuery.UaidManifestStatusFilter;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse.UaidManifestRecord;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse.UaidManifestStatus;
import org.hyperledger.iroha.android.nexus.UaidPortfolioQuery;
import org.hyperledger.iroha.android.nexus.UaidPortfolioResponse;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.TransactionBuilder;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;
import org.hyperledger.iroha.android.sorafs.AnonymityPolicy;
import org.hyperledger.iroha.android.sorafs.GatewayFetchOptions;
import org.hyperledger.iroha.android.sorafs.GatewayFetchRequest;
import org.hyperledger.iroha.android.sorafs.GatewayProvider;
import org.hyperledger.iroha.android.sorafs.TransportPolicy;
import org.hyperledger.iroha.android.sorafs.WriteModeHint;
import org.hyperledger.iroha.android.telemetry.DeviceProfile;
import org.hyperledger.iroha.android.telemetry.DeviceProfileProvider;
import org.hyperledger.iroha.android.telemetry.NetworkContext;
import org.hyperledger.iroha.android.telemetry.NetworkContextProvider;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;
import org.hyperledger.iroha.android.telemetry.TelemetryRecord;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

public final class HttpClientTransportTests {

  private HttpClientTransportTests() {}

  public static void main(final String[] args) throws Exception {
    submitBuildsToriiRequest();
    submitPropagatesExecutorFailure();
    submitSkipsRetryWhenNetworkRetriesDisabled();
    submitRetriesOnServerError();
    retryPolicyRecognizesRetryableStatus();
    submitQueuesTransactionsWhenOffline();
    submitQueuesTransactionsWithExportedKey();
    submitReplaysPendingTransactions();
    submitQueuesTransactionsSkipsExportWhenProviderDeclines();
    sorafsGatewayFetchUsesConfig();
    submitEmitsPendingQueueGauge();
    submitEmitsNetworkContextTelemetry();
    submitEmitsDeviceProfileTelemetry();
    submitEmitsRetryTelemetry();
    waitForTransactionStatusEmitsTelemetrySignals();
    pipelineStatusRedactionFailureUsesSignalId();
    uaidPortfolioRequestParsesResponse();
    uaidPortfolioRequestSupportsQuery();
    uaidRequestsRespectBasePath();
    uaidBindingsRequestParsesResponse();
    uaidManifestsRequestSupportsQuery();
    identifierPoliciesRequestParsesResponse();
    identifierResolveRequestParsesResponse();
    identifierResolveRequestAllowsNotFound();
    identifierClaimReceiptUsesAccountPath();
    identifierNormalizationCanonicalizesInputs();
    invalidateAndCancelDelegatesToExecutor();
    System.out.println("[IrohaAndroid] HTTP client transport tests passed.");
  }

  private static void submitBuildsToriiRequest() throws Exception {
    final CapturingExecutor executor = new CapturingExecutor();
    final RecordingObserver observer = new RecordingObserver();
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("http://127.0.0.1:8080"))
            .setRequestTimeout(Duration.ofSeconds(15))
            .putDefaultHeader("Authorization", "Bearer token")
            .addObserver(observer)
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    final SignedTransaction transaction = transactionWithPayload((byte) 0x01);

    final ClientResponse response = transport.submitTransaction(transaction).join();
    assert response.statusCode() == 202 : "Expected stub executor to return 202";
    assert "accepted".equals(response.message()) : "Executor message should propagate";
    final String expectedHash =
        SignedTransactionHasher.hashHex(transaction);
    final String actualHash = response.hashHex().orElse(null);
    assert actualHash != null : "ClientResponse must expose canonical hash";
    assert expectedHash.equals(actualHash)
        : "Canonical hash must match SignedTransactionHasher output";

    final TransportRequest request = executor.lastRequest;
    assert "POST".equals(request.method()) : "Submit must use POST";
    assert request.timeout() != null && request.timeout().equals(config.requestTimeout())
        : "Request timeout must match config";
    final List<String> contentTypes = request.headers().get("Content-Type");
    assert contentTypes != null && contentTypes.contains("application/x-norito")
        : "Content-Type header must be Norito";
    final List<String> acceptHeaders = request.headers().get("Accept");
    assert acceptHeaders != null
        && acceptHeaders.contains("application/x-norito, application/json")
        : "Accept header must include Norito";
    assert request.uri().toString().equals("http://127.0.0.1:8080/transaction")
        : "Submit endpoint must target Torii pipeline route";
    final List<String> authHeaders = request.headers().get("Authorization");
    assert authHeaders != null && authHeaders.contains("Bearer token")
        : "Custom headers from config must be applied";

    final byte[] body = request.body();
    final byte[] expected = SignedTransactionEncoder.encodeVersioned(transaction);
    assert java.util.Arrays.equals(body, expected)
        : "Body must include Norito-encoded signed transaction";

    assert observer.requestCount.get() == 1 : "Observer must see request";
    assert observer.responseCount.get() == 1 : "Observer must see response";
    assert observer.failureCount.get() == 0 : "Observer must not see failure";
  }

  private static void submitPropagatesExecutorFailure() {
    final RuntimeException transportError = new RuntimeException("network down");
    final FailingExecutor executor = new FailingExecutor(transportError);
    final RecordingObserver observer = new RecordingObserver();
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("http://localhost:8080"))
                .addObserver(observer)
                .build());

    final SignedTransaction transaction = transactionWithPayload((byte) 0x02);

    boolean threw = false;
    try {
      transport.submitTransaction(transaction).join();
    } catch (final RuntimeException ex) {
      threw = true;
      final Throwable cause = ex.getCause();
      assert ex == transportError || cause == transportError
          : "Original runtime exception must propagate";
    }
    assert threw : "Expected submit to rethrow executor error";
    assert observer.requestCount.get() == 1 : "Observer must see request";
    assert observer.responseCount.get() == 0 : "No response should be recorded";
    assert observer.failureCount.get() == 1 : "Observer must see failure";
  }

  private static void submitSkipsRetryWhenNetworkRetriesDisabled() {
    final CountingFailingExecutor executor =
        new CountingFailingExecutor(new RuntimeException("network down"));
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("http://localhost:8080"))
            .setRetryPolicy(
                RetryPolicy.builder()
                    .setMaxAttempts(3)
                    .setBaseDelay(Duration.ZERO)
                    .setRetryOnNetworkError(false)
                    .build())
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    final SignedTransaction transaction = transactionWithPayload((byte) 0x03);

    boolean threw = false;
    try {
      transport.submitTransaction(transaction).join();
    } catch (final RuntimeException ex) {
      threw = true;
    }
    assert threw : "Submission should fail when network retries are disabled";
    assert executor.callCount == 1 : "Transport must not retry on network failures when disabled";
  }

  private static void submitRetriesOnServerError() {
    final SequencedExecutor executor = new SequencedExecutor();
    final RecordingObserver observer = new RecordingObserver();
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("http://localhost:8080"))
            .addObserver(observer)
            .setRetryPolicy(
                RetryPolicy.builder()
                    .setMaxAttempts(2)
                    .setBaseDelay(Duration.ZERO)
                    .build())
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    final SignedTransaction transaction = transactionWithPayload((byte) 0x04);

    final ClientResponse response = transport.submitTransaction(transaction).join();
    assert response.statusCode() == 202 : "Final attempt should succeed";
    assert executor.callCount == 2 : "Transport should retry once";
    assert observer.requestCount.get() == 2 : "Observer should see both attempts";
    assert observer.responseCount.get() == 1 : "Only the successful attempt reports a response";
    assert observer.failureCount.get() == 0 : "Retries on responses should not trigger failure callback";
    final String expectedHash = SignedTransactionHasher.hashHex(transaction);
    assert expectedHash.equals(response.hashHex().orElse(null))
        : "Canonical hash must match SignedTransactionHasher output after retries";
  }

  private static void retryPolicyRecognizesRetryableStatus() {
    final RetryPolicy defaultPolicy = RetryPolicy.builder().setMaxAttempts(1).build();
    assert defaultPolicy.isRetryableStatus(503) : "Server errors should be retryable by default";
    assert defaultPolicy.isRetryableStatus(429) : "Too many requests should be retryable by default";
    assert !defaultPolicy.isRetryableStatus(400) : "Client errors should not be retryable";

    final RetryPolicy custom =
        RetryPolicy.builder()
            .setMaxAttempts(1)
            .setRetryOnServerError(false)
            .setRetryOnTooManyRequests(false)
            .addRetryStatusCode(418)
            .build();
    assert !custom.isRetryableStatus(503) : "Server errors must be disabled by policy";
    assert !custom.isRetryableStatus(429) : "429 must be disabled by policy";
    assert custom.isRetryableStatus(418) : "Custom retry codes must be honored";
  }

  private static void submitQueuesTransactionsWhenOffline() throws Exception {
    final Path tempDir = Files.createTempDirectory("iroha-queue-offline-");
    final FilePendingTransactionQueue queue =
        new FilePendingTransactionQueue(tempDir.resolve("pending.queue"));
    final SignedTransaction transaction = transactionWithPayload((byte) 0x11);

    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new FailingExecutor(new RuntimeException("offline")),
            ClientConfig.builder()
                .setBaseUri(URI.create("http://localhost:8080"))
                .setRetryPolicy(RetryPolicy.builder().setMaxAttempts(1).build())
                .setPendingQueue(queue)
                .build());

    boolean threw = false;
    try {
      transport.submitTransaction(transaction).join();
    } catch (final RuntimeException ex) {
      threw = true;
    }
    assert threw : "Submission should fail when executor errors";
    assert queue.size() == 1 : "Transaction must be queued";
    final List<SignedTransaction> persisted = queue.drain();
    assert persisted.size() == 1 : "Drain must return queued transaction";
    assert payloadEquals(transaction, persisted.get(0)) : "Queued payload must match original";
  }

  private static void submitReplaysPendingTransactions() throws Exception {
    final Path tempDir = Files.createTempDirectory("iroha-queue-replay-");
    final FilePendingTransactionQueue queue =
        new FilePendingTransactionQueue(tempDir.resolve("pending.queue"));
    final SignedTransaction first = transactionWithPayload((byte) 0x21);
    final SignedTransaction second = transactionWithPayload((byte) 0x22);
    queue.enqueue(first);
    queue.enqueue(second);

    final RecordingExecutor executor = new RecordingExecutor();
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("http://localhost:8080"))
                .setRetryPolicy(RetryPolicy.builder().setMaxAttempts(1).build())
                .setPendingQueue(queue)
                .build());

    final SignedTransaction live = transactionWithPayload((byte) 0x23);
    transport.submitTransaction(live).join();
    assert queue.size() == 0 : "Pending queue must be empty after replay";
    final List<byte[]> payloadOrder = executor.payloads;
    assert payloadOrder.size() == 3 : "Executor should receive queued transactions plus live submission";
    assert java.util.Arrays.equals(
            payloadOrder.get(0), SignedTransactionEncoder.encodeVersioned(first))
        : "First queued transaction must be sent first";
    assert java.util.Arrays.equals(
            payloadOrder.get(1), SignedTransactionEncoder.encodeVersioned(second))
        : "Second queued transaction must be sent second";
    assert java.util.Arrays.equals(
            payloadOrder.get(2), SignedTransactionEncoder.encodeVersioned(live))
        : "Live transaction must be sent last";
  }

  private static void submitEmitsPendingQueueGauge() throws Exception {
    final Path tempDir = Files.createTempDirectory("iroha-queue-telemetry-");
    final FilePendingTransactionQueue queue =
        new FilePendingTransactionQueue(tempDir.resolve("pending.queue"));
    final RecordingTelemetrySink telemetrySink = new RecordingTelemetrySink();
    final TelemetryOptions telemetryOptions =
        TelemetryOptions.builder()
            .setTelemetryRedaction(
                TelemetryOptions.Redaction.builder()
                    .setSaltHex("01020304")
                    .setSaltVersion("2026Q1")
                    .build())
            .build();

    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new FailingExecutor(new RuntimeException("offline")),
            ClientConfig.builder()
                .setBaseUri(URI.create("http://localhost:8080"))
                .setRetryPolicy(RetryPolicy.none())
                .setPendingQueue(queue)
                .setTelemetryOptions(telemetryOptions)
                .setTelemetrySink(telemetrySink)
                .build());

    final SignedTransaction transaction = transactionWithPayload((byte) 0x33);

    boolean threw = false;
    try {
      transport.submitTransaction(transaction).join();
    } catch (final RuntimeException ex) {
      threw = true;
    }
    assert threw : "Submission should fail when executor errors";

    final RecordingTelemetrySink.GaugeEvent event =
        telemetrySink.lastEvent("android.pending_queue.depth");
    assert event != null : "Telemetry sink should capture queue depth emission";
    assert "android.pending_queue.depth".equals(event.signalId())
        : "Gauge must use the pending queue signal id";
    assert "file".equals(event.fields().get("queue"))
        : "Queue label should describe the implementation";
    assert Long.valueOf(1L).equals(event.fields().get("depth"))
        : "Queue depth gauge must report pending entry count";
  }

  private static void submitEmitsNetworkContextTelemetry() throws Exception {
    final RecordingTelemetrySink telemetrySink = new RecordingTelemetrySink();
    final TelemetryOptions telemetryOptions =
        TelemetryOptions.builder()
            .setTelemetryRedaction(
                TelemetryOptions.Redaction.builder()
                    .setSaltHex("0a0b0c0d")
                    .setSaltVersion("2026Q1")
                    .build())
            .build();
    final NetworkContextProvider provider =
        () -> Optional.of(NetworkContext.of("wifi", false));

    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new CapturingExecutor(),
            ClientConfig.builder()
                .setBaseUri(URI.create("http://localhost:8080"))
                .setTelemetryOptions(telemetryOptions)
                .setTelemetrySink(telemetrySink)
                .setNetworkContextProvider(provider)
                .build());

    final SignedTransaction transaction = transactionWithPayload((byte) 0x44);
    final ClientResponse response = transport.submitTransaction(transaction).join();
    assert response.statusCode() == 202 : "Executor should accept submission";

    final RecordingTelemetrySink.GaugeEvent event =
        telemetrySink.lastEvent("android.telemetry.network_context");
    assert event != null : "Telemetry sink should capture network context emission";
    assert "android.telemetry.network_context".equals(event.signalId())
        : "Signal id must match android.telemetry.network_context";
    assert "wifi".equals(event.fields().get("network_type"))
        : "Network type should reflect provider snapshot";
    assert Boolean.FALSE.equals(event.fields().get("roaming"))
        : "Roaming flag should be forwarded as-is";
  }

  private static void submitEmitsDeviceProfileTelemetry() throws Exception {
    final RecordingTelemetrySink telemetrySink = new RecordingTelemetrySink();
    final TelemetryOptions telemetryOptions =
        TelemetryOptions.builder()
            .setTelemetryRedaction(
                TelemetryOptions.Redaction.builder()
                    .setSaltHex("0d0c0b0a")
                    .setSaltVersion("2026Q1")
                    .build())
            .build();
    final DeviceProfileProvider provider = () -> Optional.of(DeviceProfile.of("enterprise"));

    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new CapturingExecutor(),
            ClientConfig.builder()
                .setBaseUri(URI.create("http://localhost:8080"))
                .setTelemetryOptions(telemetryOptions)
                .setTelemetrySink(telemetrySink)
                .setDeviceProfileProvider(provider)
                .build());

    final SignedTransaction transaction = transactionWithPayload((byte) 0x66);
    final ClientResponse response = transport.submitTransaction(transaction).join();
    assert response.statusCode() == 202 : "Executor should accept submission";

    final RecordingTelemetrySink.GaugeEvent event =
        telemetrySink.lastEvent("android.telemetry.device_profile");
    assert event != null : "Telemetry sink should capture device profile emission";
    assert "enterprise".equals(event.fields().get("profile_bucket"))
        : "Profile bucket should match provider snapshot";
  }

  private static void submitEmitsRetryTelemetry() throws Exception {
    final RecordingTelemetrySink telemetrySink = new RecordingTelemetrySink();
    final TelemetryOptions telemetryOptions =
        TelemetryOptions.builder()
            .setTelemetryRedaction(
                TelemetryOptions.Redaction.builder()
                    .setSaltHex("0f0e0d0c")
                    .setSaltVersion("2026Q1")
                    .build())
            .build();
    final RetryPolicy retryPolicy =
        RetryPolicy.builder().setMaxAttempts(2).setBaseDelay(Duration.ofMillis(250)).build();

    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new SequencedExecutor(),
            ClientConfig.builder()
                .setBaseUri(URI.create("http://retry.test:8080"))
                .setRetryPolicy(retryPolicy)
                .setTelemetryOptions(telemetryOptions)
                .setTelemetrySink(telemetrySink)
                .build());

    final SignedTransaction transaction = transactionWithPayload((byte) 0x55);
    final ClientResponse response = transport.submitTransaction(transaction).join();
    assert response.statusCode() == 202 : "Submission should succeed after retry";

    final RecordingTelemetrySink.GaugeEvent event =
        telemetrySink.lastEvent("android.torii.http.retry");
    assert event != null : "Retry telemetry signal must be emitted";
    final Map<String, Object> fields = event.fields();
    final String expectedHash =
        telemetryOptions
            .redaction()
            .hashAuthority("retry.test:8080")
            .orElseThrow(() -> new IllegalStateException("Hash must be present"));
    assert expectedHash.equals(fields.get("authority_hash"))
        : "Retry signal should carry hashed authority";
    assert "/transaction".equals(fields.get("route"))
        : "Route must describe the Torii submit endpoint";
    assert Integer.valueOf(1).equals(fields.get("retry_count"))
        : "First retry should report attempt #1";
    assert "503".equals(fields.get("error_code"))
        : "Error code must reflect the HTTP status";
    assert Long.valueOf(250L).equals(fields.get("backoff_ms"))
        : "Backoff must follow the retry policy";
  }

  private static void waitForTransactionStatusEmitsTelemetrySignals() {
    final RecordingTelemetrySink telemetrySink = new RecordingTelemetrySink();
    final TelemetryOptions telemetryOptions =
        TelemetryOptions.builder()
            .setTelemetryRedaction(
                TelemetryOptions.Redaction.builder()
                    .setSaltHex("0a0b0c0d")
                    .setSaltVersion("2026Q2")
                    .build())
            .build();
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new ScriptedExecutor(
                new TransportResponse(202, statusPayload("Pending"), "", Map.of()),
                new TransportResponse(200, statusPayload("Committed"), "", Map.of())),
            ClientConfig.builder()
                .setBaseUri(URI.create("http://status-telemetry.test:8080"))
                .setTelemetryOptions(telemetryOptions)
                .setTelemetrySink(telemetrySink)
                .build());

    final String hashHex = "deadbeefcafefeed";
    final Map<String, Object> payload =
        transport
            .waitForTransactionStatus(
                hashHex, PipelineStatusOptions.builder().intervalMillis(0L).build())
            .join();
    assert "Committed".equals(PipelineStatusExtractor.extractStatusKind(payload).orElse(null))
        : "Expected committed status";

    final List<Map<String, Object>> signals =
        telemetrySink.eventsBySignal("android.torii.pipeline.status");
    assert signals.size() == 2 : "Expected pending and success telemetry events";
    final Map<String, Object> pending = signals.get(0);
    final Map<String, Object> success = signals.get(1);
    final String expectedAuthorityHash =
        telemetryOptions
            .redaction()
            .hashAuthority("status-telemetry.test:8080")
            .orElseThrow(() -> new IllegalStateException("authority hash missing"));

    assert expectedAuthorityHash.equals(pending.get("authority_hash"))
        : "Pending signal must carry hashed authority";
    assert hashHex.equals(pending.get("tx_hash"))
        : "Pending signal must carry transaction hash";
    assert "Pending".equals(pending.get("status_kind"))
        : "Pending signal must record status kind";
    assert "pending".equals(pending.get("outcome"))
        : "Pending signal must use pending outcome";
    assert ((Number) pending.get("attempts")).intValue() == 1
        : "Pending signal must record first attempt";

    assert expectedAuthorityHash.equals(success.get("authority_hash"))
        : "Success signal must carry hashed authority";
    assert hashHex.equals(success.get("tx_hash"))
        : "Success signal must carry transaction hash";
    assert "Committed".equals(success.get("status_kind"))
        : "Success signal must record committed status";
    assert "success".equals(success.get("outcome"))
        : "Success signal must use success outcome";
    assert ((Number) success.get("attempts")).intValue() == 2
        : "Success signal must reflect attempt count";
  }

  private static void pipelineStatusRedactionFailureUsesSignalId() {
    final RecordingTelemetrySink telemetrySink = new RecordingTelemetrySink();
    final TelemetryOptions telemetryOptions =
        TelemetryOptions.builder()
            .setTelemetryRedaction(
                TelemetryOptions.Redaction.builder()
                    .setSaltHex("0e0f1011")
                    .setSaltVersion("2026Q3")
                    .build())
            .build();
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new ScriptedExecutor(
                new TransportResponse(200, statusPayload("Committed"), "", Map.of())),
            ClientConfig.builder()
                .setBaseUri(URI.create("http:/")) // No authority -> redaction failure path.
                .setTelemetryOptions(telemetryOptions)
                .setTelemetrySink(telemetrySink)
                .build());

    transport
        .waitForTransactionStatus(
            "beadfeed", PipelineStatusOptions.builder().intervalMillis(0L).build())
        .join();

    final List<Map<String, Object>> failures =
        telemetrySink.eventsBySignal("android.telemetry.redaction.failure");
    boolean found = false;
    for (final Map<String, Object> fields : failures) {
      if ("android.torii.pipeline.status".equals(fields.get("signal_id"))) {
        assert "blank_authority".equals(fields.get("reason"))
            : "Redaction failure must report the blank authority reason";
        found = true;
        break;
      }
    }
    assert found : "Pipeline status redaction failures must reference the pipeline status signal";
  }

  private static void submitQueuesTransactionsWithExportedKey() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final IrohaKeyManager keyManager = IrohaKeyManager.fromProviders(List.of(provider));
    final TransactionBuilder builder = new TransactionBuilder(new NoritoJavaCodecAdapter(), keyManager);
    final TransactionPayload payload = TransactionPayload.builder().build();
    final SignedTransaction transaction =
        builder.encodeAndSign(payload, "queued-alias", KeySecurityPreference.SOFTWARE_ONLY);

    final char[] passphrase = "queue-passphrase".toCharArray();
    final Path tempDir = Files.createTempDirectory("iroha-queue-export-");
    final FilePendingTransactionQueue queue =
        new FilePendingTransactionQueue(tempDir.resolve("pending.queue"));

    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("http://localhost:8080"))
            .setRetryPolicy(RetryPolicy.builder().setMaxAttempts(1).build())
            .setPendingQueue(queue)
            .setExportOptions(
                ClientConfig.ExportOptions.builder()
                    .setKeyManager(keyManager)
                    .setPassphrase(passphrase)
                    .build())
            .build();

    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new FailingExecutor(new RuntimeException("offline")), config);

    boolean threw = false;
    try {
      transport.submitTransaction(transaction).join();
    } catch (final RuntimeException ex) {
      threw = true;
    }
    assert threw : "Submission should fail when executor errors";

    final List<SignedTransaction> drained = queue.drain();
    assert drained.size() == 1 : "Queued transaction expected";
    final SignedTransaction queued = drained.get(0);
    assert queued.keyAlias().orElse("?").equals("queued-alias") : "Alias must be preserved";
    assert queued.exportedKeyBundle().isPresent() : "Exported key bundle must be attached";
    java.util.Arrays.fill(passphrase, '\0');
  }

  private static void submitQueuesTransactionsSkipsExportWhenProviderDeclines() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final IrohaKeyManager keyManager = IrohaKeyManager.fromProviders(List.of(provider));
    final TransactionBuilder builder = new TransactionBuilder(new NoritoJavaCodecAdapter(), keyManager);
    final TransactionPayload payload = TransactionPayload.builder().build();
    final SignedTransaction transaction =
        builder.encodeAndSign(payload, "skip-alias", KeySecurityPreference.SOFTWARE_ONLY);

    final Path tempDir = Files.createTempDirectory("iroha-queue-skip-export-");
    final FilePendingTransactionQueue queue =
        new FilePendingTransactionQueue(tempDir.resolve("pending.queue"));

    final char[] passphrase = "skip-passphrase".toCharArray();
    final ClientConfig.ExportOptions exportOptions =
        ClientConfig.ExportOptions.builder()
            .setKeyManager(keyManager)
            .setPassphraseProvider(
                alias -> "skip-alias".equals(alias) ? null : passphrase.clone())
            .build();

    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new FailingExecutor(new RuntimeException("offline")),
            ClientConfig.builder()
                .setBaseUri(URI.create("http://localhost:8080"))
                .setRetryPolicy(RetryPolicy.builder().setMaxAttempts(1).build())
                .setPendingQueue(queue)
                .setExportOptions(exportOptions)
                .build());

    boolean threw = false;
    try {
      transport.submitTransaction(transaction).join();
    } catch (final RuntimeException ex) {
      threw = true;
    }
    assert threw : "Submission should fail when executor errors";

    final List<SignedTransaction> drained = queue.drain();
    assert drained.size() == 1 : "Queued transaction expected";
    final SignedTransaction queued = drained.get(0);
    assert queued.exportedKeyBundle().isEmpty() : "Export bundle should be omitted when provider declines";
    java.util.Arrays.fill(passphrase, '\0');
  }

  private static void sorafsGatewayFetchUsesConfig() throws Exception {
    final CapturingExecutor executor = new CapturingExecutor();
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("http://torii.example:8080"))
            .setSorafsGatewayUri(URI.create("https://gateway.example:8443/gateway"))
            .setRequestTimeout(Duration.ofSeconds(12))
            .putDefaultHeader("X-Trace", "android-client")
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    final GatewayProvider provider =
        GatewayProvider.builder()
            .setName("primary")
            .setProviderIdHex("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff")
            .setBaseUrl("https://storage.example/direct")
            .setStreamTokenBase64("dG9rZW4=")
            .build();

    final GatewayFetchRequest request =
        GatewayFetchRequest.builder()
            .setManifestIdHex("deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef")
            .setChunkerHandle("sorafs.sf1@1.0.0")
            .setOptions(
                GatewayFetchOptions.builder()
                    .setTelemetryRegion("us-east-1")
                    .setTransportPolicy(TransportPolicy.DIRECT_ONLY)
                    .setAnonymityPolicy(AnonymityPolicy.ANON_STRICT_PQ)
                    .setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)
                    .build())
            .addProvider(provider)
            .build();

    final ClientResponse response = transport.sorafsGatewayFetch(request).join();
    assert response.statusCode() == 202 : "Gateway fetch should surface executor status";

    final TransportRequest requestSent = executor.lastRequest;
    assert requestSent != null : "Executor should capture request";
    assert requestSent.uri().toString().equals(
            "https://gateway.example:8443/gateway/v1/sorafs/gateway/fetch")
        : "Gateway URI must combine base path with fetch endpoint";
    assert requestSent.headers().getOrDefault("Content-Type", List.of()).contains("application/json")
        : "Gateway fetch must set JSON content type";
    assert requestSent.headers().getOrDefault("X-Trace", List.of()).contains("android-client")
        : "Custom headers should propagate";

    final String body = readBody(requestSent);
    assert body.equals(request.toJsonString()) : "Request body must serialise fetch payload";
  }

  private static void uaidPortfolioRequestParsesResponse() {
    final String hex =
        "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    final String json =
        ("{"
            + "\"uaid\":\"uaid:"
            + hex
            + "\","
            + "\"totals\":{\"accounts\":2,\"positions\":3},"
            + "\"dataspaces\":[{"
            + "\"dataspace_id\":42,"
            + "\"dataspace_alias\":\"sandbox\","
            + "\"accounts\":[{"
            + "\"account_id\":\"alice@wonderland\","
            + "\"label\":\"Primary\","
            + "\"assets\":[{"
            + "\"asset_id\":\"xor#wonderland\",\"asset_definition_id\":\"xor#nexus\",\"quantity\":\"42\""
            + "}]"
            + "}]"
            + "}]"
            + "}");
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://torii.example"))
            .putDefaultHeader("X-Test", "uaid")
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    final UaidPortfolioResponse response =
        transport.getUaidPortfolio("  UAID:" + hex.toUpperCase() + " ").join();
    assert response.uaid().equals("uaid:" + hex)
        : "UAID literal must be normalised";
    assert response.totals().accounts() == 2 : "Accounts total should parse";
    assert response.totals().positions() == 3 : "Positions total should parse";
    assert response.dataspaces().size() == 1 : "Expected one dataspace entry";
    final UaidPortfolioResponse.UaidPortfolioDataspace dataspace = response.dataspaces().get(0);
    assert dataspace.dataspaceId() == 42 : "Dataspace ID mismatch";
    assert "sandbox".equals(dataspace.dataspaceAlias())
        : "Dataspace alias mismatch";
    assert dataspace.accounts().size() == 1 : "Expected single account entry";
    final UaidPortfolioResponse.UaidPortfolioAccount account =
        dataspace.accounts().get(0);
    assert "alice@wonderland".equals(account.accountId())
        : "Account ID mismatch";
    assert "Primary".equals(account.label()) : "Account label mismatch";
    assert account.assets().size() == 1 : "Expected single asset entry";
    final UaidPortfolioResponse.UaidPortfolioAsset asset = account.assets().get(0);
    assert "xor#wonderland".equals(asset.assetId()) : "Asset ID mismatch";
    assert "xor#nexus".equals(asset.assetDefinitionId()) : "Asset definition mismatch";
    assert "42".equals(asset.quantity()) : "Asset quantity mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "UAID request must be captured";
    assert "GET".equals(request.method()) : "UAID portfolio must use GET";
    assert request.headers().getOrDefault("Accept", List.of()).contains("application/json")
        : "Accept header must request JSON";
    assert request.headers().getOrDefault("X-Test", List.of()).contains("uaid")
        : "Custom headers must propagate";
    assert request.uri()
        .toString()
        .equals("https://torii.example/v1/accounts/uaid%3A" + hex + "/portfolio")
        : "Request URI must percent-encode UAID literal";
  }

  private static void uaidPortfolioRequestSupportsQuery() {
    final String hex =
        "f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff0102030405060708090a0b0c0d0e0f11";
    final String json =
        "{"
            + "\"uaid\":\"uaid:"
            + hex
            + "\","
            + "\"totals\":{\"accounts\":0,\"positions\":0},"
            + "\"dataspaces\":[]"
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://torii.example"))
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    final UaidPortfolioQuery query =
        UaidPortfolioQuery.builder().setAssetId("xor#wonderland").build();
    transport.getUaidPortfolio("uaid:" + hex.toUpperCase(), query).join();

    final TransportRequest request = executor.lastRequest();
    assert request != null : "UAID request must be captured";
    assert request
        .uri()
        .toString()
        .equals(
            "https://torii.example/v1/accounts/uaid%3A"
                + hex
                + "/portfolio?asset_id=xor%23wonderland")
        : "UAID portfolio query must include asset_id filter";
  }

  private static void uaidRequestsRespectBasePath() {
    final String hex =
        "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    final String json = "{\"uaid\":\"uaid:" + hex + "\"}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://torii.example/api"))
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    transport.getUaidPortfolio("uaid:" + hex).join();

    final TransportRequest request = executor.lastRequest();
    assert request != null : "UAID request should be captured";
    assert request
        .uri()
        .toString()
        .equals("https://torii.example/api/v1/accounts/uaid%3A" + hex + "/portfolio")
        : "UAID endpoints must preserve baseUri path segments";
  }

  private static void uaidBindingsRequestParsesResponse() {
    final String hex =
        "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    final String json =
        "{"
            + "\"uaid\":\"uaid:"
            + hex
            + "\","
            + "\"dataspaces\":[{"
            + "\"dataspace_id\":7,"
            + "\"dataspace_alias\":null,"
            + "\"accounts\":[\"alice@wonderland\",\"bob@sora\"]"
            + "}]"
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final ClientConfig config =
        ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    final UaidBindingsQuery query =
        UaidBindingsQuery.builder().setAddressFormat(AddressFormatOption.COMPRESSED).build();
    final UaidBindingsResponse response =
        transport.getUaidBindings("uaid:" + hex.toUpperCase(), query).join();
    assert response.dataspaces().size() == 1 : "Expected bindings entry";
    final UaidBindingsResponse.UaidBindingsDataspace dataspace = response.dataspaces().get(0);
    assert dataspace.dataspaceId() == 7 : "Dataspace ID mismatch";
    assert dataspace.accounts().size() == 2 : "Account bindings mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request.uri()
        .toString()
        .equals(
            "https://torii.example/v1/space-directory/uaids/uaid%3A"
                + hex
                + "?address_format=compressed")
        : "Bindings URI must encode UAID literal and query";
  }

  private static void uaidManifestsRequestSupportsQuery() {
    final String hex =
        "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    final String json =
        "{"
            + "\"uaid\":\"uaid:"
            + hex
            + "\","
            + "\"total\":1,"
            + "\"manifests\":[{"
            + "\"dataspace_id\":9,"
            + "\"dataspace_alias\":\"pilot\","
            + "\"manifest_hash\":\"deadbeef\","
            + "\"status\":\"Revoked\","
            + "\"lifecycle\":{"
            + "\"activated_epoch\":10,"
            + "\"expired_epoch\":null,"
            + "\"revocation\":{\"epoch\":15,\"reason\":\"policy\"}"
            + "},"
            + "\"accounts\":[\"alice@wonderland\"],"
            + "\"manifest\":{"
            + "\"version\":\"1\","
            + "\"uaid\":\"uaid:"
            + hex
            + "\","
            + "\"dataspace\":9,"
            + "\"entries\":[]"
            + "}"
            + "}]"
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final ClientConfig config =
        ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);
    final UaidManifestQuery query =
        UaidManifestQuery.builder()
            .setDataspaceId(9L)
            .setStatus(UaidManifestStatusFilter.INACTIVE)
            .setLimit(25L)
            .setOffset(5L)
            .setAddressFormat(AddressFormatOption.IH58)
            .build();

    final UaidManifestsResponse response =
        transport.getUaidManifests("uaid:" + hex, query).join();
    assert response.total() == 1 : "Total manifests must parse";
    assert response.manifests().size() == 1 : "Expected manifest record";
    final UaidManifestRecord record = response.manifests().get(0);
    assert record.dataspaceId() == 9 : "Dataspace ID mismatch";
    assert "pilot".equals(record.dataspaceAlias()) : "Dataspace alias mismatch";
    assert "deadbeef".equals(record.manifestHash()) : "Manifest hash mismatch";
    assert record.status() == UaidManifestStatus.REVOKED : "Status parsing mismatch";
    assert record.lifecycle().activatedEpoch() == 10L : "Activated epoch mismatch";
    assert record.lifecycle().expiredEpoch() == null : "Expired epoch should be null";
    assert record.lifecycle().revocation() != null : "Revocation should be present";
    assert record.lifecycle().revocation().epoch() == 15L : "Revocation epoch mismatch";
    assert "policy".equals(record.lifecycle().revocation().reason()) : "Revocation reason mismatch";
    assert record.accounts().contains("alice@wonderland") : "Accounts must surface";
    assert record.manifestJson().contains("\"version\":\"1\"") : "Manifest JSON should be stored";
    final Map<String, Object> manifestMap = record.manifestAsMap();
    assert "1".equals(manifestMap.get("version")) : "Manifest map mismatch";
    assert manifestMap.get("dataspace") instanceof Number
        && ((Number) manifestMap.get("dataspace")).longValue() == 9L
        : "Manifest dataspace mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request
        .uri()
        .toString()
        .equals(
            "https://torii.example/v1/space-directory/uaids/uaid%3A"
                + hex
                + "/manifests?dataspace=9&status=inactive&limit=25&offset=5&address_format=ih58")
        : "Manifest URI must include encoded query parameters";
  }

  private static void identifierPoliciesRequestParsesResponse() {
    final String json =
        "{"
            + "\"total\":1,"
            + "\"items\":[{"
            + "\"policy_id\":\"phone#retail\","
            + "\"owner\":\"alice@wonderland\","
            + "\"active\":true,"
            + "\"normalization\":\"phone_e164\","
            + "\"resolver_public_key\":\"ed25519:resolver-key\","
            + "\"backend\":\"bfv-affine-sha3-256-v1\","
            + "\"input_encryption\":\"bfv-v1\","
            + "\"input_encryption_public_parameters\":\"ABCD\","
            + "\"note\":\"retail phone policy\""
            + "}]"
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final IdentifierPolicyListResponse response = transport.listIdentifierPolicies().join();
    assert response.total() == 1L : "Policy list total mismatch";
    assert response.items().size() == 1 : "Expected one identifier policy";
    final IdentifierPolicySummary item = response.items().get(0);
    assert "phone#retail".equals(item.policyId()) : "Policy id mismatch";
    assert "alice@wonderland".equals(item.owner()) : "Owner mismatch";
    assert item.active() : "Policy should be active";
    assert item.normalization() == IdentifierNormalization.PHONE_E164
        : "Normalization mismatch";
    assert "bfv-v1".equals(item.inputEncryption()) : "Input encryption mismatch";
    assert "ABCD".equals(item.inputEncryptionPublicParameters())
        : "Input encryption params mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Identifier policy request must be captured";
    assert "GET".equals(request.method()) : "Identifier policy list must use GET";
    assert request.uri().toString().equals("https://torii.example/v1/identifier-policies")
        : "Identifier policy URI mismatch";
    assert request.headers().getOrDefault("Accept", List.of()).contains("application/json")
        : "Identifier policy request must accept JSON";
  }

  private static void identifierResolveRequestParsesResponse() {
    final String accountId = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
    final String json =
        "{"
            + "\"policy_id\":\"phone#retail\","
            + "\"opaque_id\":\"opaque:"
            + "11".repeat(32)
            + "\","
            + "\"receipt_hash\":\""
            + "22".repeat(32)
            + "\","
            + "\"uaid\":\"uaid:"
            + "33".repeat(31)
            + "35\","
            + "\"account_id\":\""
            + accountId
            + "\","
            + "\"resolved_at_ms\":42,"
            + "\"expires_at_ms\":142,"
            + "\"backend\":\"bfv-affine-sha3-256-v1\","
            + "\"signature\":\""
            + "aa".repeat(64)
            + "\""
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final Optional<IdentifierResolutionReceipt> response =
        transport.resolveIdentifier(" phone#retail ", " +1 (555) 123-4567 ", null).join();
    assert response.isPresent() : "Expected identifier resolution receipt";
    final IdentifierResolutionReceipt receipt = response.orElseThrow();
    assert "phone#retail".equals(receipt.policyId()) : "Policy id mismatch";
    assert ("opaque:" + "11".repeat(32)).equals(receipt.opaqueId()) : "Opaque id mismatch";
    assert "22".repeat(32).equals(receipt.receiptHash()) : "Receipt hash mismatch";
    assert ("uaid:" + "33".repeat(31) + "35").equals(receipt.uaid()) : "UAID mismatch";
    assert accountId.equals(receipt.accountId()) : "Account id mismatch";
    assert receipt.resolvedAtMs() == 42L : "Resolved timestamp mismatch";
    assert Long.valueOf(142L).equals(receipt.expiresAtMs()) : "Expiry mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Identifier resolve request must be captured";
    assert "POST".equals(request.method()) : "Identifier resolve must use POST";
    assert request.uri().toString().equals("https://torii.example/v1/identifiers/resolve")
        : "Identifier resolve URI mismatch";
    assert request.headers().getOrDefault("Content-Type", List.of()).contains("application/json")
        : "Identifier resolve must send JSON";
    assert readBody(request)
        .equals("{\"input\":\"+1 (555) 123-4567\",\"policy_id\":\"phone#retail\"}")
        : "Identifier resolve payload mismatch";
  }

  private static void identifierResolveRequestAllowsNotFound() {
    final StubResponseExecutor executor = new StubResponseExecutor(404, new byte[0], "not found");
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final Optional<IdentifierResolutionReceipt> response =
        transport.resolveIdentifier("phone#retail", null, "0xABCD").join();
    assert response.isEmpty() : "404 identifier resolution should return Optional.empty";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Identifier resolve request must be captured";
    assert readBody(request).equals("{\"encrypted_input\":\"abcd\",\"policy_id\":\"phone#retail\"}")
        : "Encrypted identifier resolve payload mismatch";
  }

  private static void identifierClaimReceiptUsesAccountPath() {
    final String accountId = "alice@wonderland";
    final String json =
        "{"
            + "\"policy_id\":\"phone#retail\","
            + "\"opaque_id\":\"opaque:"
            + "44".repeat(32)
            + "\","
            + "\"receipt_hash\":\""
            + "55".repeat(32)
            + "\","
            + "\"uaid\":\"uaid:"
            + "66".repeat(31)
            + "67\","
            + "\"account_id\":\""
            + accountId
            + "\","
            + "\"resolved_at_ms\":7,"
            + "\"backend\":\"bfv-affine-sha3-256-v1\","
            + "\"signature\":\""
            + "bb".repeat(64)
            + "\""
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build());

    final Optional<IdentifierResolutionReceipt> response =
        transport.issueIdentifierClaimReceipt(accountId, "phone#retail", null, "ABCD").join();
    assert response.isPresent() : "Claim receipt should parse";
    assert ("opaque:" + "44".repeat(32)).equals(response.orElseThrow().opaqueId())
        : "Opaque id mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Identifier claim request must be captured";
    assert request
        .uri()
        .toString()
        .equals("https://torii.example/api/v1/accounts/alice%40wonderland/identifiers/claim-receipt")
        : "Identifier claim receipt path must encode account id";
    assert readBody(request).equals("{\"encrypted_input\":\"abcd\",\"policy_id\":\"phone#retail\"}")
        : "Identifier claim payload mismatch";
  }

  private static void identifierNormalizationCanonicalizesInputs() {
    assert "+15551234567".equals(
            IdentifierNormalization.PHONE_E164.normalize(" +1 (555) 123-4567 ", "phone"))
        : "Phone normalization mismatch";
    assert "alice.example@example.com".equals(
            IdentifierNormalization.EMAIL_ADDRESS.normalize(
                " Alice.Example@Example.COM ", "email"))
        : "Email normalization mismatch";
    assert "GB82WEST1234".equals(
            IdentifierNormalization.ACCOUNT_NUMBER.normalize(" gb82-west-1234 ", "account"))
        : "Account normalization mismatch";
  }

  private static void invalidateAndCancelDelegatesToExecutor() {
    final InvalidationTrackingExecutor executor = new InvalidationTrackingExecutor();
    final ClientConfig config =
        ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);
    transport.invalidateAndCancel();
    assert executor.invalidated : "invalidateAndCancel should reach the executor";
  }

  private static String readBody(final TransportRequest request) {
    return new String(request.body(), StandardCharsets.UTF_8);
  }

  private static final class CapturingExecutor implements HttpTransportExecutor {
    private TransportRequest lastRequest;

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      this.lastRequest = Objects.requireNonNull(request, "request");
      return CompletableFuture.completedFuture(
          new TransportResponse(202, new byte[0], "accepted", Map.of()));
    }
  }

  private static final class FailingExecutor implements HttpTransportExecutor {
    private final RuntimeException error;

    private FailingExecutor(final RuntimeException error) {
      this.error = error;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      final CompletableFuture<TransportResponse> future = new CompletableFuture<>();
      future.completeExceptionally(error);
      return future;
    }
  }

  private static final class CountingFailingExecutor implements HttpTransportExecutor {
    private final RuntimeException error;
    private int callCount = 0;

    private CountingFailingExecutor(final RuntimeException error) {
      this.error = error;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      callCount++;
      final CompletableFuture<TransportResponse> future = new CompletableFuture<>();
      future.completeExceptionally(error);
      return future;
    }
  }

  private static final class SequencedExecutor implements HttpTransportExecutor {
    private int callCount = 0;

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      callCount++;
      if (callCount == 1) {
        return CompletableFuture.completedFuture(
            new TransportResponse(503, "retry".getBytes(StandardCharsets.UTF_8), "", Map.of()));
      }
      return CompletableFuture.completedFuture(
          new TransportResponse(202, new byte[0], "accepted", Map.of()));
    }
  }

  private static final class ScriptedExecutor implements HttpTransportExecutor {
    private final TransportResponse[] responses;
    private int index = 0;

    private ScriptedExecutor(final TransportResponse... responses) {
      this.responses = Objects.requireNonNull(responses, "responses");
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      final int position = index < responses.length ? index : responses.length - 1;
      index++;
      return CompletableFuture.completedFuture(responses[position]);
    }
  }

  private static final class RecordingExecutor implements HttpTransportExecutor {
    private final List<byte[]> payloads = new ArrayList<>();

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      try {
        payloads.add(request.body());
      } catch (final Exception ex) {
        final CompletableFuture<TransportResponse> failed = new CompletableFuture<>();
        failed.completeExceptionally(ex);
        return failed;
      }
      return CompletableFuture.completedFuture(
          new TransportResponse(202, new byte[0], "accepted", Map.of()));
    }
  }

  private static final class RecordingTelemetrySink implements TelemetrySink {
    private final List<GaugeEvent> events = new ArrayList<>();

    @Override
    public void onRequest(final TelemetryRecord record) {}

    @Override
    public void onResponse(final TelemetryRecord record, final ClientResponse response) {}

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {}

    @Override
    public void emitSignal(final String signalId, final Map<String, Object> fields) {
      events.add(new GaugeEvent(signalId, fields));
    }

    GaugeEvent lastEvent(final String signalId) {
      for (int i = events.size() - 1; i >= 0; --i) {
        final GaugeEvent event = events.get(i);
        if (event.signalId().equals(signalId)) {
          return event;
        }
      }
      return null;
    }

    List<Map<String, Object>> eventsBySignal(final String signalId) {
      final List<Map<String, Object>> matches = new ArrayList<>();
      for (final GaugeEvent event : events) {
        if (event.signalId().equals(signalId)) {
          matches.add(event.fields());
        }
      }
      return matches;
    }

    private static final class GaugeEvent {
      private final String signalId;
      private final Map<String, Object> fields;

      private GaugeEvent(final String signalId, final Map<String, Object> fields) {
        this.signalId = signalId;
        this.fields = fields;
      }

      String signalId() {
        return signalId;
      }

      Map<String, Object> fields() {
        return fields;
      }
    }
  }

  private static final class StubResponseExecutor implements HttpTransportExecutor {
    private final TransportResponse response;
    private TransportRequest lastRequest;

    private StubResponseExecutor(final int statusCode, final byte[] body) {
      this(statusCode, body, "accepted");
    }

    private StubResponseExecutor(
        final int statusCode, final byte[] body, final String message) {
      this.response = new TransportResponse(statusCode, body, message, Map.of());
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      lastRequest = request;
      return CompletableFuture.completedFuture(response);
    }

    TransportRequest lastRequest() {
      return lastRequest;
    }
  }

  private static final class InvalidationTrackingExecutor implements HttpTransportExecutor {
    private boolean invalidated = false;

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      return CompletableFuture.completedFuture(
          new TransportResponse(200, new byte[0], "ok", Map.of()));
    }

    @Override
    public void invalidateAndCancel() {
      invalidated = true;
    }
  }

  private static final class RecordingObserver implements ClientObserver {
    private final AtomicInteger requestCount = new AtomicInteger();
    private final AtomicInteger responseCount = new AtomicInteger();
    private final AtomicInteger failureCount = new AtomicInteger();

    @Override
    public void onRequest(final TransportRequest request) {
      requestCount.incrementAndGet();
    }

    @Override
    public void onResponse(final TransportRequest request, final ClientResponse response) {
      responseCount.incrementAndGet();
    }

    @Override
    public void onFailure(final TransportRequest request, final Throwable error) {
      failureCount.incrementAndGet();
    }
  }

  private static int aliasCounter = 0;

  private static SignedTransaction transactionWithPayload(final byte fillValue) {
    final byte[] signature = new byte[64];
    final byte[] publicKey = new byte[32];
    java.util.Arrays.fill(signature, (byte) (fillValue + 1));
    java.util.Arrays.fill(publicKey, (byte) (fillValue + 2));
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId(String.format("%08x", fillValue))
            .setAuthority("alice@wonderland")
            .setCreationTimeMs(1_700_000_000_000L + (fillValue & 0xFF))
            .setInstructionBytes(new byte[] {fillValue, (byte) (fillValue + 1)})
            .setTimeToLiveMs(5_000L)
            .setNonce(fillValue & 0xFF)
            .setMetadata(Map.of("note", "txn-" + fillValue))
            .build();
    final NoritoJavaCodecAdapter codec = new NoritoJavaCodecAdapter();
    final byte[] encodedPayload;
    try {
      encodedPayload = codec.encodeTransaction(payload);
    } catch (final Exception ex) {
      throw new IllegalStateException("Failed to encode transaction payload", ex);
    }
    return new SignedTransaction(
        encodedPayload, signature, publicKey, codec.schemaName(), "alias-" + aliasCounter++);
  }

  private static byte[] statusPayload(final String kind) {
    final String json =
        "{\"kind\":\"Transaction\",\"content\":{\"status\":{\"kind\":\"" + kind + "\"}}}";
    return json.getBytes(StandardCharsets.UTF_8);
  }

  private static boolean payloadEquals(
      final SignedTransaction expected, final SignedTransaction actual) {
    return java.util.Arrays.equals(expected.encodedPayload(), actual.encodedPayload())
        && java.util.Arrays.equals(expected.signature(), actual.signature())
        && java.util.Arrays.equals(expected.publicKey(), actual.publicKey())
        && expected.schemaName().equals(actual.schemaName())
        && expected.keyAlias().equals(actual.keyAlias())
        && expected.exportedKeyBundle().equals(actual.exportedKeyBundle());
  }

}
