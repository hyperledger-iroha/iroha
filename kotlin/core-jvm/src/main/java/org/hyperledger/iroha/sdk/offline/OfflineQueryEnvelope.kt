package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser

/** JSON envelope used by the Torii offline query POST handlers. */
class OfflineQueryEnvelope private constructor(
    private val filter: Any?,
    private val sort: Any?,
    private val limit: Long?,
    private val offset: Long?,
    private val addressFormat: String?,
) {
    fun toJsonBytes(): ByteArray {
        val json = LinkedHashMap<String, Any>()
        if (filter != null) json["filter"] = filter
        if (sort != null) json["sort"] = sort
        if (limit != null) json["limit"] = limit
        if (offset != null) json["offset"] = offset
        if (!addressFormat.isNullOrBlank()) json["address_format"] = addressFormat
        return JsonEncoder.encode(json).toByteArray(Charsets.UTF_8)
    }

    class Builder internal constructor() {
        private var filter: Any? = null
        private var sort: Any? = null
        private var limit: Long? = null
        private var offset: Long? = null
        private var addressFormat: String? = null

        fun filterJson(json: String?) = apply { this.filter = parseJson(json) }
        fun sortJson(json: String?) = apply { this.sort = parseJson(json) }
        internal fun setFilterNode(filter: Any?) = apply { this.filter = filter }
        internal fun setSortNode(sort: Any?) = apply { this.sort = sort }

        fun setLimit(limit: Long?) = apply {
            require(limit == null || limit >= 0) { "limit must be positive" }
            this.limit = limit
        }

        fun setOffset(offset: Long?) = apply {
            require(offset == null || offset >= 0) { "offset must be positive" }
            this.offset = offset
        }

        fun setAddressFormat(addressFormat: String?) = apply {
            this.addressFormat = addressFormat
        }

        fun build(): OfflineQueryEnvelope =
            OfflineQueryEnvelope(filter, sort, limit, offset, addressFormat)
    }

    companion object {
        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromListParams(params: OfflineListParams?): OfflineQueryEnvelope {
            if (params == null) return builder().build()
            val b = builder()
            params.filter?.let { parseJson(it) }?.let { b.setFilterNode(it) }
            params.sort?.let { parseJson(it) }?.let { b.setSortNode(it) }
            params.limit?.let { b.setLimit(it) }
            params.offset?.let { b.setOffset(it) }
            params.addressFormat?.let { b.setAddressFormat(it) }
            return b.build()
        }

        private fun parseJson(json: String?): Any? {
            val trimmed = json?.trim().orEmpty()
            if (trimmed.isEmpty()) return null
            return JsonParser.parse(trimmed)
        }
    }
}
