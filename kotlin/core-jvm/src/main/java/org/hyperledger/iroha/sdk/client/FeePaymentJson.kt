package org.hyperledger.iroha.sdk.client

import java.math.BigDecimal
import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets
import java.util.TreeMap
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.FeeChargeKind
import org.hyperledger.iroha.sdk.core.model.FeeChargeLimit
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.FeeSponsorProgramId
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity

internal fun sameFeeQuoteAccountIdentity(left: String, right: String): Boolean = try {
    val leftBytes = AccountAddress.parseEncodedIgnoringCurveSupport(left, null).canonicalBytes
    val rightBytes = AccountAddress.parseEncodedIgnoringCurveSupport(right, null).canonicalBytes
    leftBytes.contentEquals(rightBytes)
} catch (error: AccountAddressException) {
    throw IllegalArgumentException("fee quote account identity must use canonical I105", error)
}

private fun sameFeeQuoteProgramIdentity(
    left: FeeSponsorProgramId,
    right: FeeSponsorProgramId,
): Boolean =
    left.name == right.name && sameFeeQuoteAccountIdentity(left.sponsor, right.sponsor)

private fun sameFeeQuotePayerAndGasBound(
    left: FeePaymentIntent,
    right: FeePaymentIntent,
): Boolean {
    if (left.gasLimit != right.gasLimit) return false
    return when {
        left is FeePaymentIntent.Authority && right is FeePaymentIntent.Authority -> true
        left is FeePaymentIntent.Sponsor && right is FeePaymentIntent.Sponsor ->
            left.programRevision == right.programRevision &&
                sameFeeQuoteProgramIdentity(left.programId, right.programId)
        else -> false
    }
}

private fun sameExactFeeQuoteIntent(
    left: FeePaymentIntent,
    right: FeePaymentIntent,
): Boolean =
    sameFeeQuotePayerAndGasBound(left, right) && left.chargeLimits == right.chargeLimits

internal object FeePaymentJson {
    fun parseProgram(payload: ByteArray): FeeSponsorProgramResponse {
        val path = "fee sponsor program response"
        val root = objectValue(
            JsonParser.parse(strictUtf8(payload, path)),
            path,
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
        val scheduledActivation = if (root.containsKey("scheduled_activation")) {
            val value = root["scheduled_activation"]
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
                positiveLong(
                    activation["activate_at_height"],
                    "fee sponsor program response.scheduled_activation.activate_at_height",
                ),
            )
        } else {
            null
        }
        val activeRevision = if (root.containsKey("active_revision")) {
            positiveLong(root["active_revision"], "fee sponsor program response.active_revision")
        } else {
            null
        }
        val stagedRevision = if (root.containsKey("staged_revision")) {
            positiveLong(root["staged_revision"], "fee sponsor program response.staged_revision")
        } else {
            null
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
            activeRevision,
            stagedRevision,
            scheduledActivation,
        )
    }

