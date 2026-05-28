package org.hyperledger.iroha.sdk.offline

import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.util.Base64
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser

/** Offline Note payload kind used by NFC APDUs and Nearby envelopes. */
enum class OfflineNoteNfcPayloadKind(val code: Int) {
    RECEIVE_REQUEST(1),
    PAYMENT_TOKEN(2),
    RECEIPT_ACK(3);

    fun qrPayloadKind(): OfflineQrStream.PayloadKind =
        when (this) {
            RECEIVE_REQUEST -> OfflineQrStream.PayloadKind.OFFLINE_RECEIVE_REQUEST
            PAYMENT_TOKEN -> OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN
            RECEIPT_ACK -> OfflineQrStream.PayloadKind.OFFLINE_RECEIPT_ACK
        }

    companion object {
        @JvmStatic
        fun fromCode(code: Int): OfflineNoteNfcPayloadKind? =
            values().firstOrNull { it.code == code }
    }
}

/** NFC metadata header returned by the get-info APDU. */
class OfflineNoteNfcPayloadInfo(
    val kind: OfflineNoteNfcPayloadKind,
    val payloadLength: Int,
    val maxChunkLength: Int,
    sha256: ByteArray,
) {
    private val _sha256 = sha256.copyOf()

    fun sha256(): ByteArray = _sha256.copyOf()

    override fun equals(other: Any?): Boolean =
        other is OfflineNoteNfcPayloadInfo &&
            kind == other.kind &&
            payloadLength == other.payloadLength &&
            maxChunkLength == other.maxChunkLength &&
            _sha256.contentEquals(other._sha256)

    override fun hashCode(): Int {
        var result = kind.hashCode()
        result = 31 * result + payloadLength
        result = 31 * result + maxChunkLength
        result = 31 * result + _sha256.contentHashCode()
        return result
    }
}

/** Parsed NFC APDU command. */
sealed class OfflineNoteNfcCommand {
    object Select : OfflineNoteNfcCommand()
    object GetInfo : OfflineNoteNfcCommand()
    data class ReadChunk(val offset: Int, val requestedLength: Int) : OfflineNoteNfcCommand()
    data class WriteMeta(
        val kind: OfflineNoteNfcPayloadKind,
        val payloadLength: Int,
        val sha256: ByteArray,
    ) : OfflineNoteNfcCommand() {
        override fun equals(other: Any?): Boolean =
            other is WriteMeta &&
                kind == other.kind &&
                payloadLength == other.payloadLength &&
                sha256.contentEquals(other.sha256)

        override fun hashCode(): Int {
            var result = kind.hashCode()
            result = 31 * result + payloadLength
            result = 31 * result + sha256.contentHashCode()
            return result
        }
    }

    data class WriteChunk(val offset: Int, val bytes: ByteArray) : OfflineNoteNfcCommand() {
        override fun equals(other: Any?): Boolean =
            other is WriteChunk && offset == other.offset && bytes.contentEquals(other.bytes)

        override fun hashCode(): Int = 31 * offset + bytes.contentHashCode()
    }

    object Commit : OfflineNoteNfcCommand()
    object Unsupported : OfflineNoteNfcCommand()
    object Invalid : OfflineNoteNfcCommand()
}

/** Platform-neutral NFC APDU datastream used by Android HCE/IsoDep and iOS CardSession integrations. */
object OfflineNoteNfcApduProtocol {
    @JvmField
    val AID: ByteArray = byteArrayOf(0xF0.toByte(), 0x49, 0x52, 0x4F, 0x48, 0x41, 0x32)

    const val AID_HEX: String = "F049524F484132"
    const val ANDROID_SAFE_CHUNK_BYTES: Int = 240
    const val MAX_EXTENDED_READ_CHUNK_BYTES: Int = 1024
    const val MAX_EXTENDED_WRITE_CHUNK_BYTES: Int = 16 * 1024
    const val MAX_INCOMING_PAYLOAD_BYTES: Int = 12 * 1024

    private const val CLA_IROHA = 0x80
    private const val INS_GET_INFO = 0x10
    private const val INS_READ_CHUNK = 0x11
    private const val INS_WRITE_META = 0x20
    private const val INS_WRITE_CHUNK = 0x21
    private const val INS_COMMIT = 0x22

