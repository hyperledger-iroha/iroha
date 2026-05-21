package org.hyperledger.iroha.sdk.offline

import java.util.zip.CRC32
import org.hyperledger.iroha.sdk.crypto.Blake2b

/** Fountain QR V1 framing helpers for Offline V2 payload transfer. */
object OfflineQrStream {

    private val MAGIC = byteArrayOf(0x49, 0x51)
    private const val FRAME_VERSION: Byte = 1
    private const val ENVELOPE_VERSION: Byte = 1
    private const val ENCODING_BINARY: Byte = 0
    private const val ENVELOPE_LENGTH = 48

    enum class FrameKind(val value: Int) {
        HEADER(0), DATA(1), PARITY(2);
        companion object {
            @JvmStatic
            fun fromValue(value: Int): FrameKind =
                entries.firstOrNull { it.value == value }
                    ?: throw IllegalArgumentException("Unsupported frame kind: $value")
        }
    }

    enum class FrameEncoding { BINARY, BASE64 }

    enum class PayloadKind(val value: Int) {
        UNSPECIFIED(0),
        OFFLINE_RECEIVE_REQUEST_V2(1),
        OFFLINE_RECEIVE_CHALLENGE_V2(1),
        OFFLINE_PAYMENT_TOKEN_V2(2),
        OFFLINE_RECEIPT_ACK_V2(3);

        companion object {
            @JvmStatic
            fun fromValue(value: Int): PayloadKind? =
                entries.firstOrNull { it.value == value }
        }
    }

    object TextCodec {
        private const val PREFIX = "iroha:qr1:"

        @JvmStatic
        fun encode(data: ByteArray, encoding: FrameEncoding): String {
            val base64 = java.util.Base64.getEncoder().encodeToString(data)
            return if (encoding == FrameEncoding.BASE64) PREFIX + base64 else base64
        }

        @JvmStatic
        fun decode(value: String, encoding: FrameEncoding): ByteArray {
            val trimmed = value.trim()
            val payload = if (encoding == FrameEncoding.BASE64) {
                require(trimmed.startsWith(PREFIX)) { "QR text prefix missing" }
                trimmed.substring(PREFIX.length)
            } else {
                trimmed
            }
            return java.util.Base64.getDecoder().decode(payload)
        }
    }

    class Options @JvmOverloads constructor(
        val chunkSize: Int = 360,
        val parityGroup: Int = 3,
    ) {
        init {
            require(chunkSize in 1..0xFFFF) { "chunkSize must be between 1 and 65535" }
            require(parityGroup in 0..0xFF) { "parityGroup must be between 0 and 255" }
        }
    }

