package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

class ContractManifestTest {
    @Test
    fun fullManifestPreservesExactKotodamaV1Interface() {
        val record = ContractJsonParser.parseManifestRecord(fullResponse().toByteArray(StandardCharsets.UTF_8))
        val manifest = record.manifest

        assertEquals("Ledger", manifest.seiyakuName)
        assertEquals("b".repeat(64), manifest.codeHashHex)
        assertEquals("d".repeat(64), manifest.abiHashHex)
        assertEquals(64, manifest.accessSetHints!!.dynamicReads.single().maxKeys)
        val entrypoint = manifest.entrypoints!!.single()
        assertEquals(ContractEntrypointKind.KOTOAGE, entrypoint.kind)
        val argumentSchema = entrypoint.argumentSchema ?: error("missing argument schema")
        val returnSchema = entrypoint.returnSchema ?: error("missing return schema")
        assertEquals(2, argumentSchema.fields.first().valueType.wordCount)
        assertEquals("struct Transfer", argumentSchema.fields.first().valueType.canonicalTypeName)
        val tagsType = argumentSchema.fields.last().valueType
        assertEquals(2, tagsType.nodes.size)
        assertEquals(64, tagsType.nodes.first().listValue!!.capacity)
        assertEquals("List<Name, 64>", tagsType.canonicalTypeName)
        assertEquals(1, returnSchema.wordCount)
        assertEquals("Result<(bool, u128), string>", returnSchema.canonicalTypeName)
        assertEquals(ContractTriggerRepeatsKind.INDEFINITELY, entrypoint.triggers.single().repeats.kind)
        assertNull(entrypoint.triggers.single().repeats.exactly)
        assertEquals("transfer", entrypoint.triggers.single().callback.entrypoint)
        assertEquals("daily-settlement", entrypoint.triggers.single().metadata["purpose"])
        assertEquals("StateMap<AccountId, Amount>", manifest.states!!.single().typeName)
        assertEquals(1001, manifest.errorCodes!!.single().code)
        assertEquals("ja", manifest.kotoba!!.single().translations.last().language)
        assertEquals("ed25519:fixture", manifest.provenance!!.signer)
    }

    @Test
    fun manifestRejectsUnknownEnglishAndNoncanonicalShapes() {
        val invalid = listOf(
            fullResponse().replaceFirst("\"seiyaku_name\"", "\"contract_name\""),
            fullResponse().replaceFirst("\"Kotoage\"", "\"Public\""),
            fullResponse().replaceFirst("\"Kotoage\"", "\"View\""),
            fullResponse().replaceFirst("\"capacity\":64", "\"capacity\":65"),
            fullResponse().replaceFirst("\"name\":\"request\",\"ty\"", "\"name\":\"wrong\",\"ty\""),
            fullResponse().replaceFirst("#ABA2", "#0000"),
            fullResponse().replaceFirst("\"seiyaku_name\":\"Ledger\"", "\"seiyaku_name\":\"match\""),
            fullResponse().replaceFirst("\"seiyaku_name\":\"Ledger\"", "\"seiyaku_name\":\"Option\""),
            fullResponse().replaceFirst("\"seiyaku_name\":\"Ledger\"", "\"seiyaku_name\":\"__kotodama_link_private\""),
            fullResponse().replaceFirst("\"seiyaku_name\":\"Ledger\"", "\"seiyaku_name\":\"state_map_get\""),
            fullResponse().replaceFirst("\"namespace\":\"TransferError\"", "\"namespace\":\"Option\""),
            fullResponse().replaceFirst("\"features_bitmap\":0", "\"features_bitmap\":4"),
            fullResponse().replaceFirst("\"dynamic_writes\":[]", "\"dynamic_writes\":[],\"unknown\":true"),
            fullResponse().replaceFirst(
                "\"repeats\":{\"Indefinitely\":null}",
                "\"repeats\":{\"kind\":\"Indefinitely\",\"value\":null}",
            ),
            fullResponse().replaceFirst("\"code_hash\":\"${"b".repeat(64)}\"", "\"code_hash\":\"${"f".repeat(64)}\""),
        )

        invalid.forEach { payload ->
            assertFailsWith<IllegalStateException>(payload) {
                ContractJsonParser.parseManifestRecord(payload.toByteArray(StandardCharsets.UTF_8))
            }
        }
    }

