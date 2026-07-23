package org.hyperledger.iroha.android.alias;

import java.math.BigInteger;
import java.util.LinkedHashMap;
import java.util.Map;

/** Canonical account-alias text paired with the expected numeric dataspace ID. */
public final class ResolvedAccountAliasV1 extends AliasJsonValue {
  private final AccountAliasName canonicalName;
  private final BigInteger dataspaceId;

  /** Constructs a resolved account alias. */
  public ResolvedAccountAliasV1(
      final AccountAliasName canonicalName, final BigInteger dataspaceId) {
    if (canonicalName == null) {
      throw new IllegalArgumentException("canonicalName must not be null");
    }
    this.canonicalName = canonicalName;
    this.dataspaceId = AliasNameSupport.requireU64(dataspaceId, "dataspaceId");
  }

  /** Convenience constructor for non-negative signed identifiers. */
  public ResolvedAccountAliasV1(final AccountAliasName canonicalName, final long dataspaceId) {
    this(canonicalName, BigInteger.valueOf(dataspaceId));
  }

  /** Parses and pins an external alias literal. */
  public ResolvedAccountAliasV1(final String canonicalName, final BigInteger dataspaceId) {
    this(AccountAliasName.parse(canonicalName), dataspaceId);
  }

  /** Returns the catalog-free canonical name. */
  public AccountAliasName canonicalName() {
    return canonicalName;
  }

  /** Returns the expected parent dataspace identifier. */
  public BigInteger dataspaceId() {
    return dataspaceId;
  }

  /** Returns the optional resolved domain parent. */
  public ResolvedDomainV1 parentDomain() {
    if (canonicalName.domain() == null) return null;
    return new ResolvedDomainV1(
        canonicalName.domain() + "." + canonicalName.dataspace(), dataspaceId);
  }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("canonical_name", canonicalName.toJsonMap());
    map.put("dataspace_id", dataspaceId);
    return map;
  }

  @Override
  public String toString() {
    return canonicalName.canonicalText();
  }
}

