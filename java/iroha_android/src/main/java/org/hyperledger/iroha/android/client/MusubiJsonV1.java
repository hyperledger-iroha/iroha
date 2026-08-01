package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.hyperledger.iroha.android.client.MusubiModelsV1.AliasHistoryEntry;
import org.hyperledger.iroha.android.client.MusubiModelsV1.AliasQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.AliasRecord;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveLocation;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveLocationQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ComparatorOp;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Digest32;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ExactPackageQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ExactReleaseQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.FinalizedCursor;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Namespace;
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
import org.hyperledger.iroha.android.client.MusubiModelsV1.RegistrySnapshot;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ReleaseId;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ReleaseRecord;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ResolverIndexQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ResolverIndexPage;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ResolverReleaseRow;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Version;
import org.hyperledger.iroha.android.client.MusubiModelsV1.VersionComparator;
import org.hyperledger.iroha.android.client.MusubiModelsV1.VersionReq;
import org.hyperledger.iroha.android.client.MusubiModelsV1.WireValue;

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

  static ReleaseRecord parseExactRelease(final byte[] payload) {
    return parseReleaseRecord(parse(payload, "Musubi exact-release response"), "response");
  }

  static ResolverIndexPage parseResolverPage(final byte[] payload) {
    final Map<String, Object> root =
        exactObject(
            parse(payload, "Musubi resolver-index response"),
            "response",
            keys("chain_id", "genesis_hash", "items", "next_cursor", "snapshot"));
    final List<Object> raw = list(root.get("items"), "response.items");
    final List<ResolverReleaseRow> items = new ArrayList<>();
    for (int index = 0; index < raw.size(); index++) {
      items.add(parseResolverRow(raw.get(index), "response.items[" + index + "]"));
    }
    final RegistrySnapshot snapshot = parseSnapshot(root.get("snapshot"), "response.snapshot");
    final FinalizedCursor cursor = root.get("next_cursor") == null
        ? null : parseCursor(root.get("next_cursor"), "response.next_cursor");
    return new ResolverIndexPage(
        string(root.get("chain_id"), "response.chain_id"),
        fixedBytes(root.get("genesis_hash"), "response.genesis_hash"),
        items,
        cursor,
        snapshot);
  }

  static Page<Version> parseVersionPage(final byte[] payload) {
    return parsePage(parse(payload, "Musubi versions response"), "response", MusubiJsonV1::parseVersion);
  }

  static Page<PackageMember> parseMaintainerPage(final byte[] payload) {
    return parsePage(parse(payload, "Musubi maintainers response"), "response", MusubiJsonV1::parseMember);
  }

  static Page<ArchiveLocation> parseArchiveLocationPage(final byte[] payload) {
    return parsePage(parse(payload, "Musubi archive-locations response"), "response", MusubiJsonV1::parseArchiveLocation);
  }

  static AliasRecord parseAlias(final byte[] payload) {
    return parseAliasRecord(parse(payload, "Musubi alias response"), "response");
  }

  static Page<AliasHistoryEntry> parseAliasHistoryPage(final byte[] payload) {
    return parsePage(parse(payload, "Musubi alias-history response"), "response", MusubiJsonV1::parseAliasHistory);
  }

  static OrderedPrefixPage parseOrderedPackagePage(final byte[] payload) {
    final Map<String, Object> root =
        exactObject(
            parse(payload, "Musubi ordered-prefix response"),
            "response",
            keys("chain_id", "genesis_hash", "items", "next_cursor", "snapshot"));
    final List<Object> raw = list(root.get("items"), "response.items");
    final List<OrderedPackageEntry> items = new ArrayList<>();
    for (int index = 0; index < raw.size(); index++) {
      items.add(parseOrderedEntry(raw.get(index), "response.items[" + index + "]"));
    }
    final RegistrySnapshot snapshot = parseSnapshot(root.get("snapshot"), "response.snapshot");
    final FinalizedCursor cursor = root.get("next_cursor") == null
        ? null : parseCursor(root.get("next_cursor"), "response.next_cursor");
    return new OrderedPrefixPage(
        string(root.get("chain_id"), "response.chain_id"),
        fixedBytes(root.get("genesis_hash"), "response.genesis_hash"),
        items,
        cursor,
        snapshot);
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
      default: throw new IllegalArgumentException("unsupported Musubi V1 query path: " + path);
    }
  }

  static WireValue decodeResponse(final String path, final Object value) {
    final byte[] payload = JsonEncoder.encode(value).getBytes(StandardCharsets.UTF_8);
    switch (path) {
      case MusubiToriiClientV1.EXACT_PACKAGE_PATH: return parseExactPackage(payload);
      case MusubiToriiClientV1.EXACT_RELEASE_PATH: return parseExactRelease(payload);
      case MusubiToriiClientV1.RESOLVER_INDEX_PATH: return parseResolverPage(payload);
      case MusubiToriiClientV1.VERSIONS_PATH: return parseVersionPage(payload);
      case MusubiToriiClientV1.MAINTAINERS_PATH: return parseMaintainerPage(payload);
      case MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH: return parseArchiveLocationPage(payload);
      case MusubiToriiClientV1.ALIAS_PATH: return parseAlias(payload);
      case MusubiToriiClientV1.ALIAS_HISTORY_PATH: return parseAliasHistoryPage(payload);
      case MusubiToriiClientV1.ORDERED_PREFIX_PATH: return parseOrderedPackagePage(payload);
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
    final ReleaseId release = validateManifest(root.get("manifest"), field + ".manifest");
    digest(root.get("release_digest"), field + ".release_digest");
    final String publisher = string(root.get("published_by"), field + ".published_by");
    final BigInteger height = nonZeroU64(root.get("published_at_height"), field + ".published_at_height");
    validateYank(root.get("yank"), field + ".yank", release);
    validateGovernance(root.get("artifact_governance"), field + ".artifact_governance");
    final Map<String, Object> revisions = exactObject(root.get("revisions"), field + ".revisions", keys("yank", "artifact_governance"));
    nonZeroU64(revisions.get("yank"), field + ".revisions.yank");
    nonZeroU64(revisions.get("artifact_governance"), field + ".revisions.artifact_governance");
    return new ReleaseRecord(release, publisher, height, root);
  }

  private static ReleaseId validateManifest(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value, field,
            keys("release", "edition", "abi", "dependencies", "exports", "interface_digest",
                "metadata", "archive_id", "verification_lock_digest"));
    final ReleaseId release = parseRelease(root.get("release"), field + ".release");
    taggedUnit(root.get("edition"), field + ".edition", keys("V1"));
    validateAbi(root.get("abi"), field + ".abi");
    final List<Object> dependencies = list(root.get("dependencies"), field + ".dependencies");
    for (int index = 0; index < dependencies.size(); index++) {
      validateDependency(dependencies.get(index), field + ".dependencies[" + index + "]");
    }
    for (final String export : stringList(root.get("exports"), field + ".exports")) {
      MusubiModelsV1.requireName(export, "Musubi export");
    }
    digest(root.get("interface_digest"), field + ".interface_digest");
    validateMetadata(root.get("metadata"), field + ".metadata");
    digest(root.get("archive_id"), field + ".archive_id");
    digest(root.get("verification_lock_digest"), field + ".verification_lock_digest");
    return release;
  }

  private static void validateAbi(final Object value, final String field) {
    final Map<String, Object> root = exactObject(value, field, keys("abi_version", "abi_hash"));
    if (u16(root.get("abi_version"), field + ".abi_version") != 1) {
      throw new IllegalArgumentException(field + ".abi_version is unsupported; Musubi only supports V1");
    }
    fixedBytes(root.get("abi_hash"), field + ".abi_hash");
  }

  private static void validateDependency(final Object value, final String field) {
    final Map<String, Object> root = exactObject(value, field, keys("alias", "package", "requirement"));
    MusubiModelsV1.requireName(string(root.get("alias"), field + ".alias"), field + ".alias");
    parsePackage(root.get("package"), field + ".package");
    parseRequirement(root.get("requirement"), field + ".requirement");
  }

  private static void validateMetadata(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(value, field, keys("description", "readme", "license", "repository", "keywords"));
    for (final String key : Arrays.asList("description", "readme", "license", "repository")) {
      if (root.get(key) != null) newtypeText(root.get(key), field + "." + key);
    }
    final List<Object> keywords = list(root.get("keywords"), field + ".keywords");
    for (int index = 0; index < keywords.size(); index++) {
      MusubiModelsV1.requireAsciiKebab(
          newtypeText(keywords.get(index), field + ".keywords[" + index + "]"), 64, "keyword");
    }
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
      final Map<String, Object> payload = exactObject(root.get("value"), field + ".value", keys("action_digest", "reason", "enacted_at_height"));
      digest(payload.get("action_digest"), field + ".value.action_digest");
      newtypeText(payload.get("reason"), field + ".value.reason");
      nonZeroU64(payload.get("enacted_at_height"), field + ".value.enacted_at_height");
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
    validateAbi(root.get("abi"), field + ".abi");
    final List<Object> dependencies = list(root.get("dependencies"), field + ".dependencies");
    for (int index = 0; index < dependencies.size(); index++) {
      validateDependency(dependencies.get(index), field + ".dependencies[" + index + "]");
    }
    validateSelection(root.get("selection"), field + ".selection", release);
    final BigInteger revision = nonZeroU64(root.get("index_revision"), field + ".index_revision");
    return new ResolverReleaseRow(release, revision, root);
  }

  private static void validateSelection(final Object value, final String field, final ReleaseId release) {
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
    nonZeroU64(storage.get("index_revision"), field + ".storage.index_revision");
    validateGovernance(root.get("governance"), field + ".governance");
  }

  private static PackageMember parseMember(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(value, field, keys("package", "account", "role", "accepted_at_height", "governance_revision"));
    final Map<String, Object> role = tagged(root.get("role"), field + ".role");
    final String kind = string(role.get("kind"), field + ".role.kind");
    if ("Owner".equals(kind)) {
      require(role.get("value") == null, field + ".role.value must be null");
    } else if ("Maintainer".equals(kind)) {
      final Map<String, Object> permissions =
          exactObject(role.get("value"), field + ".role.value", keys("publish", "yank", "metadata", "archive_locations"));
      for (final Map.Entry<String, Object> entry : permissions.entrySet()) {
        bool(entry.getValue(), field + ".role.value." + entry.getKey());
      }
    } else {
      throw new IllegalArgumentException(field + ".role.kind is unsupported in Musubi V1");
    }
    return new PackageMember(
        parsePackage(root.get("package"), field + ".package"),
        string(root.get("account"), field + ".account"), kind, root);
  }

  private static ArchiveLocation parseArchiveLocation(final Object value, final String field) {
    final Map<String, Object> root =
        exactObject(
            value, field,
            keys("location_id", "archive_id", "pin_manifest", "replication_order", "providers",
                "provider_attestations", "renew_after_epoch", "expires_at_epoch", "finalized_height",
                "revision", "state"));
    final Digest32 locationId = digest(root.get("location_id"), field + ".location_id");
    final Digest32 archiveId = digest(root.get("archive_id"), field + ".archive_id");
    require(root.get("pin_manifest") != null && root.get("replication_order") != null,
        field + " must carry SoraFS pin and order identities");
    list(root.get("providers"), field + ".providers");
    list(root.get("provider_attestations"), field + ".provider_attestations");
    u64(root.get("renew_after_epoch"), field + ".renew_after_epoch");
    u64(root.get("expires_at_epoch"), field + ".expires_at_epoch");
    nonZeroU64(root.get("finalized_height"), field + ".finalized_height");
    final BigInteger revision = nonZeroU64(root.get("revision"), field + ".revision");
    final String state = taggedUnit(root.get("state"), field + ".state", keys("Pending", "Healthy", "Degraded", "Retired"));
    return new ArchiveLocation(locationId, archiveId, revision, state, root);
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

  private interface ItemParser<T extends WireValue> {
    T parse(Object value, String field);
  }

  private static <T extends WireValue> Page<T> parsePage(
      final Object value, final String field, final ItemParser<T> parser) {
    final Map<String, Object> root = exactObject(value, field, keys("items", "next_cursor", "snapshot"));
    final List<Object> raw = list(root.get("items"), field + ".items");
    final List<T> items = new ArrayList<>();
    for (int index = 0; index < raw.size(); index++) {
      items.add(parser.parse(raw.get(index), field + ".items[" + index + "]"));
    }
    final RegistrySnapshot snapshot = parseSnapshot(root.get("snapshot"), field + ".snapshot");
    final FinalizedCursor cursor = root.get("next_cursor") == null
        ? null : parseCursor(root.get("next_cursor"), field + ".next_cursor");
    return new Page<>(items, cursor, snapshot);
  }

  private static Digest32 digest(final Object value, final String field) {
    final List<Object> wrapper = list(value, field);
    require(wrapper.size() == 1, field + " must contain one Norito newtype item");
    return Digest32.fromBytes(fixedBytes(wrapper.get(0), field + "[0]"));
  }

  private static byte[] fixedBytes(final Object value, final String field) {
    final List<Object> raw = list(value, field);
    require(raw.size() == 32, field + " must contain exactly 32 bytes");
    final byte[] bytes = new byte[32];
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
