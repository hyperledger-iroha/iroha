package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest
import java.util.TreeMap

class KagemushaNfcPayloadInfo(
    val transportVersion: Int,
    val kind: KagemushaPeerPayloadKind,
    val payloadLength: Int,
    val maximumChunkLength: Int,
    digest: ByteArray,
) {
    private val digest = digest.copyOf()
    val sha256: ByteArray get() = digest.copyOf()

    override fun equals(other: Any?): Boolean = other is KagemushaNfcPayloadInfo &&
        transportVersion == other.transportVersion && kind == other.kind &&
        payloadLength == other.payloadLength &&
        maximumChunkLength == other.maximumChunkLength && digest.contentEquals(other.digest)

    override fun hashCode(): Int = 31 * kind.hashCode() + digest.contentHashCode()
}

sealed class KagemushaNfcCommand {
    data object Select : KagemushaNfcCommand()
    data object SelectOtherApplication : KagemushaNfcCommand()
    data object GetInfo : KagemushaNfcCommand()
    data class ReadChunk(val offset: Int, val requestedLength: Int) : KagemushaNfcCommand()
    class WriteMetadata(
        val kind: KagemushaPeerPayloadKind,
        val payloadLength: Int,
        digest: ByteArray,
    ) : KagemushaNfcCommand() {
        private val digest = digest.copyOf()
        val sha256: ByteArray get() = digest.copyOf()
        override fun equals(other: Any?): Boolean = other is WriteMetadata &&
            kind == other.kind && payloadLength == other.payloadLength &&
            digest.contentEquals(other.digest)
        override fun hashCode(): Int = 31 * kind.hashCode() + digest.contentHashCode()
    }
    class WriteChunk(val offset: Int, chunk: ByteArray) : KagemushaNfcCommand() {
        private val chunk = chunk.copyOf()
        val bytes: ByteArray get() = chunk.copyOf()
        override fun equals(other: Any?): Boolean = other is WriteChunk &&
            offset == other.offset && chunk.contentEquals(other.chunk)
        override fun hashCode(): Int = 31 * offset + chunk.contentHashCode()
    }
    data object Commit : KagemushaNfcCommand()
    data object Unsupported : KagemushaNfcCommand()
    data object Invalid : KagemushaNfcCommand()
}

object KagemushaNfcProtocol {
    const val RAW_TRANSPORT_VERSION = 4
    const val MINIMUM_APPLICATION_IDENTIFIER_BYTES = 5
    const val MAXIMUM_APPLICATION_IDENTIFIER_BYTES = 16
    private const val MAXIMUM_APPLICATION_IDENTIFIER_PADDING_BYTES = 8
    const val SAFE_CHUNK_BYTES = 220
    const val MAXIMUM_EXTENDED_READ_CHUNK_BYTES = 1_024
    const val MAXIMUM_EXTENDED_WRITE_CHUNK_BYTES = 16_384
    const val MAXIMUM_PAYLOAD_BYTES = KagemushaPeerTransportContract.MAXIMUM_ARCHIVE_BYTES
    private const val SPARSE_FRAGMENT_ALLOWANCE = 64
    private const val MAXIMUM_SPARSE_FRAGMENT_COUNT =
        (MAXIMUM_PAYLOAD_BYTES + SAFE_CHUNK_BYTES - 1) / SAFE_CHUNK_BYTES +
            SPARSE_FRAGMENT_ALLOWANCE
    private const val INSTRUCTION_CLASS = 0x80
    private const val INSTRUCTION_GET_INFO = 0x10
    private const val INSTRUCTION_READ_CHUNK = 0x11
    private const val INSTRUCTION_WRITE_METADATA = 0x20
    private const val INSTRUCTION_WRITE_CHUNK = 0x21
    private const val INSTRUCTION_COMMIT = 0x22
    private const val OFFSET_BYTES = 4
    private const val READ_REQUEST_BYTES = OFFSET_BYTES + 2
    private val DEFAULT_AID = hexToBytes(KagemushaPeerTransportContract.NFC_APPLICATION_IDENTIFIER_HEX)
    private val SUCCESS = byteArrayOf(0x90.toByte(), 0)

