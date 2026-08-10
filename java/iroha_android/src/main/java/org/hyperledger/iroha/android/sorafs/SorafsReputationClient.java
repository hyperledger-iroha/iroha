package org.hyperledger.iroha.android.sorafs;

import java.math.BigInteger;
import java.net.URI;
import java.time.Duration;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.CanonicalRequestSigner;
import org.hyperledger.iroha.android.client.PlatformHttpTransportExecutor;
import org.hyperledger.iroha.android.client.ToriiCanonicalRequestAuth;
import org.hyperledger.iroha.android.client.TransportSecurity;
import org.hyperledger.iroha.android.client.stream.ServerSentEvent;
import org.hyperledger.iroha.android.client.stream.ToriiEventStream;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamClient;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamListener;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamOptions;
import org.hyperledger.iroha.android.client.transport.StreamingTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.sorafs.SorafsReputationModels.EventStreamListener;
import org.hyperledger.iroha.android.sorafs.SorafsReputationModels.EventsResponseV1;
import org.hyperledger.iroha.android.sorafs.SorafsReputationModels.ProviderResponseV1;
import org.hyperledger.iroha.android.sorafs.SorafsReputationModels.SnapshotEventV1;
import org.hyperledger.iroha.android.sorafs.SorafsReputationModels.SnapshotSummaryV1;
import org.hyperledger.iroha.android.sorafs.SorafsReputationModels.WeightsResponseV1;

/**
 * Closed authenticated client for committed SoraFS reputation V1 projections.
 *
 * <p>Every operation performs exactly one GET signed over the final path, canonical query, and
 * empty body. Raw authentication headers, witness authentication, resume cursors, and automatic
 * retries are intentionally absent.
 */
public final class SorafsReputationClient {

  private static final String LATEST_PATH = "/v1/sorafs/reputation/latest";
  private static final String PROVIDER_PATH_PREFIX = "/v1/sorafs/reputation/providers/";
  private static final String SNAPSHOT_PATH_PREFIX = "/v1/sorafs/reputation/snapshots/";
  private static final String WEIGHTS_PATH = "/v1/sorafs/reputation/weights";
  private static final String EVENTS_PATH = "/v1/sorafs/reputation/events";
  private static final String EVENTS_STREAM_PATH = "/v1/sorafs/reputation/events/stream";
  private static final long MAX_RESPONSE_BYTES = 4_194_304L;
  private static final BigInteger U64_MAX = new BigInteger("18446744073709551615");
  private static final byte[] EMPTY_BODY = new byte[0];

  private final URI baseUri;
  private final NetworkId networkId;
  private final TransportExecutor transport;
  private final Duration timeout;

  /** Creates a client backed by the canonical platform transport. */
  public SorafsReputationClient(final URI baseUri, final NetworkId networkId) {
    this(baseUri, networkId, PlatformHttpTransportExecutor.createDefault(), null);
  }

  /** Creates a client with executor defaults for request timeouts. */
  public SorafsReputationClient(
      final URI baseUri, final NetworkId networkId, final TransportExecutor transport) {
    this(baseUri, networkId, transport, null);
  }

  /** Creates a client with an optional request timeout. */
  public SorafsReputationClient(
      final URI baseUri,
      final NetworkId networkId,
      final TransportExecutor transport,
      final Duration timeout) {
    if (baseUri == null
        || !baseUri.isAbsolute()
        || baseUri.getRawQuery() != null
        || baseUri.getRawFragment() != null) {
      throw new IllegalArgumentException(
          "baseUri must be an absolute URI without query or fragment");
    }
    if (transport == null) {
      throw new IllegalArgumentException("transport is required");
    }
    if (networkId == null) {
      throw new IllegalArgumentException("networkId is required");
    }
    if (timeout != null && timeout.isNegative()) {
      throw new IllegalArgumentException("timeout must be non-negative");
    }
    this.baseUri = baseUri;
    this.networkId = networkId;
    this.transport = transport;
    this.timeout = timeout;
  }

