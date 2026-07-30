package org.hyperledger.iroha.android.sorafs;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.hyperledger.iroha.android.client.JsonParser;

/** Strict parser for the closed V1 SoraFS reputation JSON projections. */
final class SorafsReputationJsonParser {

  private static final BigInteger U64_MAX = new BigInteger("18446744073709551615");
  private static final Set<String> SNAPSHOT_FIELDS =
      fields(
          "snapshot_id_hex",
          "generated_at_unix",
          "previous_snapshot_id_hex",
          "merkle_root_hex",
          "provider_count",
          "returned_provider_count",
          "limit",
          "truncated_providers",
          "alpha_bps",
          "current_score_weight_bps",
          "weights",
          "providers");
  private static final Set<String> PROVIDER_RESPONSE_FIELDS =
      fields(
          "snapshot_id_hex", "generated_at_unix", "merkle_root_hex", "provider", "proof");
  private static final Set<String> WEIGHTS_RESPONSE_FIELDS =
      fields(
          "snapshot_id_hex",
          "generated_at_unix",
          "alpha_bps",
          "current_score_weight_bps",
          "weights");
  private static final Set<String> WEIGHTS_FIELDS =
      fields(
          "version",
          "por_success_bps",
          "pdp_success_bps",
          "potr_success_bps",
          "latency_bps",
          "dispute_bps",
          "token_violation_bps",
          "repair_breach_bps");
  private static final Set<String> PROVIDER_FIELDS =
      fields(
          "provider_id",
          "score_bps",
          "degradation_flags",
          "raw_metrics",
          "raw_metrics_hash_hex");
  private static final Set<String> PROVIDER_METRICS_FIELDS =
      fields(
          "version",
          "por_success_bps",
          "pdp_success_bps",
          "potr_success_bps",
          "latency_health_bps",
          "dispute_rate_bps",
          "token_violation_rate_bps",
          "repair_breach_rate_bps");
  private static final Set<String> DEGRADATION_FLAG_FIELDS = fields("flag", "value");
  private static final Set<String> PROOF_FIELDS =
      fields("provider_id", "leaf_index", "leaf_count", "siblings_hex");
  private static final Set<String> EVENT_FIELDS =
      fields(
          "version",
          "sequence",
          "snapshot_id_hex",
          "generated_at_unix",
          "merkle_root_hex",
          "provider_count",
          "previous_snapshot_id_hex");
  private static final Set<String> EVENT_PAGE_FIELDS =
      fields("since", "limit", "count", "next_since", "events");
  private static final List<String> DEGRADATION_FLAG_ORDER =
      Collections.unmodifiableList(
          Arrays.asList(
              "reserve_warning",
              "reserve_grace",
              "reserve_delinquent",
              "reserve_default",
              "proof_success_below90",
              "proof_success_below80",
              "active_dispute",
              "slashing_event",
              "low_score"));
  private static final Set<String> DEGRADATION_FLAGS =
      Collections.unmodifiableSet(new LinkedHashSet<>(DEGRADATION_FLAG_ORDER));

  private SorafsReputationJsonParser() {}

