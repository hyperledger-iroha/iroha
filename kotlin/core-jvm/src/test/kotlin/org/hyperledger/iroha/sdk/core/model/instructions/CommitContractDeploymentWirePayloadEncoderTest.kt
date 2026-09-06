package org.hyperledger.iroha.sdk.core.model.instructions

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertFailsWith

/** Canonical hash admission for atomic contract deployment payloads. */
class CommitContractDeploymentWirePayloadEncoderTest {
    @Test
    fun `deployment code hash requires the canonical marker on encode and decode`() {
        val canonicalHash = ByteArray(32) { 0xab.toByte() }
        val canonicalHashHex = "ab".repeat(32)

        CommitContractDeploymentWirePayloadEncoder.encode(
            BigInteger.ZERO,
            CONTRACT_ADDRESS,
            canonicalHashHex,
            "audit_contract",
        )
        assertContentEquals(
            canonicalHash,
            CommitContractDeploymentWirePayloadEncoder.decodeCanonicalCodeHashBytes(
                canonicalHash,
            ),
        )

        assertFailsWith<IllegalArgumentException> {
            CommitContractDeploymentWirePayloadEncoder.encode(
                BigInteger.ZERO,
                CONTRACT_ADDRESS,
                "ab".repeat(31) + "aa",
                "audit_contract",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            CommitContractDeploymentWirePayloadEncoder.decodeCanonicalCodeHashBytes(
                canonicalHash.copyOf().also { it[it.lastIndex] = 0xaa.toByte() },
            )
        }
        assertFailsWith<IllegalArgumentException> {
            CommitContractDeploymentWirePayloadEncoder.decodeCanonicalCodeHashBytes(
                canonicalHash.copyOf(33),
            )
        }
    }

    private companion object {
        const val CONTRACT_ADDRESS =
            "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
    }
}
