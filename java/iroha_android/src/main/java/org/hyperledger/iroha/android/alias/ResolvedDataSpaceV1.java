package org.hyperledger.iroha.android.alias;

import java.math.BigInteger;
import java.util.LinkedHashMap;
import java.util.Map;

/** Canonical dataspace text paired with the numeric ID expected by the caller. */
public final class ResolvedDataSpaceV1 extends AliasJsonValue {
  private final String canonicalName;
  private final BigInteger dataspaceId;

  /** Constructs a resolved dataspace with a full unsigned 64-bit identifier. */
  public ResolvedDataSpaceV1(final String canonicalName, final BigInteger dataspaceId) {
    this.canonicalName = AliasNameSupport.segment(canonicalName, "canonicalName");
    this.dataspaceId = AliasNameSupport.requireU64(dataspaceId, "dataspaceId");
  }

  /** Convenience constructor for non-negative signed identifiers. */
  public ResolvedDataSpaceV1(final String canonicalName, final long dataspaceId) {
    this(canonicalName, BigInteger.valueOf(dataspaceId));
  }

  /** Returns the canonical textual dataspace. */
  public String canonicalName() {
    return canonicalName;
  }

  /** Returns the expected numeric dataspace identifier. */
  public BigInteger dataspaceId() {
    return dataspaceId;
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

