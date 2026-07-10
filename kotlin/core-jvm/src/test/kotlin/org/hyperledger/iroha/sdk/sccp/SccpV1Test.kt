package org.hyperledger.iroha.sdk.sccp

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files
import java.nio.file.Paths
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals
import kotlin.test.assertNull
import kotlin.test.assertSame
import org.hyperledger.iroha.sdk.client.JsonParser

class SccpV1Test {
    @Test
    fun sharedExactBindingVectorsMatchRust() {
        val fixture = fixture()
        assertEquals("sccp_exact_binding_v1", fixture.string("schema"))
        for (raw in fixture.list("networks")) {
            val vector = raw.asObject()
            val network = profile(vector.string("profile"))
            assertEquals((vector["tag"] as Number).toInt(), network.tag)
            assertEquals(vector.string("canonical_bytes"), SccpV1.encodeLowerHex(SccpV1.canonicalNetworkBytes(network)))
        }
        val payloadObject = fixture.objectValue("payload")
        val payload = SccpTokenPausePayloadV1(
            payloadObject.int("target_domain"),
            payloadObject.string("nonce").toLong(),
            SccpV1.decodeLowerHex(payloadObject.string("sora_asset_id")),
        )
        assertEquals(payloadObject.string("canonical_bytes"), SccpV1.encodeLowerHex(payload.canonicalBytes()))
        assertEquals(payloadObject.string("payload_hash"), SccpV1.encodeLowerHex(SccpV1.payloadHash(payload)))
        val binding = SccpV1.decodeLowerHex(fixture.string("destination_binding_hash"))
        for (raw in fixture.list("positive_vectors")) {
            val vector = raw.asObject()
            val lane = SccpLaneIdV1(profile(vector.string("source_profile")), profile(vector.string("target_profile")))
            assertEquals(vector.string("canonical_lane"), SccpV1.encodeLowerHex(SccpV1.canonicalLaneBytes(lane)))
            assertEquals(vector.string("lane_hash"), SccpV1.encodeLowerHex(SccpV1.laneHash(lane)))
            val commitment = SccpV1.commitment(SccpOutboundMessageContextV1(lane, binding), payload)
            assertEquals(vector.string("message_id"), SccpV1.encodeLowerHex(commitment.messageId()))
            assertEquals(vector.string("canonical_commitment"), SccpV1.encodeLowerHex(SccpV1.canonicalCommitmentBytes(commitment)))
            assertEquals(vector.string("commitment_root"), SccpV1.encodeLowerHex(SccpV1.commitmentRoot(commitment)))
            val decoded = SccpV1.decodeCanonicalCommitment(SccpV1.canonicalCommitmentBytes(commitment))
            assertContentEquals(commitment.messageId(), decoded.messageId())
        }
    }

    @Test
    fun bindingRotationPreservesIdentityAndChangesCommitment() {
        val fixture = fixture()
        val rotation = fixture.objectValue("binding_rotation")
        val payload = samplePayload()
        val lane = SccpLaneIdV1(profile(rotation.string("source_profile")), profile(rotation.string("target_profile")))
        val old = SccpV1.commitment(SccpOutboundMessageContextV1(lane, SccpV1.decodeLowerHex(rotation.string("old_binding_hash"))), payload)
        val updated = SccpV1.commitment(SccpOutboundMessageContextV1(lane, SccpV1.decodeLowerHex(rotation.string("new_binding_hash"))), payload)
        assertContentEquals(old.messageId(), updated.messageId())
        assertEquals(rotation.string("message_id_unchanged"), SccpV1.encodeLowerHex(updated.messageId()))
        assertNotEquals(SccpV1.encodeLowerHex(SccpV1.commitmentRoot(old)), SccpV1.encodeLowerHex(SccpV1.commitmentRoot(updated)))
        assertEquals(rotation.string("new_canonical_commitment"), SccpV1.encodeLowerHex(SccpV1.canonicalCommitmentBytes(updated)))
        assertEquals(rotation.string("new_commitment_root"), SccpV1.encodeLowerHex(SccpV1.commitmentRoot(updated)))
    }

