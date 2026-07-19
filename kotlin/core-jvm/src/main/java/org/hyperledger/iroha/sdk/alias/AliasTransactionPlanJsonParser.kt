package org.hyperledger.iroha.sdk.alias

import java.math.BigDecimal
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.client.JsonParser

/** Strict parser for the typed response returned by `POST /v1/aliases/setup/plan`. */
object AliasTransactionPlanJsonParser {
    /** Parses one complete alias transaction plan without accepting lossy numeric coercions. */
    @JvmStatic
    fun parse(payload: ByteArray): AliasTransactionPlanV1 {
        val root = objectValue(
            JsonParser.parse(String(payload, StandardCharsets.UTF_8)),
            "alias transaction plan",
        )
        exactKeys(root, setOf("body", "plan_hash"), "alias transaction plan")
        return AliasTransactionPlanV1(
            parseBody(objectField(root, "body", "alias transaction plan.body")),
            stringField(root, "plan_hash", "alias transaction plan.plan_hash"),
        )
    }

    private fun parseBody(root: Map<String, Any?>): AliasTransactionPlanBodyV1 {
        exactKeys(
            root,
            setOf(
                "version", "authority", "chain_id", "anchor", "resources", "instructions",
                "totals_by_asset", "warnings", "blockers", "valid_until_ms",
            ),
            "alias transaction plan.body",
        )
        return AliasTransactionPlanBodyV1(
            intField(root, "version", "body.version"),
            stringField(root, "authority", "body.authority"),
            stringField(root, "chain_id", "body.chain_id"),
            parseAnchor(objectField(root, "anchor", "body.anchor")),
            arrayField(root, "resources", "body.resources").mapIndexed { index, value ->
                parseResource(objectValue(value, "body.resources[$index]"), "body.resources[$index]")
            },
            arrayField(root, "instructions", "body.instructions").mapIndexed { index, value ->
                parseFrame(objectValue(value, "body.instructions[$index]"), "body.instructions[$index]")
            },
            arrayField(root, "totals_by_asset", "body.totals_by_asset").mapIndexed { index, value ->
                parseTotal(objectValue(value, "body.totals_by_asset[$index]"), "body.totals_by_asset[$index]")
            },
            arrayField(root, "warnings", "body.warnings").mapIndexed { index, value ->
                parseDiagnostic(objectValue(value, "body.warnings[$index]"), "body.warnings[$index]")
            },
            arrayField(root, "blockers", "body.blockers").mapIndexed { index, value ->
                parseDiagnostic(objectValue(value, "body.blockers[$index]"), "body.blockers[$index]")
            },
            longField(root, "valid_until_ms", "body.valid_until_ms"),
        )
    }

    internal fun parseAnchor(root: Map<String, Any?>): AliasPlanAnchorV1 {
        exactKeys(root, setOf("block_height", "block_hash"), "body.anchor")
        return AliasPlanAnchorV1(
            longField(root, "block_height", "body.anchor.block_height"),
            stringField(root, "block_hash", "body.anchor.block_hash"),
        )
    }

    internal fun parseResource(root: Map<String, Any?>, path: String): AliasPlanResourceV1 {
        exactKeys(root, setOf("intent", "disposition", "quote", "instruction_index"), path)
        return AliasPlanResourceV1(
            parseIntent(objectField(root, "intent", "$path.intent"), "$path.intent"),
            parseDisposition(objectField(root, "disposition", "$path.disposition"), "$path.disposition"),
            optionalObject(root, "quote", "$path.quote")?.let { parseQuote(it, "$path.quote") },
            optionalLong(root, "instruction_index", "$path.instruction_index"),
        )
    }

    internal fun parseFrame(root: Map<String, Any?>, path: String): AliasFramedInstructionV1 {
        exactKeys(root, setOf("wire_id", "framed_payload"), path)
        val bytes = arrayField(root, "framed_payload", "$path.framed_payload")
            .mapIndexed { index, value ->
                val number = exactInteger(value, "$path.framed_payload[$index]")
                check(number in BigInteger.ZERO..BigInteger.valueOf(255)) {
                    "$path.framed_payload[$index] must be an unsigned byte"
                }
                number.toByte()
            }
            .toByteArray()
        return AliasFramedInstructionV1(stringField(root, "wire_id", "$path.wire_id"), bytes)
    }

