package org.hyperledger.iroha.android.nexus;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.address.AssetIdLiteral;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse.UaidManifestLifecycle;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse.UaidManifestRecord;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse.UaidManifestRevocation;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse.UaidManifestStatus;
import org.hyperledger.iroha.android.nexus.UaidPortfolioResponse.UaidPortfolioAccount;
import org.hyperledger.iroha.android.nexus.UaidPortfolioResponse.UaidPortfolioAsset;
import org.hyperledger.iroha.android.nexus.UaidPortfolioResponse.UaidPortfolioDataspace;
import org.hyperledger.iroha.android.nexus.UaidPortfolioResponse.UaidPortfolioTotals;

/** Minimal JSON parser for UAID responses. */
public final class UaidJsonParser {

  private UaidJsonParser() {}

  public static UaidPortfolioResponse parsePortfolio(final byte[] payload) {
    final Map<String, Object> root = expectObject(parse(payload), "uaid portfolio");
    final String uaid =
        UaidLiteral.canonicalize(asString(root.get("uaid"), "uaid portfolio.uaid"), "uaid");
    final Map<String, Object> totalsObj =
        asObjectOrEmpty(root.get("totals"), "uaid portfolio.totals");
    final long accounts = asLong(totalsObj.getOrDefault("accounts", 0L), "uaid portfolio.totals.accounts");
    final long positions =
        asLong(totalsObj.getOrDefault("positions", 0L), "uaid portfolio.totals.positions");
    final UaidPortfolioTotals totals = new UaidPortfolioTotals(accounts, positions);

    final List<Object> dataspaceItems =
        asArrayOrEmpty(root.get("dataspaces"), "uaid portfolio.dataspaces");
    final List<UaidPortfolioDataspace> dataspaces = new ArrayList<>(dataspaceItems.size());
    for (int i = 0; i < dataspaceItems.size(); i++) {
      final Map<String, Object> entry =
          expectObject(dataspaceItems.get(i), "uaid portfolio.dataspaces[" + i + "]");
      final long dataspaceId =
          asLong(entry.get("dataspace_id"), "uaid portfolio.dataspaces[" + i + "].dataspace_id");
      final String dataspaceAlias =
          asOptionalString(entry.get("dataspace_alias"));
      final List<Object> accountItems =
          asArrayOrEmpty(entry.get("accounts"), "uaid portfolio.dataspaces[" + i + "].accounts");
      final List<UaidPortfolioAccount> accountsList = new ArrayList<>(accountItems.size());
      for (int j = 0; j < accountItems.size(); j++) {
        final Map<String, Object> account =
            expectObject(
                accountItems.get(j),
                "uaid portfolio.dataspaces[" + i + "].accounts[" + j + "]");
        final String accountId =
            AccountIdLiteral.extractI105Address(
                asString(
                    account.get("account_id"),
                    "uaid portfolio.dataspaces[" + i + "].accounts[" + j + "].account_id"));
        final String label = asOptionalString(account.get("label"));
        final List<Object> assetItems =
            asArrayOrEmpty(
                account.get("assets"),
                "uaid portfolio.dataspaces[" + i + "].accounts[" + j + "].assets");
        final List<UaidPortfolioAsset> assets = new ArrayList<>(assetItems.size());
        for (int k = 0; k < assetItems.size(); k++) {
          final Map<String, Object> asset =
              expectObject(
                  assetItems.get(k),
                  "uaid portfolio.dataspaces[" + i + "].accounts[" + j + "].assets[" + k + "]");
          final String assetId =
              AssetIdLiteral.normalizeEncoded(
                  asString(
                      asset.get("asset_id"),
                      "uaid portfolio.dataspaces[" + i + "].accounts[" + j + "].assets[" + k + "].asset_id"),
                  "uaid portfolio.dataspaces[" + i + "].accounts[" + j + "].assets[" + k + "].asset_id");
          assets.add(
              new UaidPortfolioAsset(
                  assetId,
                      asString(
                          asset.get("asset_definition_id"),
                          "uaid portfolio.dataspaces[" + i + "].accounts[" + j + "].assets[" + k + "].asset_definition_id"),
                  asString(
                      asset.get("quantity"),
                      "uaid portfolio.dataspaces[" + i + "].accounts[" + j + "].assets[" + k + "].quantity")));
        }
        accountsList.add(new UaidPortfolioAccount(accountId, label, assets));
      }
      dataspaces.add(new UaidPortfolioDataspace(dataspaceId, dataspaceAlias, accountsList));
    }

    return new UaidPortfolioResponse(uaid, totals, dataspaces);
  }

