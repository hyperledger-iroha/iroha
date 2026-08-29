package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.function.Consumer;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.client.queue.PendingTransactionQueue;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.TransactionAdmissionIntent;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.hyperledger.iroha.android.tx.SignedTransaction;

/** Focused tests for the fresh transaction-submission compatibility guard. */
public final class TransactionSubmissionCompatibilityTests {
  private TransactionSubmissionCompatibilityTests() {}

  /** Runs the compatibility-guard regression tests. */
  public static void main(final String[] args) {
    allFourTransactionIngressFormsGuardImmediatelyBeforePost();
    dataModelDriftFailsBeforeTransactionPost();
    schemaDriftFailsBeforeEntrypointPost();
    guardFetchesFreshCapabilitiesForEachSubmission();
    redirectAndTransientStatusesNeverRedispatchSignedBytes();
    networkFailureIsAmbiguousAndNeverRedispatchesSignedBytes();
    configuredPendingQueueIsNeverDrainedOrReplayedImplicitly();
    compatibilityFailureNeitherDrainsQueueNorDispatchesSignedBytes();
    System.out.println(
        "[IrohaAndroid] transaction submission compatibility tests passed.");
  }

  private static void allFourTransactionIngressFormsGuardImmediatelyBeforePost() {
    assertIngress(
        client -> client.submitTransaction(sampleTransaction(1)).join(),
        "/v1/pipeline/transactions");
    assertIngress(
        client ->
            client
                .submitTransactionJson(
                    "{\"version\":1}".getBytes(StandardCharsets.UTF_8))
                .join(),
        "/v1/pipeline/transactions");
    assertIngress(
        client ->
            client.submitTransactionEntrypoint(new byte[] {1, 2, 3}).join(),
        "/v1/pipeline/transaction-entrypoints");
    assertIngress(
        client ->
            client
                .submitTransactionEntrypointJson(
                    "{\"version\":1}".getBytes(StandardCharsets.UTF_8))
                .join(),
        "/v1/pipeline/transaction-entrypoints");
  }

  private static void assertIngress(
      final Consumer<HttpClientTransport> submission, final String postPath) {
    final CompatibilityExecutor executor =
        new CompatibilityExecutor(Collections.singletonList(capabilities(4, expectedSchemaHash())));

    submission.accept(transport(executor, null));

    assert executor.requests().equals(
            Arrays.asList(
                "GET /v1/node/capabilities",
                "POST " + postPath))
        : "capability GET must immediately precede transaction POST: "
            + executor.requests();
  }

  private static void dataModelDriftFailsBeforeTransactionPost() {
    final CompatibilityExecutor executor =
        new CompatibilityExecutor(Collections.singletonList(capabilities(5, expectedSchemaHash())));

    final CompletionException failure =
        expectCompletionFailure(
            () ->
                transport(executor, null)
                    .submitTransactionJson(
                        "{\"version\":1}".getBytes(StandardCharsets.UTF_8))
                    .join());

    assert failure.getCause() instanceof ToriiDataModelMismatchException
        : "expected typed data-model mismatch";
    final ToriiDataModelMismatchException mismatch =
        (ToriiDataModelMismatchException) failure.getCause();
    assert mismatch.expected() == 4 : "expected client data-model version 4";
    assert mismatch.actual() == 5 : "expected advertised data-model version 5";
    assert executor.requests().equals(
            Collections.singletonList("GET /v1/node/capabilities"))
        : "data-model drift must block POST";
  }

