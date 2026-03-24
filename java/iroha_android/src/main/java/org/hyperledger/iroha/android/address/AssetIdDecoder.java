// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import java.util.Locale;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountAddress.MultisigMemberPayload;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/**
 * Decodes norito-encoded asset identifiers to extract the asset definition ID.
 *
 * <p>Iroha returns asset IDs in {@code norito:<hex>} format. The binary payload encodes:
 *
 * <pre>
 *   struct AssetId {
 *       account: AccountId,
 *       definition: AssetDefinitionId,  // raw [u8; 16] UUIDv4 bytes
 *   }
 * </pre>
 *
 * <p>{@code AssetDefinitionId} is a one-way blake3 hash — name and domain cannot be recovered.
 * Use the app's cached asset definitions for display name lookup.
 */
public final class AssetIdDecoder {

  private static final String NORITO_PREFIX = "norito:";
  private static final byte[] ASSET_ID_SCHEMA_HASH =
      AssetIdEncoder.schemaHashForRustType("iroha_data_model::asset::id::model::AssetId");
  private static final byte[] ASSET_DEF_ID_SCHEMA_HASH =
      AssetIdEncoder.schemaHashForRustType(
          "iroha_data_model::asset::id::model::AssetDefinitionId");
  private static final int MULTISIG_POLICY_VERSION_V1 = 1;
  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<Long> UINT8_ADAPTER = NoritoAdapters.uint(8);
  private static final TypeAdapter<Long> UINT16_ADAPTER = NoritoAdapters.uint(16);
  private static final TypeAdapter<Long> UINT32_ADAPTER = NoritoAdapters.uint(32);

  private AssetIdDecoder() {}

  /**
   * Decoded asset definition carrying the canonical Base58 address.
   *
   * <p>Since {@code AssetDefinitionId} is now a one-way blake3 hash,
   * name and domain cannot be recovered from the binary representation.
   * Use the app's cached asset definitions for display info lookup.
   */
  public static final class AssetDefinition {
    private final String address;

    AssetDefinition(String address) {
      this.address = Objects.requireNonNull(address);
    }

    /** Returns the canonical unprefixed Base58 asset-definition address. */
    public String address() {
      return address;
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

  private static final int DEFINITION_BYTES_LEN = 16;

  /**
   * Decodes raw norito bytes for a full AssetId (account + definition).
   * Skips the account field and extracts the definition's 16 canonical bytes.
   */
  private static AssetDefinition decodeAssetIdBytes(byte[] data) {
    NoritoHeader.DecodeResult headerResult = NoritoHeader.decode(data, ASSET_ID_SCHEMA_HASH);
    NoritoHeader header = headerResult.header();
    byte[] payload = headerResult.payload();
    header.validateChecksum(payload);

    int flags = header.flags();
    int unsupportedFlags = flags & ~NoritoHeader.COMPACT_LEN;
    if (unsupportedFlags != 0) {
      throw new IllegalArgumentException(
          String.format(
              Locale.ROOT,
              "Unsupported norito AssetId layout flags for decoding: 0x%02x",
              unsupportedFlags));
    }
    int flagsHint = header.minor();
    boolean compactLen = (flags & NoritoHeader.COMPACT_LEN) != 0;
    NoritoDecoder decoder = new NoritoDecoder(payload, flags, flagsHint);

    // AssetId struct: { account: AccountId, definition: AssetDefinitionId, scope: ... }
    // Validate account field
    final int accountLen = checkedLength(decoder.readLength(compactLen), "Account field");
    final byte[] accountPayload = decoder.readBytes(accountLen);
    try {
      validateAccountPayload(accountPayload, flags, flagsHint);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException("Invalid AssetId.account payload", ex);
    }

    // Read definition field — [u8; 16] with per-element u64 length prefix
    final int definitionLen = checkedLength(decoder.readLength(compactLen), "Definition field");
    byte[] definitionPayload = decoder.readBytes(definitionLen);
    byte[] definitionBytes =
        decodeFixedByteArray(definitionPayload, DEFINITION_BYTES_LEN, flags, flagsHint);

    final int scopeLen = checkedLength(decoder.readLength(compactLen), "Scope field");
    final byte[] scopePayload = decoder.readBytes(scopeLen);
    validateScopePayload(scopePayload, flags, flagsHint);

    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after AssetId payload");
    }

    return new AssetDefinition(definitionAddressFromBytes(definitionBytes));
  }

