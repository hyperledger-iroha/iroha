package org.hyperledger.iroha.sdk.core.model.instructions

import java.math.BigInteger
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Tests the argument template only; native frames use the Rust codec producer. */
class UpsertProviderCreditInstructionTest {
    @Test
    fun `absence and exact current are explicit roundtripping arguments`() {
        for (guard in listOf(null, currentHash())) {
            val instruction = credit(guard)
            assertTrue(instruction.arguments.containsKey("expected_current"))
            assertEquals(guard ?: "null", instruction.arguments["expected_current"])
            assertEquals(guard, instruction.expectedCurrent)
            val roundtrip = UpsertProviderCreditInstruction.fromArguments(instruction.arguments)
            assertEquals(instruction, roundtrip)
            assertEquals(instruction.hashCode(), roundtrip.hashCode())
            assertEquals(instruction.arguments, roundtrip.toInstructionBox().arguments)
        }
        assertNull(credit(null).expectedCurrent)
        assertNotEquals(credit(null), credit(currentHash()))
    }

    @Test
    fun `missing empty and malformed current guards cannot select an unguarded update`() {
        val original = credit(null).arguments
        assertFailsWith<IllegalArgumentException> {
            UpsertProviderCreditInstruction.fromArguments(original - "expected_current")
        }
        for (invalid in listOf("", " ", "NULL", " null", "null ", "11".repeat(32),
            currentHash().lowercase(), " " + currentHash(), currentHash() + " ",
            currentHash().dropLast(1) + "!")) {
            assertFailsWith<IllegalArgumentException> {
                UpsertProviderCreditInstruction.fromArguments(original + ("expected_current" to invalid))
            }
        }
        assertFailsWith<IllegalArgumentException> { credit("null") }
        assertFailsWith<IllegalArgumentException> { credit("11".repeat(32)) }
        assertFailsWith<IllegalArgumentException> { credit("x".repeat(4096)) }
    }

    @Test
    fun `caller maps cannot change the retained guard or metadata`() {
        val metadata = linkedMapOf("region" to "jp", "tier" to "cold")
        val instruction = credit(currentHash(), metadata)
        metadata["region"] = "altered"
        (instruction.metadata as MutableMap<String, String>)["region"] = "altered again"
        (instruction.arguments as MutableMap<String, String>)["expected_current"] = "null"
        val imported = instruction.arguments.toMutableMap()
        val decoded = UpsertProviderCreditInstruction.fromArguments(imported)
        imported["expected_current"] = "null"
        imported["record.metadata.region"] = "altered"
        assertEquals("jp", instruction.metadata["region"])
        assertEquals("jp", decoded.metadata["region"])
        assertEquals(currentHash(), instruction.arguments["expected_current"])
        assertEquals(currentHash(), decoded.expectedCurrent)
    }

    @Test
    fun `record bounds and exact action remain enforced`() {
        val original = credit(null).arguments
        assertFailsWith<IllegalArgumentException> {
            UpsertProviderCreditInstruction.fromArguments(original + ("action" to "Other"))
        }
        for (field in listOf("record.available_credit_nano", "record.bonded_nano", "record.required_bond_nano",
            "record.expected_settlement_nano", "record.onboarding_epoch", "record.last_settlement_epoch",
            "record.low_balance_since_epoch", "record.slashed_nano", "record.under_delivery_strikes",
            "record.last_penalty_epoch")) {
            assertFailsWith<IllegalArgumentException> {
                UpsertProviderCreditInstruction.fromArguments(original + (field to "-1"))
            }
        }
    }

    private fun currentHash(): String = HashLiteral.canonicalize(ByteArray(32) { 0xAB.toByte() })

    private fun credit(guard: String?, metadata: Map<String, String> = emptyMap()) =
        UpsertProviderCreditInstruction(
            expectedCurrent = guard,
            providerIdHex = "11".repeat(32),
            availableCreditNano = BigInteger.valueOf(123),
            bondedNano = BigInteger.valueOf(555),
            requiredBondNano = BigInteger.valueOf(777),
            expectedSettlementNano = BigInteger.valueOf(333),
            onboardingEpoch = 1700000,
            lastSettlementEpoch = 1700800,
            lowBalanceSinceEpoch = 1700500,
            slashedNano = BigInteger.valueOf(1000),
            underDeliveryStrikes = 2,
            lastPenaltyEpoch = 1700600,
            metadata = metadata,
        )
}
