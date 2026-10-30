package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.junit.jupiter.params.ParameterizedTest
import org.junit.jupiter.params.provider.MethodSource
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Paths
import java.util.Base64
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/** Static SCCP rules exercised through the public proposal JSON reader. */
class SccpGovernanceProposalV1Test {
    @ParameterizedTest(name = "{0}")
    @MethodSource("actions")
    fun actionFieldsAndSubjectsAreExact(action: String) {
        val proposal = SccpProposalFixtures.proposal(action)
        val expected = JsonParser.parse(String(SccpProposalFixtures.encode(proposal), StandardCharsets.UTF_8))
        assertEquals(expected, read(proposal).wire)
        val body = SccpProposalFixtures.body(proposal)
        for (field in body.keys.toList()) {
            val saved = body.remove(field)
            assertFailsWith<IllegalArgumentException>("missing $action.$field") { read(proposal) }
            body[field] = saved
        }
        body["extra"] = null
        assertFailsWith<IllegalArgumentException> { read(proposal) }
        body.remove("extra")
        SccpProposalFixtures.inner(proposal)["base_revisions"] =
            listOf(
                SccpProposalFixtures.base("fast_hold", "bsc_mainnet"),
            )
        assertFailsWith<IllegalArgumentException> { read(proposal) }
    }

    @ParameterizedTest(name = "reject {0}")
    @MethodSource("invalidBodies")
    fun rejectsInvalidStaticFields(case: InvalidBody) {
        val proposal = SccpProposalFixtures.proposal(case.action)
        read(proposal)
        SccpProposalFixtures.replace(SccpProposalFixtures.body(proposal), case.path, case.value)
        assertFailsWith<IllegalArgumentException>(case.toString()) { read(proposal) }
    }

    @Test
    fun envelopeOrderMultiplicityAndExactIntegerSpelling() {
        val value = SccpProposalFixtures.proposal("remove_staged")
        val proposal = SccpProposalFixtures.inner(value)
        proposal["actions"] =
            listOf(
                SccpProposalFixtures.action("freeze_light_client", "ton_mainnet"),
                SccpProposalFixtures.action("remove_staged", "bsc_mainnet"),
                SccpProposalFixtures.action("set_taira_paused", "ethereum_mainnet"),
                SccpProposalFixtures.action("remove_staged", "bsc_mainnet"),
            )
        val bases =
            listOf(
                SccpProposalFixtures.base("route", "ethereum_mainnet"),
                SccpProposalFixtures.base("route", "bsc_mainnet", SAFE_MAX),
                SccpProposalFixtures.base("light_client", "ton_mainnet"),
            )
        proposal["base_revisions"] = bases
        read(value)
        for (invalid in listOf(emptyList(), bases.reversed(), bases + bases.first())) {
            proposal["base_revisions"] = invalid
            assertFailsWith<IllegalArgumentException> { read(value) }
        }
        val canonical = String(SccpProposalFixtures.encode(SccpProposalFixtures.proposal("remove_staged")), StandardCharsets.UTF_8)
        for (literal in listOf("1e0", "1.0", "-0", "true", "\"1\"", "9007199254740992")) {
            assertFailsWith<IllegalArgumentException>(literal) {
                ParliamentApiV1.Proposal.fromJson(
                    canonical.replace("\"revision\":1", "\"revision\":$literal").toByteArray(StandardCharsets.UTF_8),
                )
            }
        }
    }

