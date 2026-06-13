package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.address.AccountAddress
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
import kotlin.test.assertTrue

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

    @Test
    fun `claim identifier rejects account mismatch before encoding`() {
        val accountId = sampleAuthority(0x41)
        val payload = samplePayload(accountId = accountId)
        val receipt = IdentifierResolutionReceipt(
            payload = payload,
            attestation = IdentifierReceiptAttestation(
                kind = "signed",
                signature = "A1B2C3D4",
                proofBackend = null,
                proofB64 = null,
            ),
        )

        val err = assertFailsWith<IllegalArgumentException> {
            ClaimIdentifierWirePayloadEncoder.encode("${accountId}x", receipt)
        }
        assertTrue(
            err.message?.contains("ClaimIdentifier accountId must match receipt.accountId") == true,
            "mismatched ClaimIdentifier account must fail before encoding",
        )

        val paddedErr = assertFailsWith<IllegalArgumentException> {
            ClaimIdentifierWirePayloadEncoder.encode(" $accountId ", receipt)
        }
        assertTrue(
            paddedErr.message?.contains("accountId must not contain surrounding whitespace") == true,
            "padded ClaimIdentifier account must fail before encoding",
        )
    }

    @Test
    fun `identifier receipt canonical encoder rejects non-exact execution tags and proof backends`() {
        for (policyId in listOf(" phone#retail", "phone#retail ", "phone #retail", "phone# retail")) {
            assertFailsWith<IllegalArgumentException>("policy_id exactness $policyId") {
                IdentifierReceiptCanonicalEncoder.encodePayload(samplePayload(policyId = policyId))
            }
        }

        for (programId in listOf(" identifier_lookup_retail", "identifier_lookup_retail ")) {
            assertFailsWith<IllegalArgumentException>("execution program_id exactness $programId") {
                IdentifierReceiptCanonicalEncoder.encodePayload(samplePayload(programId = programId))
            }
            assertFailsWith<IllegalArgumentException>("opening program_id exactness $programId") {
                IdentifierReceiptCanonicalEncoder.encodePayload(samplePayload(openingProgramId = programId))
            }
        }

        val accountId = sampleAuthority(0x41)
        for (paddedAccountId in listOf(" $accountId", "$accountId ")) {
            assertFailsWith<IllegalArgumentException>("account_id exactness $paddedAccountId") {
                IdentifierReceiptCanonicalEncoder.encodePayload(samplePayload(accountId = paddedAccountId))
            }
        }

        for ((label, payload) in listOf(
            "opaque_id" to samplePayload(opaqueId = " " + "opaque:" + "44".repeat(32)),
            "receipt_hash" to samplePayload(receiptHash = "55".repeat(32) + " "),
            "uaid" to samplePayload(uaid = " " + "uaid:" + "66".repeat(32)),
            "program_digest" to samplePayload(programDigest = " " + "11".repeat(32)),
            "opening input_ciphertext_hash" to samplePayload(openingInputCiphertextHash = "ee".repeat(32) + " "),
        )) {
            assertFailsWith<IllegalArgumentException>("hash exactness $label") {
                IdentifierReceiptCanonicalEncoder.encodePayload(payload)
            }
        }

        for ((label, payload) in listOf(
            "executed_at_ms" to samplePayload(executedAtMs = -1L),
            "execution expires_at_ms" to samplePayload(executionExpiresAtMs = -1L),
            "opened_at_ms" to samplePayload(openedAtMs = -1L),
            "opening expires_at_ms" to samplePayload(openingExpiresAtMs = -1L),
        )) {
            assertFailsWith<IllegalArgumentException>("timestamp u64 $label") {
                IdentifierReceiptCanonicalEncoder.encodePayload(payload)
            }
        }

        for (backend in listOf(" bfv-affine-sha3-256-v1", "bfv-affine-sha3-256-v1 ", "BFV-AFFINE-SHA3-256-V1")) {
            assertFailsWith<IllegalArgumentException>("backend $backend") {
                IdentifierReceiptCanonicalEncoder.encodePayload(samplePayload(backend = backend))
            }
        }

        for (mode in listOf(" signed", "signed ", "Signed")) {
            assertFailsWith<IllegalArgumentException>("verification mode $mode") {
                IdentifierReceiptCanonicalEncoder.encodePayload(samplePayload(verificationMode = mode))
            }
        }

        for (kind in listOf(" signed", "signed ", "Signed")) {
            assertFailsWith<IllegalArgumentException>("attestation kind $kind") {
                IdentifierReceiptCanonicalEncoder.encodeAttestation(
                    IdentifierReceiptAttestation(
                        kind = kind,
                        signature = "A1B2C3D4",
                        proofBackend = null,
                        proofB64 = null,
                    ),
                )
            }
        }

        for (signature in listOf(" A1B2C3D4", "A1B2C3D4 ")) {
            assertFailsWith<IllegalArgumentException>("attestation signature $signature") {
                IdentifierReceiptCanonicalEncoder.encodeAttestation(
                    IdentifierReceiptAttestation(
                        kind = "signed",
                        signature = signature,
                        proofBackend = null,
                        proofB64 = null,
                    ),
                )
            }
        }

        for (signature in listOf(" a1b2c3d4", "a1b2c3d4 ")) {
            assertFailsWith<IllegalArgumentException>("opening signature $signature") {
                IdentifierReceiptCanonicalEncoder.encodePayload(samplePayload(openingSignature = signature))
            }
        }

        for (proofBackend in listOf(" halo2/ipa", "halo2/ipa ", " ")) {
            assertFailsWith<IllegalArgumentException>("proof backend $proofBackend") {
                IdentifierReceiptCanonicalEncoder.encodeAttestation(
                    IdentifierReceiptAttestation(
                        kind = "proof",
                        signature = null,
                        proofBackend = proofBackend,
                        proofB64 = "AQID",
                    ),
                )
            }
        }

        assertFailsWith<IllegalArgumentException>("malformed proof_b64") {
            IdentifierReceiptCanonicalEncoder.encodeAttestation(
                IdentifierReceiptAttestation(
                    kind = "proof",
                    signature = null,
                    proofBackend = "halo2/ipa",
                    proofB64 = "@@@",
                ),
            )
        }

        for (proofB64 in listOf(" AQID", "AQID ")) {
            assertFailsWith<IllegalArgumentException>("padded proof_b64 $proofB64") {
                IdentifierReceiptCanonicalEncoder.encodeAttestation(
                    IdentifierReceiptAttestation(
                        kind = "proof",
                        signature = null,
                        proofBackend = "halo2/ipa",
                        proofB64 = proofB64,
                    ),
                )
            }
        }

        val encoded = IdentifierReceiptCanonicalEncoder.encodeAttestation(
            IdentifierReceiptAttestation(
                kind = "proof",
                signature = null,
                proofBackend = "halo2/ipa",
                proofB64 = "AQID",
            ),
        )
        val mutated = encoded.copyOf()
        val needle = "halo2/ipa".toByteArray(Charsets.UTF_8)
        val offset = indexOf(mutated, needle)
        assertTrue(offset >= 0, "encoded proof backend must be present")
        mutated[offset + needle.lastIndex] = ' '.code.toByte()

        assertFailsWith<IllegalArgumentException> {
            IdentifierReceiptCanonicalEncoder.decodeAttestation(mutated)
        }
    }

    private fun samplePayload(
        policyId: String = "phone#retail",
        programId: String = "identifier_lookup_retail",
        openingProgramId: String = "identifier_lookup_retail",
        accountId: String? = null,
        backend: String = "bfv-affine-sha3-256-v1",
        verificationMode: String = "signed",
        openingSignature: String = "a1b2c3d4",
        opaqueId: String = "opaque:" + "44".repeat(32),
        receiptHash: String = "55".repeat(32),
        uaid: String = "uaid:" + "66".repeat(32),
        programDigest: String = "11".repeat(32),
        openingInputCiphertextHash: String = "ee".repeat(32),
        executedAtMs: Long = 42L,
        executionExpiresAtMs: Long? = 142L,
        openedAtMs: Long = 84L,
        openingExpiresAtMs: Long? = 184L,
    ): IdentifierResolutionPayload =
        IdentifierResolutionPayload(
            policyId = policyId,
            execution = IdentifierResolutionExecutionPayload(
                programId = programId,
                programDigest = programDigest,
                backend = backend,
                verificationMode = verificationMode,
                inputCiphertextHash = "aa".repeat(32),
                outputCiphertextHash = "bb".repeat(32),
                parameterDigest = "cc".repeat(32),
                evaluationKeyDigest = "dd".repeat(32),
                outputHash = "22".repeat(32),
                associatedDataHash = "33".repeat(32),
                executedAtMs = executedAtMs,
                expiresAtMs = executionExpiresAtMs,
            ),
            opening = RamLfeOutputOpening(
                payload = RamLfeOutputOpeningPayload(
                    programId = openingProgramId,
                    inputCiphertextHash = openingInputCiphertextHash,
                    outputCiphertextHash = "ee".repeat(32),
                    parameterDigest = "ee".repeat(32),
                    evaluationKeyDigest = "ee".repeat(32),
                    openedOutputHash = "ee".repeat(32),
                    openedAtMs = openedAtMs,
                    expiresAtMs = openingExpiresAtMs,
                ),
                signature = openingSignature,
            ),
            opaqueId = opaqueId,
            receiptHash = receiptHash,
            uaid = uaid,
            accountId = accountId ?: sampleAuthority(0x41),
        )

    private fun sampleAuthority(fill: Int): String = AccountAddress
        .fromAccount(ByteArray(32) { fill.toByte() }, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

    private fun indexOf(haystack: ByteArray, needle: ByteArray): Int {
        if (needle.isEmpty() || haystack.size < needle.size) return -1
        for (offset in 0..(haystack.size - needle.size)) {
            var matches = true
            for (index in needle.indices) {
                if (haystack[offset + index] != needle[index]) {
                    matches = false
                    break
                }
            }
            if (matches) return offset
        }
        return -1
    }
}
