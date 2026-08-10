package org.hyperledger.iroha.sdk.client

import java.math.BigDecimal
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.core.model.FeeChargeKind
import org.hyperledger.iroha.sdk.core.model.FeeChargeLimit
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.FeeSponsorProgramId

internal object FeePaymentJson {
    fun parseProgram(payload: ByteArray): FeeSponsorProgramResponse {
        val root = objectValue(
            JsonParser.parse(String(payload, StandardCharsets.UTF_8)),
            "fee sponsor program response",
        )
        requireExactKeys(
            root,
            setOf("id", "payout_account", "lifecycle", "active_revision", "staged_revision", "scheduled_activation"),
            "fee sponsor program response",
            required = setOf("id", "payout_account", "lifecycle"),
        )
        val id = objectValue(root["id"], "fee sponsor program response.id")
        requireExactKeys(id, setOf("sponsor", "name"), "fee sponsor program response.id")
        val lifecycle = objectValue(root["lifecycle"], "fee sponsor program response.lifecycle")
        requireExactKeys(lifecycle, setOf("state", "value"), "fee sponsor program response.lifecycle")
        require(lifecycle["value"] == null) {
            "fee sponsor program response.lifecycle.value must be null"
        }
        val scheduledActivation = root["scheduled_activation"]?.let { value ->
            val activation = objectValue(value, "fee sponsor program response.scheduled_activation")
            requireExactKeys(
                activation,
                setOf("revision", "activate_at_height"),
                "fee sponsor program response.scheduled_activation",
            )
            FeeSponsorProgramActivation(
                positiveLong(
                    activation["revision"],
                    "fee sponsor program response.scheduled_activation.revision",
                ),
                nonNegativeLong(
                    activation["activate_at_height"],
                    "fee sponsor program response.scheduled_activation.activate_at_height",
                ),
            )
        }
        return FeeSponsorProgramResponse(
            FeeSponsorProgramId(
                id["sponsor"] as? String
                    ?: throw IllegalArgumentException("fee sponsor program response.id.sponsor must be a string"),
                id["name"] as? String
                    ?: throw IllegalArgumentException("fee sponsor program response.id.name must be a string"),
            ),
            root["payout_account"] as? String
                ?: throw IllegalArgumentException("fee sponsor program response.payout_account must be a string"),
            when (lifecycle["state"]) {
                "staged" -> FeeSponsorProgramLifecycle.STAGED
                "paused" -> FeeSponsorProgramLifecycle.PAUSED
                "active" -> FeeSponsorProgramLifecycle.ACTIVE
                "closing" -> FeeSponsorProgramLifecycle.CLOSING
                "closed" -> FeeSponsorProgramLifecycle.CLOSED
                else -> throw IllegalArgumentException(
                    "fee sponsor program response.lifecycle.state is unsupported",
                )
            },
            root["active_revision"]?.let {
                positiveLong(it, "fee sponsor program response.active_revision")
            },
            root["staged_revision"]?.let {
                positiveLong(it, "fee sponsor program response.staged_revision")
            },
            scheduledActivation,
        )
    }

    fun parseQuote(payload: ByteArray): FeeQuoteResponse {
        val root = objectValue(
            JsonParser.parse(String(payload, StandardCharsets.UTF_8)),
            "fee quote response",
        )
        requireExactKeys(
            root,
            setOf("intent", "observation", "components", "capacities", "decision"),
            "fee quote response",
        )
        return FeeQuoteResponse(
            parse(root["intent"], "fee quote response.intent"),
            objectValue(root["observation"], "fee quote response.observation"),
            objectList(root["components"], "fee quote response.components"),
            objectList(root["capacities"], "fee quote response.capacities"),
            objectValue(root["decision"], "fee quote response.decision"),
        )
    }

    fun parse(value: Any?, path: String): FeePaymentIntent {
        val root = objectValue(value, path)
        requireExactKeys(root, setOf("payer", "value"), path)
        val payer = root["payer"] as? String
            ?: throw IllegalArgumentException("$path.payer must be a string")
        val body = objectValue(root["value"], "$path.value")
        val commonKeys = setOf("charge_limits", "gas_limit")
        val allowed = if (payer == "sponsor") {
            commonKeys + setOf("program_id", "program_revision")
        } else {
            commonKeys
        }
        requireExactKeys(body, allowed, "$path.value", required = setOf("charge_limits"))
        val rawLimits = body["charge_limits"] as? List<*>
            ?: throw IllegalArgumentException("$path.value.charge_limits must be an array")
        val limits = rawLimits.mapIndexed { index, raw -> parseLimit(raw, "$path.value.charge_limits[$index]") }
        val gasLimit = optionalPositiveLong(body["gas_limit"], "$path.value.gas_limit")
        return when (payer) {
            "authority" -> FeePaymentIntent.Authority(limits, gasLimit)
            "sponsor" -> {
                val program = objectValue(body["program_id"], "$path.value.program_id")
                requireExactKeys(program, setOf("sponsor", "name"), "$path.value.program_id")
                val sponsor = program["sponsor"] as? String
                    ?: throw IllegalArgumentException("$path.value.program_id.sponsor must be a string")
                val name = program["name"] as? String
                    ?: throw IllegalArgumentException("$path.value.program_id.name must be a string")
                FeePaymentIntent.Sponsor(
                    FeeSponsorProgramId(sponsor, name),
                    positiveLong(body["program_revision"], "$path.value.program_revision"),
                    limits,
                    gasLimit,
                )
            }
            else -> throw IllegalArgumentException("$path.payer must be authority or sponsor")
        }
    }

