package org.hyperledger.iroha.android.nexus;

import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;

final class NexusModelUtils {

  private NexusModelUtils() {}

  static byte[] copy(final byte[] value) {
    return value == null ? null : Arrays.copyOf(value, value.length);
  }

  static Map<String, String> copyMap(final Map<String, String> value) {
    if (value == null || value.isEmpty()) {
      return Collections.emptyMap();
    }
    return Collections.unmodifiableMap(new LinkedHashMap<>(value));
  }

  static Set<String> copySet(final Set<String> value) {
    if (value == null || value.isEmpty()) {
      return Collections.emptySet();
    }
    return Collections.unmodifiableSet(new java.util.LinkedHashSet<>(value));
  }

  static String requireNonBlank(final String value, final String name) {
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException(name + " must not be blank");
    }
    return value;
  }
}
