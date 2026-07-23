package org.hyperledger.iroha.android.client;

import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.hyperledger.iroha.android.model.FeeChargeKind;
import org.hyperledger.iroha.android.model.FeeChargeLimit;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.FeeSponsorProgramId;

final class FeePaymentJson {
  private FeePaymentJson() {}

  static FeeSponsorProgramResponse parseProgram(final byte[] payload) {
    final String path = "fee sponsor program response";
    final Map<String, Object> root =
        objectValue(JsonParser.parse(new String(payload, StandardCharsets.UTF_8)), path);
    requireExactKeys(
        root,
        keys("id", "lifecycle", "active_revision", "staged_revision", "scheduled_activation"),
        keys("id", "lifecycle"),
        path);
    final Map<String, Object> id = objectValue(root.get("id"), path + ".id");
    requireExactKeys(id, keys("sponsor", "name"), keys("sponsor", "name"), path + ".id");
    final Map<String, Object> lifecycle =
        objectValue(root.get("lifecycle"), path + ".lifecycle");
    requireExactKeys(
        lifecycle, keys("state", "value"), keys("state", "value"), path + ".lifecycle");
    if (lifecycle.get("value") != null) {
      throw new IllegalArgumentException(path + ".lifecycle.value must be null");
    }
    final FeeSponsorProgramLifecycle lifecycleValue;
    final String state = string(lifecycle.get("state"), path + ".lifecycle.state");
    switch (state) {
      case "staged":
        lifecycleValue = FeeSponsorProgramLifecycle.STAGED;
        break;
      case "paused":
        lifecycleValue = FeeSponsorProgramLifecycle.PAUSED;
        break;
      case "active":
        lifecycleValue = FeeSponsorProgramLifecycle.ACTIVE;
        break;
      case "closing":
        lifecycleValue = FeeSponsorProgramLifecycle.CLOSING;
        break;
      case "closed":
        lifecycleValue = FeeSponsorProgramLifecycle.CLOSED;
        break;
      default:
        throw new IllegalArgumentException(path + ".lifecycle.state is unsupported");
    }
    FeeSponsorProgramActivation activation = null;
    if (root.get("scheduled_activation") != null) {
      final Map<String, Object> value =
          objectValue(root.get("scheduled_activation"), path + ".scheduled_activation");
      requireExactKeys(
          value,
          keys("revision", "activate_at_height"),
          keys("revision", "activate_at_height"),
          path + ".scheduled_activation");
      activation =
          new FeeSponsorProgramActivation(
              positiveLong(value.get("revision"), path + ".scheduled_activation.revision"),
              nonNegativeLong(
                  value.get("activate_at_height"),
                  path + ".scheduled_activation.activate_at_height"));
    }
    return new FeeSponsorProgramResponse(
        new FeeSponsorProgramId(
            string(id.get("sponsor"), path + ".id.sponsor"),
            string(id.get("name"), path + ".id.name")),
        lifecycleValue,
        root.get("active_revision") == null
            ? null : positiveLong(root.get("active_revision"), path + ".active_revision"),
        root.get("staged_revision") == null
            ? null : positiveLong(root.get("staged_revision"), path + ".staged_revision"),
        activation);
  }

  static FeeQuoteResponse parseQuote(final byte[] payload) {
    final Map<String, Object> root = objectValue(
        JsonParser.parse(new String(payload, StandardCharsets.UTF_8)), "fee quote response");
    requireExactKeys(
        root,
        keys("intent", "observation", "components", "capacities", "decision"),
        keys("intent", "observation", "components", "capacities", "decision"),
        "fee quote response");
    return new FeeQuoteResponse(
        parse(root.get("intent"), "fee quote response.intent"),
        objectValue(root.get("observation"), "fee quote response.observation"),
        objectList(root.get("components"), "fee quote response.components"),
        objectList(root.get("capacities"), "fee quote response.capacities"),
        objectValue(root.get("decision"), "fee quote response.decision"));
  }

  static FeePaymentIntent parse(final Object value, final String path) {
    final Map<String, Object> root = objectValue(value, path);
    requireExactKeys(root, keys("payer", "value"), keys("payer", "value"), path);
    final String payer = string(root.get("payer"), path + ".payer");
    final Map<String, Object> body = objectValue(root.get("value"), path + ".value");
    final Set<String> allowed = payer.equals("sponsor")
        ? keys("charge_limits", "gas_limit", "program_id", "program_revision")
        : keys("charge_limits", "gas_limit");
    requireExactKeys(body, allowed, keys("charge_limits"), path + ".value");
    if (!(body.get("charge_limits") instanceof List<?>)) {
      throw new IllegalArgumentException(path + ".value.charge_limits must be an array");
    }
    final List<FeeChargeLimit> limits = new ArrayList<>();
    final List<?> rawLimits = (List<?>) body.get("charge_limits");
    for (int index = 0; index < rawLimits.size(); index++) {
      limits.add(parseLimit(rawLimits.get(index), path + ".value.charge_limits[" + index + "]"));
    }
    final Long gasLimit = body.get("gas_limit") == null
        ? null : positiveLong(body.get("gas_limit"), path + ".value.gas_limit");
    if (payer.equals("authority")) return FeePaymentIntent.authority(limits, gasLimit);
    if (!payer.equals("sponsor")) {
      throw new IllegalArgumentException(path + ".payer must be authority or sponsor");
    }
    final Map<String, Object> program =
        objectValue(body.get("program_id"), path + ".value.program_id");
    requireExactKeys(
        program, keys("sponsor", "name"), keys("sponsor", "name"), path + ".value.program_id");
    return FeePaymentIntent.sponsor(
        new FeeSponsorProgramId(
            string(program.get("sponsor"), path + ".value.program_id.sponsor"),
            string(program.get("name"), path + ".value.program_id.name")),
        positiveLong(body.get("program_revision"), path + ".value.program_revision"),
        limits,
        gasLimit);
  }