  static SorafsReputationModels.SnapshotSummaryV1 parseSnapshot(final byte[] payload) {
    final Map<String, Object> root = rootObject(payload, "SoraFS reputation snapshot");
    exactFields(root, SNAPSHOT_FIELDS, "SoraFS reputation snapshot");
    final List<Object> providerValues =
        list(root.get("providers"), "SoraFS reputation snapshot.providers");
    final List<SorafsReputationModels.ProviderV1> providers = new ArrayList<>();
    for (int index = 0; index < providerValues.size(); index++) {
      final String path = "SoraFS reputation snapshot.providers[" + index + "]";
      providers.add(parseProvider(objectValue(providerValues.get(index), path), path));
    }
    for (int index = 1; index < providers.size(); index++) {
      if (providers.get(index - 1).providerId().compareTo(providers.get(index).providerId()) >= 0) {
        throw new IllegalStateException(
            "SoraFS reputation snapshot.providers must be strictly ordered by provider_id");
      }
    }
    final int providerCount =
        boundedInt(
            root.get("provider_count"),
            "SoraFS reputation snapshot.provider_count",
            1,
            65_536);
    final int returnedProviderCount =
        boundedInt(
            root.get("returned_provider_count"),
            "SoraFS reputation snapshot.returned_provider_count",
            1,
            500);
    final int limit =
        boundedInt(root.get("limit"), "SoraFS reputation snapshot.limit", 1, 500);
    check(
        returnedProviderCount == providers.size(),
        "SoraFS reputation snapshot.returned_provider_count must equal providers.length");
    check(
        returnedProviderCount == Math.min(providerCount, limit),
        "SoraFS reputation snapshot.returned_provider_count must equal min(provider_count, limit)");
    check(
        providerCount >= returnedProviderCount,
        "SoraFS reputation snapshot.provider_count must cover the returned provider prefix");
    final boolean truncated =
        booleanValue(
            root.get("truncated_providers"),
            "SoraFS reputation snapshot.truncated_providers");
    check(
        truncated == (providerCount > returnedProviderCount),
        "SoraFS reputation snapshot.truncated_providers is inconsistent with provider counts");
    final String snapshotIdHex =
        snapshotId(
            root.get("snapshot_id_hex"), "SoraFS reputation snapshot.snapshot_id_hex");
    final String previousSnapshotIdHex =
        optionalSnapshotId(
            root.get("previous_snapshot_id_hex"),
            "SoraFS reputation snapshot.previous_snapshot_id_hex");
    check(
        !snapshotIdHex.equals(previousSnapshotIdHex),
        "SoraFS reputation snapshot.previous_snapshot_id_hex must differ from snapshot_id_hex");
    return new SorafsReputationModels.SnapshotSummaryV1(
        snapshotIdHex,
        positiveU64(
            root.get("generated_at_unix"), "SoraFS reputation snapshot.generated_at_unix"),
        previousSnapshotIdHex,
        digest(root.get("merkle_root_hex"), "SoraFS reputation snapshot.merkle_root_hex"),
        providerCount,
        returnedProviderCount,
        limit,
        truncated,
        exactInt(root.get("alpha_bps"), "SoraFS reputation snapshot.alpha_bps", 8_500),
        exactInt(
            root.get("current_score_weight_bps"),
            "SoraFS reputation snapshot.current_score_weight_bps",
            7_000),
        parseWeights(
            objectValue(root.get("weights"), "SoraFS reputation snapshot.weights"),
            "SoraFS reputation snapshot.weights"),
        providers);
  }

  static SorafsReputationModels.ProviderResponseV1 parseProviderResponse(
      final byte[] payload) {
    final Map<String, Object> root =
        rootObject(payload, "SoraFS reputation provider response");
    exactFields(root, PROVIDER_RESPONSE_FIELDS, "SoraFS reputation provider response");
    final SorafsReputationModels.ProviderV1 provider =
        parseProvider(
            objectValue(
                root.get("provider"), "SoraFS reputation provider response.provider"),
            "SoraFS reputation provider response.provider");
    final SorafsReputationModels.MerkleProofV1 proof =
        parseProof(
            objectValue(root.get("proof"), "SoraFS reputation provider response.proof"),
            "SoraFS reputation provider response.proof");
    check(
        provider.providerId().equals(proof.providerId()),
        "SoraFS reputation provider response proof must reference the returned provider");
    return new SorafsReputationModels.ProviderResponseV1(
        snapshotId(
            root.get("snapshot_id_hex"),
            "SoraFS reputation provider response.snapshot_id_hex"),
        positiveU64(
            root.get("generated_at_unix"),
            "SoraFS reputation provider response.generated_at_unix"),
        digest(
            root.get("merkle_root_hex"),
            "SoraFS reputation provider response.merkle_root_hex"),
        provider,
        proof);
  }