    @JvmStatic fun defaultApplicationIdentifier(): ByteArray = DEFAULT_AID.copyOf()
    @JvmStatic fun statusSuccess(): ByteArray = SUCCESS.copyOf()
    @JvmStatic fun statusWrongData(): ByteArray = byteArrayOf(0x6a, 0x80.toByte())
    @JvmStatic fun statusNotFound(): ByteArray = byteArrayOf(0x6a, 0x82.toByte())
    @JvmStatic fun statusConditionsNotSatisfied(): ByteArray = byteArrayOf(0x69, 0x85.toByte())
    @JvmStatic fun statusUnsupported(): ByteArray = byteArrayOf(0x6d, 0)

    @JvmStatic
    fun applicationIdentifier(rawHex: String): ByteArray {
        val minimumEncodedLength = MINIMUM_APPLICATION_IDENTIFIER_BYTES * 2
        val maximumEncodedLength = MAXIMUM_APPLICATION_IDENTIFIER_BYTES * 2
        val rawEncodedLength = rawHex.length
        require(
            rawEncodedLength <=
                maximumEncodedLength + MAXIMUM_APPLICATION_IDENTIFIER_PADDING_BYTES &&
                rawHex.all(::isAsciiApplicationIdentifierText),
        ) {
            "Invalid Kagemusha NFC application identifier"
        }
        val value = rawHex.trim(' ', '\t', '\r', '\n')
        val encodedLengthRange = minimumEncodedLength..maximumEncodedLength
        require(
            value.length in encodedLengthRange &&
                rawEncodedLength - value.length <= MAXIMUM_APPLICATION_IDENTIFIER_PADDING_BYTES &&
                value.length % 2 == 0 && value.all(::isAsciiHex),
        ) {
            "Invalid Kagemusha NFC application identifier"
        }
        return validateApplicationIdentifier(hexToBytes(value))
    }

    @JvmStatic
    fun applicationIdentifierHex(value: ByteArray): String =
        validateApplicationIdentifier(value).joinToString("") { "%02X".format(it.toInt() and 0xff) }

    @JvmStatic
    fun validateApplicationIdentifier(value: ByteArray): ByteArray {
        require(value.size in MINIMUM_APPLICATION_IDENTIFIER_BYTES..MAXIMUM_APPLICATION_IDENTIFIER_BYTES) {
            "Invalid Kagemusha NFC application identifier"
        }
        return value.copyOf()
    }

    @JvmStatic
    @JvmOverloads
    fun selectApplicationCommand(applicationIdentifier: ByteArray = DEFAULT_AID): ByteArray {
        val aid = validateApplicationIdentifier(applicationIdentifier)
        return ByteArray(6 + aid.size).also { out ->
            out[0] = 0
            out[1] = 0xa4.toByte()
            out[2] = 4
            out[3] = 0
            out[4] = aid.size.toByte()
            aid.copyInto(out, 5)
            out[out.lastIndex] = 0
            aid.fill(0)
        }
    }

    @JvmStatic fun getInfoCommand(): ByteArray =
        byteArrayOf(
            INSTRUCTION_CLASS.toByte(),
            INSTRUCTION_GET_INFO.toByte(),
            RAW_TRANSPORT_VERSION.toByte(),
            0,
            0,
        )

    @JvmStatic
    @JvmOverloads
    fun readChunkCommand(offset: Int, length: Int = SAFE_CHUNK_BYTES): ByteArray {
        requireChunkLength(length, MAXIMUM_EXTENDED_READ_CHUNK_BYTES)
        requireTransferRange(offset, length, MAXIMUM_EXTENDED_READ_CHUNK_BYTES)
        val data = ByteArray(READ_REQUEST_BYTES)
        data.writeU32(0, offset.toLong())
        data.writeU16(OFFSET_BYTES, length)
        return dataCommand(INSTRUCTION_READ_CHUNK, data)
    }

