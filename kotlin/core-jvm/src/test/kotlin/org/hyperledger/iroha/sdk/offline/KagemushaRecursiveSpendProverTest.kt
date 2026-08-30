package org.hyperledger.iroha.sdk.offline

import java.net.URI
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.security.KeyPairGenerator
import java.security.Signature
import java.time.Duration
import java.util.Base64
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import kotlin.test.assertFailsWith
import org.hyperledger.iroha.sdk.client.CanonicalRequestSigner
import org.hyperledger.iroha.sdk.client.LocalSigningContext
import org.hyperledger.iroha.sdk.client.ToriiCanonicalRequestAuth
import org.hyperledger.iroha.sdk.client.transport.RequestReplayPolicy
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.client.transport.TransportExecutor

class KagemushaRecursiveSpendProverTest {
    @Test
    fun heavyProofPermitIsReentrantButRejectsAnotherThreadWithoutWaiting() {
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val failure = AtomicReference<Throwable?>()
        val worker = Thread {
            try {
                KagemushaRecursiveSpendProver.withHeavyProofPermitForTest {
                    entered.countDown()
                    check(release.await(5, TimeUnit.SECONDS))
                }
            } catch (error: Throwable) {
                failure.set(error)
            }
        }
        worker.start()
        assertTrue(entered.await(5, TimeUnit.SECONDS))
        assertFailsWith<KagemushaRecursiveSpendProver.ProofWorkerBusyException> {
            KagemushaRecursiveSpendProver.withHeavyProofPermitForTest {}
        }
        release.countDown()
        worker.join(5_000)
        assertFalse(worker.isAlive)
        failure.get()?.let { throw it }

        KagemushaRecursiveSpendProver.withHeavyProofPermitForTest {
            KagemushaRecursiveSpendProver.withHeavyProofPermitForTest {}
        }
    }

