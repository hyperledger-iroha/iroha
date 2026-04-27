package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.client.JsonEncoder

/** Canonicalization helpers for the `/v1/offline/cash/` route family. */
object OfflineCashCodec {

    /** Canonical amount matching Rust `iroha_primitives::numeric::Numeric::to_string`. */
    @JvmStatic
    fun canonicalAmountString(amount: String): String {
        return parseNumeric(amount).canonicalString()
    }

    /** Lexicographically sorted `"{transferId}:{localRevision}"` keys for a receipt list. */
    @JvmStatic
    fun receiptKeys(receipts: List<OfflineTransferReceipt>): List<String> =
        receipts.map { "${it.transferId}:${it.localRevision}" }.sorted()

    /**
     * SHA-256 hex of canonical JSON of the redeem-request commitment payload, matching the Torii
     * `redeem_request_commitment_hex` helper.
     */
    @JvmStatic
    fun redeemRequestCommitmentHex(
        operationId: String,
        accountId: String,
        lineageId: String,
        assetDefinitionId: String,
        amountCanonical: String,
        offlinePublicKey: String,
        authorizationId: String,
        preStateHash: String,
        receipts: List<OfflineTransferReceipt>,
    ): String {
        val payload = LinkedHashMap<String, Any?>()
        payload["operation_id"] = operationId
        payload["kind"] = "redeem_request"
        payload["account_id"] = accountId
        payload["lineage_id"] = lineageId
        payload["asset_definition_id"] = assetDefinitionId
        payload["amount"] = amountCanonical
        payload["offline_public_key"] = offlinePublicKey
        payload["authorization_id"] = authorizationId
        payload["pre_state_hash"] = preStateHash
        payload["receipt_keys"] = receiptKeys(receipts)
        val canonical = JsonEncoder.encode(payload).toByteArray(StandardCharsets.UTF_8)
        return sha256Hex(canonical)
    }

    /**
     * Stable idempotency key header value for mutating cash routes.
     *
     * Exact port of `ToriiClient.offlineCashIdempotencyKey` from IrohaSwift
     * (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11885-11909`):
     * operation-id scoped for load / refresh / sync / redeem; a SHA-256 fingerprint of the
     * request path + encoded body for setup (which has no `operation_id`).
     */
    @JvmStatic
    fun stableIdempotencyKey(request: Any, path: String, encodedBody: ByteArray): String = when (request) {
        is OfflineCashLoadRequest -> "offline-cash:${request.operationId}"
        is OfflineCashRefreshRequest -> "offline-cash:${request.operationId}"
        is OfflineCashSyncRequest -> "offline-cash:${request.operationId}"
        is OfflineCashRedeemRequest -> "offline-cash:${request.operationId}"
        is OfflineCashSetupRequest -> {
            val digestInput = ByteArray(path.toByteArray(StandardCharsets.UTF_8).size + 1 + encodedBody.size)
            val pathBytes = path.toByteArray(StandardCharsets.UTF_8)
            System.arraycopy(pathBytes, 0, digestInput, 0, pathBytes.size)
            digestInput[pathBytes.size] = 0
            System.arraycopy(encodedBody, 0, digestInput, pathBytes.size + 1, encodedBody.size)
            "offline-cash:setup:${sha256Hex(digestInput)}"
        }
        else -> error("Unsupported cash request type for Idempotency-Key: ${request::class.java.name}")
    }

    private fun sha256Hex(bytes: ByteArray): String {
        val digest = MessageDigest.getInstance("SHA-256").digest(bytes)
        val sb = StringBuilder(digest.size * 2)
        for (b in digest) {
            val hi = (b.toInt() ushr 4) and 0x0f
            val lo = b.toInt() and 0x0f
            sb.append(HEX_DIGITS[hi]).append(HEX_DIGITS[lo])
        }
        return sb.toString()
    }

    private val HEX_DIGITS = charArrayOf(
        '0', '1', '2', '3', '4', '5', '6', '7',
        '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    )

    private fun parseNumeric(raw: String): NumericAmount {
        val trimmed = raw.trim()
        require(trimmed.isNotEmpty()) { "amount must not be blank" }

        var index = 0
        var negative = false
        if (trimmed[index] == '-' || trimmed[index] == '+') {
            negative = trimmed[index] == '-'
            index++
        }

        var seenDot = false
        var scale = 0
        val digits = StringBuilder()
        while (index < trimmed.length) {
            val c = trimmed[index++]
            if (c == '.') {
                require(!seenDot) { "amount must contain at most one decimal point" }
                seenDot = true
                continue
            }
            require(c in '0'..'9') { "amount must contain only decimal digits" }
            digits.append(c)
            if (seenDot) {
                scale++
                require(scale <= MAX_NUMERIC_SCALE) {
                    "amount scale exceeds Iroha limit of $MAX_NUMERIC_SCALE"
                }
            }
        }
        require(digits.isNotEmpty()) { "amount must contain at least one digit" }

        val magnitudeDigits = digits.toString()
        var mantissa = BigInteger(magnitudeDigits)
        if (negative) mantissa = mantissa.negate()
        require(mantissa.toByteArray().size <= MAX_NUMERIC_BYTES) {
            "amount mantissa exceeds Iroha limit of $MAX_NUMERIC_BYTES signed bytes"
        }

        val normalizedDigits = magnitudeDigits.trimStart('0').ifEmpty { "0" }
        return NumericAmount(
            negative = negative && normalizedDigits != "0",
            scale = scale,
            digits = normalizedDigits,
        )
    }

    private class NumericAmount(
        private val negative: Boolean,
        private val scale: Int,
        private val digits: String,
    ) {
        fun canonicalString(): String {
            if (scale == 0) {
                return if (negative) "-$digits" else digits
            }
            val formatted = StringBuilder(digits)
            while (formatted.length <= scale) {
                formatted.insert(0, '0')
            }
            val split = formatted.length - scale
            val body = formatted.substring(0, split) + "." + formatted.substring(split)
            return if (negative) "-$body" else body
        }
    }

    private const val MAX_NUMERIC_SCALE = 28
    private const val MAX_NUMERIC_BYTES = 64
}
