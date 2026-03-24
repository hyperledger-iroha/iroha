// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.address

import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

private const val NORITO_PREFIX = "norito:"

/**
 * Encodes asset identifiers into norito `norito:<hex>` format.
 *
 * Iroha API endpoints now require asset IDs in norito binary format
 * instead of text form (`name#domain`).
 *
 * ```
 *   struct AssetId {
 *       account: AccountId,
 *       definition: AssetDefinitionId,  // raw [u8; 16] aid_bytes
 *       scope: AssetBalanceScope,       // #[norito(default)]
 *   }
 *   // #[norito(transparent)] -- serializes directly as AccountController
 *   struct AccountId {
 *       controller: AccountController,
 *   }
 *   enum AccountController {
 *       Single(PublicKey),   // discriminant 0
 *   }
 *   // AssetDefinitionId = [u8; 16] blake3-derived aid_bytes
 * ```
 */
object AssetIdEncoder {

    private val STRING_ADAPTER = NoritoAdapters.stringAdapter()
    private val UINT32_ADAPTER = NoritoAdapters.uint(32)

    /**
     * Schema hash for `iroha_data_model::asset::id::model::AssetId`.
     * Computed as FNV-1a of the Rust type name, duplicated to 16 bytes.
     */
    private val ASSET_ID_SCHEMA_HASH =
        SchemaHash.hash16("iroha_data_model::asset::id::model::AssetId")

    /**
     * Schema hash for `iroha_data_model::asset::id::model::AssetDefinitionId`.
     * Used when encoding standalone asset definition identifiers.
     */
    private val ASSET_DEF_ID_SCHEMA_HASH =
        SchemaHash.hash16("iroha_data_model::asset::id::model::AssetDefinitionId")

    /**
     * Encodes a full asset ID to `norito:<hex>` format.
     *
     * @param assetName    the asset name (e.g., "rose")
     * @param domainName   the domain name (e.g., "wonderland")
     * @param publicKeyHex the public key in Iroha hex format (e.g., "ed0120ABCD...")
     * @return the norito-encoded string
     */
    @JvmStatic
    fun encodeAssetId(assetName: String, domainName: String, publicKeyHex: String): String {
        val flags = 0
        val encoder = NoritoEncoder(flags)

        val accountBytes = encodeAccountIdPayload(flags, publicKeyHex)
        encoder.writeUInt(accountBytes.size.toLong(), 64)
        encoder.writeBytes(accountBytes)

        val aidBytes = AssetDefinitionIdEncoder.computeAidBytes(assetName, domainName)
        val aidPayload = encodeFixedByteArrayPayload(flags, aidBytes)
        encoder.writeUInt(aidPayload.size.toLong(), 64)
        encoder.writeBytes(aidPayload)

        encodeAssetBalanceScopeGlobal(encoder)

        return wrapWithHeader(encoder.toByteArray(), flags, ASSET_ID_SCHEMA_HASH)
    }

    /**
     * Encodes a full asset ID to `norito:<hex>` format using a pre-computed `aid:` string.
     *
     * @param aidString    the asset definition ID in `aid:<hex>` format
     * @param publicKeyHex the public key in Iroha hex format (e.g., "ed0120ABCD...")
     * @return the norito-encoded string
     */
    @JvmStatic
    fun encodeAssetIdFromAid(aidString: String, publicKeyHex: String): String {
        val flags = 0
        val encoder = NoritoEncoder(flags)

        val accountBytes = encodeAccountIdPayload(flags, publicKeyHex)
        encoder.writeUInt(accountBytes.size.toLong(), 64)
        encoder.writeBytes(accountBytes)

        val aidBytes = AssetDefinitionIdEncoder.parseAidBytes(aidString)
        val aidPayload = encodeFixedByteArrayPayload(flags, aidBytes)
        encoder.writeUInt(aidPayload.size.toLong(), 64)
        encoder.writeBytes(aidPayload)

        encodeAssetBalanceScopeGlobal(encoder)

        return wrapWithHeader(encoder.toByteArray(), flags, ASSET_ID_SCHEMA_HASH)
    }

