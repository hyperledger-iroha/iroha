package org.hyperledger.iroha.android.crypto.export;

import java.nio.ByteBuffer;
import java.nio.CharBuffer;
import java.nio.charset.StandardCharsets;
import java.security.GeneralSecurityException;
import java.security.KeyFactory;
import java.security.MessageDigest;
import java.security.PrivateKey;
import java.security.PublicKey;
import java.security.spec.PKCS8EncodedKeySpec;
import java.security.spec.X509EncodedKeySpec;
import java.util.ArrayDeque;
import java.util.Arrays;
import java.util.Deque;
import java.util.HexFormat;
import java.util.LinkedHashSet;
import java.util.Objects;
import java.util.Set;
import java.security.SecureRandom;
import javax.crypto.Cipher;
import javax.crypto.Mac;
import javax.crypto.SecretKeyFactory;
import javax.crypto.spec.GCMParameterSpec;
import javax.crypto.spec.PBEKeySpec;
import javax.crypto.spec.SecretKeySpec;

/**
 * Exports and recovers software-generated Ed25519 keys using salted HKDF + AES-GCM.
 *
 * <p>Each export uses a fresh random salt and nonce bound to the alias, derives a key with a
 * memory-hard KDF (Argon2id preferred, PBKDF2 fallback), and records the KDF kind/work factor in
 * the bundle. The exported bundle contains the public key, nonce, salt, and ciphertext so the
 * receiver can validate and recover the key pair deterministically across JVMs.
 */
public final class DeterministicKeyExporter {

  private static final String HMAC_ALGORITHM = "HmacSHA256";
  private static final String DIGEST_ALGORITHM = "SHA-256";
  private static final String KEY_ALGORITHM = "Ed25519";
  private static final String PBKDF2_ALGORITHM = "PBKDF2WithHmacSHA256";
  private static final String AES_TRANSFORMATION = "AES/GCM/NoPadding";
  private static final int GCM_TAG_BITS = 128;
  private static final byte[] ED25519_SPKI_PREFIX =
      new byte[] {
        0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00
      };
  private static final int ED25519_SPKI_SIZE = 44;
  private static final byte[] ED25519_OID = new byte[] {0x2b, 0x65, 0x70};
  static final int KDF_KIND_PBKDF2_HMAC_SHA256 = 1;
  static final int KDF_KIND_ARGON2ID = 2;
  private static final int DEFAULT_PBKDF2_ITERATIONS = 350_000;
  private static final int DEFAULT_ARGON2_MEMORY_KIB = 64 * 1024;
  private static final int DEFAULT_ARGON2_ITERATIONS = 3;
  private static final int DEFAULT_ARGON2_PARALLELISM = 2;
  private static final int SALT_LENGTH_BYTES = KeyExportBundle.EXPECTED_SALT_LENGTH_BYTES;
  private static final int NONCE_LENGTH_BYTES = KeyExportBundle.EXPECTED_NONCE_LENGTH_BYTES;
  private static final int AES_KEY_LENGTH_BYTES = 32;
  private static final int MIN_PASSPHRASE_LENGTH = 12;
  private static final int MAX_TRACKED_EXPORTS = 1024;

  private static final SecureRandom SECURE_RANDOM = new SecureRandom();
  private static final Deque<String> RECENT_EXPORT_FINGERPRINTS = new ArrayDeque<>();
  private static final Set<String> RECENT_EXPORT_INDEX = new LinkedHashSet<>();

  private DeterministicKeyExporter() {}

