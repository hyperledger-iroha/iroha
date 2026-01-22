package org.hyperledger.iroha.android.telemetry;

import java.net.URI;
import java.util.Map;
import java.util.concurrent.atomic.AtomicInteger;
import org.hyperledger.iroha.android.client.ClientConfig;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.RetryPolicy;
import org.hyperledger.iroha.android.client.transport.TransportRequest;

/** Tests covering {@link TelemetryObserver} and the ClientConfig integration hook. */
public final class TelemetryObserverTests {
  private static final String SALT_SIGNAL = "android.telemetry.redaction.salt_version";

  private TelemetryObserverTests() {}

  public static void main(final String[] args) throws Exception {
    emitsHashedAuthorityRecords();
    recordsFailureMetadata();
    clientConfigAddsTelemetryObserver();
    telemetrySinkNotAddedWhenDisabled();
    emitsRedactionFailureEvent();
    emitsSaltVersionAfterConfigReload();
    System.out.println("[IrohaAndroid] Telemetry observer tests passed.");
  }

  private static TelemetryOptions buildOptions() {
    return TelemetryOptions.builder()
        .setTelemetryRedaction(
            TelemetryOptions.Redaction.builder()
                .setSaltHex("01020304")
                .setSaltVersion("2026Q1")
                .setRotationId("rot-7")
                .build())
        .build();
  }

  private static void emitsHashedAuthorityRecords() throws Exception {
    final TelemetryOptions options = buildOptions();
    final RecordingSink sink = new RecordingSink();
    final TelemetryObserver observer = new TelemetryObserver(options, sink);

    final TransportRequest request =
        TransportRequest.builder()
            .setUri(new URI("https://example.com:443/transaction"))
            .setMethod("POST")
            .build();
    observer.onRequest(request);
    observer.onResponse(request, new ClientResponse(200, new byte[0], "OK", "deadbeef"));

    assert sink.requestCount.get() == 1 : "request callbacks should fire once";
    assert sink.responseCount.get() == 1 : "response callbacks should fire once";
    final TelemetryRecord record = sink.lastResponseRecord;
    assert record != null : "response record must be captured";
    assert "76340b3f0817574dc614633eb9b740d0aebffead3ab12790a27dac1a5fcccb67"
        .equals(record.authorityHash())
        : "hashed authority mismatch";
    assert "2026Q1".equals(record.saltVersion()) : "salt version mismatch";
    assert "/transaction".equals(record.route()) : "route mismatch";
    assert "POST".equals(record.method()) : "method mismatch";
    final long latency =
        record.latencyMillis().orElseThrow(() -> new IllegalStateException("missing latency"));
    assert latency >= 0 : "latency must be non-negative";
    final int status =
        record.statusCode().orElseThrow(() -> new IllegalStateException("missing status"));
    assert status == 200 : "status code mismatch";
    assert record.errorKind().isEmpty() : "success case should not expose error kind";
    final RecordingSink.SignalEvent saltEvent =
        sink.findLastSignal("android.telemetry.redaction.salt_version");
    assert saltEvent != null : "salt version signal should be emitted";
    assert "2026Q1".equals(saltEvent.fields().get("salt_epoch")) : "salt epoch mismatch";
    assert "rot-7".equals(saltEvent.fields().get("rotation_id")) : "rotation id mismatch";
  }

  private static void recordsFailureMetadata() throws Exception {
    final TelemetryOptions options = buildOptions();
    final RecordingSink sink = new RecordingSink();
    final TelemetryObserver observer = new TelemetryObserver(options, sink);
    final TransportRequest request =
        TransportRequest.builder()
            .setUri(new URI("https://example.com:443/transaction"))
            .setMethod("POST")
            .build();
    observer.onRequest(request);
    final IllegalStateException failure = new IllegalStateException("chaos rehearsal");
    observer.onFailure(request, failure);

    assert sink.failureCount.get() == 1 : "failure callback should fire once";
    final TelemetryRecord failureRecord = sink.lastFailureRecord;
    assert failureRecord != null : "failure record missing";
    final String errorKind =
        failureRecord
            .errorKind()
            .orElseThrow(() -> new IllegalStateException("expected error kind"));
    assert "IllegalStateException".equals(errorKind) : "error kind mismatch";
    assert failureRecord.statusCode().isEmpty() : "failure should not expose status code";
    final long latency =
        failureRecord
            .latencyMillis()
            .orElseThrow(() -> new IllegalStateException("missing failure latency"));
    assert latency >= 0 : "failure latency must be non-negative";
  }

  private static void clientConfigAddsTelemetryObserver() throws Exception {
    final TelemetryOptions options = buildOptions();
    final RecordingSink sink = new RecordingSink();
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(new URI("https://example.com"))
            .setRetryPolicy(RetryPolicy.none())
            .setTelemetryOptions(options)
            .setTelemetrySink(sink)
            .build();

    assert options == config.telemetryOptions() : "telemetry options must be preserved";
    assert TelemetryExportStatusSink.unwrap(config.telemetrySink().orElse(null)) == sink
        : "telemetry sink must be retained";
    final boolean hasTelemetryObserver =
        config.observers().stream().anyMatch(obs -> obs instanceof TelemetryObserver);
    assert hasTelemetryObserver : "telemetry observer should be registered automatically";
  }

