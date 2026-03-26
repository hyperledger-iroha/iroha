package org.hyperledger.iroha.sdk.offline

/**
 * Encodes OfflineSpendReceiptPayload to Norito bytes for signing.
 *
 * This encoder serializes the receipt payload structure using the Norito codec, producing
 * bytes that can be signed by the sender's spend key.
 */
object OfflineSpendReceiptPayloadEncoder {
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

    /**
     * Encode OfflineSpendReceiptPayload to Norito bytes for signing.
     *
     * @param txIdHex 32-byte transaction ID as hex (64 chars)
     * @param fromAccountId sender AccountId (e.g., "<sender_account_i105>")
     * @param toAccountId receiver AccountId
     * @param assetId full asset ID (e.g., "token#domain#<sender_account_i105>")
     * @param amount decimal amount string
     * @param issuedAtMs timestamp in milliseconds
     * @param invoiceId invoice identifier
     * @param platformProofJson JSON-serialized OfflinePlatformProof
     * @param senderCertificateIdHex 32-byte certificate identifier as hex (64 chars)
     * @return Norito-encoded bytes
     * @throws IllegalStateException if native library is not available
     * @throws IllegalArgumentException if any parameter is invalid
     */
    @JvmStatic
    fun encode(
        txIdHex: String?,
        fromAccountId: String?,
        toAccountId: String?,
        assetId: String?,
        amount: String?,
        issuedAtMs: Long,
        invoiceId: String?,
        platformProofJson: String?,
        senderCertificateIdHex: String?,
    ): ByteArray {
        check(NATIVE_AVAILABLE) { "connect_norito_bridge is not available in this runtime" }
        require(txIdHex != null && txIdHex.length == 64) { "txIdHex must be 64 hex characters" }
        require(!fromAccountId.isNullOrEmpty()) { "fromAccountId must not be empty" }
        require(!toAccountId.isNullOrEmpty()) { "toAccountId must not be empty" }
        require(!assetId.isNullOrEmpty()) { "assetId must not be empty" }
        require(!amount.isNullOrEmpty()) { "amount must not be empty" }
        require(invoiceId != null) { "invoiceId must not be null" }
        require(!platformProofJson.isNullOrEmpty()) { "platformProofJson must not be empty" }
        require(senderCertificateIdHex != null && senderCertificateIdHex.length == 64) {
            "senderCertificateIdHex must be 64 hex characters"
        }
        val result = nativeEncode(
            txIdHex, fromAccountId, toAccountId, assetId, amount,
            issuedAtMs, invoiceId, platformProofJson, senderCertificateIdHex,
        )
        checkNotNull(result) { "nativeEncode returned null" }
        return result
    }

    @JvmStatic
    private external fun nativeEncode(
        txIdHex: String,
        fromAccountId: String,
        toAccountId: String,
        assetId: String,
        amount: String,
        issuedAtMs: Long,
        invoiceId: String,
        platformProofJson: String,
        senderCertificateIdHex: String,
    ): ByteArray?
}
