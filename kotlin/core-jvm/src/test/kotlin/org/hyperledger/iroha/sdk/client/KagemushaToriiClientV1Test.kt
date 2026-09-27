// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

/** A missing operation is retryable only under Torii's exact application-resource code. */
class KagemushaToriiClientV1Test {
    private val operationID = ByteArray(32) { 0x4a }

    @Test
    fun operationLookupDistinguishesExactResourceAbsenceFromOther404Responses() {
        val missing = response(404, "kagemusha_operation_not_found", "kagemusha_operation_not_found")
        val (client, executor) = client(missing)
        assertNull(client.getOperation(operationID).join())
        assertEquals("GET", executor.request.method)
        assertEquals(
            "/v1/kagemusha/operations/${"4a".repeat(32)}",
            executor.request.uri.path,
        )

        for (untrusted in listOf(
            response(404, null, "kagemusha_operation_not_found"),
            response(404, "route_not_found", "route_not_found"),
            response(404, "kagemusha_operation_not_found", "route_not_found"),
            response(404, "kagemusha_operation_not_found", "kagemusha_operation_not_found", "text/plain"),
            response(404, "kagemusha_operation_not_found", "kagemusha_operation_not_found",
                jsonBody = """{"code":"kagemusha_operation_not_found","message":"unknown","details":null}"""),
            response(404, "kagemusha_operation_not_found", "kagemusha_operation_not_found",
                jsonBody = " ".repeat(2_049) + """{"code":"kagemusha_operation_not_found","message":"unknown"}"""),
        )) {
            assertFailsWith<CompletionException> {
                client(untrusted).first.getOperation(operationID).join()
            }
        }
    }

    private fun client(response: TransportResponse): Pair<KagemushaToriiClientV1, FixedExecutor> {
        val executor = FixedExecutor(response)
        return KagemushaToriiClientV1.builder()
            .baseUri(URI.create("https://torii.example/"))
            .executor(executor)
            .build() to executor
    }

    private fun response(status: Int, headerCode: String?, bodyCode: String,
                         contentType: String = "application/json", jsonBody: String =
                             """{"code":"$bodyCode","message":"Operation is unknown."}"""): TransportResponse {
        val body = jsonBody.toByteArray(StandardCharsets.UTF_8)
        val response = TransportResponse.builder().setStatusCode(status)
            .setBody(body).addHeader("Content-Type", contentType)
        if (headerCode != null) response.addHeader("X-Iroha-Reject-Code", headerCode)
        return response.build()
    }

    private class FixedExecutor(private val response: TransportResponse) : HttpTransportExecutor {
        lateinit var request: TransportRequest

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            this.request = request
            return CompletableFuture.completedFuture(response)
        }
    }
}
