package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.text.Normalizer
import java.util.Base64
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.address.PublicKeyPayload
import org.hyperledger.iroha.sdk.address.decodePublicKeyLiteral
import org.hyperledger.iroha.sdk.address.encodePublicKeyMultihash
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.model.requireCanonicalV1ContractAddress

/** Recursive fail-closed admission for the exact first-release proposal wire contract. */
internal object ParliamentProposalValidatorV1 {
    private val U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private val FIRST_RELEASE_MAX_EXACT_JSON_U64 = BigInteger("9007199254740991")
    private const val SCCP_GOVERNANCE_MAX_ENTRIES = 16
    private val SCCP_EXTERNAL_NETWORKS =
        listOf(
            "ethereum_mainnet",
            "bsc_mainnet",
            "ton_mainnet",
        )

    // Rust enum declaration order fixes the canonical subject-revision list.
    private val SCCP_SUBJECTS =
        listOf(
            "route",
            "route_control",
            "light_client",
            "parameters",
            "forgery_hold",
            "fast_hold",
            "fast_pause_control",
        )
    private val SCCP_ACTIONS =
        mapOf(
            "register_route" to ("route" to setOf("revision", "deployment", "initial_committee", "max_wrapped_supply")),
            "activate_revision" to ("route" to setOf("revision")),
            "switch_revision" to ("route" to setOf("from", "to")),
            "deactivate_outbound" to ("route" to setOf("revision")),
            "retire_revision" to ("route" to setOf("revision")),
            "remove_staged" to ("route" to setOf("revision")),
            "release_stranded" to ("route" to setOf("amount", "recipient", "memo")),
            "set_taira_paused" to ("route" to setOf("paused")),
            "set_destination_paused" to ("route_control" to setOf("revision", "paused")),
            "initialize_light_client" to ("light_client" to setOf("expected", "params", "bootstrap")),
            "install_trusted_checkpoint" to ("light_client" to setOf("checkpoint")),
            "freeze_light_client" to ("light_client" to emptySet()),
            "set_parameters" to ("parameters" to setOf("next")),
            "activate_light_client_profile" to ("light_client" to setOf("version", "profile_hash")),
            "attest_deployment_progress" to ("forgery_hold" to setOf("revision", "at")),
            "quarantine_revision" to ("forgery_hold" to setOf("revision")),
            "open_reconciliation" to ("forgery_hold" to setOf("revision", "window_ms")),
            "set_fast_pause_suspended" to ("fast_pause_control" to setOf("duration_ms")),
        )
    private const val JSON_SAFE_MAX = 9_007_199_254_740_991L
    private val SCCP_PARAMETER_BOUNDS =
        mapOf(
            "committee_validity_ms" to 0L..2_592_000_000L,
            "committee_heartbeat_ms" to 3_600_000L..JSON_SAFE_MAX,
            "forgery_evidence_grace_ms" to 86_400_000L..2_592_000_000L,
            "destination_progress_cap_ms" to 0L..2_592_000_000L,
            "outbound_ttl_ms" to 86_400_000L..7_776_000_000L,
            "max_exempt_transactions_per_block" to 8L..1024L,
            "fast_pause_panel_seats" to 5L..9L,
            "fast_pause_panel_term_ms" to 3_600_000L..JSON_SAFE_MAX,
            "fast_pause_seat_accept_window_ms" to 600_000L..JSON_SAFE_MAX,
            "fast_pause_endorse_window_ms" to 300_000L..21_600_000L,
            "fast_pause_renew_lead_ms" to 0L..JSON_SAFE_MAX,
            "fast_pause_hold_ms" to 0L..2_505_600_000L,
        )
    /** Exact `ChangeSetV1` field set; its contents pass through to node admission. */
    private val RUNTIME_CHANGE_SET_FIELDS =
        setOf(
            "network", "root", "predecessor", "successor", "effective_height",
            "publish", "transitions", "qualification",
        )
    private val KEBAB = Regex("[a-z0-9]+(?:-[a-z0-9]+)*")
    private val ALPHANUMERIC_PRERELEASE = Regex("(?=.*[A-Za-z-])[A-Za-z0-9-]+")

    @Suppress("UNCHECKED_CAST")
    fun parse(bytes: ByteArray): Map<String, Any?> {
        val text = String(bytes, StandardCharsets.UTF_8)
        require(text.toByteArray(StandardCharsets.UTF_8).contentEquals(bytes)) {
            "proposal must be UTF-8 JSON"
        }
        val proposal = objectValue(JsonParser.parse(text), "proposal")
        exact(proposal, setOf("kind", "payload"), "proposal")
        val kind = text(proposal["kind"], "proposal.kind")
        require(kind in ParliamentApiV1.PROPOSAL_KINDS) { "proposal.kind is unknown or retired" }
        val payload = objectValue(proposal["payload"], "proposal.payload")
        when (kind) {
            "DeployContract" -> deployContract(payload)
            "RuntimeUpgrade" -> runtimeUpgrade(payload)
            "SccpRouteGovernance" -> sccpRoute(payload)
            "ValidationFeePolicy" -> validationFeePolicyProposal(payload)
            "ValidationFeePayoutLifecycle" -> validationFeePayoutLifecycle(payload)
            "MusubiRegistryGovernance" -> musubiAction(payload)
            "SorafsProviderGovernance" -> sorafsProvider(payload)
            "ContractLifecycleGovernance" -> contractLifecycle(payload)
            "ContractEmergencyHold" -> contractEmergencyHold(payload)
            "GlobalDataTriggerPermissionGovernance" -> globalDataTriggerPermission(payload)
            else -> throw IllegalArgumentException("proposal.kind is unknown or retired")
        }
        return proposal
    }

    private fun deployContract(value: Map<String, Any?>) {
        exact(
            value,
            setOf(
                "proposal_operator",
                "contract_address",
                "code_hash",
                "abi_hash",
                "abi_version",
                "manifest_provenance",
            ),
            "DeployContract",
        )
        account(value["proposal_operator"], "proposal_operator")
        requireCanonicalV1ContractAddress(text(value["contract_address"], "contract_address"))
        lowerHex32(value["code_hash"], "code_hash")
        lowerHex32(value["abi_hash"], "abi_hash")
        require(uint(value["abi_version"], "abi_version") == BigInteger.ONE) {
            "abi_version must equal 1"
        }
        value["manifest_provenance"]?.let {
            manifestProvenance(objectValue(it, "manifest_provenance"), "manifest_provenance")
        }
    }

