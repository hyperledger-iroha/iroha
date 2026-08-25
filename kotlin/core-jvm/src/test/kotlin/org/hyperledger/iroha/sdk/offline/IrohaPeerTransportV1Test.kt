package org.hyperledger.iroha.sdk.offline

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.hyperledger.iroha.sdk.crypto.Blake2b
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.junit.jupiter.api.Test
import java.io.ByteArrayOutputStream
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.zip.Deflater
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class IrohaPeerTransportV1Test {
    @Test
    fun `wire and QR custom limits cannot exceed V1 hard ceilings`() {
        IrohaPeerWireLimitsV1(32 * 1_024, 24_576)
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerWireLimitsV1(32 * 1_024 + 1, 24_576)
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerWireLimitsV1(32 * 1_024, 24_577)
        }

        IrohaPeerQRScanLimitsV1(3, 12, 3_072, 30_000, 180_000)
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerQRScanLimitsV1(4, 12, 3_072, 30_000, 180_000)
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerQRScanLimitsV1(3, 13, 3_072, 30_000, 180_000)
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerQRScanLimitsV1(3, 12, 3_073, 30_000, 180_000)
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerQRScanLimitsV1(3, 12, 3_072, 30_001, 180_000)
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerQRScanLimitsV1(3, 12, 3_072, 30_000, 180_001)
        }
    }

    @Test
    fun `implicit clock is sampled inside scanner serialization`() {
        val active = AtomicInteger()
        val maximum = AtomicInteger()
        val timestamp = AtomicInteger()
        val session = IrohaPeerQRScanSessionV1(clock = IrohaPeerQRClockV1 {
            val concurrent = active.incrementAndGet()
            maximum.updateAndGet { maxOf(it, concurrent) }
            Thread.sleep(5)
            active.decrementAndGet()
            timestamp.incrementAndGet().toLong()
        })
        val pool = Executors.newFixedThreadPool(8)
        val start = CountDownLatch(1)
        val finished = CountDownLatch(16)
        val failures = AtomicInteger()
        repeat(16) {
            pool.execute {
                start.await()
                try { session.expire() } catch (_: Throwable) { failures.incrementAndGet() }
                finished.countDown()
            }
        }
        start.countDown()
        assertTrue(finished.await(3, TimeUnit.SECONDS))
        pool.shutdownNow()
        assertEquals(0, failures.get())
        assertEquals(1, maximum.get())
    }

    @Test
    fun `default scanner clock uses a nonnegative captured-origin domain`() {
        val first = IrohaPeerQRMonotonicClockV1.nowMillis()
        assertTrue(first in 0 until 60_000)

        var previous = first
        repeat(256) {
            val current = IrohaPeerQRMonotonicClockV1.nowMillis()
            assertTrue(current >= 0)
            assertTrue(current >= previous)
            previous = current
        }

        // The default session consumes this same clock and must therefore be
        // safe for APIs that reject negative explicit timestamps.
        assertTrue(IrohaPeerQRScanSessionV1().expire().isEmpty())
    }

    @Test
    fun `scanner rejects explicit clock rollback until reset`() {
        val message = message("clock epoch")
        val text = IrohaPeerQRCodecV1.encode(message).single()
        val session = IrohaPeerQRScanSessionV1()
        assertTrue(session.expire(100).isEmpty())

        assertFailsWith<IllegalArgumentException> { session.ingestAt(text, 99) }
        assertFailsWith<IllegalArgumentException> { session.quarantine(message.streamId, 99) }
        assertFailsWith<IllegalArgumentException> { session.expire(99) }

        session.reset()
        assertEquals(message, session.ingestAt(text, 99).message)
    }

    @Test
    fun `Kagemusha adapter requires bridge ABI 22 and roundtrips exact bytes`() {
        assertEquals(0x0102, IrohaPeerKagemushaAdapterV1.NATIVE_ARCHIVE_SCHEMA_VERSION)
        val archive = portableOfferFixture()
        val nativeAvailable = KagemushaRecursiveSpendProver.isArtifactStreamingAvailable()
        assertTrue(
            nativeAvailable,
            "A freshly built connect_norito_bridge ABI 22 artifact-streaming library is required",
        )

        val typed = KagemushaPeerPayload.decode(
            archive,
            KagemushaPeerPayloadKind.RECEIVE_REQUEST,
        )
        val wrapped = IrohaPeerKagemushaAdapterV1.wrap(typed)
        assertEquals(
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            wrapped.canonicalPayload.profile,
        )
        assertEquals(0x0102, wrapped.canonicalPayload.schemaVersion)
        assertContentEquals(archive, wrapped.canonicalPayload.bytes)
        assertContentEquals(archive, IrohaPeerKagemushaAdapterV1.decode(wrapped).archive())
        assertEquals(12_423, archive.size)
        assertEquals(12_507, wrapped.encode().size)

        val tooSmall = IrohaPeerWireLimitsV1(
            maximumCanonicalBytes = 32 * 1024,
            maximumKagemushaEncodedBytes = archive.size - 1,
        )
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerKagemushaAdapterV1.wrap(
                typed,
                IrohaPeerWireCompressionPolicyV1.DISABLED,
                tooSmall,
            )
        }
    }

    @Test
    fun `Kagemusha profile requires exact native bridge independent Norito envelope`() {
        val canonical = kagemushaArchive(
            IrohaPeerPayloadKind.RECEIVE_REQUEST,
            byteArrayOf(0x51),
        )
        assertEquals(49, canonical.size)
        assertEquals(
            "4e5254300000bfd427e87daf1d5cfa39b7fb60a76859000100000000000000" +
                "de8130dd3f67aeb502000000000000000051",
            canonical.hex(),
        )
        val message = IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            IrohaPeerPayloadKind.RECEIVE_REQUEST,
            0x0102,
            canonical,
        ))
        assertEquals(message, IrohaPeerWireMessageV1.decode(message.encode()))

        val wrongSchema = canonical.copyOf().also { it[6] = (it[6].toInt() xor 1).toByte() }
        val shortPadding = canonical.copyOfRange(0, 40) + canonical.copyOfRange(41, canonical.size)
        val longPadding = canonical.copyOfRange(0, 40) + byteArrayOf(0) +
            canonical.copyOfRange(40, canonical.size)
        val wrongChecksum = canonical.copyOf().also { it[31] = (it[31].toInt() xor 1).toByte() }
        val wrongFlags = canonical.copyOf().also { it[39] = 0 }
        val wrongCompression = canonical.copyOf().also { it[22] = 1 }
        val trailing = canonical + byteArrayOf(0)
        val body = byteArrayOf(0x51)
        val bareSchema = NoritoHeader(
            SchemaHash.hash16("OfflineRecipientReceiveOfferV2"),
            body.size,
            CRC64.compute(body),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE,
        ).encode() + ByteArray(8) + body

        for (invalid in listOf(
            wrongSchema,
            shortPadding,
            longPadding,
            wrongChecksum,
            wrongFlags,
            wrongCompression,
            trailing,
            bareSchema,
        )) {
            assertFailsWith<IllegalArgumentException> {
                IrohaPeerCanonicalPayload(
                    IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
                    IrohaPeerPayloadKind.RECEIVE_REQUEST,
                    0x0102,
                    invalid,
                )
            }
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
                IrohaPeerPayloadKind.PAYMENT,
                0x0102,
                canonical,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerWireMessageV1.decode(rehashKagemushaMessage(message.encode(), wrongSchema))
        }
    }

    @Test
    fun `qualified Kagemusha structural fixture crosses IPM QR NFC and Nearby byte for byte`() {
        val fixture = Json.parseToJsonElement(
            String(Files.readAllBytes(sharedKagemushaFixture()), Charsets.UTF_8),
        ).jsonObject
        assertEquals("false", fixture.getValue("semantic_valid").jsonPrimitive.content)
        val norito = fixture.getValue("norito").jsonObject
        val archive = norito.getValue("archive_hex").jsonPrimitive.content.hexBytes()
        assertEquals(49, archive.size)

        val message = IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            IrohaPeerPayloadKind.RECEIVE_REQUEST,
            0x0102,
            archive,
        ))
        val ipm = fixture.getValue("ipm1").jsonObject
        assertEquals(ipm.getValue("message_bytes").jsonPrimitive.content.toInt(), message.encode().size)
        assertEquals(ipm.getValue("canonical_hash_hex").jsonPrimitive.content, message.canonicalHash.hex())
        assertEquals(ipm.getValue("wire_hash_hex").jsonPrimitive.content, message.wireHash.hex())
        assertContentEquals(ipm.getValue("encoded_hex").jsonPrimitive.content.hexBytes(), message.encode())
        assertEquals(message, IrohaPeerWireMessageV1.decode(message.encode()))

        val qr = fixture.getValue("qr").jsonObject
        val frames = IrohaPeerQRCodecV1.encode(message)
        assertEquals(qr.getValue("frame_count").jsonPrimitive.content.toInt(), frames.size)
        assertEquals(qr.getValue("static_text").jsonPrimitive.content, frames.single())
        assertEquals(message, IrohaPeerQRScanSessionV1().ingest(frames.single()).message)

        val nfc = fixture.getValue("nfc").jsonObject
        val nfcSession = nfc.getValue("session_hex").jsonPrimitive.content.hexBytes()
        val receiverCard = IrohaPeerNfcReceiverSessionV1(nfcSession, message.encode())
        assertContentEquals(nfc.getValue("info_hex").jsonPrimitive.content.hexBytes(), receiverCard.info().encode())
        val read = IrohaPeerNfcCommandV1.readRequest(
            nfcSession,
            message.canonicalHash,
            0,
            message.encode().size,
        )
        assertContentEquals(
            nfc.getValue("read_request_apdu_hex").jsonPrimitive.content.hexBytes(),
            IrohaPeerNfcAPDUCodecV1.encode(read),
        )
        assertContentEquals(
            nfc.getValue("read_request_response_hex").jsonPrimitive.content.hexBytes(),
            receiverCard.handle(read),
        )

        val nearby = fixture.getValue("nearby").jsonObject
        val nearbySession = nearby.getValue("session_hex").jsonPrimitive.content.hexBytes()
        val requestHash = nearby.getValue("request_hash_hex").jsonPrimitive.content.hexBytes()
        assertContentEquals(message.canonicalHash, requestHash)
        val sender = IrohaPeerNearbySessionV1(
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            IrohaPeerNearbyRoleV1.SENDER,
            nearbySession,
            requestHash,
            nearby.getValue("sender_certificate_hex").jsonPrimitive.content.hexBytes(),
            ByteArray(32) {
                nearby.getValue("sender_nonce_repeat_byte").jsonPrimitive.content.toInt().toByte()
            },
            IrohaPeerNearbyP256V1.fromPrivateBytes(
                ByteArray(31) + byteArrayOf(
                    nearby.getValue("sender_private_scalar").jsonPrimitive.content.toInt().toByte(),
                ),
            ),
        )
        val nearbyReceiver = IrohaPeerNearbySessionV1(
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            IrohaPeerNearbyRoleV1.RECEIVER,
            nearbySession,
            requestHash,
            nearby.getValue("receiver_certificate_hex").jsonPrimitive.content.hexBytes(),
            ByteArray(32) {
                nearby.getValue("receiver_nonce_repeat_byte").jsonPrimitive.content.toInt().toByte()
            },
            IrohaPeerNearbyP256V1.fromPrivateBytes(
                ByteArray(31) + byteArrayOf(
                    nearby.getValue("receiver_private_scalar").jsonPrimitive.content.toInt().toByte(),
                ),
            ),
        )
        sender.acceptPeerHello(nearbyReceiver.localHello)
        nearbyReceiver.acceptPeerHello(sender.localHello)
        val senderAuthentication = sender.makeAuthentication(
            nearby.getValue("sender_authentication_signature_hex").jsonPrimitive.content.hexBytes(),
        )
        val receiverAuthentication = nearbyReceiver.makeAuthentication(
            nearby.getValue("receiver_authentication_signature_hex").jsonPrimitive.content.hexBytes(),
        )
        assertEquals(
            nearby.getValue("transcript_hash_hex").jsonPrimitive.content,
            senderAuthentication.transcriptHash.hex(),
        )
        val acceptAll = IrohaPeerNearbySignatureVerifierV1 { _, _, _, _ -> true }
        sender.acceptPeerAuthentication(receiverAuthentication, acceptAll)
        nearbyReceiver.acceptPeerAuthentication(senderAuthentication, acceptAll)
        val record = sender.seal(message.encode())
        assertContentEquals(
            nearby.getValue("sender_record_hex").jsonPrimitive.content.hexBytes(),
            record.encode(),
        )
        assertContentEquals(
            message.encode(),
            nearbyReceiver.open(IrohaPeerNearbyEncryptedRecordV1.decode(record.encode())),
        )
    }

    @Test
    fun `rejects empty canonical IPM1 payloads`() {
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
                IrohaPeerPayloadKind.PAYMENT,
                0x0102,
                byteArrayOf(),
            )
        }
    }

    @Test
    fun `recovers one missing animated shard with header last`() {
        val canonical = kagemushaArchive(
            IrohaPeerPayloadKind.PAYMENT,
            ByteArray(652) { it.toByte() },
        )
        val message = IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            IrohaPeerPayloadKind.PAYMENT,
            0x0102,
            canonical,
        ))
        val texts = IrohaPeerQRCodecV1.encode(message)
        val frames = texts.map(IrohaPeerQRCodecV1::decodeFrame)
        assertEquals(
            listOf(
                IrohaPeerQRFrameKindV1.HEADER,
                IrohaPeerQRFrameKindV1.DATA,
                IrohaPeerQRFrameKindV1.DATA,
                IrohaPeerQRFrameKindV1.PARITY,
                IrohaPeerQRFrameKindV1.DATA,
                IrohaPeerQRFrameKindV1.PARITY,
            ),
            frames.map { it.frameKind },
        )
        assertTrue(texts.all { it.toByteArray().size <= 700 })
        val session = IrohaPeerQRScanSessionV1()
        val data1 = texts.first { IrohaPeerQRCodecV1.decodeFrame(it).let { frame ->
            frame.frameKind == IrohaPeerQRFrameKindV1.DATA && frame.index == 1
        } }
        val data2 = texts.first { IrohaPeerQRCodecV1.decodeFrame(it).let { frame ->
            frame.frameKind == IrohaPeerQRFrameKindV1.DATA && frame.index == 2
        } }
        val parity0 = texts.first { IrohaPeerQRCodecV1.decodeFrame(it).let { frame ->
            frame.frameKind == IrohaPeerQRFrameKindV1.PARITY && frame.index == 0
        } }
        session.ingest(data1)
        session.ingest(parity0)
        session.ingest(data2)
        val result = session.ingest(texts.first())
        assertEquals(message, result.message)
        assertEquals(1, result.recoveredDataFrames)
    }

    @Test
    fun `completed scanner rolls over and tampering fails closed`() {
        val first = message("first")
        val second = message("second")
        val session = IrohaPeerQRScanSessionV1()
        assertEquals(first, session.ingest(IrohaPeerQRCodecV1.encode(first).single()).message)
        assertEquals(second, session.ingest(IrohaPeerQRCodecV1.encode(second).single()).message)

        val tampered = first.encode()
        tampered[tampered.lastIndex] = (tampered.last().toInt() xor 1).toByte()
        assertFailsWith<IllegalArgumentException> { IrohaPeerWireMessageV1.decode(tampered) }
        val corruptFrame = IrohaPeerQRCodecV1.decodeFrame(
            IrohaPeerQRCodecV1.encode(first).single(),
        ).encode()
        corruptFrame[corruptFrame.lastIndex] = (corruptFrame.last().toInt() xor 1).toByte()
        assertFailsWith<IllegalArgumentException> { IrohaPeerQRFrameV1.decode(corruptFrame) }
    }

    @Test
    fun `application rejection can quarantine a completed stream with bounded lifetime`() {
        val limits = IrohaPeerQRScanLimitsV1(
            idleTimeoutMillis = 10,
            absoluteTimeoutMillis = 30,
        )
        val message = message("application-invalid-but-structural")
        val text = IrohaPeerQRCodecV1.encode(message).single()
        val session = IrohaPeerQRScanSessionV1(scanLimits = limits)
        val completed = session.ingestAt(text, 1_000).message
        assertEquals(message, completed)
        assertEquals(0, session.activeStreamCount)

        session.quarantine(checkNotNull(completed).streamId, 1_000)
        val quarantined = assertFailsWith<IllegalArgumentException> {
            session.ingestAt(text, 1_029)
        }
        assertTrue(quarantined.message.orEmpty().contains("quarantined"))
        assertEquals(message, session.ingestAt(text, 1_030).message)
        assertFailsWith<IllegalArgumentException> {
            session.quarantine(ByteArray(15), 1_031)
        }

        val bounded = (0..12).map { message("application-reject-$it") }
        bounded.forEachIndexed { index, item ->
            session.quarantine(item.streamId, 2_000L + index)
        }
        assertEquals(
            bounded.first(),
            session.ingestAt(IrohaPeerQRCodecV1.encode(bounded.first()).single(), 2_013).message,
        )
        val retained = assertFailsWith<IllegalArgumentException> {
            session.ingestAt(IrohaPeerQRCodecV1.encode(bounded[1]).single(), 2_013)
        }
        assertTrue(retained.message.orEmpty().contains("quarantined"))

        val saturated = IrohaPeerQRScanSessionV1(scanLimits = limits)
        saturated.quarantine(message.streamId, Long.MAX_VALUE)
        val atMaximum = assertFailsWith<IllegalArgumentException> {
            saturated.ingestAt(text, Long.MAX_VALUE)
        }
        assertTrue(atMaximum.message.orEmpty().contains("quarantined"))
        saturated.expire(Long.MAX_VALUE)
        val afterExpire = assertFailsWith<IllegalArgumentException> {
            saturated.ingestAt(text, Long.MAX_VALUE)
        }
        assertTrue(afterExpire.message.orEmpty().contains("quarantined"))
    }

    @Test
    fun `expected kind mismatch quarantines its stream ID`() {
        val acknowledgement = IrohaPeerKagemushaStructuralTestV1.message(
            IrohaPeerPayloadKind.ACKNOWLEDGEMENT,
            "ack".toByteArray(),
        )
        val kindText = IrohaPeerQRCodecV1.encode(acknowledgement).single()
        val kindSession = IrohaPeerQRScanSessionV1(
            expectedKind = IrohaPeerPayloadKind.PAYMENT,
        )
        val wrongKind = assertFailsWith<IllegalArgumentException> {
            kindSession.ingestAt(kindText, 200)
        }
        assertTrue(wrongKind.message.orEmpty().contains("kind mismatch"))
        val repeatedKind = assertFailsWith<IllegalArgumentException> {
            kindSession.ingestAt(kindText, 201)
        }
        assertTrue(repeatedKind.message.orEmpty().contains("quarantined"))
    }

    @Test
    fun `first release profile schema and Kagemusha twenty four KiB body bound are enforced`() {
        assertEquals(24_576, IrohaPeerWireMessageV1.MAXIMUM_KAGEMUSHA_ENCODED_BYTES)
        assertEquals(null, IrohaPeerPayloadProfile.fromCode(1))
        assertEquals(null, IrohaPeerPayloadProfile.fromCode(0xffff))

        val boundary = IrohaPeerKagemushaStructuralTestV1.message(
            IrohaPeerPayloadKind.PAYMENT,
            ByteArray(24_528) { 0x5a },
        )
        assertEquals(24_576, boundary.encodedBody.size)
        assertEquals(boundary, IrohaPeerWireMessageV1.decode(boundary.encode()))
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerKagemushaStructuralTestV1.message(
                IrohaPeerPayloadKind.PAYMENT,
                ByteArray(24_529) { 0x5a },
            )
        }

        val schemaMismatch = assertFailsWith<IllegalArgumentException> {
            IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
                IrohaPeerPayloadKind.PAYMENT,
                1,
                kagemushaArchive(IrohaPeerPayloadKind.PAYMENT, byteArrayOf(1)),
            )
        }
        assertTrue(schemaMismatch.message.orEmpty().contains("requires schema 258, received 1"))

        val retiredHeader = boundary.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH)
        retiredHeader[6] = 0
        retiredHeader[7] = 1
        retiredHeader[10] = 0
        retiredHeader[11] = 1
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerWireMessageV1.decodeHeader(retiredHeader)
        }

        val unknownHeader = boundary.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH)
        unknownHeader[6] = 0xff.toByte()
        unknownHeader[7] = 0xff.toByte()
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerWireMessageV1.decodeHeader(unknownHeader)
        }

        val wrongSchemaHeader =
            boundary.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH)
        wrongSchemaHeader[10] = 0
        wrongSchemaHeader[11] = 1
        val hostile = assertFailsWith<IllegalArgumentException> {
            IrohaPeerWireMessageV1.decodeHeader(wrongSchemaHeader)
        }
        assertTrue(hostile.message.orEmpty().contains("requires schema 258, received 1"))
    }

    @Test
    fun `expected schema quarantines complete and animated records until expiry`() {
        val limits = IrohaPeerQRScanLimitsV1(
            idleTimeoutMillis = 10,
            absoluteTimeoutMillis = 30,
        )
        val staticText = IrohaPeerQRCodecV1.encode(message("schema quarantine")).single()
        val staticSession = IrohaPeerQRScanSessionV1(
            expectedProfile = IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            expectedKind = IrohaPeerPayloadKind.PAYMENT,
            expectedSchemaVersion = 1,
            scanLimits = limits,
        )
        val staticMismatch = assertFailsWith<IllegalArgumentException> {
            staticSession.ingestAt(staticText, 1_000)
        }
        assertTrue(staticMismatch.message.orEmpty().contains("expected 1, received 258"))
        val staticQuarantine = assertFailsWith<IllegalArgumentException> {
            staticSession.ingestAt(staticText, 1_029)
        }
        assertTrue(staticQuarantine.message.orEmpty().contains("quarantined"))
        val expiredStatic = assertFailsWith<IllegalArgumentException> {
            staticSession.ingestAt(staticText, 1_030)
        }
        assertTrue(expiredStatic.message.orEmpty().contains("expected 1, received 258"))

        val animated = IrohaPeerQRCodecV1.animatedFrameTexts(animatedMessage(72))
        val headers = animated.filter {
            IrohaPeerQRCodecV1.decodeFrame(it).frameKind == IrohaPeerQRFrameKindV1.HEADER
        }
        val parity = animated.first {
            IrohaPeerQRCodecV1.decodeFrame(it).frameKind == IrohaPeerQRFrameKindV1.PARITY
        }
        assertTrue(headers.size >= 2)
        val animatedSession = IrohaPeerQRScanSessionV1(
            expectedProfile = IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            expectedKind = IrohaPeerPayloadKind.PAYMENT,
            expectedSchemaVersion = 1,
            scanLimits = limits,
        )
        assertFailsWith<IllegalArgumentException> {
            animatedSession.ingestAt(headers.first(), 2_000)
        }
        val trailingParity = assertFailsWith<IllegalArgumentException> {
            animatedSession.ingestAt(parity, 2_001)
        }
        assertTrue(trailingParity.message.orEmpty().contains("quarantined"))
        val repeatedHeader = assertFailsWith<IllegalArgumentException> {
            animatedSession.ingestAt(headers.last(), 2_029)
        }
        assertTrue(repeatedHeader.message.orEmpty().contains("quarantined"))
        val expiredHeader = assertFailsWith<IllegalArgumentException> {
            animatedSession.ingestAt(headers.last(), 2_030)
        }
        assertTrue(expiredHeader.message.orEmpty().contains("expected 1, received 258"))

        val kagemusha = IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            IrohaPeerPayloadKind.PAYMENT,
            0x0102,
            kagemushaArchive(IrohaPeerPayloadKind.PAYMENT, byteArrayOf(0x51)),
        ))
        assertEquals(
            kagemusha,
            IrohaPeerQRScanSessionV1(
                expectedProfile = IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
                expectedKind = IrohaPeerPayloadKind.PAYMENT,
                expectedSchemaVersion = 0x0102,
            ).ingest(IrohaPeerQRCodecV1.encode(kagemusha).single()).message,
        )
    }

    @Test
    fun `scanner keeps three interleaved streams and rejects a fourth without eviction`() {
        val messages = (1..4).map(::animatedMessage)
        val frames = messages.map(IrohaPeerQRCodecV1::animatedFrameTexts)
        val session = IrohaPeerQRScanSessionV1()
        repeat(3) { session.ingestAt(frames[it].first(), 1_000) }
        assertEquals(3, session.activeStreamCount)
        assertFailsWith<IllegalArgumentException> {
            session.ingestAt(frames[3].first(), 1_000)
        }
        assertEquals(3, session.activeStreamCount)
        var completed: IrohaPeerWireMessageV1? = null
        for (text in frames[0].drop(1)) {
            completed = session.ingestAt(text, 1_001).message ?: completed
            if (completed != null) break
        }
        assertEquals(messages[0], completed)
        assertEquals(2, session.activeStreamCount)
    }

    @Test
    fun `scanner expires idle and absolute age despite duplicate noise`() {
        val frames = IrohaPeerQRCodecV1.animatedFrameTexts(animatedMessage(9))
        val limits = IrohaPeerQRScanLimitsV1(
            idleTimeoutMillis = 10,
            absoluteTimeoutMillis = 20,
        )
        val session = IrohaPeerQRScanSessionV1(scanLimits = limits)
        session.ingestAt(frames.first(), 100)
        session.ingestAt(frames.first(), 105) // duplicate must not extend lastProgress
        assertTrue(session.expire(109).isEmpty())
        assertEquals(1, session.expire(110).size)

        val absolute = IrohaPeerQRScanSessionV1(scanLimits = limits)
        absolute.ingestAt(frames[0], 200)
        absolute.ingestAt(frames[1], 207)
        absolute.ingestAt(frames[2], 214)
        assertEquals(1, absolute.expire(220).size)
    }

    @Test
    fun `scanner bounds preheader frames and bytes then quarantines conflicts`() {
        val frames = IrohaPeerQRCodecV1.animatedFrameTexts(animatedMessage(11))
        val data = frames.filter {
            IrohaPeerQRCodecV1.decodeFrame(it).frameKind == IrohaPeerQRFrameKindV1.DATA
        }
        val session = IrohaPeerQRScanSessionV1(scanLimits = IrohaPeerQRScanLimitsV1(
            maximumPreheaderFramesPerStream = 1,
            maximumPreheaderPayloadBytesPerStream = 256,
        ))
        session.ingestAt(data[0], 1)
        assertFailsWith<IllegalArgumentException> { session.ingestAt(data[1], 2) }
        assertEquals(0, session.activeStreamCount)
        assertFailsWith<IllegalArgumentException> { session.ingestAt(data[0], 3) }
    }

    @Test
    fun `checksum-correct hostile header stays quarantined until exact expiry`() {
        val message = animatedMessage(61)
        val frames = IrohaPeerQRCodecV1.animatedFrameTexts(message)
            .map(IrohaPeerQRCodecV1::decodeFrame)
        val validHeader = frames.first { it.frameKind == IrohaPeerQRFrameKindV1.HEADER }
        val hostilePayload = validHeader.payload.also {
            it[9] = 1 // Reserved IPM1 flags; IrohaPeerQRFrameV1 emits a fresh valid CRC.
        }
        val hostileHeader = IrohaPeerQRFrameV1(
            IrohaPeerQRFrameKindV1.HEADER,
            validHeader.profile,
            validHeader.payloadKind,
            validHeader.streamId,
            validHeader.index,
            validHeader.total,
            hostilePayload,
        )
        assertEquals(
            hostileHeader,
            IrohaPeerQRCodecV1.decodeFrame(IrohaPeerQRCodecV1.encodeFrame(hostileHeader)),
        )

        val session = IrohaPeerQRScanSessionV1(scanLimits = IrohaPeerQRScanLimitsV1(
            idleTimeoutMillis = 10,
            absoluteTimeoutMillis = 30,
        ))
        assertFailsWith<IllegalArgumentException> {
            session.ingestAt(IrohaPeerQRCodecV1.encodeFrame(hostileHeader), 1_000)
        }
        assertEquals(0, session.activeStreamCount)
        assertFailsWith<IllegalArgumentException> {
            session.ingestAt(IrohaPeerQRCodecV1.encodeFrame(validHeader), 1_029)
        }
        val accepted = session.ingestAt(
            IrohaPeerQRCodecV1.encodeFrame(validHeader),
            1_030,
        )
        assertContentEquals(message.streamId, validHeader.streamId)
        assertEquals(0, accepted.receivedDataFrames)
        assertEquals(1, session.activeStreamCount)
    }

    @Test
    fun `checksum-correct hostile complete body and nonzero padding fail closed`() {
        val small = message("checksum-correct-hostile")
        val corruptMessage = small.encode().also {
            it[it.lastIndex] = (it.last().toInt() xor 1).toByte()
        }
        val hostileComplete = IrohaPeerQRFrameV1(
            IrohaPeerQRFrameKindV1.COMPLETE,
            small.canonicalPayload.profile,
            small.canonicalPayload.kind,
            small.streamId,
            0,
            1,
            corruptMessage,
        )
        val completeSession = IrohaPeerQRScanSessionV1()
        assertFailsWith<IllegalArgumentException> {
            completeSession.ingest(IrohaPeerQRCodecV1.encodeFrame(hostileComplete))
        }
        assertEquals(0, completeSession.activeStreamCount)

        val animated = IrohaPeerKagemushaStructuralTestV1.message(
            IrohaPeerPayloadKind.PAYMENT,
            ByteArray(300) { (it * 73 + 63).toByte() },
        )
        val frames = IrohaPeerQRCodecV1.animatedFrameTexts(animated)
            .map(IrohaPeerQRCodecV1::decodeFrame)
        val header = frames.first { it.frameKind == IrohaPeerQRFrameKindV1.HEADER }
        val first = frames.first {
            it.frameKind == IrohaPeerQRFrameKindV1.DATA && it.index == 0
        }
        val final = frames.first {
            it.frameKind == IrohaPeerQRFrameKindV1.DATA && it.index == 1
        }
        val nonzeroPadding = final.payload.also { it[it.lastIndex] = 1 }
        val hostileFinal = IrohaPeerQRFrameV1(
            IrohaPeerQRFrameKindV1.DATA,
            final.profile,
            final.payloadKind,
            final.streamId,
            final.index,
            final.total,
            nonzeroPadding,
        )
        val animatedSession = IrohaPeerQRScanSessionV1()
        animatedSession.ingest(IrohaPeerQRCodecV1.encodeFrame(header))
        animatedSession.ingest(IrohaPeerQRCodecV1.encodeFrame(first))
        assertFailsWith<IllegalArgumentException> {
            animatedSession.ingest(IrohaPeerQRCodecV1.encodeFrame(hostileFinal))
        }
        assertEquals(0, animatedSession.activeStreamCount)
        assertFailsWith<IllegalArgumentException> {
            animatedSession.ingest(IrohaPeerQRCodecV1.encodeFrame(header))
        }
    }

    @Test
    fun `decodes only canonically useful zlib`() {
        val canonical = kagemushaArchive(
            IrohaPeerPayloadKind.PAYMENT,
            ByteArray(1_024) { 0x41 },
        )
        val decoded = IrohaPeerWireMessageV1.decode(zlibMessage(canonical))
        assertEquals(IrohaPeerContentEncodingV1.ZLIB, decoded.encoding)
        assertContentEquals(canonical, decoded.canonicalPayload.bytes)
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerWireMessageV1.decode(zlibMessage(canonical, byteArrayOf(0)))
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerWireMessageV1.decode(zlibMessage(
                kagemushaArchive(IrohaPeerPayloadKind.PAYMENT, ByteArray(100) { 0x41 }),
            ))
        }
    }

    private fun message(text: String) = IrohaPeerKagemushaStructuralTestV1.message(
        IrohaPeerPayloadKind.PAYMENT,
        text.toByteArray(),
    )

    private fun animatedMessage(seed: Int): IrohaPeerWireMessageV1 {
        var state = seed
        val bytes = ByteArray(1_800) {
            state = state * 1_664_525 + 1_013_904_223
            (state ushr 24).toByte()
        }
        return IrohaPeerKagemushaStructuralTestV1.message(
            IrohaPeerPayloadKind.PAYMENT,
            bytes,
        )
    }

    private fun kagemushaArchive(
        kind: IrohaPeerPayloadKind,
        payload: ByteArray,
    ): ByteArray {
        val schema = when (kind) {
            IrohaPeerPayloadKind.RECEIVE_REQUEST ->
                "iroha_torii_shared::offline_api::OfflineRecipientReceiveOfferV2"
            IrohaPeerPayloadKind.PAYMENT ->
                "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4"
            IrohaPeerPayloadKind.ACKNOWLEDGEMENT ->
                "iroha_data_model::offline::model::KagemushaReceiverAcknowledgementV2"
        }
        val padding = when (kind) {
            IrohaPeerPayloadKind.RECEIVE_REQUEST, IrohaPeerPayloadKind.PAYMENT -> ByteArray(8)
            IrohaPeerPayloadKind.ACKNOWLEDGEMENT -> byteArrayOf()
        }
        val header = NoritoHeader(
            SchemaHash.hash16(schema),
            payload.size,
            CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE,
        )
        return header.encode() + padding + payload
    }

    private fun rehashKagemushaMessage(
        encoded: ByteArray,
        canonical: ByteArray,
    ): ByteArray {
        require(encoded.size == IrohaPeerWireMessageV1.HEADER_LENGTH + canonical.size)
        val result = encoded.copyOf()
        canonical.copyInto(result, IrohaPeerWireMessageV1.HEADER_LENGTH)
        val canonicalHash = Blake2b.digest256(
            "IROHA-PEER-PAYLOAD-V1\u0000".toByteArray() +
                byteArrayOf(0, 2, 1, 1, 2) + canonical,
        )
        canonicalHash.copyInto(result, 20)
        val wireHash = Blake2b.digest256(
            "IROHA-PEER-MESSAGE-V1\u0000".toByteArray() +
                result.copyOfRange(0, 52) + canonical,
        )
        wireHash.copyInto(result, 52)
        return result
    }

    private fun zlibMessage(
        canonical: ByteArray,
        trailing: ByteArray = byteArrayOf(),
    ): ByteArray {
        val deflater = Deflater(Deflater.DEFAULT_COMPRESSION, false)
        val compressed = ByteArrayOutputStream()
        val buffer = ByteArray(256)
        try {
            deflater.setInput(canonical)
            deflater.finish()
            while (!deflater.finished()) compressed.write(buffer, 0, deflater.deflate(buffer))
        } finally {
            deflater.end()
        }
        val body = compressed.toByteArray() + trailing
        val metadata = byteArrayOf(0, 2, 2, 1, 2)
        val canonicalHash = Blake2b.digest256(
            "IROHA-PEER-PAYLOAD-V1\u0000".toByteArray() + metadata + canonical,
        )
        val prefix = ByteArray(52)
        "IPM1".toByteArray().copyInto(prefix)
        prefix[4] = 1
        prefix[5] = 1
        prefix[6] = 0
        prefix[7] = 2
        prefix[8] = 2
        prefix[9] = 0
        prefix[10] = 1
        prefix[11] = 2
        prefix.writeU32(12, canonical.size)
        prefix.writeU32(16, body.size)
        canonicalHash.copyInto(prefix, 20)
        val wireHash = Blake2b.digest256(
            "IROHA-PEER-MESSAGE-V1\u0000".toByteArray() + prefix + body,
        )
        return prefix + wireHash + body
    }

    private fun sharedKagemushaFixture(): Path {
        var current = Paths.get("").toAbsolutePath()
        while (true) {
            val candidate = current.resolve("fixtures/offline/kagemusha_peer_transport_v2.json")
            if (Files.isRegularFile(candidate)) return candidate
            current = current.parent ?: error("kagemusha_peer_transport_v2.json was not found")
        }
    }

    private fun portableOfferFixture(): ByteArray {
        var current = Paths.get("").toAbsolutePath()
        while (true) {
            val candidate = current.resolve(
                "crates/connect_norito_bridge/tests/fixtures/offline_recipient_receive_offer_v2.hex",
            )
            if (Files.isRegularFile(candidate)) {
                val hex = Files.readAllBytes(candidate).toString(Charsets.US_ASCII)
                    .filterNot(Char::isWhitespace)
                return hex.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
            }
            current = current.parent ?: error("portable recipient receive offer fixture was not found")
        }
    }

    private fun ByteArray.hex(): String = joinToString("") { "%02x".format(it.toInt() and 0xff) }

    private fun String.hexBytes(): ByteArray =
        chunked(2).map { it.toInt(16).toByte() }.toByteArray()

    private fun ByteArray.writeU32(offset: Int, value: Int) {
        this[offset] = (value ushr 24).toByte()
        this[offset + 1] = (value ushr 16).toByte()
        this[offset + 2] = (value ushr 8).toByte()
        this[offset + 3] = value.toByte()
    }
}
