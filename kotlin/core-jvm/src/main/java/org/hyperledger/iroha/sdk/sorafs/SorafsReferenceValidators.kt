package org.hyperledger.iroha.sdk.sorafs

import java.nio.charset.StandardCharsets

/** Orderbook payload kind accepted by the Rust-backed SoraFS reference validator. */
enum class SorafsOrderbookPayloadKind(
    @JvmField val bridgeCode: Int,
    @JvmField val defaultLabel: String,
    @JvmField val isUserSignedPayload: Boolean,
) {
    ORDER_REQUEST(1, "order-request.to", true),
    ORDER_CANCEL(2, "order-cancel.to", true),
    TRADE_EVENT(3, "trade-event.to", false),
    SETTLEMENT_CHANNEL(4, "settlement-channel.to", false),
    SETTLEMENT_RECEIPT(5, "settlement-receipt.to", true),
    RUNTIME_SNAPSHOT(6, "orderbook-runtime-snapshot.to", false),
}

/** PDP payload kind accepted by the Rust-backed SoraFS reference validator. */
enum class SorafsPdpPayloadKind(
    @JvmField val bridgeCode: Int,
    @JvmField val defaultLabel: String,
) {
    COMMITMENT(1, "commitment.to"),
    CHALLENGE(2, "challenge.to"),
    PROOF(3, "proof.to"),
}

/** Side selector for field-level SoraFS orderbook order builders. */
enum class SorafsOrderbookSide(@JvmField val bridgeCode: Int) {
    BID(1),
    ASK(2),
}

/** Storage tier selector for field-level SoraFS orderbook order builders. */
enum class SorafsOrderbookTier(@JvmField val bridgeCode: Int) {
    HOT(1),
    WARM(2),
    ARCHIVE(3),
}

/** Cancellation reason selector for field-level SoraFS orderbook cancel builders. */
enum class SorafsOrderbookCancelReason(@JvmField val bridgeCode: Int) {
    OWNER_REQUESTED(1),
    EXPIRED(2),
    GOVERNANCE(3),
    REPLACED(4),
}

