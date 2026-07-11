package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Objects;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Norito codec for the accepted-operation reference. */
public final class OfflineOperationCodec {
  private static final String SCHEMA =
      "iroha_torii_shared::offline_api::OfflineOperationReference";
  private static final String STATUS_SCHEMA =
      "iroha_torii_shared::offline_api::OfflineOperationStatus";
  private static final String TOP_UP_ANCHOR_SCHEMA =
      "iroha_data_model::offline::model::KagemushaRecursiveSpendTopUpAnchorV2";
  private static final String TOP_UP_REQUEST_SCHEMA =
      "iroha.torii.v1.offline.top_up.request";
  private static final String REDEEM_REQUEST_SCHEMA =
      "iroha.torii.v1.offline.redeem.request";
  private static final int STATUS_HEADER_PADDING = 8;
  private static final char[] LOWER_HEX = "0123456789abcdef".toCharArray();

  private OfflineOperationCodec() {}

  /** Decode an accepted-operation reference returned by Torii. */
  public static OfflineOperationReference decodeReference(final byte[] archive) {
    return NoritoCodec.decode(
        Arrays.copyOf(Objects.requireNonNull(archive, "archive"), archive.length),
        REFERENCE_ADAPTER,
        SCHEMA);
  }

  /** Encode an accepted-operation reference using the canonical Norito layout. */
  public static byte[] encodeReference(final OfflineOperationReference reference) {
    return NoritoCodec.encode(reference, SCHEMA, REFERENCE_ADAPTER, NoritoHeader.COMPACT_LEN);
  }

  /** Decode a schema-bound typed operation status returned by Torii. */
  public static OfflineOperationStatus decodeStatus(final byte[] archive) {
    requireStatusPadding(archive);
    return NoritoCodec.decode(
        Arrays.copyOf(Objects.requireNonNull(archive, "archive"), archive.length),
        STATUS_ADAPTER,
        STATUS_SCHEMA);
  }

  /** Encode an operation status using the canonical public Norito layout. */
  public static byte[] encodeStatus(final OfflineOperationStatus status) {
    return addStatusPadding(
        NoritoCodec.encode(status, STATUS_SCHEMA, STATUS_ADAPTER, NoritoHeader.COMPACT_LEN));
  }

