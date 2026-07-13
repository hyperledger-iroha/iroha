package org.hyperledger.iroha.sdk.offline

import java.io.ByteArrayOutputStream
import java.security.MessageDigest
import java.util.zip.CRC32

data class KagemushaQrStreamOptions(
    val chunkSize: Int = STANDARD_CHUNK_SIZE,
    val parityGroup: Int = STANDARD_PARITY_GROUP,
) {
    init {
        require(chunkSize in MINIMUM_CHUNK_SIZE..MAXIMUM_CHUNK_SIZE) {
            "Kagemusha QR chunk size is unsupported"
        }
        require(parityGroup in MINIMUM_PARITY_GROUP..MAXIMUM_PARITY_GROUP) {
            "Kagemusha QR parity group is unsupported"
        }
    }

    companion object {
        const val MINIMUM_CHUNK_SIZE = 64
        const val MAXIMUM_CHUNK_SIZE = 512
        const val MINIMUM_PARITY_GROUP = 2
        const val MAXIMUM_PARITY_GROUP = 16
        const val STANDARD_CHUNK_SIZE = 256
        const val STANDARD_PARITY_GROUP = 4
        @JvmField val STANDARD = KagemushaQrStreamOptions()
    }
}

data class KagemushaQrDecodeResult(
    val payload: KagemushaPeerPayload?,
    val payloadKind: KagemushaPeerPayloadKind?,
    val receivedDataFrames: Int,
    val totalDataFrames: Int,
    val recoveredDataFrames: Int,
) {
    val isComplete: Boolean get() = payload != null
    val progress: Double
        get() = if (totalDataFrames == 0) 0.0
        else minOf(1.0, receivedDataFrames.toDouble() / totalDataFrames.toDouble())
}

object KagemushaQrStreamCodec {
    const val MAXIMUM_FRAME_TEXT_BYTES = 6 + ((542 * 4 + 2) / 3)

    @JvmStatic
    @JvmOverloads
    fun encode(
        payload: KagemushaPeerPayload,
        options: KagemushaQrStreamOptions = KagemushaQrStreamOptions.STANDARD,
    ): List<String> {
        val archive = payload.archive()
        try {
            require(archive.isNotEmpty() && archive.size <= KagemushaPeerTransportContract.MAXIMUM_ARCHIVE_BYTES) {
                "Kagemusha QR payload exceeds its bound"
            }
            val envelope = KagemushaQrEnvelope.create(payload.kind, archive, options)
            val streamId = envelope.streamId
            val frames = ArrayList<KagemushaQrFrame>()
            frames += KagemushaQrFrame(
                KagemushaQrFrameKind.HEADER,
                streamId,
                0,
                1,
                envelope.encode(),
            )
            val chunks = archive.toListOfChunks(options.chunkSize)
            chunks.forEachIndexed { index, chunk ->
                frames += KagemushaQrFrame(
                    KagemushaQrFrameKind.DATA,
                    streamId,
                    index,
                    chunks.size,
                    chunk,
                )
            }
            repeat(envelope.parityChunks) { group ->
                val parity = ByteArray(options.chunkSize)
                val start = group * options.parityGroup
                val end = minOf(start + options.parityGroup, chunks.size)
                for (chunkIndex in start until end) {
                    chunks[chunkIndex].indices.forEach { byteIndex ->
                        parity[byteIndex] = (parity[byteIndex].toInt() xor
                            chunks[chunkIndex][byteIndex].toInt()).toByte()
                    }
                }
                frames += KagemushaQrFrame(
                    KagemushaQrFrameKind.PARITY,
                    streamId,
                    group,
                    envelope.parityChunks,
                    parity,
                )
            }
            return frames.map { frame ->
                KagemushaPeerTransportContract.QR_STREAM_TEXT_PREFIX +
                    KagemushaPeerTextCodec.base64UrlEncode(frame.encode())
            }
        } finally {
            archive.fill(0)
        }
    }

