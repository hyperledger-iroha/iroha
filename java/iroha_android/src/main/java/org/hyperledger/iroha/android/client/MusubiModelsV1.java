package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.text.Normalizer;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

/** First-release-only Musubi package, SemVer, query, cursor, and page DTOs. */
public final class MusubiModelsV1 {
  static final BigInteger U64_MAX = new BigInteger("18446744073709551615");

  private MusubiModelsV1() {}

  /** Base for values encoded through the exact Norito JSON representation. */
  public abstract static class WireValue {
    abstract Object toJsonValue();

    public final byte[] toJsonBytes() {
      return JsonEncoder.encode(toJsonValue()).getBytes(StandardCharsets.UTF_8);
    }

    @Override
    public final boolean equals(final Object other) {
      return other != null
          && getClass() == other.getClass()
          && Objects.equals(toJsonValue(), ((WireValue) other).toJsonValue());
    }

    @Override
    public final int hashCode() {
      return Objects.hashCode(toJsonValue());
    }

    @Override
    public final String toString() {
      return new String(toJsonBytes(), StandardCharsets.UTF_8);
    }
  }

  /** Canonical human-facing namespace. */
  public static final class Namespace extends WireValue {
    private final String value;

    public Namespace(final String value) {
      requireNamespace(value);
      this.value = value;
    }

    public String value() { return value; }

    @Override Object toJsonValue() { return Collections.singletonList(value); }
  }

  /** Canonical lowercase ASCII kebab package name. */
  public static final class PackageName extends WireValue {
    private final String value;

    public PackageName(final String value) {
      requireAsciiKebab(value, 64, "package name");
      this.value = value;
    }

    public String value() { return value; }

    @Override Object toJsonValue() { return Collections.singletonList(value); }
  }

  /** Structural package scope. */
  public static final class PackageScope extends WireValue {
    public enum Kind { DATASPACE_ROOT, DOMAIN }

    private final Kind kind;
    private final String domain;

    private PackageScope(final Kind kind, final String domain) {
      this.kind = Objects.requireNonNull(kind, "kind");
      this.domain = domain;
      if (kind == Kind.DATASPACE_ROOT && domain != null) {
        throw new IllegalArgumentException("dataspace-root scope must not carry a domain");
      }
      if (kind == Kind.DOMAIN) requireName(domain, "package scope domain");
    }

    public static PackageScope dataspaceRoot() {
      return new PackageScope(Kind.DATASPACE_ROOT, null);
    }

    public static PackageScope domain(final String value) {
      return new PackageScope(Kind.DOMAIN, value);
    }

    public Kind kind() { return kind; }
    public String domain() { return domain; }

    @Override Object toJsonValue() {
      return object(
          "kind", kind == Kind.DATASPACE_ROOT ? "DataspaceRoot" : "Domain",
          "value", domain);
    }
  }

  /** Stable structural package identity. */
  public static final class PackageId extends WireValue {
    private final BigInteger homeDataspace;
    private final PackageScope scope;
    private final PackageName name;

    public PackageId(
        final BigInteger homeDataspace, final PackageScope scope, final PackageName name) {
      requireU64(homeDataspace, "homeDataspace");
      this.homeDataspace = homeDataspace;
      this.scope = Objects.requireNonNull(scope, "scope");
      this.name = Objects.requireNonNull(name, "name");
    }

    public PackageId(final long homeDataspace, final PackageScope scope, final PackageName name) {
      this(BigInteger.valueOf(homeDataspace), scope, name);
    }

    public BigInteger homeDataspace() { return homeDataspace; }
    public PackageScope scope() { return scope; }
    public PackageName name() { return name; }

    @Override Object toJsonValue() {
      return object(
          "home_dataspace", homeDataspace,
          "scope", scope.toJsonValue(),
          "name", name.toJsonValue());
    }
  }

  /** Public namespace/package selector. */
  public static final class PackageSelector extends WireValue {
    private final Namespace namespace;
    private final PackageName name;

    public PackageSelector(final Namespace namespace, final PackageName name) {
      this.namespace = Objects.requireNonNull(namespace, "namespace");
      this.name = Objects.requireNonNull(name, "name");
    }

    public Namespace namespace() { return namespace; }
    public PackageName name() { return name; }

    @Override Object toJsonValue() {
      return object("namespace", namespace.toJsonValue(), "name", name.toJsonValue());
    }
  }

  /** One SemVer prerelease identifier. */
  public static final class PrereleaseIdentifier extends WireValue
      implements Comparable<PrereleaseIdentifier> {
    private final BigInteger numeric;
    private final String alphaNumeric;

    private PrereleaseIdentifier(final BigInteger numeric, final String alphaNumeric) {
      if ((numeric == null) == (alphaNumeric == null)) {
        throw new IllegalArgumentException("prerelease identifier must have one representation");
      }
      if (numeric != null) requireU64(numeric, "numeric prerelease identifier");
      if (alphaNumeric != null) {
        if (alphaNumeric.isEmpty()
            || alphaNumeric.getBytes(StandardCharsets.UTF_8).length > 64
            || alphaNumeric.matches("[0-9]+")
            || !alphaNumeric.matches("[A-Za-z0-9-]+")) {
          throw new IllegalArgumentException("alphanumeric prerelease identifier is noncanonical");
        }
      }
      this.numeric = numeric;
      this.alphaNumeric = alphaNumeric;
    }

    public static PrereleaseIdentifier numeric(final BigInteger value) {
      return new PrereleaseIdentifier(value, null);
    }

    public static PrereleaseIdentifier alphaNumeric(final String value) {
      return new PrereleaseIdentifier(null, value);
    }

    static PrereleaseIdentifier parse(final String value) {
      if (value == null || value.isEmpty()) {
        throw new IllegalArgumentException("prerelease identifier must not be empty");
      }
      if (value.matches("[0-9]+")) {
        if (value.length() > 1 && value.charAt(0) == '0') {
          throw new IllegalArgumentException("numeric prerelease identifier has a leading zero");
        }
        return numeric(parseU64(value, "prerelease identifier"));
      }
      return alphaNumeric(value);
    }

    public BigInteger numeric() { return numeric; }
    public String alphaNumeric() { return alphaNumeric; }
    public String canonicalText() { return numeric == null ? alphaNumeric : numeric.toString(); }

    @Override Object toJsonValue() {
      return object(
          "kind", numeric == null ? "AlphaNumeric" : "Numeric",
          "value", numeric == null ? alphaNumeric : numeric);
    }

    @Override public int compareTo(final PrereleaseIdentifier other) {
      if (numeric != null && other.numeric != null) return numeric.compareTo(other.numeric);
      if (numeric != null) return -1;
      if (other.numeric != null) return 1;
      return alphaNumeric.compareTo(other.alphaNumeric);
    }
  }

