// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Objects;

/** Exact bounded Java model of Rust {@code OfflineAndroidAttestedDevicePropertiesV2}. */
public final class OfflineAndroidAttestedDevicePropertiesV2 {
  public static final int VERSION_V2 = 2;
  public static final long OS_VERSION_MAX_V2 = 999_999L;
  public static final int ATTESTED_PROPERTY_MAX_BYTES_V2 = 128;
  public static final int VERIFIED_BOOT_KEY_MAX_BYTES_V2 = 1_024;
  public static final int VERIFIED_BOOT_HASH_BYTES_V2 = 32;
  private static final long U32_MAX = 0xffff_ffffL;

  /** Hardware security boundary authenticated by the Android KeyDescription. */
  public enum SecurityLevel {
    TRUSTED_ENVIRONMENT(0),
    STRONG_BOX(1);

    private final long noritoDiscriminant;

    SecurityLevel(final long noritoDiscriminant) {
      this.noritoDiscriminant = noritoDiscriminant;
    }

    long noritoDiscriminant() {
      return noritoDiscriminant;
    }

    static SecurityLevel fromNoritoDiscriminant(final long value) {
      for (final SecurityLevel candidate : values()) {
        if (candidate.noritoDiscriminant == value) {
          return candidate;
        }
      }
      throw new IllegalArgumentException(
          "unknown Android device security-level discriminant: " + value);
    }
  }

  private final int version;
  private final long attestationVersion;
  private final long keymintVersion;
  private final SecurityLevel securityLevel;
  private final String brand;
  private final String device;
  private final String product;
  private final String manufacturer;
  private final String model;
  private final long osVersion;
  private final long osPatchLevel;
  private final long vendorPatchLevel;
  private final long bootPatchLevel;
  private final byte[] verifiedBootKey;
  private final byte[] verifiedBootHash;

  /**
   * Construct one canonical property snapshot.
   *
   * <p>Empty identity strings and zero version/patch values remain representable because native
   * policy classifies otherwise authenticated but incomplete evidence as drain-only. Call {@link
   * #isCompleteV2()} before authorizing new offline activity.
   */
  public OfflineAndroidAttestedDevicePropertiesV2(
      final int version,
      final long attestationVersion,
      final long keymintVersion,
      final SecurityLevel securityLevel,
      final String brand,
      final String device,
      final String product,
      final String manufacturer,
      final String model,
      final long osVersion,
      final long osPatchLevel,
      final long vendorPatchLevel,
      final long bootPatchLevel,
      final byte[] verifiedBootKey,
      final byte[] verifiedBootHash) {
    if (version != VERSION_V2) {
      throw new IllegalArgumentException(
          "Android attested-device properties version must be exactly 2");
    }
    requireU32(attestationVersion, "attestation_version");
    requireU32(keymintVersion, "keymint_version");
    requireU32(osVersion, "os_version");
    requireU32(osPatchLevel, "os_patch_level");
    requireU32(vendorPatchLevel, "vendor_patch_level");
    requireU32(bootPatchLevel, "boot_patch_level");
    this.version = version;
    this.attestationVersion = attestationVersion;
    this.keymintVersion = keymintVersion;
    this.securityLevel = Objects.requireNonNull(securityLevel, "securityLevel");
    this.brand = requireBoundedProperty(brand, "brand");
    this.device = requireBoundedProperty(device, "device");
    this.product = requireBoundedProperty(product, "product");
    this.manufacturer = requireBoundedProperty(manufacturer, "manufacturer");
    this.model = requireBoundedProperty(model, "model");
    this.osVersion = osVersion;
    this.osPatchLevel = osPatchLevel;
    this.vendorPatchLevel = vendorPatchLevel;
    this.bootPatchLevel = bootPatchLevel;
    this.verifiedBootKey = Objects.requireNonNull(verifiedBootKey, "verifiedBootKey").clone();
    this.verifiedBootHash = Objects.requireNonNull(verifiedBootHash, "verifiedBootHash").clone();
    if (this.verifiedBootKey.length > VERIFIED_BOOT_KEY_MAX_BYTES_V2) {
      throw new IllegalArgumentException(
          "verified_boot_key is outside the V2 protocol bound");
    }
    if (this.verifiedBootHash.length != VERIFIED_BOOT_HASH_BYTES_V2) {
      throw new IllegalArgumentException(
          "verified_boot_hash must contain exactly 32 bytes");
    }
  }

  /** Whether every property required for testnet eligibility is present and canonical. */
  public boolean isCompleteV2() {
    return attestationVersion > 0
        && keymintVersion > 0
        && osVersion > 0
        && osVersion <= OS_VERSION_MAX_V2
        && canonicalPatchMonth(osPatchLevel)
        && canonicalPatchDate(vendorPatchLevel)
        && canonicalPatchDate(bootPatchLevel)
        && verifiedBootKey.length > 0
        && !allZero(verifiedBootHash)
        && !brand.isEmpty()
        && !device.isEmpty()
        && !product.isEmpty()
        && !manufacturer.isEmpty()
        && !model.isEmpty()
        && isAscii(brand)
        && isAscii(device)
        && isAscii(product)
        && isAscii(manufacturer)
        && isAscii(model);
  }

