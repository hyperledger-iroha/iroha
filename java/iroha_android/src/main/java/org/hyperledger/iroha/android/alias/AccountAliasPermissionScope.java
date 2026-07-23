package org.hyperledger.iroha.android.alias;

import java.math.BigInteger;
import java.util.LinkedHashMap;
import java.util.Map;

/** Exact scope carried by account-alias manage, delegate, and resolve permissions. */
public abstract class AccountAliasPermissionScope extends AliasJsonValue {
  private AccountAliasPermissionScope() {}

  /** Returns a scope for one canonical domain. */
  public static AccountAliasPermissionScope domain(final String domain) {
    final ResolvedDomainV1 canonical = new ResolvedDomainV1(domain, BigInteger.ZERO);
    return new DomainScope(canonical.canonicalName());
  }

  /** Returns a scope for one numeric dataspace. */
  public static AccountAliasPermissionScope dataspace(final BigInteger dataspaceId) {
    return new DataspaceScope(AliasNameSupport.requireU64(dataspaceId, "dataspaceId"));
  }

  /** Returns a scope for one exact resolved account alias. */
  public static AccountAliasPermissionScope alias(final ResolvedAccountAliasV1 alias) {
    if (alias == null) throw new IllegalArgumentException("alias must not be null");
    return new AliasScope(alias);
  }

  private static Map<String, Object> scope(final String kind, final Object value) {
    final Map<String, Object> result = new LinkedHashMap<>();
    result.put("scope", kind);
    result.put("value", value);
    return result;
  }

  private static final class DomainScope extends AccountAliasPermissionScope {
    private final String domain;

    private DomainScope(final String domain) { this.domain = domain; }

    @Override
    public Map<String, Object> toJsonMap() { return scope("domain", domain); }
  }

  private static final class DataspaceScope extends AccountAliasPermissionScope {
    private final BigInteger dataspaceId;

    private DataspaceScope(final BigInteger dataspaceId) { this.dataspaceId = dataspaceId; }

    @Override
    public Map<String, Object> toJsonMap() { return scope("dataspace", dataspaceId); }
  }

  private static final class AliasScope extends AccountAliasPermissionScope {
    private final ResolvedAccountAliasV1 alias;

    private AliasScope(final ResolvedAccountAliasV1 alias) { this.alias = alias; }

    @Override
    public Map<String, Object> toJsonMap() { return scope("alias", alias.toJsonMap()); }
  }
}
