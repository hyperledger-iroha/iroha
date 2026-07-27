package org.hyperledger.iroha.android.sorafs;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.io.ByteArrayInputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Collections;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import org.hyperledger.iroha.android.client.CanonicalRequestSigner;
import org.hyperledger.iroha.android.client.ToriiCanonicalRequestAuth;
import org.hyperledger.iroha.android.client.stream.ToriiEventStream;
import org.hyperledger.iroha.android.client.transport.StreamingTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.client.transport.TransportStreamResponse;
import org.hyperledger.iroha.android.sorafs.SorafsReputationModels.EventStreamListener;
import org.hyperledger.iroha.android.sorafs.SorafsReputationModels.SnapshotEventV1;
import org.hyperledger.iroha.android.sorafs.SorafsReputationModels.SnapshotSummaryV1;
import org.junit.Test;

/** Focused hard-cut tests for authenticated Java SoraFS reputation reads. */
public final class SorafsReputationClientTest {

  private static final URI BASE_URI = URI.create("https://torii.example");
  private static final long TIMESTAMP_MS = 1_717_171_717_000L;
  private static final byte[] SIGNATURE = new byte[] {1, 2, 3};
  private static final String SNAPSHOT_ID = "abababababababababababababababab";
  private static final String NEXT_SNAPSHOT_ID = "bcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbc";
  private static final String MERKLE_ROOT =
      "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";
  private static final String METRICS_HASH =
      "efefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef";
  private static final String WEIGHTS =
      "\"version\":1,"
          + "\"por_success_bps\":2200,"
          + "\"pdp_success_bps\":2000,"
          + "\"potr_success_bps\":1800,"
          + "\"latency_bps\":1500,"
          + "\"dispute_bps\":1000,"
          + "\"token_violation_bps\":500,"
          + "\"repair_breach_bps\":1000";
  private static final String METRICS =
      "\"version\":1,"
          + "\"por_success_bps\":9500,"
          + "\"pdp_success_bps\":9600,"
          + "\"potr_success_bps\":9700,"
          + "\"latency_health_bps\":9800,"
          + "\"dispute_rate_bps\":100,"
          + "\"token_violation_rate_bps\":0,"
          + "\"repair_breach_rate_bps\":50";
  private static final String PROVIDER_OBJECT =
      "{\"provider_id\":\"provider:alpha\","
          + "\"score_bps\":900,"
          + "\"degradation_flags\":["
          + "{\"flag\":\"reserve_warning\",\"value\":null},"
          + "{\"flag\":\"low_score\",\"value\":null}],"
          + "\"raw_metrics\":{"
          + METRICS
          + "},"
          + "\"raw_metrics_hash_hex\":\""
          + METRICS_HASH
          + "\"}";
  private static final String SNAPSHOT_JSON =
      "{\"snapshot_id_hex\":\""
          + SNAPSHOT_ID
          + "\","
          + "\"generated_at_unix\":18446744073709551615,"
          + "\"previous_snapshot_id_hex\":null,"
          + "\"merkle_root_hex\":\""
          + MERKLE_ROOT
          + "\","
          + "\"provider_count\":1,"
          + "\"returned_provider_count\":1,"
          + "\"limit\":500,"
          + "\"truncated_providers\":false,"
          + "\"alpha_bps\":8500,"
          + "\"current_score_weight_bps\":7000,"
          + "\"weights\":{"
          + WEIGHTS
          + "},"
          + "\"providers\":["
          + PROVIDER_OBJECT
          + "]}";
  private static final String PROVIDER_JSON =
      "{\"snapshot_id_hex\":\""
          + SNAPSHOT_ID
          + "\","
          + "\"generated_at_unix\":18446744073709551615,"
          + "\"merkle_root_hex\":\""
          + MERKLE_ROOT
          + "\","
          + "\"provider\":"
          + PROVIDER_OBJECT
          + ","
          + "\"proof\":{\"provider_id\":\"provider:alpha\","
          + "\"leaf_index\":0,\"leaf_count\":1,\"siblings_hex\":[]}}";
  private static final String WEIGHTS_JSON =
      "{\"snapshot_id_hex\":\""
          + SNAPSHOT_ID
          + "\","
          + "\"generated_at_unix\":18446744073709551615,"
          + "\"alpha_bps\":8500,"
          + "\"current_score_weight_bps\":7000,"
          + "\"weights\":{"
          + WEIGHTS
          + "}}";
  private static final String EVENT_JSON =
      "{\"version\":1,"
          + "\"sequence\":8,"
          + "\"snapshot_id_hex\":\""
          + SNAPSHOT_ID
          + "\","
          + "\"generated_at_unix\":18446744073709551615,"
          + "\"merkle_root_hex\":\""
          + MERKLE_ROOT
          + "\","
          + "\"provider_count\":1,"
          + "\"previous_snapshot_id_hex\":null}";
  private static final String EVENT_PAGE_JSON =
      "{\"since\":7,\"limit\":1,\"count\":1,\"next_since\":8,\"events\":["
          + EVENT_JSON
          + "]}";