    class Envelope(
        val flags: Int,
        val encoding: Int,
        val parityGroup: Int,
        val chunkSize: Int,
        val dataChunks: Int,
        val parityChunks: Int,
        val payloadKind: Int,
        val payloadLength: Long,
        payloadHash: ByteArray,
    ) {
        private val _payloadHash: ByteArray

        init {
            require(payloadHash.size == 32) { "payloadHash must be 32 bytes" }
            require(flags == 0) { "Unsupported envelope flags" }
            require(encoding == ENCODING_BINARY.toInt()) { "Unsupported envelope encoding" }
            require(parityGroup in 0..0xFF) { "parityGroup must be between 0 and 255" }
            require(chunkSize in 1..0xFFFF) { "chunkSize must be between 1 and 65535" }
            require(dataChunks in 0..0xFFFF) { "dataChunks must be between 0 and 65535" }
            require(parityChunks in 0..0xFFFF) { "parityChunks must be between 0 and 65535" }
            require(payloadKind in 0..0xFFFF) { "payloadKind must be between 0 and 65535" }
            require(payloadLength in 0..0xFFFF_FFFFL) { "payloadLength must fit uint32" }
            val expectedDataChunks =
                if (payloadLength == 0L) 0L else (payloadLength + chunkSize - 1L) / chunkSize
            require(expectedDataChunks <= 0xFFFFL && dataChunks.toLong() == expectedDataChunks) {
                "dataChunks mismatch"
            }
            val expectedParityChunks =
                if (parityGroup == 0) 0 else (dataChunks + parityGroup - 1) / parityGroup
            require(parityChunks == expectedParityChunks) { "parityChunks mismatch" }
            _payloadHash = payloadHash.copyOf()
        }

        fun streamId(): ByteArray = _payloadHash.copyOf(16)
        fun payloadHash(): ByteArray = _payloadHash.copyOf()

        fun encode(): ByteArray {
            val out = ByteArray(ENVELOPE_LENGTH)
            var offset = 0
            out[offset++] = ENVELOPE_VERSION
            out[offset++] = flags.toByte()
            out[offset++] = encoding.toByte()
            out[offset++] = parityGroup.toByte()
            writeUInt16LE(out, offset, chunkSize); offset += 2
            writeUInt16LE(out, offset, dataChunks); offset += 2
            writeUInt16LE(out, offset, parityChunks); offset += 2
            writeUInt16LE(out, offset, payloadKind); offset += 2
            writeUInt32LE(out, offset, payloadLength); offset += 4
            System.arraycopy(_payloadHash, 0, out, offset, _payloadHash.size)
            return out
        }

        companion object {
            @JvmStatic
            fun decode(bytes: ByteArray): Envelope {
                require(bytes.size == ENVELOPE_LENGTH) { "Envelope length mismatch" }
                var offset = 0
                val version = bytes[offset++].toInt() and 0xFF
                require(version == ENVELOPE_VERSION.toInt()) { "Unsupported envelope version: $version" }
                val flags = bytes[offset++].toInt() and 0xFF
                val encoding = bytes[offset++].toInt() and 0xFF
                val parityGroup = bytes[offset++].toInt() and 0xFF
                val chunkSize = readUInt16LE(bytes, offset); offset += 2
                val dataChunks = readUInt16LE(bytes, offset); offset += 2
                val parityChunks = readUInt16LE(bytes, offset); offset += 2
                val payloadKind = readUInt16LE(bytes, offset); offset += 2
                val payloadLength = readUInt32LE(bytes, offset); offset += 4
                val payloadHash = bytes.copyOfRange(offset, offset + 32)
                return Envelope(flags, encoding, parityGroup, chunkSize,
                    dataChunks, parityChunks, payloadKind, payloadLength, payloadHash)
            }
        }
    }