  private static void schemaDriftFailsBeforeEntrypointPost() {
    final String driftedHash = repeat('0', 32);
    final CompatibilityExecutor executor =
        new CompatibilityExecutor(Collections.singletonList(capabilities(4, driftedHash)));

    final CompletionException failure =
        expectCompletionFailure(
            () ->
                transport(executor, null)
                    .submitTransactionEntrypoint(new byte[] {1})
                    .join());

    assert failure.getCause() instanceof ToriiTransactionSchemaMismatchException
        : "expected typed signed-transaction schema mismatch";
    final ToriiTransactionSchemaMismatchException mismatch =
        (ToriiTransactionSchemaMismatchException) failure.getCause();
    assert expectedSchemaHash().equals(mismatch.expected())
        : "expected client schema hash";
    assert driftedHash.equals(mismatch.actual()) : "expected advertised schema hash";
    assert executor.requests().equals(
            Collections.singletonList("GET /v1/node/capabilities"))
        : "schema drift must block POST";
  }

  private static void guardFetchesFreshCapabilitiesForEachSubmission() {
    final CompatibilityExecutor executor =
        new CompatibilityExecutor(
            Arrays.asList(
                capabilities(4, expectedSchemaHash()),
                capabilities(3, expectedSchemaHash())));
    final HttpClientTransport client = transport(executor, null);

    client
        .submitTransactionJson(
            "{\"version\":1}".getBytes(StandardCharsets.UTF_8))
        .join();
    final CompletionException failure =
        expectCompletionFailure(
            () ->
                client
                    .submitTransactionJson(
                        "{\"version\":1}".getBytes(StandardCharsets.UTF_8))
                    .join());

    assert failure.getCause() instanceof ToriiDataModelMismatchException
        : "second submission must observe fresh drift";
    assert executor.requests().equals(
            Arrays.asList(
                "GET /v1/node/capabilities",
                "POST /v1/pipeline/transactions",
                "GET /v1/node/capabilities"))
        : "second submission must probe again and block its POST";
  }

  private static void redirectAndTransientStatusesNeverRedispatchSignedBytes() {
    for (final int status : Arrays.asList(307, 308, 503)) {
      final SignedTransaction transaction = sampleTransaction(status);
      final CompatibilityExecutor executor =
          new CompatibilityExecutor(
              Collections.singletonList(capabilities(4, expectedSchemaHash())),
              Arrays.asList(status, 202));
      final HttpClientTransport client =
          transport(
              executor,
              null,
              RetryPolicy.builder()
                  .setMaxAttempts(2)
                  .setBaseDelay(Duration.ZERO)
                  .build());

      final CompletionException failure =
          expectCompletionFailure(() -> client.submitTransaction(transaction).join());
      assert failure.getCause() instanceof AmbiguousTransactionSubmissionException
          : "redirect/transient response must surface an ambiguous outcome";
      final AmbiguousTransactionSubmissionException ambiguous =
          (AmbiguousTransactionSubmissionException) failure.getCause();
      assert org.hyperledger.iroha.android.tx.SignedTransactionHasher.hashHex(transaction)
          .equals(ambiguous.hashHex()) : "ambiguous outcome must carry canonical hash";
      assert ambiguous.statusCode().isPresent()
          && ambiguous.statusCode().getAsInt() == status : "ambiguous status must be retained";
      assert executor.requests().equals(
              Arrays.asList(
                  "GET /v1/node/capabilities",
                  "POST /v1/pipeline/transactions"))
          : "signed bytes must be dispatched once: " + executor.requests();
    }
  }

  private static void networkFailureIsAmbiguousAndNeverRedispatchesSignedBytes() {
    final SignedTransaction transaction = sampleTransaction(7);
    final NetworkFailureExecutor executor = new NetworkFailureExecutor();
    final HttpClientTransport client =
        transport(
            executor,
            null,
            RetryPolicy.builder()
                .setMaxAttempts(3)
                .setBaseDelay(Duration.ZERO)
                .build());

    final CompletionException failure =
        expectCompletionFailure(() -> client.submitTransaction(transaction).join());
    assert failure.getCause() instanceof AmbiguousTransactionSubmissionException
        : "network failure must surface an ambiguous outcome";
    final AmbiguousTransactionSubmissionException ambiguous =
        (AmbiguousTransactionSubmissionException) failure.getCause();
    assert org.hyperledger.iroha.android.tx.SignedTransactionHasher.hashHex(transaction)
        .equals(ambiguous.hashHex()) : "ambiguous outcome must carry canonical hash";
    assert !ambiguous.statusCode().isPresent() : "network failure has no HTTP status";
    assert executor.requests.equals(
            Arrays.asList(
                "GET /v1/node/capabilities",
                "POST /v1/pipeline/transactions"))
        : "network failure must not redispatch signed bytes";
  }

