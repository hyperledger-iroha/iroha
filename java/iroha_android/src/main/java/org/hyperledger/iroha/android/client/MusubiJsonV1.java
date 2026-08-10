package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import org.hyperledger.iroha.android.client.MusubiModelsV1.AliasHistoryEntry;
import org.hyperledger.iroha.android.client.MusubiModelsV1.AliasQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.AliasRecord;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveLocation;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveLocationPage;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveLocationQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveAvailability;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveRetentionDecision;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveRetentionDisposition;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveRetentionPage;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveRetentionQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveCommitment;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveRecord;
import org.hyperledger.iroha.android.client.MusubiModelsV1.AbiBinding;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ChunkerProfileHandle;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ComparatorOp;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Digest32;
import org.hyperledger.iroha.android.client.MusubiModelsV1.DependencyRequirement;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ExactPackageQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ExactReleaseSnapshot;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ExactReleaseQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.FinalizedCursor;
import org.hyperledger.iroha.android.client.MusubiModelsV1.MaintainerDirectoryEntry;
import org.hyperledger.iroha.android.client.MusubiModelsV1.MaintainerInvitation;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Namespace;
import org.hyperledger.iroha.android.client.MusubiModelsV1.NamespaceBinding;
import org.hyperledger.iroha.android.client.MusubiModelsV1.OrderedPackageEntry;
import org.hyperledger.iroha.android.client.MusubiModelsV1.OrderedPrefixQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.OrderedPrefixPage;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackageId;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackageMember;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackageName;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackagePageQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackageRecord;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackageRevisions;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackageScope;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackageSelector;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Page;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PageRequest;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PrereleaseIdentifier;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderBundleAttestationDigest;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderBundleAttestationKey;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderBundleAttestationRecord;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderBundleAttestationSetDigest;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderBundleVerificationApproval;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderBundleVerificationAttestation;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderBundleVerificationBinding;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderBundleVerificationPayload;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderCompletionAuthority;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderCompletionSignerPolicy;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderFinalizedAnchor;
import org.hyperledger.iroha.android.client.MusubiModelsV1.RegistrySnapshot;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ReleaseId;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ReleaseManifest;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ReleaseMetadata;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ReleaseRecord;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ResolverIndexQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ResolverIndexPage;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ResolverReleaseRow;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SeedIngressReceipt;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SeedIngressReceiptApproval;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SeedIngressReceiptBinding;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SeedIngressReceiptPayload;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SearchCursor;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SearchHit;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SearchPage;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SearchPageRequest;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SearchQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SearchSnapshot;
import org.hyperledger.iroha.android.client.MusubiModelsV1.StorageAvailability;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Version;
import org.hyperledger.iroha.android.client.MusubiModelsV1.VersionComparator;
import org.hyperledger.iroha.android.client.MusubiModelsV1.VersionReq;
import org.hyperledger.iroha.android.client.MusubiModelsV1.WireValue;
import org.hyperledger.iroha.android.model.NetworkId;

/** Strict Norito JSON codec for Musubi V1 query requests and responses. */
final class MusubiJsonV1 {
  private MusubiJsonV1() {}

  static Object parse(final byte[] payload, final String field) {
    if (payload == null || payload.length == 0) {
      throw new IllegalArgumentException(field + " must not be empty");
    }
    try {
      return JsonParser.parse(new String(payload, StandardCharsets.UTF_8));
    } catch (final RuntimeException error) {
      throw new IllegalArgumentException("invalid " + field + " JSON", error);
    }
  }

  static PackageRecord parseExactPackage(final byte[] payload) {
    return parsePackageRecord(parse(payload, "Musubi exact-package response"), "response");
  }

  static ExactReleaseSnapshot parseExactRelease(final byte[] payload) {
    final Map<String, Object> root =
        exactObject(
            parse(payload, "Musubi exact-release response"),
            "response",
            keys("network_id", "snapshot", "home_release", "universal_release"));
    final RegistrySnapshot snapshot = parseSnapshot(root.get("snapshot"), "response.snapshot");
    final ReleaseRecord homeRelease =
        parseReleaseRecord(root.get("home_release"), "response.home_release");
    final ResolverReleaseRow universalRelease =
        parseResolverRow(root.get("universal_release"), "response.universal_release");
    return new ExactReleaseSnapshot(
        NetworkId.parse(string(root.get("network_id"), "response.network_id")),
        snapshot, homeRelease, universalRelease);
  }

  static void validateExactReleaseSnapshot(
      final Map<String, Object> home,
      final Map<String, Object> universal,
      final NetworkId networkId,
      final RegistrySnapshot snapshot) {
    final Map<String, Object> manifest =
        object(home.get("manifest"), "response.home_release.manifest");
    final Map<String, Object> yank = object(home.get("yank"), "response.home_release.yank");
    final Map<String, Object> governance =
        object(
            home.get("artifact_governance"),
            "response.home_release.artifact_governance");
    final Map<String, Object> revisions =
        object(home.get("revisions"), "response.home_release.revisions");
    final Map<String, Object> selection =
        object(universal.get("selection"), "response.universal_release.selection");
    final Map<String, Object> storage =
        object(
            selection.get("storage"),
            "response.universal_release.selection.storage");

    final BigInteger publishedAtHeight =
        nonZeroU64(
            home.get("published_at_height"),
            "response.home_release.published_at_height");
    final BigInteger yankChangedAtHeight =
        nonZeroU64(
            yank.get("changed_at_height"),
            "response.home_release.yank.changed_at_height");
    final BigInteger yankRevision =
        nonZeroU64(yank.get("revision"), "response.home_release.yank.revision");
    final BigInteger homeYankRevision =
        nonZeroU64(revisions.get("yank"), "response.home_release.revisions.yank");
    final BigInteger homeGovernanceRevision =
        nonZeroU64(
            revisions.get("artifact_governance"),
            "response.home_release.revisions.artifact_governance");
    final BigInteger universalRevision =
        nonZeroU64(
            universal.get("index_revision"),
            "response.universal_release.index_revision");
    final BigInteger storageRevision =
        nonZeroU64(
            storage.get("index_revision"),
            "response.universal_release.selection.storage.index_revision");
    final BigInteger storageFinalizedHeight =
        nonZeroU64(
            storage.get("finalized_height"),
            "response.universal_release.selection.storage.finalized_height");
    final byte[] storageFinalizedHash =
        fixedBytes(
            storage.get("finalized_block_hash"),
            "response.universal_release.selection.storage.finalized_block_hash");
    final String governanceKind =
        string(
            governance.get("kind"),
            "response.home_release.artifact_governance.kind");
    final BigInteger takedownHeight;
    if ("TakenDown".equals(governanceKind)) {
      final Map<String, Object> takedown =
          object(
              governance.get("value"),
              "response.home_release.artifact_governance.value");
      takedownHeight =
          nonZeroU64(
              takedown.get("applied_at_height"),
              "response.home_release.artifact_governance.value.applied_at_height");
    } else {
      takedownHeight = BigInteger.ZERO;
    }

    require(
        Objects.equals(home.get("release_digest"), universal.get("release_digest"))
            && Objects.equals(manifest.get("archive_id"), universal.get("archive_id"))
            && Objects.equals(manifest.get("archive_id"), storage.get("archive_id"))
            && Objects.equals(
                manifest.get("interface_digest"), universal.get("interface_digest"))
            && Objects.equals(manifest.get("abi"), universal.get("abi"))
            && Objects.equals(manifest.get("dependencies"), universal.get("dependencies"))
            && Objects.equals(home.get("yank"), selection.get("yank"))
            && Objects.equals(
                home.get("artifact_governance"), selection.get("governance"))
            && yankRevision.equals(homeYankRevision)
            && homeYankRevision.compareTo(snapshot.indexRevision()) <= 0
            && homeGovernanceRevision.compareTo(snapshot.indexRevision()) <= 0
            && universalRevision.compareTo(snapshot.indexRevision()) <= 0
            && storageRevision.compareTo(universalRevision) <= 0
            && storageRevision.compareTo(snapshot.indexRevision()) <= 0
            && publishedAtHeight.compareTo(snapshot.finalizedHeight()) <= 0
            && yankChangedAtHeight.compareTo(publishedAtHeight) >= 0
            && yankChangedAtHeight.compareTo(snapshot.finalizedHeight()) <= 0
            && (takedownHeight.signum() == 0
                || takedownHeight.compareTo(publishedAtHeight) >= 0)
            && takedownHeight.compareTo(snapshot.finalizedHeight()) <= 0
            && storageFinalizedHeight.compareTo(snapshot.finalizedHeight()) <= 0
            && (!BigInteger.ONE.equals(snapshot.finalizedHeight())
                || Arrays.equals(networkId.bytes(), snapshot.finalizedBlockHash()))
            && (!storageFinalizedHeight.equals(snapshot.finalizedHeight())
                || Arrays.equals(storageFinalizedHash, snapshot.finalizedBlockHash())),
        "Musubi exact release snapshot is inconsistent or not finalized");
  }