  /** Returns the configured Torii base URI. */
  public URI baseUri() {
    return baseUri;
  }

  /** Returns the optional per-request timeout. */
  public Duration timeout() {
    return timeout;
  }

  /** Fetches the latest committed snapshot, or an empty result when no snapshot exists. */
  public CompletableFuture<Optional<SnapshotSummaryV1>> getLatest(
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return getLatest(canonicalAuth, null);
  }

  /** Fetches a bounded prefix of the latest committed snapshot. */
  public CompletableFuture<Optional<SnapshotSummaryV1>> getLatest(
      final ToriiCanonicalRequestAuth canonicalAuth, final Integer limit) {
    return fetchOptional(
        LATEST_PATH,
        snapshotQuery(limit),
        canonicalAuth,
        payload -> {
          final SnapshotSummaryV1 snapshot =
              SorafsReputationJsonParser.parseSnapshot(payload);
          if (limit != null && snapshot.limit() != limit.intValue()) {
            throw new IllegalStateException(
                "SoraFS reputation latest response limit does not match the request");
          }
          return snapshot;
        },
        "SoraFS reputation latest");
  }

  /** Fetches one provider and its proof from the latest committed snapshot. */
  public CompletableFuture<Optional<ProviderResponseV1>> getProvider(
      final String providerId, final ToriiCanonicalRequestAuth canonicalAuth) {
    final String canonicalProvider = normalizeProviderId(providerId);
    return fetchOptional(
        PROVIDER_PATH_PREFIX + canonicalProvider,
        Collections.emptyMap(),
        canonicalAuth,
        payload -> {
          final ProviderResponseV1 response =
              SorafsReputationJsonParser.parseProviderResponse(payload);
          if (!canonicalProvider.equals(response.provider().providerId())) {
            throw new IllegalStateException(
                "SoraFS reputation provider response does not match the requested provider");
          }
          return response;
        },
        "SoraFS reputation provider");
  }

  /** Fetches one immutable snapshot by its exact nonzero lowercase identifier. */
  public CompletableFuture<Optional<SnapshotSummaryV1>> getSnapshot(
      final String snapshotIdHex, final ToriiCanonicalRequestAuth canonicalAuth) {
    return getSnapshot(snapshotIdHex, canonicalAuth, null);
  }

  /** Fetches a bounded prefix of one immutable snapshot. */
  public CompletableFuture<Optional<SnapshotSummaryV1>> getSnapshot(
      final String snapshotIdHex,
      final ToriiCanonicalRequestAuth canonicalAuth,
      final Integer limit) {
    final String canonicalSnapshot = normalizeSnapshotId(snapshotIdHex);
    return fetchOptional(
        SNAPSHOT_PATH_PREFIX + canonicalSnapshot,
        snapshotQuery(limit),
        canonicalAuth,
        payload -> {
          final SnapshotSummaryV1 snapshot =
              SorafsReputationJsonParser.parseSnapshot(payload);
          if (!canonicalSnapshot.equals(snapshot.snapshotIdHex())) {
            throw new IllegalStateException(
                "SoraFS reputation snapshot response does not match the requested snapshot");
          }
          if (limit != null && snapshot.limit() != limit.intValue()) {
            throw new IllegalStateException(
                "SoraFS reputation snapshot response limit does not match the request");
          }
          return snapshot;
        },
        "SoraFS reputation snapshot");
  }

  /** Fetches active scoring weights from the latest committed snapshot. */
  public CompletableFuture<WeightsResponseV1> getWeights(
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return fetchRequired(
        WEIGHTS_PATH,
        Collections.emptyMap(),
        canonicalAuth,
        SorafsReputationJsonParser::parseWeightsResponse,
        "SoraFS reputation weights");
  }