    class Frame(
        val kind: FrameKind,
        streamId: ByteArray,
        val index: Int,
        val total: Int,
        payload: ByteArray?,
    ) {
        private val _streamId: ByteArray
        private val _payload: ByteArray

        init {
            _streamId = streamId.copyOf()
            require(_streamId.size == 16) { "streamId must be 16 bytes" }
            require(index in 0..0xFFFF) { "index must be between 0 and 65535" }
            require(total in 0..0xFFFF) { "total must be between 0 and 65535" }
            _payload = payload?.copyOf() ?: byteArrayOf()
            require(_payload.size <= 0xFFFF) { "payload length exceeds 65535" }
        }

        fun payload(): ByteArray = _payload.copyOf()
        fun streamId(): ByteArray = _streamId.copyOf()

        fun encode(): ByteArray {
            val headerLength = 2 + 1 + 1 + 16 + 2 + 2 + 2
            val out = ByteArray(headerLength + _payload.size + 4)
            var offset = 0
            out[offset++] = MAGIC[0]; out[offset++] = MAGIC[1]
            out[offset++] = FRAME_VERSION; out[offset++] = kind.value.toByte()
            System.arraycopy(_streamId, 0, out, offset, _streamId.size); offset += _streamId.size
            writeUInt16LE(out, offset, index); offset += 2
            writeUInt16LE(out, offset, total); offset += 2
            writeUInt16LE(out, offset, _payload.size); offset += 2
            System.arraycopy(_payload, 0, out, offset, _payload.size); offset += _payload.size
            val crc = crc32(out, 2, offset - 2)
            writeUInt32LE(out, offset, crc)
            return out
        }

        companion object {
            @JvmStatic
            fun decode(bytes: ByteArray): Frame {
                val headerLength = 2 + 1 + 1 + 16 + 2 + 2 + 2
                require(bytes.size >= headerLength + 4) { "Frame is too short" }
                require(bytes[0] == MAGIC[0] && bytes[1] == MAGIC[1]) { "Frame magic mismatch" }
                val version = bytes[2].toInt() and 0xFF
                require(version == FRAME_VERSION.toInt()) { "Unsupported frame version: $version" }
                val kind = FrameKind.fromValue(bytes[3].toInt() and 0xFF)
                val streamId = bytes.copyOfRange(4, 20)
                val index = readUInt16LE(bytes, 20)
                val total = readUInt16LE(bytes, 22)
                val payloadLength = readUInt16LE(bytes, 24)
                val payloadEnd = 26 + payloadLength
                require(payloadEnd + 4 == bytes.size) { "Frame payload length mismatch" }
                val payload = bytes.copyOfRange(26, payloadEnd)
                val expected = readUInt32LE(bytes, payloadEnd)
                val computed = crc32(bytes, 2, payloadEnd - 2)
                require(expected == computed) { "Frame checksum mismatch" }
                return Frame(kind, streamId, index, total, payload)
            }
        }
    }

    class DecodeResult(
        val payload: ByteArray?,
        val payloadKind: PayloadKind?,
        val receivedChunks: Int,
        val totalChunks: Int,
        val recoveredChunks: Int,
    ) {
        constructor(
            payload: ByteArray?,
            receivedChunks: Int,
            totalChunks: Int,
            recoveredChunks: Int,
        ) : this(payload, null, receivedChunks, totalChunks, recoveredChunks)

        val isComplete: Boolean get() = payload != null
        fun progress(): Double = if (totalChunks == 0) 0.0 else receivedChunks / totalChunks.toDouble()
    }

    object Encoder {
        @JvmStatic
        fun encodeFrames(payload: ByteArray): List<Frame> =
            encodeFrames(payload, PayloadKind.UNSPECIFIED, Options())

        @JvmStatic
        fun encodeFrames(
            payload: ByteArray, payloadKind: PayloadKind, options: Options,
        ): List<Frame> {
            val chunkSize = options.chunkSize
            val dataChunks =
                if (payload.isEmpty()) 0
                else Math.ceil(payload.size / chunkSize.toDouble()).toInt()
            require(dataChunks <= 0xFFFF) { "dataChunks exceeds 65535" }
            val parityGroup = options.parityGroup
            val parityChunks =
                if (parityGroup > 0) Math.ceil(dataChunks / parityGroup.toDouble()).toInt() else 0
            require(parityChunks <= 0xFFFF) { "parityChunks exceeds 65535" }
            val payloadHash = Blake2b.digest256(payload)
            val envelope = Envelope(0, ENCODING_BINARY.toInt(), parityGroup, chunkSize,
                dataChunks, parityChunks, payloadKind.value, payload.size.toLong(), payloadHash)
            val frames = ArrayList<Frame>()
            frames.add(Frame(FrameKind.HEADER, envelope.streamId(), 0, 1, envelope.encode()))
            for (index in 0 until dataChunks) {
                val start = index * chunkSize
                val end = minOf(payload.size, start + chunkSize)
                frames.add(Frame(FrameKind.DATA, envelope.streamId(), index, dataChunks,
                    payload.copyOfRange(start, end)))
            }
            if (parityGroup > 0) {
                for (groupIndex in 0 until parityChunks) {
                    val parity = xorParity(payload, chunkSize, dataChunks, groupIndex, parityGroup)
                    frames.add(Frame(FrameKind.PARITY, envelope.streamId(), groupIndex, parityChunks, parity))
                }
            }
            return frames
        }

