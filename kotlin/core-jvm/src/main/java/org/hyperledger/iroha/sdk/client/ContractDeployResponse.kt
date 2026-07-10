package org.hyperledger.iroha.sdk.client

/** One contract receipt emitted by `POST /v1/contracts/deploy`. */
class ContractDeployResponseContract(
    @JvmField val name: String,
    @JvmField val contractAlias: String?,
    @JvmField val contractAddress: String?,
    @JvmField val previousContractAddress: String?,
    @JvmField val upgraded: Boolean,
    @JvmField val dataspace: String?,
    @JvmField val deployNonce: Long?,
    @JvmField val txHashHex: String?,
    @JvmField val pipelineStatus: Map<String, Any?>?,
    @JvmField val codeHashHex: String,
    @JvmField val abiHashHex: String,
    @JvmField val status: String,
)

/** One hajimari-call receipt emitted by `POST /v1/contracts/deploy`. */
class ContractDeployResponseHajimariCall(
    @JvmField val id: String,
    @JvmField val contractAlias: String?,
    @JvmField val entrypoint: String?,
    @JvmField val txHashHex: String?,
    @JvmField val pipelineStatus: Map<String, Any?>?,
    @JvmField val status: String,
)

/** One assertion receipt emitted by `POST /v1/contracts/deploy`. */
class ContractDeployResponseAssertion(
    @JvmField val id: String,
    @JvmField val contractAlias: String?,
    @JvmField val entrypoint: String?,
    @JvmField val status: String,
    @JvmField val actualResult: Any?,
    @JvmField val expectedResult: Any?,
    @JvmField val error: String?,
)

/** Canonical bundle receipt emitted by `POST /v1/contracts/deploy`. */
class ContractDeployResponse(
    @JvmField val ok: Boolean,
    @JvmField val bundleName: String,
    @JvmField val bundleDigest: String,
    @JvmField val chainFingerprint: String,
    @JvmField val dryRun: Boolean,
    @JvmField val completedStages: List<String>,
    @JvmField val failurePoint: String?,
    @JvmField val contracts: List<ContractDeployResponseContract>,
    @JvmField val hajimariCalls: List<ContractDeployResponseHajimariCall>,
    @JvmField val assertions: List<ContractDeployResponseAssertion>,
)
