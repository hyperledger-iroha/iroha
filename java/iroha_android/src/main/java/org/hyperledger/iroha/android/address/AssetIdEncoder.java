// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.util.Objects;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;
/**
 * Encodes asset identifiers into norito {@code norito:<hex>} format.
 *
 * <p>Iroha API endpoints now require asset IDs in norito binary format
 * instead of text form ({@code name#domain}).
 *
 * <pre>
 *   struct AssetId {
 *       account: AccountId,
 *       definition: AssetDefinitionId,  // raw [u8; 16] aid_bytes
 *       scope: AssetBalanceScope,       // #[norito(default)]
 *   }
 *   // #[norito(transparent)] — serializes directly as AccountController
 *   struct AccountId {
 *       controller: AccountController,
 *   }
 *   enum AccountController {
 *       Single(PublicKey),   // discriminant 0
 *   }
 *   // AssetDefinitionId = [u8; 16] blake3-derived aid_bytes
 * </pre>
 */
public final class AssetIdEncoder {

  private static final String NORITO_PREFIX = "norito:";
  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<Long> UINT32_ADAPTER = NoritoAdapters.uint(32);

  /**
   * Schema hash for {@code iroha_data_model::asset::id::model::AssetId}.
   * Computed as FNV-1a of the Rust type name, duplicated to 16 bytes.
   */
  private static final byte[] ASSET_ID_SCHEMA_HASH =
      schemaHashForRustType("iroha_data_model::asset::id::model::AssetId");

  /**
   * Schema hash for {@code iroha_data_model::asset::id::model::AssetDefinitionId}.
   * Used when encoding standalone asset definition identifiers.
   */
  private static final byte[] ASSET_DEF_ID_SCHEMA_HASH =
      schemaHashForRustType("iroha_data_model::asset::id::model::AssetDefinitionId");

  private AssetIdEncoder() {}

  /**
   * Encodes a full asset ID to {@code norito:<hex>} format.
   *
   * @param assetName    the asset name (e.g., "rose")
   * @param domainName   the domain name (e.g., "wonderland")
   * @param publicKeyHex the public key in Iroha hex format (e.g., "ed0120ABCD...")
   * @return the norito-encoded string
   */
  public static String encodeAssetId(String assetName, String domainName, String publicKeyHex) {
    Objects.requireNonNull(assetName, "assetName");
    Objects.requireNonNull(domainName, "domainName");
    Objects.requireNonNull(publicKeyHex, "publicKeyHex");

    int flags = 0;
    NoritoEncoder encoder = new NoritoEncoder(flags);

    // AccountId { controller: AccountController }
    byte[] accountBytes = encodeAccountIdPayload(flags, publicKeyHex);
    encoder.writeUInt(accountBytes.length, 64);
    encoder.writeBytes(accountBytes);

    // AssetDefinitionId = [u8; 16] aid_bytes — each byte gets u64 length prefix per Norito [T; N]
    byte[] aidBytes = AssetDefinitionIdEncoder.computeAidBytes(assetName, domainName);
    NoritoEncoder aidEncoder = new NoritoEncoder(flags);
    org.hyperledger.iroha.android.model.instructions.TransferWirePayloadEncoder
        .encodeFixedByteArray(aidEncoder, aidBytes);
    byte[] aidPayload = aidEncoder.toByteArray();
    encoder.writeUInt(aidPayload.length, 64);
    encoder.writeBytes(aidPayload);

    // scope: AssetBalanceScope::Global = enum discriminant 0, unit variant
    encodeAssetBalanceScopeGlobal(encoder);

    byte[] payload = encoder.toByteArray();
    return wrapWithHeader(payload, flags, ASSET_ID_SCHEMA_HASH);
  }

  /**
   * Encodes a full asset ID to {@code norito:<hex>} format using a pre-computed {@code aid:} string.
   *
   * @param aidString    the asset definition ID in {@code aid:<hex>} format
   * @param publicKeyHex the public key in Iroha hex format (e.g., "ed0120ABCD...")
   * @return the norito-encoded string
   */
  public static String encodeAssetIdFromAid(String aidString, String publicKeyHex) {
    Objects.requireNonNull(aidString, "aidString");
    Objects.requireNonNull(publicKeyHex, "publicKeyHex");

    int flags = 0;
    NoritoEncoder encoder = new NoritoEncoder(flags);

    byte[] accountBytes = encodeAccountIdPayload(flags, publicKeyHex);
    encoder.writeUInt(accountBytes.length, 64);
    encoder.writeBytes(accountBytes);

    byte[] aidBytes = AssetDefinitionIdEncoder.parseAidBytes(aidString);
    NoritoEncoder aidEncoder2 = new NoritoEncoder(flags);
    org.hyperledger.iroha.android.model.instructions.TransferWirePayloadEncoder
        .encodeFixedByteArray(aidEncoder2, aidBytes);
    byte[] aidPayload2 = aidEncoder2.toByteArray();
    encoder.writeUInt(aidPayload2.length, 64);
    encoder.writeBytes(aidPayload2);

    encodeAssetBalanceScopeGlobal(encoder);

    byte[] payload = encoder.toByteArray();
    return wrapWithHeader(payload, flags, ASSET_ID_SCHEMA_HASH);
  }

