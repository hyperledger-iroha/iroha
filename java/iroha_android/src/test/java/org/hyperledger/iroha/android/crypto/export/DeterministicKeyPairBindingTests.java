package org.hyperledger.iroha.android.crypto.export;

import static org.junit.Assert.fail;

import java.security.KeyPair;
import java.util.Arrays;
import org.hyperledger.iroha.android.crypto.SoftwareKeyProvider;
import org.junit.Test;

/** Regression tests for binding Ed25519 public keys to encrypted private-key exports. */
public final class DeterministicKeyPairBindingTests {

  @Test
  public void importRejectsSubstitutedEd25519PublicKey() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final KeyPair original = provider.generate("original-alias");
    final KeyPair attacker = provider.generate("attacker-alias");
    final char[] passphrase = "authenticated-key-pair".toCharArray();
    try {
      final KeyExportBundle bundle =
          DeterministicKeyExporter.exportKeyPair(
              original.getPrivate(), original.getPublic(), "restored-account", passphrase);
      final KeyExportBundle tampered =
          new KeyExportBundle(
              bundle.alias(),
              bundle.algorithmCode(),
              attacker.getPublic().getEncoded(),
              bundle.nonce(),
              bundle.ciphertext(),
              bundle.salt(),
              bundle.kdfKind(),
              bundle.kdfWorkFactor(),
              bundle.version());

      try {
        DeterministicKeyExporter.importKeyPair(tampered, passphrase);
        fail("Import must reject a substituted Ed25519 public key");
      } catch (final KeyExportException expected) {
        // Expected: the decrypted private key derives a different public key.
      }
    } finally {
      Arrays.fill(passphrase, '\0');
    }
  }

  @Test
  public void exportRejectsInconsistentEd25519KeyPair() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final KeyPair privatePair = provider.generate("private-alias");
    final KeyPair publicPair = provider.generate("public-alias");
    final char[] passphrase = "authenticated-key-pair".toCharArray();
    try {
      DeterministicKeyExporter.exportKeyPair(
          privatePair.getPrivate(),
          publicPair.getPublic(),
          "inconsistent-account",
          passphrase);
      fail("Export must reject an inconsistent Ed25519 key pair");
    } catch (final KeyExportException expected) {
      // Expected: callers cannot create a self-inconsistent recovery bundle.
    } finally {
      Arrays.fill(passphrase, '\0');
    }
  }
}
