package org.hyperledger.iroha.android.crypto.export;

import java.nio.charset.StandardCharsets;
import java.security.KeyFactory;
import java.security.KeyPair;
import java.security.PrivateKey;
import java.security.PublicKey;
import java.security.spec.PKCS8EncodedKeySpec;
import java.security.spec.X509EncodedKeySpec;
import java.util.Arrays;
import java.util.Base64;

/**
 * Locks down deterministic fallback exports against a stable golden vector so the derivation is
 * reproducible across JVMs and language bindings.
 */
public final class DeterministicKeyExporterGoldenVectorTests {

  private static final String ALIAS = "fallback-deterministic-vector";
  private static final char[] PASSPHRASE = "vector passphrase".toCharArray();
  private static final int MAGIC_LENGTH = "IRKEY".getBytes(StandardCharsets.UTF_8).length;

  private static final String PRIVATE_KEY_BASE64 =
      "MC4CAQAwBQYDK2VwBCIEIDFvnHJxgIILOohiukaja/BMWmhjN/ir3nNHNIW5xja7";
  private static final String PUBLIC_KEY_BASE64 =
      "MCowBQYDK2VwAyEAyvpirpRdvDlT7YLyUkDdpQnc2U+BaudnqeWAd8ioeTU=";
  private static final String EXPORT_BUNDLE_BASE64 =
      "SVJLRVkBAB1mYWxsYmFjay1kZXRlcm1pbmlzdGljLXZlY3RvcgAsMCowBQYDK2VwAyEAyvpirpRdvDlT7YLyUkDdpQnc2U+BaudnqeWAd8ioeTUM4GiQ2D8gfmTAv0g+AEDLIQpgFdawSTLdJ/aHWW92zv/WwRDL/ktyF6rnZ6F+D4OFBOmxnex87ctNZXXh1axvs0ybqrtJ7EMc+nUhf8In";

  private DeterministicKeyExporterGoldenVectorTests() {}

  public static void main(final String[] args) throws Exception {
    verifyLegacyVectorDecodes();
    verifyVersionZeroVectorDecodes();
    verifyImportRestoresGoldenVector();
    verifyNewExportsUseV3();
    System.out.println("[IrohaAndroid] Deterministic key exporter golden vector tests passed.");
  }

  private static void verifyLegacyVectorDecodes() throws Exception {
    final KeyExportBundle bundle = KeyExportBundle.decodeBase64(EXPORT_BUNDLE_BASE64);
    assert bundle.version() == KeyExportBundle.VERSION_V1 : "Golden vector must remain v1 for compatibility";
    assert ALIAS.equals(bundle.alias()) : "Alias should be preserved";
    assert bundle.salt().length == 0 : "Legacy vector should not carry salt";
  }

  private static void verifyVersionZeroVectorDecodes() throws Exception {
    final byte[] raw = Base64.getDecoder().decode(EXPORT_BUNDLE_BASE64);
    raw[MAGIC_LENGTH] = KeyExportBundle.VERSION_V0;
    final KeyExportBundle bundle = KeyExportBundle.decode(raw);
    assert bundle.isLegacyDeterministic() : "Version 0 bundles must be accepted as legacy";
    final char[] passphrase = PASSPHRASE.clone();
    final DeterministicKeyExporter.KeyPairData data =
        DeterministicKeyExporter.importKeyPair(bundle, passphrase);
    Arrays.fill(passphrase, '\0');
    final KeyPair pair = loadGoldenKeyPair();
    assert Arrays.equals(pair.getPrivate().getEncoded(), data.privateKey().getEncoded())
        : "Version 0 decode must restore golden private key";
    assert Arrays.equals(pair.getPublic().getEncoded(), data.publicKey().getEncoded())
        : "Version 0 decode must restore golden public key";
  }

  private static void verifyImportRestoresGoldenVector() throws Exception {
    final KeyExportBundle bundle = KeyExportBundle.decodeBase64(EXPORT_BUNDLE_BASE64);
    final char[] passphrase = PASSPHRASE.clone();
    final DeterministicKeyExporter.KeyPairData data =
        DeterministicKeyExporter.importKeyPair(bundle, passphrase);
    Arrays.fill(passphrase, '\0');

    final KeyPair pair = loadGoldenKeyPair();
    assert Arrays.equals(pair.getPrivate().getEncoded(), data.privateKey().getEncoded())
        : "Imported private key must match golden vector";
    assert Arrays.equals(pair.getPublic().getEncoded(), data.publicKey().getEncoded())
        : "Imported public key must match golden vector";
  }

  private static void verifyNewExportsUseV3() throws Exception {
    final KeyPair pair = loadGoldenKeyPair();
    final char[] passphrase = PASSPHRASE.clone();
    final KeyExportBundle bundle =
        DeterministicKeyExporter.exportKeyPair(
            pair.getPrivate(), pair.getPublic(), ALIAS, passphrase);
    Arrays.fill(passphrase, '\0');
    assert bundle.version() == KeyExportBundle.VERSION_V3 : "New exports must emit v3 bundles";
    assert bundle.salt().length > 0 : "V3 exports must carry salt";
    boolean argonAvailable = false;
    try {
      Class.forName("org.bouncycastle.crypto.generators.Argon2BytesGenerator");
      argonAvailable = true;
    } catch (final ClassNotFoundException | LinkageError ignored) {
    }
    if (argonAvailable) {
      assert bundle.kdfKind() == DeterministicKeyExporter.KDF_KIND_ARGON2ID
          : "V3 exports must prefer Argon2id when available";
    } else {
      assert bundle.kdfKind() == DeterministicKeyExporter.KDF_KIND_PBKDF2_HMAC_SHA256
          : "PBKDF2 fallback should be recorded when Argon2 is unavailable";
    }
    assert bundle.kdfWorkFactor() > 0 : "V3 exports must record work factor";
  }

  private static KeyPair loadGoldenKeyPair() throws Exception {
    final KeyFactory factory = KeyFactory.getInstance("Ed25519");
    final byte[] privateDer = Base64.getDecoder().decode(PRIVATE_KEY_BASE64);
    final byte[] publicDer = Base64.getDecoder().decode(PUBLIC_KEY_BASE64);
    final PrivateKey privateKey = factory.generatePrivate(new PKCS8EncodedKeySpec(privateDer));
    final PublicKey publicKey = factory.generatePublic(new X509EncodedKeySpec(publicDer));
    return new KeyPair(publicKey, privateKey);
  }
}
