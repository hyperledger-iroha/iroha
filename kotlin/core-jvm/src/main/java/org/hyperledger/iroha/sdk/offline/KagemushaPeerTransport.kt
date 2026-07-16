package org.hyperledger.iroha.sdk.offline

import java.util.Base64

enum class KagemushaPeerPayloadKind(
    val code: Int,
    val textPrefix: String,
    val contentType: String,
) {
    RECEIVE_REQUEST(
        1,
        KagemushaPeerTransportContract.RECEIVE_REQUEST_TEXT_PREFIX,
        KagemushaPeerTransportContract.RECEIVE_REQUEST_CONTENT_TYPE,
    ),
    PAYMENT(
        2,
        KagemushaPeerTransportContract.PAYMENT_TEXT_PREFIX,
        KagemushaPeerTransportContract.PAYMENT_CONTENT_TYPE,
    ),
    ACKNOWLEDGEMENT(
        3,
        KagemushaPeerTransportContract.ACKNOWLEDGEMENT_TEXT_PREFIX,
        KagemushaPeerTransportContract.ACKNOWLEDGEMENT_CONTENT_TYPE,
    );

    companion object {
        @JvmStatic fun fromCode(code: Int): KagemushaPeerPayloadKind? = entries.firstOrNull { it.code == code }
        @JvmStatic fun fromTextPrefix(prefix: String): KagemushaPeerPayloadKind? =
            entries.firstOrNull { it.textPrefix == prefix }
        @JvmStatic fun fromContentType(contentType: String): KagemushaPeerPayloadKind? =
            entries.firstOrNull { it.contentType == contentType }
    }
}

object KagemushaPeerTransportContract {
    const val RECEIVE_REQUEST_TEXT_PREFIX = "PKK2R."
    const val PAYMENT_TEXT_PREFIX = "PKK2P."
    const val ACKNOWLEDGEMENT_TEXT_PREFIX = "PKK2A."
    const val QR_STREAM_TEXT_PREFIX = "PKKQ1."
    const val NFC_APPLICATION_IDENTIFIER_HEX = "F0504B45504B524E464301"
    const val NEARBY_SERVICE_NAME = "pk-kagemusha"
    const val NEARBY_BONJOUR_SERVICE = "_pk-kagemusha._tcp"
    const val RECEIVE_REQUEST_CONTENT_TYPE = "text/vnd.pk.kagemusha-v2.receive-request"
    const val PAYMENT_CONTENT_TYPE = "text/vnd.pk.kagemusha-v2.payment"
    const val ACKNOWLEDGEMENT_CONTENT_TYPE = "text/vnd.pk.kagemusha-v2.ack"
    const val MAXIMUM_ARCHIVE_BYTES_V2 = KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES_V2
    const val MAXIMUM_ARCHIVE_BYTES_V4 = KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES_V4
    const val MAXIMUM_ARCHIVE_BYTES = MAXIMUM_ARCHIVE_BYTES_V4
    const val MAXIMUM_TEXT_ENVELOPE_BYTES =
        KagemushaRecursiveSpendProver.MAX_PEER_TEXT_ENVELOPE_BYTES
}

sealed class KagemushaPeerPayload {
    abstract val kind: KagemushaPeerPayloadKind
    abstract fun archive(): ByteArray

    class ReceiveRequest internal constructor(
        val request: KagemushaRecursiveSpendProver.RecipientPaymentRequest,
    ) : KagemushaPeerPayload() {
        override val kind = KagemushaPeerPayloadKind.RECEIVE_REQUEST
        override fun archive(): ByteArray = request.noritoEncoded()
    }

    class Payment internal constructor(
        val payment: KagemushaRecursiveSpendProver.PeerPayment,
    ) : KagemushaPeerPayload() {
        override val kind = KagemushaPeerPayloadKind.PAYMENT
        override fun archive(): ByteArray = payment.noritoEncoded()
    }

    class Acknowledgement internal constructor(
        val acknowledgement: KagemushaRecursiveSpendProver.ReceiverAcknowledgement,
    ) : KagemushaPeerPayload() {
        override val kind = KagemushaPeerPayloadKind.ACKNOWLEDGEMENT
        override fun archive(): ByteArray = acknowledgement.noritoEncoded()
    }