  /** Structured canonical SemVer without build metadata. */
  public static final class Version extends WireValue implements Comparable<Version> {
    private final BigInteger major;
    private final BigInteger minor;
    private final BigInteger patch;
    private final List<PrereleaseIdentifier> prerelease;

    public Version(
        final BigInteger major,
        final BigInteger minor,
        final BigInteger patch,
        final List<PrereleaseIdentifier> prerelease) {
      requireU64(major, "version.major");
      requireU64(minor, "version.minor");
      requireU64(patch, "version.patch");
      this.major = major;
      this.minor = minor;
      this.patch = patch;
      this.prerelease = immutableList(prerelease);
      if (this.prerelease.size() > 16) {
        throw new IllegalArgumentException("version has too many prerelease identifiers");
      }
    }

    public Version(final long major, final long minor, final long patch) {
      this(
          BigInteger.valueOf(major),
          BigInteger.valueOf(minor),
          BigInteger.valueOf(patch),
          Collections.emptyList());
    }

    public static Version parse(final String value) {
      requireExactText(value, "Musubi version");
      if (value.indexOf('+') >= 0) {
        throw new IllegalArgumentException("Musubi V1 versions do not permit build metadata");
      }
      final String[] split = value.split("-", 2);
      final String[] core = split[0].split("\\.", -1);
      if (core.length != 3) {
        throw new IllegalArgumentException("Musubi version must use MAJOR.MINOR.PATCH");
      }
      final List<PrereleaseIdentifier> identifiers = new ArrayList<>();
      if (split.length == 2) {
        if (split[1].isEmpty()) throw new IllegalArgumentException("empty Musubi prerelease");
        for (final String identifier : split[1].split("\\.", -1)) {
          identifiers.add(PrereleaseIdentifier.parse(identifier));
        }
      }
      return new Version(
          parseCanonicalU64(core[0], "version.major"),
          parseCanonicalU64(core[1], "version.minor"),
          parseCanonicalU64(core[2], "version.patch"),
          identifiers);
    }

    public BigInteger major() { return major; }
    public BigInteger minor() { return minor; }
    public BigInteger patch() { return patch; }
    public List<PrereleaseIdentifier> prerelease() { return prerelease; }

    public String canonicalText() {
      final StringBuilder out = new StringBuilder()
          .append(major).append('.').append(minor).append('.').append(patch);
      if (!prerelease.isEmpty()) {
        out.append('-');
        for (int index = 0; index < prerelease.size(); index++) {
          if (index > 0) out.append('.');
          out.append(prerelease.get(index).canonicalText());
        }
      }
      return out.toString();
    }

    @Override Object toJsonValue() {
      final List<Object> identifiers = new ArrayList<>();
      for (final PrereleaseIdentifier item : prerelease) identifiers.add(item.toJsonValue());
      return object(
          "major", major,
          "minor", minor,
          "patch", patch,
          "prerelease", identifiers);
    }

    @Override public int compareTo(final Version other) {
      int result = major.compareTo(other.major);
      if (result != 0) return result;
      result = minor.compareTo(other.minor);
      if (result != 0) return result;
      result = patch.compareTo(other.patch);
      if (result != 0) return result;
      if (prerelease.isEmpty() && !other.prerelease.isEmpty()) return 1;
      if (!prerelease.isEmpty() && other.prerelease.isEmpty()) return -1;
      for (int index = 0; index < Math.min(prerelease.size(), other.prerelease.size()); index++) {
        result = prerelease.get(index).compareTo(other.prerelease.get(index));
        if (result != 0) return result;
      }
      return Integer.compare(prerelease.size(), other.prerelease.size());
    }
  }

  /** Comparator operator in a canonical requirement. */
  public enum ComparatorOp {
    GREATER("Greater", ">"),
    GREATER_OR_EQUAL("GreaterOrEqual", ">="),
    LESS("Less", "<"),
    LESS_OR_EQUAL("LessOrEqual", "<="),
    EQUAL("Equal", "=");

    private final String wireName;
    private final String token;

    ComparatorOp(final String wireName, final String token) {
      this.wireName = wireName;
      this.token = token;
    }

    public String wireName() { return wireName; }
    public String token() { return token; }
    Object toJsonValue() { return object("kind", wireName, "value", null); }
  }

  /** One comparator AST node. */
  public static final class VersionComparator extends WireValue
      implements Comparable<VersionComparator> {
    private final ComparatorOp op;
    private final Version version;

    public VersionComparator(final ComparatorOp op, final Version version) {
      this.op = Objects.requireNonNull(op, "op");
      this.version = Objects.requireNonNull(version, "version");
    }

    public ComparatorOp op() { return op; }
    public Version version() { return version; }

    @Override Object toJsonValue() {
      return object("op", op.toJsonValue(), "version", version.toJsonValue());
    }

    @Override public int compareTo(final VersionComparator other) {
      final int byOp = Integer.compare(op.ordinal(), other.op.ordinal());
      return byOp == 0 ? version.compareTo(other.version) : byOp;
    }
  }

