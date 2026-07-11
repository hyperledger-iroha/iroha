package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Kind of asynchronous Offline operation accepted by Torii. */
enum class OfflineOperationKind {
    TOP_UP,
    REDEEM,
}

/** State returned when an Offline operation is first accepted. */
enum class OfflineOperationState {
    PENDING,
}

/** Canonical top-up request submitted directly as a Norito archive. */
class OfflineTopUpRequest(
    noritoArchive: ByteArray,
) {
    private val canonicalRequest = requireCanonicalOfflineRequest(
        noritoArchive,
        TOP_UP_REQUEST_SCHEMA,
        operationIdFieldIndex = 6,
        fieldCount = 8,
    )

    /** Lowercase hexadecimal operation identifier embedded in the canonical request. */
    @JvmField
    val operationId: String = canonicalRequest.operationId
    private val archive = canonicalRequest.archive

    /** Returns a defensive copy of the canonical request archive. */
    fun noritoArchive(): ByteArray = archive.copyOf()
}

/** Canonical redemption request submitted directly as a Norito archive. */
class OfflineRedeemRequest(
    noritoArchive: ByteArray,
) {
    private val canonicalRequest = requireCanonicalOfflineRequest(
        noritoArchive,
        REDEEM_REQUEST_SCHEMA,
        operationIdFieldIndex = 9,
        fieldCount = 11,
    )

    /** Lowercase hexadecimal operation identifier embedded in the canonical request. */
    @JvmField
    val operationId: String = canonicalRequest.operationId
    private val archive = canonicalRequest.archive

    /** Returns a defensive copy of the canonical request archive. */
    fun noritoArchive(): ByteArray = archive.copyOf()
}

/** Reference returned after Torii accepts an asynchronous Offline operation. */
class OfflineOperationReference(
    operationId: String,
    @JvmField val kind: OfflineOperationKind,
    @JvmField val state: OfflineOperationState,
    transactionHash: String,
    statusUri: String,
    submittedAtMs: BigInteger,
) {
    @JvmField
    val operationId: String = requireOperationId(operationId)

    @JvmField
    val transactionHash: String = requireExactNonEmptyText(transactionHash, "transactionHash")

    @JvmField
    val statusUri: String = requireExactNonEmptyText(statusUri, "statusUri")

    @JvmField
    val submittedAtMs: BigInteger = requireU64(submittedAtMs, "submittedAtMs")

    override fun equals(other: Any?): Boolean =
        other is OfflineOperationReference &&
            operationId == other.operationId &&
            kind == other.kind &&
            state == other.state &&
            transactionHash == other.transactionHash &&
            statusUri == other.statusUri &&
            submittedAtMs == other.submittedAtMs

    override fun hashCode(): Int {
        var result = operationId.hashCode()
        result = 31 * result + kind.hashCode()
        result = 31 * result + state.hashCode()
        result = 31 * result + transactionHash.hashCode()
        result = 31 * result + statusUri.hashCode()
        result = 31 * result + submittedAtMs.hashCode()
        return result
    }
}

/** Norito codec for the accepted-operation reference. */
object OfflineOperationCodec {
    private const val SCHEMA = "iroha_torii_shared::offline_api::OfflineOperationReference"
    private const val STATUS_SCHEMA = "iroha_torii_shared::offline_api::OfflineOperationStatus"
    private const val TOP_UP_ANCHOR_SCHEMA =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendTopUpAnchorV2"
    private const val STATUS_HEADER_PADDING = 8

    /** Decode an accepted-operation reference returned by Torii. */
    @JvmStatic
    fun decodeReference(archive: ByteArray): OfflineOperationReference =
        NoritoCodec.decode(archive.copyOf(), ReferenceAdapter, SCHEMA)

    /** Encode an accepted-operation reference using the canonical Norito layout. */
    @JvmStatic
    fun encodeReference(reference: OfflineOperationReference): ByteArray =
        NoritoCodec.encode(reference, SCHEMA, ReferenceAdapter, NoritoHeader.COMPACT_LEN)

    /** Decode a schema-bound typed operation status returned by Torii. */
    @JvmStatic
    fun decodeStatus(archive: ByteArray): OfflineOperationStatus {
        requireStatusPadding(archive)
        return NoritoCodec.decode(archive.copyOf(), StatusAdapter, STATUS_SCHEMA)
    }

