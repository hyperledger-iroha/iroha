package org.hyperledger.iroha.sdk.client.queue

import java.io.IOException
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardOpenOption
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.norito.NoritoException

/**
 * File-backed queue that persists transactions as Base64-encoded records separated by newlines.
 *
 * Each line contains a Base64-encoded canonical pending-transaction record.
 *
 * The queue preserves insertion order and deletes the underlying file when drained.
 */
@OptIn(ExperimentalEncodingApi::class)
class FilePendingTransactionQueue @Throws(IOException::class) constructor(
    private val queueFile: Path
) : PendingTransactionQueue {

    private val lock = Any()

    init {
        val parent = queueFile.parent
        if (parent != null) {
            Files.createDirectories(parent)
        }
        if (!Files.exists(queueFile)) {
            try {
                Files.createFile(queueFile)
            } catch (_: java.nio.file.FileAlreadyExistsException) {
            }
        }
    }

    @Throws(IOException::class)
    override fun enqueue(transaction: SignedTransaction) {
        val line: String
        try {
            line = Base64.encode(PendingTransactionRecordCodec.encode(transaction))
        } catch (ex: NoritoException) {
            throw IOException("Failed to encode pending transaction record", ex)
        }
        synchronized(lock) {
            Files.write(
                queueFile,
                (line + System.lineSeparator()).toByteArray(StandardCharsets.UTF_8),
                StandardOpenOption.CREATE,
                StandardOpenOption.APPEND
            )
        }
    }

    @Throws(IOException::class)
    override fun drain(): List<SignedTransaction> {
        synchronized(lock) {
            if (!Files.exists(queueFile)) {
                return emptyList()
            }
            val lines = Files.readAllLines(queueFile, StandardCharsets.UTF_8)
            val transactions = ArrayList<SignedTransaction>(lines.size)
            for (line in lines) {
                if (line.trim().isEmpty()) continue
                transactions.add(decodeEntry(line))
            }
            Files.write(queueFile, ByteArray(0), StandardOpenOption.TRUNCATE_EXISTING)
            return transactions
        }
    }

    @Throws(IOException::class)
    override fun size(): Int {
        synchronized(lock) {
            if (!Files.exists(queueFile)) return 0
            var count = 0
            for (line in Files.readAllLines(queueFile, StandardCharsets.UTF_8)) {
                if (line.trim().isNotEmpty()) count++
            }
            return count
        }
    }

    /** Removes all queued transactions without returning them. Primarily useful for tests. */
    @Throws(IOException::class)
    fun clear() {
        synchronized(lock) {
            if (Files.exists(queueFile)) {
                Files.write(queueFile, ByteArray(0), StandardOpenOption.TRUNCATE_EXISTING)
            }
        }
    }

    override fun telemetryQueueName(): String = "file"

    @Throws(IOException::class)
    private fun decodeEntry(line: String): SignedTransaction {
        val envelopeBytes: ByteArray
        try {
            envelopeBytes = Base64.decode(line)
        } catch (ex: IllegalArgumentException) {
            throw IOException("Failed to decode queue entry", ex)
        }
        try {
            return PendingTransactionRecordCodec.decode(envelopeBytes)
        } catch (ex: NoritoException) {
            throw IOException("Failed to decode queue entry", ex)
        }
    }

}
