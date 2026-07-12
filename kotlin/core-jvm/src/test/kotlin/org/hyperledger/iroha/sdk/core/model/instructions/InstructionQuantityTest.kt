package org.hyperledger.iroha.sdk.core.model.instructions

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
