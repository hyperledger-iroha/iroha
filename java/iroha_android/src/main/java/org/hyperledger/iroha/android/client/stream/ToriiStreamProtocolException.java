package org.hyperledger.iroha.android.client.stream;

import java.util.Objects;

/** A malformed terminal {@code stream_error} frame that cannot be interpreted safely. */
public final class ToriiStreamProtocolException extends RuntimeException {

  private final String reason;
  private final String rawData;

  ToriiStreamProtocolException(final String reason, final String rawData) {
    this(reason, rawData, null);
  }

  ToriiStreamProtocolException(
      final String reason, final String rawData, final Throwable cause) {
    super("Torii emitted a malformed stream_error event: " + reason, cause);
    this.reason = Objects.requireNonNull(reason, "reason");
    this.rawData = Objects.requireNonNull(rawData, "rawData");
  }

  /** Returns the stable explanation of the protocol violation. */
  public String reason() {
    return reason;
  }

  /** Returns the unmodified data carried by the malformed SSE frame. */
  public String rawData() {
    return rawData;
  }
}
