package org.hyperledger.iroha.android.privacy;

import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.Arrays;
import org.bouncycastle.crypto.InvalidCipherTextException;
import org.bouncycastle.crypto.agreement.X25519Agreement;
import org.bouncycastle.crypto.digests.SHA256Digest;
import org.bouncycastle.crypto.generators.HKDFBytesGenerator;
import org.bouncycastle.crypto.modes.ChaCha20Poly1305;
import org.bouncycastle.crypto.params.AEADParameters;
import org.bouncycastle.crypto.params.HKDFParameters;
import org.bouncycastle.crypto.params.KeyParameter;
import org.bouncycastle.crypto.params.X25519PrivateKeyParameters;
import org.bouncycastle.crypto.params.X25519PublicKeyParameters;
import org.hyperledger.iroha.android.model.instructions.ConfidentialEncryptedPayload;

final class ConfidentialNoteCrypto {
  private static final int KEY_LENGTH = 32;
  private static final int XCHACHA_NONCE_LENGTH = 24;
  private static final int IETF_NONCE_LENGTH = 12;
  private static final int AEAD_TAG_BITS = 128;
  private static final int NOTE_PLAINTEXT_VERSION_V1 = 1;
  private static final int NOTE_TEXT_MAX_BYTES = 4096;
  private static final byte[] NOTE_KDF_SALT =
      "iroha:confidential-note:v1:x25519".getBytes(StandardCharsets.UTF_8);
  private static final byte[] NOTE_KDF_INFO_PREFIX =
      "iroha:confidential-note:v1:xchacha20poly1305".getBytes(StandardCharsets.UTF_8);
  private static final byte[] NOTE_AAD_PREFIX =
      "iroha:confidential-note:v1".getBytes(StandardCharsets.UTF_8);

  private ConfidentialNoteCrypto() {}

  static byte[] publicKeyFromPrivateKey(final byte[] privateKey) {
    final byte[] privateBytes =
        fixedNonZeroBytes(privateKey, KEY_LENGTH, "privateKey");
    try {
      final X25519PrivateKeyParameters params = new X25519PrivateKeyParameters(privateBytes, 0);
      final byte[] publicKey = new byte[KEY_LENGTH];
      params.generatePublicKey().encode(publicKey, 0);
      return publicKey;
    } finally {
      Arrays.fill(privateBytes, (byte) 0);
    }
  }

  static ConfidentialEncryptedPayload encryptNote(
      final ConfidentialNoteOpening opening, final byte[] recipientPublicKey) {
    if (opening == null) {
      throw new IllegalArgumentException("opening must be provided");
    }
    final SecureRandom random = new SecureRandom();
    final byte[] ephemeralPrivateKey = new byte[KEY_LENGTH];
    final byte[] nonce = new byte[XCHACHA_NONCE_LENGTH];
    random.nextBytes(ephemeralPrivateKey);
    random.nextBytes(nonce);
    try {
      return encryptNote(opening, recipientPublicKey, ephemeralPrivateKey, nonce);
    } finally {
      Arrays.fill(ephemeralPrivateKey, (byte) 0);
    }
  }

  static ConfidentialEncryptedPayload encryptNote(
      final ConfidentialNoteOpening opening,
      final byte[] recipientPublicKey,
      final byte[] ephemeralPrivateKey,
      final byte[] nonce) {
    if (opening == null) {
      throw new IllegalArgumentException("opening must be provided");
    }
    final byte[] recipientPublic =
        ConfidentialNoteScalars.fixedBytes(recipientPublicKey, KEY_LENGTH, "recipientPublicKey");
    final byte[] ephemeralPrivate =
        fixedNonZeroBytes(ephemeralPrivateKey, KEY_LENGTH, "ephemeralPrivateKey");
    final byte[] nonceBytes =
        ConfidentialNoteScalars.fixedBytes(nonce, XCHACHA_NONCE_LENGTH, "nonce");
    final byte[] ephemeralPublic = publicKeyFromPrivateKey(ephemeralPrivate);
    byte[] key = null;
    byte[] plaintext = null;
    try {
      key = derivePayloadKey(ephemeralPrivate, recipientPublic, ephemeralPublic, recipientPublic);
      plaintext = encodePlaintext(opening);
      final byte[] ciphertext =
          runXChaCha20Poly1305(
              true, key, nonceBytes, payloadAad(ephemeralPublic, recipientPublic), plaintext);
      return new ConfidentialEncryptedPayload(ephemeralPublic, nonceBytes, ciphertext);
    } finally {
      if (key != null) {
        Arrays.fill(key, (byte) 0);
      }
      if (plaintext != null) {
        Arrays.fill(plaintext, (byte) 0);
      }
      Arrays.fill(ephemeralPrivate, (byte) 0);
    }
  }

