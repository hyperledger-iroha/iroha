package org.hyperledger.iroha.android.alias;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.model.NetworkId;

/** Strict parser for the typed response returned by {@code POST /v1/aliases/setup/plan}. */
public final class AliasTransactionPlanJsonParser {
  private AliasTransactionPlanJsonParser() {}

  /** Parses one complete plan without accepting lossy numeric coercions. */
  public static AliasTransactionPlanV1 parse(final byte[] payload) {
    final Map<String, Object> root =
        objectValue(
            JsonParser.parse(new String(payload, StandardCharsets.UTF_8)),
            "alias transaction plan");
    exactKeys(root, set("body", "plan_hash"), "alias transaction plan");
    return new AliasTransactionPlanV1(
        parseBody(objectField(root, "body", "alias transaction plan.body")),
        stringField(root, "plan_hash", "alias transaction plan.plan_hash"));
  }

  private static AliasSetupModels.AliasTransactionPlanBodyV1 parseBody(
      final Map<String, Object> root) {
    exactKeys(
        root,
        set(
            "version",
            "authority",
            "network_id",
            "anchor",
            "resources",
            "instructions",
            "totals_by_asset",
            "warnings",
            "blockers",
            "valid_until_ms"),
        "alias transaction plan.body");
    return new AliasSetupModels.AliasTransactionPlanBodyV1(
        intField(root, "version", "body.version"),
        stringField(root, "authority", "body.authority"),
        NetworkId.parse(stringField(root, "network_id", "body.network_id")),
        parseAnchor(objectField(root, "anchor", "body.anchor")),
        mapObjects(root, "resources", "body.resources", AliasTransactionPlanJsonParser::parseResource),
        mapObjects(root, "instructions", "body.instructions", AliasTransactionPlanJsonParser::parseFrame),
        mapObjects(root, "totals_by_asset", "body.totals_by_asset", AliasTransactionPlanJsonParser::parseTotal),
        mapObjects(root, "warnings", "body.warnings", AliasTransactionPlanJsonParser::parseDiagnostic),
        mapObjects(root, "blockers", "body.blockers", AliasTransactionPlanJsonParser::parseDiagnostic),
        longField(root, "valid_until_ms", "body.valid_until_ms"));
  }

  static AliasSetupModels.AliasPlanAnchorV1 parseAnchor(
      final Map<String, Object> root) {
    exactKeys(root, set("block_height", "block_hash"), "body.anchor");
    return new AliasSetupModels.AliasPlanAnchorV1(
        longField(root, "block_height", "body.anchor.block_height"),
        stringField(root, "block_hash", "body.anchor.block_hash"));
  }

  static AliasSetupModels.AliasPlanResourceV1 parseResource(
      final Map<String, Object> root, final String path) {
    exactKeys(root, set("intent", "disposition", "quote", "instruction_index"), path);
    final Map<String, Object> quote = optionalObject(root, "quote", path + ".quote");
    return new AliasSetupModels.AliasPlanResourceV1(
        parseIntent(objectField(root, "intent", path + ".intent"), path + ".intent"),
        parseDisposition(
            objectField(root, "disposition", path + ".disposition"), path + ".disposition"),
        quote == null ? null : parseQuote(quote, path + ".quote"),
        optionalLong(root, "instruction_index", path + ".instruction_index"));
  }

  static AliasSetupModels.AliasFramedInstructionV1 parseFrame(
      final Map<String, Object> root, final String path) {
    exactKeys(root, set("wire_id", "framed_payload"), path);
    final List<Object> values = arrayField(root, "framed_payload", path + ".framed_payload");
    final byte[] bytes = new byte[values.size()];
    for (int index = 0; index < bytes.length; index++) {
      final BigInteger value = exactInteger(values.get(index), path + ".framed_payload[" + index + "]");
      if (value.signum() < 0 || value.compareTo(BigInteger.valueOf(255)) > 0) {
        throw state(path + ".framed_payload[" + index + "] must be an unsigned byte");
      }
      bytes[index] = value.byteValue();
    }
    return new AliasSetupModels.AliasFramedInstructionV1(
        stringField(root, "wire_id", path + ".wire_id"), bytes);
  }

  static AliasSetupModels.AliasAssetTotalV1 parseTotal(
      final Map<String, Object> root, final String path) {
    exactKeys(root, set("payment_asset", "amount"), path);
    return new AliasSetupModels.AliasAssetTotalV1(
        stringField(root, "payment_asset", path + ".payment_asset"),
        stringField(root, "amount", path + ".amount"));
  }

