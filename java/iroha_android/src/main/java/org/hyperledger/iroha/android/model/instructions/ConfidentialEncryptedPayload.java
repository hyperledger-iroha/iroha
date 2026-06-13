package org.hyperledger.iroha.android.model.instructions;

import java.io.ByteArrayOutputStream;
import java.util.Arrays;
import java.util.Objects;
import org.bouncycastle.crypto.agreement.X25519Agreement;
import org.bouncycastle.crypto.params.X25519PrivateKeyParameters;
import org.bouncycastle.crypto.params.X25519PublicKeyParameters;

/**
 * X25519/XChaCha20-Poly1305 encrypted note payload carried by {@code zk::Shield}.
 *
 * <p>Ciphertext is capped at {@link #MAX_CIPHERTEXT_BYTES} because the encrypted payload is a
 * compact note descriptor, not an arbitrary attachment channel.
 */
public final class ConfidentialEncryptedPayload {
  public static final int VERSION_V1 = 1;
  public static final int MAX_CIPHERTEXT_BYTES = 64 * 1024;
  private static final byte[] LOW_ORDER_X25519_CHECK_PRIVATE_KEY = fill((byte) 1, 32);

  private final int version;
  private final byte[] ephemeralPublicKey;
  private final byte[] nonce;
  private final byte[] ciphertext;

  public ConfidentialEncryptedPayload(
      final byte[] ephemeralPublicKey, final byte[] nonce, final byte[] ciphertext) {
    this(VERSION_V1, ephemeralPublicKey, nonce, ciphertext);
  }

  public ConfidentialEncryptedPayload(
      final int version,
      final byte[] ephemeralPublicKey,
      final byte[] nonce,
      final byte[] ciphertext) {
    if (version != VERSION_V1) {
      throw new IllegalArgumentException("version must be " + VERSION_V1);
    }
    this.version = version;
    this.ephemeralPublicKey =
        ZkInstructionUtils.fixedBytes(ephemeralPublicKey, 32, "ephemeralPublicKey");
    if (isLowOrderX25519PublicKey(this.ephemeralPublicKey)) {
      throw new IllegalArgumentException("ephemeralPublicKey must not be low-order");
    }
    this.nonce = ZkInstructionUtils.fixedBytes(nonce, 24, "nonce");
    this.ciphertext = ZkInstructionUtils.copyNonEmpty(ciphertext, "ciphertext");
    if (this.ciphertext.length > MAX_CIPHERTEXT_BYTES) {
      throw new IllegalArgumentException(
          "ciphertext must not exceed " + MAX_CIPHERTEXT_BYTES + " bytes");
    }
  }

  public int version() {
    return version;
  }

  public byte[] ephemeralPublicKey() {
    return ephemeralPublicKey.clone();
  }

  public byte[] nonce() {
    return nonce.clone();
  }

  public byte[] ciphertext() {
    return ciphertext.clone();
  }

  public byte[] toWireBytes() {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(version);
    write(out, ephemeralPublicKey);
    write(out, nonce);
    writeCompactVarint(ciphertext.length, out);
    write(out, ciphertext);
    return out.toByteArray();
  }

  public byte[] wireBytes() {
    return toWireBytes();
  }

