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

/** Derived policy metrics for an ordered Offline Bearer Cash audit trail. */
data class OfflineBearerCashAuditTrailMetricsV1(
    @JvmField val custodyHops: Int,
    @JvmField val lineageSteps: Int,
)

/** Pilot policy defaults for Offline Bearer Cash v1 handoffs. */
class OfflineBearerCashPolicyV1 @JvmOverloads constructor(
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

    @JvmOverloads
    fun auditTrailMetrics(
        audits: List<OfflineNote.AuditBundle>,
        terminalAudit: OfflineNote.AuditBundle? = null,
    ): OfflineBearerCashAuditTrailMetricsV1 {
        if (terminalAudit != null) {
            require(audits.isNotEmpty() && audits.last().noritoEncoded().contentEquals(terminalAudit.noritoEncoded())) {
                "bearer audit trail must end with terminal audit"
            }
        }
        if (audits.isEmpty()) {
            return OfflineBearerCashAuditTrailMetricsV1(custodyHops = 0, lineageSteps = 0)
        }

        val tokenIds = LinkedHashSet<String>()
        val nullifiers = LinkedHashSet<String>()
        val outputProducerIndex = LinkedHashMap<String, Int>()
        audits.forEachIndexed { index, audit ->
            val tokenId = bearerCashHexLower(audit.tokenId())
            require(tokenIds.add(tokenId)) { "bearer audit trail has duplicate token id: $tokenId" }
            audit.inputNullifiers().forEach { nullifier ->
                val key = bearerCashHexLower(nullifier)
                require(nullifiers.add(key)) { "bearer audit trail has duplicate input nullifier: $key" }
            }
            val committed = audit.outputCommitments().map(::bearerCashHexLower).toSet()
            audit.outputClaims.forEach { claim ->
                val key = bearerCashHexLower(claim.noteCommitment())
                require(key in committed) { "bearer audit trail output claim is not committed: $key" }
            }
            audit.outputCommitments().forEach { output ->
                val key = bearerCashHexLower(output)
                require(!outputProducerIndex.containsKey(key)) {
                    "bearer audit trail has duplicate output commitment: $key"
                }
                outputProducerIndex[key] = index
            }
        }

        val depths = ArrayList<Int>(audits.size)
        var maxDepth = 0
        audits.forEachIndexed { index, audit ->
            var parentDepth = 0
            audit.inputClaims.forEach { claim ->
                val key = bearerCashHexLower(claim.noteCommitment())
                val producerIndex = outputProducerIndex[key] ?: return@forEach
                require(producerIndex < index) { "bearer audit trail input claim is out of order: $key" }
                parentDepth = maxOf(parentDepth, depths[producerIndex])
            }
            val depth = parentDepth + 1
            depths.add(depth)
            maxDepth = maxOf(maxDepth, depth)
        }

        return OfflineBearerCashAuditTrailMetricsV1(
            custodyHops = maxDepth,
            lineageSteps = audits.size,
        )
    }

    @JvmOverloads
    fun validateAuditTrail(
        audits: List<OfflineNote.AuditBundle>,
        terminalAudit: OfflineNote.AuditBundle? = null,
    ): OfflineBearerCashAuditTrailMetricsV1 {
        val metrics = auditTrailMetrics(audits, terminalAudit)
        require(metrics.custodyHops <= maxCustodyHops) {
            "bearer audit trail custody hops ${metrics.custodyHops} exceed maxCustodyHops $maxCustodyHops"
        }
        require(metrics.lineageSteps <= maxLineageSteps) {
            "bearer audit trail lineage steps ${metrics.lineageSteps} exceed maxLineageSteps $maxLineageSteps"
        }
        return metrics
    }

    companion object {
        @JvmField
        val DEFAULT: OfflineBearerCashPolicyV1 = OfflineBearerCashPolicyV1()
    }
}

private fun bearerCashHexLower(bytes: ByteArray): String {
    val chars = CharArray(bytes.size * 2)
    val table = "0123456789abcdef"
    for (i in bytes.indices) {
        val value = bytes[i].toInt() and 0xFF
        chars[i * 2] = table[value ushr 4]
        chars[i * 2 + 1] = table[value and 0x0F]
    }
    return String(chars)
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
