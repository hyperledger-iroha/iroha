package org.hyperledger.iroha.android.telemetry;

import java.util.Map;
import org.hyperledger.iroha.android.client.ClientResponse;

/**
 * Sink that consumes telemetry events emitted by {@link org.hyperledger.iroha.android.client.ClientObserver}
 * implementations.
 *
 * <p>Mobile apps can supply an implementation backed by their preferred observability stack (e.g.,
 * OpenTelemetry, structured logs, or Norito exporters).
 */
public interface TelemetrySink {

  void onRequest(TelemetryRecord record);

  void onResponse(TelemetryRecord record, ClientResponse response);

  void onFailure(TelemetryRecord record, Throwable error);

  /**
   * Emits structured telemetry signals that are not tied to HTTP lifecycle callbacks (e.g., queue
   * depth gauges). Default implementation is a no-op so existing sinks remain compatible.
   */
  default void emitSignal(final String signalId, final Map<String, Object> fields) {
    // Intentionally empty.
  }
}
