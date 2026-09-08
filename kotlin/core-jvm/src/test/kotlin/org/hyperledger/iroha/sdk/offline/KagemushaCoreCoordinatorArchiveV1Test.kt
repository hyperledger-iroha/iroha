// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.file.Files
import java.nio.file.Paths
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import org.junit.jupiter.api.Test
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

class KagemushaCoreCoordinatorArchiveV1Test {
    @Test fun `all Rust canonical public projections reencode byte identically`() {
        val preparation = archive("preparation")
        val candidate = archive("candidate")
        val recovery = archive("recovery")
        val receipt = archive("redemption_terminal_receipt")
        assertContentEquals(preparation, KagemushaCoreCoordinatorArchiveV1.encodePreparationShape(
            KagemushaCoreCoordinatorArchiveV1.decodePreparationShapeExact(preparation)))
        assertContentEquals(candidate, KagemushaCoreCoordinatorArchiveV1.encodeCandidateShape(
            KagemushaCoreCoordinatorArchiveV1.decodeCandidateShapeExact(candidate)))
        assertContentEquals(recovery, KagemushaCoreCoordinatorArchiveV1.encodeRecoveryShape(
            KagemushaCoreCoordinatorArchiveV1.decodeRecoveryShapeExact(recovery)))
        assertContentEquals(receipt, KagemushaCoreCoordinatorArchiveV1.encodeRedemptionReceiptShape(
            KagemushaCoreCoordinatorArchiveV1.decodeRedemptionReceiptShapeExact(receipt)))
        val decoded = KagemushaCoreCoordinatorArchiveV1.decodeCandidateShapeExact(candidate)
        assertContentEquals(preparation, KagemushaCoreCoordinatorArchiveV1.encodePreparationShape(decoded.preparation))
    }

    @Test fun `public input digest matches the Rust signed candidate exact request and operation`() {
        val candidate = KagemushaCoreCoordinatorArchiveV1.decodeCandidateShapeExact(archive("candidate"))
        val preparation = candidate.preparation
        val request = sectionBytes(load("kagemusha_v1.json"), "payment_request")
        val inputs = KagemushaDeviceSenderPublicInputsV1.SendSplit(request)
        assertContentEquals(ByteArray(32) { 7 }, preparation.operationId())
        assertContentEquals(preparation.inputsDigest(), KagemushaCoreCoordinatorArchiveV1.inputsDigestShape(
            preparation.operationId(), preparation.context, inputs))
        val substituted = preparation.operationId().also { it[0] = 8 }
        assertFalse(preparation.inputsDigest().contentEquals(
            KagemushaCoreCoordinatorArchiveV1.inputsDigestShape(substituted, preparation.context, inputs)))
    }

    @Test fun `archives reject wrong schema tails truncation excessive sizes and checksum changes`() {
        val values = listOf<Pair<String, (ByteArray) -> Any>>(
            "preparation" to KagemushaCoreCoordinatorArchiveV1::decodePreparationShapeExact,
            "candidate" to KagemushaCoreCoordinatorArchiveV1::decodeCandidateShapeExact,
            "recovery" to KagemushaCoreCoordinatorArchiveV1::decodeRecoveryShapeExact,
            "redemption_terminal_receipt" to KagemushaCoreCoordinatorArchiveV1::decodeRedemptionReceiptShapeExact,
        )
        values.forEach { (name, decode) ->
            val original = archive(name)
            listOf(byteArrayOf(), original + byteArrayOf(0), original.copyOf(original.size - 1),
                ByteArray(16 * 1024 + 1), original.copyOf().also { it[6] = (it[6].toInt() xor 1).toByte() },
                original.copyOf().also { it[it.lastIndex] = (it.last().toInt() xor 1).toByte() },
            ).forEach { assertFailsWith<IllegalArgumentException> { decode(it) } }
        }
    }