    @Test
    fun manifestEndpointValidatesPathAndParsesFullRecord() {
        val executor = ManifestExecutor(fullResponse().toByteArray(StandardCharsets.UTF_8))
        val transport = HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val record = transport.getContractManifest("b".repeat(64)).join()

        assertEquals("Ledger", record.manifest.seiyakuName)
        assertEquals(
            "https://torii.example/api/v1/contracts/code/${"b".repeat(64)}",
            executor.lastRequest.uri.toString(),
        )
        val requests = executor.requestCount
        assertFailsWith<IllegalArgumentException> { transport.getContractManifest("abc") }
        assertFailsWith<IllegalArgumentException> { transport.getContractManifest("0x${"b".repeat(64)}") }
        assertEquals(requests, executor.requestCount)
    }

    @Test
    fun flatListTapeEnforcesTheExactV1DepthBoundary() {
        val listNode = """{"kind":"List","value":{"capacity":1}},"""
        val leafNode = """{"kind":"Leaf","value":{"kind":"Int","value":null}}"""
        val validNodes = listNode.repeat(255) + leafNode
        var validTypeName = "i64"
        repeat(255) { validTypeName = "List<$validTypeName, 1>" }

        val valid = parseBoundarySchema(validNodes, validTypeName)
        assertEquals(256, valid.nodes.size)
        assertEquals(1, valid.wordCount)
        assertEquals(validTypeName, valid.canonicalTypeName)

        val malformed = listOf(
            listNode.dropLast(1),
            "$leafNode,$leafNode",
            """{"kind":"List","value":{"capacity":1,"element":{"nodes":[$leafNode]}}},$leafNode""",
            """{"kind":"List","value":{"capacity":0}},$leafNode""",
            """{"kind":"List","value":{"capacity":65}},$leafNode""",
            optionNode,
            """{"kind":"Result","value":null},$leafNode""",
            """{"kind":"Tuple","value":2},$leafNode""",
            listNode.repeat(256) + leafNode,
        )
        malformed.forEach { nodes ->
            assertFailsWith<IllegalStateException> {
                parseBoundarySchema(nodes, "i64")
            }
        }
    }

    @Test
    fun reservedQueryNominalsRequireTheirExactFlatShape() {
        val nodes =
            """{"kind":"Struct","value":{"name":"QueryPage","fields":["items","next_offset"]}},""" +
                """{"kind":"List","value":{"capacity":64}},""" +
                """{"kind":"Struct","value":{"name":"AccountView","fields":["id","metadata"]}},""" +
                """{"kind":"Leaf","value":{"kind":"AccountId","value":null}},""" +
                """{"kind":"Leaf","value":{"kind":"Json","value":null}},""" +
                """{"kind":"Option","value":null},""" +
                """{"kind":"Leaf","value":{"kind":"Int","value":null}}"""

        val schema = parseBoundarySchema(nodes, "QueryPage<AccountView>")
        assertEquals("QueryPage<AccountView>", schema.canonicalTypeName)

        listOf(
            nodes.replaceFirst("\"capacity\":64", "\"capacity\":63"),
            nodes.replaceFirst("\"kind\":\"Json\"", "\"kind\":\"String\""),
        ).forEach { forged ->
            assertFailsWith<IllegalStateException> {
                parseBoundarySchema(forged, "QueryPage<AccountView>")
            }
        }
    }

    @Test
    fun everyReservedProjectionAndPageHasAnExactNominalName() {
        val pair = listOf(
            structNode("Pair", "left", "right"),
            leafNode("Int"),
            leafNode("Bool"),
        )
        assertEquals(
            "struct Pair",
            parseBoundarySchema(pair.joinToString(","), "struct Pair").canonicalTypeName,
        )

        coreViewNames.forEach { viewName ->
            val view = coreViewNodes(viewName)
            assertEquals(
                viewName,
                parseBoundarySchema(view.joinToString(","), viewName).canonicalTypeName,
            )
            val pageName = "QueryPage<$viewName>"
            assertEquals(
                pageName,
                parseBoundarySchema(queryPageNodes(view).joinToString(","), pageName).canonicalTypeName,
            )
        }
    }