  static SorafsReputationModels.WeightsResponseV1 parseWeightsResponse(
      final byte[] payload) {
    final Map<String, Object> root =
        rootObject(payload, "SoraFS reputation weights response");
    exactFields(root, WEIGHTS_RESPONSE_FIELDS, "SoraFS reputation weights response");
    return new SorafsReputationModels.WeightsResponseV1(
        snapshotId(
            root.get("snapshot_id_hex"),
            "SoraFS reputation weights response.snapshot_id_hex"),
        positiveU64(
            root.get("generated_at_unix"),
            "SoraFS reputation weights response.generated_at_unix"),
        exactInt(
            root.get("alpha_bps"),
            "SoraFS reputation weights response.alpha_bps",
            8_500),
        exactInt(
            root.get("current_score_weight_bps"),
            "SoraFS reputation weights response.current_score_weight_bps",
            7_000),
        parseWeights(
            objectValue(root.get("weights"), "SoraFS reputation weights response.weights"),
            "SoraFS reputation weights response.weights"));
  }

  static SorafsReputationModels.EventsResponseV1 parseEventPage(final byte[] payload) {
    final Map<String, Object> root =
        rootObject(payload, "SoraFS reputation events response");
    exactFields(root, EVENT_PAGE_FIELDS, "SoraFS reputation events response");
    final List<Object> eventValues =
        list(root.get("events"), "SoraFS reputation events response.events");
    final List<SorafsReputationModels.SnapshotEventV1> events = new ArrayList<>();
    for (int index = 0; index < eventValues.size(); index++) {
      final String path = "SoraFS reputation events response.events[" + index + "]";
      events.add(parseEventObject(objectValue(eventValues.get(index), path), path));
    }
    final int count =
        boundedInt(root.get("count"), "SoraFS reputation events response.count", 0, 500);
    check(
        count == events.size(),
        "SoraFS reputation events response.count must equal events.length");
    final int limit =
        boundedInt(root.get("limit"), "SoraFS reputation events response.limit", 1, 500);
    check(
        count <= limit,
        "SoraFS reputation events response.count must not exceed limit");
    final String nextSince =
        optionalPositiveU64(
            root.get("next_since"), "SoraFS reputation events response.next_since");
    final String expectedNext =
        events.isEmpty() ? null : events.get(events.size() - 1).sequence();
    check(
        java.util.Objects.equals(nextSince, expectedNext),
        "SoraFS reputation events response.next_since must equal the last event sequence");
    final String since =
        optionalU64(root.get("since"), "SoraFS reputation events response.since");
    BigInteger previousSequence = new BigInteger(since == null ? "0" : since);
    for (int index = 0; index < events.size(); index++) {
      final SorafsReputationModels.SnapshotEventV1 event = events.get(index);
      final BigInteger sequence = new BigInteger(event.sequence());
      check(
          index == 0
              ? sequence.compareTo(previousSequence) > 0
              : sequence.equals(previousSequence.add(BigInteger.ONE)),
          "SoraFS reputation events response sequences must increase after since and be contiguous within the page");
      previousSequence = sequence;
    }
    for (int index = 1; index < events.size(); index++) {
      final SorafsReputationModels.SnapshotEventV1 previous = events.get(index - 1);
      final SorafsReputationModels.SnapshotEventV1 current = events.get(index);
      check(
          previous.snapshotIdHex().equals(current.previousSnapshotIdHex()),
          "SoraFS reputation events response previous_snapshot_id_hex must link adjacent events");
      check(
          new BigInteger(current.generatedAtUnix())
                  .compareTo(new BigInteger(previous.generatedAtUnix()))
              > 0,
          "SoraFS reputation events response generated_at_unix must strictly increase");
    }
    return new SorafsReputationModels.EventsResponseV1(
        since,
        limit,
        count,
        nextSince,
        events);
  }

  static SorafsReputationModels.SnapshotEventV1 parseEventJson(final String payload) {
    check(
        payload != null
            && !payload.isEmpty()
            && payload.equals(payload.trim())
            && containsNoWhitespace(payload),
        "SoraFS reputation SSE snapshot data must be exact compact JSON");
    return parseEventObject(
        objectValue(JsonParser.parse(payload), "SoraFS reputation SSE snapshot data"),
        "SoraFS reputation SSE snapshot data");
  }

