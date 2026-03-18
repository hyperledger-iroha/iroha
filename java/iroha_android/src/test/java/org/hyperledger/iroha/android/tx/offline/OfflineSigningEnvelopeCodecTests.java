package org.hyperledger.iroha.android.tx.offline;

import java.util.Map;
import java.util.Optional;
import org.hyperledger.iroha.android.norito.NoritoException;

public final class OfflineSigningEnvelopeCodecTests {

  private OfflineSigningEnvelopeCodecTests() {}

  public static void main(final String[] args) throws Exception {
    roundTripWithExportedKey();
    roundTripWithoutExportedKey();
    System.out.println("[IrohaAndroid] Offline signing envelope codec tests passed.");
  }

  private static void roundTripWithExportedKey() throws NoritoException {
    final byte[] payload = new byte[] {0x01, 0x02, 0x03};
    final byte[] signature = new byte[] {0x04, 0x05};
    final byte[] publicKey = new byte[] {0x06, 0x07};
    final byte[] exportedKey = new byte[] {0x09, 0x08, 0x07};

    final OfflineSigningEnvelope envelope =
        OfflineSigningEnvelope.builder()
            .setEncodedPayload(payload)
            .setSignature(signature)
            .setPublicKey(publicKey)
            .setSchemaName("iroha.test.schema")
            .setKeyAlias("alias")
            .setIssuedAtMs(42L)
            .setMetadata(Map.of("note", "offline export"))
            .setExportedKeyBundle(exportedKey)
            .build();

    final OfflineSigningEnvelopeCodec codec = new OfflineSigningEnvelopeCodec();
    final byte[] encoded = codec.encode(envelope);
    final OfflineSigningEnvelope decoded = codec.decode(encoded);

    assert envelope.equals(decoded) : "Envelope should survive encode/decode round trip";
    assert decoded.exportedKeyBundle().isPresent() : "Exported key bundle should be present";
  }

  private static void roundTripWithoutExportedKey() throws NoritoException {
    final OfflineSigningEnvelope envelope =
        OfflineSigningEnvelope.builder()
            .setEncodedPayload(new byte[] {0x0A})
            .setSignature(new byte[] {0x0B})
            .setPublicKey(new byte[] {0x0C})
            .setKeyAlias("alias")
            .setSchemaName("schema")
            .setIssuedAtMs(1L)
            .build();

    final OfflineSigningEnvelopeCodec codec = new OfflineSigningEnvelopeCodec();
    final OfflineSigningEnvelope decoded = codec.decode(codec.encode(envelope));

    assert envelope.equals(decoded) : "Envelope should round trip without exported key";
    assert decoded.exportedKeyBundle().equals(Optional.empty())
        : "Exported key bundle should be empty";
  }
}
