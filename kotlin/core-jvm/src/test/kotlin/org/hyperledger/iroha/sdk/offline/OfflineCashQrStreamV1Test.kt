package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.junit.jupiter.api.Test
import java.util.Base64
import java.util.Random
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class OfflineCashQrStreamV1Test {
    @Test
    fun `small canonical peer text roundtrips as one typed frame`() {
        val text = peerText(
            IrohaPeerPayloadKind.RECEIVE_REQUEST,
            canonicalArchive(IrohaPeerPayloadKind.RECEIVE_REQUEST, ByteArray(32) { it.toByte() }),
        )
        val frames = OfflineCashQrStreamCodecV1.encodePeerText(
            text,
            IrohaPeerPayloadKind.RECEIVE_REQUEST,
        )
        assertEquals(1, frames.size)
        val frame = OfflineCashQrStreamCodecV1.decodeFrameText(frames.single())
        assertEquals(IrohaPeerPayloadProfile.OFFLINE_CASH_V1, frame.profile)
        assertEquals(IrohaPeerPayloadKind.RECEIVE_REQUEST, frame.payloadKind)

        val result = OfflineCashQrStreamDecoderV1(
            IrohaPeerPayloadKind.RECEIVE_REQUEST,
        ).ingest(frames.single())
        assertTrue(result.isComplete)
        assertEquals(text, result.completedPeerText)
        assertEquals(1.0, result.progress)
    }

    @Test
    fun `maximum payment peer text roundtrips through reordered animated frames`() {
        val payload = ByteArray(OfflineCashPaymentV1.MAX_CANONICAL_BYTES - 48)
            .also { Random(0x4b474d32L).nextBytes(it) }
        val text = peerText(
            IrohaPeerPayloadKind.PAYMENT,
            canonicalArchive(IrohaPeerPayloadKind.PAYMENT, payload),
        )
        assertEquals(OfflineCashPeerAdapterV1.MAX_PAYMENT_TEXT_BYTES, text.toByteArray().size)
        val frames = OfflineCashQrStreamCodecV1.encodePeerText(
            text,
            IrohaPeerPayloadKind.PAYMENT,
        )
        assertTrue(frames.size > 1)
        val header = frames.first {
            OfflineCashQrStreamCodecV1.decodeFrameText(it).frameKind ==
                IrohaPeerQRFrameKindV1.HEADER
        }
        val reordered = listOf(header) + frames.asReversed().filterNot { it == header }
        val decoder = OfflineCashQrStreamDecoderV1(IrohaPeerPayloadKind.PAYMENT)
        var completed: OfflineCashQrStreamProgressV1? = null
        for (frame in reordered) {
            val update = decoder.ingest(frame)
            if (update.isComplete) {
                completed = update
                break
            }
        }
        assertEquals(text, assertNotNull(completed).completedPeerText)
    }

    @Test
    fun `decoder recovers one missing shard from each available parity pair`() {
        val payload = ByteArray(2_048).also { Random(19).nextBytes(it) }
        val text = peerText(
            IrohaPeerPayloadKind.PAYMENT,
            canonicalArchive(IrohaPeerPayloadKind.PAYMENT, payload),
        )
        val frames = OfflineCashQrStreamCodecV1.encodePeerText(
            text,
            IrohaPeerPayloadKind.PAYMENT,
            OfflineCashQrStreamOptionsV1(IrohaPeerWireCompressionPolicyV1.DISABLED),
        )
        val decoded = frames.map { it to OfflineCashQrStreamCodecV1.decodeFrameText(it) }
        val filtered = decoded.filterNot { (_, frame) ->
            frame.frameKind == IrohaPeerQRFrameKindV1.DATA && frame.index % 2 == 1
        }.map { it.first }
        val decoder = OfflineCashQrStreamDecoderV1(IrohaPeerPayloadKind.PAYMENT)
        var completed: OfflineCashQrStreamProgressV1? = null
        for (frame in filtered) {
            val update = decoder.ingest(frame)
            if (update.isComplete) {
                completed = update
                break
            }
        }
        val result = assertNotNull(completed)
        assertEquals(text, result.completedPeerText)
        assertTrue(result.recoveredDataFrames > 0)
    }

    @Test
    fun `profile bounds kind and frame integrity fail closed`() {
        assertFailsWith<IllegalArgumentException> {
            OfflineCashQrStreamCodecV1.encodePeerText(
                "kgm2:AA=",
                IrohaPeerPayloadKind.PAYMENT,
            )
        }
        val oversized = peerText(
            IrohaPeerPayloadKind.PAYMENT,
            canonicalArchive(
                IrohaPeerPayloadKind.PAYMENT,
                ByteArray(OfflineCashPaymentV1.MAX_CANONICAL_BYTES - 47),
            ),
        )
        assertTrue(oversized.toByteArray().size > OfflineCashPeerAdapterV1.MAX_PAYMENT_TEXT_BYTES)
        assertFailsWith<IllegalArgumentException> {
            OfflineCashQrStreamCodecV1.encodePeerText(
                oversized,
                IrohaPeerPayloadKind.PAYMENT,
            )
        }

        val frame = OfflineCashQrStreamCodecV1.encodePeerText(
            peerText(
                IrohaPeerPayloadKind.RECEIVE_REQUEST,
                canonicalArchive(IrohaPeerPayloadKind.RECEIVE_REQUEST, byteArrayOf(1, 2, 3)),
            ),
            IrohaPeerPayloadKind.RECEIVE_REQUEST,
        ).single()
        assertFailsWith<IllegalArgumentException> {
            OfflineCashQrStreamDecoderV1(IrohaPeerPayloadKind.PAYMENT).ingest(frame)
        }
        val corrupted = frame.dropLast(2) + (if (frame[frame.length - 2] == '0') "1" else "0") + ":"
        assertFailsWith<IllegalArgumentException> {
            OfflineCashQrStreamCodecV1.decodeFrameText(corrupted)
        }
    }

    @Test
    fun `duplicate progress and quarantine expose stream context`() {
        val text = peerText(
            IrohaPeerPayloadKind.PAYMENT,
            canonicalArchive(IrohaPeerPayloadKind.PAYMENT, ByteArray(1_024) { it.toByte() }),
        )
        val frames = OfflineCashQrStreamCodecV1.encodePeerText(
            text,
            IrohaPeerPayloadKind.PAYMENT,
            OfflineCashQrStreamOptionsV1(IrohaPeerWireCompressionPolicyV1.DISABLED),
        )
        val header = frames.first {
            OfflineCashQrStreamCodecV1.decodeFrameText(it).frameKind ==
                IrohaPeerQRFrameKindV1.HEADER
        }
        val decoder = OfflineCashQrStreamDecoderV1(IrohaPeerPayloadKind.PAYMENT)
        val accepted = decoder.ingest(header, 10)
        val duplicate = decoder.ingest(header, 11)
        assertTrue(!accepted.isDuplicate)
        assertTrue(duplicate.isDuplicate)
        assertEquals(IrohaPeerPayloadKind.PAYMENT, duplicate.kind)
        assertTrue(duplicate.streamId.contentEquals(accepted.streamId))
        decoder.quarantine(duplicate.streamId, 12)
        assertFailsWith<IllegalArgumentException> { decoder.ingest(header, 13) }
    }

    private fun peerText(
        @Suppress("UNUSED_PARAMETER") kind: IrohaPeerPayloadKind,
        bytes: ByteArray,
    ): String =
        OfflineCashPeerAdapterV1.TEXT_PREFIX +
            Base64.getUrlEncoder().withoutPadding().encodeToString(bytes)

    private fun canonicalArchive(
        kind: IrohaPeerPayloadKind,
        payload: ByteArray,
    ): ByteArray {
        val schema = when (kind) {
            IrohaPeerPayloadKind.RECEIVE_REQUEST ->
                "iroha_data_model::offline::offline_cash_v1::OfflineCashPaymentRequestV1"
            IrohaPeerPayloadKind.PAYMENT ->
                "iroha_data_model::offline::offline_cash_v1::OfflineCashPaymentV1"
            IrohaPeerPayloadKind.ACKNOWLEDGEMENT ->
                "iroha_data_model::offline::offline_cash_v1::OfflineCashAcknowledgementV1"
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
}
