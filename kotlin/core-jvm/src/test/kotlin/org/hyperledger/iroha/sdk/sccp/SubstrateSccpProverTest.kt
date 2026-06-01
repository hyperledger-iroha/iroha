package org.hyperledger.iroha.sdk.sccp

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class SubstrateSccpProverTest {
    @Test
    fun proofRequestBindsRelayContext() {
        val bundleBytes = byteArrayOf(5, 6, 7)
        val sourceProofBytes = byteArrayOf(9, 10)
        val request = SccpSubstrate.buildProofRequest(
            sampleProofRequestInput(bundleBytes = bundleBytes, sourceProofBytes = sourceProofBytes),
        )

        assertEquals(SccpSubstrate.RUNTIME_PROOF_BACKEND_V1, request.backend)
        assertEquals(SccpSubstrate.DOMAIN_SORA, request.sourceDomain)
        assertEquals(SccpSubstrate.DOMAIN_SORA2, request.targetDomain)
        assertEquals(141, request.publicInputsBytes.size)
        assertEquals("0x" + "56".repeat(32), request.statementHash)
        assertEquals("0x" + "78".repeat(32), request.destinationBindingHash)
        assertTrue(request.requestHash.matches(Regex("0x[0-9a-f]{64}")))

        val kusamaRequest = SccpSubstrate.buildProofRequest(
            sampleProofRequestInput(
                publicInputs = samplePublicInputs(targetDomain = SccpSubstrate.DOMAIN_SORA_KUSAMA),
                sourceProofBytes = byteArrayOf(9, 10),
            ),
        )
        assertEquals(SccpSubstrate.DOMAIN_SORA_KUSAMA, kusamaRequest.targetDomain)
        assertTrue(request.requestHash != kusamaRequest.requestHash)
        val wrongSource = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.buildProofRequest(
                sampleProofRequestInput(sourceDomain = SccpTron.DOMAIN_TRON, sourceProofBytes = byteArrayOf(9, 10)),
            )
        }
        assertTrue(wrongSource.message?.contains("sourceDomain must be SORA") == true)
        assertTrue(
            request.requestHash != SccpSubstrate.buildProofRequest(
                sampleProofRequestInput(
                    bundleBytes = byteArrayOf(5, 6, 7, 9),
                    sourceProofBytes = byteArrayOf(10),
                ),
            ).requestHash,
        )

        val wrongTarget = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.buildProofRequest(
                sampleProofRequestInput(publicInputs = samplePublicInputs(targetDomain = SccpTon.DOMAIN_TON)),
            )
        }
        assertTrue(wrongTarget.message?.contains("Substrate-family") == true)

        val paddedPayload = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.buildProofRequest(
                sampleProofRequestInput(publicInputs = samplePublicInputs().copy(payloadHash = " " + "22".repeat(32))),
            )
        }
        assertTrue(paddedPayload.message?.contains("payloadHash") == true)
        assertTrue(paddedPayload.message?.contains("canonical hex") == true)

        val paddedStatement = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.buildProofRequest(sampleProofRequestInput(statementHash = "56".repeat(32) + " "))
        }
        assertTrue(paddedStatement.message?.contains("statementHash") == true)
        assertTrue(paddedStatement.message?.contains("canonical hex") == true)

        for (finalityHeight in listOf("042", "0x2a", "+42", " 42", "42 ")) {
            val invalidFinalityHeight = assertFailsWith<IllegalArgumentException> {
                SccpSubstrate.buildProofRequest(
                    sampleProofRequestInput(
                        publicInputs = samplePublicInputs(finalityHeight = finalityHeight),
                    ),
                )
            }
            assertTrue(invalidFinalityHeight.message?.contains("finalityHeight") == true)
        }

        val sameDomain = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.buildProofRequest(sampleProofRequestInput(sourceDomain = SccpSubstrate.DOMAIN_SORA2))
        }
        assertTrue(sameDomain.message?.contains("sourceDomain must be SORA") == true)

        val zeroDestination = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.buildProofRequest(
                sampleProofRequestInput(destinationBindingHash = "00".repeat(32)),
            )
        }
        assertTrue(zeroDestination.message?.contains("destinationBindingHash") == true)

        val emptyBundle = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.buildProofRequest(sampleProofRequestInput(bundleBytes = byteArrayOf()))
        }
        assertTrue(emptyBundle.message?.contains("bundleBytes") == true)
        val zeroBundle = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.buildProofRequest(sampleProofRequestInput(bundleBytes = byteArrayOf(0, 0)))
        }
        assertTrue(zeroBundle.message?.contains("all zero") == true)
        val oversizedBundle = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.buildProofRequest(
                sampleProofRequestInput(
                    bundleBytes = ByteArray(SccpSubstrate.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1) { 1 },
                ),
            )
        }
        assertTrue(oversizedBundle.message?.contains("at most") == true)

        val zeroSourceProof = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.buildProofRequest(sampleProofRequestInput(sourceProofBytes = byteArrayOf(0, 0)))
        }
        assertTrue(zeroSourceProof.message?.contains("sourceProofBytes must not be all zero") == true)
        val oversizedSourceProof = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.buildProofRequest(
                sampleProofRequestInput(
                    sourceProofBytes = ByteArray(SccpSubstrate.SOURCE_STATE_MAX_PROOF_BYTES + 1) { 1 },
                ),
            )
        }
        assertTrue(oversizedSourceProof.message?.contains("sourceProofBytes must be at most") == true)
        assertContentEquals(
            ByteArray(0),
            SccpSubstrate.buildProofRequest(sampleProofRequestInput(sourceProofBytes = ByteArray(0))).sourceProofBytes,
        )

        val wrongBackend = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.buildProofRequest(sampleProofRequestInput(backend = "debug-substrate-backend"))
        }
        assertTrue(wrongBackend.message?.contains("substrate-runtime-v1") == true)

        bundleBytes[0] = 99
        sourceProofBytes[0] = 99
        assertContentEquals(byteArrayOf(5, 6, 7), request.bundleBytes)
        assertContentEquals(byteArrayOf(9, 10), request.sourceProofBytes)

        val exposedPublicInputs = request.publicInputsBytes
        val exposedBundle = request.bundleBytes
        val exposedSourceProof = request.sourceProofBytes
        exposedPublicInputs[0] = 99
        exposedBundle[0] = 99
        exposedSourceProof[0] = 99
        assertTrue(request.publicInputsBytes[0].toInt() != 99)
        assertContentEquals(byteArrayOf(5, 6, 7), request.bundleBytes)
        assertContentEquals(byteArrayOf(9, 10), request.sourceProofBytes)

        val callbackSnapshot = SccpSubstrate.callbackRequestSnapshot(request)
        assertFalse(callbackSnapshot === request)
        assertEquals(request, callbackSnapshot)
        val snapshotBundle = callbackSnapshot.bundleBytes
        snapshotBundle[0] = 77
        assertContentEquals(byteArrayOf(5, 6, 7), callbackSnapshot.bundleBytes)
    }

    @Test
    fun runtimeCallSubmissionPackagesWrappedProofResult() {
        val request = SccpSubstrate.buildProofRequest(
            sampleProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)),
        )
        val proofResult = SccpSubstrate.wrapProofResult(byteArrayOf(1, 2, 3, 4), request)
        val submission = SccpSubstrate.buildSubmission(SubstrateSccpSubmissionInput(proofResult))

        assertEquals(SccpSubstrate.STARK_FRI_PROOF_FAMILY_V1, submission.proofFamily)
        assertEquals(SccpSubstrate.RUNTIME_PROOF_BACKEND_V1, submission.verifierBackend)
        assertEquals("substrate_runtime_call", submission.platformPayload)
        assertEquals(SccpSubstrate.RUNTIME_CALL_SCALE_V1, submission.envelopeEncoding)
        assertEquals("runtime_call", submission.submissionKind)
        assertEquals(SccpSubstrate.SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1, submission.verifierEntrypoint)
        assertEquals(SccpSubstrate.DOMAIN_SORA, submission.sourceDomain)
        assertEquals(SccpSubstrate.DOMAIN_SORA2, submission.targetDomain)
        assertEquals(request.requestHash, submission.requestHash)
        assertEquals(listOf("proof_bytes", "public_inputs", "bundle_bytes"), submission.arguments.map { it.key })
        assertContentEquals(submission.runtimeCall, submission.envelopeBytes)
        assertEquals(submission.runtimeCallHex, submission.envelopeHex)
        val expectedPrefix = byteArrayOf(0x7c) +
            SccpSubstrate.SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1.toByteArray(Charsets.UTF_8) +
            byteArrayOf(0x10)
        assertContentEquals(expectedPrefix, submission.runtimeCall.copyOfRange(0, expectedPrefix.size))
        assertContentEquals(byteArrayOf(1, 2, 3, 4), submission.proofBytes)
        assertContentEquals(request.publicInputsBytes, submission.publicInputsBytes)
        assertContentEquals(byteArrayOf(5, 6, 7), submission.bundleBytes)

        val explicitSubmission = SccpSubstrate.buildSubmission(
            SubstrateSccpSubmissionInput(
                publicInputs = samplePublicInputs(),
                proofBytes = byteArrayOf(1, 2, 3, 4),
                bundleBytes = byteArrayOf(5, 6, 7),
                sourceProofBytes = ByteArray(0),
                statementHash = "56".repeat(32),
                destinationBindingHash = "78".repeat(32),
            ),
        )
        assertContentEquals(submission.runtimeCall, explicitSubmission.runtimeCall)

        val rawSourceProof = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.buildSubmission(
                SubstrateSccpSubmissionInput(
                    publicInputs = samplePublicInputs(),
                    proofBytes = byteArrayOf(1, 2, 3, 4),
                    bundleBytes = byteArrayOf(5, 6, 7),
                    sourceProofBytes = byteArrayOf(9, 10),
                    statementHash = "56".repeat(32),
                    destinationBindingHash = "78".repeat(32),
                ),
            )
        }
        assertTrue(rawSourceProof.message?.contains("sourceProofBytes requires proofResult") == true)

        val wrongBundle = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.buildSubmission(
                SubstrateSccpSubmissionInput(
                    publicInputs = proofResult.publicInputs,
                    proofBytes = proofResult.proofBytes,
                    bundleBytes = byteArrayOf(5, 6, 8),
                    sourceProofBytes = proofResult.sourceProofBytes,
                    statementHash = proofResult.statementHash,
                    destinationBindingHash = proofResult.destinationBindingHash,
                    proofResult = proofResult,
                ),
            )
        }
        assertTrue(wrongBundle.message?.contains("bundleBytes") == true)

        val zeroBundle = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.buildSubmission(
                SubstrateSccpSubmissionInput(
                    publicInputs = samplePublicInputs(),
                    proofBytes = byteArrayOf(1, 2, 3, 4),
                    bundleBytes = byteArrayOf(0, 0),
                    sourceProofBytes = byteArrayOf(9, 10),
                    statementHash = "56".repeat(32),
                    destinationBindingHash = "78".repeat(32),
                ),
            )
        }
        assertTrue(zeroBundle.message?.contains("all zero") == true)

        val wrongEnvelope = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.buildSubmission(
                SubstrateSccpSubmissionInput(proofResult.copy(envelopeHash = "0x" + "aa".repeat(32))),
            )
        }
        assertTrue(wrongEnvelope.message?.contains("proofResult") == true)
    }

    @Test
    fun proverRequiresLinkedProofEngine() {
        val error = assertFailsWith<IllegalStateException> {
            SubstrateSccpProver().prove(sampleProofRequestInput())
        }
        assertTrue(error.message?.contains("not linked") == true)
    }

    @Test
    fun proverResolvesWitnessProviderBeforeBuildingRequest() {
        var resolved = false
        val bundleBytes = byteArrayOf(5, 6, 7)
        val input = sampleProofRequestInput(bundleBytes = bundleBytes)
        val prover = SubstrateSccpProver(
            witnessProvider = SubstrateSccpWitnessProvider { input ->
                assertContentEquals(ByteArray(0), input.sourceProofBytes)
                assertFalse(input.bundleBytes === bundleBytes)
                input.bundleBytes[0] = 0x7f
                resolved = true
                input.copy(sourceProofBytes = byteArrayOf(9, 10))
            },
            proofEngine = SubstrateSccpProofEngine { request ->
                assertTrue(resolved)
                assertContentEquals(byteArrayOf(9, 10), request.sourceProofBytes)
                byteArrayOf(1, 2, 3, 4)
            },
        )

        val result = prover.prove(input)

        assertContentEquals(byteArrayOf(9, 10), result.sourceProofBytes)
        assertContentEquals(byteArrayOf(5, 6, 7), input.bundleBytes)
        assertContentEquals(byteArrayOf(5, 6, 7), bundleBytes)
    }

    @Test
    fun proverWrapsExternalProofBytes() {
        val seenRequests = mutableListOf<SubstrateSccpProofRequest>()
        val prover = SubstrateSccpProver(
            proofEngine = SubstrateSccpProofEngine { request ->
                seenRequests.add(request)
                assertEquals(SccpSubstrate.RUNTIME_PROOF_BACKEND_V1, request.backend)
                assertEquals(SccpSubstrate.DOMAIN_SORA2, request.targetDomain)
                byteArrayOf(1, 2, 3, 4)
            },
        )

        val result = prover.prove(sampleProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)))
        val omittedSourceResult = prover.prove(sampleProofRequestInput())
        val expectedRequest = SccpSubstrate.buildProofRequest(
            sampleProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)),
        )
        val expectedOmittedSourceRequest = SccpSubstrate.buildProofRequest(sampleProofRequestInput())

        assertEquals(listOf(expectedRequest, expectedOmittedSourceRequest), seenRequests)
        assertFalse(seenRequests[0] === expectedRequest)
        assertFalse(seenRequests[1] === expectedOmittedSourceRequest)

        assertEquals(listOf(1, 2, 3, 4), result.proofBytes.map { it.toInt() })
        assertContentEquals(ByteArray(0), omittedSourceResult.sourceProofBytes)
        assertEquals("AQIDBA==", result.proofBase64)
        assertEquals("0x" + "56".repeat(32), result.statementHash)
        assertEquals("0x" + "78".repeat(32), result.destinationBindingHash)
        assertTrue(result.requestHash.matches(Regex("0x[0-9a-f]{64}")))
        assertTrue(result.envelopeHash.matches(Regex("0x[0-9a-f]{64}")))
        val request = expectedRequest
        val wrongBackend = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.wrapProofResult(byteArrayOf(1), request.copy(backend = "debug-substrate-backend"))
        }
        assertTrue(wrongBackend.message?.contains("substrate-runtime-v1") == true)

        val zeroProof = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.wrapProofResult(byteArrayOf(0, 0), request)
        }
        assertTrue(zeroProof.message?.contains("all zero") == true)

        val oversizedProof = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.wrapProofResult(
                ByteArray(SccpSubstrate.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1) { 1 },
                request,
            )
        }
        assertTrue(oversizedProof.message?.contains("at most") == true)

        val wrongRequestHash = assertFailsWith<IllegalArgumentException> {
            SccpSubstrate.wrapProofResult(byteArrayOf(1), request.copy(requestHash = "0x" + "99".repeat(32)))
        }
        assertTrue(wrongRequestHash.message?.contains("canonical") == true)

        val exposedProof = result.proofBytes
        exposedProof[0] = 9
        assertContentEquals(byteArrayOf(1, 2, 3, 4), result.proofBytes)

        val mutatedRequestView =
            SccpSubstrate.buildProofRequest(sampleProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)))
        mutatedRequestView.bundleBytes[0] = 9
        SccpSubstrate.wrapProofResult(byteArrayOf(1), mutatedRequestView)
        assertContentEquals(byteArrayOf(5, 6, 7), mutatedRequestView.bundleBytes)
    }

    private fun sampleProofRequestInput(
        publicInputs: SubstrateSccpPublicInputsInput = samplePublicInputs(),
        bundleBytes: ByteArray = byteArrayOf(5, 6, 7),
        sourceProofBytes: ByteArray = ByteArray(0),
        statementHash: String = "56".repeat(32),
        destinationBindingHash: String = "78".repeat(32),
        backend: String = SccpSubstrate.RUNTIME_PROOF_BACKEND_V1,
        sourceDomain: Int = SccpSubstrate.DOMAIN_SORA,
    ): SubstrateSccpProofRequestInput =
        SubstrateSccpProofRequestInput(
            publicInputs = publicInputs,
            bundleBytes = bundleBytes,
            sourceProofBytes = sourceProofBytes,
            statementHash = statementHash,
            destinationBindingHash = destinationBindingHash,
            backend = backend,
            sourceDomain = sourceDomain,
        )

    private fun samplePublicInputs(
        targetDomain: Int = SccpSubstrate.DOMAIN_SORA2,
        finalityHeight: String = "42",
    ): SubstrateSccpPublicInputsInput =
        SubstrateSccpPublicInputsInput(
            messageId = "21".repeat(32),
            payloadHash = "22".repeat(32),
            targetDomain = targetDomain,
            commitmentRoot = "23".repeat(32),
            finalityHeight = finalityHeight,
            finalityBlockHash = "24".repeat(32),
        )
}