    @JvmStatic
    fun writeMetadataCommand(kind: KagemushaPeerPayloadKind, payloadBytes: ByteArray): ByteArray {
        requirePayloadLength(payloadBytes.size)
        val digest = sha256(payloadBytes)
        val metadata = ByteArray(38).also { out ->
            out[0] = RAW_TRANSPORT_VERSION.toByte()
            out[1] = kind.code.toByte()
            out.writeU32(2, payloadBytes.size.toLong())
            digest.copyInto(out, 6)
            digest.fill(0)
        }
        return dataCommand(INSTRUCTION_WRITE_METADATA, metadata)
    }

    @JvmStatic
    fun writeChunkCommand(offset: Int, bytes: ByteArray): ByteArray {
        requireChunkLength(bytes.size, MAXIMUM_EXTENDED_WRITE_CHUNK_BYTES)
        requireTransferRange(offset, bytes.size, MAXIMUM_EXTENDED_WRITE_CHUNK_BYTES)
        val data = ByteArray(OFFSET_BYTES + bytes.size).also { out ->
            out.writeU32(0, offset.toLong())
            bytes.copyInto(out, OFFSET_BYTES)
        }
        return dataCommand(INSTRUCTION_WRITE_CHUNK, data)
    }

    @JvmStatic fun commitCommand(): ByteArray =
        byteArrayOf(
            INSTRUCTION_CLASS.toByte(),
            INSTRUCTION_COMMIT.toByte(),
            RAW_TRANSPORT_VERSION.toByte(),
            0,
            0,
        )

    /** Builds a canonical bulk write whose non-final chunks are at least 220 bytes. */
    @JvmStatic
    @JvmOverloads
    fun writePayloadCommands(
        kind: KagemushaPeerPayloadKind,
        payloadBytes: ByteArray,
        maximumChunkLength: Int = SAFE_CHUNK_BYTES,
    ): List<ByteArray> {
        requirePayloadLength(payloadBytes.size)
        require(maximumChunkLength >= SAFE_CHUNK_BYTES) {
            "Kagemusha NFC bulk-write chunks must be at least $SAFE_CHUNK_BYTES bytes"
        }
        requireChunkLength(maximumChunkLength, MAXIMUM_EXTENDED_WRITE_CHUNK_BYTES)
        val commands = arrayListOf(writeMetadataCommand(kind, payloadBytes))
        var offset = 0
        while (offset < payloadBytes.size) {
            val end = minOf(offset + maximumChunkLength, payloadBytes.size)
            commands += writeChunkCommand(
                offset,
                payloadBytes.copyOfRange(offset, end),
            )
            offset = end
        }
        commands += commitCommand()
        return commands
    }

