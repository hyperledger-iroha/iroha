package org.hyperledger.iroha.sdk.offline

/** STARK V1 proof object: commitments, per-query fold chains, optional composition leaves and AIR. */
class OfflineStarkProofV1(
    val version: Int,
    val commits: OfflineStarkCommitmentsV1,
    queries: List<List<OfflineFoldDecommitV1>>,
    compValues: List<OfflineStarkCompositionValueV1>?,
    val air: OfflineStarkAirProofV1?,
) {
    private val _queries: List<List<OfflineFoldDecommitV1>> = queries.map { it.toList() }
    private val _compValues: List<OfflineStarkCompositionValueV1>? = compValues?.toList()

    val queries: List<List<OfflineFoldDecommitV1>> get() = _queries.map { it.toList() }
    val compValues: List<OfflineStarkCompositionValueV1>? get() = _compValues?.toList()

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
