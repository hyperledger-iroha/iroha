package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.crypto.Blake2b
import java.io.IOException
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.channels.FileChannel
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardOpenOption
import java.util.concurrent.locks.ReentrantLock
import javax.crypto.Mac
import javax.crypto.spec.SecretKeySpec

private val MAGIC = byteArrayOf(0x49, 0x4a, 0x4e, 0x4c) // "IJNL"
private const val VERSION: Byte = 1
private const val HASH_LENGTH = 32
private const val HMAC_LENGTH = 32
private const val HEADER_LENGTH = 4 + 1 // MAGIC.size + 1
private const val MIN_RECORD_LENGTH = 1 + 8 + 4 + HASH_LENGTH + HASH_LENGTH + HMAC_LENGTH

/**
 * Append-only journal mirroring the Rust/Swift `OfflineJournal` implementations.
 *
 * Records are stored as: `[kind (1)] [timestamp_le (8)] [payload_len_le (4)] [tx_id (32)]
 * [payload] [chain (32)] [hmac (32)]` where `chain = BLAKE2b-256(prev_chain || tx_id)` and
 * `hmac = HMAC-SHA256(prev_chain || record_without_hmac)`.
 */
class OfflineJournal @Throws(OfflineJournalException::class) constructor(
    val path: Path,
    key: OfflineJournalKey,
) : AutoCloseable {

    private enum class EntryKind(val tag: Byte) {
        PENDING(0),
        COMMITTED(1);

        companion object {
            fun fromByte(value: Byte): EntryKind {
                for (kind in entries) {
                    if (kind.tag == value) return kind
                }
                throw OfflineJournalException(
                    OfflineJournalException.Reason.INTEGRITY_VIOLATION,
                    "unknown offline journal entry kind: $value",
                )
            }
        }
    }

    private val keyBytes: ByteArray = key.raw()
    private val channel: FileChannel
    private val lock = ReentrantLock()
    private var lastChain: ByteArray
    private val pending: HashMap<HashKey, OfflineJournalEntry>
    private val committedSet: HashSet<HashKey>

    init {
        try {
            val parent = path.parent
            if (parent != null) Files.createDirectories(parent)
            channel = FileChannel.open(
                path,
                StandardOpenOption.CREATE,
                StandardOpenOption.READ,
                StandardOpenOption.WRITE,
            )
            ensureHeader()
            val state = loadState()
            pending = state.pending
            committedSet = state.committed
            lastChain = state.lastChain
            channel.position(channel.size())
        } catch (ex: IOException) {
            throw OfflineJournalException(
                OfflineJournalException.Reason.IO, "failed to open offline journal", ex,
            )
        }
    }

    /** Append a pending entry using the current wall clock timestamp. */
    @Throws(OfflineJournalException::class)
    fun appendPending(txId: ByteArray, payload: ByteArray?): OfflineJournalEntry =
        appendPending(txId, payload, System.currentTimeMillis())

    /** Append a pending entry with a custom timestamp (primarily for tests). */
    @Throws(OfflineJournalException::class)
    fun appendPending(txId: ByteArray, payload: ByteArray?, timestampMs: Long): OfflineJournalEntry {
        val normalizedId = requireTxId(txId)
        val normalizedPayload = payload?.copyOf() ?: ByteArray(0)
        lock.lock()
        try {
            val hk = HashKey.of(normalizedId)
            if (pending.containsKey(hk)) {
                throw OfflineJournalException(
                    OfflineJournalException.Reason.DUPLICATE_PENDING,
                    "transaction already pending in offline journal",
                )
            }
            if (committedSet.contains(hk)) {
                throw OfflineJournalException(
                    OfflineJournalException.Reason.ALREADY_COMMITTED,
                    "transaction already committed in offline journal",
                )
            }
            val chain = computeChain(lastChain, normalizedId)
            val record = encodeRecord(EntryKind.PENDING, normalizedId, normalizedPayload, timestampMs, chain)
            writeRecord(record)
            lastChain = chain
            val entry = OfflineJournalEntry(normalizedId, normalizedPayload, timestampMs, chain)
            pending[hk] = entry
            return entry
        } finally {
            lock.unlock()
        }
    }

    /** Marks a pending entry as committed. */
    @Throws(OfflineJournalException::class)
    fun markCommitted(txId: ByteArray) {
        markCommitted(txId, System.currentTimeMillis())
    }

    @Throws(OfflineJournalException::class)
    fun markCommitted(txId: ByteArray, timestampMs: Long) {
        val normalizedId = requireTxId(txId)
        lock.lock()
        try {
            val hk = HashKey.of(normalizedId)
            if (!pending.containsKey(hk)) {
                if (committedSet.contains(hk)) {
                    throw OfflineJournalException(
                        OfflineJournalException.Reason.ALREADY_COMMITTED,
                        "transaction already committed in offline journal",
                    )
                }
                throw OfflineJournalException(
                    OfflineJournalException.Reason.NOT_PENDING,
                    "transaction not pending in offline journal",
                )
            }
            pending.remove(hk)
            val chain = computeChain(lastChain, normalizedId)
            val record = encodeRecord(EntryKind.COMMITTED, normalizedId, ByteArray(0), timestampMs, chain)
            writeRecord(record)
            lastChain = chain
            committedSet.add(hk)
        } finally {
            lock.unlock()
        }
    }

    /** Returns the pending entries sorted by `tx_id`. */
    fun pendingEntries(): List<OfflineJournalEntry> {
        lock.lock()
        try {
            val entries = ArrayList(pending.values)
            entries.sortWith(Comparator.comparing(
                { it.txId },
                Comparator { a, b ->
                    val len = minOf(a.size, b.size)
                    for (i in 0 until len) {
                        val cmp = (a[i].toInt() and 0xFF) - (b[i].toInt() and 0xFF)
                        if (cmp != 0) return@Comparator cmp
                    }
                    a.size - b.size
                },
            ))
            return entries.toList()
        } finally {
            lock.unlock()
        }
    }

    @Throws(OfflineJournalException::class)
    override fun close() {
        lock.lock()
        try {
            channel.close()
        } catch (ex: IOException) {
            throw OfflineJournalException(
                OfflineJournalException.Reason.IO, "failed to close offline journal", ex,
            )
        } finally {
            lock.unlock()
        }
    }

    @Throws(IOException::class, OfflineJournalException::class)
    private fun ensureHeader() {
        channel.position(0)
        val header = ByteBuffer.allocate(HEADER_LENGTH)
        val read = channel.read(header)
        if (read >= HEADER_LENGTH) {
            header.flip()
            val magic = ByteArray(MAGIC.size)
            header.get(magic)
            val version = header.get()
            if (!magic.contentEquals(MAGIC) || version != VERSION) {
                throw OfflineJournalException(
                    OfflineJournalException.Reason.INTEGRITY_VIOLATION,
                    "offline journal header mismatch",
                )
            }
            channel.position(channel.size())
            return
        }
        channel.position(0)
        channel.write(ByteBuffer.wrap(MAGIC))
        channel.write(ByteBuffer.wrap(byteArrayOf(VERSION)))
        channel.force(true)
    }

    @Throws(OfflineJournalException::class)
    private fun loadState(): JournalState {
        try {
            val bytes = Files.readAllBytes(path)
            if (bytes.size < HEADER_LENGTH) return JournalState.empty()
            val magic = bytes.copyOfRange(0, MAGIC.size)
            val version = bytes[MAGIC.size]
            if (!magic.contentEquals(MAGIC) || version != VERSION) {
                throw OfflineJournalException(
                    OfflineJournalException.Reason.INTEGRITY_VIOLATION,
                    "offline journal header mismatch",
                )
            }
            val pendingEntries = HashMap<HashKey, OfflineJournalEntry>()
            val committedEntries = HashSet<HashKey>()
            var prevChain = ByteArray(HASH_LENGTH)
            var offset = HEADER_LENGTH
            while (offset < bytes.size) {
                if (bytes.size - offset < MIN_RECORD_LENGTH) {
                    throw OfflineJournalException(
                        OfflineJournalException.Reason.INTEGRITY_VIOLATION,
                        "truncated offline journal record",
                    )
                }
                val kindByte = bytes[offset++]
                val recordedAt = readUInt64LE(bytes, offset)
                offset += 8
                val payloadLength = readUInt32LE(bytes, offset)
                offset += 4
                if (payloadLength > Int.MAX_VALUE) {
                    throw OfflineJournalException(
                        OfflineJournalException.Reason.PAYLOAD_TOO_LARGE,
                        "payload too large in offline journal",
                    )
                }
                ensureAvailable(bytes, offset, HASH_LENGTH + payloadLength.toInt() + HASH_LENGTH + HMAC_LENGTH)
                val txId = bytes.copyOfRange(offset, offset + HASH_LENGTH)
                offset += HASH_LENGTH
                val payload = bytes.copyOfRange(offset, offset + payloadLength.toInt())
                offset += payloadLength.toInt()
                val storedChain = bytes.copyOfRange(offset, offset + HASH_LENGTH)
                offset += HASH_LENGTH
                val storedHmac = bytes.copyOfRange(offset, offset + HMAC_LENGTH)
                offset += HMAC_LENGTH

                val kind = EntryKind.fromByte(kindByte)
                val record = buildRecordWithoutHmac(kindByte, recordedAt, payloadLength, txId, payload, storedChain)
                val expectedChain = computeChain(prevChain, txId)
                if (!storedChain.contentEquals(expectedChain)) {
                    throw OfflineJournalException(
                        OfflineJournalException.Reason.INTEGRITY_VIOLATION,
                        "hash chain mismatch for tx ${toHex(txId)}",
                    )
                }
                val expectedHmac = computeHmac(prevChain, record)
                if (!storedHmac.contentEquals(expectedHmac)) {
                    throw OfflineJournalException(
                        OfflineJournalException.Reason.INTEGRITY_VIOLATION,
                        "HMAC mismatch for tx ${toHex(txId)}",
                    )
                }
                val hk = HashKey.of(txId)
                if (kind == EntryKind.PENDING) {
                    pendingEntries[hk] = OfflineJournalEntry(txId, payload, recordedAt, storedChain)
                } else {
                    pendingEntries.remove(hk)
                    committedEntries.add(hk)
                }
                prevChain = storedChain
            }
            return JournalState(pendingEntries, committedEntries, prevChain)
        } catch (ex: IOException) {
            throw OfflineJournalException(
                OfflineJournalException.Reason.IO, "failed to load offline journal", ex,
            )
        }
    }

    @Throws(OfflineJournalException::class)
    private fun encodeRecord(
        kind: EntryKind,
        txId: ByteArray,
        payload: ByteArray,
        timestampMs: Long,
        chain: ByteArray,
    ): ByteArray {
        if (payload.size.toLong() > 0xFFFF_FFFFL) {
            throw OfflineJournalException(
                OfflineJournalException.Reason.PAYLOAD_TOO_LARGE,
                "offline journal payload exceeds 4 GiB",
            )
        }
        val payloadLen = payload.size.toLong()
        val record = buildRecordWithoutHmac(kind.tag, timestampMs, payloadLen, txId, payload, chain)
        val hmac = computeHmac(lastChain, record)
        val encoded = record.copyOf(record.size + HMAC_LENGTH)
        System.arraycopy(hmac, 0, encoded, record.size, HMAC_LENGTH)
        return encoded
    }

    @Throws(OfflineJournalException::class)
    private fun writeRecord(record: ByteArray) {
        try {
            channel.position(channel.size())
            channel.write(ByteBuffer.wrap(record))
            channel.force(true)
        } catch (ex: IOException) {
            throw OfflineJournalException(
                OfflineJournalException.Reason.IO, "failed to persist offline journal record", ex,
            )
        }
    }

    @Throws(OfflineJournalException::class)
    private fun computeHmac(prevChain: ByteArray, record: ByteArray): ByteArray {
        try {
            val mac = Mac.getInstance("HmacSHA256")
            mac.init(SecretKeySpec(keyBytes, "HmacSHA256"))
            mac.update(prevChain)
            mac.update(record)
            return mac.doFinal()
        } catch (ex: java.security.GeneralSecurityException) {
            throw OfflineJournalException(
                OfflineJournalException.Reason.IO, "failed to compute offline journal HMAC", ex,
            )
        }
    }

    private class JournalState(
        val pending: HashMap<HashKey, OfflineJournalEntry>,
        val committed: HashSet<HashKey>,
        val lastChain: ByteArray,
    ) {
        companion object {
            fun empty(): JournalState =
                JournalState(HashMap(), HashSet(), ByteArray(HASH_LENGTH))
        }
    }

    private class HashKey private constructor(
        private val value: ByteArray,
        private val _hashCode: Int,
    ) {
        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is HashKey) return false
            return value.contentEquals(other.value)
        }

        override fun hashCode(): Int = _hashCode

        companion object {
            fun of(value: ByteArray): HashKey =
                HashKey(value.copyOf(), value.contentHashCode())
        }
    }

    companion object {
        private fun requireTxId(txId: ByteArray?): ByteArray {
            if (txId == null || txId.size != HASH_LENGTH) {
                throw OfflineJournalException(
                    OfflineJournalException.Reason.INVALID_TX_ID_LENGTH,
                    "offline journal tx_id must be exactly 32 bytes",
                )
            }
            return txId.copyOf()
        }

        private fun ensureAvailable(bytes: ByteArray, offset: Int, expectedRemaining: Int) {
            if (offset + expectedRemaining > bytes.size) {
                throw OfflineJournalException(
                    OfflineJournalException.Reason.INTEGRITY_VIOLATION,
                    "truncated offline journal record",
                )
            }
        }

        private fun computeChain(prevChain: ByteArray, txId: ByteArray): ByteArray {
            val input = ByteArray(prevChain.size + txId.size)
            System.arraycopy(prevChain, 0, input, 0, prevChain.size)
            System.arraycopy(txId, 0, input, prevChain.size, txId.size)
            return Blake2b.digest(input, HASH_LENGTH)
        }

        private fun buildRecordWithoutHmac(
            kind: Byte,
            timestampMs: Long,
            payloadLen: Long,
            txId: ByteArray,
            payload: ByteArray,
            chain: ByteArray,
        ): ByteArray {
            val buffer = ByteBuffer.allocate(1 + 8 + 4 + HASH_LENGTH + payloadLen.toInt() + HASH_LENGTH)
                .order(ByteOrder.LITTLE_ENDIAN)
            buffer.put(kind)
            buffer.putLong(timestampMs)
            buffer.putInt(payloadLen.toInt())
            buffer.put(txId)
            buffer.put(payload)
            buffer.put(chain)
            return buffer.array()
        }

        private fun readUInt32LE(bytes: ByteArray, offset: Int): Long {
            val value = (bytes[offset].toLong() and 0xFF) or
                ((bytes[offset + 1].toLong() and 0xFF) shl 8) or
                ((bytes[offset + 2].toLong() and 0xFF) shl 16) or
                ((bytes[offset + 3].toLong() and 0xFF) shl 24)
            return value and 0xFFFF_FFFFL
        }

        private fun readUInt64LE(bytes: ByteArray, offset: Int): Long {
            var value = 0L
            for (i in 0 until 8) {
                value = value or ((bytes[offset + i].toLong() and 0xFF) shl (i * 8))
            }
            return value
        }

        private fun toHex(bytes: ByteArray): String {
            val builder = StringBuilder(bytes.size * 2)
            for (b in bytes) {
                builder.append(String.format("%02x", b))
            }
            return builder.toString()
        }
    }
}