  /** Canonical Cargo-style version requirement AST. */
  public static final class VersionReq extends WireValue {
    public enum Kind {
      ANY, CARET, TILDE, MAJOR_WILDCARD, MINOR_WILDCARD, EXACT, COMPARATORS
    }

    private final Kind kind;
    private final Version version;
    private final BigInteger major;
    private final BigInteger minor;
    private final List<VersionComparator> comparators;

    private VersionReq(
        final Kind kind,
        final Version version,
        final BigInteger major,
        final BigInteger minor,
        final List<VersionComparator> comparators) {
      this.kind = Objects.requireNonNull(kind, "kind");
      this.version = version;
      this.major = major;
      this.minor = minor;
      this.comparators = immutableList(comparators);
      if (kind == Kind.COMPARATORS && this.comparators.isEmpty()) {
        throw new IllegalArgumentException("Musubi comparator requirement must not be empty");
      }
      if (this.comparators.size() > 16) {
        throw new IllegalArgumentException("Musubi requirement has too many comparators");
      }
      for (int index = 1; index < this.comparators.size(); index++) {
        if (this.comparators.get(index - 1).compareTo(this.comparators.get(index)) >= 0) {
          throw new IllegalArgumentException("Musubi comparators must be sorted and distinct");
        }
      }
    }

    public static VersionReq parse(final String value) {
      requireExactText(value, "Musubi version requirement");
      if ("*".equals(value)) return fromWire(Kind.ANY, null, null, null, Collections.emptyList());
      if (value.startsWith("=") && value.indexOf(',') < 0) {
        return fromWire(Kind.EXACT, Version.parse(value.substring(1)), null, null, Collections.emptyList());
      }
      if (value.indexOf(',') >= 0 || value.startsWith(">") || value.startsWith("<")) {
        final Set<VersionComparator> unique = new LinkedHashSet<>();
        for (final String item : value.split(",", -1)) unique.add(parseComparator(item.trim()));
        final List<VersionComparator> sorted = new ArrayList<>(unique);
        Collections.sort(sorted);
        return fromWire(Kind.COMPARATORS, null, null, null, sorted);
      }
      if (value.startsWith("^")) {
        return fromWire(Kind.CARET, Version.parse(value.substring(1)), null, null, Collections.emptyList());
      }
      if (value.startsWith("~")) {
        return fromWire(Kind.TILDE, Version.parse(value.substring(1)), null, null, Collections.emptyList());
      }
      if (value.endsWith(".*")) {
        final String[] parts = value.substring(0, value.length() - 2).split("\\.", -1);
        if (parts.length == 1) {
          return fromWire(
              Kind.MAJOR_WILDCARD, null,
              parseCanonicalU64(parts[0], "wildcard major"), null, Collections.emptyList());
        }
        if (parts.length == 2) {
          return fromWire(
              Kind.MINOR_WILDCARD, null,
              parseCanonicalU64(parts[0], "wildcard major"),
              parseCanonicalU64(parts[1], "wildcard minor"), Collections.emptyList());
        }
        throw new IllegalArgumentException("Musubi wildcard must be MAJOR.* or MAJOR.MINOR.*");
      }
      return fromWire(Kind.CARET, Version.parse(value), null, null, Collections.emptyList());
    }

    static VersionReq fromWire(
        final Kind kind,
        final Version version,
        final BigInteger major,
        final BigInteger minor,
        final List<VersionComparator> comparators) {
      return new VersionReq(kind, version, major, minor, comparators);
    }

    private static VersionComparator parseComparator(final String value) {
      final ComparatorOp op;
      final String version;
      if (value.startsWith(">=")) {
        op = ComparatorOp.GREATER_OR_EQUAL; version = value.substring(2);
      } else if (value.startsWith("<=")) {
        op = ComparatorOp.LESS_OR_EQUAL; version = value.substring(2);
      } else if (value.startsWith(">")) {
        op = ComparatorOp.GREATER; version = value.substring(1);
      } else if (value.startsWith("<")) {
        op = ComparatorOp.LESS; version = value.substring(1);
      } else if (value.startsWith("=")) {
        op = ComparatorOp.EQUAL; version = value.substring(1);
      } else {
        throw new IllegalArgumentException("Musubi comparator has no supported operator");
      }
      return new VersionComparator(op, Version.parse(version));
    }

    public Kind kind() { return kind; }
    public Version version() { return version; }
    public BigInteger major() { return major; }
    public BigInteger minor() { return minor; }
    public List<VersionComparator> comparators() { return comparators; }

    public String canonicalText() {
      switch (kind) {
        case ANY: return "*";
        case CARET: return "^" + version.canonicalText();
        case TILDE: return "~" + version.canonicalText();
        case MAJOR_WILDCARD: return major + ".*";
        case MINOR_WILDCARD: return major + "." + minor + ".*";
        case EXACT: return "=" + version.canonicalText();
        case COMPARATORS:
          final StringBuilder out = new StringBuilder();
          for (int index = 0; index < comparators.size(); index++) {
            if (index > 0) out.append(',');
            out.append(comparators.get(index).op.token())
                .append(comparators.get(index).version.canonicalText());
          }
          return out.toString();
        default: throw new IllegalStateException("unreachable Musubi requirement kind");
      }
    }

