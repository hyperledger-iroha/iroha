package org.hyperledger.iroha.sdk.core.model

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNotEquals

class ExecutableTest {

    @Test
    fun `instructions factory returns Instructions variant`() {
        val exec = Executable.instructions(emptyList())
        assertIs<Executable.Instructions>(exec)
    }

    @Test
    fun `ivm factory returns Ivm variant`() {
        val exec = Executable.ivm(byteArrayOf(1, 2, 3))
        assertIs<Executable.Ivm>(exec)
    }

    @Test
    fun `contract call factory returns ContractCall variant`() {
        val invocation = sampleInvocation()
        val exec = Executable.contractCall(invocation)
        assertEquals(invocation, assertIs<Executable.ContractCall>(exec).invocation)
    }

    @Test
    fun `batch builder preserves mixed item order`() {
        val instructionA = sampleInstruction("a")
        val invocation = sampleInvocation()
        val instructionB = sampleInstruction("b")

        val batch = assertIs<Executable.Batch>(
            Executable.batchBuilder()
                .addInstruction(instructionA)
                .addContractCall(invocation)
                .addItem(ExecutableBatchItem.instruction(instructionB))
                .build(),
        )

        assertEquals(instructionA, assertIs<ExecutableBatchItem.Instruction>(batch.entries[0]).instruction)
        assertEquals(invocation, assertIs<ExecutableBatchItem.ContractCall>(batch.entries[1]).invocation)
        assertEquals(instructionB, assertIs<ExecutableBatchItem.Instruction>(batch.entries[2]).instruction)
        assertEquals(true, batch.requiresTransactionGasLimit())
    }

    @Test
    fun `instruction-only batch does not require a gas limit`() {
        val batch = Executable.batch(
            listOf(ExecutableBatchItem.instruction(sampleInstruction("only"))),
        )
        assertEquals(false, batch.requiresTransactionGasLimit())
        assertEquals(true, Executable.ivm(byteArrayOf()).requiresTransactionGasLimit())
    }

    @Test
    fun `empty batches are rejected before signing`() {
        assertFailsWith<IllegalArgumentException> { Executable.Batch(emptyList()) }
        assertFailsWith<IllegalArgumentException> { Executable.batch(emptyList()) }
        assertFailsWith<IllegalArgumentException> { Executable.batchBuilder().build() }
    }

    @Test
    fun `Batch defensively copies input list`() {
        val entries = mutableListOf(ExecutableBatchItem.instruction(sampleInstruction("a")))
        val batch = Executable.Batch(entries)
        entries.add(ExecutableBatchItem.contractCall(sampleInvocation()))
        assertEquals(1, batch.entries.size)
        assertFailsWith<UnsupportedOperationException> {
            (batch.entries as MutableList).add(ExecutableBatchItem.contractCall(sampleInvocation()))
        }
    }

    @Test
    fun `ContractInvocation defensively copies byte arrays`() {
        val hash = ByteArray(32) { 0x11.toByte() }
        val arguments = byteArrayOf(1, 2, 3)
        val invocation = ContractInvocation(CONTRACT_ADDRESS, hash, "call", arguments)

        hash[0] = 0x22
        arguments[0] = 9
        assertEquals(0x11.toByte(), invocation.expectedCodeHash[0])
        assertEquals(1.toByte(), invocation.arguments!![0])

        invocation.expectedCodeHash[0] = 0x33
        invocation.arguments!![0] = 8
        assertEquals(0x11.toByte(), invocation.expectedCodeHash[0])
        assertEquals(1.toByte(), invocation.arguments!![0])
    }

    @Test
    fun `ContractInvocation validates hash and argument bounds`() {
        assertFailsWith<IllegalArgumentException> {
            ContractInvocation(CONTRACT_ADDRESS, ByteArray(31), "call")
        }
        assertFailsWith<IllegalArgumentException> {
            ContractInvocation(CONTRACT_ADDRESS, ByteArray(32), "call")
        }
        assertFailsWith<IllegalArgumentException> {
            ContractInvocation(
                CONTRACT_ADDRESS,
                ByteArray(32) { 1 },
                "call",
                ByteArray(MAX_CONTRACT_ARGUMENT_RECORD_BYTES + 1),
            )
        }
    }

