package org.hyperledger.iroha.sdk.offline

import java.io.RandomAccessFile
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.nio.file.attribute.PosixFilePermission

/** Production-size QR rail for eligibility-gated Kagemusha payments. */
object IrohaPeerQRV2 {
    const val TEXT_PREFIX = "IQR2:"
    const val TEXT_SUFFIX = ":"
    const val SHARD_BYTES = 256
    const val MAXIMUM_FRAME_TEXT_BYTES = 700
}

enum class IrohaPeerQRFrameKindV2(val code: Int) {
    HEADER(1),
    DATA(2);

    companion object {
        @JvmStatic fun fromCode(code: Int): IrohaPeerQRFrameKindV2? =
            entries.firstOrNull { it.code == code }
    }
}

/** CRC32C-protected IQR2 frame with unsigned 32-bit shard coordinates. */
class IrohaPeerQRFrameV2(
    val kind: IrohaPeerQRFrameKindV2,
    streamId: ByteArray,
    val index: Long,
    val total: Long,
    payload: ByteArray,
) {
    private val stream = streamId.copyOf()
    private val framePayload = payload.copyOf()
    val streamId: ByteArray get() = stream.copyOf()
    val payload: ByteArray get() = framePayload.copyOf()

    init {
        val maximumShards =
            (IrohaPeerWireMessageV1.MAXIMUM_KAGEMUSHA_ELIGIBILITY_ENVELOPE_BYTES.toLong() +
                IrohaPeerQRV2.SHARD_BYTES - 1) / IrohaPeerQRV2.SHARD_BYTES
        require(stream.size == 16 && total in 1..maximumShards && index in 0..0xffff_ffffL) {
            "Malformed IQR2 shard coordinates"
        }
        when (kind) {
            IrohaPeerQRFrameKindV2.HEADER -> require(
                index == 0L && framePayload.size == IrohaPeerWireMessageV1.HEADER_LENGTH,
            ) { "Malformed IQR2 header frame" }
            IrohaPeerQRFrameKindV2.DATA -> require(
                index < total && framePayload.isNotEmpty() &&
                    framePayload.size <= IrohaPeerQRV2.SHARD_BYTES,
            ) { "Malformed IQR2 data frame" }
        }
    }

    fun encode(): ByteArray {
        val payloadEnd = PAYLOAD_OFFSET + framePayload.size
        return ByteArray(payloadEnd + CHECKSUM_BYTES).also { out ->
            MAGIC.copyInto(out, 0)
            out[4] = VERSION.toByte()
            out[5] = kind.code.toByte()
            out.iqr2WriteU16(6, IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND.code)
            out[8] = IrohaPeerPayloadKind.PAYMENT.code.toByte()
            out[9] = 0
            out.iqr2WriteU16(
                10,
                IrohaPeerWireMessageV1.KAGEMUSHA_ELIGIBILITY_PAYMENT_SCHEMA_VERSION,
            )
            stream.copyInto(out, 12)
            out.iqr2WriteU32(28, index)
            out.iqr2WriteU32(32, total)
            out.iqr2WriteU16(36, framePayload.size)
            framePayload.copyInto(out, PAYLOAD_OFFSET)
            out.iqr2WriteU32(payloadEnd, iqr2Crc32c(out, 0, payloadEnd))
        }
    }

    fun encodeText(): String {
        val bytes = encode()
        return try {
            IrohaPeerQRV2.TEXT_PREFIX + iqr2Base45Encode(bytes) + IrohaPeerQRV2.TEXT_SUFFIX
        } finally {
            bytes.fill(0)
        }
    }

    override fun equals(other: Any?): Boolean = other is IrohaPeerQRFrameV2 &&
        kind == other.kind && index == other.index && total == other.total &&
        stream.contentEquals(other.stream) && framePayload.contentEquals(other.framePayload)

    override fun hashCode(): Int = 31 * kind.hashCode() + framePayload.contentHashCode()

    companion object {
        const val VERSION = 2
        const val PAYLOAD_OFFSET = 38
        const val CHECKSUM_BYTES = 4
        private val MAGIC = "IRQ2".toByteArray(Charsets.US_ASCII)

        @JvmStatic fun decode(data: ByteArray): IrohaPeerQRFrameV2 {
            require(data.size in (PAYLOAD_OFFSET + CHECKSUM_BYTES)..
                (PAYLOAD_OFFSET + IrohaPeerQRV2.SHARD_BYTES + CHECKSUM_BYTES) &&
                data.copyOfRange(0, 4).contentEquals(MAGIC) &&
                (data[4].toInt() and 0xff) == VERSION &&
                data.iqr2ReadU16(6) == IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND.code &&
                (data[8].toInt() and 0xff) == IrohaPeerPayloadKind.PAYMENT.code &&
                data[9].toInt() == 0 &&
                data.iqr2ReadU16(10) ==
                IrohaPeerWireMessageV1.KAGEMUSHA_ELIGIBILITY_PAYMENT_SCHEMA_VERSION
            ) { "Malformed IQR2 frame" }
            val kind = IrohaPeerQRFrameKindV2.fromCode(data[5].toInt() and 0xff)
                ?: throw IllegalArgumentException("Malformed IQR2 frame kind")
            val payloadLength = data.iqr2ReadU16(36)
            val payloadEnd = PAYLOAD_OFFSET + payloadLength
            require(payloadEnd + CHECKSUM_BYTES == data.size &&
                data.iqr2ReadU32(payloadEnd) == iqr2Crc32c(data, 0, payloadEnd)
            ) { "Malformed IQR2 checksum or length" }
            return IrohaPeerQRFrameV2(
                kind,
                data.copyOfRange(12, 28),
                data.iqr2ReadU32(28),
                data.iqr2ReadU32(32),
                data.copyOfRange(PAYLOAD_OFFSET, payloadEnd),
            )
        }

        @JvmStatic fun decodeText(value: String): IrohaPeerQRFrameV2 {
            require(value.toByteArray(Charsets.UTF_8).size <=
                IrohaPeerQRV2.MAXIMUM_FRAME_TEXT_BYTES &&
                value.startsWith(IrohaPeerQRV2.TEXT_PREFIX) &&
                value.endsWith(IrohaPeerQRV2.TEXT_SUFFIX) &&
                value.length > IrohaPeerQRV2.TEXT_PREFIX.length + IrohaPeerQRV2.TEXT_SUFFIX.length
            ) { "Malformed IQR2 text" }
            val body = value.substring(
                IrohaPeerQRV2.TEXT_PREFIX.length,
                value.length - IrohaPeerQRV2.TEXT_SUFFIX.length,
            )
            val bytes = iqr2Base45Decode(body)
                ?: throw IllegalArgumentException("IQR2 body is not canonical Base45")
            return try {
                require(iqr2Base45Encode(bytes) == body) { "IQR2 body is not canonical Base45" }
                decode(bytes)
            } finally {
                bytes.fill(0)
            }
        }
    }
}