  static ConfidentialNoteOpening decryptNote(
      final ConfidentialEncryptedPayload encryptedPayload,
      final byte[] recipientPrivateKey,
      final byte[] spendKey,
      final String expectedChainId) {
    if (encryptedPayload == null) {
      throw new IllegalArgumentException("encryptedPayload must be provided");
    }
    if (encryptedPayload.version() != ConfidentialEncryptedPayload.VERSION_V1) {
      throw new IllegalArgumentException(
          "encryptedPayload version must be " + ConfidentialEncryptedPayload.VERSION_V1);
    }
    final byte[] recipientPrivate =
        fixedNonZeroBytes(recipientPrivateKey, KEY_LENGTH, "recipientPrivateKey");
    final byte[] recipientPublic = publicKeyFromPrivateKey(recipientPrivate);
    final byte[] key =
        derivePayloadKey(
            recipientPrivate,
            encryptedPayload.ephemeralPublicKey(),
            encryptedPayload.ephemeralPublicKey(),
            recipientPublic);
    byte[] plaintext = null;
    try {
      plaintext =
          runXChaCha20Poly1305(
              false,
              key,
              encryptedPayload.nonce(),
              payloadAad(encryptedPayload.ephemeralPublicKey(), recipientPublic),
              encryptedPayload.ciphertext());
      final DecodedPlaintext decoded = decodePlaintext(plaintext);
      if (expectedChainId != null) {
        final String expected =
            ConfidentialNoteScalars.canonicalText(expectedChainId, "expectedChainId");
        if (!decoded.chainId.equals(expected)) {
          throw new IllegalArgumentException(
              "confidential note chainId does not match expectedChainId");
        }
      }
      return new ConfidentialNoteOpening(
          decoded.rho, spendKey, decoded.ownerTag, decoded.asset, decoded.chainId, decoded.amount);
    } finally {
      Arrays.fill(key, (byte) 0);
      if (plaintext != null) {
        Arrays.fill(plaintext, (byte) 0);
      }
      Arrays.fill(recipientPrivate, (byte) 0);
    }
  }

  private static byte[] encodePlaintext(final ConfidentialNoteOpening opening) {
    final byte[] assetBytes = opening.asset().getBytes(StandardCharsets.UTF_8);
    final byte[] chainIdBytes = opening.chainId().getBytes(StandardCharsets.UTF_8);
    final byte[] amountBytes = opening.amount().getBytes(StandardCharsets.US_ASCII);
    requireNoteTextLength(assetBytes.length, "asset");
    requireNoteTextLength(chainIdBytes.length, "chainId");
    requireNoteTextLength(amountBytes.length, "amount");
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(NOTE_PLAINTEXT_VERSION_V1);
    write(out, opening.rho());
    write(out, opening.ownerTag());
    writeVarint(assetBytes.length, out);
    write(out, assetBytes);
    writeVarint(chainIdBytes.length, out);
    write(out, chainIdBytes);
    writeVarint(amountBytes.length, out);
    write(out, amountBytes);
    return out.toByteArray();
  }