    /**
     * Encodes an asset definition ID (`name#domain`) to `norito:<hex>` format.
     *
     * @param assetName  the asset name (e.g., "rose")
     * @param domainName the domain name (e.g., "wonderland")
     * @return the norito-encoded string (e.g., "norito:4e5254...")
     */
    @JvmStatic
    fun encodeDefinition(assetName: String, domainName: String): String {
        val flags = 0
        val rawAidBytes = AssetDefinitionIdEncoder.computeAidBytes(assetName, domainName)
        val payload = encodeFixedByteArrayPayload(flags, rawAidBytes)
        return wrapWithHeader(payload, flags, ASSET_DEF_ID_SCHEMA_HASH)
    }

    /**
     * Computes a 16-byte norito schema hash from a Rust type name using FNV-1a,
     * matching `norito::core::compute_schema_hash`.
     */
    @JvmStatic
    fun schemaHashForRustType(rustTypeName: String): ByteArray = SchemaHash.hash16(rustTypeName)

    /**
     * Encodes a fixed-size byte array where each element gets a u64 length prefix of 1.
     * This matches the Norito `[u8; N]` wire format.
     */
    private fun encodeFixedByteArrayPayload(flags: Int, bytes: ByteArray): ByteArray {
        val child = NoritoEncoder(flags)
        for (b in bytes) {
            child.writeUInt(1L, 64)
            child.writeByte(b.toInt())
        }
        return child.toByteArray()
    }

    /**
     * Encodes AssetBalanceScope::Global as a length-prefixed struct field.
     * Global is a unit enum variant: u32(0) with no payload, wrapped in u64 length prefix.
     */
    private fun encodeAssetBalanceScopeGlobal(encoder: NoritoEncoder) {
        val child = NoritoEncoder(0)
        UINT32_ADAPTER.encode(child, 0L)
        val payload = child.toByteArray()
        encoder.writeUInt(payload.size.toLong(), 64)
        encoder.writeBytes(payload)
    }

    /**
     * Encodes AccountId which is `#[norito(transparent)]` over AccountController.
     * The transparent attribute means AccountId serializes directly as its inner field
     * without an extra length prefix wrapper.
     * AccountController::Single(PublicKey) = u32(0) + length-prefixed PublicKey string.
     */
    private fun encodeAccountIdPayload(flags: Int, publicKeyHex: String): ByteArray {
        val publicKey = decodePublicKeyLiteral(publicKeyHex)
            ?: throw IllegalArgumentException("Invalid public key literal: $publicKeyHex")
        val canonicalMultihash =
            encodePublicKeyMultihash(publicKey.curveId, publicKey.keyBytes)

        val controllerEncoder = NoritoEncoder(flags)
        UINT32_ADAPTER.encode(controllerEncoder, 0L)
        val publicKeyEncoder = NoritoEncoder(flags)
        STRING_ADAPTER.encode(publicKeyEncoder, canonicalMultihash)
        val publicKeyBytes = publicKeyEncoder.toByteArray()
        controllerEncoder.writeUInt(publicKeyBytes.size.toLong(), 64)
        controllerEncoder.writeBytes(publicKeyBytes)

        return controllerEncoder.toByteArray()
    }

    private fun wrapWithHeader(payload: ByteArray, flags: Int, schemaHash: ByteArray): String {
        val checksum = CRC64.compute(payload)
        val header = NoritoHeader(schemaHash, payload.size, checksum, flags, NoritoHeader.COMPRESSION_NONE)
        val headerBytes = header.encode()

        val result = ByteArray(headerBytes.size + payload.size)
        headerBytes.copyInto(result)
        payload.copyInto(result, headerBytes.size)

        return NORITO_PREFIX + bytesToHex(result)
    }

    private fun bytesToHex(bytes: ByteArray): String = buildString(bytes.size * 2) {
        for (b in bytes) {
            append("%02x".format(b.toInt() and 0xFF))
        }
    }
}