    @JvmField val STATUS_SUCCESS: ByteArray = byteArrayOf(0x90.toByte(), 0x00)
    @JvmField val STATUS_WRONG_DATA: ByteArray = byteArrayOf(0x6A.toByte(), 0x80.toByte())
    @JvmField val STATUS_NOT_FOUND: ByteArray = byteArrayOf(0x6A.toByte(), 0x82.toByte())
    @JvmField val STATUS_CONDITIONS_NOT_SATISFIED: ByteArray = byteArrayOf(0x69.toByte(), 0x85.toByte())
    @JvmField val STATUS_UNSUPPORTED: ByteArray = byteArrayOf(0x6D.toByte(), 0x00)

    @JvmStatic
    fun selectAidApdu(): ByteArray =
        byteArrayOf(0x00, 0xA4.toByte(), 0x04, 0x00, AID.size.toByte()) + AID + byteArrayOf(0x00)

    @JvmStatic
    fun getInfoApdu(): ByteArray = byteArrayOf(CLA_IROHA.toByte(), INS_GET_INFO.toByte(), 0x00, 0x00, 0x00)

    @JvmStatic
    @JvmOverloads
    fun readChunkApdu(offset: Int, length: Int = ANDROID_SAFE_CHUNK_BYTES): ByteArray {
        requireValidOffset(offset)
        requireChunkLength(length, MAX_EXTENDED_READ_CHUNK_BYTES)
        return if (length <= 0xff) {
            byteArrayOf(
                CLA_IROHA.toByte(),
                INS_READ_CHUNK.toByte(),
                ((offset ushr 8) and 0xff).toByte(),
                (offset and 0xff).toByte(),
                length.toByte(),
            )
        } else {
            byteArrayOf(
                CLA_IROHA.toByte(),
                INS_READ_CHUNK.toByte(),
                ((offset ushr 8) and 0xff).toByte(),
                (offset and 0xff).toByte(),
                0x00,
                ((length ushr 8) and 0xff).toByte(),
                (length and 0xff).toByte(),
            )
        }
    }

    @JvmStatic
    fun writeMetaApdu(kind: OfflineNoteNfcPayloadKind, payloadBytes: ByteArray): ByteArray {
        requirePayloadLength(payloadBytes.size)
        val meta = byteArrayOf(kind.code.toByte()) +
            int32(payloadBytes.size) +
            sha256(payloadBytes)
        return byteArrayOf(CLA_IROHA.toByte(), INS_WRITE_META.toByte(), 0x00, 0x00, meta.size.toByte()) + meta
    }

    @JvmStatic
    fun writeChunkApdu(offset: Int, bytes: ByteArray): ByteArray =
        writeChunkApdu(offset, bytes, 0, bytes.size)

    @JvmStatic
    fun writeChunkApdu(offset: Int, bytes: ByteArray, startIndex: Int, endIndex: Int): ByteArray {
        requireValidOffset(offset)
        require(startIndex >= 0 && startIndex <= bytes.size) { "startIndex out of bounds" }
        require(endIndex >= startIndex && endIndex <= bytes.size) { "endIndex out of bounds" }
        val length = endIndex - startIndex
        requireChunkLength(length, MAX_EXTENDED_WRITE_CHUNK_BYTES)
        val headerLength = if (length <= 0xff) 5 else 7
        val apdu = ByteArray(headerLength + length)
        appendCommandHeader(apdu, INS_WRITE_CHUNK, offset, length)
        bytes.copyInto(apdu, destinationOffset = headerLength, startIndex = startIndex, endIndex = endIndex)
        return apdu
    }

    @JvmStatic
    fun commitApdu(): ByteArray = byteArrayOf(CLA_IROHA.toByte(), INS_COMMIT.toByte(), 0x00, 0x00, 0x00)

    @JvmStatic
    @JvmOverloads
    fun writePayloadApdus(
        kind: OfflineNoteNfcPayloadKind,
        payloadBytes: ByteArray,
        maxChunkLength: Int = ANDROID_SAFE_CHUNK_BYTES,
    ): List<ByteArray> {
        requirePayloadLength(payloadBytes.size)
        requireChunkLength(maxChunkLength, MAX_EXTENDED_WRITE_CHUNK_BYTES)
        val apdus = ArrayList<ByteArray>()
        apdus.add(writeMetaApdu(kind, payloadBytes))
        var offset = 0
        while (offset < payloadBytes.size) {
            val end = minOf(offset + maxChunkLength, payloadBytes.size)
            apdus.add(writeChunkApdu(offset, payloadBytes, offset, end))
            offset = end
        }
        apdus.add(commitApdu())
        return apdus
    }

