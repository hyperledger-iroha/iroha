package org.hyperledger.iroha.sdk.connect

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.address.AccountAddress
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class ConnectCryptoTest {
    @Test
    fun deriveDirectionKeysRejectsLowOrderPeerPublicKey() {
        val err = assertFailsWith<ConnectProtocolException> {
            ConnectCrypto.deriveDirectionKeys(
                ByteArray(32) { 0x01 },
                ByteArray(32),
                ByteArray(32) { 0x02 },
            )
        }

        assertTrue(err.message.orEmpty().contains("all-zero"))
    }

    @Test
    fun buildApprovePreimageRejectsDomainQualifiedAccountAlias() {
        val sessionId = ByteArray(32) { 0x10 }
        val appPublic = ByteArray(32) { 0x20 }
        val walletPublic = ByteArray(32) { 0x30 }
        val account = sampleI105(0x44)

        val preimage = ConnectCrypto.buildApprovePreimage(
            sessionId,
            appPublic,
            walletPublic,
            account,
            null,
            null,
        )
        val reader = ByteBuffer.wrap(preimage).order(ByteOrder.LITTLE_ENDIAN)
        assertEquals("iroha-connect|approve|v1", reader.readTaggedUtf8("domain"))
        assertContentEquals(sessionId, reader.readTagged("sid"))
        assertContentEquals(appPublic, reader.readTagged("app_pk"))
        assertContentEquals(walletPublic, reader.readTagged("wallet_pk"))
        assertEquals(account, reader.readTaggedUtf8("account_id"))
        assertEquals(0, reader.remaining())

        val err = assertFailsWith<ConnectProtocolException> {
            ConnectCrypto.buildApprovePreimage(
                sessionId,
                appPublic,
                walletPublic,
                "$account@banka.dataspace",
                null,
                null,
            )
        }
        assertTrue(err.message.orEmpty().contains("canonical I105 encoded"))
    }

    private fun ByteBuffer.readTaggedUtf8(expectedTag: String): String =
        String(readTagged(expectedTag), StandardCharsets.UTF_8)

    private fun ByteBuffer.readTagged(expectedTag: String): ByteArray {
        val tagLength = short.toInt() and 0xffff
        val tagBytes = ByteArray(tagLength)
        get(tagBytes)
        assertEquals(expectedTag, String(tagBytes, StandardCharsets.UTF_8))

        val valueLength = long
        require(valueLength >= 0L && valueLength <= Int.MAX_VALUE) {
            "invalid tagged value length: $valueLength"
        }
        val value = ByteArray(valueLength.toInt())
        get(value)
        return value
    }

    private fun sampleI105(fill: Int): String = AccountAddress
        .fromAccount(ByteArray(32) { fill.toByte() }, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
}
