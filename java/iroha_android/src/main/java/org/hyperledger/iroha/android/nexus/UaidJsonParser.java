package org.hyperledger.iroha.android.nexus;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonNumbers;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.numeric.NumericV1;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse.UaidManifestLifecycle;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse.UaidManifestRecord;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse.UaidManifestRevocation;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse.UaidManifestStatus;
import org.hyperledger.iroha.android.nexus.UaidPortfolioResponse.UaidPortfolioAccount;
import org.hyperledger.iroha.android.nexus.UaidPortfolioResponse.UaidPortfolioAsset;
import org.hyperledger.iroha.android.nexus.UaidPortfolioResponse.UaidPortfolioDataspace;
import org.hyperledger.iroha.android.nexus.UaidPortfolioResponse.UaidPortfolioTotals;

/** Exact first-release JSON parser for UAID responses. */
public final class UaidJsonParser {

  private UaidJsonParser() {}

  public static UaidPortfolioResponse parsePortfolio(final byte[] payload) {
    final Map<String, Object> root =
        exactObject(
            parse(payload),
            "uaid portfolio",
            Set.of("uaid", "totals", "dataspaces"));
    final String uaid =
        UaidLiteral.canonicalize(
            exactString(root.get("uaid"), "uaid portfolio.uaid"),
            "uaid portfolio.uaid");
    final Map<String, Object> totalsObject =
        exactObject(
            root.get("totals"),
            "uaid portfolio.totals",
            Set.of("accounts", "positions"));
    final UaidPortfolioTotals totals =
        new UaidPortfolioTotals(
            asUnsignedLong(totalsObject.get("accounts"), "uaid portfolio.totals.accounts"),
            asUnsignedLong(totalsObject.get("positions"), "uaid portfolio.totals.positions"));
    final List<Object> dataspaceItems =
        requiredArray(root.get("dataspaces"), "uaid portfolio.dataspaces");
    final List<UaidPortfolioDataspace> dataspaces = new ArrayList<>(dataspaceItems.size());
    for (int i = 0; i < dataspaceItems.size(); i++) {
      dataspaces.add(parsePortfolioDataspace(dataspaceItems.get(i), i));
    }
    return new UaidPortfolioResponse(uaid, totals, dataspaces);
  }

  public static UaidBindingsResponse parseBindings(final byte[] payload) {
    final Map<String, Object> root =
        exactObject(
            parse(payload), "uaid bindings", Set.of("uaid", "dataspaces"));
    final String uaid =
        UaidLiteral.canonicalize(
            exactString(root.get("uaid"), "uaid bindings.uaid"),
            "uaid bindings.uaid");
    final List<Object> dataspaceItems =
        requiredArray(root.get("dataspaces"), "uaid bindings.dataspaces");
    final List<UaidBindingsResponse.UaidBindingsDataspace> dataspaces =
        new ArrayList<>(dataspaceItems.size());
    for (int i = 0; i < dataspaceItems.size(); i++) {
      final String path = "uaid bindings.dataspaces[" + i + "]";
      final Map<String, Object> entry =
          exactObject(
              dataspaceItems.get(i),
              path,
              Set.of("dataspace_id", "dataspace_alias", "accounts"));
      dataspaces.add(
          new UaidBindingsResponse.UaidBindingsDataspace(
              asUnsignedLong(entry.get("dataspace_id"), path + ".dataspace_id"),
              nullableExactString(entry.get("dataspace_alias"), path + ".dataspace_alias"),
              exactStringList(entry.get("accounts"), path + ".accounts")));
    }
    return new UaidBindingsResponse(uaid, dataspaces);
  }

  public static UaidManifestsResponse parseManifests(final byte[] payload) {
    final Map<String, Object> root =
        exactObject(
            parse(payload),
            "uaid manifests",
            Set.of("uaid", "total", "has_more", "count_mode", "manifests"));
    final String uaid =
        UaidLiteral.canonicalize(
            exactString(root.get("uaid"), "uaid manifests.uaid"),
            "uaid manifests.uaid");
    final long total = asUnsignedLong(root.get("total"), "uaid manifests.total");
    final boolean hasMore = asBoolean(root.get("has_more"), "uaid manifests.has_more");
    final UaidManifestCountMode countMode =
        parseCountMode(exactString(root.get("count_mode"), "uaid manifests.count_mode"));
    final List<Object> manifestItems =
        requiredArray(root.get("manifests"), "uaid manifests.manifests");
    final List<UaidManifestRecord> manifests = new ArrayList<>(manifestItems.size());
    for (int i = 0; i < manifestItems.size(); i++) {
      manifests.add(parseManifestRecord(manifestItems.get(i), i, uaid));
    }
    return new UaidManifestsResponse(uaid, total, hasMore, countMode, manifests);
  }

