package org.hyperledger.iroha.android.crypto;

import java.io.File;
import java.security.KeyPair;
import java.util.Arrays;
import org.hyperledger.iroha.android.crypto.export.FileKeyExportStore;
import org.hyperledger.iroha.android.crypto.export.InMemoryKeyExportStore;
import org.hyperledger.iroha.android.crypto.export.KeyExportStore;
import org.hyperledger.iroha.android.crypto.export.KeyPassphraseProvider;

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
            SoftwareKeyProvider.ProviderPolicy.BOUNCY_CASTLE_PREFERRED,
            store,
            passphraseProvider);

    final KeyPair generated = provider.generate("stored-alias");

    final SoftwareKeyProvider restoredProvider =
        new SoftwareKeyProvider(
            SoftwareKeyProvider.ProviderPolicy.BOUNCY_CASTLE_PREFERRED,
            store,
            passphraseProvider);
    final KeyPair restored =
        restoredProvider
            .load("stored-alias")
            .orElseThrow(() -> new AssertionError("Expected stored alias to be present"));

    assert Arrays.equals(generated.getPrivate().getEncoded(), restored.getPrivate().getEncoded())
        : "Restored key material must match the stored export";
  }

  private static void shouldRestoreFromFileExportStore() throws Exception {
    final File storeFile = File.createTempFile("iroha-keys", ".properties");
    storeFile.deleteOnExit();
    final KeyExportStore store = new FileKeyExportStore(storeFile);
    final KeyPassphraseProvider passphraseProvider = () -> "file-passphrase".toCharArray();
    final SoftwareKeyProvider provider =
        new SoftwareKeyProvider(
            SoftwareKeyProvider.ProviderPolicy.BOUNCY_CASTLE_PREFERRED,
            store,
            passphraseProvider);

    final KeyPair generated = provider.generate("file-alias");

    final SoftwareKeyProvider restoredProvider =
        new SoftwareKeyProvider(
            SoftwareKeyProvider.ProviderPolicy.BOUNCY_CASTLE_PREFERRED,
            store,
            passphraseProvider);
    final KeyPair restored =
        restoredProvider
            .load("file-alias")
            .orElseThrow(() -> new AssertionError("Expected file alias to be present"));

    assert Arrays.equals(generated.getPrivate().getEncoded(), restored.getPrivate().getEncoded())
        : "File-backed key material must match the stored export";
  }
}
