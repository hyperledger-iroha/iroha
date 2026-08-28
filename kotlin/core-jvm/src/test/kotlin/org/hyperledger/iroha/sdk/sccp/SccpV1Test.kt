package org.hyperledger.iroha.sdk.sccp

import java.math.BigInteger
import java.nio.file.Files
import java.nio.file.Paths
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotEquals
import kotlin.test.assertNull
import kotlin.test.assertSame
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.client.JsonParser

class SccpV1Test {
    @Test
    fun closedInventoryReservesRetiredTagsAndAliases() {
        assertEquals(
            listOf(1, 2, 3, 4, 5, 10, 11, 12, 14, 15),
            SccpNetworkV1.values().map(SccpNetworkV1::tag),
        )
        assertSame(SccpNetworkV1.SORA_TAIRA, SccpNetworkV1.fromProfileKey("sora-taira"))
        assertTrue(SccpNetworkV1.SORA_TAIRA.production)
        assertNull(SccpNetworkV1.fromTag(0), "removed tag 0")
        for (tag in 6..9) assertNull(SccpNetworkV1.fromTag(tag), "retired tag $tag")
        for (alias in listOf(
            "sora-nexus",
            "sora_nexus",
            "solana-mainnet-beta",
            "SORA-TAIRA",
            "sora_taira",
            "sora-taira ",
            "bsc",
            "tron",
        )) {
            assertNull(SccpNetworkV1.fromProfileKey(alias), alias)
        }
    }

    @Test
    fun allSharedEthBscTronTransferVectorsMatchRust() {
        val fixture = loadFixture("native_transfer_event_v1.json")
        assertEquals(1, (fixture["version"] as Number).toInt())
        for (raw in fixture.list("vectors")) {
            val vector = raw.asObject()
            val lane = SccpLaneIdV1(
                profile(vector.string("source_profile")),
                profile(vector.string("target_profile")),
            )
            val payloadBytes = SccpV1.decodeLowerHex(vector.string("canonical_payload_hex"))
            val payload = SccpV1.decodeCanonicalPayload(payloadBytes)
            assertContentEquals(payloadBytes, payload.canonicalBytes())
            assertEquals(
                vector.string("payload_hash_hex"),
                SccpV1.encodeLowerHex(SccpV1.payloadHash(payload)),
            )
            assertEquals(
                vector.string("canonical_lane_hex"),
                SccpV1.encodeLowerHex(SccpV1.canonicalLaneBytes(lane)),
            )
            assertEquals(
                vector.string("lane_hash_hex"),
                SccpV1.encodeLowerHex(SccpV1.laneHash(lane)),
            )
            assertEquals(
                vector.string("message_id_hex"),
                SccpV1.encodeLowerHex(SccpV1.messageId(lane, payload)),
            )
            assertEquals(
                vector.string("source_event_digest_hex"),
                SccpV1.encodeLowerHex(
                    SccpV1.sourceEventDigest(
                        lane,
                        SccpV1.decodeLowerHex(vector.string("message_id_hex")),
                        SccpV1.decodeLowerHex(vector.string("payload_hash_hex")),
                    ),
                ),
            )
        }
    }

    @Test
    fun governedHashRotationPreservesReplayIdentityButChangesCommitment() {
        val lane = SccpLaneIdV1(SccpNetworkV1.SORA_TAIRA, SccpNetworkV1.BSC_MAINNET)
        val payload = outboundPayload()
        val base = SccpV1.commitment(
            SccpOutboundMessageContextV1(lane, hash(0x21), hash(0x22)),
            payload,
        )
        val bindingRotation = SccpV1.commitment(
            SccpOutboundMessageContextV1(lane, hash(0x23), hash(0x22)),
            payload,
        )
        val configRotation = SccpV1.commitment(
            SccpOutboundMessageContextV1(lane, hash(0x21), hash(0x24)),
            payload,
        )
        assertContentEquals(base.messageId(), bindingRotation.messageId())
        assertContentEquals(base.messageId(), configRotation.messageId())
        assertNotEquals(
            SccpV1.encodeLowerHex(SccpV1.commitmentRoot(base)),
            SccpV1.encodeLowerHex(SccpV1.commitmentRoot(bindingRotation)),
        )
        assertNotEquals(
            SccpV1.encodeLowerHex(SccpV1.commitmentRoot(base)),
            SccpV1.encodeLowerHex(SccpV1.commitmentRoot(configRotation)),
        )
        val decoded = SccpV1.decodeCanonicalCommitment(SccpV1.canonicalCommitmentBytes(base))
        assertContentEquals(base.messageId(), decoded.messageId())
        assertContentEquals(hash(0x22), decoded.context.routeConfigurationHash())
    }