    @Test
    fun exactProfilesTopologyAndHashRolesRejectAdversarialInputs() {
        assertSame(SccpNetworkV1.SORA_NEXUS, SccpNetworkV1.fromProfileKey("sora-nexus"))
        for (alias in listOf("SORA-NEXUS", "sora_nexus", "sora", "sora-nexus ", " bsc-mainnet", "bsc")) {
            assertNull(SccpNetworkV1.fromProfileKey(alias), alias)
        }
        assertFailsWith<IllegalArgumentException> { SccpLaneIdV1(SccpNetworkV1.SORA_NEXUS, SccpNetworkV1.SORA_TAIRA) }
        assertFailsWith<IllegalArgumentException> { SccpLaneIdV1(SccpNetworkV1.BSC_MAINNET, SccpNetworkV1.ETHEREUM_MAINNET) }
        val reversed = SccpLaneIdV1(SccpNetworkV1.BSC_MAINNET, SccpNetworkV1.SORA_NEXUS)
        assertFailsWith<IllegalArgumentException> { SccpOutboundMessageContextV1(reversed, ByteArray(32) { 1 }) }
        val lane = SccpLaneIdV1(SccpNetworkV1.SORA_NEXUS, SccpNetworkV1.BSC_MAINNET)
        assertFailsWith<IllegalArgumentException> { SccpOutboundMessageContextV1(lane, ByteArray(32)) }
        val payload = samplePayload()
        val roles = listOf(SccpV1.laneHash(lane), SccpV1.messageId(lane, payload), SccpV1.payloadHash(payload))
        roles.forEach { collision ->
            assertFailsWith<IllegalArgumentException> {
                SccpV1.commitment(SccpOutboundMessageContextV1(lane, collision), payload)
            }
        }
        val wrongLane = SccpLaneIdV1(SccpNetworkV1.SORA_NEXUS, SccpNetworkV1.ETHEREUM_MAINNET)
        assertFailsWith<IllegalArgumentException> { SccpV1.messageId(wrongLane, payload) }
    }

    @Test
    fun canonicalCommitmentRejectsTamperingAndTrailingBytes() {
        val lane = SccpLaneIdV1(SccpNetworkV1.SORA_NEXUS, SccpNetworkV1.BSC_MAINNET)
        val encoded = SccpV1.canonicalCommitmentBytes(
            SccpV1.commitment(SccpOutboundMessageContextV1(lane, ByteArray(32) { 0x55 }), samplePayload()),
        )
        for (offset in listOf(0, 1, 2, 3)) {
            val tampered = encoded.copyOf()
            tampered[offset] = 0x7f
            assertFailsWith<IllegalArgumentException> { SccpV1.decodeCanonicalCommitment(tampered) }
        }
        assertFailsWith<IllegalArgumentException> { SccpV1.decodeCanonicalCommitment(encoded + 0) }
        val collision = encoded.copyOf()
        encoded.copyOfRange(36, 68).copyInto(collision, 4)
        assertFailsWith<IllegalArgumentException> { SccpV1.decodeCanonicalCommitment(collision) }
    }

    @Test
    fun externalAccountCodecsAreCanonicalBinary() {
        validTransfer(2, ByteArray(20) { 1 })
        validTransfer(3, ByteArray(32) { 2 })
        val ton = ByteBuffer.allocate(36).order(ByteOrder.LITTLE_ENDIAN).putInt(0).put(ByteArray(32) { 3 }).array()
        validTransfer(4, ton)
        validTransfer(5, byteArrayOf(0x41) + ByteArray(20) { 4 })

        for ((codec, malformed) in listOf(
            2 to "0x${"11".repeat(20)}".toByteArray(),
            2 to ByteArray(19) { 1 },
            3 to ByteArray(31) { 1 },
            4 to ByteArray(36) { 1 },
            4 to ByteBuffer.allocate(36).order(ByteOrder.LITTLE_ENDIAN).putInt(2).put(ByteArray(32) { 1 }).array(),
            5 to (byteArrayOf(0x42) + ByteArray(20) { 1 }),
            5 to ByteArray(20) { 1 },
        )) {
            assertFailsWith<IllegalArgumentException>("codec $codec accepted malformed bytes") { validTransfer(codec, malformed) }
        }

        validCanonicalText("!".toByteArray())
        validCanonicalText(ByteArray(256) { 'a'.code.toByte() })
        for (malformed in listOf(
            byteArrayOf(), "contains space".toByteArray(), "line\nbreak".toByteArray(),
            byteArrayOf(0x7f), "é".toByteArray(), ByteArray(257) { 'a'.code.toByte() },
        )) {
            assertFailsWith<IllegalArgumentException> { validCanonicalText(malformed) }
        }
    }

