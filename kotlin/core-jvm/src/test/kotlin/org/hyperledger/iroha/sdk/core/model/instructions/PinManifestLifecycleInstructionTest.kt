package org.hyperledger.iroha.sdk.core.model.instructions

import java.util.Base64
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse

class PinManifestLifecycleInstructionTest {

    @Test
    fun `approval omits caller supplied time and roundtrips canonical fields`() {
        val envelope = Base64.getEncoder().encodeToString("council-envelope".toByteArray())
        val instruction = ApprovePinManifestInstruction(
            digestHex = "a0".repeat(32),
            councilEnvelopeBase64 = envelope,
            councilEnvelopeDigestHex = "b1".repeat(32),
        )

        assertFalse(instruction.arguments.containsKey("approved_epoch"))
        assertEquals(
            instruction,
            ApprovePinManifestInstruction.fromArguments(instruction.arguments),
        )
    }

    @Test
    fun `approval rejects retired caller supplied epoch`() {
        val arguments = ApprovePinManifestInstruction("a0".repeat(32))
            .arguments
            .toMutableMap()
        arguments["approved_epoch"] = "42"

        assertFailsWith<IllegalArgumentException> {
            ApprovePinManifestInstruction.fromArguments(arguments)
        }
    }

    @Test
    fun `retirement omits caller supplied time and roundtrips canonical fields`() {
        val instruction = RetirePinManifestInstruction.builder()
            .setDigestHex("c0".repeat(32))
            .setReason("governance-retired")
            .build()

        assertFalse(instruction.arguments.containsKey("retired_epoch"))
        assertEquals(
            instruction,
            RetirePinManifestInstruction.fromArguments(instruction.arguments),
        )
    }

    @Test
    fun `retirement rejects retired caller supplied epoch`() {
        val arguments = RetirePinManifestInstruction.builder()
            .setDigestHex("c0".repeat(32))
            .build()
            .arguments
            .toMutableMap()
        arguments["retired_epoch"] = "99"

        assertFailsWith<IllegalArgumentException> {
            RetirePinManifestInstruction.fromArguments(arguments)
        }
    }
}