  public static UaidBindingsResponse parseBindings(final byte[] payload) {
    final Map<String, Object> root = expectObject(parse(payload), "uaid bindings");
    final String uaid =
        UaidLiteral.canonicalize(asString(root.get("uaid"), "uaid bindings.uaid"), "uaid");
    final List<Object> dataspaceItems =
        asArrayOrEmpty(root.get("dataspaces"), "uaid bindings.dataspaces");
    final List<UaidBindingsResponse.UaidBindingsDataspace> dataspaces =
        new ArrayList<>(dataspaceItems.size());
    for (int i = 0; i < dataspaceItems.size(); i++) {
      final Map<String, Object> entry =
          expectObject(dataspaceItems.get(i), "uaid bindings.dataspaces[" + i + "]");
      final long dataspaceId =
          asLong(entry.get("dataspace_id"), "uaid bindings.dataspaces[" + i + "].dataspace_id");
      final String dataspaceAlias = asOptionalString(entry.get("dataspace_alias"));
      final List<String> accounts =
          asStringList(
              entry.get("accounts"), "uaid bindings.dataspaces[" + i + "].accounts");
      final List<String> normalizedAccounts = new ArrayList<>(accounts.size());
      for (final String accountId : accounts) {
        normalizedAccounts.add(AccountIdLiteral.extractI105Address(accountId));
      }
      dataspaces.add(
          new UaidBindingsResponse.UaidBindingsDataspace(
              dataspaceId, dataspaceAlias, List.copyOf(normalizedAccounts)));
    }
    return new UaidBindingsResponse(uaid, dataspaces);
  }

  public static UaidManifestsResponse parseManifests(final byte[] payload) {
    final Map<String, Object> root = expectObject(parse(payload), "uaid manifests");
    final String uaid =
        UaidLiteral.canonicalize(asString(root.get("uaid"), "uaid manifests.uaid"), "uaid");
    final long total = asLong(root.getOrDefault("total", 0L), "uaid manifests.total");
    final List<Object> manifestItems =
        asArrayOrEmpty(root.get("manifests"), "uaid manifests.manifests");
    final List<UaidManifestRecord> manifests = new ArrayList<>(manifestItems.size());
    for (int i = 0; i < manifestItems.size(); i++) {
      final Map<String, Object> entry =
          expectObject(manifestItems.get(i), "uaid manifests.manifests[" + i + "]");
      final long dataspaceId =
          asLong(entry.get("dataspace_id"), "uaid manifests.manifests[" + i + "].dataspace_id");
      final String alias = asOptionalString(entry.get("dataspace_alias"));
      final String manifestHash =
          asString(entry.get("manifest_hash"), "uaid manifests.manifests[" + i + "].manifest_hash");
      final UaidManifestStatus status =
          parseManifestStatus(
              asString(entry.get("status"), "uaid manifests.manifests[" + i + "].status"));
      final Map<String, Object> lifecycleMap =
          asObjectOrEmpty(entry.get("lifecycle"), "uaid manifests.manifests[" + i + "].lifecycle");
      final Long activatedEpoch =
          asOptionalLong(
              lifecycleMap.get("activated_epoch"),
              "uaid manifests.manifests[" + i + "].lifecycle.activated_epoch");
      final Long expiredEpoch =
          asOptionalLong(
              lifecycleMap.get("expired_epoch"),
              "uaid manifests.manifests[" + i + "].lifecycle.expired_epoch");
      final Map<String, Object> revocationMap =
          asObjectOrNull(
              lifecycleMap.get("revocation"),
              "uaid manifests.manifests[" + i + "].lifecycle.revocation");
      final UaidManifestRevocation revocation =
          revocationMap == null
              ? null
              : new UaidManifestRevocation(
                  asLong(
                      revocationMap.get("epoch"),
                      "uaid manifests.manifests[" + i + "].lifecycle.revocation.epoch"),
                  asOptionalString(revocationMap.get("reason")));
      final UaidManifestLifecycle lifecycle =
          new UaidManifestLifecycle(activatedEpoch, expiredEpoch, revocation);
      final List<String> accounts =
          asStringList(
              entry.get("accounts"), "uaid manifests.manifests[" + i + "].accounts");
      final List<String> normalizedAccounts = new ArrayList<>(accounts.size());
      for (final String accountId : accounts) {
        normalizedAccounts.add(AccountIdLiteral.extractI105Address(accountId));
      }
      final String manifestJson =
          JsonEncoder.encode(
              expectObject(entry.get("manifest"), "uaid manifests.manifests[" + i + "].manifest"));
      manifests.add(
          new UaidManifestRecord(
              dataspaceId,
              alias,
              manifestHash,
              status,
              lifecycle,
              List.copyOf(normalizedAccounts),
              manifestJson));
    }
    return new UaidManifestsResponse(uaid, total, manifests);
  }