    @Override Object toJsonValue() {
      final String wireName;
      final Object payload;
      switch (kind) {
        case ANY: wireName = "Any"; payload = null; break;
        case CARET: wireName = "Caret"; payload = version.toJsonValue(); break;
        case TILDE: wireName = "Tilde"; payload = version.toJsonValue(); break;
        case MAJOR_WILDCARD: wireName = "MajorWildcard"; payload = major; break;
        case MINOR_WILDCARD:
          wireName = "MinorWildcard"; payload = object("major", major, "minor", minor); break;
        case EXACT: wireName = "Exact"; payload = version.toJsonValue(); break;
        case COMPARATORS:
          wireName = "Comparators";
          final List<Object> values = new ArrayList<>();
          for (final VersionComparator comparator : comparators) values.add(comparator.toJsonValue());
          payload = values;
          break;
        default: throw new IllegalStateException("unreachable Musubi requirement kind");
      }
      return object("kind", wireName, "value", payload);
    }
  }

  /** Exact release identity. */
  public static final class ReleaseId extends WireValue {
    private final PackageId packageId;
    private final Version version;

    public ReleaseId(final PackageId packageId, final Version version) {
      this.packageId = Objects.requireNonNull(packageId, "packageId");
      this.version = Objects.requireNonNull(version, "version");
    }

    public PackageId packageId() { return packageId; }
    public Version version() { return version; }

    @Override Object toJsonValue() {
      return object("package", packageId.toJsonValue(), "version", version.toJsonValue());
    }
  }

  /** Canonical one-field Norito JSON wrapper around a 32-byte digest. */
  public static final class Digest32 extends WireValue {
    private final byte[] bytes;

    private Digest32(final byte[] bytes) {
      if (bytes == null || bytes.length != 32) {
        throw new IllegalArgumentException("Musubi digest must contain exactly 32 bytes");
      }
      this.bytes = bytes.clone();
    }

    public static Digest32 fromBytes(final byte[] bytes) { return new Digest32(bytes); }

    public static Digest32 fromHex(final String hex) {
      String body = hex;
      if (body.startsWith("0x") || body.startsWith("0X")) body = body.substring(2);
      if (body.length() != 64 || !body.matches("[0-9a-fA-F]{64}")) {
        throw new IllegalArgumentException("Musubi digest must be 64 hexadecimal characters");
      }
      final byte[] bytes = new byte[32];
      for (int index = 0; index < bytes.length; index++) {
        bytes[index] = (byte) Integer.parseInt(body.substring(index * 2, index * 2 + 2), 16);
      }
      return new Digest32(bytes);
    }

    public byte[] bytes() { return bytes.clone(); }

    @Override Object toJsonValue() {
      final List<Integer> values = new ArrayList<>(32);
      for (final byte item : bytes) values.add(item & 0xff);
      return Collections.singletonList(values);
    }
  }

  /** Finalized universal registry snapshot. */
  public static final class RegistrySnapshot extends WireValue {
    private final BigInteger finalizedHeight;
    private final byte[] finalizedBlockHash;
    private final BigInteger indexRevision;

    public RegistrySnapshot(
        final BigInteger finalizedHeight,
        final byte[] finalizedBlockHash,
        final BigInteger indexRevision) {
      requireU64(finalizedHeight, "snapshot.finalizedHeight");
      requireU64(indexRevision, "snapshot.indexRevision");
      if (finalizedHeight.signum() == 0 || indexRevision.signum() == 0) {
        throw new IllegalArgumentException("Musubi snapshot anchors must be non-zero");
      }
      if (finalizedBlockHash == null || finalizedBlockHash.length != 32) {
        throw new IllegalArgumentException("Musubi finalized block hash must contain 32 bytes");
      }
      this.finalizedHeight = finalizedHeight;
      this.finalizedBlockHash = finalizedBlockHash.clone();
      this.indexRevision = indexRevision;
    }

    public BigInteger finalizedHeight() { return finalizedHeight; }
    public byte[] finalizedBlockHash() { return finalizedBlockHash.clone(); }
    public BigInteger indexRevision() { return indexRevision; }

    @Override Object toJsonValue() {
      final List<Integer> hash = new ArrayList<>(32);
      for (final byte item : finalizedBlockHash) hash.add(item & 0xff);
      return object(
          "finalized_height", finalizedHeight,
          "finalized_block_hash", hash,
          "index_revision", indexRevision);
    }
  }

  /** Finalized query cursor. */
  public static final class FinalizedCursor extends WireValue {
    private final RegistrySnapshot snapshot;
    private final Digest32 queryHash;
    private final String lastKey;
    private final String caller;

    public FinalizedCursor(
        final RegistrySnapshot snapshot,
        final Digest32 queryHash,
        final String lastKey,
        final String caller) {
      this.snapshot = Objects.requireNonNull(snapshot, "snapshot");
      this.queryHash = Objects.requireNonNull(queryHash, "queryHash");
      requireExactText(lastKey, "Musubi cursor last key");
      if (lastKey.getBytes(StandardCharsets.UTF_8).length > 512) {
        throw new IllegalArgumentException("Musubi cursor last key exceeds 512 bytes");
      }
      if (caller != null) requireExactText(caller, "Musubi cursor caller");
      this.lastKey = lastKey;
      this.caller = caller;
    }

    public RegistrySnapshot snapshot() { return snapshot; }
    public Digest32 queryHash() { return queryHash; }
    public String lastKey() { return lastKey; }
    public String caller() { return caller; }

    @Override Object toJsonValue() {
      return object(
          "snapshot", snapshot.toJsonValue(),
          "query_hash", queryHash.toJsonValue(),
          "last_key", lastKey,
          "caller", caller);
    }
  }

  /** Shared bounded page request. */
  public static final class PageRequest extends WireValue {
    private final long limit;
    private final FinalizedCursor cursor;

    public PageRequest() { this(50L, null); }

    public PageRequest(final long limit, final FinalizedCursor cursor) {
      if (limit < 0 || limit > 4_294_967_295L) {
        throw new IllegalArgumentException("Musubi page limit must fit u32");
      }
      this.limit = limit;
      this.cursor = cursor;
    }

    public long limit() { return limit; }
    public FinalizedCursor cursor() { return cursor; }

    @Override Object toJsonValue() {
      return object("limit", limit, "cursor", cursor == null ? null : cursor.toJsonValue());
    }
  }

