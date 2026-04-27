package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger

/** Composition leaf plus Merkle inclusion path under a STARK `comp_root`. */
class OfflineStarkCompositionValueV1(
    val leaf: BigInteger,
    val constant: BigInteger,
    val zCoeff: BigInteger,
    auxTerms: List<OfflineStarkCompositionTermV1>,
    val path: OfflineMerklePath,
) {
    private val _auxTerms: List<OfflineStarkCompositionTermV1> = auxTerms.toList()

    val auxTerms: List<OfflineStarkCompositionTermV1> get() = _auxTerms.toList()

    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["leaf"] = leaf
        map["constant"] = constant
        map["z_coeff"] = zCoeff
        map["aux_terms"] = _auxTerms.map { it.toJsonMap() }
        map["path"] = path.toJsonMap()
        return map
    }
}