        @JvmStatic
        fun encodeFrameBytes(
            payload: ByteArray, payloadKind: PayloadKind, options: Options,
        ): List<ByteArray> = encodeFrames(payload, payloadKind, options).map { it.encode() }
    }

    class Decoder {
        private var envelope: Envelope? = null
        private var dataChunks: Array<ByteArray?> = emptyArray()
        private var parityChunks: Array<ByteArray?> = emptyArray()
        private val pendingFrames = ArrayList<Frame>()
        private val recovered = HashSet<Int>()

        fun ingest(frameBytes: ByteArray): DecodeResult {
            val frame = Frame.decode(frameBytes)
            ingestFrame(frame)
            val payload = finalizeIfComplete()
            val payloadKind = envelope?.let { PayloadKind.fromValue(it.payloadKind) }
            val received = dataChunks.count { it != null }
            return DecodeResult(payload, payloadKind, received, dataChunks.size, recovered.size)
        }

        private fun ingestFrame(frame: Frame) {
            if (frame.kind == FrameKind.HEADER) {
                require(frame.index == 0 && frame.total == 1) { "Header frame counters mismatch" }
                val decoded = Envelope.decode(frame.payload())
                require(decoded.streamId().contentEquals(frame.streamId())) { "Stream id mismatch" }
                val env = envelope
                if (env != null && env.streamId().contentEquals(decoded.streamId())) {
                    require(env.encode().contentEquals(decoded.encode())) { "Conflicting repeated header" }
                    return
                }
                envelope = decoded
                dataChunks = arrayOfNulls(decoded.dataChunks)
                parityChunks = arrayOfNulls(decoded.parityChunks)
                if (pendingFrames.isNotEmpty()) {
                    val buffered = ArrayList(pendingFrames)
                    pendingFrames.clear()
                    for (f in buffered) {
                        if (f.streamId().contentEquals(decoded.streamId())) ingestFrame(f)
                    }
                }
                return
            }
            val env = envelope
            if (env == null) { pendingFrames.add(frame); return }
            if (!frame.streamId().contentEquals(env.streamId())) return
            if (frame.kind == FrameKind.DATA) {
                val index = frame.index
                require(frame.total == dataChunks.size) { "Data frame total mismatch" }
                require(index in dataChunks.indices) { "Data frame index out of bounds" }
                val payload = frame.payload()
                require(payload.size == expectedChunkLength(index)) { "Data chunk length mismatch" }
                val existing = dataChunks[index]
                if (existing != null) {
                    require(existing.contentEquals(payload)) { "Conflicting data chunk" }
                } else {
                    dataChunks[index] = payload
                }
            } else if (frame.kind == FrameKind.PARITY) {
                val index = frame.index
                require(frame.total == parityChunks.size) { "Parity frame total mismatch" }
                require(index in parityChunks.indices) { "Parity frame index out of bounds" }
                val payload = frame.payload()
                require(payload.size == env.chunkSize) { "Parity chunk length mismatch" }
                val existing = parityChunks[index]
                if (existing != null) {
                    require(existing.contentEquals(payload)) { "Conflicting parity chunk" }
                } else {
                    parityChunks[index] = payload
                }
            }
            recoverMissing()
        }

        private fun recoverMissing() {
            val env = envelope ?: return
            if (env.parityGroup == 0) return
            val groupSize = env.parityGroup
            val chunkSize = env.chunkSize
            for (groupIndex in parityChunks.indices) {
                val parity = parityChunks[groupIndex] ?: continue
                val startIndex = groupIndex * groupSize
                val endIndex = minOf(dataChunks.size, startIndex + groupSize)
                if (startIndex >= endIndex) continue
                var missingIndex: Int? = null
                val xor = parity.copyOf()
                for (dataIndex in startIndex until endIndex) {
                    val chunk = dataChunks[dataIndex]
                    if (chunk != null) {
                        val padded = ByteArray(chunkSize)
                        System.arraycopy(chunk, 0, padded, 0, minOf(chunk.size, chunkSize))
                        for (i in 0 until chunkSize) xor[i] = (xor[i].toInt() xor padded[i].toInt()).toByte()
                    } else if (missingIndex == null) {
                        missingIndex = dataIndex
                    } else {
                        missingIndex = null; break
                    }
                }
                if (missingIndex != null) {
                    val length = expectedChunkLength(missingIndex)
                    dataChunks[missingIndex] = xor.copyOf(length)
                    recovered.add(missingIndex)
                }
            }
        }

