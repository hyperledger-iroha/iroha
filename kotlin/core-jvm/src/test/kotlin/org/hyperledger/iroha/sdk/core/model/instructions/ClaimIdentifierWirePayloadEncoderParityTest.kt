package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.client.IdentifierReceiptAttestation
import org.hyperledger.iroha.sdk.client.IdentifierResolutionExecutionPayload
import org.hyperledger.iroha.sdk.client.IdentifierResolutionPayload
import org.hyperledger.iroha.sdk.client.IdentifierResolutionReceipt
import org.hyperledger.iroha.sdk.client.RamLfeOutputOpening
import org.hyperledger.iroha.sdk.client.RamLfeOutputOpeningPayload
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
 * - Line 3: signature bytes hex
 * - Line 4: canonical receipt hash hex
 */
class ClaimIdentifierWirePayloadEncoderParityTest {

    @Test
    fun `claim identifier encoding matches Rust fixture generator`() {
        val lines = FixtureGeneratorRunner.run("claim-identifier")
        val rustHex = lines[0]
        val accountId = lines[1]
        val signatureHex = lines[2]
        val fixtureHash = lines[3]

        val payload = IdentifierResolutionPayload(
            policyId = "phone#e164",
            execution = IdentifierResolutionExecutionPayload(
                programId = "parity_test",
                programDigest = fixtureHash,
                backend = "hkdf-sha3-512-prf-v1",
                verificationMode = "signed",
                inputCiphertextHash = fixtureHash,
                outputCiphertextHash = fixtureHash,
                parameterDigest = fixtureHash,
                evaluationKeyDigest = fixtureHash,
                outputHash = fixtureHash,
                associatedDataHash = fixtureHash,
                executedAtMs = 1_735_000_000_000L,
                expiresAtMs = null,
            ),
            opening = RamLfeOutputOpening(
                payload = RamLfeOutputOpeningPayload(
                    programId = "parity_test",
                    inputCiphertextHash = fixtureHash,
                    outputCiphertextHash = fixtureHash,
                    parameterDigest = fixtureHash,
                    evaluationKeyDigest = fixtureHash,
                    openedOutputHash = fixtureHash,
                    openedAtMs = 1_735_000_000_000L,
                    expiresAtMs = null,
                ),
                signature = signatureHex,
            ),
            opaqueId = "opaque:$fixtureHash",
            receiptHash = fixtureHash,
            uaid = "uaid:$fixtureHash",
            accountId = accountId,
        )
        val receipt = IdentifierResolutionReceipt(
            payload = payload,
            attestation = IdentifierReceiptAttestation(
                kind = "signed",
                signature = signatureHex,
                proofBackend = null,
                proofB64 = null,
            ),
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
