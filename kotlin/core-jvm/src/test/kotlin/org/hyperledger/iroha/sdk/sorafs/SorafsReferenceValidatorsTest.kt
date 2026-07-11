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
        assertEquals(1, SorafsPopPayloadKind.CREDENTIAL.bridgeCode)
        assertEquals(6, SorafsPopPayloadKind.MEMBERSHIP_PROOF.bridgeCode)
        assertEquals(7, SorafsPopPayloadKind.ISSUED_CREDENTIAL_BUNDLE.bridgeCode)
        assertEquals(1, SorafsHedgingPayloadKind.PRICE_FEED.bridgeCode)
        assertEquals(4, SorafsHedgingPayloadKind.BILLING_STATEMENT.bridgeCode)
        assertEquals(1, SorafsOrderbookSide.BID.bridgeCode)
        assertEquals(3, SorafsOrderbookTier.ARCHIVE.bridgeCode)
        assertEquals(4, SorafsOrderbookCancelReason.REPLACED.bridgeCode)
        assertEquals(16, SorafsReferenceValidators.REQUIRED_BRIDGE_ABI_VERSION)
        assertTrue(!SorafsReferenceValidators.isBridgeAbiSupported(15))
        assertTrue(SorafsReferenceValidators.isBridgeAbiSupported(16))
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
            SorafsReferenceValidators.validateHedgingPayloadJson(
                SorafsHedgingPayloadKind.PRICE_FEED,
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
    fun rejectsInvalidOrderIdDerivationInputsBeforeNativeDispatch() {
        val emptyOwner = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.deriveOrderbookOrderId(ByteArray(0), 7)
        }
        assertTrue(emptyOwner.message.orEmpty().contains("ownerAccount"))

        val zeroNonce = assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.deriveOrderbookOrderId(byteArrayOf(1), 0)
        }
        assertTrue(zeroNonce.message.orEmpty().contains("nonce"))
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

    @Test
    fun derivesCanonicalOrderIdAndRejectsExplicitMismatchWhenNativeBridgeIsAvailable() {
        assumeTrue(SorafsReferenceValidators.isNativeAvailable(), "connect_norito_bridge not available")
        val owner = "buyer@sora".toByteArray(Charsets.UTF_8)
        val orderId = SorafsReferenceValidators.deriveOrderbookOrderId(owner, 7)
        assertEquals(
            "9d91ad7700ca0c4762e031f9231aa38dd4502c6048c6ffa31d365e3c4e080b69",
            orderId.toHex(),
        )
        assertTrue(!orderId.contentEquals(SorafsReferenceValidators.deriveOrderbookOrderId(owner, 8)))
        assertTrue(
            !orderId.contentEquals(
                SorafsReferenceValidators.deriveOrderbookOrderId(
                    "provider@sora".toByteArray(Charsets.UTF_8),
                    7,
                ),
            ),
        )

        val signed = SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
            side = SorafsOrderbookSide.BID,
            tier = SorafsOrderbookTier.HOT,
            pricePerGibMicroXor = "1250000",
            quantityGib = 64,
            ownerAccount = owner,
            expiryUnix = 1_800_000_000,
            nonce = 7,
            makerFeeBps = 10,
            takerFeeBps = 15,
            privateKey = ByteArray(32) { 0xB7.toByte() },
        )
        val outcome = SorafsReferenceValidators.validateOrderbookPayloadJson(
            SorafsOrderbookPayloadKind.ORDER_REQUEST,
            signed,
            generatedAtUnix = 123,
        )
        assertTrue(outcome.contains("\"status\": \"Ok\""), outcome)

        assertThrows(IllegalArgumentException::class.java) {
            SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
                orderId = ByteArray(32) { 0x11.toByte() },
                side = SorafsOrderbookSide.BID,
                tier = SorafsOrderbookTier.HOT,
                pricePerGibMicroXor = "1250000",
                quantityGib = 64,
                ownerAccount = owner,
                expiryUnix = 1_800_000_000,
                nonce = 7,
                makerFeeBps = 10,
                takerFeeBps = 15,
                privateKey = ByteArray(32) { 0xB7.toByte() },
            )
        }
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

    private fun ByteArray.toHex(): String {
        val alphabet = "0123456789abcdef"
        val output = StringBuilder(size * 2)
        for (byte in this) {
            val value = byte.toInt() and 0xff
            output.append(alphabet[value ushr 4])
            output.append(alphabet[value and 0x0f])
        }
        return output.toString()
    }

    private fun Path.normalizeAbsolute(): Path = toAbsolutePath().normalize()
}