    @Test fun `valid checksums cannot conceal invalid version zero identity or nested selector substitution`() {
        val preparation = archive("preparation")
        val payload = NoritoHeader.decode(preparation, null).payload
        listOf(payload.copyOf().also { it[1] = 2 }, payload.copyOf().also { it.fill(0, 4, 36) }).forEach {
            val mutated = frame("iroha.kagemusha.core.v1.sender-preparation", 16, it)
            assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorArchiveV1.decodePreparationShapeExact(mutated) }
        }
        val candidate = KagemushaCoreCoordinatorArchiveV1.decodeCandidateShapeExact(archive("candidate"))
        val changed = KagemushaNativeSenderCandidateV1(candidate.preparation,
            KagemushaDeviceSenderPreparationSelectorV1(ByteArray(32) { 9 }, candidate.selector.preparationId()),
            candidate.candidateDigest(), candidate.hardwareCommitAuthorization())
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorArchiveV1.encodeCandidateShape(changed) }
    }

    @Test fun `receipt height preserves the complete unsigned u64 range`() {
        val value = KagemushaCoreCoordinatorArchiveV1.decodeRedemptionReceiptShapeExact(archive("redemption_terminal_receipt"))
        val high = KagemushaDeviceRedemptionTerminalReceiptV1(1, value.networkId(), value.operationId(), value.redemptionId(),
            value.terminalNullifier(), value.envelopeDigest(), value.reserveReceiptDigest(), value.authenticatedStatusDigest(),
            -1L, value.heightContextId())
        assertEquals(-1L, KagemushaCoreCoordinatorArchiveV1.decodeRedemptionReceiptShapeExact(
            KagemushaCoreCoordinatorArchiveV1.encodeRedemptionReceiptShape(high)).finalizedBlockHeight)
    }

    @Test fun `public projection arrays remain defensive and terminal digest is domain bound`() {
        val value = KagemushaCoreCoordinatorArchiveV1.decodePreparationShapeExact(archive("preparation"))
        val id = value.operationId()
        val input = value.inputsDigest()
        val copied = KagemushaNativeSenderPreparationV1(id, value.context, input)
        id.fill(0); input.fill(0)
        copied.operationId().fill(0); copied.inputsDigest().fill(0)
        assertContentEquals(archive("preparation"), KagemushaCoreCoordinatorArchiveV1.encodePreparationShape(copied))
        assertEquals(32, KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(byteArrayOf(1)).size)
        assertEquals(32, KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(ByteArray(7_936)).size)
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(byteArrayOf()) }
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(ByteArray(7_937)) }
    }

    private fun archive(name: String): ByteArray {
        val json = load("kagemusha_core_coordinator_archives_v1.json")
        require(json.contains("iroha.kagemusha.core.v1.archive-fixtures"))
        return sectionBytes(json, name)
    }

    private fun sectionBytes(json: String, name: String): ByteArray {
        val hex = requireNotNull(Regex("\"$name\"\\s*:\\s*\\{.*?\"norito_hex\"\\s*:\\s*\"([^\"]+)\"",
            RegexOption.DOT_MATCHES_ALL).find(json)).groupValues[1]
        return hex.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    }

    private fun load(name: String): String {
        var directory = Paths.get("").toAbsolutePath().normalize()
        while (directory != null) {
            val path = directory.resolve("fixtures/offline/$name")
            if (Files.isRegularFile(path)) return String(Files.readAllBytes(path), Charsets.UTF_8)
            directory = directory.parent
        }
        error("missing shared fixture $name")
    }

    private fun frame(schema: String, alignment: Int, payload: ByteArray): ByteArray {
        val bytes = NoritoCodec.encode(payload, schema, object : TypeAdapter<ByteArray> {
            override fun encode(encoder: NoritoEncoder, value: ByteArray) = encoder.writeBytes(value)
            override fun decode(decoder: NoritoDecoder) = decoder.readBytes(decoder.remaining())
        })
        val padding = (alignment - NoritoHeader.HEADER_LENGTH % alignment) % alignment
        return bytes.copyOfRange(0, NoritoHeader.HEADER_LENGTH) + ByteArray(padding) + bytes.copyOfRange(NoritoHeader.HEADER_LENGTH, bytes.size)
    }
}