  private static void requireU32(final long value, final String field) {
    if (value < 0 || value > U32_MAX) {
      throw new IllegalArgumentException(field + " must fit in u32");
    }
  }

  private static String requireBoundedProperty(final String value, final String field) {
    Objects.requireNonNull(value, field);
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    if (bytes.length > ATTESTED_PROPERTY_MAX_BYTES_V2) {
      throw new IllegalArgumentException(field + " exceeds the V2 protocol bound");
    }
    for (int index = 0; index < value.length(); index++) {
      if (Character.isISOControl(value.charAt(index))) {
        throw new IllegalArgumentException(field + " must not contain control characters");
      }
    }
    if (hasBoundaryWhitespace(value)) {
      throw new IllegalArgumentException(field + " must not contain surrounding whitespace");
    }
    return value;
  }

  private static boolean hasBoundaryWhitespace(final String value) {
    if (value.isEmpty()) return false;
    final int first = value.codePointAt(0);
    final int last = value.codePointBefore(value.length());
    return isUnicodeWhitespace(first) || isUnicodeWhitespace(last);
  }

  private static boolean isUnicodeWhitespace(final int codePoint) {
    return Character.isWhitespace(codePoint) || Character.isSpaceChar(codePoint);
  }

  private static boolean canonicalPatchMonth(final long value) {
    if (value < 0 || value > U32_MAX) {
      return false;
    }
    final long year = value / 100;
    final long month = value % 100;
    return year >= 2_000 && year <= 9_999 && month >= 1 && month <= 12;
  }

  private static boolean canonicalPatchDate(final long value) {
    if (value < 0 || value > U32_MAX) {
      return false;
    }
    final long year = value / 10_000;
    final long month = (value / 100) % 100;
    final long day = value % 100;
    if (year < 2_000 || year > 9_999 || month < 1 || month > 12) {
      return false;
    }
    final boolean leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    final int maximumDay;
    if (month == 2) {
      maximumDay = leap ? 29 : 28;
    } else if (month == 4 || month == 6 || month == 9 || month == 11) {
      maximumDay = 30;
    } else {
      maximumDay = 31;
    }
    return day >= 1 && day <= maximumDay;
  }

  private static boolean allZero(final byte[] value) {
    int aggregate = 0;
    for (final byte item : value) {
      aggregate |= item;
    }
    return aggregate == 0;
  }

  private static boolean isAscii(final String value) {
    for (int index = 0; index < value.length(); index++) {
      if (value.charAt(index) > 0x7f) return false;
    }
    return true;
  }

  public int version() {
    return version;
  }

  public long attestationVersion() {
    return attestationVersion;
  }

  public long keymintVersion() {
    return keymintVersion;
  }

  public SecurityLevel securityLevel() {
    return securityLevel;
  }

  public String brand() {
    return brand;
  }

  public String device() {
    return device;
  }

  public String product() {
    return product;
  }

  public String manufacturer() {
    return manufacturer;
  }

  public String model() {
    return model;
  }

  public long osVersion() {
    return osVersion;
  }

  public long osPatchLevel() {
    return osPatchLevel;
  }

  public long vendorPatchLevel() {
    return vendorPatchLevel;
  }

  public long bootPatchLevel() {
    return bootPatchLevel;
  }

  public byte[] verifiedBootKey() {
    return verifiedBootKey.clone();
  }

  public byte[] verifiedBootHash() {
    return verifiedBootHash.clone();
  }

  @Override
  public boolean equals(final Object object) {
    if (this == object) {
      return true;
    }
    if (!(object instanceof OfflineAndroidAttestedDevicePropertiesV2 other)) {
      return false;
    }
    return version == other.version
        && attestationVersion == other.attestationVersion
        && keymintVersion == other.keymintVersion
        && osVersion == other.osVersion
        && osPatchLevel == other.osPatchLevel
        && vendorPatchLevel == other.vendorPatchLevel
        && bootPatchLevel == other.bootPatchLevel
        && securityLevel == other.securityLevel
        && brand.equals(other.brand)
        && device.equals(other.device)
        && product.equals(other.product)
        && manufacturer.equals(other.manufacturer)
        && model.equals(other.model)
        && Arrays.equals(verifiedBootKey, other.verifiedBootKey)
        && Arrays.equals(verifiedBootHash, other.verifiedBootHash);
  }

  @Override
  public int hashCode() {
    int result =
        Objects.hash(
            version,
            attestationVersion,
            keymintVersion,
            securityLevel,
            brand,
            device,
            product,
            manufacturer,
            model,
            osVersion,
            osPatchLevel,
            vendorPatchLevel,
            bootPatchLevel);
    result = 31 * result + Arrays.hashCode(verifiedBootKey);
    result = 31 * result + Arrays.hashCode(verifiedBootHash);
    return result;
  }
}