    internal fun parseTotal(root: Map<String, Any?>, path: String): AliasAssetTotalV1 {
        exactKeys(root, setOf("payment_asset", "amount"), path)
        return AliasAssetTotalV1(
            stringField(root, "payment_asset", "$path.payment_asset"),
            stringField(root, "amount", "$path.amount"),
        )
    }

    internal fun parseQuote(root: Map<String, Any?>, path: String): AliasLeaseQuoteV1 {
        exactKeys(
            root,
            setOf(
                "target", "pricing_class", "exact_amount", "guard", "expires_at_ms",
                "grace_expires_at_ms", "redemption_expires_at_ms",
            ),
            path,
        )
        return AliasLeaseQuoteV1(
            parseTarget(objectField(root, "target", "$path.target"), "$path.target"),
            intField(root, "pricing_class", "$path.pricing_class"),
            stringField(root, "exact_amount", "$path.exact_amount"),
            parseGuard(objectField(root, "guard", "$path.guard"), "$path.guard"),
            longField(root, "expires_at_ms", "$path.expires_at_ms"),
            longField(root, "grace_expires_at_ms", "$path.grace_expires_at_ms"),
            longField(root, "redemption_expires_at_ms", "$path.redemption_expires_at_ms"),
        )
    }

    private fun parseIntent(root: Map<String, Any?>, path: String): AliasIntentV1 {
        exactKeys(root, setOf("kind", "intent"), path)
        val value = objectField(root, "intent", "$path.intent")
        return when (stringField(root, "kind", "$path.kind")) {
            "dataspace" -> {
                exactKeys(value, setOf("dataspace", "owner"), "$path.intent")
                AliasIntentV1.Dataspace(
                    AliasDataSpaceIntentV1(
                        parseDataspace(
                            objectField(value, "dataspace", "$path.intent.dataspace"),
                            "$path.intent.dataspace",
                        ),
                        stringField(value, "owner", "$path.intent.owner"),
                    ),
                )
            }
            "domain" -> {
                exactKeys(value, setOf("domain", "owner"), "$path.intent")
                AliasIntentV1.Domain(
                    AliasDomainIntentV1(
                        parseDomain(objectField(value, "domain", "$path.intent.domain"), "$path.intent.domain"),
                        stringField(value, "owner", "$path.intent.owner"),
                    ),
                )
            }
            "account_alias" -> {
                exactKeys(value, setOf("alias", "target_account", "provision", "role"), "$path.intent")
                AliasIntentV1.AccountAlias(
                    AliasAccountIntentV1(
                        parseAccountAlias(objectField(value, "alias", "$path.intent.alias"), "$path.intent.alias"),
                        stringField(value, "target_account", "$path.intent.target_account"),
                        when (
                            parseUnitVariant(
                                objectField(value, "provision", "$path.intent.provision"),
                                "$path.intent.provision",
                            )
                        ) {
                            "existing" -> AccountProvisionV1.EXISTING
                            "create" -> AccountProvisionV1.CREATE
                            else -> error("$path.intent.provision.kind is unsupported")
                        },
                        when (parseUnitVariant(objectField(value, "role", "$path.intent.role"), "$path.intent.role")) {
                            "primary" -> AccountAliasRoleV1.PRIMARY
                            "additional" -> AccountAliasRoleV1.ADDITIONAL
                            else -> error("$path.intent.role.kind is unsupported")
                        },
                    ),
                )
            }
            else -> error("$path.kind is unsupported")
        }
    }

