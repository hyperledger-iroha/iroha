package org.hyperledger.iroha.android.tools;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Base64;
import java.util.List;
import org.hyperledger.iroha.android.tools.PendingQueueInspector.EntrySummary;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;
import org.hyperledger.iroha.android.tx.offline.OfflineSigningEnvelope;
import org.hyperledger.iroha.android.tx.offline.OfflineSigningEnvelopeCodec;

public final class PendingQueueInspectorTests {

  private static final OfflineSigningEnvelopeCodec ENVELOPE_CODEC =
      new OfflineSigningEnvelopeCodec();

  private PendingQueueInspectorTests() {}

  public static void main(final String[] args) throws Exception {
    inspectModernEntries();
    System.out.println("[IrohaAndroid] Pending queue inspector tests passed.");
  }

  private static void inspectModernEntries() throws Exception {
    final Path queue = Files.createTempFile("pending-queue-modern-", ".txt");
    final long issuedAt = 1_706_000_000_000L;
    final byte[] signature = new byte[] {0x03};
    final byte[] publicKey = new byte[] {0x04};
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000002")
            .setAuthority("inspector@wonderland")
            .setCreationTimeMs(issuedAt)
            .setInstructionBytes(new byte[] {0x01, 0x02})
            .setTimeToLiveMs(5_000L)
            .setNonce(2)
            .setMetadata(java.util.Map.of("queue", "modern"))
            .build();
    final NoritoJavaCodecAdapter codec = new NoritoJavaCodecAdapter();
    final byte[] encodedPayload;
    try {
      encodedPayload = codec.encodeTransaction(payload);
    } catch (final Exception ex) {
      throw new IllegalStateException("Failed to encode transaction payload", ex);
    }

    final OfflineSigningEnvelope envelope =
        OfflineSigningEnvelope.builder()
            .setEncodedPayload(encodedPayload)
            .setSignature(signature)
            .setPublicKey(publicKey)
            .setSchemaName(codec.schemaName())
            .setKeyAlias("pixel-strongbox")
            .setIssuedAtMs(issuedAt)
            .build();

    final String encoded = Base64.getEncoder().encodeToString(ENVELOPE_CODEC.encode(envelope));
    Files.writeString(queue, encoded + System.lineSeparator(), StandardCharsets.UTF_8);

    final List<EntrySummary> summaries = PendingQueueInspector.inspect(queue);
    assert summaries.size() == 1 : "Expected one entry";
    final EntrySummary summary = summaries.get(0);
    assert summary.index() == 0;
    assert summary.schemaName().equals("iroha.android.transaction.Payload.v1");
    assert summary.keyAlias().equals("pixel-strongbox");
    assert summary.issuedAtMs().isPresent();
    assert summary.issuedAtMs().getAsLong() == issuedAt;
    final String expectedHash =
        SignedTransactionHasher.hashHex(envelope.toSignedTransaction());
    assert summary.hashHex().equals(expectedHash);
  }

}