    @Test
    fun everyReservedProjectionAndPageRejectsForgedStructure() {
        val forgedViews = listOf(
            "AccountView" to listOf(
                structNode("AccountView", "id", "metadata"),
                leafNode("AccountId"),
                leafNode("Bool"),
            ),
            "AssetView" to listOf(
                structNode("AssetView", "id", "amount"),
                leafNode("AssetId"),
                leafNode("U128"),
            ),
            "AssetDefinitionView" to listOf(
                structNode(
                    "AssetDefinitionView",
                    "id",
                    "name",
                    "description",
                    "owned_by",
                    "total_quantity",
                    "metadata",
                ),
                leafNode("AssetDefinitionId"),
                leafNode("String"),
                optionNode,
                leafNode("Bool"),
                leafNode("AccountId"),
                leafNode("Amount"),
                leafNode("Json"),
            ),
            "DomainView" to listOf(
                structNode("DomainView", "id", "owned_by", "metadata"),
                leafNode("DomainId"),
                leafNode("DomainId"),
                leafNode("Json"),
            ),
            "NftView" to listOf(
                structNode("NftView", "id", "owned_by", "content"),
                leafNode("NftId"),
                leafNode("AccountId"),
                leafNode("String"),
            ),
        )
        forgedViews.forEach { (typeName, nodes) ->
            assertCanonicalSchemaFailure(typeName, nodes)
        }

        val account = coreViewNodes("AccountView")
        val page = queryPageNodes(account)
        assertCanonicalSchemaFailure(
            "QueryPage<AccountView>",
            page.mapIndexed { index, node -> if (index == 1) listNode(63) else node },
        )
        assertCanonicalSchemaFailure(
            "QueryPage<AccountView>",
            page.mapIndexed { index, node ->
                if (index == page.lastIndex) leafNode("Bool") else node
            },
        )
        assertCanonicalSchemaFailure(
            "QueryPage<AccountView>",
            listOf(structNode("QueryPage", "next_offset", "items")) + page.drop(1),
        )
        assertCanonicalSchemaFailure(
            "struct QueryPage",
            listOf(
                structNode("QueryPage", "items", "next_offset"),
                listNode(64),
                structNode("Pair", "left", "right"),
                leafNode("Int"),
                leafNode("Bool"),
                optionNode,
                leafNode("Int"),
            ),
        )
    }

