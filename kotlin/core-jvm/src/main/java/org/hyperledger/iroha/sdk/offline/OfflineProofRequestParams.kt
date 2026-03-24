package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.client.JsonEncoder

/** Parameters accepted by the Torii `/v1/offline/transfers/proof` endpoint; requires a transfer payload. */
class OfflineProofRequestParams(
    transferPayload: Map<String, Any>,
    val kind: OfflineProofRequestKind,
    val counterCheckpoint: Long? = null,
    val replayLogHeadHex: String? = null,
    val replayLogTailHex: String? = null,
) {
    private val _transferPayload: Map<String, Any> = transferPayload.toMap()

    init {
        require(_transferPayload.isNotEmpty()) { "transferPayload must not be empty" }
        if (kind == OfflineProofRequestKind.REPLAY) {
            require(replayLogHeadHex != null && replayLogTailHex != null) {
                "replay proofs require replayLogHeadHex and replayLogTailHex"
            }
        } else {
            require(replayLogHeadHex == null && replayLogTailHex == null) {
                "replayLogHeadHex/replayLogTailHex are only valid for replay proofs"
            }
        }
    }

    fun toJsonBytes(): ByteArray {
        val body = LinkedHashMap<String, Any>()
        body["transfer"] = _transferPayload
        body["kind"] = kind.asParameter()
        if (counterCheckpoint != null) body["counter_checkpoint"] = counterCheckpoint
        if (replayLogHeadHex != null) body["replay_log_head_hex"] = replayLogHeadHex
        if (replayLogTailHex != null) body["replay_log_tail_hex"] = replayLogTailHex
        return JsonEncoder.encode(body).toByteArray(Charsets.UTF_8)
    }
}
