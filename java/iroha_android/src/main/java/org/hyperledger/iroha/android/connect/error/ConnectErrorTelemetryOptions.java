package org.hyperledger.iroha.android.connect.error;

/** Optional overrides when rendering telemetry attributes. */
public record ConnectErrorTelemetryOptions(
    Boolean fatal, Integer httpStatus, String underlying) {

  public ConnectErrorTelemetryOptions {
    // Defensive copies are unnecessary; fields are immutable.
  }

  public static ConnectErrorTelemetryOptions empty() {
    return new ConnectErrorTelemetryOptions(null, null, null);
  }

  public ConnectErrorTelemetryOptions withFatal(Boolean value) {
    if ((fatal == null && value == null)
        || (fatal != null && fatal.equals(value))) {
      return this;
    }
    return new ConnectErrorTelemetryOptions(value, httpStatus, underlying);
  }
}