    private class ManifestExecutor(private val payload: ByteArray) : HttpTransportExecutor {
        lateinit var lastRequest: TransportRequest
        var requestCount = 0

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requestCount += 1
            lastRequest = request
            return CompletableFuture.completedFuture(
                TransportResponse.builder().setStatusCode(200).setBody(payload).build(),
            )
        }
    }

    companion object {
        private val triggerFilter =
            "TlJUMAAAl9+YQQ4oJZjALRf6FAto0QAKAAAAAAAAANzCjydU9+jNAgIAAAAFBAAAAAA="

        private fun parseBoundarySchema(
            nodes: String,
            typeName: String,
        ): EntrypointValueTypeV1 {
            val payload =
                """{"manifest":{"entrypoints":[{"name":"inspect","kind":{"kind":"View","value":null},"params":[{"name":"value","type_name":"$typeName"}],"argument_schema":{"fields":[{"name":"value","ty":{"nodes":[$nodes]}}]}}]}}"""
            return ContractJsonParser.parseManifestRecord(payload.toByteArray(StandardCharsets.UTF_8))
                .manifest.entrypoints!!.single().argumentSchema!!.fields.single().valueType
        }

        private val coreViewNames = listOf(
            "AccountView",
            "AssetView",
            "AssetDefinitionView",
            "DomainView",
            "NftView",
        )

        private const val optionNode = """{"kind":"Option","value":null}"""

        private fun listNode(capacity: Int): String =
            """{"kind":"List","value":{"capacity":$capacity}}"""

        private fun leafNode(kind: String): String =
            """{"kind":"Leaf","value":{"kind":"$kind","value":null}}"""

        private fun structNode(name: String, vararg fields: String): String {
            val fieldJson = fields.joinToString(",") { "\"$it\"" }
            return """{"kind":"Struct","value":{"name":"$name","fields":[$fieldJson]}}"""
        }

        private fun coreViewNodes(name: String): List<String> = when (name) {
            "AccountView" -> listOf(
                structNode(name, "id", "metadata"),
                leafNode("AccountId"),
                leafNode("Json"),
            )
            "AssetView" -> listOf(
                structNode(name, "id", "amount"),
                leafNode("AssetId"),
                leafNode("Amount"),
            )
            "AssetDefinitionView" -> listOf(
                structNode(name, "id", "name", "description", "owned_by", "total_quantity", "metadata"),
                leafNode("AssetDefinitionId"),
                leafNode("String"),
                optionNode,
                leafNode("String"),
                leafNode("AccountId"),
                leafNode("Amount"),
                leafNode("Json"),
            )
            "DomainView" -> listOf(
                structNode(name, "id", "owned_by", "metadata"),
                leafNode("DomainId"),
                leafNode("AccountId"),
                leafNode("Json"),
            )
            "NftView" -> listOf(
                structNode(name, "id", "owned_by", "content"),
                leafNode("NftId"),
                leafNode("AccountId"),
                leafNode("Json"),
            )
            else -> error("unsupported test view $name")
        }

        private fun queryPageNodes(view: List<String>): List<String> =
            listOf(structNode("QueryPage", "items", "next_offset"), listNode(64)) +
                view +
                listOf(optionNode, leafNode("Int"))

        private fun assertCanonicalSchemaFailure(typeName: String, nodes: List<String>) {
            val error = assertFailsWith<IllegalStateException> {
                parseBoundarySchema(nodes.joinToString(","), typeName)
            }
            assertTrue(error.message.orEmpty().contains("canonical flat preorder"), error.message)
        }

        private fun fullResponse(): String =
            """
            {
              "manifest":{
                "seiyaku_name":"Ledger",
                "code_hash":"hash:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB#ABA2",
                "abi_hash":"hash:DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD#F071",
                "compiler_fingerprint":"kotodama_lang",
                "features_bitmap":0,
                "access_set_hints":{
                  "read_keys":["state:Balances"],
                  "write_keys":["state:Balances"],
                  "dynamic_reads":[{
                    "base_key":"state:Balances",
                    "key_type":"AccountId",
                    "bound_kind":"take",
                    "max_keys":64
                  }],
                  "dynamic_writes":[]
                },
                "entrypoints":[{
                  "name":"transfer",
                  "kind":{"kind":"Kotoage","value":null},
                  "params":[
                    {"name":"request","type_name":"struct Transfer"},
                    {"name":"tags","type_name":"List<Name, 64>"}
                  ],
                  "argument_schema":{"fields":[
                    {"name":"request","ty":{"nodes":[
                      {"kind":"Struct","value":{"name":"Transfer","fields":["amount","memo"]}},
                      {"kind":"Leaf","value":{"kind":"Amount","value":null}},
                      {"kind":"Option","value":null},
                      {"kind":"Leaf","value":{"kind":"String","value":null}}
                    ]}},
                    {"name":"tags","ty":{"nodes":[
                      {"kind":"List","value":{"capacity":64}},
                      {"kind":"Leaf","value":{"kind":"Name","value":null}}
                    ]}}
                  ]},
                  "return_type":"Result<(bool, u128), string>",
                  "return_schema":{"nodes":[
                    {"kind":"Result","value":null},
                    {"kind":"Tuple","value":2},
                    {"kind":"Leaf","value":{"kind":"Bool","value":null}},
                    {"kind":"Leaf","value":{"kind":"U128","value":null}},
                    {"kind":"Leaf","value":{"kind":"String","value":null}}
                  ]},
                  "permission":"TransferAsset",
                  "read_keys":["state:Balances"],
                  "write_keys":["state:Balances"],
                  "access_hints_complete":true,
                  "access_hints_skipped":[],
                  "triggers":[{
                    "id":"settle",
                    "repeats":{"Indefinitely":null},
                    "filter":"$triggerFilter",
                    "authority":null,
                    "metadata":{"purpose":"daily-settlement","round":7},
                    "callback":{"namespace":null,"entrypoint":"transfer"}
                  }]
                }],
                "states":[{"name":"Balances","type_name":"StateMap<AccountId, Amount>"}],
                "error_codes":[{"namespace":"TransferError","name":"InsufficientFunds","code":1001}],
                "kotoba":[{
                  "msg_id":"transfer.denied",
                  "translations":[
                    {"lang":"en","text":"Transfer denied"},
                    {"lang":"ja","text":"送金は拒否されました"}
                  ]
                }],
                "provenance":{"signer":"ed25519:fixture","signature":"fixture-signature"}
              },
              "code_hash":"${"b".repeat(64)}",
              "abi_hash":"${"d".repeat(64)}"
            }
            """.trimIndent()
    }
}
