// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import java.math.BigInteger;
import java.util.Arrays;
import org.hyperledger.iroha.android.crypto.Blake3;
import org.hyperledger.iroha.android.testing.TestAssetDefinitionIds;

public final class AssetDefinitionIdEncoderTests {
  private static final char[] BASE58_ALPHABET =
      "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".toCharArray();

  private AssetDefinitionIdEncoderTests() {}

  public static void main(final String[] args) {
    encodeFromBytesProducesUnprefixedBase58();
    parseAddressBytesExposeUuidV4VersionBits();
    parseAddressBytesExposeRfc4122VariantBits();
    encodeFromBytesIsDeterministic();
    differentCanonicalAddressesRemainDistinct();
    isCanonicalAddressReturnsTrueForValidAddress();
    isCanonicalAddressReturnsFalseForPlainTextString();
    isCanonicalAddressReturnsFalseForMalformedColonString();
    isCanonicalAddressReturnsFalseForNull();
    parseAddressBytesRoundtripsWithEncodeFromBytes();
    parseAddressBytesRejectsInvalidFormat();
    parseAddressBytesRejectsInvalidChecksum();
    parseAddressBytesRejectsBadVersionBits();
    parseAddressBytesRejectsBadVariantBits();
    System.out.println("[IrohaAndroid] AssetDefinitionIdEncoder tests passed.");
  }

  private static void encodeFromBytesProducesUnprefixedBase58() {
    final String result =
        AssetDefinitionIdEncoder.encodeFromBytes(
            AssetDefinitionIdEncoder.parseAddressBytes(TestAssetDefinitionIds.TERTIARY));
    assert !result.contains(":") : "encoded result must not contain a legacy prefix";
    assert result.length() >= 20 : "encoded result should be a compact Base58 address";
  }

  private static void parseAddressBytesExposeUuidV4VersionBits() {
    final byte[] bytes = AssetDefinitionIdEncoder.parseAddressBytes(TestAssetDefinitionIds.PRIMARY);
    final int versionNibble = (bytes[6] & 0xF0) >>> 4;
    assert versionNibble == 4 : "byte 6 upper nibble must be 0x4 (UUIDv4 version)";
  }

  private static void parseAddressBytesExposeRfc4122VariantBits() {
    final byte[] bytes = AssetDefinitionIdEncoder.parseAddressBytes(TestAssetDefinitionIds.PRIMARY);
    final int variantBits = (bytes[8] & 0xC0) >>> 6;
    assert variantBits == 2 : "byte 8 upper 2 bits must be 0b10 (RFC4122 variant)";
  }

  private static void encodeFromBytesIsDeterministic() {
    final byte[] bytes = AssetDefinitionIdEncoder.parseAddressBytes(TestAssetDefinitionIds.TERTIARY);
    final String first = AssetDefinitionIdEncoder.encodeFromBytes(bytes);
    final String second = AssetDefinitionIdEncoder.encodeFromBytes(bytes);
    assert first.equals(second) : "same canonical UUIDv4 bytes must always produce the same address";
  }

  private static void differentCanonicalAddressesRemainDistinct() {
    assert !TestAssetDefinitionIds.TERTIARY.equals(TestAssetDefinitionIds.SECONDARY)
        : "different canonical addresses must remain distinct";
  }

  private static void isCanonicalAddressReturnsTrueForValidAddress() {
    final String encoded = TestAssetDefinitionIds.PRIMARY;
    assert AssetDefinitionIdEncoder.isCanonicalAddress(encoded)
        : "isCanonicalAddress must return true for a canonical fixture address";
  }

  private static void isCanonicalAddressReturnsFalseForPlainTextString() {
    assert !AssetDefinitionIdEncoder.isCanonicalAddress("not-an-address")
        : "isCanonicalAddress must return false for plain text strings";
  }

  private static void isCanonicalAddressReturnsFalseForMalformedColonString() {
    assert !AssetDefinitionIdEncoder.isCanonicalAddress("not:an-address")
        : "isCanonicalAddress must return false for malformed colon-delimited strings";
  }

  private static void isCanonicalAddressReturnsFalseForNull() {
    assert !AssetDefinitionIdEncoder.isCanonicalAddress(null)
        : "isCanonicalAddress must return false for null";
  }

  private static void parseAddressBytesRoundtripsWithEncodeFromBytes() {
    final String encoded = TestAssetDefinitionIds.TERTIARY;
    final byte[] parsed = AssetDefinitionIdEncoder.parseAddressBytes(encoded);
    final String reencoded = AssetDefinitionIdEncoder.encodeFromBytes(parsed);
    assert encoded.equals(reencoded)
        : "parseAddressBytes + encodeFromBytes must roundtrip a canonical address";
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
    final String encoded = TestAssetDefinitionIds.PRIMARY;
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
    final byte[] bytes = AssetDefinitionIdEncoder.parseAddressBytes(TestAssetDefinitionIds.PRIMARY);
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
    final byte[] bytes = AssetDefinitionIdEncoder.parseAddressBytes(TestAssetDefinitionIds.PRIMARY);
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
