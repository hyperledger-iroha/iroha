package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger

/** FRI fold decommitment at one recursion layer. */
class OfflineFoldDecommit(
    val j: Long,
    val y0: BigInteger,
    val y1: BigInteger,
    val pathY0: OfflineMerklePath,
    val pathY1: OfflineMerklePath,
    val z: BigInteger,
    val pathZ: OfflineMerklePath,
) {
    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["j"] = j
        map["y0"] = y0
        map["y1"] = y1
        map["path_y0"] = pathY0.toJsonMap()
        map["path_y1"] = pathY1.toJsonMap()
        map["z"] = z
        map["path_z"] = pathZ.toJsonMap()
        return map
    }
}
