package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.WirePayload
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertIs

/**
 * Parity test for [TransferWirePayloadEncoder].
 *
 * Runs `kotlin-fixture-gen transfer-asset` which encodes a `TransferBox::Asset`
 * instruction using the current Rust data model and outputs:
 * - Line 1: wire payload hex
 * - Line 2: asset ID string (`<base58-def>#<i105-account>`)
 * - Line 3: amount
 * - Line 4: destination account I105
 */
class TransferWirePayloadEncoderParityTest {

    @Test
    fun `transfer asset encoding matches Rust fixture generator`() {
        val lines = FixtureGeneratorRunner.run("transfer-asset")
        val rustHex = lines[0]
        val assetId = lines[1]
        val amount = lines[2]
        val destinationAccountId = lines[3]

        val instruction = TransferWirePayloadEncoder.encodeAssetTransfer(
            assetId,
            amount,
            destinationAccountId,
        )

        assertEquals("iroha.transfer", instruction.name)
        val wirePayload = assertIs<WirePayload>(instruction.payload)
        val kotlinHex = FixtureGeneratorRunner.bytesToHex(wirePayload.payloadBytes)

        assertContentEquals(
            FixtureGeneratorRunner.hexToBytes(rustHex),
            wirePayload.payloadBytes,
            "Kotlin TransferBox encoding must match Rust. " +
                "If Rust data model changed, update TransferWirePayloadEncoder.\n" +
                "  Rust:   $rustHex\n" +
                "  Kotlin: $kotlinHex",
        )
    }
}
