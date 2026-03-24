package org.hyperledger.iroha.sdk.client

/** Summary entry returned by `GET /v1/ram-lfe/program-policies`. */
class RamLfeProgramPolicySummary(
    @JvmField val programId: String,
    @JvmField val owner: String,
    @JvmField val active: Boolean,
    @JvmField val resolverPublicKey: String,
    @JvmField val backend: String,
    @JvmField val verificationMode: String,
    @JvmField val inputEncryption: String?,
    @JvmField val inputEncryptionPublicParameters: String?,
    @JvmField val inputEncryptionPublicParametersDecoded: IdentifierBfvPublicParameters?,
    @JvmField val note: String?,
)
