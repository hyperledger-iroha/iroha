package org.hyperledger.iroha.sdk.offline

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.jupiter.api.Test
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class IrohaPeerNearbyV1Test {
    @Test
    fun `full peer message and Nearby record ceilings are exact`() {
        assertEquals(32 * 1_024 - 64, IrohaPeerNearbyV1.MAXIMUM_MESSAGE_BYTES)
        assertEquals(
            IrohaPeerWireMessageV1.HEADER_LENGTH + 24_576,
            IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES,
        )
        assertTrue(IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES <= IrohaPeerNearbyV1.MAXIMUM_MESSAGE_BYTES)

        val pair = authenticatedPair(20)
        val maximumPlaintext = ByteArray(IrohaPeerNearbyV1.MAXIMUM_MESSAGE_BYTES) { 0x5a }
        val record = pair.sender.seal(maximumPlaintext)
        assertEquals(IrohaPeerNearbyV1.MAXIMUM_MESSAGE_BYTES + 54, record.encode().size)
        assertTrue(record.encode().size <= 32 * 1_024)
        assertContentEquals(maximumPlaintext, pair.receiver.open(record))
        assertFailsWith<IllegalArgumentException> {
            pair.sender.seal(maximumPlaintext + byteArrayOf(0))
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyEncryptedRecordV1(
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerNearbyRoleV1.SENDER,
                ByteArray(16) { 0x5b },
                0,
                ByteArray(IrohaPeerNearbyV1.MAXIMUM_MESSAGE_BYTES + 17),
            )
        }
    }

    @Test
    fun `authentication signature fits common radio record ceiling`() {
        val maximum = IrohaPeerNearbyV1.MAXIMUM_AUTHENTICATION_SIGNATURE_BYTES
        val authentication = IrohaPeerNearbyAuthenticationV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.SENDER,
            ByteArray(16) { 1 },
            ByteArray(32) { 2 },
            ByteArray(maximum) { 3 },
        )
        assertEquals(32 * 1_024, authentication.encode().size)
        assertContentEquals(
            authentication.encode(),
            IrohaPeerNearbyAuthenticationV1.decode(authentication.encode()).encode(),
        )
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyAuthenticationV1(
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerNearbyRoleV1.SENDER,
                ByteArray(16) { 1 },
                ByteArray(32) { 2 },
                ByteArray(maximum + 1) { 3 },
            )
        }
    }

    @Test
    fun `bootstrap is exact and normal discovery context rejects zero halves`() {
        val bootstrap = IrohaPeerNearbyDiscoveryContextV1.senderBootstrap(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
        )
        assertContentEquals(ByteArray(16), bootstrap.sessionId)
        assertContentEquals(ByteArray(32), bootstrap.requestCanonicalHash)
        assertEquals(bootstrap, IrohaPeerNearbyDiscoveryContextV1.decode(bootstrap.encode()))

        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyDiscoveryContextV1(
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerNearbyRoleV1.SENDER,
                ByteArray(16),
                ByteArray(32) { 1 },
            )
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyDiscoveryContextV1(
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerNearbyRoleV1.SENDER,
                ByteArray(16) { 1 },
                ByteArray(32),
            )
        }
        val halfZero = bootstrap.encode()
        halfZero[8] = 1
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyDiscoveryContextV1.decode(halfZero)
        }
        val receiverBootstrap = bootstrap.encode().also {
            it[7] = IrohaPeerNearbyRoleV1.RECEIVER.code.toByte()
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyDiscoveryContextV1.decode(receiverBootstrap)
        }
    }

    @Test
    fun `zero hello auth and encrypted contexts fail closed`() {
        val key = IrohaPeerNearbyP256V1.fromPrivateBytes(ByteArray(31) + byteArrayOf(1))
        val session = ByteArray(16) { 1 }
        val request = ByteArray(32) { 2 }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyHelloV1(
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerNearbyRoleV1.SENDER,
                session,
                ByteArray(32),
                request,
                key.publicKey,
                byteArrayOf(1),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyAuthenticationV1(
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerNearbyRoleV1.SENDER,
                session,
                ByteArray(32),
                byteArrayOf(1),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyEncryptedRecordV1(
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerNearbyRoleV1.SENDER,
                ByteArray(16),
                0,
                ByteArray(16),
            )
        }
    }

    @Test
    fun `matches shared IPD1 and IPN1 record vectors`() {
        val fixture = fixture()
        val session = fixture.hex("session_hex")
        val requestHash = fixture.hex("request_hash_hex")
        val discovery = IrohaPeerNearbyDiscoveryContextV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.RECEIVER,
            session,
            requestHash,
        )
        assertEquals(fixture.text("service_id"), IrohaPeerNearbyV1.SERVICE_ID)
        assertContentEquals(fixture.hex("discovery_receiver_hex"), discovery.encode())
        assertEquals(discovery, IrohaPeerNearbyDiscoveryContextV1.decode(discovery.encode()))
        assertEquals(
            fixture.text("discovery_receiver_radio_base64url"),
            discovery.encodeRadioDiscovery(),
        )
        assertEquals(
            discovery,
            IrohaPeerNearbyDiscoveryContextV1.decodeRadioDiscovery(
                fixture.text("discovery_receiver_radio_base64url"),
            ),
        )

        val senderKey = IrohaPeerNearbyP256V1.fromPrivateBytes(ByteArray(31) + byteArrayOf(1))
        val receiverKey = IrohaPeerNearbyP256V1.fromPrivateBytes(ByteArray(31) + byteArrayOf(2))
        val sender = IrohaPeerNearbyHelloV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.SENDER,
            session,
            ByteArray(32) { 0x51 },
            requestHash,
            senderKey.publicKey,
            byteArrayOf(0xa1.toByte(), 0xa2.toByte()),
        )
        val receiver = IrohaPeerNearbyHelloV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.RECEIVER,
            session,
            ByteArray(32) { 0x52 },
            requestHash,
            receiverKey.publicKey,
            byteArrayOf(0xb1.toByte(), 0xb2.toByte(), 0xb3.toByte()),
        )
        val senderBytes = sender.encode()
        assertContentEquals(fixture.hex("sender_hello_hex"), senderBytes)
        assertEquals(163, senderBytes.size)
        assertContentEquals("IPN1".toByteArray(), senderBytes.copyOfRange(0, 4))
        assertEquals(1, senderBytes[4].toInt() and 0xff)
        assertEquals(IrohaPeerNearbyRecordKindV1.HELLO.code, senderBytes[5].toInt() and 0xff)
        assertContentEquals(byteArrayOf(0, 1), senderBytes.copyOfRange(6, 8))
        assertEquals(IrohaPeerNearbyRoleV1.SENDER.code, senderBytes[8].toInt() and 0xff)
        assertEquals(0, senderBytes[9].toInt() and 0xff)
        assertContentEquals(byteArrayOf(0, 65), senderBytes.copyOfRange(90, 92))
        assertContentEquals(byteArrayOf(0, 0, 0, 2), senderBytes.copyOfRange(157, 161))
        assertContentEquals(fixture.hex("receiver_hello_hex"), receiver.encode())
        assertEquals(sender, IrohaPeerNearbyHelloV1.decode(sender.encode()))

        val sessionState = IrohaPeerNearbySessionV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.SENDER,
            session,
            requestHash,
            byteArrayOf(0xa1.toByte(), 0xa2.toByte()),
            ByteArray(32) { 0x51 },
            senderKey,
        )
        sessionState.acceptPeerHello(receiver)
        val authentication = sessionState.makeAuthentication(
            byteArrayOf(0x99.toByte(), 0x98.toByte()),
        )
        assertEquals(
            fixture.text("transcript_hash_hex"),
            authentication.transcriptHash.hex(),
        )
        assertContentEquals(fixture.hex("sender_auth_hex"), authentication.encode())
        assertContentEquals(
            fixture.hex("encrypted_record_codec_hex"),
            IrohaPeerNearbyEncryptedRecordV1(
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerNearbyRoleV1.SENDER,
                session,
                0,
                ByteArray(20) { it.toByte() },
            ).encode(),
        )
    }

    @Test
    fun `radio discovery accepts only canonical Base64URL without padding`() {
        val canonical = fixture().text("discovery_receiver_radio_base64url")
        assertEquals(75, canonical.length)
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyDiscoveryContextV1.decodeRadioDiscovery("$canonical=")
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyDiscoveryContextV1.decodeRadioDiscovery(" ${canonical.drop(1)}")
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyDiscoveryContextV1.decodeRadioDiscovery(
                canonical.dropLast(1) + "R",
            )
        }
    }

    @Test
    fun `verification code requires four to twelve ASCII digits`() {
        assertEquals(true, IrohaPeerNearbyVerificationCodeV1.isValid("1234"))
        assertEquals(true, IrohaPeerNearbyVerificationCodeV1.isValid("123456"))
        assertEquals(true, IrohaPeerNearbyVerificationCodeV1.isValid("123456789012"))
        assertEquals(false, IrohaPeerNearbyVerificationCodeV1.isValid(""))
        assertEquals(false, IrohaPeerNearbyVerificationCodeV1.isValid("123"))
        assertEquals(false, IrohaPeerNearbyVerificationCodeV1.isValid("1234567890123"))
        assertEquals(false, IrohaPeerNearbyVerificationCodeV1.isValid("123 456"))
        assertEquals(false, IrohaPeerNearbyVerificationCodeV1.isValid("١٢٣٤٥٦"))
        assertEquals(false, IrohaPeerNearbyVerificationCodeV1.isValid("１２３４５６"))
    }

    @Test
    fun `authenticates transcript and encrypts both directions`() {
        val vector = fixture().getValue("aes_gcm").jsonObject
        val session = ByteArray(16) { 0x71 }
        val requestHash = ByteArray(32) { 0x72 }
        val senderKey = IrohaPeerNearbyP256V1.fromPrivateBytes(ByteArray(31) + byteArrayOf(3))
        val receiverKey = IrohaPeerNearbyP256V1.fromPrivateBytes(ByteArray(31) + byteArrayOf(4))
        val sender = IrohaPeerNearbySessionV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.SENDER,
            session,
            requestHash,
            byteArrayOf(1),
            ByteArray(32) { 0x73 },
            senderKey,
        )
        val receiver = IrohaPeerNearbySessionV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.RECEIVER,
            session,
            requestHash,
            byteArrayOf(2),
            ByteArray(32) { 0x74 },
            receiverKey,
        )
        sender.acceptPeerHello(receiver.localHello)
        receiver.acceptPeerHello(sender.localHello)
        val senderAuthentication = sender.makeAuthentication(byteArrayOf(0x11))
        val receiverAuthentication = receiver.makeAuthentication(byteArrayOf(0x22))
        assertEquals(
            vector.getValue("transcript_hash_hex").jsonPrimitive.content,
            senderAuthentication.transcriptHash.hex(),
        )
        val acceptAll = IrohaPeerNearbySignatureVerifierV1 { _, _, _, _ -> true }
        sender.acceptPeerAuthentication(receiverAuthentication, acceptAll)
        receiver.acceptPeerAuthentication(senderAuthentication, acceptAll)

        val payment = "IPM1-payment-fixture".toByteArray()
        val record = sender.seal(payment)
        assertContentEquals(
            vector.getValue("sender_record_hex").jsonPrimitive.content.hexBytes(),
            record.encode(),
        )
        assertContentEquals(payment, receiver.open(IrohaPeerNearbyEncryptedRecordV1.decode(record.encode())))
        assertFailsWith<IllegalArgumentException> { receiver.open(record) }

        val acknowledgement = "IPM1-ack-fixture".toByteArray()
        val acknowledgementRecord = receiver.seal(acknowledgement)
        assertContentEquals(
            vector.getValue("receiver_record_hex").jsonPrimitive.content.hexBytes(),
            acknowledgementRecord.encode(),
        )
        assertContentEquals(acknowledgement, sender.open(acknowledgementRecord))
    }

    @Test
    fun `hello and authentication replay never reset sequence state`() {
        val session = ByteArray(16) { 0x31 }
        val requestHash = ByteArray(32) { 0x32 }
        val sender = IrohaPeerNearbySessionV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.SENDER,
            session,
            requestHash,
            byteArrayOf(1),
            ByteArray(32) { 0x33 },
            IrohaPeerNearbyP256V1.fromPrivateBytes(ByteArray(31) + byteArrayOf(5)),
        )
        val receiver = IrohaPeerNearbySessionV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.RECEIVER,
            session,
            requestHash,
            byteArrayOf(2),
            ByteArray(32) { 0x34 },
            IrohaPeerNearbyP256V1.fromPrivateBytes(ByteArray(31) + byteArrayOf(6)),
        )
        sender.acceptPeerHello(receiver.localHello)
        receiver.acceptPeerHello(sender.localHello)
        assertFailsWith<IllegalArgumentException> { sender.acceptPeerHello(receiver.localHello) }
        val senderAuth = sender.makeAuthentication(byteArrayOf(0x41))
        val receiverAuth = receiver.makeAuthentication(byteArrayOf(0x42))
        val acceptAll = IrohaPeerNearbySignatureVerifierV1 { _, _, _, _ -> true }
        sender.acceptPeerAuthentication(receiverAuth, acceptAll)
        receiver.acceptPeerAuthentication(senderAuth, acceptAll)
        assertEquals(0, sender.seal(byteArrayOf(1)).sequence)
        assertFailsWith<IllegalArgumentException> {
            sender.acceptPeerAuthentication(receiverAuth, acceptAll)
        }
        assertEquals(1, sender.seal(byteArrayOf(2)).sequence)
    }

    @Test
    fun `record decoders reject every truncation trailing bytes and forged lengths`() {
        val session = ByteArray(16) { 0x41 }
        val request = ByteArray(32) { 0x42 }
        val hello = IrohaPeerNearbyHelloV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.SENDER,
            session,
            ByteArray(32) { 0x43 },
            request,
            IrohaPeerNearbyP256V1.fromPrivateBytes(ByteArray(31) + byteArrayOf(7)).publicKey,
            ByteArray(32) { 0x44 },
        ).encode()
        val authentication = IrohaPeerNearbyAuthenticationV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.SENDER,
            session,
            ByteArray(32) { 0x45 },
            ByteArray(64) { 0x46 },
        ).encode()
        val encrypted = IrohaPeerNearbyEncryptedRecordV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.SENDER,
            session,
            -1L, // Unsigned 0xffff_ffff_ffff_ffff on the wire.
            ByteArray(48) { 0x47 },
        ).encode()
        val discovery = IrohaPeerNearbyDiscoveryContextV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.RECEIVER,
            session,
            request,
        ).encode()

        for (cut in hello.indices) {
            assertFailsWith<IllegalArgumentException>("Hello truncation at $cut") {
                IrohaPeerNearbyHelloV1.decode(hello.copyOf(cut))
            }
        }
        for (cut in authentication.indices) {
            assertFailsWith<IllegalArgumentException>("Authentication truncation at $cut") {
                IrohaPeerNearbyAuthenticationV1.decode(authentication.copyOf(cut))
            }
        }
        for (cut in encrypted.indices) {
            assertFailsWith<IllegalArgumentException>("Encrypted record truncation at $cut") {
                IrohaPeerNearbyEncryptedRecordV1.decode(encrypted.copyOf(cut))
            }
        }
        for (cut in discovery.indices) {
            assertFailsWith<IllegalArgumentException>("Discovery truncation at $cut") {
                IrohaPeerNearbyDiscoveryContextV1.decode(discovery.copyOf(cut))
            }
        }

        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyHelloV1.decode(hello + byteArrayOf(0))
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyAuthenticationV1.decode(authentication + byteArrayOf(0))
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyEncryptedRecordV1.decode(encrypted + byteArrayOf(0))
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyDiscoveryContextV1.decode(discovery + byteArrayOf(0))
        }

        for (publicKeyLength in intArrayOf(0, 64, 66, 0xffff)) {
            val forged = hello.copyOf().also { it.writeNearbyU16(90, publicKeyLength) }
            assertFailsWith<IllegalArgumentException> { IrohaPeerNearbyHelloV1.decode(forged) }
        }
        val zeroCertificate = hello.copyOf().also { it.writeNearbyU32(157, 0) }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyHelloV1.decode(zeroCertificate)
        }
        val oversizedCertificate = hello.copyOf().also {
            it.writeNearbyU32(157, IrohaPeerNearbyV1.MAXIMUM_CERTIFICATE_BYTES + 1)
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyHelloV1.decode(oversizedCertificate)
        }
        val zeroSignature = authentication.copyOf().also { it.writeNearbyU16(58, 0) }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyAuthenticationV1.decode(zeroSignature)
        }
        val oversizedCiphertext = encrypted.copyOf().also {
            it.writeNearbyU32(34, IrohaPeerNearbyV1.MAXIMUM_MESSAGE_BYTES + 17)
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyEncryptedRecordV1.decode(oversizedCiphertext)
        }
    }

    @Test
    fun `unsigned sequence extremes round trip and rejected records do not advance state`() {
        for (sequence in longArrayOf(0, Long.MAX_VALUE, Long.MIN_VALUE, -2L, -1L)) {
            val record = IrohaPeerNearbyEncryptedRecordV1(
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerNearbyRoleV1.SENDER,
                ByteArray(16) { 0x51 },
                sequence,
                ByteArray(16) { 0x52 },
            )
            assertEquals(sequence, IrohaPeerNearbyEncryptedRecordV1.decode(record.encode()).sequence)
        }

        val reordered = authenticatedPair(8)
        val first = reordered.sender.seal("first".toByteArray())
        val second = reordered.sender.seal("second".toByteArray())
        assertFailsWith<IllegalArgumentException> { reordered.receiver.open(second) }
        assertContentEquals("first".toByteArray(), reordered.receiver.open(first))
        assertContentEquals("second".toByteArray(), reordered.receiver.open(second))

        val tampered = authenticatedPair(10)
        val original = tampered.sender.seal("payment".toByteArray())
        val forged = original.encode().also {
            it[it.lastIndex] = (it.last().toInt() xor 1).toByte()
        }
        assertFailsWith<IllegalArgumentException> {
            tampered.receiver.open(IrohaPeerNearbyEncryptedRecordV1.decode(forged))
        }
        assertContentEquals("payment".toByteArray(), tampered.receiver.open(original))
    }

    @Test
    fun `P256 public key access is a defensive copy`() {
        val key = IrohaPeerNearbyP256V1.fromPrivateBytes(ByteArray(31) + byteArrayOf(12))
        val expected = key.publicKey
        val mutated = key.publicKey
        mutated.fill(0)
        assertContentEquals(expected, key.publicKey)
    }

    private class AuthenticatedPair(
        val sender: IrohaPeerNearbySessionV1,
        val receiver: IrohaPeerNearbySessionV1,
    )

    private fun authenticatedPair(seed: Int): AuthenticatedPair {
        val session = ByteArray(16) { seed.toByte() }
        val request = ByteArray(32) { (seed + 1).toByte() }
        val sender = IrohaPeerNearbySessionV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.SENDER,
            session,
            request,
            byteArrayOf(1),
            ByteArray(32) { (seed + 2).toByte() },
            IrohaPeerNearbyP256V1.fromPrivateBytes(
                ByteArray(31) + byteArrayOf((seed + 1).toByte()),
            ),
        )
        val receiver = IrohaPeerNearbySessionV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.RECEIVER,
            session,
            request,
            byteArrayOf(2),
            ByteArray(32) { (seed + 3).toByte() },
            IrohaPeerNearbyP256V1.fromPrivateBytes(
                ByteArray(31) + byteArrayOf((seed + 2).toByte()),
            ),
        )
        sender.acceptPeerHello(receiver.localHello)
        receiver.acceptPeerHello(sender.localHello)
        val senderAuthentication = sender.makeAuthentication(byteArrayOf(3))
        val receiverAuthentication = receiver.makeAuthentication(byteArrayOf(4))
        val acceptAll = IrohaPeerNearbySignatureVerifierV1 { _, _, _, _ -> true }
        sender.acceptPeerAuthentication(receiverAuthentication, acceptAll)
        receiver.acceptPeerAuthentication(senderAuthentication, acceptAll)
        return AuthenticatedPair(sender, receiver)
    }

    private fun ByteArray.writeNearbyU16(offset: Int, value: Int) {
        this[offset] = (value ushr 8).toByte()
        this[offset + 1] = value.toByte()
    }

    private fun ByteArray.writeNearbyU32(offset: Int, value: Int) {
        this[offset] = (value ushr 24).toByte()
        this[offset + 1] = (value ushr 16).toByte()
        this[offset + 2] = (value ushr 8).toByte()
        this[offset + 3] = value.toByte()
    }

    private fun fixture(): Map<String, kotlinx.serialization.json.JsonElement> {
        val parsed = Json.parseToJsonElement(
            String(Files.readAllBytes(sharedFixture()), Charsets.UTF_8),
        ).jsonObject
        return parsed
    }

    private fun Map<String, kotlinx.serialization.json.JsonElement>.text(key: String): String =
        getValue(key).jsonPrimitive.content

    private fun Map<String, kotlinx.serialization.json.JsonElement>.hex(key: String): ByteArray =
        text(key).chunked(2).map { it.toInt(16).toByte() }.toByteArray()

    private fun ByteArray.hex(): String = joinToString("") { "%02x".format(it.toInt() and 0xff) }

    private fun String.hexBytes(): ByteArray =
        chunked(2).map { it.toInt(16).toByte() }.toByteArray()

    private fun sharedFixture(): Path {
        var current = Paths.get("").toAbsolutePath()
        while (true) {
            val candidate = current.resolve("fixtures/offline/peer_nearby_v1.json")
            if (Files.isRegularFile(candidate)) return candidate
            current = current.parent ?: error("peer_nearby_v1.json was not found")
        }
    }
}