    /** Encode an operation status using the canonical public Norito layout. */
    @JvmStatic
    fun encodeStatus(status: OfflineOperationStatus): ByteArray =
        addStatusPadding(
            NoritoCodec.encode(status, STATUS_SCHEMA, StatusAdapter, NoritoHeader.COMPACT_LEN),
        )

    private object ReferenceAdapter : TypeAdapter<OfflineOperationReference> {
        override fun encode(encoder: NoritoEncoder, value: OfflineOperationReference) {
            writeField(encoder) { writeString(it, value.operationId) }
            writeField(encoder) { it.writeUInt(value.kind.ordinal.toLong(), 32) }
            writeField(encoder) { it.writeUInt(value.state.ordinal.toLong(), 32) }
            writeField(encoder) { writeString(it, value.transactionHash) }
            writeField(encoder) { writeString(it, value.statusUri) }
            writeField(encoder) { it.writeUInt(value.submittedAtMs.toLong(), 64) }
        }

        override fun decode(decoder: NoritoDecoder): OfflineOperationReference {
            val operationId = readField(decoder, ::readString)
            val kind = when (readField(decoder) { it.readUInt(32) }) {
                0L -> OfflineOperationKind.TOP_UP
                1L -> OfflineOperationKind.REDEEM
                else -> throw IllegalArgumentException("Invalid Offline operation kind")
            }
            val state = when (readField(decoder) { it.readUInt(32) }) {
                0L -> OfflineOperationState.PENDING
                else -> throw IllegalArgumentException("Invalid Offline operation state")
            }
            return OfflineOperationReference(
                operationId = operationId,
                kind = kind,
                state = state,
                transactionHash = readField(decoder, ::readString),
                statusUri = readField(decoder, ::readString),
                submittedAtMs = readField(decoder) { unsignedLongToBigInteger(it.readUInt(64)) },
            )
        }
    }

    private object StatusAdapter : TypeAdapter<OfflineOperationStatus> {
        override fun encode(encoder: NoritoEncoder, value: OfflineOperationStatus) {
            when (value) {
                is OfflineOperationStatus.Pending -> {
                    encoder.writeUInt(0, 32)
                    writeField(encoder) { child -> writeString(child, value.operationId) }
                    writeField(encoder) { child -> writeKind(child, value.kind) }
                    writeField(encoder) { child -> writeString(child, value.transactionHash) }
                    writeField(encoder) { child -> writeU64(child, value.submittedAtMs) }
                }
                is OfflineOperationStatus.Applied -> {
                    encoder.writeUInt(1, 32)
                    writeField(encoder) { child -> writeString(child, value.operationId) }
                    writeField(encoder) { child -> writeResult(child, value.result) }
                }
                is OfflineOperationStatus.Rejected -> {
                    encoder.writeUInt(2, 32)
                    writeField(encoder) { child -> writeString(child, value.operationId) }
                    writeField(encoder) { child -> writeKind(child, value.kind) }
                    writeField(encoder) { child -> writeString(child, value.transactionHash) }
                    writeField(encoder) { child -> writeError(child, value.error) }
                }
            }
        }

        override fun decode(decoder: NoritoDecoder): OfflineOperationStatus {
            return when (val tag = decoder.readUInt(32)) {
                0L -> OfflineOperationStatus.Pending(
                    operationId = readField(decoder, ::readString),
                    kind = readField(decoder, ::readKind),
                    transactionHash = readField(decoder, ::readString),
                    submittedAtMs = readField(decoder, ::readU64),
                )
                1L -> OfflineOperationStatus.Applied(
                    operationId = readField(decoder, ::readString),
                    result = readField(decoder, ::readResult),
                )
                2L -> OfflineOperationStatus.Rejected(
                    operationId = readField(decoder, ::readString),
                    kind = readField(decoder, ::readKind),
                    transactionHash = readField(decoder, ::readString),
                    error = readField(decoder, ::readError),
                )
                else -> throw IllegalArgumentException("Invalid Offline operation status tag: $tag")
            }
        }
    }

