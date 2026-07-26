package org.hyperledger.iroha.android;

import java.util.List;
import org.hyperledger.iroha.android.crypto.SoftwareKeyProvider;
import org.hyperledger.iroha.android.crypto.export.InMemoryKeyExportStore;
import org.hyperledger.iroha.android.crypto.export.KeyExportBundle;
import org.hyperledger.iroha.android.crypto.export.KeyPassphraseProvider;

public final class IrohaKeyManagerDeterministicExportTests {

  private IrohaKeyManagerDeterministicExportTests() {}

  public static void main(final String[] args) throws Exception {
    shouldExportAndImportDeterministicKey();
    shouldPersistExportableSoftwareKeys();
    System.out.println("[IrohaAndroid] Key manager deterministic export tests passed.");
  }

  private static void shouldExportAndImportDeterministicKey() throws Exception {
    final SoftwareKeyProvider originalProvider = new SoftwareKeyProvider();
    final IrohaKeyManager keyManager = IrohaKeyManager.fromProviders(List.of(originalProvider));
    final String alias = "deterministic-alias";
    keyManager.generateOrLoad(alias, IrohaKeyManager.KeySecurityPreference.SOFTWARE_ONLY);

    final char[] passphrase = "export-passphrase".toCharArray();
    final KeyExportBundle bundle = keyManager.exportDeterministicKey(alias, passphrase);

    final SoftwareKeyProvider restoredProvider = new SoftwareKeyProvider();
    final IrohaKeyManager restoredManager =
        IrohaKeyManager.fromProviders(List.of(restoredProvider));
    restoredManager.importDeterministicKey(bundle, passphrase);
    final java.security.KeyPair recovered =
        restoredManager.generateOrLoad(alias, IrohaKeyManager.KeySecurityPreference.SOFTWARE_ONLY);
    assert recovered.getPrivate() != null : "Recovered key pair should contain private key";
    assert recovered.getPublic() != null : "Recovered key pair should contain public key";

    java.util.Arrays.fill(passphrase, '\0');
  }

  private static void shouldPersistExportableSoftwareKeys() throws Exception {
    final InMemoryKeyExportStore store = new InMemoryKeyExportStore();
    final KeyPassphraseProvider passphraseProvider = () -> "export-passphrase".toCharArray();
    final IrohaKeyManager keyManager =
        IrohaKeyManager.withExportableSoftwareKeys(store, passphraseProvider);
    final String alias = "exportable-alias";
    keyManager.generateOrLoad(alias, IrohaKeyManager.KeySecurityPreference.SOFTWARE_ONLY);
    assert store.load(alias).isPresent() : "Export store should persist key material";
  }
}
