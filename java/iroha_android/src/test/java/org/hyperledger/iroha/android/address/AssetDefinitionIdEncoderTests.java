// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import java.math.BigInteger;
import java.util.Arrays;
import org.hyperledger.iroha.android.crypto.Blake3;

public final class AssetDefinitionIdEncoderTests {
  private static final char[] BASE58_ALPHABET =
      "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".toCharArray();

  private AssetDefinitionIdEncoderTests() {}

  public static void main(final String[] args) {
    encodeProducesUnprefixedBase58();
    encodeSetsUuidV4VersionBits();
    encodeSetsRfc4122VariantBits();
    encodeIsDeterministic();
    differentInputsProduceDifferentAddresses();
    isCanonicalAddressReturnsTrueForValidAddress();
    isCanonicalAddressReturnsFalseForNoritoString();
    isCanonicalAddressReturnsFalseForLegacyAidFormat();
    isCanonicalAddressReturnsFalseForNull();
    parseAddressBytesRoundtripsWithComputeDefinitionBytes();
    parseAddressBytesRejectsInvalidFormat();
    parseAddressBytesRejectsInvalidChecksum();
    parseAddressBytesRejectsBadVersionBits();
    parseAddressBytesRejectsBadVariantBits();
    System.out.println("[IrohaAndroid] AssetDefinitionIdEncoder tests passed.");
  }

  private static void encodeProducesUnprefixedBase58() {
    final String result = AssetDefinitionIdEncoder.encode("pkr", "sbp");
    assert !result.contains(":") : "encoded result must not contain a legacy prefix";
    assert result.length() >= 20 : "encoded result should be a compact Base58 address";
  }

  private static void encodeSetsUuidV4VersionBits() {
    final byte[] bytes = AssetDefinitionIdEncoder.computeDefinitionBytes("rose", "wonderland");
    final int versionNibble = (bytes[6] & 0xF0) >>> 4;
    assert versionNibble == 4 : "byte 6 upper nibble must be 0x4 (UUIDv4 version)";
  }

  private static void encodeSetsRfc4122VariantBits() {
    final byte[] bytes = AssetDefinitionIdEncoder.computeDefinitionBytes("rose", "wonderland");
    final int variantBits = (bytes[8] & 0xC0) >>> 6;
    assert variantBits == 2 : "byte 8 upper 2 bits must be 0b10 (RFC4122 variant)";
  }

  private static void encodeIsDeterministic() {
    final String first = AssetDefinitionIdEncoder.encode("pkr", "sbp");
    final String second = AssetDefinitionIdEncoder.encode("pkr", "sbp");
    assert first.equals(second) : "same name#domain must always produce the same address";
  }

  private static void differentInputsProduceDifferentAddresses() {
    final String pkr = AssetDefinitionIdEncoder.encode("pkr", "sbp");
    final String usd = AssetDefinitionIdEncoder.encode("usd", "sbp");
    assert !pkr.equals(usd) : "different asset names must produce different addresses";
  }

  private static void isCanonicalAddressReturnsTrueForValidAddress() {
    final String encoded = AssetDefinitionIdEncoder.encode("rose", "wonderland");
    assert AssetDefinitionIdEncoder.isCanonicalAddress(encoded)
        : "isCanonicalAddress must return true for output of encode()";
  }

  private static void isCanonicalAddressReturnsFalseForNoritoString() {
    assert !AssetDefinitionIdEncoder.isCanonicalAddress("norito:4e5254")
        : "isCanonicalAddress must return false for norito: strings";
  }

  private static void isCanonicalAddressReturnsFalseForLegacyAidFormat() {
    assert !AssetDefinitionIdEncoder.isCanonicalAddress("aid:2f17c72466f84a4bb8a8e24884fdcd2f")
        : "isCanonicalAddress must return false for legacy aid: strings";
  }

