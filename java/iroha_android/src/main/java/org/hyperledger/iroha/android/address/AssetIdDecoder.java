// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import java.util.Locale;
import java.util.Objects;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoHeader;

/**
 * Decodes norito-encoded asset identifiers to extract the asset definition ID.
 *
 * <p>Iroha returns asset IDs in {@code norito:<hex>} format. The binary payload encodes:
 *
 * <pre>
 *   struct AssetId {
 *       account: AccountId,
 *       definition: AssetDefinitionId,  // raw [u8; 16] aid_bytes
 *   }
 * </pre>
 *
 * <p>{@code AssetDefinitionId} is a one-way blake3 hash — name and domain cannot be recovered.
 * Use the app's cached asset definitions for display name lookup.
 */
public final class AssetIdDecoder {

  private static final String NORITO_PREFIX = "norito:";

  private AssetIdDecoder() {}

  /**
   * Decoded asset definition carrying the {@code aid:<hex>} identifier.
   *
   * <p>Since {@code AssetDefinitionId} is now a one-way blake3 hash,
   * name and domain cannot be recovered from the binary representation.
   * Use the app's cached asset definitions for display info lookup.
   */
  public static final class AssetDefinition {
    private final String aidHex;

    AssetDefinition(String aidHex) {
      this.aidHex = Objects.requireNonNull(aidHex);
    }

    /** Returns the {@code aid:<32-hex>} representation. */
    public String aidHex() {
      return aidHex;
    }
  }

  /**
   * Checks whether the given string is a norito-encoded identifier.
   *
   * @param value the string to test
   * @return {@code true} if the string starts with {@code norito:}
   */
  public static boolean isNoritoEncoded(String value) {
    return value != null && value.regionMatches(true, 0, NORITO_PREFIX, 0, NORITO_PREFIX.length());
  }

  /**
   * Decodes a {@code norito:<hex>} asset identifier (full AssetId with account + definition).
   *
   * @param noritoAssetId the full {@code norito:<hex>} string
   * @return decoded asset definition with name and domain
   */
  public static AssetDefinition decode(String noritoAssetId) {
    Objects.requireNonNull(noritoAssetId, "noritoAssetId");
    byte[] data = extractNoritoBytes(noritoAssetId);
    return decodeAssetIdBytes(data);
  }

  /**
   * Decodes a {@code norito:<hex>} asset definition identifier (AssetDefinitionId only).
   *
   * @param noritoDefinitionId the full {@code norito:<hex>} string
   * @return decoded asset definition with name and domain
   */
  public static AssetDefinition decodeDefinition(String noritoDefinitionId) {
    Objects.requireNonNull(noritoDefinitionId, "noritoDefinitionId");
    byte[] data = extractNoritoBytes(noritoDefinitionId);
    return decodeDefinitionIdBytes(data);
  }

  private static final int AID_BYTES_LEN = 16;

  /**
   * Decodes raw norito bytes for a full AssetId (account + definition).
   * Skips the account field and extracts the definition's 16-byte aid.
   */
  private static AssetDefinition decodeAssetIdBytes(byte[] data) {
    NoritoHeader.DecodeResult headerResult = NoritoHeader.decode(data, null);
    NoritoHeader header = headerResult.header();
    byte[] payload = headerResult.payload();
    header.validateChecksum(payload);

    int flags = header.flags();
    int flagsHint = header.minor();
    boolean compactLen = (flags & NoritoHeader.COMPACT_LEN) != 0;
    NoritoDecoder decoder = new NoritoDecoder(payload, flags, flagsHint);

    // AssetId struct: { account: AccountId, definition: AssetDefinitionId, scope: ... }
    // Skip account field
    long accountLen = decoder.readLength(compactLen);
    if (accountLen > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Account field too large");
    }
    decoder.readBytes((int) accountLen);

    // Read definition field — [u8; 16] with per-element u64 length prefix
    long definitionLen = decoder.readLength(compactLen);
    if (definitionLen > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Definition field too large");
    }
    byte[] definitionPayload = decoder.readBytes((int) definitionLen);
    byte[] aidBytes = decodeFixedByteArray(definitionPayload, AID_BYTES_LEN, flags, flagsHint);

    return new AssetDefinition(aidBytesToHex(aidBytes));
  }

  /**
   * Decodes raw norito bytes for an AssetDefinitionId (no account, just the [u8; 16] aid).
   */
  private static AssetDefinition decodeDefinitionIdBytes(byte[] data) {
    NoritoHeader.DecodeResult headerResult = NoritoHeader.decode(data, null);
    NoritoHeader header = headerResult.header();
    byte[] payload = headerResult.payload();
    header.validateChecksum(payload);

    byte[] aidBytes = decodeFixedByteArray(payload, AID_BYTES_LEN, header.flags(), header.minor());
    return new AssetDefinition(aidBytesToHex(aidBytes));
  }

  /**
   * Decodes a Norito fixed-size byte array ({@code [u8; N]}) where each element is u64-prefixed.
   */
  private static byte[] decodeFixedByteArray(
      byte[] payload, int expectedLen, int flags, int flagsHint) {
    // Fast path: raw N bytes without per-element length headers (Rust core.rs:2071)
    if (payload.length == expectedLen) {
      return payload.clone();
    }
    // Slow path: per-element length-prefixed bytes
    NoritoDecoder d = new NoritoDecoder(payload, flags, flagsHint);
    boolean compactLen = (flags & NoritoHeader.COMPACT_LEN) != 0;
    byte[] result = new byte[expectedLen];
    for (int i = 0; i < expectedLen; i++) {
      long elemLen = d.readLength(compactLen);
      if (elemLen != 1) {
        throw new IllegalArgumentException("Expected 1-byte element, got " + elemLen);
      }
      result[i] = (byte) d.readByte();
    }
    if (d.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after fixed byte array");
    }
    return result;
  }

  private static String aidBytesToHex(byte[] aidBytes) {
    StringBuilder sb = new StringBuilder("aid:");
    for (byte b : aidBytes) {
      sb.append(String.format(Locale.ROOT, "%02x", b & 0xFF));
    }
    return sb.toString();
  }

  private static byte[] extractNoritoBytes(String noritoString) {
    if (!noritoString.regionMatches(true, 0, NORITO_PREFIX, 0, NORITO_PREFIX.length())) {
      throw new IllegalArgumentException("Value must start with norito: prefix");
    }
    String hex = noritoString.substring(NORITO_PREFIX.length()).toLowerCase(Locale.ROOT);
    return hexToBytes(hex);
  }

  private static byte[] hexToBytes(String hex) {
    if (hex.length() % 2 != 0) {
      throw new IllegalArgumentException("Hex string must have even length");
    }
    byte[] bytes = new byte[hex.length() / 2];
    for (int i = 0; i < bytes.length; i++) {
      int hi = Character.digit(hex.charAt(i * 2), 16);
      int lo = Character.digit(hex.charAt(i * 2 + 1), 16);
      if (hi < 0 || lo < 0) {
        throw new IllegalArgumentException("Invalid hex character at position " + (i * 2));
      }
      bytes[i] = (byte) ((hi << 4) | lo);
    }
    return bytes;
  }
}
