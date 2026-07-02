package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

public final class OfflineJournalTest {

  private OfflineJournalTest() {}

  public static void main(final String[] args) throws Exception {
    appendAndCommitCycle();
    pendingEntriesSortTxIdsUnsigned();
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

  private static void pendingEntriesSortTxIdsUnsigned() throws Exception {
    final Path file = Files.createTempFile("offline_journal_sort", ".bin");
    try {
      final OfflineJournalKey key =
          OfflineJournalKey.derive("sort-seed".getBytes(StandardCharsets.UTF_8));
      final byte[] smallest = txIdWithFirstByte(0x00);
      final byte[] middle = txIdWithFirstByte(0x80);
      final byte[] largest = txIdWithFirstByte(0xFF);
      try (OfflineJournal journal = new OfflineJournal(file, key)) {
        journal.appendPending(largest, new byte[] {0x03});
        journal.appendPending(smallest, new byte[] {0x01});
        journal.appendPending(middle, new byte[] {0x02});

        final java.util.List<OfflineJournalEntry> pending = journal.pendingEntries();
        assert Arrays.equals(smallest, pending.get(0).txId()) : "smallest tx_id order mismatch";
        assert Arrays.equals(middle, pending.get(1).txId()) : "middle tx_id order mismatch";
        assert Arrays.equals(largest, pending.get(2).txId()) : "largest tx_id order mismatch";
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

  private static byte[] txIdWithFirstByte(final int firstByte) {
    final byte[] txId = new byte[32];
    txId[0] = (byte) firstByte;
    return txId;
  }
}