  @Test
  public void finiteReadsUseExactAuthenticatedGetTargetsAndTypedProjection() {
    final RecordingStreamingExecutor executor = new RecordingStreamingExecutor();
    final SorafsReputationClient client = new SorafsReputationClient(BASE_URI, executor);

    executor.enqueueJson(SNAPSHOT_JSON);
    final RecordedAuth latestAuth = auth("latest");
    final Optional<SnapshotSummaryV1> latest = client.getLatest(latestAuth.auth, 500).join();
    assertTrue(latest.isPresent());
    assertEquals(SNAPSHOT_ID, latest.get().snapshotIdHex());
    assertEquals("18446744073709551615", latest.get().generatedAtUnix());
    assertEquals("provider:alpha", latest.get().providers().get(0).providerId());
    assertExactSignedGet(
        executor.requests.get(0),
        "/v1/sorafs/reputation/latest",
        "limit=500",
        latestAuth);

    executor.enqueueJson(PROVIDER_JSON);
    final RecordedAuth providerAuth = auth("provider");
    assertEquals(
        "provider:alpha",
        client
            .getProvider("provider:alpha", providerAuth.auth)
            .join()
            .get()
            .proof()
            .providerId());
    assertExactSignedGet(
        executor.requests.get(1),
        "/v1/sorafs/reputation/providers/provider:alpha",
        null,
        providerAuth);
    assertFalse(executor.requests.get(1).uri().toASCIIString().toLowerCase().contains("%3a"));

    executor.enqueueJson(SNAPSHOT_JSON.replace("\"limit\":500", "\"limit\":1"));
    final RecordedAuth snapshotAuth = auth("snapshot");
    assertEquals(
        SNAPSHOT_ID,
        client.getSnapshot(SNAPSHOT_ID, snapshotAuth.auth, 1).join().get().snapshotIdHex());
    assertExactSignedGet(
        executor.requests.get(2),
        "/v1/sorafs/reputation/snapshots/" + SNAPSHOT_ID,
        "limit=1",
        snapshotAuth);

    executor.enqueueJson(WEIGHTS_JSON);
    final RecordedAuth weightsAuth = auth("weights");
    assertEquals(2200, client.getWeights(weightsAuth.auth).join().weights().porSuccessBps());
    assertExactSignedGet(
        executor.requests.get(3),
        "/v1/sorafs/reputation/weights",
        null,
        weightsAuth);

    executor.enqueueJson(EVENT_PAGE_JSON);
    final RecordedAuth eventsAuth = auth("events");
    assertEquals("8", client.listEvents(eventsAuth.auth, "7", 1).join().nextSince());
    assertExactSignedGet(
        executor.requests.get(4),
        "/v1/sorafs/reputation/events",
        "since=7&limit=1",
        eventsAuth);

    executor.enqueueStatus(404);
    assertFalse(client.getLatest(auth("missing").auth).join().isPresent());
    assertEquals(6, executor.requests.size());
  }

  @Test
  public void rejectsNoncanonicalInputsAndPartialAuthenticationBeforeTransport() {
    final RecordingStreamingExecutor executor = new RecordingStreamingExecutor();
    final SorafsReputationClient client = new SorafsReputationClient(BASE_URI, executor);
    final ToriiCanonicalRequestAuth canonicalAuth = auth("valid").auth;
    final String[] snapshots = {
      repeat("AB", 16),
      "0x" + repeat("ab", 16),
      " " + repeat("ab", 16),
      repeat("0", 32),
      repeat("ab", 15)
    };
    for (final String snapshot : snapshots) {
      assertThrows(
          IllegalArgumentException.class, () -> client.getSnapshot(snapshot, canonicalAuth));
    }
    final String[] providers = {
      "",
      "provider alias",
      "provider/alpha",
      "provider%3Aalpha",
      "provider\u00e9",
      ".",
      "..",
      repeat("a", 257)
    };
    for (final String provider : providers) {
      assertThrows(
          IllegalArgumentException.class, () -> client.getProvider(provider, canonicalAuth));
    }
    final String[] cursors = {
      "", "01", "-1", "+1", " 1", "18446744073709551616"
    };
    for (final String cursor : cursors) {
      assertThrows(
          IllegalArgumentException.class, () -> client.listEvents(canonicalAuth, cursor, 1));
    }
    for (final int limit : new int[] {0, 501, -1}) {
      assertThrows(
          IllegalArgumentException.class, () -> client.listEvents(canonicalAuth, "0", limit));
    }
    final AtomicInteger partialSignCalls = new AtomicInteger();
    final ToriiCanonicalRequestAuth partialAuth =
        new ToriiCanonicalRequestAuth(
            "reputation-reader@sora",
            message -> {
              partialSignCalls.incrementAndGet();
              return SIGNATURE.clone();
            },
            TIMESTAMP_MS,
            null);
    assertThrows(IllegalArgumentException.class, () -> client.getWeights(partialAuth));
    assertEquals(0, partialSignCalls.get());
    assertTrue(executor.requests.isEmpty());
  }