  public static String requireOperationId(final String value) {
    Objects.requireNonNull(value, "operationId");
    if (value.length() != 64) {
      throw new IllegalArgumentException(
          "operationId must be 64 lowercase hexadecimal characters");
    }
    boolean nonZero = false;
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (!((character >= '0' && character <= '9')
          || (character >= 'a' && character <= 'f'))) {
        throw new IllegalArgumentException(
            "operationId must be 64 lowercase hexadecimal characters");
      }
      nonZero |= character != '0';
    }
    if (!nonZero) {
      throw new IllegalArgumentException("operationId must be non-zero");
    }
    return value;
  }

  static String requireTransactionHash(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.length() != 64) {
      throw new IllegalArgumentException(
          field + " must be exactly 32 bytes encoded as lowercase hexadecimal");
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (!((character >= '0' && character <= '9')
          || (character >= 'a' && character <= 'f'))) {
        throw new IllegalArgumentException(
            field + " must be exactly 32 bytes encoded as lowercase hexadecimal");
      }
    }
    return value;
  }

  static String requireOperationStatusUri(final String value, final String operationId) {
    Objects.requireNonNull(value, "statusUri");
    final String expected = "/v1/offline/operations/" + requireOperationId(operationId);
    if (!value.equals(expected)) {
      throw new IllegalArgumentException(
          "statusUri must equal the canonical operation resource " + expected);
    }
    return value;
  }

  static String requireStableErrorCode(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.isEmpty() || value.length() > 64) {
      throw new IllegalArgumentException(
          field + " must be a 1-64 character lowercase stable identifier");
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      final boolean valid =
          (character >= 'a' && character <= 'z')
              || (character >= '0' && character <= '9')
              || (index > 0 && character == '_');
      if (!valid) {
        throw new IllegalArgumentException(
            field + " must be a 1-64 character lowercase stable identifier");
      }
    }
    return value;
  }

  static CanonicalRequest requireTopUpRequest(final byte[] archive) {
    return requireCanonicalRequest(archive, TOP_UP_REQUEST_SCHEMA, 6, 8);
  }

  static CanonicalRequest requireRedeemRequest(final byte[] archive) {
    return requireCanonicalRequest(archive, REDEEM_REQUEST_SCHEMA, 9, 11);
  }

  private static CanonicalRequest requireCanonicalRequest(
      final byte[] value,
      final String schema,
      final int operationIdFieldIndex,
      final int fieldCount) {
    Objects.requireNonNull(value, "noritoArchive");
    if (value.length == 0) {
      throw new IllegalArgumentException("noritoArchive must not be empty");
    }
    final byte[] archive = Arrays.copyOf(value, value.length);
    final NoritoHeader.DecodeResult decoded =
        NoritoHeader.decode(archive, SchemaHash.hash16(schema));
    if (decoded.header().compression() != NoritoHeader.COMPRESSION_NONE) {
      throw new IllegalArgumentException("Offline request archive must not be compressed");
    }
    if (decoded.header().flags() != NoritoHeader.COMPACT_LEN) {
      throw new IllegalArgumentException(
          "Offline request root must use canonical compact sequential field framing");
    }
    if (archive.length != NoritoHeader.HEADER_LENGTH + decoded.header().payloadLength()) {
      throw new IllegalArgumentException(
          "Offline request archive must not contain header padding");
    }
    decoded.header().validateChecksum(decoded.payload());
    final NoritoDecoder decoder =
        new NoritoDecoder(
            decoded.payload(), decoded.header().flags(), decoded.header().minor());
    byte[] operationIdBytes = null;
    for (int fieldIndex = 0; fieldIndex < fieldCount; fieldIndex++) {
      final long length =
          decoder.readLength((decoder.flags() & NoritoHeader.COMPACT_LEN) != 0);
      if (length < 0 || length > Integer.MAX_VALUE) {
        throw new IllegalArgumentException("Offline request field length overflow");
      }
      final byte[] field = decoder.readBytes((int) length);
      if (fieldIndex == operationIdFieldIndex) {
        if (field.length != 32) {
          throw new IllegalArgumentException(
              "Offline request operation_id must contain exactly 32 raw bytes");
        }
        operationIdBytes = field;
      }
    }
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException(
          "Trailing fields or bytes after canonical Offline request");
    }
    if (operationIdBytes == null) {
      throw new IllegalArgumentException("Offline request operation_id field is missing");
    }
    boolean nonZero = false;
    for (final byte valueByte : operationIdBytes) {
      nonZero |= valueByte != 0;
    }
    if (!nonZero) {
      throw new IllegalArgumentException("Offline request operation_id must be non-zero");
    }
    return new CanonicalRequest(lowercaseHex(operationIdBytes), archive);
  }

  private static String lowercaseHex(final byte[] value) {
    final char[] result = new char[value.length * 2];
    for (int index = 0; index < value.length; index++) {
      final int unsigned = value[index] & 0xFF;
      result[index * 2] = LOWER_HEX[unsigned >>> 4];
      result[index * 2 + 1] = LOWER_HEX[unsigned & 0x0F];
    }
    return new String(result);
  }

  static final class CanonicalRequest {
    private final String operationId;
    private final byte[] archive;

    private CanonicalRequest(final String operationId, final byte[] archive) {
      this.operationId = operationId;
      this.archive = Arrays.copyOf(archive, archive.length);
    }

    String operationId() {
      return operationId;
    }

    byte[] archive() {
      return Arrays.copyOf(archive, archive.length);
    }
  }

  private static final TypeAdapter<OfflineOperationReference> REFERENCE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(
            final NoritoEncoder encoder, final OfflineOperationReference value) {
          writeField(encoder, child -> writeString(child, value.operationId()));
          writeField(encoder, child -> child.writeUInt(value.kind().ordinal(), 32));
          writeField(encoder, child -> child.writeUInt(value.state().ordinal(), 32));
          writeField(encoder, child -> writeString(child, value.transactionHash()));
          writeField(encoder, child -> writeString(child, value.statusUri()));
          writeField(encoder, child -> child.writeUInt(value.submittedAtMs().longValue(), 64));
        }

        @Override
        public OfflineOperationReference decode(final NoritoDecoder decoder) {
          final String operationId = readField(decoder, OfflineOperationCodec::readString);
          final long kindTag = readField(decoder, child -> child.readUInt(32));
          final OfflineOperationKind kind;
          if (kindTag == 0) {
            kind = OfflineOperationKind.TOP_UP;
          } else if (kindTag == 1) {
            kind = OfflineOperationKind.REDEEM;
          } else {
            throw new IllegalArgumentException("Invalid Offline operation kind");
          }
          final long stateTag = readField(decoder, child -> child.readUInt(32));
          if (stateTag != 0) {
            throw new IllegalArgumentException("Invalid Offline operation state");
          }
          return new OfflineOperationReference(
              operationId,
              kind,
              OfflineOperationState.PENDING,
              readField(decoder, OfflineOperationCodec::readString),
              readField(decoder, OfflineOperationCodec::readString),
              readField(decoder, child -> unsignedLongToBigInteger(child.readUInt(64))));
        }
      };

  private static final TypeAdapter<OfflineOperationStatus> STATUS_ADAPTER =
      new TypeAdapter<OfflineOperationStatus>() {
        @Override
        public void encode(final NoritoEncoder encoder, final OfflineOperationStatus value) {
          if (value instanceof OfflineOperationStatus.Pending) {
            final OfflineOperationStatus.Pending pending = (OfflineOperationStatus.Pending) value;
            encoder.writeUInt(0, 32);
            writeField(encoder, child -> writeString(child, pending.operationId()));
            writeField(encoder, child -> writeKind(child, pending.kind()));
            writeField(encoder, child -> writeString(child, pending.transactionHash()));
            writeField(encoder, child -> writeU64(child, pending.submittedAtMs()));
          } else if (value instanceof OfflineOperationStatus.Applied) {
            final OfflineOperationStatus.Applied applied = (OfflineOperationStatus.Applied) value;
            encoder.writeUInt(1, 32);
            writeField(encoder, child -> writeString(child, applied.operationId()));
            writeField(encoder, child -> writeResult(child, applied.result()));
          } else if (value instanceof OfflineOperationStatus.Rejected) {
            final OfflineOperationStatus.Rejected rejected = (OfflineOperationStatus.Rejected) value;
            encoder.writeUInt(2, 32);
            writeField(encoder, child -> writeString(child, rejected.operationId()));
            writeField(encoder, child -> writeKind(child, rejected.kind()));
            writeField(encoder, child -> writeString(child, rejected.transactionHash()));
            writeField(encoder, child -> writeError(child, rejected.error()));
          } else {
            throw new IllegalArgumentException("Unsupported Offline operation status type");
          }
        }

        @Override
        public OfflineOperationStatus decode(final NoritoDecoder decoder) {
          final long tag = decoder.readUInt(32);
          final OfflineOperationStatus status;
          if (tag == 0) {
            status =
                new OfflineOperationStatus.Pending(
                    readField(decoder, OfflineOperationCodec::readString),
                    readField(decoder, OfflineOperationCodec::readKind),
                    readField(decoder, OfflineOperationCodec::readString),
                    readField(decoder, OfflineOperationCodec::readU64));
          } else if (tag == 1) {
            status =
                new OfflineOperationStatus.Applied(
                    readField(decoder, OfflineOperationCodec::readString),
                    readField(decoder, OfflineOperationCodec::readResult));
          } else if (tag == 2) {
            status =
                new OfflineOperationStatus.Rejected(
                    readField(decoder, OfflineOperationCodec::readString),
                    readField(decoder, OfflineOperationCodec::readKind),
                    readField(decoder, OfflineOperationCodec::readString),
                    readField(decoder, OfflineOperationCodec::readError));
          } else {
            throw new IllegalArgumentException(
                "Invalid Offline operation status tag: " + tag);
          }
          return status;
        }
      };

  private static void writeResult(
      final NoritoEncoder encoder, final OfflineOperationStatus.Result result) {
    if (result instanceof OfflineOperationStatus.Result.TopUp) {
      final OfflineOperationStatus.TopUpResult value =
          ((OfflineOperationStatus.Result.TopUp) result).value();
      writeVariant(
          encoder,
          0,
          variant -> {
            writeField(variant, child -> writeString(child, value.transactionHash()));
            writeField(variant, child -> writeU64(child, value.finalizedBlockHeight()));
            writeField(variant, child -> writeU64(child, value.serverTimeMs()));
            writeField(
                variant,
                child -> {
                  final NoritoCodec.ArchiveView view =
                      NoritoCodec.fromBytesView(
                          value.anchor().noritoArchive(), TOP_UP_ANCHOR_SCHEMA);
                  if (view.flags() != encoder.flags()) {
                    throw new IllegalArgumentException(
                        "Top-up anchor flags must match operation status flags");
                  }
                  child.writeBytes(view.asBytes());
                });
          });
    } else if (result instanceof OfflineOperationStatus.Result.Redeem) {
      final OfflineOperationStatus.RedeemResult value =
          ((OfflineOperationStatus.Result.Redeem) result).value();
      writeVariant(
          encoder,
          1,
          variant -> {
            writeField(variant, child -> writeString(child, value.transactionHash()));
            writeField(variant, child -> writeU64(child, value.finalizedBlockHeight()));
            writeField(variant, child -> writeU64(child, value.serverTimeMs()));
          });
    } else {
      throw new IllegalArgumentException("Unsupported Offline operation result type");
    }
  }

  private static OfflineOperationStatus.Result readResult(final NoritoDecoder decoder) {
    final Variant variant = readVariant(decoder);
    final OfflineOperationStatus.Result result;
    if (variant.tag == 0) {
      final String transactionHash = readField(variant.decoder, OfflineOperationCodec::readString);
      final BigInteger finalizedHeight = readField(variant.decoder, OfflineOperationCodec::readU64);
      final BigInteger serverTime = readField(variant.decoder, OfflineOperationCodec::readU64);
      final byte[] anchorPayload =
          readField(variant.decoder, OfflineOperationCodec::readRemainingBytes);
      final byte[] anchorArchive =
          frameArchive(TOP_UP_ANCHOR_SCHEMA, anchorPayload, decoder.flags());
      result =
          new OfflineOperationStatus.Result.TopUp(
              new OfflineOperationStatus.TopUpResult(
                  transactionHash,
                  finalizedHeight,
                  serverTime,
                  new OfflineOperationStatus.TopUpAnchor(anchorArchive)));
    } else if (variant.tag == 1) {
      result =
          new OfflineOperationStatus.Result.Redeem(
              new OfflineOperationStatus.RedeemResult(
                  readField(variant.decoder, OfflineOperationCodec::readString),
                  readField(variant.decoder, OfflineOperationCodec::readU64),
                  readField(variant.decoder, OfflineOperationCodec::readU64)));
    } else {
      throw new IllegalArgumentException("Invalid Offline operation result tag: " + variant.tag);
    }
    requireConsumed(variant.decoder, "Offline result variant");
    return result;
  }

  private static void writeError(
      final NoritoEncoder encoder, final OfflineOperationStatus.Error error) {
    writeField(encoder, child -> writeString(child, error.code()));
    writeField(encoder, child -> writeString(child, error.message()));
    writeField(encoder, child -> writeOption(child, error.details(), OfflineOperationCodec::writeErrorDetails));
  }

  private static OfflineOperationStatus.Error readError(final NoritoDecoder decoder) {
    return new OfflineOperationStatus.Error(
        readField(decoder, OfflineOperationCodec::readString),
        readField(decoder, OfflineOperationCodec::readString),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readErrorDetails)));
  }

  private static void writeErrorDetails(
      final NoritoEncoder encoder, final OfflineOperationStatus.ErrorDetails details) {
    writeField(encoder, child -> writeOption(child, details.layer, OfflineOperationCodec::writeString));
    writeField(encoder, child -> writeOption(child, details.rejectCode, OfflineOperationCodec::writeString));
    writeField(encoder, child -> writeOption(child, details.queue, OfflineOperationCodec::writeQueue));
    writeField(encoder, child -> writeOption(child, details.retryAfterSeconds, OfflineOperationCodec::writeU64));
    writeField(encoder, child -> writeOption(child, details.endpoint, OfflineOperationCodec::writeString));
    writeField(encoder, child -> writeOption(child, details.field, OfflineOperationCodec::writeString));
    writeField(encoder, child -> writeOption(child, details.expected, OfflineOperationCodec::writeString));
    writeField(encoder, child -> writeOption(child, details.actual, OfflineOperationCodec::writeString));
    writeField(encoder, child -> writeOption(child, details.profile, OfflineOperationCodec::writeString));
    writeField(
        encoder,
        child ->
            writeOption(
                child,
                details.chainDiscriminant,
                (out, value) -> out.writeUInt(value.longValue(), 16)));
    writeField(encoder, child -> writeOption(child, details.transactionHash, OfflineOperationCodec::writeString));
    writeField(encoder, child -> writeOption(child, details.lastStatus, OfflineOperationCodec::writeString));
    writeField(encoder, child -> writeOption(child, details.hint, OfflineOperationCodec::writeString));
    writeField(encoder, child -> writeOption(child, details.axt, OfflineOperationCodec::writeAxt));
  }

  private static OfflineOperationStatus.ErrorDetails readErrorDetails(
      final NoritoDecoder decoder) {
    return new OfflineOperationStatus.ErrorDetails(
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readString)),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readString)),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readQueue)),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readU64)),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readString)),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readString)),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readString)),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readString)),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readString)),
        readField(decoder, child -> readOption(child, value -> (int) value.readUInt(16))),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readString)),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readString)),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readString)),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readAxt)));
  }

  private static void writeQueue(
      final NoritoEncoder encoder, final OfflineOperationStatus.QueueErrorSnapshot queue) {
    writeField(encoder, child -> writeString(child, queue.state));
    writeField(encoder, child -> writeU64(child, queue.queued));
    writeField(encoder, child -> writeU64(child, queue.capacity));
    writeField(encoder, child -> child.writeByte(queue.saturated ? 1 : 0));
  }

  private static OfflineOperationStatus.QueueErrorSnapshot readQueue(
      final NoritoDecoder decoder) {
    return new OfflineOperationStatus.QueueErrorSnapshot(
        readField(decoder, OfflineOperationCodec::readString),
        readField(decoder, OfflineOperationCodec::readU64),
        readField(decoder, OfflineOperationCodec::readU64),
        readField(decoder, OfflineOperationCodec::readBool));
  }

  private static void writeAxt(
      final NoritoEncoder encoder, final OfflineOperationStatus.AxtErrorDetails axt) {
    writeField(encoder, child -> writeOption(child, axt.code, OfflineOperationCodec::writeString));
    writeField(encoder, child -> writeOption(child, axt.reason, OfflineOperationCodec::writeString));
    writeField(encoder, child -> writeOption(child, axt.snapshotVersion, OfflineOperationCodec::writeU64));
    writeField(encoder, child -> writeOption(child, axt.dataspace, OfflineOperationCodec::writeU64));
    writeField(
        encoder,
        child -> writeOption(child, axt.lane, (out, value) -> out.writeUInt(value, 32)));
    writeField(encoder, child -> writeOption(child, axt.nextMinHandleEra, OfflineOperationCodec::writeU64));
    writeField(encoder, child -> writeOption(child, axt.nextMinSubNonce, OfflineOperationCodec::writeU64));
  }

  private static OfflineOperationStatus.AxtErrorDetails readAxt(
      final NoritoDecoder decoder) {
    return new OfflineOperationStatus.AxtErrorDetails(
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readString)),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readString)),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readU64)),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readU64)),
        readField(decoder, child -> readOption(child, value -> value.readUInt(32))),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readU64)),
        readField(decoder, child -> readOption(child, OfflineOperationCodec::readU64)));
  }

  private interface FieldWriter {
    void write(NoritoEncoder encoder);
  }

  private interface FieldReader<T> {
    T read(NoritoDecoder decoder);
  }

  private interface ValueWriter<T> {
    void write(NoritoEncoder encoder, T value);
  }

  private static final class Variant {
    private final long tag;
    private final NoritoDecoder decoder;

    private Variant(final long tag, final NoritoDecoder decoder) {
      this.tag = tag;
      this.decoder = decoder;
    }
  }

  private static void writeField(final NoritoEncoder encoder, final FieldWriter write) {
    final NoritoEncoder child = encoder.childEncoder();
    write.write(child);
    final byte[] payload = child.toByteArray();
    encoder.writeLength(payload.length, true);
    encoder.writeBytes(payload);
  }

  private static <T> T readField(
      final NoritoDecoder decoder, final FieldReader<T> read) {
    final long length = decoder.readLength(true);
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Offline operation field length overflow");
    }
    final NoritoDecoder child =
        new NoritoDecoder(
            decoder.readBytes((int) length), decoder.flags(), decoder.flagsHint());
    final T value = read.read(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after Offline operation field");
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
      throw new IllegalArgumentException("Offline operation string length overflow");
    }
    final byte[] bytes = decoder.readBytes((int) length);
    final String value;
    try {
      value =
          StandardCharsets.UTF_8
              .newDecoder()
              .onMalformedInput(CodingErrorAction.REPORT)
              .onUnmappableCharacter(CodingErrorAction.REPORT)
              .decode(ByteBuffer.wrap(bytes))
              .toString();
    } catch (final CharacterCodingException error) {
      throw new IllegalArgumentException("Offline operation string must be valid UTF-8", error);
    }
    if (value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException("Offline operation string must be exact non-empty text");
    }
    return value;
  }

  private static void writeKind(
      final NoritoEncoder encoder, final OfflineOperationKind kind) {
    encoder.writeUInt(kind.ordinal(), 32);
  }

  private static OfflineOperationKind readKind(final NoritoDecoder decoder) {
    final long tag = decoder.readUInt(32);
    if (tag == 0) {
      return OfflineOperationKind.TOP_UP;
    }
    if (tag == 1) {
      return OfflineOperationKind.REDEEM;
    }
    throw new IllegalArgumentException("Invalid Offline operation kind");
  }

  private static void writeU64(final NoritoEncoder encoder, final BigInteger value) {
    encoder.writeUInt(value.longValue(), 64);
  }

  private static BigInteger readU64(final NoritoDecoder decoder) {
    return unsignedLongToBigInteger(decoder.readUInt(64));
  }

  private static boolean readBool(final NoritoDecoder decoder) {
    final int value = decoder.readByte();
    if (value == 0) {
      return false;
    }
    if (value == 1) {
      return true;
    }
    throw new IllegalArgumentException("Invalid boolean value: " + value);
  }

  private static <T> void writeOption(
      final NoritoEncoder encoder, final T value, final ValueWriter<T> write) {
    if (value == null) {
      encoder.writeByte(0);
      return;
    }
    encoder.writeByte(1);
    final NoritoEncoder child = encoder.childEncoder();
    write.write(child, value);
    final byte[] payload = child.toByteArray();
    encoder.writeLength(payload.length, compact(encoder.flags()));
    encoder.writeBytes(payload);
  }

  private static <T> T readOption(
      final NoritoDecoder decoder, final FieldReader<T> read) {
    final int tag = decoder.readByte();
    if (tag == 0) {
      return null;
    }
    if (tag != 1) {
      throw new IllegalArgumentException("Invalid Offline option tag: " + tag);
    }
    final long length = decoder.readLength(compact(decoder.flags()));
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Offline option payload is too large");
    }
    final NoritoDecoder child =
        new NoritoDecoder(
            decoder.readBytes((int) length), decoder.flags(), decoder.flagsHint());
    final T value = read.read(child);
    requireConsumed(child, "Offline option");
    return value;
  }

  private static void writeVariant(
      final NoritoEncoder encoder, final long tag, final FieldWriter write) {
    encoder.writeUInt(tag, 32);
    final NoritoEncoder child = encoder.childEncoder();
    write.write(child);
    final byte[] payload = child.toByteArray();
    encoder.writeLength(payload.length, compact(encoder.flags()));
    encoder.writeBytes(payload);
  }

  private static Variant readVariant(final NoritoDecoder decoder) {
    final long tag = decoder.readUInt(32);
    final long length = decoder.readLength(compact(decoder.flags()));
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Offline variant payload is too large");
    }
    return new Variant(
        tag,
        new NoritoDecoder(
            decoder.readBytes((int) length), decoder.flags(), decoder.flagsHint()));
  }

  private static byte[] frameArchive(
      final String schema, final byte[] payload, final int flags) {
    final NoritoHeader header =
        new NoritoHeader(
            SchemaHash.hash16(schema),
            payload.length,
            CRC64.compute(payload),
            flags,
            NoritoHeader.COMPRESSION_NONE);
    final byte[] headerBytes = header.encode();
    final byte[] archive = Arrays.copyOf(headerBytes, headerBytes.length + payload.length);
    System.arraycopy(payload, 0, archive, headerBytes.length, payload.length);
    return archive;
  }

  private static byte[] readRemainingBytes(final NoritoDecoder decoder) {
    return decoder.readBytes(decoder.remaining());
  }

  private static boolean compact(final int flags) {
    return (flags & NoritoHeader.COMPACT_LEN) != 0;
  }

  private static void requireConsumed(final NoritoDecoder decoder, final String field) {
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after " + field);
    }
  }

  private static void requireStatusPadding(final byte[] archive) {
    final NoritoHeader.DecodeResult decoded =
        NoritoHeader.decode(archive, SchemaHash.hash16(STATUS_SCHEMA));
    final int padding =
        archive.length - NoritoHeader.HEADER_LENGTH - decoded.header().payloadLength();
    if (padding != STATUS_HEADER_PADDING) {
      throw new IllegalArgumentException(
          "Offline operation status must contain canonical 8-byte enum alignment padding");
    }
  }

  private static byte[] addStatusPadding(final byte[] archive) {
    final byte[] padded = new byte[archive.length + STATUS_HEADER_PADDING];
    System.arraycopy(archive, 0, padded, 0, NoritoHeader.HEADER_LENGTH);
    System.arraycopy(
        archive,
        NoritoHeader.HEADER_LENGTH,
        padded,
        NoritoHeader.HEADER_LENGTH + STATUS_HEADER_PADDING,
        archive.length - NoritoHeader.HEADER_LENGTH);
    return padded;
  }

  private static BigInteger unsignedLongToBigInteger(final long value) {
    if (value >= 0) {
      return BigInteger.valueOf(value);
    }
    return BigInteger.valueOf(value & Long.MAX_VALUE).setBit(63);
  }
}