  private static SorafsReputationModels.WeightsV1 parseWeights(
      final Map<String, Object> root, final String path) {
    exactFields(root, WEIGHTS_FIELDS, path);
    final SorafsReputationModels.WeightsV1 weights =
        new SorafsReputationModels.WeightsV1(
            exactInt(root.get("version"), path + ".version", 1),
            basisPoints(root.get("por_success_bps"), path + ".por_success_bps"),
            basisPoints(root.get("pdp_success_bps"), path + ".pdp_success_bps"),
            basisPoints(root.get("potr_success_bps"), path + ".potr_success_bps"),
            basisPoints(root.get("latency_bps"), path + ".latency_bps"),
            basisPoints(root.get("dispute_bps"), path + ".dispute_bps"),
            basisPoints(
                root.get("token_violation_bps"), path + ".token_violation_bps"),
            basisPoints(root.get("repair_breach_bps"), path + ".repair_breach_bps"));
    final int total =
        weights.porSuccessBps()
            + weights.pdpSuccessBps()
            + weights.potrSuccessBps()
            + weights.latencyBps()
            + weights.disputeBps()
            + weights.tokenViolationBps()
            + weights.repairBreachBps();
    check(total == 10_000, path + " basis-point fields must sum to exactly 10000");
    return weights;
  }

  private static SorafsReputationModels.ProviderV1 parseProvider(
      final Map<String, Object> root, final String path) {
    exactFields(root, PROVIDER_FIELDS, path);
    final List<Object> flagValues =
        list(root.get("degradation_flags"), path + ".degradation_flags");
    final List<String> flags = new ArrayList<>();
    for (int index = 0; index < flagValues.size(); index++) {
      final String flagPath = path + ".degradation_flags[" + index + "]";
      final Map<String, Object> flag = objectValue(flagValues.get(index), flagPath);
      exactFields(flag, DEGRADATION_FLAG_FIELDS, flagPath);
      check(flag.get("value") == null, flagPath + ".value must be null");
      final String label = string(flag.get("flag"), flagPath + ".flag");
      check(DEGRADATION_FLAGS.contains(label), flagPath + ".flag is unsupported");
      flags.add(label);
    }
    check(
        flags.size() <= 5 && new LinkedHashSet<>(flags).size() == flags.size(),
        path + ".degradation_flags must be unique and contain at most five entries");
    for (int index = 1; index < flags.size(); index++) {
      check(
          DEGRADATION_FLAG_ORDER.indexOf(flags.get(index - 1))
              < DEGRADATION_FLAG_ORDER.indexOf(flags.get(index)),
          path + ".degradation_flags must use canonical enum order");
    }
    return new SorafsReputationModels.ProviderV1(
        providerId(root.get("provider_id"), path + ".provider_id"),
        boundedInt(root.get("score_bps"), path + ".score_bps", 500, 9_900),
        flags,
        parseProviderMetrics(
            objectValue(root.get("raw_metrics"), path + ".raw_metrics"),
            path + ".raw_metrics"),
        digest(root.get("raw_metrics_hash_hex"), path + ".raw_metrics_hash_hex"));
  }

  private static SorafsReputationModels.ProviderMetricsV1 parseProviderMetrics(
      final Map<String, Object> root, final String path) {
    exactFields(root, PROVIDER_METRICS_FIELDS, path);
    return new SorafsReputationModels.ProviderMetricsV1(
        exactInt(root.get("version"), path + ".version", 1),
        basisPoints(root.get("por_success_bps"), path + ".por_success_bps"),
        basisPoints(root.get("pdp_success_bps"), path + ".pdp_success_bps"),
        basisPoints(root.get("potr_success_bps"), path + ".potr_success_bps"),
        basisPoints(root.get("latency_health_bps"), path + ".latency_health_bps"),
        basisPoints(root.get("dispute_rate_bps"), path + ".dispute_rate_bps"),
        basisPoints(
            root.get("token_violation_rate_bps"), path + ".token_violation_rate_bps"),
        basisPoints(
            root.get("repair_breach_rate_bps"), path + ".repair_breach_rate_bps"));
  }