  /** Exact package query. */
  public static final class ExactPackageQuery extends WireValue {
    private final PackageId packageId;
    public ExactPackageQuery(final PackageId packageId) {
      this.packageId = Objects.requireNonNull(packageId, "packageId");
    }
    public PackageId packageId() { return packageId; }
    @Override Object toJsonValue() { return object("package", packageId.toJsonValue()); }
  }

  /** Exact release query. */
  public static final class ExactReleaseQuery extends WireValue {
    private final ReleaseId release;
    public ExactReleaseQuery(final ReleaseId release) {
      this.release = Objects.requireNonNull(release, "release");
    }
    public ReleaseId release() { return release; }
    @Override Object toJsonValue() { return object("release", release.toJsonValue()); }
  }

  /** Resolver-index range query. */
  public static final class ResolverIndexQuery extends WireValue {
    private final PackageId packageId;
    private final VersionReq requirement;
    private final PageRequest page;
    public ResolverIndexQuery(
        final PackageId packageId, final VersionReq requirement, final PageRequest page) {
      this.packageId = Objects.requireNonNull(packageId, "packageId");
      this.requirement = requirement;
      this.page = Objects.requireNonNull(page, "page");
    }
    public PackageId packageId() { return packageId; }
    public VersionReq requirement() { return requirement; }
    public PageRequest page() { return page; }
    @Override Object toJsonValue() {
      return object(
          "package", packageId.toJsonValue(),
          "requirement", requirement == null ? null : requirement.toJsonValue(),
          "page", page.toJsonValue());
    }
  }

  /** Package-scoped page query used by versions and maintainers. */
  public static final class PackagePageQuery extends WireValue {
    private final PackageId packageId;
    private final PageRequest page;
    public PackagePageQuery(final PackageId packageId, final PageRequest page) {
      this.packageId = Objects.requireNonNull(packageId, "packageId");
      this.page = Objects.requireNonNull(page, "page");
    }
    public PackageId packageId() { return packageId; }
    public PageRequest page() { return page; }
    @Override Object toJsonValue() {
      return object("package", packageId.toJsonValue(), "page", page.toJsonValue());
    }
  }

  /** Archive-location page query. */
  public static final class ArchiveLocationQuery extends WireValue {
    private final Digest32 archiveId;
    private final PageRequest page;
    public ArchiveLocationQuery(final Digest32 archiveId, final PageRequest page) {
      this.archiveId = Objects.requireNonNull(archiveId, "archiveId");
      this.page = Objects.requireNonNull(page, "page");
    }
    public Digest32 archiveId() { return archiveId; }
    public PageRequest page() { return page; }
    @Override Object toJsonValue() {
      return object("archive_id", archiveId.toJsonValue(), "page", page.toJsonValue());
    }
  }

  /** Exact alias or alias-history query. */
  public static final class AliasQuery extends WireValue {
    private final String alias;
    private final PageRequest page;
    public AliasQuery(final String alias, final PageRequest page) {
      requireAsciiKebab(alias, 32, "alias");
      this.alias = alias;
      this.page = Objects.requireNonNull(page, "page");
    }
    public String alias() { return alias; }
    public PageRequest page() { return page; }
    @Override Object toJsonValue() {
      return object("alias", Collections.singletonList(alias), "page", page.toJsonValue());
    }
  }

  /** Deterministic ordered-prefix query. */
  public static final class OrderedPrefixQuery extends WireValue {
    private final String prefix;
    private final PageRequest page;
    public OrderedPrefixQuery(final String prefix, final PageRequest page) {
      requireExactText(prefix, "Musubi ordered prefix");
      if (prefix.getBytes(StandardCharsets.UTF_8).length > 512) {
        throw new IllegalArgumentException("Musubi ordered prefix exceeds 512 bytes");
      }
      this.prefix = prefix;
      this.page = Objects.requireNonNull(page, "page");
    }
    public String prefix() { return prefix; }
    public PageRequest page() { return page; }
    @Override Object toJsonValue() {
      return object("prefix", Collections.singletonList(prefix), "page", page.toJsonValue());
    }
  }

  /** Package compare-and-set revisions. */
  public static final class PackageRevisions extends WireValue {
    private final BigInteger governance;
    private final BigInteger metadata;
    private final BigInteger archiveLocations;
    public PackageRevisions(
        final BigInteger governance,
        final BigInteger metadata,
        final BigInteger archiveLocations) {
      for (final BigInteger value : Arrays.asList(governance, metadata, archiveLocations)) {
        requireU64(value, "package revision");
        if (value.signum() == 0) throw new IllegalArgumentException("package revisions must be non-zero");
      }
      this.governance = governance;
      this.metadata = metadata;
      this.archiveLocations = archiveLocations;
    }
    public BigInteger governance() { return governance; }
    public BigInteger metadata() { return metadata; }
    public BigInteger archiveLocations() { return archiveLocations; }
    @Override Object toJsonValue() {
      return object(
          "governance", governance,
          "metadata", metadata,
          "archive_locations", archiveLocations);
    }
  }

