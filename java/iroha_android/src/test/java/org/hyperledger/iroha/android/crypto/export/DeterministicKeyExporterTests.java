package org.hyperledger.iroha.android.crypto.export;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.security.NoSuchProviderException;
import java.security.Provider;
import java.security.Security;
import java.util.Arrays;
import javax.crypto.Cipher;
import javax.crypto.Mac;
import javax.crypto.spec.GCMParameterSpec;
import javax.crypto.spec.SecretKeySpec;
import org.hyperledger.iroha.android.crypto.SoftwareKeyProvider;

public final class DeterministicKeyExporterTests {

  private static final int MAGIC_LENGTH = "IRKEY".getBytes(StandardCharsets.UTF_8).length;

  private DeterministicKeyExporterTests() {}

  public static void main(final String[] args) throws Exception {
    exportIsRandomized();
    importRecoversOriginalKey();
    exportSupportsBouncyCastleKeys();
    wrongPassphraseFails();
    corruptedBundleFails();
    rejectsNonEd25519Keys();
    tamperedSaltFails();
    tamperedNonceFails();
    saltLengthTamperFails();
    nonceLengthTamperFails();
    legacyBundleRemainsDecodable();
    shortPassphraseRejected();
    zeroSaltOrNonceRejected();
    System.out.println("[IrohaAndroid] Deterministic key exporter tests passed.");
  }

  private static void exportIsRandomized() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final KeyPair keyPair = provider.generate("alias");
    final char[] passphrase = "correct horse battery staple".toCharArray();

    final KeyExportBundle bundleA =
        DeterministicKeyExporter.exportKeyPair(keyPair.getPrivate(), keyPair.getPublic(), "alias", passphrase);
    final KeyExportBundle bundleB =
        DeterministicKeyExporter.exportKeyPair(keyPair.getPrivate(), keyPair.getPublic(), "alias", passphrase);

