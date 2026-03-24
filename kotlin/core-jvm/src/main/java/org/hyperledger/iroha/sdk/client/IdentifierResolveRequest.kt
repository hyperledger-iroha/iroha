package org.hyperledger.iroha.sdk.client

/** Typed request wrapper for identifier resolve and claim-receipt flows. */
class IdentifierResolveRequest private constructor(
    @JvmField val policyId: String,
    @JvmField val input: String?,
    @JvmField val encryptedInputHex: String?,
) {
    companion object {
        @JvmStatic
        fun plaintext(policyId: String, input: String): IdentifierResolveRequest {
            val normalizedPolicyId = HttpClientTransport.normalizeNonBlank(policyId, "policyId")
            val normalizedInput = HttpClientTransport.normalizeNonBlank(input, "input")
            return IdentifierResolveRequest(normalizedPolicyId, normalizedInput, null)
        }

        @JvmStatic
        fun encrypted(policyId: String, encryptedInputHex: String): IdentifierResolveRequest {
            val normalizedPolicyId = HttpClientTransport.normalizeNonBlank(policyId, "policyId")
            val normalizedEncryptedInput =
                HttpClientTransport.normalizeEvenLengthHex(encryptedInputHex, "encryptedInputHex")
            return IdentifierResolveRequest(normalizedPolicyId, null, normalizedEncryptedInput)
        }

        @JvmStatic
        fun plaintext(policy: IdentifierPolicySummary, input: String): IdentifierResolveRequest {
            val normalized = policy.normalization.normalize(input, "input")
            return plaintext(policy.policyId, normalized)
        }

        @JvmStatic
        fun encrypted(
            policy: IdentifierPolicySummary,
            encryptedInputHex: String,
        ): IdentifierResolveRequest {
            require("bfv-v1".equals(policy.inputEncryption, ignoreCase = true)) {
                "Policy ${policy.policyId} does not publish BFV encrypted-input support"
            }
            return encrypted(policy.policyId, encryptedInputHex)
        }

        @JvmStatic
        @JvmOverloads
        fun encryptedFromInput(
            policy: IdentifierPolicySummary,
            input: String,
            seed: ByteArray? = null,
        ): IdentifierResolveRequest =
            encrypted(policy.policyId, IdentifierBfvEnvelopeBuilder.encrypt(policy, input, seed))
    }
}
