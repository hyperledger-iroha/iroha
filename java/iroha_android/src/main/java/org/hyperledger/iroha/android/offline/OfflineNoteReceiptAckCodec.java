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

/** QR/Norito handoff codec for Offline Note receipt ACKs. */
public final class OfflineNoteReceiptAckCodec {
  public static final String TYPE = "offline_receipt_ack";
  public static final String TEXT_PREFIX = "wallet-offline-ack:";
  private static final String RECEIPT_ACK_ENVELOPE_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteReceiptAckEnvelope";

  private OfflineNoteReceiptAckCodec() {}

  public static byte[] encodeNorito(final OfflineNoteReceiptAck ack) {
    return NoritoCodec.encode(
        Objects.requireNonNull(ack, "ack"),
        RECEIPT_ACK_ENVELOPE_SCHEMA,
        RECEIPT_ACK_ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  public static OfflineNoteReceiptAck decodeNorito(final byte[] payload) {
    return NoritoCodec.decode(
        Objects.requireNonNull(payload, "payload"),
        RECEIPT_ACK_ADAPTER,
        RECEIPT_ACK_ENVELOPE_SCHEMA);
  }

  public static String encodeText(final OfflineNoteReceiptAck ack) {
    return TEXT_PREFIX + Base64.getUrlEncoder().withoutPadding().encodeToString(encodeNorito(ack));
  }

  public static OfflineNoteReceiptAck decodeText(final String text) {
    final String trimmed = Objects.requireNonNull(text, "text").trim();
    if (!trimmed.startsWith(TEXT_PREFIX)) {
      throw new IllegalArgumentException("Offline Note receipt ACK prefix missing");
    }
    return decodeNorito(Base64.getUrlDecoder().decode(trimmed.substring(TEXT_PREFIX.length())));
  }

  public static List<byte[]> encodeQrFrameBytes(final OfflineNoteReceiptAck ack) {
    return encodeQrFrameBytes(ack, new OfflineQrStream.Options());
  }

  public static List<byte[]> encodeQrFrameBytes(
      final OfflineNoteReceiptAck ack, final OfflineQrStream.Options options) {
    return OfflineQrStream.Encoder.encodeFrameBytes(
        encodeNorito(ack),
        OfflineQrStream.PayloadKind.OFFLINE_RECEIPT_ACK,
        Objects.requireNonNull(options, "options"));
  }

  public static OfflineNoteReceiptAck decodeQrPayload(final byte[] payload) {
    return decodeNorito(payload);
  }

  private static final TypeAdapter<OfflineNoteReceiptAck> RECEIPT_ACK_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final OfflineNoteReceiptAck value) {
          writeField(encoder, child -> writeString(child, value.chainId()));
          writeField(encoder, child -> writeString(child, value.paymentRequestId()));
          writeField(encoder, child -> child.writeBytes(value.tokenId()));
          writeField(encoder, child -> writeString(child, value.recipientAccountId()));
          writeField(encoder, child -> child.writeUInt(value.acceptedAtMs(), 64));
        }

        @Override
        public OfflineNoteReceiptAck decode(final NoritoDecoder decoder) {
          final String chainId = readField(decoder, OfflineNoteReceiptAckCodec::readString);
          final String paymentRequestId =
              readField(decoder, OfflineNoteReceiptAckCodec::readString);
          final byte[] tokenId = readField(decoder, child -> child.readBytes(32));
          final String recipientAccountId =
              readField(decoder, OfflineNoteReceiptAckCodec::readString);
          final long acceptedAtMs = readField(decoder, child -> child.readUInt(64));
          return new OfflineNoteReceiptAck(
              chainId, paymentRequestId, tokenId, recipientAccountId, acceptedAtMs);
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
      throw new IllegalArgumentException("Offline Note receipt ACK field length overflow");
    }
    final NoritoDecoder child =
        new NoritoDecoder(decoder.readBytes((int) length), decoder.flags(), decoder.flagsHint());
    final T value = read.read(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException(
          "Trailing bytes after Offline Note receipt ACK field decode");
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
      throw new IllegalArgumentException("Offline Note receipt ACK string length overflow");
    }
    final String value = new String(decoder.readBytes((int) length), StandardCharsets.UTF_8);
    if (value.trim().isEmpty()) {
      throw new IllegalArgumentException("Offline Note receipt ACK string must not be blank");
    }
    return value;
  }
}