  public static ConfidentialEncryptedPayload fromWireBytes(final byte[] bytes) {
    if (bytes == null) {
      throw new IllegalArgumentException("bytes must be provided");
    }
    if (bytes.length < 1 + 32 + 24) {
      throw new IllegalArgumentException("confidential encrypted payload is truncated");
    }
    final int version = bytes[0] & 0xff;
    final byte[] ephemeral = Arrays.copyOfRange(bytes, 1, 33);
    final byte[] nonce = Arrays.copyOfRange(bytes, 33, 57);
    final Varint ciphertextLength = readCompactVarint(bytes, 57);
    if (ciphertextLength.value > MAX_CIPHERTEXT_BYTES) {
      throw new IllegalArgumentException(
          "ciphertext must not exceed " + MAX_CIPHERTEXT_BYTES + " bytes");
    }
    final int ciphertextStart = 57 + ciphertextLength.encodedBytes;
    final int ciphertextEnd = ciphertextStart + ciphertextLength.value;
    if (ciphertextEnd > bytes.length) {
      throw new IllegalArgumentException(
          "confidential encrypted payload ciphertext is truncated");
    }
    if (ciphertextEnd != bytes.length) {
      throw new IllegalArgumentException("confidential encrypted payload has trailing bytes");
    }
    return new ConfidentialEncryptedPayload(
        version, ephemeral, nonce, Arrays.copyOfRange(bytes, ciphertextStart, ciphertextEnd));
  }

  public static ConfidentialEncryptedPayload decodeWire(final byte[] bytes) {
    return fromWireBytes(bytes);
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof ConfidentialEncryptedPayload other)) {
      return false;
    }
    return version == other.version
        && Arrays.equals(ephemeralPublicKey, other.ephemeralPublicKey)
        && Arrays.equals(nonce, other.nonce)
        && Arrays.equals(ciphertext, other.ciphertext);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        version,
        Arrays.hashCode(ephemeralPublicKey),
        Arrays.hashCode(nonce),
        Arrays.hashCode(ciphertext));
  }

  private static void writeCompactVarint(final int value, final ByteArrayOutputStream out) {
    if (value < 0) {
      throw new IllegalArgumentException("compact varint value must be non-negative");
    }
    int remaining = value;
    while (true) {
      int next = remaining & 0x7f;
      remaining >>>= 7;
      if (remaining != 0) {
        next |= 0x80;
      }
      out.write(next);
      if (remaining == 0) {
        return;
      }
    }
  }

  private static Varint readCompactVarint(final byte[] bytes, final int offset) {
    int value = 0;
    int shift = 0;
    int cursor = offset;
    while (cursor < bytes.length && shift < 28) {
      final int next = bytes[cursor] & 0xff;
      value |= (next & 0x7f) << shift;
      cursor++;
      if ((next & 0x80) == 0) {
        final int encodedBytes = cursor - offset;
        if (encodedBytes > 1 && value < (1 << (7 * (encodedBytes - 1)))) {
          throw new IllegalArgumentException(
              "non-canonical confidential encrypted payload length");
        }
        return new Varint(value, encodedBytes);
      }
      shift += 7;
    }
    throw new IllegalArgumentException("invalid confidential encrypted payload length");
  }

  private static void write(final ByteArrayOutputStream out, final byte[] bytes) {
    out.write(bytes, 0, bytes.length);
  }

  private static boolean isLowOrderX25519PublicKey(final byte[] publicKey) {
    final X25519PublicKeyParameters peer =
        new X25519PublicKeyParameters(
            ZkInstructionUtils.fixedBytes(publicKey, 32, "ephemeralPublicKey"), 0);
    final X25519PrivateKeyParameters probe =
        new X25519PrivateKeyParameters(LOW_ORDER_X25519_CHECK_PRIVATE_KEY, 0);
    final X25519Agreement agreement = new X25519Agreement();
    final byte[] shared = new byte[32];
    try {
      agreement.init(probe);
      agreement.calculateAgreement(peer, shared, 0);
      return ZkInstructionUtils.isAllZero(shared);
    } catch (final IllegalStateException ignored) {
      return true;
    } finally {
      Arrays.fill(shared, (byte) 0);
    }
  }

  private static byte[] fill(final byte value, final int size) {
    final byte[] bytes = new byte[size];
    Arrays.fill(bytes, value);
    return bytes;
  }

  private static final class Varint {
    private final int value;
    private final int encodedBytes;

    private Varint(final int value, final int encodedBytes) {
      this.value = value;
      this.encodedBytes = encodedBytes;
    }
  }
}
