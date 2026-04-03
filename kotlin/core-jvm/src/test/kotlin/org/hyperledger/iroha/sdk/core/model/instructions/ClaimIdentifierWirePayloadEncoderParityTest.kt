package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.client.IdentifierResolutionPayload
import org.hyperledger.iroha.sdk.client.IdentifierResolutionReceipt
import org.hyperledger.iroha.sdk.core.model.WirePayload
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertIs

/**
 * Parity test for [ClaimIdentifierWirePayloadEncoder].
 *
 * Runs `kotlin-fixture-gen claim-identifier` which encodes a `ClaimIdentifier`
 * instruction using the current Rust data model and outputs:
 * - Line 1: full wire payload hex
 * - Line 2: account I105
 * - Line 3: receipt payload bytes hex
 * - Line 4: signature bytes hex
 */
class ClaimIdentifierWirePayloadEncoderParityTest {

    @Test
    fun `claim identifier encoding matches Rust fixture generator`() {
        val lines = FixtureGeneratorRunner.run("claim-identifier")
        val rustHex = lines[0]
        val accountId = lines[1]
        val receiptPayloadHex = lines[2]
        val signatureHex = lines[3]

        val dummyPayload = IdentifierResolutionPayload(
            policyId = "unused",
            opaqueId = "unused",
            receiptHash = "unused",
            uaid = "unused",
            accountId = accountId,
            resolvedAtMs = 0L,
            expiresAtMs = null,
        )
        val receipt = IdentifierResolutionReceipt(
            policyId = "unused",
            opaqueId = "unused",
            receiptHash = "unused",
            uaid = "unused",
            accountId = accountId,
            resolvedAtMs = 0L,
            expiresAtMs = null,
            backend = "unused",
            signature = signatureHex,
            signaturePayloadHex = receiptPayloadHex,
            signaturePayload = dummyPayload,
        )

        val instruction = ClaimIdentifierWirePayloadEncoder.encode(accountId, receipt)

        assertEquals("identity::ClaimIdentifier", instruction.name)
        val wirePayload = assertIs<WirePayload>(instruction.payload)
        val kotlinHex = FixtureGeneratorRunner.bytesToHex(wirePayload.payloadBytes)

        assertContentEquals(
            FixtureGeneratorRunner.hexToBytes(rustHex),
            wirePayload.payloadBytes,
            "Kotlin ClaimIdentifier encoding must match Rust. " +
                "If Rust data model changed, update ClaimIdentifierWirePayloadEncoder.\n" +
                "  Rust:   $rustHex\n" +
                "  Kotlin: $kotlinHex",
        )
    }
}
