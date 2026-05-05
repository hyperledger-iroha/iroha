package org.hyperledger.iroha.sdk.client

/** Governance binding emitted by `GET /v1/gov/contracts/{contract_address}`. */
class GovernanceContractResponse(
    @JvmField val found: Boolean,
    @JvmField val contractAddress: String,
    @JvmField val dataspace: String?,
    @JvmField val codeHashHex: String?,
)
