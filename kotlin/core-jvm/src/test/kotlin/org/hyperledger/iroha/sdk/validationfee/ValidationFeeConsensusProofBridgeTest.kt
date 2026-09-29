package org.hyperledger.iroha.sdk.validationfee

import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertArrayEquals
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
                byteArrayOf(),
            )
        }
        assertThrows(IllegalArgumentException::class.java) {
            ValidationFeeConsensusProofBridge.encodeCurrentPolicyProofRequestV1(
                ByteArray(32) { 1 },
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
                trustedCheckpoint = byteArrayOf(0x4e, 0x52, 0x54, 0x30),
            )
        }
    }
    @Test
    fun `verified page retains its own checkpoint without exposing mutable storage`() {
        // Structural owner test only; actual proof authority is granted by the native verifier.
        val source = byteArrayOf(0x4e, 0x52, 0x54, 0x30, 1)
        val original = source.copyOf()
        val page = ValidationFeeConsensusProofBridge.VerifiedPolicyPage("{}", source)
        source[4] = 2
        val returned = page.promotedCheckpoint()
        returned[4] = 3
        assertArrayEquals(original, page.promotedCheckpoint())
        val equivalent = ValidationFeeConsensusProofBridge.VerifiedPolicyPage("{}", original)
        assertEquals(equivalent, page)
        assertEquals(equivalent.hashCode(), page.hashCode())
        assertThrows(IllegalArgumentException::class.java) {
            ValidationFeeConsensusProofBridge.VerifiedPolicyPage("", original)
        }
        assertThrows(IllegalArgumentException::class.java) {
            ValidationFeeConsensusProofBridge.VerifiedPolicyPage("{}", ByteArray(32) { 1 })
        }
    }
}
