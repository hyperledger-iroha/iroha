package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.Objects;

/**
 * Bounded handoff adapter for the frozen three-message KAGEMUSHA V1 IPM1 exchange.
 */
public final class IrohaPeerKagemushaAdapterV1 {
  public static final int ARCHIVE_SCHEMA_VERSION = 1;

  private IrohaPeerKagemushaAdapterV1() {}

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
              IrohaPeerPayloadProfile.KAGEMUSHA_V1,
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
    if (payload.profile() != IrohaPeerPayloadProfile.KAGEMUSHA_V1
        || payload.schemaVersion() != ARCHIVE_SCHEMA_VERSION) {
      throw new IllegalArgumentException("Unsupported KAGEMUSHA V1 peer payload");
    }
    return payload.bytes();
  }
}
