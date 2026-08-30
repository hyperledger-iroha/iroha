package org.hyperledger.iroha.sdk.client

/** Governance binding emitted by `GET /v1/gov/contracts/{contract_address}`. */
class GovernanceContractResponse(
    @JvmField val found: Boolean,
    @JvmField val contractAddress: String,
    @JvmField val contractSubjectAccount: String?,
    @JvmField val dataspace: String?,
    @JvmField val active: Boolean?,
    @JvmField val lifecycle: GovernanceContractLifecycle?,
    @JvmField val emergencyHoldActive: Boolean?,
    @JvmField val codeHashHex: String?,
    @JvmField val abiHashHex: String?,
    @JvmField val publicEntrypoints: List<String>?,
)

/** Complete retained ownership and lifecycle projection for one contract. */
class GovernanceContractLifecycle(
    /** Exact first-release lifecycle schema version. */
    @JvmField val version: Int,
    @JvmField val origin: String,
    @JvmField val originAccount: String,
    @JvmField val originProposalContentIdHex: String?,
    @JvmField val originGovernanceAttemptIdHex: String?,
    @JvmField val owner: String,
    @JvmField val pendingOwner: String?,
    @JvmField val parliamentDelegated: Boolean,
    @JvmField val activeCodeHashHex: String?,
    @JvmField val revision: Long,
    @JvmField val emergencyHold: GovernanceContractEmergencyHold?,
)

/** Retained bounded Parliament emergency-hold projection. */
class GovernanceContractEmergencyHold(
    @JvmField val incidentDigestHex: String,
    @JvmField val proposalContentIdHex: String,
    @JvmField val governanceAttemptIdHex: String,
    @JvmField val reason: String,
    @JvmField val imposedAtHeight: Long,
    @JvmField val expiresAtHeight: Long,
)
