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
    final String normalizedBackend = backend == null ? "" : backend.trim();
    if (normalizedBackend.isEmpty()) {
      throw new IllegalArgumentException("backend must not be blank");
    }
    if (bytes == null || bytes.length == 0) {
      throw new IllegalArgumentException("bytes must not be empty");
    }
    return NoritoCodec.encode(
        new VerifyingKeyBox(normalizedBackend, bytes.clone()),
        SCHEMA,
        ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  public static byte[] encode(final String backend, final byte[] bytes) {
    return encodeNorito(backend, bytes);
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
          throw new UnsupportedOperationException("VerifyingKeyBox decoding is not supported");
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

  private static boolean compact(final NoritoEncoder encoder) {
    return (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
  }

  private interface FieldWriter {
    void write(NoritoEncoder encoder);
  }

  private static final class VerifyingKeyBox {
    private final String backend;
    private final byte[] bytes;

    private VerifyingKeyBox(final String backend, final byte[] bytes) {
      this.backend = backend;
      this.bytes = bytes;
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
