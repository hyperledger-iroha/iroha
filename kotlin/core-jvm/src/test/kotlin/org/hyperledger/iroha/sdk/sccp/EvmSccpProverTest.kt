package org.hyperledger.iroha.sdk.sccp

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class EvmSccpProverTest {
    @Test
    fun proofRequestBindsPublicSignalsAndRelayContext() {
        val bundleBytes = byteArrayOf(5, 6, 7)
        val sourceProofBytes = byteArrayOf(9, 10)
        val request = SccpEvm.buildProofRequest(
            sampleProofRequestInput(bundleBytes = bundleBytes, sourceProofBytes = sourceProofBytes),
        )

        assertEquals(SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1, request.backend)
        assertEquals(SccpSolana.DOMAIN_SORA, request.sourceDomain)
        assertEquals(SccpEvm.DOMAIN_ETH, request.targetDomain)
        assertEquals(
            SccpEvm.groth16Bn254PublicSignalWords(
                publicInputs = samplePublicInputs(),
                sourceDomain = SccpSolana.DOMAIN_SORA,
                statementHash = "56".repeat(32),
                destinationBindingHash = "78".repeat(32),
            ),
            request.publicSignalWords,
        )
        assertEquals(
            "0x2eb6b5dbab56255a979f433862429637ba1e8251106271606f0a279f593d7a39",
            request.publicSignalWords[2],
        )
        assertEquals("0x" + "56".repeat(32), request.statementHash)
        assertEquals("0x" + "78".repeat(32), request.destinationBindingHash)
        assertEquals(
            "0xfb990c2ffdf826c9beb0e74105b060af467570720a1382b48abc42d32850f5ea",
            request.requestHash,
        )

        val destinationBinding = sampleDestinationBinding()
        val boundRequest = SccpEvm.buildProofRequest(
            EvmSccpProofRequestInput(
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

        val bscRequest = SccpEvm.buildProofRequest(
            sampleProofRequestInput(
                publicInputs = samplePublicInputs(targetDomain = SccpEvm.DOMAIN_BSC),
                sourceProofBytes = byteArrayOf(9, 10),
            ),
        )
        assertEquals(SccpEvm.DOMAIN_BSC, bscRequest.targetDomain)
        assertTrue(request.publicSignalWords[2] != bscRequest.publicSignalWords[2])
        assertTrue(request.requestHash != bscRequest.requestHash)
        val shiftedSplitRequest = SccpEvm.buildProofRequest(
            sampleProofRequestInput(
                bundleBytes = byteArrayOf(5, 6, 7, 9),
                sourceProofBytes = byteArrayOf(10),
            ),
        )
        assertTrue(request.requestHash != shiftedSplitRequest.requestHash)

        val error = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(sampleProofRequestInput(statementHash = ""))
        }
        assertTrue(error.message?.contains("statementHash") == true)

        val zeroFinality = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(
                sampleProofRequestInput(publicInputs = samplePublicInputs(finalityHeight = "0")),
            )
        }
        assertTrue(zeroFinality.message?.contains("finalityHeight") == true)

        val paddedPayload = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(
                sampleProofRequestInput(publicInputs = samplePublicInputs().copy(payloadHash = " " + "22".repeat(32))),
            )
        }
        assertTrue(paddedPayload.message?.contains("payloadHash") == true)
        assertTrue(paddedPayload.message?.contains("canonical hex") == true)

        val paddedStatement = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(sampleProofRequestInput(statementHash = "56".repeat(32) + " "))
        }
        assertTrue(paddedStatement.message?.contains("statementHash") == true)
        assertTrue(paddedStatement.message?.contains("canonical hex") == true)

        for (finalityHeight in listOf("019", "0x13", "+19", " 19", "19 ")) {
            val invalidFinalityHeight = assertFailsWith<IllegalArgumentException> {
                SccpEvm.buildProofRequest(
                    sampleProofRequestInput(
                        publicInputs = samplePublicInputs(finalityHeight = finalityHeight),
                    ),
                )
            }
            assertTrue(invalidFinalityHeight.message?.contains("finalityHeight") == true)
        }

        val sameDomain = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(
                sampleProofRequestInput(sourceDomain = SccpEvm.DOMAIN_ETH),
            )
        }
        assertTrue(sameDomain.message?.contains("sourceDomain must be SORA") == true)

        val wrongSource = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(
                sampleProofRequestInput(sourceDomain = SccpTon.DOMAIN_TON),
            )
        }
        assertTrue(wrongSource.message?.contains("sourceDomain must be SORA") == true)

        val wrongTarget = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(
                sampleProofRequestInput(publicInputs = samplePublicInputs(targetDomain = SccpTon.DOMAIN_TON)),
            )
        }
        assertTrue(wrongTarget.message?.contains("publicInputs.targetDomain must be ETH or BSC") == true)

        val zeroDestination = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(
                sampleProofRequestInput(destinationBindingHash = "00".repeat(32)),
            )
        }
        assertTrue(zeroDestination.message?.contains("destinationBindingHash") == true)

        val emptyBundle = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(sampleProofRequestInput(bundleBytes = ByteArray(0)))
        }
        assertTrue(emptyBundle.message?.contains("bundleBytes") == true)

        val zeroSourceProof = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(sampleProofRequestInput(sourceProofBytes = byteArrayOf(0, 0)))
        }
        assertTrue(zeroSourceProof.message?.contains("sourceProofBytes must not be all zero") == true)
        val oversizedSourceProof = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(
                sampleProofRequestInput(
                    sourceProofBytes = ByteArray(SccpEvm.SOURCE_STATE_MAX_PROOF_BYTES + 1) { 1 },
                ),
            )
        }
        assertTrue(oversizedSourceProof.message?.contains("sourceProofBytes must be at most") == true)
        assertContentEquals(
            ByteArray(0),
            SccpEvm.buildProofRequest(sampleProofRequestInput(sourceProofBytes = ByteArray(0))).sourceProofBytes,
        )

        val wrongBackend = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(sampleProofRequestInput(backend = "debug-evm-backend"))
        }
        assertTrue(wrongBackend.message?.contains("evm-groth16-bn254-v1") == true)

        val wrongBindingSource = assertFailsWith<IllegalArgumentException> {
            EvmSccpProofRequestInput(
                publicInputs = samplePublicInputs(),
                bundleBytes = byteArrayOf(5, 6, 7),
                statementHash = "56".repeat(32),
                destinationBinding = destinationBinding.copy(sourceDomain = SccpEvm.DOMAIN_ETH),
            )
        }
        assertTrue(wrongBindingSource.message?.contains("destinationBinding.sourceDomain") == true)

        val forgedBindingHash = assertFailsWith<IllegalArgumentException> {
            EvmSccpProofRequestInput(
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

        val callbackSnapshot = SccpEvm.callbackRequestSnapshot(request)
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
            EvmSccpProver().prove(sampleProofRequestInput())
        }
        assertTrue(error.message?.contains("not linked") == true)
    }

    @Test
    fun proverWrapsExternalProofBytes() {
        val proofBytes = sampleGroth16ProofBytes()
        val seenRequests = mutableListOf<EvmSccpProofRequest>()
        val prover = EvmSccpProver(
            proofEngine = EvmSccpProofEngine { request ->
                seenRequests.add(request)
                assertEquals(SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1, request.backend)
                assertEquals(SccpEvm.DOMAIN_ETH, request.targetDomain)
                assertEquals(9, request.publicSignalWords.size)
                proofBytes
            },
        )

        val result = prover.prove(sampleProductionProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)))
        val omittedSourceResult = prover.prove(sampleProductionProofRequestInput())
        val expectedRequest = SccpEvm.buildProofRequest(
            sampleProductionProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)),
        )
        val expectedOmittedSourceRequest = SccpEvm.buildProofRequest(sampleProductionProofRequestInput())

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
            SccpEvm.wrapProofResult(byteArrayOf(0, 0), request)
        }
        assertTrue(zeroProof.message?.contains("all zero") == true)

        val shortProof = assertFailsWith<IllegalArgumentException> {
            SccpEvm.wrapProofResult(byteArrayOf(1, 2, 3, 4), request)
        }
        assertTrue(shortProof.message?.contains("384 bytes") == true)

        val wrongBackend = assertFailsWith<IllegalArgumentException> {
            SccpEvm.wrapProofResult(byteArrayOf(1), request.copy(backend = "debug-evm-backend"))
        }
        assertTrue(wrongBackend.message?.contains("evm-groth16-bn254-v1") == true)

        val wrongRequestHash = assertFailsWith<IllegalArgumentException> {
            SccpEvm.wrapProofResult(proofBytes, request.copy(requestHash = "0x" + "99".repeat(32)))
        }
        assertTrue(wrongRequestHash.message?.contains("canonical") == true)

        val hashOnlyRequest = SccpEvm.buildProofRequest(
            sampleProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)),
        )
        val missingBinding = assertFailsWith<IllegalArgumentException> {
            SccpEvm.wrapProofResult(proofBytes, hashOnlyRequest)
        }
        assertTrue(missingBinding.message?.contains("destinationBinding") == true)

        val exposedProof = result.proofBytes
        exposedProof[0] = 9
        assertContentEquals(proofBytes, result.proofBytes)

        val mutatedRequestView =
            SccpEvm.buildProofRequest(sampleProductionProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)))
        mutatedRequestView.bundleBytes[0] = 9
        SccpEvm.wrapProofResult(proofBytes, mutatedRequestView)
        assertContentEquals(byteArrayOf(5, 6, 7), mutatedRequestView.bundleBytes)
    }

    @Test
    fun proverResolvesWitnessProviderBeforeBuildingRequest() {
        var resolved = false
        val proofBytes = sampleGroth16ProofBytes()
        val bundleBytes = byteArrayOf(5, 6, 7)
        val input = sampleProductionProofRequestInput(bundleBytes = bundleBytes)
        val prover = EvmSccpProver(
            witnessProvider = EvmSccpWitnessProvider { input ->
                assertContentEquals(ByteArray(0), input.sourceProofBytes)
                assertFalse(input.bundleBytes === bundleBytes)
                input.bundleBytes[0] = 0x7f
                resolved = true
                input.copy(sourceProofBytes = byteArrayOf(9, 10))
            },
            proofEngine = EvmSccpProofEngine { request ->
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
    fun rejectsMalformedGroth16ProofTuple() {
        val request = SccpEvm.buildProofRequest(sampleProductionProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)))

        val wrongVersion = assertFailsWith<IllegalArgumentException> {
            SccpEvm.wrapProofResult(sampleGroth16ProofBytes(mapOf(0 to abiWord(2))), request)
        }
        assertTrue(wrongVersion.message?.contains("proofBytes.version") == true)

        val outOfRangeCoordinate = assertFailsWith<IllegalArgumentException> {
            SccpEvm.wrapProofResult(sampleGroth16ProofBytes(mapOf(4 to ByteArray(32) { 0xff.toByte() })), request)
        }
        assertTrue(outOfRangeCoordinate.message?.contains("BN254 base-field") == true)

        val zeroB = assertFailsWith<IllegalArgumentException> {
            SccpEvm.wrapProofResult(
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

        val offCurveC = assertFailsWith<IllegalArgumentException> {
            SccpEvm.wrapProofResult(sampleGroth16ProofBytes(mapOf(11 to abiWord(3))), request)
        }
        assertTrue(offCurveC.message?.contains("proofBytes.c") == true)

        val nonSubgroupB = assertFailsWith<IllegalArgumentException> {
            SccpEvm.wrapProofResult(
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
            SccpEvm.wrapProofResult(sampleGroth16ProofBytes(mapOf(1 to repeatedWord(0x22))), request)
        }
        assertTrue(mismatchedMessageId.message?.contains("messageId must match") == true)

        val mismatchedSourceDomain = assertFailsWith<IllegalArgumentException> {
            SccpEvm.wrapProofResult(sampleGroth16ProofBytes(mapOf(2 to abiWord(999))), request)
        }
        assertTrue(mismatchedSourceDomain.message?.contains("sourceDomain must match") == true)
        val directMismatchedSourceDomain = assertFailsWith<IllegalArgumentException> {
            SccpEvm.submitMessageProofCallData(
                sampleGroth16ProofBytes(mapOf(2 to abiWord(SccpEvm.DOMAIN_ETH.toLong()))),
                samplePublicInputs(),
                "0x" + "56".repeat(32),
            )
        }
        assertTrue(directMismatchedSourceDomain.message?.contains("sourceDomain must match") == true)
        val directWrongSourceDomain = assertFailsWith<IllegalArgumentException> {
            SccpEvm.submitMessageProofCallData(
                sampleGroth16ProofBytes(mapOf(2 to abiWord(SccpEvm.DOMAIN_ETH.toLong()))),
                samplePublicInputs(),
                "0x" + "56".repeat(32),
                SccpEvm.DOMAIN_ETH,
            )
        }
        assertTrue(directWrongSourceDomain.message?.contains("sourceDomain must be SORA") == true)

        val mismatchedCommitmentRoot = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildSubmission(
                EvmSccpSubmissionInput(
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
        val request = SccpEvm.buildProofRequest(sampleProductionProofRequestInput(sourceProofBytes = byteArrayOf(9, 10)))
        val proofResult = SccpEvm.wrapProofResult(proofBytes, request)
        val submission = SccpEvm.buildSubmission(EvmSccpSubmissionInput(proofResult))

        assertEquals("contract_call", submission.submissionKind)
        assertEquals("evm_groth16_contract_call", submission.platformPayload)
        assertEquals(SccpEvm.CONTRACT_CALL_ABI_TUPLE_V1, submission.envelopeEncoding)
        assertEquals(SccpEvm.SUBMIT_MESSAGE_PROOF_SELECTOR_V1, submission.functionSelector)
        assertTrue(submission.callDataHex.startsWith(SccpEvm.SUBMIT_MESSAGE_PROOF_SELECTOR_V1))
        assertEquals(676, submission.callData.size)
        assertEquals("0x" + "00".repeat(30) + "0100", "0x" + hexLower(submission.callData.copyOfRange(4, 36)))
        assertEquals("0x" + "00".repeat(30) + "0180", "0x" + hexLower(submission.callData.copyOfRange(260, 292)))
        assertEquals(SccpEvm.messageTransparentPublicInputAbiWords(samplePublicInputs()), submission.publicInputWords)
        assertEquals(proofResult.publicSignalWords, submission.publicSignalWords)
        assertContentEquals(byteArrayOf(5, 6, 7), proofResult.bundleBytes)
        assertContentEquals(byteArrayOf(9, 10), proofResult.sourceProofBytes)
        assertContentEquals(proofBytes, submission.proofBytes)
        assertContentEquals(submission.callData, submission.envelopeBytes)
        assertContentEquals(
            submission.callData,
            SccpEvm.submitMessageProofCallData(
                proofBytes,
                proofResult.publicInputs,
                proofResult.statementHash,
            ),
        )
        val destinationBinding = sampleDestinationBinding()
        val bindingSubmission = SccpEvm.buildSubmission(
            EvmSccpSubmissionInput(
                publicInputs = proofResult.publicInputs,
                proofBytes = proofBytes,
                statementHash = proofResult.statementHash,
                destinationBinding = destinationBinding,
            ),
        )
        assertEquals(destinationBinding.hash, bindingSubmission.destinationBindingHash)

        val omittedSourceProofResult = SccpEvm.wrapProofResult(
            proofBytes,
            SccpEvm.buildProofRequest(sampleProductionProofRequestInput()),
        )
        val omittedSourceSubmission =
            SccpEvm.buildSubmission(EvmSccpSubmissionInput(omittedSourceProofResult))
        assertContentEquals(ByteArray(0), omittedSourceProofResult.sourceProofBytes)
        assertContentEquals(proofBytes, omittedSourceSubmission.proofBytes)

        val exposedCallData = submission.callData
        exposedCallData[0] = 0
        assertTrue(submission.callData[0].toInt() != 0)

        val proofMismatch = proofBytes.copyOf()
        proofMismatch[4 * 32 + 31] = 9
        val proofMismatchError = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildSubmission(
                EvmSccpSubmissionInput(
                    publicInputs = proofResult.publicInputs,
                    proofBytes = proofMismatch,
                    statementHash = proofResult.statementHash,
                    destinationBindingHash = proofResult.destinationBindingHash,
                    proofResult = proofResult,
                ),
            )
        }
        assertTrue(proofMismatchError.message?.contains("proofBytes") == true)

        val wrongBindingTarget = assertFailsWith<IllegalArgumentException> {
            EvmSccpSubmissionInput(
                publicInputs = proofResult.publicInputs,
                proofBytes = proofBytes,
                statementHash = proofResult.statementHash,
                destinationBinding = destinationBinding.copy(targetDomain = SccpEvm.DOMAIN_BSC),
            )
        }
        assertTrue(wrongBindingTarget.message?.contains("destinationBinding.targetDomain") == true)

        val tamperedEnvelopeError = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildSubmission(
                EvmSccpSubmissionInput(
                    proofResult = proofResult.copy(envelopeHash = "0x" + "aa".repeat(32)),
                ),
            )
        }
        assertTrue(tamperedEnvelopeError.message?.contains("wrapped proof bytes") == true)

        val tamperedBase64Error = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildSubmission(
                EvmSccpSubmissionInput(
                    proofResult = proofResult.copy(proofBase64 = "AAAA"),
                ),
            )
        }
        assertTrue(tamperedBase64Error.message?.contains("proofBase64") == true)

        val staleRequestError = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildSubmission(
                EvmSccpSubmissionInput(
                    proofResult = proofResult.copy(bundleBytes = byteArrayOf(5, 6, 8)),
                ),
            )
        }
        assertTrue(staleRequestError.message?.contains("requestHash") == true)

        val signalMismatchError = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildSubmission(
                EvmSccpSubmissionInput(
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
            SccpEvm.buildSubmission(
                EvmSccpSubmissionInput(
                    publicInputs = samplePublicInputs(targetDomain = SccpTon.DOMAIN_TON),
                    proofBytes = proofBytes,
                    statementHash = proofResult.statementHash,
                    destinationBindingHash = proofResult.destinationBindingHash,
                ),
            )
        }
        assertTrue(wrongTargetError.message?.contains("ETH or BSC") == true)
    }

    @Test
    fun bscMainnetFacadeRequiresChainId56AndBscTarget() {
        val proofBytes = sampleGroth16ProofBytes()
        val binding = SccpBsc.destinationBinding(
            verifierAddress = "0x" + "11".repeat(20),
            bridgeAddress = "0x" + "22".repeat(20),
            verifierCodeHash = "0x" + "bb".repeat(32),
            verifierKeyHash = "0x" + "cc".repeat(32),
        )
        assertEquals(SccpSourceProofs.BSC_MAINNET_NETWORK_ID, binding.networkId)
        assertEquals(SccpEvm.DOMAIN_BSC, binding.targetDomain)
        assertEquals(binding.hash, SccpBsc.destinationBindingHash(
            verifierAddress = "0x" + "11".repeat(20),
            bridgeAddress = "0x" + "22".repeat(20),
            verifierCodeHash = "0x" + "bb".repeat(32),
            verifierKeyHash = "0x" + "cc".repeat(32),
        ))

        val request = SccpBsc.buildProofRequest(
            EvmSccpProofRequestInput(
                publicInputs = samplePublicInputs(targetDomain = SccpEvm.DOMAIN_BSC),
                bundleBytes = byteArrayOf(5, 6, 7),
                sourceProofBytes = byteArrayOf(9, 10),
                statementHash = "56".repeat(32),
                destinationBinding = binding,
            ),
        )
        assertEquals(SccpEvm.DOMAIN_BSC, request.targetDomain)
        assertEquals(binding.hash, request.destinationBindingHash)

        val result = SccpBsc.wrapProofResult(proofBytes, request)
        val submission = SccpBsc.buildSubmission(EvmSccpSubmissionInput(result))
        assertEquals(SccpEvm.DOMAIN_BSC, submission.targetDomain)
        assertEquals("evm_groth16_contract_call", submission.platformPayload)
        assertContentEquals(proofBytes, submission.proofBytes)
        val submitted = BscMainnetSccp(
            outboundSubmitter = BscMainnetOutboundSubmitter { outboundSubmission ->
                assertEquals(SccpEvm.DOMAIN_BSC, outboundSubmission.targetDomain)
                assertContentEquals(proofBytes, outboundSubmission.proofBytes)
                assertEquals(binding.hash, outboundSubmission.destinationBindingHash)
                "bsc-submitted"
            },
        ).submitOutboundToBsc(EvmSccpSubmissionInput(result))
        assertEquals("bsc-submitted", submitted)
        assertFailsWith<IllegalStateException> {
            BscMainnetSccp().submitOutboundToBsc(EvmSccpSubmissionInput(result))
        }

        val wrongChainBinding = SccpSourceProofs.evmDestinationBinding(
            targetDomain = SccpEvm.DOMAIN_BSC,
            networkId = "0x" + "33".repeat(32),
            verifierAddress = "0x" + "11".repeat(20),
            bridgeAddress = "0x" + "22".repeat(20),
            verifierCodeHash = "0x" + "bb".repeat(32),
            verifierKeyHash = "0x" + "cc".repeat(32),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpBsc.destinationBinding(
                verifierAddress = "0x" + "11".repeat(20),
                bridgeAddress = "0x" + "22".repeat(20),
                verifierCodeHash = "0x" + "bb".repeat(32),
                verifierKeyHash = "0x" + "cc".repeat(32),
                networkId = "0x" + "33".repeat(32),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBsc.buildProofRequest(
                EvmSccpProofRequestInput(
                    publicInputs = samplePublicInputs(targetDomain = SccpEvm.DOMAIN_BSC),
                    bundleBytes = byteArrayOf(5, 6, 7),
                    statementHash = "56".repeat(32),
                    destinationBinding = wrongChainBinding,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBsc.buildProofRequest(sampleProductionProofRequestInput())
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBsc.buildSubmission(
                EvmSccpSubmissionInput(
                    publicInputs = samplePublicInputs(),
                    proofBytes = proofBytes,
                    statementHash = "56".repeat(32),
                    destinationBindingHash = binding.hash,
                ),
            )
        }
    }

    @Test
    fun bscMainnetFacadeBuildsLocalAdmissionSubmission() {
        val input = BscMainnetLocalAdmissionSubmissionInput(
            proofBytes = byteArrayOf(1, 2, 3),
            publicInputsBytes = byteArrayOf(4, 5, 6),
            bundleBytes = byteArrayOf(7, 8, 9),
            envelopeBytes = byteArrayOf(10, 11, 12),
            statementHash = "0x" + "66".repeat(32),
            sourceVerifierMaterialHash = "0x" + "77".repeat(32),
            sourceAdapterEngineDeploymentHash = "0x" + "88".repeat(32),
        )
        val submission = SccpBsc.buildLocalAdmissionSubmission(input)
        val facadeSubmission = BscMainnetSccp().buildLocalAdmissionSubmission(input)

        assertEquals(SccpBsc.LOCAL_ADMISSION_SUBMISSION_KIND_V1, submission.platformPayload)
        assertEquals(SccpBsc.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1, submission.envelopeEncoding)
        assertEquals(SccpBsc.LOCAL_ADMISSION_ENTRYPOINT_V1, submission.verifierEntrypoint)
        assertEquals(SccpEvm.DOMAIN_BSC, submission.sourceDomain)
        assertEquals(SccpEvm.DOMAIN_SORA, submission.targetDomain)
        assertEquals(emptyList<EvmSccpSubmissionArgument>(), submission.arguments)
        assertContentEquals(byteArrayOf(1, 2, 3), submission.proofBytes)
        assertContentEquals(byteArrayOf(4, 5, 6), submission.publicInputsBytes)
        assertContentEquals(byteArrayOf(7, 8, 9), submission.bundleBytes)
        assertContentEquals(byteArrayOf(10, 11, 12), submission.envelopeBytes)
        assertContentEquals(byteArrayOf(1, 2, 3), submission.localAdmission.proofBytes)
        assertEquals(submission.envelopeHex, facadeSubmission.envelopeHex)

        input.proofBytes[0] = 99
        assertContentEquals(byteArrayOf(1, 2, 3), submission.proofBytes)

        assertFailsWith<IllegalArgumentException> {
            SccpBsc.buildLocalAdmissionSubmission(
                input.copy(sourceDomain = SccpEvm.DOMAIN_ETH),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBsc.buildLocalAdmissionSubmission(input.copy(proofBytes = byteArrayOf(0, 0)))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBsc.buildLocalAdmissionSubmission(input.copy(envelopeBytes = byteArrayOf()))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBsc.buildLocalAdmissionSubmission(input.copy(envelopeEncoding = "abi_tuple_v1"))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBsc.buildLocalAdmissionSubmission(input.copy(proofFamily = "debug-proof-family"))
        }
    }

    @Test
    fun ethereumMainnetFacadeRequiresChainId1AndEthTarget() {
        val proofBytes = sampleGroth16ProofBytes()
        SccpEthereumMainnet.requireMainnetChainId(1L)
        assertEquals(
            "0x577b41c65ffbce226de59f224b464797257063747891b88ebec1bcd57af82727",
            SccpEthereumMainnet.sourceEventTopic(),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.requireMainnetChainId(56L)
        }
        val binding = SccpEthereumMainnet.destinationBinding(
            verifierAddress = "0x" + "11".repeat(20),
            bridgeAddress = "0x" + "22".repeat(20),
            verifierCodeHash = "0x" + "bb".repeat(32),
            verifierKeyHash = "0x" + "cc".repeat(32),
        )
        assertEquals(SccpSourceProofs.ETH_MAINNET_NETWORK_ID, binding.networkId)
        assertEquals(SccpEvm.DOMAIN_ETH, binding.targetDomain)
        assertEquals(binding.hash, SccpEthereumMainnet.destinationBindingHash(
            verifierAddress = "0x" + "11".repeat(20),
            bridgeAddress = "0x" + "22".repeat(20),
            verifierCodeHash = "0x" + "bb".repeat(32),
            verifierKeyHash = "0x" + "cc".repeat(32),
        ))

        val input = EvmSccpProofRequestInput(
            publicInputs = samplePublicInputs(targetDomain = SccpEvm.DOMAIN_ETH),
            bundleBytes = byteArrayOf(5, 6, 7),
            sourceProofBytes = byteArrayOf(9, 10),
            statementHash = "56".repeat(32),
            destinationBinding = binding,
        )
        val request = SccpEthereumMainnet.buildProofRequest(input)
        assertEquals(SccpEvm.DOMAIN_ETH, request.targetDomain)
        assertEquals(binding.hash, request.destinationBindingHash)
        assertEquals(request, EthereumMainnetSccp().buildOutboundProofRequest(input))

        val result = SccpEthereumMainnet.wrapProofResult(proofBytes, request)
        val submission = EthereumMainnetSccp().buildEthereumCalldata(EvmSccpSubmissionInput(result))
        assertEquals(SccpEvm.DOMAIN_ETH, submission.targetDomain)
        assertContentEquals(proofBytes, submission.proofBytes)
        val submitted = EthereumMainnetSccp(
            outboundSubmitter = EthereumMainnetOutboundSubmitter { outboundSubmission ->
                assertEquals(SccpEvm.DOMAIN_ETH, outboundSubmission.targetDomain)
                assertContentEquals(proofBytes, outboundSubmission.proofBytes)
                "eth-submitted"
            },
        ).submitOutboundToEthereum(EvmSccpSubmissionInput(result))
        assertEquals("eth-submitted", submitted)
        assertFailsWith<IllegalStateException> {
            EthereumMainnetSccp().submitOutboundToEthereum(EvmSccpSubmissionInput(result))
        }
        assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp().buildEthereumCalldata(
                EvmSccpSubmissionInput(
                    publicInputs = samplePublicInputs(targetDomain = SccpEvm.DOMAIN_ETH),
                    proofBytes = proofBytes,
                    statementHash = "56".repeat(32),
                    destinationBindingHash = binding.hash,
                ),
            )
        }

        assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.destinationBinding(
                verifierAddress = "0x" + "11".repeat(20),
                bridgeAddress = "0x" + "22".repeat(20),
                verifierCodeHash = "0x" + "bb".repeat(32),
                verifierKeyHash = "0x" + "cc".repeat(32),
                networkId = "0x" + "33".repeat(32),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.buildProofRequest(
                EvmSccpProofRequestInput(
                    publicInputs = samplePublicInputs(targetDomain = SccpEvm.DOMAIN_BSC),
                    bundleBytes = byteArrayOf(5, 6, 7),
                    statementHash = "56".repeat(32),
                    destinationBinding = binding,
                ),
            )
        }
    }

    @Test
    fun ethereumMainnetFacadeBuildsLocalAdmissionSubmission() {
        val input = EthereumMainnetLocalAdmissionSubmissionInput(
            proofBytes = byteArrayOf(1, 2, 3),
            publicInputsBytes = byteArrayOf(4, 5, 6),
            bundleBytes = byteArrayOf(7, 8, 9),
            envelopeBytes = byteArrayOf(10, 11, 12),
            statementHash = "0x" + "66".repeat(32),
            sourceVerifierMaterialHash = "0x" + "77".repeat(32),
            sourceAdapterEngineDeploymentHash = "0x" + "88".repeat(32),
        )
        val submission = SccpEthereumMainnet.buildLocalAdmissionSubmission(input)
        val facadeSubmission = EthereumMainnetSccp().buildLocalAdmissionSubmission(input)

        assertEquals(SccpEthereumMainnet.LOCAL_ADMISSION_SUBMISSION_KIND_V1, submission.platformPayload)
        assertEquals(SccpEthereumMainnet.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1, submission.envelopeEncoding)
        assertEquals(SccpEthereumMainnet.LOCAL_ADMISSION_ENTRYPOINT_V1, submission.verifierEntrypoint)
        assertEquals(SccpEvm.DOMAIN_ETH, submission.sourceDomain)
        assertEquals(SccpEvm.DOMAIN_SORA, submission.targetDomain)
        assertEquals(emptyList<EvmSccpSubmissionArgument>(), submission.arguments)
        assertContentEquals(byteArrayOf(1, 2, 3), submission.proofBytes)
        assertContentEquals(byteArrayOf(4, 5, 6), submission.publicInputsBytes)
        assertContentEquals(byteArrayOf(7, 8, 9), submission.bundleBytes)
        assertContentEquals(byteArrayOf(10, 11, 12), submission.envelopeBytes)
        assertContentEquals(byteArrayOf(1, 2, 3), submission.localAdmission.proofBytes)
        assertEquals(submission.envelopeHex, facadeSubmission.envelopeHex)

        input.proofBytes[0] = 99
        assertContentEquals(byteArrayOf(1, 2, 3), submission.proofBytes)

        assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.buildLocalAdmissionSubmission(
                input.copy(sourceDomain = SccpEvm.DOMAIN_BSC),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.buildLocalAdmissionSubmission(input.copy(proofBytes = byteArrayOf(0, 0)))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.buildLocalAdmissionSubmission(input.copy(publicInputsBytes = byteArrayOf(0, 0)))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.buildLocalAdmissionSubmission(input.copy(bundleBytes = byteArrayOf(0, 0)))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.buildLocalAdmissionSubmission(input.copy(envelopeBytes = byteArrayOf()))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.buildLocalAdmissionSubmission(input.copy(envelopeBytes = byteArrayOf(0, 0)))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.buildLocalAdmissionSubmission(input.copy(statementHash = "0x" + "00".repeat(32)))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.buildLocalAdmissionSubmission(
                input.copy(sourceVerifierMaterialHash = "0x" + "00".repeat(32)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.buildLocalAdmissionSubmission(
                input.copy(sourceAdapterEngineDeploymentHash = "0x" + "00".repeat(32)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.buildLocalAdmissionSubmission(input.copy(envelopeEncoding = "abi_tuple_v1"))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.buildLocalAdmissionSubmission(input.copy(proofFamily = "debug-proof-family"))
        }
    }

    @Test
    fun ethereumMainnetInboundEvidenceUsesMainnetRpcAndRejectsDrift() {
        val txHash = "0x" + "aa".repeat(32)
        val blockHash = "0x" + "bb".repeat(32)
        val sourceEventDigest = "0x" + "ee".repeat(32)
        val sourceBridgeEmitterAddress = "0x" + "12".repeat(20)
        val unrelatedLog = mapOf<String, Any?>(
            "address" to ("0x" + "00".repeat(20)),
            "topics" to listOf("0x" + "00".repeat(32)),
            "data" to "0x1234",
        )
        val sourceEventLog = mapOf<String, Any?>(
            "address" to sourceBridgeEmitterAddress,
            "topics" to listOf(SccpEthereumMainnet.sourceEventTopic(), sourceEventDigest),
            "data" to "0x",
        )
        val receipt = mapOf<String, Any?>(
            "transactionHash" to txHash,
            "blockHash" to blockHash,
            "blockNumber" to "0x1234",
            "status" to "0x1",
        )
        val receiptWithSourceEvent = receipt + ("logs" to listOf(unrelatedLog, sourceEventLog))
        val block = mapOf<String, Any?>(
            "hash" to blockHash,
            "number" to "0x1234",
            "receiptsRoot" to "0x" + "cc".repeat(32),
        )
        val beaconFinalityEvidence = EthereumMainnetBeaconFinalityEvidence(
            executionBlockNumber = "0x1234",
            executionBlockHash = blockHash,
            executionReceiptsRoot = "0x" + "cc".repeat(32),
            additionalFields = mapOf("finalizedHeaderRoot" to ("0x" + "dd".repeat(32))),
        )
        val beaconFinality = beaconFinalityEvidence.toMap()
        val receiptProof = EthereumMainnetReceiptProof(
            sourceEventDigest = "0x" + "ee".repeat(32),
            beaconSlot = "32",
            executionBlockNumber = "4660",
            executionBlockHash = blockHash,
            executionReceiptsRoot = "0x" + "cc".repeat(32),
            beaconFinalizedRoot = "0x" + "dd".repeat(32),
            syncCommitteeRoot = "0x" + "aa".repeat(32),
            receiptRootIndex = "3",
            receiptTrieProofNodes = listOf(byteArrayOf(0x01), byteArrayOf(0x02, 0x03)),
            inclusionBranch = listOf(ByteArray(32) { 0x11.toByte() }),
        )
        val receiptProofHash = SccpSourceProofs.evmReceiptProofHash(
            sourceEventDigest = receiptProof.sourceEventDigest,
            beaconSlot = receiptProof.beaconSlot,
            executionBlockNumber = receiptProof.executionBlockNumber,
            executionBlockHash = receiptProof.executionBlockHash,
            executionReceiptsRoot = receiptProof.executionReceiptsRoot,
            beaconFinalizedRoot = receiptProof.beaconFinalizedRoot,
            syncCommitteeRoot = receiptProof.syncCommitteeRoot,
            receiptRootIndex = receiptProof.receiptRootIndex,
            receiptTrieProofNodes = receiptProof.receiptTrieProofNodes,
            inclusionBranch = receiptProof.inclusionBranch,
        )
        assertEquals("0x39f014e3f5f8d38b44d59f1afdf72ceb71d10d6d937f268c404b046f092b38f0", receiptProofHash)
        val calls = mutableListOf<String>()
        var consensusCalls = 0
        val sdk = EthereumMainnetSccp(
            executionProvider = EthereumMainnetExecutionProvider { method, params ->
                calls.add(method)
                when (method) {
                    "eth_chainId" -> "0x1"
                    "eth_getTransactionReceipt" -> {
                        assertEquals(listOf(txHash), params)
                        receipt
                    }
                    "eth_getBlockByHash" -> {
                        assertEquals(listOf(blockHash, false), params)
                        block
                    }
                    else -> throw IllegalArgumentException("unexpected method $method")
                }
            },
            consensusProvider = EthereumMainnetConsensusProvider { collectedReceipt, collectedBlock, collectedTransactionHash ->
                consensusCalls += 1
                assertEquals(receipt["blockHash"], collectedBlock?.get("hash"))
                assertEquals(receipt["transactionHash"], collectedTransactionHash)
                assertEquals(receipt, collectedReceipt)
                beaconFinality
            },
            inboundProver = EthereumMainnetInboundProver { evidence ->
                assertEquals(SccpEvm.DOMAIN_ETH, evidence.sourceDomain)
                assertEquals(SccpEvm.DOMAIN_SORA, evidence.targetDomain)
                assertEquals(txHash, evidence.transactionHash)
                assertEquals("4660", evidence.beaconFinality?.get("executionBlockNumber"))
                assertEquals(blockHash, evidence.beaconFinality?.get("executionBlockHash"))
                assertEquals(receiptProofHash, evidence.receiptProofHash)
                assertEquals(receiptProof.sourceEventDigest, evidence.receiptProof?.sourceEventDigest)
                byteArrayOf(1, 2, 3)
            },
            inboundSubmitter = EthereumMainnetInboundSubmitter { proofBytes ->
                assertContentEquals(byteArrayOf(1, 2, 3), proofBytes)
                "submitted"
            },
        )

        val evidence = sdk.collectInboundEvidenceFromReceipt(
            EthereumMainnetInboundEvidence(transactionHash = txHash),
        )
        assertEquals(txHash, evidence.transactionHash)
        assertEquals(receipt, evidence.receipt)
        assertEquals(block, evidence.block)
        assertEquals("4660", evidence.beaconFinality?.get("executionBlockNumber"))
        assertEquals("0x" + "cc".repeat(32), evidence.beaconFinality?.get("executionReceiptsRoot"))
        assertEquals(1, consensusCalls)
        assertEquals(listOf("eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"), calls)
        assertContentEquals(
            byteArrayOf(1, 2, 3),
            sdk.proveInboundToSora(evidence.copy(receiptProof = receiptProof, receiptProofHash = receiptProofHash)),
        )
        assertEquals("submitted", sdk.submitInboundToIroha(byteArrayOf(1, 2, 3)))

        val sourceEventEvidence = sdk.collectInboundEvidenceFromReceipt(
            EthereumMainnetInboundEvidence(
                receipt = receiptWithSourceEvent,
                block = block,
                beaconFinality = beaconFinality,
                sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            ),
        )
        assertEquals(sourceEventDigest, sourceEventEvidence.sourceEventDigest)
        assertEquals(sourceBridgeEmitterAddress, sourceEventEvidence.sourceBridgeEmitterAddress)
        val explicitSourceEventEvidence = sdk.collectInboundEvidenceFromReceipt(
            EthereumMainnetInboundEvidence(
                receipt = receiptWithSourceEvent,
                block = block,
                beaconFinality = beaconFinality,
                sourceEventDigest = sourceEventDigest,
                sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            ),
        )
        assertEquals(sourceEventDigest, explicitSourceEventEvidence.sourceEventDigest)

        val receiptProofEvidence = EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
            EthereumMainnetInboundEvidence(
                receiptProof = receiptProof,
                receiptProofHash = receiptProofHash,
            ),
        )
        assertEquals(receiptProofHash, receiptProofEvidence.receiptProofHash)
        assertFalse(receiptProof === receiptProofEvidence.receiptProof)
        val mutableReceiptProofNode = receiptProof.receiptTrieProofNodes[0]
        val mutableReceiptProofBranch = receiptProof.inclusionBranch[0]
        mutableReceiptProofNode[0] = 0x7f
        mutableReceiptProofBranch[0] = 0x7f
        assertContentEquals(byteArrayOf(0x01), receiptProofEvidence.receiptProof?.receiptTrieProofNodes?.get(0))
        assertContentEquals(ByteArray(32) { 0x11.toByte() }, receiptProofEvidence.receiptProof?.inclusionBranch?.get(0))
        mutableReceiptProofNode[0] = 0x01
        mutableReceiptProofBranch[0] = 0x11
        assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receiptProof = receiptProof,
                    receiptProofHash = "0x" + "99".repeat(32),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receiptProof = receiptProof.copy(sourceDomain = SccpEvm.DOMAIN_BSC)),
            )
        }

        assertContentEquals(
            byteArrayOf(7, 8, 9),
            EthereumMainnetSccp(
                inboundProver = EthereumMainnetInboundProver { typedEvidence ->
                    assertEquals(txHash, typedEvidence.transactionHash)
                    assertEquals(blockHash, typedEvidence.beaconFinality?.get("executionBlockHash"))
                    byteArrayOf(7, 8, 9)
                },
            ).proveInboundToSora(
                EthereumMainnetInboundEvidence.withBeaconFinalityEvidence(
                    receipt = receipt,
                    block = block,
                    beaconFinalityEvidence = beaconFinalityEvidence,
                    receiptProof = receiptProof,
                    receiptProofHash = receiptProofHash,
                ),
            ),
        )

        val perCallProviderCalls = mutableListOf<String>()
        var perCallConsensusCalls = 0
        val perCallSdk = EthereumMainnetSccp(
            inboundProver = EthereumMainnetInboundProver { evidence ->
                assertEquals(txHash, evidence.transactionHash)
                assertEquals(blockHash, evidence.beaconFinality?.get("executionBlockHash"))
                byteArrayOf(4, 5, 6)
            },
        )
        assertContentEquals(
            byteArrayOf(4, 5, 6),
            perCallSdk.proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    transactionHash = txHash,
                    receiptProof = receiptProof,
                    receiptProofHash = receiptProofHash,
                ),
                provider = EthereumMainnetExecutionProvider { method, _ ->
                    perCallProviderCalls.add(method)
                    when (method) {
                        "eth_chainId" -> "0x1"
                        "eth_getTransactionReceipt" -> receipt
                        "eth_getBlockByHash" -> block
                        else -> throw IllegalArgumentException("unexpected method $method")
                    }
                },
                consensusProvider = EthereumMainnetConsensusProvider { _, _, _ ->
                    perCallConsensusCalls += 1
                    beaconFinality
                },
            ),
        )
        assertEquals(listOf("eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"), perCallProviderCalls)
        assertEquals(1, perCallConsensusCalls)

        var proverCalls = 0
        assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                inboundProver = EthereumMainnetInboundProver {
                    proverCalls += 1
                    byteArrayOf(1, 2, 3)
                },
            ).proveInboundToSora(EthereumMainnetInboundEvidence(receipt = receipt, block = block))
        }
        assertEquals(0, proverCalls)

        val missingReceiptProof = assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                inboundProver = EthereumMainnetInboundProver {
                    proverCalls += 1
                    byteArrayOf(1, 2, 3)
                },
            ).proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    receipt = receipt,
                    block = block,
                    beaconFinality = beaconFinality,
                    receiptProofHash = receiptProofHash,
                ),
            )
        }
        assertTrue(missingReceiptProof.message?.contains("receiptProof") == true)
        assertEquals(0, proverCalls)

        val driftedReceiptProof = assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                inboundProver = EthereumMainnetInboundProver {
                    proverCalls += 1
                    byteArrayOf(1, 2, 3)
                },
            ).proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    receipt = receipt,
                    block = block,
                    beaconFinality = beaconFinality,
                    receiptProof = receiptProof.copy(executionReceiptsRoot = "0x" + "99".repeat(32)),
                ),
            )
        }
        assertTrue(driftedReceiptProof.message?.contains("receiptProof.executionReceiptsRoot") == true)
        assertEquals(0, proverCalls)

        val missingProvider = assertFailsWith<IllegalStateException> {
            EthereumMainnetSccp()
                .collectInboundEvidenceFromReceipt(EthereumMainnetInboundEvidence(transactionHash = txHash))
        }
        assertTrue(missingProvider.message?.contains("execution provider") == true)

        assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                executionProvider = EthereumMainnetExecutionProvider { _, _ -> "0x38" },
            ).collectInboundEvidenceFromReceipt(EthereumMainnetInboundEvidence(receipt = receipt))
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receipt = receipt + ("status" to "0x0")),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receipt = receipt - "blockNumber", block = block),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receipt = receipt + ("blockNumber" to "0x0"), block = block),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    transactionHash = txHash,
                    receipt = receipt + ("transactionHash" to ("0x" + "ab".repeat(32))),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt,
                    block = block + ("hash" to ("0x" + "bc".repeat(32))),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt,
                    block = block - "number",
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt,
                    block = block + ("number" to "0x0"),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt + ("transactionHash" to txHash.uppercase()),
                    block = block,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt,
                    block = block,
                    beaconFinality = beaconFinality + ("executionBlockHash" to ("0x" + "bc".repeat(32))),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt,
                    block = block,
                    beaconFinality = beaconFinality + ("executionBlockNumber" to "0x1235"),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt,
                    block = block,
                    beaconFinality = beaconFinality + ("executionReceiptsRoot" to ("0x" + "cd".repeat(32))),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receiptWithSourceEvent,
                    block = block,
                    beaconFinality = beaconFinality,
                    sourceEventDigest = sourceEventDigest,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt,
                    block = block,
                    beaconFinality = beaconFinality,
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receiptWithSourceEvent,
                    block = block,
                    beaconFinality = beaconFinality,
                    sourceBridgeEmitterAddress = "0x" + "13".repeat(20),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt + (
                        "logs" to listOf(sourceEventLog + ("topics" to listOf("0x" + "ab".repeat(32), sourceEventDigest)))
                    ),
                    block = block,
                    beaconFinality = beaconFinality,
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt + ("logs" to listOf(sourceEventLog, sourceEventLog)),
                    block = block,
                    beaconFinality = beaconFinality,
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt + ("logs" to listOf(sourceEventLog + ("removed" to true))),
                    block = block,
                    beaconFinality = beaconFinality,
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.submitInboundToIroha(byteArrayOf(0, 0))
        }
    }

    @Test
    fun bscMainnetInboundEvidenceUsesMainnetRpcAndRejectsDrift() {
        val txHash = "0x" + "aa".repeat(32)
        val blockHash = "0x" + "bb".repeat(32)
        val receipt = mapOf<String, Any?>(
            "transactionHash" to txHash,
            "blockHash" to blockHash,
            "blockNumber" to "0x1234",
            "status" to "0x1",
        )
        val block = mapOf<String, Any?>(
            "hash" to blockHash,
            "number" to "0x1234",
            "receiptsRoot" to "0x" + "cc".repeat(32),
        )
        val parliaFinalityEvidence = BscMainnetParliaFinalityEvidence(
            executionBlockNumber = "0x1234",
            executionBlockHash = blockHash,
            executionReceiptsRoot = "0x" + "cc".repeat(32),
            additionalFields = mapOf(
                "validatorEpoch" to "0x24",
                "commitSealHash" to ("0x" + "dd".repeat(32)),
            ),
        )
        val parliaFinality = parliaFinalityEvidence.toMap()
        val calls = mutableListOf<String>()
        val sdk = BscMainnetSccp(
            executionProvider = BscMainnetExecutionProvider { method, params ->
                calls.add(method)
                when (method) {
                    "eth_chainId" -> "0x38"
                    "eth_getTransactionReceipt" -> {
                        assertEquals(listOf(txHash), params)
                        receipt
                    }
                    "eth_getBlockByHash" -> {
                        assertEquals(listOf(blockHash, false), params)
                        block
                    }
                    else -> throw IllegalArgumentException("unexpected method $method")
                }
            },
            inboundProver = BscMainnetInboundProver { evidence ->
                assertEquals(SccpEvm.DOMAIN_BSC, evidence.sourceDomain)
                assertEquals(SccpEvm.DOMAIN_SORA, evidence.targetDomain)
                assertEquals(txHash, evidence.transactionHash)
                assertEquals(blockHash, evidence.parliaFinality?.get("executionBlockHash"))
                byteArrayOf(1, 2, 3)
            },
            inboundSubmitter = BscMainnetInboundSubmitter { proofBytes ->
                assertContentEquals(byteArrayOf(1, 2, 3), proofBytes)
                "submitted"
            },
        )

        val evidence = sdk.collectInboundEvidenceFromReceipt(
            BscMainnetInboundEvidence.withParliaFinalityEvidence(
                transactionHash = txHash,
                parliaFinalityEvidence = parliaFinalityEvidence,
            ),
        )
        assertEquals(txHash, evidence.transactionHash)
        assertEquals(receipt, evidence.receipt)
        assertEquals(block, evidence.block)
        assertEquals("4660", evidence.parliaFinality?.get("executionBlockNumber"))
        assertEquals(blockHash, evidence.parliaFinality?.get("executionBlockHash"))
        assertEquals(listOf("eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"), calls)
        val providerFinality = BscMainnetSccp(
            executionProvider = BscMainnetExecutionProvider { method, params ->
                when (method) {
                    "eth_chainId" -> "0x38"
                    "eth_getTransactionReceipt" -> receipt
                    "eth_getBlockByHash" -> block
                    else -> throw IllegalArgumentException("unexpected method $method $params")
                }
            },
            consensusProvider = BscMainnetConsensusProvider { _, _, _ -> parliaFinality },
        ).collectInboundEvidenceFromReceipt(BscMainnetInboundEvidence(transactionHash = txHash))
        assertEquals(blockHash, providerFinality.parliaFinality?.get("executionBlockHash"))
        assertContentEquals(byteArrayOf(1, 2, 3), sdk.proveInboundToSora(evidence))
        assertEquals("submitted", sdk.submitInboundToIroha(byteArrayOf(1, 2, 3)))

        assertFailsWith<IllegalArgumentException> {
            BscMainnetSccp(
                executionProvider = BscMainnetExecutionProvider { _, _ -> "0x1" },
            ).collectInboundEvidenceFromReceipt(BscMainnetInboundEvidence(receipt = receipt))
        }
        assertFailsWith<IllegalArgumentException> {
            BscMainnetSccp().collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(sourceDomain = SccpEvm.DOMAIN_ETH, receipt = receipt),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(receipt = receipt + ("status" to "0x0")),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(receipt = receipt - "blockNumber", block = block),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(receipt = receipt + ("blockNumber" to "0x0"), block = block),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    transactionHash = txHash,
                    receipt = receipt + ("transactionHash" to ("0x" + "ab".repeat(32))),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    receipt = receipt,
                    block = block + ("hash" to ("0x" + "bc".repeat(32))),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    receipt = receipt,
                    block = block - "number",
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    receipt = receipt,
                    block = block + ("number" to "0x0"),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    receipt = receipt,
                    block = block + ("number" to "0x1235"),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    receipt = receipt + ("transactionHash" to txHash.uppercase()),
                    block = block,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            BscMainnetSccp(
                inboundProver = BscMainnetInboundProver {
                    throw AssertionError("prover callback must not run without Parlia finality")
                },
            ).proveInboundToSora(BscMainnetInboundEvidence(receiptProofHash = "0x" + "ee".repeat(32)))
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    receipt = receipt,
                    block = block,
                    parliaFinality = parliaFinality + ("executionBlockHash" to ("0x" + "bc".repeat(32))),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    receipt = receipt,
                    block = block,
                    parliaFinality = parliaFinality + ("executionBlockNumber" to "0x1235"),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    receipt = receipt,
                    block = block,
                    parliaFinality = parliaFinality + ("executionReceiptsRoot" to ("0x" + "cd".repeat(32))),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.submitInboundToIroha(byteArrayOf(0, 0))
        }
    }

    private fun sampleGroth16ProofBytes(overrides: Map<Int, ByteArray> = emptyMap()): ByteArray {
        val words = mutableListOf(
            abiWord(1),
            repeatedWord(0x11),
            abiWord(SccpSolana.DOMAIN_SORA.toLong()),
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
        val out = ByteArray(SccpEvm.GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1)
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
        publicInputs: EvmSccpPublicInputsInput = samplePublicInputs(),
        bundleBytes: ByteArray = byteArrayOf(5, 6, 7),
        sourceProofBytes: ByteArray = ByteArray(0),
        statementHash: String = "56".repeat(32),
        destinationBindingHash: String = "78".repeat(32),
        backend: String = SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1,
        sourceDomain: Int = SccpSolana.DOMAIN_SORA,
    ): EvmSccpProofRequestInput =
        EvmSccpProofRequestInput(
            publicInputs = publicInputs,
            bundleBytes = bundleBytes,
            sourceProofBytes = sourceProofBytes,
            statementHash = statementHash,
            destinationBindingHash = destinationBindingHash,
            backend = backend,
            sourceDomain = sourceDomain,
        )

    private fun sampleProductionProofRequestInput(
        publicInputs: EvmSccpPublicInputsInput = samplePublicInputs(),
        bundleBytes: ByteArray = byteArrayOf(5, 6, 7),
        sourceProofBytes: ByteArray = ByteArray(0),
        statementHash: String = "56".repeat(32),
        backend: String = SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1,
        sourceDomain: Int = SccpSolana.DOMAIN_SORA,
    ): EvmSccpProofRequestInput =
        EvmSccpProofRequestInput(
            publicInputs = publicInputs,
            bundleBytes = bundleBytes,
            sourceProofBytes = sourceProofBytes,
            statementHash = statementHash,
            destinationBinding = sampleDestinationBinding(publicInputs),
            backend = backend,
            sourceDomain = sourceDomain,
        )

    private fun sampleDestinationBinding(
        publicInputs: EvmSccpPublicInputsInput = samplePublicInputs(),
    ): SccpSourceProofs.EvmDestinationBinding =
        SccpSourceProofs.evmDestinationBinding(
            targetDomain = publicInputs.targetDomain,
            networkId = "0x" + "33".repeat(32),
            verifierAddress = "0x" + "11".repeat(20),
            bridgeAddress = "0x" + "22".repeat(20),
            verifierCodeHash = "0x" + "bb".repeat(32),
            verifierKeyHash = "0x" + "cc".repeat(32),
        )

    private fun samplePublicInputs(
        targetDomain: Int = SccpEvm.DOMAIN_ETH,
        finalityHeight: String = "19",
    ): EvmSccpPublicInputsInput =
        EvmSccpPublicInputsInput(
            messageId = "11".repeat(32),
            payloadHash = "22".repeat(32),
            targetDomain = targetDomain,
            commitmentRoot = "33".repeat(32),
            finalityHeight = finalityHeight,
            finalityBlockHash = "44".repeat(32),
        )
}