  static AliasSetupModels.AliasLeaseQuoteV1 parseQuote(
      final Map<String, Object> root, final String path) {
    exactKeys(
        root,
        set(
            "target",
            "pricing_class",
            "exact_amount",
            "guard",
            "expires_at_ms",
            "grace_expires_at_ms",
            "redemption_expires_at_ms"),
        path);
    return new AliasSetupModels.AliasLeaseQuoteV1(
        parseTarget(objectField(root, "target", path + ".target"), path + ".target"),
        intField(root, "pricing_class", path + ".pricing_class"),
        stringField(root, "exact_amount", path + ".exact_amount"),
        parseGuard(objectField(root, "guard", path + ".guard"), path + ".guard"),
        longField(root, "expires_at_ms", path + ".expires_at_ms"),
        longField(root, "grace_expires_at_ms", path + ".grace_expires_at_ms"),
        longField(root, "redemption_expires_at_ms", path + ".redemption_expires_at_ms"));
  }

  private static AliasSetupModels.AliasIntentV1 parseIntent(
      final Map<String, Object> root, final String path) {
    exactKeys(root, set("kind", "intent"), path);
    final String kind = stringField(root, "kind", path + ".kind");
    final Map<String, Object> value = objectField(root, "intent", path + ".intent");
    switch (kind) {
      case "dataspace":
        exactKeys(value, set("dataspace", "owner"), path + ".intent");
        return new AliasSetupModels.DataspaceIntent(
            new AliasSetupModels.AliasDataSpaceIntentV1(
                parseDataspace(
                    objectField(value, "dataspace", path + ".intent.dataspace"),
                    path + ".intent.dataspace"),
                stringField(value, "owner", path + ".intent.owner")));
      case "domain":
        exactKeys(value, set("domain", "owner"), path + ".intent");
        return new AliasSetupModels.DomainIntent(
            new AliasSetupModels.AliasDomainIntentV1(
                parseDomain(
                    objectField(value, "domain", path + ".intent.domain"),
                    path + ".intent.domain"),
                stringField(value, "owner", path + ".intent.owner")));
      case "account_alias":
        exactKeys(
            value,
            set("alias", "target_account", "provision", "role"),
            path + ".intent");
        return new AliasSetupModels.AccountAliasIntent(
            new AliasSetupModels.AliasAccountIntentV1(
                parseAccountAlias(
                    objectField(value, "alias", path + ".intent.alias"),
                    path + ".intent.alias"),
                stringField(value, "target_account", path + ".intent.target_account"),
                provision(
                    parseUnitVariant(
                        objectField(value, "provision", path + ".intent.provision"),
                        path + ".intent.provision"),
                    path + ".intent.provision"),
                role(
                    parseUnitVariant(
                        objectField(value, "role", path + ".intent.role"),
                        path + ".intent.role"),
                    path + ".intent.role")));
      default:
        throw state(path + ".kind is unsupported");
    }
  }

  private static AliasSetupModels.AccountProvisionV1 provision(
      final String value, final String path) {
    if ("existing".equals(value)) return AliasSetupModels.AccountProvisionV1.EXISTING;
    if ("create".equals(value)) return AliasSetupModels.AccountProvisionV1.CREATE;
    throw state(path + ".kind is unsupported");
  }

  private static AliasSetupModels.AccountAliasRoleV1 role(
      final String value, final String path) {
    if ("primary".equals(value)) return AliasSetupModels.AccountAliasRoleV1.PRIMARY;
    if ("additional".equals(value)) return AliasSetupModels.AccountAliasRoleV1.ADDITIONAL;
    throw state(path + ".kind is unsupported");
  }

  static AliasSetupModels.AliasTargetV1 parseTarget(
      final Map<String, Object> root, final String path) {
    exactKeys(root, set("kind", "resource"), path);
    final String kind = stringField(root, "kind", path + ".kind");
    final Map<String, Object> resource = objectField(root, "resource", path + ".resource");
    switch (kind) {
      case "dataspace":
        return new AliasSetupModels.DataspaceTarget(parseDataspace(resource, path + ".resource"));
      case "domain":
        return new AliasSetupModels.DomainTarget(parseDomain(resource, path + ".resource"));
      case "account_alias":
        return new AliasSetupModels.AccountAliasTarget(
            parseAccountAlias(resource, path + ".resource"));
      default:
        throw state(path + ".kind is unsupported");
    }
  }

