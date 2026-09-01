package org.hyperledger.iroha.sdk.offline

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
    /** Maximum frame count for one stream, including its header. */
    const val MAXIMUM_STREAM_FRAMES = 4096

    const val MAXIMUM_FRAME_TEXT_BYTES =
        6 + ((KagemushaQrFrame.MAXIMUM_ENCODED_BYTES * 4 + 2) / 3)

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
            val frames = ArrayList<KagemushaQrFrame>(
                1 + envelope.dataChunks + envelope.parityChunks,
            )
            val chunks = ArrayList<ByteArray>(envelope.dataChunks)
            try {
                val headerBytes = envelope.encode()
                try {
                    frames += KagemushaQrFrame(
                        KagemushaQrFrameKind.HEADER,
                        streamId,
                        0,
                        1,
                        headerBytes,
                    )
                } finally {
                    headerBytes.fill(0)
                }
                chunks += archive.toListOfChunks(options.chunkSize)
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
                    try {
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
                    } finally {
                        parity.fill(0)
                    }
                }
                return frames.map { frame ->
                    val encoded = frame.encode()
                    try {
                        KagemushaPeerTransportContract.QR_STREAM_TEXT_PREFIX +
                            KagemushaPeerTextCodec.base64UrlEncode(encoded)
                    } finally {
                        encoded.fill(0)
                    }
                }
            } finally {
                chunks.forEach { it.fill(0) }
                frames.forEach { it.zeroize() }
                streamId.fill(0)
                envelope.zeroize()
            }
        } finally {
            archive.fill(0)
        }
    }

    internal fun preflightStreamFrameCount(
        payloadBytes: Int,
        options: KagemushaQrStreamOptions,
    ): Int {
        require(payloadBytes in 1..KagemushaPeerTransportContract.MAXIMUM_ARCHIVE_BYTES) {
            "Kagemusha QR payload exceeds its bound"
        }
        val dataChunks = (payloadBytes + options.chunkSize - 1) / options.chunkSize
        val parityChunks = (dataChunks + options.parityGroup - 1) / options.parityGroup
        val frameCount = 1 + dataChunks + parityChunks
        require(frameCount <= MAXIMUM_STREAM_FRAMES) {
            "Kagemusha QR stream requires $frameCount frames; the limit is $MAXIMUM_STREAM_FRAMES"
        }
        return frameCount
    }

    @JvmStatic
    fun decodeFrameText(value: String): KagemushaQrFrame {
        val prefix = KagemushaPeerTransportContract.QR_STREAM_TEXT_PREFIX
        require(value.length <= MAXIMUM_FRAME_TEXT_BYTES &&
            value.startsWith(prefix) &&
            value.all { it.code <= 0x7f }
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
    private var parityFrames = linkedMapOf<Int, ByteArray>()
    private var recovered = linkedSetOf<Int>()
    private var completedPayload: KagemushaPeerPayload? = null

    @Synchronized
    fun reset() {
        resetState()
    }

    private fun resetState() {
        clearMap(dataFrames)
        clearMap(parityFrames)
        streamId?.fill(0)
        envelope?.zeroize()
        streamId = null
        envelope = null
        dataFrames = linkedMapOf()
        parityFrames = linkedMapOf()
        recovered = linkedSetOf()
        completedPayload = null
    }

    @Synchronized
    fun ingest(frameText: String): KagemushaQrDecodeResult {
        val frame = KagemushaQrStreamCodec.decodeFrameText(frameText)
        try {
            return ingest(frame)
        } finally {
            frame.zeroize()
        }
    }

    private fun ingest(frame: KagemushaQrFrame): KagemushaQrDecodeResult {
        val header = envelope
        if (header == null) {
            require(frame.kind == KagemushaQrFrameKind.HEADER) {
                "Kagemusha QR header must be ingested first"
            }
            val headerBytes = frame.payload
            val decoded = try {
                KagemushaQrEnvelope.decode(headerBytes)
            } finally {
                headerBytes.fill(0)
            }
            val frameStreamId = frame.streamId
            val decodedStreamId = decoded.streamId
            var retained = false
            try {
                require(decodedStreamId.contentEquals(frameStreamId)) {
                    "Kagemusha QR digest mismatch"
                }
                streamId = frameStreamId.copyOf()
                envelope = decoded
                retained = true
                return result()
            } finally {
                decodedStreamId.fill(0)
                frameStreamId.fill(0)
                if (!retained) decoded.zeroize()
            }
        }

        val frameStreamId = frame.streamId
        try {
            require(streamId?.contentEquals(frameStreamId) == true) {
                "Kagemusha QR frame belongs to another stream"
            }
        } finally {
            frameStreamId.fill(0)
        }
        when (frame.kind) {
            KagemushaQrFrameKind.HEADER -> {
                val headerBytes = frame.payload
                val decoded = try {
                    KagemushaQrEnvelope.decode(headerBytes)
                } finally {
                    headerBytes.fill(0)
                }
                try {
                    val decodedStreamId = decoded.streamId
                    val expectedStreamId = frame.streamId
                    try {
                        require(decodedStreamId.contentEquals(expectedStreamId)) {
                            "Kagemusha QR digest mismatch"
                        }
                    } finally {
                        decodedStreamId.fill(0)
                        expectedStreamId.fill(0)
                    }
                    require(header == decoded) { "Conflicting Kagemusha QR header" }
                } finally {
                    decoded.zeroize()
                }
            }
            KagemushaQrFrameKind.DATA -> ingestData(frame, header)
            KagemushaQrFrameKind.PARITY -> ingestParity(frame, header)
        }
        return result()
    }

    private fun ingestData(frame: KagemushaQrFrame, header: KagemushaQrEnvelope) {
        require(frame.total == header.dataChunks && frame.index in 0 until header.dataChunks) {
            "Kagemusha QR data frame count is invalid"
        }
        val payload = frame.payload
        if (payload.size != header.expectedDataChunkLength(frame.index)) {
            payload.fill(0)
            throw IllegalArgumentException("Kagemusha QR data frame does not match its header")
        }
        val existing = dataFrames[frame.index]
        if (existing != null) {
            val matches = existing.contentEquals(payload)
            payload.fill(0)
            require(matches) { "Conflicting duplicate Kagemusha QR frame" }
            return
        }
        ingestNewFrame(frame, dataFrames, payload, frame.index / header.parityGroup, header)
    }

    private fun ingestParity(frame: KagemushaQrFrame, header: KagemushaQrEnvelope) {
        require(frame.total == header.parityChunks && frame.index in 0 until header.parityChunks) {
            "Kagemusha QR parity frame count is invalid"
        }
        val payload = frame.payload
        if (payload.size != header.chunkSize) {
            payload.fill(0)
            throw IllegalArgumentException("Kagemusha QR parity frame does not match its header")
        }
        val existing = parityFrames[frame.index]
        if (existing != null) {
            val matches = existing.contentEquals(payload)
            payload.fill(0)
            require(matches) { "Conflicting duplicate Kagemusha QR frame" }
            return
        }
        ingestNewFrame(frame, parityFrames, payload, frame.index, header)
    }

    private fun ingestNewFrame(
        frame: KagemushaQrFrame,
        frames: MutableMap<Int, ByteArray>,
        payload: ByteArray,
        parityGroup: Int,
        header: KagemushaQrEnvelope,
    ) {
        frames[frame.index] = payload
        var recoveredIndex: Int? = null
        try {
            recoveredIndex = recoverGroup(header, parityGroup)
        } catch (failure: RuntimeException) {
            recoveredIndex?.let { index ->
                dataFrames.remove(index)?.fill(0)
                recovered.remove(index)
            }
            frames.remove(frame.index)?.fill(0)
            throw failure
        }
        if (completedPayload != null || dataFrames.size != header.dataChunks) return
        try {
            completedPayload = finalizeComplete(header)
        } catch (failure: RuntimeException) {
            // Exact coverage consumes a failing stream so another final-frame
            // retry cannot repeat the whole allocation/hash/decode operation.
            resetState()
            throw failure
        }
    }

    private fun recoverGroup(header: KagemushaQrEnvelope, group: Int): Int? {
        val parity = parityFrames[group] ?: return null
        val start = group * header.parityGroup
        val end = minOf(start + header.parityGroup, header.dataChunks)
        val missing = (start until end).filter { dataFrames[it] == null }
        if (missing.size != 1) return null
        val missingIndex = missing.single()
        val chunk = parity.copyOf()
        for (index in start until end) {
            if (index == missingIndex) continue
            val present = dataFrames[index] ?: throw IllegalArgumentException("Incomplete parity group")
            present.indices.forEach { byteIndex ->
                chunk[byteIndex] = (chunk[byteIndex].toInt() xor present[byteIndex].toInt()).toByte()
            }
        }
        val exact = chunk.copyOf(header.expectedDataChunkLength(missingIndex))
        chunk.fill(0)
        dataFrames[missingIndex] = exact
        recovered += missingIndex
        return missingIndex
    }

    private fun finalizeComplete(header: KagemushaQrEnvelope): KagemushaPeerPayload {
        require(dataFrames.size == header.dataChunks) { "Kagemusha QR archive is incomplete" }
        val archive = ByteArray(header.totalBytes)
        try {
            var offset = 0
            repeat(header.dataChunks) { index ->
                val chunk = dataFrames[index]
                    ?: throw IllegalArgumentException("Kagemusha QR archive is incomplete")
                chunk.copyInto(archive, offset)
                offset += chunk.size
            }
            require(offset == header.totalBytes) { "Kagemusha QR archive size mismatch" }
            val digest = sha256(archive)
            try {
                require(header.matchesPayloadDigest(digest)) {
                    "Kagemusha QR digest mismatch"
                }
            } finally {
                digest.fill(0)
            }
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
    private val digest: ByteArray
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
        require(1L + dataChunks.toLong() + parityChunks.toLong() <=
            KagemushaQrStreamCodec.MAXIMUM_STREAM_FRAMES.toLong())
        require(payloadDigest.size == 32 && payloadDigest.any { it.toInt() != 0 })
        digest = payloadDigest.copyOf()
    }

    fun expectedDataChunkLength(index: Int): Int =
        if (index == dataChunks - 1) totalBytes - index * chunkSize else chunkSize

    fun encode(): ByteArray = ByteArray(ENCODED_LENGTH).also { out ->
        out[0] = VERSION.toByte()
        out[1] = payloadKind.code.toByte()
        out[2] = parityGroup.toByte()
        out[3] = 0
        out.writeU16(4, chunkSize)
        out.writeU32(6, dataChunks.toLong())
        out.writeU32(10, parityChunks.toLong())
        out.writeU32(14, totalBytes.toLong())
        digest.copyInto(out, 18)
    }

    internal fun zeroize() {
        digest.fill(0)
    }

    internal fun matchesPayloadDigest(candidate: ByteArray): Boolean =
        digest.contentEquals(candidate)

    override fun equals(other: Any?): Boolean = other is KagemushaQrEnvelope &&
        payloadKind == other.payloadKind && parityGroup == other.parityGroup &&
        chunkSize == other.chunkSize && dataChunks == other.dataChunks &&
        parityChunks == other.parityChunks && totalBytes == other.totalBytes &&
        digest.contentEquals(other.digest)

    override fun hashCode(): Int = 31 * payloadKind.hashCode() + digest.contentHashCode()

    companion object {
        const val VERSION = 1
        const val ENCODED_LENGTH = 50

        fun create(
            kind: KagemushaPeerPayloadKind,
            payload: ByteArray,
            options: KagemushaQrStreamOptions,
        ): KagemushaQrEnvelope {
            require(payload.isNotEmpty() && payload.size <= KagemushaPeerTransportContract.MAXIMUM_ARCHIVE_BYTES)
            KagemushaQrStreamCodec.preflightStreamFrameCount(payload.size, options)
            val dataChunks = (payload.size + options.chunkSize - 1) / options.chunkSize
            val digest = sha256(payload)
            try {
                return KagemushaQrEnvelope(
                    kind,
                    options.parityGroup,
                    options.chunkSize,
                    dataChunks,
                    (dataChunks + options.parityGroup - 1) / options.parityGroup,
                    payload.size,
                    digest,
                )
            } finally {
                digest.fill(0)
            }
        }

        fun decode(data: ByteArray): KagemushaQrEnvelope {
            require(data.size == ENCODED_LENGTH && data[0].toInt() == VERSION && data[3].toInt() == 0) {
                "Invalid Kagemusha QR header"
            }
            val kind = KagemushaPeerPayloadKind.fromCode(data[1].toInt() and 0xff)
                ?: throw IllegalArgumentException("Invalid Kagemusha QR payload kind")
            val digest = data.copyOfRange(18, 50)
            try {
                return KagemushaQrEnvelope(
                    kind,
                    data[2].toInt() and 0xff,
                    data.readU16(4),
                    data.readU32Int(6),
                    data.readU32Int(10),
                    data.readU32Int(14),
                    digest,
                )
            } finally {
                digest.fill(0)
            }
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
    private val stream: ByteArray
    private val bytes: ByteArray
    val streamId: ByteArray get() = stream.copyOf()
    val payload: ByteArray get() = bytes.copyOf()

    init {
        require(streamId.size == 16 && streamId.any { it.toInt() != 0 })
        require(total in 1 until KagemushaQrStreamCodec.MAXIMUM_STREAM_FRAMES)
        require(index in 0 until total)
        require(payload.isNotEmpty() && payload.size <= KagemushaQrStreamOptions.MAXIMUM_CHUNK_SIZE)
        when (kind) {
            KagemushaQrFrameKind.HEADER ->
                require(index == 0 && total == 1 && payload.size == KagemushaQrEnvelope.ENCODED_LENGTH)
            KagemushaQrFrameKind.DATA, KagemushaQrFrameKind.PARITY -> Unit
        }
        stream = streamId.copyOf()
        bytes = payload.copyOf()
    }

    fun encode(): ByteArray {
        val payloadEnd = 30 + bytes.size
        return ByteArray(payloadEnd + 4).also { out ->
            out[0] = 0x4b
            out[1] = 0x51
            out[2] = VERSION.toByte()
            out[3] = kind.code.toByte()
            stream.copyInto(out, 4)
            out.writeU32(20, index.toLong())
            out.writeU32(24, total.toLong())
            out.writeU16(28, bytes.size)
            bytes.copyInto(out, 30)
            out.writeU32(payloadEnd, crc32(out, 2, payloadEnd))
        }
    }

    internal fun zeroize() {
        stream.fill(0)
        bytes.fill(0)
    }

    override fun equals(other: Any?): Boolean = other is KagemushaQrFrame &&
        kind == other.kind && index == other.index && total == other.total &&
        stream.contentEquals(other.stream) && bytes.contentEquals(other.bytes)

    override fun hashCode(): Int = 31 * (31 * kind.hashCode() + stream.contentHashCode()) +
        bytes.contentHashCode()

    companion object {
        const val VERSION = 1
        const val FIXED_OVERHEAD = 34
        const val MAXIMUM_ENCODED_BYTES = FIXED_OVERHEAD + KagemushaQrStreamOptions.MAXIMUM_CHUNK_SIZE
        fun decode(data: ByteArray): KagemushaQrFrame {
            require(data.size in FIXED_OVERHEAD..MAXIMUM_ENCODED_BYTES &&
                data[0] == 0x4b.toByte() && data[1] == 0x51.toByte() && data[2].toInt() == VERSION
            ) { "Malformed Kagemusha QR frame" }
            val kind = KagemushaQrFrameKind.entries.firstOrNull {
                it.code == (data[3].toInt() and 0xff)
            } ?: throw IllegalArgumentException("Malformed Kagemusha QR frame kind")
            val payloadLength = data.readU16(28)
            val payloadEnd = 30 + payloadLength
            require(payloadEnd + 4 == data.size) { "Malformed Kagemusha QR frame length" }
            require(data.readU32(payloadEnd) == crc32(data, 2, payloadEnd)) {
                "Kagemusha QR frame checksum mismatch"
            }
            val streamId = data.copyOfRange(4, 20)
            val payload = data.copyOfRange(30, payloadEnd)
            try {
                return KagemushaQrFrame(
                    kind,
                    streamId,
                    data.readU32Int(20),
                    data.readU32Int(24),
                    payload,
                )
            } finally {
                streamId.fill(0)
                payload.fill(0)
            }
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

private fun ByteArray.readU32Int(offset: Int): Int {
    val value = readU32(offset)
    require(value <= Int.MAX_VALUE.toLong()) { "Kagemusha QR count exceeds the SDK limit" }
    return value.toInt()
}

private fun sha256(value: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(value)

private fun crc32(value: ByteArray, start: Int, endExclusive: Int): Long {
    val crc = CRC32()
    crc.update(value, start, endExclusive - start)
    return crc.value
}

private fun clearMap(source: Map<Int, ByteArray>) = source.values.forEach { it.fill(0) }
