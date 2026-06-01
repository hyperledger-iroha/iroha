package org.hyperledger.iroha.sdk.sccp

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class TronSccpProverTest {
    @Test
    fun derivesTronRouteCanaryEvidenceHash() {
        val evidence = sampleTronRouteCanaryEvidence()

        assertEquals(551, SccpTron.canonicalRouteCanaryEvidenceBytes(evidence).size)
        assertEquals(
            "0xe0a96ff7e8f523599fd60fffe8bb3b9fda9519126b7ba00c89c922b323b64e56",
            SccpTron.routeCanaryEvidenceHash(evidence),
        )

        assertFailsWith<IllegalArgumentException> {
            SccpTron.routeCanaryEvidenceHash(evidence.copy(routeAllowlistHash = "0x" + "78".repeat(32)))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTron.routeCanaryEvidenceHash(evidence.copy(destinationBindingHash = "0x" + "78".repeat(32)))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTron.routeCanaryEvidenceHash(evidence.copy(targetDomain = SccpSourceProofs.DOMAIN_ETH))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTron.routeCanaryEvidenceHash(evidence.copy(blockNumber = "0"))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTron.routeCanaryEvidenceHash(evidence.copy(usedMessageProof = false))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTron.routeCanaryEvidenceHash(evidence.copy(rawDataOwnerMatchesTransaction = false))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTron.routeCanaryEvidenceHash(evidence.copy(signatureRecoversToOwner = false))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTron.routeCanaryEvidenceHash(
                evidence.copy(signatureRecoveredAddress = "0x41" + "12".repeat(20)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTron.routeCanaryEvidenceHash(
                evidence.copy(expectedDestinationBindingHash = "0x" + "78".repeat(32)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpTron.routeCanaryEvidenceHash(evidence.copy(routeCanaryEvidenceHash = "0x" + "78".repeat(32)))
        }
    }

    @Test
    fun derivesGroth16PublicSignalWords() {
        val signals = SccpTron.groth16Bn254PublicSignalWords(
            publicInputs = samplePublicInputs(),
            sourceDomain = SccpTron.DOMAIN_SORA,
            statementHash = "55".repeat(32),
            destinationBindingHash = "66".repeat(32),
        )

        assertEquals(
            listOf(
                "0x0ffdbc782e79d1dc508e08af01e87f16d93b6e58e4861a0b8155455e3ee7a683",
                "0x0c5398ea95021a790e276e3ece1592b32b85751dc77e50293c867a5f2e0131bb",
                "0x21aac4195d8db839756f61c0780675823e15456c92acf135c36e02367c8fd11f",
                "0x01c73f2f9156a52493a9beabeec73e62deed32fcef2e3e6fac86a79f0764f0bc",
                "0x0ca6bbc36d23183d027c8df09f06c39e64abbb0bb4d6a4c37369d2c36f41a888",
                "0x2b153d0fe1bc6e2a6d44e851523edb1511dac55443ca80c22cbe9cb7423886dc",
                "0x2697e4e42f34b673b4aa254c6a92de09304e84c1a667c7d266777775a231efb4",
                "0x16fbe0c1d659f142b3e7815b24df66da3cfd89cc42d051b04bc31aae6925c396",
                "0x1157cd422e2089145c9cf93794dd6a0a1c3b1a611c22a5fe999d0542f62535d8",
            ),
            signals,
        )

        val changed = SccpTron.groth16Bn254PublicSignalWords(
            publicInputs = samplePublicInputs(),
            sourceDomain = SccpTron.DOMAIN_SORA,
            statementHash = "55".repeat(32),
            destinationBindingHash = "67".repeat(32),
        )
        assertEquals(signals.take(8), changed.take(8))
        assertTrue(signals[8] != changed[8])
    }

    @Test
    fun proofRequestBindsPublicSignalsAndRelayContext() {
        val bundleBytes = byteArrayOf(5, 6, 7)
        val sourceProofBytes = byteArrayOf(9, 10)
        val request = SccpTron.buildProofRequest(
            sampleProofRequestInput(bundleBytes = bundleBytes, sourceProofBytes = sourceProofBytes),
        )

        assertEquals(SccpTron.GROTH16_BN254_PROOF_BACKEND_V1, request.backend)
        assertEquals(SccpTron.DOMAIN_SORA, request.sourceDomain)
        assertEquals(SccpTron.DOMAIN_TRON, request.targetDomain)
        assertEquals(
            SccpTron.groth16Bn254PublicSignalWords(
                publicInputs = samplePublicInputs(),
                sourceDomain = SccpTron.DOMAIN_SORA,
                statementHash = "56".repeat(32),
                destinationBindingHash = "78".repeat(32),
            ),
            request.publicSignalWords,
        )
        assertEquals("0x" + "56".repeat(32), request.statementHash)
        assertEquals("0x" + "78".repeat(32), request.destinationBindingHash)
        assertTrue(request.requestHash.matches(Regex("0x[0-9a-f]{64}")))

        val destinationBinding = sampleDestinationBinding()
        val boundRequest = SccpTron.buildProofRequest(
            TronSccpProofRequestInput(
                publicInputs = samplePublicInputs(),
                bundleBytes = byteArrayOf(5, 6, 7),
                sourceProofBytes = byteArrayOf(9, 10),
                statementHash = "56".repeat(32),
                destinationBinding = destinationBinding,
            ),
        )
        assertEquals(destinationBinding.hash, boundRequest.destinationBindingHash)
        assertEquals(destinationBinding, boundRequest.destinationBinding)
        assertTrue(request.requestHash != boundRequest.requestHash)

        assertTrue(
            request.requestHash != SccpTron.buildProofRequest(
                sampleProofRequestInput(sourceProofBytes = byteArrayOf(9, 11)),
            ).requestHash,
        )
        assertTrue(
            request.requestHash != SccpTron.buildProofRequest(
                sampleProofRequestInput(
                    bundleBytes = byteArrayOf(5, 6, 7, 9),
                    sourceProofBytes = byteArrayOf(10),
                ),
            ).requestHash,
        )
        val error = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildProofRequest(sampleProofRequestInput(statementHash = ""))
        }
        assertTrue(error.message?.contains("statementHash") == true)

        val zeroPayload = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildProofRequest(
                sampleProofRequestInput(publicInputs = samplePublicInputs(payloadHash = "00".repeat(32))),
            )
        }
        assertTrue(zeroPayload.message?.contains("payloadHash") == true)

        val paddedPayload = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildProofRequest(
                sampleProofRequestInput(publicInputs = samplePublicInputs(payloadHash = " " + "22".repeat(32))),
            )
        }
        assertTrue(paddedPayload.message?.contains("payloadHash") == true)
        assertTrue(paddedPayload.message?.contains("canonical hex") == true)

        val paddedStatement = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildProofRequest(sampleProofRequestInput(statementHash = "56".repeat(32) + " "))
        }
        assertTrue(paddedStatement.message?.contains("statementHash") == true)
        assertTrue(paddedStatement.message?.contains("canonical hex") == true)

        for (finalityHeight in listOf("019", "0x13", "+19", " 19", "19 ")) {
            val invalidFinalityHeight = assertFailsWith<IllegalArgumentException> {
                SccpTron.buildProofRequest(
                    sampleProofRequestInput(
                        publicInputs = samplePublicInputs(finalityHeight = finalityHeight),
                    ),
                )
            }
            assertTrue(invalidFinalityHeight.message?.contains("finalityHeight") == true)
        }

        val zeroFinalityHeight = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildProofRequest(
                sampleProofRequestInput(publicInputs = samplePublicInputs(finalityHeight = "0")),
            )
        }
        assertTrue(zeroFinalityHeight.message?.contains("finalityHeight") == true)

        val emptyBundle = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildProofRequest(sampleProofRequestInput(bundleBytes = byteArrayOf()))
        }
        assertTrue(emptyBundle.message?.contains("bundleBytes") == true)

        val zeroSourceProof = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildProofRequest(sampleProofRequestInput(sourceProofBytes = byteArrayOf(0, 0)))
        }
        assertTrue(zeroSourceProof.message?.contains("sourceProofBytes must not be all zero") == true)
        val oversizedSourceProof = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildProofRequest(
                sampleProofRequestInput(
                    sourceProofBytes = ByteArray(SccpTron.SOURCE_STATE_MAX_PROOF_BYTES + 1) { 1 },
                ),
            )
        }
        assertTrue(oversizedSourceProof.message?.contains("sourceProofBytes must be at most") == true)
        assertContentEquals(
            ByteArray(0),
            SccpTron.buildProofRequest(sampleProofRequestInput(sourceProofBytes = ByteArray(0))).sourceProofBytes,
        )

        val wrongSource = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildProofRequest(sampleProofRequestInput(sourceDomain = SccpSourceProofs.DOMAIN_ETH))
        }
        assertTrue(wrongSource.message?.contains("sourceDomain must be SORA") == true)

        val wrongTarget = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildProofRequest(
                sampleProofRequestInput(publicInputs = samplePublicInputs(targetDomain = SccpTon.DOMAIN_TON)),
            )
        }
        assertTrue(wrongTarget.message?.contains("targetDomain must be TRON") == true)

        val zeroDestination = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildProofRequest(
                sampleProofRequestInput(destinationBindingHash = "00".repeat(32)),
            )
        }
        assertTrue(zeroDestination.message?.contains("destinationBindingHash") == true)

        val wrongBackend = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildProofRequest(sampleProofRequestInput(backend = "debug-tron-backend"))
        }
        assertTrue(wrongBackend.message?.contains("tron-groth16-bn254-v1") == true)

        val wrongBindingSource = assertFailsWith<IllegalArgumentException> {
            TronSccpProofRequestInput(
                publicInputs = samplePublicInputs(),
                bundleBytes = byteArrayOf(5, 6, 7),
                statementHash = "56".repeat(32),
                destinationBinding = destinationBinding.copy(sourceDomain = SccpSourceProofs.DOMAIN_ETH),
            )
        }
        assertTrue(wrongBindingSource.message?.contains("destinationBinding.sourceDomain") == true)

        val forgedBindingHash = assertFailsWith<IllegalArgumentException> {
            TronSccpProofRequestInput(
                publicInputs = samplePublicInputs(),
                bundleBytes = byteArrayOf(5, 6, 7),
                statementHash = "56".repeat(32),
                destinationBinding = destinationBinding.copy(hash = "0x" + "99".repeat(32)),
            )
        }
        assertTrue(forgedBindingHash.message?.contains("deployment material") == true)

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

        val callbackSnapshot = SccpTron.callbackRequestSnapshot(request)
        assertFalse(callbackSnapshot === request)
        assertEquals(request, callbackSnapshot)
        val snapshotBundle = callbackSnapshot.bundleBytes
        val snapshotSourceProof = callbackSnapshot.sourceProofBytes
        snapshotBundle[0] = 77
        snapshotSourceProof[0] = 77
        assertContentEquals(byteArrayOf(5, 6, 7), callbackSnapshot.bundleBytes)
        assertContentEquals(byteArrayOf(9, 10), callbackSnapshot.sourceProofBytes)
    }

    @Test
    fun proverRequiresLinkedProofEngine() {
        val error = assertFailsWith<IllegalStateException> {
            TronSccpProver().prove(sampleProofRequestInput())
        }
        assertTrue(error.message?.contains("not linked") == true)
    }

    @Test
    fun proverResolvesWitnessProviderBeforeBuildingRequest() {
        var resolved = false
        val proofBytes = sampleGroth16ProofBytes()
        val bundleBytes = byteArrayOf(5, 6, 7)
        val input = sampleProductionProofRequestInput(bundleBytes = bundleBytes)
        val prover = TronSccpProver(
            witnessProvider = TronSccpWitnessProvider { input ->
                assertContentEquals(ByteArray(0), input.sourceProofBytes)
                assertFalse(input.bundleBytes === bundleBytes)
                input.bundleBytes[0] = 0x7f
                resolved = true
                input.copy(sourceProofBytes = byteArrayOf(9, 10))
            },
            proofEngine = TronSccpProofEngine { request ->
                assertTrue(resolved)
                assertContentEquals(byteArrayOf(9, 10), request.sourceProofBytes)
                proofBytes
            },
        )

        val result = prover.prove(input)

        assertContentEquals(byteArrayOf(9, 10), result.sourceProofBytes)
        assertContentEquals(byteArrayOf(5, 6, 7), input.bundleBytes)
        assertContentEquals(byteArrayOf(5, 6, 7), bundleBytes)
    }

    @Test
    fun proverWrapsExternalProofBytes() {
        val proofBytes = sampleGroth16ProofBytes()
        val seenRequests = mutableListOf<TronSccpProofRequest>()
        val prover = TronSccpProver(
            proofEngine = TronSccpProofEngine { request ->
                seenRequests.add(request)
                assertEquals(SccpTron.GROTH16_BN254_PROOF_BACKEND_V1, request.backend)
                assertEquals(9, request.publicSignalWords.size)
                proofBytes
            },
        )

        val result = prover.prove(sampleProductionProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)))
        val omittedSourceResult = prover.prove(sampleProductionProofRequestInput())
        val expectedRequest = SccpTron.buildProofRequest(
            sampleProductionProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)),
        )
        val expectedOmittedSourceRequest = SccpTron.buildProofRequest(sampleProductionProofRequestInput())

        assertEquals(listOf(expectedRequest, expectedOmittedSourceRequest), seenRequests)
        assertFalse(seenRequests[0] === expectedRequest)
        assertFalse(seenRequests[1] === expectedOmittedSourceRequest)

        assertContentEquals(proofBytes, result.proofBytes)
        assertContentEquals(ByteArray(0), omittedSourceResult.sourceProofBytes)
        assertTrue(result.proofBase64.isNotEmpty())
        assertEquals("0x" + "56".repeat(32), result.statementHash)
        assertEquals(expectedRequest.destinationBindingHash, result.destinationBindingHash)
        assertEquals(expectedRequest.destinationBinding, result.destinationBinding)
        assertEquals(expectedRequest.requestHash, result.requestHash)
        assertTrue(result.envelopeHash.matches(Regex("0x[0-9a-f]{64}")))

        val request = expectedRequest
        val zeroProof = assertFailsWith<IllegalArgumentException> {
            SccpTron.wrapProofResult(byteArrayOf(0, 0), request)
        }
        assertTrue(zeroProof.message?.contains("all zero") == true)

        val shortProof = assertFailsWith<IllegalArgumentException> {
            SccpTron.wrapProofResult(byteArrayOf(1, 2, 3, 4), request)
        }
        assertTrue(shortProof.message?.contains("384 bytes") == true)

        val wrongBackend = assertFailsWith<IllegalArgumentException> {
            SccpTron.wrapProofResult(byteArrayOf(1), request.copy(backend = "debug-tron-backend"))
        }
        assertTrue(wrongBackend.message?.contains("tron-groth16-bn254-v1") == true)

        val wrongRequestHash = assertFailsWith<IllegalArgumentException> {
            SccpTron.wrapProofResult(proofBytes, request.copy(requestHash = "0x" + "99".repeat(32)))
        }
        assertTrue(wrongRequestHash.message?.contains("canonical") == true)

        val hashOnlyRequest = SccpTron.buildProofRequest(
            sampleProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)),
        )
        val missingBinding = assertFailsWith<IllegalArgumentException> {
            SccpTron.wrapProofResult(proofBytes, hashOnlyRequest)
        }
        assertTrue(missingBinding.message?.contains("destinationBinding") == true)

        val exposedProof = result.proofBytes
        exposedProof[0] = 9
        assertContentEquals(proofBytes, result.proofBytes)

        val mutatedRequestView = SccpTron.buildProofRequest(
            sampleProductionProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)),
        )
        mutatedRequestView.bundleBytes[0] = 9
        SccpTron.wrapProofResult(proofBytes, mutatedRequestView)
        assertContentEquals(byteArrayOf(5, 6, 7), mutatedRequestView.bundleBytes)
    }

    @Test
    fun rejectsMalformedGroth16ProofTuple() {
        val request = SccpTron.buildProofRequest(sampleProductionProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)))

        val wrongVersion = assertFailsWith<IllegalArgumentException> {
            SccpTron.wrapProofResult(sampleGroth16ProofBytes(mapOf(0 to abiWord(2))), request)
        }
        assertTrue(wrongVersion.message?.contains("proofBytes.version") == true)

        val outOfRangeCoordinate = assertFailsWith<IllegalArgumentException> {
            SccpTron.wrapProofResult(sampleGroth16ProofBytes(mapOf(4 to ByteArray(32) { 0xff.toByte() })), request)
        }
        assertTrue(outOfRangeCoordinate.message?.contains("BN254 base-field") == true)

        val zeroA = assertFailsWith<IllegalArgumentException> {
            SccpTron.wrapProofResult(
                sampleGroth16ProofBytes(
                    mapOf(
                        4 to ByteArray(32),
                        5 to ByteArray(32),
                    ),
                ),
                request,
            )
        }
        assertTrue(zeroA.message?.contains("proofBytes.a") == true)

        val zeroB = assertFailsWith<IllegalArgumentException> {
            SccpTron.wrapProofResult(
                sampleGroth16ProofBytes(
                    mapOf(
                        6 to ByteArray(32),
                        7 to ByteArray(32),
                        8 to ByteArray(32),
                        9 to ByteArray(32),
                    ),
                ),
                request,
            )
        }
        assertTrue(zeroB.message?.contains("proofBytes.b") == true)

        val zeroC = assertFailsWith<IllegalArgumentException> {
            SccpTron.wrapProofResult(
                sampleGroth16ProofBytes(
                    mapOf(
                        10 to ByteArray(32),
                        11 to ByteArray(32),
                    ),
                ),
                request,
            )
        }
        assertTrue(zeroC.message?.contains("proofBytes.c") == true)

        val offCurveB = assertFailsWith<IllegalArgumentException> {
            SccpTron.wrapProofResult(sampleGroth16ProofBytes(mapOf(6 to abiWord(4))), request)
        }
        assertTrue(offCurveB.message?.contains("proofBytes.b") == true)

        val nonSubgroupB = assertFailsWith<IllegalArgumentException> {
            SccpTron.wrapProofResult(
                sampleGroth16ProofBytes(
                    mapOf(
                        6 to abiWord(0),
                        7 to abiWord(1),
                        8 to hexWord("0cf32d3c49a2cb8a092f24ec3201e68dc299b6216e6321ee60573e3a7f596ea8"),
                        9 to hexWord("07bca656753ef8cbee60335acbffe3def91636952d4ab9eb0b839c7f3566c0e2"),
                    ),
                ),
                request,
            )
        }
        assertTrue(nonSubgroupB.message?.contains("proofBytes.b") == true)

        val mismatchedMessageId = assertFailsWith<IllegalArgumentException> {
            SccpTron.wrapProofResult(sampleGroth16ProofBytes(mapOf(1 to repeatedWord(0x22))), request)
        }
        assertTrue(mismatchedMessageId.message?.contains("messageId must match") == true)

        val mismatchedSourceDomain = assertFailsWith<IllegalArgumentException> {
            SccpTron.wrapProofResult(sampleGroth16ProofBytes(mapOf(2 to abiWord(999))), request)
        }
        assertTrue(mismatchedSourceDomain.message?.contains("sourceDomain must match") == true)
        val directMismatchedSourceDomain = assertFailsWith<IllegalArgumentException> {
            SccpTron.submitMessageProofCallData(
                sampleGroth16ProofBytes(mapOf(2 to abiWord(SccpSourceProofs.DOMAIN_ETH.toLong()))),
                samplePublicInputs(),
                "0x" + "56".repeat(32),
            )
        }
        assertTrue(directMismatchedSourceDomain.message?.contains("sourceDomain must match") == true)
        val directWrongSourceDomain = assertFailsWith<IllegalArgumentException> {
            SccpTron.submitMessageProofCallData(
                sampleGroth16ProofBytes(mapOf(2 to abiWord(SccpSourceProofs.DOMAIN_ETH.toLong()))),
                samplePublicInputs(),
                "0x" + "56".repeat(32),
                SccpSourceProofs.DOMAIN_ETH,
            )
        }
        assertTrue(directWrongSourceDomain.message?.contains("sourceDomain must be SORA") == true)

        val mismatchedCommitmentRoot = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildSubmission(
                TronSccpSubmissionInput(
                    proofBytes = sampleGroth16ProofBytes(mapOf(3 to repeatedWord(0x44))),
                    publicInputs = samplePublicInputs(),
                    statementHash = "0x" + "56".repeat(32),
                    destinationBindingHash = "0x" + "78".repeat(32),
                ),
            )
        }
        assertTrue(mismatchedCommitmentRoot.message?.contains("commitmentRoot must match") == true)
    }

    @Test
    fun buildsContractCallSubmission() {
        val proofBytes = sampleGroth16ProofBytes()
        val request = SccpTron.buildProofRequest(sampleProductionProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)))
        val proofResult = SccpTron.wrapProofResult(proofBytes, request)
        val submission = SccpTron.buildSubmission(TronSccpSubmissionInput(proofResult))

        assertEquals("contract_call", submission.submissionKind)
        assertEquals("tron_contract_call", submission.platformPayload)
        assertEquals(SccpTron.CONTRACT_CALL_ABI_TUPLE_V1, submission.envelopeEncoding)
        assertEquals(SccpTron.SUBMIT_MESSAGE_PROOF_SELECTOR_V1, submission.functionSelector)
        assertTrue(submission.callDataHex.startsWith(SccpTron.SUBMIT_MESSAGE_PROOF_SELECTOR_V1))
        assertEquals(676, submission.callData.size)
        assertEquals("0x" + "00".repeat(30) + "0100", "0x" + hexLower(submission.callData.copyOfRange(4, 36)))
        assertEquals("0x" + "00".repeat(30) + "0180", "0x" + hexLower(submission.callData.copyOfRange(260, 292)))
        assertEquals(SccpTron.messageTransparentPublicInputAbiWords(samplePublicInputs()), submission.publicInputWords)
        assertEquals(proofResult.publicSignalWords, submission.publicSignalWords)
        assertContentEquals(byteArrayOf(5, 6, 7), proofResult.bundleBytes)
        assertContentEquals(byteArrayOf(9, 10), proofResult.sourceProofBytes)
        assertContentEquals(proofBytes, submission.proofBytes)
        assertContentEquals(submission.callData, submission.envelopeBytes)
        assertContentEquals(
            submission.callData,
            SccpTron.submitMessageProofCallData(
                proofBytes,
                proofResult.publicInputs,
                proofResult.statementHash,
            ),
        )
        val destinationBinding = sampleDestinationBinding()
        val bindingSubmission = SccpTron.buildSubmission(
            TronSccpSubmissionInput(
                publicInputs = proofResult.publicInputs,
                proofBytes = proofBytes,
                statementHash = proofResult.statementHash,
                destinationBinding = destinationBinding,
            ),
        )
        assertEquals(destinationBinding.hash, bindingSubmission.destinationBindingHash)

        val omittedSourceProofResult = SccpTron.wrapProofResult(
            proofBytes,
            SccpTron.buildProofRequest(sampleProductionProofRequestInput()),
        )
        val omittedSourceSubmission =
            SccpTron.buildSubmission(TronSccpSubmissionInput(omittedSourceProofResult))
        assertContentEquals(ByteArray(0), omittedSourceProofResult.sourceProofBytes)
        assertContentEquals(proofBytes, omittedSourceSubmission.proofBytes)

        val exposedCallData = submission.callData
        exposedCallData[0] = 0
        assertTrue(submission.callData[0].toInt() != 0)

        val proofMismatch = proofBytes.copyOf()
        proofMismatch[4 * 32 + 31] = 9
        val proofMismatchError = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildSubmission(
                TronSccpSubmissionInput(
                    publicInputs = proofResult.publicInputs,
                    proofBytes = proofMismatch,
                    statementHash = proofResult.statementHash,
                    destinationBindingHash = proofResult.destinationBindingHash,
                    proofResult = proofResult,
                ),
            )
        }
        assertTrue(proofMismatchError.message?.contains("proofBytes") == true)

        val wrongBindingSource = assertFailsWith<IllegalArgumentException> {
            TronSccpSubmissionInput(
                publicInputs = proofResult.publicInputs,
                proofBytes = proofBytes,
                statementHash = proofResult.statementHash,
                destinationBinding = destinationBinding.copy(sourceDomain = SccpSourceProofs.DOMAIN_ETH),
            )
        }
        assertTrue(wrongBindingSource.message?.contains("destinationBinding.sourceDomain") == true)

        val tamperedEnvelopeError = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildSubmission(
                TronSccpSubmissionInput(
                    proofResult = proofResult.copy(envelopeHash = "0x" + "aa".repeat(32)),
                ),
            )
        }
        assertTrue(tamperedEnvelopeError.message?.contains("wrapped proof bytes") == true)

        val tamperedBase64Error = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildSubmission(
                TronSccpSubmissionInput(
                    proofResult = proofResult.copy(proofBase64 = "AAAA"),
                ),
            )
        }
        assertTrue(tamperedBase64Error.message?.contains("proofBase64") == true)

        val staleRequestError = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildSubmission(
                TronSccpSubmissionInput(
                    proofResult = proofResult.copy(bundleBytes = byteArrayOf(5, 6, 8)),
                ),
            )
        }
        assertTrue(staleRequestError.message?.contains("requestHash") == true)

        val signalMismatchError = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildSubmission(
                TronSccpSubmissionInput(
                    publicInputs = proofResult.publicInputs,
                    proofBytes = proofBytes,
                    statementHash = proofResult.statementHash,
                    destinationBindingHash = proofResult.destinationBindingHash,
                    publicSignalWords = listOf("0x" + "99".repeat(32)) + proofResult.publicSignalWords.drop(1),
                ),
            )
        }
        assertTrue(signalMismatchError.message?.contains("publicSignalWords") == true)

        val wrongTargetError = assertFailsWith<IllegalArgumentException> {
            SccpTron.buildSubmission(
                TronSccpSubmissionInput(
                    publicInputs = samplePublicInputs(targetDomain = SccpTon.DOMAIN_TON),
                    proofBytes = proofBytes,
                    statementHash = proofResult.statementHash,
                    destinationBindingHash = proofResult.destinationBindingHash,
                ),
            )
        }
        assertTrue(wrongTargetError.message?.contains("TRON") == true)
    }

    private fun sampleGroth16ProofBytes(overrides: Map<Int, ByteArray> = emptyMap()): ByteArray {
        val words = mutableListOf(
            abiWord(1),
            repeatedWord(0x11),
            abiWord(SccpTron.DOMAIN_SORA.toLong()),
            repeatedWord(0x33),
            abiWord(1),
            abiWord(2),
            hexWord("1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed"),
            hexWord("198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2"),
            hexWord("12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa"),
            hexWord("090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b"),
            abiWord(1),
            abiWord(2),
        )
        for ((index, word) in overrides) {
            words[index] = word.copyOf()
        }
        val out = ByteArray(SccpTron.GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1)
        words.forEachIndexed { index, word -> System.arraycopy(word, 0, out, index * 32, 32) }
        return out
    }

    private fun abiWord(value: Long): ByteArray {
        val out = ByteArray(32)
        var working = value
        for (index in 31 downTo 0) {
            out[index] = (working and 0xffL).toByte()
            working = working ushr 8
            if (working == 0L) break
        }
        return out
    }

    private fun repeatedWord(value: Int): ByteArray =
        ByteArray(32) { value.toByte() }

    private fun hexWord(hex: String): ByteArray {
        require(hex.length == 64)
        val out = ByteArray(32)
        for (index in out.indices) {
            out[index] = hex.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
        return out
    }

    private fun hexLower(bytes: ByteArray): String {
        val builder = StringBuilder(bytes.size * 2)
        for (byte in bytes) builder.append(String.format("%02x", byte.toInt() and 0xff))
        return builder.toString()
    }

    private fun sampleProofRequestInput(
        publicInputs: TronSccpPublicInputsInput = samplePublicInputs(),
        bundleBytes: ByteArray = byteArrayOf(5, 6, 7),
        sourceProofBytes: ByteArray = ByteArray(0),
        sourceDomain: Int = SccpTron.DOMAIN_SORA,
        statementHash: String = "56".repeat(32),
        destinationBindingHash: String = "78".repeat(32),
        backend: String = SccpTron.GROTH16_BN254_PROOF_BACKEND_V1,
    ): TronSccpProofRequestInput =
        TronSccpProofRequestInput(
            publicInputs = publicInputs,
            bundleBytes = bundleBytes,
            sourceProofBytes = sourceProofBytes,
            statementHash = statementHash,
            destinationBindingHash = destinationBindingHash,
            backend = backend,
            sourceDomain = sourceDomain,
        )

    private fun sampleProductionProofRequestInput(
        publicInputs: TronSccpPublicInputsInput = samplePublicInputs(),
        bundleBytes: ByteArray = byteArrayOf(5, 6, 7),
        sourceProofBytes: ByteArray = ByteArray(0),
        sourceDomain: Int = SccpTron.DOMAIN_SORA,
        statementHash: String = "56".repeat(32),
        backend: String = SccpTron.GROTH16_BN254_PROOF_BACKEND_V1,
    ): TronSccpProofRequestInput =
        TronSccpProofRequestInput(
            publicInputs = publicInputs,
            bundleBytes = bundleBytes,
            sourceProofBytes = sourceProofBytes,
            statementHash = statementHash,
            destinationBinding = sampleDestinationBinding(publicInputs),
            backend = backend,
            sourceDomain = sourceDomain,
        )

    private fun sampleDestinationBinding(
        publicInputs: TronSccpPublicInputsInput = samplePublicInputs(),
    ): SccpSourceProofs.TronDestinationBinding =
        SccpSourceProofs.tronDestinationBinding(
            targetDomain = publicInputs.targetDomain,
            networkId = "0x" + "33".repeat(32),
            verifierAddress = "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            verifierCodeHash = "0x" + "bb".repeat(32),
            verifierKeyHash = "0x" + "cc".repeat(32),
        )

    private fun sampleTronRouteCanaryEvidence(): TronSccpRouteCanaryEvidenceInput {
        val destinationBinding = sampleDestinationBinding()
        return TronSccpRouteCanaryEvidenceInput(
            routeAllowlistHash = "0xfea8effb3cddfa458ea79a5a9af6f2d2c33a460b3a66d9305963908c2a3ea67a",
            destinationBindingHash = destinationBinding.hash,
            sourceVerifierMaterialHash = "0x68c20262e44676bd5f3c4ec428f063373147a1ca14c5885648a9c651b3bcd8d8",
            sourceAdapterEngineDeploymentHash = "0x94dbe28a2fb16e043b83639b6dea8ec62f53679599ef1dd220fd13c71c7bdcb8",
            networkId = destinationBinding.networkId,
            verifierAddress = destinationBinding.verifierAddress,
            verifierCodeHash = destinationBinding.verifierCodeHash,
            verifierKeyHash = destinationBinding.verifierKeyHash,
            transactionId = "0x" + "fa".repeat(32),
            transactionOwnerAddress = "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf",
            blockNumber = "234",
            blockTimestamp = "567000",
            logIndex = 0,
            messageId = "0x" + "dd".repeat(32),
            callDataSha256 = "0xf96dfb36d47a61e7e80df4f19e00b78c12f9a3f3c542e8dac06a7422e1d5f951",
            payloadHash = "0x" + "ab".repeat(32),
            commitmentRoot = "0x" + "ee".repeat(32),
            finalityHeight = "0x" + "00".repeat(31) + "7b",
            finalityBlockHash = "0x" + "cd".repeat(32),
            statementHash = "0x" + "f1".repeat(32),
            usedMessageProof = true,
            rawDataOwnerMatchesTransaction = true,
            signatureSha256 = "0x" + "c4".repeat(32),
            signatureRecoveredAddress = "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf",
            signatureRecoversToOwner = true,
            routeCanaryEvidenceHash = "0xe0a96ff7e8f523599fd60fffe8bb3b9fda9519126b7ba00c89c922b323b64e56",
        )
    }

    private fun samplePublicInputs(
        payloadHash: String = "22".repeat(32),
        targetDomain: Int = SccpTron.DOMAIN_TRON,
        finalityHeight: String = "19",
    ): TronSccpPublicInputsInput =
        TronSccpPublicInputsInput(
            messageId = "11".repeat(32),
            payloadHash = payloadHash,
            targetDomain = targetDomain,
            commitmentRoot = "33".repeat(32),
            finalityHeight = finalityHeight,
            finalityBlockHash = "44".repeat(32),
        )
}
