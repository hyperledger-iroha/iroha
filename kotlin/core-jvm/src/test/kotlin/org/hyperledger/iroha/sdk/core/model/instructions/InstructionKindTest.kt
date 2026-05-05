package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class InstructionKindTest {

    @Test
    fun `fromDisplayName resolves exact name`() {
        assertEquals(InstructionKind.REGISTER, InstructionKind.fromDisplayName("Register"))
    }

    @Test
    fun `fromDisplayName is case insensitive`() {
        assertEquals(InstructionKind.REGISTER, InstructionKind.fromDisplayName("register"))
        assertEquals(InstructionKind.REGISTER, InstructionKind.fromDisplayName("REGISTER"))
        assertEquals(InstructionKind.EXECUTE_TRIGGER, InstructionKind.fromDisplayName("executetrigger"))
    }

    @Test
    fun `fromDisplayName throws on unknown name`() {
        assertFailsWith<IllegalArgumentException> {
            InstructionKind.fromDisplayName("NonExistent")
        }
    }

    @Test
    fun `fromDiscriminant resolves valid value`() {
        assertEquals(InstructionKind.MINT, InstructionKind.fromDiscriminant(5L))
    }

    @Test
    fun `fromDiscriminant resolves boundary values`() {
        assertEquals(InstructionKind.SET_PARAMETER, InstructionKind.fromDiscriminant(0L))
        assertEquals(InstructionKind.CUSTOM, InstructionKind.fromDiscriminant(13L))
    }

    @Test
    fun `fromDiscriminant throws on unknown value`() {
        assertFailsWith<IllegalArgumentException> {
            InstructionKind.fromDiscriminant(99L)
        }
    }
}