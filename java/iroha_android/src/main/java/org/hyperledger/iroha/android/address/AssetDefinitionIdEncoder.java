// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Objects;
import org.hyperledger.iroha.android.crypto.Blake3;

/**
 * Computes and validates canonical unprefixed Base58 asset-definition addresses.
 *
 * <p>Iroha represents {@code AssetDefinitionId} as canonical UUIDv4 bytes wrapped in a versioned
 * Base58 address with a BLAKE3 checksum:
 *
 * <ol>
 *   <li>BLAKE3({@code "name#domain"}) → 32 bytes</li>
 *   <li>Take the first 16 bytes</li>
 *   <li>Set UUIDv4 version bits: {@code bytes[6] = (bytes[6] & 0x0F) | 0x40}</li>
 *   <li>Set RFC 4122 variant bits: {@code bytes[8] = (bytes[8] & 0x3F) | 0x80}</li>
 *   <li>Wrap as {@code version_byte + 16 uuid bytes + 4-byte BLAKE3 checksum}</li>
 *   <li>Encode the 21-byte payload as unprefixed Base58</li>
 * </ol>
 */
public final class AssetDefinitionIdEncoder {

  private static final int ADDRESS_VERSION = 1;
  private static final int UUID_BYTES_LEN = 16;
  private static final int CHECKSUM_LEN = 4;
  private static final int ADDRESS_PAYLOAD_LEN = 1 + UUID_BYTES_LEN + CHECKSUM_LEN;
  private static final char[] BASE58_ALPHABET =
      "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".toCharArray();
  private static final int[] BASE58_INDEX = new int[128];
  private static final BigInteger BASE_58 = BigInteger.valueOf(58);

  static {
    Arrays.fill(BASE58_INDEX, -1);
    for (int i = 0; i < BASE58_ALPHABET.length; i++) {
      BASE58_INDEX[BASE58_ALPHABET[i]] = i;
    }
  }

  private AssetDefinitionIdEncoder() {}

  /**
   * Computes the canonical asset-definition address from asset name and domain.
   *
   * @param name the asset name (e.g., "pkr")
   * @param domain the domain name (e.g., "sbp")
   * @return the canonical unprefixed Base58 address
   */
  public static String encode(String name, String domain) {
    Objects.requireNonNull(name, "name");
    Objects.requireNonNull(domain, "domain");
    return encodeFromBytes(computeDefinitionBytes(name, domain));
  }

  /**
   * Wraps canonical UUIDv4 bytes as a canonical unprefixed Base58 address.
   *
   * @param definitionBytes 16-byte UUIDv4 payload
   * @return the canonical unprefixed Base58 address
   */
  public static String encodeFromBytes(byte[] definitionBytes) {
    Objects.requireNonNull(definitionBytes, "definitionBytes");
    if (definitionBytes.length != UUID_BYTES_LEN) {
      throw new IllegalArgumentException(
          "Asset definition bytes must be exactly 16 bytes: " + definitionBytes.length);
    }
    if ((definitionBytes[6] & 0xF0) != 0x40) {
      throw new IllegalArgumentException("Asset definition bytes must encode UUIDv4 version bits");
    }
    if ((definitionBytes[8] & 0xC0) != 0x80) {
      throw new IllegalArgumentException(
          "Asset definition bytes must encode RFC 4122 variant bits");
    }
    return encodeBase58(addressPayload(definitionBytes));
  }

  /**
   * Computes the raw 16-byte UUIDv4 payload from asset name and domain.
   *
   * @param name the asset name
   * @param domain the domain name
   * @return the 16 canonical UUIDv4 bytes
   */
  public static byte[] computeDefinitionBytes(String name, String domain) {
    Objects.requireNonNull(name, "name");
    Objects.requireNonNull(domain, "domain");
    byte[] input = (name + "#" + domain).getBytes(StandardCharsets.UTF_8);
    byte[] hash = Blake3.hash(input);

    byte[] definitionBytes = new byte[UUID_BYTES_LEN];
    System.arraycopy(hash, 0, definitionBytes, 0, UUID_BYTES_LEN);
    definitionBytes[6] = (byte) ((definitionBytes[6] & 0x0F) | 0x40);
    definitionBytes[8] = (byte) ((definitionBytes[8] & 0x3F) | 0x80);
    return definitionBytes;
  }

  /**
   * Checks whether the given value is a canonical unprefixed Base58 asset-definition address.
   *
   * @param value the string to test
   * @return {@code true} when the string parses and roundtrips as a canonical address
   */
  public static boolean isCanonicalAddress(String value) {
    if (value == null) {
      return false;
    }
    final String trimmed = value.trim();
    if (!trimmed.equals(value) || trimmed.isEmpty()) {
      return false;
    }
    try {
      return encodeFromBytes(parseAddressBytes(trimmed)).equals(trimmed);
    } catch (IllegalArgumentException ex) {
      return false;
    }
  }