    @Test
    fun nativeSourceEventDigestMatchesSharedVectorsAndRejectsRoleCollisions() {
        val fixture = loadFixture("native_transfer_event_v1.json")
        assertEquals(1, (fixture["version"] as Number).toInt())
        for (raw in fixture.list("vectors")) {
            val vector = raw.asObject()
            val lane = SccpLaneIdV1(
                profile(vector.string("source_profile")),
                profile(vector.string("target_profile")),
            )
            val messageId = SccpV1.decodeLowerHex(vector.string("message_id_hex"))
            val payloadHash = SccpV1.decodeLowerHex(vector.string("payload_hash_hex"))
            assertEquals(vector.string("canonical_lane_hex"), SccpV1.encodeLowerHex(SccpV1.canonicalLaneBytes(lane)))
            assertEquals(vector.string("lane_hash_hex"), SccpV1.encodeLowerHex(SccpV1.laneHash(lane)))
            assertEquals(
                vector.string("source_event_digest_hex"),
                SccpV1.encodeLowerHex(SccpV1.sourceEventDigest(lane, messageId, payloadHash)),
            )
        }
        val lane = SccpLaneIdV1(SccpNetworkV1.BSC_MAINNET, SccpNetworkV1.SORA_TAIRA)
        val laneHash = SccpV1.laneHash(lane)
        val message = ByteArray(32) { 1 }
        val payload = ByteArray(32) { 2 }
        assertFailsWith<IllegalArgumentException> { SccpV1.sourceEventDigest(lane, ByteArray(32), payload) }
        assertFailsWith<IllegalArgumentException> { SccpV1.sourceEventDigest(lane, message, message) }
        assertFailsWith<IllegalArgumentException> { SccpV1.sourceEventDigest(lane, laneHash, payload) }
    }

    private fun validTransfer(codec: Int, recipient: ByteArray) = SccpTransferPayloadV1(
        source = 0, destination = when (codec) { 2 -> 1; 3 -> 3; 4 -> 4; 5 -> 5; else -> error("codec") }, nonce = BigInteger.ONE, assetHomeDomain = 0,
        assetIdCodec = 6, assetId = ByteArray(32) { 9 }, amount = BigInteger.ONE,
        senderCodec = 1, sender = "alice".toByteArray(), recipientCodec = codec,
        recipient = recipient, routeIdCodec = 1, routeId = "route".toByteArray(),
    )

    private fun validCanonicalText(value: ByteArray) =
        SccpAssetRegisterPayloadV1(1, 0, 1, 1, value, 18)

    private fun samplePayload() = SccpTokenPausePayloadV1(2, 41, ByteArray(32) { 0x31 })

    private fun profile(value: String): SccpNetworkV1 = SccpNetworkV1.fromProfileKey(value) ?: error("fixture profile")

    @Suppress("UNCHECKED_CAST")
    private fun fixture(): Map<String, Any?> = loadFixture("exact_binding_v1.json")

    @Suppress("UNCHECKED_CAST")
    private fun loadFixture(name: String): Map<String, Any?> {
        val candidates = listOf(
            Paths.get("../../fixtures/sccp/$name"),
            Paths.get("../fixtures/sccp/$name"),
            Paths.get("fixtures/sccp/$name"),
        )
        val path = candidates.firstOrNull(Files::exists) ?: error("SCCP fixture missing: $name")
        return JsonParser.parse(String(Files.readAllBytes(path), Charsets.UTF_8)) as Map<String, Any?>
    }
}

@Suppress("UNCHECKED_CAST")
private fun Any?.asObject(): Map<String, Any?> = this as? Map<String, Any?> ?: error("expected object")
private fun Map<String, Any?>.objectValue(key: String): Map<String, Any?> = this[key].asObject()
private fun Map<String, Any?>.list(key: String): List<Any?> = this[key] as? List<Any?> ?: error("expected list")
private fun Map<String, Any?>.string(key: String): String = this[key] as? String ?: error("expected string")
private fun Map<String, Any?>.int(key: String): Int = (this[key] as? Number)?.toInt() ?: error("expected number")
