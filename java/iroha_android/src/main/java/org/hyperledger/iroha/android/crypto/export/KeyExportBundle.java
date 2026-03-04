package org.hyperledger.iroha.android.crypto.export;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;
import java.util.Objects;

/**
 * Encapsulates the deterministic representation of an exported key.
 *
 * <p>The bundle layout is stable and versioned so future tooling (Rust bindings, Android clients) can
 * decode the same payload deterministically:
 *
 * <pre>
 * magic[5] = "IRKEY"
 * version[1] = 0x03
 * alias_len[2] (big-endian)
 * alias_bytes
 * public_key_len[2]
 * public_key_bytes
 * nonce_len[1]
 * nonce_bytes
 * ciphertext_len[2]
 * ciphertext_bytes (GCM ciphertext + tag)
 * salt_len[1] (v2+)
 * salt_bytes (v2+)
 * kdf_kind[1] (v2+)
 * kdf_work_factor[4] (big-endian, v2+)
 * </pre>
 */
public final class KeyExportBundle {
  private static final byte[] MAGIC = "IRKEY".getBytes(StandardCharsets.UTF_8);
  public static final byte VERSION_V3 = 3;
  public static final int EXPECTED_NONCE_LENGTH_BYTES = 12;
  public static final int EXPECTED_SALT_LENGTH_BYTES = 16;

  private final String alias;
  private final byte[] publicKey;
  private final byte[] nonce;
  private final byte[] ciphertext;
  private final byte[] salt;
  private final int kdfKind;
  private final int kdfWorkFactor;
  private final byte version;

  KeyExportBundle(
      final String alias,
      final byte[] publicKey,
      final byte[] nonce,
      final byte[] ciphertext,
      final byte[] salt,
      final int kdfKind,
      final int kdfWorkFactor,
      final byte version) {
    this.alias = Objects.requireNonNull(alias, "alias");
    this.publicKey = publicKey.clone();
    this.nonce = nonce.clone();
    this.ciphertext = ciphertext.clone();
    this.salt = salt == null ? new byte[0] : salt.clone();
    this.kdfKind = kdfKind;
    this.kdfWorkFactor = kdfWorkFactor;
    this.version = version;
  }

  public String alias() {
    return alias;
  }

  public byte[] publicKey() {
    return publicKey.clone();
  }

  public byte[] nonce() {
    return nonce.clone();
  }

  public byte[] ciphertext() {
    return ciphertext.clone();
  }

  public byte[] salt() {
    return salt.clone();
  }

  public int kdfKind() {
    return kdfKind;
  }

  public int kdfWorkFactor() {
    return kdfWorkFactor;
  }

  public byte version() {
    return version;
  }

  /** Serializes the bundle to a base64 string with the canonical layout. */
  public String encodeBase64() {
    final byte[] encoded = encode();
    return Base64.getEncoder().encodeToString(encoded);
  }

  /** Serializes the bundle to its canonical byte representation. */
  public byte[] encode() {
    final byte[] aliasBytes = alias.getBytes(StandardCharsets.UTF_8);
    if (aliasBytes.length > 0xFFFF) {
      throw new IllegalArgumentException("alias is too long");
    }
    if (version != VERSION_V3) {
      throw new IllegalArgumentException("unsupported key export version: " + version);
    }
    if (publicKey.length > 0xFFFF) {
      throw new IllegalArgumentException("publicKey is too long");
    }
    if (ciphertext.length > 0xFFFF) {
      throw new IllegalArgumentException("ciphertext is too long");
    }
    if (nonce.length != EXPECTED_NONCE_LENGTH_BYTES) {
      throw new IllegalArgumentException(
          "nonce must be "
              + EXPECTED_NONCE_LENGTH_BYTES
              + " bytes, found "
              + nonce.length);
    }
    if (salt.length != EXPECTED_SALT_LENGTH_BYTES) {
      throw new IllegalArgumentException(
          "salt must be "
              + EXPECTED_SALT_LENGTH_BYTES
              + " bytes, found "
              + salt.length);
    }
    if (kdfWorkFactor < 0) {
      throw new IllegalArgumentException("kdfWorkFactor must be non-negative");
    }

    final ByteArrayOutputStream output = new ByteArrayOutputStream();
    try {
      output.write(MAGIC);
      output.write(version);
      output.write(shortBytes(aliasBytes.length));
      output.write(aliasBytes);
      output.write(shortBytes(publicKey.length));
      output.write(publicKey);
      output.write(nonce.length);
      output.write(nonce);
      output.write(shortBytes(ciphertext.length));
      output.write(ciphertext);
      output.write(salt.length);
      output.write(salt);
      output.write(kdfKind & 0xFF);
      output.write(intBytes(kdfWorkFactor));
    } catch (final IOException ex) {
      throw new IllegalStateException("Unexpected I/O error writing bundle", ex);
    }
    return output.toByteArray();
  }

