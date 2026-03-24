package org.hyperledger.iroha.sdk.offline

/** Immutable log entry describing an offline transfer for audit exports. */
class OfflineAuditEntry(
    val txId: String,
    val senderId: String,
    val receiverId: String,
    val assetId: String,
    val amount: String,
    val timestampMs: Long,
) {
    internal fun toJson(): Map<String, Any> {
        val json = LinkedHashMap<String, Any>()
        json["tx_id"] = txId
        json["sender_id"] = senderId
        json["receiver_id"] = receiverId
        json["asset_id"] = assetId
        json["amount"] = amount
        json["timestamp_ms"] = timestampMs
        return json
    }

    internal companion object {
        @JvmStatic
        fun fromJsonMap(json: Map<String, Any>): OfflineAuditEntry =
            OfflineAuditEntry(
                txId = requireString(json["tx_id"], "tx_id"),
                senderId = requireString(json["sender_id"], "sender_id"),
                receiverId = requireString(json["receiver_id"], "receiver_id"),
                assetId = requireString(json["asset_id"], "asset_id"),
                amount = requireString(json["amount"], "amount"),
                timestampMs = requireLong(json["timestamp_ms"], "timestamp_ms"),
            )

        private fun requireString(value: Any?, field: String): String {
            checkNotNull(value) { "$field is missing" }
            if (value is String) return value
            return value.toString()
        }

        private fun requireLong(value: Any?, field: String): Long {
            check(value is Number) { "$field is not numeric" }
            check(value !is Float && value !is Double) { "$field must be an integer" }
            return value.toLong()
        }
    }
}
