package org.hyperledger.iroha.sdk.offline

/** Verifier-owned AIR statement embedded in a V1 STARK envelope. */
class OfflineStarkAirProofV1(
    val version: Int,
    val circuitId: String,
    publicDigest: ByteArray,
    traceRoot: ByteArray,
    compositionRoot: ByteArray,
    val traceWidth: Int,
    openings: List<OfflineStarkAirOpeningV1>,
) {
    private val _publicDigest: ByteArray = publicDigest.copyOf()
    private val _traceRoot: ByteArray = traceRoot.copyOf()
    private val _compositionRoot: ByteArray = compositionRoot.copyOf()
    private val _openings: List<OfflineStarkAirOpeningV1> = openings.toList()

    val publicDigest: ByteArray get() = _publicDigest.copyOf()
    val traceRoot: ByteArray get() = _traceRoot.copyOf()
    val compositionRoot: ByteArray get() = _compositionRoot.copyOf()
    val openings: List<OfflineStarkAirOpeningV1> get() = _openings.toList()

    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["version"] = version.toLong()
        map["circuit_id"] = circuitId
        map["public_digest"] = OfflineMerklePath.encodeBytesAsHex(_publicDigest)
        map["trace_root"] = OfflineMerklePath.encodeBytesAsHex(_traceRoot)
        map["composition_root"] = OfflineMerklePath.encodeBytesAsHex(_compositionRoot)
        map["trace_width"] = traceWidth.toLong()
        map["openings"] = _openings.map { it.toJsonMap() }
        return map
    }
}
