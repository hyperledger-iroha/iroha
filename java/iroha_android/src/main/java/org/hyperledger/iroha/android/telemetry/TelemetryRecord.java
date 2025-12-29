package org.hyperledger.iroha.android.telemetry;

import java.util.Objects;
import java.util.Optional;
import java.util.OptionalInt;
import java.util.OptionalLong;

/**
 * Immutable snapshot describing a single HTTP request/response pair observed by the client.
 */
public final class TelemetryRecord {
  private final String authorityHash;
  private final String saltVersion;
  private final String route;
  private final String method;
  private final Long latencyMillis;
  private final Integer statusCode;
  private final String errorKind;

  public TelemetryRecord(
      final String authorityHash,
      final String saltVersion,
      final String route,
      final String method) {
    this(authorityHash, saltVersion, route, method, null, null, null);
  }

  private TelemetryRecord(
      final String authorityHash,
      final String saltVersion,
      final String route,
      final String method,
      final Long latencyMillis,
      final Integer statusCode,
      final String errorKind) {
    this.authorityHash = authorityHash;
    this.saltVersion = saltVersion;
    this.route = route;
    this.method = method;
    this.latencyMillis = latencyMillis;
    this.statusCode = statusCode;
    this.errorKind = errorKind;
  }

  /** Returns a new record that copies this snapshot and adds outcome metadata. */
  public TelemetryRecord withOutcome(
      final Long latencyMillis,
      final Integer statusCode,
      final String errorKind) {
    final Long normalisedLatency =
        latencyMillis == null
            ? null
            : Long.valueOf(Math.max(0L, latencyMillis.longValue()));
    return new TelemetryRecord(
        authorityHash, saltVersion, route, method, normalisedLatency, statusCode, errorKind);
  }

  /** Returns the hashed authority (or {@code null} when redaction is disabled). */
  public String authorityHash() {
    return authorityHash;
  }

  public String saltVersion() {
    return saltVersion;
  }

  public String route() {
    return route;
  }

  public String method() {
    return method;
  }

  /** Returns the observed latency in milliseconds when available. */
  public OptionalLong latencyMillis() {
    return latencyMillis == null
        ? OptionalLong.empty()
        : OptionalLong.of(latencyMillis.longValue());
  }

  /** Returns the HTTP status code (or {@link OptionalInt#empty()} when unknown). */
  public OptionalInt statusCode() {
    return statusCode == null ? OptionalInt.empty() : OptionalInt.of(statusCode.intValue());
  }

  /** Returns the error classification when the request failed. */
  public Optional<String> errorKind() {
    return Optional.ofNullable(errorKind);
  }

  @Override
  public String toString() {
    return "TelemetryRecord{"
        + "authorityHash='"
        + authorityHash
        + '\''
        + ", saltVersion='"
        + saltVersion
        + '\''
        + ", route='"
        + route
        + '\''
        + ", method='"
        + method
        + '\''
        + ", latencyMillis="
        + latencyMillis
        + ", statusCode="
        + statusCode
        + ", errorKind='"
        + errorKind
        + '\''
        + '}';
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof TelemetryRecord)) {
      return false;
    }
    final TelemetryRecord other = (TelemetryRecord) obj;
    return Objects.equals(authorityHash, other.authorityHash)
        && Objects.equals(saltVersion, other.saltVersion)
        && Objects.equals(route, other.route)
        && Objects.equals(method, other.method)
        && Objects.equals(latencyMillis, other.latencyMillis)
        && Objects.equals(statusCode, other.statusCode)
        && Objects.equals(errorKind, other.errorKind);
  }

  @Override
  public int hashCode() {
    return Objects.hash(authorityHash, saltVersion, route, method, latencyMillis, statusCode, errorKind);
  }
}
