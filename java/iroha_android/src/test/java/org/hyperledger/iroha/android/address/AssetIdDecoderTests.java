// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import java.util.Locale;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoHeader;

public final class AssetIdDecoderTests {

  private static final String ED25519_KEY =
      "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";

  private AssetIdDecoderTests() {}

  public static void main(final String[] args) {
    isNoritoEncodedReturnsTrueForNoritoPrefix();
    isNoritoEncodedReturnsFalseForHashFormat();
    isNoritoEncodedReturnsFalseForNull();
    decodeExtractsAidHexFromNoritoEncodedAssetId();
    decodeRoundtripForRoseInWonderland();
    decodeRejectsWrongAssetIdSchema();
    decodeDefinitionExtractsAidHex();
    decodeDefinitionHandlesRaw16BytePayload();
    decodeDefinitionRejectsWrongSchema();
    aidHexStartsWithAidPrefix();
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

  private static void decodeExtractsAidHexFromNoritoEncodedAssetId() {
    final String encoded = AssetIdEncoder.encodeAssetId(
        "cabbage", "garden_of_live_flowers", ED25519_KEY);

    final AssetIdDecoder.AssetDefinition result = AssetIdDecoder.decode(encoded);
    final String expectedAid = AssetDefinitionIdEncoder.encode("cabbage", "garden_of_live_flowers");

    assert expectedAid.equals(result.aidHex())
        : "decoded aidHex must match expected aid for cabbage#garden_of_live_flowers";
  }

  private static void decodeRoundtripForRoseInWonderland() {
    final String encoded = AssetIdEncoder.encodeAssetId(
        "rose", "wonderland", ED25519_KEY);

    final AssetIdDecoder.AssetDefinition result = AssetIdDecoder.decode(encoded);
    final String expectedAid = AssetDefinitionIdEncoder.encode("rose", "wonderland");

    assert expectedAid.equals(result.aidHex())
        : "decoded aidHex must match expected aid for rose#wonderland";
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

  private static void decodeDefinitionExtractsAidHex() {
    final String encoded = AssetIdEncoder.encodeDefinition("rose", "wonderland");
    final AssetIdDecoder.AssetDefinition result = AssetIdDecoder.decodeDefinition(encoded);
    final String expectedAid = AssetDefinitionIdEncoder.encode("rose", "wonderland");

    assert expectedAid.equals(result.aidHex())
        : "decoded aidHex must match expected aid for rose#wonderland";
  }

  private static void decodeDefinitionHandlesRaw16BytePayload() {
    final byte[] aidBytes = AssetDefinitionIdEncoder.computeAidBytes("rose", "wonderland");
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

    assert expectedAid.equals(result.aidHex())
        : "raw 16-byte fast path must produce same aidHex as per-element encoding";
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

  private static void aidHexStartsWithAidPrefix() {
    final String encoded = AssetIdEncoder.encodeDefinition("pkr", "sbp");
    final AssetIdDecoder.AssetDefinition result = AssetIdDecoder.decodeDefinition(encoded);

    assert result.aidHex().startsWith("aid:") : "aidHex must start with aid: prefix";
  }

  private static String rewriteSchemaHash(final String noritoLiteral, final byte[] schemaHash) {
    final byte[] bytes = hexToBytes(noritoLiteral.substring("norito:".length()));
    final int schemaOffset = NoritoHeader.MAGIC.length + 2;
    System.arraycopy(schemaHash, 0, bytes, schemaOffset, schemaHash.length);
    return "norito:" + bytesToHex(bytes);
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