  /**
   * Decodes raw norito bytes for an AssetDefinitionId (no account, just the [u8; 16] bytes).
   */
  private static AssetDefinition decodeDefinitionIdBytes(byte[] data) {
    NoritoHeader.DecodeResult headerResult = NoritoHeader.decode(data, ASSET_DEF_ID_SCHEMA_HASH);
    NoritoHeader header = headerResult.header();
    byte[] payload = headerResult.payload();
    header.validateChecksum(payload);
    final int flags = header.flags();
    final int unsupportedFlags = flags & ~NoritoHeader.COMPACT_LEN;
    if (unsupportedFlags != 0) {
      throw new IllegalArgumentException(
          String.format(
              Locale.ROOT,
              "Unsupported norito AssetDefinitionId layout flags for decoding: 0x%02x",
              unsupportedFlags));
    }

    byte[] definitionBytes =
        decodeFixedByteArray(payload, DEFINITION_BYTES_LEN, header.flags(), header.minor());
    return new AssetDefinition(definitionAddressFromBytes(definitionBytes));
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

  private static void validateScopePayload(final byte[] payload, final int flags, final int flagsHint) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, flags, flagsHint);
    final long scopeTag = UINT32_ADAPTER.decode(decoder);
    if (scopeTag == 0L) {
      if (decoder.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after AssetBalanceScope::Global");
      }
      return;
    }
    if (scopeTag == 1L) {
      final boolean compactLen = (flags & NoritoHeader.COMPACT_LEN) != 0;
      final int variantLen =
          checkedLength(decoder.readLength(compactLen), "Dataspace scope payload");
      final byte[] variantPayload = decoder.readBytes(variantLen);
      if (decoder.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after AssetBalanceScope payload");
      }
      final NoritoDecoder variantDecoder = new NoritoDecoder(variantPayload, flags, flagsHint);
      variantDecoder.readUInt(64);
      if (variantDecoder.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after AssetBalanceScope::Dataspace value");
      }
      return;
    }
    throw new IllegalArgumentException(
        "Unknown AssetBalanceScope discriminant in AssetId.scope: " + scopeTag);
  }