    private fun writeResult(encoder: NoritoEncoder, result: OfflineOperationStatus.Result) {
        when (result) {
            is OfflineOperationStatus.Result.TopUp -> writeVariant(encoder, 0) {
                val value = result.value
                writeField(it) { child -> writeString(child, value.transactionHash) }
                writeField(it) { child -> writeU64(child, value.finalizedBlockHeight) }
                writeField(it) { child -> writeU64(child, value.serverTimeMs) }
                writeField(it) { child ->
                    val view = NoritoCodec.fromBytesView(
                        value.anchor.noritoArchive(),
                        TOP_UP_ANCHOR_SCHEMA,
                    )
                    require(view.flags == encoder.flags) {
                        "Top-up anchor flags must match operation status flags"
                    }
                    child.writeBytes(view.asBytes())
                }
            }
            is OfflineOperationStatus.Result.Redeem -> writeVariant(encoder, 1) {
                val value = result.value
                writeField(it) { child -> writeString(child, value.transactionHash) }
                writeField(it) { child -> writeU64(child, value.finalizedBlockHeight) }
                writeField(it) { child -> writeU64(child, value.serverTimeMs) }
            }
        }
    }

    private fun readResult(decoder: NoritoDecoder): OfflineOperationStatus.Result {
        val (tag, variant) = readVariant(decoder)
        val result = when (tag) {
            0L -> {
                val transactionHash = readField(variant, ::readString)
                val finalizedHeight = readField(variant, ::readU64)
                val serverTime = readField(variant, ::readU64)
                val anchorPayload = readField(variant, ::readRemainingBytes)
                val anchorArchive = frameArchive(TOP_UP_ANCHOR_SCHEMA, anchorPayload, decoder.flags)
                OfflineOperationStatus.Result.TopUp(
                    OfflineOperationStatus.TopUpResult(
                        transactionHash,
                        finalizedHeight,
                        serverTime,
                        OfflineOperationStatus.TopUpAnchor(anchorArchive),
                    ),
                )
            }
            1L -> OfflineOperationStatus.Result.Redeem(
                OfflineOperationStatus.RedeemResult(
                    readField(variant, ::readString),
                    readField(variant, ::readU64),
                    readField(variant, ::readU64),
                ),
            )
            else -> throw IllegalArgumentException("Invalid Offline operation result tag: $tag")
        }
        require(variant.remaining() == 0) { "Trailing bytes after Offline result variant" }
        return result
    }

    private fun writeError(encoder: NoritoEncoder, error: OfflineOperationStatus.Error) {
        writeField(encoder) { writeString(it, error.code) }
        writeField(encoder) { writeString(it, error.message) }
        writeField(encoder) { writeOption(it, error.details, ::writeErrorDetails) }
    }

    private fun readError(decoder: NoritoDecoder): OfflineOperationStatus.Error =
        OfflineOperationStatus.Error(
            readField(decoder, ::readString),
            readField(decoder, ::readString),
            readField(decoder) { readOption(it, ::readErrorDetails) },
        )

    private fun writeErrorDetails(
        encoder: NoritoEncoder,
        details: OfflineOperationStatus.ErrorDetails,
    ) {
        writeField(encoder) { writeOption(it, details.layer, ::writeString) }
        writeField(encoder) { writeOption(it, details.rejectCode, ::writeString) }
        writeField(encoder) { writeOption(it, details.queue, ::writeQueue) }
        writeField(encoder) { writeOption(it, details.retryAfterSeconds, ::writeU64) }
        writeField(encoder) { writeOption(it, details.endpoint, ::writeString) }
        writeField(encoder) { writeOption(it, details.field, ::writeString) }
        writeField(encoder) { writeOption(it, details.expected, ::writeString) }
        writeField(encoder) { writeOption(it, details.actual, ::writeString) }
        writeField(encoder) { writeOption(it, details.profile, ::writeString) }
        writeField(encoder) { writeOption(it, details.chainDiscriminant) { out, value -> out.writeUInt(value.toLong(), 16) } }
        writeField(encoder) { writeOption(it, details.transactionHash, ::writeString) }
        writeField(encoder) { writeOption(it, details.lastStatus, ::writeString) }
        writeField(encoder) { writeOption(it, details.hint, ::writeString) }
        writeField(encoder) { writeOption(it, details.axt, ::writeAxt) }
    }