  private static SorafsReputationModels.MerkleProofV1 parseProof(
      final Map<String, Object> root, final String path) {
    exactFields(root, PROOF_FIELDS, path);
    final int leafIndex =
        boundedInt(root.get("leaf_index"), path + ".leaf_index", 0, 65_535);
    final int leafCount =
        boundedInt(root.get("leaf_count"), path + ".leaf_count", 1, 65_536);
    check(leafIndex < leafCount, path + ".leaf_index must be less than leaf_count");
    final List<Object> siblingValues = list(root.get("siblings_hex"), path + ".siblings_hex");
    final List<String> siblings = new ArrayList<>();
    for (int index = 0; index < siblingValues.size(); index++) {
      siblings.add(digest(siblingValues.get(index), path + ".siblings_hex[" + index + "]"));
    }
    check(
        siblings.size() == merkleDepth(leafCount),
        path + ".siblings_hex must have the exact Merkle depth for leaf_count");
    return new SorafsReputationModels.MerkleProofV1(
        providerId(root.get("provider_id"), path + ".provider_id"),
        leafIndex,
        leafCount,
        siblings);
  }

  private static SorafsReputationModels.SnapshotEventV1 parseEventObject(
      final Map<String, Object> root, final String path) {
    exactFields(root, EVENT_FIELDS, path);
    final String snapshotIdHex =
        snapshotId(root.get("snapshot_id_hex"), path + ".snapshot_id_hex");
    final String previousSnapshotIdHex =
        optionalSnapshotId(
            root.get("previous_snapshot_id_hex"), path + ".previous_snapshot_id_hex");
    check(
        !snapshotIdHex.equals(previousSnapshotIdHex),
        path + ".previous_snapshot_id_hex must differ from snapshot_id_hex");
    return new SorafsReputationModels.SnapshotEventV1(
        exactInt(root.get("version"), path + ".version", 1),
        positiveU64(root.get("sequence"), path + ".sequence"),
        snapshotIdHex,
        positiveU64(root.get("generated_at_unix"), path + ".generated_at_unix"),
        digest(root.get("merkle_root_hex"), path + ".merkle_root_hex"),
        boundedInt(root.get("provider_count"), path + ".provider_count", 1, 65_536),
        previousSnapshotIdHex);
  }

  private static int merkleDepth(final int leafCount) {
    int width = leafCount;
    int depth = 0;
    while (width > 1) {
      width = (width + 1) / 2;
      depth++;
    }
    return depth;
  }

  private static Map<String, Object> rootObject(
      final byte[] payload, final String context) {
    check(payload != null && payload.length > 0, context + " returned an empty payload");
    final String json = new String(payload, StandardCharsets.UTF_8);
    check(!json.isEmpty() && json.equals(json.trim()), context + " must be exact JSON");
    return objectValue(JsonParser.parse(json), context);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> objectValue(final Object value, final String path) {
    check(value instanceof Map<?, ?>, path + " must be a JSON object");
    final Map<?, ?> raw = (Map<?, ?>) value;
    for (final Object key : raw.keySet()) {
      check(key instanceof String, path + " must use string keys");
    }
    return (Map<String, Object>) raw;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> list(final Object value, final String path) {
    check(value instanceof List<?>, path + " must be a JSON array");
    return (List<Object>) value;
  }

  private static void exactFields(
      final Map<String, Object> root, final Set<String> expected, final String path) {
    if (!root.keySet().equals(expected)) {
      final Set<String> missing = new LinkedHashSet<>(expected);
      missing.removeAll(root.keySet());
      final Set<String> extra = new LinkedHashSet<>(root.keySet());
      extra.removeAll(expected);
      throw new IllegalStateException(
          path + " fields are not canonical; missing=" + missing + " extra=" + extra);
    }
  }

  private static String string(final Object value, final String path) {
    check(value instanceof String, path + " must be an exact non-empty string");
    final String text = (String) value;
    check(!text.isEmpty() && text.equals(text.trim()), path + " must be an exact non-empty string");
    return text;
  }

  private static String providerId(final Object value, final String path) {
    final String provider = string(value, path);
    check(
        provider.length() <= 256
            && !provider.equals(".")
            && !provider.equals("..")
            && allProviderCharacters(provider),
        path
            + " must be 1..256 ASCII characters from [A-Za-z0-9_.:-] and not be a dot segment");
    return provider;
  }

  private static boolean allProviderCharacters(final String value) {
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (!((character >= 'A' && character <= 'Z')
          || (character >= 'a' && character <= 'z')
          || (character >= '0' && character <= '9')
          || character == '_'
          || character == '.'
          || character == ':'
          || character == '-')) {
        return false;
      }
    }
    return true;
  }

  private static String snapshotId(final Object value, final String path) {
    check(value instanceof String, path + " must be exactly 32 lowercase hexadecimal characters");
    final String literal = (String) value;
    check(
        literal.length() == 32 && allLowerHex(literal),
        path + " must be exactly 32 lowercase hexadecimal characters");
    check(!isAllZero(literal), path + " must be nonzero");
    return literal;
  }

  private static String optionalSnapshotId(final Object value, final String path) {
    return value == null ? null : snapshotId(value, path);
  }

  private static String digest(final Object value, final String path) {
    check(value instanceof String, path + " must be exactly 64 lowercase hexadecimal characters");
    final String literal = (String) value;
    check(
        literal.length() == 64 && allLowerHex(literal),
        path + " must be exactly 64 lowercase hexadecimal characters");
    return literal;
  }

  private static boolean allLowerHex(final String value) {
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (!((character >= '0' && character <= '9')
          || (character >= 'a' && character <= 'f'))) {
        return false;
      }
    }
    return true;
  }