  private static UaidPortfolioDataspace parsePortfolioDataspace(
      final Object value, final int index) {
    final String path = "uaid portfolio.dataspaces[" + index + "]";
    final Map<String, Object> entry =
        exactObject(
            value,
            path,
            Set.of("dataspace_id", "dataspace_alias", "accounts"));
    final List<Object> accountItems = requiredArray(entry.get("accounts"), path + ".accounts");
    final List<UaidPortfolioAccount> accounts = new ArrayList<>(accountItems.size());
    for (int i = 0; i < accountItems.size(); i++) {
      accounts.add(parsePortfolioAccount(accountItems.get(i), path + ".accounts[" + i + "]"));
    }
    return new UaidPortfolioDataspace(
        asUnsignedLong(entry.get("dataspace_id"), path + ".dataspace_id"),
        nullableExactString(entry.get("dataspace_alias"), path + ".dataspace_alias"),
        accounts);
  }

  private static UaidPortfolioAccount parsePortfolioAccount(
      final Object value, final String path) {
    final Map<String, Object> account =
        exactObject(value, path, Set.of("account_id", "label", "assets"));
    final List<Object> assetItems = requiredArray(account.get("assets"), path + ".assets");
    final List<UaidPortfolioAsset> assets = new ArrayList<>(assetItems.size());
    for (int i = 0; i < assetItems.size(); i++) {
      final String assetPath = path + ".assets[" + i + "]";
      final Map<String, Object> asset =
          exactObject(
              assetItems.get(i),
              assetPath,
              Set.of("asset_id", "asset_definition_id", "quantity"));
      assets.add(
          new UaidPortfolioAsset(
              exactString(asset.get("asset_id"), assetPath + ".asset_id"),
              exactString(
                  asset.get("asset_definition_id"), assetPath + ".asset_definition_id"),
              NumericV1.decodeQuantityJsonValue(asset.get("quantity")).toString()));
    }
    return new UaidPortfolioAccount(
        exactString(account.get("account_id"), path + ".account_id"),
        nullableExactString(account.get("label"), path + ".label"),
        assets);
  }

  private static UaidManifestRecord parseManifestRecord(
      final Object value, final int index, final String responseUaid) {
    final String path = "uaid manifests.manifests[" + index + "]";
    final Map<String, Object> entry =
        exactObject(
            value,
            path,
            Set.of(
                "dataspace_id",
                "dataspace_alias",
                "manifest_hash",
                "status",
                "lifecycle",
                "accounts",
                "manifest"));
    final long dataspaceId =
        asUnsignedLong(entry.get("dataspace_id"), path + ".dataspace_id");
    final String manifestHash =
        exactString(entry.get("manifest_hash"), path + ".manifest_hash");
    if (!manifestHash.matches("[0-9a-f]{64}")) {
      throw new IllegalStateException(
          path + ".manifest_hash must be exactly 64 lowercase hexadecimal characters");
    }
    final Map<String, Object> lifecycleMap =
        exactObject(
            entry.get("lifecycle"),
            path + ".lifecycle",
            Set.of("activated_epoch", "expired_epoch", "revocation"));
    final Object revocationValue = lifecycleMap.get("revocation");
    final UaidManifestRevocation revocation;
    if (revocationValue == null) {
      revocation = null;
    } else {
      final String revocationPath = path + ".lifecycle.revocation";
      final Map<String, Object> revocationMap =
          exactObject(revocationValue, revocationPath, Set.of("epoch", "reason"));
      revocation =
          new UaidManifestRevocation(
              asUnsignedLong(revocationMap.get("epoch"), revocationPath + ".epoch"),
              nullableString(revocationMap.get("reason"), revocationPath + ".reason"));
    }
    final Map<String, Object> manifest =
        validateManifest(entry.get("manifest"), path + ".manifest", responseUaid, dataspaceId);
    return new UaidManifestRecord(
        dataspaceId,
        nullableExactString(entry.get("dataspace_alias"), path + ".dataspace_alias"),
        manifestHash,
        parseManifestStatus(exactString(entry.get("status"), path + ".status")),
        new UaidManifestLifecycle(
            nullableUnsignedLong(
                lifecycleMap.get("activated_epoch"), path + ".lifecycle.activated_epoch"),
            nullableUnsignedLong(
                lifecycleMap.get("expired_epoch"), path + ".lifecycle.expired_epoch"),
            revocation),
        exactStringList(entry.get("accounts"), path + ".accounts"),
        JsonEncoder.encode(manifest));
  }

