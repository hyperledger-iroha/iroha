package org.hyperledger.iroha.sdk.offline

/** Parsed response from `/v1/offline/bundle/proof_status`. */
class OfflineBundleProofStatus(
    val bundleIdHex: String,
    val receiptsRootHex: String,
    val aggregateProofRootHex: String?,
    val receiptsRootMatches: Boolean?,
    val proofStatus: String,
    val proofSummary: ProofSummary?,
) {
    /** Aggregate proof metadata summary. */
    class ProofSummary(
        val version: Int,
        val proofSumBytes: Long?,
        val proofCounterBytes: Long?,
        val proofReplayBytes: Long?,
        metadataKeys: List<String>,
    ) {
        private val _metadataKeys: List<String> = metadataKeys.toList()

        val metadataKeys: List<String> get() = _metadataKeys
    }
}