    private fun manifestProvenance(value: Map<String, Any?>, label: String) {
        exact(value, setOf("signer", "signature"), label)
        canonicalPublicKey(value["signer"], "$label.signer")
        canonicalSignature(value["signature"], "$label.signature")
    }

    /**
     * Admits the closed governed change-set proposal (`specs/live_runtime_upgrades.md` §6):
     * schedule one `ChangeSetV1` or cancel one scheduled change-set id, with the notice and
     * proposal time the node froze at admission. The SDK checks every envelope field set and
     * first-release exact JSON integer; the node authenticates the change set's network, root,
     * selections, publications, transitions and qualification digest.
     */
    private fun runtimeUpgrade(value: Map<String, Any?>) {
        exact(value, setOf("proposal_operator", "action", "notice", "proposed_ms"), "RuntimeUpgrade")
        account(value["proposal_operator"], "RuntimeUpgrade.proposal_operator")
        val action = objectValue(value["action"], "RuntimeUpgrade.action")
        exact(action, setOf("kind", "content"), "RuntimeUpgrade.action")
        when (text(action["kind"], "RuntimeUpgrade.action.kind")) {
            "Schedule" -> {
                val changeSet = objectValue(action["content"], "RuntimeUpgrade.action.content")
                exact(changeSet, RUNTIME_CHANGE_SET_FIELDS, "RuntimeUpgrade.action.content")
                uint(changeSet["effective_height"], "RuntimeUpgrade.action.content.effective_height")
            }
            "Cancel" -> lowerHex32(action["content"], "RuntimeUpgrade.action.content")
            else -> throw IllegalArgumentException("RuntimeUpgrade.action.kind must be Schedule or Cancel")
        }
        val notice = objectValue(value["notice"], "RuntimeUpgrade.notice")
        exact(notice, setOf("blocks", "ms", "approval_cushion_ms"), "RuntimeUpgrade.notice")
        uint(notice["blocks"], "RuntimeUpgrade.notice.blocks")
        uint(notice["ms"], "RuntimeUpgrade.notice.ms")
        uint(notice["approval_cushion_ms"], "RuntimeUpgrade.notice.approval_cushion_ms")
        uint(value["proposed_ms"], "RuntimeUpgrade.proposed_ms")
    }

    /**
     * Checks the envelope, all static action rules and exact subject-revision set
     * (specs/sccp.md §4.14.3). The node checks live network identity, authorization,
     * authenticated committee/bootstrap evidence and committed state.
     */
    private fun sccpRoute(value: Map<String, Any?>) {
        exact(value, setOf("proposal"), "SccpRouteGovernance")
        val proposal = objectValue(value["proposal"], "SccpRouteGovernance.proposal")
        exact(
            proposal,
            setOf("network_id", "base_revisions", "actions"),
            "SccpRouteGovernance.proposal",
        )
        NetworkId.parse(text(proposal["network_id"], "proposal.network_id"))
        val baseRevisions = list(proposal["base_revisions"], "proposal.base_revisions")
        require(baseRevisions.size in 1..SCCP_GOVERNANCE_MAX_ENTRIES) {
            "proposal.base_revisions must hold 1 to $SCCP_GOVERNANCE_MAX_ENTRIES entries"
        }
        val actualSubjects =
            baseRevisions.mapIndexed { index, item ->
                val label = "proposal.base_revisions[$index]"
                val entry = objectValue(item, label)
                exact(entry, setOf("subject", "revision"), label)
                uint(entry["revision"], "$label.revision")
                sccpGovernanceSubject(objectValue(entry["subject"], "$label.subject"), "$label.subject")
            }
        val actions = list(proposal["actions"], "proposal.actions")
        require(actions.size in 1..SCCP_GOVERNANCE_MAX_ENTRIES) {
            "proposal.actions must hold 1 to $SCCP_GOVERNANCE_MAX_ENTRIES entries"
        }
        val subjects = sortedSetOf<Int>()
        val destinations = mutableSetOf<List<Byte>>()
        actions.forEachIndexed { index, item ->
            val label = "proposal.actions[$index]"
            val action = objectValue(item, label)
            exact(action, setOf("action", "payload"), label)
            val tag = text(action["action"], "$label.action")
            val (subject, fields) =
                SCCP_ACTIONS[tag]
                    ?: throw IllegalArgumentException("$label.action is unknown")
            val body = objectValue(action["payload"], "$label.payload")
            exact(body, if (subject == "parameters") fields else fields + "network", "$label.payload")
            val network =
                if (subject == "parameters") {
                    null
                } else {
                    sccpExternalNetwork(objectValue(body["network"], "$label.payload.network"), "$label.payload.network")
                }
            subjects.add(sccpSubjectOrder(subject, network))
            sccpAction(tag, body, network, "$label.payload", destinations)
        }
        require(actualSubjects == subjects.toList()) {
            "proposal.base_revisions must list exactly the action subjects in canonical order, without duplicates"
        }
    }

