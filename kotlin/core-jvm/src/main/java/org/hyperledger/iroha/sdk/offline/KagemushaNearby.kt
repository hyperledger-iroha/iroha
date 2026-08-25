package org.hyperledger.iroha.sdk.offline

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
internal enum class KagemushaNearbyPairingSymbol(val code: Int) {
    @SerialName("nearby_pairing_stars") STARS(1),
    @SerialName("nearby_pairing_bird") BIRD(2),
    @SerialName("nearby_pairing_mask") MASK(3);

    companion object {
        @JvmStatic fun fromCode(code: Int): KagemushaNearbyPairingSymbol? =
            entries.firstOrNull { it.code == code }
    }
}

@JvmInline
@Serializable
internal value class KagemushaNearbyPairingChallenge(val symbol: KagemushaNearbyPairingSymbol)

@Serializable
internal enum class KagemushaNearbyMessageKind(val code: Int) {
    @SerialName("receive_request") RECEIVE_REQUEST(1),
    @SerialName("payment") PAYMENT(2),
    @SerialName("acknowledgement") ACKNOWLEDGEMENT(3),
    @SerialName("rejected") REJECTED(4);

    companion object {
        @JvmStatic fun fromCode(code: Int): KagemushaNearbyMessageKind? =
            entries.firstOrNull { it.code == code }
    }
}

internal data class KagemushaNearbyDecoded(
    val messageKind: KagemushaNearbyMessageKind,
    val payload: KagemushaPeerPayload?,
    val pairingChallenge: KagemushaNearbyPairingChallenge?,
)

/**
 * Exact cross-platform Kagemusha Nearby envelope.
 *
 * PKNB1 is deliberately binary: it transports the same authenticated IPM1 bytes used by QR,
 * NFC, and the final Nearby rail without expanding a portable receive offer through JSON/Base64.
 * The fixed header is `PKNB1 || kind:u8 || pairing:u8 || reserved:u8 || length:u32be`.
 */
internal object KagemushaNearbyEnvelopeCodec {
    const val MAXIMUM_ENVELOPE_BYTES = 32_704
    const val HEADER_LENGTH = 12
    private val MAGIC = "PKNB1".toByteArray(Charsets.US_ASCII)

    @JvmStatic
    @JvmOverloads
    fun encode(
        payload: KagemushaPeerPayload,
        pairingChallenge: KagemushaNearbyPairingChallenge? = null,
    ): ByteArray {
        when (payload.kind) {
            KagemushaPeerPayloadKind.RECEIVE_REQUEST -> require(pairingChallenge != null) {
                "Kagemusha receive request requires a pairing challenge"
            }
            KagemushaPeerPayloadKind.PAYMENT,
            KagemushaPeerPayloadKind.ACKNOWLEDGEMENT -> require(pairingChallenge == null) {
                "Kagemusha payment and acknowledgement cannot carry a pairing challenge"
            }
        }
        val message = IrohaPeerKagemushaAdapterV1.wrap(payload).encode()
        return try {
            encodeEnvelope(
                payload.kind.toNearbyKind(),
                pairingChallenge?.symbol?.code ?: 0,
                message,
            )
        } finally {
            message.fill(0)
        }
    }

    @JvmStatic
    fun encodeRejection(): ByteArray =
        encodeEnvelope(KagemushaNearbyMessageKind.REJECTED, 0, ByteArray(0))

    @JvmStatic
    fun decode(data: ByteArray): KagemushaNearbyDecoded {
        require(data.size in HEADER_LENGTH..MAXIMUM_ENVELOPE_BYTES &&
            data.copyOfRange(0, MAGIC.size).contentEquals(MAGIC)
        ) { "Invalid Kagemusha Nearby envelope" }
        val messageKind = KagemushaNearbyMessageKind.fromCode(data[5].toInt() and 0xff)
            ?: throw IllegalArgumentException("Invalid Kagemusha Nearby message kind")
        val pairingCode = data[6].toInt() and 0xff
        require(data[7].toInt() == 0) { "Invalid Kagemusha Nearby envelope flags" }
        val payloadLength = data.readU32Be(8)
        require(payloadLength <= MAXIMUM_ENVELOPE_BYTES - HEADER_LENGTH &&
            data.size == HEADER_LENGTH + payloadLength
        ) { "Kagemusha Nearby envelope length mismatch" }

        if (messageKind == KagemushaNearbyMessageKind.REJECTED) {
            require(pairingCode == 0 && payloadLength == 0) {
                "Invalid Kagemusha Nearby rejection"
            }
            return KagemushaNearbyDecoded(messageKind, null, null)
        }

        val challenge = if (pairingCode == 0) null else {
            val symbol = KagemushaNearbyPairingSymbol.fromCode(pairingCode)
                ?: throw IllegalArgumentException("Invalid Kagemusha Nearby pairing challenge")
            KagemushaNearbyPairingChallenge(symbol)
        }
        val expectedKind = messageKind.toPeerKind()
        when (expectedKind) {
            KagemushaPeerPayloadKind.RECEIVE_REQUEST -> require(challenge != null) {
                "Kagemusha receive request requires a pairing challenge"
            }
            KagemushaPeerPayloadKind.PAYMENT,
            KagemushaPeerPayloadKind.ACKNOWLEDGEMENT -> require(challenge == null) {
                "Kagemusha payment and acknowledgement cannot carry a pairing challenge"
            }
        }
        val payloadBytes = data.copyOfRange(HEADER_LENGTH, data.size)
        return try {
            val message = IrohaPeerWireMessageV1.decode(
                payloadBytes,
                IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
                expectedKind.toWireKind(),
            )
            KagemushaNearbyDecoded(
                messageKind,
                IrohaPeerKagemushaAdapterV1.decode(message),
                challenge,
            )
        } catch (failure: RuntimeException) {
            throw IllegalArgumentException("Invalid Kagemusha Nearby payload", failure)
        } finally {
            payloadBytes.fill(0)
        }
    }

