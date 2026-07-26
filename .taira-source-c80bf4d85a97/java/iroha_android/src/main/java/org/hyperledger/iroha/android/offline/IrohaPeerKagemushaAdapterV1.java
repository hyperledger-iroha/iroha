package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.Objects;

/** Bounded small-handoff adapter to the existing native-canonical Kagemusha API. */
public final class IrohaPeerKagemushaAdapterV1 {
  public static final int NATIVE_ARCHIVE_SCHEMA_VERSION = 0x0102;

  private IrohaPeerKagemushaAdapterV1() {}

  public static IrohaPeerWireMessageV1 wrap(final KagemushaPeerTransport.Payload payload) {
    return wrap(
        payload,
        IrohaPeerWireCompressionPolicyV1.DISABLED,
        IrohaPeerWireLimitsV1.PEER_V1);
  }

  public static IrohaPeerWireMessageV1 wrap(
      final KagemushaPeerTransport.Payload payload,
      final IrohaPeerWireCompressionPolicyV1 compressionPolicy,
      final IrohaPeerWireLimitsV1 limits) {
    Objects.requireNonNull(payload, "payload");
    final IrohaPeerPayloadKind kind = switch (payload.kind()) {
      case RECEIVE_REQUEST -> IrohaPeerPayloadKind.RECEIVE_REQUEST;
      case PAYMENT -> IrohaPeerPayloadKind.PAYMENT;
      case ACKNOWLEDGEMENT -> IrohaPeerPayloadKind.ACKNOWLEDGEMENT;
    };
    final byte[] bytes = payload.archive();
    try {
      return new IrohaPeerWireMessageV1(
          new IrohaPeerCanonicalPayload(
              IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
              kind,
              NATIVE_ARCHIVE_SCHEMA_VERSION,
              bytes),
          Objects.requireNonNull(compressionPolicy, "compressionPolicy"),
          Objects.requireNonNull(limits, "limits"));
    } finally {
      Arrays.fill(bytes, (byte) 0);
    }
  }

  public static KagemushaPeerTransport.Payload decode(final IrohaPeerWireMessageV1 message) {
    final IrohaPeerCanonicalPayload payload =
        Objects.requireNonNull(message, "message").canonicalPayload();
    if (payload.profile() != IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND) {
      throw new IllegalArgumentException("Unexpected peer payload profile");
    }
    if (payload.schemaVersion() != NATIVE_ARCHIVE_SCHEMA_VERSION) {
      throw new IllegalArgumentException("Unsupported Kagemusha native archive schema");
    }
    final KagemushaPeerTransport.Kind kind = switch (payload.kind()) {
      case RECEIVE_REQUEST -> KagemushaPeerTransport.Kind.RECEIVE_REQUEST;
      case PAYMENT -> KagemushaPeerTransport.Kind.PAYMENT;
      case ACKNOWLEDGEMENT -> KagemushaPeerTransport.Kind.ACKNOWLEDGEMENT;
    };
    final byte[] bytes = payload.bytes();
    try {
      return KagemushaPeerTransport.Payload.decode(bytes, kind);
    } finally {
      Arrays.fill(bytes, (byte) 0);
    }
  }
}
