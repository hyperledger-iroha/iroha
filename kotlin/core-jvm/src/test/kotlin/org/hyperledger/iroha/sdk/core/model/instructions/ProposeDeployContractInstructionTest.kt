package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull

class ProposeDeployContractInstructionTest {
    @Test
    fun `builder serializes alias selector into canonical arguments`() {
        val instruction = ProposeDeployContractInstruction(
            contractAlias = "router::universal",
            codeHashHex = "aa".repeat(32),
            abiHashHex = "bb".repeat(32),
            abiVersion = "1",
            window = GovernanceInstructionUtils.AtWindow(10, 20),
            votingMode = GovernanceInstructionUtils.VotingMode.PLAIN,
        )

        assertEquals("router::universal", instruction.contractAlias)
        assertNull(instruction.contractAddress)
        assertEquals("router::universal", instruction.arguments["contract_alias"])
        assertEquals("aa".repeat(32), instruction.arguments["code_hash_hex"])
        assertEquals("bb".repeat(32), instruction.arguments["abi_hash_hex"])
        assertEquals("1", instruction.arguments["abi_version"])
        assertEquals("10", instruction.arguments["window.lower"])
        assertEquals("20", instruction.arguments["window.upper"])
        assertEquals("Plain", instruction.arguments["mode"])
    }

    @Test
    fun `fromArguments parses address selector`() {
        val args = linkedMapOf(
            "action" to "ProposeDeployContract",
            "contract_address" to "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
            "code_hash_hex" to "cc".repeat(32),
            "abi_hash_hex" to "dd".repeat(32),
            "abi_version" to "1",
        )

        val instruction = ProposeDeployContractInstruction.fromArguments(args)

        assertEquals("tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7", instruction.contractAddress)
        assertNull(instruction.contractAlias)
        assertEquals(args["contract_address"], instruction.arguments["contract_address"])
    }

    @Test
    fun `fromArguments rejects ambiguous selector`() {
        val error = assertFailsWith<IllegalArgumentException> {
            ProposeDeployContractInstruction.fromArguments(
                linkedMapOf(
                    "action" to "ProposeDeployContract",
                    "contract_address" to "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
                    "contract_alias" to "router::universal",
                    "code_hash_hex" to "ee".repeat(32),
                    "abi_hash_hex" to "ff".repeat(32),
                    "abi_version" to "1",
                ),
            )
        }

        assertEquals(
            "Instruction arguments must include exactly one of contract_address or contract_alias",
            error.message,
        )
    }
}