    private fun sccpAction(
        action: String,
        value: Map<String, Any?>,
        network: String?,
        label: String,
        destinations: MutableSet<List<Byte>>,
    ) {
        if ("revision" in value) sccpUint(value["revision"], "$label.revision", 1, 0xffff_ffffL)
        when (action) {
            "register_route" -> {
                val word = sccpDeployment(value["deployment"], requireNotNull(network), "$label.deployment")
                sccpCommitteePin(value["initial_committee"], "$label.initial_committee")
                val bits = if (network == "ton_mainnet") 96 else 128
                sccpAmount(
                    value["max_wrapped_supply"],
                    "$label.max_wrapped_supply",
                    BigInteger.ONE,
                    BigInteger.ONE.shiftLeft(bits).subtract(BigInteger.ONE),
                )
                require(destinations.add(word.toList())) { "$label.deployment repeats a registered destination word" }
            }
            "release_stranded" -> {
                sccpAmount(
                    value["amount"],
                    "$label.amount",
                    BigInteger.ONE,
                    BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE),
                )
                val memo = string(value["memo"], "$label.memo")
                require(
                    memo.length <= 256 && StandardCharsets.UTF_8.newEncoder().canEncode(memo) &&
                        memo.toByteArray(StandardCharsets.UTF_8).size <= 256,
                ) {
                    "$label.memo must contain valid Unicode scalar values and at most 256 UTF-8 bytes"
                }
                account(value["recipient"], "$label.recipient")
            }
            "initialize_light_client" -> {
                val expected = objectValue(value["expected"], "$label.expected")
                exact(expected, setOf("expected", "detail"), "$label.expected")
                require(expected["expected"] in listOf("absent", "unusable") && expected["detail"] == null) {
                    "$label.expected must name absent or unusable with null detail"
                }
                sccpLightClientParams(value["params"], requireNotNull(network), "$label.params")
                sccpBootstrap(value["bootstrap"], network, "$label.bootstrap")
            }
            "set_parameters" -> sccpParameters(value["next"], "$label.next")
            "switch_revision" -> {
                val from = sccpUint(value["from"], "$label.from", 1, 0xffff_ffffL)
                val to = sccpUint(value["to"], "$label.to", 1, 0xffff_ffffL)
                require(from != to) { "$label.from and .to must name distinct revisions" }
            }
            "set_taira_paused", "set_destination_paused" ->
                require(value["paused"] is Boolean) { "$label.paused must be a boolean" }
            "activate_light_client_profile" -> {
                sccpUint(value["version"], "$label.version", 2, 0xffff_ffffL)
                bytes(value["profile_hash"], 32, "$label.profile_hash", true)
            }
            "install_trusted_checkpoint" -> {
                require(network != "ton_mainnet") { "$label.network cannot be TON; its light client reads no checkpoints" }
                sccpCheckpoint(value["checkpoint"], "$label.checkpoint")
            }
            "attest_deployment_progress" -> sccpProgress(value["at"], "$label.at")
            "open_reconciliation" -> sccpUint(value["window_ms"], "$label.window_ms", 1)
            "set_fast_pause_suspended" -> sccpUint(value["duration_ms"], "$label.duration_ms", 0, 7_776_000_000L)
        }
    }

    private fun sccpUint(
        value: Any?,
        label: String,
        minimum: Long = 0,
        maximum: Long = JSON_SAFE_MAX,
    ): Long {
        val number = uint(value, label).longValueExact()
        require(number in minimum..maximum) { "$label must be within $minimum..$maximum" }
        return number
    }

    private fun sccpAmount(
        value: Any?,
        label: String,
        minimum: BigInteger,
        maximum: BigInteger,
    ) {
        val literal = string(value, label)
        require(literal.length <= maximum.toString().length && Regex("0|[1-9][0-9]*").matches(literal)) {
            "$label must be a bounded canonical unsigned decimal string"
        }
        require(BigInteger(literal) in minimum..maximum) { "$label must be within $minimum..$maximum" }
    }

    private fun sccpDeployment(
        value: Any?,
        network: String,
        label: String,
    ): ByteArray {
        val record = objectValue(value, label)
        exact(record, setOf("family", "deployment"), label)
        val body = objectValue(record["deployment"], "$label.deployment")
        if (record["family"] == "evm" && network != "ton_mainnet") {
            exact(body, setOf("address", "runtime_code_hash"), "$label.deployment")
            val address = bytes(body["address"], 20, "$label.deployment.address", true)
            bytes(body["runtime_code_hash"], 32, "$label.deployment.runtime_code_hash", false)
            return ByteArray(12) + address
        }
        if (record["family"] == "ton" && network == "ton_mainnet") {
            exact(body, setOf("master_account", "minter_code", "wallet_code", "bucket_code"), "$label.deployment")
            val word = bytes(body["master_account"], 32, "$label.deployment.master_account", true)
            for (field in listOf("minter_code", "wallet_code", "bucket_code")) {
                val code = objectValue(body[field], "$label.deployment.$field")
                exact(code, setOf("hash", "depth"), "$label.deployment.$field")
                bytes(code["hash"], 32, "$label.deployment.$field.hash", false)
                sccpUint(code["depth"], "$label.deployment.$field.depth", 0, 65535)
            }
            return word
        }
        throw IllegalArgumentException("$label.family must match the external network")
    }

    private fun sccpCommitteePin(
        value: Any?,
        label: String,
    ) {
        val pin = objectValue(value, label)
        exact(pin, setOf("generation", "committee_root", "start_height", "until_ms"), label)
        for (field in listOf("generation", "start_height", "until_ms")) sccpUint(pin[field], "$label.$field", 1)
        bytes(pin["committee_root"], 32, "$label.committee_root", true)
    }

    private fun sccpLightClientParams(
        value: Any?,
        network: String,
        label: String,
    ) {
        val jsonFields = setOf("ws_bound_ms", "set_retention_ms", "checkpoint_stride", "checkpoint_prune_after_ms")
        val u32Fields =
            setOf(
                "max_updates_per_advance",
                "max_segment_headers",
                "max_ancestry_headers",
                "max_backfill_headers",
                "max_advance_bytes",
                "max_proof_bytes",
            )
        val params = objectValue(value, label)
        exact(params, jsonFields + u32Fields + "network", label)
        require(sccpExternalNetwork(objectValue(params["network"], "$label.network"), "$label.network") == network) {
            "$label.network must match the action network"
        }
        for (field in jsonFields) {
            val derived = network == "ton_mainnet" && field in setOf("ws_bound_ms", "checkpoint_stride")
            sccpUint(params[field], "$label.$field", if (derived) 0 else 1, if (derived) 0 else JSON_SAFE_MAX)
        }
        for (field in u32Fields) sccpUint(params[field], "$label.$field", 1, 0xffff_ffffL)
    }

    private fun sccpBootstrap(
        value: Any?,
        network: String,
        label: String,
    ) {
        val bootstrap = objectValue(value, label)
        exact(bootstrap, setOf("network", "bytes"), label)
        require(sccpExternalNetwork(objectValue(bootstrap["network"], "$label.network"), "$label.network") == network) {
            "$label.network must match the action network"
        }
        val encoded = string(bootstrap["bytes"], "$label.bytes")
        val maximum = 1_048_576
        require(encoded.length in 1..(4 * ((maximum + 2) / 3))) { "$label.bytes must encode 1..$maximum bytes" }
        canonicalBase64(encoded, "$label.bytes")
        val length =
            encoded.length / 4 * 3 -
                if (encoded.endsWith("==")) {
                    2
                } else if (encoded.endsWith("=")) {
                    1
                } else {
                    0
                }
        require(length <= maximum) { "$label.bytes exceeds $maximum bytes" }
        // Per-chain native decoding and bootstrap authentication remain node-owned.
    }

    private fun sccpCheckpoint(
        value: Any?,
        label: String,
    ) {
        val checkpoint = objectValue(value, label)
        exact(checkpoint, setOf("source_height", "block_hash", "state_root", "receipts_or_tx_root", "source_time_ms"), label)
        for (field in listOf("source_height", "source_time_ms")) uint(checkpoint[field], "$label.$field")
        for (field in listOf("block_hash", "receipts_or_tx_root")) bytes(checkpoint[field], 32, "$label.$field", false)
        checkpoint["state_root"]?.let { bytes(it, 32, "$label.state_root", false) }
    }

    private fun sccpProgress(
        value: Any?,
        label: String,
    ) {
        val progress = objectValue(value, label)
        exact(progress, setOf("at", "detail"), label)
        when (progress["at"]) {
            "frozen" -> require(progress["detail"] == null) { "$label.detail must be null" }
            "generation" -> {
                val detail = objectValue(progress["detail"], "$label.detail")
                exact(detail, setOf("generation", "committee_root"), "$label.detail")
                sccpUint(detail["generation"], "$label.detail.generation", 1)
                bytes(detail["committee_root"], 32, "$label.detail.committee_root", true)
            }
            else -> throw IllegalArgumentException("$label.at must be frozen or generation")
        }
    }

    private fun sccpParameters(
        value: Any?,
        label: String,
    ) {
        val params = objectValue(value, label)
        exact(params, SCCP_PARAMETER_BOUNDS.keys + setOf("enabled", "min_outbound_amount", "inbound_self_claim_fee"), label)
        require(params["enabled"] is Boolean) { "$label.enabled must be a boolean" }
        val parsed =
            SCCP_PARAMETER_BOUNDS.mapValues { (field, bounds) ->
                sccpUint(params[field], "$label.$field", bounds.first, bounds.last)
            }
        sccpAmount(params["min_outbound_amount"], "$label.min_outbound_amount", BigInteger.ONE, BigInteger("1000000000000000"))
        sccpAmount(params["inbound_self_claim_fee"], "$label.inbound_self_claim_fee", BigInteger.ZERO, BigInteger("10000000000"))
        require(parsed.getValue("fast_pause_panel_seats") in setOf(5L, 7L, 9L)) { "$label.fast_pause_panel_seats must be 5, 7 or 9" }
        // Every input is at most 2^53-1, so these sums and doubles fit exactly in Long.
        val floor = parsed.getValue("fast_pause_hold_ms") + 2 * parsed.getValue("committee_heartbeat_ms") + 86_400_000
        require(parsed.getValue("committee_validity_ms") >= floor) {
            "$label.committee_validity_ms is below the hold, heartbeat and pin margin"
        }
        require(2 * parsed.getValue("fast_pause_seat_accept_window_ms") <= parsed.getValue("fast_pause_panel_term_ms")) {
            "$label.fast_pause_seat_accept_window_ms exceeds half the panel term"
        }
        val lead = parsed.getValue("fast_pause_renew_lead_ms")
        require(lead >= 2 * parsed.getValue("fast_pause_endorse_window_ms") && 2 * lead <= parsed.getValue("fast_pause_hold_ms")) {
            "$label.fast_pause_renew_lead_ms must span two endorsement windows and fit half the hold"
        }
        // Committed staking delay and Parliament attempt latency are checked at enactment.
    }

    private fun sccpGovernanceSubject(
        value: Map<String, Any?>,
        label: String,
    ): Int {
        exact(value, setOf("subject", "key"), label)
        val tag = text(value["subject"], "$label.subject")
        require(tag in SCCP_SUBJECTS) { "$label.subject is unknown" }
        val network =
            if (tag == "parameters") {
                require(value["key"] == null) { "$label.key must be null" }
                null
            } else {
                sccpExternalNetwork(objectValue(value["key"], "$label.key"), "$label.key")
            }
        return sccpSubjectOrder(tag, network)
    }

    private fun sccpSubjectOrder(
        subject: String,
        network: String?,
    ): Int = SCCP_SUBJECTS.indexOf(subject) * 4 + if (network == null) 0 else SCCP_EXTERNAL_NETWORKS.indexOf(network) + 1

    private fun sccpExternalNetwork(
        value: Map<String, Any?>,
        label: String,
    ): String {
        exact(value, setOf("network", "profile"), label)
        require(text(value["network"], "$label.network") in SCCP_EXTERNAL_NETWORKS) {
            "$label.network is unknown"
        }
        require(value["profile"] == null) { "$label.profile must be null" }
        return value["network"] as String
    }

    private fun validationFeePolicyProposal(value: Map<String, Any?>) {
        exact(
            value,
            setOf("proposal_operator", "policy"),
            "ValidationFeePolicy",
        )
        account(value["proposal_operator"], "proposal_operator")
        val policy = objectValue(value["policy"], "policy")
        validationFeePolicy(policy)

    }

    private fun validationFeePolicy(value: Map<String, Any?>) {
        exact(value, setOf(
            "schema_version", "network_id", "policy_version", "previous_policy_hash",
            "ds_asset_id", "ds_scale", "retail_schedule", "effective_from_ms", "notice_published_at_ms",
            "fee", "treasury_account_id", "charging_mode",
            "exemption_classes", "reward_custody",
        ), "validation fee policy")
        require(uint(value["schema_version"], "policy.schema_version") == BigInteger.ONE)
        NetworkId.parse(text(value["network_id"], "policy.network_id"))
        val version = u64String(value["policy_version"], "policy.policy_version", true)
        val previous = value["previous_policy_hash"]?.let { nonzeroFeeHash(it, "policy.previous_policy_hash") }
        require((version == BigInteger.ONE) == (previous == null))
        asset(value["ds_asset_id"], "policy.ds_asset_id")
        require(uint(value["ds_scale"], "policy.ds_scale") == BigInteger.valueOf(2))
        val fee = quantity(value["fee"], "policy.fee")
        require(fee != "0" && fee.substringAfter('.', "").length <= 2) { "Institutional fee must be positive exact minor units" }
        account(value["treasury_account_id"], "policy.treasury_account_id")
        val mode = objectValue(value["charging_mode"], "policy.charging_mode")
        exact(mode, setOf("charging_mode", "value"), "charging_mode")
        require(mode["charging_mode"] == "RETAIL_MONTHLY_ALLOWANCE" && mode["value"] == null)
        val effective = uint(value["effective_from_ms"], "policy.effective_from_ms").longValueExact()
        val notice = uint(value["notice_published_at_ms"], "policy.notice_published_at_ms").longValueExact()
        val boundary = java.time.Instant.ofEpochMilli(effective).atOffset(java.time.ZoneOffset.ofHours(11))
        require(boundary.dayOfMonth == 1 && boundary.toLocalTime() == java.time.LocalTime.MIDNIGHT && effective - notice >= 30L * 86_400_000)
        val schedule = objectValue(value["retail_schedule"], "policy.retail_schedule")
        exact(schedule, setOf("included_payments", "overage_minor", "maintenance_tiers"), "retail_schedule")
        require(uint(schedule["included_payments"], "included_payments") in BigInteger.ONE..BigInteger("4294967295"))
        require(uint(schedule["overage_minor"], "overage_minor") > BigInteger.ZERO)
        val tiers = list(schedule["maintenance_tiers"], "maintenance_tiers")
        require(tiers.size in 1..32)
        var previousMinimum = BigInteger.valueOf(-1)
        var previousFee = BigInteger.ZERO
        tiers.forEachIndexed { index, item ->
            val tier = objectValue(item, "maintenance_tiers[$index]")
            exact(tier, setOf("minimum_average_balance_minor", "monthly_fee_minor"), "maintenance tier")
            val minimum = uint(tier["minimum_average_balance_minor"], "minimum_average_balance_minor")
            require(minimum > previousMinimum && (index != 0 || minimum == BigInteger.ZERO))
            val monthlyFee = uint(tier["monthly_fee_minor"], "monthly_fee_minor")
            require(monthlyFee > BigInteger.ZERO && monthlyFee >= previousFee)
            previousMinimum = minimum
            previousFee = monthlyFee
        }
        val exemptions = list(value["exemption_classes"], "exemption_classes")
        require(exemptions == listOf("TREASURY_PAYOUT"))
        val custody=objectValue(value["reward_custody"],"reward_custody")
        exact(custody,setOf("contract_address","treasury_account_id","ds_asset_id","xor_asset_id","reward_pool_account_id","validator_lane_id"),"reward_custody")
        requireCanonicalV1ContractAddress(text(custody["contract_address"],"reward contract"))
        require(account(custody["treasury_account_id"],"custody treasury")==account(value["treasury_account_id"],"policy treasury"))
        require(asset(custody["ds_asset_id"],"custody asset")==asset(value["ds_asset_id"],"policy asset"))
        require(asset(custody["xor_asset_id"],"custody XOR")!=custody["ds_asset_id"])
        require(account(custody["reward_pool_account_id"],"reward pool")!=custody["treasury_account_id"])
        require(uint(custody["validator_lane_id"],"validator lane")<=BigInteger("4294967295"))
    }

    private fun validationFeePayoutLifecycle(value: Map<String, Any?>) {
        exact(value, setOf("proposal_operator", "payout_binding"), "ValidationFeePayoutLifecycle")
        account(value["proposal_operator"], "proposal_operator")
        payoutBinding(objectValue(value["payout_binding"], "payout_binding"))
    }

    private fun payoutBinding(value: Map<String, Any?>) {
        exact(value, setOf(
            "contract_address", "code_hash", "entrypoint", "treasury_account_id", "ds_asset_id", "xor_asset_id",
            "pool_contract_address", "pool_code_hash", "pool_vault_account_id", "reward_pool_account_id",
            "reference_feed_id", "reference_feed_config_version", "reference_provider_accounts",
            "max_sbd_per_attempt_minor", "max_sbd_per_day_minor", "min_interval_ms", "max_source_age_ms",
            "max_slippage_bps", "validator_lane_id", "min_reward_claim_xor_minor",
        ), "payout_binding")
        for (field in listOf("contract_address", "pool_contract_address")) requireCanonicalV1ContractAddress(text(value[field], field))
        for (field in listOf("code_hash", "pool_code_hash")) nonzeroFeeHash(value[field], field)
        require(text(value["entrypoint"], "entrypoint") == "autonomous_validation_fee_tick")
        val accounts = listOf("treasury_account_id", "pool_vault_account_id", "reward_pool_account_id").map { account(value[it], it) }
        require(accounts.distinct().size == 3)
        require(asset(value["ds_asset_id"], "ds_asset_id") != asset(value["xor_asset_id"], "xor_asset_id"))
        text(stringTuple(value["reference_feed_id"], "reference_feed_id"), "reference_feed_id")
        require(uint(value["reference_feed_config_version"], "reference_feed_config_version") in BigInteger.ONE..BigInteger("4294967295"))
        val providers = list(value["reference_provider_accounts"], "reference_provider_accounts").map { account(it, "reference provider") }
        require(providers.size == 5 && providers.distinct().size == 5)
        val attempt = uint(value["max_sbd_per_attempt_minor"], "max_sbd_per_attempt_minor")
        require(attempt > BigInteger.ZERO && uint(value["max_sbd_per_day_minor"], "max_sbd_per_day_minor") >= attempt)
        for (field in listOf("min_interval_ms", "max_source_age_ms", "min_reward_claim_xor_minor")) require(uint(value[field], field) > BigInteger.ZERO)
        require(uint(value["max_slippage_bps"], "max_slippage_bps") < BigInteger.valueOf(10000))
        require(uint(value["validator_lane_id"], "validator_lane_id") <= BigInteger("4294967295"))
    }

    private fun nonzeroFeeHash(value: Any?, label: String): String {
        require(value is String && Regex("[0-9A-F]{64}").matches(value) && value != "0".repeat(64)) {
            "$label must be nonzero canonical uppercase 32-byte hexadecimal"
        }
        return value
    }

    private fun musubiAction(value: Map<String, Any?>) {
        exact(value, setOf("kind", "value"), "MusubiRegistryGovernance")
        val kind = text(value["kind"], "MusubiRegistryGovernance.kind")
        val action = objectValue(value["value"], "MusubiRegistryGovernance.value")
        when (kind) {
            "RecoverPackageOwners" -> {
                exact(action, setOf("package", "owners", "expected_revision"), kind)
                musubiPackage(objectValue(action["package"], "$kind.package"), "$kind.package")
                val owners = list(action["owners"], "$kind.owners").mapIndexed { index, item ->
                    account(item, "$kind.owners[$index]")
                }
                require(owners.size in 1..64 && owners.distinct().size == owners.size)
                require(uint(action["expected_revision"], "$kind.expected_revision") > BigInteger.ZERO)
            }
            "RetargetAlias" -> {
                exact(action, setOf("alias", "target", "expected_revision"), kind)
                kebab(stringTuple(action["alias"], "$kind.alias"), "$kind.alias", 32)
                musubiPackage(objectValue(action["target"], "$kind.target"), "$kind.target")
                require(uint(action["expected_revision"], "$kind.expected_revision") > BigInteger.ZERO)
            }
            "TakedownArtifact" -> {
                exact(action, setOf("release", "reason", "expected_artifact_governance_revision"), kind)
                musubiRelease(objectValue(action["release"], "$kind.release"), "$kind.release")
                reason(stringTuple(action["reason"], "$kind.reason"), "$kind.reason")
                require(
                    uint(
                        action["expected_artifact_governance_revision"],
                        "$kind.expected_artifact_governance_revision",
                    ) > BigInteger.ZERO,
                )
            }
            "SetRegistryPolicy" -> {
                exact(action, setOf("policy", "expected_revision"), kind)
                val expected = uint(action["expected_revision"], "$kind.expected_revision")
                require(expected > BigInteger.ZERO)
                val revision = musubiRegistryPolicy(objectValue(action["policy"], "$kind.policy"), "$kind.policy")
                require(revision == expected + BigInteger.ONE) { "policy revision must follow expected_revision" }
            }
            else -> throw IllegalArgumentException("Musubi governance action is unsupported")
        }
    }

    private fun musubiPackage(value: Map<String, Any?>, label: String) {
        exact(value, setOf("home_dataspace", "scope", "name"), label)
        uint(value["home_dataspace"], "$label.home_dataspace")
        val scope = objectValue(value["scope"], "$label.scope")
        exact(scope, setOf("kind", "value"), "$label.scope")
        when (text(scope["kind"], "$label.scope.kind")) {
            "DataspaceRoot" -> require(scope["value"] == null) { "$label.scope.value must be null" }
            "Domain" -> canonicalName(scope["value"], "$label.scope.value")
            else -> throw IllegalArgumentException("$label.scope.kind is unsupported")
        }
        kebab(stringTuple(value["name"], "$label.name"), "$label.name", 64)
    }

    private fun musubiRelease(value: Map<String, Any?>, label: String) {
        exact(value, setOf("package", "version"), label)
        musubiPackage(objectValue(value["package"], "$label.package"), "$label.package")
        val version = objectValue(value["version"], "$label.version")
        exact(version, setOf("major", "minor", "patch", "prerelease"), "$label.version")
        uint(version["major"], "$label.version.major")
        uint(version["minor"], "$label.version.minor")
        uint(version["patch"], "$label.version.patch")
        val prerelease = list(version["prerelease"], "$label.version.prerelease")
        require(prerelease.size <= 16)
        prerelease.forEachIndexed { index, item ->
            val identifier = objectValue(item, "$label.version.prerelease[$index]")
            exact(identifier, setOf("kind", "value"), "$label.version.prerelease[$index]")
            when (text(identifier["kind"], "$label.version.prerelease[$index].kind")) {
                "Numeric" -> uint(identifier["value"], "$label.version.prerelease[$index].value")
                "AlphaNumeric" -> {
                    val literal = text(identifier["value"], "$label.version.prerelease[$index].value")
                    require(literal.toByteArray(StandardCharsets.UTF_8).size <= 64 && ALPHANUMERIC_PRERELEASE.matches(literal))
                }
                else -> throw IllegalArgumentException("unsupported prerelease identifier")
            }
        }
    }

    private fun musubiRegistryPolicy(value: Map<String, Any?>, label: String): BigInteger {
        exact(value, setOf("version", "revision", "mode", "allowlisted_dataspaces", "alias_pricing"), label)
        require(uint(value["version"], "$label.version") == BigInteger.ONE)
        val revision = uint(value["revision"], "$label.revision")
        require(revision > BigInteger.ZERO)
        val mode = objectValue(value["mode"], "$label.mode")
        exact(mode, setOf("kind", "value"), "$label.mode")
        val modeKind = text(mode["kind"], "$label.mode.kind")
        require(modeKind in setOf("Closed", "Allowlisted", "Open") && mode["value"] == null)
        val allowed = list(value["allowlisted_dataspaces"], "$label.allowlisted_dataspaces").mapIndexed { index, item ->
            uint(item, "$label.allowlisted_dataspaces[$index]")
        }
        require(allowed.zipWithNext().all { (left, right) -> left < right })
        require(modeKind == "Allowlisted" || allowed.isEmpty())
        val pricing = objectValue(value["alias_pricing"], "$label.alias_pricing")
        val pricingFields = setOf(
            "revision", "length_1_xor", "length_2_xor", "length_3_xor",
            "length_4_xor", "length_5_to_32_xor",
        )
        exact(pricing, pricingFields, "$label.alias_pricing")
        pricingFields.forEach { field ->
            require(uint(pricing[field], "$label.alias_pricing.$field") > BigInteger.ZERO)
        }
        return revision
    }

    private fun sorafsProvider(value: Map<String, Any?>) {
        exact(value, setOf("action"), "SorafsProviderGovernance")
        val action = objectValue(value["action"], "SorafsProviderGovernance.action")
        exact(action, setOf("action", "value"), "SorafsProviderGovernance.action")
        val kind = text(action["action"], "SorafsProviderGovernance.action.action")
        val payload = objectValue(action["value"], "SorafsProviderGovernance.action.value")
        when (kind) {
            "establish" -> {
                exact(payload, setOf("provider_id", "owner"), "provider establish")
                providerId(payload["provider_id"], "provider_id")
                account(payload["owner"], "owner")
            }
            "rebind" -> {
                exact(payload, setOf("provider_id", "expected_owner", "next_owner"), "provider rebind")
                providerId(payload["provider_id"], "provider_id")
                val current = account(payload["expected_owner"], "expected_owner")
                val next = account(payload["next_owner"], "next_owner")
                require(current != next) { "next_owner must differ from expected_owner" }
            }
            "remove" -> {
                exact(payload, setOf("provider_id", "expected_owner"), "provider remove")
                providerId(payload["provider_id"], "provider_id")
                account(payload["expected_owner"], "expected_owner")
            }
            else -> throw IllegalArgumentException("Sorafs provider action is unsupported")
        }
    }

    private fun contractLifecycle(value: Map<String, Any?>) {
        exact(
            value,
            setOf("proposal_operator", "contract_address", "expected_revision", "action"),
            "ContractLifecycleGovernance",
        )
        account(
            value["proposal_operator"],
            "ContractLifecycleGovernance.proposal_operator",
        )
        requireCanonicalV1ContractAddress(
            text(value["contract_address"], "ContractLifecycleGovernance.contract_address"),
        )
        require(
            uint(value["expected_revision"], "ContractLifecycleGovernance.expected_revision") >
                BigInteger.ZERO,
        ) { "contract lifecycle expected_revision must be nonzero" }
        val action = objectValue(value["action"], "ContractLifecycleGovernance.action")
        exact(action, setOf("action", "payload"), "ContractLifecycleGovernance.action")
        val kind = text(action["action"], "ContractLifecycleGovernance.action.action")
        require(kind in ParliamentApiV1.CONTRACT_LIFECYCLE_ACTIONS) {
            "contract lifecycle action is unsupported"
        }
        if (kind == "CancelOwnershipOffer" || kind == "AcceptParliamentOwnership") {
            require(action["payload"] == null) { "$kind payload must be null" }
            return
        }
        val payload = objectValue(action["payload"], "ContractLifecycleGovernance.action.payload")
        when (kind) {
            "Activate" -> {
                exact(
                    payload,
                    setOf("code_hash", "abi_hash", "abi_version", "manifest_provenance"),
                    kind,
                )
                lowerHex32(payload["code_hash"], "$kind.code_hash")
                lowerHex32(payload["abi_hash"], "$kind.abi_hash")
                require(uint(payload["abi_version"], "$kind.abi_version") == BigInteger.ONE) {
                    "$kind.abi_version must equal 1"
                }
                payload["manifest_provenance"]?.let {
                    manifestProvenance(objectValue(it, "$kind.manifest_provenance"), "$kind.manifest_provenance")
                }
            }
            "Deactivate" -> {
                val fields = if (payload.containsKey("reason")) {
                    setOf("expected_code_hash", "reason")
                } else {
                    setOf("expected_code_hash")
                }
                exact(payload, fields, kind)
                lowerHex32(payload["expected_code_hash"], "$kind.expected_code_hash")
                payload["reason"]?.let { string(it, "$kind.reason") }
            }
            "OfferOwnership" -> {
                exact(payload, setOf("new_owner"), kind)
                account(payload["new_owner"], "$kind.new_owner")
            }
            "CompleteEmergencyHoldRetrospective" -> {
                exact(
                    payload,
                    setOf(
                        "hold_proposal_content_id", "hold_governance_attempt_id",
                        "incident_digest", "retrospective_finding_root",
                    ),
                    kind,
                )
                bytes(payload["hold_proposal_content_id"], 32, "$kind.hold_proposal_content_id", true)
                bytes(payload["hold_governance_attempt_id"], 32, "$kind.hold_governance_attempt_id", true)
                bytes(payload["incident_digest"], 32, "$kind.incident_digest", true)
                bytes(payload["retrospective_finding_root"], 32, "$kind.retrospective_finding_root", true)
            }
            else -> throw IllegalArgumentException("contract lifecycle action is unsupported")
        }
    }

    private fun globalDataTriggerPermission(value: Map<String, Any?>) {
        exact(
            value,
            setOf("authority", "action"),
            "GlobalDataTriggerPermissionGovernance",
        )
        account(
            value["authority"],
            "GlobalDataTriggerPermissionGovernance.authority",
        )
        val action = objectValue(
            value["action"],
            "GlobalDataTriggerPermissionGovernance.action",
        )
        exact(
            action,
            setOf("action", "value"),
            "GlobalDataTriggerPermissionGovernance.action",
        )
        val kind = text(
            action["action"],
            "GlobalDataTriggerPermissionGovernance.action.action",
        )
        require(kind == "grant" || kind == "revoke") {
            "GlobalDataTriggerPermissionGovernance.action.action must be grant or revoke"
        }
        require(action["value"] == null) {
            "GlobalDataTriggerPermissionGovernance.action.value must be null"
        }
    }

    private fun contractEmergencyHold(value: Map<String, Any?>) {
        exact(
            value,
            setOf(
                "contract_address", "expected_revision", "expected_code_hash",
                "incident_digest", "reason", "duration_blocks",
            ),
            "ContractEmergencyHold",
        )
        requireCanonicalV1ContractAddress(
            text(value["contract_address"], "ContractEmergencyHold.contract_address"),
        )
        require(
            uint(value["expected_revision"], "ContractEmergencyHold.expected_revision") >
                BigInteger.ZERO,
        ) { "contract emergency-hold expected_revision must be nonzero" }
        lowerHex32(value["expected_code_hash"], "ContractEmergencyHold.expected_code_hash")
        bytes(value["incident_digest"], 32, "ContractEmergencyHold.incident_digest", true)
        require(string(value["reason"], "ContractEmergencyHold.reason").trim().isNotEmpty()) {
            "ContractEmergencyHold.reason must not be blank"
        }
        val duration = uint(value["duration_blocks"], "ContractEmergencyHold.duration_blocks")
        require(duration in BigInteger.ONE..BigInteger.valueOf(3_600)) {
            "ContractEmergencyHold.duration_blocks must be in 1..3600"
        }
    }

    private fun providerId(value: Any?, label: String) {
        val tuple = list(value, label)
        require(tuple.size == 1) { "$label must use the exact ProviderId tuple" }
        bytes(tuple[0], 32, "$label[0]", true)
    }

    private fun account(value: Any?, label: String): String =
        requireCanonicalI105Address(text(value, label), label)

    private fun asset(value: Any?, label: String): String {
        val literal = text(value, label)
        require(AssetDefinitionIdEncoder.isCanonicalAddress(literal)) { "$label must be a canonical AssetDefinitionId" }
        return literal
    }

    private fun quantity(value: Any?, label: String): String {
        val literal = string(value, label)
        require(Regex("(?:0|[1-9][0-9]*)(?:\\.[0-9]*[1-9])?").matches(literal)) {
            "$label must be a canonical non-negative quantity"
        }
        return literal
    }

    private fun canonicalBase64(value: Any?, label: String) {
        val literal = string(value, label)
        val decoded = try {
            Base64.getDecoder().decode(literal)
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("$label must be canonical padded base64", ex)
        }
        require(Base64.getEncoder().encodeToString(decoded) == literal) {
            "$label must be canonical padded base64"
        }
    }

    private fun canonicalPublicKey(value: Any?, label: String): PublicKeyPayload {
        val literal = text(value, label)
        val parsed = requireNotNull(decodePublicKeyLiteral(literal)) {
            "$label must be a canonical public-key multihash"
        }
        require(encodePublicKeyMultihash(parsed.curveId, parsed.keyBytes) == literal) {
            "$label must use the canonical bare public-key spelling"
        }
        return parsed
    }

    private fun canonicalSignature(value: Any?, label: String) {
        val literal = string(value, label)
        require(literal.length % 2 == 0 && Regex("[0-9A-F]+").matches(literal)) {
            "$label must be nonempty canonical uppercase hexadecimal"
        }
        require(literal.chunked(2).any { it != "00" }) { "$label must be nonzero" }
    }

    private fun stringTuple(value: Any?, label: String): String {
        val tuple = list(value, label)
        require(tuple.size == 1 && tuple[0] is String) { "$label must use an exact one-string tuple" }
        return tuple[0] as String
    }

    private fun kebab(value: String, label: String, maximumBytes: Int) {
        require(value.toByteArray(StandardCharsets.UTF_8).size <= maximumBytes && KEBAB.matches(value)) {
            "$label must be canonical lowercase ASCII kebab text"
        }
    }

    private fun canonicalName(value: Any?, label: String) {
        val literal = text(value, label)
        require(
            literal.toByteArray(StandardCharsets.UTF_8).size <= 255 &&
                Normalizer.normalize(literal, Normalizer.Form.NFC) == literal &&
                literal.none { it.isWhitespace() || it in setOf('@', '#', '$') || Character.isISOControl(it) } &&
                literal.none { it.code == 0x061c || it.code in 0x200e..0x200f || it.code in 0x202a..0x202e || it.code in 0x2066..0x2069 },
        ) { "$label must be a canonical Iroha Name" }
    }

    private fun reason(value: String, label: String) {
        require(
            value.isNotEmpty() && value == value.trim() &&
                value.toByteArray(StandardCharsets.UTF_8).size <= 1024 &&
                value.none { it.code in 0..0x1f || it.code == 0x7f },
        ) { "$label must be bounded canonical public text" }
    }

    private fun lowerHex32(value: Any?, label: String) {
        require(value is String && Regex("[0-9a-f]{64}").matches(value)) {
            "$label must contain 32 lowercase hexadecimal bytes"
        }
    }

    private fun bytes(value: Any?, size: Int, label: String, nonzero: Boolean): ByteArray {
        val items = list(value, label)
        require(items.size == size) { "$label must contain exactly $size bytes" }
        val result = ByteArray(size)
        items.forEachIndexed { index, item ->
            val parsed = uint(item, "$label[$index]")
            require(parsed <= BigInteger.valueOf(255)) { "$label[$index] must be a byte" }
            result[index] = parsed.toByte()
        }
        require(!nonzero || result.any { it.toInt() != 0 }) { "$label must be nonzero" }
        return result
    }

    private fun u64String(value: Any?, label: String, positive: Boolean): BigInteger {
        val literal = string(value, label)
        require(Regex("0|[1-9][0-9]*").matches(literal)) { "$label must be a canonical u64 decimal string" }
        val parsed = BigInteger(literal)
        require(parsed <= U64_MAX && (!positive || parsed > BigInteger.ZERO)) { "$label is outside u64" }
        return parsed
    }

    private fun uint(
        value: Any?,
        label: String,
    ): BigInteger {
        // JsonParser uses Long/BigInteger only for integer tokens. BigDecimal can
        // render an exponent spelling such as 1e0 as "1", losing the wire distinction.
        require(value is Long || value is BigInteger) { "$label must be an unsigned JSON integer" }
        val literal = value.toString()
        require(Regex("0|[1-9][0-9]*").matches(literal)) { "$label must be an unsigned JSON integer" }
        val parsed = BigInteger(literal)
        require(parsed <= FIRST_RELEASE_MAX_EXACT_JSON_U64) {
            "$label exceeds the first-release exact JSON integer bound"
        }
        return parsed
    }

    private fun text(value: Any?, label: String): String {
        val literal = string(value, label)
        require(literal.isNotEmpty() && literal == literal.trim()) { "$label must be canonical nonempty text" }
        return literal
    }

    private fun string(value: Any?, label: String): String =
        value as? String ?: throw IllegalArgumentException("$label must be a string")

    private fun list(value: Any?, label: String): List<*> =
        value as? List<*> ?: throw IllegalArgumentException("$label must be an array")

    @Suppress("UNCHECKED_CAST")
    private fun objectValue(value: Any?, label: String): Map<String, Any?> {
        require(value is Map<*, *> && value.keys.all { it is String }) { "$label must be an object" }
        return value as Map<String, Any?>
    }

    private fun exact(value: Map<String, Any?>, fields: Set<String>, label: String) {
        require(value.keys == fields) { "$label contains unknown, aliased, or missing fields" }
    }
}