    @JvmStatic
    fun decodeFrameText(value: String): KagemushaQrFrame {
        val prefix = KagemushaPeerTransportContract.QR_STREAM_TEXT_PREFIX
        require(value.toByteArray(Charsets.UTF_8).size <= MAXIMUM_FRAME_TEXT_BYTES &&
            value.startsWith(prefix)
        ) { "Kagemusha QR frame is not canonical" }
        val body = value.substring(prefix.length)
        val bytes = KagemushaPeerTextCodec.base64UrlDecode(body)
            ?: throw IllegalArgumentException("Kagemusha QR frame is not canonical Base64URL")
        try {
            require(prefix + KagemushaPeerTextCodec.base64UrlEncode(bytes) == value) {
                "Kagemusha QR frame is not canonical"
            }
            return KagemushaQrFrame.decode(bytes)
        } finally {
            bytes.fill(0)
        }
    }
}

class KagemushaQrStreamDecoder {
    private var streamId: ByteArray? = null
    private var envelope: KagemushaQrEnvelope? = null
    private var dataFrames = linkedMapOf<Int, ByteArray>()
    private var dataTotals = linkedMapOf<Int, Int>()
    private var parityFrames = linkedMapOf<Int, ByteArray>()
    private var parityTotals = linkedMapOf<Int, Int>()
    private var recovered = linkedSetOf<Int>()
    private var completedPayload: KagemushaPeerPayload? = null

    @Synchronized
    fun reset() {
        clearMap(dataFrames)
        clearMap(parityFrames)
        streamId?.fill(0)
        streamId = null
        envelope = null
        dataFrames = linkedMapOf()
        dataTotals = linkedMapOf()
        parityFrames = linkedMapOf()
        parityTotals = linkedMapOf()
        recovered = linkedSetOf()
        completedPayload = null
    }

    @Synchronized
    fun ingest(frameText: String): KagemushaQrDecodeResult {
        val frame = KagemushaQrStreamCodec.decodeFrameText(frameText)
        val snapshot = snapshot()
        try {
            return ingest(frame).also { snapshot.clearCopies() }
        } catch (failure: RuntimeException) {
            restore(snapshot)
            throw failure
        }
    }

    private fun ingest(frame: KagemushaQrFrame): KagemushaQrDecodeResult {
        streamId?.let { require(it.contentEquals(frame.streamId)) { "Kagemusha QR frame belongs to another stream" } }
            ?: run { streamId = frame.streamId.copyOf() }
        when (frame.kind) {
            KagemushaQrFrameKind.HEADER -> {
                val decoded = KagemushaQrEnvelope.decode(frame.payload)
                require(decoded.streamId.contentEquals(frame.streamId)) { "Kagemusha QR digest mismatch" }
                envelope?.let { require(it == decoded) { "Conflicting Kagemusha QR header" } }
                envelope = decoded
            }
            KagemushaQrFrameKind.DATA -> {
                require(frame.total <= MAXIMUM_DATA_FRAMES) { "Kagemusha QR data frame count is invalid" }
                store(frame, dataFrames, dataTotals)
            }
            KagemushaQrFrameKind.PARITY -> {
                require(frame.total <= MAXIMUM_PARITY_FRAMES) { "Kagemusha QR parity frame count is invalid" }
                store(frame, parityFrames, parityTotals)
            }
        }
        envelope?.let { header ->
            validateBuffered(header)
            recover(header)
            if (completedPayload == null) completedPayload = finalize(header)
        }
        return result()
    }

    private fun store(
        frame: KagemushaQrFrame,
        frames: MutableMap<Int, ByteArray>,
        totals: MutableMap<Int, Int>,
    ) {
        require(frame.index in 0 until frame.total) { "Kagemusha QR frame index is invalid" }
        frames[frame.index]?.let { previous ->
            require(previous.contentEquals(frame.payload) && totals[frame.index] == frame.total) {
                "Conflicting duplicate Kagemusha QR frame"
            }
        } ?: run {
            frames[frame.index] = frame.payload.copyOf()
            totals[frame.index] = frame.total
        }
    }

