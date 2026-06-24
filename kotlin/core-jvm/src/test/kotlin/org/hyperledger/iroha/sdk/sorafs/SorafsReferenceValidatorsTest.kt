package org.hyperledger.iroha.sdk.sorafs

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Assumptions.assumeTrue
import org.junit.jupiter.api.Test

class SorafsReferenceValidatorsTest {
    @Test
    fun exposesBridgeSelectors() {
        assertEquals(1, SorafsOrderbookPayloadKind.ORDER_REQUEST.bridgeCode)
        assertEquals(6, SorafsOrderbookPayloadKind.RUNTIME_SNAPSHOT.bridgeCode)
        assertTrue(SorafsOrderbookPayloadKind.ORDER_REQUEST.isUserSignedPayload)
        assertTrue(!SorafsOrderbookPayloadKind.RUNTIME_SNAPSHOT.isUserSignedPayload)
        assertEquals(1, SorafsPdpPayloadKind.COMMITMENT.bridgeCode)
        assertEquals(3, SorafsPdpPayloadKind.PROOF.bridgeCode)
        assertEquals(1, SorafsOrderbookSide.BID.bridgeCode)
        assertEquals(3, SorafsOrderbookTier.ARCHIVE.bridgeCode)
        assertEquals(4, SorafsOrderbookCancelReason.REPLACED.bridgeCode)
        assertEquals(10, SorafsReferenceValidators.REQUIRED_BRIDGE_ABI_VERSION)
    }

    @Test
    fun rejectsGeneratedAtBeforeNativeDispatch() {
        val error = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.validateOrderbookPayloadJson(
                SorafsOrderbookPayloadKind.ORDER_REQUEST,
                ByteArray(0),
                generatedAtUnix = -1,
            )
        }
        assertTrue(error.message.orEmpty().contains("generatedAtUnix"))
    }

    @Test
    fun rejectsBlankLabelBeforeNativeDispatch() {
        val error = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.validatePdpPayloadJson(
                SorafsPdpPayloadKind.PROOF,
                ByteArray(0),
                label = " ",
                generatedAtUnix = 1,
            )
        }
        assertTrue(error.message.orEmpty().contains("label"))
    }

    @Test
    fun rejectsRuntimeSnapshotSigningBeforeNativeDispatch() {
        val error = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.signOrderbookPayload(
                SorafsOrderbookPayloadKind.RUNTIME_SNAPSHOT,
                ByteArray(0),
                ByteArray(32) { 0xB7.toByte() },
            )
        }
        assertTrue(error.message.orEmpty().contains("cannot be signed"))
    }

    @Test
    fun rejectsBadSigningKeyBeforeNativeDispatch() {
        val error = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.signOrderbookPayload(
                SorafsOrderbookPayloadKind.ORDER_REQUEST,
                ByteArray(0),
                ByteArray(32),
            )
        }
        assertTrue(error.message.orEmpty().contains("privateKey"))
    }

    @Test
    fun rejectsOrderbookOrderRequestFieldsBeforeNativeDispatch() {
        val error = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
                orderId = ByteArray(31) { 0x11.toByte() },
                side = SorafsOrderbookSide.BID,
                tier = SorafsOrderbookTier.HOT,
                pricePerGibMicroXor = "42",
                quantityGib = 7,
                ownerAccount = byteArrayOf(0x01),
                expiryUnix = 123,
                nonce = 1,
                makerFeeBps = 0,
                takerFeeBps = 25,
                privateKey = ByteArray(32) { 0xB7.toByte() },
            )
        }
        assertTrue(error.message.orEmpty().contains("orderId"))
    }

    @Test
    fun rejectsOrderbookSettlementReceiptFieldsBeforeNativeDispatch() {
        val error = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.buildSignedOrderbookSettlementReceipt(
                receiptId = ByteArray(32) { 0x21.toByte() },
                channelId = ByteArray(32) { 0x22.toByte() },
                tradeId = ByteArray(32) { 0x23.toByte() },
                rangeStart = 0,
                rangeEnd = 64,
                chunkHash = ByteArray(32) { 0x24.toByte() },
                bytesDelivered = 64,
                xorDebitedMicroXor = "not-a-decimal",
                providerCreditMicroXor = "10",
                feeAmountMicroXor = "1",
                issuedAtUnix = 123,
                privateKey = ByteArray(32) { 0xB7.toByte() },
            )
        }
        assertTrue(error.message.orEmpty().contains("xorDebitedMicroXor"))
    }

    @Test
    fun validatesOrderbookFixtureWhenNativeBridgeIsAvailable() {
        assumeTrue(SorafsReferenceValidators.isNativeAvailable(), "connect_norito_bridge not available")
        val payload = fixture("sorafs_manifest", "orderbook", "order_request_v1.to")
        val json = SorafsReferenceValidators.validateOrderbookPayloadJson(
            SorafsOrderbookPayloadKind.ORDER_REQUEST,
            payload,
            generatedAtUnix = 123,
        )
        assertTrue(json.contains("\"status\": \"Ok\""), json)
        assertTrue(json.contains("\"code\": \"SFS-OK-000\""), json)
    }

    @Test
    fun signsOrderbookFixtureWhenNativeBridgeIsAvailable() {
        assumeTrue(SorafsReferenceValidators.isNativeAvailable(), "connect_norito_bridge not available")
        val payload = fixture("sorafs_manifest", "orderbook", "order_request_v1.to")
        val signed = SorafsReferenceValidators.signOrderbookPayload(
            SorafsOrderbookPayloadKind.ORDER_REQUEST,
            payload,
            ByteArray(32) { 0xB7.toByte() },
        )
        assertTrue(signed.isNotEmpty())
        assertTrue(!signed.contentEquals(payload))
    }

    private fun fixture(vararg parts: String): ByteArray {
        val cwd = Paths.get(System.getProperty("user.dir")).toAbsolutePath()
        val relative = parts.fold(Paths.get("fixtures")) { path, part -> path.resolve(part) }
        val candidates = listOf(
            cwd.resolve(relative),
            cwd.resolve("..").resolve(relative),
            cwd.resolve("..").resolve("..").resolve(relative),
        )
        val path = candidates.firstOrNull { Files.exists(it) }
            ?: throw IllegalStateException("missing fixture ${relative.joinToString("/")}")
        return Files.readAllBytes(path.normalizeAbsolute())
    }

    private fun Path.normalizeAbsolute(): Path = toAbsolutePath().normalize()
}
