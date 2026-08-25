package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;
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
    this.bytes = Objects.requireNonNull(bytes, "bytes").clone();
    require(this.bytes.length > 0, "Peer payload is empty");
    require(
        this.bytes.length <= IrohaPeerWireMessageV1.MAXIMUM_CANONICAL_BYTES,
        "Peer payload exceeds its bound");
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
    if (profile == IrohaPeerPayloadProfile.OFFLINE_CASH_V1) {
      validateOfflineCashPeerText(kind, bytes);
      return;
    }
    if (profile != IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND) return;
    final String schema = switch (kind) {
      case RECEIVE_REQUEST ->
          "iroha_torii_shared::offline_api::OfflineRecipientReceiveOfferV2";
      case PAYMENT ->
          "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4";
      case ACKNOWLEDGEMENT ->
          "iroha_data_model::offline::model::KagemushaReceiverAcknowledgementV2";
    };
    final int requiredPadding = switch (kind) {
      case RECEIVE_REQUEST, PAYMENT -> 8;
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
          "Kagemusha canonical payload must use canonical compact Norito framing");
      header.validateChecksum(decoded.payload());
    } catch (RuntimeException failure) {
      throw new IllegalArgumentException(
          "Invalid Kagemusha canonical payload for " + kind, failure);
    }
  }

  private static void validateOfflineCashPeerText(
      final IrohaPeerPayloadKind kind, final byte[] bytes) {
    final int textMaximum = switch (kind) {
      case RECEIVE_REQUEST ->
          IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_PAYMENT_REQUEST_TEXT_BYTES;
      case PAYMENT -> IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_PAYMENT_TEXT_BYTES;
      case ACKNOWLEDGEMENT ->
          IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_ACKNOWLEDGEMENT_TEXT_BYTES;
    };
    final int rawMaximum = switch (kind) {
      case RECEIVE_REQUEST ->
          IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_PAYMENT_REQUEST_RAW_BYTES;
      case PAYMENT -> IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_PAYMENT_RAW_BYTES;
      case ACKNOWLEDGEMENT ->
          IrohaPeerWireLimitsV1.MAXIMUM_OFFLINE_CASH_ACKNOWLEDGEMENT_RAW_BYTES;
    };
    final String schema = switch (kind) {
      case RECEIVE_REQUEST ->
          "iroha_data_model::offline::offline_cash_v1::OfflineCashPaymentRequestV1";
      case PAYMENT ->
          "iroha_data_model::offline::offline_cash_v1::OfflineCashPaymentV1";
      case ACKNOWLEDGEMENT ->
          "iroha_data_model::offline::offline_cash_v1::OfflineCashAcknowledgementV1";
    };
    final int requiredPadding = switch (kind) {
      case RECEIVE_REQUEST, PAYMENT -> 8;
      case ACKNOWLEDGEMENT -> 0;
    };
    require(bytes.length <= textMaximum, "Offline Cash V1 peer text exceeds its bound");
    final String text = new String(bytes, StandardCharsets.UTF_8);
    require(
        Arrays.equals(text.getBytes(StandardCharsets.UTF_8), bytes)
            && text.startsWith(IrohaPeerWireLimitsV1.OFFLINE_CASH_TEXT_PREFIX),
        "Offline Cash V1 peer text is not canonical UTF-8");
    final String body = text.substring(IrohaPeerWireLimitsV1.OFFLINE_CASH_TEXT_PREFIX.length());
    require(
        !body.isEmpty()
            && body.chars().allMatch(value ->
                value >= 'A' && value <= 'Z'
                    || value >= 'a' && value <= 'z'
                    || value >= '0' && value <= '9'
                    || value == '-'
                    || value == '_'),
        "Offline Cash V1 peer text is not canonical Base64URL");
    final byte[] decoded;
    try {
      decoded = Base64.getUrlDecoder().decode(body);
    } catch (IllegalArgumentException failure) {
      throw new IllegalArgumentException(
          "Offline Cash V1 peer text is not canonical Base64URL", failure);
    }
    try {
      require(
          Base64.getUrlEncoder().withoutPadding().encodeToString(decoded).equals(body),
          "Offline Cash V1 peer text is not canonical Base64URL");
      require(decoded.length <= rawMaximum, "Offline Cash V1 canonical message exceeds its bound");
      final NoritoHeader.DecodeResult archive =
          NoritoHeader.decode(decoded, SchemaHash.hash16(schema));
      final NoritoHeader header = archive.header();
      require(
          header.compression() == NoritoHeader.COMPRESSION_NONE
              && header.flags() == NoritoHeader.COMPACT_LEN
              && archive.payload().length != 0
              && decoded.length
                  == NoritoHeader.HEADER_LENGTH + requiredPadding + archive.payload().length
              && Arrays.equals(
                  header.encode(), Arrays.copyOfRange(decoded, 0, NoritoHeader.HEADER_LENGTH)),
          "Offline Cash V1 peer text must carry canonical compact Norito");
      header.validateChecksum(archive.payload());
    } catch (RuntimeException failure) {
      throw new IllegalArgumentException(
          "Invalid Offline Cash V1 canonical payload for " + kind, failure);
    } finally {
      Arrays.fill(decoded, (byte) 0);
    }
  }
}
