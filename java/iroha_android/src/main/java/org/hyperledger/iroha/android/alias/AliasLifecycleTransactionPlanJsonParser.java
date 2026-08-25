package org.hyperledger.iroha.android.alias;

import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.arrayField;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.exactKeys;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.intField;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.longField;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.objectField;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.objectValue;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.optionalObject;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.set;
import static org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser.stringField;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.model.NetworkId;

/** Strict parser for lease-renewal and auto-renew planner responses. */
public final class AliasLifecycleTransactionPlanJsonParser {
  private AliasLifecycleTransactionPlanJsonParser() {}

  /** Parses one complete lifecycle plan without accepting unknown fields. */
  public static AliasLifecycleTransactionPlanV1 parse(final byte[] payload) {
    final Map<String, Object> root =
        objectValue(
            JsonParser.parse(new String(payload, StandardCharsets.UTF_8)),
            "alias lifecycle transaction plan");
    exactKeys(root, set("body", "plan_hash"), "alias lifecycle transaction plan");
    return new AliasLifecycleTransactionPlanV1(
        parseBody(objectField(root, "body", "alias lifecycle transaction plan.body")),
        stringField(root, "plan_hash", "alias lifecycle transaction plan.plan_hash"));
  }

  private static AliasLifecycleTransactionPlanBodyV1 parseBody(
      final Map<String, Object> root) {
    exactKeys(
        root,
        set(
            "version",
            "authority",
            "network_id",
            "anchor",
            "operation",
            "disposition",
            "instruction",
            "quote",
            "totals_by_asset",
            "warnings",
            "blockers",
            "valid_until_ms"),
        "alias lifecycle transaction plan.body");
    final Map<String, Object> instruction = optionalObject(root, "instruction", "body.instruction");
    final Map<String, Object> quote = optionalObject(root, "quote", "body.quote");
    return new AliasLifecycleTransactionPlanBodyV1(
        intField(root, "version", "body.version"),
        stringField(root, "authority", "body.authority"),
        NetworkId.parseNoritoJsonLiteral(
            stringField(root, "network_id", "body.network_id")),
        AliasTransactionPlanJsonParser.parseAnchor(objectField(root, "anchor", "body.anchor")),
        parseOperation(objectField(root, "operation", "body.operation"), "body.operation"),
        parseDisposition(objectField(root, "disposition", "body.disposition"), "body.disposition"),
        instruction == null
            ? null
            : AliasTransactionPlanJsonParser.parseFrame(instruction, "body.instruction"),
        quote == null ? null : AliasTransactionPlanJsonParser.parseQuote(quote, "body.quote"),
        parseTotals(root),
        parseDiagnostics(root, "warnings"),
        parseDiagnostics(root, "blockers"),
        longField(root, "valid_until_ms", "body.valid_until_ms"));
  }

  private static List<AliasSetupModels.AliasAssetTotalV1> parseTotals(
      final Map<String, Object> root) {
    final List<Object> values = arrayField(root, "totals_by_asset", "body.totals_by_asset");
    final List<AliasSetupModels.AliasAssetTotalV1> result = new ArrayList<>(values.size());
    for (int index = 0; index < values.size(); index++) {
      final String path = "body.totals_by_asset[" + index + "]";
      result.add(AliasTransactionPlanJsonParser.parseTotal(objectValue(values.get(index), path), path));
    }
    return result;
  }

  private static List<AliasSetupModels.AliasSetupDiagnosticV1> parseDiagnostics(
      final Map<String, Object> root, final String field) {
    final String base = "body." + field;
    final List<Object> values = arrayField(root, field, base);
    final List<AliasSetupModels.AliasSetupDiagnosticV1> result = new ArrayList<>(values.size());
    for (int index = 0; index < values.size(); index++) {
      final String path = base + "[" + index + "]";
      result.add(
          AliasTransactionPlanJsonParser.parseDiagnostic(objectValue(values.get(index), path), path));
    }
    return result;
  }

  private static AliasLifecycleOperationV1 parseOperation(
      final Map<String, Object> root, final String path) {
    exactKeys(root, set("kind", "operation"), path);
    final String kind = stringField(root, "kind", path + ".kind");
    final Map<String, Object> operation = objectField(root, "operation", path + ".operation");
    if ("renew_lease".equals(kind)) {
      return new AliasLifecycleOperationV1.RenewLease(
          parseRenewal(operation, path + ".operation"));
    }
    if ("configure_auto_renew".equals(kind)) {
      return new AliasLifecycleOperationV1.ConfigureAutoRenew(
          parseAutoRenew(operation, path + ".operation"));
    }
    throw new IllegalStateException(path + ".kind is unsupported");
  }

  private static RenewAliasLease parseRenewal(
      final Map<String, Object> root, final String path) {
    exactKeys(
        root,
        set("target", "expected_current_expiry_ms", "target_expiry_ms", "quote_guard"),
        path);
    return new RenewAliasLease(
        AliasTransactionPlanJsonParser.parseTarget(
            objectField(root, "target", path + ".target"), path + ".target"),
        longField(root, "expected_current_expiry_ms", path + ".expected_current_expiry_ms"),
        longField(root, "target_expiry_ms", path + ".target_expiry_ms"),
        AliasTransactionPlanJsonParser.parseGuard(
            objectField(root, "quote_guard", path + ".quote_guard"), path + ".quote_guard"));
  }

  private static ConfigureAliasAutoRenew parseAutoRenew(
      final Map<String, Object> root, final String path) {
    exactKeys(root, set("target", "expected_revision", "config"), path);
    final Map<String, Object> config = optionalObject(root, "config", path + ".config");
    return new ConfigureAliasAutoRenew(
        AliasTransactionPlanJsonParser.parseTarget(
            objectField(root, "target", path + ".target"), path + ".target"),
        longField(root, "expected_revision", path + ".expected_revision"),
        config == null ? null : parseAutoRenewConfig(config, path + ".config"));
  }

  private static AliasAutoRenewConfigV1 parseAutoRenewConfig(
      final Map<String, Object> root, final String path) {
    exactKeys(
        root,
        set(
            "term_years",
            "policy_version",
            "payment_asset",
            "max_amount",
            "renew_before_expiry_ms",
            "retry_backoff_ms",
            "max_failures"),
        path);
    return new AliasAutoRenewConfigV1(
        intField(root, "term_years", path + ".term_years"),
        intField(root, "policy_version", path + ".policy_version"),
        stringField(root, "payment_asset", path + ".payment_asset"),
        stringField(root, "max_amount", path + ".max_amount"),
        longField(root, "renew_before_expiry_ms", path + ".renew_before_expiry_ms"),
        longField(root, "retry_backoff_ms", path + ".retry_backoff_ms"),
        longField(root, "max_failures", path + ".max_failures"));
  }

  private static AliasLifecyclePlanDispositionV1 parseDisposition(
      final Map<String, Object> root, final String path) {
    final String value = AliasTransactionPlanJsonParser.parseTaggedVariant(root, "kind", path);
    if ("no_op".equals(value)) return AliasLifecyclePlanDispositionV1.NO_OP;
    if ("apply".equals(value)) return AliasLifecyclePlanDispositionV1.APPLY;
    throw new IllegalStateException(path + ".kind is unsupported");
  }
}
