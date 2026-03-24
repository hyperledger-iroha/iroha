package org.hyperledger.iroha.sdk.offline

/** Response payload for submitting an offline settlement bundle. */
class OfflineSettlementSubmitResponse(
    val bundleIdHex: String,
    val transactionHashHex: String,
)