    @Test
    fun sixteenActionsAndSixteenDistinctSubjectsPreserveActionOrder() {
        val value = SccpProposalFixtures.proposal("remove_staged")
        val proposal = SccpProposalFixtures.inner(value)
        proposal["actions"] = List(16) { SccpProposalFixtures.action("remove_staged") }
        read(value)
        proposal["actions"] = List(17) { SccpProposalFixtures.action("remove_staged") }
        assertFailsWith<IllegalArgumentException> { read(value) }
        val actions = mutableListOf<Map<String, Any?>>()
        val bases = mutableListOf<Map<String, Any?>>()
        for (action in listOf(
            "set_taira_paused",
            "set_destination_paused",
            "freeze_light_client",
            "set_parameters",
            "quarantine_revision",
            "set_fast_pause_suspended",
        )) {
            for (name in if (action == "set_parameters") listOf("bsc_mainnet") else SccpProposalFixtures.networks) {
                actions.add(SccpProposalFixtures.action(action, name))
                bases.add(SccpProposalFixtures.base(SccpProposalFixtures.subjects.getValue(action), name))
            }
        }
        assertEquals(16, actions.size)
        proposal["actions"] = actions.reversed()
        proposal["base_revisions"] = bases
        read(value)
    }

    @Test
    fun registrationAndBootstrapBoundsAreNetworkSpecific() {
        for (name in SccpProposalFixtures.networks) {
            val value = SccpProposalFixtures.proposal("register_route", name)
            val body = SccpProposalFixtures.body(value)
            val cap = BigInteger.ONE.shiftLeft(if (name == "ton_mainnet") 96 else 128)
            body["max_wrapped_supply"] = cap.subtract(BigInteger.ONE).toString()
            body["revision"] = 0xffff_ffffL
            read(value)
            body["max_wrapped_supply"] = cap.toString()
            assertFailsWith<IllegalArgumentException> { read(value) }
            val initialize = SccpProposalFixtures.proposal("initialize_light_client", name)
            SccpProposalFixtures.replace(
                SccpProposalFixtures.body(initialize),
                listOf("bootstrap", "bytes"),
                Base64.getEncoder().encodeToString(ByteArray(1_048_576)),
            )
            read(initialize)
            SccpProposalFixtures.replace(
                SccpProposalFixtures.body(initialize),
                listOf("bootstrap", "bytes"),
                Base64.getEncoder().encodeToString(ByteArray(1_048_577)),
            )
            assertFailsWith<IllegalArgumentException> { read(initialize) }
        }
        for (field in listOf("ws_bound_ms", "checkpoint_stride")) {
            val value = SccpProposalFixtures.proposal("initialize_light_client", "ton_mainnet")
            SccpProposalFixtures.replace(SccpProposalFixtures.body(value), listOf("params", field), 1)
            assertFailsWith<IllegalArgumentException> { read(value) }
        }
        for (field in listOf("minter_code", "wallet_code", "bucket_code")) {
            val value = SccpProposalFixtures.proposal("register_route", "ton_mainnet")
            val path = listOf("deployment", "deployment", field, "depth")
            SccpProposalFixtures.replace(SccpProposalFixtures.body(value), path, 65535)
            read(value)
            SccpProposalFixtures.replace(SccpProposalFixtures.body(value), path, 65536)
            assertFailsWith<IllegalArgumentException> { read(value) }
        }
    }

    @Test
    fun repeatedDestinationWordsRejectEvenAcrossFamilies() {
        for (name in listOf("ethereum_mainnet", "ton_mainnet")) {
            val value = SccpProposalFixtures.proposal("register_route")
            val proposal = SccpProposalFixtures.inner(value)
            val other = SccpProposalFixtures.action("register_route", name)
            val otherBody = SccpProposalFixtures.map(other["payload"])
            val path = listOf("deployment", "deployment", if (name == "ton_mainnet") "master_account" else "address")
            val word = if (name == "ton_mainnet") List(12) { 0 } + List(20) { 1 } else List(20) { 1 }
            SccpProposalFixtures.replace(otherBody, path, word)
            proposal["actions"] = listOf(SccpProposalFixtures.action("register_route"), other)
            proposal["base_revisions"] =
                (if (name == "ton_mainnet") listOf("bsc_mainnet", name) else listOf(name, "bsc_mainnet")).map {
                    SccpProposalFixtures.base("route", it)
                }
            assertFailsWith<IllegalArgumentException> { read(value) }
            SccpProposalFixtures.replace(otherBody, path, word.dropLast(1) + 2)
            read(value)
        }
    }

