package org.hyperledger.iroha.sdk.offline

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.jupiter.api.Test
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.security.MessageDigest
import java.util.Base64
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class OfflineCashInteropFixtureV1Test {
    @Test
    fun `Rust fixture pins semantic profile 3 messages and exact kgm2 archives`() {
        val fixtureBytes = Files.readAllBytes(rustFixturePath())
        assertEquals(RUST_FIXTURE_SHA256, fixtureBytes.sha256Hex())
        val fixture = Json.parseToJsonElement(fixtureBytes.toString(Charsets.UTF_8)).jsonObject
        assertEquals("iroha.offline-cash.peer-transport.v1", fixture.string("schema"))
        assertEquals(22, fixture.int("native_bridge_abi"))

        val transport = fixture.objectValue("transport")
        assertEquals(3, transport.int("iroha_peer_wire_profile"))
        assertEquals(0x0100, transport.int("native_text_schema_version"))
        assertEquals("kgm2:", transport.string("text_prefix"))

        val limits = fixture.objectValue("limits")
        assertEquals(
            OfflineCashLimitsV1.PAYMENT_REQUEST_RAW_MAX_BYTES,
            limits.int("payment_request_raw_max_bytes"),
        )
        assertEquals(OfflineCashLimitsV1.PAYMENT_RAW_MAX_BYTES, limits.int("payment_raw_max_bytes"))
        assertEquals(
            OfflineCashLimitsV1.ACKNOWLEDGEMENT_RAW_MAX_BYTES,
            limits.int("acknowledgement_raw_max_bytes"),
        )
        assertEquals(
            OfflineCashLimitsV1.PAYMENT_REQUEST_TEXT_MAX_BYTES,
            limits.int("payment_request_text_max_bytes"),
        )
        assertEquals(OfflineCashLimitsV1.PAYMENT_TEXT_MAX_BYTES, limits.int("payment_text_max_bytes"))
        assertEquals(
            OfflineCashLimitsV1.ACKNOWLEDGEMENT_TEXT_MAX_BYTES,
            limits.int("acknowledgement_text_max_bytes"),
        )
        assertEquals(OfflineCashLimitsV1.RAW_SESSION_MAX_BYTES, limits.int("raw_session_max_bytes"))
        assertEquals(OfflineCashLimitsV1.TEXT_SESSION_MAX_BYTES, limits.int("text_session_max_bytes"))
        assertEquals(OfflineCashLimitsV1.PAIRED_PROOF_MAX_BYTES, limits.int("paired_proof_max_bytes"))
        assertEquals(OfflineCashLimitsV1.PARITY_PROOF_MAX_BYTES, limits.int("parity_proof_max_bytes"))
        assertEquals(
            OfflineCashLimitsV1.ENCRYPTED_CREDIT_MAX_BYTES,
            limits.int("encrypted_credit_max_bytes"),
        )

        val messages = fixture.objectValue("messages")
        assertMessage(
            messages.objectValue("payment_request"),
            IrohaPeerPayloadKind.RECEIVE_REQUEST,
            "receive_request",
            "receiver_payment_request",
            1,
            768,
            1_029,
            533,
            716,
        )
        assertMessage(
            messages.objectValue("payment"),
            IrohaPeerPayloadKind.PAYMENT,
            "payment",
            "sender_payment",
            2,
            7_936,
            10_587,
            2_067,
            2_761,
        )
        assertMessage(
            messages.objectValue("acknowledgement"),
            IrohaPeerPayloadKind.ACKNOWLEDGEMENT,
            "acknowledgement",
            "receiver_acknowledgement_after_persist",
            3,
            256,
            347,
            249,
            337,
        )
        assertEquals(2_849, fixture.objectValue("session").int("raw_norito_bytes"))
        assertEquals(3_814, fixture.objectValue("session").int("kgm2_text_bytes"))
    }

    @Test
    fun `profile 3 payment matches the shared IPM1 and IQR1 vector`() {
        val fixtureBytes = Files.readAllBytes(transportFixturePath())
        assertEquals(TRANSPORT_FIXTURE_SHA256, fixtureBytes.sha256Hex())
        val fixture = Json.parseToJsonElement(fixtureBytes.toString(Charsets.UTF_8)).jsonObject
        assertEquals("iroha.offline-cash.profile3-ipm-iqr.v1", fixture.string("schema"))
        assertTrue(fixture.string("source").contains("Rust does not generate IPM1 or IQR1"))

        val semanticFixture = fixture.objectValue("semantic_fixture")
        assertEquals(RUST_FIXTURE_SHA256, semanticFixture.string("sha256_hex"))
        assertEquals("payment", semanticFixture.string("message"))
        val payment = rustFixture()
            .objectValue("messages")
            .objectValue("payment")
        assertEquals(
            payment.string("kgm2_text_sha256_hex"),
            semanticFixture.string("kgm2_text_sha256_hex"),
        )
        val text = payment.string("kgm2_text")
        val payload = IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.OFFLINE_CASH_V1,
            IrohaPeerPayloadKind.PAYMENT,
            0x0100,
            text.toByteArray(Charsets.UTF_8),
        )
        val message = IrohaPeerWireMessageV1(
            payload,
            IrohaPeerWireCompressionPolicyV1.PEER_OPTIMIZED,
        )
        val transport = fixture.objectValue("transport")
        assertEquals(3, transport.int("profile"))
        assertEquals("payment", transport.string("payload_kind"))
        assertEquals(2, transport.int("payload_kind_id"))
        assertEquals(0x0100, transport.int("schema_version"))
        assertEquals("peer_optimized", transport.string("compression_policy"))
        assertEquals(IrohaPeerContentEncodingV1.ZLIB, message.encoding)
        assertEquals(message.encoding.name.lowercase(), transport.string("selected_content_encoding"))
        assertEquals(transport.int("canonical_payload_bytes"), payload.byteCount)
        assertEquals(transport.string("canonical_hash_hex"), message.canonicalHash.hex())
        assertEquals(transport.string("wire_hash_hex"), message.wireHash.hex())
        assertEquals(transport.string("stream_id_hex"), message.streamId.hex())
        assertEquals(transport.int("encoded_body_bytes"), message.encodedBody.size)
        assertEquals(transport.string("encoded_body_sha256_hex"), message.encodedBody.sha256Hex())

        val encoded = message.encode()
        assertEquals(transport.int("ipm1_bytes"), encoded.size)
        assertEquals(transport.string("ipm1_sha256_hex"), encoded.sha256Hex())
        assertEquals(transport.string("ipm1_encoded_hex"), encoded.hex())
        assertEquals(
            message,
            IrohaPeerWireMessageV1.decode(
                encoded,
                IrohaPeerPayloadProfile.OFFLINE_CASH_V1,
                IrohaPeerPayloadKind.PAYMENT,
            ),
        )

        val qr = fixture.objectValue("qr")
        assertEquals(JsonNull, qr.getValue("static_complete_text"))
        val staticText = IrohaPeerQRCodecV1.staticCompleteTextCandidate(message)
        val animatedTexts = IrohaPeerQRCodecV1.animatedFrameTexts(message)
        val frames = animatedTexts.map(IrohaPeerQRCodecV1::decodeFrame)
        assertEquals(null, staticText)
        assertEquals(qr.int("animated_frame_count"), frames.size)
        assertEquals(qr.int("data_frame_total"), frames.count {
            it.frameKind == IrohaPeerQRFrameKindV1.DATA
        })
        assertEquals(IrohaPeerQRCodecV1.PARITY_GROUP, qr.int("parity_group_width"))

        val expectedFrames = qr.getValue("frames").jsonArray.map { it.jsonObject }
        assertEquals(expectedFrames.map { it.string("text") }, animatedTexts)
        expectedFrames.forEachIndexed { index, expected ->
            val frame = frames[index]
            assertEquals(index, expected.int("sequence"))
            assertEquals(expected.string("frame_kind"), frame.frameKind.name.lowercase())
            assertEquals(expected.int("frame_kind_id"), frame.frameKind.code)
            assertEquals(IrohaPeerPayloadProfile.OFFLINE_CASH_V1, frame.profile)
            assertEquals(IrohaPeerPayloadKind.PAYMENT, frame.payloadKind)
            assertContentEquals(message.streamId, frame.streamId)
            assertEquals(expected.int("index"), frame.index)
            assertEquals(expected.int("total"), frame.total)
            assertEquals(expected.string("payload_sha256_hex"), frame.payload.sha256Hex())
            assertEquals(expected.string("encoded_frame_sha256_hex"), frame.encode().sha256Hex())
            assertEquals(
                expected.string("text_sha256_hex"),
                animatedTexts[index].toByteArray(Charsets.UTF_8).sha256Hex(),
            )
        }
        assertTrue(frames.any { it.frameKind == IrohaPeerQRFrameKindV1.PARITY })
        assertParity(frames)
    }

    private fun assertMessage(
        fixture: JsonObject,
        kind: IrohaPeerPayloadKind,
        peerKind: String,
        stage: String,
        kindId: Int,
        rawMaximum: Int,
        textMaximum: Int,
        rawLength: Int,
        textLength: Int,
    ) {
        assertEquals("true", fixture.getValue("semantic_valid").jsonPrimitive.content)
        assertEquals(3, fixture.int("iroha_peer_wire_profile"))
        assertEquals(0x0100, fixture.int("native_text_schema_version"))
        assertEquals(kindId, fixture.int("payload_kind_id"))
        assertEquals(kind.code, kindId)
        assertEquals(peerKind, fixture.string("peer_payload_kind"))
        assertEquals(stage, fixture.string("stage"))
        assertEquals(rawMaximum, fixture.int("maximum_raw_norito_bytes"))
        assertEquals(textMaximum, fixture.int("maximum_kgm2_text_bytes"))
        assertEquals(rawLength, fixture.int("raw_norito_bytes"))
        assertEquals(textLength, fixture.int("kgm2_text_bytes"))

        val raw = fixture.string("raw_norito_hex").hexBytes()
        val text = fixture.string("kgm2_text")
        assertEquals(rawLength, raw.size)
        assertEquals(textLength, text.toByteArray(Charsets.UTF_8).size)
        assertEquals(
            "kgm2:" + Base64.getUrlEncoder().withoutPadding().encodeToString(raw),
            text,
        )
        assertEquals(fixture.string("raw_norito_sha256_hex"), raw.sha256Hex())
        assertEquals(
            fixture.string("kgm2_text_sha256_hex"),
            text.toByteArray(Charsets.UTF_8).sha256Hex(),
        )

        val payload = IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.OFFLINE_CASH_V1,
            kind,
            0x0100,
            text.toByteArray(Charsets.UTF_8),
        )
        assertContentEquals(text.toByteArray(Charsets.UTF_8), payload.bytes)
        val message = IrohaPeerWireMessageV1(
            payload,
            IrohaPeerWireCompressionPolicyV1.DISABLED,
        )
        val decoded = IrohaPeerWireMessageV1.decode(
            message.encode(),
            IrohaPeerPayloadProfile.OFFLINE_CASH_V1,
            kind,
        )
        assertEquals(message, decoded)
        assertContentEquals(text.toByteArray(Charsets.UTF_8), decoded.canonicalPayload.bytes)
    }

    private fun assertParity(frames: List<IrohaPeerQRFrameV1>) {
        val data = frames.filter { it.frameKind == IrohaPeerQRFrameKindV1.DATA }
            .associateBy { it.index }
        frames.filter { it.frameKind == IrohaPeerQRFrameKindV1.PARITY }.forEach { parity ->
            val first = checkNotNull(data[parity.index * 2]).payload
            val second = data[parity.index * 2 + 1]?.payload ?: ByteArray(first.size)
            val expected = ByteArray(first.size) { first[it].toInt().xor(second[it].toInt()).toByte() }
            assertContentEquals(expected, parity.payload)
        }
    }

    private fun rustFixture(): JsonObject = Json.parseToJsonElement(
        Files.readAllBytes(rustFixturePath()).toString(Charsets.UTF_8),
    ).jsonObject

    private fun rustFixturePath(): Path = findFixture("offline_cash_peer_transport_v1.json")

    private fun transportFixturePath(): Path =
        findFixture("offline_cash_profile3_ipm_iqr_v1.json")

    private fun findFixture(name: String): Path {
        var current: Path? = Paths.get("").toAbsolutePath()
        while (current != null) {
            val candidate = current.resolve("fixtures/offline").resolve(name)
            if (Files.isRegularFile(candidate)) return candidate
            current = current.parent
        }
        error("fixtures/offline/$name was not found")
    }

    private fun JsonObject.objectValue(name: String): JsonObject = getValue(name).jsonObject

    private fun JsonObject.string(name: String): String = getValue(name).jsonPrimitive.content

    private fun JsonObject.int(name: String): Int = string(name).toInt()

    private fun String.hexBytes(): ByteArray {
        require(length % 2 == 0)
        return ByteArray(length / 2) { index ->
            substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private fun ByteArray.hex(): String = joinToString("") { "%02x".format(it.toInt() and 0xff) }

    private fun ByteArray.sha256Hex(): String =
        MessageDigest.getInstance("SHA-256").digest(this).hex()

    companion object {
        private const val RUST_FIXTURE_SHA256 =
            "dc56c0852d926c9496c6f24e59e9143d28be5529e635473355bb2a8c696de257"
        private const val TRANSPORT_FIXTURE_SHA256 =
            "f61c3f5be020dd99d034b89cc17f0e44e10ed8516e821caf109c3743a8f176b4"
    }
}
