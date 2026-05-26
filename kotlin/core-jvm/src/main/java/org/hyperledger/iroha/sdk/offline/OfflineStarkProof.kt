package org.hyperledger.iroha.sdk.offline

/** STARK proof object: commitments, per-query fold chains, optional composition leaves and AIR. */
class OfflineStarkProof(
    val version: Int,
    val commits: OfflineStarkCommitments,
    queries: List<List<OfflineFoldDecommit>>,
    compValues: List<OfflineStarkCompositionValue>?,
    val air: OfflineStarkAirProof?,
) {
    private val _queries: List<List<OfflineFoldDecommit>> = queries.map { it.toList() }
    private val _compValues: List<OfflineStarkCompositionValue>? = compValues?.toList()

    val queries: List<List<OfflineFoldDecommit>> get() = _queries.map { it.toList() }
    val compValues: List<OfflineStarkCompositionValue>? get() = _compValues?.toList()

    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["version"] = version.toLong()
        map["commits"] = commits.toJsonMap()
        map["queries"] = _queries.map { chain -> chain.map { it.toJsonMap() } }
        map["comp_values"] = _compValues?.map { it.toJsonMap() }
        map["air"] = air?.toJsonMap()
        return map
    }
}
