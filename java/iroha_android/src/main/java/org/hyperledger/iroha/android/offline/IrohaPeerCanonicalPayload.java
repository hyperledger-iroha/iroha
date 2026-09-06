package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.Objects;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;

/** Exact canonical profile bytes. This class never re-serializes them. */
public final class IrohaPeerCanonicalPayload {
  private final IrohaPeerPayloadProfile profile;
  private final IrohaPeerPayloadKind kind;
  private final int schemaVersion;
  private final byte[] bytes;

  public IrohaPeerCanonicalPayload(
      final IrohaPeerPayloadProfile profile,
      final IrohaPeerPayloadKind kind,
      final int schemaVersion,
      final byte[] bytes) {
    this.profile = Objects.requireNonNull(profile, "profile");
    this.kind = Objects.requireNonNull(kind, "kind");
    require(schemaVersion >= 1 && schemaVersion <= 0xffff, "Invalid peer schema version");
    require(
        schemaVersion == profile.requiredSchemaVersion(),
        "Peer payload profile "
            + profile
            + " requires schema "
            + profile.requiredSchemaVersion()
            + ", received "
            + schemaVersion);
    final byte[] requiredBytes = Objects.requireNonNull(bytes, "bytes");
    require(requiredBytes.length > 0, "Peer payload is empty");
    require(
        requiredBytes.length <= maximumCanonicalBytes(this.profile, this.kind),
        "Peer payload exceeds the frozen " + this.kind + " bound");
    this.bytes = requiredBytes.clone();
    validateTypedCanonicalPayload(this.profile, this.kind, this.bytes);
    this.schemaVersion = schemaVersion;
  }

  public IrohaPeerPayloadProfile profile() {
    return profile;
  }

  public IrohaPeerPayloadKind kind() {
    return kind;
  }

  public int schemaVersion() {
    return schemaVersion;
  }

  public byte[] bytes() {
    return bytes.clone();
  }

  public int byteCount() {
    return bytes.length;
  }

  @Override
  public boolean equals(final Object other) {
    return other instanceof IrohaPeerCanonicalPayload that
        && profile == that.profile
        && kind == that.kind
        && schemaVersion == that.schemaVersion
        && Arrays.equals(bytes, that.bytes);
  }

  @Override
  public int hashCode() {
    return 31 * (31 * (31 * profile.hashCode() + kind.hashCode()) + schemaVersion)
        + Arrays.hashCode(bytes);
  }

  private static void require(final boolean condition, final String message) {
    if (!condition) throw new IllegalArgumentException(message);
  }

  private static void validateTypedCanonicalPayload(
      final IrohaPeerPayloadProfile profile,
      final IrohaPeerPayloadKind kind,
      final byte[] bytes) {
    if (profile != IrohaPeerPayloadProfile.KAGEMUSHA_V1) return;
    final String schema = switch (kind) {
      case REQUEST ->
          "iroha_data_model::kagemusha::kagemusha_v1::KagemushaPaymentRequestV1";
      case PAYMENT ->
          "iroha_data_model::kagemusha::kagemusha_v1::KagemushaPaymentV1";
      case ACKNOWLEDGEMENT ->
          "iroha_data_model::kagemusha::kagemusha_v1::KagemushaAcknowledgementV1";
    };
    final int requiredPadding = switch (kind) {
      case REQUEST, PAYMENT -> 8;
      case ACKNOWLEDGEMENT -> 0;
    };
    try {
      final NoritoHeader.DecodeResult decoded =
          NoritoHeader.decode(bytes, SchemaHash.hash16(schema));
      final NoritoHeader header = decoded.header();
      require(
          header.compression() == NoritoHeader.COMPRESSION_NONE
              && header.flags() == NoritoHeader.COMPACT_LEN
              && decoded.payload().length != 0
              && bytes.length
                  == NoritoHeader.HEADER_LENGTH + requiredPadding + decoded.payload().length
              && Arrays.equals(
                  header.encode(), Arrays.copyOfRange(bytes, 0, NoritoHeader.HEADER_LENGTH)),
          "KAGEMUSHA V1 payload must use canonical compact Norito framing");
      header.validateChecksum(decoded.payload());
    } catch (RuntimeException failure) {
      throw new IllegalArgumentException(
          "Invalid KAGEMUSHA V1 payload for " + kind, failure);
    }
  }

  private static int maximumCanonicalBytes(
      final IrohaPeerPayloadProfile profile, final IrohaPeerPayloadKind kind) {
    return switch (profile) {
      case KAGEMUSHA_V1 -> switch (kind) {
        case REQUEST -> KagemushaWireV1.MAXIMUM_PAYMENT_REQUEST_BYTES;
        case PAYMENT -> KagemushaWireV1.MAXIMUM_PAYMENT_BYTES;
        case ACKNOWLEDGEMENT -> KagemushaWireV1.MAXIMUM_ACKNOWLEDGEMENT_BYTES;
      };
    };
  }
}
