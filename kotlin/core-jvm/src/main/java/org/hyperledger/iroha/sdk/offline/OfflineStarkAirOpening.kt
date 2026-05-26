package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger

/** Sampled AIR trace row plus composition evaluation for a single FRI query. */
class OfflineStarkAirOpening(
    val index: Long,
    row: List<BigInteger>,
    nextRow: List<BigInteger>,
    val rowPath: OfflineMerklePath,
    val nextRowPath: OfflineMerklePath,
    val compositionValue: BigInteger,
    val compositionPath: OfflineMerklePath,
) {
    private val _row: List<BigInteger> = row.toList()
    private val _nextRow: List<BigInteger> = nextRow.toList()

    val row: List<BigInteger> get() = _row.toList()
    val nextRow: List<BigInteger> get() = _nextRow.toList()

    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["index"] = index
        map["row"] = _row
        map["next_row"] = _nextRow
        map["row_path"] = rowPath.toJsonMap()
        map["next_row_path"] = nextRowPath.toJsonMap()
        map["composition_value"] = compositionValue
        map["composition_path"] = compositionPath.toJsonMap()
        return map
    }
}