  /** Authoritative exact package response. */
  public static final class PackageRecord extends WireValue {
    private final PackageId packageId;
    private final Namespace claimedNamespace;
    private final Digest32 claimedNamespaceBinding;
    private final List<String> owners;
    private final List<String> memberAccounts;
    private final BigInteger claimedAtHeight;
    private final PackageRevisions revisions;
    public PackageRecord(
        final PackageId packageId,
        final Namespace claimedNamespace,
        final Digest32 claimedNamespaceBinding,
        final List<String> owners,
        final List<String> memberAccounts,
        final BigInteger claimedAtHeight,
        final PackageRevisions revisions) {
      this.packageId = Objects.requireNonNull(packageId, "packageId");
      this.claimedNamespace = Objects.requireNonNull(claimedNamespace, "claimedNamespace");
      this.claimedNamespaceBinding = Objects.requireNonNull(claimedNamespaceBinding, "binding");
      this.owners = immutableList(owners);
      this.memberAccounts = immutableList(memberAccounts);
      if (this.owners.isEmpty()) throw new IllegalArgumentException("package must retain an owner");
      requireU64(claimedAtHeight, "claimedAtHeight");
      this.claimedAtHeight = claimedAtHeight;
      this.revisions = Objects.requireNonNull(revisions, "revisions");
    }
    public PackageId packageId() { return packageId; }
    public Namespace claimedNamespace() { return claimedNamespace; }
    public Digest32 claimedNamespaceBinding() { return claimedNamespaceBinding; }
    public List<String> owners() { return owners; }
    public List<String> memberAccounts() { return memberAccounts; }
    public BigInteger claimedAtHeight() { return claimedAtHeight; }
    public PackageRevisions revisions() { return revisions; }
    @Override Object toJsonValue() {
      return object(
          "package", packageId.toJsonValue(),
          "claimed_namespace", claimedNamespace.toJsonValue(),
          "claimed_namespace_binding", claimedNamespaceBinding.toJsonValue(),
          "owners", owners,
          "member_accounts", memberAccounts,
          "claimed_at_height", claimedAtHeight,
          "revisions", revisions.toJsonValue());
    }
  }

  abstract static class StrictRecord extends WireValue {
    private final Map<String, Object> raw;
    StrictRecord(final Map<String, Object> raw) { this.raw = immutableObject(raw); }
    @Override final Object toJsonValue() { return raw; }
  }

  /** Exact release response. */
  public static final class ReleaseRecord extends StrictRecord {
    private final ReleaseId release;
    private final String publishedBy;
    private final BigInteger publishedAtHeight;
    ReleaseRecord(
        final ReleaseId release,
        final String publishedBy,
        final BigInteger publishedAtHeight,
        final Map<String, Object> raw) {
      super(raw); this.release = release; this.publishedBy = publishedBy;
      this.publishedAtHeight = publishedAtHeight;
    }
    public ReleaseId release() { return release; }
    public String publishedBy() { return publishedBy; }
    public BigInteger publishedAtHeight() { return publishedAtHeight; }
  }

  /** Compact resolver row response. */
  public static final class ResolverReleaseRow extends StrictRecord {
    private final ReleaseId release;
    private final BigInteger indexRevision;
    ResolverReleaseRow(
        final ReleaseId release, final BigInteger indexRevision, final Map<String, Object> raw) {
      super(raw); this.release = release; this.indexRevision = indexRevision;
    }
    public ReleaseId release() { return release; }
    public BigInteger indexRevision() { return indexRevision; }
  }

  /** Accepted package member response. */
  public static final class PackageMember extends StrictRecord {
    private final PackageId packageId;
    private final String account;
    private final String roleKind;
    PackageMember(
        final PackageId packageId,
        final String account,
        final String roleKind,
        final Map<String, Object> raw) {
      super(raw); this.packageId = packageId; this.account = account; this.roleKind = roleKind;
    }
    public PackageId packageId() { return packageId; }
    public String account() { return account; }
    public String roleKind() { return roleKind; }
  }

  /** Renewable archive-location response. */
  public static final class ArchiveLocation extends StrictRecord {
    private final Digest32 locationId;
    private final Digest32 archiveId;
    private final BigInteger revision;
    private final String stateKind;
    ArchiveLocation(
        final Digest32 locationId,
        final Digest32 archiveId,
        final BigInteger revision,
        final String stateKind,
        final Map<String, Object> raw) {
      super(raw); this.locationId = locationId; this.archiveId = archiveId;
      this.revision = revision; this.stateKind = stateKind;
    }
    public Digest32 locationId() { return locationId; }
    public Digest32 archiveId() { return archiveId; }
    public BigInteger revision() { return revision; }
    public String stateKind() { return stateKind; }
  }

  /** Permanent global alias response. */
  public static final class AliasRecord extends WireValue {
    private final String alias;
    private final PackageId target;
    private final String registeredBy;
    private final BigInteger pricingRevision;
    private final BigInteger paidXor;
    private final BigInteger registeredAtHeight;
    private final BigInteger historyRevision;
    AliasRecord(
        final String alias,
        final PackageId target,
        final String registeredBy,
        final BigInteger pricingRevision,
        final BigInteger paidXor,
        final BigInteger registeredAtHeight,
        final BigInteger historyRevision) {
      requireAsciiKebab(alias, 32, "alias");
      this.alias = alias; this.target = target; this.registeredBy = registeredBy;
      this.pricingRevision = pricingRevision; this.paidXor = paidXor;
      this.registeredAtHeight = registeredAtHeight; this.historyRevision = historyRevision;
    }
    public String alias() { return alias; }
    public PackageId target() { return target; }
    public String registeredBy() { return registeredBy; }
    public BigInteger pricingRevision() { return pricingRevision; }
    public BigInteger paidXor() { return paidXor; }
    public BigInteger registeredAtHeight() { return registeredAtHeight; }
    public BigInteger historyRevision() { return historyRevision; }
    @Override Object toJsonValue() {
      return object(
          "alias", Collections.singletonList(alias), "target", target.toJsonValue(),
          "registered_by", registeredBy, "pricing_revision", pricingRevision,
          "paid_xor", paidXor, "registered_at_height", registeredAtHeight,
          "history_revision", historyRevision);
    }
  }

