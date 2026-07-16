package org.hyperledger.iroha.sdk.offline

import java.net.URI
import java.util.concurrent.CompletableFuture
import java.util.concurrent.atomic.AtomicReference
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import kotlin.test.assertFailsWith
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.client.transport.TransportExecutor

class KagemushaRecursiveSpendProverTest {
    @Test
    fun exactAbi20IsRequired() {
        assertTrue(KagemushaRecursiveSpendProver.isExactBridgeAbi(20))
        assertFalse(KagemushaRecursiveSpendProver.isExactBridgeAbi(19))
        assertTrue(
            KagemushaRecursiveSpendProver.detectExactNativeAvailability(
                loadLibrary = {},
                abiVersion = { 20 },
                symbolProbe = { true },
            ),
        )
    }

    @Test
    fun artifactContractAndInventoryAreCurrentOnly() {
        assertEquals(20, KagemushaRecursiveSpendProver.REQUIRED_NATIVE_BRIDGE_ABI_VERSION)
        assertEquals(8, KagemushaRecursiveSpendProver.ARTIFACT_COUNT)
        assertEquals(2, KagemushaRecursiveSpendProver.MAXIMUM_INPUTS_PER_TRANSITION)
        assertEquals(2, KagemushaRecursiveSpendProver.MAXIMUM_LOCAL_APPEND_BUILDER_INPUTS)
        assertEquals(2, KagemushaRecursiveSpendProver.MAXIMUM_BRANCH_CLAIMS)
        assertEquals(8, KagemushaRecursiveSpendProver.MAXIMUM_PEER_HOPS)
        assertEquals(32 * 1024, KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES)
        assertEquals(9_211, KagemushaRecursiveSpendProver.MAX_PEER_TEXT_ARCHIVE_BYTES)
        assertEquals(16, KagemushaRecursiveSpendProver.CONFIDENTIAL_TREE_DEPTH)
        assertEquals(
            "kagemusha.offline.recursive_spend.artifact_manifest.v4",
            KagemushaRecursiveSpendProver.ARTIFACT_MANIFEST_SCHEMA,
        )
        assertEquals(
            listOf(
                "step-eq.parameters.krv4",
                "step-eq.proving-key.krv4",
                "step-eq.verifying-key.krv4",
                "step-eq.bootstrap-witness.krv4",
                "step-ep.parameters.krv4",
                "step-ep.proving-key.krv4",
                "step-ep.verifying-key.krv4",
                "step-ep.bootstrap-witness.krv4",
            ),
            KagemushaRecursiveSpendProver.ARTIFACT_FILES,
        )
        val installFactory = KagemushaRecursiveSpendProver::class.java.getDeclaredMethod(
            "beginArtifactInstallSession",
            ByteArray::class.java,
            ByteArray::class.java,
            KagemushaRecursiveSpendProver.ReleaseAuthentication::class.java,
        )
        assertEquals(3, installFactory.parameterCount)
        val nativeInstall = KagemushaRecursiveSpendProver::class.java.getDeclaredMethod(
            "nativeArtifactSetInstallV4",
            ByteArray::class.java,
            ByteArray::class.java,
            ByteArray::class.java,
            ByteArray::class.java,
            ByteArray::class.java,
            ByteArray::class.java,
            LongArray::class.java,
        )
        assertEquals(7, nativeInstall.parameterCount)
        val methods = KagemushaRecursiveSpendProver::class.java.declaredMethods
            .filter {
                java.lang.reflect.Modifier.isPublic(it.modifiers) &&
                    !it.isSynthetic &&
                    !it.name.startsWith("access\$")
            }
            .map { it.name }
            .toSet()
        assertEquals(
            setOf(
                "beginArtifactIngest",
                "beginArtifactInstallSession",
                "appendSpendV4",
                "buildAppendRequestV4",
                "buildInitRequestV4",
                "buildRedeemV4",
                "buildRedeemRequestV4",
                "buildVerifyRequestV4",
                "decodeAppendRequestV4",
                "decodeBundleV4",
                "decodeInitRequestV4",
                "decodeInitResultV4",
                "decodeNoteMembershipWitness",
                "decodeNoteOpening",
                "decodePeerPayment",
                "decodeRedeemRequestV4",
                "decodeReceiverAcknowledgement",
                "decodeRecipientPaymentRequest",
                "decodeRedeemBuildResultV4",
                "decodeRedeemSubmissionRequest",
                "decodeSplitResultV4",
                "decodeTopUpAnchorV4",
                "decodeTopUpFinalityEvidenceV4",
                "decodeTopUpRequest",
                "decodeVerifyRequestV4",
                "decodeVerifyResultV4",
                "decodeTopUpFinalityRosterArtifact",
                "finalizeRedeemV4",
                "finalizeTopUp",
                "initSpendV4",
                "isArtifactStreamingAvailable",
                "isProofBackendAvailable",
                "newToriiClient",
                "prepareAcknowledgement",
                "prepareNoteOpening",
                "prepareRecipientPaymentRequest",
                "prepareRequestAuthorization",
                "prepareTopUp",
                "projectOperationStatus",
                "projectPeerPayment",
                "projectRecipientPaymentRequest",
                "projectReadiness",
                "restoreSpendableBranchV4",
                "signAcknowledgement",
                "signRecipientPaymentRequest",
                "signRequestAuthorization",
                "verifyAcknowledgement",
                "verifyRecipientPaymentRequest",
                "verifySpendV4",
            ),
            methods,
        )
        val declaredNames = KagemushaRecursiveSpendProver::class.java.declaredMethods
            .map { it.name }
            .toSet()
        for (retired in listOf(
            "projectInitResult",
            "restoreSpendableBranch",
            "buildAppendRequest",
            "buildInitRequest",
            "buildRedeemRequest",
            "buildVerifyRequest",
            "nativeProjectInitResultV2",
            "nativeRestoreSpendableBranchV2",
        )) {
            assertFalse(retired in declaredNames, "$retired must remain absent from the exact-state JVM surface")
        }
        val appendBuilder = KagemushaRecursiveSpendProver::class.java.declaredMethods.single {
            it.name == "buildAppendRequestV4" && java.lang.reflect.Modifier.isPublic(it.modifiers)
        }
        assertEquals(java.util.List::class.java, appendBuilder.parameterTypes[0])
        val branchMethods = KagemushaRecursiveSpendProver.BranchProjection::class.java.declaredMethods
            .map { it.name }
            .toSet()
        assertTrue("branchClaims" in branchMethods)
        assertTrue("bundleDigest" in branchMethods)
        assertFalse("branchClaim" in branchMethods)
        assertFalse("branchClaimDigest" in branchMethods)
        assertFalse("parentBranchClaimDigest" in branchMethods)
        for (name in listOf(
            "decodeAppendRequestV4",
            "decodeSplitResultV4",
            "decodeRedeemRequestV4",
            "decodeRedeemBuildResultV4",
        )) {
            val candidates = KagemushaRecursiveSpendProver::class.java.declaredMethods
                .filter { it.name == name && java.lang.reflect.Modifier.isPublic(it.modifiers) }
            assertEquals(1, candidates.size)
            assertTrue(
                candidates.single().parameterTypes.contentEquals(
                    arrayOf(ByteArray::class.java, KagemushaRecursiveSpendProver.NoteOpening::class.java),
                ),
                "$name must restore its optional persisted change opening explicitly",
            )
        }
        val appendNative = KagemushaRecursiveSpendProver::class.java.declaredMethods
            .single { it.name == "nativeBuildAppendRequestV4" }
        assertTrue(appendNative.parameterTypes.take(3).all { it == Array<ByteArray>::class.java })
        assertEquals(
            1,
            KagemushaRecursiveSpendProver::class.java.declaredMethods.count {
                it.name == "buildAppendRequestV4" && java.lang.reflect.Modifier.isPublic(it.modifiers)
            },
        )
    }

