package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.client.IdentifierReceiptAttestation
import org.hyperledger.iroha.sdk.client.IdentifierReceiptCanonicalEncoder
import org.hyperledger.iroha.sdk.client.IdentifierResolutionExecutionPayload
import org.hyperledger.iroha.sdk.client.IdentifierResolutionPayload
import org.hyperledger.iroha.sdk.client.IdentifierResolutionReceipt
import org.hyperledger.iroha.sdk.client.RamLfeOutputOpening
import org.hyperledger.iroha.sdk.client.RamLfeOutputOpeningPayload
import org.hyperledger.iroha.sdk.core.model.WirePayload
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
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
        val decodedClaim = ClaimIdentifierWirePayloadEncoder.decodePayload(wirePayload.payloadBytes)
        val expectedReceiptPayload = IdentifierReceiptCanonicalEncoder.encodePayload(payload)
        val expectedAttestationPayload = IdentifierReceiptCanonicalEncoder.encodeAttestation(receipt.attestation)

        assertEquals(accountId, decodedClaim.accountId)
        assertContentEquals(expectedReceiptPayload, decodedClaim.receiptPayloadBytes)
        assertContentEquals(expectedAttestationPayload, decodedClaim.attestationPayloadBytes)

        val decodedReceiptPayload = IdentifierReceiptCanonicalEncoder.decodePayload(
            decodedClaim.receiptPayloadBytes,
        )
        assertEquals(payload.policyId, decodedReceiptPayload.policyId)
        assertEquals(payload.execution.programId, decodedReceiptPayload.execution.programId)
        assertEquals(payload.execution.backend, decodedReceiptPayload.execution.backend)
        assertEquals(payload.execution.verificationMode, decodedReceiptPayload.execution.verificationMode)
        assertEquals(payload.execution.expiresAtMs, decodedReceiptPayload.execution.expiresAtMs)
        assertEquals(payload.opaqueId, decodedReceiptPayload.opaqueId)
        assertEquals(payload.uaid, decodedReceiptPayload.uaid)
        assertEquals(payload.accountId, decodedReceiptPayload.accountId)

        val decodedAttestation = IdentifierReceiptCanonicalEncoder.decodeAttestation(
            decodedClaim.attestationPayloadBytes,
        )
        assertEquals(receipt.attestation.kind, decodedAttestation.kind)
        assertEquals(requireNotNull(receipt.attestation.signature).lowercase(), decodedAttestation.signature)

        assertContentEquals(
            FixtureGeneratorRunner.hexToBytes(rustHex),
            wirePayload.payloadBytes,
            "Kotlin ClaimIdentifier encoding must match Rust. " +
                "If Rust data model changed, update ClaimIdentifierWirePayloadEncoder.\n" +
                "  Rust:   $rustHex\n" +
                "  Kotlin: $kotlinHex",
        )

        assertFailsWith<IllegalArgumentException> {
            ClaimIdentifierWirePayloadEncoder.decodePayload(wirePayload.payloadBytes.copyOf(12))
        }
        val mutated = wirePayload.payloadBytes.copyOf()
        mutated[mutated.lastIndex] = (mutated.last().toInt() xor 0x01).toByte()
        assertFailsWith<IllegalArgumentException> {
            ClaimIdentifierWirePayloadEncoder.decodePayload(mutated)
        }
    }

    @Test
    fun `identifier attestation decoding covers proof payloads and rejects bad tags`() {
        val attestation = IdentifierReceiptAttestation(
            kind = "proof",
            signature = null,
            proofBackend = "halo2/ipa",
            proofB64 = "AQID",
        )
        val encoded = IdentifierReceiptCanonicalEncoder.encodeAttestation(attestation)

        val decoded = IdentifierReceiptCanonicalEncoder.decodeAttestation(encoded)

        assertEquals("proof", decoded.kind)
        assertEquals("halo2/ipa", decoded.proofBackend)
        assertEquals("AQID", decoded.proofB64)

        assertFailsWith<IllegalArgumentException> {
            IdentifierReceiptCanonicalEncoder.decodeAttestation(byteArrayOf(2, 0, 0, 0))
        }
    }
}
