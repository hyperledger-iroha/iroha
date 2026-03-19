package org.hyperledger.iroha.android.connect;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.Arrays;
import java.util.Objects;
import org.bouncycastle.crypto.InvalidCipherTextException;
import org.bouncycastle.crypto.agreement.X25519Agreement;
import org.bouncycastle.crypto.digests.Blake2bDigest;
import org.bouncycastle.crypto.digests.SHA256Digest;
import org.bouncycastle.crypto.generators.HKDFBytesGenerator;
import org.bouncycastle.crypto.modes.ChaCha20Poly1305;
import org.bouncycastle.crypto.params.AEADParameters;
import org.bouncycastle.crypto.params.HKDFParameters;
import org.bouncycastle.crypto.params.KeyParameter;
import org.bouncycastle.crypto.params.X25519PrivateKeyParameters;
import org.bouncycastle.crypto.params.X25519PublicKeyParameters;

/** Cryptographic helpers for the wallet-role Connect session. */
public final class ConnectCrypto {

  private static final int KEY_LENGTH = 32;
  private static final int NONCE_LENGTH = 12;
  private static final int AEAD_TAG_BITS = 128;
  private static final byte[] X25519_HKDF_SALT =
      "iroha:x25519:hkdf:v1".getBytes(StandardCharsets.UTF_8);
  private static final byte[] X25519_HKDF_INFO =
      "iroha:x25519:session-key".getBytes(StandardCharsets.UTF_8);

  private ConnectCrypto() {}

  public static final class KeyPair {
    private final byte[] publicKey;
    private final byte[] privateKey;

    private KeyPair(final byte[] publicKey, final byte[] privateKey) {
      this.publicKey = publicKey;
      this.privateKey = privateKey;
    }

    public byte[] publicKey() {
      return publicKey.clone();
    }

    public byte[] privateKey() {
      return privateKey.clone();
    }
  }

  public static final class DirectionKeys {
    private final byte[] appToWallet;
    private final byte[] walletToApp;

    private DirectionKeys(final byte[] appToWallet, final byte[] walletToApp) {
      this.appToWallet = appToWallet;
      this.walletToApp = walletToApp;
    }

    public byte[] appToWallet() {
      return appToWallet.clone();
    }

    public byte[] walletToApp() {
      return walletToApp.clone();
    }

    public byte[] keyForDirection(final ConnectDirection direction) {
      if (direction == ConnectDirection.APP_TO_WALLET) {
        return appToWallet();
      }
      return walletToApp();
    }
  }

  public static KeyPair generateKeyPair() {
    final X25519PrivateKeyParameters privateKey = new X25519PrivateKeyParameters(new SecureRandom());
    final X25519PublicKeyParameters publicKey = privateKey.generatePublicKey();
    final byte[] privateBytes = new byte[KEY_LENGTH];
    final byte[] publicBytes = new byte[KEY_LENGTH];
    privateKey.encode(privateBytes, 0);
    publicKey.encode(publicBytes, 0);
    return new KeyPair(publicBytes, privateBytes);
  }

  public static DirectionKeys deriveDirectionKeys(
      final byte[] localPrivateKey,
      final byte[] peerPublicKey,
      final byte[] sessionId)
      throws ConnectProtocolException {
    requireLength(localPrivateKey, KEY_LENGTH, "localPrivateKey");
    requireLength(peerPublicKey, KEY_LENGTH, "peerPublicKey");
    requireLength(sessionId, KEY_LENGTH, "sessionId");

    final X25519PrivateKeyParameters local = new X25519PrivateKeyParameters(localPrivateKey, 0);
    final X25519PublicKeyParameters peer = new X25519PublicKeyParameters(peerPublicKey, 0);
    final X25519Agreement agreement = new X25519Agreement();
    agreement.init(local);
    final byte[] shared = new byte[KEY_LENGTH];
    agreement.calculateAgreement(peer, shared, 0);
    if (isAllZero(shared)) {
      Arrays.fill(shared, (byte) 0);
      throw new ConnectProtocolException(
          "x25519 shared secret is all-zero (invalid public key)");
    }

    final byte[] sessionKey = hkdfExpand(shared, X25519_HKDF_SALT, X25519_HKDF_INFO);
    final byte[] salt = blake2b32("iroha-connect|salt|".getBytes(StandardCharsets.UTF_8), sessionId);
    final byte[] appKey =
        hkdfExpand(sessionKey, salt, "iroha-connect|k_app".getBytes(StandardCharsets.UTF_8));
    final byte[] walletKey =
        hkdfExpand(sessionKey, salt, "iroha-connect|k_wallet".getBytes(StandardCharsets.UTF_8));
    Arrays.fill(sessionKey, (byte) 0);
    Arrays.fill(shared, (byte) 0);
    return new DirectionKeys(appKey, walletKey);
  }

  public static byte[] encryptEnvelope(
      final byte[] envelope,
      final byte[] key,
      final byte[] sessionId,
      final ConnectDirection direction,
      final long sequence)
      throws ConnectProtocolException {
    requireLength(key, KEY_LENGTH, "key");
    requireLength(sessionId, KEY_LENGTH, "sessionId");
    Objects.requireNonNull(direction, "direction");
    Objects.requireNonNull(envelope, "envelope");

    final byte[] aad = buildAad(sessionId, direction, sequence);
    final byte[] nonce = nonceFromSequence(sequence);
    return runAead(true, key, nonce, aad, envelope);
  }