    assert !bundleA.encodeBase64().equals(bundleB.encodeBase64())
        : "Export must include per-run randomness (salt/nonce)";
    Arrays.fill(passphrase, '\0');
  }

  private static void importRecoversOriginalKey() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final KeyPair original = provider.generate("alias");
    final char[] passphrase = "passphrase-longer".toCharArray();

    final KeyExportBundle bundle =
        provider.exportDeterministic("alias", passphrase);
    provider.importDeterministic(bundle, passphrase);
    final KeyPair recovered = provider.load("alias").orElseThrow();

    assert Arrays.equals(original.getPrivate().getEncoded(), recovered.getPrivate().getEncoded())
        : "Private key should round-trip";
    assert Arrays.equals(original.getPublic().getEncoded(), recovered.getPublic().getEncoded())
        : "Public key should round-trip";
    Arrays.fill(passphrase, '\0');
  }

  private static void exportSupportsBouncyCastleKeys() throws Exception {
    if (!ensureBouncyCastleProvider()) {
      System.out.println(
          "[IrohaAndroid] Skipping BouncyCastle export test (provider unavailable).");
      return;
    }
    final KeyPairGenerator generator = bouncyCastleEd25519Generator();
    final KeyPair keyPair = generator.generateKeyPair();
    final char[] passphrase = "bouncy-castle-passphrase".toCharArray();
    final KeyExportBundle bundle =
        DeterministicKeyExporter.exportKeyPair(
            keyPair.getPrivate(), keyPair.getPublic(), "alias", passphrase);
    final DeterministicKeyExporter.KeyPairData imported =
        DeterministicKeyExporter.importKeyPair(bundle, passphrase);
    assert Arrays.equals(keyPair.getPrivate().getEncoded(), imported.privateKey().getEncoded())
        : "BouncyCastle private key should round-trip";
    assert Arrays.equals(keyPair.getPublic().getEncoded(), imported.publicKey().getEncoded())
        : "BouncyCastle public key should round-trip";
    Arrays.fill(passphrase, '\0');
  }

  private static void wrongPassphraseFails() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    provider.generate("alias");
    final KeyExportBundle bundle =
        provider.exportDeterministic("alias", "expected-passphrase".toCharArray());
    boolean threw = false;
    try {
      provider.importDeterministic(bundle, "wrong-passphrase".toCharArray());
    } catch (final KeyExportException ex) {
      threw = true;
    }
    assert threw : "Import must fail when passphrase is incorrect";
  }

  private static void rejectsNonEd25519Keys() throws Exception {
    final KeyPairGenerator generator = KeyPairGenerator.getInstance("RSA");
    generator.initialize(2048);
    final KeyPair keyPair = generator.generateKeyPair();
    boolean threw = false;
    final char[] passphrase = "non-ed25519-passphrase".toCharArray();
    try {
      DeterministicKeyExporter.exportKeyPair(
          keyPair.getPrivate(), keyPair.getPublic(), "alias", passphrase);
    } catch (final KeyExportException expected) {
      threw = true;
    } finally {
      Arrays.fill(passphrase, '\0');
    }
    assert threw : "Non-Ed25519 keys must be rejected";
  }

  private static void corruptedBundleFails() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final KeyPair keyPair = provider.generate("alias");
    final char[] passphrase = "passphrase-correct".toCharArray();
    final KeyExportBundle bundle =
        DeterministicKeyExporter.exportKeyPair(keyPair.getPrivate(), keyPair.getPublic(), "alias", passphrase);
    final byte[] raw = bundle.encode();
    // Flip final ciphertext byte to corrupt authentication tag.
    raw[raw.length - 1] ^= (byte) 0xFF;
    boolean threw = false;
    try {
      final KeyExportBundle corrupted = KeyExportBundle.decode(raw);
      provider.importDeterministic(corrupted, passphrase);
    } catch (final KeyExportException ex) {
      threw = true;
    }
    assert threw : "Import should fail when bundle ciphertext is corrupted";
  }

  private static void tamperedSaltFails() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final KeyPair keyPair = provider.generate("alias-salt");
    final char[] passphrase = "tamper-resistant-passphrase".toCharArray();
    final KeyExportBundle bundle =
        DeterministicKeyExporter.exportKeyPair(
            keyPair.getPrivate(), keyPair.getPublic(), "alias-salt", passphrase);
    final byte[] salt = bundle.salt();
    salt[0] ^= 0xFF;
    final KeyExportBundle tampered =
        new KeyExportBundle(
            bundle.alias(),
            bundle.publicKey(),
            bundle.nonce(),
            bundle.ciphertext(),
            salt,
            bundle.kdfKind(),
            bundle.kdfWorkFactor(),
            bundle.version());
    boolean threw = false;
    try {
      DeterministicKeyExporter.importKeyPair(tampered, passphrase);
    } catch (final KeyExportException expected) {
      threw = true;
    }
    assert threw : "Import must fail when salt is tampered";
    Arrays.fill(passphrase, '\0');
  }

  private static void tamperedNonceFails() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final KeyPair keyPair = provider.generate("alias-nonce");
    final char[] passphrase = "nonce-tamper-passphrase".toCharArray();
    final KeyExportBundle bundle =
        DeterministicKeyExporter.exportKeyPair(
            keyPair.getPrivate(), keyPair.getPublic(), "alias-nonce", passphrase);
    final byte[] nonce = bundle.nonce();
    nonce[nonce.length - 1] ^= 0xFF;
    final KeyExportBundle tampered =
        new KeyExportBundle(
            bundle.alias(),
            bundle.publicKey(),
            nonce,
            bundle.ciphertext(),
            bundle.salt(),
            bundle.kdfKind(),
            bundle.kdfWorkFactor(),
            bundle.version());
    boolean threw = false;
    try {
      DeterministicKeyExporter.importKeyPair(tampered, passphrase);
    } catch (final KeyExportException expected) {
      threw = true;
    }
    assert threw : "Import must fail when nonce is tampered";
    Arrays.fill(passphrase, '\0');
  }

  private static void saltLengthTamperFails() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final KeyPair keyPair = provider.generate("alias-salt-length");
    final char[] passphrase = "tamper-resistant-passphrase".toCharArray();
    final KeyExportBundle bundle =
        DeterministicKeyExporter.exportKeyPair(
            keyPair.getPrivate(), keyPair.getPublic(), "alias-salt-length", passphrase);
    final byte[] raw = bundle.encode();
    final int saltLengthOffset = offsetToSaltLength(raw);
    raw[saltLengthOffset] = 0; // invalid length
    boolean threw = false;
    try {
      KeyExportBundle.decode(raw);
    } catch (final KeyExportException expected) {
      threw = true;
    }
    assert threw : "Decoder must reject salt length tampering";
    Arrays.fill(passphrase, '\0');
  }

  private static void nonceLengthTamperFails() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final KeyPair keyPair = provider.generate("alias-nonce-length");
    final char[] passphrase = "nonce-tamper-passphrase".toCharArray();
    final KeyExportBundle bundle =
        DeterministicKeyExporter.exportKeyPair(
            keyPair.getPrivate(), keyPair.getPublic(), "alias-nonce-length", passphrase);
    final byte[] raw = bundle.encode();
    final int nonceLengthOffset = offsetToNonceLength(raw);
    raw[nonceLengthOffset] = 0; // invalid length
    boolean threw = false;
    try {
      KeyExportBundle.decode(raw);
    } catch (final KeyExportException expected) {
      threw = true;
    }
    assert threw : "Decoder must reject nonce length tampering";
    Arrays.fill(passphrase, '\0');
  }

  private static int offsetToNonceLength(final byte[] raw) {
    int offset = MAGIC_LENGTH;
    offset += 1; // version
    final int aliasLength = readU16(raw, offset);
    offset += 2 + aliasLength;
    final int publicKeyLength = readU16(raw, offset);
    offset += 2 + publicKeyLength;
    return offset;
  }

  private static int offsetToSaltLength(final byte[] raw) {
    int offset = offsetToNonceLength(raw);
    final int nonceLength = raw[offset] & 0xFF;
    offset += 1 + nonceLength;
    final int cipherLength = readU16(raw, offset);
    offset += 2 + cipherLength;
    return offset;
  }

  private static int readU16(final byte[] raw, final int offset) {
    return ((raw[offset] & 0xFF) << 8) | (raw[offset + 1] & 0xFF);
  }

  private static boolean ensureBouncyCastleProvider() {
    try {
      final Class<?> providerClass =
          Class.forName("org.bouncycastle.jce.provider.BouncyCastleProvider");
      final Provider provider =
          (Provider) providerClass.getDeclaredConstructor().newInstance();
      if (Security.getProvider(provider.getName()) == null) {
        Security.addProvider(provider);
      }
      return true;
    } catch (final ReflectiveOperationException | ClassCastException ignored) {
      return false;
    }
  }

  private static KeyPairGenerator bouncyCastleEd25519Generator() throws Exception {
    try {
      return KeyPairGenerator.getInstance("Ed25519", "BC");
    } catch (final NoSuchAlgorithmException | NoSuchProviderException ex) {
      return KeyPairGenerator.getInstance("EdDSA", "BC");
    }
  }

  private static void legacyBundleRemainsDecodable() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final KeyPair legacyKeyPair = provider.generate("legacy-alias");
    final char[] passphrase = "legacy-passphrase".toCharArray();

    final KeyExportBundle legacyBundle =
        exportLegacyBundle(legacyKeyPair, "legacy-alias", passphrase);
    final byte[] encoded = legacyBundle.encode();
    final KeyExportBundle decoded = KeyExportBundle.decode(encoded);
    final DeterministicKeyExporter.KeyPairData imported =
        DeterministicKeyExporter.importKeyPair(decoded, passphrase);

    assert Arrays.equals(legacyKeyPair.getPrivate().getEncoded(), imported.privateKey().getEncoded())
        : "Legacy v0 bundle should still import correctly";
    assert Arrays.equals(legacyKeyPair.getPublic().getEncoded(), imported.publicKey().getEncoded())
        : "Legacy v0 bundle should preserve the public key";
    Arrays.fill(passphrase, '\0');
  }

  private static void shortPassphraseRejected() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final KeyPair keyPair = provider.generate("short-pass");
    final char[] weak = "too-short".toCharArray();
    boolean threw = false;
    try {
      DeterministicKeyExporter.exportKeyPair(
          keyPair.getPrivate(), keyPair.getPublic(), "short-pass", weak);
    } catch (final KeyExportException expected) {
      threw = true;
    }
    assert threw : "Exporter must enforce minimum passphrase length for v3 bundles";
    Arrays.fill(weak, '\0');
  }

  private static void zeroSaltOrNonceRejected() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final KeyPair keyPair = provider.generate("zero-salt");
    final char[] passphrase = "reject-zero-seeds".toCharArray();
    final KeyExportBundle valid =
        DeterministicKeyExporter.exportKeyPair(
            keyPair.getPrivate(), keyPair.getPublic(), "zero-salt", passphrase);

    final byte[] zeroSalt = new byte[KeyExportBundle.EXPECTED_SALT_LENGTH_BYTES];
    final KeyExportBundle zeroSaltBundle =
        new KeyExportBundle(
            valid.alias(),
            valid.publicKey(),
            valid.nonce(),
            valid.ciphertext(),
            zeroSalt,
            valid.kdfKind(),
            valid.kdfWorkFactor(),
            valid.version());
    boolean saltThrew = false;
    try {
      DeterministicKeyExporter.importKeyPair(zeroSaltBundle, passphrase);
    } catch (final KeyExportException expected) {
      saltThrew = true;
    }
    assert saltThrew : "Import must reject all-zero salt";

    final byte[] zeroNonce = new byte[KeyExportBundle.EXPECTED_NONCE_LENGTH_BYTES];
    final KeyExportBundle zeroNonceBundle =
        new KeyExportBundle(
            valid.alias(),
            valid.publicKey(),
            zeroNonce,
            valid.ciphertext(),
            valid.salt(),
            valid.kdfKind(),
            valid.kdfWorkFactor(),
            valid.version());
    boolean nonceThrew = false;
    try {
      DeterministicKeyExporter.importKeyPair(zeroNonceBundle, passphrase);
    } catch (final KeyExportException expected) {
      nonceThrew = true;
    }
    assert nonceThrew : "Import must reject all-zero nonce";
    Arrays.fill(passphrase, '\0');
  }

  private static KeyExportBundle exportLegacyBundle(
      final KeyPair keyPair, final String alias, final char[] passphrase) throws Exception {
    final byte[] aliasBytes = alias.getBytes(StandardCharsets.UTF_8);
    final byte[] salt = sha256("iroha-android-software-export-salt", aliasBytes);
    final byte[] info = legacyInfo("iroha-android-software-export-info", aliasBytes);
    final byte[] passphraseBytes = encodeUtf8(passphrase);
    final byte[] keyNonceMaterial;
    try {
      keyNonceMaterial =
          hkdf(passphraseBytes, salt, info, 32 + KeyExportBundle.EXPECTED_NONCE_LENGTH_BYTES);
    } finally {
      Arrays.fill(passphraseBytes, (byte) 0);
    }
    final byte[] aesKey = Arrays.copyOfRange(keyNonceMaterial, 0, 32);
    final byte[] nonce =
        Arrays.copyOfRange(keyNonceMaterial, 32, keyNonceMaterial.length);
    Arrays.fill(keyNonceMaterial, (byte) 0);

    final Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
    cipher.init(
        Cipher.ENCRYPT_MODE, new SecretKeySpec(aesKey, "AES"), new GCMParameterSpec(128, nonce));
    cipher.updateAAD(aliasBytes);
    final byte[] ciphertext = cipher.doFinal(keyPair.getPrivate().getEncoded());
    Arrays.fill(aesKey, (byte) 0);

    return new KeyExportBundle(
        alias,
        keyPair.getPublic().getEncoded(),
        nonce,
        ciphertext,
        new byte[0],
        0,
        0,
        KeyExportBundle.VERSION_V0);
  }

  private static byte[] legacyInfo(final String domain, final byte[] aliasBytes) {
    final byte[] domainBytes = domain.getBytes(StandardCharsets.UTF_8);
    final ByteBuffer buffer =
        ByteBuffer.allocate(domainBytes.length + 2 + aliasBytes.length)
            .put(domainBytes)
            .putShort((short) aliasBytes.length)
            .put(aliasBytes);
    return buffer.array();
  }

  private static byte[] encodeUtf8(final char[] chars) {
    final byte[] encoded = new String(chars).getBytes(StandardCharsets.UTF_8);
    return encoded;
  }

  private static byte[] hkdf(
      final byte[] ikm, final byte[] salt, final byte[] info, final int length) throws Exception {
    final Mac mac = Mac.getInstance("HmacSHA256");
    final byte[] saltValue = salt == null ? new byte[mac.getMacLength()] : salt.clone();
    mac.init(new SecretKeySpec(saltValue, "HmacSHA256"));
    final byte[] prk = mac.doFinal(ikm);
    final byte[] result = new byte[length];
    byte[] previous = new byte[0];
    int offset = 0;
    int blockIndex = 1;
    while (offset < length) {
      mac.init(new SecretKeySpec(prk, "HmacSHA256"));
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
    Arrays.fill(saltValue, (byte) 0);
    return result;
  }

  private static byte[] sha256(final String domain, final byte[] data) throws Exception {
    final MessageDigest digest = MessageDigest.getInstance("SHA-256");
    digest.update(domain.getBytes(StandardCharsets.UTF_8));
    digest.update(data);
    return digest.digest();
  }
}