    private fun encodeEnvelope(
        kind: KagemushaNearbyMessageKind,
        pairingCode: Int,
        payload: ByteArray,
    ): ByteArray {
        require(pairingCode in 0..3 && payload.size <= MAXIMUM_ENVELOPE_BYTES - HEADER_LENGTH) {
            "Kagemusha Nearby envelope is too large"
        }
        return ByteArray(HEADER_LENGTH + payload.size).also { encoded ->
            MAGIC.copyInto(encoded, 0)
            encoded[5] = kind.code.toByte()
            encoded[6] = pairingCode.toByte()
            encoded[7] = 0
            encoded.writeU32Be(8, payload.size)
            payload.copyInto(encoded, HEADER_LENGTH)
        }
    }

    private fun KagemushaPeerPayloadKind.toNearbyKind(): KagemushaNearbyMessageKind = when (this) {
        KagemushaPeerPayloadKind.RECEIVE_REQUEST -> KagemushaNearbyMessageKind.RECEIVE_REQUEST
        KagemushaPeerPayloadKind.PAYMENT -> KagemushaNearbyMessageKind.PAYMENT
        KagemushaPeerPayloadKind.ACKNOWLEDGEMENT -> KagemushaNearbyMessageKind.ACKNOWLEDGEMENT
    }

    private fun KagemushaNearbyMessageKind.toPeerKind(): KagemushaPeerPayloadKind = when (this) {
        KagemushaNearbyMessageKind.RECEIVE_REQUEST -> KagemushaPeerPayloadKind.RECEIVE_REQUEST
        KagemushaNearbyMessageKind.PAYMENT -> KagemushaPeerPayloadKind.PAYMENT
        KagemushaNearbyMessageKind.ACKNOWLEDGEMENT -> KagemushaPeerPayloadKind.ACKNOWLEDGEMENT
        KagemushaNearbyMessageKind.REJECTED -> error("A rejection has no peer payload")
    }

    private fun KagemushaPeerPayloadKind.toWireKind(): IrohaPeerPayloadKind = when (this) {
        KagemushaPeerPayloadKind.RECEIVE_REQUEST -> IrohaPeerPayloadKind.RECEIVE_REQUEST
        KagemushaPeerPayloadKind.PAYMENT -> IrohaPeerPayloadKind.PAYMENT
        KagemushaPeerPayloadKind.ACKNOWLEDGEMENT -> IrohaPeerPayloadKind.ACKNOWLEDGEMENT
    }

    private fun ByteArray.readU32Be(offset: Int): Int {
        val value = ((this[offset].toLong() and 0xff) shl 24) or
            ((this[offset + 1].toLong() and 0xff) shl 16) or
            ((this[offset + 2].toLong() and 0xff) shl 8) or
            (this[offset + 3].toLong() and 0xff)
        require(value <= Int.MAX_VALUE) { "Kagemusha Nearby envelope length is invalid" }
        return value.toInt()
    }

    private fun ByteArray.writeU32Be(offset: Int, value: Int) {
        this[offset] = (value ushr 24).toByte()
        this[offset + 1] = (value ushr 16).toByte()
        this[offset + 2] = (value ushr 8).toByte()
        this[offset + 3] = value.toByte()
    }
}

internal object KagemushaNearbyTransportPolicy {
    const val SERVICE_NAME = KagemushaPeerTransportContract.NEARBY_SERVICE_NAME
    const val BONJOUR_SERVICE = KagemushaPeerTransportContract.NEARBY_BONJOUR_SERVICE
    const val DISCOVERY_PROTOCOL = "kagemusha-v2"
    const val REQUIRES_CERTIFICATE_AUTHENTICATED_ECDH_TRANSCRIPT = true
    const val HAS_AUDITED_AUTHENTICATED_TRANSCRIPT_BACKEND = false
    const val IS_AVAILABLE = HAS_AUDITED_AUTHENTICATED_TRANSCRIPT_BACKEND
}