    companion object {
        @JvmStatic
        fun decode(archive: ByteArray, kind: KagemushaPeerPayloadKind): KagemushaPeerPayload {
            require(archive.isNotEmpty()) { "Kagemusha peer payload is empty" }
            require(archive.size <= KagemushaPeerTransportContract.MAXIMUM_ARCHIVE_BYTES) {
                "Kagemusha peer archive exceeds its bound"
            }
            return try {
                when (kind) {
                    KagemushaPeerPayloadKind.RECEIVE_REQUEST -> ReceiveRequest(
                        KagemushaRecursiveSpendProver.decodeRecipientPaymentRequest(archive),
                    )
                    KagemushaPeerPayloadKind.PAYMENT -> Payment(
                        KagemushaRecursiveSpendProver.decodePeerPayment(archive),
                    )
                    KagemushaPeerPayloadKind.ACKNOWLEDGEMENT -> Acknowledgement(
                        KagemushaRecursiveSpendProver.decodeReceiverAcknowledgement(archive),
                    )
                }
            } catch (failure: RuntimeException) {
                throw IllegalArgumentException("Invalid Kagemusha ${kind.name.lowercase()} archive", failure)
            }
        }
    }
}

object KagemushaPeerTextCodec {
    private val encoder = Base64.getUrlEncoder().withoutPadding()
    private val decoder = Base64.getUrlDecoder()

    @JvmStatic
    fun encode(payload: KagemushaPeerPayload): String {
        val archive = payload.archive()
        try {
            require(archive.isNotEmpty()) { "Kagemusha peer payload is empty" }
            require(archive.size <= KagemushaPeerTransportContract.MAXIMUM_ARCHIVE_BYTES) {
                "Kagemusha peer archive exceeds its bound"
            }
            val value = payload.kind.textPrefix + base64UrlEncode(archive)
            require(value.toByteArray(Charsets.UTF_8).size <=
                KagemushaPeerTransportContract.MAXIMUM_TEXT_ENVELOPE_BYTES
            ) { "Kagemusha peer text exceeds its bound" }
            return value
        } finally {
            archive.fill(0)
        }
    }

    @JvmStatic
    @JvmOverloads
    fun decode(
        value: String,
        expectedKind: KagemushaPeerPayloadKind? = null,
    ): KagemushaPeerPayload {
        require(value.toByteArray(Charsets.UTF_8).size <=
            KagemushaPeerTransportContract.MAXIMUM_TEXT_ENVELOPE_BYTES
        ) { "Kagemusha peer text exceeds its bound" }
        val kind = kindOf(value) ?: throw IllegalArgumentException("Kagemusha peer prefix is invalid")
        require(expectedKind == null || expectedKind == kind) {
            "Unexpected Kagemusha peer payload kind"
        }
        val body = value.substring(kind.textPrefix.length)
        val archive = base64UrlDecode(body)
            ?: throw IllegalArgumentException("Kagemusha peer text is not canonical Base64URL")
        try {
            require(kind.textPrefix + base64UrlEncode(archive) == value) {
                "Kagemusha peer text is not canonical"
            }
            return KagemushaPeerPayload.decode(archive, kind)
        } finally {
            archive.fill(0)
        }
    }

    @JvmStatic
    @JvmOverloads
    fun decodeUserPresented(
        value: String,
        expectedKind: KagemushaPeerPayloadKind? = null,
    ): KagemushaPeerPayload {
        require(value.toByteArray(Charsets.UTF_8).size <=
            KagemushaPeerTransportContract.MAXIMUM_TEXT_ENVELOPE_BYTES
        ) { "Kagemusha peer text exceeds its bound" }
        return decode(canonicalizeUserPresented(value), expectedKind)
    }

    @JvmStatic
    fun canonicalizeUserPresented(value: String): String =
        value.trim(' ', '\t', '\r', '\n')

    @JvmStatic
    fun kindOf(value: String): KagemushaPeerPayloadKind? =
        KagemushaPeerPayloadKind.entries.firstOrNull { value.startsWith(it.textPrefix) }

    @JvmStatic
    fun base64UrlEncode(data: ByteArray): String = encoder.encodeToString(data)

    @JvmStatic
    fun base64UrlDecode(value: String): ByteArray? {
        if (value.isEmpty() || value.length % 4 == 1 ||
            !value.all { it in '0'..'9' || it in 'A'..'Z' || it in 'a'..'z' || it == '-' || it == '_' }
        ) return null
        val decoded = try {
            decoder.decode(value)
        } catch (_: IllegalArgumentException) {
            return null
        }
        if (decoded.isEmpty() || base64UrlEncode(decoded) != value) {
            decoded.fill(0)
            return null
        }
        return decoded
    }
}