    @JvmStatic
    @JvmOverloads
    fun readPayloadApdus(payloadLength: Int, maxChunkLength: Int = ANDROID_SAFE_CHUNK_BYTES): List<ByteArray> {
        requirePayloadLength(payloadLength)
        requireChunkLength(maxChunkLength, MAX_EXTENDED_READ_CHUNK_BYTES)
        val apdus = ArrayList<ByteArray>()
        var offset = 0
        while (offset < payloadLength) {
            apdus.add(readChunkApdu(offset, maxChunkLength))
            offset += maxChunkLength
        }
        return apdus
    }

    @JvmStatic
    fun parseCommand(apdu: ByteArray?): OfflineNoteNfcCommand {
        if (apdu == null || apdu.size < 4) return OfflineNoteNfcCommand.Invalid
        if (isSelectAid(apdu)) return OfflineNoteNfcCommand.Select
        if ((apdu[0].toInt() and 0xff) != CLA_IROHA) return OfflineNoteNfcCommand.Unsupported
        val ins = apdu[1].toInt() and 0xff
        val offset = ((apdu[2].toInt() and 0xff) shl 8) or (apdu[3].toInt() and 0xff)
        return when (ins) {
            INS_GET_INFO -> if (offset == 0 && isNoDataApdu(apdu)) {
                OfflineNoteNfcCommand.GetInfo
            } else {
                OfflineNoteNfcCommand.Invalid
            }
            INS_READ_CHUNK -> if (isReadChunkApdu(apdu)) {
                OfflineNoteNfcCommand.ReadChunk(offset, requestedReadChunkLength(apdu))
            } else {
                OfflineNoteNfcCommand.Invalid
            }
            INS_WRITE_META -> {
                if (offset != 0) return OfflineNoteNfcCommand.Invalid
                parseWriteMeta(commandData(apdu) ?: return OfflineNoteNfcCommand.Invalid)
            }
            INS_WRITE_CHUNK -> {
                val data = commandData(apdu) ?: return OfflineNoteNfcCommand.Invalid
                if (data.isEmpty() || data.size > MAX_EXTENDED_WRITE_CHUNK_BYTES) {
                    OfflineNoteNfcCommand.Invalid
                } else {
                    OfflineNoteNfcCommand.WriteChunk(offset, data)
                }
            }
            INS_COMMIT -> if (offset == 0 && isNoDataApdu(apdu)) {
                OfflineNoteNfcCommand.Commit
            } else {
                OfflineNoteNfcCommand.Invalid
            }
            else -> OfflineNoteNfcCommand.Unsupported
        }
    }

    @JvmStatic
    @JvmOverloads
    fun encodeInfo(
        kind: OfflineNoteNfcPayloadKind,
        payloadBytes: ByteArray,
        maxChunkLength: Int = ANDROID_SAFE_CHUNK_BYTES,
    ): ByteArray {
        requirePayloadLength(payloadBytes.size)
        requireChunkLength(maxChunkLength, MAX_EXTENDED_READ_CHUNK_BYTES)
        return byteArrayOf(kind.code.toByte()) +
            int32(payloadBytes.size) +
            uint16(maxChunkLength) +
            sha256(payloadBytes)
    }

    @JvmStatic
    fun decodeInfo(data: ByteArray): OfflineNoteNfcPayloadInfo? {
        if (data.size != 39) return null
        val kind = OfflineNoteNfcPayloadKind.fromCode(data[0].toInt() and 0xff) ?: return null
        val length = readInt32(data, 1)
        val chunkLength = readUInt16(data, 5)
        if (length <= 0 ||
            length > MAX_INCOMING_PAYLOAD_BYTES ||
            chunkLength <= 0 ||
            chunkLength > MAX_EXTENDED_READ_CHUNK_BYTES
        ) {
            return null
        }
        return OfflineNoteNfcPayloadInfo(kind, length, chunkLength, data.copyOfRange(7, 39))
    }

    @JvmStatic
    @JvmOverloads
    fun response(data: ByteArray = ByteArray(0)): ByteArray = response(data, 0, data.size)

