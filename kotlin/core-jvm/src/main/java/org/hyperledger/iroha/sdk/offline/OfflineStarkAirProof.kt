package org.hyperledger.iroha.sdk.offline

/** Verifier-owned AIR statement embedded in a STARK envelope. */
class OfflineStarkAirProof(
    val version: Int,
    val circuitId: String,
    publicDigest: ByteArray,
    traceRoot: ByteArray,
    compositionRoot: ByteArray,
    val traceWidth: Int,
    openings: List<OfflineStarkAirOpening>,
) {
    private val _publicDigest: ByteArray = publicDigest.copyOf()
    private val _traceRoot: ByteArray = traceRoot.copyOf()
    private val _compositionRoot: ByteArray = compositionRoot.copyOf()
    private val _openings: List<OfflineStarkAirOpening> = openings.toList()

    val publicDigest: ByteArray get() = _publicDigest.copyOf()
    val traceRoot: ByteArray get() = _traceRoot.copyOf()
    val compositionRoot: ByteArray get() = _compositionRoot.copyOf()
    val openings: List<OfflineStarkAirOpening> get() = _openings.toList()

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