        private fun expectedChunkLength(index: Int): Int {
            val env = envelope ?: return 0
            val chunkSize = env.chunkSize
            val total = env.dataChunks
            if (total == 0) return 0
            if (index < total - 1) return chunkSize
            val tail = env.payloadLength - chunkSize.toLong() * (total - 1)
            return maxOf(0, minOf(chunkSize.toLong(), tail)).toInt()
        }

        private fun finalizeIfComplete(): ByteArray? {
            val env = envelope ?: return null
            for (chunk in dataChunks) { if (chunk == null) return null }
            var totalLength = 0
            for (i in dataChunks.indices) totalLength += expectedChunkLength(i)
            val payload = ByteArray(totalLength)
            var offset = 0
            for (i in dataChunks.indices) {
                val chunk = dataChunks[i]!!
                val length = expectedChunkLength(i)
                require(chunk.size == length) { "Data chunk length mismatch" }
                System.arraycopy(chunk, 0, payload, offset, length)
                offset += length
            }
            val hash = Blake2b.digest256(payload)
            require(hash.contentEquals(env.payloadHash())) { "Payload hash mismatch" }
            return payload
        }
    }

    class ScanSession {
        private val decoder = Decoder()
        fun ingest(frameBytes: ByteArray): DecodeResult = decoder.ingest(frameBytes)
    }

    class Color(@JvmField val red: Double, @JvmField val green: Double, @JvmField val blue: Double)

    class FrameStyle(
        @JvmField val petalPhase: Double,
        @JvmField val accentStrength: Double,
        @JvmField val gradientAngle: Double,
    )

    class PlaybackStyle(
        @JvmField val petalPhase: Double,
        @JvmField val accentStrength: Double,
        @JvmField val gradientAngle: Double,
        @JvmField val driftOffset: Double,
        @JvmField val progressAlpha: Double,
    )

    class Theme(
        @JvmField val name: String,
        @JvmField val backgroundStart: Color,
        @JvmField val backgroundEnd: Color,
        @JvmField val accent: Color,
        @JvmField val petal: Color,
        @JvmField val petalCount: Int,
        @JvmField val pulsePeriod: Double,
    ) {
        fun frameStyle(frameIndex: Int, totalFrames: Int): FrameStyle {
            val safeTotal = maxOf(totalFrames, 1)
            val phase = (frameIndex % safeTotal) / safeTotal.toDouble()
            val pulse = (Math.sin((frameIndex / pulsePeriod) * Math.PI * 2.0) + 1.0) / 2.0
            return FrameStyle(phase, pulse, phase * 360.0)
        }
    }

    class PlaybackSkin(
        @JvmField val name: String,
        @JvmField val theme: Theme,
        @JvmField val frameRate: Double,
        @JvmField val petalDriftSpeed: Double,
        @JvmField val progressOverlayAlpha: Double,
        @JvmField val reducedMotion: Boolean,
        @JvmField val lowPower: Boolean,
    ) {
        fun frameStyle(frameIndex: Int, totalFrames: Int, progress: Double): PlaybackStyle {
            val safeTotal = maxOf(totalFrames, 1)
            val phase = (frameIndex % safeTotal) / safeTotal.toDouble()
            val pulse = (Math.sin((frameIndex / theme.pulsePeriod) * Math.PI * 2.0) + 1.0) / 2.0
            val angle = phase * 360.0
            val drift = if (reducedMotion) 0.0 else Math.sin(phase * Math.PI * 2.0) * petalDriftSpeed
            val clamped = progress.coerceIn(0.0, 1.0)
            return PlaybackStyle(phase, pulse, angle, drift, progressOverlayAlpha * clamped)
        }
    }

