package org.hyperledger.iroha.sdk.connect

import java.net.URI
import java.net.URLEncoder
import java.security.MessageDigest
import java.util.Base64
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.testing.TestNetworkIds

class ConnectWalletRequestTest {
    private val networkId = TestNetworkIds.canonical()
    private val appPublicKey = ByteArray(32) { (it + 1).toByte() }
    private val nonce = ByteArray(16) { (it + 65).toByte() }
    private val sid = ConnectCrypto.deriveSessionId(networkId, appPublicKey, nonce)

    @Test
    fun acceptsOnlyCanonicalLaunchIdentity() {
        val request = parseRequest()

        assertEquals(b64(sid), request.sidBase64Url)
        assertEquals("wallet-token", request.token)
        assertEquals("relay-token", request.relayToken)
        assertEquals(networkId, request.networkId)
        assertContentEquals(appPublicKey, request.appPublicKey())
        assertContentEquals(nonce, request.nonce())
        assertEquals("https://taira.sora.org", request.baseUri.toString())
        assertEquals(
            "wss://taira.sora.org/v1/connect/ws?sid=${b64(sid)}&role=wallet",
            request.webSocketUri.toString(),
        )
    }

    @Test
    fun rejectsWrongNetworkSidAndAppKeySubstitution() {
        val otherNetwork = TestNetworkIds.fromSeed(77)
        assertFailsWith<ConnectProtocolException> {
            ConnectWalletRequest.parse(
                canonicalUri(network = otherNetwork),
                URI("https://default.sora.org"),
            )
        }
        assertFailsWith<ConnectProtocolException> {
            ConnectWalletRequest.parse(
                canonicalUri(sidBytes = sid.copyOf().also { it[0] = (it[0].toInt() xor 1).toByte() }),
                URI("https://default.sora.org"),
            )
        }
        assertFailsWith<ConnectProtocolException> {
            ConnectWalletRequest.parse(
                canonicalUri(appKey = appPublicKey.copyOf().also {
                    it[0] = (it[0].toInt() xor 1).toByte()
                }),
                URI("https://default.sora.org"),
            )
        }
    }

    @Test
    fun rejectsDuplicateAndRetiredLaunchParameters() {
        val uri = canonicalUri()
        assertFailsWith<ConnectProtocolException> {
            ConnectWalletRequest.parse("$uri&sid=${b64(sid)}", URI("https://default.sora.org"))
        }
        assertFailsWith<ConnectProtocolException> {
            ConnectWalletRequest.parse(
                uri.replace("iroha://", "irohaconnect://"),
                URI("https://default.sora.org"),
            )
        }
        assertFailsWith<ConnectProtocolException> {
            ConnectWalletRequest.parse(
                "$uri&chain_id=taira-testnet",
                URI("https://default.sora.org"),
            )
        }
        assertFailsWith<ConnectProtocolException> {
            ConnectWalletRequest.parse(
                uri.replace("token=wallet-token", "token_wallet=wallet-token"),
                URI("https://default.sora.org"),
            )
        }
    }

    @Test
    fun openIsBoundToLaunchAndCannotBeReplayed() {
        val request = parseRequest()
        val openFrame = ConnectFrameCodec.encodeOpenFrame(sid, appPublicKey, networkId)
        val open = request.acceptOpen(openFrame)

        assertEquals(networkId, open.networkId)
        assertContentEquals(appPublicKey, open.appPublicKey())
        assertFailsWith<ConnectProtocolException> { request.acceptOpen(openFrame) }

        val wrongNetworkRequest = parseRequest()
        assertFailsWith<ConnectProtocolException> {
            wrongNetworkRequest.acceptOpen(
                ConnectFrameCodec.encodeOpenFrame(sid, appPublicKey, TestNetworkIds.fromSeed(7)),
            )
        }
        val wrongAppRequest = parseRequest()
        assertFailsWith<ConnectProtocolException> {
            wrongAppRequest.acceptOpen(
                ConnectFrameCodec.encodeOpenFrame(
                    sid,
                    appPublicKey.copyOf().also { it[0] = (it[0].toInt() xor 1).toByte() },
                    networkId,
                ),
            )
        }
        val wrongSidRequest = parseRequest()
        assertFailsWith<ConnectProtocolException> {
            wrongSidRequest.acceptOpen(
                ConnectFrameCodec.encodeOpenFrame(
                    sid.copyOf().also { it[0] = (it[0].toInt() xor 1).toByte() },
                    appPublicKey,
                    networkId,
                ),
            )
        }
    }

    @Test
    fun approvalRequiresAcceptedOpenAndBindsRelayAuthorization() {
        val request = parseRequest()
        val walletPublicKey = ByteArray(32) { (it + 99).toByte() }
        val account = org.hyperledger.iroha.sdk.address.AccountAddress
            .fromAccount(org.hyperledger.iroha.sdk.testing.TestEd25519Keys.publicKey(0x44), "ed25519")
            .toI105(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT)

        assertFailsWith<ConnectProtocolException> {
            request.buildApprovePreimage(walletPublicKey, account, null, null)
        }
        request.acceptOpen(ConnectFrameCodec.encodeOpenFrame(sid, appPublicKey, networkId))
        val preimage = request.buildApprovePreimage(walletPublicKey, account, null, null)
        val expectedRelay = ConnectCrypto.relayAuthHash(sid, "relay-token")

        assertTrue(preimage.asList().windowed(expectedRelay.size).any { it == expectedRelay.asList() })
    }

    @Test
    fun derivesRelayAuthHash() {
        val expected = MessageDigest.getInstance("SHA-256").digest(
            "iroha-connect|relay-auth|v1".toByteArray(Charsets.UTF_8) +
                sid +
                "relay-token".toByteArray(Charsets.UTF_8),
        )

        assertContentEquals(expected, ConnectCrypto.relayAuthHash(sid, "relay-token"))
    }

    @Test
    fun relayAuthHashMatchesSharedFixture() {
        val fixtureSid = ByteArray(32) { it.toByte() }

        assertEquals(
            "65de07a9c6110f16b6b7c64e63c71437d88d122344e1a67d2c932a16187cce2f",
            ConnectCrypto.relayAuthHash(fixtureSid, "relay-token-vector").toHex(),
        )
    }

    private fun parseRequest(): ConnectWalletRequest = ConnectWalletRequest.parse(
        canonicalUri(),
        URI("https://default.sora.org"),
    )

    private fun canonicalUri(
        network: org.hyperledger.iroha.sdk.core.model.NetworkId = networkId,
        sidBytes: ByteArray = sid,
        appKey: ByteArray = appPublicKey,
    ): String = "iroha://connect?" + listOf(
        "sid=${b64(sidBytes)}",
        "network_id=${url(network.literal)}",
        "app_pk=${b64(appKey)}",
        "nonce=${b64(nonce)}",
        "node=taira.sora.org",
        "v=1",
        "role=wallet",
        "token=wallet-token",
        "relay=relay-token",
    ).joinToString("&")

    private fun b64(value: ByteArray): String =
        Base64.getUrlEncoder().withoutPadding().encodeToString(value)

    private fun url(value: String): String = URLEncoder.encode(value, Charsets.UTF_8.name())

    private fun ByteArray.toHex(): String = joinToString("") { "%02x".format(it.toInt() and 0xff) }
}
