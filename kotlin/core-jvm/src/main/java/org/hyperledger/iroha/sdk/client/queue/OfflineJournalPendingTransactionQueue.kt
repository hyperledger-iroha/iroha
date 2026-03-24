package org.hyperledger.iroha.sdk.client.queue

import java.io.IOException
import org.hyperledger.iroha.sdk.offline.OfflineJournal
import org.hyperledger.iroha.sdk.offline.OfflineJournalException
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher
import org.hyperledger.iroha.sdk.tx.norito.NoritoException
import org.hyperledger.iroha.sdk.tx.offline.OfflineSigningEnvelope
import org.hyperledger.iroha.sdk.tx.offline.OfflineSigningEnvelopeCodec

/**
 * Pending queue backed by [OfflineJournal]. Entries are stored as Norito-encoded
 * [OfflineSigningEnvelope] blobs authenticated via BLAKE2b + HMAC-SHA256, matching the
 * Rust/Swift WAL layout.
 */
class OfflineJournalPendingTransactionQueue(
    private val journal: OfflineJournal
) : PendingTransactionQueue {

    @Throws(IOException::class)
    override fun enqueue(transaction: SignedTransaction) {
        val envelope = buildEnvelope(transaction)
        val payload: ByteArray
        try {
            payload = ENVELOPE_CODEC.encode(envelope)
        } catch (ex: NoritoException) {
            throw IOException("Failed to encode offline signing envelope", ex)
        }
        try {
            journal.appendPending(SignedTransactionHasher.hash(transaction), payload)
        } catch (ex: OfflineJournalException) {
            throw IOException("Failed to append offline journal entry", ex)
        }
    }

    @Throws(IOException::class)
    override fun drain(): List<SignedTransaction> {
        val entries = journal.pendingEntries()
        val transactions = ArrayList<SignedTransaction>(entries.size)
        for (entry in entries) {
            val transaction = decodeEntry(entry.payload)
            transactions.add(transaction)
            try {
                journal.markCommitted(entry.txId)
            } catch (ex: OfflineJournalException) {
                throw IOException("Failed to mark offline journal entry committed", ex)
            }
        }
        return transactions
    }

    @Throws(IOException::class)
    override fun size(): Int = journal.pendingEntries().size

    override fun telemetryQueueName(): String = "offline_journal"

    companion object {
        private val ENVELOPE_CODEC = OfflineSigningEnvelopeCodec()

        private fun buildEnvelope(transaction: SignedTransaction): OfflineSigningEnvelope =
            OfflineSigningEnvelope(
                encodedPayload = transaction.encodedPayload(),
                signature = transaction.signature(),
                publicKey = transaction.publicKey(),
                schemaName = transaction.schemaName(),
                keyAlias = transaction.keyAlias().orElse("pending.queue"),
                issuedAtMs = System.currentTimeMillis(),
                exportedKeyBundle = transaction.exportedKeyBundle().orElse(null),
            )

        @Throws(IOException::class)
        private fun decodeEntry(payload: ByteArray): SignedTransaction {
            try {
                val envelope = ENVELOPE_CODEC.decode(payload)
                return SignedTransaction(
                    envelope.encodedPayload,
                    envelope.signature,
                    envelope.publicKey,
                    envelope.schemaName,
                    envelope.keyAlias,
                    envelope.exportedKeyBundle
                )
            } catch (ex: NoritoException) {
                throw IOException("Failed to decode offline signing envelope", ex)
            }
        }
    }
}