  /**
   * Exports the {@code privateKey} deterministically under {@code passphrase} and {@code alias}.
   *
   * <p>The passphrase is consumed as UTF-8 bytes and cleared after derivation.
   */
  public static KeyExportBundle exportKeyPair(
      final PrivateKey privateKey, final PublicKey publicKey, final String alias, final char[] passphrase)
      throws KeyExportException {
    Objects.requireNonNull(privateKey, "privateKey");
    Objects.requireNonNull(publicKey, "publicKey");
    Objects.requireNonNull(alias, "alias");
    Objects.requireNonNull(passphrase, "passphrase");
    ensureEd25519KeyPair(privateKey, publicKey);
    ensurePassphraseStrength(passphrase);

    final byte[] privateKeyBytes = privateKey.getEncoded();
    final byte[] publicKeyBytes = publicKey.getEncoded();
    final byte[] salt = new byte[SALT_LENGTH_BYTES];
    fillRandomNonZero(salt);
    final byte[] nonce = new byte[NONCE_LENGTH_BYTES];
    fillRandomNonZero(nonce);
    guardSaltNonceReuse(salt, nonce);
    final byte bundleVersion = KeyExportBundle.VERSION_V3;
    final KdfResult kdf = derivePreferredKey(alias, passphrase, salt);
    try {
      final Cipher cipher = Cipher.getInstance(AES_TRANSFORMATION);
      final GCMParameterSpec spec = new GCMParameterSpec(GCM_TAG_BITS, nonce);
      cipher.init(
          Cipher.ENCRYPT_MODE,
          new SecretKeySpec(kdf.key(), "AES"),
          spec);
      cipher.updateAAD(alias.getBytes(StandardCharsets.UTF_8));
      final byte[] ciphertext = cipher.doFinal(privateKeyBytes);
      return new KeyExportBundle(
          alias,
          publicKeyBytes,
          nonce,
          ciphertext,
          salt,
          kdf.kind(),
          kdf.workFactor(),
          bundleVersion);
    } catch (final GeneralSecurityException ex) {
      throw new KeyExportException("Failed to encrypt private key", ex);
    } finally {
      Arrays.fill(privateKeyBytes, (byte) 0);
      kdf.zero();
    }
  }

  private static void ensureEd25519KeyPair(
      final PrivateKey privateKey, final PublicKey publicKey) throws KeyExportException {
    if (!isEd25519PrivateKey(privateKey) || !isEd25519PublicKey(publicKey)) {
      throw new KeyExportException("Deterministic export currently supports Ed25519 keys only");
    }
  }

  private static boolean isEd25519PrivateKey(final PrivateKey privateKey) {
    if (privateKey == null) {
      return false;
    }
    final byte[] encoded = privateKey.getEncoded();
    if (encoded == null || encoded.length == 0) {
      return false;
    }
    return hasEd25519Algorithm(encoded);
  }

  private static boolean isEd25519PublicKey(final PublicKey publicKey) {
    if (publicKey == null) {
      return false;
    }
    final byte[] encoded = publicKey.getEncoded();
    if (encoded == null || encoded.length == 0) {
      return false;
    }
    if (encoded.length == ED25519_SPKI_SIZE) {
      boolean matches = true;
      for (int i = 0; i < ED25519_SPKI_PREFIX.length; i++) {
        if (encoded[i] != ED25519_SPKI_PREFIX[i]) {
          matches = false;
          break;
        }
      }
      if (matches) {
        return true;
      }
    }
    return hasEd25519Spki(encoded);
  }

  private static boolean hasEd25519Algorithm(final byte[] encoded) {
    return hasEd25519Pkcs8(encoded);
  }

  private static boolean hasEd25519Pkcs8(final byte[] encoded) {
    if (encoded.length < 16 || encoded[0] != 0x30) {
      return false;
    }
    int offset = 1;
    final int[] lengthBytes = new int[1];
    final int totalLength = readDerLength(encoded, offset, lengthBytes);
    if (totalLength < 0) {
      return false;
    }
    offset += lengthBytes[0];
    if (offset + totalLength > encoded.length) {
      return false;
    }
    if (offset >= encoded.length || encoded[offset++] != 0x02) {
      return false;
    }
    final int versionLength = readDerLength(encoded, offset, lengthBytes);
    if (versionLength < 0) {
      return false;
    }
    offset += lengthBytes[0];
    if (offset + versionLength > encoded.length) {
      return false;
    }
    offset += versionLength;
    return readAlgorithmOid(encoded, offset, lengthBytes);
  }

