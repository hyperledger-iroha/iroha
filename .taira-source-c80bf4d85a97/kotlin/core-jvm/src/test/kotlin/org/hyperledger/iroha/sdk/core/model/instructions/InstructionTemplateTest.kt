package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.InstructionBox
import kotlin.test.Test
import kotlin.test.assertEquals

class InstructionTemplateTest {

    private val stubTemplate = object : InstructionTemplate {
        override val kind: InstructionKind = InstructionKind.MINT
        override val arguments: Map<String, String> = mapOf("asset" to "gold")
    }

    @Test
    fun `toInstructionBox returns box wrapping this payload`() {
        val box = stubTemplate.toInstructionBox()
        assertEquals(InstructionKind.MINT, box.kind)
        assertEquals(mapOf("asset" to "gold"), box.arguments)
    }

    @Test
    fun `toInstructionBox matches InstructionBox of`() {
        val box = stubTemplate.toInstructionBox()
        val expected = InstructionBox.of(stubTemplate)
        assertEquals(expected, box)
    }
}
