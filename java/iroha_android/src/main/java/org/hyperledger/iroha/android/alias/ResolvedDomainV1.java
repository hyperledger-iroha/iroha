package org.hyperledger.iroha.android.alias;

import java.math.BigInteger;
import java.util.LinkedHashMap;
import java.util.Map;

/** Canonical `domain.dataspace` text paired with the expected numeric dataspace ID. */
public final class ResolvedDomainV1 extends AliasJsonValue {
  private final String canonicalName;
  private final BigInteger dataspaceId;

  /** Constructs a resolved domain with a full unsigned 64-bit identifier. */
  public ResolvedDomainV1(final String canonicalName, final BigInteger dataspaceId) {
    this.canonicalName = AliasNameSupport.qualifiedDomain(canonicalName);
    this.dataspaceId = AliasNameSupport.requireU64(dataspaceId, "dataspaceId");
  }

  /** Convenience constructor for non-negative signed identifiers. */
  public ResolvedDomainV1(final String canonicalName, final long dataspaceId) {
    this(canonicalName, BigInteger.valueOf(dataspaceId));
  }

  /** Returns the canonical fully-qualified domain. */
  public String canonicalName() {
    return canonicalName;
  }

  /** Returns the expected parent dataspace identifier. */
  public BigInteger dataspaceId() {
    return dataspaceId;
  }

  /** Returns the resolved parent dataspace. */
  public ResolvedDataSpaceV1 parentDataspace() {
    return new ResolvedDataSpaceV1(
        canonicalName.substring(canonicalName.indexOf('.') + 1), dataspaceId);
  }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("canonical_name", canonicalName);
    map.put("dataspace_id", dataspaceId);
    return map;
  }

  @Override
  public String toString() {
    return canonicalName;
  }
}