  @Test
  public void authenticatedSseUsesOneStreamingAttemptWithoutResumeSurface() throws Exception {
    final RecordingStreamingExecutor executor = new RecordingStreamingExecutor();
    final SorafsReputationClient client = new SorafsReputationClient(BASE_URI, executor);
    final RecordedAuth canonicalAuth = auth("stream");
    executor.enqueueStream(
        "id: 8\n"
            + "event: reputation_snapshot\n"
            + "data: "
            + EVENT_JSON
            + "\n\n"
            + "event: lagged\n"
            + "data: 2\n\n");
    final List<String> snapshots = new ArrayList<>();
    final List<String> lagged = new ArrayList<>();
    final CountDownLatch closed = new CountDownLatch(1);

    final ToriiEventStream stream =
        client.openEventStream(
            canonicalAuth.auth,
            new EventStreamListener() {
              @Override
              public void onSnapshot(final SnapshotEventV1 event) {
                snapshots.add(event.sequence());
              }

              @Override
              public void onLagged(final String skipped) {
                lagged.add(skipped);
              }

              @Override
              public void onClosed() {
                closed.countDown();
              }
            },
            "7",
            1);

    stream.completion().get(5, TimeUnit.SECONDS);
    assertTrue(closed.await(5, TimeUnit.SECONDS));
    assertEquals(Collections.singletonList("8"), snapshots);
    assertEquals(Collections.singletonList("2"), lagged);
    assertEquals(1, executor.streamRequests.size());
    final TransportRequest request = executor.streamRequests.get(0);
    assertExactSignedGet(
        request,
        "/v1/sorafs/reputation/events/stream",
        "since=7&limit=1",
        canonicalAuth);
    for (final String header : request.headers().keySet()) {
      assertFalse("Last-Event-ID".equalsIgnoreCase(header));
    }
    assertEquals(Collections.singletonList("text/event-stream"), request.headers().get("Accept"));
  }

  @Test
  public void sseRejectsBufferedFallbackBeforeSigningOrRequest() {
    final AtomicInteger requests = new AtomicInteger();
    final TransportExecutor bufferedOnly =
        request -> {
          requests.incrementAndGet();
          return CompletableFuture.completedFuture(
              TransportResponse.builder().setStatusCode(200).build());
        };
    final SorafsReputationClient client = new SorafsReputationClient(BASE_URI, bufferedOnly);
    final RecordedAuth canonicalAuth = auth("no-fallback");

    assertThrows(
        IllegalArgumentException.class,
        () ->
            client.openEventStream(
                canonicalAuth.auth,
                new EventStreamListener() {
                  @Override
                  public void onSnapshot(final SnapshotEventV1 event) {}
                }));
    assertEquals(0, requests.get());
    assertEquals(0, canonicalAuth.signCalls.get());
  }