  private static boolean hasEd25519Spki(final byte[] encoded) {
    if (encoded.length < 12 || encoded[0] != 0x30) {
      return false;
    }
    int offset = 1;
    final int[] lengthBytes = new int[1];
    final int totalLength = readDerLength(encoded, offset, lengthBytes);
    if (totalLength < 0) {
      return false;
    }
    offset += lengthBytes[0];
    if (offset + totalLength > encoded.length) {
      return false;
    }
    return readAlgorithmOid(encoded, offset, lengthBytes);
  }

  private static boolean readAlgorithmOid(
      final byte[] encoded, int offset, final int[] lengthBytes) {
    if (offset >= encoded.length || encoded[offset++] != 0x30) {
      return false;
    }
    final int algLength = readDerLength(encoded, offset, lengthBytes);
    if (algLength < 0) {
      return false;
    }
    offset += lengthBytes[0];
    if (offset + algLength > encoded.length) {
      return false;
    }
    if (offset >= encoded.length || encoded[offset++] != 0x06) {
      return false;
    }
    final int oidLength = readDerLength(encoded, offset, lengthBytes);
    if (oidLength != ED25519_OID.length) {
      return false;
    }
    offset += lengthBytes[0];
    if (offset + oidLength > encoded.length) {
      return false;
    }
    for (int i = 0; i < oidLength; i++) {
      if (encoded[offset + i] != ED25519_OID[i]) {
        return false;
      }
    }
    return true;
  }

  private static int readDerLength(
      final byte[] encoded, final int offset, final int[] lengthBytes) {
    if (offset >= encoded.length) {
      return -1;
    }
    final int first = encoded[offset] & 0xFF;
    if ((first & 0x80) == 0) {
      lengthBytes[0] = 1;
      return first;
    }
    final int count = first & 0x7F;
    if (count == 0 || count > 4 || offset + count >= encoded.length) {
      return -1;
    }
    int length = 0;
    for (int i = 0; i < count; i++) {
      length = (length << 8) | (encoded[offset + 1 + i] & 0xFF);
    }
    lengthBytes[0] = 1 + count;
    return length;
  }

  /** Imports a key pair from the supplied bundle using {@code passphrase}. */
  public static KeyPairData importKeyPair(
      final KeyExportBundle bundle, final char[] passphrase) throws KeyExportException {
    Objects.requireNonNull(bundle, "bundle");
    Objects.requireNonNull(passphrase, "passphrase");
    if (bundle.version() != KeyExportBundle.VERSION_V3) {
      throw new KeyExportException("Unsupported key export version: " + bundle.version());
    }
    ensurePassphraseStrength(passphrase);
    if (isAllZero(bundle.nonce()) || isAllZero(bundle.salt())) {
      throw new KeyExportException("Salt and nonce must be non-zero for deterministic import");
    }
    final byte[] aesKey =
        deriveKeyForBundle(
            bundle.alias(),
            passphrase,
            bundle.salt(),
            bundle.kdfKind(),
            bundle.kdfWorkFactor());
    try {
      final Cipher cipher = Cipher.getInstance(AES_TRANSFORMATION);
      final GCMParameterSpec spec = new GCMParameterSpec(GCM_TAG_BITS, bundle.nonce());
      cipher.init(
          Cipher.DECRYPT_MODE,
          new SecretKeySpec(aesKey, "AES"),
          spec);
      cipher.updateAAD(bundle.alias().getBytes(StandardCharsets.UTF_8));
      final byte[] privateKeyBytes = cipher.doFinal(bundle.ciphertext());
      try {
        final KeyFactory factory = KeyFactory.getInstance(KEY_ALGORITHM);
        final PrivateKey privateKey =
            factory.generatePrivate(new PKCS8EncodedKeySpec(privateKeyBytes));
        final PublicKey publicKey =
            factory.generatePublic(new X509EncodedKeySpec(bundle.publicKey()));
        return new KeyPairData(privateKey, publicKey);
      } catch (final GeneralSecurityException ex) {
        throw new KeyExportException("Failed to reconstruct Ed25519 key pair", ex);
      } finally {
        Arrays.fill(privateKeyBytes, (byte) 0);
      }
    } catch (final GeneralSecurityException ex) {
      throw new KeyExportException("Failed to decrypt private key", ex);
    } finally {
      Arrays.fill(aesKey, (byte) 0);
    }
  }