    private fun validateBuffered(header: KagemushaQrEnvelope) {
        dataFrames.forEach { (index, payload) ->
            require(index < header.dataChunks && dataTotals[index] == header.dataChunks &&
                payload.size == header.expectedDataChunkLength(index)
            ) { "Kagemusha QR data frame does not match its header" }
        }
        parityFrames.forEach { (index, payload) ->
            require(index < header.parityChunks && parityTotals[index] == header.parityChunks &&
                payload.size == header.chunkSize
            ) { "Kagemusha QR parity frame does not match its header" }
        }
    }

    private fun recover(header: KagemushaQrEnvelope) {
        repeat(header.parityChunks) { group ->
            val parity = parityFrames[group] ?: return@repeat
            val start = group * header.parityGroup
            val end = minOf(start + header.parityGroup, header.dataChunks)
            val missing = (start until end).filter { dataFrames[it] == null }
            if (missing.size != 1) return@repeat
            val chunk = parity.copyOf()
            for (index in start until end) {
                if (index == missing.single()) continue
                val present = dataFrames[index] ?: throw IllegalArgumentException("Incomplete parity group")
                present.indices.forEach { byteIndex ->
                    chunk[byteIndex] = (chunk[byteIndex].toInt() xor present[byteIndex].toInt()).toByte()
                }
            }
            val exact = chunk.copyOf(header.expectedDataChunkLength(missing.single()))
            chunk.fill(0)
            dataFrames[missing.single()] = exact
            dataTotals[missing.single()] = header.dataChunks
            recovered += missing.single()
        }
    }

    private fun finalize(header: KagemushaQrEnvelope): KagemushaPeerPayload? {
        if (dataFrames.size != header.dataChunks) return null
        val archive = ByteArrayOutputStream(header.totalBytes).use { output ->
            repeat(header.dataChunks) { index ->
                output.write(dataFrames[index] ?: return null)
            }
            output.toByteArray()
        }
        try {
            require(archive.size == header.totalBytes) { "Kagemusha QR archive size mismatch" }
            require(sha256(archive).contentEquals(header.payloadDigest)) { "Kagemusha QR digest mismatch" }
            return KagemushaPeerPayload.decode(archive, header.payloadKind)
        } finally {
            archive.fill(0)
        }
    }

    private fun result() = KagemushaQrDecodeResult(
        completedPayload,
        envelope?.payloadKind,
        dataFrames.size,
        envelope?.dataChunks ?: 0,
        recovered.size,
    )

    private data class Snapshot(
        val streamId: ByteArray?,
        val envelope: KagemushaQrEnvelope?,
        val dataFrames: LinkedHashMap<Int, ByteArray>,
        val dataTotals: LinkedHashMap<Int, Int>,
        val parityFrames: LinkedHashMap<Int, ByteArray>,
        val parityTotals: LinkedHashMap<Int, Int>,
        val recovered: LinkedHashSet<Int>,
        val completedPayload: KagemushaPeerPayload?,
    ) {
        fun clearCopies() {
            streamId?.fill(0)
            clearMap(dataFrames)
            clearMap(parityFrames)
        }
    }

    private fun snapshot() = Snapshot(
        streamId?.copyOf(),
        envelope,
        copyMap(dataFrames),
        LinkedHashMap(dataTotals),
        copyMap(parityFrames),
        LinkedHashMap(parityTotals),
        LinkedHashSet(recovered),
        completedPayload,
    )

    private fun restore(snapshot: Snapshot) {
        clearMap(dataFrames)
        clearMap(parityFrames)
        streamId?.fill(0)
        streamId = snapshot.streamId
        envelope = snapshot.envelope
        dataFrames = snapshot.dataFrames
        dataTotals = snapshot.dataTotals
        parityFrames = snapshot.parityFrames
        parityTotals = snapshot.parityTotals
        recovered = snapshot.recovered
        completedPayload = snapshot.completedPayload
    }

