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

/** QR/Norito handoff codec for Offline Note V2 receipt ACKs. */
public final class OfflineNoteV2ReceiptAckCodec {
  public static final String TYPE = "offline_receipt_ack_v2";
  public static final long VERSION = 2L;
  public static final String TEXT_PREFIX = "wallet-offline-ack-v2:";
  private static final String RECEIPT_ACK_ENVELOPE_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteReceiptAckEnvelopeV2";

  private OfflineNoteV2ReceiptAckCodec() {}

  public static byte[] encodeNorito(final OfflineNoteV2ReceiptAck ack) {
    return NoritoCodec.encode(
        Objects.requireNonNull(ack, "ack"),
        RECEIPT_ACK_ENVELOPE_SCHEMA,
        RECEIPT_ACK_ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  public static OfflineNoteV2ReceiptAck decodeNorito(final byte[] payload) {
    return NoritoCodec.decode(
        Objects.requireNonNull(payload, "payload"),
        RECEIPT_ACK_ADAPTER,
        RECEIPT_ACK_ENVELOPE_SCHEMA);
  }

  public static String encodeText(final OfflineNoteV2ReceiptAck ack) {
    return TEXT_PREFIX + Base64.getUrlEncoder().withoutPadding().encodeToString(encodeNorito(ack));
  }

  public static OfflineNoteV2ReceiptAck decodeText(final String text) {
    final String trimmed = Objects.requireNonNull(text, "text").trim();
    if (!trimmed.startsWith(TEXT_PREFIX)) {
      throw new IllegalArgumentException("Offline Note V2 receipt ACK prefix missing");
    }
    return decodeNorito(Base64.getUrlDecoder().decode(trimmed.substring(TEXT_PREFIX.length())));
  }

  public static List<byte[]> encodeQrFrameBytes(final OfflineNoteV2ReceiptAck ack) {
    return encodeQrFrameBytes(ack, new OfflineQrStream.Options());
  }

  public static List<byte[]> encodeQrFrameBytes(
      final OfflineNoteV2ReceiptAck ack, final OfflineQrStream.Options options) {
    return OfflineQrStream.Encoder.encodeFrameBytes(
        encodeNorito(ack),
        OfflineQrStream.PayloadKind.OFFLINE_RECEIPT_ACK_V2,
        Objects.requireNonNull(options, "options"));
  }

  public static OfflineNoteV2ReceiptAck decodeQrPayload(final byte[] payload) {
    return decodeNorito(payload);
  }

  private static final TypeAdapter<OfflineNoteV2ReceiptAck> RECEIPT_ACK_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final OfflineNoteV2ReceiptAck value) {
          writeField(encoder, child -> child.writeUInt(VERSION, 64));
          writeField(encoder, child -> writeString(child, value.chainId()));
          writeField(encoder, child -> writeString(child, value.paymentRequestId()));
          writeField(encoder, child -> child.writeBytes(value.tokenId()));
          writeField(encoder, child -> writeString(child, value.recipientAccountId()));
          writeField(encoder, child -> child.writeUInt(value.acceptedAtMs(), 64));
        }

        @Override
        public OfflineNoteV2ReceiptAck decode(final NoritoDecoder decoder) {
          final long version = readField(decoder, child -> child.readUInt(64));
          if (version != VERSION) {
            throw new IllegalArgumentException(
                "Offline Note V2 receipt ACK Norito version must be " + VERSION);
          }
          final String chainId = readField(decoder, OfflineNoteV2ReceiptAckCodec::readString);
          final String paymentRequestId =
              readField(decoder, OfflineNoteV2ReceiptAckCodec::readString);
          final byte[] tokenId = readField(decoder, child -> child.readBytes(32));
          final String recipientAccountId =
              readField(decoder, OfflineNoteV2ReceiptAckCodec::readString);
          final long acceptedAtMs = readField(decoder, child -> child.readUInt(64));
          return new OfflineNoteV2ReceiptAck(
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
      throw new IllegalArgumentException("Offline Note V2 receipt ACK field length overflow");
    }
    final NoritoDecoder child =
        new NoritoDecoder(decoder.readBytes((int) length), decoder.flags(), decoder.flagsHint());
    final T value = read.read(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException(
          "Trailing bytes after Offline Note V2 receipt ACK field decode");
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
      throw new IllegalArgumentException("Offline Note V2 receipt ACK string length overflow");
    }
    final String value = new String(decoder.readBytes((int) length), StandardCharsets.UTF_8);
    if (value.trim().isEmpty()) {
      throw new IllegalArgumentException("Offline Note V2 receipt ACK string must not be blank");
    }
    return value;
  }
}
