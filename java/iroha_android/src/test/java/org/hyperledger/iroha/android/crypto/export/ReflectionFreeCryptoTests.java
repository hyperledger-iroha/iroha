package org.hyperledger.iroha.android.crypto.export;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertTrue;

import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.Signature;
import java.util.Arrays;
import org.hyperledger.iroha.android.crypto.SoftwareKeyProvider;
import org.hyperledger.iroha.android.crypto.SoftwareKeyProvider.ProviderPolicy;
import org.junit.Test;

/** Direct-link regression tests for the Java software-key crypto path. */
public final class ReflectionFreeCryptoTests {

  @Test
  public void directArgon2ExporterRoundTripsBouncyCastleKey() throws Exception {
    final KeyPair keyPair =
        new SoftwareKeyProvider(ProviderPolicy.BOUNCY_CASTLE_REQUIRED).generateEphemeral();
    final char[] passphrase = "reflection-free-export".toCharArray();
    try {
      final KeyExportBundle bundle =
          DeterministicKeyExporter.exportKeyPair(
              keyPair.getPrivate(), keyPair.getPublic(), "release-test", passphrase);
      final DeterministicKeyExporter.KeyPairData restored =
          DeterministicKeyExporter.importKeyPair(bundle, passphrase);

      assertArrayEquals(keyPair.getPublic().getEncoded(), restored.publicKey().getEncoded());
      final byte[] payload = "argon2-round-trip".getBytes(StandardCharsets.UTF_8);
      final Signature signer = Signature.getInstance("Ed25519", "BC");
      signer.initSign(restored.privateKey());
      signer.update(payload);
      final Signature verifier = Signature.getInstance("Ed25519", "BC");
      verifier.initVerify(keyPair.getPublic());
      verifier.update(payload);
      assertTrue(verifier.verify(signer.sign()));
    } finally {
      Arrays.fill(passphrase, '\0');
    }
  }
}
