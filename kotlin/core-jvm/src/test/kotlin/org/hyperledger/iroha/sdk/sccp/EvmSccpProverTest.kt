package org.hyperledger.iroha.sdk.sccp

import com.sun.net.httpserver.HttpServer
import java.io.ByteArrayOutputStream
import java.net.InetSocketAddress
import java.security.MessageDigest
import org.bouncycastle.crypto.digests.KeccakDigest
import org.hyperledger.iroha.sdk.crypto.Blake2b
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class EvmSccpProverTest {
    private val ethereumSyncCommitteeSupermajorityBits =
        "0x" + "ff".repeat(42) + "3f" + "00".repeat(21)
    private val ethereumSyncCommitteeSupermajorityParticipation = "342"
    private val ethereumFinalityBranch =
        (0 until 6).map { "0x" + (0x50 + it).toString(16).padStart(2, '0').repeat(32) }

    @Test
    fun proofRequestBindsPublicSignalsAndRelayContext() {
        val bundleBytes = sampleBundleBytes()
        val sourceProofBytes = ByteArray(0)
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
        assertEquals("0xe6082be4ad0d601651e761ee0df68e703f75a0bd9be8bcd5ec3c278ac354d4f1", request.requestHash)

        val destinationBinding = sampleDestinationBinding()
        val boundRequest = SccpEvm.buildProofRequest(
            EvmSccpProofRequestInput(
                publicInputs = samplePublicInputs(),
                bundleBytes = sampleBundleBytes(),
                sourceProofBytes = ByteArray(0),
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
                bundleBytes = sampleBundleBytes(targetDomain = SccpEvm.DOMAIN_BSC),
                sourceProofBytes = ByteArray(0),
            ),
        )
        assertEquals(SccpEvm.DOMAIN_BSC, bscRequest.targetDomain)
        assertTrue(request.publicSignalWords[2] != bscRequest.publicSignalWords[2])
        assertTrue(request.requestHash != bscRequest.requestHash)
        val shiftedSplitRequest = SccpEvm.buildProofRequest(
            sampleProofRequestInput(
                publicInputs = sampleBundleFixture(nonce = 328L).publicInputs,
                bundleBytes = sampleBundleFixture(nonce = 328L).bundleBytes,
                sourceProofBytes = ByteArray(0),
            ),
        )
        assertTrue(request.requestHash != shiftedSplitRequest.requestHash)
        val extraneousSoraSourceProof = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(
                sampleProofRequestInput(sourceProofBytes = byteArrayOf(9, 11)),
            )
        }
        assertTrue(
            extraneousSoraSourceProof.message?.contains("sourceProofBytes must be empty for SORA source bundle") == true,
        )
        val nonSoraBundle = sampleBundleFixture(sourceDomain = SccpEvm.DOMAIN_BSC)
        val mismatchedNonSoraProof = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(
                sampleProofRequestInput(
                    publicInputs = nonSoraBundle.publicInputs,
                    bundleBytes = nonSoraBundle.bundleBytes,
                    sourceProofBytes = byteArrayOf(9, 10),
                ),
            )
        }
        assertTrue(
            mismatchedNonSoraProof.message?.contains("sourceProofBytes must match bundleBytes finality proof") == true,
        )
        val nonSoraBundleSource = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(
                sampleProofRequestInput(
                    publicInputs = nonSoraBundle.publicInputs,
                    bundleBytes = nonSoraBundle.bundleBytes,
                    sourceProofBytes = byteArrayOf(0x01, 0x02, 0x03),
                ),
            )
        }
        assertTrue(
            nonSoraBundleSource.message?.contains("sourceProofBytes must decode as SccpSourceChainProofEnvelopeV1") == true,
        )
        val artifactRequest = SccpEvm.buildProofRequest(
            sampleProofRequestInput(
                sourceProofBytes = ByteArray(0),
                proofArtifactHash = "91".repeat(32),
                provingKeyHash = "92".repeat(32),
            ),
        )
        assertEquals("0x" + "91".repeat(32), artifactRequest.proofArtifactHash)
        assertEquals("0x" + "92".repeat(32), artifactRequest.provingKeyHash)
        assertTrue(request.requestHash != artifactRequest.requestHash)
        val missingProvingKey = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(sampleProofRequestInput(proofArtifactHash = "91".repeat(32)))
        }
        assertTrue(missingProvingKey.message?.contains("proofArtifactHash and provingKeyHash") == true)
        val zeroProofArtifact = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(
                sampleProofRequestInput(
                    proofArtifactHash = "00".repeat(32),
                    provingKeyHash = "92".repeat(32),
                ),
            )
        }
        assertTrue(zeroProofArtifact.message?.contains("proofArtifactHash") == true)

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

        val canonicalEip55Recipient = "0x52908400098527886E0F7030069857D2E4169EE7"
        val canonicalEip55Bundle = sampleBundleFixture(recipient = canonicalEip55Recipient)
        SccpEvm.buildProofRequest(
            sampleProofRequestInput(
                publicInputs = canonicalEip55Bundle.publicInputs,
                bundleBytes = canonicalEip55Bundle.bundleBytes,
            ),
        )
        val lowercaseRequiredEip55Recipient = "0xde709f2102306220921060314715629080e2fb77"
        val lowercaseRequiredEip55Bundle = sampleBundleFixture(recipient = lowercaseRequiredEip55Recipient)
        SccpEvm.buildProofRequest(
            sampleProofRequestInput(
                publicInputs = lowercaseRequiredEip55Bundle.publicInputs,
                bundleBytes = lowercaseRequiredEip55Bundle.bundleBytes,
            ),
        )
        val noncanonicalEip55Bundle = sampleBundleFixture(recipient = canonicalEip55Recipient.lowercase())
        val noncanonicalEip55 = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(
                sampleProofRequestInput(
                    publicInputs = noncanonicalEip55Bundle.publicInputs,
                    bundleBytes = noncanonicalEip55Bundle.bundleBytes,
                ),
            )
        }
        assertTrue(noncanonicalEip55.message?.contains("bundleBytes.payload.recipient") == true)
        assertTrue(noncanonicalEip55.message?.contains("EIP-55") == true)
        for (invalidRecipient in listOf(
            lowercaseRequiredEip55Recipient.uppercase(),
            "0X" + canonicalEip55Recipient.drop(2),
            "0x52908400098527886E0F7030069857D2E4169EEZ",
        )) {
            val invalidBundle = sampleBundleFixture(recipient = invalidRecipient)
            val invalidEip55 = assertFailsWith<IllegalArgumentException> {
                SccpEvm.buildProofRequest(
                    sampleProofRequestInput(
                        publicInputs = invalidBundle.publicInputs,
                        bundleBytes = invalidBundle.bundleBytes,
                    ),
                )
            }
            assertTrue(invalidEip55.message?.contains("bundleBytes.payload.recipient") == true)
        }

        val nulPrefixedNameBundle = sampleTokenAddBundleFixture(
            name = byteArrayOf(0) + "Token".toByteArray(Charsets.UTF_8) + ByteArray(26),
        )
        val nulPrefixedName = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(
                sampleProofRequestInput(
                    publicInputs = nulPrefixedNameBundle.publicInputs,
                    bundleBytes = nulPrefixedNameBundle.bundleBytes,
                ),
            )
        }
        assertTrue(nulPrefixedName.message?.contains("bundleBytes.payload.name") == true)
        val nulPrefixedSymbolBundle = sampleTokenAddBundleFixture(
            symbol = byteArrayOf(0) + "TOK".toByteArray(Charsets.UTF_8) + ByteArray(28),
        )
        val nulPrefixedSymbol = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildProofRequest(
                sampleProofRequestInput(
                    publicInputs = nulPrefixedSymbolBundle.publicInputs,
                    bundleBytes = nulPrefixedSymbolBundle.bundleBytes,
                ),
            )
        }
        assertTrue(nulPrefixedSymbol.message?.contains("bundleBytes.payload.symbol") == true)

        val wrongBindingSource = assertFailsWith<IllegalArgumentException> {
            EvmSccpProofRequestInput(
                publicInputs = samplePublicInputs(),
                bundleBytes = sampleBundleBytes(),
                statementHash = "56".repeat(32),
                destinationBinding = destinationBinding.copy(sourceDomain = SccpEvm.DOMAIN_ETH),
            )
        }
        assertTrue(wrongBindingSource.message?.contains("destinationBinding.sourceDomain") == true)

        val forgedBindingHash = assertFailsWith<IllegalArgumentException> {
            EvmSccpProofRequestInput(
                publicInputs = samplePublicInputs(),
                bundleBytes = sampleBundleBytes(),
                statementHash = "56".repeat(32),
                destinationBinding = destinationBinding.copy(hash = "0x" + "99".repeat(32)),
            )
        }
        assertTrue(forgedBindingHash.message?.contains("deployment material") == true)

        bundleBytes[0] = 99
        assertContentEquals(sampleBundleBytes(), request.bundleBytes)
        assertContentEquals(ByteArray(0), request.sourceProofBytes)

        val exposedPublicInputs = request.publicInputsBytes
        val exposedBundle = request.bundleBytes
        val exposedSourceProof = request.sourceProofBytes
        exposedPublicInputs[0] = 99
        exposedBundle[0] = 99
        assertTrue(request.publicInputsBytes[0].toInt() != 99)
        assertContentEquals(sampleBundleBytes(), request.bundleBytes)
        assertContentEquals(ByteArray(0), exposedSourceProof)
        assertContentEquals(ByteArray(0), request.sourceProofBytes)

        val callbackSnapshot = SccpEvm.callbackRequestSnapshot(request)
        assertFalse(callbackSnapshot === request)
        assertEquals(request, callbackSnapshot)
        val snapshotBundle = callbackSnapshot.bundleBytes
        val snapshotSourceProof = callbackSnapshot.sourceProofBytes
        snapshotBundle[0] = 77
        assertContentEquals(sampleBundleBytes(), callbackSnapshot.bundleBytes)
        assertContentEquals(ByteArray(0), snapshotSourceProof)
        assertContentEquals(ByteArray(0), callbackSnapshot.sourceProofBytes)
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

        val result = prover.prove(sampleProductionProofRequestInput())
        val omittedSourceResult = prover.prove(sampleProductionProofRequestInput())
        val expectedRequest = SccpEvm.buildProofRequest(
            sampleProductionProofRequestInput(),
        )
        val expectedOmittedSourceRequest = SccpEvm.buildProofRequest(sampleProductionProofRequestInput())

        assertEquals(listOf(expectedRequest, expectedOmittedSourceRequest), seenRequests)
        assertFalse(seenRequests[0] === expectedRequest)
        assertFalse(seenRequests[1] === expectedOmittedSourceRequest)

        assertContentEquals(proofBytes, result.proofBytes)
        assertContentEquals(ByteArray(0), result.sourceProofBytes)
        assertContentEquals(ByteArray(0), omittedSourceResult.sourceProofBytes)
        assertTrue(result.proofBase64.isNotEmpty())
        assertEquals("0x" + "56".repeat(32), result.statementHash)
        assertEquals(expectedRequest.destinationBindingHash, result.destinationBindingHash)
        assertEquals(expectedRequest.destinationBinding, result.destinationBinding)
        assertEquals(expectedRequest.requestHash, result.requestHash)
        assertTrue(result.envelopeHash.matches(Regex("0x[0-9a-f]{64}")))
        val artifactRequest = SccpEvm.buildProofRequest(
            sampleProductionProofRequestInput(
                proofArtifactHash = "91".repeat(32),
                provingKeyHash = "92".repeat(32),
            ),
        )
        val artifactResult = SccpEvm.wrapProofResult(proofBytes, artifactRequest)
        assertEquals(artifactRequest.proofArtifactHash, artifactResult.proofArtifactHash)
        assertEquals(artifactRequest.provingKeyHash, artifactResult.provingKeyHash)
        assertTrue(artifactRequest.requestHash != expectedRequest.requestHash)

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
            sampleProofRequestInput(),
        )
        val missingBinding = assertFailsWith<IllegalArgumentException> {
            SccpEvm.wrapProofResult(proofBytes, hashOnlyRequest)
        }
        assertTrue(missingBinding.message?.contains("destinationBinding") == true)

        val exposedProof = result.proofBytes
        exposedProof[0] = 9
        assertContentEquals(proofBytes, result.proofBytes)

        val mutatedRequestView =
            SccpEvm.buildProofRequest(sampleProductionProofRequestInput())
        mutatedRequestView.bundleBytes[0] = 9
        SccpEvm.wrapProofResult(proofBytes, mutatedRequestView)
        assertContentEquals(sampleBundleBytes(), mutatedRequestView.bundleBytes)
    }

    @Test
    fun proverResolvesWitnessProviderBeforeBuildingRequest() {
        var resolved = false
        val proofBytes = sampleGroth16ProofBytes()
        val bundleBytes = sampleBundleBytes()
        val input = sampleProductionProofRequestInput(bundleBytes = bundleBytes)
        val prover = EvmSccpProver(
            witnessProvider = EvmSccpWitnessProvider { input ->
                assertContentEquals(ByteArray(0), input.sourceProofBytes)
                assertFalse(input.bundleBytes === bundleBytes)
                input.bundleBytes[0] = 0x7f
                resolved = true
                sampleProductionProofRequestInput(
                    publicInputs = input.publicInputs,
                    statementHash = input.statementHash,
                )
            },
            proofEngine = EvmSccpProofEngine { request ->
                assertTrue(resolved)
                assertContentEquals(ByteArray(0), request.sourceProofBytes)
                proofBytes
            },
        )

        val result = prover.prove(input)

        assertContentEquals(ByteArray(0), result.sourceProofBytes)
        assertContentEquals(sampleBundleBytes(), input.bundleBytes)
        assertContentEquals(sampleBundleBytes(), bundleBytes)
    }

    @Test
    fun rejectsMalformedGroth16ProofTuple() {
        val request = SccpEvm.buildProofRequest(sampleProductionProofRequestInput())

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
        val request = SccpEvm.buildProofRequest(sampleProductionProofRequestInput())
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
        assertContentEquals(sampleBundleBytes(), proofResult.bundleBytes)
        assertContentEquals(ByteArray(0), proofResult.sourceProofBytes)
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

        val extraneousSourceProofError = assertFailsWith<IllegalArgumentException> {
            SccpEvm.buildSubmission(
                EvmSccpSubmissionInput(
                    proofResult = proofResult.copy(sourceProofBytes = byteArrayOf(9, 11)),
                ),
            )
        }
        assertTrue(extraneousSourceProofError.message?.contains("sourceProofBytes") == true)

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
                bundleBytes = sampleBundleBytes(targetDomain = SccpEvm.DOMAIN_BSC),
                sourceProofBytes = ByteArray(0),
                statementHash = "56".repeat(32),
                destinationBinding = binding,
            ),
        )
        assertEquals(SccpEvm.DOMAIN_BSC, request.targetDomain)
        assertEquals(binding.hash, request.destinationBindingHash)

        val proofBytes = sampleGroth16ProofBytes(publicInputs = request.publicInputs)
        val result = SccpBsc.wrapProofResult(proofBytes, request)
        assertFailsWith<IllegalArgumentException> {
            SccpBsc.wrapProofResult(
                proofBytes,
                request.copy(destinationBindingHash = "0x" + "99".repeat(32)),
            )
        }.also { error ->
            assertTrue(error.message?.contains("destinationBindingHash") == true)
        }
        val submission = SccpBsc.buildSubmission(EvmSccpSubmissionInput(result))
        assertEquals(SccpEvm.DOMAIN_BSC, submission.targetDomain)
        assertEquals("evm_groth16_contract_call", submission.platformPayload)
        assertContentEquals(proofBytes, submission.proofBytes)
        val tamperedBscBase64ProofResultError = assertFailsWith<IllegalArgumentException> {
            SccpBsc.buildSubmission(
                EvmSccpSubmissionInput(
                    proofResult = result.copy(proofBase64 = "AAAA"),
                ),
            )
        }
        assertTrue(tamperedBscBase64ProofResultError.message?.contains("proofBase64") == true)
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
                    bundleBytes = sampleBundleBytes(targetDomain = SccpEvm.DOMAIN_BSC),
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
    fun bscTestnetFacadeRequiresChainId97AndBscTarget() {
        SccpBscTestnet.requireTestnetChainId(97)
        val binding = SccpBscTestnet.destinationBinding(
            verifierAddress = "0x" + "11".repeat(20),
            bridgeAddress = "0x" + "22".repeat(20),
            verifierCodeHash = "0x" + "bb".repeat(32),
            verifierKeyHash = "0x" + "cc".repeat(32),
        )
        assertEquals(SccpSourceProofs.BSC_TESTNET_NETWORK_ID, binding.networkId)
        assertEquals(SccpEvm.DOMAIN_BSC, binding.targetDomain)
        assertEquals("0x16eb6817844e492f8fea4fc4742b9e464a80ae392f25d5e6fad9960d49414dcc", binding.hash)
        assertEquals(binding.hash, SccpBscTestnet.destinationBindingHash(
            verifierAddress = "0x" + "11".repeat(20),
            bridgeAddress = "0x" + "22".repeat(20),
            verifierCodeHash = "0x" + "bb".repeat(32),
            verifierKeyHash = "0x" + "cc".repeat(32),
        ))

        val request = SccpBscTestnet.buildProofRequest(
            EvmSccpProofRequestInput(
                publicInputs = samplePublicInputs(targetDomain = SccpEvm.DOMAIN_BSC),
                bundleBytes = sampleBundleBytes(targetDomain = SccpEvm.DOMAIN_BSC),
                sourceProofBytes = ByteArray(0),
                statementHash = "56".repeat(32),
                destinationBinding = binding,
            ),
        )
        assertEquals(SccpEvm.DOMAIN_BSC, request.targetDomain)
        assertEquals(binding.hash, request.destinationBindingHash)

        val proofBytes = sampleGroth16ProofBytes(publicInputs = request.publicInputs)
        val result = SccpBscTestnet.wrapProofResult(proofBytes, request)
        val submission = SccpBscTestnet.buildSubmission(EvmSccpSubmissionInput(result))
        assertEquals(SccpEvm.DOMAIN_BSC, submission.targetDomain)
        assertEquals("evm_groth16_contract_call", submission.platformPayload)
        assertContentEquals(proofBytes, submission.proofBytes)

        assertFailsWith<IllegalArgumentException> {
            SccpBscTestnet.requireTestnetChainId(56)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBscTestnet.destinationBinding(
                verifierAddress = "0x" + "11".repeat(20),
                bridgeAddress = "0x" + "22".repeat(20),
                verifierCodeHash = "0x" + "bb".repeat(32),
                verifierKeyHash = "0x" + "cc".repeat(32),
                networkId = SccpSourceProofs.BSC_MAINNET_NETWORK_ID,
            )
        }
        val mainnetBinding = SccpSourceProofs.bscMainnetDestinationBinding(
            verifierAddress = "0x" + "11".repeat(20),
            bridgeAddress = "0x" + "22".repeat(20),
            verifierCodeHash = "0x" + "bb".repeat(32),
            verifierKeyHash = "0x" + "cc".repeat(32),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpBscTestnet.buildProofRequest(
                EvmSccpProofRequestInput(
                    publicInputs = samplePublicInputs(targetDomain = SccpEvm.DOMAIN_BSC),
                    bundleBytes = sampleBundleBytes(targetDomain = SccpEvm.DOMAIN_BSC),
                    statementHash = "56".repeat(32),
                    destinationBinding = mainnetBinding,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBscTestnet.wrapProofResult(
                proofBytes,
                request.copy(destinationBindingHash = "0x" + "99".repeat(32)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBscTestnet.buildSubmission(
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
    fun bscTestnetFacadeBuildsLocalAdmissionSubmission() {
        val input = BscTestnetLocalAdmissionSubmissionInput(
            proofBytes = byteArrayOf(1, 2, 3),
            publicInputsBytes = byteArrayOf(4, 5, 6),
            bundleBytes = byteArrayOf(7, 8, 9),
            envelopeBytes = byteArrayOf(10, 11, 12),
            statementHash = "0x" + "66".repeat(32),
            sourceVerifierMaterialHash = "0x" + "77".repeat(32),
            sourceAdapterEngineDeploymentHash = "0x" + "88".repeat(32),
        )
        val submission = SccpBscTestnet.buildLocalAdmissionSubmission(input)

        assertEquals(SccpBscTestnet.LOCAL_ADMISSION_SUBMISSION_KIND_V1, submission.platformPayload)
        assertEquals(SccpBscTestnet.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1, submission.envelopeEncoding)
        assertEquals(SccpBscTestnet.LOCAL_ADMISSION_ENTRYPOINT_V1, submission.verifierEntrypoint)
        assertEquals(SccpEvm.DOMAIN_BSC, submission.sourceDomain)
        assertEquals(SccpEvm.DOMAIN_SORA, submission.targetDomain)
        assertEquals(emptyList<EvmSccpSubmissionArgument>(), submission.arguments)
        assertContentEquals(byteArrayOf(1, 2, 3), submission.proofBytes)
        assertContentEquals(byteArrayOf(4, 5, 6), submission.publicInputsBytes)
        assertContentEquals(byteArrayOf(7, 8, 9), submission.bundleBytes)
        assertContentEquals(byteArrayOf(10, 11, 12), submission.envelopeBytes)
        assertContentEquals(byteArrayOf(1, 2, 3), submission.localAdmission.proofBytes)
        assertEquals("0x0a0b0c", submission.envelopeHex)

        input.proofBytes[0] = 99
        assertContentEquals(byteArrayOf(1, 2, 3), submission.proofBytes)

        assertFailsWith<IllegalArgumentException> {
            SccpBscTestnet.buildLocalAdmissionSubmission(
                input.copy(sourceDomain = SccpEvm.DOMAIN_ETH),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBscTestnet.buildLocalAdmissionSubmission(input.copy(proofBytes = byteArrayOf(0, 0)))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBscTestnet.buildLocalAdmissionSubmission(input.copy(envelopeBytes = byteArrayOf()))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBscTestnet.buildLocalAdmissionSubmission(input.copy(envelopeEncoding = "abi_tuple_v1"))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBscTestnet.buildLocalAdmissionSubmission(input.copy(proofFamily = "debug-proof-family"))
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
        assertEquals(SccpEvm.DOMAIN_SORA, binding.sourceDomain)
        assertEquals(SccpEvm.DOMAIN_ETH, binding.targetDomain)
        assertEquals(binding.hash, SccpEthereumMainnet.destinationBindingHash(
            verifierAddress = "0x" + "11".repeat(20),
            bridgeAddress = "0x" + "22".repeat(20),
            verifierCodeHash = "0x" + "bb".repeat(32),
            verifierKeyHash = "0x" + "cc".repeat(32),
        ))

        val input = EvmSccpProofRequestInput(
            publicInputs = samplePublicInputs(targetDomain = SccpEvm.DOMAIN_ETH),
            bundleBytes = sampleBundleBytes(),
            sourceProofBytes = ByteArray(0),
            statementHash = "56".repeat(32),
            destinationBinding = binding,
        )
        val request = SccpEthereumMainnet.buildProofRequest(input)
        assertEquals(SccpEvm.DOMAIN_ETH, request.targetDomain)
        assertEquals(binding.hash, request.destinationBindingHash)
        assertEquals(request, EthereumMainnetSccp().buildOutboundProofRequest(input))
        val nativeProverBundle = sampleEthereumNativeEvmProverBundle(binding.hash)
        val parsedNativeProverBundle = SccpEvm.EthereumMainnetNativeEvmProverBundle.fromJson(
            sampleEthereumNativeEvmProverBundleJson(binding.hash),
            expectedDestinationBindingHash = binding.hash,
        )
        assertEquals(nativeProverBundle.proofArtifactHash, parsedNativeProverBundle.proofArtifactHash)
        assertEquals("artifacts/eth-mainnet/proof-artifact.bin", parsedNativeProverBundle.proofArtifact)
        assertEquals(nativeProverBundle.provingKeyHash, parsedNativeProverBundle.provingKeyHash)
        assertEquals("artifacts/eth-mainnet/proving-key.bin", parsedNativeProverBundle.provingKey)
        assertEquals("artifacts/eth-mainnet/verifier-key.bin", parsedNativeProverBundle.verifierKey)
        assertEquals(nativeProverBundle.destinationBindingHash, parsedNativeProverBundle.destinationBindingHash)
        assertEquals(
            nativeProverBundle.nativeSdkArtifacts.map { it.sdk },
            parsedNativeProverBundle.nativeSdkArtifacts.map { it.sdk },
        )
        assertEquals(
            "artifacts/eth-mainnet/kotlin-implementation.bin",
            parsedNativeProverBundle.nativeSdkArtifacts.first { it.sdk == "kotlin" }.implementationArtifact,
        )
        val parityFixture = SccpEvm.EthereumMainnetNativeEvmProverParityFixture.fromJson(
            sampleEthereumNativeEvmProverParityFixtureJson(nativeProverBundle),
            nativeProverBundle,
        )
        assertEquals(SccpEvm.ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1, parityFixture.schema)
        assertEquals(binding.hash, parityFixture.destinationBindingHash)
        assertEquals(9, parityFixture.publicSignalWords.size)
        assertEquals(
            parityFixture.toriiSubmitPayloadHash,
            parityFixture.sdkResults.getValue("kotlin").toriiSubmitPayloadHash,
        )
        val driftedParityFixture = sampleEthereumNativeEvmProverParityFixtureJson(
            nativeProverBundle,
            kotlinCalldataHash = "0x" + "96".repeat(32),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverParityFixture.fromJson(
                driftedParityFixture,
                nativeProverBundle,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverParityFixture.fromJson(
                sampleEthereumNativeEvmProverParityFixtureJson(nativeProverBundle)
                    .replace(
                        "\"schema\": \"${SccpEvm.ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1}\"",
                        "\"schema\": \"forged\", \"schema\": \"${SccpEvm.ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1}\"",
                    ),
                nativeProverBundle,
            )
        }.also { error ->
            assertTrue(error.message?.contains("Duplicate JSON object key: schema") == true)
        }
        val selfTestFixture = SccpEvm.EthereumMainnetNativeEvmProverSelfTestFixture.fromJson(
            sampleEthereumNativeEvmProverSelfTestFixtureJson(nativeProverBundle),
            nativeProverBundle,
        )
        assertEquals(SccpEvm.ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1, selfTestFixture.schema)
        assertEquals(binding.hash, selfTestFixture.destinationBindingHash)
        assertEquals(9, selfTestFixture.publicSignalWords.size)
        assertEquals(
            selfTestFixture.proofHash,
            selfTestFixture.sdkResults.getValue("kotlin").proofHash,
        )
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverSelfTestFixture.fromJson(
                sampleEthereumNativeEvmProverSelfTestFixtureJson(
                    nativeProverBundle,
                    kotlinProofHash = "0x" + "97".repeat(32),
                ),
                nativeProverBundle,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverSelfTestFixture.fromJson(
                sampleEthereumNativeEvmProverSelfTestFixtureJson(nativeProverBundle)
                    .replace(
                        "\"schema\": \"${SccpEvm.ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1}\"",
                        "\"schema\": \"forged\", \"schema\": \"${SccpEvm.ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1}\"",
                    ),
                nativeProverBundle,
            )
        }.also { error ->
            assertTrue(error.message?.contains("Duplicate JSON object key: schema") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverBundle.fromJson(
                sampleEthereumNativeEvmProverBundleJson(binding.hash, noWasm = false),
                expectedDestinationBindingHash = binding.hash,
            )
        }.also { error ->
            assertTrue(error.message?.contains("noWasm") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverBundle.fromJson(
                sampleEthereumNativeEvmProverBundleJson("0x" + "95".repeat(32)),
                expectedDestinationBindingHash = binding.hash,
            )
        }.also { error ->
            assertTrue(error.message?.contains("destinationBindingHash") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverBundle.fromJson(
                sampleEthereumNativeEvmProverBundleJson(binding.hash)
                    .replace("\"domain\": ${SccpEvm.DOMAIN_ETH}", "\"domain\": \"01\""),
                expectedDestinationBindingHash = binding.hash,
            )
        }.also { error ->
            assertTrue(error.message?.contains("domain") == true)
            assertTrue(error.message?.contains("canonical decimal integer") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverBundle.fromJson(
                sampleEthereumNativeEvmProverBundleJson(binding.hash)
                    .replace("\"sdk\": \"kotlin\"", "\"sdk\": \" kotlin \""),
                expectedDestinationBindingHash = binding.hash,
            )
        }.also { error ->
            assertTrue(error.message?.contains("nativeSdkArtifacts.sdk") == true)
            assertTrue(error.message?.contains("canonical string") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverBundle.fromJson(
                sampleEthereumNativeEvmProverBundleJson(binding.hash)
                    .replace(
                        "\"bundle_id\": \"sccp:eth:native-evm-groth16-prover:ethereum-mainnet:v1\"",
                        "\"bundle_id\": \"forged\", \"bundle_id\": \"sccp:eth:native-evm-groth16-prover:ethereum-mainnet:v1\"",
                    ),
                expectedDestinationBindingHash = binding.hash,
            )
        }.also { error ->
            assertTrue(error.message?.contains("Duplicate JSON object key: bundle_id") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverBundle.fromJson(
                sampleEthereumNativeEvmProverBundleJson(
                    binding.hash,
                    proofArtifact = "../proof-artifact.bin",
                ),
                expectedDestinationBindingHash = binding.hash,
            )
        }.also { error ->
            assertTrue(error.message?.contains("proofArtifact") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverBundle.fromJson(
                sampleEthereumNativeEvmProverBundleJson(
                    binding.hash,
                    proofArtifact = "ipfs:proof-artifact.bin",
                ),
                expectedDestinationBindingHash = binding.hash,
            )
        }.also { error ->
            assertTrue(error.message?.contains("proofArtifact") == true)
            assertTrue(error.message?.contains("URI schemes") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverBundle.fromJson(
                sampleEthereumNativeEvmProverBundleJson(
                    binding.hash,
                    proofArtifact = "artifacts/eth-mainnet/proof.wasm",
                ),
                expectedDestinationBindingHash = binding.hash,
            )
        }.also { error ->
            assertTrue(error.message?.contains("proofArtifact") == true)
            assertTrue(error.message?.contains("forbidden prover dependency marker: wasm") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverBundle.fromJson(
                sampleEthereumNativeEvmProverBundleJson(binding.hash)
                    .replace(
                        "\"audit_hashes\":",
                        "\"experimental_manifest_note\":true,\"audit_hashes\":",
                    ),
                expectedDestinationBindingHash = binding.hash,
            )
        }.also { error ->
            assertTrue(error.message?.contains("nativeProverBundle") == true)
            assertTrue(error.message?.contains("experimental_manifest_note") == true)
            assertTrue(error.message?.contains("unknown field") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverBundle.fromJson(
                sampleEthereumNativeEvmProverBundleJson(binding.hash)
                    .replace(
                        "\"proof_artifact_hash\": \"0x${"91".repeat(32)}\"",
                        "\"proofArtifactHash\": \"0x${"91".repeat(32)}\", \"proof_artifact_hash\": \"0x${"91".repeat(32)}\"",
                    ),
                expectedDestinationBindingHash = binding.hash,
            )
        }.also { error ->
            assertTrue(error.message?.contains("proofArtifactHash") == true)
            assertTrue(error.message?.contains("multiple aliases") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverBundle.fromJson(
                sampleEthereumNativeEvmProverBundleJson(binding.hash)
                    .replace(
                        "\"implementation_hash\":",
                        "\"experimental_manifest_note\":true,\"implementation_hash\":",
                    ),
                expectedDestinationBindingHash = binding.hash,
            )
        }.also { error ->
            assertTrue(error.message?.contains("nativeSdkArtifacts[0]") == true)
            assertTrue(error.message?.contains("experimental_manifest_note") == true)
            assertTrue(error.message?.contains("unknown field") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverBundle.fromJson(
                sampleEthereumNativeEvmProverBundleJson(binding.hash)
                    .replace("\"0x${"a1".repeat(32)}\"", "\"0x${"A1".repeat(32)}\""),
                expectedDestinationBindingHash = binding.hash,
            )
        }.also { error ->
            assertTrue(error.message?.contains("auditHashes.circuit_security_audit") == true)
            assertTrue(error.message?.contains("canonical lowercase") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpEvm.EthereumMainnetNativeEvmProverBundle.fromJson(
                sampleEthereumNativeEvmProverBundleJson(binding.hash)
                    .replace("\"0x${"a1".repeat(32)}\"", "\"0x${"91".repeat(32)}\""),
                expectedDestinationBindingHash = binding.hash,
            )
        }.also { error ->
            assertTrue(error.message?.contains("auditHashes.circuit_security_audit") == true)
            assertTrue(error.message?.contains("proofArtifactHash") == true)
            assertTrue(error.message?.contains("role-separated") == true)
        }
        val bundledRequest = SccpEthereumMainnet.buildProofRequest(input, nativeProverBundle)
        assertEquals("0x" + "91".repeat(32), bundledRequest.proofArtifactHash)
        assertEquals("0x" + "92".repeat(32), bundledRequest.provingKeyHash)
        assertTrue(bundledRequest.requestHash != request.requestHash)
        assertEquals(
            bundledRequest,
            EthereumMainnetSccp(nativeProverBundle = nativeProverBundle)
                .buildOutboundProofRequest(input),
        )
        assertFailsWith<IllegalArgumentException> {
            sampleEthereumNativeEvmProverBundle(
                binding.hash,
                verifierKeyHash = "0x" + "dd".repeat(32),
            ).applyTo(input)
        }.also { error ->
            assertTrue(error.message?.contains("nativeProverBundle.verifierKeyHash") == true)
        }
        val proofArtifactBytes = nativeEvmProverArtifactBytes("kotlin proof artifact v1")
        val provingKeyBytes = nativeEvmProverArtifactBytes("kotlin proving key v1")
        val verifierKeyBytes = nativeEvmProverArtifactBytes("kotlin verifier key v1")
        val implementationBytes = nativeEvmProverArtifactBytes("kotlin implementation artifact v1")
        val proofArtifactHash = sha256Hex(proofArtifactBytes)
        val provingKeyHash = sha256Hex(provingKeyBytes)
        val verifierKeyHash = sha256Hex(verifierKeyBytes)
        val implementationHash = sha256Hex(implementationBytes)
        val artifactBinding = SccpEthereumMainnet.destinationBinding(
            verifierAddress = "0x" + "11".repeat(20),
            bridgeAddress = "0x" + "22".repeat(20),
            verifierCodeHash = "0x" + "bb".repeat(32),
            verifierKeyHash = verifierKeyHash,
        )
        val artifactInput = input.copy(
            destinationBindingHash = artifactBinding.hash,
            destinationBinding = artifactBinding,
        )
        val draftVerifiedBundle = SccpEvm.EthereumMainnetNativeEvmProverBundle(
            proofArtifact = "artifacts/eth-mainnet/proof-artifact.bin",
            proofArtifactHash = proofArtifactHash,
            provingKey = "artifacts/eth-mainnet/proving-key.bin",
            provingKeyHash = provingKeyHash,
            verifierKey = "artifacts/eth-mainnet/verifier-key.bin",
            verifierKeyHash = verifierKeyHash,
            destinationBindingHash = artifactBinding.hash,
            nativeSdkArtifacts = SccpEvm.ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1
                .entries
                .sortedBy { it.key }
                .mapIndexed { index, entry ->
                    SccpEvm.EthereumMainnetNativeEvmProverBundleSdkArtifact(
                        sdk = entry.key,
                        implementation = entry.value,
                        proofArtifactHash = proofArtifactHash,
                        provingKeyHash = provingKeyHash,
                        implementationArtifact = "artifacts/eth-mainnet/${entry.key}-implementation.bin",
                        implementationHash = if (entry.key == "kotlin") {
                            implementationHash
                        } else {
                            "0x" + (index + 1).toString(16).padStart(2, '0').repeat(32)
                        },
                    )
                },
            crossSdkFixtureParityArtifact = "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
            nativeProverSelfTestArtifact = "artifacts/eth-mainnet/native-prover-self-test.json",
            auditHashes = sampleEthereumNativeAuditHashes(),
            expectedDestinationBindingHash = artifactBinding.hash,
        )
        val parityFixtureBytes = sampleEthereumNativeEvmProverParityFixtureJson(draftVerifiedBundle)
            .toByteArray(Charsets.UTF_8)
        val parityFixtureHash = sha256Hex(parityFixtureBytes)
        val selfTestFixtureBytes = sampleEthereumNativeEvmProverSelfTestFixtureJson(draftVerifiedBundle)
            .toByteArray(Charsets.UTF_8)
        val selfTestFixtureHash = sha256Hex(selfTestFixtureBytes)
        val verifiedBundle = SccpEvm.EthereumMainnetNativeEvmProverBundle(
            proofArtifact = "artifacts/eth-mainnet/proof-artifact.bin",
            proofArtifactHash = proofArtifactHash,
            provingKey = "artifacts/eth-mainnet/proving-key.bin",
            provingKeyHash = provingKeyHash,
            verifierKey = "artifacts/eth-mainnet/verifier-key.bin",
            verifierKeyHash = verifierKeyHash,
            destinationBindingHash = artifactBinding.hash,
            nativeSdkArtifacts = draftVerifiedBundle.nativeSdkArtifacts,
            crossSdkFixtureParityArtifact = "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
            nativeProverSelfTestArtifact = "artifacts/eth-mainnet/native-prover-self-test.json",
            auditHashes = sampleEthereumNativeAuditHashes() +
                ("cross_sdk_fixture_parity" to parityFixtureHash) +
                ("native_prover_self_test" to selfTestFixtureHash),
            expectedDestinationBindingHash = artifactBinding.hash,
        )
        fun hashConsistentNativeEvmProverBundle(
            proofArtifactBytesOverride: ByteArray? = null,
            provingKeyBytesOverride: ByteArray? = null,
            verifierKeyBytesOverride: ByteArray? = null,
            implementationBytesOverride: ByteArray? = null,
            crossSdkFixtureParityBytesOverride: ByteArray? = null,
            nativeProverSelfTestBytesOverride: ByteArray? = null,
        ): Triple<SccpEvm.EthereumMainnetNativeEvmProverBundle, ByteArray, ByteArray> {
            val selectedProofArtifactBytes = proofArtifactBytesOverride ?: proofArtifactBytes
            val selectedProvingKeyBytes = provingKeyBytesOverride ?: provingKeyBytes
            val selectedVerifierKeyBytes = verifierKeyBytesOverride ?: verifierKeyBytes
            val selectedImplementationBytes = implementationBytesOverride ?: implementationBytes
            val selectedProofArtifactHash = sha256Hex(selectedProofArtifactBytes)
            val selectedProvingKeyHash = sha256Hex(selectedProvingKeyBytes)
            val selectedVerifierKeyHash = sha256Hex(selectedVerifierKeyBytes)
            val selectedImplementationHash = sha256Hex(selectedImplementationBytes)
            val draftBundle = SccpEvm.EthereumMainnetNativeEvmProverBundle(
                proofArtifact = "artifacts/eth-mainnet/proof-artifact.bin",
                proofArtifactHash = selectedProofArtifactHash,
                provingKey = "artifacts/eth-mainnet/proving-key.bin",
                provingKeyHash = selectedProvingKeyHash,
                verifierKey = "artifacts/eth-mainnet/verifier-key.bin",
                verifierKeyHash = selectedVerifierKeyHash,
                destinationBindingHash = artifactBinding.hash,
                nativeSdkArtifacts = SccpEvm.ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1
                    .entries
                    .sortedBy { it.key }
                    .mapIndexed { index, entry ->
                        SccpEvm.EthereumMainnetNativeEvmProverBundleSdkArtifact(
                            sdk = entry.key,
                            implementation = entry.value,
                            proofArtifactHash = selectedProofArtifactHash,
                            provingKeyHash = selectedProvingKeyHash,
                            implementationArtifact = "artifacts/eth-mainnet/${entry.key}-implementation.bin",
                            implementationHash = if (entry.key == "kotlin") {
                                selectedImplementationHash
                            } else {
                                "0x" + (index + 1).toString(16).padStart(2, '0').repeat(32)
                            },
                        )
                    },
                crossSdkFixtureParityArtifact = "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
                nativeProverSelfTestArtifact = "artifacts/eth-mainnet/native-prover-self-test.json",
                auditHashes = sampleEthereumNativeAuditHashes(),
                expectedDestinationBindingHash = artifactBinding.hash,
            )
            val selectedParityFixtureBytes = crossSdkFixtureParityBytesOverride
                ?: sampleEthereumNativeEvmProverParityFixtureJson(draftBundle)
                    .toByteArray(Charsets.UTF_8)
            val selectedSelfTestFixtureBytes = nativeProverSelfTestBytesOverride
                ?: sampleEthereumNativeEvmProverSelfTestFixtureJson(draftBundle)
                    .toByteArray(Charsets.UTF_8)
            val selectedBundle = SccpEvm.EthereumMainnetNativeEvmProverBundle(
                proofArtifact = "artifacts/eth-mainnet/proof-artifact.bin",
                proofArtifactHash = selectedProofArtifactHash,
                provingKey = "artifacts/eth-mainnet/proving-key.bin",
                provingKeyHash = selectedProvingKeyHash,
                verifierKey = "artifacts/eth-mainnet/verifier-key.bin",
                verifierKeyHash = selectedVerifierKeyHash,
                destinationBindingHash = artifactBinding.hash,
                nativeSdkArtifacts = draftBundle.nativeSdkArtifacts,
                crossSdkFixtureParityArtifact = "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
                nativeProverSelfTestArtifact = "artifacts/eth-mainnet/native-prover-self-test.json",
                auditHashes = sampleEthereumNativeAuditHashes() +
                    ("cross_sdk_fixture_parity" to sha256Hex(selectedParityFixtureBytes)) +
                    ("native_prover_self_test" to sha256Hex(selectedSelfTestFixtureBytes)),
                expectedDestinationBindingHash = artifactBinding.hash,
            )
            return Triple(selectedBundle, selectedParityFixtureBytes, selectedSelfTestFixtureBytes)
        }
        val verifiedArtifacts = verifiedBundle.verifiedArtifacts(
            proofArtifactBytes = proofArtifactBytes,
            provingKeyBytes = provingKeyBytes,
            verifierKeyBytes = verifierKeyBytes,
            sdk = "kotlin",
            implementationBytes = implementationBytes,
            crossSdkFixtureParityBytes = parityFixtureBytes,
            nativeProverSelfTestBytes = selfTestFixtureBytes,
        )
        assertEquals(SccpEvm.NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1, verifiedArtifacts.hashAlgorithm)
        assertEquals(proofArtifactHash, verifiedArtifacts.proofArtifactHash)
        assertEquals(provingKeyHash, verifiedArtifacts.provingKeyHash)
        assertEquals(verifierKeyHash, verifiedArtifacts.verifierKeyHash)
        assertEquals(parityFixtureHash, verifiedArtifacts.crossSdkFixtureParityHash)
        assertEquals("0x" + "d3".repeat(32), verifiedArtifacts.crossSdkFixtureParity?.calldataHash)
        assertEquals(selfTestFixtureHash, verifiedArtifacts.nativeProverSelfTestHash)
        assertEquals("0x" + "e4".repeat(32), verifiedArtifacts.nativeProverSelfTest?.proofHash)
        assertEquals("native-kotlin", verifiedArtifacts.implementation)
        assertEquals(implementationHash, verifiedArtifacts.implementationHash)
        val kotlinImplementationPath = verifiedBundle.nativeSdkArtifacts
            .first { it.sdk == "kotlin" }
            .implementationArtifact
        val artifactBytesByPath = mapOf(
            verifiedBundle.proofArtifact to proofArtifactBytes,
            verifiedBundle.provingKey to provingKeyBytes,
            verifiedBundle.verifierKey to verifierKeyBytes,
            verifiedBundle.crossSdkFixtureParityArtifact to parityFixtureBytes,
            verifiedBundle.nativeProverSelfTestArtifact to selfTestFixtureBytes,
            kotlinImplementationPath to implementationBytes,
        )
        val verifiedFromResolver = verifiedBundle.verifiedArtifacts("kotlin") { path ->
            artifactBytesByPath[path] ?: throw IllegalArgumentException(path)
        }
        assertEquals(implementationHash, verifiedFromResolver.implementationHash)
        assertEquals(parityFixtureHash, verifiedFromResolver.crossSdkFixtureParityHash)
        assertEquals(selfTestFixtureHash, verifiedFromResolver.nativeProverSelfTestHash)
        assertFailsWith<IllegalArgumentException> {
            verifiedBundle.verifiedArtifacts(" kotlin ") { path ->
                artifactBytesByPath[path] ?: throw IllegalArgumentException(path)
            }
        }.also { error ->
            assertTrue(error.message?.contains("sdk must be a non-empty canonical string") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            verifiedBundle.verifiedArtifacts("kotlin") { path ->
                if (path == verifiedBundle.crossSdkFixtureParityArtifact) {
                    throw IllegalArgumentException("crossSdkFixtureParityArtifact")
                }
                artifactBytesByPath.getValue(path)
            }
        }.also { error ->
            assertTrue(error.message?.contains("crossSdkFixtureParityArtifact") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            verifiedBundle.verifiedArtifacts("kotlin") { path ->
                if (path == verifiedBundle.nativeProverSelfTestArtifact) {
                    throw IllegalArgumentException("nativeProverSelfTestArtifact")
                }
                artifactBytesByPath.getValue(path)
            }
        }.also { error ->
            assertTrue(error.message?.contains("nativeProverSelfTestArtifact") == true)
        }
        var missingArtifactsProverCalled = false
        val missingArtifactsFacade = EthereumMainnetSccp(
            proofEngine = EvmSccpProofEngine {
                missingArtifactsProverCalled = true
                proofBytes
            },
        )
        assertFailsWith<IllegalArgumentException> {
            missingArtifactsFacade.proveOutboundToEthereum(input)
        }.also { error ->
            assertTrue(error.message?.contains("verified native EVM prover artifacts") == true)
        }
        assertFalse(missingArtifactsProverCalled)
        var artifactBoundRequest: EvmSccpProofRequest? = null
        var artifactBoundSelfTestCalled = false
        val artifactBoundFacade = EthereumMainnetSccp(
            proofEngine = EvmSccpProofEngine { callbackRequest ->
                artifactBoundRequest = callbackRequest
                assertEquals(proofArtifactHash, callbackRequest.proofArtifactHash)
                assertEquals(provingKeyHash, callbackRequest.provingKeyHash)
                proofBytes
            },
            nativeProverSelfTest = EthereumMainnetNativeProverSelfTest { fixture, expected, artifacts ->
                artifactBoundSelfTestCalled = true
                assertEquals("0x" + "e4".repeat(32), fixture.proofHash)
                assertEquals(selfTestFixtureHash, artifacts.nativeProverSelfTestHash)
                expected
            },
            nativeProverArtifacts = verifiedArtifacts,
        )
        val preflightResult = artifactBoundFacade.runNativeProverSelfTest()
        assertTrue(artifactBoundSelfTestCalled)
        assertEquals("0x" + "e4".repeat(32), preflightResult.proofHash)
        val paddedSdkArtifacts = verifiedArtifacts.copy(sdk = " kotlin ")
        var paddedSelfTestHookCalled = false
        assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                nativeProverSelfTest = EthereumMainnetNativeProverSelfTest { _, expected, _ ->
                    paddedSelfTestHookCalled = true
                    expected
                },
                nativeProverArtifacts = paddedSdkArtifacts,
            ).runNativeProverSelfTest()
        }.also { error ->
            assertTrue(
                error.message?.contains(
                    "nativeProverArtifacts sdk must be a non-empty canonical string",
                ) == true,
            )
        }
        assertFalse(paddedSelfTestHookCalled)
        artifactBoundSelfTestCalled = false
        val artifactBoundResult = artifactBoundFacade.proveOutboundToEthereum(artifactInput)
        assertTrue(artifactBoundSelfTestCalled)
        assertEquals(proofArtifactHash, artifactBoundRequest?.proofArtifactHash)
        assertEquals(proofArtifactHash, artifactBoundResult.proofArtifactHash)
        assertEquals(provingKeyHash, artifactBoundResult.provingKeyHash)
        var missingSelfTestHookProverCalled = false
        assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                proofEngine = EvmSccpProofEngine {
                    missingSelfTestHookProverCalled = true
                    proofBytes
                },
                nativeProverArtifacts = verifiedArtifacts,
            ).proveOutboundToEthereum(artifactInput)
        }.also { error ->
            assertTrue(error.message?.contains("nativeProverSelfTest runner") == true)
        }
        assertFalse(missingSelfTestHookProverCalled)
        var driftingSelfTestHookProverCalled = false
        assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                proofEngine = EvmSccpProofEngine {
                    driftingSelfTestHookProverCalled = true
                    proofBytes
                },
                nativeProverSelfTest = EthereumMainnetNativeProverSelfTest { _, expected, _ ->
                    expected.copy(proofHash = "0x" + "97".repeat(32))
                },
                nativeProverArtifacts = verifiedArtifacts,
            ).proveOutboundToEthereum(artifactInput)
        }.also { error ->
            assertTrue(error.message?.contains("nativeProverSelfTest result") == true)
        }
        assertFalse(driftingSelfTestHookProverCalled)
        var factoryBoundRequest: EvmSccpProofRequest? = null
        val factoryBoundFacade = EthereumMainnetSccp.fromNativeProverBundle(
            proofEngine = EvmSccpProofEngine { callbackRequest ->
                factoryBoundRequest = callbackRequest
                proofBytes
            },
            nativeProverSelfTest = EthereumMainnetNativeProverSelfTest { _, expected, _ -> expected },
            nativeProverBundle = verifiedBundle,
            sdk = "kotlin",
            artifactResolver = SccpEvm.NativeEvmProverArtifactResolver { path ->
                artifactBytesByPath[path] ?: throw IllegalArgumentException(path)
            },
        )
        val factoryBoundResult = factoryBoundFacade.proveOutboundToEthereum(artifactInput)
        assertEquals(proofArtifactHash, factoryBoundRequest?.proofArtifactHash)
        assertEquals(provingKeyHash, factoryBoundRequest?.provingKeyHash)
        assertEquals(proofArtifactHash, factoryBoundResult.proofArtifactHash)
        assertEquals(provingKeyHash, factoryBoundResult.provingKeyHash)
        assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp.fromNativeProverBundle(
                nativeProverBundle = verifiedBundle,
                sdk = "kotlin",
                artifactResolver = SccpEvm.NativeEvmProverArtifactResolver { path ->
                    if (path == verifiedBundle.crossSdkFixtureParityArtifact) {
                        throw IllegalArgumentException("crossSdkFixtureParityArtifact")
                    }
                    artifactBytesByPath.getValue(path)
                },
            )
        }.also { error ->
            assertTrue(error.message?.contains("crossSdkFixtureParityArtifact") == true)
        }
        val implementationUnboundArtifacts = SccpEvm.EthereumMainnetNativeEvmProverArtifacts(
            hashAlgorithm = SccpEvm.NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1,
            nativeProverBundle = verifiedBundle,
            proofArtifactHash = proofArtifactHash,
            provingKeyHash = provingKeyHash,
            verifierKeyHash = verifierKeyHash,
            crossSdkFixtureParityHash = parityFixtureHash,
            crossSdkFixtureParity = verifiedArtifacts.crossSdkFixtureParity,
            nativeProverSelfTestHash = selfTestFixtureHash,
            nativeProverSelfTest = verifiedArtifacts.nativeProverSelfTest,
            sdk = "kotlin",
            implementation = "native-kotlin",
            implementationHash = null,
        )
        var implementationUnboundProverCalled = false
        assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                proofEngine = EvmSccpProofEngine {
                    implementationUnboundProverCalled = true
                    proofBytes
                },
                nativeProverArtifacts = implementationUnboundArtifacts,
            ).proveOutboundToEthereum(artifactInput)
        }.also { error ->
            assertTrue(
                error.message?.contains(
                    "nativeProverArtifacts must bind sdk implementation and implementationHash",
                ) == true,
            )
        }
        assertFalse(implementationUnboundProverCalled)
        var paddedSdkProverCalled = false
        assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                proofEngine = EvmSccpProofEngine {
                    paddedSdkProverCalled = true
                    proofBytes
                },
                nativeProverArtifacts = paddedSdkArtifacts,
            ).proveOutboundToEthereum(artifactInput)
        }.also { error ->
            assertTrue(
                error.message?.contains(
                    "nativeProverArtifacts must bind sdk implementation and implementationHash",
                ) == true,
            )
        }
        assertFalse(paddedSdkProverCalled)
        val verifierKeyUnboundArtifacts = SccpEvm.EthereumMainnetNativeEvmProverArtifacts(
            hashAlgorithm = SccpEvm.NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1,
            nativeProverBundle = verifiedBundle,
            proofArtifactHash = proofArtifactHash,
            provingKeyHash = provingKeyHash,
            verifierKeyHash = "0x" + "ef".repeat(32),
            crossSdkFixtureParityHash = parityFixtureHash,
            crossSdkFixtureParity = verifiedArtifacts.crossSdkFixtureParity,
            nativeProverSelfTestHash = selfTestFixtureHash,
            nativeProverSelfTest = verifiedArtifacts.nativeProverSelfTest,
            sdk = "kotlin",
            implementation = "native-kotlin",
            implementationHash = implementationHash,
        )
        var verifierKeyUnboundProverCalled = false
        assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                proofEngine = EvmSccpProofEngine {
                    verifierKeyUnboundProverCalled = true
                    proofBytes
                },
                nativeProverArtifacts = verifierKeyUnboundArtifacts,
            ).proveOutboundToEthereum(artifactInput)
        }.also { error ->
            assertTrue(
                error.message?.contains("nativeProverArtifacts verifierKeyHash must match nativeProverBundle") == true,
            )
        }
        assertFalse(verifierKeyUnboundProverCalled)
        assertFailsWith<IllegalArgumentException> {
            verifiedBundle.verifiedArtifacts(
                proofArtifactBytes = byteArrayOf(0),
                provingKeyBytes = provingKeyBytes,
                verifierKeyBytes = verifierKeyBytes,
            )
        }.also { error ->
            assertTrue(error.message?.contains("proofArtifactBytes sha256") == true)
        }
        val tinyProofArtifactBytes = byteArrayOf(1, 2, 3, 4, 5, 6, 7)
        val tinyProofArtifactHash = sha256Hex(tinyProofArtifactBytes)
        val draftTinyBundle = SccpEvm.EthereumMainnetNativeEvmProverBundle(
            proofArtifact = "artifacts/eth-mainnet/proof-artifact.bin",
            proofArtifactHash = tinyProofArtifactHash,
            provingKey = "artifacts/eth-mainnet/proving-key.bin",
            provingKeyHash = provingKeyHash,
            verifierKey = "artifacts/eth-mainnet/verifier-key.bin",
            verifierKeyHash = verifierKeyHash,
            destinationBindingHash = artifactBinding.hash,
            nativeSdkArtifacts = SccpEvm.ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1
                .entries
                .sortedBy { it.key }
                .mapIndexed { index, entry ->
                    SccpEvm.EthereumMainnetNativeEvmProverBundleSdkArtifact(
                        sdk = entry.key,
                        implementation = entry.value,
                        proofArtifactHash = tinyProofArtifactHash,
                        provingKeyHash = provingKeyHash,
                        implementationArtifact = "artifacts/eth-mainnet/${entry.key}-implementation.bin",
                        implementationHash = if (entry.key == "kotlin") {
                            implementationHash
                        } else {
                            "0x" + (index + 1).toString(16).padStart(2, '0').repeat(32)
                        },
                    )
                },
            crossSdkFixtureParityArtifact = "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
            nativeProverSelfTestArtifact = "artifacts/eth-mainnet/native-prover-self-test.json",
            auditHashes = sampleEthereumNativeAuditHashes(),
            expectedDestinationBindingHash = artifactBinding.hash,
        )
        val tinyProofArtifactParityFixtureBytes = sampleEthereumNativeEvmProverParityFixtureJson(draftTinyBundle)
            .toByteArray(Charsets.UTF_8)
        val tinyProofArtifactSelfTestFixtureBytes = sampleEthereumNativeEvmProverSelfTestFixtureJson(draftTinyBundle)
            .toByteArray(Charsets.UTF_8)
        val tinyBundle = SccpEvm.EthereumMainnetNativeEvmProverBundle(
            proofArtifact = "artifacts/eth-mainnet/proof-artifact.bin",
            proofArtifactHash = tinyProofArtifactHash,
            provingKey = "artifacts/eth-mainnet/proving-key.bin",
            provingKeyHash = provingKeyHash,
            verifierKey = "artifacts/eth-mainnet/verifier-key.bin",
            verifierKeyHash = verifierKeyHash,
            destinationBindingHash = artifactBinding.hash,
            nativeSdkArtifacts = draftTinyBundle.nativeSdkArtifacts,
            crossSdkFixtureParityArtifact = "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
            nativeProverSelfTestArtifact = "artifacts/eth-mainnet/native-prover-self-test.json",
            auditHashes = sampleEthereumNativeAuditHashes() +
                ("cross_sdk_fixture_parity" to sha256Hex(tinyProofArtifactParityFixtureBytes)) +
                ("native_prover_self_test" to sha256Hex(tinyProofArtifactSelfTestFixtureBytes)),
            expectedDestinationBindingHash = artifactBinding.hash,
        )
        assertFailsWith<IllegalArgumentException> {
            tinyBundle.verifiedArtifacts(
                proofArtifactBytes = tinyProofArtifactBytes,
                provingKeyBytes = provingKeyBytes,
                verifierKeyBytes = verifierKeyBytes,
                sdk = "kotlin",
                implementationBytes = implementationBytes,
                crossSdkFixtureParityBytes = tinyProofArtifactParityFixtureBytes,
                nativeProverSelfTestBytes = tinyProofArtifactSelfTestFixtureBytes,
            )
        }.also { error ->
            assertTrue(error.message?.contains("proofArtifactBytes must be at least 65536 bytes") == true)
        }
        val tinyProvingKeyBytes = byteArrayOf(8, 9, 10, 11)
        val (tinyProvingKeyBundle, tinyProvingKeyParityBytes, tinyProvingKeySelfTestBytes) =
            hashConsistentNativeEvmProverBundle(provingKeyBytesOverride = tinyProvingKeyBytes)
        assertFailsWith<IllegalArgumentException> {
            tinyProvingKeyBundle.verifiedArtifacts(
                proofArtifactBytes = proofArtifactBytes,
                provingKeyBytes = tinyProvingKeyBytes,
                verifierKeyBytes = verifierKeyBytes,
                sdk = "kotlin",
                implementationBytes = implementationBytes,
                crossSdkFixtureParityBytes = tinyProvingKeyParityBytes,
                nativeProverSelfTestBytes = tinyProvingKeySelfTestBytes,
            )
        }.also { error ->
            assertTrue(error.message?.contains("provingKeyBytes must be at least 65536 bytes") == true)
        }
        val tinyVerifierKeyBytes = byteArrayOf(12, 13, 14, 15)
        val (tinyVerifierKeyBundle, tinyVerifierKeyParityBytes, tinyVerifierKeySelfTestBytes) =
            hashConsistentNativeEvmProverBundle(verifierKeyBytesOverride = tinyVerifierKeyBytes)
        assertFailsWith<IllegalArgumentException> {
            tinyVerifierKeyBundle.verifiedArtifacts(
                proofArtifactBytes = proofArtifactBytes,
                provingKeyBytes = provingKeyBytes,
                verifierKeyBytes = tinyVerifierKeyBytes,
                sdk = "kotlin",
                implementationBytes = implementationBytes,
                crossSdkFixtureParityBytes = tinyVerifierKeyParityBytes,
                nativeProverSelfTestBytes = tinyVerifierKeySelfTestBytes,
            )
        }.also { error ->
            assertTrue(error.message?.contains("verifierKeyBytes must be at least 128 bytes") == true)
        }
        val tinyParityFixtureBytes = "{}".toByteArray(Charsets.UTF_8)
        val (tinyParityBundle, _, tinyParitySelfTestBytes) =
            hashConsistentNativeEvmProverBundle(
                crossSdkFixtureParityBytesOverride = tinyParityFixtureBytes,
            )
        assertFailsWith<IllegalArgumentException> {
            tinyParityBundle.verifiedArtifacts(
                proofArtifactBytes = proofArtifactBytes,
                provingKeyBytes = provingKeyBytes,
                verifierKeyBytes = verifierKeyBytes,
                sdk = "kotlin",
                implementationBytes = implementationBytes,
                crossSdkFixtureParityBytes = tinyParityFixtureBytes,
                nativeProverSelfTestBytes = tinyParitySelfTestBytes,
            )
        }.also { error ->
            assertTrue(error.message?.contains("crossSdkFixtureParityBytes must be at least 128 bytes") == true)
        }
        val tinySelfTestFixtureBytes = "{}".toByteArray(Charsets.UTF_8)
        val (tinySelfTestBundle, tinySelfTestParityBytes, _) =
            hashConsistentNativeEvmProverBundle(
                nativeProverSelfTestBytesOverride = tinySelfTestFixtureBytes,
            )
        assertFailsWith<IllegalArgumentException> {
            tinySelfTestBundle.verifiedArtifacts(
                proofArtifactBytes = proofArtifactBytes,
                provingKeyBytes = provingKeyBytes,
                verifierKeyBytes = verifierKeyBytes,
                sdk = "kotlin",
                implementationBytes = implementationBytes,
                crossSdkFixtureParityBytes = tinySelfTestParityBytes,
                nativeProverSelfTestBytes = tinySelfTestFixtureBytes,
            )
        }.also { error ->
            assertTrue(error.message?.contains("nativeProverSelfTestBytes must be at least 128 bytes") == true)
        }
        val tinyImplementationBytes = byteArrayOf(16, 17, 18, 19)
        val (tinyImplementationBundle, tinyImplementationParityBytes, tinyImplementationSelfTestBytes) =
            hashConsistentNativeEvmProverBundle(implementationBytesOverride = tinyImplementationBytes)
        assertFailsWith<IllegalArgumentException> {
            tinyImplementationBundle.verifiedArtifacts(
                proofArtifactBytes = proofArtifactBytes,
                provingKeyBytes = provingKeyBytes,
                verifierKeyBytes = verifierKeyBytes,
                sdk = "kotlin",
                implementationBytes = tinyImplementationBytes,
                crossSdkFixtureParityBytes = tinyImplementationParityBytes,
                nativeProverSelfTestBytes = tinyImplementationSelfTestBytes,
            )
        }.also { error ->
            assertTrue(error.message?.contains("implementationBytes must be at least 1024 bytes") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            verifiedBundle.verifiedArtifacts(
                proofArtifactBytes = proofArtifactBytes,
                provingKeyBytes = provingKeyBytes,
                verifierKeyBytes = verifierKeyBytes,
                crossSdkFixtureParityBytes = parityFixtureBytes,
                nativeProverSelfTestBytes = selfTestFixtureBytes,
                implementationBytes = implementationBytes,
            )
        }.also { error ->
            assertTrue(error.message?.contains("sdk must be a non-empty canonical string") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            verifiedBundle.verifiedArtifacts(
                proofArtifactBytes = proofArtifactBytes,
                provingKeyBytes = provingKeyBytes,
                verifierKeyBytes = verifierKeyBytes,
                crossSdkFixtureParityBytes = parityFixtureBytes,
                nativeProverSelfTestBytes = selfTestFixtureBytes,
                sdk = " kotlin ",
                implementationBytes = implementationBytes,
            )
        }.also { error ->
            assertTrue(error.message?.contains("sdk must be a non-empty canonical string") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            verifiedBundle.verifiedArtifacts(
                proofArtifactBytes = proofArtifactBytes,
                provingKeyBytes = provingKeyBytes,
                verifierKeyBytes = verifierKeyBytes,
                crossSdkFixtureParityBytes = parityFixtureBytes,
                nativeProverSelfTestBytes = selfTestFixtureBytes,
                sdk = "kotlin",
            )
        }.also { error ->
            assertTrue(error.message?.contains("implementationBytes are required") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            verifiedBundle.verifiedArtifacts(
                proofArtifactBytes = proofArtifactBytes,
                provingKeyBytes = provingKeyBytes,
                verifierKeyBytes = verifierKeyBytes,
                sdk = "kotlin",
                implementationBytes = "tampered".toByteArray(),
                crossSdkFixtureParityBytes = parityFixtureBytes,
                nativeProverSelfTestBytes = selfTestFixtureBytes,
            )
        }.also { error ->
            assertTrue(error.message?.contains("implementationBytes sha256") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            verifiedBundle.verifiedArtifacts(
                proofArtifactBytes = proofArtifactBytes,
                provingKeyBytes = provingKeyBytes,
                verifierKeyBytes = verifierKeyBytes,
                sdk = "kotlin",
                implementationBytes = implementationBytes,
            )
        }.also { error ->
            assertTrue(error.message?.contains("crossSdkFixtureParityBytes") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            verifiedBundle.verifiedArtifacts(
                proofArtifactBytes = proofArtifactBytes,
                provingKeyBytes = provingKeyBytes,
                verifierKeyBytes = verifierKeyBytes,
                sdk = "kotlin",
                implementationBytes = implementationBytes,
                crossSdkFixtureParityBytes = parityFixtureBytes,
            )
        }.also { error ->
            assertTrue(error.message?.contains("nativeProverSelfTestBytes") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            verifiedBundle.verifiedArtifacts(
                proofArtifactBytes = proofArtifactBytes,
                provingKeyBytes = provingKeyBytes,
                verifierKeyBytes = verifierKeyBytes,
                sdk = "kotlin",
                implementationBytes = implementationBytes,
                crossSdkFixtureParityBytes = "{}".toByteArray(),
            )
        }.also { error ->
            assertTrue(error.message?.contains("crossSdkFixtureParityBytes sha256") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            verifiedBundle.verifiedArtifacts(
                proofArtifactBytes = proofArtifactBytes,
                provingKeyBytes = provingKeyBytes,
                verifierKeyBytes = verifierKeyBytes,
                sdk = "kotlin",
                implementationBytes = implementationBytes,
                crossSdkFixtureParityBytes = parityFixtureBytes,
                nativeProverSelfTestBytes = "{}".toByteArray(),
            )
        }.also { error ->
            assertTrue(error.message?.contains("nativeProverSelfTestBytes sha256") == true)
        }
        val flaggedArtifactBytes = nativeEvmProverArtifactBytes("proof.wasm kotlin artifact marker")
        val flaggedArtifactHash = sha256Hex(flaggedArtifactBytes)
        val draftFlaggedBundle = SccpEvm.EthereumMainnetNativeEvmProverBundle(
            proofArtifact = "artifacts/eth-mainnet/proof-artifact.bin",
            proofArtifactHash = flaggedArtifactHash,
            provingKey = "artifacts/eth-mainnet/proving-key.bin",
            provingKeyHash = provingKeyHash,
            verifierKey = "artifacts/eth-mainnet/verifier-key.bin",
            verifierKeyHash = verifierKeyHash,
            destinationBindingHash = artifactBinding.hash,
            nativeSdkArtifacts = SccpEvm.ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1
                .entries
                .sortedBy { it.key }
                .mapIndexed { index, entry ->
                    SccpEvm.EthereumMainnetNativeEvmProverBundleSdkArtifact(
                        sdk = entry.key,
                        implementation = entry.value,
                        proofArtifactHash = flaggedArtifactHash,
                        provingKeyHash = provingKeyHash,
                        implementationArtifact = "artifacts/eth-mainnet/${entry.key}-implementation.bin",
                        implementationHash = if (entry.key == "kotlin") {
                            implementationHash
                        } else {
                            "0x" + (index + 1).toString(16).padStart(2, '0').repeat(32)
                        },
                    )
                },
            crossSdkFixtureParityArtifact = "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
            nativeProverSelfTestArtifact = "artifacts/eth-mainnet/native-prover-self-test.json",
            auditHashes = sampleEthereumNativeAuditHashes(),
            expectedDestinationBindingHash = artifactBinding.hash,
        )
        val flaggedParityFixtureBytes = sampleEthereumNativeEvmProverParityFixtureJson(draftFlaggedBundle)
            .toByteArray(Charsets.UTF_8)
        val flaggedSelfTestFixtureBytes = sampleEthereumNativeEvmProverSelfTestFixtureJson(draftFlaggedBundle)
            .toByteArray(Charsets.UTF_8)
        val flaggedBundle = SccpEvm.EthereumMainnetNativeEvmProverBundle(
            proofArtifact = "artifacts/eth-mainnet/proof-artifact.bin",
            proofArtifactHash = flaggedArtifactHash,
            provingKey = "artifacts/eth-mainnet/proving-key.bin",
            provingKeyHash = provingKeyHash,
            verifierKey = "artifacts/eth-mainnet/verifier-key.bin",
            verifierKeyHash = verifierKeyHash,
            destinationBindingHash = artifactBinding.hash,
            nativeSdkArtifacts = draftFlaggedBundle.nativeSdkArtifacts,
            crossSdkFixtureParityArtifact = "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
            nativeProverSelfTestArtifact = "artifacts/eth-mainnet/native-prover-self-test.json",
            auditHashes = sampleEthereumNativeAuditHashes() +
                ("cross_sdk_fixture_parity" to sha256Hex(flaggedParityFixtureBytes)) +
                ("native_prover_self_test" to sha256Hex(flaggedSelfTestFixtureBytes)),
            expectedDestinationBindingHash = artifactBinding.hash,
        )
        assertFailsWith<IllegalArgumentException> {
            flaggedBundle.verifiedArtifacts(
                proofArtifactBytes = flaggedArtifactBytes,
                provingKeyBytes = provingKeyBytes,
                verifierKeyBytes = verifierKeyBytes,
                crossSdkFixtureParityBytes = flaggedParityFixtureBytes,
                nativeProverSelfTestBytes = flaggedSelfTestFixtureBytes,
            )
        }.also { error ->
            assertTrue(error.message?.contains("proofArtifactBytes contains forbidden") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            sampleEthereumNativeEvmProverBundle(binding.hash, noWasm = false)
        }.also { error ->
            assertTrue(error.message?.contains("noWasm") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            sampleEthereumNativeEvmProverBundle(
                "0x" + "95".repeat(32),
                expectedDestinationBindingHash = binding.hash,
            )
        }.also { error ->
            assertTrue(error.message?.contains("destinationBindingHash") == true)
        }

        val result = SccpEthereumMainnet.wrapProofResult(proofBytes, request)
        assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.wrapProofResult(
                proofBytes,
                request.copy(destinationBindingHash = "0x" + "99".repeat(32)),
            )
        }.also { error ->
            assertTrue(error.message?.contains("destinationBindingHash") == true)
        }
        assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp().buildEthereumCalldata(EvmSccpSubmissionInput(result))
        }.also { error ->
            assertTrue(error.message?.contains("verified native EVM prover artifacts") == true)
        }
        val submission = EthereumMainnetSccp(
            nativeProverArtifacts = verifiedArtifacts,
        ).buildEthereumCalldata(EvmSccpSubmissionInput(artifactBoundResult))
        assertEquals(SccpEvm.DOMAIN_ETH, submission.targetDomain)
        assertContentEquals(proofBytes, submission.proofBytes)
        val tamperedEthereumBase64ProofResultError = assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                nativeProverArtifacts = verifiedArtifacts,
            ).buildEthereumCalldata(
                EvmSccpSubmissionInput(
                    artifactBoundResult.copy(proofBase64 = "AAAA"),
                ),
            )
        }
        assertTrue(tamperedEthereumBase64ProofResultError.message?.contains("proofBase64") == true)
        val submitted = EthereumMainnetSccp(
            outboundSubmitter = EthereumMainnetOutboundSubmitter { outboundSubmission ->
                assertEquals(SccpEvm.DOMAIN_ETH, outboundSubmission.targetDomain)
                assertContentEquals(proofBytes, outboundSubmission.proofBytes)
                "eth-submitted"
            },
            nativeProverArtifacts = verifiedArtifacts,
        ).submitOutboundToEthereum(EvmSccpSubmissionInput(artifactBoundResult))
        assertEquals("eth-submitted", submitted)
        var guardedSubmitterCalled = false
        val guarded = EthereumMainnetSccp(
            executionProvider = EthereumMainnetExecutionProvider { method, _ ->
                assertEquals("eth_chainId", method)
                "0x38"
            },
            outboundSubmitter = EthereumMainnetOutboundSubmitter {
                guardedSubmitterCalled = true
                "wrong-chain"
            },
            nativeProverArtifacts = verifiedArtifacts,
        )
        assertFailsWith<IllegalArgumentException> {
            guarded.submitOutboundToEthereum(EvmSccpSubmissionInput(artifactBoundResult))
        }.also { error ->
            assertTrue(error.message!!.contains("eth_chainId == 1"))
        }
        assertFalse(guardedSubmitterCalled)
        assertFailsWith<IllegalStateException> {
            EthereumMainnetSccp(nativeProverArtifacts = verifiedArtifacts)
                .submitOutboundToEthereum(EvmSccpSubmissionInput(artifactBoundResult))
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
                    bundleBytes = sampleBundleBytes(targetDomain = SccpEvm.DOMAIN_BSC),
                    statementHash = "56".repeat(32),
                    destinationBinding = binding,
                ),
            )
        }
        var outboundProverCalled = false
        val guardedProveFacade = EthereumMainnetSccp(
            proofEngine = EvmSccpProofEngine {
                outboundProverCalled = true
                proofBytes
            },
        )
        assertFailsWith<IllegalArgumentException> {
            guardedProveFacade.proveOutboundToEthereum(
                input.copy(publicInputs = samplePublicInputs(targetDomain = SccpEvm.DOMAIN_BSC)),
            )
        }.also { error ->
            assertTrue(error.message?.contains("target ETH") == true)
        }
        assertFalse(
            outboundProverCalled,
            "Ethereum outbound prover callback must not see BSC requests",
        )
        val wrongSourceRequest = assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.buildProofRequest(input.copy(sourceDomain = SccpEvm.DOMAIN_BSC))
        }
        assertTrue(wrongSourceRequest.message?.contains("SORA -> ETH") == true)

        val wrongSourceBinding = assertFailsWith<IllegalArgumentException> {
            SccpEthereumMainnet.buildProofRequest(
                input.copy(destinationBinding = binding.copy(sourceDomain = SccpEvm.DOMAIN_BSC)),
            )
        }
        assertTrue(wrongSourceBinding.message?.contains("destinationBinding") == true)
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
    fun ethereumMainnetBeaconRestConsensusProviderCollectsFinalizedTargetEvidence() {
        val txHash = "0x" + "aa".repeat(32)
        val blockHash = "0x" + "bb".repeat(32)
        val sourceEventDigest = "0x" + "ee".repeat(32)
        val sourceBridgeEmitterAddress = "0x" + "44".repeat(20)
        fun sourceEventLog(overrides: Map<String, Any?> = emptyMap()): Map<String, Any?> =
            mapOf<String, Any?>(
                "address" to sourceBridgeEmitterAddress,
                "transactionHash" to txHash,
                "blockHash" to blockHash,
                "blockNumber" to "0x1234",
                "topics" to listOf(SccpEthereumMainnet.sourceEventTopic(), sourceEventDigest),
                "data" to "0x",
            ) + overrides
        val receipt = mapOf<String, Any?>(
            "transactionHash" to txHash,
            "blockHash" to blockHash,
            "blockNumber" to "0x1234",
            "status" to "0x1",
            "logs" to listOf(sourceEventLog()),
        )
        val block = mapOf<String, Any?>(
            "hash" to blockHash,
            "number" to "0x1234",
            "receiptsRoot" to ("0x" + "cc".repeat(32)),
            "beaconSlot" to "64",
        )
        val calls = mutableListOf<Pair<String, Map<String, String>>>()
        val transport = EthereumMainnetBeaconRestTransport { url, headers ->
            calls.add(url to headers)
            when (url) {
                "https://beacon.example/eth/v1/beacon/headers/finalized" ->
                    EthereumMainnetBeaconRestResponse(200, beaconHeaderJson().toByteArray(Charsets.UTF_8))
                "https://beacon.example/eth/v1/beacon/headers/64" ->
                    EthereumMainnetBeaconRestResponse(
                        200,
                        beaconHeaderJson(slot = "64").toByteArray(Charsets.UTF_8),
                    )
                "https://beacon.example/eth/v1/beacon/blocks/64/root" ->
                    EthereumMainnetBeaconRestResponse(200, beaconBlockRootJson().toByteArray(Charsets.UTF_8))
                "https://beacon.example/eth/v2/beacon/blocks/64" ->
                    EthereumMainnetBeaconRestResponse(200, beaconBlockJson(slot = "64").toByteArray(Charsets.UTF_8))
                "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints" ->
                    EthereumMainnetBeaconRestResponse(200, beaconCheckpointJson().toByteArray(Charsets.UTF_8))
                "https://beacon.example/eth/v1/beacon/light_client/finality_update" ->
                    EthereumMainnetBeaconRestResponse(200, beaconFinalityUpdateJson().toByteArray(Charsets.UTF_8))
                else -> error("unexpected Beacon REST URL $url")
            }
        }
        val provider = EthereumMainnetBeaconRestConsensusProvider(
            endpoint = "https://beacon.example/eth/v1",
            syncCommitteeRoot = "0x" + "ee".repeat(32),
            headers = mapOf("Authorization" to "Bearer local"),
            transport = transport,
        )
        val evidence = EthereumMainnetSccp(consensusProvider = provider)
            .collectInboundEvidenceFromReceipt(EthereumMainnetInboundEvidence(receipt = receipt, block = block))

        assertEquals("4660", evidence.beaconFinality?.get("executionBlockNumber"))
        assertEquals(blockHash, evidence.beaconFinality?.get("executionBlockHash"))
        assertEquals("0x" + "cc".repeat(32), evidence.beaconFinality?.get("executionReceiptsRoot"))
        assertEquals(
            "0xbb44a971e8c280f585ba430bfabfe87d9c59adf38bf9f77266b69687a148048c",
            evidence.beaconFinality?.get("finalizedHeaderRoot"),
        )
        assertEquals("0x" + "ee".repeat(32), evidence.beaconFinality?.get("syncCommitteeRoot"))
        assertEquals("64", evidence.beaconFinality?.get("beaconSlot"))
        assertEquals(ethereumFinalityBranch, evidence.beaconFinality?.get("finalityBranch"))
        assertEquals(ethereumSyncCommitteeSupermajorityBits, evidence.beaconFinality?.get("syncCommitteeBits"))
        assertEquals("0x" + "34".repeat(96), evidence.beaconFinality?.get("syncCommitteeSignature"))
        assertEquals(
            ethereumSyncCommitteeSupermajorityParticipation,
            evidence.beaconFinality?.get("syncCommitteeParticipation"),
        )
        assertEquals("65", evidence.beaconFinality?.get("syncSignatureSlot"))
        assertEquals(
            listOf(
                "https://beacon.example/eth/v1/beacon/headers/finalized",
                "https://beacon.example/eth/v1/beacon/headers/64",
                "https://beacon.example/eth/v1/beacon/blocks/64/root",
                "https://beacon.example/eth/v2/beacon/blocks/64",
                "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints",
                "https://beacon.example/eth/v1/beacon/light_client/finality_update",
            ),
            calls.map { it.first },
        )
        assertEquals("Bearer local", calls.first().second["Authorization"])
    }

    @Test
    fun ethereumMainnetBeaconRestConsensusProviderDerivesTargetSlotFromTimestamp() {
        val txHash = "0x" + "aa".repeat(32)
        val blockHash = "0x" + "bb".repeat(32)
        val sourceEventDigest = "0x" + "ee".repeat(32)
        val sourceBridgeEmitterAddress = "0x" + "44".repeat(20)
        val receipt = mapOf<String, Any?>(
            "transactionHash" to txHash,
            "blockHash" to blockHash,
            "blockNumber" to "0x1234",
            "status" to "0x1",
            "logs" to listOf(
                mapOf<String, Any?>(
                    "address" to sourceBridgeEmitterAddress,
                    "transactionHash" to txHash,
                    "blockHash" to blockHash,
                    "blockNumber" to "0x1234",
                    "topics" to listOf(SccpEthereumMainnet.sourceEventTopic(), sourceEventDigest),
                    "data" to "0x",
                ),
            ),
        )
        val block = mapOf<String, Any?>(
            "hash" to blockHash,
            "number" to "0x1234",
            "receiptsRoot" to ("0x" + "cc".repeat(32)),
            "timestamp" to "0x364",
        )
        val calls = mutableListOf<String>()
        val transport = EthereumMainnetBeaconRestTransport { url, _ ->
            calls.add(url)
            when (url) {
                "https://beacon.example/eth/v1/beacon/genesis" ->
                    EthereumMainnetBeaconRestResponse(200, beaconGenesisJson("100").toByteArray(Charsets.UTF_8))
                "https://beacon.example/eth/v1/beacon/headers/finalized" ->
                    EthereumMainnetBeaconRestResponse(200, beaconHeaderJson().toByteArray(Charsets.UTF_8))
                "https://beacon.example/eth/v1/beacon/headers/64" ->
                    EthereumMainnetBeaconRestResponse(
                        200,
                        beaconHeaderJson(slot = "64").toByteArray(Charsets.UTF_8),
                    )
                "https://beacon.example/eth/v1/beacon/blocks/64/root" ->
                    EthereumMainnetBeaconRestResponse(200, beaconBlockRootJson().toByteArray(Charsets.UTF_8))
                "https://beacon.example/eth/v2/beacon/blocks/64" ->
                    EthereumMainnetBeaconRestResponse(200, beaconBlockJson(slot = "64").toByteArray(Charsets.UTF_8))
                "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints" ->
                    EthereumMainnetBeaconRestResponse(200, beaconCheckpointJson().toByteArray(Charsets.UTF_8))
                "https://beacon.example/eth/v1/beacon/light_client/finality_update" ->
                    EthereumMainnetBeaconRestResponse(200, beaconFinalityUpdateJson().toByteArray(Charsets.UTF_8))
                else -> error("unexpected Beacon REST URL $url")
            }
        }
        val provider = EthereumMainnetBeaconRestConsensusProvider(
            endpoint = "https://beacon.example/eth/v1",
            syncCommitteeRoot = "0x" + "ee".repeat(32),
            transport = transport,
        )

        val evidence = EthereumMainnetSccp(consensusProvider = provider)
            .collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt,
                    block = block,
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                ),
            )

        assertEquals(
            "0xbb44a971e8c280f585ba430bfabfe87d9c59adf38bf9f77266b69687a148048c",
            evidence.beaconFinality?.get("finalizedHeaderRoot"),
        )
        assertEquals("64", evidence.beaconFinality?.get("beaconSlot"))
        assertEquals(
            listOf(
                "https://beacon.example/eth/v1/beacon/genesis",
                "https://beacon.example/eth/v1/beacon/headers/finalized",
                "https://beacon.example/eth/v1/beacon/headers/64",
                "https://beacon.example/eth/v1/beacon/blocks/64/root",
                "https://beacon.example/eth/v2/beacon/blocks/64",
                "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints",
                "https://beacon.example/eth/v1/beacon/light_client/finality_update",
            ),
            calls,
        )
    }

    @Test
    fun ethereumMainnetBeaconRestHttpTransportRejectsOversizedBodies() {
        val server = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        server.createContext("/oversized") { exchange ->
            val body = ByteArray(1024 * 1024 + 1) { 0x7b }
            exchange.sendResponseHeaders(200, body.size.toLong())
            exchange.responseBody.use { it.write(body) }
        }
        server.start()
        try {
            val error = assertFailsWith<IllegalArgumentException> {
                EthereumMainnetBeaconRestHttpTransport.get(
                    "http://127.0.0.1:${server.address.port}/oversized",
                    emptyMap(),
                )
            }
            assertTrue(error.message?.contains("response body must be at most") == true)
        } finally {
            server.stop(0)
        }
    }

    @Test
    fun ethereumMainnetBeaconRestConsensusProviderRejectsUnsafeFinality() {
        val block = mapOf<String, Any?>(
            "hash" to ("0x" + "bb".repeat(32)),
            "number" to "0x1234",
            "receiptsRoot" to ("0x" + "cc".repeat(32)),
        )
        fun provider(
            header: EthereumMainnetBeaconRestResponse =
                EthereumMainnetBeaconRestResponse(200, beaconHeaderJson().toByteArray(Charsets.UTF_8)),
            finalizedBlockRoot: EthereumMainnetBeaconRestResponse =
                EthereumMainnetBeaconRestResponse(200, beaconBlockRootJson().toByteArray(Charsets.UTF_8)),
            finalizedBlock: EthereumMainnetBeaconRestResponse =
                EthereumMainnetBeaconRestResponse(200, beaconBlockJson().toByteArray(Charsets.UTF_8)),
            checkpoint: EthereumMainnetBeaconRestResponse =
                EthereumMainnetBeaconRestResponse(200, beaconCheckpointJson().toByteArray(Charsets.UTF_8)),
            finalityUpdate: EthereumMainnetBeaconRestResponse =
                EthereumMainnetBeaconRestResponse(200, beaconFinalityUpdateJson().toByteArray(Charsets.UTF_8)),
            syncCommitteeRoot: String? = "0x" + "ee".repeat(32),
            syncCommitteePayload: ByteArray? = null,
        ): EthereumMainnetBeaconRestConsensusProvider =
            EthereumMainnetBeaconRestConsensusProvider(
                endpoint = "https://beacon.example",
                syncCommitteeRoot = syncCommitteeRoot,
                syncCommitteePayload = syncCommitteePayload,
                transport = EthereumMainnetBeaconRestTransport { url, _ ->
                    when {
                        url.endsWith("/eth/v1/beacon/headers/finalized") -> header
                        url.endsWith("/eth/v1/beacon/blocks/finalized/root") -> finalizedBlockRoot
                        url.endsWith("/eth/v2/beacon/blocks/finalized") -> finalizedBlock
                        url.endsWith("/eth/v1/beacon/states/finalized/finality_checkpoints") -> checkpoint
                        url.endsWith("/eth/v1/beacon/light_client/finality_update") -> finalityUpdate
                        else -> error("unexpected Beacon REST URL $url")
                    }
                },
            )

        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider().collectFinalityEvidence(null, null, null)
            }.message?.contains("requires block") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    header = EthereumMainnetBeaconRestResponse(
                        503,
                        "{}".toByteArray(Charsets.UTF_8),
                        "Unavailable",
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("request failed 503 Unavailable") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    header = EthereumMainnetBeaconRestResponse(
                        200,
                        ByteArray(1024 * 1024 + 1),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("response body must be at most") == true,
        )
        val historicalProvider = EthereumMainnetBeaconRestConsensusProvider(
            endpoint = "https://beacon.example/eth/v1",
            syncCommitteeRoot = "0x" + "ee".repeat(32),
            transport = EthereumMainnetBeaconRestTransport { url, _ ->
                when (url) {
                    "https://beacon.example/eth/v1/beacon/headers/finalized" ->
                        EthereumMainnetBeaconRestResponse(200, beaconHeaderJson().toByteArray(Charsets.UTF_8))
                    "https://beacon.example/eth/v1/beacon/headers/32" ->
                        EthereumMainnetBeaconRestResponse(
                            200,
                            beaconHeaderJson(rootByte = "aa", slot = "32").toByteArray(Charsets.UTF_8),
                        )
                    else -> error("unexpected Beacon REST URL $url")
                }
            },
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                historicalProvider.collectFinalityEvidence(
                    null,
                    block + ("beaconSlot" to "32"),
                    null,
                )
            }.message?.contains("historical target blocks require an ancestry proof") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    header = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconHeaderJson(executionOptimistic = true).toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("must not be execution optimistic") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    header = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconHeaderJson()
                            .replace("\"execution_optimistic\": false", "\"execution_optimistic\": \"false\"")
                            .toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("execution_optimistic must be a boolean") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    header = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconHeaderJson()
                            .replace("\"finalized\": true", "\"finalized\": \"true\"")
                            .toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("finalized must be a boolean") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    header = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconHeaderJson()
                            .replace("\"canonical\": true", "\"canonical\": \"true\"")
                            .toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("canonical must be a boolean") == true,
        )
        for ((field, bytePair) in listOf(
            "parent_root" to "01",
            "state_root" to "02",
            "body_root" to "03",
        )) {
            assertTrue(
                assertFailsWith<IllegalArgumentException> {
                    provider(
                        header = EthereumMainnetBeaconRestResponse(
                            200,
                            beaconHeaderJson()
                                .replace(
                                    "\"$field\": \"0x${bytePair.repeat(32)}\"",
                                    "\"$field\": \"0x\"",
                                )
                                .toByteArray(Charsets.UTF_8),
                        ),
                    ).collectFinalityEvidence(null, block, null)
                }.message?.contains(field) == true,
            )
        }
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    header = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconHeaderJson()
                            .replace(
                                "\"signature\": \"0x${"12".repeat(96)}\"",
                                "\"signature\": \"0x${"12".repeat(95)}\"",
                            )
                            .toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("signature") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    finalizedBlockRoot = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconBlockRootJson(rootByte = "99").toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("finalized block root must match finalized header root") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    finalizedBlock = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconBlockJson(slot = "65").toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("finalized block slot must match finalized header slot") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    finalizedBlock = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconBlockJson(blockHash = "0x" + "99".repeat(32)).toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("execution payload block_hash must match block.hash") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    finalizedBlock = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconBlockJson(blockNumber = "4661").toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("execution payload block_number must match block.number") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    finalizedBlock = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconBlockJson(receiptsRoot = "0x" + "99".repeat(32)).toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("execution payload receipts_root must match block.receiptsRoot") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    header = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconHeaderJson(finalized = false).toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("must be finalized") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    checkpoint = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconCheckpointJson(rootByte = "99").toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("checkpoint root must match") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    finalityUpdate = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconFinalityUpdateJson(syncCommitteeBits = "0x" + "00".repeat(64))
                            .toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("sync_committee_bits must contain at least one participant") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    finalityUpdate = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconFinalityUpdateJson(syncCommitteeBits = "0x01" + "00".repeat(63))
                            .toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("sync_committee_bits must contain Ethereum sync committee supermajority") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    finalityUpdate = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconFinalityUpdateJson(includeFinalityBranch = false)
                            .toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("finality_branch") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    finalityUpdate = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconFinalityUpdateJson(finalityBranch = ethereumFinalityBranch.take(5))
                            .toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("finality_branch") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(
                    finalityUpdate = EthereumMainnetBeaconRestResponse(
                        200,
                        beaconFinalityUpdateJson(syncCommitteeSignature = "0x" + "00".repeat(96))
                            .toByteArray(Charsets.UTF_8),
                    ),
                ).collectFinalityEvidence(null, block, null)
            }.message?.contains("sync_committee_signature must not be zero") == true,
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(syncCommitteeRoot = null).collectFinalityEvidence(null, block, null)
            }.message?.contains("requires syncCommitteeRoot or syncCommitteePayload") == true,
        )
        val syncCommitteePayload = SccpSourceProofs.canonicalEthSyncCommitteePayloadBytes(
            syncCommitteePublicKeys = List(512) { index ->
                indexedSyncCommitteeBytes(0x11, 48, index)
            },
            syncCommitteeWeights = List(512) { "1" },
            syncCommitteePops = List(512) { index ->
                indexedSyncCommitteeBytes(0x22, 96, index)
            },
        )
        assertTrue(
            assertFailsWith<IllegalArgumentException> {
                provider(syncCommitteePayload = syncCommitteePayload)
                    .collectFinalityEvidence(null, block, null)
            }.message?.contains("syncCommitteeRoot must match syncCommitteePayload") == true,
        )
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
            "transactionHash" to txHash,
            "blockHash" to blockHash,
            "blockNumber" to "0x1234",
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
            additionalFields = mapOf(
                "finalizedHeaderRoot" to ("0x" + "dd".repeat(32)),
                "syncCommitteeRoot" to ("0x" + "aa".repeat(32)),
                "finalityBranch" to ethereumFinalityBranch,
            ),
            beaconSlot = "0x20",
            syncCommitteeBits = ethereumSyncCommitteeSupermajorityBits,
            syncCommitteeSignature = "0x" + "34".repeat(96),
            syncCommitteeParticipation = ethereumSyncCommitteeSupermajorityParticipation,
            syncSignatureSlot = "65",
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
                assertEquals("0x" + "dd".repeat(32), evidence.beaconFinality?.get("finalizedHeaderRoot"))
                assertEquals("0x" + "aa".repeat(32), evidence.beaconFinality?.get("syncCommitteeRoot"))
                assertEquals("32", evidence.beaconFinality?.get("beaconSlot"))
                assertEquals(receiptProofHash, evidence.receiptProofHash)
                assertEquals(receiptProof.sourceEventDigest, evidence.receiptProof?.sourceEventDigest)
                assertEquals(sourceEventDigest, evidence.sourceEventDigest)
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
        assertEquals("0x" + "dd".repeat(32), evidence.beaconFinality?.get("finalizedHeaderRoot"))
        assertEquals("0x" + "aa".repeat(32), evidence.beaconFinality?.get("syncCommitteeRoot"))
        assertEquals("32", evidence.beaconFinality?.get("beaconSlot"))
        assertEquals(1, consensusCalls)
        assertEquals(listOf("eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"), calls)
        val unanchoredProofEvidence = evidence.copy(receiptProof = receiptProof, receiptProofHash = receiptProofHash)
        val unanchoredProof = assertFailsWith<IllegalArgumentException> {
            sdk.proveInboundToSora(unanchoredProofEvidence)
        }
        assertTrue(unanchoredProof.message?.contains("receipt source event validation") == true)

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
        val configuredSourceEventEvidence = EthereumMainnetSccp(
            sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
        ).collectInboundEvidenceFromReceipt(
            EthereumMainnetInboundEvidence(
                receipt = receiptWithSourceEvent,
                block = block,
                beaconFinality = beaconFinality,
            ),
        )
        assertEquals(sourceEventDigest, configuredSourceEventEvidence.sourceEventDigest)
        assertEquals(sourceBridgeEmitterAddress, configuredSourceEventEvidence.sourceBridgeEmitterAddress)
        assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            ).collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receiptWithSourceEvent,
                    block = block,
                    beaconFinality = beaconFinality,
                    sourceBridgeEmitterAddress = "0x" + "13".repeat(20),
                ),
            )
        }

        val proofReadyEvidence = sourceEventEvidence.copy(receiptProof = receiptProof, receiptProofHash = receiptProofHash)
        val missingFinalityBranch = assertFailsWith<IllegalArgumentException> {
            sdk.proveInboundToSora(
                proofReadyEvidence.copy(
                    beaconFinality = proofReadyEvidence.beaconFinality?.minus("finalityBranch"),
                ),
            )
        }
        assertTrue(missingFinalityBranch.message?.contains("beaconFinality.finalityBranch") == true)
        val missingSyncBits = assertFailsWith<IllegalArgumentException> {
            sdk.proveInboundToSora(
                proofReadyEvidence.copy(
                    beaconFinality = proofReadyEvidence.beaconFinality?.minus("syncCommitteeBits"),
                ),
            )
        }
        assertTrue(missingSyncBits.message?.contains("beaconFinality.syncCommitteeBits") == true)
        val conflictingSyncBits = assertFailsWith<IllegalArgumentException> {
            sdk.proveInboundToSora(
                proofReadyEvidence.copy(
                    beaconFinality = proofReadyEvidence.beaconFinality?.plus(
                        "sync_committee_bits" to ("0x02" + "00".repeat(63)),
                    ),
                ),
            )
        }
        assertTrue(conflictingSyncBits.message?.contains("beaconFinality.syncCommitteeBits") == true)
        val mismatchedSyncParticipation = assertFailsWith<IllegalArgumentException> {
            sdk.proveInboundToSora(
                proofReadyEvidence.copy(
                    beaconFinality = proofReadyEvidence.beaconFinality?.plus(
                        "syncCommitteeParticipation" to "341",
                    ),
                ),
            )
        }
        assertTrue(
            mismatchedSyncParticipation.message?.contains("beaconFinality.syncCommitteeParticipation") == true,
        )
        val underQuorumSyncBits = assertFailsWith<IllegalArgumentException> {
            sdk.proveInboundToSora(
                proofReadyEvidence.copy(
                    beaconFinality = proofReadyEvidence.beaconFinality?.plus(
                        mapOf(
                            "syncCommitteeBits" to ("0x01" + "00".repeat(63)),
                            "syncCommitteeParticipation" to "1",
                        ),
                    ),
                ),
            )
        }
        assertTrue(underQuorumSyncBits.message?.contains("beaconFinality.syncCommitteeBits") == true)
        val staleSyncSignatureSlot = assertFailsWith<IllegalArgumentException> {
            sdk.proveInboundToSora(
                proofReadyEvidence.copy(
                    beaconFinality = proofReadyEvidence.beaconFinality?.plus(
                        "syncSignatureSlot" to "31",
                    ),
                ),
            )
        }
        assertTrue(staleSyncSignatureSlot.message?.contains("beaconFinality.syncSignatureSlot") == true)
        val zeroSyncCommitteeSignature = assertFailsWith<IllegalArgumentException> {
            sdk.proveInboundToSora(
                proofReadyEvidence.copy(
                    beaconFinality = proofReadyEvidence.beaconFinality?.plus(
                        "syncCommitteeSignature" to ("0x" + "00".repeat(96)),
                    ),
                ),
            )
        }
        assertTrue(zeroSyncCommitteeSignature.message?.contains("beaconFinality.syncCommitteeSignature") == true)
        val aliasOnlyFinality = mapOf<String, Any?>(
            "execution_block_number" to "0x1234",
            "finality_block_hash" to blockHash,
            "receipts_root" to ("0x" + "cc".repeat(32)),
            "finalized_header_root" to ("0x" + "dd".repeat(32)),
            "sync_committee_root" to ("0x" + "aa".repeat(32)),
            "beacon_slot" to "0x20",
            "finality_branch" to ethereumFinalityBranch,
            "sync_committee_bits" to ethereumSyncCommitteeSupermajorityBits,
            "sync_committee_signature" to ("0x" + "34".repeat(96)),
            "sync_committee_participation" to ethereumSyncCommitteeSupermajorityParticipation,
            "signature_slot" to "65",
            "extensionWitness" to "kept",
        )
        val aliasOnlyProof = EthereumMainnetSccp(
            inboundProver = EthereumMainnetInboundProver { aliasEvidence ->
                val finality = aliasEvidence.beaconFinality ?: error("beaconFinality required")
                assertEquals("4660", finality["executionBlockNumber"])
                assertEquals(blockHash, finality["executionBlockHash"])
                assertEquals("0x" + "cc".repeat(32), finality["executionReceiptsRoot"])
                assertEquals("0x" + "dd".repeat(32), finality["finalizedHeaderRoot"])
                assertEquals("0x" + "aa".repeat(32), finality["syncCommitteeRoot"])
                assertEquals("32", finality["beaconSlot"])
                assertEquals(ethereumFinalityBranch, finality["finalityBranch"])
                assertEquals(ethereumSyncCommitteeSupermajorityBits, finality["syncCommitteeBits"])
                assertEquals("0x" + "34".repeat(96), finality["syncCommitteeSignature"])
                assertEquals(ethereumSyncCommitteeSupermajorityParticipation, finality["syncCommitteeParticipation"])
                assertEquals("65", finality["syncSignatureSlot"])
                assertEquals("kept", finality["extensionWitness"])
                for (alias in listOf(
                    "execution_block_number",
                    "finalityHeight",
                    "finality_block_hash",
                    "receipts_root",
                    "finalized_header_root",
                    "sync_committee_root",
                    "beacon_slot",
                    "sync_committee_bits",
                    "sync_committee_signature",
                    "sync_committee_participation",
                    "signature_slot",
                )) {
                    assertTrue(alias !in finality)
                }
                byteArrayOf(4, 5, 6)
            },
        ).proveInboundToSora(
            EthereumMainnetInboundEvidence(
                transactionHash = evidence.transactionHash,
                receipt = receiptWithSourceEvent,
                block = evidence.block,
                beaconFinality = aliasOnlyFinality,
                receiptProof = receiptProof,
                receiptProofHash = receiptProofHash,
                sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            ),
        )
        assertContentEquals(byteArrayOf(4, 5, 6), aliasOnlyProof)
        for ((alias, value, label) in listOf(
            Triple("finalized_header_root", "0x" + "13".repeat(32), "beaconFinality.finalizedHeaderRoot"),
            Triple("sync_committee_root", "0x" + "14".repeat(32), "beaconFinality.syncCommitteeRoot"),
            Triple("beacon_slot", "33", "beaconFinality.beaconSlot"),
        )) {
            val conflictingFinalityAlias = assertFailsWith<IllegalArgumentException> {
                sdk.proveInboundToSora(
                    proofReadyEvidence.copy(
                        beaconFinality = proofReadyEvidence.beaconFinality?.plus(alias to value),
                    ),
                )
            }
            assertTrue(conflictingFinalityAlias.message?.contains(label) == true)
        }
        assertContentEquals(byteArrayOf(1, 2, 3), sdk.proveInboundToSora(proofReadyEvidence))
        val emptyProof = assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                inboundProver = EthereumMainnetInboundProver { byteArrayOf() },
            ).proveInboundToSora(proofReadyEvidence)
        }
        assertTrue(emptyProof.message?.contains("proofBytes must not be empty") == true)
        val zeroProof = assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                inboundProver = EthereumMainnetInboundProver { byteArrayOf(0, 0) },
            ).proveInboundToSora(proofReadyEvidence)
        }
        assertTrue(zeroProof.message?.contains("proofBytes must not be all zero") == true)
        val oversizedInboundProof = ByteArray(SccpEvm.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1) { 1 }
        val oversizedProof = assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                inboundProver = EthereumMainnetInboundProver { oversizedInboundProof },
            ).proveInboundToSora(proofReadyEvidence)
        }
        assertTrue(oversizedProof.message?.contains("proofBytes must be at most") == true)
        val oversizedSubmit = assertFailsWith<IllegalArgumentException> {
            sdk.submitInboundToIroha(oversizedInboundProof)
        }
        assertTrue(oversizedSubmit.message?.contains("proofBytes must be at most") == true)
        assertEquals("submitted", sdk.submitInboundToIroha(byteArrayOf(1, 2, 3)))

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
        val receiptProofHashOnlyEvidence = EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
            EthereumMainnetInboundEvidence(receiptProofHash = receiptProofHash),
        )
        assertEquals(receiptProofHash, receiptProofHashOnlyEvidence.receiptProofHash)
        assertNull(receiptProofHashOnlyEvidence.receiptProof)
        val zeroReceiptProofHash = assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receiptProofHash = "0x" + "00".repeat(32)),
            )
        }
        assertTrue(zeroReceiptProofHash.message?.contains("receiptProofHash must not be zero") == true)
        val noncanonicalReceiptProofHash = assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receiptProofHash = receiptProofHash + " "),
            )
        }
        assertTrue(noncanonicalReceiptProofHash.message?.contains("receiptProofHash must be canonical") == true)
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

        var prebuiltProofOnlyProverCalls = 0
        val prebuiltProofWithoutSourceEvent = assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                inboundProver = EthereumMainnetInboundProver {
                    prebuiltProofOnlyProverCalls += 1
                    byteArrayOf(1, 2, 3)
                },
            ).proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    beaconFinality = beaconFinality,
                    receiptProof = receiptProof,
                    receiptProofHash = receiptProofHash,
                ),
            )
        }
        assertTrue(prebuiltProofWithoutSourceEvent.message?.contains("receipt source event validation") == true)
        assertEquals(0, prebuiltProofOnlyProverCalls)

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
                    receipt = receiptWithSourceEvent,
                    block = block,
                    beaconFinalityEvidence = beaconFinalityEvidence,
                    receiptProof = receiptProof,
                    receiptProofHash = receiptProofHash,
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
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
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                ),
                provider = EthereumMainnetExecutionProvider { method, _ ->
                    perCallProviderCalls.add(method)
                    when (method) {
                        "eth_chainId" -> "0x1"
                        "eth_getTransactionReceipt" -> receiptWithSourceEvent
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

        val missingFinalizedRoot = assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                inboundProver = EthereumMainnetInboundProver {
                    proverCalls += 1
                    byteArrayOf(1, 2, 3)
                },
            ).proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    receipt = receiptWithSourceEvent,
                    block = block,
                    beaconFinality = beaconFinality - "finalizedHeaderRoot",
                    receiptProof = receiptProof,
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                ),
            )
        }
        assertTrue(missingFinalizedRoot.message?.contains("beaconFinality.finalizedHeaderRoot") == true)
        assertEquals(0, proverCalls)

        val missingSyncCommitteeRoot = assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                inboundProver = EthereumMainnetInboundProver {
                    proverCalls += 1
                    byteArrayOf(1, 2, 3)
                },
            ).proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    receipt = receiptWithSourceEvent,
                    block = block,
                    beaconFinality = beaconFinality - "syncCommitteeRoot",
                    receiptProof = receiptProof,
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                ),
            )
        }
        assertTrue(missingSyncCommitteeRoot.message?.contains("beaconFinality.syncCommitteeRoot") == true)
        assertEquals(0, proverCalls)

        val missingBeaconSlot = assertFailsWith<IllegalArgumentException> {
            EthereumMainnetSccp(
                inboundProver = EthereumMainnetInboundProver {
                    proverCalls += 1
                    byteArrayOf(1, 2, 3)
                },
            ).proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    receipt = receiptWithSourceEvent,
                    block = block,
                    beaconFinality = beaconFinality - "beaconSlot",
                    receiptProof = receiptProof,
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                ),
            )
        }
        assertTrue(missingBeaconSlot.message?.contains("beaconFinality.beaconSlot") == true)
        assertEquals(0, proverCalls)

        val driftedFinalizedRootProof = assertFailsWith<IllegalArgumentException> {
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
                    receiptProof = receiptProof.copy(beaconFinalizedRoot = "0x" + "99".repeat(32)),
                ),
            )
        }
        assertTrue(driftedFinalizedRootProof.message?.contains("receiptProof.beaconFinalizedRoot") == true)
        assertEquals(0, proverCalls)

        val driftedSyncCommitteeRootProof = assertFailsWith<IllegalArgumentException> {
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
                    receiptProof = receiptProof.copy(syncCommitteeRoot = "0x" + "99".repeat(32)),
                ),
            )
        }
        assertTrue(driftedSyncCommitteeRootProof.message?.contains("receiptProof.syncCommitteeRoot") == true)
        assertEquals(0, proverCalls)

        val driftedBeaconSlotProof = assertFailsWith<IllegalArgumentException> {
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
                    receiptProof = receiptProof.copy(beaconSlot = "33"),
                ),
            )
        }
        assertTrue(driftedBeaconSlotProof.message?.contains("receiptProof.beaconSlot") == true)
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
        for (chainId in listOf<Any>("1", "0x01", "0X1", " 0x1", "0x1 ", 1L)) {
            assertFailsWith<IllegalArgumentException> {
                EthereumMainnetSccp(
                    executionProvider = EthereumMainnetExecutionProvider { _, _ -> chainId },
                ).collectInboundEvidenceFromReceipt(EthereumMainnetInboundEvidence(receipt = receipt))
            }
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
                    receipt = receipt + (
                        "logs" to listOf(
                            sourceEventLog + (
                                "topics" to listOf(
                                    SccpEthereumMainnet.sourceEventTopic(),
                                    sourceEventDigest,
                                    "0x" + "66".repeat(32),
                                )
                            ),
                        )
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
                    receipt = receipt + ("logs" to listOf(sourceEventLog + ("data" to "0x01"))),
                    block = block,
                    beaconFinality = beaconFinality,
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt + (
                        "logs" to listOf(
                            sourceEventLog + (
                                "topics" to listOf(
                                    SccpEthereumMainnet.sourceEventTopic(),
                                    "0x" + "00".repeat(32),
                                )
                            ),
                        )
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
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt + ("logs" to listOf("not-a-log")),
                    block = block,
                    beaconFinality = beaconFinality,
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt + ("logs" to listOf(sourceEventLog - "data")),
                    block = block,
                    beaconFinality = beaconFinality,
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                ),
            )
        }
        for (missingField in listOf("transactionHash", "blockHash", "blockNumber")) {
            assertFailsWith<IllegalArgumentException> {
                sdk.collectInboundEvidenceFromReceipt(
                    EthereumMainnetInboundEvidence(
                        receipt = receipt + ("logs" to listOf(sourceEventLog - missingField)),
                        block = block,
                        beaconFinality = beaconFinality,
                        sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                    ),
                )
            }
        }
        for ((alias, value, label) in listOf(
            Triple("transaction_hash", "0x" + "ab".repeat(32), "receipt.logs[0].transactionHash"),
            Triple("block_hash", "0x" + "ac".repeat(32), "receipt.logs[0].blockHash"),
            Triple("block_number", "0x1235", "receipt.logs[0].blockNumber"),
        )) {
            val conflictingLogAlias = assertFailsWith<IllegalArgumentException> {
                sdk.collectInboundEvidenceFromReceipt(
                    EthereumMainnetInboundEvidence(
                        receipt = receipt + ("logs" to listOf(sourceEventLog + (alias to value))),
                        block = block,
                        beaconFinality = beaconFinality,
                        sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                    ),
                )
            }
            assertTrue(conflictingLogAlias.message?.contains(label) == true)
        }
        assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt + (
                        "logs" to listOf(
                            sourceEventLog + ("transactionHash" to ("0x" + "ab".repeat(32))),
                        )
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
                    receipt = receipt + (
                        "logs" to listOf(
                            sourceEventLog + ("blockHash" to ("0x" + "ab".repeat(32))),
                        )
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
                    receipt = receipt + ("logs" to listOf(sourceEventLog + ("blockNumber" to "0x1235"))),
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
    fun ethereumMainnetInboundProverReceivesCallbackEvidenceSnapshot() {
        val txHash = "0x" + "aa".repeat(32)
        val blockHash = "0x" + "bb".repeat(32)
        val sourceEventDigest = "0x" + "ee".repeat(32)
        val sourceBridgeEmitterAddress = "0x" + "44".repeat(20)
        val receiptsRoot = "0x" + "cc".repeat(32)
        val finalizedRoot = "0x" + "dd".repeat(32)
        val syncCommitteeRoot = "0x" + "aa".repeat(32)
        val receiptNested = linkedMapOf<String, Any?>(
            "value" to "keep",
            "bytes" to byteArrayOf(0xbb.toByte()),
        )
        val receiptWitness = mutableListOf<Any?>(receiptNested)
        val blockWitness = linkedMapOf<String, Any?>(
            "value" to "block",
            "bytes" to byteArrayOf(0xcc.toByte()),
        )
        val finalityBranchWitness = ethereumFinalityBranch.toMutableList()
        val finalityBytes = byteArrayOf(0xaa.toByte())
        val finalityWitness = linkedMapOf<String, Any?>(
            "branch" to finalityBranchWitness,
            "bytes" to finalityBytes,
        )
        val blockReceiptsWitness = mutableListOf<Any?>("receipt-list")
        val sourceEventLog = mapOf<String, Any?>(
            "address" to sourceBridgeEmitterAddress,
            "transactionHash" to txHash,
            "blockHash" to blockHash,
            "blockNumber" to "0x1234",
            "topics" to listOf(SccpEthereumMainnet.sourceEventTopic(), sourceEventDigest),
            "data" to "0x",
        )
        val receipt = linkedMapOf<String, Any?>(
            "transactionHash" to txHash,
            "blockHash" to blockHash,
            "blockNumber" to "0x1234",
            "status" to "0x1",
            "logs" to listOf(sourceEventLog),
            "mutableWitness" to receiptWitness,
        )
        val block = linkedMapOf<String, Any?>(
            "hash" to blockHash,
            "number" to "0x1234",
            "receiptsRoot" to receiptsRoot,
            "mutableWitness" to blockWitness,
        )
        val beaconFinality = linkedMapOf<String, Any?>(
            "executionBlockNumber" to "0x1234",
            "executionBlockHash" to blockHash,
            "executionReceiptsRoot" to receiptsRoot,
            "finalizedHeaderRoot" to finalizedRoot,
            "syncCommitteeRoot" to syncCommitteeRoot,
            "beaconSlot" to "0x20",
            "finalityBranch" to ethereumFinalityBranch,
            "syncCommitteeBits" to ethereumSyncCommitteeSupermajorityBits,
            "syncCommitteeSignature" to ("0x" + "34".repeat(96)),
            "syncCommitteeParticipation" to ethereumSyncCommitteeSupermajorityParticipation,
            "syncSignatureSlot" to "65",
            "mutableWitness" to finalityWitness,
        )
        val blockReceipt = LinkedHashMap(receipt)
        blockReceipt["mutableWitness"] = blockReceiptsWitness
        val mutableReceiptProofNode = byteArrayOf(0x01, 0x02)
        val mutableReceiptProofBranch = ByteArray(32) { 0x11 }
        val mutableInputBranch = byteArrayOf(0x44)
        val receiptProof = EthereumMainnetReceiptProof(
            sourceEventDigest = sourceEventDigest,
            beaconSlot = "32",
            executionBlockNumber = "4660",
            executionBlockHash = blockHash,
            executionReceiptsRoot = receiptsRoot,
            beaconFinalizedRoot = finalizedRoot,
            syncCommitteeRoot = syncCommitteeRoot,
            receiptRootIndex = "0",
            receiptTrieProofNodes = listOf(mutableReceiptProofNode),
            inclusionBranch = listOf(mutableReceiptProofBranch),
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

        val proofBytes = EthereumMainnetSccp(
            inboundProver = EthereumMainnetInboundProver { evidence ->
                receiptWitness.add("changed")
                receiptNested["value"] = "changed"
                (receiptNested["bytes"] as ByteArray)[0] = 0x7f
                blockWitness["value"] = "changed"
                (blockWitness["bytes"] as ByteArray)[0] = 0x7e
                finalityBranchWitness.add("0x" + "99".repeat(32))
                finalityBytes[0] = 0x7d
                finalityWitness["new"] = "changed"
                blockReceiptsWitness.add("changed")
                mutableReceiptProofNode[0] = 0x7c
                mutableReceiptProofBranch[0] = 0x7b
                mutableInputBranch[0] = 0x45

                @Suppress("UNCHECKED_CAST")
                val receiptSnapshot = evidence.receipt?.get("mutableWitness") as List<Any?>
                assertEquals(1, receiptSnapshot.size)
                @Suppress("UNCHECKED_CAST")
                val receiptNestedSnapshot = receiptSnapshot[0] as Map<String, Any?>
                assertEquals("keep", receiptNestedSnapshot["value"])
                assertContentEquals(byteArrayOf(0xbb.toByte()), receiptNestedSnapshot["bytes"] as ByteArray)

                @Suppress("UNCHECKED_CAST")
                val blockSnapshot = evidence.block?.get("mutableWitness") as Map<String, Any?>
                assertEquals("block", blockSnapshot["value"])
                assertContentEquals(byteArrayOf(0xcc.toByte()), blockSnapshot["bytes"] as ByteArray)

                @Suppress("UNCHECKED_CAST")
                val finalitySnapshot = evidence.beaconFinality?.get("mutableWitness") as Map<String, Any?>
                @Suppress("UNCHECKED_CAST")
                val branchSnapshot = finalitySnapshot["branch"] as List<String>
                assertEquals(ethereumFinalityBranch.size, branchSnapshot.size)
                assertEquals(ethereumFinalityBranch.first(), branchSnapshot.first())
                assertContentEquals(byteArrayOf(0xaa.toByte()), finalitySnapshot["bytes"] as ByteArray)

                val blockReceiptsSnapshot = evidence.blockReceipts ?: error("blockReceipts required")
                @Suppress("UNCHECKED_CAST")
                val blockReceiptWitnessSnapshot = blockReceiptsSnapshot[0]["mutableWitness"] as List<Any?>
                assertEquals(listOf("receipt-list"), blockReceiptWitnessSnapshot)

                assertContentEquals(byteArrayOf(0x44), evidence.inclusionBranch?.get(0))
                assertContentEquals(byteArrayOf(0x01, 0x02), evidence.receiptProof?.receiptTrieProofNodes?.get(0))
                assertContentEquals(ByteArray(32) { 0x11 }, evidence.receiptProof?.inclusionBranch?.get(0))
                assertEquals(receiptProofHash, evidence.receiptProofHash)
                byteArrayOf(9, 8, 7)
            },
        ).proveInboundToSora(
            EthereumMainnetInboundEvidence(
                receipt = receipt,
                block = block,
                beaconFinality = beaconFinality,
                receiptProof = receiptProof,
                receiptProofHash = receiptProofHash,
                sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                blockReceipts = listOf(blockReceipt),
                inclusionBranch = listOf(mutableInputBranch),
            ),
        )

        assertContentEquals(byteArrayOf(9, 8, 7), proofBytes)
    }

    @Test
    fun ethereumMainnetCollectInboundEvidenceSnapshotsConsensusBoundary() {
        val txHash = "0x" + "aa".repeat(32)
        val blockHash = "0x" + "bb".repeat(32)
        val sourceEventDigest = "0x" + "ee".repeat(32)
        val sourceBridgeEmitterAddress = "0x" + "44".repeat(20)
        val receiptsRoot = "0x" + "cc".repeat(32)
        val finalizedRoot = "0x" + "dd".repeat(32)
        val syncCommitteeRoot = "0x" + "aa".repeat(32)
        val receiptNested = linkedMapOf<String, Any?>(
            "value" to "keep",
            "bytes" to byteArrayOf(0xbb.toByte()),
        )
        val receiptWitness = mutableListOf<Any?>(receiptNested)
        val blockWitness = linkedMapOf<String, Any?>(
            "value" to "block",
            "bytes" to byteArrayOf(0xcc.toByte()),
        )
        val finalityBranchWitness = ethereumFinalityBranch.toMutableList()
        val finalityBytes = byteArrayOf(0xaa.toByte())
        val finalityWitness = linkedMapOf<String, Any?>(
            "branch" to finalityBranchWitness,
            "bytes" to finalityBytes,
        )
        val sourceEventLog = mapOf<String, Any?>(
            "address" to sourceBridgeEmitterAddress,
            "transactionHash" to txHash,
            "blockHash" to blockHash,
            "blockNumber" to "0x1234",
            "topics" to listOf(SccpEthereumMainnet.sourceEventTopic(), sourceEventDigest),
            "data" to "0x",
        )
        val receipt = linkedMapOf<String, Any?>(
            "transactionHash" to txHash,
            "blockHash" to blockHash,
            "blockNumber" to "0x1234",
            "status" to "0x1",
            "logs" to listOf(sourceEventLog),
            "mutableWitness" to receiptWitness,
        )
        val block = linkedMapOf<String, Any?>(
            "hash" to blockHash,
            "number" to "0x1234",
            "receiptsRoot" to receiptsRoot,
            "mutableWitness" to blockWitness,
        )
        val beaconFinality = linkedMapOf<String, Any?>(
            "executionBlockNumber" to "0x1234",
            "executionBlockHash" to blockHash,
            "executionReceiptsRoot" to receiptsRoot,
            "finalizedHeaderRoot" to finalizedRoot,
            "syncCommitteeRoot" to syncCommitteeRoot,
            "beaconSlot" to "0x20",
            "finalityBranch" to ethereumFinalityBranch,
            "syncCommitteeBits" to ethereumSyncCommitteeSupermajorityBits,
            "syncCommitteeSignature" to ("0x" + "34".repeat(96)),
            "syncCommitteeParticipation" to ethereumSyncCommitteeSupermajorityParticipation,
            "syncSignatureSlot" to "65",
            "mutableWitness" to finalityWitness,
        )
        var consensusCalls = 0
        val consensusProvider = EthereumMainnetConsensusProvider { collectedReceipt, collectedBlock, collectedTransactionHash ->
            consensusCalls += 1
            assertEquals(txHash, collectedTransactionHash)
            assertFalse(collectedReceipt?.get("mutableWitness") === receiptWitness)
            @Suppress("UNCHECKED_CAST")
            val receiptSnapshot = collectedReceipt?.get("mutableWitness") as List<Any?>
            @Suppress("UNCHECKED_CAST")
            val receiptNestedSnapshot = receiptSnapshot[0] as Map<String, Any?>
            assertEquals("keep", receiptNestedSnapshot["value"])
            assertContentEquals(byteArrayOf(0xbb.toByte()), receiptNestedSnapshot["bytes"] as ByteArray)
            assertFalse(collectedBlock?.get("mutableWitness") === blockWitness)
            @Suppress("UNCHECKED_CAST")
            val blockSnapshot = collectedBlock?.get("mutableWitness") as Map<String, Any?>
            assertEquals("block", blockSnapshot["value"])
            assertContentEquals(byteArrayOf(0xcc.toByte()), blockSnapshot["bytes"] as ByteArray)

            receiptWitness.add("changed")
            receiptNested["value"] = "changed"
            (receiptNested["bytes"] as ByteArray)[0] = 0x7f
            blockWitness["value"] = "changed"
            (blockWitness["bytes"] as ByteArray)[0] = 0x7e
            beaconFinality
        }

        val evidence = EthereumMainnetSccp(
            consensusProvider = consensusProvider,
            sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
        ).collectInboundEvidenceFromReceipt(EthereumMainnetInboundEvidence(receipt = receipt, block = block))
        finalityBranchWitness.add("0x" + "99".repeat(32))
        finalityBytes[0] = 0x7d
        finalityWitness["new"] = "changed"

        assertEquals(1, consensusCalls)
        @Suppress("UNCHECKED_CAST")
        val receiptSnapshot = evidence.receipt?.get("mutableWitness") as List<Any?>
        assertEquals(1, receiptSnapshot.size)
        @Suppress("UNCHECKED_CAST")
        val receiptNestedSnapshot = receiptSnapshot[0] as Map<String, Any?>
        assertEquals("keep", receiptNestedSnapshot["value"])
        assertContentEquals(byteArrayOf(0xbb.toByte()), receiptNestedSnapshot["bytes"] as ByteArray)
        @Suppress("UNCHECKED_CAST")
        val blockSnapshot = evidence.block?.get("mutableWitness") as Map<String, Any?>
        assertEquals("block", blockSnapshot["value"])
        assertContentEquals(byteArrayOf(0xcc.toByte()), blockSnapshot["bytes"] as ByteArray)
        @Suppress("UNCHECKED_CAST")
        val finalitySnapshot = evidence.beaconFinality?.get("mutableWitness") as Map<String, Any?>
        @Suppress("UNCHECKED_CAST")
        val branchSnapshot = finalitySnapshot["branch"] as List<String>
        assertEquals(ethereumFinalityBranch.size, branchSnapshot.size)
        assertEquals(ethereumFinalityBranch.first(), branchSnapshot.first())
        assertContentEquals(byteArrayOf(0xaa.toByte()), finalitySnapshot["bytes"] as ByteArray)
        assertNull(finalitySnapshot["new"])
    }

    @Test
    fun bscMainnetCollectInboundEvidenceSnapshotsConsensusBoundary() {
        val txHash = "0x" + "aa".repeat(32)
        val blockHash = "0x" + "bb".repeat(32)
        val sourceEventDigest = "0x" + "ee".repeat(32)
        val sourceBridgeEmitterAddress = "0x" + "44".repeat(20)
        val receiptsRoot = "0x" + "cc".repeat(32)
        val validatorSetHash = "0x" + "ab".repeat(32)
        val commitSealHash = "0x" + "dd".repeat(32)
        val receiptNested = linkedMapOf<String, Any?>(
            "value" to "keep",
            "bytes" to byteArrayOf(0xbb.toByte()),
        )
        val receiptWitness = mutableListOf<Any?>(receiptNested)
        val blockWitness = linkedMapOf<String, Any?>(
            "value" to "block",
            "bytes" to byteArrayOf(0xcc.toByte()),
        )
        val finalityBranchWitness = mutableListOf<Any?>(validatorSetHash)
        val finalityBytes = byteArrayOf(0xaa.toByte())
        val finalityWitness = linkedMapOf<String, Any?>(
            "branch" to finalityBranchWitness,
            "bytes" to finalityBytes,
        )
        val sourceEventLog = mapOf<String, Any?>(
            "address" to sourceBridgeEmitterAddress,
            "transactionHash" to txHash,
            "blockHash" to blockHash,
            "blockNumber" to "0x1234",
            "topics" to listOf(SccpEthereumMainnet.sourceEventTopic(), sourceEventDigest),
            "data" to "0x",
        )
        val receipt = linkedMapOf<String, Any?>(
            "transactionHash" to txHash,
            "blockHash" to blockHash,
            "blockNumber" to "0x1234",
            "status" to "0x1",
            "logs" to listOf(sourceEventLog),
            "mutableWitness" to receiptWitness,
        )
        val block = linkedMapOf<String, Any?>(
            "hash" to blockHash,
            "number" to "0x1234",
            "receiptsRoot" to receiptsRoot,
            "mutableWitness" to blockWitness,
        )
        val parliaFinality = linkedMapOf<String, Any?>(
            "executionBlockNumber" to "0x1234",
            "executionBlockHash" to blockHash,
            "executionReceiptsRoot" to receiptsRoot,
            "validatorEpoch" to "0x24",
            "validatorSetHash" to validatorSetHash,
            "commitSealHash" to commitSealHash,
            "mutableWitness" to finalityWitness,
        )
        var consensusCalls = 0
        val consensusProvider = BscMainnetConsensusProvider { collectedReceipt, collectedBlock, collectedTransactionHash ->
            consensusCalls += 1
            assertEquals(txHash, collectedTransactionHash)
            assertFalse(collectedReceipt?.get("mutableWitness") === receiptWitness)
            @Suppress("UNCHECKED_CAST")
            val receiptSnapshot = collectedReceipt?.get("mutableWitness") as List<Any?>
            @Suppress("UNCHECKED_CAST")
            val receiptNestedSnapshot = receiptSnapshot[0] as Map<String, Any?>
            assertEquals("keep", receiptNestedSnapshot["value"])
            assertContentEquals(byteArrayOf(0xbb.toByte()), receiptNestedSnapshot["bytes"] as ByteArray)
            assertFalse(collectedBlock?.get("mutableWitness") === blockWitness)
            @Suppress("UNCHECKED_CAST")
            val blockSnapshot = collectedBlock?.get("mutableWitness") as Map<String, Any?>
            assertEquals("block", blockSnapshot["value"])
            assertContentEquals(byteArrayOf(0xcc.toByte()), blockSnapshot["bytes"] as ByteArray)

            receiptWitness.add("changed")
            receiptNested["value"] = "changed"
            (receiptNested["bytes"] as ByteArray)[0] = 0x7f
            blockWitness["value"] = "changed"
            (blockWitness["bytes"] as ByteArray)[0] = 0x7e
            parliaFinality
        }

        val evidence = BscMainnetSccp(
            consensusProvider = consensusProvider,
            sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
        ).collectInboundEvidenceFromReceipt(BscMainnetInboundEvidence(receipt = receipt, block = block))
        finalityBranchWitness.add("0x" + "99".repeat(32))
        finalityBytes[0] = 0x7d
        finalityWitness["new"] = "changed"

        assertEquals(1, consensusCalls)
        @Suppress("UNCHECKED_CAST")
        val receiptSnapshot = evidence.receipt?.get("mutableWitness") as List<Any?>
        assertEquals(1, receiptSnapshot.size)
        @Suppress("UNCHECKED_CAST")
        val receiptNestedSnapshot = receiptSnapshot[0] as Map<String, Any?>
        assertEquals("keep", receiptNestedSnapshot["value"])
        assertContentEquals(byteArrayOf(0xbb.toByte()), receiptNestedSnapshot["bytes"] as ByteArray)
        @Suppress("UNCHECKED_CAST")
        val blockSnapshot = evidence.block?.get("mutableWitness") as Map<String, Any?>
        assertEquals("block", blockSnapshot["value"])
        assertContentEquals(byteArrayOf(0xcc.toByte()), blockSnapshot["bytes"] as ByteArray)
        @Suppress("UNCHECKED_CAST")
        val finalitySnapshot = evidence.parliaFinality?.get("mutableWitness") as Map<String, Any?>
        @Suppress("UNCHECKED_CAST")
        val branchSnapshot = finalitySnapshot["branch"] as List<Any?>
        assertEquals(listOf(validatorSetHash), branchSnapshot)
        assertContentEquals(byteArrayOf(0xaa.toByte()), finalitySnapshot["bytes"] as ByteArray)
        assertNull(finalitySnapshot["new"])
    }

    @Test
    fun ethereumReceiptTrieProofBuilderUsesRlpTransactionIndexKeys() {
        val receipt = sampleEvmReceipt(
            transactionIndex = 0,
            transactionHash = "0x" + "aa".repeat(32),
            blockHash = "0x" + "bb".repeat(32),
            blockNumber = "0x1234",
        )
        val proof = SccpSourceProofs.buildEvmReceiptTrieProofFromReceipts(listOf(receipt), "0x0")

        assertEquals("0x80", SccpSourceProofs.evmReceiptTrieKey("0x0"))
        assertEquals("0x01", SccpSourceProofs.evmReceiptTrieKey("0x1"))
        assertEquals("0x8180", SccpSourceProofs.evmReceiptTrieKey("0x80"))
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.evmReceiptTrieKey("0x01")
        }
        assertEquals("0x80", proof.receiptTrieKey)
        assertEquals("0x" + hexLower(SccpSourceProofs.canonicalEvmReceiptRlp(receipt)), proof.receiptRlp)
        assertTrue(proof.receiptsRoot.matches(Regex("0x[0-9a-f]{64}")))
        assertTrue(proof.receiptTrieProofNodes.isNotEmpty())
        val zeroTopicReceipt = sampleEvmReceipt(
            transactionIndex = 1,
            transactionHash = "0x" + "ab".repeat(32),
            blockHash = "0x" + "bb".repeat(32),
            blockNumber = "0x1234",
        ) + ("logs" to listOf(
            mapOf(
                "address" to "0x" + "12".repeat(20),
                "topics" to listOf("0x" + "00".repeat(32)),
                "data" to "0x",
            ),
        ))
        val zeroTopicProof = SccpSourceProofs.buildEvmReceiptTrieProofFromReceipts(
            listOf(receipt, zeroTopicReceipt),
            "0x0",
        )
        assertEquals(proof.receiptRlp, zeroTopicProof.receiptRlp)
        val zeroAddressReceipt = sampleEvmReceipt(
            transactionIndex = 1,
            transactionHash = "0x" + "ac".repeat(32),
            blockHash = "0x" + "bb".repeat(32),
            blockNumber = "0x1234",
        ) + ("logs" to listOf(
            mapOf(
                "address" to "0x" + "00".repeat(20),
                "topics" to listOf("0x" + "44".repeat(32)),
                "data" to "0x",
            ),
        ))
        val zeroAddressProof = SccpSourceProofs.buildEvmReceiptTrieProofFromReceipts(
            listOf(receipt, zeroAddressReceipt),
            "0x0",
        )
        assertEquals(proof.receiptRlp, zeroAddressProof.receiptRlp)

        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.buildEvmReceiptTrieProofFromReceipts(
                listOf(receipt + ("transactionIndex" to "0x1")),
                "0x0",
            )
        }
        val conflictingIndex = assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.buildEvmReceiptTrieProofFromReceipts(
                listOf(receipt + ("transaction_index" to "0x0")),
                "0x0",
            )
        }
        assertTrue(conflictingIndex.message?.contains("blockReceipts[0].transactionIndex") == true)
        val conflictingHash = assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.buildEvmReceiptTrieProofFromReceipts(
                listOf(receipt + ("transaction_hash" to receipt["transactionHash"])),
                "0x0",
            )
        }
        assertTrue(conflictingHash.message?.contains("blockReceipts[0].transactionHash") == true)
        val conflictingGas = assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.buildEvmReceiptTrieProofFromReceipts(
                listOf(receipt + ("cumulative_gas_used" to "0x5208")),
                "0x0",
            )
        }
        assertTrue(conflictingGas.message?.contains("receipt.cumulativeGasUsed") == true)
        val conflictingBloom = assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.buildEvmReceiptTrieProofFromReceipts(
                listOf(receipt + ("logs_bloom" to ("0x" + "00".repeat(256)))),
                "0x0",
            )
        }
        assertTrue(conflictingBloom.message?.contains("receipt.logsBloom") == true)
        val duplicateHashReceipt = sampleEvmReceipt(
            transactionIndex = 1,
            transactionHash = "0x" + "aa".repeat(32),
            blockHash = "0x" + "bb".repeat(32),
            blockNumber = "0x1234",
        )
        val duplicateHashError = assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.buildEvmReceiptTrieProofFromReceipts(
                listOf(receipt, duplicateHashReceipt),
                "0x0",
            )
        }
        assertTrue(duplicateHashError.message?.contains("transactionHash values must be unique") == true)
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.buildEvmReceiptTrieProofFromReceipts(listOf(receipt), "0x1")
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.buildEvmReceiptTrieProofFromReceipts(emptyList(), "0x0")
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.buildEvmReceiptTrieProofFromReceipts(List(4_097) { receipt }, "0x0")
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEvmReceiptRlp(receipt + ("logsBloom" to "0x" + "AA".repeat(256)))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEvmReceiptRlp(receipt + ("type" to "0x80"))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEvmReceiptRlp(receipt + ("type" to "0x7f"))
        }
        val validReceiptLog = mapOf<String, Any?>(
            "address" to "0x" + "11".repeat(20),
            "topics" to listOf("0x" + "22".repeat(32)),
            "data" to "0x",
        )
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEvmReceiptRlp(
                receipt + ("logs" to listOf(validReceiptLog + ("removed" to true))),
            )
        }
        val tooManyTopicsLog = validReceiptLog + ("topics" to List(5) { "0x" + "22".repeat(32) })
        assertFailsWith<IllegalArgumentException> {
            SccpSourceProofs.canonicalEvmReceiptRlp(
                receipt + ("logs" to listOf(tooManyTopicsLog)),
            )
        }
    }

    @Test
    fun ethereumInboundCollectionBuildsReceiptProofFromBlockReceipts() {
        val txHash = "0x" + "aa".repeat(32)
        val otherTxHash = "0x" + "ab".repeat(32)
        val blockHash = "0x" + "bb".repeat(32)
        val sourceBridgeEmitterAddress = "0x" + "12".repeat(20)
        val sourceEventDigest = "0x" + "ee".repeat(32)
        val sourceEventLog = mapOf<String, Any?>(
            "address" to sourceBridgeEmitterAddress,
            "transactionHash" to txHash,
            "blockHash" to blockHash,
            "blockNumber" to "0x1234",
            "topics" to listOf(SccpEthereumMainnet.sourceEventTopic(), sourceEventDigest),
            "data" to "0x",
        )
        val receipt = sampleEvmReceipt(
            transactionIndex = 0,
            transactionHash = txHash,
            blockHash = blockHash,
            blockNumber = "0x1234",
            logs = listOf(sourceEventLog),
        )
        val otherReceipt = sampleEvmReceipt(
            transactionIndex = 1,
            transactionHash = otherTxHash,
            blockHash = blockHash,
            blockNumber = "0x1234",
        )
        val blockReceipts = listOf(receipt, otherReceipt)
        val trieProof = SccpSourceProofs.buildEvmReceiptTrieProofFromReceipts(blockReceipts, "0x0")
        val block = mapOf<String, Any?>(
            "hash" to blockHash,
            "number" to "0x1234",
            "receiptsRoot" to trieProof.receiptsRoot,
        )
        val beaconFinality = mapOf<String, Any?>(
            "executionBlockNumber" to "0x1234",
            "executionBlockHash" to blockHash,
            "executionReceiptsRoot" to trieProof.receiptsRoot,
            "finalizedHeaderRoot" to ("0x" + "dd".repeat(32)),
            "syncCommitteeRoot" to ("0x" + "cc".repeat(32)),
            "beaconSlot" to "0x20",
        ) + ethereumBeaconFinalityUpdateFields()
        val inclusionBranch = listOf(ByteArray(32) { 0x44 })
        val calls = mutableListOf<String>()
        val sdk = EthereumMainnetSccp(
            executionProvider = EthereumMainnetExecutionProvider { method, params ->
                calls.add(method)
                when (method) {
                    "eth_chainId" -> "0x1"
                    "eth_getBlockReceipts" -> {
                        assertEquals(listOf("0x1234"), params)
                        blockReceipts
                    }
                    else -> throw IllegalArgumentException("unexpected method $method")
                }
            },
        )

        val evidence = sdk.collectInboundEvidenceFromReceipt(
            EthereumMainnetInboundEvidence(
                receipt = receipt,
                block = block,
                beaconFinality = beaconFinality,
                sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                inclusionBranch = inclusionBranch,
            ),
        )

        assertEquals(listOf("eth_chainId", "eth_getBlockReceipts"), calls)
        assertEquals(sourceEventDigest, evidence.sourceEventDigest)
        assertEquals(blockReceipts, evidence.blockReceipts)
        val receiptProof = evidence.receiptProof ?: throw AssertionError("receiptProof must be auto-built")
        assertEquals(SccpEvm.DOMAIN_ETH, receiptProof.sourceDomain)
        assertEquals("0", receiptProof.receiptRootIndex)
        assertEquals("32", receiptProof.beaconSlot)
        assertEquals("4660", receiptProof.executionBlockNumber)
        assertEquals(trieProof.receiptsRoot, receiptProof.executionReceiptsRoot)
        assertEquals(trieProof.receiptTrieProofNodes.size, receiptProof.receiptTrieProofNodes.size)
        assertContentEquals(trieProof.receiptTrieProofNodes[0], receiptProof.receiptTrieProofNodes[0])
        assertContentEquals(inclusionBranch[0], receiptProof.inclusionBranch[0])
        assertEquals(
            SccpSourceProofs.evmReceiptProofHash(
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
            ),
            evidence.receiptProofHash,
        )

        for ((field, label) in listOf(
            "finalizedHeaderRoot" to "beaconFinality.finalizedHeaderRoot",
            "syncCommitteeRoot" to "beaconFinality.syncCommitteeRoot",
            "beaconSlot" to "beaconFinality.beaconSlot",
        )) {
            val missingFinality = assertFailsWith<IllegalArgumentException> {
                sdk.collectInboundEvidenceFromReceipt(
                    EthereumMainnetInboundEvidence(
                        receipt = receipt,
                        block = block,
                        beaconFinality = beaconFinality - field,
                        sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                        blockReceipts = blockReceipts,
                        inclusionBranch = inclusionBranch,
                    ),
                )
            }
            assertTrue(missingFinality.message?.contains(label) == true)
        }

        for ((alias, value, label) in listOf(
            Triple("transaction_hash", "0x" + "ac".repeat(32), "receipt.transactionHash"),
            Triple("block_hash", "0x" + "ac".repeat(32), "receipt.blockHash"),
            Triple("block_number", "0x1235", "receipt.blockNumber"),
            Triple("transaction_index", "0x0", "receipt.transactionIndex"),
        )) {
            val aliasConflict = assertFailsWith<IllegalArgumentException> {
                sdk.collectInboundEvidenceFromReceipt(
                    EthereumMainnetInboundEvidence(
                        receipt = receipt + (alias to value),
                        block = block,
                        beaconFinality = beaconFinality,
                        sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                        blockReceipts = blockReceipts,
                        inclusionBranch = inclusionBranch,
                    ),
                )
            }
            assertTrue(aliasConflict.message?.contains(label) == true)
        }
        for ((alias, value) in listOf("blockNumber" to "0x1235", "block_number" to "0x1235")) {
            val aliasConflict = assertFailsWith<IllegalArgumentException> {
                sdk.collectInboundEvidenceFromReceipt(
                    EthereumMainnetInboundEvidence(
                        receipt = receipt,
                        block = block + (alias to value),
                        beaconFinality = beaconFinality,
                        sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                        blockReceipts = blockReceipts,
                        inclusionBranch = inclusionBranch,
                    ),
                )
            }
            assertTrue(aliasConflict.message?.contains("block.number") == true)
        }
        val receiptsRootAliasConflict = assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt,
                    block = block + ("receipts_root" to ("0x" + "ac".repeat(32))),
                    beaconFinality = beaconFinality,
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                    blockReceipts = blockReceipts,
                    inclusionBranch = inclusionBranch,
                ),
            )
        }
        assertTrue(receiptsRootAliasConflict.message?.contains("block.receiptsRoot") == true)
        for ((alias, value, label) in listOf(
            Triple("block_hash", "0x" + "ac".repeat(32), "blockReceipts.blockHash"),
            Triple("block_number", "0x1235", "blockReceipts.blockNumber"),
        )) {
            val aliasConflict = assertFailsWith<IllegalArgumentException> {
                sdk.collectInboundEvidenceFromReceipt(
                    EthereumMainnetInboundEvidence(
                        receipt = receipt,
                        block = block,
                        beaconFinality = beaconFinality,
                        sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                        blockReceipts = listOf(receipt + (alias to value), otherReceipt),
                        inclusionBranch = inclusionBranch,
                    ),
                )
            }
            assertTrue(aliasConflict.message?.contains(label) == true)
        }
        val indexedHashAliasConflict = assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt,
                    block = block,
                    beaconFinality = beaconFinality,
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                    blockReceipts = listOf(receipt + ("transaction_hash" to receipt["transactionHash"]), otherReceipt),
                    inclusionBranch = inclusionBranch,
                ),
            )
        }
        assertTrue(indexedHashAliasConflict.message?.contains("blockReceipts[0].transactionHash") == true)

        val rootMismatch = assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt,
                    block = block + ("receiptsRoot" to ("0x" + "99".repeat(32))),
                    beaconFinality = beaconFinality + ("executionReceiptsRoot" to ("0x" + "99".repeat(32))),
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                    blockReceipts = blockReceipts,
                    inclusionBranch = inclusionBranch,
                ),
            )
        }
        assertTrue(rootMismatch.message?.contains("computed receipt trie root") == true)

        val mismatchedIndexedReceipt = receipt + ("logs" to emptyList<Map<String, Any?>>())
        val mismatchedBlockReceipts = listOf(mismatchedIndexedReceipt, otherReceipt)
        val mismatchedReceiptProof = SccpSourceProofs.buildEvmReceiptTrieProofFromReceipts(
            mismatchedBlockReceipts,
            "0x0",
        )
        val receiptRlpMismatch = assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt,
                    block = block + ("receiptsRoot" to mismatchedReceiptProof.receiptsRoot),
                    beaconFinality = beaconFinality + ("executionReceiptsRoot" to mismatchedReceiptProof.receiptsRoot),
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                    blockReceipts = mismatchedBlockReceipts,
                    inclusionBranch = inclusionBranch,
                ),
            )
        }
        assertTrue(receiptRlpMismatch.message?.contains("receipt RLP") == true)

        val blockHashDriftReceipt = receipt + ("blockHash" to ("0x" + "99".repeat(32)))
        val blockHashDrift = assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt,
                    block = block,
                    beaconFinality = beaconFinality,
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                    blockReceipts = listOf(blockHashDriftReceipt, otherReceipt),
                    inclusionBranch = inclusionBranch,
                ),
            )
        }
        assertTrue(blockHashDrift.message?.contains("blockHash") == true)

        val blockNumberDriftReceipt = receipt + ("blockNumber" to "0x1235")
        val blockNumberDrift = assertFailsWith<IllegalArgumentException> {
            sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt = receipt,
                    block = block,
                    beaconFinality = beaconFinality,
                    sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                    blockReceipts = listOf(blockNumberDriftReceipt, otherReceipt),
                    inclusionBranch = inclusionBranch,
                ),
            )
        }
        assertTrue(blockNumberDrift.message?.contains("blockNumber") == true)
    }

    @Test
    fun bscMainnetInboundEvidenceUsesMainnetRpcAndRejectsDrift() {
        val txHash = "0x" + "aa".repeat(32)
        val blockHash = "0x" + "bb".repeat(32)
        val sourceEventDigest = "0x" + "ee".repeat(32)
        val sourceBridgeEmitterAddress = "0x" + "44".repeat(20)
        fun sourceEventLog(overrides: Map<String, Any?> = emptyMap()): Map<String, Any?> =
            mapOf<String, Any?>(
                "address" to sourceBridgeEmitterAddress,
                "transactionHash" to txHash,
                "blockHash" to blockHash,
                "blockNumber" to "0x1234",
                "topics" to listOf(SccpEthereumMainnet.sourceEventTopic(), sourceEventDigest),
                "data" to "0x",
            ) + overrides
        val receipt = mapOf<String, Any?>(
            "transactionHash" to txHash,
            "blockHash" to blockHash,
            "blockNumber" to "0x1234",
            "status" to "0x1",
            "logs" to listOf(sourceEventLog()),
        )
        val block = mapOf<String, Any?>(
            "hash" to blockHash,
            "number" to "0x1234",
            "receiptsRoot" to "0x" + "cc".repeat(32),
        )
        val receiptProof = BscMainnetReceiptProof(
            sourceEventDigest = sourceEventDigest,
            validatorEpoch = "36",
            blockNumber = "4660",
            blockHash = blockHash,
            receiptsRoot = "0x" + "cc".repeat(32),
            validatorSetHash = "0x" + "ab".repeat(32),
            commitSealHash = "0x" + "dd".repeat(32),
            receiptRootIndex = "3",
            receiptTrieProofNodes = listOf(byteArrayOf(0x01), byteArrayOf(0x02, 0x03)),
            inclusionBranch = listOf(ByteArray(32) { 0x11.toByte() }),
        )
        val receiptProofHash = SccpSourceProofs.bscReceiptProofHash(
            sourceEventDigest = receiptProof.sourceEventDigest,
            validatorEpoch = receiptProof.validatorEpoch,
            blockNumber = receiptProof.blockNumber,
            blockHash = receiptProof.blockHash,
            receiptsRoot = receiptProof.receiptsRoot,
            validatorSetHash = receiptProof.validatorSetHash,
            commitSealHash = receiptProof.commitSealHash,
            receiptRootIndex = receiptProof.receiptRootIndex,
            receiptTrieProofNodes = receiptProof.receiptTrieProofNodes,
            inclusionBranch = receiptProof.inclusionBranch,
        )
        val parliaFinalityEvidence = BscMainnetParliaFinalityEvidence(
            executionBlockNumber = "0x1234",
            executionBlockHash = blockHash,
            executionReceiptsRoot = "0x" + "cc".repeat(32),
            additionalFields = mapOf(
                "validatorEpoch" to "0x24",
                "validatorSetHash" to ("0x" + "ab".repeat(32)),
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
                assertEquals(receiptProofHash, evidence.receiptProofHash)
                assertEquals(receiptProof.blockHash, evidence.receiptProof?.blockHash)
                assertEquals(sourceEventDigest, evidence.receiptProof?.sourceEventDigest)
                assertEquals(sourceEventDigest, evidence.sourceEventDigest)
                assertEquals(sourceBridgeEmitterAddress, evidence.sourceBridgeEmitterAddress)
                byteArrayOf(1, 2, 3)
            },
            inboundSubmitter = BscMainnetInboundSubmitter { proofBytes ->
                assertContentEquals(byteArrayOf(1, 2, 3), proofBytes)
                "submitted"
            },
            sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
        )

        val evidence = sdk.collectInboundEvidenceFromReceipt(
            BscMainnetInboundEvidence.withParliaFinalityEvidence(
                transactionHash = txHash,
                parliaFinalityEvidence = parliaFinalityEvidence,
                receiptProof = receiptProof,
            ),
        )
        assertEquals(txHash, evidence.transactionHash)
        assertEquals(receipt, evidence.receipt)
        assertEquals(block, evidence.block)
        assertEquals("4660", evidence.parliaFinality?.get("executionBlockNumber"))
        assertEquals(blockHash, evidence.parliaFinality?.get("executionBlockHash"))
        assertEquals(receiptProofHash, evidence.receiptProofHash)
        assertEquals(sourceEventDigest, evidence.sourceEventDigest)
        assertEquals(sourceBridgeEmitterAddress, evidence.sourceBridgeEmitterAddress)
        assertFalse(receiptProof === evidence.receiptProof)
        val mutableReceiptProofNode = receiptProof.receiptTrieProofNodes[0]
        val mutableReceiptProofBranch = receiptProof.inclusionBranch[0]
        mutableReceiptProofNode[0] = 0x7f
        mutableReceiptProofBranch[0] = 0x7f
        assertContentEquals(byteArrayOf(0x01), evidence.receiptProof?.receiptTrieProofNodes?.get(0))
        assertContentEquals(ByteArray(32) { 0x11.toByte() }, evidence.receiptProof?.inclusionBranch?.get(0))
        mutableReceiptProofNode[0] = 0x01
        mutableReceiptProofBranch[0] = 0x11
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
            sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
        ).collectInboundEvidenceFromReceipt(BscMainnetInboundEvidence(transactionHash = txHash, receiptProof = receiptProof))
        assertEquals(blockHash, providerFinality.parliaFinality?.get("executionBlockHash"))
        assertEquals(receiptProofHash, providerFinality.receiptProofHash)
        assertEquals(sourceEventDigest, providerFinality.sourceEventDigest)
        assertContentEquals(byteArrayOf(1, 2, 3), sdk.proveInboundToSora(evidence))
        assertEquals("submitted", sdk.submitInboundToIroha(byteArrayOf(1, 2, 3)))

        val receiptProofHashOnlyEvidence = BscMainnetSccp().collectInboundEvidenceFromReceipt(
            BscMainnetInboundEvidence(receiptProofHash = receiptProofHash),
        )
        assertEquals(receiptProofHash, receiptProofHashOnlyEvidence.receiptProofHash)
        assertNull(receiptProofHashOnlyEvidence.receiptProof)
        assertFailsWith<IllegalArgumentException> {
            BscMainnetSccp().collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(receiptProof = receiptProof, receiptProofHash = "0x" + "99".repeat(32)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            BscMainnetSccp().collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(receiptProof = receiptProof.copy(sourceDomain = SccpEvm.DOMAIN_ETH)),
            )
        }

        assertFailsWith<IllegalArgumentException> {
            BscMainnetSccp(
                executionProvider = BscMainnetExecutionProvider { _, _ -> "0x1" },
            ).collectInboundEvidenceFromReceipt(BscMainnetInboundEvidence(receipt = receipt))
        }
        assertFailsWith<IllegalArgumentException> {
            BscMainnetSccp(
                executionProvider = BscMainnetExecutionProvider { _, _ -> "56" },
            ).collectInboundEvidenceFromReceipt(BscMainnetInboundEvidence(receipt = receipt))
        }
        assertFailsWith<IllegalArgumentException> {
            BscMainnetSccp(
                executionProvider = BscMainnetExecutionProvider { _, _ -> 56L },
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
        var calledWithHashOnly = false
        val hashOnly = assertFailsWith<IllegalArgumentException> {
            BscMainnetSccp(
                inboundProver = BscMainnetInboundProver {
                    calledWithHashOnly = true
                    byteArrayOf(1, 2, 3)
                },
            ).proveInboundToSora(
                BscMainnetInboundEvidence(
                    parliaFinality = parliaFinality,
                    receiptProofHash = receiptProofHash,
                ),
            )
        }
        assertTrue(hashOnly.message?.contains("receiptProof") == true)
        assertFalse(calledWithHashOnly)
        var calledWithoutSourceEvent = false
        val noSourceEvent = assertFailsWith<IllegalArgumentException> {
            BscMainnetSccp(
                inboundProver = BscMainnetInboundProver {
                    calledWithoutSourceEvent = true
                    byteArrayOf(1, 2, 3)
                },
            ).proveInboundToSora(
                BscMainnetInboundEvidence(
                    parliaFinality = parliaFinality,
                    receiptProof = receiptProof,
                ),
            )
        }
        assertTrue(noSourceEvent.message?.contains("receipt source event validation") == true)
        assertFalse(calledWithoutSourceEvent)
        val driftedReceiptProof = assertFailsWith<IllegalArgumentException> {
            BscMainnetSccp(
                inboundProver = BscMainnetInboundProver {
                    throw AssertionError("prover callback must not run with drifted receiptProof")
                },
            ).proveInboundToSora(
                BscMainnetInboundEvidence(
                    receipt = receipt,
                    block = block,
                    parliaFinality = parliaFinality,
                    receiptProof = receiptProof.copy(receiptsRoot = "0x" + "99".repeat(32)),
                ),
            )
        }
        assertTrue(driftedReceiptProof.message?.contains("receiptProof.receiptsRoot") == true)
        val driftedSourceReceipt = receipt + (
            "logs" to listOf(
                sourceEventLog(
                    mapOf(
                        "topics" to listOf(SccpEthereumMainnet.sourceEventTopic(), "0x" + "99".repeat(32)),
                    ),
                ),
            )
            )
        val driftedSourceEvent = assertFailsWith<IllegalArgumentException> {
            BscMainnetSccp(
                inboundProver = BscMainnetInboundProver {
                    throw AssertionError("prover callback must not run with drifted receipt source event")
                },
                sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            ).proveInboundToSora(
                BscMainnetInboundEvidence(
                    receipt = driftedSourceReceipt,
                    block = block,
                    parliaFinality = parliaFinality,
                    receiptProof = receiptProof,
                ),
            )
        }
        assertTrue(driftedSourceEvent.message?.contains("receiptProof.sourceEventDigest") == true)
        val sourceEventGuardSdk = BscMainnetSccp(sourceBridgeEmitterAddress = sourceBridgeEmitterAddress)
        fun bscReceiptWithSourceLogs(logs: List<Map<String, Any?>>): Map<String, Any?> =
            receipt + ("logs" to logs)

        val extraTopicBscSourceLog = assertFailsWith<IllegalArgumentException> {
            sourceEventGuardSdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    receipt = bscReceiptWithSourceLogs(
                        listOf(
                            sourceEventLog(
                                mapOf(
                                    "topics" to listOf(
                                        SccpEthereumMainnet.sourceEventTopic(),
                                        sourceEventDigest,
                                        "0x" + "66".repeat(32),
                                    ),
                                ),
                            ),
                        ),
                    ),
                    block = block,
                ),
            )
        }
        assertTrue(extraTopicBscSourceLog.message?.contains("exactly 2 topics") == true)
        val nonEmptyDataBscSourceLog = assertFailsWith<IllegalArgumentException> {
            sourceEventGuardSdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    receipt = bscReceiptWithSourceLogs(listOf(sourceEventLog(mapOf("data" to "0x01")))),
                    block = block,
                ),
            )
        }
        assertTrue(nonEmptyDataBscSourceLog.message?.contains("data must be 0x") == true)
        val zeroDigestBscSourceLog = assertFailsWith<IllegalArgumentException> {
            sourceEventGuardSdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    receipt = bscReceiptWithSourceLogs(
                        listOf(
                            sourceEventLog(
                                mapOf(
                                    "topics" to listOf(
                                        SccpEthereumMainnet.sourceEventTopic(),
                                        "0x" + "00".repeat(32),
                                    ),
                                ),
                            ),
                        ),
                    ),
                    block = block,
                ),
            )
        }
        assertTrue(zeroDigestBscSourceLog.message?.contains("digest must not be zero") == true)
        val duplicateBscSourceLog = assertFailsWith<IllegalArgumentException> {
            sourceEventGuardSdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    receipt = bscReceiptWithSourceLogs(listOf(sourceEventLog(), sourceEventLog())),
                    block = block,
                ),
            )
        }
        assertTrue(duplicateBscSourceLog.message?.contains("exactly one matching") == true)
        val removedBscSourceLog = assertFailsWith<IllegalArgumentException> {
            sourceEventGuardSdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    receipt = bscReceiptWithSourceLogs(listOf(sourceEventLog(mapOf("removed" to true)))),
                    block = block,
                ),
            )
        }
        assertTrue(removedBscSourceLog.message?.contains("removed logs") == true)
        val missingBscSourceContextLog = assertFailsWith<IllegalArgumentException> {
            sourceEventGuardSdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    receipt = bscReceiptWithSourceLogs(listOf(sourceEventLog() - "transactionHash")),
                    block = block,
                ),
            )
        }
        assertTrue(missingBscSourceContextLog.message?.contains("receipt.logs[0].transactionHash") == true)
        for ((alias, value, label) in listOf(
            Triple("transaction_hash", "0x" + "ab".repeat(32), "receipt.logs[0].transactionHash"),
            Triple("block_hash", "0x" + "ac".repeat(32), "receipt.logs[0].blockHash"),
            Triple("block_number", "0x1235", "receipt.logs[0].blockNumber"),
        )) {
            val conflictingBscSourceContextLog = assertFailsWith<IllegalArgumentException> {
                sourceEventGuardSdk.collectInboundEvidenceFromReceipt(
                    BscMainnetInboundEvidence(
                        receipt = bscReceiptWithSourceLogs(listOf(sourceEventLog(mapOf(alias to value)))),
                        block = block,
                    ),
                )
            }
            assertTrue(conflictingBscSourceContextLog.message?.contains(label) == true)
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

    private fun sampleEvmReceipt(
        transactionIndex: Int,
        transactionHash: String,
        blockHash: String,
        blockNumber: String,
        status: String = "0x1",
        type: String? = null,
        logs: List<Map<String, Any?>> = emptyList(),
    ): Map<String, Any?> {
        val receipt = linkedMapOf<String, Any?>(
            "transactionHash" to transactionHash,
            "transactionIndex" to "0x" + transactionIndex.toString(16),
            "blockHash" to blockHash,
            "blockNumber" to blockNumber,
            "status" to status,
            "cumulativeGasUsed" to "0x" + (21_000L * (transactionIndex + 1)).toString(16),
            "logsBloom" to ("0x" + "00".repeat(256)),
            "logs" to logs,
        )
        if (type != null) {
            receipt["type"] = type
        }
        return receipt
    }

    private fun sampleGroth16ProofBytes(
        overrides: Map<Int, ByteArray> = emptyMap(),
        publicInputs: EvmSccpPublicInputsInput = samplePublicInputs(),
        sourceDomain: Int = SccpEvm.DOMAIN_SORA,
    ): ByteArray {
        val words = mutableListOf(
            abiWord(1),
            hexWord(publicInputs.messageId.removePrefix("0x")),
            abiWord(sourceDomain.toLong()),
            hexWord(publicInputs.commitmentRoot.removePrefix("0x")),
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

    private fun beaconHeaderJson(
        executionOptimistic: Boolean = false,
        finalized: Boolean = true,
        rootByte: String? = null,
        slot: String = "64",
        root: String = "0xbb44a971e8c280f585ba430bfabfe87d9c59adf38bf9f77266b69687a148048c",
    ): String =
        """
        {
          "execution_optimistic": $executionOptimistic,
          "finalized": $finalized,
          "data": {
            "root": "${rootByte?.let { "0x${it.repeat(32)}" } ?: root}",
            "canonical": true,
            "header": {
              "message": {
                "slot": "$slot",
                "proposer_index": "1",
                "parent_root": "0x${"01".repeat(32)}",
                "state_root": "0x${"02".repeat(32)}",
                "body_root": "0x${"03".repeat(32)}"
              },
              "signature": "0x${"12".repeat(96)}"
            }
          }
        }
        """.trimIndent()

    private fun beaconCheckpointJson(
        rootByte: String? = null,
        root: String = "0xbb44a971e8c280f585ba430bfabfe87d9c59adf38bf9f77266b69687a148048c",
    ): String =
        """
        {
          "execution_optimistic": false,
          "finalized": true,
          "data": {
            "finalized": {
              "root": "${rootByte?.let { "0x${it.repeat(32)}" } ?: root}",
              "epoch": "2"
            }
          }
        }
        """.trimIndent()

    private fun beaconBlockRootJson(
        rootByte: String? = null,
        root: String = "0xbb44a971e8c280f585ba430bfabfe87d9c59adf38bf9f77266b69687a148048c",
    ): String =
        """
        {
          "execution_optimistic": false,
          "finalized": true,
          "data": {
            "root": "${rootByte?.let { "0x${it.repeat(32)}" } ?: root}"
          }
        }
        """.trimIndent()

    private fun beaconBlockJson(
        slot: String = "64",
        blockHash: String = "0x" + "bb".repeat(32),
        blockNumber: String = "4660",
        receiptsRoot: String = "0x" + "cc".repeat(32),
    ): String =
        """
        {
          "execution_optimistic": false,
          "finalized": true,
          "data": {
            "message": {
              "slot": "$slot",
              "body": {
                "execution_payload": {
                  "block_hash": "$blockHash",
                  "block_number": "$blockNumber",
                  "receipts_root": "$receiptsRoot"
                }
              }
            }
          }
        }
        """.trimIndent()

    private fun beaconGenesisJson(genesisTime: String = "100"): String =
        """
        {
          "data": {
            "genesis_time": "$genesisTime",
            "genesis_validators_root": "0x${"ab".repeat(32)}",
            "genesis_fork_version": "0x00000000"
          }
        }
        """.trimIndent()

    private fun ethereumBeaconFinalityUpdateFields(): Map<String, Any?> =
        mapOf(
            "finalityBranch" to ethereumFinalityBranch,
            "syncCommitteeBits" to ethereumSyncCommitteeSupermajorityBits,
            "syncCommitteeSignature" to ("0x" + "34".repeat(96)),
            "syncCommitteeParticipation" to ethereumSyncCommitteeSupermajorityParticipation,
            "syncSignatureSlot" to "65",
        )

    private fun beaconFinalityUpdateJson(
        slot: String = "64",
        signatureSlot: String = "65",
        syncCommitteeBits: String = ethereumSyncCommitteeSupermajorityBits,
        syncCommitteeSignature: String = "0x" + "34".repeat(96),
        includeFinalityBranch: Boolean = true,
        finalityBranch: List<String> = ethereumFinalityBranch,
    ): String =
        run {
            val quotedFinalityBranch = finalityBranch.joinToString(",") { "\"$it\"" }
            val finalityBranchField =
                if (includeFinalityBranch) {
                    "\"finality_branch\": [$quotedFinalityBranch],"
                } else {
                    ""
                }
        """
        {
          "execution_optimistic": false,
          "data": {
            "finalized_header": {
              "beacon": {
                "slot": "$slot",
                "proposer_index": "1",
                "parent_root": "0x${"01".repeat(32)}",
                "state_root": "0x${"02".repeat(32)}",
                "body_root": "0x${"03".repeat(32)}"
              }
            },
            $finalityBranchField
            "sync_aggregate": {
              "sync_committee_bits": "$syncCommitteeBits",
              "sync_committee_signature": "$syncCommitteeSignature"
            },
            "signature_slot": "$signatureSlot"
          }
        }
        """.trimIndent()
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

    private fun indexedSyncCommitteeBytes(fill: Int, count: Int, index: Int): ByteArray =
        ByteArray(count) { fill.toByte() }.also {
            it[count - 2] = ((index ushr 8) and 0xff).toByte()
            it[count - 1] = (index and 0xff).toByte()
        }

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

    private fun sha256Hex(bytes: ByteArray): String =
        "0x" + hexLower(MessageDigest.getInstance("SHA-256").digest(bytes))

    private data class SampleBundleFixture(
        val publicInputs: EvmSccpPublicInputsInput,
        val bundleBytes: ByteArray,
    )

    private fun sampleBundleBytes(
        sourceDomain: Int = SccpSolana.DOMAIN_SORA,
        targetDomain: Int = SccpEvm.DOMAIN_ETH,
        nonce: Long = 327L,
        recipient: String? = null,
    ): ByteArray =
        sampleBundleFixture(
            sourceDomain = sourceDomain,
            targetDomain = targetDomain,
            nonce = nonce,
            recipient = recipient,
        ).bundleBytes

    private fun sampleBundleFixture(
        sourceDomain: Int = SccpSolana.DOMAIN_SORA,
        targetDomain: Int = SccpEvm.DOMAIN_ETH,
        nonce: Long = 327L,
        recipient: String? = null,
    ): SampleBundleFixture {
        val recipientCodec = if (targetDomain == SccpTron.DOMAIN_TRON) 5 else 2
        val defaultRecipient = if (targetDomain == SccpTron.DOMAIN_TRON) {
            "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8"
        } else {
            "0x" + "11".repeat(20)
        }
        val resolvedRecipient = recipient ?: defaultRecipient
        val senderCodec = if (sourceDomain == SccpSolana.DOMAIN_SORA) 1 else 2
        val sender = if (sourceDomain == SccpSolana.DOMAIN_SORA) {
            "alice@sora"
        } else {
            "0x52908400098527886E0F7030069857D2E4169EE7"
        }
        val routeId = when (targetDomain) {
            SccpEvm.DOMAIN_BSC -> "sora-bsc-xor"
            SccpTron.DOMAIN_TRON -> "sora-tron-xor"
            else -> "sora-eth-xor"
        }
        val payloadBody = ByteArrayOutputStream()
        payloadBody.write(1)
        writeTestU32Le(payloadBody, sourceDomain)
        writeTestU32Le(payloadBody, targetDomain)
        writeTestU64Le(payloadBody, nonce)
        writeTestU32Le(payloadBody, SccpSolana.DOMAIN_SORA)
        payloadBody.write(1)
        writeTestBytes(payloadBody, "xor#sccp".toByteArray(Charsets.UTF_8))
        writeTestU128Le(payloadBody, 42L)
        payloadBody.write(senderCodec)
        writeTestBytes(payloadBody, sender.toByteArray(Charsets.UTF_8))
        payloadBody.write(recipientCodec)
        writeTestBytes(payloadBody, resolvedRecipient.toByteArray(Charsets.UTF_8))
        payloadBody.write(1)
        writeTestBytes(payloadBody, routeId.toByteArray(Charsets.UTF_8))

        val payloadBodyBytes = payloadBody.toByteArray()
        val payloadBytes = byteArrayOf(0x02) + payloadBodyBytes
        val messageId = "0x" + hexLower(prefixedKeccakBytes("sccp:transfer:v1", payloadBodyBytes))
        val payloadHash = "0x" + hexLower(
            Blake2b.digest256("sccp:payload:v1".toByteArray(Charsets.UTF_8) + payloadBytes),
        )

        val commitment = ByteArrayOutputStream()
        commitment.write(1)
        commitment.write(6)
        writeTestU32Le(commitment, targetDomain)
        commitment.write(hexBytes(messageId.removePrefix("0x")))
        commitment.write(hexBytes(payloadHash.removePrefix("0x")))
        val commitmentBytes = commitment.toByteArray()
        val currentRoot = Blake2b.digest256("sccp:hub:leaf:v1".toByteArray(Charsets.UTF_8) + commitmentBytes)
        val commitmentRoot = "0x" + hexLower(currentRoot)

        val merkleProof = ByteArrayOutputStream()
        writeTestU32Le(merkleProof, 0)

        val bundle = ByteArrayOutputStream()
        bundle.write(1)
        bundle.write(currentRoot)
        writeTestBytes(bundle, commitmentBytes)
        writeTestBytes(bundle, merkleProof.toByteArray())
        writeTestBytes(bundle, payloadBytes)
        writeTestBytes(bundle, byteArrayOf(0x01, 0x02, 0x03))

        return SampleBundleFixture(
            publicInputs = EvmSccpPublicInputsInput(
                messageId = messageId,
                payloadHash = payloadHash,
                targetDomain = targetDomain,
                commitmentRoot = commitmentRoot,
                finalityHeight = "19",
                finalityBlockHash = "44".repeat(32),
            ),
            bundleBytes = bundle.toByteArray(),
        )
    }

    private fun sampleTokenAddBundleFixture(
        targetDomain: Int = SccpEvm.DOMAIN_ETH,
        nonce: Long = 327L,
        name: ByteArray = fixedTestAscii32("Token"),
        symbol: ByteArray = fixedTestAscii32("TOK"),
    ): SampleBundleFixture {
        require(name.size == 32)
        require(symbol.size == 32)

        val payloadBody = ByteArrayOutputStream()
        payloadBody.write(1)
        writeTestU32Le(payloadBody, targetDomain)
        writeTestU64Le(payloadBody, nonce)
        payloadBody.write(ByteArray(32) { 0x11.toByte() })
        payloadBody.write(18)
        payloadBody.write(name)
        payloadBody.write(symbol)

        val payloadBodyBytes = payloadBody.toByteArray()
        val payloadBytes = byteArrayOf(0x03) + payloadBodyBytes
        val messageId = "0x" + hexLower(prefixedKeccakBytes("sccp:token:add:v1", payloadBodyBytes))
        val payloadHash = "0x" + hexLower(
            Blake2b.digest256("sccp:payload:v1".toByteArray(Charsets.UTF_8) + payloadBytes),
        )

        val commitment = ByteArrayOutputStream()
        commitment.write(1)
        commitment.write(1)
        writeTestU32Le(commitment, targetDomain)
        commitment.write(hexBytes(messageId.removePrefix("0x")))
        commitment.write(hexBytes(payloadHash.removePrefix("0x")))
        val commitmentBytes = commitment.toByteArray()
        val currentRoot = Blake2b.digest256("sccp:hub:leaf:v1".toByteArray(Charsets.UTF_8) + commitmentBytes)
        val commitmentRoot = "0x" + hexLower(currentRoot)

        val merkleProof = ByteArrayOutputStream()
        writeTestU32Le(merkleProof, 0)

        val bundle = ByteArrayOutputStream()
        bundle.write(1)
        bundle.write(currentRoot)
        writeTestBytes(bundle, commitmentBytes)
        writeTestBytes(bundle, merkleProof.toByteArray())
        writeTestBytes(bundle, payloadBytes)
        writeTestBytes(bundle, byteArrayOf(0x01, 0x02, 0x03))

        return SampleBundleFixture(
            publicInputs = EvmSccpPublicInputsInput(
                messageId = messageId,
                payloadHash = payloadHash,
                targetDomain = targetDomain,
                commitmentRoot = commitmentRoot,
                finalityHeight = "19",
                finalityBlockHash = "44".repeat(32),
            ),
            bundleBytes = bundle.toByteArray(),
        )
    }

    private fun fixedTestAscii32(value: String): ByteArray {
        val bytes = value.toByteArray(Charsets.UTF_8)
        require(bytes.size <= 32)
        return bytes.copyOf(32)
    }

    private fun writeTestBytes(out: ByteArrayOutputStream, value: ByteArray) {
        writeTestU32Le(out, value.size)
        out.write(value)
    }

    private fun writeTestU32Le(out: ByteArrayOutputStream, value: Int) {
        require(value >= 0)
        out.write(value and 0xff)
        out.write((value ushr 8) and 0xff)
        out.write((value ushr 16) and 0xff)
        out.write((value ushr 24) and 0xff)
    }

    private fun writeTestU64Le(out: ByteArrayOutputStream, value: Long) {
        var working = value
        repeat(8) {
            out.write((working and 0xffL).toInt())
            working = working ushr 8
        }
    }

    private fun writeTestU128Le(out: ByteArrayOutputStream, value: Long) {
        writeTestU64Le(out, value)
        writeTestU64Le(out, 0L)
    }

    private fun hexBytes(hex: String): ByteArray {
        require(hex.length % 2 == 0)
        val out = ByteArray(hex.length / 2)
        for (index in out.indices) {
            out[index] = hex.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
        return out
    }

    private fun prefixedKeccakBytes(prefix: String, payload: ByteArray): ByteArray {
        val digest = KeccakDigest(256)
        val prefixBytes = prefix.toByteArray(Charsets.UTF_8)
        digest.update(prefixBytes, 0, prefixBytes.size)
        digest.update(payload, 0, payload.size)
        val out = ByteArray(32)
        digest.doFinal(out, 0)
        return out
    }

    private fun nativeEvmProverArtifactBytes(label: String, size: Int = 64 * 1024): ByteArray {
        val labelBytes = label.toByteArray(Charsets.UTF_8)
        val bytes = ByteArray(size) { index ->
            ((index * 37 + labelBytes.size * 11) and 0xff).toByte()
        }
        labelBytes.copyInto(bytes, endIndex = labelBytes.size.coerceAtMost(bytes.size))
        return bytes
    }

    private fun sampleProofRequestInput(
        publicInputs: EvmSccpPublicInputsInput = samplePublicInputs(),
        bundleBytes: ByteArray = sampleBundleBytes(targetDomain = publicInputs.targetDomain),
        sourceProofBytes: ByteArray = ByteArray(0),
        statementHash: String = "56".repeat(32),
        destinationBindingHash: String = "78".repeat(32),
        backend: String = SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1,
        sourceDomain: Int = SccpSolana.DOMAIN_SORA,
        proofArtifactHash: String? = null,
        provingKeyHash: String? = null,
    ): EvmSccpProofRequestInput =
        EvmSccpProofRequestInput(
            publicInputs = publicInputs,
            bundleBytes = bundleBytes,
            sourceProofBytes = sourceProofBytes,
            statementHash = statementHash,
            destinationBindingHash = destinationBindingHash,
            backend = backend,
            sourceDomain = sourceDomain,
            proofArtifactHash = proofArtifactHash,
            provingKeyHash = provingKeyHash,
        )

    private fun sampleProductionProofRequestInput(
        publicInputs: EvmSccpPublicInputsInput = samplePublicInputs(),
        bundleBytes: ByteArray = sampleBundleBytes(targetDomain = publicInputs.targetDomain),
        sourceProofBytes: ByteArray = ByteArray(0),
        statementHash: String = "56".repeat(32),
        backend: String = SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1,
        sourceDomain: Int = SccpSolana.DOMAIN_SORA,
        proofArtifactHash: String? = null,
        provingKeyHash: String? = null,
    ): EvmSccpProofRequestInput =
        EvmSccpProofRequestInput(
            publicInputs = publicInputs,
            bundleBytes = bundleBytes,
            sourceProofBytes = sourceProofBytes,
            statementHash = statementHash,
            destinationBinding = sampleDestinationBinding(publicInputs),
            backend = backend,
            sourceDomain = sourceDomain,
            proofArtifactHash = proofArtifactHash,
            provingKeyHash = provingKeyHash,
        )

    private fun sampleEthereumNativeEvmProverBundle(
        destinationBindingHash: String,
        verifierKeyHash: String = "0x" + "cc".repeat(32),
        noWasm: Boolean = true,
        remoteProverRequired: Boolean = false,
        expectedDestinationBindingHash: String? = null,
    ): SccpEvm.EthereumMainnetNativeEvmProverBundle {
        val proofArtifactHash = "0x" + "91".repeat(32)
        val provingKeyHash = "0x" + "92".repeat(32)
        val artifacts = SccpEvm.ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1
            .entries
            .sortedBy { it.key }
            .mapIndexed { index, entry ->
                SccpEvm.EthereumMainnetNativeEvmProverBundleSdkArtifact(
                    sdk = entry.key,
                    implementation = entry.value,
                        proofArtifactHash = proofArtifactHash,
                        provingKeyHash = provingKeyHash,
                        implementationHash = "0x" + (index + 1).toString(16).padStart(2, '0').repeat(32),
                        implementationArtifact = "artifacts/eth-mainnet/${entry.key}-implementation.bin",
                    )
                }
        return SccpEvm.EthereumMainnetNativeEvmProverBundle(
            proofArtifact = "artifacts/eth-mainnet/proof-artifact.bin",
            proofArtifactHash = proofArtifactHash,
            provingKey = "artifacts/eth-mainnet/proving-key.bin",
            provingKeyHash = provingKeyHash,
            verifierKey = "artifacts/eth-mainnet/verifier-key.bin",
            verifierKeyHash = verifierKeyHash,
            destinationBindingHash = destinationBindingHash,
            noWasm = noWasm,
            remoteProverRequired = remoteProverRequired,
            nativeSdkArtifacts = artifacts,
            crossSdkFixtureParityArtifact = "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
            nativeProverSelfTestArtifact = "artifacts/eth-mainnet/native-prover-self-test.json",
            auditHashes = sampleEthereumNativeAuditHashes(),
            expectedDestinationBindingHash = expectedDestinationBindingHash,
        )
    }

    private fun sampleEthereumNativeAuditHashes(): Map<String, String> = mapOf(
        "circuit_security_audit" to "0x" + "a1".repeat(32),
        "native_implementation_audit" to "0x" + "a2".repeat(32),
        "reproducible_build_attestation" to "0x" + "a3".repeat(32),
        "cross_sdk_fixture_parity" to "0x" + "a4".repeat(32),
        "native_prover_self_test" to "0x" + "a5".repeat(32),
        "no_wasm_no_remote_scan" to "0x" + "a6".repeat(32),
    )

    private fun sampleEthereumNativeEvmProverBundleJson(
        destinationBindingHash: String,
        proofArtifact: String = "artifacts/eth-mainnet/proof-artifact.bin",
        noWasm: Boolean = true,
        remoteProverRequired: Boolean = false,
    ): String {
        val proofArtifactHash = "0x" + "91".repeat(32)
        val provingKeyHash = "0x" + "92".repeat(32)
        val artifacts = SccpEvm.ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1
            .entries
            .sortedBy { it.key }
            .mapIndexed { index, entry ->
                """
                {
                  "sdk": "${entry.key}",
                  "implementation": "${entry.value}",
                  "prover_artifact_hash": "$proofArtifactHash",
                  "proving_key_hash": "$provingKeyHash",
                  "implementation_artifact": "artifacts/eth-mainnet/${entry.key}-implementation.bin",
                  "implementation_hash": "0x${(index + 1).toString(16).padStart(2, '0').repeat(32)}"
                }
                """.trimIndent()
            }
            .joinToString(",")
        return """
            {
              "schema": "${SccpEvm.NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1}",
              "bundle_id": "${SccpEvm.ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1}",
              "domain": ${SccpEvm.DOMAIN_ETH},
              "chain": "eth",
              "proof_backend": "${SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1}",
              "proof_artifact": "$proofArtifact",
              "proof_artifact_hash": "$proofArtifactHash",
              "proving_key": "artifacts/eth-mainnet/proving-key.bin",
              "proving_key_hash": "$provingKeyHash",
              "verifier_key": "artifacts/eth-mainnet/verifier-key.bin",
              "verifier_key_hash": "0x${"cc".repeat(32)}",
              "destination_binding_hash": "$destinationBindingHash",
              "no_wasm": $noWasm,
              "remote_prover_required": $remoteProverRequired,
              "browser_implementation": "pure-typescript",
              "native_sdk_artifacts": [$artifacts],
              "cross_sdk_fixture_parity_artifact": "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
              "native_prover_self_test_artifact": "artifacts/eth-mainnet/native-prover-self-test.json",
              "audit_hashes": {
                "circuit_security_audit": "0x${"a1".repeat(32)}",
                "native_implementation_audit": "0x${"a2".repeat(32)}",
                "reproducible_build_attestation": "0x${"a3".repeat(32)}",
                "cross_sdk_fixture_parity": "0x${"a4".repeat(32)}",
                "native_prover_self_test": "0x${"a5".repeat(32)}",
                "no_wasm_no_remote_scan": "0x${"a6".repeat(32)}"
              }
            }
        """.trimIndent()
    }

    private fun sampleEthereumNativeEvmProverParityFixtureJson(
        nativeProverBundle: SccpEvm.EthereumMainnetNativeEvmProverBundle,
        kotlinCalldataHash: String? = null,
    ): String {
        val publicSignalWords = (0 until 9).map { index ->
            "0x" + (index + 0x10).toString(16).padStart(2, '0').repeat(32)
        }
        fun sdkResult(calldataHash: String = "0x" + "d3".repeat(32)): String =
            """
            {
              "receipt_proof_hash": "0x${"d1".repeat(32)}",
              "source_proof_hash": "0x${"d2".repeat(32)}",
              "destination_binding_hash": "${nativeProverBundle.destinationBindingHash}",
              "public_signal_words": [${publicSignalWords.joinToString(",") { "\"$it\"" }}],
              "calldata_hash": "$calldataHash",
              "torii_submit_payload_hash": "0x${"d4".repeat(32)}"
            }
            """.trimIndent()
        val defaultCalldataHash = "0x" + "d3".repeat(32)
        val sdkResults = SccpEvm.ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1
            .keys
            .sorted()
            .joinToString(",") { sdk ->
                val calldataHash = if (sdk == "kotlin") {
                    kotlinCalldataHash ?: defaultCalldataHash
                } else {
                    defaultCalldataHash
                }
                "\"$sdk\": ${sdkResult(calldataHash)}"
            }
        return """
            {
              "schema": "${SccpEvm.ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1}",
              "domain": ${SccpEvm.DOMAIN_ETH},
              "chain": "eth",
              "proof_backend": "${SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1}",
              "proof_artifact_hash": "${nativeProverBundle.proofArtifactHash}",
              "proving_key_hash": "${nativeProverBundle.provingKeyHash}",
              "verifier_key_hash": "${nativeProverBundle.verifierKeyHash}",
              "destination_binding_hash": "${nativeProverBundle.destinationBindingHash}",
              "receipt_proof_hash": "0x${"d1".repeat(32)}",
              "source_proof_hash": "0x${"d2".repeat(32)}",
              "public_signal_words": [${publicSignalWords.joinToString(",") { "\"$it\"" }}],
              "calldata_hash": "$defaultCalldataHash",
              "torii_submit_payload_hash": "0x${"d4".repeat(32)}",
              "sdk_results": {
                $sdkResults
              }
            }
        """.trimIndent()
    }

    private fun sampleEthereumNativeEvmProverSelfTestFixtureJson(
        nativeProverBundle: SccpEvm.EthereumMainnetNativeEvmProverBundle,
        kotlinProofHash: String? = null,
    ): String {
        val publicSignalWords = (0 until 9).map { index ->
            "0x" + (index + 0x20).toString(16).padStart(2, '0').repeat(32)
        }
        fun sdkResult(proofHash: String = "0x" + "e4".repeat(32)): String =
            """
            {
              "request_hash": "0x${"e1".repeat(32)}",
              "witness_hash": "0x${"e2".repeat(32)}",
              "source_proof_hash": "0x${"e3".repeat(32)}",
              "proof_hash": "$proofHash",
              "public_signal_words": [${publicSignalWords.joinToString(",") { "\"$it\"" }}],
              "calldata_hash": "0x${"e5".repeat(32)}",
              "torii_submit_payload_hash": "0x${"e6".repeat(32)}"
            }
            """.trimIndent()
        val defaultProofHash = "0x" + "e4".repeat(32)
        val sdkResults = SccpEvm.ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1
            .keys
            .sorted()
            .joinToString(",") { sdk ->
                val proofHash = if (sdk == "kotlin") {
                    kotlinProofHash ?: defaultProofHash
                } else {
                    defaultProofHash
                }
                "\"$sdk\": ${sdkResult(proofHash)}"
            }
        return """
            {
              "schema": "${SccpEvm.ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1}",
              "domain": ${SccpEvm.DOMAIN_ETH},
              "chain": "eth",
              "proof_backend": "${SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1}",
              "proof_artifact_hash": "${nativeProverBundle.proofArtifactHash}",
              "proving_key_hash": "${nativeProverBundle.provingKeyHash}",
              "verifier_key_hash": "${nativeProverBundle.verifierKeyHash}",
              "destination_binding_hash": "${nativeProverBundle.destinationBindingHash}",
              "request_hash": "0x${"e1".repeat(32)}",
              "witness_hash": "0x${"e2".repeat(32)}",
              "source_proof_hash": "0x${"e3".repeat(32)}",
              "proof_hash": "$defaultProofHash",
              "public_signal_words": [${publicSignalWords.joinToString(",") { "\"$it\"" }}],
              "calldata_hash": "0x${"e5".repeat(32)}",
              "torii_submit_payload_hash": "0x${"e6".repeat(32)}",
              "sdk_results": {
                $sdkResults
              }
            }
        """.trimIndent()
    }

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
    ): EvmSccpPublicInputsInput {
        val fixture = sampleBundleFixture(targetDomain = targetDomain)
        return EvmSccpPublicInputsInput(
            messageId = fixture.publicInputs.messageId,
            payloadHash = fixture.publicInputs.payloadHash,
            targetDomain = targetDomain,
            commitmentRoot = fixture.publicInputs.commitmentRoot,
            finalityHeight = finalityHeight,
            finalityBlockHash = fixture.publicInputs.finalityBlockHash,
        )
    }
}
