package org.hyperledger.iroha.sdk.privacy

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class PrivacyNativeBridgeTest {
    @Test
    fun exposesStableFailClosedErrorCodes() {
        assertEquals(7, PrivacyNativeBridge.REQUIRED_BRIDGE_ABI_VERSION)
        assertEquals(1, PrivacyNativeBridge.PRIVACY_FFI_VERSION_V1)
        assertEquals("privacy-production-gate-v1", PrivacyNativeBridge.PRODUCTION_GATE_VERSION)
        assertEquals(1, PrivacyNativeBridge.STATUS_ERROR)
        assertEquals(1, PrivacyNativeBridge.ERROR_NULL_POINTER)
        assertEquals(2, PrivacyNativeBridge.ERROR_MALFORMED_NORITO)
        assertEquals(3, PrivacyNativeBridge.ERROR_UNSUPPORTED_ALGORITHM)
        assertEquals(4, PrivacyNativeBridge.ERROR_PRODUCTION_DISABLED)
        assertEquals(5, PrivacyNativeBridge.ERROR_INVALID_REQUEST)
        assertEquals(64 * 1024 * 1024, PrivacyNativeBridge.PRIVACY_NATIVE_ARCHIVE_MAX_BYTES)
    }

    @Test
    fun reportsFailClosedPrivacyCapabilities() {
        val current = PrivacyNativeBridge.privacyCapabilities()
        assertTrue(current.kotlinSdkAvailable)
        assertEquals(PrivacyNativeBridge.isNativeAvailable(), current.bridgeAvailable)
        assertFailClosedProductionGate(current)

        val bridgeAvailable = PrivacyNativeBridge.privacyCapabilities(bridgeAvailable = true)
        assertTrue(bridgeAvailable.kotlinSdkAvailable)
        assertTrue(bridgeAvailable.bridgeAvailable)
        assertFailClosedProductionGate(bridgeAvailable)

        val bridgeUnavailable = PrivacyNativeBridge.privacyCapabilities(bridgeAvailable = false)
        assertTrue(bridgeUnavailable.kotlinSdkAvailable)
        assertFalse(bridgeUnavailable.bridgeAvailable)
        assertFailClosedProductionGate(bridgeUnavailable)

        assertFailsWith<UnsupportedOperationException> {
            @Suppress("UNCHECKED_CAST")
            (bridgeAvailable.productionGate.missing as MutableList<String>).add("tampered")
        }
        assertFailsWith<UnsupportedOperationException> {
            @Suppress("UNCHECKED_CAST")
            (bridgeAvailable.productionGate.requiredGates as MutableList<String>).add("tampered")
        }
        assertFailsWith<UnsupportedOperationException> {
            @Suppress("UNCHECKED_CAST")
            (bridgeAvailable.productionGate.auditReferences as MutableList<String>).add(
                "https://audit.example/forged-signoff",
            )
        }

        val fresh = PrivacyNativeBridge.privacyCapabilities(bridgeAvailable = true)
        assertFalse(fresh.productionGate.missing.contains("tampered"))
        assertFalse(fresh.productionGate.requiredGates.contains("tampered"))
        assertFalse(
            fresh.productionGate.auditReferences.contains(
                "https://audit.example/forged-signoff",
            ),
        )
        assertEquals(
            PrivacyNativeBridge.PrivacyProductionGate.MISSING_REASONS,
            fresh.productionGate.missing,
        )
        assertEquals(
            PrivacyNativeBridge.PrivacyProductionGate.REQUIRED_GATES,
            fresh.productionGate.requiredGates,
        )
    }

    @Test
    fun rejectsEmptyRequestsBeforeNativeDispatch() {
        val helpers = listOf<(ByteArray?) -> ByteArray>(
            PrivacyNativeBridge::buildProof,
            PrivacyNativeBridge::buildConfidentialTransferProofV2,
            PrivacyNativeBridge::buildConfidentialUnshieldProofV3,
            PrivacyNativeBridge::buildZkAceAuthorizationProofV1,
            PrivacyNativeBridge::buildJindoLatticeProofV0,
            PrivacyNativeBridge::buildSisHintsAnonymousCredentialProofV0,
            PrivacyNativeBridge::buildSilentThresholdCredentialShowingProofV0,
            PrivacyNativeBridge::buildVegaCredentialPredicateProofV0,
            PrivacyNativeBridge::buildZkAmsAdmissionBatchProofV0,
            PrivacyNativeBridge::buildZkAtPolicyProofV1,
            PrivacyNativeBridge::verifyJindoPolynomialCommitmentV0,
            PrivacyNativeBridge::verifySisHintsAnonymousCredentialProofV0,
            PrivacyNativeBridge::verifySilentThresholdCredentialShowingProofV0,
            PrivacyNativeBridge::verifyVegaCredentialPredicateProofV0,
            PrivacyNativeBridge::verifyZkAmsAdmissionBatchProofV0,
            PrivacyNativeBridge::verifyZkAtPolicyProofV1,
            PrivacyNativeBridge::verifyProof,
        )

        for (helper in helpers) {
            assertFailsWith<IllegalArgumentException> {
                helper(ByteArray(0))
            }
            assertFailsWith<IllegalArgumentException> {
                helper(null)
            }
        }
        val oversized = ByteArray(PrivacyNativeBridge.PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1)
        for (helper in helpers) {
            assertFailsWith<IllegalArgumentException> {
                helper(oversized)
            }
            assertFailsWith<IllegalArgumentException> {
                helper(privacyNoritoFrame(0x52))
            }
        }
    }

    @Test
    fun rejectsInvalidProofRequestComponentsBeforeNativeDispatch() {
        assertFailsWith<IllegalArgumentException> {
            PrivacyNativeBridge.privacyProofRequestV1(
                null,
                "buildZkAceAuthorizationProofV1",
                "stark-fri:zk_ace_pq_authorization_v0",
                "public-inputs".toByteArray(),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            PrivacyNativeBridge.privacyProofRequestV1(
                "zk-ace-pq-authorization-v0",
                "buildZkAceAuthorizationProofV1",
                "stark-fri:zk_ace_pq_authorization_v0",
                ByteArray(0),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            PrivacyNativeBridge.privacyProofRequestV1(
                "zk-ace-pq-authorization-v0",
                "buildZkAceAuthorizationProofV1",
                "stark-fri:zk_ace_pq_authorization_v0",
                "public-inputs".toByteArray(),
                ByteArray(PrivacyNativeBridge.PRIVACY_NATIVE_ARCHIVE_MAX_BYTES / 2 + 1),
                ByteArray(0),
            )
        }
    }

    @Test
    fun nativeAvailabilityProbeArchiveIsStableAndDefensive() {
        val first = PrivacyNativeBridge.privacyNativeAvailabilityProbeArchive()
        val second = PrivacyNativeBridge.privacyNativeAvailabilityProbeArchive()

        assertTrue(first !== second)
        assertTrue(first.contentEquals(privacyNoritoFrame(0x52)))
        assertTrue(PrivacyNativeBridge.isValidPrivacyNoritoArchive(first))
        assertFalse(
            first.contentEquals(
                "iroha-privacy-native-availability-probe-v1".toByteArray(),
            ),
        )
        first[0] = 0x7f
        assertTrue(second.contentEquals(privacyNoritoFrame(0x52)))
    }

    @Test
    fun nativeProbeRequiresAbiAndAllPrivacySymbols() {
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { privacyNoritoFrame(0x50) })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { privacyNoritoFrameWithPayload(0x51) })
        assertTrue(PrivacyNativeBridge.returnsOutputProbe(0x50) { privacyNoritoFrameWithPadding(0x50, 64) })
        val validProbeOutput = privacyNoritoFrameWithPayload(0x42)
        assertTrue(PrivacyNativeBridge.returnsOutputProbe(0x42) { validProbeOutput })
        assertAllZero(validProbeOutput)
        val invalidProbeOutput = invalidPrivacyNoritoPayloadTamper()
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { invalidProbeOutput })
        assertAllZero(invalidProbeOutput)
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { privacyNoritoFrame(0x50) })
        assertTrue(PrivacyNativeBridge.returnsOutputProbe(0x42) { privacyNoritoFrameWithPayload(0x42) })
        assertTrue(PrivacyNativeBridge.returnsOutputProbe(0x56) { privacyNoritoFrameWithPayload(0x56) })
        assertTrue(PrivacyNativeBridge.returnsOutputProbe(0x42) { privacyNoritoFrameWithFlags(0x42, 0x26) })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { privacyNoritoFrameWithPayload(0x42) })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x42) { privacyNoritoFrameWithPayload(0x56) })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x56) { privacyNoritoFrameWithPayload(0x50) })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { byteArrayOf(1) })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { invalidPrivacyNoritoFrame(0, 'X'.code) })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { invalidPrivacyNoritoFrame(4, 1) })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { invalidPrivacyNoritoFrame(5, 1) })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { invalidPrivacyNoritoFrame(22, 1) })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { invalidPrivacyNoritoDeclaredPayloadLength() })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { invalidPrivacyNoritoOversizedPayloadLength() })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { invalidPrivacyNoritoFrame(39, 0x40) })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { invalidPrivacyNoritoFrame(39, 0x20) })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { invalidPrivacyNoritoWithNonzeroPadding() })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { invalidPrivacyNoritoWithExcessivePadding() })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { invalidPrivacyNoritoFrame(31, 1) })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { invalidPrivacyNoritoPayloadTamper() })
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { ByteArray(0) })
        assertFalse(
            PrivacyNativeBridge.returnsOutputProbe(0x50) {
                ByteArray(PrivacyNativeBridge.PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1)
            },
        )
        assertFalse(PrivacyNativeBridge.returnsOutputProbe(0x50) { null })
        assertFalse(
            PrivacyNativeBridge.returnsOutputProbe(0x50) {
                throw UnsatisfiedLinkError("missing symbol")
            },
        )
        assertFalse(
            PrivacyNativeBridge.returnsOutputProbe(0x50) {
                throw IllegalArgumentException("bad probe")
            },
        )
        assertFalse(
            PrivacyNativeBridge.returnsOutputProbe(0x50) {
                throw SecurityException("blocked probe")
            },
        )
        assertFalse(
            PrivacyNativeBridge.returnsOutputProbe(0x50) {
                throw RuntimeException("unexpected probe failure")
            },
        )
        assertFalse(
            PrivacyNativeBridge.returnsOutputProbe(0x50) {
                throw LinkageError("bad linked bridge")
            },
        )

        assertTrue(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { 7 },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { 6 },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { 7 },
                probeSymbol = { false },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = { throw UnsatisfiedLinkError("missing bridge") },
                bridgeAbiVersion = { 6 },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = { throw IllegalArgumentException("bad library name") },
                bridgeAbiVersion = { 6 },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = { throw SecurityException("blocked library") },
                bridgeAbiVersion = { 6 },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = { throw RuntimeException("unexpected library failure") },
                bridgeAbiVersion = { 6 },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = { throw LinkageError("bad linked bridge") },
                bridgeAbiVersion = { 6 },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { throw UnsatisfiedLinkError("missing ABI symbol") },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { throw IllegalArgumentException("bad ABI") },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { throw SecurityException("blocked ABI") },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { throw RuntimeException("unexpected ABI failure") },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { throw LinkageError("bad ABI bridge") },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { 7 },
                probeSymbol = { throw UnsatisfiedLinkError("missing privacy symbol") },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { 7 },
                probeSymbol = { throw IllegalArgumentException("bad privacy probe") },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { 7 },
                probeSymbol = { throw SecurityException("blocked privacy probe") },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { 7 },
                probeSymbol = { throw RuntimeException("unexpected privacy probe") },
            ),
        )
        assertFalse(
            PrivacyNativeBridge.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { 7 },
                probeSymbol = { throw LinkageError("bad privacy bridge") },
            ),
        )
    }

    @Test
    fun rejectsNullAndEmptyNativeOutputs() {
        val missing = assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.requireNativeOutput(null, "privacy build proof")
        }
        assertTrue(missing.message.orEmpty().contains("returned no output"))

        val empty = assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.requireNativeOutput(ByteArray(0), "privacy verify proof")
        }
        assertTrue(empty.message.orEmpty().contains("returned empty output"))

        val emptyPayload = assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.requireNativeOutput(
                privacyNoritoFrame(0x50),
                "privacy capabilities",
            )
        }
        assertTrue(emptyPayload.message.orEmpty().contains("empty privacy result payload"))
        val emptyBuildPayload = assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.requireNativeOutput(
                privacyNoritoFrame(0x42),
                "privacy build proof",
            )
        }
        assertTrue(emptyBuildPayload.message.orEmpty().contains("empty privacy result payload"))
        val emptyVerifyPayload = assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.requireNativeOutput(
                privacyNoritoFrame(0x56),
                "privacy verify proof",
            )
        }
        assertTrue(emptyVerifyPayload.message.orEmpty().contains("empty privacy result payload"))

        val oversized = assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.requireNativeOutput(
                ByteArray(PrivacyNativeBridge.PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1),
                "privacy capabilities",
            )
        }
        assertTrue(oversized.message.orEmpty().contains("returned oversized output"))

        val output = privacyNoritoFrameWithPayload(0x50)
        val expectedOutput = privacyNoritoFrameWithPayload(0x50)
        val archive = PrivacyNativeBridge.requireNativeOutput(output, "privacy capabilities")
        assertTrue(archive !== output)
        assertTrue(archive.contentEquals(expectedOutput))
        assertAllZero(output)
        archive[0] = 9
        assertEquals('N'.code.toByte(), expectedOutput[0])
    }

    @Test
    fun rejectsInvalidNoritoNativeOutputs() {
        val oneByte = assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.requireNativeOutput(byteArrayOf(1), "privacy capabilities")
        }
        assertTrue(oneByte.message.orEmpty().contains("invalid Norito V1 archive"))

        for (invalid in listOf(
            invalidPrivacyNoritoFrame(0, 'X'.code),
            invalidPrivacyNoritoFrame(4, 1),
            invalidPrivacyNoritoFrame(5, 1),
            invalidPrivacyNoritoFrame(22, 1),
            invalidPrivacyNoritoDeclaredPayloadLength(0x42),
            invalidPrivacyNoritoOversizedPayloadLength(0x42),
            invalidPrivacyNoritoFrame(39, 0x40),
            invalidPrivacyNoritoFrame(39, 0x20),
            invalidPrivacyNoritoWithNonzeroPadding(),
            invalidPrivacyNoritoWithExcessivePadding(),
            invalidPrivacyNoritoFrame(31, 1),
            invalidPrivacyNoritoPayloadTamper(),
        )) {
            val error = assertFailsWith<IllegalStateException> {
                PrivacyNativeBridge.requireNativeOutput(invalid, "privacy build proof")
            }
            assertTrue(error.message.orEmpty().contains("invalid Norito V1 archive"))
        }
    }

    @Test
    fun rejectsWrongOperationSchemaNativeOutputs() {
        for ((label, expectedSchema, wrongSchemas) in listOf(
            Triple("privacy capabilities", 0x50, listOf(0x42, 0x56, 0x52)),
            Triple("privacy build proof", 0x42, listOf(0x50, 0x56, 0x52)),
            Triple("privacy verify proof", 0x56, listOf(0x50, 0x42, 0x52)),
        )) {
            val accepted = PrivacyNativeBridge.requireNativeOutput(
                privacyNoritoFrameWithPayload(expectedSchema),
                label,
            )
            assertTrue(accepted.contentEquals(privacyNoritoFrameWithPayload(expectedSchema)))
            for (mixedSchema in listOf(
                privacyNoritoFrameWithSchemaOverride(expectedSchema, 6, wrongSchemas.first()),
                privacyNoritoFrameWithSchemaOverride(expectedSchema, 21, wrongSchemas.first()),
            )) {
                val error = assertFailsWith<IllegalStateException> {
                    PrivacyNativeBridge.requireNativeOutput(mixedSchema, label)
                }
                assertTrue(error.message.orEmpty().contains("unexpected privacy result schema"))
            }

            for (wrongSchema in wrongSchemas) {
                val error = assertFailsWith<IllegalStateException> {
                    PrivacyNativeBridge.requireNativeOutput(
                        privacyNoritoFrameWithPayload(wrongSchema),
                        label,
                    )
                }
                assertTrue(error.message.orEmpty().contains("unexpected privacy result schema"))
            }
        }
    }

    @Test
    fun privacySchemaMatcherRequiresExplicitExpectedSchema() {
        val capabilities = privacyNoritoFrameWithPayload(0x50)

        assertFalse(PrivacyNativeBridge.hasPrivacyNoritoSchema(capabilities, -1))
        assertFalse(PrivacyNativeBridge.hasPrivacyNoritoSchema(capabilities, 0x42))
        assertTrue(PrivacyNativeBridge.hasPrivacyNoritoSchema(capabilities, 0x50))
    }

    @Test
    fun rejectsUnknownOperationSchemaNativeOutputs() {
        val missingSchemaError = assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.requireNativeOutput(
                privacyNoritoFrameWithPayload(0x50),
                "privacy capabilities",
                -1,
            )
        }
        assertTrue(missingSchemaError.message.orEmpty().contains("not a supported privacy native operation"))

        val outputError = assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.requireNativeOutput(
                privacyNoritoFrameWithPayload(0x50),
                "privacy forged operation",
            )
        }
        assertTrue(outputError.message.orEmpty().contains("not a supported privacy native operation"))

        var invoked = false
        val dispatchError = assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.call(
                label = "forged proof",
                requestArchive = privacyNoritoFrameWithPayload(0x52),
                nativeCall = {
                    invoked = true
                    privacyNoritoFrameWithPayload(0x42)
                },
                bridgeAvailable = true,
            )
        }
        assertTrue(dispatchError.message.orEmpty().contains("not a supported privacy native operation"))
        assertFalse(invoked, "unsupported privacy operations must not reach native dispatch")
    }

    @Test
    fun nativeDispatchReturnsDefensiveOutputCopy() {
        val nativeOutput = privacyNoritoFrameWithPayload(0x42)
        val expectedOutput = privacyNoritoFrameWithPayload(0x42)

        val archive = PrivacyNativeBridge.call(
            label = "build proof",
            requestArchive = privacyNoritoFrameWithPadding(0x52, 64),
            nativeCall = { nativeOutput },
            bridgeAvailable = true,
        )

        assertTrue(archive !== nativeOutput)
        assertTrue(archive.contentEquals(expectedOutput))
        assertAllZero(nativeOutput)

        archive[0] = 0x7f
        assertEquals('N'.code.toByte(), expectedOutput[0])
    }

    @Test
    fun acceptsCompleteFieldBitsetNoritoFlags() {
        val requestArchive = privacyNoritoFrameWithFlags(0x52, 0x26)
        val nativeOutput = privacyNoritoFrameWithFlags(0x42, 0x26)
        val expectedOutput = privacyNoritoFrameWithFlags(0x42, 0x26)

        val archive = PrivacyNativeBridge.call(
            label = "build proof",
            requestArchive = requestArchive,
            nativeCall = { request ->
                assertTrue(request.contentEquals(requestArchive))
                nativeOutput
            },
            bridgeAvailable = true,
        )

        assertTrue(archive.contentEquals(expectedOutput))
        assertAllZero(nativeOutput)
    }

    @Test
    fun nativeExceptionsAreSanitizedBeforeExposingRequestBytes() {
        val witness = "kotlin-sdk-private-witness-never-echo-18ea"
        val requestArchive = privacyNoritoFrameWithPayload(0x52)
        var buildRequest: ByteArray? = null
        var verifyRequest: ByteArray? = null

        val capabilitiesError = assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.invokeNativeOutput("privacy capabilities") {
                throw RuntimeException("native panic included $witness")
            }
        }
        assertSanitized(capabilitiesError, "privacy capabilities failed", witness)

        val buildError = assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.call(
                label = "build proof",
                requestArchive = requestArchive,
                nativeCall = { request ->
                    buildRequest = request
                    assertTrue(request !== requestArchive)
                    assertTrue(request.contentEquals(requestArchive))
                    throw RuntimeException("native panic included $witness")
                },
                bridgeAvailable = true,
            )
        }
        assertSanitized(buildError, "privacy build proof failed", witness)

        val verifyError = assertFailsWith<IllegalStateException> {
            PrivacyNativeBridge.call(
                label = "verify proof",
                requestArchive = requestArchive,
                nativeCall = { request ->
                    verifyRequest = request
                    assertTrue(request !== requestArchive)
                    assertTrue(request.contentEquals(requestArchive))
                    throw UnsatisfiedLinkError("native panic included $witness")
                },
                bridgeAvailable = true,
            )
        }
        assertSanitized(verifyError, "privacy verify proof failed", witness)
        assertAllZero(buildRequest)
        assertAllZero(verifyRequest)
        assertTrue(requestArchive.contentEquals(privacyNoritoFrameWithPayload(0x52)))
    }

    @Test
    fun nativeDispatchClearsTemporaryRequestCopyWithoutMutatingCallerArchive() {
        val requestArchive = privacyNoritoFrameWithPayload(0x52)
        val originalArchive = requestArchive.copyOf()
        var buildRequest: ByteArray? = null
        var verifyRequest: ByteArray? = null

        val buildOutput = PrivacyNativeBridge.call(
            label = "build proof",
            requestArchive = requestArchive,
            nativeCall = { request ->
                buildRequest = request
                assertTrue(request !== requestArchive)
                assertTrue(request.contentEquals(originalArchive))
                privacyNoritoFrameWithPayload(0x42)
            },
            bridgeAvailable = true,
        )
        assertTrue(buildOutput.contentEquals(privacyNoritoFrameWithPayload(0x42)))

        val verifyOutput = PrivacyNativeBridge.call(
            label = "verify proof",
            requestArchive = requestArchive,
            nativeCall = { request ->
                verifyRequest = request
                assertTrue(request !== requestArchive)
                assertTrue(request.contentEquals(originalArchive))
                privacyNoritoFrameWithPayload(0x56)
            },
            bridgeAvailable = true,
        )
        assertTrue(verifyOutput.contentEquals(privacyNoritoFrameWithPayload(0x56)))

        assertTrue(requestArchive.contentEquals(originalArchive))
        assertAllZero(buildRequest)
        assertAllZero(verifyRequest)
    }

    @Test
    fun hostileNativeRequestMutationCannotMutateCallerArchive() {
        val requestArchive = privacyNoritoFrameWithPayload(0x52)
        val originalArchive = requestArchive.copyOf()
        var buildRequest: ByteArray? = null
        var verifyRequest: ByteArray? = null

        val buildOutput = PrivacyNativeBridge.call(
            label = "build proof",
            requestArchive = requestArchive,
            nativeCall = { request ->
                buildRequest = request
                request[0] = 0x00.toByte()
                request[6] = 0x7f.toByte()
                privacyNoritoFrameWithPayload(0x42)
            },
            bridgeAvailable = true,
        )
        assertTrue(buildOutput.contentEquals(privacyNoritoFrameWithPayload(0x42)))

        val verifyOutput = PrivacyNativeBridge.call(
            label = "verify proof",
            requestArchive = requestArchive,
            nativeCall = { request ->
                verifyRequest = request
                request[0] = 0x00.toByte()
                request[6] = 0x7f.toByte()
                privacyNoritoFrameWithPayload(0x56)
            },
            bridgeAvailable = true,
        )
        assertTrue(verifyOutput.contentEquals(privacyNoritoFrameWithPayload(0x56)))

        assertTrue(requestArchive.contentEquals(originalArchive))
        assertAllZero(buildRequest)
        assertAllZero(verifyRequest)
    }

    @Test
    fun rejectsInvalidNoritoRequestsBeforeNativeDispatch() {
        val emptyBuildRequest = assertFailsWith<IllegalArgumentException> {
            PrivacyNativeBridge.call(
                label = "build proof",
                requestArchive = privacyNoritoFrame(0x52),
                nativeCall = {
                    throw AssertionError("empty-payload build request must not reach native dispatch")
                },
                bridgeAvailable = true,
            )
        }
        assertTrue(
            emptyBuildRequest.message.orEmpty()
                .contains("requestArchive must contain a non-empty privacy request payload"),
        )
        val emptyVerifyRequest = assertFailsWith<IllegalArgumentException> {
            PrivacyNativeBridge.call(
                label = "verify proof",
                requestArchive = privacyNoritoFrame(0x52),
                nativeCall = {
                    throw AssertionError("empty-payload verify request must not reach native dispatch")
                },
                bridgeAvailable = true,
            )
        }
        assertTrue(
            emptyVerifyRequest.message.orEmpty()
                .contains("requestArchive must contain a non-empty privacy request payload"),
        )
        for (malformedArchive in invalidPrivacyRequestArchives()) {
            val buildError = assertFailsWith<IllegalArgumentException> {
                PrivacyNativeBridge.call(
                    label = "build proof",
                    requestArchive = malformedArchive.copyOf(),
                    nativeCall = {
                        throw AssertionError("invalid build request must not reach native dispatch")
                    },
                    bridgeAvailable = true,
                )
            }
            assertTrue(
                buildError.message.orEmpty().contains("requestArchive must be a valid Norito V1 archive"),
            )
            val verifyError = assertFailsWith<IllegalArgumentException> {
                PrivacyNativeBridge.call(
                    label = "verify proof",
                    requestArchive = malformedArchive.copyOf(),
                    nativeCall = {
                        throw AssertionError("invalid verify request must not reach native dispatch")
                    },
                    bridgeAvailable = true,
                )
            }
            assertTrue(
                verifyError.message.orEmpty().contains("requestArchive must be a valid Norito V1 archive"),
            )
        }
    }

    @Test
    fun rejectsWrongSchemaRequestsBeforeNativeDispatch() {
        for (forgedRequest in wrongSchemaPrivacyRequestArchives()) {
            val buildError = assertFailsWith<IllegalArgumentException> {
                PrivacyNativeBridge.call(
                    label = "build proof",
                    requestArchive = forgedRequest.copyOf(),
                    nativeCall = {
                        throw AssertionError("wrong-schema build request must not reach native dispatch")
                    },
                    bridgeAvailable = true,
                )
            }
            assertTrue(
                buildError.message.orEmpty().contains("requestArchive must use the privacy request schema"),
            )
            val verifyError = assertFailsWith<IllegalArgumentException> {
                PrivacyNativeBridge.call(
                    label = "verify proof",
                    requestArchive = forgedRequest.copyOf(),
                    nativeCall = {
                        throw AssertionError("wrong-schema verify request must not reach native dispatch")
                    },
                    bridgeAvailable = true,
                )
            }
            assertTrue(
                verifyError.message.orEmpty().contains("requestArchive must use the privacy request schema"),
            )
        }
    }

    private fun assertFailClosedProductionGate(
        capabilities: PrivacyNativeBridge.PrivacyCapabilities,
    ) {
        assertFalse(capabilities.productionReady)
        assertEquals(PrivacyNativeBridge.PRODUCTION_GATE_VERSION, capabilities.productionGate.version)
        assertFalse(capabilities.productionGate.ready)
        assertFalse(capabilities.productionGate.realProving)
        assertFalse(capabilities.productionGate.realVerification)
        assertFalse(capabilities.productionGate.chainAdmission)
        assertFalse(capabilities.productionGate.sdkParity)
        assertFalse(capabilities.productionGate.walletState)
        assertFalse(capabilities.productionGate.witnessPrivacyChecks)
        assertFalse(capabilities.productionGate.deterministicTests)
        assertFalse(capabilities.productionGate.negativeAdversarialTests)
        assertFalse(capabilities.productionGate.replayNullifierTests)
        assertFalse(capabilities.productionGate.fuzzing)
        assertFalse(capabilities.productionGate.parserFuzzing)
        assertFalse(capabilities.productionGate.verifierFuzzing)
        assertFalse(capabilities.productionGate.performanceGates)
        assertFalse(capabilities.productionGate.externalAudit)
        assertEquals(emptyList(), capabilities.productionGate.auditReferences)
        assertEquals(
            PrivacyNativeBridge.PrivacyProductionGate.REQUIRED_GATES,
            capabilities.productionGate.requiredGates,
        )
        assertEquals(
            PrivacyNativeBridge.PrivacyProductionGate.MISSING_REASONS,
            capabilities.productionGate.missing,
        )
        assertTrue(
            capabilities.productionGate.missing.contains(
                "real proving engine is not registered",
            ),
        )
        assertTrue(
            capabilities.productionGate.missing.contains(
                "chain admission path is not enabled",
            ),
        )
        assertTrue(
            capabilities.productionGate.missing.contains(
                "witness privacy checks are incomplete",
            ),
        )
        assertTrue(
            capabilities.productionGate.missing.contains(
                "negative/adversarial tests are incomplete",
            ),
        )
        assertTrue(
            capabilities.productionGate.missing.contains(
                "replay/nullifier rejection tests are incomplete",
            ),
        )
        assertTrue(
            capabilities.productionGate.missing.contains(
                "parser fuzzing gate is incomplete",
            ),
        )
        assertTrue(
            capabilities.productionGate.missing.contains(
                "verifier fuzzing gate is incomplete",
            ),
        )
        assertTrue(
            capabilities.productionGate.missing.contains(
                "internal cryptographic review signoff is missing",
            ),
        )
        assertTrue(
            capabilities.productionGate.missing.contains(
                "implementation stage is not production-hardened",
            ),
        )
        assertTrue(
            capabilities.productionGate.missing.contains(
                "planned SDK entrypoints remain",
            ),
        )
        assertTrue(
            capabilities.productionGate.missing.contains(
                "dev fixture entrypoints are not production entrypoints",
            ),
        )
        assertTrue(
            capabilities.productionGate.missing.contains(
                "Iroha production allowlist is not enabled for this audited row",
            ),
        )
    }

    private fun assertSanitized(
        error: IllegalStateException,
        message: String,
        witness: String,
    ) {
        assertEquals(message, error.message)
        assertEquals(null, error.cause)
        assertFalse(error.message.orEmpty().contains(witness))
        assertFalse(error.toString().contains(witness))
    }

    private fun assertAllZero(bytes: ByteArray?) {
        assertTrue(bytes != null)
        assertTrue(bytes.all { it == 0.toByte() })
    }

    private fun privacyNoritoFrame(schemaByte: Int): ByteArray {
        val frame = ByteArray(40)
        frame[0] = 'N'.code.toByte()
        frame[1] = 'R'.code.toByte()
        frame[2] = 'T'.code.toByte()
        frame[3] = '0'.code.toByte()
        frame.fill(schemaByte.toByte(), 6, 22)
        return frame
    }

    private fun privacyNoritoFrameWithPayload(schemaByte: Int): ByteArray {
        val frame = privacyNoritoFrame(schemaByte).copyOf(45)
        frame[23] = 3
        val checksum = byteArrayOf(
            0xb9.toByte(),
            0xd3.toByte(),
            0xa8.toByte(),
            0x0c.toByte(),
            0xcd.toByte(),
            0x5d.toByte(),
            0x13.toByte(),
            0x24.toByte(),
        )
        checksum.copyInto(frame, destinationOffset = 31)
        frame[42] = 0xa5.toByte()
        frame[43] = 0x5a.toByte()
        frame[44] = 0x11.toByte()
        return frame
    }

    private fun privacyNoritoFrameWithPadding(
        schemaByte: Int,
        paddingLength: Int,
    ): ByteArray {
        val frame = privacyNoritoFrame(schemaByte).copyOf(43 + paddingLength)
        frame[23] = 3
        val checksum = byteArrayOf(
            0xb9.toByte(),
            0xd3.toByte(),
            0xa8.toByte(),
            0x0c.toByte(),
            0xcd.toByte(),
            0x5d.toByte(),
            0x13.toByte(),
            0x24.toByte(),
        )
        checksum.copyInto(frame, destinationOffset = 31)
        frame[40 + paddingLength] = 0xa5.toByte()
        frame[41 + paddingLength] = 0x5a.toByte()
        frame[42 + paddingLength] = 0x11.toByte()
        return frame
    }

    private fun privacyNoritoFrameWithSchemaOverride(
        schemaByte: Int,
        offset: Int,
        value: Int,
    ): ByteArray {
        val frame = privacyNoritoFrameWithPayload(schemaByte)
        frame[offset] = value.toByte()
        return frame
    }

    private fun privacyNoritoFrameWithDeclaredPayloadLength(
        schemaByte: Int,
        payloadLength: Long,
    ): ByteArray {
        val frame = privacyNoritoFrameWithPayload(schemaByte)
        for (index in 0 until 8) {
            frame[23 + index] = ((payloadLength ushr (8 * index)) and 0xffL).toByte()
        }
        return frame
    }

    private fun privacyNoritoFrameWithFlags(schemaByte: Int, flags: Int): ByteArray {
        val frame = privacyNoritoFrameWithPayload(schemaByte)
        frame[39] = flags.toByte()
        return frame
    }

    private fun invalidPrivacyNoritoFrame(offset: Int, value: Int): ByteArray {
        val frame = privacyNoritoFrame(0x50)
        frame[offset] = value.toByte()
        return frame
    }

    private fun invalidPrivacyNoritoDeclaredPayloadLength(schemaByte: Int = 0x50): ByteArray =
        privacyNoritoFrameWithDeclaredPayloadLength(schemaByte, 6L)

    private fun invalidPrivacyNoritoOversizedPayloadLength(schemaByte: Int = 0x50): ByteArray =
        privacyNoritoFrameWithDeclaredPayloadLength(schemaByte, Long.MIN_VALUE)


    private fun invalidPrivacyNoritoWithNonzeroPadding(): ByteArray {
        val frame = privacyNoritoFrame(0x50).copyOf(41)
        frame[40] = 1
        return frame
    }

    private fun invalidPrivacyNoritoWithExcessivePadding(): ByteArray =
        privacyNoritoFrameWithPadding(0x50, 65)

    private fun invalidPrivacyNoritoPayloadTamper(): ByteArray {
        val frame = privacyNoritoFrameWithPayload(0x50)
        frame[44] = (frame[44].toInt() xor 0x7f).toByte()
        return frame
    }

    private fun invalidPrivacyRequestArchives(): List<ByteArray> = listOf(
        byteArrayOf(1),
        invalidPrivacyNoritoFrame(0, 'X'.code),
        invalidPrivacyNoritoFrame(4, 1),
        invalidPrivacyNoritoFrame(5, 1),
        invalidPrivacyNoritoFrame(22, 1),
        invalidPrivacyNoritoDeclaredPayloadLength(0x52),
        invalidPrivacyNoritoOversizedPayloadLength(0x52),
        invalidPrivacyNoritoFrame(39, 0x40),
        invalidPrivacyNoritoFrame(39, 0x20),
        invalidPrivacyNoritoWithNonzeroPadding(),
        invalidPrivacyNoritoWithExcessivePadding(),
        invalidPrivacyNoritoFrame(31, 1),
        invalidPrivacyNoritoPayloadTamper(),
    )

    private fun wrongSchemaPrivacyRequestArchives(): List<ByteArray> = listOf(
        privacyNoritoFrameWithPayload(0x50),
        privacyNoritoFrameWithPayload(0x42),
        privacyNoritoFrameWithPayload(0x56),
        privacyNoritoFrameWithSchemaOverride(0x52, 6, 0x42),
        privacyNoritoFrameWithSchemaOverride(0x52, 21, 0x56),
    )
}
