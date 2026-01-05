package org.hyperledger.iroha.android.connect;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

public final class ConnectQueueJournalTests {

  private ConnectQueueJournalTests() {}

  public static void main(final String[] args) throws Exception {
    appendAndReadRoundTrip();
    pruneExpiredRecords();
    popOldestRemovesEntries();
    enforceSizeLimits();
    decodeAcceptsPadding();
    System.out.println("[IrohaAndroid] Connect queue journal tests passed.");
  }

  private static void appendAndReadRoundTrip() throws Exception {
    final Path root = Files.createTempDirectory("connect-journal-roundtrip");
    final ConnectQueueJournal journal =
        new ConnectQueueJournal(
            "session-rt".getBytes(),
            new ConnectQueueJournal.Configuration(root, 8, 1 << 20, 60_000L));
    journal.append(
        ConnectDirection.APP_TO_WALLET, 1L, "ciphertext-1".getBytes(), 100L, 10_000L);
    journal.append(
        ConnectDirection.APP_TO_WALLET, 2L, "ciphertext-2".getBytes(), 200L, 10_000L);

    final List<ConnectJournalRecord> records =
        journal.records(ConnectDirection.APP_TO_WALLET, 500L);
    assert records.size() == 2 : "expected two records";
    assert records.get(0).sequence() == 1L : "first record sequence mismatch";
    assert records.get(1).sequence() == 2L : "second record sequence mismatch";
  }

  private static void pruneExpiredRecords() throws Exception {
    final Path root = Files.createTempDirectory("connect-journal-expiry");
    final ConnectQueueJournal journal =
        new ConnectQueueJournal(
            "session-expire".getBytes(),
            new ConnectQueueJournal.Configuration(root, 8, 1 << 20, 60_000L));
    journal.append(
        ConnectDirection.WALLET_TO_APP, 1L, "ciphertext-expire".getBytes(), 100L, 1L);
    final List<ConnectJournalRecord> before =
        journal.records(ConnectDirection.WALLET_TO_APP, 100L);
    assert before.size() == 1 : "record should exist before expiry";
    final List<ConnectJournalRecord> after =
        journal.records(ConnectDirection.WALLET_TO_APP, 200L);
    assert after.isEmpty() : "record should be pruned after expiry";
  }

  private static void popOldestRemovesEntries() throws Exception {
    final Path root = Files.createTempDirectory("connect-journal-pop");
    final ConnectQueueJournal journal =
        new ConnectQueueJournal(
            "session-pop".getBytes(),
            new ConnectQueueJournal.Configuration(root, 8, 1 << 20, 60_000L));
    for (int i = 0; i < 3; i++) {
      final byte[] payload = ("payload-" + i).getBytes();
      journal.append(ConnectDirection.APP_TO_WALLET, i + 1L, payload, 100L + i, 10_000L);
    }

    final List<ConnectJournalRecord> removed =
        journal.popOldest(ConnectDirection.APP_TO_WALLET, 2, 200L);
    assert removed.size() == 2 : "expected two removed entries";
    assert removed.get(0).sequence() == 1L : "first removed sequence mismatch";
    assert removed.get(1).sequence() == 2L : "second removed sequence mismatch";
    final List<ConnectJournalRecord> remaining =
        journal.records(ConnectDirection.APP_TO_WALLET, 200L);
    assert remaining.size() == 1 : "expected one remaining entry";
    assert remaining.get(0).sequence() == 3L : "remaining sequence mismatch";
  }

  private static void enforceSizeLimits() throws Exception {
    final Path root = Files.createTempDirectory("connect-journal-limits");
    final ConnectQueueJournal journal =
        new ConnectQueueJournal(
            "session-limit".getBytes(),
            new ConnectQueueJournal.Configuration(root, 2, 256, 60_000L));
    journal.append(ConnectDirection.WALLET_TO_APP, 1L, "a".getBytes(), 100L, 10_000L);
    journal.append(ConnectDirection.WALLET_TO_APP, 2L, "b".getBytes(), 101L, 10_000L);
    journal.append(ConnectDirection.WALLET_TO_APP, 3L, "c".getBytes(), 102L, 10_000L);

    final List<ConnectJournalRecord> records =
        journal.records(ConnectDirection.WALLET_TO_APP, 150L);
    assert records.size() == 2 : "limits should prune to two records";
    assert records.get(0).sequence() == 2L : "oldest record should be dropped";
    assert records.get(1).sequence() == 3L : "latest record missing";
  }

  private static void decodeAcceptsPadding() throws Exception {
    final ConnectJournalRecord record =
        new ConnectJournalRecord(
            ConnectDirection.APP_TO_WALLET,
            9L,
            ConnectJournalRecord.computePayloadHash("pad".getBytes()),
            "pad".getBytes(),
            100L,
            200L);
    final byte[] encoded = record.encode();
    final int headerLen = org.hyperledger.iroha.norito.NoritoHeader.HEADER_LENGTH;
    final int payloadLen = encoded.length - headerLen;
    final int paddingLen = 8;
    final byte[] padded = new byte[encoded.length + paddingLen];
    System.arraycopy(encoded, 0, padded, 0, headerLen);
    System.arraycopy(encoded, headerLen, padded, headerLen + paddingLen, payloadLen);
    final ConnectJournalRecord.DecodeResult result = ConnectJournalRecord.decode(padded, 0);
    assert result.bytesConsumed() == padded.length : "padding should count toward bytes consumed";
    assert result.record().sequence() == record.sequence() : "record mismatch after padding";
  }
}