    private fun readErrorDetails(decoder: NoritoDecoder): OfflineOperationStatus.ErrorDetails =
        OfflineOperationStatus.ErrorDetails(
            layer = readField(decoder) { readOption(it, ::readString) },
            rejectCode = readField(decoder) { readOption(it, ::readString) },
            queue = readField(decoder) { readOption(it, ::readQueue) },
            retryAfterSeconds = readField(decoder) { readOption(it, ::readU64) },
            endpoint = readField(decoder) { readOption(it, ::readString) },
            field = readField(decoder) { readOption(it, ::readString) },
            expected = readField(decoder) { readOption(it, ::readString) },
            actual = readField(decoder) { readOption(it, ::readString) },
            profile = readField(decoder) { readOption(it, ::readString) },
            chainDiscriminant = readField(decoder) { readOption(it) { child -> child.readUInt(16).toInt() } },
            transactionHash = readField(decoder) { readOption(it, ::readString) },
            lastStatus = readField(decoder) { readOption(it, ::readString) },
            hint = readField(decoder) { readOption(it, ::readString) },
            axt = readField(decoder) { readOption(it, ::readAxt) },
        )

    private fun writeQueue(encoder: NoritoEncoder, queue: OfflineOperationStatus.QueueErrorSnapshot) {
        writeField(encoder) { writeString(it, queue.state) }
        writeField(encoder) { writeU64(it, queue.queued) }
        writeField(encoder) { writeU64(it, queue.capacity) }
        writeField(encoder) { it.writeByte(if (queue.saturated) 1 else 0) }
    }

    private fun readQueue(decoder: NoritoDecoder): OfflineOperationStatus.QueueErrorSnapshot =
        OfflineOperationStatus.QueueErrorSnapshot(
            readField(decoder, ::readString),
            readField(decoder, ::readU64),
            readField(decoder, ::readU64),
            readField(decoder, ::readBool),
        )

    private fun writeAxt(encoder: NoritoEncoder, axt: OfflineOperationStatus.AxtErrorDetails) {
        writeField(encoder) { writeOption(it, axt.code, ::writeString) }
        writeField(encoder) { writeOption(it, axt.reason, ::writeString) }
        writeField(encoder) { writeOption(it, axt.snapshotVersion, ::writeU64) }
        writeField(encoder) { writeOption(it, axt.dataspace, ::writeU64) }
        writeField(encoder) { writeOption(it, axt.lane) { out, value -> out.writeUInt(value, 32) } }
        writeField(encoder) { writeOption(it, axt.nextMinHandleEra, ::writeU64) }
        writeField(encoder) { writeOption(it, axt.nextMinSubNonce, ::writeU64) }
    }

    private fun readAxt(decoder: NoritoDecoder): OfflineOperationStatus.AxtErrorDetails =
        OfflineOperationStatus.AxtErrorDetails(
            code = readField(decoder) { readOption(it, ::readString) },
            reason = readField(decoder) { readOption(it, ::readString) },
            snapshotVersion = readField(decoder) { readOption(it, ::readU64) },
            dataspace = readField(decoder) { readOption(it, ::readU64) },
            lane = readField(decoder) { readOption(it) { child -> child.readUInt(32) } },
            nextMinHandleEra = readField(decoder) { readOption(it, ::readU64) },
            nextMinSubNonce = readField(decoder) { readOption(it, ::readU64) },
        )

    private fun writeKind(encoder: NoritoEncoder, kind: OfflineOperationKind) {
        encoder.writeUInt(kind.ordinal.toLong(), 32)
    }

    private fun readKind(decoder: NoritoDecoder): OfflineOperationKind = when (decoder.readUInt(32)) {
        0L -> OfflineOperationKind.TOP_UP
        1L -> OfflineOperationKind.REDEEM
        else -> throw IllegalArgumentException("Invalid Offline operation kind")
    }

    private fun writeU64(encoder: NoritoEncoder, value: BigInteger) {
        encoder.writeUInt(requireU64(value, "u64").toLong(), 64)
    }

    private fun readU64(decoder: NoritoDecoder): BigInteger =
        unsignedLongToBigInteger(decoder.readUInt(64))

    private fun readBool(decoder: NoritoDecoder): Boolean = when (val value = decoder.readByte()) {
        0 -> false
        1 -> true
        else -> throw IllegalArgumentException("Invalid boolean value: $value")
    }