    @JvmStatic
    fun response(data: ByteArray, offset: Int, length: Int): ByteArray {
        require(offset >= 0 && offset <= data.size) { "offset out of bounds" }
        require(length >= 0 && offset + length <= data.size) { "length out of bounds" }
        val response = ByteArray(length + STATUS_SUCCESS.size)
        data.copyInto(response, destinationOffset = 0, startIndex = offset, endIndex = offset + length)
        STATUS_SUCCESS.copyInto(response, destinationOffset = length)
        return response
    }

    @JvmStatic
    fun responseStatus(response: ByteArray): Int {
        if (response.size < 2) return -1
        return ((response[response.lastIndex - 1].toInt() and 0xff) shl 8) or
            (response[response.lastIndex].toInt() and 0xff)
    }

    @JvmStatic
    fun responseData(response: ByteArray): ByteArray {
        if (response.size < 2) return ByteArray(0)
        return response.copyOfRange(0, response.size - 2)
    }

    @JvmStatic
    fun sha256(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(bytes)

    @JvmStatic
    fun payloadDigestMatches(payloadBytes: ByteArray, expectedSha256: ByteArray): Boolean =
        sha256(payloadBytes).contentEquals(expectedSha256)

    @JvmStatic
    fun requestedReadChunkLength(apdu: ByteArray): Int {
        if (apdu.size < 5 ||
            (apdu[0].toInt() and 0xff) != CLA_IROHA ||
            (apdu[1].toInt() and 0xff) != INS_READ_CHUNK
        ) {
            return ANDROID_SAFE_CHUNK_BYTES
        }
        val length = apdu[4].toInt() and 0xff
        if (length == 0 && apdu.size >= 7) {
            val extendedLength = ((apdu[5].toInt() and 0xff) shl 8) or (apdu[6].toInt() and 0xff)
            return extendedLength.coerceIn(1, MAX_EXTENDED_READ_CHUNK_BYTES)
        }
        return length.coerceIn(1, ANDROID_SAFE_CHUNK_BYTES)
    }

    @JvmStatic
    fun iosFastWriteChunkLength(peerSupportsExtendedChunks: Boolean): Int =
        if (peerSupportsExtendedChunks) MAX_EXTENDED_WRITE_CHUNK_BYTES else ANDROID_SAFE_CHUNK_BYTES

    private fun isSelectAid(apdu: ByteArray): Boolean {
        if (apdu.size < 5) return false
        if ((apdu[0].toInt() and 0xff) != 0x00) return false
        if ((apdu[1].toInt() and 0xff) != 0xA4) return false
        if ((apdu[2].toInt() and 0xff) != 0x04) return false
        if ((apdu[3].toInt() and 0xff) != 0x00) return false
        val length = apdu[4].toInt() and 0xff
        val payloadEnd = 5 + length
        if (apdu.size != payloadEnd && apdu.size != payloadEnd + 1) return false
        if (apdu.size == payloadEnd + 1 && (apdu[payloadEnd].toInt() and 0xff) != 0x00) return false
        return apdu.copyOfRange(5, payloadEnd).contentEquals(AID)
    }

    private fun commandData(apdu: ByteArray): ByteArray? {
        if (apdu.size == 4) return ByteArray(0)
        if (apdu.size < 5) return null
        val length = apdu[4].toInt() and 0xff
        if (length == 0) {
            if (apdu.size == 5) return ByteArray(0)
            if (apdu.size < 7) return null
            val extendedLength = ((apdu[5].toInt() and 0xff) shl 8) or (apdu[6].toInt() and 0xff)
            if (extendedLength <= 0 || apdu.size != 7 + extendedLength) return null
            return apdu.copyOfRange(7, 7 + extendedLength)
        }
        if (apdu.size != 5 + length) return null
        return apdu.copyOfRange(5, 5 + length)
    }

    private fun isNoDataApdu(apdu: ByteArray): Boolean =
        apdu.size == 4 || (apdu.size == 5 && (apdu[4].toInt() and 0xff) == 0)

    private fun isReadChunkApdu(apdu: ByteArray): Boolean {
        if (apdu.size == 4) return true
        if (apdu.size == 5) return (apdu[4].toInt() and 0xff) != 0
        if (apdu.size != 7 || (apdu[4].toInt() and 0xff) != 0) return false
        val extendedLength = ((apdu[5].toInt() and 0xff) shl 8) or (apdu[6].toInt() and 0xff)
        return extendedLength > 0 && extendedLength <= MAX_EXTENDED_READ_CHUNK_BYTES
    }

    private fun parseWriteMeta(data: ByteArray): OfflineNoteNfcCommand {
        if (data.size != 37) return OfflineNoteNfcCommand.Invalid
        val kind = OfflineNoteNfcPayloadKind.fromCode(data[0].toInt() and 0xff)
            ?: return OfflineNoteNfcCommand.Invalid
        val length = readInt32(data, 1)
        if (length <= 0 || length > MAX_INCOMING_PAYLOAD_BYTES) return OfflineNoteNfcCommand.Invalid
        return OfflineNoteNfcCommand.WriteMeta(kind, length, data.copyOfRange(5, 37))
    }

    private fun appendCommandHeader(apdu: ByteArray, instruction: Int, offset: Int, length: Int) {
        apdu[0] = CLA_IROHA.toByte()
        apdu[1] = instruction.toByte()
        apdu[2] = ((offset ushr 8) and 0xff).toByte()
        apdu[3] = (offset and 0xff).toByte()
        if (length <= 0xff) {
            apdu[4] = length.toByte()
        } else {
            apdu[4] = 0x00
            apdu[5] = ((length ushr 8) and 0xff).toByte()
            apdu[6] = (length and 0xff).toByte()
        }
    }

    private fun requireValidOffset(offset: Int) {
        require(offset in 0..0xffff) { "offset out of bounds" }
    }

    private fun requirePayloadLength(length: Int) {
        require(length in 1..MAX_INCOMING_PAYLOAD_BYTES) { "payload length out of bounds" }
    }

    private fun requireChunkLength(length: Int, maxChunkLength: Int) {
        require(length in 1..maxChunkLength) { "chunk length out of bounds" }
    }

    private fun int32(value: Int): ByteArray =
        byteArrayOf(
            ((value ushr 24) and 0xff).toByte(),
            ((value ushr 16) and 0xff).toByte(),
            ((value ushr 8) and 0xff).toByte(),
            (value and 0xff).toByte(),
        )

    private fun uint16(value: Int): ByteArray =
        byteArrayOf(((value ushr 8) and 0xff).toByte(), (value and 0xff).toByte())

    private fun readInt32(bytes: ByteArray, offset: Int): Int =
        ((bytes[offset].toInt() and 0xff) shl 24) or
            ((bytes[offset + 1].toInt() and 0xff) shl 16) or
            ((bytes[offset + 2].toInt() and 0xff) shl 8) or
            (bytes[offset + 3].toInt() and 0xff)

    private fun readUInt16(bytes: ByteArray, offset: Int): Int =
        ((bytes[offset].toInt() and 0xff) shl 8) or (bytes[offset + 1].toInt() and 0xff)
}

/** Incrementally validates APDU write chunks before exposing a completed NFC payload. */
class OfflineNoteNfcPayloadAssembler constructor(
    val kind: OfflineNoteNfcPayloadKind,
    val expectedLength: Int,
    expectedSha256: ByteArray,
) {
    constructor(info: OfflineNoteNfcPayloadInfo) : this(info.kind, info.payloadLength, info.sha256())

    private val _expectedSha256 = expectedSha256.copyOf()
    private val bytes: ByteArray
    private val written: BooleanArray
    private var writtenCount = 0

    init {
        require(expectedLength in 1..OfflineNoteNfcApduProtocol.MAX_INCOMING_PAYLOAD_BYTES) {
            "payload length out of bounds"
        }
        require(expectedSha256.size == 32) { "sha256 must be 32 bytes" }
        bytes = ByteArray(expectedLength)
        written = BooleanArray(expectedLength)
    }

    fun expectedSha256(): ByteArray = _expectedSha256.copyOf()

    fun isComplete(): Boolean = writtenCount == expectedLength

    fun write(offset: Int, chunk: ByteArray): Boolean {
        if (offset < 0 || offset > expectedLength || chunk.isEmpty()) return false
        if (chunk.size > OfflineNoteNfcApduProtocol.MAX_EXTENDED_WRITE_CHUNK_BYTES) return false
        if (chunk.size > expectedLength - offset) return false
        val end = offset + chunk.size
        for (index in chunk.indices) {
            val writeIndex = offset + index
            if (written[writeIndex] && bytes[writeIndex] != chunk[index]) return false
        }
        chunk.copyInto(bytes, destinationOffset = offset)
        for (index in offset until end) {
            if (!written[index]) {
                written[index] = true
                writtenCount += 1
            }
        }
        return true
    }

    fun commit(): ByteArray {
        require(isComplete()) { "payload is incomplete" }
        require(OfflineNoteNfcApduProtocol.payloadDigestMatches(bytes, _expectedSha256)) {
            "payload checksum mismatch"
        }
        return bytes.copyOf()
    }
}

enum class OfflineNoteNearbyMessageKind(val wireName: String) {
    RECEIVE_REQUEST("receive_request"),
    PAYMENT("payment"),
    RECEIPT_ACK("receipt_ack"),
    REJECTED("rejected");