    @Test
    fun checkpointAndGenerationProgressKeepExactBoundaryValues() {
        val value = SccpProposalFixtures.proposal("install_trusted_checkpoint")
        val checkpoint = SccpProposalFixtures.map(SccpProposalFixtures.body(value)["checkpoint"])
        checkpoint["source_height"] = SAFE_MAX
        checkpoint["source_time_ms"] = SAFE_MAX
        checkpoint["state_root"] = List(32) { 0 }
        read(value)
        val progress = SccpProposalFixtures.proposal("attest_deployment_progress")
        SccpProposalFixtures.body(progress)["at"] =
            linkedMapOf(
                "at" to "generation",
                "detail" to
                    linkedMapOf(
                        "generation" to SAFE_MAX,
                        "committee_root" to
                            List(32) {
                                1
                            },
                    ),
            )
        read(progress)
        for ((field, invalid) in listOf("generation" to 0, "generation" to SAFE_MAX + 1, "committee_root" to List(32) { 0 })) {
            val detail = SccpProposalFixtures.map(SccpProposalFixtures.map(SccpProposalFixtures.body(progress)["at"])["detail"])
            val saved = detail[field]
            detail[field] = invalid
            assertFailsWith<IllegalArgumentException> { read(progress) }
            detail[field] = saved
        }
    }

    @Test
    fun everyNestedRecordHasAnExactShape() {
        for ((action, name, path) in listOf(
            Triple("register_route", "bsc_mainnet", listOf("deployment")),
            Triple("register_route", "bsc_mainnet", listOf("deployment", "deployment")),
            Triple("register_route", "bsc_mainnet", listOf("initial_committee")),
            Triple("register_route", "ton_mainnet", listOf("deployment", "deployment")),
            Triple("register_route", "ton_mainnet", listOf("deployment", "deployment", "minter_code")),
            Triple("initialize_light_client", "bsc_mainnet", listOf("expected")),
            Triple("initialize_light_client", "bsc_mainnet", listOf("params")),
            Triple("initialize_light_client", "bsc_mainnet", listOf("bootstrap")),
            Triple("install_trusted_checkpoint", "bsc_mainnet", listOf("checkpoint")),
            Triple("set_parameters", "bsc_mainnet", listOf("next")),
            Triple("attest_deployment_progress", "bsc_mainnet", listOf("at")),
        )) {
            val value = SccpProposalFixtures.proposal(action, name)
            read(value)
            var target = SccpProposalFixtures.body(value)
            for (field in path) target = SccpProposalFixtures.map(target[field])
            for (field in target.keys.toList()) {
                val saved = target.remove(field)
                assertFailsWith<IllegalArgumentException>("$action.$path.$field") { read(value) }
                target[field] = saved
            }
            target["extra"] = null
            assertFailsWith<IllegalArgumentException> { read(value) }
        }
    }

    @Test
    fun releaseRejectsInvalidAmountAndMemoBeforeNativeAccountAdmission() {
        for ((field, invalid) in listOf(
            "amount" to "0", "amount" to "01", "amount" to "-1", "amount" to "1.0",
            "amount" to 1, "amount" to BigInteger.ONE.shiftLeft(128).toString(),
            "amount" to "9".repeat(10000), "memo" to null, "memo" to 1,
            "memo" to "a".repeat(257), "memo" to "é".repeat(129), "memo" to "🦊".repeat(65),
        )) {
            val value = SccpProposalFixtures.proposal("release_stranded")
            SccpProposalFixtures.body(value)[field] = invalid
            assertFailsWith<IllegalArgumentException> { read(value) }
        }
        val encoded = String(SccpProposalFixtures.encode(SccpProposalFixtures.proposal("release_stranded")), StandardCharsets.UTF_8)
        for (escaped in listOf("\\ud800", "\\udc00", "\\ud800a", "\\ud800\\ud800")) {
            // Invalid surrogate escapes are rejected by the JSON parser itself.
            assertFailsWith<IllegalStateException> {
                ParliamentApiV1.Proposal.fromJson(
                    encoded.replace("\"memo\":\"\"", "\"memo\":\"$escaped\"").toByteArray(StandardCharsets.UTF_8),
                )
            }
        }
    }