    @Test
    fun payloadDecoderRejectsRetiredVariantsTruncationTrailingAndNoncanonicalFields() {
        val canonical = outboundPayload().canonicalBytes()
        for (discriminant in listOf(0, 1, 3, 4, 5, 255)) {
            val hostile = canonical.copyOf().also { it[0] = discriminant.toByte() }
            assertFailsWith<IllegalArgumentException> { SccpV1.decodeCanonicalPayload(hostile) }
        }
        for (length in listOf(0, 1, canonical.size - 1)) {
            assertFailsWith<IllegalArgumentException> {
                SccpV1.decodeCanonicalPayload(canonical.copyOf(length))
            }
        }
        assertFailsWith<IllegalArgumentException> {
            SccpV1.decodeCanonicalPayload(canonical + 0)
        }
        val zeroRevision = canonical.copyOf().also { bytes ->
            // discriminant + version + source + destination + nonce
            bytes.fill(0, 18, 22)
        }
        assertFailsWith<IllegalArgumentException> { SccpV1.decodeCanonicalPayload(zeroRevision) }
        val wrongVersion = canonical.copyOf().also { it[1] = 2 }
        assertFailsWith<IllegalArgumentException> { SccpV1.decodeCanonicalPayload(wrongVersion) }
    }

    @Test
    fun transferRejectsRetiredDomainsCodecsAndInvalidWidths() {
        for (domain in listOf(3, 6, -1)) {
            assertFailsWith<IllegalArgumentException> {
                transfer(source = domain, destination = 0, senderCodec = 1, recipientCodec = 1)
            }
        }
        for (codec in listOf(3, 4, 6, 0, 255)) {
            assertFailsWith<IllegalArgumentException> {
                transfer(assetCodec = codec, asset = ByteArray(32) { 1 })
            }
        }
        assertFailsWith<IllegalArgumentException> { transfer(routeRevision = 0) }
        assertFailsWith<IllegalArgumentException> { transfer(routeRevision = 0x1_0000_0000L) }
        assertFailsWith<IllegalArgumentException> { transfer(amount = BigInteger.ZERO) }
        assertFailsWith<IllegalArgumentException> { transfer(recipient = ByteArray(19) { 1 }) }
        assertFailsWith<IllegalArgumentException> { transfer(recipient = ByteArray(20)) }
        assertFailsWith<IllegalArgumentException> { transfer(asset = "contains space".toByteArray()) }
        assertFailsWith<IllegalArgumentException> { transfer(asset = ByteArray(257) { 'a'.code.toByte() }) }
        assertFailsWith<IllegalArgumentException> {
            transfer(destination = 5, recipientCodec = 5, recipient = byteArrayOf(0x42) + ByteArray(20) { 1 })
        }
        val tonRecipient = ByteArray(36).also { it.fill(0x31.toByte(), 4) }
        val ton = transfer(
            destination = 4,
            recipientCodec = 7,
            recipient = tonRecipient,
            route = "taira_ton_xor".toByteArray(),
        )
        assertContentEquals(tonRecipient, ton.recipient())
        assertFailsWith<IllegalArgumentException> {
            transfer(
                destination = 4,
                recipientCodec = 7,
                recipient = tonRecipient.copyOf().also { it[3] = 1 },
            )
        }
    }

    @Test
    fun tonProfilesBindCanonicalZeroStates() {
        val mainnet = SccpV1.canonicalNetworkBytes(SccpNetworkV1.TON_MAINNET)
        val testnet = SccpV1.canonicalNetworkBytes(SccpNetworkV1.TON_TESTNET)
        assertEquals(90, mainnet.size)
        assertEquals(90, testnet.size)
        assertContentEquals(
            byteArrayOf(1, 14, 4, 0, 0, 0, 0x11, 0xff.toByte(), 0xff.toByte(), 0xff.toByte()),
            mainnet.copyOfRange(0, 10),
        )
        assertContentEquals(
            byteArrayOf(1, 15, 4, 0, 0, 0, 0xfd.toByte(), 0xff.toByte(), 0xff.toByte(), 0xff.toByte()),
            testnet.copyOfRange(0, 10),
        )
        assertFalse(mainnet.contentEquals(testnet))
    }

