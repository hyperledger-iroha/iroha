package org.hyperledger.iroha.sdk.offline

import java.io.ByteArrayOutputStream
import java.net.URI
import java.nio.file.Files
import java.nio.file.Paths
import java.nio.charset.StandardCharsets
import java.security.KeyPairGenerator
import java.security.SecureRandom
import java.util.Base64
import java.util.Locale
import java.util.UUID
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import java.util.concurrent.CountDownLatch
import java.util.concurrent.ExecutionException
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.function.LongSupplier
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.bouncycastle.crypto.generators.Ed25519KeyPairGenerator
import org.bouncycastle.crypto.params.Ed25519KeyGenerationParameters
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import org.hyperledger.iroha.sdk.client.IrohaClient
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.client.ToriiCanonicalRequestAuth
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

class OfflineNoteTest {
    @Test
    fun certificateSigningBytesMatchRustVector() {
        val fixture = loadFixture()
        val sender = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"))
        val certificates = obj(obj(fixture, "chain_vectors"), "certificates")
        val verifier = certificateVerifier(fixture)

        assertEquals(string(certificates, "sender_payload_base64"), base64(sender.signingBytes()))
        assertEquals(string(certificates, "sender_payload_hash"), hex(sender.payloadHash()))
        assertTrue(verifier.verifyIssuerCertificate(sender))
        assertFalse(verifier.verifyOwnerCertificate(sender))

        val tamperedSignature = sender.issuerSignature()
        tamperedSignature[0] = (tamperedSignature[0].toInt() xor 0x01).toByte()
        val tampered = OfflineNote.KeyCertificate(
            version = sender.version,
            platform = sender.platform,
            keyId = sender.keyId,
            deviceId = sender.deviceId,
            accountId = sender.accountId,
            publicKey = sender.publicKey(),
            assertionScheme = sender.assertionScheme,
            assertionKeyAlgorithm = sender.assertionKeyAlgorithm,
            assertionPublicKey = sender.assertionPublicKey(),
            assertionUsageCountLimit = sender.assertionUsageCountLimit,
            oneUse = sender.oneUse,
            issuerSignature = tamperedSignature,
        )
        assertFalse(verifier.verifyIssuerCertificate(tampered))
        assertFalse(RejectingOfflineNoteCertificateVerifier().verifyIssuerCertificate(sender))
        assertFalse(RejectingOfflineNoteCertificateVerifier().verifyOwnerCertificate(sender))
        assertFalse(
            Ed25519OfflineNoteCertificateVerifier(listOf(ByteArray(32) { 0x42.toByte() }))
                .verifyIssuerCertificate(sender)
        )

        val ownerSigner = TestOwnerCertificateSigner()
        val ownerCertificate = ownerSigner.freshOwnerCertificate(ownerSigner.accountId)
        assertEquals(ownerSigner.accountId, ownerCertificate.accountId)
        assertTrue(verifier.verifyOwnerCertificate(ownerCertificate))
        assertFalse(verifier.verifyIssuerCertificate(ownerCertificate))
        val secondOwnerCertificate = ownerSigner.freshOwnerCertificate(ownerSigner.accountId)
        assertFalse(
            ownerCertificate.publicKey().contentEquals(secondOwnerCertificate.publicKey())
        )
    }

    @Test
    fun keyCertificatesRequireOneUseHardwareLimitWhenPresent() {
        val fixture = loadFixture()
        val sender = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"))

        assertFailsWith<IllegalArgumentException> {
            OfflineNote.KeyCertificatePayload(
                version = sender.version,
                platform = sender.platform,
                keyId = sender.keyId,
                deviceId = sender.deviceId,
                accountId = sender.accountId,
                publicKey = sender.publicKey(),
                assertionScheme = sender.assertionScheme,
                assertionKeyAlgorithm = sender.assertionKeyAlgorithm,
                assertionPublicKey = sender.assertionPublicKey(),
                assertionUsageCountLimit = 2,
                oneUse = true,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNote.KeyCertificate(
                version = sender.version,
                platform = sender.platform,
                keyId = sender.keyId,
                deviceId = sender.deviceId,
                accountId = sender.accountId,
                publicKey = sender.publicKey(),
                assertionScheme = sender.assertionScheme,
                assertionKeyAlgorithm = sender.assertionKeyAlgorithm,
                assertionPublicKey = sender.assertionPublicKey(),
                assertionUsageCountLimit = 0,
                oneUse = true,
                issuerSignature = sender.issuerSignature(),
            )
        }
        OfflineNote.KeyCertificate(
            version = sender.version,
            platform = sender.platform,
            keyId = sender.keyId,
            deviceId = sender.deviceId,
            accountId = sender.accountId,
            publicKey = sender.publicKey(),
            assertionScheme = sender.assertionScheme,
            assertionKeyAlgorithm = sender.assertionKeyAlgorithm,
            assertionPublicKey = sender.assertionPublicKey(),
            assertionUsageCountLimit = 1,
            oneUse = true,
            issuerSignature = sender.issuerSignature(),
        )
    }

    @Test
    fun offlineNoteModelsMatchRustNoritoVectors() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")

        assertEquals(string(obj(chain, "issue"), "norito_base64"), base64(issue(fixture).noritoEncoded()))
        assertEquals(string(obj(chain, "audit"), "norito_base64"), base64(audit(fixture).noritoEncoded()))
        assertEquals(string(obj(chain, "redeem"), "norito_base64"), base64(redeem(fixture).noritoEncoded()))
    }

    @Test
    fun kagemushaRecordBackedNativeProverValidatesInput() {
        val emptyPayloadArchive = kagemushaNoritoFrame(0x4b)
        val oversizedArchive = ByteArray(KagemushaCompactPaymentTokenProver.NATIVE_ARCHIVE_MAX_BYTES + 1)
        assertIllegalArgumentContains("recordBundleArchive must not be empty") {
            KagemushaCompactPaymentTokenProver.proveVerifiedCompactPaymentTokenWithRecords(ByteArray(0))
        }
        assertIllegalArgumentContains("recordBundleArchive must not exceed") {
            KagemushaCompactPaymentTokenProver.proveVerifiedCompactPaymentTokenWithRecords(oversizedArchive)
        }
        assertIllegalArgumentContains("recordBundleArchive must be a valid Norito archive") {
            KagemushaCompactPaymentTokenProver
                .proveVerifiedCompactPaymentTokenWithRecords(byteArrayOf(0x01, 0x02))
        }
        assertIllegalArgumentContains("recordBundleArchive must contain a non-empty Norito payload") {
            KagemushaCompactPaymentTokenProver.proveVerifiedCompactPaymentTokenWithRecords(emptyPayloadArchive)
        }
    }

