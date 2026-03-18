package org.hyperledger.iroha.android.telemetry;

import java.util.Locale;
import java.util.Objects;

/** Immutable descriptor for {@code android.telemetry.device_profile}. */
public final class DeviceProfile {
  private final String bucket;

  private DeviceProfile(final String bucket) {
    this.bucket = bucket;
  }

  public static DeviceProfile of(final String bucket) {
    final String normalized = normalize(bucket);
    if (normalized.isEmpty()) {
      throw new IllegalArgumentException("Device profile bucket must be non-empty");
    }
    return new DeviceProfile(normalized);
  }

  /** Returns the canonical bucket name. */
  public String bucket() {
    return bucket;
  }

  private static String normalize(final String value) {
    if (value == null) {
      return "";
    }
    return value.trim().toLowerCase(Locale.ROOT);
  }

  @Override
  public String toString() {
    return "DeviceProfile{" + "bucket='" + bucket + '\'' + '}';
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof DeviceProfile)) {
      return false;
    }
    final DeviceProfile other = (DeviceProfile) obj;
    return Objects.equals(bucket, other.bucket);
  }

  @Override
  public int hashCode() {
    return bucket.hashCode();
  }
}
