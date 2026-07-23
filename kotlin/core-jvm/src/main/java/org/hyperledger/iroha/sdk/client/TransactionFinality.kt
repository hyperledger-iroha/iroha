package org.hyperledger.iroha.sdk.client

/** Canonical transaction-execution finality checks shared by high-level SDK facades. */
object TransactionFinality {
    /**
     * Require the authoritative global, state-resolved `Applied` envelope for [hashHex].
     */
    @JvmStatic
    fun requireApplied(payload: Map<String, Any>, hashHex: String) {
        check(PipelineStatusExtractor.requireAuthoritativeStatus(payload, hashHex) == "Applied") {
            "Transaction did not reach exact Applied execution finality"
        }
    }
}
