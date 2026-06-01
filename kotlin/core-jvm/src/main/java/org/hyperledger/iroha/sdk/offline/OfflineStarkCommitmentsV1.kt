package org.hyperledger.iroha.sdk.offline

/** Merkle commitment roots per FRI layer plus the optional composition root. */
class OfflineStarkCommitmentsV1(
    val version: Int,
    roots: List<ByteArray>,
    compRoot: ByteArray?,
) {
    private val _roots: List<ByteArray> = roots.map { it.copyOf() }
    private val _compRoot: ByteArray? = compRoot?.copyOf()

    val roots: List<ByteArray> get() = _roots.map { it.copyOf() }
    val compRoot: ByteArray? get() = _compRoot?.copyOf()

    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["version"] = version.toLong()
        map["roots"] = _roots.map { OfflineMerklePath.encodeBytesAsHex(it) }
        map["comp_root"] = _compRoot?.let { OfflineMerklePath.encodeBytesAsHex(it) }
        return map
    }
}
