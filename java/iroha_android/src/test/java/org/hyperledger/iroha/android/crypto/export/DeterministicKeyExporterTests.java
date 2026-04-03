package org.hyperledger.iroha.android.crypto.export;

import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.NoSuchAlgorithmException;
import java.security.NoSuchProviderException;
import java.security.Provider;
import java.security.Security;
import java.util.Arrays;
import org.hyperledger.iroha.android.crypto.NativeSignerBridge;
import org.hyperledger.iroha.android.crypto.SigningAlgorithm;
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
    mlDsaExportRoundTrips();
    mixedAlgorithmRestoreRejected();
    saltLengthTamperFails();
    nonceLengthTamperFails();
    rejectsUnsupportedVersion();
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

  private static void mlDsaExportRoundTrips() throws Exception {
    if (!NativeSignerBridge.isNativeAvailable()) {
      System.out.println(
          "[IrohaAndroid] Skipping ML-DSA export test (native bridge unavailable).");
      return;
    }
    final SoftwareKeyProvider provider = new SoftwareKeyProvider(SigningAlgorithm.ML_DSA);
    final KeyPair original = provider.generate("ml-dsa-alias");
    final char[] passphrase = "ml-dsa-export-passphrase".toCharArray();

    final KeyExportBundle bundle = provider.exportDeterministic("ml-dsa-alias", passphrase);
    final KeyPair recovered = provider.importDeterministic(bundle, passphrase);

    assert bundle.signingAlgorithm() == SigningAlgorithm.ML_DSA
        : "Bundle should retain ML-DSA metadata";
    assert Arrays.equals(original.getPrivate().getEncoded(), recovered.getPrivate().getEncoded())
        : "ML-DSA private key should round-trip";
    assert Arrays.equals(original.getPublic().getEncoded(), recovered.getPublic().getEncoded())
        : "ML-DSA public key should round-trip";
    Arrays.fill(passphrase, '\0');
  }

  private static void mixedAlgorithmRestoreRejected() throws Exception {
    if (!NativeSignerBridge.isNativeAvailable()) {
      System.out.println(
          "[IrohaAndroid] Skipping mixed-algorithm export test (native bridge unavailable).");
      return;
    }
    final SoftwareKeyProvider mlDsaProvider = new SoftwareKeyProvider(SigningAlgorithm.ML_DSA);
    mlDsaProvider.generate("ml-dsa-cross-alias");
    final char[] passphrase = "ml-dsa-export-passphrase".toCharArray();
    final KeyExportBundle bundle = mlDsaProvider.exportDeterministic("ml-dsa-cross-alias", passphrase);

    final SoftwareKeyProvider ed25519Provider = new SoftwareKeyProvider(SigningAlgorithm.ED25519);
    boolean threw = false;
    try {
      ed25519Provider.importDeterministic(bundle, passphrase);
    } catch (final KeyExportException expected) {
      threw = true;
    }
    assert threw : "Provider must reject deterministic exports with a mismatched algorithm";
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
    final int version = raw[offset] & 0xFF;
    offset += 1; // version
    if (version >= KeyExportBundle.VERSION_V4) {
      offset += 1; // algorithm code
    }
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

  private static void rejectsUnsupportedVersion() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final KeyPair keyPair = provider.generate("version-reject");
    final char[] passphrase = "versioned-passphrase".toCharArray();
    final KeyExportBundle bundle =
        DeterministicKeyExporter.exportKeyPair(
            keyPair.getPrivate(), keyPair.getPublic(), "version-reject", passphrase);
    final byte[] raw = bundle.encode();
    raw[MAGIC_LENGTH] = 0x01;
    boolean threw = false;
    try {
      KeyExportBundle.decode(raw);
    } catch (final KeyExportException expected) {
      threw = true;
    }
    assert threw : "Decoder must reject unsupported bundle versions";
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
    assert threw : "Exporter must enforce minimum passphrase length";
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

}