    internal fun parseTarget(root: Map<String, Any?>, path: String): AliasTargetV1 {
        exactKeys(root, setOf("kind", "resource"), path)
        val resource = objectField(root, "resource", "$path.resource")
        return when (stringField(root, "kind", "$path.kind")) {
            "dataspace" -> AliasTargetV1.Dataspace(parseDataspace(resource, "$path.resource"))
            "domain" -> AliasTargetV1.Domain(parseDomain(resource, "$path.resource"))
            "account_alias" -> AliasTargetV1.AccountAlias(parseAccountAlias(resource, "$path.resource"))
            else -> error("$path.kind is unsupported")
        }
    }

    private fun parseDataspace(root: Map<String, Any?>, path: String): ResolvedDataSpaceV1 {
        exactKeys(root, setOf("canonical_name", "dataspace_id"), path)
        return ResolvedDataSpaceV1(
            stringField(root, "canonical_name", "$path.canonical_name"),
            u64Field(root, "dataspace_id", "$path.dataspace_id"),
        )
    }

    private fun parseDomain(root: Map<String, Any?>, path: String): ResolvedDomainV1 {
        exactKeys(root, setOf("canonical_name", "dataspace_id"), path)
        return ResolvedDomainV1(
            stringField(root, "canonical_name", "$path.canonical_name"),
            u64Field(root, "dataspace_id", "$path.dataspace_id"),
        )
    }

    private fun parseAccountAlias(root: Map<String, Any?>, path: String): ResolvedAccountAliasV1 {
        exactKeys(root, setOf("canonical_name", "dataspace_id"), path)
        val name = objectField(root, "canonical_name", "$path.canonical_name")
        exactKeys(name, setOf("label", "domain", "dataspace"), "$path.canonical_name")
        return ResolvedAccountAliasV1(
            AccountAliasName(
                stringField(name, "label", "$path.canonical_name.label"),
                optionalString(name, "domain", "$path.canonical_name.domain"),
                stringField(name, "dataspace", "$path.canonical_name.dataspace"),
            ),
            u64Field(root, "dataspace_id", "$path.dataspace_id"),
        )
    }

    internal fun parseGuard(root: Map<String, Any?>, path: String): AliasQuoteGuardV1 {
        exactKeys(
            root,
            setOf("expected_policy_version", "expected_payment_asset", "max_amount", "valid_until_ms"),
            path,
        )
        return AliasQuoteGuardV1(
            intField(root, "expected_policy_version", "$path.expected_policy_version"),
            stringField(root, "expected_payment_asset", "$path.expected_payment_asset"),
            stringField(root, "max_amount", "$path.max_amount"),
            longField(root, "valid_until_ms", "$path.valid_until_ms"),
        )
    }

    private fun parseDisposition(root: Map<String, Any?>, path: String): AliasPlanDispositionV1 =
        when (parseUnitVariant(root, path)) {
            "no_op" -> AliasPlanDispositionV1.NO_OP
            "repair" -> AliasPlanDispositionV1.REPAIR
            "create" -> AliasPlanDispositionV1.CREATE
            "conflict" -> AliasPlanDispositionV1.CONFLICT
            else -> error("$path.kind is unsupported")
        }

    internal fun parseDiagnostic(root: Map<String, Any?>, path: String): AliasSetupDiagnosticV1 {
        exactKeys(
            root,
            setOf("phase", "code", "severity", "resource", "config_path", "expected", "actual", "remediation"),
            path,
        )
        val phase = when (parseTaggedVariant(objectField(root, "phase", "$path.phase"), "phase", "$path.phase")) {
            "config" -> AliasSetupValidationPhaseV1.CONFIG
            "catalog" -> AliasSetupValidationPhaseV1.CATALOG
            "bootstrap" -> AliasSetupValidationPhaseV1.BOOTSTRAP
            "world_state" -> AliasSetupValidationPhaseV1.WORLD_STATE
            "planning" -> AliasSetupValidationPhaseV1.PLANNING
            else -> error("$path.phase is unsupported")
        }
        val severity = when (
            parseTaggedVariant(
                objectField(root, "severity", "$path.severity"),
                "severity",
                "$path.severity",
            )
        ) {
            "info" -> AliasSetupSeverityV1.INFO
            "warning" -> AliasSetupSeverityV1.WARNING
            "error" -> AliasSetupSeverityV1.ERROR
            else -> error("$path.severity is unsupported")
        }
        return AliasSetupDiagnosticV1(
            phase,
            stringField(root, "code", "$path.code"),
            severity,
            optionalString(root, "resource", "$path.resource"),
            optionalString(root, "config_path", "$path.config_path"),
            optionalString(root, "expected", "$path.expected"),
            optionalString(root, "actual", "$path.actual"),
            stringField(root, "remediation", "$path.remediation"),
        )
    }

