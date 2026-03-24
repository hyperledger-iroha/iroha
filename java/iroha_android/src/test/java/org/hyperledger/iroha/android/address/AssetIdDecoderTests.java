// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import java.util.Locale;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;

public final class AssetIdDecoderTests {

  private static final String ED25519_KEY =
      "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
  private static final byte[] ASSET_ID_SCHEMA_HASH =
      AssetIdEncoder.schemaHashForRustType("iroha_data_model::asset::id::model::AssetId");

  private AssetIdDecoderTests() {}

  public static void main(final String[] args) {
    isNoritoEncodedReturnsTrueForNoritoPrefix();
    isNoritoEncodedReturnsFalseForHashFormat();
    isNoritoEncodedReturnsFalseForNull();
    decodeExtractsDefinitionAddressFromNoritoEncodedAssetId();
    decodeRoundtripForRoseInWonderland();
    decodeRejectsWrongAssetIdSchema();
    decodeRejectsMalformedAccountPayload();
    decodeRejectsUnsupportedAssetIdLayoutFlags();
    decodeRejectsMalformedScopePayload();
    decodeRejectsTrailingBytesAfterScopePayload();
    decodeRejectsNegativeAccountLength();
    decodeDefinitionExtractsDefinitionAddress();
    decodeDefinitionHandlesRaw16BytePayload();
    decodeDefinitionRejectsWrongSchema();
    decodeDefinitionRejectsUnsupportedLayoutFlags();
    decodedAddressUsesCanonicalBase58();
    System.out.println("[IrohaAndroid] AssetIdDecoder tests passed.");
  }

  private static void isNoritoEncodedReturnsTrueForNoritoPrefix() {
    assert AssetIdDecoder.isNoritoEncoded("norito:4e5254")
        : "isNoritoEncoded must return true for norito: prefix";
  }

  private static void isNoritoEncodedReturnsFalseForHashFormat() {
    assert !AssetIdDecoder.isNoritoEncoded("cabbage#garden")
        : "isNoritoEncoded must return false for hash format";
  }

  private static void isNoritoEncodedReturnsFalseForNull() {
    assert !AssetIdDecoder.isNoritoEncoded(null)
        : "isNoritoEncoded must return false for null";
  }

  private static void decodeExtractsDefinitionAddressFromNoritoEncodedAssetId() {
    final String encoded = AssetIdEncoder.encodeAssetId(
        "cabbage", "garden_of_live_flowers", ED25519_KEY);

    final AssetIdDecoder.AssetDefinition result = AssetIdDecoder.decode(encoded);
    final String expectedAid = AssetDefinitionIdEncoder.encode("cabbage", "garden_of_live_flowers");

    assert expectedAid.equals(result.address())
        : "decoded address must match expected asset-definition address";
  }

  private static void decodeRoundtripForRoseInWonderland() {
    final String encoded = AssetIdEncoder.encodeAssetId(
        "rose", "wonderland", ED25519_KEY);

    final AssetIdDecoder.AssetDefinition result = AssetIdDecoder.decode(encoded);
    final String expectedAid = AssetDefinitionIdEncoder.encode("rose", "wonderland");

    assert expectedAid.equals(result.address())
        : "decoded address must match expected asset-definition address";
  }

  private static void decodeRejectsWrongAssetIdSchema() {
    final String encoded = AssetIdEncoder.encodeAssetId("rose", "wonderland", ED25519_KEY);
    final String wrongSchema =
        rewriteSchemaHash(
            encoded,
            AssetIdEncoder.schemaHashForRustType(
                "iroha_data_model::asset::id::model::AssetDefinitionId"));

    boolean threw = false;
    try {
      AssetIdDecoder.decode(wrongSchema);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage() != null && ex.getMessage().contains("Schema mismatch");
    }

    assert threw : "decode must reject norito payloads with a mismatched schema hash";
  }

