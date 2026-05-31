package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Base64;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** QR/Norito handoff codec for Offline Note payment tokens. */
public final class OfflineNotePaymentTokenCodec {
  public static final String TYPE = "offline_payment_token";
  public static final String TEXT_PREFIX = "wallet-offline-bearer-cash-payment:";
  public static final long ENVELOPE_VERSION = 2L;
  private static final String TOKEN_ENVELOPE_SCHEMA =
      "iroha_data_model::offline::model::OfflineNotePaymentTokenEnvelope";

  private OfflineNotePaymentTokenCodec() {}

  public static byte[] encodeNorito(final OfflineNotePaymentToken token) {
    return NoritoCodec.encode(
        Objects.requireNonNull(token, "token"),
        TOKEN_ENVELOPE_SCHEMA,
        TOKEN_ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  public static OfflineNotePaymentToken decodeNorito(final byte[] payload) {
    return NoritoCodec.decode(
        Objects.requireNonNull(payload, "payload"), TOKEN_ADAPTER, TOKEN_ENVELOPE_SCHEMA);
  }

  public static byte[] encodeJson(final OfflineNotePaymentToken token) {
    return encodeNorito(token);
  }

  public static OfflineNotePaymentToken decodeJson(final byte[] payload) {
    return decodeNorito(payload);
  }

  public static String encodeText(final OfflineNotePaymentToken token) {
    return TEXT_PREFIX + Base64.getUrlEncoder().withoutPadding().encodeToString(encodeNorito(token));
  }

  public static OfflineNotePaymentToken decodeText(final String text) {
    final String trimmed = Objects.requireNonNull(text, "text").trim();
    if (!trimmed.startsWith(TEXT_PREFIX)) {
      throw new IllegalArgumentException("Offline Note payment token prefix missing");
    }
    return decodeNorito(
        OfflineBase64Url.decodeUnpadded(
            trimmed.substring(TEXT_PREFIX.length()), "Offline Note payment token payload"));
  }

  public static List<byte[]> encodeQrFrameBytes(final OfflineNotePaymentToken token) {
    return encodeQrFrameBytes(token, new OfflineQrStream.Options());
  }

  public static List<byte[]> encodeQrFrameBytes(
      final OfflineNotePaymentToken token, final OfflineQrStream.Options options) {
    return OfflineQrStream.Encoder.encodeFrameBytes(
        encodeNorito(token), OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN, options);
  }

  public static OfflineNotePaymentToken decodeQrPayload(final byte[] payload) {
    return decodeNorito(payload);
  }

  private static final TypeAdapter<OfflineNotePaymentToken> TOKEN_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final OfflineNotePaymentToken value) {
          writeField(encoder, child -> child.writeUInt(ENVELOPE_VERSION, 64));
          writeField(encoder, child -> writeString(child, value.chainId()));
          writeField(encoder, child -> writeString(child, value.paymentRequestId()));
          writeField(encoder, child -> child.writeUInt(value.createdAtMs(), 64));
          writeField(encoder, child -> writeBytesVec(child, value.tokenNonce()));
          writeField(encoder, child -> child.writeBytes(value.tokenId()));
          writeField(encoder, child -> writeBytesVec(child, value.audit().noritoEncoded()));
          writeField(encoder, child -> writeAuditTrail(child, value.bearerAuditTrail()));
        }

        @Override
        public OfflineNotePaymentToken decode(final NoritoDecoder decoder) {
          final long version = readField(decoder, child -> child.readUInt(64));
          if (version != ENVELOPE_VERSION) {
            throw new IllegalArgumentException(
                "Offline Note payment token envelope version is unsupported");
          }
          final String chainId = readField(decoder, OfflineNotePaymentTokenCodec::readString);
          final String paymentRequestId =
              readField(decoder, OfflineNotePaymentTokenCodec::readString);
          final long createdAtMs = readField(decoder, child -> child.readUInt(64));
          final byte[] tokenNonce = readField(decoder, OfflineNotePaymentTokenCodec::readBytesVec);
          final byte[] tokenId = readField(decoder, child -> child.readBytes(32));
          final OfflineNote.AuditBundle audit =
              OfflineNote.decodeAudit(readField(decoder, OfflineNotePaymentTokenCodec::readBytesVec));
          final List<OfflineNote.AuditBundle> bearerAuditTrail =
              readField(decoder, OfflineNotePaymentTokenCodec::readAuditTrail);
          if (!java.util.Arrays.equals(audit.tokenId(), tokenId)) {
            throw new IllegalArgumentException(
                "Offline Note payment token id does not match audit bundle");
          }
          return new OfflineNotePaymentToken(
              chainId, paymentRequestId, tokenNonce, tokenId, audit, bearerAuditTrail, createdAtMs);
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
      throw new IllegalArgumentException("Offline Note payment token field length overflow");
    }
    final NoritoDecoder child =
        new NoritoDecoder(decoder.readBytes((int) length), decoder.flags(), decoder.flagsHint());
    final T value = read.read(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after Offline Note payment token field decode");
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
      throw new IllegalArgumentException("Offline Note payment token string length overflow");
    }
    final String value = new String(decoder.readBytes((int) length), StandardCharsets.UTF_8);
    if (value.trim().isEmpty()) {
      throw new IllegalArgumentException("Offline Note payment token string must not be blank");
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
      throw new IllegalArgumentException("Offline Note payment token bytes length overflow");
    }
    return decoder.readBytes((int) length);
  }

  private static void writeAuditTrail(
      final NoritoEncoder encoder, final List<OfflineNote.AuditBundle> audits) {
    encoder.writeUInt(audits.size(), 64);
    for (final OfflineNote.AuditBundle audit : audits) {
      writeField(encoder, child -> writeBytesVec(child, audit.noritoEncoded()));
    }
  }

  private static List<OfflineNote.AuditBundle> readAuditTrail(final NoritoDecoder decoder) {
    final long count = decoder.readUInt(64);
    if (count > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Offline Note bearer audit trail length overflow");
    }
    final List<OfflineNote.AuditBundle> audits = new ArrayList<>((int) count);
    for (int index = 0; index < count; index++) {
      audits.add(OfflineNote.decodeAudit(readField(decoder, OfflineNotePaymentTokenCodec::readBytesVec)));
    }
    return audits;
  }
}
