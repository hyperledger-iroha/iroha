package org.hyperledger.iroha.sdk.validationfee

import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Test

class ValidationFeeConsensusProofBridgeTest {
    @Test
    fun `hash validation accepts a nonzero even-ending hash`() {
        ValidationFeeConsensusProofBridge.requireNonzeroHash(
            ByteArray(32) { 2 },
            "evenEndingHash",
        )
    }

    @Test
    fun `request rejects invalid checkpoint before loading native code`() {
        assertThrows(IllegalArgumentException::class.java) {
            ValidationFeeConsensusProofBridge.encodeCurrentPolicyProofRequestV1(
                0,
                ByteArray(32) { 1 },
            )
        }
        assertThrows(IllegalArgumentException::class.java) {
            ValidationFeeConsensusProofBridge.encodeCurrentPolicyProofRequestV1(
                1,
                ByteArray(32),
            )
        }
    }

    @Test
    fun `verifier rejects malformed immutable binding before loading native code`() {
        assertThrows(IllegalArgumentException::class.java) {
            ValidationFeeConsensusProofBridge.verifyCurrentPolicyProofV1(
                proofNorito = byteArrayOf(1),
                chainId = "chain",
                boundGenesisHash = ByteArray(31) { 1 },
                policyChainGenesisHash = ByteArray(32) { 1 },
                trustedCheckpointHeight = 1,
                trustedCheckpointContextId = ByteArray(32) { 1 },
            )
        }
    }
}