    private fun <T> writeOption(
        encoder: NoritoEncoder,
        value: T?,
        write: (NoritoEncoder, T) -> Unit,
    ) {
        if (value == null) {
            encoder.writeByte(0)
            return
        }
        encoder.writeByte(1)
        val child = encoder.childEncoder()
        write(child, value)
        val payload = child.toByteArray()
        encoder.writeLength(payload.size.toLong(), compact(encoder.flags))
        encoder.writeBytes(payload)
    }

    private fun <T> readOption(decoder: NoritoDecoder, read: (NoritoDecoder) -> T): T? {
        return when (val tag = decoder.readByte()) {
            0 -> null
            1 -> {
                val length = decoder.readLength(compact(decoder.flags))
                require(length <= Int.MAX_VALUE) { "Offline option payload is too large" }
                val child = NoritoDecoder(decoder.readBytes(length.toInt()), decoder.flags, decoder.flagsHint)
                val value = read(child)
                require(child.remaining() == 0) { "Trailing bytes after Offline option" }
                value
            }
            else -> throw IllegalArgumentException("Invalid Offline option tag: $tag")
        }
    }

    private fun writeVariant(
        encoder: NoritoEncoder,
        tag: Long,
        write: (NoritoEncoder) -> Unit,
    ) {
        encoder.writeUInt(tag, 32)
        val child = encoder.childEncoder()
        write(child)
        val payload = child.toByteArray()
        encoder.writeLength(payload.size.toLong(), compact(encoder.flags))
        encoder.writeBytes(payload)
    }

    private fun readVariant(decoder: NoritoDecoder): Pair<Long, NoritoDecoder> {
        val tag = decoder.readUInt(32)
        val length = decoder.readLength(
            decoder.flags and NoritoHeader.COMPACT_LEN != 0,
        )
        require(length <= Int.MAX_VALUE) { "Offline variant payload is too large" }
        return tag to NoritoDecoder(
            decoder.readBytes(length.toInt()),
            decoder.flags,
            decoder.flagsHint,
        )
    }

    private fun frameArchive(schema: String, payload: ByteArray, flags: Int): ByteArray {
        val header = NoritoHeader(
            SchemaHash.hash16(schema),
            payload.size,
            CRC64.compute(payload),
            flags,
            NoritoHeader.COMPRESSION_NONE,
        ).encode()
        return header + payload
    }

    private fun readRemainingBytes(decoder: NoritoDecoder): ByteArray =
        decoder.readBytes(decoder.remaining())

    private fun compact(flags: Int): Boolean = (flags and NoritoHeader.COMPACT_LEN) != 0

    private fun requireStatusPadding(archive: ByteArray) {
        val decoded = NoritoHeader.decode(archive, SchemaHash.hash16(STATUS_SCHEMA))
        val padding = archive.size - NoritoHeader.HEADER_LENGTH - decoded.header.payloadLength
        require(padding == STATUS_HEADER_PADDING) {
            "Offline operation status must contain canonical 8-byte enum alignment padding"
        }
    }

    private fun addStatusPadding(archive: ByteArray): ByteArray {
        val padded = ByteArray(archive.size + STATUS_HEADER_PADDING)
        archive.copyInto(padded, endIndex = NoritoHeader.HEADER_LENGTH)
        archive.copyInto(
            padded,
            destinationOffset = NoritoHeader.HEADER_LENGTH + STATUS_HEADER_PADDING,
            startIndex = NoritoHeader.HEADER_LENGTH,
        )
        return padded
    }
}

private const val TOP_UP_REQUEST_SCHEMA =
    "iroha_data_model::offline::model::KagemushaRecursiveSpendTopUpRequestV2"
private const val REDEEM_REQUEST_SCHEMA =
    "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV2"
private val LOWER_HEX = "0123456789abcdef".toCharArray()

private class CanonicalOfflineRequest(
    val operationId: String,
    val archive: ByteArray,
)

