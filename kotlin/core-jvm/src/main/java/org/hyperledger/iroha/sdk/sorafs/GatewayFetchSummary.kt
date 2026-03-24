package org.hyperledger.iroha.sdk.sorafs

import org.hyperledger.iroha.sdk.client.JsonParser

/**
 * Immutable view of the JSON summary returned by the SoraFS orchestrator after a gateway fetch.
 */
class GatewayFetchSummary private constructor(
    @JvmField val manifestIdHex: String,
    @JvmField val chunkerHandle: String,
    @JvmField val clientId: String?,
    @JvmField val chunkCount: Long,
    @JvmField val contentLength: Long,
    @JvmField val assembledBytes: Long,
    @JvmField val providerReports: List<ProviderReport>,
    @JvmField val chunkReceipts: List<ChunkReceipt>,
    @JvmField val anonymityPolicy: String,
    @JvmField val anonymityStatus: String,
    @JvmField val anonymityReason: String?,
    @JvmField val anonymitySoranetSelected: Long,
    @JvmField val anonymityPqSelected: Long,
    @JvmField val anonymityClassicalSelected: Long,
    @JvmField val anonymityClassicalRatio: Double,
    @JvmField val anonymityPqRatio: Double,
    @JvmField val anonymityCandidateRatio: Double,
    @JvmField val anonymityDeficitRatio: Double,
    @JvmField val anonymitySupplyDelta: Double,
    @JvmField val anonymityBrownout: Boolean,
    @JvmField val anonymityBrownoutEffective: Boolean,
    @JvmField val anonymityUsesClassical: Boolean,
) {
    /** Provider-level outcome summary. */
    class ProviderReport(
        @JvmField val provider: String,
        @JvmField val successes: Long,
        @JvmField val failures: Long,
        @JvmField val disabled: Boolean,
    )

    /** Chunk-level receipt emitted by the orchestrator. */
    class ChunkReceipt(
        @JvmField val chunkIndex: Int,
        @JvmField val provider: String,
        @JvmField val attempts: Long,
    )

    companion object {
        /** Parses a JSON payload (UTF-8 bytes) into a `GatewayFetchSummary`. */
        @JvmStatic
        fun fromJsonBytes(payload: ByteArray): GatewayFetchSummary {
            require(payload.isNotEmpty()) { "Gateway summary JSON must not be empty" }
            val json = String(payload, Charsets.UTF_8)
            val parsed = JsonParser.parse(json)
            if (parsed !is Map<*, *>) throw SorafsStorageException("Gateway summary must be a JSON object")
            @Suppress("UNCHECKED_CAST")
            return fromJsonObject(parsed as Map<String, Any>)
        }

        @Suppress("UNCHECKED_CAST")
        private fun fromJsonObject(root: Map<String, Any>): GatewayFetchSummary {
            val providerReports = (requireList(root, "provider_reports")).map { item ->
                if (item !is Map<*, *>) throw SorafsStorageException("provider_reports entries must be objects")
                val map = item as Map<String, Any>
                ProviderReport(
                    provider = requireString(map, "provider"),
                    successes = requireLong(map, "successes"),
                    failures = requireLong(map, "failures"),
                    disabled = requireBoolean(map, "disabled"),
                )
            }
            val chunkReceipts = (requireList(root, "chunk_receipts")).map { item ->
                if (item !is Map<*, *>) throw SorafsStorageException("chunk_receipts entries must be objects")
                val map = item as Map<String, Any>
                ChunkReceipt(
                    chunkIndex = requireInt(map, "chunk_index"),
                    provider = requireString(map, "provider"),
                    attempts = requireLong(map, "attempts"),
                )
            }
            return GatewayFetchSummary(
                manifestIdHex = SorafsInputValidator.normalizeHexBytes(
                    requireString(root, "manifest_id_hex"), "manifest_id_hex", 32),
                chunkerHandle = requireString(root, "chunker_handle"),
                clientId = optionalString(root, "client_id"),
                chunkCount = requireLong(root, "chunk_count"),
                contentLength = requireLong(root, "content_length"),
                assembledBytes = requireLong(root, "assembled_bytes"),
                providerReports = providerReports,
                chunkReceipts = chunkReceipts,
                anonymityPolicy = requireString(root, "anonymity_policy"),
                anonymityStatus = requireString(root, "anonymity_status"),
                anonymityReason = optionalString(root, "anonymity_reason"),
                anonymitySoranetSelected = requireLong(root, "anonymity_soranet_selected"),
                anonymityPqSelected = requireLong(root, "anonymity_pq_selected"),
                anonymityClassicalSelected = requireLong(root, "anonymity_classical_selected"),
                anonymityClassicalRatio = requireDouble(root, "anonymity_classical_ratio"),
                anonymityPqRatio = requireDouble(root, "anonymity_pq_ratio"),
                anonymityCandidateRatio = requireDouble(root, "anonymity_candidate_ratio"),
                anonymityDeficitRatio = requireDouble(root, "anonymity_deficit_ratio"),
                anonymitySupplyDelta = requireDouble(root, "anonymity_supply_delta"),
                anonymityBrownout = requireBoolean(root, "anonymity_brownout"),
                anonymityBrownoutEffective = requireBoolean(root, "anonymity_brownout_effective"),
                anonymityUsesClassical = requireBoolean(root, "anonymity_uses_classical"),
            )
        }

        private fun requireString(map: Map<String, Any>, key: String): String {
            val value = map[key]
            if (value is String) return value
            throw SorafsStorageException("Expected string for `$key`")
        }

        private fun optionalString(map: Map<String, Any>, key: String): String? {
            val value = map[key] ?: return null
            if (value is String) return value
            throw SorafsStorageException("Expected string for `$key`")
        }

        private fun requireLong(map: Map<String, Any>, key: String): Long {
            val value = map[key]
            if (value is Number) {
                if (value is Float || value is Double) throw SorafsStorageException("Expected integer for `$key`")
                return value.toLong()
            }
            throw SorafsStorageException("Expected number for `$key`")
        }

        private fun requireInt(map: Map<String, Any>, key: String): Int {
            val value = requireLong(map, key)
            if (value < 0 || value > Int.MAX_VALUE) throw SorafsStorageException("Expected int range for `$key`")
            return value.toInt()
        }

        private fun requireDouble(map: Map<String, Any>, key: String): Double {
            val value = map[key]
            if (value is Number) return value.toDouble()
            throw SorafsStorageException("Expected number for `$key`")
        }

        private fun requireBoolean(map: Map<String, Any>, key: String): Boolean {
            val value = map[key]
            if (value is Boolean) return value
            throw SorafsStorageException("Expected boolean for `$key`")
        }

        @Suppress("UNCHECKED_CAST")
        private fun requireList(map: Map<String, Any>, key: String): List<Any> {
            val value = map[key]
            if (value is List<*>) return value as List<Any>
            throw SorafsStorageException("Expected list for `$key`")
        }
    }
}