  private static void decodeRejectsMalformedAccountPayload() {
    final String encoded = AssetIdEncoder.encodeAssetId("rose", "wonderland", ED25519_KEY);
    final ParsedAssetIdPayload parsed = parseAssetIdPayload(encoded);
    final byte[] malformedAccountPayload = new byte[] {0x01};
    final String malformedLiteral =
        buildAssetIdLiteral(
            malformedAccountPayload,
            parsed.definitionPayload,
            parsed.scopePayload,
            parsed.flags,
            null);

    boolean threw = false;
    try {
      AssetIdDecoder.decode(malformedLiteral);
    } catch (final IllegalArgumentException ex) {
      final String message = ex.getMessage();
      threw = message != null && message.contains("Invalid AssetId.account payload");
    }

    assert threw : "decode must reject malformed AssetId.account payloads";
  }

  private static void decodeRejectsUnsupportedAssetIdLayoutFlags() {
    final String encoded = AssetIdEncoder.encodeAssetId("rose", "wonderland", ED25519_KEY);
    final String malformedLiteral = rewriteFlags(encoded, NoritoHeader.PACKED_STRUCT);

    boolean threw = false;
    try {
      AssetIdDecoder.decode(malformedLiteral);
    } catch (final IllegalArgumentException ex) {
      final String message = ex.getMessage();
      threw =
          message != null
              && message.contains("Unsupported norito AssetId layout flags for decoding");
    }

    assert threw : "decode must reject unsupported AssetId layout flags";
  }

  private static void decodeRejectsMalformedScopePayload() {
    final String encoded = AssetIdEncoder.encodeAssetId("rose", "wonderland", ED25519_KEY);
    final ParsedAssetIdPayload parsed = parseAssetIdPayload(encoded);

    // AssetBalanceScope::Dataspace without the required variant payload.
    final byte[] malformedScope = new byte[] {0x01, 0x00, 0x00, 0x00};
    final String malformedLiteral =
        buildAssetIdLiteral(
            parsed.accountPayload, parsed.definitionPayload, malformedScope, parsed.flags, null);

    boolean threw = false;
    try {
      AssetIdDecoder.decode(malformedLiteral);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }

    assert threw : "decode must reject malformed AssetId.scope payloads";
  }

  private static void decodeRejectsTrailingBytesAfterScopePayload() {
    final String encoded = AssetIdEncoder.encodeAssetId("rose", "wonderland", ED25519_KEY);
    final ParsedAssetIdPayload parsed = parseAssetIdPayload(encoded);
    final String malformedLiteral =
        buildAssetIdLiteral(
            parsed.accountPayload,
            parsed.definitionPayload,
            parsed.scopePayload,
            parsed.flags,
            new byte[] {0x7f});

    boolean threw = false;
    try {
      AssetIdDecoder.decode(malformedLiteral);
    } catch (final IllegalArgumentException ex) {
      final String message = ex.getMessage();
      threw = message != null && message.contains("Trailing bytes after AssetId payload");
    }

    assert threw : "decode must reject trailing bytes after the AssetId scope field";
  }

  private static void decodeRejectsNegativeAccountLength() {
    final String encoded = AssetIdEncoder.encodeAssetId("rose", "wonderland", ED25519_KEY);
    final byte[] raw = hexToBytes(encoded.substring("norito:".length()));
    final NoritoHeader.DecodeResult decoded = NoritoHeader.decode(raw, ASSET_ID_SCHEMA_HASH);
    final byte[] payload = decoded.payload();
    for (int i = 0; i < Long.BYTES; i++) {
      payload[i] = (byte) 0xFF;
    }

    final String malformedLiteral = wrapAssetIdPayload(payload, decoded.header().flags());
    boolean threw = false;
    try {
      AssetIdDecoder.decode(malformedLiteral);
    } catch (final IllegalArgumentException ex) {
      final String message = ex.getMessage();
      threw = message != null && message.contains("Account field must be non-negative");
    }

    assert threw : "decode must reject negative account field lengths";
  }

  private static void decodeDefinitionExtractsDefinitionAddress() {
    final String encoded = AssetIdEncoder.encodeDefinition("rose", "wonderland");
    final AssetIdDecoder.AssetDefinition result = AssetIdDecoder.decodeDefinition(encoded);
    final String expectedAid = AssetDefinitionIdEncoder.encode("rose", "wonderland");

    assert expectedAid.equals(result.address())
        : "decoded address must match expected asset-definition address";
  }

