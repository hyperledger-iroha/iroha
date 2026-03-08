package org.hyperledger.iroha.android.client.queue;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.hyperledger.iroha.android.offline.OfflineJournal;
import org.hyperledger.iroha.android.offline.OfflineJournalKey;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.offline.OfflineSigningEnvelope;

public final class OfflineJournalPendingTransactionQueueTest {

  private OfflineJournalPendingTransactionQueueTest() {}

  public static void main(final String[] args) throws Exception {
    enqueuesAndDrainsEntries();
    rewritesJournalAfterDrain();
    System.out.println("[IrohaAndroid] OfflineJournalPendingTransactionQueueTest passed.");
  }

  private static void enqueuesAndDrainsEntries() throws Exception {
    final Path file = Files.createTempFile("offline_journal_queue", ".bin");
    try {
      final OfflineJournalKey key =
          OfflineJournalKey.derive("queue-seed".getBytes(StandardCharsets.UTF_8));
      try (OfflineJournal journal = new OfflineJournal(file, key)) {
        final OfflineJournalPendingTransactionQueue queue =
            new OfflineJournalPendingTransactionQueue(journal);
        final SignedTransaction signed = TestTransactions.sampleSignedTransaction();
        queue.enqueue(signed);
        assert queue.size() == 1 : "queue size mismatch";
        final List<SignedTransaction> drained = queue.drain();
        assert drained.size() == 1 : "drain size mismatch";
        assert TestTransactions.equalsByHash(signed, drained.get(0)) : "transaction mismatch";
        assert queue.size() == 0 : "queue should be empty after drain";
      }
    } finally {
      Files.deleteIfExists(file);
    }
  }

  private static void rewritesJournalAfterDrain() throws Exception {
    final Path file = Files.createTempFile("offline_journal_queue_reopen", ".bin");
    try {
      final OfflineJournalKey key =
          OfflineJournalKey.derive("queue-reopen".getBytes(StandardCharsets.UTF_8));
      final SignedTransaction signed = TestTransactions.sampleSignedTransaction();
      try (OfflineJournal journal = new OfflineJournal(file, key)) {
        final OfflineJournalPendingTransactionQueue queue =
            new OfflineJournalPendingTransactionQueue(journal);
        queue.enqueue(signed);
        queue.drain();
      }
      try (OfflineJournal journal = new OfflineJournal(file, key)) {
        final OfflineJournalPendingTransactionQueue queue =
            new OfflineJournalPendingTransactionQueue(journal);
        assert queue.size() == 0 : "journal should reopen without pending entries";
      }
    } finally {
      Files.deleteIfExists(file);
    }
  }

  /** Minimal helper for constructing deterministic signed transactions. */
  private static final class TestTransactions {
    private static final byte[] SIGNATURE = new byte[64];
    private static final byte[] PUBLIC_KEY = new byte[32];

    static {
      SIGNATURE[0] = 1;
      PUBLIC_KEY[0] = 2;
    }

    static SignedTransaction sampleSignedTransaction() {
      final TransactionPayload payload =
          TransactionPayload.builder()
              .setChainId("00000001")
              .setAuthority("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp")
              .setCreationTimeMs(1_700_000_000_000L)
              .setInstructionBytes("payload".getBytes(StandardCharsets.UTF_8))
              .setTimeToLiveMs(5_000L)
              .setNonce(1)
              .setMetadata(java.util.Map.of("test", "true"))
              .build();
      final NoritoJavaCodecAdapter codec = new NoritoJavaCodecAdapter();
      final byte[] encoded;
      try {
        encoded = codec.encodeTransaction(payload);
      } catch (final Exception ex) {
        throw new IllegalStateException("Failed to encode transaction payload", ex);
      }
      final OfflineSigningEnvelope envelope =
          OfflineSigningEnvelope.builder()
              .setEncodedPayload(encoded)
              .setSignature(SIGNATURE)
              .setPublicKey(PUBLIC_KEY)
              .setSchemaName(codec.schemaName())
              .setKeyAlias("test")
              .setIssuedAtMs(1_700_000_000_000L)
              .setMetadata(java.util.Map.of("test", "true"))
              .build();
      return envelope.toSignedTransaction();
    }

    static boolean equalsByHash(final SignedTransaction a, final SignedTransaction b) {
      return java.util.Arrays.equals(
          org.hyperledger.iroha.android.tx.SignedTransactionHasher.hash(a),
          org.hyperledger.iroha.android.tx.SignedTransactionHasher.hash(b));
    }
  }
}
