package org.hyperledger.iroha.sdk.offline

/** Settlement proof emitted by Torii after a load or redeem mutation finalizes on-ledger. */
class OfflineMutationSettlement(
    val kind: String,
    val operationId: String,
    val chainTxHash: String,
    val entryHash: String,
    val blockHeight: Long,
    val preStateHash: String,
    val postStateHash: String,
    val settlementCommitmentHex: String,
    val proof: OfflineTransparentZkProof,
) {
    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["kind"] = kind
        map["operation_id"] = operationId
        map["chain_tx_hash"] = chainTxHash
        map["entry_hash"] = entryHash
        map["block_height"] = blockHeight
        map["pre_state_hash"] = preStateHash
        map["post_state_hash"] = postStateHash
        map["settlement_commitment_hex"] = settlementCommitmentHex
        map["proof"] = proof.toJsonMap()
        return map
    }
}