  private static void decodeDefinitionHandlesRaw16BytePayload() {
    final byte[] aidBytes = AssetDefinitionIdEncoder.computeDefinitionBytes("rose", "wonderland");
    final byte[] schemaHash = AssetIdEncoder.schemaHashForRustType(
        "iroha_data_model::asset::id::model::AssetDefinitionId");
    final long checksum = CRC64.compute(aidBytes);
    final NoritoHeader header = new NoritoHeader(
        schemaHash, aidBytes.length, checksum, 0, NoritoHeader.COMPRESSION_NONE);
    final byte[] headerBytes = header.encode();

    final byte[] raw = new byte[headerBytes.length + aidBytes.length];
    System.arraycopy(headerBytes, 0, raw, 0, headerBytes.length);
    System.arraycopy(aidBytes, 0, raw, headerBytes.length, aidBytes.length);

    final StringBuilder sb = new StringBuilder("norito:");
    for (final byte b : raw) {
      sb.append(String.format(Locale.ROOT, "%02x", b & 0xFF));
    }

    final AssetIdDecoder.AssetDefinition result = AssetIdDecoder.decodeDefinition(sb.toString());
    final String expectedAid = AssetDefinitionIdEncoder.encode("rose", "wonderland");

    assert expectedAid.equals(result.address())
        : "raw 16-byte fast path must produce the same address as per-element encoding";
  }

  private static void decodeDefinitionRejectsWrongSchema() {
    final String encoded = AssetIdEncoder.encodeDefinition("rose", "wonderland");
    final String wrongSchema =
        rewriteSchemaHash(
            encoded,
            AssetIdEncoder.schemaHashForRustType("iroha_data_model::asset::id::model::AssetId"));

    boolean threw = false;
    try {
      AssetIdDecoder.decodeDefinition(wrongSchema);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage() != null && ex.getMessage().contains("Schema mismatch");
    }

    assert threw : "decodeDefinition must reject norito payloads with a mismatched schema hash";
  }

  private static void decodeDefinitionRejectsUnsupportedLayoutFlags() {
    final String encoded = AssetIdEncoder.encodeDefinition("rose", "wonderland");
    final String malformedLiteral = rewriteFlags(encoded, NoritoHeader.PACKED_STRUCT);

    boolean threw = false;
    try {
      AssetIdDecoder.decodeDefinition(malformedLiteral);
    } catch (final IllegalArgumentException ex) {
      final String message = ex.getMessage();
      threw =
          message != null
              && message.contains("Unsupported norito AssetDefinitionId layout flags for decoding");
    }

    assert threw : "decodeDefinition must reject unsupported AssetDefinitionId layout flags";
  }

  private static void decodedAddressUsesCanonicalBase58() {
    final String encoded = AssetIdEncoder.encodeDefinition("pkr", "sbp");
    final AssetIdDecoder.AssetDefinition result = AssetIdDecoder.decodeDefinition(encoded);

    assert AssetDefinitionIdEncoder.isCanonicalAddress(result.address())
        : "decoded definition must use canonical Base58 address form";
  }

  private static String rewriteSchemaHash(final String noritoLiteral, final byte[] schemaHash) {
    final byte[] bytes = hexToBytes(noritoLiteral.substring("norito:".length()));
    final int schemaOffset = NoritoHeader.MAGIC.length + 2;
    System.arraycopy(schemaHash, 0, bytes, schemaOffset, schemaHash.length);
    return "norito:" + bytesToHex(bytes);
  }

  private static String rewriteFlags(final String noritoLiteral, final int flags) {
    final byte[] bytes = hexToBytes(noritoLiteral.substring("norito:".length()));
    bytes[NoritoHeader.HEADER_LENGTH - 1] = (byte) (flags & 0xFF);
    return "norito:" + bytesToHex(bytes);
  }