    private companion object {
        const val MAXIMUM_DATA_FRAMES =
            (KagemushaPeerTransportContract.MAXIMUM_ARCHIVE_BYTES +
                KagemushaQrStreamOptions.MINIMUM_CHUNK_SIZE - 1) /
                KagemushaQrStreamOptions.MINIMUM_CHUNK_SIZE
        const val MAXIMUM_PARITY_FRAMES =
            (MAXIMUM_DATA_FRAMES + KagemushaQrStreamOptions.MINIMUM_PARITY_GROUP - 1) /
                KagemushaQrStreamOptions.MINIMUM_PARITY_GROUP
    }
}

enum class KagemushaQrFrameKind(val code: Int) { HEADER(0), DATA(1), PARITY(2) }

class KagemushaQrEnvelope private constructor(
    val payloadKind: KagemushaPeerPayloadKind,
    val parityGroup: Int,
    val chunkSize: Int,
    val dataChunks: Int,
    val parityChunks: Int,
    val totalBytes: Int,
    payloadDigest: ByteArray,
) {
    private val digest = payloadDigest.copyOf()
    val payloadDigest: ByteArray get() = digest.copyOf()
    val streamId: ByteArray get() = digest.copyOfRange(0, 16)

    init {
        require(chunkSize in KagemushaQrStreamOptions.MINIMUM_CHUNK_SIZE..
            KagemushaQrStreamOptions.MAXIMUM_CHUNK_SIZE)
        require(parityGroup in KagemushaQrStreamOptions.MINIMUM_PARITY_GROUP..
            KagemushaQrStreamOptions.MAXIMUM_PARITY_GROUP)
        require(totalBytes in 1..KagemushaPeerTransportContract.MAXIMUM_ARCHIVE_BYTES)
        require(dataChunks == (totalBytes + chunkSize - 1) / chunkSize)
        require(parityChunks == (dataChunks + parityGroup - 1) / parityGroup)
        require(digest.size == 32 && digest.any { it.toInt() != 0 })
    }

    fun expectedDataChunkLength(index: Int): Int =
        if (index == dataChunks - 1) totalBytes - index * chunkSize else chunkSize

    fun encode(): ByteArray = ByteArray(ENCODED_LENGTH).also { out ->
        out[0] = VERSION.toByte()
        out[1] = payloadKind.code.toByte()
        out[2] = parityGroup.toByte()
        out[3] = 0
        out.writeU16(4, chunkSize)
        out.writeU16(6, dataChunks)
        out.writeU16(8, parityChunks)
        out.writeU32(10, totalBytes.toLong())
        digest.copyInto(out, 14)
    }

    override fun equals(other: Any?): Boolean = other is KagemushaQrEnvelope &&
        payloadKind == other.payloadKind && parityGroup == other.parityGroup &&
        chunkSize == other.chunkSize && dataChunks == other.dataChunks &&
        parityChunks == other.parityChunks && totalBytes == other.totalBytes &&
        digest.contentEquals(other.digest)

    override fun hashCode(): Int = 31 * payloadKind.hashCode() + digest.contentHashCode()

    companion object {
        const val VERSION = 1
        const val ENCODED_LENGTH = 46

        fun create(
            kind: KagemushaPeerPayloadKind,
            payload: ByteArray,
            options: KagemushaQrStreamOptions,
        ): KagemushaQrEnvelope {
            require(payload.isNotEmpty() && payload.size <= KagemushaPeerTransportContract.MAXIMUM_ARCHIVE_BYTES)
            val dataChunks = (payload.size + options.chunkSize - 1) / options.chunkSize
            return KagemushaQrEnvelope(
                kind,
                options.parityGroup,
                options.chunkSize,
                dataChunks,
                (dataChunks + options.parityGroup - 1) / options.parityGroup,
                payload.size,
                sha256(payload),
            )
        }

        fun decode(data: ByteArray): KagemushaQrEnvelope {
            require(data.size == ENCODED_LENGTH && data[0].toInt() == VERSION && data[3].toInt() == 0) {
                "Invalid Kagemusha QR header"
            }
            val kind = KagemushaPeerPayloadKind.fromCode(data[1].toInt() and 0xff)
                ?: throw IllegalArgumentException("Invalid Kagemusha QR payload kind")
            return KagemushaQrEnvelope(
                kind,
                data[2].toInt() and 0xff,
                data.readU16(4),
                data.readU16(6),
                data.readU16(8),
                data.readU32(10).toInt(),
                data.copyOfRange(14, 46),
            )
        }
    }
}

