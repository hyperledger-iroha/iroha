package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** QR/Norito handoff codec for Offline Note V2 payment tokens. */
public final class OfflineNoteV2PaymentTokenCodec {
  public static final String TYPE = "offline_payment_token_v2";
  public static final long VERSION = 2L;
  public static final String TEXT_PREFIX = "wallet-offline-payment-v2:";
  private static final String TOKEN_ENVELOPE_SCHEMA =
      "iroha_data_model::offline::model::OfflineNotePaymentTokenEnvelopeV2";

  private OfflineNoteV2PaymentTokenCodec() {}

  public static byte[] encodeNorito(final OfflineNoteV2PaymentToken token) {
    return NoritoCodec.encode(
        Objects.requireNonNull(token, "token"),
        TOKEN_ENVELOPE_SCHEMA,
        TOKEN_ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  public static OfflineNoteV2PaymentToken decodeNorito(final byte[] payload) {
    return NoritoCodec.decode(
        Objects.requireNonNull(payload, "payload"), TOKEN_ADAPTER, TOKEN_ENVELOPE_SCHEMA);
  }

  public static byte[] encodeJson(final OfflineNoteV2PaymentToken token) {
    return encodeNorito(token);
  }

  public static OfflineNoteV2PaymentToken decodeJson(final byte[] payload) {
    return decodeNorito(payload);
  }

  public static String encodeText(final OfflineNoteV2PaymentToken token) {
    return TEXT_PREFIX + Base64.getUrlEncoder().withoutPadding().encodeToString(encodeNorito(token));
  }

  public static OfflineNoteV2PaymentToken decodeText(final String text) {
    final String trimmed = Objects.requireNonNull(text, "text").trim();
    if (!trimmed.startsWith(TEXT_PREFIX)) {
      throw new IllegalArgumentException("Offline Note V2 payment token prefix missing");
    }
    return decodeNorito(Base64.getUrlDecoder().decode(trimmed.substring(TEXT_PREFIX.length())));
  }

  public static List<byte[]> encodeQrFrameBytes(final OfflineNoteV2PaymentToken token) {
    return encodeQrFrameBytes(token, new OfflineQrStream.Options());
  }

  public static List<byte[]> encodeQrFrameBytes(
      final OfflineNoteV2PaymentToken token, final OfflineQrStream.Options options) {
    return OfflineQrStream.Encoder.encodeFrameBytes(
        encodeNorito(token), OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN_V2, options);
  }

  public static OfflineNoteV2PaymentToken decodeQrPayload(final byte[] payload) {
    return decodeNorito(payload);
  }

  private static final TypeAdapter<OfflineNoteV2PaymentToken> TOKEN_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final OfflineNoteV2PaymentToken value) {
          writeField(encoder, child -> child.writeUInt(VERSION, 64));
          writeField(encoder, child -> writeString(child, value.chainId()));
          writeField(encoder, child -> writeString(child, value.paymentRequestId()));
          writeField(encoder, child -> child.writeUInt(value.createdAtMs(), 64));
          writeField(encoder, child -> writeBytesVec(child, value.tokenNonce()));
          writeField(encoder, child -> child.writeBytes(value.tokenId()));
          writeField(encoder, child -> writeBytesVec(child, value.audit().noritoEncoded()));
        }

        @Override
        public OfflineNoteV2PaymentToken decode(final NoritoDecoder decoder) {
          final long version = readField(decoder, child -> child.readUInt(64));
          if (version != VERSION) {
            throw new IllegalArgumentException(
                "Offline Note V2 payment token Norito version must be " + VERSION);
          }
          final String chainId = readField(decoder, OfflineNoteV2PaymentTokenCodec::readString);
          final String paymentRequestId =
              readField(decoder, OfflineNoteV2PaymentTokenCodec::readString);
          final long createdAtMs = readField(decoder, child -> child.readUInt(64));
          final byte[] tokenNonce = readField(decoder, OfflineNoteV2PaymentTokenCodec::readBytesVec);
          final byte[] tokenId = readField(decoder, child -> child.readBytes(32));
          final OfflineNoteV2.AuditBundleV2 audit =
              OfflineNoteV2.decodeAudit(readField(decoder, OfflineNoteV2PaymentTokenCodec::readBytesVec));
          if (!java.util.Arrays.equals(audit.tokenId(), tokenId)) {
            throw new IllegalArgumentException(
                "Offline Note V2 payment token id does not match audit bundle");
          }
          return new OfflineNoteV2PaymentToken(
              chainId, paymentRequestId, tokenNonce, tokenId, audit, createdAtMs);
        }
      };

  private interface FieldWriter {
    void write(NoritoEncoder encoder);
  }

  private interface FieldReader<T> {
    T read(NoritoDecoder decoder);
  }

  private static void writeField(final NoritoEncoder encoder, final FieldWriter write) {
    final NoritoEncoder child = encoder.childEncoder();
    write.write(child);
    final byte[] payload = child.toByteArray();
    encoder.writeLength(payload.length, true);
    encoder.writeBytes(payload);
  }

  private static <T> T readField(final NoritoDecoder decoder, final FieldReader<T> read) {
    final long length = decoder.readLength(true);
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Offline Note V2 payment token field length overflow");
    }
    final NoritoDecoder child =
        new NoritoDecoder(decoder.readBytes((int) length), decoder.flags(), decoder.flagsHint());
    final T value = read.read(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after Offline Note V2 payment token field decode");
    }
    return value;
  }

  private static void writeString(final NoritoEncoder encoder, final String value) {
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    encoder.writeLength(bytes.length, true);
    encoder.writeBytes(bytes);
  }

  private static String readString(final NoritoDecoder decoder) {
    final long length = decoder.readLength(true);
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Offline Note V2 payment token string length overflow");
    }
    final String value = new String(decoder.readBytes((int) length), StandardCharsets.UTF_8);
    if (value.trim().isEmpty()) {
      throw new IllegalArgumentException("Offline Note V2 payment token string must not be blank");
    }
    return value;
  }

  private static void writeBytesVec(final NoritoEncoder encoder, final byte[] value) {
    encoder.writeUInt(value.length, 64);
    encoder.writeBytes(value);
  }

  private static byte[] readBytesVec(final NoritoDecoder decoder) {
    final long length = decoder.readUInt(64);
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Offline Note V2 payment token bytes length overflow");
    }
    return decoder.readBytes((int) length);
  }
}
