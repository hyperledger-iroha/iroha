package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

public final class OfflineJournalTest {

  private OfflineJournalTest() {}

  public static void main(final String[] args) throws Exception {
    appendAndCommitCycle();
    duplicatePendingRejected();
    markMissingEntryFails();
    System.out.println("[IrohaAndroid] OfflineJournalTest passed.");
  }

  private static void appendAndCommitCycle() throws Exception {
    final Path file = Files.createTempFile("offline_journal", ".bin");
    try {
      final OfflineJournalKey key =
          OfflineJournalKey.derive("seed".getBytes(StandardCharsets.UTF_8));
      final byte[] txId = new byte[32];
      Arrays.fill(txId, (byte) 0xAB);
      final byte[] payload = new byte[] {0x01, 0x02, 0x03};
      try (OfflineJournal journal = new OfflineJournal(file, key)) {
        final OfflineJournalEntry entry =
            journal.appendPending(txId, payload, 1_700_000_000_000L);
        assert Arrays.equals(txId, entry.txId()) : "txId mismatch";
        assert Arrays.equals(payload, entry.payload()) : "payload mismatch";
        assert entry.hashChain().length == 32 : "hash chain length mismatch";
        assert journal.pendingEntries().size() == 1 : "pending size mismatch";
        journal.markCommitted(txId, 1_700_000_000_500L);
        assert journal.pendingEntries().isEmpty() : "pending should be empty after commit";
      }

      // Re-open to ensure persisted journal can be read without integrity errors.
      try (OfflineJournal reopened = new OfflineJournal(file, key)) {
        assert reopened.pendingEntries().isEmpty() : "reopened journal should have no pending";
      }
    } finally {
      Files.deleteIfExists(file);
    }
  }

  private static void duplicatePendingRejected() throws Exception {
    final Path file = Files.createTempFile("offline_journal_dup", ".bin");
    try {
      final OfflineJournalKey key =
          OfflineJournalKey.derive("dup-seed".getBytes(StandardCharsets.UTF_8));
      final byte[] txId = new byte[32];
      Arrays.fill(txId, (byte) 0xCD);
      try (OfflineJournal journal = new OfflineJournal(file, key)) {
        journal.appendPending(txId, new byte[] {0x01});
        boolean threw = false;
        try {
          journal.appendPending(txId, new byte[] {0x02});
        } catch (final OfflineJournalException ex) {
          threw = ex.reason() == OfflineJournalException.Reason.DUPLICATE_PENDING;
        }
        assert threw : "expected duplicate pending error";
      }
    } finally {
      Files.deleteIfExists(file);
    }
  }

  private static void markMissingEntryFails() throws Exception {
    final Path file = Files.createTempFile("offline_journal_missing", ".bin");
    try {
      final OfflineJournalKey key =
          OfflineJournalKey.derive("missing".getBytes(StandardCharsets.UTF_8));
      final byte[] txId = new byte[32];
      Arrays.fill(txId, (byte) 0xEF);
      try (OfflineJournal journal = new OfflineJournal(file, key)) {
        boolean threw = false;
        try {
          journal.markCommitted(txId);
        } catch (final OfflineJournalException ex) {
          threw = ex.reason() == OfflineJournalException.Reason.NOT_PENDING;
        }
        assert threw : "expected not pending error";
      }
    } finally {
      Files.deleteIfExists(file);
    }
  }
}
