// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Objects;

/** Exact ABI22 snapshot of Android build, patch, and verified-boot properties. */
final class OfflineAndroidAttestedDevicePropertiesV2 {
  public static final int VERSION = 2;
  public static final int MAX_PROPERTY_UTF8_BYTES = 128;
  public static final int MAX_VERIFIED_BOOT_KEY_BYTES = 1_024;
  static final long U32_MAX = 0xffff_ffffL;

  private final int version;
  private final long attestationVersion;
  private final long keymintVersion;
  private final OfflineAndroidDeviceSecurityLevelV2 securityLevel;
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

  public OfflineAndroidAttestedDevicePropertiesV2(
      final int version,
      final long attestationVersion,
      final long keymintVersion,
      final OfflineAndroidDeviceSecurityLevelV2 securityLevel,
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
    if (version != VERSION) {
      throw new IllegalArgumentException(
          "Android attested-device property version must be exactly 2");
    }
    this.version = version;
    this.attestationVersion = positiveU32(attestationVersion, "attestation_version");
    this.keymintVersion = positiveU32(keymintVersion, "keymint_version");
    this.securityLevel = Objects.requireNonNull(securityLevel, "security_level");
    this.brand = canonicalProperty(brand, "brand");
    this.device = canonicalProperty(device, "device");
    this.product = canonicalProperty(product, "product");
    this.manufacturer = canonicalProperty(manufacturer, "manufacturer");
    this.model = canonicalProperty(model, "model");
    this.osVersion = positiveU32(osVersion, "os_version");
    this.osPatchLevel = positiveU32(osPatchLevel, "os_patch_level");
    this.vendorPatchLevel = positiveU32(vendorPatchLevel, "vendor_patch_level");
    this.bootPatchLevel = positiveU32(bootPatchLevel, "boot_patch_level");
    this.verifiedBootKey = Objects.requireNonNull(verifiedBootKey, "verified_boot_key").clone();
    if (this.verifiedBootKey.length == 0
        || this.verifiedBootKey.length > MAX_VERIFIED_BOOT_KEY_BYTES) {
      throw new IllegalArgumentException(
          "verified_boot_key must contain 1.." + MAX_VERIFIED_BOOT_KEY_BYTES + " bytes");
    }
    this.verifiedBootHash = Objects.requireNonNull(verifiedBootHash, "verified_boot_hash").clone();
    if (this.verifiedBootHash.length != 32 || allZero(this.verifiedBootHash)) {
      throw new IllegalArgumentException(
          "verified_boot_hash must be one non-zero 32-byte value");
    }
  }

  private static long positiveU32(final long value, final String field) {
    if (value <= 0 || value > U32_MAX) {
      throw new IllegalArgumentException(field + " must be a positive u32");
    }
    return value;
  }

  private static String canonicalProperty(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.isEmpty()
        || !value.equals(value.trim())
        || value.getBytes(StandardCharsets.UTF_8).length > MAX_PROPERTY_UTF8_BYTES) {
      throw new IllegalArgumentException(
          field
              + " must be canonical non-empty printable ASCII within "
              + MAX_PROPERTY_UTF8_BYTES
              + " bytes");
    }
    for (int index = 0; index < value.length(); index++) {
      final char item = value.charAt(index);
      if (item < 0x20 || item > 0x7e) {
        throw new IllegalArgumentException(
            field
                + " must be canonical non-empty printable ASCII within "
                + MAX_PROPERTY_UTF8_BYTES
                + " bytes");
      }
    }
    return value;
  }

  private static boolean allZero(final byte[] value) {
    int aggregate = 0;
    for (final byte item : value) aggregate |= item;
    return aggregate == 0;
  }

  public int version() { return version; }

  public long attestationVersion() { return attestationVersion; }

  public long keymintVersion() { return keymintVersion; }

  public OfflineAndroidDeviceSecurityLevelV2 securityLevel() { return securityLevel; }

  public String brand() { return brand; }

  public String device() { return device; }

  public String product() { return product; }

  public String manufacturer() { return manufacturer; }

  public String model() { return model; }

  public long osVersion() { return osVersion; }

  public long osPatchLevel() { return osPatchLevel; }

  public long vendorPatchLevel() { return vendorPatchLevel; }

  public long bootPatchLevel() { return bootPatchLevel; }

  public byte[] verifiedBootKey() { return verifiedBootKey.clone(); }

  public byte[] verifiedBootHash() { return verifiedBootHash.clone(); }

  @Override
  public boolean equals(final Object object) {
    if (this == object) return true;
    if (!(object instanceof OfflineAndroidAttestedDevicePropertiesV2 other)) return false;
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
    int result = Objects.hash(
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
