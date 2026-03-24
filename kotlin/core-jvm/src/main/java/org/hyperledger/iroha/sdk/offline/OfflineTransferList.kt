package org.hyperledger.iroha.sdk.offline

import java.util.Optional
import org.hyperledger.iroha.sdk.client.JsonParser

/** Immutable view over `/v1/offline/transfers` responses. */
class OfflineTransferList(
    items: List<OfflineTransferItem>,
    val total: Long,
) {
    val items: List<OfflineTransferItem> = items.toList()

    class OfflineTransferItem(
        val bundleIdHex: String,
        val receiverId: String,
        val receiverDisplay: String,
        val depositAccountId: String,
        val depositAccountDisplay: String,
        val assetId: String?,
        val receiptCount: Long,
        val totalAmount: String,
        val claimedDelta: String,
        val status: String?,
        val recordedAtMs: Long?,
        val recordedAtHeight: Long?,
        val statusTransitionsJson: String?,
        val platformPolicy: String?,
        val platformTokenSnapshot: PlatformTokenSnapshot?,
        /** Raw Norito JSON of the submitted bundle. */
        val transferJson: String?,
    ) {
        constructor(
            bundleIdHex: String,
            receiverId: String,
            receiverDisplay: String,
            depositAccountId: String,
            depositAccountDisplay: String,
            assetId: String?,
            receiptCount: Long,
            totalAmount: String,
            claimedDelta: String,
            platformPolicy: String?,
            platformTokenSnapshot: PlatformTokenSnapshot?,
            transferJson: String?,
        ) : this(
            bundleIdHex, receiverId, receiverDisplay, depositAccountId,
            depositAccountDisplay, assetId, receiptCount, totalAmount, claimedDelta,
            null, null, null, null, platformPolicy, platformTokenSnapshot, transferJson,
        )

        /**
         * Serialises this transfer item into a JSON-ready map.
         *
         * Use [org.hyperledger.iroha.sdk.client.JsonEncoder.encode] to cache the
         * resulting structure locally.
         */
        fun toJsonMap(): Map<String, Any> {
            val json = LinkedHashMap<String, Any>()
            json["bundle_id_hex"] = bundleIdHex
            json["receiver_id"] = receiverId
            json["receiver_display"] = receiverDisplay
            json["deposit_account_id"] = depositAccountId
            json["deposit_account_display"] = depositAccountDisplay
            if (assetId != null) json["asset_id"] = assetId
            json["receipt_count"] = receiptCount
            json["total_amount"] = totalAmount
            json["claimed_delta"] = claimedDelta
            if (status != null) json["status"] = status
            if (recordedAtMs != null) json["recorded_at_ms"] = recordedAtMs
            if (recordedAtHeight != null) json["recorded_at_height"] = recordedAtHeight
            if (!statusTransitionsJson.isNullOrBlank()) {
                json["status_transitions"] = JsonParser.parse(statusTransitionsJson) as Any
            }
            if (platformPolicy != null) json["platform_policy"] = platformPolicy
            if (platformTokenSnapshot != null) {
                json["platform_token_snapshot"] = platformTokenSnapshot.toJsonMap()
            }
            json["transfer"] = transferAsMap()
            return java.util.Collections.unmodifiableMap(json)
        }

        /**
         * Parses the raw transfer JSON into an immutable map representation.
         *
         * @throws IllegalStateException if the JSON payload is malformed or not an object
         */
        @Suppress("UNCHECKED_CAST")
        fun transferAsMap(): Map<String, Any> {
            if (transferJson.isNullOrBlank()) return emptyMap()
            val parsed = JsonParser.parse(transferJson)
            check(parsed is Map<*, *>) { "transfer is not a JSON object" }
            return java.util.Collections.unmodifiableMap(parsed as Map<String, Any>)
        }

        /**
         * Returns a summary of the first receipt embedded in the transfer payload, when present.
         *
         * Transfers aggregate multiple receipts; for audit logging we only need the first entry to
         * capture the sender/receiver/asset context associated with the bundle.
         */
        @Suppress("UNCHECKED_CAST")
        fun firstReceiptSummary(): Optional<ReceiptSummary> {
            val payload = transferAsMap()
            if (payload.isEmpty()) return Optional.empty()
            val receiptsNode = payload["receipts"]
            if (receiptsNode !is List<*> || receiptsNode.isEmpty()) return Optional.empty()
            val first = receiptsNode[0]
            if (first !is Map<*, *>) return Optional.empty()
            val receipt = first as Map<String, Any>
            val sender = stringValue(receipt["from"])
            val receiver = stringValue(receipt["to"])
            val asset = stringValue(receipt["asset"])
            val amount = stringValue(receipt["amount"])
            if (sender == null || receiver == null || amount == null) return Optional.empty()
            return Optional.of(ReceiptSummary(sender, receiver, asset, amount))
        }

        /** Lightweight projection of receipt details for audit logging. */
        class ReceiptSummary internal constructor(
            val senderId: String,
            val receiverId: String,
            val assetId: String?,
            val amount: String,
        )

        /** Snapshot of the platform token captured at settlement time. */
        class PlatformTokenSnapshot(
            val policy: String,
            val attestationJwsB64: String,
        ) {
            /** Returns a JSON-ready map representation of the snapshot. */
            fun toJsonMap(): Map<String, String> {
                val snapshot = LinkedHashMap<String, String>()
                snapshot["policy"] = policy
                snapshot["attestation_jws_b64"] = attestationJwsB64
                return java.util.Collections.unmodifiableMap(snapshot)
            }
        }

        companion object {
            private fun stringValue(value: Any?): String? {
                if (value == null) return null
                if (value is String) return value
                val converted = value.toString().trim()
                return converted.ifEmpty { null }
            }
        }
    }
}
