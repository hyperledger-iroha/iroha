package org.hyperledger.iroha.sdk.client

/** Successful response emitted by `POST /v1/contracts/call`. */
class ContractCallResponse(
    @JvmField val ok: Boolean,
    @JvmField val submitted: Boolean,
    @JvmField val dataspace: String,
    @JvmField val codeHashHex: String,
    @JvmField val abiHashHex: String,
    @JvmField val creationTimeMs: Long,
    @JvmField val contractAddress: String?,
    @JvmField val txHashHex: String?,
    @JvmField val entrypoint: String?,
    @JvmField val transactionScaffoldB64: String?,
    @JvmField val signedTransactionB64: String?,
    @JvmField val signingMessageB64: String?,
)
