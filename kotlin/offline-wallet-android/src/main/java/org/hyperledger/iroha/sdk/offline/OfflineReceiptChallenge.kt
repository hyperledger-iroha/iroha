package org.hyperledger.iroha.sdk.offline

/** Wrapper around the native offline receipt challenge encoder. */
class OfflineReceiptChallenge private constructor(
    private val _preimage: ByteArray,
    private val _irohaHash: ByteArray,
    private val _clientDataHash: ByteArray,
) {
    val preimage: ByteArray get() = _preimage.copyOf()
    val irohaHash: ByteArray get() = _irohaHash.copyOf()
    val clientDataHash: ByteArray get() = _clientDataHash.copyOf()

    companion object {
        private val NATIVE_AVAILABLE: Boolean

        init {
            var available: Boolean
            try {
                System.loadLibrary("connect_norito_bridge")
                available = true
            } catch (_: UnsatisfiedLinkError) {
                available = false
            }
            NATIVE_AVAILABLE = available
        }

        @JvmStatic
        fun isNativeAvailable(): Boolean = NATIVE_AVAILABLE

        @JvmStatic
        fun compute(
            chainId: String,
            invoiceId: String,
            receiverAccountId: String,
            assetId: String,
            amount: String,
            issuedAtMs: Long,
            senderCertificateIdHex: String,
            nonceHex: String,
        ): OfflineReceiptChallenge {
            require(!chainId.isNullOrBlank()) { "chainId must not be empty" }
            requireCertificateIdHex(senderCertificateIdHex)
            requireNumericAmount(amount)
            return computeInternal(
                chainId, invoiceId, receiverAccountId, assetId, amount,
                issuedAtMs, senderCertificateIdHex, nonceHex,
            )
        }

        @JvmStatic
        fun compute(
            chainId: String,
            invoiceId: String,
            receiverAccountId: String,
            assetId: String,
            amount: String,
            issuedAtMs: Long,
            senderCertificateIdHex: String,
            nonceHex: String,
            expectedScale: Int,
        ): OfflineReceiptChallenge {
            require(!chainId.isNullOrBlank()) { "chainId must not be empty" }
            require(expectedScale >= 0) { "expectedScale must be non-negative" }
            requireCertificateIdHex(senderCertificateIdHex)
            val scale = requireNumericAmount(amount)
            require(scale == expectedScale) { "amount must use scale $expectedScale: $amount" }
            return computeInternal(
                chainId, invoiceId, receiverAccountId, assetId, amount,
                issuedAtMs, senderCertificateIdHex, nonceHex,
            )
        }

        private fun computeInternal(
            chainId: String,
            invoiceId: String,
            receiverAccountId: String,
            assetId: String,
            amount: String,
            issuedAtMs: Long,
            senderCertificateIdHex: String,
            nonceHex: String,
        ): OfflineReceiptChallenge {
            check(NATIVE_AVAILABLE) { "connect_norito_bridge is not available in this runtime" }
            val irohaHash = ByteArray(32)
            val clientHash = ByteArray(32)
            val preimage = nativeCompute(
                chainId, invoiceId, receiverAccountId, assetId, amount,
                issuedAtMs, senderCertificateIdHex, nonceHex, irohaHash, clientHash,
            )
            checkNotNull(preimage) { "connect_norito_offline_receipt_challenge failed" }
            return OfflineReceiptChallenge(preimage, irohaHash, clientHash)
        }

        @JvmStatic
        private external fun nativeCompute(
            chainId: String,
            invoiceId: String,
            receiverAccountId: String,
            assetId: String,
            amount: String,
            issuedAtMs: Long,
            senderCertificateIdHex: String,
            nonceHex: String,
            irohaHashOut: ByteArray,
            clientHashOut: ByteArray,
        ): ByteArray?

        private fun requireCertificateIdHex(senderCertificateIdHex: String?) {
            require(senderCertificateIdHex != null && senderCertificateIdHex.length == 64) {
                "senderCertificateIdHex must be 64 hex characters"
            }
        }

        private fun requireNumericAmount(amount: String?): Int {
            requireNotNull(amount) { "amount must not be null" }
            val trimmed = amount.trim()
            require(trimmed.isNotEmpty()) { "amount must not be empty" }
            var seenDot = false
            var scale = 0
            var index = 0
            val first = trimmed[0]
            if (first == '+' || first == '-') index = 1
            require(index < trimmed.length) { "amount must be numeric: $amount" }
            var seenDigit = false
            for (i in index until trimmed.length) {
                val ch = trimmed[i]
                if (ch == '.') {
                    require(!seenDot) { "amount must be numeric: $amount" }
                    seenDot = true
                    continue
                }
                require(ch in '0'..'9') { "amount must be numeric: $amount" }
                seenDigit = true
                if (seenDot) scale++
            }
            require(seenDigit) { "amount must be numeric: $amount" }
            require(scale <= 28) { "amount scale exceeds 28: $amount" }
            return scale
        }
    }
}