  private static FeeChargeLimit parseLimit(final Object value, final String path) {
    final Map<String, Object> item = objectValue(value, path);
    requireExactKeys(
        item,
        keys("kind", "asset_definition_id", "max_amount"),
        keys("kind", "asset_definition_id", "max_amount"),
        path);
    final Map<String, Object> kindObject = objectValue(item.get("kind"), path + ".kind");
    requireExactKeys(kindObject, keys("kind", "value"), keys("kind", "value"), path + ".kind");
    if (kindObject.get("value") != null) {
      throw new IllegalArgumentException(path + ".kind.value must be null");
    }
    final String kindText = string(kindObject.get("kind"), path + ".kind.kind");
    final FeeChargeKind kind;
    if (kindText.equals("nexus")) kind = FeeChargeKind.NEXUS;
    else if (kindText.equals("pipeline_gas")) kind = FeeChargeKind.PIPELINE_GAS;
    else throw new IllegalArgumentException(path + ".kind.kind must be nexus or pipeline_gas");
    return new FeeChargeLimit(
        kind,
        string(item.get("asset_definition_id"), path + ".asset_definition_id"),
        string(item.get("max_amount"), path + ".max_amount"));
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> objectValue(final Object value, final String path) {
    if (!(value instanceof Map<?, ?>)) {
      throw new IllegalArgumentException(path + " must be an object");
    }
    final Map<?, ?> map = (Map<?, ?>) value;
    for (final Object key : map.keySet()) {
      if (!(key instanceof String)) throw new IllegalArgumentException(path + " keys must be strings");
    }
    return (Map<String, Object>) map;
  }

  private static List<Map<String, Object>> objectList(final Object value, final String path) {
    if (!(value instanceof List<?>)) throw new IllegalArgumentException(path + " must be an array");
    final List<Map<String, Object>> out = new ArrayList<>();
    final List<?> list = (List<?>) value;
    for (int index = 0; index < list.size(); index++) {
      out.add(objectValue(list.get(index), path + "[" + index + "]"));
    }
    return out;
  }

  private static String string(final Object value, final String path) {
    if (!(value instanceof String)) throw new IllegalArgumentException(path + " must be a string");
    return (String) value;
  }

  private static long positiveLong(final Object value, final String path) {
    final long result;
    try {
      if (value instanceof BigDecimal) result = ((BigDecimal) value).longValueExact();
      else if (value instanceof Byte || value instanceof Short
          || value instanceof Integer || value instanceof Long) result = ((Number) value).longValue();
      else throw new IllegalArgumentException(path + " must be an integer");
    } catch (final ArithmeticException ex) {
      throw new IllegalArgumentException(path + " must be an integer", ex);
    }
    if (result <= 0L) throw new IllegalArgumentException(path + " must be positive");
    return result;
  }

  private static long nonNegativeLong(final Object value, final String path) {
    final long result;
    try {
      if (value instanceof BigDecimal) result = ((BigDecimal) value).longValueExact();
      else if (value instanceof Byte || value instanceof Short
          || value instanceof Integer || value instanceof Long) result = ((Number) value).longValue();
      else throw new IllegalArgumentException(path + " must be an integer");
    } catch (final ArithmeticException ex) {
      throw new IllegalArgumentException(path + " must be an integer", ex);
    }
    if (result < 0L) throw new IllegalArgumentException(path + " must be non-negative");
    return result;
  }

  private static Set<String> keys(final String... values) {
    return new HashSet<>(Arrays.asList(values));
  }

  private static void requireExactKeys(
      final Map<String, Object> value,
      final Set<String> allowed,
      final Set<String> required,
      final String path) {
    final Set<String> unknown = new HashSet<>(value.keySet());
    unknown.removeAll(allowed);
    if (!unknown.isEmpty()) throw new IllegalArgumentException(path + " contains unknown fields: " + unknown);
    final Set<String> missing = new HashSet<>(required);
    missing.removeAll(value.keySet());
    if (!missing.isEmpty()) throw new IllegalArgumentException(path + " is missing required fields: " + missing);
  }
}