    private fun parseLimit(value: Any?, path: String): FeeChargeLimit {
        val item = objectValue(value, path)
        requireExactKeys(item, setOf("kind", "asset_definition_id", "max_amount"), path)
        val kindObject = objectValue(item["kind"], "$path.kind")
        requireExactKeys(kindObject, setOf("kind", "value"), "$path.kind")
        require(kindObject["value"] == null) { "$path.kind.value must be null" }
        val kind = when (kindObject["kind"]) {
            "nexus" -> FeeChargeKind.NEXUS
            "pipeline_gas" -> FeeChargeKind.PIPELINE_GAS
            else -> throw IllegalArgumentException("$path.kind.kind must be nexus or pipeline_gas")
        }
        return FeeChargeLimit(
            kind,
            item["asset_definition_id"] as? String
                ?: throw IllegalArgumentException("$path.asset_definition_id must be a string"),
            item["max_amount"] as? String
                ?: throw IllegalArgumentException("$path.max_amount must be a string"),
        )
    }

    @Suppress("UNCHECKED_CAST")
    private fun objectValue(value: Any?, path: String): Map<String, Any?> {
        val map = value as? Map<*, *> ?: throw IllegalArgumentException("$path must be an object")
        require(map.keys.all { it is String }) { "$path keys must be strings" }
        return map as Map<String, Any?>
    }

    private fun objectList(value: Any?, path: String): List<Map<String, Any?>> {
        val list = value as? List<*> ?: throw IllegalArgumentException("$path must be an array")
        return list.mapIndexed { index, item -> objectValue(item, "$path[$index]") }
    }

    private fun requireExactKeys(
        value: Map<String, Any?>,
        allowed: Set<String>,
        path: String,
        required: Set<String> = allowed,
    ) {
        val unknown = value.keys - allowed
        require(unknown.isEmpty()) { "$path contains unknown fields: ${unknown.sorted()}" }
        val missing = required - value.keys
        require(missing.isEmpty()) { "$path is missing required fields: ${missing.sorted()}" }
    }

    private fun optionalPositiveLong(value: Any?, path: String): Long? =
        if (value == null) null else positiveLong(value, path)

    private fun positiveLong(value: Any?, path: String): Long {
        val number = when (value) {
            is Byte, is Short, is Int, is Long -> (value as Number).toLong()
            is BigDecimal -> try {
                value.longValueExact()
            } catch (ex: ArithmeticException) {
                throw IllegalArgumentException("$path must be an integer", ex)
            }
            else -> throw IllegalArgumentException("$path must be an integer")
        }
        require(number > 0) { "$path must be positive" }
        return number
    }

    private fun nonNegativeLong(value: Any?, path: String): Long {
        val number = when (value) {
            is Byte, is Short, is Int, is Long -> (value as Number).toLong()
            is BigDecimal -> try {
                value.longValueExact()
            } catch (ex: ArithmeticException) {
                throw IllegalArgumentException("$path must be an integer", ex)
            }
            else -> throw IllegalArgumentException("$path must be an integer")
        }
        require(number >= 0) { "$path must be non-negative" }
        return number
    }
}

/** Successful deterministic fee quote preserving the exact payer and gas bound. */
class FeeQuoteResponse(
    @JvmField val intent: FeePaymentIntent,
    observation: Map<String, Any?>,
    components: List<Map<String, Any?>>,
    capacities: List<Map<String, Any?>>,
    decision: Map<String, Any?>,
) {
    private val observationSnapshot = observation.toMap()
    private val componentSnapshot = components.map { it.toMap() }
    private val capacitySnapshot = capacities.map { it.toMap() }
    private val decisionSnapshot = decision.toMap()

    val observation: Map<String, Any?> get() = observationSnapshot.toMap()
    val components: List<Map<String, Any?>> get() = componentSnapshot.map { it.toMap() }
    val capacities: List<Map<String, Any?>> get() = capacitySnapshot.map { it.toMap() }
    val decision: Map<String, Any?> get() = decisionSnapshot.toMap()
}
