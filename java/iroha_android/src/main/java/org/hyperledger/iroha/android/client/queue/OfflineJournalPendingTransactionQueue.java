package org.hyperledger.iroha.android.client.queue;

import java.io.IOException;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.offline.OfflineJournal;
import org.hyperledger.iroha.android.offline.OfflineJournalEntry;
import org.hyperledger.iroha.android.offline.OfflineJournalException;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;
import org.hyperledger.iroha.android.tx.offline.OfflineSigningEnvelope;
import org.hyperledger.iroha.android.tx.offline.OfflineSigningEnvelopeCodec;

/**
 * Pending queue backed by {@link OfflineJournal}. Entries are stored as Norito-encoded
 * {@link OfflineSigningEnvelope} blobs authenticated via BLAKE2b + HMAC-SHA256, matching the
 * Rust/Swift WAL layout.
 */
public final class OfflineJournalPendingTransactionQueue implements PendingTransactionQueue {

  private static final OfflineSigningEnvelopeCodec ENVELOPE_CODEC =
      new OfflineSigningEnvelopeCodec();

  private final OfflineJournal journal;

  public OfflineJournalPendingTransactionQueue(final OfflineJournal journal) {
    this.journal = Objects.requireNonNull(journal, "journal");
  }

  @Override
  public void enqueue(final SignedTransaction transaction) throws IOException {
    Objects.requireNonNull(transaction, "transaction");
    final OfflineSigningEnvelope envelope = buildEnvelope(transaction);
    final byte[] payload;
    try {
      payload = ENVELOPE_CODEC.encode(envelope);
    } catch (final NoritoException ex) {
      throw new IOException("Failed to encode offline signing envelope", ex);
    }
    try {
      journal.appendPending(SignedTransactionHasher.hash(transaction), payload);
    } catch (final OfflineJournalException ex) {
      throw new IOException("Failed to append offline journal entry", ex);
    }
  }

  @Override
  public List<SignedTransaction> drain() throws IOException {
    final List<OfflineJournalEntry> entries = journal.pendingEntries();
    final List<SignedTransaction> transactions = new ArrayList<>(entries.size());
    for (final OfflineJournalEntry entry : entries) {
      final SignedTransaction transaction = decodeEntry(entry.payload());
      transactions.add(transaction);
      try {
        journal.markCommitted(entry.txId());
      } catch (final OfflineJournalException ex) {
        throw new IOException("Failed to mark offline journal entry committed", ex);
      }
    }
    return transactions;
  }

  @Override
  public int size() throws IOException {
    return journal.pendingEntries().size();
  }

  @Override
  public String telemetryQueueName() {
    return "offline_journal";
  }

  private static OfflineSigningEnvelope buildEnvelope(final SignedTransaction transaction) {
    return OfflineSigningEnvelope.builder()
        .setEncodedPayload(transaction.encodedPayload())
        .setSignature(transaction.signature())
        .setPublicKey(transaction.publicKey())
        .setSchemaName(transaction.schemaName())
        .setKeyAlias(transaction.keyAlias().orElse("pending.queue"))
        .setIssuedAtMs(System.currentTimeMillis())
        .setExportedKeyBundle(transaction.exportedKeyBundle().orElse(null))
        .build();
  }

  private static SignedTransaction decodeEntry(final byte[] payload) throws IOException {
    try {
      final OfflineSigningEnvelope envelope = ENVELOPE_CODEC.decode(payload);
      return new SignedTransaction(
          envelope.encodedPayload(),
          envelope.signature(),
          envelope.publicKey(),
          envelope.schemaName(),
          envelope.keyAlias(),
          envelope.exportedKeyBundle().orElse(null));
    } catch (final NoritoException ex) {
      throw new IOException("Failed to decode offline signing envelope", ex);
    }
  }

}
