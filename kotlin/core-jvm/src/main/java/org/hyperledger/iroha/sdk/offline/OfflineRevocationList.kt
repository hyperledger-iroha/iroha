package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.client.JsonParser

/**
 * Immutable view over `/v1/offline/revocations` responses.
 *
 * Items surface both the flattened fields (verdict id, issuer, revoked timestamp, etc.) and the
 * raw Norito JSON so POS clients can mirror ledger metadata without reimplementing codecs.
 */
class OfflineRevocationList(
    items: List<OfflineRevocationItem>,
    val total: Long,
) {
    val items: List<OfflineRevocationItem> = items.toList()

    /** Individual revocation entry returned by Torii. */
    class OfflineRevocationItem(
        val verdictIdHex: String,
        val issuerId: String,
        val issuerDisplay: String,
        val revokedAtMs: Long,
        val reason: String,
        val note: String?,
        /** Raw metadata JSON attached by the issuer (may be null when no metadata was supplied). */
        val metadataJson: String?,
        /** Raw Norito JSON representation of the revocation record. */
        val recordJson: String?,
    ) {
        /**
         * Parses the optional metadata JSON into an immutable map representation.
         *
         * @return structured metadata map or an empty map when metadata was omitted
         * @throws IllegalStateException if the JSON payload is malformed or not an object
         */
        @Suppress("UNCHECKED_CAST")
        fun metadataAsMap(): Map<String, Any> {
            if (metadataJson.isNullOrBlank()) return emptyMap()
            val parsed = JsonParser.parse(metadataJson)
            check(parsed is Map<*, *>) { "metadata is not a JSON object" }
            return java.util.Collections.unmodifiableMap(parsed as Map<String, Any>)
        }

        /**
         * Parses the raw record JSON into an immutable map representation.
         *
         * @throws IllegalStateException if the JSON payload is malformed or not an object
         */
        @Suppress("UNCHECKED_CAST")
        fun recordAsMap(): Map<String, Any> {
            if (recordJson.isNullOrBlank()) return emptyMap()
            val parsed = JsonParser.parse(recordJson)
            check(parsed is Map<*, *>) { "record is not a JSON object" }
            return java.util.Collections.unmodifiableMap(parsed as Map<String, Any>)
        }
    }
}