  private static Map<String, Object> validateManifest(
      final Object value,
      final String path,
      final String responseUaid,
      final long responseDataspace) {
    final Map<String, Object> manifest =
        exactObject(
            value,
            path,
            Set.of("version", "uaid", "dataspace", "issued_ms", "activation_epoch", "entries"),
            Set.of("expiry_epoch"));
    if (asUnsignedLong(manifest.get("version"), path + ".version") != 1L) {
      throw new IllegalStateException(path + ".version must be the numeric value 1");
    }
    final String manifestUaid =
        UaidLiteral.canonicalize(exactString(manifest.get("uaid"), path + ".uaid"), path + ".uaid");
    if (!manifestUaid.equals(responseUaid)) {
      throw new IllegalStateException(path + ".uaid must match the response UAID");
    }
    if (asUnsignedLong(manifest.get("dataspace"), path + ".dataspace") != responseDataspace) {
      throw new IllegalStateException(
          path + ".dataspace must match the manifest record dataspace_id");
    }
    asUnsignedLong(manifest.get("issued_ms"), path + ".issued_ms");
    asUnsignedLong(manifest.get("activation_epoch"), path + ".activation_epoch");
    if (manifest.containsKey("expiry_epoch")) {
      if (manifest.get("expiry_epoch") == null) {
        throw new IllegalStateException(path + ".expiry_epoch must be omitted instead of null");
      }
      asUnsignedLong(manifest.get("expiry_epoch"), path + ".expiry_epoch");
    }
    final List<Object> entries = requiredArray(manifest.get("entries"), path + ".entries");
    for (int i = 0; i < entries.size(); i++) {
      validateManifestEntry(entries.get(i), path + ".entries[" + i + "]");
    }
    return manifest;
  }

  private static void validateManifestEntry(final Object value, final String path) {
    final Map<String, Object> entry =
        exactObject(value, path, Set.of("scope", "effect"), Set.of("notes"));
    if (entry.containsKey("notes")) {
      if (entry.get("notes") == null) {
        throw new IllegalStateException(path + ".notes must be omitted instead of null");
      }
      asString(entry.get("notes"), path + ".notes");
    }
    validateManifestScope(entry.get("scope"), path + ".scope");
    validateManifestEffect(entry.get("effect"), path + ".effect");
  }

  private static void validateManifestScope(final Object value, final String path) {
    final Map<String, Object> scope =
        exactObject(
            value,
            path,
            Set.of(),
            Set.of("dataspace", "program", "method", "asset", "role"));
    for (final Map.Entry<String, Object> field : scope.entrySet()) {
      if (field.getValue() == null) {
        throw new IllegalStateException(
            path + "." + field.getKey() + " must be omitted instead of null");
      }
      switch (field.getKey()) {
        case "dataspace" -> asUnsignedLong(field.getValue(), path + ".dataspace");
        case "program", "method", "asset" ->
            exactString(field.getValue(), path + "." + field.getKey());
        case "role" -> {
          final String role = exactString(field.getValue(), path + ".role");
          if (!role.equals("Initiator") && !role.equals("Participant")) {
            throw new IllegalStateException(path + ".role must be Initiator or Participant");
          }
        }
        default -> throw new IllegalStateException("unreachable manifest scope field");
      }
    }
  }

  private static void validateManifestEffect(final Object value, final String path) {
    final Map<String, Object> effect = expectObject(value, path);
    if (effect.size() != 1
        || (!effect.containsKey("Allow") && !effect.containsKey("Deny"))) {
      throw new IllegalStateException(path + " must contain exactly one of Allow or Deny");
    }
    if (effect.containsKey("Allow")) {
      final Map<String, Object> allowance =
          exactObject(
              effect.get("Allow"), path + ".Allow", Set.of("window"), Set.of("max_amount"));
      final String window = exactString(allowance.get("window"), path + ".Allow.window");
      if (!Set.of("PerSlot", "PerMinute", "PerDay").contains(window)) {
        throw new IllegalStateException(
            path + ".Allow.window is not a canonical allowance window");
      }
      if (allowance.containsKey("max_amount")) {
        if (allowance.get("max_amount") == null) {
          throw new IllegalStateException(
              path + ".Allow.max_amount must be omitted instead of null");
        }
        NumericV1.decodeQuantityJsonValue(allowance.get("max_amount"));
      }
    } else {
      final Map<String, Object> denial =
          exactObject(effect.get("Deny"), path + ".Deny", Set.of(), Set.of("reason"));
      if (denial.containsKey("reason")) {
        if (denial.get("reason") == null) {
          throw new IllegalStateException(path + ".Deny.reason must be omitted instead of null");
        }
        asString(denial.get("reason"), path + ".Deny.reason");
      }
    }
  }

