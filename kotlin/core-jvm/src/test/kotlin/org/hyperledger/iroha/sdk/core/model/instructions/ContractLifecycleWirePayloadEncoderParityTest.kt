package org.hyperledger.iroha.sdk.core.model.instructions

import java.math.BigInteger
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.WirePayload
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Rust/Kotlin parity coverage for all first-release contract lifecycle owner instructions. */
class ContractLifecycleWirePayloadEncoderParityTest {
    @Test
    fun `all lifecycle instructions preserve their revision and match Rust`() {
        val fixture = FixtureGeneratorRunner.run("contract-lifecycle")
        assertEquals(7, fixture.size)
        val contractAddress = fixture[5]
        val accountId = fixture[6]
        val chainDiscriminant = requireNotNull(
            AccountAddress.detectI105Discriminant(accountId),
        )

        val set = ContractLifecycleWirePayloadEncoder.encodeSetContractParliamentDelegation(
            contractAddress,
            BigInteger.valueOf(7),
            true,
        )
        val offerAccount =
            ContractLifecycleWirePayloadEncoder.encodeOfferContractOwnershipToAccount(
                contractAddress,
                BigInteger.valueOf(8),
                accountId,
            )
        val offerParliament =
            ContractLifecycleWirePayloadEncoder.encodeOfferContractOwnershipToParliament(
                contractAddress,
                BigInteger.valueOf(9),
            )
        val accept = ContractLifecycleWirePayloadEncoder.encodeAcceptContractOwnership(
            contractAddress,
            BigInteger.TEN,
        )
        val cancel = ContractLifecycleWirePayloadEncoder.encodeCancelContractOwnershipOffer(
            contractAddress,
            BigInteger.valueOf(11),
        )
        val instructions = listOf(set, offerAccount, offerParliament, accept, cancel)

        assertEquals(
            ContractLifecycleWirePayloadEncoder.WIRE_NAMES,
            instructions.map(InstructionBox::name).distinct(),
        )
        instructions.forEachIndexed { index, instruction ->
            assertContentEquals(
                FixtureGeneratorRunner.hexToBytes(fixture[index]),
                wirePayload(instruction),
                "contract lifecycle payload $index must match Rust",
            )
        }

        val decodedSet =
            ContractLifecycleWirePayloadEncoder.decodeSetContractParliamentDelegation(
                wirePayload(set),
            )
        assertEquals(contractAddress, decodedSet.contractAddress)
        assertEquals(BigInteger.valueOf(7), decodedSet.expectedRevision)
        assertTrue(decodedSet.delegated)

        val decodedAccount = ContractLifecycleWirePayloadEncoder.decodeOfferContractOwnership(
            wirePayload(offerAccount),
            chainDiscriminant,
        )
        assertEquals(BigInteger.valueOf(8), decodedAccount.expectedRevision)
        assertEquals(accountId, decodedAccount.newOwnerAccountId)

        val decodedParliament = ContractLifecycleWirePayloadEncoder.decodeOfferContractOwnership(
            wirePayload(offerParliament),
            chainDiscriminant,
        )
        assertEquals(BigInteger.valueOf(9), decodedParliament.expectedRevision)
        assertNull(decodedParliament.newOwnerAccountId)

        assertEquals(
            BigInteger.TEN,
            ContractLifecycleWirePayloadEncoder.decodeAcceptContractOwnership(
                wirePayload(accept),
            ).expectedRevision,
        )
        assertEquals(
            BigInteger.valueOf(11),
            ContractLifecycleWirePayloadEncoder.decodeCancelContractOwnershipOffer(
                wirePayload(cancel),
            ).expectedRevision,
        )
    }

    @Test
    fun `lifecycle revision guards reject zero overflow and schema substitution`() {
        val contractAddress =
            "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
        assertFailsWith<IllegalArgumentException> {
            ContractLifecycleWirePayloadEncoder.encodeAcceptContractOwnership(
                contractAddress,
                BigInteger.ZERO,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ContractLifecycleWirePayloadEncoder.encodeCancelContractOwnershipOffer(
                contractAddress,
                BigInteger.ONE.shiftLeft(64),
            )
        }

        val maxRevision = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
        val accept = ContractLifecycleWirePayloadEncoder.encodeAcceptContractOwnership(
            contractAddress,
            maxRevision,
        )
        assertEquals(
            maxRevision,
            ContractLifecycleWirePayloadEncoder.decodeAcceptContractOwnership(
                wirePayload(accept),
            ).expectedRevision,
        )
        assertFailsWith<IllegalArgumentException> {
            ContractLifecycleWirePayloadEncoder.decodeCancelContractOwnershipOffer(
                wirePayload(accept),
            )
        }
    }

    private fun wirePayload(instruction: InstructionBox): ByteArray =
        assertIs<WirePayload>(instruction.payload).payloadBytes
}
