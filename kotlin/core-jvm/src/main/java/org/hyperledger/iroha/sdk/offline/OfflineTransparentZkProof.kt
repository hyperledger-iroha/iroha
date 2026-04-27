package org.hyperledger.iroha.sdk.offline

/** Transparent ZK proof payload binding a settlement commitment to a STARK envelope. */
class OfflineTransparentZkProof(
    val backend: String,
    val circuitId: String,
    val recursionDepth: Int,
    val publicInputsHex: String,
    val envelope: OfflineStarkVerifyEnvelopeV1,
) {
    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["backend"] = backend
        map["circuit_id"] = circuitId
        map["recursion_depth"] = recursionDepth.toLong()
        map["public_inputs_hex"] = publicInputsHex
        map["envelope"] = envelope.toJsonMap()
        return map
    }
}