  private static byte[] deriveKeyForBundle(
      final String alias,
      final char[] passphrase,
      final byte[] salt,
      final int kdfKind,
      final int workFactor)
      throws KeyExportException {
    if (salt == null || salt.length != SALT_LENGTH_BYTES) {
      throw new KeyExportException("Salt must be provided for key import");
    }
    if (workFactor <= 0) {
      throw new KeyExportException("Invalid KDF work factor: " + workFactor);
    }
    return switch (kdfKind) {
      case KDF_KIND_PBKDF2_HMAC_SHA256 ->
          derivePbkdf2Key(alias, passphrase, salt, workFactor);
      case KDF_KIND_ARGON2ID ->
          deriveArgon2Key(alias, passphrase, salt, workFactor);
      default -> throw new KeyExportException("Unsupported KDF kind: " + kdfKind);
    };
  }

  private static KdfResult derivePreferredKey(
      final String alias, final char[] passphrase, final byte[] salt)
      throws KeyExportException {
    if (argon2Available()) {
      try {
        final byte[] argonKey =
            deriveArgon2Key(alias, passphrase, salt, DEFAULT_ARGON2_ITERATIONS);
        return new KdfResult(argonKey, KDF_KIND_ARGON2ID, DEFAULT_ARGON2_ITERATIONS);
      } catch (final KeyExportException | RuntimeException | LinkageError ex) {
        // fall through to PBKDF2 fallback
      }
    }
    final byte[] pbkdfKey =
        derivePbkdf2Key(alias, passphrase, salt, DEFAULT_PBKDF2_ITERATIONS);
    return new KdfResult(pbkdfKey, KDF_KIND_PBKDF2_HMAC_SHA256, DEFAULT_PBKDF2_ITERATIONS);
  }

  private static byte[] deriveArgon2Key(
      final String alias,
      final char[] passphrase,
      final byte[] salt,
      final int iterations)
      throws KeyExportException {
    if (!argon2Available()) {
      throw new KeyExportException("Argon2id derivation unavailable");
    }
    final byte[] aliasBytes = alias.getBytes(StandardCharsets.UTF_8);
    final byte[] kdfSalt = ByteBuffer.allocate(salt.length + aliasBytes.length)
        .put(salt)
        .put(aliasBytes)
        .array();
    final byte[] passphraseBytes = encodeUtf8(passphrase);
    final byte[] kdfOutput = new byte[AES_KEY_LENGTH_BYTES];
    try {
      final Class<?> paramsClass = Class.forName("org.bouncycastle.crypto.params.Argon2Parameters");
      final Class<?> builderClass =
          Class.forName("org.bouncycastle.crypto.params.Argon2Parameters$Builder");
      final int argon2id = paramsClass.getField("ARGON2_id").getInt(null);
      final Object builder =
          builderClass.getConstructor(int.class).newInstance(Integer.valueOf(argon2id));
      builderClass.getMethod("withSalt", byte[].class).invoke(builder, kdfSalt);
      builderClass
          .getMethod("withParallelism", int.class)
          .invoke(builder, Integer.valueOf(DEFAULT_ARGON2_PARALLELISM));
      builderClass
          .getMethod("withIterations", int.class)
          .invoke(builder, Integer.valueOf(iterations));
      builderClass
          .getMethod("withMemoryAsKB", int.class)
          .invoke(builder, Integer.valueOf(DEFAULT_ARGON2_MEMORY_KIB));
      final Object params = builderClass.getMethod("build").invoke(builder);
      final Class<?> generatorClass =
          Class.forName("org.bouncycastle.crypto.generators.Argon2BytesGenerator");
      final Object generator = generatorClass.getConstructor().newInstance();
      generatorClass.getMethod("init", paramsClass).invoke(generator, params);
      generatorClass
          .getMethod("generateBytes", byte[].class, byte[].class)
          .invoke(generator, passphraseBytes, kdfOutput);
      final byte[] derived =
          hkdf(
              kdfOutput,
              sha256(hkdfSaltDomain(), aliasBytes),
              hkdfInfoDomain().getBytes(StandardCharsets.UTF_8),
              AES_KEY_LENGTH_BYTES);
      Arrays.fill(kdfOutput, (byte) 0);
      return derived;
    } catch (final ReflectiveOperationException ex) {
      throw new KeyExportException("Argon2id derivation unavailable", ex);
    } finally {
      Arrays.fill(kdfSalt, (byte) 0);
      Arrays.fill(passphraseBytes, (byte) 0);
      Arrays.fill(kdfOutput, (byte) 0);
    }
  }