/** Lazy encoder; callers render or transmit one IQR2 frame at a time. */
class IrohaPeerQREncoderV2(message: IrohaPeerWireMessageV1) {
    private val header: ByteArray
    private val body = message.encodedBody
    private val stream = message.streamId
    val dataShardCount: Long
        get() = (body.size.toLong() + IrohaPeerQRV2.SHARD_BYTES - 1) /
            IrohaPeerQRV2.SHARD_BYTES

    init {
        require(message.canonicalPayload.profile ==
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND &&
            message.canonicalPayload.kind == IrohaPeerPayloadKind.PAYMENT &&
            message.canonicalPayload.schemaVersion ==
            IrohaPeerWireMessageV1.KAGEMUSHA_ELIGIBILITY_PAYMENT_SCHEMA_VERSION
        ) { "IQR2 carries only eligibility PAYMENT 0x0103" }
        val encoded = message.encode()
        try {
            header = encoded.copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH)
        } finally {
            encoded.fill(0)
        }
        require(dataShardCount in 1..0xffff_ffffL) { "IQR2 message is too large" }
    }

    fun headerFrame(): IrohaPeerQRFrameV2 = IrohaPeerQRFrameV2(
        IrohaPeerQRFrameKindV2.HEADER,
        stream,
        0,
        dataShardCount,
        header,
    )

    fun dataFrame(index: Long): IrohaPeerQRFrameV2 {
        require(index in 0 until dataShardCount) { "Invalid IQR2 shard index" }
        val start = Math.multiplyExact(index, IrohaPeerQRV2.SHARD_BYTES.toLong()).toInt()
        return IrohaPeerQRFrameV2(
            IrohaPeerQRFrameKindV2.DATA,
            stream,
            index,
            dataShardCount,
            body.copyOfRange(start, minOf(start + IrohaPeerQRV2.SHARD_BYTES, body.size)),
        )
    }
}

/**
 * File-backed bounded IQR2 receiver. The exact IPM1 header is validated before
 * a file is created or sized, and duplicate shards must be byte-identical.
 */