  private static void telemetrySinkNotAddedWhenDisabled() throws Exception {
    final TelemetryOptions options = TelemetryOptions.disabled();
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(new URI("https://example.org"))
            .setTelemetryOptions(options)
            .setTelemetrySink(new RecordingSink())
            .build();
    assert !config.telemetryOptions().enabled() : "telemetry options unexpectedly enabled";
    final boolean hasTelemetryObserver =
        config.observers().stream().anyMatch(obs -> obs instanceof TelemetryObserver);
    assert !hasTelemetryObserver : "disabled telemetry should not register observers";
  }

  private static void emitsRedactionFailureEvent() throws Exception {
    final TelemetryOptions options = buildOptions();
    final RecordingSink sink = new RecordingSink();
    final TelemetryObserver observer = new TelemetryObserver(options, sink);
    final TransportRequest request =
        TransportRequest.builder()
            .setUri(new URI("https:///transaction"))
            .setMethod("GET")
            .build();
    observer.onRequest(request);
    final RecordingSink.SignalEvent failure =
        sink.findLastSignal("android.telemetry.redaction.failure");
    assert failure != null : "redaction failure signal should be emitted";
    assert "android.torii.http.request".equals(failure.fields().get("signal_id"))
        : "signal id must reference the Torii request metric";
    assert "blank_authority".equals(failure.fields().get("reason"))
        : "reason must describe the failure";
  }

  private static void emitsSaltVersionAfterConfigReload() throws Exception {
    final TelemetryOptions options = buildOptions();
    final RecordingSink sink = new RecordingSink();
    final ClientConfig initialConfig =
        ClientConfig.builder()
            .setBaseUri(new URI("https://example.com"))
            .setRetryPolicy(RetryPolicy.none())
            .setTelemetryOptions(options)
            .setTelemetrySink(sink)
            .build();
    final TelemetryObserver firstObserver = findTelemetryObserver(initialConfig);
    final TransportRequest request =
        TransportRequest.builder()
            .setUri(new URI("https://example.com/transaction"))
            .setMethod("POST")
            .build();
    firstObserver.onRequest(request);
    final int initialSaltSignals = sink.signalCount(SALT_SIGNAL);
    assert initialSaltSignals == 1 : "salt version must emit once per observer instance";

    final TelemetryOptions reloadedOptions =
        options
            .toBuilder()
            .setTelemetryRedaction(
                TelemetryOptions.Redaction.builder()
                    .setSaltHex("05060708")
                    .setSaltVersion("2026Q2")
                    .setRotationId("rot-8")
                    .build())
            .build();
    final ClientConfig reloadedConfig =
        initialConfig.toBuilder().setTelemetryOptions(reloadedOptions).build();
    final TelemetryObserver reloadedObserver = findTelemetryObserver(reloadedConfig);
    assert reloadedObserver != firstObserver : "config reload must install a new telemetry observer";

    reloadedObserver.onRequest(request);
    final int totalSaltSignals = sink.signalCount(SALT_SIGNAL);
    assert totalSaltSignals == initialSaltSignals + 1
        : "salt version must emit again after config reload";
    final RecordingSink.SignalEvent latestSalt = sink.findLastSignal(SALT_SIGNAL);
    assert latestSalt != null : "salt version event should be recorded";
    assert "2026Q2".equals(latestSalt.fields().get("salt_epoch"))
        : "reload should emit the refreshed salt epoch";
    assert "rot-8".equals(latestSalt.fields().get("rotation_id"))
        : "reload should emit the refreshed rotation id";
  }

  private static TelemetryObserver findTelemetryObserver(final ClientConfig config) {
    return config.observers().stream()
        .filter(observer -> observer instanceof TelemetryObserver)
        .map(observer -> (TelemetryObserver) observer)
        .findFirst()
        .orElseThrow(() -> new IllegalStateException("TelemetryObserver missing from config"));
  }

  private static final class RecordingSink implements TelemetrySink {
    private final AtomicInteger requestCount = new AtomicInteger();
    private final AtomicInteger responseCount = new AtomicInteger();
    private final AtomicInteger failureCount = new AtomicInteger();
    private volatile TelemetryRecord lastRequestRecord;
    private volatile TelemetryRecord lastResponseRecord;
    private volatile TelemetryRecord lastFailureRecord;
    private final java.util.List<SignalEvent> signals = new java.util.ArrayList<>();

    @Override
    public void onRequest(final TelemetryRecord record) {
      requestCount.incrementAndGet();
      lastRequestRecord = record;
    }

    @Override
    public void onResponse(final TelemetryRecord record, final ClientResponse response) {
      responseCount.incrementAndGet();
      lastResponseRecord = record;
    }

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {
      failureCount.incrementAndGet();
      lastFailureRecord = record;
    }

    @Override
    public void emitSignal(final String signalId, final Map<String, Object> fields) {
      signals.add(new SignalEvent(signalId, Map.copyOf(fields)));
    }

    SignalEvent findLastSignal(final String signalId) {
      for (int i = signals.size() - 1; i >= 0; --i) {
        final SignalEvent event = signals.get(i);
        if (event.signalId.equals(signalId)) {
          return event;
        }
      }
      return null;
    }

    int signalCount(final String signalId) {
      int count = 0;
      for (final SignalEvent event : signals) {
        if (event.signalId.equals(signalId)) {
          count++;
        }
      }
      return count;
    }

    private static final class SignalEvent {
      private final String signalId;
      private final Map<String, Object> fields;

      private SignalEvent(final String signalId, final Map<String, Object> fields) {
        this.signalId = signalId;
        this.fields = fields;
      }

      Map<String, Object> fields() {
        return fields;
      }
    }
  }

}