  private static Object parse(final byte[] payload) {
    if (payload == null || payload.length == 0) {
      throw new IllegalStateException("UAID endpoint returned an empty payload");
    }
    final String json = new String(payload, StandardCharsets.UTF_8).trim();
    if (json.isEmpty()) {
      throw new IllegalStateException("UAID endpoint returned a blank payload");
    }
    final Object parsed = JsonParser.parse(json);
    if (parsed == null) {
      throw new IllegalStateException("UAID endpoint returned null JSON");
    }
    return parsed;
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> expectObject(final Object value, final String path) {
    if (!(value instanceof Map<?, ?> map)) {
      throw new IllegalStateException(path + " must be a JSON object");
    }
    if (!map.keySet().stream().allMatch(String.class::isInstance)) {
      throw new IllegalStateException(path + " field names must be strings");
    }
    return (Map<String, Object>) map;
  }

  private static Map<String, Object> exactObject(
      final Object value, final String path, final Set<String> required) {
    return exactObject(value, path, required, Set.of());
  }

  private static Map<String, Object> exactObject(
      final Object value,
      final String path,
      final Set<String> required,
      final Set<String> optional) {
    final Map<String, Object> object = expectObject(value, path);
    final Set<String> allowed = new HashSet<>(required);
    allowed.addAll(optional);
    final Set<String> unknown = new HashSet<>(object.keySet());
    unknown.removeAll(allowed);
    if (!unknown.isEmpty()) {
      throw new IllegalStateException(path + " contains unknown fields: " + unknown);
    }
    final Set<String> missing = new HashSet<>(required);
    missing.removeAll(object.keySet());
    if (!missing.isEmpty()) {
      throw new IllegalStateException(path + " is missing required fields: " + missing);
    }
    return object;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> requiredArray(final Object value, final String path) {
    if (!(value instanceof List<?> list)) {
      throw new IllegalStateException(path + " must be a JSON array");
    }
    return (List<Object>) list;
  }

  private static String asString(final Object value, final String path) {
    if (!(value instanceof String string)) {
      throw new IllegalStateException(path + " must be a string");
    }
    return string;
  }

  private static String exactString(final Object value, final String path) {
    final String string = asString(value, path);
    if (string.isEmpty()) {
      throw new IllegalStateException(path + " must not be empty");
    }
    if (!string.trim().equals(string)) {
      throw new IllegalStateException(path + " must not contain surrounding whitespace");
    }
    return string;
  }

  private static String nullableString(final Object value, final String path) {
    return value == null ? null : asString(value, path);
  }

  private static String nullableExactString(final Object value, final String path) {
    return value == null ? null : exactString(value, path);
  }

  private static long asUnsignedLong(final Object value, final String path) {
    final long parsed = JsonNumbers.asLong(value, path);
    if (parsed < 0L) {
      throw new IllegalStateException(path + " must be an unsigned integer");
    }
    return parsed;
  }

  private static Long nullableUnsignedLong(final Object value, final String path) {
    return value == null ? null : asUnsignedLong(value, path);
  }

  private static boolean asBoolean(final Object value, final String path) {
    if (!(value instanceof Boolean bool)) {
      throw new IllegalStateException(path + " must be a boolean");
    }
    return bool;
  }

  private static List<String> exactStringList(final Object value, final String path) {
    final List<Object> items = requiredArray(value, path);
    final List<String> strings = new ArrayList<>(items.size());
    for (int i = 0; i < items.size(); i++) {
      strings.add(exactString(items.get(i), path + "[" + i + "]"));
    }
    return List.copyOf(strings);
  }

  private static UaidManifestCountMode parseCountMode(final String value) {
    return switch (value) {
      case "bounded" -> UaidManifestCountMode.BOUNDED;
      case "exact" -> UaidManifestCountMode.EXACT;
      default -> throw new IllegalStateException("Unsupported manifest count_mode: " + value);
    };
  }

  private static UaidManifestStatus parseManifestStatus(final String value) {
    return switch (value) {
      case "Pending" -> UaidManifestStatus.PENDING;
      case "Active" -> UaidManifestStatus.ACTIVE;
      case "Expired" -> UaidManifestStatus.EXPIRED;
      case "Revoked" -> UaidManifestStatus.REVOKED;
      default -> throw new IllegalStateException("Unsupported manifest status: " + value);
    };
  }
}