  static ProviderBundleAttestationRecord parseProviderBundleAttestation(
      final byte[] payload) {
    return parseProviderBundleAttestationRecord(
        parse(payload, "Musubi provider-bundle-attestation response"), "response");
  }

  static ResolverIndexPage parseResolverPage(final byte[] payload) {
    final Map<String, Object> root =
        exactObject(
            parse(payload, "Musubi resolver-index response"),
            "response",
            keys("query", "network_id", "items", "next_cursor", "snapshot"));
    final List<Object> raw = list(root.get("items"), "response.items");
    final List<ResolverReleaseRow> items = new ArrayList<>();
    for (int index = 0; index < raw.size(); index++) {
      items.add(parseResolverRow(raw.get(index), "response.items[" + index + "]"));
    }
    final RegistrySnapshot snapshot = parseSnapshot(root.get("snapshot"), "response.snapshot");
    final FinalizedCursor cursor = root.get("next_cursor") == null
        ? null : parseCursor(root.get("next_cursor"), "response.next_cursor");
    final ResolverIndexPage page = new ResolverIndexPage(
        (ResolverIndexQuery) decodeQuery(
            MusubiToriiClientV1.RESOLVER_INDEX_PATH, root.get("query")),
        NetworkId.parse(string(root.get("network_id"), "response.network_id")),
        items,
        cursor,
        snapshot);
    page.requireMatches(page.query());
    return page;
  }

  static Page<Version> parseVersionPage(final byte[] payload) {
    final Page<Version> page =
        parsePage(
            parse(payload, "Musubi versions response"),
            "response",
            MusubiToriiClientV1.VERSIONS_PATH,
            MusubiJsonV1::parseVersion);
    for (int index = 1; index < page.items().size(); index++) {
      if (page.items().get(index - 1).compareTo(page.items().get(index)) >= 0) {
        throw new IllegalArgumentException(
            "Musubi version page must be sorted and distinct");
      }
    }
    page.requireVersionMatches((PackagePageQuery) page.query());
    return page;
  }

  static Page<MaintainerDirectoryEntry> parseMaintainerPage(final byte[] payload) {
    final Page<MaintainerDirectoryEntry> page = parsePage(
        parse(payload, "Musubi maintainers response"),
        "response",
        MusubiToriiClientV1.MAINTAINERS_PATH,
        MusubiJsonV1::parseMaintainerDirectoryEntry);
    for (int index = 1; index < page.items().size(); index++) {
      if (MusubiModelsV1.compareMaintainerEntries(
              page.items().get(index - 1), page.items().get(index)) >= 0) {
        throw new IllegalArgumentException(
            "Musubi maintainer page must be sorted and distinct");
      }
    }
    page.requireMaintainerMatches((PackagePageQuery) page.query());
    return page;
  }

  static ArchiveLocationPage parseArchiveLocationPage(final byte[] payload) {
    final Map<String, Object> root =
        exactObject(
            parse(payload, "Musubi archive-locations response"),
            "response",
            keys("network_id", "archive", "items", "next_cursor", "snapshot"));
    final List<Object> raw = list(root.get("items"), "response.items");
    final List<ArchiveLocation> items = new ArrayList<>();
    for (int index = 0; index < raw.size(); index++) {
      items.add(parseArchiveLocation(raw.get(index), "response.items[" + index + "]"));
    }
    final RegistrySnapshot snapshot = parseSnapshot(root.get("snapshot"), "response.snapshot");
    final FinalizedCursor cursor = root.get("next_cursor") == null
        ? null : parseCursor(root.get("next_cursor"), "response.next_cursor");
    return new ArchiveLocationPage(
        NetworkId.parse(string(root.get("network_id"), "response.network_id")),
        parseArchiveRecord(root.get("archive"), "response.archive"),
        items,
        cursor,
        snapshot);
  }

  static ArchiveRetentionPage parseArchiveRetentionPage(final byte[] payload) {
    final Map<String, Object> root =
        exactObject(
            parse(payload, "Musubi archive-retention response"),
            "response",
            keys("network_id", "items", "finalized_time_ms", "snapshot"));
    final List<Object> raw = list(root.get("items"), "response.items");
    final List<ArchiveRetentionDecision> items = new ArrayList<>();
    for (int index = 0; index < raw.size(); index++) {
      items.add(parseArchiveRetentionDecision(
          raw.get(index), "response.items[" + index + "]"));
    }
    return new ArchiveRetentionPage(
        NetworkId.parse(string(root.get("network_id"), "response.network_id")),
        items,
        u64(root.get("finalized_time_ms"), "response.finalized_time_ms"),
        parseSnapshot(root.get("snapshot"), "response.snapshot"));
  }

  static AliasRecord parseAlias(final byte[] payload) {
    return parseAliasRecord(parse(payload, "Musubi alias response"), "response");
  }

  static Page<AliasHistoryEntry> parseAliasHistoryPage(final byte[] payload) {
    final Page<AliasHistoryEntry> page =
        parsePage(
            parse(payload, "Musubi alias-history response"),
            "response",
            MusubiToriiClientV1.ALIAS_HISTORY_PATH,
            MusubiJsonV1::parseAliasHistory);
    for (int index = 1; index < page.items().size(); index++) {
      if (MusubiModelsV1.compareAliasHistoryEntries(
              page.items().get(index - 1), page.items().get(index)) >= 0) {
        throw new IllegalArgumentException(
            "Musubi alias-history page must be sorted and distinct");
      }
    }
    page.requireAliasHistoryMatches((AliasQuery) page.query());
    return page;
  }

  static OrderedPrefixPage parseOrderedPackagePage(final byte[] payload) {
    final Map<String, Object> root =
        exactObject(
            parse(payload, "Musubi ordered-prefix response"),
            "response",
            keys(
                "query", "network_id", "namespace_binding", "items",
                "next_cursor", "snapshot"));
    final List<Object> raw = list(root.get("items"), "response.items");
    final List<OrderedPackageEntry> items = new ArrayList<>();
    for (int index = 0; index < raw.size(); index++) {
      items.add(parseOrderedEntry(raw.get(index), "response.items[" + index + "]"));
    }
    final RegistrySnapshot snapshot = parseSnapshot(root.get("snapshot"), "response.snapshot");
    final FinalizedCursor cursor = root.get("next_cursor") == null
        ? null : parseCursor(root.get("next_cursor"), "response.next_cursor");
    final OrderedPrefixPage page = new OrderedPrefixPage(
        (OrderedPrefixQuery) decodeQuery(
            MusubiToriiClientV1.ORDERED_PREFIX_PATH, root.get("query")),
        NetworkId.parse(string(root.get("network_id"), "response.network_id")),
        parseNamespaceBinding(root.get("namespace_binding"), "response.namespace_binding"),
        items,
        cursor,
        snapshot);
    page.requireMatches(page.query());
    return page;
  }

  static SearchPage parseSearchPage(final byte[] payload) {
    final Map<String, Object> root =
        exactObject(
            parse(payload, "Musubi search response"),
            "response",
            keys("query", "items", "next_cursor", "snapshot"));
    final List<Object> raw = list(root.get("items"), "response.items");
    final List<SearchHit> items = new ArrayList<>();
    for (int index = 0; index < raw.size(); index++) {
      items.add(parseSearchHit(raw.get(index), "response.items[" + index + "]"));
    }
    final SearchSnapshot snapshot =
        parseSearchSnapshot(root.get("snapshot"), "response.snapshot");
    final SearchCursor cursor = root.get("next_cursor") == null
        ? null : parseSearchCursor(root.get("next_cursor"), "response.next_cursor");
    final SearchPage page = new SearchPage(
        (SearchQuery) decodeQuery(MusubiToriiClientV1.SEARCH_PATH, root.get("query")),
        items,
        cursor,
        snapshot);
    page.requireMatches(page.query());
    return page;
  }