    @Test
    fun exactBridgeAbi23IsRequired() {
        assertTrue(KagemushaRecursiveSpendProver.isExactBridgeAbi(23))
        assertFalse(KagemushaRecursiveSpendProver.isExactBridgeAbi(22))
        assertTrue(
            KagemushaRecursiveSpendProver.detectExactNativeAvailability(
                loadLibrary = {},
                abiVersion = { 23 },
                symbolProbe = { true },
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.detectProductionProofBackendCompilation {
                throw IllegalArgumentException("production bridge reached artifact validation")
            },
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectProductionProofBackendCompilation {
                throw IllegalStateException("default bridge rejected production")
            },
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectProductionProofBackendCompilation {
                throw UnsatisfiedLinkError("missing")
            },
        )
    }

    @Test
    fun appAttestNativeProjectionRejectsCorruptAuxiliaryFields() {
        val authorizationArchive = archive("KagemushaRequestAuthorizationV2")
        val rawLowSSignature = ByteArray(64).also {
            it[31] = 1
            it[63] = 1
        }
        val extensionAuthenticatorData = ByteArray(38).also {
            it[32] = 0x80.toByte()
            it[36] = 1
            it[37] = 0xa0.toByte()
        }
        val accepted = KagemushaRecursiveSpendProver
            .requestAuthorizationFromIosAppAttestNativeProjection(
                arrayOf(authorizationArchive, rawLowSSignature, extensionAuthenticatorData),
            )
        assertContentEquals(authorizationArchive, accepted.noritoEncoded())

        val invalidProjections = listOf(
            arrayOf(authorizationArchive, rawLowSSignature),
            arrayOf(authorizationArchive, ByteArray(0), extensionAuthenticatorData),
            arrayOf(authorizationArchive, rawLowSSignature.copyOf(63), extensionAuthenticatorData),
            arrayOf(authorizationArchive, ByteArray(64), extensionAuthenticatorData),
            arrayOf(authorizationArchive, rawLowSSignature, ByteArray(36)),
            arrayOf(authorizationArchive, rawLowSSignature, ByteArray(38)),
            arrayOf(
                authorizationArchive,
                rawLowSSignature,
                ByteArray(37).also { it[36] = 1 },
            ),
            arrayOf(
                authorizationArchive,
                rawLowSSignature,
                ByteArray(37).also { it[32] = 0x80.toByte() },
            ),
            arrayOf(
                authorizationArchive,
                rawLowSSignature,
                ByteArray(38).also {
                    it[32] = 0x01
                    it[37] = 0xa0.toByte()
                },
            ),
            arrayOf(
                authorizationArchive,
                rawLowSSignature,
                ByteArray(4 * 1024 + 1).also { it[32] = 0x80.toByte() },
            ),
        )
        invalidProjections.forEach { projection ->
            assertFailsWith<IllegalStateException> {
                KagemushaRecursiveSpendProver
                    .requestAuthorizationFromIosAppAttestNativeProjection(projection)
            }
        }
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
    fun redemptionChangePreparationTransfersOpeningOwnershipExactlyOnce() {
        val opening = KagemushaRecursiveSpendProver.NoteOpening(
            archive("KagemushaNoteOpeningV2"),
        )
        val preparation = KagemushaRecursiveSpendProver.RedemptionChangePreparationV4(
            opening = opening,
            rho = ByteArray(32) { 0x21 },
            diversifier = ByteArray(32) { 0x22 },
            commitment = ByteArray(32) { 0x23 },
            spendNullifier = ByteArray(32) { 0x24 },
            amount = KagemushaScaledAmount.fromAtomicUnits("375", 2),
        )
        val rho = preparation.rho()
        rho.fill(0)
        assertTrue(preparation.rho().all { it == 0x21.toByte() })
        assertEquals("375", preparation.amount.atomicUnits)

        val transferred = preparation.takeOpening()
        try {
            assertTrue(transferred === opening)
            assertFailsWith<IllegalStateException> { preparation.takeOpening() }

            preparation.close()
            preparation.close()
            assertFalse(transferred.isDestroyed())
            assertTrue(transferred.noritoEncoded().isNotEmpty())
            assertFailsWith<IllegalStateException> { preparation.takeOpening() }
            assertFailsWith<IllegalStateException> { preparation.rho() }
            assertFailsWith<IllegalStateException> { preparation.commitment() }
            assertFailsWith<IllegalStateException> { preparation.amount }
        } finally {
            transferred.destroy()
        }
    }

    @Test
    fun redemptionChangePreparationDestroysOnlyAnUntransferredOpening() {
        val opening = KagemushaRecursiveSpendProver.NoteOpening(
            archive("KagemushaNoteOpeningV2"),
        )
        val preparation = KagemushaRecursiveSpendProver.RedemptionChangePreparationV4(
            opening = opening,
            rho = ByteArray(32) { 0x31 },
            diversifier = ByteArray(32) { 0x32 },
            commitment = ByteArray(32) { 0x33 },
            spendNullifier = ByteArray(32) { 0x34 },
            amount = KagemushaScaledAmount.fromAtomicUnits("125", 2),
        )

        preparation.close()
        preparation.close()
        assertTrue(opening.isDestroyed())
        assertFailsWith<IllegalStateException> { preparation.takeOpening() }
        assertFailsWith<IllegalStateException> { preparation.diversifier() }
        assertFailsWith<IllegalStateException> { preparation.amount }

        val rejectedOpening = KagemushaRecursiveSpendProver.NoteOpening(
            archive("KagemushaNoteOpeningV2"),
        )
        assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.RedemptionChangePreparationV4(
                opening = rejectedOpening,
                rho = ByteArray(32) { 0x41 },
                diversifier = ByteArray(32) { 0x41 },
                commitment = ByteArray(32) { 0x43 },
                spendNullifier = ByteArray(32) { 0x44 },
                amount = KagemushaScaledAmount.fromAtomicUnits("125", 2),
            )
        }
        assertTrue(rejectedOpening.isDestroyed())
    }

    @Test
    fun redemptionChangeRequestFailureDestroysTheTransferredOpening() {
        val opening = KagemushaRecursiveSpendProver.NoteOpening(
            archive("KagemushaNoteOpeningV2", 0x45),
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.decodeRedeemRequestV4(
                archive = byteArrayOf(),
                changeOpening = opening,
            )
        }
        assertTrue(opening.isDestroyed())

        val redeemInput = spendableBranch(0x47)
        val backendOpening = KagemushaRecursiveSpendProver.NoteOpening(
            archive("KagemushaNoteOpeningV2", 0x46),
        )
        try {
            assertFailsWith<RuntimeException> {
                KagemushaRecursiveSpendProver.buildRedeemRequestV4(
                    input = redeemInput,
                    recipientAccountId = "alice@wonderland",
                    chainDiscriminant =
                        org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT,
                    amount = KagemushaScaledAmount.fromAtomicUnits("125", 2),
                    changeOpening = backendOpening,
                    changeOutputMembershipPaths = redemptionChangeOutputMembershipPaths(),
                    unshieldVerifierCommitment = ByteArray(32) { 0x48 },
                    operationId = ByteArray(32) { 0x49 },
                    blockHeight = 1,
                )
            }
            assertTrue(backendOpening.isDestroyed())
        } finally {
            redeemInput.close()
        }

        val appendInput = spendableBranch(0x50)
        val appendOpening = KagemushaRecursiveSpendProver.NoteOpening(
            archive("KagemushaNoteOpeningV2", 0x51),
        )
        try {
            assertFailsWith<RuntimeException> {
                KagemushaRecursiveSpendProver.buildAppendRequestV4(
                    inputs = listOf(appendInput),
                    changeOpening = appendOpening,
                    outputMembershipPaths = appendChangeOutputMembershipPaths(),
                    transferVerifierCommitment = ByteArray(32) { 0x52 },
                    operationId = ByteArray(32) { 0x53 },
                    blockHeight = 1,
                )
            }
            assertTrue(appendOpening.isDestroyed())
        } finally {
            appendInput.close()
        }

        val restoreOpening = KagemushaRecursiveSpendProver.NoteOpening(
            archive("KagemushaNoteOpeningV2", 0x54),
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.restoreSpendableBranchV4(
                bundle = KagemushaRecursiveSpendProver.decodeBundleV4(
                    archive("KagemushaRecursiveSpendBundleV4", 0x55),
                ),
                membershipWitness = KagemushaRecursiveSpendProver.NoteMembershipWitness(
                    archive("KagemushaNoteMembershipWitnessV2", 0x56),
                ),
                opening = restoreOpening,
                topUpProvenance = KagemushaRecursiveSpendProver.decodeTopUpProvenanceV4(
                    archive("KagemushaRecursiveSpendTopUpProvenanceV4", 0x57),
                ),
                blockHeight = 0,
            )
        }
        assertTrue(restoreOpening.isDestroyed())
    }

    @Test
    fun redemptionChangeCarriersCloseOrTransferOneOwnerExactlyOnce() {
        val closeOwnedOpening = KagemushaRecursiveSpendProver.NoteOpening(
            archive("KagemushaNoteOpeningV2", 0x4a),
        )
        val closeOwnedRequest = KagemushaRecursiveSpendProver.RedeemRequestV4(
            archive("KagemushaRecursiveSpendRedeemLocalRequestV4", 0x4b),
            closeOwnedOpening,
        )
        closeOwnedRequest.close()
        closeOwnedRequest.close()
        assertTrue(closeOwnedOpening.isDestroyed())
        assertFailsWith<IllegalStateException> { closeOwnedRequest.takeChangeOpening() }

        val handedOffOpening = KagemushaRecursiveSpendProver.NoteOpening(
            archive("KagemushaNoteOpeningV2", 0x4c),
        )
        val request = KagemushaRecursiveSpendProver.RedeemRequestV4(
            archive("KagemushaRecursiveSpendRedeemLocalRequestV4", 0x4d),
            handedOffOpening,
        )
        val requestHandoff = request.takeChangeOpening()
        assertTrue(requestHandoff === handedOffOpening)
        request.close()
        request.close()
        assertFalse(handedOffOpening.isDestroyed())
        assertFailsWith<IllegalStateException> { request.takeChangeOpening() }

        val result = KagemushaRecursiveSpendProver.RedeemBuildResultV4(
            archive("KagemushaRecursiveSpendRedeemBuildResultV4", 0x4e),
            requestHandoff,
        )
        val resultHandoff = result.takeChangeOpening()
        assertTrue(resultHandoff === handedOffOpening)
        result.close()
        result.close()
        assertFalse(handedOffOpening.isDestroyed())
        assertFailsWith<IllegalStateException> { result.takeChangeOpening() }
        resultHandoff.destroy()
        assertTrue(handedOffOpening.isDestroyed())

        val spendableBranch = spendableBranch(0x4f)
        val spendableOpening = spendableBranch.opening
        spendableBranch.close()
        spendableBranch.close()
        assertTrue(spendableOpening.isDestroyed())
    }

    @Test
    fun temporarySecretArchivesAreWipedAfterPartialConstructionAndOwnerCopy() {
        val first = ByteArray(32) { 0x61 }
        val second = ByteArray(32) { 0x62 }
        val partiallyConstructed = arrayOf<ByteArray?>(first, null, second)
        KagemushaRecursiveSpendProver.SecretArchiveWiper.wipeAll(partiallyConstructed)
        assertTrue(first.all { it == 0.toByte() })
        assertTrue(second.all { it == 0.toByte() })
        KagemushaRecursiveSpendProver.SecretArchiveWiper.wipeAll(null)

        val rawNativeArchive = archive("KagemushaRecursiveSpendRedeemLocalRequestV4", 0x63)
        val owner = KagemushaRecursiveSpendProver.RedeemRequestV4(rawNativeArchive, null)
        KagemushaRecursiveSpendProver.SecretArchiveWiper.wipe(rawNativeArchive)
        assertTrue(rawNativeArchive.all { it == 0.toByte() })
        assertTrue(owner.noritoEncoded().size > NoritoHeader.HEADER_LENGTH)
        owner.close()
        KagemushaRecursiveSpendProver.SecretArchiveWiper.wipe(null)

        val firstCopyObserved = mutableListOf<ByteArray>()
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.SecretArchiveWiper.withOpeningDigests(
                ByteArray(32) { 0x64 },
                "spendKey",
                ByteArray(31),
                "rho",
                ByteArray(32) { 0x66 },
                "diversifier",
                observer = { firstCopyObserved += it },
            ) { _, _, _ -> Unit }
        }
        assertEquals(1, firstCopyObserved.size)
        assertTrue(firstCopyObserved.single().all { it == 0.toByte() })

        val firstAndSecondCopiesObserved = mutableListOf<ByteArray>()
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.SecretArchiveWiper.withOpeningDigests(
                ByteArray(32) { 0x67 },
                "spendKey",
                ByteArray(32) { 0x68 },
                "rho",
                ByteArray(31),
                "diversifier",
                observer = { firstAndSecondCopiesObserved += it },
            ) { _, _, _ -> Unit }
        }
        assertEquals(2, firstAndSecondCopiesObserved.size)
        assertTrue(firstAndSecondCopiesObserved.all { copy -> copy.all { it == 0.toByte() } })

        val rawOpeningArchive = archive("KagemushaNoteOpeningV2", 0x69)
        val openingOwner = KagemushaRecursiveSpendProver.decodeNoteOpening(rawOpeningArchive)
        KagemushaRecursiveSpendProver.SecretArchiveWiper.wipe(rawOpeningArchive)
        assertTrue(rawOpeningArchive.all { it == 0.toByte() })
        assertTrue(openingOwner.noritoEncoded().size > NoritoHeader.HEADER_LENGTH)
        openingOwner.close()

