package org.hyperledger.iroha.sdk.client

/** Successful response emitted by `POST /v1/contracts/deploy`. */
class ContractDeployResponse(
    @JvmField val ok: Boolean,
    @JvmField val contractAlias: String?,
    @JvmField val contractAddress: String?,
    @JvmField val previousContractAddress: String?,
    @JvmField val upgraded: Boolean,
    @JvmField val dataspace: String?,
    @JvmField val deployNonce: Long?,
    @JvmField val txHashHex: String?,
    @JvmField val codeHashHex: String,
    @JvmField val abiHashHex: String,
)