  public static byte[] decryptCiphertext(
      final byte[] ciphertext,
      final byte[] key,
      final byte[] sessionId,
      final ConnectDirection direction,
      final long sequence)
      throws ConnectProtocolException {
    requireLength(key, KEY_LENGTH, "key");
    requireLength(sessionId, KEY_LENGTH, "sessionId");
    Objects.requireNonNull(direction, "direction");
    Objects.requireNonNull(ciphertext, "ciphertext");

    final byte[] aad = buildAad(sessionId, direction, sequence);
    final byte[] nonce = nonceFromSequence(sequence);
    return runAead(false, key, nonce, aad, ciphertext);
  }

  public static byte[] buildApprovePreimage(
      final byte[] sessionId,
      final byte[] appPublicKey,
      final byte[] walletPublicKey,
      final String accountId,
      final byte[] permissionsHash,
      final byte[] proofHash)
      throws ConnectProtocolException {
    requireLength(sessionId, KEY_LENGTH, "sessionId");
    requireLength(appPublicKey, KEY_LENGTH, "appPublicKey");
    requireLength(walletPublicKey, KEY_LENGTH, "walletPublicKey");
    if (accountId == null || accountId.trim().isEmpty()) {
      throw new ConnectProtocolException("accountId must not be empty");
    }

    final byte[] accountBytes = accountId.getBytes(StandardCharsets.UTF_8);
    int size = "iroha-connect|approve|".getBytes(StandardCharsets.UTF_8).length + sessionId.length + appPublicKey.length + walletPublicKey.length + accountBytes.length;
    if (permissionsHash != null) {
      size += permissionsHash.length;
    }
    if (proofHash != null) {
      size += proofHash.length;
    }

    final ByteBuffer buffer = ByteBuffer.allocate(size);
    buffer.put("iroha-connect|approve|".getBytes(StandardCharsets.UTF_8));
    buffer.put(sessionId);
    buffer.put(appPublicKey);
    buffer.put(walletPublicKey);
    buffer.put(accountBytes);
    if (permissionsHash != null) {
      buffer.put(permissionsHash);
    }
    if (proofHash != null) {
      buffer.put(proofHash);
    }
    return buffer.array();
  }

  public static byte[] nonceFromSequence(final long sequence) {
    final byte[] nonce = new byte[NONCE_LENGTH];
    final ByteBuffer buffer = ByteBuffer.wrap(nonce).order(ByteOrder.LITTLE_ENDIAN);
    buffer.putInt(0);
    buffer.putLong(sequence);
    return nonce;
  }

  private static byte[] buildAad(
      final byte[] sessionId, final ConnectDirection direction, final long sequence) {
    final byte[] prefix = "connect:v1".getBytes(StandardCharsets.UTF_8);
    final ByteBuffer buffer = ByteBuffer.allocate(prefix.length + KEY_LENGTH + 1 + Long.BYTES + 1)
        .order(ByteOrder.LITTLE_ENDIAN);
    buffer.put(prefix);
    buffer.put(sessionId);
    buffer.put((byte) (direction == ConnectDirection.APP_TO_WALLET ? 0 : 1));
    buffer.putLong(sequence);
    buffer.put((byte) 1);
    return buffer.array();
  }

  private static byte[] runAead(
      final boolean encrypt,
      final byte[] key,
      final byte[] nonce,
      final byte[] aad,
      final byte[] input)
      throws ConnectProtocolException {
    try {
      final ChaCha20Poly1305 cipher = new ChaCha20Poly1305();
      final AEADParameters params = new AEADParameters(new KeyParameter(key), AEAD_TAG_BITS, nonce, aad);
      cipher.init(encrypt, params);
      final byte[] out = new byte[cipher.getOutputSize(input.length)];
      int written = cipher.processBytes(input, 0, input.length, out, 0);
      written += cipher.doFinal(out, written);
      return Arrays.copyOf(out, written);
    } catch (final InvalidCipherTextException ex) {
      throw new ConnectProtocolException(encrypt ? "Connect encryption failed" : "Connect decryption failed", ex);
    } catch (final RuntimeException ex) {
      throw new ConnectProtocolException("Connect AEAD failure", ex);
    }
  }

  private static byte[] hkdfExpand(final byte[] ikm, final byte[] salt, final byte[] info)
      throws ConnectProtocolException {
    try {
      final HKDFBytesGenerator hkdf = new HKDFBytesGenerator(new SHA256Digest());
      hkdf.init(new HKDFParameters(ikm, salt, info));
      final byte[] out = new byte[KEY_LENGTH];
      hkdf.generateBytes(out, 0, out.length);
      return out;
    } catch (final RuntimeException ex) {
      throw new ConnectProtocolException("Connect HKDF expansion failed", ex);
    }
  }

  private static byte[] blake2b32(final byte[]... segments) {
    final Blake2bDigest digest = new Blake2bDigest(256);
    for (final byte[] segment : segments) {
      digest.update(segment, 0, segment.length);
    }
    final byte[] out = new byte[KEY_LENGTH];
    digest.doFinal(out, 0);
    return out;
  }

  private static boolean isAllZero(final byte[] value) {
    for (final byte b : value) {
      if (b != 0) {
        return false;
      }
    }
    return true;
  }

  private static void requireLength(final byte[] value, final int expected, final String name)
      throws ConnectProtocolException {
    if (value == null || value.length != expected) {
      final int actual = value == null ? 0 : value.length;
      throw new ConnectProtocolException(name + " must contain " + expected + " bytes (got " + actual + ")");
    }
  }
}
