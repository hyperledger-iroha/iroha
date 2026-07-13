package org.hyperledger.iroha.sdk.offline

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

@Serializable
enum class KagemushaNearbyPairingSymbol {
    @SerialName("nearby_pairing_stars") STARS,
    @SerialName("nearby_pairing_bird") BIRD,
    @SerialName("nearby_pairing_mask") MASK,
}

@JvmInline
@Serializable
value class KagemushaNearbyPairingChallenge(val symbol: KagemushaNearbyPairingSymbol)

@Serializable
enum class KagemushaNearbyMessageKind {
    @SerialName("receive_request") RECEIVE_REQUEST,
    @SerialName("payment") PAYMENT,
    @SerialName("acknowledgement") ACKNOWLEDGEMENT,
    @SerialName("rejected") REJECTED,
}

data class KagemushaNearbyDecoded(
    val messageKind: KagemushaNearbyMessageKind,
    val payload: KagemushaPeerPayload?,
    val pairingChallenge: KagemushaNearbyPairingChallenge?,
)

object KagemushaNearbyEnvelopeCodec {
    const val MAXIMUM_ENVELOPE_BYTES = 20 * 1024
    private const val REJECTION_CONTENT_TYPE = "text/plain"
    private const val REJECTION_TEXT = "rejected"
    private val json = Json {
        encodeDefaults = true
        explicitNulls = false
        ignoreUnknownKeys = false
        isLenient = false
        coerceInputValues = false
        useAlternativeNames = false
        allowTrailingComma = false
        allowComments = false
    }

    /* Declaration order is canonical sorted-key order: contentType, kind, pairingChallenge, payload. */
    @Serializable
    private data class Envelope(
        val contentType: String,
        val kind: KagemushaNearbyMessageKind,
        val pairingChallenge: KagemushaNearbyPairingChallenge? = null,
        val payload: String,
    )

    @JvmStatic
    @JvmOverloads
    fun encode(
        payload: KagemushaPeerPayload,
        pairingChallenge: KagemushaNearbyPairingChallenge? = null,
    ): ByteArray {
        when (payload.kind) {
            KagemushaPeerPayloadKind.RECEIVE_REQUEST -> require(pairingChallenge != null)
            KagemushaPeerPayloadKind.PAYMENT,
            KagemushaPeerPayloadKind.ACKNOWLEDGEMENT -> require(pairingChallenge == null)
        }
        val text = KagemushaPeerTextCodec.encode(payload).toByteArray(Charsets.UTF_8)
        try {
            return encodeEnvelope(
                Envelope(
                    contentType = payload.kind.contentType,
                    kind = payload.kind.toNearbyKind(),
                    pairingChallenge = pairingChallenge,
                    payload = KagemushaPeerTextCodec.base64UrlEncode(text),
                ),
            )
        } finally {
            text.fill(0)
        }
    }

    @JvmStatic
    fun encodeRejection(): ByteArray {
        val bytes = REJECTION_TEXT.toByteArray(Charsets.UTF_8)
        try {
            return encodeEnvelope(
                Envelope(
                    contentType = REJECTION_CONTENT_TYPE,
                    kind = KagemushaNearbyMessageKind.REJECTED,
                    payload = KagemushaPeerTextCodec.base64UrlEncode(bytes),
                ),
            )
        } finally {
            bytes.fill(0)
        }
    }

    @JvmStatic
    fun decode(data: ByteArray): KagemushaNearbyDecoded {
        require(data.isNotEmpty() && data.size <= MAXIMUM_ENVELOPE_BYTES) {
            "Invalid Kagemusha Nearby envelope"
        }
        val text = data.toString(Charsets.UTF_8)
        val envelope = runCatching { json.decodeFromString(Envelope.serializer(), text) }
            .getOrElse { throw IllegalArgumentException("Invalid Kagemusha Nearby envelope", it) }
        require(encodeEnvelope(envelope).contentEquals(data)) {
            "Kagemusha Nearby envelope is not canonical"
        }
        val payloadBytes = KagemushaPeerTextCodec.base64UrlDecode(envelope.payload)
            ?: throw IllegalArgumentException("Invalid Kagemusha Nearby payload")
        try {
            require(payloadBytes.size <= KagemushaPeerTransportContract.MAXIMUM_TEXT_ENVELOPE_BYTES)
            if (envelope.kind == KagemushaNearbyMessageKind.REJECTED) {
                require(envelope.contentType == REJECTION_CONTENT_TYPE &&
                    envelope.pairingChallenge == null &&
                    payloadBytes.contentEquals(REJECTION_TEXT.toByteArray(Charsets.UTF_8))
                ) { "Invalid Kagemusha Nearby rejection" }
                return KagemushaNearbyDecoded(envelope.kind, null, null)
            }
            val kind = KagemushaPeerPayloadKind.fromContentType(envelope.contentType)
                ?: throw IllegalArgumentException("Invalid Kagemusha Nearby content type")
            require(kind.toNearbyKind() == envelope.kind)
            val payloadText = payloadBytes.toString(Charsets.UTF_8)
            val payload = KagemushaPeerTextCodec.decode(payloadText, kind)
            when (kind) {
                KagemushaPeerPayloadKind.RECEIVE_REQUEST -> require(envelope.pairingChallenge != null)
                KagemushaPeerPayloadKind.PAYMENT,
                KagemushaPeerPayloadKind.ACKNOWLEDGEMENT -> require(envelope.pairingChallenge == null)
            }
            return KagemushaNearbyDecoded(envelope.kind, payload, envelope.pairingChallenge)
        } finally {
            payloadBytes.fill(0)
        }
    }

    private fun encodeEnvelope(envelope: Envelope): ByteArray =
        json.encodeToString(Envelope.serializer(), envelope).toByteArray(Charsets.UTF_8).also {
            require(it.size <= MAXIMUM_ENVELOPE_BYTES) { "Kagemusha Nearby envelope is too large" }
        }

    private fun KagemushaPeerPayloadKind.toNearbyKind(): KagemushaNearbyMessageKind = when (this) {
        KagemushaPeerPayloadKind.RECEIVE_REQUEST -> KagemushaNearbyMessageKind.RECEIVE_REQUEST
        KagemushaPeerPayloadKind.PAYMENT -> KagemushaNearbyMessageKind.PAYMENT
        KagemushaPeerPayloadKind.ACKNOWLEDGEMENT -> KagemushaNearbyMessageKind.ACKNOWLEDGEMENT
    }
}

object KagemushaNearbyTransportPolicy {
    const val SERVICE_NAME = KagemushaPeerTransportContract.NEARBY_SERVICE_NAME
    const val BONJOUR_SERVICE = KagemushaPeerTransportContract.NEARBY_BONJOUR_SERVICE
    const val DISCOVERY_PROTOCOL = "kagemusha-v2"
    const val REQUIRES_CERTIFICATE_AUTHENTICATED_ECDH_TRANSCRIPT = true
    const val HAS_AUDITED_AUTHENTICATED_TRANSCRIPT_BACKEND = false
    const val IS_AVAILABLE = HAS_AUDITED_AUTHENTICATED_TRANSCRIPT_BACKEND
}
