package org.hyperledger.iroha.sdk.client

/** Typed request wrapper for identifier resolve and claim-receipt flows. */
class IdentifierResolveRequest private constructor(
    @JvmField val policyId: String,
    @JvmField val encryptedInputHex: String,
    @JvmField val outputOpening: RamLfeOutputOpening,
) {
    companion object {
        @JvmStatic
        fun encrypted(
            policyId: String,
            encryptedInputHex: String,
            outputOpening: RamLfeOutputOpening,
        ): IdentifierResolveRequest {
            val normalizedPolicyId = HttpClientTransport.normalizeNonBlank(policyId, "policyId")
            val normalizedEncryptedInput =
                HttpClientTransport.normalizeEvenLengthHex(encryptedInputHex, "encryptedInputHex")
            return IdentifierResolveRequest(normalizedPolicyId, normalizedEncryptedInput, outputOpening)
        }

        @JvmStatic
        fun encrypted(
            policy: IdentifierPolicySummary,
            encryptedInputHex: String,
            outputOpening: RamLfeOutputOpening,
        ): IdentifierResolveRequest {
            require("bfv-v1".equals(policy.inputEncryption, ignoreCase = true)) {
                "Policy ${policy.policyId} does not publish BFV encrypted-input support"
            }
            return encrypted(policy.policyId, encryptedInputHex, outputOpening)
        }

        @JvmStatic
        @JvmOverloads
        fun encryptedFromInput(
            policy: IdentifierPolicySummary,
            input: String,
            outputOpening: RamLfeOutputOpening,
            seed: ByteArray? = null,
        ): IdentifierResolveRequest =
            encrypted(policy.policyId, IdentifierBfvEnvelopeBuilder.encrypt(policy, input, seed), outputOpening)
    }
}
