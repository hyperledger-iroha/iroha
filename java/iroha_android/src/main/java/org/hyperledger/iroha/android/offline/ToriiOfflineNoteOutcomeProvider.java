package org.hyperledger.iroha.android.offline;

import java.io.UnsupportedEncodingException;
import java.net.URI;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.ClientObserver;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.client.PlatformHttpTransportExecutor;
import org.hyperledger.iroha.android.client.TransportSecurity;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/** Torii explorer-backed provider for Offline Note wallet reconciliation outcomes. */
public final class ToriiOfflineNoteOutcomeProvider implements OfflineNoteOutcomeProvider {
  private static final String EXPLORER_INSTRUCTIONS_PATH = "/v1/explorer/instructions";

  private final HttpTransportExecutor executor;
  private final URI baseUri;
  private final Duration timeout;
  private final Map<String, String> defaultHeaders;
  private final List<ClientObserver> observers;
  private final int perPage;

  public ToriiOfflineNoteOutcomeProvider() {
    this(
        PlatformHttpTransportExecutor.createDefault(),
        URI.create("http://localhost:8080"),
        Duration.ofSeconds(15),
        Map.of(),
        List.of(),
        100);
  }

  public ToriiOfflineNoteOutcomeProvider(
      final HttpTransportExecutor executor,
      final URI baseUri,
      final Duration timeout,
      final Map<String, String> defaultHeaders,
      final List<ClientObserver> observers,
      final int perPage) {
    this.executor = Objects.requireNonNull(executor, "executor");
    this.baseUri = Objects.requireNonNull(baseUri, "baseUri");
    this.timeout = timeout;
    this.defaultHeaders = new LinkedHashMap<>(defaultHeaders == null ? Map.of() : defaultHeaders);
    this.observers = List.copyOf(observers == null ? List.of() : observers);
    this.perPage = perPage;
  }

  @Override
  public CompletableFuture<List<OfflineNoteExplorerInstructionOutcome>> listOutcomes() {
    final CompletableFuture<List<OfflineNoteExplorerInstructionOutcome>> audit =
        fetchKind(OfflineNoteOutcomeIndex.KIND_AUDIT);
    final CompletableFuture<List<OfflineNoteExplorerInstructionOutcome>> redeem =
        fetchKind(OfflineNoteOutcomeIndex.KIND_REDEEM);
    return CompletableFuture.allOf(audit, redeem)
        .thenApply(
            ignored -> {
              final List<OfflineNoteExplorerInstructionOutcome> outcomes = new ArrayList<>();
              outcomes.addAll(audit.join());
              outcomes.addAll(redeem.join());
              return outcomes;
            });
  }

  private CompletableFuture<List<OfflineNoteExplorerInstructionOutcome>> fetchKind(
      final String kind) {
    final Map<String, String> params = new LinkedHashMap<>();
    params.put("kind", kind);
    params.put("per_page", Integer.toString(perPage));
    final TransportRequest request = buildGetRequest(EXPLORER_INSTRUCTIONS_PATH, params);
    notifyRequest(request);
    final CompletableFuture<List<OfflineNoteExplorerInstructionOutcome>> future =
        new CompletableFuture<>();
    executor.execute(request)
        .whenComplete(
            (response, throwable) -> {
              if (throwable != null) {
                final OfflineToriiException error =
                    new OfflineToriiException(
                        "Offline Note outcome lookup failed: "
                            + (throwable.getMessage() == null
                                ? throwable.getClass().getSimpleName()
                                : throwable.getMessage()),
                        throwable);
                notifyFailure(request, error);
                future.completeExceptionally(error);
                return;
              }
              handleResponse(request, response, future);
            });
    return future;
  }

  private void handleResponse(
      final TransportRequest request,
      final TransportResponse response,
      final CompletableFuture<List<OfflineNoteExplorerInstructionOutcome>> future) {
    final String rejectCode = rejectCode(response.headers());
    final ClientResponse clientResponse =
        new ClientResponse(response.statusCode(), response.body(), response.message(), null, rejectCode);
    if (response.statusCode() < 200 || response.statusCode() >= 300) {
      final OfflineToriiException error =
          new OfflineToriiException(
              "Offline Note outcome lookup failed with HTTP " + response.statusCode(),
              response.statusCode(),
              rejectCode,
              bodyPreview(response.body()));
      notifyFailure(request, error);
      future.completeExceptionally(error);
      return;
    }
    try {
      final List<OfflineNoteExplorerInstructionOutcome> parsed =
          parseExplorerOutcomes(response.body());
      notifyResponse(request, clientResponse);
      future.complete(parsed);
    } catch (final RuntimeException ex) {
      final OfflineToriiException error =
          new OfflineToriiException(
              "Failed to parse Offline Note explorer outcomes",
              ex,
              response.statusCode(),
              rejectCode,
              bodyPreview(response.body()));
      notifyFailure(request, error);
      future.completeExceptionally(error);
    }
  }