class IrohaPeerQRFileAssemblerV2 @JvmOverloads constructor(
    private val directory: Path = Paths.get(System.getProperty("java.io.tmpdir")),
    private val limits: IrohaPeerWireLimitsV1 = IrohaPeerWireLimitsV1.PEER_V1,
) : AutoCloseable {
    private var inspectedHeader: IrohaPeerWireMessageV1.Header? = null
    private var headerBytes: ByteArray? = null
    private var streamId: ByteArray? = null
    private var total = 0L
    private var bitmap = ByteArray(0)
    private var received = 0L
    private var path: Path? = null
    private var file: RandomAccessFile? = null

    @Synchronized
    fun accept(frame: IrohaPeerQRFrameV2): IrohaPeerWireMessageV1? = when (frame.kind) {
        IrohaPeerQRFrameKindV2.HEADER -> {
            acceptHeader(frame)
            null
        }
        IrohaPeerQRFrameKindV2.DATA -> acceptData(frame)
    }

    @Synchronized
    override fun close() {
        runCatching { file?.close() }
        file = null
        path?.let { runCatching { Files.deleteIfExists(it) } }
        path = null
        inspectedHeader = null
        headerBytes?.fill(0)
        headerBytes = null
        streamId?.fill(0)
        streamId = null
        total = 0
        bitmap.fill(0)
        bitmap = ByteArray(0)
        received = 0
    }

    private fun acceptHeader(frame: IrohaPeerQRFrameV2) {
        val bytes = frame.payload
        val inspected = try {
            IrohaPeerWireMessageV1.decodeHeader(bytes, limits)
        } catch (failure: RuntimeException) {
            throw IllegalArgumentException("Invalid IQR2 IPM1 header", failure)
        }
        require(inspected.profile == IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND &&
            inspected.kind == IrohaPeerPayloadKind.PAYMENT &&
            inspected.schemaVersion ==
            IrohaPeerWireMessageV1.KAGEMUSHA_ELIGIBILITY_PAYMENT_SCHEMA_VERSION &&
            inspected.streamId.contentEquals(frame.streamId) &&
            frame.total == (inspected.encodedLength.toLong() + IrohaPeerQRV2.SHARD_BYTES - 1) /
            IrohaPeerQRV2.SHARD_BYTES
        ) { "IQR2 header does not identify eligibility PAYMENT 0x0103" }
        headerBytes?.let { existing ->
            require(existing.contentEquals(bytes) &&
                checkNotNull(streamId).contentEquals(frame.streamId) && total == frame.total
            ) { "Conflicting IQR2 header duplicate" }
            return
        }
        require(Files.isDirectory(directory)) { "IQR2 assembly directory is unavailable" }
        val created = Files.createTempFile(directory, "iroha-iqr2-", ".part")
        try {
            runCatching {
                Files.setPosixFilePermissions(
                    created,
                    setOf(PosixFilePermission.OWNER_READ, PosixFilePermission.OWNER_WRITE),
                )
            }
            val opened = RandomAccessFile(created.toFile(), "rw")
            opened.setLength(inspected.encodedLength.toLong())
            inspectedHeader = inspected
            headerBytes = bytes.copyOf()
            streamId = frame.streamId
            total = frame.total
            bitmap = ByteArray(((frame.total + 7) / 8).toInt())
            path = created
            file = opened
        } catch (failure: Throwable) {
            runCatching { Files.deleteIfExists(created) }
            throw failure
        }
    }

    private fun acceptData(frame: IrohaPeerQRFrameV2): IrohaPeerWireMessageV1? {
        val inspected = inspectedHeader
            ?: throw IllegalStateException("IQR2 header must be accepted first")
        val expectedStream = checkNotNull(streamId)
        val opened = checkNotNull(file)
        require(frame.streamId.contentEquals(expectedStream) && frame.total == total &&
            frame.index in 0 until total
        ) { "Conflicting IQR2 stream" }
        val offset = Math.multiplyExact(frame.index, IrohaPeerQRV2.SHARD_BYTES.toLong())
        val expected = minOf(
            IrohaPeerQRV2.SHARD_BYTES.toLong(),
            inspected.encodedLength.toLong() - offset,
        ).toInt()
        val payload = frame.payload
        require(expected > 0 && payload.size == expected) { "Malformed IQR2 data shard" }
        val byteIndex = (frame.index / 8).toInt()
        val mask = 1 shl (frame.index % 8).toInt()
        if ((bitmap[byteIndex].toInt() and mask) != 0) {
            val existing = ByteArray(expected)
            opened.seek(offset)
            opened.readFully(existing)
            require(existing.contentEquals(payload)) { "Conflicting IQR2 data duplicate" }
            existing.fill(0)
            payload.fill(0)
            return null
        }
        opened.seek(offset)
        opened.write(payload)
        payload.fill(0)
        bitmap[byteIndex] = (bitmap[byteIndex].toInt() or mask).toByte()
        received += 1
        if (received != total) return null

        opened.fd.sync()
        opened.close()
        file = null
        val body = Files.readAllBytes(checkNotNull(path))
        val prefix = checkNotNull(headerBytes)
        val encoded = prefix + body
        return try {
            val message = IrohaPeerWireMessageV1.decode(
                encoded,
                IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
                IrohaPeerPayloadKind.PAYMENT,
                limits,
            )
            require(message.canonicalPayload.schemaVersion ==
                IrohaPeerWireMessageV1.KAGEMUSHA_ELIGIBILITY_PAYMENT_SCHEMA_VERSION
            ) { "IQR2 completed a non-eligibility message" }
            close()
            message
        } finally {
            body.fill(0)
            encoded.fill(0)
        }
    }
}