  private static DecodedPlaintext decodePlaintext(final byte[] bytes) {
    if (bytes == null || bytes.length == 0) {
      throw new IllegalArgumentException("confidential note plaintext must not be empty");
    }
    if ((bytes[0] & 0xff) != NOTE_PLAINTEXT_VERSION_V1) {
      throw new IllegalArgumentException("unsupported confidential note plaintext version");
    }
    int offset = 1;
    if (bytes.length < offset + 64) {
      throw new IllegalArgumentException("confidential note plaintext is truncated");
    }
    final byte[] rho = Arrays.copyOfRange(bytes, offset, offset + KEY_LENGTH);
    offset += KEY_LENGTH;
    final byte[] ownerTag =
        ConfidentialNoteScalars.fixedScalar(
            Arrays.copyOfRange(bytes, offset, offset + KEY_LENGTH), "ownerTag");
    offset += KEY_LENGTH;
    final Varint assetLen = readVarint(bytes, offset);
    offset += assetLen.encodedBytes;
    requireDecodedTextLength(assetLen.value, "asset");
    if (bytes.length < offset + assetLen.value) {
      throw new IllegalArgumentException("asset is truncated");
    }
    final String asset =
        ConfidentialNoteScalars.canonicalText(
            decodeUtf8(bytes, offset, assetLen.value, "asset"), "asset");
    offset += assetLen.value;
    final Varint chainLen = readVarint(bytes, offset);
    offset += chainLen.encodedBytes;
    requireDecodedTextLength(chainLen.value, "chainId");
    if (bytes.length < offset + chainLen.value) {
      throw new IllegalArgumentException("chainId is truncated");
    }
    final String chainId =
        ConfidentialNoteScalars.canonicalText(
            decodeUtf8(bytes, offset, chainLen.value, "chainId"), "chainId");
    offset += chainLen.value;
    final Varint amountLen = readVarint(bytes, offset);
    offset += amountLen.encodedBytes;
    requireDecodedTextLength(amountLen.value, "amount");
    if (bytes.length < offset + amountLen.value) {
      throw new IllegalArgumentException("amount is truncated");
    }
    final String amount =
        ConfidentialNoteScalars.canonicalU128(
            new String(bytes, offset, amountLen.value, StandardCharsets.US_ASCII), "amount");
    offset += amountLen.value;
    if (offset != bytes.length) {
      throw new IllegalArgumentException("confidential note plaintext has trailing bytes");
    }
    return new DecodedPlaintext(rho, ownerTag, asset, chainId, amount);
  }

  private static byte[] derivePayloadKey(
      final byte[] localPrivateKey,
      final byte[] peerPublicKey,
      final byte[] ephemeralPublicKey,
      final byte[] recipientPublicKey) {
    final byte[] localPrivate =
        fixedNonZeroBytes(localPrivateKey, KEY_LENGTH, "localPrivateKey");
    final X25519PrivateKeyParameters local = new X25519PrivateKeyParameters(localPrivate, 0);
    final X25519PublicKeyParameters peer =
        new X25519PublicKeyParameters(
            ConfidentialNoteScalars.fixedBytes(peerPublicKey, KEY_LENGTH, "peerPublicKey"), 0);
    final X25519Agreement agreement = new X25519Agreement();
    agreement.init(local);
    final byte[] shared = new byte[KEY_LENGTH];
    try {
      try {
        agreement.calculateAgreement(peer, shared, 0);
      } catch (final IllegalStateException ex) {
        throw new IllegalArgumentException("peerPublicKey must not be low-order", ex);
      }
      if (isAllZero(shared)) {
        throw new IllegalArgumentException("X25519 shared secret is all zero");
      }
      final HKDFBytesGenerator hkdf = new HKDFBytesGenerator(new SHA256Digest());
      hkdf.init(
          new HKDFParameters(
              shared, NOTE_KDF_SALT, payloadKdfInfo(ephemeralPublicKey, recipientPublicKey)));
      final byte[] out = new byte[KEY_LENGTH];
      hkdf.generateBytes(out, 0, out.length);
      return out;
    } finally {
      Arrays.fill(shared, (byte) 0);
      Arrays.fill(localPrivate, (byte) 0);
    }
  }

