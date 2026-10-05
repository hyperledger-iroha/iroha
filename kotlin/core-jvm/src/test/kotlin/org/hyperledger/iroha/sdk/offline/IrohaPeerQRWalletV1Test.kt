// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Paths
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.Varint

/** QR carries complete KAGEMUSHA wallet V1 envelopes up to the 10,000-byte message bound. */
class IrohaPeerQRWalletV1Test {
    @Test
    fun `every wallet envelope vector round trips through QR under its own kind`() {
        val envelopes = vectors().getValue("envelopes").jsonArray.map { it.jsonObject }
        for (vector in envelopes) {
            val frame = hex(vector.getValue("canonical_hex").jsonPrimitive.content)
            val kind = requireNotNull(IrohaPeerPayloadKind.fromCode(vector.getValue("tag").jsonPrimitive.int))
            val message = IrohaPeerKagemushaWalletAdapterV1.wrap(
                frame,
                IrohaPeerWireCompressionPolicyV1.PEER_OPTIMIZED,
            )
            assertEquals(kind, message.canonicalPayload.kind)
            val session = IrohaPeerQRScanSessionV1(
                IrohaPeerPayloadProfile.KAGEMUSHA_WALLET_V1,
                kind,
                1,
            )
            val result = IrohaPeerQRCodecV1.encode(message).asSequence()
                .map { session.ingestAt(it, 0) }
                .first { it.isComplete }
            val decoded = assertNotNull(result.message)
            assertContentEquals(frame, IrohaPeerKagemushaWalletAdapterV1.decode(decoded))
        }
    }

    @Test
    fun `a maximum wallet message uses forty data shards and recovers one lost shard`() {
        val frame = envelope(KagemushaWalletMessageKindV1.PAYMENT, KagemushaWalletWireV1.MESSAGE_MAX_BYTES)
        val message = IrohaPeerKagemushaWalletAdapterV1.wrap(frame)
        assertEquals(10_000, message.encodedBody.size)
        assertNull(IrohaPeerQRCodecV1.staticCompleteTextCandidate(message))
        val texts = IrohaPeerQRCodecV1.animatedFrameTexts(message)
        val frames = texts.map { IrohaPeerQRCodecV1.decodeFrame(it) }
        assertEquals(66, texts.size)
        assertEquals(6, frames.count { it.frameKind == IrohaPeerQRFrameKindV1.HEADER })
        assertEquals(40, frames.count { it.frameKind == IrohaPeerQRFrameKindV1.DATA })
        assertEquals(20, frames.count { it.frameKind == IrohaPeerQRFrameKindV1.PARITY })
        assertEquals(setOf(40), frames.map { it.total }.toSet())

        val session = IrohaPeerQRScanSessionV1(IrohaPeerPayloadProfile.KAGEMUSHA_WALLET_V1)
        val lost = frames.indexOfFirst { it.frameKind == IrohaPeerQRFrameKindV1.DATA && it.index == 7 }
        val result = texts.filterIndexed { index, _ -> index != lost }.asSequence()
            .map { session.ingestAt(it, 0) }
            .first { it.isComplete }
        assertEquals(1, result.recoveredDataFrames)
        assertContentEquals(frame, IrohaPeerKagemushaWalletAdapterV1.decode(assertNotNull(result.message)))
    }

    @Test
    fun `envelopes above their kind bound or under another kind are refused`() {
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerKagemushaWalletAdapterV1.wrap(
                envelope(KagemushaWalletMessageKindV1.PAYMENT, KagemushaWalletWireV1.MESSAGE_MAX_BYTES + 1),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerKagemushaWalletAdapterV1.wrap(
                envelope(KagemushaWalletMessageKindV1.OFFER, KagemushaWalletWireV1.SESSION_MAX_BYTES + 1),
            )
        }
        val offer = envelope(KagemushaWalletMessageKindV1.OFFER, KagemushaWalletWireV1.SESSION_MAX_BYTES)
        assertEquals(IrohaPeerPayloadKind.OFFER, IrohaPeerKagemushaWalletAdapterV1.wrap(offer).canonicalPayload.kind)
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.KAGEMUSHA_WALLET_V1,
                IrohaPeerPayloadKind.REQUEST,
                1,
                offer,
            )
        }
    }

    private companion object {
        fun vectors() = Json.parseToJsonElement(
            String(Files.readAllBytes(fixturePath()), StandardCharsets.UTF_8),
        ).jsonObject

        fun fixturePath(): java.nio.file.Path {
            var current: java.nio.file.Path? = Paths.get("").toAbsolutePath().normalize()
            while (current != null) {
                val candidate = current.resolve("fixtures/kagemusha/wallet_v1_vectors.json")
                if (Files.isRegularFile(candidate)) return candidate
                current = current.parent
            }
            error("fixtures/kagemusha/wallet_v1_vectors.json was not found above the test working directory")
        }

        fun hex(text: String): ByteArray =
            ByteArray(text.length / 2) { index -> text.substring(2 * index, 2 * index + 2).toInt(16).toByte() }

        /**
         * A structurally valid envelope of exactly [frameLength] bytes whose message tag is [kind]
         * and whose variant field is filler; only the carrier's structural checks apply to it.
         */
        fun envelope(kind: KagemushaWalletMessageKindV1, frameLength: Int): ByteArray {
            val overhead = NoritoHeader.HEADER_LENGTH + KagemushaWalletWireV1.ENVELOPE_PADDING_BYTES
            var variantLength = 1
            while (true) {
                val messageLength = 4 + Varint.encode(variantLength.toLong()).size + variantLength
                val total = overhead + 3 + Varint.encode(messageLength.toLong()).size + messageLength
                if (total == frameLength) break
                check(total < frameLength) { "no envelope of exactly $frameLength bytes" }
                variantLength += 1
            }
            val variant = Varint.encode(variantLength.toLong()) + ByteArray(variantLength) { 0x5a }
            val message = byteArrayOf(kind.wireTag.toByte(), 0, 0, 0) + variant
            val payload = byteArrayOf(2, 1, 0) + Varint.encode(message.size.toLong()) + message
            return NoritoHeader(
                KagemushaWalletWireV1.envelopeSchemaHash(),
                payload.size,
                CRC64.compute(payload),
                NoritoHeader.COMPACT_LEN,
                NoritoHeader.COMPRESSION_NONE,
            ).encode() + ByteArray(KagemushaWalletWireV1.ENVELOPE_PADDING_BYTES) + payload
        }
    }
}