  private static ResolvedDataSpaceV1 parseDataspace(
      final Map<String, Object> root, final String path) {
    exactKeys(root, set("canonical_name", "dataspace_id"), path);
    return new ResolvedDataSpaceV1(
        stringField(root, "canonical_name", path + ".canonical_name"),
        u64Field(root, "dataspace_id", path + ".dataspace_id"));
  }

  private static ResolvedDomainV1 parseDomain(
      final Map<String, Object> root, final String path) {
    exactKeys(root, set("canonical_name", "dataspace_id"), path);
    return new ResolvedDomainV1(
        stringField(root, "canonical_name", path + ".canonical_name"),
        u64Field(root, "dataspace_id", path + ".dataspace_id"));
  }

  private static ResolvedAccountAliasV1 parseAccountAlias(
      final Map<String, Object> root, final String path) {
    exactKeys(root, set("canonical_name", "dataspace_id"), path);
    final Map<String, Object> name =
        objectField(root, "canonical_name", path + ".canonical_name");
    exactKeys(name, set("label", "domain", "dataspace"), path + ".canonical_name");
    return new ResolvedAccountAliasV1(
        new AccountAliasName(
            stringField(name, "label", path + ".canonical_name.label"),
            optionalString(name, "domain", path + ".canonical_name.domain"),
            stringField(name, "dataspace", path + ".canonical_name.dataspace")),
        u64Field(root, "dataspace_id", path + ".dataspace_id"));
  }

  static AliasQuoteGuardV1 parseGuard(
      final Map<String, Object> root, final String path) {
    exactKeys(
        root,
        set(
            "expected_policy_version",
            "expected_payment_asset",
            "max_amount",
            "valid_until_ms"),
        path);
    return new AliasQuoteGuardV1(
        intField(root, "expected_policy_version", path + ".expected_policy_version"),
        stringField(root, "expected_payment_asset", path + ".expected_payment_asset"),
        stringField(root, "max_amount", path + ".max_amount"),
        longField(root, "valid_until_ms", path + ".valid_until_ms"));
  }

  private static AliasSetupModels.AliasPlanDispositionV1 parseDisposition(
      final Map<String, Object> root, final String path) {
    final String value = parseUnitVariant(root, path);
    if ("no_op".equals(value)) return AliasSetupModels.AliasPlanDispositionV1.NO_OP;
    if ("repair".equals(value)) return AliasSetupModels.AliasPlanDispositionV1.REPAIR;
    if ("create".equals(value)) return AliasSetupModels.AliasPlanDispositionV1.CREATE;
    if ("conflict".equals(value)) return AliasSetupModels.AliasPlanDispositionV1.CONFLICT;
    throw state(path + ".kind is unsupported");
  }

  static AliasSetupModels.AliasSetupDiagnosticV1 parseDiagnostic(
      final Map<String, Object> root, final String path) {
    exactKeys(
        root,
        set(
            "phase",
            "code",
            "severity",
            "resource",
            "config_path",
            "expected",
            "actual",
            "remediation"),
        path);
    return new AliasSetupModels.AliasSetupDiagnosticV1(
        phase(
            parseTaggedVariant(
                objectField(root, "phase", path + ".phase"), "phase", path + ".phase"),
            path + ".phase"),
        stringField(root, "code", path + ".code"),
        severity(
            parseTaggedVariant(
                objectField(root, "severity", path + ".severity"),
                "severity",
                path + ".severity"),
            path + ".severity"),
        optionalString(root, "resource", path + ".resource"),
        optionalString(root, "config_path", path + ".config_path"),
        optionalString(root, "expected", path + ".expected"),
        optionalString(root, "actual", path + ".actual"),
        stringField(root, "remediation", path + ".remediation"));
  }

  private static AliasSetupModels.AliasSetupValidationPhaseV1 phase(
      final String value, final String path) {
    for (final AliasSetupModels.AliasSetupValidationPhaseV1 candidate :
        AliasSetupModels.AliasSetupValidationPhaseV1.values()) {
      if (candidate.wireValue().equals(value)) return candidate;
    }
    throw state(path + " is unsupported");
  }

  private static AliasSetupModels.AliasSetupSeverityV1 severity(
      final String value, final String path) {
    for (final AliasSetupModels.AliasSetupSeverityV1 candidate :
        AliasSetupModels.AliasSetupSeverityV1.values()) {
      if (candidate.wireValue().equals(value)) return candidate;
    }
    throw state(path + " is unsupported");
  }

  private static String parseUnitVariant(
      final Map<String, Object> root, final String path) {
    return parseTaggedVariant(root, "kind", path);
  }

