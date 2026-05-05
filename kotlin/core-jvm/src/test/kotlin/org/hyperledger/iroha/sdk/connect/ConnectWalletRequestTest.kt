package org.hyperledger.iroha.sdk.connect

import java.net.URI
import java.net.URLEncoder
import java.security.MessageDigest
import java.util.Base64
import kotlin.test.Test
import kotlin.test.assertEquals

class ConnectWalletRequestTest {
    @Test
    fun acceptsIrohaconnectLaunchUri() {
        val request = ConnectWalletRequest.parse(
            "irohaconnect://connect?sid=${sampleSid()}&chain_id=taira-testnet&node=taira.sora.org&token=wallet-token&relay=relay-token",
            URI("https://default.sora.org"),
        )

        assertEquals(sampleSid(), request.sidBase64Url)
        assertEquals("wallet-token", request.token)
        assertEquals("relay-token", request.relayToken)
        assertEquals("taira-testnet", request.chainId)
        assertEquals("https://taira.sora.org", request.baseUri.toString())
        assertEquals(
            "wss://taira.sora.org/v1/connect/ws?sid=${sampleSid()}&role=wallet",
            request.webSocketUri.toString(),
        )
    }

    @Test
    fun acceptsWrappedIrohaconnectLaunchUri() {
        val embeddedUri =
            "irohaconnect://connect?sid=${sampleSid()}&chain_id=taira-testnet&token=wallet-token&relay=relay-token"
        val request = ConnectWalletRequest.parse(
            "irohaconnect://wc?uri=${URLEncoder.encode(embeddedUri, Charsets.UTF_8.name())}",
            URI("https://taira.sora.org"),
        )

        assertEquals(sampleSid(), request.sidBase64Url)
        assertEquals("wallet-token", request.token)
        assertEquals("relay-token", request.relayToken)
        assertEquals("taira-testnet", request.chainId)
        assertEquals("https://taira.sora.org", request.baseUri.toString())
    }

    @Test
    fun derivesRelayAuthHash() {
        val sid = ByteArray(32) { it.toByte() }
        val expected = MessageDigest.getInstance("SHA-256").digest(
            "iroha-connect|relay-auth|v1".toByteArray(Charsets.UTF_8) +
                sid +
                "relay-token".toByteArray(Charsets.UTF_8),
        )

        assertEquals(
            Base64.getEncoder().encodeToString(expected),
            Base64.getEncoder().encodeToString(ConnectCrypto.relayAuthHash(sid, "relay-token")),
        )
    }

    @Test
    fun relayAuthHashMatchesSharedFixture() {
        val sid = ByteArray(32) { it.toByte() }

        assertEquals(
            "65de07a9c6110f16b6b7c64e63c71437d88d122344e1a67d2c932a16187cce2f",
            ConnectCrypto.relayAuthHash(sid, "relay-token-vector").toHex(),
        )
    }

    private fun sampleSid(): String =
        Base64.getUrlEncoder().withoutPadding().encodeToString(ByteArray(32))

    private fun ByteArray.toHex(): String = joinToString("") { "%02x".format(it.toInt() and 0xff) }
}