    companion object {
        fun fromWireName(value: String): OfflineNoteNearbyMessageKind? =
            values().firstOrNull { it.wireName == value }
    }
}

object OfflineNoteNearbyTransportPolicy {
    const val RECEIPT_ACK_DISCONNECT_GRACE_MILLIS: Long = 1_500L
    const val RECEIPT_ACK_DISCONNECT_GRACE_NANOS: Long =
        RECEIPT_ACK_DISCONNECT_GRACE_MILLIS * 1_000_000L

    @JvmStatic
    fun requiresDisconnectGraceAfterSending(kind: OfflineNoteNearbyMessageKind): Boolean =
        kind == OfflineNoteNearbyMessageKind.RECEIPT_ACK

    @JvmStatic
    fun disconnectGraceMillisAfterSending(kind: OfflineNoteNearbyMessageKind): Long =
        if (requiresDisconnectGraceAfterSending(kind)) RECEIPT_ACK_DISCONNECT_GRACE_MILLIS else 0L

    @JvmStatic
    fun disconnectGraceNanosAfterSending(kind: OfflineNoteNearbyMessageKind): Long =
        if (requiresDisconnectGraceAfterSending(kind)) RECEIPT_ACK_DISCONNECT_GRACE_NANOS else 0L
}

class OfflineNoteNearbyPairingChallenge(assetName: String) {
    val assetName: String = assetName.trim()

