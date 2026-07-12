package org.hyperledger.iroha.android.client.stream;

import java.math.BigInteger;
import java.util.Objects;

/** A terminal error reported after a canonical Torii SSE stream has started. */
public final class ToriiStreamException extends RuntimeException {

  private final String code;
  private final String serverMessage;
  private final BigInteger droppedMessages;
  private final boolean replayAvailable;
  private final String rawData;

  ToriiStreamException(
      final String code,
      final String serverMessage,
      final BigInteger droppedMessages,
      final boolean replayAvailable,
      final String rawData) {
    super(code + ": " + serverMessage);
    this.code = Objects.requireNonNull(code, "code");
    this.serverMessage = Objects.requireNonNull(serverMessage, "serverMessage");
    this.droppedMessages = droppedMessages;
    this.replayAvailable = replayAvailable;
    this.rawData = Objects.requireNonNull(rawData, "rawData");
  }

  /** Returns the stable machine-readable error code supplied by Torii. */
  public String code() {
    return code;
  }

  /** Returns the human-readable error message supplied by Torii. */
  public String serverMessage() {
    return serverMessage;
  }

  /** Returns the skipped broadcast-message count, or {@code null} when it was not reported. */
  public BigInteger droppedMessages() {
    return droppedMessages;
  }

  /** Returns whether Torii can replay the missing portion of the stream. */
  public boolean replayAvailable() {
    return replayAvailable;
  }

  /** Returns the unmodified JSON data carried by the terminal SSE frame. */
  public String rawData() {
    return rawData;
  }
}
