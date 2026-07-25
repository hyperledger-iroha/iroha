package org.hyperledger.iroha.sdk.validationfee

/**
 * Native boundary for the Parliament-governed validation-fee consensus proof.
 *
 * Callers must persist the returned evaluated height/context atomically before
 * requesting the next page. A missing or stale native ABI fails closed.
 */
class ValidationFeeConsensusProofBridge private constructor() {
    companion object {
        private const val LIBRARY_NAME = "connect_norito_bridge"
        private const val REQUIRED_BRIDGE_ABI_VERSION = 21
        private const val HASH_BYTES = 32
        private const val MAX_PROOF_BYTES = 4 * 1024 * 1024

        private val nativeLoadResult: Result<Unit> by lazy {
            runCatching {
                System.loadLibrary(LIBRARY_NAME)
                val actualAbi = nativeBridgeAbiVersion()
                check(actualAbi == REQUIRED_BRIDGE_ABI_VERSION) {
                    "native validation-fee consensus verifier ABI mismatch: " +
                        "expected $REQUIRED_BRIDGE_ABI_VERSION, found $actualAbi"
                }
            }
        }

        /**
         * Encode the canonical Norito request body for one bounded proof page.
         *
         * The context is validated but is intentionally absent from the frozen
         * V1 request body; it is supplied again to [verifyCurrentPolicyProofV1].
         */
        @JvmStatic
        fun encodeCurrentPolicyProofRequestV1(
            trustedCheckpointHeight: Long,
            trustedCheckpointContextId: ByteArray,
        ): ByteArray {
            require(trustedCheckpointHeight > 0) {
                "trustedCheckpointHeight must be positive"
            }
            requireIrohaHash(trustedCheckpointContextId, "trustedCheckpointContextId")
            requireNative()
            return nativeEncodeCurrentPolicyProofRequestV1(
                trustedCheckpointHeight,
                trustedCheckpointContextId.copyOf(),
            ).copyOf()
        }

        /**
         * Verify one proof page and return canonical UTF-8 JSON using schema
         * `iroha.validation_fee.verified_policy_projection.v1`.
         */
        @JvmStatic
        fun verifyCurrentPolicyProofV1(
            proofNorito: ByteArray,
            chainId: String,
            boundGenesisHash: ByteArray,
            policyChainGenesisHash: ByteArray,
            trustedCheckpointHeight: Long,
            trustedCheckpointContextId: ByteArray,
        ): String {
            require(proofNorito.isNotEmpty() && proofNorito.size <= MAX_PROOF_BYTES) {
                "proofNorito must contain 1..$MAX_PROOF_BYTES bytes"
            }
            require(
                chainId.isNotEmpty() &&
                    chainId.length <= 256 &&
                    chainId == chainId.trim() &&
                    chainId.none(Char::isISOControl),
            ) {
                "chainId must be canonical bounded text"
            }
            requireIrohaHash(boundGenesisHash, "boundGenesisHash")
            requireIrohaHash(policyChainGenesisHash, "policyChainGenesisHash")
            require(trustedCheckpointHeight > 0) {
                "trustedCheckpointHeight must be positive"
            }
            requireIrohaHash(trustedCheckpointContextId, "trustedCheckpointContextId")
            requireNative()
            val json = nativeVerifyCurrentPolicyProofV1(
                proofNorito.copyOf(),
                chainId.toByteArray(Charsets.UTF_8),
                boundGenesisHash.copyOf(),
                policyChainGenesisHash.copyOf(),
                trustedCheckpointHeight,
                trustedCheckpointContextId.copyOf(),
            )
            require(json.isNotEmpty()) { "native proof verifier returned an empty projection" }
            return json.toString(Charsets.UTF_8)
        }

        internal fun requireIrohaHash(value: ByteArray, label: String) {
            require(value.size == HASH_BYTES && (value[HASH_BYTES - 1].toInt() and 1) == 1) {
                "$label must contain one canonical 32-byte Iroha hash"
            }
        }

        private fun requireNative() {
            nativeLoadResult.getOrElse { failure ->
                throw IllegalStateException(
                    "native validation-fee consensus verifier is unavailable",
                    failure,
                )
            }
        }

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic
        private external fun nativeEncodeCurrentPolicyProofRequestV1(
            trustedCheckpointHeight: Long,
            trustedCheckpointContextId: ByteArray,
        ): ByteArray

        @JvmStatic
        private external fun nativeVerifyCurrentPolicyProofV1(
            proofNorito: ByteArray,
            chainId: ByteArray,
            boundGenesisHash: ByteArray,
            policyChainGenesisHash: ByteArray,
            trustedCheckpointHeight: Long,
            trustedCheckpointContextId: ByteArray,
        ): ByteArray
    }
}
