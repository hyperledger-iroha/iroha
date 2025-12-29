package org.hyperledger.iroha.android.tx;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Base64;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;

/**
 * Utility executable that regenerates `.norito` binaries and base64 encodings for the transaction
 * payload fixtures. Run via `javac/java` similarly to `TransactionPayloadFixtureTests`.
 */
public final class TransactionPayloadFixtureExporter {

  private TransactionPayloadFixtureExporter() {}

  public static void main(final String[] args) throws Exception {
    Path path = Path.of("java/iroha_android/src/test/resources/transaction_payloads.json");
    if (!Files.exists(path)) {
      path = Path.of("src/test/resources/transaction_payloads.json");
    }
    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    for (TransactionPayloadFixtures.Fixture fixture : TransactionPayloadFixtures.load(path)) {
      final TransactionPayload payload = fixture.toPayload();
      final byte[] encoded = adapter.encodeTransaction(payload);
      final String base64 = Base64.getEncoder().encodeToString(encoded);
      Files.write(Path.of("java/iroha_android/src/test/resources/" + fixture.name() + ".norito"), encoded);
      System.out.println(fixture.name() + "=" + base64);
    }
  }
}