  /** Immutable alias-history entry. */
  public static final class AliasHistoryEntry extends WireValue {
    private final String alias;
    private final BigInteger revision;
    private final String actionKind;
    private final PackageId previousTarget;
    private final PackageId target;
    private final Digest32 governanceAction;
    private final BigInteger finalizedHeight;
    AliasHistoryEntry(
        final String alias,
        final BigInteger revision,
        final String actionKind,
        final PackageId previousTarget,
        final PackageId target,
        final Digest32 governanceAction,
        final BigInteger finalizedHeight) {
      this.alias = alias; this.revision = revision; this.actionKind = actionKind;
      this.previousTarget = previousTarget; this.target = target;
      this.governanceAction = governanceAction; this.finalizedHeight = finalizedHeight;
    }
    public String alias() { return alias; }
    public BigInteger revision() { return revision; }
    public String actionKind() { return actionKind; }
    public PackageId previousTarget() { return previousTarget; }
    public PackageId target() { return target; }
    public Digest32 governanceAction() { return governanceAction; }
    public BigInteger finalizedHeight() { return finalizedHeight; }
    @Override Object toJsonValue() {
      return object(
          "alias", Collections.singletonList(alias), "revision", revision,
          "action", object("kind", actionKind, "value", null),
          "previous_target", previousTarget == null ? null : previousTarget.toJsonValue(),
          "target", target.toJsonValue(),
          "governance_action", governanceAction == null ? null : governanceAction.toJsonValue(),
          "finalized_height", finalizedHeight);
    }
  }

  /** Ordered public package-directory entry. */
  public static final class OrderedPackageEntry extends WireValue {
    private final PackageSelector selector;
    private final PackageId packageId;
    private final Version latestSelectable;
    private final BigInteger metadataRevision;
    private final BigInteger indexRevision;
    OrderedPackageEntry(
        final PackageSelector selector,
        final PackageId packageId,
        final Version latestSelectable,
        final BigInteger metadataRevision,
        final BigInteger indexRevision) {
      this.selector = selector; this.packageId = packageId; this.latestSelectable = latestSelectable;
      this.metadataRevision = metadataRevision; this.indexRevision = indexRevision;
    }
    public PackageSelector selector() { return selector; }
    public PackageId packageId() { return packageId; }
    public Version latestSelectable() { return latestSelectable; }
    public BigInteger metadataRevision() { return metadataRevision; }
    public BigInteger indexRevision() { return indexRevision; }
    @Override Object toJsonValue() {
      return object(
          "selector", selector.toJsonValue(), "package", packageId.toJsonValue(),
          "latest_selectable", latestSelectable == null ? null : latestSelectable.toJsonValue(),
          "metadata_revision", metadataRevision, "index_revision", indexRevision);
    }
  }

  /** Typed finalized response page. */
  public static final class Page<T extends WireValue> extends WireValue {
    private final List<T> items;
    private final FinalizedCursor nextCursor;
    private final RegistrySnapshot snapshot;
    Page(final List<T> items, final FinalizedCursor nextCursor, final RegistrySnapshot snapshot) {
      this.items = immutableList(items);
      if (this.items.size() > 100) throw new IllegalArgumentException("Musubi page exceeds 100 items");
      if (nextCursor != null && !nextCursor.snapshot().equals(snapshot)) {
        throw new IllegalArgumentException("Musubi page cursor uses another snapshot");
      }
      this.nextCursor = nextCursor; this.snapshot = snapshot;
    }
    public List<T> items() { return items; }
    public FinalizedCursor nextCursor() { return nextCursor; }
    public RegistrySnapshot snapshot() { return snapshot; }
    @Override Object toJsonValue() {
      final List<Object> values = new ArrayList<>();
      for (final T item : items) values.add(item.toJsonValue());
      return object(
          "items", values,
          "next_cursor", nextCursor == null ? null : nextCursor.toJsonValue(),
          "snapshot", snapshot.toJsonValue());
    }
  }

  /** Resolver page carrying the exact chain/genesis identity required by lockfiles. */
  public static final class ResolverIndexPage extends WireValue {
    private final String chainId;
    private final byte[] genesisHash;
    private final List<ResolverReleaseRow> items;
    private final FinalizedCursor nextCursor;
    private final RegistrySnapshot snapshot;

    ResolverIndexPage(
        final String chainId,
        final byte[] genesisHash,
        final List<ResolverReleaseRow> items,
        final FinalizedCursor nextCursor,
        final RegistrySnapshot snapshot) {
      requireExactText(chainId, "Musubi resolver chain ID");
      if (genesisHash == null || genesisHash.length != 32) {
        throw new IllegalArgumentException("Musubi genesis hash must contain 32 bytes");
      }
      this.chainId = chainId;
      this.genesisHash = genesisHash.clone();
      this.items = immutableList(items);
      if (this.items.size() > 100) {
        throw new IllegalArgumentException("Musubi resolver page exceeds 100 items");
      }
      if (nextCursor != null && !nextCursor.snapshot().equals(snapshot)) {
        throw new IllegalArgumentException("Musubi resolver cursor uses another snapshot");
      }
      this.nextCursor = nextCursor;
      this.snapshot = snapshot;
    }

    public String chainId() { return chainId; }
    public byte[] genesisHash() { return genesisHash.clone(); }
    public List<ResolverReleaseRow> items() { return items; }
    public FinalizedCursor nextCursor() { return nextCursor; }
    public RegistrySnapshot snapshot() { return snapshot; }

    @Override Object toJsonValue() {
      final List<Object> values = new ArrayList<>();
      for (final ResolverReleaseRow item : items) values.add(item.toJsonValue());
      final List<Integer> genesis = new ArrayList<>();
      for (final byte value : genesisHash) genesis.add(Integer.valueOf(value & 0xff));
      return object(
          "chain_id", chainId,
          "genesis_hash", genesis,
          "items", values,
          "next_cursor", nextCursor == null ? null : nextCursor.toJsonValue(),
          "snapshot", snapshot.toJsonValue());
    }
  }

  /** Ordered-directory page carrying exact chain/genesis identity for lock creation. */
  public static final class OrderedPrefixPage extends WireValue {
    private final String chainId;
    private final byte[] genesisHash;
    private final List<OrderedPackageEntry> items;
    private final FinalizedCursor nextCursor;
    private final RegistrySnapshot snapshot;