  private static Object parse(final byte[] payload) {
    if (payload == null || payload.length == 0) {
      throw new IllegalStateException("UAID endpoint returned an empty payload");
    }
    final String json = new String(payload, StandardCharsets.UTF_8).trim();
    if (json.isEmpty()) {
      throw new IllegalStateException("UAID endpoint returned a blank payload");
    }
    return JsonParser.parse(json);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> expectObject(final Object value, final String path) {
    if (!(value instanceof Map)) {
      throw new IllegalStateException(path + " must be a JSON object");
    }
    return (Map<String, Object>) value;
  }

  private static Map<String, Object> asObjectOrEmpty(final Object value, final String path) {
    if (value == null) {
      return Map.of();
    }
    return expectObject(value, path);
  }

  private static Map<String, Object> asObjectOrNull(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    return expectObject(value, path);
  }

  @SuppressWarnings("unchecked")
  private static List<Object> asArrayOrEmpty(final Object value, final String path) {
    if (value == null) {
      return List.of();
    }
    if (!(value instanceof List<?> list)) {
      throw new IllegalStateException(path + " must be a JSON array");
    }
    return (List<Object>) list;
  }

  private static String asString(final Object value, final String path) {
    if (value == null) {
      throw new IllegalStateException(path + " is missing");
    }
    if (value instanceof String string) {
      return string;
    }
    return String.valueOf(value);
  }

  private static String asOptionalString(final Object value) {
    if (value == null) {
      return null;
    }
    return value instanceof String string ? string : String.valueOf(value);
  }

  private static long asLong(final Object value, final String path) {
    if (!(value instanceof Number number)) {
      throw new IllegalStateException(path + " is not a number");
    }
    if (number instanceof Float || number instanceof Double) {
      throw new IllegalStateException(path + " must be an integer");
    }
    return number.longValue();
  }

  private static Long asOptionalLong(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    return asLong(value, path);
  }

  private static List<String> asStringList(final Object value, final String path) {
    final List<Object> items = asArrayOrEmpty(value, path);
    final List<String> strings = new ArrayList<>(items.size());
    for (int i = 0; i < items.size(); i++) {
      final Object entry = items.get(i);
      if (entry == null) {
        continue;
      }
      strings.add(entry instanceof String string ? string : String.valueOf(entry));
    }
    return List.copyOf(strings);
  }

  private static UaidManifestStatus parseManifestStatus(final String value) {
    Objects.requireNonNull(value, "status");
    return switch (value) {
      case "Pending" -> UaidManifestStatus.PENDING;
      case "Active" -> UaidManifestStatus.ACTIVE;
      case "Expired" -> UaidManifestStatus.EXPIRED;
      case "Revoked" -> UaidManifestStatus.REVOKED;
      default -> throw new IllegalStateException("Unsupported manifest status: " + value);
    };
  }
}
