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

/** QR/Norito handoff codec for Offline Note receive requests. */
public final class OfflineNoteReceiveRequestCodec {
  public static final String TYPE = "offline_receive_request";
  public static final String TEXT_PREFIX = "wallet-offline-bearer-cash-receive:";
  private static final String RECEIVE_REQUEST_ENVELOPE_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteReceiveRequestEnvelope";

  private OfflineNoteReceiveRequestCodec() {}

  public static byte[] encodeNorito(final OfflineNoteReceiveRequest request) {
    return NoritoCodec.encode(
        Objects.requireNonNull(request, "request"),
        RECEIVE_REQUEST_ENVELOPE_SCHEMA,
        RECEIVE_REQUEST_ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  public static OfflineNoteReceiveRequest decodeNorito(final byte[] payload) {
    return NoritoCodec.decode(
        Objects.requireNonNull(payload, "payload"),
        RECEIVE_REQUEST_ADAPTER,
        RECEIVE_REQUEST_ENVELOPE_SCHEMA);
  }

  public static String encodeText(final OfflineNoteReceiveRequest request) {
    return TEXT_PREFIX
        + Base64.getUrlEncoder().withoutPadding().encodeToString(encodeNorito(request));
  }

  public static OfflineNoteReceiveRequest decodeText(final String text) {
    final String trimmed = Objects.requireNonNull(text, "text").trim();
    if (!trimmed.startsWith(TEXT_PREFIX)) {
      throw new IllegalArgumentException("Offline Note receive request prefix missing");
    }
    return decodeNorito(
        OfflineBase64Url.decodeUnpadded(
            trimmed.substring(TEXT_PREFIX.length()), "Offline Note receive request payload"));
  }

  public static List<byte[]> encodeQrFrameBytes(final OfflineNoteReceiveRequest request) {
    return encodeQrFrameBytes(request, new OfflineQrStream.Options());
  }

  public static List<byte[]> encodeQrFrameBytes(
      final OfflineNoteReceiveRequest request, final OfflineQrStream.Options options) {
    return OfflineQrStream.Encoder.encodeFrameBytes(
        encodeNorito(request),
        OfflineQrStream.PayloadKind.OFFLINE_RECEIVE_REQUEST,
        options);
  }

  public static OfflineNoteReceiveRequest decodeQrPayload(final byte[] payload) {
    return decodeNorito(payload);
  }

  private static final TypeAdapter<OfflineNoteReceiveRequest> RECEIVE_REQUEST_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final OfflineNoteReceiveRequest value) {
          writeField(encoder, child -> writeString(child, value.chainId()));
          writeField(encoder, child -> writeString(child, value.paymentRequestId()));
          writeField(encoder, child -> writeString(child, value.accountId()));
          writeField(encoder, child -> writeString(child, value.assetDefinitionId()));
          writeField(encoder, child -> writeString(child, value.assetId()));
          writeField(encoder, child -> writeString(child, value.canonicalAmount()));
          writeField(encoder, child -> writeBytesVec(child, value.keyCertificate().noritoEncoded()));
          writeField(encoder, child -> child.writeBytes(value.outputCommitment()));
        }

        @Override
        public OfflineNoteReceiveRequest decode(final NoritoDecoder decoder) {
          final String chainId = readField(decoder, OfflineNoteReceiveRequestCodec::readString);
          final String paymentRequestId =
              readField(decoder, OfflineNoteReceiveRequestCodec::readString);
          final String accountId = readField(decoder, OfflineNoteReceiveRequestCodec::readString);
          final String assetDefinitionId =
              readField(decoder, OfflineNoteReceiveRequestCodec::readString);
          final String assetId = readField(decoder, OfflineNoteReceiveRequestCodec::readString);
          final String amount = readField(decoder, OfflineNoteReceiveRequestCodec::readString);
          final OfflineNote.KeyCertificate keyCertificate =
              OfflineNote.decodeCertificate(
                  readField(decoder, OfflineNoteReceiveRequestCodec::readBytesVec));
          final byte[] outputCommitment = readField(decoder, child -> child.readBytes(32));
          return new OfflineNoteReceiveRequest(
              chainId,
              paymentRequestId,
              accountId,
              assetDefinitionId,
              assetId,
              amount,
              keyCertificate,
              outputCommitment);
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
      throw new IllegalArgumentException("Offline Note receive request field length overflow");
    }
    final NoritoDecoder child =
        new NoritoDecoder(decoder.readBytes((int) length), decoder.flags(), decoder.flagsHint());
    final T value = read.read(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException(
          "Trailing bytes after Offline Note receive request field decode");
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
      throw new IllegalArgumentException("Offline Note receive request string length overflow");
    }
    final String value = new String(decoder.readBytes((int) length), StandardCharsets.UTF_8);
    if (value.trim().isEmpty()) {
      throw new IllegalArgumentException("Offline Note receive request string must not be blank");
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
      throw new IllegalArgumentException("Offline Note receive request bytes length overflow");
    }
    return decoder.readBytes((int) length);
  }
}