    OrderedPrefixPage(
        final String chainId,
        final byte[] genesisHash,
        final List<OrderedPackageEntry> items,
        final FinalizedCursor nextCursor,
        final RegistrySnapshot snapshot) {
      requireExactText(chainId, "Musubi directory chain ID");
      if (genesisHash == null || genesisHash.length != 32) {
        throw new IllegalArgumentException("Musubi genesis hash must contain 32 bytes");
      }
      this.chainId = chainId;
      this.genesisHash = genesisHash.clone();
      this.items = immutableList(items);
      if (this.items.size() > 100) {
        throw new IllegalArgumentException("Musubi ordered-prefix page exceeds 100 items");
      }
      if (nextCursor != null && !nextCursor.snapshot().equals(snapshot)) {
        throw new IllegalArgumentException("Musubi ordered-prefix cursor uses another snapshot");
      }
      this.nextCursor = nextCursor;
      this.snapshot = snapshot;
    }

    public String chainId() { return chainId; }
    public byte[] genesisHash() { return genesisHash.clone(); }
    public List<OrderedPackageEntry> items() { return items; }
    public FinalizedCursor nextCursor() { return nextCursor; }
    public RegistrySnapshot snapshot() { return snapshot; }

    @Override Object toJsonValue() {
      final List<Object> values = new ArrayList<>();
      for (final OrderedPackageEntry item : items) values.add(item.toJsonValue());
      final List<Integer> genesis = new ArrayList<>();
      for (final byte value : genesisHash) genesis.add(Integer.valueOf(value & 0xff));
      return object(
          "chain_id", chainId,
          "genesis_hash", genesis,
          "items", values,
          "next_cursor", nextCursor == null ? null : nextCursor.toJsonValue(),
          "snapshot", snapshot.toJsonValue());
    }
  }

  static void requireU64(final BigInteger value, final String field) {
    if (value == null || value.signum() < 0 || value.compareTo(U64_MAX) > 0) {
      throw new IllegalArgumentException(field + " must fit u64");
    }
  }

  static BigInteger parseU64(final String value, final String field) {
    if (value == null || !value.matches("[0-9]+")) {
      throw new IllegalArgumentException(field + " must be numeric");
    }
    final BigInteger parsed = new BigInteger(value);
    requireU64(parsed, field);
    return parsed;
  }

  static BigInteger parseCanonicalU64(final String value, final String field) {
    if (value != null && value.length() > 1 && value.charAt(0) == '0') {
      throw new IllegalArgumentException(field + " has a leading zero");
    }
    return parseU64(value, field);
  }

  static void requireExactText(final String value, final String field) {
    if (value == null || value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be exact non-empty text");
    }
    for (int index = 0; index < value.length(); index++) {
      if (Character.isISOControl(value.charAt(index))) {
        throw new IllegalArgumentException(field + " contains a control character");
      }
    }
  }

  static void requireAsciiKebab(final String value, final int maximum, final String field) {
    requireExactText(value, "Musubi " + field);
    if (value.length() > maximum
        || value.startsWith("-") || value.endsWith("-") || value.contains("--")
        || !value.matches("[a-z0-9-]+")) {
      throw new IllegalArgumentException("Musubi " + field + " must be lowercase ASCII kebab text");
    }
  }

  static void requireName(final String value, final String field) {
    requireExactText(value, field);
    if (value.getBytes(StandardCharsets.UTF_8).length > 255
        || value.contains("@") || value.contains("#") || value.contains("$")
        || !Normalizer.normalize(value, Normalizer.Form.NFC).equals(value)) {
      throw new IllegalArgumentException(field + " is not a canonical Iroha Name");
    }
    for (int index = 0; index < value.length(); index++) {
      if (Character.isWhitespace(value.charAt(index))) {
        throw new IllegalArgumentException(field + " contains whitespace");
      }
    }
  }

  static void requireNamespace(final String value) {
    requireExactText(value, "Musubi namespace");
    if (value.getBytes(StandardCharsets.UTF_8).length > 255
        || value.contains("/") || value.contains("@") || value.contains(":")) {
      throw new IllegalArgumentException("Musubi namespace contains a reserved character");
    }
    final String[] segments = value.split("\\.", -1);
    if (segments.length < 1 || segments.length > 2) {
      throw new IllegalArgumentException("Musubi namespace must be dataspace or domain.dataspace");
    }
    for (final String segment : segments) requireName(segment, "Musubi namespace segment");
  }

  @SuppressWarnings("unchecked")
  static Map<String, Object> immutableObject(final Map<String, Object> source) {
    final Map<String, Object> copy = new LinkedHashMap<>();
    for (final Map.Entry<String, Object> entry : source.entrySet()) {
      copy.put(entry.getKey(), immutableValue(entry.getValue()));
    }
    return Collections.unmodifiableMap(copy);
  }

  @SuppressWarnings("unchecked")
  private static Object immutableValue(final Object value) {
    if (value instanceof Map<?, ?>) return immutableObject((Map<String, Object>) value);
    if (value instanceof List<?>) {
      final List<Object> copy = new ArrayList<>();
      for (final Object item : (List<?>) value) copy.add(immutableValue(item));
      return Collections.unmodifiableList(copy);
    }
    return value;
  }

  static <T> List<T> immutableList(final List<T> source) {
    return Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(source, "source")));
  }

  static Map<String, Object> object(final Object... pairs) {
    if ((pairs.length & 1) != 0) throw new IllegalArgumentException("object pairs must be even");
    final Map<String, Object> result = new LinkedHashMap<>();
    for (int index = 0; index < pairs.length; index += 2) {
      result.put((String) pairs[index], pairs[index + 1]);
    }
    return result;
  }
}
