package org.hyperledger.iroha.sdk.nexus

/** Query parameters for `/v1/space-directory/uaids/{uaid}/manifests`. */
class UaidManifestQuery(
    dataspaceId: Long? = null,
    val status: UaidManifestStatusFilter? = null,
    limit: Long? = null,
    offset: Long? = null,
) {
    val dataspaceId: Long? = dataspaceId?.also {
        require(it >= 0) { "dataspaceId must be non-negative" }
    }

    val limit: Long? = limit?.also {
        require(it >= 0) { "limit must be non-negative" }
    }

    val offset: Long? = offset?.also {
        require(it >= 0) { "offset must be non-negative" }
    }

    /** Serialises the query into URL parameters suitable for Torii. */
    fun toQueryParameters(): Map<String, String> = buildMap {
        this@UaidManifestQuery.dataspaceId?.let { put("dataspace", it.toString()) }
        status?.let { put("status", it.parameterValue) }
        this@UaidManifestQuery.limit?.let { put("limit", it.toString()) }
        this@UaidManifestQuery.offset?.let { put("offset", it.toString()) }
    }

    /** Status filter accepted by Torii manifests endpoint. */
    enum class UaidManifestStatusFilter(val parameterValue: String) {
        ACTIVE("active"),
        INACTIVE("inactive"),
        ALL("all"),
    }
}