  /** Fetches the default bounded finalized event page. */
  public CompletableFuture<EventsResponseV1> listEvents(
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return listEvents(canonicalAuth, null, null);
  }

  /** Fetches a bounded finalized event page after an optional canonical u64 cursor. */
  public CompletableFuture<EventsResponseV1> listEvents(
      final ToriiCanonicalRequestAuth canonicalAuth,
      final String since,
      final Integer limit) {
    final Map<String, String> query = eventQuery(since, limit);
    return fetchRequired(
        EVENTS_PATH,
        query,
        canonicalAuth,
        payload -> {
          final EventsResponseV1 page =
              SorafsReputationJsonParser.parseEventPage(payload);
          if (!java.util.Objects.equals(page.since(), query.get("since"))) {
            throw new IllegalStateException(
                "SoraFS reputation events response since does not match the request");
          }
          if (limit != null && page.limit() != limit.intValue()) {
            throw new IllegalStateException(
                "SoraFS reputation events response limit does not match the request");
          }
          return page;
        },
        "SoraFS reputation events");
  }

  /** Opens the authenticated event stream without resume or reconnection support. */
  public ToriiEventStream openEventStream(
      final ToriiCanonicalRequestAuth canonicalAuth, final EventStreamListener listener) {
    return openEventStream(canonicalAuth, listener, null, null);
  }

  /**
   * Opens the authenticated event stream with an optional initial cursor and bounded batch size.
   *
   * <p>The configured executor must surface a true stream. This method performs one transport
   * attempt and never reuses the caller's fixed nonce for retry or reconnection.
   */
  public ToriiEventStream openEventStream(
      final ToriiCanonicalRequestAuth canonicalAuth,
      final EventStreamListener listener,
      final String since,
      final Integer limit) {
    if (!(transport instanceof StreamingTransportExecutor)) {
      throw new IllegalArgumentException(
          "SoraFS reputation SSE requires a StreamingTransportExecutor; buffered fallback is unsupported");
    }
    if (listener == null) {
      throw new IllegalArgumentException("listener is required");
    }
    final Map<String, String> query = eventQuery(since, limit);
    final URI target = buildTarget(EVENTS_STREAM_PATH, query);
    final Map<String, String> headers = canonicalHeaders(target, canonicalAuth);
    final ToriiEventStreamOptions.Builder options =
        ToriiEventStreamOptions.builder().queryParameters(query).headers(headers);
    if (timeout != null) {
      options.setTimeout(timeout);
    }
    final ToriiEventStreamClient streamClient =
        ToriiEventStreamClient.builder()
            .setBaseUri(baseUri)
            .setTransportExecutor(transport)
            .build();
    return streamClient.openSseStream(
        EVENTS_STREAM_PATH,
        options.build(),
        new ToriiEventStreamListener() {
          @Override
          public void onOpen() {
            listener.onOpen();
          }

          @Override
          public void onEvent(final ServerSentEvent event) {
            if ("reputation_snapshot".equals(event.event())) {
              final SnapshotEventV1 parsed =
                  SorafsReputationJsonParser.parseEventJson(event.data());
              final String eventId =
                  normalizeU64(event.id(), "SoraFS reputation SSE event id", false);
              if (!eventId.equals(parsed.sequence())) {
                throw new IllegalStateException(
                    "SoraFS reputation SSE event id must equal data.sequence");
              }
              listener.onSnapshot(parsed);
              return;
            }
            if ("lagged".equals(event.event())) {
              if (event.id() != null) {
                throw new IllegalStateException(
                    "SoraFS reputation lagged SSE frames must not carry an id");
              }
              listener.onLagged(
                  normalizeU64(
                      event.data(), "SoraFS reputation SSE lagged count", false));
              return;
            }
            throw new IllegalStateException(
                "unsupported SoraFS reputation SSE event `" + event.event() + "`");
          }

          @Override
          public void onClosed() {
            listener.onClosed();
          }

          @Override
          public void onError(final Throwable error) {
            listener.onError(error);
          }
        });
  }

