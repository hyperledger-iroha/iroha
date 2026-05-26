package org.hyperledger.iroha.sdk.offline

/** Top-level STARK/FRI verification envelope. */
class OfflineStarkVerifyEnvelope(
    val params: OfflineStarkFriParams,
    val proof: OfflineStarkProof,
    val transcriptLabel: String,
) {
    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["params"] = params.toJsonMap()
        map["proof"] = proof.toJsonMap()
        map["transcript_label"] = transcriptLabel
        return map
    }
}