  @Test
  public void parserEnforcesOrderingProofAndEventChainInvariants() {
    final String secondProvider =
        PROVIDER_OBJECT.replace("provider:alpha", "provider:aardvark");
    final String unsortedProviders =
        SNAPSHOT_JSON
            .replace(
                "\"providers\":[" + PROVIDER_OBJECT + "]",
                "\"providers\":[" + PROVIDER_OBJECT + "," + secondProvider + "]")
            .replace("\"provider_count\":1", "\"provider_count\":2")
            .replace("\"returned_provider_count\":1", "\"returned_provider_count\":2");
    assertThrows(
        IllegalStateException.class,
        () ->
            SorafsReputationJsonParser.parseSnapshot(
                unsortedProviders.getBytes(StandardCharsets.UTF_8)));

    final String reversedFlags =
        SNAPSHOT_JSON.replace(
            "\"degradation_flags\":["
                + "{\"flag\":\"reserve_warning\",\"value\":null},"
                + "{\"flag\":\"low_score\",\"value\":null}]",
            "\"degradation_flags\":["
                + "{\"flag\":\"low_score\",\"value\":null},"
                + "{\"flag\":\"reserve_warning\",\"value\":null}]");
    assertThrows(
        IllegalStateException.class,
        () ->
            SorafsReputationJsonParser.parseSnapshot(
                reversedFlags.getBytes(StandardCharsets.UTF_8)));

    final String underfilledProviderPrefix =
        SNAPSHOT_JSON
            .replace("\"provider_count\":1", "\"provider_count\":2")
            .replace("\"limit\":500", "\"limit\":2")
            .replace("\"truncated_providers\":false", "\"truncated_providers\":true");
    assertThrows(
        IllegalStateException.class,
        () ->
            SorafsReputationJsonParser.parseSnapshot(
                underfilledProviderPrefix.getBytes(StandardCharsets.UTF_8)));

    final String selfPredecessor =
        SNAPSHOT_JSON.replace(
            "\"previous_snapshot_id_hex\":null",
            "\"previous_snapshot_id_hex\":\"" + SNAPSHOT_ID + "\"");
    assertThrows(
        IllegalStateException.class,
        () ->
            SorafsReputationJsonParser.parseSnapshot(
                selfPredecessor.getBytes(StandardCharsets.UTF_8)));

    final String wrongProofDepth =
        PROVIDER_JSON.replace(
            "\"siblings_hex\":[]", "\"siblings_hex\":[\"" + MERKLE_ROOT + "\"]");
    assertThrows(
        IllegalStateException.class,
        () ->
            SorafsReputationJsonParser.parseProviderResponse(
                wrongProofDepth.getBytes(StandardCharsets.UTF_8)));

    final String retainedFirst =
        EVENT_JSON
            .replace("\"sequence\":8", "\"sequence\":9")
            .replace(
                "\"generated_at_unix\":18446744073709551615", "\"generated_at_unix\":9");
    final String retainedPage =
        "{\"since\":7,\"limit\":2,\"count\":1,\"next_since\":9,\"events\":["
            + retainedFirst
            + "]}";
    assertEquals(
        "9",
        SorafsReputationJsonParser.parseEventPage(
                retainedPage.getBytes(StandardCharsets.UTF_8))
            .nextSince());

    final String retainedSecond =
        EVENT_JSON
            .replace("\"sequence\":8", "\"sequence\":11")
            .replace(SNAPSHOT_ID, NEXT_SNAPSHOT_ID)
            .replace(
                "\"generated_at_unix\":18446744073709551615", "\"generated_at_unix\":10")
            .replace(
                "\"previous_snapshot_id_hex\":null",
                "\"previous_snapshot_id_hex\":\"" + SNAPSHOT_ID + "\"");
    final String overLimitSecond =
        retainedSecond.replace("\"sequence\":11", "\"sequence\":10");
    final String overLimitPage =
        "{\"since\":7,\"limit\":1,\"count\":2,\"next_since\":10,\"events\":["
            + retainedFirst
            + ","
            + overLimitSecond
            + "]}";
    assertThrows(
        IllegalStateException.class,
        () ->
            SorafsReputationJsonParser.parseEventPage(
                overLimitPage.getBytes(StandardCharsets.UTF_8)));
    final String internalGap =
        "{\"since\":7,\"limit\":2,\"count\":2,\"next_since\":11,\"events\":["
            + retainedFirst
            + ","
            + retainedSecond
            + "]}";
    assertThrows(
        IllegalStateException.class,
        () ->
            SorafsReputationJsonParser.parseEventPage(
                internalGap.getBytes(StandardCharsets.UTF_8)));

    final String unlinkedSecond =
        retainedSecond
            .replace("\"sequence\":11", "\"sequence\":10")
            .replace(
                "\"previous_snapshot_id_hex\":\"" + SNAPSHOT_ID + "\"",
                "\"previous_snapshot_id_hex\":null");
    final String unlinkedPage =
        "{\"since\":7,\"limit\":2,\"count\":2,\"next_since\":10,\"events\":["
            + retainedFirst
            + ","
            + unlinkedSecond
            + "]}";
    assertThrows(
        IllegalStateException.class,
        () ->
            SorafsReputationJsonParser.parseEventPage(
                unlinkedPage.getBytes(StandardCharsets.UTF_8)));

    final String nonIncreasingSecond =
        retainedSecond
            .replace("\"sequence\":11", "\"sequence\":10")
            .replace("\"generated_at_unix\":10", "\"generated_at_unix\":9");
    final String nonIncreasingPage =
        "{\"since\":7,\"limit\":2,\"count\":2,\"next_since\":10,\"events\":["
            + retainedFirst
            + ","
            + nonIncreasingSecond
            + "]}";
    assertThrows(
        IllegalStateException.class,
        () ->
            SorafsReputationJsonParser.parseEventPage(
                nonIncreasingPage.getBytes(StandardCharsets.UTF_8)));

    final String selfLinkedEvent =
        EVENT_JSON.replace(
            "\"previous_snapshot_id_hex\":null",
            "\"previous_snapshot_id_hex\":\"" + SNAPSHOT_ID + "\"");
    assertThrows(
        IllegalStateException.class,
        () -> SorafsReputationJsonParser.parseEventJson(selfLinkedEvent));
    assertThrows(
        IllegalStateException.class,
        () -> SorafsReputationJsonParser.parseEventJson(EVENT_JSON.replace(",", ", ")));
  }

