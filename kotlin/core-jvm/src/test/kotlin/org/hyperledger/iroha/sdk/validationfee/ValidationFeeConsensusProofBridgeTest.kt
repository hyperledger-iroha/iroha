package org.hyperledger.iroha.sdk.validationfee

import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Test

class ValidationFeeConsensusProofBridgeTest {
    @Test
    fun `hash validation requires the canonical Iroha marker`() {
        ValidationFeeConsensusProofBridge.requireIrohaHash(
            ByteArray(32) { 3 },
            "markedHash",
        )
        assertThrows(IllegalArgumentException::class.java) {
            ValidationFeeConsensusProofBridge.requireIrohaHash(
                ByteArray(32),
                "zeroHash",
            )
        }
        assertThrows(IllegalArgumentException::class.java) {
            ValidationFeeConsensusProofBridge.requireIrohaHash(
                ByteArray(32) { 2 },
                "unmarkedHash",
            )
        }
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
                networkId = NetworkId.fromBytes(ByteArray(32) { 1 }),
                policyChainGenesisHash = ByteArray(31) { 1 },
                trustedCheckpointHeight = 1,
                trustedCheckpointContextId = ByteArray(32) { 1 },
            )
        }
    }
}