/** Each physical rail is independently fail-closed. */
enum class IrohaPeerEligibilityTransportRailV1 {
    QR_IQR2,
    NFC,
    NEARBY,
}

data class IrohaPeerEligibilityTransportReadinessV1(
    val qrIqr2Ready: Boolean = false,
    val nfcReady: Boolean = false,
    val nearbyReady: Boolean = false,
) {
    fun isReady(rail: IrohaPeerEligibilityTransportRailV1): Boolean = when (rail) {
        IrohaPeerEligibilityTransportRailV1.QR_IQR2 -> qrIqr2Ready
        IrohaPeerEligibilityTransportRailV1.NFC -> nfcReady
        IrohaPeerEligibilityTransportRailV1.NEARBY -> nearbyReady
    }
}

private val iqr2Alphabet =
    "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ \$%*+-./:".toByteArray(Charsets.US_ASCII)
private val iqr2Reverse = IntArray(128) { -1 }.also { table ->
    iqr2Alphabet.forEachIndexed { index, byte -> table[byte.toInt()] = index }
}

private fun iqr2Base45Encode(data: ByteArray): String {
    val output = ByteArray((data.size / 2) * 3 + (data.size % 2) * 2)
    var source = 0
    var target = 0
    while (source + 1 < data.size) {
        var value = (data[source].toInt() and 0xff) * 256 + (data[source + 1].toInt() and 0xff)
        output[target++] = iqr2Alphabet[value % 45]
        value /= 45
        output[target++] = iqr2Alphabet[value % 45]
        output[target++] = iqr2Alphabet[value / 45]
        source += 2
    }
    if (source < data.size) {
        val value = data[source].toInt() and 0xff
        output[target++] = iqr2Alphabet[value % 45]
        output[target] = iqr2Alphabet[value / 45]
    }
    return output.toString(Charsets.US_ASCII)
}

private fun iqr2Base45Decode(value: String): ByteArray? {
    val input = value.toByteArray(Charsets.US_ASCII)
    if (input.isEmpty() || input.size % 3 == 1 || input.size != value.length) return null
    val output = ByteArray((input.size / 3) * 2 + if (input.size % 3 == 2) 1 else 0)
    fun digit(byte: Byte): Int? {
        val code = byte.toInt() and 0xff
        return if (code < iqr2Reverse.size && iqr2Reverse[code] >= 0) iqr2Reverse[code] else null
    }
    var source = 0
    var target = 0
    while (source + 2 < input.size) {
        val decoded = (digit(input[source]) ?: return null) +
            (digit(input[source + 1]) ?: return null) * 45 +
            (digit(input[source + 2]) ?: return null) * 2025
        if (decoded > 0xffff) return null
        output[target++] = (decoded / 256).toByte()
        output[target++] = decoded.toByte()
        source += 3
    }
    if (source < input.size) {
        val decoded = (digit(input[source]) ?: return null) +
            (digit(input[source + 1]) ?: return null) * 45
        if (decoded > 0xff) return null
        output[target] = decoded.toByte()
    }
    return output
}

private fun iqr2Crc32c(value: ByteArray, start: Int, endExclusive: Int): Long {
    var crc = -1
    for (index in start until endExclusive) {
        crc = crc xor (value[index].toInt() and 0xff)
        repeat(8) {
            crc = if ((crc and 1) == 0) crc ushr 1 else
                (crc ushr 1) xor 0x82f63b78.toInt()
        }
    }
    return (crc xor -1).toLong() and 0xffff_ffffL
}

private fun ByteArray.iqr2WriteU16(offset: Int, value: Int) {
    this[offset] = (value ushr 8).toByte()
    this[offset + 1] = value.toByte()
}

private fun ByteArray.iqr2WriteU32(offset: Int, value: Long) {
    require(value in 0..0xffff_ffffL)
    this[offset] = (value ushr 24).toByte()
    this[offset + 1] = (value ushr 16).toByte()
    this[offset + 2] = (value ushr 8).toByte()
    this[offset + 3] = value.toByte()
}

private fun ByteArray.iqr2ReadU16(offset: Int): Int =
    ((this[offset].toInt() and 0xff) shl 8) or (this[offset + 1].toInt() and 0xff)

private fun ByteArray.iqr2ReadU32(offset: Int): Long =
    ((this[offset].toLong() and 0xff) shl 24) or
        ((this[offset + 1].toLong() and 0xff) shl 16) or
        ((this[offset + 2].toLong() and 0xff) shl 8) or
        (this[offset + 3].toLong() and 0xff)
