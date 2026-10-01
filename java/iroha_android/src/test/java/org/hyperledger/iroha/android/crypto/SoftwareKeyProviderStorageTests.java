package org.hyperledger.iroha.android.crypto;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.Signature;
import java.util.Arrays;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.bouncycastle.crypto.util.PrivateKeyFactory;
import org.hyperledger.iroha.sdk.crypto.SigningAlgorithm;
import org.hyperledger.iroha.sdk.crypto.SoftwareKeyProvider;
import org.hyperledger.iroha.sdk.crypto.export.FileKeyExportStore;
import org.hyperledger.iroha.sdk.crypto.export.InMemoryKeyExportStore;
import org.hyperledger.iroha.sdk.crypto.export.KeyExportStore;
import org.hyperledger.iroha.sdk.crypto.export.KeyPassphraseProvider;

public final class SoftwareKeyProviderStorageTests {

  private SoftwareKeyProviderStorageTests() {}

  public static void main(final String[] args) throws Exception {
    shouldRestoreFromExportStore();
    shouldRestoreFromFileExportStore();
    System.out.println("[IrohaAndroid] SoftwareKeyProvider storage tests passed.");
  }

  private static void shouldRestoreFromExportStore() throws Exception {
    final KeyExportStore store = new InMemoryKeyExportStore();
    final KeyPassphraseProvider passphraseProvider = () -> "storage-passphrase".toCharArray();
    final SoftwareKeyProvider provider =
        new SoftwareKeyProvider(
            SoftwareKeyProvider.ProviderPolicy.BOUNCY_CASTLE_REQUIRED,
            store,
            passphraseProvider,
            SigningAlgorithm.ED25519);

    final KeyPair generated = provider.generate("stored-alias");

    final SoftwareKeyProvider restoredProvider =
        new SoftwareKeyProvider(
            SoftwareKeyProvider.ProviderPolicy.BOUNCY_CASTLE_REQUIRED,
            store,
            passphraseProvider,
            SigningAlgorithm.ED25519);
    final KeyPair restored = restoredProvider.load("stored-alias");
    if (restored == null) {
      throw new AssertionError("Expected stored alias to be present");
    }

    assert samePrivateSeed(generated, restored)
        : "Restored key material must match the stored export";
    assertRestoredIdentity(generated, restored);
  }

  private static void shouldRestoreFromFileExportStore() throws Exception {
    final File storeFile = File.createTempFile("iroha-keys", ".properties");
    storeFile.deleteOnExit();
    final KeyExportStore store = new FileKeyExportStore(storeFile);
    final KeyPassphraseProvider passphraseProvider = () -> "file-passphrase".toCharArray();
    final SoftwareKeyProvider provider =
        new SoftwareKeyProvider(
            SoftwareKeyProvider.ProviderPolicy.BOUNCY_CASTLE_REQUIRED,
            store,
            passphraseProvider,
            SigningAlgorithm.ED25519);

    final KeyPair generated = provider.generate("file-alias");

    final SoftwareKeyProvider restoredProvider =
        new SoftwareKeyProvider(
            SoftwareKeyProvider.ProviderPolicy.BOUNCY_CASTLE_REQUIRED,
            store,
            passphraseProvider,
            SigningAlgorithm.ED25519);
    final KeyPair restored = restoredProvider.load("file-alias");
    if (restored == null) {
      throw new AssertionError("Expected file alias to be present");
    }

    assert samePrivateSeed(generated, restored)
        : "File-backed key material must match the stored export";
    assertRestoredIdentity(generated, restored);
  }

  private static boolean samePrivateSeed(final KeyPair generated, final KeyPair restored)
      throws Exception {
    // JCA providers may encode the same Ed25519 seed in different PKCS8 containers.
    final byte[] generatedEncoding = generated.getPrivate().getEncoded();
    final byte[] restoredEncoding = restored.getPrivate().getEncoded();
    byte[] generatedSeed = null;
    byte[] restoredSeed = null;
    try {
      generatedSeed =
          ((Ed25519PrivateKeyParameters) PrivateKeyFactory.createKey(generatedEncoding)).getEncoded();
      restoredSeed =
          ((Ed25519PrivateKeyParameters) PrivateKeyFactory.createKey(restoredEncoding)).getEncoded();
      assert generatedSeed.length == 32 && restoredSeed.length == 32 : "expected Ed25519 seeds";
      return Arrays.equals(generatedSeed, restoredSeed);
    } finally {
      Arrays.fill(generatedEncoding, (byte) 0);
      Arrays.fill(restoredEncoding, (byte) 0);
      if (generatedSeed != null) Arrays.fill(generatedSeed, (byte) 0);
      if (restoredSeed != null) Arrays.fill(restoredSeed, (byte) 0);
    }
  }

  private static void assertRestoredIdentity(final KeyPair generated, final KeyPair restored)
      throws Exception {
    assert Arrays.equals(generated.getPublic().getEncoded(), restored.getPublic().getEncoded())
        : "Restored public key must match the original identity";
    final byte[] message = "canonical-key-restoration".getBytes(StandardCharsets.UTF_8);
    final Signature signer = Signature.getInstance("Ed25519", "BC");
    signer.initSign(generated.getPrivate());
    signer.update(message);
    final byte[] originalSignature = signer.sign();
    signer.initSign(restored.getPrivate());
    signer.update(message);
    final byte[] restoredSignature = signer.sign();
    assert Arrays.equals(originalSignature, restoredSignature)
        : "Restored private seed must produce the original deterministic signature";
    signer.initVerify(restored.getPublic());
    signer.update(message);
    assert signer.verify(originalSignature) : "Restored public key must verify the original signature";
    signer.initVerify(generated.getPublic());
    signer.update(message);
    assert signer.verify(restoredSignature) : "Original public key must verify the restored signature";
    message[0] ^= 1;
    signer.initVerify(restored.getPublic());
    signer.update(message);
    assert !signer.verify(originalSignature) : "Restoration must preserve altered-message refusal";
  }
}