    @Test
    fun artifactRoleInventoryRejectsCountsDuplicatesAndReordering() {
        val canonical = KagemushaRecursiveSpendProver.ArtifactRoleV4.entries
        KagemushaRecursiveSpendProver.requireCanonicalV4ArtifactRoleInventory(canonical)

        for (count in listOf(6, 7, 9)) {
            val invalid = List(count) { canonical[it % canonical.size] }
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver
                    .requireCanonicalV4ArtifactRoleInventory(invalid)
            }
        }

        val duplicate = canonical.toMutableList().also { it[1] = it[0] }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver
                .requireCanonicalV4ArtifactRoleInventory(duplicate)
        }

        val reordered = canonical.toMutableList().also {
            val first = it[0]
            it[0] = it[1]
            it[1] = first
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver
                .requireCanonicalV4ArtifactRoleInventory(reordered)
        }
    }

    @Test
    fun releaseAuthenticationIsMandatoryAndBounded() {
        val one = byteArrayOf(1)
        KagemushaRecursiveSpendProver.ReleaseAuthentication(one, one, one, one)
        repeat(4) { emptyIndex ->
            val values = Array(4) { one }
            values[emptyIndex] = byteArrayOf()
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.ReleaseAuthentication(
                    values[0], values[1], values[2], values[3],
                )
            }
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.ReleaseAuthentication(
                ByteArray(KagemushaRecursiveSpendProver.MAX_TRUSTED_RELEASE_POLICY_BYTES + 1),
                one,
                one,
                one,
            )
        }
    }

    @Test
    fun exactStateProjectionModelCarriesArtifactClaimsAndRejectsTampering() {
        val mutableClaims = mutableListOf(
            KagemushaRecursiveSpendProver.BranchClaim(
                archive("KagemushaRecursiveSpendBranchClaimV2"),
            ),
        )
        val projection = KagemushaRecursiveSpendProver.BranchProjection(
            KagemushaRecursiveSpendProver.Bundle(
                archive("KagemushaRecursiveSpendBundleV2"),
            ),
            KagemushaRecursiveSpendProver.NoteMembershipWitness(
                archive("KagemushaNoteMembershipWitnessV2"),
            ),
            ByteArray(32) { 1 },
            ByteArray(32) { 2 },
            KagemushaScaledAmount.fromAtomicUnits("7", 0),
            1,
            2,
            ByteArray(32) { 3 },
            KagemushaRecursiveSpendProver.ArtifactBinding(
                archive("KagemushaRecursiveSpendArtifactBindingV3"),
            ),
            mutableClaims,
        )
        mutableClaims.clear()
        assertEquals(1, projection.branchClaims.size)
        assertEquals(2, projection.proofStepCount)
        val digest = projection.bundleDigest()
        digest.fill(0)
        assertTrue(projection.bundleDigest().all { it == 3.toByte() })

        val methods = KagemushaRecursiveSpendProver.BranchProjection::class.java.methods
            .map { it.name }
            .toSet()
        assertTrue(setOf("getArtifactBinding", "getBranchClaims", "bundleDigest").all(methods::contains))
        assertFalse("parentBranchClaimDigest" in methods)
        assertFalse("branchClaimDigest" in methods)

        val tampered = archive("KagemushaRecursiveSpendArtifactBindingV3")
        tampered[tampered.lastIndex] = (tampered.last().toInt() xor 1).toByte()
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.ArtifactBinding(tampered)
        }
    }

    @Test
    fun appendJoinRejectsZeroThreeAndDuplicateInputsBeforeNativeDispatch() {
        val first = spendableBranch(0x21)
        val second = spendableBranch(0x31)
        val third = spendableBranch(0x41)
        val verifier = ByteArray(32) { 0x61 }
        val operation = ByteArray(32) { 0x62 }
        val outputMembershipPaths = outputMembershipPaths()

        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.buildAppendRequestV4(
                emptyList(), null, outputMembershipPaths, verifier, operation, 1,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.buildAppendRequestV4(
                listOf(first, second, third), null, outputMembershipPaths, verifier, operation, 1,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.buildAppendRequestV4(
                listOf(first, first), null, outputMembershipPaths, verifier, operation, 1,
            )
        }
    }

    @Test
    fun lifecycleArchivesAreTypedDefensiveAndFailClosed() {
        val initBytes = archive("KagemushaRecursiveSpendInitLocalRequestV4")
        val init = KagemushaRecursiveSpendProver.decodeInitRequestV4(initBytes)
        initBytes[initBytes.lastIndex] = 0
        assertEquals(0x51, init.noritoEncoded().last().toInt() and 0xff)

        val append = KagemushaRecursiveSpendProver.decodeAppendRequestV4(
            archive("KagemushaRecursiveSpendAppendLocalRequestV4"),
            null,
        )
        val verify = KagemushaRecursiveSpendProver.decodeVerifyRequestV4(
            archive("KagemushaRecursiveSpendVerifyLocalRequestV4"),
        )
        val redeem = KagemushaRecursiveSpendProver.decodeRedeemRequestV4(
            archive("KagemushaRecursiveSpendRedeemLocalRequestV4"),
            null,
        )
        val topUpSubmission = KagemushaRecursiveSpendProver.decodeTopUpRequest(
            archive("iroha.torii.v1.offline.top_up.request"),
        )
        val redeemSubmission = KagemushaRecursiveSpendProver.decodeRedeemSubmissionRequest(
            archive("iroha.torii.v1.offline.redeem.request"),
        )
        val opening = KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2"),
        )
        assertTrue(append.noritoEncoded().isNotEmpty())
        assertTrue(verify.noritoEncoded().isNotEmpty())
        assertTrue(redeem.noritoEncoded().isNotEmpty())
        assertTrue(topUpSubmission.noritoEncoded().isNotEmpty())
        assertTrue(redeemSubmission.noritoEncoded().isNotEmpty())
        assertTrue(opening.noritoEncoded().isNotEmpty())
        assertTrue(
            KagemushaRecursiveSpendProver.decodeInitResultV4(
                archive("KagemushaRecursiveSpendInitResultV4"),
            ).noritoEncoded().isNotEmpty(),
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.decodeVerifyRequestV4(
                archive("KagemushaRecursiveSpendInitLocalRequestV4"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.appendSpendV4(
                append,
                KagemushaRecursiveSpendProver.decodeRecipientPaymentRequest(
                    archive("KagemushaRecipientPaymentRequestV2"),
                ),
                0,
            )
        }
        if (!KagemushaRecursiveSpendProver.isProofBackendAvailable()) {
            assertFailsWith<IllegalStateException> {
                KagemushaRecursiveSpendProver.initSpendV4(init)
            }
        }
        append.close()
        redeem.close()
        assertTrue(append.isDestroyed())
        assertTrue(redeem.isDestroyed())
        assertFailsWith<IllegalStateException> { append.noritoEncoded() }
    }

    @Test
    fun topUpZeroPathIsTypedDefensiveAndExact() {
        val directions = ByteArray(16)
        directions[0] = 1
        directions[2] = 1
        val siblings = List(16) { ByteArray(32) { (it + 1).toByte() } }
        val path = KagemushaRecursiveSpendProver.TopUpZeroPath(
            5,
            siblings,
            directions,
            ByteArray(32) { 0x51 },
        )
        directions.fill(0)
        siblings[0].fill(0)
        assertEquals(5, path.leafIndex)
        assertEquals(1, path.directions()[0].toInt())
        assertEquals(1, path.siblings()[0][0].toInt())
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.TopUpZeroPath(
                5,
                List(16) { ByteArray(32) },
                ByteArray(16),
                ByteArray(32) { 1 },
            )
        }
    }

    @Test
    fun readinessRequiresEveryVerifierRoleAtOneSnapshot() {
        fun verifier(withdrawalHeight: Long? = null) =
            KagemushaRecursiveSpendProver.ActiveVerifier(
                "halo2/ipa",
                "kagemusha-role",
                1,
                "kagemusha-circuit",
                ByteArray(32) { 1 },
                ByteArray(32) { 2 },
                12 * 1024,
                10,
                withdrawalHeight,
            )
        fun readiness(
            transfer: KagemushaRecursiveSpendProver.ActiveVerifier? = verifier(),
            unshield: KagemushaRecursiveSpendProver.ActiveVerifier? = verifier(),
        ) = KagemushaRecursiveSpendProver.ReadinessProjection(
            19,
            8,
            "pkr#sbp",
            9,
            20,
            ByteArray(32) { 3 },
            true,
            true,
            true,
            transfer,
            verifier(),
            unshield,
            verifier(),
            verifier(),
            emptyList(),
        )

        assertTrue(readiness().allVerifiersActive)
        assertTrue(readiness().chainArtifactSetReady)
        assertFalse(readiness(transfer = null).allVerifiersActive)
        assertFalse(readiness(unshield = verifier(withdrawalHeight = 20)).chainArtifactSetReady)
    }

    @Test
    fun scaledAmountsAreExactAndNeverRound() {
        val amount = KagemushaScaledAmount.fromDecimal("10.75", 9)
        assertEquals("10750000000", amount.atomicUnits)
        assertEquals("10.750000000", amount.scaledNumericDecimal)
        assertEquals("10.75", amount.displayDecimal)
        assertEquals(
            "10750000000",
            KagemushaScaledAmount.sum(
                listOf(
                    KagemushaScaledAmount.fromDecimal("4.50", 9),
                    KagemushaScaledAmount.fromDecimal("6.25", 9),
                ),
            ).atomicUnits,
        )
        assertEquals(
            "0.000000001",
            KagemushaScaledAmount.fromAtomicUnits("1", 9).scaledNumericDecimal,
        )
        assertEquals(
            KagemushaScaledAmount.MAXIMUM_ATOMIC_UNITS,
            KagemushaScaledAmount.fromAtomicUnits(
                KagemushaScaledAmount.MAXIMUM_ATOMIC_UNITS,
                28,
            ).atomicUnits,
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaScaledAmount.fromDecimal("1.001", 2)
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaScaledAmount.fromAtomicUnits(
                "340282366920938463463374607431768211456",
                9,
            )
        }
    }

    @Test
    fun canonicalPeerCodecsAreTypedAndDefensive() {
        val requestArchive = archive("KagemushaRecipientPaymentRequestV2")
        val request = KagemushaRecursiveSpendProver.decodeRecipientPaymentRequest(requestArchive)
        requestArchive[requestArchive.lastIndex] = 0
        assertEquals(0x51, request.noritoEncoded().last().toInt() and 0xff)

        assertTrue(
            KagemushaRecursiveSpendProver.decodePeerPayment(
                archive("KagemushaRecursiveSpendPeerPaymentV2"),
            ).noritoEncoded().size > NoritoHeader.HEADER_LENGTH,
        )
        assertTrue(
            KagemushaRecursiveSpendProver.decodeReceiverAcknowledgement(
                archive("KagemushaReceiverAcknowledgementV2"),
            ).noritoEncoded().size > NoritoHeader.HEADER_LENGTH,
        )
        assertTrue(
            KagemushaRecursiveSpendProver.decodeNoteMembershipWitness(
                archive("KagemushaNoteMembershipWitnessV2"),
            ).noritoEncoded().size > NoritoHeader.HEADER_LENGTH,
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.decodePeerPayment(
                archive("KagemushaRecipientPaymentRequestV2"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.decodePeerPayment(byteArrayOf(1, 2, 3))
        }
        val corrupted = archive("KagemushaRecursiveSpendPeerPaymentV2")
        corrupted[corrupted.lastIndex] = 0
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.decodePeerPayment(corrupted)
        }
    }

    @Test
    fun peerTransportGoldenVectorsAreExact() {
        val request = KagemushaPeerPayload.decode(
            archive("KagemushaRecipientPaymentRequestV2"),
            KagemushaPeerPayloadKind.RECEIVE_REQUEST,
        )
        val text = KagemushaPeerTextCodec.encode(request)

        assertEquals(
            "PKK2R.TlJUMAAA27ZYXi51qDW87RkAOqt6zQABAAAAAAAAAN6BMN0_Z661AlE",
            text,
        )
        assertEquals(
            KagemushaPeerPayloadKind.RECEIVE_REQUEST,
            KagemushaPeerTextCodec.decode(text).kind,
        )
        assertEquals(
            KagemushaPeerPayloadKind.RECEIVE_REQUEST,
            KagemushaPeerTextCodec.decodeUserPresented(" \n$text\t").kind,
        )
        assertEquals("PKK2R.", KagemushaPeerTransportContract.RECEIVE_REQUEST_TEXT_PREFIX)
        assertEquals("PKK2P.", KagemushaPeerTransportContract.PAYMENT_TEXT_PREFIX)
        assertEquals("PKK2A.", KagemushaPeerTransportContract.ACKNOWLEDGEMENT_TEXT_PREFIX)
        assertEquals("PKKQ1.", KagemushaPeerTransportContract.QR_STREAM_TEXT_PREFIX)
    }

    @Test
    fun qrNfcAndNearbyGoldenVectorsAreExact() {
        val request = KagemushaPeerPayload.decode(
            archive("KagemushaRecipientPaymentRequestV2"),
            KagemushaPeerPayloadKind.RECEIVE_REQUEST,
        )
        val frames = KagemushaQrStreamCodec.encode(
            request,
            KagemushaQrStreamOptions.STANDARD,
        )
        assertEquals(
            listOf(
                "PKKQ1.S1EBALu6J7gkvW_mKvRoE04Tc9IAAAABAC4BAQQAAQAAAQABAAAAKbu6J7gkvW_mKvRoE04Tc9L3Ile8Baahf0wb7ZGckATmMK4Faw",
                "PKKQ1.S1EBAbu6J7gkvW_mKvRoE04Tc9IAAAABAClOUlQwAADbtlheLnWoNbztGQA6q3rNAAEAAAAAAAAA3oEw3T9nrrUCUZiX9lk",
                "PKKQ1.S1EBAru6J7gkvW_mKvRoE04Tc9IAAAABAQBOUlQwAADbtlheLnWoNbztGQA6q3rNAAEAAAAAAAAA3oEw3T9nrrUCUQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA4vsCHg",
            ),
            frames,
        )
        val decoder = KagemushaQrStreamDecoder()
        assertFalse(decoder.ingest(frames[0]).isComplete)
        val recovered = decoder.ingest(frames[2])
        assertTrue(recovered.isComplete)
        assertEquals(1, recovered.recoveredDataFrames)

        val rawArchive = request.archive()
        val commands = KagemushaNfcProtocol.writePayloadCommands(
            KagemushaPeerPayloadKind.RECEIVE_REQUEST,
            rawArchive,
            KagemushaNfcProtocol.SAFE_CHUNK_BYTES,
        )
        assertEquals(
            "802000002602010000002d16b35168fd7dce091904f3b0b2597831528dbf9c19bd154b8eba509b92b1f84c",
            commands[0].toHex(),
        )
        assertEquals(
            "802100002d4e5254300000dbb6585e2e75a835bced19003aab7acd000100000000000000de8130dd3f67aeb502519897f659",
            commands[1].toHex(),
        )
        assertEquals("8022000000", commands[2].toHex())
        assertEquals(
            "F0504B45504B524E464301",
            KagemushaNfcProtocol.applicationIdentifierHex(
                KagemushaNfcProtocol.defaultApplicationIdentifier(),
            ),
        )
        assertEquals(220, KagemushaNfcProtocol.SAFE_CHUNK_BYTES)
        assertEquals(2, KagemushaNfcProtocol.RAW_TRANSPORT_VERSION)
        assertTrue(KagemushaNfcProtocol.parseCommand(commands[0]) is KagemushaNfcCommand.WriteMetadata)

        val nearby = KagemushaNearbyEnvelopeCodec.encode(
            request,
            KagemushaNearbyPairingChallenge(KagemushaNearbyPairingSymbol.STARS),
        )
        assertEquals(
            "{\"contentType\":\"text/vnd.pk.kagemusha-v2.receive-request\",\"kind\":\"receive_request\",\"pairingChallenge\":\"nearby_pairing_stars\",\"payload\":\"UEtLMlIuVGxKVU1BQUEyN1pZWGk1MXFEVzg3UmtBT3F0NnpRQUJBQUFBQUFBQUFONkJNTjBfWjY2MUFsRQ\"}",
            nearby.toString(Charsets.UTF_8),
        )
        assertEquals(
            KagemushaPeerPayloadKind.RECEIVE_REQUEST,
            KagemushaNearbyEnvelopeCodec.decode(nearby).payload?.kind,
        )
        assertFalse(KagemushaNearbyTransportPolicy.IS_AVAILABLE)
        rawArchive.fill(0)
        nearby.fill(0)
    }

    @Test
    fun toriiLifecycleRoutesAndHeadersAreExact() {
        val captured = AtomicReference<TransportRequest>()
        val client = KagemushaRecursiveSpendProver.newToriiClient(
            URI.create("https://torii.example/api/"),
            object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                    captured.set(request)
                    val command = request.method == "POST"
                    return CompletableFuture.completedFuture(
                        TransportResponse.builder()
                            .setStatusCode(if (command) 202 else 200)
                            .addHeader("Content-Type", "application/x-norito")
                            .setBody(
                                archive(
                                    if (command) {
                                        "OfflineOperationReference"
                                    } else if (request.uri.path.contains("/operations/")) {
                                        "OfflineOperationStatus"
                                    } else {
                                        "OfflineReadiness"
                                    },
                                ),
                            )
                            .build(),
                    )
                }
            },
        )

        client.getReadiness("pkr#sbp").join()
        assertEquals(
            "https://torii.example/api/v1/offline/readiness?asset_definition_id=pkr%23sbp",
            captured.get().uri.toString(),
        )
        assertEquals(listOf("application/x-norito"), captured.get().headers["Accept"])

        val operationId = "11".repeat(32)
        client.submitTopUp(
            KagemushaRecursiveSpendProver.TopUpRequest(
                archive("iroha.torii.v1.offline.top_up.request"),
            ),
            operationId,
        ).join()
        assertEquals("POST", captured.get().method)
        assertEquals("/api/v1/offline/top-up", captured.get().uri.path)
        assertEquals(
            listOf("application/x-norito"),
            captured.get().headers["Content-Type"],
        )
        assertEquals(listOf(operationId), captured.get().headers["Idempotency-Key"])

        client.submitRedeem(
            KagemushaRecursiveSpendProver.RedeemSubmissionRequest(
                archive("iroha.torii.v1.offline.redeem.request"),
            ),
            operationId,
        ).join()
        assertEquals("/api/v1/offline/redeem", captured.get().uri.path)

        client.getOperation(operationId).join()
        assertEquals("/api/v1/offline/operations/$operationId", captured.get().uri.path)
    }

    private fun spendableBranch(seed: Int): KagemushaRecursiveSpendProver.SpendableBranchV4 =
        KagemushaRecursiveSpendProver.restoreSpendableBranchV4(
            KagemushaRecursiveSpendProver.decodeBundleV4(
                archive("KagemushaRecursiveSpendBundleV4", seed),
            ),
            KagemushaRecursiveSpendProver.NoteMembershipWitness(
                archive("KagemushaNoteMembershipWitnessV2", seed + 1),
            ),
            KagemushaRecursiveSpendProver.NoteOpening(
                archive("KagemushaNoteOpeningV2", seed + 2),
            ),
        )

    private fun outputMembershipPaths(): KagemushaRecursiveSpendProver.OutputMembershipPaths {
        val initialRoot = ByteArray(32) { 0x11 }
        val finalRoot = ByteArray(32) { 0x22 }
        fun path(root: ByteArray, leafIndex: Int = 0) =
            KagemushaRecursiveSpendProver.OutputMembershipPath(
            leafIndex = leafIndex,
            siblings = List(KagemushaRecursiveSpendProver.CONFIDENTIAL_TREE_DEPTH) { ByteArray(32) },
            directions = ByteArray(KagemushaRecursiveSpendProver.CONFIDENTIAL_TREE_DEPTH).also {
                it[0] = leafIndex.toByte()
            },
            root = root,
        )
        return KagemushaRecursiveSpendProver.OutputMembershipPaths(
            initialRoot = initialRoot,
            finalRoot = finalRoot,
            recipient = KagemushaRecursiveSpendProver.OutputMembershipLeafPaths(
                updatePath = path(initialRoot),
                membershipPath = path(finalRoot),
            ),
            change = null,
            dummyPath = path(finalRoot, 1),
        )
    }

    private fun archive(schema: String, marker: Int = 0x51): ByteArray {
        val payload = byteArrayOf(marker.toByte())
        val header = NoritoHeader(
            SchemaHash.hash16(schema),
            payload.size,
            CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE,
        )
        return header.encode() + payload
    }

    private fun ByteArray.toHex(): String =
        joinToString(separator = "") { byte -> "%02x".format(byte.toInt() and 0xff) }
}