  static String parseTaggedVariant(
      final Map<String, Object> root, final String tag, final String path) {
    exactKeys(root, set(tag, "value"), path);
    if (root.get("value") != null) throw state(path + ".value must be null");
    return stringField(root, tag, path + "." + tag);
  }

  private interface ObjectParser<T> {
    T parse(Map<String, Object> root, String path);
  }

  private static <T> List<T> mapObjects(
      final Map<String, Object> root,
      final String field,
      final String path,
      final ObjectParser<T> parser) {
    final List<Object> values = arrayField(root, field, path);
    final List<T> result = new ArrayList<>(values.size());
    for (int index = 0; index < values.size(); index++) {
      final String itemPath = path + "[" + index + "]";
      result.add(parser.parse(objectValue(values.get(index), itemPath), itemPath));
    }
    return result;
  }

  static Set<String> set(final String... values) {
    return new HashSet<>(Arrays.asList(values));
  }

  static void exactKeys(
      final Map<String, Object> root, final Set<String> expected, final String path) {
    final Set<String> unknown = new HashSet<>(root.keySet());
    unknown.removeAll(expected);
    if (!unknown.isEmpty()) throw state(path + " contains unknown fields: " + unknown);
    final Set<String> missing = new HashSet<>(expected);
    missing.removeAll(root.keySet());
    if (!missing.isEmpty()) throw state(path + " is missing fields: " + missing);
  }

  @SuppressWarnings("unchecked")
  static Map<String, Object> objectValue(final Object value, final String path) {
    if (!(value instanceof Map<?, ?>)) throw state(path + " must be an object");
    final Map<?, ?> map = (Map<?, ?>) value;
    for (final Object key : map.keySet()) {
      if (!(key instanceof String)) throw state(path + " keys must be strings");
    }
    return (Map<String, Object>) map;
  }

  static Map<String, Object> objectField(
      final Map<String, Object> root, final String field, final String path) {
    return objectValue(root.get(field), path);
  }

  static Map<String, Object> optionalObject(
      final Map<String, Object> root, final String field, final String path) {
    return root.get(field) == null ? null : objectValue(root.get(field), path);
  }

  @SuppressWarnings("unchecked")
  static List<Object> arrayField(
      final Map<String, Object> root, final String field, final String path) {
    final Object value = root.get(field);
    if (!(value instanceof List<?>)) throw state(path + " must be an array");
    return (List<Object>) value;
  }

  static String stringField(
      final Map<String, Object> root, final String field, final String path) {
    final Object value = root.get(field);
    if (!(value instanceof String)) throw state(path + " must be a string");
    return (String) value;
  }

  static String optionalString(
      final Map<String, Object> root, final String field, final String path) {
    final Object value = root.get(field);
    if (value == null) return null;
    if (!(value instanceof String)) throw state(path + " must be a string or null");
    return (String) value;
  }

  static int intField(
      final Map<String, Object> root, final String field, final String path) {
    try {
      return exactInteger(root.get(field), path).intValueExact();
    } catch (final ArithmeticException error) {
      throw new IllegalStateException(path + " must fit in a signed 32-bit integer", error);
    }
  }

  static long longField(
      final Map<String, Object> root, final String field, final String path) {
    try {
      return exactInteger(root.get(field), path).longValueExact();
    } catch (final ArithmeticException error) {
      throw new IllegalStateException(path + " must fit in a signed 64-bit integer", error);
    }
  }

  private static Long optionalLong(
      final Map<String, Object> root, final String field, final String path) {
    return root.get(field) == null ? null : Long.valueOf(longField(root, field, path));
  }

  private static BigInteger u64Field(
      final Map<String, Object> root, final String field, final String path) {
    final BigInteger value = exactInteger(root.get(field), path);
    if (value.signum() < 0 || value.bitLength() > 64) {
      throw state(path + " must be an unsigned 64-bit integer");
    }
    return value;
  }

  private static BigInteger exactInteger(final Object value, final String path) {
    if (value instanceof BigInteger) return (BigInteger) value;
    if (value instanceof BigDecimal) {
      try {
        return ((BigDecimal) value).toBigIntegerExact();
      } catch (final ArithmeticException error) {
        throw new IllegalStateException(path + " must be an integer", error);
      }
    }
    if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      return BigInteger.valueOf(((Number) value).longValue());
    }
    throw state(path + " must be an exact integer");
  }

  private static IllegalStateException state(final String message) {
    return new IllegalStateException(message);
  }
}
