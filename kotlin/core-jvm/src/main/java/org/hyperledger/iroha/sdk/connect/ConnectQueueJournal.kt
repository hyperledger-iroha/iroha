package org.hyperledger.iroha.sdk.connect

import java.io.ByteArrayOutputStream
import java.io.IOException
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardCopyOption
import java.nio.file.StandardOpenOption
import java.security.MessageDigest
import java.security.NoSuchAlgorithmException
import java.util.concurrent.locks.ReentrantLock
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

/** Append-only Connect queue journal mirrored across SDKs (IOS-CONNECT-003). */
class ConnectQueueJournal @Throws(ConnectJournalException::class) constructor(
    sessionId: ByteArray,
    private val configuration: JournalConfiguration = JournalConfiguration(),
) {
    private val _sessionId: ByteArray = sessionId.copyOf()
    private val storage = ConnectQueueStorage(_sessionId, configuration.rootDirectory)
    private val appQueue = ConnectJournalFile(
        ConnectDirection.APP_TO_WALLET, storage.appQueuePath, configuration,
    )
    private val walletQueue = ConnectJournalFile(
        ConnectDirection.WALLET_TO_APP, storage.walletQueuePath, configuration,
    )

    @Throws(ConnectJournalException::class)
    fun append(direction: ConnectDirection, sequence: Long, ciphertext: ByteArray) {
        append(direction, sequence, ciphertext, timestampNow(), null)
    }

    @Throws(ConnectJournalException::class)
    fun append(
        direction: ConnectDirection,
        sequence: Long,
        ciphertext: ByteArray,
        receivedAtMs: Long,
        ttlOverrideMs: Long?,
    ) {
        val retention = ttlOverrideMs ?: configuration.retentionMillis
        val expiresAt = Math.addExact(receivedAtMs, maxOf(retention, 1L))
        val digest = ConnectJournalRecord.computePayloadHash(ciphertext)
        val record = ConnectJournalRecord(direction, sequence, digest, ciphertext, receivedAtMs, expiresAt)
        fileFor(direction).append(record, receivedAtMs)
    }

    @Throws(ConnectJournalException::class)
    fun records(direction: ConnectDirection): List<ConnectJournalRecord> =
        records(direction, timestampNow())

    @Throws(ConnectJournalException::class)
    fun records(direction: ConnectDirection, nowMillis: Long): List<ConnectJournalRecord> =
        fileFor(direction).records(nowMillis)

    @Throws(ConnectJournalException::class)
    fun popOldest(direction: ConnectDirection, count: Int): List<ConnectJournalRecord> =
        popOldest(direction, count, timestampNow())

    @Throws(ConnectJournalException::class)
    fun popOldest(
        direction: ConnectDirection,
        count: Int,
        nowMillis: Long,
    ): List<ConnectJournalRecord> = fileFor(direction).popOldest(count, nowMillis)

    private fun fileFor(direction: ConnectDirection): ConnectJournalFile =
        if (direction == ConnectDirection.APP_TO_WALLET) appQueue else walletQueue

    private class ConnectQueueStorage(
        sessionId: ByteArray,
        rootDirectory: Path,
    ) {
        private val sessionDirectory: Path
        val appQueuePath: Path
        val walletQueuePath: Path

        init {
            val directoryName = hashSessionId(sessionId)
            sessionDirectory = rootDirectory.toAbsolutePath().normalize()
                .resolve(directoryName).normalize()
            appQueuePath = sessionDirectory.resolve("app_to_wallet.queue")
            walletQueuePath = sessionDirectory.resolve("wallet_to_app.queue")
        }

        @Throws(IOException::class)
        fun ensureSessionDirectory() {
            Files.createDirectories(sessionDirectory)
        }

        companion object {
            @OptIn(ExperimentalEncodingApi::class)
            @Throws(ConnectJournalException::class)
            private fun hashSessionId(sessionId: ByteArray): String {
                try {
                    val digest = MessageDigest.getInstance("SHA-256")
                    val hash = digest.digest(sessionId)
                    val builder = StringBuilder(hash.size * 2)
                    for (b in hash) {
                        builder.append(String.format("%02x", b))
                    }
                    return builder.toString()
                } catch (_: NoSuchAlgorithmException) {
                    return Base64.UrlSafe.encode(sessionId).replace('=', '_')
                } catch (ex: Exception) {
                    throw ConnectJournalException("failed to hash Connect session id", ex)
                }
            }
        }
    }

    private inner class ConnectJournalFile(
        private val direction: ConnectDirection,
        private val path: Path,
        private val configuration: JournalConfiguration,
    ) {
        private val lock = ReentrantLock()

        @Throws(ConnectJournalException::class)
        fun append(record: ConnectJournalRecord, nowMillis: Long) {
            lock.lock()
            try {
                storage.ensureSessionDirectory()
                val records = readLocked()
                records.add(record)
                pruneExpired(records, nowMillis)
                pruneLimits(records)
                writeLocked(records)
            } catch (ex: IOException) {
                throw ConnectJournalException("failed to append journal record", ex)
            } finally {
                lock.unlock()
            }
        }

        @Throws(ConnectJournalException::class)
        fun records(nowMillis: Long): List<ConnectJournalRecord> {
            lock.lock()
            try {
                val records = readLocked()
                if (pruneExpired(records, nowMillis)) {
                    writeLocked(records)
                }
                return records.toList()
            } finally {
                lock.unlock()
            }
        }

        @Throws(ConnectJournalException::class)
        fun popOldest(count: Int, nowMillis: Long): List<ConnectJournalRecord> {
            require(count > 0) { "count must be positive" }
            lock.lock()
            try {
                val records = readLocked()
                pruneExpired(records, nowMillis)
                val toRemove = minOf(count, records.size)
                val removed = ArrayList<ConnectJournalRecord>(toRemove)
                for (i in 0 until toRemove) {
                    removed.add(records.removeAt(0))
                }
                if (toRemove > 0) {
                    writeLocked(records)
                }
                return removed
            } finally {
                lock.unlock()
            }
        }

        @Throws(ConnectJournalException::class)
        private fun readLocked(): MutableList<ConnectJournalRecord> {
            if (!Files.exists(path)) {
                return ArrayList()
            }
            try {
                val bytes = Files.readAllBytes(path)
                val records = ArrayList<ConnectJournalRecord>()
                var offset = 0
                while (offset < bytes.size) {
                    val result = ConnectJournalRecord.decode(bytes, offset)
                    records.add(result.record)
                    offset += result.bytesConsumed
                }
                return records
            } catch (ex: IOException) {
                throw ConnectJournalException("failed to read journal file", ex)
            }
        }

        @Throws(ConnectJournalException::class)
        private fun writeLocked(records: List<ConnectJournalRecord>) {
            try {
                if (records.isEmpty()) {
                    Files.deleteIfExists(path)
                    return
                }
                val buffer = ByteArrayOutputStream()
                for (record in records) {
                    val encoded = record.encode()
                    buffer.write(encoded, 0, encoded.size)
                }
                val temp = path.resolveSibling(
                    "${path.fileName}.tmp-${System.nanoTime()}",
                )
                Files.createDirectories(path.parent)
                Files.write(
                    temp,
                    buffer.toByteArray(),
                    StandardOpenOption.CREATE,
                    StandardOpenOption.WRITE,
                    StandardOpenOption.TRUNCATE_EXISTING,
                )
                Files.move(temp, path, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE)
            } catch (ex: IOException) {
                throw ConnectJournalException("failed to write journal file", ex)
            }
        }

        private fun pruneExpired(records: MutableList<ConnectJournalRecord>, nowMillis: Long): Boolean {
            val iterator = records.iterator()
            var changed = false
            while (iterator.hasNext()) {
                val record = iterator.next()
                if (record.expiresAtMs <= nowMillis) {
                    iterator.remove()
                    changed = true
                }
            }
            return changed
        }

        private fun pruneLimits(records: MutableList<ConnectJournalRecord>): Boolean {
            var changed = false
            while (records.size > configuration.maxRecordsPerQueue) {
                records.removeAt(0)
                changed = true
            }
            while (totalEncodedSize(records) > configuration.maxBytesPerQueue && records.isNotEmpty()) {
                records.removeAt(0)
                changed = true
            }
            return changed
        }

        private fun totalEncodedSize(records: List<ConnectJournalRecord>): Int {
            var total = 0
            for (record in records) {
                total += record.encodedLength()
            }
            return total
        }
    }

    companion object {
        private fun timestampNow(): Long = System.currentTimeMillis()
    }
}
