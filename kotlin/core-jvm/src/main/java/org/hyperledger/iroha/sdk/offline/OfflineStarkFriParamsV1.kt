package org.hyperledger.iroha.sdk.offline

/** Parameters describing a STARK/FRI verification envelope. */
class OfflineStarkFriParamsV1(
    val version: Int,
    val nLog2: Int,
    val blowupLog2: Int,
    val foldArity: Int,
    val queries: Int,
    val merkleArity: Int,
    val hashFn: Int,
    val domainTag: String,
) {
    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["version"] = version.toLong()
        map["n_log2"] = nLog2.toLong()
        map["blowup_log2"] = blowupLog2.toLong()
        map["fold_arity"] = foldArity.toLong()
        map["queries"] = queries.toLong()
        map["merkle_arity"] = merkleArity.toLong()
        map["hash_fn"] = hashFn.toLong()
        map["domain_tag"] = domainTag
        return map
    }
}