  private <T> CompletableFuture<Optional<T>> fetchOptional(
      final String path,
      final Map<String, String> query,
      final ToriiCanonicalRequestAuth canonicalAuth,
      final PayloadParser<T> parser,
      final String context) {
    return execute(path, query, canonicalAuth, parser, context, true)
        .thenApply(Optional::ofNullable);
  }

  private <T> CompletableFuture<T> fetchRequired(
      final String path,
      final Map<String, String> query,
      final ToriiCanonicalRequestAuth canonicalAuth,
      final PayloadParser<T> parser,
      final String context) {
    return execute(path, query, canonicalAuth, parser, context, false);
  }

  private <T> CompletableFuture<T> execute(
      final String path,
      final Map<String, String> query,
      final ToriiCanonicalRequestAuth canonicalAuth,
      final PayloadParser<T> parser,
      final String context,
      final boolean nullableOnNotFound) {
    final URI target = buildTarget(path, query);
    final Map<String, String> headers = canonicalHeaders(target, canonicalAuth);
    TransportSecurity.requireHttpRequestAllowed(
        "SorafsReputationClient", baseUri, target, headers, null);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod("GET")
            .addHeader("Accept", "application/json")
            .setMaximumResponseBytes(Long.valueOf(MAX_RESPONSE_BYTES));
    if (timeout != null) {
      builder.setTimeout(timeout);
    }
    for (final Map.Entry<String, String> header : headers.entrySet()) {
      builder.addHeader(header.getKey(), header.getValue());
    }
    final CompletableFuture<T> future = new CompletableFuture<>();
    transport
        .execute(builder.build())
        .whenComplete(
            (response, throwable) -> {
              if (throwable != null) {
                future.completeExceptionally(
                    new RuntimeException(context + " request failed", unwrapCompletion(throwable)));
                return;
              }
              try {
                if (response == null) {
                  throw new IllegalStateException(context + " returned no transport response");
                }
                if (nullableOnNotFound && response.statusCode() == 404) {
                  future.complete(null);
                  return;
                }
                if (response.statusCode() != 200) {
                  throw new IllegalStateException(
                      context + " request failed with status " + response.statusCode());
                }
                future.complete(parser.parse(response.body()));
              } catch (final RuntimeException error) {
                future.completeExceptionally(error);
              }
            });
    return future;
  }

  private Map<String, String> canonicalHeaders(
      final URI target, final ToriiCanonicalRequestAuth canonicalAuth) {
    if (canonicalAuth == null) {
      throw new IllegalArgumentException("canonicalAuth is required");
    }
    final Long timestampMs = canonicalAuth.timestampMs();
    final String nonce = canonicalAuth.nonce();
    if ((timestampMs == null) != (nonce == null)) {
      throw new IllegalArgumentException("timestampMs and nonce must be provided together");
    }
    if (timestampMs == null) {
      return CanonicalRequestSigner.buildHeaders(
          networkId, "GET", target, EMPTY_BODY, canonicalAuth);
    }
    if (timestampMs.longValue() < 0L) {
      throw new IllegalArgumentException("timestampMs must be non-negative");
    }
    return CanonicalRequestSigner.buildHeaders(
        networkId,
        "GET",
        target,
        EMPTY_BODY,
        canonicalAuth,
        timestampMs.longValue(),
        nonce);
  }

  private URI buildTarget(final String path, final Map<String, String> query) {
    final String base = baseUri.toString();
    final String normalizedPath = path.startsWith("/") ? path.substring(1) : path;
    final String resolved =
        base.endsWith("/") ? base + normalizedPath : base + "/" + normalizedPath;
    if (query.isEmpty()) {
      return URI.create(resolved);
    }
    final StringBuilder target = new StringBuilder(resolved).append('?');
    boolean first = true;
    for (final Map.Entry<String, String> entry : query.entrySet()) {
      if (!first) {
        target.append('&');
      }
      target.append(entry.getKey()).append('=').append(entry.getValue());
      first = false;
    }
    return URI.create(target.toString());
  }