    @Test
    fun kagemushaRecursiveAggregationNativeProverValidatesInput() {
        val validArchive = kagemushaNoritoFrameWithPayload(0x4b)
        val emptyPayloadArchive = kagemushaNoritoFrame(0x4b)
        val oversizedArchive = ByteArray(KagemushaCompactPaymentTokenProver.NATIVE_ARCHIVE_MAX_BYTES + 1)
        assertIllegalArgumentContains("recordBundleArchive must not be empty") {
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    ByteArray(0),
                    validArchive,
                )
        }
        assertIllegalArgumentContains("pallasOpenEnvelopesArchive must not be empty") {
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validArchive,
                    ByteArray(0),
                )
        }
        assertIllegalArgumentContains("recordBundleArchive must not exceed") {
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    oversizedArchive,
                    validArchive,
                )
        }
        assertIllegalArgumentContains("pallasOpenEnvelopesArchive must not exceed") {
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validArchive,
                    oversizedArchive,
                )
        }
        assertIllegalArgumentContains("recordBundleArchive must be a valid Norito archive") {
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    byteArrayOf(0x01, 0x02),
                    validArchive,
                )
        }
        assertIllegalArgumentContains("pallasOpenEnvelopesArchive must be a valid Norito archive") {
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validArchive,
                    byteArrayOf(0x03, 0x04),
                )
        }
        assertIllegalArgumentContains("recordBundleArchive must contain a non-empty Norito payload") {
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    emptyPayloadArchive,
                    validArchive,
                )
        }
        assertIllegalArgumentContains("pallasOpenEnvelopesArchive must contain a non-empty Norito payload") {
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validArchive,
                    emptyPayloadArchive,
                )
        }
    }

    @Test
    fun kagemushaCompactNativeInputCopiesBeforeDispatch() {
        val archive = kagemushaNoritoFrameWithPayload(0x4b)
        val expected = archive.copyOf()
        val ownedArchive = KagemushaCompactPaymentTokenProver.ownedNativeInput(
            archive,
            "recordBundleArchive",
        )
        archive[6] = 0x7f.toByte()
        assertFalse(ownedArchive === archive)
        assertContentEquals(expected, ownedArchive)
    }

    @Test
    fun kagemushaRecordBackedNativeProversRejectJavaNullsWithStableFieldMarkers() {
        val validArchive = kagemushaNoritoFrameWithPayload(0x4b)
        assertIllegalArgumentContains("recordBundleArchive must not be empty") {
            KagemushaCompactPaymentTokenProver.proveVerifiedCompactPaymentTokenWithRecords(null)
        }
        assertIllegalArgumentContains("recordBundleArchive must not be empty") {
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    null,
                    validArchive,
                )
        }
        assertIllegalArgumentContains("pallasOpenEnvelopesArchive must not be empty") {
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validArchive,
                    null,
                )
        }
    }

    @Test
    fun verifyingKeyBoxCanonicalEncodingMatchesStandaloneCodec() {
        val sourceBytes = byteArrayOf(1, 2, 3)
        val verifyingKey = OfflineNote.VerifyingKeyBox("halo2/ipa", sourceBytes)
        sourceBytes[0] = 0x7f.toByte()

        assertEquals("halo2/ipa", verifyingKey.backend)
        assertContentEquals(byteArrayOf(1, 2, 3), verifyingKey.bytes())

        val returnedBytes = verifyingKey.bytes()
        returnedBytes[1] = 0x7e.toByte()
        assertContentEquals(byteArrayOf(1, 2, 3), verifyingKey.bytes())

        val expected = VerifyingKeyBoxCodec.encodeNorito("halo2/ipa", byteArrayOf(1, 2, 3))
        assertContentEquals(expected, OfflineNote.encodeVerifyingKeyBox(verifyingKey))
        assertContentEquals(expected, verifyingKey.noritoEncoded())

        assertIllegalArgumentContains("verifying key backend must not be empty") {
            OfflineNote.VerifyingKeyBox(" ", byteArrayOf(1))
        }
        assertIllegalArgumentContains("verifying key backend must not contain surrounding whitespace") {
            OfflineNote.VerifyingKeyBox(" halo2/ipa ", byteArrayOf(1))
        }
        assertIllegalArgumentContains("verifying key bytes must not be empty") {
            OfflineNote.VerifyingKeyBox("halo2/ipa", ByteArray(0))
        }
    }

    @Test
    fun verifyingKeyBoxStandaloneCodecDecodesAndRejectsMalformedArchives() {
        val encoded = VerifyingKeyBoxCodec.encodeNorito("halo2/ipa", byteArrayOf(1, 2, 3))
        val decoded = VerifyingKeyBoxCodec.decodeNorito(encoded)

        assertEquals("halo2/ipa", decoded.backend)
        assertContentEquals(byteArrayOf(1, 2, 3), decoded.bytes())
        val decodedBytes = decoded.bytes()
        decodedBytes[0] = 0x7f.toByte()
        assertContentEquals(byteArrayOf(1, 2, 3), decoded.bytes())
        assertContentEquals(encoded, VerifyingKeyBoxCodec.encodeNorito(decoded.backend, decoded.bytes()))

        assertIllegalArgumentContains("backend must not contain surrounding whitespace") {
            VerifyingKeyBoxCodec.decodeNorito(rawVerifyingKeyBoxNorito(" halo2/ipa ", byteArrayOf(1)))
        }
        assertIllegalArgumentContains("bytes must not be empty") {
            VerifyingKeyBoxCodec.decodeNorito(rawVerifyingKeyBoxNorito("halo2/ipa", ByteArray(0)))
        }
        assertIllegalArgumentContains("Trailing bytes after VerifyingKeyBox field decode") {
            VerifyingKeyBoxCodec.decodeNorito(
                rawVerifyingKeyBoxNoritoFields(
                    encodeString("halo2/ipa", compact = true) + byteArrayOf(0),
                    encodeBytesVec(byteArrayOf(1)),
                ),
            )
        }
    }

    @Test
    fun kagemushaRecursiveSpendNativeProverValidatesInput() {
        assertEquals(
            "recursive_spend_v1",
            KagemushaRecursiveSpendProver.preferredMode(true).wireName,
        )
        assertEquals(
            "checked_prefold_v1",
            KagemushaRecursiveSpendProver.preferredMode(false).wireName,
        )
        assertEquals(6, KagemushaRecursiveSpendProver.REQUIRED_NATIVE_BRIDGE_ABI_VERSION)
        assertEquals(
            "kagemusha-recursive-aggregation-v1",
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        )
        assertTrue(VerifyingKeyBoxCodec.encodeNorito("halo2/ipa", byteArrayOf(1, 2, 3)).isNotEmpty())
        assertFailsWith<IllegalArgumentException> {
            VerifyingKeyBoxCodec.encodeNorito(" ", byteArrayOf(1))
        }
        assertFailsWith<IllegalArgumentException> {
            VerifyingKeyBoxCodec.encodeNorito("halo2/ipa", ByteArray(0))
        }
        assertEquals(
            "kagemusha-recursive-spend-lineage-v1",
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        )
        assertTrue(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = {},
                nativeBridgeAbiVersionProbe = { 6 },
                probeSymbol = {
                    KagemushaRecursiveSpendProver.expectIllegalArgumentProbe {
                        throw IllegalArgumentException("empty archive probe")
                    }
                },
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = {},
                nativeBridgeAbiVersionProbe = { 6 },
                probeSymbol = { false },
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = {},
                nativeBridgeAbiVersionProbe = { 5 },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = {},
                nativeBridgeAbiVersionProbe = { throw IllegalArgumentException("broken ABI probe") },
                probeSymbol = { error("probe must not run") },
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = { throw UnsatisfiedLinkError("missing library") },
                nativeBridgeAbiVersionProbe = { 6 },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = {},
                nativeBridgeAbiVersionProbe = { 6 },
                probeSymbol = { throw UnsatisfiedLinkError("missing recursive spend symbol") },
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = {},
                nativeBridgeAbiVersionProbe = { 6 },
                probeSymbol = { throw SecurityException("native bridge denied") },
            ),
        )

        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.initSpend(ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.appendSpend(ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                ByteArray(0),
                byteArrayOf(0x01),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                byteArrayOf(0x01),
                ByteArray(0),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                ByteArray(0),
                byteArrayOf(0x01),
                byteArrayOf(0x02),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                byteArrayOf(0x01),
                ByteArray(0),
                byteArrayOf(0x02),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                byteArrayOf(0x01),
                byteArrayOf(0x02),
                ByteArray(0),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.verifySpend(ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.redeemSpend(ByteArray(0))
        }
        if (KagemushaRecursiveSpendProver.isNativeAvailable()) {
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.initSpend(byteArrayOf(0x01, 0x02))
            }
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.appendSpend(byteArrayOf(0x01, 0x02))
            }
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                    byteArrayOf(0x01, 0x02),
                    byteArrayOf(0x03, 0x04),
                )
            }
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                    byteArrayOf(0x01, 0x02),
                    byteArrayOf(0x03, 0x04),
                    byteArrayOf(0x05, 0x06),
                )
            }
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.verifySpend(byteArrayOf(0x01, 0x02))
            }
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.redeemSpend(byteArrayOf(0x01, 0x02))
            }
        }
    }

    @Test
    fun chainVkOfflineNoteProofWrappersValidateInputs() {
        assertFailsWith<IllegalArgumentException> {
            ChainVkOfflineNoteProofProvider(ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            ChainVkOfflineNoteProofVerifier(ByteArray(0))
        }
        ChainVkOfflineNoteProofProvider(byteArrayOf(0x01))
        ChainVkOfflineNoteProofVerifier(byteArrayOf(0x01))

        assertTrue(
            NativeOfflineNoteProver.detectNativeAvailability(
                loadLibrary = {},
                probeSymbol = { throw IllegalArgumentException("empty native probe") },
            ),
        )
        assertFalse(
            NativeOfflineNoteProver.detectNativeAvailability(
                loadLibrary = { throw UnsatisfiedLinkError("missing library") },
                probeSymbol = {},
            ),
        )
        assertFalse(
            NativeOfflineNoteProver.detectNativeAvailability(
                loadLibrary = {},
                probeSymbol = { throw UnsatisfiedLinkError("missing symbol") },
            ),
        )
        assertFalse(
            NativeOfflineNoteProver.detectNativeAvailability(
                loadLibrary = {},
                probeSymbol = { throw SecurityException("native bridge denied") },
            ),
        )

        assertFailsWith<IllegalArgumentException> {
            NativeOfflineNoteProver.proveRedeem(ByteArray(0), byteArrayOf(0x01))
        }
        assertFailsWith<IllegalArgumentException> {
            NativeOfflineNoteProver.proveRedeem(byteArrayOf(0x01), ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            NativeOfflineNoteProver.proveAudit(ByteArray(0), byteArrayOf(0x01))
        }
        assertFailsWith<IllegalArgumentException> {
            NativeOfflineNoteProver.proveAudit(byteArrayOf(0x01), ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            NativeOfflineNoteProver.verifyRedeem(ByteArray(0), byteArrayOf(0x01))
        }
        assertFailsWith<IllegalArgumentException> {
            NativeOfflineNoteProver.verifyRedeem(byteArrayOf(0x01), ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            NativeOfflineNoteProver.verifyAudit(ByteArray(0), byteArrayOf(0x01))
        }
        assertFailsWith<IllegalArgumentException> {
            NativeOfflineNoteProver.verifyAudit(byteArrayOf(0x01), ByteArray(0))
        }
    }

    @Test
    fun kagemushaNativeProversRejectMissingAndEmptyNativeOutputs() {
        val missing = assertFailsWith<IllegalStateException> {
            KagemushaCompactPaymentTokenProver.requireNativeOutput(null, "native test")
        }
        assertTrue(missing.message!!.contains("returned no output"))

        val empty = assertFailsWith<IllegalStateException> {
            KagemushaCompactPaymentTokenProver.requireNativeOutput(ByteArray(0), "native test")
        }
        assertTrue(empty.message!!.contains("returned empty output"))

        val oversized = assertFailsWith<IllegalStateException> {
            KagemushaCompactPaymentTokenProver.requireNativeOutput(
                ByteArray(KagemushaCompactPaymentTokenProver.NATIVE_ARCHIVE_MAX_BYTES + 1),
                "native test",
            )
        }
        assertTrue(oversized.message!!.contains("returned oversized output"))

        val invalid = assertFailsWith<IllegalStateException> {
            KagemushaCompactPaymentTokenProver.requireNativeOutput(byteArrayOf(0x01, 0x02), "native test")
        }
        assertTrue(invalid.message!!.contains("returned invalid Norito archive"))

        val emptyPayload = assertFailsWith<IllegalStateException> {
            KagemushaCompactPaymentTokenProver.requireNativeOutput(kagemushaNoritoFrame(0x4b), "native test")
        }
        assertTrue(emptyPayload.message!!.contains("returned empty Norito payload"))

        val validOutput = issue(loadFixture()).noritoEncoded()
        assertContentEquals(
            validOutput,
            KagemushaCompactPaymentTokenProver.requireNativeOutput(validOutput, "native test"),
        )
    }

    @Test
    fun kagemushaNativeAvailabilityRequiresJniEntrypoint() {
        assertTrue(
            KagemushaCompactPaymentTokenProver.detectNativeAvailability(
                loadLibrary = {},
                probeSymbol = {
                    KagemushaCompactPaymentTokenProver.expectIllegalArgumentProbe {
                        throw IllegalArgumentException("invalid archive")
                    }
                },
            )
        )
        assertFalse(
            KagemushaCompactPaymentTokenProver.detectNativeAvailability(
                loadLibrary = {},
                probeSymbol = { false },
            )
        )
        assertFalse(
            KagemushaCompactPaymentTokenProver.detectNativeAvailability(
                loadLibrary = {},
                probeSymbol = { throw UnsatisfiedLinkError("missing symbol") },
            )
        )
        assertFalse(
            KagemushaCompactPaymentTokenProver.detectNativeAvailability(
                loadLibrary = { throw UnsatisfiedLinkError("missing library") },
                probeSymbol = { error("probe must not run") },
            )
        )
        assertFalse(
            KagemushaCompactPaymentTokenProver.detectNativeAvailability(
                loadLibrary = { throw IllegalArgumentException("bad library name") },
                probeSymbol = { error("probe must not run") },
            )
        )
        assertFalse(
            KagemushaCompactPaymentTokenProver.detectNativeAvailability(
                loadLibrary = { throw SecurityException("denied") },
                probeSymbol = { true },
            )
        )
    }

    @Test
    fun kagemushaRecursiveAggregationNativeAvailabilityRequiresJniEntrypoint() {
        assertTrue(
            KagemushaRecursiveAggregationProofBundleProver.detectNativeAvailability(
                loadLibrary = {},
                probeSymbol = {
                    KagemushaRecursiveAggregationProofBundleProver.expectIllegalArgumentProbe {
                        throw IllegalArgumentException("invalid archive")
                    }
                },
            )
        )
        assertFalse(
            KagemushaRecursiveAggregationProofBundleProver.detectNativeAvailability(
                loadLibrary = {},
                probeSymbol = { false },
            )
        )
        assertFalse(
            KagemushaRecursiveAggregationProofBundleProver.detectNativeAvailability(
                loadLibrary = {},
                probeSymbol = { throw UnsatisfiedLinkError("missing symbol") },
            )
        )
        assertFalse(
            KagemushaRecursiveAggregationProofBundleProver.detectNativeAvailability(
                loadLibrary = { throw UnsatisfiedLinkError("missing library") },
                probeSymbol = { error("probe must not run") },
            )
        )
        assertFalse(
            KagemushaRecursiveAggregationProofBundleProver.detectNativeAvailability(
                loadLibrary = { throw IllegalArgumentException("bad library name") },
                probeSymbol = { error("probe must not run") },
            )
        )
        assertFalse(
            KagemushaRecursiveAggregationProofBundleProver.detectNativeAvailability(
                loadLibrary = { throw SecurityException("denied") },
                probeSymbol = { true },
            )
        )
    }

    @Test
    fun publicNoritoDecodersRoundTripFixturePayloads() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val certificates = obj(chain, "certificates")
        val derivation = obj(chain, "derivation")
        val issueVector = obj(chain, "issue")
        val redeemVector = obj(chain, "redeem")
        val payment = obj(fixture, "payment_token")
        val senderCertificate = certificate(obj(payment, "sender_key_certificate"))
        val senderPayloadBytes = base64Bytes(string(certificates, "sender_payload_base64"))
        val issueBytes = base64Bytes(string(issueVector, "norito_base64"))
        val auditBytes = base64Bytes(string(obj(chain, "audit"), "norito_base64"))
        val redeemBytes = base64Bytes(string(redeemVector, "norito_base64"))

        assertEquals(
            base64(senderPayloadBytes),
            base64(OfflineNote.decodeCertificatePayload(senderPayloadBytes).noritoEncoded()),
        )
        assertEquals(
            base64(senderCertificate.noritoEncoded()),
            base64(OfflineNote.decodeCertificate(senderCertificate.noritoEncoded()).noritoEncoded()),
        )
        assertEquals(base64(issueBytes), base64(OfflineNote.decodeIssue(issueBytes).noritoEncoded()))

        val decodedAudit = OfflineNote.decodeAudit(auditBytes)
        assertEquals(base64(auditBytes), base64(decodedAudit.noritoEncoded()))
        assertEquals(
            base64(decodedAudit.inputClaims.first().noritoEncoded()),
            base64(OfflineNote.decodeIssuedClaim(decodedAudit.inputClaims.first().noritoEncoded()).noritoEncoded()),
        )
        assertEquals(
            base64(decodedAudit.publicInputs().noritoEncoded()),
            base64(OfflineNote.decodeAuditPublicInputs(decodedAudit.publicInputs().noritoEncoded()).noritoEncoded()),
        )

        val decodedRedeem = OfflineNote.decodeRedeem(redeemBytes)
        assertEquals(base64(redeemBytes), base64(decodedRedeem.noritoEncoded()))
        assertEquals(
            base64(decodedRedeem.publicInputs().noritoEncoded()),
            base64(OfflineNote.decodeRedeemPublicInputs(decodedRedeem.publicInputs().noritoEncoded()).noritoEncoded()),
        )

        val commitmentPreimage = OfflineNote.NoteCommitmentPreimage(
            chainId = string(derivation, "chain_id"),
            ownerKeyCertificatePayloadHash = hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
            assetId = string(issueVector, "asset_id"),
            amount = string(redeemVector, "amount"),
            noteSecret = hexBytes(string(derivation, "source_note_secret_hex")),
            origin = OfflineNote.CommitmentOrigin.IssuerLoad(
                operationId = string(derivation, "issuer_load_operation_id"),
                lineageId = string(derivation, "issuer_load_lineage_id"),
                localRevision = long(derivation, "issuer_load_local_revision"),
            ),
        )
        assertEquals(
            base64(commitmentPreimage.noritoEncoded()),
            base64(OfflineNote.decodeNoteCommitmentPreimage(commitmentPreimage.noritoEncoded()).noritoEncoded()),
        )

        val nullifierPreimage = OfflineNote.InputNullifierPreimage(
            chainId = string(derivation, "chain_id"),
            sourceNoteCommitment = hexBytes(string(derivation, "source_note_commitment")),
            ownerKeyCertificatePayloadHash = hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
            noteSecret = hexBytes(string(derivation, "source_note_secret_hex")),
        )
        assertEquals(
            base64(nullifierPreimage.noritoEncoded()),
            base64(OfflineNote.decodeInputNullifierPreimage(nullifierPreimage.noritoEncoded()).noritoEncoded()),
        )

        val tokenPreimage = OfflineNote.PaymentTokenIdPreimage(
            chainId = string(derivation, "chain_id"),
            paymentRequestId = string(derivation, "payment_request_id"),
            createdAtMs = long(payment, "created_at_ms"),
            tokenNonce = hexBytes(string(derivation, "token_nonce_hex")),
            senderKeyCertificatePayloadHash = hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
            inputNullifiers = listOf(hexBytes(string(derivation, "input_nullifier"))),
            outputCommitments = listOf(
                hexBytes(string(derivation, "recipient_output_commitment")),
                hexBytes(string(derivation, "change_output_commitment")),
            ),
        )
        assertEquals(
            base64(tokenPreimage.noritoEncoded()),
            base64(OfflineNote.decodePaymentTokenIdPreimage(tokenPreimage.noritoEncoded()).noritoEncoded()),
        )
    }

    @Test
    fun publicNoritoInstructionDecodersReadExplorerEnvelopeBytes() {
        val fixture = loadFixture()
        val issue = issue(fixture)
        val audit = audit(fixture)
        val redeem = redeem(fixture)

        assertEquals(
            base64(issue.noritoEncoded()),
            base64(OfflineNote.decodeIssueInstruction(rawInstructionPair(
                OfflineNote.ISSUE_INSTRUCTION_SCHEMA,
                wirePayloadBytes(OfflineNote.issueInstruction(issue)),
            )).noritoEncoded()),
        )
        assertEquals(
            base64(audit.noritoEncoded()),
            base64(OfflineNote.decodeAuditInstruction(rawInstructionPair(
                OfflineNote.AUDIT_INSTRUCTION_SCHEMA,
                wirePayloadBytes(OfflineNote.auditInstruction(audit)),
            )).noritoEncoded()),
        )
        assertEquals(
            base64(redeem.noritoEncoded()),
            base64(OfflineNote.decodeRedeemInstruction(rawInstructionPair(
                OfflineNote.REDEEM_INSTRUCTION_SCHEMA,
                wirePayloadBytes(OfflineNote.redeemInstruction(redeem)),
            )).noritoEncoded()),
        )
    }

    @Test
    fun walletDerivationsMatchRustVectors() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val issueVector = obj(chain, "issue")
        val payment = obj(fixture, "payment_token")
        val outputClaims = list(payment, "output_claims").map { it as Map<String, Any?> }
        val recipientOutput = outputClaims[0]
        val changeOutput = outputClaims[1]
        val chainId = string(derivation, "chain_id")

        val sourcePreimage = OfflineNote.NoteCommitmentPreimage(
            chainId = chainId,
            ownerKeyCertificatePayloadHash = hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
            assetId = string(issueVector, "asset_id"),
            amount = string(issueVector, "amount"),
            noteSecret = hexBytes(string(derivation, "source_note_secret_hex")),
            origin = OfflineNote.CommitmentOrigin.IssuerLoad(
                operationId = string(derivation, "issuer_load_operation_id"),
                lineageId = string(derivation, "issuer_load_lineage_id"),
                localRevision = long(derivation, "issuer_load_local_revision"),
            ),
        )
        assertEquals(
            string(derivation, "source_note_commitment_preimage_hex"),
            hex(OfflineNote.encodeNoteCommitmentPreimage(sourcePreimage)),
        )
        val sourceCommitment = OfflineNote.deriveNoteCommitment(sourcePreimage)
        assertEquals(string(derivation, "source_note_commitment"), hex(sourceCommitment))

        val inputNullifier = OfflineNote.deriveInputNullifier(
            OfflineNote.InputNullifierPreimage(
                chainId = chainId,
                sourceNoteCommitment = sourceCommitment,
                ownerKeyCertificatePayloadHash = hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
                noteSecret = hexBytes(string(derivation, "source_note_secret_hex")),
            )
        )
        assertEquals(string(derivation, "input_nullifier"), hex(inputNullifier))

        val recipientCommitment = OfflineNote.deriveNoteCommitment(
            OfflineNote.NoteCommitmentPreimage(
                chainId = chainId,
                ownerKeyCertificatePayloadHash = hexBytes(string(derivation, "recipient_key_certificate_payload_hash")),
                assetId = "${string(recipientOutput, "asset_definition_id")}#${string(recipientOutput, "account_id")}",
                amount = string(recipientOutput, "amount"),
                noteSecret = hexBytes(string(derivation, "recipient_note_secret_hex")),
                origin = OfflineNote.CommitmentOrigin.P2pOutput(
                    paymentRequestId = string(derivation, "payment_request_id"),
                    outputIndex = 0,
                ),
            )
        )
        assertEquals(string(derivation, "recipient_output_commitment"), hex(recipientCommitment))

        val changeCommitment = OfflineNote.deriveNoteCommitment(
            OfflineNote.NoteCommitmentPreimage(
                chainId = chainId,
                ownerKeyCertificatePayloadHash = hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
                assetId = "${string(changeOutput, "asset_definition_id")}#${string(changeOutput, "account_id")}",
                amount = string(changeOutput, "amount"),
                noteSecret = hexBytes(string(derivation, "change_note_secret_hex")),
                origin = OfflineNote.CommitmentOrigin.P2pOutput(
                    paymentRequestId = string(derivation, "payment_request_id"),
                    outputIndex = 1,
                ),
            )
        )
        assertEquals(string(derivation, "change_output_commitment"), hex(changeCommitment))

        val tokenId = OfflineNote.derivePaymentTokenId(
            OfflineNote.PaymentTokenIdPreimage(
                chainId = chainId,
                paymentRequestId = string(derivation, "payment_request_id"),
                createdAtMs = long(payment, "created_at_ms"),
                tokenNonce = hexBytes(string(derivation, "token_nonce_hex")),
                senderKeyCertificatePayloadHash = hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
                inputNullifiers = listOf(inputNullifier),
                outputCommitments = listOf(recipientCommitment, changeCommitment),
            )
        )
        assertEquals(string(derivation, "payment_token_id"), hex(tokenId))

        val redeemNullifier = OfflineNote.deriveInputNullifier(
            OfflineNote.InputNullifierPreimage(
                chainId = chainId,
                sourceNoteCommitment = recipientCommitment,
                ownerKeyCertificatePayloadHash = hexBytes(string(derivation, "recipient_key_certificate_payload_hash")),
                noteSecret = hexBytes(string(derivation, "recipient_note_secret_hex")),
            )
        )
        assertEquals(string(derivation, "redeem_nullifier"), hex(redeemNullifier))
    }

    @Test
    fun paymentTokensExposeOutputCommitmentMatchingForRecipientAndChange() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val payment = obj(fixture, "payment_token")
        val outputClaims = list(payment, "output_claims").map { it as Map<String, Any?> }
        val changeOutput = outputClaims[1]
        val token = OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64"))
        )

        assertTrue(token.containsOutputNoteCommitmentHex(string(derivation, "recipient_output_commitment")))
        assertTrue(token.containsOutputNoteCommitmentHex(
            " 0x${string(derivation, "change_output_commitment").uppercase(Locale.ROOT)} "
        ))
        assertEquals(
            string(changeOutput, "amount"),
            token.outputClaimForNoteCommitmentHex(string(derivation, "change_output_commitment"))?.amount,
        )
        assertTrue(token.audit.containsOutputNoteCommitment(
            hexBytes(string(derivation, "change_output_commitment"))
        ))
        assertFalse(token.containsOutputNoteCommitment(ByteArray(32) { 0xff.toByte() }))
        assertFalse(token.containsOutputNoteCommitmentHex("not-hex"))
    }

    @Test
    fun publicInputHashesMatchRustVectors() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val audit = audit(fixture)
        val redeem = redeem(fixture)

        assertEquals(string(obj(chain, "audit"), "public_inputs_hash"), hex(audit.publicInputsHash()))
        assertEquals(string(obj(chain, "redeem"), "public_inputs_hash"), hex(redeem.publicInputsHash()))
        audit.validateProofBinding()
        redeem.validateProofBinding()
        audit.replacingRecursiveProof(audit.recursiveProof).validateProofBinding()
        redeem.replacingRecursiveProof(redeem.recursiveProof).validateProofBinding()
    }

    @Test
    fun proofBindingRejectsMismatch() {
        val fixture = loadFixture()
        val redeem = redeem(fixture)
        val badProof = OfflineNote.RecursiveProof(
            publicInputsHash = OfflineNote.hash("wrong-public-inputs".toByteArray()),
            proof = OfflineNote.ProofBox(
                OfflineNote.RECURSIVE_BACKEND,
                "offline-vector-redeem-proof".toByteArray()
            )
        )
        val forged = OfflineNote.Redeem(
            sourceNoteCommitment = redeem.sourceNoteCommitment(),
            inputNullifiers = redeem.inputNullifiers(),
            senderKeyCertificate = redeem.senderKeyCertificate,
            recipient = redeem.recipient,
            assetId = redeem.assetId,
            amount = redeem.amount,
            recursiveProof = badProof,
        )

        assertFailsWith<IllegalArgumentException> {
            forged.validateProofBinding()
        }
    }

    @Test
    fun proofBindingRejectsRecursiveMetadataSubstitution() {
        val fixture = loadFixture()
        val audit = audit(fixture)
        val wrongVerifier = OfflineNote.RecursiveProof(
            verifierKeyId = OfflineNote.VerifyingKeyIdReference(
                "halo2/kzg",
                OfflineNote.RECURSIVE_VERIFIER_NAME,
            ),
            publicInputsHash = audit.publicInputsHash(),
            proof = audit.recursiveProof.proof,
        )
        assertFailsWith<IllegalArgumentException> {
            audit.replacingRecursiveProof(wrongVerifier).validateProofBinding()
        }

        val redeem = redeem(fixture)
        val wrongProofBackend = OfflineNote.RecursiveProof(
            publicInputsHash = redeem.publicInputsHash(),
            proof = OfflineNote.ProofBox(
                "groth16",
                redeem.recursiveProof.proof.bytes(),
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            redeem.replacingRecursiveProof(wrongProofBackend).validateProofBinding()
        }

        val draftPlaceholder = OfflineNote.RecursiveProof(
            publicInputsHash = redeem.publicInputsHash(),
            proof = OfflineNote.ProofBox(
                "offline-note/draft-placeholder",
                byteArrayOf(0),
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            redeem.replacingRecursiveProof(draftPlaceholder).validateProofBinding()
        }

        for (backend in listOf("halo2/ipa:KZG", "halo2/ipa: KZG", "halo2/ipa:Mock-Proof")) {
            val nonProductionBackend = OfflineNote.RecursiveProof(
                publicInputsHash = redeem.publicInputsHash(),
                proof = OfflineNote.ProofBox(
                    backend,
                    redeem.recursiveProof.proof.bytes(),
                ),
            )
            assertFailsWith<IllegalArgumentException>("backend $backend must not pass proof binding") {
                redeem.replacingRecursiveProof(nonProductionBackend).validateProofBinding()
            }
        }
    }

    @Test
    fun recursiveProofMetadataRejectsPaddedAndMalformedVerifierKeys() {
        assertFailsWith<IllegalArgumentException> {
            OfflineNote.VerifyingKeyIdReference(
                "  ${OfflineNote.RECURSIVE_BACKEND}  ",
                OfflineNote.RECURSIVE_VERIFIER_NAME,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNote.VerifyingKeyIdReference(
                OfflineNote.RECURSIVE_BACKEND,
                "  ${OfflineNote.RECURSIVE_VERIFIER_NAME}  ",
            )
        }
        val verifier = OfflineNote.VerifyingKeyIdReference(
            OfflineNote.RECURSIVE_BACKEND,
            OfflineNote.RECURSIVE_VERIFIER_NAME,
        )
        assertEquals(OfflineNote.RECURSIVE_BACKEND, verifier.backend)
        assertEquals(OfflineNote.RECURSIVE_VERIFIER_NAME, verifier.name)

        assertFailsWith<IllegalArgumentException> {
            OfflineNote.ProofBox(
                "  ${OfflineNote.RECURSIVE_BACKEND}  ",
                byteArrayOf(0x01),
            )
        }

        val proof = OfflineNote.ProofBox(
            OfflineNote.RECURSIVE_BACKEND,
            byteArrayOf(0x01),
        )
        assertEquals(OfflineNote.RECURSIVE_BACKEND, proof.backend)

        assertFailsWith<IllegalArgumentException> {
            OfflineNote.VerifyingKeyIdReference("halo2/ipa:KZG", OfflineNote.RECURSIVE_VERIFIER_NAME)
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNote.VerifyingKeyIdReference(OfflineNote.RECURSIVE_BACKEND, "offline:note")
        }
    }

    @Test
    fun commitmentOriginIdsRejectSurroundingWhitespace() {
        val issuerLoad = OfflineNote.CommitmentOrigin.IssuerLoad("operation-1", "lineage-1", 0)
        assertEquals("operation-1", issuerLoad.operationId)
        assertEquals("lineage-1", issuerLoad.lineageId)

        val p2pOutput = OfflineNote.CommitmentOrigin.P2pOutput("payment-1", 0)
        assertEquals("payment-1", p2pOutput.paymentRequestId)

        assertIllegalArgumentContains("operation_id must not contain surrounding whitespace") {
            OfflineNote.CommitmentOrigin.IssuerLoad(" operation-1", "lineage-1", 0)
        }
        assertIllegalArgumentContains("operation_id must not contain surrounding whitespace") {
            OfflineNote.CommitmentOrigin.IssuerLoad("operation-1\n", "lineage-1", 0)
        }
        assertIllegalArgumentContains("lineage_id must not contain surrounding whitespace") {
            OfflineNote.CommitmentOrigin.IssuerLoad("operation-1", " lineage-1", 0)
        }
        assertIllegalArgumentContains("payment_request_id must not contain surrounding whitespace") {
            OfflineNote.CommitmentOrigin.P2pOutput("payment-1 ", 0)
        }
    }

    @Test
    fun derivationPreimageIdsRejectSurroundingWhitespace() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val chainId = string(derivation, "chain_id")
        val assetId = string(obj(chain, "issue"), "asset_id")
        val paymentRequestId = string(derivation, "payment_request_id")
        val hash = ByteArray(32) { 0x11.toByte() }
        val origin = OfflineNote.CommitmentOrigin.IssuerLoad(
            string(derivation, "issuer_load_operation_id"),
            string(derivation, "issuer_load_lineage_id"),
            0,
        )

        OfflineNote.NoteCommitmentPreimage(
            chainId = chainId,
            ownerKeyCertificatePayloadHash = hash,
            assetId = assetId,
            amount = "1",
            noteSecret = hash,
            origin = origin,
        )
        OfflineNote.InputNullifierPreimage(
            chainId = chainId,
            sourceNoteCommitment = hash,
            ownerKeyCertificatePayloadHash = hash,
            noteSecret = hash,
        )
        OfflineNote.PaymentTokenIdPreimage(
            chainId = chainId,
            paymentRequestId = paymentRequestId,
            createdAtMs = 1_700_000_000_000L,
            tokenNonce = hash,
            senderKeyCertificatePayloadHash = hash,
            inputNullifiers = listOf(hash),
            outputCommitments = listOf(hash),
        )

        assertIllegalArgumentContains("chain_id must not contain surrounding whitespace") {
            OfflineNote.NoteCommitmentPreimage(
                chainId = " $chainId",
                ownerKeyCertificatePayloadHash = hash,
                assetId = assetId,
                amount = "1",
                noteSecret = hash,
                origin = origin,
            )
        }
        assertIllegalArgumentContains("chain_id must not contain surrounding whitespace") {
            OfflineNote.InputNullifierPreimage(
                chainId = "$chainId\n",
                sourceNoteCommitment = hash,
                ownerKeyCertificatePayloadHash = hash,
                noteSecret = hash,
            )
        }
        assertIllegalArgumentContains("payment_request_id must not contain surrounding whitespace") {
            OfflineNote.PaymentTokenIdPreimage(
                chainId = chainId,
                paymentRequestId = "$paymentRequestId ",
                createdAtMs = 1_700_000_000_000L,
                tokenNonce = hash,
                senderKeyCertificatePayloadHash = hash,
                inputNullifiers = listOf(hash),
                outputCommitments = listOf(hash),
            )
        }
    }

    @Test
    fun offlineNoteDomainsRejectSubstitutionAndPadding() {
        val fixture = loadFixture()
        val certificate = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"))
        val audit = audit(fixture)
        val redeem = redeem(fixture)
        val claim = audit.inputClaims.first()
        val auditPublic = audit.publicInputs()
        val redeemPublic = redeem.publicInputs()

        assertIllegalArgumentContains("unsupported key certificate payload domain") {
            OfflineNote.KeyCertificatePayload(
                domain = "${OfflineNote.KEY_CERTIFICATE_PAYLOAD_DOMAIN} ",
                version = certificate.version,
                platform = certificate.platform,
                keyId = certificate.keyId,
                deviceId = certificate.deviceId,
                accountId = certificate.accountId,
                publicKey = certificate.publicKey(),
                assertionScheme = certificate.assertionScheme,
                assertionKeyAlgorithm = certificate.assertionKeyAlgorithm,
                assertionPublicKey = certificate.assertionPublicKey(),
                assertionUsageCountLimit = certificate.assertionUsageCountLimit,
                oneUse = certificate.oneUse,
            )
        }
        assertIllegalArgumentContains("unsupported issued claim domain") {
            OfflineNote.IssuedClaim(
                domain = "${OfflineNote.ISSUED_CLAIM_DOMAIN}\n",
                noteCommitment = claim.noteCommitment(),
                keyCertificatePayloadHash = claim.keyCertificatePayloadHash(),
                assetId = claim.assetId,
                amount = claim.amount,
            )
        }
        assertIllegalArgumentContains("unsupported redeem public inputs domain") {
            OfflineNote.RedeemPublicInputs(
                domain = "forged:${OfflineNote.REDEEM_PUBLIC_INPUTS_DOMAIN}",
                sourceNoteCommitment = redeemPublic.sourceNoteCommitment(),
                inputNullifiers = redeemPublic.inputNullifiers(),
                keyCertificatePayloadHash = redeemPublic.keyCertificatePayloadHash(),
                recipient = redeemPublic.recipient,
                assetId = redeemPublic.assetId,
                amount = redeemPublic.amount,
            )
        }
        assertIllegalArgumentContains("unsupported audit public inputs domain") {
            OfflineNote.AuditPublicInputs(
                domain = " ${OfflineNote.AUDIT_PUBLIC_INPUTS_DOMAIN}",
                tokenId = auditPublic.tokenId(),
                keyCertificatePayloadHash = auditPublic.keyCertificatePayloadHash(),
                inputNullifiers = auditPublic.inputNullifiers(),
                inputClaims = auditPublic.inputClaims,
                outputCommitments = auditPublic.outputCommitments(),
                outputClaims = auditPublic.outputClaims,
            )
        }
    }

    @Test
    fun instanceValuesMatchRustVectors() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val auditValues = OfflineNote.InstanceBuilder.auditInstanceValues(audit(fixture))
        val redeemValues = OfflineNote.InstanceBuilder.redeemInstanceValues(redeem(fixture))
        val auditPublic = auditValues.publicValues()
        val redeemPublic = redeemValues.publicValues()

        assertEquals(
            string(obj(chain, "audit"), "public_inputs_hash"),
            hex(hashFromPublicValues(auditPublic)),
        )
        assertEquals(
            string(obj(chain, "redeem"), "public_inputs_hash"),
            hex(hashFromPublicValues(redeemPublic)),
        )
        assertEquals(2L, auditPublic[4])
        assertEquals(1L, auditPublic[5])
        assertEquals(2L, auditPublic[6])
        assertEquals(52L, auditPublic[7])
        assertEquals(52L, auditPublic[8])
        assertEquals(1L, redeemPublic[4])
        assertEquals(1L, redeemPublic[5])
        assertEquals(1L, redeemPublic[6])
        assertEquals(5L, redeemPublic[7])
        assertEquals(5L, redeemPublic[8])
        assertEquals(52L, auditValues.inputAmounts()[0])
        assertEquals(5L, auditValues.outputAmounts()[0])
        assertEquals(47L, auditValues.outputAmounts()[1])
        assertEquals(5L, redeemValues.inputAmounts()[0])
        assertEquals(5L, redeemValues.outputAmounts()[0])
        assertEquals(
            OfflineNote.instanceScalarBytes(auditPublic[0]).toList(),
            auditValues.publicInstanceColumns()[0].toList(),
        )
    }

    @Test
    fun auditInstanceValuesRejectUnanchoredClaimsAndHiddenOutputs() {
        val fixture = loadFixture()
        val audit = audit(fixture)

        assertFailsWith<IllegalArgumentException> {
            OfflineNote.AuditBundle(
                tokenId = audit.tokenId(),
                senderKeyCertificate = audit.senderKeyCertificate,
                inputNullifiers = audit.inputNullifiers(),
                inputClaims = audit.inputClaims,
                outputCommitments = audit.outputCommitments() + listOf(flippedHash(audit.outputCommitments()[0])),
                outputClaims = audit.outputClaims,
                recursiveProof = audit.recursiveProof,
            )
        }

        assertTrue(audit.outputCommitments().size > 1)
        assertFailsWith<IllegalArgumentException> {
            OfflineNote.AuditBundle(
                tokenId = audit.tokenId(),
                senderKeyCertificate = audit.senderKeyCertificate,
                inputNullifiers = audit.inputNullifiers(),
                inputClaims = audit.inputClaims,
                outputCommitments = audit.outputCommitments().reversed(),
                outputClaims = audit.outputClaims,
                recursiveProof = audit.recursiveProof,
            )
        }

        val forgedClaim = OfflineNote.IssuedClaim(
            domain = audit.inputClaims[0].domain,
            noteCommitment = audit.inputClaims[0].noteCommitment(),
            keyCertificatePayloadHash = flippedHash(audit.inputClaims[0].keyCertificatePayloadHash()),
            assetId = audit.inputClaims[0].assetId,
            amount = audit.inputClaims[0].amount,
        )
        val forgedAudit = OfflineNote.AuditBundle(
            tokenId = audit.tokenId(),
            senderKeyCertificate = audit.senderKeyCertificate,
            inputNullifiers = audit.inputNullifiers(),
            inputClaims = listOf(forgedClaim),
            outputCommitments = audit.outputCommitments(),
            outputClaims = audit.outputClaims,
            recursiveProof = audit.recursiveProof,
        )
        assertFailsWith<IllegalArgumentException> {
            OfflineNote.InstanceBuilder.auditInstanceValues(forgedAudit)
        }
    }

    @Test
    fun nativeHalo2ProverProducesVerifyingPayloadWhenRequested() {
        if (System.getenv("IROHA_JVM_OFFLINE_PROVER_TEST") != "1") {
            return
        }
        val fixture = loadFixture()
        val audit = audit(fixture)
        val values = OfflineNote.InstanceBuilder.auditInstanceValues(audit)
        OfflineNoteHalo2Prover.prewarm()
        val payload = OfflineNoteHalo2Prover.proveZk1Payload(values)
        System.getenv("IROHA_JVM_OFFLINE_PAYLOAD_OUT")?.let {
            Files.write(Paths.get(it), payload)
        }

        assertTrue(OfflineNoteHalo2Prover.verifyZk1Payload(payload, values.publicValues()))
        val proof = OfflineNoteHalo2Prover.proveAudit(audit)
        audit.replacingRecursiveProof(proof).validateProofBinding()
        assertTrue(proof.proof.bytes().size <= OfflineNoteHalo2Prover.MAX_ENVELOPE_BYTES)
        val envelope = OfflineNoteHalo2Prover.proveOpenVerifyEnvelope(values)
        assertTrue(envelope.isNotEmpty())
        assertTrue(OfflineNoteHalo2Prover.verifyOpenVerifyEnvelope(proof.proof.bytes(), values.publicValues()))
        assertTrue(OfflineNoteHalo2Prover.verifyOpenVerifyEnvelope(proof.proof.bytes(), hex(proof.publicInputsHash())))
        assertFalse(
            OfflineNoteHalo2Prover.verifyOpenVerifyEnvelope(
                proof.proof.bytes(),
                "00".repeat(32),
            ),
        )
    }

    @Test
    fun nativeHalo2ProverPerformanceWhenRequested() {
        if (System.getenv("IROHA_JVM_OFFLINE_BENCH") != "1") {
            return
        }
        val iterations = System.getenv("IROHA_JVM_OFFLINE_BENCH_ITERATIONS")?.toInt() ?: 20
        assertTrue(iterations > 0)
        val fixture = loadFixture()
        val audit = audit(fixture)
        val redeem = redeem(fixture)
        OfflineNoteHalo2Prover.prewarm()
        OfflineNoteHalo2Prover.proveAudit(audit)
        OfflineNoteHalo2Prover.proveRedeem(redeem)

        val auditSeconds = benchmarkSeconds(iterations) {
            OfflineNoteHalo2Prover.proveAudit(audit)
        }
        val redeemSeconds = benchmarkSeconds(iterations) {
            OfflineNoteHalo2Prover.proveRedeem(redeem)
        }
        println("offline_note_jvm_bench audit=${summary(auditSeconds)} redeem=${summary(redeemSeconds)}")
    }

    @Test
    fun qrFixtureUsesSdkTextPrefix() {
        val fountain = obj(loadFixture(), "fountain_qr")
        assertEquals("iroha:qr:", string(fountain, "frame_prefix"))
    }

    @Test
    fun qrTextCodecRejectsLegacyVersionedPrefix() {
        val payload = ByteArray(64) { index -> (index * 31 + 7).toByte() }
        val legacy = "iroha:qr-old:" + Base64.getEncoder().encodeToString(payload)

        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.TextCodec.decode(legacy, OfflineQrStream.FrameEncoding.BASE64)
        }
    }

    @Test
    fun paymentTokenCodecRoundTripsNoritoTextAndQrFrames() {
        val fixture = loadFixture()
        val derivation = obj(obj(fixture, "chain_vectors"), "derivation")
        val payment = obj(fixture, "payment_token")
        val sdkInterop = obj(fixture, "sdk_interop")
        val token = OfflineNotePaymentToken(
            chainId = string(derivation, "chain_id"),
            paymentRequestId = string(payment, "invoice_id"),
            tokenNonce = hexBytes(string(derivation, "token_nonce_hex")),
            tokenId = hexBytes(string(payment, "token_id")),
            audit = audit(fixture),
            createdAtMs = long(payment, "created_at_ms"),
        )
        val canonicalPayload = base64Bytes(string(sdkInterop, "payment_token_norito_base64"))
        assertContentEquals(canonicalPayload, OfflineNotePaymentTokenCodec.encodeNorito(token))

        val noritoDecoded = OfflineNotePaymentTokenCodec.decodeNorito(
            OfflineNotePaymentTokenCodec.encodeNorito(token)
        )
        assertEquals(token.tokenIdHex(), noritoDecoded.tokenIdHex())
        assertEquals(token.paymentRequestId, noritoDecoded.paymentRequestId)
        assertEquals(base64(token.audit.noritoEncoded()), base64(noritoDecoded.audit.noritoEncoded()))
        val canonicalDecoded = OfflineNotePaymentTokenCodec.decodeNorito(canonicalPayload)
        assertEquals(token.tokenIdHex(), canonicalDecoded.tokenIdHex())
        assertEquals(base64(token.audit.noritoEncoded()), base64(canonicalDecoded.audit.noritoEncoded()))

        val text = OfflineNotePaymentTokenCodec.encodeText(token)
        assertEquals(string(sdkInterop, "payment_token_text"), text)
        assertTrue(text.startsWith(OfflineNotePaymentTokenCodec.TEXT_PREFIX))
        assertEquals(OfflineBearerCashTextCodec.PAYMENT_TEXT_PREFIX, OfflineNotePaymentTokenCodec.TEXT_PREFIX)
        assertEquals(token.tokenIdHex(), OfflineNotePaymentTokenCodec.decodeText(text).tokenIdHex())
        assertEquals(token.tokenIdHex(), OfflineBearerCashTextCodec.decodePaymentText(text).tokenIdHex())
        assertEquals(
            token.tokenIdHex(),
            OfflineNotePaymentTokenCodec.decodeText(string(sdkInterop, "payment_token_text")).tokenIdHex(),
        )
        assertFailsWith<IllegalArgumentException> {
            OfflineNotePaymentTokenCodec.decodeText(
                "wallet-offline-bearer-cash-payment-invalid:${text.substringAfter(':')}",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNotePaymentTokenCodec.decodeText("$text=")
        }

        val frames = OfflineNotePaymentTokenCodec.encodeQrFrameBytes(
            token,
            OfflineQrStream.Options(chunkSize = 180, parityGroup = 2),
        )
        @Suppress("UNCHECKED_CAST")
        val expectedFrames = list(obj(sdkInterop, "payment_token_qr"), "frames")
            .map { string(it as Map<String, Any?>, "bytes_hex") }
        assertEquals(expectedFrames, frames.map(::hex))
        val decoder = OfflineQrStream.Decoder()
        var payload: ByteArray? = null
        for (frame in frames) {
            payload = decoder.ingest(frame).payload ?: payload
        }
        val qrDecoded = OfflineNotePaymentTokenCodec.decodeQrPayload(assertNotNull(payload))
        assertEquals(token.tokenIdHex(), qrDecoded.tokenIdHex())
        assertEquals(base64(token.audit.noritoEncoded()), base64(qrDecoded.audit.noritoEncoded()))

        val canonicalDecoder = OfflineQrStream.Decoder()
        var canonicalQrPayload: ByteArray? = null
        for (frame in expectedFrames) {
            canonicalQrPayload = canonicalDecoder.ingest(hexBytes(frame)).payload ?: canonicalQrPayload
        }
        assertContentEquals(canonicalPayload, assertNotNull(canonicalQrPayload))
        assertEquals(
            token.tokenIdHex(),
            OfflineNotePaymentTokenCodec.decodeQrPayload(assertNotNull(canonicalQrPayload)).tokenIdHex(),
        )
    }

    @Test
    fun offlineBearerCashPolicyAndPrefixesUseSingleAppSurface() {
        val policy = OfflineBearerCashPolicyV1.DEFAULT
        assertEquals(5, policy.maxCustodyHops)
        assertEquals(32, policy.maxLineageSteps)
        assertEquals(2_048, policy.maxSingleQrPayloadBytes)
        assertEquals(12_288, policy.maxStreamPayloadBytes)
        assertEquals(20, policy.androidKeyPoolTarget)
        assertEquals(8, policy.androidKeyPoolReplenishBelow)
        assertEquals(40, policy.androidKeyPoolCap)
        assertEquals(OfflineBearerCashTransport.STATIC_QR, policy.recommendedTransportForPayloadByteCount(2_048))
        assertEquals(OfflineBearerCashTransport.STREAMING_QR, policy.recommendedTransportForPayloadByteCount(2_049))
        assertEquals(
            OfflineBearerCashTransport.FRAMED_BYTE_TRANSPORT,
            policy.recommendedTransportForPayloadByteCount(12_289),
        )
        val fixture = loadFixture()
        val audit = audit(fixture)
        val ancestor = ancestorAuditProducingFirstInput(audit, 0xB0)
        val metrics = policy.auditTrailMetrics(listOf(ancestor, audit), audit)
        assertEquals(2, metrics.custodyHops)
        assertEquals(2, metrics.lineageSteps)
        assertFailsWith<IllegalArgumentException> {
            OfflineBearerCashPolicyV1(maxCustodyHops = 1).validateAuditTrail(
                listOf(ancestor, audit),
                audit,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineBearerCashPolicyV1(maxLineageSteps = 1).validateAuditTrail(
                listOf(ancestor, audit),
                audit,
            )
        }
        assertEquals("wallet-offline-bearer-cash-receive:", OfflineNoteReceiveRequestCodec.TEXT_PREFIX)
        assertEquals("wallet-offline-bearer-cash-payment:", OfflineNotePaymentTokenCodec.TEXT_PREFIX)
        assertEquals("wallet-offline-bearer-cash-ack:", OfflineNoteReceiptAckCodec.TEXT_PREFIX)
        assertNull(OfflineBearerCashTextCodec.payloadKind("wallet-offline-bearer-cash-unknown:AAAA"))
    }

    @Test
    fun receiveRequestCodecRoundTripsNoritoTextAndQrFrames() {
        val fixture = loadFixture()
        val request = receiveRequestFixture(fixture)

        val noritoDecoded = OfflineNoteReceiveRequestCodec.decodeNorito(
            OfflineNoteReceiveRequestCodec.encodeNorito(request)
        )
        assertEquals(request.paymentRequestId, noritoDecoded.paymentRequestId)
        assertEquals(request.accountId, noritoDecoded.accountId)
        assertEquals(request.assetId, noritoDecoded.assetId)
        assertEquals(request.canonicalAmount, noritoDecoded.canonicalAmount)
        assertEquals(request.outputCommitmentHex(), noritoDecoded.outputCommitmentHex())
        assertEquals(hex(request.keyCertificate.payloadHash()), hex(noritoDecoded.keyCertificate.payloadHash()))

        val text = OfflineNoteReceiveRequestCodec.encodeText(request)
        assertTrue(text.startsWith(OfflineNoteReceiveRequestCodec.TEXT_PREFIX))
        assertEquals(
            request.outputCommitmentHex(),
            OfflineNoteReceiveRequestCodec.decodeText(text).outputCommitmentHex(),
        )
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteReceiveRequestCodec.decodeText("$text=")
        }

        val frames = OfflineNoteReceiveRequestCodec.encodeQrFrameBytes(
            request,
            OfflineQrStream.Options(chunkSize = 180, parityGroup = 2),
        )
        val decoder = OfflineQrStream.Decoder()
        var payload: ByteArray? = null
        for (frame in frames) {
            val result = decoder.ingest(frame)
            assertEquals(OfflineQrStream.PayloadKind.OFFLINE_RECEIVE_REQUEST, result.payloadKind)
            payload = result.payload ?: payload
        }
        assertEquals(
            request.outputCommitmentHex(),
            OfflineNoteReceiveRequestCodec.decodeQrPayload(assertNotNull(payload)).outputCommitmentHex(),
        )
    }

    @Test
    fun receiptAckCodecRoundTripsNoritoTextAndQrFrames() {
        val fixture = loadFixture()
        val payment = obj(fixture, "payment_token")
        val token = OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")),
        )
        val ack = OfflineNoteReceiptAck.fromPaymentToken(
            token = token,
            recipientAccountId = string(payment, "recipient_account_id"),
            acceptedAtMs = long(obj(fixture, "receipt_ack"), "accepted_at_ms"),
        )

        val noritoDecoded = OfflineNoteReceiptAckCodec.decodeNorito(
            OfflineNoteReceiptAckCodec.encodeNorito(ack),
        )
        assertEquals(ack.chainId, noritoDecoded.chainId)
        assertEquals(ack.paymentRequestId, noritoDecoded.paymentRequestId)
        assertEquals(ack.tokenIdHex(), noritoDecoded.tokenIdHex())
        assertEquals(ack.recipientAccountId, noritoDecoded.recipientAccountId)
        assertTrue(noritoDecoded.matchesPaymentToken(token))

        val text = OfflineNoteReceiptAckCodec.encodeText(ack)
        assertTrue(text.startsWith(OfflineNoteReceiptAckCodec.TEXT_PREFIX))
        assertEquals(ack.tokenIdHex(), OfflineNoteReceiptAckCodec.decodeText(text).tokenIdHex())
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteReceiptAckCodec.decodeText("$text=")
        }

        val frames = OfflineNoteReceiptAckCodec.encodeQrFrameBytes(
            ack,
            OfflineQrStream.Options(chunkSize = 180, parityGroup = 2),
        )
        val decoder = OfflineQrStream.Decoder()
        var payload: ByteArray? = null
        for (frame in frames) {
            val result = decoder.ingest(frame)
            assertEquals(OfflineQrStream.PayloadKind.OFFLINE_RECEIPT_ACK, result.payloadKind)
            payload = result.payload ?: payload
        }
        assertEquals(
            ack.tokenIdHex(),
            OfflineNoteReceiptAckCodec.decodeQrPayload(assertNotNull(payload)).tokenIdHex(),
        )
    }

    @Test
    fun receiptAckCodecRejectsNonPositiveAcceptedAtDecode() {
        val fixture = loadFixture()
        val payment = obj(fixture, "payment_token")
        val token = OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")),
        )
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteReceiptAck.fromPaymentToken(
                token = token,
                recipientAccountId = string(payment, "recipient_account_id"),
                acceptedAtMs = 0L,
            )
        }

        val ack = OfflineNoteReceiptAck.fromPaymentToken(
            token = token,
            recipientAccountId = string(payment, "recipient_account_id"),
            acceptedAtMs = long(obj(fixture, "receipt_ack"), "accepted_at_ms"),
        )
        val malformed = OfflineNoteReceiptAckCodec.encodeNorito(ack).copyOf()
        malformed.fill(0, malformed.size - java.lang.Long.BYTES, malformed.size)
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteReceiptAckCodec.decodeNorito(malformed)
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteReceiptAckCodec.decodeText(
                OfflineNoteReceiptAckCodec.TEXT_PREFIX +
                    Base64.getUrlEncoder().withoutPadding().encodeToString(malformed),
            )
        }
    }

    @Test
    fun receiptAckRejectsPaddedIdentifiers() {
        val fixture = loadFixture()
        val payment = obj(fixture, "payment_token")
        val token = OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")),
        )
        val recipientAccountId = string(payment, "recipient_account_id")
        val acceptedAtMs = long(obj(fixture, "receipt_ack"), "accepted_at_ms")

        assertFailsWith<IllegalArgumentException> {
            OfflineNoteReceiptAck(
                chainId = " ${token.chainId}",
                paymentRequestId = token.paymentRequestId,
                tokenId = token.tokenId(),
                recipientAccountId = recipientAccountId,
                acceptedAtMs = acceptedAtMs,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteReceiptAck(
                chainId = token.chainId,
                paymentRequestId = "${token.paymentRequestId}\n",
                tokenId = token.tokenId(),
                recipientAccountId = recipientAccountId,
                acceptedAtMs = acceptedAtMs,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteReceiptAck(
                chainId = token.chainId,
                paymentRequestId = token.paymentRequestId,
                tokenId = token.tokenId(),
                recipientAccountId = "$recipientAccountId ",
                acceptedAtMs = acceptedAtMs,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteReceiptAck.fromPaymentToken(
                token = token,
                recipientAccountId = " $recipientAccountId",
                acceptedAtMs = acceptedAtMs,
            )
        }
    }

    private fun receiveRequestFixture(fixture: Map<String, Any?>): OfflineNoteReceiveRequest {
        val derivation = obj(obj(fixture, "chain_vectors"), "derivation")
        val payment = obj(fixture, "payment_token")
        return OfflineNoteReceiveRequest(
            chainId = string(derivation, "chain_id"),
            paymentRequestId = string(derivation, "payment_request_id"),
            accountId = string(payment, "recipient_account_id"),
            assetDefinitionId = string(payment, "asset_definition_id"),
            assetId = "${string(payment, "asset_definition_id")}#${string(payment, "recipient_account_id")}",
            amount = string(payment, "amount"),
            keyCertificate = certificate(obj(payment, "recipient_key_certificate")),
            outputCommitment = hexBytes(string(derivation, "recipient_output_commitment")),
        )
    }

    @Test
    fun qrStreamRejectsAdversarialEnvelopesAndChunkShapes() {
        val payload = ByteArray(300) { ((it * 31 + 7) and 0xff).toByte() }
        val frames = OfflineQrStream.Encoder.encodeFrames(
            payload,
            OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN,
            OfflineQrStream.Options(chunkSize = 100, parityGroup = 2),
        )
        val header = frames.first { it.kind == OfflineQrStream.FrameKind.HEADER }
        val dataFrames = frames.filter { it.kind == OfflineQrStream.FrameKind.DATA }
        val parityFrames = frames.filter { it.kind == OfflineQrStream.FrameKind.PARITY }
        val firstData = dataFrames.first()
        val firstParity = parityFrames.first()

        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Frame(
                OfflineQrStream.FrameKind.DATA,
                header.streamId(),
                0,
                1,
                ByteArray(0x1_0000),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Frame(OfflineQrStream.FrameKind.DATA, header.streamId(), -1, 1, ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Frame(OfflineQrStream.FrameKind.DATA, header.streamId(), 0x1_0000, 1, ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Frame(OfflineQrStream.FrameKind.DATA, header.streamId(), 0, -1, ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Frame(OfflineQrStream.FrameKind.DATA, header.streamId(), 0, 0x1_0000, ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Envelope(0, 0, 0, 1, 1, 0, -1, 1, ByteArray(32))
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Envelope(0, 0, 0, 1, 1, 0, 0x1_0000, 1, ByteArray(32))
        }

        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Frame.decode(header.encode() + byteArrayOf(0x00))
        }

        val unknownFrameKind = header.encode()
        unknownFrameKind[2] = 0x7f
        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Frame.decode(unknownFrameKind)
        }

        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Decoder().ingest(
                OfflineQrStream.Frame(
                    OfflineQrStream.FrameKind.HEADER,
                    ByteArray(16) { 0xa5.toByte() },
                    0,
                    1,
                    header.payload(),
                ).encode(),
            )
        }

        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Decoder().ingest(
                OfflineQrStream.Frame(
                    OfflineQrStream.FrameKind.HEADER,
                    header.streamId(),
                    1,
                    1,
                    header.payload(),
                ).encode(),
            )
        }

        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Decoder().ingest(mutatedHeaderFrame(header) { it + byteArrayOf(0x00) })
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Decoder().ingest(mutatedHeaderFrame(header) { envelope ->
                envelope.apply { this[1] = 0x7f }
            })
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Decoder().ingest(mutatedHeaderFrame(header) { envelope ->
                envelope.apply { writeUInt16LE(this, 3, 0) }
            })
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Decoder().ingest(mutatedHeaderFrame(header) { envelope ->
                envelope.apply { writeUInt16LE(this, 5, 1) }
            })
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Decoder().ingest(mutatedHeaderFrame(header) { envelope ->
                envelope.apply { writeUInt16LE(this, 7, 0) }
            })
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineQrStream.Decoder().ingest(mutatedHeaderFrame(header) { envelope ->
                envelope.apply { this[0] = 0x01 }
            })
        }

        val repeatedHeaderDecoder = OfflineQrStream.Decoder()
        repeatedHeaderDecoder.ingest(header.encode())
        repeatedHeaderDecoder.ingest(header.encode())
        assertFailsWith<IllegalArgumentException> {
            repeatedHeaderDecoder.ingest(mutatedHeaderFrame(header) { envelope ->
                envelope.apply {
                    writeUInt16LE(this, 9, OfflineQrStream.PayloadKind.OFFLINE_RECEIVE_REQUEST.value)
                }
            })
        }

        val shortDataDecoder = OfflineQrStream.Decoder()
        shortDataDecoder.ingest(header.encode())
        val shortDataPayload = firstData.payload().copyOf(firstData.payload().size - 1)
        assertFailsWith<IllegalArgumentException> {
            shortDataDecoder.ingest(
                OfflineQrStream.Frame(
                    OfflineQrStream.FrameKind.DATA,
                    firstData.streamId(),
                    firstData.index,
                    firstData.total,
                    shortDataPayload,
                ).encode(),
            )
        }

        val longDataDecoder = OfflineQrStream.Decoder()
        longDataDecoder.ingest(header.encode())
        assertFailsWith<IllegalArgumentException> {
            longDataDecoder.ingest(
                OfflineQrStream.Frame(
                    OfflineQrStream.FrameKind.DATA,
                    firstData.streamId(),
                    firstData.index,
                    firstData.total,
                    firstData.payload() + byteArrayOf(0x00),
                ).encode(),
            )
        }

        val wrongTotalDecoder = OfflineQrStream.Decoder()
        wrongTotalDecoder.ingest(header.encode())
        assertFailsWith<IllegalArgumentException> {
            wrongTotalDecoder.ingest(
                OfflineQrStream.Frame(
                    OfflineQrStream.FrameKind.DATA,
                    firstData.streamId(),
                    firstData.index,
                    firstData.total + 1,
                    firstData.payload(),
                ).encode(),
            )
        }

        val pendingBadDataDecoder = OfflineQrStream.Decoder()
        pendingBadDataDecoder.ingest(
            OfflineQrStream.Frame(
                OfflineQrStream.FrameKind.DATA,
                firstData.streamId(),
                firstData.index,
                firstData.total + 1,
                firstData.payload(),
            ).encode(),
        )
        assertFailsWith<IllegalArgumentException> {
            pendingBadDataDecoder.ingest(header.encode())
        }

        val conflictingDataDecoder = OfflineQrStream.Decoder()
        conflictingDataDecoder.ingest(header.encode())
        conflictingDataDecoder.ingest(firstData.encode())
        val conflictingDataPayload = firstData.payload()
        conflictingDataPayload[0] = (conflictingDataPayload[0].toInt() xor 0xff).toByte()
        assertFailsWith<IllegalArgumentException> {
            conflictingDataDecoder.ingest(
                OfflineQrStream.Frame(
                    OfflineQrStream.FrameKind.DATA,
                    firstData.streamId(),
                    firstData.index,
                    firstData.total,
                    conflictingDataPayload,
                ).encode(),
            )
        }

        val poisonedParityDecoder = OfflineQrStream.Decoder()
        poisonedParityDecoder.ingest(header.encode())
        poisonedParityDecoder.ingest(firstData.encode())
        val poisonedParityPayload = firstParity.payload()
        poisonedParityPayload[0] = (poisonedParityPayload[0].toInt() xor 0xff).toByte()
        poisonedParityDecoder.ingest(
            OfflineQrStream.Frame(
                OfflineQrStream.FrameKind.PARITY,
                firstParity.streamId(),
                firstParity.index,
                firstParity.total,
                poisonedParityPayload,
            ).encode(),
        )
        assertFailsWith<IllegalArgumentException> {
            poisonedParityDecoder.ingest(dataFrames[1].encode())
        }

        val hashMismatchDecoder = OfflineQrStream.Decoder()
        hashMismatchDecoder.ingest(header.encode())
        val mutatedFirstDataPayload = firstData.payload()
        mutatedFirstDataPayload[0] = (mutatedFirstDataPayload[0].toInt() xor 0xff).toByte()
        hashMismatchDecoder.ingest(
            OfflineQrStream.Frame(
                OfflineQrStream.FrameKind.DATA,
                firstData.streamId(),
                firstData.index,
                firstData.total,
                mutatedFirstDataPayload,
            ).encode(),
        )
        hashMismatchDecoder.ingest(dataFrames[1].encode())
        assertFailsWith<IllegalArgumentException> {
            hashMismatchDecoder.ingest(dataFrames[2].encode())
        }

        val shortParityDecoder = OfflineQrStream.Decoder()
        shortParityDecoder.ingest(header.encode())
        assertFailsWith<IllegalArgumentException> {
            shortParityDecoder.ingest(
                OfflineQrStream.Frame(
                    OfflineQrStream.FrameKind.PARITY,
                    firstParity.streamId(),
                    firstParity.index,
                    firstParity.total,
                    firstParity.payload().copyOf(firstParity.payload().size - 1),
                ).encode(),
            )
        }

        val conflictingParityDecoder = OfflineQrStream.Decoder()
        conflictingParityDecoder.ingest(header.encode())
        conflictingParityDecoder.ingest(firstParity.encode())
        val conflictingParityPayload = firstParity.payload()
        conflictingParityPayload[0] = (conflictingParityPayload[0].toInt() xor 0xff).toByte()
        assertFailsWith<IllegalArgumentException> {
            conflictingParityDecoder.ingest(
                OfflineQrStream.Frame(
                    OfflineQrStream.FrameKind.PARITY,
                    firstParity.streamId(),
                    firstParity.index,
                    firstParity.total,
                    conflictingParityPayload,
                ).encode(),
            )
        }
    }

    @Test
    fun transferHandoffSupportsQrNfcAndNearbyPayloads() {
        val fixture = loadFixture()
        val derivation = obj(obj(fixture, "chain_vectors"), "derivation")
        val payment = obj(fixture, "payment_token")
        val sdkInterop = obj(fixture, "sdk_interop")
        val token = OfflineNotePaymentToken(
            chainId = string(derivation, "chain_id"),
            paymentRequestId = string(payment, "invoice_id"),
            tokenNonce = hexBytes(string(derivation, "token_nonce_hex")),
            tokenId = hexBytes(string(payment, "token_id")),
            audit = audit(fixture),
            createdAtMs = long(payment, "created_at_ms"),
        )
        val receiptAck = OfflineNoteReceiptAck.fromPaymentToken(
            token = token,
            recipientAccountId = string(payment, "recipient_account_id"),
            acceptedAtMs = long(obj(fixture, "receipt_ack"), "accepted_at_ms"),
        )
        val canonicalPayload = base64Bytes(string(sdkInterop, "payment_token_norito_base64"))

        val capabilities = OfflineNoteTransferCapabilities.current()
        assertTrue(capabilities.supportedModalities().contains(OfflineNoteTransferModality.QR_STREAMING))
        assertTrue(capabilities.supportedModalities().contains(OfflineNoteTransferModality.NEARBY))
        assertFalse(capabilities.supportedModalities().contains(OfflineNoteTransferModality.NFC))

        val nearby = OfflineNoteTransferHandoff.nearbyPayload(token)
        assertEquals(OfflineNoteTransferModality.NEARBY, nearby.modality)
        assertEquals(OfflineNoteTransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE, nearby.contentType)
        assertContentEquals(canonicalPayload, nearby.payload())
        assertEquals(token.tokenIdHex(), OfflineNoteTransferHandoff.decodePaymentToken(nearby).tokenIdHex())
        val nearbyAck = OfflineNoteTransferHandoff.receiptAckPayload(
            receiptAck,
            OfflineNoteTransferModality.NEARBY,
        )
        assertEquals(receiptAck.tokenIdHex(), OfflineNoteTransferHandoff.decodeReceiptAck(nearbyAck).tokenIdHex())
        assertEquals(
            receiptAck.tokenIdHex(),
            OfflineNoteTransferHandoff.decodeNearbyReceiptAck(
                OfflineNoteTransferHandoff.nearbyReceiptAckEnvelopeBytes(receiptAck),
            ).tokenIdHex(),
        )

        @Suppress("UNCHECKED_CAST")
        val expectedFrames = list(obj(sdkInterop, "payment_token_qr"), "frames")
            .map { string(it as Map<String, Any?>, "bytes_hex") }
        val qrFrames = OfflineNoteTransferHandoff.qrStreamingFrameBytes(token)
        assertEquals(expectedFrames, qrFrames.map(::hex))
        val qrReceiver = OfflineNoteTransferStreamReceiver()
        var qrResult: OfflineNoteTransferStreamResult? = null
        for (frame in qrFrames) {
            qrResult = qrReceiver.ingestFrame(frame)
        }
        assertEquals(token.tokenIdHex(), assertNotNull(qrResult?.token).tokenIdHex())

        val nfcFrames = OfflineNoteTransferHandoff.nfcFrameBytes(token)
        assertTrue(nfcFrames.all { it.size <= 250 })
        val nfcReceiver = OfflineNoteTransferStreamReceiver()
        var nfcResult: OfflineNoteTransferStreamResult? = null
        for (frame in nfcFrames) {
            nfcResult = nfcReceiver.ingestFrame(frame)
        }
        assertEquals(token.tokenIdHex(), assertNotNull(nfcResult?.token).tokenIdHex())

        val ackFrames = OfflineNoteTransferHandoff.qrStreamingFrameBytes(receiptAck)
        val ackReceiver = OfflineNoteTransferStreamReceiver()
        var ackResult: OfflineNoteTransferStreamResult? = null
        for (frame in ackFrames) {
            ackResult = ackReceiver.ingestFrame(frame)
        }
        assertEquals(receiptAck.tokenIdHex(), assertNotNull(ackResult?.receiptAck).tokenIdHex())
    }

    @Test
    fun transferHandoffRejectsAdversarialStreamsAndMetadata() {
        val fixture = loadFixture()
        val token = OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")),
        )
        val rawPayload = OfflineNoteTransferHandoff.rawPaymentTokenBytes(token)
        val payload = OfflineNoteTransferHandoff.paymentTokenPayload(
            token,
            OfflineNoteTransferModality.QR_STREAMING,
        )
        val wrongContentType = OfflineNoteTransferPayload(
            OfflineNoteTransferModality.NEARBY,
            OfflineNoteTransferHandoff.RECEIPT_ACK_CONTENT_TYPE,
            payload.payload(),
        )
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteTransferHandoff.decodePaymentToken(wrongContentType)
        }

        val frames = OfflineNoteTransferHandoff.qrStreamingFrameBytes(
            token,
            OfflineQrStream.Options(chunkSize = 128, parityGroup = 0),
        )
        assertTrue(frames.size > 2)

        fun assertRejectedFrame(frame: ByteArray) {
            assertFailsWith<IllegalArgumentException> {
                OfflineNoteTransferStreamReceiver().ingestFrame(frame)
            }
        }

        val badMagic = frames[0].copyOf()
        badMagic[0] = 0x00
        assertRejectedFrame(badMagic)

        val badVersion = frames[0].copyOf()
        badVersion[2] = 0x7f
        assertRejectedFrame(badVersion)

        val badChecksum = frames[1].copyOf()
        badChecksum[badChecksum.lastIndex] = (badChecksum[badChecksum.lastIndex].toInt() xor 0x01).toByte()
        assertRejectedFrame(badChecksum)

        assertRejectedFrame(frames[0].copyOfRange(0, 8))

        val header = OfflineQrStream.Frame.decode(frames[0])
        val mismatchedHeaderStreamId = header.streamId()
        mismatchedHeaderStreamId[0] = (mismatchedHeaderStreamId[0].toInt() xor 0x01).toByte()
        val mismatchedHeader = OfflineQrStream.Frame(
            OfflineQrStream.FrameKind.HEADER,
            mismatchedHeaderStreamId,
            header.index,
            header.total,
            header.payload(),
        ).encode()
        assertRejectedFrame(mismatchedHeader)

        val firstData = OfflineQrStream.Frame.decode(frames[1])
        val wrongStreamId = firstData.streamId()
        wrongStreamId[0] = (wrongStreamId[0].toInt() xor 0x7f).toByte()
        val wrongStreamFrame = OfflineQrStream.Frame(
            OfflineQrStream.FrameKind.DATA,
            wrongStreamId,
            firstData.index,
            firstData.total,
            firstData.payload(),
        ).encode()
        val ignoreWrongStreamReceiver = OfflineNoteTransferStreamReceiver()
        assertFalse(ignoreWrongStreamReceiver.ingestFrame(frames[0]).isComplete)
        assertFalse(ignoreWrongStreamReceiver.ingestFrame(wrongStreamFrame).isComplete)
        var completed: OfflineNoteTransferStreamResult? = null
        frames.drop(1).forEach { completed = ignoreWrongStreamReceiver.ingestFrame(it) }
        assertEquals(token.tokenIdHex(), assertNotNull(completed?.token).tokenIdHex())

        val poisonedPayload = firstData.payload()
        poisonedPayload[0] = (poisonedPayload[0].toInt() xor 0x01).toByte()
        val poisonedFrame = OfflineQrStream.Frame(
            OfflineQrStream.FrameKind.DATA,
            firstData.streamId(),
            firstData.index,
            firstData.total,
            poisonedPayload,
        ).encode()
        val poisonedReceiver = OfflineNoteTransferStreamReceiver()
        poisonedReceiver.ingestFrame(frames[0])
        poisonedReceiver.ingestFrame(poisonedFrame)
        assertFailsWith<IllegalArgumentException> {
            frames.drop(2).forEach { poisonedReceiver.ingestFrame(it) }
        }

        val wrongKindFrames = OfflineQrStream.Encoder.encodeFrameBytes(
            rawPayload,
            OfflineQrStream.PayloadKind.OFFLINE_RECEIPT_ACK,
            OfflineQrStream.Options(chunkSize = 512, parityGroup = 0),
        )
        val wrongKindReceiver = OfflineNoteTransferStreamReceiver()
        assertFailsWith<IllegalArgumentException> {
            wrongKindFrames.forEach { wrongKindReceiver.ingestFrame(it) }
        }
    }

    @Test
    fun nfcApduProtocolSupportsAndroidSafeAndIosFastChunks() {
        val fixture = loadFixture()
        val token = OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")),
        )
        val payload = OfflineNoteTransferHandoff.rawPaymentTokenBytes(token)

        assertEquals(OfflineNoteTransferHandoff.DEFAULT_NFC_AID_HEX, OfflineNoteNfcApduProtocol.AID_HEX)
        assertEquals(OfflineNoteNfcCommand.Select, OfflineNoteNfcApduProtocol.parseCommand(OfflineNoteNfcApduProtocol.selectAidApdu()))
        assertEquals(OfflineNoteNfcCommand.GetInfo, OfflineNoteNfcApduProtocol.parseCommand(OfflineNoteNfcApduProtocol.getInfoApdu()))

        val infoBytes = OfflineNoteNfcApduProtocol.encodeInfo(OfflineNoteNfcPayloadKind.PAYMENT_TOKEN, payload)
        val info = assertNotNull(OfflineNoteNfcApduProtocol.decodeInfo(infoBytes))
        assertEquals(OfflineNoteNfcPayloadKind.PAYMENT_TOKEN, info.kind)
        assertEquals(payload.size, info.payloadLength)
        assertEquals(OfflineNoteNfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES, info.maxChunkLength)
        assertTrue(OfflineNoteNfcApduProtocol.payloadDigestMatches(payload, info.sha256()))

        val androidApdus = OfflineNoteTransferHandoff.nfcPaymentTokenWriteApdus(token)
        assertEquals(
            OfflineNoteNfcCommand.WriteMeta(OfflineNoteNfcPayloadKind.PAYMENT_TOKEN, payload.size, info.sha256()),
            OfflineNoteNfcApduProtocol.parseCommand(androidApdus.first()),
        )
        androidApdus.drop(1).dropLast(1).forEach { apdu ->
            val command = OfflineNoteNfcApduProtocol.parseCommand(apdu)
            assertTrue(command is OfflineNoteNfcCommand.WriteChunk)
            assertTrue(command.bytes.size <= OfflineNoteNfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES)
        }
        assertEquals(OfflineNoteNfcCommand.Commit, OfflineNoteNfcApduProtocol.parseCommand(androidApdus.last()))

        val fastPayload = ByteArray(512) { 0x5a.toByte() }
        val fastApdu = OfflineNoteNfcApduProtocol.writeChunkApdu(1024, fastPayload)
        assertContentEquals(
            byteArrayOf(0x80.toByte(), 0x21, 0x04, 0x00, 0x00, 0x02, 0x00),
            fastApdu.copyOfRange(0, 7),
        )
        assertEquals(
            OfflineNoteNfcCommand.WriteChunk(1024, fastPayload),
            OfflineNoteNfcApduProtocol.parseCommand(fastApdu),
        )
        val fastRead = OfflineNoteNfcApduProtocol.readChunkApdu(
            256,
            OfflineNoteNfcApduProtocol.MAX_EXTENDED_READ_CHUNK_BYTES,
        )
        assertEquals(
            OfflineNoteNfcCommand.ReadChunk(
                256,
                OfflineNoteNfcApduProtocol.MAX_EXTENDED_READ_CHUNK_BYTES,
            ),
            OfflineNoteNfcApduProtocol.parseCommand(fastRead),
        )
    }

    @Test
    fun transportWireFormatMatchesSharedFixture() {
        val fixture = loadFixture()
        val token = OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")),
        )
        val payload = OfflineNoteTransferHandoff.rawPaymentTokenBytes(token)
        val writeApdus = OfflineNoteTransferHandoff.nfcPaymentTokenWriteApdus(token)
        val readApdus = OfflineNoteNfcApduProtocol.readPayloadApdus(payload.size)
        val nearbyBytes = OfflineNoteTransferHandoff.nearbyPaymentEnvelopeBytes(token)

        assertEquals(4_674, payload.size)
        assertEquals("00a4040007f049524f48413200", hex(OfflineNoteNfcApduProtocol.selectAidApdu()))
        assertEquals("8010000000", hex(OfflineNoteNfcApduProtocol.getInfoApdu()))
        assertEquals(
            "020000124200f068ca7bc10b9a8c2d2da698c943d94f84eccc0fb795ede09337399075fb330d3c",
            hex(OfflineNoteNfcApduProtocol.encodeInfo(OfflineNoteNfcPayloadKind.PAYMENT_TOKEN, payload)),
        )
        assertEquals(
            "8020000025020000124268ca7bc10b9a8c2d2da698c943d94f84eccc0fb795ede09337399075fb330d3c",
            hex(OfflineNoteNfcApduProtocol.writeMetaApdu(OfflineNoteNfcPayloadKind.PAYMENT_TOKEN, payload)),
        )
        assertEquals(22, writeApdus.size)
        assertEquals(
            "8020000025020000124268ca7bc10b9a8c2d2da698c943d94f84eccc0fb795ede09337399075fb330d3c",
            hex(writeApdus[0]),
        )
        assertEquals(
            "53d4d61b3f22e432a5a309c4813f55f5562d74b96b59c657bc230f6d5a0031d4",
            hex(OfflineNoteNfcApduProtocol.sha256(writeApdus[1])),
        )
        assertEquals(
            "802111d0720968616c6f322f69706117166f66666c696e652d6e6f74652d72656375727369766520699b945eaef37b763f70ce18b173caed4fe4fec9bb8110fc5231feb9f868d7a52e0a0968616c6f322f697061221a000000000000006f66666c696e652d766563746f722d61756469742d70726f6f66",
            hex(writeApdus[writeApdus.size - 2]),
        )
        assertEquals("8022000000", hex(writeApdus.last()))
        assertEquals(20, readApdus.size)
        assertEquals("80110000f0", hex(readApdus.first()))
        assertEquals(6_330, nearbyBytes.size)
        assertEquals(
            "fa386f2157f8d9be82828eb1e79b6b57e05b9d4777d5e46b0c0684de11892184",
            hex(OfflineNoteNfcApduProtocol.sha256(nearbyBytes)),
        )
    }

    @Test
    fun nfcApduProtocolRejectsAdversarialPayloadsBeforeCommit() {
        val payload = "offline-payment".toByteArray()
        val info = assertNotNull(
            OfflineNoteNfcApduProtocol.decodeInfo(
                OfflineNoteNfcApduProtocol.encodeInfo(OfflineNoteNfcPayloadKind.RECEIPT_ACK, payload),
            ),
        )
        val assembler = OfflineNoteNfcPayloadAssembler(info)

        assertFalse(assembler.write(payload.size - 2, ByteArray(4) { 1 }))
        assertTrue(assembler.write(0, payload.copyOfRange(0, 6)))
        assertTrue(assembler.write(0, payload.copyOfRange(0, 6)))
        assertFalse(assembler.write(0, "OFFLIN".toByteArray()))
        assertFailsWith<IllegalArgumentException> { assembler.commit() }
        assertTrue(assembler.write(6, payload.copyOfRange(6, payload.size)))
        assertContentEquals(payload, assembler.commit())

        val oversizedInfo = OfflineNoteNfcApduProtocol.encodeInfo(OfflineNoteNfcPayloadKind.PAYMENT_TOKEN, payload)
        val oversized = OfflineNoteNfcApduProtocol.MAX_INCOMING_PAYLOAD_BYTES + 1
        oversizedInfo[1] = ((oversized ushr 24) and 0xff).toByte()
        oversizedInfo[2] = ((oversized ushr 16) and 0xff).toByte()
        oversizedInfo[3] = ((oversized ushr 8) and 0xff).toByte()
        oversizedInfo[4] = (oversized and 0xff).toByte()
        assertEquals(null, OfflineNoteNfcApduProtocol.decodeInfo(oversizedInfo))

        val badAssembler = OfflineNoteNfcPayloadAssembler(
            OfflineNoteNfcPayloadKind.PAYMENT_TOKEN,
            payload.size,
            ByteArray(32),
        )
        assertTrue(badAssembler.write(0, payload))
        assertFailsWith<IllegalArgumentException> { badAssembler.commit() }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteNfcPayloadAssembler(
                OfflineNoteNfcPayloadKind.PAYMENT_TOKEN,
                OfflineNoteNfcApduProtocol.MAX_INCOMING_PAYLOAD_BYTES + 1,
                ByteArray(32),
            )
        }
    }

    @Test
    fun nfcApduProtocolRejectsMalformedCommandsAndBounds() {
        assertEquals(OfflineNoteNfcCommand.Invalid, OfflineNoteNfcApduProtocol.parseCommand(null))
        assertEquals(OfflineNoteNfcCommand.Invalid, OfflineNoteNfcApduProtocol.parseCommand(byteArrayOf(0x00)))
        assertEquals(
            OfflineNoteNfcCommand.Unsupported,
            OfflineNoteNfcApduProtocol.parseCommand(
                byteArrayOf(0x00, 0xA4.toByte(), 0x04, 0x00, 0x01, 0xFF.toByte(), 0x00),
            ),
        )
        val selectWithNonZeroLe = OfflineNoteNfcApduProtocol.selectAidApdu()
        selectWithNonZeroLe[selectWithNonZeroLe.lastIndex] = 0x01
        assertEquals(OfflineNoteNfcCommand.Unsupported, OfflineNoteNfcApduProtocol.parseCommand(selectWithNonZeroLe))
        assertEquals(
            OfflineNoteNfcCommand.Unsupported,
            OfflineNoteNfcApduProtocol.parseCommand(byteArrayOf(0x81.toByte(), 0x10, 0x00, 0x00, 0x00)),
        )
        assertEquals(
            OfflineNoteNfcCommand.Invalid,
            OfflineNoteNfcApduProtocol.parseCommand(byteArrayOf(0x80.toByte(), 0x10, 0x00, 0x01, 0x00)),
        )
        assertEquals(
            OfflineNoteNfcCommand.Invalid,
            OfflineNoteNfcApduProtocol.parseCommand(byteArrayOf(0x80.toByte(), 0x10, 0x00, 0x00, 0x01)),
        )
        assertEquals(
            OfflineNoteNfcCommand.Invalid,
            OfflineNoteNfcApduProtocol.parseCommand(byteArrayOf(0x80.toByte(), 0x10, 0x00, 0x00, 0x01, 0x00)),
        )
        assertEquals(
            OfflineNoteNfcCommand.Invalid,
            OfflineNoteNfcApduProtocol.parseCommand(byteArrayOf(0x80.toByte(), 0x11, 0x00, 0x00, 0x00)),
        )
        assertEquals(
            OfflineNoteNfcCommand.Invalid,
            OfflineNoteNfcApduProtocol.parseCommand(
                byteArrayOf(0x80.toByte(), 0x11, 0x00, 0x00, 0x00, 0x00, 0x00),
            ),
        )
        assertEquals(
            OfflineNoteNfcCommand.Invalid,
            OfflineNoteNfcApduProtocol.parseCommand(byteArrayOf(0x80.toByte(), 0x20, 0x00, 0x00, 0x01, 0x01)),
        )
        val writeMetaWithOffset = OfflineNoteNfcApduProtocol.writeMetaApdu(
            OfflineNoteNfcPayloadKind.RECEIPT_ACK,
            byteArrayOf(0x01),
        )
        writeMetaWithOffset[3] = 0x01
        assertEquals(OfflineNoteNfcCommand.Invalid, OfflineNoteNfcApduProtocol.parseCommand(writeMetaWithOffset))
        val zeroLengthMeta = byteArrayOf(OfflineNoteNfcPayloadKind.PAYMENT_TOKEN.code.toByte(), 0x00, 0x00, 0x00, 0x00) +
            ByteArray(32)
        assertEquals(
            OfflineNoteNfcCommand.Invalid,
            OfflineNoteNfcApduProtocol.parseCommand(
                byteArrayOf(0x80.toByte(), 0x20, 0x00, 0x00, zeroLengthMeta.size.toByte()) + zeroLengthMeta,
            ),
        )
        assertEquals(
            OfflineNoteNfcCommand.Invalid,
            OfflineNoteNfcApduProtocol.parseCommand(byteArrayOf(0x80.toByte(), 0x21, 0x00, 0x00, 0x00)),
        )
        assertEquals(
            OfflineNoteNfcCommand.Invalid,
            OfflineNoteNfcApduProtocol.parseCommand(byteArrayOf(0x80.toByte(), 0x21, 0x00, 0x00, 0x02, 0x01)),
        )
        assertEquals(
            OfflineNoteNfcCommand.Invalid,
            OfflineNoteNfcApduProtocol.parseCommand(byteArrayOf(0x80.toByte(), 0x22, 0x00, 0x00, 0x01, 0x00)),
        )
        assertEquals(
            OfflineNoteNfcCommand.Invalid,
            OfflineNoteNfcApduProtocol.parseCommand(byteArrayOf(0x80.toByte(), 0x22, 0x01, 0x00, 0x00)),
        )
        assertEquals(
            OfflineNoteNfcCommand.Invalid,
            OfflineNoteNfcApduProtocol.parseCommand(byteArrayOf(0x80.toByte(), 0x22, 0x00, 0x00, 0x01)),
        )

        assertFailsWith<IllegalArgumentException> { OfflineNoteNfcApduProtocol.writeChunkApdu(0x1_0000, byteArrayOf(0x01)) }
        assertFailsWith<IllegalArgumentException> { OfflineNoteNfcApduProtocol.writeChunkApdu(0, ByteArray(0)) }
        assertFailsWith<IllegalArgumentException> { OfflineNoteNfcApduProtocol.readChunkApdu(0, 0) }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteNfcApduProtocol.readChunkApdu(
                0,
                OfflineNoteNfcApduProtocol.MAX_EXTENDED_READ_CHUNK_BYTES + 1,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteNfcApduProtocol.writePayloadApdus(
                OfflineNoteNfcPayloadKind.PAYMENT_TOKEN,
                byteArrayOf(0x01),
                0,
            )
        }
        assertFailsWith<IllegalArgumentException> { OfflineNoteNfcApduProtocol.readPayloadApdus(0) }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteNfcApduProtocol.readPayloadApdus(
                1,
                OfflineNoteNfcApduProtocol.MAX_EXTENDED_READ_CHUNK_BYTES + 1,
            )
        }

        val response = OfflineNoteNfcApduProtocol.response(byteArrayOf(0xAA.toByte(), 0xBB.toByte()))
        assertContentEquals(byteArrayOf(0xAA.toByte(), 0xBB.toByte(), 0x90.toByte(), 0x00), response)
        assertEquals(0x9000, OfflineNoteNfcApduProtocol.responseStatus(response))
        assertEquals(-1, OfflineNoteNfcApduProtocol.responseStatus(byteArrayOf(0x90.toByte())))
        assertContentEquals(byteArrayOf(0xAA.toByte(), 0xBB.toByte()), OfflineNoteNfcApduProtocol.responseData(response))
        assertContentEquals(ByteArray(0), OfflineNoteNfcApduProtocol.responseData(byteArrayOf(0x90.toByte())))

        val assembler = OfflineNoteNfcPayloadAssembler(
            OfflineNoteNfcPayloadKind.RECEIPT_ACK,
            4,
            OfflineNoteNfcApduProtocol.sha256(byteArrayOf(0x01, 0x02, 0x03, 0x04)),
        )
        assertFalse(assembler.write(Int.MAX_VALUE, byteArrayOf(0x01)))
        assertFalse(assembler.write(4, byteArrayOf(0x01)))
        assertFalse(assembler.write(-1, byteArrayOf(0x01)))
        assertFalse(assembler.write(0, ByteArray(0)))
        assertTrue(assembler.write(0, byteArrayOf(0x01, 0x02)))
        assertTrue(assembler.write(1, byteArrayOf(0x02, 0x03)))
        assertFalse(assembler.write(1, byteArrayOf(0x09, 0x09)))
    }

    @Test
    fun nearbyEnvelopeRoundTripsPairingPaymentAndAck() {
        val fixture = loadFixture()
        val token = OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")),
        )
        val receiveRequest = receiveRequestFixture(fixture)
        val receiptAck = OfflineNoteReceiptAck.fromPaymentToken(
            token = token,
            recipientAccountId = string(obj(fixture, "payment_token"), "recipient_account_id"),
            acceptedAtMs = long(obj(fixture, "receipt_ack"), "accepted_at_ms"),
        )
        val challenge = OfflineNoteNearbyPairingChallenge(" nearby_pairing_bird ")
        val challengeEnvelope = OfflineNoteNearbyEnvelope(
            kind = OfflineNoteNearbyMessageKind.RECEIVE_REQUEST,
            payload = OfflineNoteTransferHandoff.rawReceiveRequestBytes(receiveRequest),
            contentType = OfflineNoteTransferHandoff.RECEIVE_REQUEST_CONTENT_TYPE,
            pairingChallenge = challenge,
        )
        val paymentBytes = OfflineNoteTransferHandoff.nearbyPaymentEnvelopeBytes(token)
        val paymentEnvelope = OfflineNoteNearbyEnvelope.decode(paymentBytes)
        val ackEnvelope = OfflineNoteNearbyEnvelope(
            kind = OfflineNoteNearbyMessageKind.RECEIPT_ACK,
            payload = OfflineNoteTransferHandoff.rawReceiptAckBytes(receiptAck),
            contentType = OfflineNoteTransferHandoff.RECEIPT_ACK_CONTENT_TYPE,
        )

        assertEquals(challenge, OfflineNoteNearbyEnvelope.decode(challengeEnvelope.encoded()).pairingChallenge)
        assertEquals(
            receiveRequest.outputCommitmentHex(),
            OfflineNoteNearbyEnvelope.decode(challengeEnvelope.encoded()).receiveRequest().outputCommitmentHex(),
        )
        assertEquals(OfflineNoteNearbyMessageKind.PAYMENT, paymentEnvelope.kind)
        assertEquals(token.tokenIdHex(), paymentEnvelope.paymentToken().tokenIdHex())
        assertEquals(token.tokenIdHex(), OfflineNoteTransferHandoff.decodeNearbyPaymentToken(paymentBytes).tokenIdHex())
        assertTrue(OfflineNoteNearbyEnvelope.decode(ackEnvelope.encoded()).receiptAck().matchesPaymentToken(token))
        assertFalse(challengeEnvelope.requiresDisconnectGraceAfterSend())
        assertFalse(paymentEnvelope.requiresDisconnectGraceAfterSend())
        assertTrue(ackEnvelope.requiresDisconnectGraceAfterSend())
        assertEquals(
            OfflineNoteNearbyTransportPolicy.RECEIPT_ACK_DISCONNECT_GRACE_MILLIS,
            ackEnvelope.recommendedDisconnectGraceMillisAfterSend(),
        )
        assertEquals(
            0L,
            OfflineNoteNearbyTransportPolicy.disconnectGraceMillisAfterSending(
                OfflineNoteNearbyMessageKind.PAYMENT,
            ),
        )
    }

    @Test
    fun nearbyEnvelopeRejectsAdversarialMessages() {
        val fixture = loadFixture()
        val tokenPayload = base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64"))
        val pairing = OfflineNoteNearbyPairingChallenge("nearby_pairing_mask")

        assertFailsWith<IllegalArgumentException> { OfflineNoteNearbyPairingChallenge("nearby_pairing_mask<script>") }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteNearbyEnvelope(
                kind = OfflineNoteNearbyMessageKind.RECEIVE_REQUEST,
                payload = "challenge".toByteArray(),
                contentType = OfflineNoteTransferHandoff.RECEIVE_REQUEST_CONTENT_TYPE,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteNearbyEnvelope(
                kind = OfflineNoteNearbyMessageKind.RECEIVE_REQUEST,
                payload = "challenge".toByteArray(),
                contentType = OfflineNoteTransferHandoff.RECEIPT_ACK_CONTENT_TYPE,
                pairingChallenge = pairing,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteNearbyEnvelope(
                kind = OfflineNoteNearbyMessageKind.PAYMENT,
                payload = tokenPayload,
                contentType = OfflineNoteTransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE,
                pairingChallenge = pairing,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteNearbyEnvelope(
                kind = OfflineNoteNearbyMessageKind.PAYMENT,
                payload = ByteArray(OfflineNoteNfcApduProtocol.MAX_INCOMING_PAYLOAD_BYTES + 1) { 1 },
                contentType = OfflineNoteTransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteNearbyEnvelope(
                kind = OfflineNoteNearbyMessageKind.RECEIPT_ACK,
                payload = "ok".toByteArray(),
                contentType = OfflineNoteTransferHandoff.RECEIVE_REQUEST_CONTENT_TYPE,
            )
        }

        val unknownField = """
            {"kind":"payment","payload":"AQID","contentType":"application/vnd.iroha.offline.payment-token+norito","extra":true}
        """.trimIndent().toByteArray()
        val challengeContentTypeDowngrade = """
            {"kind":"receive_request","payload":"YQ","contentType":"application/vnd.iroha.offline.receipt-ack+norito","pairingChallenge":"nearby_pairing_bird"}
        """.trimIndent().toByteArray()
        val ackContentTypeDowngrade = """
            {"kind":"receipt_ack","payload":"b2s","contentType":"application/vnd.iroha.offline.receive-request+norito"}
        """.trimIndent().toByteArray()
        val paddedPayload = """
            {"kind":"receive_request","payload":"YQ==","contentType":"application/vnd.iroha.offline.receive-request+norito","pairingChallenge":"nearby_pairing_bird"}
        """.trimIndent().toByteArray()
        assertFailsWith<IllegalArgumentException> { OfflineNoteNearbyEnvelope.decode(unknownField) }
        assertFailsWith<IllegalArgumentException> { OfflineNoteNearbyEnvelope.decode(challengeContentTypeDowngrade) }
        assertFailsWith<IllegalArgumentException> { OfflineNoteNearbyEnvelope.decode(ackContentTypeDowngrade) }
        assertFailsWith<IllegalArgumentException> { OfflineNoteNearbyEnvelope.decode(paddedPayload) }

        val topLevelArray = "[]".toByteArray()
        val invalidBase64Payload = """
            {"kind":"receive_request","payload":"!!!!","contentType":"application/vnd.iroha.offline.receive-request+norito","pairingChallenge":"nearby_pairing_bird"}
        """.trimIndent().toByteArray()
        val badPairingObject = """
            {"kind":"receive_request","payload":"YQ","contentType":"application/vnd.iroha.offline.receive-request+norito","pairingChallenge":{"assetName":1}}
        """.trimIndent().toByteArray()
        val smuggledPairingObject = """
            {"kind":"receive_request","payload":"YQ","contentType":"application/vnd.iroha.offline.receive-request+norito","pairingChallenge":{"assetName":"nearby_pairing_bird","extra":true}}
        """.trimIndent().toByteArray()
        val ackWithPairing = """
            {"kind":"receipt_ack","payload":"b2s","contentType":"application/vnd.iroha.offline.receipt-ack+norito","pairingChallenge":"nearby_pairing_bird"}
        """.trimIndent().toByteArray()
        assertFailsWith<IllegalArgumentException> { OfflineNoteNearbyEnvelope.decode(topLevelArray) }
        assertFailsWith<IllegalArgumentException> { OfflineNoteNearbyEnvelope.decode(invalidBase64Payload) }
        assertFailsWith<IllegalArgumentException> { OfflineNoteNearbyEnvelope.decode(badPairingObject) }
        assertFailsWith<IllegalArgumentException> { OfflineNoteNearbyEnvelope.decode(smuggledPairingObject) }
        assertFailsWith<IllegalArgumentException> { OfflineNoteNearbyEnvelope.decode(ackWithPairing) }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteNearbyEnvelope(
                kind = OfflineNoteNearbyMessageKind.PAYMENT,
                payload = byteArrayOf(0x01, 0x02, 0x03),
                contentType = OfflineNoteTransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteNearbyEnvelope(
                kind = OfflineNoteNearbyMessageKind.RECEIPT_ACK,
                payload = ByteArray(0),
                contentType = OfflineNoteTransferHandoff.RECEIPT_ACK_CONTENT_TYPE,
            )
        }
    }

    @Test
    fun walletAcceptsOwnerCertPaymentTokenFromSender() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val chainId = string(obj(chain, "derivation"), "chain_id")
        val assetDefinitionId = assetDefinitionFromAssetId(string(obj(chain, "issue"), "asset_id"))
        val amount = string(obj(chain, "redeem"), "amount")
        val testIssuer = TestIssuerCertificateSigner()
        val verifier = Ed25519OfflineNoteCertificateVerifier(listOf(testIssuer.publicKey))
        val senderSigner = TestOwnerCertificateSigner()
        val recipientSigner = TestOwnerCertificateSigner()
        val senderStore = InMemoryOfflineNoteStore()
        senderStore.upsert(
            issuerSourceWalletNote(
                chainId = chainId,
                accountId = senderSigner.accountId,
                assetDefinitionId = assetDefinitionId,
                amount = amount,
                issuerCertificate = testIssuer.issuerCertificate(senderSigner.accountId),
                noteSecret = ByteArray(32) { 0x12.toByte() },
                operationSuffix = "accept-source",
                createdAtMs = 1_700_000_000_000L,
            )
        )
        val senderWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = senderSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(senderSigner.accountId)),
            ownerCertificateSigner = senderSigner,
            store = senderStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(listOf(ByteArray(32) { 0x23.toByte() })),
            idGenerator = FixedIdGenerator(string(obj(chain, "derivation"), "payment_request_id")),
            clock = { 1_700_000_001_000L },
        )
        val recipientStore = InMemoryOfflineNoteStore()
        val recipientWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = recipientSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(recipientSigner.accountId)),
            ownerCertificateSigner = recipientSigner,
            store = recipientStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(listOf(ByteArray(32) { 0x33.toByte() })),
            idGenerator = FixedIdGenerator(string(obj(chain, "derivation"), "payment_request_id")),
            clock = LongSupplier { 1_700_000_001_200L },
        )
        val receiveRequest = recipientWallet.prepareReceive(
            assetDefinitionId = assetDefinitionId,
            amount = amount,
        )
        assertEquals(recipientSigner.accountId, receiveRequest.keyCertificate.accountId)

        val token = senderWallet.pay(receiveRequest)
        assertEquals(1, token.audit.outputClaims.size)
        val accepted = recipientWallet.accept(token)

        assertContentEquals(receiveRequest.outputCommitment(), accepted.noteCommitment())
        assertEquals(OfflineNoteWalletNoteState.SPENDABLE, accepted.state)
        assertEquals(
            OfflineNoteWalletNoteState.SPENDABLE,
            recipientStore.findNote(receiveRequest.outputCommitment())?.state,
        )
    }

    @Test
    fun walletRejectsBearerCashCustodyPolicyOverflow() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val chainId = string(obj(chain, "derivation"), "chain_id")
        val assetDefinitionId = assetDefinitionFromAssetId(string(obj(chain, "issue"), "asset_id"))
        val amount = string(obj(chain, "redeem"), "amount")
        val testIssuer = TestIssuerCertificateSigner()
        val verifier = Ed25519OfflineNoteCertificateVerifier(listOf(testIssuer.publicKey))
        val senderSigner = TestOwnerCertificateSigner()
        val recipientSigner = TestOwnerCertificateSigner()
        val senderStore = InMemoryOfflineNoteStore()
        senderStore.upsert(
            issuerSourceWalletNote(
                chainId = chainId,
                accountId = senderSigner.accountId,
                assetDefinitionId = assetDefinitionId,
                amount = amount,
                issuerCertificate = testIssuer.issuerCertificate(senderSigner.accountId),
                noteSecret = ByteArray(32) { 0x13.toByte() },
                operationSuffix = "custody-source",
                createdAtMs = 1_700_000_000_000L,
            )
        )
        val senderWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = senderSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(senderSigner.accountId)),
            ownerCertificateSigner = senderSigner,
            store = senderStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(listOf(ByteArray(32) { 0x24.toByte() })),
            idGenerator = FixedIdGenerator(string(obj(chain, "derivation"), "payment_request_id")),
            clock = { 1_700_000_001_000L },
        )
        val recipientWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = recipientSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(recipientSigner.accountId)),
            ownerCertificateSigner = recipientSigner,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(listOf(ByteArray(32) { 0x34.toByte() })),
            idGenerator = FixedIdGenerator(string(obj(chain, "derivation"), "payment_request_id")),
            clock = LongSupplier { 1_700_000_001_250L },
            bearerCashPolicy = OfflineBearerCashPolicyV1(maxCustodyHops = 1),
        )
        val receiveRequest = recipientWallet.prepareReceive(
            assetDefinitionId = assetDefinitionId,
            amount = amount,
        )
        val token = senderWallet.pay(receiveRequest)
        val ancestor = ancestorAuditProducingFirstInput(token.audit, 0xC0)
        val overLimit = paymentTokenReplacingBearerAuditTrail(token, listOf(ancestor, token.audit))

        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(overLimit)
        }
    }

    @Test
    fun walletNoteScopeIdsRejectSurroundingWhitespace() {
        val fixture = loadFixture()
        val derivation = obj(obj(fixture, "chain_vectors"), "derivation")
        val senderCertificate = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"))
        val note = sourceWalletNote(fixture, senderCertificate)
        val spentPaymentRequestId = string(derivation, "payment_request_id")

        fun copy(
            chainId: String = note.chainId,
            accountId: String = note.accountId,
            spentPaymentRequestId: String? = note.spentPaymentRequestId,
        ): OfflineNoteWalletNote =
            OfflineNoteWalletNote(
                chainId = chainId,
                accountId = accountId,
                assetId = note.assetId,
                amount = note.canonicalAmount,
                keyCertificate = note.keyCertificate,
                noteCommitment = note.noteCommitment(),
                noteSecret = note.noteSecret(),
                origin = note.origin,
                bearerAuditTrail = note.bearerAuditTrail(),
                state = note.state,
                createdAtMs = note.createdAtMs,
                updatedAtMs = note.updatedAtMs,
                spentPaymentRequestId = spentPaymentRequestId,
            )

        assertEquals(spentPaymentRequestId, copy(spentPaymentRequestId = spentPaymentRequestId).spentPaymentRequestId)
        assertFailsWith<IllegalArgumentException> { copy(chainId = " ${note.chainId}") }
        assertFailsWith<IllegalArgumentException> { copy(accountId = "${note.accountId}\n") }
        assertFailsWith<IllegalArgumentException> { copy(spentPaymentRequestId = "$spentPaymentRequestId ") }
        assertFailsWith<IllegalArgumentException> { copy(spentPaymentRequestId = "") }
    }

    @Test
    fun walletLoadDerivesCommitmentBeforeIssuerSubmission() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val issue = obj(chain, "issue")
        val senderCertificate = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"))
        val loadContext = OfflineNoteLoadContext(
            operationId = string(derivation, "issuer_load_operation_id"),
            lineageId = string(derivation, "issuer_load_lineage_id"),
            localRevision = long(derivation, "issuer_load_local_revision"),
            keyCertificate = senderCertificate,
        )
        val issuerClient = RecordingIssuerClient(loadContext)
        val wallet = OfflineNoteWallet(
            chainId = string(derivation, "chain_id"),
            accountId = accountFromAssetId(string(issue, "asset_id")),
            attestationProvider = StaticAttestationProvider(senderCertificate),
            ownerCertificateSigner = TestOwnerCertificateSigner(),
            issuerClient = issuerClient,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = certificateVerifier(fixture),
            randomSource = QueueRandomSource(listOf(hexBytes(string(derivation, "source_note_secret_hex")))),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_001_000L },
        )

        val note = wallet.load(assetDefinitionFromAssetId(string(issue, "asset_id")), string(issue, "amount")).get()

        assertEquals(string(derivation, "source_note_commitment"), note.noteCommitmentHex())
        assertEquals(
            string(derivation, "source_note_commitment"),
            issuerClient.lastIssueRequest?.noteCommitmentHex(),
        )
        assertEquals(OfflineNoteWalletNoteState.SPENDABLE, note.state)
    }

    @Test
    fun walletLoadDoesNotBlockIssuerCompletionThread() {
        val fixture = loadFixture()
        val token = obj(fixture, "payment_token")
        val derivation = obj(obj(fixture, "chain_vectors"), "derivation")
        val issue = obj(obj(fixture, "chain_vectors"), "issue")
        val senderCertificate = certificate(obj(token, "sender_key_certificate"))
        val accountId = accountFromAssetId(string(issue, "asset_id"))
        val loadContext = OfflineNoteLoadContext(
            operationId = string(derivation, "issuer_load_operation_id"),
            lineageId = string(derivation, "issuer_load_lineage_id"),
            localRevision = long(derivation, "issuer_load_local_revision"),
            keyCertificate = senderCertificate,
        )
        val issuerClient = CompletionControlledIssuerClient(loadContext)
        val store = BlockingOfflineNoteStore()
        val wallet = OfflineNoteWallet(
            chainId = string(derivation, "chain_id"),
            accountId = accountId,
            attestationProvider = StaticAttestationProvider(senderCertificate),
            ownerCertificateSigner = TestOwnerCertificateSigner(),
            store = store,
            issuerClient = issuerClient,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = certificateVerifier(fixture),
            randomSource = QueueRandomSource(listOf(hexBytes(string(derivation, "source_note_secret_hex")))),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_001_000L },
        )

        val load = wallet.load(assetDefinitionFromAssetId(string(issue, "asset_id")), string(issue, "amount"))
        assertTrue(issuerClient.issueRequested.await(5, TimeUnit.SECONDS))
        val request = requireNotNull(issuerClient.lastIssueRequest)
        val response = OfflineNoteIssueResponse(
            noteCommitment = request.noteCommitment(),
            operationId = request.loadContext.operationId,
            lineageId = request.loadContext.lineageId,
            localRevision = request.loadContext.localRevision,
            keyCertificate = request.loadContext.keyCertificate,
            settlementEntryHashHex = "settlement-entry-hash",
        )
        val completeReturned = AtomicBoolean(false)
        val issuerCompleter = Executors.newSingleThreadExecutor { Thread(it, "offline-note-issuer-completer") }
        try {
            issuerCompleter.submit {
                issuerClient.issueFuture.complete(response)
                completeReturned.set(true)
            }
            assertTrue(store.entered.await(5, TimeUnit.SECONDS))
            assertTrue(completeReturned.get())
            store.release.countDown()
            assertEquals(
                string(derivation, "source_note_commitment"),
                load.get(5, TimeUnit.SECONDS).noteCommitmentHex(),
            )
        } finally {
            store.release.countDown()
            issuerCompleter.shutdownNow()
        }
    }

    @Test
    fun walletLoadCompletesExceptionallyWhenIssuerThrowsSynchronously() {
        val fixture = loadFixture()
        val token = obj(fixture, "payment_token")
        val derivation = obj(obj(fixture, "chain_vectors"), "derivation")
        val issue = obj(obj(fixture, "chain_vectors"), "issue")
        val senderCertificate = certificate(obj(token, "sender_key_certificate"))
        val accountId = accountFromAssetId(string(issue, "asset_id"))
        val loadContext = OfflineNoteLoadContext(
            operationId = string(derivation, "issuer_load_operation_id"),
            lineageId = string(derivation, "issuer_load_lineage_id"),
            localRevision = long(derivation, "issuer_load_local_revision"),
            keyCertificate = senderCertificate,
        )
        val wallet = OfflineNoteWallet(
            chainId = string(derivation, "chain_id"),
            accountId = accountId,
            attestationProvider = StaticAttestationProvider(senderCertificate),
            ownerCertificateSigner = TestOwnerCertificateSigner(),
            issuerClient = SynchronouslyThrowingIssuerClient(loadContext),
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = certificateVerifier(fixture),
            randomSource = QueueRandomSource(listOf(hexBytes(string(derivation, "source_note_secret_hex")))),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_001_000L },
        )

        val failure = assertFailsWith<ExecutionException> {
            wallet.load(assetDefinitionFromAssetId(string(issue, "asset_id")), string(issue, "amount"))
                .get(5, TimeUnit.SECONDS)
        }
        assertTrue(failure.cause is IllegalStateException)
        assertEquals("issuer exploded", failure.cause?.message)
    }

    @Test
    fun toriiIssuerClientBodySignsRefillAndIssuesWalletCommitment() {
        val fixture = loadFixture()
        val certificateJson = obj(obj(fixture, "payment_token"), "sender_key_certificate")
        val accountId = string(certificateJson, "account_id")
        val assetDefinitionId = assetDefinitionFromAssetId(string(obj(obj(fixture, "chain_vectors"), "issue"), "asset_id"))
        val offlinePublicKey = "a5".repeat(32)
        val deviceBinding = OfflineNoteIssuerDeviceBinding(
            deviceId = "device-1",
            offlinePublicKey = offlinePublicKey,
            deviceBinding = linkedMapOf(
                "device_id" to "device-1",
                "attestation_key_id" to "attestation-key-1",
                "offline_public_key" to offlinePublicKey,
                "signature_base64" to "nested-device-signature-is-not-body-auth",
            ),
        )
        val executor = OfflineIssuerExecutor(certificateJson, serverStateHash = " lineage-state-hash ")
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        var nowMs = 1_700_000_000_000L
        val client = ToriiOfflineNoteIssuerClient(
            canonicalAuth = ToriiCanonicalRequestAuth(accountId, keyPair.private),
            deviceBindingProvider = object : OfflineNoteIssuerDeviceBindingProvider {
                override fun currentDeviceBinding(
                    chainId: String,
                    accountId: String,
                    assetDefinitionId: String,
                ): OfflineNoteIssuerDeviceBinding = deviceBinding
            },
            executor = executor,
            baseUri = URI.create("https://torii.example"),
            clock = java.util.function.LongSupplier { nowMs },
            nonceGenerator = SequenceIdGenerator(
                "operation-refill-1",
                "auth-refill-1",
                "auth-issue-1",
                "operation-refill-2",
                "auth-refill-2",
            ),
        )

        val context = client.prepareLoad("chain-1", accountId, assetDefinitionId, "5").join()
        assertEquals("operation-refill-1", context.operationId)
        assertEquals("lineage-1", context.lineageId)
        assertEquals(1L, context.localRevision)

        val commitment = ByteArray(32) { (it + 1).toByte() }
        val response = client.issueNote(
            OfflineNoteIssueRequest(
                chainId = "chain-1",
                accountId = accountId,
                assetDefinitionId = assetDefinitionId,
                assetId = "$assetDefinitionId#$accountId",
                amount = "5",
                loadContext = context,
                noteCommitment = commitment,
            )
        ).join()

        assertEquals(hex(commitment), hex(response.noteCommitment()))
        assertEquals("settlement-entry-hash", response.settlementEntryHashHex)
        assertEquals(2, executor.requests.size)
        assertEquals("/v1/offline/keys/refill", executor.requests[0].uri.path)
        assertEquals("/v1/offline/notes/issue", executor.requests[1].uri.path)
        for (request in executor.requests) {
            assertFalse(request.headers.keys.any { it.startsWith("X-Iroha-", ignoreCase = true) })
        }

        val refillBody = executor.requestBody(0)
        assertEquals(accountId, string(refillBody, "account_id"))
        assertEquals("operation-refill-1", string(refillBody, "operation_id"))
        assertEquals(0L, long(refillBody, "local_revision"))
        assertEquals("", string(refillBody, "local_state_hash"))
        assertEquals("attestation-key-1", string(refillBody, "attestation_key_id"))
        assertEquals("auth-refill-1", string(refillBody, "nonce"))
        assertTrue(string(refillBody, "signature_base64").isNotBlank())
        assertEquals(
            "nested-device-signature-is-not-body-auth",
            string(obj(refillBody, "device_binding"), "signature_base64"),
        )

        val issueBody = executor.requestBody(1)
        assertEquals(hex(commitment), string(issueBody, "note_commitment"))
        assertEquals(0L, long(issueBody, "local_revision"))
        assertEquals("0", string(issueBody, "local_balance"))
        assertEquals("auth-issue-1", string(issueBody, "nonce"))
        assertNotNull(obj(issueBody, "lineage_state"))

        nowMs = 1_700_000_060_001L
        val refillContext = client.prepareLoad("chain-1", accountId, assetDefinitionId, "7").join()
        assertEquals("operation-refill-2", refillContext.operationId)
        assertEquals(3, executor.requests.size)
        val secondRefillBody = executor.requestBody(2)
        assertEquals(" lineage-state-hash ", string(secondRefillBody, "local_state_hash"))
    }

    @Test
    fun toriiIssuerClientRejectsMalformedCertificateUsageLimits() {
        val fixture = loadFixture()
        val baseCertificateJson = obj(obj(fixture, "payment_token"), "sender_key_certificate")
        val accountId = string(baseCertificateJson, "account_id")
        val assetDefinitionId = assetDefinitionFromAssetId(string(obj(obj(fixture, "chain_vectors"), "issue"), "asset_id"))
        val offlinePublicKey = "a5".repeat(32)
        val deviceBinding = OfflineNoteIssuerDeviceBinding(
            deviceId = "device-1",
            offlinePublicKey = offlinePublicKey,
            deviceBinding = linkedMapOf(
                "device_id" to "device-1",
                "attestation_key_id" to "attestation-key-1",
                "offline_public_key" to offlinePublicKey,
            ),
        )

        fun assertRejected(certificateJson: Map<String, Any?>) {
            val client = ToriiOfflineNoteIssuerClient(
                canonicalAuth = ToriiCanonicalRequestAuth(
                    accountId,
                    KeyPairGenerator.getInstance("Ed25519").generateKeyPair().private,
                ),
                deviceBindingProvider = object : OfflineNoteIssuerDeviceBindingProvider {
                    override fun currentDeviceBinding(
                        chainId: String,
                        accountId: String,
                        assetDefinitionId: String,
                    ): OfflineNoteIssuerDeviceBinding = deviceBinding
                },
                executor = OfflineIssuerExecutor(certificateJson),
                baseUri = URI.create("https://torii.example"),
                clock = LongSupplier { 1_700_000_000_000L },
                nonceGenerator = SequenceIdGenerator("operation-refill-malformed", "auth-refill-malformed"),
            )

            val failure = assertFailsWith<ExecutionException> {
                client.prepareLoad("chain-1", accountId, assetDefinitionId, "5").get(5, TimeUnit.SECONDS)
            }
            var root = failure.cause
            while (root is CompletionException && root.cause != null) {
                root = root.cause
            }
            assertTrue(
                root is OfflineToriiException ||
                    root is IllegalStateException ||
                    root is IllegalArgumentException,
                "unexpected failure ${root?.javaClass?.name}: ${root?.message}",
            )
        }

        for (invalidLimit in listOf<Any?>(0L, 2L, 4_294_967_297L, "1")) {
            val certificateJson = LinkedHashMap(baseCertificateJson)
            certificateJson["assertion_usage_count_limit"] = invalidLimit
            assertRejected(certificateJson)
        }
        for (invalidVersion in listOf<Any?>(0L, 2L, 4_294_967_297L, "1")) {
            val certificateJson = LinkedHashMap(baseCertificateJson)
            certificateJson["version"] = invalidVersion
            assertRejected(certificateJson)
        }
    }

    @Test
    fun walletLifecycleBuildsAuditAcceptAndRedeemTransactions() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val chainIssue = obj(chain, "issue")
        val chainId = string(derivation, "chain_id")
        val assetDefinitionId = assetDefinitionFromAssetId(string(chainIssue, "asset_id"))
        val testIssuer = TestIssuerCertificateSigner()
        val verifier = Ed25519OfflineNoteCertificateVerifier(listOf(testIssuer.publicKey))
        val senderSigner = TestOwnerCertificateSigner()
        val recipientSigner = TestOwnerCertificateSigner()
        val sourceNote = issuerSourceWalletNote(
            chainId = chainId,
            accountId = senderSigner.accountId,
            assetDefinitionId = assetDefinitionId,
            amount = string(chainIssue, "amount"),
            issuerCertificate = testIssuer.issuerCertificate(senderSigner.accountId),
            noteSecret = ByteArray(32) { 0x11.toByte() },
            operationSuffix = "lifecycle-source",
            createdAtMs = 1_700_000_000_000L,
        )
        val senderStore = InMemoryOfflineNoteStore()
        senderStore.upsert(sourceNote)
        val senderWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = senderSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(senderSigner.accountId)),
            ownerCertificateSigner = senderSigner,
            store = senderStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(
                listOf(
                    ByteArray(32) { 0x21.toByte() },
                    ByteArray(32) { 0x22.toByte() },
                )
            ),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { long(obj(fixture, "payment_token"), "created_at_ms") },
        )
        val recipientSubmitter = RecordingTransactionSubmitter()
        val recipientWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = recipientSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(recipientSigner.accountId)),
            ownerCertificateSigner = recipientSigner,
            transactionSubmitter = recipientSubmitter,
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(listOf(ByteArray(32) { 0x31.toByte() })),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_001_200L },
        )

        val receiveRequest = recipientWallet.prepareReceive(
            assetDefinitionId = assetDefinitionId,
            amount = chainRedeemAmount(fixture),
        )
        assertEquals(recipientSigner.accountId, receiveRequest.keyCertificate.accountId)
        assertTrue(verifier.verifyOwnerCertificate(receiveRequest.keyCertificate))

        val token = senderWallet.pay(receiveRequest)

        assertEquals(string(derivation, "payment_request_id"), token.paymentRequestId)
        assertEquals(2, token.audit.outputClaims.size)
        val recipientOutput = token.audit.outputClaims[0]
        val changeOutput = token.audit.outputClaims[1]
        assertEquals(recipientSigner.accountId, recipientOutput.keyCertificate.accountId)
        assertEquals(senderSigner.accountId, changeOutput.keyCertificate.accountId)
        assertTrue(verifier.verifyOwnerCertificate(recipientOutput.keyCertificate))
        assertTrue(verifier.verifyOwnerCertificate(changeOutput.keyCertificate))
        assertFalse(
            recipientOutput.keyCertificate.payloadHash()
                .contentEquals(token.audit.senderKeyCertificate.payloadHash())
        )
        assertFalse(
            changeOutput.keyCertificate.payloadHash()
                .contentEquals(token.audit.senderKeyCertificate.payloadHash())
        )
        val auditMetrics = OfflineBearerCashPolicyV1.DEFAULT.auditTrailMetrics(token.bearerAuditTrail(), token.audit)
        assertEquals(1, auditMetrics.custodyHops)
        assertEquals(1, auditMetrics.lineageSteps)
        assertEquals(
            OfflineNoteWalletNoteState.SPENT,
            senderStore.findNote(sourceNote.noteCommitment())?.state,
        )
        assertEquals(
            OfflineNoteWalletNoteState.SPENDABLE,
            senderStore.findNote(changeOutput.noteCommitment())?.state,
        )

        val accepted = recipientWallet.accept(token)

        assertEquals(OfflineNoteWalletNoteState.SPENDABLE, accepted.state)
        assertContentEquals(recipientOutput.noteCommitment(), accepted.noteCommitment())
        assertEquals(0, recipientSubmitter.audits.size)
        recipientWallet.publishAudit(token).get()
        assertEquals(1, recipientSubmitter.audits.size)
        val redeeming = recipientWallet.redeem(accepted).get()
        assertEquals(OfflineNoteWalletNoteState.REDEEM_PENDING, redeeming.state)
        assertEquals(0, recipientSubmitter.redemptions.size)
        assertEquals(1, recipientSubmitter.defunds.size)
        assertContentEquals(accepted.noteCommitment(), recipientSubmitter.defunds[0].first.sourceNoteCommitment())
        assertEquals(1, recipientSubmitter.defunds[0].second.size)
        assertEquals(token.tokenIdHex(), hex(recipientSubmitter.defunds[0].second[0].tokenId()))
    }

    private fun chainRedeemAmount(fixture: Map<String, Any?>): String =
        string(obj(obj(fixture, "chain_vectors"), "redeem"), "amount")

    @Test
    fun offlineNoteTransactionSubmitterIncludesFeeMetadata() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val payment = obj(fixture, "payment_token")
        val codec = NoritoJavaCodecAdapter()
        val client = CapturingIrohaClient()
        val metadata = IrohaOfflineNoteTransactionSubmitter.feeMetadata(
            gasAssetId = "xor#universal",
            feeSponsor = string(payment, "recipient_account_id"),
        )
        val submitter = IrohaOfflineNoteTransactionSubmitter(
            client = client,
            signer = FakeSigner(),
            chainId = string(derivation, "chain_id"),
            authority = string(payment, "sender_account_id"),
            codecAdapter = codec,
            clock = LongSupplier { 1_736_000_000_000L },
            transactionMetadata = metadata,
        )

        submitter.submitAudit(audit(fixture)).get(5, TimeUnit.SECONDS)

        val signed = assertNotNull(client.submittedTransaction)
        val payload = codec.decodeTransaction(signed.encodedPayload())
        assertEquals(metadata.mapValues { JsonValue.string(it.value) }, payload.metadata)
    }

    @Test
    fun walletRejectsExactAmountReceiveRequestReplayAfterRestart() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val chainIssue = obj(chain, "issue")
        val chainId = string(derivation, "chain_id")
        val assetDefinitionId = assetDefinitionFromAssetId(string(chainIssue, "asset_id"))
        val amount = string(chainIssue, "amount")
        val testIssuer = TestIssuerCertificateSigner()
        val verifier = Ed25519OfflineNoteCertificateVerifier(listOf(testIssuer.publicKey))
        val senderSigner = TestOwnerCertificateSigner()
        val recipientSigner = TestOwnerCertificateSigner()
        val senderStore = InMemoryOfflineNoteStore()
        senderStore.upsert(
            issuerSourceWalletNote(
                chainId = chainId,
                accountId = senderSigner.accountId,
                assetDefinitionId = assetDefinitionId,
                amount = amount,
                issuerCertificate = testIssuer.issuerCertificate(senderSigner.accountId),
                noteSecret = ByteArray(32) { 0x14.toByte() },
                operationSuffix = "exact-source",
                createdAtMs = 1_700_000_000_000L,
            )
        )
        senderStore.upsert(
            issuerSourceWalletNote(
                chainId = chainId,
                accountId = senderSigner.accountId,
                assetDefinitionId = assetDefinitionId,
                amount = amount,
                issuerCertificate = testIssuer.issuerCertificate(senderSigner.accountId),
                noteSecret = ByteArray(32) { 0x15.toByte() },
                operationSuffix = "exact-source-extra",
                createdAtMs = 1_700_000_000_100L,
            )
        )
        val senderWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = senderSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(senderSigner.accountId)),
            ownerCertificateSigner = senderSigner,
            store = senderStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(listOf(ByteArray(32) { 0x25.toByte() })),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_002_400L },
        )
        val recipientWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = recipientSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(recipientSigner.accountId)),
            ownerCertificateSigner = recipientSigner,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(listOf(ByteArray(32) { 0x35.toByte() })),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_002_500L },
        )

        val receiveRequest = recipientWallet.prepareReceive(
            assetDefinitionId = assetDefinitionId,
            amount = amount,
        )
        senderWallet.pay(receiveRequest)
        val spentNotes = senderStore.listNotes().filter { it.state == OfflineNoteWalletNoteState.SPENT }
        assertEquals(1, spentNotes.size)
        assertEquals(string(derivation, "payment_request_id"), spentNotes[0].spentPaymentRequestId)

        val restoredStore = InMemoryOfflineNoteStore()
        senderStore.listNotes().forEach { restoredStore.upsert(it) }
        val restoredWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = senderSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(senderSigner.accountId)),
            ownerCertificateSigner = senderSigner,
            store = restoredStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(emptyList()),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_002_600L },
        )

        assertFailsWith<IllegalArgumentException> {
            restoredWallet.pay(receiveRequest)
        }
        assertEquals(
            1,
            restoredStore.listNotes().count { it.state == OfflineNoteWalletNoteState.SPENDABLE },
        )
    }

    @Test
    fun walletSyncReconcilesPendingSpendChangeAndRedeemStates() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val chainIssue = obj(chain, "issue")
        val chainRedeem = obj(chain, "redeem")
        val chainId = string(derivation, "chain_id")
        val assetDefinitionId = assetDefinitionFromAssetId(string(chainIssue, "asset_id"))
        val testIssuer = TestIssuerCertificateSigner()
        val verifier = Ed25519OfflineNoteCertificateVerifier(listOf(testIssuer.publicKey))
        val senderSigner = TestOwnerCertificateSigner()
        val recipientSigner = TestOwnerCertificateSigner()
        val senderStore = InMemoryOfflineNoteStore()
        val sourceNote = issuerSourceWalletNote(
            chainId = chainId,
            accountId = senderSigner.accountId,
            assetDefinitionId = assetDefinitionId,
            amount = string(chainIssue, "amount"),
            issuerCertificate = testIssuer.issuerCertificate(senderSigner.accountId),
            noteSecret = ByteArray(32) { 0x16.toByte() },
            operationSuffix = "sync-source",
            createdAtMs = 1_700_000_000_000L,
        )
        senderStore.upsert(sourceNote)
        val resolutions = linkedMapOf<String, OfflineNoteWalletNoteState>()
        val syncResolver = RecordingSyncResolver(resolutions)
        val senderWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = senderSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(senderSigner.accountId)),
            ownerCertificateSigner = senderSigner,
            store = senderStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            syncResolver = syncResolver,
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(
                listOf(
                    ByteArray(32) { 0x26.toByte() },
                    ByteArray(32) { 0x27.toByte() },
                )
            ),
            clock = { 1_700_000_002_000L },
        )
        val recipientWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = recipientSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(recipientSigner.accountId)),
            ownerCertificateSigner = recipientSigner,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(listOf(ByteArray(32) { 0x36.toByte() })),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_002_100L },
        )

        val receiveRequest = recipientWallet.prepareReceive(
            assetDefinitionId = assetDefinitionId,
            amount = string(chainRedeem, "amount"),
        )
        val token = senderWallet.pay(receiveRequest)
        val changeCommitment = token.audit.outputClaims[1].noteCommitment()
        val sourceCommitmentHex = sourceNote.noteCommitmentHex()
        val changeCommitmentHex = hex(changeCommitment)
        resolutions[sourceCommitmentHex] = OfflineNoteWalletNoteState.SPENT
        resolutions[changeCommitmentHex] = OfflineNoteWalletNoteState.SPENDABLE
        senderWallet.sync().get()

        assertEquals(
            OfflineNoteWalletNoteState.SPENT,
            senderStore.findNote(sourceNote.noteCommitment())?.state,
        )
        val spendableChange = senderStore.findNote(changeCommitment)
        assertEquals(OfflineNoteWalletNoteState.SPENDABLE, spendableChange?.state)
        assertEquals(
            emptyList(),
            syncResolver.resolvedCommitments,
        )

        resolutions[changeCommitmentHex] = OfflineNoteWalletNoteState.REDEEMED
        val redeeming = senderWallet.redeem(requireNotNull(spendableChange)).get()
        assertEquals(OfflineNoteWalletNoteState.REDEEM_PENDING, redeeming.state)

        senderWallet.sync().get()

        assertEquals(
            OfflineNoteWalletNoteState.REDEEMED,
            senderStore.findNote(changeCommitment)?.state,
        )
    }

    @Test
    fun walletRejectsDuplicateTokenAndAlreadyPendingInputs() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val chainIssue = obj(chain, "issue")
        val chainRedeem = obj(chain, "redeem")
        val chainId = string(derivation, "chain_id")
        val assetDefinitionId = assetDefinitionFromAssetId(string(chainIssue, "asset_id"))
        val testIssuer = TestIssuerCertificateSigner()
        val verifier = Ed25519OfflineNoteCertificateVerifier(listOf(testIssuer.publicKey))
        val senderSigner = TestOwnerCertificateSigner()
        val recipientSigner = TestOwnerCertificateSigner()
        val senderStore = InMemoryOfflineNoteStore()
        senderStore.upsert(
            issuerSourceWalletNote(
                chainId = chainId,
                accountId = senderSigner.accountId,
                assetDefinitionId = assetDefinitionId,
                amount = string(chainIssue, "amount"),
                issuerCertificate = testIssuer.issuerCertificate(senderSigner.accountId),
                noteSecret = ByteArray(32) { 0x17.toByte() },
                operationSuffix = "duplicate-source",
                createdAtMs = 1_700_000_000_000L,
            )
        )
        val senderWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = senderSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(senderSigner.accountId)),
            ownerCertificateSigner = senderSigner,
            store = senderStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(
                listOf(
                    ByteArray(32) { 0x28.toByte() },
                    ByteArray(32) { 0x29.toByte() },
                )
            ),
            clock = { 1_700_000_002_200L },
        )
        val recipientWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = recipientSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(recipientSigner.accountId)),
            ownerCertificateSigner = recipientSigner,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(listOf(ByteArray(32) { 0x37.toByte() })),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_002_300L },
        )

        val receiveRequest = recipientWallet.prepareReceive(
            assetDefinitionId = assetDefinitionId,
            amount = string(chainRedeem, "amount"),
        )
        val token = senderWallet.pay(receiveRequest)

        assertFailsWith<IllegalArgumentException> {
            senderWallet.pay(receiveRequest)
        }

        val accepted = recipientWallet.accept(token)
        assertEquals(OfflineNoteWalletNoteState.SPENDABLE, accepted.state)
        assertFailsWith<IllegalStateException> {
            recipientWallet.accept(token)
        }
    }

    @Test
    fun walletRedeemReservesNoteBeforeSubmitCompletes() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val chainIssue = obj(chain, "issue")
        val payment = obj(fixture, "payment_token")
        val senderCertificate = certificate(obj(payment, "sender_key_certificate"))
        val store = InMemoryOfflineNoteStore()
        val note = sourceWalletNote(fixture, senderCertificate)
        store.upsert(note)
        val submitter = PendingDefundTransactionSubmitter()
        val wallet = OfflineNoteWallet(
            chainId = string(derivation, "chain_id"),
            accountId = accountFromAssetId(string(chainIssue, "asset_id")),
            attestationProvider = StaticAttestationProvider(senderCertificate),
            ownerCertificateSigner = TestOwnerCertificateSigner(),
            store = store,
            transactionSubmitter = submitter,
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = certificateVerifier(fixture),
            randomSource = QueueRandomSource(emptyList()),
            clock = { 1_700_000_004_000L },
        )

        val redeeming = wallet.redeem(note)
        assertFalse(redeeming.isDone)
        assertEquals(1, submitter.defunds.size)
        assertEquals(
            OfflineNoteWalletNoteState.REDEEM_PENDING,
            store.findNote(note.noteCommitment())?.state,
        )
        assertFailsWith<IllegalArgumentException> {
            wallet.redeem(note)
        }
        assertEquals(1, submitter.defunds.size)

        submitter.completeAccepted()
        assertEquals(OfflineNoteWalletNoteState.REDEEM_PENDING, redeeming.get(1, TimeUnit.SECONDS).state)
    }

    @Test
    fun walletRejectsAdversarialCertificateBindings() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val chainIssue = obj(chain, "issue")
        val chainRedeem = obj(chain, "redeem")
        val payment = obj(fixture, "payment_token")
        val chainId = string(derivation, "chain_id")
        val assetDefinitionId = assetDefinitionFromAssetId(string(chainIssue, "asset_id"))
        val sourceAmount = string(chainIssue, "amount")
        val receiveAmount = string(chainRedeem, "amount")
        val testIssuer = TestIssuerCertificateSigner()
        val verifier = Ed25519OfflineNoteCertificateVerifier(listOf(testIssuer.publicKey))
        val senderSigner = TestOwnerCertificateSigner()
        val recipientSigner = TestOwnerCertificateSigner()
        val senderAccountId = senderSigner.accountId
        // A cert minted by the trusted issuer but bound to the wrong account (used as a foreign cert).
        val foreignIssuerCertificate = testIssuer.issuerCertificate(recipientSigner.accountId)

        // A wallet whose default rejecting verifier blocks owner-cert minting in prepareReceive.
        val defaultRejectingSigner = TestOwnerCertificateSigner()
        val defaultRejectingWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = defaultRejectingSigner.accountId,
            attestationProvider = StaticAttestationProvider(
                testIssuer.issuerCertificate(defaultRejectingSigner.accountId)
            ),
            ownerCertificateSigner = defaultRejectingSigner,
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            randomSource = QueueRandomSource(emptyList()),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_002_700L },
        )
        assertFailsWith<IllegalArgumentException> {
            defaultRejectingWallet.prepareReceive(
                assetDefinitionId = assetDefinitionId,
                amount = receiveAmount,
            )
        }
        // A wallet whose owner signer controls a different account than the wallet account, so
        // prepareReceive cannot mint a self-signed certificate for the wallet account.
        val mismatchedSignerWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = senderAccountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(senderAccountId)),
            ownerCertificateSigner = recipientSigner,
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(emptyList()),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_002_710L },
        )
        assertFailsWith<IllegalArgumentException> {
            mismatchedSignerWallet.prepareReceive(
                assetDefinitionId = assetDefinitionId,
                amount = receiveAmount,
            )
        }

        val senderStore = InMemoryOfflineNoteStore()
        senderStore.upsert(adversarialSourceNote(chainId, senderSigner, assetDefinitionId, sourceAmount, testIssuer, 0x51))
        val senderWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = senderAccountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(senderAccountId)),
            ownerCertificateSigner = senderSigner,
            store = senderStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(
                listOf(
                    ByteArray(32) { 0x52.toByte() },
                    ByteArray(32) { 0x53.toByte() },
                )
            ),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { long(payment, "created_at_ms") },
        )
        val recipientWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = recipientSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(recipientSigner.accountId)),
            ownerCertificateSigner = recipientSigner,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(listOf(ByteArray(32) { 0x54.toByte() })),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_002_800L },
        )
        val receiveRequest = recipientWallet.prepareReceive(
            assetDefinitionId = assetDefinitionId,
            amount = receiveAmount,
        )
        val accountSubstitution = OfflineNoteReceiveRequest(
            chainId = receiveRequest.chainId,
            paymentRequestId = receiveRequest.paymentRequestId,
            accountId = senderAccountId,
            assetDefinitionId = receiveRequest.assetDefinitionId,
            assetId = receiveRequest.assetId,
            amount = receiveRequest.amount,
            keyCertificate = receiveRequest.keyCertificate,
            outputCommitment = receiveRequest.outputCommitment(),
        )
        assertFailsWith<IllegalArgumentException> {
            senderWallet.pay(accountSubstitution)
        }
        val chainSubstitution = OfflineNoteReceiveRequest(
            chainId = "${receiveRequest.chainId}-evil",
            paymentRequestId = receiveRequest.paymentRequestId,
            accountId = receiveRequest.accountId,
            assetDefinitionId = receiveRequest.assetDefinitionId,
            assetId = receiveRequest.assetId,
            amount = receiveRequest.amount,
            keyCertificate = receiveRequest.keyCertificate,
            outputCommitment = receiveRequest.outputCommitment(),
        )
        assertFailsWith<IllegalArgumentException> {
            senderWallet.pay(chainSubstitution)
        }
        val assetOwnerSubstitution = OfflineNoteReceiveRequest(
            chainId = receiveRequest.chainId,
            paymentRequestId = receiveRequest.paymentRequestId,
            accountId = receiveRequest.accountId,
            assetDefinitionId = receiveRequest.assetDefinitionId,
            assetId = "${receiveRequest.assetDefinitionId}#$senderAccountId",
            amount = receiveRequest.amount,
            keyCertificate = receiveRequest.keyCertificate,
            outputCommitment = receiveRequest.outputCommitment(),
        )
        val assetOwnerSubstitutionStore = InMemoryOfflineNoteStore()
        assetOwnerSubstitutionStore.upsert(
            adversarialSourceNote(chainId, senderSigner, assetDefinitionId, sourceAmount, testIssuer, 0x55)
        )
        val assetOwnerSubstitutionSender = OfflineNoteWallet(
            chainId = chainId,
            accountId = senderAccountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(senderAccountId)),
            ownerCertificateSigner = senderSigner,
            store = assetOwnerSubstitutionStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(
                listOf(
                    ByteArray(32) { 0x56.toByte() },
                    ByteArray(32) { 0x57.toByte() },
                )
            ),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { long(payment, "created_at_ms") + 3 },
        )
        assertFailsWith<IllegalArgumentException> {
            assetOwnerSubstitutionSender.pay(assetOwnerSubstitution)
        }

        val forgedInputStore = InMemoryOfflineNoteStore()
        forgedInputStore.upsert(
            adversarialSourceNote(
                chainId,
                senderSigner,
                assetDefinitionId,
                sourceAmount,
                issuerCertificate = tamperedSignatureCertificate(testIssuer.issuerCertificate(senderAccountId)),
                secretByte = 0x58,
            )
        )
        val forgedInputWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = senderAccountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(senderAccountId)),
            ownerCertificateSigner = senderSigner,
            store = forgedInputStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(emptyList()),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_002_900L },
        )
        assertFailsWith<IllegalArgumentException> {
            forgedInputWallet.pay(receiveRequest)
        }
        val wrongAccountInputStore = InMemoryOfflineNoteStore()
        wrongAccountInputStore.upsert(
            adversarialSourceNote(
                chainId,
                senderSigner,
                assetDefinitionId,
                sourceAmount,
                issuerCertificate = foreignIssuerCertificate,
                secretByte = 0x59,
            )
        )
        val wrongAccountInputWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = senderAccountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(senderAccountId)),
            ownerCertificateSigner = senderSigner,
            store = wrongAccountInputStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(emptyList()),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_002_910L },
        )
        assertFailsWith<IllegalArgumentException> {
            wrongAccountInputWallet.pay(receiveRequest)
        }
        val commitmentSubstitutionStore = InMemoryOfflineNoteStore()
        commitmentSubstitutionStore.upsert(
            adversarialSourceNote(chainId, senderSigner, assetDefinitionId, sourceAmount, testIssuer, 0x5A)
        )
        val commitmentSubstitutionSender = OfflineNoteWallet(
            chainId = chainId,
            accountId = senderAccountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(senderAccountId)),
            ownerCertificateSigner = senderSigner,
            store = commitmentSubstitutionStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(
                listOf(
                    ByteArray(32) { 0x5B.toByte() },
                    ByteArray(32) { 0x5C.toByte() },
                )
            ),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { long(payment, "created_at_ms") + 1 },
        )
        val commitmentSubstitution = OfflineNoteReceiveRequest(
            chainId = receiveRequest.chainId,
            paymentRequestId = receiveRequest.paymentRequestId,
            accountId = receiveRequest.accountId,
            assetDefinitionId = receiveRequest.assetDefinitionId,
            assetId = receiveRequest.assetId,
            amount = receiveRequest.amount,
            keyCertificate = receiveRequest.keyCertificate,
            outputCommitment = ByteArray(32) { 0xA5.toByte() },
        )
        assertFailsWith<IllegalStateException> {
            recipientWallet.accept(commitmentSubstitutionSender.pay(commitmentSubstitution))
        }
        val forgedOutputAmount = if (receiveRequest.amount == "1") "2" else "1"
        val amountSubstitutionStore = InMemoryOfflineNoteStore()
        amountSubstitutionStore.upsert(
            adversarialSourceNote(chainId, senderSigner, assetDefinitionId, sourceAmount, testIssuer, 0x5D)
        )
        val amountSubstitutionSender = OfflineNoteWallet(
            chainId = chainId,
            accountId = senderAccountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(senderAccountId)),
            ownerCertificateSigner = senderSigner,
            store = amountSubstitutionStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(
                listOf(
                    ByteArray(32) { 0x5E.toByte() },
                    ByteArray(32) { 0x5F.toByte() },
                )
            ),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { long(payment, "created_at_ms") + 2 },
        )
        val amountSubstitution = OfflineNoteReceiveRequest(
            chainId = receiveRequest.chainId,
            paymentRequestId = receiveRequest.paymentRequestId,
            accountId = receiveRequest.accountId,
            assetDefinitionId = receiveRequest.assetDefinitionId,
            assetId = receiveRequest.assetId,
            amount = forgedOutputAmount,
            keyCertificate = receiveRequest.keyCertificate,
            outputCommitment = receiveRequest.outputCommitment(),
        )
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(amountSubstitutionSender.pay(amountSubstitution))
        }

        val token = senderWallet.pay(receiveRequest)
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(paymentTokenReplacingChainId(token, "${token.chainId}-evil"))
        }
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(
                paymentTokenReplacingPaymentRequestId(token, "${token.paymentRequestId}-evil")
            )
        }
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(paymentTokenReplacingTopLevelTokenId(token))
        }
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(paymentTokenReplacingAuditTokenId(token))
        }
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(
                paymentTokenReplacingFirstOutputAmountWithoutProofRebind(token, forgedOutputAmount)
            )
        }
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(paymentTokenReplacingFirstOutputAmount(token, forgedOutputAmount))
        }
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(
                paymentTokenReplacingFirstOutputAsset(
                    token,
                    "${receiveRequest.assetId}#dataspace:1",
                )
            )
        }
        assertTrue(token.audit.outputClaims.size >= 2)
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(paymentTokenReversingOutputs(token))
        }
        assertFailsWith<IllegalStateException> {
            recipientWallet.accept(paymentTokenDroppingFirstOutput(token))
        }
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(paymentTokenReplacingFirstOutputCertificate(token, foreignIssuerCertificate))
        }
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(paymentTokenReplacingLastOutputCertificate(token, foreignIssuerCertificate))
        }
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(
                paymentTokenReplacingFirstInputClaimHash(token, foreignIssuerCertificate.payloadHash())
            )
        }
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(paymentTokenReplacingSenderCertificate(token, foreignIssuerCertificate))
        }
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(paymentTokenReplacingBearerAuditTrail(token, emptyList()))
        }
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(
                paymentTokenReplacingBearerAuditTrail(
                    token,
                    listOf(auditReplacingTokenIdWithoutProofRebind(token.audit)),
                )
            )
        }
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(paymentTokenReplacingBearerAuditTrail(token, listOf(token.audit, token.audit)))
        }
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(
                paymentTokenReplacingBearerAuditTrail(
                    token,
                    listOf(
                        auditReplacingFirstOutputAmountWithoutProofRebind(token.audit, forgedOutputAmount),
                        token.audit,
                    ),
                )
            )
        }
        assertFailsWith<IllegalArgumentException> {
            recipientWallet.accept(
                paymentTokenReplacingBearerAuditTrail(
                    token,
                    listOf(selfConsumingAudit(token.audit), token.audit),
                )
            )
        }

        assertEquals(OfflineNoteWalletNoteState.SPENDABLE, recipientWallet.accept(token).state)
    }

    private fun adversarialSourceNote(
        chainId: String,
        ownerSigner: TestOwnerCertificateSigner,
        assetDefinitionId: String,
        amount: String,
        issuer: TestIssuerCertificateSigner,
        secretByte: Int,
    ): OfflineNoteWalletNote =
        adversarialSourceNote(
            chainId,
            ownerSigner,
            assetDefinitionId,
            amount,
            issuerCertificate = issuer.issuerCertificate(ownerSigner.accountId),
            secretByte = secretByte,
        )

    private fun adversarialSourceNote(
        chainId: String,
        ownerSigner: TestOwnerCertificateSigner,
        assetDefinitionId: String,
        amount: String,
        issuerCertificate: OfflineNote.KeyCertificate,
        secretByte: Int,
    ): OfflineNoteWalletNote =
        issuerSourceWalletNote(
            chainId = chainId,
            accountId = ownerSigner.accountId,
            assetDefinitionId = assetDefinitionId,
            amount = amount,
            issuerCertificate = issuerCertificate,
            noteSecret = ByteArray(32) { secretByte.toByte() },
            operationSuffix = "adversarial-${secretByte.toString(16)}",
            createdAtMs = 1_700_000_000_000L,
        )

    @Test
    fun walletSyncReconcilesFailedAuditAndRedeemOutcomes() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val chainIssue = obj(chain, "issue")
        val chainRedeem = obj(chain, "redeem")
        val chainId = string(derivation, "chain_id")
        val assetDefinitionId = assetDefinitionFromAssetId(string(chainIssue, "asset_id"))
        val testIssuer = TestIssuerCertificateSigner()
        val verifier = Ed25519OfflineNoteCertificateVerifier(listOf(testIssuer.publicKey))
        val senderSigner = TestOwnerCertificateSigner()
        val recipientSigner = TestOwnerCertificateSigner()
        val senderStore = InMemoryOfflineNoteStore()
        val sourceNote = issuerSourceWalletNote(
            chainId = chainId,
            accountId = senderSigner.accountId,
            assetDefinitionId = assetDefinitionId,
            amount = string(chainIssue, "amount"),
            issuerCertificate = testIssuer.issuerCertificate(senderSigner.accountId),
            noteSecret = ByteArray(32) { 0x18.toByte() },
            operationSuffix = "failed-sync-source",
            createdAtMs = 1_700_000_000_000L,
        )
        senderStore.upsert(sourceNote)
        val senderWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = senderSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(senderSigner.accountId)),
            ownerCertificateSigner = senderSigner,
            store = senderStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            syncResolver = RecordingSyncResolver(emptyMap()),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(
                listOf(
                    ByteArray(32) { 0x2A.toByte() },
                    ByteArray(32) { 0x2B.toByte() },
                )
            ),
            clock = { 1_700_000_002_400L },
        )
        val recipientStore = InMemoryOfflineNoteStore()
        val recipientWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = recipientSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(recipientSigner.accountId)),
            ownerCertificateSigner = recipientSigner,
            store = recipientStore,
            transactionSubmitter = RejectingTransactionSubmitter(),
            syncResolver = RecordingSyncResolver(emptyMap()),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(listOf(ByteArray(32) { 0x3A.toByte() })),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_002_500L },
        )

        val receiveRequest = recipientWallet.prepareReceive(
            assetDefinitionId = assetDefinitionId,
            amount = string(chainRedeem, "amount"),
        )
        val token = senderWallet.pay(receiveRequest)
        val changeCommitment = token.audit.outputClaims[1].noteCommitment()

        val accepted = recipientWallet.accept(token)
        assertEquals(OfflineNoteWalletNoteState.SPENDABLE, accepted.state)
        assertFutureFails(recipientWallet.publishAudit(token))
        assertEquals(
            OfflineNoteWalletNoteState.SPENDABLE,
            recipientStore.findNote(receiveRequest.outputCommitment())?.state,
        )

        senderWallet.sync().join()
        recipientWallet.sync().join()

        assertEquals(
            OfflineNoteWalletNoteState.SPENT,
            senderStore.findNote(sourceNote.noteCommitment())?.state,
        )
        assertEquals(
            OfflineNoteWalletNoteState.SPENDABLE,
            senderStore.findNote(changeCommitment)?.state,
        )
        assertEquals(
            OfflineNoteWalletNoteState.SPENDABLE,
            recipientStore.findNote(receiveRequest.outputCommitment())?.state,
        )

        val redeemStore = InMemoryOfflineNoteStore()
        val redeemNote = issuerSourceWalletNote(
            chainId = chainId,
            accountId = senderSigner.accountId,
            assetDefinitionId = assetDefinitionId,
            amount = string(chainIssue, "amount"),
            issuerCertificate = testIssuer.issuerCertificate(senderSigner.accountId),
            noteSecret = ByteArray(32) { 0x19.toByte() },
            operationSuffix = "failed-sync-redeem",
            createdAtMs = 1_700_000_000_000L,
        )
        redeemStore.upsert(redeemNote)
        val redeemWallet = OfflineNoteWallet(
            chainId = chainId,
            accountId = senderSigner.accountId,
            attestationProvider = StaticAttestationProvider(testIssuer.issuerCertificate(senderSigner.accountId)),
            ownerCertificateSigner = senderSigner,
            store = redeemStore,
            transactionSubmitter = RejectingTransactionSubmitter(),
            syncResolver = RecordingSyncResolver(
                mapOf(redeemNote.noteCommitmentHex() to OfflineNoteWalletNoteState.SPENDABLE)
            ),
            proofProvider = BindingProofProvider,
            proofVerifier = BindingProofVerifier,
            certificateVerifier = verifier,
            randomSource = QueueRandomSource(emptyList()),
            clock = { 1_700_000_002_600L },
        )

        assertFutureFails(redeemWallet.redeem(redeemNote))
        assertEquals(
            OfflineNoteWalletNoteState.SPENDABLE,
            redeemStore.findNote(redeemNote.noteCommitment())?.state,
        )

        redeemWallet.sync().join()

        assertEquals(
            OfflineNoteWalletNoteState.SPENDABLE,
            redeemStore.findNote(redeemNote.noteCommitment())?.state,
        )
    }

    @Test
    fun outcomeIndexResolvesCommittedAndRejectedExplorerInstructions() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val issueVector = obj(chain, "issue")
        val payment = obj(fixture, "payment_token")
        val audit = audit(fixture)
        val redeem = redeem(fixture)
        val recipientCertificate = certificate(obj(payment, "recipient_key_certificate"))
        val redeemPending = OfflineNoteWalletNote(
            chainId = string(derivation, "chain_id"),
            accountId = string(payment, "recipient_account_id"),
            assetId = string(issueVector, "asset_id"),
            amount = string(obj(chain, "redeem"), "amount"),
            keyCertificate = recipientCertificate,
            noteCommitment = redeem.sourceNoteCommitment(),
            noteSecret = hexBytes(string(derivation, "recipient_note_secret_hex")),
            origin = OfflineNote.CommitmentOrigin.P2pOutput(
                paymentRequestId = string(derivation, "payment_request_id"),
                outputIndex = 0,
            ),
            state = OfflineNoteWalletNoteState.REDEEM_PENDING,
            createdAtMs = 1_700_000_003_100L,
            updatedAtMs = 1_700_000_003_100L,
        )

        val committed = OfflineNoteOutcomeIndex.fromExplorerOutcomes(
            listOf(
                OfflineNoteExplorerInstructionOutcome(
                    kind = OfflineNoteOutcomeIndex.KIND_AUDIT,
                    transactionStatus = "Committed",
                    transactionHashHex = "audit-tx",
                    encodedInstruction = rawInstructionPair(
                        OfflineNote.AUDIT_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNote.auditInstruction(audit)),
                    ),
                ),
                OfflineNoteExplorerInstructionOutcome(
                    kind = OfflineNoteOutcomeIndex.KIND_REDEEM,
                    transactionStatus = "Committed",
                    transactionHashHex = "redeem-tx",
                    encodedInstruction = rawInstructionPair(
                        OfflineNote.REDEEM_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNote.redeemInstruction(redeem)),
                    ),
                ),
            )
        )
        assertEquals(null, committed.resolve(sourceWalletNote(fixture, certificate(obj(payment, "sender_key_certificate")))))
        assertEquals(OfflineNoteWalletNoteState.REDEEMED, committed.resolve(redeemPending)?.state)

        val rejected = OfflineNoteOutcomeIndex()
            .recordRejectedAudit(audit, "audit-rejected")
            .recordRejectedRedeem(redeem, "redeem-rejected")
        assertEquals(OfflineNoteWalletNoteState.SPENDABLE, rejected.resolve(redeemPending)?.state)
    }

    private fun issue(fixture: Map<String, Any?>): OfflineNote.Issue {
        val chainIssue = obj(obj(fixture, "chain_vectors"), "issue")
        return OfflineNote.Issue(
            noteCommitment = hexBytes(string(chainIssue, "note_commitment")),
            keyCertificate = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate")),
            assetId = string(chainIssue, "asset_id"),
            amount = string(chainIssue, "amount"),
        )
    }

    private fun redeem(fixture: Map<String, Any?>): OfflineNote.Redeem {
        val vector = obj(obj(fixture, "chain_vectors"), "redeem")
        val payment = obj(fixture, "payment_token")
        return OfflineNote.Redeem(
            sourceNoteCommitment = hexBytes(string(vector, "source_note_commitment")),
            inputNullifiers = list(vector, "input_nullifiers").map { hexBytes(it as String) },
            senderKeyCertificate = certificate(obj(payment, "recipient_key_certificate")),
            recipient = string(payment, "recipient_account_id"),
            assetId = string(vector, "asset_id"),
            amount = string(vector, "amount"),
            recursiveProof = OfflineNote.RecursiveProof(
                publicInputsHash = hexBytes(string(vector, "public_inputs_hash")),
                proof = OfflineNote.ProofBox(
                    OfflineNote.RECURSIVE_BACKEND,
                    "offline-vector-redeem-proof".toByteArray()
                )
            )
        )
    }

    private fun audit(fixture: Map<String, Any?>): OfflineNote.AuditBundle {
        val vector = obj(obj(fixture, "chain_vectors"), "audit")
        val payment = obj(fixture, "payment_token")
        return OfflineNote.AuditBundle(
            tokenId = hexBytes(string(vector, "token_id")),
            senderKeyCertificate = certificate(obj(payment, "sender_key_certificate")),
            inputNullifiers = list(vector, "input_nullifiers").map { hexBytes(it as String) },
            inputClaims = list(payment, "input_claims").map { issuedClaim(it as Map<String, Any?>) },
            outputCommitments = list(vector, "output_commitments").map { hexBytes(it as String) },
            outputClaims = list(payment, "output_claims").map { auditOutputClaim(it as Map<String, Any?>) },
            recursiveProof = OfflineNote.RecursiveProof(
                publicInputsHash = hexBytes(string(vector, "public_inputs_hash")),
                proof = OfflineNote.ProofBox(
                    OfflineNote.RECURSIVE_BACKEND,
                    "offline-vector-audit-proof".toByteArray()
                )
            )
        )
    }

    private fun ancestorAuditProducingFirstInput(
        child: OfflineNote.AuditBundle,
        seed: Int,
    ): OfflineNote.AuditBundle {
        val childInput = child.inputClaims.first()
        val parentInput = OfflineNote.IssuedClaim(
            noteCommitment = ByteArray(32) { (seed or 1).toByte() },
            keyCertificatePayloadHash = child.senderKeyCertificate.payloadHash(),
            assetId = childInput.assetId,
            amount = childInput.amount,
        )
        val output = OfflineNote.AuditOutputClaim(
            noteCommitment = childInput.noteCommitment(),
            keyCertificate = child.senderKeyCertificate,
            assetId = childInput.assetId,
            amount = childInput.amount,
        )
        val tokenId = ByteArray(32) { ((seed + 2) or 1).toByte() }
        val inputNullifiers = listOf(ByteArray(32) { ((seed + 4) or 1).toByte() })
        val outputCommitments = listOf(childInput.noteCommitment())
        val auditPublicInputs = OfflineNote.AuditPublicInputs(
            tokenId = tokenId,
            keyCertificatePayloadHash = child.senderKeyCertificate.payloadHash(),
            inputNullifiers = inputNullifiers,
            inputClaims = listOf(parentInput),
            outputCommitments = outputCommitments,
            outputClaims = listOf(output.issuedClaim()),
        )
        val draft = OfflineNote.AuditBundle(
            tokenId = tokenId,
            senderKeyCertificate = child.senderKeyCertificate,
            inputNullifiers = inputNullifiers,
            inputClaims = listOf(parentInput),
            outputCommitments = outputCommitments,
            outputClaims = listOf(output),
            recursiveProof = OfflineNote.RecursiveProof(
                publicInputsHash = auditPublicInputs.publicInputsHash(),
                proof = OfflineNote.ProofBox(
                    OfflineNote.RECURSIVE_BACKEND,
                    "ancestor-audit-provisional".toByteArray(),
                ),
            ),
        )
        return draft.replacingRecursiveProof(
            OfflineNote.RecursiveProof(
                publicInputsHash = draft.publicInputsHash(),
                proof = OfflineNote.ProofBox(
                    OfflineNote.RECURSIVE_BACKEND,
                    "ancestor-audit-proof".toByteArray(),
                ),
            )
        )
    }

    private fun certificate(json: Map<String, Any?>): OfflineNote.KeyCertificate =
        OfflineNote.KeyCertificate(
            version = int(json, "version"),
            platform = string(json, "platform"),
            keyId = string(json, "key_id"),
            deviceId = string(json, "device_id"),
            accountId = string(json, "account_id"),
            publicKey = base64Bytes(string(json, "public_key")),
            assertionScheme = string(json, "assertion_scheme"),
            assertionKeyAlgorithm = string(json, "assertion_key_algorithm"),
            assertionPublicKey = base64Bytes(string(json, "assertion_public_key")),
            assertionUsageCountLimit = nullableInt(json, "assertion_usage_count_limit"),
            oneUse = bool(json, "one_use"),
            issuerSignature = base64Bytes(string(json, "issuer_signature_base64")),
        )

    private fun certificateVerifier(fixture: Map<String, Any?>): Ed25519OfflineNoteCertificateVerifier =
        Ed25519OfflineNoteCertificateVerifier(
            listOf(base64Bytes(string(fixture, "offline_fi_public_key_base64")))
        )

    private fun tamperedSignatureCertificate(
        certificate: OfflineNote.KeyCertificate,
    ): OfflineNote.KeyCertificate {
        val signature = certificate.issuerSignature()
        signature[0] = (signature[0].toInt() xor 0x01).toByte()
        return OfflineNote.KeyCertificate(
            version = certificate.version,
            platform = certificate.platform,
            keyId = certificate.keyId,
            deviceId = certificate.deviceId,
            accountId = certificate.accountId,
            publicKey = certificate.publicKey(),
            assertionScheme = certificate.assertionScheme,
            assertionKeyAlgorithm = certificate.assertionKeyAlgorithm,
            assertionPublicKey = certificate.assertionPublicKey(),
            assertionUsageCountLimit = certificate.assertionUsageCountLimit,
            oneUse = certificate.oneUse,
            issuerSignature = signature,
        )
    }

    private fun paymentTokenReplacingFirstOutputCertificate(
        token: OfflineNotePaymentToken,
        certificate: OfflineNote.KeyCertificate,
    ): OfflineNotePaymentToken {
        val outputClaims = token.audit.outputClaims.toMutableList()
        val output = outputClaims.first()
        outputClaims[0] = OfflineNote.AuditOutputClaim(
            noteCommitment = output.noteCommitment(),
            keyCertificate = certificate,
            assetId = output.assetId,
            amount = output.amount,
        )
        return paymentTokenReplacingAuditClaims(token, token.audit.inputClaims, outputClaims)
    }

    private fun paymentTokenReplacingFirstOutputAmount(
        token: OfflineNotePaymentToken,
        amount: String,
    ): OfflineNotePaymentToken {
        val outputClaims = token.audit.outputClaims.toMutableList()
        val output = outputClaims.first()
        outputClaims[0] = OfflineNote.AuditOutputClaim(
            noteCommitment = output.noteCommitment(),
            keyCertificate = output.keyCertificate,
            assetId = output.assetId,
            amount = amount,
        )
        return paymentTokenReplacingAuditClaims(token, token.audit.inputClaims, outputClaims)
    }

    private fun paymentTokenReplacingFirstOutputAmountWithoutProofRebind(
        token: OfflineNotePaymentToken,
        amount: String,
    ): OfflineNotePaymentToken {
        val outputClaims = token.audit.outputClaims.toMutableList()
        val output = outputClaims.first()
        outputClaims[0] = OfflineNote.AuditOutputClaim(
            noteCommitment = output.noteCommitment(),
            keyCertificate = output.keyCertificate,
            assetId = output.assetId,
            amount = amount,
        )
        return OfflineNotePaymentToken(
            chainId = token.chainId,
            paymentRequestId = token.paymentRequestId,
            tokenNonce = token.tokenNonce(),
            tokenId = token.tokenId(),
            audit = OfflineNote.AuditBundle(
                tokenId = token.audit.tokenId(),
                senderKeyCertificate = token.audit.senderKeyCertificate,
                inputNullifiers = token.audit.inputNullifiers(),
                inputClaims = token.audit.inputClaims,
                outputCommitments = token.audit.outputCommitments(),
                outputClaims = outputClaims,
                recursiveProof = token.audit.recursiveProof,
            ),
            createdAtMs = token.createdAtMs,
        )
    }

    private fun paymentTokenReplacingFirstOutputAsset(
        token: OfflineNotePaymentToken,
        assetId: String,
    ): OfflineNotePaymentToken {
        val outputClaims = token.audit.outputClaims.toMutableList()
        val output = outputClaims.first()
        outputClaims[0] = OfflineNote.AuditOutputClaim(
            noteCommitment = output.noteCommitment(),
            keyCertificate = output.keyCertificate,
            assetId = assetId,
            amount = output.amount,
        )
        return paymentTokenReplacingAuditClaims(token, token.audit.inputClaims, outputClaims)
    }

    private fun paymentTokenReversingOutputs(
        token: OfflineNotePaymentToken,
    ): OfflineNotePaymentToken =
        paymentTokenReplacingOutputs(
            token,
            outputClaims = token.audit.outputClaims.reversed(),
            outputCommitments = token.audit.outputCommitments().reversed(),
        )

    private fun paymentTokenDroppingFirstOutput(
        token: OfflineNotePaymentToken,
    ): OfflineNotePaymentToken =
        paymentTokenReplacingOutputs(
            token,
            outputClaims = token.audit.outputClaims.drop(1),
            outputCommitments = token.audit.outputCommitments().drop(1),
        )

    private fun paymentTokenReplacingChainId(
        token: OfflineNotePaymentToken,
        chainId: String,
    ): OfflineNotePaymentToken =
        OfflineNotePaymentToken(
            chainId = chainId,
            paymentRequestId = token.paymentRequestId,
            tokenNonce = token.tokenNonce(),
            tokenId = token.tokenId(),
            audit = token.audit,
            createdAtMs = token.createdAtMs,
        )

    private fun paymentTokenReplacingLastOutputCertificate(
        token: OfflineNotePaymentToken,
        certificate: OfflineNote.KeyCertificate,
    ): OfflineNotePaymentToken {
        val outputClaims = token.audit.outputClaims.toMutableList()
        val output = outputClaims.last()
        outputClaims[outputClaims.lastIndex] = OfflineNote.AuditOutputClaim(
            noteCommitment = output.noteCommitment(),
            keyCertificate = certificate,
            assetId = output.assetId,
            amount = output.amount,
        )
        return paymentTokenReplacingAuditClaims(token, token.audit.inputClaims, outputClaims)
    }

    private fun paymentTokenReplacingFirstInputClaimHash(
        token: OfflineNotePaymentToken,
        keyCertificatePayloadHash: ByteArray,
    ): OfflineNotePaymentToken {
        val inputClaims = token.audit.inputClaims.toMutableList()
        val input = inputClaims.first()
        inputClaims[0] = OfflineNote.IssuedClaim(
            domain = input.domain,
            noteCommitment = input.noteCommitment(),
            keyCertificatePayloadHash = keyCertificatePayloadHash,
            assetId = input.assetId,
            amount = input.amount,
        )
        return paymentTokenReplacingAuditClaims(token, inputClaims, token.audit.outputClaims)
    }

    private fun paymentTokenReplacingSenderCertificate(
        token: OfflineNotePaymentToken,
        certificate: OfflineNote.KeyCertificate,
    ): OfflineNotePaymentToken {
        val certificateHash = certificate.payloadHash()
        val inputClaims = token.audit.inputClaims.map { input ->
            OfflineNote.IssuedClaim(
                domain = input.domain,
                noteCommitment = input.noteCommitment(),
                keyCertificatePayloadHash = certificateHash,
                assetId = input.assetId,
                amount = input.amount,
            )
        }
        val tokenId = OfflineNote.derivePaymentTokenId(
            OfflineNote.PaymentTokenIdPreimage(
                chainId = token.chainId,
                paymentRequestId = token.paymentRequestId,
                createdAtMs = token.createdAtMs,
                tokenNonce = token.tokenNonce(),
                senderKeyCertificatePayloadHash = certificateHash,
                inputNullifiers = token.audit.inputNullifiers(),
                outputCommitments = token.audit.outputCommitments(),
            )
        )
        val draft = OfflineNote.AuditBundle(
            tokenId = tokenId,
            senderKeyCertificate = certificate,
            inputNullifiers = token.audit.inputNullifiers(),
            inputClaims = inputClaims,
            outputCommitments = token.audit.outputCommitments(),
            outputClaims = token.audit.outputClaims,
            recursiveProof = token.audit.recursiveProof,
        )
        val proof = OfflineNote.RecursiveProof(
            verifierKeyId = token.audit.recursiveProof.verifierKeyId,
            publicInputsHash = draft.publicInputsHash(),
            proof = token.audit.recursiveProof.proof,
        )
        return OfflineNotePaymentToken(
            chainId = token.chainId,
            paymentRequestId = token.paymentRequestId,
            tokenNonce = token.tokenNonce(),
            tokenId = tokenId,
            audit = draft.replacingRecursiveProof(proof),
            createdAtMs = token.createdAtMs,
        )
    }

    private fun paymentTokenReplacingPaymentRequestId(
        token: OfflineNotePaymentToken,
        paymentRequestId: String,
    ): OfflineNotePaymentToken {
        val tokenId = OfflineNote.derivePaymentTokenId(
            OfflineNote.PaymentTokenIdPreimage(
                chainId = token.chainId,
                paymentRequestId = paymentRequestId,
                createdAtMs = token.createdAtMs,
                tokenNonce = token.tokenNonce(),
                senderKeyCertificatePayloadHash = token.audit.senderKeyCertificate.payloadHash(),
                inputNullifiers = token.audit.inputNullifiers(),
                outputCommitments = token.audit.outputCommitments(),
            )
        )
        val draft = OfflineNote.AuditBundle(
            tokenId = tokenId,
            senderKeyCertificate = token.audit.senderKeyCertificate,
            inputNullifiers = token.audit.inputNullifiers(),
            inputClaims = token.audit.inputClaims,
            outputCommitments = token.audit.outputCommitments(),
            outputClaims = token.audit.outputClaims,
            recursiveProof = token.audit.recursiveProof,
        )
        val proof = OfflineNote.RecursiveProof(
            verifierKeyId = token.audit.recursiveProof.verifierKeyId,
            publicInputsHash = draft.publicInputsHash(),
            proof = token.audit.recursiveProof.proof,
        )
        return OfflineNotePaymentToken(
            chainId = token.chainId,
            paymentRequestId = paymentRequestId,
            tokenNonce = token.tokenNonce(),
            tokenId = tokenId,
            audit = draft.replacingRecursiveProof(proof),
            createdAtMs = token.createdAtMs,
        )
    }

    private fun paymentTokenReplacingTopLevelTokenId(
        token: OfflineNotePaymentToken,
    ): OfflineNotePaymentToken =
        OfflineNotePaymentToken(
            chainId = token.chainId,
            paymentRequestId = token.paymentRequestId,
            tokenNonce = token.tokenNonce(),
            tokenId = flippedHash(token.tokenId()),
            audit = token.audit,
            createdAtMs = token.createdAtMs,
        )

    private fun paymentTokenReplacingAuditTokenId(
        token: OfflineNotePaymentToken,
    ): OfflineNotePaymentToken {
        val auditTokenId = flippedHash(token.audit.tokenId())
        val draft = OfflineNote.AuditBundle(
            tokenId = auditTokenId,
            senderKeyCertificate = token.audit.senderKeyCertificate,
            inputNullifiers = token.audit.inputNullifiers(),
            inputClaims = token.audit.inputClaims,
            outputCommitments = token.audit.outputCommitments(),
            outputClaims = token.audit.outputClaims,
            recursiveProof = token.audit.recursiveProof,
        )
        val proof = OfflineNote.RecursiveProof(
            verifierKeyId = token.audit.recursiveProof.verifierKeyId,
            publicInputsHash = draft.publicInputsHash(),
            proof = token.audit.recursiveProof.proof,
        )
        return OfflineNotePaymentToken(
            chainId = token.chainId,
            paymentRequestId = token.paymentRequestId,
            tokenNonce = token.tokenNonce(),
            tokenId = token.tokenId(),
            audit = draft.replacingRecursiveProof(proof),
            createdAtMs = token.createdAtMs,
        )
    }

    private fun paymentTokenReplacingBearerAuditTrail(
        token: OfflineNotePaymentToken,
        bearerAuditTrail: List<OfflineNote.AuditBundle>,
    ): OfflineNotePaymentToken =
        OfflineNotePaymentToken(
            chainId = token.chainId,
            paymentRequestId = token.paymentRequestId,
            tokenNonce = token.tokenNonce(),
            tokenId = token.tokenId(),
            audit = token.audit,
            bearerAuditTrail = bearerAuditTrail,
            createdAtMs = token.createdAtMs,
        )

    private fun auditReplacingTokenIdWithoutProofRebind(
        audit: OfflineNote.AuditBundle,
    ): OfflineNote.AuditBundle =
        OfflineNote.AuditBundle(
            tokenId = flippedHash(audit.tokenId()),
            senderKeyCertificate = audit.senderKeyCertificate,
            inputNullifiers = audit.inputNullifiers(),
            inputClaims = audit.inputClaims,
            outputCommitments = audit.outputCommitments(),
            outputClaims = audit.outputClaims,
            recursiveProof = audit.recursiveProof,
        )

    private fun auditReplacingFirstOutputAmountWithoutProofRebind(
        audit: OfflineNote.AuditBundle,
        amount: String,
    ): OfflineNote.AuditBundle {
        val outputClaims = audit.outputClaims.toMutableList()
        val output = outputClaims.first()
        outputClaims[0] = OfflineNote.AuditOutputClaim(
            noteCommitment = output.noteCommitment(),
            keyCertificate = output.keyCertificate,
            assetId = output.assetId,
            amount = amount,
        )
        return OfflineNote.AuditBundle(
            tokenId = flippedHash(audit.tokenId()),
            senderKeyCertificate = audit.senderKeyCertificate,
            inputNullifiers = audit.inputNullifiers().map(::flippedHash),
            inputClaims = audit.inputClaims,
            outputCommitments = audit.outputCommitments(),
            outputClaims = outputClaims,
            recursiveProof = audit.recursiveProof,
        )
    }

    private fun selfConsumingAudit(audit: OfflineNote.AuditBundle): OfflineNote.AuditBundle {
        val firstInput = audit.inputClaims.first()
        val firstOutput = audit.outputClaims.first()
        val replacementOutputCommitment = flippedHash(firstOutput.noteCommitment())
        val inputClaims = audit.inputClaims.toMutableList()
        inputClaims[0] = OfflineNote.IssuedClaim(
            domain = firstInput.domain,
            noteCommitment = replacementOutputCommitment,
            keyCertificatePayloadHash = firstInput.keyCertificatePayloadHash(),
            assetId = firstInput.assetId,
            amount = firstInput.amount,
        )
        val outputClaims = audit.outputClaims.toMutableList()
        outputClaims[0] = OfflineNote.AuditOutputClaim(
            noteCommitment = replacementOutputCommitment,
            keyCertificate = firstOutput.keyCertificate,
            assetId = firstOutput.assetId,
            amount = firstOutput.amount,
        )
        val outputCommitments = audit.outputCommitments().toMutableList()
        outputCommitments[0] = replacementOutputCommitment
        val draft = OfflineNote.AuditBundle(
            tokenId = flippedHash(audit.tokenId()),
            senderKeyCertificate = audit.senderKeyCertificate,
            inputNullifiers = audit.inputNullifiers().map(::flippedHash),
            inputClaims = inputClaims,
            outputCommitments = outputCommitments,
            outputClaims = outputClaims,
            recursiveProof = audit.recursiveProof,
        )
        return draft.replacingRecursiveProof(
            OfflineNote.RecursiveProof(
                publicInputsHash = draft.publicInputsHash(),
                proof = OfflineNote.ProofBox(
                    OfflineNote.RECURSIVE_BACKEND,
                    "self-consuming-audit-proof".toByteArray(),
                ),
            )
        )
    }

    private fun paymentTokenReplacingOutputs(
        token: OfflineNotePaymentToken,
        outputClaims: List<OfflineNote.AuditOutputClaim>,
        outputCommitments: List<ByteArray>,
    ): OfflineNotePaymentToken {
        val tokenId = OfflineNote.derivePaymentTokenId(
            OfflineNote.PaymentTokenIdPreimage(
                chainId = token.chainId,
                paymentRequestId = token.paymentRequestId,
                createdAtMs = token.createdAtMs,
                tokenNonce = token.tokenNonce(),
                senderKeyCertificatePayloadHash = token.audit.senderKeyCertificate.payloadHash(),
                inputNullifiers = token.audit.inputNullifiers(),
                outputCommitments = outputCommitments,
            )
        )
        val draft = OfflineNote.AuditBundle(
            tokenId = tokenId,
            senderKeyCertificate = token.audit.senderKeyCertificate,
            inputNullifiers = token.audit.inputNullifiers(),
            inputClaims = token.audit.inputClaims,
            outputCommitments = outputCommitments,
            outputClaims = outputClaims,
            recursiveProof = token.audit.recursiveProof,
        )
        val proof = OfflineNote.RecursiveProof(
            verifierKeyId = token.audit.recursiveProof.verifierKeyId,
            publicInputsHash = draft.publicInputsHash(),
            proof = token.audit.recursiveProof.proof,
        )
        return OfflineNotePaymentToken(
            chainId = token.chainId,
            paymentRequestId = token.paymentRequestId,
            tokenNonce = token.tokenNonce(),
            tokenId = tokenId,
            audit = draft.replacingRecursiveProof(proof),
            createdAtMs = token.createdAtMs,
        )
    }

    private fun flippedHash(hash: ByteArray): ByteArray {
        val copy = hash.copyOf()
        copy[0] = (copy[0].toInt() xor 0x01).toByte()
        return copy
    }

    private fun paymentTokenReplacingAuditClaims(
        token: OfflineNotePaymentToken,
        inputClaims: List<OfflineNote.IssuedClaim>,
        outputClaims: List<OfflineNote.AuditOutputClaim>,
    ): OfflineNotePaymentToken {
        val draft = OfflineNote.AuditBundle(
            tokenId = token.audit.tokenId(),
            senderKeyCertificate = token.audit.senderKeyCertificate,
            inputNullifiers = token.audit.inputNullifiers(),
            inputClaims = inputClaims,
            outputCommitments = token.audit.outputCommitments(),
            outputClaims = outputClaims,
            recursiveProof = token.audit.recursiveProof,
        )
        val proof = OfflineNote.RecursiveProof(
            verifierKeyId = token.audit.recursiveProof.verifierKeyId,
            publicInputsHash = draft.publicInputsHash(),
            proof = token.audit.recursiveProof.proof,
        )
        return OfflineNotePaymentToken(
            chainId = token.chainId,
            paymentRequestId = token.paymentRequestId,
            tokenNonce = token.tokenNonce(),
            tokenId = token.tokenId(),
            audit = draft.replacingRecursiveProof(proof),
            createdAtMs = token.createdAtMs,
        )
    }

    private fun issuedClaim(json: Map<String, Any?>): OfflineNote.IssuedClaim =
        OfflineNote.IssuedClaim(
            domain = string(json, "domain"),
            noteCommitment = hexBytes(string(json, "note_commitment")),
            keyCertificatePayloadHash = hexBytes(string(json, "key_certificate_payload_hash")),
            assetId = string(json, "asset_id"),
            amount = string(json, "amount"),
        )

    private fun auditOutputClaim(json: Map<String, Any?>): OfflineNote.AuditOutputClaim =
        OfflineNote.AuditOutputClaim(
            noteCommitment = hexBytes(string(json, "note_commitment")),
            keyCertificate = certificate(obj(json, "key_certificate")),
            assetId = "${string(json, "asset_definition_id")}#${string(json, "account_id")}",
            amount = string(json, "amount"),
        )

    private fun sourceWalletNote(
        fixture: Map<String, Any?>,
        certificate: OfflineNote.KeyCertificate,
    ): OfflineNoteWalletNote {
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val issue = obj(chain, "issue")
        return OfflineNoteWalletNote(
            chainId = string(derivation, "chain_id"),
            accountId = accountFromAssetId(string(issue, "asset_id")),
            assetId = string(issue, "asset_id"),
            amount = string(issue, "amount"),
            keyCertificate = certificate,
            noteCommitment = hexBytes(string(derivation, "source_note_commitment")),
            noteSecret = hexBytes(string(derivation, "source_note_secret_hex")),
            origin = OfflineNote.CommitmentOrigin.IssuerLoad(
                operationId = string(derivation, "issuer_load_operation_id"),
                lineageId = string(derivation, "issuer_load_lineage_id"),
                localRevision = long(derivation, "issuer_load_local_revision"),
            ),
            state = OfflineNoteWalletNoteState.SPENDABLE,
            createdAtMs = 1_700_000_000_000L,
            updatedAtMs = 1_700_000_000_000L,
        )
    }

    private fun issuerSourceWalletNote(
        chainId: String,
        accountId: String,
        assetDefinitionId: String,
        amount: String,
        issuerCertificate: OfflineNote.KeyCertificate,
        noteSecret: ByteArray,
        operationSuffix: String,
        createdAtMs: Long,
    ): OfflineNoteWalletNote {
        val assetId = "$assetDefinitionId#$accountId"
        val origin = OfflineNote.CommitmentOrigin.IssuerLoad(
            operationId = "operation-$operationSuffix",
            lineageId = "lineage-$operationSuffix",
            localRevision = 1L,
        )
        val noteCommitment = OfflineNote.deriveNoteCommitment(
            OfflineNote.NoteCommitmentPreimage(
                chainId = chainId,
                ownerKeyCertificatePayloadHash = issuerCertificate.payloadHash(),
                assetId = assetId,
                amount = amount,
                noteSecret = noteSecret,
                origin = origin,
            )
        )
        return OfflineNoteWalletNote(
            chainId = chainId,
            accountId = accountId,
            assetId = assetId,
            amount = amount,
            keyCertificate = issuerCertificate,
            noteCommitment = noteCommitment,
            noteSecret = noteSecret,
            origin = origin,
            state = OfflineNoteWalletNoteState.SPENDABLE,
            createdAtMs = createdAtMs,
            updatedAtMs = createdAtMs,
        )
    }

    private class StaticAttestationProvider(
        private val certificate: OfflineNote.KeyCertificate,
    ) : OfflineNoteAttestationProvider {
        override fun currentKeyCertificate(): OfflineNote.KeyCertificate = certificate
    }

    /**
     * Mints owner-self-signed Offline Note key certificates for an account id the test controls.
     *
     * The signer owns the Ed25519 keypair behind [accountId], so the `issuerSignature` it produces
     * is a RAW Ed25519 signature (no Blake2b prehash) over [OfflineNote.KeyCertificate.signingBytes]
     * that [Ed25519OfflineNoteCertificateVerifier.verifyOwnerCertificate] accepts: that verifier
     * re-derives the owner public key from the certificate `accountId` and checks the signature
     * against it. Each call generates a fresh throwaway `publicKey` so successive certificates differ.
     */
    private class TestOwnerCertificateSigner : OfflineNoteOwnerCertificateSigner {
        private val ownerKey: Ed25519PrivateKeyParameters
        private val ownerPublicKey: ByteArray
        val accountId: String
        private val freshKeyGenerator = Ed25519KeyPairGenerator().apply {
            init(Ed25519KeyGenerationParameters(SecureRandom()))
        }

        init {
            val generator = Ed25519KeyPairGenerator().apply {
                init(Ed25519KeyGenerationParameters(SecureRandom()))
            }
            val pair = generator.generateKeyPair()
            ownerKey = pair.private as Ed25519PrivateKeyParameters
            ownerPublicKey = (pair.public as Ed25519PublicKeyParameters).encoded
            accountId = AccountAddress.fromAccount(ownerPublicKey, "ed25519")
                .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        }

        override fun freshOwnerCertificate(accountId: String): OfflineNote.KeyCertificate {
            require(accountId == this.accountId) {
                "test owner signer only mints certificates for its own account id"
            }
            val freshPublicKey = (freshKeyGenerator.generateKeyPair().public as Ed25519PublicKeyParameters).encoded
            return signCertificate(
                ownerKey,
                accountId = accountId,
                publicKey = freshPublicKey,
                assertionPublicKey = ownerPublicKey,
                assertionScheme = "self-signed",
                platform = "android-software-ed25519",
                keyId = "owner-key-${UUID.randomUUID()}",
            )
        }
    }

    /**
     * Mints issuer-signed Offline Note key certificates for a wallet account the test controls.
     *
     * Mirrors the topup/issue path: the certificate `accountId` is the wallet account, while the
     * `issuerSignature` is a RAW Ed25519 signature by this test issuer (financial institution) key.
     * Build a wallet verifier with [Ed25519OfflineNoteCertificateVerifier] trusting [publicKey] so
     * source notes minted here pass [Ed25519OfflineNoteCertificateVerifier.verifyIssuerCertificate].
     */
    private class TestIssuerCertificateSigner {
        private val issuerKey: Ed25519PrivateKeyParameters
        val publicKey: ByteArray
        private val freshKeyGenerator = Ed25519KeyPairGenerator().apply {
            init(Ed25519KeyGenerationParameters(SecureRandom()))
        }

        init {
            val generator = Ed25519KeyPairGenerator().apply {
                init(Ed25519KeyGenerationParameters(SecureRandom()))
            }
            val pair = generator.generateKeyPair()
            issuerKey = pair.private as Ed25519PrivateKeyParameters
            publicKey = (pair.public as Ed25519PublicKeyParameters).encoded
        }

        fun issuerCertificate(accountId: String): OfflineNote.KeyCertificate {
            val freshPublicKey = (freshKeyGenerator.generateKeyPair().public as Ed25519PublicKeyParameters).encoded
            return signCertificate(
                issuerKey,
                accountId = accountId,
                publicKey = freshPublicKey,
                assertionPublicKey = freshPublicKey,
                assertionScheme = "issuer-attested",
                platform = "test-issuer",
                keyId = "issuer-key-${UUID.randomUUID()}",
            )
        }
    }

    private class QueueRandomSource(
        private val values: List<ByteArray>,
    ) : OfflineNoteRandomSource {
        private var index = 0

        override fun nextBytes(length: Int): ByteArray {
            require(index < values.size) { "test random source exhausted" }
            val value = values[index++]
            require(value.size == length) { "test random source returned ${value.size} bytes" }
            return value.copyOf()
        }
    }

    private class FixedIdGenerator(
        private val id: String,
    ) : OfflineNoteIdGenerator {
        override fun nextId(prefix: String): String = id
    }

    private class SequenceIdGenerator(
        private vararg val ids: String,
    ) : OfflineNoteIdGenerator {
        private var index = 0

        override fun nextId(prefix: String): String {
            require(index < ids.size) { "test id generator exhausted" }
            return ids[index++]
        }
    }

    private inner class OfflineIssuerExecutor(
        private val certificateJson: Map<String, Any?>,
        private val serverStateHash: String? = null,
    ) : HttpTransportExecutor {
        val requests = ArrayList<TransportRequest>()

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requests.add(request)
            val body = requestBody(request)
            val response = when (request.uri.path) {
                "/v1/offline/keys/refill" -> linkedMapOf<String, Any?>(
                    "operation_id" to string(body, "operation_id"),
                    "lineage_state" to lineageState(0, "0"),
                    "key_certificate" to certificateWithExpiry(),
                    "key_certificates" to listOf(certificateWithExpiry()),
                )
                "/v1/offline/notes/issue" -> linkedMapOf<String, Any?>(
                    "operation_id" to string(body, "operation_id"),
                    "settlement" to linkedMapOf("entry_hash" to "settlement-entry-hash"),
                    "lineage_state" to lineageState(1, "5"),
                    "local_balance" to "5",
                    "locked_balance" to "0",
                    "local_revision" to 1L,
                    "local_state_hash" to "lineage-state-hash",
                    "issued_note_commitment" to string(body, "note_commitment"),
                    "key_certificate" to certificateWithExpiry(),
                    "key_certificates" to listOf(certificateWithExpiry()),
                )
                else -> throw IllegalStateException("unexpected path ${request.uri.path}")
            }
            return CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(200)
                    .setBody(JsonEncoder.encode(response).toByteArray(StandardCharsets.UTF_8))
                    .build()
            )
        }

        fun requestBody(index: Int): Map<String, Any?> = requestBody(requests[index])

        private fun requestBody(request: TransportRequest): Map<String, Any?> {
            @Suppress("UNCHECKED_CAST")
            return JsonParser.parse(String(request.body, StandardCharsets.UTF_8)) as Map<String, Any?>
        }

        private fun certificateWithExpiry(): Map<String, Any?> {
            val copy = LinkedHashMap(certificateJson)
            copy["expires_at_ms"] = 1_700_000_060_000L
            return copy
        }

        private fun lineageState(revision: Long, balance: String): Map<String, Any?> {
            val state = linkedMapOf<String, Any?>(
                "lineage_id" to "lineage-1",
                "server_revision" to revision,
                "pending_local_revision" to revision,
                "balance" to balance,
                "locked_balance" to "0",
                "authorization" to linkedMapOf(
                    "expires_at_ms" to 1_700_000_060_000L,
                ),
            )
            if (serverStateHash != null) {
                state["server_state_hash"] = serverStateHash
            }
            return state
        }
    }

    private object BindingProofProvider : OfflineNoteProofProvider {
        override fun proveAudit(audit: OfflineNote.AuditBundle): OfflineNote.RecursiveProof {
            return OfflineNote.RecursiveProof(
                publicInputsHash = audit.publicInputsHash(),
                proof = OfflineNote.ProofBox(
                    OfflineNote.RECURSIVE_BACKEND,
                    "wallet-audit-proof".toByteArray(),
                ),
            )
        }

        override fun proveRedeem(redemption: OfflineNote.Redeem): OfflineNote.RecursiveProof {
            return OfflineNote.RecursiveProof(
                publicInputsHash = redemption.publicInputsHash(),
                proof = OfflineNote.ProofBox(
                    OfflineNote.RECURSIVE_BACKEND,
                    "wallet-redeem-proof".toByteArray(),
                ),
            )
        }
    }

    private object BindingProofVerifier : OfflineNoteProofVerifier {
        override fun verifyAudit(audit: OfflineNote.AuditBundle): Boolean =
            audit.recursiveProof.publicInputsHash().contentEquals(audit.publicInputsHash())

        override fun verifyRedeem(redemption: OfflineNote.Redeem): Boolean =
            redemption.recursiveProof.publicInputsHash().contentEquals(redemption.publicInputsHash())
    }

    private class RecordingIssuerClient(
        private val loadContext: OfflineNoteLoadContext,
    ) : OfflineNoteIssuerClient {
        var lastIssueRequest: OfflineNoteIssueRequest? = null

        override fun prepareLoad(
            chainId: String,
            accountId: String,
            assetDefinitionId: String,
            amount: String,
        ): CompletableFuture<OfflineNoteLoadContext> = CompletableFuture.completedFuture(loadContext)

        override fun issueNote(request: OfflineNoteIssueRequest): CompletableFuture<OfflineNoteIssueResponse> {
            lastIssueRequest = request
            return CompletableFuture.completedFuture(
                OfflineNoteIssueResponse(
                    noteCommitment = request.noteCommitment(),
                    operationId = request.loadContext.operationId,
                    lineageId = request.loadContext.lineageId,
                    localRevision = request.loadContext.localRevision,
                    keyCertificate = request.loadContext.keyCertificate,
                    settlementEntryHashHex = "settlement-entry-hash",
                )
            )
        }
    }

    private class CompletionControlledIssuerClient(
        private val loadContext: OfflineNoteLoadContext,
    ) : OfflineNoteIssuerClient {
        val issueRequested = CountDownLatch(1)
        val issueFuture = CompletableFuture<OfflineNoteIssueResponse>()

        @Volatile
        var lastIssueRequest: OfflineNoteIssueRequest? = null

        override fun prepareLoad(
            chainId: String,
            accountId: String,
            assetDefinitionId: String,
            amount: String,
        ): CompletableFuture<OfflineNoteLoadContext> = CompletableFuture.completedFuture(loadContext)

        override fun issueNote(request: OfflineNoteIssueRequest): CompletableFuture<OfflineNoteIssueResponse> {
            lastIssueRequest = request
            issueRequested.countDown()
            return issueFuture
        }
    }

    private class SynchronouslyThrowingIssuerClient(
        private val loadContext: OfflineNoteLoadContext,
    ) : OfflineNoteIssuerClient {
        override fun prepareLoad(
            chainId: String,
            accountId: String,
            assetDefinitionId: String,
            amount: String,
        ): CompletableFuture<OfflineNoteLoadContext> = CompletableFuture.completedFuture(loadContext)

        override fun issueNote(request: OfflineNoteIssueRequest): CompletableFuture<OfflineNoteIssueResponse> {
            throw IllegalStateException("issuer exploded")
        }
    }

    private class BlockingOfflineNoteStore : OfflineNoteStore {
        private val notes = LinkedHashMap<String, OfflineNoteWalletNote>()
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)

        @Synchronized
        override fun <T> mutateNotes(mutator: (MutableMap<String, OfflineNoteWalletNote>) -> T): T {
            entered.countDown()
            check(release.await(5, TimeUnit.SECONDS)) {
                "timed out waiting to release blocked note store"
            }
            return mutator(notes)
        }
    }

    private class RecordingTransactionSubmitter : OfflineNoteTransactionSubmitter {
        val audits = ArrayList<OfflineNote.AuditBundle>()
        val redemptions = ArrayList<OfflineNote.Redeem>()
        val defunds = ArrayList<Pair<OfflineNote.Redeem, List<OfflineNote.AuditBundle>>>()

        override fun submitAudit(audit: OfflineNote.AuditBundle): CompletableFuture<ClientResponse> {
            audits.add(audit)
            return CompletableFuture.completedFuture(ClientResponse(202, byteArrayOf(), "accepted"))
        }

        override fun submitRedeem(redemption: OfflineNote.Redeem): CompletableFuture<ClientResponse> {
            redemptions.add(redemption)
            return CompletableFuture.completedFuture(ClientResponse(202, byteArrayOf(), "accepted"))
        }

        override fun submitDefund(
            redemption: OfflineNote.Redeem,
            bearerAuditTrail: List<OfflineNote.AuditBundle>,
        ): CompletableFuture<ClientResponse> {
            defunds.add(redemption to bearerAuditTrail)
            return CompletableFuture.completedFuture(ClientResponse(202, byteArrayOf(), "accepted"))
        }
    }

    private class CapturingIrohaClient : IrohaClient {
        var submittedTransaction: SignedTransaction? = null

        override fun submitTransaction(transaction: SignedTransaction): CompletableFuture<ClientResponse> {
            submittedTransaction = transaction
            return CompletableFuture.completedFuture(ClientResponse(202, byteArrayOf(), "accepted"))
        }
    }

    private class FakeSigner : Signer {
        override fun sign(message: ByteArray): ByteArray =
            message + "-signature".toByteArray()

        override fun publicKey(): ByteArray =
            "fake-public-key".toByteArray()

        override fun algorithm(): String = "Ed25519"
    }

    private class RejectingTransactionSubmitter : OfflineNoteTransactionSubmitter {
        override fun submitAudit(audit: OfflineNote.AuditBundle): CompletableFuture<ClientResponse> =
            CompletableFuture.completedFuture(ClientResponse(409, byteArrayOf(), "rejected"))

        override fun submitRedeem(redemption: OfflineNote.Redeem): CompletableFuture<ClientResponse> =
            CompletableFuture.completedFuture(ClientResponse(409, byteArrayOf(), "rejected"))

        override fun submitDefund(
            redemption: OfflineNote.Redeem,
            bearerAuditTrail: List<OfflineNote.AuditBundle>,
        ): CompletableFuture<ClientResponse> =
            CompletableFuture.completedFuture(ClientResponse(409, byteArrayOf(), "rejected"))
    }

    private class PendingDefundTransactionSubmitter : OfflineNoteTransactionSubmitter {
        val defunds = ArrayList<Pair<OfflineNote.Redeem, List<OfflineNote.AuditBundle>>>()
        private val pending = CompletableFuture<ClientResponse>()

        override fun submitAudit(audit: OfflineNote.AuditBundle): CompletableFuture<ClientResponse> =
            CompletableFuture.completedFuture(ClientResponse(202, byteArrayOf(), "accepted"))

        override fun submitRedeem(redemption: OfflineNote.Redeem): CompletableFuture<ClientResponse> =
            CompletableFuture.completedFuture(ClientResponse(202, byteArrayOf(), "accepted"))

        override fun submitDefund(
            redemption: OfflineNote.Redeem,
            bearerAuditTrail: List<OfflineNote.AuditBundle>,
        ): CompletableFuture<ClientResponse> {
            defunds.add(redemption to bearerAuditTrail)
            return pending
        }

        fun completeAccepted() {
            pending.complete(ClientResponse(202, byteArrayOf(), "accepted"))
        }
    }

    private class RecordingSyncResolver(
        private val resolutions: Map<String, OfflineNoteWalletNoteState>,
    ) : OfflineNoteSyncResolver {
        val resolvedCommitments = ArrayList<String>()

        override fun resolvePendingNote(
            note: OfflineNoteWalletNote,
        ): CompletableFuture<OfflineNoteSyncResolution?> {
            val commitment = note.noteCommitmentHex()
            resolvedCommitments.add(commitment)
            return CompletableFuture.completedFuture(
                resolutions[commitment]?.let { OfflineNoteSyncResolution(it, "tx-$commitment") }
            )
        }
    }

    private fun wirePayloadBytes(instruction: org.hyperledger.iroha.sdk.core.model.InstructionBox): ByteArray =
        (instruction.payload as WirePayload).payloadBytes

    private fun rawInstructionPair(wireName: String, wirePayload: ByteArray, compact: Boolean = true): ByteArray {
        val out = ByteArrayOutputStream()
        writeField(out, encodeString(wireName, compact), compact)
        writeField(out, encodeBytesVec(wirePayload), compact)
        return out.toByteArray()
    }

    private fun rawVerifyingKeyBoxNorito(backend: String, bytes: ByteArray): ByteArray =
        rawVerifyingKeyBoxNoritoFields(
            encodeString(backend, compact = true),
            encodeBytesVec(bytes),
        )

    private fun rawVerifyingKeyBoxNoritoFields(
        backendFieldPayload: ByteArray,
        bytesFieldPayload: ByteArray,
    ): ByteArray {
        val adapter = object : TypeAdapter<Unit> {
            override fun encode(encoder: NoritoEncoder, value: Unit) {
                writeVerifyingKeyBoxRawField(encoder, backendFieldPayload)
                writeVerifyingKeyBoxRawField(encoder, bytesFieldPayload)
            }

            override fun decode(decoder: NoritoDecoder): Unit =
                throw AssertionError("raw VerifyingKeyBox test adapter is encode-only")
        }
        return NoritoCodec.encode(
            Unit,
            "iroha_data_model::proof::VerifyingKeyBox",
            adapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun writeVerifyingKeyBoxRawField(encoder: NoritoEncoder, payload: ByteArray) {
        val compact = encoder.flags and NoritoHeader.COMPACT_LEN != 0
        encoder.writeLength(payload.size.toLong(), compact)
        encoder.writeBytes(payload)
    }

    private fun encodeString(value: String, compact: Boolean): ByteArray {
        val bytes = value.toByteArray(StandardCharsets.UTF_8)
        val out = ByteArrayOutputStream()
        writeLength(out, bytes.size.toLong(), compact)
        out.write(bytes)
        return out.toByteArray()
    }

    private fun encodeBytesVec(value: ByteArray): ByteArray {
        val out = ByteArrayOutputStream()
        writeUInt64(out, value.size.toLong())
        out.write(value)
        return out.toByteArray()
    }

    private fun writeField(out: ByteArrayOutputStream, payload: ByteArray, compact: Boolean) {
        writeLength(out, payload.size.toLong(), compact)
        out.write(payload)
    }

    private fun writeLength(out: ByteArrayOutputStream, value: Long, compact: Boolean) {
        if (!compact) {
            writeUInt64(out, value)
            return
        }
        var remaining = value
        while (remaining >= 0x80) {
            out.write(((remaining and 0x7f) or 0x80).toInt())
            remaining = remaining ushr 7
        }
        out.write(remaining.toInt())
    }

    private fun writeUInt64(out: ByteArrayOutputStream, value: Long) {
        var remaining = value
        repeat(8) {
            out.write((remaining and 0xff).toInt())
            remaining = remaining ushr 8
        }
    }

    private fun loadFixture(): Map<String, Any?> {
        val path = Paths.get("..", "..", "fixtures", "offline", "interop_contract.json")
        val parsed = JsonParser.parse(String(Files.readAllBytes(path), Charsets.UTF_8))
        @Suppress("UNCHECKED_CAST")
        return parsed as Map<String, Any?>
    }

    private fun obj(map: Map<String, Any?>, key: String): Map<String, Any?> {
        @Suppress("UNCHECKED_CAST")
        return map[key] as Map<String, Any?>
    }

    private fun list(map: Map<String, Any?>, key: String): List<Any?> {
        @Suppress("UNCHECKED_CAST")
        return map[key] as List<Any?>
    }

    private fun string(map: Map<String, Any?>, key: String): String = map[key] as String
    private fun bool(map: Map<String, Any?>, key: String): Boolean = map[key] as Boolean
    private fun int(map: Map<String, Any?>, key: String): Int = (map[key] as Number).toInt()
    private fun long(map: Map<String, Any?>, key: String): Long = (map[key] as Number).toLong()
    private fun nullableInt(map: Map<String, Any?>, key: String): Int? = (map[key] as Number?)?.toInt()

    private fun base64(bytes: ByteArray): String = Base64.getEncoder().encodeToString(bytes)
    private fun base64Bytes(value: String): ByteArray = Base64.getDecoder().decode(value)

    private fun hex(bytes: ByteArray): String =
        bytes.joinToString(separator = "") { "%02x".format(it.toInt() and 0xFF) }

    private fun mutatedHeaderFrame(
        header: OfflineQrStream.Frame,
        mutate: (ByteArray) -> ByteArray,
    ): ByteArray =
        OfflineQrStream.Frame(
            OfflineQrStream.FrameKind.HEADER,
            header.streamId(),
            header.index,
            header.total,
            mutate(header.payload()),
        ).encode()

    private fun writeUInt16LE(bytes: ByteArray, offset: Int, value: Int) {
        bytes[offset] = (value and 0xff).toByte()
        bytes[offset + 1] = ((value ushr 8) and 0xff).toByte()
    }

    private fun assetDefinitionFromAssetId(assetId: String): String = assetId.substringBefore('#')

    private fun accountFromAssetId(assetId: String): String =
        assetId.substringAfter('#').substringBefore("#dataspace:")

    private fun hashFromPublicValues(values: LongArray): ByteArray {
        val out = ByteArray(32)
        for (idx in 0 until 4) {
            var word = values[idx]
            for (offset in 0 until 8) {
                out[idx * 8 + offset] = (word and 0xffL).toByte()
                word = word ushr 8
            }
        }
        return out
    }

    private fun benchmarkSeconds(iterations: Int, body: () -> Unit): DoubleArray {
        val durations = DoubleArray(iterations)
        for (idx in 0 until iterations) {
            val start = System.nanoTime()
            body()
            durations[idx] = (System.nanoTime() - start).toDouble() / 1_000_000_000.0
        }
        return durations
    }

    private fun summary(values: DoubleArray): String {
        val sorted = values.sorted()
        if (sorted.isEmpty()) {
            return "empty"
        }
        val median = if (sorted.size % 2 == 0) {
            (sorted[sorted.size / 2 - 1] + sorted[sorted.size / 2]) / 2.0
        } else {
            sorted[sorted.size / 2]
        }
        val p95Index = minOf(sorted.size - 1, maxOf(0, kotlin.math.ceil(sorted.size * 0.95).toInt() - 1))
        return "median=%.3fs p95=%.3fs max=%.3fs n=%d".format(
            Locale.ROOT,
            median,
            sorted[p95Index],
            sorted.last(),
            sorted.size,
        )
    }

    private fun hexBytes(value: String): ByteArray {
        require(value.length % 2 == 0) { "hex length must be even" }
        val out = ByteArray(value.length / 2)
        var offset = 0
        while (offset < value.length) {
            out[offset / 2] = value.substring(offset, offset + 2).toInt(16).toByte()
            offset += 2
        }
        return out
    }

    private fun assertFutureFails(future: CompletableFuture<*>) {
        assertFailsWith<CompletionException> {
            future.join()
        }
    }

    private fun assertIllegalArgumentContains(
        expected: String,
        block: () -> Unit,
    ) {
        val error = assertFailsWith<IllegalArgumentException> {
            block()
        }
        assertTrue(
            error.message.orEmpty().contains(expected),
            "expected IllegalArgumentException to contain '$expected', actual: '${error.message}'",
        )
    }

    private fun kagemushaNoritoFrame(schemaByte: Int): ByteArray {
        val frame = ByteArray(40)
        frame[0] = 'N'.code.toByte()
        frame[1] = 'R'.code.toByte()
        frame[2] = 'T'.code.toByte()
        frame[3] = '0'.code.toByte()
        frame.fill(schemaByte.toByte(), 6, 22)
        return frame
    }

    private fun kagemushaNoritoFrameWithPayload(schemaByte: Int): ByteArray {
        val frame = ByteArray(45)
        kagemushaNoritoFrame(schemaByte).copyInto(frame, 0)
        frame[23] = 3.toByte()
        byteArrayOf(
            0xb9.toByte(),
            0xd3.toByte(),
            0xa8.toByte(),
            0x0c.toByte(),
            0xcd.toByte(),
            0x5d.toByte(),
            0x13.toByte(),
            0x24.toByte(),
        ).copyInto(frame, 31)
        frame[42] = 0xa5.toByte()
        frame[43] = 0x5a.toByte()
        frame[44] = 0x11.toByte()
        return frame
    }
}

private fun signCertificate(
    signingKey: Ed25519PrivateKeyParameters,
    accountId: String,
    publicKey: ByteArray,
    assertionPublicKey: ByteArray,
    assertionScheme: String,
    platform: String,
    keyId: String,
): OfflineNote.KeyCertificate {
    val unsigned = OfflineNote.KeyCertificate(
        platform = platform,
        keyId = keyId,
        deviceId = "test-device",
        accountId = accountId,
        publicKey = publicKey,
        assertionScheme = assertionScheme,
        assertionKeyAlgorithm = "ed25519",
        assertionPublicKey = assertionPublicKey,
        assertionUsageCountLimit = null,
        oneUse = true,
        issuerSignature = ByteArray(64),
    )
    val signer = Ed25519Signer()
    signer.init(true, signingKey)
    val message = unsigned.signingBytes()
    signer.update(message, 0, message.size)
    return OfflineNote.KeyCertificate(
        platform = unsigned.platform,
        keyId = unsigned.keyId,
        deviceId = unsigned.deviceId,
        accountId = unsigned.accountId,
        publicKey = unsigned.publicKey(),
        assertionScheme = unsigned.assertionScheme,
        assertionKeyAlgorithm = unsigned.assertionKeyAlgorithm,
        assertionPublicKey = unsigned.assertionPublicKey(),
        assertionUsageCountLimit = unsigned.assertionUsageCountLimit,
        oneUse = unsigned.oneUse,
        issuerSignature = signer.generateSignature(),
    )
}
