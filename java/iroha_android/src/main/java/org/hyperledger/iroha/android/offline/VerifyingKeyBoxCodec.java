package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Norito encoder for chain-supplied Offline Note VerifyingKeyBox records. */
public final class VerifyingKeyBoxCodec {
  private static final String SCHEMA = "iroha_data_model::proof::VerifyingKeyBox";

  private VerifyingKeyBoxCodec() {}

  public static byte[] encodeNorito(final String backend, final byte[] bytes) {
    return NoritoCodec.encode(
        new VerifyingKeyBox(backend, bytes),
        SCHEMA,
        ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  public static byte[] encode(final String backend, final byte[] bytes) {
    return encodeNorito(backend, bytes);
  }

  public static VerifyingKeyBox decodeNorito(final byte[] payload) {
    return NoritoCodec.decode(payload, ADAPTER, SCHEMA);
  }

  public static VerifyingKeyBox decode(final byte[] payload) {
    return decodeNorito(payload);
  }

  private static final TypeAdapter<VerifyingKeyBox> ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final VerifyingKeyBox value) {
          writeField(encoder, child -> writeString(child, value.backend));
          writeField(encoder, child -> writeBytesVec(child, value.bytes));
        }

        @Override
        public VerifyingKeyBox decode(final NoritoDecoder decoder) {
          return new VerifyingKeyBox(
              readField(decoder, VerifyingKeyBoxCodec::readString),
              readField(decoder, VerifyingKeyBoxCodec::readBytesVec));
        }
      };

  private static void writeField(final NoritoEncoder parent, final FieldWriter writer) {
    final NoritoEncoder child = parent.childEncoder();
    writer.write(child);
    final byte[] payload = child.toByteArray();
    parent.writeLength(payload.length, compact(parent));
    parent.writeBytes(payload);
  }

  private static void writeString(final NoritoEncoder encoder, final String value) {
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    encoder.writeLength(bytes.length, compact(encoder));
    encoder.writeBytes(bytes);
  }

  private static void writeBytesVec(final NoritoEncoder encoder, final byte[] bytes) {
    encoder.writeUInt(bytes.length, 64);
    encoder.writeBytes(bytes);
  }

  private static <T> T readField(
      final NoritoDecoder parent, final FieldReader<T> readPayload) {
    final int length = checkedLength(parent.readLength(compact(parent)), "field length");
    final NoritoDecoder child =
        new NoritoDecoder(parent.readBytes(length), parent.flags(), parent.flagsHint());
    final T value = readPayload.read(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after VerifyingKeyBox field decode");
    }
    return value;
  }

  private static String readString(final NoritoDecoder decoder) {
    final int length = checkedLength(decoder.readLength(compact(decoder)), "string length");
    return new String(decoder.readBytes(length), StandardCharsets.UTF_8);
  }

  private static byte[] readBytesVec(final NoritoDecoder decoder) {
    final int length = checkedLength(decoder.readUInt(64), "byte vector length");
    return decoder.readBytes(length);
  }

  private static int checkedLength(final long value, final String field) {
    if (value < 0) {
      throw new IllegalArgumentException(field + " must be non-negative");
    }
    if (value > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(field + " exceeds JVM array limit");
    }
    return (int) value;
  }

  private static boolean compact(final NoritoEncoder encoder) {
    return (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
  }

  private static boolean compact(final NoritoDecoder decoder) {
    return (decoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
  }

  private static String requireNonBlankUnpadded(final String value, final String field) {
    if (value == null || value.trim().isEmpty()) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
    if (!value.trim().equals(value)) {
      throw new IllegalArgumentException(field + " must not contain surrounding whitespace");
    }
    return value;
  }

  private interface FieldWriter {
    void write(NoritoEncoder encoder);
  }

  private interface FieldReader<T> {
    T read(NoritoDecoder decoder);
  }

  public static final class VerifyingKeyBox {
    private final String backend;
    private final byte[] bytes;

    private VerifyingKeyBox(final String backend, final byte[] bytes) {
      this.backend = requireNonBlankUnpadded(backend, "backend");
      if (bytes == null || bytes.length == 0) {
        throw new IllegalArgumentException("bytes must not be empty");
      }
      this.bytes = bytes.clone();
    }

    public String backend() {
      return backend;
    }

    public byte[] bytes() {
      return bytes.clone();
    }

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof VerifyingKeyBox)) {
        return false;
      }
      final VerifyingKeyBox that = (VerifyingKeyBox) other;
      return backend.equals(that.backend) && Arrays.equals(bytes, that.bytes);
    }

    @Override
    public int hashCode() {
      return 31 * backend.hashCode() + Arrays.hashCode(bytes);
    }
  }
}