  static WireValue decodeQuery(final String path, final Object value) {
    switch (path) {
      case MusubiToriiClientV1.EXACT_PACKAGE_PATH: {
        final Map<String, Object> root = exactObject(value, "request", keys("package"));
        return new ExactPackageQuery(parsePackage(root.get("package"), "request.package"));
      }
      case MusubiToriiClientV1.EXACT_RELEASE_PATH: {
        final Map<String, Object> root = exactObject(value, "request", keys("release"));
        return new ExactReleaseQuery(parseRelease(root.get("release"), "request.release"));
      }
      case MusubiToriiClientV1.PROVIDER_BUNDLE_ATTESTATION_PATH:
        return parseProviderBundleAttestationKey(value, "request");
      case MusubiToriiClientV1.RESOLVER_INDEX_PATH: {
        final Map<String, Object> root =
            exactObject(value, "request", keys("package", "requirement", "page"));
        return new ResolverIndexQuery(
            parsePackage(root.get("package"), "request.package"),
            root.get("requirement") == null
                ? null : parseRequirement(root.get("requirement"), "request.requirement"),
            parsePageRequest(root.get("page"), "request.page"));
      }
      case MusubiToriiClientV1.VERSIONS_PATH:
      case MusubiToriiClientV1.MAINTAINERS_PATH: {
        final Map<String, Object> root = exactObject(value, "request", keys("package", "page"));
        return new PackagePageQuery(
            parsePackage(root.get("package"), "request.package"),
            parsePageRequest(root.get("page"), "request.page"));
      }
      case MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH: {
        final Map<String, Object> root = exactObject(value, "request", keys("archive_id", "page"));
        return new ArchiveLocationQuery(
            digest(root.get("archive_id"), "request.archive_id"),
            parsePageRequest(root.get("page"), "request.page"));
      }
      case MusubiToriiClientV1.ARCHIVE_RETENTION_PATH: {
        final Map<String, Object> root =
            exactObject(value, "request", keys("archive_ids", "expected_snapshot"));
        final List<Object> raw = list(root.get("archive_ids"), "request.archive_ids");
        final List<Digest32> archiveIds = new ArrayList<>();
        for (int index = 0; index < raw.size(); index++) {
          archiveIds.add(digest(raw.get(index), "request.archive_ids[" + index + "]"));
        }
        return new ArchiveRetentionQuery(
            archiveIds,
            root.get("expected_snapshot") == null
                ? null : parseSnapshot(
                    root.get("expected_snapshot"), "request.expected_snapshot"));
      }
      case MusubiToriiClientV1.ALIAS_PATH:
      case MusubiToriiClientV1.ALIAS_HISTORY_PATH: {
        final Map<String, Object> root = exactObject(value, "request", keys("alias", "page"));
        return new AliasQuery(
            newtypeText(root.get("alias"), "request.alias"),
            parsePageRequest(root.get("page"), "request.page"));
      }
      case MusubiToriiClientV1.ORDERED_PREFIX_PATH: {
        final Map<String, Object> root = exactObject(value, "request", keys("prefix", "page"));
        return new OrderedPrefixQuery(
            newtypeText(root.get("prefix"), "request.prefix"),
            parsePageRequest(root.get("page"), "request.page"));
      }
      case MusubiToriiClientV1.SEARCH_PATH: {
        final Map<String, Object> root = exactObject(value, "request", keys("query", "page"));
        return new SearchQuery(
            string(root.get("query"), "request.query"),
            parseSearchPageRequest(root.get("page"), "request.page"));
      }
      default: throw new IllegalArgumentException("unsupported Musubi V1 query path: " + path);
    }
  }

  static WireValue decodeResponse(final String path, final Object value) {
    final byte[] payload = JsonEncoder.encode(value).getBytes(StandardCharsets.UTF_8);
    switch (path) {
      case MusubiToriiClientV1.EXACT_PACKAGE_PATH: return parseExactPackage(payload);
      case MusubiToriiClientV1.EXACT_RELEASE_PATH: return parseExactRelease(payload);
      case MusubiToriiClientV1.PROVIDER_BUNDLE_ATTESTATION_PATH:
        return parseProviderBundleAttestation(payload);
      case MusubiToriiClientV1.RESOLVER_INDEX_PATH: return parseResolverPage(payload);
      case MusubiToriiClientV1.VERSIONS_PATH: return parseVersionPage(payload);
      case MusubiToriiClientV1.MAINTAINERS_PATH: return parseMaintainerPage(payload);
      case MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH: return parseArchiveLocationPage(payload);
      case MusubiToriiClientV1.ARCHIVE_RETENTION_PATH: return parseArchiveRetentionPage(payload);
      case MusubiToriiClientV1.ALIAS_PATH: return parseAlias(payload);
      case MusubiToriiClientV1.ALIAS_HISTORY_PATH: return parseAliasHistoryPage(payload);
      case MusubiToriiClientV1.ORDERED_PREFIX_PATH: return parseOrderedPackagePage(payload);
      case MusubiToriiClientV1.SEARCH_PATH: return parseSearchPage(payload);
      default: throw new IllegalArgumentException("unsupported Musubi V1 query path: " + path);
    }
  }

  private static PackageId parsePackage(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(value, field, keys("home_dataspace", "scope", "name"));
    return new PackageId(
        u64(root.get("home_dataspace"), field + ".home_dataspace"),
        parseScope(root.get("scope"), field + ".scope"),
        new PackageName(newtypeText(root.get("name"), field + ".name")));
  }

  private static PackageScope parseScope(final Object value, final String field) {
    final Map<String, Object> root = tagged(value, field);
    final String kind = string(root.get("kind"), field + ".kind");
    if ("DataspaceRoot".equals(kind)) {
      require(root.get("value") == null, field + ".value must be null");
      return PackageScope.dataspaceRoot();
    }
    if ("Domain".equals(kind)) return PackageScope.domain(string(root.get("value"), field + ".value"));
    throw new IllegalArgumentException(field + ".kind is unsupported in Musubi V1");
  }

