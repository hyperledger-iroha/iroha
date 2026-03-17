// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import java.util.Arrays;

public final class AssetDefinitionIdEncoderTests {

  private AssetDefinitionIdEncoderTests() {}

  public static void main(final String[] args) {
    encodeProducesAidPrefix();
    encodeProduces32HexCharsAfterPrefix();
    encodeSetsUuidV4VersionBits();
    encodeSetsRfc4122VariantBits();
    encodeIsDeterministic();
    differentInputsProduceDifferentAids();
    isAidEncodedReturnsTrueForValidAid();
    isAidEncodedReturnsFalseForNoritoString();
    isAidEncodedReturnsFalseForLegacyFormat();
    isAidEncodedReturnsFalseForNull();
    parseAidBytesRoundtripsWithComputeAidBytes();
    parseAidBytesRejectsInvalidFormat();
    parseAidBytesRejectsBadVersionBits();
    parseAidBytesRejectsBadVariantBits();
    System.out.println("[IrohaAndroid] AssetDefinitionIdEncoder tests passed.");
  }

  private static void encodeProducesAidPrefix() {
    final String result = AssetDefinitionIdEncoder.encode("pkr", "sbp");
    assert result.startsWith("aid:") : "encoded result must start with aid: prefix";
  }

  private static void encodeProduces32HexCharsAfterPrefix() {
    final String result = AssetDefinitionIdEncoder.encode("rose", "wonderland");
    final String hex = result.substring("aid:".length());
    assert hex.length() == 32 : "aid: hex part must be exactly 32 characters (16 bytes)";
  }

  private static void encodeSetsUuidV4VersionBits() {
    final byte[] bytes = AssetDefinitionIdEncoder.computeAidBytes("rose", "wonderland");
    final int versionNibble = (bytes[6] & 0xF0) >>> 4;
    assert versionNibble == 4 : "byte 6 upper nibble must be 0x4 (UUIDv4 version)";
  }

  private static void encodeSetsRfc4122VariantBits() {
    final byte[] bytes = AssetDefinitionIdEncoder.computeAidBytes("rose", "wonderland");
    final int variantBits = (bytes[8] & 0xC0) >>> 6;
    assert variantBits == 2 : "byte 8 upper 2 bits must be 0b10 (RFC4122 variant)";
  }

  private static void encodeIsDeterministic() {
    final String first = AssetDefinitionIdEncoder.encode("pkr", "sbp");
    final String second = AssetDefinitionIdEncoder.encode("pkr", "sbp");
    assert first.equals(second) : "same name#domain must always produce the same aid";
  }

  private static void differentInputsProduceDifferentAids() {
    final String pkr = AssetDefinitionIdEncoder.encode("pkr", "sbp");
    final String usd = AssetDefinitionIdEncoder.encode("usd", "sbp");
    assert !pkr.equals(usd) : "different asset names must produce different aids";
  }

  private static void isAidEncodedReturnsTrueForValidAid() {
    final String encoded = AssetDefinitionIdEncoder.encode("rose", "wonderland");
    assert AssetDefinitionIdEncoder.isAidEncoded(encoded)
        : "isAidEncoded must return true for output of encode()";
  }

  private static void isAidEncodedReturnsFalseForNoritoString() {
    assert !AssetDefinitionIdEncoder.isAidEncoded("norito:4e5254")
        : "isAidEncoded must return false for norito: strings";
  }

  private static void isAidEncodedReturnsFalseForLegacyFormat() {
    assert !AssetDefinitionIdEncoder.isAidEncoded("pkr#sbp")
        : "isAidEncoded must return false for name#domain strings";
  }

  private static void isAidEncodedReturnsFalseForNull() {
    assert !AssetDefinitionIdEncoder.isAidEncoded(null)
        : "isAidEncoded must return false for null";
  }

  private static void parseAidBytesRoundtripsWithComputeAidBytes() {
    final String encoded = AssetDefinitionIdEncoder.encode("pkr", "sbp");
    final byte[] parsed = AssetDefinitionIdEncoder.parseAidBytes(encoded);
    final byte[] computed = AssetDefinitionIdEncoder.computeAidBytes("pkr", "sbp");
    assert Arrays.equals(computed, parsed)
        : "parseAidBytes must return the same bytes as computeAidBytes";
  }

  private static void parseAidBytesRejectsInvalidFormat() {
    boolean threw = false;
    try {
      AssetDefinitionIdEncoder.parseAidBytes("not-an-aid");
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "parseAidBytes must reject invalid format";
  }

  private static void parseAidBytesRejectsBadVersionBits() {
    // Build a valid-looking aid: string but with wrong UUIDv4 version nibble (byte 6)
    final byte[] bytes = AssetDefinitionIdEncoder.computeAidBytes("rose", "wonderland");
    bytes[6] = (byte) ((bytes[6] & 0x0F) | 0x30); // version 3 instead of 4
    final String bad = "aid:" + bytesToHex(bytes);
    boolean threw = false;
    try {
      AssetDefinitionIdEncoder.parseAidBytes(bad);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("UUIDv4 version");
    }
    assert threw : "parseAidBytes must reject aid: with wrong UUIDv4 version bits";
  }

  private static void parseAidBytesRejectsBadVariantBits() {
    // Build a valid-looking aid: string but with wrong RFC 4122 variant bits (byte 8)
    final byte[] bytes = AssetDefinitionIdEncoder.computeAidBytes("rose", "wonderland");
    bytes[8] = (byte) (bytes[8] & 0x3F); // clear variant bits (00 instead of 10)
    final String bad = "aid:" + bytesToHex(bytes);
    boolean threw = false;
    try {
      AssetDefinitionIdEncoder.parseAidBytes(bad);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("RFC 4122 variant");
    }
    assert threw : "parseAidBytes must reject aid: with wrong RFC 4122 variant bits";
  }

  private static String bytesToHex(final byte[] bytes) {
    final StringBuilder sb = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      sb.append(String.format(java.util.Locale.ROOT, "%02x", b & 0xFF));
    }
    return sb.toString();
  }
}