    @JvmStatic
    @JvmOverloads
    fun parseCommand(
        command: ByteArray?,
        applicationIdentifier: ByteArray = DEFAULT_AID,
    ): KagemushaNfcCommand {
        if (command == null || command.size < 4) return KagemushaNfcCommand.Invalid
        if (isSelect(command, applicationIdentifier)) return KagemushaNfcCommand.Select
        if (isAnySelect(command)) return KagemushaNfcCommand.SelectOtherApplication
        if ((command[0].toInt() and 0xff) != INSTRUCTION_CLASS) return KagemushaNfcCommand.Unsupported
        val instruction = command[1].toInt() and 0xff
        val canonicalParameters =
            (command[2].toInt() and 0xff) == RAW_TRANSPORT_VERSION && command[3].toInt() == 0
        return when (instruction) {
            INSTRUCTION_GET_INFO -> if (canonicalParameters && isNoData(command))
                KagemushaNfcCommand.GetInfo else KagemushaNfcCommand.Invalid
            INSTRUCTION_READ_CHUNK -> if (!canonicalParameters) KagemushaNfcCommand.Invalid else {
                val data = commandData(command)
                if (data == null || data.size != READ_REQUEST_BYTES) {
                    KagemushaNfcCommand.Invalid
                } else {
                    val offset = data.readU32(0).toInt()
                    val length = data.readU16(OFFSET_BYTES)
                    if (transferRangeIsValid(offset, length, MAXIMUM_EXTENDED_READ_CHUNK_BYTES)) {
                        KagemushaNfcCommand.ReadChunk(offset, length)
                    } else KagemushaNfcCommand.Invalid
                }
            }
            INSTRUCTION_WRITE_METADATA -> parseMetadata(canonicalParameters, command)
            INSTRUCTION_WRITE_CHUNK -> if (!canonicalParameters) KagemushaNfcCommand.Invalid else {
                val data = commandData(command)
                if (data == null || data.size <= OFFSET_BYTES ||
                    data.size > OFFSET_BYTES + MAXIMUM_EXTENDED_WRITE_CHUNK_BYTES
                ) {
                    KagemushaNfcCommand.Invalid
                } else {
                    val offset = data.readU32(0).toInt()
                    val chunk = data.copyOfRange(OFFSET_BYTES, data.size)
                    if (transferRangeIsValid(offset, chunk.size, MAXIMUM_EXTENDED_WRITE_CHUNK_BYTES)) {
                        KagemushaNfcCommand.WriteChunk(offset, chunk)
                    } else KagemushaNfcCommand.Invalid
                }
            }
            INSTRUCTION_COMMIT -> if (canonicalParameters && isNoData(command))
                KagemushaNfcCommand.Commit else KagemushaNfcCommand.Invalid
            else -> KagemushaNfcCommand.Unsupported
        }
    }

    @JvmStatic
    @JvmOverloads
    fun encodeInfo(
        kind: KagemushaPeerPayloadKind,
        payloadBytes: ByteArray,
        maximumChunkLength: Int = SAFE_CHUNK_BYTES,
    ): ByteArray {
        requirePayloadLength(payloadBytes.size)
        requireChunkLength(maximumChunkLength, MAXIMUM_EXTENDED_READ_CHUNK_BYTES)
        val digest = sha256(payloadBytes)
        return ByteArray(40).also { out ->
            out[0] = RAW_TRANSPORT_VERSION.toByte()
            out[1] = kind.code.toByte()
            out.writeU32(2, payloadBytes.size.toLong())
            out.writeU16(6, maximumChunkLength)
            digest.copyInto(out, 8)
            digest.fill(0)
        }
    }

    @JvmStatic
    fun decodeInfo(data: ByteArray): KagemushaNfcPayloadInfo? {
        if (data.size != 40 || (data[0].toInt() and 0xff) != RAW_TRANSPORT_VERSION) return null
        val kind = KagemushaPeerPayloadKind.fromCode(data[1].toInt() and 0xff) ?: return null
        val length = data.readU32(2).toInt()
        val chunk = data.readU16(6)
        val digest = data.copyOfRange(8, 40)
        if (length !in 1..MAXIMUM_PAYLOAD_BYTES ||
            chunk !in 1..MAXIMUM_EXTENDED_READ_CHUNK_BYTES || digest.all { it.toInt() == 0 }
        ) {
            digest.fill(0)
            return null
        }
        return KagemushaNfcPayloadInfo(RAW_TRANSPORT_VERSION, kind, length, chunk, digest)
    }