    @Test
    fun sharedRustSubjectFixturesAgree() {
        val path =
            generateSequence(Paths.get("").toAbsolutePath()) { it.parent }
                .map { it.resolve("fixtures/sccp/governance_v1.json") }
                .first { Files.isRegularFile(it) }
        val fixture = SccpProposalFixtures.map(JsonParser.parse(String(Files.readAllBytes(path), StandardCharsets.UTF_8)))
        for (rowValue in fixture["rows"] as List<*>) {
            val row = SccpProposalFixtures.map(rowValue)
            val input = SccpProposalFixtures.map(row["input"])
            val value = SccpProposalFixtures.proposal("remove_staged")
            val proposal = SccpProposalFixtures.inner(value)
            val networkHex = (input["network_id"] as String).removePrefix("0x")
            proposal["network_id"] =
                NetworkId.fromBytes(
                    networkHex.chunked(2).map { it.toInt(16).toByte() }.toByteArray(),
                ).literal
            proposal["base_revisions"] =
                (input["base_revisions"] as List<*>).map {
                    val entry = SccpProposalFixtures.map(it)
                    val subject = SccpProposalFixtures.map(entry["subject"])
                    SccpProposalFixtures.base(
                        subject["kind"] as String,
                        (subject["network"] as? String ?: "").replace('-', '_'),
                        entry["revision"],
                    )
                }
            proposal["actions"] =
                (input["actions"] as List<*>).map {
                    val entry = SccpProposalFixtures.map(it)
                    linkedMapOf(
                        "action" to entry["kind"],
                        "payload" to entry.filterKeys {
                                key ->
                            key != "kind" && key != "network"
                        } + ("network" to SccpProposalFixtures.network((entry["network"] as String).replace('-', '_'))),
                    )
                }
            if (SccpProposalFixtures.map(row["expected"])["static_outcome"] == "Accepted") {
                read(value)
            } else {
                assertFailsWith<IllegalArgumentException>(row["label"] as String) { read(value) }
            }
        }
    }

    private fun read(value: Map<String, Any?>): ParliamentApiV1.Proposal =
        ParliamentApiV1.Proposal.fromJson(SccpProposalFixtures.encode(value))

    class InvalidBody(val action: String, val path: List<String>, val value: Any?) {
        override fun toString(): String = "$action.${path.joinToString(".")}=${value.toString().take(60)}"
    }

