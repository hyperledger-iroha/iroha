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
    final Path path = TransactionPayloadFixtures.resolveFixturePath();
    final Path outputDir = path.getParent();
    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    for (TransactionPayloadFixtures.Fixture fixture : TransactionPayloadFixtures.load(path)) {
      if (fixture.encoded().isPresent()) {
        final String base64 = fixture.encoded().get();
        try {
          final byte[] encoded = Base64.getDecoder().decode(base64);
          if (outputDir != null) {
            Files.write(outputDir.resolve(fixture.name() + ".norito"), encoded);
          }
          System.out.println(fixture.name() + "=" + base64);
        } catch (final IllegalArgumentException ex) {
          System.err.println("[fixture] " + fixture.name() + " skipped (invalid base64 payload)");
        }
        continue;
      }
      if (!fixture.isDecodable()) {
        final String name = fixture.name();
        System.out.println("[fixture] " + name + " skipped (payload not decodable)");
        continue;
      }
      final TransactionPayload payload = fixture.toPayload();
      final byte[] encoded = adapter.encodeTransaction(payload);
      final String base64 = Base64.getEncoder().encodeToString(encoded);
      if (outputDir != null) {
        Files.write(outputDir.resolve(fixture.name() + ".norito"), encoded);
      }
      System.out.println(fixture.name() + "=" + base64);
    }
  }
}