    @Test
    fun canonicalTextAcceptsExactI105AndRejectsUnicodeSubstitutions() {
        val canonical = AccountAddress
            .fromAccount(TestEd25519Keys.publicKey(0x55), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        assertTrue(canonical.any { it.code > 0x7f }, "fixture must use non-ASCII I105 digits")
        val accepted = transfer(
            source = 1,
            destination = 0,
            senderCodec = 2,
            sender = ByteArray(20) { 1 },
            recipientCodec = 1,
            recipient = canonical.toByteArray(Charsets.UTF_8),
        )
        assertContentEquals(canonical.toByteArray(Charsets.UTF_8), accepted.recipient())

        val checksumAlias = canonical.dropLast(1) + if (canonical.last() == '1') "2" else "1"
        for (invalid in listOf(
            checksumAlias.toByteArray(Charsets.UTF_8),
            "ｲ".toByteArray(Charsets.UTF_8),
            "two words".toByteArray(Charsets.UTF_8),
            "line\nbreak".toByteArray(Charsets.UTF_8),
            byteArrayOf(0xff.toByte()),
            ByteArray(257) { 0x21 },
        )) {
            assertFailsWith<IllegalArgumentException> {
                transfer(
                    source = 1,
                    destination = 0,
                    senderCodec = 2,
                    sender = ByteArray(20) { 1 },
                    recipientCodec = 1,
                    recipient = invalid,
                )
            }
        }
    }

    @Test
    fun contextAndCommitmentRejectEveryZeroOrAliasedHashRole() {
        val lane = SccpLaneIdV1(SccpNetworkV1.SORA_TAIRA, SccpNetworkV1.BSC_MAINNET)
        val payload = outboundPayload()
        assertFailsWith<IllegalArgumentException> {
            SccpOutboundMessageContextV1(lane, ByteArray(32), hash(2))
        }
        assertFailsWith<IllegalArgumentException> {
            SccpOutboundMessageContextV1(lane, hash(1), hash(1))
        }
        val roles = listOf(
            SccpV1.laneHash(lane),
            SccpV1.messageId(lane, payload),
            SccpV1.payloadHash(payload),
        )
        roles.forEach { collision ->
            assertFailsWith<IllegalArgumentException> {
                SccpV1.commitment(
                    SccpOutboundMessageContextV1(lane, collision, hash(0x7f)),
                    payload,
                )
            }
            assertFailsWith<IllegalArgumentException> {
                SccpV1.commitment(
                    SccpOutboundMessageContextV1(lane, hash(0x7e), collision),
                    payload,
                )
            }
        }
    }

    @Test
    fun commitmentDecoderRejectsTagTamperingCollisionsAndTrailingBytes() {
        val lane = SccpLaneIdV1(SccpNetworkV1.SORA_TAIRA, SccpNetworkV1.TRON_NILE)
        val payload = transfer(
            destination = 5,
            recipientCodec = 5,
            recipient = byteArrayOf(0x41) + ByteArray(20) { 4 },
            route = "taira_tron_xor".toByteArray(),
        )
        val encoded = SccpV1.canonicalCommitmentBytes(
            SccpV1.commitment(
                SccpOutboundMessageContextV1(lane, hash(0x31), hash(0x32)),
                payload,
            ),
        )
        assertEquals(132, encoded.size)
        for (offset in 0..3) {
            val hostile = encoded.copyOf().also { it[offset] = 0x7f }
            assertFailsWith<IllegalArgumentException> { SccpV1.decodeCanonicalCommitment(hostile) }
        }
        assertFailsWith<IllegalArgumentException> { SccpV1.decodeCanonicalCommitment(encoded + 0) }
        for ((source, target) in listOf(4 to 36, 4 to 68, 36 to 68, 68 to 100)) {
            val collision = encoded.copyOf()
            encoded.copyOfRange(source, source + 32).copyInto(collision, target)
            assertFailsWith<IllegalArgumentException> {
                SccpV1.decodeCanonicalCommitment(collision)
            }
        }
    }

    @Test
    fun payloadAndContextDefensivelyCopyCallerBuffers() {
        val asset = "xor".toByteArray()
        val binding = hash(0x41)
        val configuration = hash(0x42)
        val payload = transfer(asset = asset)
        val context = SccpOutboundMessageContextV1(
            SccpLaneIdV1(SccpNetworkV1.SORA_TAIRA, SccpNetworkV1.BSC_MAINNET),
            binding,
            configuration,
        )
        asset.fill(0)
        binding.fill(0)
        configuration.fill(0)
        assertContentEquals("xor".toByteArray(), payload.assetId())
        assertTrue(context.destinationBindingHash().any { it.toInt() != 0 })
        assertTrue(context.routeConfigurationHash().any { it.toInt() != 0 })
        val exposed = context.destinationBindingHash()
        exposed.fill(0)
        assertTrue(context.destinationBindingHash().any { it.toInt() != 0 })
    }

    private fun outboundPayload(): SccpTransferPayloadV1 = transfer()

    private fun transfer(
        source: Int = 0,
        destination: Int = 2,
        routeRevision: Long = 1,
        assetCodec: Int = 1,
        asset: ByteArray = "xor".toByteArray(),
        amount: BigInteger = BigInteger.ONE,
        senderCodec: Int = 1,
        sender: ByteArray = "alice@taira".toByteArray(),
        recipientCodec: Int = 2,
        recipient: ByteArray = ByteArray(20) { 1 },
        route: ByteArray = "taira_bsc_xor".toByteArray(),
    ) = SccpTransferPayloadV1(
        source,
        destination,
        BigInteger.ONE,
        routeRevision,
        0,
        assetCodec,
        asset,
        amount,
        senderCodec,
        sender,
        recipientCodec,
        recipient,
        1,
        route,
    )

    private fun hash(value: Int): ByteArray = ByteArray(32) { value.toByte() }

    private fun profile(value: String): SccpNetworkV1 =
        SccpNetworkV1.fromProfileKey(value) ?: error("fixture profile")

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
private fun Any?.asObject(): Map<String, Any?> =
    this as? Map<String, Any?> ?: error("expected object")

private fun Map<String, Any?>.list(key: String): List<Any?> =
    this[key] as? List<Any?> ?: error("expected list")

private fun Map<String, Any?>.string(key: String): String =
    this[key] as? String ?: error("expected string")
