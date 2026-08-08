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
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.Set;
import java.util.TreeSet;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.model.instructions.TransferWirePayloadEncoder;

/** First-release-only Musubi package, SemVer, query, cursor, and page DTOs. */
public final class MusubiModelsV1 {
  static final BigInteger U64_MAX = new BigInteger("18446744073709551615");
  // Conservative finalized-cursor ceiling with headroom for the 8 KiB canonical account cap,
  // lowercase hex, the maintainer-state suffix, and one 32-byte invitation identity.
  static final int MUSUBI_MAX_CURSOR_KEY_BYTES_V1 = 2 * 8_192 + 1 + 8 + 2 * 32;
  // A namespace is at most 255 bytes and the portable package prefix is at most 64 bytes.
  static final int MUSUBI_MAX_ORDERED_PREFIX_BYTES_V1 = 255 + 1 + 64;
  // The CAR plan covers source files plus three mandatory canonical bundle metadata entries.
  static final long MUSUBI_MAX_BUNDLE_PAYLOAD_BYTES_V1 = 96L * 1024L * 1024L;
  static final long MUSUBI_MAX_CAR_BYTES_V1 = 96L * 1024L * 1024L;

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

  /** Permanent lowercase ASCII kebab alias. */
  public static final class AliasName extends WireValue {
    private final String value;

    public AliasName(final String value) {
      requireAsciiKebab(value, 32, "alias");
      this.value = value;
    }

    public String value() { return value; }

    @Override Object toJsonValue() { return Collections.singletonList(value); }
  }

  /** Java carrier for canonical {@code MusubiReasonV1}, bounded to 1,024 UTF-8 bytes. */
  public static final class Reason extends WireValue {
    private static final int MAX_UTF8_BYTES = 1_024;

    private final String value;

    public Reason(final String value) {
      requireBoundedCleanText(value, MAX_UTF8_BYTES, "Musubi reason");
      this.value = value;
    }

    public String value() { return value; }

    @Override Object toJsonValue() { return Collections.singletonList(value); }
  }

  /** Independent capabilities granted to an accepted Musubi package maintainer. */
  public static final class MaintainerPermissions extends WireValue {
    private final boolean publish;
    private final boolean yank;
    private final boolean metadata;
    private final boolean archiveLocations;

    public MaintainerPermissions(
        final boolean publish,
        final boolean yank,
        final boolean metadata,
        final boolean archiveLocations) {
      if (!publish && !yank && !metadata && !archiveLocations) {
        throw new IllegalArgumentException(
            "Musubi maintainer permissions must grant at least one capability");
      }
      this.publish = publish;
      this.yank = yank;
      this.metadata = metadata;
      this.archiveLocations = archiveLocations;
    }

    public boolean publish() { return publish; }
    public boolean yank() { return yank; }
    public boolean metadata() { return metadata; }
    public boolean archiveLocations() { return archiveLocations; }

    @Override Object toJsonValue() {
      return object(
          "publish", Boolean.valueOf(publish),
          "yank", Boolean.valueOf(yank),
          "metadata", Boolean.valueOf(metadata),
          "archive_locations", Boolean.valueOf(archiveLocations));
    }
  }

  /** Owner or explicitly permissioned maintainer role for a Musubi package member. */
  public static final class PackageRole extends WireValue {
    public enum Kind { OWNER, MAINTAINER }

    private final Kind kind;
    private final MaintainerPermissions permissions;

    private PackageRole(final Kind kind, final MaintainerPermissions permissions) {
      this.kind = Objects.requireNonNull(kind, "kind");
      if ((kind == Kind.OWNER && permissions != null)
          || (kind == Kind.MAINTAINER && permissions == null)) {
        throw new IllegalArgumentException("Musubi package role payload does not match its kind");
      }
      this.permissions = permissions;
    }

    public static PackageRole owner() {
      return new PackageRole(Kind.OWNER, null);
    }

    public static PackageRole maintainer(final MaintainerPermissions permissions) {
      return new PackageRole(Kind.MAINTAINER, Objects.requireNonNull(permissions, "permissions"));
    }

    public Kind kind() { return kind; }
    public MaintainerPermissions permissions() { return permissions; }

    @Override Object toJsonValue() {
      return object(
          "kind", kind == Kind.OWNER ? "Owner" : "Maintainer",
          "value", permissions == null ? null : permissions.toJsonValue());
    }
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

  /** Immutable public namespace binding used to authorize first publication. */
  public static final class NamespaceBinding extends WireValue {
    private final Namespace namespace;
    private final BigInteger homeDataspace;
    private final PackageScope scope;
    private final BigInteger generation;

    public NamespaceBinding(
        final Namespace namespace,
        final BigInteger homeDataspace,
        final PackageScope scope,
        final BigInteger generation) {
      Objects.requireNonNull(namespace, "namespace");
      Objects.requireNonNull(scope, "scope");
      requireU64(homeDataspace, "namespaceBinding.homeDataspace");
      requireU64(generation, "namespaceBinding.generation");
      if (generation.signum() == 0) {
        throw new IllegalArgumentException("Musubi namespace generation must be non-zero");
      }
      final int separator = namespace.value().indexOf('.');
      final String domain = separator < 0 ? null : namespace.value().substring(0, separator);
      if ((scope.kind() == PackageScope.Kind.DATASPACE_ROOT && domain != null)
          || (scope.kind() == PackageScope.Kind.DOMAIN && !Objects.equals(scope.domain(), domain))) {
        throw new IllegalArgumentException("Musubi namespace binding text and scope disagree");
      }
      this.namespace = namespace;
      this.homeDataspace = homeDataspace;
      this.scope = scope;
      this.generation = generation;
    }

    public Namespace namespace() { return namespace; }
    public BigInteger homeDataspace() { return homeDataspace; }
    public PackageScope scope() { return scope; }
    public BigInteger generation() { return generation; }

