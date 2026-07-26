package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity
import org.hyperledger.iroha.sdk.sccp.SccpV1
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
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
    fun `transfer asset wire codec enforces canonical quantities`() {
        val lines = FixtureGeneratorRunner.run("transfer-asset")
        val assetId = lines[1]
        val destinationAccountId = lines[3]
        listOf(" ", "+1", "01", "1e0", "-1", "1.0", "1.2300").forEach { amount ->
            assertFailsWith<IllegalArgumentException> {
                TransferWirePayloadEncoder.encodeAssetTransfer(assetId, amount, destinationAccountId)
            }
        }

        val typed = TransferWirePayloadEncoder.encodeAssetTransfer(
            assetId,
            KotodamaQuantity.parseCanonical("10"),
            destinationAccountId,
        )
        val canonicalWire = assertIs<WirePayload>(typed.payload).payloadBytes
        val decoded = NoritoHeader.decode(canonicalWire, null)
        decoded.header.validateChecksum(decoded.payload)
        val payload = decoded.payload.copyOf()
        val canonicalNumeric = byteArrayOf(5, 1, 0, 0, 0, 10, 4, 0, 0, 0, 0)
        val numericOffset = payload.indexOfSubsequence(canonicalNumeric)
        require(numericOffset >= 0) { "canonical transfer numeric payload was not found" }
        payload[numericOffset + canonicalNumeric.lastIndex] = 1

        assertFailsWith<IllegalArgumentException> {
            TransferWirePayloadEncoder.decodeAssetTransferPayload(
                reframe(decoded.header, payload),
                SccpV1.TAIRA_I105_DISCRIMINANT_V1,
            )
        }
    }

    @Test
    fun `transfer asset encoding matches Rust fixture generator`() {
        assertTransferAssetParity("transfer-asset")
    }

    @Test
    fun `dataspace scoped transfer asset encoding matches Rust fixture generator`() {
        assertTransferAssetParity("transfer-asset-scoped")
    }

    private fun assertTransferAssetParity(fixtureName: String) {
        val lines = FixtureGeneratorRunner.run(fixtureName)
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
        val decoded =
            TransferWirePayloadEncoder.decodeAssetTransferPayload(
                wirePayload.payloadBytes,
                SccpV1.TAIRA_I105_DISCRIMINANT_V1,
            )

        assertEquals(assetId, decoded.assetId)
        assertEquals(amount, decoded.amount)
        assertEquals(destinationAccountId, decoded.destinationAccountId)
        assertEquals(
            SccpV1.TAIRA_I105_DISCRIMINANT_V1,
            AccountAddress.detectI105Discriminant(decoded.destinationAccountId),
        )

        assertContentEquals(
            FixtureGeneratorRunner.hexToBytes(rustHex),
            wirePayload.payloadBytes,
            "Kotlin TransferBox encoding must match Rust. " +
                "If Rust data model changed, update TransferWirePayloadEncoder.\n" +
                "  Rust:   $rustHex\n" +
                "  Kotlin: $kotlinHex",
        )

        assertFailsWith<IllegalArgumentException> {
            TransferWirePayloadEncoder.decodeAssetTransferPayload(
                wirePayload.payloadBytes.copyOf(12),
                SccpV1.TAIRA_I105_DISCRIMINANT_V1,
            )
        }
        val mutated = wirePayload.payloadBytes.copyOf()
        mutated[mutated.lastIndex] = (mutated.last().toInt() xor 0x01).toByte()
        assertFailsWith<IllegalArgumentException> {
            TransferWirePayloadEncoder.decodeAssetTransferPayload(
                mutated,
                SccpV1.TAIRA_I105_DISCRIMINANT_V1,
            )
        }
    }

    private fun reframe(header: NoritoHeader, payload: ByteArray): ByteArray {
        val reframed = NoritoHeader(
            schemaHash = header.schemaHash,
            payloadLength = payload.size,
            checksum = CRC64.compute(payload),
            flags = header.flags,
            compression = NoritoHeader.COMPRESSION_NONE,
            minor = header.minor,
        )
        return reframed.encode() + payload
    }

    private fun ByteArray.indexOfSubsequence(needle: ByteArray): Int {
        if (needle.isEmpty()) return 0
        for (start in 0..size - needle.size) {
            if (needle.indices.all { this[start + it] == needle[it] }) return start
        }
        return -1
    }
}