        val rawInitArchive = archive("KagemushaRecursiveSpendInitLocalRequestV4", 0x6a)
        val initOwner = KagemushaRecursiveSpendProver.decodeInitRequestV4(rawInitArchive)
        KagemushaRecursiveSpendProver.SecretArchiveWiper.wipe(rawInitArchive)
        assertTrue(rawInitArchive.all { it == 0.toByte() })
        assertTrue(initOwner.noritoEncoded().size > NoritoHeader.HEADER_LENGTH)
        initOwner.close()
    }

    @Test
    fun noteOpeningAndInitRequestCloseIdempotently() {
        assertTrue(
            AutoCloseable::class.java.isAssignableFrom(
                KagemushaRecursiveSpendProver.NoteOpening::class.java,
            ),
        )
        val opening = KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2", 0x5c),
        )
        opening.close()
        opening.close()
        assertTrue(opening.isDestroyed())
        assertFailsWith<IllegalStateException> { opening.noritoEncoded() }

        assertTrue(
            AutoCloseable::class.java.isAssignableFrom(
                KagemushaRecursiveSpendProver.InitRequestV4::class.java,
            ),
        )
        val init = KagemushaRecursiveSpendProver.decodeInitRequestV4(
            archive("KagemushaRecursiveSpendInitLocalRequestV4", 0x5d),
        )
        init.close()
        init.close()
        assertTrue(init.isDestroyed())
        assertFailsWith<IllegalStateException> { init.noritoEncoded() }
    }

    @Test
    fun canonicalArchiveHashSurvivesZeroizationWithoutRetainingEquality() {
        val encoded = archive("KagemushaNoteOpeningV2", 0x6b)
        val first = KagemushaRecursiveSpendProver.decodeNoteOpening(encoded)
        val equal = KagemushaRecursiveSpendProver.decodeNoteOpening(encoded)
        val different = KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2", 0x6c),
        )
        val initialHashCode = first.hashCode()
        val indexed = hashSetOf(first)
        assertEquals(equal, first)

        val start = CountDownLatch(1)
        val failure = AtomicReference<Throwable?>()
        val forward = Thread {
            try {
                check(start.await(5, TimeUnit.SECONDS))
                repeat(10_000) { first == equal }
                synchronized(first) { first.close() }
            } catch (error: Throwable) {
                failure.compareAndSet(null, error)
            }
        }
        val reverse = Thread {
            try {
                check(start.await(5, TimeUnit.SECONDS))
                repeat(10_000) { equal == first }
            } catch (error: Throwable) {
                failure.compareAndSet(null, error)
            }
        }
        forward.isDaemon = true
        reverse.isDaemon = true
        forward.start()
        reverse.start()
        start.countDown()
        forward.join(5_000)
        reverse.join(5_000)

        assertFalse(forward.isAlive)
        assertFalse(reverse.isAlive)
        failure.get()?.let { throw it }
        assertEquals(first, first)
        assertFalse(first == equal)
        assertFalse(equal == first)
        assertEquals(initialHashCode, first.hashCode())
        assertTrue(first in indexed)
        assertFalse(first == different)
        equal.close()
        different.close()
    }

    @Test
    fun preparationConstructorFailuresDestroyStagedOpenings() {
        val recipientOpening = KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2", 0x5e),
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.RecipientRequestPreparation(
                payload = KagemushaRecursiveSpendProver.RecipientRequestPayload(
                    archive("KagemushaRecipientPaymentRequestSigningPayloadV2", 0x5f),
                ),
                signingBytes = byteArrayOf(1),
                opening = recipientOpening,
                commitment = ByteArray(32) { 0x60 },
                nullifier = ByteArray(31),
                amount = KagemushaScaledAmount.fromAtomicUnits("125", 2),
            )
        }
        assertTrue(recipientOpening.isDestroyed())

        val topUpOpening = KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2", 0x70),
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.TopUpPreparation(
                unsigned = KagemushaRecursiveSpendProver.TopUpUnsigned(
                    archive("KagemushaRecursiveSpendTopUpUnsignedV4", 0x71),
                ),
                authorizationDigest = ByteArray(32) { 0x72 },
                opening = topUpOpening,
                noteCommitment = ByteArray(32) { 0x73 },
                spendNullifier = ByteArray(32) { 0x74 },
                initialRoot = ByteArray(32) { 0x75 },
                finalizedRoot = ByteArray(32) { 0x76 },
                operationId = ByteArray(31),
                amount = KagemushaScaledAmount.fromAtomicUnits("125", 2),
                leafIndex = 0,
            )
        }
        assertTrue(topUpOpening.isDestroyed())
    }

    @Test
    fun artifactContractAndInventoryAreCurrentOnly() {
        assertEquals(23, KagemushaRecursiveSpendProver.REQUIRED_NATIVE_BRIDGE_ABI_VERSION)
        assertEquals(8, KagemushaRecursiveSpendProver.ARTIFACT_COUNT)
        assertEquals(1024 * 1024, KagemushaRecursiveSpendProver.MAX_ARTIFACT_CHUNK_BYTES)
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.requireChunk(
                ByteArray(KagemushaRecursiveSpendProver.MAX_ARTIFACT_CHUNK_BYTES + 1),
            )
        }
        assertEquals(2, KagemushaRecursiveSpendProver.MAXIMUM_INPUTS_PER_TRANSITION)
        assertEquals(2, KagemushaRecursiveSpendProver.MAXIMUM_LOCAL_APPEND_BUILDER_INPUTS)
        assertEquals(2, KagemushaRecursiveSpendProver.MAXIMUM_BRANCH_CLAIMS)
        assertEquals(8, KagemushaRecursiveSpendProver.MAXIMUM_PEER_HOPS)
        assertEquals(
            384 * 1024,
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
        assertEquals(12 * 1024, KagemushaRecursiveSpendProver.MAX_PEER_TEXT_ENVELOPE_BYTES)
        assertEquals(9_211, KagemushaRecursiveSpendProver.MAX_PEER_TEXT_ARCHIVE_BYTES)
        assertEquals(24_576, KagemushaRecursiveSpendProver.MAX_RECIPIENT_RECEIVE_OFFER_BYTES_V2)
        assertEquals(2_048, KagemushaRecursiveSpendProver.MAX_PUBLISHER_CHECKPOINT_ENVELOPE_BYTES_V1)
        assertEquals(40, KagemushaRecursiveSpendProver.PROMOTED_FINALITY_CHECKPOINT_BYTES_V2)
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
        val recipientRequestMethods = KagemushaRecursiveSpendProver::class.java.declaredMethods
            .filter {
                java.lang.reflect.Modifier.isPublic(it.modifiers) &&
                    !it.isSynthetic &&
                    it.name == "prepareRecipientPaymentRequest"
            }
        assertEquals(1, recipientRequestMethods.size)
        assertEquals(13, recipientRequestMethods.single().parameterCount)
        assertEquals(java.lang.Integer.TYPE, recipientRequestMethods.single().parameterTypes[1])
        val nativeRecipientRequest = KagemushaRecursiveSpendProver::class.java.getDeclaredMethod(
            "nativePrepareRecipientRequestV2",
            ByteArray::class.java,
            java.lang.Integer.TYPE,
            ByteArray::class.java,
            ByteArray::class.java,
            java.lang.Integer.TYPE,
            ByteArray::class.java,
            ByteArray::class.java,
            ByteArray::class.java,
            ByteArray::class.java,
            java.lang.Long.TYPE,
            java.lang.Long.TYPE,
            ByteArray::class.java,
            ByteArray::class.java,
            ByteArray::class.java,
        )
        assertEquals(14, nativeRecipientRequest.parameterCount)
        val lineageQueryMethods = KagemushaRecursiveSpendProver::class.java.declaredMethods
            .filter {
                java.lang.reflect.Modifier.isPublic(it.modifiers) &&
                    !it.isSynthetic &&
                    it.name == "createRecipientLineageQueryV2"
            }
        assertEquals(1, lineageQueryMethods.size)
        assertEquals(6, lineageQueryMethods.single().parameterCount)
        assertEquals(java.lang.Integer.TYPE, lineageQueryMethods.single().parameterTypes[1])
        val nativeLineageQuery = KagemushaRecursiveSpendProver::class.java.getDeclaredMethod(
            "nativeCreateRecipientLineageQueryV2",
            ByteArray::class.java,
            java.lang.Integer.TYPE,
            ByteArray::class.java,
            ByteArray::class.java,
            ByteArray::class.java,
            java.lang.Long.TYPE,
        )
        assertEquals(6, nativeLineageQuery.parameterCount)
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
            ByteArray::class.java,
            LongArray::class.java,
        )
        assertEquals(9, nativeInstall.parameterCount)
        val nativeAuthorizationPrepare = KagemushaRecursiveSpendProver::class.java.getDeclaredMethod(
            "nativePrepareAuthorizationV2",
            ByteArray::class.java,
            java.lang.Integer.TYPE,
            ByteArray::class.java,
            ByteArray::class.java,
            ByteArray::class.java,
            java.lang.Long.TYPE,
            java.lang.Long.TYPE,
            ByteArray::class.java,
            ByteArray::class.java,
            ByteArray::class.java,
            ByteArray::class.java,
        )
        assertEquals(arrayOf<ByteArray>().javaClass, nativeAuthorizationPrepare.returnType)
        val retiredAuthorizationFinalizer = listOf(
            "native",
            "Create",
            "Authorization",
            "V2",
        ).joinToString("")
        assertTrue(
            KagemushaRecursiveSpendProver::class.java.declaredMethods.none {
                it.name == retiredAuthorizationFinalizer
            },
        )
        val nativeAuthorizationFinalize = KagemushaRecursiveSpendProver::class.java.getDeclaredMethod(
            "nativeFinalizeHardwareAuthorizationV2",
            ByteArray::class.java,
            ByteArray::class.java,
            ByteArray::class.java,
        )
        assertEquals(arrayOf<ByteArray>().javaClass, nativeAuthorizationFinalize.returnType)
        val nativeIosAuthorizationFinalize = KagemushaRecursiveSpendProver::class.java.getDeclaredMethod(
            "nativeFinalizeIosAppAttestAuthorizationV2",
            ByteArray::class.java,
            ByteArray::class.java,
        )
        assertEquals(arrayOf<ByteArray>().javaClass, nativeIosAuthorizationFinalize.returnType)
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
                "createRecipientLineageQueryV2",
                "createRecipientReceiveOfferV2",
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
                "decodeRecipientReceiveOfferV2",
                "decodeRecipientRegistrationLineageV2",
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
                "finalizeIosAppAttest",
                "finalizeRequestAuthorization",
                "finalizeRedeemV4",
                "finalizeTopUp",
                "initSpendV4",
                "installedArtifactManifestSha256V4",
                "isArtifactStreamingAvailable",
                "isProductionProofBackendCompiled",
                "isProofBackendAvailable",
                "newToriiClient",
                "prepareAcknowledgement",
                "prepareNoteOpening",
                "preparePeerSplitChangeV4",
                "prepareRedemptionChangeV4",
                "prepareRecipientPaymentRequest",
                "prepareRequestAuthorization",
                "prepareTopUp",
                "projectOperationReference",
                "projectOperationStatus",
                "projectRedeemRequestIdentity",
                "projectTopUpRequestIdentity",
                "projectPeerPayment",
                "projectInitResultV4",
                "projectRedeemBuildResultV4",
                "projectRecipientPaymentRequest",
                "projectRecipientReceiveOfferV2",
                "projectSplitResultV4",
                "projectVerifyResultV4",
                "restoreInitBranchV4",
                "restorePeerPaymentBranchV4",
                "restoreRedeemChangeBranchV4",
                "restoreSpendableBranchV4",
                "restoreSplitChangeBranchV4",
                "signAcknowledgement",
                "signRecipientPaymentRequest",
                "verifyAcknowledgement",
                "verifyRecipientPaymentRequest",
                "verifyRecipientReceiveOfferV2",
                "verifyRecipientRegistrationLineageV2",
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
            "decodeReadiness",
            "projectReadiness",
            "nativeProjectReadinessV4",
            "nativeProjectAuthenticatedArtifactSetV4",
            "nativeProjectActiveVerifierV2",
        )) {
            assertFalse(retired in declaredNames, "$retired must remain absent from the exact-state JVM surface")
        }
        val appendBuilder = KagemushaRecursiveSpendProver::class.java.declaredMethods.single {
            it.name == "buildAppendRequestV4" && java.lang.reflect.Modifier.isPublic(it.modifiers)
        }
        assertEquals(List::class.java, appendBuilder.parameterTypes[0])
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
        KagemushaRecursiveSpendProver.ReleaseAuthentication(one, one, one, one, one, one)
        repeat(6) { emptyIndex ->
            val values = Array(6) { one }
            values[emptyIndex] = byteArrayOf()
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.ReleaseAuthentication(
                    values[0], values[1], values[2], values[3], values[4], values[5],
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
                one,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.ReleaseAuthentication(
                one,
                one,
                ByteArray(
                    KagemushaRecursiveSpendProver.MAX_INTERNAL_VALIDATION_RECEIPT_BYTES + 1,
                ),
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
                ByteArray(KagemushaRecursiveSpendProver.MAX_CRYPTOGRAPHIC_REVIEW_BYTES + 1),
                one,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.ReleaseAuthentication(
                one,
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
        for (schema in listOf(
            "iroha.torii.v1.offline.top_up.request",
            "iroha.torii.v1.offline.redeem.request",
        )) {
            val canonical = archive(schema)
            val missingPadding = canonical.copyOfRange(0, NoritoHeader.HEADER_LENGTH) +
                canonical.copyOfRange(NoritoHeader.HEADER_LENGTH + 8, canonical.size)
            assertFailsWith<IllegalArgumentException> {
                if (schema.contains("top_up")) {
                    KagemushaRecursiveSpendProver.decodeTopUpRequest(missingPadding)
                } else {
                    KagemushaRecursiveSpendProver.decodeRedeemSubmissionRequest(missingPadding)
                }
            }
        }
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
                    archive("iroha_data_model::offline::model::KagemushaRecipientPaymentRequestV2"),
                ),
                0,
            )
        }
        if (!KagemushaRecursiveSpendProver.isProofBackendAvailable()) {
            assertFailsWith<IllegalStateException> {
                KagemushaRecursiveSpendProver.initSpendV4(init)
            }
            assertTrue(init.isDestroyed())
            assertFailsWith<IllegalStateException> {
                KagemushaRecursiveSpendProver.initSpendV4(init)
            }
        }
        init.close()
        init.close()
        assertTrue(init.isDestroyed())
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
            archive("iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4"),
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
        assertEquals(65_463, KagemushaRecursiveSpendProver.TOP_UP_SHIELD_INSERTION_CAPACITY)
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
        val lastValidLeaf = KagemushaRecursiveSpendProver.TOP_UP_SHIELD_INSERTION_CAPACITY - 1
        val lastValidDirections = ByteArray(16) { level ->
            ((lastValidLeaf ushr level) and 1).toByte()
        }
        assertEquals(
            lastValidLeaf,
            KagemushaRecursiveSpendProver.TopUpZeroPath(
                lastValidLeaf,
                List(16) { ByteArray(32) },
                lastValidDirections,
                ByteArray(32) { 1 },
            ).leafIndex,
        )
        val reservedLeaf = KagemushaRecursiveSpendProver.TOP_UP_SHIELD_INSERTION_CAPACITY
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.TopUpZeroPath(
                reservedLeaf,
                List(16) { ByteArray(32) },
                ByteArray(16) { level -> ((reservedLeaf ushr level) and 1).toByte() },
                ByteArray(32) { 1 },
            )
        }
    }

    @Test
    fun scaledAmountsAreExactAndNeverRound() {
        val amount = KagemushaScaledAmount.fromDecimal("10.75", 9)
        assertEquals("10750000000", amount.atomicUnits)
        assertEquals("10.750000000", amount.fixedScaleDecimal)
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
            KagemushaScaledAmount.fromAtomicUnits("1", 9).fixedScaleDecimal,
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
        requireNativeArtifactStreaming()
        val requestArchive = archive(
            "iroha_data_model::offline::model::KagemushaRecipientPaymentRequestV2",
        )
        val request = KagemushaRecursiveSpendProver.decodeRecipientPaymentRequest(requestArchive)
        requestArchive[requestArchive.lastIndex] = 0
        assertEquals(0x51, request.noritoEncoded().last().toInt() and 0xff)

        val offer = KagemushaRecursiveSpendProver.decodeRecipientReceiveOfferV2(
            portableOfferFixture("offline_recipient_receive_offer_v2.hex"),
        )
        val offerProjection = KagemushaRecursiveSpendProver.projectRecipientReceiveOfferV2(offer)
        assertContentEquals(
            portableOfferFixture("offline_recipient_payment_request_v2.hex"),
            offerProjection.request.noritoEncoded(),
        )
        assertContentEquals(
            portableOfferFixture("offline_recipient_registration_lineage_v2.hex"),
            offerProjection.lineage.noritoEncoded(),
        )
        assertContentEquals(
            portableOfferFixture("offline_recipient_checkpoint_envelope.hex"),
            offerProjection.publisherCheckpointEnvelope(),
        )

        assertTrue(
            KagemushaRecursiveSpendProver.decodePeerPayment(
                archive("iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4"),
            ).noritoEncoded().size > NoritoHeader.HEADER_LENGTH,
        )
        assertTrue(
            KagemushaRecursiveSpendProver.decodeReceiverAcknowledgement(
                archive("iroha_data_model::offline::model::KagemushaReceiverAcknowledgementV2"),
            ).noritoEncoded().size > NoritoHeader.HEADER_LENGTH,
        )
        assertTrue(
            KagemushaRecursiveSpendProver.decodeNoteMembershipWitness(
                archive("KagemushaNoteMembershipWitnessV2"),
            ).noritoEncoded().size > NoritoHeader.HEADER_LENGTH,
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.decodePeerPayment(
                archive("iroha_data_model::offline::model::KagemushaRecipientPaymentRequestV2"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.decodePeerPayment(byteArrayOf(1, 2, 3))
        }
        val corrupted = archive(
            "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4",
        )
        corrupted[corrupted.lastIndex] = 0
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.decodePeerPayment(corrupted)
        }
    }

    @Test
    fun peerTransportGoldenVectorsAreExact() {
        requireNativeArtifactStreaming()
        val offerArchive = portableOfferFixture("offline_recipient_receive_offer_v2.hex")
        val request = KagemushaPeerPayload.decode(
            offerArchive,
            KagemushaPeerPayloadKind.RECEIVE_REQUEST,
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaPeerTextCodec.encode(request)
        }
        val oversizedText = KagemushaPeerTransportContract.RECEIVE_REQUEST_TEXT_PREFIX +
            KagemushaPeerTextCodec.base64UrlEncode(offerArchive)
        assertTrue(
            oversizedText.toByteArray(Charsets.UTF_8).size >
                KagemushaRecursiveSpendProver.MAX_PEER_TEXT_ENVELOPE_BYTES,
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaPeerTextCodec.decode(oversizedText)
        }
        val acknowledgementArchive = archive(
            "iroha_data_model::offline::model::KagemushaReceiverAcknowledgementV2",
        )
        val acknowledgement = KagemushaPeerPayload.decode(
            acknowledgementArchive,
            KagemushaPeerPayloadKind.ACKNOWLEDGEMENT,
        )
        val text = KagemushaPeerTextCodec.encode(acknowledgement)

        assertTrue(text.startsWith("PKK2A."))
        assertContentEquals(acknowledgementArchive, KagemushaPeerTextCodec.decode(text).archive())
        assertEquals(
            KagemushaPeerPayloadKind.ACKNOWLEDGEMENT,
            KagemushaPeerTextCodec.decode(text).kind,
        )
        assertEquals(
            KagemushaPeerPayloadKind.ACKNOWLEDGEMENT,
            KagemushaPeerTextCodec.decodeUserPresented(" \n$text\t").kind,
        )
        assertEquals(
            KagemushaRecursiveSpendProver.MAX_PEER_TEXT_ENVELOPE_BYTES,
            KagemushaPeerTransportContract.ACKNOWLEDGEMENT_TEXT_PREFIX.length +
                KagemushaPeerTextCodec.base64UrlEncode(
                    ByteArray(KagemushaRecursiveSpendProver.MAX_PEER_TEXT_ARCHIVE_BYTES),
                ).length,
        )
        assertTrue(
            KagemushaPeerTransportContract.ACKNOWLEDGEMENT_TEXT_PREFIX.length +
                KagemushaPeerTextCodec.base64UrlEncode(
                    ByteArray(KagemushaRecursiveSpendProver.MAX_PEER_TEXT_ARCHIVE_BYTES + 1),
                ).length > KagemushaRecursiveSpendProver.MAX_PEER_TEXT_ENVELOPE_BYTES,
        )
        assertEquals("PKK2R.", KagemushaPeerTransportContract.RECEIVE_REQUEST_TEXT_PREFIX)
        assertEquals("PKK2P.", KagemushaPeerTransportContract.PAYMENT_TEXT_PREFIX)
        assertEquals("PKK2A.", KagemushaPeerTransportContract.ACKNOWLEDGEMENT_TEXT_PREFIX)
        assertEquals("PKKQ1.", KagemushaPeerTransportContract.QR_STREAM_TEXT_PREFIX)
    }

    @Test
    fun qrNfcAndNearbyGoldenVectorsAreExact() {
        requireNativeArtifactStreaming()
        val offerArchive = portableOfferFixture("offline_recipient_receive_offer_v2.hex")
        val request = KagemushaPeerPayload.decode(
            offerArchive,
            KagemushaPeerPayloadKind.RECEIVE_REQUEST,
        )
        val frames = KagemushaQrStreamCodec.encode(
            request,
            KagemushaQrStreamOptions.STANDARD,
        )
        assertTrue(frames.size > 3)
        assertTrue(frames.all { it.startsWith("PKKQ1.") })
        val decoder = KagemushaQrStreamDecoder()
        var recovered: KagemushaQrDecodeResult? = null
        frames.forEach { frame -> recovered = decoder.ingest(frame) }
        val completed = requireNotNull(recovered)
        assertTrue(completed.isComplete)
        assertContentEquals(offerArchive, requireNotNull(completed.payload).archive())

        val rawArchive = request.archive()
        val commands = KagemushaNfcProtocol.writePayloadCommands(
            KagemushaPeerPayloadKind.RECEIVE_REQUEST,
            rawArchive,
            KagemushaNfcProtocol.SAFE_CHUNK_BYTES,
        )
        assertTrue(commands.size > 3)
        assertEquals("8022040000", commands.last().toHex())
        assertEquals(
            "F0504B45504B524E464301",
            KagemushaNfcProtocol.applicationIdentifierHex(
                KagemushaNfcProtocol.defaultApplicationIdentifier(),
            ),
        )
        assertEquals(
            IrohaPeerNfcV1.APPLICATION_IDENTIFIER_HEX,
            KagemushaPeerTransportContract.NFC_APPLICATION_IDENTIFIER_HEX,
        )
        assertEquals(220, KagemushaNfcProtocol.SAFE_CHUNK_BYTES)
        assertEquals(4, KagemushaNfcProtocol.RAW_TRANSPORT_VERSION)
        assertTrue(KagemushaNfcProtocol.parseCommand(commands[0]) is KagemushaNfcCommand.WriteMetadata)

        val nearby = KagemushaNearbyEnvelopeCodec.encode(
            request,
            KagemushaNearbyPairingChallenge(KagemushaNearbyPairingSymbol.STARS),
        )
        assertEquals(offerArchive.size + IrohaPeerWireMessageV1.HEADER_LENGTH + 12, nearby.size)
        assertEquals("504b4e4231010100", nearby.copyOfRange(0, 8).toHex())
        assertEquals(nearby.size - 12, nearby.copyOfRange(8, 12).readU32BeForTest())
        val nearbyDecoded = KagemushaNearbyEnvelopeCodec.decode(nearby)
        assertEquals(
            KagemushaPeerPayloadKind.RECEIVE_REQUEST,
            nearbyDecoded.payload?.kind,
        )
        assertContentEquals(offerArchive, requireNotNull(nearbyDecoded.payload).archive())
        assertEquals(
            "504b4e423104000000000000",
            KagemushaNearbyEnvelopeCodec.encodeRejection().toHex(),
        )
        assertEquals(
            KagemushaNearbyMessageKind.REJECTED,
            KagemushaNearbyEnvelopeCodec.decode(
                KagemushaNearbyEnvelopeCodec.encodeRejection(),
            ).messageKind,
        )
        assertFalse(KagemushaNearbyTransportPolicy.IS_AVAILABLE)
        rawArchive.fill(0)
        nearby.fill(0)
    }

    private fun requireNativeArtifactStreaming() {
        assertTrue(
            KagemushaRecursiveSpendProver.isArtifactStreamingAvailable(),
            "A freshly built connect_norito_bridge ABI 23 artifact-streaming library is required",
        )
    }

    private fun ByteArray.readU32BeForTest(): Int =
        ((this[0].toInt() and 0xff) shl 24) or
            ((this[1].toInt() and 0xff) shl 16) or
            ((this[2].toInt() and 0xff) shl 8) or
            (this[3].toInt() and 0xff)

    @Test
    fun nfcV4StreamsBeyondLegacyLimitAndRejectsDowngrade() {
        val mutableAid = KagemushaNfcProtocol.defaultApplicationIdentifier()
        mutableAid.fill(0)
        assertEquals(
            "F0504B45504B524E464301",
            KagemushaNfcProtocol.defaultApplicationIdentifier().toHex().uppercase(),
        )
        val mutableSuccess = KagemushaNfcProtocol.statusSuccess()
        mutableSuccess.fill(0)
        assertEquals(0x9000, KagemushaNfcProtocol.responseStatus(KagemushaNfcProtocol.response()))

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
        val networkId = org.hyperledger.iroha.sdk.core.model.NetworkId.parse(
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
        )
        val otherNetworkId = org.hyperledger.iroha.sdk.core.model.NetworkId.parse(
            "hash:0E5751C026E543B2E8AB2EB06099DAA1D1E5DF47778F7787FAAB45CDF12FE3A9#6A22",
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val timestampMs = 1_700_000_000_500L
        val nonce = "offline-lineage-1"
        val canonicalAuth = ToriiCanonicalRequestAuth(
            "alice@universal",
            keyPair.private,
            timestampMs,
            nonce,
        )
        val captured = AtomicReference<TransportRequest>()
        val neverTransport = object : TransportExecutor {
            override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
                throw AssertionError("invalid Torii base URI reached transport")
        }
        listOf("http:opaque", "https:/missing-host").forEach { invalidBaseUri ->
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.newToriiClient(
                    URI.create(invalidBaseUri),
                    neverTransport,
                    LocalSigningContext(networkId),
                )
            }
        }
        val transport = object : TransportExecutor {
            override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                captured.set(request)
                val capability = request.uri.path.endsWith("/readiness")
                val lineage = request.uri.path.endsWith("/receiver-lineage")
                val command = request.method == "POST" && !lineage
                return CompletableFuture.completedFuture(
                    TransportResponse.builder()
                        .setStatusCode(if (command) 202 else 200)
                        .addHeader(
                            "content-type",
                            if (capability) "application/json" else "application/x-norito",
                        )
                        .addHeader(
                            "Location",
                            "/v1/offline/operations/${"11".repeat(32)}",
                        )
                        .addHeader("Retry-After", "1")
                        .setBody(
                            if (capability) {
                                """{"cash_handoff_capability":"cash_handoff_v1","required_bridge_abi_version":23,"max_hops":8,"ready":true}"""
                                    .toByteArray(StandardCharsets.UTF_8)
                            } else {
                                archive(
                                    if (command) {
                                        "OfflineOperationReference"
                                    } else if (lineage) {
                                        "iroha_torii_shared::offline_api::OfflineRecipientRegistrationLineage"
                                    } else if (request.uri.path.contains("/operations/")) {
                                        "OfflineOperationStatus"
                                    } else {
                                        error("unexpected Torii route")
                                    },
                                )
                            },
                        )
                        .build(),
                )
            }
        }
        val toriiBaseUri = URI.create("https://torii.example/api/")
        val localSigningContext = LocalSigningContext(networkId)
        KagemushaRecursiveSpendProver.newToriiClient(
            toriiBaseUri,
            transport,
            localSigningContext,
        ).getOfflineCapability().join()
        assertEquals(null, captured.get().timeout)
        KagemushaRecursiveSpendProver.newToriiClient(
            toriiBaseUri,
            transport,
            localSigningContext,
            null,
        ).getOfflineCapability().join()
        assertEquals(null, captured.get().timeout)
        KagemushaRecursiveSpendProver.newToriiClient(
            toriiBaseUri,
            transport,
            localSigningContext,
            Duration.ZERO,
        ).getOfflineCapability().join()
        assertEquals(Duration.ZERO, captured.get().timeout)
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.newToriiClient(
                toriiBaseUri,
                transport,
                localSigningContext,
                Duration.ofMillis(-1),
            )
        }
        val requestTimeout = Duration.ofSeconds(37)
        val projectedOperationId = ByteArray(32) { 0x11 }
        val topUpIdentity = KagemushaRecursiveSpendProver.OperationRequestIdentity(
            KagemushaRecursiveSpendProver.OperationKind.TOP_UP,
            projectedOperationId,
            1,
        )
        val redeemIdentity = KagemushaRecursiveSpendProver.OperationRequestIdentity(
            KagemushaRecursiveSpendProver.OperationKind.REDEEM,
            projectedOperationId,
            1,
        )
        val client = KagemushaRecursiveSpendProver.ToriiClient(
            toriiBaseUri,
            transport,
            localSigningContext,
            requestTimeout,
            topUpRequestIdentityProjector = { topUpIdentity },
            redeemRequestIdentityProjector = { redeemIdentity },
            operationReferenceProjector = {
                KagemushaRecursiveSpendProver.OperationReferenceProjection(
                    if (captured.get().uri.path.endsWith("/redeem")) {
                        KagemushaRecursiveSpendProver.OperationKind.REDEEM
                    } else {
                        KagemushaRecursiveSpendProver.OperationKind.TOP_UP
                    },
                    projectedOperationId,
                    projectedOperationId,
                    "/v1/offline/operations/${"11".repeat(32)}",
                    1,
                )
            },
            operationStatusProjector = {
                KagemushaRecursiveSpendProver.OperationStatusProjection(
                    KagemushaRecursiveSpendProver.OperationState.PENDING,
                    KagemushaRecursiveSpendProver.OperationKind.TOP_UP,
                    projectedOperationId,
                    projectedOperationId,
                    1,
                    null,
                    null,
                    null,
                    null,
                )
            },
        )

        val status = client.getOfflineCapability().join()
        assertTrue(
            KagemushaRecursiveSpendProver.ToriiClient::class.java.declaredMethods.none {
                it.name == "getReadiness"
            },
            "selector-taking offline readiness alias must remain absent",
        )
        assertEquals("cash_handoff_v1", status.cashHandoffCapability)
        assertEquals(23, status.requiredBridgeAbiVersion)
        assertEquals(8, status.maximumHops)
        assertTrue(status.ready)
        assertEquals(
            "https://torii.example/api/v1/offline/readiness",
            captured.get().uri.toString(),
        )
        assertEquals(listOf("application/json"), captured.get().headers["Accept"])
        assertEquals(requestTimeout, captured.get().timeout)

        val lineageQuery = KagemushaRecursiveSpendProver.RecipientLineageQueryV2(
            archive("iroha_torii_shared::offline_api::OfflineRecipientLineageRequest"),
        )
        client.getRecipientRegistrationLineage(lineageQuery, canonicalAuth).join()
        val lineageRequest = captured.get()
        assertEquals("/api/v1/offline/receiver-lineage", lineageRequest.uri.path)
        assertEquals(listOf("application/x-norito"), lineageRequest.headers["Content-Type"])
        assertEquals(
            listOf("alice@universal"),
            lineageRequest.headers[CanonicalRequestSigner.HEADER_ACCOUNT],
        )
        assertEquals(
            listOf(timestampMs.toString()),
            lineageRequest.headers[CanonicalRequestSigner.HEADER_TIMESTAMP_MS],
        )
        assertEquals(listOf(nonce), lineageRequest.headers[CanonicalRequestSigner.HEADER_NONCE])
        assertEquals(RequestReplayPolicy.ONE_SHOT, lineageRequest.replayPolicy)
        assertEquals(requestTimeout, lineageRequest.timeout)
        val signature = Base64.getDecoder().decode(
            lineageRequest.headers.getValue(CanonicalRequestSigner.HEADER_SIGNATURE).single(),
        )
        fun verifiesOn(
            candidate: org.hyperledger.iroha.sdk.core.model.NetworkId = networkId,
            method: String = lineageRequest.method,
            uri: URI = lineageRequest.uri,
            body: ByteArray = lineageRequest.body,
        ): Boolean {
            val verifier = Signature.getInstance("Ed25519")
            verifier.initVerify(keyPair.public)
            verifier.update(
                CanonicalRequestSigner.canonicalRequestSignatureMessage(
                    candidate,
                    method,
                    uri,
                    body,
                    timestampMs,
                    nonce,
                ),
            )
            return verifier.verify(signature)
        }
        assertTrue(verifiesOn())
        assertFalse(verifiesOn(candidate = otherNetworkId))
        assertFalse(verifiesOn(method = "GET"))
        assertFalse(verifiesOn(uri = URI.create("https://torii.example/api/v1/offline/readiness")))
        assertFalse(verifiesOn(body = lineageRequest.body + byteArrayOf(0)))

        val operationId = "11".repeat(32)
        val topUpHandle = client.submitTopUp(
            KagemushaRecursiveSpendProver.TopUpRequest(
                archive("iroha.torii.v1.offline.top_up.request"),
            ),
        ).join()
        assertEquals(operationId, topUpHandle.operationIdHex())
        assertEquals("POST", captured.get().method)
        assertEquals("/api/v1/offline/top-up", captured.get().uri.path)
        assertEquals(
            listOf("application/x-norito"),
            captured.get().headers["Content-Type"],
        )
        assertEquals(listOf(operationId), captured.get().headers["Idempotency-Key"])
        assertEquals(requestTimeout, captured.get().timeout)

        client.submitRedeem(
            KagemushaRecursiveSpendProver.RedeemSubmissionRequest(
                archive("iroha.torii.v1.offline.redeem.request"),
            ),
        ).join()
        assertEquals("/api/v1/offline/redeem", captured.get().uri.path)
        assertEquals(requestTimeout, captured.get().timeout)

        client.getOperation(topUpHandle).join()
        assertEquals("/api/v1/offline/operations/$operationId", captured.get().uri.path)
        assertEquals(requestTimeout, captured.get().timeout)
    }

    @Test
    fun operationStatusProjectionRejectsZeroTimesAndUnstableCodes() {
        val digest = ByteArray(32) { 1 }
        fun projection(
            state: KagemushaRecursiveSpendProver.OperationState,
            kind: KagemushaRecursiveSpendProver.OperationKind,
            transactionHash: ByteArray = digest,
            submittedAt: Long? = null,
            finalizedHeight: Long? = null,
            serverTime: Long? = null,
            rejection: KagemushaRecursiveSpendProver.OperationRejection? = null,
        ) = KagemushaRecursiveSpendProver.OperationStatusProjection(
            state,
            kind,
            digest,
            transactionHash,
            submittedAt,
            finalizedHeight,
            serverTime,
            null,
            rejection,
        )

        assertEquals(
            1,
            projection(
                KagemushaRecursiveSpendProver.OperationState.PENDING,
                KagemushaRecursiveSpendProver.OperationKind.TOP_UP,
                submittedAt = 1,
            ).submittedAtMilliseconds,
        )
        assertFailsWith<IllegalArgumentException> {
            projection(
                KagemushaRecursiveSpendProver.OperationState.PENDING,
                KagemushaRecursiveSpendProver.OperationKind.TOP_UP,
                submittedAt = 0,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            projection(
                KagemushaRecursiveSpendProver.OperationState.PENDING,
                KagemushaRecursiveSpendProver.OperationKind.TOP_UP,
                transactionHash = digest.copyOf().also { it[it.lastIndex] = 2 },
                submittedAt = 1,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            projection(
                KagemushaRecursiveSpendProver.OperationState.APPLIED,
                KagemushaRecursiveSpendProver.OperationKind.REDEEM,
                finalizedHeight = 0,
                serverTime = 1,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            projection(
                KagemushaRecursiveSpendProver.OperationState.APPLIED,
                KagemushaRecursiveSpendProver.OperationKind.REDEEM,
                finalizedHeight = 1,
                serverTime = 0,
            )
        }
        for (code in listOf("_invalid", "UPPER", "bad-code", "a".repeat(65))) {
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.OperationRejection(code, "rejected")
            }
        }
        val rejection = KagemushaRecursiveSpendProver.OperationRejection(
            "offline_operation_rejected",
            "rejected",
        )
        assertEquals(
            1_024,
            KagemushaRecursiveSpendProver.OperationRejection(
                "offline_operation_rejected",
                "😀".repeat(1_024),
            ).message.codePointCount(0, "😀".repeat(1_024).length),
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.OperationRejection(
                "offline_operation_rejected",
                "x".repeat(1_025),
            )
        }
        assertEquals(
            rejection,
            projection(
                KagemushaRecursiveSpendProver.OperationState.REJECTED,
                KagemushaRecursiveSpendProver.OperationKind.REDEEM,
                rejection = rejection,
            ).rejection,
        )
    }

    @Test
    fun toriiCommandsRejectNoncanonicalRecoveryHeaders() {
        val operationId = "11".repeat(32)
        val expectedLocation = "/v1/offline/operations/$operationId"
        val networkId = org.hyperledger.iroha.sdk.core.model.NetworkId.parse(
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
        )
        val request = KagemushaRecursiveSpendProver.TopUpRequest(
            archive("iroha.torii.v1.offline.top_up.request"),
        )
        fun client(
            locations: List<String>,
            retryAfter: List<String>,
        ): KagemushaRecursiveSpendProver.ToriiClient =
            KagemushaRecursiveSpendProver.ToriiClient(
                URI.create("https://torii.example"),
                object : TransportExecutor {
                    override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                        val response = TransportResponse.builder()
                            .setStatusCode(202)
                            .addHeader("Content-Type", "application/x-norito")
                            .setBody(archive("OfflineOperationReference"))
                        locations.forEach { response.addHeader("Location", it) }
                        retryAfter.forEach { response.addHeader("Retry-After", it) }
                        return CompletableFuture.completedFuture(response.build())
                    }
                },
                LocalSigningContext(networkId),
                null,
                topUpRequestIdentityProjector = {
                    KagemushaRecursiveSpendProver.OperationRequestIdentity(
                        KagemushaRecursiveSpendProver.OperationKind.TOP_UP,
                        ByteArray(32) { 0x11 },
                        1,
                    )
                },
            )

        val invalidHeaders = listOf(
            emptyList<String>() to listOf("1"),
            listOf("/v1/offline/operations/${"22".repeat(32)}") to listOf("1"),
            listOf(expectedLocation) to emptyList(),
            listOf(expectedLocation) to listOf("0"),
            listOf(expectedLocation) to listOf("18446744073709551616"),
            listOf(expectedLocation) to listOf("9".repeat(10_000)),
            listOf(expectedLocation) to listOf("1", "2"),
        )
        invalidHeaders.forEach { (locations, retryAfter) ->
            assertFailsWith<RuntimeException> {
                client(locations, retryAfter).submitTopUp(request).join()
            }
        }
    }

    @Test
    fun toriiOperationIdentityBindingsRejectSubstitutionBeforeCompletion() {
        val operationId = ByteArray(32) { 0x11 }
        val otherOperationId = ByteArray(32) { 0x13 }
        val operationIdHex = operationId.toHex()
        val identity = KagemushaRecursiveSpendProver.OperationRequestIdentity(
            KagemushaRecursiveSpendProver.OperationKind.TOP_UP,
            operationId,
            7,
        )
        val reference = KagemushaRecursiveSpendProver.OperationReferenceProjection(
            KagemushaRecursiveSpendProver.OperationKind.TOP_UP,
            operationId,
            operationId,
            "/v1/offline/operations/$operationIdHex",
            7,
        )
        val handle = KagemushaRecursiveSpendProver.ToriiClient.requireOperationReferenceMatches(
            reference,
            identity,
        )
        assertEquals(operationIdHex, handle.operationIdHex())
        assertEquals(7, handle.submittedAtMilliseconds)
        assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.ToriiClient.requireOperationReferenceMatches(
                reference,
                KagemushaRecursiveSpendProver.OperationRequestIdentity(
                    KagemushaRecursiveSpendProver.OperationKind.TOP_UP,
                    otherOperationId,
                    7,
                ),
            )
        }
        assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.ToriiClient.requireOperationReferenceMatches(
                reference,
                KagemushaRecursiveSpendProver.OperationRequestIdentity(
                    KagemushaRecursiveSpendProver.OperationKind.REDEEM,
                    operationId,
                    7,
                ),
            )
        }
        assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.ToriiClient.requireOperationReferenceMatches(
                reference,
                KagemushaRecursiveSpendProver.OperationRequestIdentity(
                    KagemushaRecursiveSpendProver.OperationKind.TOP_UP,
                    operationId,
                    8,
                ),
            )
        }
        val status = KagemushaRecursiveSpendProver.OperationStatusProjection(
            KagemushaRecursiveSpendProver.OperationState.PENDING,
            KagemushaRecursiveSpendProver.OperationKind.TOP_UP,
            operationId,
            operationId,
            7,
            null,
            null,
            null,
            null,
        )
        KagemushaRecursiveSpendProver.ToriiClient.requireOperationStatusMatches(status, handle)
        assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.ToriiClient.requireOperationStatusMatches(
                status,
                KagemushaRecursiveSpendProver.OperationHandle(
                    otherOperationId,
                    KagemushaRecursiveSpendProver.OperationKind.TOP_UP,
                    operationId,
                    7,
                ),
            )
        }
        assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.ToriiClient.requireOperationStatusMatches(
                status,
                KagemushaRecursiveSpendProver.OperationHandle(
                    operationId,
                    KagemushaRecursiveSpendProver.OperationKind.REDEEM,
                    operationId,
                    7,
                ),
            )
        }
        assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.ToriiClient.requireOperationStatusMatches(
                status,
                KagemushaRecursiveSpendProver.OperationHandle(
                    operationId,
                    KagemushaRecursiveSpendProver.OperationKind.TOP_UP,
                    otherOperationId,
                    7,
                ),
            )
        }
        assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.ToriiClient.requireOperationStatusMatches(
                status,
                KagemushaRecursiveSpendProver.OperationHandle(
                    operationId,
                    KagemushaRecursiveSpendProver.OperationKind.TOP_UP,
                    operationId,
                    8,
                ),
            )
        }
        val appliedRedeem = KagemushaRecursiveSpendProver.OperationStatusProjection(
            KagemushaRecursiveSpendProver.OperationState.APPLIED,
            KagemushaRecursiveSpendProver.OperationKind.REDEEM,
            operationId,
            operationId,
            null,
            1,
            1,
            null,
            null,
        )
        assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.ToriiClient.requireOperationStatusMatches(
                appliedRedeem,
                handle,
            )
        }

        val dispatched = AtomicReference<TransportRequest?>()
        val networkId = org.hyperledger.iroha.sdk.core.model.NetworkId.parse(
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
        )
        val client = KagemushaRecursiveSpendProver.ToriiClient(
            URI.create("https://torii.example"),
            object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                    dispatched.set(request)
                    throw AssertionError("mismatched request must fail before transport")
                }
            },
            LocalSigningContext(networkId),
            null,
            topUpRequestIdentityProjector = {
                KagemushaRecursiveSpendProver.OperationRequestIdentity(
                    KagemushaRecursiveSpendProver.OperationKind.REDEEM,
                    otherOperationId,
                    7,
                )
            },
        )
        val request = KagemushaRecursiveSpendProver.TopUpRequest(
            archive("iroha.torii.v1.offline.top_up.request"),
        )
        assertFailsWith<IllegalStateException> { client.submitTopUp(request) }
        assertEquals(null, dispatched.get())
    }

    @Test
    fun offlineCapabilityRejectsRetiredReadinessClaims() {
        val invalidPayloads = listOf(
            """{"mandatory":true,"cash_handoff_capability":"cash_handoff_v1","required_bridge_abi_version":23,"max_hops":8,"ready":true,"assets":[],"blockers":[]}""",
            """{"cash_handoff_capability":"cash_handoff_v2","required_bridge_abi_version":23,"max_hops":8,"ready":true}""",
            """{"cash_handoff_capability":"cash_handoff_v1","required_bridge_abi_version":22,"max_hops":8,"ready":true}""",
            """{"cash_handoff_capability":"cash_handoff_v1","required_bridge_abi_version":23,"max_hops":9,"ready":true}""",
            """{"cash_handoff_capability":"cash_handoff_v1","required_bridge_abi_version":23,"max_hops":8,"ready":false}""",
            """{"cash_handoff_capability":"cash_handoff_v1","required_bridge_abi_version":23,"max_hops":8,"ready":true,"assets":[{}],"blockers":[]}""",
            """{"cash_handoff_capability":"cash_handoff_v1","required_bridge_abi_version":23,"max_hops":8,"ready":true,"assets":[],"blockers":[{"code":"unexpected","message":"unexpected"}]}""",
            """{"cash_handoff_capability":"cash_handoff_v1","required_bridge_abi_version":23,"max_hops":8,"ready":true,"future":true}""",
        )
        invalidPayloads.forEach { payload ->
            assertFailsWith<RuntimeException> {
                KagemushaRecursiveSpendProver.OfflineStatus.decode(
                    payload.toByteArray(StandardCharsets.UTF_8),
                )
            }
        }
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

    private fun appendChangeOutputMembershipPaths():
        KagemushaRecursiveSpendProver.OutputMembershipPaths {
        val initialRoot = ByteArray(32) { 0x31 }
        val afterRecipientRoot = ByteArray(32) { 0x32 }
        val finalRoot = ByteArray(32) { 0x33 }
        return KagemushaRecursiveSpendProver.OutputMembershipPaths(
            initialRoot = initialRoot,
            finalRoot = finalRoot,
            recipient = KagemushaRecursiveSpendProver.OutputMembershipLeafPaths(
                updatePath = outputMembershipPath(initialRoot, 0),
                membershipPath = outputMembershipPath(finalRoot, 0),
            ),
            change = KagemushaRecursiveSpendProver.OutputMembershipLeafPaths(
                updatePath = outputMembershipPath(afterRecipientRoot, 1),
                membershipPath = outputMembershipPath(finalRoot, 1),
            ),
            dummyPath = outputMembershipPath(finalRoot, 2),
        )
    }

    private fun redemptionChangeOutputMembershipPaths():
        KagemushaRecursiveSpendProver.OutputMembershipPaths {
        val initialRoot = ByteArray(32) { 0x31 }
        val finalRoot = ByteArray(32) { 0x32 }
        return KagemushaRecursiveSpendProver.OutputMembershipPaths(
            initialRoot = initialRoot,
            finalRoot = finalRoot,
            recipient = null,
            change = KagemushaRecursiveSpendProver.OutputMembershipLeafPaths(
                updatePath = outputMembershipPath(initialRoot, 0),
                membershipPath = outputMembershipPath(finalRoot, 0),
            ),
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
        val padding = when (schema) {
            "iroha_data_model::offline::model::KagemushaRecipientPaymentRequestV2",
            "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4",
            "iroha.torii.v1.offline.top_up.request",
            "iroha.torii.v1.offline.redeem.request" ->
                ByteArray(8)
            else -> byteArrayOf()
        }
        return header.encode() + padding + payload
    }

    private fun portableOfferFixture(name: String): ByteArray {
        var current: Path? = Paths.get(System.getProperty("user.dir")).toAbsolutePath()
        while (current != null) {
            val candidate = current.resolve("crates/connect_norito_bridge/tests/fixtures").resolve(name)
            if (Files.isRegularFile(candidate)) {
                val hex = Files.readAllBytes(candidate).toString(Charsets.US_ASCII)
                    .filterNot(Char::isWhitespace)
                require(hex.length % 2 == 0)
                return ByteArray(hex.length / 2) { index ->
                    hex.substring(index * 2, index * 2 + 2).toInt(16).toByte()
                }
            }
            current = current.parent
        }
        error("portable Kagemusha fixture is missing: $name")
    }

    private fun ByteArray.toHex(): String =
        joinToString(separator = "") { byte -> "%02x".format(byte.toInt() and 0xff) }
}