  /**
   * Encodes an asset definition ID ({@code name#domain}) to {@code norito:<hex>} format.
   *
   * @param assetName  the asset name (e.g., "rose")
   * @param domainName the domain name (e.g., "wonderland")
   * @return the norito-encoded string (e.g., "norito:4e5254...")
   */
  public static String encodeDefinition(String assetName, String domainName) {
    Objects.requireNonNull(assetName, "assetName");
    Objects.requireNonNull(domainName, "domainName");

    int flags = 0;
    byte[] rawAidBytes = AssetDefinitionIdEncoder.computeAidBytes(assetName, domainName);
    NoritoEncoder aidDefEncoder = new NoritoEncoder(flags);
    org.hyperledger.iroha.android.model.instructions.TransferWirePayloadEncoder
        .encodeFixedByteArray(aidDefEncoder, rawAidBytes);
    byte[] payload = aidDefEncoder.toByteArray();
    return wrapWithHeader(payload, flags, ASSET_DEF_ID_SCHEMA_HASH);
  }

  /**
   * Encodes AssetBalanceScope::Global as a length-prefixed struct field.
   * Global is a unit enum variant: u32(0) with no payload, wrapped in u64 length prefix.
   */
  private static void encodeAssetBalanceScopeGlobal(NoritoEncoder encoder) {
    NoritoEncoder child = new NoritoEncoder(0);
    UINT32_ADAPTER.encode(child, 0L);
    byte[] payload = child.toByteArray();
    encoder.writeUInt(payload.length, 64);
    encoder.writeBytes(payload);
  }

  /**
   * Encodes AccountId which is {@code #[norito(transparent)]} over AccountController.
   * The transparent attribute means AccountId serializes directly as its inner field
   * without an extra length prefix wrapper.
   * AccountController::Single(PublicKey) = u32(0) + length-prefixed PublicKey string.
   */
  private static byte[] encodeAccountIdPayload(int flags, String publicKeyHex) {
    PublicKeyCodec.PublicKeyPayload publicKey =
        PublicKeyCodec.decodePublicKeyLiteral(publicKeyHex);
    if (publicKey == null) {
      throw new IllegalArgumentException("Invalid public key literal: " + publicKeyHex);
    }
    String canonicalMultihash =
        PublicKeyCodec.encodePublicKeyMultihash(publicKey.curveId(), publicKey.keyBytes());

    // AccountId is #[norito(transparent)] -> encodes directly as AccountController
    NoritoEncoder controllerEncoder = new NoritoEncoder(flags);
    UINT32_ADAPTER.encode(controllerEncoder, 0L); // Single variant discriminant
    NoritoEncoder publicKeyEncoder = new NoritoEncoder(flags);
    STRING_ADAPTER.encode(publicKeyEncoder, canonicalMultihash);
    byte[] publicKeyBytes = publicKeyEncoder.toByteArray();
    controllerEncoder.writeUInt(publicKeyBytes.length, 64);
    controllerEncoder.writeBytes(publicKeyBytes);

    return controllerEncoder.toByteArray();
  }

  private static String wrapWithHeader(byte[] payload, int flags, byte[] schemaHash) {
    long checksum = CRC64.compute(payload);
    NoritoHeader header =
        new NoritoHeader(schemaHash, payload.length, checksum, flags, NoritoHeader.COMPRESSION_NONE);
    byte[] headerBytes = header.encode();

    byte[] result = new byte[headerBytes.length + payload.length];
    System.arraycopy(headerBytes, 0, result, 0, headerBytes.length);
    System.arraycopy(payload, 0, result, headerBytes.length, payload.length);

    return NORITO_PREFIX + bytesToHex(result);
  }

  private static String bytesToHex(byte[] bytes) {
    StringBuilder sb = new StringBuilder(bytes.length * 2);
    for (byte b : bytes) {
      sb.append(String.format("%02x", b & 0xff));
    }
    return sb.toString();
  }

  /**
   * Computes a 16-byte norito schema hash from a Rust type name using FNV-1a,
   * matching {@code norito::core::compute_schema_hash}.
   */
  static byte[] schemaHashForRustType(String rustTypeName) {
    long hash = 0xcbf29ce484222325L;
    byte[] nameBytes = rustTypeName.getBytes(StandardCharsets.UTF_8);
    for (byte b : nameBytes) {
      hash ^= (b & 0xffL);
      hash *= 0x100000001b3L;
    }
    byte[] part = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(hash).array();
    byte[] out = new byte[16];
    System.arraycopy(part, 0, out, 0, 8);
    System.arraycopy(part, 0, out, 8, 8);
    return out;
  }
}