class KagemushaQrFrame(
    val kind: KagemushaQrFrameKind,
    streamId: ByteArray,
    val index: Int,
    val total: Int,
    payload: ByteArray,
) {
    private val stream = streamId.copyOf()
    private val bytes = payload.copyOf()
    val streamId: ByteArray get() = stream.copyOf()
    val payload: ByteArray get() = bytes.copyOf()

    init {
        require(stream.size == 16 && stream.any { it.toInt() != 0 })
        require(index in 0 until total && total in 1..0xffff)
        require(bytes.isNotEmpty() && bytes.size <= KagemushaQrStreamOptions.MAXIMUM_CHUNK_SIZE)
        if (kind == KagemushaQrFrameKind.HEADER) {
            require(index == 0 && total == 1 && bytes.size == KagemushaQrEnvelope.ENCODED_LENGTH)
        }
    }

    fun encode(): ByteArray {
        val payloadEnd = 26 + bytes.size
        return ByteArray(payloadEnd + 4).also { out ->
            out[0] = 0x4b
            out[1] = 0x51
            out[2] = VERSION.toByte()
            out[3] = kind.code.toByte()
            stream.copyInto(out, 4)
            out.writeU16(20, index)
            out.writeU16(22, total)
            out.writeU16(24, bytes.size)
            bytes.copyInto(out, 26)
            out.writeU32(payloadEnd, crc32(out, 2, payloadEnd))
        }
    }

    override fun equals(other: Any?): Boolean = other is KagemushaQrFrame &&
        kind == other.kind && index == other.index && total == other.total &&
        stream.contentEquals(other.stream) && bytes.contentEquals(other.bytes)

    override fun hashCode(): Int = 31 * (31 * kind.hashCode() + stream.contentHashCode()) +
        bytes.contentHashCode()

    companion object {
        const val VERSION = 1
        const val FIXED_OVERHEAD = 30
        const val MAXIMUM_ENCODED_BYTES = FIXED_OVERHEAD + KagemushaQrStreamOptions.MAXIMUM_CHUNK_SIZE

        fun decode(data: ByteArray): KagemushaQrFrame {
            require(data.size in FIXED_OVERHEAD..MAXIMUM_ENCODED_BYTES &&
                data[0] == 0x4b.toByte() && data[1] == 0x51.toByte() && data[2].toInt() == VERSION
            ) { "Malformed Kagemusha QR frame" }
            val kind = KagemushaQrFrameKind.entries.firstOrNull {
                it.code == (data[3].toInt() and 0xff)
            } ?: throw IllegalArgumentException("Malformed Kagemusha QR frame kind")
            val payloadLength = data.readU16(24)
            val payloadEnd = 26 + payloadLength
            require(payloadEnd + 4 == data.size) { "Malformed Kagemusha QR frame length" }
            require(data.readU32(payloadEnd) == crc32(data, 2, payloadEnd)) {
                "Kagemusha QR frame checksum mismatch"
            }
            return KagemushaQrFrame(
                kind,
                data.copyOfRange(4, 20),
                data.readU16(20),
                data.readU16(22),
                data.copyOfRange(26, payloadEnd),
            )
        }
    }
}

private fun ByteArray.toListOfChunks(size: Int): List<ByteArray> =
    (indices step size).map { start -> copyOfRange(start, minOf(start + size, this.size)) }

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

private fun sha256(value: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(value)

private fun crc32(value: ByteArray, start: Int, endExclusive: Int): Long {
    val crc = CRC32()
    crc.update(value, start, endExclusive - start)
    return crc.value
}

private fun copyMap(source: Map<Int, ByteArray>): LinkedHashMap<Int, ByteArray> =
    LinkedHashMap<Int, ByteArray>().also { output ->
        source.forEach { (key, value) -> output[key] = value.copyOf() }
    }

private fun clearMap(source: Map<Int, ByteArray>) = source.values.forEach { it.fill(0) }
