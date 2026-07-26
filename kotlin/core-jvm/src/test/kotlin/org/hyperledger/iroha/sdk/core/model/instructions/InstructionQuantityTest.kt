package org.hyperledger.iroha.sdk.core.model.instructions

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity

class InstructionQuantityTest {
    @Test
    fun `asset and rwa instructions reject alternate quantity spellings`() {
        INVALID_QUANTITIES.forEach { quantity ->
            constructionAttempts(quantity).forEach { construct ->
                assertFailsWith<IllegalArgumentException>("constructor accepted '$quantity'") {
                    construct()
                }
            }
        }
    }

    @Test
    fun `asset and rwa argument readback rejects noncanonical quantities`() {
        readbackAttempts("1.0").forEach { read ->
            assertFailsWith<IllegalArgumentException> {
                read()
            }
        }
    }

    @Test
    fun `asset and rwa instructions accept the lossless quantity type`() {
        val quantity = KotodamaQuantity.parseCanonical("1.25")
        val values = listOf(
            MintAssetInstruction("asset", quantity).quantity,
            BurnAssetInstruction("asset", quantity).quantity,
            TransferAssetInstruction("asset", quantity, "destination").quantity,
            HoldRwaInstruction("rwa", quantity).quantity,
            ReleaseRwaInstruction("rwa", quantity).quantity,
            RedeemRwaInstruction("rwa", quantity).quantity,
            TransferRwaInstruction("source", "rwa", quantity, "destination").quantity,
            ForceTransferRwaInstruction("rwa", quantity, "destination").quantity,
        )

        values.forEach { assertEquals("1.25", it) }
    }

    @Test
    fun `plain ballot amount is a canonical lossless quantity`() {
        val ballot = CastPlainBallotInstruction(
            referendumId = "referendum",
            ownerAccountId = "owner",
            amount = "18446744073709551616.25",
            durationBlocks = 10,
            direction = 1,
        )
        assertEquals("18446744073709551616.25", ballot.amount)
        assertEquals("18446744073709551616.25", ballot.arguments["amount"])
        assertFailsWith<IllegalArgumentException>("BigInteger constructor accepted an out-of-range Quantity") {
            CastPlainBallotInstruction(
                "referendum",
                "owner",
                BigInteger.ONE.shiftLeft(511),
                10,
                1,
            )
        }
        assertEquals(
            "1.25",
            CastPlainBallotInstruction(
                "referendum",
                "owner",
                KotodamaQuantity.parseCanonical("1.25"),
                10,
                1,
            ).amount,
        )

        INVALID_QUANTITIES.forEach { amount ->
            assertFailsWith<IllegalArgumentException>("constructor accepted '$amount'") {
                CastPlainBallotInstruction("referendum", "owner", amount, 10, 1)
            }
            assertFailsWith<IllegalArgumentException>("readback accepted '$amount'") {
                CastPlainBallotInstruction.fromArguments(
                    mapOf(
                        "action" to "CastPlainBallot",
                        "referendum_id" to "referendum",
                        "owner" to "owner",
                        "amount" to amount,
                        "duration_blocks" to "10",
                        "direction" to "1",
                    ),
                )
            }
        }
    }

    private fun constructionAttempts(quantity: String): List<() -> Any> = listOf(
        { MintAssetInstruction("asset", quantity) },
        { BurnAssetInstruction("asset", quantity) },
        { TransferAssetInstruction("asset", quantity, "destination") },
        { HoldRwaInstruction("rwa", quantity) },
        { ReleaseRwaInstruction("rwa", quantity) },
        { RedeemRwaInstruction("rwa", quantity) },
        { TransferRwaInstruction("source", "rwa", quantity, "destination") },
        { ForceTransferRwaInstruction("rwa", quantity, "destination") },
    )

    private fun readbackAttempts(quantity: String): List<() -> Any> = listOf(
        { MintAssetInstruction.fromArguments(mapOf("asset" to "asset", "quantity" to quantity)) },
        { BurnAssetInstruction.fromArguments(mapOf("asset" to "asset", "quantity" to quantity)) },
        {
            TransferAssetInstruction.fromArguments(
                mapOf("asset" to "asset", "quantity" to quantity, "destination" to "destination"),
            )
        },
        { HoldRwaInstruction.fromArguments(mapOf("rwa" to "rwa", "quantity" to quantity)) },
        { ReleaseRwaInstruction.fromArguments(mapOf("rwa" to "rwa", "quantity" to quantity)) },
        { RedeemRwaInstruction.fromArguments(mapOf("rwa" to "rwa", "quantity" to quantity)) },
        {
            TransferRwaInstruction.fromArguments(
                mapOf(
                    "source" to "source",
                    "rwa" to "rwa",
                    "quantity" to quantity,
                    "destination" to "destination",
                ),
            )
        },
        {
            ForceTransferRwaInstruction.fromArguments(
                mapOf("rwa" to "rwa", "quantity" to quantity, "destination" to "destination"),
            )
        },
    )

    private companion object {
        val INVALID_QUANTITIES = listOf(
            "",
            " ",
            "\t1",
            "1 ",
            "+1",
            "01",
            "1.",
            ".5",
            "1e0",
            "-1",
            "-0",
            "-0.0",
            "1.0",
            "1.2300",
            "0.0",
        )
    }
}
