package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.Objects;

/** Bounded handoff adapter for the sole canonical Offline Cash V1 wire values. */
public final class IrohaPeerOfflineCashAdapterV1 {
  public static final int ARCHIVE_SCHEMA_VERSION = 1;

  private IrohaPeerOfflineCashAdapterV1() {}

  public static IrohaPeerWireMessageV1 wrap(
      final IrohaPeerPayloadKind kind, final byte[] canonicalPayload) {
    return wrap(
        kind,
        canonicalPayload,
        IrohaPeerWireCompressionPolicyV1.DISABLED,
        IrohaPeerWireLimitsV1.PEER_V1);
  }

  public static IrohaPeerWireMessageV1 wrap(
      final IrohaPeerPayloadKind kind,
      final byte[] canonicalPayload,
      final IrohaPeerWireCompressionPolicyV1 compressionPolicy,
      final IrohaPeerWireLimitsV1 limits) {
    final byte[] bytes = Objects.requireNonNull(canonicalPayload, "canonicalPayload").clone();
    try {
      return new IrohaPeerWireMessageV1(
          new IrohaPeerCanonicalPayload(
              IrohaPeerPayloadProfile.OFFLINE_CASH_V1,
              Objects.requireNonNull(kind, "kind"),
              ARCHIVE_SCHEMA_VERSION,
              bytes),
          Objects.requireNonNull(compressionPolicy, "compressionPolicy"),
          Objects.requireNonNull(limits, "limits"));
    } finally {
      Arrays.fill(bytes, (byte) 0);
    }
  }

  public static byte[] decode(final IrohaPeerWireMessageV1 message) {
    final IrohaPeerCanonicalPayload payload =
        Objects.requireNonNull(message, "message").canonicalPayload();
    if (payload.profile() != IrohaPeerPayloadProfile.OFFLINE_CASH_V1
        || payload.schemaVersion() != ARCHIVE_SCHEMA_VERSION) {
      throw new IllegalArgumentException("Unsupported Offline Cash V1 peer payload");
    }
    return payload.bytes();
  }
}