    companion object {
        private const val SAFE_MAX = 9_007_199_254_740_991L

        @JvmStatic fun actions() = SccpProposalFixtures.subjects.keys.filter { it != "release_stranded" }

        @JvmStatic fun invalidBodies(): List<InvalidBody> {
            val cases = mutableListOf<InvalidBody>()

            fun add(
                action: String,
                path: String,
                value: Any?,
            ) {
                cases.add(InvalidBody(action, path.split('.'), value))
            }
            for (action in listOf(
                "register_route", "activate_revision", "deactivate_outbound", "retire_revision",
                "remove_staged", "quarantine_revision", "set_destination_paused",
                "attest_deployment_progress", "open_reconciliation",
            )) {
                for (value in listOf(0, -1, true, 1.0, "1", 0x1_0000_0000L)) add(action, "revision", value)
            }
            for (action in listOf("set_taira_paused", "set_destination_paused")) {
                for (value in listOf(0, "true", null)) add(action, "paused", value)
            }
            add("switch_revision", "to", 1)
            add("switch_revision", "from", 0)
            add("activate_light_client_profile", "version", 1)
            for (value in listOf(
                List(32) {
                    0
                },
                List(31) { 1 },
                List(32) { true },
                List(32) { 256 },
            )) add("activate_light_client_profile", "profile_hash", value)
            add("open_reconciliation", "window_ms", 0)
            add("open_reconciliation", "window_ms", SAFE_MAX + 1)
            add("set_fast_pause_suspended", "duration_ms", 7_776_000_001L)
            add("attest_deployment_progress", "at.detail", emptyMap<String, Any?>())
            add("install_trusted_checkpoint", "network", SccpProposalFixtures.network("ton_mainnet"))
            add("install_trusted_checkpoint", "checkpoint.source_height", SAFE_MAX + 1)
            add("install_trusted_checkpoint", "checkpoint.state_root", List(31) { 0 })
            add("register_route", "deployment.deployment.address", List(20) { 0 })
            add("register_route", "deployment.family", "ton")
            for (field in listOf("generation", "start_height", "until_ms")) {
                add("register_route", "initial_committee.$field", 0)
                add("register_route", "initial_committee.$field", SAFE_MAX + 1)
            }
            add("register_route", "initial_committee.committee_root", List(32) { 0 })
            for (value in listOf(
                "0",
                "01",
                "-1",
                "1.0",
                1,
                BigInteger.ONE.shiftLeft(128).toString(),
                "9".repeat(10000),
            )) add("register_route", "max_wrapped_supply", value)
            add("initialize_light_client", "expected.expected", "present")
            add("initialize_light_client", "expected.detail", emptyMap<String, Any?>())
            add("initialize_light_client", "params.network", SccpProposalFixtures.network("ethereum_mainnet"))
            add("initialize_light_client", "bootstrap.network", SccpProposalFixtures.network("ton_mainnet"))
            for (value in listOf("", "AA", "AB==", " AA==", "AA==\n", "AA-_", 1)) add("initialize_light_client", "bootstrap.bytes", value)
            for (field in listOf(
                "ws_bound_ms", "checkpoint_stride", "set_retention_ms", "checkpoint_prune_after_ms",
                "max_updates_per_advance", "max_segment_headers", "max_ancestry_headers",
                "max_backfill_headers", "max_advance_bytes", "max_proof_bytes",
            )) add(
                "initialize_light_client",
                "params.$field",
                0,
            )
            add("initialize_light_client", "params.max_proof_bytes", 0x1_0000_0000L)
            add("initialize_light_client", "params.set_retention_ms", SAFE_MAX + 1)
            for ((field, value) in listOf(
                "enabled" to 1, "committee_validity_ms" to 2_592_000_001L, "committee_validity_ms" to 1,
                "committee_heartbeat_ms" to 3_599_999, "committee_heartbeat_ms" to SAFE_MAX,
                "forgery_evidence_grace_ms" to 86_399_999, "forgery_evidence_grace_ms" to 2_592_000_001L,
                "destination_progress_cap_ms" to 2_592_000_001L, "outbound_ttl_ms" to 86_399_999,
                "outbound_ttl_ms" to 7_776_000_001L, "min_outbound_amount" to "0", "min_outbound_amount" to "1000000000000001",
                "inbound_self_claim_fee" to "10000000001", "max_exempt_transactions_per_block" to 7,
                "max_exempt_transactions_per_block" to 1025, "fast_pause_panel_seats" to 6,
                "fast_pause_panel_term_ms" to 3_599_999, "fast_pause_panel_term_ms" to SAFE_MAX + 1,
                "fast_pause_seat_accept_window_ms" to 599_999, "fast_pause_seat_accept_window_ms" to 302_400_001,
                "fast_pause_endorse_window_ms" to 299_999, "fast_pause_endorse_window_ms" to 21_600_001,
                "fast_pause_renew_lead_ms" to 3_599_999, "fast_pause_renew_lead_ms" to 432_000_001,
                "fast_pause_hold_ms" to 2_505_600_001L,
            )) add("set_parameters", "next.$field", value)
            return cases
        }
    }
}