  private static Map<String, String> snapshotQuery(final Integer limit) {
    if (limit == null) {
      return Collections.emptyMap();
    }
    final Map<String, String> query = new LinkedHashMap<>();
    query.put("limit", normalizeLimit(limit.intValue()));
    return query;
  }

  private static Map<String, String> eventQuery(final String since, final Integer limit) {
    final Map<String, String> query = new LinkedHashMap<>();
    if (since != null) {
      query.put("since", normalizeU64(since, "since", true));
    }
    if (limit != null) {
      query.put("limit", normalizeLimit(limit.intValue()));
    }
    return query;
  }

  private static String normalizeLimit(final int limit) {
    if (limit < 1 || limit > 500) {
      throw new IllegalArgumentException("limit must be between 1 and 500");
    }
    return Integer.toString(limit);
  }

  private static String normalizeSnapshotId(final String snapshotIdHex) {
    if (snapshotIdHex == null
        || snapshotIdHex.length() != 32
        || !allLowerHex(snapshotIdHex)) {
      throw new IllegalArgumentException(
          "snapshotIdHex must be exactly 32 lowercase hexadecimal characters");
    }
    if (isAllZero(snapshotIdHex)) {
      throw new IllegalArgumentException("snapshotIdHex must be nonzero");
    }
    return snapshotIdHex;
  }

  private static String normalizeProviderId(final String providerId) {
    if (providerId == null
        || providerId.isEmpty()
        || providerId.length() > 256
        || providerId.equals(".")
        || providerId.equals("..")
        || !allProviderCharacters(providerId)) {
      throw new IllegalArgumentException(
          "providerId must contain 1..256 ASCII characters from [A-Za-z0-9_.:-] and not be a dot segment");
    }
    return providerId;
  }

  private static String normalizeU64(
      final String value, final String context, final boolean allowZero) {
    if (!isCanonicalUnsignedDecimal(value)) {
      throw new IllegalArgumentException(
          context + " must be a canonical unsigned decimal integer");
    }
    final BigInteger parsed = new BigInteger(value);
    if (parsed.compareTo(U64_MAX) > 0 || (!allowZero && parsed.signum() == 0)) {
      throw new IllegalArgumentException(
          context + " must fit " + (allowZero ? "u64" : "positive u64"));
    }
    return value;
  }

  private static boolean isCanonicalUnsignedDecimal(final String value) {
    if ("0".equals(value)) {
      return true;
    }
    if (value == null
        || value.isEmpty()
        || value.charAt(0) < '1'
        || value.charAt(0) > '9') {
      return false;
    }
    for (int index = 1; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (character < '0' || character > '9') {
        return false;
      }
    }
    return true;
  }

  private static boolean allProviderCharacters(final String value) {
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (!((character >= 'A' && character <= 'Z')
          || (character >= 'a' && character <= 'z')
          || (character >= '0' && character <= '9')
          || character == '_'
          || character == '.'
          || character == ':'
          || character == '-')) {
        return false;
      }
    }
    return true;
  }

  private static boolean allLowerHex(final String value) {
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (!((character >= '0' && character <= '9')
          || (character >= 'a' && character <= 'f'))) {
        return false;
      }
    }
    return true;
  }

  private static boolean isAllZero(final String value) {
    for (int index = 0; index < value.length(); index++) {
      if (value.charAt(index) != '0') {
        return false;
      }
    }
    return true;
  }

  private static Throwable unwrapCompletion(final Throwable error) {
    if (error instanceof CompletionException && error.getCause() != null) {
      return error.getCause();
    }
    return error;
  }

  @FunctionalInterface
  private interface PayloadParser<T> {
    T parse(byte[] payload);
  }
}
