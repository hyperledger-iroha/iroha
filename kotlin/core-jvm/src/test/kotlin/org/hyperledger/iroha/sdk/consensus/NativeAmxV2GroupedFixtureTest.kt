package org.hyperledger.iroha.sdk.consensus

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

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
        val firstLeg = group.receipts.first().legs.first()
        assertEquals(
            "hash:33F884E54077B6570826E5DB30B64CEA24B8B559C057F152848E4D1DE7FE8041#6EF8",
            firstLeg.participantProposal.descriptor.validatorSetHash.value,
        )
        assertEquals(
            "hash:568077DEBB5ECE0F6655571DBD81F8B8935CA5FB064F6B74864B4F58F3CB1A33#E6A5",
            firstLeg.participantProposal.descriptor.descriptorHash.value,
        )
        assertEquals(
            "hash:AAC0F352914C21699F3F8D571196C9A5DFCAA9EF1272A7DEFA7FFD35A93C21AD#8B3F",
            firstLeg.participantProposal.proposalHash.value,
        )
        assertEquals(null, firstLeg.participantProposal.payloadBlockHint)
        assertEquals(
            "hash:C6B18DBE6BEC468DB021B79604233F3CB9E2D6CDF3384C491CE7A6DA89747825#9D72",
            firstLeg.participantSettlementHash.value,
        )
        assertTrue(
            NativeAmxV2.isCanonicalBlsNormalPeerId(
                firstLeg.participantProposal.descriptor.validatorSet.first(),
            ),
        )
        group.receipts.forEach { receipt ->
            assertEquals(2, receipt.legs.size)
            assertEquals(BigInteger.valueOf(9), receipt.laneBlockView)
            receipt.legs.forEach { leg ->
                assertEquals(NativeAmxV2.Phase.PREPARE, leg.prepareQc.body.phase)
                assertEquals(NativeAmxV2.Phase.COMMIT, leg.commitQc.body.phase)
                assertEquals(BigInteger.valueOf(6), leg.prepareQc.body.round.view)
                assertEquals(
                    BigInteger.valueOf(9),
                    leg.prepareQc.body.coordinatorLaneBlockView,
                )
                assertEquals(96, leg.prepareQc.aggregateSignature.size)
                assertEquals(
                    expectedSources,
                    leg.participantSettlement.receipts.map { it.sourceId.value },
                )
            }
        }
        val remoteLeg = group.receipts.first().legs.single { it.laneId == 8L }
        assertEquals(BigInteger.ZERO, remoteLeg.participantProposal.descriptor.laneBlockView)
        assertEquals(
            "hash:40C7FCA7AA143B323B473A9958B96F49896C03C3547B83DD340FAE2FC1A85D29#B452",
            remoteLeg.participantSettlementHash.value,
        )
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
        validateApplicationEvidence(fixture)

        val invalidGroupUtf8 = assertFailsWith<IllegalArgumentException> {
            NativeAmxV2.parseReceiptGroup(byteArrayOf(0xff.toByte()))
        }
        assertEquals(
            "Native AMX receipt group must be valid UTF-8",
            invalidGroupUtf8.message,
        )
        val invalidReceiptUtf8 = assertFailsWith<IllegalArgumentException> {
            NativeAmxV2.parseReceipt(byteArrayOf(0xff.toByte()))
        }
        assertEquals(
            "Native AMX V2 receipt must be valid UTF-8",
            invalidReceiptUtf8.message,
        )
    }

    @Test
    fun `participant proposal requires an explicit null payload hint`() {
        val group = fixture()
            .objectValue("golden")
            .objectValue("receipt_group")
        val proposalPath = listOf(
            "native_amx_receipts",
            "0",
            "legs",
            "0",
            "participant_proposal",
        )
        val proposal = resolve(group, proposalPath).jsonObject
        val wireNull = proposal.getValue("payload_block_hint")
        val invalidProposals = listOf(
            JsonObject(proposal - "payload_block_hint"),
            JsonObject(proposal + ("payload_block_hint" to proposal.getValue("descriptor"))),
            JsonObject(proposal + ("future_proposal_field" to wireNull)),
        )

        invalidProposals.forEach { invalidProposal ->
            val mutated = assign(group, proposalPath, invalidProposal)
            assertFailsWith<IllegalArgumentException> {
                NativeAmxV2.parseReceiptGroup(mutated.toString())
            }
        }
    }

    @Test
    fun `Rust-owned negative corpus is consumable`() {
        val canonical = fixture()
        val controls = canonical.arrayValue("negative_controls")
        val identifiers = controls.map { it.jsonObject.string("id") }.toSet()
        assertTrue(
            identifiers.containsAll(
                setOf(
                    "coherent_forged_validator_set_hash",
                    "coherent_stale_descriptor_hash",
                    "coherent_stale_proposal_hash",
                    "coherent_stale_settlement_hash",
                    "coherent_duplicate_validator_set",
                    "coherent_over_quorum_requirement",
                    "manifest_leaf_hash_tampering",
                    "non_canonical_validator_peer_id",
                    "execution_commitment_merge_carrier_wrong_version",
                    "execution_commitment_missing_merge_carrier_field",
                ),
            ),
        )
        assertFalse(
            NativeAmxV2.isCanonicalBlsNormalPeerId(
                "ea0130" + "00".repeat(48),
            ),
        )
        for (controlElement in controls) {
            val control = controlElement.jsonObject
            assertEquals("reject", control.string("expectation"), control.string("id"))
            var mutated: JsonElement = canonical
            for (mutation in control.arrayValue("mutations")) {
                mutated = applyMutation(mutated, mutation.jsonObject)
            }
            if (control.string("validator") == "application_evidence") {
                assertFailsWith<IllegalArgumentException>(control.string("id")) {
                    validateApplicationEvidence(mutated.jsonObject)
                }
                continue
            }
            assertEquals("receipt_group", control.string("validator"))
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
        val group = fixture()
            .objectValue("golden")
            .objectValue("receipt_group")
        val parsed = NativeAmxV2.parseReceiptGroup(group.toString())
        val remote = parsed.receipts.first().legs.single { it.laneId == 8L }
        val current = remote.prepareQc.body.transactionEntrypointHash
        assertFalse(
            NativeAmxV2.requiresMixedRoleAnchorValidation(
                remote.participantProposal.descriptor,
                current,
            ),
        )
        val absent = NativeAmxV2.TransactionEntrypointHash(
            "hash:07BAE6F998F2D195BD9481ADDFB26789F771FDD7F6BB476A9C3157F70FB85AB7#9781",
        )
        assertTrue(
            NativeAmxV2.requiresMixedRoleAnchorValidation(
                remote.participantProposal.descriptor,
                absent,
            ),
        )
    }

    @Test
    fun `Native u64 fields preserve the complete numeric token domain`() {
        val canonical = fixture()
            .objectValue("golden")
            .objectValue("receipt_group")
            .toString()
        val accepted = listOf(
            BigInteger.ONE.shiftLeft(63),
            BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE),
        )

        accepted.forEach { boundary ->
            val wire = canonical.replace("\"epoch\":3", "\"epoch\":$boundary")
            assertNotEquals(canonical, wire)
            val group = NativeAmxV2.parseReceiptGroup(wire)
            group.receipts.forEach { receipt ->
                receipt.legs.forEach { leg ->
                    assertEquals(boundary, leg.prepareQc.body.epoch)
                    assertEquals(boundary, leg.commitQc.body.epoch)
                }
            }
        }
    }

    @Test
    fun `Native u64 fields reject non-integer strings and out-of-range tokens`() {
        val canonical = fixture()
            .objectValue("golden")
            .objectValue("receipt_group")
            .toString()
        val rejected = listOf(
            "18446744073709551616",
            "-1",
            "1.0",
            "1e0",
            "01",
            "\"18446744073709551615\"",
        )

        rejected.forEach { token ->
            val wire = canonical.replace("\"epoch\":3", "\"epoch\":$token")
            assertNotEquals(canonical, wire)
            assertFailsWith<IllegalArgumentException>(token) {
                NativeAmxV2.parseReceiptGroup(wire)
            }
        }
    }

    @Test
    fun `Native predecessor arithmetic fails closed at u64 max`() {
        val canonical = fixture()
            .objectValue("golden")
            .objectValue("receipt_group")
            .toString()
        val wire = canonical.replace(
            "\"participant_previous_block_height\":41",
            "\"participant_previous_block_height\":18446744073709551615",
        )
        assertNotEquals(canonical, wire)

        assertFailsWith<IllegalArgumentException> {
            NativeAmxV2.parseReceiptGroup(wire)
        }
    }

    private fun validateApplicationEvidence(document: JsonObject) {
        val golden = document.objectValue("golden")
        val group = golden.objectValue("receipt_group")
        val evidence = golden.objectValue("application_evidence")
        val execution = evidence.objectValue("execution_commitment")
        val artifacts = evidence.arrayValue("manifest_artifacts")
        require(execution.int("native_amx_application_manifest_version") == 1)
        require(execution.containsKey("merge_carrier"))
        val rawMergeCarrier = execution.getValue("merge_carrier")
        require(rawMergeCarrier is JsonObject)
        val mergeCarrier = rawMergeCarrier.jsonObject
        require(mergeCarrier.keys == setOf("version", "entry_hash"))
        val mergeCarrierVersion = mergeCarrier.getValue("version").jsonPrimitive
        require(!mergeCarrierVersion.isString && mergeCarrierVersion.int == 1)
        val mergeCarrierEntryHash = mergeCarrier.getValue("entry_hash").jsonPrimitive
        require(mergeCarrierEntryHash.isString)
        NativeAmxV2.ConsensusHash(mergeCarrierEntryHash.content)
        val manifestCount = execution.int("native_amx_application_manifest_count")
        require(
            manifestCount == artifacts.size && artifacts.size == 1,
        )
        val artifact = artifacts.single().jsonObject
        val leaf = artifact.objectValue("leaf")
        val proof = artifact.objectValue("proof")
        require(artifact.int("version") == 1 && leaf.int("version") == 1)
        require(artifact.int("leaf_index") == 0 && proof.int("leaf_index") == 0)
        require(proof.arrayValue("audit_path").isEmpty())
        require(artifact.int("manifest_leaf_count") == manifestCount)
        val expectedManifestRoot =
            applicationManifestSingletonRoot(artifact.string("leaf_hash"))
        require(
            artifact.string("manifest_root") == expectedManifestRoot &&
                execution.string("native_amx_application_manifest_root") == expectedManifestRoot,
        ) {
            "singleton manifest root must authenticate the domain-separated leaf hash"
        }
        require(
            leaf.getValue("executed_block_wire_hash") ==
                execution.getValue("executed_block_wire_hash"),
        )
        require(execution.getValue("executed_block_wire_len").jsonPrimitive.long == 49L)
        require(leaf.int("predecessor_height") + 1 == leaf.int("participant_height"))
        val active = evidence.arrayValue("active_lane_incarnations").single().jsonObject
        require(active.getValue("lane_id") == leaf.getValue("lane_id"))
        require(active.getValue("dataspace_id") == leaf.getValue("dataspace_id"))
        require(active.getValue("lane_incarnation") == leaf.getValue("lane_incarnation"))
        require(
            leaf.getValue("lane_id") != group.getValue("lane_id") ||
                leaf.getValue("dataspace_id") != group.getValue("dataspace_id"),
        )

        val members = leaf.arrayValue("members")
        val receipts = group.arrayValue("native_amx_receipts")
        require(members.size in 1..4096 && members.size == receipts.size)
        require(
            members.map { it.jsonObject.getValue("source_id") } ==
                receipts.map { it.jsonObject.getValue("source_id") },
        )
        require(members.map { it.jsonObject.string("source_id") }.toSet().size == members.size)
        require(
            members.zipWithNext().all { (left, right) ->
                left.jsonObject.int("entrypoint_index") <
                    right.jsonObject.int("entrypoint_index")
            },
        )
        val carrierEntrypoints =
            evidence.arrayValue("carrier_entrypoint_hashes").toSet()
        receipts.zip(members).forEach { (receiptValue, memberValue) ->
            val receipt = receiptValue.jsonObject
            val member = memberValue.jsonObject
            val leg =
                receipt.arrayValue("legs").map { it.jsonObject }.singleOrNull {
                    it.getValue("lane_id") == leaf.getValue("lane_id") &&
                        it.getValue("dataspace_id") == leaf.getValue("dataspace_id")
                }
            requireNotNull(leg)
            val proposal = leg.objectValue("participant_proposal")
            val descriptor = proposal.objectValue("descriptor")
            require(descriptor.getValue("lane_incarnation") == leaf.getValue("lane_incarnation"))
            require(descriptor.getValue("lane_block_height") == leaf.getValue("participant_height"))
            require(descriptor.getValue("lane_block_view") == leaf.getValue("participant_view"))
            require(
                descriptor.getValue("previous_lane_block_height") ==
                    leaf.getValue("predecessor_height"),
            )
            require(
                descriptor["previous_lane_block_descriptor_hash"] ==
                    leaf["predecessor_descriptor_hash"],
            )
            require(descriptor.getValue("descriptor_hash") == leaf.getValue("descriptor_hash"))
            require(proposal.getValue("proposal_hash") == leaf.getValue("proposal_hash"))
            require(
                leg.getValue("participant_settlement_hash") ==
                    leaf.getValue("settlement_hash"),
            )
            val body = leg.objectValue("prepare_qc").objectValue("body")
            require(body.getValue("source_id") == member.getValue("source_id"))
            require(
                body.getValue("tx_entrypoint_hash") ==
                    member.getValue("entrypoint_hash"),
            )
            require(
                descriptor.arrayValue("accepted_candidate_indices")
                    .contains(member.getValue("entrypoint_index")),
            )
            require(
                descriptor.arrayValue("accepted_transaction_hashes")
                    .all { carrierEntrypoints.contains(it) },
            )
        }

        val row =
            golden.objectValue("expected_diagnostics")
                .arrayValue("native_amx_participant_applications")
                .single()
                .jsonObject
        listOf(
            "lane_id",
            "dataspace_id",
            "lane_incarnation",
            "participant_height",
            "participant_view",
            "predecessor_height",
            "predecessor_descriptor_hash",
            "descriptor_hash",
            "proposal_hash",
            "settlement_hash",
            "application_block_height",
            "application_block_hash",
        ).forEach { field -> require(row[field] == leaf[field]) }
        require(row.int("source_count") == members.size)
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

    private fun applicationManifestSingletonRoot(leafHash: String): String {
        val leafHashBytes = HashLiteral.decode(leafHash)
        require(HashLiteral.canonicalize(leafHashBytes) == leafHash) {
            "manifest leaf hash must be canonical"
        }
        val domain =
            "iroha:merkle:leaf:v1\u0000".toByteArray(StandardCharsets.UTF_8)
        return HashLiteral.canonicalize(IrohaHash.prehash(domain + leafHashBytes))
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
