package org.hyperledger.iroha.sdk.client.transport

import java.net.URI
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class TransportResponseTest {
    @Test
    fun headersRemainCaseInsensitiveAfterDefensiveCopy() {
        val source = linkedMapOf(
            "Content-Type" to listOf("application/json"),
            "content-type" to listOf("application/x-norito"),
        )

        val response = TransportResponse(200, null, null, source, null, false)
        source.clear()

        assertEquals(
            listOf("application/json", "application/x-norito"),
            response.headers["CONTENT-TYPE"],
        )
        assertFailsWith<UnsupportedOperationException> {
            @Suppress("UNCHECKED_CAST")
            (response.headers as MutableMap<String, List<String>>)["X-Test"] = listOf("changed")
        }
    }

    @Test
    fun networkProvenanceIsAlwaysExplicit() {
        val provenanceUnavailable =
            TransportResponse(200, null, null, emptyMap(), null, false)
        assertEquals(null, provenanceUnavailable.finalUri)
        assertEquals(false, provenanceUnavailable.redirected)

        val finalUri = URI.create("https://torii.example/v1/validation-fee/hijiri/quote")
        val response = TransportResponse.builder()
            .setStatusCode(200)
            .setNetworkProvenance(finalUri, redirected = true)
            .build()

        assertEquals(finalUri, response.finalUri)
        assertEquals(true, response.redirected)
    }
}