  private static void validateAccountPayload(final byte[] payload, final int flags, final int flagsHint) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, flags, flagsHint);
    final boolean compactLen = (flags & NoritoHeader.COMPACT_LEN) != 0;
    final long controllerTag = UINT32_ADAPTER.decode(decoder);
    final int variantLen =
        checkedLength(decoder.readLength(compactLen), "AccountController variant payload");
    final byte[] variantPayload = decoder.readBytes(variantLen);
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after AssetId.account payload");
    }

    if (controllerTag == 0L) {
      validateSingleControllerVariant(variantPayload, flags, flagsHint);
      return;
    }
    if (controllerTag == 1L) {
      validateMultisigControllerVariant(variantPayload, flags, flagsHint);
      return;
    }
    throw new IllegalArgumentException(
        "Unknown AccountController discriminant in AssetId.account: " + controllerTag);
  }

  private static void validateSingleControllerVariant(
      final byte[] payload, final int flags, final int flagsHint) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, flags, flagsHint);
    final String multihash = STRING_ADAPTER.decode(decoder);
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after AssetId.account single controller");
    }
    if (PublicKeyCodec.decodePublicKeyLiteral(multihash) == null) {
      throw new IllegalArgumentException("Invalid public key multihash in AssetId.account");
    }
  }

  private static void validateMultisigControllerVariant(
      final byte[] payload, final int flags, final int flagsHint) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, flags, flagsHint);
    final int version =
        Math.toIntExact(
            decodeSizedTypedField(decoder, UINT8_ADAPTER, "MultisigPolicy.version"));
    final int threshold =
        Math.toIntExact(
            decodeSizedTypedField(decoder, UINT16_ADAPTER, "MultisigPolicy.threshold"));
    final int membersPayloadLen =
        checkedLength(
            decoder.readLength((flags & NoritoHeader.COMPACT_LEN) != 0),
            "MultisigPolicy.members payload");
    final byte[] membersPayload = decoder.readBytes(membersPayloadLen);
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after AssetId.account multisig policy");
    }

    final NoritoDecoder membersDecoder = new NoritoDecoder(membersPayload, flags, flagsHint);
    final int membersCount =
        checkedLength(membersDecoder.readLength(false), "Multisig members count");
    final java.util.List<MultisigMemberPayload> members =
        new java.util.ArrayList<>(membersCount);
    for (int i = 0; i < membersCount; i++) {
      final int memberLen =
          checkedLength(
              membersDecoder.readLength((flags & NoritoHeader.COMPACT_LEN) != 0),
              "Multisig member payload");
      final byte[] memberPayload = membersDecoder.readBytes(memberLen);
      final NoritoDecoder memberDecoder = new NoritoDecoder(memberPayload, flags, flagsHint);

      final String memberMultihash =
          decodeSizedTypedField(memberDecoder, STRING_ADAPTER, "Multisig member public key");
      final int weight =
          Math.toIntExact(
              decodeSizedTypedField(memberDecoder, UINT16_ADAPTER, "Multisig member weight"));
      if (memberDecoder.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after multisig member payload");
      }

      final PublicKeyCodec.PublicKeyPayload keyPayload =
          PublicKeyCodec.decodePublicKeyLiteral(memberMultihash);
      if (keyPayload == null) {
        throw new IllegalArgumentException("Invalid multisig member public key");
      }
      members.add(MultisigMemberPayload.of(keyPayload.curveId(), weight, keyPayload.keyBytes()));
    }
    if (membersDecoder.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after multisig member vector payload");
    }

    validateMultisigPolicySemantics(version, threshold, members);
  }

  private static void validateMultisigPolicySemantics(
      final int version, final int threshold, final java.util.List<MultisigMemberPayload> members) {
    if (version != MULTISIG_POLICY_VERSION_V1) {
      throw new IllegalArgumentException(
          "Invalid multisig policy: unsupported version " + version);
    }
    if (members.isEmpty()) {
      throw new IllegalArgumentException("Invalid multisig policy: zero members");
    }
    long totalWeight = 0L;
    final java.util.List<byte[]> sortKeys = new java.util.ArrayList<>(members.size());
    for (final MultisigMemberPayload member : members) {
      if (member.weight() <= 0) {
        throw new IllegalArgumentException("Invalid multisig policy: non-positive weight");
      }
      if (member.publicKey().length == 0) {
        throw new IllegalArgumentException("Invalid multisig policy: empty public key");
      }
      totalWeight += member.weight();
      sortKeys.add(canonicalSortKey(member));
    }
    if (threshold <= 0) {
      throw new IllegalArgumentException("Invalid multisig policy: zero threshold");
    }
    if (totalWeight < threshold) {
      throw new IllegalArgumentException("Invalid multisig policy: threshold exceeds total weight");
    }
    sortKeys.sort(AssetIdDecoder::compareUnsigned);
    for (int i = 1; i < sortKeys.size(); i++) {
      if (java.util.Arrays.equals(sortKeys.get(i - 1), sortKeys.get(i))) {
        throw new IllegalArgumentException("Invalid multisig policy: duplicate member");
      }
    }
  }

  private static byte[] canonicalSortKey(final MultisigMemberPayload member) {
    final String algorithm = PublicKeyCodec.algorithmForCurveId(member.curveId());
    if (algorithm == null) {
      throw new IllegalArgumentException("Invalid multisig policy: unknown curve id");
    }
    final byte[] algorithmBytes = algorithm.getBytes(java.nio.charset.StandardCharsets.UTF_8);
    final byte[] keyBytes = member.publicKey();
    final byte[] sortKey = new byte[algorithmBytes.length + 1 + keyBytes.length];
    System.arraycopy(algorithmBytes, 0, sortKey, 0, algorithmBytes.length);
    sortKey[algorithmBytes.length] = 0;
    System.arraycopy(keyBytes, 0, sortKey, algorithmBytes.length + 1, keyBytes.length);
    return sortKey;
  }

  private static int compareUnsigned(final byte[] a, final byte[] b) {
    final int len = Math.min(a.length, b.length);
    for (int i = 0; i < len; i++) {
      final int cmp = (a[i] & 0xFF) - (b[i] & 0xFF);
      if (cmp != 0) {
        return cmp;
      }
    }
    return Integer.compare(a.length, b.length);
  }

  private static <T> T decodeSizedTypedField(
      final NoritoDecoder decoder, final TypeAdapter<T> adapter, final String fieldName) {
    final int payloadLength =
        checkedLength(
            decoder.readLength((decoder.flags() & NoritoHeader.COMPACT_LEN) != 0),
            fieldName + " payload");
    final byte[] payload = decoder.readBytes(payloadLength);
    final NoritoDecoder child = new NoritoDecoder(payload, decoder.flags(), decoder.flagsHint());
    final T value = adapter.decode(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after " + fieldName + " payload");
    }
    return value;
  }

  private static int checkedLength(final long length, final String fieldName) {
    if (length < 0L) {
      throw new IllegalArgumentException(fieldName + " must be non-negative");
    }
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(fieldName + " too large");
    }
    return (int) length;
  }

  private static String definitionAddressFromBytes(byte[] definitionBytes) {
    return AssetDefinitionIdEncoder.encodeFromBytes(definitionBytes);
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