  private static void configuredPendingQueueIsNeverDrainedOrReplayedImplicitly() {
    final MemoryPendingQueue queue = new MemoryPendingQueue();
    queue.enqueue(sampleTransaction(10));
    final CompatibilityExecutor executor =
        new CompatibilityExecutor(Collections.singletonList(capabilities(4, expectedSchemaHash())));

    transport(executor, queue).submitTransaction(sampleTransaction(11)).join();

    assert queue.size() == 1 : "submission must not drain persisted signed bytes";
    assert executor.requests().equals(
            Arrays.asList(
                "GET /v1/node/capabilities",
                "POST /v1/pipeline/transactions"))
        : "only the caller-supplied transaction may be dispatched";
  }

  private static void compatibilityFailureNeitherDrainsQueueNorDispatchesSignedBytes() {
    final MemoryPendingQueue queue = new MemoryPendingQueue();
    queue.enqueue(sampleTransaction(20));
    final CompatibilityExecutor executor =
        new CompatibilityExecutor(Collections.singletonList(capabilities(9, expectedSchemaHash())));

    final CompletionException failure =
        expectCompletionFailure(
            () ->
                transport(executor, queue)
                    .submitTransaction(sampleTransaction(21))
                    .join());

    assert failure.getCause() instanceof ToriiDataModelMismatchException
        : "live submission must report the fresh mismatch";
    assert queue.size() == 1 : "failed queued replay must retain its current transaction";
    assert executor.requests().equals(
            Collections.singletonList("GET /v1/node/capabilities"))
        : "compatibility failure must stop before POST";
  }

  private static HttpClientTransport transport(
      final HttpTransportExecutor executor, final PendingTransactionQueue queue) {
    return transport(executor, queue, RetryPolicy.none());
  }

  private static HttpClientTransport transport(
      final HttpTransportExecutor executor,
      final PendingTransactionQueue queue,
      final RetryPolicy retryPolicy) {
    return HttpClientTransport.withExecutor(
        executor,
        ClientConfig.builder()
            .setBaseUri(URI.create("https://torii.example"))
            .setPendingQueue(queue)
            .setRetryPolicy(retryPolicy)
            .build());
  }

  private static SignedTransaction sampleTransaction(final int seed) {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setFeePayment(
                FeePaymentIntent.authority(Collections.emptyList(), 1L))
            .setNetworkId(
                org.hyperledger.iroha.android.testing.TestNetworkIds.fromSeed(seed))
            .setAuthority(TestAccountIds.ed25519Authority(0x37))
            .setCreationTimeMs(1_700_000_000_000L + seed)
            .setInstructionBytes(new byte[] {(byte) seed, (byte) (seed + 1)})
            .setTimeToLiveMs(5_000L)
            .setNonce(seed + 1L)
            .setAdmissionIntent(TransactionAdmissionIntent.QUEUE_PLAN_SYNCED)
            .setMetadata(
                Collections.singletonMap("note", "compat-" + seed))
            .build();
    final NoritoJavaCodecAdapter codec =
        new NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    try {
      return new SignedTransaction(
          codec.encodeTransaction(payload),
          filledBytes(64, (byte) (seed + 1)),
          filledBytes(32, (byte) (seed + 2)),
          codec.schemaName());
    } catch (final Exception error) {
      throw new IllegalStateException("failed to encode compatibility test transaction", error);
    }
  }

  private static byte[] filledBytes(final int size, final byte value) {
    final byte[] bytes = new byte[size];
    Arrays.fill(bytes, value);
    return bytes;
  }

