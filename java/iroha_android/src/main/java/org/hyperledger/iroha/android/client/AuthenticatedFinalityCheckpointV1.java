package org.hyperledger.iroha.android.client;

import java.nio.ByteBuffer;
import java.util.Arrays;

/** Application-persisted Sumeragi-v2 finality checkpoint verified by ABI-22 native code. */
public final class AuthenticatedFinalityCheckpointV1 {
  public static final int CONTEXT_ID_BYTES = 32;
  public static final int PROJECTION_BYTES = 8 + CONTEXT_ID_BYTES;

  private final long height;
  private final byte[] heightContextId;

  public AuthenticatedFinalityCheckpointV1(final long height, final byte[] heightContextId) {
    if (height <= 0) {
      throw new IllegalArgumentException("height must be positive");
    }
    if (heightContextId == null
        || heightContextId.length != CONTEXT_ID_BYTES
        || (heightContextId[CONTEXT_ID_BYTES - 1] & 1) == 0) {
      throw new IllegalArgumentException(
          "heightContextId must contain one exact marked 32-byte Iroha hash");
    }
    this.height = height;
    this.heightContextId = heightContextId.clone();
  }

  public long height() { return height; }

  public byte[] heightContextId() { return heightContextId.clone(); }

  /** Exact ABI-22 persistence form: positive u64 big-endian followed by the marked context id. */
  public byte[] projectionBytes() {
    final byte[] projection = new byte[PROJECTION_BYTES];
    ByteBuffer.wrap(projection).putLong(height).put(heightContextId);
    return projection;
  }

  static AuthenticatedFinalityCheckpointV1 fromProjection(final byte[] projection) {
    if (projection == null || projection.length != PROJECTION_BYTES) {
      throw new IllegalStateException("native finality checkpoint projection has invalid shape");
    }
    final ByteBuffer buffer = ByteBuffer.wrap(projection.clone());
    final long height = buffer.getLong();
    final byte[] contextId = new byte[CONTEXT_ID_BYTES];
    buffer.get(contextId);
    return new AuthenticatedFinalityCheckpointV1(height, contextId);
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) return true;
    if (!(other instanceof AuthenticatedFinalityCheckpointV1)) return false;
    final AuthenticatedFinalityCheckpointV1 that =
        (AuthenticatedFinalityCheckpointV1) other;
    return height == that.height && Arrays.equals(heightContextId, that.heightContextId);
  }

  @Override
  public int hashCode() {
    return 31 * Long.hashCode(height) + Arrays.hashCode(heightContextId);
  }
}