    init {
        require(this.assetName in ASSET_NAMES) { "Unsupported nearby pairing challenge" }
    }

    override fun equals(other: Any?): Boolean =
        other is OfflineNoteNearbyPairingChallenge && assetName == other.assetName

    override fun hashCode(): Int = assetName.hashCode()

    override fun toString(): String = assetName

    companion object {
        @JvmField
        val ASSET_NAMES: List<String> =
            listOf("nearby_pairing_stars", "nearby_pairing_bird", "nearby_pairing_mask")

        @JvmField
        val ALL_CHOICES: List<OfflineNoteNearbyPairingChallenge> =
            ASSET_NAMES.map(::OfflineNoteNearbyPairingChallenge)

        @JvmStatic
        fun fromAssetName(value: String): OfflineNoteNearbyPairingChallenge =
            OfflineNoteNearbyPairingChallenge(value.trim())

        @JvmStatic
        fun random(): OfflineNoteNearbyPairingChallenge =
            ALL_CHOICES.random()
    }
}

/** JSON envelope for Nearby byte transports; apps bind it to Nearby Connections or Multipeer. */
class OfflineNoteNearbyEnvelope @JvmOverloads constructor(
    val kind: OfflineNoteNearbyMessageKind,
    payload: ByteArray,
    val contentType: String,
    val pairingChallenge: OfflineNoteNearbyPairingChallenge? = null,
) {
    private val _payload = payload.copyOf()

    init {
        validateForTransport(kind, _payload, contentType, pairingChallenge)
    }

    fun payload(): ByteArray = _payload.copyOf()

    fun textPayload(): String = String(_payload, StandardCharsets.UTF_8)

    fun requiresDisconnectGraceAfterSend(): Boolean =
        OfflineNoteNearbyTransportPolicy.requiresDisconnectGraceAfterSending(kind)

    fun recommendedDisconnectGraceMillisAfterSend(): Long =
        OfflineNoteNearbyTransportPolicy.disconnectGraceMillisAfterSending(kind)

    fun recommendedDisconnectGraceNanosAfterSend(): Long =
        OfflineNoteNearbyTransportPolicy.disconnectGraceNanosAfterSending(kind)

    fun encoded(): ByteArray {
        val map = LinkedHashMap<String, Any?>()
        map["kind"] = kind.wireName
        map["payload"] = base64UrlEncode(_payload)
        map["contentType"] = contentType
        if (pairingChallenge != null) {
            map["pairingChallenge"] = pairingChallenge.assetName
        }
        return JsonEncoder.encode(map).toByteArray(StandardCharsets.UTF_8)
    }

    fun paymentToken(): OfflineNotePaymentToken {
        require(kind == OfflineNoteNearbyMessageKind.PAYMENT) { "Nearby envelope is not a payment" }
        return when (contentType) {
            OfflineNoteTransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE ->
                OfflineNotePaymentTokenCodec.decodeNorito(_payload)
            OfflineNoteTransferHandoff.TEXT_PAYMENT_TOKEN_CONTENT_TYPE ->
                OfflineNotePaymentTokenCodec.decodeText(textPayload())
            else -> throw IllegalArgumentException("Nearby envelope content type is not a payment token")
        }
    }

    fun receiveRequest(): OfflineNoteReceiveRequest {
        require(kind == OfflineNoteNearbyMessageKind.RECEIVE_REQUEST) { "Nearby envelope is not a receive request" }
        return when (contentType) {
            OfflineNoteTransferHandoff.RECEIVE_REQUEST_CONTENT_TYPE ->
                OfflineNoteReceiveRequestCodec.decodeNorito(_payload)
            OfflineNoteTransferHandoff.TEXT_RECEIVE_REQUEST_CONTENT_TYPE ->
                OfflineNoteReceiveRequestCodec.decodeText(textPayload())
            else -> throw IllegalArgumentException("Nearby envelope content type is not a receive request")
        }
    }

    fun receiptAck(): OfflineNoteReceiptAck {
        require(kind == OfflineNoteNearbyMessageKind.RECEIPT_ACK) { "Nearby envelope is not a receipt ACK" }
        return when (contentType) {
            OfflineNoteTransferHandoff.RECEIPT_ACK_CONTENT_TYPE ->
                OfflineNoteReceiptAckCodec.decodeNorito(_payload)
            OfflineNoteTransferHandoff.TEXT_RECEIPT_ACK_CONTENT_TYPE ->
                OfflineNoteReceiptAckCodec.decodeText(textPayload())
            else -> throw IllegalArgumentException("Nearby envelope content type is not a receipt ACK")
        }
    }

    companion object {
        @JvmStatic
        fun decode(bytes: ByteArray): OfflineNoteNearbyEnvelope {
            val parsed = try {
                JsonParser.parse(bytes.toString(StandardCharsets.UTF_8))
            } catch (ex: RuntimeException) {
                throw IllegalArgumentException("Invalid nearby envelope JSON", ex)
            }
            val root = parsed as? Map<*, *> ?: throw IllegalArgumentException("Nearby envelope must be a JSON object")
            val allowedKeys = setOf("kind", "payload", "contentType", "pairingChallenge")
            require(root.keys.all { it is String && it in allowedKeys }) { "Nearby envelope contains unknown fields" }
            val kind = OfflineNoteNearbyMessageKind.fromWireName(root["kind"] as? String ?: "")
                ?: throw IllegalArgumentException("Nearby envelope kind is invalid")
            val payload = base64UrlDecode(root["payload"] as? String ?: "")
                ?: throw IllegalArgumentException("Nearby envelope payload is invalid")
            val contentType = root["contentType"] as? String
                ?: throw IllegalArgumentException("Nearby envelope content type is missing")
            val pairingChallenge = decodePairingChallenge(root["pairingChallenge"])
            return OfflineNoteNearbyEnvelope(kind, payload, contentType, pairingChallenge)
        }

        private fun validateForTransport(
            kind: OfflineNoteNearbyMessageKind,
            payload: ByteArray,
            contentType: String,
            pairingChallenge: OfflineNoteNearbyPairingChallenge?,
        ) {
            require(payload.isNotEmpty()) { "Nearby envelope payload is blank" }
            require(payload.size <= OfflineNoteNfcApduProtocol.MAX_INCOMING_PAYLOAD_BYTES) {
                "Nearby envelope payload is too large"
            }
            require(contentType.trim().isNotEmpty()) { "Nearby envelope content type is blank" }
            when (kind) {
                OfflineNoteNearbyMessageKind.RECEIVE_REQUEST -> {
                    require(pairingChallenge != null) { "Receive request envelope requires pairing challenge" }
                    require(
                        contentType == OfflineNoteTransferHandoff.RECEIVE_REQUEST_CONTENT_TYPE ||
                            contentType == OfflineNoteTransferHandoff.TEXT_RECEIVE_REQUEST_CONTENT_TYPE,
                    ) {
                        "Receive request envelope content type mismatch"
                    }
                    validateReceiveRequestPayload(payload, contentType)
                }
                OfflineNoteNearbyMessageKind.PAYMENT -> {
                    require(pairingChallenge == null) { "Payment envelope must not include pairing challenge" }
                    require(
                        contentType == OfflineNoteTransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE ||
                            contentType == OfflineNoteTransferHandoff.TEXT_PAYMENT_TOKEN_CONTENT_TYPE,
                    ) {
                        "Payment envelope content type mismatch"
                    }
                    validatePaymentPayload(payload, contentType)
                }
                OfflineNoteNearbyMessageKind.RECEIPT_ACK -> {
                    require(pairingChallenge == null) { "Envelope must not include pairing challenge" }
                    require(
                        contentType == OfflineNoteTransferHandoff.RECEIPT_ACK_CONTENT_TYPE ||
                            contentType == OfflineNoteTransferHandoff.TEXT_RECEIPT_ACK_CONTENT_TYPE,
                    ) {
                        "Receipt ACK envelope content type mismatch"
                    }
                    validateReceiptAckPayload(payload, contentType)
                }
                OfflineNoteNearbyMessageKind.REJECTED ->
                    require(pairingChallenge == null) { "Envelope must not include pairing challenge" }
            }
        }

        private fun decodePairingChallenge(value: Any?): OfflineNoteNearbyPairingChallenge? {
            if (value == null) return null
            if (value is String) return OfflineNoteNearbyPairingChallenge.fromAssetName(value)
            if (value is Map<*, *>) {
                require(value.keys.all { it is String && it == "assetName" }) {
                    "Nearby pairing challenge contains unknown fields"
                }
                val assetName = value["assetName"] as? String
                    ?: throw IllegalArgumentException("Nearby pairing challenge asset name is missing")
                return OfflineNoteNearbyPairingChallenge.fromAssetName(assetName)
            }
            throw IllegalArgumentException("Nearby pairing challenge is invalid")
        }

        private fun validateReceiveRequestPayload(payload: ByteArray, contentType: String) {
            try {
                if (contentType == OfflineNoteTransferHandoff.RECEIVE_REQUEST_CONTENT_TYPE) {
                    OfflineNoteReceiveRequestCodec.decodeNorito(payload)
                } else {
                    OfflineNoteReceiveRequestCodec.decodeText(String(payload, StandardCharsets.UTF_8))
                }
            } catch (ex: RuntimeException) {
                throw IllegalArgumentException("Receive request envelope payload is invalid", ex)
            }
        }

        private fun validatePaymentPayload(payload: ByteArray, contentType: String) {
            try {
                if (contentType == OfflineNoteTransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE) {
                    OfflineNotePaymentTokenCodec.decodeNorito(payload)
                } else {
                    OfflineNotePaymentTokenCodec.decodeText(String(payload, StandardCharsets.UTF_8))
                }
            } catch (ex: RuntimeException) {
                throw IllegalArgumentException("Payment envelope payload is invalid", ex)
            }
        }

        private fun validateReceiptAckPayload(payload: ByteArray, contentType: String) {
            try {
                if (contentType == OfflineNoteTransferHandoff.RECEIPT_ACK_CONTENT_TYPE) {
                    OfflineNoteReceiptAckCodec.decodeNorito(payload)
                } else {
                    OfflineNoteReceiptAckCodec.decodeText(String(payload, StandardCharsets.UTF_8))
                }
            } catch (ex: RuntimeException) {
                throw IllegalArgumentException("Receipt ACK envelope payload is invalid", ex)
            }
        }

        private fun base64UrlEncode(bytes: ByteArray): String =
            Base64.getUrlEncoder().withoutPadding().encodeToString(bytes)

        private fun base64UrlDecode(value: String): ByteArray? {
            if (value.isBlank() || value.contains("=")) return null
            if (!value.all { it in 'A'..'Z' || it in 'a'..'z' || it in '0'..'9' || it == '-' || it == '_' }) {
                return null
            }
            return try {
                Base64.getUrlDecoder().decode(value)
            } catch (_: IllegalArgumentException) {
                null
            }
        }
    }
}
