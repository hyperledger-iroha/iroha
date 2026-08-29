package org.hyperledger.iroha.android.client;

import java.nio.ByteBuffer;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Map;
import java.util.Objects;

/** Operation-tagged JSON body produced by the native Rust wallet/coordinator. */
public final class AtomicPrivateSettlementPreparedRequestV1 implements AutoCloseable {
  private final AtomicPrivateSettlementOperationV1 operation;
  private final byte[] body;
  private boolean closed;

  private AtomicPrivateSettlementPreparedRequestV1(
      final AtomicPrivateSettlementOperationV1 operation, final byte[] body) {
    this.operation = operation;
    this.body = body.clone();
  }

  /**
   * Validate strict UTF-8 JSON and bind it to exactly one mutation route.
   *
   * <p>Proof witnesses and capsule plaintext are never accepted by this API; the native worker
   * produces the complete Torii DTO.
   */
  public static AtomicPrivateSettlementPreparedRequestV1 fromNativePreparedJson(
      final AtomicPrivateSettlementOperationV1 operation, final byte[] json) {
    Objects.requireNonNull(operation, "operation");
    if (json == null || json.length == 0 || json.length > operation.maximumRequestBytes()) {
      throw new IllegalArgumentException("atomic private settlement request exceeds its route limit");
    }
    final String text;
    try {
      text =
          StandardCharsets.UTF_8
              .newDecoder()
              .onMalformedInput(CodingErrorAction.REPORT)
              .onUnmappableCharacter(CodingErrorAction.REPORT)
              .decode(ByteBuffer.wrap(json))
              .toString();
    } catch (final Exception error) {
      throw new IllegalArgumentException(
          "atomic private settlement request must be exact UTF-8", error);
    }
    final Object parsed;
    try {
      parsed = JsonParser.parse(text);
    } catch (final RuntimeException error) {
      throw new IllegalArgumentException(
          "atomic private settlement request must be one strict JSON object", error);
    }
    if (!(parsed instanceof Map<?, ?> fields)
        || !fields.keySet().equals(operation.topLevelFields())) {
      throw new IllegalArgumentException(
          "atomic private settlement request has the wrong top-level fields");
    }
    final byte[] canonical = JsonEncoder.encode(parsed).getBytes(StandardCharsets.UTF_8);
    if (canonical.length > operation.maximumRequestBytes()) {
      throw new IllegalArgumentException(
          "canonical atomic private settlement request exceeds its route limit");
    }
    return new AtomicPrivateSettlementPreparedRequestV1(operation, canonical);
  }

  /** Exact operation to which this request is bound. */
  public AtomicPrivateSettlementOperationV1 operation() {
    return operation;
  }

  /** Defensive body copy for one exact signed request. */
  public synchronized byte[] bytes() {
    if (closed) {
      throw new IllegalStateException("atomic private settlement request is closed");
    }
    return body.clone();
  }

  /** Erase the SDK-owned request copy. */
  @Override
  public synchronized void close() {
    if (!closed) {
      Arrays.fill(body, (byte) 0);
      closed = true;
    }
  }

  @Override
  public String toString() {
    return "AtomicPrivateSettlementPreparedRequestV1(operation="
        + operation
        + ", body=[REDACTED])";
  }
}
