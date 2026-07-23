package org.hyperledger.iroha.sdk.consensus

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.nio.charset.StandardCharsets
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class NativeAmxV2GroupedFixtureTest {
    @Test
    fun `Rust-owned grouped golden is consumable`() {
        val fixture = fixture()
        assertEquals("iroha-native-amx-v2-grouped", fixture.string("format"))
        assertEquals(1, fixture.int("fixture_version"))
        assertEquals("iroha_data_model::block::consensus", fixture.string("rust_owner"))

        val golden = fixture.objectValue("golden")
        val groupWire = golden.objectValue("receipt_group")
        val group = NativeAmxV2.parseReceiptGroup(groupWire.toString())
        val expectedSources =
            golden.arrayValue("ordered_source_ids").map { it.jsonPrimitive.content }
        assertEquals(expectedSources, group.receipts.map { it.sourceId.value })
        assertEquals(2, group.receipts.size)
        group.receipts.forEach { receipt ->
            assertEquals(2, receipt.legs.size)
            assertEquals(9L, receipt.laneBlockView)
            receipt.legs.forEach { leg ->
                assertEquals(NativeAmxV2.Phase.PREPARE, leg.prepareQc.body.phase)
                assertEquals(NativeAmxV2.Phase.COMMIT, leg.commitQc.body.phase)
                assertEquals(6L, leg.prepareQc.body.round.view)
                assertEquals(9L, leg.prepareQc.body.coordinatorLaneBlockView)
                assertEquals(96, leg.prepareQc.aggregateSignature.size)
                assertEquals(
                    expectedSources,
                    leg.participantSettlement.receipts.map { it.sourceId.value },
                )
            }
        }
        val remoteLeg = group.receipts.first().legs.single { it.laneId == 8L }
        assertEquals(0L, remoteLeg.participantProposal.descriptor.laneBlockView)
        assertEquals(false, remoteLeg.requiresMixedRoleAnchorValidation)

        val diagnostics = golden.objectValue("expected_diagnostics")
        assertEquals(groupWire, diagnostics.arrayValue("lane_settlement_commitments").single())
        val application =
            Json.decodeFromJsonElement<SumeragiNativeAmxParticipantApplication>(
                diagnostics.arrayValue("native_amx_participant_applications").single(),
            )
        assertEquals(2L, application.sourceCount)
        assertEquals(
            SumeragiNativeAmxParticipantApplicationState.DURABLY_APPLIED,
            application.state,
        )
    }

    @Test
    fun `Rust-owned negative corpus is consumable`() {
        val canonical = fixture()
        for (controlElement in canonical.arrayValue("negative_controls")) {
            val control = controlElement.jsonObject
            assertEquals("reject", control.string("expectation"), control.string("id"))
            var mutated: JsonElement = canonical
            for (mutation in control.arrayValue("mutations")) {
                mutated = applyMutation(mutated, mutation.jsonObject)
            }
            val group =
                mutated.jsonObject
                    .objectValue("golden")
                    .objectValue("receipt_group")
            assertFailsWith<IllegalArgumentException>(control.string("id")) {
                NativeAmxV2.parseReceiptGroup(group.toString())
            }
        }
    }

    @Test
    fun `mixed-role participant exposes deferred anchor validation`() {
        var mutated: JsonElement = fixture()
        val descriptorPath =
            "/golden/receipt_group/native_amx_receipts/0/legs/1/" +
                "participant_proposal/descriptor"
        val descriptor = resolve(mutated, pointerTokens(descriptorPath)).jsonObject
        val hashes = descriptor.arrayValue("accepted_transaction_hashes")
        val indices = descriptor.arrayValue("accepted_candidate_indices")
        mutated = assign(
            mutated,
            pointerTokens("$descriptorPath/accepted_transaction_hashes"),
            JsonArray(listOf(hashes[1])),
        )
        mutated = assign(
            mutated,
            pointerTokens("$descriptorPath/accepted_candidate_indices"),
            JsonArray(listOf(indices[1])),
        )
        val group =
            mutated.jsonObject
                .objectValue("golden")
                .objectValue("receipt_group")
        val parsed = NativeAmxV2.parseReceiptGroup(group.toString())
        val remote = parsed.receipts.first().legs.single { it.laneId == 8L }
        assertEquals(true, remote.requiresMixedRoleAnchorValidation)
    }

    private fun applyMutation(root: JsonElement, mutation: JsonObject): JsonElement {
        val path = pointerTokens(mutation.string("path"))
        return when (mutation.string("op")) {
            "replace" -> assign(root, path, mutation.getValue("value"))
            "remove" -> remove(root, path)
            "copy" -> {
                val source = mutation.objectValue("value").string("from")
                assign(root, path, resolve(root, pointerTokens(source)))
            }
            "swap" -> {
                val options = mutation.objectValue("value")
                val array = resolve(root, path).jsonArray.toMutableList()
                val left = options.int("left")
                val right = options.int("right")
                val temporary = array[left]
                array[left] = array[right]
                array[right] = temporary
                assign(root, path, JsonArray(array))
            }
            "repeat" -> {
                val options = mutation.objectValue("value")
                val array = resolve(root, path).jsonArray
                assign(
                    root,
                    path,
                    JsonArray(List(options.int("count")) { array[options.int("source_index")] }),
                )
            }
            else -> error("unsupported fixture mutation")
        }
    }

    private fun resolve(root: JsonElement, tokens: List<String>): JsonElement =
        tokens.fold(root) { current, token ->
            when (current) {
                is JsonObject -> current.getValue(token)
                is JsonArray -> current[token.toInt()]
                else -> error("JSON pointer does not resolve")
            }
        }

    private fun assign(
        root: JsonElement,
        tokens: List<String>,
        replacement: JsonElement,
    ): JsonElement {
        if (tokens.isEmpty()) return replacement
        val head = tokens.first()
        val tail = tokens.drop(1)
        return when (root) {
            is JsonObject ->
                JsonObject(
                    root.toMutableMap().also { map ->
                        map[head] = assign(map.getValue(head), tail, replacement)
                    },
                )
            is JsonArray ->
                JsonArray(
                    root.toMutableList().also { array ->
                        val index = head.toInt()
                        array[index] = assign(array[index], tail, replacement)
                    },
                )
            else -> error("JSON pointer does not resolve")
        }
    }

    private fun remove(root: JsonElement, tokens: List<String>): JsonElement {
        require(tokens.isNotEmpty())
        val head = tokens.first()
        val tail = tokens.drop(1)
        return when (root) {
            is JsonObject ->
                JsonObject(
                    root.toMutableMap().also { map ->
                        if (tail.isEmpty()) {
                            check(map.remove(head) != null)
                        } else {
                            map[head] = remove(map.getValue(head), tail)
                        }
                    },
                )
            is JsonArray ->
                JsonArray(
                    root.toMutableList().also { array ->
                        val index = head.toInt()
                        if (tail.isEmpty()) {
                            array.removeAt(index)
                        } else {
                            array[index] = remove(array[index], tail)
                        }
                    },
                )
            else -> error("JSON pointer does not resolve")
        }
    }

    private fun pointerTokens(pointer: String): List<String> {
        require(pointer.startsWith('/'))
        return pointer.drop(1).split('/').map {
            it.replace("~1", "/").replace("~0", "~")
        }
    }

    private fun fixture(): JsonObject =
        Json.parseToJsonElement(
            String(Files.readAllBytes(fixturePath()), StandardCharsets.UTF_8),
        ).jsonObject

    private fun fixturePath(): Path {
        var current = Paths.get("").toAbsolutePath()
        while (true) {
            val candidate =
                current.resolve("fixtures/sumeragi_v2/native_amx_v2_grouped.json")
            if (Files.isRegularFile(candidate)) return candidate
            current = current.parent
                ?: error("fixtures/sumeragi_v2/native_amx_v2_grouped.json was not found")
        }
    }

    private fun JsonObject.objectValue(name: String): JsonObject =
        getValue(name).jsonObject

    private fun JsonObject.arrayValue(name: String): JsonArray =
        getValue(name).jsonArray

    private fun JsonObject.string(name: String): String =
        getValue(name).jsonPrimitive.content

    private fun JsonObject.int(name: String): Int =
        getValue(name).jsonPrimitive.int
}