    @JvmStatic fun response(data: ByteArray = ByteArray(0)): ByteArray = data + SUCCESS
    @JvmStatic fun responseStatus(response: ByteArray): Int? = if (response.size < 2) null else
        ((response[response.size - 2].toInt() and 0xff) shl 8) or
            (response.last().toInt() and 0xff)
    @JvmStatic fun responseData(response: ByteArray): ByteArray = if (response.size < 2)
        ByteArray(0) else response.copyOf(response.size - 2)
    @JvmStatic fun sha256(data: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(data)

    private fun parseMetadata(
        canonicalParameters: Boolean,
        command: ByteArray,
    ): KagemushaNfcCommand {
        if (!canonicalParameters) return KagemushaNfcCommand.Invalid
        val data = commandData(command) ?: return KagemushaNfcCommand.Invalid
        if (data.size != 38 ||
            (data[0].toInt() and 0xff) != RAW_TRANSPORT_VERSION
        ) return KagemushaNfcCommand.Invalid
        val kind = KagemushaPeerPayloadKind.fromCode(data[1].toInt() and 0xff)
            ?: return KagemushaNfcCommand.Invalid
        val length = data.readU32(2).toInt()
        val digest = data.copyOfRange(6, 38)
        if (length !in 1..MAXIMUM_PAYLOAD_BYTES || digest.all { it.toInt() == 0 }) {
            digest.fill(0)
            return KagemushaNfcCommand.Invalid
        }
        return KagemushaNfcCommand.WriteMetadata(kind, length, digest)
    }

    private fun isSelect(command: ByteArray, aid: ByteArray): Boolean {
        if (!isAnySelect(command) || aid.size !in MINIMUM_APPLICATION_IDENTIFIER_BYTES..
            MAXIMUM_APPLICATION_IDENTIFIER_BYTES) return false
        val length = command[4].toInt() and 0xff
        val end = 5 + length
        return length == aid.size && command.copyOfRange(5, end).contentEquals(aid)
    }

    private fun isAnySelect(command: ByteArray): Boolean {
        if (command.size < 5 || command[0].toInt() != 0 ||
            (command[1].toInt() and 0xff) != 0xa4 || command[2].toInt() != 4 ||
            command[3].toInt() != 0) return false
        val length = command[4].toInt() and 0xff
        val end = 5 + length
        return length > 0 && (command.size == end ||
            (command.size == end + 1 && command[end].toInt() == 0))
    }

    private fun isNoData(command: ByteArray): Boolean = command.size == 4 ||
        (command.size == 5 && command[4].toInt() == 0)

    private fun commandData(command: ByteArray): ByteArray? {
        if (command.size < 5) return null
        val shortLength = command[4].toInt() and 0xff
        if (shortLength > 0) return command.takeIf { it.size == 5 + shortLength }
            ?.copyOfRange(5, command.size)
        if (command.size < 7) return null
        val extended = ((command[5].toInt() and 0xff) shl 8) or (command[6].toInt() and 0xff)
        return command.takeIf { extended > 0 && it.size == 7 + extended }
            ?.copyOfRange(7, command.size)
    }

    private fun dataCommand(instruction: Int, data: ByteArray): ByteArray {
        require(data.isNotEmpty() && data.size <= 0xffff) { "Invalid NFC APDU data length" }
        val header = if (data.size <= 0xff) 5 else 7
        return ByteArray(header + data.size).also { out ->
            out[0] = INSTRUCTION_CLASS.toByte()
            out[1] = instruction.toByte()
            out[2] = RAW_TRANSPORT_VERSION.toByte()
            out[3] = 0
            if (header == 5) {
                out[4] = data.size.toByte()
            } else {
                out[4] = 0
                out[5] = (data.size ushr 8).toByte()
                out[6] = data.size.toByte()
            }
            data.copyInto(out, header)
        }
    }

    private fun transferRangeIsValid(offset: Int, length: Int, maximumChunkLength: Int): Boolean =
        offset >= 0 && length in 1..maximumChunkLength &&
            offset < MAXIMUM_PAYLOAD_BYTES &&
            offset.toLong() + length.toLong() <= MAXIMUM_PAYLOAD_BYTES.toLong()

    private fun requireTransferRange(offset: Int, length: Int, maximumChunkLength: Int) =
        require(transferRangeIsValid(offset, length, maximumChunkLength)) {
            "Invalid NFC transfer range"
        }

    private fun requirePayloadLength(length: Int) =
        require(length in 1..MAXIMUM_PAYLOAD_BYTES) { "Invalid NFC payload length" }
    private fun requireChunkLength(length: Int, maximum: Int) =
        require(length in 1..maximum) { "Invalid NFC chunk length" }

    internal fun sparseFragmentBudget(payloadLength: Int): Int {
        val canonicalFragments =
            (payloadLength + SAFE_CHUNK_BYTES - 1) / SAFE_CHUNK_BYTES
        return minOf(
            canonicalFragments + SPARSE_FRAGMENT_ALLOWANCE,
            MAXIMUM_SPARSE_FRAGMENT_COUNT,
        )
    }
}

class KagemushaNfcPayloadAssembler(
    val kind: KagemushaPeerPayloadKind,
    val expectedLength: Int,
    expectedSha256: ByteArray,
) {
    private class StoredFragment(val offset: Int, val bytes: ByteArray) {
        val end: Int get() = offset + bytes.size
    }

    private val expectedDigest: ByteArray
    private val fragments = TreeMap<Int, ByteArray>()
    private val coveredRanges = TreeMap<Int, Int>()
    // The canonical 220-byte writer needs ceil(length / 220) fragments. A
    // fixed allowance admits modest overlap/out-of-order splitting without
    // allowing attacker-selected one-byte writes to create unbounded nodes.
    private val fragmentBudget: Int
    private var bufferedBytes = 0
    private var cleared = false

    init {
        require(expectedLength in 1..KagemushaNfcProtocol.MAXIMUM_PAYLOAD_BYTES) {
            "Invalid NFC payload length"
        }
        require(expectedSha256.size == 32 && expectedSha256.any { it.toInt() != 0 }) {
            "Invalid NFC payload digest"
        }
        expectedDigest = expectedSha256.copyOf()
        fragmentBudget = KagemushaNfcProtocol.sparseFragmentBudget(expectedLength)
    }

    constructor(info: KagemushaNfcPayloadInfo) : this(
        info.kind,
        info.payloadLength,
        info.sha256,
    )

    /** Unique payload bytes currently retained by the sparse assembler. */
    @get:Synchronized
    val bufferedByteCount: Int get() = bufferedBytes

    @get:Synchronized
    val isComplete: Boolean get() = !cleared && bufferedBytes == expectedLength &&
        coveredRanges.size == 1 && coveredRanges.firstKey() == 0 &&
        coveredRanges.firstEntry().value == expectedLength

    @Synchronized
    fun write(offset: Int, chunk: ByteArray): Boolean {
        if (cleared || offset < 0 || offset > expectedLength || chunk.isEmpty() ||
            chunk.size > KagemushaNfcProtocol.MAXIMUM_EXTENDED_WRITE_CHUNK_BYTES ||
            chunk.size > expectedLength - offset) return false

        val end = offset + chunk.size
        val overlaps = overlappingFragments(offset, end)
        overlaps.forEach { fragment ->
            val overlapStart = maxOf(offset, fragment.offset)
            val overlapEnd = minOf(end, fragment.end)
            for (target in overlapStart until overlapEnd) {
                if (fragment.bytes[target - fragment.offset] != chunk[target - offset]) return false
            }
        }

        var proposedFragmentCount = 0
        var budgetCursor = offset
        overlaps.forEach { fragment ->
            if (budgetCursor < fragment.offset) proposedFragmentCount += 1
            budgetCursor = maxOf(budgetCursor, minOf(fragment.end, end))
        }
        if (budgetCursor < end) proposedFragmentCount += 1
        if (fragments.size + proposedFragmentCount > fragmentBudget) {
            clear()
            return false
        }

        // Allocate only uncovered bytes, and do so before mutating state. The
        // interval map coalesces coverage while immutable fragments avoid
        // repeatedly copying a growing segment for sequential writes.
        val additions = ArrayList<StoredFragment>()
        var cursor = offset
        overlaps.forEach { fragment ->
            if (cursor < fragment.offset) {
                val gapEnd = minOf(fragment.offset, end)
                additions += StoredFragment(
                    cursor,
                    chunk.copyOfRange(cursor - offset, gapEnd - offset),
                )
            }
            cursor = maxOf(cursor, minOf(fragment.end, end))
        }
        if (cursor < end) {
            additions += StoredFragment(cursor, chunk.copyOfRange(cursor - offset, end - offset))
        }
        if (additions.isEmpty()) return true
        additions.forEach { addition -> fragments[addition.offset] = addition.bytes }
        bufferedBytes += additions.sumOf { it.bytes.size }
        mergeCoverage(offset, end)
        return true
    }

    @Synchronized
    fun commit(): ByteArray {
        check(!cleared) { "NFC payload assembler is cleared" }
        check(isComplete) { "NFC payload is incomplete" }
        var assembled: ByteArray? = null
        var succeeded = false
        try {
            val output = ByteArray(expectedLength)
            assembled = output
            var cursor = 0
            fragments.forEach { (offset, fragment) ->
                check(offset == cursor && fragment.size <= expectedLength - cursor) {
                    "NFC payload reconstruction mismatch"
                }
                fragment.copyInto(output, cursor)
                cursor += fragment.size
            }
            check(cursor == expectedLength) { "NFC payload reconstruction mismatch" }
            check(KagemushaNfcProtocol.sha256(output).contentEquals(expectedDigest)) {
                "NFC payload checksum mismatch"
            }
            succeeded = true
            return output
        } finally {
            if (!succeeded) assembled?.fill(0)
            // Exact coverage makes commit a one-shot operation. Both success
            // and terminal validation failure consume and zeroize the state.
            clear()
        }
    }

    @Synchronized
    fun clear() {
        expectedDigest.fill(0)
        fragments.values.forEach { it.fill(0) }
        fragments.clear()
        coveredRanges.clear()
        bufferedBytes = 0
        cleared = true
    }

    private fun overlappingFragments(start: Int, end: Int): List<StoredFragment> {
        val overlapping = ArrayList<StoredFragment>()
        var entry = fragments.floorEntry(start)
        if (entry == null || entry.key + entry.value.size <= start) {
            entry = fragments.ceilingEntry(start)
        }
        while (entry != null && entry.key < end) {
            overlapping += StoredFragment(entry.key, entry.value)
            entry = fragments.higherEntry(entry.key)
        }
        return overlapping
    }

    private fun mergeCoverage(start: Int, end: Int) {
        var mergedStart = start
        var mergedEnd = end
        val floor = coveredRanges.floorEntry(mergedStart)
        if (floor != null && floor.value >= mergedStart) {
            mergedStart = floor.key
            mergedEnd = maxOf(mergedEnd, floor.value)
            coveredRanges.remove(floor.key)
        }
        var next = coveredRanges.ceilingEntry(mergedStart)
        while (next != null && next.key <= mergedEnd) {
            mergedEnd = maxOf(mergedEnd, next.value)
            coveredRanges.remove(next.key)
            next = coveredRanges.ceilingEntry(mergedStart)
        }
        coveredRanges[mergedStart] = mergedEnd
    }
}

private fun isAsciiHex(value: Char): Boolean =
    value in '0'..'9' || value in 'A'..'F' || value in 'a'..'f'

private fun isAsciiApplicationIdentifierText(value: Char): Boolean =
    isAsciiHex(value) || value == ' ' || value == '\t' || value == '\r' || value == '\n'

private fun hexToBytes(value: String): ByteArray = ByteArray(value.length / 2) { index ->
    value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
}

private fun ByteArray.writeU16(offset: Int, value: Int) {
    this[offset] = (value ushr 8).toByte()
    this[offset + 1] = value.toByte()
}

private fun ByteArray.writeU32(offset: Int, value: Long) {
    this[offset] = (value ushr 24).toByte()
    this[offset + 1] = (value ushr 16).toByte()
    this[offset + 2] = (value ushr 8).toByte()
    this[offset + 3] = value.toByte()
}

private fun ByteArray.readU16(offset: Int): Int =
    ((this[offset].toInt() and 0xff) shl 8) or (this[offset + 1].toInt() and 0xff)

private fun ByteArray.readU32(offset: Int): Long =
    ((this[offset].toLong() and 0xff) shl 24) or
        ((this[offset + 1].toLong() and 0xff) shl 16) or
        ((this[offset + 2].toLong() and 0xff) shl 8) or
        (this[offset + 3].toLong() and 0xff)