  private static void isCanonicalAddressReturnsFalseForNull() {
    assert !AssetDefinitionIdEncoder.isCanonicalAddress(null)
        : "isCanonicalAddress must return false for null";
  }

  private static void parseAddressBytesRoundtripsWithComputeDefinitionBytes() {
    final String encoded = AssetDefinitionIdEncoder.encode("pkr", "sbp");
    final byte[] parsed = AssetDefinitionIdEncoder.parseAddressBytes(encoded);
    final byte[] computed = AssetDefinitionIdEncoder.computeDefinitionBytes("pkr", "sbp");
    assert Arrays.equals(computed, parsed)
        : "parseAddressBytes must return the same bytes as computeDefinitionBytes";
  }

  private static void parseAddressBytesRejectsInvalidFormat() {
    boolean threw = false;
    try {
      AssetDefinitionIdEncoder.parseAddressBytes("not-an-address");
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "parseAddressBytes must reject invalid format";
  }

  private static void parseAddressBytesRejectsInvalidChecksum() {
    final String encoded = AssetDefinitionIdEncoder.encode("rose", "wonderland");
    final char replacement = encoded.charAt(encoded.length() - 1) == '1' ? '2' : '1';
    final String bad = encoded.substring(0, encoded.length() - 1) + replacement;
    boolean threw = false;
    try {
      AssetDefinitionIdEncoder.parseAddressBytes(bad);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("checksum");
    }
    assert threw : "parseAddressBytes must reject addresses with invalid checksums";
  }

  private static void parseAddressBytesRejectsBadVersionBits() {
    final byte[] bytes = AssetDefinitionIdEncoder.computeDefinitionBytes("rose", "wonderland");
    bytes[6] = (byte) ((bytes[6] & 0x0F) | 0x30); // version 3 instead of 4
    final String bad = addressFromBytesWithChecksum(bytes);
    boolean threw = false;
    try {
      AssetDefinitionIdEncoder.parseAddressBytes(bad);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("UUIDv4 version");
    }
    assert threw : "parseAddressBytes must reject addresses with wrong UUIDv4 version bits";
  }

  private static void parseAddressBytesRejectsBadVariantBits() {
    final byte[] bytes = AssetDefinitionIdEncoder.computeDefinitionBytes("rose", "wonderland");
    bytes[8] = (byte) (bytes[8] & 0x3F); // clear variant bits (00 instead of 10)
    final String bad = addressFromBytesWithChecksum(bytes);
    boolean threw = false;
    try {
      AssetDefinitionIdEncoder.parseAddressBytes(bad);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("RFC 4122 variant");
    }
    assert threw : "parseAddressBytes must reject addresses with wrong RFC 4122 variant bits";
  }

  private static String addressFromBytesWithChecksum(final byte[] definitionBytes) {
    final byte[] payload = new byte[21];
    payload[0] = 1;
    System.arraycopy(definitionBytes, 0, payload, 1, 16);
    final byte[] checksum = Blake3.hash(Arrays.copyOf(payload, 17));
    System.arraycopy(checksum, 0, payload, 17, 4);
    return encodeBase58(payload);
  }

  private static String encodeBase58(final byte[] input) {
    if (input.length == 0) {
      return String.valueOf(BASE58_ALPHABET[0]);
    }
    BigInteger value = new BigInteger(1, input);
    final StringBuilder builder = new StringBuilder();
    while (value.compareTo(BigInteger.ZERO) > 0) {
      final BigInteger[] divRem = value.divideAndRemainder(BigInteger.valueOf(58));
      builder.append(BASE58_ALPHABET[divRem[1].intValue()]);
      value = divRem[0];
    }
    for (int index = 0; index < input.length && input[index] == 0; index++) {
      builder.append(BASE58_ALPHABET[0]);
    }
    if (builder.length() == 0) {
      builder.append(BASE58_ALPHABET[0]);
    }
    return builder.reverse().toString();
  }
}
