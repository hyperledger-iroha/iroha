package org.hyperledger.iroha.android.telemetry;

import java.lang.reflect.Field;
import java.util.Locale;
import java.util.Optional;
import java.util.Set;

/** Device-profile provider that reflects into {@code android.os.Build}. */
public final class AndroidDeviceProfileProvider implements DeviceProfileProvider {

  private static final AndroidDeviceProfileProvider INSTANCE = new AndroidDeviceProfileProvider();
  private static final Set<String> ENTERPRISE_MANUFACTURERS =
      Set.of("zebra", "honeywell", "panasonic", "sonim", "datalogic", "bluebird");

  private AndroidDeviceProfileProvider() {}

  public static DeviceProfileProvider create() {
    return INSTANCE;
  }

  @Override
  public Optional<DeviceProfile> snapshot() {
    try {
      final Class<?> buildClass = Class.forName("android.os.Build");
      final String fingerprint = readStaticField(buildClass, "FINGERPRINT");
      final String model = readStaticField(buildClass, "MODEL");
      final String manufacturer = readStaticField(buildClass, "MANUFACTURER");
      final String brand = readStaticField(buildClass, "BRAND");
      final String hardware = readStaticField(buildClass, "HARDWARE");
      final String bucket = classify(fingerprint, model, manufacturer, brand, hardware);
      return Optional.of(DeviceProfile.of(bucket));
    } catch (final ReflectiveOperationException | SecurityException ignored) {
      return Optional.empty();
    }
  }

  private static String classify(
      final String fingerprint,
      final String model,
      final String manufacturer,
      final String brand,
      final String hardware) {
    if (isEmulator(fingerprint, model, manufacturer, brand, hardware)) {
      return "emulator";
    }
    if (isEnterprise(manufacturer, brand, model)) {
      return "enterprise";
    }
    return "consumer";
  }

  private static boolean isEmulator(
      final String fingerprint,
      final String model,
      final String manufacturer,
      final String brand,
      final String hardware) {
    final String normalizedFingerprint = normalize(fingerprint);
    if (normalizedFingerprint.contains("generic")
        || normalizedFingerprint.contains("unknown")
        || normalizedFingerprint.contains("emulator")) {
      return true;
    }
    final String normalizedModel = normalize(model);
    if (normalizedModel.contains("sdk")
        || normalizedModel.contains("emulator")
        || normalizedModel.contains("gphone")) {
      return true;
    }
    final String normalizedBrand = normalize(brand);
    final String normalizedManufacturer = normalize(manufacturer);
    if (normalizedBrand.startsWith("generic")
        || normalizedManufacturer.startsWith("generic")
        || normalizedManufacturer.contains("android")) {
      return true;
    }
    final String normalizedHardware = normalize(hardware);
    return normalizedHardware.contains("goldfish")
        || normalizedHardware.contains("ranchu")
        || normalizedHardware.contains("emulator");
  }

  private static boolean isEnterprise(
      final String manufacturer, final String brand, final String model) {
    final String normalizedManufacturer = normalize(manufacturer);
    final String normalizedBrand = normalize(brand);
    if (ENTERPRISE_MANUFACTURERS.contains(normalizedManufacturer)
        || ENTERPRISE_MANUFACTURERS.contains(normalizedBrand)) {
      return true;
    }
    final String normalizedModel = normalize(model);
    return normalizedModel.startsWith("tc")
        || normalizedModel.startsWith("ec")
        || normalizedModel.startsWith("et");
  }

  private static String readStaticField(final Class<?> clazz, final String fieldName)
      throws ReflectiveOperationException {
    final Field field = clazz.getField(fieldName);
    final Object value = field.get(null);
    return value == null ? "" : value.toString();
  }

  private static String normalize(final String value) {
    if (value == null) {
      return "";
    }
    return value.trim().toLowerCase(Locale.ROOT);
  }
}
