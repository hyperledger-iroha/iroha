package org.hyperledger.iroha.sdk.offline

/** Top-level STARK/FRI verification envelope. */
class OfflineStarkVerifyEnvelopeV1(
    val params: OfflineStarkFriParamsV1,
    val proof: OfflineStarkProofV1,
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