  private static byte[] derivePbkdf2Key(
      final String alias,
      final char[] passphrase,
      final byte[] salt,
      final int iterations)
      throws KeyExportException {
    final byte[] aliasBytes = alias.getBytes(StandardCharsets.UTF_8);
    final byte[] kdfSalt = ByteBuffer.allocate(salt.length + aliasBytes.length)
        .put(salt)
        .put(aliasBytes)
        .array();
    PBEKeySpec spec = null;
    byte[] kdfOutput = null;
    try {
      spec = new PBEKeySpec(passphrase, kdfSalt, iterations, AES_KEY_LENGTH_BYTES * 8);
      final SecretKeyFactory factory = SecretKeyFactory.getInstance(PBKDF2_ALGORITHM);
      kdfOutput = factory.generateSecret(spec).getEncoded();
      final byte[] derived =
          hkdf(
              kdfOutput,
              sha256(hkdfSaltDomain(), aliasBytes),
              hkdfInfoDomain().getBytes(StandardCharsets.UTF_8),
              AES_KEY_LENGTH_BYTES);
      Arrays.fill(kdfOutput, (byte) 0);
      return derived;
    } catch (final GeneralSecurityException ex) {
      throw new KeyExportException("PBKDF2 derivation failed", ex);
    } finally {
      if (spec != null) {
        spec.clearPassword();
      }
      if (kdfOutput != null) {
        Arrays.fill(kdfOutput, (byte) 0);
      }
      Arrays.fill(kdfSalt, (byte) 0);
    }
  }

  private static byte[] hkdf(
      final byte[] ikm, final byte[] salt, final byte[] info, final int length)
      throws KeyExportException {
    try {
      final Mac mac = Mac.getInstance(HMAC_ALGORITHM);
      final byte[] saltValue =
          salt != null ? salt.clone() : new byte[mac.getMacLength()];
      mac.init(new SecretKeySpec(saltValue, HMAC_ALGORITHM));
      final byte[] prk = mac.doFinal(ikm);
      final byte[] result = new byte[length];
      byte[] previous = new byte[0];
      int offset = 0;
      int blockIndex = 1;
      while (offset < length) {
        mac.init(new SecretKeySpec(prk, HMAC_ALGORITHM));
        mac.update(previous);
        mac.update(info);
        mac.update((byte) blockIndex);
        previous = mac.doFinal();
        final int copy = Math.min(previous.length, length - offset);
        System.arraycopy(previous, 0, result, offset, copy);
        offset += copy;
        blockIndex++;
      }
      Arrays.fill(prk, (byte) 0);
      Arrays.fill(previous, (byte) 0);
      return result;
    } catch (final GeneralSecurityException ex) {
      throw new KeyExportException("HKDF derivation failed", ex);
    }
  }

