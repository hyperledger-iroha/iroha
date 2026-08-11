package org.hyperledger.iroha.sdk.connect

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.crypto.Blake2b
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.testing.TestNetworkIds
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
        val networkId = TestNetworkIds.canonical()
        val sessionId = ByteArray(32) { 0x10 }
        val appPublic = ByteArray(32) { 0x20 }
        val walletPublic = ByteArray(32) { 0x30 }
        val account = sampleI105(0x44)
        val relayAuth = ByteArray(32) { 0x55 }

        val preimage = ConnectCrypto.buildApprovePreimage(
            networkId,
            sessionId,
            appPublic,
            walletPublic,
            account,
            null,
            null,
            relayAuth,
        )
        val reader = ByteBuffer.wrap(preimage).order(ByteOrder.LITTLE_ENDIAN)
        assertEquals("iroha-connect|approve|v1", reader.readTaggedUtf8("domain"))
        assertContentEquals(networkId.bytes(), reader.readTagged("network_id"))
        val constraints = ByteBuffer.allocate(40).order(ByteOrder.LITTLE_ENDIAN)
            .putLong(32L)
            .put(networkId.bytes())
            .array()
        assertContentEquals(Blake2b.digest256(constraints), reader.readTagged("constraints"))
        assertContentEquals(sessionId, reader.readTagged("sid"))
        assertContentEquals(appPublic, reader.readTagged("app_pk"))
        assertContentEquals(walletPublic, reader.readTagged("wallet_pk"))
        assertEquals(account, reader.readTaggedUtf8("account_id"))
        assertContentEquals(relayAuth, reader.readTagged("relay_auth"))
        assertEquals(0, reader.remaining())

        val err = assertFailsWith<ConnectProtocolException> {
            ConnectCrypto.buildApprovePreimage(
                networkId,
                sessionId,
                appPublic,
                walletPublic,
                "$account@banka.dataspace",
                null,
                null,
                relayAuth,
            )
        }
        assertTrue(err.message.orEmpty().contains("canonical I105 encoded"))
    }

    @Test
    fun sessionIdBindsExactNetworkAppKeyAndNonce() {
        val networkId = TestNetworkIds.canonical()
        val appPublic = ByteArray(32) { (it + 1).toByte() }
        val nonce = ByteArray(16) { (it + 33).toByte() }
        val sid = ConnectCrypto.deriveSessionId(networkId, appPublic, nonce)

        assertEquals(32, sid.size)
        assertTrue(!sid.contentEquals(
            ConnectCrypto.deriveSessionId(TestNetworkIds.fromSeed(99), appPublic, nonce),
        ))
        assertTrue(!sid.contentEquals(
            ConnectCrypto.deriveSessionId(networkId, appPublic.copyOf().also { it[0]++ }, nonce),
        ))
        assertTrue(!sid.contentEquals(
            ConnectCrypto.deriveSessionId(networkId, appPublic, nonce.copyOf().also { it[0]++ }),
        ))
        assertFailsWith<ConnectProtocolException> {
            ConnectCrypto.deriveSessionId(networkId, ByteArray(32), nonce)
        }
        assertFailsWith<ConnectProtocolException> {
            ConnectCrypto.deriveSessionId(networkId, appPublic, ByteArray(16))
        }
    }

    @Test
    fun approvalSignatureRejectsNetworkAccountRelayAndSignatureSubstitution() {
        val signer = Ed25519PrivateKeyParameters(ByteArray(32) { 0x42 }, 0)
        val account = AccountAddress
            .fromAccount(signer.generatePublicKey().encoded, "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val networkId = TestNetworkIds.canonical()
        val sid = ByteArray(32) { (it + 1).toByte() }
        val appPublic = ByteArray(32) { (it + 11).toByte() }
        val walletPublic = ByteArray(32) { (it + 21).toByte() }
        val relayAuth = ConnectCrypto.relayAuthHash(sid, "relay-token")
        val preimage = ConnectCrypto.buildApprovePreimage(
            networkId,
            sid,
            appPublic,
            walletPublic,
            account,
            null,
            null,
            relayAuth,
        )
        val signature = Ed25519Signer().run {
            init(true, signer)
            update(preimage, 0, preimage.size)
            generateSignature()
        }

        fun verifies(
            network: org.hyperledger.iroha.sdk.core.model.NetworkId = networkId,
            accountId: String = account,
            relay: ByteArray = relayAuth,
            sig: ByteArray = signature,
        ): Boolean = ConnectCrypto.verifyApprovalSignature(
            network,
            sid,
            appPublic,
            walletPublic,
            accountId,
            null,
            null,
            relay,
            "ed25519",
            sig,
        )

        assertTrue(verifies())
        assertTrue(!verifies(network = TestNetworkIds.fromSeed(7)))
        assertTrue(!verifies(accountId = sampleI105(0x45)))
        assertTrue(!verifies(relay = ConnectCrypto.relayAuthHash(sid, "other-relay")))
        assertTrue(!verifies(sig = signature.copyOf().also { it[0] = (it[0].toInt() xor 1).toByte() }))
        assertTrue(!ConnectCrypto.verifyApprovalSignature(
            networkId, sid, appPublic, walletPublic, account, null, null, relayAuth,
            "Ed25519", signature,
        ))
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
        .fromAccount(TestEd25519Keys.publicKey(fill), "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
}
