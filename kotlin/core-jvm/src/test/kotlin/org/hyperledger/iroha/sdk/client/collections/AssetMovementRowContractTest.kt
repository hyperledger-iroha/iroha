package org.hyperledger.iroha.sdk.client.collections

import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.concurrent.CancellationException
import java.util.concurrent.CompletableFuture
import org.hyperledger.iroha.sdk.client.ClientConfig
import org.hyperledger.iroha.sdk.client.HttpClientTransport
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import org.hyperledger.iroha.sdk.client.LocalSigningContext
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.json.Json
import org.hyperledger.iroha.sdk.json.JsonObject
import org.hyperledger.iroha.sdk.query.Filter
import org.hyperledger.iroha.sdk.query.listQuery
import org.hyperledger.iroha.sdk.testing.TestNetworkIds
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class AssetMovementRowContractTest {
    private val definition = "5HFTUK41MFWnFdeHo5XySHC7PQun"
    private val hash = "cc9ce14e2c0a848abb93cd5d2ee03a0701ce5b9930f38434e7e8834898b558b5"

    /** A complete row as `routing/asset_movements.rs` writes it. */
    private fun row(vararg changes: Pair<String, String?>): String {
        val fields = linkedMapOf(
            "amount" to "\"0.5\"",
            "asset_definition_id" to "\"$definition\"",
            "authority" to "\"payer\"",
            "block_height" to "5503",
            "block_index" to "0",
            "from_account_id" to "\"payer\"",
            "id" to "\"$hash:0\"",
            "kind" to "\"OFFLINE_LOAD\"",
            "movement_index" to "0",
            "result_ok" to "true",
            "timestamp_ms" to "1793147618051",
            "to_account_id" to "\"reserve\"",
            "tx_hash" to "\"$hash\"",
        )
        for ((name, value) in changes) if (value == null) fields.remove(name) else fields[name] = value
        return fields.entries.joinToString(",", "{", "}") { (name, value) -> "\"$name\":$value" }
    }

    private fun decode(json: String) = AssetMovementRow.fromJson(Json.parse(json) as JsonObject)

    @Test fun offlineMovementKeepsItsKindExactAmountAndCoordinates() {
        val load = decode(row())
        assertEquals(AssetMovementRow.Kind.OFFLINE_LOAD, load.kind)
        assertTrue(load.kind.isOffline)
        assertEquals("0.5", load.amount)
        assertEquals("payer", load.fromAccountId); assertEquals("reserve", load.toAccountId)
        assertEquals(5503L, load.blockHeight); assertEquals(0L, load.blockIndex); assertEquals(0L, load.movementIndex)
        assertEquals(1793147618051L, load.timestampMs)
        assertEquals("$hash:0", load.id)
        assertTrue(load.resultOk)
        val mint = decode(row("kind" to "\"MINT\"", "from_account_id" to "null", "amount" to "\"312\""))
        assertNull(mint.fromAccountId); assertFalse(mint.kind.isOffline)
        val burn = decode(row("kind" to "\"BURN\"", "to_account_id" to "null"))
        assertNull(burn.toAccountId)
        val failedTransfer = decode(row("kind" to "\"TRANSFER\"", "result_ok" to "false", "id" to "\"$hash:2\"", "movement_index" to "2"))
        assertFalse(failedTransfer.resultOk)
        // Fields Torii adds later stay in the JSON and change nothing else.
        val extended = decode(row("note" to "\"later\""))
        assertEquals(Json.of("later"), extended.json["note"])
        assertEquals(load.amount, extended.amount)
    }

    @Test fun everyKindTheIndexWritesDecodes() {
        val kinds = listOf("TRANSFER", "MINT", "BURN", "OFFLINE_LOAD", "OFFLINE_UNLOAD", "OFFLINE_FEE", "OFFLINE_CHARGE")
        assertEquals(kinds, AssetMovementRow.Kind.entries.map { it.name })
        for (kind in kinds) {
            val decoded = decode(row("kind" to "\"$kind\"",
                "from_account_id" to (if (kind == "MINT") "null" else "\"payer\""),
                "to_account_id" to (if (kind == "BURN") "null" else "\"reserve\"")))
            assertEquals(kind, decoded.kind.name)
            assertEquals(kind.startsWith("OFFLINE_"), decoded.kind.isOffline)
        }
    }

    @Test fun malformedRowsAreRejectedAtTheSharedBoundary() {
        for (changes in listOf(
            arrayOf("kind" to "\"SWAP\""),
            arrayOf("kind" to null),
            arrayOf("amount" to "\"0.50\""),
            arrayOf("amount" to "\"01\""),
            arrayOf("amount" to "\"-1\""),
            arrayOf("amount" to "\"1e3\""),
            arrayOf("amount" to "0.5"),
            arrayOf("from_account_id" to "null"),
            arrayOf("from_account_id" to null),
            arrayOf("from_account_id" to "\"\""),
            arrayOf("to_account_id" to null),
            arrayOf("kind" to "\"MINT\""),
            arrayOf("kind" to "\"BURN\""),
            arrayOf("tx_hash" to "\"${hash.uppercase()}\"", "id" to "\"${hash.uppercase()}:0\""),
            arrayOf("id" to "\"$hash:1\""),
            arrayOf("id" to "\"other\""),
            arrayOf("block_height" to "0"),
            arrayOf("movement_index" to "-1"),
            arrayOf("timestamp_ms" to null),
            arrayOf("result_ok" to null),
            arrayOf("result_ok" to "false"),
            arrayOf("authority" to "\"\""),
            arrayOf("asset_definition_id" to null),
        )) {
            assertFailsWith<IllegalArgumentException>(changes.joinToString()) { decode(row(*changes)) }
        }
    }

    @Test fun oneAccountReadsItsMovementsWithAFilteredHistoryQuery() {
        val executor = ScriptedExecutor()
        val client = HttpClientTransport(executor, ClientConfig.builder()
            .setBaseUri(URI("https://torii.example/api"))
            .setLocalSigningContext(LocalSigningContext(TestNetworkIds.canonical()))
            .build())
        executor.reply(200, """{"items":[${row()}],"next_cursor":"q1_older"}""")
        val movements = client.assetMovements(definition)
        val account = Filter.field("from_account_id").eq("payer") or Filter.field("to_account_id").eq("payer")
        val page = movements.page(listQuery { filter(account); limit(50) }).join()
        assertEquals(AssetMovementRow.Kind.OFFLINE_LOAD, page.items.single().kind)
        assertEquals("q1_older", page.nextCursor)
        val request = executor.requests.single()
        assertEquals("POST", request.method)
        assertEquals("/api/v1/assets/$definition/movements/query", request.uri.path)
        assertEquals(
            """{"filter":{"op":"or","args":[{"op":"eq","args":["from_account_id","payer"]},""" +
                """{"op":"eq","args":["to_account_id","payer"]}]},"limit":50}""",
            String(request.body, StandardCharsets.UTF_8),
        )
        // A history collection: Torii's order only, rejected before anything is sent.
        assertFailsWith<IllegalArgumentException> { movements.page(listQuery { sort("-block_height") }).join() }
        assertFailsWith<IllegalArgumentException> { movements.page(listQuery { includeTotal() }).join() }
        assertEquals(1, executor.requests.size)
        assertFailsWith<IllegalArgumentException> { client.assetMovements("") }
    }

    /** Answers requests synchronously from a queue of scripted responses. */
    private class ScriptedExecutor : HttpTransportExecutor {
        val requests = ArrayList<TransportRequest>()
        private val responses = ArrayDeque<TransportResponse>()

        fun reply(status: Int, body: String) {
            responses.addLast(
                TransportResponse.builder()
                    .setStatusCode(status)
                    .setBody(body.toByteArray(StandardCharsets.UTF_8))
                    .setHeaders(mapOf("Content-Type" to listOf("application/json")))
                    .build(),
            )
        }

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requests.add(request)
            val response = responses.removeFirstOrNull()
                ?: return CompletableFuture<TransportResponse>().also {
                    it.completeExceptionally(CancellationException("no scripted response"))
                }
            return CompletableFuture.completedFuture(response)
        }
    }
}