  /**
   * Parses canonical UUIDv4 bytes from an unprefixed Base58 asset-definition address.
   *
   * @param address the canonical unprefixed Base58 address
   * @return the 16 canonical UUIDv4 bytes
   * @throws IllegalArgumentException if the address is malformed, versioned incorrectly, or fails
   *     checksum validation
   */
  public static byte[] parseAddressBytes(String address) {
    Objects.requireNonNull(address, "address");
    final String trimmed = address.trim();
    if (!trimmed.equals(address) || trimmed.isEmpty()) {
      throw new IllegalArgumentException(
          "Asset definition id must use canonical unprefixed Base58 form");
    }
    final byte[] payload = decodeBase58(trimmed);
    if (payload.length != ADDRESS_PAYLOAD_LEN) {
      throw new IllegalArgumentException(
          "Asset definition id must decode to exactly 21 bytes");
    }
    if ((payload[0] & 0xFF) != ADDRESS_VERSION) {
      throw new IllegalArgumentException("Asset definition id version is not supported");
    }

    final byte[] definitionBytes = Arrays.copyOfRange(payload, 1, 1 + UUID_BYTES_LEN);
    final byte[] expectedChecksum =
        checksum(Arrays.copyOfRange(payload, 0, 1 + UUID_BYTES_LEN));
    final byte[] actualChecksum =
        Arrays.copyOfRange(payload, 1 + UUID_BYTES_LEN, ADDRESS_PAYLOAD_LEN);
    if (!Arrays.equals(expectedChecksum, actualChecksum)) {
      throw new IllegalArgumentException("Asset definition id checksum is invalid");
    }
    if ((definitionBytes[6] & 0xF0) != 0x40) {
      throw new IllegalArgumentException(
          "Asset definition bytes lack UUIDv4 version nibble (byte 6): " + address);
    }
    if ((definitionBytes[8] & 0xC0) != 0x80) {
      throw new IllegalArgumentException(
          "Asset definition bytes lack RFC 4122 variant bits (byte 8): " + address);
    }
    return definitionBytes;
  }

  private static byte[] addressPayload(byte[] definitionBytes) {
    final byte[] payload = new byte[ADDRESS_PAYLOAD_LEN];
    payload[0] = (byte) ADDRESS_VERSION;
    System.arraycopy(definitionBytes, 0, payload, 1, UUID_BYTES_LEN);
    final byte[] checksum = checksum(Arrays.copyOfRange(payload, 0, 1 + UUID_BYTES_LEN));
    System.arraycopy(checksum, 0, payload, 1 + UUID_BYTES_LEN, CHECKSUM_LEN);
    return payload;
  }

  private static byte[] checksum(byte[] payload) {
    final byte[] digest = Blake3.hash(payload);
    return Arrays.copyOf(digest, CHECKSUM_LEN);
  }

  private static String encodeBase58(final byte[] input) {
    if (input.length == 0) {
      return String.valueOf(BASE58_ALPHABET[0]);
    }
    BigInteger value = new BigInteger(1, input);
    final StringBuilder builder = new StringBuilder();
    while (value.compareTo(BigInteger.ZERO) > 0) {
      final BigInteger[] divRem = value.divideAndRemainder(BASE_58);
      builder.append(BASE58_ALPHABET[divRem[1].intValue()]);
      value = divRem[0];
    }
    final char zeroSymbol = BASE58_ALPHABET[0];
    for (int i = 0; i < input.length && input[i] == 0; i++) {
      builder.append(zeroSymbol);
    }
    if (builder.length() == 0) {
      builder.append(zeroSymbol);
    }
    return builder.reverse().toString();
  }

  private static byte[] decodeBase58(final String encoded) {
    if (encoded == null || encoded.isEmpty()) {
      throw new IllegalArgumentException(
          "Asset definition id must use canonical unprefixed Base58 form");
    }
    BigInteger value = BigInteger.ZERO;
    for (int index = 0; index < encoded.length(); index++) {
      final char symbol = encoded.charAt(index);
      if (symbol >= BASE58_INDEX.length || BASE58_INDEX[symbol] < 0) {
        throw new IllegalArgumentException("Asset definition id must be valid Base58");
      }
      value = value.multiply(BASE_58).add(BigInteger.valueOf(BASE58_INDEX[symbol]));
    }
    byte[] decoded = value.toByteArray();
    if (decoded.length > 0 && decoded[0] == 0) {
      decoded = Arrays.copyOfRange(decoded, 1, decoded.length);
    }
    final char zeroChar = BASE58_ALPHABET[0];
    int leadingZeros = 0;
    while (leadingZeros < encoded.length() && encoded.charAt(leadingZeros) == zeroChar) {
      leadingZeros++;
    }
    final byte[] result = new byte[leadingZeros + decoded.length];
    System.arraycopy(decoded, 0, result, leadingZeros, decoded.length);
    return result;
  }
}
