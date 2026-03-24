package org.hyperledger.iroha.sdk.offline

/** Response payload for issuing an operator-signed offline build claim. */
class OfflineBuildClaimIssueResponse(
    val claimIdHex: String,
    buildClaim: Map<String, Any>,
    val typedBuildClaim: OfflineBuildClaim,
) {
    private val _buildClaim: Map<String, Any> = buildClaim.toMap()

    val buildClaim: Map<String, Any> get() = _buildClaim
}
