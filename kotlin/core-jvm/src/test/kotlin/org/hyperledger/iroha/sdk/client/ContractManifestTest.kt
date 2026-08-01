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
        val accessSetHints = manifest.accessSetHints ?: error("missing access-set hints")
        assertEquals(64, accessSetHints.dynamicReads.single().maxKeys)
        assertEquals("AccountId", accessSetHints.dynamicReads.single().keyType)
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
        assertEquals("Result<(bool, decimal), string>", returnSchema.canonicalTypeName)
        assertEquals(ContractTriggerRepeatsKind.INDEFINITELY, entrypoint.triggers.single().repeats.kind)
        assertNull(entrypoint.triggers.single().repeats.exactly)
        assertEquals("transfer", entrypoint.triggers.single().callback.entrypoint)
        assertEquals("daily-settlement", entrypoint.triggers.single().metadata["purpose"])
        assertEquals("StateMap<AccountId, quantity>", manifest.states!!.single().typeName)
        assertEquals(1001, manifest.errorCodes!!.single().code)
        assertEquals("ja", manifest.kotoba!!.single().translations.last().language)
        assertEquals("ed25519:fixture", manifest.provenance!!.signer)
    }

    @Test
    fun triggerBoundariesRejectExactAmountSourceFormOnly() {
        val retired = listOf(
            fullResponse().replaceFirst("\"id\":\"settle\"", "\"id\":\"Amount\""),
            fullResponse().replaceFirst("\"namespace\":null", "\"namespace\":\"Amount\""),
        )
        retired.forEach { payload ->
            assertFailsWith<IllegalStateException>(payload) {
                ContractJsonParser.parseManifestRecord(payload.toByteArray(StandardCharsets.UTF_8))
            }
        }

        val lowercase = fullResponse()
            .replaceFirst("\"id\":\"settle\"", "\"id\":\"amount\"")
            .replaceFirst("\"namespace\":null", "\"namespace\":\"RemoteLedger\"")
        val trigger = ContractJsonParser.parseManifestRecord(
            lowercase.toByteArray(StandardCharsets.UTF_8),
        ).manifest.entrypoints!!.single().triggers.single()
        assertEquals("amount", trigger.id)
        assertEquals("RemoteLedger", trigger.callback.namespace)
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
            fullResponse().replaceFirst("\"seiyaku_name\":\"Ledger\"", "\"seiyaku_name\":\"Amount\""),
            fullResponse().replaceFirst("\"seiyaku_name\":\"Ledger\"", "\"seiyaku_name\":\"amount\""),
            fullResponse().replaceFirst("\"name\":\"transfer\"", "\"name\":\"Amount\""),
            fullResponse().replaceFirst("\"name\":\"request\",\"type_name\"", "\"name\":\"Amount\",\"type_name\""),
            fullResponse().replaceFirst("\"fields\":[\"amount\",\"memo\"]", "\"fields\":[\"Amount\",\"memo\"]"),
            fullResponse().replaceFirst("\"name\":\"Balances\",\"type_name\"", "\"name\":\"Amount\",\"type_name\""),
            fullResponse().replaceFirst("\"name\":\"InsufficientFunds\",\"code\"", "\"name\":\"Amount\",\"code\""),
            fullResponse().replaceFirst("\"base_key\":\"state:Balances\"", "\"base_key\":\"state:Amount\""),
            fullResponse().replaceFirst("\"seiyaku_name\":\"Ledger\"", "\"seiyaku_name\":\"__kotodama_link_private\""),
            fullResponse().replaceFirst("\"seiyaku_name\":\"Ledger\"", "\"seiyaku_name\":\"state_map_get\""),
            fullResponse().replaceFirst(
                "\"seiyaku_name\":\"Ledger\"",
                "\"seiyaku_name\":\"__kotodama_quantity_ratio_round\"",
            ),
            fullResponse().replaceFirst(
                "\"seiyaku_name\":\"Ledger\"",
                "\"seiyaku_name\":\"__kotodama_decimal_to_int_trunc\"",
            ),
            fullResponse().replaceFirst(
                "\"seiyaku_name\":\"Ledger\"",
                "\"seiyaku_name\":\"__kotodama_decimal_to_int_round\"",
            ),
            fullResponse().replaceFirst("\"kind\":\"Quantity\"", "\"kind\":\"Amount\""),
            fullResponse().replaceFirst("\"kind\":\"Decimal\"", "\"kind\":\"U128\""),
            fullResponse().replaceFirst("\"namespace\":\"TransferError\"", "\"namespace\":\"Option\""),
            fullResponse().replaceFirst("\"features_bitmap\":0", "\"features_bitmap\":4"),
            fullResponse().replaceFirst("\"dynamic_writes\":[]", "\"dynamic_writes\":[],\"unknown\":true"),
            fullResponse().replaceFirst(
                "\"repeats\":{\"Indefinitely\":null}",
                "\"repeats\":{\"kind\":\"Indefinitely\",\"value\":null}",
            ),
            fullResponse().replaceFirst("\"code_hash\":\"${"b".repeat(64)}\"", "\"code_hash\":\"${"f".repeat(64)}\""),
        )

        invalid.forEachIndexed { index, payload ->
            assertFailsWith<IllegalStateException>("invalid[$index] mutation was accepted: $payload") {
                ContractJsonParser.parseManifestRecord(payload.toByteArray(StandardCharsets.UTF_8))
            }
        }
    }

    @Test
    fun retiredNumericTypeNamesAreRejectedOnlyInTypePositions() {
        fun statePayload(typeName: String): String =
            """{"manifest":{"states":[{"name":"Balances","type_name":"$typeName"}]},"code_hash":null,"abi_hash":null}"""

        val maximumDepth = "Option<".repeat(255) + "int" + ">".repeat(255)
        val maximumMapDepth = "Option<".repeat(254) + "int" + ">".repeat(254)
        listOf(
            "quantity",
            "(int, decimal)",
            "Option<Result<quantity, string>>",
            "List<Transfer{amount: quantity}, 64>",
            "StateMap<AccountId, Transfer{amount: quantity, memo: Option<string>}>",
            "List<Envelope{items: List<Transfer{amount: quantity}, 64>}, 1>",
            maximumDepth,
            wideTupleType(255),
            "StateMap<AccountId, ${wideTupleType(255)}>",
            "StateMap<AccountId, $maximumMapDepth>",
        ).forEach { legalType ->
            val legal = statePayload(legalType)
            assertEquals(
                legalType,
                ContractJsonParser.parseManifestRecord(legal.toByteArray(StandardCharsets.UTF_8))
                    .manifest.states!!.single().typeName,
            )
        }

        listOf(
            "Amount",
            "amount",
            "Foo{Amount: quantity}",
            "Foo{Amount:quantity}",
            "StateMap<AccountId, int>",
            "Аmount",
        ).forEach { invalidHint ->
            val payload = fullResponse().replace(
                "\"key_type\":\"AccountId\"",
                "\"key_type\":\"$invalidHint\"",
            )
            assertFailsWith<IllegalStateException> {
                ContractJsonParser.parseManifestRecord(payload.toByteArray(StandardCharsets.UTF_8))
            }
        }

        listOf(
            "Amount",
            "amount",
            "Amount: quantity",
            "Option<Amount>",
            "List<amount, 1>",
            "StateMap<AccountId, Amount>",
            "StateMap<AccountId, Amount: quantity>",
            "Transfer{amount: amount}",
            "Transfer{amount:: quantity}",
            "Transfer{Amount: quantity}",
            "Amount{amount: quantity}",
            "Transfer{amount: quantity, amount: int}",
            "Transfer{}",
            "Option<StateMap<AccountId, quantity>>",
            "StateMap<Json, quantity>",
            "(int)",
            "Result<int,string>",
            "List<quantity, 0>",
            "List<quantity, 65>",
            "List<quantity, 01>",
            "Transfer {amount: quantity}",
            "Transfer{amount: quantity, memo:string}",
            "Transfer{amøunt: quantity}",
            "Tránsfer{amount: quantity}",
            "Transfer{__kotodama_link_private: quantity}",
            "Option<".repeat(256) + "int" + ">".repeat(256),
            wideTupleType(256),
            "StateMap<AccountId, ${wideTupleType(256)}>",
            "StateMap<AccountId, $maximumDepth>",
        ).forEach { retiredType ->
            val payload = statePayload(retiredType)
            assertFailsWith<IllegalStateException> {
                ContractJsonParser.parseManifestRecord(payload.toByteArray(StandardCharsets.UTF_8))
            }
        }
    }

    @Test
    fun dynamicAccessHintsEnforceTheExactV1Policy() {
        fun parse(payload: String): ContractDynamicAccessHint {
            val manifest =
                ContractJsonParser.parseManifestRecord(payload.toByteArray(StandardCharsets.UTF_8))
                    .manifest
            val accessSetHints = manifest.accessSetHints ?: error("missing access-set hints")
            return accessSetHints.dynamicReads.single()
        }

        val keyTypes = listOf(
            "int",
            "decimal",
            "quantity",
            "bool",
            "string",
            "bytes",
            "DataSpaceId",
            "AccountId",
            "AssetDefinitionId",
            "AssetId",
            "NftId",
            "DomainId",
            "Name",
        )
        keyTypes.forEach { keyType ->
            val payload = fullResponse().replaceFirst(
                "\"key_type\":\"AccountId\"",
                "\"key_type\":\"$keyType\"",
            ).replaceFirst(
                "StateMap<AccountId, quantity>",
                "StateMap<$keyType, quantity>",
            )
            assertEquals(keyType, parse(payload).keyType)
        }

        listOf("range", "take").forEach { boundKind ->
            val payload = fullResponse().replaceFirst(
                "\"bound_kind\":\"take\"",
                "\"bound_kind\":\"$boundKind\"",
            )
            assertEquals(boundKind, parse(payload).boundKind)
        }
        listOf("state:Balances", "state:amount").forEach { baseKey ->
            var payload = fullResponse().replaceFirst(
                "\"base_key\":\"state:Balances\"",
                "\"base_key\":\"$baseKey\"",
            )
            if (baseKey == "state:amount") {
                payload = payload.replaceFirst(
                    "\"name\":\"Balances\",\"type_name\":\"StateMap<AccountId, quantity>\"",
                    "\"name\":\"amount\",\"type_name\":\"StateMap<AccountId, quantity>\"",
                )
            }
            assertEquals(baseKey, parse(payload).baseKey)
        }
        listOf(1, 64).forEach { maxKeys ->
            val payload = fullResponse().replaceFirst("\"max_keys\":64", "\"max_keys\":$maxKeys")
            assertEquals(maxKeys.toLong(), parse(payload).maxKeys)
        }

        listOf(
            "",
            "state:",
            "state:*",
            "state:Balances.more",
            "state:Balances:Other",
            "state:state:Balances",
            "state:match",
            "state:StateMap",
            "state:__kotodama_link_Balances",
            "state: Balances",
            "state:Balances ",
            "state:Бalances",
            "states:Balances",
            "Balances",
            "state:amount.more",
        ).forEach { baseKey ->
            val payload = fullResponse().replaceFirst(
                "\"base_key\":\"state:Balances\"",
                "\"base_key\":\"$baseKey\"",
            )
            assertFailsWith<IllegalStateException>("accepted base_key `$baseKey`") { parse(payload) }
        }
        listOf(
            "",
            "Json",
            "Int",
            "Amount",
            "amount",
            "AccountID",
            "Transfer",
            "StateMap",
            "StateMap<AccountId, quantity>",
            " AccountId",
            "AccountId ",
            "АccountId",
        ).forEach { keyType ->
            val payload = fullResponse().replaceFirst(
                "\"key_type\":\"AccountId\"",
                "\"key_type\":\"$keyType\"",
            )
            assertFailsWith<IllegalStateException>("accepted key_type `$keyType`") { parse(payload) }
        }
        listOf("", "Range", "Take", "all", "prefix", "range ", " take").forEach { boundKind ->
            val payload = fullResponse().replaceFirst(
                "\"bound_kind\":\"take\"",
                "\"bound_kind\":\"$boundKind\"",
            )
            assertFailsWith<IllegalStateException>("accepted bound_kind `$boundKind`") { parse(payload) }
        }
        listOf("0", "65", "4294967295", "-1", "1.0", "\"1\"").forEach { maxKeys ->
            val payload = fullResponse().replaceFirst("\"max_keys\":64", "\"max_keys\":$maxKeys")
            assertFailsWith<IllegalStateException>("accepted max_keys `$maxKeys`") { parse(payload) }
        }
        listOf(
            fullResponse().replaceFirst(
                "\"max_keys\":64",
                "\"max_keys\":64,\"unknown\":true",
            ),
            fullResponse().replaceFirst("\"base_key\":\"state:Balances\"", "\"base_key\":null"),
            fullResponse().replaceFirst("\"key_type\":\"AccountId\"", "\"key_type\":false"),
            fullResponse().replaceFirst("\"bound_kind\":\"take\"", "\"bound_kind\":1"),
            fullResponse().replaceFirst("\"max_keys\":64", "\"max_keys\":null"),
        ).forEach { payload ->
            assertFailsWith<IllegalStateException> { parse(payload) }
        }
    }

    @Test
    fun dynamicAccessHintsResolveExactDeclaredStateMaps() {
        fun hint(
            baseKey: String = "state:Balances",
            keyType: String = "AccountId",
        ): String =
            """{"base_key":"$baseKey","key_type":"$keyType","bound_kind":"take","max_keys":1}"""

        fun payload(
            dynamicReads: List<String>,
            dynamicWrites: List<String>,
            stateName: String = "Balances",
            stateType: String = "StateMap<AccountId, quantity>",
        ): String =
            """
            {
              "manifest":{
                "access_set_hints":{
                  "read_keys":[],
                  "write_keys":[],
                  "dynamic_reads":[${dynamicReads.joinToString(",")}],
                  "dynamic_writes":[${dynamicWrites.joinToString(",")}]
                },
                "states":[{"name":"$stateName","type_name":"$stateType"}]
              },
              "code_hash":null,
              "abi_hash":null
            }
            """.trimIndent()

        fun parse(value: String): ContractManifestRecord =
            ContractJsonParser.parseManifestRecord(value.toByteArray(StandardCharsets.UTF_8))

        val canonical = hint()
        listOf(
            payload(listOf(canonical, canonical), emptyList()),
            payload(emptyList(), listOf(canonical, canonical)),
            payload(listOf(hint(baseKey = "state:Missing")), emptyList()),
            payload(listOf(canonical), emptyList(), stateType = "quantity"),
            payload(listOf(hint(keyType = "Name")), emptyList()),
        ).forEach { malformed ->
            assertFailsWith<IllegalStateException> { parse(malformed) }
        }

        val amount = hint(baseKey = "state:amount")
        val accepted = parse(
            payload(
                dynamicReads = listOf(amount),
                dynamicWrites = listOf(amount),
                stateName = "amount",
            ),
        )
        val acceptedHints = accepted.manifest.accessSetHints ?: error("missing access-set hints")
        assertEquals("state:amount", acceptedHints.dynamicReads.single().baseKey)
        assertEquals("state:amount", acceptedHints.dynamicWrites.single().baseKey)
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
        var validTypeName = "int"
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
                parseBoundarySchema(nodes, "int")
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
                leafNode("Decimal"),
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
                leafNode("Quantity"),
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

        private fun wideTupleType(elements: Int): String = buildString(elements * 5 + 2) {
            append('(')
            repeat(elements) { index ->
                if (index > 0) append(", ")
                append("int")
            }
            append(')')
        }

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
                leafNode("Quantity"),
            )
            "AssetDefinitionView" -> listOf(
                structNode(name, "id", "name", "description", "owned_by", "total_quantity", "metadata"),
                leafNode("AssetDefinitionId"),
                leafNode("String"),
                optionNode,
                leafNode("String"),
                leafNode("AccountId"),
                leafNode("Quantity"),
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
                      {"kind":"Leaf","value":{"kind":"Quantity","value":null}},
                      {"kind":"Option","value":null},
                      {"kind":"Leaf","value":{"kind":"String","value":null}}
                    ]}},
                    {"name":"tags","ty":{"nodes":[
                      {"kind":"List","value":{"capacity":64}},
                      {"kind":"Leaf","value":{"kind":"Name","value":null}}
                    ]}}
                  ]},
                  "return_type":"Result<(bool, decimal), string>",
                  "return_schema":{"nodes":[
                    {"kind":"Result","value":null},
                    {"kind":"Tuple","value":2},
                    {"kind":"Leaf","value":{"kind":"Bool","value":null}},
                    {"kind":"Leaf","value":{"kind":"Decimal","value":null}},
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
                "states":[{"name":"Balances","type_name":"StateMap<AccountId, quantity>"}],
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
