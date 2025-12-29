package org.hyperledger.iroha.android.telemetry;

import java.net.URI;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.atomic.AtomicBoolean;
import org.hyperledger.iroha.android.client.ClientObserver;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.transport.TransportRequest;

/**
 * {@link ClientObserver} implementation that hashes authorities using {@link TelemetryOptions} and
 * forwards structured records to a {@link TelemetrySink}.
 */
public final class TelemetryObserver implements ClientObserver {

  private static final String REDACTION_FAILURE_SIGNAL = "android.telemetry.redaction.failure";
  private static final String SALT_VERSION_SIGNAL = "android.telemetry.redaction.salt_version";
  private static final String TORII_REQUEST_SIGNAL = "android.torii.http.request";

  private final TelemetryOptions options;
  private final TelemetrySink sink;
  private final AtomicBoolean saltVersionEmitted = new AtomicBoolean(false);
  private final Map<TransportRequest, RequestContext> inflightRequests =
      Collections.synchronizedMap(new IdentityHashMap<>());

  public TelemetryObserver(final TelemetryOptions options, final TelemetrySink sink) {
    this.options = Objects.requireNonNull(options, "options");
    this.sink = Objects.requireNonNull(sink, "sink");
  }

  @Override
  public void onRequest(final TransportRequest request) {
    if (!options.enabled()) {
      return;
    }
    final TelemetryRecord record = buildRecord(request);
    trackRequest(request, record);
    sink.onRequest(record);
  }

  @Override
  public void onResponse(final TransportRequest request, final ClientResponse response) {
    if (!options.enabled()) {
      return;
    }
    final TelemetryRecord record = buildRecordWithOutcome(request, response, null);
    sink.onResponse(record, response);
  }

  @Override
  public void onFailure(final TransportRequest request, final Throwable error) {
    if (!options.enabled()) {
      return;
    }
    final TelemetryRecord record = buildRecordWithOutcome(request, null, error);
    sink.onFailure(record, error);
  }

  private void trackRequest(final TransportRequest request, final TelemetryRecord record) {
    inflightRequests.put(request, new RequestContext(record, System.nanoTime()));
  }

  private TelemetryRecord buildRecordWithOutcome(
      final TransportRequest request,
      final ClientResponse response,
      final Throwable error) {
    final RequestContext context = inflightRequests.remove(request);
    final TelemetryRecord base = context == null ? buildRecord(request) : context.record();
    final Long latencyMillis =
        context == null ? null : (System.nanoTime() - context.startNano()) / 1_000_000L;
    final Integer statusCode = response == null ? null : response.statusCode();
    final String errorKind = error == null ? null : error.getClass().getSimpleName();
    return base.withOutcome(latencyMillis, statusCode, errorKind);
  }

  private TelemetryRecord buildRecord(final TransportRequest request) {
    final URI uri = request.uri();
    final String authority = resolveAuthority(uri, request);
    final String route = uri == null ? "" : nullToEmpty(uri.getRawPath());
    final String method = nullToEmpty(request.method());
    final boolean redactionEnabled = options.redaction().enabled();
    final String saltVersion = redactionEnabled ? options.redaction().saltVersion() : "none";
    if (redactionEnabled) {
      emitSaltVersionGaugeOnce(saltVersion);
    }
    final String authorityHash =
        redactionEnabled ? hashAuthorityOrRecordFailure(authority) : null;
    return new TelemetryRecord(authorityHash, saltVersion, route, method);
  }

  private String hashAuthorityOrRecordFailure(final String authority) {
    final String normalised = authority == null ? "" : authority.trim();
    if (normalised.isEmpty()) {
      emitRedactionFailure("blank_authority");
      return null;
    }
    return options.redaction().hashAuthority(normalised).orElseGet(() -> {
      emitRedactionFailure("hash_failed");
      return null;
    });
  }

  private void emitRedactionFailure(final String reason) {
    sink.emitSignal(
        REDACTION_FAILURE_SIGNAL,
        Map.of(
            "signal_id", TORII_REQUEST_SIGNAL,
            "reason", reason));
  }

  private void emitSaltVersionGaugeOnce(final String saltVersion) {
    if (!saltVersionEmitted.compareAndSet(false, true)) {
      return;
    }
    sink.emitSignal(
        SALT_VERSION_SIGNAL,
        Map.of(
            "salt_epoch", saltVersion,
            "rotation_id", options.redaction().rotationId()));
  }

  private static String resolveAuthority(final URI uri, final TransportRequest request) {
    if (uri != null && uri.getAuthority() != null) {
      return uri.getAuthority();
    }
    final List<String> host = request.headers().get("Host");
    return host == null || host.isEmpty() ? "" : host.get(0);
  }

  private static String nullToEmpty(final String value) {
    return value == null ? "" : value;
  }

  private static final class RequestContext {
    private final TelemetryRecord record;
    private final long startNano;

    private RequestContext(final TelemetryRecord record, final long startNano) {
      this.record = record;
      this.startNano = startNano;
    }

    private TelemetryRecord record() {
      return record;
    }

    private long startNano() {
      return startNano;
    }
  }
}
