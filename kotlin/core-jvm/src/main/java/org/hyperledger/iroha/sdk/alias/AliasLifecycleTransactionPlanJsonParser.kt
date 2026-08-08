package org.hyperledger.iroha.sdk.alias

import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.model.NetworkId

/** Strict parser for lease-renewal and auto-renew planner responses. */
object AliasLifecycleTransactionPlanJsonParser {
    private val parser = AliasTransactionPlanJsonParser

    /** Parses one complete lifecycle plan without accepting unknown fields. */
    @JvmStatic
    fun parse(payload: ByteArray): AliasLifecycleTransactionPlanV1 {
        val root = parser.objectValue(
            JsonParser.parse(String(payload, StandardCharsets.UTF_8)),
            "alias lifecycle transaction plan",
        )
        parser.exactKeys(root, setOf("body", "plan_hash"), "alias lifecycle transaction plan")
        return AliasLifecycleTransactionPlanV1(
            parseBody(parser.objectField(root, "body", "alias lifecycle transaction plan.body")),
            parser.stringField(root, "plan_hash", "alias lifecycle transaction plan.plan_hash"),
        )
    }

    private fun parseBody(root: Map<String, Any?>): AliasLifecycleTransactionPlanBodyV1 {
        parser.exactKeys(
            root,
            setOf(
                "version", "authority", "network_id", "anchor", "operation", "disposition",
                "instruction", "quote", "totals_by_asset", "warnings", "blockers",
                "valid_until_ms",
            ),
            "alias lifecycle transaction plan.body",
        )
        return AliasLifecycleTransactionPlanBodyV1(
            parser.intField(root, "version", "body.version"),
            parser.stringField(root, "authority", "body.authority"),
            NetworkId.parse(parser.stringField(root, "network_id", "body.network_id")),
            parser.parseAnchor(parser.objectField(root, "anchor", "body.anchor")),
            parseOperation(parser.objectField(root, "operation", "body.operation"), "body.operation"),
            parseDisposition(
                parser.objectField(root, "disposition", "body.disposition"),
                "body.disposition",
            ),
            parser.optionalObject(root, "instruction", "body.instruction")?.let {
                parser.parseFrame(it, "body.instruction")
            },
            parser.optionalObject(root, "quote", "body.quote")?.let {
                parser.parseQuote(it, "body.quote")
            },
            parser.arrayField(root, "totals_by_asset", "body.totals_by_asset")
                .mapIndexed { index, value ->
                    parser.parseTotal(
                        parser.objectValue(value, "body.totals_by_asset[$index]"),
                        "body.totals_by_asset[$index]",
                    )
                },
            parser.arrayField(root, "warnings", "body.warnings").mapIndexed { index, value ->
                parser.parseDiagnostic(
                    parser.objectValue(value, "body.warnings[$index]"),
                    "body.warnings[$index]",
                )
            },
            parser.arrayField(root, "blockers", "body.blockers").mapIndexed { index, value ->
                parser.parseDiagnostic(
                    parser.objectValue(value, "body.blockers[$index]"),
                    "body.blockers[$index]",
                )
            },
            parser.longField(root, "valid_until_ms", "body.valid_until_ms"),
        )
    }

    private fun parseOperation(root: Map<String, Any?>, path: String): AliasLifecycleOperationV1 {
        parser.exactKeys(root, setOf("kind", "operation"), path)
        val operation = parser.objectField(root, "operation", "$path.operation")
        return when (parser.stringField(root, "kind", "$path.kind")) {
            "renew_lease" -> AliasLifecycleOperationV1.RenewLease(parseRenewal(operation, "$path.operation"))
            "configure_auto_renew" -> AliasLifecycleOperationV1.ConfigureAutoRenew(
                parseAutoRenew(operation, "$path.operation"),
            )
            else -> error("$path.kind is unsupported")
        }
    }

    private fun parseRenewal(root: Map<String, Any?>, path: String): RenewAliasLease {
        parser.exactKeys(
            root,
            setOf("target", "expected_current_expiry_ms", "target_expiry_ms", "quote_guard"),
            path,
        )
        return RenewAliasLease(
            parser.parseTarget(parser.objectField(root, "target", "$path.target"), "$path.target"),
            parser.longField(root, "expected_current_expiry_ms", "$path.expected_current_expiry_ms"),
            parser.longField(root, "target_expiry_ms", "$path.target_expiry_ms"),
            parser.parseGuard(
                parser.objectField(root, "quote_guard", "$path.quote_guard"),
                "$path.quote_guard",
            ),
        )
    }

    private fun parseAutoRenew(root: Map<String, Any?>, path: String): ConfigureAliasAutoRenew {
        parser.exactKeys(root, setOf("target", "expected_revision", "config"), path)
        return ConfigureAliasAutoRenew(
            parser.parseTarget(parser.objectField(root, "target", "$path.target"), "$path.target"),
            parser.longField(root, "expected_revision", "$path.expected_revision"),
            parser.optionalObject(root, "config", "$path.config")?.let { parseAutoRenewConfig(it, "$path.config") },
        )
    }

    private fun parseAutoRenewConfig(root: Map<String, Any?>, path: String): AliasAutoRenewConfigV1 {
        parser.exactKeys(
            root,
            setOf(
                "term_years", "policy_version", "payment_asset", "max_amount",
                "renew_before_expiry_ms", "retry_backoff_ms", "max_failures",
            ),
            path,
        )
        return AliasAutoRenewConfigV1(
            parser.intField(root, "term_years", "$path.term_years"),
            parser.intField(root, "policy_version", "$path.policy_version"),
            parser.stringField(root, "payment_asset", "$path.payment_asset"),
            parser.stringField(root, "max_amount", "$path.max_amount"),
            parser.longField(root, "renew_before_expiry_ms", "$path.renew_before_expiry_ms"),
            parser.longField(root, "retry_backoff_ms", "$path.retry_backoff_ms"),
            parser.longField(root, "max_failures", "$path.max_failures"),
        )
    }

    private fun parseDisposition(
        root: Map<String, Any?>,
        path: String,
    ): AliasLifecyclePlanDispositionV1 =
        when (parser.parseTaggedVariant(root, "kind", path)) {
            "no_op" -> AliasLifecyclePlanDispositionV1.NO_OP
            "apply" -> AliasLifecyclePlanDispositionV1.APPLY
            else -> error("$path.kind is unsupported")
        }
}
