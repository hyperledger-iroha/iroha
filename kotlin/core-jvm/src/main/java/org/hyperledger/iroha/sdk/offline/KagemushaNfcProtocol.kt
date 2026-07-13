package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest

class KagemushaNfcPayloadInfo(
    val kind: KagemushaPeerPayloadKind,
    val payloadLength: Int,
    val maximumChunkLength: Int,
    digest: ByteArray,
) {
    private val digest = digest.copyOf()
    val sha256: ByteArray get() = digest.copyOf()

    override fun equals(other: Any?): Boolean = other is KagemushaNfcPayloadInfo &&
        kind == other.kind && payloadLength == other.payloadLength &&
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
    const val MINIMUM_APPLICATION_IDENTIFIER_BYTES = 5
    const val MAXIMUM_APPLICATION_IDENTIFIER_BYTES = 16
    const val SAFE_CHUNK_BYTES = 220
    const val MAXIMUM_EXTENDED_READ_CHUNK_BYTES = 1_024
    const val MAXIMUM_EXTENDED_WRITE_CHUNK_BYTES = 16_384
    const val MAXIMUM_PAYLOAD_BYTES = KagemushaPeerTransportContract.MAXIMUM_TEXT_ENVELOPE_BYTES
    private const val INSTRUCTION_CLASS = 0x80
    private const val INSTRUCTION_GET_INFO = 0x10
    private const val INSTRUCTION_READ_CHUNK = 0x11
    private const val INSTRUCTION_WRITE_METADATA = 0x20
    private const val INSTRUCTION_WRITE_CHUNK = 0x21
    private const val INSTRUCTION_COMMIT = 0x22
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
        val value = rawHex.trim(' ', '\t', '\r', '\n')
        require(value.isNotEmpty() && value.length % 2 == 0 && value.all(::isAsciiHex)) {
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
        byteArrayOf(INSTRUCTION_CLASS.toByte(), INSTRUCTION_GET_INFO.toByte(), 0, 0, 0)

    @JvmStatic
    @JvmOverloads
    fun readChunkCommand(offset: Int, length: Int = SAFE_CHUNK_BYTES): ByteArray {
        requireOffset(offset)
        requireChunkLength(length, MAXIMUM_EXTENDED_READ_CHUNK_BYTES)
        return if (length <= 0xff) {
            byteArrayOf(
                INSTRUCTION_CLASS.toByte(), INSTRUCTION_READ_CHUNK.toByte(),
                (offset ushr 8).toByte(), offset.toByte(), length.toByte(),
            )
        } else {
            byteArrayOf(
                INSTRUCTION_CLASS.toByte(), INSTRUCTION_READ_CHUNK.toByte(),
                (offset ushr 8).toByte(), offset.toByte(), 0,
                (length ushr 8).toByte(), length.toByte(),
            )
        }
    }

    @JvmStatic
    fun writeMetadataCommand(kind: KagemushaPeerPayloadKind, payloadBytes: ByteArray): ByteArray {
        requirePayloadLength(payloadBytes.size)
        val digest = sha256(payloadBytes)
        return ByteArray(42).also { out ->
            out[0] = INSTRUCTION_CLASS.toByte()
            out[1] = INSTRUCTION_WRITE_METADATA.toByte()
            out[2] = 0
            out[3] = 0
            out[4] = 37
            out[5] = kind.code.toByte()
            out.writeU32(6, payloadBytes.size.toLong())
            digest.copyInto(out, 10)
            digest.fill(0)
        }
    }

    @JvmStatic
    fun writeChunkCommand(offset: Int, bytes: ByteArray): ByteArray {
        requireOffset(offset)
        requireChunkLength(bytes.size, MAXIMUM_EXTENDED_WRITE_CHUNK_BYTES)
        val header = if (bytes.size <= 0xff) 5 else 7
        return ByteArray(header + bytes.size).also { out ->
            out[0] = INSTRUCTION_CLASS.toByte()
            out[1] = INSTRUCTION_WRITE_CHUNK.toByte()
            out[2] = (offset ushr 8).toByte()
            out[3] = offset.toByte()
            if (header == 5) out[4] = bytes.size.toByte()
            else {
                out[4] = 0
                out[5] = (bytes.size ushr 8).toByte()
                out[6] = bytes.size.toByte()
            }
            bytes.copyInto(out, header)
        }
    }

    @JvmStatic fun commitCommand(): ByteArray =
        byteArrayOf(INSTRUCTION_CLASS.toByte(), INSTRUCTION_COMMIT.toByte(), 0, 0, 0)

    @JvmStatic
    @JvmOverloads
    fun writePayloadCommands(
        kind: KagemushaPeerPayloadKind,
        payloadBytes: ByteArray,
        maximumChunkLength: Int = SAFE_CHUNK_BYTES,
    ): List<ByteArray> {
        requirePayloadLength(payloadBytes.size)
        requireChunkLength(maximumChunkLength, MAXIMUM_EXTENDED_WRITE_CHUNK_BYTES)
        val commands = arrayListOf(writeMetadataCommand(kind, payloadBytes))
        var offset = 0
        while (offset < payloadBytes.size) {
            commands += writeChunkCommand(
                offset,
                payloadBytes.copyOfRange(offset, minOf(offset + maximumChunkLength, payloadBytes.size)),
            )
            offset += maximumChunkLength
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
        val offset = ((command[2].toInt() and 0xff) shl 8) or (command[3].toInt() and 0xff)
        return when (instruction) {
            INSTRUCTION_GET_INFO -> if (offset == 0 && isNoData(command))
                KagemushaNfcCommand.GetInfo else KagemushaNfcCommand.Invalid
            INSTRUCTION_READ_CHUNK -> readCommandLength(command)?.let {
                KagemushaNfcCommand.ReadChunk(offset, it)
            } ?: KagemushaNfcCommand.Invalid
            INSTRUCTION_WRITE_METADATA -> parseMetadata(offset, command)
            INSTRUCTION_WRITE_CHUNK -> commandData(command)?.takeIf {
                it.isNotEmpty() && it.size <= MAXIMUM_EXTENDED_WRITE_CHUNK_BYTES
            }?.let { KagemushaNfcCommand.WriteChunk(offset, it) } ?: KagemushaNfcCommand.Invalid
            INSTRUCTION_COMMIT -> if (offset == 0 && isNoData(command))
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
        return ByteArray(39).also { out ->
            out[0] = kind.code.toByte()
            out.writeU32(1, payloadBytes.size.toLong())
            out.writeU16(5, maximumChunkLength)
            digest.copyInto(out, 7)
            digest.fill(0)
        }
    }

    @JvmStatic
    fun decodeInfo(data: ByteArray): KagemushaNfcPayloadInfo? {
        if (data.size != 39) return null
        val kind = KagemushaPeerPayloadKind.fromCode(data[0].toInt() and 0xff) ?: return null
        val length = data.readU32(1).toInt()
        val chunk = data.readU16(5)
        val digest = data.copyOfRange(7, 39)
        if (length !in 1..MAXIMUM_PAYLOAD_BYTES ||
            chunk !in 1..MAXIMUM_EXTENDED_READ_CHUNK_BYTES || digest.all { it.toInt() == 0 }
        ) {
            digest.fill(0)
            return null
        }
        return KagemushaNfcPayloadInfo(kind, length, chunk, digest)
    }

    @JvmStatic fun response(data: ByteArray = ByteArray(0)): ByteArray = data + SUCCESS
    @JvmStatic fun responseStatus(response: ByteArray): Int? = if (response.size < 2) null else
        ((response[response.size - 2].toInt() and 0xff) shl 8) or
            (response.last().toInt() and 0xff)
    @JvmStatic fun responseData(response: ByteArray): ByteArray = if (response.size < 2)
        ByteArray(0) else response.copyOf(response.size - 2)
    @JvmStatic fun sha256(data: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(data)

    private fun parseMetadata(offset: Int, command: ByteArray): KagemushaNfcCommand {
        if (offset != 0) return KagemushaNfcCommand.Invalid
        val data = commandData(command) ?: return KagemushaNfcCommand.Invalid
        if (data.size != 37) return KagemushaNfcCommand.Invalid
        val kind = KagemushaPeerPayloadKind.fromCode(data[0].toInt() and 0xff)
            ?: return KagemushaNfcCommand.Invalid
        val length = data.readU32(1).toInt()
        val digest = data.copyOfRange(5, 37)
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

    private fun readCommandLength(command: ByteArray): Int? = when {
        command.size == 5 && (command[4].toInt() and 0xff) > 0 -> command[4].toInt() and 0xff
        command.size == 7 && command[4].toInt() == 0 ->
            (((command[5].toInt() and 0xff) shl 8) or (command[6].toInt() and 0xff))
                .takeIf { it in 1..MAXIMUM_EXTENDED_READ_CHUNK_BYTES }
        else -> null
    }

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

    private fun requireOffset(offset: Int) = require(offset in 0..0xffff) { "Invalid NFC offset" }
    private fun requirePayloadLength(length: Int) =
        require(length in 1..MAXIMUM_PAYLOAD_BYTES) { "Invalid NFC payload length" }
    private fun requireChunkLength(length: Int, maximum: Int) =
        require(length in 1..maximum) { "Invalid NFC chunk length" }
}

class KagemushaNfcPayloadAssembler(
    val kind: KagemushaPeerPayloadKind,
    val expectedLength: Int,
    expectedSha256: ByteArray,
) {
    private val expectedDigest = expectedSha256.copyOf()
    private val bytes: ByteArray
    private val written: BooleanArray
    private var writtenCount = 0

    init {
        require(expectedLength in 1..KagemushaNfcProtocol.MAXIMUM_PAYLOAD_BYTES) {
            "Invalid NFC payload length"
        }
        require(expectedDigest.size == 32 && expectedDigest.any { it.toInt() != 0 }) {
            "Invalid NFC payload digest"
        }
        bytes = ByteArray(expectedLength)
        written = BooleanArray(expectedLength)
    }

    constructor(info: KagemushaNfcPayloadInfo) : this(
        info.kind,
        info.payloadLength,
        info.sha256,
    )

    val isComplete: Boolean get() = writtenCount == expectedLength

    @Synchronized
    fun write(offset: Int, chunk: ByteArray): Boolean {
        if (offset < 0 || offset > expectedLength || chunk.isEmpty() ||
            chunk.size > KagemushaNfcProtocol.MAXIMUM_EXTENDED_WRITE_CHUNK_BYTES ||
            chunk.size > expectedLength - offset) return false
        chunk.indices.forEach { index ->
            val target = offset + index
            if (written[target] && bytes[target] != chunk[index]) return false
        }
        chunk.copyInto(bytes, offset)
        repeat(chunk.size) { index ->
            val target = offset + index
            if (!written[target]) {
                written[target] = true
                writtenCount += 1
            }
        }
        return true
    }

    @Synchronized
    fun commit(): ByteArray {
        check(isComplete) { "NFC payload is incomplete" }
        check(KagemushaNfcProtocol.sha256(bytes).contentEquals(expectedDigest)) {
            "NFC payload checksum mismatch"
        }
        return bytes.copyOf()
    }
}

private fun isAsciiHex(value: Char): Boolean =
    value in '0'..'9' || value in 'A'..'F' || value in 'a'..'f'

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
