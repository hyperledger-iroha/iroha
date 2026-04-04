package org.hyperledger.iroha.sdk.connect

import java.net.URI
import java.net.URLEncoder
import java.util.Base64
import kotlin.test.Test
import kotlin.test.assertEquals

class ConnectWalletRequestTest {
    @Test
    fun acceptsIrohaconnectLaunchUri() {
        val request = ConnectWalletRequest.parse(
            "irohaconnect://connect?sid=${sampleSid()}&chain_id=taira-testnet&node=taira.sora.org&token=wallet-token",
            URI("https://default.sora.org"),
        )

        assertEquals(sampleSid(), request.sidBase64Url)
        assertEquals("wallet-token", request.token)
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
            "irohaconnect://connect?sid=${sampleSid()}&chain_id=taira-testnet&token=wallet-token"
        val request = ConnectWalletRequest.parse(
            "irohaconnect://wc?uri=${URLEncoder.encode(embeddedUri, Charsets.UTF_8.name())}",
            URI("https://taira.sora.org"),
        )

        assertEquals(sampleSid(), request.sidBase64Url)
        assertEquals("wallet-token", request.token)
        assertEquals("taira-testnet", request.chainId)
        assertEquals("https://taira.sora.org", request.baseUri.toString())
    }

    private fun sampleSid(): String =
        Base64.getUrlEncoder().withoutPadding().encodeToString(ByteArray(32))
}
