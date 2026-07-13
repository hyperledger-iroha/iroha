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
    fun exactAbi19IsRequired() {
        assertTrue(KagemushaRecursiveSpendProver.isExactBridgeAbi(19))
        assertFalse(KagemushaRecursiveSpendProver.isExactBridgeAbi(20))
        assertTrue(
            KagemushaRecursiveSpendProver.detectExactNativeAvailability(
                loadLibrary = {},
                abiVersion = { 19 },
                symbolProbe = { true },
            ),
        )
    }

    @Test
    fun artifactContractAndInventoryAreCurrentOnly() {
        assertEquals(19, KagemushaRecursiveSpendProver.REQUIRED_NATIVE_BRIDGE_ABI_VERSION)
        assertEquals(6, KagemushaRecursiveSpendProver.ARTIFACT_COUNT)
        assertEquals(1, KagemushaRecursiveSpendProver.MAXIMUM_INPUTS_PER_TRANSITION)
        assertEquals(64, KagemushaRecursiveSpendProver.MAXIMUM_PEER_HOPS)
        assertEquals(16, KagemushaRecursiveSpendProver.CONFIDENTIAL_TREE_DEPTH)
        assertEquals(
            "kagemusha.offline.recursive_spend.artifact_manifest.v3",
            KagemushaRecursiveSpendProver.ARTIFACT_MANIFEST_SCHEMA,
        )
        assertEquals(
            listOf(
                "step-eq.parameters.krv3",
                "step-eq.proving-key.krv3",
                "step-eq.verifying-key.krv3",
                "step-ep.parameters.krv3",
                "step-ep.proving-key.krv3",
                "step-ep.verifying-key.krv3",
            ),
            KagemushaRecursiveSpendProver.ARTIFACT_FILES,
        )
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
                "appendSpend",
                "buildAppendRequest",
                "buildInitRequest",
                "buildRedeem",
                "buildRedeemRequest",
                "buildVerifyRequest",
                "decodeAppendRequest",
                "decodeInitRequest",
                "decodeInitResult",
                "decodeNoteMembershipWitness",
                "decodeNoteOpening",
                "decodePeerPayment",
                "decodeRedeemRequest",
                "decodeReceiverAcknowledgement",
                "decodeRecipientPaymentRequest",
                "decodeRedeemBuildResult",
                "decodeRedeemSubmissionRequest",
                "decodeSplitResult",
                "decodeTopUpRequest",
                "decodeVerifyRequest",
                "decodeVerifyResult",
                "decodeTopUpFinalityRosterArtifact",
                "finalizeRedeem",
                "finalizeTopUp",
                "initSpend",
                "isArtifactStreamingAvailable",
                "isProofBackendAvailable",
                "newToriiClient",
                "prepareAcknowledgement",
                "prepareNoteOpening",
                "prepareRecipientPaymentRequest",
                "prepareRequestAuthorization",
                "prepareTopUp",
                "projectInitResult",
                "projectOperationStatus",
                "projectPeerPayment",
                "projectRecipientPaymentRequest",
                "projectReadiness",
                "projectRedeemBuildResult",
                "projectSplitResult",
                "projectVerifyResult",
                "restoreSpendableBranch",
                "signAcknowledgement",
                "signRecipientPaymentRequest",
                "signRequestAuthorization",
                "verifyAcknowledgement",
                "verifyRecipientPaymentRequest",
                "verifySpend",
            ),
            methods,
        )
        for (name in listOf(
            "decodeAppendRequest",
            "decodeSplitResult",
            "decodeRedeemRequest",
            "decodeRedeemBuildResult",
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
    }

    @Test
    fun lifecycleArchivesAreTypedDefensiveAndFailClosed() {
        val initBytes = archive("KagemushaRecursiveSpendInitRequestV2")
        val init = KagemushaRecursiveSpendProver.decodeInitRequest(initBytes)
        initBytes[initBytes.lastIndex] = 0
        assertEquals(0x51, init.noritoEncoded().last().toInt() and 0xff)

        val append = KagemushaRecursiveSpendProver.decodeAppendRequest(
            archive("KagemushaRecursiveSpendAppendLocalRequestV2"),
            null,
        )
        val verify = KagemushaRecursiveSpendProver.decodeVerifyRequest(
            archive("KagemushaRecursiveSpendVerifyRequestV2"),
        )
        val redeem = KagemushaRecursiveSpendProver.decodeRedeemRequest(
            archive("KagemushaRecursiveSpendRedeemLocalRequestV2"),
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
            KagemushaRecursiveSpendProver.decodeInitResult(
                archive("KagemushaRecursiveSpendInitResultV2"),
            ).noritoEncoded().isNotEmpty(),
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.decodeVerifyRequest(
                archive("KagemushaRecursiveSpendInitRequestV2"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.appendSpend(
                append,
                KagemushaRecursiveSpendProver.decodeRecipientPaymentRequest(
                    archive("KagemushaRecipientPaymentRequestV2"),
                ),
                0,
            )
        }
        if (!KagemushaRecursiveSpendProver.isProofBackendAvailable()) {
            assertFailsWith<IllegalStateException> {
                KagemushaRecursiveSpendProver.initSpend(init)
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
            64,
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

        val text = KagemushaPeerTextCodec.encode(request).toByteArray(Charsets.UTF_8)
        val commands = KagemushaNfcProtocol.writePayloadCommands(
            KagemushaPeerPayloadKind.RECEIVE_REQUEST,
            text,
            KagemushaNfcProtocol.SAFE_CHUNK_BYTES,
        )
        assertEquals(
            "8020000025010000003d67c2b6e61aef6d1f5e6b10692f58e4c864e988e98d164a43139a8e5b343e77bc",
            commands[0].toHex(),
        )
        assertEquals(
            "802100003d504b4b32522e546c4a554d41414132375a59586935317144573837526b414f7174367a5141424141414141414141414e36424d4e305f5a363631416c45",
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
        text.fill(0)
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

    private fun archive(schema: String): ByteArray {
        val payload = byteArrayOf(0x51)
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
