package org.hyperledger.iroha.sdk.core.model

import kotlin.test.Test
import kotlin.test.assertEquals
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
}