    @JvmField
    val SAKURA_THEME = Theme("sakura",
        Color(0.98, 0.94, 0.96), Color(1.0, 0.98, 0.99),
        Color(0.92, 0.48, 0.6), Color(0.98, 0.8, 0.86), 6, 48.0)

    @JvmField
    val SAKURA_STORM_THEME = Theme("sakura-storm",
        Color(0.05, 0.02, 0.08), Color(0.02, 0.01, 0.04),
        Color(0.95, 0.71, 0.87), Color(0.98, 0.92, 0.97), 8, 36.0)

    @JvmField
    val SAKURA_SKIN = PlaybackSkin("sakura", SAKURA_THEME, 12.0, 1.0, 0.4, false, false)

    @JvmField
    val SAKURA_REDUCED_MOTION_SKIN = PlaybackSkin(
        "sakura-reduced-motion", SAKURA_THEME, 6.0, 0.0, 0.25, true, false)

    @JvmField
    val SAKURA_LOW_POWER_SKIN = PlaybackSkin("sakura-low-power",
        Theme("sakura-low-power", SAKURA_THEME.backgroundStart, SAKURA_THEME.backgroundEnd,
            SAKURA_THEME.accent, SAKURA_THEME.petal, 4, 72.0),
        8.0, 0.4, 0.3, false, true)

    @JvmField
    val SAKURA_STORM_SKIN = PlaybackSkin(
        "sakura-storm", SAKURA_STORM_THEME, 12.0, 0.6, 0.34, false, false)

    private fun xorParity(
        payload: ByteArray, chunkSize: Int, dataChunks: Int, groupIndex: Int, groupSize: Int,
    ): ByteArray {
        val parity = ByteArray(chunkSize)
        val startIndex = groupIndex * groupSize
        val endIndex = minOf(dataChunks, startIndex + groupSize)
        if (startIndex >= endIndex) return parity
        for (chunkIndex in startIndex until endIndex) {
            val start = chunkIndex * chunkSize
            val end = minOf(payload.size, start + chunkSize)
            for (offset in start until end) {
                parity[offset - start] = (parity[offset - start].toInt() xor payload[offset].toInt()).toByte()
            }
        }
        return parity
    }

    private fun crc32(bytes: ByteArray, offset: Int, length: Int): Long {
        val crc = CRC32()
        crc.update(bytes, offset, length)
        return crc.value
    }

    private fun writeUInt16LE(bytes: ByteArray, offset: Int, value: Int) {
        bytes[offset] = (value and 0xFF).toByte()
        bytes[offset + 1] = ((value shr 8) and 0xFF).toByte()
    }

    private fun writeUInt32LE(bytes: ByteArray, offset: Int, value: Long) {
        bytes[offset] = (value and 0xFF).toByte()
        bytes[offset + 1] = ((value shr 8) and 0xFF).toByte()
        bytes[offset + 2] = ((value shr 16) and 0xFF).toByte()
        bytes[offset + 3] = ((value shr 24) and 0xFF).toByte()
    }

    private fun readUInt16LE(bytes: ByteArray, offset: Int): Int =
        (bytes[offset].toInt() and 0xFF) or ((bytes[offset + 1].toInt() and 0xFF) shl 8)

    private fun readUInt32LE(bytes: ByteArray, offset: Int): Long =
        (bytes[offset].toLong() and 0xFF) or
            ((bytes[offset + 1].toLong() and 0xFF) shl 8) or
            ((bytes[offset + 2].toLong() and 0xFF) shl 16) or
            ((bytes[offset + 3].toLong() and 0xFF) shl 24)
}