    @Test
    fun `ContractInvocation accepts only canonical V1 Bech32m addresses`() {
        sampleInvocation()

        listOf(
            "abc",
            " $CONTRACT_ADDRESS",
            CONTRACT_ADDRESS.uppercase(),
            CONTRACT_ADDRESS.dropLast(1) + "q",
            BECH32_CHECKSUM_ADDRESS,
            NON_CANONICAL_PADDING_ADDRESS,
            SHORT_PAYLOAD_ADDRESS,
            VERSION_TWO_ADDRESS,
        ).forEach { invalidAddress ->
            assertFailsWith<IllegalArgumentException>("address: $invalidAddress") {
                ContractInvocation(
                    invalidAddress,
                    ByteArray(32) { 1 },
                    "call",
                )
            }
        }
    }

    @Test
    fun `Instructions defensively copies input list`() {
        val list = mutableListOf<InstructionBox>()
        val exec = Executable.Instructions(list)
        list.add(InstructionBox.fromWirePayload("iroha.mint", byteArrayOf(1)))
        assertEquals(0, exec.instructions.size)
    }

    @Test
    fun `Instructions list is immutable`() {
        val exec = Executable.Instructions(emptyList())
        val list = exec.instructions
        assertIs<List<InstructionBox>>(list)
    }

    @Test
    fun `Ivm defensively copies input bytes`() {
        val original = byteArrayOf(1, 2, 3)
        val exec = Executable.Ivm(original)
        original[0] = 99
        assertEquals(1, exec.ivmBytes[0])
    }

    @Test
    fun `Ivm defensively copies output bytes`() {
        val exec = Executable.Ivm(byteArrayOf(1, 2, 3))
        exec.ivmBytes[0] = 99
        assertEquals(1, exec.ivmBytes[0])
    }

    @Test
    fun `Instructions equals for same content`() {
        val a = Executable.Instructions(emptyList())
        val b = Executable.Instructions(emptyList())
        assertEquals(a, b)
        assertEquals(a.hashCode(), b.hashCode())
    }

    @Test
    fun `Ivm equals for same content`() {
        val a = Executable.Ivm(byteArrayOf(1, 2, 3))
        val b = Executable.Ivm(byteArrayOf(1, 2, 3))
        assertEquals(a, b)
        assertEquals(a.hashCode(), b.hashCode())
    }

    @Test
    fun `Ivm not equal for different content`() {
        val a = Executable.Ivm(byteArrayOf(1, 2, 3))
        val b = Executable.Ivm(byteArrayOf(4, 5, 6))
        assertNotEquals(a, b)
    }

    @Test
    fun `Instructions and Ivm are not equal`() {
        val instructions = Executable.Instructions(emptyList())
        val ivm = Executable.Ivm(byteArrayOf())
        assertNotEquals<Executable>(instructions, ivm)
    }

    private fun sampleInvocation(): ContractInvocation = ContractInvocation(
        contractAddress = CONTRACT_ADDRESS,
        expectedCodeHash = ByteArray(32) { 0x45.toByte() },
        entrypoint = "call",
        arguments = byteArrayOf(1, 2, 3),
    )

    private fun sampleInstruction(suffix: String): InstructionBox {
        val payload = suffix.toByteArray()
        return InstructionBox.fromWirePayload("iroha.test.$suffix", payload)
    }

    companion object {
        private const val CONTRACT_ADDRESS =
            "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8"
        private const val BECH32_CHECKSUM_ADDRESS =
            "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqc3gg99"
        private const val NON_CANONICAL_PADDING_ADDRESS =
            "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjpsmv3a4"
        private const val SHORT_PAYLOAD_ADDRESS = "tairac1qyqqqqqqqqqqqqpu6elzr2"
        private const val VERSION_TWO_ADDRESS =
            "tairac1qgqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqdjp7qw"
    }
}