    fun parseQuote(payload: ByteArray): FeeQuoteResponse {
        val path = "fee quote response"
        val root = objectValue(
            JsonParser.parse(strictUtf8(payload, path)),
            path,
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

    private fun strictUtf8(payload: ByteArray, path: String): String = try {
        StandardCharsets.UTF_8.newDecoder()
            .onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT)
            .decode(ByteBuffer.wrap(payload))
            .toString()
    } catch (error: CharacterCodingException) {
        throw IllegalArgumentException("$path must be valid UTF-8", error)
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
        requireExactKeys(body, allowed, "$path.value")
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
    private val parsedObservation = parseObservation(observationSnapshot)
    private val parsedComponents = componentSnapshot.mapIndexed(::parseComponent)
    private val parsedCapacities = capacitySnapshot.mapIndexed(::parseCapacity)
    private val parsedDecision = parseDecision(decisionSnapshot)

    val observation: Map<String, Any?> get() = observationSnapshot.toMap()
    val components: List<Map<String, Any?>> get() = componentSnapshot.map { it.toMap() }
    val capacities: List<Map<String, Any?>> get() = capacitySnapshot.map { it.toMap() }
    val decision: Map<String, Any?> get() = decisionSnapshot.toMap()

    /** Validate this quote against the unsigned transaction payload used to request it. */
    fun validateForDraft(payload: TransactionPayload) {
        validateForDraft(payload.feePayment, payload.authority)
    }

    /** Validate this quote against a payload containing the exact quoted fee intent. */
    fun validateForSignedPayload(payload: TransactionPayload) {
        require(sameExactFeeQuoteIntent(intent, payload.feePayment)) {
            "fee quote intent differs from the signed payload"
        }
        validateSemantics(payload.authority)
    }

    internal fun validateForDraft(
        draftIntent: FeePaymentIntent,
        authority: String,
    ) {
        require(sameFeeQuotePayerAndGasBound(intent, draftIntent)) {
            "fee quote changed the draft payer, sponsor revision, or gas bound"
        }
        validateSemantics(authority)
    }

    private fun validateSemantics(authority: String) {
        require(parsedObservation.nextBlockHeight.signum() > 0) {
            "fee quote next_block_height must be non-zero"
        }
        require(
            parsedComponents.size == intent.chargeLimits.size &&
                parsedComponents.zip(intent.chargeLimits).all { (component, limit) ->
                    component.kind == limit.kind &&
                        component.assetDefinitionId == limit.assetDefinitionId &&
                        component.maxAmount.toString() == limit.maxAmount
                },
        ) {
            "fee quote components differ from the quoted intent"
        }

        when (val payment = intent) {
            is FeePaymentIntent.Authority -> {
                val source = parsedDecision.debitSource
                require(
                    source is ParsedDebitSource.Account &&
                        sameFeeQuoteAccountIdentity(source.accountId, authority) &&
                        parsedDecision.programRevision == null,
                ) {
                    "authority-paid fee quote has an inconsistent admission decision"
                }
                require(parsedCapacities.isEmpty()) {
                    "authority-paid fee quote must not contain capacities"
                }
            }
            is FeePaymentIntent.Sponsor -> {
                val source = parsedDecision.debitSource
                require(
                    source is ParsedDebitSource.SponsorProgram &&
                        sameFeeQuoteProgramIdentity(source.programId, payment.programId) &&
                        parsedDecision.programRevision == BigInteger.valueOf(payment.programRevision),
                ) {
                    "sponsored fee quote has an inconsistent admission decision"
                }
                validateSponsorCapacities()
            }
        }
    }

    private fun validateSponsorCapacities() {
        require(parsedCapacities.isEmpty() == parsedComponents.isEmpty()) {
            "sponsored fee quote capacities must be empty exactly when components are empty"
        }
        val aggregateByAsset = TreeMap<String, KotodamaQuantity>(::compareAssetIds)
        parsedComponents.forEach { component ->
            val previous = aggregateByAsset[component.assetDefinitionId]
            aggregateByAsset[component.assetDefinitionId] = if (previous == null) {
                component.maxAmount
            } else {
                addQuantities(
                    previous,
                    component.maxAmount,
                    "fee quote component aggregate for ${component.assetDefinitionId} is invalid",
                )
            }
        }
        require(parsedCapacities.size == aggregateByAsset.size) {
            "sponsored fee quote must contain one capacity per component asset"
        }
        parsedCapacities.zip(aggregateByAsset.entries).forEach { (capacity, entry) ->
            require(capacity.assetDefinitionId == entry.key) {
                "sponsored fee quote capacities are duplicated, unrelated, or not in canonical asset order"
            }
            val requiredVaultBalance = addQuantities(
                capacity.reserveFloor,
                entry.value,
                "fee quote required vault balance for ${entry.key} is invalid",
            )
            require(compareQuantities(capacity.vaultBalance, requiredVaultBalance) >= 0) {
                "fee quote vault capacity for ${entry.key} does not cover its reserve and aggregate charge"
            }
            listOf(
                "block" to capacity.blockRemaining,
                "program epoch" to capacity.programEpochRemaining,
                "beneficiary epoch" to capacity.beneficiaryEpochRemaining,
            ).forEach { (window, remaining) ->
                require(compareQuantities(remaining, entry.value) >= 0) {
                    "fee quote $window capacity for ${entry.key} does not cover its aggregate charge"
                }
            }
        }
    }

    private data class ParsedObservation(
        val ledgerTimeMs: BigInteger,
        val nextBlockHeight: BigInteger,
        val routeDataspaceId: BigInteger,
    )

    private data class ParsedComponent(
        val kind: FeeChargeKind,
        val assetDefinitionId: String,
        val maxAmount: KotodamaQuantity,
    )

    private data class ParsedCapacity(
        val assetDefinitionId: String,
        val vaultBalance: KotodamaQuantity,
        val reserveFloor: KotodamaQuantity,
        val blockRemaining: KotodamaQuantity,
        val programEpochRemaining: KotodamaQuantity,
        val beneficiaryEpochRemaining: KotodamaQuantity,
    )

    private sealed class ParsedDebitSource {
        data class Account(val accountId: String) : ParsedDebitSource()
        data class SponsorProgram(val programId: FeeSponsorProgramId) : ParsedDebitSource()
    }

    private data class ParsedDecision(
        val debitSource: ParsedDebitSource,
        val programRevision: BigInteger?,
    )

    private companion object {
        private val U64_MAX = BigInteger("18446744073709551615")

        private fun parseObservation(value: Map<String, Any?>): ParsedObservation {
            requireExactKeys(
                value,
                setOf("ledger_time_ms", "next_block_height", "route_dataspace_id"),
                "fee quote response.observation",
            )
            return ParsedObservation(
                unsignedInteger(value["ledger_time_ms"], "fee quote response.observation.ledger_time_ms"),
                unsignedInteger(value["next_block_height"], "fee quote response.observation.next_block_height"),
                unsignedInteger(value["route_dataspace_id"], "fee quote response.observation.route_dataspace_id"),
            )
        }

        private fun parseComponent(index: Int, value: Map<String, Any?>): ParsedComponent {
            val path = "fee quote response.components[$index]"
            requireExactKeys(value, setOf("kind", "asset_definition_id", "max_amount"), path)
            val kindValue = objectValue(value["kind"], "$path.kind")
            requireExactKeys(kindValue, setOf("kind", "value"), "$path.kind")
            require(kindValue["value"] == null) { "$path.kind.value must be null" }
            val kind = when (kindValue["kind"]) {
                "nexus" -> FeeChargeKind.NEXUS
                "pipeline_gas" -> FeeChargeKind.PIPELINE_GAS
                else -> throw IllegalArgumentException("$path.kind.kind must be nexus or pipeline_gas")
            }
            val assetDefinitionId = canonicalAssetId(value["asset_definition_id"], "$path.asset_definition_id")
            return ParsedComponent(
                kind,
                assetDefinitionId,
                quantity(value["max_amount"], "$path.max_amount"),
            )
        }

        private fun parseCapacity(index: Int, value: Map<String, Any?>): ParsedCapacity {
            val path = "fee quote response.capacities[$index]"
            requireExactKeys(
                value,
                setOf(
                    "asset_definition_id",
                    "vault_balance",
                    "reserve_floor",
                    "block_remaining",
                    "program_epoch_remaining",
                    "beneficiary_epoch_remaining",
                ),
                path,
            )
            return ParsedCapacity(
                canonicalAssetId(value["asset_definition_id"], "$path.asset_definition_id"),
                quantity(value["vault_balance"], "$path.vault_balance"),
                quantity(value["reserve_floor"], "$path.reserve_floor"),
                quantity(value["block_remaining"], "$path.block_remaining"),
                quantity(value["program_epoch_remaining"], "$path.program_epoch_remaining"),
                quantity(value["beneficiary_epoch_remaining"], "$path.beneficiary_epoch_remaining"),
            )
        }

        private fun parseDecision(value: Map<String, Any?>): ParsedDecision {
            val path = "fee quote response.decision"
            requireExactKeys(value, setOf("status", "value"), path)
            require(value["status"] == "accepted") { "$path.status must be accepted" }
            val accepted = objectValue(value["value"], "$path.value")
            requireExactKeys(accepted, setOf("debit_source", "program_revision"), "$path.value")
            val debitSource = objectValue(accepted["debit_source"], "$path.value.debit_source")
            requireExactKeys(debitSource, setOf("kind", "value"), "$path.value.debit_source")
            return when (debitSource["kind"]) {
                "account" -> {
                    val accountId = debitSource["value"] as? String
                        ?: throw IllegalArgumentException("$path.value.debit_source.value must be a string")
                    requireCanonicalI105Address(accountId, "$path.value.debit_source.value")
                    require(accepted["program_revision"] == null) {
                        "$path.value.program_revision must be null for an account debit"
                    }
                    ParsedDecision(ParsedDebitSource.Account(accountId), null)
                }
                "sponsor_program" -> {
                    val program = objectValue(debitSource["value"], "$path.value.debit_source.value")
                    requireExactKeys(program, setOf("sponsor", "name"), "$path.value.debit_source.value")
                    val sponsor = program["sponsor"] as? String
                        ?: throw IllegalArgumentException("$path.value.debit_source.value.sponsor must be a string")
                    val name = program["name"] as? String
                        ?: throw IllegalArgumentException("$path.value.debit_source.value.name must be a string")
                    ParsedDecision(
                        ParsedDebitSource.SponsorProgram(FeeSponsorProgramId(sponsor, name)),
                        positiveInteger(accepted["program_revision"], "$path.value.program_revision"),
                    )
                }
                else -> throw IllegalArgumentException(
                    "$path.value.debit_source.kind must be account or sponsor_program",
                )
            }
        }

        @Suppress("UNCHECKED_CAST")
        private fun objectValue(value: Any?, path: String): Map<String, Any?> {
            val map = value as? Map<*, *> ?: throw IllegalArgumentException("$path must be an object")
            require(map.keys.all { it is String }) { "$path keys must be strings" }
            return map as Map<String, Any?>
        }

        private fun requireExactKeys(value: Map<String, Any?>, expected: Set<String>, path: String) {
            val unknown = value.keys - expected
            require(unknown.isEmpty()) { "$path contains unknown fields: ${unknown.sorted()}" }
            val missing = expected - value.keys
            require(missing.isEmpty()) { "$path is missing required fields: ${missing.sorted()}" }
        }

        private fun canonicalAssetId(value: Any?, path: String): String {
            val literal = value as? String ?: throw IllegalArgumentException("$path must be a string")
            require(AssetDefinitionIdEncoder.isCanonicalAddress(literal)) {
                "$path must be a canonical asset definition id"
            }
            return literal
        }

        private fun quantity(value: Any?, path: String): KotodamaQuantity {
            val literal = value as? String ?: throw IllegalArgumentException("$path must be a string")
            return try {
                KotodamaQuantity.parseCanonical(literal)
            } catch (error: IllegalArgumentException) {
                throw IllegalArgumentException("$path must be a canonical quantity", error)
            }
        }

        private fun unsignedInteger(value: Any?, path: String): BigInteger {
            val integer = exactInteger(value, path)
            require(integer.signum() >= 0 && integer <= U64_MAX) { "$path must fit in u64" }
            return integer
        }

        private fun positiveInteger(value: Any?, path: String): BigInteger =
            unsignedInteger(value, path).also { require(it.signum() > 0) { "$path must be positive" } }

        private fun exactInteger(value: Any?, path: String): BigInteger = when (value) {
            is BigInteger -> value
            is BigDecimal -> try {
                value.toBigIntegerExact()
            } catch (error: ArithmeticException) {
                throw IllegalArgumentException("$path must be an integer", error)
            }
            is Byte, is Short, is Int, is Long -> BigInteger.valueOf((value as Number).toLong())
            else -> throw IllegalArgumentException("$path must be an integer")
        }

        private fun addQuantities(
            left: KotodamaQuantity,
            right: KotodamaQuantity,
            path: String,
        ): KotodamaQuantity {
            val scale = maxOf(left.scale, right.scale)
            val sum = left.mantissa.multiply(BigInteger.TEN.pow(scale - left.scale))
                .add(right.mantissa.multiply(BigInteger.TEN.pow(scale - right.scale)))
            return try {
                KotodamaQuantity.of(sum, scale)
            } catch (error: IllegalArgumentException) {
                throw IllegalArgumentException(path, error)
            }
        }

        private fun compareQuantities(left: KotodamaQuantity, right: KotodamaQuantity): Int {
            val scale = maxOf(left.scale, right.scale)
            val leftMantissa = left.mantissa.multiply(BigInteger.TEN.pow(scale - left.scale))
            val rightMantissa = right.mantissa.multiply(BigInteger.TEN.pow(scale - right.scale))
            return leftMantissa.compareTo(rightMantissa)
        }

        private fun compareAssetIds(left: String, right: String): Int {
            val leftBytes = AssetDefinitionIdEncoder.parseAddressBytes(left)
            val rightBytes = AssetDefinitionIdEncoder.parseAddressBytes(right)
            for (index in leftBytes.indices) {
                val comparison = (leftBytes[index].toInt() and 0xff)
                    .compareTo(rightBytes[index].toInt() and 0xff)
                if (comparison != 0) return comparison
            }
            return 0
        }
    }
}
