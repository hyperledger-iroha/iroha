package org.hyperledger.iroha.sdk.client.transport

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

        val response = TransportResponse(200, null, null, source)
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
}