  private static boolean isAllZero(final String value) {
    for (int index = 0; index < value.length(); index++) {
      if (value.charAt(index) != '0') {
        return false;
      }
    }
    return true;
  }

  private static boolean containsNoWhitespace(final String value) {
    for (int index = 0; index < value.length(); index++) {
      if (Character.isWhitespace(value.charAt(index))) {
        return false;
      }
    }
    return true;
  }

  private static boolean booleanValue(final Object value, final String path) {
    check(value instanceof Boolean, path + " must be a boolean");
    return (Boolean) value;
  }

  private static int basisPoints(final Object value, final String path) {
    return boundedInt(value, path, 0, 10_000);
  }

  private static int exactInt(final Object value, final String path, final int expected) {
    return boundedInt(value, path, expected, expected);
  }

  private static int boundedInt(
      final Object value, final String path, final int minimum, final int maximum) {
    final BigInteger parsed = new BigInteger(canonicalU64(value, path, minimum == 0));
    check(
        parsed.compareTo(BigInteger.valueOf(minimum)) >= 0
            && parsed.compareTo(BigInteger.valueOf(maximum)) <= 0,
        path + " must be between " + minimum + " and " + maximum);
    return parsed.intValue();
  }

  private static String optionalU64(final Object value, final String path) {
    return value == null ? null : canonicalU64(value, path, true);
  }

  private static String optionalPositiveU64(final Object value, final String path) {
    return value == null ? null : canonicalU64(value, path, false);
  }

  private static String positiveU64(final Object value, final String path) {
    return canonicalU64(value, path, false);
  }

  private static String canonicalU64(
      final Object value, final String path, final boolean allowZero) {
    final String literal;
    if (value instanceof Long || value instanceof Integer || value instanceof BigInteger) {
      literal = value.toString();
    } else {
      throw new IllegalStateException(path + " must be a canonical unsigned integer");
    }
    check(isCanonicalUnsignedDecimal(literal), path + " must be a canonical unsigned integer");
    final BigInteger parsed = new BigInteger(literal);
    check(
        parsed.signum() >= 0 && parsed.compareTo(U64_MAX) <= 0,
        path + " must fit canonical u64");
    check(allowZero || parsed.signum() > 0, path + " must be positive");
    return literal;
  }

  private static boolean isCanonicalUnsignedDecimal(final String value) {
    if ("0".equals(value)) {
      return true;
    }
    if (value.isEmpty() || value.charAt(0) < '1' || value.charAt(0) > '9') {
      return false;
    }
    for (int index = 1; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (character < '0' || character > '9') {
        return false;
      }
    }
    return true;
  }

  private static Set<String> fields(final String... names) {
    return Collections.unmodifiableSet(new LinkedHashSet<>(Arrays.asList(names)));
  }

  private static void check(final boolean condition, final String message) {
    if (!condition) {
      throw new IllegalStateException(message);
    }
  }
}