/** Thin JVM/JNI wrapper around the SoraFS reference validators in `connect_norito_bridge`. */
class SorafsReferenceValidators private constructor() {
    companion object {
        private const val LIBRARY_NAME = "connect_norito_bridge"
        const val REQUIRED_BRIDGE_ABI_VERSION: Int = 10
        private val nativeAvailable: Boolean = loadLibrary()

        @JvmStatic
        fun isNativeAvailable(): Boolean = nativeAvailable

        @JvmStatic
        @JvmOverloads
        fun validateOrderbookPayloadJson(
            kind: SorafsOrderbookPayloadKind,
            noritoBytes: ByteArray,
            label: String? = null,
            generatedAtUnix: Long = currentEpochSeconds(),
        ): String {
            requireGeneratedAt(generatedAtUnix)
            val payload = noritoBytes.copyOf()
            val labelBytes = labelBytes(label, kind.defaultLabel)
            requireNative()
            return requireJsonOutput(
                nativeValidateOrderbookPayloadJson(
                    kind.bridgeCode,
                    payload,
                    labelBytes,
                    generatedAtUnix,
                ),
                "SoraFS orderbook validation",
            )
        }

        @JvmStatic
        fun signOrderbookPayload(
            kind: SorafsOrderbookPayloadKind,
            noritoBytes: ByteArray,
            privateKey: ByteArray,
        ): ByteArray {
            val selected = requireUserSignedOrderbookKind(kind)
            val payload = noritoBytes.copyOf()
            val key = requirePrivateKey(privateKey)
            try {
                requireNative()
                return requireBytesOutput(
                    nativeSignOrderbookPayload(selected.bridgeCode, payload, key),
                    "SoraFS orderbook signing",
                )
            } finally {
                key.fill(0)
            }
        }

        @JvmStatic
        @JvmOverloads
        fun buildSignedOrderbookOrderRequest(
            orderId: ByteArray,
            side: SorafsOrderbookSide,
            tier: SorafsOrderbookTier,
            pricePerGibMicroXor: String,
            quantityGib: Long,
            ownerAccount: ByteArray,
            expiryUnix: Long,
            nonce: Long,
            makerFeeBps: Int,
            takerFeeBps: Int,
            privateKey: ByteArray,
            remainingGib: Long = quantityGib,
        ): ByteArray {
            val orderIdBytes = requireFixed32(orderId, "orderId")
            val ownerBytes = requireNonEmptyBytes(ownerAccount, "ownerAccount")
            val priceBytes = decimalBytes(pricePerGibMicroXor, "pricePerGibMicroXor", positive = true)
            requirePositive(quantityGib, "quantityGib")
            requirePositive(remainingGib, "remainingGib")
            requirePositive(expiryUnix, "expiryUnix")
            requirePositive(nonce, "nonce")
            val makerFee = requireFeeBps(makerFeeBps, "makerFeeBps")
            val takerFee = requireFeeBps(takerFeeBps, "takerFeeBps")
            val key = requirePrivateKey(privateKey)
            try {
                requireNative()
                return requireBytesOutput(
                    nativeBuildSignedOrderbookOrderRequest(
                        orderIdBytes,
                        side.bridgeCode,
                        tier.bridgeCode,
                        priceBytes,
                        quantityGib,
                        remainingGib,
                        ownerBytes,
                        expiryUnix,
                        nonce,
                        makerFee,
                        takerFee,
                        key,
                    ),
                    "SoraFS orderbook order request builder",
                )
            } finally {
                key.fill(0)
            }
        }

        @JvmStatic
        fun buildSignedOrderbookOrderCancel(
            orderId: ByteArray,
            ownerAccount: ByteArray,
            reason: SorafsOrderbookCancelReason,
            nonce: Long,
            privateKey: ByteArray,
        ): ByteArray {
            val orderIdBytes = requireFixed32(orderId, "orderId")
            val ownerBytes = requireNonEmptyBytes(ownerAccount, "ownerAccount")
            requirePositive(nonce, "nonce")
            val key = requirePrivateKey(privateKey)
            try {
                requireNative()
                return requireBytesOutput(
                    nativeBuildSignedOrderbookOrderCancel(
                        orderIdBytes,
                        ownerBytes,
                        reason.bridgeCode,
                        nonce,
                        key,
                    ),
                    "SoraFS orderbook cancel builder",
                )
            } finally {
                key.fill(0)
            }
        }

        @JvmStatic
        fun buildSignedOrderbookSettlementReceipt(
            receiptId: ByteArray,
            channelId: ByteArray,
            tradeId: ByteArray,
            rangeStart: Long,
            rangeEnd: Long,
            chunkHash: ByteArray,
            bytesDelivered: Long,
            xorDebitedMicroXor: String,
            providerCreditMicroXor: String,
            feeAmountMicroXor: String,
            issuedAtUnix: Long,
            privateKey: ByteArray,
        ): ByteArray {
            val receiptIdBytes = requireFixed32(receiptId, "receiptId")
            val channelIdBytes = requireFixed32(channelId, "channelId")
            val tradeIdBytes = requireFixed32(tradeId, "tradeId")
            requireNonNegative(rangeStart, "rangeStart")
            requirePositive(rangeEnd, "rangeEnd")
            val chunkHashBytes = requireFixed32(chunkHash, "chunkHash")
            requirePositive(bytesDelivered, "bytesDelivered")
            val debitBytes = decimalBytes(xorDebitedMicroXor, "xorDebitedMicroXor", positive = true)
            val creditBytes = decimalBytes(providerCreditMicroXor, "providerCreditMicroXor", positive = false)
            val feeBytes = decimalBytes(feeAmountMicroXor, "feeAmountMicroXor", positive = false)
            requirePositive(issuedAtUnix, "issuedAtUnix")
            val key = requirePrivateKey(privateKey)
            try {
                requireNative()
                return requireBytesOutput(
                    nativeBuildSignedOrderbookSettlementReceipt(
                        receiptIdBytes,
                        channelIdBytes,
                        tradeIdBytes,
                        rangeStart,
                        rangeEnd,
                        chunkHashBytes,
                        bytesDelivered,
                        debitBytes,
                        creditBytes,
                        feeBytes,
                        issuedAtUnix,
                        key,
                    ),
                    "SoraFS orderbook settlement receipt builder",
                )
            } finally {
                key.fill(0)
            }
        }

        @JvmStatic
        @JvmOverloads
        fun validatePdpPayloadJson(
            kind: SorafsPdpPayloadKind,
            noritoBytes: ByteArray,
            label: String? = null,
            generatedAtUnix: Long = currentEpochSeconds(),
        ): String {
            requireGeneratedAt(generatedAtUnix)
            val payload = noritoBytes.copyOf()
            val labelBytes = labelBytes(label, kind.defaultLabel)
            requireNative()
            return requireJsonOutput(
                nativeValidatePdpPayloadJson(
                    kind.bridgeCode,
                    payload,
                    labelBytes,
                    generatedAtUnix,
                ),
                "SoraFS PDP validation",
            )
        }

        @JvmStatic
        @JvmOverloads
        fun validatePdpCommitmentChallengeJson(
            commitment: ByteArray,
            challenge: ByteArray,
            commitmentLabel: String? = null,
            challengeLabel: String? = null,
            generatedAtUnix: Long = currentEpochSeconds(),
        ): String {
            requireGeneratedAt(generatedAtUnix)
            val commitmentPayload = commitment.copyOf()
            val commitmentLabelBytes = labelBytes(commitmentLabel, SorafsPdpPayloadKind.COMMITMENT.defaultLabel)
            val challengePayload = challenge.copyOf()
            val challengeLabelBytes = labelBytes(challengeLabel, SorafsPdpPayloadKind.CHALLENGE.defaultLabel)
            requireNative()
            return requireJsonOutput(
                nativeValidatePdpCommitmentChallengeJson(
                    commitmentPayload,
                    commitmentLabelBytes,
                    challengePayload,
                    challengeLabelBytes,
                    generatedAtUnix,
                ),
                "SoraFS PDP commitment/challenge validation",
            )
        }

        @JvmStatic
        @JvmOverloads
        fun validatePdpChallengeProofJson(
            challenge: ByteArray,
            proof: ByteArray,
            challengeLabel: String? = null,
            proofLabel: String? = null,
            generatedAtUnix: Long = currentEpochSeconds(),
        ): String {
            requireGeneratedAt(generatedAtUnix)
            val challengePayload = challenge.copyOf()
            val challengeLabelBytes = labelBytes(challengeLabel, SorafsPdpPayloadKind.CHALLENGE.defaultLabel)
            val proofPayload = proof.copyOf()
            val proofLabelBytes = labelBytes(proofLabel, SorafsPdpPayloadKind.PROOF.defaultLabel)
            requireNative()
            return requireJsonOutput(
                nativeValidatePdpChallengeProofJson(
                    challengePayload,
                    challengeLabelBytes,
                    proofPayload,
                    proofLabelBytes,
                    generatedAtUnix,
                ),
                "SoraFS PDP challenge/proof validation",
            )
        }

        @JvmStatic
        @JvmOverloads
        fun validatePdpBundleJson(
            commitment: ByteArray,
            challenge: ByteArray,
            proof: ByteArray,
            commitmentLabel: String? = null,
            challengeLabel: String? = null,
            proofLabel: String? = null,
            generatedAtUnix: Long = currentEpochSeconds(),
        ): String {
            requireGeneratedAt(generatedAtUnix)
            val commitmentPayload = commitment.copyOf()
            val commitmentLabelBytes = labelBytes(commitmentLabel, SorafsPdpPayloadKind.COMMITMENT.defaultLabel)
            val challengePayload = challenge.copyOf()
            val challengeLabelBytes = labelBytes(challengeLabel, SorafsPdpPayloadKind.CHALLENGE.defaultLabel)
            val proofPayload = proof.copyOf()
            val proofLabelBytes = labelBytes(proofLabel, SorafsPdpPayloadKind.PROOF.defaultLabel)
            requireNative()
            return requireJsonOutput(
                nativeValidatePdpBundleJson(
                    commitmentPayload,
                    commitmentLabelBytes,
                    challengePayload,
                    challengeLabelBytes,
                    proofPayload,
                    proofLabelBytes,
                    generatedAtUnix,
                ),
                "SoraFS PDP bundle validation",
            )
        }

        private fun currentEpochSeconds(): Long = System.currentTimeMillis() / 1000L

        private fun requireGeneratedAt(generatedAtUnix: Long) {
            require(generatedAtUnix >= 0L) { "generatedAtUnix must be non-negative" }
        }

        private fun requireNative() {
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
        }

        private fun requireUserSignedOrderbookKind(kind: SorafsOrderbookPayloadKind): SorafsOrderbookPayloadKind {
            require(kind.isUserSignedPayload) { "orderbook payload kind ${kind.name} cannot be signed" }
            return kind
        }

        private fun requirePrivateKey(privateKey: ByteArray): ByteArray {
            require(privateKey.size == 32) { "privateKey must be 32 bytes" }
            require(privateKey.any { byte -> byte.toInt() != 0 }) { "privateKey must not be all zero" }
            return privateKey.copyOf()
        }

        private fun requireFixed32(bytes: ByteArray, field: String): ByteArray {
            require(bytes.size == 32) { "$field must be 32 bytes" }
            return bytes.copyOf()
        }

        private fun requireNonEmptyBytes(bytes: ByteArray, field: String): ByteArray {
            require(bytes.isNotEmpty()) { "$field must not be empty" }
            return bytes.copyOf()
        }

        private fun requireNonNegative(value: Long, field: String) {
            require(value >= 0L) { "$field must be non-negative" }
        }

        private fun requirePositive(value: Long, field: String) {
            require(value > 0L) { "$field must be greater than zero" }
        }

        private fun requireFeeBps(value: Int, field: String): Int {
            require(value in 0..0xFFFF) { "$field must fit in u16 basis points" }
            return value
        }

        private fun decimalBytes(value: String, field: String, positive: Boolean): ByteArray {
            require(value.isNotEmpty() && value.all { char -> char in '0'..'9' }) {
                "$field must be an unsigned decimal integer"
            }
            if (positive) {
                require(value.any { char -> char != '0' }) { "$field must be greater than zero" }
            }
            return value.toByteArray(StandardCharsets.UTF_8)
        }

        private fun labelBytes(label: String?, fallback: String): ByteArray {
            val value = label ?: fallback
            require(value.isNotBlank()) { "label must not be blank" }
            require(value.trim() == value) { "label must not contain surrounding whitespace" }
            require(value.indexOf('\u0000') < 0) { "label must not contain NUL" }
            return value.toByteArray(StandardCharsets.UTF_8)
        }

        private fun requireJsonOutput(output: ByteArray?, context: String): String {
            check(output != null) { "$context returned no outcome JSON" }
            check(output.isNotEmpty()) { "$context returned empty outcome JSON" }
            val json = String(output, StandardCharsets.UTF_8)
            check(json.trimStart().startsWith("{")) { "$context returned malformed outcome JSON" }
            return json
        }

        private fun requireBytesOutput(output: ByteArray?, context: String): ByteArray {
            check(output != null) { "$context returned no bytes" }
            check(output.isNotEmpty()) { "$context returned empty bytes" }
            return output.copyOf()
        }

        private fun loadLibrary(): Boolean =
            try {
                System.loadLibrary(LIBRARY_NAME)
                nativeBridgeAbiVersion() >= REQUIRED_BRIDGE_ABI_VERSION
            } catch (_: UnsatisfiedLinkError) {
                false
            } catch (_: SecurityException) {
                false
            }

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic
        private external fun nativeValidateOrderbookPayloadJson(
            kind: Int,
            payload: ByteArray,
            label: ByteArray,
            generatedAtUnix: Long,
        ): ByteArray?

        @JvmStatic
        private external fun nativeSignOrderbookPayload(
            kind: Int,
            payload: ByteArray,
            privateKey: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeBuildSignedOrderbookOrderRequest(
            orderId: ByteArray,
            side: Int,
            tier: Int,
            pricePerGibMicroXor: ByteArray,
            quantityGib: Long,
            remainingGib: Long,
            ownerAccount: ByteArray,
            expiryUnix: Long,
            nonce: Long,
            makerFeeBps: Int,
            takerFeeBps: Int,
            privateKey: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeBuildSignedOrderbookOrderCancel(
            orderId: ByteArray,
            ownerAccount: ByteArray,
            reason: Int,
            nonce: Long,
            privateKey: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeBuildSignedOrderbookSettlementReceipt(
            receiptId: ByteArray,
            channelId: ByteArray,
            tradeId: ByteArray,
            rangeStart: Long,
            rangeEnd: Long,
            chunkHash: ByteArray,
            bytesDelivered: Long,
            xorDebitedMicroXor: ByteArray,
            providerCreditMicroXor: ByteArray,
            feeAmountMicroXor: ByteArray,
            issuedAtUnix: Long,
            privateKey: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeValidatePdpPayloadJson(
            kind: Int,
            payload: ByteArray,
            label: ByteArray,
            generatedAtUnix: Long,
        ): ByteArray?

        @JvmStatic
        private external fun nativeValidatePdpCommitmentChallengeJson(
            commitment: ByteArray,
            commitmentLabel: ByteArray,
            challenge: ByteArray,
            challengeLabel: ByteArray,
            generatedAtUnix: Long,
        ): ByteArray?

        @JvmStatic
        private external fun nativeValidatePdpChallengeProofJson(
            challenge: ByteArray,
            challengeLabel: ByteArray,
            proof: ByteArray,
            proofLabel: ByteArray,
            generatedAtUnix: Long,
        ): ByteArray?

        @JvmStatic
        private external fun nativeValidatePdpBundleJson(
            commitment: ByteArray,
            commitmentLabel: ByteArray,
            challenge: ByteArray,
            challengeLabel: ByteArray,
            proof: ByteArray,
            proofLabel: ByteArray,
            generatedAtUnix: Long,
        ): ByteArray?
    }
}