  private static RecordedAuth auth(final String nonceSuffix) {
    final AtomicReference<byte[]> signedMessage = new AtomicReference<>();
    final AtomicInteger signCalls = new AtomicInteger();
    final ToriiCanonicalRequestAuth auth =
        new ToriiCanonicalRequestAuth(
            "reputation-reader@sora",
            message -> {
              signCalls.incrementAndGet();
              signedMessage.set(message.clone());
              return SIGNATURE.clone();
            },
            TIMESTAMP_MS,
            "nonce-" + nonceSuffix);
    return new RecordedAuth(auth, signedMessage, signCalls);
  }

  private static void assertExactSignedGet(
      final TransportRequest request,
      final String path,
      final String query,
      final RecordedAuth recordedAuth) {
    assertEquals("GET", request.method());
    assertEquals(path, request.uri().getRawPath());
    assertEquals(query, request.uri().getRawQuery());
    assertArrayEquals(new byte[0], request.body());
    assertArrayEquals(
        CanonicalRequestSigner.canonicalRequestSignatureMessage(
            "GET",
            request.uri(),
            new byte[0],
            TIMESTAMP_MS,
            recordedAuth.auth.nonce()),
        recordedAuth.signedMessage.get());
    assertEquals(1, recordedAuth.signCalls.get());
    assertEquals(
        Collections.singletonList(Base64.getEncoder().encodeToString(SIGNATURE)),
        request.headers().get(CanonicalRequestSigner.HEADER_SIGNATURE));
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder builder = new StringBuilder(value.length() * count);
    for (int index = 0; index < count; index++) {
      builder.append(value);
    }
    return builder.toString();
  }

  private static final class RecordedAuth {
    private final ToriiCanonicalRequestAuth auth;
    private final AtomicReference<byte[]> signedMessage;
    private final AtomicInteger signCalls;

    private RecordedAuth(
        final ToriiCanonicalRequestAuth auth,
        final AtomicReference<byte[]> signedMessage,
        final AtomicInteger signCalls) {
      this.auth = auth;
      this.signedMessage = signedMessage;
      this.signCalls = signCalls;
    }
  }

  private static final class RecordingStreamingExecutor implements StreamingTransportExecutor {
    private final List<TransportRequest> requests = new ArrayList<>();
    private final List<TransportRequest> streamRequests = new ArrayList<>();
    private final ArrayDeque<TransportResponse> responses = new ArrayDeque<>();
    private final ArrayDeque<TransportStreamResponse> streams = new ArrayDeque<>();

    private void enqueueJson(final String json) {
      responses.add(
          TransportResponse.builder()
              .setStatusCode(200)
              .setBody(json.getBytes(StandardCharsets.UTF_8))
              .build());
    }

    private void enqueueStatus(final int status) {
      responses.add(TransportResponse.builder().setStatusCode(status).build());
    }

    private void enqueueStream(final String body) {
      streams.add(
          new TransportStreamResponse(
              200,
              new ByteArrayInputStream(body.getBytes(StandardCharsets.UTF_8)),
              "",
              Collections.emptyMap(),
              null));
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      requests.add(request);
      return CompletableFuture.completedFuture(responses.removeFirst());
    }

    @Override
    public CompletableFuture<TransportStreamResponse> openStream(
        final TransportRequest request) {
      streamRequests.add(request);
      return CompletableFuture.completedFuture(streams.removeFirst());
    }
  }
}
