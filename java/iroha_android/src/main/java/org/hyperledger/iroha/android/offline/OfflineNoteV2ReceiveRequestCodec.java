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

/** QR/Norito handoff codec for Offline Note V2 receive requests. */
public final class OfflineNoteV2ReceiveRequestCodec {
  public static final String TYPE = "offline_receive_request_v2";
  public static final long VERSION = 2L;
  public static final String TEXT_PREFIX = "wallet-offline-receive-v2:";
  private static final String RECEIVE_REQUEST_ENVELOPE_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteReceiveRequestEnvelopeV2";

  private OfflineNoteV2ReceiveRequestCodec() {}

  public static byte[] encodeNorito(final OfflineNoteV2ReceiveRequest request) {
    return NoritoCodec.encode(
        Objects.requireNonNull(request, "request"),
        RECEIVE_REQUEST_ENVELOPE_SCHEMA,
        RECEIVE_REQUEST_ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  public static OfflineNoteV2ReceiveRequest decodeNorito(final byte[] payload) {
    return NoritoCodec.decode(
        Objects.requireNonNull(payload, "payload"),
        RECEIVE_REQUEST_ADAPTER,
        RECEIVE_REQUEST_ENVELOPE_SCHEMA);
  }

  public static String encodeText(final OfflineNoteV2ReceiveRequest request) {
    return TEXT_PREFIX
        + Base64.getUrlEncoder().withoutPadding().encodeToString(encodeNorito(request));
  }

  public static OfflineNoteV2ReceiveRequest decodeText(final String text) {
    final String trimmed = Objects.requireNonNull(text, "text").trim();
    if (!trimmed.startsWith(TEXT_PREFIX)) {
      throw new IllegalArgumentException("Offline Note V2 receive request prefix missing");
    }
    return decodeNorito(Base64.getUrlDecoder().decode(trimmed.substring(TEXT_PREFIX.length())));
  }

  public static List<byte[]> encodeQrFrameBytes(final OfflineNoteV2ReceiveRequest request) {
    return encodeQrFrameBytes(request, new OfflineQrStream.Options());
  }

  public static List<byte[]> encodeQrFrameBytes(
      final OfflineNoteV2ReceiveRequest request, final OfflineQrStream.Options options) {
    return OfflineQrStream.Encoder.encodeFrameBytes(
        encodeNorito(request),
        OfflineQrStream.PayloadKind.OFFLINE_RECEIVE_REQUEST_V2,
        options);
  }

  public static OfflineNoteV2ReceiveRequest decodeQrPayload(final byte[] payload) {
    return decodeNorito(payload);
  }

  private static final TypeAdapter<OfflineNoteV2ReceiveRequest> RECEIVE_REQUEST_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final OfflineNoteV2ReceiveRequest value) {
          writeField(encoder, child -> child.writeUInt(VERSION, 64));
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
        public OfflineNoteV2ReceiveRequest decode(final NoritoDecoder decoder) {
          final long version = readField(decoder, child -> child.readUInt(64));
          if (version != VERSION) {
            throw new IllegalArgumentException(
                "Offline Note V2 receive request Norito version must be " + VERSION);
          }
          final String chainId = readField(decoder, OfflineNoteV2ReceiveRequestCodec::readString);
          final String paymentRequestId =
              readField(decoder, OfflineNoteV2ReceiveRequestCodec::readString);
          final String accountId = readField(decoder, OfflineNoteV2ReceiveRequestCodec::readString);
          final String assetDefinitionId =
              readField(decoder, OfflineNoteV2ReceiveRequestCodec::readString);
          final String assetId = readField(decoder, OfflineNoteV2ReceiveRequestCodec::readString);
          final String amount = readField(decoder, OfflineNoteV2ReceiveRequestCodec::readString);
          final OfflineNoteV2.KeyCertificateV2 keyCertificate =
              OfflineNoteV2.decodeCertificate(
                  readField(decoder, OfflineNoteV2ReceiveRequestCodec::readBytesVec));
          final byte[] outputCommitment = readField(decoder, child -> child.readBytes(32));
          return new OfflineNoteV2ReceiveRequest(
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
      throw new IllegalArgumentException("Offline Note V2 receive request field length overflow");
    }
    final NoritoDecoder child =
        new NoritoDecoder(decoder.readBytes((int) length), decoder.flags(), decoder.flagsHint());
    final T value = read.read(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException(
          "Trailing bytes after Offline Note V2 receive request field decode");
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
      throw new IllegalArgumentException("Offline Note V2 receive request string length overflow");
    }
    final String value = new String(decoder.readBytes((int) length), StandardCharsets.UTF_8);
    if (value.trim().isEmpty()) {
      throw new IllegalArgumentException("Offline Note V2 receive request string must not be blank");
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
      throw new IllegalArgumentException("Offline Note V2 receive request bytes length overflow");
    }
    return decoder.readBytes((int) length);
  }
}
