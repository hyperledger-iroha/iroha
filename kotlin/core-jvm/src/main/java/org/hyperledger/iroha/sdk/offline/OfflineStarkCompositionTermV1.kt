package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger

/** Auxiliary composition-polynomial term (wire index, Goldilocks value, coefficient). */
class OfflineStarkCompositionTermV1(
    val wireIndex: Long,
    val value: BigInteger,
    val coeff: BigInteger,
) {
    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["wire_index"] = wireIndex
        map["value"] = value
        map["coeff"] = coeff
        return map
    }
}
