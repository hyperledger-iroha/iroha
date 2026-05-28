package org.hyperledger.iroha.sdk.offline

/** Client-side builder for offline settlement proofs required by the cash routes. */
object OfflineSettlementProofs {

    private const val BACKEND: String = "stark/fri/sha256-goldilocks"
    private const val REDEEM_REQUEST_CIRCUIT_ID: String = "offline-bearer-redeem-request"
    private const val RECURSION_DEPTH: Int = 1

    /**
     * Build the redeem-request proof for the legacy cash redeem payload shape.
     * Output is pinned against the committed Rust-generated fixtures.
     */
    @JvmStatic
    fun buildRedeemRequestProof(
        operationId: String,
        accountId: String,
        lineageId: String,
        assetDefinitionId: String,
        amount: String,
        offlinePublicKey: String,
        authorizationId: String,
        preStateHash: String,
        receipts: List<OfflineTransferReceipt>,
    ): OfflineRedeemRequestProof {
        val canonicalAmount = OfflineCashCodec.canonicalAmountString(amount)
        val commitmentHex = OfflineCashCodec.redeemRequestCommitmentHex(
            operationId = operationId,
            accountId = accountId,
            lineageId = lineageId,
            assetDefinitionId = assetDefinitionId,
            amountCanonical = canonicalAmount,
            offlinePublicKey = offlinePublicKey,
            authorizationId = authorizationId,
            preStateHash = preStateHash,
            receipts = receipts,
        )
        val envelope = OfflineStarkEnvelopeProver.buildEnvelope(
            domainTag = commitmentHex,
            transcriptLabel = REDEEM_REQUEST_CIRCUIT_ID,
        )
        return OfflineRedeemRequestProof(
            backend = BACKEND,
            circuitId = REDEEM_REQUEST_CIRCUIT_ID,
            recursionDepth = RECURSION_DEPTH,
            publicInputsHex = commitmentHex,
            envelope = envelope,
        )
    }
}