/** Shared authored payloads for pure parsing and mandatory native recipient tests. */
internal object SccpProposalFixtures {
    val networks = listOf("ethereum_mainnet", "bsc_mainnet", "ton_mainnet")
    val subjects =
        linkedMapOf(
            "register_route" to "route", "activate_revision" to "route", "switch_revision" to "route",
            "deactivate_outbound" to "route", "retire_revision" to "route", "remove_staged" to "route",
            "release_stranded" to "route", "set_taira_paused" to "route", "set_destination_paused" to "route_control",
            "initialize_light_client" to "light_client", "install_trusted_checkpoint" to "light_client",
            "freeze_light_client" to "light_client", "set_parameters" to "parameters",
            "activate_light_client_profile" to "light_client", "attest_deployment_progress" to "forgery_hold",
            "quarantine_revision" to "forgery_hold", "open_reconciliation" to "forgery_hold",
            "set_fast_pause_suspended" to "fast_pause_control",
        )
    private const val NETWORK_ID = "hash:A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5#95D7"

    fun network(name: String = "bsc_mainnet"): MutableMap<String, Any?> = linkedMapOf("network" to name, "profile" to null)

    fun base(
        tag: String,
        name: String,
        revision: Any? = 0,
    ): MutableMap<String, Any?> =
        linkedMapOf(
            "subject" to linkedMapOf("subject" to tag, "key" to if (tag == "parameters") null else network(name)),
            "revision" to revision,
        )

    fun proposal(
        action: String,
        name: String = "bsc_mainnet",
    ): MutableMap<String, Any?> =
        linkedMapOf(
            "kind" to "SccpRouteGovernance",
            "payload" to
                linkedMapOf(
                    "proposal" to
                        linkedMapOf(
                            "network_id" to NETWORK_ID, "base_revisions" to listOf(base(subjects.getValue(action), name)),
                            "actions" to listOf(action(action, name)),
                        ),
                ),
        )

    @Suppress("UNCHECKED_CAST")
    fun map(value: Any?): MutableMap<String, Any?> = value as MutableMap<String, Any?>

    fun inner(value: Map<String, Any?>) = map(map(value["payload"])["proposal"])

    fun body(value: Map<String, Any?>) = map(map((inner(value)["actions"] as List<*>).first())["payload"])

    fun replace(
        value: MutableMap<String, Any?>,
        path: List<String>,
        replacement: Any?,
    ) {
        var target = value
        for (field in path.dropLast(1)) target = map(target[field])
        target[path.last()] = replacement
    }

    fun encode(value: Map<String, Any?>) = JsonEncoder.encode(value).toByteArray(StandardCharsets.UTF_8)