  private static byte[] payloadKdfInfo(
      final byte[] ephemeralPublicKey, final byte[] recipientPublicKey) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    write(out, NOTE_KDF_INFO_PREFIX);
    write(
        out,
        ConfidentialNoteScalars.fixedBytes(
            ephemeralPublicKey, KEY_LENGTH, "ephemeralPublicKey"));
    write(
        out,
        ConfidentialNoteScalars.fixedBytes(
            recipientPublicKey, KEY_LENGTH, "recipientPublicKey"));
    return out.toByteArray();
  }

  private static byte[] payloadAad(
      final byte[] ephemeralPublicKey, final byte[] recipientPublicKey) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    write(out, NOTE_AAD_PREFIX);
    out.write(NOTE_PLAINTEXT_VERSION_V1);
    write(
        out,
        ConfidentialNoteScalars.fixedBytes(
            ephemeralPublicKey, KEY_LENGTH, "ephemeralPublicKey"));
    write(
        out,
        ConfidentialNoteScalars.fixedBytes(
            recipientPublicKey, KEY_LENGTH, "recipientPublicKey"));
    return out.toByteArray();
  }

  private static byte[] runXChaCha20Poly1305(
      final boolean encrypt,
      final byte[] key,
      final byte[] nonce,
      final byte[] aad,
      final byte[] input) {
    final byte[] nonceBytes =
        ConfidentialNoteScalars.fixedBytes(nonce, XCHACHA_NONCE_LENGTH, "nonce");
    final byte[] subkey =
        hChaCha20(
            ConfidentialNoteScalars.fixedBytes(key, KEY_LENGTH, "key"),
            Arrays.copyOfRange(nonceBytes, 0, 16));
    final byte[] ietfNonce = new byte[IETF_NONCE_LENGTH];
    System.arraycopy(nonceBytes, 16, ietfNonce, 4, 8);
    try {
      final ChaCha20Poly1305 cipher = new ChaCha20Poly1305();
      cipher.init(
          encrypt,
          new AEADParameters(new KeyParameter(subkey), AEAD_TAG_BITS, ietfNonce, aad));
      final byte[] out = new byte[cipher.getOutputSize(input.length)];
      int written = cipher.processBytes(input, 0, input.length, out, 0);
      written += cipher.doFinal(out, written);
      return Arrays.copyOf(out, written);
    } catch (final InvalidCipherTextException ex) {
      throw new SecurityException("confidential note payload authentication failed", ex);
    } catch (final RuntimeException ex) {
      throw new IllegalArgumentException("confidential note payload cryptography failed", ex);
    } finally {
      Arrays.fill(subkey, (byte) 0);
    }
  }

  private static byte[] hChaCha20(final byte[] key, final byte[] nonce16) {
    if (key.length != KEY_LENGTH) {
      throw new IllegalArgumentException("key must be 32 bytes");
    }
    if (nonce16.length != 16) {
      throw new IllegalArgumentException("nonce16 must be 16 bytes");
    }
    final int[] state = new int[16];
    state[0] = 0x61707865;
    state[1] = 0x3320646e;
    state[2] = 0x79622d32;
    state[3] = 0x6b206574;
    for (int i = 0; i < 8; i++) {
      state[4 + i] = leI32(key, i * 4);
    }
    for (int i = 0; i < 4; i++) {
      state[12 + i] = leI32(nonce16, i * 4);
    }
    for (int i = 0; i < 10; i++) {
      quarterRound(state, 0, 4, 8, 12);
      quarterRound(state, 1, 5, 9, 13);
      quarterRound(state, 2, 6, 10, 14);
      quarterRound(state, 3, 7, 11, 15);
      quarterRound(state, 0, 5, 10, 15);
      quarterRound(state, 1, 6, 11, 12);
      quarterRound(state, 2, 7, 8, 13);
      quarterRound(state, 3, 4, 9, 14);
    }
    final byte[] out = new byte[KEY_LENGTH];
    intToLe(state[0], out, 0);
    intToLe(state[1], out, 4);
    intToLe(state[2], out, 8);
    intToLe(state[3], out, 12);
    intToLe(state[12], out, 16);
    intToLe(state[13], out, 20);
    intToLe(state[14], out, 24);
    intToLe(state[15], out, 28);
    return out;
  }

  private static void quarterRound(
      final int[] state, final int a, final int b, final int c, final int d) {
    state[a] += state[b];
    state[d] = Integer.rotateLeft(state[d] ^ state[a], 16);
    state[c] += state[d];
    state[b] = Integer.rotateLeft(state[b] ^ state[c], 12);
    state[a] += state[b];
    state[d] = Integer.rotateLeft(state[d] ^ state[a], 8);
    state[c] += state[d];
    state[b] = Integer.rotateLeft(state[b] ^ state[c], 7);
  }

  private static int leI32(final byte[] bytes, final int offset) {
    return (bytes[offset] & 0xff)
        | ((bytes[offset + 1] & 0xff) << 8)
        | ((bytes[offset + 2] & 0xff) << 16)
        | ((bytes[offset + 3] & 0xff) << 24);
  }

  private static void intToLe(final int value, final byte[] out, final int offset) {
    out[offset] = (byte) value;
    out[offset + 1] = (byte) (value >>> 8);
    out[offset + 2] = (byte) (value >>> 16);
    out[offset + 3] = (byte) (value >>> 24);
  }

  private static void writeVarint(final int value, final ByteArrayOutputStream out) {
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

  private static Varint readVarint(final byte[] bytes, final int offset) {
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
              "non-canonical confidential note plaintext length");
        }
        return new Varint(value, encodedBytes);
      }
      shift += 7;
    }
    throw new IllegalArgumentException("invalid confidential note plaintext length");
  }

  private static String decodeUtf8(
      final byte[] bytes, final int offset, final int length, final String name) {
    try {
      return StandardCharsets.UTF_8
          .newDecoder()
          .onMalformedInput(CodingErrorAction.REPORT)
          .onUnmappableCharacter(CodingErrorAction.REPORT)
          .decode(ByteBuffer.wrap(bytes, offset, length))
          .toString();
    } catch (final CharacterCodingException ex) {
      throw new IllegalArgumentException(name + " must be valid UTF-8", ex);
    }
  }

  private static void write(final ByteArrayOutputStream out, final byte[] bytes) {
    out.write(bytes, 0, bytes.length);
  }

  private static void requireNoteTextLength(final int length, final String name) {
    if (length > NOTE_TEXT_MAX_BYTES) {
      throw new IllegalArgumentException(name + " is too large");
    }
  }

  private static void requireDecodedTextLength(final int length, final String name) {
    if (length < 1 || length > NOTE_TEXT_MAX_BYTES) {
      throw new IllegalArgumentException(name + " length is invalid");
    }
  }

  private static boolean isAllZero(final byte[] bytes) {
    for (final byte b : bytes) {
      if (b != 0) {
        return false;
      }
    }
    return true;
  }

  private static byte[] fixedNonZeroBytes(
      final byte[] value, final int expected, final String name) {
    final byte[] bytes = ConfidentialNoteScalars.fixedBytes(value, expected, name);
    if (isAllZero(bytes)) {
      throw new IllegalArgumentException(name + " must not be all zero");
    }
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

  private static final class DecodedPlaintext {
    private final byte[] rho;
    private final byte[] ownerTag;
    private final String asset;
    private final String chainId;
    private final String amount;

    private DecodedPlaintext(
        final byte[] rho,
        final byte[] ownerTag,
        final String asset,
        final String chainId,
        final String amount) {
      this.rho = rho;
      this.ownerTag = ownerTag;
      this.asset = asset;
      this.chainId = chainId;
      this.amount = amount;
    }
  }
}
