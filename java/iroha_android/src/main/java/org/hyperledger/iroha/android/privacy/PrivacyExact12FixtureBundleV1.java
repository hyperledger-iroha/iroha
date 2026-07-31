package org.hyperledger.iroha.android.privacy;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/** Typed outer bundle containing exactly the twelve canonical privacy rows. */
public final class PrivacyExact12FixtureBundleV1 {
  private final int version;
  private final List<PrivacyExact12TypedFixtureRowV1> rows;

  public PrivacyExact12FixtureBundleV1(
      final int version, final List<PrivacyExact12TypedFixtureRowV1> rows) {
    if (version != PrivacyExact12FixtureCodecV1.VERSION) {
      throw new IllegalArgumentException(
          "exact-12 fixture version must be " + PrivacyExact12FixtureCodecV1.VERSION);
    }
    if (rows == null || rows.size() != PrivacyExact12FixtureCodecV1.ROW_COUNT) {
      throw new IllegalArgumentException(
          "exact-12 fixture must contain exactly "
              + PrivacyExact12FixtureCodecV1.ROW_COUNT
              + " rows");
    }
    final PrivacyNativeBridge.ProtocolIdV1[] expected =
        PrivacyNativeBridge.ProtocolIdV1.values();
    if (expected.length != PrivacyExact12FixtureCodecV1.ROW_COUNT) {
      throw new IllegalStateException("privacy protocol registry must contain exactly 12 entries");
    }
    final List<PrivacyExact12TypedFixtureRowV1> copy = new ArrayList<>(rows.size());
    long aggregate = 0L;
    for (int index = 0; index < rows.size(); index++) {
      final PrivacyExact12TypedFixtureRowV1 row = rows.get(index);
      if (row == null) {
        throw new IllegalArgumentException("exact-12 row " + index + " must be provided");
      }
      if (row.protocolId() != expected[index]) {
        throw new IllegalArgumentException(
            "exact-12 row " + index + " is out of canonical protocol order");
      }
      aggregate = Math.addExact(aggregate, row.nestedByteCount());
      if (aggregate > PrivacyExact12FixtureCodecV1.MAX_AGGREGATE_NESTED_BYTES) {
        throw new IllegalArgumentException(
            "exact-12 bundle exceeds the aggregate nested-byte limit");
      }
      copy.add(row);
    }
    this.version = version;
    this.rows = Collections.unmodifiableList(copy);
  }

  public int version() {
    return version;
  }

  public List<PrivacyExact12TypedFixtureRowV1> rows() {
    return rows;
  }

  @Override
  public boolean equals(final Object object) {
    return this == object
        || (object instanceof PrivacyExact12FixtureBundleV1 other
            && version == other.version
            && rows.equals(other.rows));
  }

  @Override
  public int hashCode() {
    return 31 * version + rows.hashCode();
  }
}
