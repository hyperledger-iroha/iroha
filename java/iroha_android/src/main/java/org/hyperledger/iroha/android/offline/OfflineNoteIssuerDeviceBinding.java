package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** Device-binding material required by the Torii Offline Note issuer endpoints. */
public final class OfflineNoteIssuerDeviceBinding {
  private static final List<String> RETIRED_ASSERTION_PUBLIC_KEY_ALIAS_FIELDS =
      Arrays.asList("device_public_key", "app_attest_public_key_base64");

  private final String deviceId;
  private final String offlinePublicKey;
  private final Map<String, Object> deviceBinding;

  public OfflineNoteIssuerDeviceBinding(
      final String deviceId,
      final String offlinePublicKey,
      final Map<String, Object> deviceBinding) {
    this.deviceId = requireExactNonEmptyText(deviceId, "deviceId");
    this.offlinePublicKey = requireExactNonEmptyText(offlinePublicKey, "offlinePublicKey");
    rejectRetiredDeviceBindingAliases(Objects.requireNonNull(deviceBinding, "deviceBinding"));
    this.deviceBinding = deepCopyObject(deviceBinding);
    final Object bindingDeviceId = this.deviceBinding.get("device_id");
    if (bindingDeviceId instanceof String value && !value.equals(this.deviceId)) {
      throw new IllegalArgumentException("device_binding.device_id must match deviceId");
    }
    final Object bindingPublicKey = this.deviceBinding.get("offline_public_key");
    if (bindingPublicKey instanceof String value && !value.equals(this.offlinePublicKey)) {
      throw new IllegalArgumentException(
          "device_binding.offline_public_key must match offlinePublicKey");
    }
  }

  public String deviceId() {
    return deviceId;
  }

  public String offlinePublicKey() {
    return offlinePublicKey;
  }

  public String attestationKeyId() {
    final Object keyId = deviceBinding.get("attestation_key_id");
    if (keyId instanceof String value) {
      if (value.isEmpty()) {
        throw new IllegalStateException("device_binding.attestation_key_id is required");
      }
      if (!isExactNonEmptyText(value)) {
        throw new IllegalStateException(
            "device_binding.attestation_key_id must be exact non-empty text");
      }
      return value;
    }
    throw new IllegalStateException("device_binding.attestation_key_id is required");
  }

  public Map<String, Object> deviceBinding() {
    return deepCopyObject(deviceBinding);
  }

  private static void rejectRetiredDeviceBindingAliases(final Map<String, Object> deviceBinding) {
    for (final String retiredKey : RETIRED_ASSERTION_PUBLIC_KEY_ALIAS_FIELDS) {
      if (deviceBinding.containsKey(retiredKey)) {
        throw new IllegalArgumentException(
            "device_binding." + retiredKey + " is retired; use assertion_public_key");
      }
    }
  }

  static Map<String, Object> deepCopyObject(final Map<String, Object> source) {
    final Map<String, Object> copy = new LinkedHashMap<>();
    for (final Map.Entry<String, Object> entry : source.entrySet()) {
      copy.put(entry.getKey(), normalizeJsonValue(entry.getValue()));
    }
    return copy;
  }

  @SuppressWarnings("unchecked")
  private static Object normalizeJsonValue(final Object value) {
    if (value == null
        || value instanceof String
        || value instanceof Number
        || value instanceof Boolean) {
      return value;
    }
    if (value instanceof Map<?, ?> map) {
      final Map<String, Object> object = new LinkedHashMap<>();
      for (final Map.Entry<?, ?> entry : map.entrySet()) {
        if (!(entry.getKey() instanceof String key)) {
          throw new IllegalStateException("JSON object keys must be strings");
        }
        object.put(key, normalizeJsonValue(entry.getValue()));
      }
      return object;
    }
    if (value instanceof java.util.List<?> list) {
      final java.util.List<Object> copy = new java.util.ArrayList<>(list.size());
      for (final Object item : list) {
        copy.add(normalizeJsonValue(item));
      }
      return copy;
    }
    throw new IllegalStateException("Unsupported JSON value: " + value.getClass());
  }

  private static String requireExactNonEmptyText(final String value, final String field) {
    if (value == null || !isExactNonEmptyText(value)) {
      throw new IllegalArgumentException(field + " must be exact non-empty text");
    }
    return value;
  }

  private static boolean isExactNonEmptyText(final String value) {
    return !value.isEmpty() && value.equals(value.trim());
  }
}