private fun requireCanonicalOfflineRequest(
    value: ByteArray,
    schema: String,
    operationIdFieldIndex: Int,
    fieldCount: Int,
): CanonicalOfflineRequest {
    require(value.isNotEmpty()) { "noritoArchive must not be empty" }
    val archive = value.copyOf()
    val decoded = NoritoHeader.decode(archive, SchemaHash.hash16(schema))
    require(decoded.header.compression == NoritoHeader.COMPRESSION_NONE) {
        "Offline request archive must not be compressed"
    }
    require(decoded.header.flags == NoritoHeader.COMPACT_LEN) {
        "Offline request root must use canonical compact sequential field framing"
    }
    require(archive.size == NoritoHeader.HEADER_LENGTH + decoded.header.payloadLength) {
        "Offline request archive must not contain header padding"
    }
    decoded.header.validateChecksum(decoded.payload)
    val decoder = NoritoDecoder(
        decoded.payload,
        decoded.header.flags,
        decoded.header.minor,
    )
    var operationIdBytes: ByteArray? = null
    repeat(fieldCount) { fieldIndex ->
        val length = decoder.readLength(
            (decoder.flags and NoritoHeader.COMPACT_LEN) != 0,
        )
        require(length >= 0 && length <= Int.MAX_VALUE) {
            "Offline request field length overflow"
        }
        val field = decoder.readBytes(length.toInt())
        if (fieldIndex == operationIdFieldIndex) {
            require(field.size == 32) {
                "Offline request operation_id must contain exactly 32 raw bytes"
            }
            operationIdBytes = field
        }
    }
    require(decoder.remaining() == 0) {
        "Trailing fields or bytes after canonical Offline request"
    }
    val operationId = checkNotNull(operationIdBytes)
    require(operationId.any { it.toInt() != 0 }) {
        "Offline request operation_id must be non-zero"
    }
    return CanonicalOfflineRequest(lowercaseHex(operationId), archive)
}

private fun lowercaseHex(value: ByteArray): String {
    val result = CharArray(value.size * 2)
    value.forEachIndexed { index, byte ->
        val unsigned = byte.toInt() and 0xFF
        result[index * 2] = LOWER_HEX[unsigned ushr 4]
        result[index * 2 + 1] = LOWER_HEX[unsigned and 0x0F]
    }
    return String(result)
}

internal fun requireOperationId(value: String): String {
    require(value.length == 64 && value.all { it in '0'..'9' || it in 'a'..'f' }) {
        "operationId must be 64 lowercase hexadecimal characters"
    }
    require(value.any { it != '0' }) { "operationId must be non-zero" }
    return value
}

internal fun requireExactNonEmptyText(value: String, field: String): String {
    require(value.isNotEmpty() && value == value.trim()) { "$field must be exact non-empty text" }
    return value
}

internal fun requireU64(value: BigInteger, field: String): BigInteger {
    require(value >= BigInteger.ZERO && value <= BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)) {
        "$field must fit in an unsigned 64-bit integer"
    }
    return value
}

private fun unsignedLongToBigInteger(value: Long): BigInteger =
    if (value >= 0) {
        BigInteger.valueOf(value)
    } else {
        BigInteger.valueOf(value and Long.MAX_VALUE).setBit(63)
    }

private fun writeField(encoder: NoritoEncoder, write: (NoritoEncoder) -> Unit) {
    val child = encoder.childEncoder()
    write(child)
    val payload = child.toByteArray()
    encoder.writeLength(payload.size.toLong(), true)
    encoder.writeBytes(payload)
}

private fun <T> readField(decoder: NoritoDecoder, read: (NoritoDecoder) -> T): T {
    val length = decoder.readLength(true)
    require(length <= Int.MAX_VALUE) { "Offline operation field length overflow" }
    val child = NoritoDecoder(decoder.readBytes(length.toInt()), decoder.flags, decoder.flagsHint)
    val value = read(child)
    require(child.remaining() == 0) { "Trailing bytes after Offline operation field" }
    return value
}

private fun writeString(encoder: NoritoEncoder, value: String) {
    val bytes = value.toByteArray(StandardCharsets.UTF_8)
    encoder.writeLength(bytes.size.toLong(), true)
    encoder.writeBytes(bytes)
}

private fun readString(decoder: NoritoDecoder): String {
    val length = decoder.readLength(true)
    require(length <= Int.MAX_VALUE) { "Offline operation string length overflow" }
    return requireExactNonEmptyText(
        String(decoder.readBytes(length.toInt()), StandardCharsets.UTF_8),
        "Offline operation string",
    )
}
