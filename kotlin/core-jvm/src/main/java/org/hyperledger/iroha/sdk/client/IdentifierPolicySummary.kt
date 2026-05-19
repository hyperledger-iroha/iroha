package org.hyperledger.iroha.sdk.client

/** Summary entry returned by `GET /v1/identifier-policies`. */
class IdentifierPolicySummary @JvmOverloads constructor(
    @JvmField val policyId: String,
    @JvmField val owner: String,
    @JvmField val active: Boolean,
    @JvmField val normalization: IdentifierNormalization,
    @JvmField val resolverPublicKey: String,
    @JvmField val backend: String,
    @JvmField val inputEncryption: String?,
    @JvmField val inputEncryptionPublicParameters: String?,
    @JvmField val inputEncryptionPublicParametersDecoded: IdentifierBfvPublicParameters?,
    @JvmField val note: String?,
    @JvmField val proofVerifier: RamLfeProofVerifierMetadata? = null,
) {
    fun encryptedRequest(encryptedInputHex: String, outputOpening: RamLfeOutputOpening): IdentifierResolveRequest =
        IdentifierResolveRequest.encrypted(this, encryptedInputHex, outputOpening)

    @JvmOverloads
    fun encryptInput(input: String, seed: ByteArray? = null): String =
        IdentifierBfvEnvelopeBuilder.encrypt(this, input, seed)

    @JvmOverloads
    fun encryptedRequestFromInput(
        input: String,
        outputOpening: RamLfeOutputOpening,
        seed: ByteArray? = null,
    ): IdentifierResolveRequest =
        IdentifierResolveRequest.encryptedFromInput(this, input, outputOpening, seed)
}
