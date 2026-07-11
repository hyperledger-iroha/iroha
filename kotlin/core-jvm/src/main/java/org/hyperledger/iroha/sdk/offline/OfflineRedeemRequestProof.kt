package org.hyperledger.iroha.sdk.offline

/** Proof artifact binding an offline redemption request to its public inputs. */
class OfflineRedeemRequestProof(
    val backend: String,
    val circuitId: String,
    val recursionDepth: Int,
    val publicInputsHex: String,
    val envelope: OfflineStarkVerifyEnvelope,
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