    @Override Object toJsonValue() {
      return object(
          "namespace", namespace.toJsonValue(),
          "home_dataspace", homeDataspace,
          "scope", scope.toJsonValue(),
          "generation", generation);
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
      if (kind == Kind.COMPARATORS
          && this.comparators.size() == 1
          && this.comparators.get(0).op() == ComparatorOp.EQUAL) {
        throw new IllegalArgumentException(
            "Musubi singleton equality comparator must use the exact requirement variant");
      }
      int exactComparators = 0;
      for (final VersionComparator comparator : this.comparators) {
        if (comparator.op() == ComparatorOp.EQUAL) exactComparators++;
      }
      if (exactComparators > 1) {
        throw new IllegalArgumentException(
            "Musubi comparator list contains contradictory exact versions");
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
        for (final String item : value.split(",", -1)) {
          unique.add(parseComparator(trimAsciiSpaces(item)));
        }
        final List<VersionComparator> sorted = new ArrayList<>(unique);
        Collections.sort(sorted);
        if (sorted.size() == 1 && sorted.get(0).op() == ComparatorOp.EQUAL) {
          return fromWire(
              Kind.EXACT, sorted.get(0).version(), null, null, Collections.emptyList());
        }
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

    private static String trimAsciiSpaces(final String value) {
      int start = 0;
      int end = value.length();
      while (start < end && value.charAt(start) == ' ') start++;
      while (end > start && value.charAt(end - 1) == ' ') end--;
      return value.substring(start, end);
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

    /** Return whether an exact version satisfies this requirement under Cargo prerelease rules. */
    public boolean matches(final Version candidate) {
      Objects.requireNonNull(candidate, "candidate");
      if (!candidate.prerelease().isEmpty() && !prereleaseEligible(candidate)) return false;
      switch (kind) {
        case ANY:
          return true;
        case CARET:
          return candidate.compareTo(version) >= 0
              && caretCoreIsCompatible(version, candidate);
        case TILDE:
          return candidate.compareTo(version) >= 0
              && candidate.major().equals(version.major())
              && candidate.minor().equals(version.minor());
        case MAJOR_WILDCARD:
          return candidate.major().equals(major);
        case MINOR_WILDCARD:
          return candidate.major().equals(major) && candidate.minor().equals(minor);
        case EXACT:
          return candidate.equals(version);
        case COMPARATORS:
          for (final VersionComparator comparator : comparators) {
            final int ordering = candidate.compareTo(comparator.version());
            switch (comparator.op()) {
              case GREATER: if (ordering <= 0) return false; break;
              case GREATER_OR_EQUAL: if (ordering < 0) return false; break;
              case LESS: if (ordering >= 0) return false; break;
              case LESS_OR_EQUAL: if (ordering > 0) return false; break;
              case EQUAL: if (ordering != 0) return false; break;
              default: throw new IllegalStateException("unhandled Musubi comparator");
            }
          }
          return true;
        default:
          throw new IllegalStateException("unhandled Musubi requirement");
      }
    }

    private boolean prereleaseEligible(final Version candidate) {
      if (kind == Kind.CARET || kind == Kind.TILDE || kind == Kind.EXACT) {
        return explicitlyNamesPrereleaseCore(version, candidate);
      }
      if (kind == Kind.COMPARATORS) {
        for (final VersionComparator comparator : comparators) {
          if (explicitlyNamesPrereleaseCore(comparator.version(), candidate)) return true;
        }
      }
      return false;
    }

    private static boolean explicitlyNamesPrereleaseCore(
        final Version named, final Version candidate) {
      return named != null
          && !named.prerelease().isEmpty()
          && named.major().equals(candidate.major())
          && named.minor().equals(candidate.minor())
          && named.patch().equals(candidate.patch());
    }

    private static boolean caretCoreIsCompatible(
        final Version base, final Version candidate) {
      if (base.major().signum() > 0) return candidate.major().equals(base.major());
      if (base.minor().signum() > 0) {
        return candidate.major().signum() == 0 && candidate.minor().equals(base.minor());
      }
      return candidate.major().signum() == 0
          && candidate.minor().signum() == 0
          && candidate.patch().equals(base.patch());
    }

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
  public static class Digest32 extends WireValue {
    private final byte[] bytes;

    protected Digest32(final byte[] bytes) {
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

  /** Domain-separated digest of one complete provider bundle attestation. */
  public static final class ProviderBundleAttestationDigest extends Digest32 {
    private ProviderBundleAttestationDigest(final byte[] bytes) {
      super(bytes);
      if (allZero(bytes)) {
        throw new IllegalArgumentException(
            "Musubi provider bundle attestation digest must be non-zero");
      }
    }

    public static ProviderBundleAttestationDigest fromBytes(final byte[] bytes) {
      return new ProviderBundleAttestationDigest(bytes);
    }
  }

  /** Archive/order-bound digest of a provider-sorted attestation set. */
  public static final class ProviderBundleAttestationSetDigest extends Digest32 {
    private ProviderBundleAttestationSetDigest(final byte[] bytes) {
      super(bytes);
      if (allZero(bytes)) {
        throw new IllegalArgumentException(
            "Musubi provider bundle attestation set digest must be non-zero");
      }
    }

    public static ProviderBundleAttestationSetDigest fromBytes(final byte[] bytes) {
      return new ProviderBundleAttestationSetDigest(bytes);
    }
  }

  /** Enacted Parliament decision bound to one exact Musubi governance action. */
  public static final class GovernanceDecision extends WireValue {
    private final Digest32 decisionId;
    private final Digest32 actionDigest;
    private final BigInteger enactedAtHeight;
    private final BigInteger executeAfterHeight;

    public GovernanceDecision(
        final Digest32 decisionId,
        final Digest32 actionDigest,
        final BigInteger enactedAtHeight,
        final BigInteger executeAfterHeight) {
      this.decisionId = Objects.requireNonNull(decisionId, "decisionId");
      this.actionDigest = Objects.requireNonNull(actionDigest, "actionDigest");
      if (allZero(this.decisionId.bytes()) || allZero(this.actionDigest.bytes())) {
        throw new IllegalArgumentException("Musubi governance decision digests must be non-zero");
      }
      requireU64(enactedAtHeight, "governanceDecision.enactedAtHeight");
      requireU64(executeAfterHeight, "governanceDecision.executeAfterHeight");
      if (enactedAtHeight.signum() == 0
          || executeAfterHeight.compareTo(enactedAtHeight) <= 0) {
        throw new IllegalArgumentException(
            "Musubi governance execution height must follow a non-zero enactment height");
      }
      this.enactedAtHeight = enactedAtHeight;
      this.executeAfterHeight = executeAfterHeight;
    }

    public Digest32 decisionId() { return decisionId; }
    public Digest32 actionDigest() { return actionDigest; }
    public BigInteger enactedAtHeight() { return enactedAtHeight; }
    public BigInteger executeAfterHeight() { return executeAfterHeight; }

    @Override Object toJsonValue() {
      return object(
          "decision_id", unsignedBytes(decisionId.bytes()),
          "action_digest", actionDigest.toJsonValue(),
          "enacted_at_height", enactedAtHeight,
          "execute_after_height", executeAfterHeight);
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
      if (finalizedBlockHash == null || finalizedBlockHash.length != 32
          || allZero(finalizedBlockHash)) {
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
      if (allZero(queryHash.bytes())) {
        throw new IllegalArgumentException("Musubi cursor query hash must not be inert");
      }
      requireExactText(lastKey, "Musubi cursor last key");
      if (lastKey.getBytes(StandardCharsets.UTF_8).length
          > MUSUBI_MAX_CURSOR_KEY_BYTES_V1) {
        throw new IllegalArgumentException(
            "Musubi cursor last key exceeds "
                + MUSUBI_MAX_CURSOR_KEY_BYTES_V1
                + " UTF-8 bytes");
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
      if (limit < 0 || limit > 100L) {
        throw new IllegalArgumentException("Musubi page limit exceeds 100");
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

  /** Fresh-selection availability for one finalized archive projection. */
  public enum StorageAvailability {
    SELECTABLE("Selectable"),
    BELOW_QUORUM("BelowQuorum"),
    UNAVAILABLE("Unavailable");

    private final String wireKind;
    StorageAvailability(final String wireKind) { this.wireKind = wireKind; }
    String wireKind() { return wireKind; }
  }

  /** Finalized aggregate storage projection carried by retention decisions. */
  public static final class ArchiveAvailability extends WireValue {
    private final Digest32 archiveId;
    private final StorageAvailability availability;
    private final int healthyReplicas;
    private final int activeLocations;
    private final BigInteger finalizedHeight;
    private final byte[] finalizedBlockHash;
    private final BigInteger indexRevision;

    ArchiveAvailability(
        final Digest32 archiveId,
        final StorageAvailability availability,
        final int healthyReplicas,
        final int activeLocations,
        final BigInteger finalizedHeight,
        final byte[] finalizedBlockHash,
        final BigInteger indexRevision) {
      this.archiveId = Objects.requireNonNull(archiveId, "archiveId");
      this.availability = Objects.requireNonNull(availability, "availability");
      requireU64(finalizedHeight, "archiveAvailability.finalizedHeight");
      requireU64(indexRevision, "archiveAvailability.indexRevision");
      if (allZero(archiveId.bytes())
          || healthyReplicas < 0 || healthyReplicas > 65_535
          || activeLocations < 0 || activeLocations > 4
          || healthyReplicas > activeLocations * 64
          || finalizedHeight.signum() == 0 || indexRevision.signum() == 0
          || finalizedBlockHash == null || finalizedBlockHash.length != 32
          || allZero(finalizedBlockHash)) {
        throw new IllegalArgumentException("Musubi archive availability record is invalid");
      }
      final StorageAvailability expected;
      if (healthyReplicas >= 3) expected = StorageAvailability.SELECTABLE;
      else if (activeLocations > 0 && healthyReplicas > 0) {
        expected = StorageAvailability.BELOW_QUORUM;
      } else expected = StorageAvailability.UNAVAILABLE;
      if (availability != expected) {
        throw new IllegalArgumentException(
            "Musubi archive availability classification is inconsistent with its counts");
      }
      this.healthyReplicas = healthyReplicas;
      this.activeLocations = activeLocations;
      this.finalizedHeight = finalizedHeight;
      this.finalizedBlockHash = finalizedBlockHash.clone();
      this.indexRevision = indexRevision;
    }

    public Digest32 archiveId() { return archiveId; }
    public StorageAvailability availability() { return availability; }
    public int healthyReplicas() { return healthyReplicas; }
    public int activeLocations() { return activeLocations; }
    public BigInteger finalizedHeight() { return finalizedHeight; }
    public byte[] finalizedBlockHash() { return finalizedBlockHash.clone(); }
    public BigInteger indexRevision() { return indexRevision; }

    @Override Object toJsonValue() {
      return object(
          "archive_id", archiveId.toJsonValue(),
          "availability", object("kind", availability.wireKind(), "value", null),
          "healthy_replicas", Integer.valueOf(healthyReplicas),
          "active_locations", Integer.valueOf(activeLocations),
          "finalized_height", finalizedHeight,
          "finalized_block_hash", unsignedBytes(finalizedBlockHash),
          "index_revision", indexRevision);
    }
  }

  /** Authoritative cache-retention classification for one exact archive. */
  public enum ArchiveRetentionDisposition {
    RETAIN_UNKNOWN("RetainUnknown", true),
    RETAIN_REFERENCED("RetainReferenced", true),
    PRUNE_UNREFERENCED("PruneUnreferenced", false),
    PRUNE_GOVERNED_TAKEDOWN("PruneGovernedTakedown", false);

    private final String wireKind;
    private final boolean mustRetain;
    ArchiveRetentionDisposition(final String wireKind, final boolean mustRetain) {
      this.wireKind = wireKind;
      this.mustRetain = mustRetain;
    }
    String wireKind() { return wireKind; }
    public boolean mustRetain() { return mustRetain; }
  }

  /** One exact finalized cache-retention decision. */
  public static final class ArchiveRetentionDecision extends WireValue {
    private final Digest32 archiveId;
    private final ArchiveRetentionDisposition disposition;
    private final int activeReleases;
    private final int yankedReleases;
    private final int takenDownReleases;
    private final ArchiveAvailability storage;

    ArchiveRetentionDecision(
        final Digest32 archiveId,
        final ArchiveRetentionDisposition disposition,
        final int activeReleases,
        final int yankedReleases,
        final int takenDownReleases,
        final ArchiveAvailability storage) {
      this.archiveId = Objects.requireNonNull(archiveId, "archiveId");
      this.disposition = Objects.requireNonNull(disposition, "disposition");
      if (allZero(archiveId.bytes())
          || activeReleases < 0 || activeReleases > 65_535
          || yankedReleases < 0 || yankedReleases > 65_535
          || takenDownReleases < 0 || takenDownReleases > 65_535) {
        throw new IllegalArgumentException("Musubi archive retention decision is invalid");
      }
      final int referenced = activeReleases + yankedReleases + takenDownReleases;
      if (referenced > 1_024 || (storage != null && !storage.archiveId().equals(archiveId))) {
        throw new IllegalArgumentException(
            "Musubi archive retention decision exceeds its bound or changes identity");
      }
      final int available = activeReleases + yankedReleases;
      final boolean canonical;
      switch (disposition) {
        case RETAIN_UNKNOWN:
          canonical = referenced == 0 && storage == null;
          break;
        case RETAIN_REFERENCED:
          canonical = available > 0 && storage != null;
          break;
        case PRUNE_UNREFERENCED:
          canonical = referenced == 0 && storage != null;
          break;
        case PRUNE_GOVERNED_TAKEDOWN:
          canonical = available == 0 && takenDownReleases > 0 && storage != null;
          break;
        default:
          throw new IllegalStateException("unhandled Musubi archive-retention disposition");
      }
      if (!canonical) {
        throw new IllegalArgumentException(
            "Musubi archive retention decision is internally inconsistent");
      }
      this.activeReleases = activeReleases;
      this.yankedReleases = yankedReleases;
      this.takenDownReleases = takenDownReleases;
      this.storage = storage;
    }

    public Digest32 archiveId() { return archiveId; }
    public ArchiveRetentionDisposition disposition() { return disposition; }
    public int activeReleases() { return activeReleases; }
    public int yankedReleases() { return yankedReleases; }
    public int takenDownReleases() { return takenDownReleases; }
    public ArchiveAvailability storage() { return storage; }
    public boolean mustRetain() { return disposition.mustRetain(); }

    @Override Object toJsonValue() {
      return object(
          "archive_id", archiveId.toJsonValue(),
          "disposition", object("kind", disposition.wireKind(), "value", null),
          "active_releases", Integer.valueOf(activeReleases),
          "yanked_releases", Integer.valueOf(yankedReleases),
          "taken_down_releases", Integer.valueOf(takenDownReleases),
          "storage", storage == null ? null : storage.toJsonValue());
    }
  }

  /** Bounded, sorted exact archive identities for authoritative cache retention. */
  public static final class ArchiveRetentionQuery extends WireValue {
    private final List<Digest32> archiveIds;
    private final RegistrySnapshot expectedSnapshot;

    public ArchiveRetentionQuery(
        final List<Digest32> archiveIds, final RegistrySnapshot expectedSnapshot) {
      this.archiveIds = immutableList(archiveIds);
      this.expectedSnapshot = expectedSnapshot;
      if (this.archiveIds.isEmpty() || this.archiveIds.size() > 100) {
        throw new IllegalArgumentException(
            "Musubi archive retention batch is empty or oversized");
      }
      for (int index = 0; index < this.archiveIds.size(); index++) {
        final Digest32 current = Objects.requireNonNull(this.archiveIds.get(index), "archiveId");
        if (allZero(current.bytes())
            || (index > 0 && compareUnsignedBytes(
                this.archiveIds.get(index - 1).bytes(), current.bytes()) >= 0)) {
          throw new IllegalArgumentException(
              "Musubi archive retention batch is noncanonical");
        }
      }
    }

    public List<Digest32> archiveIds() { return archiveIds; }
    public RegistrySnapshot expectedSnapshot() { return expectedSnapshot; }

    @Override Object toJsonValue() {
      final List<Object> values = new ArrayList<>();
      for (final Digest32 archiveId : archiveIds) values.add(archiveId.toJsonValue());
      return object(
          "archive_ids", values,
          "expected_snapshot",
          expectedSnapshot == null ? null : expectedSnapshot.toJsonValue());
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
      if (prefix.getBytes(StandardCharsets.UTF_8).length
          > MUSUBI_MAX_ORDERED_PREFIX_BYTES_V1) {
        throw new IllegalArgumentException(
            "Musubi ordered prefix exceeds "
                + MUSUBI_MAX_ORDERED_PREFIX_BYTES_V1
                + " UTF-8 bytes");
      }
      final int separator = prefix.indexOf('/');
      if (separator < 0 || separator != prefix.lastIndexOf('/')) {
        throw new IllegalArgumentException(
            "Musubi ordered prefix must use exactly one namespace/package-prefix separator");
      }
      new Namespace(prefix.substring(0, separator));
      final String packagePrefix = prefix.substring(separator + 1);
      if (packagePrefix.getBytes(StandardCharsets.UTF_8).length > 64
          || packagePrefix.startsWith("-")
          || packagePrefix.contains("--")
          || !packagePrefix.matches("[a-z0-9-]*")) {
        throw new IllegalArgumentException(
            "Musubi ordered package prefix is not portable canonical text");
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

  /** Finalized anchor for the rebuildable package-search projection. */
  public static final class SearchSnapshot extends WireValue {
    private final BigInteger finalizedHeight;
    private final byte[] finalizedBlockHash;
    private final BigInteger projectionRevision;

    public SearchSnapshot(
        final BigInteger finalizedHeight,
        final byte[] finalizedBlockHash,
        final BigInteger projectionRevision) {
      requireU64(finalizedHeight, "searchSnapshot.finalizedHeight");
      requireU64(projectionRevision, "searchSnapshot.projectionRevision");
      if (finalizedHeight.signum() == 0 || projectionRevision.signum() == 0
          || finalizedBlockHash == null || finalizedBlockHash.length != 32
          || allZero(finalizedBlockHash)) {
        throw new IllegalArgumentException("Musubi search snapshot is invalid");
      }
      this.finalizedHeight = finalizedHeight;
      this.finalizedBlockHash = finalizedBlockHash.clone();
      this.projectionRevision = projectionRevision;
    }

    public BigInteger finalizedHeight() { return finalizedHeight; }
    public byte[] finalizedBlockHash() { return finalizedBlockHash.clone(); }
    public BigInteger projectionRevision() { return projectionRevision; }

    @Override Object toJsonValue() {
      return object(
          "finalized_height", finalizedHeight,
          "finalized_block_hash", unsignedBytes(finalizedBlockHash),
          "projection_revision", projectionRevision);
    }
  }

  /** Search continuation bound to one exact query and projection snapshot. */
  public static final class SearchCursor extends WireValue {
    private final SearchSnapshot snapshot;
    private final Digest32 queryHash;
    private final PackageId lastPackage;

    public SearchCursor(
        final SearchSnapshot snapshot, final Digest32 queryHash, final PackageId lastPackage) {
      this.snapshot = Objects.requireNonNull(snapshot, "snapshot");
      this.queryHash = Objects.requireNonNull(queryHash, "queryHash");
      this.lastPackage = Objects.requireNonNull(lastPackage, "lastPackage");
      if (allZero(queryHash.bytes())) {
        throw new IllegalArgumentException("Musubi search cursor query hash must not be inert");
      }
    }

    public SearchSnapshot snapshot() { return snapshot; }
    public Digest32 queryHash() { return queryHash; }
    public PackageId lastPackage() { return lastPackage; }

    @Override Object toJsonValue() {
      return object(
          "snapshot", snapshot.toJsonValue(),
          "query_hash", queryHash.toJsonValue(),
          "last_package", lastPackage.toJsonValue());
    }
  }

  /** Bounded page controls for rich package discovery. */
  public static final class SearchPageRequest extends WireValue {
    private final long limit;
    private final SearchCursor cursor;

    public SearchPageRequest() { this(50L, null); }

    public SearchPageRequest(final long limit, final SearchCursor cursor) {
      if (limit < 0 || limit > 100) {
        throw new IllegalArgumentException("Musubi search page limit exceeds 100");
      }
      this.limit = limit;
      this.cursor = cursor;
    }

    public long limit() { return limit; }
    public SearchCursor cursor() { return cursor; }

    @Override Object toJsonValue() {
      return object("limit", limit, "cursor", cursor == null ? null : cursor.toJsonValue());
    }
  }

  /** Bounded exact-token description and keyword search query. */
  public static final class SearchQuery extends WireValue {
    private final String query;
    private final SearchPageRequest page;

    public SearchQuery(final String query) { this(query, new SearchPageRequest()); }

    public SearchQuery(final String query, final SearchPageRequest page) {
      requireExactText(query, "Musubi search query");
      if (query.getBytes(StandardCharsets.UTF_8).length > 256) {
        throw new IllegalArgumentException("Musubi search query exceeds 256 UTF-8 bytes");
      }
      normalizedSearchTerms(query);
      this.query = query;
      this.page = Objects.requireNonNull(page, "page");
    }

    public String query() { return query; }
    public SearchPageRequest page() { return page; }

    @Override Object toJsonValue() {
      return object("query", query, "page", page.toJsonValue());
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

    /** Rejects an exact-package response for a different structural package. */
    public void requireMatches(final ExactPackageQuery request) {
      if (!packageId.equals(Objects.requireNonNull(request, "request").packageId())) {
        throw new IllegalArgumentException(
            "Musubi exact-package response does not match the request");
      }
    }

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
    final Map<String, Object> rawValue() { return raw; }
    @Override final Object toJsonValue() { return raw; }
  }

  /** Exact release response. */
  public static final class ReleaseRecord extends StrictRecord {
    private final ReleaseManifest manifest;
    private final ReleaseId release;
    private final Digest32 releaseDigest;
    private final String publishedBy;
    private final BigInteger publishedAtHeight;
    ReleaseRecord(
        final ReleaseManifest manifest,
        final Digest32 releaseDigest,
        final String publishedBy,
        final BigInteger publishedAtHeight,
        final Map<String, Object> raw) {
      super(raw);
      this.manifest = Objects.requireNonNull(manifest, "manifest");
      this.release = manifest.release();
      this.releaseDigest = Objects.requireNonNull(releaseDigest, "releaseDigest");
      if (!Arrays.equals(
          releaseDigest.bytes(), MusubiInstructionsV1.releaseManifestDigest(manifest))) {
        throw new IllegalArgumentException(
            "Musubi release digest does not match its canonical manifest");
      }
      this.publishedBy =
          AccountIdLiteral.requireCanonicalI105Address(publishedBy, "release publisher");
      requireU64(publishedAtHeight, "publishedAtHeight");
      if (publishedAtHeight.signum() == 0) {
        throw new IllegalArgumentException("Musubi publication height must be non-zero");
      }
      this.publishedAtHeight = publishedAtHeight;
    }
    public ReleaseManifest manifest() { return manifest; }
    public ReleaseId release() { return release; }
    public Digest32 releaseDigest() { return releaseDigest; }
    public String publishedBy() { return publishedBy; }
    public BigInteger publishedAtHeight() { return publishedAtHeight; }

    /** Rejects an exact-release response for a different immutable release. */
    public void requireMatches(final ExactReleaseQuery request) {
      if (!release.equals(Objects.requireNonNull(request, "request").release())) {
        throw new IllegalArgumentException(
            "Musubi exact-release response does not match the request");
      }
    }
  }

  /** Compact resolver row response. */
  public static final class ResolverReleaseRow extends StrictRecord {
    private final ReleaseId release;
    private final BigInteger indexRevision;
    private final BigInteger storageIndexRevision;
    ResolverReleaseRow(
        final ReleaseId release,
        final BigInteger indexRevision,
        final BigInteger storageIndexRevision,
        final Map<String, Object> raw) {
      super(raw);
      this.release = release;
      this.indexRevision = indexRevision;
      this.storageIndexRevision = storageIndexRevision;
    }
    public ReleaseId release() { return release; }
    public BigInteger indexRevision() { return indexRevision; }
    public BigInteger storageIndexRevision() { return storageIndexRevision; }
  }

  /** Finalized paired home-dataspace and universal-index view of one exact release. */
  public static final class ExactReleaseSnapshot extends WireValue {
    private final String chainId;
    private final byte[] genesisHash;
    private final RegistrySnapshot snapshot;
    private final ReleaseRecord homeRelease;
    private final ResolverReleaseRow universalRelease;

    ExactReleaseSnapshot(
        final String chainId,
        final byte[] genesisHash,
        final RegistrySnapshot snapshot,
        final ReleaseRecord homeRelease,
        final ResolverReleaseRow universalRelease) {
      requireChainId(chainId, "Musubi exact release chain ID");
      if (genesisHash == null || genesisHash.length != 32 || allZero(genesisHash)) {
        throw new IllegalArgumentException(
            "Musubi exact release genesis hash must be non-zero and 32 bytes");
      }
      this.snapshot = Objects.requireNonNull(snapshot, "snapshot");
      this.homeRelease = Objects.requireNonNull(homeRelease, "homeRelease");
      this.universalRelease = Objects.requireNonNull(universalRelease, "universalRelease");
      if (!homeRelease.release().equals(universalRelease.release())
          || homeRelease.publishedAtHeight().compareTo(snapshot.finalizedHeight()) > 0
          || universalRelease.storageIndexRevision().compareTo(
                  universalRelease.indexRevision()) > 0
          || universalRelease.indexRevision().compareTo(snapshot.indexRevision()) > 0) {
        throw new IllegalArgumentException(
            "Musubi exact release projections are inconsistent with their finalized snapshot");
      }
      MusubiJsonV1.validateExactReleaseSnapshot(
          homeRelease.rawValue(),
          universalRelease.rawValue(),
          genesisHash,
          snapshot);
      this.chainId = chainId;
      this.genesisHash = genesisHash.clone();
    }

    public String chainId() { return chainId; }
    public byte[] genesisHash() { return genesisHash.clone(); }
    public RegistrySnapshot snapshot() { return snapshot; }
    public ReleaseRecord homeRelease() { return homeRelease; }
    public ResolverReleaseRow universalRelease() { return universalRelease; }

    /** Rejects a paired snapshot for a different immutable release. */
    public void requireMatches(final ExactReleaseQuery request) {
      final ReleaseId expected = Objects.requireNonNull(request, "request").release();
      if (!homeRelease.release().equals(expected)
          || !universalRelease.release().equals(expected)) {
        throw new IllegalArgumentException(
            "Musubi exact-release snapshot does not match the request");
      }
    }

    @Override Object toJsonValue() {
      return object(
          "chain_id", chainId,
          "genesis_hash", unsignedBytes(genesisHash),
          "snapshot", snapshot.toJsonValue(),
          "home_release", homeRelease.toJsonValue(),
          "universal_release", universalRelease.toJsonValue());
    }
  }

  /** Accepted package member response. */
  public static final class PackageMember extends StrictRecord {
    private final PackageId packageId;
    private final String account;
    private final String roleKind;
    private final BigInteger acceptedAtHeight;
    private final BigInteger governanceRevision;
    PackageMember(
        final PackageId packageId,
        final String account,
        final String roleKind,
        final BigInteger acceptedAtHeight,
        final BigInteger governanceRevision,
        final Map<String, Object> raw) {
      super(raw);
      this.packageId = packageId;
      this.account = AccountIdLiteral.requireCanonicalI105Address(account, "member account");
      this.roleKind = roleKind;
      requireU64(acceptedAtHeight, "member.acceptedAtHeight");
      requireU64(governanceRevision, "member.governanceRevision");
      if (acceptedAtHeight.signum() == 0 || governanceRevision.signum() == 0) {
        throw new IllegalArgumentException("Musubi package member heights must be non-zero");
      }
      this.acceptedAtHeight = acceptedAtHeight;
      this.governanceRevision = governanceRevision;
    }
    public PackageId packageId() { return packageId; }
    public String account() { return account; }
    public String roleKind() { return roleKind; }
    public BigInteger acceptedAtHeight() { return acceptedAtHeight; }
    public BigInteger governanceRevision() { return governanceRevision; }
  }

  /** Pending package-governance invitation that has not created authority. */
  public static final class MaintainerInvitation extends StrictRecord {
    private final Digest32 inviteId;
    private final PackageId packageId;
    private final String invitedBy;
    private final String invitedAccount;
    private final String roleKind;
    private final BigInteger expectedGovernanceRevision;
    private final BigInteger expiresAtHeight;
    private final String stateKind;

    MaintainerInvitation(
        final Digest32 inviteId,
        final PackageId packageId,
        final String invitedBy,
        final String invitedAccount,
        final String roleKind,
        final BigInteger expectedGovernanceRevision,
        final BigInteger expiresAtHeight,
        final String stateKind,
        final Map<String, Object> raw) {
      super(raw);
      this.inviteId = inviteId;
      this.packageId = packageId;
      this.invitedBy =
          AccountIdLiteral.requireCanonicalI105Address(invitedBy, "invitation inviter");
      this.invitedAccount =
          AccountIdLiteral.requireCanonicalI105Address(invitedAccount, "invited account");
      this.roleKind = roleKind;
      this.expectedGovernanceRevision = expectedGovernanceRevision;
      this.expiresAtHeight = expiresAtHeight;
      this.stateKind = stateKind;
    }

    public Digest32 inviteId() { return inviteId; }
    public PackageId packageId() { return packageId; }
    public String invitedBy() { return invitedBy; }
    public String invitedAccount() { return invitedAccount; }
    public String roleKind() { return roleKind; }
    public BigInteger expectedGovernanceRevision() { return expectedGovernanceRevision; }
    public BigInteger expiresAtHeight() { return expiresAtHeight; }
    public String stateKind() { return stateKind; }
  }

  /** Accepted member or pending invitation returned by the maintainer directory. */
  public static final class MaintainerDirectoryEntry extends WireValue {
    public enum Kind { ACCEPTED, PENDING_INVITATION }

    private final Kind kind;
    private final PackageMember acceptedMember;
    private final MaintainerInvitation pendingInvitation;

    private MaintainerDirectoryEntry(
        final Kind kind,
        final PackageMember acceptedMember,
        final MaintainerInvitation pendingInvitation) {
      this.kind = Objects.requireNonNull(kind, "kind");
      this.acceptedMember = acceptedMember;
      this.pendingInvitation = pendingInvitation;
    }

    static MaintainerDirectoryEntry accepted(final PackageMember member) {
      return new MaintainerDirectoryEntry(
          Kind.ACCEPTED, Objects.requireNonNull(member, "member"), null);
    }

    static MaintainerDirectoryEntry pendingInvitation(final MaintainerInvitation invitation) {
      return new MaintainerDirectoryEntry(
          Kind.PENDING_INVITATION, null, Objects.requireNonNull(invitation, "invitation"));
    }

    public Kind kind() { return kind; }
    public PackageMember acceptedMember() { return acceptedMember; }
    public MaintainerInvitation pendingInvitation() { return pendingInvitation; }

    @Override Object toJsonValue() {
      return object(
          "kind", kind == Kind.ACCEPTED ? "Accepted" : "PendingInvitation",
          "value",
          kind == Kind.ACCEPTED
              ? acceptedMember.toJsonValue()
              : pendingInvitation.toJsonValue());
    }
  }

  /** Renewable archive-location response. */
  public static final class ArchiveLocation extends StrictRecord {
    private final Digest32 locationId;
    private final Digest32 archiveId;
    private final List<String> providers;
    private final ProviderBundleAttestationSetDigest providerAttestationSetDigest;
    private final BigInteger finalizedHeight;
    private final BigInteger revision;
    private final String stateKind;
    ArchiveLocation(
        final Digest32 locationId,
        final Digest32 archiveId,
        final List<String> providers,
        final ProviderBundleAttestationSetDigest providerAttestationSetDigest,
        final BigInteger finalizedHeight,
        final BigInteger revision,
        final String stateKind,
        final Map<String, Object> raw) {
      super(raw); this.locationId = locationId; this.archiveId = archiveId;
      this.providers = immutableList(providers);
      if (this.providers.isEmpty() || this.providers.size() > 64) {
        throw new IllegalArgumentException(
            "Musubi archive location needs between 1 and 64 providers");
      }
      byte[] previous = null;
      for (final String provider : this.providers) {
        if (provider == null || !provider.matches("[0-9A-F]{64}")) {
          throw new IllegalArgumentException(
              "Musubi archive location provider ID must be canonical uppercase hexadecimal");
        }
        final byte[] current = hexBytes(provider);
        if (allZero(current)
            || (previous != null && compareUnsignedBytes(previous, current) >= 0)) {
          throw new IllegalArgumentException(
              "Musubi archive location providers must be nonzero, sorted, and distinct");
        }
        previous = current;
      }
      this.providerAttestationSetDigest =
          Objects.requireNonNull(providerAttestationSetDigest, "providerAttestationSetDigest");
      this.finalizedHeight = finalizedHeight;
      this.revision = revision; this.stateKind = stateKind;
    }
    public Digest32 locationId() { return locationId; }
    public Digest32 archiveId() { return archiveId; }
    public List<String> providers() { return providers; }
    public ProviderBundleAttestationSetDigest providerAttestationSetDigest() {
      return providerAttestationSetDigest;
    }
    public BigInteger finalizedHeight() { return finalizedHeight; }
    public BigInteger revision() { return revision; }
    public String stateKind() { return stateKind; }
  }

  /** Exact SoraFS chunker profile bound into an archive commitment. */
  public static final class ChunkerProfileHandle extends WireValue {
    private final long profileId;
    private final String namespace;
    private final String name;
    private final String semver;
    private final BigInteger multihashCode;

    public ChunkerProfileHandle(
        final long profileId,
        final String namespace,
        final String name,
        final String semver,
        final BigInteger multihashCode) {
      if (profileId < 0L || profileId > 0xffff_ffffL) {
        throw new IllegalArgumentException("Musubi chunker profile id is outside u32");
      }
      requireExactText(namespace, "Musubi chunker namespace");
      requireExactText(name, "Musubi chunker name");
      requireExactText(semver, "Musubi chunker SemVer");
      if ((namespace + "." + name + "@" + semver).getBytes(StandardCharsets.UTF_8).length > 128) {
        throw new IllegalArgumentException("Musubi chunker handle exceeds 128 UTF-8 bytes");
      }
      this.profileId = profileId;
      this.namespace = namespace;
      this.name = name;
      this.semver = semver;
      requireU64(multihashCode, "chunker.multihashCode");
      this.multihashCode = multihashCode;
    }

    public long profileId() { return profileId; }
    public String namespace() { return namespace; }
    public String name() { return name; }
    public String semver() { return semver; }
    public BigInteger multihashCode() { return multihashCode; }

    @Override Object toJsonValue() {
      return object(
          "profile_id", profileId, "namespace", namespace, "name", name,
          "semver", semver, "multihash_code", multihashCode);
    }
  }

  /**
   * Complete immutable source-archive commitment returned by the registry.
   * {@code contentLength} counts the concatenated canonical bundle payload, including mandatory
   * metadata.
   */
  public static final class ArchiveCommitment extends WireValue {
    private final byte[] rootCid;
    private final ChunkerProfileHandle chunker;
    private final Digest32 chunkPlanDigest;
    private final Digest32 porRoot;
    private final BigInteger contentLength;
    private final Digest32 carDigest;
    private final BigInteger carSize;
    private final Digest32 bundleDigest;
    private final Digest32 sourceTreeDigest;
    private final Digest32 descriptorDigest;
    private final long fileCount;
    private final long chunkCount;

    public ArchiveCommitment(
        final byte[] rootCid,
        final ChunkerProfileHandle chunker,
        final Digest32 chunkPlanDigest,
        final Digest32 porRoot,
        final BigInteger contentLength,
        final Digest32 carDigest,
        final BigInteger carSize,
        final Digest32 bundleDigest,
        final Digest32 sourceTreeDigest,
        final Digest32 descriptorDigest,
        final long fileCount,
        final long chunkCount) {
      Objects.requireNonNull(chunker, "chunker");
      Objects.requireNonNull(chunkPlanDigest, "chunkPlanDigest");
      Objects.requireNonNull(porRoot, "porRoot");
      Objects.requireNonNull(carDigest, "carDigest");
      Objects.requireNonNull(bundleDigest, "bundleDigest");
      Objects.requireNonNull(sourceTreeDigest, "sourceTreeDigest");
      Objects.requireNonNull(descriptorDigest, "descriptorDigest");
      requireU64(contentLength, "archive.contentLength");
      requireU64(carSize, "archive.carSize");
      if (rootCid == null || rootCid.length != 36
          || (rootCid[0] & 0xff) != 1 || (rootCid[1] & 0xff) != 113
          || (rootCid[2] & 0xff) != 31 || (rootCid[3] & 0xff) != 32
          || allZero(Arrays.copyOfRange(rootCid, 4, rootCid.length))) {
        throw new IllegalArgumentException(
            "Musubi root CID must use the canonical CIDv1/dag-cbor/BLAKE3-256 shape");
      }
      if (contentLength.signum() <= 0
          || contentLength.compareTo(BigInteger.valueOf(MUSUBI_MAX_BUNDLE_PAYLOAD_BYTES_V1)) > 0
          || carSize.signum() <= 0
          || carSize.compareTo(BigInteger.valueOf(MUSUBI_MAX_CAR_BYTES_V1)) > 0
          || fileCount < 1 || fileCount > 4_096
          || chunkCount < 1 || chunkCount > 16_384
          || allZero(chunkPlanDigest.bytes()) || allZero(porRoot.bytes())
          || allZero(carDigest.bytes()) || allZero(bundleDigest.bytes())
          || allZero(sourceTreeDigest.bytes()) || allZero(descriptorDigest.bytes())) {
        throw new IllegalArgumentException("Musubi archive commitment is out of bounds");
      }
      this.rootCid = rootCid.clone();
      this.chunker = chunker;
      this.chunkPlanDigest = chunkPlanDigest;
      this.porRoot = porRoot;
      this.contentLength = contentLength;
      this.carDigest = carDigest;
      this.carSize = carSize;
      this.bundleDigest = bundleDigest;
      this.sourceTreeDigest = sourceTreeDigest;
      this.descriptorDigest = descriptorDigest;
      this.fileCount = fileCount;
      this.chunkCount = chunkCount;
    }

    public byte[] rootCid() { return rootCid.clone(); }
    public ChunkerProfileHandle chunker() { return chunker; }
    public Digest32 chunkPlanDigest() { return chunkPlanDigest; }
    public Digest32 porRoot() { return porRoot; }
    public BigInteger contentLength() { return contentLength; }
    public Digest32 carDigest() { return carDigest; }
    public BigInteger carSize() { return carSize; }
    public Digest32 bundleDigest() { return bundleDigest; }
    public Digest32 sourceTreeDigest() { return sourceTreeDigest; }
    public Digest32 descriptorDigest() { return descriptorDigest; }
    public long fileCount() { return fileCount; }
    public long chunkCount() { return chunkCount; }

    @Override Object toJsonValue() {
      return object(
          "root_cid", unsignedBytes(rootCid),
          "chunker", chunker.toJsonValue(),
          "chunk_plan_digest", chunkPlanDigest.toJsonValue(),
          "por_root", porRoot.toJsonValue(),
          "content_length", contentLength,
          "car_digest", carDigest.toJsonValue(),
          "car_size", carSize,
          "bundle_digest", bundleDigest.toJsonValue(),
          "source_tree_digest", sourceTreeDigest.toJsonValue(),
          "descriptor_digest", descriptorDigest.toJsonValue(),
          "file_count", fileCount,
          "chunk_count", chunkCount);
    }
  }

  /** Exact deployment and CAR-body binding signed by seed ingress. */
  public static final class SeedIngressReceiptBinding extends WireValue {
    private final String chainId;
    private final byte[] genesisBlockHash;
    private final String publisher;
    private final String ingressBroker;
    private final String seedProvider;
    private final Digest32 semanticReleaseManifestDigest;
    private final Digest32 archiveId;
    private final Digest32 carBodyDigest;
    private final BigInteger carBodyLength;
    private final byte[] nonce;

    public SeedIngressReceiptBinding(
        final String chainId,
        final byte[] genesisBlockHash,
        final String publisher,
        final String ingressBroker,
        final String seedProvider,
        final Digest32 semanticReleaseManifestDigest,
        final Digest32 archiveId,
        final Digest32 carBodyDigest,
        final BigInteger carBodyLength,
        final byte[] nonce) {
      requireChainId(chainId, "Musubi seed-ingress chain ID");
      final String canonicalPublisher =
          AccountIdLiteral.requireCanonicalI105Address(publisher, "publisher");
      final String canonicalIngressBroker =
          AccountIdLiteral.requireCanonicalI105Address(ingressBroker, "ingressBroker");
      Objects.requireNonNull(semanticReleaseManifestDigest, "semanticReleaseManifestDigest");
      Objects.requireNonNull(archiveId, "archiveId");
      Objects.requireNonNull(carBodyDigest, "carBodyDigest");
      requireU64(carBodyLength, "seedIngress.carBodyLength");
      if (genesisBlockHash == null || genesisBlockHash.length != 32
          || nonce == null || nonce.length != 32
          || allZero(genesisBlockHash) || allZero(nonce)
          || seedProvider == null || !seedProvider.matches("[0-9A-F]{64}")
          || allZero(hexBytes(seedProvider))
          || carBodyLength.signum() <= 0
          || carBodyLength.compareTo(BigInteger.valueOf(96L << 20)) > 0
          || allZero(semanticReleaseManifestDigest.bytes())
          || allZero(archiveId.bytes()) || allZero(carBodyDigest.bytes())) {
        throw new IllegalArgumentException("Musubi seed-ingress binding is invalid");
      }
      this.chainId = chainId;
      this.genesisBlockHash = genesisBlockHash.clone();
      this.publisher = canonicalPublisher;
      this.ingressBroker = canonicalIngressBroker;
      this.seedProvider = seedProvider;
      this.semanticReleaseManifestDigest = semanticReleaseManifestDigest;
      this.archiveId = archiveId;
      this.carBodyDigest = carBodyDigest;
      this.carBodyLength = carBodyLength;
      this.nonce = nonce.clone();
    }

    public String chainId() { return chainId; }
    public byte[] genesisBlockHash() { return genesisBlockHash.clone(); }
    public String publisher() { return publisher; }
    public String ingressBroker() { return ingressBroker; }
    public String seedProvider() { return seedProvider; }
    public Digest32 semanticReleaseManifestDigest() { return semanticReleaseManifestDigest; }
    public Digest32 archiveId() { return archiveId; }
    public Digest32 carBodyDigest() { return carBodyDigest; }
    public BigInteger carBodyLength() { return carBodyLength; }
    public byte[] nonce() { return nonce.clone(); }

    @Override Object toJsonValue() {
      return object(
          "chain_id", chainId,
          "genesis_block_hash", unsignedBytes(genesisBlockHash),
          "publisher", publisher,
          "ingress_broker", ingressBroker,
          "seed_provider", Collections.singletonList(seedProvider),
          "semantic_release_manifest_digest", semanticReleaseManifestDigest.toJsonValue(),
          "archive_id", archiveId.toJsonValue(),
          "car_body_digest", carBodyDigest.toJsonValue(),
          "car_body_length", carBodyLength,
          "nonce", unsignedBytes(nonce));
    }
  }

  /** One controller approval over a seed-ingress receipt. */
  public static final class SeedIngressReceiptApproval extends WireValue {
    private final String publicKey;
    private final String signature;
    public SeedIngressReceiptApproval(final String publicKey, final String signature) {
      this.publicKey = requireCanonicalPublicKey(publicKey, "Musubi receipt public key");
      this.signature = requireCanonicalSignature(signature, "Musubi receipt signature");
    }
    public String publicKey() { return publicKey; }
    public String signature() { return signature; }
    @Override Object toJsonValue() {
      return object("public_key", publicKey, "signature", signature);
    }
  }

  /** Version-one signed seed-ingress receipt payload. */
  public static final class SeedIngressReceiptPayload extends WireValue {
    private final SeedIngressReceiptBinding binding;
    private final BigInteger issuedAtMs;
    private final BigInteger expiresAtMs;
    public SeedIngressReceiptPayload(
        final SeedIngressReceiptBinding binding,
        final BigInteger issuedAtMs,
        final BigInteger expiresAtMs) {
      requireU64(issuedAtMs, "issuedAtMs");
      requireU64(expiresAtMs, "expiresAtMs");
      if (issuedAtMs.signum() <= 0 || expiresAtMs.compareTo(issuedAtMs) <= 0
          || expiresAtMs.subtract(issuedAtMs).compareTo(BigInteger.valueOf(86_400_000L)) > 0) {
        throw new IllegalArgumentException("Musubi seed-ingress receipt lifetime is invalid");
      }
      this.binding = Objects.requireNonNull(binding, "binding");
      this.issuedAtMs = issuedAtMs;
      this.expiresAtMs = expiresAtMs;
    }
    public SeedIngressReceiptBinding binding() { return binding; }
    public BigInteger issuedAtMs() { return issuedAtMs; }
    public BigInteger expiresAtMs() { return expiresAtMs; }
    @Override Object toJsonValue() {
      return object(
          "version", 1, "binding", binding.toJsonValue(),
          "issued_at_ms", issuedAtMs, "expires_at_ms", expiresAtMs);
    }
  }

  /** Authenticated seed-ingress receipt retained by archive registration. */
  public static final class SeedIngressReceipt extends WireValue {
    private final SeedIngressReceiptPayload payload;
    private final List<SeedIngressReceiptApproval> approvals;
    public SeedIngressReceipt(
        final SeedIngressReceiptPayload payload,
        final List<SeedIngressReceiptApproval> approvals) {
      this.payload = Objects.requireNonNull(payload, "payload");
      this.approvals = immutableList(approvals);
      if (this.approvals.isEmpty() || this.approvals.size() > 64) {
        throw new IllegalArgumentException("Musubi seed-ingress receipt needs bounded approvals");
      }
      requireStrictlyOrderedApprovals(this.approvals);
    }
    public SeedIngressReceiptPayload payload() { return payload; }
    public List<SeedIngressReceiptApproval> approvals() { return approvals; }
    @Override Object toJsonValue() {
      final List<Object> values = new ArrayList<>();
      for (final SeedIngressReceiptApproval approval : approvals) {
        values.add(approval.toJsonValue());
      }
      return object("payload", payload.toJsonValue(), "approvals", values);
    }
  }

  /** Governed signer-policy identity bound into a provider ingest completion. */
  public static final class ProviderCompletionSignerPolicy extends WireValue {
    private final byte[] policyId;
    private final BigInteger revision;
    private final byte[] predecessorDigest;
    private final byte[] policyDigest;

    public ProviderCompletionSignerPolicy(
        final byte[] policyId,
        final BigInteger revision,
        final byte[] predecessorDigest,
        final byte[] policyDigest) {
      requireU64(revision, "providerSignerPolicy.revision");
      if (policyId == null || policyId.length != 32 || allZero(policyId)
          || policyDigest == null || policyDigest.length != 32 || allZero(policyDigest)
          || (revision.equals(BigInteger.ONE) && predecessorDigest != null)
          || (revision.compareTo(BigInteger.ONE) > 0
              && (predecessorDigest == null
                  || predecessorDigest.length != 32
                  || allZero(predecessorDigest)))
          || revision.signum() == 0) {
        throw new IllegalArgumentException("Musubi provider signer policy is invalid");
      }
      this.policyId = policyId.clone();
      this.revision = revision;
      this.predecessorDigest = predecessorDigest == null ? null : predecessorDigest.clone();
      this.policyDigest = policyDigest.clone();
    }

    public byte[] policyId() { return policyId.clone(); }
    public BigInteger revision() { return revision; }
    public byte[] predecessorDigest() {
      return predecessorDigest == null ? null : predecessorDigest.clone();
    }
    public byte[] policyDigest() { return policyDigest.clone(); }

    @Override Object toJsonValue() {
      return object(
          "policy_id", unsignedBytes(policyId),
          "revision", revision,
          "predecessor_digest",
          predecessorDigest == null ? null : unsignedBytes(predecessorDigest),
          "policy_digest", unsignedBytes(policyDigest));
    }
  }

  /** Exact chain-authoritative owner and signer policy for provider completion. */
  public static final class ProviderCompletionAuthority extends WireValue {
    private final String providerOwner;
    private final ProviderCompletionSignerPolicy signerPolicy;

    public ProviderCompletionAuthority(
        final String providerOwner, final ProviderCompletionSignerPolicy signerPolicy) {
      this.providerOwner =
          AccountIdLiteral.requireCanonicalI105Address(providerOwner, "providerOwner");
      this.signerPolicy = Objects.requireNonNull(signerPolicy, "signerPolicy");
    }

    public String providerOwner() { return providerOwner; }
    public ProviderCompletionSignerPolicy signerPolicy() { return signerPolicy; }

    @Override Object toJsonValue() {
      return object(
          "provider_owner", providerOwner,
          "signer_policy", signerPolicy.toJsonValue());
    }
  }

  /** Finalized chain anchor accepted for one provider completion. */
  public static final class ProviderFinalizedAnchor extends WireValue {
    private final BigInteger height;
    private final byte[] blockHash;

    public ProviderFinalizedAnchor(final BigInteger height, final byte[] blockHash) {
      requireU64(height, "providerFinalizedAnchor.height");
      if (height.signum() == 0 || blockHash == null || blockHash.length != 32
          || allZero(blockHash)) {
        throw new IllegalArgumentException("Musubi provider finalized anchor is invalid");
      }
      this.height = height;
      this.blockHash = blockHash.clone();
    }

    public BigInteger height() { return height; }
    public byte[] blockHash() { return blockHash.clone(); }

    @Override Object toJsonValue() {
      return object("height", height, "block_hash", unsignedBytes(blockHash));
    }
  }

  /** Exact provider, completion, deployment, and parsed-bundle commitment binding. */
  public static final class ProviderBundleVerificationBinding extends WireValue {
    private final String chainId;
    private final byte[] genesisBlockHash;
    private final String providerId;
    private final String completedBy;
    private final ProviderCompletionAuthority completionAuthority;
    private final Digest32 replicationOrder;
    private final BigInteger assignmentRevision;
    private final BigInteger completionEpoch;
    private final ProviderFinalizedAnchor finalizedAnchor;
    private final Digest32 archiveId;
    private final Digest32 bundleDigest;
    private final Digest32 descriptorDigest;
    private final Digest32 semanticReleaseManifestDigest;
    private final Digest32 verificationLockDigest;
    private final Digest32 sourceTreeDigest;

    public ProviderBundleVerificationBinding(
        final String chainId,
        final byte[] genesisBlockHash,
        final String providerId,
        final String completedBy,
        final ProviderCompletionAuthority completionAuthority,
        final Digest32 replicationOrder,
        final BigInteger assignmentRevision,
        final BigInteger completionEpoch,
        final ProviderFinalizedAnchor finalizedAnchor,
        final Digest32 archiveId,
        final Digest32 bundleDigest,
        final Digest32 descriptorDigest,
        final Digest32 semanticReleaseManifestDigest,
        final Digest32 verificationLockDigest,
        final Digest32 sourceTreeDigest) {
      requireChainId(chainId, "Musubi provider bundle chain ID");
      final String canonicalCompletedBy =
          AccountIdLiteral.requireCanonicalI105Address(completedBy, "completedBy");
      Objects.requireNonNull(completionAuthority, "completionAuthority");
      requireU64(assignmentRevision, "providerBundle.assignmentRevision");
      requireU64(completionEpoch, "providerBundle.completionEpoch");
      if (genesisBlockHash == null || genesisBlockHash.length != 32
          || allZero(genesisBlockHash)
          || providerId == null || !providerId.matches("[0-9A-F]{64}")
          || allZero(hexBytes(providerId))
          || !Arrays.equals(
              TransferWirePayloadEncoder.encodeAccountIdPayload(canonicalCompletedBy),
              TransferWirePayloadEncoder.encodeAccountIdPayload(
                  completionAuthority.providerOwner()))
          || assignmentRevision.signum() == 0 || completionEpoch.signum() == 0) {
        throw new IllegalArgumentException("Musubi provider bundle binding is invalid");
      }
      this.replicationOrder = requireNonZeroModelDigest(replicationOrder, "replicationOrder");
      this.finalizedAnchor = Objects.requireNonNull(finalizedAnchor, "finalizedAnchor");
      this.archiveId = requireNonZeroModelDigest(archiveId, "archiveId");
      this.bundleDigest = requireNonZeroModelDigest(bundleDigest, "bundleDigest");
      this.descriptorDigest = requireNonZeroModelDigest(descriptorDigest, "descriptorDigest");
      this.semanticReleaseManifestDigest = requireNonZeroModelDigest(
          semanticReleaseManifestDigest, "semanticReleaseManifestDigest");
      this.verificationLockDigest = requireNonZeroModelDigest(
          verificationLockDigest, "verificationLockDigest");
      this.sourceTreeDigest = requireNonZeroModelDigest(sourceTreeDigest, "sourceTreeDigest");
      this.chainId = chainId;
      this.genesisBlockHash = genesisBlockHash.clone();
      this.providerId = providerId;
      this.completedBy = canonicalCompletedBy;
      this.completionAuthority = completionAuthority;
      this.assignmentRevision = assignmentRevision;
      this.completionEpoch = completionEpoch;
    }

    public String chainId() { return chainId; }
    public byte[] genesisBlockHash() { return genesisBlockHash.clone(); }
    public String providerId() { return providerId; }
    public String completedBy() { return completedBy; }
    public ProviderCompletionAuthority completionAuthority() { return completionAuthority; }
    public Digest32 replicationOrder() { return replicationOrder; }
    public BigInteger assignmentRevision() { return assignmentRevision; }
    public BigInteger completionEpoch() { return completionEpoch; }
    public ProviderFinalizedAnchor finalizedAnchor() { return finalizedAnchor; }
    public Digest32 archiveId() { return archiveId; }
    public Digest32 bundleDigest() { return bundleDigest; }
    public Digest32 descriptorDigest() { return descriptorDigest; }
    public Digest32 semanticReleaseManifestDigest() { return semanticReleaseManifestDigest; }
    public Digest32 verificationLockDigest() { return verificationLockDigest; }
    public Digest32 sourceTreeDigest() { return sourceTreeDigest; }

    @Override Object toJsonValue() {
      return object(
          "chain_id", chainId,
          "genesis_block_hash", unsignedBytes(genesisBlockHash),
          "provider_id", Collections.singletonList(providerId),
          "completed_by", completedBy,
          "completion_authority", completionAuthority.toJsonValue(),
          "replication_order", replicationOrder.toJsonValue(),
          "assignment_revision", assignmentRevision,
          "completion_epoch", completionEpoch,
          "finalized_anchor", finalizedAnchor.toJsonValue(),
          "archive_id", archiveId.toJsonValue(),
          "bundle_digest", bundleDigest.toJsonValue(),
          "descriptor_digest", descriptorDigest.toJsonValue(),
          "semantic_release_manifest_digest", semanticReleaseManifestDigest.toJsonValue(),
          "verification_lock_digest", verificationLockDigest.toJsonValue(),
          "source_tree_digest", sourceTreeDigest.toJsonValue());
    }
  }

  /** Version-one provider bundle verification statement. */
  public static final class ProviderBundleVerificationPayload extends WireValue {
    private final ProviderBundleVerificationBinding binding;

    public ProviderBundleVerificationPayload(final ProviderBundleVerificationBinding binding) {
      this.binding = Objects.requireNonNull(binding, "binding");
    }

    public ProviderBundleVerificationBinding binding() { return binding; }

    @Override Object toJsonValue() {
      return object("version", Integer.valueOf(1), "binding", binding.toJsonValue());
    }
  }

  /** One provider-owner controller approval of a bundle verification statement. */
  public static final class ProviderBundleVerificationApproval extends WireValue {
    private final String publicKey;
    private final String signature;

    public ProviderBundleVerificationApproval(final String publicKey, final String signature) {
      this.publicKey = requireCanonicalPublicKey(publicKey, "Musubi provider approval key");
      this.signature = requireCanonicalSignature(signature, "Musubi provider approval signature");
    }

    public String publicKey() { return publicKey; }
    public String signature() { return signature; }

    @Override Object toJsonValue() {
      return object("public_key", publicKey, "signature", signature);
    }
  }

  /** Signed provider proof that the canonical bundle was parsed before completion. */
  public static final class ProviderBundleVerificationAttestation extends WireValue {
    private final ProviderBundleVerificationPayload payload;
    private final List<ProviderBundleVerificationApproval> approvals;

    public ProviderBundleVerificationAttestation(
        final ProviderBundleVerificationPayload payload,
        final List<ProviderBundleVerificationApproval> approvals) {
      this.payload = Objects.requireNonNull(payload, "payload");
      this.approvals = immutableList(approvals);
      if (this.approvals.isEmpty() || this.approvals.size() > 64) {
        throw new IllegalArgumentException("Musubi provider approvals are out of bounds");
      }
      for (int index = 1; index < this.approvals.size(); index++) {
        if (comparePublicKeyLiterals(
                this.approvals.get(index - 1).publicKey(),
                this.approvals.get(index).publicKey()) >= 0) {
          throw new IllegalArgumentException(
              "Musubi provider approvals must be sorted by distinct public keys");
        }
      }
    }

    public ProviderBundleVerificationPayload payload() { return payload; }
    public List<ProviderBundleVerificationApproval> approvals() { return approvals; }

    @Override Object toJsonValue() {
      final List<Object> values = new ArrayList<>();
      for (final ProviderBundleVerificationApproval approval : approvals) {
        values.add(approval.toJsonValue());
      }
      return object("payload", payload.toJsonValue(), "approvals", values);
    }
  }

  /** Immutable archive/order/provider identity of one registered provider proof. */
  public static final class ProviderBundleAttestationKey extends WireValue {
    private final Digest32 archiveId;
    private final Digest32 replicationOrder;
    private final String providerId;

    public ProviderBundleAttestationKey(
        final Digest32 archiveId,
        final Digest32 replicationOrder,
        final String providerId) {
      this.archiveId = requireNonZeroModelDigest(archiveId, "archiveId");
      this.replicationOrder =
          requireNonZeroModelDigest(replicationOrder, "replicationOrder");
      if (providerId == null
          || !providerId.matches("[0-9A-F]{64}")
          || allZero(hexBytes(providerId))) {
        throw new IllegalArgumentException(
            "Musubi provider attestation key provider ID must be canonical and non-zero");
      }
      this.providerId = providerId;
    }

    public Digest32 archiveId() { return archiveId; }
    public Digest32 replicationOrder() { return replicationOrder; }
    public String providerId() { return providerId; }

    @Override Object toJsonValue() {
      return object(
          "archive_id", archiveId.toJsonValue(),
          "replication_order", replicationOrder.toJsonValue(),
          "provider_id", Collections.singletonList(providerId));
    }
  }

  /** Complete immutable provider proof returned by the exact audit query. */
  public static final class ProviderBundleAttestationRecord extends WireValue {
    private final ProviderBundleAttestationKey key;
    private final ProviderBundleAttestationDigest attestationDigest;
    private final ProviderBundleVerificationAttestation attestation;
    private final String registeredBy;
    private final BigInteger registeredAtHeight;

    ProviderBundleAttestationRecord(
        final ProviderBundleAttestationKey key,
        final ProviderBundleAttestationDigest attestationDigest,
        final ProviderBundleVerificationAttestation attestation,
        final String registeredBy,
        final BigInteger registeredAtHeight) {
      this.key = Objects.requireNonNull(key, "key");
      this.attestationDigest = Objects.requireNonNull(attestationDigest, "attestationDigest");
      this.attestation = Objects.requireNonNull(attestation, "attestation");
      final ProviderBundleVerificationBinding binding = attestation.payload().binding();
      if (!key.archiveId().equals(binding.archiveId())
          || !key.replicationOrder().equals(binding.replicationOrder())
          || !key.providerId().equals(binding.providerId())) {
        throw new IllegalArgumentException(
            "Musubi provider attestation record key disagrees with its signed binding");
      }
      if (!Arrays.equals(
          attestationDigest.bytes(),
          MusubiInstructionsV1.providerBundleAttestationDigest(attestation))) {
        throw new IllegalArgumentException(
            "Musubi provider attestation digest disagrees with its canonical attestation bytes");
      }
      this.registeredBy =
          AccountIdLiteral.requireCanonicalI105Address(registeredBy, "registeredBy");
      requireU64(registeredAtHeight, "providerAttestationRecord.registeredAtHeight");
      if (registeredAtHeight.signum() == 0) {
        throw new IllegalArgumentException(
            "Musubi provider attestation registration height must be non-zero");
      }
      this.registeredAtHeight = registeredAtHeight;
    }

    public ProviderBundleAttestationKey key() { return key; }
    public ProviderBundleAttestationDigest attestationDigest() { return attestationDigest; }
    public ProviderBundleVerificationAttestation attestation() { return attestation; }
    public String registeredBy() { return registeredBy; }
    public BigInteger registeredAtHeight() { return registeredAtHeight; }

    /** Rejects an audit response for a different archive/order/provider identity. */
    public void requireMatches(final ProviderBundleAttestationKey request) {
      if (!key.equals(Objects.requireNonNull(request, "request"))) {
        throw new IllegalArgumentException(
            "Musubi provider attestation response does not match the request");
      }
    }

    @Override Object toJsonValue() {
      return object(
          "key", key.toJsonValue(),
          "attestation_digest", attestationDigest.toJsonValue(),
          "attestation", attestation.toJsonValue(),
          "registered_by", registeredBy,
          "registered_at_height", registeredAtHeight);
    }
  }

  /** Immutable release metadata and complete replacement package metadata. */
  public static final class ReleaseMetadata extends WireValue {
    private final String description;
    private final String readme;
    private final String license;
    private final String repository;
    private final List<String> keywords;

    public ReleaseMetadata(
        final String description,
        final String readme,
        final String license,
        final String repository,
        final List<String> keywords) {
      if (description != null) {
        requireBoundedCleanText(description, 4_096, "Musubi description");
      }
      for (final String value : Arrays.asList(readme, license, repository)) {
        if (value != null) requireBoundedCleanText(value, 2_048, "Musubi document reference");
      }
      this.keywords = immutableList(keywords);
      if (this.keywords.size() > 32) {
        throw new IllegalArgumentException("Musubi metadata has too many keywords");
      }
      for (int index = 0; index < this.keywords.size(); index++) {
        requireAsciiKebab(this.keywords.get(index), 64, "keyword");
        if (index > 0 && this.keywords.get(index - 1).compareTo(this.keywords.get(index)) >= 0) {
          throw new IllegalArgumentException("Musubi keywords must be sorted and distinct");
        }
      }
      this.description = description;
      this.readme = readme;
      this.license = license;
      this.repository = repository;
    }

    public String description() { return description; }
    public String readme() { return readme; }
    public String license() { return license; }
    public String repository() { return repository; }
    public List<String> keywords() { return keywords; }

    @Override Object toJsonValue() {
      final List<Object> keywordValues = new ArrayList<>();
      for (final String keyword : keywords) {
        keywordValues.add(Collections.singletonList(keyword));
      }
      return object(
          "description", description == null ? null : Collections.singletonList(description),
          "readme", readme == null ? null : Collections.singletonList(readme),
          "license", license == null ? null : Collections.singletonList(license),
          "repository", repository == null ? null : Collections.singletonList(repository),
          "keywords", keywordValues);
    }
  }

  /** Exact first-release IVM ABI binding. */
  public static final class AbiBinding extends WireValue {
    private final byte[] abiHash;

    public AbiBinding(final byte[] abiHash) {
      if (abiHash == null || abiHash.length != 32 || allZero(abiHash)) {
        throw new IllegalArgumentException("Musubi ABI hash must contain 32 non-zero bytes");
      }
      this.abiHash = abiHash.clone();
    }

    public int abiVersion() { return 1; }
    public byte[] abiHash() { return abiHash.clone(); }

    @Override Object toJsonValue() {
      return object("abi_version", Integer.valueOf(1), "abi_hash", unsignedBytes(abiHash));
    }
  }

  /** One normal dependency range in a published release manifest. */
  public static final class DependencyRequirement extends WireValue
      implements Comparable<DependencyRequirement> {
    private final String alias;
    private final PackageId packageId;
    private final VersionReq requirement;

    public DependencyRequirement(
        final String alias, final PackageId packageId, final VersionReq requirement) {
      requireName(alias, "Musubi dependency alias");
      this.alias = alias;
      this.packageId = Objects.requireNonNull(packageId, "packageId");
      this.requirement = Objects.requireNonNull(requirement, "requirement");
    }

    public String alias() { return alias; }
    public PackageId packageId() { return packageId; }
    public VersionReq requirement() { return requirement; }

    @Override public int compareTo(final DependencyRequirement other) {
      int comparison = compareUtf8Strings(alias, other.alias);
      if (comparison != 0) return comparison;
      comparison = comparePackageIds(packageId, other.packageId);
      return comparison != 0
          ? comparison
          : compareVersionRequirements(requirement, other.requirement);
    }

    @Override Object toJsonValue() {
      return object(
          "alias", alias,
          "package", packageId.toJsonValue(),
          "requirement", requirement.toJsonValue());
    }
  }

  /** Normal or root-local development exact dependency edge. */
  public enum DependencyKind { NORMAL, DEVELOPMENT }

  /** Parent-local exact edge in a publication verification lock. */
  public static final class ExactDependencyEdge extends WireValue
      implements Comparable<ExactDependencyEdge> {
    private final String alias;
    private final DependencyKind kind;
    private final PackageId packageId;
    private final VersionReq requirement;
    private final ReleaseId selected;

    public ExactDependencyEdge(
        final String alias,
        final DependencyKind kind,
        final PackageId packageId,
        final VersionReq requirement,
        final ReleaseId selected) {
      requireName(alias, "Musubi exact dependency alias");
      this.alias = alias;
      this.kind = Objects.requireNonNull(kind, "kind");
      this.packageId = Objects.requireNonNull(packageId, "packageId");
      this.requirement = Objects.requireNonNull(requirement, "requirement");
      this.selected = Objects.requireNonNull(selected, "selected");
      if (!packageId.equals(selected.packageId()) || !requirement.matches(selected.version())) {
        throw new IllegalArgumentException(
            "Musubi exact dependency does not satisfy its package requirement");
      }
    }

    public String alias() { return alias; }
    public DependencyKind kind() { return kind; }
    public PackageId packageId() { return packageId; }
    public VersionReq requirement() { return requirement; }
    public ReleaseId selected() { return selected; }

    @Override public int compareTo(final ExactDependencyEdge other) {
      int comparison = compareUtf8Strings(alias, other.alias);
      if (comparison != 0) return comparison;
      comparison = kind.compareTo(other.kind);
      if (comparison != 0) return comparison;
      comparison = comparePackageIds(packageId, other.packageId);
      if (comparison != 0) return comparison;
      comparison = compareVersionRequirements(requirement, other.requirement);
      return comparison != 0 ? comparison : compareReleaseIds(selected, other.selected);
    }

    @Override Object toJsonValue() {
      return object(
          "alias", alias,
          "kind", object(
              "kind", kind == DependencyKind.NORMAL ? "Normal" : "Development",
              "value", null),
          "package", packageId.toJsonValue(),
          "requirement", requirement.toJsonValue(),
          "selected", selected.toJsonValue());
    }
  }

  /** One exact immutable dependency node in a publication verification graph. */
  public static final class VerificationNode extends WireValue {
    private final ReleaseId release;
    private final Digest32 releaseDigest;
    private final Digest32 archiveId;
    private final Digest32 sourceDigest;
    private final Digest32 interfaceDigest;
    private final AbiBinding abi;
    private final List<ExactDependencyEdge> dependencies;

    public VerificationNode(
        final ReleaseId release,
        final Digest32 releaseDigest,
        final Digest32 archiveId,
        final Digest32 sourceDigest,
        final Digest32 interfaceDigest,
        final AbiBinding abi,
        final List<ExactDependencyEdge> dependencies) {
      this.release = Objects.requireNonNull(release, "release");
      this.releaseDigest = requireNonZeroModelDigest(releaseDigest, "releaseDigest");
      this.archiveId = requireNonZeroModelDigest(archiveId, "archiveId");
      this.sourceDigest = requireNonZeroModelDigest(sourceDigest, "sourceDigest");
      this.interfaceDigest = requireNonZeroModelDigest(interfaceDigest, "interfaceDigest");
      this.abi = Objects.requireNonNull(abi, "abi");
      this.dependencies = immutableList(dependencies);
      if (this.dependencies.size() > 256) {
        throw new IllegalArgumentException("Musubi verification node has too many dependencies");
      }
      requireStrictOrder(this.dependencies, "Musubi verification-node dependencies");
      requireUniqueExactDependencyAliases(
          this.dependencies, "Musubi verification-node dependencies");
    }

    public ReleaseId release() { return release; }
    public Digest32 releaseDigest() { return releaseDigest; }
    public Digest32 archiveId() { return archiveId; }
    public Digest32 sourceDigest() { return sourceDigest; }
    public Digest32 interfaceDigest() { return interfaceDigest; }
    public AbiBinding abi() { return abi; }
    public List<ExactDependencyEdge> dependencies() { return dependencies; }

    @Override Object toJsonValue() {
      final List<Object> edges = new ArrayList<>();
      for (final ExactDependencyEdge edge : dependencies) edges.add(edge.toJsonValue());
      return object(
          "release", release.toJsonValue(),
          "release_digest", releaseDigest.toJsonValue(),
          "archive_id", archiveId.toJsonValue(),
          "source_digest", sourceDigest.toJsonValue(),
          "interface_digest", interfaceDigest.toJsonValue(),
          "abi", abi.toJsonValue(),
          "dependencies", edges);
    }
  }

  /** Normalized exact verification lock packaged into a release bundle. */
  public static final class VerificationLock extends WireValue {
    private final ReleaseId root;
    private final List<ExactDependencyEdge> rootDependencies;
    private final List<VerificationNode> nodes;

    public VerificationLock(
        final ReleaseId root,
        final List<ExactDependencyEdge> rootDependencies,
        final List<VerificationNode> nodes) {
      this.root = Objects.requireNonNull(root, "root");
      this.rootDependencies = immutableList(rootDependencies);
      this.nodes = immutableList(nodes);
      if (this.rootDependencies.size() > 256 || this.nodes.size() > 1_024) {
        throw new IllegalArgumentException("Musubi verification lock exceeds graph bounds");
      }
      requireStrictOrder(this.rootDependencies, "Musubi root dependencies");
      requireUniqueExactDependencyAliases(this.rootDependencies, "Musubi root dependencies");
      for (int index = 1; index < this.nodes.size(); index++) {
        if (compareReleaseIds(
                this.nodes.get(index - 1).release(), this.nodes.get(index).release()) >= 0) {
          throw new IllegalArgumentException("Musubi verification nodes must be sorted and unique");
        }
      }
      for (final ExactDependencyEdge edge : this.rootDependencies) {
        if (edge.kind() != DependencyKind.NORMAL || findNode(edge.selected()) == null) {
          throw new IllegalArgumentException(
              "Musubi root dependency must be normal and select a proof node");
        }
      }
      for (final VerificationNode node : this.nodes) {
        for (final ExactDependencyEdge edge : node.dependencies()) {
          if (edge.kind() == DependencyKind.NORMAL && findNode(edge.selected()) == null) {
            throw new IllegalArgumentException(
                "Musubi verification graph references a missing node");
          }
        }
      }
      validateAcyclicGraph();
    }

    public String schema() { return "musubi-verification-lock"; }
    public int version() { return 1; }
    public ReleaseId root() { return root; }
    public List<ExactDependencyEdge> rootDependencies() { return rootDependencies; }
    public List<VerificationNode> nodes() { return nodes; }

    private VerificationNode findNode(final ReleaseId release) {
      for (final VerificationNode node : nodes) {
        if (node.release().equals(release)) return node;
      }
      return null;
    }

    private void validateAcyclicGraph() {
      final List<ReleaseId> complete = new ArrayList<>();
      final List<ReleaseId> visiting = new ArrayList<>();
      for (final VerificationNode node : nodes) {
        visit(node.release(), 1, visiting, complete);
      }
    }

    private void visit(
        final ReleaseId release,
        final int depth,
        final List<ReleaseId> visiting,
        final List<ReleaseId> complete) {
      if (depth > 64) {
        throw new IllegalArgumentException("Musubi verification graph exceeds depth 64");
      }
      if (complete.contains(release)) return;
      if (visiting.contains(release)) {
        throw new IllegalArgumentException("Musubi verification graph contains a cycle");
      }
      visiting.add(release);
      final VerificationNode node = findNode(release);
      if (node == null) {
        throw new IllegalArgumentException("Musubi verification graph references a missing node");
      }
      for (final ExactDependencyEdge edge : node.dependencies()) {
        if (edge.kind() == DependencyKind.NORMAL) {
          visit(edge.selected(), depth + 1, visiting, complete);
        }
      }
      visiting.remove(visiting.size() - 1);
      complete.add(release);
    }

    @Override Object toJsonValue() {
      final List<Object> edges = new ArrayList<>();
      final List<Object> nodeValues = new ArrayList<>();
      for (final ExactDependencyEdge edge : rootDependencies) edges.add(edge.toJsonValue());
      for (final VerificationNode node : nodes) nodeValues.add(node.toJsonValue());
      return object(
          "schema", schema(),
          "version", Integer.valueOf(version()),
          "root", root.toJsonValue(),
          "root_dependencies", edges,
          "nodes", nodeValues);
    }
  }

  /** Finalized registry snapshot plus normalized exact verification lock. */
  public static final class ResolutionProof extends WireValue {
    private final RegistrySnapshot snapshot;
    private final VerificationLock lock;

    public ResolutionProof(final RegistrySnapshot snapshot, final VerificationLock lock) {
      this.snapshot = Objects.requireNonNull(snapshot, "snapshot");
      this.lock = Objects.requireNonNull(lock, "lock");
    }

    public RegistrySnapshot snapshot() { return snapshot; }
    public VerificationLock lock() { return lock; }

    @Override Object toJsonValue() {
      return object("snapshot", snapshot.toJsonValue(), "lock", lock.toJsonValue());
    }
  }

  /** Immutable registry release manifest binding semantic content to one archive. */
  public static final class ReleaseManifest extends WireValue {
    private final ReleaseId release;
    private final AbiBinding abi;
    private final List<DependencyRequirement> dependencies;
    private final List<String> exports;
    private final Digest32 interfaceDigest;
    private final ReleaseMetadata metadata;
    private final Digest32 archiveId;
    private final Digest32 verificationLockDigest;

    public ReleaseManifest(
        final ReleaseId release,
        final AbiBinding abi,
        final List<DependencyRequirement> dependencies,
        final List<String> exports,
        final Digest32 interfaceDigest,
        final ReleaseMetadata metadata,
        final Digest32 archiveId,
        final Digest32 verificationLockDigest) {
      this.release = Objects.requireNonNull(release, "release");
      this.abi = Objects.requireNonNull(abi, "abi");
      this.dependencies = immutableList(dependencies);
      this.exports = immutableList(exports);
      this.interfaceDigest = requireNonZeroModelDigest(interfaceDigest, "interfaceDigest");
      this.metadata = Objects.requireNonNull(metadata, "metadata");
      this.archiveId = requireNonZeroModelDigest(archiveId, "archiveId");
      this.verificationLockDigest =
          requireNonZeroModelDigest(verificationLockDigest, "verificationLockDigest");
      if (this.dependencies.size() > 256 || this.exports.size() > 1_024) {
        throw new IllegalArgumentException("Musubi release manifest exceeds collection bounds");
      }
      requireStrictOrder(this.dependencies, "Musubi manifest dependencies");
      requireUniqueDependencyAliases(this.dependencies, "Musubi manifest dependencies");
      for (int index = 0; index < this.dependencies.size(); index++) {
        if (this.dependencies.get(index).packageId().equals(release.packageId())) {
          throw new IllegalArgumentException("Musubi release cannot depend on its own package");
        }
      }
      for (int index = 0; index < this.exports.size(); index++) {
        requireName(this.exports.get(index), "Musubi export");
        if (index > 0
            && compareUtf8Strings(this.exports.get(index - 1), this.exports.get(index)) >= 0) {
          throw new IllegalArgumentException("Musubi exports must be sorted and distinct");
        }
      }
    }

    public ReleaseId release() { return release; }
    public int editionVersion() { return 1; }
    public AbiBinding abi() { return abi; }
    public List<DependencyRequirement> dependencies() { return dependencies; }
    public List<String> exports() { return exports; }
    public Digest32 interfaceDigest() { return interfaceDigest; }
    public ReleaseMetadata metadata() { return metadata; }
    public Digest32 archiveId() { return archiveId; }
    public Digest32 verificationLockDigest() { return verificationLockDigest; }

    @Override Object toJsonValue() {
      final List<Object> dependencyValues = new ArrayList<>();
      for (final DependencyRequirement dependency : dependencies) {
        dependencyValues.add(dependency.toJsonValue());
      }
      return object(
          "release", release.toJsonValue(),
          "edition", object("kind", "V1", "value", null),
          "abi", abi.toJsonValue(),
          "dependencies", dependencyValues,
          "exports", exports,
          "interface_digest", interfaceDigest.toJsonValue(),
          "metadata", metadata.toJsonValue(),
          "archive_id", archiveId.toJsonValue(),
          "verification_lock_digest", verificationLockDigest.toJsonValue());
    }
  }

  /** Release manifest plus its independently validated exact resolution proof. */
  public static final class Publication extends WireValue {
    private final ReleaseManifest manifest;
    private final ResolutionProof resolution;

    public Publication(final ReleaseManifest manifest, final ResolutionProof resolution) {
      this.manifest = Objects.requireNonNull(manifest, "manifest");
      this.resolution = Objects.requireNonNull(resolution, "resolution");
      if (!resolution.lock().root().equals(manifest.release())
          || manifest.dependencies().size() != resolution.lock().rootDependencies().size()) {
        throw new IllegalArgumentException("Musubi publication proof does not bind its root");
      }
      for (int index = 0; index < manifest.dependencies().size(); index++) {
        final DependencyRequirement range = manifest.dependencies().get(index);
        final ExactDependencyEdge exact = resolution.lock().rootDependencies().get(index);
        if (exact.kind() != DependencyKind.NORMAL
            || !range.alias().equals(exact.alias())
            || !range.packageId().equals(exact.packageId())
            || !range.requirement().equals(exact.requirement())) {
          throw new IllegalArgumentException(
              "Musubi publication direct dependency proof is inconsistent");
        }
      }
    }

    public ReleaseManifest manifest() { return manifest; }
    public ResolutionProof resolution() { return resolution; }

    @Override Object toJsonValue() {
      return object("manifest", manifest.toJsonValue(), "resolution", resolution.toJsonValue());
    }
  }

  /** Canonical generation-bound namespace delegation statement. */
  public static final class NamespaceDelegationPayload extends WireValue {
    private final Digest32 namespaceBinding;
    private final BigInteger ownerGeneration;
    private final String owner;
    private final String delegate;
    private final BigInteger expiresAtHeight;

    public NamespaceDelegationPayload(
        final Digest32 namespaceBinding,
        final BigInteger ownerGeneration,
        final String owner,
        final String delegate,
        final BigInteger expiresAtHeight) {
      this.namespaceBinding = requireNonZeroModelDigest(namespaceBinding, "namespaceBinding");
      requireU64(ownerGeneration, "namespaceDelegation.ownerGeneration");
      requireU64(expiresAtHeight, "namespaceDelegation.expiresAtHeight");
      if (ownerGeneration.signum() == 0 || expiresAtHeight.signum() == 0) {
        throw new IllegalArgumentException("Musubi namespace delegation revisions are invalid");
      }
      this.ownerGeneration = ownerGeneration;
      this.owner = AccountIdLiteral.requireCanonicalI105Address(owner, "owner");
      this.delegate = AccountIdLiteral.requireCanonicalI105Address(delegate, "delegate");
      this.expiresAtHeight = expiresAtHeight;
    }

    public int version() { return 1; }
    public Digest32 namespaceBinding() { return namespaceBinding; }
    public BigInteger ownerGeneration() { return ownerGeneration; }
    public String owner() { return owner; }
    public String delegate() { return delegate; }
    public BigInteger expiresAtHeight() { return expiresAtHeight; }

    @Override Object toJsonValue() {
      return object(
          "version", Integer.valueOf(1),
          "namespace_binding", namespaceBinding.toJsonValue(),
          "owner_generation", ownerGeneration,
          "owner", owner,
          "delegate", delegate,
          "expires_at_height", expiresAtHeight);
    }
  }

  /** One namespace-owner controller approval. */
  public static final class NamespaceDelegationApproval extends WireValue {
    private final String publicKey;
    private final String signature;

    public NamespaceDelegationApproval(final String publicKey, final String signature) {
      this.publicKey = requireCanonicalPublicKey(publicKey, "Musubi delegation approval key");
      this.signature = requireCanonicalSignature(signature, "Musubi delegation signature");
    }

    public String publicKey() { return publicKey; }
    public String signature() { return signature; }

    @Override Object toJsonValue() {
      return object("public_key", publicKey, "signature", signature);
    }
  }

  /** Signed generation-bound authority to claim an absent package. */
  public static final class NamespaceDelegation extends WireValue {
    private final NamespaceDelegationPayload payload;
    private final List<NamespaceDelegationApproval> approvals;

    public NamespaceDelegation(
        final NamespaceDelegationPayload payload,
        final List<NamespaceDelegationApproval> approvals) {
      this.payload = Objects.requireNonNull(payload, "payload");
      this.approvals = immutableList(approvals);
      if (this.approvals.isEmpty() || this.approvals.size() > 64) {
        throw new IllegalArgumentException("Musubi namespace approvals are out of bounds");
      }
      for (int index = 1; index < this.approvals.size(); index++) {
        if (comparePublicKeyLiterals(
                this.approvals.get(index - 1).publicKey(),
                this.approvals.get(index).publicKey()) >= 0) {
          throw new IllegalArgumentException(
              "Musubi namespace approvals must be sorted by distinct public keys");
        }
      }
    }

    public NamespaceDelegationPayload payload() { return payload; }
    public List<NamespaceDelegationApproval> approvals() { return approvals; }

    @Override Object toJsonValue() {
      final List<Object> values = new ArrayList<>();
      for (final NamespaceDelegationApproval approval : approvals) {
        values.add(approval.toJsonValue());
      }
      return object("payload", payload.toJsonValue(), "approvals", values);
    }
  }

  /** Prospective whole-XOR prices for permanent global aliases. */
  public static final class AliasPricingPolicy extends WireValue {
    private final BigInteger revision;
    private final BigInteger length1Xor;
    private final BigInteger length2Xor;
    private final BigInteger length3Xor;
    private final BigInteger length4Xor;
    private final BigInteger length5To32Xor;

    public AliasPricingPolicy(
        final BigInteger revision,
        final BigInteger length1Xor,
        final BigInteger length2Xor,
        final BigInteger length3Xor,
        final BigInteger length4Xor,
        final BigInteger length5To32Xor) {
      final List<BigInteger> values = Arrays.asList(
          revision, length1Xor, length2Xor, length3Xor, length4Xor, length5To32Xor);
      for (final BigInteger value : values) {
        requireU64(value, "aliasPricing");
        if (value.signum() == 0) {
          throw new IllegalArgumentException("Musubi alias pricing values must be non-zero");
        }
      }
      this.revision = revision;
      this.length1Xor = length1Xor;
      this.length2Xor = length2Xor;
      this.length3Xor = length3Xor;
      this.length4Xor = length4Xor;
      this.length5To32Xor = length5To32Xor;
    }

    public BigInteger revision() { return revision; }
    public BigInteger length1Xor() { return length1Xor; }
    public BigInteger length2Xor() { return length2Xor; }
    public BigInteger length3Xor() { return length3Xor; }
    public BigInteger length4Xor() { return length4Xor; }
    public BigInteger length5To32Xor() { return length5To32Xor; }

    @Override Object toJsonValue() {
      return object(
          "revision", revision,
          "length_1_xor", length1Xor,
          "length_2_xor", length2Xor,
          "length_3_xor", length3Xor,
          "length_4_xor", length4Xor,
          "length_5_to_32_xor", length5To32Xor);
    }
  }

  /** Admission mode for new archives, releases, and aliases. */
  public enum RegistryAdmissionMode { CLOSED, ALLOWLISTED, OPEN }

  /** Closed first-release registry admission and alias-pricing policy. */
  public static final class RegistryPolicy extends WireValue {
    private final BigInteger revision;
    private final RegistryAdmissionMode mode;
    private final List<BigInteger> allowlistedDataspaces;
    private final AliasPricingPolicy aliasPricing;

    public RegistryPolicy(
        final BigInteger revision,
        final RegistryAdmissionMode mode,
        final List<BigInteger> allowlistedDataspaces,
        final AliasPricingPolicy aliasPricing) {
      requireU64(revision, "registryPolicy.revision");
      if (revision.signum() == 0) {
        throw new IllegalArgumentException("Musubi registry policy revision must be non-zero");
      }
      this.mode = Objects.requireNonNull(mode, "mode");
      this.allowlistedDataspaces = immutableList(allowlistedDataspaces);
      this.aliasPricing = Objects.requireNonNull(aliasPricing, "aliasPricing");
      if (this.allowlistedDataspaces.size() > 1_024
          || (mode != RegistryAdmissionMode.ALLOWLISTED
              && !this.allowlistedDataspaces.isEmpty())) {
        throw new IllegalArgumentException("Musubi registry policy allowlist is invalid");
      }
      for (int index = 0; index < this.allowlistedDataspaces.size(); index++) {
        requireU64(this.allowlistedDataspaces.get(index), "allowlistedDataspace");
        if (index > 0
            && this.allowlistedDataspaces.get(index - 1)
                .compareTo(this.allowlistedDataspaces.get(index)) >= 0) {
          throw new IllegalArgumentException(
              "Musubi allowlisted dataspaces must be sorted and distinct");
        }
      }
      this.revision = revision;
    }

    public int version() { return 1; }
    public BigInteger revision() { return revision; }
    public RegistryAdmissionMode mode() { return mode; }
    public List<BigInteger> allowlistedDataspaces() { return allowlistedDataspaces; }
    public AliasPricingPolicy aliasPricing() { return aliasPricing; }

    @Override Object toJsonValue() {
      final String modeName;
      switch (mode) {
        case CLOSED: modeName = "Closed"; break;
        case ALLOWLISTED: modeName = "Allowlisted"; break;
        case OPEN: modeName = "Open"; break;
        default: throw new IllegalStateException("unhandled Musubi policy mode");
      }
      return object(
          "version", Integer.valueOf(1),
          "revision", revision,
          "mode", object("kind", modeName, "value", null),
          "allowlisted_dataspaces", allowlistedDataspaces,
          "alias_pricing", aliasPricing.toJsonValue());
    }
  }

  /** Authoritative immutable archive registration. */
  public static final class ArchiveRecord extends WireValue {
    private final Digest32 archiveId;
    private final ArchiveCommitment commitment;
    private final SeedIngressReceipt stagingReceipt;
    private final String registeredBy;
    private final BigInteger registeredAtHeight;
    private final BigInteger locationRevision;
    private final List<Digest32> locationIds;

    ArchiveRecord(
        final Digest32 archiveId,
        final ArchiveCommitment commitment,
        final SeedIngressReceipt stagingReceipt,
        final String registeredBy,
        final BigInteger registeredAtHeight,
        final BigInteger locationRevision,
        final List<Digest32> locationIds) {
      if (!stagingReceipt.payload().binding().archiveId().equals(archiveId)
          || !stagingReceipt.payload().binding().carBodyDigest().equals(commitment.carDigest())
          || !stagingReceipt.payload().binding().carBodyLength().equals(commitment.carSize())
          || !stagingReceipt.payload().binding().publisher().equals(registeredBy)
          || registeredAtHeight.signum() <= 0 || locationRevision.signum() <= 0) {
        throw new IllegalArgumentException("Musubi archive record binding is invalid");
      }
      this.archiveId = archiveId;
      this.commitment = commitment;
      this.stagingReceipt = stagingReceipt;
      this.registeredBy = registeredBy;
      this.registeredAtHeight = registeredAtHeight;
      this.locationRevision = locationRevision;
      this.locationIds = immutableList(locationIds);
      if (this.locationIds.size() > 4) {
        throw new IllegalArgumentException("Musubi archive has too many locations");
      }
      for (int index = 0; index < this.locationIds.size(); index++) {
        if (allZero(this.locationIds.get(index).bytes())
            || (index > 0 && compareUnsignedBytes(
                this.locationIds.get(index - 1).bytes(),
                this.locationIds.get(index).bytes()) >= 0)) {
          throw new IllegalArgumentException(
              "Musubi archive location IDs must be non-inert, sorted, and distinct");
        }
      }
    }

    public Digest32 archiveId() { return archiveId; }
    public ArchiveCommitment commitment() { return commitment; }
    public SeedIngressReceipt stagingReceipt() { return stagingReceipt; }
    public String registeredBy() { return registeredBy; }
    public BigInteger registeredAtHeight() { return registeredAtHeight; }
    public BigInteger locationRevision() { return locationRevision; }
    public List<Digest32> locationIds() { return locationIds; }

    @Override Object toJsonValue() {
      final List<Object> ids = new ArrayList<>();
      for (final Digest32 locationId : locationIds) ids.add(locationId.toJsonValue());
      return object(
          "archive_id", archiveId.toJsonValue(),
          "commitment", commitment.toJsonValue(),
          "staging_receipt", stagingReceipt.toJsonValue(),
          "registered_by", registeredBy,
          "registered_at_height", registeredAtHeight,
          "location_revision", locationRevision,
          "location_ids", ids);
    }
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
      AccountIdLiteral.requireCanonicalI105Address(registeredBy, "alias registrant");
      for (final BigInteger value :
          Arrays.asList(pricingRevision, paidXor, registeredAtHeight, historyRevision)) {
        requireU64(value, "alias record counter");
        if (value.signum() == 0) {
          throw new IllegalArgumentException("Musubi alias record counters must be non-zero");
        }
      }
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

    /** Rejects an exact-alias response for another permanent alias. */
    public void requireMatches(final AliasQuery request) {
      if (!alias.equals(Objects.requireNonNull(request, "request").alias())) {
        throw new IllegalArgumentException("Musubi alias response does not match the request");
      }
    }

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
      requireAsciiKebab(alias, 32, "alias history alias");
      requireU64(revision, "alias history revision");
      requireU64(finalizedHeight, "alias history finalized height");
      final boolean validAction =
          ("Registered".equals(actionKind)
                  && revision.equals(BigInteger.ONE)
                  && previousTarget == null
                  && governanceAction == null)
              || ("ParliamentRetarget".equals(actionKind)
                  && revision.compareTo(BigInteger.ONE) > 0
                  && previousTarget != null
                  && governanceAction != null
                  && !allZero(governanceAction.bytes()));
      if (!validAction || finalizedHeight.signum() == 0) {
        throw new IllegalArgumentException("Musubi alias-history entry is invalid");
      }
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
      if (!selector.name().equals(packageId.name())) {
        throw new IllegalArgumentException(
            "Musubi ordered entry selector and package names disagree");
      }
      if (metadataRevision.signum() <= 0 || indexRevision.signum() <= 0) {
        throw new IllegalArgumentException("Musubi ordered entry revisions must be non-zero");
      }
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
    private final WireValue query;
    private final List<T> items;
    private final FinalizedCursor nextCursor;
    private final RegistrySnapshot snapshot;
    Page(
        final WireValue query,
        final List<T> items,
        final FinalizedCursor nextCursor,
        final RegistrySnapshot snapshot) {
      this.query = Objects.requireNonNull(query, "query");
      this.items = immutableList(items);
      if (this.items.size() > 100) throw new IllegalArgumentException("Musubi page exceeds 100 items");
      if (nextCursor != null && !nextCursor.snapshot().equals(snapshot)) {
        throw new IllegalArgumentException("Musubi page cursor uses another snapshot");
      }
      this.nextCursor = nextCursor; this.snapshot = snapshot;
    }
    public WireValue query() { return query; }
    public List<T> items() { return items; }
    public FinalizedCursor nextCursor() { return nextCursor; }
    public RegistrySnapshot snapshot() { return snapshot; }

    void requireVersionMatches(final PackagePageQuery request) {
      final List<Version> versions = new ArrayList<>();
      for (final T item : items) {
        if (!(item instanceof Version)) {
          throw new IllegalArgumentException("Musubi version page contains another item type");
        }
        versions.add((Version) item);
      }
      if (!query.equals(request)
          || !semVerPageAdvances(request.page(), versions.isEmpty() ? null : versions.get(0))) {
        throw new IllegalArgumentException(
            "Musubi version response does not match its structured request cursor");
      }
      requireFinalizedPageMatches(
          request.page(),
          versions.size(),
          versions.isEmpty() ? null : versions.get(0).canonicalText(),
          versions.isEmpty() ? null : versions.get(versions.size() - 1).canonicalText(),
          snapshot,
          nextCursor);
    }

    void requireMaintainerMatches(final PackagePageQuery request) {
      final List<MaintainerDirectoryEntry> maintainers = new ArrayList<>();
      for (final T item : items) {
        if (!(item instanceof MaintainerDirectoryEntry)) {
          throw new IllegalArgumentException("Musubi maintainer page contains another item type");
        }
        final MaintainerDirectoryEntry entry = (MaintainerDirectoryEntry) item;
        if (!maintainerPackageId(entry).equals(request.packageId())) {
          throw new IllegalArgumentException(
              "Musubi maintainer response contains another package");
        }
        maintainers.add(entry);
      }
      if (!query.equals(request)
          || !maintainerPageAdvances(request.page(), maintainers)) {
        throw new IllegalArgumentException(
            "Musubi maintainer response does not match its structured request cursor");
      }
      requireFinalizedPageMatches(
          request.page(),
          maintainers.size(),
          maintainers.isEmpty() ? null : maintainerCursorKey(maintainers.get(0)),
          maintainers.isEmpty()
              ? null : maintainerCursorKey(maintainers.get(maintainers.size() - 1)),
          snapshot,
          nextCursor);
    }

    void requireAliasHistoryMatches(final AliasQuery request) {
      final List<AliasHistoryEntry> history = new ArrayList<>();
      for (final T item : items) {
        if (!(item instanceof AliasHistoryEntry)) {
          throw new IllegalArgumentException(
              "Musubi alias-history page contains another item type");
        }
        final AliasHistoryEntry entry = (AliasHistoryEntry) item;
        if (!entry.alias().equals(request.alias())) {
          throw new IllegalArgumentException(
              "Musubi alias-history response contains another alias");
        }
        history.add(entry);
      }
      if (!query.equals(request)
          || !aliasHistoryPageAdvances(
              request, history.isEmpty() ? null : history.get(0))) {
        throw new IllegalArgumentException(
            "Musubi alias-history response does not match its structured request cursor");
      }
      requireFinalizedPageMatches(
          request.page(),
          history.size(),
          history.isEmpty() ? null : aliasHistoryCursorKey(history.get(0)),
          history.isEmpty() ? null : aliasHistoryCursorKey(history.get(history.size() - 1)),
          snapshot,
          nextCursor);
    }
    @Override Object toJsonValue() {
      final List<Object> values = new ArrayList<>();
      for (final T item : items) values.add(item.toJsonValue());
      return object(
          "query", query.toJsonValue(),
          "items", values,
          "next_cursor", nextCursor == null ? null : nextCursor.toJsonValue(),
          "snapshot", snapshot.toJsonValue());
    }
  }

  /** Resolver page carrying the exact chain/genesis identity required by lockfiles. */
  public static final class ResolverIndexPage extends WireValue {
    private final ResolverIndexQuery query;
    private final String chainId;
    private final byte[] genesisHash;
    private final List<ResolverReleaseRow> items;
    private final FinalizedCursor nextCursor;
    private final RegistrySnapshot snapshot;

    ResolverIndexPage(
        final ResolverIndexQuery query,
        final String chainId,
        final byte[] genesisHash,
        final List<ResolverReleaseRow> items,
        final FinalizedCursor nextCursor,
        final RegistrySnapshot snapshot) {
      this.query = Objects.requireNonNull(query, "query");
      requireExactText(chainId, "Musubi resolver chain ID");
      if (genesisHash == null || genesisHash.length != 32 || allZero(genesisHash)) {
        throw new IllegalArgumentException("Musubi genesis hash must contain 32 bytes");
      }
      this.chainId = chainId;
      this.genesisHash = genesisHash.clone();
      this.items = immutableList(items);
      if (this.items.size() > 100) {
        throw new IllegalArgumentException("Musubi resolver page exceeds 100 items");
      }
      for (int index = 1; index < this.items.size(); index++) {
        if (compareReleaseIds(
                this.items.get(index - 1).release(), this.items.get(index).release()) >= 0) {
          throw new IllegalArgumentException(
              "Musubi resolver page rows must be sorted and distinct");
        }
      }
      if (nextCursor != null && !nextCursor.snapshot().equals(snapshot)) {
        throw new IllegalArgumentException("Musubi resolver cursor uses another snapshot");
      }
      this.nextCursor = nextCursor;
      this.snapshot = snapshot;
    }

    public ResolverIndexQuery query() { return query; }
    public String chainId() { return chainId; }
    public byte[] genesisHash() { return genesisHash.clone(); }
    public List<ResolverReleaseRow> items() { return items; }
    public FinalizedCursor nextCursor() { return nextCursor; }
    public RegistrySnapshot snapshot() { return snapshot; }

    /** Binds every resolver row and any continuation snapshot to the exact request. */
    public void requireMatches(final ResolverIndexQuery request) {
      Objects.requireNonNull(request, "request");
      if (!query.equals(request)) {
        throw new IllegalArgumentException(
            "Musubi resolver response query does not match the request");
      }
      for (final ResolverReleaseRow item : items) {
        if (!item.release().packageId().equals(request.packageId())
            || (request.requirement() != null
                && !request.requirement().matches(item.release().version()))) {
          throw new IllegalArgumentException(
              "Musubi resolver response contains another package or an excluded version");
        }
      }
      final Version firstVersion = items.isEmpty() ? null : items.get(0).release().version();
      if (!semVerPageAdvances(request.page(), firstVersion)) {
        throw new IllegalArgumentException(
            "Musubi resolver response does not advance its structured cursor");
      }
      requireFinalizedPageMatches(
          request.page(),
          items.size(),
          firstVersion == null ? null : firstVersion.canonicalText(),
          items.isEmpty()
              ? null : items.get(items.size() - 1).release().version().canonicalText(),
          snapshot,
          nextCursor,
          false);
    }

    @Override Object toJsonValue() {
      final List<Object> values = new ArrayList<>();
      for (final ResolverReleaseRow item : items) values.add(item.toJsonValue());
      final List<Integer> genesis = new ArrayList<>();
      for (final byte value : genesisHash) genesis.add(Integer.valueOf(value & 0xff));
      return object(
          "query", query.toJsonValue(),
          "chain_id", chainId,
          "genesis_hash", genesis,
          "items", values,
          "next_cursor", nextCursor == null ? null : nextCursor.toJsonValue(),
          "snapshot", snapshot.toJsonValue());
    }
  }

  /** Archive-location page carrying deployment identity and the immutable commitment. */
  public static final class ArchiveLocationPage extends WireValue {
    private final String chainId;
    private final byte[] genesisHash;
    private final ArchiveRecord archive;
    private final List<ArchiveLocation> items;
    private final FinalizedCursor nextCursor;
    private final RegistrySnapshot snapshot;

    ArchiveLocationPage(
        final String chainId,
        final byte[] genesisHash,
        final ArchiveRecord archive,
        final List<ArchiveLocation> items,
        final FinalizedCursor nextCursor,
        final RegistrySnapshot snapshot) {
      requireExactText(chainId, "Musubi archive-location chain ID");
      if (genesisHash == null || genesisHash.length != 32 || allZero(genesisHash)
          || !archive.stagingReceipt().payload().binding().chainId().equals(chainId)
          || !Arrays.equals(
              archive.stagingReceipt().payload().binding().genesisBlockHash(), genesisHash)
          || archive.registeredAtHeight().compareTo(snapshot.finalizedHeight()) > 0) {
        throw new IllegalArgumentException("Musubi archive page deployment identity is invalid");
      }
      this.chainId = chainId;
      this.genesisHash = genesisHash.clone();
      this.archive = archive;
      this.items = immutableList(items);
      if (this.items.size() > 4) {
        throw new IllegalArgumentException("Musubi archive-location page has too many items");
      }
      for (int index = 0; index < this.items.size(); index++) {
        final ArchiveLocation item = this.items.get(index);
        if (!item.archiveId().equals(archive.archiveId())
            || !archive.locationIds().contains(item.locationId())
            || "Retired".equals(item.stateKind())
            || item.finalizedHeight().compareTo(snapshot.finalizedHeight()) > 0
            || item.revision().compareTo(archive.locationRevision()) > 0
            || (index > 0 && compareUnsignedBytes(
                this.items.get(index - 1).locationId().bytes(), item.locationId().bytes()) >= 0)) {
          throw new IllegalArgumentException(
              "Musubi archive-location page item is not a current archive location");
        }
      }
      if (nextCursor != null && !nextCursor.snapshot().equals(snapshot)) {
        throw new IllegalArgumentException("Musubi archive-location cursor uses another snapshot");
      }
      this.nextCursor = nextCursor;
      this.snapshot = snapshot;
    }

    public String chainId() { return chainId; }
    public byte[] genesisHash() { return genesisHash.clone(); }
    public ArchiveRecord archive() { return archive; }
    public List<ArchiveLocation> items() { return items; }
    public FinalizedCursor nextCursor() { return nextCursor; }
    public RegistrySnapshot snapshot() { return snapshot; }

    /** Binds the archive record and any continuation snapshot to the exact request. */
    public void requireMatches(final ArchiveLocationQuery request) {
      Objects.requireNonNull(request, "request");
      if (!archive.archiveId().equals(request.archiveId())) {
        throw new IllegalArgumentException(
            "Musubi archive-location response does not match the request");
      }
      requirePageMatches(request.page(), snapshot, nextCursor, items.size());
    }

    @Override Object toJsonValue() {
      final List<Object> values = new ArrayList<>();
      for (final ArchiveLocation item : items) values.add(item.toJsonValue());
      return object(
          "chain_id", chainId,
          "genesis_hash", unsignedBytes(genesisHash),
          "archive", archive.toJsonValue(),
          "items", values,
          "next_cursor", nextCursor == null ? null : nextCursor.toJsonValue(),
          "snapshot", snapshot.toJsonValue());
    }
  }

  /** Exact finalized cache-retention decisions for one bounded request batch. */
  public static final class ArchiveRetentionPage extends WireValue {
    private final String chainId;
    private final byte[] genesisHash;
    private final List<ArchiveRetentionDecision> items;
    private final BigInteger finalizedTimeMs;
    private final RegistrySnapshot snapshot;

    ArchiveRetentionPage(
        final String chainId,
        final byte[] genesisHash,
        final List<ArchiveRetentionDecision> items,
        final BigInteger finalizedTimeMs,
        final RegistrySnapshot snapshot) {
      requireExactText(chainId, "Musubi archive-retention chain ID");
      requireU64(finalizedTimeMs, "archiveRetention.finalizedTimeMs");
      if (genesisHash == null || genesisHash.length != 32 || allZero(genesisHash)) {
        throw new IllegalArgumentException("Musubi archive-retention genesis hash is invalid");
      }
      this.chainId = chainId;
      this.genesisHash = genesisHash.clone();
      this.items = immutableList(items);
      this.finalizedTimeMs = finalizedTimeMs;
      this.snapshot = Objects.requireNonNull(snapshot, "snapshot");
      if (this.items.isEmpty() || this.items.size() > 100) {
        throw new IllegalArgumentException(
            "Musubi archive-retention page has an invalid item bound");
      }
      for (int index = 0; index < this.items.size(); index++) {
        final ArchiveRetentionDecision decision =
            Objects.requireNonNull(this.items.get(index), "retentionDecision");
        if (index > 0 && compareUnsignedBytes(
            this.items.get(index - 1).archiveId().bytes(), decision.archiveId().bytes()) >= 0) {
          throw new IllegalArgumentException(
              "Musubi archive-retention page identities are not canonical");
        }
        final ArchiveAvailability storage = decision.storage();
        if (storage != null
            && (storage.finalizedHeight().compareTo(snapshot.finalizedHeight()) > 0
                || storage.indexRevision().compareTo(snapshot.indexRevision()) > 0
                || (storage.finalizedHeight().equals(snapshot.finalizedHeight())
                    && !Arrays.equals(
                        storage.finalizedBlockHash(), snapshot.finalizedBlockHash())))) {
          throw new IllegalArgumentException(
              "Musubi archive-retention storage projection exceeds the page snapshot");
        }
      }
    }

    public String chainId() { return chainId; }
    public byte[] genesisHash() { return genesisHash.clone(); }
    public List<ArchiveRetentionDecision> items() { return items; }
    public BigInteger finalizedTimeMs() { return finalizedTimeMs; }
    public RegistrySnapshot snapshot() { return snapshot; }

    /** Enforces the exact request identity order and optional snapshot binding. */
    public void requireMatches(final ArchiveRetentionQuery request) {
      Objects.requireNonNull(request, "request");
      if ((request.expectedSnapshot() != null
              && !request.expectedSnapshot().equals(snapshot))
          || request.archiveIds().size() != items.size()) {
        throw new IllegalArgumentException(
            "Musubi archive-retention response does not match the exact request");
      }
      for (int index = 0; index < items.size(); index++) {
        if (!items.get(index).archiveId().equals(request.archiveIds().get(index))) {
          throw new IllegalArgumentException(
              "Musubi archive-retention response identities differ from the exact request");
        }
      }
    }

    @Override Object toJsonValue() {
      final List<Object> values = new ArrayList<>();
      for (final ArchiveRetentionDecision item : items) values.add(item.toJsonValue());
      return object(
          "chain_id", chainId,
          "genesis_hash", unsignedBytes(genesisHash),
          "items", values,
          "finalized_time_ms", finalizedTimeMs,
          "snapshot", snapshot.toJsonValue());
    }
  }

  /** Ordered-directory page carrying exact chain/genesis identity for lock creation. */
  public static final class OrderedPrefixPage extends WireValue {
    private final OrderedPrefixQuery query;
    private final String chainId;
    private final byte[] genesisHash;
    private final NamespaceBinding namespaceBinding;
    private final List<OrderedPackageEntry> items;
    private final FinalizedCursor nextCursor;
    private final RegistrySnapshot snapshot;

    OrderedPrefixPage(
        final OrderedPrefixQuery query,
        final String chainId,
        final byte[] genesisHash,
        final NamespaceBinding namespaceBinding,
        final List<OrderedPackageEntry> items,
        final FinalizedCursor nextCursor,
        final RegistrySnapshot snapshot) {
      this.query = Objects.requireNonNull(query, "query");
      requireExactText(chainId, "Musubi directory chain ID");
      if (genesisHash == null || genesisHash.length != 32 || allZero(genesisHash)) {
        throw new IllegalArgumentException("Musubi genesis hash must contain 32 bytes");
      }
      this.chainId = chainId;
      this.genesisHash = genesisHash.clone();
      this.namespaceBinding = namespaceBinding;
      this.items = immutableList(items);
      if (this.items.size() > 100) {
        throw new IllegalArgumentException("Musubi ordered-prefix page exceeds 100 items");
      }
      if (nextCursor != null && !nextCursor.snapshot().equals(snapshot)) {
        throw new IllegalArgumentException("Musubi ordered-prefix cursor uses another snapshot");
      }
      for (final OrderedPackageEntry item : this.items) {
        if (!item.selector().namespace().equals(namespaceBinding.namespace())
            || !item.packageId().homeDataspace().equals(namespaceBinding.homeDataspace())
            || !item.packageId().scope().equals(namespaceBinding.scope())) {
          throw new IllegalArgumentException(
              "Musubi ordered-prefix row does not match its namespace binding");
        }
      }
      for (int index = 1; index < this.items.size(); index++) {
        final PackageSelector left = this.items.get(index - 1).selector();
        final PackageSelector right = this.items.get(index).selector();
        final int namespaceOrder = compareUnsignedBytes(
            left.namespace().value().getBytes(StandardCharsets.UTF_8),
            right.namespace().value().getBytes(StandardCharsets.UTF_8));
        final int nameOrder = compareUnsignedBytes(
            left.name().value().getBytes(StandardCharsets.UTF_8),
            right.name().value().getBytes(StandardCharsets.UTF_8));
        if (namespaceOrder > 0 || (namespaceOrder == 0 && nameOrder >= 0)) {
          throw new IllegalArgumentException(
              "Musubi ordered-prefix rows must be sorted and distinct");
        }
      }
      this.nextCursor = nextCursor;
      this.snapshot = snapshot;
    }

    public OrderedPrefixQuery query() { return query; }
    public String chainId() { return chainId; }
    public byte[] genesisHash() { return genesisHash.clone(); }
    public NamespaceBinding namespaceBinding() { return namespaceBinding; }
    public List<OrderedPackageEntry> items() { return items; }
    public FinalizedCursor nextCursor() { return nextCursor; }
    public RegistrySnapshot snapshot() { return snapshot; }

    /** Binds directory rows and any continuation snapshot to the requested prefix. */
    public void requireMatches(final OrderedPrefixQuery request) {
      Objects.requireNonNull(request, "request");
      if (!query.equals(request)) {
        throw new IllegalArgumentException(
            "Musubi ordered-prefix response query does not match the request");
      }
      final int separator = request.prefix().indexOf('/');
      if (separator <= 0
          || !namespaceBinding.namespace().value().equals(request.prefix().substring(0, separator))) {
        throw new IllegalArgumentException(
            "Musubi ordered-prefix namespace binding does not match the request");
      }
      for (final OrderedPackageEntry item : items) {
        final String selector =
            item.selector().namespace().value() + "/" + item.selector().name().value();
        if (!selector.startsWith(request.prefix())) {
          throw new IllegalArgumentException(
              "Musubi ordered-prefix response contains a selector outside the request");
        }
      }
      if (!orderedPrefixPageAdvances(request, items.isEmpty() ? null : items.get(0))) {
        throw new IllegalArgumentException(
            "Musubi ordered-prefix response does not advance its structured cursor");
      }
      requireFinalizedPageMatches(
          request.page(),
          items.size(),
          items.isEmpty() ? null : orderedSelectorCursorKey(items.get(0)),
          items.isEmpty() ? null : orderedSelectorCursorKey(items.get(items.size() - 1)),
          snapshot,
          nextCursor);
    }

    @Override Object toJsonValue() {
      final List<Object> values = new ArrayList<>();
      for (final OrderedPackageEntry item : items) values.add(item.toJsonValue());
      final List<Integer> genesis = new ArrayList<>();
      for (final byte value : genesisHash) genesis.add(Integer.valueOf(value & 0xff));
      return object(
          "query", query.toJsonValue(),
          "chain_id", chainId,
          "genesis_hash", genesis,
          "namespace_binding", namespaceBinding.toJsonValue(),
          "items", values,
          "next_cursor", nextCursor == null ? null : nextCursor.toJsonValue(),
          "snapshot", snapshot.toJsonValue());
    }
  }

  /** One exact-token package metadata search result. */
  public static final class SearchHit extends WireValue {
    private final PackageId packageId;
    private final Namespace claimedNamespace;
    private final String description;
    private final List<String> keywords;
    private final BigInteger metadataRevision;

    SearchHit(
        final PackageId packageId,
        final Namespace claimedNamespace,
        final String description,
        final List<String> keywords,
        final BigInteger metadataRevision) {
      this.packageId = Objects.requireNonNull(packageId, "packageId");
      this.claimedNamespace = Objects.requireNonNull(claimedNamespace, "claimedNamespace");
      if (description != null) {
        requireExactText(description, "Musubi search description");
        if (description.getBytes(StandardCharsets.UTF_8).length > 4_096) {
          throw new IllegalArgumentException("Musubi search description exceeds 4096 bytes");
        }
      }
      this.description = description;
      this.keywords = immutableList(keywords);
      if (this.keywords.size() > 32) {
        throw new IllegalArgumentException("Musubi search hit has too many keywords");
      }
      for (final String keyword : this.keywords) requireAsciiKebab(keyword, 64, "keyword");
      for (int index = 1; index < this.keywords.size(); index++) {
        if (this.keywords.get(index - 1).compareTo(this.keywords.get(index)) >= 0) {
          throw new IllegalArgumentException("Musubi search keywords must be sorted and distinct");
        }
      }
      requireU64(metadataRevision, "searchHit.metadataRevision");
      if (metadataRevision.signum() == 0 || !namespaceMatchesScope(packageId, claimedNamespace)) {
        throw new IllegalArgumentException("Musubi search hit is invalid");
      }
      this.metadataRevision = metadataRevision;
    }

    public PackageId packageId() { return packageId; }
    public Namespace claimedNamespace() { return claimedNamespace; }
    public String description() { return description; }
    public List<String> keywords() { return keywords; }
    public BigInteger metadataRevision() { return metadataRevision; }

    @Override Object toJsonValue() {
      final List<List<String>> keywordValues = new ArrayList<>();
      for (final String keyword : keywords) keywordValues.add(Collections.singletonList(keyword));
      return object(
          "package", packageId.toJsonValue(),
          "claimed_namespace", claimedNamespace.toJsonValue(),
          "description", description == null ? null : Collections.singletonList(description),
          "keywords", keywordValues,
          "metadata_revision", metadataRevision);
    }
  }

  /** Bounded page from the finalized-event package-search projection. */
  public static final class SearchPage extends WireValue {
    private final SearchQuery query;
    private final List<SearchHit> items;
    private final SearchCursor nextCursor;
    private final SearchSnapshot snapshot;

    SearchPage(
        final SearchQuery query,
        final List<SearchHit> items,
        final SearchCursor nextCursor,
        final SearchSnapshot snapshot) {
      this.query = Objects.requireNonNull(query, "query");
      this.items = immutableList(items);
      this.nextCursor = nextCursor;
      this.snapshot = Objects.requireNonNull(snapshot, "snapshot");
      if (this.items.size() > 100) {
        throw new IllegalArgumentException("Musubi search page exceeds 100 items");
      }
      for (int index = 1; index < this.items.size(); index++) {
        if (comparePackageIds(
                this.items.get(index - 1).packageId(), this.items.get(index).packageId()) >= 0) {
          throw new IllegalArgumentException("Musubi search page items must be sorted and distinct");
        }
      }
      if (nextCursor != null
          && (!nextCursor.snapshot().equals(snapshot)
              || this.items.isEmpty()
              || !nextCursor.lastPackage().equals(this.items.get(this.items.size() - 1).packageId()))) {
        throw new IllegalArgumentException("Musubi search page cursor is invalid");
      }
    }

    public SearchQuery query() { return query; }
    public List<SearchHit> items() { return items; }
    public SearchCursor nextCursor() { return nextCursor; }
    public SearchSnapshot snapshot() { return snapshot; }

    /** Binds the echoed query and any continuation to the exact request. */
    public void requireMatches(final SearchQuery request) {
      Objects.requireNonNull(request, "request");
      if (!query.equals(request)) {
        throw new IllegalArgumentException(
            "Musubi search response query does not match the request");
      }
      final int effectiveLimit = effectivePageLimit(request.page().limit());
      final SearchCursor cursor = request.page().cursor();
      if (items.size() > effectiveLimit
          || (cursor != null
              && (!snapshot.equals(cursor.snapshot())
                  || (!items.isEmpty()
                      && comparePackageIds(cursor.lastPackage(), items.get(0).packageId()) >= 0)
                  || (nextCursor != null
                      && !nextCursor.queryHash().equals(cursor.queryHash()))))
          || (nextCursor != null
              && (!nextCursor.snapshot().equals(snapshot)
                  || items.size() != effectiveLimit
                  || items.isEmpty()
                  || !nextCursor.lastPackage().equals(
                      items.get(items.size() - 1).packageId())))) {
        throw new IllegalArgumentException(
            "Musubi search response has an invalid structured cursor or page boundary");
      }
    }

    @Override Object toJsonValue() {
      final List<Object> values = new ArrayList<>();
      for (final SearchHit item : items) values.add(item.toJsonValue());
      return object(
          "query", query.toJsonValue(),
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

  private static void requireChainId(final String value, final String field) {
    requireExactText(value, field);
    if (value.length() > 128
        || !value.matches("[A-Za-z0-9](?:[A-Za-z0-9._:-]*[A-Za-z0-9])?")) {
      throw new IllegalArgumentException(field + " is not a canonical ChainId");
    }
  }

  private static String requireCanonicalPublicKey(final String value, final String field) {
    final PublicKeyCodec.PublicKeyPayload decoded = PublicKeyCodec.decodePublicKeyLiteral(value);
    if (decoded == null) {
      throw new IllegalArgumentException(field + " is not a supported canonical public key");
    }
    final String canonical =
        PublicKeyCodec.encodePublicKeyMultihash(decoded.curveId(), decoded.keyBytes());
    if (!canonical.equals(value)) {
      throw new IllegalArgumentException(field + " is not in canonical multihash form");
    }
    return value;
  }

  private static String requireCanonicalSignature(final String value, final String field) {
    requireExactText(value, field);
    if ((value.length() & 1) != 0 || !value.matches("[0-9A-F]+")) {
      throw new IllegalArgumentException(field + " must be canonical uppercase hex");
    }
    boolean nonZero = false;
    for (int index = 0; index < value.length(); index += 2) {
      nonZero |= Integer.parseInt(value.substring(index, index + 2), 16) != 0;
    }
    if (!nonZero) throw new IllegalArgumentException(field + " must not be inert");
    return value;
  }

  private static int comparePublicKeyLiterals(final String left, final String right) {
    final PublicKeyCodec.PublicKeyPayload leftKey = PublicKeyCodec.decodePublicKeyLiteral(left);
    final PublicKeyCodec.PublicKeyPayload rightKey = PublicKeyCodec.decodePublicKeyLiteral(right);
    return compareUnsignedBytes(
        PublicKeyCodec.compactPublicKeyPayload(leftKey.curveId(), leftKey.keyBytes()),
        PublicKeyCodec.compactPublicKeyPayload(rightKey.curveId(), rightKey.keyBytes()));
  }

  private static void requireStrictlyOrderedApprovals(
      final List<SeedIngressReceiptApproval> approvals) {
    for (int index = 1; index < approvals.size(); index++) {
      if (comparePublicKeyLiterals(
              approvals.get(index - 1).publicKey(), approvals.get(index).publicKey()) >= 0) {
        throw new IllegalArgumentException(
            "Musubi controller approvals must be sorted by distinct public keys");
      }
    }
  }

  private static void requireBoundedCleanText(
      final String value, final int maximumUtf8Bytes, final String field) {
    if (value == null || value.isEmpty()) {
      throw new IllegalArgumentException(field + " must be non-empty");
    }
    if (isRustWhitespace(value.codePointAt(0))
        || isRustWhitespace(value.codePointBefore(value.length()))) {
      throw new IllegalArgumentException(field + " must not have surrounding whitespace");
    }
    for (int offset = 0; offset < value.length(); ) {
      final int codePoint = value.codePointAt(offset);
      if (codePoint >= Character.MIN_SURROGATE && codePoint <= Character.MAX_SURROGATE) {
        throw new IllegalArgumentException(field + " contains an unpaired UTF-16 surrogate");
      }
      if (Character.isISOControl(codePoint)) {
        throw new IllegalArgumentException(field + " contains a control character");
      }
      offset += Character.charCount(codePoint);
    }
    if (value.getBytes(StandardCharsets.UTF_8).length > maximumUtf8Bytes) {
      throw new IllegalArgumentException(field + " exceeds " + maximumUtf8Bytes + " UTF-8 bytes");
    }
  }

  private static boolean isRustWhitespace(final int codePoint) {
    return (codePoint >= 0x0009 && codePoint <= 0x000d)
        || codePoint == 0x0020
        || codePoint == 0x0085
        || codePoint == 0x00a0
        || codePoint == 0x1680
        || (codePoint >= 0x2000 && codePoint <= 0x200a)
        || codePoint == 0x2028
        || codePoint == 0x2029
        || codePoint == 0x202f
        || codePoint == 0x205f
        || codePoint == 0x3000;
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
    for (int offset = 0; offset < value.length(); ) {
      final int codePoint = value.codePointAt(offset);
      if (Character.isWhitespace(codePoint) || isBidiControl(codePoint)) {
        throw new IllegalArgumentException(field + " contains a forbidden character");
      }
      offset += Character.charCount(codePoint);
    }
  }

  private static boolean isBidiControl(final int codePoint) {
    return codePoint == 0x061c
        || codePoint == 0x200e
        || codePoint == 0x200f
        || (codePoint >= 0x202a && codePoint <= 0x202e)
        || (codePoint >= 0x2066 && codePoint <= 0x2069);
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
    final List<T> copy = new ArrayList<>(Objects.requireNonNull(source, "source"));
    for (final T value : copy) Objects.requireNonNull(value, "list element");
    return Collections.unmodifiableList(copy);
  }

  private static Digest32 requireNonZeroModelDigest(final Digest32 value, final String field) {
    Objects.requireNonNull(value, field);
    if (allZero(value.bytes())) {
      throw new IllegalArgumentException("Musubi " + field + " must not be inert");
    }
    return value;
  }

  private static byte[] hexBytes(final String value) {
    if (value == null || (value.length() & 1) != 0 || !value.matches("[0-9A-Fa-f]*")) {
      throw new IllegalArgumentException("Musubi value is not hexadecimal");
    }
    final byte[] bytes = new byte[value.length() / 2];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] = (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return bytes;
  }

  private static <T extends Comparable<? super T>> void requireStrictOrder(
      final List<T> values, final String field) {
    for (int index = 1; index < values.size(); index++) {
      if (values.get(index - 1).compareTo(values.get(index)) >= 0) {
        throw new IllegalArgumentException(field + " must be sorted and distinct");
      }
    }
  }

  static void requireCanonicalDependencyRequirements(
      final List<DependencyRequirement> dependencies, final String field) {
    if (dependencies.size() > 256) {
      throw new IllegalArgumentException(field + " exceeds the dependency bound");
    }
    requireStrictOrder(dependencies, field);
    requireUniqueDependencyAliases(dependencies, field);
  }

  private static void requireUniqueDependencyAliases(
      final List<DependencyRequirement> dependencies, final String field) {
    final Set<String> aliases = new LinkedHashSet<>();
    for (final DependencyRequirement dependency : dependencies) {
      if (!aliases.add(dependency.alias())) {
        throw new IllegalArgumentException(field + " must use unique parent-local aliases");
      }
    }
  }

  private static void requireUniqueExactDependencyAliases(
      final List<ExactDependencyEdge> dependencies, final String field) {
    final Set<String> aliases = new LinkedHashSet<>();
    for (final ExactDependencyEdge dependency : dependencies) {
      if (!aliases.add(dependency.alias())) {
        throw new IllegalArgumentException(field + " must use unique parent-local aliases");
      }
    }
  }

  private static List<Integer> unsignedBytes(final byte[] bytes) {
    final List<Integer> values = new ArrayList<>();
    for (final byte value : bytes) values.add(Integer.valueOf(value & 0xff));
    return values;
  }

  private static boolean allZero(final byte[] bytes) {
    for (final byte value : bytes) if (value != 0) return false;
    return true;
  }

  private static int compareUnsignedBytes(final byte[] left, final byte[] right) {
    final int common = Math.min(left.length, right.length);
    for (int index = 0; index < common; index++) {
      final int comparison = Integer.compare(left[index] & 0xff, right[index] & 0xff);
      if (comparison != 0) return comparison;
    }
    return Integer.compare(left.length, right.length);
  }

  private static int compareUtf8Strings(final String left, final String right) {
    return compareUnsignedBytes(
        left.getBytes(StandardCharsets.UTF_8), right.getBytes(StandardCharsets.UTF_8));
  }

  private static int comparePackageIds(final PackageId left, final PackageId right) {
    int comparison = left.homeDataspace().compareTo(right.homeDataspace());
    if (comparison != 0) return comparison;
    comparison = left.scope().kind().compareTo(right.scope().kind());
    if (comparison != 0) return comparison;
    if (left.scope().domain() != null) {
      comparison = compareUnsignedBytes(
          left.scope().domain().getBytes(StandardCharsets.UTF_8),
          right.scope().domain().getBytes(StandardCharsets.UTF_8));
      if (comparison != 0) return comparison;
    }
    return compareUnsignedBytes(
        left.name().value().getBytes(StandardCharsets.UTF_8),
        right.name().value().getBytes(StandardCharsets.UTF_8));
  }

  static int compareReleaseIds(final ReleaseId left, final ReleaseId right) {
    final int packageComparison = comparePackageIds(left.packageId(), right.packageId());
    return packageComparison != 0
        ? packageComparison
        : left.version().compareTo(right.version());
  }

  static int compareMaintainerEntries(
      final MaintainerDirectoryEntry left, final MaintainerDirectoryEntry right) {
    int comparison = comparePackageIds(maintainerPackageId(left), maintainerPackageId(right));
    if (comparison != 0) return comparison;
    comparison = compareAccountIds(maintainerAccount(left), maintainerAccount(right));
    if (comparison != 0) return comparison;
    final Digest32 leftInvitation = maintainerInvitation(left);
    final Digest32 rightInvitation = maintainerInvitation(right);
    if (leftInvitation == null) return rightInvitation == null ? 0 : -1;
    if (rightInvitation == null) return 1;
    return compareUnsignedBytes(leftInvitation.bytes(), rightInvitation.bytes());
  }

  static PackageId maintainerPackageId(final MaintainerDirectoryEntry entry) {
    return entry.kind() == MaintainerDirectoryEntry.Kind.ACCEPTED
        ? entry.acceptedMember().packageId()
        : entry.pendingInvitation().packageId();
  }

  private static String maintainerAccount(final MaintainerDirectoryEntry entry) {
    return entry.kind() == MaintainerDirectoryEntry.Kind.ACCEPTED
        ? entry.acceptedMember().account()
        : entry.pendingInvitation().invitedAccount();
  }

  private static Digest32 maintainerInvitation(final MaintainerDirectoryEntry entry) {
    return entry.kind() == MaintainerDirectoryEntry.Kind.ACCEPTED
        ? null : entry.pendingInvitation().inviteId();
  }

  static String maintainerCursorKey(final MaintainerDirectoryEntry entry) {
    final String account = lowerHex(
        TransferWirePayloadEncoder.encodeAccountIdPayload(
            AccountIdLiteral.requireCanonicalI105Address(
                maintainerAccount(entry), "maintainer cursor account")));
    final Digest32 invitation = maintainerInvitation(entry);
    return account + "|" + (invitation == null
        ? "accepted" : "pending-" + lowerHex(invitation.bytes()));
  }

  private static boolean maintainerPageAdvances(
      final PageRequest request, final List<MaintainerDirectoryEntry> entries) {
    final FinalizedCursor cursor = request.cursor();
    if (cursor == null) return true;
    if (!parseMaintainerCursorBoundary(cursor.lastKey())) return false;
    for (final MaintainerDirectoryEntry entry : entries) {
      if (maintainerCursorKey(entry).equals(cursor.lastKey())) return false;
    }
    return true;
  }

  private static boolean parseMaintainerCursorBoundary(
      final String value) {
    final int separator = value.indexOf('|');
    if (separator <= 0 || separator != value.lastIndexOf('|')) return false;
    if (separator > 16_384) return false;
    final byte[] account = decodeLowerHex(value.substring(0, separator));
    if (account == null || !isCanonicalAccountCursorPayload(account)) return false;
    final String invitation = value.substring(separator + 1);
    if ("accepted".equals(invitation)) return true;
    if (!invitation.startsWith("pending-")) return false;
    final byte[] invitationBytes = decodeLowerHex(invitation.substring("pending-".length()));
    return invitationBytes != null && invitationBytes.length == 32 && !allZero(invitationBytes);
  }

  private static boolean isCanonicalAccountCursorPayload(final byte[] payload) {
    try {
      // AccountId payloads are chain-independent; the discriminant is needed only to render a
      // temporary canonical I105 literal for the existing encoder.
      final String account =
          TransferWirePayloadEncoder.decodeAccountIdPayload(
              payload, AccountAddress.DEFAULT_I105_DISCRIMINANT);
      return Arrays.equals(payload, TransferWirePayloadEncoder.encodeAccountIdPayload(account));
    } catch (final RuntimeException error) {
      return false;
    }
  }

  private static String lowerHex(final byte[] bytes) {
    final char[] alphabet = "0123456789abcdef".toCharArray();
    final char[] encoded = new char[bytes.length * 2];
    for (int index = 0; index < bytes.length; index++) {
      final int value = bytes[index] & 0xff;
      encoded[index * 2] = alphabet[value >>> 4];
      encoded[index * 2 + 1] = alphabet[value & 0x0f];
    }
    return new String(encoded);
  }

  private static byte[] decodeLowerHex(final String value) {
    if (value == null || value.isEmpty() || (value.length() & 1) != 0) return null;
    final byte[] decoded = new byte[value.length() / 2];
    for (int index = 0; index < decoded.length; index++) {
      final int high = lowerHexNibble(value.charAt(index * 2));
      final int low = lowerHexNibble(value.charAt(index * 2 + 1));
      if (high < 0 || low < 0) return null;
      decoded[index] = (byte) ((high << 4) | low);
    }
    return decoded;
  }

  private static int lowerHexNibble(final char value) {
    if (value >= '0' && value <= '9') return value - '0';
    if (value >= 'a' && value <= 'f') return value - 'a' + 10;
    return -1;
  }

  private static boolean semVerPageAdvances(
      final PageRequest request, final Version first) {
    final FinalizedCursor cursor = request.cursor();
    if (cursor == null) return true;
    final Version previous;
    try {
      previous = Version.parse(cursor.lastKey());
    } catch (final IllegalArgumentException error) {
      return false;
    }
    return first == null || previous.compareTo(first) < 0;
  }

  static String aliasHistoryCursorKey(final AliasHistoryEntry entry) {
    return entry.alias() + ":" + padU64Revision(entry.revision());
  }

  private static boolean aliasHistoryPageAdvances(
      final AliasQuery request, final AliasHistoryEntry first) {
    final FinalizedCursor cursor = request.page().cursor();
    if (cursor == null) return true;
    final int separator = cursor.lastKey().lastIndexOf(':');
    if (separator <= 0) return false;
    final String alias = cursor.lastKey().substring(0, separator);
    final String revisionText = cursor.lastKey().substring(separator + 1);
    if (!alias.equals(request.alias())
        || revisionText.length() != 20
        || !revisionText.matches("[0-9]{20}")) {
      return false;
    }
    final BigInteger revision;
    try {
      revision = new BigInteger(revisionText);
      requireU64(revision, "alias-history cursor revision");
    } catch (final IllegalArgumentException error) {
      return false;
    }
    if (!padU64Revision(revision).equals(revisionText)) return false;
    return first == null
        || first.alias().equals(alias) && first.revision().compareTo(revision) > 0;
  }

  private static String padU64Revision(final BigInteger revision) {
    final String text = revision.toString();
    final StringBuilder padded = new StringBuilder(20);
    for (int index = text.length(); index < 20; index++) padded.append('0');
    return padded.append(text).toString();
  }

  static String orderedSelectorCursorKey(final OrderedPackageEntry entry) {
    return entry.selector().namespace().value() + "/" + entry.selector().name().value();
  }

  private static boolean orderedPrefixPageAdvances(
      final OrderedPrefixQuery request, final OrderedPackageEntry first) {
    final FinalizedCursor cursor = request.page().cursor();
    if (cursor == null) return true;
    final PackageSelector previous = parseSelectorCursor(cursor.lastKey());
    final int separator = request.prefix().indexOf('/');
    if (previous == null
        || separator <= 0
        || separator != request.prefix().lastIndexOf('/')
        || !previous.namespace().value().equals(request.prefix().substring(0, separator))
        || !cursor.lastKey().startsWith(request.prefix())) {
      return false;
    }
    return first == null || compareSelectors(previous, first.selector()) < 0;
  }

  private static PackageSelector parseSelectorCursor(final String value) {
    final int separator = value.indexOf('/');
    if (separator <= 0 || separator != value.lastIndexOf('/')
        || separator == value.length() - 1) {
      return null;
    }
    try {
      final PackageSelector selector = new PackageSelector(
          new Namespace(value.substring(0, separator)),
          new PackageName(value.substring(separator + 1)));
      return orderedSelectorText(selector).equals(value) ? selector : null;
    } catch (final IllegalArgumentException error) {
      return null;
    }
  }

  private static String orderedSelectorText(final PackageSelector selector) {
    return selector.namespace().value() + "/" + selector.name().value();
  }

  private static int compareSelectors(
      final PackageSelector left, final PackageSelector right) {
    final int namespaceOrder = compareUtf8Strings(
        left.namespace().value(), right.namespace().value());
    return namespaceOrder != 0
        ? namespaceOrder : compareUtf8Strings(left.name().value(), right.name().value());
  }

  private static int compareAccountIds(final String left, final String right) {
    AccountIdLiteral.requireCanonicalI105Address(left, "maintainer account");
    AccountIdLiteral.requireCanonicalI105Address(right, "maintainer account");
    final AccountAddress leftAddress;
    final AccountAddress rightAddress;
    try {
      leftAddress = AccountAddress.parseEncodedIgnoringCurveSupport(left, null).address;
      rightAddress = AccountAddress.parseEncodedIgnoringCurveSupport(right, null).address;
    } catch (final AccountAddress.AccountAddressException error) {
      throw new IllegalArgumentException("maintainer account must be canonical I105", error);
    }

    try {
      final Optional<AccountAddress.SingleKeyPayload> leftSingle =
          leftAddress.singleKeyPayloadIgnoringCurveSupport();
      final Optional<AccountAddress.SingleKeyPayload> rightSingle =
          rightAddress.singleKeyPayloadIgnoringCurveSupport();
      if (leftSingle.isPresent() || rightSingle.isPresent()) {
        if (!leftSingle.isPresent()) return 1;
        if (!rightSingle.isPresent()) return -1;
        return compareUnsignedBytes(
            PublicKeyCodec.compactPublicKeyPayload(
                leftSingle.get().curveId(), leftSingle.get().publicKey()),
            PublicKeyCodec.compactPublicKeyPayload(
                rightSingle.get().curveId(), rightSingle.get().publicKey()));
      }

      final AccountAddress.MultisigPolicyPayload leftPolicy =
          leftAddress.multisigPolicyPayloadIgnoringCurveSupport().get();
      final AccountAddress.MultisigPolicyPayload rightPolicy =
          rightAddress.multisigPolicyPayloadIgnoringCurveSupport().get();
      int comparison = Integer.compare(leftPolicy.version(), rightPolicy.version());
      if (comparison != 0) return comparison;
      comparison = Integer.compare(leftPolicy.threshold(), rightPolicy.threshold());
      if (comparison != 0) return comparison;
      final int common = Math.min(leftPolicy.members().size(), rightPolicy.members().size());
      for (int index = 0; index < common; index++) {
        final AccountAddress.MultisigMemberPayload leftMember = leftPolicy.members().get(index);
        final AccountAddress.MultisigMemberPayload rightMember = rightPolicy.members().get(index);
        comparison = compareUnsignedBytes(
            PublicKeyCodec.compactPublicKeyPayload(
                leftMember.curveId(), leftMember.publicKey()),
            PublicKeyCodec.compactPublicKeyPayload(
                rightMember.curveId(), rightMember.publicKey()));
        if (comparison != 0) return comparison;
        comparison = Integer.compare(leftMember.weight(), rightMember.weight());
        if (comparison != 0) return comparison;
      }
      return Integer.compare(leftPolicy.members().size(), rightPolicy.members().size());
    } catch (final AccountAddress.AccountAddressException error) {
      throw new IllegalArgumentException("maintainer account must be canonical I105", error);
    }
  }

  static int compareAliasHistoryEntries(
      final AliasHistoryEntry left, final AliasHistoryEntry right) {
    final int aliasComparison = compareUtf8Strings(left.alias(), right.alias());
    return aliasComparison != 0
        ? aliasComparison : left.revision().compareTo(right.revision());
  }

  private static int effectivePageLimit(final long limit) {
    return limit == 0L ? 50 : (int) Math.min(limit, 100L);
  }

  private static void requirePageMatches(
      final PageRequest request,
      final RegistrySnapshot snapshot,
      final FinalizedCursor nextCursor,
      final int itemCount) {
    // Typed page carriers validate their echoed query identity before this shared page check.
    Objects.requireNonNull(request, "request");
    final FinalizedCursor cursor = request.cursor();
    if (itemCount > effectivePageLimit(request.limit())
        || (cursor != null
            && (!cursor.snapshot().equals(snapshot)
                || (nextCursor != null
                    && (!nextCursor.queryHash().equals(cursor.queryHash())
                        || !Objects.equals(nextCursor.caller(), cursor.caller())))))) {
      throw new IllegalArgumentException(
          "Musubi response does not use the requested page limit or cursor binding");
    }
  }

  private static void requireFinalizedPageMatches(
      final PageRequest request,
      final int itemCount,
      final String firstKey,
      final String lastKey,
      final RegistrySnapshot snapshot,
      final FinalizedCursor nextCursor) {
    requireFinalizedPageMatches(
        request, itemCount, firstKey, lastKey, snapshot, nextCursor, true);
  }

  private static void requireFinalizedPageMatches(
      final PageRequest request,
      final int itemCount,
      final String firstKey,
      final String lastKey,
      final RegistrySnapshot snapshot,
      final FinalizedCursor nextCursor,
      final boolean nextCursorRequiresFullPage) {
    final int effectiveLimit = effectivePageLimit(request.limit());
    if (itemCount > effectiveLimit
        || (itemCount == 0 && (firstKey != null || lastKey != null))
        || (itemCount > 0 && (firstKey == null || lastKey == null))) {
      throw new IllegalArgumentException(
          "Musubi response exceeds its requested bound or has invalid cursor keys");
    }
    final FinalizedCursor requestCursor = request.cursor();
    if (requestCursor != null
        && (!requestCursor.snapshot().equals(snapshot) || requestCursor.caller() != null)) {
      throw new IllegalArgumentException(
          "Musubi response does not continue its public finalized cursor");
    }
    if (nextCursor != null
        && (!nextCursor.snapshot().equals(snapshot)
            || nextCursor.caller() != null
            || (nextCursorRequiresFullPage && itemCount != effectiveLimit)
            || !nextCursor.lastKey().equals(lastKey)
            || (requestCursor != null
                && !requestCursor.queryHash().equals(nextCursor.queryHash())))) {
      throw new IllegalArgumentException(
          "Musubi next cursor does not bind its exact response page");
    }
  }

  private static int compareVersionRequirements(final VersionReq left, final VersionReq right) {
    int comparison = left.kind().compareTo(right.kind());
    if (comparison != 0) return comparison;
    switch (left.kind()) {
      case ANY:
        return 0;
      case CARET:
      case TILDE:
      case EXACT:
        return left.version().compareTo(right.version());
      case MAJOR_WILDCARD:
        return left.major().compareTo(right.major());
      case MINOR_WILDCARD:
        comparison = left.major().compareTo(right.major());
        return comparison != 0 ? comparison : left.minor().compareTo(right.minor());
      case COMPARATORS:
        final int common = Math.min(left.comparators().size(), right.comparators().size());
        for (int index = 0; index < common; index++) {
          comparison = left.comparators().get(index).compareTo(right.comparators().get(index));
          if (comparison != 0) return comparison;
        }
        return Integer.compare(left.comparators().size(), right.comparators().size());
      default:
        throw new IllegalStateException("unhandled Musubi version requirement");
    }
  }

  private static boolean namespaceMatchesScope(
      final PackageId packageId, final Namespace namespace) {
    final int separator = namespace.value().indexOf('.');
    final String domain = separator < 0 ? null : namespace.value().substring(0, separator);
    return packageId.scope().kind() == PackageScope.Kind.DATASPACE_ROOT
        ? domain == null
        : Objects.equals(packageId.scope().domain(), domain);
  }

  private static Set<String> normalizedSearchTerms(final String query) {
    final Set<String> terms = new TreeSet<>();
    for (final String component : query.split("\\s+")) {
      if (component.getBytes(StandardCharsets.UTF_8).length <= 64
          && component.matches("[A-Za-z0-9-]+")) {
        terms.add(component.toLowerCase(Locale.ROOT));
      }
      final StringBuilder word = new StringBuilder();
      for (int offset = 0; offset < component.length();) {
        final int codePoint = component.codePointAt(offset);
        offset += Character.charCount(codePoint);
        if (Character.isLetterOrDigit(codePoint)) {
          word.appendCodePoint(codePoint);
        } else if (word.length() != 0) {
          addSearchTerm(terms, word.toString());
          word.setLength(0);
        }
      }
      if (word.length() != 0) addSearchTerm(terms, word.toString());
      if (terms.size() > 16) {
        throw new IllegalArgumentException("Musubi search query exceeds 16 normalized terms");
      }
    }
    if (terms.isEmpty()) {
      throw new IllegalArgumentException("Musubi search query has no bounded normalized terms");
    }
    return terms;
  }

  private static void addSearchTerm(final Set<String> terms, final String word) {
    final String normalized = word.toLowerCase(Locale.ROOT);
    if (normalized.getBytes(StandardCharsets.UTF_8).length > 64) {
      throw new IllegalArgumentException("Musubi search term exceeds 64 UTF-8 bytes");
    }
    terms.add(normalized);
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
