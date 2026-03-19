// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import java.nio.charset.StandardCharsets;
import java.util.Locale;
import java.util.Objects;
import org.hyperledger.iroha.android.crypto.Blake3;

/**
 * Computes canonical {@code aid:} format asset definition identifiers.
 *
 * <p>Iroha now represents {@code AssetDefinitionId} as a 16-byte hash derived from
 * the legacy {@code "name#domain"} format. The algorithm (verified against
 * {@code iroha_data_model/src/asset/id.rs}):
 *
 * <ol>
 *   <li>BLAKE3({@code "name#domain"}) → 32 bytes</li>
 *   <li>Take first 16 bytes</li>
 *   <li>Set UUIDv4 version bits: {@code bytes[6] = (bytes[6] & 0x0F) | 0x40}</li>
 *   <li>Set RFC 4122 variant bits: {@code bytes[8] = (bytes[8] & 0x3F) | 0x80}</li>
 *   <li>Result: {@code "aid:" + lowercase_hex(16_bytes)}</li>
 * </ol>
 */
public final class AssetDefinitionIdEncoder {

  private static final String AID_PREFIX = "aid:";
  private static final int AID_BYTES_LEN = 16;
  private static final int AID_HEX_LEN = AID_BYTES_LEN * 2;

  private AssetDefinitionIdEncoder() {}

  /**
   * Computes the canonical {@code aid:} identifier from asset name and domain.
   *
   * @param name   the asset name (e.g., "pkr")
   * @param domain the domain name (e.g., "sbp")
   * @return the canonical identifier (e.g., "aid:abcdef0123456789...")
   */
  public static String encode(String name, String domain) {
    Objects.requireNonNull(name, "name");
    Objects.requireNonNull(domain, "domain");
    byte[] aidBytes = computeAidBytes(name, domain);
    return AID_PREFIX + bytesToHex(aidBytes);
  }

  /**
   * Computes the raw 16-byte {@code aid} bytes from asset name and domain.
   *
   * @param name   the asset name
   * @param domain the domain name
   * @return 16-byte array with UUIDv4 version and RFC 4122 variant bits set
   */
  public static byte[] computeAidBytes(String name, String domain) {
    byte[] input = (name + "#" + domain).getBytes(StandardCharsets.UTF_8);
    byte[] hash = Blake3.hash(input);

    byte[] aidBytes = new byte[AID_BYTES_LEN];
    System.arraycopy(hash, 0, aidBytes, 0, AID_BYTES_LEN);

    // UUIDv4 version bits
    aidBytes[6] = (byte) ((aidBytes[6] & 0x0F) | 0x40);
    // RFC 4122 variant bits
    aidBytes[8] = (byte) ((aidBytes[8] & 0x3F) | 0x80);

    return aidBytes;
  }

  /**
   * Checks whether the given string is in {@code aid:} format.
   *
   * @param value the string to test
   * @return {@code true} if the string starts with {@code aid:} and has 32 hex characters
   */
  public static boolean isAidEncoded(String value) {
    if (value == null || !value.startsWith(AID_PREFIX)) {
      return false;
    }
    String hex = value.substring(AID_PREFIX.length());
    if (hex.length() != AID_HEX_LEN) {
      return false;
    }
    for (int i = 0; i < hex.length(); i++) {
      char c = hex.charAt(i);
      if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F'))) {
        return false;
      }
    }
    return true;
  }

  /**
   * Parses the 16 raw bytes from an {@code aid:<hex>} string.
   *
   * @param aidString the {@code aid:} prefixed string
   * @return 16-byte array
   * @throws IllegalArgumentException if the format is invalid
   */
  public static byte[] parseAidBytes(String aidString) {
    if (!isAidEncoded(aidString)) {
      throw new IllegalArgumentException("Invalid aid: format: " + aidString);
    }
    String hex = aidString.substring(AID_PREFIX.length());
    byte[] bytes = hexToBytes(hex);
    if ((bytes[6] & 0xF0) != 0x40) {
      throw new IllegalArgumentException(
          "aid: bytes lack UUIDv4 version nibble (byte 6): " + aidString);
    }
    if ((bytes[8] & 0xC0) != 0x80) {
      throw new IllegalArgumentException(
          "aid: bytes lack RFC 4122 variant bits (byte 8): " + aidString);
    }
    return bytes;
  }

  private static String bytesToHex(byte[] bytes) {
    StringBuilder sb = new StringBuilder(bytes.length * 2);
    for (byte b : bytes) {
      sb.append(String.format(Locale.ROOT, "%02x", b & 0xFF));
    }
    return sb.toString();
  }

  private static byte[] hexToBytes(String hex) {
    byte[] bytes = new byte[hex.length() / 2];
    for (int i = 0; i < bytes.length; i++) {
      int hi = Character.digit(hex.charAt(i * 2), 16);
      int lo = Character.digit(hex.charAt(i * 2 + 1), 16);
      bytes[i] = (byte) ((hi << 4) | lo);
    }
    return bytes;
  }
}