  /** Decodes a bundle from the canonical base64 representation. */
  public static KeyExportBundle decodeBase64(final String encoded) throws KeyExportException {
    Objects.requireNonNull(encoded, "encoded");
    try {
      final byte[] raw = Base64.getDecoder().decode(encoded);
      return decode(raw);
    } catch (final IllegalArgumentException ex) {
      throw new KeyExportException("Key export bundle is not valid base64", ex);
    }
  }

  /** Decodes a bundle from the canonical byte representation. */
  public static KeyExportBundle decode(final byte[] encoded) throws KeyExportException {
    Objects.requireNonNull(encoded, "encoded");
    try (ByteArrayInputStream input = new ByteArrayInputStream(encoded)) {
      final byte[] magic = input.readNBytes(MAGIC.length);
      if (!Arrays.equals(MAGIC, magic)) {
        throw new KeyExportException("Key export bundle magic mismatch");
      }
      final int version = input.read();
      if (version != VERSION_V3) {
        throw new KeyExportException(
            "Unsupported key export version: " + (version < 0 ? "EOF" : version));
      }
      final int aliasLength = readShort(input);
      final byte[] aliasBytes = input.readNBytes(aliasLength);
      if (aliasBytes.length != aliasLength) {
        throw new KeyExportException("Unexpected end of stream while reading alias");
      }
      final int pubKeyLength = readShort(input);
      final byte[] pubKey = input.readNBytes(pubKeyLength);
      if (pubKey.length != pubKeyLength) {
        throw new KeyExportException("Unexpected end of stream while reading public key");
      }
      final int nonceLength = input.read();
      if (nonceLength <= 0) {
        throw new KeyExportException("Nonce must be present in export bundle");
      }
      if (nonceLength != EXPECTED_NONCE_LENGTH_BYTES) {
        throw new KeyExportException(
            "Nonce length mismatch: expected "
                + EXPECTED_NONCE_LENGTH_BYTES
                + " bytes, found "
                + nonceLength);
      }
      final byte[] nonce = input.readNBytes(nonceLength);
      if (nonce.length != nonceLength) {
        throw new KeyExportException("Unexpected end of stream while reading nonce");
      }
      final int cipherLength = readShort(input);
      final byte[] cipher = input.readNBytes(cipherLength);
      if (cipher.length != cipherLength) {
        throw new KeyExportException("Unexpected end of stream while reading ciphertext");
      }
      final int saltLength = input.read();
      if (saltLength < 0) {
        throw new KeyExportException("Unexpected end of stream while reading salt length");
      }
      final byte[] salt = input.readNBytes(saltLength);
      if (salt.length != saltLength) {
        throw new KeyExportException("Unexpected end of stream while reading salt");
      }
      if (saltLength != EXPECTED_SALT_LENGTH_BYTES) {
        throw new KeyExportException(
            "Salt length mismatch: expected "
                + EXPECTED_SALT_LENGTH_BYTES
                + " bytes, found "
                + saltLength);
      }
      final int kdfKind = input.read();
      if (kdfKind < 0) {
        throw new KeyExportException("Unexpected end of stream while reading kdf kind");
      }
      final byte[] workFactorBytes = input.readNBytes(4);
      if (workFactorBytes.length != 4) {
        throw new KeyExportException("Unexpected end of stream while reading kdf work factor");
      }
      final int kdfWorkFactor = ByteBuffer.wrap(workFactorBytes).getInt();
      if (input.read() != -1) {
        throw new KeyExportException("Trailing data found in key export bundle");
      }
      return new KeyExportBundle(
          new String(aliasBytes, StandardCharsets.UTF_8),
          pubKey,
          nonce,
          cipher,
          salt,
          kdfKind,
          kdfWorkFactor,
          (byte) version);
    } catch (final IOException ex) {
      throw new KeyExportException("Failed to decode key export bundle", ex);
    }
  }

  private static byte[] shortBytes(final int value) {
    return ByteBuffer.allocate(2).putShort((short) value).array();
  }

  private static byte[] intBytes(final int value) {
    return ByteBuffer.allocate(4).putInt(value).array();
  }

  private static int readShort(final ByteArrayInputStream input) throws IOException {
    final byte[] buffer = input.readNBytes(2);
    if (buffer.length != 2) {
      throw new IOException("Unexpected end of stream while reading length");
    }
    return ByteBuffer.wrap(buffer).getShort() & 0xFFFF;
  }
}