    fun action(
        action: String,
        name: String = "bsc_mainnet",
    ): MutableMap<String, Any?> {
        val body: MutableMap<String, Any?> = linkedMapOf("network" to network(name))
        when (action) {
            "activate_revision", "deactivate_outbound", "retire_revision", "remove_staged", "quarantine_revision" -> body["revision"] = 1
            "switch_revision" -> body.putAll(mapOf("from" to 1, "to" to 2))
            "set_taira_paused" -> body["paused"] = false
            "set_destination_paused" -> body.putAll(mapOf("revision" to 1, "paused" to true))
            "freeze_light_client" -> Unit
            "release_stranded" ->
                body.putAll(
                    mapOf(
                        "amount" to "1",
                        "recipient" to "sorauﾛ1PｺfMﾇﾘｾﾄoﾂﾊﾔH7ZdﾘhﾚmAｸdnｳu1ｱﾄ1ｺﾋuSﾑﾀﾇﾐuHEB5DP",
                        "memo" to "",
                    ),
                )
            "activate_light_client_profile" -> body.putAll(mapOf("version" to 2, "profile_hash" to List(32) { 1 }))
            "attest_deployment_progress" -> body.putAll(mapOf("revision" to 1, "at" to linkedMapOf("at" to "frozen", "detail" to null)))
            "open_reconciliation" -> body.putAll(mapOf("revision" to 1, "window_ms" to 1))
            "set_fast_pause_suspended" -> body["duration_ms"] = 0
            "install_trusted_checkpoint" ->
                body["checkpoint"] =
                    linkedMapOf(
                        "source_height" to 0,
                        "block_hash" to
                            List(32) {
                                0
                            },
                        "state_root" to null, "receipts_or_tx_root" to List(32) { 0 }, "source_time_ms" to 0,
                    )
            "register_route" ->
                body.putAll(
                    mapOf(
                        "revision" to 1,
                        "max_wrapped_supply" to "1",
                        "initial_committee" to
                            linkedMapOf(
                                "generation" to 1,
                                "committee_root" to
                                    List(32) {
                                        1
                                    },
                                "start_height" to 1, "until_ms" to 1,
                            ),
                        "deployment" to
                            if (name == "ton_mainnet") {
                                linkedMapOf(
                                    "family" to "ton",
                                    "deployment" to
                                        linkedMapOf(
                                            "master_account" to
                                                List(32) {
                                                    1
                                                },
                                            "minter_code" to
                                                linkedMapOf(
                                                    "hash" to
                                                        List(32) {
                                                            0
                                                        },
                                                    "depth" to 0,
                                                ),
                                            "wallet_code" to
                                                linkedMapOf(
                                                    "hash" to
                                                        List(32) {
                                                            0
                                                        },
                                                    "depth" to 0,
                                                ),
                                            "bucket_code" to
                                                linkedMapOf(
                                                    "hash" to
                                                        List(32) {
                                                            0
                                                        },
                                                    "depth" to 0,
                                                ),
                                        ),
                                )
                            } else {
                                linkedMapOf(
                                    "family" to "evm",
                                    "deployment" to
                                        linkedMapOf(
                                            "address" to
                                                List(20) {
                                                    1
                                                },
                                            "runtime_code_hash" to List(32) { 0 },
                                        ),
                                )
                            },
                    ),
                )
            "initialize_light_client" ->
                body.putAll(
                    mapOf(
                        "expected" to linkedMapOf("expected" to "absent", "detail" to null),
                        "bootstrap" to linkedMapOf("network" to network(name), "bytes" to "AA=="),
                        "params" to
                            linkedMapOf(
                                "network" to network(name),
                                "ws_bound_ms" to if (name == "ton_mainnet") 0 else 1,
                                "checkpoint_stride" to if (name == "ton_mainnet") 0 else 1,
                                "set_retention_ms" to 1,
                                "checkpoint_prune_after_ms" to 1,
                                "max_updates_per_advance" to 1,
                                "max_segment_headers" to 1,
                                "max_ancestry_headers" to 1,
                                "max_backfill_headers" to 1,
                                "max_advance_bytes" to 1,
                                "max_proof_bytes" to 1,
                            ),
                    ),
                )
            "set_parameters" -> {
                body.clear()
                body["next"] =
                    linkedMapOf(
                        "enabled" to true,
                        "committee_validity_ms" to 1_209_600_000L,
                        "committee_heartbeat_ms" to 86_400_000,
                        "forgery_evidence_grace_ms" to 604_800_000,
                        "destination_progress_cap_ms" to 2_592_000_000L,
                        "outbound_ttl_ms" to 604_800_000,
                        "min_outbound_amount" to "1000000000",
                        "inbound_self_claim_fee" to "10000000",
                        "max_exempt_transactions_per_block" to 32,
                        "fast_pause_panel_seats" to 9,
                        "fast_pause_panel_term_ms" to 604_800_000,
                        "fast_pause_seat_accept_window_ms" to 43_200_000,
                        "fast_pause_endorse_window_ms" to 1_800_000,
                        "fast_pause_renew_lead_ms" to 86_400_000,
                        "fast_pause_hold_ms" to 864_000_000,
                    )
            }
            else -> error("missing action fixture: $action")
        }
        return linkedMapOf("action" to action, "payload" to body)
    }
}
