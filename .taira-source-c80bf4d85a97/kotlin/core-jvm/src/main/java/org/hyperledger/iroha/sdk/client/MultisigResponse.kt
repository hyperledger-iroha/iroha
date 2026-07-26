package org.hyperledger.iroha.sdk.client

/** Response payload returned by Torii multisig participation endpoints. */
data class MultisigResponse(
    val ok: Boolean,
    val resolvedMultisigAccountId: String,
    val submitted: Boolean?,
    val proposalId: String?,
    val instructionsHash: String?,
    val txHashHex: String?,
    val executedTxHashHex: String?,
    val creationTimeMs: Long?,
    val signingMessageB64: String?,
)
