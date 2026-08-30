package org.hyperledger.iroha.sdk.offline

import java.nio.file.Files
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class IrohaPeerQRV2Test {
    @Test
    fun `IQR2 round trips unsigned u32 shard coordinates`() {
        val frame = IrohaPeerQRFrameV2(
            IrohaPeerQRFrameKindV2.DATA,
            ByteArray(16) { it.toByte() },
            65_536,
            65_537,
            byteArrayOf(0x5a),
        )
        assertEquals(frame, IrohaPeerQRFrameV2.decode(frame.encode()))
        assertEquals(frame, IrohaPeerQRFrameV2.decodeText(frame.encodeText()))
    }

    @Test
    fun `IQR2 assembly is header first file backed bounded and self cleaning`() {
        val message = IrohaPeerKagemushaStructuralTestV1.message(
            IrohaPeerPayloadKind.PAYMENT,
            ByteArray(700) { (it and 0xff).toByte() },
            IrohaPeerWireMessageV1.KAGEMUSHA_ELIGIBILITY_PAYMENT_SCHEMA_VERSION,
        )
        val encoder = IrohaPeerQREncoderV2(message)
        val directory = Files.createTempDirectory("iroha-iqr2-test-")
        try {
            IrohaPeerQRFileAssemblerV2(directory).use { assembler ->
                assertFailsWith<IllegalStateException> {
                    assembler.accept(encoder.dataFrame(0))
                }
                assertNull(assembler.accept(encoder.headerFrame()))
                var completed: IrohaPeerWireMessageV1? = null
                for (index in 0 until encoder.dataShardCount) {
                    completed = assembler.accept(encoder.dataFrame(index)) ?: completed
                }
                assertEquals(message, completed)
            }
            Files.list(directory).use { assertEquals(0, it.count()) }
        } finally {
            Files.deleteIfExists(directory)
        }
    }

    @Test
    fun `eligibility rail readiness never cross enables another rail`() {
        val closed = IrohaPeerEligibilityTransportReadinessV1()
        assertFalse(closed.isReady(IrohaPeerEligibilityTransportRailV1.QR_IQR2))
        assertFalse(closed.isReady(IrohaPeerEligibilityTransportRailV1.NFC))
        assertFalse(closed.isReady(IrohaPeerEligibilityTransportRailV1.NEARBY))

        val nfcOnly = IrohaPeerEligibilityTransportReadinessV1(nfcReady = true)
        assertFalse(nfcOnly.isReady(IrohaPeerEligibilityTransportRailV1.QR_IQR2))
        assertTrue(nfcOnly.isReady(IrohaPeerEligibilityTransportRailV1.NFC))
        assertFalse(nfcOnly.isReady(IrohaPeerEligibilityTransportRailV1.NEARBY))
    }
}
