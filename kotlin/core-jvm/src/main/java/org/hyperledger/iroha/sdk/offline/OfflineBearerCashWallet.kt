package org.hyperledger.iroha.sdk.offline

typealias OfflineBearerCashWallet = OfflineNoteWallet
typealias OfflineBearerCashNote = OfflineNoteWalletNote
typealias OfflineBearerCashReceiveRequestV1 = OfflineNoteReceiveRequest
typealias OfflineBearerCashPaymentTokenV1 = OfflineNotePaymentToken
typealias OfflineBearerCashAckV1 = OfflineNoteReceiptAck

/** Transport selected by the Offline Bearer Cash v1 payload-size policy. */
enum class OfflineBearerCashTransport {
    STATIC_QR,
    STREAMING_QR,
    FRAMED_BYTE_TRANSPORT,
}

/** Pilot policy defaults for Offline Bearer Cash v1 handoffs. */
class OfflineBearerCashPolicyV1 @JvmOverloads constructor(
    // TODO: Enforce custody and lineage limits when note audit payloads carry those counters.
    val maxCustodyHops: Int = 5,
    val maxLineageSteps: Int = 32,
    val maxSingleQrPayloadBytes: Int = 2_048,
    val maxStreamPayloadBytes: Int = 12_288,
    val androidKeyPoolTarget: Int = 20,
    val androidKeyPoolReplenishBelow: Int = 8,
    val androidKeyPoolCap: Int = 40,
) {
    init {
        require(maxCustodyHops > 0) { "maxCustodyHops must be positive" }
        require(maxLineageSteps > 0) { "maxLineageSteps must be positive" }
        require(maxSingleQrPayloadBytes > 0) { "maxSingleQrPayloadBytes must be positive" }
        require(maxStreamPayloadBytes >= maxSingleQrPayloadBytes) {
            "maxStreamPayloadBytes must cover maxSingleQrPayloadBytes"
        }
        require(androidKeyPoolReplenishBelow > 0) { "androidKeyPoolReplenishBelow must be positive" }
        require(androidKeyPoolTarget >= androidKeyPoolReplenishBelow) {
            "androidKeyPoolTarget must cover androidKeyPoolReplenishBelow"
        }
        require(androidKeyPoolCap >= androidKeyPoolTarget) { "androidKeyPoolCap must cover androidKeyPoolTarget" }
    }

    fun recommendedTransportForPayloadByteCount(payloadByteCount: Int): OfflineBearerCashTransport {
        require(payloadByteCount > 0) { "payloadByteCount must be positive" }
        return when {
            payloadByteCount <= maxSingleQrPayloadBytes -> OfflineBearerCashTransport.STATIC_QR
            payloadByteCount <= maxStreamPayloadBytes -> OfflineBearerCashTransport.STREAMING_QR
            else -> OfflineBearerCashTransport.FRAMED_BYTE_TRANSPORT
        }
    }

    companion object {
        @JvmField
        val DEFAULT: OfflineBearerCashPolicyV1 = OfflineBearerCashPolicyV1()
    }
}

/** Text payload kind accepted by Offline Bearer Cash v1 app transports. */
enum class OfflineBearerCashPayloadKindV1 {
    RECEIVE_REQUEST,
    PAYMENT,
    ACK,
}

/** Bearer Cash v1 text codec over the ZK Offline Note wire payloads. */
object OfflineBearerCashTextCodec {
    const val RECEIVE_REQUEST_TEXT_PREFIX: String = "wallet-offline-bearer-cash-receive:"
    const val PAYMENT_TEXT_PREFIX: String = "wallet-offline-bearer-cash-payment:"
    const val ACK_TEXT_PREFIX: String = "wallet-offline-bearer-cash-ack:"

    @JvmStatic
    fun encodeReceiveRequestText(request: OfflineBearerCashReceiveRequestV1): String =
        OfflineNoteReceiveRequestCodec.encodeText(request)

    @JvmStatic
    fun decodeReceiveRequestText(text: String): OfflineBearerCashReceiveRequestV1 =
        OfflineNoteReceiveRequestCodec.decodeText(text)

    @JvmStatic
    fun encodePaymentText(token: OfflineBearerCashPaymentTokenV1): String =
        OfflineNotePaymentTokenCodec.encodeText(token)

    @JvmStatic
    fun decodePaymentText(text: String): OfflineBearerCashPaymentTokenV1 =
        OfflineNotePaymentTokenCodec.decodeText(text)

    @JvmStatic
    fun encodeAckText(ack: OfflineBearerCashAckV1): String =
        OfflineNoteReceiptAckCodec.encodeText(ack)

    @JvmStatic
    fun decodeAckText(text: String): OfflineBearerCashAckV1 =
        OfflineNoteReceiptAckCodec.decodeText(text)

    @JvmStatic
    fun payloadKind(text: String): OfflineBearerCashPayloadKindV1? {
        val trimmed = text.trim()
        return when {
            trimmed.startsWith(RECEIVE_REQUEST_TEXT_PREFIX) -> OfflineBearerCashPayloadKindV1.RECEIVE_REQUEST
            trimmed.startsWith(PAYMENT_TEXT_PREFIX) -> OfflineBearerCashPayloadKindV1.PAYMENT
            trimmed.startsWith(ACK_TEXT_PREFIX) -> OfflineBearerCashPayloadKindV1.ACK
            else -> null
        }
    }
}