  private TransportRequest buildGetRequest(final String path, final Map<String, String> params) {
    final URI target = appendQuery(resolvePath(path), params);
    final Map<String, String> headers = mergeHeaders();
    TransportSecurity.requireHttpRequestAllowed(
        "ToriiOfflineNoteOutcomeProvider", baseUri, target, headers, null);
    final TransportRequest.Builder builder =
        TransportRequest.builder().setUri(target).setMethod("GET").setTimeout(timeout);
    for (final Map.Entry<String, String> entry : headers.entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    return builder.build();
  }

  private URI resolvePath(final String path) {
    final String normalized = path.startsWith("/") ? path.substring(1) : path;
    final String base = baseUri.toString();
    return URI.create(base.endsWith("/") ? base + normalized : base + "/" + normalized);
  }

  private URI appendQuery(final URI uri, final Map<String, String> params) {
    if (params.isEmpty()) {
      return uri;
    }
    final StringBuilder query = new StringBuilder();
    for (final Map.Entry<String, String> entry : params.entrySet()) {
      if (query.length() > 0) {
        query.append('&');
      }
      query.append(urlEncode(entry.getKey())).append('=').append(urlEncode(entry.getValue()));
    }
    final String base = uri.toString();
    return URI.create(base + (base.contains("?") ? "&" : "?") + query);
  }

  private Map<String, String> mergeHeaders() {
    final Map<String, String> headers = new LinkedHashMap<>(defaultHeaders);
    final String existing = findHeader(headers, "Accept");
    headers.put(existing == null ? "Accept" : existing, "application/json");
    return headers;
  }

  private List<OfflineNoteExplorerInstructionOutcome> parseExplorerOutcomes(
      final byte[] payload) {
    final Object parsed = JsonParser.parse(new String(payload, StandardCharsets.UTF_8));
    final Map<String, Object> root = requireObject(parsed, "explorer response");
    final Object rawItems = root.get("items");
    if (!(rawItems instanceof List<?> items)) {
      throw new IllegalArgumentException("items must be an array");
    }
    final List<OfflineNoteExplorerInstructionOutcome> outcomes = new ArrayList<>();
    for (final Object rawItem : items) {
      final Map<String, Object> item = requireObject(rawItem, "instruction item");
      final Map<String, Object> box = requireObject(
          item.containsKey("r#box") ? item.get("r#box") : item.get("box"), "instruction box");
      final Object rawEncoded = box.get("encoded");
      final String encoded = rawEncoded instanceof String
          ? (String) rawEncoded
          : requireNestedEncoded(box);
      outcomes.add(
          new OfflineNoteExplorerInstructionOutcome(
              requiredString(item, "kind"),
              requiredString(item, "transaction_status"),
              item.get("transaction_hash") instanceof String ? (String) item.get("transaction_hash") : null,
              hexBytes(encoded, "encoded")));
    }
    return outcomes;
  }

  private String requireNestedEncoded(final Map<String, Object> box) {
    final Map<String, Object> json = requireObject(box.get("json"), "instruction box json");
    final Object encoded = json.get("encoded");
    if (!(encoded instanceof String value) || value.isBlank()) {
      throw new IllegalArgumentException("instruction box encoded payload missing");
    }
    return value;
  }

  @SuppressWarnings("unchecked")
  private Map<String, Object> requireObject(final Object value, final String path) {
    if (!(value instanceof Map<?, ?>)) {
      throw new IllegalArgumentException(path + " must be an object");
    }
    return (Map<String, Object>) value;
  }

  private String requiredString(final Map<String, Object> value, final String field) {
    final Object raw = value.get(field);
    if (!(raw instanceof String text) || text.isBlank()) {
      throw new IllegalArgumentException(field + " must be a non-empty string");
    }
    return text;
  }

  private byte[] hexBytes(final String value, final String field) {
    final String trimmed = value.trim();
    final String withoutPrefix =
        trimmed.regionMatches(true, 0, "0x", 0, 2) ? trimmed.substring(2) : trimmed;
    final String normalized = withoutPrefix.toLowerCase(Locale.ROOT);
    if ((normalized.length() & 1) != 0) {
      throw new IllegalArgumentException(field + " must have an even hex length");
    }
    final byte[] out = new byte[normalized.length() / 2];
    for (int index = 0; index < out.length; index++) {
      final int hi = Character.digit(normalized.charAt(index * 2), 16);
      final int lo = Character.digit(normalized.charAt(index * 2 + 1), 16);
      if (hi < 0 || lo < 0) {
        throw new IllegalArgumentException(field + " must be hex");
      }
      out[index] = (byte) ((hi << 4) | lo);
    }
    return out;
  }

  private String rejectCode(final Map<String, List<String>> headers) {
    for (final Map.Entry<String, List<String>> entry : headers.entrySet()) {
      if (!"x-iroha-reject-code".equalsIgnoreCase(entry.getKey())) {
        continue;
      }
      for (final String value : entry.getValue()) {
        if (value != null && !value.isBlank()) {
          return value.trim();
        }
      }
    }
    return null;
  }

  private String bodyPreview(final byte[] body) {
    if (body == null || body.length == 0) {
      return null;
    }
    final String text = new String(body, StandardCharsets.UTF_8).trim();
    return text.isEmpty() ? null : text.substring(0, Math.min(512, text.length()));
  }

  private void notifyRequest(final TransportRequest request) {
    for (final ClientObserver observer : observers) {
      observer.onRequest(request);
    }
  }

  private void notifyResponse(final TransportRequest request, final ClientResponse response) {
    for (final ClientObserver observer : observers) {
      observer.onResponse(request, response);
    }
  }

  private void notifyFailure(final TransportRequest request, final Throwable error) {
    for (final ClientObserver observer : observers) {
      observer.onFailure(request, error);
    }
  }

  private String findHeader(final Map<String, String> headers, final String name) {
    for (final String key : headers.keySet()) {
      if (key.equalsIgnoreCase(name)) {
        return key;
      }
    }
    return null;
  }

  private String urlEncode(final String value) {
    try {
      return URLEncoder.encode(value, StandardCharsets.UTF_8.name());
    } catch (final UnsupportedEncodingException ex) {
      throw new IllegalStateException(ex);
    }
  }
}
