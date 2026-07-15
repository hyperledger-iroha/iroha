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
    fun outputMembershipPathsRejectNonconsecutiveDummyFrontier() {
        val initialRoot = ByteArray(32) { 0x11 }
        val finalRoot = ByteArray(32) { 0x22 }
        val afterRecipientRoot = ByteArray(32) { 0x33 }
        val recipient = KagemushaRecursiveSpendProver.OutputMembershipLeafPaths(
            updatePath = outputMembershipPath(initialRoot, 0),
            membershipPath = outputMembershipPath(finalRoot, 0),
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.OutputMembershipPaths(
                initialRoot = initialRoot,
                finalRoot = finalRoot,
                recipient = recipient,
                change = null,
                dummyPath = outputMembershipPath(finalRoot, 2),
            )
        }
        val change = KagemushaRecursiveSpendProver.OutputMembershipLeafPaths(
            updatePath = outputMembershipPath(afterRecipientRoot, 1),
            membershipPath = outputMembershipPath(finalRoot, 1),
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.OutputMembershipPaths(
                initialRoot = initialRoot,
                finalRoot = finalRoot,
                recipient = recipient,
                change = change,
                dummyPath = outputMembershipPath(finalRoot, 3),
            )
        }
        val redemptionChange = KagemushaRecursiveSpendProver.OutputMembershipLeafPaths(
            updatePath = outputMembershipPath(initialRoot, 5),
            membershipPath = outputMembershipPath(finalRoot, 5),
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.OutputMembershipPaths(
                initialRoot = initialRoot,
                finalRoot = finalRoot,
                recipient = null,
                change = redemptionChange,
                dummyPath = outputMembershipPath(finalRoot, 7),
            )
        }
    }

    @Test
    fun artifactContractAndInventoryAreCurrentOnly() {
        assertEquals(20, KagemushaRecursiveSpendProver.REQUIRED_NATIVE_BRIDGE_ABI_VERSION)
        assertEquals(8, KagemushaRecursiveSpendProver.ARTIFACT_COUNT)
        assertEquals(2, KagemushaRecursiveSpendProver.MAXIMUM_INPUTS_PER_TRANSITION)
        assertEquals(2, KagemushaRecursiveSpendProver.MAXIMUM_LOCAL_APPEND_BUILDER_INPUTS)
        assertEquals(2, KagemushaRecursiveSpendProver.MAXIMUM_BRANCH_CLAIMS)
        assertEquals(8, KagemushaRecursiveSpendProver.MAXIMUM_PEER_HOPS)
        assertEquals(
            16 * 1024 * 1024,
            KagemushaRecursiveSpendProver.MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4,
        )
        assertEquals(32 * 1024, KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES_V2)
        assertEquals(32 * 1024 * 1024, KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES_V4)
        assertEquals(
            6_488_064,
            KagemushaRecursiveSpendProver.MAX_TOP_UP_PROVENANCE_ARCHIVE_BYTES_V4,
        )
        assertEquals(
            KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES_V4,
            KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES,
        )
        assertEquals(32 * 1024, KagemushaPeerTransportContract.MAXIMUM_ARCHIVE_BYTES_V2)
        assertEquals(32 * 1024 * 1024, KagemushaPeerTransportContract.MAXIMUM_ARCHIVE_BYTES_V4)
        assertEquals(
            KagemushaPeerTransportContract.MAXIMUM_ARCHIVE_BYTES_V4,
            KagemushaPeerTransportContract.MAXIMUM_ARCHIVE_BYTES,
        )
        assertEquals(9_211, KagemushaRecursiveSpendProver.MAX_PEER_TEXT_ARCHIVE_BYTES)
        assertEquals(
            512 * 1024,
            KagemushaRecursiveSpendProver.MAX_TORII_TOP_UP_REQUEST_BYTES_V4,
        )
        assertEquals(
            48 * 1024 * 1024,
            KagemushaRecursiveSpendProver.MAX_TORII_REDEEM_REQUEST_BYTES_V4,
        )
        assertEquals(16, KagemushaRecursiveSpendProver.CONFIDENTIAL_TREE_DEPTH)
        assertEquals(
            4 * 1024,
            KagemushaRecursiveSpendProver.MAX_OUTPUT_MEMBERSHIP_FRONTIER_ARCHIVE_BYTES_V4,
        )
        assertEquals(
            16 * 1024,
            KagemushaRecursiveSpendProver.MAX_OUTPUT_MEMBERSHIP_PATHS_ARCHIVE_BYTES_V4,
        )
        assertEquals(
            "kagemusha.offline.recursive_spend.artifact_manifest.v4",
            KagemushaRecursiveSpendProver.ARTIFACT_MANIFEST_SCHEMA,
        )
        assertEquals(
            listOf(
                "step-eq.params-ipa.krv4",
                "step-eq.proving-key.krv4",
                "step-eq.verifying-key.krv4",
                "step-eq.bootstrap-witness.krv4",
                "step-ep.params-ipa.krv4",
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
            ByteArray::class.java,
            LongArray::class.java,
        )
        assertEquals(8, nativeInstall.parameterCount)
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
                "buildOutputMembershipFrontierV4",
                "buildTopUpProvenanceV4",
                "buildRedeemV4",
                "buildRedeemRequestV4",
                "buildVerifyRequestV4",
                "decodeAppendRequestV4",
                "decodeBundleV4",
                "decodeInitRequestV4",
                "decodeInitResultV4",
                "decodeNoteMembershipWitness",
                "decodeNoteOpening",
                "decodeOutputMembershipFrontierV4",
                "decodePeerPayment",
                "decodeRedeemRequestV4",
                "decodeReceiverAcknowledgement",
                "decodeRecipientPaymentRequest",
                "decodeRedeemBuildResultV4",
                "decodeRedeemSubmissionRequest",
                "decodeSplitResultV4",
                "decodeTopUpAnchorV4",
                "decodeTopUpFinalityEvidenceV4",
                "decodeTopUpProvenanceV4",
                "decodeTopUpRequest",
                "decodeVerifyRequestV4",
                "decodeVerifyResultV4",
                "decodeTopUpFinalityRosterArtifact",
                "deriveOutputMembershipPathsV4",
                "finalizeRedeemV4",
                "finalizeTopUp",
                "initSpendV4",
                "installedArtifactManifestSha256V4",
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
                "projectInitResultV4",
                "projectRedeemBuildResultV4",
                "projectRecipientPaymentRequest",
                "projectReadiness",
                "projectSplitResultV4",
                "projectVerifyResultV4",
                "restoreInitBranchV4",
                "restorePeerPaymentBranchV4",
                "restoreRedeemChangeBranchV4",
                "restoreSpendableBranchV4",
                "restoreSplitChangeBranchV4",
                "signAcknowledgement",
                "signRecipientPaymentRequest",
                "signRequestAuthorization",
                "verifyAcknowledgement",
                "verifyRecipientPaymentRequest",
                "verifySpendV4",
                "validateTopUpProvenanceV4",
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

        val initProjectionMethods =
            KagemushaRecursiveSpendProver.InitProjectionV4::class.java.declaredMethods
                .map { it.name }
                .toSet()
        assertTrue("getBranch" in initProjectionMethods)
        assertTrue("getTopUpProvenance" in initProjectionMethods)
        val redeemProjectionMethods =
            KagemushaRecursiveSpendProver.RedeemBuildProjection::class.java.declaredMethods
                .map { it.name }
                .toSet()
        assertTrue("getChangeTopUpProvenance" in redeemProjectionMethods)
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
        assertTrue(appendNative.parameterTypes.take(4).all { it == Array<ByteArray>::class.java })
        val verifyBuilder = KagemushaRecursiveSpendProver::class.java.declaredMethods.single {
            it.name == "buildVerifyRequestV4" && java.lang.reflect.Modifier.isPublic(it.modifiers)
        }
        assertEquals(
            KagemushaRecursiveSpendProver.TopUpProvenanceV4::class.java,
            verifyBuilder.parameterTypes[2],
        )
        assertEquals(
            1,
            KagemushaRecursiveSpendProver::class.java.declaredMethods.count {
                it.name == "buildAppendRequestV4" && java.lang.reflect.Modifier.isPublic(it.modifiers)
            },
        )
    }

    @Test
    fun outputMembershipFrontierIsPersistableAndRestoreIsProofBound() {
        val bytes = archive("connect_norito_bridge::KagemushaOutputMembershipFrontierV4")
        val frontier = KagemushaRecursiveSpendProver.decodeOutputMembershipFrontierV4(bytes)
        bytes[bytes.lastIndex] = (bytes.last() + 1).toByte()
        val first = frontier.noritoEncoded()
        first[first.lastIndex] = (first.last() + 1).toByte()
        assertFalse(first.contentEquals(frontier.noritoEncoded()))

        val build = KagemushaRecursiveSpendProver::class.java.getDeclaredMethod(
            "nativeBuildOutputMembershipFrontierV4",
            Int::class.javaPrimitiveType,
            ByteArray::class.java,
            ByteArray::class.java,
            ByteArray::class.java,
        )
        assertEquals(ByteArray::class.java, build.returnType)
        val derive = KagemushaRecursiveSpendProver::class.java.getDeclaredMethod(
            "nativeDeriveOutputMembershipPathsV4",
            ByteArray::class.java,
            ByteArray::class.java,
            ByteArray::class.java,
        )
        assertEquals(Array<ByteArray>::class.java, derive.returnType)
        val validate = KagemushaRecursiveSpendProver::class.java.getDeclaredMethod(
            "nativeValidateSpendableBranchV4",
            ByteArray::class.java,
            ByteArray::class.java,
            ByteArray::class.java,
            ByteArray::class.java,
            Long::class.javaPrimitiveType,
        )
        assertEquals(ByteArray::class.java, validate.returnType)
        assertTrue(
            KagemushaRecursiveSpendProver.SpendableBranchV4::class.java.methods.any {
                it.name == "getFrontier" &&
                    it.returnType ==
                    KagemushaRecursiveSpendProver.OutputMembershipFrontierV4::class.java
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
        KagemushaRecursiveSpendProver.ReleaseAuthentication(one, one, one, one, one)
        repeat(5) { emptyIndex ->
            val values = Array(5) { one }
            values[emptyIndex] = byteArrayOf()
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.ReleaseAuthentication(
                    values[0], values[1], values[2], values[3], values[4],
                )
            }
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.ReleaseAuthentication(
                ByteArray(KagemushaRecursiveSpendProver.MAX_TRUSTED_RELEASE_POLICY_BYTES + 1),
                one,
                one,
                one,
                one,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.ReleaseAuthentication(
                one,
                one,
                one,
                one,
                ByteArray(KagemushaRecursiveSpendProver.MAX_PROMOTION_RECORD_BYTES + 1),
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
            KagemushaRecursiveSpendProver.BundleV4(
                archive("KagemushaRecursiveSpendBundleV4"),
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
            KagemushaRecursiveSpendProver.ArtifactBindingV4(
                archive("KagemushaRecursiveSpendArtifactBindingV4"),
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

        val tampered = archive("KagemushaRecursiveSpendArtifactBindingV4")
        tampered[tampered.lastIndex] = (tampered.last().toInt() xor 1).toByte()
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.ArtifactBindingV4(tampered)
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
    fun branchRestoreRejectsMissingHeightAndLocalChangeOpeningsBeforeNativeDispatch() {
        val opening = KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2"),
        )
        val init = KagemushaRecursiveSpendProver.decodeInitResultV4(
            archive("KagemushaRecursiveSpendInitResultV4"),
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.restoreInitBranchV4(init, opening, 0)
        }
        val payment = KagemushaRecursiveSpendProver.decodePeerPayment(
            archive("KagemushaRecursiveSpendPeerPaymentV4"),
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.restorePeerPaymentBranchV4(payment, opening, 0)
        }

        val split = KagemushaRecursiveSpendProver.decodeSplitResultV4(
            archive("KagemushaRecursiveSpendSplitResultV4"),
            null,
        )
        assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.restoreSplitChangeBranchV4(split, 1)
        }
        val redeem = KagemushaRecursiveSpendProver.decodeRedeemBuildResultV4(
            archive("KagemushaRecursiveSpendRedeemBuildResultV4"),
            null,
        )
        assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.restoreRedeemChangeBranchV4(redeem, 1)
        }
    }

    @Test
    fun topUpProvenanceArchiveIsCanonicalBoundedAndDefensive() {
        val bytes = archive("KagemushaRecursiveSpendTopUpProvenanceV4")
        val provenance = KagemushaRecursiveSpendProver.decodeTopUpProvenanceV4(bytes)
        bytes[bytes.lastIndex] = 0
        assertEquals(0x51, provenance.noritoEncoded().last().toInt() and 0xff)

        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.decodeTopUpProvenanceV4(
                archive("KagemushaRecursiveSpendTopUpFinalityEvidenceV4"),
            )
        }
        val corrupted = archive("KagemushaRecursiveSpendTopUpProvenanceV4").also {
            it[it.lastIndex] = (it.last().toInt() xor 1).toByte()
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.decodeTopUpProvenanceV4(corrupted)
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.decodeTopUpProvenanceV4(
                ByteArray(KagemushaRecursiveSpendProver.MAX_TOP_UP_PROVENANCE_ARCHIVE_BYTES_V4 + 1),
            )
        }

        val bundle = KagemushaRecursiveSpendProver.decodeBundleV4(
            archive("KagemushaRecursiveSpendBundleV4"),
        )
        val roster = KagemushaRecursiveSpendProver.decodeTopUpFinalityRosterArtifact(
            archive("KagemushaTopUpFinalityRosterArtifactV2"),
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.buildTopUpProvenanceV4(
                bundle,
                roster,
                emptyList(),
                emptyList(),
                1,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.buildTopUpProvenanceV4(
                bundle,
                roster,
                listOf(KagemushaRecursiveSpendProver.decodeTopUpAnchorV4(
                    archive("KagemushaRecursiveSpendTopUpAnchorV4"),
                )),
                emptyList(),
                1,
            )
        }
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
    fun readinessPreservesExactReleaseCapabilitiesIndependently() {
        fun verifier(
            name: String,
            circuitId: String,
            seed: Int,
            withdrawalHeight: Long? = null,
        ) =
            KagemushaRecursiveSpendProver.ActiveVerifier(
                "halo2/ipa",
                name,
                1,
                circuitId,
                ByteArray(32) { seed.toByte() },
                ByteArray(32) { (seed + 16).toByte() },
                12 * 1024,
                10,
                withdrawalHeight,
            )
        fun artifactSet() = KagemushaRecursiveSpendProver.AuthenticatedArtifactSet(
            "release-v4",
            ByteArray(32) { 0x31 },
            ByteArray(32) { 0x32 },
            ByteArray(32) { 0x33 },
            10,
            30,
            12 * 1024,
            9,
        )
        val transfer = verifier(
            "confidential_transfer_v2_verifier_record",
            "halo2/pasta/ipa/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
            1,
        )
        val topUp = verifier(
            "kagemusha_topup_shield_v2_verifier_record",
            "halo2/pasta/ipa/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
            2,
        )
        val unshield = verifier(
            "confidential_unshield_v3_verifier_record",
            "halo2/pasta/ipa/confidential-unshield-change-merkle16-axiom-poseidon-v4",
            3,
        )
        val stepEq = verifier(
            "kagemusha_recursive_step_eq_v4_verifier_record",
            "kagemusha-recursive-spend-step-eq-authenticated-layout-v4",
            4,
            30,
        )
        val stepEp = verifier(
            "kagemusha_recursive_step_ep_v4_verifier_record",
            "kagemusha-recursive-spend-step-ep-authenticated-layout-v4",
            5,
            30,
        )
        fun readiness(
            transferVerifier: KagemushaRecursiveSpendProver.ActiveVerifier? = transfer,
            unshieldVerifier: KagemushaRecursiveSpendProver.ActiveVerifier? = unshield,
            recursiveStepEqVerifier: KagemushaRecursiveSpendProver.ActiveVerifier? = stepEq,
            artifact: KagemushaRecursiveSpendProver.AuthenticatedArtifactSet? = artifactSet(),
            proofBackendAvailable: Boolean = true,
        ) = KagemushaRecursiveSpendProver.ReadinessProjection(
            20,
            8,
            "xor#sora",
            9,
            20,
            ByteArray(32) { 0x41 },
            proofBackendAvailable,
            true,
            true,
            transferVerifier,
            topUp,
            unshieldVerifier,
            recursiveStepEqVerifier,
            stepEp,
            artifact,
            emptyList(),
        )

        assertTrue(readiness().allVerifiersActive)
        assertTrue(readiness().chainArtifactSetReady)
        assertFalse(readiness().offlineReady)
        assertFalse(readiness(transferVerifier = null).allVerifiersActive)
        assertTrue(readiness(transferVerifier = null).chainArtifactSetReady)
        assertFalse(
            readiness(
                unshieldVerifier = verifier(
                    "confidential_unshield_v3_verifier_record",
                    "halo2/pasta/ipa/confidential-unshield-change-merkle16-axiom-poseidon-v4",
                    3,
                    20,
                ),
            ).allVerifiersActive,
        )
        assertFalse(
            readiness(
                recursiveStepEqVerifier = verifier(
                    "kagemusha_recursive_step_eq_v4_verifier_record",
                    "kagemusha-recursive-spend-step-eq-authenticated-layout-v4",
                    4,
                    20,
                ),
            ).chainArtifactSetReady,
        )
        assertFalse(readiness(artifact = null).chainArtifactSetReady)
        assertFalse(readiness(proofBackendAvailable = false).chainArtifactSetReady)

        val artifact = artifactSet()
        val exposedManifestDigest = artifact.manifestSha256()
        exposedManifestDigest.fill(0)
        assertEquals(0x31, artifact.manifestSha256()[0].toInt())
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.AuthenticatedArtifactSet(
                "release-v4",
                ByteArray(32) { 0x31 },
                ByteArray(32) { 0x31 },
                ByteArray(32) { 0x33 },
                10,
                30,
                12 * 1024,
                9,
            )
        }
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
                archive("KagemushaRecursiveSpendPeerPaymentV4"),
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
        val corrupted = archive("KagemushaRecursiveSpendPeerPaymentV4")
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
            "8020040026040100000029bbba27b824bd6fe62af468134e1373d2f72257bc05a6a17f4c1bed919c9004e6",
            commands[0].toHex(),
        )
        assertEquals(
            "802104002d000000004e5254300000dbb6585e2e75a835bced19003aab7acd000100000000000000de8130dd3f67aeb50251",
            commands[1].toHex(),
        )
        assertEquals("8022040000", commands[2].toHex())
        assertEquals(
            "F0504B45504B524E464301",
            KagemushaNfcProtocol.applicationIdentifierHex(
                KagemushaNfcProtocol.defaultApplicationIdentifier(),
            ),
        )
        assertEquals(220, KagemushaNfcProtocol.SAFE_CHUNK_BYTES)
        assertEquals(4, KagemushaNfcProtocol.RAW_TRANSPORT_VERSION)
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
    fun nfcV4StreamsBeyondLegacyLimitAndRejectsDowngrade() {
        val payload = ByteArray(70_003) { index -> (index * 29 + 7).toByte() }
        val commands = KagemushaNfcProtocol.writePayloadCommands(
            KagemushaPeerPayloadKind.PAYMENT,
            payload,
            KagemushaNfcProtocol.MAXIMUM_EXTENDED_WRITE_CHUNK_BYTES,
        )
        val metadata = KagemushaNfcProtocol.parseCommand(commands.first())
            as KagemushaNfcCommand.WriteMetadata
        assertEquals(payload.size, metadata.payloadLength)
        val assembler = KagemushaNfcPayloadAssembler(
            metadata.kind,
            metadata.payloadLength,
            metadata.sha256,
        )
        commands.subList(1, commands.lastIndex).asReversed().forEach { encoded ->
            val chunk = KagemushaNfcProtocol.parseCommand(encoded) as KagemushaNfcCommand.WriteChunk
            assertTrue(assembler.write(chunk.offset, chunk.bytes))
        }
        assertTrue(assembler.isComplete)
        assertTrue(payload.contentEquals(assembler.commit()))

        val atFFFF = KagemushaNfcProtocol.writeChunkCommand(0xffff, byteArrayOf(0x5a))
        val at10000 = KagemushaNfcProtocol.writeChunkCommand(0x1_0000, byteArrayOf(0x5a))
        assertEquals("80210400050000ffff5a", atFFFF.toHex())
        assertEquals("8021040005000100005a", at10000.toHex())
        assertEquals(
            0xffff,
            (KagemushaNfcProtocol.parseCommand(atFFFF) as KagemushaNfcCommand.WriteChunk).offset,
        )
        assertEquals(
            0x1_0000,
            (KagemushaNfcProtocol.parseCommand(at10000) as KagemushaNfcCommand.WriteChunk).offset,
        )
        assertEquals(
            "80110400060000ffff0400",
            KagemushaNfcProtocol.readChunkCommand(0xffff, 1_024).toHex(),
        )

        assertEquals(
            KagemushaNfcCommand.Invalid,
            KagemushaNfcProtocol.parseCommand(byteArrayOf(0x80.toByte(), 0x21, 0, 0, 1, 0x5a)),
        )
        assertEquals(
            KagemushaNfcCommand.Invalid,
            KagemushaNfcProtocol.parseCommand(
                byteArrayOf(0x80.toByte(), 0x21, 0xff.toByte(), 0xff.toByte(), 1, 0x5a),
            ),
        )
        assertEquals(
            KagemushaNfcCommand.Invalid,
            KagemushaNfcProtocol.parseCommand(byteArrayOf(0x80.toByte(), 0x11, 0, 0, 0)),
        )
        assertEquals(
            KagemushaNfcCommand.Invalid,
            KagemushaNfcProtocol.parseCommand(at10000.copyOf(at10000.size - 1)),
        )

        val maximumInfo = ByteArray(40).also {
            it[0] = KagemushaNfcProtocol.RAW_TRANSPORT_VERSION.toByte()
            it[1] = KagemushaPeerPayloadKind.PAYMENT.code.toByte()
            it[2] = 0x02
            it[7] = KagemushaNfcProtocol.SAFE_CHUNK_BYTES.toByte()
            it[8] = 1
        }
        assertEquals(32 * 1024 * 1024, KagemushaNfcProtocol.decodeInfo(maximumInfo)?.payloadLength)
        maximumInfo[5] = 1
        assertEquals(null, KagemushaNfcProtocol.decodeInfo(maximumInfo))
        assertFailsWith<IllegalArgumentException> {
            KagemushaNfcProtocol.writeChunkCommand(
                KagemushaNfcProtocol.MAXIMUM_PAYLOAD_BYTES,
                byteArrayOf(1),
            )
        }

        assembler.clear()
        payload.fill(0)
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
        KagemushaRecursiveSpendProver.SpendableBranchV4(
            KagemushaRecursiveSpendProver.decodeBundleV4(
                archive("KagemushaRecursiveSpendBundleV4", seed),
            ),
            KagemushaRecursiveSpendProver.NoteMembershipWitness(
                archive("KagemushaNoteMembershipWitnessV2", seed + 1),
            ),
            KagemushaRecursiveSpendProver.NoteOpening(
                archive("KagemushaNoteOpeningV2", seed + 2),
            ),
            KagemushaRecursiveSpendProver.decodeTopUpProvenanceV4(
                archive("KagemushaRecursiveSpendTopUpProvenanceV4", seed + 3),
            ),
            KagemushaRecursiveSpendProver.decodeOutputMembershipFrontierV4(
                archive(
                    "connect_norito_bridge::KagemushaOutputMembershipFrontierV4",
                    seed + 4,
                ),
            ),
        )

    private fun outputMembershipPaths(): KagemushaRecursiveSpendProver.OutputMembershipPaths {
        val initialRoot = ByteArray(32) { 0x11 }
        val finalRoot = ByteArray(32) { 0x22 }
        return KagemushaRecursiveSpendProver.OutputMembershipPaths(
            initialRoot = initialRoot,
            finalRoot = finalRoot,
            recipient = KagemushaRecursiveSpendProver.OutputMembershipLeafPaths(
                updatePath = outputMembershipPath(initialRoot, 0),
                membershipPath = outputMembershipPath(finalRoot, 0),
            ),
            change = null,
            dummyPath = outputMembershipPath(finalRoot, 1),
        )
    }

    private fun outputMembershipPath(
        root: ByteArray,
        leafIndex: Int,
    ): KagemushaRecursiveSpendProver.OutputMembershipPath =
        KagemushaRecursiveSpendProver.OutputMembershipPath(
            leafIndex = leafIndex,
            siblings = List(KagemushaRecursiveSpendProver.CONFIDENTIAL_TREE_DEPTH) { ByteArray(32) },
            directions = ByteArray(KagemushaRecursiveSpendProver.CONFIDENTIAL_TREE_DEPTH).also {
                for (level in it.indices) {
                    it[level] = ((leafIndex ushr level) and 1).toByte()
                }
            },
            root = root,
        )

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
