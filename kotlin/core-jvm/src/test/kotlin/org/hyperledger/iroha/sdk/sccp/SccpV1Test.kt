package org.hyperledger.iroha.sdk.sccp

import java.io.ByteArrayOutputStream
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
import org.hyperledger.iroha.sdk.core.model.instructions.TransferWirePayloadEncoder
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.client.JsonParser

class SccpV1Test {
    @Test
    fun replayForestMatchesLocalFinalV1Vector() {
        val boundary = SccpReplayBoundaryV1.EVM_DESTINATION_MINT
        val domainHash = SccpReplayV1.domainHash(
            SccpNetworkV1.SORA_TAIRA,
            SccpNetworkV1.ETHEREUM_MAINNET,
            boundary,
            7,
            hash(0x44),
            SccpReplayActorV1.evm(ByteArray(20) { 0x33 }),
        )
        assertEquals(
            "ebc495541ef2265beebe7ee9e4e8764595c2a55ed67dc6d0a8ff69ccd3ff3228",
            SccpV1.encodeLowerHex(domainHash),
        )
        val key = SccpReplayV1.replayKey(domainHash, hash(0x11))
        assertEquals(
            "035bcebe9423edd4f1b945bae54905e0f0860bcc54718d372b1a58797ce614d4",
            SccpV1.encodeLowerHex(key),
        )
        val record = SccpReplayV1.recordDigest(
            SccpReplayBoundaryV1.EVM_DESTINATION_MINT,
            hash(0x11),
            hash(0x22),
            BigInteger.valueOf(9),
            SccpReplayPrincipalV1.evm(ByteArray(20) { 0x33 }),
            hash(0x55),
        )
        assertEquals(
            "bb0a7e99f5d2d136375e46ba231903611366ea85ec0e10130488a085fa05bf4f",
            SccpV1.encodeLowerHex(record),
        )
        val empty = SccpReplayV1.emptyHashes()
        assertEquals(249, empty.size)
        assertEquals(
            "cefd4f39c0d2ba5c33835008c6c3e7bca47d6ea1c4da5bfc8a63f09dbc66651f",
            SccpV1.encodeLowerHex(empty.last()),
        )
        val zero = ByteArray(32)
        val nonMembership = SccpReplayV1.rootFromWitness(
            key,
            zero,
            SccpSparseMerkleWitnessV1(empty.last(), zero, zero, emptyList()),
        )
        assertTrue(nonMembership.matchesExpectedRoot)
        assertEquals(3, nonMembership.shard)
        val occupiedRoot = SccpV1.decodeLowerHex(
            "ec10fe878a6429557c7af279b8cb6fa5cc51165f4e6a54fb27ed6ad8525caf91",
        )
        val membership = SccpReplayV1.rootFromWitness(
            key,
            record,
            SccpSparseMerkleWitnessV1(occupiedRoot, record, zero, emptyList()),
        )
        assertTrue(membership.matchesExpectedRoot)
        assertContentEquals(occupiedRoot, membership.root())

        assertFailsWith<IllegalArgumentException> {
            SccpReplayV1.rootFromWitness(
                key,
                zero,
                SccpSparseMerkleWitnessV1(
                    empty.last(),
                    zero,
                    ByteArray(32).also { it[0] = 1 },
                    listOf(hash(0x77)),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpReplayV1.rootFromWitness(
                key,
                zero,
                SccpSparseMerkleWitnessV1(
                    empty.last(),
                    zero,
                    ByteArray(32).also { it[31] = 1 },
                    listOf(empty.first()),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpReplayV1.recordDigest(
                boundary,
                hash(0x11),
                hash(0x22),
                BigInteger.ONE.shiftLeft(128),
                SccpReplayPrincipalV1.evm(ByteArray(20) { 0x33 }),
                hash(0x55),
            )
        }

        assertTrue(
            SccpReplayV1.verifyAgainstCurrentRoot(
                zero,
                zero,
                SccpSparseMerkleWitnessV1(empty.last(), zero, zero, emptyList()),
                empty.last(),
            ).matchesExpectedRoot,
        )
        val zeroExpectedWitness = SccpSparseMerkleWitnessV1(zero, zero, zero, emptyList())
        assertFalse(SccpReplayV1.rootFromWitness(
            zero,
            zero,
            zeroExpectedWitness,
        ).matchesExpectedRoot)
        assertFailsWith<IllegalArgumentException> {
            SccpReplayV1.verifyAgainstCurrentRoot(zero, zero, zeroExpectedWitness, zero)
        }
        val zeroSiblingBitmap = ByteArray(32).also { it[31] = 1 }
        val zeroSiblingWitness = SccpSparseMerkleWitnessV1(
            empty.last(),
            zero,
            zeroSiblingBitmap,
            listOf(zero),
        )
        val zeroSiblingRoot = SccpReplayV1.rootFromWitness(
            zero,
            zero,
            zeroSiblingWitness,
        ).root()
        val boundZeroSiblingWitness = SccpSparseMerkleWitnessV1(
            zeroSiblingRoot,
            zero,
            zeroSiblingBitmap,
            listOf(zero),
        )
        assertTrue(
            SccpReplayV1.verifyAgainstCurrentRoot(
                zero,
                zero,
                boundZeroSiblingWitness,
                zeroSiblingRoot,
            ).matchesExpectedRoot,
        )
        assertFailsWith<IllegalArgumentException> {
            SccpReplayV1.verifyAgainstCurrentRoot(
                zero,
                zero,
                boundZeroSiblingWitness,
                hash(0x77),
            )
        }
    }

    @Test
    fun replayOperationPrincipalAndTonDirectionsAreExact() {
        assertEquals(0x35, SccpReplayBoundaryV1.TON_WALLET_BURN_AUTHORIZATION.tag)
        assertEquals(0x36, SccpReplayBoundaryV1.TON_WALLET_BURN_LOCK.tag)
        assertEquals(0x37, SccpReplayBoundaryV1.TON_WALLET_BURN_REFUND.tag)
        val tonActor = SccpReplayActorV1.ton(0, hash(0x66))
        for (boundary in listOf(
            SccpReplayBoundaryV1.TON_BRIDGE_INBOUND_MINT,
            SccpReplayBoundaryV1.TON_MASTER_MINT,
            SccpReplayBoundaryV1.TON_WALLET_MINT_CREDIT,
        )) {
            SccpReplayV1.domainHash(
                SccpNetworkV1.SORA_TAIRA,
                SccpNetworkV1.TON_MAINNET,
                boundary,
                7,
                hash(0x44),
                tonActor,
            )
        }
        for (boundary in listOf(
            SccpReplayBoundaryV1.TON_BRIDGE_OUTBOUND_BURN,
            SccpReplayBoundaryV1.TON_MASTER_BURN,
            SccpReplayBoundaryV1.TON_WALLET_BURN_AUTHORIZATION,
            SccpReplayBoundaryV1.TON_WALLET_BURN_LOCK,
            SccpReplayBoundaryV1.TON_WALLET_BURN_REFUND,
        )) {
            SccpReplayV1.domainHash(
                SccpNetworkV1.TON_MAINNET,
                SccpNetworkV1.SORA_TAIRA,
                boundary,
                7,
                hash(0x44),
                tonActor,
            )
            assertFailsWith<IllegalArgumentException> {
                SccpReplayV1.domainHash(
                    SccpNetworkV1.SORA_TAIRA,
                    SccpNetworkV1.TON_MAINNET,
                    boundary,
                    7,
                    hash(0x44),
                    tonActor,
                )
            }
        }
        assertFailsWith<IllegalArgumentException> {
            SccpReplayV1.recordDigest(
                SccpReplayBoundaryV1.SORA_OUTBOUND_LOCK,
                hash(0x11),
                hash(0x22),
                BigInteger.valueOf(9),
                SccpReplayPrincipalV1.evm(ByteArray(20) { 0x33 }),
                hash(0x55),
            )
        }
    }

    @Test
    fun soraReplayPrincipalRequiresExactCanonicalAccountIdPayload() {
        val account = AccountAddress
            .fromAccount(TestEd25519Keys.publicKey(0x61), "ed25519")
            .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
        val canonical = TransferWirePayloadEncoder.encodeAccountIdPayload(account)
        SccpReplayPrincipalV1.soraAccount(canonical)

        val nonCompact = nonCompactSingleAccountPayload(canonical)
        assertEquals(
            account,
            TransferWirePayloadEncoder.decodeAccountIdPayload(
                nonCompact,
                SccpV1.TAIRA_I105_DISCRIMINANT_V1,
                flags = 0,
            ),
            "alternate fixture must be a valid non-compact encoding of the same AccountId",
        )
        val wrongController = canonical.copyOf().also { it[0] = 2 }
        val wrongAlgorithm = canonical.copyOf().also { it[14] = 0x7f }
        for (invalid in listOf(
            ByteArray(0),
            byteArrayOf(0, 0, 0),
            nonCompact,
            wrongController,
            wrongAlgorithm,
            canonical + byteArrayOf(0),
        )) {
            assertFailsWith<IllegalArgumentException> {
                SccpReplayPrincipalV1.soraAccount(invalid)
            }
        }
    }

    @Test
    fun closedInventoryReservesRetiredTagsAndAliases() {
        assertEquals(
            listOf(0x40, 0x41, 0x42, 0x43, 0x44),
            SccpNetworkV1.values().map(SccpNetworkV1::tag),
        )
        assertEquals(listOf(0, 1, 2, 5, 4), SccpNetworkV1.values().map(SccpNetworkV1::domainId))
        assertSame(SccpNetworkV1.SORA_TAIRA, SccpNetworkV1.fromProfileKey("sora-taira"))
        assertTrue(SccpNetworkV1.SORA_TAIRA.production)
        for (tag in 0..255) {
            if (tag !in 0x40..0x44) assertNull(SccpNetworkV1.fromTag(tag), "unsupported tag $tag")
        }
        for (alias in listOf(
            "sora-nexus",
            "sora_nexus",
            "solana-mainnet-beta",
            "ethereum-sepolia",
            "bsc-testnet",
            "tron-nile",
            "tron-shasta",
            "ton-testnet",
            "solana-testnet",
            "SORA-TAIRA",
            "sora_taira",
            "sora-taira ",
            "bsc",
            "tron",
        )) {
            assertNull(SccpNetworkV1.fromProfileKey(alias), alias)
        }
        assertEquals(5, SccpHubMessageKindV1.TRANSFER.tag)
        assertSame(SccpHubMessageKindV1.TRANSFER, SccpHubMessageKindV1.fromTag(5))
        assertNull(SccpHubMessageKindV1.fromTag(0))
    }

    @Test
    fun allSharedMainnetTransferVectorsMatchRust() {
        val fixture = loadFixture("native_transfer_event_v1.json")
        assertEquals(1, (fixture["version"] as Number).toInt())
        var supported = 0
        for (raw in fixture.list("vectors")) {
            val vector = raw.asObject()
            val source = SccpNetworkV1.fromProfileKey(vector.string("source_profile"))
                ?: throw AssertionError("fixture contains a retired SCCP source profile")
            val target = SccpNetworkV1.fromProfileKey(vector.string("target_profile"))
                ?: throw AssertionError("fixture contains a retired SCCP target profile")
            supported++
            val lane = SccpLaneIdV1(
                source,
                target,
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
        assertEquals(4, supported)
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
        assertEquals(2, canonical[0].toInt())
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
        for (codec in listOf(0, 3, 4, 6, 255)) {
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
    fun tonMainnetBindsCanonicalZeroState() {
        val mainnet = SccpV1.canonicalNetworkBytes(SccpNetworkV1.TON_MAINNET)
        assertEquals(90, mainnet.size)
        assertContentEquals(
            byteArrayOf(1, 0x44, 4, 0, 0, 0, 0x11, 0xff.toByte(), 0xff.toByte(), 0xff.toByte()),
            mainnet.copyOfRange(0, 10),
        )
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
        val lane = SccpLaneIdV1(SccpNetworkV1.SORA_TAIRA, SccpNetworkV1.TRON_MAINNET)
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
        assertEquals(5, encoded[1].toInt())
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

    private fun nonCompactSingleAccountPayload(compact: ByteArray): ByteArray {
        require(compact.size > 14 && (compact[4].toInt() and 0xff) == compact.size - 5)
        val elementCount = readU64Le(compact, 5)
        require(elementCount > 0 && elementCount <= Int.MAX_VALUE)
        var offset = 13
        val elements = ByteArray(elementCount.toInt())
        for (index in elements.indices) {
            require((compact[offset].toInt() and 0xff) == 1)
            elements[index] = compact[offset + 1]
            offset += 2
        }
        require(offset == compact.size)
        val out = ByteArrayOutputStream()
        out.write(compact, 0, 4)
        writeU64Le(out, 8L + 9L * elements.size)
        writeU64Le(out, elements.size.toLong())
        for (element in elements) {
            writeU64Le(out, 1)
            out.write(element.toInt())
        }
        return out.toByteArray()
    }

    private fun readU64Le(value: ByteArray, offset: Int): Long {
        var result = 0L
        for (index in 0 until 8) {
            result = result or ((value[offset + index].toLong() and 0xff) shl (index * 8))
        }
        return result
    }

    private fun writeU64Le(out: ByteArrayOutputStream, value: Long) {
        for (index in 0 until 8) out.write((value ushr (index * 8)).toInt() and 0xff)
    }

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