  private static CompletionException expectCompletionFailure(final Runnable action) {
    try {
      action.run();
    } catch (final CompletionException failure) {
      return failure;
    }
    throw new AssertionError("expected completion failure");
  }

  private static String capabilities(
      final int dataModelVersion, final String schemaHash) {
    return "{\"data_model_version\":"
        + dataModelVersion
        + ",\"signed_transaction_schema_hash_hex\":\""
        + schemaHash
        + "\"}";
  }

  private static String expectedSchemaHash() {
    return ToriiTransactionCompatibility
        .EXPECTED_SIGNED_TRANSACTION_SCHEMA_HASH_HEX;
  }

  private static String repeat(final char value, final int count) {
    final StringBuilder result = new StringBuilder(count);
    for (int index = 0; index < count; index++) {
      result.append(value);
    }
    return result.toString();
  }

  private static final class CompatibilityExecutor
      implements HttpTransportExecutor {
    private final ArrayDeque<String> capabilityResponses;
    private final ArrayDeque<Integer> postResponses;
    private final List<String> requests = new ArrayList<>();
    private String lastCapability;
    private int lastPostStatus;

    private CompatibilityExecutor(final List<String> capabilities) {
      this(capabilities, Collections.singletonList(202));
    }

    private CompatibilityExecutor(
        final List<String> capabilities, final List<Integer> postStatuses) {
      capabilityResponses = new ArrayDeque<>(capabilities);
      postResponses = new ArrayDeque<>(postStatuses);
      lastCapability = capabilities.get(capabilities.size() - 1);
      lastPostStatus = postStatuses.get(postStatuses.size() - 1);
    }

    @Override
    public CompletableFuture<TransportResponse> execute(
        final TransportRequest request) {
      requests.add(request.method() + " " + request.uri().getPath());
      if ("GET".equals(request.method())
          && request.uri().getPath().endsWith("/v1/node/capabilities")) {
        if (!capabilityResponses.isEmpty()) {
          lastCapability = capabilityResponses.removeFirst();
        }
        return CompletableFuture.completedFuture(
            new TransportResponse(
                200,
                lastCapability.getBytes(StandardCharsets.UTF_8),
                "",
                Collections.emptyMap(),
                null,
                false));
      }
      if (!postResponses.isEmpty()) {
        lastPostStatus = postResponses.removeFirst();
      }
      return CompletableFuture.completedFuture(
          new TransportResponse(
              lastPostStatus, new byte[0], "", Collections.emptyMap(),
              null,
              false));
    }

    private List<String> requests() {
      return requests;
    }
  }

  private static final class NetworkFailureExecutor
      implements HttpTransportExecutor {
    private final List<String> requests = new ArrayList<>();

    @Override
    public CompletableFuture<TransportResponse> execute(
        final TransportRequest request) {
      requests.add(request.method() + " " + request.uri().getPath());
      if ("GET".equals(request.method())
          && request.uri().getPath().endsWith("/v1/node/capabilities")) {
        return CompletableFuture.completedFuture(
            new TransportResponse(
                200,
                capabilities(4, expectedSchemaHash()).getBytes(StandardCharsets.UTF_8),
                "",
                Collections.emptyMap(),
                null,
                false));
      }
      final CompletableFuture<TransportResponse> failed = new CompletableFuture<>();
      failed.completeExceptionally(new RuntimeException("simulated network failure"));
      return failed;
    }
  }

  private static final class MemoryPendingQueue
      implements PendingTransactionQueue {
    private final List<SignedTransaction> pending = new ArrayList<>();

    @Override
    public void enqueue(final SignedTransaction transaction) {
      pending.add(transaction);
    }

    @Override
    public List<SignedTransaction> drain() {
      final List<SignedTransaction> drained = new ArrayList<>(pending);
      pending.clear();
      return drained;
    }

    @Override
    public int size() {
      return pending.size();
    }

    @Override
    public String telemetryQueueName() {
      return "memory";
    }
  }
}