  private static byte[] encodeUtf8(final char[] chars) {
    final CharBuffer charBuffer = CharBuffer.wrap(chars);
    final java.nio.charset.CharsetEncoder encoder = StandardCharsets.UTF_8.newEncoder();
    final ByteBuffer byteBuffer;
    try {
      byteBuffer = encoder.encode(charBuffer);
    } catch (final java.nio.charset.CharacterCodingException ex) {
      // UTF-8 encoder should never fail on Unicode char input.
      throw new IllegalArgumentException("Failed to encode passphrase as UTF-8", ex);
    }
    final byte[] out = new byte[byteBuffer.remaining()];
    byteBuffer.get(out);
    if (byteBuffer.hasArray()) {
      Arrays.fill(byteBuffer.array(), (byte) 0);
    }
    return out;
  }

  private static byte[] sha256(final String domain, final byte[] data) throws KeyExportException {
    try {
      final MessageDigest digest = MessageDigest.getInstance(DIGEST_ALGORITHM);
      digest.update(domain.getBytes(StandardCharsets.UTF_8));
      digest.update(data);
      return digest.digest();
    } catch (final GeneralSecurityException ex) {
      throw new KeyExportException("SHA-256 digest failed", ex);
    }
  }

  private static boolean argon2Available() {
    try {
      Class.forName("org.bouncycastle.crypto.generators.Argon2BytesGenerator");
      return true;
    } catch (final ClassNotFoundException | LinkageError ignored) {
      return false;
    }
  }

  private static String hkdfSaltDomain() {
    return "iroha-android-software-export-v3-salt";
  }

  private static String hkdfInfoDomain() {
    return "iroha-android-software-export-v3-info";
  }

  private static void guardSaltNonceReuse(final byte[] salt, final byte[] nonce)
      throws KeyExportException {
    if (salt == null || nonce == null) {
      throw new KeyExportException("Salt and nonce must be present");
    }
    final byte[] fingerprintMaterial =
        ByteBuffer.allocate(salt.length + nonce.length).put(salt).put(nonce).array();
    final String fingerprint =
        HexFormat.of()
            .formatHex(
                sha256(
                    "iroha-android-software-export-v3-fingerprint", fingerprintMaterial));
    Arrays.fill(fingerprintMaterial, (byte) 0);
    synchronized (RECENT_EXPORT_INDEX) {
      if (RECENT_EXPORT_INDEX.contains(fingerprint)) {
        throw new KeyExportException("Salt/nonce pair must not be reused across exports");
      }
      RECENT_EXPORT_INDEX.add(fingerprint);
      RECENT_EXPORT_FINGERPRINTS.addLast(fingerprint);
      while (RECENT_EXPORT_INDEX.size() > MAX_TRACKED_EXPORTS) {
        final String evicted = RECENT_EXPORT_FINGERPRINTS.removeFirst();
        RECENT_EXPORT_INDEX.remove(evicted);
      }
    }
  }

  private static void fillRandomNonZero(final byte[] target) {
    if (target == null) {
      return;
    }
    do {
      SECURE_RANDOM.nextBytes(target);
    } while (isAllZero(target));
  }

  private static boolean isAllZero(final byte[] data) {
    if (data == null) {
      return true;
    }
    for (final byte b : data) {
      if (b != 0) {
        return false;
      }
    }
    return true;
  }

  private static void ensurePassphraseStrength(final char[] passphrase) throws KeyExportException {
    if (passphrase == null || passphrase.length < MIN_PASSPHRASE_LENGTH) {
      throw new KeyExportException(
          "Passphrase must be at least " + MIN_PASSPHRASE_LENGTH + " characters long");
    }
  }

  private record KdfResult(byte[] key, int kind, int workFactor) {
    void zero() {
      if (key != null) {
        Arrays.fill(key, (byte) 0);
      }
    }
  }

  /** Holds derived key pair material. */
  public static final class KeyPairData {
    private final PrivateKey privateKey;
    private final PublicKey publicKey;

    KeyPairData(final PrivateKey privateKey, final PublicKey publicKey) {
      this.privateKey = privateKey;
      this.publicKey = publicKey;
    }

    public PrivateKey privateKey() {
      return privateKey;
    }

    public PublicKey publicKey() {
      return publicKey;
    }
  }

}
