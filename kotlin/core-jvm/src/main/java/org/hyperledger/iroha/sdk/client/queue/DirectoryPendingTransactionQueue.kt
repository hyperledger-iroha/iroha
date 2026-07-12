package org.hyperledger.iroha.sdk.client.queue

import java.io.IOException
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardOpenOption
import java.util.regex.Pattern
import java.util.stream.Collectors
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.norito.NoritoException

/**
 * Directory-backed pending queue that persists each transaction as a discrete envelope file.
 *
 * This variant is intended for OEM/governed environments that want a stable on-disk layout
 * (one file per queued transaction) rather than a single append-only log. Entries survive process
 * restarts and preserve insertion order via monotonically increasing file names.
 */
class DirectoryPendingTransactionQueue @Throws(IOException::class) constructor(
    private val rootDir: Path
) : PendingTransactionQueue {

    private val lock = Any()
    private var nextId: Long

    init {
        Files.createDirectories(rootDir)
        nextId = initialiseNextId(rootDir)
    }

    @Throws(IOException::class)
    override fun enqueue(transaction: SignedTransaction) {
        val encoded: ByteArray
        try {
            encoded = PendingTransactionRecordCodec.encode(transaction)
        } catch (ex: NoritoException) {
            throw IOException("Failed to encode pending transaction record", ex)
        }
        synchronized(lock) {
            val path = rootDir.resolve(formatEntryName(nextId++))
            Files.write(path, encoded, StandardOpenOption.CREATE_NEW, StandardOpenOption.WRITE)
        }
    }

    @Throws(IOException::class)
    override fun drain(): List<SignedTransaction> {
        val entries = listEntriesInOrder()
        val transactions = ArrayList<SignedTransaction>(entries.size)
        for (entry in entries) {
            val payload = Files.readAllBytes(entry)
            transactions.add(decodeEnvelope(payload))
            Files.deleteIfExists(entry)
        }
        return transactions
    }

    @Throws(IOException::class)
    override fun size(): Int = listEntriesInOrder().size

    override fun telemetryQueueName(): String = "oem_directory"

    @Throws(IOException::class)
    private fun listEntriesInOrder(): List<Path> {
        val entries = ArrayList<Path>()
        synchronized(lock) {
            Files.list(rootDir).use { stream ->
                stream
                    .filter { path -> ENTRY_PATTERN.matcher(path.fileName.toString()).matches() }
                    .sorted(Comparator.comparingLong { path: Path -> extractId(path) })
                    .forEach { entries.add(it) }
            }
        }
        return entries
    }

    companion object {
        private val ENTRY_PATTERN = Pattern.compile("^pending-(\\d+)\\.bin$")

        @Throws(IOException::class)
        private fun initialiseNextId(rootDir: Path): Long {
            var highest = -1L
            Files.list(rootDir).use { stream ->
                for (entry in stream.collect(Collectors.toList())) {
                    val matcher = ENTRY_PATTERN.matcher(entry.fileName.toString())
                    if (matcher.matches()) {
                        try {
                            val id = matcher.group(1).toLong()
                            highest = maxOf(highest, id)
                        } catch (_: NumberFormatException) {
                        }
                    }
                }
            }
            return highest + 1L
        }

        private fun extractId(path: Path): Long {
            val matcher = ENTRY_PATTERN.matcher(path.fileName.toString())
            if (!matcher.matches()) return Long.MAX_VALUE
            return try {
                matcher.group(1).toLong()
            } catch (_: NumberFormatException) {
                Long.MAX_VALUE
            }
        }

        @Throws(IOException::class)
        private fun decodeEnvelope(payload: ByteArray): SignedTransaction {
            try {
                return PendingTransactionRecordCodec.decode(payload)
            } catch (ex: NoritoException) {
                throw IOException("Failed to decode pending transaction record", ex)
            }
        }

        private fun formatEntryName(id: Long): String =
            String.format("pending-%020d.bin", id)
    }
}