  private static NamespaceBinding parseNamespaceBinding(
      final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value,
            field,
            keys("namespace", "home_dataspace", "scope", "generation"));
    return new NamespaceBinding(
        new Namespace(newtypeText(root.get("namespace"), field + ".namespace")),
        u64(root.get("home_dataspace"), field + ".home_dataspace"),
        parseScope(root.get("scope"), field + ".scope"),
        nonZeroU64(root.get("generation"), field + ".generation"));
  }

  private static PackageSelector parseSelector(final Object value, final String field) {
    final Map<String, Object> root = exactObject(value, field, keys("namespace", "name"));
    return new PackageSelector(
        new Namespace(newtypeText(root.get("namespace"), field + ".namespace")),
        new PackageName(newtypeText(root.get("name"), field + ".name")));
  }

  private static ReleaseId parseRelease(final Object value, final String field) {
    final Map<String, Object> root = exactObject(value, field, keys("package", "version"));
    return new ReleaseId(
        parsePackage(root.get("package"), field + ".package"),
        parseVersion(root.get("version"), field + ".version"));
  }

  private static Version parseVersion(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(value, field, keys("major", "minor", "patch", "prerelease"));
    final List<PrereleaseIdentifier> identifiers = new ArrayList<>();
    final List<Object> raw = list(root.get("prerelease"), field + ".prerelease");
    for (int index = 0; index < raw.size(); index++) {
      final String itemField = field + ".prerelease[" + index + "]";
      final Map<String, Object> item = tagged(raw.get(index), itemField);
      final String kind = string(item.get("kind"), itemField + ".kind");
      if ("Numeric".equals(kind)) {
        identifiers.add(PrereleaseIdentifier.numeric(u64(item.get("value"), itemField + ".value")));
      } else if ("AlphaNumeric".equals(kind)) {
        identifiers.add(PrereleaseIdentifier.alphaNumeric(string(item.get("value"), itemField + ".value")));
      } else {
        throw new IllegalArgumentException(itemField + ".kind is unsupported in Musubi V1");
      }
    }
    return new Version(
        u64(root.get("major"), field + ".major"),
        u64(root.get("minor"), field + ".minor"),
        u64(root.get("patch"), field + ".patch"),
        identifiers);
  }

  private static VersionReq parseRequirement(final Object value, final String field) {
    final Map<String, Object> root = tagged(value, field);
    final String kind = string(root.get("kind"), field + ".kind");
    switch (kind) {
      case "Any":
        require(root.get("value") == null, field + ".value must be null");
        return VersionReq.fromWire(VersionReq.Kind.ANY, null, null, null, new ArrayList<>());
      case "Caret":
        return VersionReq.fromWire(VersionReq.Kind.CARET, parseVersion(root.get("value"), field + ".value"), null, null, new ArrayList<>());
      case "Tilde":
        return VersionReq.fromWire(VersionReq.Kind.TILDE, parseVersion(root.get("value"), field + ".value"), null, null, new ArrayList<>());
      case "Exact":
        return VersionReq.fromWire(VersionReq.Kind.EXACT, parseVersion(root.get("value"), field + ".value"), null, null, new ArrayList<>());
      case "MajorWildcard":
        return VersionReq.fromWire(VersionReq.Kind.MAJOR_WILDCARD, null, u64(root.get("value"), field + ".value"), null, new ArrayList<>());
      case "MinorWildcard": {
        final Map<String, Object> wildcard = exactObject(root.get("value"), field + ".value", keys("major", "minor"));
        return VersionReq.fromWire(
            VersionReq.Kind.MINOR_WILDCARD, null,
            u64(wildcard.get("major"), field + ".value.major"),
            u64(wildcard.get("minor"), field + ".value.minor"), new ArrayList<>());
      }
      case "Comparators": {
        final List<VersionComparator> comparators = new ArrayList<>();
        final List<Object> raw = list(root.get("value"), field + ".value");
        for (int index = 0; index < raw.size(); index++) {
          comparators.add(parseComparator(raw.get(index), field + ".value[" + index + "]"));
        }
        return VersionReq.fromWire(VersionReq.Kind.COMPARATORS, null, null, null, comparators);
      }
      default: throw new IllegalArgumentException(field + ".kind is unsupported in Musubi V1");
    }
  }

  private static VersionComparator parseComparator(final Object value, final String field) {
    final Map<String, Object> root = exactObject(value, field, keys("op", "version"));
    final Map<String, Object> opRoot = tagged(root.get("op"), field + ".op");
    require(opRoot.get("value") == null, field + ".op.value must be null");
    final String kind = string(opRoot.get("kind"), field + ".op.kind");
    final ComparatorOp op;
    if ("Greater".equals(kind)) op = ComparatorOp.GREATER;
    else if ("GreaterOrEqual".equals(kind)) op = ComparatorOp.GREATER_OR_EQUAL;
    else if ("Less".equals(kind)) op = ComparatorOp.LESS;
    else if ("LessOrEqual".equals(kind)) op = ComparatorOp.LESS_OR_EQUAL;
    else if ("Equal".equals(kind)) op = ComparatorOp.EQUAL;
    else throw new IllegalArgumentException(field + ".op.kind is unsupported");
    return new VersionComparator(op, parseVersion(root.get("version"), field + ".version"));
  }

  private static RegistrySnapshot parseSnapshot(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(value, field, keys("finalized_height", "finalized_block_hash", "index_revision"));
    return new RegistrySnapshot(
        u64(root.get("finalized_height"), field + ".finalized_height"),
        fixedBytes(root.get("finalized_block_hash"), field + ".finalized_block_hash"),
        u64(root.get("index_revision"), field + ".index_revision"));
  }

  private static FinalizedCursor parseCursor(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(value, field, keys("snapshot", "query_hash", "last_key", "caller"));
    return new FinalizedCursor(
        parseSnapshot(root.get("snapshot"), field + ".snapshot"),
        digest(root.get("query_hash"), field + ".query_hash"),
        string(root.get("last_key"), field + ".last_key"),
        root.get("caller") == null ? null : string(root.get("caller"), field + ".caller"));
  }

  private static PageRequest parsePageRequest(final Object value, final String field) {
    final Map<String, Object> root = exactObject(value, field, keys("limit", "cursor"));
    return new PageRequest(
        u32(root.get("limit"), field + ".limit"),
        root.get("cursor") == null ? null : parseCursor(root.get("cursor"), field + ".cursor"));
  }

  private static SearchSnapshot parseSearchSnapshot(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value,
            field,
            keys("finalized_height", "finalized_block_hash", "projection_revision"));
    return new SearchSnapshot(
        u64(root.get("finalized_height"), field + ".finalized_height"),
        fixedBytes(root.get("finalized_block_hash"), field + ".finalized_block_hash"),
        u64(root.get("projection_revision"), field + ".projection_revision"));
  }

  private static SearchCursor parseSearchCursor(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(value, field, keys("snapshot", "query_hash", "last_package"));
    return new SearchCursor(
        parseSearchSnapshot(root.get("snapshot"), field + ".snapshot"),
        digest(root.get("query_hash"), field + ".query_hash"),
        parsePackage(root.get("last_package"), field + ".last_package"));
  }

  private static SearchPageRequest parseSearchPageRequest(
      final Object value, final String field) {
    final Map<String, Object> root = exactObject(value, field, keys("limit", "cursor"));
    return new SearchPageRequest(
        u32(root.get("limit"), field + ".limit"),
        root.get("cursor") == null
            ? null : parseSearchCursor(root.get("cursor"), field + ".cursor"));
  }

  private static PackageRecord parsePackageRecord(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value,
            field,
            keys("package", "claimed_namespace", "claimed_namespace_binding", "owners",
                "member_accounts", "claimed_at_height", "revisions"));
    final Map<String, Object> revisions =
        exactObject(root.get("revisions"), field + ".revisions", keys("governance", "metadata", "archive_locations"));
    return new PackageRecord(
        parsePackage(root.get("package"), field + ".package"),
        new Namespace(newtypeText(root.get("claimed_namespace"), field + ".claimed_namespace")),
        digest(root.get("claimed_namespace_binding"), field + ".claimed_namespace_binding"),
        stringList(root.get("owners"), field + ".owners"),
        stringList(root.get("member_accounts"), field + ".member_accounts"),
        nonZeroU64(root.get("claimed_at_height"), field + ".claimed_at_height"),
        new PackageRevisions(
            nonZeroU64(revisions.get("governance"), field + ".revisions.governance"),
            nonZeroU64(revisions.get("metadata"), field + ".revisions.metadata"),
            nonZeroU64(revisions.get("archive_locations"), field + ".revisions.archive_locations")));
  }

  private static ReleaseRecord parseReleaseRecord(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value, field,
            keys("manifest", "release_digest", "published_by", "published_at_height", "yank",
                "artifact_governance", "revisions"));
    final ReleaseManifest manifest = parseManifest(root.get("manifest"), field + ".manifest");
    final Digest32 releaseDigest = digest(root.get("release_digest"), field + ".release_digest");
    final String publisher = string(root.get("published_by"), field + ".published_by");
    final BigInteger height = nonZeroU64(root.get("published_at_height"), field + ".published_at_height");
    validateYank(root.get("yank"), field + ".yank", manifest.release());
    validateGovernance(root.get("artifact_governance"), field + ".artifact_governance");
    final Map<String, Object> revisions = exactObject(root.get("revisions"), field + ".revisions", keys("yank", "artifact_governance"));
    nonZeroU64(revisions.get("yank"), field + ".revisions.yank");
    nonZeroU64(revisions.get("artifact_governance"), field + ".revisions.artifact_governance");
    return new ReleaseRecord(manifest, releaseDigest, publisher, height, root);
  }

  private static ReleaseManifest parseManifest(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value, field,
            keys("release", "edition", "abi", "dependencies", "exports", "interface_digest",
                "metadata", "archive_id", "verification_lock_digest"));
    final ReleaseId release = parseRelease(root.get("release"), field + ".release");
    taggedUnit(root.get("edition"), field + ".edition", keys("V1"));
    final AbiBinding abi = parseAbi(root.get("abi"), field + ".abi");
    final List<Object> dependencies = list(root.get("dependencies"), field + ".dependencies");
    final List<DependencyRequirement> parsedDependencies = new ArrayList<>();
    for (int index = 0; index < dependencies.size(); index++) {
      parsedDependencies.add(
          parseDependency(dependencies.get(index), field + ".dependencies[" + index + "]"));
    }
    final List<String> exports = stringList(root.get("exports"), field + ".exports");
    for (final String export : exports) {
      MusubiModelsV1.requireName(export, "Musubi export");
    }
    return new ReleaseManifest(
        release,
        abi,
        parsedDependencies,
        exports,
        digest(root.get("interface_digest"), field + ".interface_digest"),
        parseMetadata(root.get("metadata"), field + ".metadata"),
        digest(root.get("archive_id"), field + ".archive_id"),
        digest(root.get("verification_lock_digest"), field + ".verification_lock_digest"));
  }

  private static AbiBinding parseAbi(final Object value, final String field) {
    final Map<String, Object> root = exactObject(value, field, keys("abi_version", "abi_hash"));
    if (u16(root.get("abi_version"), field + ".abi_version") != 1) {
      throw new IllegalArgumentException(field + ".abi_version is unsupported; Musubi only supports V1");
    }
    return new AbiBinding(fixedBytes(root.get("abi_hash"), field + ".abi_hash"));
  }

  private static DependencyRequirement parseDependency(final Object value, final String field) {
    final Map<String, Object> root = exactObject(value, field, keys("alias", "package", "requirement"));
    return new DependencyRequirement(
        string(root.get("alias"), field + ".alias"),
        parsePackage(root.get("package"), field + ".package"),
        parseRequirement(root.get("requirement"), field + ".requirement"));
  }

  private static ReleaseMetadata parseMetadata(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(value, field, keys("description", "readme", "license", "repository", "keywords"));
    final List<Object> keywords = list(root.get("keywords"), field + ".keywords");
    final List<String> parsedKeywords = new ArrayList<>();
    for (int index = 0; index < keywords.size(); index++) {
      parsedKeywords.add(newtypeText(
          keywords.get(index), field + ".keywords[" + index + "]"));
    }
    return new ReleaseMetadata(
        root.get("description") == null
            ? null : newtypeText(root.get("description"), field + ".description"),
        root.get("readme") == null
            ? null : newtypeText(root.get("readme"), field + ".readme"),
        root.get("license") == null
            ? null : newtypeText(root.get("license"), field + ".license"),
        root.get("repository") == null
            ? null : newtypeText(root.get("repository"), field + ".repository"),
        parsedKeywords);
  }

  private static void validateYank(final Object value, final String field, final ReleaseId expected) {
    final Map<String, Object> root =
        exactObject(value, field, keys("release", "yanked", "reason", "changed_by", "changed_at_height", "revision"));
    require(expected.equals(parseRelease(root.get("release"), field + ".release")), field + ".release differs from manifest");
    bool(root.get("yanked"), field + ".yanked");
    newtypeText(root.get("reason"), field + ".reason");
    string(root.get("changed_by"), field + ".changed_by");
    nonZeroU64(root.get("changed_at_height"), field + ".changed_at_height");
    nonZeroU64(root.get("revision"), field + ".revision");
  }

  private static void validateGovernance(final Object value, final String field) {
    final Map<String, Object> root = tagged(value, field);
    final String kind = string(root.get("kind"), field + ".kind");
    if ("Available".equals(kind)) {
      require(root.get("value") == null, field + ".value must be null");
      return;
    }
    if ("TakenDown".equals(kind)) {
      final Map<String, Object> payload = exactObject(root.get("value"), field + ".value", keys("action_digest", "reason", "applied_at_height"));
      digest(payload.get("action_digest"), field + ".value.action_digest");
      newtypeText(payload.get("reason"), field + ".value.reason");
      nonZeroU64(payload.get("applied_at_height"), field + ".value.applied_at_height");
      return;
    }
    throw new IllegalArgumentException(field + ".kind is unsupported in Musubi V1");
  }

  private static ResolverReleaseRow parseResolverRow(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value, field,
            keys("release", "release_digest", "archive_id", "source_digest", "interface_digest",
                "abi", "dependencies", "selection", "index_revision"));
    final ReleaseId release = parseRelease(root.get("release"), field + ".release");
    for (final String key : Arrays.asList("release_digest", "archive_id", "source_digest", "interface_digest")) {
      digest(root.get(key), field + "." + key);
    }
    parseAbi(root.get("abi"), field + ".abi");
    final List<Object> dependencies = list(root.get("dependencies"), field + ".dependencies");
    final List<DependencyRequirement> parsedDependencies = new ArrayList<>();
    for (int index = 0; index < dependencies.size(); index++) {
      parsedDependencies.add(
          parseDependency(dependencies.get(index), field + ".dependencies[" + index + "]"));
    }
    MusubiModelsV1.requireCanonicalDependencyRequirements(
        parsedDependencies, field + ".dependencies");
    final BigInteger storageRevision =
        validateSelection(root.get("selection"), field + ".selection", release);
    final BigInteger revision = nonZeroU64(root.get("index_revision"), field + ".index_revision");
    return new ResolverReleaseRow(release, revision, storageRevision, root);
  }

  private static BigInteger validateSelection(
      final Object value, final String field, final ReleaseId release) {
    final Map<String, Object> root = exactObject(value, field, keys("yank", "storage", "governance"));
    validateYank(root.get("yank"), field + ".yank", release);
    final Map<String, Object> storage =
        exactObject(root.get("storage"), field + ".storage",
            keys("archive_id", "availability", "healthy_replicas", "active_locations",
                "finalized_height", "finalized_block_hash", "index_revision"));
    digest(storage.get("archive_id"), field + ".storage.archive_id");
    taggedUnit(storage.get("availability"), field + ".storage.availability", keys("Selectable", "BelowQuorum", "Unavailable"));
    u16(storage.get("healthy_replicas"), field + ".storage.healthy_replicas");
    u8(storage.get("active_locations"), field + ".storage.active_locations");
    nonZeroU64(storage.get("finalized_height"), field + ".storage.finalized_height");
    fixedBytes(storage.get("finalized_block_hash"), field + ".storage.finalized_block_hash");
    final BigInteger storageRevision =
        nonZeroU64(storage.get("index_revision"), field + ".storage.index_revision");
    validateGovernance(root.get("governance"), field + ".governance");
    return storageRevision;
  }

  private static MaintainerDirectoryEntry parseMaintainerDirectoryEntry(
      final Object value, final String field) {
    final Map<String, Object> root = tagged(value, field);
    final String kind = string(root.get("kind"), field + ".kind");
    if ("Accepted".equals(kind)) {
      return MaintainerDirectoryEntry.accepted(
          parseMember(root.get("value"), field + ".value"));
    }
    if ("PendingInvitation".equals(kind)) {
      return MaintainerDirectoryEntry.pendingInvitation(
          parseMaintainerInvitation(root.get("value"), field + ".value"));
    }
    throw new IllegalArgumentException(field + ".kind is unsupported in Musubi V1");
  }

  private static PackageMember parseMember(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(value, field, keys("package", "account", "role", "accepted_at_height", "governance_revision"));
    final String kind = parsePackageRole(root.get("role"), field + ".role");
    return new PackageMember(
        parsePackage(root.get("package"), field + ".package"),
        string(root.get("account"), field + ".account"),
        kind,
        nonZeroU64(root.get("accepted_at_height"), field + ".accepted_at_height"),
        nonZeroU64(root.get("governance_revision"), field + ".governance_revision"),
        root);
  }

  private static MaintainerInvitation parseMaintainerInvitation(
      final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value,
            field,
            keys(
                "invite_id", "package", "invited_by", "invited_account", "role",
                "expected_governance_revision", "expires_at_height", "state"));
    final Digest32 inviteId = digest(root.get("invite_id"), field + ".invite_id");
    require(
        !Arrays.equals(inviteId.bytes(), new byte[32]),
        field + ".invite_id must not be inert");
    final String state =
        taggedUnit(root.get("state"), field + ".state", keys("Pending"));
    return new MaintainerInvitation(
        inviteId,
        parsePackage(root.get("package"), field + ".package"),
        string(root.get("invited_by"), field + ".invited_by"),
        string(root.get("invited_account"), field + ".invited_account"),
        parsePackageRole(root.get("role"), field + ".role"),
        nonZeroU64(
            root.get("expected_governance_revision"),
            field + ".expected_governance_revision"),
        nonZeroU64(root.get("expires_at_height"), field + ".expires_at_height"),
        state,
        root);
  }

  private static String parsePackageRole(final Object value, final String field) {
    final Map<String, Object> role = tagged(value, field);
    final String kind = string(role.get("kind"), field + ".kind");
    if ("Owner".equals(kind)) {
      require(role.get("value") == null, field + ".value must be null");
      return kind;
    }
    if ("Maintainer".equals(kind)) {
      final Map<String, Object> permissions =
          exactObject(
              role.get("value"),
              field + ".value",
              keys("publish", "yank", "metadata", "archive_locations"));
      boolean grantsPermission = false;
      for (final Map.Entry<String, Object> entry : permissions.entrySet()) {
        grantsPermission |= bool(entry.getValue(), field + ".value." + entry.getKey());
      }
      require(grantsPermission, field + ".value must grant at least one permission");
      return kind;
    }
    throw new IllegalArgumentException(field + ".kind is unsupported in Musubi V1");
  }

  private static ArchiveRetentionDecision parseArchiveRetentionDecision(
      final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value,
            field,
            keys(
                "archive_id", "disposition", "active_releases", "yanked_releases",
                "taken_down_releases", "storage"));
    final String kind = taggedUnit(
        root.get("disposition"),
        field + ".disposition",
        keys(
            "RetainUnknown", "RetainReferenced", "PruneUnreferenced",
            "PruneGovernedTakedown"));
    final ArchiveRetentionDisposition disposition;
    switch (kind) {
      case "RetainUnknown":
        disposition = ArchiveRetentionDisposition.RETAIN_UNKNOWN;
        break;
      case "RetainReferenced":
        disposition = ArchiveRetentionDisposition.RETAIN_REFERENCED;
        break;
      case "PruneUnreferenced":
        disposition = ArchiveRetentionDisposition.PRUNE_UNREFERENCED;
        break;
      default:
        disposition = ArchiveRetentionDisposition.PRUNE_GOVERNED_TAKEDOWN;
        break;
    }
    return new ArchiveRetentionDecision(
        digest(root.get("archive_id"), field + ".archive_id"),
        disposition,
        u16(root.get("active_releases"), field + ".active_releases"),
        u16(root.get("yanked_releases"), field + ".yanked_releases"),
        u16(root.get("taken_down_releases"), field + ".taken_down_releases"),
        root.get("storage") == null
            ? null : parseArchiveAvailability(root.get("storage"), field + ".storage"));
  }

  private static ArchiveAvailability parseArchiveAvailability(
      final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value,
            field,
            keys(
                "archive_id", "availability", "healthy_replicas", "active_locations",
                "finalized_height", "finalized_block_hash", "index_revision"));
    final String kind = taggedUnit(
        root.get("availability"),
        field + ".availability",
        keys("Selectable", "BelowQuorum", "Unavailable"));
    final StorageAvailability availability;
    if ("Selectable".equals(kind)) availability = StorageAvailability.SELECTABLE;
    else if ("BelowQuorum".equals(kind)) availability = StorageAvailability.BELOW_QUORUM;
    else availability = StorageAvailability.UNAVAILABLE;
    return new ArchiveAvailability(
        digest(root.get("archive_id"), field + ".archive_id"),
        availability,
        u16(root.get("healthy_replicas"), field + ".healthy_replicas"),
        u8(root.get("active_locations"), field + ".active_locations"),
        nonZeroU64(root.get("finalized_height"), field + ".finalized_height"),
        fixedBytes(root.get("finalized_block_hash"), field + ".finalized_block_hash"),
        nonZeroU64(root.get("index_revision"), field + ".index_revision"));
  }

  private static ArchiveLocation parseArchiveLocation(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value, field,
            keys("location_id", "archive_id", "pin_manifest", "replication_order", "providers",
                "provider_attestation_set_digest", "renew_after_epoch", "expires_at_epoch",
                "finalized_height", "revision", "state"));
    final Digest32 locationId = digest(root.get("location_id"), field + ".location_id");
    final Digest32 archiveId = digest(root.get("archive_id"), field + ".archive_id");
    require(root.get("pin_manifest") != null && root.get("replication_order") != null,
        field + " must carry SoraFS pin and order identities");
    final List<String> providers = new ArrayList<>();
    final List<Object> rawProviders = list(root.get("providers"), field + ".providers");
    for (int index = 0; index < rawProviders.size(); index++) {
      providers.add(newtypeText(rawProviders.get(index), field + ".providers[" + index + "]"));
    }
    final ProviderBundleAttestationSetDigest providerAttestationSetDigest =
        ProviderBundleAttestationSetDigest.fromBytes(
            digest(
                    root.get("provider_attestation_set_digest"),
                    field + ".provider_attestation_set_digest")
                .bytes());
    u64(root.get("renew_after_epoch"), field + ".renew_after_epoch");
    u64(root.get("expires_at_epoch"), field + ".expires_at_epoch");
    final BigInteger finalizedHeight =
        nonZeroU64(root.get("finalized_height"), field + ".finalized_height");
    final BigInteger revision = nonZeroU64(root.get("revision"), field + ".revision");
    final String state = taggedUnit(root.get("state"), field + ".state", keys("Pending", "Healthy", "Degraded", "Retired"));
    return new ArchiveLocation(
        locationId,
        archiveId,
        providers,
        providerAttestationSetDigest,
        finalizedHeight,
        revision,
        state,
        root);
  }

  private static ArchiveRecord parseArchiveRecord(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value,
            field,
            keys(
                "archive_id", "commitment", "staging_receipt", "registered_by",
                "registered_at_height", "location_revision", "location_ids"));
    final List<Object> rawIds = list(root.get("location_ids"), field + ".location_ids");
    final List<Digest32> locationIds = new ArrayList<>();
    for (int index = 0; index < rawIds.size(); index++) {
      locationIds.add(digest(rawIds.get(index), field + ".location_ids[" + index + "]"));
    }
    return new ArchiveRecord(
        digest(root.get("archive_id"), field + ".archive_id"),
        parseArchiveCommitment(root.get("commitment"), field + ".commitment"),
        parseSeedIngressReceipt(root.get("staging_receipt"), field + ".staging_receipt"),
        string(root.get("registered_by"), field + ".registered_by"),
        nonZeroU64(root.get("registered_at_height"), field + ".registered_at_height"),
        nonZeroU64(root.get("location_revision"), field + ".location_revision"),
        locationIds);
  }

  private static ArchiveCommitment parseArchiveCommitment(
      final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value,
            field,
            keys(
                "root_cid", "chunker", "chunk_plan_digest", "por_root", "content_length",
                "car_digest", "car_size", "bundle_digest", "source_tree_digest",
                "descriptor_digest", "file_count", "chunk_count"));
    final Map<String, Object> chunker =
        exactObject(
            root.get("chunker"),
            field + ".chunker",
            keys("profile_id", "namespace", "name", "semver", "multihash_code"));
    final ChunkerProfileHandle profile =
        new ChunkerProfileHandle(
            u32(chunker.get("profile_id"), field + ".chunker.profile_id"),
            string(chunker.get("namespace"), field + ".chunker.namespace"),
            string(chunker.get("name"), field + ".chunker.name"),
            string(chunker.get("semver"), field + ".chunker.semver"),
            u64(chunker.get("multihash_code"), field + ".chunker.multihash_code"));
    return new ArchiveCommitment(
        byteArray(root.get("root_cid"), field + ".root_cid", 36),
        profile,
        digest(root.get("chunk_plan_digest"), field + ".chunk_plan_digest"),
        digest(root.get("por_root"), field + ".por_root"),
        u64(root.get("content_length"), field + ".content_length"),
        digest(root.get("car_digest"), field + ".car_digest"),
        u64(root.get("car_size"), field + ".car_size"),
        digest(root.get("bundle_digest"), field + ".bundle_digest"),
        digest(root.get("source_tree_digest"), field + ".source_tree_digest"),
        digest(root.get("descriptor_digest"), field + ".descriptor_digest"),
        u32(root.get("file_count"), field + ".file_count"),
        u32(root.get("chunk_count"), field + ".chunk_count"));
  }

  private static SeedIngressReceipt parseSeedIngressReceipt(
      final Object value, final String field) {
    final Map<String, Object> root = exactObject(value, field, keys("payload", "approvals"));
    final Map<String, Object> payload =
        exactObject(
            root.get("payload"),
            field + ".payload",
            keys("version", "binding", "issued_at_ms", "expires_at_ms"));
    require(u8(payload.get("version"), field + ".payload.version") == 1,
        field + ".payload.version is unsupported in Musubi V1");
    final Map<String, Object> binding =
        exactObject(
            payload.get("binding"),
            field + ".payload.binding",
            keys(
                "network_id", "publisher", "ingress_broker",
                "seed_provider", "semantic_release_manifest_digest", "archive_id",
                "car_body_digest", "car_body_length", "nonce"));
    final SeedIngressReceiptBinding typedBinding =
        new SeedIngressReceiptBinding(
            NetworkId.parse(
                string(binding.get("network_id"), field + ".payload.binding.network_id")),
            string(binding.get("publisher"), field + ".payload.binding.publisher"),
            string(binding.get("ingress_broker"), field + ".payload.binding.ingress_broker"),
            newtypeText(binding.get("seed_provider"), field + ".payload.binding.seed_provider"),
            digest(
                binding.get("semantic_release_manifest_digest"),
                field + ".payload.binding.semantic_release_manifest_digest"),
            digest(binding.get("archive_id"), field + ".payload.binding.archive_id"),
            digest(binding.get("car_body_digest"), field + ".payload.binding.car_body_digest"),
            u64(binding.get("car_body_length"), field + ".payload.binding.car_body_length"),
            fixedBytes(binding.get("nonce"), field + ".payload.binding.nonce"));
    final SeedIngressReceiptPayload typedPayload =
        new SeedIngressReceiptPayload(
            typedBinding,
            u64(payload.get("issued_at_ms"), field + ".payload.issued_at_ms"),
            u64(payload.get("expires_at_ms"), field + ".payload.expires_at_ms"));
    final List<Object> rawApprovals = list(root.get("approvals"), field + ".approvals");
    final List<SeedIngressReceiptApproval> approvals = new ArrayList<>();
    for (int index = 0; index < rawApprovals.size(); index++) {
      final String approvalField = field + ".approvals[" + index + "]";
      final Map<String, Object> approval =
          exactObject(rawApprovals.get(index), approvalField, keys("public_key", "signature"));
      approvals.add(
          new SeedIngressReceiptApproval(
              string(approval.get("public_key"), approvalField + ".public_key"),
              string(approval.get("signature"), approvalField + ".signature")));
    }
    return new SeedIngressReceipt(typedPayload, approvals);
  }

  private static ProviderBundleAttestationKey parseProviderBundleAttestationKey(
      final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value,
            field,
            keys("archive_id", "replication_order", "provider_id"));
    return new ProviderBundleAttestationKey(
        digest(root.get("archive_id"), field + ".archive_id"),
        digest(root.get("replication_order"), field + ".replication_order"),
        newtypeText(root.get("provider_id"), field + ".provider_id"));
  }

  private static ProviderBundleAttestationRecord parseProviderBundleAttestationRecord(
      final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value,
            field,
            keys(
                "key", "attestation_digest", "attestation", "registered_by",
                "registered_at_height"));
    return new ProviderBundleAttestationRecord(
        parseProviderBundleAttestationKey(root.get("key"), field + ".key"),
        ProviderBundleAttestationDigest.fromBytes(
            digest(root.get("attestation_digest"), field + ".attestation_digest").bytes()),
        parseProviderBundleAttestation(root.get("attestation"), field + ".attestation"),
        string(root.get("registered_by"), field + ".registered_by"),
        nonZeroU64(root.get("registered_at_height"), field + ".registered_at_height"));
  }

  private static ProviderBundleVerificationAttestation parseProviderBundleAttestation(
      final Object value, final String field) {
    final Map<String, Object> root = exactObject(value, field, keys("payload", "approvals"));
    final Map<String, Object> payload =
        exactObject(root.get("payload"), field + ".payload", keys("version", "binding"));
    require(
        u8(payload.get("version"), field + ".payload.version") == 1,
        field + ".payload.version is unsupported in Musubi V1");
    final String bindingField = field + ".payload.binding";
    final Map<String, Object> binding =
        exactObject(
            payload.get("binding"),
            bindingField,
            keys(
                "network_id", "provider_id", "completed_by",
                "completion_authority", "replication_order", "assignment_revision",
                "completion_epoch", "finalized_anchor", "archive_id", "bundle_digest",
                "descriptor_digest", "semantic_release_manifest_digest",
                "verification_lock_digest", "source_tree_digest"));
    final String authorityField = bindingField + ".completion_authority";
    final Map<String, Object> authority =
        exactObject(
            binding.get("completion_authority"),
            authorityField,
            keys("provider_owner", "signer_policy"));
    final String signerField = authorityField + ".signer_policy";
    final Map<String, Object> signer =
        exactObject(
            authority.get("signer_policy"),
            signerField,
            keys("policy_id", "revision", "predecessor_digest", "policy_digest"));
    final String anchorField = bindingField + ".finalized_anchor";
    final Map<String, Object> anchor =
        exactObject(
            binding.get("finalized_anchor"),
            anchorField,
            keys("height", "block_hash"));
    final ProviderBundleVerificationBinding typedBinding =
        new ProviderBundleVerificationBinding(
            NetworkId.parse(string(binding.get("network_id"), bindingField + ".network_id")),
            newtypeText(binding.get("provider_id"), bindingField + ".provider_id"),
            string(binding.get("completed_by"), bindingField + ".completed_by"),
            new ProviderCompletionAuthority(
                string(authority.get("provider_owner"), authorityField + ".provider_owner"),
                new ProviderCompletionSignerPolicy(
                    fixedBytes(signer.get("policy_id"), signerField + ".policy_id"),
                    nonZeroU64(signer.get("revision"), signerField + ".revision"),
                    signer.get("predecessor_digest") == null
                        ? null
                        : fixedBytes(
                            signer.get("predecessor_digest"),
                            signerField + ".predecessor_digest"),
                    fixedBytes(signer.get("policy_digest"), signerField + ".policy_digest"))),
            digest(binding.get("replication_order"), bindingField + ".replication_order"),
            nonZeroU64(binding.get("assignment_revision"), bindingField + ".assignment_revision"),
            nonZeroU64(binding.get("completion_epoch"), bindingField + ".completion_epoch"),
            new ProviderFinalizedAnchor(
                nonZeroU64(anchor.get("height"), anchorField + ".height"),
                fixedBytes(anchor.get("block_hash"), anchorField + ".block_hash")),
            digest(binding.get("archive_id"), bindingField + ".archive_id"),
            digest(binding.get("bundle_digest"), bindingField + ".bundle_digest"),
            digest(binding.get("descriptor_digest"), bindingField + ".descriptor_digest"),
            digest(
                binding.get("semantic_release_manifest_digest"),
                bindingField + ".semantic_release_manifest_digest"),
            digest(
                binding.get("verification_lock_digest"),
                bindingField + ".verification_lock_digest"),
            digest(binding.get("source_tree_digest"), bindingField + ".source_tree_digest"));
    final List<Object> rawApprovals = list(root.get("approvals"), field + ".approvals");
    final List<ProviderBundleVerificationApproval> approvals = new ArrayList<>();
    for (int index = 0; index < rawApprovals.size(); index++) {
      final String approvalField = field + ".approvals[" + index + "]";
      final Map<String, Object> approval =
          exactObject(
              rawApprovals.get(index), approvalField, keys("public_key", "signature"));
      approvals.add(
          new ProviderBundleVerificationApproval(
              string(approval.get("public_key"), approvalField + ".public_key"),
              string(approval.get("signature"), approvalField + ".signature")));
    }
    return new ProviderBundleVerificationAttestation(
        new ProviderBundleVerificationPayload(typedBinding), approvals);
  }

  private static AliasRecord parseAliasRecord(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(value, field, keys("alias", "target", "registered_by", "pricing_revision", "paid_xor", "registered_at_height", "history_revision"));
    return new AliasRecord(
        newtypeText(root.get("alias"), field + ".alias"),
        parsePackage(root.get("target"), field + ".target"),
        string(root.get("registered_by"), field + ".registered_by"),
        nonZeroU64(root.get("pricing_revision"), field + ".pricing_revision"),
        u64(root.get("paid_xor"), field + ".paid_xor"),
        nonZeroU64(root.get("registered_at_height"), field + ".registered_at_height"),
        nonZeroU64(root.get("history_revision"), field + ".history_revision"));
  }

  private static AliasHistoryEntry parseAliasHistory(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(value, field, keys("alias", "revision", "action", "previous_target", "target", "governance_action", "finalized_height"));
    return new AliasHistoryEntry(
        newtypeText(root.get("alias"), field + ".alias"),
        nonZeroU64(root.get("revision"), field + ".revision"),
        taggedUnit(root.get("action"), field + ".action", keys("Registered", "ParliamentRetarget")),
        root.get("previous_target") == null ? null : parsePackage(root.get("previous_target"), field + ".previous_target"),
        parsePackage(root.get("target"), field + ".target"),
        root.get("governance_action") == null ? null : digest(root.get("governance_action"), field + ".governance_action"),
        nonZeroU64(root.get("finalized_height"), field + ".finalized_height"));
  }

  private static OrderedPackageEntry parseOrderedEntry(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(value, field, keys("selector", "package", "latest_selectable", "metadata_revision", "index_revision"));
    return new OrderedPackageEntry(
        parseSelector(root.get("selector"), field + ".selector"),
        parsePackage(root.get("package"), field + ".package"),
        root.get("latest_selectable") == null ? null : parseVersion(root.get("latest_selectable"), field + ".latest_selectable"),
        nonZeroU64(root.get("metadata_revision"), field + ".metadata_revision"),
        nonZeroU64(root.get("index_revision"), field + ".index_revision"));
  }

  private static SearchHit parseSearchHit(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value,
            field,
            keys(
                "package", "claimed_namespace", "description", "keywords",
                "metadata_revision"));
    final List<Object> rawKeywords = list(root.get("keywords"), field + ".keywords");
    final List<String> keywords = new ArrayList<>();
    for (int index = 0; index < rawKeywords.size(); index++) {
      keywords.add(newtypeText(rawKeywords.get(index), field + ".keywords[" + index + "]"));
    }
    return new SearchHit(
        parsePackage(root.get("package"), field + ".package"),
        new Namespace(newtypeText(root.get("claimed_namespace"), field + ".claimed_namespace")),
        root.get("description") == null
            ? null : newtypeText(root.get("description"), field + ".description"),
        keywords,
        nonZeroU64(root.get("metadata_revision"), field + ".metadata_revision"));
  }

  private interface ItemParser<T extends WireValue> {
    T parse(Object value, String field);
  }

  private static <T extends WireValue> Page<T> parsePage(
      final Object value,
      final String field,
      final String queryPath,
      final ItemParser<T> parser) {
    final Map<String, Object> root =
        exactObject(value, field, keys("query", "items", "next_cursor", "snapshot"));
    final List<Object> raw = list(root.get("items"), field + ".items");
    final List<T> items = new ArrayList<>();
    for (int index = 0; index < raw.size(); index++) {
      items.add(parser.parse(raw.get(index), field + ".items[" + index + "]"));
    }
    final RegistrySnapshot snapshot = parseSnapshot(root.get("snapshot"), field + ".snapshot");
    final FinalizedCursor cursor = root.get("next_cursor") == null
        ? null : parseCursor(root.get("next_cursor"), field + ".next_cursor");
    return new Page<>(decodeQuery(queryPath, root.get("query")), items, cursor, snapshot);
  }

  private static Digest32 digest(final Object value, final String field) {
    final List<Object> wrapper = list(value, field);
    require(wrapper.size() == 1, field + " must contain one Norito newtype item");
    return Digest32.fromBytes(fixedBytes(wrapper.get(0), field + "[0]"));
  }

  private static byte[] fixedBytes(final Object value, final String field) {
    return byteArray(value, field, 32);
  }

  private static byte[] byteArray(final Object value, final String field, final int size) {
    final List<Object> raw = list(value, field);
    require(raw.size() == size, field + " must contain exactly " + size + " bytes");
    final byte[] bytes = new byte[size];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] = (byte) u8(raw.get(index), field + "[" + index + "]");
    }
    return bytes;
  }

  private static String newtypeText(final Object value, final String field) {
    final List<Object> wrapper = list(value, field);
    require(wrapper.size() == 1, field + " must contain one Norito newtype item");
    return string(wrapper.get(0), field + "[0]");
  }

  private static Map<String, Object> tagged(final Object value, final String field) {
    return exactObject(value, field, keys("kind", "value"));
  }

  private static String taggedUnit(
      final Object value, final String field, final Set<String> allowed) {
    final Map<String, Object> root = tagged(value, field);
    final String kind = string(root.get("kind"), field + ".kind");
    require(allowed.contains(kind), field + ".kind is unsupported in Musubi V1");
    require(root.get("value") == null, field + ".value must be null");
    return kind;
  }

  private static Map<String, Object> exactObject(
      final Object value, final String field, final Set<String> keys) {
    final Map<String, Object> root = object(value, field);
    require(root.keySet().equals(keys), field + " contains unknown or missing fields");
    return root;
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> object(final Object value, final String field) {
    if (!(value instanceof Map<?, ?>)) throw new IllegalArgumentException(field + " must be an object");
    for (final Object key : ((Map<?, ?>) value).keySet()) {
      if (!(key instanceof String)) throw new IllegalArgumentException(field + " keys must be strings");
    }
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> list(final Object value, final String field) {
    if (!(value instanceof List<?>)) throw new IllegalArgumentException(field + " must be an array");
    return (List<Object>) value;
  }

  private static List<String> stringList(final Object value, final String field) {
    final List<Object> raw = list(value, field);
    final List<String> strings = new ArrayList<>();
    for (int index = 0; index < raw.size(); index++) {
      strings.add(string(raw.get(index), field + "[" + index + "]"));
    }
    return strings;
  }

  private static String string(final Object value, final String field) {
    if (!(value instanceof String)) throw new IllegalArgumentException(field + " must be a string");
    return (String) value;
  }

  private static boolean bool(final Object value, final String field) {
    if (!(value instanceof Boolean)) throw new IllegalArgumentException(field + " must be a boolean");
    return ((Boolean) value).booleanValue();
  }

  private static BigInteger integer(final Object value, final String field) {
    if (value instanceof BigInteger) return (BigInteger) value;
    if (value instanceof Byte || value instanceof Short || value instanceof Integer || value instanceof Long) {
      return BigInteger.valueOf(((Number) value).longValue());
    }
    throw new IllegalArgumentException(field + " must be an integer");
  }

  private static BigInteger u64(final Object value, final String field) {
    final BigInteger result = integer(value, field);
    MusubiModelsV1.requireU64(result, field);
    return result;
  }

  private static BigInteger nonZeroU64(final Object value, final String field) {
    final BigInteger result = u64(value, field);
    require(result.signum() > 0, field + " must be non-zero");
    return result;
  }

  private static long u32(final Object value, final String field) {
    final BigInteger result = integer(value, field);
    require(result.signum() >= 0 && result.compareTo(new BigInteger("4294967295")) <= 0,
        field + " must fit u32");
    return result.longValue();
  }

  private static int u16(final Object value, final String field) {
    final BigInteger result = integer(value, field);
    require(result.signum() >= 0 && result.compareTo(BigInteger.valueOf(65_535)) <= 0,
        field + " must fit u16");
    return result.intValue();
  }

  private static int u8(final Object value, final String field) {
    final BigInteger result = integer(value, field);
    require(result.signum() >= 0 && result.compareTo(BigInteger.valueOf(255)) <= 0,
        field + " must fit u8");
    return result.intValue();
  }

  private static Set<String> keys(final String... values) {
    return new LinkedHashSet<>(Arrays.asList(values));
  }

  private static void require(final boolean condition, final String message) {
    if (!condition) throw new IllegalArgumentException(message);
  }
}