  private static ParsedAssetIdPayload parseAssetIdPayload(final String noritoLiteral) {
    final byte[] raw = hexToBytes(noritoLiteral.substring("norito:".length()));
    final NoritoHeader.DecodeResult decoded = NoritoHeader.decode(raw, ASSET_ID_SCHEMA_HASH);
    final NoritoHeader header = decoded.header();
    final byte[] payload = decoded.payload();
    header.validateChecksum(payload);

    final boolean compactLen = (header.flags() & NoritoHeader.COMPACT_LEN) != 0;
    final NoritoDecoder decoder = new NoritoDecoder(payload, header.flags(), header.minor());

    final int accountLen = checkedLength(decoder.readLength(compactLen));
    final byte[] accountPayload = decoder.readBytes(accountLen);
    final int definitionLen = checkedLength(decoder.readLength(compactLen));
    final byte[] definitionPayload = decoder.readBytes(definitionLen);
    final int scopeLen = checkedLength(decoder.readLength(compactLen));
    final byte[] scopePayload = decoder.readBytes(scopeLen);
    if (decoder.remaining() != 0) {
      throw new AssertionError("fixture AssetId payload must not have trailing bytes");
    }

    return new ParsedAssetIdPayload(accountPayload, definitionPayload, scopePayload, header.flags());
  }

  private static String buildAssetIdLiteral(
      final byte[] accountPayload,
      final byte[] definitionPayload,
      final byte[] scopePayload,
      final int flags,
      final byte[] trailing) {
    final boolean compactLen = (flags & NoritoHeader.COMPACT_LEN) != 0;
    final NoritoEncoder encoder = new NoritoEncoder(flags);
    encoder.writeLength(accountPayload.length, compactLen);
    encoder.writeBytes(accountPayload);
    encoder.writeLength(definitionPayload.length, compactLen);
    encoder.writeBytes(definitionPayload);
    encoder.writeLength(scopePayload.length, compactLen);
    encoder.writeBytes(scopePayload);
    if (trailing != null && trailing.length > 0) {
      encoder.writeBytes(trailing);
    }
    return wrapAssetIdPayload(encoder.toByteArray(), flags);
  }

  private static String wrapAssetIdPayload(final byte[] payload, final int flags) {
    final NoritoHeader header =
        new NoritoHeader(
            ASSET_ID_SCHEMA_HASH,
            payload.length,
            CRC64.compute(payload),
            flags,
            NoritoHeader.COMPRESSION_NONE);
    final byte[] headerBytes = header.encode();
    final byte[] full = new byte[headerBytes.length + payload.length];
    System.arraycopy(headerBytes, 0, full, 0, headerBytes.length);
    System.arraycopy(payload, 0, full, headerBytes.length, payload.length);
    return "norito:" + bytesToHex(full);
  }

  private static int checkedLength(final long length) {
    if (length < 0L || length > Integer.MAX_VALUE) {
      throw new AssertionError("fixture length out of range: " + length);
    }
    return (int) length;
  }

  private static final class ParsedAssetIdPayload {
    private final byte[] accountPayload;
    private final byte[] definitionPayload;
    private final byte[] scopePayload;
    private final int flags;

    private ParsedAssetIdPayload(
        final byte[] accountPayload,
        final byte[] definitionPayload,
        final byte[] scopePayload,
        final int flags) {
      this.accountPayload = accountPayload;
      this.definitionPayload = definitionPayload;
      this.scopePayload = scopePayload;
      this.flags = flags;
    }
  }

  private static byte[] hexToBytes(final String hex) {
    final byte[] bytes = new byte[hex.length() / 2];
    for (int i = 0; i < bytes.length; i++) {
      final int hi = Character.digit(hex.charAt(i * 2), 16);
      final int lo = Character.digit(hex.charAt(i * 2 + 1), 16);
      bytes[i] = (byte) ((hi << 4) | lo);
    }
    return bytes;
  }

  private static String bytesToHex(final byte[] bytes) {
    final StringBuilder sb = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      sb.append(String.format(Locale.ROOT, "%02x", b & 0xFF));
    }
    return sb.toString();
  }
}