    private fun parseUnitVariant(root: Map<String, Any?>, path: String): String =
        parseTaggedVariant(root, "kind", path)

    internal fun parseTaggedVariant(root: Map<String, Any?>, tag: String, path: String): String {
        exactKeys(root, setOf(tag, "value"), path)
        check(!root.containsKey("value") || root["value"] == null) { "$path.value must be null" }
        return stringField(root, tag, "$path.$tag")
    }

    internal fun exactKeys(root: Map<String, Any?>, expected: Set<String>, path: String) {
        val unknown = root.keys - expected
        check(unknown.isEmpty()) { "$path contains unknown fields: ${unknown.sorted().joinToString(",")}" }
        val missing = expected.filter { !root.containsKey(it) }
        check(missing.isEmpty()) { "$path is missing fields: ${missing.sorted().joinToString(",")}" }
    }

    @Suppress("UNCHECKED_CAST")
    internal fun objectValue(value: Any?, path: String): Map<String, Any?> {
        check(value is Map<*, *>) { "$path must be an object" }
        check(value.keys.all { it is String }) { "$path keys must be strings" }
        return value as Map<String, Any?>
    }

    internal fun objectField(root: Map<String, Any?>, field: String, path: String): Map<String, Any?> =
        objectValue(root[field], path)

    internal fun optionalObject(root: Map<String, Any?>, field: String, path: String): Map<String, Any?>? =
        if (root[field] == null) null else objectValue(root[field], path)

    internal fun arrayField(root: Map<String, Any?>, field: String, path: String): List<Any?> {
        val value = root[field]
        check(value is List<*>) { "$path must be an array" }
        return value
    }

    internal fun stringField(root: Map<String, Any?>, field: String, path: String): String {
        val value = root[field]
        check(value is String) { "$path must be a string" }
        return value
    }

    internal fun optionalString(root: Map<String, Any?>, field: String, path: String): String? {
        val value = root[field] ?: return null
        check(value is String) { "$path must be a string or null" }
        return value
    }

    internal fun intField(root: Map<String, Any?>, field: String, path: String): Int =
        try {
            exactInteger(root[field], path).intValueExact()
        } catch (error: ArithmeticException) {
            throw IllegalStateException("$path must fit in a signed 32-bit integer", error)
        }

    internal fun longField(root: Map<String, Any?>, field: String, path: String): Long =
        try {
            exactInteger(root[field], path).longValueExact()
        } catch (error: ArithmeticException) {
            throw IllegalStateException("$path must fit in a signed 64-bit integer", error)
        }

    private fun optionalLong(root: Map<String, Any?>, field: String, path: String): Long? =
        if (root[field] == null) null else longField(root, field, path)

    private fun u64Field(root: Map<String, Any?>, field: String, path: String): BigInteger =
        exactInteger(root[field], path).also {
            check(it >= BigInteger.ZERO && it.bitLength() <= 64) { "$path must be an unsigned 64-bit integer" }
        }

    private fun exactInteger(value: Any?, path: String): BigInteger {
        check(value is Number) { "$path must be an integer" }
        return when (value) {
            is BigInteger -> value
            is BigDecimal -> try {
                value.toBigIntegerExact()
            } catch (error: ArithmeticException) {
                throw IllegalStateException("$path must be an integer", error)
            }
            is Byte, is Short, is Int, is Long -> BigInteger.valueOf(value.toLong())
            else -> throw IllegalStateException("$path must be an exact integer")
        }
    }
}
